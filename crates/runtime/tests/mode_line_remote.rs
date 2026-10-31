use remacs_runtime::{
    frontend::{Grid, KeyIo, render_grid},
    lisp::{Flow, Interp, Value},
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

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn assert_marker(grid: &Grid, marker: char) {
    for prefix in ['M', 'H', 'T'] {
        let expected = format!("{prefix}{marker}");
        assert!(
            grid.rows.iter().any(|row| row.trim_end() == expected),
            "missing {expected:?}: {:?}",
            grid.rows
        );
    }
}

fn check(interp: &mut Interp, frontend: &mut Frontend, marker: char) {
    let buffer = interp.current_buffer;
    let point = interp.current_buffer_ref().unwrap().borrow().point();
    frontend.run_pre_redisplay_functions(interp).unwrap();
    assert_eq!(interp.current_buffer, buffer);
    assert_eq!(interp.current_buffer_ref().unwrap().borrow().point(), point);
    assert_marker(&render_grid(interp, 40, 12), marker);
}

#[test]
fn remote_marker_uses_each_buffers_directory_and_filename_handlers() {
    let mut interp = Interp::new();
    let mut frontend = Frontend;
    eval(
        &mut interp,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-size nil 40 12)
      (setq pre-redisplay-function nil)
      (setq-local fontification-functions nil)
      (setq-local mode-line-format "M%@" header-line-format "H%@" tab-line-format "T%@")
      (setq-local default-directory "/tmp/")
      (setq remacs-test-original-handlers file-name-handler-alist)
      (setq remacs-test-remote-calls 0 remacs-test-remote-enabled t)
      (defun remacs-test-remote-handler (operation name &rest options)
        (when (eq operation 'file-remote-p)
          (setq remacs-test-remote-calls (1+ remacs-test-remote-calls))
          (setq remacs-test-remote-argument-ok
                (and (eq name default-directory)
                     (eq (get-text-property 0 'remote-test name) 'kept)
                     (equal options '(nil nil))))
          (string-match "x" "x")
          (when remacs-test-remote-enabled "/mock:host:")))
      (setq file-name-handler-alist
            '(("\\`/mock:" . remacs-test-remote-handler)
              ("\\`/claimed/" . remacs-test-remote-handler))))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);
    check(&mut interp, &mut frontend, '-');
    eval(
        &mut interp,
        "(setq-local default-directory (propertize \"/mock:host:/path/\" 'remote-test 'kept))",
    );
    eval(&mut interp, "(string-match \"aa\" \"baaz\")");
    check(&mut interp, &mut frontend, '@');
    assert!(eval(&mut interp, "remacs-test-remote-argument-ok").truthy());
    let matches = eval(&mut interp, "(match-data t)");
    assert_eq!(interp.prin1_to_string(&matches), "(1 3)");
    eval(&mut interp, "(setq remacs-test-remote-enabled nil)");
    check(&mut interp, &mut frontend, '-');
    eval(
        &mut interp,
        "(setq remacs-test-remote-enabled t inhibit-file-name-handlers '(remacs-test-remote-handler) inhibit-file-name-operation 'file-remote-p)",
    );
    check(&mut interp, &mut frontend, '-');
    eval(
        &mut interp,
        "(setq inhibit-file-name-handlers nil inhibit-file-name-operation nil)",
    );
    eval(
        &mut interp,
        "(put 'remacs-test-remote-handler 'operations '(file-exists-p))",
    );
    check(&mut interp, &mut frontend, '-');
    eval(
        &mut interp,
        "(put 'remacs-test-remote-handler 'operations nil)",
    );
    eval(
        &mut interp,
        "(setq-local default-directory (propertize \"/claimed/directory/\" 'remote-test 'kept))",
    );
    check(&mut interp, &mut frontend, '@');

    let before = eval(&mut interp, "remacs-test-remote-calls").int();
    eval(
        &mut interp,
        "(setq-local mode-line-format \"literal %%@\" header-line-format nil tab-line-format nil)",
    );
    frontend.run_pre_redisplay_functions(&mut interp).unwrap();
    assert_eq!(eval(&mut interp, "remacs-test-remote-calls").int(), before);
    assert!(
        render_grid(&interp, 40, 12)
            .rows
            .iter()
            .any(|row| row.trim_end() == "literal %@")
    );
    eval(
        &mut interp,
        "(setq-local mode-line-format '(remacs-test-show-remote \"%@\" \"local\")) (setq remacs-test-show-remote nil)",
    );
    frontend.run_pre_redisplay_functions(&mut interp).unwrap();
    assert_eq!(eval(&mut interp, "remacs-test-remote-calls").int(), before);

    eval(
        &mut interp,
        "(setq-local mode-line-format \"M%@\" header-line-format \"H%@\" tab-line-format \"T%@\") (setq-local default-directory (propertize \"/mock:host:/path/\" 'remote-test 'kept))",
    );
    check(&mut interp, &mut frontend, '@');
    eval(&mut interp, "(aset default-directory 1 ?x)");
    // A changed directory cannot reuse the old remote result, even in place.
    assert_marker(&render_grid(&interp, 40, 12), '-');
    check(&mut interp, &mut frontend, '-');

    eval(
        &mut interp,
        r#"(progn
      (setq-local default-directory "/tmp/")
      (with-current-buffer (get-buffer-create "remote-other")
        (setq-local default-directory (propertize "/claimed/other/" 'remote-test 'kept))
        (setq-local fontification-functions nil)
        (setq-local mode-line-format "M%@" header-line-format nil tab-line-format nil))
      (setq remacs-test-other-window (split-window-below 6))
      (set-window-buffer remacs-test-other-window "remote-other"))"#,
    );
    frontend.run_pre_redisplay_functions(&mut interp).unwrap();
    let grid = render_grid(&interp, 40, 12);
    let other = eval(&mut interp, "remacs-test-other-window");
    let Value::Window(other) = other else {
        panic!("other window");
    };
    assert_eq!(grid.mode_lines.len(), 2);
    for line in &grid.mode_lines {
        let text = grid.rows[line.row]
            .chars()
            .skip(line.left)
            .take(line.width)
            .collect::<String>();
        assert_eq!(
            text.trim_end(),
            if line.window_id == other.borrow().id {
                "M@"
            } else {
                "M-"
            }
        );
    }

    eval(
        &mut interp,
        r#"(progn
      (delete-other-windows)
      (setq-local default-directory "/tmp/")
      (setq file-name-handler-alist remacs-test-original-handlers)
      (require 'tramp)
      (dolist (function '(make-network-process open-network-stream start-process make-process call-process))
        (fset function (lambda (&rest _) (error "Unexpected connection/process in remote marker")))))"#,
    );
    for (directory, marker) in [
        ("/tmp/", '-'),
        ("/tmp/a:b/", '-'),
        ("/ssh:example.invalid:/tmp/", '@'),
        ("/sudo::/tmp/", '@'),
        ("/ssh:jump.invalid|sudo:root@example.invalid:/var/", '@'),
        ("/:/ssh:example.invalid:/tmp/", '-'),
    ] {
        let name = interp
            .prin1_to_string(&Value::string(directory))
            .to_unicode_lossy();
        eval(
            &mut interp,
            &format!("(setq-local default-directory {name})"),
        );
        check(&mut interp, &mut frontend, marker);
    }
    eval(&mut interp, "(setq file-name-handler-alist nil)");
    eval(
        &mut interp,
        "(setq-local default-directory \"/ssh:example.invalid:/tmp/\")",
    );
    check(&mut interp, &mut frontend, '-');
}
