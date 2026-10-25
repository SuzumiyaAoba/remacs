;;; -*- lexical-binding: nil; -*-
;; nconc links the previous tail before validating the next non-nil arg (#499).
(qa
 (let ((list (list 'before)))
   (condition-case nil (nconc list 42 nil) (error nil))
   list)
 '(before . 42))
(qa
 (let ((list (list 'before)))
   (condition-case nil (nconc list nil 42 nil) (error nil))
   list)
 '(before . 42))
;; Linking a later list can create a cycle before its traversal signals.
(qa
 (let ((prefix (list 'prefix)) (tail (list 'tail)))
   (setcdr tail prefix)
   (condition-case err
       (nconc prefix nil tail nil)
     (circular-list
      (list (eq (cdr prefix) tail) (eq (cdr tail) prefix)))))
 '(t t))
(princ (qa--done))
