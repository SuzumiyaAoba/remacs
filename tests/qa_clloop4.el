;; cl-loop — GNU 31.1 pins: `it' binds the TEST value in if/when/unless
;; clauses (even on the else side); `until' tests AFTER the body; bare
;; `loop' is NOT defined in 31.1; mixed anonymous collect+sum errors.
;; remacs: cl-loop comes from prelude.el.
(require 'cl-lib)

;; == for VAR being the … ==
;; `being the elements' works now (the skip pin was stale).
(qa (eval '(cl-loop for x being the elements of '(1 2 3) collect x)) '(1 2 3))
(qa (eval '(cl-loop for x being the elements of [1 2] collect (* x 2))) '(2 4))
(qa (eval '(cl-loop for x being the elements of "ab" collect x)) '(97 98))
(qa (eval '(cl-loop for x being the elements of nil collect x)) nil)
(let ((h (make-hash-table)))
  (puthash 'a 1 h) (puthash 'b 2 h)
  (qa (cl-loop for k being the hash-keys of h count t) 2)
  (qa (cl-loop for v being the hash-values of h sum v) 3))
(let ((h (make-hash-table)))
  (puthash 'a 1 h) (puthash 'b 2 h)
  (qa (cl-loop for k being the hash-keys of h using (hash-values v)
               collect (list k v))
      '((a 1) (b 2))))
(qa (cl-loop for k being the hash-keys of (make-hash-table) collect k) nil)
(qa (> (eval '(cl-loop for x being the symbols of obarray count t)) 1000) t)
(qa (eval '(cl-loop for x being the overlays of (current-buffer) collect x)) nil)
(qa (> (eval '(cl-loop for x being the key-codes of (current-global-map) count t)) 100) t)
(qa (>= (cl-loop for x being the buffers count t) 3) t)
(princ (qa--done))
