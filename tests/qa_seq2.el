;; seq.el deep coverage — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - seq-nth / seq-take-last / seq-drop-last / seq-notany-p /
;;     seq-delete / seq-sum / seq-zip / seq-interpose are NOT fns
;;     in 31.1 batch.
;;   - seq-first/-rest/-elt/-length/-take/-drop work on lists,
;;     vectors and strings.
;;   - seq-map on a vector returns a LIST.
;;   - seq-reduce calls fn as (FN ACC ELT).
;;   - seq-group-by returns an alist keyed by the fn's result.
;;   - seq-contains is an alias of seq-contains-p and returns the
;;     matching element (not t).
;;   - seq-split groups a sequence into sub-seqs of length N.
;;
;; NOTE: `qa' evals each form dynamically — bind locals INSIDE the
;; checked form (a lexical `let' outside `qa' is invisible to eval).

(require 'seq)

;; == basic access ==
(qa (seq-length '(a b c)) 3)
(qa (seq-length [a b c]) 3)
(qa (seq-length "abc") 3)
(qa (seq-elt '(a b c) 1) 'b)
(qa (seq-elt [a b c] 1) 'b)
(qa (seq-elt "abc" 1) ?b)
(qa (seq-first '(a b c)) 'a)
(qa (seq-first nil) nil)
(qa (seq-rest '(a b c)) '(b c))
(qa (seq-rest '(a)) nil)
;; seq-nth not a fn in 31.1
(qa (fboundp 'seq-nth) nil)
(qa (fboundp 'seq-take-last) nil)
(qa (fboundp 'seq-drop-last) nil)

;; == seq-take / seq-drop ==
(qa (seq-take '(a b c d) 2) '(a b))
(qa (seq-drop '(a b c d) 2) '(c d))
(qa (seq-take '(a b) 5) '(a b))
(qa (seq-drop '(a b) 5) nil)
;; vectors too
(qa (seq-take [a b c d] 2) [a b])
(qa (seq-drop [a b c d] 2) [c d])
;; Character tables have fixed size, so subsequences are vectors.
(qa (let ((table (make-char-table 'generic nil)))
      (list (sequencep table)
            (type-of (seq-take table 2))
            (type-of (seq-drop table (- (seq-length table) 2)))))
    '(t vector vector))

;; == seq-take-while / drop-while ==
(qa (seq-take-while #'evenp '(2 4 5 6)) '(2 4))
(qa (seq-drop-while #'evenp '(2 4 5 6)) '(5 6))

;; == seq-do / seq-doseq ==
(qa (let ((acc nil))
      (seq-do (lambda (x) (push x acc)) '(1 2 3))
      acc)
    '(3 2 1))
(qa (let ((acc nil))
      (seq-doseq (x '(1 2 3))
        (push (* x 2) acc))
      acc)
    '(6 4 2))

;; == seq-map / -mapn / -mapcat / -keep / -map-indexed ==
(qa (seq-map #'1+ '(1 2 3)) '(2 3 4))
;; seq-map on a vector returns a LIST (not vector)
(qa (seq-map #'1+ [1 2 3]) '(2 3 4))
(qa (seq-mapn #'+ '(1 2) '(10 20)) '(11 22))
(qa (seq-mapcat #'list '(1 2)) '(1 2))
(qa (seq-keep #'identity '(nil a nil b)) '(a b))
(qa (seq-map-indexed (lambda (x i) (list i x)) '(a b c))
    '((0 a) (1 b) (2 c)))

;; == seq-reverse ==
(qa (seq-reverse '(a b c)) '(c b a))
(qa (seq-reverse [a b c]) [c b a])
(qa (let* ((table (make-char-table 'generic nil))
           (length (seq-length table))
           (last (1- length)))
      (set-char-table-range table 0 'first)
      (set-char-table-range table last 'last)
      (let ((reversed (seq-reverse table)))
        (list (vectorp reversed)
              (= (length reversed) length)
              (aref reversed 0)
              (aref reversed last))))
    '(t t last first))

;; == seq-contains-p / seq-contains / seq-empty-p / seq-set-equal-p ==
(qa (seq-contains-p '(a b) 'b) t)
(qa (seq-contains-p '(a b) 'z) nil)
;; seq-contains is an alias returning the matched ELEMENT
(qa (seq-contains '(a b) 'a) 'a)
(qa (seq-contains '(a b) 'z) nil)
(qa (seq-empty-p nil) t)
(qa (seq-empty-p '(a)) nil)
(qa (seq-set-equal-p '(a b) '(b a)) t)
(qa (seq-set-equal-p '(a b) '(b c)) nil)

;; == seq-sort / seq-sort-by / seq-uniq ==
(qa (seq-sort #'> '(3 1 2)) '(3 2 1))
(qa (seq-sort-by #'car #'< '((3 . c) (1 . a) (2 . b)))
    '((1 . a) (2 . b) (3 . c)))
(qa (let ((u (seq-uniq '(a a b a c))))
      (sort (copy-sequence u) #'string<))
    '(a b c))

;; == seq-position / seq-positions / seq-find ==
(qa (seq-position '(a b c) 'b) 1)
(qa (seq-position '(a b c) 'z) nil)
(qa (seq-positions '(a b a c a) 'a) '(0 2 4))
(qa (seq-positions '(a b) 'z) nil)
(qa (seq-find #'evenp '(1 3 4)) 4)
(qa (seq-find #'evenp '(1 3)) nil)

;; == seq-count / seq-some / seq-every-p / seq-notany-p ==
(qa (seq-count #'evenp '(1 2 3 4)) 2)
(qa (seq-some #'evenp '(1 3 4)) t)
(qa (seq-some #'evenp '(1 3)) nil)
(qa (seq-every-p #'evenp '(2 4)) t)
(qa (seq-every-p #'evenp '(2 3)) nil)
(qa (fboundp 'seq-notany-p) nil)

;; == seq-remove / seq-filter ==
;; seq-remove takes a PRED; seq-delete is NOT a fn in 31.1
(qa (seq-remove #'evenp '(1 2 3 4)) '(1 3))
(qa (fboundp 'seq-delete) nil)
(qa (seq-filter #'evenp '(1 2 3 4)) '(2 4))

;; == seq-reduce ==
(qa (seq-reduce #'+ '(1 2 3) 0) 6)
;; seq-reduce calls fn as (ACC ELT); cons accumulates as nested conses
(qa (seq-reduce #'cons '(1 2) nil) '((nil . 1) . 2))

;; == seq-group-by / seq-min / seq-max / seq-sum ==
(qa (let ((g (seq-group-by #'evenp '(1 2 3 4))))
      (list (if (assq t g) t nil)
            (sort (copy-sequence (cdr (assq t g))) #'<)
            (sort (copy-sequence (cdr (assq nil g))) #'<)))
    '(t (2 4) (1 3)))
(qa (seq-min '(3 1 4 1)) 1)
(qa (seq-max '(3 1 4 1)) 4)
;; seq-sum is NOT a fn in 31.1
(qa (fboundp 'seq-sum) nil)

;; seq-zip / seq-interpose are NOT fns in 31.1 batch
(qa (fboundp 'seq-zip) nil)
(qa (fboundp 'seq-interpose) nil)

;; == seq-split ==
(qa (seq-split '(a b c d e) 2) '((a b) (c d) (e)))
(qa (seq-split "abcde" 2) '("ab" "cd" "e"))
(qa-err (seq-split '(a b) 0) 'error)

;; == seq-subseq ==
(qa (seq-subseq '(a b c d) 1 3) '(b c))
(qa (seq-subseq [a b c d] 1 3) [b c])

;; == seq-concatenate ==
(qa (seq-concatenate 'list '(a) '(b)) '(a b))
(qa (seq-concatenate 'vector [a] [b]) [a b])
(qa (seq-concatenate 'string "a" "b") "ab")

;; == seq-copy ==
(qa (let ((s (seq-copy '(a b))))
      (list s (not (eq s '(a b)))))
    '((a b) t))

;; == seq-into ==
(qa (seq-into '(a b) 'list) '(a b))
(qa (seq-into [a b] 'list) '(a b))
(qa (seq-into '(a b) 'vector) [a b])

(princ (qa--done))
