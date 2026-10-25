;;; -*- lexical-binding: nil; -*-
;; String casing emits high-Mule case-table results (#383).
(let* ((table (copy-case-table (standard-case-table)))
       (high #x110000))
  (set-case-syntax-pair high ?a table)
  (with-temp-buffer
    (set-case-table table)
    (qa (equal (upcase "a") (string high)) t)
    (qa (equal (downcase (string high)) "a") t)))
(princ (qa--done))
