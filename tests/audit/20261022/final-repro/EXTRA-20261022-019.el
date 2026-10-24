;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "insert:insert-char-zero-readonly" '(with-temp-buffer (setq buffer-read-only t) (insert-char 65 0)))
(princ "AUDIT-DONE\n")
