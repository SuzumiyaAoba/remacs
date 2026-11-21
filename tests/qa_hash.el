;; Hash tables — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - define-hash-table-test NAME TEST HASH arg order (test first).
;;   - sxhash-equal stable for equal strings, ignoring text properties;
;;     sxhash-equal-including-properties differs.
;;   - hash-table-weakness 'key / rehash-size / rehash-threshold.
;;   - hash-table-seed is not a var/fn in batch.
;;   - make-finalizer is a fn.
;;   - hash-table-count / hash-table-size return ints.

;; == fn surface ==
(qa (fboundp 'maphash-keys) nil)
(qa (fboundp 'maphash-values) nil)
(qa (fboundp 'sxhash-eql) t)
(qa (fboundp 'sxhash-equal-including-properties) t)
(qa (fboundp 'make-finalizer) t)

;; == sxhash-eq / sxhash-eql / sxhash-equal / sxhash-string ==
(qa (integerp (sxhash-eq 'sym)) t)
(qa (integerp (sxhash-eql 42)) t)
(qa (integerp (sxhash-equal "abc")) t)
;; GNU 31.1 pins deterministic hashes for builtin symbols, fixnums, and
;; content-hashed strings/structures. Dynamic symbol and identity hashes are
;; intentionally excluded because GNU does not promise session stability.
(qa (list (sxhash-eq 't) (sxhash-equal 't)
          (sxhash-eq 'car) (sxhash-equal 'car)
          (sxhash-equal "abc") (sxhash-equal '(1 2))
          (sxhash-equal [1 2]))
    '(15 15 5790 5790 8059383 22 662))
;; deterministic
(qa (= (sxhash-equal "abc") (sxhash-equal "abc")) t)
;; sxhash-equal ignores text properties
(let ((s1 (copy-sequence "abc"))
      (s2 (copy-sequence "abc")))
  (put-text-property 0 1 'x t s2)
  (qa (= (sxhash-equal s1) (sxhash-equal s2)) t))
;; sxhash-equal-including-properties: differs when props differ
(let ((s1 (copy-sequence "abc"))
      (s2 (copy-sequence "abc")))
  (put-text-property 0 1 'x t s2)
  (qa (= (sxhash-equal-including-properties s1)
         (sxhash-equal-including-properties s2))
      nil))

;; == maphash ==
(let ((h (make-hash-table))
      (keys nil)
      (vals nil))
  (puthash 'a 1 h)
  (puthash 'b 2 h)
  (maphash (lambda (k v) (push k keys) (push v vals)) h)
  (qa (sort keys #'string<) '(a b))
  (qa (sort vals #'<) '(1 2)))

;; == define-hash-table-test / custom test ==
(qa (fboundp 'define-hash-table-test) t)
(define-hash-table-test 'qa-str-test #'string= #'sxhash-equal)
(let ((h (make-hash-table :test 'qa-str-test)))
  (puthash "a" 1 h)
  (qa (gethash "a" h) 1)
  ;; a distinct string object with same contents still hits
  (qa (gethash (copy-sequence "a") h) 1))

;; == hash-table-weakness / rehash-size / rehash-threshold ==
(let ((h (make-hash-table :weakness 'key :rehash-size 2.0 :rehash-threshold 0.5)))
  (qa (hash-table-weakness h) 'key)
  (qa (hash-table-rehash-size h) 1.5)
  (qa (hash-table-rehash-threshold h) 0.8125))

;; == hash-table-count / hash-table-size ==
(let ((h (make-hash-table)))
  (qa (hash-table-count h) 0)
  (puthash 'a 1 h)
  (puthash 'b 2 h)
  (qa (hash-table-count h) 2)
  (remhash 'a h)
  (qa (hash-table-count h) 1)
  (clrhash h)
  (qa (hash-table-count h) 0))

;; == hash-table-p / type checks ==
(qa (hash-table-p (make-hash-table)) t)
(qa (hash-table-p "abc") nil)
(qa-err (hash-table-count "abc") 'wrong-type-argument)

;; == hash-table-test returns the test symbol ==
(let ((h (make-hash-table :test 'equal)))
  (qa (hash-table-test h) 'equal))
(let ((h (make-hash-table :test 'eq)))
  (qa (hash-table-test h) 'eq))
(let ((h (make-hash-table :test 'eql)))
  (qa (hash-table-test h) 'eql))
;; custom tests resolve through the test-name → (hash-fn test-fn) alist
(define-hash-table-test 'qa-eip #'equal-including-properties
                        #'sxhash-equal-including-properties)
(let ((h (make-hash-table :test 'qa-eip)))
  (qa (hash-table-test h) 'qa-eip))

;; == puthash returns the value ==
(let ((h (make-hash-table)))
  (qa (puthash 'k 42 h) 42))
;; gethash default arg
(let ((h (make-hash-table)))
  (qa (gethash 'missing h) nil)
  (qa (gethash 'missing h 'dflt) 'dflt))

;; == clrhash returns the same table (emptied) ==
(let ((h (make-hash-table)))
  (puthash 'a 1 h)
  (qa (eq (clrhash h) h) t)
  (qa (hash-table-count h) 0))

;; == remhash returns nil on missing key (no error) ==
(let ((h (make-hash-table)))
  (qa (remhash 'missing h) nil))

;; Built-in hash tests retain their symbols after their names are uninterned.
(qa (let* ((eq-test 'eq)
           (eql-test 'eql)
           (equal-test 'equal)
           (eq-key (list 'key))
           (eq-other (list 'key))
           (equal-key (list 'key))
           (equal-other (list 'key)))
      (unintern eq-test obarray)
      (unintern eql-test obarray)
      (unintern equal-test obarray)
      (let ((eq-table (make-hash-table :test eq-test))
            (eql-table (make-hash-table :test eql-test))
            (equal-table (make-hash-table :test equal-test)))
        (puthash eq-key 'stored eq-table)
        (puthash 5 'stored eql-table)
        (puthash equal-key 'stored equal-table)
        (list (eq (hash-table-test eq-table) eq-test)
              (gethash eq-other eq-table 'missing)
              (eq (hash-table-test eql-table) eql-test)
              (gethash 5 eql-table)
              (eq (hash-table-test equal-table) equal-test)
              (gethash equal-other equal-table))))
    '(t missing t stored t stored))

;; Hash-table weakness tokens also keep their original symbol identities.
(qa (let* ((key-symbol 'key)
           (value-symbol 'value)
           (both-symbol 'key-and-value)
           (either-symbol 'key-or-value))
      (unintern key-symbol obarray)
      (unintern value-symbol obarray)
      (unintern both-symbol obarray)
      (unintern either-symbol obarray)
      (list (hash-table-weakness (make-hash-table :weakness key-symbol))
            (hash-table-weakness (make-hash-table :weakness value-symbol))
            (hash-table-weakness (make-hash-table :weakness both-symbol))
            (hash-table-weakness (make-hash-table :weakness either-symbol))
            (let ((table (make-hash-table :weakness key-symbol)))
              (let ((key (list 'weak-key))) (puthash key t table))
              (garbage-collect)
              (garbage-collect)
              (hash-table-count table))))
    '(key value key-and-value key-or-value 0))

(princ (qa--done))
