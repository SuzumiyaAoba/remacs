;;; audit_match_data.el --- reuse and reseating of match data -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "match-data:reuse-integers"
 '(progn
    (string-match ".." "abc")
    (let ((reuse (match-data t)))
      (string-match "b" "abc")
      (let ((out (match-data t reuse)))
        (list (eq out reuse) out reuse)))))

(audit-check "match-data:reuse-long-list"
 '(let ((reuse (make-list 6 'old)))
    (string-match "a" "a")
    (let ((out (match-data t reuse)))
      (list (eq out reuse) out reuse))))

(audit-check "match-data:reseat-old-markers"
 '(with-temp-buffer
    (insert "ab") (goto-char 1) (re-search-forward "a")
    (let* ((reuse (match-data)) (start (car reuse)) (end (cadr reuse)))
      (string-match "b" "b")
      (let ((out (match-data t reuse t)))
        (list (eq out reuse) (null (marker-buffer start)) (null (marker-buffer end))
              (mapcar (lambda (x) (if (integerp x) x 'marker)) reuse) out)))))

(princ "AUDIT-DONE\n")
