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
    assert!(
        e.contains("no-catch") || e.contains("throw"),
        "stderr: {}",
        e
    );
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

#[test]
fn interactive_without_tty_errors() {
    // No tty in test env → run_editor fails cleanly.
    let (_o, e, code) = remacs(&["-nw"], "");
    assert_ne!(code, 0);
    assert!(
        e.contains("terminal") || e.contains("error"),
        "stderr: {}",
        e
    );
}

#[test]
fn file_arg_enters_interactive_path() {
    let dir = std::env::temp_dir().join("remacs-cli-visit.txt");
    std::fs::write(&dir, "hello").unwrap();
    let (_o, _e, code) = remacs(&["-nw", dir.to_str().unwrap()], "");
    assert_ne!(code, 0); // still fails at run_editor without a tty
    let _ = std::fs::remove_file(&dir);
}

#[test]
fn execute_alias_works() {
    let (o, _e, code) = remacs(&["--execute", "(princ 7)"], "");
    assert_eq!(o, "7");
    assert_eq!(code, 0);
}

#[test]
fn load_missing_file_errors() {
    let (_o, _e, code) = remacs(&["--batch", "-l", "/nonexistent/x.el"], "");
    assert_ne!(code, 0);
}

#[test]
fn quit_flow_via_throw() {
    let (_o, e, code) = remacs(&["--eval", "(catch 'zz (throw 'zz 5))"], "");
    assert_eq!(code, 0);
    let _ = e;
}

/// Drive the real interactive loop through a pseudo-terminal (the
/// `script` utility allocates one). Types text, runs M-x, searches
/// with isearch, then quits with C-x C-c. Skipped when `script` is
/// unavailable.
#[test]
fn pty_editor_smoke() {
    let script = match Command::new("script")
        .args(["-q", "/dev/null", "true"])
        .output()
    {
        Ok(_) => true,
        Err(_) => false,
    };
    if !script {
        eprintln!("skipping: `script` unavailable");
        return;
    }
    let mut cmd = Command::new("script");
    cmd.args(["-q", "/dev/null", env!("CARGO_BIN_EXE_remacs"), "-nw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn script");
    let mut stdin = child.stdin.take().unwrap();
    let keys: Vec<&[u8]> = vec![
        b"hello",
        b"\x15\x35z",              // C-u 5 z (universal/digit-argument path)
        b"\x1b-\x33x",             // M-- 3 x (negative-argument path)
        b"\x182",                  // C-x 2 (split-window-below)
        b"\x18o",                  // C-x o (other-window)
        b"\x18bbb\r",              // C-x b bb RET (switch-to-buffer)
        b"\x1b:(+ 1 2)\r",         // M-: eval-expression
        b"\x13el",                 // C-s el (isearch, stay in search)
        b"\x13",                   // C-s again (repeat-forward dispatch)
        b"\x7f",                   // DEL (isearch pop)
        b"\x06",                   // C-f inside isearch (ReDispatch path)
        b"\x13z",                  // C-s z (re-enter isearch)
        b"\r",                     // RET exits isearch
        "あ".as_bytes(),           // unbound multibyte char → self-insert
        b"\x1e",                   // C-^ (unbound control char)
        b"\x07",                   // C-g
        b"\x0b",                   // C-k (kill-line)
        b"\x19",                   // C-y (yank)
        b"\x1by",                  // M-y (yank-pop)
        b"\x1f",                   // C-/ (undo)
        b"\x0c",                   // C-l (recenter-top-bottom)
        b"\x08k\x06",              // C-h k C-f (describe-key)
        b"\x15\x07",               // C-u C-g (abort prefix arg)
        b"\x1bxxdescrib\r",        // M-x with partial completion
        b"\x1bxdescribe-bindings\r", // M-x describe-bindings RET
        b"\x18\x03",               // C-x C-c
    ];
    for k in keys {
        stdin.write_all(k).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("scratch"),
        "expected a rendered frame, got: {}",
        &text[..text.len().min(400)]
    );
    assert!(out.status.success());
}
