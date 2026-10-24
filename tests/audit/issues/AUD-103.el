;;; -*- lexical-binding: t; -*-
(defvar audit-eval-events nil)
(defvar audit-eval-result nil)
;; 'eval-buffer:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((audit-eval-events nil)) (with-temp-buffer (insert "(push 'outside audit-eval-events)\n(push 'inside audit-eval-events)") (goto-char 1) (forward-line 1) (narrow-to-region (point) (point-max)) (eval-buffer) (nreverse audit-eval-events))) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-region:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((audit-eval-events nil)) (with-temp-buffer (insert "(push 'outside audit-eval-events)\n(push 'inside audit-eval-events)") (goto-char 1) (forward-line 1) (narrow-to-region (point) (point-max)) (eval-region nil nil) (nreverse audit-eval-events))) t))
           (error (list 'error err))))
  (terpri))

;; 'eval-region:inaccessible-bounds'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((audit-eval-events nil)) (with-temp-buffer (insert "(push 'outside audit-eval-events)\n(push 'inside audit-eval-events)") (let ((end (point-max))) (goto-char 1) (forward-line 1) (narrow-to-region (point) end) (list (condition-case err (progn (eval-region 1 end) 'evaluated) (error (car err))) (nreverse audit-eval-events))))) t))
           (error (list 'error err))))
  (terpri))
