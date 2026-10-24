;;; -*- lexical-binding: t; -*-

;; 'process:large-pty-write'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((b (generate-new-buffer " *audit-process-count*")) p) (unwind-protect (let ((payload (apply #'concat (make-list 2000 (concat (make-string 63 120) "\n"))))) (setq p (make-process :name "audit-process-count" :buffer b :noquery t :sentinel #'ignore :connection-type 'pty :coding 'utf-8-unix :command (list "/usr/bin/python3" "-u" "-c" "import sys\nn = 0\nfor line in sys.stdin.buffer:\n p = line.find(b'#END#')\n if p >= 0:\n  print(n + p, flush=True)\n  break\n n += len(line)\n"))) (let ((send-result (condition-case err (progn (process-send-string p payload) 'sent) (error (car err))))) (sleep-for 0.2) (process-send-string p "#END#\n") (let ((attempt 0)) (while (and (process-live-p p) (< attempt 200)) (setq attempt (1+ attempt)) (accept-process-output p 0.01))) (accept-process-output p 0.01) (with-current-buffer b (list send-result (length payload) (string-to-number (buffer-string)))))) (when p (ignore-errors (delete-process p))) (kill-buffer b))) t))
           (error (list 'error err))))
  (terpri))
