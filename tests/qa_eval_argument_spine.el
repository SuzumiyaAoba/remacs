;;; -*- lexical-binding: t; -*-
;; GNU snapshots the next argument cell before evaluating the current one.
(defvar qa-eval-spine-form nil)

(defun qa-eval-spine-replace-tail ()
  (setcdr (cdr qa-eval-spine-form) '((quote added)))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-replace-tail))))
      (eval qa-eval-spine-form))
    '(first))

(defun qa-eval-spine-change-next-car ()
  (setcar (cddr qa-eval-spine-form) '(quote changed))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-change-next-car) '(quote original))))
      (eval qa-eval-spine-form))
    '(first changed))

(defun qa-eval-spine-change-next-cdr ()
  (setcdr (cddr qa-eval-spine-form) '((quote added)))
  'first)

(qa (let ((qa-eval-spine-form
           (list 'list '(qa-eval-spine-change-next-cdr) '(quote original))))
      (eval qa-eval-spine-form))
    '(first original))

;; Circular argument spines signal before evaluating any argument forms.
(defvar qa-eval-spine-circular-side nil)
(defun qa-eval-spine-circular-function (&rest args) args)
(qa (let* ((args (list '(setq qa-eval-spine-circular-side t)))
           (form (cons 'qa-eval-spine-circular-function args)))
      (setcdr args args)
      (condition-case err (eval form) (error (car err))))
    'circular-list)
(qa qa-eval-spine-circular-side nil)

(princ (qa--done))
