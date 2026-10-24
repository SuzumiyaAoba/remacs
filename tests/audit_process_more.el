;;; audit_process_more.el --- process lifetime and large writes -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "process:killed-buffer-identity"
 '(let* ((b (generate-new-buffer " *audit-process-dead-buffer*"))
         (p (make-pipe-process :name "audit-process-dead-buffer" :buffer b :noquery t)))
    (unwind-protect
        (progn (kill-buffer b)
               (list (buffer-live-p b) (eq (process-buffer p) b)))
      (ignore-errors (delete-process p))
      (when (buffer-live-p b) (kill-buffer b)))))

(audit-check "process:large-pty-write"
 '(let ((b (generate-new-buffer " *audit-process-count*")) p)
    (unwind-protect
        (let ((payload (apply #'concat (make-list 2000 (concat (make-string 63 ?x) "\n")))))
          (setq p (make-process
                   :name "audit-process-count" :buffer b :noquery t :sentinel #'ignore
                   :connection-type 'pty :coding 'utf-8-unix
                   :command (list "/usr/bin/python3" "-u" "-c"
                     "import sys\nn = 0\nfor line in sys.stdin.buffer:\n p = line.find(b'#END#')\n if p >= 0:\n  print(n + p, flush=True)\n  break\n n += len(line)\n")))
          (let ((send-result (condition-case err (progn (process-send-string p payload) 'sent)
                               (error (car err)))))
            (sleep-for 0.2)
            (process-send-string p "#END#\n")
            (let ((attempt 0))
              (while (and (process-live-p p) (< attempt 200))
                (setq attempt (1+ attempt))
                (accept-process-output p 0.01)))
            (accept-process-output p 0.01)
            (with-current-buffer b
              (list send-result (length payload) (string-to-number (buffer-string))))))
      (when p (ignore-errors (delete-process p)))
      (kill-buffer b))))

(princ "AUDIT-DONE\n")
