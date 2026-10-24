;;; -*- lexical-binding: t; -*-

;; 'undo:read-only-text'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (put-text-property 1 4 'read-only t) (setq buffer-undo-list nil) (list (condition-case err (progn (primitive-undo 1 '((1 . 4))) 'undone) (error (car err))) (buffer-substring-no-properties (point-min) (point-max)))) t))
           (error (list 'error err))))
  (terpri))

;; 'undo:read-only-buffer'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (setq buffer-undo-list nil buffer-read-only t) (list (condition-case err (progn (primitive-undo 1 '((1 . 4))) 'undone) (error (car err))) (buffer-substring-no-properties (point-min) (point-max)))) t))
           (error (list 'error err))))
  (terpri))

;; 'undo:inhibit-read-only-binding'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (setq buffer-undo-list nil) (let ((seen 'not-called)) (let ((before-change-functions (list (lambda (&rest _args) (setq seen inhibit-read-only))))) (primitive-undo 1 '((1 . 4)))) (list seen (buffer-string)))) t))
           (error (list 'error err))))
  (terpri))
