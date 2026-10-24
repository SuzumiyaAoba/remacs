;;; audit_network_extra.el --- additional network options -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "network:datagram-socket"
 '(let ((p (make-network-process :name "audit-udp" :host "127.0.0.1"
                                 :family 'ipv4 :type 'datagram :server t
                                 :service 0 :noquery t :sentinel #'ignore)))
    (unwind-protect
        (list (process-type p) (process-status p))
      (delete-process p))))

(audit-check "network:random-service"
 '(let ((p (make-network-process :name "audit-random" :host "127.0.0.1"
                                 :family 'ipv4 :server t :service t
                                 :noquery t :sentinel #'ignore)))
    (unwind-protect (integerp (process-contact p :service))
      (delete-process p))))

(audit-check "network:nowait-connect"
 '(let ((server (make-network-process :name "audit-async-server" :host "127.0.0.1"
                                      :server t :service 0 :noquery t :sentinel #'ignore))
        client)
    (unwind-protect
        (progn
          (setq client (make-network-process :name "audit-async-client" :host "127.0.0.1"
                                             :service (process-contact server :service)
                                             :nowait t :noquery t :sentinel #'ignore))
          (process-status client))
      (when client (delete-process client))
      (delete-process server))))

(princ "AUDIT-DONE\n")
