;;; -*- lexical-binding: nil; -*-
;; delete compares Lisp character codes and preserves source byte kind (#386).
(let ((result (delete 255 (unibyte-string 65 255 66))))
  (qa result "AB")
  (qa (list (multibyte-string-p result) (string-bytes result)) '(nil 2)))
(qa (delete #x110000 (string #x110000 ?a)) "a")
(princ (qa--done))
