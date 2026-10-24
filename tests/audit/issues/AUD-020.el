;;; -*- lexical-binding: t; -*-
;; "sql:select-error"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-select db "select * from nonexistent") (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))

;; "sql:constraint-error"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (progn (sqlite-execute db "create table t(x unique)") (sqlite-execute db "insert into t values(1)") (sqlite-execute db "insert into t values(1)")) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))

;; "sql:extra-one"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-select db "select ?" '(7 8)) (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))

;; "sql:empty"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((db (sqlite-open))) (unwind-protect (sqlite-execute db "") (sqlite-close db))) t))
           (error (list 'error err))))
  (terpri))
