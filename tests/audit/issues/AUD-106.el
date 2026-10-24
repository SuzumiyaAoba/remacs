;;; -*- lexical-binding: t; -*-
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)
;; 'eval-region:read-function'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((reads 0)) (with-temp-buffer (insert "(+ 1 2)") (eval-region (point-min) (point-max) nil (lambda (stream) (setq reads (1+ reads)) (read stream))) (> reads 0))) t))
           (error (list 'error err))))
  (terpri))
