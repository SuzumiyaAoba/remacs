;;; Do not start smerge from stale Bazaar sidecars alone.

(require 'cl-lib)

;; Load the hook form from the source, without loading VC's command backend.
(qa (let ((source (expand-file-name "lisp/vc-bzr.el" default-directory)))
      (with-temp-buffer
        (set-syntax-table emacs-lisp-mode-syntax-table)
        (insert-file-contents source)
        (goto-char (point-min))
        (search-forward "(defun vc-bzr-find-file-hook")
        (goto-char (match-beginning 0))
        (eval (read (current-buffer)) t))
      (functionp (symbol-function 'vc-bzr-find-file-hook)))
    t)

(qa (let* ((file (make-temp-file "qa-vc-bzr-conflict-"))
           (status-conflict nil)
           (started nil)
           (result nil))
      (unwind-protect
          (progn
            (with-temp-file file (insert "<<<<<<< conflict\n"))
            (dolist (suffix '(".BASE" ".OTHER" ".THIS"))
              (with-temp-file (concat file suffix) (insert "sidecar")))
            (with-temp-buffer
              (insert-file-contents file)
              (setq buffer-file-name file)
              (let ((vc-bzr-resolve-conflicts nil))
                (cl-letf (((symbol-function 'vc-bzr-conflicted-files)
                           (lambda (name)
                             (when status-conflict
                               (list (expand-file-name name)))))
                          ((symbol-function 'smerge-start-session)
                           (lambda () (setq started t)))
                          ((symbol-function 'vc-message-unresolved-conflicts)
                           (lambda (_file) nil)))
                  (vc-bzr-find-file-hook)
                  (setq result started)
                  (setq status-conflict t)
                  (vc-bzr-find-file-hook)
                  (list result started)))))
        (delete-file file)
        (dolist (suffix '(".BASE" ".OTHER" ".THIS"))
          (delete-file (concat file suffix)))))
    '(nil t))

(princ (qa--done))
