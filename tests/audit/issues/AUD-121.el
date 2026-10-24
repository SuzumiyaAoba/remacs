;;; -*- lexical-binding: t; -*-

;; 'undo:position-only-history'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (goto-char 1) (setq buffer-undo-list nil) (primitive-undo 1 '(2)) (list (point) buffer-undo-list)) t))
           (error (list 'error err))))
  (terpri))

;; 'undo:apply-disabled-history'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (setq buffer-undo-list t) (primitive-undo 1 '((apply ignore))) buffer-undo-list) t))
           (error (list 'error err))))
  (terpri))
