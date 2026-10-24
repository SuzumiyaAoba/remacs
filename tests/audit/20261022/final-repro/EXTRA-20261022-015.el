;;; -*- lexical-binding: t; -*-
(load "/Users/suzumiyaaoba/ghq/github.com/SuzumiyaAoba/remacs/tests/audit_helpers.el" nil t)
(audit-check "insert:indent-to-inherit" '(with-temp-buffer (insert (propertize "a" 'face 'bold)) (indent-to 4) (list (buffer-string) (get-text-property 2 'face))))
(princ "AUDIT-DONE\n")
