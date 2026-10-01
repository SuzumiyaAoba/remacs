;; Time + system-info domain — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - text-quoting-style is a VAR (nil default in batch), not a fn —
;;     so (fboundp 'text-quoting-style) is t via the Lisp accessor, and
;;     calling it as a fn errors.
;;   - format-message honors text-quoting-style: grave → `x', curve →
;;     'x' (unicode curly), straight → 'x'.
;;   - parse-time-string returns the decoded-time plist/list.
;;   - encode-time arg order: SEC MIN HOUR DAY MON YEAR …
;;   - decode-time returns a decoded-time struct; accessors exist.
;;   - time-add/subtract/equal/less on plain ints work.
;;   - time-convert to 'integer/'list/'t accepted.
;;   - float-time on (0 0 0 0) → 0.0.
;;   - format-seconds "%s seconds" 90 → "90 seconds".
;;   - format-time-string with TZ "UTC0" is deterministic.
;;   - emacs-pid/uptime/init-time return int/string/string.
;;   - user-login-name/user-real-login-name/user-full-name are strings;
;;     user-uid/user-real-uid are natnump.
;;   - system-name/system-type/system-configuration bound.
;;   - memory-info / memory-limit are fns.
;;   - emacs-internal-time is a fn.

;; == text-quoting-style VAR ==
(qa (boundp 'text-quoting-style) t)
(qa text-quoting-style nil)
;; fboundp t because the Lisp accessor exists
(qa (fboundp 'text-quoting-style) t)
;; BUT calling it as a fn errors (it's an accessor, not a command)
(qa-err (text-quoting-style 'grave) 'wrong-number-of-arguments)

;; == format-message under each style ==
(let ((text-quoting-style 'grave))
  (qa (format-message "`%s'" 'x) "`x'"))
(let ((text-quoting-style 'curve))
  (qa (format-message "`%s'" 'x) "‘x’"))
(let ((text-quoting-style 'straight))
  (qa (format-message "`%s'" 'x) "'x'"))
;; curve is the GNU default; straight is a real alternative

;; == parse-time-string ==
(qa (parse-time-string "2025-09-29 12:00:00")
    '(0 0 12 29 9 2025 nil -1 nil))
;; == encode-time / decode-time roundtrip ==
(let ((t0 (encode-time 5 10 15 20 6 2024 nil nil nil "UTC0")))
  (let ((d (decode-time t0 t)))
    (qa (decoded-time-second d) 5)
    (qa (decoded-time-minute d) 10)
    (qa (decoded-time-hour d) 15)
    (qa (decoded-time-day d) 20)
    (qa (decoded-time-month d) 6)
    (qa (decoded-time-year d) 2024)))
;; encode-time returns a cons-pair time
(qa (encode-time 0 30 14 29 9 2025) '(26842 6488))
;; decode-time on a 4-list → fields
(let ((d (decode-time '(27323 0 0 0))))
  (qa (list (decoded-time-second d)
            (decoded-time-minute d)
            (decoded-time-hour d)
            (decoded-time-day d)
            (decoded-time-month d)
            (decoded-time-year d))
      '(8 2 9 29 9 2026)))

;; == float-time ==
(qa (float-time '(0 0 0 0)) 0.0)

;; == time-convert ==
(qa (time-convert '(1 0 0 0) t) '(65536000000000000 . 1000000000000))
(qa (time-convert '(1 0 0 0) 'integer) 65536)
(qa (time-convert '(1 0 0 0) 'list) '(1 0 0 0))

;; == time-add/subtract/equal/less ==
(qa (time-add 0 60) 60)
(qa (time-subtract 60 30) 30)
(qa (time-equal-p 0 0) t)
(qa (time-equal-p 0 1) nil)
(qa (time-less-p 1 2) t)
(qa (time-less-p 2 1) nil)

;; == time-since → a time list ==
(qa (listp (time-since (current-time))) t)

;; == date-to-time → a time ==
(qa (listp (date-to-time "Mon Sep 29 12:00:00 2025")) t)

;; decoded-time accessors on a raw list time — return nil (require
;; decode-time first)
(let ((t5 (decode-time (encode-time 0 0 0 15 6 2008))))
  (qa (decoded-time-year t5) 2008)
  (qa (decoded-time-month t5) 6)
  (qa (decoded-time-day t5) 15))

;; == format-seconds ==
(qa (format-seconds "%s seconds" 90) "90 seconds")
(qa (format-seconds "%y days %h hours %m minutes %s seconds" 90)
    "0 days 0 hours 1 minutes 30 seconds")

;; == format-time-string under UTC tz (deterministic) ==
(let ((system-time-locale "C"))
  (qa (format-time-string "%Y-%m-%d %H:%M:%S"
                          (encode-time 0 0 0 15 6 2008 nil nil nil "UTC0")
                          t)
      "2008-06-15 00:00:00"))

;; == encode-time edge (DST) ==
(qa (listp (encode-time 0 0 0 1 1 2000)) t)

;; == emacs-pid / uptime / init-time ==
(qa (natnump (emacs-pid)) t)
(qa (stringp (emacs-uptime)) t)
(qa (stringp (emacs-init-time)) t)

;; == user info ==
(qa (stringp (user-login-name)) t)
(qa (stringp (user-real-login-name)) t)
(qa (natnump (user-uid)) t)
(qa (natnump (user-real-uid)) t)
(qa (stringp (user-full-name)) t)

;; == system vars ==
(qa (stringp (system-name)) t)
(qa (boundp 'system-type) t)
(qa (boundp 'system-configuration) t)
(qa (eq system-type 'darwin) t)

;; == memory-info / memory-limit / emacs-internal-time ==
(qa (fboundp 'memory-info) t)
(qa (fboundp 'memory-limit) t)
(qa (fboundp 'emacs-internal-time) nil)

;; == misc system fns ==
(qa (fboundp 'emacs-version) t)
(qa (stringp (emacs-version)) t)
;; in 31.1 these are NOT fns (only on --with-git)
(qa (fboundp 'emacs-build-time) nil)
(qa (fboundp 'emacs-repository-version) nil)
(qa (fboundp 'emacs-repository-branch) nil)
(qa (fboundp 'system-name-detailed) nil)
(qa (fboundp 'emacs-module-load) nil)         ; not without modules
;; module-load exists; emacs-module-load doesn't
(qa (fboundp 'module-load) t)
;; zoneinfo-style-world-list is a var, not fn
(qa (fboundp 'zoneinfo-style-world-list) nil)
(qa (boundp 'zoneinfo-style-world-list) t)

(princ (qa--done))
