;;; audit_compile_wait.el --- compilation test waiting semantics -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)
(require 'compile)

(defun audit-compile-wait (command wait-for-exit explicit-shell)
  (let ((shell-file-name (if explicit-shell "/bin/sh" shell-file-name))
        (shell-command-switch (if explicit-shell "-c" shell-command-switch))
        (compilation-ask-about-save nil) p)
    (unwind-protect
        (progn
          (compile command)
          (setq p (get-buffer-process "*compilation*"))
          (if wait-for-exit
              (let ((attempts 0))
                (while (and (process-live-p p) (< attempts 100))
                  (accept-process-output p 0.05)
                  (setq attempts (1+ attempts))))
            (accept-process-output p 2))
          (let ((before (process-status p)))
            (delete-process p)
            (delete-process p)
            (list before (process-status p)
                  (with-current-buffer "*compilation*"
                    (and (string-match-p "hello-out" (buffer-string)) t))
                  (with-current-buffer "*compilation*"
                    (and (string-match-p "Compilation finished" (buffer-string)) t)))))
      (when (and p (process-live-p p)) (delete-process p)))))

(audit-check "compile-wait:original-default-shell"
 '(audit-compile-wait "echo hello-out" nil nil))
(audit-check "compile-wait:original-sh-control"
 '(audit-compile-wait "echo hello-out" nil t))
(audit-check "compile-wait:delayed-exit"
 '(audit-compile-wait "printf hello-out; sleep 0.25" nil t))
(audit-check "compile-wait:exit-loop-control"
 '(audit-compile-wait "printf hello-out; sleep 0.25" t t))

(princ "AUDIT-DONE\n")
