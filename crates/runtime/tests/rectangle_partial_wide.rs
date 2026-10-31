use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (require 'rect)
  (dolist (delete '(nil t))
    (dolist (case '(("界X" 0 1 " ")
                    ("界X" 0 2 "界")
                    ("界X" 1 2 " ")
                    ("界X" 1 3 " X")
                    ("a界X" 0 2 "a ")
                    ("a界X" 1 3 "界")
                    ("ab界cd" 1 3 "b ")
                    ("ab界cd" 3 5 " c")
                    ("界界X" 1 3 "  ")
                    ("abc" 4 6 "  ")
                    ("界X" 1 1 "")))
      (with-temp-buffer
        (setq tab-width 8)
        (insert (car case))
        (goto-char (point-min))
        (let ((lines (list nil)))
          (if delete
              (delete-extract-rectangle-line (nth 1 case) (nth 2 case) lines nil)
            (extract-rectangle-line (nth 1 case) (nth 2 case) lines))
          (unless (equal (cdr lines) (list (nth 3 case)))
            (error "Partial wide slice delete=%S case=%S got=%S"
                   delete case (cdr lines)))
          (unless (= (string-width (cadr lines)) (- (nth 2 case) (nth 1 case)))
            (error "The extracted row lost display columns"))))))
  (let ((char-width-table (copy-sequence char-width-table)))
    (set-char-table-range char-width-table ?界 4)
    (dolist (delete '(nil t))
      (with-temp-buffer
        (insert "界X")
        (goto-char (point-min))
        (unless (and (= (move-to-column 1) 4)
                     (= (point) 2)
                     (= (current-column) 4))
          (error "Column movement ignored the live character width table"))
        (goto-char (point-min))
        (let ((lines (list nil)))
          (if delete
              (delete-extract-rectangle-line 0 1 lines nil)
            (extract-rectangle-line 0 1 lines))
          (unless (equal (cdr lines) '(" "))
            (error "A width-four glyph lost its selected display column"))))))
  (dolist (delete '(nil t))
    (with-temp-buffer
      (insert "界X")
      (rectangle--col-pos 0 'point)
      (let ((start (point)) end)
        (rectangle--col-pos 1 'mark)
        (setq end (point))
        (unless (equal (rectangle--pos-cols start end) '(0 . 1))
          (error "Rectangle position setup lost its columns"))
        (if delete
            (progn
              (kill-rectangle start end)
              (unless (and (equal killed-rectangle '(" "))
                           (equal (buffer-string) "X"))
                (error "kill-rectangle lost the selected display column")))
          (unless (and (equal (extract-rectangle start end) '(" "))
                       (equal (buffer-string) "界X"))
            (error "extract-rectangle lost the selected display column"))))))
  (with-temp-buffer
    (insert (propertize "a界X" 'rectangle-test-property t))
    (goto-char (point-min))
    (let ((lines (list nil))
          (buffer-read-only t))
      (extract-rectangle-line 0 2 lines)
      (unless (and (equal (cadr lines) "a ")
                   (get-text-property 0 'rectangle-test-property (cadr lines))
                   (equal (buffer-string) "a界X"))
        (error "Partial extraction lost retained text properties"))))
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
fn rectangle_extraction_preserves_partial_wide_columns() {
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
