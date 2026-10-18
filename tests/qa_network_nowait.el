;;; -*- lexical-binding: nil -*-
;; Issue #179: asynchronous TCP connection lifecycle, GNU Emacs 31.1 oracle.
;; Every socket is owned by this fixture and bound only to loopback. The
;; native companion tests cover a genuinely pending connect and refusal.
(setq text-quoting-style 'grave)
(defvar qa179-owned nil)
(defvar qa179-original-processes (process-list))
(defun qa179-remember (p) (push p qa179-owned) p)
(defun qa179-record (p event)
  (process-put p 'events (append (process-get p 'events) (list event))))
(defun qa179-sentinel (p event)
  (qa179-record p (list 'sentinel event (process-status p)
                       (and (process-live-p p) t))))
(defun qa179-filter (p text)
  (qa179-record p (list 'filter (process-status p)))
  (process-put p 'received (concat (process-get p 'received) text)))
(defun qa179-echo (p text) (process-send-string p text))
(defun qa179-server (&rest options)
  (qa179-remember
   (apply #'make-network-process
          (append options
                  '(:name "qa179-server" :host "127.0.0.1" :family ipv4
                    :server t :service 0 :noquery t :coding binary
                    :filter qa179-echo :sentinel ignore)))))
(defun qa179-client (server &rest options)
  (qa179-remember
   (apply #'make-network-process
          (append options
                  (list :name "qa179-client" :host "127.0.0.1" :family 'ipv4
                        :service (process-contact server :service) :nowait t
                        :coding 'binary :noquery t :sentinel #'qa179-sentinel
                        :filter #'qa179-filter)))))
(defun qa179-wait (predicate)
  (let ((deadline (+ (float-time) 3)))
    (while (and (not (funcall predicate)) (< (float-time) deadline))
      (accept-process-output nil 0.01))))
(defun qa179-pump ()
  (let ((deadline (+ (float-time) 0.08)))
    (while (< (float-time) deadline) (accept-process-output nil 0.01))))
(unwind-protect
    (progn
      ;; Even immediate loopback completion is initially reported as connect.
      ;; The open sentinel must run before echoed bytes reach the filter.
      (let* ((s (qa179-server)) (c (qa179-client s)))
        (qa (process-status c) 'connect)
        (qa (process-get c 'events) nil)
        (qa (process-type c) 'network)
        (qa (and (process-live-p c) t) t)
        (qa (and (memq c (process-list)) t) t)
        (qa (process-query-on-exit-flag c) nil)
        (qa (process-contact c :nowait) t)
        (qa179-wait (lambda () (eq (process-status c) 'open)))
        (process-send-string c "greeting:")
        (qa179-wait (lambda () (equal (process-get c 'received) "greeting:")))
        (qa (process-status c) 'open)
        (qa (process-exit-status c) 0)
        (qa (process-get c 'received) "greeting:")
        (qa (car (process-get c 'events)) '(sentinel "open\n" open t))
        (qa (car (cdr (process-get c 'events))) '(filter open))
        (qa (process-send-string c "payload") nil)
        (qa179-wait (lambda () (equal (process-get c 'received) "greeting:payload")))
        (qa (process-get c 'received) "greeting:payload")
        (qa179-pump)
        (qa (length (delq nil (mapcar (lambda (event)
                                      (and (eq (car event) 'sentinel) event))
                                    (process-get c 'events)))) 1))
      ;; Sending before completion waits while dispatching the open sentinel,
      ;; then sends the bytes; merely queueing or rejecting the send is wrong.
      (let* ((s (qa179-server)) (c (qa179-client s)))
        (qa (process-status c) 'connect)
        (qa (process-send-string c "early") nil)
        (qa (process-status c) 'open)
        (qa (car (process-get c 'events)) '(sentinel "open\n" open t))
        (qa179-wait (lambda () (equal (process-get c 'received) "early")))
        (qa (process-get c 'received) "early"))
      ;; Deleting before the first event-loop poll suppresses completion and
      ;; removes the process immediately, even with a live Lisp reference.
      (let* ((s (qa179-server)) (c (qa179-client s)))
        (qa (process-status c) 'connect)
        (qa (delete-process c) nil)
        (qa (process-status c) 'closed)
        (qa (process-live-p c) nil)
        (qa (process-exit-status c) 0)
        (qa (get-process (process-name c)) nil)
        (qa (process-get c 'events) '((sentinel "deleted\n" closed nil)))
        (qa179-pump)
        (qa (process-get c 'events) '((sentinel "deleted\n" closed nil)))
        (qa (process-get c 'received) nil)
        (delete-process c)
        (qa (process-get c 'events) '((sentinel "deleted\n" closed nil))))
      ;; An open sentinel may send and replace its filter, then reenter the
      ;; event loop. It runs exactly once and never observes connect status.
      (let* ((s (qa179-server))
             (c (qa179-client
                 s :sentinel
                 (lambda (p event)
                   (qa179-sentinel p event)
                   (when (equal event "open\n")
                     (set-process-filter p #'qa179-filter)
                     (process-send-string p "reentrant")
                     (accept-process-output p 0.01)
                     (process-put p 'during-open (process-get p 'received)))))))
        (qa179-wait (lambda () (equal (process-get c 'received) "reentrant")))
        (qa (process-get c 'during-open) nil)
        (qa (process-get c 'received) "reentrant")
        (qa (car (process-get c 'events)) '(sentinel "open\n" open t))
        (qa (length (delq nil (mapcar (lambda (event)
                                      (and (eq (car event) 'sentinel) event))
                                    (process-get c 'events)))) 1))
      ;; Replacing the sentinel and deleting from its open callback must
      ;; notify the replacement synchronously and suppress all queued input.
      (let* ((s (qa179-server))
             (c (qa179-client
                 s :sentinel
                 (lambda (p event)
                   (qa179-sentinel p event)
                   (when (equal event "open\n")
                     (set-process-sentinel
                      p (lambda (proc message)
                          (qa179-record proc (list 'replacement message
                                                   (process-status proc)))))
                     (delete-process p)
                     (qa179-record p (list 'resumed (process-status p))))))))
        (qa179-wait (lambda () (eq (process-status c) 'closed)))
        (qa (process-get c 'events)
            '((sentinel "open\n" open t)
              (replacement "deleted\n" closed) (resumed closed)))
        (qa (get-process (process-name c)) nil)
        (qa (process-get c 'received) nil)
        (qa179-pump)
        (qa (process-get c 'received) nil))
      ;; A sentinel can temporarily disable filters without losing TCP bytes.
      (let* ((s (qa179-server))
             (c (qa179-client s :sentinel
                  (lambda (p event)
                    (qa179-sentinel p event)
                    (when (equal event "open\n") (set-process-filter p t))))))
        (qa (process-send-string c "held-by-filter") nil)
        (qa (process-status c) 'open)
        (qa179-pump)
        (qa (process-get c 'events) '((sentinel "open\n" open t)))
        (qa (process-get c 'received) nil)
        (set-process-filter c #'qa179-filter)
        (qa179-wait (lambda () (equal (process-get c 'received) "held-by-filter")))
        (qa (process-get c 'received) "held-by-filter"))
      ;; Stopped input is independent of the asynchronous connection. Sending
      ;; still completes the connection, while filters wait for continue-process.
      (let* ((s (qa179-server)) (c (qa179-client s :stop t)))
        (qa (process-status c) 'stop)
        (qa (process-command c) t)
        (qa (process-send-string c "stopped") nil)
        (qa (process-status c) 'stop)
        (qa (car (process-get c 'events)) '(sentinel "open\n" stop t))
        (qa179-pump)
        (qa (process-get c 'received) nil)
        (qa-same (continue-process c) c)
        (qa (process-status c) 'open)
        (qa (process-command c) nil)
        (qa179-wait (lambda () (equal (process-get c 'received) "stopped")))
        (qa (process-get c 'received) "stopped")
        (qa (length (delq nil (mapcar (lambda (event)
                                      (and (eq (car event) 'sentinel) event))
                                    (process-get c 'events)))) 1))
      ;; Keep IPv6 optional, but never silently fall back to a public host.
      (let ((s (condition-case nil (qa179-server :host "::1" :family 'ipv6)
                 (file-error nil))))
        (when s
          (let ((c (qa179-client s :host "::1" :family 'ipv6)))
            (qa (process-status c) 'connect)
            (qa (process-send-string c "ipv6") nil)
            (qa179-wait (lambda () (equal (process-get c 'received) "ipv6")))
            (qa (process-status c) 'open)
            (qa (process-get c 'received) "ipv6")
            (qa (car (process-get c 'events)) '(sentinel "open\n" open t))))))
  ;; Accepted peers have no application data in some cancellation cases.
  ;; Every process created during this isolated fixture is one of ours.
  (dolist (p (process-list))
    (unless (memq p qa179-original-processes)
      (set-process-sentinel p #'ignore)
      (delete-process p)))
  (dolist (p qa179-owned)
    (set-process-sentinel p #'ignore)
    (delete-process p)))
(princ (qa--done)) (terpri)
