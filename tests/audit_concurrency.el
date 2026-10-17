;;; audit_concurrency.el --- timers, thread context and mutexes -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(require 'timer)
(defvar audit-thread-ready nil)
(defvar audit-thread-special 'global)

(audit-check "thread:parent-worker-coordination"
 '(progn
    (setq audit-thread-ready nil)
    (let ((worker (make-thread
                   (lambda ()
                     (let ((n 0))
                       (while (and (not audit-thread-ready) (< n 200))
                         (setq n (1+ n)) (sleep-for 0.005))
                       (if audit-thread-ready 'ready 'timed-out))))))
      (setq audit-thread-ready t)
      (thread-join worker))))

(audit-check "thread:current-buffer-isolation"
 '(let ((a (generate-new-buffer " *audit-thread-a*"))
        (b (generate-new-buffer " *audit-thread-b*")))
    (unwind-protect
        (save-current-buffer
          (set-buffer a)
          (let ((worker (make-thread (lambda () (set-buffer b)))))
            (thread-join worker)
            (eq (current-buffer) a)))
      (kill-buffer a) (kill-buffer b))))

(audit-check "thread:dynamic-bindings"
 '(let ((audit-thread-special 'parent))
    (thread-join (make-thread (lambda () (symbol-value 'audit-thread-special))))))

(audit-check "thread:invalid-function"
 '(unwind-protect (threadp (make-thread 42)) (thread-last-error t)))

(audit-check "mutex:recursive-depth"
 '(let ((m (make-mutex)))
    (mutex-lock m) (mutex-lock m)
    (unwind-protect
        (list (mutex-unlock m) (mutex-unlock m))
      (ignore-errors (mutex-unlock m)))))

(audit-check "mutex:foreign-unlock"
 '(let ((m (make-mutex)))
    (mutex-lock m)
    (unwind-protect
        (thread-join
         (make-thread
          (lambda () (condition-case e (progn (mutex-unlock m) 'unlocked)
                       (error (car e))))))
      (ignore-errors (mutex-unlock m)))))

(audit-check "mutex:busy-lock"
 '(let ((m (make-mutex)) (entered nil) worker)
    (mutex-lock m)
    (unwind-protect
        (progn
          (setq worker (make-thread
                        (lambda ()
                          (mutex-lock m)
                          (unwind-protect (setq entered t) (mutex-unlock m)))))
          (sleep-for 0.03)
          (let ((before-unlock entered))
            (ignore-errors (mutex-unlock m))
            (thread-join worker)
            (list before-unlock entered)))
      (ignore-errors (mutex-unlock m)))))

(audit-check "condition:wait-without-lock"
 '(condition-wait (make-condition-variable (make-mutex))))

(audit-check "condition:notify-without-lock"
 '(condition-notify (make-condition-variable (make-mutex))))

(audit-check "timer:normal-dispatch"
 '(let ((timer-list nil) (hits 0))
    (unwind-protect
        (progn (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
               (sleep-for 0.04) hits)
      (mapc #'cancel-timer timer-list))))

(audit-check "timer:cancel-other-due"
 '(let ((timer-list nil) (hits nil) later)
    (unwind-protect
        (progn
          (setq later (run-at-time -1 nil (lambda () (push 'later hits))))
          (run-at-time -2 nil (lambda () (push 'first hits) (cancel-timer later)))
          (sleep-for 0.04) hits)
      (mapc #'cancel-timer timer-list))))

(audit-check "timer:accept-process-output"
 '(let ((timer-list nil) (hits 0))
    (unwind-protect
        (progn (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
               (accept-process-output nil 0.04) hits)
      (mapc #'cancel-timer timer-list))))

(audit-check "timer:accept-process-output-inhibit"
 '(let ((timer-list nil) (hits 0)
        (p (make-pipe-process :name "audit-timer-inhibit" :noquery t)))
    (unwind-protect
        (progn (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
               (accept-process-output p 0.04 nil 1) hits)
      (mapc #'cancel-timer timer-list) (delete-process p))))

(princ "AUDIT-DONE\n")
