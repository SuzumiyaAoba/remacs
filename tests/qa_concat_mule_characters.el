;;; -*- lexical-binding: nil; -*-
;; concat accepts the full Emacs character range from lists and vectors (#551).
(qa (string-to-list (concat '(1114112))) '(1114112))
(qa (string-to-list (concat [2097152])) '(2097152))
(qa (list (string-to-list (concat '(4194175)))
          (string-to-list (concat [4194175])))
    '((4194175) (4194175)))
;; Eight-bit Mule codes stay unibyte and round-trip as raw bytes.
(qa (let ((from-list (concat '(4194176)))
      (from-vector (concat [4194176])))
      (list (multibyte-string-p from-list) (string-to-list from-list)
            (multibyte-string-p from-vector) (string-to-list from-vector)))
    '(nil (128) nil (128)))
(princ (qa--done))
