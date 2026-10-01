;; List functions — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - (nth -1 LIST) returns (car LIST) — negative indices clamp to 0.
;;   - (nthcdr -1 LIST) returns LIST.
;;   - (last LIST 0) => nil; (butlast LIST 0) => copy of LIST.
;;   - (proper-list-p '(1 . 2)) => nil; (proper-list-p nil) => 0;
;;     proper lists return the LENGTH, not t.
;;   - (member-if #'oddp '(2 4 5)) is fbound in batch and => (5).
;;   - (list-tail) is NOT fbound in 31.1.
;;   - append's last arg need not be a list: (append '(1) [2 3])
;;     => (1 . [2 3]), but a non-last non-list arg errors.
;;   - delete/remove work on vectors too (returning vectors).

;; == cons / car / cdr family ==
(qa (car '(a b c)) 'a)
(qa (cdr '(a b c)) '(b c))
(qa (car nil) nil)
(qa (cdr nil) nil)
(qa (car-safe "x") nil)
(qa (car-safe '(1 2)) 1)
(qa (cdr-safe "x") nil)
(qa (caar '((a b) c)) 'a)
(qa (cadr '(a b c)) 'b)
(qa (caddr '(a b c)) 'c)
(qa (cdddr '(1 2 3 4)) '(4))
(qa-err (car 5) 'wrong-type-argument)
(qa-err (cdr 5) 'wrong-type-argument)

;; == predicates ==
(qa (consp '(1)) t)
(qa (consp nil) nil)
(qa (listp '(a . b)) t)
(qa (listp nil) t)
(qa (listp "x") nil)
(qa (nlistp '(1)) nil)
(qa (nlistp 5) t)
(qa (atom '(a)) nil)
(qa (atom nil) t)
(qa (null nil) t)
(qa (null '()) t)
(qa (proper-list-p '(1 2)) 2)
(qa (proper-list-p '(1 . 2)) nil)
(qa (proper-list-p 'a) nil)
(qa (proper-list-p nil) 0)
(qa (fboundp 'list-tail) nil)

;; == list / list* ==
(qa (list) nil)
(qa (list 1 2 3) '(1 2 3))
(qa (fboundp 'list*) nil)

;; == nth / nthcdr / last ==
(qa (nth 0 '(a b c)) 'a)
(qa (nth 2 '(a b c)) 'c)
(qa (nth 5 '(a b c)) nil)
(qa (nth -1 '(a b c)) 'a)
(qa (nthcdr 0 '(a b c)) '(a b c))
(qa (nthcdr 2 '(a b c)) '(c))
(qa (nthcdr 5 '(a b)) nil)
(qa (nthcdr -1 '(a b)) '(a b))
(qa (let ((l '(a b c))) (eq (nthcdr 1 l) (cdr l))) t)
(qa (last '(1 2 3)) '(3))
(qa (last '(1 2 3) 2) '(2 3))
(qa (last '(1 2 3) 0) nil)
(qa (last nil) nil)
(qa (fboundp 'list-length) nil)
(qa (safe-length '(a b)) 2)
(qa (length '(a b c)) 3)
(qa (length nil) 0)
(qa (elt '(a b c) 1) 'b)

;; == butlast / nbutlast ==
(qa (butlast '(1 2 3)) '(1 2))
(qa (butlast '(1 2 3) 2) '(1))
(qa (butlast '(1 2 3) 0) '(1 2 3))
(qa (butlast '(1 2) 5) nil)
(qa (let ((l (list 1 2 3))) (list (nbutlast l) l)) '((1 2) (1 2)))

;; == member / memq / memql ==
(qa (member 2 '(1 2 3)) '(2 3))
(qa (member 9 '(1 2 3)) nil)
(qa (member "b" '("a" "b" "c")) '("b" "c"))
(qa (memq 'b '(a b c)) '(b c))
(qa (memq "b" '("a" "b" "c")) nil)
(qa (memql 1.0 '(1.0 2.0)) '(1.0 2.0))
(qa (member-if #'oddp '(2 4 5)) '(5))

;; == delq / remq / delete / remove ==
(qa (delq 'b '(a b c b)) '(a c))
(qa (remq 'b '(a b c b)) '(a c))
(qa (delq nil '(a nil b)) '(a b))
(qa (delete "a" '("a" "b" "a")) '("b"))
(qa (remove "a" '("a" "b" "a")) '("b"))
(qa (remove 2 [1 2 3]) [1 3])
(qa (delete 2 [1 2 3]) [1 3])
;; delete may share structure: removing head returns cdr
(qa (let ((l (list 1 2))) (eq (delete 1 l) (cdr l))) t)

;; == assoc / assq / rassoc ==
(qa (assq 'b '((a . 1) (b . 2))) '(b . 2))
(qa (assoc "b" '(("a" . 1) ("b" . 2))) '("b" . 2))
(qa (assq "b" '(("a" . 1) ("b" . 2))) nil)
(qa (assoc 'x '(a (x . 1))) '(x . 1))
(qa (rassoc 2 '((a . 1) (b . 2))) '(b . 2))
(qa (rassq 'b '((a . x) (b . b))) '(b . b))
(qa (assoc-default 'b '((a . 1) (b 2 3)) nil 'd) '(2 3))
(qa (assoc-default 'z '((a . 1)) nil 'd) nil)
(qa (alist-get 'b '((a . 1) (b . 2))) 2)
(qa (alist-get 'z '((a . 1)) 'dflt) 'dflt)
(qa (fboundp 'assoc-if) nil)

;; == append / reverse / nconc ==
(qa (append '(1 2) '(3 4)) '(1 2 3 4))
(qa (append nil nil) nil)
(qa (append '(1) 2) '(1 . 2))
(qa (append '(1) [2 3]) '(1 . [2 3]))
(qa (append [1 2] '(3)) '(1 2 3))
(qa-err (append 'a '(1)) 'wrong-type-argument)
(qa (reverse '(1 2 3)) '(3 2 1))
(qa (reverse "abc") "cba")
(qa (reverse [1 2 3]) [3 2 1])
(qa (nreverse '(1 2 3)) '(3 2 1))
(qa (let ((a (list 1))) (nconc a '(2)) a) '(1 2))
(qa (nconc '(1) 2) '(1 . 2))

;; == copy-sequence / copy-tree / copy-alist ==
(qa (copy-sequence '(1 2 3)) '(1 2 3))
(qa (let ((l '(1 2))) (eq l (copy-sequence l))) nil)
(qa (let ((l '(1 2))) (eq (cdr l) (cdr (copy-sequence l)))) nil)
(qa (copy-sequence "abc") "abc")
(qa (copy-sequence [1 2]) [1 2])
(qa (copy-tree '((1) (2))) '((1) (2)))
(qa (let ((l '((1) (2)))) (eq (car l) (car (copy-tree l)))) nil)
(qa (let ((l '((1)))) (eq (car l) (car (copy-alist l)))) nil)

;; == setcar / setcdr / push / pop ==
(qa (let ((l (list 1 2))) (setcar l 'x) l) '(x 2))
(qa (let ((l (list 1 2))) (list (setcdr l '(9)) l)) '((9) (1 9)))
(qa (let ((s '(b c))) (list (push 'a s) s)) '((a b c) (a b c)))
(qa (let ((s '(a b c))) (list (pop s) s)) '(a (b c)))

;; == mapping ==
(qa (mapcar #'1+ '(1 2 3)) '(2 3 4))
(qa (mapcar #'1+ [1 2 3]) '(2 3 4))
(qa-err (mapcar #'cons '(a b) '(1 2)) 'wrong-number-of-arguments)
(qa (let ((acc nil)) (mapc (lambda (x) (push x acc)) '(1 2)) acc) '(2 1))
(qa (let ((acc nil)) (eq (mapc (lambda (x) (push x acc)) '(1 2)) '(1 2))) nil)
(qa (mapc #'identity '(1 2)) '(1 2))
(qa (mapconcat #'upcase '("a" "b") "-") "A-B")
(qa (mapcan #'list '(a b c)) '(a b c))
(qa (mapcan (lambda (x) (and (oddp x) (list x))) '(1 2 3)) '(1 3))

;; == equal / eq semantics on lists ==
(qa (equal '(1 (2)) '(1 (2))) t)
(qa (eq '(1) '(1)) nil)
(qa (equal "a" 'a) nil)
(qa-err (nthcdr 'x '(a b)) 'wrong-type-argument)

(princ (qa--done))
