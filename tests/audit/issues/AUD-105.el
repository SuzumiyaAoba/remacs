;;; -*- lexical-binding: t; -*-
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)
;; 'eval-buffer:lexical-header'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((audit-eval-result nil)) (with-temp-buffer (insert ";;; -*- lexical-binding: t -*-\n" "(setq audit-eval-result (let ((x 7)) (lambda () x)))") (let ((lexical-binding nil)) (eval-buffer)) (condition-case err (funcall audit-eval-result) (error (car err))))) t))
           (error (list 'error err))))
  (terpri))
