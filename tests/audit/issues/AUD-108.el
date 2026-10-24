;;; -*- lexical-binding: t; -*-

;; 'delete-blank-lines:nbsp'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "head\n \nbody\n") (goto-char 6) (delete-blank-lines) (string-to-list (buffer-string))) t))
           (error (list 'error err))))
  (terpri))

;; 'delete-blank-lines:carriage-return'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "head\n\15\nbody\n") (goto-char 6) (delete-blank-lines) (string-to-list (buffer-string))) t))
           (error (list 'error err))))
  (terpri))

;; 'delete-blank-lines:ascii-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "head\n \11\nbody\n") (goto-char 6) (delete-blank-lines) (string-to-list (buffer-string))) t))
           (error (list 'error err))))
  (terpri))
