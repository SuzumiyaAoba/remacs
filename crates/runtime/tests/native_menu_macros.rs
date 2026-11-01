use remacs_runtime::{
    frontend::{GridMenu, GridMenuItem, InputEvent, KeyIo, render_grid, run_editor_with},
    lisp::Interp,
};
use std::{collections::VecDeque, io, time::Duration};

enum Step {
    Key(i128),
    Menu(&'static str),
    Command(&'static str),
}
struct Frontend {
    menus: Vec<GridMenu>,
    steps: VecDeque<Step>,
}

fn item_named<'a>(items: &'a [GridMenuItem], name: &str) -> Option<&'a GridMenuItem> {
    for item in items {
        if item.label == name {
            return Some(item);
        }
        if let Some(found) = item_named(&item.items, name) {
            return Some(found);
        }
    }
    None
}

impl KeyIo for Frontend {
    fn poll_key(&mut self, duration: Duration) -> io::Result<Option<i128>> {
        match self.poll_input(duration)? {
            Some(InputEvent::Key(key)) => Ok(Some(key)),
            _ => panic!("expected a key"),
        }
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let event = match self.steps.pop_front().ok_or(io::ErrorKind::BrokenPipe)? {
            Step::Key(key) => InputEvent::Key(key),
            Step::Command(name) => InputEvent::Command(name.into()),
            Step::Menu(label) => {
                let (name, token) = self
                    .menus
                    .iter()
                    .find_map(|menu| {
                        if menu.label == label {
                            Some((&menu.command, &menu.command_id))
                        } else {
                            item_named(&menu.items, label)
                                .map(|item| (&item.command, &item.command_id))
                        }
                    })
                    .expect("menu item must be visible");
                if let Some(id) = token {
                    InputEvent::NativeMenuCommand(id.clone())
                } else {
                    InputEvent::Command(name.clone().expect("selectable menu item"))
                }
            }
        };
        Ok(Some(event))
    }
    fn unread(&mut self, key: i128) {
        self.steps.push_front(Step::Key(key));
    }
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.menus = render_grid(interp, 80, 24).menus;
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

fn run(interp: &mut Interp, steps: Vec<Step>) {
    let error = run_editor_with(
        Frontend {
            menus: Vec::new(),
            steps: steps.into(),
        },
        interp,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}
fn check(interp: &mut Interp, expression: &str) {
    let passed = interp.eval_str(expression).unwrap().truthy();
    let state = interp
        .eval_str("(list (buffer-string) last-kbd-macro)")
        .unwrap();
    assert!(
        passed,
        "{expression}: {}; message: {}",
        interp.prin1_to_string(&state),
        interp.echo_message
    );
}
fn record(items: Vec<Step>) -> Vec<Step> {
    let mut steps = vec![Step::Key(24), Step::Key(40)];
    steps.extend(items);
    steps.extend([
        Step::Key(24),
        Step::Key(41),
        Step::Command("call-last-kbd-macro"),
    ]);
    steps
}

#[test]
fn native_menu_selection_is_recorded_and_replayed_as_input() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let mut interp = Interp::new_with_noninteractive(false);
        interp.eval_str(r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq pre-redisplay-function nil menu-bar-update-hook nil qa-menu-events nil)
          (setq-local fontification-functions nil)
          (erase-buffer)
          (defun qa-menu-insert-x () (interactive)
            (setq qa-menu-events (append qa-menu-events
              (list (list (this-command-keys-vector) last-command-event))))
            (insert "x"))
          (defun qa-menu-insert-y () (interactive) (insert "y"))
          (defun qa-menu-root () (interactive) (insert "r"))
          (defun qa-menu-repeat (count) (interactive "p") (insert (make-string count ?z)))
          (defun qa-menu-prompt (text) (interactive "sMenu value: ") (insert text))
          (defun qa-menu-event (event) (interactive "e")
            (setq qa-menu-parameter-events (append qa-menu-parameter-events (list event))) (insert "e"))
          (defun qa-append-menu-macro () (interactive) (start-kbd-macro t t))
          (defun qa-menu-cancel () (interactive) (cancel-kbd-macro-events) (insert "c"))
          (setq qa-menu-map (make-sparse-keymap))
          (define-key qa-menu-map [insert-x] '(menu-item "Insert x" qa-menu-insert-x))
          (define-key qa-menu-map [other-x] '(menu-item "Other x" qa-menu-insert-x))
          (define-key qa-menu-map [repeat] '(menu-item "Repeat" qa-menu-repeat))
          (define-key qa-menu-map [prompt] '(menu-item "Prompt" qa-menu-prompt))
          (define-key qa-menu-map [event] '(menu-item "Event" qa-menu-event))
          (define-key qa-menu-map [cancel] '(menu-item "Cancel" qa-menu-cancel))
          (let ((map (make-sparse-keymap "More"))
                (command (eval '(let ((counter 0))
                                 (lambda () (interactive)
                                   (setq counter (1+ counter)) (insert (number-to-string counter)))) t)))
            (define-key map [97] (list 'menu-item "Nested closure" command))
            (define-key qa-menu-map [more] (cons "More" map)))
          (define-key global-map [menu-bar qa-macro] (cons "QA Macro" qa-menu-map))
          (define-key global-map [menu-bar qa-root] '(menu-item "Root" qa-menu-root))
          (menu-bar-mode 1))"#).unwrap();

        // The issue's exact C-x ( / menu / C-x ) sequence, with GNU's
        // parameterized prefix and no recording of the stop keys.
        run(&mut interp, record(vec![Step::Menu("Insert x")]));
        check(&mut interp, r#"(and (equal (buffer-string) "xx")
          (equal last-kbd-macro '[(menu-bar) qa-macro insert-x])
          (equal qa-menu-events
            '(([(menu-bar) qa-macro insert-x] insert-x)
              ([(menu-bar) qa-macro insert-x] insert-x))))"#);
        interp.eval_str(r#"(progn
          (setq qa-recorded-menu-macro last-kbd-macro)
          (erase-buffer)
          (define-key qa-menu-map [insert-x] '(menu-item "Insert x" qa-menu-insert-y))
          (execute-kbd-macro qa-recorded-menu-macro))"#).unwrap();
        check(&mut interp, r#"(equal (buffer-string) "y")"#);

        // Root commands and nested closures keep their complete key path.
        interp.eval_str("(erase-buffer)").unwrap();
        run(&mut interp, record(vec![Step::Menu("Root"), Step::Menu("Nested closure")]));
        check(&mut interp, r#"(and (equal (buffer-string) "r1r2")
          (equal last-kbd-macro '[(menu-bar) qa-root (menu-bar) qa-macro more 97]))"#);

        // Numeric prefix digits are recorded even on the argument fast path.
        interp.eval_str("(erase-buffer)").unwrap();
        run(&mut interp, record(vec![Step::Key(21), Step::Key(51), Step::Menu("Repeat")]));
        check(&mut interp, r#"(and (equal (buffer-string) "zzzzzz")
          (equal last-kbd-macro '[21 51 (menu-bar) qa-macro repeat]))"#);

        // Argument reads consume recorded macro input before device input.
        interp.eval_str("(erase-buffer)").unwrap();
        let mut prompt = vec![Step::Menu("Prompt")];
        prompt.extend("選択\r".chars().map(|ch| Step::Key(ch as i128)));
        run(&mut interp, record(prompt));
        check(&mut interp, r#"(and (equal (buffer-string) "選択選択")
          (equal last-kbd-macro '[(menu-bar) qa-macro prompt 36984 25246 13]))"#);

        interp.eval_str("(erase-buffer) (setq qa-menu-parameter-events nil)").unwrap();
        run(&mut interp, record(vec![Step::Menu("Event")]));
        check(&mut interp, r#"(and (equal (buffer-string) "ee")
          (equal qa-menu-parameter-events '((menu-bar) (menu-bar))))"#);

        // Append preserves vector events, including an append with no new input.
        interp.eval_str(r#"(progn (erase-buffer)
          (define-key qa-menu-map [insert-x] '(menu-item "Insert x" qa-menu-insert-x))
          (setq last-kbd-macro qa-recorded-menu-macro))"#).unwrap();
        run(&mut interp, vec![Step::Command("qa-append-menu-macro"), Step::Menu("Root"),
          Step::Key(24), Step::Key(41), Step::Command("call-last-kbd-macro")]);
        check(&mut interp, r#"(and (equal (buffer-string) "rxr")
          (equal last-kbd-macro '[(menu-bar) qa-macro insert-x (menu-bar) qa-root]))"#);
        interp.eval_str("(setq qa-appended-menu-macro last-kbd-macro)").unwrap();
        run(&mut interp, vec![Step::Command("qa-append-menu-macro"), Step::Key(24), Step::Key(41)]);
        check(&mut interp, "(equal last-kbd-macro qa-appended-menu-macro)");

        interp.eval_str("(erase-buffer)").unwrap();
        run(&mut interp, vec![Step::Command("start-kbd-macro"), Step::Menu("Cancel"),
            Step::Command("end-kbd-macro")]);
        check(&mut interp, r#"(and (equal (buffer-string) "c") (equal last-kbd-macro []))"#);
    }).unwrap().join().unwrap();
}
