;;; -*- lexical-binding: t; -*-
(require 'timer)

(defun audit-with-timers (fn)
  (let ((timer-list nil) (timer-idle-list nil))
    (unwind-protect (funcall fn)
      (mapc #'cancel-timer timer-list)
      (mapc #'cancel-timer timer-idle-list))))
;; 'timer:zero-sleep'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-with-timers (lambda nil (let ((hits 0)) (run-at-time 0 nil (lambda nil (setq hits (1+ hits)))) (sleep-for 0) hits))) t))
           (error (list 'error err))))
  (terpri))

;; 'timer:negative-sleep'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-with-timers (lambda nil (let ((hits 0)) (run-at-time 0 nil (lambda nil (setq hits (1+ hits)))) (sleep-for -1) hits))) t))
           (error (list 'error err))))
  (terpri))

;; 'timer:zero-sit'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-with-timers (lambda nil (let ((hits 0)) (run-at-time 0 nil (lambda nil (setq hits (1+ hits)))) (sit-for 0) hits))) t))
           (error (list 'error err))))
  (terpri))

;; 'timer:positive-sleep-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-with-timers (lambda nil (let ((hits 0)) (run-at-time 0 nil (lambda nil (setq hits (1+ hits)))) (sleep-for 0.03) hits))) t))
           (error (list 'error err))))
  (terpri))
