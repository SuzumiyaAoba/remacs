;;; qa_elisp_completion_args.el --- Elisp optional-argument completion regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/elisp-mode.el"
       (file-name-directory load-file-name)))

(defun qa-elisp-local-variables-in (source)
  "Return local variable candidates while point is at SOURCE's end."
  (with-temp-buffer
    (emacs-lisp-mode)
    (insert source)
    (goto-char (point-max))
    (cl-letf (((symbol-function 'trusted-content-p)
               (lambda () t)))
      (elisp--local-variables))))

(qa
 (qa-elisp-local-variables-in
  "(lambda (&optional (local-var 1)) (list loc")
 '(local-var))

(qa
 (qa-elisp-local-variables-in
  "(lambda (&optional local-var) (list loc")
 '(local-var))

(qa
 (qa-elisp-local-variables-in
  "(lambda (local-var) (list loc")
 '(local-var))

(princ (qa--done))
