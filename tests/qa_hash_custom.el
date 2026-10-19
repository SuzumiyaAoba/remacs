;;; -*- lexical-binding: t; -*-
;; Issue #191: custom hash tables must call and retain both Lisp callbacks.
;; Every expectation is checked against GNU Emacs 31.1, with no xfails.
;; Evaluate checks immediately so callback closures retain lexical bindings.
(defmacro qa191-check (form expected)
  `(let ((actual (condition-case err ,form
                   (error (list 'qa--err (car err) (cdr err)))))
         (wanted ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal actual wanted)
       (push (list ',form wanted actual) qa--fails))))

(defmacro qa191-error (form)
  `(condition-case err (progn ,form 'no-error) (error (car err))))

;; The exact report: distinct case-folded strings denote the same entry.
(define-hash-table-test 'audit-case-fold
  (lambda (a b) (string= (downcase a) (downcase b)))
  (lambda (a) (sxhash-equal (downcase a))))
(qa191-check
    (let ((h (make-hash-table :test 'audit-case-fold)))
      (puthash "HELLO" 42 h)
      (list (gethash "hello" h) (hash-table-test h)))
  '(42 audit-case-fold))

;; CRUD, replacement identity, iteration, copying and metadata.
(let* ((h (make-hash-table :test 'audit-case-fold :size 20
                           :rehash-size 2.0 :rehash-threshold 0.5))
       (original (copy-sequence "Hello"))
       (replacement (copy-sequence "HELLO"))
       (keys nil)
       (pairs nil))
  (qa191-check (gethash "absent" h) nil)
  (qa191-check (gethash "absent" h 'missing) 'missing)
  (qa191-check (puthash original 42 h) 42)
  (qa191-check (puthash replacement 99 h) 99)
  (qa191-check (hash-table-count h) 1)
  (qa191-check (gethash "hElLo" h 'missing) 99)
  (qa191-check (maphash (lambda (key value)
                         (push key keys) (push (cons key value) pairs)) h)
    nil)
  (qa191-check (eq (car keys) original) t)
  (qa191-check (eq (car keys) replacement) nil)
  (qa191-check pairs '(("Hello" . 99)))
  (qa191-check (puthash "NIL" nil h) nil)
  (qa191-check (gethash "nil" h 'missing) nil)
  (qa191-check (hash-table-count h) 2)
  (let ((copy (copy-hash-table h)))
    (qa191-check (eq copy h) nil)
    (qa191-check (hash-table-test copy) 'audit-case-fold)
    (qa191-check (hash-table-size copy) (hash-table-size h))
    (qa191-check (hash-table-weakness copy) nil)
    (qa191-check (hash-table-rehash-size copy) (hash-table-rehash-size h))
    (qa191-check (hash-table-rehash-threshold copy)
      (hash-table-rehash-threshold h))
    (qa191-check (gethash "hello" copy) 99)
    (qa191-check (puthash "HELLO" 7 copy) 7)
    (qa191-check (gethash "hello" h) 99)
    (qa191-check (remhash "HeLLo" copy) nil)
    (qa191-check (gethash "hello" copy 'missing) 'missing)
    (qa191-check (hash-table-count copy) 1)
    (qa191-check (gethash "HELLO" h) 99)
    (qa191-check (eq (clrhash copy) copy) t)
    (qa191-check (hash-table-count copy) 0))
  (qa191-check (remhash "not-there" h) nil)
  (qa191-check (hash-table-count h) 2)
  (qa191-check (remhash "hello" h) nil)
  (qa191-check (hash-table-count h) 1)
  (qa191-check (gethash "HELLO" h 'missing) 'missing)
  (qa191-check (eq (clrhash h) h) t)
  (qa191-check (hash-table-count h) 0))

;; Full hash values gate equality; a comparator-only linear search is wrong.
(let ((comparisons 0) (hashes nil))
  (define-hash-table-test 'qa191-disjoint
    (lambda (_a _b) (setq comparisons (1+ comparisons)) t)
    (lambda (key) (push key hashes) key))
  (let ((h (make-hash-table :test 'qa191-disjoint)))
    (puthash 0 'zero h)
    (puthash 1 'one h)
    (qa191-check (list (gethash 0 h) (gethash 1 h) (hash-table-count h))
      '(zero one 2))
    (qa191-check comparisons 0)
    (qa191-check (nreverse hashes) '(0 1 0 1))))

;; Eq keys still hash, but bypass comparison.  Comparison is query first.
(let ((calls nil))
  (define-hash-table-test 'qa191-sequence
    (lambda (query stored) (push (list 'compare query stored) calls) nil)
    (lambda (key) (push (list 'hash key) calls) 0))
  (let ((h (make-hash-table :test 'qa191-sequence)))
    (puthash 'stored 11 h)
    (qa191-check (gethash 'stored h 'missing) 11)
    (qa191-check (gethash 'query h 'missing) 'missing)
    (qa191-check (nreverse calls)
      '((hash stored) (hash stored) (hash query) (compare query stored)))))

;; Every non-nil comparison result is true, including numeric zero.
(define-hash-table-test 'qa191-truthy
  (lambda (_a _b) 0) (lambda (_key) 0))
(qa191-check
    (let ((h (make-hash-table :test 'qa191-truthy)) (keys nil))
      (puthash 'first 1 h)
      (puthash 'second 2 h)
      (maphash (lambda (key _value) (push key keys)) h)
      (list (hash-table-count h) (gethash 'third h) keys))
  '(1 2 (first)))

;; Deliberate collisions, growth and deletion preserve distinct entries.
(define-hash-table-test 'qa191-collisions
  (lambda (a b) (= (car a) (car b))) (lambda (_key) 0))
(let ((h (make-hash-table :test 'qa191-collisions :size 1))
      (sum 0))
  (dotimes (i 80) (puthash (list i) (* i i) h))
  (qa191-check (hash-table-count h) 80)
  (qa191-check
      (let ((correct t))
        (dotimes (i 80)
          (unless (equal (gethash (list i) h 'missing) (* i i))
            (setq correct nil)))
        correct)
    t)
  (dotimes (i 40) (remhash (list (* i 2)) h))
  (qa191-check (hash-table-count h) 40)
  (qa191-check (gethash '(20) h 'missing) 'missing)
  (qa191-check (gethash '(21) h) 441)
  (maphash (lambda (key _value) (setq sum (+ sum (car key)))) h)
  (qa191-check sum 1600))

;; Collision order follows reusable entry slots, then slot order on resize.
;; The two-slot case reproduces the independent review's adversarial check.
(let ((calls nil))
  (define-hash-table-test 'qa191-slot-order
    (lambda (a b) (push (list a b) calls) (eq a b)) (lambda (_key) 0))
  (let ((h (make-hash-table :test 'qa191-slot-order :size 2)))
    (puthash 'a 1 h)
    (puthash 'b 2 h)
    (remhash 'a h)
    (puthash 'c 3 h)
    (setq calls nil)
    (gethash 'missing h)
    (qa191-check (nreverse calls) '((missing c) (missing b)))
    (puthash 'd 4 h)
    (setq calls nil)
    (gethash 'missing h)
    (qa191-check (nreverse calls) '((missing d) (missing b) (missing c)))))

;; Multiple removals reuse freed slots in LIFO order.  Growth uses stored
;; hashes, copying retains lookup order, and clearing retains capacity.
(let ((calls nil) (hash-calls 0))
  (define-hash-table-test 'qa191-lifo-order
    (lambda (a b) (push (list a b) calls) (eq a b))
    (lambda (_key) (setq hash-calls (1+ hash-calls)) 0))
  (let ((h (make-hash-table :test 'qa191-lifo-order :size 4)))
    (dolist (key '(a b c d)) (puthash key key h))
    (qa191-check (hash-table-size h) 4)
    (remhash 'b h)
    (remhash 'd h)
    (puthash 'e 'e h)
    (puthash 'f 'f h)
    (setq calls nil)
    (gethash 'missing h)
    (qa191-check (nreverse calls)
      '((missing f) (missing e) (missing c) (missing a)))
    (setq hash-calls 0)
    (puthash 'g 'g h)
    (qa191-check hash-calls 1)
    (qa191-check (hash-table-size h) 24)
    (setq calls nil)
    (gethash 'missing h)
    (qa191-check (nreverse calls)
      '((missing g) (missing e) (missing c) (missing f) (missing a)))
    (let ((copy (copy-hash-table h)))
      (setq calls nil)
      (gethash 'missing copy)
      (qa191-check (nreverse calls)
        '((missing g) (missing e) (missing c) (missing f) (missing a))))
    (clrhash h)
    (qa191-check (list (hash-table-size h) (hash-table-count h)) '(24 0))
    (dolist (key '(x y z)) (puthash key key h))
    (setq calls nil)
    (gethash 'missing h)
    (qa191-check (nreverse calls) '((missing z) (missing y) (missing x)))
    (qa191-check (hash-table-size h) 24)))

;; Hash functions may return any Lisp object.  GNU hashes non-fixnums with
;; sxhash; accepting only nonnegative machine integers would be too strict.
(dolist (result (list -1 most-negative-fixnum 0 most-positive-fixnum
                      (1+ most-positive-fixnum) (- most-negative-fixnum 1)
                      3.25 nil t "abc" '(a b) [a b]))
  (qa191-check
      (progn
        (define-hash-table-test 'qa191-hash-object #'equal
          (lambda (_key) result))
        (let ((h (make-hash-table :test 'qa191-hash-object)))
          (puthash (list 'key) 'value h)
          (list (gethash (list 'key) h 'missing) (hash-table-count h))))
    '(value 1)))

;; Folding to GNU's 32-bit lookup hash matters, including negative fixnums
;; and the sxhash-derived values of bignums and floats.
(dolist (pair (list '(0 4294967297) '(-1 3221225472)
                    (list (1+ most-positive-fixnum) 536870912)
                    '(1.0 1072693248)))
  (qa191-check
      (let ((comparisons 0))
        (define-hash-table-test 'qa191-folded
          (lambda (_a _b) (setq comparisons (1+ comparisons)) t)
          (lambda (key) (if (eq key 'first) (car pair) (cadr pair))))
        (let ((h (make-hash-table :test 'qa191-folded)))
          (puthash 'first 'value h)
          (list (gethash 'second h 'missing) comparisons)))
    '(value 1)))

;; Equal freshly allocated hash-return objects must produce the same hash.
(define-hash-table-test 'qa191-object-value
  (lambda (a b) (string= (downcase a) (downcase b)))
  (lambda (key) (list (downcase key))))
(qa191-check
    (let ((h (make-hash-table :test 'qa191-object-value)))
      (puthash "HASH-OBJECT" 'value h)
      (gethash "hash-object" h 'missing))
  'value)

;; Read lookups hash even an empty table, but copying and clearing do not.
(let ((calls nil))
  (define-hash-table-test 'qa191-empty-calls #'equal
    (lambda (key) (push key calls) 0))
  (let ((h (make-hash-table :test 'qa191-empty-calls)))
    (gethash 'get h)
    (remhash 'remove h)
    (clrhash h)
    (copy-hash-table h)
    (qa191-check (nreverse calls) '(get remove))))
(let* ((h (make-hash-table :test 'audit-case-fold :weakness 'key))
       (copy (copy-hash-table h)))
  (qa191-check (list (hash-table-test copy) (hash-table-weakness copy))
    '(audit-case-fold key)))

;; Creation snapshots the callbacks, and copying preserves that snapshot.
;; Removing the symbol property must not release captured callback state.
(qa191-check
    (let ((h nil) (copy nil) (new nil))
      (let ((prefix "captured-"))
        (define-hash-table-test 'qa191-retained
          (lambda (a b) (equal (concat prefix a) (concat prefix b)))
          (lambda (key) (sxhash-equal (concat prefix key))))
        (setq h (make-hash-table :test 'qa191-retained))
        (puthash (copy-sequence "key") 'old h)
        (setq copy (copy-hash-table h)))
      (define-hash-table-test 'qa191-retained
        (lambda (_a _b) nil) (lambda (_key) 0))
      (setq new (make-hash-table :test 'qa191-retained))
      (puthash (copy-sequence "key") 'new new)
      (put 'qa191-retained 'hash-table-test nil)
      (garbage-collect)
      (list (gethash (copy-sequence "key") h 'missing)
            (gethash (copy-sequence "key") copy 'missing)
            (gethash (copy-sequence "key") new 'missing)
            (hash-table-test copy)))
  '(old old missing qa191-retained))

;; Callback symbols are retained as symbols, so their definitions stay live.
(fset 'qa191-symbol-compare #'equal)
(fset 'qa191-symbol-hash (lambda (_key) 0))
(define-hash-table-test 'qa191-symbols 'qa191-symbol-compare 'qa191-symbol-hash)
(let ((h (make-hash-table :test 'qa191-symbols)))
  (puthash (list 'key) 1 h)
  (fset 'qa191-symbol-compare (lambda (_a _b) nil))
  (qa191-check (gethash (list 'key) h 'missing) 'missing)
  (fset 'qa191-symbol-hash (lambda (_key) (error "changed hash")))
  (qa191-check (condition-case err (gethash 'key h) (error err))
    '(error "changed hash")))

;; Byte-compiled closures keep captured state and execute as callbacks.
(let* ((state (list 0 0))
       (compare (byte-compile
                 (lambda (a b)
                   (setcar state (1+ (car state)))
                   (string= (downcase a) (downcase b)))))
       (hash (byte-compile
              (lambda (key)
                (setcar (cdr state) (1+ (cadr state)))
                (sxhash-equal (downcase key))))))
  (qa191-check (byte-code-function-p compare) t)
  (qa191-check (byte-code-function-p hash) t)
  (define-hash-table-test 'qa191-bytecode compare hash)
  (let ((h (make-hash-table :test 'qa191-bytecode)))
    (puthash "BYTECODE" 23 h)
    (put 'qa191-bytecode 'hash-table-test nil)
    (garbage-collect)
    (qa191-check (gethash "bytecode" h 'missing) 23)
    (qa191-check state '(1 2))))

;; Registration is lazy about function validity, but validates the name.
(qa191-check (define-hash-table-test 'qa191-invalid 1 2) '(1 2))
(qa191-check (hash-table-test (make-hash-table :test 'qa191-invalid))
  'qa191-invalid)
(qa191-check (condition-case err
                 (gethash 'key (make-hash-table :test 'qa191-invalid))
               (error err))
  '(invalid-function 2))
(qa191-check (qa191-error (define-hash-table-test 1 #'equal #'identity))
  'wrong-type-argument)
(qa191-check (condition-case err
                 (make-hash-table :test 'qa191-undefined) (error err))
  '(error "Invalid hash table test" qa191-undefined))
(qa191-check (qa191-error (make-hash-table :test '(equal)))
  'wrong-type-argument)
(define-hash-table-test 'qa191-invalid-compare 1 (lambda (_key) 0))
(qa191-check
    (let ((h (make-hash-table :test 'qa191-invalid-compare)))
      (puthash 'original 1 h)
      (condition-case err (gethash 'different h) (error err)))
  '(invalid-function 1))
(define-hash-table-test 'qa191-invalid-hash-arity #'equal (lambda () 0))
(qa191-check
    (qa191-error (gethash 'key (make-hash-table :test 'qa191-invalid-hash-arity)))
  'wrong-number-of-arguments)
(define-hash-table-test 'qa191-invalid-compare-arity
  (lambda (_key) t) (lambda (_key) 0))
(qa191-check
    (let ((h (make-hash-table :test 'qa191-invalid-compare-arity)))
      (puthash 'original 1 h)
      (qa191-error (gethash 'different h)))
  'wrong-number-of-arguments)

;; The callback property needs two cells; a dotted tail after them is legal.
(dolist (property '(nil 7 (equal) (equal . identity)))
  (qa191-check
      (progn
        (put 'qa191-malformed 'hash-table-test property)
        (condition-case err (make-hash-table :test 'qa191-malformed)
          (error err)))
    '(error "Invalid hash table test" qa191-malformed)))
(put 'qa191-dotted 'hash-table-test '(equal identity . ignored))
(qa191-check
    (let ((h (make-hash-table :test 'qa191-dotted)))
      (puthash 7 'value h)
      (list (hash-table-test h) (gethash 7 h)))
  '(qa191-dotted value))

;; Built-in tests ignore callbacks registered under their reserved names.
(dolist (test '(eq eql equal))
  (let ((saved (get test 'hash-table-test)) (calls nil))
    (unwind-protect
        (progn
          (define-hash-table-test test
            (lambda (_a _b) (push 'compare calls) (not (eq test 'equal)))
            (lambda (_key) (push 'hash calls) 0))
          (qa191-check
              (let ((h (make-hash-table :test test)))
                (puthash (list 'key) 'value h)
                (list (gethash (list 'key) h 'missing) calls
                      (hash-table-test h)))
            (list (if (eq test 'equal) 'value 'missing) nil test)))
      (put test 'hash-table-test saved))))

;; Both callbacks protect their own table during each lookup operation.
;; Even a no-op deletion is prohibited; errors must unwind the protection.
(dolist (callback '(hash compare))
  (dolist (operation '(gethash puthash remhash))
    (dolist (mutation '(puthash remhash clrhash))
      (qa191-check
          (let ((h nil) (armed nil) (calls nil))
            (let ((invoke
                   (lambda ()
                     (push mutation calls)
                     (cond ((eq mutation 'puthash) (puthash 'other 2 h))
                           ((eq mutation 'remhash) (remhash 'absent h))
                           ((eq mutation 'clrhash) (clrhash h))))))
              (define-hash-table-test 'qa191-mutation
                (lambda (a b)
                  (when (and armed (eq callback 'compare)) (funcall invoke))
                  (equal a b))
                (lambda (_key)
                  (when (and armed (eq callback 'hash)) (funcall invoke)) 0)))
            (setq h (make-hash-table :test 'qa191-mutation))
            (puthash (list 'key) 1 h)
            (setq armed t)
            (let ((err
                   (condition-case err
                       (cond ((eq operation 'gethash) (gethash (list 'key) h))
                             ((eq operation 'puthash) (puthash (list 'key) 9 h))
                             ((eq operation 'remhash) (remhash (list 'key) h)))
                     (error (list (car err) (cadr err) (eq h (caddr err)))))))
              (setq armed nil)
              (list err calls (hash-table-count h) (gethash (list 'key) h)
                    (puthash 'after 3 h) (hash-table-count h))))
        (list '(error "hash table test modifies table" t)
              (list mutation) 1 1 3 2)))))

;; Mutation protection applies to empty tables too.
(qa191-check
    (let ((h nil))
      (define-hash-table-test 'qa191-empty-mutation #'equal
        (lambda (_key) (clrhash h) 0))
      (setq h (make-hash-table :test 'qa191-empty-mutation))
      (condition-case err (gethash 'missing h)
        (error (list (car err) (cadr err) (eq h (caddr err))))))
  '(error "hash table test modifies table" t))

;; Nonlocal exits from either callback leave later writes usable and atomic.
(dolist (callback '(hash compare))
  (dolist (exit-kind '(error throw))
    (qa191-check
        (let ((h nil) (armed nil))
          (let ((leave (lambda () (if (eq exit-kind 'throw)
                                     (throw 'qa191-stop 'thrown)
                                   (error "callback stopped")))))
            (define-hash-table-test 'qa191-nonlocal
              (lambda (a b)
                (when (and armed (eq callback 'compare)) (funcall leave))
                (equal a b))
              (lambda (_key)
                (when (and armed (eq callback 'hash)) (funcall leave)) 0)))
          (setq h (make-hash-table :test 'qa191-nonlocal))
          (puthash (list 'key) 1 h)
          (setq armed t)
          (let ((result (catch 'qa191-stop
                          (condition-case err (puthash (list 'key) 9 h)
                            (error (cadr err))))))
            (setq armed nil)
            (list result (gethash (list 'key) h) (puthash 'after 3 h)
                  (hash-table-count h))))
      (list (if (eq exit-kind 'throw) 'thrown "callback stopped") 1 3 2))))

;; Recursive reads are valid; returning from one keeps the outer guard.
(qa191-check
    (let ((h nil) (armed nil) (busy nil) (nested nil))
      (define-hash-table-test 'qa191-reentrant #'equal
        (lambda (_key)
          (when (and armed (not busy))
            (unwind-protect
                (progn (setq busy t) (setq nested (gethash 'known h)))
              (setq busy nil)))
          0))
      (setq h (make-hash-table :test 'qa191-reentrant))
      (puthash 'known 42 h)
      (setq armed t)
      (list (gethash 'missing h 'default) nested))
  '(default 42))
(qa191-check
    (let ((h nil) (armed nil) (busy nil))
      (define-hash-table-test 'qa191-nested-guard #'equal
        (lambda (_key)
          (when (and armed (not busy))
            (unwind-protect (progn (setq busy t) (gethash 'known h))
              (setq busy nil))
            (clrhash h))
          0))
      (setq h (make-hash-table :test 'qa191-nested-guard))
      (puthash 'known 42 h)
      (setq armed t)
      (let ((err (condition-case err (gethash 'known h)
                   (error (list (car err) (cadr err))))))
        (setq armed nil)
        (list err (puthash 'after 7 h) (hash-table-count h))))
  '((error "hash table test modifies table") 7 2))

;; Callback-local error handling allows the original write to finish.
(qa191-check
    (let ((h nil) (armed nil) (message nil))
      (define-hash-table-test 'qa191-caught-mutation #'equal
        (lambda (_key)
          (when armed
            (setq message (condition-case err (clrhash h) (error (cadr err)))))
          0))
      (setq h (make-hash-table :test 'qa191-caught-mutation))
      (puthash 'known 42 h)
      (setq armed t)
      (list (puthash 'other 99 h) message (hash-table-count h)))
  '(99 "hash table test modifies table" 2))

;; Internal callers must also honor the guard: print--preprocess can write
;; to print-number-table, which may be the custom table currently in use.
(dolist (callback '(hash compare))
  (qa191-check
      (let ((h nil) (armed nil))
        (let ((preprocess
               (lambda ()
                 (setq armed nil)
                 (let ((print-circle t) (print-number-table h))
                   (print--preprocess (list 1 2))))))
          (define-hash-table-test 'qa191-print-guard
            (lambda (a b)
              (when (and armed (eq callback 'compare)) (funcall preprocess))
              (eq a b))
            (lambda (_key)
              (when (and armed (eq callback 'hash)) (funcall preprocess)) 0)))
        (setq h (make-hash-table :test 'qa191-print-guard))
        (puthash 'a 1 h)
        (setq armed t)
        (let ((err (condition-case err (gethash 'query h)
                     (error (list (car err) (cadr err) (eq h (caddr err)))))))
          (setq armed nil)
          (list err (hash-table-count h) (gethash 'a h)
                (puthash 'after 2 h) (hash-table-count h))))
    '((error "hash table test modifies table" t) 1 1 2 2)))

;; Ordinary preprocessing must use custom lookups and writes as well.
(qa191-check
    (let ((hashes 0) (comparisons 0))
      (define-hash-table-test 'qa191-print-ordinary
        (lambda (a b) (setq comparisons (1+ comparisons)) (eq a b))
        (lambda (_key) (setq hashes (1+ hashes)) 0))
      (let* ((h (make-hash-table :test 'qa191-print-ordinary))
             (print-circle t) (print-number-table h)
             (result (print--preprocess (list 1 2))))
        (list result hashes comparisons (hash-table-count h))))
  '(nil 4 2 2))

;; Distinct tables, including a fresh copy, remain writable in callbacks.
(qa191-check
    (let ((h nil) (other (make-hash-table)) (copy nil) (armed nil))
      (define-hash-table-test 'qa191-other-table #'equal
        (lambda (_key)
          (when armed
            (puthash 'key 7 other)
            (setq copy (copy-hash-table h))
            (clrhash copy))
          0))
      (setq h (make-hash-table :test 'qa191-other-table))
      (puthash 'known 42 h)
      (setq armed t)
      (list (gethash 'known h) (gethash 'key other)
            (hash-table-count h) (hash-table-count copy)))
  '(42 7 1 0))

;; Copying/printing do not run callbacks.  Reading does, including merging
;; equal keys and resolving the registered test for the reconstructed table.
(let ((calls nil))
  (define-hash-table-test 'qa191-readable
    (lambda (a b) (push 'compare calls) (equal a b))
    (lambda (_key) (push 'hash calls) 0))
  (let ((h (make-hash-table :test 'qa191-readable)))
    (puthash (list 'key) 42 h)
    (setq calls nil)
    (qa191-check (hash-table-count (copy-hash-table h)) 1)
    (qa191-check calls nil)
    (let ((printed (prin1-to-string h)))
      (qa191-check printed "#s(hash-table test qa191-readable data ((key) 42))")
      (qa191-check calls nil)
      (let ((read-back (read printed)))
        (qa191-check (hash-table-test read-back) 'qa191-readable)
        (qa191-check (gethash (list 'key) read-back 'missing) 42)
        (qa191-check (nreverse calls) '(hash hash compare)))))
  (setq calls nil)
  (let ((h (read "#s(hash-table test qa191-readable data ((key) 1 (key) 2))")))
    (qa191-check (hash-table-count h) 1)
    (qa191-check (nreverse calls) '(hash hash compare))
    (qa191-check (gethash (list 'key) h 'missing) 2)))
(qa191-check
    (condition-case err
        (read "#s(hash-table test qa191-undefined data (a b))") (error err))
  '(error "Invalid hash table test" qa191-undefined))
(qa191-check
    (condition-case err
        (read "#s(hash-table test qa191-invalid data (a b))") (error err))
  '(invalid-function 2))

;; Literal data determines initial capacity before equivalent keys coalesce.
;; Legacy size and unknown fields are ignored; the first data field wins.
(define-hash-table-test 'qa191-reader-fields #'equal (lambda (_key) 0))
(qa191-check
    (let ((h (read "#s(hash-table test qa191-reader-fields data (\"x\" 1 \"x\" 2 y 3))")))
      (list (hash-table-size h) (hash-table-count h)
            (gethash "x" h) (gethash 'y h)))
  '(3 2 2 3))
(qa191-check
    (let ((h (read "#s(hash-table test qa191-reader-fields size 999 data (a 1 b 2))")))
      (list (hash-table-size h) (hash-table-count h)
            (gethash 'a h) (gethash 'b h)))
  '(2 2 1 2))
(qa191-check
    (let ((h (read "#s(hash-table test qa191-reader-fields data (a 1) data (b 2))")))
      (list (hash-table-size h) (hash-table-count h)
            (gethash 'a h) (gethash 'b h 'missing)))
  '(1 1 1 missing))
(qa191-check
    (let ((h (read "#s(hash-table test qa191-reader-fields unknown 1 data (a 1))")))
      (list (hash-table-size h) (hash-table-count h) (gethash 'a h)))
  '(1 1 1))
(qa191-check
    (let ((h (read "#s(hash-table test qa191-reader-fields size 999 data ())")))
      (list (hash-table-size h) (hash-table-count h)))
  '(0 0))

;; Malformed literal data is rejected before invoking any hash callback.
(let ((calls nil))
  (define-hash-table-test 'qa191-reader-errors #'equal
    (lambda (_key) (push 'hash calls) 0))
  (dolist (case '(("#s(hash-table test qa191-reader-errors data (a 1 b))"
                   (error "Hash table data length is odd"))
                  ("#s(hash-table test qa191-reader-errors data 5)"
                   (error "Hash table data is not a list"))
                  ("#s(hash-table test qa191-reader-errors data [a 1])"
                   (error "Hash table data is not a list"))
                  ("#s(hash-table test qa191-reader-errors data (a . 1))"
                   (wrong-type-argument listp 1))
                  ("#s(hash-table test qa191-reader-errors data (a 1 . b))"
                   (wrong-type-argument listp b))
                  ("#s(hash-table test qa191-reader-errors weakness 17 data ())"
                   (error "Invalid hash table weakness" 17))))
    (setq calls nil)
    (qa191-check
        (list (condition-case err (read (car case)) (error err)) calls)
      (list (cadr case) nil))))

(princ (concat (qa--done) "\n"))
