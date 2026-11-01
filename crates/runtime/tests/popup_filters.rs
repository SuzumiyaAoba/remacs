use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::{Interp, Value},
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

type Row = (String, Option<String>, bool);

struct Library(std::path::PathBuf);
impl Library {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "remacs-popup-filter-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    frames: Vec<Vec<Row>>,
    cancel: bool,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        if self.cancel {
            return Ok(Some(InputEvent::Key(27)));
        }
        let parent = self
            .grid
            .as_ref()
            .unwrap()
            .popup_menu_items
            .last()
            .and_then(|item| item.parent);
        let item = self
            .grid
            .as_ref()
            .unwrap()
            .popup_menu_items
            .iter()
            .find(|item| item.parent == parent && item.selection.is_some())
            .ok_or_else(|| io::Error::other("popup has no selectable item"))?;
        Ok(Some(InputEvent::PopupMenuSelection {
            menu_id: item.menu_id,
            index: item.selection,
        }))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        let grid = render_grid(interp, 80, 24);
        self.frames.push(
            grid.popup_menu_items
                .iter()
                .map(|item| {
                    let label = grid.rows[item.row]
                        .chars()
                        .skip(item.left)
                        .take(item.width)
                        .collect::<String>()
                        .trim()
                        .to_owned();
                    (label, item.command.clone(), item.selection.is_some())
                })
                .collect(),
        );
        self.grid = Some(grid);
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn open(interp: &mut Interp, input: &Rc<RefCell<Input>>, cancel: bool, source: &str) -> Value {
    input.borrow_mut().frames.clear();
    input.borrow_mut().cancel = cancel;
    let value = eval(interp, source);
    assert!(interp.gui_popup_menu.is_none());
    value
}

fn displayed(input: &Rc<RefCell<Input>>) -> Vec<Row> {
    input
        .borrow()
        .frames
        .iter()
        .find(|rows| !rows.is_empty())
        .cloned()
        .unwrap_or_default()
}

fn opened_submenu(input: &Rc<RefCell<Input>>, label: &str) -> Vec<Row> {
    input
        .borrow()
        .frames
        .iter()
        .find(|rows| rows.iter().any(|row| row.0 == label))
        .cloned()
        .expect("submenu must be opened before selecting its leaf")
}

#[test]
fn popup_filters_replace_bindings_before_display_and_keep_gnu_evaluation_order() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(|| {
        let mut interp = Interp::new();
        eval(&mut interp, r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq qa-filter-ran nil qa-filter-calls 0 qa-filter-log nil)
          (defun qa-filter-target () (interactive) (setq qa-filter-ran t))
          (defun qa-filter-command (binding)
            (setq qa-filter-calls (1+ qa-filter-calls))
            'qa-filter-target))"#);
        let input = Rc::new(RefCell::new(Input::default()));
        install_minibuf_reader(&input, &mut interp);

        let value = open(&mut interp, &input, false, r#"(let ((map (make-sparse-keymap)))
          (define-key map [run] '(menu-item "Run filtered command" ignore :filter qa-filter-command))
          (popup-menu map)
          qa-filter-ran)"#);
        assert!(value.truthy(), "the public popup-menu wrapper must execute the filtered command");
        assert_eq!(displayed(&input), [("Run filtered command".into(), Some("qa-filter-target".into()), true)]);

        let value = open(&mut interp, &input, false, r#"(progn
          (setq qa-filter-ran nil qa-filter-log nil)
          (let ((map (make-sparse-keymap)) (child (make-sparse-keymap)))
            (define-key child [leaf] '(menu-item "Filtered leaf" qa-filter-target))
            (define-key map [branch]
              (list 'menu-item "Branch" 'ignore :filter (lambda (_binding) child)))
            (list (x-popup-menu t map) qa-filter-ran)))"#);
        assert_eq!(interp.prin1_to_string(&value), "((branch leaf) nil)");
        assert_eq!(displayed(&input), [("Branch ▸".into(), None, true)]);
        assert_eq!(opened_submenu(&input, "Filtered leaf"), [
            ("Branch ▸".into(), None, true),
            ("Filtered leaf".into(), Some("qa-filter-target".into()), true),
        ]);

        let library = Library::new();
        let path = library.0.join("filtered-map.el");
        std::fs::write(&path, r#";; -*- lexical-binding: t; -*-
          (setq qa-filter-autoloaded t)
          (let ((map (make-sparse-keymap)))
            (define-key map [loaded] '(menu-item "Loaded leaf" ignore))
            (fset 'qa-filter-autoload map))"#).unwrap();
        let filename = interp.prin1_to_string(&Value::string(path.to_str().unwrap())).to_unicode_lossy();
        let source = format!(r#"(progn
          (setq qa-filter-autoloaded nil)
          (autoload 'qa-filter-autoload {filename} nil nil 'keymap)
          (let ((map (make-sparse-keymap)))
            (define-key map [auto]
              '(menu-item "Autoload branch" ignore :filter (lambda (_) 'qa-filter-autoload)))
            (list (x-popup-menu t map) qa-filter-autoloaded)))"#);
        let value = open(&mut interp, &input, false, &source);
        assert_eq!(interp.prin1_to_string(&value), "((auto loaded) t)");
        assert_eq!(displayed(&input), [("Autoload branch ▸".into(), None, true)]);
        assert_eq!(opened_submenu(&input, "Loaded leaf"), [
            ("Autoload branch ▸".into(), None, true),
            ("Loaded leaf".into(), Some("ignore".into()), true),
        ]);

        let value = open(&mut interp, &input, false, r#"(progn
          (setq qa-filter-log nil)
          (let ((map (make-sparse-keymap)))
            (define-key map [ordered]
              '(menu-item (progn (push 'name qa-filter-log) "Dynamic name") ignore
                 :enable (progn (push 'enable qa-filter-log) t)
                 :filter (lambda (binding)
                           (push (list 'filter binding inhibit-redisplay) qa-filter-log)
                           'qa-filter-target)
                 :visible (progn (push 'visible qa-filter-log) t)))
            (list (x-popup-menu t map) (reverse qa-filter-log))))"#);
        assert_eq!(interp.prin1_to_string(&value), "((ordered) (visible name (filter ignore t) enable))");
        assert_eq!(displayed(&input)[0].0, "Dynamic name");

        let value = open(&mut interp, &input, false, r#"(progn
          (setq qa-filter-log nil qa-filter-global 'qa-filter-target)
          (eval '(let ((qa-filter-global 'ignore) (map (make-sparse-keymap)))
                   (define-key map [dynamic]
                     '(menu-item "Global filter" nil :filter (lambda (binding)
                         (push binding qa-filter-log) qa-filter-global)))
                   (x-popup-menu t map)) t))"#);
        assert_eq!(interp.prin1_to_string(&value), "(dynamic)");
        assert_eq!(displayed(&input)[0].1.as_deref(), Some("qa-filter-target"));
        let log = eval(&mut interp, "qa-filter-log");
        assert_eq!(interp.prin1_to_string(&log), "(nil)");

        let value = open(&mut interp, &input, false, r#"(progn
          (setq qa-filter-original (list 'not-a-function) qa-filter-identity nil)
          (let ((map (make-sparse-keymap)))
            (define-key map [original]
              (list 'menu-item "Original object" qa-filter-original
                    :filter (lambda (binding)
                              (setq qa-filter-identity (eq binding qa-filter-original))
                              'qa-filter-target)))
            (list (x-popup-menu t map) qa-filter-identity)))"#);
        assert_eq!(interp.prin1_to_string(&value), "((original) t)");

        let value = open(&mut interp, &input, false, r#"(progn
          (setq qa-filter-live
            (list 'menu-item
                  '(progn (setcar (nthcdr 6 qa-filter-live) 'qa-filter-command) "Live filter")
                  'ignore :filter (lambda (_) (error "old filter"))
                  :filter (lambda (_) (error "replaced by name"))))
          (x-popup-menu t (list 'keymap (cons 'live qa-filter-live))))"#);
        assert_eq!(interp.prin1_to_string(&value), "(live)");
        assert_eq!(displayed(&input)[0].1.as_deref(), Some("qa-filter-target"));

        open(&mut interp, &input, true, r#"(progn
          (setq qa-filter-log nil)
          (let ((map (make-sparse-keymap)))
            (define-key map [hidden]
              '(menu-item "Hidden" ignore :filter (lambda (_) (push 'hidden-filter qa-filter-log)
                                                        (error "hidden filter")) :visible nil))
            (define-key map [disabled]
              '(menu-item "Disabled" ignore
                 :filter (lambda (_) (push 'filtered qa-filter-log) 'qa-filter-target)
                 :enable (progn (push 'enabled qa-filter-log) nil)))
            (condition-case nil (x-popup-menu t map) (quit nil))))"#);
        assert_eq!(displayed(&input), [("Disabled".into(), Some("qa-filter-target".into()), false)]);
        let log = eval(&mut interp, "(reverse qa-filter-log)");
        assert_eq!(interp.prin1_to_string(&log), "(filtered enabled)");

        open(&mut interp, &input, true, r#"(let ((map (make-sparse-keymap)))
          (define-key map [empty] '(menu-item "Nil result" ignore :filter (lambda (_) nil)))
          (define-key map [error] '(menu-item "Error result" ignore :filter (lambda (_) (error "filter"))))
          (condition-case nil (x-popup-menu t map) (quit nil)))"#);
        assert_eq!(displayed(&input), [
            ("Error result".into(), None, false),
            ("Nil result".into(), None, false),
        ]);

        let quit = open(&mut interp, &input, true, r#"(condition-case nil
          (x-popup-menu t '(keymap (item menu-item "Quit filter" ignore
                                       :filter (lambda (_) (signal 'quit nil)))))
          (quit 'quit))"#);
        assert_eq!(interp.prin1_to_string(&quit), "quit");
        assert!(displayed(&input).is_empty());
        assert!(eval(&mut interp, "inhibit-redisplay").is_nil());
    }).unwrap().join().unwrap();
}
