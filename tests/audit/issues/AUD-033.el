;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar audit-thread-ready nil)
(defvar audit-thread-special 'global)
;; "thread:current-buffer-isolation"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((a (generate-new-buffer " *audit-thread-a*")) (b (generate-new-buffer " *audit-thread-b*"))) (unwind-protect (save-current-buffer (set-buffer a) (let ((worker (make-thread (lambda nil (set-buffer b))))) (thread-join worker) (eq (current-buffer) a))) (kill-buffer a) (kill-buffer b))) t))
           (error (list 'error err))))
  (terpri))

;; "thread:dynamic-bindings"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((audit-thread-special 'parent)) (thread-join (make-thread (lambda nil (symbol-value 'audit-thread-special))))) t))
           (error (list 'error err))))
  (terpri))
