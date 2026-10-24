;;; audit_match_properties.el --- match extraction and replacement intervals -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "match-string:buffer-properties"
 '(with-temp-buffer
    (insert (propertize "abc" 'face 'bold)) (goto-char 1)
    (re-search-forward "b")
    (let ((out (match-string 0)))
      (list (substring-no-properties out) (get-text-property 0 'face out)))))

(audit-check "match-string:string-properties"
 '(let ((source (propertize "abc" 'face 'bold)))
    (string-match "b" source)
    (let ((out (match-string 0 source)))
      (list (substring-no-properties out) (get-text-property 0 'face out)))))

(audit-check "match-string:no-properties-control"
 '(let ((source (propertize "abc" 'face 'bold)))
    (string-match "b" source)
    (let ((out (match-string-no-properties 0 source)))
      (list (substring-no-properties out) (get-text-property 0 'face out)))))

(audit-check "match-string:unibyte-result"
 '(let ((source (unibyte-string 128 97 255)))
    (string-match ".*" source)
    (let ((out (match-string 0 source)))
      (list (multibyte-string-p out) (string-to-list out)))))

(audit-check "match-string:no-properties-unibyte-control"
 '(let ((source (unibyte-string 128 97 255)))
    (string-match ".*" source)
    (let ((out (match-string-no-properties 0 source)))
      (list (multibyte-string-p out) (string-to-list out)))))

(audit-check "replace-match:buffer-properties"
 '(with-temp-buffer
    (insert (concat (propertize "a" 'face 'bold) "b" (propertize "c" 'face 'italic)))
    (goto-char 1) (re-search-forward "b")
    (replace-match (propertize "X" 'face 'underline) t t)
    (list (buffer-substring-no-properties (point-min) (point-max))
          (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))

(princ "AUDIT-DONE\n")
