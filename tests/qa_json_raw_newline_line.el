;;; -*- lexical-binding: nil; -*-
;; Raw newline in a JSON string increments the error line first (#524).
(qa-err-data
 (json-parse-string (concat (char-to-string 34) "a"
                            (char-to-string 10) (char-to-string 34)))
 'json-parse-error '(2 nil 3))
(princ (qa--done))
