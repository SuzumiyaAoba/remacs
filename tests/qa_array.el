;; array semantics — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - `arrayp' returns t for string, vector, char-table, bool-vector.
;;   - `arrayp' returns nil for list, hash-table, record.
;;   - `sequencep' is broader — includes list.
;;   - `vectorp' is narrower — excludes string/char-table/bool-vector.

;; == arrayp ==
(qa (arrayp "abc") t)
(qa (arrayp [1 2]) t)
(qa (arrayp (make-char-table 'general)) t)
(qa (arrayp (bool-vector t nil)) t)
(qa (arrayp '(1 2)) nil)
(qa (arrayp (make-hash-table)) nil)
(qa (arrayp (record 'a)) nil)
(qa (arrayp 42) nil)
(qa (arrayp nil) nil)
(qa (arrayp t) nil)

;; == arrayp vs sequencep ==
(qa (sequencep "abc") t)
(qa (sequencep [1 2]) t)
(qa (sequencep '(1 2)) t)
;; sequencep on hash-table / char-table / bool-vector
(qa (sequencep (make-hash-table)) nil)
;; bool-vector IS a sequence in 31.1
(qa (sequencep (bool-vector t nil)) t)

;; == vectorp vs arrayp ==
(qa (vectorp [1 2]) t)
(qa (vectorp "abc") nil)
(qa (vectorp (make-hash-table)) nil)
(qa (vectorp (make-char-table 'general)) nil)
(qa (vectorp (bool-vector t nil)) nil)

;; == char-table-p / bool-vector-p / hash-table-p are disjoint ==
(qa (char-table-p (make-char-table 'general)) t)
(qa (char-table-p "abc") nil)
(qa (char-table-p [1]) nil)
(qa (bool-vector-p (bool-vector t)) t)
(qa (bool-vector-p [t]) nil)
(qa (bool-vector-p "t") nil)
(qa (hash-table-p (make-hash-table)) t)
(qa (hash-table-p (make-hash-table :test 'eq)) t)
(qa (hash-table-p '()) nil)

;; == sequence-length works on all arrays ==
(qa (length "abc") 3)
(qa (length [1 2 3]) 3)
(qa (length '(1 2 3 4)) 4)
(qa (length (bool-vector t nil t)) 3)

;; == recordp / sequencep interaction ==
(qa (recordp (record 'foo 1 2)) t)
(qa (sequencep (record 'foo 1 2)) nil)
(qa (arrayp (record 'foo 1 2)) nil)

;; == char-table contents via char-table-range ==
(qa (let ((ct (make-char-table 'general 42)))
      (char-table-range ct ?a))
    42)
(qa (let ((ct (make-char-table 'general 42)))
      (aset ct ?a 99)
      (list (char-table-range ct ?a) (char-table-range ct ?z)))
    '(99 42))

;; == bool-vector-count — counts t elements ==
(qa (let ((bv (bool-vector t nil t nil t)))
      (bool-vector-count-population bv))
    3)

;; == bool-vector-exclusive-or / -intersection / -union ==
(qa (let ((a (bool-vector t nil t))
          (b (bool-vector nil t t)))
      (bool-vector-exclusive-or a b))
    (bool-vector t t nil))
(qa (let ((a (bool-vector t nil t))
          (b (bool-vector nil t t)))
      (bool-vector-intersection a b))
    (bool-vector nil nil t))
(qa (let ((a (bool-vector t nil t))
          (b (bool-vector nil t t)))
      (bool-vector-union a b))
    (bool-vector t t t))

;; == bool-vector-not ==
(qa (bool-vector-not (bool-vector t nil t)) (bool-vector nil t nil))

;; == char-table-extra-slot / char-table-parent / char-table-subtype ==
(qa (char-table-subtype (make-char-table 'syntax-table)) 'syntax-table)

;; == aset on string/vector/bool-vector (let INSIDE qa for dynamic eval) ==
(qa (let ((s "abc")) (aset s 0 ?X) s) "Xbc")
(qa (let ((v [1 2 3])) (aset v 1 99) v) [1 99 3])
(qa (let ((bv (bool-vector nil nil))) (aset bv 0 t) bv) (bool-vector t nil))

;; == aref on all ==
(qa (aref "abc" 1) ?b)
(qa (aref [1 2 3] 0) 1)
(qa (aref (bool-vector t nil) 0) t)
(qa (aref (make-char-table 'general 7) ?x) 7)

(princ (qa--done))
