;;; -*- lexical-binding: nil; -*-
;; eventp accepts arbitrary-precision integers and symbol-headed conses (#592).
(qa
 (mapcar #'eventp
         '(100000000000000000000000 (1) (1 . 2) a (a) nil :keyword
           #:uninterned))
 '(t nil nil t t nil nil t))
(qa
 (let ((positioned (position-symbol 'alpha 1)))
   (list (eventp positioned)
         (eventp (cons positioned nil))
         (let ((symbols-with-pos-enabled t))
           (list (eventp positioned) (eventp (cons positioned nil))))))
 '(nil nil (t t)))
(princ (qa--done))
