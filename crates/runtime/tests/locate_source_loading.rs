use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-2987-positions nil)
  (unless (and (fboundp 'locate-get-file-positions)
               (fboundp 'locate-current-line-number)
               (fboundp 'locate-get-filename))
    (error "locate definitions after locate-get-file-positions were not loaded"))
  (with-temp-buffer
    (insert "    /tmp/remacs-locate-entry   \n")
    (goto-char (point-min))
    (setq remacs-2987-positions (locate-get-file-positions))
    (unless (equal (buffer-substring-no-properties
                    (car remacs-2987-positions)
                    (cadr remacs-2987-positions))
                   "/tmp/remacs-locate-entry")
      (error "locate-get-file-positions returned %S" remacs-2987-positions)))
  t)"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn locate_source_loads_and_file_positions_are_usable() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/locate.el")
        .canonicalize()
        .unwrap();
    let code = format!("(progn (load {} nil t t) {})", lisp_string(source.to_str().unwrap()), CASES);
    let mut interp = Interp::new();
    match interp.eval_str(&code) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
