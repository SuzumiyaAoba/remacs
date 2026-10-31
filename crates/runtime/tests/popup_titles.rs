use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::{Interp, Value},
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn rows(interp: &Interp) -> Vec<(String, bool)> {
    let grid = render_grid(interp, 80, 24);
    grid.popup_menu_items
        .iter()
        .map(|item| {
            (
                grid.rows[item.row]
                    .chars()
                    .skip(item.left)
                    .take(item.width)
                    .collect::<String>()
                    .trim()
                    .to_owned(),
                item.selection.is_some(),
            )
        })
        .collect()
}

fn caption(rows: &[(String, bool)], expected: &str) {
    assert_eq!(rows.first(), Some(&(expected.to_owned(), false)));
    assert!(!rows[1].0.is_empty() && rows[1].0.chars().all(|ch| ch == '─'));
    assert!(!rows[1].1, "the title separator is not a choice");
}

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    polls: usize,
}
impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let grid = self.grid.as_ref().unwrap();
        self.polls += 1;
        let (menu_id, index) = if self.polls == 1 {
            (grid.popup_menu_items[0].menu_id, Some(0))
        } else {
            let item = grid
                .popup_menu_items
                .iter()
                .find(|item| {
                    grid.rows[item.row]
                        .chars()
                        .skip(item.left)
                        .take(item.width)
                        .collect::<String>()
                        .trim()
                        == "Second"
                })
                .unwrap();
            (item.menu_id, item.selection)
        };
        Ok(Some(InputEvent::PopupMenuSelection { menu_id, index }))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.grid = Some(render_grid(interp, 80, 24));
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

#[test]
fn popup_captions_are_visible_nonselectable_and_preserve_both_menu_formats() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(&mut interp, "(set-frame-parameter nil 'window-system 'ns)");
            eval(
                &mut interp,
                r#"(x-popup-menu t '("Choices"
          ("Pane" ("First" . first-choice) ("Second" . second-choice))))"#,
            );
            let rendered = rows(&interp);
            caption(&rendered, "Choices");
            assert_eq!(
                rendered[2..],
                [
                    ("Pane".into(), false),
                    ("First".into(), true),
                    ("Second".into(), true)
                ]
            );

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-first (make-sparse-keymap "First")
                qa-title-second (make-sparse-keymap "Second"))
          (define-key qa-title-first [one] '(menu-item "One" ignore))
          (define-key qa-title-second [two] '(menu-item "Two" ignore))
          (x-popup-menu t (list qa-title-first qa-title-second)))"#,
            );
            let rendered = rows(&interp);
            caption(&rendered, "First");
            assert_eq!(
                rendered
                    .iter()
                    .filter(|(_, choice)| *choice)
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>(),
                ["One", "Two"]
            );

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-first (make-sparse-keymap))
          (define-key qa-title-first [one] '(menu-item "One" ignore))
          (x-popup-menu t (list qa-title-first qa-title-second)))"#,
            );
            caption(&rows(&interp), "Second");

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-mutating (make-sparse-keymap "Before"))
          (define-key qa-title-mutating [item]
            '(menu-item "Item" ignore :visible (progn (setcar (last qa-title-mutating) "After") t)))
          (x-popup-menu t qa-title-mutating))"#,
            );
            caption(&rows(&interp), "After");
            eval(
                &mut interp,
                r#"(progn
          (setcar (last qa-title-mutating) "Before")
          (x-popup-menu t (list qa-title-mutating)))"#,
            );
            caption(&rows(&interp), "Before");

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-string (copy-sequence "Before")
                qa-title-mutating (make-sparse-keymap qa-title-string))
          (define-key qa-title-mutating [item]
            '(menu-item "Item" ignore :visible (progn (aset qa-title-string 0 ?b) t)))
          (x-popup-menu t (list qa-title-mutating)))"#,
            );
            caption(&rows(&interp), "before");

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-hidden (make-sparse-keymap (propertize "Hidden caption" 'hide t)))
          (define-key qa-title-hidden [item] '(menu-item "Item" ignore))
          (x-popup-menu t qa-title-hidden))"#,
            );
            assert_eq!(rows(&interp), [("Item".into(), true)]);
            eval(&mut interp, "(x-popup-menu t (list qa-title-hidden))");
            caption(&rows(&interp), "Hidden caption");

            eval(
                &mut interp,
                r#"(progn
          (setq qa-title-empty (make-sparse-keymap ""))
          (define-key qa-title-empty [item] '(menu-item "Item" ignore))
          (x-popup-menu t qa-title-empty))"#,
            );
            caption(&rows(&interp), "");
            eval(
                &mut interp,
                "(x-popup-menu t (make-sparse-keymap \"Empty menu\"))",
            );
            let rendered = rows(&interp);
            caption(&rendered, "Empty menu");
            assert_eq!(rendered.len(), 2);

            interp.gui_popup_menu = None;
            let input = Rc::new(RefCell::new(Input::default()));
            install_minibuf_reader(&input, &mut interp);
            let choice = eval(
                &mut interp,
                r#"(x-popup-menu t '("Choices"
          ("Pane" ("First" . first-choice) ("Second" . second-choice))))"#,
            );
            assert_eq!(interp.prin1_to_string(&choice), "second-choice");
            assert_eq!(
                input.borrow().polls,
                2,
                "a title index cannot select an item"
            );
            assert!(interp.gui_popup_menu.is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}
