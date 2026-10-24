;;; -*- lexical-binding: t; -*-
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter")))
(unless (treesit-language-available-p 'json) (error "JSON grammar unavailable for audit"))
(defvar audit-treesit-hits 0)
(defun audit-treesit-notifier (_ranges _parser) (setq audit-treesit-hits (1+ audit-treesit-hits)))
;; "treesit:edit-same-length"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "[1,2]") (let ((p (treesit-parser-create 'json))) (treesit-parser-root-node p) (goto-char 2) (delete-char 1) (insert "x") (let ((root (treesit-parser-root-node p))) (list (treesit-node-check root 'has-error) (treesit-node-string root))))) t))
           (error (list 'error err))))
  (terpri))

;; "treesit:edit-longer"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "[1,2]") (let ((p (treesit-parser-create 'json))) (treesit-parser-root-node p) (goto-char 2) (delete-char 1) (insert "null") (treesit-node-string (treesit-parser-root-node p)))) t))
           (error (list 'error err))))
  (terpri))
