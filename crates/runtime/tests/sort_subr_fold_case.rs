use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r##"(progn
  (defun remacs-3135-sort-string-keys (text fold &optional predicate)
    (with-temp-buffer
      (insert text)
      (goto-char (point-min))
      (let ((sort-fold-case fold))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (buffer-substring-no-properties
                      (line-beginning-position) (line-end-position)))
                   nil predicate))
      (buffer-string)))
  (defun remacs-3135-sort-numeric-keys (text fold)
    (with-temp-buffer
      (insert text)
      (goto-char (point-min))
      (let ((sort-fold-case fold))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (string-to-number
                      (buffer-substring-no-properties
                       (line-beginning-position) (line-end-position)))))
        (buffer-string))))
  (defun remacs-3135-sort-symbol-keys (text fold)
    (with-temp-buffer
      (insert text)
      (goto-char (point-min))
      (let ((sort-fold-case fold))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (intern (buffer-substring-no-properties
                              (line-beginning-position) (line-end-position)))))
        (buffer-string))))
  (defun remacs-3135-sort-mixed-string-symbol-keys (text fold)
    (with-temp-buffer
      (insert text)
      (goto-char (point-min))
      (let ((sort-fold-case fold))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (let ((key (buffer-substring-no-properties
                                 (line-beginning-position)
                                 (line-end-position))))
                       (if (string= key "C") 'C key))))
        (buffer-string))))
  (defun remacs-3135-sort-position-keys (text fold)
    (with-temp-buffer
      (insert text)
      (goto-char (point-min))
      (let ((sort-fold-case fold))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (cons (line-beginning-position) (line-end-position))))
        (buffer-string))))
  (defun remacs-3135-sort-custom-case-table ()
    (with-temp-buffer
      (let ((table (copy-case-table (current-case-table))))
        (set-case-syntax-pair ?X ?y table)
        (set-case-table table))
      (insert "y\nX\n")
      (goto-char (point-min))
      (let ((sort-fold-case t))
        (sort-subr nil 'forward-line 'end-of-line
                   (lambda ()
                     (buffer-substring-no-properties
                      (line-beginning-position) (line-end-position))))
        (buffer-string))))
  (defun remacs-3135-sort-upcase-vs-canonical-table ()
    (with-temp-buffer
      (let ((table (copy-case-table (standard-case-table))))
        (set-case-table table)
        ;; Make string comparison treat # as X while leaving the case
        ;; table's canonicalize slot unchanged.
        (aset (char-table-extra-slot table 0) ?# ?X)
        (set-case-table table))
      (insert "X\n#\n")
      (goto-char (point-min))
      (let ((case-fold-search t)
            (sort-fold-case t))
        (let ((string-comparison (compare-strings "X" nil nil "#" nil nil t))
              (buffer-comparison
               (compare-buffer-substrings nil 1 2 nil 3 4)))
          (sort-subr nil 'forward-line 'end-of-line
                     (lambda ()
                       (buffer-substring-no-properties
                        (line-beginning-position) (line-end-position)))
                     nil nil)
          (list string-comparison (> buffer-comparison 0) (buffer-string))))))
  (list
   (equal (remacs-3135-sort-string-keys "a\nB\n" t) "a\nB\n")
   (equal (remacs-3135-sort-string-keys "a\nB\n" nil) "B\na\n")
   (equal (remacs-3135-sort-string-keys "A\nb\na\nB\n" t)
          "A\na\nb\nB\n")
   (equal (remacs-3135-sort-string-keys "a#\na!\n" t) "a!\na#\n")
   (equal (remacs-3135-sort-string-keys "a\nB\n" t #'string<)
          "B\na\n")
   (equal (remacs-3135-sort-symbol-keys "a\nB\n" t) "a\nB\n")
   (equal (remacs-3135-sort-symbol-keys "a\nB\n" nil) "B\na\n")
   (equal (remacs-3135-sort-mixed-string-symbol-keys "C\na\nB\n" t)
          "a\nB\nC\n")
   (equal (remacs-3135-sort-numeric-keys "10\n2\n" t) "2\n10\n")
   (equal (remacs-3135-sort-position-keys "a\nB\n" t) "a\nB\n")
   (equal (remacs-3135-sort-position-keys "a\nB\n" nil) "B\na\n")
   (equal (remacs-3135-sort-string-keys "ä\nÄ\n" t) "ä\nÄ\n")
   (equal (remacs-3135-sort-string-keys "ä\nÄ\n" nil) "Ä\nä\n")
   (equal (remacs-3135-sort-custom-case-table) "y\nX\n")
   (equal (remacs-3135-sort-upcase-vs-canonical-table)
          '(t t "X\n#\n"))))"##;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn sort_subr_string_keys_follow_sort_fold_case() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/sort.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t t t t t t t t t t t)");
}
