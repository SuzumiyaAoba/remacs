;;; audit_sweep_followup.el --- isolate whole-sweep candidates -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "macroexpand:atom-unused-environment"
 '(macroexpand "x" "bad"))

(audit-check "macroexpand:single-atom-unused-environment"
 '(macroexpand-1 "x" "bad"))

(audit-check "macroexpand:all-atom-unused-environment"
 '(macroexpand-all "x" "bad"))

(audit-check "macroexpand:bad-environment-used-control"
 '(condition-case err (macroexpand '(when t 1) 5)
    (error (car err))))

(audit-check "scroll:single-window-other-down"
 '(save-window-excursion
    (delete-other-windows)
    (condition-case err (scroll-other-window-down)
      (error err))))

(audit-check "scroll:single-window-other-up"
 '(save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (dotimes (n 100) (insert (format "line %d\n" n)))
      (goto-char 1)
      (condition-case err (progn (scroll-other-window) (list 'scrolled (point)))
        (error err)))))

(audit-check "scroll:other-window-selection"
 '(save-window-excursion
    (delete-other-windows)
    (condition-case err (eq (other-window-for-scrolling) (selected-window))
      (error err))))

(audit-check "keymap:control-home"
 '(list (key-binding [C-home]) (key-binding [C-end])))

(audit-check "probe:bury-unbury-context"
 '(with-temp-buffer
    (insert "xyz")
    (let ((original (current-buffer)))
      (bury-buffer)
      (let ((same-after-bury (eq (current-buffer) original)))
        (unbury-buffer)
        (list same-after-bury (eq (current-buffer) original) (buffer-string))))))

(princ "AUDIT-DONE\n")
