;;; -*- lexical-binding: t; -*-

;; "network:random-service"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((p (make-network-process :name "audit-random" :host "127.0.0.1" :family 'ipv4 :server t :service t :noquery t :sentinel #'ignore))) (unwind-protect (integerp (process-contact p :service)) (delete-process p))) t))
           (error (list 'error err))))
  (terpri))
