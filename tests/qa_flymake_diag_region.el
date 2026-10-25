;;; -*- lexical-binding: nil; -*-
;; Column diagnostics extend to the line end and invalid columns return nil (#576).
(require 'flymake)
(qa (with-temp-buffer
      (insert "abcdef")
      (flymake-diag-region (current-buffer) 1 3))
    '(3 . 7))
(qa (with-temp-buffer
      (insert "abc")
      (flymake-diag-region (current-buffer) 1 5))
    nil)
(qa (with-temp-buffer
      (insert "abc\ndef")
      (flymake-diag-region (current-buffer) 2))
    '(5 . 8))
(princ (qa--done))
