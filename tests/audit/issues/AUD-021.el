;;; -*- lexical-binding: t; -*-
;; "sql:forged"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (let ((fake (record 'sqlite 'db 999999))) (list (sqlitep fake) (sqlite-close fake))) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
