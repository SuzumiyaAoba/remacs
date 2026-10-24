;;; audit_time_modifiers.el --- recheck issue 77 -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "time:width-modifiers"
 '(list (format-time-string "%5Y" 0 t)
        (format-time-string "%03d" 0 t)
        (format-time-string "%_5Y" 0 t)
        (format-time-string "%05Y" 0 t)))

(audit-check "time:combined-padding-flags"
 '(list (format-time-string "%-_5Y" 0 t)
        (format-time-string "%_-5Y" 0 t)
        (format-time-string "%-05Y" 0 t)
        (format-time-string "%0-5Y" 0 t)))

(audit-check "time:invalid-alternative-modifiers"
 '(list (format-time-string "%-5Ea" 0 t)
        (format-time-string "%-5EQ" 0 t)
        (format-time-string "%_-5Oa" 0 t)))

(audit-check "time:uppercase-control"
 '(format-time-string "%^B" 0 t))

(princ "AUDIT-DONE\n")
