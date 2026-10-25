;;; -*- lexical-binding: nil; -*-
;; value< returns tied immediately for the identical circular object (#563).
(qa (let ((cycle (list 0)))
      (setcdr cycle cycle)
      (value< cycle cycle))
    nil)
(qa (let ((cycle (vector nil)))
      (aset cycle 0 cycle)
      (value< cycle cycle))
    nil)
(princ (qa--done))
