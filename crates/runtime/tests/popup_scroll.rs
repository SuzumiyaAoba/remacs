use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, install_minibuf_reader, render_grid},
    lisp::Interp,
};
use std::{
    cell::RefCell,
    collections::{BTreeSet, VecDeque},
    io,
    rc::Rc,
    time::Duration,
};

#[derive(Default)]
struct Input {
    grid: Option<Grid>,
    offset: usize,
    capacity: usize,
    total: usize,
    polls: usize,
    seen: BTreeSet<usize>,
    expected: BTreeSet<usize>,
    result: String,
    steps: VecDeque<(usize, f32, bool)>,
}

impl KeyIo for Input {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        unreachable!()
    }
    fn unread(&mut self, _: i128) {}
    fn size(&self) -> (usize, usize) {
        (40, 10)
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        self.grid = Some(render_grid(i, 40, 10));
        if let Some(menu) = &i.gui_popup_menu {
            self.offset = menu.scroll_offset.get();
            self.capacity = menu.visible_rows.get();
            self.total = menu.entries.len();
            self.expected = menu
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| entry.value.as_ref().map(|_| index))
                .collect();
            self.result = i
                .prin1_to_string(menu.entries.last().unwrap().value.as_ref().unwrap())
                .to_unicode_lossy();
            assert!(self.capacity > 0 && self.capacity < self.total);
            for item in &self.grid.as_ref().unwrap().popup_menu_items {
                if let Some(index) = item.selection {
                    self.seen.insert(index);
                    let (start, end) = item.byte_range.unwrap();
                    assert_eq!(
                        self.grid.as_ref().unwrap().rows[item.row][start..end].trim(),
                        menu.entries[index].label
                    );
                }
            }
        }
        Ok(())
    }
    fn poll_key_or_mouse(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.polls += 1;
        assert!(self.polls < 100, "scrolling stopped making progress");
        let grid = self.grid.as_ref().unwrap();
        let menu_id = grid.popup_menu_items[0].menu_id;
        if let Some((expected, delta_lines, stale)) = self.steps.pop_front() {
            assert_eq!(self.offset, expected);
            return Ok(Some(InputEvent::PopupMenuScroll {
                menu_id: menu_id + usize::from(stale),
                delta_lines,
            }));
        }
        if self.offset == self.total - self.capacity {
            assert_eq!(
                self.seen, self.expected,
                "every selectable entry must have been visible"
            );
            let item = grid
                .popup_menu_items
                .iter()
                .find(|item| item.selection == Some(self.total - 1))
                .expect("last choice must have a visible hit region");
            Ok(Some(InputEvent::PopupMenuSelection {
                menu_id,
                index: item.selection,
            }))
        } else {
            Ok(Some(InputEvent::PopupMenuScroll {
                menu_id,
                delta_lines: self.capacity as f32,
            }))
        }
    }
}

#[test]
fn oversized_popups_scroll_all_choices_and_return_the_original_last_value() {
    let mut i = Interp::new();
    i.eval_str(
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-window-fringes nil 0 0)
        (setq qa-long-menu (make-sparse-keymap "Many choices")
              qa-long-legacy (list "Many choices" (cons "Pane" nil)))
        (dotimes (n 40)
          (define-key qa-long-menu (vector (intern (format "item-%d" n)))
            (list 'menu-item (format "Item %02d" n) 'ignore))
          (setcdr (cadr qa-long-legacy)
            (append (cdadr qa-long-legacy) (list (cons (format "Item %02d" n) n))))))"#,
    )
    .unwrap();
    let input = Rc::new(RefCell::new(Input::default()));
    install_minibuf_reader(&input, &mut i);
    for source in [
        "(x-popup-menu t qa-long-menu)",
        "(x-popup-menu t qa-long-legacy)",
    ] {
        *input.borrow_mut() = Input {
            steps: [
                (0, 20.0, true),
                (0, 0.4, false),
                (0, 0.4, false),
                (0, 0.4, false),
                (1, f32::NAN, false),
                (1, f32::INFINITY, false),
                (1, -f32::MAX, false),
            ]
            .into(),
            ..Default::default()
        };
        let value = i.eval_str(source).unwrap();
        assert_eq!(
            i.prin1_to_string(&value).to_unicode_lossy(),
            input.borrow().result
        );
        assert!(i.gui_popup_menu.is_none());
    }
}
