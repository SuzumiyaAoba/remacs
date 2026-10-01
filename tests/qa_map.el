;; map.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - map-elt/map-put/map-delete/map-contains-key work on alists,
;;     plists and hash-tables.
;;   - map-p / map-flatten-map / map-vals-apply / map-find are NOT
;;     functions in 31.1.
;;   - map-insert on nil creates a fresh alist.
;;   - map-pairs returns a list of (key . val) for alists.
;;   - map-elt with DEFAULT returns DEFAULT for missing keys.
;;   - map-contains-key on a plist returns the tail from the key,
;;     i.e. a LIST, not t.
;;   - map-keys-apply / map-values-apply map over keys / values only.
;;
;; NOTE: `qa' evals each form dynamically — bind locals INSIDE the
;; checked form (a lexical `let' outside `qa' is invisible to eval).

(require 'map)

;; == surface ==
(qa (fboundp 'map-elt) t)
(qa (fboundp 'map-put) t)
(qa (fboundp 'map-delete) t)
(qa (fboundp 'map-contains-key) t)
(qa (fboundp 'map-keys) t)
(qa (fboundp 'map-values) t)
(qa (fboundp 'map-apply) t)
(qa (fboundp 'map-do) t)
(qa (fboundp 'map-into) t)
(qa (fboundp 'map-length) t)
(qa (fboundp 'map-empty-p) t)
(qa (fboundp 'map-copy) t)
(qa (fboundp 'map-filter) t)
(qa (fboundp 'map-remove) t)
(qa (fboundp 'map-merge) t)
(qa (fboundp 'map-merge-with) t)
(qa (fboundp 'map-some) t)
(qa (fboundp 'map-every-p) t)
(qa (fboundp 'map-pairs) t)
(qa (fboundp 'map-insert) t)
(qa (fboundp 'map-keys-apply) t)
(qa (fboundp 'map-values-apply) t)
(qa (fboundp 'nested-alist-p) t)
;; NOT fns
(qa (fboundp 'map-p) nil)
(qa (fboundp 'map-find) nil)
(qa (fboundp 'map-flatten-map) nil)
(qa (fboundp 'map-vals-apply) nil)

;; == on hash-tables ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "a" 1 h)
      (puthash "b" 2 h)
      (list (map-elt h "a")
            (map-length h)
            (map-contains-key h "a")
            (map-contains-key h "c")
            ;; map-keys order is unstable — sort
            (sort (map-keys h) #'string<)
            (sort (map-values h) #'<)))
    '(1 2 t nil ("a" "b") (1 2)))
;; map-put mutates the table
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "a" 1 h)
      (map-put h "c" 3)
      (list (map-elt h "c") (map-length h)))
    '(3 2))
;; map-put with nil VALUE: hash-tables can store nil (key stays,
;; map-elt → nil, still "present"); alists DELETE the pair instead.
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "a" 1 h)
      (map-put h "a" nil)
      (list (map-elt h "a") (map-contains-key h "a") (map-length h)))
    '(nil t 1))
(qa (let ((al (list (cons 'a 1))))
      (map-put al 'a nil))
    nil)
;; on a literal alist form it signals invalid-function
(qa-err (map-put '((a . 1)) 'a nil) 'invalid-function)
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "a" 1 h)
      (puthash "b" 2 h)
      (map-delete h "a")
      (list (map-contains-key h "a") (map-length h)))
    '(nil 1))
;; map-elt DEFAULT on a hash-table
(qa (map-elt (make-hash-table :test 'equal) "nope" 'D) 'D)

;; == on alists ==
(qa (let ((al '((a . 1) (b . 2))))
      (list (map-elt al 'a)
            (map-elt al 'b)
            (map-elt al 'z 'DEFAULT)
            (map-length al)
            (map-contains-key al 'a)
            (map-contains-key al 'z)
            ;; map-keys returns keys in order
            (map-keys al)
            (map-values al)
            (map-pairs al)))
    '(1 2 DEFAULT 2 t nil (a b) (1 2) ((a . 1) (b . 2))))

;; == on plists ==
(qa (let ((pl '(:a 1 :b 2)))
      (list (map-elt pl :a)
            (map-length pl)
            ;; map-contains-key on a plist returns the ENTRY tail, not t
            (map-contains-key pl :a)
            (map-contains-key pl :z)))
    '(1 2 (:a 1 :b 2) nil))

;; == map-apply applies a 2-arg fn to each pair ==
(qa (let ((h (make-hash-table :test 'equal))
          (acc nil))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (map-apply (lambda (k v) (push (cons k v) acc)) h)
      (sort acc (lambda (a b) (string< (symbol-name (car a))
                                       (symbol-name (car b))))))
    '((a . 1) (b . 2)))

;; == map-do on alist ==
(qa (let ((acc nil))
      (map-do (lambda (k v) (push (list k v) acc)) '((x . 1) (y . 2)))
      acc)
    '((y 2) (x 1)))

;; == map-into: coerce between representations ==
(qa (let ((h (map-into '((a . 1) (b . 2)) 'hash-table)))
      (list (hash-table-p h) (map-length h)))
    '(t 2))
(qa (let ((al (map-into '((a . 1) (b . 2)) 'alist)))
      (listp al))
    t)

;; == map-empty-p ==
(qa (map-empty-p '()) t)
(qa (map-empty-p '((a . 1))) nil)
(qa (map-empty-p (make-hash-table)) t)

;; == map-insert ==
(qa (map-insert nil 'a 5) '((a . 5)))
(qa (let ((al (map-insert '((b . 2)) 'a 5)))
      ;; inserts at front
      (list (map-contains-key al 'a) (map-length al)))
    '(t 2))

;; == map-copy ==
(qa (let* ((orig '((a . 1) (b . 2)))
           (copy (map-copy orig)))
      (list (equal copy orig)
            ;; distinct object
            (eq copy orig)))
    '(t nil))

;; == map-filter ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (puthash 'c 3 h)
      (let ((filtered (map-filter (lambda (k v) (> v 1)) h)))
        (list (map-length filtered)
              (map-contains-key filtered 'a))))
    '(2 nil))

;; == map-remove ==
(qa (let* ((al '((a . 1) (b . 2) (c . 3)))
           (rest (map-remove (lambda (k v) (= v 2)) al)))
      (list (map-length rest)
            (map-contains-key rest 'b)))
    '(2 nil))

;; == map-merge ==
(qa (let ((m1 '((a . 1) (b . 2)))
          (m2 '((b . 3) (c . 4))))
      (let ((merged (map-merge 'alist m1 m2)))
        ;; b from m2 wins
        (list (map-length merged) (map-elt merged 'b))))
    '(3 3))

;; == map-every-p / map-some ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (list (map-every-p (lambda (k v) (numberp v)) h)
            (map-every-p (lambda (k v) (= v 1)) h)
            (map-some (lambda (k v) (= v 2)) h)
            (map-some (lambda (k v) (= v 9)) h)))
    '(t nil t nil))

;; nested-alist-p returns t for any list of conses — flat alists qualify
(qa (nested-alist-p '((a . 1) (b . 2))) t)
(qa (nested-alist-p '((a . ((x . 1) (y . 2))))) t)
(qa (nested-alist-p nil) nil)
(qa (nested-alist-p '(a b c)) t)
(qa (nested-alist-p '((a . 1))) t)

;; == map-keys-apply / map-values-apply ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (let ((keys (map-keys-apply #'symbol-name h)))
        (sort keys #'string<)))
    '("a" "b"))
;; map-values-apply applies FN to each VALUE (one arg)
(qa (map-values-apply #'1+ '((a . 1) (b . 2))) '(2 3))
(qa (map-values-apply #'upcase '(:a "x" :b "y")) '("X" "Y"))
(qa (map-keys-apply #'symbol-name '((a . 1) (b . 2))) '("a" "b"))

(princ (qa--done))
