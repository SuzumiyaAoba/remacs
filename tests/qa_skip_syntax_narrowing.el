;;; -*- lexical-binding: nil; -*-
;; skip-syntax-forward/backward count only accessible characters (#475).
(with-temp-buffer
  (insert "abcdef")
  (narrow-to-region 1 3)
  (goto-char (point-min))
  (qa (list (skip-syntax-forward "w" 7) (point)) '(2 3)))
(with-temp-buffer
  (insert "abcdef")
  (narrow-to-region 4 7)
  (goto-char (point-max))
  (qa (list (skip-syntax-backward "w" 1) (point)) '(-3 4)))
(princ (qa--done))
