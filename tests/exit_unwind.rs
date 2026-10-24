//! Accepted shutdown must not run Lisp cleanup or replace the exit status.
mod common;

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use common::*;
use remacs::lisp::{Flow, Value};

fn check(expression: &str, status: i32, expected: &str) -> Vec<Output> {
    check_before(expression, status, expected, Duration::from_secs(120))
}

fn check_before(expression: &str, status: i32, expected: &str, timeout: Duration) -> Vec<Output> {
    let mut binaries = vec![
        (OsStr::new(env!("CARGO_BIN_EXE_remacs")).to_os_string(), false),
        (OsStr::new(env!("CARGO_BIN_EXE_remacs")).to_os_string(), true),
    ];
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        binaries.push((gnu, false));
    }
    let mut outputs = Vec::new();
    for (binary, source) in binaries {
        let mut command = Command::new(&binary);
        command.args(["-Q", "--batch", "--eval", expression])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if source { command.env("REMACS_NO_ELC", "1"); }
        let mut child = command.spawn().unwrap();
        let collect = |mut pipe: Box<dyn Read + Send>| std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        });
        let stdout = collect(Box::new(child.stdout.take().unwrap()));
        let stderr = collect(Box::new(child.stderr.take().unwrap()));
        let deadline = Instant::now() + timeout;
        let mut timed_out = false;
        let exit_status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline {
                timed_out = true;
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output = Output { status: exit_status, stdout: stdout.join().unwrap(), stderr: stderr.join().unwrap() };
        let out = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(!timed_out, "{binary:?} source={source} timed out\n{out}\n{err}");
        assert_eq!(output.status.code(), Some(status), "{binary:?} source={source}\n{out}\n{err}");
        assert_eq!(out, expected, "{binary:?} source={source}\n{err}");
        assert!(!err.contains("panicked"), "{err}");
        outputs.push(output);
    }
    outputs
}

#[test]
fn string_exit_argument_does_not_print_a_diagnostic() {
    for output in check(r#"(kill-emacs "SHUTDOWN-INPUT")"#, 0, "") {
        assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn pending_exit_bypasses_the_public_function_and_input_throw_tag() {
    check(r#"(progn
      (fset 'kill-emacs (lambda (&rest _) (princ "SHADOW")))
      (setq kill-emacs-hook (list (lambda () (princ "HOOK"))))
      (let ((inhibit-quit t) (throw-on-input 'kill-emacs))
        (princ "BODY")
        (catch 'kill-emacs
          (setq quit-flag 'kill-emacs)
          (let ((inhibit-quit nil)) (ignore)))
        (princ "AFTER")))"#, 0, "BODYHOOK");
}

#[test]
fn a_pending_exit_hook_can_abort_before_acceptance() {
    check(r#"(let ((inhibit-quit t)
                  (kill-emacs-hook (list (lambda () (throw 'cancel-exit 'stopped)))))
      (let ((result (catch 'cancel-exit
                      (setq quit-flag 'kill-emacs)
                      (let ((inhibit-quit nil)) (ignore)))))
        (prin1 (list result quit-flag))))"#, 0, "(stopped nil)");
}

#[test]
fn accepted_exit_skips_interpreted_cleanup_and_success_handlers() {
    check(r#"(progn (setq kill-emacs-hook nil)
      (condition-case nil
          (unwind-protect (progn (princ "BODY") (kill-emacs 17))
            (princ "CLEANUP") (kill-emacs 99))
        (:success (princ "SUCCESS")) (error (princ "ERROR"))))"#, 17, "BODY");
}

#[test]
fn accepted_exit_skips_bytecode_cleanup() {
    check(r#"(progn (require 'bytecomp) (setq kill-emacs-hook nil)
      (funcall (byte-compile
                '(lambda ()
                   (unwind-protect (progn (princ "BODY") (kill-emacs 17))
                     (princ "CLEANUP") (error "replace-exit"))))))"#, 17, "BODY");
}

#[test]
fn accepted_exit_crosses_a_native_entry_without_running_cleanup() {
    let directory = std::env::temp_dir().join(format!("remacs-exit-native-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("exit.eln");
    let expression = format!(r#"(progn (require 'comp) (setq kill-emacs-hook nil)
      (let* ((user-emacs-directory {directory:?})
             (native-comp-eln-load-path (list {directory:?}))
             (function (native-compile
                        (lambda ()
                          (unwind-protect (progn (princ "BODY") (kill-emacs 17))
                            (princ "CLEANUP") (kill-emacs 99))) {path:?})))
        (unless (native-comp-function-p function) (error "Actual native compilation is required"))
        (funcall function)))"#,
        directory = directory.to_str().unwrap(), path = path.to_str().unwrap());
    // Source bootstrap plus loading GNU's native compiler helpers can exceed
    // the ordinary CLI budget in debug builds; still bound genuine hangs.
    check_before(&expression, 17, "BODY", Duration::from_secs(300));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn accepted_exit_does_not_notify_unlet_watchers() {
    check(r#"(progn (defvar qa-exit-local 0) (setq kill-emacs-hook nil)
      (add-variable-watcher 'qa-exit-local
        (lambda (_symbol _value operation _where)
          (when (eq operation 'unlet) (princ "WATCHER") (error "unlet"))))
      (let ((qa-exit-local 1)) (princ "BODY") (kill-emacs 17)))"#, 17, "BODY");
}

#[test]
fn exit_from_worker_skips_parent_cleanup() {
    check(r#"(progn (setq kill-emacs-hook nil)
      (let ((worker (make-thread (lambda () (kill-emacs 23)))))
        (unwind-protect (progn (princ "BODY") (thread-join worker))
          (princ "PARENT-CLEANUP"))))"#, 23, "BODY");
}

#[test]
fn a_hook_can_abort_before_exit_is_accepted() {
    check(r#"(progn
      (let ((kill-emacs-hook (list (lambda () (throw 'abort-exit 'cancelled)))))
        (prin1 (catch 'abort-exit
                 (unwind-protect (kill-emacs 17) (princ "CLEANUP")))))
      (princ "AFTER"))"#, 0, "CLEANUPcancelledAFTER");
}

#[test]
fn a_hook_scope_restoration_error_can_abort_before_exit_is_accepted() {
    check(r#"(progn
      (defun qa-exit-watch (symbol _value operation _where)
        (when (eq operation 'unlet)
          (remove-variable-watcher symbol #'qa-exit-watch)
          (error "restore-error")))
      (let ((kill-emacs-hook (list (lambda () (princ "HOOK")))))
        (add-variable-watcher 'inhibit-quit #'qa-exit-watch)
        (condition-case err (kill-emacs 17) (error (princ (cadr err)))))
      (princ "AFTER"))"#, 0, "HOOKrestore-errorAFTER");
}

#[test]
fn nested_exit_in_hook_keeps_the_inner_status() {
    check(r#"(let ((kill-emacs-hook
                    (list (lambda () (setq kill-emacs-hook nil) (princ "HOOK") (kill-emacs 23)))))
      (unwind-protect (kill-emacs 17) (princ "CLEANUP") (kill-emacs 99)))"#, 23, "HOOK");
}

#[test]
fn implicit_batch_exit_runs_remaining_hooks_after_an_error() {
    check(r#"(setq kill-emacs-hook
                   (list (lambda () (princ "FIRST") (error "hook-error"))
                         (lambda () (princ "SECOND"))))"#, 0, "FIRSTSECOND");
}

#[test]
fn shutdown_hooks_observe_changes_to_the_remaining_functions() {
    check(r#"(let ((kill-emacs-hook
                    (list (lambda ()
                            (princ "FIRST")
                            (setcdr kill-emacs-hook (list (lambda () (princ "NEW")))))
                          (lambda () (princ "OLD")))))
      (kill-emacs 17))"#, 17, "FIRSTNEW");
}

#[test]
fn hook_removal_watcher_errors_abort_shutdown() {
    check(r#"(progn
      (defun qa-exit-bad () (princ "BAD") (error "failure"))
      (defun qa-exit-watch (symbol _value operation _where)
        (when (eq operation 'set)
          (remove-variable-watcher symbol #'qa-exit-watch)
          (error "remove-failure")))
      (let ((kill-emacs-hook (list #'qa-exit-bad (lambda () (princ "LATER")))))
        (add-variable-watcher 'kill-emacs-hook #'qa-exit-watch)
        (condition-case err (kill-emacs 17) (error (princ (cadr err)))))
      (princ "AFTER"))"#, 0, "BADremove-failureAFTER");
}

#[test]
fn hook_removal_notifies_watchers_of_local_bindings() {
    check(r#"(progn
      (defun qa-exit-bad () (princ "BAD") (error "failure"))
      (defun qa-exit-watch (symbol _value operation where)
        (when (eq operation 'set)
          (remove-variable-watcher symbol #'qa-exit-watch)
          (when (bufferp where) (princ "LOCAL"))
          (error "remove-failure")))
      (with-temp-buffer
        (setq-local kill-emacs-hook (list #'qa-exit-bad (lambda () (princ "LATER"))))
        (add-variable-watcher 'kill-emacs-hook #'qa-exit-watch)
        (condition-case err (kill-emacs 17) (error (princ (cadr err)))))
      (princ "AFTER"))"#, 0, "BADLOCALremove-failureAFTER");
}

#[test]
fn hook_removal_changes_the_default_without_overwriting_local_entries() {
    check(r#"(progn
      (defun qa-exit-bad () (error "failure"))
      (defun qa-exit-stop () (throw 'abort-exit 'stopped))
      (let ((kill-emacs-hook (list #'qa-exit-bad)))
        (with-temp-buffer
          (setq-local kill-emacs-hook (list t #'qa-exit-stop))
          (catch 'abort-exit (kill-emacs 17))
          (prin1 (list (default-value 'kill-emacs-hook) kill-emacs-hook)))))"#,
          0, "(nil (t qa-exit-stop))");
}

#[test]
fn implicit_batch_exit_exposes_the_real_shutdown_frames_and_argument() {
    check(r#"(setq kill-emacs-hook
                   (list (lambda ()
                           (let (frames)
                             (dolist (frame (backtrace-frames))
                               (when (memq (cadr frame) '(kill-emacs command-line normal-top-level))
                                 (push (list (cadr frame) (nth 2 frame)) frames)))
                             (prin1 (nreverse frames))))))"#,
          0, "((kill-emacs (t)) (command-line nil) (normal-top-level nil))");
}

#[test]
fn implicit_batch_exit_calls_advice_on_the_public_function() {
    check(r#"(advice-add 'kill-emacs :before (lambda (&rest args) (prin1 args)))"#,
          0, "(t)");
}

#[test]
fn implicit_batch_exit_calls_a_replacement_function_cell() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest args) (princ "WRAPPED") (apply #'qa-original-kill args))))"#,
          0, "WRAPPED");
}

#[test]
fn a_returning_shutdown_replacement_allows_startup_hooks_to_continue() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest args) (prin1 args)))
      (setq emacs-startup-hook (list (lambda () (princ "STARTUP")))
            term-setup-hook (list (lambda () (princ "TERM")))
            window-setup-hook (list (lambda () (princ "WINDOW") (qa-original-kill 19)))))"#,
          19, "(t)STARTUPTERMWINDOW");
}

#[test]
fn a_returning_shutdown_replacement_allows_queued_commands_to_run() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest args) (princ "RETURN")))
      (global-set-key [qa-exit] (lambda () (interactive) (princ "COMMAND") (qa-original-kill 21)))
      (setq unread-command-events '(qa-exit)))"#,
          21, "RETURNCOMMAND");
}

#[test]
fn a_returning_shutdown_replacement_keeps_the_timer_loop_alive() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest args) (princ "RETURN")))
      (run-at-time 0.2 nil (lambda () (princ "TIMER") (qa-original-kill 22))))"#,
          22, "RETURNTIMER");
}

#[test]
fn startup_hooks_remain_inhibited_after_a_shutdown_replacement_returns() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest args) (princ "RETURN")))
      (setq inhibit-startup-hooks t emacs-startup-hook (list (lambda () (princ "BAD"))))
      (run-at-time 0.2 nil (lambda () (qa-original-kill 23))))"#,
          23, "RETURN");
}

#[test]
fn a_startup_hook_can_inhibit_the_later_window_setup_hook() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest _) (princ "RETURN")))
      (setq emacs-startup-hook (list (lambda () (setq inhibit-startup-hooks t) (princ "STARTUP")))
            term-setup-hook (list (lambda () (princ "TERM")))
            window-setup-hook (list (lambda () (princ "BAD"))))
      (run-at-time 0.2 nil (lambda () (qa-original-kill 24))))"#,
          24, "RETURNSTARTUPTERM");
}

#[test]
fn a_batch_command_error_reports_before_post_command_hooks_and_restores_input_state() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest _) (princ "RETURN")))
      (setq post-command-hook (list (lambda () (princ "POST")))
            pre-command-hook (list (lambda () (princ "PRE"))))
      (setq command-error-function (lambda (&rest _) (princ "ERROR")))
      (global-set-key [qa-fail] (lambda () (interactive) (error "failure")))
      (global-set-key [qa-exit] (lambda () (interactive)
        (prin1 (list inhibit-quit quit-flag (eq standard-input t) (eq standard-output t)))
        (qa-original-kill 25)))
      (setq unread-command-events '(qa-fail qa-exit)))"#,
          25, "RETURNPOSTPREERRORPOSTPRE(nil nil t t)");
}

#[test]
fn continued_startup_calls_the_public_hook_runner() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest _) (princ "RETURN")))
      (advice-add 'run-hooks :before (lambda (&rest hooks)
        (when (or (memq 'emacs-startup-hook hooks) (memq 'window-setup-hook hooks))
          (prin1 hooks))))
      (setq window-setup-hook (list (lambda () (qa-original-kill 26)))))"#,
          26, "RETURN(emacs-startup-hook term-setup-hook)(window-setup-hook)");
}

#[test]
fn a_reported_shutdown_error_allows_the_batch_command_loop_to_continue() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'debug-early--handler #'ignore)
      (fset 'kill-emacs (lambda (&rest _) (error "shutdown-error")))
      (setq emacs-startup-hook (list (lambda () (princ "STARTUP"))))
      (setq command-error-function (lambda (&rest _) (princ "REPORT")))
      (run-at-time 0.2 nil (lambda () (qa-original-kill 27))))"#,
          27, "STARTUPREPORT");
}

#[test]
fn a_shutdown_replacement_can_throw_to_the_top_level() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest _) (princ "THROW") (top-level)))
      (setq emacs-startup-hook (list (lambda () (princ "STARTUP"))))
      (run-at-time 0.2 nil (lambda () (qa-original-kill 28))))"#,
          28, "THROWSTARTUP");
}

#[test]
fn a_failing_batch_command_still_uses_the_direct_fatal_exit() {
    for output in check(r#"(progn
      (fset 'kill-emacs (lambda (&rest _) (princ "RETURN")))
      (global-set-key [qa-fail] (lambda () (interactive) (error "failure")))
      (setq unread-command-events '(qa-fail)))"#,
          255, "RETURN") {
        assert!(String::from_utf8_lossy(&output.stderr).contains("failure"));
    }
}

#[test]
fn a_timer_top_level_throw_reaches_the_direct_batch_exit() {
    check(r#"(progn
      (fset 'qa-original-kill (symbol-function 'kill-emacs))
      (fset 'kill-emacs (lambda (&rest _) (princ "RETURN")))
      (setq kill-emacs-hook (list (lambda () (princ "HOOK"))))
      (run-at-time 0.1 nil (lambda () (princ "TIMER") (top-level)))
      (run-at-time 0.2 nil (lambda () (princ "BAD") (qa-original-kill 29))))"#,
          0, "RETURNTIMERHOOK");
}

#[test]
fn interactive_host_stops_lisp_after_shutdown_and_retains_the_exit() {
    on_big_stack(|| {
        let (mut i, out) = interp();
        let result = i.eval_str(r#"(progn (setq kill-emacs-hook nil)
          (unwind-protect (kill-emacs 31) (princ "CLEANUP")))"#);
        assert!(matches!(result, Err(Flow::Exit(31))), "{result:?}");
        assert!(i.quit_editor);
        assert!(out.borrow().is_empty(), "{}", out.borrow());
        assert!(matches!(i.eval_str("(princ \"AFTER\")"), Err(Flow::Exit(31))));
        let print = Value::Sym(i.intern("princ"));
        assert!(matches!(i.apply(&print, vec![Value::string("AFTER")]), Err(Flow::Exit(31))));
        assert!(out.borrow().is_empty());
    });
}
