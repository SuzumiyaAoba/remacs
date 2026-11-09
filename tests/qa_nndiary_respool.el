;;; qa_nndiary_respool.el --- Regression tests for nndiary respooling -*- lexical-binding:t -*-

(require 'nndiary)

(defvar qa-nndiary-respool-observed-split-methods nil)

(defun qa-nndiary-respool-noop (&rest _args)
  t)

(defun qa-nndiary-respool-scheduled-p ()
  t)

(defun qa-nndiary-respool-split-article (&rest _args)
  (setq qa-nndiary-respool-observed-split-methods
        (copy-tree nnmail-split-methods))
  '(("diary" . 7)))

(defun qa-nndiary-respool-save-mail (&rest _args)
  '(saved))

(qa
 (let* ((nndiary-split-methods '(("diary" "subject-pattern")))
        (nnmail-split-methods '(("mail.default" "")))
        (nnmail-cache-accepted-message-ids nil)
        (nndiary-request-accept-article-functions nil)
        (qa-nndiary-respool-observed-split-methods nil)
        (old-change-directory
         (symbol-function 'nndiary-possibly-change-directory))
        (old-check-syntax (symbol-function 'nnmail-check-syntax))
        (old-schedule (symbol-function 'nndiary-schedule))
        (old-activate (symbol-function 'nnmail-activate))
        (old-article-group (symbol-function 'nnmail-article-group))
        (old-save-mail (symbol-function 'nndiary-save-mail))
        result)
   (unwind-protect
       (progn
         (fset 'nndiary-possibly-change-directory
               #'qa-nndiary-respool-noop)
         (fset 'nnmail-check-syntax #'qa-nndiary-respool-noop)
         (fset 'nndiary-schedule #'qa-nndiary-respool-scheduled-p)
         (fset 'nnmail-activate #'qa-nndiary-respool-noop)
         (fset 'nnmail-article-group #'qa-nndiary-respool-split-article)
         (fset 'nndiary-save-mail #'qa-nndiary-respool-save-mail)
         (setq result (nndiary-request-accept-article nil))
         (list result qa-nndiary-respool-observed-split-methods))
     (fset 'nndiary-possibly-change-directory old-change-directory)
     (fset 'nnmail-check-syntax old-check-syntax)
     (fset 'nndiary-schedule old-schedule)
     (fset 'nnmail-activate old-activate)
     (fset 'nnmail-article-group old-article-group)
     (fset 'nndiary-save-mail old-save-mail)))
 '(saved (("diary" "subject-pattern"))))

(princ (qa--done))
