;; map.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - map-elt works on alists, hash-tables, vectors.
;;   - map-keys / map-values / map-pairs round-trip.
;;   - map-put / map-delete mutate the container.

(require 'map)

;; == surface ==
(qa (fboundp 'map-elt) t)
(qa (fboundp 'map-put) t)
(qa (fboundp 'map-delete) t)
(qa (fboundp 'map-keys) t)
(qa (fboundp 'map-values) t)
(qa (fboundp 'map-pairs) t)
(qa (fboundp 'map-length) t)
(qa (fboundp 'map-contains-key) t)
(qa (fboundp 'map-empty-p) t)
(qa (fboundp 'map-apply) t)
(qa (fboundp 'map-do) t)
(qa (fboundp 'map-every-p) t)
(qa (fboundp 'map-some) t)
;; map-find is not provided by GNU 31.1 map.el
(qa (fboundp 'map-find) nil)
(qa (fboundp 'map-copy) t)
(qa (fboundp 'map-filter) t)
(qa (fboundp 'map-remove) t)
(qa (fboundp 'map-into) t)
(qa (fboundp 'map-merge) t)
(qa (fboundp 'map-merge-with) t)
(qa (fboundp 'map-keys-apply) t)
(qa (fboundp 'map-values-apply) t)

;; == map-elt on alist ==
(qa (map-elt '((a . 1) (b . 2)) 'a) 1)
(qa (map-elt '((a . 1) (b . 2)) 'b) 2)
(qa (map-elt '((a . 1) (b . 2)) 'c) nil)
;; with default
(qa (map-elt '((a . 1)) 'z 'DEF) 'DEF)

;; == map-elt on hash-table ==
(qa (let ((h (make-hash-table)))
      (puthash 'x 42 h)
      (list (map-elt h 'x) (map-elt h 'y)))
    '(42 nil))

;; == map-elt on vector ==
(qa (map-elt [10 20 30] 0) 10)
(qa (map-elt [10 20 30] 2) 30)

;; == map-elt on string — indexable ==
(qa (map-elt "abc" 1) ?b)

;; == map-keys / map-values on alist ==
(qa (let ((al '((a . 1) (b . 2))))
      (list (sort (map-keys al) #'string<)
            (sort (map-values al) #'<)))
    '((a b) (1 2)))

;; == map-keys / map-values on hash ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (list (sort (map-keys h) #'string<)
            (sort (map-values h) #'<)))
    '((a b) (1 2)))

;; == map-pairs on hash ==
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      ;; unordered — sort the pairs
      (sort (map-pairs h) (lambda (a b) (string< (car a) (car b)))))
    '((a . 1) (b . 2)))

;; == map-length ==
(qa (map-length '((a . 1) (b . 2))) 2)
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (map-length h))
    1)
(qa (map-length [1 2 3]) 3)

;; == map-contains-key ==
(qa (map-contains-key '((a . 1)) 'a) t)
(qa (map-contains-key '((a . 1)) 'b) nil)
(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (list (map-contains-key h 'a)
            (map-contains-key h 'b)))
    '(t nil))

;; == map-empty-p ==
(qa (map-empty-p nil) t)
(qa (map-empty-p '()) t)
(qa (map-empty-p '((a . 1))) nil)
(qa (let ((h (make-hash-table)))
      (list (map-empty-p h)
            (progn (puthash 'a 1 h)
                   (map-empty-p h))))
    '(t nil))

;; == map-put mutates ==
(qa (let ((al '((a . 1))))
      (map-put al 'b 2)
      (map-elt al 'b))
    2)

(qa (let ((h (make-hash-table)))
      (map-put h 'k 99)
      (map-elt h 'k))
    99)

;; == map-delete removes ==
(qa (let ((al '((a . 1) (b . 2))))
      (setq al (map-delete al 'a))
      (list (map-elt al 'a) (map-elt al 'b)))
    '(nil 2))

(qa (let ((h (make-hash-table)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      (map-delete h 'a)
      (list (map-elt h 'a) (map-elt h 'b)))
    '(nil 2))

;; == map-apply — apply fn to each k/v ==
(qa (let ((al '((a . 1) (b . 2)))
          (acc nil))
      (map-do (lambda (k v) (push (list k v) acc)) al)
      (list (length acc) (car acc)))
    '(2 (b 2)))

;; == map-every-p / map-some ==
(qa (map-every-p (lambda (k v) (numberp v)) '((a . 1) (b . 2))) t)
(qa (map-every-p (lambda (k v) (> v 1)) '((a . 1) (b . 2))) nil)
;; map-some returns the PREDICATE result (t for (> v 1)), not the value
(qa (map-some (lambda (k v) (> v 1)) '((a . 1) (b . 2))) t)
(qa (map-some (lambda (k v) (> v 5)) '((a . 1) (b . 2))) nil)

;; == map-find — not in GNU 31.1 map.el (skipping) ==

;; == map-filter — keeps matching pairs ==
(qa (let ((f (map-filter (lambda (k v) (> v 1)) '((a . 1) (b . 2)))))
      (list (length f) (car (car f))))
    '(1 b))

;; == map-remove — drops matching pairs ==
(qa (let ((r (map-remove (lambda (k v) (> v 1)) '((a . 1) (b . 2)))))
      (list (length r) (car (car r))))
    '(1 a))

;; == map-copy makes a new container ==
(qa (let* ((orig '((a . 1)))
           (c (map-copy orig)))
      ;; a copy — not eq
      (list (eq c orig) (equal c orig)))
    '(nil t))

;; == map-into converts between container types ==
(qa (let ((v (map-into '((a . 1) (b . 2)) 'hash-table)))
      (list (hash-table-p v) (map-elt v 'a)))
    '(t 1))

;; == map-merge combines two maps ==
(qa (let ((m1 '((a . 1)))
          (m2 '((b . 2))))
      (let ((m (map-merge 'list m1 m2)))
        (list (map-elt m 'a) (map-elt m 'b))))
    '(1 2))

;; == map-merge-with — merge fn for conflicts ==
(qa (let ((m1 '((x . 1)))
          (m2 '((x . 10))))
      (let ((m (map-merge-with 'list #'+ m1 m2)))
        (map-elt m 'x)))
    11)

;; == map-keys-apply — apply fn to keys ==
(qa (let ((al '((a . 1) (b . 2))))
      (map-keys-apply #'symbol-name al))
    '("a" "b"))

;; == map-values-apply — apply fn to values ==
(qa (let ((al '((a . 1) (b . 2))))
      (map-values-apply #'1+ al))
    '(2 3))

(princ (qa--done))
