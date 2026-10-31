;;; -*- lexical-binding: t -*-
;; The actual library must load and expose both formerly unclosed forms (#1791).
(require 'dirtrack)

(qa (featurep 'dirtrack) t)
(qa (fboundp 'dirtrack--change-directory) t)
(qa (fboundp 'dirtrack) t)

(qa (with-temp-buffer
      (let ((input (copy-sequence "untouched")))
        (setq-local dirtrack-mode nil)
        (eq (dirtrack input) input)))
    t)
(qa (with-temp-buffer
      (let ((input (copy-sequence "empty buffer")))
        (setq-local dirtrack-mode t)
        (eq (dirtrack input) input)))
    t)
(qa (with-temp-buffer
      (dirtrack-mode 1)
      (let ((enabled (not (null (memq 'dirtrack comint-preoutput-filter-functions)))))
        (dirtrack-mode -1)
        (list enabled
              (not (null (memq 'dirtrack comint-preoutput-filter-functions)))
              dirtrack--pending-input)))
    '(t nil nil))

(defun qa-dirtrack--paths (chunks)
  (let ((original (symbol-function 'dirtrack--change-directory))
        seen)
    (unwind-protect
        (progn
          (fset 'dirtrack--change-directory
                (lambda (path _input) (push path seen)))
          (with-temp-buffer
            (insert "x")
            (setq-local dirtrack-mode t
                        dirtrack-list '("^P\\([^>]+\\)> " 1)
                        dirtrack--pending-input "")
            (dolist (chunk chunks) (dirtrack chunk)))
          (nreverse seen))
      (fset 'dirtrack--change-directory original))))

(qa (qa-dirtrack--paths '("P/a/> \nP/b/> ")) '("/a/" "/b/"))
(qa (qa-dirtrack--paths '("P/" "split/" "> ")) '("/split/"))

(princ (qa--done))
