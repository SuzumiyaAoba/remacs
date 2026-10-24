;;; -*- lexical-binding: t; -*-
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter")))
(unless (treesit-language-available-p 'json) (error "JSON grammar unavailable for audit"))
(defvar audit-treesit-hits 0)
(defun audit-treesit-notifier (_ranges _parser) (setq audit-treesit-hits (1+ audit-treesit-hits)))
;; "treesit:query-repeat-capture"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "[1,2]") (let ((p (treesit-parser-create 'json))) (mapcar #'car (treesit-query-capture p "((array (number) @n (number) @n) (#eq? @n \"1\"))")))) t))
           (error (list 'error err))))
  (terpri))
