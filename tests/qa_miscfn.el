;; type-of / env / misc builtins — GNU 31.1 -Q --batch ground truth.

;; type-of lattice (GNU: keymap → cons, frame → frame, nil → symbol)
(qa (type-of 1) 'integer)
(qa (type-of 1.5) 'float)
(qa (type-of "s") 'string)
(qa (type-of 'a) 'symbol)
(qa (type-of nil) 'symbol)
(qa (type-of t) 'symbol)
(qa (type-of :kw) 'symbol)
(qa (type-of (cons 1 2)) 'cons)
(qa (type-of '(1)) 'cons)
(qa (type-of [1]) 'vector)
(qa (type-of (symbol-function 'car)) 'subr)
(qa (type-of (current-buffer)) 'buffer)
(qa (type-of (make-marker)) 'marker)
(qa (type-of (make-hash-table)) 'hash-table)
(qa (type-of (selected-window)) 'window)
(qa (type-of (make-keymap)) 'cons)
(qa (type-of (make-sparse-keymap)) 'cons)
(qa (type-of #'when) 'symbol) ;; symbol arg echoes symbol even if macro
(qa (type-of (record 'tag 1 2)) 'tag) ;; record type-of → first slot
(qa (type-of (selected-frame)) 'frame)
(qa-err (type-of) 'wrong-number-of-arguments)

;; recordp / record accessors
(let ((r (record 'rectag 10 "x")))
  (qa (recordp r) t)
  (qa (recordp '(a)) nil)
  (qa (length r) 3)
  (qa (aref r 0) 'rectag)
  (qa (aref r 2) "x"))

;; macrop / special-form-p distinctions
(qa (macrop 'when) t)
(qa (macrop (symbol-function 'when)) t)
(qa (macrop (symbol-function 'if)) nil) ;; special form, not macro
(qa (macrop (symbol-function 'car)) nil)
(qa (special-form-p 'if) t)
(qa (special-form-p 'let) t)
(qa (special-form-p 'quote) t)
(qa (special-form-p 'interactive) t)
(qa (special-form-p 'track-mouse) nil) ;; macro in 31.1
(qa (special-form-p 'dolist) nil)

;; environment / system info — type-level pins only
(qa (stringp (system-name)) t)
(qa (natnump (emacs-pid)) t)
(qa (stringp (emacs-version)) t)
(qa (stringp (emacs-uptime)) t)
(qa (stringp (emacs-init-time)) t)
(qa noninteractive t)
(qa (symbolp system-type) t)
(qa (natnump (user-uid)) t)
(qa (natnump (user-real-uid)) t)
(qa (stringp (user-login-name)) t)
(qa (stringp (user-real-login-name)) t)
(qa (stringp invocation-name) t)
(qa (stringp invocation-directory) t)
(qa (stringp user-emacs-directory) t)
(qa null-device "/dev/null")
(qa path-separator ":")
(qa (boundp 'directory-sep-char) nil) ;; gone in 31.1

;; process-environment / getenv / setenv
(qa (consp process-environment) t)
(progn
  (setenv "QAMISC_XX" "vv")
  (qa (getenv "QAMISC_XX") "vv")
  ;; member returns the tail starting at the match — project with car
  (qa (car (member "QAMISC_XX=vv" process-environment)) "QAMISC_XX=vv")
  (setenv "QAMISC_XX" nil) ;; unset
  (qa (getenv "QAMISC_XX") nil)
  ;; setenv-internal: (ENV VAR VAL SUBSTITUTE-ENV-VARS) — GNU keeps a
  ;; cached view for getenv; go through setenv for the observable part.
  (qa (listp (setenv-internal process-environment "QAMISC_Y" "zz" t)) t)
  (setenv "QAMISC_Y" "zz2")
  (qa (getenv "QAMISC_Y") "zz2")
  (setenv "QAMISC_Y" nil))
(qa (getenv "QAMISC_NEVER_XYZ") nil)
(qa-err (getenv 5) 'wrong-type-argument)

;; command line
(qa (listp command-line-args) t)
(qa (stringp (car command-line-args)) t)
(qa (listp command-line-args-left) t)

;; recursion limits are integer vars
(qa (recursion-depth) 0)
(qa (integerp max-lisp-eval-depth) t)
(qa (integerp max-specpdl-size) t)

;; features
(qa (featurep 'emacs) t)
(qa (eq (car (memq 'emacs features)) 'emacs) t) ;; memq returns tail
(qa (featurep 'qamisc-no-such-feature-zz) nil)
(qa (listp features) t)
;; provide/require roundtrip on a synthetic feature
(progn
  (provide 'qamisc-feat)
  (qa (featurep 'qamisc-feat) t)
  (qa (eq (car (memq 'qamisc-feat features)) 'qamisc-feat) t)
  (qa (eq (car features) 'qamisc-feat) t)) ;; provide pushes to front
(qa (featurep 'qamisc-feat 'sub) nil)

;; gc / timing introspection (type-level only)
(qa (listp (garbage-collect)) t)
(qa (listp (get-internal-run-time)) t)
(qa (listp (memory-use-counts)) t)
(qa (listp (current-time)) t)
(qa (length (current-time)) 4)
(qa (fboundp 'memory-limit) t)
(qa (input-pending-p) nil)

;; fixnum bounds — GNU 61-bit on 64-bit builds
(qa most-positive-fixnum 2305843009213693951)
(qa most-negative-fixnum -2305843009213693952)
;; bignum overflow: GNU promotes fixnum arithmetic
(qa (bignump (1+ most-positive-fixnum)) t)
(qa (prin1-to-string (1+ most-positive-fixnum)) "2305843009213693952")
(qa (= (1+ most-positive-fixnum) (* (1+ most-positive-fixnum) 1)) t)
(qa (bignump (- most-negative-fixnum 1)) t)
(qa (fixnump 5) t)
(qa (fixnump (1+ most-positive-fixnum)) nil)
(qa (bignump 5) nil)
(qa (integerp (1+ most-positive-fixnum)) t)
(qa (natnump (1+ most-positive-fixnum)) t)
(qa (< most-positive-fixnum (1+ most-positive-fixnum)) t)

;; float specials
(qa (car (read-from-string "1e999")) 1.0e+INF)
(qa (car (read-from-string "-0.0")) -0.0)
(qa (isnan (car (read-from-string "0.0e+NaN"))) t)
(qa-prin1 (/ 0.0 0.0) "0.0e+NaN")
(qa-prin1 (/ 1.0 0.0) "1.0e+INF")
(qa-prin1 (/ -1.0 0.0) "-1.0e+INF")
(qa-err (/ 1 0) 'arith-error)

;; misc predicates on odd types
(qa (bool-vector-p (make-bool-vector 3 t)) t)
(qa (length (make-bool-vector 3 t)) 3)
(qa (aref (make-bool-vector 3 t) 0) t)
(qa (aref (make-bool-vector 3 nil) 0) nil)
(let ((bv (make-bool-vector 4 nil)))
  (aset bv 1 t)
  (qa (aref bv 1) t)
  (qa (bool-vector-count-population bv) 1)
  (qa (equal (bool-vector-exclusive-or bv (bool-vector nil t nil nil) nil)
             (bool-vector nil nil nil nil))
      t)
  (qa (bool-vector-count-consecutive bv nil 0) 1)
  (qa (bool-vector-subsetp (bool-vector nil t nil nil) bv) t)
  (qa (equal (bool-vector-not bv) (bool-vector t nil t t)) t)
  (qa (equal (bool-vector-intersection bv (bool-vector t t t t))
             (bool-vector nil t nil nil))
      t)
  (qa (equal (bool-vector-union bv (bool-vector nil nil nil t))
             (bool-vector nil t nil t))
      t))

;; commandp / command modes
(qa (commandp 'previous-line) t) ;; interactive spec from C
(qa (commandp 'car) nil)
(qa (commandp '(lambda () (interactive) 1)) t)
(qa (commandp '(lambda (x) x)) nil)
(qa (commandp 'nonexistent-cmd-zz) nil)

(princ (qa--done))
