;;; audit_network.el --- bounded loopback network lifecycle probes -*- lexical-binding: t; -*-
;; Uses only transient test listeners and loopback clients; all are deleted.
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(defun audit-network-check (id form)
  (when (or (not (getenv "REMACS_AUDIT_ONLY"))
            (equal id (getenv "REMACS_AUDIT_ONLY")))
    (message "NETWORK|%s" id))
  (audit-check id form))

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

(audit-network-check "network:pair-control"
 '(audit-net-with-pair nil
    (lambda (server client peer)
      (list (process-status server) (process-status client) (process-status peer)))))

(audit-network-check "network:send-eof"
 '(audit-net-with-pair nil
    (lambda (_server client peer)
      (process-send-eof client)
      (let ((n 0))
        (while (and (process-live-p peer) (< n 30))
          (setq n (1+ n)) (accept-process-output nil 0.01)))
      (process-status peer))))

(audit-network-check "network:deleted-listener-rebind"
 '(let* ((server (audit-net-server)) (port (process-contact server :service)) second)
    (unwind-protect
        (progn (delete-process server)
               (setq second (audit-net-server (list :service port)))
               'bound)
      (when second (ignore-errors (delete-process second)))
      (ignore-errors (delete-process server)))))

(audit-network-check "network:deleted-local-listener-unlink"
 '(let* ((dir (make-temp-file "remacs-audit-socket-" t))
         (path (expand-file-name "socket" dir)) server)
    (unwind-protect
        (progn
          (setq server (make-network-process :name "audit-local" :family 'local
                                             :server t :service path :noquery t :sentinel #'ignore))
          (delete-process server)
          (file-exists-p path))
      (when server (ignore-errors (delete-process server)))
      (delete-directory dir t))))

(audit-network-check "network:accept-log-callback"
 '(let ((called nil))
    (audit-net-with-pair
     (list :log (lambda (_server _client _message) (setq called t)))
     (lambda (_server _client _peer) called))))

(audit-network-check "network:accept-sentinel-target"
 '(let (notified)
    (audit-net-with-pair
     (list :sentinel (lambda (p _message) (push (process-status p) notified)))
     (lambda (_server _client _peer) (nreverse notified)))))

(audit-network-check "network:accepted-noquery-inheritance"
 '(audit-net-with-pair '(:noquery t)
    (lambda (server _client peer)
      (list (process-query-on-exit-flag server) (process-query-on-exit-flag peer)))))

(audit-network-check "network:accepted-coding-inheritance"
 '(audit-net-with-pair '(:coding binary)
    (lambda (server _client peer)
      (equal (process-coding-system server) (process-coding-system peer)))))

(audit-network-check "network:accepted-plist-control"
 '(audit-net-with-pair '(:plist (audit-property 42))
    (lambda (server _client peer) (equal (process-plist server) (process-plist peer)))))

(audit-network-check "network:accepted-contact"
 '(audit-net-with-pair nil
    (lambda (_server _client peer)
      (let ((contact (process-contact peer)))
        (list (stringp (car contact)) (integerp (cadr contact))
              (vectorp (process-contact peer :remote))
              (vectorp (process-contact peer :local)))))))

(audit-network-check "network:resolved-remote-address"
 '(let ((server (audit-net-server)) client)
    (unwind-protect
        (progn
          (setq client (make-network-process :name "audit-dns-client" :host "localhost"
                                             :family 'ipv4 :service (process-contact server :service)
                                             :noquery t :sentinel #'ignore))
          (let ((remote (process-contact client :remote)))
            (list (aref remote 0) (aref remote 1) (aref remote 2) (aref remote 3))))
      (when client (ignore-errors (delete-process client)))
      (ignore-errors (delete-process server)))))

(audit-network-check "network:stop-reading"
 '(audit-net-with-pair nil
    (lambda (_server client peer)
      (let ((text ""))
        (set-process-filter peer (lambda (_p chunk) (setq text (concat text chunk))))
        (stop-process peer)
        (process-send-string client "payload")
        (accept-process-output nil 0.05)
        (let ((before text))
          (continue-process peer) (accept-process-output nil 0.05)
          (list before text))))))

(princ "AUDIT-DONE\n")
