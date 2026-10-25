;;; -*- lexical-binding: nil; -*-
;; format-time-string rejects unrepresentable timestamps and offsets (#489).
(qa (condition-case err (format-time-string "%Y" (expt 2 80) t)
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err (format-time-string "%Y" (- (expt 2 80)) t)
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err
        (format-time-string "%Y" 9223372036854775807 1)
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (format-time-string "%Y-%m-%d" 0 t) "1970-01-01")
(princ (qa--done))
