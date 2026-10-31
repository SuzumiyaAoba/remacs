use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'format)
  (let* ((replacement (concat (propertize "β" 'face 'bold 'explicit nil)
                              (propertize "日" 'help-echo "hint") "x"))
         (saved (copy-sequence replacement)))
    (with-temp-buffer
      (insert (propertize "foo" 'source t 'face 'italic 'explicit t))
      (format-replace-strings (list (cons "foo" replacement)))
      (unless (and (equal (buffer-substring-no-properties (point-min) (point-max)) "β日x")
                   (equal (mapcar (lambda (pos) (get-text-property pos 'face)) '(1 2 3))
                          '(bold italic italic))
                   (equal (mapcar (lambda (pos) (get-text-property pos 'source)) '(1 2 3))
                          '(t t t))
                   (equal (mapcar (lambda (pos) (get-text-property pos 'explicit)) '(1 2 3))
                          '(nil t t))
                   (equal (mapcar (lambda (pos) (get-text-property pos 'help-echo)) '(1 2 3))
                          '(nil "hint" nil)))
        (error "Replacement properties did not override inherited defaults: %S"
               (buffer-string))))
    (unless (equal-including-properties replacement saved)
      (error "Replacement string was mutated")))
  (with-temp-buffer
    (insert "foo foo foo")
    (goto-char 2)
    (narrow-to-region 1 8)
    (format-replace-strings (list (cons (propertize "Z" 'face 'bold) "foo")) t 5 8)
    (unless (and (= (point) 2) (= (point-min) 1) (= (point-max) 6)
                 (equal (buffer-string) "foo Z")
                 (eq (get-text-property 5 'face) 'bold))
      (error "Reverse replacement lost properties, bounds or point"))
    (widen)
    (unless (and (equal (buffer-string) "foo Z foo")
                 (null (get-text-property 7 'face)))
      (error "Replacement changed text outside its region")))
  (with-temp-buffer
    (insert "FOO foo")
    (let ((case-fold-search t))
      (format-replace-strings '(("foo" . "bar\\1\\&"))))
    (unless (equal (buffer-string) "bar\\1\\& bar\\1\\&")
      (error "Literal replacement or case was changed")))
  (with-temp-buffer
    (insert (propertize "foo foo" 'source t))
    (format-replace-strings (list (cons "foo" (propertize "bar" 'face 'bold))
                                 (cons "bar" (propertize "Q" 'help-echo "next"))))
    (unless (and (equal (buffer-string) "Q Q")
                 (eq (get-text-property 1 'face) 'bold)
                 (eq (get-text-property 3 'face) 'bold)
                 (eq (get-text-property 1 'source) t)
                 (equal (get-text-property 3 'help-echo) "next"))
      (error "Multiple replacement passes lost properties"))
    (format-replace-strings '(("Q" . "")))
    (unless (equal (buffer-string) " ")
      (error "Empty replacement no longer deletes text")))
  (with-temp-buffer
    (insert "unchanged")
    (unless (and (equal (condition-case err
                           (format-replace-strings '(("" . "x")))
                         (error err))
                       '(error "Empty FROM string in format-replace-strings"))
                 (equal (buffer-string) "unchanged"))
      (error "Empty search string was not rejected before mutation")))
  t)
"#;

#[test]
fn format_replacements_preserve_explicit_and_inherited_properties() {
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
