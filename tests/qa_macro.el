;; Macros & backquote — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - defmacro returns the macro NAME (a symbol).
;;   - macroexpand-1 does ONE expansion step; macroexpand recurses;
;;     macroexpand-all recurses into subforms (incl. inside defun bodies).
;;   - macroexpand on a non-macro list/atom returns it unchanged;
;;     no error.  Special forms are never expanded.
;;   - `macro-function' is NOT a function in GNU 31.1.
;;   - `when' → (if A (progn B...)); `unless' → (if A nil B...);
;;     `pop' → (car-safe (prog1 X (setq X (cdr X)))).
;;   - Backquote eval: ,X unquotes; ,@X splices; ,@ after `.' is a
;;     literal list element (splice only valid inside the list proper);
;;     `,.` was REMOVED — `,.b' reads as (unquote .b) = (\, .b).
;;   - `,@` before `.' inside a backquote is a READ error.
;;   - macroexpand-all leaves `quote' bodies alone; wraps lambdas as #'.

;; == defmacro basics ==
;; defmacro returns the macro name
(qa (defmacro qa-m-name (x) `(list ,x)) 'qa-m-name)
;; macrop / fboundp on the result
(qa (let () (defmacro qa-m-p (x) `(list ,x)) (macrop 'qa-m-p)) t)
(qa (let () (defmacro qa-m-fb (x) `(list ,x)) (fboundp 'qa-m-fb)) t)
;; the macro actually works
(qa (let () (defmacro qa-m-call (x) `(list ,x)) (qa-m-call 7)) '(7))
;; macro arg arrives UNevaluated
(qa (let () (defmacro qa-m-raw (x) `',x) (qa-m-raw (+ 1 2))) '(+ 1 2))
;; &optional / &rest in macro lambda lists
(qa (let ()
      (defmacro qa-m-or (a &optional b) `(list ,a ,b))
      (qa-m-or 1))
    '(1 nil))
(qa (let ()
      (defmacro qa-m-or (a &optional b) `(list ,a ,b))
      (qa-m-or 1 2))
    '(1 2))
(qa (let ()
      (defmacro qa-m-rest (a &rest b) `(list ',a ,@b))
      (qa-m-rest x 1 2 3))
    '(x 1 2 3))
;; body may have several forms (implicit progn); declare is allowed
(qa (let ()
      (defmacro qa-m-body (x)
        (declare (indent 1))
        'ignored
        `(+ ,x 1))
      (qa-m-body 10))
    11)

;; == macroexpand-1: one step ==
(qa (let ()
      (defmacro qa-m1 (x) `(list ,x))
      (macroexpand-1 '(qa-m1 5)))
    '(list 5))
;; nested macro output is NOT further expanded by macroexpand-1
(qa (macroexpand-1 '(when a b c)) '(if a (progn b c)))
(qa (macroexpand-1 '(unless a b)) '(if a nil b))
(qa (macroexpand-1 '(pop x)) '(car-safe (prog1 x (setq x (cdr x)))))
;; non-macro forms returned verbatim, no error
(qa (macroexpand-1 'x) 'x)
(qa (macroexpand-1 nil) nil)
(qa (macroexpand-1 42) 42)
(qa (macroexpand-1 '(car 1)) '(car 1))
(qa (macroexpand-1 '(a . b)) '(a . b))
;; special forms never expand
(qa (macroexpand-1 '(if a b)) '(if a b))
;; undefined function symbol → returned unchanged (not an error)
(qa (macroexpand-1 '(qa-no-such-fn-xyz)) '(qa-no-such-fn-xyz))
;; wrong macro arity → wrong-number-of-arguments
(qa-err (let ()
          (defmacro qa-m-arity (x) `(list ,x))
          (macroexpand-1 '(qa-m-arity)))
        'wrong-number-of-arguments)

;; == macroexpand: recursive at top level ==
(qa (let ()
      (defmacro qa-m2 (x) `(when t ,x))
      (macroexpand '(qa-m2 5)))
    '(if t (progn 5)))
(qa (macroexpand '(unless a b)) '(if a nil b))
;; does NOT descend into progn subforms
(qa (macroexpand '(progn (when a b))) '(progn (when a b)))
;; destructuring lambda lists are NOT supported by defmacro in 31.1
;; (that's cl-defmacro territory): calling it → invalid-function
;; remacs: unbinds a/b from the dynamic env → void-variable, not invalid-function
(qa-err (let ()
             (defmacro qa-m-dst ((a b) c) `(list ,a ,b ,c))
             (qa-m-dst (1 2) 3))
           'invalid-function)

;; == macroexpand-all: descends into subforms ==
(qa (macroexpand-all '(progn (when a b) (unless c d)))
    '(progn (if a (progn b)) (if c nil d)))
(qa (macroexpand-all '(defun qa-m-df (x) (when x x)))
    '(defalias 'qa-m-df #'(lambda (x) (if x (progn x)))))
;; quote-quoted forms are left alone (macroexpand-all sees the
;; evaluated (quote ...) list, not the code inside it)
(qa (macroexpand-all '(quote (when x))) (list 'quote '(when x)))
(qa (macroexpand-all ''(when x)) (list 'quote '(when x)))
;; atom input → atom
(qa (macroexpand-all 'x) 'x)
(qa (macroexpand-all 3.14) 3.14)
;; let bodies get expanded too
(qa (macroexpand-all '(let ((a 1)) (when a (incf a))))
    '(let ((a 1)) (if a (progn (setq a (+ a 1))))))

;; == env arg of macroexpand: symbol macros are NOT expanded
;;    by macroexpand/macroexpand-all in batch ==
(qa (macroexpand 'x '((x . (car y)))) 'x)
(qa (macroexpand-all 'x '((x . (car y)))) 'x)

;; == macro-function does not exist ==
(qa (fboundp 'macro-function) nil)
(qa (fboundp 'macroexpand-1) t)
(qa (fboundp 'macroexpand-all) t)

;; == backquote reader ==
;; NOTE: `qa' evaluates EXPECTED too — reader results containing
;; backquote/unquote markers must be written in their raw (\, x) form.
(qa (car (read-from-string "`(a ,b ,@c)")) '(\` (a (\, b) (\,@ c))))
(qa (car (read-from-string "`,x")) '(\` (\, x)))
;; `,.` is gone: `,.b' reads as unquote of symbol `.b'
(qa (car (read-from-string "(a ,.b)")) '(a (\, .b)))
(qa (car (read-from-string "`(a ,.b)")) '(\` (a (\, .b))))
;; , outside a backquote reads as (unquote sym)
(qa (car (read-from-string "(,a)")) '((\, a)))
;; ,@ outside backquote reads as (\,@ sym); after a literal dot too
(qa (car (read-from-string "(a ,@b)")) '(a (\,@ b)))
(qa (car (read-from-string "(a ,@b . c)")) '(a (\,@ b) . c))
;; ,@ before `.' inside a backquote reads fine and splices the tail
(qa (car (read-from-string "`(a ,@b . c)")) '(\` (a (\,@ b) . c)))
(qa (eval (car (read-from-string "`(a ,@'(1 2) . c)"))) '(a 1 2 . c))

;; == backquote eval ==
(qa (let ((x 10) (y '(1 2))) (eval '`(a ,x ,@y b))) '(a 10 1 2 b))
(qa (let ((x 10)) (eval '`(a ,x . ,(list 1)))) '(a 10 . (1)))
(qa (eval '`(1 ,(+ 1 1) ,@(list 3 4))) '(1 2 3 4))
(qa (eval '`(a . ,(+ 1 2))) '(a . 3))
;; ,@ after `.' is a literal element, not a splice
(qa (eval '`(a . ,@'(1 2))) '(a \,@ '(1 2)))
;; splicing nil / empty list inserts nothing
(qa (eval '`(splice ,@nil end)) '(splice end))
(qa (eval '`(,@(list))) nil)
;; vector backquote splices too
(qa (eval '`[1 ,(+ 1 1) ,@(list 3)]) [1 2 3])
;; nested backquote: ``(a ,,x) evaluates inner level once → (a ,7)
(qa (let ((x 7)) (eval '``(a ,,x))) '(\` (a (\, 7))))
;; ``(a ,b) reads as nested (\` (\` (a (\, b))))
(qa (car (read-from-string "``(a ,b)")) '(\` (\` (a (\, b)))))
;; macroexpand of a backquote form yields its cons-ing code
(qa (macroexpand '`(a ,b ,@c)) '(cons 'a (cons b c)))
;; a macro defined with nested backquote expands via macroexpand-1
(qa (let ()
      (defmacro qa-m-bq (x y) ``(,x ,,y))
      (macroexpand-1 '(qa-m-bq 1 2)))
    '(\` ((\, x) (\, 2))))

;; == macroexpand-all: `cond' clause tests expand too ==
;; GNU `macroexp--all-clauses' expands every element of each clause —
;; including the test.  Regression: remacs used to leave test-position
;; macro calls raw (comp.el's `with-comp-cstr-accessors' inside `cond').
(qa (macroexpand-all '(cond ((when a b)) (t c)))
    '(cond ((if a (progn b))) (t c)))
(qa (macroexpand '(cl-macrolet ((foo (x) `(+ ,x 1)))
                     (cond ((foo a)) (t b))))
    '(cond ((+ a 1)) (t b)))
(qa (macroexpand-all '(cond ((and a b) (when c d))))
    '(cond ((and a b) (if c (progn d)))))
;; A non-symbol non-lambda head makes the whole form verbatim in GNU
;; (`macroexp--expand-all' `_' arm) — args are NOT descended into.
(qa (macroexpand '(cl-macrolet ((foo (x) `(+ ,x 1)))
                     ((foo a) (foo b))))
    '((foo a) (foo b)))
(qa (macroexpand-all '((if x y))) '((if x y)))

(princ (qa--done))
