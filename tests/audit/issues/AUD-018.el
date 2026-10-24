;;; -*- lexical-binding: t; -*-
;; "sql:missing-all"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-select db "select ?, ?") (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))

;; "sql:missing-one"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-select db "select ?, ?" '(7)) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
