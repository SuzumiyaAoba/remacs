//! Live backtraces through the binary's command-line and source loader.
use std::path::PathBuf;
use std::process::{Command, Output};

const EXPRESSION: &str = "(prin1 (mapcar #'cadr (backtrace-frames)))";

fn source(name: &str, body: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("remacs-backtrace-{}-{name}.el", std::process::id()));
    std::fs::write(&path, format!(";;; -*- lexical-binding: t; -*-\n{body}\n")).unwrap();
    path
}

fn run(exe: &std::ffi::OsStr, args: &[String], source: bool) -> Output {
    let mut command = Command::new(exe);
    command.args(args);
    if source {
        command.env("REMACS_NO_ELC", "1");
    }
    command.output().unwrap()
}

fn check(args: &[String], expected: &str) {
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let output = run(&gnu, args, false);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
    for source in [false, true] {
        let output = run(
            std::ffi::OsStr::new(env!("CARGO_BIN_EXE_remacs")),
            args,
            source,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
}

#[test]
fn eval_records_pending_expression_and_real_command_line_entries() {
    check(
        &[
            "-Q".into(),
            "--batch".into(),
            "--eval".into(),
            EXPRESSION.into(),
        ],
        "(backtrace-frames mapcar prin1 eval command-line-1 command-line normal-top-level)",
    );
}

#[test]
fn source_load_records_conversion_and_nested_load_calls() {
    let path = source("load", EXPRESSION);
    let path_str = path.to_str().unwrap();
    check(&["-Q".into(), "--batch".into(), "--load".into(), path_str.into()],
          "(backtrace-frames mapcar prin1 load-with-code-conversion command-line-1 command-line normal-top-level)");
    check(&["-Q".into(), "--batch".into(), "--eval".into(), format!("(load {path_str:?} nil t)")],
          "(backtrace-frames mapcar prin1 load-with-code-conversion load eval command-line-1 command-line normal-top-level)");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn funcall_uses_its_own_action_without_an_eval_or_funcall_wrapper() {
    check(
        &[
            "-Q".into(),
            "--batch".into(),
            "--eval".into(),
            format!("(defun qa118-cli () {EXPRESSION})"),
            "-f".into(),
            "qa118-cli".into(),
        ],
        "(backtrace-frames mapcar prin1 qa118-cli command-line-1 command-line normal-top-level)",
    );
}

#[test]
fn script_uses_the_real_loader_callback() {
    // Native and interpreted callback identities differ. Check the named
    // dispatch frames and the live anonymous callback's buffer/file arguments.
    let body = r#"(let ((frames (backtrace-frames)) names callback)
  (dolist (frame frames)
    (let ((function (nth 1 frame)) (arguments (nth 2 frame)))
      (when (memq function '(backtrace-frames load-with-code-conversion
                             command-line--load-script command-line-1
                             command-line normal-top-level))
        (push function names))
      (when (and (not (symbolp function)) (functionp function)
                 (bufferp (car arguments)) (equal (cadr arguments) load-file-name))
        (setq callback t))))
  (prin1 (list (nreverse names) callback)))"#;
    let path = source("script", body);
    check(&["--script".into(), path.to_str().unwrap().into()],
          "((backtrace-frames load-with-code-conversion command-line--load-script command-line-1 command-line normal-top-level) t)");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn error_snapshots_have_real_frames_without_duplicate_command_line_tails() {
    for expression in ["(let ((value (car 1))) value)", ")"] {
        let args = vec!["--batch".into(), "--eval".into(), expression.into()];
        let output = run(
            std::ffi::OsStr::new(env!("CARGO_BIN_EXE_remacs")),
            &args,
            false,
        );
        assert_eq!(output.status.code(), Some(255));
        let errors = String::from_utf8(output.stderr).unwrap();
        for name in ["command-line-1(", "command-line()", "normal-top-level()"] {
            assert_eq!(errors.matches(name).count(), 1, "{errors}");
        }
        if expression.starts_with("(let") {
            assert!(errors.contains("car(1)"), "{errors}");
            assert!(errors.contains("let(((value (car 1))) value)"), "{errors}");
        }
    }
}

#[test]
fn conversion_arguments_use_boolean_load_options() {
    let body = r#"(let ((frames (backtrace-frames)))
      (while (not (eq (cadar frames) 'load-with-code-conversion))
        (setq frames (cdr frames)))
      (prin1 (car frames)))
    (provide 'qa118-conversion)"#;
    let path = source("flags", body);
    let path_str = path.to_str().unwrap();
    let expected = format!("(t load-with-code-conversion ({path_str:?} {path_str:?} t t) nil)");
    for expression in [
        format!("(load {path_str:?} 'foo 'bar)"),
        format!("(require 'qa118-conversion {path_str:?} 'foo)"),
    ] {
        check(
            &["-Q".into(), "--batch".into(), "--eval".into(), expression],
            &expected,
        );
    }
    std::fs::remove_file(path).unwrap();
}
