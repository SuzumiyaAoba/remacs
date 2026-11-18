;;; -*- lexical-binding: t; -*-

;; A macro function cell contains one `(macro . FUNCTION)' wrapper.  If its
;; function is itself a macro cell, GNU funcalls that object and reports
;; `invalid-function'; recursively unwrapping a circular cell must not loop.
(qa
 (let* ((cyclic (cons 'macro nil))
        (inner (cons 'macro #'(lambda (form) form)))
        (nested (cons 'macro inner)))
   (setcdr cyclic cyclic)
   (fset 'qa-cyclic-macro-function cyclic)
   (fset 'qa-nested-macro-function nested)
   (unwind-protect
       (list
        (condition-case err
            (macroexpand '(qa-cyclic-macro-function))
          (invalid-function (list (car err) (eq (cadr err) cyclic))))
        (condition-case err
            (eval '(qa-cyclic-macro-function))
          (invalid-function (list (car err) (eq (cadr err) cyclic))))
        (condition-case err
            (macroexpand '(qa-nested-macro-function))
          (invalid-function (list (car err) (eq (cadr err) inner))))
        (condition-case err
            (eval '(qa-nested-macro-function))
          (invalid-function (list (car err) (eq (cadr err) inner)))))
     ;; Break the Rc/Lisp cycle before dropping the local reference.
     (setcdr cyclic nil)
     (fmakunbound 'qa-cyclic-macro-function)
     (fmakunbound 'qa-nested-macro-function)))
 '((invalid-function t) (invalid-function t)
   (invalid-function t) (invalid-function t)))

(princ (qa--done))
