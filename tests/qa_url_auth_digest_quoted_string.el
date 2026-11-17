;;; qa_url_auth_digest_quoted_string.el --- reject controls in Digest headers -*- lexical-binding: t; -*-

(condition-case nil
    (load-file "lisp/url-auth.el")
  ;; The file has a separate tracked end-of-file issue after these helpers.
  (end-of-file nil))

(defun qa--url-digest-auth-response (username)
  (url-digest-auth-build-response
   (list nil username "01234567890123456789012345678901")
   (url-generic-parse-url "http://example.com/path")
   "test-realm"
   '(("nonce" . "test-nonce"))))

(qa (url-digest-auth-name-value-string
     '((username . "Mufasa") (realm . "test-realm")))
    "username=\"Mufasa\", realm=\"test-realm\"")

;; HTAB is permitted in an HTTP quoted-string.
(qa (url-digest-auth-name-value-string
     (list (cons 'username (concat "a" (string 9) "b"))))
    (concat "username=\"a" (string 9) "b\""))

(qa (condition-case nil
        (progn
          (url-digest-auth-name-value-string
           (list (cons 'username
                       (concat "user" (string 13 10) "X-Injected: yes"))))
          nil)
      (error t))
    t)

(qa (condition-case nil
        (progn
          (qa--url-digest-auth-response
           (concat "user" (string 13 10) "X-Injected: yes"))
          nil)
      (error t))
    t)

(princ (qa--done))
