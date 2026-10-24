;;; -*- lexical-binding: t; -*-
(defun audit-window-parameter-fixture (fn)
  (let* ((window (selected-window))
         (old (window-parameter window 'audit-persistent)))
    (unwind-protect (funcall fn window)
      (set-window-parameter window 'audit-persistent old))))
;; 'window-state:persistence-filter'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters '((audit-persistent . t)))) (set-window-parameter window 'audit-persistent 1) (list (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window))))) (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window t))))))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-state:writable-parameter'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters '((audit-persistent . writable)))) (set-window-parameter window 'audit-persistent 1) (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window t)))))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-state:nonpersistent-raw'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters nil)) (set-window-parameter window 'audit-persistent 1) (cdr (assq 'parameters (window-state-get window t)))))) t))
           (error (list 'error err))))
  (terpri))

;; 'window-state:put-parameter-alist'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-window-parameter-fixture (lambda (window) (let ((window-persistent-parameters '((audit-persistent . writable)))) (set-window-parameter window 'audit-persistent 1) (let ((state (window-state-get window t))) (setcdr (assq 'parameters state) '((audit-persistent . 1))) (set-window-parameter window 'audit-persistent 2) (window-state-put state window) (window-parameter window 'audit-persistent))))) t))
           (error (list 'error err))))
  (terpri))
