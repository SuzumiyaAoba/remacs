;; timer.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - timer-create returns a vector-like timer.
;;   - timer-set-time arms it — timer--triggered becomes t.
;;   - run-at-time pushes onto the GLOBAL timer-list.
;;   - cancel-timer removes it.

(require 'timer)

;; == surface ==
(qa (fboundp 'timerp) t)
(qa (fboundp 'timer-create) t)
(qa (fboundp 'run-at-time) t)
(qa (fboundp 'run-with-timer) t)
(qa (fboundp 'run-with-idle-timer) t)
(qa (fboundp 'cancel-timer) t)
(qa (fboundp 'cancel-timer-internal) t)
(qa (fboundp 'cancel-function-timers) t)
(qa (fboundp 'timer-activate) t)
(qa (fboundp 'timer-activate-when-idle) t)
(qa (fboundp 'timer-event-handler) t)
(qa (fboundp 'timer-set-time) t)
(qa (fboundp 'timer-set-function) t)
(qa (fboundp 'timer-set-idle-time) t)
(qa (fboundp 'timer-relative-time) t)
(qa (fboundp 'timer-until) t)
(qa (fboundp 'timer-inc-time) t)
;; NOT fns
(qa (fboundp 'run-with-idle-timer-repeat) nil)
(qa (fboundp 'timer-event-handler-list) nil)
(qa (fboundp 'timer--idle-repeat) nil)
(qa (fboundp 'timer--process) nil)
(qa (fboundp 'timer--high-precision) nil)

;; == timer vars ==
(qa (boundp 'timer-list) t)
(qa (boundp 'timer-idle-list) t)
(qa (boundp 'timer-max-repeats) t)

;; == timer-create returns a timer ==
(qa (let ((tm (timer-create))) (timerp tm)) t)

;; == non-timers ==
(qa (timerp nil) nil)
(qa (timerp '(1 2 3)) nil)
;; == timer--triggered — defaults to t (the timer fires when scheduled) ==
(qa (let ((tm (timer-create))) (timer--triggered tm)) t)


;; == timer-set-time arms it → triggered becomes t ==
(qa (let ((tm (timer-create)))
      (timer-set-time tm '(100 0))
      (list (timer--triggered tm) (timer--time tm)))
    '(t (100 0 0 0)))

;; == timer-set-function ==
(qa (let ((tm (timer-create)))
      (timer-set-function tm #'car)
      (timer--function tm))
    'car)

;; == timer-set-function with args ==
(qa (let ((tm (timer-create)))
      (timer-set-function tm #'car '(1 2 3))
      (list (timer--function tm) (timer--args tm)))
    '(car (1 2 3)))

;; == run-at-time — pushes onto GLOBAL timer-list ==
(qa (let ((orig-tl timer-list))
      (setq timer-list nil)
      (unwind-protect
          (let ((tm (run-at-time 1 nil #'car)))
            (list (timerp tm)
                  (eq (car timer-list) tm)
                  (length timer-list)))
        (setq timer-list orig-tl)))
    '(t t 1))

;; == cancel-timer removes it ==
(qa (let ((orig-tl timer-list))
      (setq timer-list nil)
      (unwind-protect
          (let ((tm (run-at-time 1 nil #'car)))
            (cancel-timer tm)
            timer-list)
        (setq timer-list orig-tl)))
    nil)

;; == run-with-timer — schedules REPEATS ==
(qa (let ((orig-tl timer-list))
      (setq timer-list nil)
      (unwind-protect
          (let ((tm (run-with-timer 1 5 #'car)))
            (list (timerp tm) (timer--repeat-delay tm)))
        (setq timer-list orig-tl)))
    '(t 5))

;; == timer-until — returns a time or number ==
(qa (let ((tm (timer-create)))
      (timer-set-time tm '(100 0))
      ;; returns a time-difference value
      (or (listp (timer-until tm '(50 0 0 0)))
          (numberp (timer-until tm '(50 0 0 0)))))
    t)

;; == timer-max-repeats is a var ==
(qa (boundp 'timer-max-repeats) t)
(qa timer-max-repeats 10)

;; == cancel-function-timers removes all matching ==
(qa (let ((orig-tl timer-list))
      (setq timer-list nil)
      (unwind-protect
          (progn
            (run-at-time 1 nil #'car)
            (run-at-time 2 nil #'cdr)
            (run-at-time 3 nil #'car)
            (let ((before (length timer-list)))
              (cancel-function-timers #'car)
              (list before
                    (length timer-list)
                    (timer--function (car timer-list)))))
        (setq timer-list orig-tl)))
    '(3 1 cdr))

;; == timer-event-handler is a command ==
(qa (fboundp 'timer-event-handler) t)

;; Issue #290: nonpositive waits leave due timers queued.
(defun qa290-with-timers (thunk)
 (let ((timer-list nil) (timer-idle-list nil))
  (unwind-protect (funcall thunk)
   (mapc #'cancel-timer timer-list) (mapc #'cancel-timer timer-idle-list))))
(defun qa290-wait (function arguments)
 (qa290-with-timers
  (lambda ()
   (let ((hits 0))
    (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
    (let ((value (apply function arguments)))
     (list value hits (not (null timer-list))))))))

(qa
(qa290-wait 'sleep-for '(0))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(0.0))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(-1))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(-0.001))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(0 0))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(0 -1))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(1 -1000))
'(nil 0 t)
)

(qa
(qa290-wait 'sleep-for '(-1 1000))
'(nil 0 t)
)

(qa
(qa290-wait 'sit-for '(0))
'(t 0 t)
)

(qa
(qa290-wait 'sit-for '(-1))
'(t 0 t)
)

(qa
(qa290-wait 'sit-for '(0 t))
'(t 0 t)
)

(qa
(qa290-wait 'sleep-for '(0.02))
'(nil 1 nil)
)

(qa
(qa290-wait 'sleep-for '(0 20))
'(nil 1 nil)
)

(qa
(qa290-wait 'sleep-for '(-1 1020))
'(nil 1 nil)
)

(qa
(qa290-wait 'sit-for '(0.02))
'(t 1 nil)
)

(qa
(qa290-with-timers
 (lambda ()
  (let ((hits 0))
   (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
   (sleep-for 0)
   (let ((before hits))
    (sleep-for 0.02) (list before hits (null timer-list))))))
'(0 1 t)
)

(qa
(qa290-wait 'sleep-for '(0.0e+NaN))
'(nil 0 t)
)

(qa
(qa290-wait 'sit-for '(0.0e+NaN))
'(t 0 t)
)

(princ (qa--done))
