;;; -*- lexical-binding: nil; -*-
;; Trim regex sequences keep Remacs' Emacs character proxies (#377).
(let ((s (string #x3fff80)))
  (qa (string= (string-trim-left s (vector #x3fff80)) "") t)
  (qa (string= (string-trim-left s (list #x3fff80)) "") t))
(princ (qa--done))
