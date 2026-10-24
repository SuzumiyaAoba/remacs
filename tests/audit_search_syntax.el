;;; audit_search_syntax.el --- syntax properties in regexp matching -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "regexp:syntax-property-word"
 '(with-temp-buffer
    (insert "a.a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
    (let ((parse-sexp-lookup-properties t))
      (goto-char 1) (re-search-forward "\\w+")
      (list (match-beginning 0) (match-end 0)))))

(audit-check "regexp:syntax-property-boundary"
 '(with-temp-buffer
    (insert "a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
    (let ((parse-sexp-lookup-properties t))
      (goto-char 2) (looking-at "\\b"))))

(audit-check "regexp:syntax-property-disabled-control"
 '(with-temp-buffer
    (insert "a.a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w"))
    (let ((parse-sexp-lookup-properties nil))
      (goto-char 1) (re-search-forward "\\w+")
      (list (match-beginning 0) (match-end 0)))))

(audit-check "regexp:syntax-property-string"
 '(let ((source (propertize "a.a" 'syntax-table (string-to-syntax "w")))
        (parse-sexp-lookup-properties t))
    (string-match "\\w+" source)
    (list (match-beginning 0) (match-end 0))))

(audit-check "regexp:syntax-table-control"
 '(with-temp-buffer
    (insert "a.a")
    (let ((table (copy-syntax-table (syntax-table))))
      (modify-syntax-entry ?. "w" table) (set-syntax-table table)
      (goto-char 1) (re-search-forward "\\w+")
      (list (match-beginning 0) (match-end 0)))))

(princ "AUDIT-DONE\n")
