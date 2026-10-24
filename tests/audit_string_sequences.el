;;; audit_string_sequences.el --- sequence conversion and mixed encodings -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "string-to-list:list-alias"
 '(let* ((source (list 1 2)) (out (string-to-list source)))
    (let ((shared (eq source out)))
      (setcar out 9)
      (list shared source out))))

(audit-check "string-to-list:improper-list"
 '(string-to-list '(1 . 2)))

(audit-check "string-to-vector:list"
 '(string-to-vector '(1 2)))

(audit-check "string-to-vector:vector"
 '(let* ((source (vector 1 2)) (out (string-to-vector source)))
    (list (eq source out) out)))

(audit-check "string-to-vector:bool-vector"
 '(string-to-vector (bool-vector t nil)))

(audit-check "string-compare:unibyte-unicode"
 '(let ((byte (unibyte-string 233)) (unicode (string 233)))
    (list (equal byte unicode) (string= byte unicode)
          (string-lessp byte unicode) (string-lessp unicode byte)
          (compare-strings byte nil nil unicode nil nil))))

(audit-check "string-compare:unibyte-order"
 '(let ((byte (unibyte-string 233)) (unicode (string 256)))
    (list (string-lessp byte unicode) (string-lessp unicode byte)
          (compare-strings byte nil nil unicode nil nil))))

(audit-check "string-compare:custom-case-table"
 '(with-temp-buffer
    (let ((ct (copy-case-table (standard-case-table))) (case-fold-search t))
      (set-case-syntax-pair ?X ?y ct)
      (set-case-table ct)
      (list (upcase ?y) (char-equal ?X ?y)
            (compare-strings "X" nil nil "y" nil nil t)))))

(princ "AUDIT-DONE\n")
