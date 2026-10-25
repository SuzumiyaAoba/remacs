;;; -*- lexical-binding: nil; -*-
;; Empty symbol names print as ## unless a #: gensym prefix represents them (#590).
(qa
 (let* ((print-gensym t)
        (symbol (intern "")))
   (list (prin1-to-string symbol)
         (with-output-to-string (princ symbol))))
 '("##" "##"))
(qa
 (let* ((print-gensym t)
        (symbol (make-symbol "")))
   (list (prin1-to-string symbol)
         (with-output-to-string (princ symbol))))
 '("#:" "#:"))
(qa
 (let* ((print-gensym nil)
        (symbol (make-symbol "")))
   (list (prin1-to-string symbol)
         (with-output-to-string (princ symbol))))
 '("##" "##"))
(qa
 (let* ((print-gensym t)
        (symbol (position-symbol (make-symbol "") 1)))
   (prin1-to-string symbol))
 "#<symbol #: at 1>")
(princ (qa--done))
