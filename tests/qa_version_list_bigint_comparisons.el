;;; -*- lexical-binding: nil; -*-
;; version-list comparisons accept arbitrary-precision integer elements (#583).
(qa
 (let* ((big (string-to-number (concat (make-string 42 ?9))))
        (larger (1+ big)))
   (list (version-list-< (list big) (list larger))
         (version-list-<= (list big) (list big))
         (version-list-= (list larger) (list larger))))
 '(t t t))
(princ (qa--done))
