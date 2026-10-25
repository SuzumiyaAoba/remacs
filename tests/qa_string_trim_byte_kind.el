;;; -*- lexical-binding: nil; -*-
;; string-trim retains unibyte representation through substring (#389).
(let ((s (unibyte-string 233)))
  (qa (list (multibyte-string-p (string-trim s))
            (multibyte-string-p (string-trim-left s "x"))
            (multibyte-string-p
             (string-trim-left (unibyte-string 32 233))))
      '(nil nil nil)))
(princ (qa--done))
