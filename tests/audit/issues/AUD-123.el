;;; -*- lexical-binding: t; -*-
(defun audit-window-parameter-fixture (fn)
  (let* ((window (selected-window))
         (old (window-parameter window 'audit-persistent)))
    (unwind-protect (funcall fn window)
      (set-window-parameter window 'audit-persistent old))))
;; 'window-config:persistent-parameter'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters '((audit-persistent . t)))) (set-window-parameter window 'audit-persistent 1) (let ((saved (current-window-configuration))) (set-window-parameter window 'audit-persistent 2) (set-window-configuration saved) (window-parameter window 'audit-persistent))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:nonpersistent-parameter'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters nil)) (set-window-parameter window 'audit-persistent 1) (let ((saved (current-window-configuration))) (set-window-parameter window 'audit-persistent 2) (set-window-configuration saved) (window-parameter window 'audit-persistent))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-config:new-nonpersistent-parameter'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters nil)) (set-window-parameter window 'audit-persistent nil) (let ((saved (current-window-configuration))) (set-window-parameter window 'audit-persistent 9) (set-window-configuration saved) (window-parameter window 'audit-persistent))))) t))
           (error (list 'error err))))
  (terpri))
