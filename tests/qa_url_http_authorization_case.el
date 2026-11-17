;;; qa_url_http_authorization_case.el --- match auth fields without regard to case -*- lexical-binding: t; -*-

(require 'cl-lib)

;; url-auth.el has an unrelated incomplete tail in this checkout; the Digest
;; helpers used below are defined before it.
(condition-case nil
    (load-file (expand-file-name "../lisp/url-auth.el"
                                 (file-name-directory load-file-name)))
  (end-of-file nil))
(provide 'url-auth)
(require 'url-http)

(defun qa--url-http-request-with-headers (headers)
  (let ((url-http-target-url (url-generic-parse-url "http://example.com/path"))
        (url-http-extra-headers headers)
        (url-http-proxy nil)
        (url-http-referer nil)
        (url-http-method "GET")
        (url-http-data nil)
        (url-http-extensions-header nil)
        (url-mime-accept-string "*/*")
        (url-mime-language-string nil)
        (url-mime-charset-string nil))
    (setf (url-use-cookies url-http-target-url) nil)
    (cl-letf (((symbol-function 'url-get-authentication)
               (lambda (&rest _) "Basic saved"))
              ((symbol-function 'url-http-user-agent-string)
               (lambda () "")))
      (url-http-create-request))))

(qa (url-http--find-header
     "Authorization" '(("authorization" . "Bearer custom")))
    '("authorization" . "Bearer custom"))
(qa (url-http--find-header
     "Authorization" '(("aUtHoRiZaTiOn" . "Bearer custom")))
    '("aUtHoRiZaTiOn" . "Bearer custom"))
(qa (url-http--remove-header
     "Authorization"
     '(("authorization" . "Bearer custom")
       ("X-Test" . "keep")
       ("AUTHORIZATION" . "Basic secret")))
    '(("X-Test" . "keep")))

(qa (let ((request
           (qa--url-http-request-with-headers
            '(("authorization" . "Bearer custom")))))
      (list (and (string-match-p "authorization: Bearer custom" request) t)
            (and (string-match-p "Authorization: Basic saved" request) t)))
    '(t nil))

(princ (qa--done))
