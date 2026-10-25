;;; -*- lexical-binding: nil; -*-
;; value< includes compared objects in type-mismatch data (#559).
(qa-err-data (value< "text" 1) 'type-mismatch '("text" 1))
(qa-err-data (value< 1 "text") 'type-mismatch '(1 "text"))

;; GNU reverses bignum-left mismatches with non-vectorlike values.
(qa-err-data (value< (expt 2 65) "text") 'type-mismatch
             '("text" 36893488147419103232))
(qa-err-data (value< "text" (expt 2 65)) 'type-mismatch
             '("text" 36893488147419103232))
(let ((big (expt 2 65))
      (marker (make-marker)))
  (qa-err-data (value< big marker) 'type-mismatch (list big marker))
  (qa-err-data (value< marker big) 'type-mismatch (list marker big)))
(princ (qa--done))
