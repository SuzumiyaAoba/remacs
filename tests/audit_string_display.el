;;; audit_string_display.el --- display columns, composition, and intervals -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "string-width:custom-table"
 '(let ((char-width-table (copy-sequence char-width-table)))
    (aset char-width-table ?a 5)
    (list (char-width ?a) (string-width "aa") (truncate-string-to-width "aa" 6))))

(audit-check "string-width:tab-width"
 '(let ((tab-width 4))
    (list (char-width ?\t) (string-width "\t") (truncate-string-to-width "\tX" 4))))

(audit-check "truncate:ellipsis-text-property"
 '(let ((s (truncate-string-to-width "abcdef" 4 nil nil "." t)))
    (list (substring-no-properties s)
          (mapcar (lambda (p) (get-text-property p 'display s)) '(0 1 2 3 4 5)))))

(audit-check "truncate:padding-wide-start"
 '(truncate-string-to-width "日abc" 4 1 ?.))

(audit-check "truncate:properties"
 '(let ((s (truncate-string-to-width (propertize "abcdef" 'face 'bold) 3)))
    (list (substring-no-properties s) (get-text-property 0 'face s))))

(audit-check "string:glyph-explicit-composition"
 '(let ((s (copy-sequence "ab")))
    (compose-string s 0 2)
    (mapcar #'length (string-glyph-split s))))

(dolist (operation '(reverse nreverse))
  (audit-check (format "string:properties-%s" operation)
   `(let* ((s (concat (propertize "a" 'face 'bold) "bc"))
           (out (,operation s)))
      (list (substring-no-properties out)
            (mapcar (lambda (p) (get-text-property p 'face out)) '(0 1 2))))))

(princ "AUDIT-DONE\n")
