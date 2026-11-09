;;; qa_ediff_undo.el --- Regression tests for Ediff undo tracking -*- lexical-binding:t -*-

(require 'ediff)

(defvar qa-ediff-undo-update-count 0)

(defun qa-ediff-undo-count-update ()
  (setq qa-ediff-undo-update-count (1+ qa-ediff-undo-update-count)))

(qa
 (let ((buffer-a (generate-new-buffer " *qa-ediff-undo-a*"))
       (control (generate-new-buffer " *qa-ediff-undo-control*"))
       (original-update (symbol-function 'ediff-update-diffs))
       (qa-ediff-undo-update-count 0)
       result)
   (unwind-protect
       (progn
         (with-current-buffer control
           (setq major-mode 'ediff-mode))
         (fset 'ediff-update-diffs #'qa-ediff-undo-count-update)
         (with-current-buffer buffer-a
           (setq buffer-undo-list t)
           (insert "left")
           (setq buffer-undo-list nil)
           (setq-local ediff-this-buffer-ediff-sessions (list control))
           (ediff--install-undo-diff-hooks buffer-a)
           (delete-region (point-min) (point-max))
           (undo-boundary)
           (undo 1)
           (let ((noticed ediff--undo-diffs-changed)
                 (hook-installed
                  (and (memq #'ediff--recompute-diffs-after-undo
                             post-command-hook)
                       t)))
             (run-hooks 'post-command-hook)
             (setq result
                   (list (buffer-string)
                         hook-installed
                         noticed
                         qa-ediff-undo-update-count
                         ediff--undo-diffs-changed))))
         result)
     (fset 'ediff-update-diffs original-update)
     (when (buffer-live-p buffer-a)
       (with-current-buffer buffer-a
         (setq ediff-this-buffer-ediff-sessions nil)
         (ediff--remove-undo-diff-hooks-if-unused))
       (kill-buffer buffer-a))
     (when (buffer-live-p control) (kill-buffer control))))
 '("left" t t 1 nil))

(princ (qa--done))
