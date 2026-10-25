;;; -*- lexical-binding: nil; -*-
;; Version components retain arbitrary-precision integer values (#582).
(qa
 (let ((a (concat (make-string 42 ?9) ".1"))
       (b (concat "1" (make-string 42 ?0) ".0")))
   (list (version-to-list a)
         (version-to-list b)
         (version< a b)
         (version-list-< (version-to-list a) (version-to-list b))))
 '((999999999999999999999999999999999999999999 1)
   (1000000000000000000000000000000000000000000 0)
   t t))
(princ (qa--done))
