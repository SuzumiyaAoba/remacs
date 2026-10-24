;;; audit_undo_behavior.el --- undo protection bindings and synthetic records -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "undo:read-only-text"
 '(with-temp-buffer
    (insert "abc") (put-text-property 1 4 'read-only t)
    (setq buffer-undo-list nil)
    (list (condition-case err (progn (primitive-undo 1 '((1 . 4))) 'undone)
            (error (car err)))
          (buffer-substring-no-properties (point-min) (point-max)))))

(audit-check "undo:read-only-buffer"
 '(with-temp-buffer
    (insert "abc") (setq buffer-undo-list nil buffer-read-only t)
    (list (condition-case err (progn (primitive-undo 1 '((1 . 4))) 'undone)
            (error (car err)))
          (buffer-substring-no-properties (point-min) (point-max)))))

(audit-check "undo:inhibit-read-only-binding"
 '(with-temp-buffer
    (insert "abc") (setq buffer-undo-list nil)
    (let ((seen 'not-called))
      (let ((before-change-functions
             (list (lambda (&rest _args) (setq seen inhibit-read-only)))))
        (primitive-undo 1 '((1 . 4))))
      (list seen (buffer-string)))))

(audit-check "undo:position-only-history"
 '(with-temp-buffer
    (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
    (primitive-undo 1 '(2))
    (list (point) buffer-undo-list)))

(audit-check "undo:apply-disabled-history"
 '(with-temp-buffer
    (setq buffer-undo-list t)
    (primitive-undo 1 '((apply ignore)))
    buffer-undo-list))

(princ "AUDIT-DONE\n")
