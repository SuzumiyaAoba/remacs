;;; Cookie expiry dates use UTC regardless of timezone tokens (Issue #1808).

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

(qa (list
     (format-time-string
      "%Y-%m-%d %H:%M:%S"
      (url-cookie--expires-date-to-time "Wed, 09 Jun 2021 10:18:14 PST")
      t)
     (format-time-string
      "%Y-%m-%d %H:%M:%S"
      (url-cookie--expires-date-to-time "Wed, 09 Jun 2021 10:18:14 +0800")
      t))
    '("2021-06-09 10:18:14" "2021-06-09 10:18:14"))

(defun qa-url-cookie--netscape-expiry (timestamp &optional long-session)
  (let ((file (make-temp-file "qa-url-cookie-expiry-"))
        (url-cookie-storage nil)
        (url-cookie-secure-storage nil)
        (url-cookies-changed-since-last-save nil))
    (unwind-protect
        (progn
          (with-temp-file file
            (insert "# Netscape HTTP Cookie File\n"
                    (format "example.com\tFALSE\t/\tFALSE\t%s\tsid\tvalue\n"
                            timestamp)))
          (let ((before (floor (float-time))))
            (url-cookie-parse-file-netscape file long-session)
            (list before
                  (format-time-string
                   "%s"
                   (url-cookie--expires-date-to-time
                    (url-cookie-expires (cadar url-cookie-storage)))
                   t))))
      (delete-file file))))

(let ((original-zone (getenv "TZ")))
  (unwind-protect
      (progn
        (set-time-zone-rule "JST-9")
        (qa (cadr (qa-url-cookie--netscape-expiry "1462299217"))
            "1462299217")
        (let* ((expiry (qa-url-cookie--netscape-expiry "0" t))
               (expected (+ (car expiry) (* 365 24 60 60)))
               (actual (string-to-number (cadr expiry))))
          (qa (<= (abs (- actual expected)) 2) t)))
    (set-time-zone-rule original-zone)))

(princ (qa--done))
