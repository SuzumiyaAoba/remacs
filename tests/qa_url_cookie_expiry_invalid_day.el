;;; Reject an invalid cookie day instead of treating it as a year.

;; Load url-cookie's helper without loading its unrelated URL dependencies;
;; url-parse's date-time path is covered by Issue #1218.
(provide 'url-util)
(provide 'url-parse)
(provide 'url-domsuf)
(provide 'generate-lisp-file)
(require 'url-cookie)

(qa-err (url-cookie--expires-date-to-time "Thu, 50 Jan 10:18:14 GMT") 'error)

(princ (qa--done))
