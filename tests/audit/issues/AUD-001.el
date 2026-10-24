;;; -*- lexical-binding: t; -*-
;; "sql:blob-read"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (let ((s (caar (sqlite-select db "select x'0080ff'")))) (list (multibyte-string-p s) (string-to-list s))) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
