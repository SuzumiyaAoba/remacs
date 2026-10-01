;; ring.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - make-ring N creates an empty ring of size N; make-ring 0 →
;;     a degenerate empty ring; make-ring -1 → wrong-type-argument.
;;   - ring-insert pushes on the FRONT (newest); ring-elements lists
;;     newest-first.
;;   - Overflow drops the OLDEST element silently.
;;   - ring-ref INDEX: 0 = newest; negative counts from OLDEST back
;;     toward newer (-1 = oldest, -2, -3 step toward newest);
;;     out-of-range → nil slot, no error.
;;   - ring-remove with no arg removes+returns the OLDEST; with INDEX
;;     removes the element at INDEX (0 = newest).
;;   - ring-pop is NOT a function in 31.1 (use ring-remove).
;;   - ring-extend grows by N only; ring-insert+extend inserts, growing
;;     by N only when full.
;;   - ring-convert-sequence-to-ring preserves input order — elements
;;     come out in input order (first input = oldest… verify: returned
;;     (1 2 3) → ring-elements (1 2 3)).

(require 'ring)

;; == make-ring / predicates ==
(let ((r (make-ring 3)))
  (qa (ring-p r) t)
  (qa (ring-size r) 3)
  (qa (ring-length r) 0)
  (qa (ring-empty-p r) t))
;; make-ring 0 → degenerate empty ring (no error)
(qa (ring-empty-p (make-ring 0)) t)
;; negative size → wrong-type-argument
(qa-err (make-ring -1) 'wrong-type-argument)

;; == insert / ordering ==
(let ((r (make-ring 5)))
  (ring-insert r 'a)
  (qa (ring-empty-p r) nil)
  (ring-insert r 'b)
  (ring-insert r 'c)
  ;; ring-elements lists NEWEST first
  (qa (ring-elements r) '(c b a)))

;; == overflow drops oldest ==
(let ((r (make-ring 3)))
  (dolist (e '(a b c d)) (ring-insert r e))
  (qa (ring-length r) 3)
  (qa (ring-elements r) '(d c b)))

;; == ring-ref ==
(let ((r (make-ring 5)))
  (dolist (e '(a b c d)) (ring-insert r e))
  ;; 0 = newest
  (qa (ring-ref r 0) 'd)
  (qa (ring-ref r 1) 'c)
  (qa (ring-ref r 2) 'b)
  (qa (ring-ref r 3) 'a)
  ;; negative: -1 = oldest, then steps toward newest
  (qa (ring-ref r -1) 'a)
  (qa (ring-ref r -2) 'b)
  (qa (ring-ref r -3) 'c)
  ;; out-of-range wraps modulo ring-length
  (qa (ring-ref r 4) 'd)
  (qa (ring-ref r 5) 'c)
  (qa (ring-ref r -4) 'd))

;; == ring-remove ==
(let ((r (make-ring 3)))
  (dolist (e '(x y z)) (ring-insert r e))
  ;; no arg: removes+returns the OLDEST
  (qa (ring-remove r) 'x)
  (qa (ring-length r) 2)
  ;; with INDEX: removes element at INDEX (0 = newest)
  (qa (ring-remove r 0) 'z)
  (qa (ring-length r) 1))

;; == ring-pop: not a function in 31.1 ==
(qa (fboundp 'ring-pop) nil)

;; == ring-insert+extend ==
(let ((r (make-ring 2)))
  (ring-insert+extend r 'a 1)
  (ring-insert+extend r 'b 1)
  (ring-insert+extend r 'c 1)         ; full → extends by 1
  (qa (ring-length r) 3)
  (qa (ring-size r) 3))

;; == ring-insert-at-beginning ==
(let ((r (make-ring 5)))
  (ring-insert r 'x)
  (ring-insert r 'y)
  (ring-insert-at-beginning r 'z)
  ;; ring-elements newest-first → (y x z)
  (qa (ring-elements r) '(y x z)))

;; == ring-copy ==
(let ((r (make-ring 3)))
  (ring-insert r 'a)
  (ring-insert r 'b)
  (let ((rc (ring-copy r)))
    (qa (ring-p rc) t)
    (qa (ring-elements rc) (ring-elements r))
    ;; independent: mutating rc doesn't affect r
    (ring-insert rc 'zz)
    (qa (ring-length r) 2)
    (qa (ring-length rc) 3)))

;; == ring-member ==
(let ((r (make-ring 3)))
  (ring-insert r 'a)
  (qa (ring-member r 'a) 0)
  (qa (ring-member r 'z) nil))

;; == ring-extend ==
(let ((r (make-ring 3)))
  (ring-extend r 2)
  (qa (ring-size r) 5))

;; == ring-convert-sequence-to-ring (Emacs 29+) ==
(qa (fboundp 'ring-convert-sequence-to-ring) t)
;; preserves input order
(let ((r (ring-convert-sequence-to-ring '(1 2 3))))
  (qa (ring-p r) t)
  (qa (ring-elements r) '(1 2 3)))

(princ (qa--done))
