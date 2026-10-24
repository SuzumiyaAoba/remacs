;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "insert:insert-char-inherit-t" '(with-temp-buffer (insert (propertize "a" 'face 'bold)) (insert-char 32 2 t) (list (buffer-string) (get-text-property 2 'face))))
(princ "AUDIT-DONE\n")
