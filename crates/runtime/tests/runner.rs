//! Exercise the standalone binary without either frontend package.
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_remacs-lisp"));
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    // EOF is essential when evaluating standard input.
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

#[test]
fn evaluates_ordered_actions_and_loads_bundled_libraries() {
    let out = run(
        &[
            "--eval",
            "(setq standalone-value 40)",
            "--eval",
            "(progn (require 'cl-lib) (cl-incf standalone-value 2))",
        ],
        None,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "40\n42\n");
}

#[test]
fn evaluates_stdin_and_reports_errors_and_exit_status() {
    let out = run(&[], Some("(princ (+ 20 22))"));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"42");
    let out = run(&["--eval", "(error \"standalone failure\")"], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("standalone failure"));
    let out = run(&["--eval", "(kill-emacs 7)"], None);
    assert_eq!(out.status.code(), Some(7));
}

#[test]
fn loads_files_with_lexical_binding_and_runs_shutdown_hooks() {
    let path = std::env::temp_dir().join(format!("remacs-standalone-{}.el", std::process::id()));
    std::fs::write(&path, ";;; -*- lexical-binding: t; -*-\n(let ((x 42)) (setq standalone-closure (lambda () x)))\n(add-hook 'kill-emacs-hook (lambda () (princ \"shutdown\")))").unwrap();
    let out = run(
        &[
            "--load",
            path.to_str().unwrap(),
            "--eval",
            "(funcall standalone-closure)",
        ],
        None,
    );
    let _ = std::fs::remove_file(path);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\nshutdown");
}
