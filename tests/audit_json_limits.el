;;; -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(audit-check "json:cyclic-vector" '(let ((v (vector nil))) (aset v 0 v) (condition-case e (json-serialize v) (error (car e)))))
(audit-check "json:cyclic-alist" '(let ((v (list (cons 'x 1)))) (setcdr v v) (condition-case e (json-serialize v) (error (car e)))))
(audit-check "json:cyclic-hash" '(let ((h (make-hash-table :test 'equal))) (puthash "x" h h) (condition-case e (json-serialize h) (error (car e)))))
(audit-check "json:parse-depth" '(condition-case e (progn (json-parse-string (concat (make-string 1200 ?\[) "0" (make-string 1200 ?\]))) 'parsed) (error (car e))))
(audit-check "json:serialize-depth" '(let ((v 0)) (dotimes (_ 1200) (setq v (vector v))) (condition-case e (progn (json-serialize v) 'serialized) (error (car e)))))
(audit-check "json:invalid-unicode" '(condition-case e (json-serialize (string #xd800)) (error (car e))))
(princ "AUDIT-DONE\n")
