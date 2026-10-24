;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar audit-thread-ready nil)
(defvar audit-thread-special 'global)
;; "mutex:busy-lock"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((m (make-mutex)) (entered nil) worker) (mutex-lock m) (unwind-protect (progn (setq worker (make-thread (lambda nil (mutex-lock m) (unwind-protect (setq entered t) (mutex-unlock m))))) (sleep-for 0.03) (let ((before-unlock entered)) (ignore-errors (mutex-unlock m)) (thread-join worker) (list before-unlock entered))) (ignore-errors (mutex-unlock m)))) t))
           (error (list 'error err))))
  (terpri))

;; "mutex:foreign-unlock"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((m (make-mutex))) (mutex-lock m) (unwind-protect (thread-join (make-thread (lambda nil (condition-case e (progn (mutex-unlock m) 'unlocked) (error (car e)))))) (ignore-errors (mutex-unlock m)))) t))
           (error (list 'error err))))
  (terpri))

;; "mutex:recursive-depth"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((m (make-mutex))) (mutex-lock m) (mutex-lock m) (unwind-protect (list (mutex-unlock m) (mutex-unlock m)) (ignore-errors (mutex-unlock m)))) t))
           (error (list 'error err))))
  (terpri))
