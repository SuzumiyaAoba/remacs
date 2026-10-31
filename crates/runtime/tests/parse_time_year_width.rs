use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (require 'parse-time)
  (defvar remacs-3149-custom-months '(("foo" . 7)))
  (defvar remacs-3149-original-tokenizer nil)
  (defvar remacs-3149-copy-tokenizer-result nil)
  (defvar remacs-3149-tokenizer-calls 0)
  (list
   (list (parse-time-tokenize "001")
         (parse-time-tokenize "01"))
   (mapcar
    (lambda (date)
      (let ((parsed (parse-time-string date)))
        (list (nth 3 parsed) (nth 4 parsed) (nth 5 parsed))))
    '("1 Jan 001 00:00:00 +0000"
      "1 Jan 01 00:00:00 +0000"
      "1 Jan 050 00:00:00 +0000"
      "1 Jan 999 00:00:00 +0000"
      "1 Jan 0001 00:00:00 +0000"
      "001 Jan 2020 00:00:00 +0000"))
   (let ((parse-time-rules
          (cons '((4) remacs-3149-custom-months) parse-time-rules)))
     (let ((parsed (parse-time-string "1 Foo 001 00:00:00 +0000")))
       (list (nth 3 parsed) (nth 4 parsed) (nth 5 parsed))))
   ;; Advice around the public one-argument tokenizer still supplies the
   ;; token cells whose source widths the parser records.
   (let ((remacs-3149-copy-tokenizer-result nil)
         (remacs-3149-tokenizer-calls 0))
     (setq remacs-3149-original-tokenizer (symbol-function 'parse-time-tokenize))
     (unwind-protect
         (progn
           (fset 'parse-time-tokenize
                 (lambda (string)
                   (setq remacs-3149-tokenizer-calls
                         (1+ remacs-3149-tokenizer-calls))
                   (let ((tokens (funcall remacs-3149-original-tokenizer string)))
                     (if remacs-3149-copy-tokenizer-result
                         (copy-sequence tokens)
                       tokens))))
           (let ((parsed (parse-time-string "1 Jan 001 00:00:00 +0000")))
             (list remacs-3149-tokenizer-calls (nth 5 parsed))))
       (fset 'parse-time-tokenize remacs-3149-original-tokenizer)))
   ;; A custom tokenizer that returns copied tokens has no width metadata;
   ;; numeric-width fallback keeps its previous behavior.
   (let ((remacs-3149-copy-tokenizer-result t)
         (remacs-3149-tokenizer-calls 0))
     (setq remacs-3149-original-tokenizer (symbol-function 'parse-time-tokenize))
     (unwind-protect
         (progn
           (fset 'parse-time-tokenize
                 (lambda (string)
                   (setq remacs-3149-tokenizer-calls
                         (1+ remacs-3149-tokenizer-calls))
                   (let ((tokens (funcall remacs-3149-original-tokenizer string)))
                     (if remacs-3149-copy-tokenizer-result
                         (copy-sequence tokens)
                       tokens))))
           (let ((parsed (parse-time-string "1 Jan 001 00:00:00 +0000")))
             (list remacs-3149-tokenizer-calls (nth 5 parsed))))
       (fset 'parse-time-tokenize remacs-3149-original-tokenizer)))))"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn parse_time_year_rules_preserve_numeric_token_width() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/parse-time.el")
        .canonicalize()
        .unwrap();
    let mut interp = Interp::new();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(
        interp.prin1_to_string(&result),
        "(((1) (1)) ((1 1 1901) (1 1 2001) (1 1 1950) (1 1 2899) (1 1 1) (1 1 2020)) (1 7 1901) (1 1901) (1 2001))"
    );
}
