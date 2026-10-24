;;; audit_search_case.el --- current case table in search primitives -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "search:custom-case-table"
 '(with-temp-buffer
    (let ((ct (copy-case-table (standard-case-table))) (case-fold-search t))
      (set-case-syntax-pair ?X ?y ct) (set-case-table ct)
      (insert "y") (goto-char 1)
      (let ((literal (search-forward "X" nil t)))
        (goto-char 1)
        (list (upcase ?y) literal (re-search-forward "X" nil t)
              (string-match "X" "y"))))))

(audit-check "search:default-case-table-control"
 '(with-temp-buffer
    (let ((case-fold-search t))
      (insert "x") (goto-char 1)
      (let ((literal (search-forward "X" nil t)))
        (goto-char 1)
        (list literal (re-search-forward "X" nil t) (string-match "X" "x"))))))

(princ "AUDIT-DONE\n")
