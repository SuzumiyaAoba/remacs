;;; -*- lexical-binding: t; -*-
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter")))
(unless (treesit-language-available-p 'json) (error "JSON grammar unavailable for audit"))
(defvar audit-treesit-hits 0)
(defun audit-treesit-notifier (_ranges _parser) (setq audit-treesit-hits (1+ audit-treesit-hits)))
;; "treesit:indirect-buffer"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "[1,2]") (let* ((base (current-buffer)) (p (treesit-parser-create 'json)) (indirect (make-indirect-buffer base " *audit-treesit-indirect*"))) (unwind-protect (list (eq p (treesit-parser-create 'json indirect)) (length (treesit-parser-list indirect))) (kill-buffer indirect)))) t))
           (error (list 'error err))))
  (terpri))
