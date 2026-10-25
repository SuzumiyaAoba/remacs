;;; -*- lexical-binding: nil; -*-
;; encode-time rejects unrepresentable struct tm fields in every zone (#488).
(qa (condition-case err
        (encode-time '(0 0 0 1 1 1000000000000 nil nil nil))
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err
        (encode-time '(0 1000000000000 0 1 1 1970 nil nil nil))
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err
        (encode-time '(0 0 0 1 1 1000000000000 nil nil t))
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (condition-case err
        (encode-time '(0 1000000000000 0 1 1 1970 nil nil t))
      (error (error-message-string err)))
    "Specified time is not representable")
(qa (format-time-string "%Y-%m-%d"
        (encode-time '(0 0 24 1 1 2000 nil -1 t)) t)
    "2000-01-02")
(princ (qa--done))
