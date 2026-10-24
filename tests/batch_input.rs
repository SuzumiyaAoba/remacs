//! Physical stdin is a minibuffer byte stream, distinct from keyboard events.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run(binary: &OsString, source: bool, expression: &str, input: &[u8]) -> Output {
    let mut command = Command::new(binary);
    command.args(["-Q", "--batch", "--eval", expression])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if source { command.env("REMACS_NO_ELC", "1"); }
    else { command.env_remove("REMACS_NO_ELC"); }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let read = |mut pipe: Box<dyn Read + Send>| std::thread::spawn(move || {
        let mut bytes = Vec::new(); pipe.read_to_end(&mut bytes).unwrap(); bytes
    });
    let stdout = read(Box::new(child.stdout.take().unwrap()));
    let stderr = read(Box::new(child.stderr.take().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() { break status; }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = Output { status, stdout: stdout.join().unwrap(), stderr: stderr.join().unwrap() };
    assert!(!timed_out, "{binary:?} source={source} timed out\n{}\n{}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    output
}

fn check(expression: &str, input: &[u8], expected: &[u8], source: bool) {
    let reference_only = std::env::var_os("REMACS_BATCH_GNU_ONLY").is_some();
    let mut binaries = Vec::new();
    if !reference_only {
        let binary = OsString::from(env!("CARGO_BIN_EXE_remacs"));
        binaries.push((binary.clone(), false));
        if source { binaries.push((binary, true)); }
    }
    if let Some(gnu) = std::env::var_os("GNU_EMACS") { binaries.push((gnu, false)); }
    else { assert!(!reference_only, "GNU_EMACS is required for reference-only checks"); }
    for (binary, source) in binaries {
        let output = run(&binary, source, expression, input);
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{binary:?} source={source}\n{}\n{err}", String::from_utf8_lossy(&output.stdout));
        assert_eq!(output.stdout, expected, "{binary:?} source={source}\n{err}");
    }
}

const STRING_INFO: &str = r#"(let ((s (read-string "P:"))) (prin1 (list (multibyte-string-p s) (string-to-list s))))"#;

#[test]
fn ascii_and_empty_lines_are_unibyte() {
    check(STRING_INFO, b"abc\n", b"P:(nil (97 98 99))", false);
    check(STRING_INFO, b"\n", b"P:(nil nil)", false);
}

#[test]
fn valid_utf8_is_multibyte() {
    check(STRING_INFO, "é日本😀\n".as_bytes(), b"P:(t (233 26085 26412 128512))", false);
}

#[test]
fn invalid_bytes_are_preserved_including_nul() {
    check(STRING_INFO, b"\0\xff\n", b"P:(nil (0 255))", false);
}

#[test]
fn one_invalid_byte_preserves_the_entire_original_line() {
    check(STRING_INFO, b"\xc3\xa9\xff\n", b"P:(nil (195 169 255))", false);
}

#[test]
fn truncated_and_overlong_sequences_are_unibyte() {
    check(STRING_INFO, b"\xc3\n", b"P:(nil (195))", false);
    check(STRING_INFO, b"\xc0\xaf\n", b"P:(nil (192 175))", false);
    check(STRING_INFO, b"\xe0\x80\xaf\n", b"P:(nil (224 128 175))", false);
    check(STRING_INFO, b"\xc1\xbf\n", b"P:(nil (193 191))", false);
}

#[test]
fn canonical_surrogate_and_mule_sequences_are_multibyte() {
    check(STRING_INFO, b"\xed\xa0\x80\xf4\x90\x80\x80\xf8\x88\x80\x80\x80\n",
          b"P:(t (55296 1114112 2097152))", false);
}

#[test]
fn reserved_byte8_codes_are_not_accepted_as_five_byte_sequences() {
    check(STRING_INFO, b"\xf8\x8f\xbf\xbf\xbf\n", b"P:(nil (248 143 191 191 191))", false);
    check(STRING_INFO, b"\xf8\x8f\xbf\xbd\xbf\n", b"P:(t (4194175))", false);
}

#[test]
fn input_detection_ignores_locale_coding_and_preserves_the_last_coding_variable() {
    check(r#"(let ((locale-coding-system 'latin-1) (last-coding-system-used 'sentinel))
      (let ((s (read-string "P:"))) (prin1 (list (multibyte-string-p s) (string-to-list s) last-coding-system-used))))"#,
      b"\xc3\xa9\n", b"P:(t (233) sentinel)", false);
}

#[test]
fn carriage_return_does_not_consume_the_following_line_feed() {
    check(r#"(prin1 (list (read-string "A:") (read-string "B:") (read-string "C:")))"#,
      b"a\r\nb\n", b"A:B:C:(\"a\" \"\" \"b\")", false);
}

#[test]
fn a_partial_line_at_eof_is_returned_before_the_next_read_signals() {
    check(r#"(prin1 (list (read-string "A:") (condition-case err (read-string "B:") (error err))))"#,
      b"tail", b"A:B:(\"tail\" (end-of-file \"Error reading from stdin\"))", false);
}

#[test]
fn stdin_string_readers_keep_byte_kind() {
    for reader in [r#"(read-from-minibuffer "P:")"#, r#"(completing-read "P:" nil)"#,
                   r#"(read-buffer "P:")"#, r#"(read-regexp "P:")"#] {
        let expression = format!("(let ((s {reader})) (prin1 (list (multibyte-string-p s) (string-to-list s))))");
        check(&expression, b"\xff\n", b"P:(nil (255))", false);
    }
}

#[test]
fn object_reads_preserve_raw_string_and_symbol_name_bytes() {
    for expression in [
        r#"(let ((s (read-from-minibuffer "P:" nil nil t)))
             (prin1 (list (multibyte-string-p s) (string-to-list s))))"#,
        r#"(progn (defun qa-input-object (s) (interactive "xP:")
                    (prin1 (list (multibyte-string-p s) (string-to-list s))))
                  (call-interactively 'qa-input-object))"#,
    ] {
        check(expression, b"\"\xff\"\n", b"P:(nil (255))", false);
    }
    check(r#"(let ((s (symbol-name (read-from-minibuffer "P:" nil nil t))))
        (prin1 (list (multibyte-string-p s) (string-to-list s))))"#,
        b"#:\xff\n", b"P:(nil (255))", false);
    check(r#"(let* ((default (concat "\"" (unibyte-string 255) "\""))
                  (s (read-from-minibuffer "P:" nil nil t nil default)))
        (prin1 (list (multibyte-string-p s) (string-to-list s))))"#,
        b"\n", b"P:(nil (255))", false);
}

#[test]
fn read_and_positioned_read_share_the_minibuffer_stdin_dispatch() {
    for reader in ["read", "read-positioning-symbols"] {
        for stream in ["t", "nil", "'read-char"] {
            let expression = format!(r#"(let ((s ({reader} {stream})))
              (prin1 (list (multibyte-string-p s) (string-to-list s))))"#);
            check(&expression, b"\"\xff\"\n", b"Lisp expression: (nil (255))", false);
        }
    }
    check(r#"(let ((value (read-positioning-symbols t)))
        (prin1 (list value (symbol-with-pos-p value))))"#,
        b"qa-input-name\n", b"Lisp expression: (qa-input-name nil)", false);
}

#[test]
fn completion_overrides_run_before_batch_input_or_argument_validation() {
    check(r#"(progn
       (let ((completing-read-function (lambda (&rest arguments) arguments)))
         (prin1 (completing-read 17 'table)))
       (prin1 (read-string "R:")))"#,
       b"tail\n", b"(17 table nil nil nil nil nil nil)R:\"tail\"", false);
}

#[test]
fn empty_completion_selects_defaults_without_losing_the_empty_string() {
    check(r#"(dolist (default '(nil "x" ("x" "y") (nil "y") (17 "y") 17 symbol))
       (prin1 (completing-read "P:" nil nil nil nil nil default)) (terpri))"#,
       b"\n\n\n\n\n\n\n", b"P:\"\"\nP:\"x\"\nP:\"x\"\nP:nil\nP:17\nP:17\nP:symbol\n", false);
}

#[test]
fn multiple_completion_uses_the_real_separator_and_preserves_bytes() {
    check(r#"(prin1 (mapcar (lambda (s) (list (multibyte-string-p s) (string-to-list s)))
       (completing-read-multiple "P:" nil)))"#,
       b"\xff, \xfe\n", b"[comma-separated list] P:((nil (255)) (nil (254)))", false);
    check(r#"(progn (require 'crm)
       (let ((crm-separator "[:;]+") (crm-prompt "[CRM%s] %p"))
         (prin1 (completing-read-multiple "P:" nil))))"#,
       b"a;b::c\n", b"[CRM[:;]+] P:(\"a\" \"b\" \"c\")", false);
    check(r#"(prin1 (completing-read-multiple "P:" nil))"#,
       b"\n", b"[comma-separated list] P:nil", false);
}

#[test]
fn empty_input_returns_the_original_default_object() {
    check(r#"(let ((default (propertize "default" 'tag t)))
      (prin1 (eq (read-string "P:" nil nil default) default)))"#,
      b"\n", b"P:t", false);
}

#[test]
fn printing_raw_input_preserves_the_gnu_output_encoding() {
    check(r#"(princ (read-string ""))"#, b"\xff\n", b"\xc1\xbf", false);
}

#[test]
fn keyboard_events_wait_without_consuming_stdin() {
    for reader in ["(read-char)", "(read-event)", "(read-char-exclusive)"] {
        let expression = format!(r#"(progn
          (run-at-time 0.01 nil (lambda () (setq unread-command-events (list 90))))
          (prin1 (list {reader} (read-string "P:"))))"#);
        check(&expression, b"a\n", b"P:(90 \"a\")", false);
    }
}

#[test]
fn key_sequence_reads_prefixes_and_keeps_stdin_untouched() {
    for reader in [r#"(read-key-sequence "")"#, r#"(read-key-sequence-vector "")"#] {
        let expression = format!(r#"(progn
          (run-at-time 0.01 nil (lambda () (setq unread-command-events (list 24 19))))
          (let ((keys {reader})) (prin1 (list (append keys nil) (read-string "P:")))))"#);
        check(&expression, b"a\n", b"P:((24 19) \"a\")", false);
    }
}

#[test]
fn already_queued_prefix_events_are_read_as_one_sequence() {
    check(r#"(let ((unread-command-events (list 24 19)))
      (prin1 (append (read-key-sequence "") nil)))"#,
      b"", b"(24 19)", false);
}

#[test]
fn key_sequence_strings_pack_ascii_meta_events() {
    check(r#"(let ((unread-command-events (list (logior 97 #x8000000))))
      (let ((s (read-key-sequence ""))) (prin1 (list (multibyte-string-p s) (string-to-list s)))))"#,
      b"", b"(nil (225))", false);
}

#[test]
fn interactive_character_and_key_specs_read_events() {
    check(r#"(progn (defun qa-batch-c (c) (interactive "cChar: ") c)
      (run-at-time 0.01 nil (lambda () (setq unread-command-events (list 90))))
      (prin1 (list (call-interactively 'qa-batch-c) (read-string "P:"))))"#,
      b"a\n", b"P:(90 \"a\")", false);
    for spec in ["k", "K"] {
        let expression = format!(r#"(progn (defun qa-batch-k (k) (interactive "{spec}") k)
          (run-at-time 0.01 nil (lambda () (setq unread-command-events (list 97))))
          (let ((s (call-interactively 'qa-batch-k)))
            (prin1 (list (stringp s) (append s nil) (read-string "P:")))))"#);
        let expected: &[u8] = if spec == "K" { b"P:(nil (97) \"a\")" } else { b"P:(t (97) \"a\")" };
        check(&expression, b"a\n", expected, false);
    }
}

#[test]
fn interactive_event_specs_use_parameter_events_from_keys() {
    check(r#"(progn (defun qa-batch-e (a b) (interactive "e\ne") (list a b))
      (let* ((one (list 'mouse-1 'one)) (two (list 'mouse-2 'two))
             (result (call-interactively 'qa-batch-e nil (vector 97 one 98 two))))
        (prin1 (list (eq (car result) one) (eq (cadr result) two) (read-string "P:")))))"#,
      b"left\n", b"P:(t t \"left\")", false);
    check(r#"(progn (defun qa-batch-e (event) (interactive "e") event)
      (prin1 (condition-case err (call-interactively 'qa-batch-e) (error err)))
      (princ (read-string "P:")))"#,
      b"left\n", b"(error \"qa-batch-e must be bound to an event with parameters\")P:left", false);
}

#[test]
fn inhibited_interaction_does_not_consume_queued_events() {
    for reader in ["(read-char)", "(read-event)", "(read-char-exclusive)"] {
        let expression = format!(r#"(let ((inhibit-interaction t) (unread-command-events (list 97)))
          (prin1 (list (condition-case err {reader} (error err)) unread-command-events)))"#);
        check(&expression, b"left\n", b"((inhibited-interaction) (97))", false);
    }
}

#[test]
fn key_sequence_readers_do_not_apply_the_character_read_inhibition() {
    check(r#"(let ((inhibit-interaction t) (unread-command-events (list 97)))
      (prin1 (list (read-key-sequence "") unread-command-events)))"#,
      b"", b"(\"a\" nil)", false);
    check(r#"(let ((inhibit-interaction t) (unread-command-events (list 97)))
      (prin1 (list (read-key-sequence-vector "") unread-command-events)))"#,
      b"", b"([97] nil)", false);
}

#[test]
fn interaction_inhibition_has_the_forwarded_gnu_default() {
    check(r#"(prin1 (list (boundp 'inhibit-interaction) inhibit-interaction
      (condition-case err (makunbound 'inhibit-interaction) (error (car err)))))"#,
      b"", b"(t nil error)", false);
}

#[test]
fn source_boot_keeps_raw_input_and_keyboard_events_separate() {
    check(r#"(let* ((a (read-string "A:")) (b (read-string "B:")))
      (run-at-time 0.01 nil (lambda () (setq unread-command-events (list 90))))
      (prin1 (list (list (multibyte-string-p a) (string-to-list a))
                   (list (multibyte-string-p b) (string-to-list b))
                   (read-char) (read-string "C:"))))"#,
      b"\xff\n\xc3\xa9\na\n", b"A:B:C:((nil (255)) (t (233)) 90 \"a\")", true);
}
