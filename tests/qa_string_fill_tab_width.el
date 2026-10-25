;;; -*- lexical-binding: nil; -*-
;; string-fill uses the active tab-width for retained indentation (#418).
(let ((old (default-value 'tab-width)))
  (unwind-protect
      (progn
        (setq-default tab-width 4)
        (qa (string-fill "\ta b" 8) "\ta b"))
    (setq-default tab-width old)))
(princ (qa--done))
