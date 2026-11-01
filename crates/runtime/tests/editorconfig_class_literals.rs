use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'editorconfig-fnmatch)
  (defun remacs-editorconfig-3012-check (label expected name pattern)
    (let ((matched (not (null (editorconfig-fnmatch-p name pattern)))))
      (unless (eq matched expected)
        (error "EditorConfig #3012 %S: %S vs %S -> %S"
               label name pattern matched))))
  (let ((case-fold-search nil))
    (remacs-editorconfig-3012-check 'question-is-literal t "?" "[?]")
    (remacs-editorconfig-3012-check 'question-does-not-expand nil "x" "[?]")
    (remacs-editorconfig-3012-check 'question-class-is-one-character nil "??" "[?]")
    (remacs-editorconfig-3012-check 'star-is-literal t "*" "[*]")
    (remacs-editorconfig-3012-check 'star-does-not-expand nil "abc" "[*]")
    (remacs-editorconfig-3012-check 'negated-literal-star t "?" "[!*]")
    (remacs-editorconfig-3012-check 'negated-class-excludes-star nil "*" "[!*]")
    (remacs-editorconfig-3012-check 'spec-class-star-brace-content t "*" "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'spec-class-first-range-digit t "1" "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'spec-class-last-range-digit t "2" "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'spec-class-open-brace-is-literal t "{" "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'spec-class-dot-is-literal t "." "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'spec-class-final-brace-is-literal t "}" "[ab*c{1..2}]")
    (remacs-editorconfig-3012-check 'class-comma-is-literal t "," "[a,b]")
    (remacs-editorconfig-3012-check 'class-comma-is-not-alternative nil "a,b" "[a,b]")
    (remacs-editorconfig-3012-check 'class-comma-in-nested-alternative t "," "{[a,b],x}")
    (remacs-editorconfig-3012-check 'class-range-braces-in-alternative t "{" "{[ab*c{1..2}],x}")
    (remacs-editorconfig-3012-check 'class-open-brace-does-not-unbalance t "{" "{[a{],x}")
    (remacs-editorconfig-3012-check 'class-close-brace-does-not-unbalance t "}" "{[a}],x}")
    (remacs-editorconfig-3012-check 'negated-class-excludes-question nil "?" "[!a?*]")
    (remacs-editorconfig-3012-check 'negated-class-excludes-star-too nil "*" "[!a?*]")
    (remacs-editorconfig-3012-check 'class-open-bracket-member t "[" "[[]")
    (remacs-editorconfig-3012-check 'class-close-bracket-member t "]" "[]]")
    (remacs-editorconfig-3012-check 'class-close-bracket-member-negative nil "a" "[]]")
    (remacs-editorconfig-3012-check 'negated-leading-close-control t "a" "[!]]")
    (remacs-editorconfig-3012-check 'negated-leading-close-excludes-close nil "]" "[!]]")
    (remacs-editorconfig-3012-check 'escaped-close-class-member t "]" "[a\\]]")
    (remacs-editorconfig-3012-check 'escaped-close-class-negative nil "b" "[a\\]]")
    (remacs-editorconfig-3012-check 'escaped-open-does-not-hide-alternative t "[b" "\\[{a,b}")
    (remacs-editorconfig-3012-check 'class-hyphen-range-preserved t "b" "[a-c]")
    ;; Special regexp syntax characters remain members of the original ranges.
    (remacs-editorconfig-3012-check 'range-starting-hyphen t "?" "[--a]")
    (remacs-editorconfig-3012-check 'range-starting-hyphen-includes-bracket t "[" "[--a]")
    (remacs-editorconfig-3012-check 'range-starting-hyphen-bounded nil "b" "[--a]")
    (remacs-editorconfig-3012-check 'range-ending-caret t "Z" "[A-^]")
    (remacs-editorconfig-3012-check 'range-ending-caret-includes-backslash t
                                     (char-to-string 92) "[A-^]")
    (remacs-editorconfig-3012-check 'range-ending-caret-bounded nil "a" "[A-^]")
    (remacs-editorconfig-3012-check 'negative-hyphen-range-excludes nil "?" "[!--a]")
    (remacs-editorconfig-3012-check 'negative-hyphen-range-includes t "b" "[!--a]")
    (remacs-editorconfig-3012-check 'negative-caret-range-excludes nil "Z" "[!A-^]")
    (remacs-editorconfig-3012-check 'negative-caret-range-excludes-backslash nil
                                     (char-to-string 92) "[!A-^]")
    (remacs-editorconfig-3012-check 'negative-caret-range-includes t "a" "[!A-^]")
    (remacs-editorconfig-3012-check 'descending-range-is-empty nil "z" "[z-a]")
    (remacs-editorconfig-3012-check 'descending-negative-range-is-complement t "z" "[!z-a]")
    (remacs-editorconfig-3012-check 'descending-negative-range-includes-newline t "\n" "[!z-a]")
    (unless (equal (editorconfig-fnmatch-translate "[abc")
                   (concat "\\`" "[abc" "\\'"))
      (error "Unfinished positive class translation changed"))
    (unless (equal (editorconfig-fnmatch-translate "[!abc")
                   (concat "\\`" "[^abc" "\\'"))
      (error "Unfinished negated class translation changed"))
    (remacs-editorconfig-3012-check 'question-outside-class-remains-wildcard t "x" "?")
    (remacs-editorconfig-3012-check 'star-outside-class-remains-wildcard t "abc" "*"))
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
fn editorconfig_fnmatch_classes_treat_question_and_star_literally_3012() {
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
