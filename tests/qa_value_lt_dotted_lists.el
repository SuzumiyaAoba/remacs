;;; -*- lexical-binding: nil; -*-
;; value< compares dotted tails and only inspects reachable cdrs (#568).
(qa (value< '(1 . 2) '(1 . 3)) t)
(qa (value< '(1 . 4) '(1 . 3)) nil)
(qa (value< '(0 . invalid-tail) '(1 . invalid-tail)) t)

;; A tied car must continue into the tails; a reachable cycle still signals.
(qa (let ((a (list 0)) (b (list 0)))
      (setcdr a a)
      (setcdr b b)
      (condition-case e (value< a b) (error (car e))))
    'circular-list)
(qa (let ((a (list 0)) (b (list 1)))
      (setcdr a a)
      (setcdr b b)
      (value< a b))
    t)
(princ (qa--done))
