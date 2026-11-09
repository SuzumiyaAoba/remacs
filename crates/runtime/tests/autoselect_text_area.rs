use remacs_runtime::{
    editor::{event_code_for, reflow_frame_windows},
    frontend::{GridWindowTextArea, InputEvent, KeyIo, render_grid, run_editor_with},
    lisp::{Interp, Value},
};
use std::{collections::VecDeque, io, time::Duration};

enum Part {
    ModeLine,
    Header,
    Tab,
    FirstText,
    LastText,
    Outside,
}
enum Step {
    Input(InputEvent),
    Move(Part),
}
struct Frontend {
    events: VecDeque<Step>,
    trace: u32,
    filled: u32,
    expected: Vec<bool>,
    frame: usize,
    target: usize,
    areas: Vec<GridWindowTextArea>,
    mode_row: Option<usize>,
}
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        let step = self.events.pop_front().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "autoselect test exhausted events",
            )
        })?;
        Ok(Some(match step {
            Step::Input(event) => event,
            Step::Move(part) => {
                let area = self
                    .areas
                    .iter()
                    .find(|area| area.window_id == self.target)
                    .unwrap();
                let (x, y) = match part {
                    Part::ModeLine => (area.left + 1, self.mode_row.unwrap()),
                    Part::Header => (area.left + 1, area.top - 1),
                    Part::Tab => (area.left + 1, area.top - 2),
                    Part::FirstText => (area.left + 1, area.top),
                    Part::LastText => (area.left + 1, area.top + area.rows - 1),
                    Part::Outside => (area.left + area.width, area.top),
                };
                let text_window_id = self
                    .areas
                    .iter()
                    .find(|area| {
                        x >= area.left
                            && x < area.left + area.width
                            && y >= area.top
                            && y < area.top + area.rows
                    })
                    .map(|area| area.window_id);
                InputEvent::MouseMoved {
                    frame_id: Some(self.frame),
                    text_window_id,
                    frame_position: Some((x as i128, y as i128)),
                    frame_pixel_position: None,
                    position: None,
                    modifiers: 0,
                }
            }
        }))
    }
    fn unread(&mut self, _: i128) {}
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 16)
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let trace = i.symbol_value(self.trace).list_to_vec().unwrap();
        assert!(trace.len() <= self.expected.len());
        for (index, state) in trace.iter().enumerate() {
            assert_eq!(
                state.truthy(),
                self.expected[index],
                "selection after motion {index}"
            );
        }
        let grid = render_grid(i, 40, 16);
        for line in &grid.mode_lines {
            let area = grid
                .window_text_areas
                .iter()
                .find(|area| area.window_id == line.window_id)
                .unwrap();
            assert_eq!(
                line.row,
                area.top + area.rows,
                "mode line must follow the visible text body"
            );
            if i.symbol_value(self.filled).truthy() {
                assert_eq!(
                    grid.rows[line.row - 1].trim(),
                    "x",
                    "body must fill the row before the mode line"
                );
            }
        }
        self.mode_row = grid
            .mode_lines
            .iter()
            .find(|line| line.window_id == self.target)
            .map(|line| line.row);
        self.areas = grid.window_text_areas;
        Ok(())
    }
}

#[test]
fn autoselect_uses_the_text_rectangle_including_blank_body_space() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 16) (set-window-fringes nil 0 0)
      (erase-buffer)
      (setq-default fontification-functions nil)
      (setq-local fontification-functions nil)
      (setq-local mode-line-format "Mode" header-line-format nil tab-line-format nil)
      (setq remacs-autoselect-upper (selected-window))
      (setq remacs-autoselect-lower (split-window-below 8))
      (setq mouse-autoselect-window t remacs-autoselect-trace nil remacs-autoselect-filled nil)
      (setq remacs-autoselect-delay-started nil remacs-autoselect-delay-cancelled nil)
      (defun remacs-autoselect-reset () (interactive) (select-window remacs-autoselect-upper))
      (defun remacs-autoselect-record ()
        (interactive)
        (setq remacs-autoselect-trace
          (append remacs-autoselect-trace
            (list (eq (selected-window) remacs-autoselect-lower)))))
      (defun remacs-autoselect-decorations ()
        (interactive) (setq-local header-line-format "Header" tab-line-format "Tab"))
      (defun remacs-autoselect-no-mode-line () (interactive) (setq-local mode-line-format nil))
      (defun remacs-autoselect-fill ()
        (interactive) (dotimes (_ 40) (insert "x\n")) (goto-char 1)
        (setq remacs-autoselect-filled t))
      (defun remacs-autoselect-delay-start ()
        (interactive) (setq-local mode-line-format "Mode") (setq mouse-autoselect-window 60)
        (mouse-autoselect-window-start (mouse-position) remacs-autoselect-lower)
        (setq remacs-autoselect-delay-started (and (memq mouse-autoselect-window-timer timer-list) t)))
      (defun remacs-autoselect-delay-record ()
        (interactive) (setq remacs-autoselect-delay-cancelled (not (memq mouse-autoselect-window-timer timer-list))))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#).unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 40, 16);
    let Value::Window(lower) = i.eval_str("remacs-autoselect-lower").unwrap() else {
        panic!("lower window");
    };
    let mut events = VecDeque::new();
    let mut expected = Vec::new();
    let check = |events: &mut VecDeque<Step>, expected: &mut Vec<bool>, part, selected| {
        events.push_back(Step::Input(InputEvent::Command(
            "remacs-autoselect-reset".into(),
        )));
        events.push_back(Step::Move(part));
        events.push_back(Step::Input(InputEvent::Command(
            "remacs-autoselect-record".into(),
        )));
        expected.push(selected);
    };
    check(&mut events, &mut expected, Part::ModeLine, false);
    check(&mut events, &mut expected, Part::FirstText, true);
    check(&mut events, &mut expected, Part::Outside, false);
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-decorations".into(),
    )));
    for (part, selected) in [
        (Part::Tab, false),
        (Part::Header, false),
        (Part::FirstText, true),
        (Part::LastText, true),
        (Part::ModeLine, false),
    ] {
        check(&mut events, &mut expected, part, selected);
    }
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-fill".into(),
    )));
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-no-mode-line".into(),
    )));
    check(&mut events, &mut expected, Part::LastText, true);
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-reset".into(),
    )));
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-delay-start".into(),
    )));
    events.push_back(Step::Move(Part::ModeLine));
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-delay-record".into(),
    )));
    events.push_back(Step::Input(InputEvent::Command(
        "remacs-autoselect-record".into(),
    )));
    expected.push(false);
    events.push_back(Step::Input(InputEvent::Key(event_code_for("f12"))));
    let trace = i.intern("remacs-autoselect-trace");
    let filled = i.intern("remacs-autoselect-filled");
    let count = expected.len();
    let frame_id = frame.borrow().id;
    let target = lower.borrow().id;
    run_editor_with(
        Frontend {
            events,
            trace,
            filled,
            expected,
            frame: frame_id,
            target,
            areas: Vec::new(),
            mode_row: None,
        },
        &mut i,
    )
    .unwrap();
    assert_eq!(i.symbol_value(trace).list_to_vec().unwrap().len(), count);
    for name in [
        "remacs-autoselect-delay-started",
        "remacs-autoselect-delay-cancelled",
    ] {
        let symbol = i.intern(name);
        assert!(i.symbol_value(symbol).truthy(), "{name}");
    }
}
