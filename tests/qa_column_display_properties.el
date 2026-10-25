;;; -*- lexical-binding: nil; -*-
;; current-column and move-to-column measure display text properties (#473).
(with-temp-buffer
  (insert "abc")
  (put-text-property 1 2 'display "long")
  (goto-char (point-max))
  (qa (current-column) 6)
  (goto-char (point-min))
  (qa (move-to-column 6) 6)
  (qa (point) 4)
  (qa (current-column) 6))
(princ (qa--done))
