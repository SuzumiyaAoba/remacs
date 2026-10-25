;;; -*- lexical-binding: nil; -*-
;; nconc returns the final argument without traversing it (#496).
(qa (let ((cycle (list 'a)))
      (setcdr cycle cycle)
      (eq (nconc cycle) cycle))
    t)
(qa (let ((prefix (list 'p)) (cycle (list 'c)))
      (setcdr cycle cycle)
      (let ((result (nconc prefix cycle)))
        (list (eq result prefix) (eq (cdr prefix) cycle) (eq (cdr cycle) cycle))))
    '(t t t))
(qa (nconc (list 'a) 42) '(a . 42))
(qa (nconc) nil)
(princ (qa--done))
