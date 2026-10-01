;;; -*- lexical-binding: t -*-
;; subr-x — GNU-verified: if-let/when-let are obsolete macros in 31.1 and
;; warn at macroexpansion time, so they are exercised through `eval' under
;; `warning-suppress-types'.  Missing in 31.1 (skipped): thread-all,
;; hash-table-key-exists-p, ensure-vector, ntake-while,
;; defvar-permanently-local, with-muffled-echo-area, read-multiple-choice
;; (needs input).  string-pad/string-replace/string-clean-whitespace/
;; string-empty-p/string-chop-newline and string-pixel-width/string-glyph-*
;; are left to the strings/display files.
(require 'subr-x)

;; == if-let / when-let (obsolete in 31.1) ==
(let ((warning-suppress-types '(obsolete)))
  ;; single binding may be written without the extra parens
  (qa (eval '(if-let (x 42) x 'no)) 42)
  (qa (eval '(if-let ((x 42)) x 'no)) 42)
  ;; bare (x) spec binds x to its own value and tests it
  (qa (eval '(let ((x 4)) (if-let (x) x 'n))) 4)
  (qa (eval '(let ((x nil)) (if-let (x) x 'n))) 'n)
  ;; a nil value takes the else branch; missing else => nil
  (qa (eval '(if-let ((x nil)) 'yes 'no)) 'no)
  (qa (eval '(if-let ((x nil)) 'yes)) nil)
  ;; multi-binding if-let behaves like if-let* (sequential, all evaled)
  (qa (eval '(if-let ((a 1) (b (1+ a))) (list a b))) '(1 2))
  (qa (eval '(if-let ((x nil) (y 1)) 'yes 'no)) 'no)
  ;; when-let: no else; body is implicit progn, returns last form
  (qa (eval '(when-let ((x 5)) (+ x 1) (* x 2))) 10)
  (qa (eval '(when-let (x 3) x)) 3)
  (qa (eval '(when-let ((x nil)) 'never)) nil))

;; == if-let* / when-let* ==
(qa (if-let* ((x 42)) x 'no) 42)
(qa (if-let* ((x nil)) 'yes 'no) 'no)
;; else clause is an implicit progn — last form wins
(qa (if-let* ((x nil)) 'yes 'e1 'e2) 'e2)
;; sequential binding: b sees a
(qa (if-let* ((a 1) (b (1+ a))) (list a b)) '(1 2))
;; bare (y) spec reuses the outer value and tests it
(qa (let ((y 9)) (if-let* ((y)) (1+ y) 'no)) 10)
(qa (let ((y nil)) (if-let* ((y)) 'yes 'no)) 'no)
;; a nil binding stops evaluation of later binding forms
(qa (let ((qasx-s nil))
      (list (if-let* ((a (progn (push 'a qasx-s) nil))
                     (b (progn (push 'b qasx-s) 1)))
                'yes 'no)
            qasx-s))
    '(no (a)))

(qa (when-let* ((a 1) (b (1+ a))) b) 2)
(qa (when-let* ((a 1) (b nil)) 'never) nil)
(qa (when-let* ((a 1) (b 2)) a b (+ a b)) 3)
(qa (let ((qasx-s nil))
      (list (when-let* ((a nil) (b (progn (push 'b qasx-s) 1))) 'y)
            qasx-s))
    '(nil nil))

;; == and-let* ==
;; with no body, returns the last binding's value
(qa (and-let* ((a 1) (b 2))) 2)
(qa (and-let* ((a 1) (b t))) t)
(qa (and-let* ((a 1) (b nil) (c 3))) nil)
;; body forms override the default value
(qa (and-let* ((a 1) (b 2)) (+ a b)) 3)
(qa (and-let* () 'end) 'end)
;; an empty nil varlist expands to t, not nil
(qa (and-let* nil) t)
;; bindings are sequential; later ones are not evaluated after nil
(qa (let ((qasx-s nil))
      (list (and-let* ((a (progn (push 'a qasx-s) 1))
                       (b (progn (push 'b qasx-s) nil))
                       (c (progn (push 'c qasx-s) 3)))
              'yes)
            qasx-s))
    '(nil (b a)))

;; == thread-first / thread-last (no thread-all in 31.1) ==
;; thread-first puts the accumulated value in argument position 1
(qa (thread-first (+ 1 2) (* 10) (- 100)) -70)
;; thread-last puts it last
(qa (thread-last (+ 1 2) (* 10) (- 100)) 70)
;; a bare symbol f means (f acc)
(qa (thread-first 'x list) '(x))
(qa (thread-last 5 list) '(5))
(qa (thread-first 5 -) -5)
(qa (thread-first "ab" (concat "c") upcase) "ABC")
(qa (thread-last '(1 2 3) cdr car) 2)
(qa (thread-last '(1 2) (mapcar #'1+) (append '(0))) '(0 2 3))
;; zero forms returns the value unchanged
(qa (thread-first 5) 5)
(qa (thread-last 'z) 'z)

;; == named-let ==
;; == named-let — needs lexical binding; use qa-lex ==
(qa-lex (named-let lp ((i 0) (acc 0))
          (if (= i 5) acc (lp (1+ i) (+ acc i))))
        10)
(qa-lex (named-let rev ((xs '(a b c)) (r nil))
          (if (null xs) (nreverse r) (rev (cdr xs) (cons (car xs) r))))
        '(a b c))
;; empty binding list, no recursion
(qa-lex (named-let g () 42) 42)
;; iterative gcd-style recursion
(qa-lex (named-let qasx-gcd ((a 48) (b 36))
          (if (zerop b) a (qasx-gcd b (mod a b))))
        12)
;; initial values are evaluated once, not per iteration
(qa-lex (let ((qasx-calls 0))
          (list (named-let lp ((i 0) (m (progn (setq qasx-calls (1+ qasx-calls)) 3)))
                  (if (= i m) i (lp (1+ i) m)))
                qasx-calls))
        '(3 1))
;; named-let refuses dynamic scope
(qa-err (eval '(named-let f () 1)) 'error)

;; == hash-table helpers ==
(qa (let ((qasx-h (make-hash-table :test #'equal)))
      (puthash "b" 2 qasx-h)
      (puthash "a" 1 qasx-h)
      (puthash "c" 3 qasx-h)
      (list (hash-table-empty-p qasx-h)
            (sort (hash-table-keys qasx-h) #'string<)
            (sort (hash-table-values qasx-h) #'<)))
    '(nil ("a" "b" "c") (1 2 3)))
(qa (hash-table-empty-p (make-hash-table)) t)
(qa (hash-table-keys (make-hash-table)) nil)
(qa (hash-table-values (make-hash-table)) nil)
(qa-err (hash-table-empty-p nil) 'wrong-type-argument)

;; == alist-get ==
(qa (alist-get 'b '((a . 1) (b . 2))) 2)
(qa (alist-get 'z '((a . 1)) 'dflt) 'dflt)
;; first match wins
(qa (alist-get 'a '((a . 1) (a . 2))) 1)
;; a present-but-nil value yields nil, NOT the default
(qa (alist-get 'a '((a)) 'dflt) nil)
;; string keys need a TESTFN
(qa (alist-get "x" '(("x" . 9))) nil)
(qa (alist-get "x" '(("x" . 9)) nil nil #'equal) 9)
;; not a generic map getter: hash tables are rejected
(qa-err (let ((qasx-h (make-hash-table)))
          (puthash 'k 5 qasx-h)
          (alist-get 'k qasx-h))
        'wrong-type-argument)

;; == ensure-list (no ensure-vector in 31.1) ==
(qa (ensure-list 'a) '(a))
(qa (ensure-list '(a b)) '(a b))
(qa (ensure-list nil) nil)
;; non-list non-nil atoms are wrapped whole
(qa (ensure-list [1 2]) '([1 2]))
(qa (ensure-list "xy") '("xy"))

;; == take / ntake / take-while (no ntake-while in 31.1) ==
(qa (take 2 '(a b c d)) '(a b))
(qa (take 0 '(a b)) nil)
(qa (take 5 '(a b)) '(a b))
(qa (take 2 nil) nil)
;; negative n yields nil, not a suffix
(qa (take -1 '(a b c)) nil)
(qa-err (take 2 "abc") 'wrong-type-argument)
(qa-err (take 2 '(a . b)) 'wrong-type-argument)
(qa (ntake 2 (list 'a 'b 'c 'd)) '(a b))
(qa (ntake 0 (list 'a 'b)) nil)
(qa (ntake 5 (list 'a 'b)) '(a b))
(qa (ntake -1 (list 'a 'b)) nil)
;; ntake truncates its argument destructively
(qa (let ((qasx-l (list 'a 'b 'c)))
      (list (ntake 1 qasx-l) qasx-l))
    '((a) (a)))
(qa (take-while #'oddp '(1 3 4 5)) '(1 3))
(qa (take-while #'oddp '(2 4)) nil)
(qa (take-while #'oddp nil) nil)
;; take-while returns a fresh list, not a shared tail
(qa (let ((qasx-l (list 1 3 4)))
      (setcar (take-while #'oddp qasx-l) 99)
      qasx-l)
    '(1 3 4))

;; == proper-list-p: returns the length, not t ==
(qa (proper-list-p '(a b c)) 3)
(qa (proper-list-p '()) 0)
(qa (proper-list-p '(a b . c)) nil)
(qa (let ((qasx-circ (list 1 2)))
      (setcdr (cdr qasx-circ) qasx-circ)
      (proper-list-p qasx-circ))
    nil)
(qa (proper-list-p 5) nil)
(qa (proper-list-p '(a . b)) nil)
(qa (proper-list-p [1 2]) nil)

;; == add-hook / remove-hook ==
;; == add-hook / remove-hook on a symbol-as-hook ==
(qa (let ((qasx-h (make-symbol "qasx-h"))
          (qasx-f1 (lambda () 'f1))
          (qasx-f2 (lambda () 'f2)))
      (list (boundp qasx-h)
            (progn (add-hook qasx-h qasx-f1)
                   (add-hook qasx-h qasx-f2)
                   (list (length (symbol-value qasx-h))
                         (eq (car (symbol-value qasx-h)) qasx-f2)))
            (progn (add-hook qasx-h qasx-f1)
                   (length (symbol-value qasx-h)))
            (progn (remove-hook qasx-h qasx-f1)
                   (length (symbol-value qasx-h)))
            (progn (remove-hook qasx-h qasx-f1)
                   (length (symbol-value qasx-h)))
            (progn (remove-hook qasx-h qasx-f2)
                   (symbol-value qasx-h))
            (boundp qasx-h)))
    '(nil (2 t) 2 1 1 nil t))

;; DEPTH: larger depths run later; t is still accepted as append
;; DEPTH: larger depths run later
;; DEPTH: larger depths run later — compare via symbol-name since
;; qasx-* are uninterned
(qa (let ((qasx-h (make-symbol "qasx-hd"))
          (qasx-s (make-symbol "s"))
          (qasx-m (make-symbol "m"))
          (qasx-d (make-symbol "d")))
      (add-hook qasx-h qasx-s 0)
      (add-hook qasx-h qasx-m 50)
      (add-hook qasx-h qasx-d -50)
      (mapcar #'symbol-name (symbol-value qasx-h)))
    '("d" "s" "m"))
(qa (let ((qasx-h (make-symbol "qasx-ha"))
          (qasx-f1 (lambda () 'a))
          (qasx-f2 (lambda () 'b)))
      (add-hook qasx-h qasx-f1)
      (add-hook qasx-h qasx-f2 t)
      ;; compare the fn's return value
      (funcall (car (last (symbol-value qasx-h)))))
    'b)
;; the same function added at another depth is still a duplicate
(qa (let ((qasx-h (make-symbol "qasx-hd2"))
          (qasx-f (lambda () 'f)))
      (add-hook qasx-h qasx-f 0)
      (add-hook qasx-h qasx-f 50)
      (length (symbol-value qasx-h)))
    1)
;; LOCAL: hook value ends in the t marker inside the buffer only
(qa (let ((qasx-h (make-symbol "qasx-hl"))
          (qasx-f (lambda () 'x)))
      (with-temp-buffer
        (add-hook qasx-h qasx-f nil t)
        (list (length (symbol-value qasx-h))
              (car (last (symbol-value qasx-h)))))
      ;; after leaving the temp buffer the global value is nil
      (list (boundp qasx-h) (symbol-value qasx-h)))
    '(t nil))

;; == run-hook-with-args* ==
(qa (let ((qasx-calls nil)
          (qasx-h (make-symbol "qasx-rh")))
      (set qasx-h (list (lambda (a b) (push (+ a b) qasx-calls))
                        (lambda (a b) (push (* a b) qasx-calls))))
      (list (run-hook-with-args qasx-h 3 4)
            qasx-calls))
    '(nil (12 7)))
;; a non-list hook value is a single function
(qa (let ((qasx-calls nil)
          (qasx-h (make-symbol "qasx-rh1")))
      (set qasx-h (lambda (x) (push x qasx-calls)))
      (list (run-hook-with-args qasx-h 5)
            qasx-calls))
    '(nil (5)))
;; until-success stops at the first non-nil and returns it
(qa (let ((qasx-calls nil)
          (qasx-h (make-symbol "qasx-us")))
      (set qasx-h (list (lambda (x) (push (cons 'f1 x) qasx-calls) nil)
                        (lambda (x) (push (cons 'f2 x) qasx-calls) 'hit)
                        (lambda (x) (push (cons 'f3 x) qasx-calls) 'late)))
      (list (run-hook-with-args-until-success qasx-h 99)
            qasx-calls))
    '(hit ((f2 . 99) (f1 . 99))))
;; until-failure stops at the first nil and returns nil
(qa (let ((qasx-calls nil)
          (qasx-h (make-symbol "qasx-uf")))
      (set qasx-h (list (lambda (x) (push x qasx-calls) 'yes)
                        (lambda (x) (push x qasx-calls) nil)
                        (lambda (x) (push x qasx-calls) 'late)))
      (list (run-hook-with-args-until-failure qasx-h 7)
            qasx-calls))
    '(nil (7 7)))
;; all-succeeding until-failure returns t
(qa (let ((qasx-h (make-symbol "qasx-uf2")))
      (set qasx-h (list (lambda (x) 'a) (lambda (x) 'b)))
      (run-hook-with-args-until-failure qasx-h 1))
    t)
;; empty hooks
(qa (let ((qasx-h (make-symbol "qasx-empty")))
      (list (run-hook-with-args-until-success qasx-h)
            (run-hook-with-args-until-failure qasx-h)
            (run-hook-with-args qasx-h 1)))
    '(nil t nil))

;; == define-error ==
(define-error 'qasx-err1 "qasx went wrong" 'arith-error)
(qa (get 'qasx-err1 'error-message) "qasx went wrong")
(qa (get 'qasx-err1 'error-conditions) '(qasx-err1 arith-error error))
;; signaling with data appends ": DATA" to the message
(qa (condition-case qasx-e
        (signal 'qasx-err1 '(extra))
      (error (error-message-string qasx-e)))
    "qasx went wrong: extra")
;; parent is the car of a signaled condition
(qa (condition-case qasx-e
        (signal 'qasx-err1 nil)
      (arith-error (car qasx-e)))
    'qasx-err1)
;; no message and no parent: message is nil, parent defaults to error
(define-error 'qasx-err2 nil nil)
(qa (get 'qasx-err2 'error-conditions) '(qasx-err2 error))
(qa (condition-case qasx-e
        (signal 'qasx-err2 nil)
      (error (error-message-string qasx-e)))
    "peculiar error")

;; == defvar-local / local-variable-p ==
(defvar-local qasx-bufvar 7)
(qa qasx-bufvar 7)
;; automatically-local vars have no buffer-local cell until setq
(qa (local-variable-p 'qasx-bufvar) nil)
(qa (default-value 'qasx-bufvar) 7)
(with-temp-buffer
  (qa (local-variable-p 'qasx-bufvar) nil)
  (setq qasx-bufvar 99)
  (qa (local-variable-p 'qasx-bufvar) t)
  (qa qasx-bufvar 99))
;; the default is untouched
(qa qasx-bufvar 7)
(qa (local-variable-p 'fill-column) nil)

;; == string-to-syntax: returns (CODE), a cons ending in nil ==
(qa (string-to-syntax "w") '(2))
(qa (string-to-syntax ".") '(1))
(qa (string-to-syntax "(") '(4))
(qa (string-to-syntax " ") '(0))
(qa (string-to-syntax "'") '(6))
(qa-err (string-to-syntax "q") 'error)
(qa-err (string-to-syntax "") 'error)

;; == truncate-string-to-width ==
(qa (truncate-string-to-width "abcdef" 4) "abcd")
(qa (truncate-string-to-width "abc" 10) "abc")
(qa (truncate-string-to-width "abc" 0) "")
;; CJK characters count double width
(qa (truncate-string-to-width "あいうえお" 6) "あいう")
;; START skips leading columns
(qa (truncate-string-to-width "abcdef" 4 1) "bcd")
;; PADDING fills up to WIDTH with that character
(qa (truncate-string-to-width "ab" 5 nil ?.) "ab...")
;; ELLIPSIS non-nil uses U+2026 and reserves its column
(qa (truncate-string-to-width "abcdef" 4 nil nil t) "ab…")
(qa (truncate-string-to-width "abcdef" 3 nil nil "…") "a…")
;; a tab jumps to the next tab stop and drops following overflow
(qa (truncate-string-to-width "a\tbc" 8) "a")

;; == replace-region-contents ==
;; SOURCE may be a buffer or a function returning one; return value is t
(qa (let ((qasx-src (get-buffer-create " qasx-src")))
      (unwind-protect
          (progn
            (with-current-buffer qasx-src
              (insert "abXYef"))
            (with-temp-buffer
              (insert "abcdef")
              (list (replace-region-contents (point-min) (point-max) qasx-src)
                    (buffer-string))))
        (kill-buffer qasx-src)))
    '(t "abXYef"))

;; == while-no-input: with no input pending, body runs normally ==
(qa (while-no-input (+ 1 2)) 3)
(qa (while-no-input 'a 'b 'c) 'c)

(princ (qa--done))
