;;; -*- lexical-binding: nil; -*-
;; file-name-as-directory preserves its input representation (#456).
(qa (list (multibyte-string-p (file-name-as-directory (unibyte-string 233)))
          (multibyte-string-p (file-name-as-directory (string-to-multibyte "abc"))))
    '(nil t))
(princ (qa--done))
