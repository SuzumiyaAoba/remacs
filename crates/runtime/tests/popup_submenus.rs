use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::Interp,
};
use std::{cell::RefCell, io, rc::Rc, time::Duration};

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    labels: Vec<String>,
    stage: usize,
}

impl Input {
    fn shown(&self, label: &str) -> bool {
        self.grid
            .as_ref()
            .unwrap()
            .popup_menu_items
            .iter()
            .any(|item| {
                item.selection
                    .is_some_and(|index| self.labels[index] == label)
            })
    }
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let grid = self.grid.as_ref().unwrap();
        let menu_id = grid.popup_menu_items[0].menu_id;
        let label = if self.stage == 0 {
            assert!(
                !self.shown("First"),
                "submenu children must be hidden until their parent opens"
            );
            "More"
        } else {
            assert_eq!(self.stage, 1);
            assert!(self.shown("First"));
            let parent = grid
                .popup_menu_items
                .iter()
                .find(|item| {
                    item.selection
                        .is_some_and(|index| self.labels[index] == "More")
                })
                .unwrap();
            let child = grid
                .popup_menu_items
                .iter()
                .find(|item| {
                    item.selection
                        .is_some_and(|index| self.labels[index] == "First")
                })
                .unwrap();
            assert!(
                child.left > parent.left + parent.width,
                "submenu must occupy a separate column"
            );
            "First"
        };
        let index = self.labels.iter().position(|name| name == label).unwrap();
        assert!(
            grid.popup_menu_items
                .iter()
                .any(|item| item.selection == Some(index)),
            "parent must be actionable"
        );
        self.stage += 1;
        Ok(Some(InputEvent::PopupMenuSelection {
            menu_id,
            index: Some(index),
        }))
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.grid = Some(render_grid(interp, 80, 24));
        if let Some(menu) = &interp.gui_popup_menu {
            self.labels = menu
                .entries
                .iter()
                .map(|entry| entry.label.clone())
                .collect();
        }
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
fn submenu_opens_in_another_column_and_returns_the_complete_key_path() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq qa-popup-runs 0)
          (defun qa-popup-command () (interactive) (setq qa-popup-runs (1+ qa-popup-runs)))
          (setq qa-parent (make-sparse-keymap "Main") qa-child (make-sparse-keymap "More"))
          (define-key qa-child [first] '(menu-item "First" qa-popup-command))
          (define-key qa-parent [more] (cons "More" qa-child)))"#,
                )
                .unwrap();
            let input = Rc::new(RefCell::new(Input::default()));
            install_minibuf_reader(&input, &mut interp);
            let value = interp
                .eval_str("(list (x-popup-menu t qa-parent) qa-popup-runs)")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&value), "((more first) 0)");
            assert_eq!(input.borrow().stage, 2);
            assert!(interp.gui_popup_menu.is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[derive(Debug)]
enum Step {
    Hidden(&'static str),
    Column(&'static str, Option<&'static str>),
    Hover(Option<&'static str>, Option<&'static str>, bool),
    Click(&'static str),
    Key(&'static str),
    Resize(usize, usize),
    BackHeader,
    Scroll(&'static str, f32),
    RootOffset(usize),
}
struct ScriptInput {
    grid: Option<Grid>,
    labels: Vec<String>,
    steps: std::collections::VecDeque<Step>,
    width: usize,
    height: usize,
    redraw: bool,
    root_offset: usize,
}
impl ScriptInput {
    fn index(&self, label: &str) -> usize {
        self.labels
            .iter()
            .position(|name| name == label)
            .unwrap_or_else(|| panic!("missing entry {label}"))
    }
    fn visible(&self, label: &str) -> bool {
        let index = self.index(label);
        self.grid
            .as_ref()
            .unwrap()
            .popup_menu_items
            .iter()
            .any(|item| item.selection == Some(index))
    }
}
impl KeyIo for ScriptInput {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        loop {
            let step = self.steps.pop_front().ok_or(io::ErrorKind::BrokenPipe)?;
            let menu_id = self.grid.as_ref().unwrap().popup_menu_items[0].menu_id;
            let input = match step {
                Step::Hidden(label) => {
                    assert!(!self.visible(label), "{label} must remain collapsed");
                    continue;
                }
                Step::Column(label, parent) => {
                    let index = self.index(label);
                    let expected = parent.map(|name| self.index(name));
                    assert!(
                        self.grid
                            .as_ref()
                            .unwrap()
                            .popup_menu_items
                            .iter()
                            .any(|item| item.selection == Some(index) && item.parent == expected),
                        "{label}: expected column {parent:?}"
                    );
                    continue;
                }
                Step::Hover(parent, label, stale) => InputEvent::PopupMenuHover {
                    menu_id: menu_id + usize::from(stale) * 1000,
                    parent: parent.map(|name| self.index(name)),
                    index: label.map(|name| self.index(name)),
                },
                Step::Click(label) => {
                    assert!(
                        self.visible(label),
                        "{label} must be painted before a click"
                    );
                    InputEvent::PopupMenuSelection {
                        menu_id,
                        index: Some(self.index(label)),
                    }
                }
                Step::Key(name) => InputEvent::Key(match name {
                    "enter" => 13,
                    "escape" => 27,
                    "quit" => 7,
                    _ => remacs_runtime::editor::event_code_for(name),
                }),
                Step::Resize(width, height) => {
                    self.width = width;
                    self.height = height;
                    self.redraw = true;
                    return Ok(None);
                }
                Step::BackHeader => {
                    assert!(
                        self.grid
                            .as_ref()
                            .unwrap()
                            .popup_menu_items
                            .iter()
                            .any(|item| item.parent.is_some()
                                && item.selection == item.parent
                                && item.byte_range.is_some_and(|(start, end)| self
                                    .grid
                                    .as_ref()
                                    .unwrap()
                                    .rows[item.row][start..end]
                                    .starts_with('‹')))
                    );
                    continue;
                }
                Step::Scroll(parent, delta_lines) => InputEvent::PopupMenuScrollAt {
                    menu_id,
                    parent: Some(self.index(parent)),
                    delta_lines,
                },
                Step::RootOffset(offset) => {
                    assert_eq!(self.root_offset, offset);
                    continue;
                }
            };
            return Ok(Some(input));
        }
    }
    fn unread(&mut self, _: i128) {}
    fn take_render_request(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }
    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        let grid = render_grid(interp, self.width, self.height);
        for row in &grid.popup_menu_items {
            assert!(
                row.left + row.width <= self.width && row.row < self.height,
                "popup must remain in the viewport"
            );
            let (start, end) = row.byte_range.expect("painted row needs a UTF-8 hit range");
            assert!(grid.rows[row.row].get(start..end).is_some());
        }
        if let Some(menu) = &interp.gui_popup_menu {
            self.labels = menu
                .entries
                .iter()
                .map(|entry| entry.label.clone())
                .collect();
            self.root_offset = menu.scroll_offset.get();
        }
        self.grid = Some(grid);
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

fn scenario(interp: &mut Interp, source: &str, steps: Vec<Step>) -> String {
    let input = Rc::new(RefCell::new(ScriptInput {
        grid: None,
        labels: Vec::new(),
        steps: steps.into(),
        width: 80,
        height: 24,
        redraw: false,
        root_offset: 0,
    }));
    install_minibuf_reader(&input, interp);
    let value = interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("popup flow {flow:?}"));
    assert!(
        input.borrow().steps.is_empty(),
        "caller returned before navigation finished: {:?}",
        input.borrow().steps
    );
    assert!(interp.gui_popup_menu.is_none());
    interp.prin1_to_string(&value).to_unicode_lossy()
}

#[test]
fn submenus_support_hover_keyboard_back_resize_and_independent_scrolling() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq qa-runs 0)
          (defun qa-never-run () (interactive) (setq qa-runs (1+ qa-runs)))
          (setq qa-deep '(keymap "Nested" (deep menu-item "Deep" qa-never-run)))
          (setq qa-level (list 'keymap "More"
            '(first menu-item "First" qa-never-run)
            '(separator menu-item "--")
            (list 'nested 'menu-item "Nested" qa-deep)
            '(disabled menu-item "Disabled" nil)))
          (setq qa-main (list 'keymap "Main" (list 'more 'menu-item "More" qa-level))))"#,
                )
                .unwrap();
            use Step::*;
            assert_eq!(
                scenario(
                    &mut interp,
                    "(x-popup-menu t qa-main)",
                    vec![
                        Hidden("First"),
                        Hover(None, Some("More"), true),
                        Hidden("First"),
                        Hover(None, Some("More"), false),
                        Column("First", Some("More")),
                        Hidden("Deep"),
                        Hover(Some("More"), Some("Nested"), false),
                        Column("Deep", Some("Nested")),
                        Click("Deep"),
                    ]
                ),
                "(more nested deep)"
            );
            interp.eval_str("(setq qa-other '(keymap \"Other\" (other menu-item \"Other item\" qa-never-run)))").unwrap();
            assert_eq!(
                scenario(
                    &mut interp,
                    "(x-popup-menu t (list qa-other qa-main))",
                    vec![
                        Column("Other item", None),
                        Hidden("First"),
                        Click("More"),
                        Column("Other item", None),
                        Column("First", Some("More")),
                        Click("First"),
                    ],
                ),
                "(more first)"
            );
            assert_eq!(
                scenario(
                    &mut interp,
                    "(x-popup-menu t qa-main)",
                    vec![
                        Key("down"),
                        Key("right"),
                        Column("First", Some("More")),
                        Key("down"),
                        Key("right"),
                        Column("Deep", Some("Nested")),
                        Key("left"),
                        Hidden("Deep"),
                        Key("right"),
                        Key("escape"),
                        Hidden("Deep"),
                        Key("left"),
                        Hidden("First"),
                        Key("right"),
                        Key("enter"),
                    ]
                ),
                "(more first)"
            );
            assert_eq!(
                scenario(
                    &mut interp,
                    "(x-popup-menu t qa-main)",
                    vec![
                        Click("More"),
                        Resize(10, 4),
                        BackHeader,
                        Column("First", Some("More")),
                        Click("More"),
                        Hidden("First"),
                        Resize(80, 24),
                        Click("More"),
                        Click("First"),
                    ]
                ),
                "(more first)"
            );
            interp
                .eval_str(
                    r#"(progn
          (setq qa-many (make-sparse-keymap))
          (dotimes (n 30) (define-key qa-many (vector (intern (format "item-%d" n)))
            (list 'menu-item (format "Item %02d" n) 'qa-never-run)))
          (setq qa-long (list 'keymap "Long" (list 'more 'menu-item "More" qa-many))))"#,
                )
                .unwrap();
            assert_eq!(
                scenario(
                    &mut interp,
                    "(x-popup-menu t qa-long)",
                    vec![
                        Resize(40, 6),
                        Click("More"),
                        Scroll("More", f32::MAX),
                        RootOffset(0),
                        Column("Item 00", Some("More")),
                        Click("Item 00"),
                    ]
                ),
                "(more item-0)"
            );
            assert!(interp.eval_str("(= qa-runs 0)").unwrap().truthy());
        })
        .unwrap()
        .join()
        .unwrap();
}
