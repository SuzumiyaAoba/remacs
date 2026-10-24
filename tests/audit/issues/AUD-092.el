;;; -*- lexical-binding: t; -*-

;; 'string:properties-nreverse'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((s (concat (propertize "a" 'face 'bold) "bc")) (out (nreverse s))) (list (substring-no-properties out) (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2)))) t))
           (error (list 'error err))))
  (terpri))

;; 'string:properties-reverse'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((s (concat (propertize "a" 'face 'bold) "bc")) (out (reverse s))) (list (substring-no-properties out) (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2)))) t))
           (error (list 'error err))))
  (terpri))
