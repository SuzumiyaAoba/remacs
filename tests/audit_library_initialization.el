;;; audit_library_initialization.el --- isolate remaining corpus differences -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "library:file-local-property-line"
 '(with-temp-buffer
    (emacs-lisp-mode)
    (insert ";; -*- lexical-binding: t -*-\n")
    (add-file-local-variable-prop-line 'fill-column 70)
    (buffer-string)))

(audit-check "library:files-x-after-require"
 '(progn
    (require 'files-x)
    (list (and (memq 'connection-local-variables-alist ignored-local-variables) t)
          (fboundp 'modify-file-local-variable-prop-line))))

(audit-check "library:text-scale-after-autoload"
 '(progn
    (with-temp-buffer (text-scale-increase 1))
    (and (assq 'text-scale-mode minor-mode-alist) t)))

(audit-check "library:text-scale-after-require"
 '(progn
    (require 'face-remap)
    (list (and (assq 'text-scale-mode minor-mode-alist) t)
          (and (assq 'buffer-face-mode minor-mode-alist) t))))

(princ "AUDIT-DONE\n")
