;;; -*- lexical-binding: nil; -*-
;; line-number-at-pos clips to point-max in a narrowed buffer (#472).
(with-temp-buffer
  (insert "a\nb\nc")
  (narrow-to-region (point-min) 3)
  (qa (line-number-at-pos 6) 2))
(princ (qa--done))
