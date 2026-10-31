use remacs_runtime::lisp::{Flow, Interp, Value};
use std::path::Path;

const CASES: &str = r#"
(progn
  (require 'abbrev)
  (defun remacs-abbrev-enable-a () t)
  (defun remacs-abbrev-enable-b () nil)
  (let ((table (make-abbrev-table)))
    (define-abbrev table "abc" "alphabet" nil
      :case-fixed nil :enable-function #'remacs-abbrev-enable-a)
    (setq abbrevs-changed nil)
    (define-abbrev table "abc" "alphabet" nil
      :case-fixed t :enable-function #'remacs-abbrev-enable-a)
    (unless abbrevs-changed
      (error "Changing only :case-fixed did not mark abbrevs changed"))
    (setq abbrevs-changed nil)
    (define-abbrev table "abc" "alphabet" nil
      :case-fixed t :enable-function #'remacs-abbrev-enable-b)
    (unless abbrevs-changed
      (error "Changing only :enable-function did not mark abbrevs changed"))
    (setq abbrevs-changed nil)
    (define-abbrev table "abc" "alphabet" nil
      :case-fixed t :enable-function #'remacs-abbrev-enable-b)
    (when abbrevs-changed
      (error "Redefining an unchanged abbrev marked it changed"))
    (setq abbrevs-changed nil)
    (define-abbrev table "abc" "alphabet" nil)
    (unless abbrevs-changed
      (error "Removing saved abbrev properties was not detected")))
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
fn changing_saved_abbrev_properties_marks_table_dirty() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_abbrev(&mut interp);
    eval(&mut interp, CASES);
}
