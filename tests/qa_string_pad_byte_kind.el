;;; -*- lexical-binding: nil; -*-
;; string-pad preserves source representation through concat (#387).
(qa (list (multibyte-string-p (string-pad (unibyte-string 233) 1))
          (multibyte-string-p (string-pad (string-to-multibyte "abc") 3))
          (multibyte-string-p (string-pad (string-to-multibyte "abc") 4)))
    '(nil t t))
(princ (qa--done))
