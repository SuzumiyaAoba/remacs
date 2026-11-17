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

(defun qa--url-http-request-with-headers (headers &optional proxy)
  (let ((url-http-target-url (url-generic-parse-url "http://example.com/path"))
        (url-http-extra-headers headers)
        (url-http-proxy proxy)
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

;; Proxy-Authorization is omitted for direct requests but kept for a proxy.
(qa (let ((request
           (qa--url-http-request-with-headers
            '(("proxy-authorization" . "Basic proxy-secret")))))
      (and (not (string-match-p
                 "proxy-authorization: Basic proxy-secret" request))
           (not (string-match-p "Proxy-Authorization: Basic saved" request))))
    t)
(qa (let ((request
           (qa--url-http-request-with-headers
            '(("proxy-authorization" . "Basic proxy-secret"))
            "http://proxy.example:8080")))
      (list (and (string-match-p
                  "proxy-authorization: Basic proxy-secret" request) t)
            (and (string-match-p "Proxy-Authorization: Basic saved" request) t)))
    '(t nil))
;; Redirect filtering strips both field names case-insensitively.
(qa (url-http--remove-authentication-headers
     '(("authorization" . "Bearer origin")
       ("proxy-authorization" . "Basic proxy")
       ("X-Test" . "keep")))
    '(("X-Test" . "keep")))

(princ (qa--done))
