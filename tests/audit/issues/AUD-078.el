;;; -*- lexical-binding: t; -*-

;; 'replace-region:callback-narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "[abc]") (let (seen) (replace-region-contents 2 5 (lambda nil (setq seen (list (point-min) (point-max) (buffer-string))) "X")) (list seen (buffer-string) (point-min) (point-max)))) t))
           (error (list 'error err))))
  (terpri))
