;;; Apply the cookie year rule to 3- and 4-digit tokens (Issue #1806).

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

(qa (mapcar
     (lambda (date)
       (format-time-string "%Y"
                           (url-cookie--expires-date-to-time date)
                           t))
     '("Wed, 09 Jun 050 10:18:14 GMT"
       "Wed, 09 Jun 0050 10:18:14 GMT"
       "Wed, 09 Jun 50 10:18:14 GMT"
       "Wed, 09 Jun 069 10:18:14 GMT"
       "Wed, 09 Jun 0069 10:18:14 GMT"
       "Wed, 09 Jun 69 10:18:14 GMT"))
    '("2050" "2050" "2050" "2069" "2069" "2069"))

(princ (qa--done))
