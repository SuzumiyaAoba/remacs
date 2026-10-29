;;; -*- lexical-binding: t; -*-
;; `setq' evaluates a paired RHS before validating the variable symbol.
(defvar qa-setq-validation-side nil)

(defun qa-setq-validation-result (form)
  (let ((qa-setq-validation-side nil))
    (condition-case err
        (list 'ok (eval form) qa-setq-validation-side)
      (error (list (car err) (cdr err) qa-setq-validation-side)))))

(qa (qa-setq-validation-result
     '(setq 5 (setq qa-setq-validation-side 'rhs)))
    '(wrong-type-argument (symbolp 5) rhs))

;; Prior assignments still happen before a dangling final variable is found.
(qa (qa-setq-validation-result
     '(setq qa-setq-validation-side 'first 5))
    '(wrong-number-of-arguments (setq 3) first))

(princ (qa--done))
