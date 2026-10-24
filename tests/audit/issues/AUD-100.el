;;; -*- lexical-binding: t; -*-

;; 'read:function-stream-one-form'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((input (string-to-list "(a) (b)"))) (let ((stream (lambda (&optional unread) (if unread (push unread input) (pop input))))) (let ((first (read stream))) (list first input (condition-case err (read stream) (error (car err))))))) t))
           (error (list 'error err))))
  (terpri))

;; 'read:function-stream-unread'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((input (string-to-list "123 456")) (unreads 0)) (let ((stream (lambda (&optional unread) (if unread (progn (setq unreads (1+ unreads)) (push unread input)) (pop input))))) (let ((first (read stream))) (list first (> unreads 0) (condition-case err (read stream) (error (car err))))))) t))
           (error (list 'error err))))
  (terpri))
