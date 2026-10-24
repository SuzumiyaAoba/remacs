;;; -*- lexical-binding: t; -*-

;; 'process:killed-buffer-identity'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((b (generate-new-buffer " *audit-process-dead-buffer*")) (p (make-pipe-process :name "audit-process-dead-buffer" :buffer b :noquery t))) (unwind-protect (progn (kill-buffer b) (list (buffer-live-p b) (eq (process-buffer p) b))) (ignore-errors (delete-process p)) (when (buffer-live-p b) (kill-buffer b)))) t))
           (error (list 'error err))))
  (terpri))
