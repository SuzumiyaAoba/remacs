;;; audit_startup_environment.el --- startup shell and key descriptions -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "shell:initial-shell-environment"
 '(list (or (getenv "ESHELL") (getenv "SHELL") "/bin/sh") shell-file-name))

(audit-check "shell:explicit-shell-control"
 '(let ((shell-file-name "/bin/sh"))
    (shell-command-to-string "printf audit-shell-ok")))

(audit-check "keymap:key-parse-repeat"
 '(list (append (key-parse "3*C-x") nil) (append (key-parse "2*a") nil)))

(audit-check "keymap:key-parse-repeat-control"
 '(list (append (key-parse "C-x C-x C-x") nil) (append (key-parse "a a") nil)))

(princ "AUDIT-DONE\n")
