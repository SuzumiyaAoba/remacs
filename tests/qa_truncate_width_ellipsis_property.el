;;; -*- lexical-binding: nil; -*-
;; ELLIPSIS-TEXT-PROPERTY keeps elided text under a display property (#428).
(let ((s (truncate-string-to-width "abcdef" 3 0 nil "…" t)))
  (qa (substring-no-properties s) "abcdef")
  (qa (get-text-property 2 'display s) "…"))
(princ (qa--done))
