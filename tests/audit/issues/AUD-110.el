;;; -*- lexical-binding: t; -*-

;; 'print:killed-buffer'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((dead (generate-new-buffer " *audit-print-dead*"))) (kill-buffer dead) (with-temp-buffer (insert "original") (let ((result (condition-case err (progn (princ "payload" dead) 'printed) (error (car err))))) (list result (buffer-string))))) t))
           (error (list 'error err))))
  (terpri))

;; 'print:detached-marker'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((m (make-marker))) (condition-case err (progn (princ "payload" m) 'printed) (error (car err)))) t))
           (error (list 'error err))))
  (terpri))

;; 'print:live-marker-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "ab") (goto-char 1) (let ((m (copy-marker 2))) (princ "X" m) (list (buffer-string) (point) (marker-position m)))) t))
           (error (list 'error err))))
  (terpri))

;; 'print:readonly-buffer'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (setq buffer-read-only t) (let ((result (condition-case err (progn (princ "payload" (current-buffer)) 'printed) (error (car err))))) (list result (buffer-string)))) t))
           (error (list 'error err))))
  (terpri))

;; 'print:readonly-marker'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((m (point-marker))) (setq buffer-read-only t) (let ((result (condition-case err (progn (princ "payload" m) 'printed) (error (car err))))) (list result (buffer-string) (marker-position m))))) t))
           (error (list 'error err))))
  (terpri))
