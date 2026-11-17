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
        (url-cookie-file nil)
        (url-cookie-save-interval nil)
        (url-cookies-changed-since-last-save nil)
        (url-current-object (url-generic-parse-url "http://example.com/")))
    (url-cookie-handle-set-cookie (concat "sid=value" attributes))
    (list (length url-cookie-storage)
          (length (url-cookie-retrieve "example.com" "/"))
          (url-cookie-generate-header-lines "example.com" "/" nil))))

;; Every non-positive age expires, including negative zero and values too
;; large for a civil timestamp.  Expired cookies are evicted and not sent
;; (#2588).
(qa (mapcar (lambda (age)
              (qa-url-cookie--max-age (concat "; Max-Age=" age)))
            (list "0" "-0" "-1" "-0001" "-2000000000"
                  (concat "-" (make-string 80 ?9))
                  (concat "-" (make-string 101 ?9))))
    (make-list 7 '(0 0 "")))

;; Session cookies stay live.  Valid decimal Max-Age overrides Expires,
;; whereas invalid Max-Age falls back to Expires.
(qa (qa-url-cookie--max-age "") '(1 1 "Cookie: sid=value\r\n"))
(qa (qa-url-cookie--max-age
     "; Max-Age=0003600; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(1 1 "Cookie: sid=value\r\n"))
(qa (qa-url-cookie--max-age
     "; Max-Age=invalid; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(0 0 ""))

(qa (qa-url-cookie--max-age "; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
    '(0 0 ""))

(defun qa-url-cookie--list-evicts-expired ()
  (let* ((expired
          (url-cookie-create :name "sid" :value "x"
                             :expires "Thu, 01 Jan 1970 00:00:00 GMT"
                             :localpart "/" :domain "example.com" :secure nil))
         (live
          (url-cookie-create :name "live" :value "y" :expires nil
                             :localpart "/" :domain "example.com" :secure nil))
         (url-cookie-storage (list (list "example.com" expired live)))
         (url-cookie-secure-storage nil)
         (url-cookie-file nil)
         (url-cookie-save-interval nil)
         (url-cookies-changed-since-last-save nil))
    (url-cookie-list)
    (with-current-buffer "*url cookies*"
      (let ((contents (buffer-string)))
        (list (mapcar #'url-cookie-name
                      (cdr (assoc-string "example.com" url-cookie-storage t)))
              (and (string-match-p "sid" contents) t)
              (and (string-match-p "live" contents) t)
              url-cookies-changed-since-last-save)))))

(qa (qa-url-cookie--list-evicts-expired) '(("live") nil t t))

(princ (qa--done))
