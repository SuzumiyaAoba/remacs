;; Vectors & other arrays — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - vconcat ALWAYS returns a vector, even for string args:
;;     (vconcat "ab" "cd") => [97 98 99 100].
;;   - Strings/bool-vectors are NOT vectorp.
;;   - aref/aset work on strings (char codes) and bool-vectors.
;;   - (append '(1) [2 3]) => (1 . [2 3]): last arg may be any object.
;;   - aset returns the new element, not the vector.
;;   - (make-vector -1 x) => wrong-type-argument.

;; == constructors ==
(qa (vector 1 2 3) [1 2 3])
(qa (vector) [])
(qa (make-vector 3 'x) [x x x])
(qa (make-vector 0 'x) [])
(qa (let ((v (make-vector 2 'x))) (eq (aref v 0) (aref v 1))) t)
(qa (condition-case e (make-vector -1 0) (error (car e)))
    'wrong-type-argument)

;; == predicates ==
(qa (vectorp [1 2]) t)
(qa (vectorp "ab") nil)
(qa (vectorp '(1 2)) nil)
(qa (vectorp (make-bool-vector 3 t)) nil)
(qa (vector-or-char-table-p [1]) t)
(qa (arrayp [1]) t)
(qa (arrayp "ab") t)
(qa (arrayp '(1)) nil)
(qa (sequencep [1]) t)
(qa (sequencep '(1)) t)
(qa (sequencep "ab") t)
(qa (sequencep 5) nil)
(qa (bool-vector-p (make-bool-vector 3 t)) t)
(qa (char-table-p (make-char-table nil)) t)
(qa (fboundp 'recordp) t)

;; == aref / aset ==
(qa (aref [a b c] 0) 'a)
(qa (aref [a b c] 2) 'c)
(qa (aref "abc" 1) 98)
(qa (aref (make-bool-vector 4 t) 0) t)
(qa (let ((v (vector 1 2 3))) (aset v 1 'x) v) [1 x 3])
(qa (let ((v (vector 1 2))) (aset v 0 'x)) 'x)
(qa (let ((s (copy-sequence "abc"))) (aset s 1 ?Z) s) "aZc")
(qa-err (aref [1 2] 5) 'args-out-of-range)
(qa-err (aref [1 2] -1) 'args-out-of-range)
(qa-err (let ((v (vector 1 2))) (aset v 3 'x)) 'args-out-of-range)
(qa-err (aset (vector 1) 0 'x 'extra) 'wrong-number-of-arguments)

;; == length / elt ==
(qa (length [1 2 3]) 3)
(qa (length "abc") 3)
(qa (length (make-bool-vector 4 t)) 4)
(qa (elt [a b c] 2) 'c)
(qa (elt '(a b c) 1) 'b)
(qa (elt "abc" 0) 97)
(qa-err (elt [1] 5) 'args-out-of-range)

;; == vconcat / append / copy-sequence ==
(qa (vconcat [1 2] [3]) [1 2 3])
(qa (vconcat [1 2] '(3 4) "ab") [1 2 3 4 97 98])
(qa (vconcat "ab" "cd") [97 98 99 100])
(qa (vconcat) [])
(qa (vconcat nil [1] nil) [1])
(qa-err (vconcat [1] 5) 'wrong-type-argument)
(qa (append [1 2] '(3)) '(1 2 3))
(qa (append '(1) [2 3]) '(1 . [2 3]))
(qa (copy-sequence [1 2]) [1 2])
(qa (let ((v [1 2])) (eq v (copy-sequence v))) nil)
(qa (reverse [1 2 3]) [3 2 1])
(qa (let ((v (vector 1 2 3))) (nreverse v)) [3 2 1])
(qa (remove 2 [1 2 3]) [1 3])
(qa (append [1 2] [3 4]) '(1 2 . [3 4]))
;; == equal on vectors ==
(qa (equal [1 [2]] [1 [2]]) t)
(qa (eq [1] [1]) nil)
(qa (equal [1 2] '(1 2)) nil)
(qa (eq (vector) (vector)) t)

;; == fill / sort / map ==
(qa (let ((v (vector 1 2 3))) (fillarray v 0) v) [0 0 0])
(qa (let ((v (vector 3 1 2))) (sort v #'<)) [1 2 3])
(qa (let ((v (vector 3 1 2))) (list (sort v #'>) v)) '([3 2 1] [3 2 1]))
(qa (mapcar #'1+ [1 2 3]) '(2 3 4))
(qa (mapconcat #'number-to-string [1 2 3] ",") "1,2,3")
(qa (let ((acc nil))
      (mapc (lambda (x) (push x acc)) [1 2])
      acc)
    '(2 1))

;; == bool vectors & char tables ==
(qa (equal (make-bool-vector 4 t) (make-bool-vector 4 t)) t)
(qa (eq (make-bool-vector 4 t) (make-bool-vector 4 t)) nil)
(qa (length (bool-vector-exclusive-or (make-bool-vector 2 t)
                                      (make-bool-vector 2 nil)))
    2)
(qa (bool-vector-count-population (make-bool-vector 4 t)) 4)
(qa (let ((bv (make-bool-vector 4 nil)))
      (aset bv 1 t)
      (list (aref bv 0) (aref bv 1)))
    '(nil t))
(qa (char-table-p (make-char-table 'syntax-table)) t)
(qa (let ((ct (make-char-table nil)))
      (set-char-table-range ct ?a 'val)
      (list (char-table-range ct ?a) (char-table-range ct ?b)))
    '(val nil))

;; == records ==
(qa (recordp (record 'a 1 2)) t)
(qa (record 'a 1 2) #s(a 1 2))
(qa (let ((r (record 'a 1 2))) (aref r 0)) 'a)
(qa (vectorp (record 'a 1)) nil)

(princ (qa--done))
