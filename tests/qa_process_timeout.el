;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar qa209-seconds)

(defun qa209-context (mode seconds &optional flag global)
  (let* ((timer-list nil) (fired nil)
         (directory (make-temp-file "qa209-ready-" t))
         (marker (expand-file-name "ready" directory))
         (buffer (generate-new-buffer " *qa209-output*")) p writer timer)
    (unwind-protect
        (progn
          (if (memq mode '(future repeat))
              (progn
                (setq p (make-process :name "qa209-future" :buffer buffer :noquery t
                                      :connection-type 'pipe :sentinel #'ignore :command '("/bin/cat")))
                (setq timer (run-at-time 0.02 (and (eq mode 'repeat) 60)
                                         (lambda () (setq fired t) (process-send-string p "DATA\n")))))
            (setq p (make-pipe-process :name "qa209-queued" :buffer buffer :noquery t :sentinel #'ignore))
            (unless (memq mode '(dead dead-due))
              (setq writer (make-process :name "qa209-writer" :stderr p :noquery t
                                         :connection-type 'pipe :sentinel #'ignore
                                         :command (list "/bin/sh" "-c"
                                                        (if (eq mode 'exited)
                                                            "read go; printf DATA >&2; : > \"$1\""
                                                          "read go; printf DATA >&2; : > \"$1\"; read done")
                                                        "qa209" marker)))
              ;; Flush earlier child-exit notifications before queuing
              ;; data; the producer then stays alive during the wait.
              (accept-process-output nil 0)
              (process-send-string writer "go\n")
              (let ((deadline (+ (float-time) 3)))
                ;; Prove bytes are queued without dispatching process I/O.
                (while (and (not (file-exists-p marker)) (< (float-time) deadline))
                  (thread-yield))
                (unless (file-exists-p marker) (error "Output was not queued")))
              (when (eq mode 'exited)
                (let ((deadline (+ (float-time) 3)) done)
                  (while (and (not done) (< (float-time) deadline))
                    (let* ((attributes (process-attributes (process-id writer)))
                           (state (cdr (assq 'state attributes))))
                      (setq done (or (null attributes)
                                     (and (stringp state) (string-prefix-p "Z" state)))))
                    (unless done (thread-yield)))
                  (unless done (error "Producer did not exit"))))))
          (when (memq mode '(dead dead-due)) (delete-process p))
          (when (eq mode 'stopped) (stop-process p))
          (when (memq mode '(due dead-due next))
            (setq timer (run-at-time (if (eq mode 'next) 60 0) nil (lambda () (setq fired t)))))
          (let* ((result (condition-case err
                            (accept-process-output (unless global p) seconds nil flag)
                          (error err)))
                 (snapshot (list result fired (with-current-buffer buffer (buffer-string)))))
            ;; Failed select must leave queued output untouched.
            (when (and (process-live-p p) (consp result) (eq (car result) 'file-error))
              (when (eq mode 'stopped) (continue-process p))
              (accept-process-output p 0.1 nil t))
            (append snapshot (list (with-current-buffer buffer (buffer-string))))))
      (when timer (cancel-timer timer))
      (dolist (process (list writer p))
        (when (and process (process-live-p process)) (delete-process process)))
      (kill-buffer buffer) (delete-directory directory t))))

;; A ready descriptor does not bypass the platform's select validation.
(when (eq system-type 'darwin)
  ;; This errno comes from Darwin select's timeout range. Other hosts
  ;; may accept the same timespec, which must remain host-defined.
  (dolist (qa209-seconds '(1e30 1e100 1.0e+INF))
    (qa (qa209-context 'queued qa209-seconds)
        '((file-error "Failed select" "Invalid argument") nil "" "DATA")))
  (qa (qa209-context 'queued 1e30 nil t)
      '((file-error "Failed select" "Invalid argument") nil "" "DATA")))
;; A pending child-exit notification makes GNU select without waiting.
(qa (qa209-context 'exited 1e30) '(t nil "DATA" "DATA"))
;; Due and future one-shot timers run before select fails. Their
;; callbacks do not make the queued bytes disappear into a failed wait.
(when (eq system-type 'darwin)
  (qa (qa209-context 'due 1e30)
      '((file-error "Failed select" "Invalid argument") t "" "DATA"))
  (qa (qa209-context 'future 1e30)
      '((file-error "Failed select" "Invalid argument") t "" "DATA\n")))
;; A pending timer bounds the real select timeout and makes the same
;; enormous user value usable. Repeat timers retain that bound.
(qa (qa209-context 'next 1e30) '(t nil "DATA" "DATA"))
(qa (qa209-context 'repeat 1e30) '(t t "DATA\n" "DATA\n"))
(qa (qa209-context 'next 1.0e+INF) '(t nil "DATA" "DATA"))
;; Fixnum JUST-THIS-ONE inhibits both timer callbacks and their bound.
(when (eq system-type 'darwin)
  (qa (qa209-context 'next 1e30 0)
      '((file-error "Failed select" "Invalid argument") nil "" "DATA"))
  (qa (qa209-context 'due 1e30 1)
      '((file-error "Failed select" "Invalid argument") nil "" "DATA")))
(qa (qa209-context 'next 1e30 t) '(t nil "DATA" "DATA"))
;; Closed targets do not select; due timers are still serviced first.
(qa (qa209-context 'dead 1e30) '(nil nil "" ""))
(qa (qa209-context 'dead-due 1e30) '(nil t "" ""))
;; Listener targets do not select or wait for ordinary process output.
(qa-lex
 (let ((server (make-network-process :name "qa209-listener" :server t :noquery t
                                     :sentinel #'ignore :host "127.0.0.1" :service t)))
   (unwind-protect
       (list (accept-process-output server 1e30) (process-status server))
     (delete-process server)))
 '(nil listen))
(qa-lex
 (let* ((timer-list nil) (fired nil)
        (server (make-network-process :name "qa209-listener-default" :server t :noquery t
                                      :sentinel #'ignore :host "127.0.0.1" :service t))
        (timer (run-at-time 0.02 nil (lambda () (setq fired t) (delete-process server)))))
   (unwind-protect
       (list (accept-process-output server) fired (process-status server))
     (cancel-timer timer) (delete-process server)))
 '(nil nil listen))
;; A stopped connection has a different internal status from a listener
;; or stopped child and still validates an explicit select timeout.
(when (eq system-type 'darwin)
  (qa (qa209-context 'stopped 1e30)
      '((file-error "Failed select" "Invalid argument") nil "" "DATA")))
;; Non-positive/NaN waits poll immediately and can consume ready data.
(dolist (qa209-seconds '(-1e30 -1.0e+INF 0.0e+NaN 0.0))
  (qa (qa209-context 'queued qa209-seconds) '(t nil "DATA" "DATA")))
;; Large but representable select intervals must not be rejected merely
;; because a Rust Instant cannot hold an arbitrary future timestamp.
(dolist (qa209-seconds '(1000000.0 100000000.0 1000000000.0))
  (qa (qa209-context 'queued qa209-seconds) '(t nil "DATA" "DATA")))

;; Preserve GNU's argument contracts and their order. Big integers are
;; rejected as numberp here; the legacy convention requires fixnums.
(qa-err-data (accept-process-output nil (expt 2 100))
             'wrong-type-argument (list 'numberp (expt 2 100)))
(qa-err-data (accept-process-output nil "seconds") 'wrong-type-argument '(numberp "seconds"))
(qa-err-data (accept-process-output nil 1.0 0) 'wrong-type-argument '(fixnump 1.0))
(qa-err-data (accept-process-output nil "seconds" "millis")
             'wrong-type-argument '(fixnump "millis"))
(qa-err-data (accept-process-output nil 1e30 0) 'wrong-type-argument '(fixnump 1e30))
(qa-err-data (accept-process-output nil nil 0.5) 'wrong-type-argument '(fixnump 0.5))
(qa-err-data (accept-process-output nil nil (expt 2 100))
             'wrong-type-argument (list 'fixnump (expt 2 100)))
(qa-err-data (accept-process-output 42 1e30 "millis") 'wrong-type-argument '(processp 42))
(qa (accept-process-output nil 0 0) nil)
(princ (qa--done)) (terpri)
