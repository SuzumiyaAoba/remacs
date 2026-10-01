;; errors — condition system: signal symbols, data payloads, handlers.
;; GNU-verified: car/cdr check `listp' (not `consp'; setcar uses consp);
;; (/ 1 0) => arith-error with nil data; float ops yield INF/NaN, never
;; signal; elt on a list past end => nil (vectors error); (signal sym …)
;; on a symbol with no error-conditions => error "Invalid error symbol";
;; `quit' is not an `error' — an `error' handler does not catch it;
;; handler-bind exists in 31.1, `ignore-error' does not.

;; == wrong-type-argument ==
(qa-err-data (car 5) 'wrong-type-argument '(listp 5))
(qa-err-data (cdr 5) 'wrong-type-argument '(listp 5))
(qa-err-data (car "abc") 'wrong-type-argument '(listp "abc"))
(qa-err-data (caar 5) 'wrong-type-argument '(listp 5))
(qa (car nil) nil)
(qa (cdr nil) nil)
(qa-err-data (setcar 5 'x) 'wrong-type-argument '(consp 5))
(qa-err-data (setcdr 5 'x) 'wrong-type-argument '(consp 5))
(qa (setcdr (list 'a) 5) 5)
(qa-err-data (+ 1 'a) 'wrong-type-argument '(number-or-marker-p a))
(qa-err-data (1+ 'a) 'wrong-type-argument '(number-or-marker-p a))
(qa-err-data (concat 5) 'wrong-type-argument '(sequencep 5))
(qa-err-data (length 5) 'wrong-type-argument '(sequencep 5))
(qa-err-data (append 5 '(x)) 'wrong-type-argument '(sequencep 5))
(qa-err-data (aref 5 0) 'wrong-type-argument '(arrayp 5))
(qa-err-data (aref [1] 'a) 'wrong-type-argument '(fixnump a))
(qa-err-data (make-string -1 ?x) 'wrong-type-argument '(wholenump -1))
(qa-err-data (string< 5 "a") 'wrong-type-argument '(stringp 5))
(qa-err-data (intern 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (symbol-name 5) 'wrong-type-argument '(symbolp 5))
(qa-err-data (member 5 "abc") 'wrong-type-argument '(listp "abc"))
(qa-err-data (assoc 'a 5) 'wrong-type-argument '(listp 5))
(qa (upcase 5) 5)                        ; an integer is a char: no error
(qa (plist-get 5 :k) nil)

;; == args-out-of-range / sequence edge cases ==
(qa-err-data (aref [1 2] 5) 'args-out-of-range '([1 2] 5))
(qa-err-data (aref [1 2] -1) 'args-out-of-range '([1 2] -1))
(qa-err-data (aset [1 2] 5 'x) 'args-out-of-range '([1 2] 5))
(qa-err-data (aref "ab" 5) 'args-out-of-range '("ab" 5))
(qa-err-data (aset "ab" 5 ?x) 'args-out-of-range '("ab" 5))
(qa-err-data (elt [1 2] 5) 'args-out-of-range '([1 2] 5))
(qa-err-data (elt [1 2] -1) 'args-out-of-range '([1 2] -1))
(qa (elt '(1 2) 5) nil)                  ; list OOB returns nil, no error
(qa (elt '(1 2) -1) 1)
(qa-err-data (substring "ab" 0 5) 'args-out-of-range '("ab" 0 5))
(qa (substring "ab" -1) "b")
(qa (nth -1 '(a b)) 'a)
(qa (nth 10 '(a b)) nil)
(qa (nthcdr -1 '(a b)) '(a b))

;; == arith-error ==
(qa-err-data (/ 1 0) 'arith-error nil)
(qa-err-data (% 1 0) 'arith-error nil)
(qa-err-data (mod 1 0) 'arith-error nil)
(qa (/ 5.0 0) 1.0e+INF)
(qa-prin1 (/ 0.0 0.0) "0.0e+NaN")
(qa-prin1 (sqrt -1) "0.0e+NaN")
(qa-prin1 (exp 1000) "1.0e+INF")
(qa-prin1 (log 0) "-1.0e+INF")
(qa-err (truncate 1.0e+INF) 'overflow-error)
(qa (= (lsh 1 100) 1267650600228229401496703205376) t)   ; bignum, no error
(qa (get 'arith-error 'error-conditions) '(arith-error error))
(qa (get 'arith-error 'error-message) "Arithmetic error")
(qa (get 'overflow-error 'error-conditions)
    '(overflow-error range-error arith-error error))
(qa (get 'singularity-error 'error-conditions)
    '(singularity-error domain-error arith-error error))
(qa (condition-case v (/ 1 0) (arith-error 'ae) (error 'gen)) 'ae)
(qa (condition-case v (car 5) (arith-error 'ae) (error 'gen)) 'gen)
(qa (condition-case v (/ 1 0) (error v)) '(arith-error))

;; == void-function / void-variable / invalid-function ==
(qa-err-data (qaer-undef-fn 1) 'void-function '(qaer-undef-fn))
(qa-err-data (apply 'qaer-undef-fn '(1)) 'void-function '(qaer-undef-fn))
(qa-err qaer-undef-var 'void-variable)
(qa (condition-case e qaer-undef-var (error e)) '(void-variable qaer-undef-var))
(qa-err-data (funcall 5) 'invalid-function '(5))
(qa-err-data (funcall '(1 2)) 'invalid-function '((1 2)))
(qa-err-data (5) 'invalid-function '(5))
(qa (makunbound 'qaer-novar) 'qaer-novar)   ; unbound symbol: no error
(qa (bound-and-true-p qaer-novar) nil)

;; == wrong-number-of-arguments ==
;; Data is (FUNCTION COUNT); FUNCTION prints differently for subr vs
;; symbol vs closure, so pin the pieces that are stable.
(qa (condition-case e (car 1 2) (error e))
    '(wrong-number-of-arguments car 2))
;; funcall'd subr data carries the subr object, not the symbol
(qa (condition-case e (funcall #'car) (error e))
    (list 'wrong-number-of-arguments (symbol-function 'car) 0))
(qa (condition-case e (funcall #'car 1 2) (error (nth 2 e))) 2)
(qa (condition-case e (apply #'car '(1 2 3)) (error (nth 2 e))) 3)
(qa (condition-case e ((lambda (a b) a) 1) (error (list (car e) (nth 2 e))))
    '(wrong-number-of-arguments 1))
(qa (condition-case e ((lambda (a) a) 1 2) (error (nth 2 e))) 2)
(qa-err (eval '(let)) 'wrong-number-of-arguments)
(qa (eval '(setq)) nil)

;; == error / signal construction and data shape ==
(qa (condition-case e (error "val=%s n=%d" 'foo 42) (error e))
    '(error "val=foo n=42"))
(qa (condition-case e (signal 'error '("a" "b")) (error e))
    '(error "a" "b"))
;; (signal 'error STRING) builds a dotted condition (error . STRING).
(qa (condition-case e (signal 'error "raw") (error (cdr e))) "raw")
(qa-err-data (error 'sym-as-msg) 'wrong-type-argument '(stringp sym-as-msg))
;; A signal symbol with no `error-conditions' property is rejected.
(qa-err-data (signal 'qaer-bogus-undef nil)
             'error '("Invalid error symbol" qaer-bogus-undef))
(qa-err (format "%d" 'x) 'error)
(qa (condition-case e (format "%d" 'x) (error (cadr e)))
    "Format specifier doesn’t match argument type")
(qa (condition-case e (format "%s") (error (cadr e)))
    "Not enough arguments for format string")
(qa (error-message-string '(arith-error)) "Arithmetic error")
(qa (error-message-string '(wrong-type-argument listp 5))
    "Wrong type argument: listp, 5")
(qa (error-message-string '(void-function myfn))
    "Symbol’s function definition is void: myfn")

;; == user-error: under `error' in the hierarchy ==
(qa (condition-case e (user-error "oops %d" 7) (user-error e))
    '(user-error "oops 7"))
(qa (condition-case e (user-error "x") (error 'caught)) 'caught)
;; A `user-error' handler does not catch plain `error'.
(qa (condition-case e (error "x") (user-error 'u) (t 'propagated))
    'propagated)
(qa (get 'user-error 'error-conditions) '(user-error error))
(qa (get 'error 'error-conditions) '(error))

;; == condition-case mechanics ==
(qa (condition-case v (car 5) (error 'first) (wrong-type-argument 'second))
    'first)
(qa (condition-case nil (car 5) (wrong-type-argument 'caught)) 'caught)
(qa (condition-case v 42 (:success (list 'ok v)) (error 'e)) '(ok 42))
(qa (condition-case v (car 5) (:success (list 'ok v))
                    (error (list 'bad (car v))))
    '(bad wrong-type-argument))
(qa (condition-case v (/ 1 0) ((arith-error void-function) 'hit) (error 'g))
    'hit)
(qa (condition-case v (qaer-undef-fn2) ((arith-error void-function) 'hit)
                    (error 'g))
    'hit)
(qa (condition-case v (car 5) ((arith-error void-function) 'hit) (error 'g))
    'g)
(qa (condition-case-unless-debug v (car 5) (error (car v)))
    'wrong-type-argument)
;; `quit' has only itself in error-conditions: `error' handlers miss it.
(qa (get 'quit 'error-conditions) '(quit))
(qa (condition-case e (signal 'quit nil) (error 'no) (quit 'qc)) 'qc)
(qa (condition-case e (signal 'quit nil) (t 't-catches-all)) 't-catches-all)
(qa (condition-case e
        (condition-case i (signal 'quit nil) (error 'inner))
      (quit 'outer-quit))
    'outer-quit)

;; == define-error / signal on custom conditions ==
(define-error 'qaer-mine "my custom" 'arith-error)
(put 'qaer-nonerr 'error-conditions '(qaer-nonerr))
(qa (condition-case v (signal 'qaer-mine nil) (arith-error 'parent)
                    (error 'gen))
    'parent)
(qa (condition-case v (signal 'qaer-mine '(d1 d2)) (qaer-mine v))
    '(qaer-mine d1 d2))
(qa (get 'qaer-mine 'error-conditions) '(qaer-mine arith-error error))
(qa (get 'qaer-mine 'error-message) "my custom")
(qa (error-message-string '(qaer-mine d)) "my custom: d")
;; A defined non-error condition: caught by its own or `t' handler only.
(qa (condition-case e (signal 'qaer-nonerr '(d)) (qaer-nonerr e))
    '(qaer-nonerr d))
(qa (condition-case e (signal 'qaer-nonerr nil) (error 'e) (t 't-caught))
    't-caught)

;; == catch / throw ==
(qa-err-data (throw 'qaer-missing-tag 'val) 'no-catch '(qaer-missing-tag val))
(qa (catch 't1 (throw 't1 'inner) 'unreached) 'inner)
;; Inner catch on the same tag wins; the outer catch keeps running.
(qa (catch 'same (catch 'same (throw 'same 'inner-val)) 'outer-after)
    'outer-after)
(qa (catch 'same (list (catch 'same (throw 'same 'inner)) 'after))
    '(inner after))
(qa (catch (intern "qaer-dyntag") (throw 'qaer-dyntag 'v)) 'v)

;; == unwind-protect ==
(let ((log nil))
  (qa (unwind-protect (progn (push 'body log) 'ret) (push 'clean log)) 'ret)
  (qa (reverse log) '(body clean)))
(let ((log nil))
  (qa (catch 't (unwind-protect (throw 't 'tv) (push 'clean log)) 'after) 'tv)
  (qa (reverse log) '(clean)))
(let ((log nil))
  (qa-err (unwind-protect (error "orig") (push 'clean log)) 'error)
  (qa (reverse log) '(clean)))
;; An error raised by the cleanup form replaces the body's error.
(qa (condition-case e (unwind-protect (error "orig") (error "cleanup-err"))
                      (error (cdr e)))
    '("cleanup-err"))
(qa (condition-case e (catch 't (unwind-protect (throw 't 'v) (error "cln")))
                      (error 'masked))
    'masked)
;; A throw out of the cleanup form wins over the body's error.
(qa (catch 't (unwind-protect (error "e") (throw 't 'cleanup-threw)) 'never)
    'cleanup-threw)
(let ((log nil))
  (qa (condition-case v (unwind-protect (signal 'quit nil) (push 'cl log))
                        (quit (list (car v) log)))
      '(quit (cl))))

;; == ignore-errors / with-demoted-errors ==
(qa (ignore-errors (car 5)) nil)
(qa (ignore-errors (+ 1 2)) 3)
(qa (ignore-errors) nil)
;; Demoted errors still `message' — inhibit-message keeps stdout clean.
(qa (let ((inhibit-message t)) (with-demoted-errors "Err: %s" (car 5))) nil)
(qa (with-demoted-errors "Err: %s" 42) 42)
(qa (let ((inhibit-message t)) (with-demoted-errors "E %s" (car 5))) nil)
(qa (with-demoted-errors "E %s" 'ok) 'ok)

;; == handler-bind (Emacs 29+) ==
(qa (fboundp 'handler-bind) t)
;; Handlers receive the whole condition; they observe, not handle —
;; a normal return lets signaling continue.
(qa (catch 'out (handler-bind ((error (lambda (e) (throw 'out (car e)))))
                             (car 5)))
    'wrong-type-argument)
(qa (catch 'out (handler-bind ((qaer-mine (lambda (e) (throw 'out (car e)))))
                             (signal 'qaer-mine '(x))))
    'qaer-mine)
;; Parent conditions fire handlers for their children.
(qa (catch 'out (handler-bind ((arith-error (lambda (e) (throw 'out (car e)))))
                             (signal 'qaer-mine '(x))))
    'qaer-mine)
(let ((seen nil))
  (qa (condition-case e
          (handler-bind ((qaer-nonerr (lambda (e) (push 'saw seen) nil)))
            (signal 'qaer-nonerr '(p)) 'ret)
        (qaer-nonerr 'sig-caught) (t 't-c))
      'sig-caught)
  (qa seen '(saw)))
(qa (handler-bind ((error #'ignore)) 42) 42)

;; == reader errors ==
(qa-err-data (read-from-string "(") 'end-of-file nil)
(qa-err-data (read-from-string "(1 2") 'end-of-file nil)
(qa-err-data (read-from-string "'") 'end-of-file nil)
(qa-err (read-from-string ")") 'invalid-read-syntax)
(qa-err (read-from-string "#z") 'invalid-read-syntax)
(qa (condition-case e (read-from-string "(1 . 2 3)") (error (cadr e)))
    "expected )")
(qa (string-match "(" "x") nil)          ; bare ( is a literal, not an error
(qa-err (string-match "\\(" "x") 'invalid-regexp)

;; == file-error family ==
(qa (condition-case e (make-directory "/") (error e))
    '(file-already-exists "File exists" "/"))
(qa (make-directory "/" t) t)
(qa (condition-case e (insert-file-contents "/nonexistent-qaer-xyz")
                      (error (list (car e) (cadr e))))
    '(file-missing "Opening input file"))
(qa (condition-case e (make-directory "/") (file-error 'fe) (error 'g)) 'fe)
(qa (get 'file-error 'error-conditions) '(file-error error))
(qa (get 'file-missing 'error-conditions) '(file-missing file-error error))

;; == buffer / motion errors ==
(qa-err (with-temp-buffer (forward-char 5)) 'end-of-buffer)
(qa-err (with-temp-buffer (backward-char)) 'beginning-of-buffer)
(qa (with-temp-buffer (goto-char 5)) 5)
(qa (condition-case e (with-temp-buffer (down-list)) (error e))
    '(scan-error "Unbalanced parentheses" 1 1))
(qa-err (with-temp-buffer (up-list)) 'scan-error)
(qa-err (with-temp-buffer (setq buffer-read-only t) (insert "x"))
        'buffer-read-only)
(qa (progn (get-buffer-create "qaer-tmp-b") (kill-buffer "qaer-tmp-b")) t)
(qa-err (with-current-buffer "qaer-tmp-b" 1) 'error)
(qa-err (kill-buffer "definitely-no-such-buffer-xyz") 'error)

;; == setting-constant ==
(qa-err-data (let () (setq t 5)) 'setting-constant '(t))
(qa-err-data (let () (setq nil 5)) 'setting-constant '(nil))
(qa (condition-case e (eval '(setq t 5)) (error e)) '(setting-constant t))

(qa (boundp 'debug-on-error) t)

(princ (qa--done))
