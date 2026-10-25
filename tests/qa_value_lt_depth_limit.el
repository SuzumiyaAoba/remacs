;;; -*- lexical-binding: nil; -*-
;; value< permits 200 nested comparisons, independent of Lisp eval depth (#564).
(defun qa-value-lt-nest (depth leaf)
  (dotimes (_ depth) (setq leaf (vector leaf)))
  leaf)
(qa (let ((max-lisp-eval-depth 20))
      (value< (qa-value-lt-nest 65 1) (qa-value-lt-nest 65 2)))
    t)
(qa (let ((max-lisp-eval-depth 20))
      (value< (qa-value-lt-nest 200 1) (qa-value-lt-nest 200 2)))
    t)
(qa-err-data (value< (qa-value-lt-nest 201 1) (qa-value-lt-nest 201 2))
             'error '("Maximum depth exceeded in comparison"))
(princ (qa--done))
