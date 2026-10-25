;;; -*- lexical-binding: nil; -*-
;; file-name-directory preserves unibyte directory prefixes (#455).
(let ((directory (file-name-directory (unibyte-string 233 47 120))))
  (qa (list (multibyte-string-p directory) (string-bytes directory)) '(nil 2)))
(princ (qa--done))
