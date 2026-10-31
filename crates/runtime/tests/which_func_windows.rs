use remacs_runtime::{
    frontend::{Grid, KeyIo, render_grid},
    lisp::{Interp, Value},
};
use std::{io, time::Duration};

struct Frontend;
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 12)
    }
}

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn window_id(i: &mut Interp, name: &str) -> usize {
    let Value::Window(window) = eval(i, name) else {
        panic!("window {name}");
    };
    window.borrow().id
}

fn check(i: &mut Interp, frontend: &mut Frontend, first: usize, second: usize, names: [&str; 2]) {
    let selected = eval(i, "(selected-window)");
    let buffer = i.current_buffer;
    let point = eval(i, "(point)");
    frontend.run_pre_redisplay_functions(i).unwrap();
    let grid: Grid = render_grid(i, 40, 12);
    assert_eq!(grid.mode_lines.len(), 2);
    for (window, name) in [(first, names[0]), (second, names[1])] {
        let line = grid
            .mode_lines
            .iter()
            .find(|line| line.window_id == window)
            .unwrap();
        let mode = grid.rows[line.row]
            .chars()
            .skip(line.left)
            .take(line.width)
            .collect::<String>();
        assert_eq!(mode.trim_end(), format!("M[{name}]"));
        let header = grid.rows[0]
            .chars()
            .skip(line.left)
            .take(line.width)
            .collect::<String>();
        assert_eq!(header.trim_end(), format!("H[{name}]"));
    }
    assert_eq!(i.current_buffer, buffer);
    let after_selected = eval(i, "(selected-window)");
    let after_point = eval(i, "(point)");
    assert!(remacs_runtime::lisp::eq_values(&selected, &after_selected));
    assert_eq!(i.prin1_to_string(&point), i.prin1_to_string(&after_point));
}

#[test]
fn which_function_formats_use_the_window_whose_line_is_being_rendered() {
    let mut i = Interp::new();
    let mut frontend = Frontend;
    let flags = eval(
        &mut i,
        "(list (get 'which-func-format 'risky-local-variable) (get 'which-func-current 'risky-local-variable))",
    );
    assert_eq!(i.prin1_to_string(&flags).to_unicode_lossy(), "(t t)");
    eval(
        &mut i,
        r#"(progn
      (require 'which-func)
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
      (setq pre-redisplay-function nil)
      (setq-default fontification-functions nil))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    eval(
        &mut i,
        r#"(progn
      (setq which-func-unknown "unknown")
      (defun remacs-which-provider () (format "%s%%p%s" (buffer-name) (point)))
      (dolist (name '("wf-a" "wf-b"))
        (with-current-buffer (get-buffer-create name)
          (erase-buffer) (insert "abcdef") (goto-char 1)
          (setq-local which-func-mode t fontification-functions nil
                      which-func-functions '(remacs-which-provider)
                      mode-line-format '("M" which-func-format)
                      header-line-format '("H" which-func-format)
                      tab-line-format nil)))
      (set-window-buffer nil "wf-a")
      (setq remacs-which-first (selected-window))
      (setq remacs-which-second (split-window-right 20))
      (set-window-buffer remacs-which-second "wf-b")
      (set-window-point remacs-which-first 1)
      (set-window-point remacs-which-second 4)
      (which-func-update-1 remacs-which-first)
      (which-func-update-1 remacs-which-second))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let first = window_id(&mut i, "remacs-which-first");
    let second = window_id(&mut i, "remacs-which-second");
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "wf-b%p4"]);

    eval(
        &mut i,
        "(progn (select-window remacs-which-second) (goto-char 5) (which-func-update-1 remacs-which-second))",
    );
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "wf-b%p5"]);
    eval(
        &mut i,
        "(progn (select-window remacs-which-first) (remhash remacs-which-second which-func-table))",
    );
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "unknown"]);

    eval(
        &mut i,
        "(progn (set-window-buffer remacs-which-second \"wf-a\") (set-window-point remacs-which-second 4) (which-func-update-1 remacs-which-first) (which-func-update-1 remacs-which-second))",
    );
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "wf-a%p4"]);
    eval(&mut i, "(select-window remacs-which-second)");
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "wf-a%p4"]);
    eval(
        &mut i,
        "(progn (goto-char 6) (which-func-update-1 remacs-which-second))",
    );
    check(&mut i, &mut frontend, first, second, ["wf-a%p1", "wf-a%p6"]);
}
