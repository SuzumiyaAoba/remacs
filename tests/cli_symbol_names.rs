use std::ffi::OsString;
use std::process::{Command, Stdio};

fn check(arguments: &[&str], expected: &str) {
    let mut binaries = vec![OsString::from(env!("CARGO_BIN_EXE_remacs"))];
    if let Some(gnu) = std::env::var_os("GNU_EMACS") { binaries.push(gnu); }
    for binary in binaries {
        let output = Command::new(&binary).args(["-Q", "--batch"]).args(arguments)
            .stdin(Stdio::null()).output().unwrap();
        assert!(output.status.success(), "{binary:?}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{binary:?}");
    }
}

#[test]
fn ascii_eval_arguments_create_unibyte_symbol_names() {
    check(&["--eval", "(prin1 (multibyte-string-p (symbol-name 'qa-cli-fresh-ascii)))"], "nil");
}

#[test]
fn live_eval_arguments_keep_their_string_kind() {
    check(&["--eval", r#"(setq command-line-args-left
        (list "--eval" (substring (concat (string #x100)
          "(prin1 (multibyte-string-p (symbol-name 'qa-cli-forced-multi)))") 1)))"#], "t");
    check(&["--eval", r#"(setq command-line-args-left
        (list (substring (concat (string #x100)
          "--eval=(prin1 (multibyte-string-p (symbol-name 'qa-cli-inline-multi)))") 1)))"#], "t");
    check(&["--eval", r#"(setq command-line-args-left
        (list "--eval" (concat "(let ((name (symbol-name 'qa-cli-" (unibyte-string 255)
          "))) (prin1 (list (multibyte-string-p name) (aref name (1- (length name))))))")))"#], "(nil 255)");
}

#[test]
fn funcall_uses_the_public_interning_protocol() {
    check(&["--eval", r#"(progn (defun qa-cli-target () (princ "SHORT"))
         (setq read-symbol-shorthands '(("s-" . "qa-cli-"))))"#,
        "--funcall", "s-target"], "SHORT");
    check(&["--eval", r#"(let ((table (obarray-make)))
         (fset (intern "qa-custom-command" table) (lambda () (princ "CUSTOM")))
         (setq obarray table))"#,
        "--funcall", "qa-custom-command"], "CUSTOM");
}

#[test]
fn uninterning_nil_and_t_keeps_the_original_objects_but_releases_the_names() {
    check(&["--eval", r#"(let (results)
      (dolist (old '(nil t))
        (let ((name (symbol-name old)) (table obarray))
          (unintern old table)
          (let ((fresh (intern name table)))
            (push (list (not (eq old fresh)) (boundp fresh) (eq (read name) fresh)) results))))
      (prin1 (nreverse results)))"#], "((t nil t) (t nil t))");
}

#[test]
fn gensym_keeps_its_compiled_counter_symbol_after_uninterning() {
    check(&["--eval", r#"(let ((old 'gensym-counter))
      (set old 3) (unintern old obarray)
      (let ((fresh (intern "gensym-counter")))
        (set fresh 100)
        (prin1 (list (symbol-name (gensym)) (symbol-value old) (symbol-value fresh)))))"#],
      "(\"g3\" 4 100)");
}
