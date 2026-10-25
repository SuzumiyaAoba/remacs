;;; -*- lexical-binding: nil; -*-
;; file-name-concat combines component byte kinds (#459).
(let ((path (file-name-concat (unibyte-string 233) "x")))
  (qa (list (multibyte-string-p path) (string-bytes path)) '(nil 3)))
(princ (qa--done))
