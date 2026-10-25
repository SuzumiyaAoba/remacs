;;; -*- lexical-binding: nil; -*-
;; nconc uses cycle detection instead of a fixed traversal limit (#497).
(qa
 (let ((cycle (list 'x)))
   (setcdr cycle cycle)
   (condition-case err
       (nconc cycle nil)
     (circular-list
      (list (car err) (eq (cadr err) cycle)))))
 '(circular-list t))
(qa
 (let ((prefix (list 'p)) (cycle (list 'c)))
   (setcdr cycle cycle)
   (condition-case err
       (nconc prefix cycle nil)
     (circular-list
      (list (car err) (eq (cadr err) cycle)
            (eq (cdr prefix) cycle) (eq (cdr cycle) cycle)))))
 '(circular-list t t t))
(qa (let ((long (make-list 1000001 nil)))
      (eq (nconc long nil) long))
    t)
(princ (qa--done))
