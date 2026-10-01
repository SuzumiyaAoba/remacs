;; Condition system — GNU 31.1 -Q --batch ground truth.
;; Framework note: `qa' runs the form via `eval', so condition-case
;; clauses here always use an ATOMIC handler name — a list handler
;; like (foo bar) is interpreted as "list of condition names".
;;
;; Pinned semantics (verified):
;;  * An UNDEFINED signal symbol has implicit error-conditions (error):
;;    (condition-case … (error 'e)) CATCHES (signal 'foo data), but a
;;    (foo …) handler does NOT.  No handler or only non-matching ones =>
;;    the default handler reports "Invalid error symbol: foo".
;;  * `t' as handler name matches EVERY signal, including undefined ones.
;;  * Handlers are tried in order; first match wins.
;;  * `error' formats args with `format'; (car e) is `error', cdr is the
;;    one-element list holding the formatted message string.
;;  * `user-error' is a subtype of `error' (conditions (user-error error)).
;;  * :success runs its body on success; var is bound to the BODY's
;;    value during the clause, and the clause's value is returned.
;;  * condition-case var nil discards the error object.
;;  * An uncaught `throw' re-signals as (no-catch TAG VALUE); no-catch
;;    IS under `error', so `error' handlers catch it — but `throw' inside
;;    a matching `catch' never becomes a signal at all.
;;  * `quit' is not under `error': an `error' handler does NOT catch it.

;; == signal: undefined symbols are plain `error' ==
(qa (condition-case nil (signal 'foo '(1)) (error 'e)) 'e)
(qa (condition-case e (signal 'foo '(1))
      (error (error-message-string e)))
    "Invalid error symbol: foo")
(qa-err (signal 'foo '(1)) 'error)
;; a named handler for an UNDEFINED symbol never matches
(qa (condition-case nil (signal 'foo nil) (foo 'f) (t 'fell)) 'fell)
(qa (condition-case nil (signal 'foo nil) (bar 'b) (t 'all)) 'all)
;; `t' handler catches everything
(qa (condition-case nil (signal 'foo nil) (t 'all)) 'all)
(qa (condition-case nil (/ 1 0) (t 'tfirst)) 'tfirst)
;; once error-conditions is set, named handlers match by membership
(qa (let ((s (make-symbol "u")))
      (put s 'error-conditions '(u-error error))
      (condition-case e (signal s '(d))
        (error (eq (car e) s))))
    t)
(qa (progn (put 'qa-err2-test 'error-conditions '(qa-err2-test error))
           (put 'qa-err2-test 'error-message "Test happened")
           (condition-case nil (signal 'qa-err2-test '(1))
             (qa-err2-test 'matched) (error 'e)))
    'matched)
;; error-message prop is used for printing: "MSG: data…"
(qa (progn (put 'qa-err2-msg 'error-conditions '(error))
           (put 'qa-err2-msg 'error-message "Foo happened")
           (condition-case e (signal 'qa-err2-msg '(1))
             (error (error-message-string e))))
    "Foo happened: 1")
;; signaling `error' directly: data is the payload verbatim
(qa (condition-case e (signal 'error '(a b)) (error (cdr e))) '(a b))
(qa (condition-case e (signal 'error nil) (error (list (car e) (cdr e))))
    '(error nil))
;; arith-error carries the data list verbatim
(qa (condition-case e (signal 'arith-error '(d1 d2))
      (error (list 'e (car e) (cdr e))))
    '(e arith-error (d1 d2)))
(qa (condition-case e (signal 'arith-error nil)
      (arith-error 'ae) (error 'e))
    'ae)
;; real arith-error carries nil data
(qa (condition-case e (/ 1 0) (arith-error (list (car e) (cdr e))))
    '(arith-error nil))

;; == error / user-error ==
(qa (condition-case e (error "x=%d" 7) (error (car e))) 'error)
(qa (condition-case e (error "x=%d" 7) (error (cdr e))) '("x=7"))
(qa (condition-case e (error "x=%d" 7) (error (error-message-string e)))
    "x=7")
(qa (condition-case e (error "a %s b %s" 'x 'y)
      (error (error-message-string e)))
    "a x b y")
(qa (condition-case e (error "100%%") (error (error-message-string e)))
    "100%")
(qa-err (error "boom") 'error)
(qa (condition-case e (user-error "oops")
      (user-error (error-message-string e)))
    "oops")
;; user-error is catchable via plain `error' too
(qa (condition-case e (user-error "oops") (error (car e))) 'user-error)
(qa-err (user-error "oops") 'user-error)
(qa (get 'user-error 'error-conditions) '(user-error error))
(qa (get 'arith-error 'error-conditions) '(arith-error error))
(qa (get 'qa-err2-nosuch 'error-conditions) nil)

;; == condition-case structure ==
;; no error => body's value, handlers skipped
(qa (condition-case e (+ 1 2) (error 'bad)) 3)
;; var nil discards the error object
(qa (condition-case nil (car 1) (error 'ok)) 'ok)
;; :success clause: var bound to the BODY's value; clause value returned
(qa (condition-case e (progn (+ 1 2)) (:success 'win) (error 'lost))
    'win)
(qa (condition-case e (car 1) (:success 'win) (error 'lost)) 'lost)
(qa (condition-case e 5 (:success e) (error 'bad)) 5)
;; real error object contents for wrong-type-argument
(qa (condition-case e (car 1) (error (car e))) 'wrong-type-argument)
(qa (condition-case e (car 1) (error (error-message-string e)))
    "Wrong type argument: listp, 1")
;; quit is not an `error'
(qa (condition-case e (signal 'quit nil) (error 'caught) (quit 'quit))
    'quit)
;; uncaught throw => (no-catch TAG VALUE), which IS an `error'
(qa (condition-case e (throw 'tag 7) (error (list (car e) (cdr e))))
    '(no-catch (tag 7)))
(qa (condition-case e (throw 'foo 1) (no-catch 'nc)) 'nc)
(qa (condition-case e (throw 'foo 1) (t 'caught)) 'caught)
(qa (get 'no-catch 'error-conditions) '(no-catch error))

;; == unwind-protect ==
(qa (let ((x 0))
      (unwind-protect (setq x 5) (setq x (1+ x)))
      x)
    6)
(qa (let ((x 0))
      (condition-case e
          (unwind-protect (car 1) (setq x 9))
        (error (list 'caught x))))
    '(caught 9))
;; cleanup runs on throw too; the nonlocal exit continues after
(qa (let ((x 0))
      (list (unwind-protect (catch 't (throw 't 'done)) (setq x 3))
            x))
    '(done 3))
;; multiple cleanup forms, value = body value
(qa (let ((x nil))
      (list (unwind-protect 'body (push 1 x) (push 2 x)) x))
    '(body (2 1)))
;; cleanup error replaces the original signal
(qa (condition-case e
        (unwind-protect (signal 'arith-error nil) (car 1))
      (arith-error 'arith)
      (error 'other))
    'other)
(qa (condition-case e (unwind-protect 1 (car 1)) (error 'cleanup-err))
    'cleanup-err)

;; == ignore-errors ==
(qa (ignore-errors (+ 1 2)) 3)
(qa (ignore-errors (car 1)) nil)
(qa (ignore-errors) nil)
;; evaluates ALL body forms
(qa (let ((x 0)) (list (ignore-errors (setq x 1) (setq x 2)) x))
    '(2 2))
;; body stops at the first error
(qa (let ((x 0))
      (list (ignore-errors (setq x 1) (car 1) (setq x 2)) x))
    '(nil 1))
;; uncaught throw inside ignore-errors => nil
(qa (ignore-errors (throw 't 1)) nil)

;; == with-demoted-errors / demote-errors ==
(qa (with-demoted-errors "E:%S" (car 1)) nil)
(qa (with-demoted-errors "E:%S" (+ 1 2)) 3)
;; demoted: an enclosing condition-case does NOT see the error
(qa (condition-case e (with-demoted-errors "E:%S" (car 1))
      (error 'esc))
    nil)
;; demote-errors alone does NOT swallow the signal
(qa-err (let ((demote-errors t)) (car 1)) 'wrong-type-argument)

;; == arity ==
(qa-err (signal) 'wrong-number-of-arguments)
(qa-err (signal 'foo) 'error)
(qa-err (condition-case) 'wrong-number-of-arguments)

(princ (qa--done))
