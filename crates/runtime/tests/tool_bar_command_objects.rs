use remacs_runtime::{
    frontend::{
        Grid, InputEvent, KeyIo, NativeMenuCommandId, WeakNativeMenuCommandId,
        prepare_native_menus, render_grid, run_editor_with_shared,
    },
    lisp::{Flow, Interp, Value},
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{source}: {} {}",
            i.prin1_to_string(&signal.symbol()),
            i.prin1_to_string(&signal.data())
        ),
        other => panic!("{source}: {other:?}"),
    })
}

fn token(grid: &Grid, label: &str) -> NativeMenuCommandId {
    grid.tool_bar_items
        .iter()
        .find(|item| {
            let (start, end) = item.byte_range.unwrap();
            grid.rows[item.row][start..end].trim() == label
        })
        .unwrap_or_else(|| panic!("missing toolbar {label}: {:?}", grid.rows))
        .command_id
        .clone()
        .expect("command object")
}

enum Step {
    Click(&'static str),
    Replace,
    Saved,
}
struct Frontend {
    grid: Grid,
    steps: VecDeque<Step>,
    saved: Option<NativeMenuCommandId>,
    previous: Option<WeakNativeMenuCommandId>,
}
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        Ok(Some(
            match self.steps.pop_front().ok_or(io::ErrorKind::BrokenPipe)? {
                Step::Click(label) => InputEvent::NativeMenuCommand(token(&self.grid, label)),
                Step::Replace => {
                    let command = token(&self.grid, "Captured");
                    self.previous = Some(command.downgrade());
                    self.saved = Some(command);
                    InputEvent::Command("qa-tool-replace".into())
                }
                Step::Saved => {
                    let previous = self.saved.take().unwrap();
                    assert_ne!(previous, token(&self.grid, "Captured"));
                    InputEvent::NativeMenuCommand(previous)
                }
            },
        ))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        self.grid = render_grid(i, 120, 24);
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (120, 24)
    }
}

#[test]
fn tool_bar_objects_keep_captures_keys_and_snapshot_lifetimes() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 2)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap)
              qa-tool-raw 0 qa-tool-symbol 0 qa-tool-uninterned 0 qa-tool-results nil)
        (defun qa-tool-captured (initial)
          (eval `(let ((count ,initial))
            (lambda (prefix event) (interactive "p\ne")
              (setq count (+ count prefix))
              (let ((keys (this-command-keys-vector)))
                (setq qa-tool-results
                  (append qa-tool-results
                    (list (list count prefix
                      (and (= (length keys) 2) (eq (aref keys 0) 'tool-bar)
                        (eq (car event) 'captured) (eq event (aref keys 1))
                        (eq (car (cadr event)) (selected-frame))
                        (equal (cadr (cadr event)) '(tool-bar)))))))))) t))
        (setq qa-tool-first (qa-tool-captured 40) qa-tool-second (qa-tool-captured 100))
        (define-key tool-bar-map [raw]
          '(menu-item "Run lambda" (lambda () (interactive)
            (setq qa-tool-raw (1+ qa-tool-raw)) (message "ran"))))
        (define-key tool-bar-map [captured] (list 'menu-item "Captured" qa-tool-first))
        (defun qa-tool-symbol-command () (interactive) (setq qa-tool-symbol (1+ qa-tool-symbol)))
        (define-key tool-bar-map [symbol] '(menu-item "Symbol" qa-tool-symbol-command))
        (let ((command (make-symbol "ignore")))
          (fset command (lambda () (interactive)
            (setq qa-tool-uninterned (1+ qa-tool-uninterned))))
          (define-key tool-bar-map [uninterned] (list 'menu-item "Uninterned" command)))
        (define-key tool-bar-map [disabled] '(menu-item "Disabled" (lambda () (interactive)) :enable nil))
        (define-key tool-bar-map [hidden] '(menu-item "Hidden" (lambda () (interactive)) :visible nil))
        (defun qa-tool-replace () (interactive)
          (define-key tool-bar-map [captured] (list 'menu-item "Captured" qa-tool-second))
          (setq qa-tool-first nil prefix-arg 7)))"#,
    );
    prepare_native_menus(&mut i).unwrap();
    let first = render_grid(&i, 120, 24);
    assert!(
        first.menus.is_empty(),
        "toolbar must not depend on menu-bar-mode"
    );
    let captured = token(&first, "Captured");
    prepare_native_menus(&mut i).unwrap();
    let refreshed = render_grid(&i, 120, 24);
    assert_eq!(
        captured,
        token(&refreshed, "Captured"),
        "refresh keeps press identity"
    );
    assert!(!refreshed.rows.iter().any(|row| row.contains("Hidden")));
    let disabled = refreshed
        .tool_bar_items
        .iter()
        .find(|item| {
            let (a, b) = item.byte_range.unwrap();
            refreshed.rows[item.row][a..b].trim() == "Disabled"
        })
        .unwrap();
    assert!(!disabled.enabled, "disabled toolbar item must remain disabled");
    assert!(disabled.command.is_empty() && disabled.command_id.is_none());
    let enabled = refreshed
        .tool_bar_items
        .iter()
        .find(|item| {
            let (a, b) = item.byte_range.unwrap();
            refreshed.rows[item.row][a..b].trim() == "Symbol"
        })
        .unwrap();
    assert_ne!(
        refreshed.attrs[disabled.row][disabled.left + 1].fg,
        refreshed.attrs[enabled.row][enabled.left + 1].fg,
        "disabled toolbar item must use the shadow foreground"
    );
    drop((captured, first, refreshed));
    let frontend = Rc::new(RefCell::new(Frontend {
        grid: Grid::blank(120, 24),
        saved: None,
        previous: None,
        steps: VecDeque::from([
            Step::Click("Run lambda"),
            Step::Replace,
            Step::Saved,
            Step::Click("Captured"),
            Step::Click("Uninterned"),
            Step::Click("Symbol"),
        ]),
    }));
    let failure = run_editor_with_shared(frontend.clone(), &mut i).unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::BrokenPipe);
    assert!(
        eval(
            &mut i,
            r#"(and (= qa-tool-raw 1) (= qa-tool-symbol 1) (= qa-tool-uninterned 1)
        (equal qa-tool-results '((47 7 t) (101 1 t))))"#
        )
        .truthy()
    );
    assert!(
        frontend
            .borrow()
            .previous
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none(),
        "old action released"
    );
    eval(&mut i, "(setq tool-bar-map (make-sparse-keymap))");
    prepare_native_menus(&mut i).unwrap();
    assert!(render_grid(&i, 120, 24).tool_bar_items.is_empty());
    // Each graphical frame resolves its selected buffer's map. Callbacks see
    // that frame/window, while the caller's unrelated buffer and point survive.
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(progn
        (setq qa-tool-contexts nil)
        (defun qa-tool-context-filter (command)
          (setq qa-tool-contexts (cons (list (buffer-name)
            (eq (selected-frame) menu-updating-frame) (point)) qa-tool-contexts))
          command)
        (setq qa-tool-shared '(lambda () (interactive)))
        (with-current-buffer (get-buffer-create "tool-local-one")
          (insert "one") (goto-char 3) (setq-local tool-bar-map (make-sparse-keymap))
          (define-key tool-bar-map [local]
            (list 'menu-item "Local one" qa-tool-shared :filter 'qa-tool-context-filter)))
        (with-current-buffer (get-buffer-create "tool-local-two")
          (insert "second") (goto-char 5) (setq-local tool-bar-map (make-sparse-keymap))
          (define-key tool-bar-map [local]
            (list 'menu-item "Local two" qa-tool-shared :filter 'qa-tool-context-filter)))
        (set-window-buffer (selected-window) "tool-local-one")
        (setq qa-tool-other-frame (make-frame '((window-system . ns) (tool-bar-lines . 1)
          (menu-bar-lines . 0) (tab-bar-lines . 0) (width . 120) (height . 24))))
        (set-window-buffer (frame-selected-window qa-tool-other-frame) "tool-local-two")
        (set-window-point (frame-selected-window qa-tool-other-frame) 4)
        (set-buffer (get-buffer-create "tool-caller")) (insert "caller") (goto-char 2)
        qa-tool-other-frame)"#,
    ) else {
        panic!("other toolbar frame")
    };
    let caller = i.current_buffer;
    prepare_native_menus(&mut i).unwrap();
    assert_eq!(i.current_buffer, caller);
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), 1);
    assert!(Rc::ptr_eq(i.selected_frame.as_ref().unwrap(), &primary));
    let first =
        remacs_runtime::frontend::render_frame_grid_with_input_cached(&i, &primary, 120, 24, None);
    let second =
        remacs_runtime::frontend::render_frame_grid_with_input_cached(&i, &other, 120, 24, None);
    assert_eq!(first.tool_bar_items.len(), 1);
    assert_eq!(second.tool_bar_items.len(), 1);
    assert_ne!(
        token(&first, "Local one"),
        token(&second, "Local two"),
        "originating frame is part of command identity"
    );
    assert!(
        eval(
            &mut i,
            r#"(and (member '("tool-local-one" t 3) qa-tool-contexts)
                               (member '("tool-local-two" t 4) qa-tool-contexts))"#
        )
        .truthy()
    );
}
