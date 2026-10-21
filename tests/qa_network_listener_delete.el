;;; -*- lexical-binding: t; -*-
(defvar qa218-events nil)
(defvar qa218-replacement nil)
(defvar qa218-local nil)
(defvar qa218-service nil)
(defvar qa218-nested nil)
(defvar qa218-in-sentinel nil)

(defun qa218-filter (process text)
  (process-put process 'text (concat (process-get process 'text) text)))

(defun qa218-server (local service)
  ;; GNU closes local listeners but leaves their filesystem pathname.
  (when (and local (file-exists-p service)) (delete-file service))
  (apply #'make-network-process
         (append (list :name "qa218-server" :server t :noquery t :coding 'binary
                       :filter #'qa218-filter :sentinel #'ignore)
                 (if local (list :family 'local :service service)
                   (list :host "127.0.0.1" :service (or service 0))))))

(defun qa218-client (local service)
  (apply #'make-network-process
         (append (list :name "qa218-client" :noquery t :coding 'binary
                       :filter #'qa218-filter :sentinel #'ignore)
                 (if local (list :family 'local :service service)
                   (list :host "127.0.0.1" :service service)))))

(defun qa218-case (local function)
  (let* ((before (process-list))
         (path (and local (make-temp-name (expand-file-name "qa218-" temporary-file-directory))))
         (server (qa218-server local path)))
    (unwind-protect
        (funcall function server local (process-contact server :service))
      (dolist (process (process-list))
        (unless (memq process before) (ignore-errors (delete-process process))))
      (when (and path (file-exists-p path)) (delete-file path)))))

(defun qa218-peer (before client)
  (let ((deadline (+ (float-time) 3)) peer)
    (while (and (not peer) (< (float-time) deadline))
      (accept-process-output nil 0.01)
      (dolist (process (process-list))
        (when (and (not (memq process before)) (not (eq process client)))
          (setq peer process))))
    (or peer (error "No accepted listener-test peer"))))

(defun qa218-text (process expected)
  (let ((deadline (+ (float-time) 3)))
    (while (and (not (equal (process-get process 'text) expected)) (< (float-time) deadline))
      (accept-process-output process 0.01 nil t)))
  (process-get process 'text))

(defun qa218-roundtrip (local service)
  (let* ((before (process-list)) (client (qa218-client local service))
         (peer (qa218-peer before client)))
    (process-send-string client "PING")
    (let ((request (qa218-text peer "PING")))
      (process-send-string peer "PONG")
      (list request (qa218-text client "PONG")))))

(defun qa218-rebind (server local service)
  (let* ((deleted (delete-process server))
         (second (qa218-server local service)))
    ;; The old process remains referenced throughout, and repeated deletion
    ;; must not close the replacement or remove its Unix socket pathname.
    (delete-process server) (delete-process server)
    (list deleted (process-status server) (process-status second) (eq server second)
          (equal (process-contact server :service) service)
          (qa218-roundtrip local service))))

(defun qa218-sentinel (server event)
  (if qa218-in-sentinel
      (push '(reentered) qa218-events)
    (let ((qa218-in-sentinel t))
      (when qa218-nested (delete-process server))
      (let ((result (condition-case err
                        (progn (setq qa218-replacement (qa218-server qa218-local qa218-service)) 'bound)
                      (error (car err)))))
        (push (list event (process-status server) result) qa218-events)))))

(defun qa218-callback (server local service &optional nested)
  (let ((qa218-events nil) (qa218-replacement nil)
        (qa218-local local) (qa218-service service) (qa218-nested nested))
    (set-process-sentinel server #'qa218-sentinel)
    (delete-process server)
    (unless qa218-replacement (setq qa218-replacement (qa218-server local service)))
    (delete-process server)
    (list (nreverse qa218-events) (process-status server)
          (process-status qa218-replacement) (qa218-roundtrip local service))))

(defun qa218-accepted (server local service)
  (let* ((before (process-list)) (client (qa218-client local service))
         (peer (qa218-peer before client)))
    (delete-process server)
    (let ((second (qa218-server local service)))
      (process-send-string client "PING")
      (let ((request (qa218-text peer "PING")))
        (process-send-string peer "PONG")
        (list (process-status server) (process-status second)
              request (qa218-text client "PONG") (qa218-roundtrip local service))))))

(qa (qa218-case nil #'qa218-rebind) '(nil closed listen nil t ("PING" "PONG")))
(qa (qa218-case t #'qa218-rebind) '(nil closed listen nil t ("PING" "PONG")))
(qa (qa218-case nil #'qa218-callback) '((("deleted\n" closed bound)) closed listen ("PING" "PONG")))
(qa (qa218-case t #'qa218-callback) '((("deleted\n" closed bound)) closed listen ("PING" "PONG")))
(qa (qa218-case nil (lambda (server local service) (qa218-callback server local service t)))
    '((("deleted\n" closed bound)) closed listen ("PING" "PONG")))
(qa (qa218-case t (lambda (server local service) (qa218-callback server local service t)))
    '((("deleted\n" closed bound)) closed listen ("PING" "PONG")))
(qa (qa218-case nil #'qa218-accepted) '(closed listen "PING" "PONG" ("PING" "PONG")))
(qa (qa218-case t #'qa218-accepted) '(closed listen "PING" "PONG" ("PING" "PONG")))
(qa (qa218-case t (lambda (server _local service)
                   (delete-process server)
                   (list (process-status server) (file-exists-p service))))
    '(closed t))
(princ (qa--done)) (terpri)
