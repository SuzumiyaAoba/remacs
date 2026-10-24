;;; -*- lexical-binding: t; -*-
(require 'timer)

(defun audit-process-drain (p)
  (let ((n 0))
    (while (and (process-live-p p) (< n 200))
      (setq n (1+ n)) (accept-process-output p 0.01))
    (accept-process-output p 0.01)))

(defun audit-process-with (command options fn)
  (let ((b (generate-new-buffer " *audit-child-output*")) p)
    (unwind-protect
        (progn
          (setq p (apply #'make-process :name "audit-child" :buffer b
                         :command command :noquery t :sentinel #'ignore options))
          (funcall fn p b))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer b))))
;; "process:filter-t"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-process-with '("/bin/sh" "-c" "printf ok") '(:connection-type pipe :filter t) (lambda (p b) (let ((first (condition-case e (progn (accept-process-output p 0.1) 'ok) (error (car e))))) (set-process-filter p nil) (audit-process-drain p) (list first (with-current-buffer b (buffer-string)))))) t))
           (error (list 'error err))))
  (terpri))
