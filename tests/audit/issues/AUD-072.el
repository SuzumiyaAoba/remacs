;;; -*- lexical-binding: t; -*-

;; 'buffer:undo-deleted-properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (buffer-enable-undo) (insert (propertize "abc" 'face 'bold)) (setq buffer-undo-list nil) (delete-region 2 3) (setq buffer-undo-list (primitive-undo 1 buffer-undo-list)) (list (buffer-substring-no-properties 1 4) (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))) t))
           (error (list 'error err))))
  (terpri))
