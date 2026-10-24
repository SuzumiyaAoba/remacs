;;; -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(audit-check "temp-write:public" '(let ((dir (make-temp-file "remacs-audit-write-" t)) file
                       (old-modes (default-file-modes)))
  (unwind-protect
      (progn
        (unwind-protect
            (progn (set-default-file-modes #o400) (setq file (make-temp-file (expand-file-name "data-" dir) nil nil "abc")))
          (set-default-file-modes old-modes))
        (list (logand (file-modes file) #o777)
              (with-temp-buffer (insert-file-contents-literally file) (buffer-string))))
    (when file (delete-file file)) (delete-directory dir t))))
(audit-check "temp-write:internal" '(let ((dir (make-temp-file "remacs-audit-write-" t)) file
                       (old-modes (default-file-modes)))
  (unwind-protect
      (progn
        (unwind-protect
            (progn (set-default-file-modes #o400) (setq file (make-temp-file-internal (expand-file-name "data-" dir) nil "" "abc")))
          (set-default-file-modes old-modes))
        (list (logand (file-modes file) #o777)
              (with-temp-buffer (insert-file-contents-literally file) (buffer-string))))
    (when file (delete-file file)) (delete-directory dir t))))
(princ "AUDIT-DONE\n")
