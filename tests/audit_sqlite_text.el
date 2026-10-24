;;; audit_sqlite_text.el --- SQLite text decoding probes -*- lexical-binding: t; -*-
(load (expand-file-name "tests/audit_helpers.el") nil t)

(dolist (bytes '("" "61" "00" "610062" "c3a9e38182" "80ff" "c328" "e282" "eda080" "f4908080"))
  (audit-check
   (concat "sqlite-text:" bytes)
   `(let ((db (sqlite-open)))
      (unwind-protect
          (let ((s (caar (sqlite-select db ,(concat "select cast(x'" bytes "' as text)")))))
            (list (multibyte-string-p s) (string-bytes s) (string-to-list s)))
        (sqlite-close db)))))

(princ "AUDIT-DONE\n")
