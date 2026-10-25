;;; -*- lexical-binding: nil; -*-
;; pos-bol and pos-eol stay inside narrowed bounds (#474).
(with-temp-buffer
  (insert "abcdef\nxyz")
  (narrow-to-region 3 5)
  (goto-char (point-min))
  (qa (list (point-min) (point-max) (pos-bol) (pos-eol)) '(3 5 3 5)))
(princ (qa--done))
