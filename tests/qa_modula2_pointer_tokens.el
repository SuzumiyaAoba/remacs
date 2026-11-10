;;; qa_modula2_pointer_tokens.el --- Modula-2 pointer token boundaries -*- lexical-binding:t -*-

;; The SMIE scanner must split "^." into distinct pointer and field tokens.

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
