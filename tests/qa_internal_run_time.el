;;; -*- lexical-binding: nil; -*-
;; get-internal-run-time measures elapsed process time, not the epoch (#500).
(qa (time-less-p (get-internal-run-time) (current-time)) t)
(qa (let ((before (get-internal-run-time))
          (after (get-internal-run-time)))
      (not (time-less-p after before)))
    t)
(princ (qa--done))
