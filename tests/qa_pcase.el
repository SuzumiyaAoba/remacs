;; pcase — GNU-verified expectations for UPATs/QPATs and pcase-* helpers.
(require 'cl-lib)
(require 'seq)   ;; `seq' UPAT
(require 'map)   ;; `map' UPAT is NOT loaded by default in -Q batch

;; == literal UPATs ==
;; Numbers, strings, chars and keywords self-quote; symbols need '.
(qa (pcase 5 (5 'int)) 'int)
(qa (pcase "ab" ("ab" 'str)) 'str)
(qa (pcase ?x (?x 'char)) 'char)
(qa (pcase :k (:k 'key)) 'key)
(qa (pcase 'sym ('sym 'qsym)) 'qsym)
;; nil/t/floats are NOT self-quoting patterns: bare `nil' is a macroexpand
;; error ("Unknown pattern 'nil'"); quote them.
(qa (pcase nil ('nil 'nil-lit)) 'nil-lit)
(qa (pcase t ('t 't-lit)) 't-lit)
(qa (pcase 'sym ('nil 'x) ('t 'y) (_ 'no)) 'no)
(qa (pcase 1.5 ('1.5 'float)) 'float)
;; Quoted composite literals compare with `equal'.
(qa (pcase '(a b) ('(a b) 'qlist)) 'qlist)
(qa (pcase "s" ('"s" 'qstr)) 'qstr)

;; == `_` wildcard and variable binders ==
(qa (pcase 99 (_ 'wild)) 'wild)
(qa (pcase 7 (x x)) 7)
(qa (pcase 7 (x (list x x))) '(7 7))
(qa (pcase 1 (x 'v)) 'v)  ; bare var is a catch-all
;; Repeating a var inside one pattern is an `eql' check, not a fresh bind:
;; equal strings differ under eql, equal floats compare equal.
(qa (pcase '(1 1) (`(,x ,x) 'same) (_ 'diff)) 'same)
(qa (pcase '(1 2) (`(,x ,x) 'same) (_ 'diff)) 'diff)
(qa (pcase '("ab" . "ab") (`(,x . ,x) 'same) (_ 'diff)) 'diff)
(qa (pcase '(1.0 . 1.0) (`(,x . ,x) 'same) (_ 'diff)) 'same)

;; == backquote QPATs ==
(qa (pcase '(1 2 3) (`(1 ,a 3) a)) 2)
(qa (pcase '(1 (2 3)) (`(1 (2 ,b)) b)) 3)
(qa (pcase '(a . b) (`(a . ,d) d)) 'b)
(qa (pcase '(1 2 3) (`(,a . ,rest) (list a rest))) '(1 (2 3)))
(qa (pcase '(x . 2) (`(x . 2) 'lit)) 'lit)
(qa (pcase '(x . 3) (`(x . 2) 'lit) (_ 'no)) 'no)
(qa (pcase '("k" . 1) (`("k" . ,v) v)) 1)
(qa (pcase '(1 2) (`(1 ,b) b)) 2)
(qa (pcase '(1 2 3) (`(,_ ,b ,_) b)) 2)
;; Nested , inside ,.
(qa (pcase '(1 . (2 3)) (`(,a . (,_ ,c)) (list a c))) '(1 3))
(qa (pcase '((a 1) (b 2)) (`((a ,x) (,y 2)) (list x y))) '(1 b))
;; QPATs match vectors too.
(qa (pcase [1 2] (`[,a ,b] (list a b))) '(1 2))
(qa (pcase [x y] (`[,p ,q] (list p q))) '(x y))
;; `(,x . ,y) only matches conses.
(qa (pcase 'zz (`(,x . ,y) 'm)) nil)
;; ,@ is an unsupported QPAT (signals `error' at macroexpand; eval'd so the
;; load-time macroexpander can't see it).
(qa-err (eval '(pcase '(1 2 3) (`(,a ,@b) (list a b)))) 'error)

;; == (app F UPAT) == — F applied to EXPVAL, result matched by UPAT.
(qa (pcase "abc" ((app length 3) 'len3)) 'len3)
(qa (pcase '(1 2) ((app car 1) 'car1)) 'car1)
(qa (pcase '(1 2) ((app car a) a)) 1)
(qa (pcase '(1 2 3) ((app cdr `(,_ ,b)) b)) 3)
(qa (pcase "hi" ((app intern 'hi) 'sym)) 'sym)
(qa (pcase '(1 2) ((and (app car 1) (app cadr b)) b)) 2)

;; == (and UPAT...) — all must match; bindings cumulate ==
(qa (pcase 5 ((and (pred integerp) x) x)) 5)
(qa (pcase '(5 . 5) ((and `(,x . ,x) (app car 5)) 'all)) 'all)
(qa (pcase 7 ((and (pred integerp) (pred (< 3))) 'big) (_ 'no)) 'big) ; (< 3 7)
(qa (pcase '(3) ((and `(,x) (pred listp) (guard (cl-oddp x))) 'ok) (_ 'no)) 'ok)

;; == (or UPAT...) — every branch must bind the same vars ==
(qa (pcase 5 ((or 6 5) 'm)) 'm)
(qa (pcase 'a ((or 'a 'b) 'm)) 'm)
(qa (pcase '(a . 1) ((or `(,k . 1) `(,k . 2)) k)) 'a)
(qa (pcase '(a . 2) ((or `(,k . 1) `(,k . 2)) k)) 'a)
(qa (pcase 7 ((or (pred stringp) (pred (< 6))) 'm)) 'm) ; (< 6 7)
(qa (pcase 5 ((or (and (pred integerp) (guard t)) 'z) 'm)) 'm)
(qa (pcase '(2 . 4) ((or (and `(,a . ,b) (guard (= (* 2 a) b)))
                         `(,(pred integerp) . 4))
                     'm)
            (_ 'no))
    'm)

;; == guard — an UPAT, used via `and'; EXPs see earlier bindings ==
(qa (pcase '(3 4) ((and `(,x ,y) (guard (< x y))) 'lt) (_ 'no)) 'lt)
(qa (pcase '(4 3) ((and `(,x ,y) (guard (< x y))) 'lt) (_ 'no)) 'no)
(qa (pcase '(3 4) ((and `(,x ,y) (guard (and (< x y) (cl-oddp x)))) 'lt-odd)) 'lt-odd)
(qa (pcase '(3 4) ((and `(,x ,y) (guard (let ((s (+ x y))) (= s 7)))) 'guard-let)) 'guard-let)
(qa (pcase '(2 4) ((and `(,x ,y) (guard (or (< x y) (= x y)))) 'g-or)) 'g-or)

;; == let — binds fresh vars to arbitrary EXPs inside a pattern ==
(qa (pcase 10 ((and x (let y (+ x 1))) (list x y))) '(10 11))
(qa (pcase '(1 . 2) ((let p (cons 'a 'b)) p)) '(a . b))
(qa (pcase '(1 2) ((and `(,a ,b) (let s (+ a b)) (guard (= s 3))) 'ok)) 'ok)
;; `let' on an already-bound var is an equality test: mismatch = whole
;; pattern fails (no clause matches -> nil).
(qa (pcase '(1 2) ((and `(,a ,_) (let a 99)) a)) nil)

;; == pred — (pred F), (pred #'lambda), (pred (F a1...)) ==
(qa (pcase 4 ((pred integerp) 'i)) 'i)
(qa (pcase 4 ((pred (lambda (n) (> n 2))) 'big)) 'big)
(qa (pcase :k1 ((pred keywordp) 'kw)) 'kw)
(qa (pcase '(a b) ((pred (equal '(a b))) 'ok)) 'ok)
;; (pred (F a b)) calls F with EXPVAL as the LAST arg.
(qa (pcase 4 ((pred (= 4)) 'eq4)) 'eq4)
(qa (pcase 4 ((pred (= 3)) 'eq3) (_ 'no)) 'no)
(qa (pcase 5 ((pred (< 3)) 'x)) 'x)          ; (< 3 5) -> t: expval last
(qa (pcase 5 ((pred (> 3)) 'x) (_ 'no)) 'no) ; (> 3 5) -> nil
;; memq gets the list arg first, EXPVAL last: (memq 'a EXPVAL) needs a list.
(qa (pcase '(a b) ((pred (memq 'a)) 'x) (_ 'no)) 'x)
(qa (pcase '(c d) ((pred (memq 'a)) 'x) (_ 'no)) 'no)

;; == seq UPAT — positions bind nil past the end; non-seq fails ==
(qa (pcase '(1 2 3) ((seq a b c) (list a b c))) '(1 2 3))
(qa (pcase [1 2] ((seq a b) (list a b))) '(1 2))
(qa (pcase "xy" ((seq a b) (list a b))) '(120 121))
(qa (pcase '(1 2 3 4) ((seq a b _) (list a b))) '(1 2))
(qa (pcase nil ((seq) 'empty)) 'empty)
;; GNU binds missing elements to nil — even for vectors/strings.
(qa (pcase '(1) ((seq a b) (list a b))) '(1 nil))
(qa (pcase [1] ((seq a b) (list a b))) '(1 nil))
(qa (pcase "ab" ((seq a b c) (list a b c))) '(97 98 nil))
(qa (pcase 5 ((seq a) a)) nil)
;; Nested seq patterns.
(qa (pcase '(1 (2 3) 4) ((seq a (seq b c) d) (list a b c d))) '(1 2 3 4))

;; == map UPAT — needs (require 'map); missing keys bind nil, still match ==
(qa (pcase '((a . 1) (b . 2)) ((map a b) (list a b))) '(1 2))
(qa (pcase '((a . 1)) ((map a b) (list a b))) '(1 nil))
(qa (pcase '((a . 1) (b . 2)) ((map ('a x)) x)) 1)
(qa (pcase '(:k 1 :j 2) ((map (:k x)) x)) 1)
(qa (let ((h (make-hash-table)))
      (puthash 'k 9 h)
      (pcase h ((map k) k)))
    9)
(qa (let ((h (make-hash-table)))
      (puthash 'k 9 h)
      (pcase h ((map k ('absent z)) (list k z))))
    '(9 nil))

;; == cl-struct UPAT == — cl-defstruct registers no named pattern;
;; only (cl-struct TYPE FIELD|(FIELD PAT)...) exists.
(cl-defstruct qapc-pt x y)
(qa (let ((p (make-qapc-pt :x 1 :y 2)))
      (pcase p ((cl-struct qapc-pt x y) (list x y))))
    '(1 2))
(qa (let ((p (make-qapc-pt :x 1 :y 2)))
      (pcase p ((cl-struct qapc-pt (x a) (y b)) (list a b))))
    '(1 2))
(qa (let ((p (make-qapc-pt :x 1 :y 2)))
      (pcase p ((cl-struct qapc-pt (x 9)) 'nope) (_ 'no)))
    'no)
;; A non-struct value falls through to later clauses.
(qa (pcase 'x ((cl-struct qapc-pt x y) (list x y)) (_ 'no)) 'no)
;; The struct name is not a usable pattern: (qapc-pt x y) tries to call a
;; function qapc-pt.  eval keeps the macroexpansion error out of load time.
(qa-err (eval '(pcase (make-qapc-pt :x 1 :y 2) ((qapc-pt x y) (list x y)))) 'error)

;; == rx UPAT == — substring search, not a full match.
(qa (pcase "abc" ((rx "abc") 'm)) 'm)
(qa (pcase "abc" ((rx "bc") 'm)) 'm)     ; substring counts
(qa (pcase "xbc" ((rx "a.c") 'm)) nil)   ; rx strings are literals
(qa (pcase "a.c" ((rx "a.c") 'm)) 'm)
(qa (pcase "aYc" ((rx "a" any "c") 'm)) 'm)

;; == no match -> nil; multiple body forms evaluate in order ==
(qa (pcase 5 (6 'x)) nil)
(qa (pcase 1 (1 'ignored 'last)) 'last)

;; == (not UPAT) is NOT valid in 31.1 ==
(qa-err (eval '(pcase 1 ((not 1) 'x) (_ 'no))) 'error)

;; == pcase-let / pcase-let* ==
(qa (pcase-let ((`(,a ,b) '(1 2))) (list a b)) '(1 2))
(qa (pcase-let ((`[,a ,b] [3 4])) (list a b)) '(3 4))
(qa (pcase-let ((`(,a . ,b) '(1 2))) (list a b)) '(1 (2)))
(qa (pcase-let (((seq a b) '(1 2))) (list a b)) '(1 2))
(qa (pcase-let ((`(,a ,b) '(1 2))
                ((map c) '((c . 9))))
     (list a b c))
    '(1 2 9))
;; Non-matching patterns bind nil, not skip.
(qa (pcase-let ((`(,a ,b) 'notalist)) (list a b)) '(nil nil))
(qa (pcase-let* ((`(,a ,b) '(1 2))
                 (`(,c ,d) (list b a)))
     (list a b c d))
    '(1 2 2 1))

;; == pcase-dolist — non-matching elements still run the body with nils ==
;; (lets are inside the qa form: `eval' cannot see outer lexical vars)
(qa (let ((acc nil))
      (pcase-dolist (`(,k ,v) '((a 1) (b 2))) (push (list k v) acc))
      (nreverse acc))
    '((a 1) (b 2)))
(qa (let ((acc nil))
      (pcase-dolist (`(,k ,v) '((a 1) skip (b 2))) (push (list k v) acc))
      (nreverse acc))
    '((a 1) (nil nil) (b 2)))

;; == pcase-setq — failed destructuring sets vars to nil ==
(qa (let (a b)
      (pcase-setq `(,a ,b) '(7 8))
      (list a b))
    '(7 8))
(qa (let ((a 'pa) (b 'pb))
      (pcase-setq `(,a ,b) 'x)
      (list a b))
    '(nil nil))

;; == pcase-lambda — args are patterns in the lambda list ==
(qa (funcall (pcase-lambda (`(,a ,b)) (+ a b)) '(3 4)) 7)
(qa (funcall (pcase-lambda (a b) (+ a b)) 3 4) 7)
(qa (mapcar (pcase-lambda ((map k)) k) '(((k . 5)) ((k . 6)))) '(5 6))

;; == pcase-exhaustive — signals plain `error' on no match ==
(qa (pcase-exhaustive 1 (1 'a) (2 'b)) 'a)
(qa-err (pcase-exhaustive 5 (1 'a) (2 'b)) 'error)
(qa-err-data (pcase-exhaustive 5 (1 'a) (2 'b)) 'error '("No clause matching ‘5’"))

;; == pcase-defmacro == — defines NAME--pcase-macroexpander + symbol prop.
(pcase-defmacro qapc-two (a b)
  ``(,,a . ,,b))
(pcase-defmacro qapc-wrap (p) ``(wrap ,,p))
(qa (fboundp 'qapc-two--pcase-macroexpander) t)
(qa (eq (get 'qapc-two 'pcase-macroexpander) #'qapc-two--pcase-macroexpander) t)
;; The expander takes the pattern's ARGS and returns a raw UPAT form.
;; ``(,,a . ,,b) with a=v1 b=v2 -> `(,v1 \, v2).
(qa (qapc-two--pcase-macroexpander 'v1 'v2) '`(,v1 \, v2))
(qa (pcase '(1 2) ((qapc-two x y) (list x y))) '(1 (2)))
(qa (pcase '(wrap 9) ((qapc-wrap v) v)) 9)
;; Custom-pattern non-match with a real fallthrough.
(qa (pcase '((1 2)) ((qapc-wrap v) v) ((pred listp) 'other)) 'other)

(princ (qa--done))
