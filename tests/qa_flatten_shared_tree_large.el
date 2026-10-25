;;; -*- lexical-binding: nil; -*-
;; flatten-tree does not truncate finite shared trees at an arbitrary visit limit (#462).
(let ((tree nil))
  (dotimes (_ 20) (setq tree (cons tree tree)))
  (qa (flatten-tree (cons tree 'sentinel)) '(sentinel)))
(princ (qa--done))
