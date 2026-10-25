;;; -*- lexical-binding: nil; -*-
;; string-pixel-width uses its subr-x pixel measurement and BUFFER argument (#442).
(let ((buffer (get-buffer-create " *qa-pixel-width*")))
  (unwind-protect
      (progn
        (with-current-buffer buffer (erase-buffer) (insert "abc"))
        (qa (numberp (string-pixel-width "abc")) t)
        (qa (numberp (string-pixel-width "abc" buffer)) t))
    (kill-buffer buffer)))
(princ (qa--done))
