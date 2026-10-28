;;; Cookie expiry dates use UTC regardless of timezone tokens (Issue #1808).

;; Load the cookie helper without unrelated URL dependencies and exercise
;; parse-time's RFC-date fallback without loading the broken ISO parser (#1218).
(provide 'url-util)
(provide 'url-parse)
(provide 'url-domsuf)
(provide 'generate-lisp-file)
(require 'url-cookie)
(provide 'iso8601)
(defun iso8601-parse (&rest _arguments)
  (signal 'wrong-type-argument (list 'iso8601-time-string)))
(require 'parse-time)

(qa (list
     (format-time-string
      "%Y-%m-%d %H:%M:%S"
      (url-cookie--expires-date-to-time "Wed, 09 Jun 2021 10:18:14 PST")
      t)
     (format-time-string
      "%Y-%m-%d %H:%M:%S"
      (url-cookie--expires-date-to-time "Wed, 09 Jun 2021 10:18:14 +0800")
      t))
    '("2021-06-09 10:18:14" "2021-06-09 10:18:14"))

(princ (qa--done))
