;;; -*- lexical-binding: nil; -*-
;; subst-char-in-string preserves properties on retained/replaced text (#610).
(qa
 (let ((s (propertize "éxé" 'face 'bold)))
   (object-intervals (subst-char-in-string 233 234 s)))
 '((0 3 (face bold))))
(qa
 (let ((s (propertize "abc" 'audit-prop 'kept)))
   (object-intervals (subst-char-in-string ?b ?x s)))
 '((0 3 (audit-prop kept))))
(princ (qa--done))
