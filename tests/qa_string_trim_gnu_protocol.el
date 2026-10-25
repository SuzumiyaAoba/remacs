;;; -*- lexical-binding: nil; -*-
;; Keep string-trim variants on GNU's string-match/substring path (#613).
(let ((s (copy-sequence "abc")))
  (qa (eq (string-trim s) s) t)
  (qa (eq (string-trim-left s) s) t)
  (qa (eq (string-trim-right s) s) t))
(let ((s (propertize " abc " 'audit-prop 'kept)))
  (qa (get-text-property 0 'audit-prop (string-trim s)) 'kept)
  (qa (get-text-property 0 'audit-prop (string-trim-left s)) 'kept)
  (qa (get-text-property 0 'audit-prop (string-trim-right s)) 'kept))
(qa (progn
      (string-match "z" "----z")
      (string-trim-left "bbbx" "b+")
      (match-data))
    '(0 3))
(qa-err-data (string-trim 7 "a" '(bad))
             'wrong-type-argument '(stringp 7))
(qa-err (string-trim-left "a" "[") 'invalid-regexp)
(princ (qa--done))
