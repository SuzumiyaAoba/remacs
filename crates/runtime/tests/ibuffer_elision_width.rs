use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
;;; -*- lexical-binding: t; -*-
(progn
  (require 'ibuffer)
  (require 'cl-lib)
  (require 'bytecomp)
  (defvar remacs-test-ibuffer-phase nil)
  (defvar remacs-test-ibuffer-width 6)
  (defun remacs-test-ibuffer-render (name spec marker)
    (setq remacs-test-ibuffer-phase (list 'format name spec marker))
    (let ((buffer (generate-new-buffer name))
          (ibuffer-eliding-string marker)
          (ibuffer-always-compile-formats t))
      (unwind-protect
          (let ((formatter (ibuffer-compile-format spec)))
            (with-temp-buffer
              (funcall formatter buffer nil)
              (buffer-string)))
        (kill-buffer buffer))))
  (dolist (direction '(1 -1))
    (dolist (case '((6 "12345678" "123456")
                    (6 "......" "......")
                    (6 "界界界界" "界界界")
                    (7 "界界界界" "界界界 ")
                    (6 "界abcde" "界abcd")
                    (7 "abcdef界" "abcdef ")
                    (8 "12345678" "12345678")))
      (let* ((width (nth 0 case)) (marker (nth 1 case))
             (expected (nth 2 case))
             (spec `((name ,(* direction width) ,width :left t)))
             (actual (remacs-test-ibuffer-render "abcdefghij" spec marker)))
        (setq remacs-test-ibuffer-phase (list 'bounded-marker direction case))
        (unless (and (equal actual expected) (= (string-width actual) width))
          (error "Overlong marker exceeded its column: %S expected %S"
                 actual expected))))
    (dolist (case '((1 "a" "j") (5 "abcde" "fghij")))
      (let* ((width (nth 0 case))
             (actual (remacs-test-ibuffer-render
                      "abcdefghij" `((name ,(* direction width) ,width :left t)) "12345678")))
        (unless (equal actual (nth (if (> direction 0) 1 2) case))
          (error "Short-column elision threshold changed: %S" actual))))
    (dolist (case '(("..." "abc..." "...hij")
                    ("" "abcdef" "efghij")))
      (let ((actual (remacs-test-ibuffer-render
                     "abcdefghij" `((name ,(* direction 6) 6 :left t)) (car case))))
        (unless (equal actual (nth (if (> direction 0) 1 2) case))
          (error "Normal/empty marker changed: %S" actual))))
    (let ((actual (remacs-test-ibuffer-render
                   "abcdefghij" `((name ,(* direction 6) 6 :left nil)) "12345678")))
      (unless (equal actual (if (> direction 0) "abcdef" "efghij"))
        (error "Disabled elision unexpectedly added a marker: %S" actual))))
  (unless (equal (remacs-test-ibuffer-render "abcdefghij" '((name 0 0 :left t)) "12345678") "")
    (error "Zero maximum stopped being authoritative"))
  (dolist (case '((:left "abc   ") (:center " abc  ") (:right "   abc")))
    (dolist (direction '(1 -1))
      (let ((actual (remacs-test-ibuffer-render
                     "abc" `((name ,(* direction 6) 6 ,(car case) t)) "12345678")))
        (unless (equal actual (cadr case))
          (error "Short names no longer use the requested alignment: %S" actual)))))
  (let ((actual (remacs-test-ibuffer-render
                 "aaaa你你你" '((name -8 8 :left t)) "...")))
    (unless (and (string-prefix-p "..." actual) (string-suffix-p "你你" actual)
                 (<= (string-width actual) 8))
      (error "Wide suffix regression: %S" actual)))
  (let* ((marker (propertize "12345678" 'remacs-marker t 'font-lock-face 'italic))
         (before (copy-sequence marker))
         (actual (remacs-test-ibuffer-render "abcdefghij" '((name -6 6 :left t)) marker)))
    (unless (and (equal actual "123456")
                 (eq (get-text-property 0 'font-lock-face actual) 'bold)
                 (get-text-property 0 'remacs-marker actual)
                 (equal-including-properties marker before))
      (error "Elision lost marker properties or mutated the customization")))
  (unless (equal (remacs-test-ibuffer-render
                  "abcdefghij" '((name 6 6 :left t) " | " (name -6 6 :right t)) "12345678")
                 "123456 | 123456")
    (error "One overlong marker displaced the next column"))
  ;; A single compiled formatter must re-bound the original marker on every call.
  (let ((buffer (generate-new-buffer "abcdefghij"))
        (ibuffer-eliding-string "12345678")
        (ibuffer-always-compile-formats t))
    (unwind-protect
        (let ((formatter (ibuffer-compile-format
                          '((name -1 remacs-test-ibuffer-width :left t)))))
          (dolist (case '((6 "123456") (9 "12345678j") (6 "123456")))
            (setq remacs-test-ibuffer-width (car case))
            (let ((actual (with-temp-buffer (funcall formatter buffer nil) (buffer-string))))
              (unless (equal actual (cadr case))
                (error "Dynamic column width or captured marker was stale: %S" actual)))))
      (kill-buffer buffer)))
  ;; Evaluate the generated helper form directly as interpreted Lisp as well.
  (dolist (from-end '(nil t))
    (let* ((ibuffer-eliding-string "12345678")
           (input (make-symbol "ibuffer-ellipsis"))
           (formatter (eval `(lambda (,input strlen)
                               ,(ibuffer-compile-make-eliding-form input t from-end)) t)))
      (unless (equal (funcall formatter "abcdef" 6) "123456")
        (error "Interpreted elision or generated-variable hygiene changed"))))
  t)
"##;

fn check(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => {
            let symbol = interp.intern("remacs-test-ibuffer-phase");
            let phase = interp.symbol_value(symbol);
            panic!(
                "{} {} phase={}",
                interp.prin1_to_string(&signal.symbol()),
                interp.prin1_to_string(&signal.data()),
                interp.prin1_to_string(&phase)
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn eliding_markers_respect_column_display_width() {
    let mut interp = Interp::new();
    eprintln!("default Ibuffer compiled/interpreted elision");
    check(&mut interp, CASES);
    check(&mut interp, r#"(load "ibuffer.el" nil t)"#);
    eprintln!("source Ibuffer compiled/interpreted elision");
    check(&mut interp, CASES);
}
