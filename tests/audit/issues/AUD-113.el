;;; -*- lexical-binding: t; -*-



;; 'replace-match:string-properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((source (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic))) (replacement (propertize "X" 'face 'underline))) (string-match "b" source) (let ((out (replace-match replacement t nil source))) (list (substring-no-properties out) (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2))))) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-regexp-in-string:properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let* ((source (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic))) (replacement (propertize "X" 'face 'underline)) (out (replace-regexp-in-string "b" replacement source t))) (list (substring-no-properties out) (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2)))) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-match:unibyte-result'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (unibyte-string 128 97 255))) (string-match "a" source) (let ((out (replace-match "X" t nil source))) (list (multibyte-string-p out) (string-to-list out)))) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-regexp-in-string:unibyte-result'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((out (replace-regexp-in-string "a" "X" (unibyte-string 128 97 255) t))) (list (multibyte-string-p out) (string-to-list out))) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-match:buffer-properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic))) (goto-char 1) (re-search-forward "b") (replace-match (propertize "X" 'face 'underline) t t) (list (buffer-substring-no-properties (point-min) (point-max)) (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))) t))
           (error (list 'error err))))
  (terpri))

;; 'match-string:string-properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (propertize "abc" 'face 'bold))) (string-match "b" source) (let ((out (match-string 0 source))) (list (substring-no-properties out) (get-text-property 0 'face out)))) t))
           (error (list 'error err))))
  (terpri))
