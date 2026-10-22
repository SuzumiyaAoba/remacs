;;; qa_temp_private_modes.el --- Temporary objects start private -*- lexical-binding: t; -*-

(require 'subr-x)

(defun qa326-temp-mode (internal directory &optional text)
  (let* ((parent (make-temp-file "qa326-parent-" t))
         (prefix (expand-file-name "private-" parent))
         (file nil))
    (unwind-protect
        (progn
          (setq file (if internal
                         (make-temp-file-internal prefix directory ".data" text)
                       (make-temp-file prefix directory ".data" text)))
          (list (logand (file-modes file) 511)
                (if (file-directory-p file) 'directory 'file)
                (string-suffix-p ".data" file)
                (unless directory
                  (with-temp-buffer (insert-file-contents-literally file) (buffer-string)))))
      (delete-directory parent t))))

(qa (qa326-temp-mode nil nil nil)
    '(384 file t ""))

(qa (qa326-temp-mode nil nil "payload")
    '(384 file t "payload"))

(qa (qa326-temp-mode nil t nil)
    '(448 directory t nil))

(qa (qa326-temp-mode t nil nil)
    '(384 file t ""))

(qa (qa326-temp-mode t nil "payload")
    '(384 file t "payload"))

(qa (qa326-temp-mode t t nil)
    '(448 directory t nil))

(princ (qa--done))
