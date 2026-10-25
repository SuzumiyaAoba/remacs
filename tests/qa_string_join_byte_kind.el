;;; -*- lexical-binding: nil; -*-
;; string-join follows concat's unibyte result rule (#385).
(let ((joined (string-join (list (unibyte-string 233 255)))))
  (qa (multibyte-string-p joined) nil)
  (qa (string-bytes joined) 2))
(princ (qa--done))
