;;; audit_window_parameters.el --- persistent parameters and window margins -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(defun audit-window-parameter-fixture (fn)
  (let* ((window (selected-window))
         (old (window-parameter window 'audit-persistent)))
    (unwind-protect (funcall fn window)
      (set-window-parameter window 'audit-persistent old))))

(audit-check "window-config:nonpersistent-parameter"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters nil))
       (set-window-parameter window 'audit-persistent 1)
       (let ((saved (current-window-configuration)))
         (set-window-parameter window 'audit-persistent 2)
         (set-window-configuration saved)
         (window-parameter window 'audit-persistent))))))

(audit-check "window-config:new-nonpersistent-parameter"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters nil))
       (set-window-parameter window 'audit-persistent nil)
       (let ((saved (current-window-configuration)))
         (set-window-parameter window 'audit-persistent 9)
         (set-window-configuration saved)
         (window-parameter window 'audit-persistent))))))

(audit-check "window-config:persistent-parameter"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters '((audit-persistent . t))))
       (set-window-parameter window 'audit-persistent 1)
       (let ((saved (current-window-configuration)))
         (set-window-parameter window 'audit-persistent 2)
         (set-window-configuration saved)
         (window-parameter window 'audit-persistent))))))

(audit-check "window-state:persistence-filter"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters '((audit-persistent . t))))
       (set-window-parameter window 'audit-persistent 1)
       (list (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window)))))
             (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window t))))))))))

(audit-check "window-state:writable-parameter"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters '((audit-persistent . writable))))
       (set-window-parameter window 'audit-persistent 1)
       (cdr (assq 'audit-persistent (cdr (assq 'parameters (window-state-get window t)))))))))

(audit-check "window-config:margin-restoration"
 '(let* ((window (selected-window)) (old (window-margins window)))
    (unwind-protect
        (progn
          (set-window-margins window 2 3)
          (let ((before (window-margins window)) (saved (current-window-configuration)))
            (set-window-margins window 4 5)
            (let ((changed (window-margins window)))
              (set-window-configuration saved)
              (list before changed (window-margins window)))))
      (set-window-margins window (car old) (cdr old)))))

(audit-check "window-state:nonpersistent-raw"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters nil))
       (set-window-parameter window 'audit-persistent 1)
       (cdr (assq 'parameters (window-state-get window t)))))))

(audit-check "window-state:put-parameter-alist"
 '(audit-window-parameter-fixture
   (lambda (window)
     (let ((window-persistent-parameters '((audit-persistent . writable))))
       (set-window-parameter window 'audit-persistent 1)
       (let ((state (window-state-get window t)))
         (setcdr (assq 'parameters state) '((audit-persistent . 1)))
         (set-window-parameter window 'audit-persistent 2)
         (window-state-put state window)
         (window-parameter window 'audit-persistent))))))

(princ "AUDIT-DONE\n")
