;;; -*- lexical-binding: nil; -*-
;; file-name-nondirectory preserves unibyte filename representation (#453).
(let ((base (file-name-nondirectory (unibyte-string 233))))
  (qa (list (multibyte-string-p base) (string-bytes base)) '(nil 1)))
(princ (qa--done))
