;;; audit_library_lifecycle_more.el --- fresh library and closure probes -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)

(audit-check "library:shortdoc-doc-direct"
 '(list (require 'shortdoc-doc) (featurep 'shortdoc)))

(audit-check "library:shortdoc-first-control"
 '(progn (require 'shortdoc) (list (require 'shortdoc-doc) (featurep 'shortdoc))))

(audit-check "closure:dotted-arglist"
 '(list
   (condition-case e
       (eval '(funcall (lambda (a . b) (list a b)) 1 2 3) t)
     (error e))
   (condition-case e
       (eval '(funcall (lambda (a b . c) (list a b c)) 1 2 3 4) t)
     (error e))))

(audit-check "closure:dotted-constructor-only"
 '(functionp (make-interpreted-closure '(a . b) '(a) nil)))

(princ "AUDIT-DONE\n")
