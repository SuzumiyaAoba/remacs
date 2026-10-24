;;; -*- lexical-binding: t; -*-
;; "sql:empty-select"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-select db "") (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
