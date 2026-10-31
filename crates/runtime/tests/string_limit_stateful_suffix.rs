use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'cl-lib)
  ;; Automatic composition needs a displayed buffer in GNU.  Use an explicit
  ;; composition below so this batch test can still verify whole glyphs.
  (let ((auto-composition-mode nil))
  (unless (equal (string-limit "語A" 5 t 'utf-7)
                 (encode-coding-string "A" 'utf-7))
    (error "Reverse UTF-7 truncation exceeded its byte limit"))
  (unless (equal (string-limit "A語" 5 t 'utf-7)
                 (encode-coding-string "A語" 'utf-7))
    (error "Reverse UTF-7 truncation discarded a suffix that fits"))
  ;; Enumerate all complete-glyph candidates independently of the binary
  ;; search.  Byte counts must be measured in the actual output order.
  (dolist (text (list "語A" "A語" "語A語B"
                      (let ((composed (copy-sequence "é")))
                        (compose-string composed 0 2)
                        composed)
                      ""))
    (let* ((glyphs (string-glyph-split text))
           (count (length glyphs)))
      (dolist (coding '(utf-7 utf-8 utf-16 iso-2022-jp))
        (dotimes (limit (+ 2 (length (encode-coding-string text coding))))
          (dolist (end '(nil t))
            (let ((expected ""))
              (dotimes (index count)
                (let* ((size (1+ index))
                       (candidate (encode-coding-string
                                   (string-join
                                    (if end (last glyphs size)
                                      (cl-subseq glyphs 0 size)))
                                   coding)))
                  (when (<= (length candidate) limit)
                    (setq expected candidate))))
              (let ((actual (string-limit text limit end coding)))
                (unless (and (<= (length actual) limit)
                             (equal actual expected))
                  (error "Truncation mismatch: %S %S %S %S => %S, expected %S"
                         text limit end coding actual expected)))))))))
  t))
"#;

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
fn string_limit_measures_stateful_suffixes_in_output_order() {
    let mut interp = Interp::new();
    evaluate(&mut interp, CASES);
    // Also exercise the source autoload, independently of the boot definition.
    evaluate(
        &mut interp,
        "(progn (fmakunbound 'string-limit) (load \"subr-x\") t)",
    );
    evaluate(&mut interp, CASES);
}
