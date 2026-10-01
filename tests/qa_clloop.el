;; cl-loop — GNU 31.1 pins: `it' binds the TEST value in if/when/unless
;; clauses (even on the else side); `until' tests AFTER the body; bare
;; `loop' is NOT defined in 31.1; mixed anonymous collect+sum errors.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == for VAR from/downfrom/upfrom ==
(qa (cl-loop for i from 1 to 5 collect i) '(1 2 3 4 5))
(qa (cl-loop for i from 1 below 5 collect i) '(1 2 3 4))
(qa (cl-loop for i from 5 downto 1 collect i) '(5 4 3 2 1))
(qa (cl-loop for i from 5 above 1 collect i) '(5 4 3 2))
(qa (cl-loop for i from 1 to 10 by 3 collect i) '(1 4 7 10))
(qa (cl-loop for i downfrom 5 to 1 collect i) '(5 4 3 2 1))
(qa (cl-loop for i downfrom 5 above 2 collect i) '(5 4 3))
(qa (cl-loop for i upfrom 1 to 4 collect i) '(1 2 3 4))
(qa (cl-loop for i upfrom 0 below 3 collect i) '(0 1 2))
(qa (cl-loop for i from 10 downto 0 by 3 collect i) '(10 7 4 1))
(qa (cl-loop for i from 0 downto -2 collect i) '(0 -1 -2))
(qa (cl-loop for i from 3 downto 1 by 2 collect i) '(3 1))
(qa (cl-loop for i below 5 by 2 collect i) '(0 2 4))
(qa (cl-loop for i below 3 collect i) '(0 1 2))
(qa (cl-loop for i below 0 collect i) nil)
(qa (cl-loop for i below 3.0 collect i) '(0 1 2)) ;; float bound, integer steps
(princ (qa--done))
