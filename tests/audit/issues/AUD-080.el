;;; -*- lexical-binding: t; -*-

;; 'transpose:properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert (propertize "ab" 'face 'bold) "--" (propertize "XYZ" 'face 'italic)) (transpose-regions 1 3 5 8) (list (buffer-substring-no-properties 1 8) (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3 4 5 6 7)))) t))
           (error (list 'error err))))
  (terpri))
