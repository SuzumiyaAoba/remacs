use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (require 'editorconfig-fnmatch)
  (defun remacs-editorconfig-1685-check (label expected name pattern)
    (let ((matched (not (null (editorconfig-fnmatch-p name pattern)))))
      (unless (eq matched expected)
        (error "EditorConfig #1685 %S: %S vs %S -> %S"
               label name pattern matched))))
  (let ((case-fold-search nil)
        (backslash (char-to-string 92)))
    ;; Backslash quotes shell operators outside a class.
    (remacs-editorconfig-1685-check
     'escaped-star-literal t "foo*" (concat "foo" backslash "*"))
    (remacs-editorconfig-1685-check
     'escaped-star-does-not-wildcard nil "foobar" (concat "foo" backslash "*"))
    (remacs-editorconfig-1685-check
     'escaped-question-literal t "foo?" (concat "foo" backslash "?"))
    (remacs-editorconfig-1685-check
     'escaped-question-does-not-wildcard nil "foox" (concat "foo" backslash "?"))
    (remacs-editorconfig-1685-check
     'escaped-left-bracket-literal t "[" (concat backslash "["))
    (remacs-editorconfig-1685-check
     'escaped-left-bracket-does-not-open-class nil "a" (concat backslash "["))
    (remacs-editorconfig-1685-check
     'escaped-right-bracket-literal t "x]" (concat "x" backslash "]"))
    ;; An escaped `[' before an outer brace expression must not hide the
    ;; alternative's braces from the brace-balance scan.
    (remacs-editorconfig-1685-check
     'escaped-bracket-before-alternative-a t "[a" (concat backslash "[{a,b}"))
    (remacs-editorconfig-1685-check
     'escaped-bracket-before-alternative-b t "[b" (concat backslash "[{a,b}"))
    (remacs-editorconfig-1685-check
     'escaped-bracket-before-alternative-c nil "[c" (concat backslash "[{a,b}"))
    ;; Ordinary classes keep braces and commas as class members.
    (remacs-editorconfig-1685-check 'plain-class-open-brace t "{" "[a{]")
    (remacs-editorconfig-1685-check 'class-inside-brace-alternative t "a" "{[a],x}")
    ;; A literal opening bracket member and an escaped closing bracket member.
    (remacs-editorconfig-1685-check 'literal-open-bracket-member t "[" "[[]")
    (remacs-editorconfig-1685-check 'literal-open-bracket-member-negative nil "a" "[[]")
    (remacs-editorconfig-1685-check 'literal-close-bracket-member t "]" "[]]")
    (remacs-editorconfig-1685-check 'literal-close-bracket-member-negative nil "a" "[]]")
    (let ((close-class (concat "[a" backslash "]" "]")))
      (remacs-editorconfig-1685-check 'escaped-close-bracket-member t "]" close-class)
      (remacs-editorconfig-1685-check 'escaped-close-class-other-member t "a" close-class)
      (remacs-editorconfig-1685-check 'escaped-close-class-negative nil "b" close-class))
    (remacs-editorconfig-1685-check 'negated-leading-close-control t "a" "[!]]")
    (remacs-editorconfig-1685-check 'negated-leading-close-excludes-close nil "]" "[!]]")
    ;; Negative-class quoting must exclude the special member without
    ;; accidentally matching it due to Emacs regexp bracket syntax.
    (let ((not-close (concat "[!" backslash "]" "]"))
          (not-caret (concat "[!" backslash "^" "]"))
          (not-backslash (concat "[!" backslash backslash "]")))
      (remacs-editorconfig-1685-check 'negative-close-control t "a" not-close)
      (remacs-editorconfig-1685-check 'negative-close-excludes-close nil "]" not-close)
      (remacs-editorconfig-1685-check 'negative-caret-control t "a" not-caret)
      (remacs-editorconfig-1685-check 'negative-caret-excludes-caret nil "^" not-caret)
      (remacs-editorconfig-1685-check 'negative-backslash-control t "a" not-backslash)
      (remacs-editorconfig-1685-check 'negative-backslash-excludes-backslash nil backslash not-backslash))
    ;; Escaped hyphen is a member; an unescaped range keeps its prior meaning.
    (let ((escaped-hyphen (concat "[a" backslash "-c]")))
      (remacs-editorconfig-1685-check 'escaped-hyphen-member t "-" escaped-hyphen)
      (remacs-editorconfig-1685-check 'escaped-hyphen-not-range nil "b" escaped-hyphen))
    ;; Escaping an ordinary endpoint does not disable an unescaped range
    ;; operator between its endpoints.
    (let ((escaped-left-endpoint (concat "[" backslash "a-c]"))
          (escaped-right-endpoint (concat "[a-" backslash "c]")))
      (remacs-editorconfig-1685-check
       'escaped-left-range-endpoint t "b" escaped-left-endpoint)
      (remacs-editorconfig-1685-check
       'escaped-right-range-endpoint t "b" escaped-right-endpoint))
    (let ((special-endpoint (concat "[" backslash "[" "-c]")))
      (remacs-editorconfig-1685-check
       'special-range-endpoint-is-literal t "[" special-endpoint)
      (remacs-editorconfig-1685-check
       'special-range-hyphen-not-member nil "-" special-endpoint)
      (remacs-editorconfig-1685-check
       'special-range-keeps-interval t "b" special-endpoint))
    ;; Ranges that cross Emacs regexp bracket syntax characters must retain
    ;; the complete character interval without expanding it character by
    ;; character in the generated regexp.
    (let ((open-to-a "[[-a]")
          (close-to-a "[]-a]")
          (backslash-to-a (concat "[" backslash backslash "-a]"))
          (not-backslash-to-a
           (concat "[!" backslash backslash "-a]")))
      (dotimes (offset 7)
        (remacs-editorconfig-1685-check
         'open-to-a-range t (char-to-string (+ 91 offset)) open-to-a))
      (remacs-editorconfig-1685-check 'open-to-a-range-before nil "Z" open-to-a)
      (remacs-editorconfig-1685-check 'open-to-a-range-after nil "b" open-to-a)
      (dotimes (offset 5)
        (remacs-editorconfig-1685-check
         'close-to-a-range t (char-to-string (+ 93 offset)) close-to-a))
      (dotimes (offset 6)
        (let ((name (char-to-string (+ 92 offset))))
          (remacs-editorconfig-1685-check
           'backslash-to-a-range t name backslash-to-a)
          (remacs-editorconfig-1685-check
           'negative-backslash-to-a-range nil name not-backslash-to-a)))
      (remacs-editorconfig-1685-check
       'negative-backslash-to-a-range-before t "[" not-backslash-to-a)
      (remacs-editorconfig-1685-check
       'negative-backslash-to-a-range-after t "b" not-backslash-to-a))
    (remacs-editorconfig-1685-check 'existing-hyphen-range t "b" "[a-c]"))
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
fn editorconfig_fnmatch_honors_backslash_glob_escapes_1685() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/editorconfig-fnmatch.el");
    let mut interp = Interp::new();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    assert_eq!(eval(&mut interp, &code), "t");
}
