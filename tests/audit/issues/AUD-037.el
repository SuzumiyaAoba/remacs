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
;; "process:decode-latin1"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-process-with '("/bin/sh" "-c" "printf '\\351'") '(:connection-type pipe :coding iso-latin-1) (lambda (p b) (audit-process-drain p) (with-current-buffer b (string-to-list (buffer-string))))) t))
           (error (list 'error err))))
  (terpri))

;; "process:encode-latin1"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-process-with '("/usr/bin/od" "-An" "-tx1") '(:connection-type pipe :coding iso-latin-1) (lambda (p b) (process-send-string p "é\n") (process-send-eof p) (audit-process-drain p) (with-current-buffer b (string-trim (buffer-string))))) t))
           (error (list 'error err))))
  (terpri))

;; "process:invalid-coding"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((p (make-pipe-process :name "audit-coding" :noquery t))) (unwind-protect (progn (set-process-coding-system p 'audit-no-such-coding 'audit-no-such-coding) 'accepted) (delete-process p))) t))
           (error (list 'error err))))
  (terpri))

;; "network:accepted-coding-inheritance"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-net-with-pair '(:coding binary) (lambda (server _client peer) (equal (process-coding-system server) (process-coding-system peer)))) t))
           (error (list 'error err))))
  (terpri))
