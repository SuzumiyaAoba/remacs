;;; -*- lexical-binding: t; -*-

;; "network:datagram-socket"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((p (make-network-process :name "audit-udp" :host "127.0.0.1" :family 'ipv4 :type 'datagram :server t :service 0 :noquery t :sentinel #'ignore))) (unwind-protect (list (process-type p) (process-status p)) (delete-process p))) t))
           (error (list 'error err))))
  (terpri))
