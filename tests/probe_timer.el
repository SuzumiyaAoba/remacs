;;; probe_timer.el — timer.el cluster vs GNU semantics. -*- lexical-binding: t -*-
;; Self-asserting: signals on mismatch.

;; Structure
(cl-assert (timerp (timer-create)))
(cl-assert (= (length (timer-create)) 10))
(cl-assert (not (timerp 5)))
(cl-assert (not (timerp [1 2 3])))
(cl-assert (timer--triggered (timer-create)))
(let ((tm (timer-create)))
  (setf (timer--usecs tm) 42)
  (cl-assert (= (timer--usecs tm) 42))
  (setf (timer--time tm) '(1 2 3 4))
  (cl-assert (equal (timer--time tm) '(1 2 3 4)))
  (timer-set-function tm #'ignore '(a b))
  (cl-assert (eq (timer--function tm) 'ignore))
  (cl-assert (equal (timer--args tm) '(a b))))

;; Firing during sleep-for (GNU timer_check semantics)
(let ((fired nil))
  (let ((tm (run-with-timer 0.02 nil (lambda () (setq fired t)))))
    (cl-assert (memq tm timer-list))
    (sleep-for 0.15)
    (cl-assert fired)
    ;; GNU marks triggered when dispatching; one-shot leaves list.
    (cl-assert (timer--triggered tm))
    (cl-assert (not (memq tm timer-list)))))

;; Repeat timers keep firing
(let ((n 0))
  (let ((tm (run-with-timer 0.02 0.02 (lambda () (setq n (1+ n))))))
    (sleep-for 0.15)
    (cl-assert (> n 1))
    (cl-assert (memq tm timer-list))
    (cancel-timer tm)
    (cl-assert (not (memq tm timer-list)))
    (setq n 0)
    (sleep-for 0.1)
    (cl-assert (= n 0))))

;; Ordering: earlier timer fires first
(let ((order nil))
  (run-with-timer 0.08 nil (lambda () (push 'b order)))
  (run-with-timer 0.01 nil (lambda () (push 'a order)))
  (sleep-for 0.2)
  (cl-assert (equal order '(b a))))

;; sit-for fires timers
(let ((f nil))
  (run-with-timer 0.01 nil (lambda () (setq f t)))
  (sit-for 0.1)
  (cl-assert f))

;; with-timeout
(cl-assert (= (with-timeout (1 'timed-out) 99) 99))
(cl-assert (eq (with-timeout (0.02 'timed-out) (sleep-for 5) 99) 'timed-out))
(cl-assert (= (with-timeout (5 'to) 1 2 3) 3))

;; Idle timers never fire in batch; current-idle-time is nil
(let ((f nil))
  (run-with-idle-timer 0.01 t (lambda () (setq f t)))
  (sleep-for 0.1)
  (cl-assert (not f))
  (cl-assert (null (current-idle-time))))

;; timer-event-last tracking
(cl-assert (timerp timer-event-last))

;; run-at-time argument forms
(cl-assert (timerp (run-at-time nil nil #'ignore)))
(cl-assert (timerp (run-at-time 100 nil #'ignore)))
(cl-assert (timerp (run-at-time "2 hours" nil #'ignore)))

;; Errors match GNU
(cl-assert (eq (condition-case nil (progn (run-at-time nil -1 #'ignore) nil)
                 (error 'error))
               'error))
(cl-assert (equal (condition-case e (progn (timer-activate 5) nil)
                    (error (cadr e)))
                  "Invalid or uninitialized timer"))
(cl-assert (equal (condition-case e (progn (timer-activate (timer-create)) nil)
                    (error (cadr e)))
                  "Invalid or uninitialized timer"))

;; timer-duration parsing
(cl-assert (= (timer-duration "1 hour 30 min") 5400))
(cl-assert (= (timer-duration "2.5") 2.5))
(cl-assert (= (timer-duration "5 milliseconds") 0.005))
(cl-assert (= (timer-duration "3 weeks") 1814400))
(cl-assert (= (timer-duration "") 0))
(cl-assert (null (timer-duration "bogus")))
(cl-assert (null (timer-duration "abc")))

;; decoded-time accessors + setters
(let ((d (list 1 2 3 4 5 6 7 -1 0)))
  (cl-assert (= (decoded-time-second d) 1))
  (cl-assert (= (decoded-time-zone d) 0))
  (setf (decoded-time-hour d) 9)
  (cl-assert (= (decoded-time-hour d) 9)))

;; timer-event-handler on a canceled timer is a no-op
(let ((tm (run-with-timer 0.01 nil #'ignore)))
  (cancel-timer tm)
  (timer-event-handler tm)
  (cl-assert (not (memq tm timer-list))))

;; timeout-event-p
(cl-assert (timeout-event-p '(timer-event x)))
(cl-assert (not (timeout-event-p 'x)))

;; idle list population
(let ((tm (timer-create)))
  (timer-set-function tm #'ignore)
  (timer-set-idle-time tm 5 t)
  (timer-activate-when-idle tm t)
  (cl-assert (memq tm timer-idle-list))
  (cancel-timer tm)
  (cl-assert (not (memq tm timer-idle-list))))

;; add-timeout / disable-timeout aliases
(cl-assert (fboundp 'add-timeout))
(cl-assert (fboundp 'disable-timeout))

(princ "timer-ok\n")
