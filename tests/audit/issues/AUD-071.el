;;; -*- lexical-binding: t; -*-

;; "network:nowait-connect"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((server (make-network-process :name "audit-async-server" :host "127.0.0.1" :server t :service 0 :noquery t :sentinel #'ignore)) client) (unwind-protect (progn (setq client (make-network-process :name "audit-async-client" :host "127.0.0.1" :service (process-contact server :service) :nowait t :noquery t :sentinel #'ignore)) (process-status client)) (when client (delete-process client)) (delete-process server))) t))
           (error (list 'error err))))
  (terpri))
