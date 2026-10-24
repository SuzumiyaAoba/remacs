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
;; "network:send-eof"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-net-with-pair nil (lambda (_server client peer) (process-send-eof client) (let ((n 0)) (while (and (process-live-p peer) (< n 30)) (setq n (1+ n)) (accept-process-output nil 0.01))) (process-status peer))) t))
           (error (list 'error err))))
  (terpri))
