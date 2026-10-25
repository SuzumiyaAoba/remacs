;;; -*- lexical-binding: nil; -*-
;; Time-zone consumers reject malformed conses and normalize invalid offsets (#470).
(let ((bad-type (cons "bad" "X"))
      (bad-shape (cons 3600 "X")))
  (qa (condition-case err (current-time-zone (current-time) bad-type)
        (error (car err))) 'error)
  (qa (condition-case err (decode-time 0 bad-shape)
        (error (car err))) 'error)
  (qa (condition-case err (format-time-string "%Z" 0 bad-shape)
        (error (car err))) 'error))
(qa (current-time-zone (current-time) 800000) '(0 "UTC"))
(qa (format-time-string "%Y %z %Z" 0 800000) "1970 +0000 UTC")
(qa (decode-time 0 800000) '(0 0 0 1 1 1970 4 nil 0))
(princ (qa--done))
