;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar audit-thread-ready nil)
(defvar audit-thread-special 'global)
;; "timer:accept-process-output-inhibit"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((timer-list nil) (hits 0) (p (make-pipe-process :name "audit-timer-inhibit" :noquery t))) (unwind-protect (progn (run-at-time 0 nil (lambda nil (setq hits (1+ hits)))) (accept-process-output p 0.04 nil 1) hits) (mapc #'cancel-timer timer-list) (delete-process p))) t))
           (error (list 'error err))))
  (terpri))
