;;; -*- lexical-binding: nil; -*-
;; string-fill preserves sentence-ending double spaces (#405).
(let ((sentence-end-double-space t))
  (qa (string-fill "First sentence.  Second sentence." 80)
      "First sentence.  Second sentence."))
(princ (qa--done))
