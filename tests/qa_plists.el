;; Property lists and symbol properties — GNU Emacs 31.1 semantics.
;;
;; plist-get/plist-member/plist-put compare keys with `eq' and accept an
;; optional PREDICATE argument (a function; non-functions signal
;; invalid-function).  Odd-length proper tails are ignored by the readers:
;; the last key has no value cell.  Walking into a DOTTED tail makes
;; plist-member signal wrong-type-argument (plistp PLIST); plist-put validates
;; whenever it must append and signals the same for odd/improper input, but
;; in-place updates of an existing key skip validation.  plist-put mutates
;; existing pairs in place and APPENDS missing pairs at the tail via nconc.
;; lax-plist-get/lax-plist-put use `equal'; `lax-plist-member' does not exist.
;; `put' appends to a symbol's plist; storing nil keeps the (PROP nil) pair.
;; get/put/setplist/symbol-plist require symbols; `function-get' tolerates
;; non-symbols (returns nil) and reads props through function-cell
;; indirection (the alias target's plist), while `function-put' writes on the
;; symbol itself.  `setplist' stores ANY object verbatim.  Keywords are
;; interned self-quoting constants: keywordp is nil for uninterned symbols.
;; cl-lib provides cl-remprop/cl-getf/cl-remf/cl-incf/cl-decf/cl-plusp/
;; cl-minusp; `remprop' itself is not fboundp in 31.1.

(require 'cl-lib)

;; == plist-get: eq key comparison ==
(qa (plist-get '(a 1 b 2) 'a) 1)
(qa (plist-get '(a 1 b 2) 'c) nil)
(qa (plist-get nil 'a) nil)
(qa (plist-get '(a nil) 'a) nil) ;; stored nil == missing, only member tells
(qa (plist-get '(a 1 a 2) 'a) 1) ;; first match wins
(qa (plist-get '(:a 1 :b 2) :b) 2)
(qa (plist-get (list "k" 1) "k") nil) ;; string keys: eq, not equal
(qa (plist-get (list 1 'i 2 'x) 2) 'x) ;; numeric keys work via eq
(qa (plist-get (list 1 'i) 1.0) nil) ;; 1.0 is not `eq' to 1
(qa (plist-get '((a) v) '(a)) nil) ;; list key: eq compares identity
(let ((u (make-symbol "k")))
  (qa (plist-get (list u 'v) u) 'v))
(let ((u1 (make-symbol "k")) (u2 (make-symbol "k")))
  (qa (plist-get (list u1 'v) u2) nil)) ;; distinct uninterned symbols

;; == readers tolerate odd/dotted tails ==
(qa (plist-get '(a) 'a) nil) ;; key present but no value cell
(qa (plist-member '(a) 'a) '(a))
(qa (plist-get '(a 1 b) 'b) nil)
(qa (plist-member '(a 1 b) 'b) '(b))
(qa (plist-member '(a 1 b) 'z) nil) ;; odd but proper tail: no error
(qa (plist-get '(a . b) 'a) nil)
(qa (plist-get '(a . b) 'z) nil)
(qa (plist-get 5 'a) nil)
(qa (plist-member '(a . b) 'a) '(a . b)) ;; found before the bad tail
(qa (plist-member '(a 1 . b) 'a) '(a 1 . b))
;; full walks validate the whole list
(qa-err-data (plist-member 5 'a) 'wrong-type-argument '(plistp 5))
(qa-err-data (plist-member '(a . b) 'z) 'wrong-type-argument '(plistp (a . b)))
(qa-err-data (plist-member '(a 1 . b) 'z) 'wrong-type-argument '(plistp (a 1 . b)))

;; == plist-member returns the tail ==
(qa (plist-member '(a 1 b 2) 'b) '(b 2))
(qa (plist-member '(a 1 b 2) 'z) nil)
(qa (plist-member '(a nil) 'a) '(a nil)) ;; disambiguates nil value
(let ((l (list 'a 1 'b 2 'c 3)))
  (qa-same (plist-member l 'b) (nthcdr 2 l))) ;; the actual cons object

;; == plist-put ==
(qa (plist-put nil 'a 1) '(a 1))
(let ((l (list 'a 1)))
  (qa (plist-put l 'a 9) '(a 9))
  (qa l '(a 9))) ;; in-place update
(let ((l (list 'a 1)))
  (qa (plist-put l 'b 2) '(a 1 b 2)) ;; missing key APPENDS at tail
  (qa l '(a 1 b 2))) ;; destructive nconc — the variable sees it
(let ((l (list 'a 1 'b 2)))
  (qa (plist-put l 'b 9) '(a 1 b 9))
  (qa (plist-put l 'c 3) '(a 1 b 9 c 3)))
(qa (plist-put (list "k" 1) "k" 9) '("k" 1 "k" 9)) ;; eq miss appends a dup
(qa-err-data (plist-put 5 'a 1) 'wrong-type-argument '(plistp 5))
(qa-err-data (plist-put '(a . b) 'z 1) 'wrong-type-argument '(plistp (a . b)))
(qa-err-data (plist-put '(a 1 b) 'b 9) 'wrong-type-argument '(plistp (a 1 b)))
(qa-err-data (plist-put '(a) 'a 9) 'wrong-type-argument '(plistp (a)))
(qa (plist-put '(a 1 . b) 'a 9) '(a 9 . b)) ;; in-place path skips validation

;; == optional PREDICATE argument ==
(qa (plist-get (list "k" 1) "k" #'equal) 1)
(qa (plist-get (list "k" 1) "k" #'eq) nil)
(qa (plist-get '(a 1) 'a #'equal) 1)
(qa (plist-member (list "k" 1) "k" #'equal) '("k" 1))
(qa (plist-member (list "k" 1) "k" #'eq) nil)
(let ((l (list "k" 1)))
  (qa (plist-put l "k" 9 #'equal) '("k" 9))
  (qa l '("k" 9)))
(qa-err-data (plist-get '(a 1) 'a 5) 'invalid-function '(5))
(qa-err-data (plist-put '(a 1) 'a 2 5) 'invalid-function '(5))

;; == lax-plist-*: equal comparison ==
(qa (fboundp 'lax-plist-member) nil)
(qa (lax-plist-get (list "k" 1) "k") 1)
(qa (lax-plist-get '((a) v) '(a)) 'v) ;; list keys match under equal
(qa (lax-plist-get (list 1 'i) 1) 'i)
(qa (lax-plist-get (list 1 'i) 1.0) nil) ;; 1 /= 1.0 under equal
(let ((u1 (make-symbol "k")) (u2 (make-symbol "k")))
  (qa (lax-plist-get (list u1 'v) u2) nil)) ;; uninterned: not equal either
(qa (lax-plist-get '(a 1) 'a) 1)
(qa (lax-plist-get '(a 1) 'z) nil)
(qa (lax-plist-get '(a) 'a) nil)
(qa (lax-plist-get '(a . b) 'a) nil)
(qa (lax-plist-get '(a 1 b) 'b) nil)
(qa (lax-plist-get '("k" 1 "k" 9) "k") 1) ;; first equal key wins
(qa (lax-plist-put nil "k" 1) '("k" 1))
(let ((l (list "k" 1)))
  (qa (lax-plist-put l "k" 9) '("k" 9)) ;; equal key updates in place
  (qa l '("k" 9)))
(let ((l (list 'a 1)))
  (qa (lax-plist-put l 'b 2) '(a 1 b 2))
  (qa l '(a 1 b 2)))
(qa-err-data (lax-plist-put '(a . b) 'a 9)
             'wrong-type-argument '(plistp (a . b)))
(qa-err-data (lax-plist-put '(a 1 b) 'b 9)
             'wrong-type-argument '(plistp (a 1 b)))

;; == plistp ==
(qa (plistp nil) t)
(qa (plistp 'a) nil)
(qa (plistp '(a b)) t)
(qa (plistp '(a b c)) nil)
(qa (plistp '(a . b)) nil)
(qa (plistp "ab") nil)
(qa (plistp [1 2]) nil)

;; == get / put on symbols ==
(let ((s (make-symbol "qap-s")))
  (qa (put s 'p 'v) 'v) ;; put returns VALUE
  (qa (get s 'p) 'v)
  (qa (symbol-plist s) '(p v))
  (qa (put s 'p nil) nil)
  (qa (get s 'p) nil)
  (qa (symbol-plist s) '(p nil))) ;; (PROP nil) pair persists
(let ((s (make-symbol "qap-ord")))
  (put s 'a 1)
  (put s 'b 2)
  (qa (symbol-plist s) '(a 1 b 2)) ;; put appends
  (put s 'a 9)
  (qa (symbol-plist s) '(a 9 b 2))) ;; overwrite keeps position
(let ((s (make-symbol "qap-str")))
  (qa (put s "p" 'v) 'v)
  (qa (get s "p") nil)               ;; eq on strings: literal is a different object
  (qa (symbol-plist s) '("p" v)))
(let ((s (make-symbol "qap-nil")))
  (put s nil 42)
  (qa (get s nil) 42) ;; nil is a valid property name
  (qa (symbol-plist s) '(nil 42)))
(let ((s :qap-kw2))
  (put s 'qp 'v)
  (qa (get :qap-kw2 'qp) 'v)) ;; keywords are ordinary symbols here
(qa-err-data (get 5 'p) 'wrong-type-argument '(symbolp 5))
(qa-err-data (put 5 'p 'v) 'wrong-type-argument '(symbolp 5))
(qa-err-data (get '(a) 'b) 'wrong-type-argument '(symbolp (a)))
(qa-err-data (get 'a) 'wrong-number-of-arguments '(get 1))
(qa-err-data (put 'a 'b) 'wrong-number-of-arguments '(put 2))

;; == symbol-plist / setplist ==
(let ((s (make-symbol "qap-sp")))
  (qa (setplist s '(a 1 b 2)) '(a 1 b 2)) ;; returns the plist argument
  (qa (symbol-plist s) '(a 1 b 2))
  (qa (get s 'b) 2)
  (qa-same (symbol-plist s) (symbol-plist s)) ;; same cons object
  (setplist s '(x 1))
  (qa (get s 'a) nil) ;; replaced wholesale, old props gone
  (qa (symbol-plist s) '(x 1))
  (qa (setplist s nil) nil)
  (qa (symbol-plist s) nil))
(let ((s (make-symbol "qap-junk")))
  (qa (setplist s 'atom) 'atom) ;; ANY object stored verbatim
  (qa (symbol-plist s) 'atom)
  (qa (get s 'p) nil))
(let ((s (make-symbol "qap-dot")))
  (setplist s '(a 1 . b))
  (qa (symbol-plist s) '(a 1 . b))
  (qa (get s 'a) 1))
(qa-err-data (symbol-plist 5) 'wrong-type-argument '(symbolp 5))
(qa-err-data (setplist 5 '(a)) 'wrong-type-argument '(symbolp 5))

;; == keywords ==
(qa (keywordp :x) t)
(qa (keywordp 'x) nil)
(qa (keywordp :) t)
(qa (keywordp "x") nil)
(qa (keywordp (make-symbol ":x")) nil) ;; uninterned: not a keyword
(qa (keywordp (intern ":qap-k")) t)
(qa-same (intern ":qap-k") :qap-k)
(qa (boundp (intern ":qap-b")) t) ;; keywords are always bound
(qa-err-data (set (intern ":qap-c") 1) 'setting-constant '(:qap-c))
(qa-err-data (makunbound (intern ":qap-d")) 'setting-constant '(:qap-d))
(qa-err-data (setq :qap-e 1) 'setting-constant '(:qap-e))

;; == function-get / function-put ==
(qa (fboundp 'function-get) t)
(qa (fboundp 'function-put) t)
;; function-get reads through function-cell indirection; get does not
(defalias 'qap-fa 'qap-fr)
(put 'qap-fr 'qap-p 'deep)
(qa (function-get 'qap-fa 'qap-p) 'deep)
(qa (get 'qap-fa 'qap-p) nil)
(defalias 'qap-fa2 'qap-fa)
(qa (function-get 'qap-fa2 'qap-p) 'deep) ;; two hops
;; function-put writes on the symbol itself, not the target
(qa (function-put 'qap-fa2 'qap-p2 'v2) 'v2)
(qa (get 'qap-fa2 'qap-p2) 'v2)
(qa (get 'qap-fr 'qap-p2) nil)
(qa (function-get 'qap-nofunc 'p) nil) ;; void function cell: still nil
(qa (function-get 'car 'qap-missing) nil)
(qa (function-get 5 'p) nil) ;; non-symbol tolerated
(qa-err-data (function-put 5 'p 'v)
             'wrong-type-argument '((symbolp symbol-with-pos-p) 5))

;; == cl-lib property functions ==
(qa (fboundp 'remprop) nil) ;; the bare `remprop' does not exist in 31.1
(qa (fboundp 'cl-remprop) t)
(let ((s (make-symbol "qap-cr")))
  (put s 'p 'v)
  (qa (cl-remprop s 'p) t) ;; t when a property was removed
  (qa (get s 'p) nil)
  (qa (symbol-plist s) nil)
  (qa (cl-remprop s 'absent) nil))
(let ((s (make-symbol "qap-cg")))
  (put s 'p 'v)
  (qa (cl-get s 'p) 'v))
(let ((l (list 'a 1 'b 2)))
  (qa (cl-getf l 'b) 2)
  (qa (cl-getf l 'z) nil)
  (qa (cl-getf l 'z 'd) 'd))
(let ((l (list 'a 1 'b 2)))
  (qa (cl-remf l 'a) t) ;; generalized place: the variable is rebound
  (qa l '(b 2)))
(let ((l (list 'a 1 'b 2)))
  (qa (cl-remf l 'z) nil)
  (qa l '(a 1 b 2)))
;; symbol-plist is itself a valid generalized place
(let ((s (make-symbol "qap-spl")))
  (setplist s '(a 1 b 2))
  (qa (cl-remf (symbol-plist s) 'a) t)
  (qa (symbol-plist s) '(b 2)))
(let ((s (make-symbol "qap-ss")))
  (qa (setf (symbol-plist s) '(x 1)) '(x 1))
  (qa (symbol-plist s) '(x 1)))

;; == cl-incf / cl-decf on property places ==
(let ((s (make-symbol "qap-ci")))
  (put s 'n 5)
  (qa (cl-incf (get s 'n)) 6)
  (qa (get s 'n) 6)
  (qa (cl-decf (get s 'n) 2) 4)
  (qa (get s 'n) 4))
(let ((l (list :n 10)))
  (qa (cl-incf (plist-get l :n) 3) 13)
  (qa l '(:n 13))
  (qa (cl-decf (plist-get l :n)) 12)
  (qa l '(:n 12)))
(let ((l (list :a 1)))
  (qa-err-data (cl-incf (plist-get l :z))
               'wrong-type-argument '(number-or-marker-p nil))
  (qa l '(:a 1)))
(let ((l (list 'a 5)))
  (qa (cl-incf (cl-getf l 'a) 5) 10)
  (qa l '(a 10)))

;; == cl-plusp / cl-minusp ==
(qa (cl-plusp 1) t)
(qa (cl-plusp 1.5) t)
(qa (cl-plusp 0) nil)
(qa (cl-plusp -1) nil)
(qa-err-data (cl-plusp 'a) 'wrong-type-argument '(number-or-marker-p a))
(qa (cl-minusp -1) t)
(qa (cl-minusp 0) nil)
(qa (cl-minusp 1) nil)
(qa-err-data (cl-minusp 'a) 'wrong-type-argument '(number-or-marker-p a))

;; == setf places ==
(let ((l (list 'a 1)))
  (setf (plist-get l 'a) 9)
  (qa l '(a 9)))
(let ((l (list 'a 1)))
  (setf (plist-get l 'b) 9)
  (qa l '(b 9 a 1))) ;; the gv setter PREPENDS a missing key
(let ((s (make-symbol "qap-sv")))
  (setf (get s 'p) 'v)
  (qa (symbol-plist s) '(p v)))
;; == hash-table supplements ==
;; stored nil is indistinguishable from missing, even with a default
(let ((h (make-hash-table)))
  (puthash 'k nil h)
  (qa (gethash 'k h 'dflt) nil)
  (qa (hash-table-count h) 1))
(qa (gethash 'k-miss (make-hash-table) 'dflt) 'dflt)
;; define-hash-table-test installs a named test
(define-hash-table-test 'qap-ht-test #'eq #'sxhash)
(let ((h (make-hash-table :test 'qap-ht-test)))
  (puthash 'k 1 h)
  (qa (gethash 'k h) 1)
  (qa (hash-table-test h) 'qap-ht-test))
(qa-err-data (define-hash-table-test 5 #'eq #'sxhash)
             'wrong-type-argument '(symbolp 5))
;; weakness values and validation
(let ((h (make-hash-table)))
  (qa (hash-table-weakness h) nil))
(let ((h (make-hash-table :weakness 'value)))
  (qa (hash-table-weakness h) 'value))
(let ((h (make-hash-table :weakness 'key-or-value)))
  (qa (hash-table-weakness h) 'key-or-value))
(let ((h (make-hash-table :weakness 'key-and-value)))
  (qa (hash-table-weakness h) 'key-and-value))
(qa-err-data (make-hash-table :weakness 'qap-bogus)
             'error '("Invalid hash table weakness" qap-bogus))
;; size validation
(qa-err-data (make-hash-table :size -1) 'error '("Invalid hash table size" -1))
(qa-err-data (make-hash-table :size 1.5) 'error '("Invalid hash table size" 1.5))
(let ((h (make-hash-table :size 0)))
  (qa (hash-table-size h) 0))
;; accessor type checks
(qa-err-data (hash-table-test 'x) 'wrong-type-argument '(hash-table-p x))
(qa-err-data (hash-table-size '(1)) 'wrong-type-argument '(hash-table-p (1)))
(qa-err-data (hash-table-weakness nil) 'wrong-type-argument '(hash-table-p nil))
;; sxhash-equal: equal objects hash identically (pin the property only)
(qa (= (sxhash-equal '(a b)) (sxhash-equal (list 'a 'b))) t)
(qa (= (sxhash-equal "x") (sxhash-equal (copy-sequence "x"))) t)

(princ (qa--done))
