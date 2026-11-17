;;; qa_url_http_referer_credentials.el --- omit URI credentials and fragments -*- lexical-binding: t; -*-

;; url-auth.el has an unrelated incomplete tail in this checkout.
(condition-case nil
    (load-file (expand-file-name "../lisp/url-auth.el"
                                 (file-name-directory load-file-name)))
  (end-of-file nil))
(provide 'url-auth)
(require 'url-http)

(qa (let ((url-privacy-level nil)
          (url-lastloc-privacy-level 'domain-match)
          (url-current-lastloc
           (url-generic-parse-url
            "https://user:secret@example.com/private?token=secret#private-frag")))
      (url-http--get-referer
       (url-generic-parse-url "https://sub.example.com/next")))
    "https://example.com/private?token=secret")

(qa (let ((url-privacy-level nil)
          (url-lastloc-privacy-level 'host-match)
          (url-current-lastloc
           (url-generic-parse-url "http://user:secret@example.com/path#private")))
      (url-http--get-referer
       (url-generic-parse-url "http://example.com/next")))
    "http://example.com/path")

(princ (qa--done))
