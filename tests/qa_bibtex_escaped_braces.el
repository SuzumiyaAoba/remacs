;; Escaped braces inside braced BibTeX values do not terminate the field.
;; Avoid the unrelated iso8601 load failure while loading bibtex on Remacs.
(provide 'iso8601)
(require 'bibtex)

(qa (with-temp-buffer
      (bibtex-mode)
      (insert "@article{key,\n  title = {A \\} B},\n  year = {2024}\n}\n")
      (goto-char (point-min))
      (let ((entry (bibtex-parse-entry t)))
        (list (cdr (assoc "title" entry))
              (cdr (assoc "year" entry)))))
    '("A \\} B" "2024"))

(qa (with-temp-buffer
      (bibtex-mode)
      (insert "@article{key,\n  title = {A \\{ B},\n  year = {2024}\n}\n")
      (goto-char (point-min))
      (let ((entry (bibtex-parse-entry t)))
        (list (cdr (assoc "title" entry))
              (cdr (assoc "year" entry)))))
    '("A \\{ B" "2024"))

(princ (qa--done))
