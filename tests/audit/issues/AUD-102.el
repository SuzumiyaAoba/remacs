;;; -*- lexical-binding: t; -*-

;; 'print:override-length'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(prin1-to-string '(1 2 3) nil '((length . 1))) t))
           (error (list 'error err))))
  (terpri))

;; 'print:override-defaults'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((print-length 1)) (prin1-to-string '(1 2 3) nil t)) t))
           (error (list 'error err))))
  (terpri))

;; 'print:override-buffer-stream'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (prin1 '(1 2 3) (current-buffer) '((length . 1))) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))

;; 'print:override-invalid-setting'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(prin1-to-string '(1 2 3) nil '((not-a-print-setting . 1))) t))
           (error (list 'error err))))
  (terpri))
