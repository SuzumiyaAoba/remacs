;;; -*- lexical-binding: nil; -*-
;; string-join preserves element and separator text properties (#371).
(let ((s (propertize "a" 'face 'bold))
      (separator (propertize "," 'face 'italic)))
  (qa (list (get-text-property 0 'face (string-join (list s s) separator))
            (get-text-property 1 'face (string-join (list s s) separator)))
      '(bold italic)))
(princ (qa--done))
