;;; audit_treesit.el --- JSON grammar edit/lifecycle/query probes -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter")))
(unless (treesit-language-available-p 'json) (error "JSON grammar unavailable for audit"))
(defvar audit-treesit-hits 0)
(defun audit-treesit-notifier (_ranges _parser) (setq audit-treesit-hits (1+ audit-treesit-hits)))

(audit-check "treesit:basic-control"
 '(with-temp-buffer
    (insert "[1,2]")
    (let ((p (treesit-parser-create 'json)))
      (treesit-node-string (treesit-parser-root-node p)))))

(audit-check "treesit:edit-same-length"
 '(with-temp-buffer
    (insert "[1,2]")
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-root-node p)
      (goto-char 2) (delete-char 1) (insert "x")
      (let ((root (treesit-parser-root-node p)))
        (list (treesit-node-check root 'has-error) (treesit-node-string root))))))

(audit-check "treesit:edit-longer"
 '(with-temp-buffer
    (insert "[1,2]")
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-root-node p)
      (goto-char 2) (delete-char 1) (insert "null")
      (treesit-node-string (treesit-parser-root-node p)))))

(audit-check "treesit:old-node-after-edit"
 '(with-temp-buffer
    (insert "[1,2]")
    (let* ((p (treesit-parser-create 'json)) (old (treesit-parser-root-node p)))
      (goto-char (point-max)) (insert " ")
      (let ((before (treesit-node-check old 'outdated)))
        (treesit-parser-root-node p)
        (list before (treesit-node-check old 'outdated)
              (condition-case e (treesit-node-type old) (error (car e))))))))

(audit-check "treesit:old-node-after-repeated-reparse"
 '(with-temp-buffer
    (insert "[1,2]")
    (let* ((p (treesit-parser-create 'json)) (old (treesit-parser-root-node p)) hits)
      (dotimes (_ 8)
        (goto-char (point-max)) (insert " ")
        (treesit-parser-root-node p)
        (push (treesit-node-check old 'outdated) hits))
      (nreverse hits))))

(audit-check "treesit:notifier-dedup"
 '(with-temp-buffer
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-add-notifier p 'audit-treesit-notifier)
      (treesit-parser-add-notifier p 'audit-treesit-notifier)
      (length (treesit-parser-notifiers p)))))

(audit-check "treesit:notifier-invalid"
 '(with-temp-buffer
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-add-notifier p 42) 'accepted)))

(audit-check "treesit:notifier-lambda"
 '(with-temp-buffer
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-add-notifier p (lambda (_ranges _parser) nil)) 'accepted)))

(audit-check "treesit:delete-twice"
 '(with-temp-buffer
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-delete p) (treesit-parser-delete p))))

(audit-check "treesit:killed-buffer-parser"
 '(let* ((b (generate-new-buffer " *audit-treesit*"))
         (p (treesit-parser-create 'json b)))
    (kill-buffer b)
    (list (treesit-parser-p p)
          (condition-case e (treesit-parser-language p) (error (car e))))))

(audit-check "treesit:ranges-invalid"
 '(with-temp-buffer
    (insert "[1,2]")
    (treesit-parser-set-included-ranges (treesit-parser-create 'json) '((0 . 99)))))

(audit-check "treesit:ranges-edit"
 '(with-temp-buffer
    (insert "xx [1] yy")
    (let ((p (treesit-parser-create 'json)))
      (treesit-parser-set-included-ranges p '((4 . 7)))
      (treesit-parser-root-node p)
      (goto-char 1) (insert "ZZ")
      (treesit-parser-included-ranges p))))

(audit-check "treesit:query-control"
 '(with-temp-buffer
    (insert "[1,2]")
    (let ((p (treesit-parser-create 'json)))
      (mapcar #'car (treesit-query-capture p "(number) @n")))))

(audit-check "treesit:query-match-case"
 '(with-temp-buffer
    (insert "[\"A\",\"a\"]")
    (let ((p (treesit-parser-create 'json)) (case-fold-search t))
      (length (treesit-query-capture p "((string) @s (#match? @s \"A\"))")))))

(audit-check "treesit:query-repeat-capture"
 '(with-temp-buffer
    (insert "[1,2]")
    (let ((p (treesit-parser-create 'json)))
      (mapcar #'car (treesit-query-capture p "((array (number) @n (number) @n) (#eq? @n \"1\"))")))))

(audit-check "treesit:parser-reuse-control"
 '(with-temp-buffer
    (let ((p (treesit-parser-create 'json)))
      (eq p (treesit-parser-create 'json)))))

(audit-check "treesit:indirect-buffer"
 '(with-temp-buffer
    (insert "[1,2]")
    (let* ((base (current-buffer)) (p (treesit-parser-create 'json))
           (indirect (make-indirect-buffer base " *audit-treesit-indirect*")))
      (unwind-protect
          (list (eq p (treesit-parser-create 'json indirect))
                (length (treesit-parser-list indirect)))
        (kill-buffer indirect)))))

(princ "AUDIT-DONE\n")
