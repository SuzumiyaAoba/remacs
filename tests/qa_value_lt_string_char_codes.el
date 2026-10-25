;;; -*- lexical-binding: nil; -*-
;; value< strings and symbol names compare Lisp character codes, not proxies (#569).
(qa (value< (unibyte-string 233) (string 234)) t)
(qa (value< (string 234) (unibyte-string 233)) nil)
(qa (value< (unibyte-string 233) (string 233)) nil)
(qa (value< (string 233) (unibyte-string 233)) nil)
(qa (let ((a (intern (unibyte-string 233)))
       (b (intern (string 234))))
      (value< a b))
    t)
(princ (qa--done))
