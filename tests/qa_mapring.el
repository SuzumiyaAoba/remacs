;; map.el + ring.el + let-alist — GNU-verified 31.1 semantics.
;; `map-put' (obsolete since 27.1, byte-compile warnings) is deliberately
;; untested; `map-put!' and (setf (map-elt ...)) are covered instead.
;; Note: a non-nil list whose car is an atom is treated as a *plist*
;; (map--plist-p checks only `(atom (car list))'), so '(a (b . 2)) is a
;; plist whose first "value" is the cons (b . 2).
(require 'map)
(require 'ring)
(require 'let-alist)

;; == map-elt: lookup on alist / plist / vector / string / hash ==
(qa (map-elt '((a . 1) (b . 2)) 'a) 1)
(qa (map-elt '((a . 1) (b . 2)) 'z) nil)
(qa (map-elt '((a . 1) (b . 2)) 'z 'dflt) 'dflt)
(qa (map-elt '(a 1 b 2) 'b) 2)
(qa (map-elt '(a 1 b 2) 'z 'dflt) 'dflt)
(qa (map-elt [x y z] 0) 'x)
(qa (map-elt [x y z] 2) 'z)
(qa (map-elt "abc" 1) ?b)
;; out-of-bounds on an array returns DEFAULT, never an error.
(qa (map-elt [x y] 5) nil)
(qa (map-elt [x y] 5 'dflt) 'dflt)
(qa (map-elt [x y] -1) nil)
(qa (map-elt "ab" 9 'd) 'd)
(qa (map-elt nil 'a 'd) 'd)
;; a present key whose value is nil does NOT yield DEFAULT.
(qa (map-elt '(a nil b 2) 'a 'dflt) nil)
(qa (map-elt '((a) (b . 2)) 'a 'dflt) nil)
;; alist value is the whole cdr of the pair.
(qa (map-elt '(("k" 1 2)) "k") '(1 2))
;; TESTFN (deprecated): alist defaults to `equal'.
(qa (map-elt '(("k" . 1)) "k" 'd) 1)
(qa (map-elt '(("k" . 1)) "k" 'd #'eq) 'd)
(qa (map-elt '((a . 1)) 'a 'd #'eq) 1)
;; plist keys default to `eq' comparison.
(qa (map-elt '("k" 1) "k" 'd) 'd)
(qa (map-elt '("k" 1) "k" 'd #'equal) 1)
;; hash-table TESTFN is ignored; the table's own test is used.
(let ((h (make-hash-table)))
  (qa (puthash "k" 7 h) 7)
  (qa (map-elt h "k") nil)
  (qa (map-elt h "k" 'd #'equal) 'd))
(let ((h (make-hash-table :test 'equal)))
  (puthash "k" 7 h)
  (qa (map-elt h "k") 7)
  (qa (map-elt h "z" 'd) 'd)
  (qa (map-elt h "k" 'd #'eq) 7))
(qa (map-elt (make-bool-vector 2 t) 1) t)
(qa (map-elt (make-bool-vector 2 t) 5 'd) 'd)
;; the "looks-like-a-plist" ambiguity: '(a (b . 2)) is a plist.
(qa (map-elt '(a (b . 2)) 'a) '(b . 2))
(qa (map-elt '(1 a 2 b) 1) 'a)
(qa-err (map-elt 5 'a) 'cl-no-applicable-method)

;; == map-contains-key ==
;; alist/vector/string/hash return t or nil; plist returns the tail cons.
(qa (map-contains-key '((a . 1)) 'a) t)
(qa (map-contains-key '((a . 1)) 'z) nil)
(qa (map-contains-key '(a 1 b 2) 'b) '(b 2))
(qa (map-contains-key '(a nil) 'a) '(a nil))
(qa (map-contains-key '(a 1 b 2) 'z) nil)
(qa (map-contains-key '(("k" . 1)) "k") t)
(qa (map-contains-key '(("k" . 1)) "k" #'eq) nil)
(qa (map-contains-key [a b] 0) t)
(qa (map-contains-key [a b] 2) nil)
(qa (map-contains-key [a b] -1) nil)
(qa (map-contains-key "ab" 0) t)
(qa (map-contains-key nil 'a) nil)
(let ((h (make-hash-table)))
  (puthash 'a 1 h)
  (qa (map-contains-key h 'a) t)
  (qa (map-contains-key h 'z) nil))

;; == map-put! / (setf (map-elt ...)) ==
;; map-put! returns VALUE; existing keys update in place.
(let ((m (list '(a . 1))))
  (qa (map-put! m 'a 9) 9)
  (qa m '((a . 9))))
(let ((m '(a 1 b 2)))
  (qa (map-put! m 'b 9) 9)
  (qa m '(a 1 b 9)))
;; a NEW key on a plist mutates the tail cons in place.
(let ((m '(a 1)))
  (qa (map-put! m 'b 2) 2)
  (qa m '(a 1 b 2)))
;; a NEW key on an alist cannot be added in place -> map-not-inplace.
(let ((m (list '(a . 1))))
  (qa-err-data (map-put! m 'b 2) 'map-not-inplace '(((a . 1)))))
(let ((m nil))
  (qa-err (map-put! m 'a 1) 'map-not-inplace))
(let ((v (vector 'a 'b)))
  (qa (map-put! v 0 'z) 'z)
  (qa v '[z b])
  (qa-err-data (map-put! v 5 'z) 'args-out-of-range '([z b] 5)))
(let ((s (copy-sequence "ab")))
  (qa (map-put! s 0 ?x) ?x)
  (qa s "xb"))
(let ((h (make-hash-table)))
  (qa (map-put! h 'k 1) 1)
  (qa (map-elt h 'k) 1))
;; (setf (map-elt ...)) falls back to map-insert on map-not-inplace,
;; so adding a new alist key works through setf (prepends, non-inplace).
(let ((m (list '(a . 1))))
  (setf (map-elt m 'b) 2)
  (qa m '((b . 2) (a . 1))))
(let ((m nil))
  (setf (map-elt m 'a) 1)
  (qa m '((a . 1))))
(let ((h (make-hash-table)))
  (setf (map-elt h 'a) 1)
  (qa (map-elt h 'a) 1))

;; == map-delete ==
;; deleting the first pair of a list returns the tail WITHOUT mutating.
(let ((m (list '(a . 1) '(b . 2))))
  (qa (map-delete m 'a) '((b . 2)))
  (qa m '((a . 1) (b . 2))))
;; deleting a non-first pair splices the list in place.
(let ((m (list '(a . 1) '(b . 2) '(c . 3))))
  (qa (map-delete m 'b) '((a . 1) (c . 3)))
  (qa m '((a . 1) (c . 3))))
(let ((m '(a 1 b 2 c 3)))
  (qa (map-delete m 'b) '(a 1 c 3))
  (qa m '(a 1 c 3)))
(let ((m '(a 1 b 2)))
  (qa (map-delete m 'a) '(b 2))
  (qa m '(a 1 b 2)))
;; deleting an absent key is a no-op returning the map.
(let ((m (list '(a . 1))))
  (qa (map-delete m 'zzz) '((a . 1))))
;; array slots can't be removed; KEY is stored as nil instead.
(let ((v (vector 'a 'b)))
  (qa (map-delete v 0) '[nil b])
  (qa v '[nil b]))
(let ((h (make-hash-table)))
  (puthash 'a 1 h) (puthash 'b 2 h)
  (qa (eq h (map-delete h 'a)) t)
  (qa (map-keys h) '(b)))
(qa-err-data (map-delete "ab" 0) 'wrong-type-argument '(characterp nil))

;; == map-keys / map-values / map-pairs / map-length ==
(qa (map-keys '((a . 1) (b . 2))) '(a b))
(qa (map-values '((a . 1) (b . 2))) '(1 2))
(qa (map-keys '(a 1 b 2)) '(a b))
(qa (map-values '(a 1 b 2)) '(1 2))
(qa (map-keys [x y]) '(0 1))
(qa (map-values [x y]) '(x y))
(qa (map-pairs '((a . 1) (b . 2))) '((a . 1) (b . 2)))
(qa (map-pairs '(a 1 b 2)) '((a . 1) (b . 2)))
(qa (map-pairs [x y]) '((0 . x) (1 . y)))
(qa (map-pairs "ab") '((0 . 97) (1 . 98)))
(qa (map-pairs (make-bool-vector 2 nil)) '((0) (1)))
(qa (map-pairs nil) nil)
(qa (map-length '((a . 1) (b . 2))) 2)
(qa (map-length '(a 1 b 2)) 2)
(qa (map-length [a b c]) 3)
(qa (map-length "abc") 3)
(qa (map-length nil) 0)
(let ((h (make-hash-table)))
  (qa (map-length h) 0)
  (puthash 'a 1 h) (puthash 'b 2 h)
  (qa (map-length h) 2)
  (qa (sort (map-keys h) #'string<) '(a b))
  (qa (sort (map-values h) #'<) '(1 2)))

;; == map-copy / map-empty-p / mapp ==
(qa (map-copy '((a . 1))) '((a . 1)))
(qa (type-of (map-copy [a b])) 'vector)
(qa (map-copy "ab") "ab")
(qa (map-copy nil) nil)
(let* ((m (list (cons 'a 1))) (c (map-copy m)))
  (setf (map-elt c 'a) 9)
  (qa m '((a . 1)))
  (qa c '((a . 9))))
(let ((h (make-hash-table :test 'equal)))
  (qa (hash-table-test (map-copy h)) 'equal))
(qa (map-empty-p '()) t)
(qa (map-empty-p nil) t)
(qa (map-empty-p '((a . 1))) nil)
(qa (map-empty-p '(a 1)) nil)
(qa (map-empty-p []) t)
(qa (map-empty-p "") t)
(qa (mapp '((a . 1))) t)
(qa (mapp '(a 1)) t)
(qa (mapp nil) t)
(qa (mapp [1]) t)
(qa (mapp "ab") t)
(qa (mapp 5) nil)

;; == map-apply / map-do / map-keys-apply / map-values-apply ==
(qa (map-apply #'cons '((a . 1) (b . 2))) '((a . 1) (b . 2)))
(qa (map-apply #'list '(a 1 b 2)) '((a 1) (b 2)))
(qa (map-apply #'cons [a b]) '((0 . a) (1 . b)))
(qa (map-apply #'cons nil) nil)
(qa (map-apply (lambda (k v) (+ k v)) '((1 . 10) (2 . 20))) '(11 22))
(let ((acc nil))
  (qa (map-do (lambda (k v) (push (list k v) acc)) '((a . 1) (b . 2))) nil)
  (qa acc '((b 2) (a 1))))
(let ((h (make-hash-table)))
  (puthash 'a 1 h) (puthash 'b 2 h)
  (qa (sort (map-apply #'cons h)
            (lambda (x y) (string< (symbol-name (car x)) (symbol-name (car y)))))
      '((a . 1) (b . 2)))
  (qa (sort (map-values-apply #'1+ h) #'<) '(2 3)))
(qa (map-keys-apply #'1+ '((1 . a) (2 . b))) '(2 3))
(qa (map-values-apply #'1+ '((a . 1) (b . 2))) '(2 3))
(qa (map-values-apply #'1+ [1 2 3]) '(2 3 4))

;; == map-into ==
(qa (map-into '((a . 1)) 'list) '((a . 1)))
(qa (map-into '((a . 1) (b . 2)) 'alist) '((a . 1) (b . 2)))
(qa (map-into '((a . 1) (b . 2)) 'plist) '(a 1 b 2))
(qa (map-into '(a 1 b 2) 'list) '((a . 1) (b . 2)))
(qa (map-into [x y] 'list) '((0 . x) (1 . y)))
(qa (map-into "ab" 'list) '((0 . 97) (1 . 98)))
(let ((h (map-into '((a . 1) (b . 2)) 'hash-table)))
  (qa (hash-table-p h) t)
  (qa (hash-table-test h) 'equal)
  (qa (map-elt h 'b) 2))
(let ((h (map-into [x y] 'hash-table)))
  (qa (map-elt h 1) 'y))
;; TYPE may be (hash-table KEYWORD-ARGS...) forwarded to make-hash-table.
(qa (hash-table-test (map-into '((a . 1)) '(hash-table :test eq))) 'eq)
;; no vector/string/bool-vector target exists.
(qa-err (map-into '((a . 1)) 'vector) 'cl-no-applicable-method)
(qa-err (map-into '(97 98) 'string) 'cl-no-applicable-method)

;; == map-merge / map-merge-with ==
;; later maps win; result converted back to TYPE.
(qa (map-merge 'list '((a . 1)) '((b . 2))) '((a . 1) (b . 2)))
(qa (map-merge 'list '((a . 1)) '((a . 9) (b . 2))) '((a . 9) (b . 2)))
(qa (map-merge 'list '((a . 1)) '((b . 2)) '((a . 9))) '((a . 9) (b . 2)))
(qa (map-merge 'plist '(a 1 b 2) '(b 9 c 3)) '(a 1 b 9 c 3))
(qa (map-merge 'list '(a 1) '((a . 9))) '((a . 9)))
(qa (map-merge 'list '((a . 1)) [x y]) '((a . 1) (0 . x) (1 . y)))
(let ((h (map-merge 'hash-table '((a . 1)) '((a . 9) (b . 2)))))
  (qa (map-elt h 'a) 9)
  (qa (map-elt h 'b) 2))
(qa (map-merge-with 'list #'max '((a . 1)) '((a . 9))) '((a . 9)))
;; FUNCTION folds left over successive maps.
(qa (map-merge-with 'list #'cons '((a . 1)) '((a . 2)) '((a . 3)))
    '((a (1 . 2) . 3)))
(qa (map-merge-with 'plist #'+ '(a 1) '(a 10 b 2)) '(a 11 b 2))
(let ((h (map-merge-with 'hash-table #'max '((a . 1)) '((a . 9)))))
  (qa (map-elt h 'a) 9))

;; == map-filter / map-remove / map-some / map-every-p ==
;; filter/remove ALWAYS return an alist (arrays get index keys).
(qa (map-filter (lambda (k v) (> v 1)) '((a . 1) (b . 2) (c . 3)))
    '((b . 2) (c . 3)))
(qa (map-remove (lambda (k v) (> v 1)) '((a . 1) (b . 2) (c . 3)))
    '((a . 1)))
(qa (map-filter (lambda (k v) (eq k 'a)) '(a 1 b 2)) '((a . 1)))
(qa (map-remove (lambda (k v) (eq k 'a)) '(a 1 b 2)) '((b . 2)))
(qa (map-filter (lambda (k v) (> v 0)) [1 -2 3]) '((0 . 1) (2 . 3)))
(let ((h (make-hash-table)))
  (puthash 'a 1 h) (puthash 'b -2 h) (puthash 'c 3 h)
  (qa (sort (map-filter (lambda (k v) (> v 0)) h)
            (lambda (x y) (string< (symbol-name (car x)) (symbol-name (car y)))))
      '((a . 1) (c . 3))))
;; map-some returns the pred's value, not t.
(qa (map-some (lambda (k v) (and (> v 1) (list k v))) '((a . 1) (b . 2)))
    '(b 2))
(qa (map-some (lambda (k v) v) '((a))) nil)
(qa (map-some (lambda (k v) t) '()) nil)
(qa (map-every-p (lambda (k v) (> v 0)) '((a . 1) (b . 2))) t)
(qa (map-every-p (lambda (k v) (> v 0)) '((a . 1) (b . -2))) nil)
(qa (map-every-p (lambda (k v) nil) '()) t)
(qa (map-every-p (lambda (k v) (numberp v)) '(a 1 b 2)) t)
;; pred is called with TWO args; a one-arg pred is an error.
(qa-err (map-filter #'numberp '((a . 1))) 'wrong-number-of-arguments)

;; == map-insert == — non-destructive; returns a new map.
(let ((m (list '(a . 1))))
  (qa (map-insert m 'b 2) '((b . 2) (a . 1)))
  (qa m '((a . 1))))
;; inserting an existing alist key shadows it (new pair in front).
(qa (map-insert '((a . 1)) 'a 9) '((a . 9) (a . 1)))
(qa (map-insert '(a 1) 'b 2) '(b 2 a 1))
(qa (map-insert '(a 1) 'a 9) '(a 9 a 1))
(qa (map-insert nil 'a 1) '((a . 1)))
(let ((v (vector 'a 'b)))
  (qa (map-insert v 1 'z) '[a z])
  (qa v '[a b])
  ;; can't grow an array; index out of range is an error.
  (qa-err-data (map-insert v 2 'z) 'args-out-of-range '([a b] 2)))
(let ((h (make-hash-table)))
  (puthash 'a 1 h)
  (let ((h2 (map-insert h 'b 2)))
    (qa (eq h h2) nil)
    (qa (map-elt h 'b) nil)
    (qa (map-elt h2 'b) 2)))

;; == map-nested-elt ==
(qa (map-nested-elt '((a . ((b . 5)))) '(a b)) 5)
(qa (map-nested-elt '((a . ((b . ((c . 7)))))) '(a b c)) 7)
(qa (map-nested-elt '((a . ((b . 5)))) '(a z) 'dflt) 'dflt)
(qa (map-nested-elt '((a . ((b . 5)))) '(a z)) nil)
(qa (map-nested-elt '((a . [x y])) '(a 1)) 'y)
(qa (map-nested-elt '((a . (b 2 c 3))) '(a c)) 3)
(qa (map-nested-elt '((a . 5)) '(a)) 5)
;; a non-map intermediate yields nil/default rather than an error.
(qa (map-nested-elt '((a . ((b . 1)))) '(a b c) 'd) 'd)
(let ((h (make-hash-table)))
  (puthash 'k '((x . 5)) h)
  (qa (map-nested-elt h '(k x)) 5))
;; no gv setter exists.
(qa-err (setf (map-nested-elt '((a . ((b . 1)))) '(a b)) 9) 'void-function)

;; == map-let ==
(qa (map-let (a b) '((a . 1) (b . 2)) (list a b)) '(1 2))
(qa-err (map-let (a) '((a . 1) (b . 2)) (list a b)) 'void-variable)
(qa (map-let (a) '() a) nil)
;; a keyword K binds the var named (substring K 1).
(qa (map-let (:a) '(:a 9) a) 9)
;; (KEY VAR [DEFAULT]): KEY and DEFAULT are evaluated forms.
(qa (map-let (('b ib)) '((b . 5)) ib) 5)
(qa (map-let (('b ib 99)) '() ib) 99)
(qa (map-let (('b ib (+ 40 2))) '() ib) 42)
(qa (map-let ((0 v)) [x y] v) 'x)
(qa-err (map-let ((b ib)) '((b . 5)) ib) 'void-variable)

;; == let-alist ==
(qa (let-alist '((a . 1) (b . 2)) (list .a .b)) '(1 2))
(qa (let-alist '((a . 1)) .z) nil)
(qa (let-alist '((a . 1) (b . ((c . 2)))) .b.c) 2)
(qa (let-alist '((a . ((b . 2)))) (list .a .a.b)) '(((b . 2)) 2))
;; value of an entry is its cdr, not cadr.
(qa (let-alist '((a x y)) .a) '(x y))
(qa (let-alist '((a b . 1)) .a) '(b . 1))
;; missing key vs nil value are indistinguishable.
(qa (let-alist '((a)) .a.b) nil)
;; .foo.N takes (nth N value); `nth' requires a list value —
;; a vector value signals wrong-type-argument.
(qa (let-alist '((a x y z)) .a.1) 'y)
(qa-err (let-alist '((a . [x y z])) .a.2) 'wrong-type-argument)
;; likewise .foo.bar requires a list value to descend into.
(qa-err (let-alist '((a . [x y])) .a.b) 'wrong-type-argument)
;; dotted symbols in evaluated positions are bound, but a vector
;; literal is self-quoting: [.a] keeps the symbol.
(qa (let-alist '((a . 1)) (vector .a)) '[1])
(qa (let-alist '((a . 1)) [.a]) '[.a])
(qa (let-alist '((a . 1)) (let ((x .a)) x)) 1)
;; `..foo' escapes to the outer variable named `.foo'.
(let ((.x 42))
  (qa (let-alist '((x . 1)) (list .x ..x)) '(1 42)))
;; ALIST is evaluated.
(let ((al '((a . 1))))
  (qa (let-alist al .a) 1))
(qa (let-alist (list (cons 'a 1)) .a) 1)

;; == ring: construction / predicates ==
(let ((r (make-ring 3)))
  (qa (ring-p r) t)
  (qa (ring-size r) 3)
  (qa (ring-length r) 0)
  (qa (ring-empty-p r) t)
  (qa (ring-elements r) nil))
(qa (ring-p '()) nil)
(qa (ring-p '(0 0 . [])) t)
(qa-err-data (make-ring -1) 'wrong-type-argument '(wholenump -1))

;; == ring: insert / ref / elements ==
(let ((r (make-ring 3)))
  (qa (ring-insert r 'a) 'a)
  (ring-insert r 'b)
  (ring-insert r 'c)
  (qa (ring-elements r) '(c b a))     ; newest first
  (qa (ring-length r) 3)
  (ring-insert r 'd)                  ; overflow discards oldest
  (qa (ring-elements r) '(d c b))
  (qa (ring-ref r 0) 'd)
  (qa (ring-ref r 1) 'c)
  (qa (ring-ref r 2) 'b)
  ;; INDEX is taken modulo ring-length — negatives wrap too.
  (qa (ring-ref r 3) 'd)
  (qa (ring-ref r -1) 'b)
  (qa (ring-ref r -3) 'd)
  (qa (ring-ref r -4) 'b))
(let ((r (make-ring 1)))
  (ring-insert r 'a)
  (ring-insert r 'b)
  (qa (ring-elements r) '(b))
  (qa (ring-ref r -1) 'b))
(qa-err (ring-ref (make-ring 3) 0) 'error)
(qa-err (ring-insert (make-ring 0) 'a) 'arith-error)

;; == ring: remove / member / next / previous ==
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  (qa (ring-remove r) 'a)             ; nil index removes the OLDEST
  (qa (ring-elements r) '(c b)))
(let ((r (make-ring 4)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  (qa (ring-remove r 0) 'c)           ; numeric index = ring-ref index
  (qa (ring-elements r) '(b a)))
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  (qa (ring-remove r -1) 'a)          ; -1 = oldest
  (qa (ring-elements r) '(c b)))
(qa-err (ring-remove (make-ring 3)) 'error)
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  (qa (ring-member r 'a) 2)
  (qa (ring-member r 'c) 0)
  (qa (ring-member r 'z) nil))
(let ((r (make-ring 3)))
  (ring-insert r "a")
  (qa (ring-member r "a") 0))         ; comparison is `equal'
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  ;; next/previous take an ITEM, not an index, and wrap around.
  (qa (ring-next r 'c) 'b)
  (qa (ring-next r 'a) 'c)
  (qa (ring-previous r 'c) 'a)
  (qa (ring-previous r 'a) 'b)
  (qa-err (ring-next r 'zz) 'error))

;; == ring: insert+extend / remove+insert+extend / insert-at-beginning ==
(let ((r (make-ring 2)))
  (ring-insert r 'a)
  (qa (ring-insert+extend r 'b nil) 'b)
  ;; GROW-P grows the ring only when it is full.
  (qa (ring-insert+extend r 'c t) 'c)
  (qa (ring-size r) 3)
  (qa (ring-elements r) '(c b a)))
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b)
  (qa (ring-insert+extend r 'c t) 'c) ; non-full: no growth
  (qa (ring-size r) 3))
(let ((r (make-ring 2)))
  (ring-insert r 'a) (ring-insert r 'b)
  ;; removes ITEM first (ensuring uniqueness), then inserts.
  (qa (ring-remove+insert+extend r 'x nil) 'x)
  (qa (ring-elements r) '(x b))
  (qa (ring-remove+insert+extend r 'y t) 'y)
  (qa (ring-elements r) '(y x b))
  (qa (ring-size r) 3))
(let ((r (make-ring 3)))
  (qa (ring-insert-at-beginning r 'z) 1)   ; returns new ring length
  (qa (ring-elements r) '(z)))
(let ((r (make-ring 2)))
  (ring-insert r 'a) (ring-insert r 'b)
  ;; on a full ring the "oldest" slot overwritten is the newest's.
  (qa (ring-insert-at-beginning r 'z) 2)
  (qa (ring-elements r) '(a z)))

;; == ring: plus1/minus1/index/extend/resize/copy/convert ==
(qa (ring-plus1 0 3) 1)
(qa (ring-plus1 2 3) 0)
(qa (ring-minus1 0 3) 2)
(qa (ring-minus1 2 3) 1)
(qa (ring-index 0 0 3 5) 2)
(qa (ring-index 1 0 3 5) 1)
(qa (ring-index 2 1 3 5) 1)
(qa (ring-index 0 4 3 5) 1)
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b)
  (qa (ring-extend r 2) 0)
  (qa (ring-size r) 5)
  (qa (ring-elements r) '(b a))
  (qa (ring-extend r -1) nil))
(let ((r (make-ring 3)))
  (ring-insert r 'a) (ring-insert r 'b)
  (qa (ring-resize r 5) 0)
  (qa (ring-size r) 5)
  (qa (ring-elements r) '(b a)))
(let ((r (make-ring 4)))
  (ring-insert r 'a) (ring-insert r 'b) (ring-insert r 'c)
  (ring-resize r 2)                   ; shrinking drops the OLDEST
  (qa (ring-size r) 2)
  (qa (ring-length r) 2)
  (qa (ring-elements r) '(c b)))
(let* ((r (make-ring 3)) (c (progn (ring-insert r 'a) (ring-copy r))))
  (qa (eq r c) nil)
  (qa (ring-elements c) '(a))
  (qa (ring-size c) 3)
  (ring-insert c 'b)
  (qa (ring-elements r) '(a)))        ; independent copy
(let ((r (ring-convert-sequence-to-ring '(a b c))))
  (qa (ring-elements r) '(a b c))     ; seq order preserved
  (qa (ring-size r) 3))
(let ((r (ring-convert-sequence-to-ring [a b c])))
  (qa (ring-elements r) '(a b c)))
(let ((r (ring-convert-sequence-to-ring "abc")))
  (qa (ring-elements r) '(?a ?b ?c)))
;; elements `equal' to the first member are skipped (size still SEQ len).
(let ((r (ring-convert-sequence-to-ring '(a b a c))))
  (qa (ring-elements r) '(a b c))
  (qa (ring-size r) 4)
  (qa (ring-length r) 3))
(let ((r (make-ring 2)))
  (ring-insert r 'a)
  (qa (eq r (ring-convert-sequence-to-ring r)) t))

(princ (qa--done))
