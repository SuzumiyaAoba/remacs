;;; -*- lexical-binding: t; -*-
;;; Zero-width rectangles still contain one empty string per line (Issue #2310).

(require 'rect)

(qa (with-temp-buffer
      (insert "abc")
      (extract-rectangle 1 1))
    '(""))

(qa (with-temp-buffer
      (insert "abc\ndef")
      (extract-rectangle 1 5))
    '("" ""))

(princ (qa--done))
