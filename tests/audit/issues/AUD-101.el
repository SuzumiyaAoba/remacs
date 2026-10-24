;;; -*- lexical-binding: t; -*-

;; 'read:buffer-error-point'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "(a") (goto-char 1) (list (condition-case err (read (current-buffer)) (error (car err))) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'read:marker-error-point'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "(a") (let ((m (copy-marker 1))) (list (condition-case err (read m) (error (car err))) (marker-position m)))) t))
           (error (list 'error err))))
  (terpri))

;; 'read:buffer-success-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "(a) (b)") (goto-char 1) (list (read (current-buffer)) (point) (read (current-buffer)) (point))) t))
           (error (list 'error err))))
  (terpri))
