use remacs_runtime::lisp::{Flow, Interp, Value};
use std::path::Path;

const CASES: &str = r#"
(progn
  (require 'abbrev)
  (defun remacs-abbrev-table-enabled () t)
  (let* ((parent (make-abbrev-table))
         (_parent-abbrev (define-abbrev parent "parent" "parent value"))
         (regexp "\\([[:alpha:]]+\\)")
         (enable-function #'remacs-abbrev-table-enabled)
         (table (make-abbrev-table
                 (list :case-fixed t
                       :regexp regexp
                       :parents (list parent)
                       :enable-function enable-function))))
    (define-abbrev table "abc" "alphabet" nil :case-fixed t)
    (let* ((copy (copy-abbrev-table table))
           (copied-abbrev (obarray-get copy "abc")))
      (unless (and (abbrev-table-p copy)
                   (eq (abbrev-table-get copy :case-fixed) t)
                   (equal (abbrev-table-get copy :regexp) regexp)
                   (eq (car (abbrev-table-get copy :parents)) parent)
                   (eq (abbrev-table-get copy :enable-function) enable-function)
                   (equal (symbol-value copied-abbrev) "alphabet"))
        (error "copy-abbrev-table dropped table properties or definitions"))
      (unless (and (eq (abbrev-symbol "abc" copy) copied-abbrev)
                   (null (abbrev-symbol "ABC" copy))
                   (eq (abbrev-symbol "parent" copy)
                       (obarray-get parent "parent")))
        (error "Copied table changed case-fixed or parent lookup"))
      (when (abbrev-get copied-abbrev :case-fixed)
        (error "copy-abbrev-table copied per-abbrev properties"))
      (abbrev-table-put copy :regexp "copy-only")
      (unless (and (equal (abbrev-table-get copy :regexp) "copy-only")
                   (equal (abbrev-table-get table :regexp) regexp))
        (error "Copied table properties share their plist with the source"))))
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
fn copied_abbrev_tables_keep_table_properties_only() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_abbrev(&mut interp);
    eval(&mut interp, CASES);
}
