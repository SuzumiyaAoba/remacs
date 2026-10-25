;;; -*- lexical-binding: nil; -*-
;; truncate-string-to-width retains source and ellipsis properties (#430).
(let ((s (propertize "abcdef" 'face 'bold))
      (ellipsis (propertize "…" 'face 'italic)))
  (qa (get-text-property 0 'face (truncate-string-to-width s 3)) 'bold)
  (qa (get-text-property 0 'face
                         (truncate-string-to-width s 3 0 nil ellipsis))
      'italic))
(princ (qa--done))
