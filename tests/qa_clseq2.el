;; cl-lib sequence functions — GNU 31.1 pins (verified): cl-incf/cl-decf/
;; cl-pushnew + the cl- sequence fns with keyword args.
;; NOTE: `qa' evals FORMs dynamically — `let' must be INSIDE each form.
;;
;; GNU truths pinned:
;;   - cl-incf/cl-decf on a place return the new value; work on car/gethash.
;;   - cl-pushnew returns the (possibly) extended list and mutates the
;;     place; :key applies to ITEM and elements.
;;   - cl-remove/-if: :count limits removals; :from-end counts from the
;;     right; :start/:end bound the active window (elements outside kept).
;;   - cl-reduce #'- '(10 1 2) => 7 left-assoc, 11 with :from-end.
;;   - cl-reduce on nil calls FN with no args (=> 0 for +); single elt
;;     returns the elt without calling FN.
;;   - cl-find/cl-position/cl-count accept :key/:test/:from-end/:start/:end.
;;   - cl-every/cl-some take multiple sequences (like cl-mapcar).
;;   - cl-mismatch returns index into SEQ1; :from-end returns the index of
;;     the FIRST differing elt counting from the left... no — returns the
;;     index one past the common suffix (2 for (1 2 3 4)/(1 2 9 4)).
;;   - cl-union prepends list2-only items to list1: (4 1 2 3).
;;   - cl-intersection returns elements in REVERSE list1 order: (3 2).
;;   - cl-set-difference keeps list1 order.
;;   - remacs loads cl-lib/cl-seq/cl-extra at boot, so the cl- fns are
;;     the real GNU definitions here.

(require 'cl-lib)

;; == cl-incf / cl-decf ==
(qa (let ((x 5)) (list (cl-incf x) x)) '(6 6))
(qa (let ((x 5)) (list (cl-incf x 3) x)) '(8 8))
(qa (let ((x 5)) (list (cl-decf x) x)) '(4 4))
(qa (let ((x 10)) (list (cl-decf x 4) x)) '(6 6))
(qa (let ((l (list 1 2))) (list (cl-incf (car l) 10) l)) '(11 (11 2)))
(qa (let ((h (make-hash-table)))
      (puthash 'k 1 h)
      (cl-incf (gethash 'k h) 9)
      (gethash 'k h))
    10)

;; == cl-pushnew ==
(qa (let ((l '(a b))) (list (cl-pushnew 'c l) l)) '((c a b) (c a b)))
(qa (let ((l '(a b))) (cl-pushnew 'a l) l) '(a b))
(qa (let ((l nil)) (cl-pushnew 'x l) (cl-pushnew 'y l) l) '(y x))
(qa (let ((l '(1 2))) (list (cl-pushnew 2 l :key #'identity) l))
    '((1 2) (1 2)))

;; == cl-remove-if / cl-remove / cl-delete family ==
(qa (cl-remove-if #'cl-evenp '(1 2 3 4 5)) '(1 3 5))
(qa (cl-remove-if #'cl-evenp '(1 2 3 4 5) :count 1) '(1 3 4 5))
(qa (cl-remove-if #'cl-evenp '(1 2 3 4) :from-end t :count 1) '(1 2 3))
(qa (cl-remove-if #'cl-evenp [1 2 3 4]) [1 3])
(qa (cl-remove-if #'cl-evenp '(1 2 3 4) :start 1 :end 3) '(1 3 4))
(qa (cl-remove-if-not #'cl-evenp '(1 2 3 4)) '(2 4))
(qa (cl-remove 2 '(1 2 3 2) :count 1) '(1 3 2))
(qa (cl-remove 2 '(1 2 3 2) :count 1 :from-end t) '(1 2 3))
(qa (cl-remove "a" '("a" "b") :test #'equal) '("b"))
(qa (cl-remove 'a '(a b a) :key #'identity) '(b))
(qa (cl-delete 2 (list 1 2 3 2) :count 1) '(1 3 2))
(qa (cl-delete-if #'cl-evenp (list 1 2 3 4) :count 1) '(1 3 4))
(qa (cl-delete-if-not #'cl-evenp (list 1 2 3 4)) '(2 4))

;; == cl-mapcar stops at the shortest ==
(qa (cl-mapcar #'list '(1 2) '(a b c)) '((1 a) (2 b)))
(qa (cl-mapcar #'cons '(1 2 3) '(a b)) '((1 . a) (2 . b)))
(qa (cl-mapcar #'+ nil '(1 2)) nil)

;; == cl-reduce ==
(qa (cl-reduce #'- '(10 1 2)) 7)
(qa (cl-reduce #'- '(10 1 2) :from-end t) 11)
(qa (cl-reduce #'list '(a b c)) '((a b) c))
(qa (cl-reduce #'list '(a b c) :from-end t) '(a (b c)))
(qa (cl-reduce #'+ nil) 0)
(qa (cl-reduce #'+ nil :initial-value 5) 5)
(qa (cl-reduce #'+ '(7)) 7)
(qa (cl-reduce #'+ '(7) :from-end t) 7)
(qa (cl-reduce #'+ '(1 2 3 4) :start 1 :end 3) 5)
(qa (cl-reduce #'max '(3 1 4 1 5)) 5)

;; == cl-find / cl-position / cl-count ==
(qa (cl-find 3 '(1 2 3) :key #'identity) 3)
(qa (cl-find 'b '((1 . a) (2 . b)) :key #'cdr) '(2 . b))
(qa (cl-find-if #'cl-evenp '(1 2 3 4) :from-end t) 4)
(qa (cl-find 'x '(1 2 3)) nil)
(qa (cl-position 2 '(1 2 3 2) :from-end t) 3)
(qa (cl-position-if #'cl-evenp '(1 2 3 4) :from-end t) 3)
(qa (cl-position 'b '(a b c) :test #'eq) 1)
(qa (cl-position "x" '("a" "x") :test #'equal) 1)
(qa (cl-position 9 '(1 2 3)) nil)
(qa (cl-count 1 '(1 2 1 3) :start 1) 1)
(qa (cl-count 'b '(a b c b)) 2)

;; == cl-every / cl-some (multi-seq like mapcar) ==
(qa (cl-every #'cl-evenp '(2 4 6)) t)
(qa (cl-every #'cl-evenp '(2 3 6)) nil)
(qa (cl-every #'< '(1 2) '(2 3)) t)
(qa (cl-some #'cl-evenp '(1 3 5)) nil)
(qa (cl-some #'< '(1 5) '(3 4)) t)
(qa (cl-some #'identity '(nil nil 7)) 7)
(qa (cl-notany #'cl-evenp '(1 3)) t)
(qa (cl-notevery #'cl-evenp '(2 5)) t)

;; == cl-mismatch ==
(qa (cl-mismatch '(1 2 3) '(1 2 4)) 2)
(qa (cl-mismatch '(1 2 3) '(1 2)) 2)
(qa (cl-mismatch '(1 2) '(1 2 3)) 2)
(qa (cl-mismatch "abcd" "abXd") 2)
(qa (cl-mismatch '(a b c) '(a b c)) nil)
(qa (cl-mismatch '(1 2 3 4) '(1 2 9 4) :from-end t) 2)
(qa (cl-mismatch '(a b c) '(a b c) :from-end t) nil)
(qa (cl-mismatch '(1 2) '(1 2 3) :from-end t) 1)
(qa (cl-mismatch [1 2 3] '(1 2 3)) nil)
(qa (cl-mismatch '(1 2 3 4) '(1 9 3 4) :start1 1 :end1 2 :start2 1 :end2 2) 1)

;; == cl-substitute / cl-nsubstitute ==
(qa (cl-substitute 'x 'a '(a b a) :count 1) '(x b a))
(qa (cl-substitute 'x 'a '(a b a) :count 1 :from-end t) '(a b x))
(qa (cl-substitute 0 2 '(1 2 3) :test #'eq) '(1 0 3))
(qa (cl-substitute-if 'x #'cl-evenp '(1 2 3 4)) '(1 x 3 x))
(qa (cl-nsubstitute 'x 'a (list 'a 'b 'a)) '(x b x))

;; == cl-member / cl-member-if ==
(qa (cl-member 2 '(1 2 3)) '(2 3))
(qa (cl-member "a" '("x" "a") :test #'equal) '("a"))
(qa (cl-member 1 '(1 2) :key #'1+) nil)
(qa (cl-member-if #'cl-evenp '(1 2 3)) '(2 3))
(qa (cl-member-if-not #'cl-evenp '(2 4 5)) '(5))

;; == set ops ==
(qa (cl-union '(1 2 3) '(2 3 4)) '(4 1 2 3))
(qa (cl-union '(a b) '(c d) :test #'eq) '(d c a b))
(qa (cl-union '(a b) nil) '(a b))
(qa (cl-intersection '(1 2 3) '(2 3 4)) '(3 2))
(qa (cl-intersection nil '(a)) nil)
(qa (cl-set-difference '(1 2 3 4) '(2 4)) '(1 3))
(qa (cl-set-difference '(1 2 3) '(9)) '(1 2 3))
(qa (cl-set-exclusive-or '(a b) '(b c)) '(a c))
(qa (cl-subsetp '(a b) '(a b c)) t)

;; == cl-search / cl-map / cl-adjoin ==
(qa (cl-search [2 3] [1 2 3 4]) 1)
(qa (cl-search "bc" "abcd") 1)
(qa (cl-search '(9) '(1 2 3)) nil)
(qa (cl-map 'vector #'1+ '(1 2 3)) [2 3 4])
(qa (cl-adjoin 'a '(a b) :test #'eq) '(a b))
(qa (cl-adjoin 'z '(a b)) '(z a b))

(princ (qa--done))
