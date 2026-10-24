;;; audit_process_window.el --- PTY window resize -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "process:set-window-size"
 '(let ((b (generate-new-buffer " *audit-process-size*")) p)
    (unwind-protect
        (progn
          (setq p (make-process
                   :name "audit-process-size" :buffer b :noquery t :sentinel #'ignore
                   :connection-type 'pty
                   :command '("/usr/bin/python3" "-u" "-c"
                              "import os, sys; sys.stdin.readline(); print(tuple(os.get_terminal_size(0)), flush=True)")))
          (set-process-window-size p 12 34)
          (process-send-string p "\n")
          (let ((attempt 0))
            (while (and (process-live-p p) (< attempt 200))
              (setq attempt (1+ attempt)) (accept-process-output p 0.01)))
          (accept-process-output p 0.01)
          (with-current-buffer b (string-trim (buffer-string))))
      (when p (ignore-errors (delete-process p)))
      (kill-buffer b))))

(audit-check "process:set-window-size-invalid"
 '(set-process-window-size 'not-a-process 12 34))

(princ "AUDIT-DONE\n")
