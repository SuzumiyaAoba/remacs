;;; -*- lexical-binding: nil; -*-
;; assoc-string has no fixed scan limit on finite alists (#532).
(qa (let* ((source (make-list 500001 42))
           (end (last source))
           (tail (list (cons "target" 'value))))
      (setcdr end tail)
      (unwind-protect (eq (assoc-string "target" source) (car tail))
        (setcdr end source)))
    t)
(princ (qa--done))
