;;; -*- lexical-binding: t; -*-
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)
;; 'eval-buffer:return-value'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "42") (eval-buffer)) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-region:return-value'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "42") (eval-region (point-min) (point-max))) t))
           (error (list 'error err))))
  (terpri))
