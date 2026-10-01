;; cl-loop — GNU 31.1 pins: `it' binds the TEST value in if/when/unless
;; clauses (even on the else side); `until' tests AFTER the body; bare
;; `loop' is NOT defined in 31.1; mixed anonymous collect+sum errors.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == for VAR across (array iteration) ==
(qa (cl-loop for x across [1 2 3] collect x) '(1 2 3))
(qa (cl-loop for c across "abc" collect c) '(97 98 99))
(qa (cl-loop for c across "aéb" collect c) '(97 233 98)) ;; chars, not bytes
(qa (cl-loop for x across nil collect x) nil)
(qa (cl-loop for x across "" collect x) nil)
(qa (cl-loop for x across [1] with y = 99 collect y) '(99))
(princ (qa--done))
