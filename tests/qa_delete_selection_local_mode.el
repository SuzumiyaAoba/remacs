;;; qa_delete_selection_local_mode.el --- Local delete-selection hook -*- lexical-binding: t -*-

;; Regression for #2271.  The buffer-local mode hook runs after both transitions.
(qa (with-temp-buffer
       (let (states)
         (setq-local delete-selection-mode nil)
         (add-hook 'delete-selection-local-mode-hook
                   (lambda () (push delete-selection-mode states)) nil t)
         (delete-selection-local-mode 1)
         (delete-selection-local-mode -1)
         (list (length states) (nreverse states))))
    '(2 (t nil)))

(princ (qa--done))
