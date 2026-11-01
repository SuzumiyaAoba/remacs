use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'editorconfig-fnmatch)
  (defun remacs-editorconfig-3010-check (label expected name pattern)
    (let ((matched (not (null (editorconfig-fnmatch-p name pattern)))))
      (unless (eq matched expected)
        (error "EditorConfig #3010 %S: %S vs %S -> %S"
               label name pattern matched))))
  (let ((case-fold-search nil))
    ;; A brace expression is a numeric range only when its entire body is the
    ;; signed-number '..' signed-number form.
    (remacs-editorconfig-3010-check
     'embedded-range-body-is-literal t "{foo1..3bar}" "{foo1..3bar}")
    (remacs-editorconfig-3010-check
     'embedded-range-body-does-not-expand nil "1" "{foo1..3bar}")
    (remacs-editorconfig-3010-check
     'embedded-range-body-does-not-match-interior nil "2" "{foo1..3bar}")
    (remacs-editorconfig-3010-check
     'embedded-literal-with-prefix-suffix t "pre{foo1..3bar}post"
     "pre{foo1..3bar}post")
    (remacs-editorconfig-3010-check
     'true-range-still-expands t "2" "{1..3}")
    (remacs-editorconfig-3010-check
     'true-range-still-bounds-values nil "4" "{1..3}")
    ;; Ranges inside a larger glob sequence and existing nested alternatives
    ;; remain composable.
    (remacs-editorconfig-3010-check
     'embedded-range-expression t "foo2bar" "foo{1..3}bar")
    (remacs-editorconfig-3010-check
     'nested-alternative-range t "file+2.txt" "{plain,file{1..3}.txt}")
    (remacs-editorconfig-3010-check
     'string-alternative-unchanged t "b" "{a,b}"))
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

#[test]
fn editorconfig_fnmatch_braces_only_expand_whole_numeric_ranges_3010() {
    let path = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/editorconfig-fnmatch.el");
    let mut interp = Interp::new();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(path.to_str().unwrap()),
        CASES
    );
    assert_eq!(eval(&mut interp, &code), "t");
}
