;;; -*- lexical-binding: nil; -*-
;; directory-file-name preserves unibyte path representation (#457).
(let ((file (directory-file-name (unibyte-string 233 47))))
  (qa (list (multibyte-string-p file) (string-bytes file)) '(nil 1)))
(princ (qa--done))
