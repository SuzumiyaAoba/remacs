;;; -*- lexical-binding: t; -*-
(defun audit-net-server (&optional options)
  (apply #'make-network-process
         (append options '(:name "audit-net-server" :host "127.0.0.1"
                           :server t :service 0 :noquery t :filter ignore :sentinel ignore))))

(defun audit-net-with-pair (options fn)
  (let ((before (process-list)) server client peer)
    (unwind-protect
        (progn
          (setq server (audit-net-server options))
          (message "NETWORK server port %S" (process-contact server :service))
          (setq client (make-network-process :name "audit-net-client" :host "127.0.0.1"
                                             :service (process-contact server :service)
                                             :noquery t :filter #'ignore :sentinel #'ignore))
          (let ((n 0))
            (while (and (not peer) (< n 50))
              (setq n (1+ n))
              (accept-process-output nil 0.01)
              (dolist (p (process-list))
                (when (and (not (memq p before)) (not (eq p server)) (not (eq p client)))
                  (setq peer p)))))
          (unless peer (error "No accepted peer in audit fixture"))
          (funcall fn server client peer))
      (dolist (p (process-list))
        (unless (memq p before) (ignore-errors (delete-process p)))))))
;; "network:accepted-contact"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-net-with-pair nil (lambda (_server _client peer) (let ((contact (process-contact peer))) (list (stringp (car contact)) (integerp (cadr contact)) (vectorp (process-contact peer :remote)) (vectorp (process-contact peer :local)))))) t))
           (error (list 'error err))))
  (terpri))

;; "network:resolved-remote-address"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((server (audit-net-server)) client) (unwind-protect (progn (setq client (make-network-process :name "audit-dns-client" :host "localhost" :family 'ipv4 :service (process-contact server :service) :noquery t :sentinel #'ignore)) (let ((remote (process-contact client :remote))) (list (aref remote 0) (aref remote 1) (aref remote 2) (aref remote 3)))) (when client (ignore-errors (delete-process client))) (ignore-errors (delete-process server)))) t))
           (error (list 'error err))))
  (terpri))
