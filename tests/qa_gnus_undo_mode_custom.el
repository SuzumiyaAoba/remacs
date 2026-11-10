;;; qa_gnus_undo_mode_custom.el --- Gnus undo mode customization regression -*- lexical-binding:t -*-

(require 'gnus-undo)

(qa (get 'gnus-undo-mode 'custom-type) nil)

(qa
 (with-temp-buffer
   (gnus-undo-mode 1)
   (list gnus-undo-mode
         (local-variable-p 'gnus-undo-actions)
         (local-variable-p 'gnus-undo-boundary)
         (local-variable-p 'post-command-hook)
         (and (memq 'gnus-undo-boundary post-command-hook) t)))
 '(t t t t t))

(princ (qa--done))
