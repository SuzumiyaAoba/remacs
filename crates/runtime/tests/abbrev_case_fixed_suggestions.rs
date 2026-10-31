use remacs_runtime::lisp::{Flow, Interp, Value};
use std::path::Path;

const CASES: &str = r#"
(progn
  (require 'abbrev)
  (defun remacs-abbrev-suggest-case (text table-case-fixed abbrev-case-fixed)
    (let ((abbrev-suggest-hint-threshold 0)
          (abbrev--suggest-saved-recommendations nil))
      (with-temp-buffer
        (let ((table (make-abbrev-table
                      (and table-case-fixed '(:case-fixed t)))))
          (setq-local local-abbrev-table table)
          (define-abbrev table "hw" "hello world" nil
            :case-fixed abbrev-case-fixed)
          (insert text)
          (abbrev--suggest-maybe-suggest)
          (length abbrev--suggest-saved-recommendations)))))
  (unless (= (remacs-abbrev-suggest-case "HELLO world" t nil) 0)
    (error "A case-fixed table suggested a differently capitalized expansion"))
  (unless (= (remacs-abbrev-suggest-case "HELLO world" nil t) 0)
    (error "A case-fixed abbrev suggested a differently capitalized expansion"))
  (unless (= (remacs-abbrev-suggest-case "hello world" t nil) 1)
    (error "A case-fixed table rejected an exact expansion"))
  (unless (= (remacs-abbrev-suggest-case "HELLO world" nil nil) 1)
    (error "A case-flexible abbrev stopped ignoring case"))
  t)
"#;

fn eval(interp: &mut Interp, source: &str) {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
}

fn load_source_abbrev(interp: &mut Interp) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/abbrev.el")
        .canonicalize()
        .unwrap();
    let path = interp.prin1_to_string(&Value::string(path.to_string_lossy().into_owned()));
    eval(interp, &format!("(load {path} nil t)"));
}

#[test]
fn case_fixed_abbrevs_do_not_suggest_different_capitalization() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_abbrev(&mut interp);
    eval(&mut interp, CASES);
}
