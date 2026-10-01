;; System/build/time/GC domain — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - emacs-major-version/emacs-minor-version are plain integers.
;;   - (emacs-init-time) returns a STRING "N seconds" (not a list).
;;   - before-init-time/after-init-time are TIMESTAMP lists.
;;   - (emacs-uptime) returns a STRING like "0 seconds".
;;   - (load-average) returns nil in batch (-Q).
;;   - decode-time is a 9-element list; encode-time roundtrips to a
;;     (TICKS . HZ) cons.
;;   - (time-convert TIME 'list) returns the canonical 4-int list.
;;   - user-real-login-name is a STRING (not a number).
;;   - mail-host-name, pure-space-overflow are UNBOUND in a batch -Q build.
;;   - emacs-repository-* nil for a release tarball.

;; == version ==
(qa (integerp emacs-major-version) t)
(qa (integerp emacs-minor-version) t)
(qa (numberp (+ (* 1000 emacs-major-version) emacs-minor-version)) t)
;; emacs-version variable starts with "31"
(qa (string-prefix-p "31" emacs-version) t)
;; (emacs-version) returns a fuller string starting with "GNU Emacs"
(qa (string-prefix-p "GNU Emacs" (emacs-version)) t)
;; repo metadata is set on this unstable build
(qa emacs-repository-version "emacs-31.1")
;; remacs: branch not recorded → nil
(qa emacs-repository-branch "master")
;; remacs: invocation-directory/name are nil under --batch
(qa (stringp invocation-directory) t)
(qa (stringp invocation-name) t)
;; site-run-file unbound/nil under -Q
(qa site-run-file nil)
;; system-configuration is a string
(qa (stringp system-configuration) t)
;; system-name is a string
(qa (stringp (system-name)) t)
;; system-groups / system-users are lists (possibly empty)
(qa (listp (system-groups)) t)
(qa (listp (system-users)) t)

;; == init/uptime/loads ==
;; (emacs-init-time) → "N seconds" string
(qa (stringp (emacs-init-time)) t)
;; before-init-time/after-init-time are TIMESTAMP lists (not strings)
(qa (listp before-init-time) t)
(qa (listp after-init-time) t)
(qa (listp (load-average)) t)
(qa (= 3 (length (load-average))) t)
;; (daemonp) → nil in a non-daemon batch
(qa (daemonp) nil)

;; == command line ==
(qa (listp command-line-args) t)
;; under -Q batch command-line-args is empty? GNU pin: empty list
(qa (listp command-line-args-left) t)

;; == GC ==
(qa (numberp gc-cons-percentage) t)
(qa (integerp gc-cons-threshold) t)
;; gc-elapsed is a float
(qa (floatp gc-elapsed) t)
(qa (integerp gcs-done) t)
;; (garbage-collect) returns a 9-element stats list
(qa (length (garbage-collect)) 9)
(qa (listp (garbage-collect)) t)
;; each entry is a cons (name . count) shape: head is a symbol
(qa (symbolp (caar (garbage-collect))) t)
;; (memory-use-counts) is a list of ints
(qa (listp (memory-use-counts)) t)
;; (memory-limit) returns an integer (bytes)
(qa (integerp (memory-limit)) t)
;; (memory-info) returns nil on macOS (no sysinfo syscall)
(qa (memory-info) nil)
(qa (boundp 'purify-flag) t)
(qa purify-flag nil)
;; pure-space-overflow is UNBOUND in a non-pure build
(qa (boundp 'pure-space-overflow) nil)

;; == process/system enumeration ==
(qa (numberp (emacs-pid)) t)
(qa (listp (list-system-processes)) t)
(qa (listp (process-attributes (emacs-pid))) t)
;; (process-attributes PID) → alist with 'euid key? check shape
(qa (consp (car (process-attributes (emacs-pid)))) t)

;; == user identity ==
(qa (numberp (user-uid)) t)
(qa (numberp (user-real-uid)) t)
(qa (stringp (user-login-name)) t)
;; user-real-login-name is a STRING (not a number)
(qa (stringp (user-real-login-name)) t)
;; user-full-name is a string (possibly empty, but a string)
(qa (stringp (user-full-name)) t)

;; == time primitives ==
;; current-time: 4-int list
(qa (listp (current-time)) t)
(qa (length (current-time)) 4)
;; float-time returns a float
(qa (floatp (float-time)) t)
;; float-time '(0 0 0 0) = 0.0
(qa (float-time '(0 0 0 0)) 0.0)
;; decode-time: 9-element list
(qa (length (decode-time)) 9)
;; decode-time '(0 0 0 0) → fields (sec min hour day month year dow dst off)
;; epoch UTC: (0 0 0 1 1 1970 4 - 0) — hour is LOCAL, so only pin
;; the timezone-independent fields and types
(let ((d (decode-time '(0 0 0 0))))
  (qa (list (nth 0 d) (nth 3 d) (nth 4 d) (nth 5 d) (nth 6 d))
      '(0 1 1 1970 4)))
;; encode-time roundtrip → cons
(qa (consp (encode-time (decode-time '(0 0 0 0)))) t)
;; time-less-p / time-equal-p
(qa (time-less-p '(0 0 0 0) '(1 0 0 0)) t)
(qa (time-less-p '(1 0 0 0) '(0 0 0 0)) nil)
(qa (time-equal-p '(1 0 0 0) '(1 0 0 0)) t)
(qa (time-equal-p '(1 0 0 0) '(2 0 0 0)) nil)
;; time-add/time-subtract
(qa (car (time-add '(0 0 0 0) '(1 0 0 0))) 1)
(qa (car (time-subtract '(2 0 0 0) '(1 0 0 0))) 1)
;; time-to-days: epoch = 719163
(qa (time-to-days '(0 0 0 0)) 719163)
;; days-to-time → list
(qa (listp (days-to-time 1)) t)
;; time-convert 'list roundtrips
(qa (time-convert '(0 0 0 0) 'list) '(0 0 0 0))
(qa (time-convert 0 'list) '(0 0 0 0))
;; time-convert integer → integer
(qa (time-convert '(0 0 0 0) 'integer) 0)

;; == format-time-string (numeric formats only — %A/%B are locale) ==
(qa (format-time-string "%Y-%m-%d %H:%M:%S" '(0 0 0 0) "UTC")
    "1970-01-01 00:00:00")
(qa (format-time-string "%j" '(0 0 0 0) "UTC") "001")
(qa (format-time-string "%U %W" '(0 0 0 0) "UTC") "00 00")
(qa (format-time-string "%F %T" '(0 0 0 0) "UTC")
    "1970-01-01 00:00:00")
(qa (format-time-string "%s" '(0 0 0 0) "UTC") "0")
(qa (format-time-string "%e" '(0 0 0 0) "UTC") " 1")
;; %F+%T roundtrip on a recent timestamp
(qa (string-match-p "\\`[0-9]\\{4\\}-[0-9]\\{2\\}-[0-9]\\{2\\}"
                    (format-time-string "%F" nil "UTC"))
    0)
;; current-time-string returns a string
(qa (stringp (current-time-string)) t)
(qa (stringp (current-time-string '(0 0 0 0))) t)
;; GNU honors the ZONE arg; remacs ignores it → xfail
(qa (let ((z (current-time-zone '(0 0 0 0) "UTC")))
      (list (car z) (cadr z)))
    '(0 "UTC"))
;; with nil ZONE → local; type only
(qa (listp (current-time-zone)) t)

;; == decode-time DST field ==
;; (nth 7 (decode-time)) → DST flag; UTC has nil? probe: print and pin
;; via a fixed epoch where DST is well-defined. Use TZ arg.
(let ((d (decode-time '(0 0 0 0) "UTC")))
  ;; hour field local: with explicit UTC it's 0
  (qa (nth 2 d) 0)
  (qa (nth 7 d) nil)   ;; UTC has no DST
  (qa (nth 8 d) 0))    ;; offset 0

;; == sleep-for timing ==
;; sleep-for returns nil
(qa (sleep-for 0.001) nil)

;; == set-time-zone-rule ==
;; POSIX TZ string; returns nil? pin return + effect on format-time-string
;; set-time-zone-rule + format-time-string %Z — remacs keeps local TZ
(qa (progn
      (set-time-zone-rule "UTC0")
      (format-time-string "%Z" '(0 0 0 0)))
    "UTC")
(qa (progn
      (set-time-zone-rule "EST5")
      (format-time-string "%Z" '(0 0 0 0)))
    "EST")
(set-time-zone-rule "UTC0")

;; == getenv/setenv ==
(progn
  (setenv "QA_TEST_VAR" "hello")
  (qa (getenv "QA_TEST_VAR") "hello")
  (setenv "QA_TEST_VAR" nil) ; unset → getenv returns nil
  (qa (getenv "QA_TEST_VAR") nil)
  ;; plain string value roundtrip
  (setenv "QA_TEST_VAR2" "/a/b")
  (qa (getenv "QA_TEST_VAR2") "/a/b"))

;; == getenv edge ==
(qa (getenv "QA_NONEXISTENT_XYZ_123") nil)

;; == misc ==
;; command-line-args head: GNU has program name, remacs empty under -Q
(qa (stringp (car command-line-args)) t)
;; kill-emacs is a function (don't call it!)
(qa (fboundp 'kill-emacs) t)
(qa (fboundp 'emacs-pid) t)

(princ (qa--done))
