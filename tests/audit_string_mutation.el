;;; audit_string_mutation.el --- string storage and mutation -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "string:fillarray-unibyte-ascii"
 '(let ((s (unibyte-string 128 255)))
    (list (condition-case err (progn (fillarray s ?a) 'filled) (error (car err)))
          (multibyte-string-p s) (string-to-list s))))

(audit-check "string:fillarray-unibyte-high"
 '(let ((s (unibyte-string 128 255)))
    (list (condition-case err (progn (fillarray s 233) 'filled) (error (car err)))
          (multibyte-string-p s) (string-to-list s))))

(audit-check "string:fillarray-multibyte-ascii"
 '(let ((s (with-temp-buffer (insert "aa") (buffer-string))))
    (list (condition-case err (progn (fillarray s 233) 'filled) (error (car err)))
          (multibyte-string-p s) (string-to-list s))))

(audit-check "string:clear-unibyte"
 '(let ((s (unibyte-string 128 255)))
    (clear-string s)
    (list (length s) (string-bytes s) (string-to-list s))))

(audit-check "string:clear-multibyte-control"
 '(let ((s (copy-sequence "éあ")))
    (clear-string s)
    (list (length s) (string-bytes s) (multibyte-string-p s) (string-to-list s))))

(audit-check "string:store-nonbyte-unibyte"
 '(let ((s (unibyte-string 65 66)))
    (list (condition-case err (progn (store-substring s 0 256) 'stored) (error (car err)))
          (multibyte-string-p s) (string-to-list s))))

(audit-check "string:store-byte-width"
 '(let ((s (copy-sequence "éé")))
    (list (condition-case err (progn (store-substring s 0 "a") 'stored) (error (car err)))
          (string-to-list s))))

(audit-check "string:store-invalid-character"
 '(let ((s (copy-sequence "ab")))
    (list (condition-case err (progn (store-substring s 0 -1) 'stored) (error (car err)))
          (string-to-list s))))

(audit-check "array:aset-marker-index"
 '(with-temp-buffer
    (insert "abc") (goto-char 1)
    (let ((v (vector 'a 'b 'c)))
      (list (condition-case err (progn (aset v (point-marker) 'x) 'stored) (error (car err)))
            v))))

(dolist (operation '(reverse nreverse string-reverse))
  (audit-check (format "string:unibyte-%s" operation)
   `(let* ((s (unibyte-string 128 255)) (out (,operation s)))
      (list (multibyte-string-p out) (string-to-list out)
            (multibyte-string-p s) (string-to-list s)))))

(princ "AUDIT-DONE\n")
