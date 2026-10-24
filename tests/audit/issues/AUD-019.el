;;; -*- lexical-binding: t; -*-
;; "sql:lazy-insert"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (progn (sqlite-execute db "create table t(a)") (sqlite-select db "insert into t values(7) returning a" nil 'set) (sqlite-select db "select * from t")) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
