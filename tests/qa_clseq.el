;; cl-lib sequences/structures — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - butlast/nbutlast (no cl- prefix) not cl-butlast.
;;   - cl-copy-alist / cl-copy-tree / cl-reverse / cl-nreverse are NOT fns;
;;     use copy-alist / copy-tree / reverse / nreverse.
;;   - cl-destructuring-bind handles simple + &optional; nested patterns
;;     ((a b) &rest rest) — covered.
;;   - cl-sublis / cl-acons / cl-pairlis are fns.
;;   - cl-ldiff / cl-tailp work on shared tails.
;;   - cl-union returns order-unpredictable but complete lists.

(require 'cl-lib)

;; == butlast family (no cl- prefix) ==
(qa (fboundp 'cl-butlast) nil)
(qa (fboundp 'butlast) t)
(qa (fboundp 'cl-nbutlast) nil)
(qa (fboundp 'nbutlast) t)
(qa (butlast '(a b c d) 2) '(a b))
(qa (nbutlast (list 'a 'b 'c 'd) 2) '(a b))

;; == cl-subseq / cl-copy-list ==
(qa (cl-subseq '(a b c d) 1 3) '(b c))
(qa (cl-copy-list '(a b c)) '(a b c))

;; == set ops ==
;; cl-union returns member-ordered result; just test membership
(let ((u (cl-union '(a b) '(b c))))
  (qa (sort (copy-sequence u) #'string<) '(a b c)))
(qa (cl-intersection '(a b c) '(b c d)) '(c b))
(qa (cl-set-difference '(a b c) '(b)) '(a c))
(qa (cl-set-exclusive-or '(a b) '(b c)) '(a c))

;; == case forms ==
(qa (cl-case 'x (a 1) (b 2) (t 99)) 99)
(qa (cl-case 'a (a 1) (b 2) (t 99)) 1)
(qa (cl-typecase 5 (integer 'int) (string 'str) (t 'other)) 'int)
(qa (cl-typecase "s" (integer 'int) (string 'str) (t 'other)) 'str)
(qa (cl-etypecase 'x (integer 'i) (symbol 's)) 's)
(qa-err (cl-etypecase 'x (integer 'i)) 'error)

;; == cl-destructuring-bind ==
(qa (cl-destructuring-bind (a b &optional c) '(1 2 3)
      (list a b c))
    '(1 2 3))
(qa (cl-destructuring-bind (a b &optional (c 99)) '(1 2)
      (list a b c))
    '(1 2 99))
;; nested destructuring
(qa (cl-destructuring-bind ((a b) &rest rest) '((1 2) 3 4)
        (list a b rest))
    '(1 2 (3 4)))

;; == cl-block / cl-return-from ==
(qa (cl-block foo (cl-return-from foo 7) 99) 7)
(qa (cl-block foo 1 2 3) 3)
(qa (cl-block nil (cl-return-from nil 'x) 'y) 'x)

;; == cl-sublis / cl-acons / cl-pairlis / copy-tree ==
(qa (cl-sublis '((a . 1) (b . 2)) '(a b c a)) '(1 2 c 1))
(qa (cl-acons 'k 'v '((a . 1))) '((k . v) (a . 1)))
(qa (cl-pairlis '(a b) '(1 2)) '((a . 1) (b . 2)))
;; copy-alist/copy-tree are plain fns (no cl- prefix)
(qa (fboundp 'cl-copy-alist) nil)
(qa (fboundp 'copy-alist) t)
(qa (copy-alist '((a . 1) (b . 2))) '((a . 1) (b . 2)))
(qa (fboundp 'cl-copy-tree) nil)
(qa (fboundp 'copy-tree) t)
(qa (copy-tree '(a (b c) d)) '(a (b c) d))

;; == cl-ldiff / cl-tailp ==
(let ((l '(a b c d)))
  (qa (cl-ldiff l (nthcdr 2 l)) '(a b))
  (qa (cl-tailp (nthcdr 2 l) l) t)
  (qa (cl-tailp '(x) l) nil))

;; == cl-list* / cl-endp ==
(qa (cl-list* 'a 'b 'c) '(a b . c))
(qa (cl-list* 'a 'b 'c nil) '(a b c))
(qa (cl-endp nil) t)
(qa (cl-endp '(x)) nil)

;; == cl-cxr accessors ==
(qa (cl-caaar '(((a b) (c)) ((d)))) 'a)
(qa (cl-first '(a b)) 'a)
(qa (cl-second '(a b)) 'b)
(qa (cl-third '(a b c)) 'c)
(qa (cl-tenth '(1 2 3 4 5 6 7 8 9 10)) 10)
(qa (cl-rest '(a b)) '(b))

;; == cl-getf ==
(qa (cl-getf '(a 1 b 2) 'b) 2)
(qa (cl-getf '(a 1 b 2) 'c) nil)
(qa (cl-getf '(a 1 b 2) 'c 99) 99)

;; == cl-remove / cl-delete / -if / -if-not ==
(qa (cl-remove 2 '(1 2 3 2)) '(1 3))
(qa (cl-delete 2 (list 1 2 3 2)) '(1 3))
(qa (cl-remove-if #'cl-evenp '(1 2 3 4)) '(1 3))
(qa (cl-remove-if-not #'cl-evenp '(1 2 3 4)) '(2 4))
(qa (cl-delete-if #'cl-evenp (list 1 2 3 4)) '(1 3))
(qa (cl-delete-if-not #'cl-evenp (list 1 2 3 4)) '(2 4))

;; == cl-remove-duplicates / cl-delete-duplicates ==
(qa (sort (copy-sequence (cl-remove-duplicates '(1 2 1 3 2))) #'<) '(1 2 3))
(qa (sort (copy-sequence (cl-delete-duplicates (list 1 2 1 3 2))) #'<) '(1 2 3))

;; == cl-position / cl-find / cl-count ==
(qa (cl-position 'b '(a b c)) 1)
(qa (cl-position 'x '(a b c)) nil)
(qa (cl-find 'b '(a b c)) 'b)
(qa (cl-find 'x '(a b c)) nil)
(qa (cl-count 'b '(a b c b)) 2)
;; -if / -if-not variants
(qa (cl-count-if #'cl-evenp '(1 2 3 4 5)) 2)
(qa (cl-count-if-not #'cl-evenp '(1 2 3 4 5)) 3)
(qa (cl-find-if #'cl-evenp '(1 3 4 5)) 4)
(qa (cl-find-if-not #'cl-evenp '(2 4 5)) 5)
(qa (cl-position-if #'cl-evenp '(1 3 4)) 2)
(qa (cl-position-if-not #'cl-evenp '(2 4 5)) 2)

;; == cl-substitute / -if / -if-not ==
(qa (cl-substitute 'x 'a '(a b a)) '(x b x))
(qa (cl-substitute-if 'x #'cl-evenp '(1 2 3 4)) '(1 x 3 x))
(qa (cl-substitute-if-not 'x #'cl-evenp '(1 2 3 4)) '(x 2 x 4))

;; == cl-member / cl-member-if / cl-member-if-not ==
(qa (cl-member 'b '(a b c)) '(b c))
(qa (cl-member-if #'cl-evenp '(1 3 4)) '(4))
(qa (cl-member-if-not #'cl-evenp '(2 4 5)) '(5))

;; == cl-adjoin ==
(qa (cl-adjoin 'a '(b c)) '(a b c))
(qa (cl-adjoin 'b '(b c)) '(b c))

;; == cl-subsetp ==
(qa (cl-subsetp '(a b) '(a b c)) t)
(qa (cl-subsetp '(a z) '(a b c)) nil)

;; == cl-map / cl-mapcar / cl-mapc / cl-some / cl-every / cl-notany / cl-notevery ==
(qa (cl-map 'list #'+ '(1 2 3) '(10 20 30)) '(11 22 33))
(qa (cl-mapcar #'+ '(1 2) '(10 20)) '(11 22))
(let ((acc nil))
  (cl-mapc (lambda (x) (push x acc)) '(1 2 3))
  (qa acc '(3 2 1)))
(qa (cl-some #'cl-evenp '(1 3 4)) t)
(qa (cl-every #'cl-evenp '(2 4)) t)
(qa (cl-notany #'cl-evenp '(1 3)) t)
(qa (cl-notevery #'cl-evenp '(2 5)) t)

;; == cl-sort / cl-stable-sort / cl-merge ==
(qa (cl-sort (list 3 1 2) #'<) '(1 2 3))
(qa (cl-stable-sort (list 3 1 2) #'<) '(1 2 3))
(qa (cl-merge 'list '(1 3) '(2 4) #'<) '(1 2 3 4))

;; == cl-search / cl-mismatch ==
(qa (cl-search '(2 3) '(1 2 3 4)) 1)
(qa (cl-search '(9) '(1 2 3)) nil)
(qa (cl-mismatch '(1 2 3) '(1 2 4)) 2)
(qa (cl-mismatch '(1 2 3) '(1 2 3)) nil)

;; == cl-reverse / cl-nreverse not in cl-lib ==
(qa (fboundp 'cl-reverse) nil)
(qa (fboundp 'cl-nreverse) nil)
(qa (reverse '(a b c)) '(c b a))
(qa (nreverse (list 'a 'b 'c)) '(c b a))
(qa (cl-concatenate 'list '(a) '(b)) '(a b))
(qa (cl-concatenate 'vector [a] [b]) [a b])
(qa (cl-concatenate 'string "a" "b") "ab")

;; == cl-replace / cl-fill ==
(qa (cl-replace (list 'a 'b 'c 'd) '(x y)) '(x y c d))
(qa (let ((v (vector 1 2 3 4))) (cl-fill v 0) v) [0 0 0 0])

;; == cl-reduce ==
(qa (cl-reduce #'+ '(1 2 3 4)) 10)
(qa (cl-reduce #'+ '(1 2 3 4) :initial-value 100) 110)
(qa (cl-reduce #'+ '(1 2 3 4) :from-end t) 10)

(princ (qa--done))
