;;; -*- lexical-binding: nil; -*-
;; store-substring rejects a non-byte character in an unibyte string.
(let ((s (string ?a)))
  (qa-err (store-substring s 0 #x110000) 'error)
  (qa (aref s 0) ?a))
(princ (qa--done))
