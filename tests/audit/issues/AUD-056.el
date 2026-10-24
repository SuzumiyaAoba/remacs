;;; -*- lexical-binding: t; -*-
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter")))
(unless (treesit-language-available-p 'json) (error "JSON grammar unavailable for audit"))
(defvar audit-treesit-hits 0)
(defun audit-treesit-notifier (_ranges _parser) (setq audit-treesit-hits (1+ audit-treesit-hits)))
;; "treesit:notifier-dedup"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((p (treesit-parser-create 'json))) (treesit-parser-add-notifier p 'audit-treesit-notifier) (treesit-parser-add-notifier p 'audit-treesit-notifier) (length (treesit-parser-notifiers p)))) t))
           (error (list 'error err))))
  (terpri))

;; "treesit:notifier-invalid"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((p (treesit-parser-create 'json))) (treesit-parser-add-notifier p 42) 'accepted)) t))
           (error (list 'error err))))
  (terpri))

;; "treesit:notifier-lambda"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((p (treesit-parser-create 'json))) (treesit-parser-add-notifier p (lambda (_ranges _parser) nil)) 'accepted)) t))
           (error (list 'error err))))
  (terpri))
