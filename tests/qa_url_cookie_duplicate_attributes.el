;;; -*- lexical-binding: t -*-
;; RFC 6265 regressions for #3390/#1878.  Exercise the real parser, storage,
;; domain/path filtering, and expiry checks, including rejected attributes.
(require 'url-cookie)

(defun qa-url-cookie--duplicates (header)
  (let ((url-cookie-storage nil)
        (url-cookie-secure-storage nil)
        (url-cookie-confirmation nil)
        (url-cookie-trusted-urls nil)
        (url-cookie-untrusted-urls nil)
        (url-privacy-level nil)
        (url-current-object
         (url-generic-parse-url "http://example.com/dir/file")))
    (url-cookie-handle-set-cookie header)
    (list (length (url-cookie-retrieve "example.com" "/dir/file"))
          (length (url-cookie-retrieve "example.com" "/first/file"))
          (length (url-cookie-retrieve "example.com" "/second/file"))
          (length (url-cookie-retrieve "sub.example.com" "/dir/file")))))

(qa (qa-url-cookie--duplicates "sid=v; Max-Age=0; mAx-AgE=3600") '(1 0 0 0))
(qa (qa-url-cookie--duplicates "sid=v; Max-Age=3600; Max-Age=0") '(0 0 0 0))
(qa (qa-url-cookie--duplicates
     "sid=v; Expires=Thu, 01 Jan 1970 00:00:00 GMT; eXpIrEs=Fri, 31 Dec 9999 23:59:59 GMT")
    '(1 0 0 0))
(qa (qa-url-cookie--duplicates
     "sid=v; Expires=Fri, 31 Dec 9999 23:59:59 GMT; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(0 0 0 0))
(qa (qa-url-cookie--duplicates "sid=v; Path=/first; pAtH=/second") '(0 0 1 0))
(qa (qa-url-cookie--duplicates "sid=v; Domain=other.com; dOmAiN=example.com")
    '(1 0 0 1))
(qa (qa-url-cookie--duplicates "sid=v; Domain=example.com; Domain=other.com")
    '(0 0 0 0))

;; Ignored attributes must not replace an earlier valid value.  Invalid Path
;; values instead select the request's default path, as the RFC specifies.
(qa (qa-url-cookie--duplicates "sid=v; Max-Age=0; Max-Age=invalid") '(0 0 0 0))
(qa (qa-url-cookie--duplicates
     "sid=v; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Expires=invalid")
    '(0 0 0 0))
(qa (qa-url-cookie--duplicates "sid=v; Domain=example.com; Domain=") '(1 0 0 1))
(qa (qa-url-cookie--duplicates "sid=v; Path=/first; Path=invalid") '(1 0 0 0))

;; The cookie name is not an attribute, even if it has a reserved spelling.
(qa (qa-url-cookie--duplicates "Max-Age=0") '(1 0 0 0))
(qa (qa-url-cookie--duplicates "Secure=v") '(1 0 0 0))
(qa (mapcar #'car (url-parse-args "sid=v; Path=/first; Path=/second" t))
    '("sid" "Path" "Path"))

(princ (qa--done))
