;;; Modula-2 pointer dereference and field selection are separate tokens.

(require 'modula2)

(qa (with-temp-buffer
      (m2-mode)
      (insert "x ^. y")
      (goto-char 3)
      (list (m2-smie-forward-token) (char-after)))
    '("^" ?.))

(qa (with-temp-buffer
      (m2-mode)
      (insert "x ^. y")
      (goto-char 6)
      (list (m2-smie-backward-token) (char-before)))
    '("." ?^))

(princ (qa--done))
