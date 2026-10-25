;;; -*- lexical-binding: nil; -*-
;; Trimming must preserve properties on retained characters (#584).
(qa
 (let ((s (propertize "  abc  " 'audit-prop 'kept)))
   (list (text-properties-at 0 (string-trim s))
         (text-properties-at 0 (string-trim-left s))
         (text-properties-at 0 (string-trim-right s))))
 '((audit-prop kept) (audit-prop kept) (audit-prop kept)))
(qa
 (let* ((s (propertize "  a   b \n c  " 'audit-prop 'kept))
        (out (string-clean-whitespace s)))
   (list out
         (mapcar (lambda (i) (get-text-property i 'audit-prop out))
                 '(0 1 2 3 4))))
 '(#("a b c" 0 1 (audit-prop kept) 2 3 (audit-prop kept) 4 5 (audit-prop kept))
   (kept nil kept nil kept)))
(princ (qa--done))
