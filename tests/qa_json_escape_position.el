;;; -*- lexical-binding: nil; -*-
;; JSON escape errors report the consumed offending character's position (#522).
(qa-err-data (json-parse-string "\"\\q\"")
             'json-escape-sequence-error '(1 nil 3))
(qa-err-data (json-parse-string "\"\\uDC00\"")
             'json-invalid-surrogate-error '(1 nil 7))
(qa-err-data (json-parse-string "\"x\\uDC00\"")
             'json-invalid-surrogate-error '(1 nil 8))
(qa-err-data (json-parse-string "\"\\uD800\\u0041\"")
             'json-invalid-surrogate-error '(1 nil 13))
(princ (qa--done))
