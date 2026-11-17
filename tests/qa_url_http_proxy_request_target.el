;;; qa_url_http_proxy_request_target.el --- sanitize proxy absolute-form targets -*- lexical-binding: t; -*-

(require 'cl-lib)

;; url-auth.el has an unrelated incomplete tail in this checkout.
(condition-case nil
    (load-file (expand-file-name "../lisp/url-auth.el"
                                 (file-name-directory load-file-name)))
  (end-of-file nil))
(provide 'url-auth)
(require 'url-http)

(defun qa--url-http-proxy-request-line (target-url)
  (let ((url-http-target-url (copy-sequence target-url))
        (url-http-extra-headers nil)
        (url-http-proxy (url-generic-parse-url "http://proxy.example:8080"))
        (url-http-referer nil)
        (url-http-method "GET")
        (url-http-data nil)
        (url-http-version "1.1")
        (url-http-attempt-keepalives t)
        (url-http-extensions-header nil)
        (url-mime-accept-string "*/*")
        (url-mime-language-string nil)
        (url-mime-charset-string nil))
    (setf (url-use-cookies url-http-target-url) nil)
    (cl-letf (((symbol-function 'url-get-authentication)
               (lambda (&rest _) nil))
              ((symbol-function 'url-http-user-agent-string)
               (lambda () "")))
      (let ((request (url-http-create-request)))
        (substring request 0 (string-match "\r\n" request))))))

(qa (let* ((target
            (url-generic-parse-url
             "http://alice:secret@example.com/private?token=secret#private-frag"))
           (request-line (qa--url-http-proxy-request-line target)))
      (list request-line (url-user target) (url-password target) (url-target target)))
    '("GET http://example.com/private?token=secret HTTP/1.1"
      "alice" "secret" "private-frag"))

(princ (qa--done))
