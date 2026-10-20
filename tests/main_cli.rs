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
    let (o, _e, code) = remacs(&["--eval", "(princ (+ 1 2))"], "");
    assert_eq!(o, "3");
    assert_eq!(code, 0);
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
    // Headless runners give script a 0x0 PTY. Set the geometry that
    // this split-window / mode-line test actually requires; otherwise
    // the clamped 10x3 screen truncates "scratch" before the wait below.
    cmd.args([
        "-q",
        "/dev/null",
        "/bin/sh",
        "-c",
        "stty rows 24 cols 80; exec \"$1\" -nw",
        "remacs-pty",
        env!("CARGO_BIN_EXE_remacs"),
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn script");
    let mut stdin = child.stdin.take().unwrap();
    // Collect rendered output in the background so we can wait for the
    // first frame before typing — otherwise input sent during startup
    // is merely echoed and the editor may see C-x C-c before ever
    // drawing, making the test racy under load.
    let stdout = child.stdout.take().unwrap();
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let buf_reader = buf.clone();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut so = stdout;
        let mut tmp = [0u8; 8192];
        loop {
            match so.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf_reader.lock().unwrap().extend_from_slice(&tmp[..n]),
            }
        }
        buf_reader
    });
    // Wait for the initial frame (the *scratch* mode line).  Under
    // parallel test load the ~30s prelude can stretch past 60s;
    // 120s still bounds a wedged first-render loop.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        if String::from_utf8_lossy(&buf.lock().unwrap()).contains("scratch") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for the first rendered frame"
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
    let status = child.wait().unwrap();
    let out = reader.join().unwrap();
    let text = String::from_utf8_lossy(&out.lock().unwrap()).into_owned();
    assert!(
        text.contains("scratch"),
        "expected a rendered frame, got: {}",
        &text[..text.len().min(400)]
    );
    assert!(status.success());
}
