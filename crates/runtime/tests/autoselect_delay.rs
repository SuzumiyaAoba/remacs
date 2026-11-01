use remacs_runtime::{
    editor::{event_code_for, reflow_frame_windows},
    frontend::{GridWindowTextArea, InputEvent, KeyIo, render_grid, run_editor_with},
    lisp::{Interp, Value},
};
use std::{
    cell::Cell,
    collections::VecDeque,
    io,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

enum Step {
    Move {
        window: usize,
        offset: usize,
    },
    Wait(Duration),
    Check {
        selected: usize,
        label: &'static str,
    },
    Reset,
    Negative,
    Quit,
}
struct Frontend {
    steps: VecDeque<Step>,
    frame: usize,
    areas: Vec<GridWindowTextArea>,
    deadline: Option<Instant>,
    check: Option<(usize, &'static str)>,
    checked: Rc<Cell<usize>>,
}
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn poll_input(&mut self, duration: Duration) -> io::Result<Option<InputEvent>> {
        loop {
            if let Some(deadline) = self.deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if !remaining.is_zero() {
                    thread::sleep(remaining.min(duration).min(Duration::from_millis(10)));
                    return Ok(None);
                }
                self.deadline = None;
            }
            return match self.steps.pop_front().ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "delay test exhausted steps")
            })? {
                Step::Wait(duration) => {
                    self.deadline = Some(Instant::now() + duration);
                    continue;
                }
                Step::Check { selected, label } => {
                    self.check = Some((selected, label));
                    Ok(None)
                }
                Step::Reset => Ok(Some(InputEvent::Command("remacs-delay-reset".into()))),
                Step::Negative => Ok(Some(InputEvent::Command("remacs-delay-negative".into()))),
                Step::Quit => Ok(Some(InputEvent::Key(event_code_for("f12")))),
                Step::Move { window, offset } => {
                    let area = self
                        .areas
                        .iter()
                        .find(|area| area.window_id == window)
                        .unwrap();
                    let x = area.left + area.width / 2 + offset;
                    let y = area.top + area.rows / 2;
                    Ok(Some(InputEvent::MouseMoved {
                        frame_id: Some(self.frame),
                        text_window_id: Some(window),
                        frame_position: Some((x as i128, y as i128)),
                        frame_pixel_position: None,
                        position: None,
                    }))
                }
            };
        }
    }
    fn unread(&mut self, _: i128) {}
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 16)
    }
    fn render_on_idle(&self) -> bool {
        true
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        if let Some((expected, label)) = self.check.take() {
            let selected = i
                .selected_frame
                .as_ref()
                .unwrap()
                .borrow()
                .selected
                .borrow()
                .id;
            let unread = i
                .intern_soft("unread-command-events")
                .map(|id| i.symbol_value(id))
                .unwrap_or(Value::Nil);
            let diagnostic = [
                "mouse-autoselect-window-state",
                "mouse-autoselect-window-position",
                "mouse-autoselect-window-position-1",
                "mouse-autoselect-window-window",
                "timer-list",
            ]
            .map(|name| {
                let value = i
                    .intern_soft(name)
                    .map(|id| i.symbol_value(id))
                    .unwrap_or(Value::Nil);
                format!("{name}={}", i.prin1_to_string(&value).to_unicode_lossy())
            });
            assert_eq!(
                selected,
                expected,
                "{label}; unread={}; state={diagnostic:?}; mouse={:?}; tail={:?}",
                i.prin1_to_string(&unread).to_unicode_lossy(),
                i.gui_mouse_frame_position,
                &render_grid(i, 40, 16).rows[13..]
            );
            self.checked.set(self.checked.get() + 1);
        }
        self.areas = render_grid(i, 40, 16).window_text_areas;
        Ok(())
    }
}

#[test]
fn positive_delay_requires_residence_and_does_not_restart_for_motion_inside_the_window() {
    let mut i = Interp::new();
    i.eval_str(r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-size nil 40 16) (set-window-fringes nil 0 0)
      (erase-buffer) (setq-default fontification-functions nil)
      (setq-local fontification-functions nil mode-line-format "Mode" header-line-format nil tab-line-format nil)
      (setq remacs-delay-upper (selected-window) remacs-delay-lower (split-window-below 8))
      (setq mouse-autoselect-window 1.0)
      (defun remacs-delay-reset () (interactive)
        (mouse-autoselect-window-cancel t) (select-window remacs-delay-upper))
      (defun remacs-delay-negative () (interactive)
        (remacs-delay-reset) (setq mouse-autoselect-window -0.5))
      (global-set-key [f12] (lambda () (interactive) (kill-emacs))))"#).unwrap();
    let frame = i.selected_frame.as_ref().unwrap().clone();
    reflow_frame_windows(&frame, 40, 16);
    let frame_id = frame.borrow().id;
    let upper = frame.borrow().selected.borrow().id;
    let Value::Window(lower) = i.eval_str("remacs-delay-lower").unwrap() else {
        panic!("lower window");
    };
    let lower = lower.borrow().id;
    let steps = VecDeque::from([
        Step::Move {
            window: lower,
            offset: 0,
        },
        Step::Wait(Duration::from_millis(150)),
        Step::Check {
            selected: upper,
            label: "entry must not select immediately",
        },
        Step::Move {
            window: upper,
            offset: 0,
        },
        Step::Wait(Duration::from_millis(1200)),
        Step::Check {
            selected: upper,
            label: "short visit must not select the departed window",
        },
        Step::Reset,
        Step::Move {
            window: lower,
            offset: 0,
        },
        Step::Wait(Duration::from_millis(50)),
        Step::Move {
            window: lower,
            offset: 1,
        },
        Step::Wait(Duration::from_millis(150)),
        Step::Check {
            selected: upper,
            label: "residence before the deadline",
        },
        Step::Wait(Duration::from_millis(1200)),
        Step::Check {
            selected: lower,
            label: "residence after the deadline",
        },
        Step::Reset,
        Step::Move {
            window: lower,
            offset: 0,
        },
        Step::Wait(Duration::from_millis(650)),
        Step::Move {
            window: lower,
            offset: 1,
        },
        Step::Wait(Duration::from_millis(650)),
        Step::Check {
            selected: lower,
            label: "positive delay measures residence, including motion",
        },
        Step::Negative,
        Step::Move {
            window: lower,
            offset: 0,
        },
        Step::Wait(Duration::from_millis(350)),
        Step::Move {
            window: lower,
            offset: 1,
        },
        Step::Wait(Duration::from_millis(350)),
        Step::Check {
            selected: upper,
            label: "negative delay waits for stationary residence",
        },
        Step::Wait(Duration::from_millis(400)),
        Step::Check {
            selected: lower,
            label: "negative delay selects after the pointer stops",
        },
        Step::Quit,
    ]);
    let checked = Rc::new(Cell::new(0));
    run_editor_with(
        Frontend {
            steps,
            frame: frame_id,
            areas: Vec::new(),
            deadline: None,
            check: None,
            checked: checked.clone(),
        },
        &mut i,
    )
    .unwrap();
    assert_eq!(checked.get(), 7);
}
