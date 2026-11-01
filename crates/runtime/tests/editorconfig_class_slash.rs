use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (defun remacs-editorconfig-3022-check (label expected name pattern)
    (let ((matched
           (not (null (editorconfig-core-handle--fnmatch-p name pattern)))))
      (unless (eq matched expected)
        (error "EditorConfig #3022 %S: %S vs %S -> %S"
               label name pattern matched))))
  (let ((case-fold-search nil))
    ;; A slash inside the class remains a class member and does not anchor
    ;; the section glob relative to the current directory.
    (remacs-editorconfig-3022-check
     'class-slash-matches-a t "afoo" "[a/]foo")
    (remacs-editorconfig-3022-check
     'class-slash-pattern-matches-subdirectory t "sub/afoo" "[a/]foo")
    (remacs-editorconfig-3022-check
     'class-slash-is-a-character-member t "/" "[a/]")
    (remacs-editorconfig-3022-check
     'escaped-close-bracket-class-then-path-slash t "]/foo" "[a\\]]/foo")
    (remacs-editorconfig-3022-check
     'escaped-close-bracket-class-keeps-members t "a/foo" "[a\\]]/foo")
    (remacs-editorconfig-3022-check
     'escaped-close-bracket-class-remains-anchored nil "other/]/foo" "[a\\]]/foo")
    ;; An escaped opening bracket is literal, so the following slash is still
    ;; outside the bracket expression and anchors the section pattern.
    (remacs-editorconfig-3022-check
     'escaped-open-bracket-followed-by-slash t "[a/b]" "\\[a/b]")
    (remacs-editorconfig-3022-check
     'escaped-open-bracket-followed-by-slash-anchors nil "sub/[a/b]" "\\[a/b]")
    (remacs-editorconfig-3022-check
     'escaped-slash-in-class-does-not-anchor t "sub/a" "[a\\/]")
    ;; A slash outside brackets still makes the pattern relative to this dir.
    (remacs-editorconfig-3022-check
     'outside-slash-same-path t "sub/foo" "sub/foo")
    (remacs-editorconfig-3022-check
     'outside-slash-remains-anchored nil "other/sub/foo" "sub/foo")
    ;; Keep existing escaped slash handling: it is still a path separator.
    (remacs-editorconfig-3022-check
     'escaped-slash-same-path t "foo/bar" "foo\\/bar")
    (remacs-editorconfig-3022-check
     'escaped-slash-remains-anchored nil "other/foo/bar" "foo\\/bar"))
  t)"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

fn load_source(interp: &mut Interp, relative: &str) {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join(relative);
    eval(
        interp,
        &format!("(load {} nil t t)", lisp_string(source.to_str().unwrap())),
    );
}

#[test]
fn editorconfig_class_slash_does_not_anchor_path_patterns_3022() {
    let mut interp = Interp::new();
    load_source(&mut interp, "lisp/editorconfig-fnmatch.el");
    load_source(&mut interp, "lisp/editorconfig-core-handle.el");
    assert_eq!(eval(&mut interp, CASES), "t");
}
