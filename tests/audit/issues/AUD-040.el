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
;; "process:working-directory"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((dir (make-temp-file "remacs-audit-cwd-" t)) (default-directory dir)) (unwind-protect (audit-process-with '("/bin/pwd") '(:connection-type pipe) (lambda (p b) (audit-process-drain p) (equal (file-truename (with-current-buffer b (string-trim (buffer-string)))) (directory-file-name (file-truename dir))))) (delete-directory dir))) t))
           (error (list 'error err))))
  (terpri))
