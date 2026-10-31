;;; -*- lexical-binding: t -*-
;; RFC 6265 regressions for #3391 and #1866.  The overflow cases intentionally
;; improve on GNU 31.1, which turns these expired cookies into session cookies.
(require 'url-cookie)

(defun qa-url-cookie--max-age (attributes)
  (let ((url-cookie-storage nil)
        (url-cookie-secure-storage nil)
        (url-cookie-confirmation nil)
        (url-cookie-trusted-urls nil)
        (url-cookie-untrusted-urls nil)
        (url-privacy-level nil)
        (url-current-object (url-generic-parse-url "http://example.com/")))
    (url-cookie-handle-set-cookie (concat "sid=value" attributes))
    (let ((cookie (cadar url-cookie-storage)))
      (list (and (url-cookie-expires cookie) t)
            (and (url-cookie-expired-p cookie) t)
            (length (url-cookie-retrieve "example.com" "/"))))))

;; Every non-positive age expires, including negative zero and values too
;; large for a civil timestamp.  Expired cookies must not be sent back.
(qa (mapcar (lambda (age)
              (qa-url-cookie--max-age (concat "; Max-Age=" age)))
            (list "0" "-0" "-1" "-0001" "-2000000000"
                  (concat "-" (make-string 80 ?9))
                  (concat "-" (make-string 101 ?9))))
    (make-list 7 '(t t 0)))

;; Session cookies stay live.  Valid decimal Max-Age overrides Expires,
;; whereas invalid Max-Age falls back to Expires.
(qa (qa-url-cookie--max-age "") '(nil nil 1))
(qa (qa-url-cookie--max-age
     "; Max-Age=0003600; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(t nil 1))
(qa (qa-url-cookie--max-age
     "; Max-Age=invalid; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(t t 0))

(princ (qa--done))
