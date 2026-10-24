;;; Edebug's signal hook precedes validation and handler selection. -*- lexical-binding: t; -*-
(require 'bytecomp)
(define-error 'qa-sh-class "Hook condition")
(defvar qa-sh-events nil)
(defvar qa-sh-function nil)
(defvar qa-sh-input nil)
(defvar qa-sh-data nil)

(dolist (qa-sh-function
         (list (lambda () (signal 'qa-sh-class '(payload)))
               (byte-compile '(lambda () (signal 'qa-sh-class '(payload))))))
  (qa (let (qa-sh-events
             (signal-hook-function (lambda (_symbol _data) (push 'hook qa-sh-events))))
        (condition-case nil
            (handler-bind ((qa-sh-class (lambda (_) (push 'observer qa-sh-events))))
              (funcall qa-sh-function))
          (qa-sh-class (push 'case qa-sh-events)))
        (nreverse qa-sh-events)) '(hook observer case))
  (qa (let (qa-sh-events
             (signal-hook-function
              (lambda (symbol data) (push (cons symbol data) qa-sh-events))))
        (condition-case nil (funcall qa-sh-function) (qa-sh-class nil))
        qa-sh-events) '((qa-sh-class payload))))

(qa (let (qa-sh-events
           (signal-hook-function (lambda (symbol _data) (push symbol qa-sh-events))))
      (condition-case nil (+ 'bad 1) (error nil))
      qa-sh-events) '(wrong-type-argument))
(qa (let (qa-sh-events
           (signal-hook-function (lambda (symbol _data) (push symbol qa-sh-events))))
      (condition-case nil (signal 'quit nil) (quit nil))
      qa-sh-events) '(quit))

(qa (let ((signal-hook-function
           (lambda (_symbol _data)
             (setq signal-hook-function nil)
             (error "hook replacement"))))
      (condition-case err (signal 'qa-sh-class nil)
        (qa-sh-class 'original) (error err))) '(error "hook replacement"))
(qa (let ((signal-hook-function (lambda (_symbol _data) (throw 'hook-exit 'escaped))))
      (catch 'hook-exit (condition-case nil (signal 'qa-sh-class nil) (error 'caught)))) 'escaped)

;; A replacement property in the hook affects this signal; handler-bind
;; changes still use the property identity selected after the hook.
(qa (let ((saved (get 'qa-sh-class 'error-conditions)) qa-sh-events
          (signal-hook-function
           (lambda (_symbol _data)
             (put 'qa-sh-class 'error-conditions '(file-error error))
             (push 'hook qa-sh-events))))
      (unwind-protect
          (progn
            (condition-case nil
                (handler-bind ((file-error (lambda (_) (push 'observer qa-sh-events))))
                  (signal 'qa-sh-class nil))
              (qa-sh-class (push 'old qa-sh-events))
              (file-error (push 'new qa-sh-events)))
            (nreverse qa-sh-events))
        (put 'qa-sh-class 'error-conditions saved))) '(hook observer new))

(qa (let* ((qa-sh-input (list 'qa-sh-class 'payload))
           (signal-hook-function (lambda (_symbol _data) (setcar qa-sh-input 'file-error))))
      (condition-case err (signal qa-sh-input)
        (qa-sh-class (list (eq err qa-sh-input) (car err)))
        (file-error 'wrong-handler))) '(t file-error))
(qa (let* ((qa-sh-data (list 'payload))
           (signal-hook-function (lambda (_symbol data) (setcar data 'changed))))
      (condition-case err (signal 'qa-sh-class qa-sh-data)
        (qa-sh-class (list (eq (cdr err) qa-sh-data) err)))) '(t (qa-sh-class changed)))

(qa (let (qa-sh-events
           (signal-hook-function
            (lambda (symbol _data)
              (push symbol qa-sh-events)
              (when (eq symbol 'qa-sh-class) (signal 'file-error '(nested))))))
      (list (condition-case nil (signal 'qa-sh-class nil)
              (qa-sh-class 'original) (file-error 'replacement))
            (nreverse qa-sh-events))) '(replacement (qa-sh-class file-error)))

;; Even invalid condition names reach the hook before Fget rejects them.
(qa (let (qa-sh-events
           (signal-hook-function
            (lambda (symbol _data) (push symbol qa-sh-events))))
      (list (condition-case err (signal [invalid]) (error err))
            (nreverse qa-sh-events)))
    '((wrong-type-argument symbolp [invalid]) ([invalid] wrong-type-argument)))
(qa (let (qa-sh-events
           (signal-hook-function
            (lambda (symbol _data) (push symbol qa-sh-events))))
      (list (condition-case err (signal 'qa-sh-undefined '(payload)) (error err))
            (nreverse qa-sh-events)))
    '((error "Invalid error symbol" qa-sh-undefined) (qa-sh-undefined error)))
(qa (let ((signal-hook-function
           (lambda (symbol _data)
             (when (eq symbol 'qa-sh-repair)
               (put symbol 'error-conditions '(qa-sh-repair error))))))
      (unwind-protect
          (condition-case err (signal 'qa-sh-repair '(payload)) (qa-sh-repair err))
        (put 'qa-sh-repair 'error-conditions nil))) '(qa-sh-repair payload))

(qa (condition-case err (signal nil nil) (error err)) '(error))
(qa (condition-case err (signal nil 7) (error err)) '(error . 7))
(qa (let (qa-sh-events
           (signal-hook-function (lambda (symbol _data) (push symbol qa-sh-events))))
      (condition-case nil (signal nil nil) (error nil))
      qa-sh-events) '(error))
(qa (let* ((input (list 'error 'memory)) qa-sh-events
           (signal-hook-function (lambda (symbol _data) (push symbol qa-sh-events))))
      (list (condition-case err (signal nil input) (error (eq err input))) qa-sh-events)) '(t nil))
(princ (qa--done)) (terpri)
