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

;; Load just the status helpers so this checks path handling without Bazaar.
(let ((source (expand-file-name "lisp/vc-bzr.el" default-directory)))
  (with-temp-buffer
    (set-syntax-table emacs-lisp-mode-syntax-table)
    (insert-file-contents source)
    (dolist (definition '("(defconst vc-bzr-state-words"
                          "(defun vc-bzr--symlink-target"
                          "(defun vc-bzr-status"))
      (goto-char (point-min))
      (search-forward definition)
      (goto-char (match-beginning 0))
      (eval (read (current-buffer)) t)))
  (qa (and (functionp (symbol-function 'vc-bzr--symlink-target))
           (functionp (symbol-function 'vc-bzr-status)))
      t))

(qa
 (let* ((temp-root (make-temp-file "qa-vc-bzr-status-" t))
        (root (file-name-as-directory (file-truename temp-root)))
        (target (expand-file-name "target" root))
        (link (expand-file-name "link" root))
        (responses nil)
        (calls nil)
        (results nil)
        (vc-bzr-state-words
         "added\\|ignored\\|kind changed\\|modified\\|removed\\|renamed\\|unknown"))
   (unwind-protect
       (progn
         (with-temp-file target (insert "target contents"))
         (make-symbolic-link target link)
         (cl-letf (((symbol-function 'vc-bzr-root)
                    (lambda (_file) root))
                   ((symbol-function 'vc-bzr-file-name-relative)
                    (lambda (file) (file-relative-name file root)))
                   ((symbol-function 'vc-bzr-command)
                    (lambda (_command _buffer _okstatus file &rest _args)
                      (push file calls)
                      (let ((output (cdr (assoc file responses))))
                        (when output (insert output))))))
           (setq responses `((,target . "modified:\n  target\n")))
           (setq calls nil)
           (push (list (file-name-nondirectory
                        (or (vc-bzr--symlink-target link) ""))
                       (vc-bzr-status link)
                       (nreverse (mapcar #'file-name-nondirectory calls)))
                 results)
           (setq responses `((,target . "unknown:\n  target\n")))
           (push (vc-bzr-status link) results)
           (setq responses `((,link . "modified:\n  link\n")
                             (,target . "modified:\n  target\n")))
           (setq calls nil)
           (push (list (vc-bzr-status link) (length calls)) results)
           (nreverse results)))
     (when (file-exists-p link) (delete-file link))
     (when (file-exists-p target) (delete-file target))
     (when (file-directory-p temp-root) (delete-directory temp-root))))
 '(("target" (modified . nil) ("link" "target"))
   (unchanged . nil) ((modified . nil) 1)))

(princ (qa--done))
