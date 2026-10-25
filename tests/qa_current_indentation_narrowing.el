;;; -*- lexical-binding: nil; -*-
;; current-indentation ignores whitespace before point-min (#476).
(with-temp-buffer
  (insert "   abc")
  (narrow-to-region 4 7)
  (goto-char (point-min))
  (qa (current-indentation) 0))
(princ (qa--done))
