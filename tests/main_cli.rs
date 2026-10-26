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
fn external_debugging_output_emits_high_mule_bytes() {
    let output = Command::new(env!("CARGO_BIN_EXE_remacs"))
        .args(["-Q", "--batch", "--eval", "(external-debugging-output #x110000)"])
        .output()
        .expect("run remacs");
    assert!(output.status.success());
    assert_eq!(output.stderr, [0xF4, 0x90, 0x80, 0x80]);
}

#[test]
fn string_reverse_is_void_until_subrx_installs_the_real_alias() {
    let code = "(let ((before (fboundp 'string-reverse)) \
        (call (condition-case e (string-reverse [a b]) (error (car e))))) \
        (require 'subr-x) \
        (prin1 (list before call (symbol-function 'string-reverse) (string-reverse [a b]))))";
    for source in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
        command.args(["-Q", "--batch", "--eval", code]);
        if source {
            command.env("REMACS_NO_ELC", "1");
        } else {
            command.env_remove("REMACS_NO_ELC");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "source={source}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "(nil void-function reverse [b a])", "source={source}");
    }
}

#[cfg(target_os = "macos")]
fn remacs_before_deadline(args: &[&str]) -> (String, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_remacs"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn remacs");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "remacs did not finish: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
#[cfg(target_os = "macos")]
fn huge_process_timeout_is_catchable_and_evaluation_continues() {
    let (out, err, code) = remacs_before_deadline(&["-Q", "--batch", "--eval",
        "(progn (prin1 (condition-case err (accept-process-output nil 1e30) (file-error err))) (terpri) (princ \"AUDIT-DONE\"))"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "(file-error \"Failed select\" \"Invalid argument\")\nAUDIT-DONE"
    );
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
#[cfg(target_os = "macos")]
fn unhandled_huge_process_timeout_is_a_failure_exit() {
    let (out, err, code) = remacs_before_deadline(&[
        "-Q",
        "--batch",
        "--eval",
        "(progn (accept-process-output nil 1e30) (princ \"unreachable\"))",
    ]);
    assert_ne!(code, 0, "{out}\n{err}");
    assert!(!out.contains("unreachable"), "{out}");
    assert!(err.contains("Failed select"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
#[cfg(target_os = "macos")]
fn huge_process_timeout_does_not_reuse_a_child_exit_notification() {
    let (out, err, code) = remacs_before_deadline(&["-Q", "--batch", "--eval",
        "(let ((p (make-pipe-process :name \"timeout-target\" :noquery t :sentinel #'ignore)) \
               (other (make-process :name \"timeout-exit\" :noquery t :connection-type 'pipe :sentinel #'ignore :command '(\"/bin/sh\" \"-c\" \"exit 0\")))) \
          (unwind-protect \
              (progn \
                (let ((deadline (+ (float-time) 3)) done) \
                  (while (and (not done) (< (float-time) deadline)) \
                    (let* ((attrs (process-attributes (process-id other))) (state (cdr (assq 'state attrs)))) \
                      (setq done (or (null attrs) (and (stringp state) (string-prefix-p \"Z\" state))))) \
                    (unless done (thread-yield))) \
                  (unless done (error \"Child did not exit\"))) \
                (prin1 (condition-case err (accept-process-output p 1e30 nil t) (file-error err))) \
                (princ \" AUDIT-DONE\")) \
            (mapc #'delete-process (list p other))))"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "(file-error \"Failed select\" \"Invalid argument\") AUDIT-DONE"
    );
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn version_prints_and_exits() {
    let (o, _e, code) = remacs(&["--version"], "");
    assert!(o.contains("remacs"), "version output: {}", o);
    assert_eq!(code, 0);
}

#[test]
fn build_identity_is_reproducible_and_records_the_configured_target() {
    let expression = "(let ((print-escape-newlines t)) \
        (prin1 (list system-configuration emacs-build-system emacs-build-time (emacs-version))))";
    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_remacs"))
            .args(["-Q", "--batch", "--eval", expression])
            // The configured target belongs to the binary, not its caller.
            .env("REMACS_SYSTEM_CONFIGURATION", "runtime-value")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let first = run();
    assert!(
        first.starts_with(&format!(
            "(\"{}\" nil nil \"GNU Emacs 31.1 (build ",
            env!("REMACS_SYSTEM_CONFIGURATION")
        )),
        "{first}"
    );
    assert!(!first.contains("\\n of "), "{first}");
    assert_eq!(first, run(), "build identity changed between processes");
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let output = Command::new(gnu)
            .args(["-Q", "--batch", "--eval", expression])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(first, String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn eval_runs_lisp() {
    let (o, _e, code) = remacs(&["--batch", "--eval", "(princ (+ 1 2))"], "");
    assert_eq!(o, "3");
    assert_eq!(code, 0);
}

// Issue #225: the reference GNU 31.1 reads only the first object from
// each argument. Later forms and even invalid trailing syntax are unread.
#[test]
fn eval_first_object_leaves_trailing_input_unread() {
    for option in ["--eval", "--execute"] {
        for (expression, expected) in [
            ("(princ 1) (princ 2)", "1"),
            ("(princ 1))", "1"),
            ("; leading comment\n(princ 1) (error \"unread\")", "1"),
            ("(princ 1) ; trailing comment", "1"),
            ("nil (princ 2)", ""),
            ("123", ""),
        ] {
            let args = ["-Q", "--batch", option, expression];
            if let Some(gnu) = std::env::var_os("GNU_EMACS") {
                let result = Command::new(gnu).args(args).output().unwrap();
                assert_eq!(result.status.code(), Some(0), "{expression}");
                assert_eq!(String::from_utf8_lossy(&result.stdout), expected);
            }
            let (output, errors, status) = remacs(&args, "");
            assert_eq!(status, 0, "{option} {expression}: {errors}");
            assert_eq!(output, expected, "{option} {expression}");
        }
    }
}

#[test]
fn eval_first_read_error_aborts_remaining_options() {
    for option in ["--eval", "--execute"] {
        for (expression, condition) in [
            ("", "end-of-file"),
            (" \t\n", "end-of-file"),
            ("; only a comment", "end-of-file"),
            ("(princ 1", "end-of-file"),
            (") (princ 2)", "invalid-read-syntax"),
        ] {
            // A read failure must exit 255 before a later action can exit 17.
            let args = [
                "-Q",
                "--batch",
                option,
                expression,
                "--eval",
                "(kill-emacs 17)",
            ];
            if let Some(gnu) = std::env::var_os("GNU_EMACS") {
                let result = Command::new(gnu).args(args).output().unwrap();
                assert_eq!(result.status.code(), Some(255), "{expression}");
                let diagnostic = format!(
                    "{}{}",
                    String::from_utf8_lossy(&result.stdout),
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(diagnostic.contains(condition), "{diagnostic}");
            }
            let (_output, errors, status) = remacs(&args, "");
            assert_eq!(status, 255, "{option} {expression}: {errors}");
            assert!(errors.contains(condition), "{errors}");
        }
    }
}

#[test]
fn eval_first_form_keeps_lexical_evaluation_and_option_order() {
    for (first, second) in [("--eval", "--execute"), ("--execute", "--eval")] {
        let args = [
            "-Q",
            "--batch",
            first,
            "(setq qa225-value 41) (setq qa225-value 0)",
            second,
            "(prin1 (list lexical-binding (let ((x qa225-value)) (funcall (lambda () (+ x 1))))))",
        ];
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu).args(args).output().unwrap();
            assert_eq!(result.status.code(), Some(0));
            assert_eq!(String::from_utf8_lossy(&result.stdout), "(t 42)");
        }
        let (output, errors, status) = remacs(&args, "");
        assert_eq!(status, 0, "{errors}");
        assert_eq!(output, "(t 42)");
    }
}

#[test]
fn a_worker_can_exit_the_batch_process() {
    let (out, _, code) = remacs(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(progn (make-thread (lambda () (thread-yield) (kill-emacs 23))) \
           (thread-yield) (thread-yield) (princ \"unreachable\"))",
        ],
        "",
    );
    assert_eq!(code, 23);
    assert_eq!(out, "");
}

#[test]
fn quick_start_symbol_metadata_matches_gnu() {
    // Probe before loading Lisp source: those loads may legitimately
    // require cl-macs, pcase, gv and icons and change their metadata.
    let expression = "(prin1 (list (length (face-list)) \
        (mapcar #'fboundp '(cl-declaim cl-proclaim cl-loop cl-case)) \
        (get 'pcase 'lisp-indent-function) (get 'pcase 'edebug-form-spec) \
        (symbol-plist 'car) (featurep 'cl-macs) (featurep 'gv) \
        (featurep 'icons) (featurep 'cl-lib)))";
    let expected = "(187 (nil nil nil nil) nil nil (slot-name t) nil nil nil nil)";
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let output = Command::new(gnu)
            .args(["-Q", "--batch", "--eval", expression])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
    let (output, errors, status) = remacs(&["-Q", "--batch", "--eval", expression], "");
    assert_eq!(status, 0, "{errors}");
    assert_eq!(output, expected);
}

#[test]
fn boot_dependencies_allow_unloading_cl_lib() {
    let expression = "(progn (require 'cl-lib) \
        (prin1 (list (unload-feature 'cl-lib) (featurep 'cl-lib) (fboundp 'cl-plusp))))";
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("remacs-unload-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    for cwd in [std::path::Path::new(env!("CARGO_MANIFEST_DIR")), &directory] {
        for source in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
            command
                .current_dir(cwd)
                .args(["-Q", "--batch", "--eval", expression]);
            if source {
                command.env("REMACS_NO_ELC", "1");
            } else {
                command.env_remove("REMACS_NO_ELC");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "cwd={cwd:?}, source={source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout), "(nil nil nil)");
        }
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn boot_menu_properties_keep_functions_and_runtime_closures_trim_context() {
    let expression = r#"(let ((cua-mode nil) (mark-active nil))
      (prin1 (list
        (mapcar (lambda (name)
          (let* ((item (cdr (assq name (cdr menu-bar-edit-menu))))
                 (keys (plist-get (nthcdr 3 item) :keys))
                 (wanted (cond ((eq name 'cut) "\\[kill-region]")
                               ((eq name 'paste) "\\[yank]")
                               ((featurep 'ns) "\\[ns-copy-including-secondary]")
                               (t "\\[kill-ring-save]"))))
            (list (functionp keys) (and keys (equal (funcall keys) wanted)))))
          '(cut copy paste))
        (let* ((item (cdr (assq 'paste (cdr menu-bar-edit-menu))))
               (enable (plist-get (nthcdr 3 item) :enable)))
          (functionp (cadr (cadr enable))))
        (eval '(let ((unused-menu (make-sparse-keymap "E")))
                 (aref (lambda () "k") 2)) t))))"#;
    let expected = "(((t t) (t t) (t t)) t (t))";
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let output = Command::new(gnu)
            .args(["-Q", "--batch", "--eval", expression])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
    let (output, errors, status) = remacs(&["-Q", "--batch", "--eval", expression], "");
    assert_eq!(status, 0, "{errors}");
    assert_eq!(output, expected);
}

#[test]
fn remote_identification_autoloads_tramp_on_first_call() {
    // No source/framework is loaded before the first file-remote-p call.
    let expression = "(prin1 (list (featurep 'tramp) \
        (file-remote-p \"/ssh:qa21.invalid:/file\") \
        (file-remote-p \"/ssh:u@qa21.invalid#2222:/file\" 'host) \
        (file-remote-p \"/ssh:qa21.invalid:/file\" nil t) (featurep 'tramp)))";
    let expected = "(nil \"/ssh:qa21.invalid:\" \"qa21.invalid#2222\" nil t)";
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let output = Command::new(gnu)
            .args(["-Q", "--batch", "--eval", expression])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
    let (output, errors, status) = remacs(&["-Q", "--batch", "--eval", expression], "");
    assert_eq!(status, 0, "{errors}");
    assert_eq!(output, expected);
}

#[test]
fn synchronous_process_stderr_stays_with_destination() {
    let expression = r#"(let ((print-escape-newlines t))
      (prin1 (list
        (with-temp-buffer
          (call-process "sh" nil t nil "-c" "echo out; echo qa25-child-stderr >&2")
          (buffer-string))
        (with-temp-buffer
          (insert "input")
          (call-process-region 1 6 "sh" t '(t nil) nil "-c"
                               "echo out; echo qa25-child-stderr >&2")
          (buffer-string)))))"#;
    let expected = r#"("out\nqa25-child-stderr\n" "out\n")"#;
    let (output, errors, status) = remacs(&["-Q", "--batch", "--eval", expression], "");
    assert_eq!(status, 0, "{errors}");
    assert_eq!(output, expected);
    assert!(
        !errors.contains("qa25-child-stderr"),
        "child stderr leaked: {errors}"
    );
}

#[test]
fn eval_error_exits_nonzero() {
    let (_o, e, code) = remacs(&["--batch", "--eval", "(car 1)"], "");
    assert_ne!(code, 0);
    assert!(e.contains("Error"), "stderr: {}", e);
}

#[test]
fn eval_uncaught_throw_reports() {
    let (_o, e, code) = remacs(&["--batch", "--eval", "(throw 'x 1)"], "");
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
fn source_with_elc_suffix_requires_script_evaluation() {
    let file = std::env::temp_dir().join(format!("remacs-cli-header-{}.elc", std::process::id()));
    std::fs::write(&file, "(princ \"source-script\")").unwrap();
    let path = file.to_str().unwrap();
    let (output, error, code) = remacs(&["--batch", "-l", path], "");
    assert_ne!(code, 0);
    assert!(output.is_empty());
    assert!(error.contains("was not compiled in Emacs"), "{error}");
    let (output, error, code) = remacs(&["--script", path], "");
    let _ = std::fs::remove_file(&file);
    assert_eq!(code, 0, "{error}");
    assert_eq!(output, "source-script");
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
    let (_o, e, code) = remacs(&["--batch", "--eval", "(signal 'quit nil)"], "");
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

// Issue #226: file arguments are actions in the same stream as --eval.
struct FileArgumentFixture(std::path::PathBuf);
impl FileArgumentFixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "remacs-cli-files-{}-{nonce}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/first file.txt"), "alpha\n").unwrap();
        std::fs::write(dir.join("second.txt"), "beta\n").unwrap();
        Self(dir)
    }

    fn check(&self, args: &[&str], expected: &str) {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(
                result.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&result.stdout), expected);
        }
        for source in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
            command.current_dir(&self.0).args(args);
            if source {
                command.env("REMACS_NO_ELC", "1");
            } else {
                command.env_remove("REMACS_NO_ELC");
            }
            let result = command.output().unwrap();
            assert_eq!(
                result.status.code(),
                Some(0),
                "source={source}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&result.stdout),
                expected,
                "source={source}"
            );
        }
    }
}
impl Drop for FileArgumentFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn batch_file_arguments_use_the_startup_directory_in_order() {
    let files = FileArgumentFixture::new();
    files.check(
        &[
            "-Q",
            "--batch",
            "sub/first file.txt",
            "--eval",
            "(princ (buffer-string))",
            "second.txt",
            "--execute",
            "(princ (buffer-string))",
        ],
        "alpha\nbeta\n",
    );
    // Batch is a startup switch even when it follows the first file.
    files.check(
        &[
            "-Q",
            "sub/first file.txt",
            "--batch",
            "--eval",
            "(princ (buffer-string))",
        ],
        "alpha\n",
    );
}

#[test]
fn batch_file_arguments_reselect_existing_buffers_and_keep_point() {
    let files = FileArgumentFixture::new();
    files.check(
        &[
            "-Q",
            "--batch",
            "sub/first file.txt",
            "--eval",
            "(progn (princ (buffer-string)) (goto-char 3))",
            "second.txt",
            "--eval",
            "(princ (buffer-string))",
            "sub/first file.txt",
            "--eval",
            "(prin1 (list (point) (buffer-string)))",
        ],
        "alpha\nbeta\n(3 \"alpha\n\")",
    );
}

#[test]
fn batch_file_arguments_reach_exit_hooks_and_allow_new_files() {
    let files = FileArgumentFixture::new();
    files.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(add-hook 'kill-emacs-hook (lambda () (princ (buffer-string))))",
            "sub/first file.txt",
            "second.txt",
        ],
        "beta\n",
    );
    files.check(&["-Q", "--batch", "new file.txt", "--eval",
                  "(prin1 (list (file-name-nondirectory buffer-file-name) (buffer-string) (file-exists-p buffer-file-name)))"],
                "(\"new file.txt\" \"\" nil)");
    assert!(!files.0.join("new file.txt").exists());
}
#[test]
fn execute_alias_works() {
    let (o, _e, code) = remacs(&["--batch", "--execute", "(princ 7)"], "");
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
    let (_o, e, code) = remacs(&["--batch", "--eval", "(catch 'zz (throw 'zz 5))"], "");
    assert_eq!(code, 0);
    let _ = e;
}

// Inspect printable terminal output, including names split by SGR changes.
// CSI commands and OSC titles are control data, not rendered buffer names.
fn terminal_text(output: &[u8]) -> String {
    let mut text = Vec::new();
    let mut bytes = output.iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != 0x1b {
            text.push(byte);
            continue;
        }
        match bytes.next() {
            Some(b'[') => {
                for byte in bytes.by_ref() {
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            Some(b']') => {
                let mut escape = false;
                for byte in bytes.by_ref() {
                    if byte == 7 || (escape && byte == b'\\') {
                        break;
                    }
                    escape = byte == 0x1b;
                }
            }
            Some(0x20..=0x2f) => {
                for byte in bytes.by_ref() {
                    if (0x30..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    String::from_utf8_lossy(&text).into_owned()
}

#[test]
fn pty_frame_detection_ignores_ansi_sequences() {
    let split_name = b"----- *scr\x1b[0match*   L1 C0  [sel]";
    assert!(!String::from_utf8_lossy(split_name).contains("scratch"));
    assert!(terminal_text(split_name).contains("*scratch*"));
    assert_eq!(
        terminal_text(b"\x1b]0;*scratch*\x07\x1b[2Jloading\x1b[0m"),
        "loading"
    );
    assert_eq!(terminal_text(b"\x1b]0;title\x1b\\\x1b(Btext"), "text");
    assert!(!terminal_text(b"*scr\x1b[0").contains("*scratch*"));
}

struct PtyTranscript(std::path::PathBuf);
impl PtyTranscript {
    fn text(&self) -> String {
        terminal_text(&std::fs::read(&self.0).expect("read PTY transcript"))
    }
}
impl Drop for PtyTranscript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct PtyChild(std::process::Child);
impl Drop for PtyChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let transcript = PtyTranscript(std::env::temp_dir().join(format!(
        "remacs-pty-{}-{nonce}.log",
        std::process::id()
    )));
    let output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&transcript.0)
        .unwrap();
    let mut cmd = Command::new("script");
    // Headless runners give script a 0x0 PTY. Set the geometry that
    // this split-window / mode-line test actually requires; otherwise
    // the clamped 10x3 screen truncates "scratch" before the wait below.
    cmd.args([
        "-q",
        "/dev/null",
        "/bin/sh",
        "-c",
        "stty rows 24 cols 80 && exec \"$1\" -Q -nw",
        "remacs-pty",
        env!("CARGO_BIN_EXE_remacs"),
    ])
    .env("TERM", "xterm-256color")
    .stdin(Stdio::piped())
    .stdout(output.try_clone().unwrap())
    .stderr(output);
    let mut child = PtyChild(cmd.spawn().expect("spawn script"));
    let mut stdin = child.0.stdin.take().unwrap();
    // A regular file keeps inherited pipe writers from blocking collection.
    // Wait for the initial frame (the *scratch* mode line).  Under
    // parallel test load the ~30s prelude can stretch past 60s;
    // 120s still bounds a wedged first-render loop.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let text = transcript.text();
        if text.contains("*scratch*") {
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("editor exited before its first frame: {status}\n{text}");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the first rendered frame\n{text}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let keys: Vec<&[u8]> = vec![
        b"hello",
        b"\x15\x35z",      // C-u 5 z (universal/digit-argument path)
        b"\x1b-\x33x",     // M-- 3 x (negative-argument path)
        b"\x182",          // C-x 2 (split-window-below)
        b"\x18o",          // C-x o (other-window)
        b"\x18bbb\r",      // C-x b bb RET (switch-to-buffer)
        b"\x1b:(+ 1 2)\r", // M-: eval-expression
        b"\x13el",         // C-s el (isearch, stay in search)
        b"\x13",           // C-s again (repeat-forward dispatch)
        b"\x7f",           // DEL (isearch pop)
        b"\x06",           // C-f inside isearch (ReDispatch path)
        b"\x13z",          // C-s z (re-enter isearch)
        b"\r",             // RET exits isearch
        "あ".as_bytes(),   // unbound multibyte char → self-insert
        b"\x1e",           // C-^ (unbound control char)
        b"\x07",           // C-g
        b"\x0b",           // C-k (kill-line)
        b"\x19",           // C-y (yank)
        b"\x1by",          // M-y (yank-pop)
        b"\x1f",           // C-/ (undo)
        b"\x0c",           // C-l (recenter-top-bottom)
        b"\x08k\x06",      // C-h k C-f (describe-key)
        b"\x15\x07",       // C-u C-g (abort prefix arg)
        // GNU `read-extended-command' passes require-match=t to
        // `completing-read', so a name with no matching command is
        // refused on RET ("No match" + reprompt) — abort that read
        // with C-g like a real user would.
        b"\x1bxxdescrib\r",          // M-x xdescrib RET → refused
        b"\x07",                     // C-g aborts the M-x minibuffer
        b"\x1bxdescribe-bindings\r", // M-x describe-bindings RET
        b"\x18\x03",                 // C-x C-c
    ];
    for k in keys {
        stdin.write_all(k).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    drop(stdin);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "editor did not exit after C-x C-c\n{}",
            transcript.text()
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let text = transcript.text();
    assert!(
        text.contains("*scratch*"),
        "expected a rendered frame, got: {}",
        text.chars().take(400).collect::<String>()
    );
    assert!(status.success());
}
