;;; -*- lexical-binding: nil; -*-
;; decode-time checks fixed-zone addition at i64 boundaries (#493).
(qa (condition-case err (decode-time 9223372036854775800 100)
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err (decode-time -9223372036854775800 -100)
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (decode-time 0 0) '(0 0 0 1 1 1970 4 nil 0))
(princ (qa--done))
