;;; Parse valid hyphen-delimited cookie dates (Issue #1809).

;; Load the cookie helper without loading URL dependencies whose date parser
;; path is tracked separately by Issue #1218.
(provide 'url-util)
(provide 'url-parse)
(provide 'url-domsuf)
(provide 'generate-lisp-file)
(require 'url-cookie)

;; Use parse-time's RFC-date fallback without loading the broken ISO parser
;; tracked separately by Issue #1218.
(provide 'iso8601)
(defun iso8601-parse (&rest _arguments)
  (signal 'wrong-type-argument (list 'iso8601-time-string)))
(require 'parse-time)

(qa (format-time-string
     "%Y-%m-%d %H:%M:%S"
     (date-to-time "09 Jun 2021 10:18:14 GMT")
     t)
    "2021-06-09 10:18:14")

(qa (format-time-string
     "%Y-%m-%d %H:%M:%S"
     (url-cookie--expires-date-to-time "09-Jun-2021 10:18:14 GMT")
     t)
    "2021-06-09 10:18:14")

(princ (qa--done))
