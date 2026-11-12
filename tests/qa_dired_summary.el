;;; -*- lexical-binding: t; -*-
;; dired-summary must report bindings from the active Dired keymap.
(require 'cl-lib)
(require 'dired)

(defun qa-dired-summary-reassigned-command ()
  (interactive))

(qa
 (let ((dired-mode-map (copy-keymap dired-mode-map))
       shown)
   (define-key dired-mode-map "d" #'qa-dired-summary-reassigned-command)
   (define-key dired-mode-map "z" #'dired-flag-file-deletion)
   (cl-letf (((symbol-function 'dired-why) (lambda () nil))
             ((symbol-function 'message)
              (lambda (format-string &rest args)
                (setq shown (apply #'format format-string args)))))
     (dired-summary))
   (list (lookup-key dired-mode-map "d")
         (string-match "z delete" shown)
         (string-match "\\_<d\\_> delete" shown)))
 '(qa-dired-summary-reassigned-command 0 nil))

(princ (qa--done))
