use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r###"
(progn
  ;; The first mode scan must work before syntax helpers have been lazily loaded.
  (dolist (case '(("(defun f (x) `(a ,@x))\n" ",@" 6)
                  ("(defun f () '##)\n" "##" 3)
                  ("(defun f () ?\\N{LATIN CAPITAL LETTER A})\n"
                   "?\\N{LATIN CAPITAL LETTER A}" 3)
                  ("(defun f () #&8\"x\")\n" "&8" 6)
                  ("(defun f () #s(foo :x 1))\n" "s" 6)
                  ("(defun f () #^[nil nil])\n" "^" 6)
                  ("(defun f () #^^[nil])\n" "^^" 6)
                  ("(defun f () \",@ ## #s(foo)\")\n" nil nil)
                  ("; ,@ ## #s(foo)\n(defun f () t)\n" nil nil)))
    (with-temp-buffer
      (insert (car case))
      (emacs-lisp-mode)
      (check-parens)
      (let* ((text (car case))
             (token (nth 1 case))
             (start (and token (string-search token text)))
             (length (if token (length token) 0)))
        (dotimes (index (length text))
          (let* ((property (get-text-property (1+ index) 'syntax-table))
                 (actual (and property (car property)))
                 (expected (and start (<= start index) (< index (+ start length))
                                (nth 2 case))))
            (unless (equal actual expected)
              (error "Wrong Elisp syntax property at %s in %S: got %S expected %S"
                     index text actual expected)))))))
  t)
"###;

#[test]
fn elisp_syntax_rules_keep_their_original_match_ranges() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
