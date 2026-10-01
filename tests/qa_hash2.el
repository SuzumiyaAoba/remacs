;; Hash tables, second slice — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - hash-table-keys / hash-table-values are NOT fbound until
;;     (require 'subr-x); they come from subr-x.
;;   - remhash returns nil in ALL cases (removed or missing key).
;;   - clrhash returns the SAME table object (eq).
;;   - maphash returns nil; iteration order unspecified.
;;   - :test eql treats 1.0 and 1 as different keys.
;;   - defaults: rehash-size 1.5, rehash-threshold 0.8125,
;;     weakness nil, size 0 (grows), test eql.
;;   - puthash/gethash on non-tables -> wrong-type-argument.

;; == fn surface ==
(qa (fboundp 'hash-table-keys) nil)
(qa (fboundp 'hash-table-values) nil)
(require 'subr-x)
(qa (fboundp 'hash-table-keys) t)
(qa (fboundp 'hash-table-values) t)

;; == defaults ==
(qa (hash-table-rehash-size (make-hash-table)) 1.5)
(qa (hash-table-rehash-threshold (make-hash-table)) 0.8125)
(qa (hash-table-weakness (make-hash-table)) nil)
(qa (hash-table-size (make-hash-table :size 100)) 100)
(qa (hash-table-p "abc") nil)
(qa (hash-table-p [1 2]) nil)

;; == gethash / puthash ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (list (gethash 'a h) (gethash 'b h) (gethash 'c h 'none)))
    '(1 2 none))
(qa (let ((h (make-hash-table)))
      (puthash 'k 'v h))
    'v)
(qa (let ((h (make-hash-table)))
      (puthash 'a nil h)
      (hash-table-count h))
    1)
(qa-err (puthash 'a 1 'not-a-table) 'wrong-type-argument)
(qa-err (gethash 'a nil) 'wrong-type-argument)
(qa-err (gethash 'a 'sym 'dflt 'extra) 'wrong-number-of-arguments)

;; == remhash / clrhash ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (list (remhash 'a h) (remhash 'a h) (hash-table-count h)))
    '(nil nil 0))
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (eq h (clrhash h)))
    t)
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h) (puthash 'b 2 h)
      (clrhash h)
      (list (hash-table-count h) (gethash 'a h 'gone)))
    '(0 gone))

;; == maphash ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (maphash (lambda (k v) k) h))
    nil)
(qa (let ((h (make-hash-table)) (acc nil))
      (puthash 'x 1 h)
      (puthash 'y 2 h)
      (maphash (lambda (k v) (push (cons k v) acc)) h)
      (sort acc (lambda (a b) (string< (symbol-name (car a))
                                     (symbol-name (car b))))))
    '((x . 1) (y . 2)))
;; mutation of existing entries during maphash is legal
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (maphash (lambda (k v) (puthash k (1+ v) h)) h)
      (gethash 'a h))
    2)
(qa (let ((h (make-hash-table)) (sum 0))
      (puthash 'a 10 h) (puthash 'b 20 h) (puthash 'c 30 h)
      (maphash (lambda (_k v) (setq sum (+ sum v))) h)
      sum)
    60)

;; == hash-table-keys / hash-table-values (subr-x) ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "b" 1 h) (puthash "a" 2 h)
      (sort (hash-table-keys h) #'string<))
    '("a" "b"))
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h) (puthash 'b 2 h)
      (sort (hash-table-values h) #'<))
    '(1 2))
(qa (hash-table-keys (make-hash-table)) nil)
(qa (hash-table-values (make-hash-table)) nil)

;; == :test variations ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "key" 1 h)
      (gethash (copy-sequence "key") h))
    1)
(qa (let ((h (make-hash-table :test 'eq)))
      (puthash "key" 1 h)
      (gethash (copy-sequence "key") h 'miss))
    'miss)
(qa (let ((h (make-hash-table :test 'eql)))
      (puthash 1.0 'float h)
      (list (gethash 1.0 h) (gethash 1 h 'int-miss)))
    '(float int-miss))
(qa (let ((h (make-hash-table :test 'eql)))
      (puthash 1 'one h)
      (gethash 1 h))
    'one)
(qa (hash-table-test (make-hash-table)) 'eql)

;; == count / size as table grows ==
(qa (let ((h (make-hash-table)))
      (dotimes (i 50) (puthash i i h))
      (hash-table-count h))
    50)
(qa (let ((h (make-hash-table)))
      (dotimes (i 10) (puthash i i h))
      (remhash 0 h)
      (hash-table-count h))
    9)

;; == copy / eq identity ==
(qa (let ((h (make-hash-table))) (eq h h)) t)
;; equal on hash tables is identity-based in 31.1 (contents ignored)
(qa (equal (let ((h (make-hash-table))) (puthash 'a 1 h) h)
           (let ((h (make-hash-table))) (puthash 'a 1 h) h))
    nil)
(qa (equal (let ((h (make-hash-table))) (puthash 'a 1 h) h)
           (let ((h (make-hash-table))) (puthash 'a 2 h) h))
    nil)

;; == sxhash consistency ==
(qa (= (sxhash-equal "ab") (sxhash-equal (copy-sequence "ab"))) t)
(qa (integerp (sxhash 'a)) t)
(qa (= (sxhash-eq 'a) (sxhash-eq 'a)) t)

;; == purecopy tables ==
(qa (let ((h (make-hash-table :purecopy t)))
      (puthash 'a 1 h)
      (hash-table-count h))
    1)

(princ (qa--done))
