//! Exercises the `remacs` binary's CLI surface: argument parsing,
//! batch/eval/script modes, error reporting, and exit codes.
use std::io::Write;
use std::process::{Command, Stdio};

fn remacs(args: &[&str], stdin: &str) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_remacs"));
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn remacs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn version_prints_and_exits() {
    let (o, _e, code) = remacs(&["--version"], "");
    assert!(o.contains("remacs"), "version output: {}", o);
    assert_eq!(code, 0);
}

#[test]
fn eval_runs_lisp() {
    let (o, _e, code) = remacs(&["--eval", "(princ (+ 1 2))"], "");
    assert_eq!(o, "3");
    assert_eq!(code, 0);
}

#[test]
fn eval_error_exits_nonzero() {
    let (_o, e, code) = remacs(&["--eval", "(car 1)"], "");
    assert_ne!(code, 0);
    assert!(e.contains("Error"), "stderr: {}", e);
}

#[test]
fn eval_uncaught_throw_reports() {
    let (_o, e, code) = remacs(&["--eval", "(throw 'x 1)"], "");
    assert_ne!(code, 0);
    assert!(e.contains("no-catch") || e.contains("throw"), "stderr: {}", e);
}

#[test]
fn script_loads_file() {
    let dir = std::env::temp_dir().join("remacs-cli-test.el");
    std::fs::write(&dir, "(princ \"from-file\")").unwrap();
    let (o, _e, code) = remacs(&["--script", dir.to_str().unwrap()], "");
    assert_eq!(o, "from-file");
    assert_eq!(code, 0);
    let _ = std::fs::remove_file(&dir);
}

#[test]
fn script_missing_file_errors() {
    let (_o, e, code) = remacs(&["--script", "/nonexistent/remacs-nope.el"], "");
    assert_ne!(code, 0);
    let _ = e;
}

#[test]
fn load_flag_evals_file() {
    let dir = std::env::temp_dir().join("remacs-cli-load.el");
    std::fs::write(&dir, "(princ 42)").unwrap();
    let (o, _e, code) = remacs(&["--batch", "-l", dir.to_str().unwrap()], "");
    assert_eq!(o, "42");
    assert_eq!(code, 0);
    let _ = std::fs::remove_file(&dir);
}

#[test]
fn batch_flag_exits_cleanly() {
    let (_o, _e, code) = remacs(&["--batch", "-Q"], "");
    assert_eq!(code, 0);
}

#[test]
fn no_window_flag_is_accepted() {
    let (_o, _e, code) = remacs(&["-nw", "--batch"], "");
    assert_eq!(code, 0);
}

#[test]
fn quit_flow_reports() {
    let (_o, e, code) = remacs(&["--eval", "(signal 'quit nil)"], "");
    assert_ne!(code, 0);
    let _ = e;
}
