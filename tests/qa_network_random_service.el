;;; -*- lexical-binding: nil -*-
;; Issue #178: :service t chooses an ephemeral Internet server port.
;; GNU Emacs 31.1 oracle. All sockets are owned loopback listeners.
(setq text-quoting-style 'grave)
(defvar qa178-owned nil)
(defun qa178-server (service server)
  (let ((p (make-network-process :name "qa178-server" :host "127.0.0.1"
                                :family 'ipv4 :server server :service service
                                :noquery t :sentinel #'ignore)))
    (push p qa178-owned)
    p))
(unwind-protect
    (progn
      ;; t, zero, and the numeric string all ask for a fresh kernel port.
      ;; Integer :server values supply the backlog; zero is non-nil too.
      (dolist (service '(t 0 "0"))
        (dolist (server '(t 5 0))
          (let* ((p (qa178-server service server))
                 (port (process-contact p :service)))
            (qa (integerp port) t)
            (qa (and (> port 0) (<= port 65535)) t)
            (qa (process-type p) 'network)
            (qa (process-status p) 'listen)
            (qa (process-query-on-exit-flag p) nil)
            (qa (process-contact p) (list "127.0.0.1" port))
            (qa (plist-get (process-contact p t) :service) port)
            (qa (process-contact p :local) (vector 127 0 0 1 port))
            (qa (process-contact p :server) server))))
      ;; Keep both open so ephemeral allocation must produce distinct ports.
      (let ((one (qa178-server t t)) (two (qa178-server t t)))
        (qa (= (process-contact one :service) (process-contact two :service)) nil))
      ;; The special value does not become a local socket pathname.
      (qa (condition-case err
              (make-network-process :name "qa178-local-error" :family 'local
                                    :server t :service t)
            (error err))
          '(wrong-type-argument stringp t))
      (qa (condition-case err
              (make-network-process :name "qa178-nowait-error" :host "127.0.0.1"
                                    :family 'ipv4 :server t :service t :nowait t)
            (error err))
          '(error "`:server' is incompatible with `:nowait'"))
      (dolist (service '(nil bogus))
        (qa (condition-case err
                (make-network-process :name "qa178-service-error" :host "127.0.0.1"
                                      :family 'ipv4 :server t :service service)
              (error err))
            (list 'wrong-type-argument 'stringp service))))
  (dolist (p qa178-owned) (delete-process p)))
(princ (qa--done)) (terpri)
