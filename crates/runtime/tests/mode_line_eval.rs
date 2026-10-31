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

fn redraw(i: &mut Interp, frontend: &mut Frontend) -> Grid {
    frontend.run_pre_redisplay_functions(i).unwrap();
    render_grid(i, 40, 12)
}

fn mode_line(grid: &Grid, window: usize) -> String {
    let line = grid
        .mode_lines
        .iter()
        .find(|line| line.window_id == window)
        .unwrap();
    grid.rows[line.row]
        .chars()
        .skip(line.left)
        .take(line.width)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn assert_lisp(i: &mut Interp, source: &str, expected: &str) {
    let value = eval(i, source);
    assert_eq!(
        i.prin1_to_string(&value).to_unicode_lossy(),
        expected,
        "{source}"
    );
}

#[test]
fn dynamic_formats_evaluate_once_in_their_windows_and_preserve_the_caller() {
    let mut i = Interp::new();
    let mut frontend = Frontend;
    eval(
        &mut i,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
      (setq pre-redisplay-function nil)
      (setq-default fontification-functions nil)
      (setq-local fontification-functions nil header-line-format nil tab-line-format nil)
      (erase-buffer) (insert "abcdef") (goto-char 1)
      (setq-local mode-line-format '("static " (:eval "dynamic"))))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let window = i
        .selected_frame
        .as_ref()
        .unwrap()
        .borrow()
        .selected
        .borrow()
        .id;
    assert_eq!(
        mode_line(&redraw(&mut i, &mut frontend), window),
        "static dynamic"
    );

    eval(
        &mut i,
        r#"(progn (setq remacs-eval-count 0)
      (setq-local mode-line-format '(:eval (format "count %s" (setq remacs-eval-count (1+ remacs-eval-count))))))"#,
    );
    let grid = redraw(&mut i, &mut frontend);
    assert_eq!(mode_line(&grid, window), "count 1");
    assert_eq!(mode_line(&render_grid(&i, 40, 12), window), "count 1");
    assert_lisp(&mut i, "remacs-eval-count", "1");
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "count 2");

    eval(
        &mut i,
        r#"(progn
      (setq-local header-line-format '(:eval "header dynamic") tab-line-format '(:eval "tab dynamic"))
      (setq-local mode-line-format '(:eval '("nested " (:eval "value")))))"#,
    );
    let grid = redraw(&mut i, &mut frontend);
    assert_eq!(mode_line(&grid, window), "nested value");
    assert!(
        grid.rows
            .iter()
            .any(|row| row.trim_end() == "header dynamic")
    );
    assert!(grid.rows.iter().any(|row| row.trim_end() == "tab dynamic"));

    eval(
        &mut i,
        r#"(progn
      (setq-local header-line-format nil tab-line-format nil)
      (setq remacs-eval-label "before")
      (setq-local mode-line-format '("" remacs-eval-label (:eval (progn (setq remacs-eval-label "after") "|")) remacs-eval-label)))"#,
    );
    assert_eq!(
        mode_line(&redraw(&mut i, &mut frontend), window),
        "before|after"
    );
    eval(
        &mut i,
        r#"(progn (setq remacs-eval-count 0 remacs-eval-active nil)
      (setq-local mode-line-format '(remacs-eval-active (:eval (progn (setq remacs-eval-count (1+ remacs-eval-count)) "on")) "off")))"#,
    );
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "off");
    assert_lisp(&mut i, "remacs-eval-count", "0");
    eval(&mut i, "(setq remacs-eval-active t)");
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "on");
    assert_lisp(&mut i, "remacs-eval-count", "1");
    eval(
        &mut i,
        r#"(progn (setq remacs-eval-count 0)
      (setq-local mode-line-format '("%-" (:eval (setq remacs-eval-count 1)))))"#,
    );
    redraw(&mut i, &mut frontend);
    assert_lisp(&mut i, "remacs-eval-count", "0");

    eval(
        &mut i,
        r#"(progn (setq remacs-eval-count 0)
      (setq remacs-eval-variable '(:eval (progn (setq remacs-eval-count (1+ remacs-eval-count)) "trusted")))
      (put 'remacs-eval-variable 'risky-local-variable nil)
      (setq-local mode-line-format '("" remacs-eval-variable)))"#,
    );
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "");
    assert_lisp(&mut i, "remacs-eval-count", "0");
    eval(
        &mut i,
        "(put 'remacs-eval-variable 'risky-local-variable t)",
    );
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "trusted");
    assert_lisp(&mut i, "remacs-eval-count", "1");

    eval(
        &mut i,
        r#"(progn
      (setq-local minor-mode-alist '((remacs-eval-minor (:eval " lighter"))))
      (setq remacs-eval-minor t)
      (setq-local mode-line-format '("M" mode-line-minor-modes)))"#,
    );
    assert_eq!(
        mode_line(&redraw(&mut i, &mut frontend), window),
        "M lighter"
    );

    eval(
        &mut i,
        r#"(progn
      (setq remacs-eval-debugger 0 debug-on-error t debug-on-signal t)
      (fset 'remacs-eval-debugger-function (lambda (&rest _) (setq remacs-eval-debugger (1+ remacs-eval-debugger))))
      (setq debugger #'remacs-eval-debugger-function)
      (setq-local mode-line-format '(:eval (error "mode-line failure"))))"#,
    );
    let grid = redraw(&mut i, &mut frontend);
    assert_eq!(mode_line(&grid, window), "");
    assert_eq!(grid.mode_lines.len(), 1);
    assert_lisp(&mut i, "remacs-eval-debugger", "0");
    eval(
        &mut i,
        r#"(setq-local mode-line-format '(:eval (if debug-on-error "enabled" "disabled")))"#,
    );
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "enabled");
    assert_lisp(
        &mut i,
        "(list inhibit-redisplay inhibit-quit debug-on-error)",
        "(nil nil t)",
    );
    eval(&mut i, "(setq debug-on-error nil debug-on-signal nil)");

    eval(
        &mut i,
        r#"(progn
      (string-match "aa" "baaz")
      (setq-local mode-line-format '(:eval (progn (goto-char (point-max)) (string-match "x" "x") "safe"))))"#,
    );
    assert_eq!(mode_line(&redraw(&mut i, &mut frontend), window), "safe");
    assert_lisp(&mut i, "(point)", "1");
    assert_lisp(&mut i, "(match-data t)", "(1 3)");

    eval(
        &mut i,
        r#"(progn
      (setq remacs-eval-source (current-buffer))
      (set-window-parameter nil 'remacs-eval-label "one")
      (setq remacs-eval-other (split-window-right 20))
      (set-window-parameter remacs-eval-other 'remacs-eval-label "two")
      (set-window-point remacs-eval-other 4)
      (setq-local mode-line-format '(:eval (format "%s:%s:%s" (window-parameter (selected-window) 'remacs-eval-label) (point) (if (mode-line-window-selected-p) "active" "inactive"))))
      (set-buffer (get-buffer-create "mode-line-caller")) (insert "caller") (goto-char 2))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let caller = i.current_buffer;
    let other = eval(&mut i, "remacs-eval-other");
    let Value::Window(other) = other else {
        panic!("other window");
    };
    let grid = redraw(&mut i, &mut frontend);
    assert_eq!(mode_line(&grid, window), "one:1:active");
    assert_eq!(mode_line(&grid, other.borrow().id), "two:4:inactive");
    assert_eq!(i.current_buffer, caller);
    assert_lisp(&mut i, "(point)", "2");
    assert_lisp(&mut i, "(eq (selected-window) remacs-eval-other)", "nil");
    assert_lisp(&mut i, "(eq (old-selected-window) (selected-window))", "t");
    assert_lisp(&mut i, "(boundp 'remacs--mode-line-selected-window)", "nil");
    assert_lisp(&mut i, "(window-point remacs-eval-other)", "4");
    assert_lisp(
        &mut i,
        "(with-current-buffer remacs-eval-source (point))",
        "1",
    );
    eval(
        &mut i,
        r#"(progn
      (setq remacs-eval-remote-calls 0)
      (defun remacs-eval-remote-handler (operation _name &rest _options)
        (when (eq operation 'file-remote-p)
          (setq remacs-eval-remote-calls (1+ remacs-eval-remote-calls))
          "remote"))
      (with-current-buffer remacs-eval-source
        (setq-local default-directory "/remacs-mode-remote:host:/")
        (setq-local mode-line-format '(:eval (if (eq (selected-window) remacs-eval-other) "remote%@" "local"))))
      (setq file-name-handler-alist '(("remacs-mode-remote:" . remacs-eval-remote-handler))))"#,
    );
    let grid = redraw(&mut i, &mut frontend);
    assert_eq!(mode_line(&grid, window), "local");
    assert_eq!(mode_line(&grid, other.borrow().id), "remote@");
    assert_lisp(&mut i, "remacs-eval-remote-calls", "1");
}
