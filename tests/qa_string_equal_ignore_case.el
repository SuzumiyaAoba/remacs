;;; -*- lexical-binding: nil; -*-
;; string-equal-ignore-case delegates to compare-strings (#434).
(qa (list (string-equal-ignore-case "Σ" "ς")
          (string-equal-ignore-case "İ" "i\u0307")
          (string-equal-ignore-case (unibyte-string 233) "é"))
    '(t nil nil))
(let ((table (copy-case-table (standard-case-table))))
  (set-case-syntax-pair ?X ?y table)
  (with-temp-buffer
    (set-case-table table)
    (qa (string-equal-ignore-case "X" "y") t)))
(princ (qa--done))
