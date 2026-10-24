;;; audit_case_compare.el --- case folding and character display tables -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "char-equal:nonunicode"
 '(let ((case-fold-search t))
    (list (char-equal #x110000 #x110001)
          (char-equal #xd800 #xd801)
          (char-equal #x3fff80 #x3fffff))))

(audit-check "char-equal:nonunicode-no-fold-control"
 '(let ((case-fold-search nil))
    (list (char-equal #x110000 #x110001)
          (char-equal #xd800 #xd801)
          (char-equal #x3fff80 #x3fffff))))

(audit-check "char-equal:greek-sigma"
 '(let ((case-fold-search t))
    (list (char-equal ?Σ ?ς) (char-equal ?σ ?ς))))

(audit-check "compare-strings:sharp-s"
 '(compare-strings "ß" nil nil "S" nil nil t))

(audit-check "string-width:nonascii-custom-table"
 '(let ((char-width-table (copy-sequence char-width-table)))
    (set-char-table-range char-width-table ?日 5)
    (list (char-width ?日) (string-width "日日")
          (truncate-string-to-width "日日" 6))))

(princ "AUDIT-DONE\n")
