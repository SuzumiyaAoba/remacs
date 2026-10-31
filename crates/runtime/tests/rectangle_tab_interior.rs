use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (require 'rect)
  (dolist (case '(("\tX" 2 4 "  ")
                  ("\tX" 4 8 "    ")
                  ("\tX" 2 2 "")
                  ("a\tX" 2 4 "  ")
                  ("abc" 1 3 "bc")))
    (with-temp-buffer
      (setq tab-width 8)
      (insert (car case))
      (goto-char (point-min))
      (let ((lines (list nil)))
        (extract-rectangle-line (nth 1 case) (nth 2 case) lines)
        (unless (equal (cdr lines) (list (nth 3 case)))
          (error "Rectangle slice %S was dropped or changed: %S" case (cdr lines)))
        (unless (equal (buffer-string) (car case))
          (error "Extraction changed its source buffer")))))
  (with-temp-buffer
    (setq tab-width 8)
    (insert "aa\n\tX\nbbbb")
    (let ((start (+ (point-min) 2))
          (end (point-max)))
      (unless (equal (extract-rectangle start end) '("  " "  " "bb"))
        (error "A tab-interior row disappeared from extract-rectangle"))))
  t)"#;

fn evaluate(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn rectangle_extraction_keeps_tab_interior_rows() {
    let mut interp = Interp::new();
    evaluate(&mut interp, CASES);
    let compiled = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp-elc/rect.elc");
    let compiled = compiled
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    evaluate(&mut interp, &format!("(progn (load \"{compiled}\" nil t t) t)"));
    evaluate(&mut interp, CASES);
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/rect.el");
    let filename = source
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    evaluate(&mut interp, &format!("(progn (load \"{filename}\" nil t t) t)"));
    evaluate(&mut interp, CASES);
}
