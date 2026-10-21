;;; -*- lexical-binding: t; -*-
(defun qa217-filter (process text)
  (process-put process 'text (concat (process-get process 'text) text)))

(defun qa217-pair (local function)
  (let ((before (process-list)) server client peer
        (path (and local (make-temp-name (expand-file-name "qa217-" temporary-file-directory)))))
    (unwind-protect
        (progn
          (setq server (apply #'make-network-process
                              (append (list :name "qa217-server" :server t :coding 'binary
                                            :noquery t :filter #'qa217-filter :sentinel #'ignore)
                                      (if local (list :family 'local :service path)
                                        '(:host "127.0.0.1" :service 0)))))
          (setq client (apply #'make-network-process
                              (append (list :name "qa217-client" :coding 'binary :noquery t
                                            :filter #'qa217-filter :sentinel #'ignore)
                                      (if local (list :family 'local :service path)
                                        (list :host "127.0.0.1" :service (process-contact server :service))))))
          (let ((deadline (+ (float-time) 3)))
            (while (and (not peer) (< (float-time) deadline))
              (accept-process-output nil 0.01)
              (dolist (process (process-list))
                (when (and (not (memq process before)) (not (eq process server)) (not (eq process client)))
                  (setq peer process)))))
          (unless peer (error "No accepted EOF-test peer"))
          (funcall function server client peer))
      (dolist (process (process-list))
        (unless (memq process before) (ignore-errors (delete-process process))))
      (when (and path (file-exists-p path)) (delete-file path)))))

(defun qa217-closed (peer)
  (let ((deadline (+ (float-time) 3)))
    (while (and (process-live-p peer) (< (float-time) deadline))
      (accept-process-output nil 0.01)))
  (process-status peer))

(defun qa217-text (process expected)
  (let ((deadline (+ (float-time) 3)))
    (while (and (not (equal (process-get process 'text) expected)) (< (float-time) deadline))
      (accept-process-output process 0.01 nil t)))
  (process-get process 'text))

(defun qa217-send (_server client peer)
  (process-send-string client "BEFORE")
  (let ((first (eq (process-send-eof client) client))
        (status (process-status client)))
    (process-send-string client "DISCARDED")
    (list first status (eq (process-send-eof client) client)
          (qa217-closed peer) (process-get peer 'text))))

(defun qa217-receive (_server client peer)
  (process-send-eof client)
  (process-send-string peer "REPLY")
  (list (qa217-text client "REPLY") (process-status client) (process-status peer)))

(defun qa217-return (_server client _peer)
  (with-temp-buffer
    (set-process-buffer client (current-buffer))
    (let ((name (process-name client)) (buffer (current-buffer)))
      (list (eq (process-send-eof name) name) (eq (process-send-eof buffer) buffer)
            (process-send-eof nil) (process-send-eof)))))

(defun qa217-iso (_server client peer)
  (set-process-coding-system client 'binary 'iso-2022-jp)
  (process-send-string client "あ")
  (process-send-eof client)
  (process-send-string client "DISCARDED")
  (list (qa217-closed peer) (string-to-list (process-get peer 'text))))

(defun qa217-empty (_server client peer)
  (set-process-coding-system client 'binary 'utf-16le-with-signature)
  (process-send-eof client)
  (list (qa217-closed peer) (process-get peer 'text)))

(defun qa217-listener (server _client _peer)
  (list (condition-case err (process-send-eof server) (error err)) (process-status server)))

(qa (qa217-pair nil #'qa217-send) '(t open t closed "BEFORE"))
(qa (qa217-pair t #'qa217-send) '(t open t closed "BEFORE"))
(qa (qa217-pair nil #'qa217-receive) '("REPLY" open open))
(qa (qa217-pair t #'qa217-receive) '("REPLY" open open))
(qa (qa217-pair nil #'qa217-return) '(t t nil nil))
(qa (qa217-pair t #'qa217-return) '(t t nil nil))
(qa (qa217-pair nil #'qa217-iso) '(closed (27 36 66 36 34)))
(qa (qa217-pair t #'qa217-iso) '(closed (27 36 66 36 34)))
(qa (qa217-pair nil #'qa217-empty) '(closed nil))
(qa (qa217-pair t #'qa217-empty) '(closed nil))
(qa (qa217-pair nil #'qa217-listener)
    '((error "Process qa217-server not running: listen") listen))
(qa (qa217-pair t #'qa217-listener)
    '((error "Process qa217-server not running: listen") listen))
(qa (let (server client)
      (unwind-protect
          (progn
            (setq server (make-network-process :name "qa217-udp-server" :type 'datagram
                                              :host "127.0.0.1" :server t :service 0 :coding 'binary
                                              :noquery t :filter #'qa217-filter :sentinel #'ignore))
            (setq client (make-network-process :name "qa217-udp-client" :type 'datagram
                                              :host "127.0.0.1" :service (process-contact server :service)
                                              :coding 'binary :noquery t :filter #'ignore :sentinel #'ignore))
            (let ((same (eq (process-send-eof client) client)))
              (process-send-string client "AFTER")
              (list same (process-status client) (qa217-text server "AFTER"))))
        (when client (delete-process client))
        (when server (delete-process server))))
    '(t open "AFTER"))
(princ (qa--done)) (terpri)
