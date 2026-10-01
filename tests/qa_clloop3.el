;; cl-loop — GNU 31.1 pins: `it' binds the TEST value in if/when/unless
;; clauses (even on the else side); `until' tests AFTER the body; bare
;; `loop' is NOT defined in 31.1; mixed anonymous collect+sum errors.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == for VAR in/on (list iteration) ==
(qa (cl-loop for x in '(a b c) collect x) '(a b c))
(qa (cl-loop for x in '(1 2 3 4) by #'cddr collect x) '(1 3))
(qa (cl-loop for x in nil collect x) nil)
(qa (cl-loop for x in '(a b . c) collect x) '(a b)) ;; dotted tail ends iteration
(qa (cl-loop for x in 5 collect x) nil) ;; non-list in silently empties
(qa (cl-loop for x on '(a b c) collect x) '((a b c) (b c) (c)))
(qa (cl-loop for x on '(a b c) by #'cddr collect x) '((a b c) (c)))
(princ (qa--done))
