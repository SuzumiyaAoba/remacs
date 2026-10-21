;;; -*- lexical-binding: t; -*-
;; Issue #222: GNU truncates an existing output before checking whether
;; it is the input inode, then reports file-error. Use owned temp files.
(defvar qa222-root)
(defvar qa222-source)
(defvar qa222-target)
(defvar qa222-kind)
(defvar qa222-flags)
(defvar qa222-expected-target)
(let ((qa222-root (make-temp-file "qa222-copy-self-" t)))
  (unwind-protect
      (dolist (qa222-kind '(same hardlink symlink source-symlink relative directory directory-slash))
        (let* ((default-directory qa222-root)
               (qa222-source (expand-file-name "source"))
               (qa222-target (expand-file-name "alias")))
          (with-temp-file qa222-source (insert "original"))
          (set-file-modes qa222-source #o600)
          (cond
           ((eq qa222-kind 'same) (setq qa222-target qa222-source))
           ((eq qa222-kind 'hardlink) (add-name-to-file qa222-source qa222-target))
           ((eq qa222-kind 'symlink) (make-symbolic-link qa222-source qa222-target))
           ((eq qa222-kind 'source-symlink)
            (make-symbolic-link qa222-source qa222-target)
            (setq qa222-target qa222-source qa222-source (expand-file-name "alias")))
           ((eq qa222-kind 'relative) (setq qa222-source "source" qa222-target "./source"))
           ((eq qa222-kind 'directory) (setq qa222-target qa222-root))
           ((eq qa222-kind 'directory-slash) (setq qa222-target (file-name-as-directory qa222-root))))
          (let ((qa222-expected-target
                 (if (eq qa222-kind 'directory-slash)
                     (expand-file-name "source")
                   (expand-file-name qa222-target))))
            ;; An overwrite refusal happens before opening/truncating.
            (qa-err-data (copy-file qa222-source qa222-target nil)
                         'file-already-exists
                         (list "File already exists" qa222-expected-target))
            (qa (with-temp-buffer (insert-file-contents-literally qa222-source) (buffer-string))
                "original")
            (dolist (qa222-flags '((nil nil nil) (nil nil t) (nil t nil) (nil t t)
                                  (t nil nil) (t nil t) (t t nil) (t t t)))
              (with-temp-file qa222-source (insert "original"))
              (qa (let ((error (condition-case error
                                  (apply #'copy-file qa222-source qa222-target t qa222-flags)
                                (error error))))
                    (list (car error) (cadr error) (stringp (nth 2 error))
                          (nth 3 error) (nth 4 error) (length error)))
                  (if (eq qa222-kind 'directory)
                      (list 'file-error "Opening output file" t qa222-expected-target nil 4)
                    (list 'file-error "Input and output files are the same" t
                          (expand-file-name qa222-source) qa222-expected-target 5)))
              (qa (with-temp-buffer (insert-file-contents-literally qa222-source) (buffer-string))
                  (if (eq qa222-kind 'directory) "original" ""))
              (qa (file-modes qa222-source) #o600)))
          (delete-file qa222-source)
          (when (file-exists-p (expand-file-name "source")) (delete-file (expand-file-name "source")))
          (when (file-symlink-p (expand-file-name "alias")) (delete-file (expand-file-name "alias")))
          (when (file-exists-p (expand-file-name "alias")) (delete-file (expand-file-name "alias")))))
    (delete-directory qa222-root t)))

;; Failure to open the output takes precedence over the identity error,
;; and does not truncate the source. These checks need an unprivileged uid.
(unless (= (user-uid) 0)
  (let* ((qa222-root (make-temp-file "qa222-copy-readonly-" t))
         (qa222-source (expand-file-name "source" qa222-root)))
    (unwind-protect
        (progn
          (with-temp-file qa222-source (insert "original"))
          (set-file-modes qa222-source #o400)
          (qa (let ((error (condition-case error (copy-file qa222-source qa222-source t)
                                (error error))))
                (list (car error) (cadr error) (stringp (nth 2 error))
                      (nth 3 error) (length error)))
              (list 'permission-denied "Opening output file" t qa222-source 4))
          (qa (with-temp-buffer (insert-file-contents-literally qa222-source) (buffer-string)) "original"))
      (delete-directory qa222-root t))))

(princ (qa--done)) (terpri)
