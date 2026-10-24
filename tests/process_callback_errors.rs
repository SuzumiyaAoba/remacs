//! Process error reporters must keep GNU's batch exit and hook semantics.
use std::ffi::OsStr;
use std::process::{Command, Output, Stdio};

fn run(binary: &OsStr, expression: &str, source: bool) -> Output {
    let mut command = Command::new(binary);
    command
        .args(["-Q", "--batch", "--eval", expression])
        .stdin(Stdio::null());
    if source {
        command.env("REMACS_NO_ELC", "1");
    }
    command.output().unwrap()
}

fn check(expression: &str, expected_status: i32, expected_stdout: &str, stderr: &str) {
    let mut binaries = vec![
        (OsStr::new(env!("CARGO_BIN_EXE_remacs")).to_os_string(), false),
        (OsStr::new(env!("CARGO_BIN_EXE_remacs")).to_os_string(), true),
    ];
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        binaries.push((gnu, false));
    }
    for (binary, source) in binaries {
        let output = run(&binary, expression, source);
        let out = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(expected_status), "{binary:?} source={source}\n{out}\n{err}");
        assert_eq!(out, expected_stdout, "{binary:?} source={source}\n{err}");
        assert!(err.contains(stderr), "{binary:?} source={source}\n{err}");
        assert!(!err.contains("panicked"), "{err}");
    }
}

fn callback_expression(kind: &str, fail_hook: bool) -> String {
    let constructor = if kind == "sentinel" {
        "(make-pipe-process :name \"callback-error\" :noquery t :sentinel callback)"
    } else {
        "(make-process :name \"callback-error\" :noquery t :sentinel #'ignore :filter callback \
          :connection-type 'pipe :command '(\"/bin/sh\" \"-c\" \"printf payload\"))"
    };
    let invoke = if kind == "sentinel" {
        "(delete-process p)"
    } else {
        "(let ((deadline (+ (float-time) 3))) \
           (while (and (not seen) (< (float-time) deadline)) (accept-process-output p 0.01)))"
    };
    let hook = if fail_hook {
        "(error \"kill-hook-error\")"
    } else {
        "(princ \"HOOK\")"
    };
    let later_hook = if fail_hook {
        "(lambda () (princ \"HOOK-AFTER:\") (prin1 seen-errors))"
    } else { "" };
    format!(r#"(let ((debug-on-error nil) (process-error-pause-time 0)
                      seen-errors seen p)
      (let ((kill-emacs-hook (list (lambda () {hook}) {later_hook})))
      (unwind-protect
          (let ((callback (lambda (_process _text)
                            (setq seen t) (error "fatal-callback"))))
            (setq p {constructor})
            (princ "BEFORE:")
            (condition-case err
                (handler-bind ((error (lambda (_) (push 'outer seen-errors)))) {invoke})
              (error (princ "CAUGHT-") (princ (error-message-string err)) (princ ":")))
            (princ "AFTER"))
        (when p (set-process-sentinel p #'ignore) (delete-process p)))))"#)
}

#[test]
fn filter_default_reporter_exits_batch_after_running_exit_hooks() {
    check(&callback_expression("filter", false), 255, "BEFORE:HOOK",
          "error in process filter: fatal-callback");
}

#[test]
fn sentinel_default_reporter_exits_batch_after_running_exit_hooks() {
    check(&callback_expression("sentinel", false), 255, "BEFORE:HOOK",
          "error in process sentinel: fatal-callback");
}

#[test]
fn exit_hook_errors_are_reported_and_remaining_hooks_run() {
    for kind in ["filter", "sentinel"] {
        check(&callback_expression(kind, true), 255, "BEFORE:HOOK-AFTER:nil",
              "Error in kill-emacs-hook");
    }
}

#[test]
fn default_reporter_validates_context_and_does_not_exit_for_minibuffer_quit() {
    check(r#"(progn
      (prin1 (mapcar (lambda (args)
                       (condition-case err (apply #'command-error-default-function args)
                         (error err)))
                     '(((42 "bad") 7 nil) ((error "bad") 7 nil)
                       ((minibuffer-quit) 7 nil))))
      (command-error-default-function '(minibuffer-quit) "context" nil)
      (princ "AFTER"))"#,
      0, "((wrong-type-argument symbolp 42) (wrong-type-argument stringp 7) (wrong-type-argument stringp 7))\x07contextQuitAFTER", "");
}
