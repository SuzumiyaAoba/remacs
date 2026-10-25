;;; -*- lexical-binding: nil; -*-
;; Filename extension helpers preserve unibyte substrings (#458).
(let* ((name (unibyte-string 233 46 255))
       (parts (list (file-name-extension name)
                    (file-name-sans-extension name)
                    (file-name-base name))))
  (qa (mapcar #'multibyte-string-p parts) '(nil nil nil))
  (qa (mapcar #'string-bytes parts) '(1 1 1)))
(princ (qa--done))
