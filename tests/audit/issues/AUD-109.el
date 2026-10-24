;;; -*- lexical-binding: t; -*-



;; 'delete-blank-lines:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "head\n\n\nbody\n") (put-text-property 6 7 'face 'bold) (put-text-property 7 8 'face 'italic) (narrow-to-region 6 7) (goto-char 6) (delete-blank-lines) (let ((bounds (list (point-min) (point-max))) (visible (string-to-list (buffer-string)))) (widen) (list bounds visible (buffer-substring-no-properties (point-min) (point-max)) (get-text-property 6 'face)))) t))
           (error (list 'error err))))
  (terpri))

;; 'delete-horizontal-space:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "A    B") (narrow-to-region 3 5) (goto-char 4) (delete-horizontal-space) (widen) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))

;; 'delete-indentation:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "head\n  body") (narrow-to-region 8 12) (goto-char 8) (delete-indentation) (widen) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))

;; 'just-one-space:narrowing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "A    B") (narrow-to-region 3 5) (goto-char 4) (just-one-space) (widen) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))
