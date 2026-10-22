;;; qa_temp_write_modes.el --- Write initial contents using the creation descriptor -*- lexical-binding: t; -*-
(defun qa340-mode (internal mode)
  (let* ((root (make-temp-file "qa340-" t))
         (old (default-file-modes)) file actual)
    (unwind-protect
        (progn
          (unwind-protect
              (progn
                (set-default-file-modes mode)
                (setq file (if internal
                               (make-temp-file-internal (expand-file-name "data-" root) nil "" "abc")
                             (make-temp-file (expand-file-name "data-" root) nil nil "abc"))))
            (set-default-file-modes old))
          (setq actual (logand (file-modes file) 511))
          (set-file-modes file 384)
          (list actual (with-temp-buffer
                         (insert-file-contents-literally file)
                         (buffer-string))))
      (set-default-file-modes old)
      (delete-directory root t))))

(qa (qa340-mode nil 0) '(0 "abc"))

(qa (qa340-mode nil 128) '(128 "abc"))

(qa (qa340-mode nil 256) '(256 "abc"))

(qa (qa340-mode nil 384) '(384 "abc"))

(qa (qa340-mode t 0) '(0 "abc"))

(qa (qa340-mode t 128) '(128 "abc"))

(qa (qa340-mode t 256) '(256 "abc"))

(qa (qa340-mode t 384) '(384 "abc"))

(princ (qa--done))
