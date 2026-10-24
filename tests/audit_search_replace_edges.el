;;; audit_search_replace_edges.el --- counts, restrictions, replacement metadata -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "search:backward-count-all-or-nothing"
 '(with-temp-buffer
    (insert "a") (goto-char (point-max))
    (list (search-backward "a" nil t 2) (point))))

(dolist (entry '((re-search-backward 2) (posix-search-backward 2) (search-forward -2)))
  (audit-check (format "search:count-%s" (car entry))
   `(with-temp-buffer
      (insert "a") (goto-char (point-max))
      (list (,(car entry) "a" nil t ,(cadr entry)) (point)))))

(audit-check "search:backward-count-error"
 '(with-temp-buffer
    (insert "a") (goto-char (point-max))
    (list (condition-case err (search-backward "a" nil nil 2) (error (car err))) (point))))

(audit-check "search:backward-one-control"
 '(with-temp-buffer
    (insert "a") (goto-char (point-max))
    (list (search-backward "a" nil t 1) (point))))

(audit-check "replace-match:backrefs-after-widen"
 '(with-temp-buffer
    (insert "prefix ab suffix")
    (narrow-to-region 8 10) (goto-char (point-min))
    (re-search-forward "\\(a\\)\\(b\\)")
    (widen)
    (replace-match "\\2\\1" t)
    (buffer-string)))

(audit-check "replace-match:backrefs-narrowed-control"
 '(with-temp-buffer
    (insert "prefix ab suffix")
    (narrow-to-region 8 10) (goto-char (point-min))
    (re-search-forward "\\(a\\)\\(b\\)")
    (replace-match "\\2\\1" t)
    (widen) (buffer-string)))

(audit-check "replace-match:string-properties"
 '(let* ((source (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic)))
         (replacement (propertize "X" 'face 'underline)))
    (string-match "b" source)
    (let ((out (replace-match replacement t nil source)))
      (list (substring-no-properties out)
            (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2))))))

(audit-check "replace-regexp-in-string:properties"
 '(let* ((source (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic)))
         (replacement (propertize "X" 'face 'underline))
         (out (replace-regexp-in-string "b" replacement source t)))
    (list (substring-no-properties out)
          (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2)))))

(audit-check "replace-match:unibyte-result"
 '(let ((source (unibyte-string 128 97 255)))
    (string-match "a" source)
    (let ((out (replace-match "X" t nil source)))
      (list (multibyte-string-p out) (string-to-list out)))))

(audit-check "replace-regexp-in-string:unibyte-result"
 '(let ((out (replace-regexp-in-string "a" "X" (unibyte-string 128 97 255) t)))
    (list (multibyte-string-p out) (string-to-list out))))

(princ "AUDIT-DONE\n")
