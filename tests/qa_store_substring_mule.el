;;; -*- lexical-binding: nil; -*-
;; store-substring stores high-Mule integers as their string proxy (#382).
(let ((s (string ?a)))
  (store-substring s 0 #x110000)
  (qa (aref s 0) #x110000))
(princ (qa--done))
