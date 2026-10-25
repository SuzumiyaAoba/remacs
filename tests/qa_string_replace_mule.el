;;; -*- lexical-binding: nil; -*-
;; Sequence replacements use concat's Emacs character conversion (#363).
(dolist (replacement (list (vector #x3fff80) (list #x3fff80)))
  (qa (equal (string-replace "x" replacement "x")
             (concat replacement)) t))
(princ (qa--done))
