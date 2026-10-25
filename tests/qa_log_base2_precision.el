;;; -*- lexical-binding: nil; -*-
;; Base 2 uses the dedicated logarithm operation (#543).
(qa (log 10 2) 3.321928094887362)
(qa (log 10 2.0) 3.321928094887362)
(qa (log 8 2) 3.0)
(qa (log 1000 10) 3.0)
(qa (log 10 3) 2.095903274289385)
(qa (log 0 2) -1.0e+INF)
(qa (log -1 2) 0.0e+NaN)
(qa (log 10 nil) 2.302585092994046)
(princ (qa--done))
