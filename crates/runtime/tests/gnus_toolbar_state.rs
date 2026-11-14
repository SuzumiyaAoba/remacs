use remacs_runtime::{
    frontend::{Grid, InputEvent, KeyIo, run_editor_with_shared},
    lisp::Interp,
};
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc, time::Duration};

const SETUP: &str = r#"(progn
  (set-frame-parameter nil 'window-system 'ns)
  (set-frame-parameter nil 'menu-bar-lines 0)
  (set-frame-parameter nil 'tool-bar-lines 1)
  (set-frame-parameter nil 'tab-bar-lines 0)
  (defvar gnus-group-update-tool-bar nil)
  (setq menu-bar-mode nil
        tool-bar-map (make-sparse-keymap)
        gnus-group-update-tool-bar t
        qa-toolbar-update-points nil)
  (defun gnus-group--setup-tool-bar-update (beg end)
    (when gnus-group-update-tool-bar
      (if (fboundp 'cursor-sensor-mode)
          (progn
            (unless (bound-and-true-p cursor-sensor-mode)
              (cursor-sensor-mode 1))
            (put-text-property beg end 'cursor-sensor-functions
                               '(gnus-tool-bar-update)))
        (put-text-property beg end 'point-entered #'gnus-tool-bar-update)
        (put-text-property beg end 'point-left #'gnus-tool-bar-update))))
  (define-key tool-bar-map [describe]
    '(menu-item "Describe" ignore :active (get-text-property (point) 'qa-enabled)))
  (switch-to-buffer (get-buffer-create "*gnus-toolbar-probe*"))
  (erase-buffer)
  (insert (propertize "Group A\n" 'qa-enabled t) "Group B\n")
  (goto-char (point-min))
  (defun qa-next-group () (interactive) (forward-line 1))
  (defun gnus-tool-bar-update (&rest _)
    (push (point) qa-toolbar-update-points))
  (gnus-group--setup-tool-bar-update (point-min) (point-max)))"#;

struct Frontend {
    grid: Grid,
    events: VecDeque<InputEvent>,
}

impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }

    fn poll_input(&mut self, _: Duration) -> io::Result<Option<InputEvent>> {
        self.events
            .pop_front()
            .map(Some)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }

    fn unread(&mut self, _: i128) {}

    fn render(&mut self, interp: &Interp) -> io::Result<()> {
        self.grid = remacs_runtime::frontend::render_grid(interp, 80, 24);
        Ok(())
    }

    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

fn describe_enabled(grid: &Grid) -> bool {
    let item = grid
        .tool_bar_items
        .iter()
        .find(|item| {
            let (start, end) = item.byte_range.unwrap();
            grid.rows[item.row][start..end].trim() == "Describe"
        })
        .expect("Describe toolbar item");
    item.enabled
}

#[test]
fn gnus_group_toolbar_state_tracks_point_movement() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.eval_str(SETUP).unwrap_or_else(|flow| match flow {
                remacs_runtime::lisp::Flow::Signal(signal) => panic!(
                    "{} {}",
                    interp.prin1_to_string(&signal.symbol()),
                    interp.prin1_to_string(&signal.data())
                ),
                other => panic!("{other:?}"),
            });
            let frontend = Rc::new(RefCell::new(Frontend {
                grid: Grid::blank(80, 24),
                events: VecDeque::from([InputEvent::Command("qa-next-group".into())]),
            }));
            let error = run_editor_with_shared(frontend.clone(), &mut interp).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
            assert!(!describe_enabled(&frontend.borrow().grid));
            assert!(
                interp
                    .eval_str("(memq (point) qa-toolbar-update-points)")
                    .unwrap()
                    .truthy(),
                "toolbar update callback did not see the new group point: {}",
                interp.eval_str("qa-toolbar-update-points").map(|v| interp.prin1_to_string(&v)).unwrap_or_default()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
