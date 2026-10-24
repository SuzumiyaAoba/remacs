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
;; "process:send-region-marker"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (let ((p (make-pipe-process :name "audit-region" :noquery t))) (unwind-protect (process-send-region p (copy-marker 1) (copy-marker 3)) (delete-process p)))) t))
           (error (list 'error err))))
  (terpri))

;; "process:send-region-low-bound"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (let ((p (make-pipe-process :name "audit-region" :noquery t))) (unwind-protect (condition-case e (progn (process-send-region p 0 2) 'sent) (error (car e))) (delete-process p)))) t))
           (error (list 'error err))))
  (terpri))

;; "process:send-region-narrowed"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (narrow-to-region 2 3) (let ((p (make-pipe-process :name "audit-region" :noquery t))) (unwind-protect (condition-case e (progn (process-send-region p 1 4) 'sent) (error (car e))) (delete-process p)))) t))
           (error (list 'error err))))
  (terpri))
