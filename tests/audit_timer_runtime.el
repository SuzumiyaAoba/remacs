;;; audit_timer_runtime.el --- timer dispatch and wait boundaries -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(require 'timer)

(defun audit-with-timers (fn)
  (let ((timer-list nil) (timer-idle-list nil))
    (unwind-protect (funcall fn)
      (mapc #'cancel-timer timer-list)
      (mapc #'cancel-timer timer-idle-list))))

(audit-check "timer:match-data-control"
 '(audit-with-timers
   (lambda ()
     (let ((hits 0))
       (run-at-time 0 nil (lambda () (setq hits (1+ hits)) (string-match "b" "b")))
       (string-match "\\(a\\)" "a")
       (sleep-for 0.03)
       (list hits (match-string 1 "a"))))))

(audit-check "timer:zero-sleep"
 '(audit-with-timers
   (lambda ()
     (let ((hits 0))
       (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
       (sleep-for 0)
       hits))))

(audit-check "timer:negative-sleep"
 '(audit-with-timers
   (lambda ()
     (let ((hits 0))
       (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
       (sleep-for -1)
       hits))))

(audit-check "timer:zero-sit"
 '(audit-with-timers
   (lambda ()
     (let ((hits 0))
       (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
       (sit-for 0)
       hits))))

(audit-check "timer:positive-sleep-control"
 '(audit-with-timers
   (lambda ()
     (let ((hits 0))
       (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
       (sleep-for 0.03)
       hits))))

(audit-check "timer:current-buffer-control"
 '(audit-with-timers
   (lambda ()
     (with-temp-buffer
       (let ((original (current-buffer))
             (other (generate-new-buffer " *audit-timer-buffer*"))
             (hits 0))
         (unwind-protect
             (progn
               (run-at-time 0 nil (lambda () (setq hits (1+ hits)) (set-buffer other)))
               (sleep-for 0.03)
               (list hits (eq (current-buffer) original)))
           (kill-buffer other)))))))

(audit-check "timer:inhibit-quit-control"
 '(audit-with-timers
   (lambda ()
     (let ((observed 'not-run))
       (run-at-time 0 nil (lambda () (setq observed inhibit-quit)))
       (sleep-for 0.03)
       observed))))

(princ "AUDIT-DONE\n")
