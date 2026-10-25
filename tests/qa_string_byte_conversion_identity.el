;;; -*- lexical-binding: nil; -*-
;; No-op byte representation conversions preserve identity and properties (#421).
(let* ((s (propertize (unibyte-string 65) 'tag 7))
       (r (string-to-unibyte s)))
  (qa (eq s r) t)
  (qa (get-text-property 0 'tag r) 7)
  (qa (eq s (string-as-unibyte s)) t)
  (qa (eq s (string-make-unibyte s)) t))
(let ((s (string-to-multibyte "abc")))
  (qa (eq s (string-as-multibyte s)) t))
(princ (qa--done))
