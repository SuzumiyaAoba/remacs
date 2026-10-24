;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar audit-thread-ready nil)
(defvar audit-thread-special 'global)
;; "thread:parent-worker-coordination"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (setq audit-thread-ready nil) (let ((worker (make-thread (lambda nil (let ((n 0)) (while (and (not audit-thread-ready) (< n 200)) (setq n (1+ n)) (sleep-for 0.005)) (if audit-thread-ready 'ready 'timed-out)))))) (setq audit-thread-ready t) (thread-join worker))) t))
           (error (list 'error err))))
  (terpri))
