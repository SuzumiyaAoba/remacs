;;; -*- lexical-binding: t; -*-

;; 'process:set-window-size'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((b (generate-new-buffer " *audit-process-size*")) p) (unwind-protect (progn (setq p (make-process :name "audit-process-size" :buffer b :noquery t :sentinel #'ignore :connection-type 'pty :command '("/usr/bin/python3" "-u" "-c" "import os, sys; sys.stdin.readline(); print(tuple(os.get_terminal_size(0)), flush=True)"))) (set-process-window-size p 12 34) (process-send-string p "\n") (let ((attempt 0)) (while (and (process-live-p p) (< attempt 200)) (setq attempt (1+ attempt)) (accept-process-output p 0.01))) (accept-process-output p 0.01) (with-current-buffer b (string-trim (buffer-string)))) (when p (ignore-errors (delete-process p))) (kill-buffer b))) t))
           (error (list 'error err))))
  (terpri))

;; 'process:set-window-size-invalid'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(set-process-window-size 'not-a-process 12 34) t))
           (error (list 'error err))))
  (terpri))
