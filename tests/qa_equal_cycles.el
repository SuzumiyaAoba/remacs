;;; -*- lexical-binding: t; -*-
;; Issue #181: separately allocated cycles must terminate under `equal'.
;; These expected results are also checked against GNU Emacs 31.1.
(defun qa181-cycle (values &optional prefix)
  (let ((xs (copy-sequence values)))
    (setcdr (last xs) (nthcdr (or prefix 0) xs))
    xs))
(defun qa181-both (a b)
  (list (equal a b) (equal b a)))

;; Different finite graph shapes may describe the same infinite list.
(qa (qa181-both (qa181-cycle '(1)) (qa181-cycle '(1))) '(t t))
(qa (qa181-both (qa181-cycle '(1)) (qa181-cycle '(1 1))) '(t t))
(qa (qa181-both (qa181-cycle '(1 2)) (qa181-cycle '(1 2 1 2))) '(t t))
(qa (qa181-both (qa181-cycle '(1 1 1) 2) (qa181-cycle '(1))) '(t t))
(qa (qa181-both (qa181-cycle '(0 1 2) 1)
               (qa181-cycle '(0 1 2 1 2) 3)) '(t t))
(qa (qa181-both (qa181-cycle '(1)) (qa181-cycle '(2))) '(nil nil))
(qa (qa181-both (qa181-cycle '(1)) (qa181-cycle '(1 2))) '(nil nil))
(qa (qa181-both (qa181-cycle '(1 2)) (qa181-cycle '(1 2 1))) '(nil nil))
(qa (qa181-both (qa181-cycle '(0 1 2) 1)
               (qa181-cycle '(0 1 3) 1)) '(nil nil))
(qa (qa181-both (qa181-cycle '(0 1 2) 1)
               (qa181-cycle '(0 1 2) 2)) '(nil nil))

;; Detecting only the first cycle must not skip the other list's end or
;; a mismatch beyond the initial cycle-detection window.
(qa (qa181-both (qa181-cycle '(1)) nil) '(nil nil))
(qa (qa181-both (qa181-cycle '(1)) '(1 1 1 1 1)) '(nil nil))
(qa (qa181-both (qa181-cycle '(1)) '(1 1 1 1 . 1)) '(nil nil))
(qa (qa181-both (qa181-cycle '(1)) [1]) '(nil nil))
(qa (qa181-both (qa181-cycle '(1))
               (qa181-cycle (append (make-list 40 1) '(2)))) '(nil nil))
(qa (qa181-both (qa181-cycle '(1))
               (qa181-cycle (append (make-list 40 1) '(2)) 40)) '(nil nil))

;; Pin GNU's detector cadence, including its early-match behavior after
;; two detected cycles. GNU can report t for different periodic tails
;; and can be asymmetric here; preserve these compatibility quirks.
(qa (qa181-both (qa181-cycle '(0 1))
               (qa181-cycle '(0 1 0 1) 3)) '(nil nil))
(qa (qa181-both (qa181-cycle '(0 1))
               (qa181-cycle '(0 1 0 1 0) 4)) '(t nil))

;; Identity and sharing remain shortcuts, not requirements for equality.
(qa (let ((a (qa181-cycle '(1 2)))) (equal a a)) t)
(qa (let ((tail (qa181-cycle '(2 3))))
      (equal (cons 1 tail) (cons 1 tail))) t)
(qa (let ((a (list 1 2)))
      (qa181-both (list a a) (list (list 1 2) (list 1 2)))) '(t t))
(qa (let ((a (list 1 2)))
      (qa181-both (list a a) (list (list 1 2) (list 1 3)))) '(nil nil))
(qa (qa181-both '(1 2 . 3) '(1 2 . 3)) '(t t))
(qa (qa181-both '(1 2 . 3) '(1 2 . 4)) '(nil nil))
(qa (qa181-both '(1 2) '(1 2 3)) '(nil nil))

;; Cycles through cars, vectors, and records exercise recursive memoization.
;; Branching backedges must not expand exponentially before the memo depth.
(qa (let ((a (cons nil nil)) (b (cons nil nil)))
      (setcar a a) (setcdr a a) (setcar b b) (setcdr b b)
      (qa181-both a b)) '(t t))
(qa (let ((a (vector nil nil)) (b (vector nil nil)))
      (aset a 0 a) (aset a 1 a) (aset b 0 b) (aset b 1 b)
      (qa181-both a b)) '(t t))
(qa (let ((a (vector nil nil 1)) (b (vector nil nil 2)))
      (aset a 0 a) (aset a 1 a) (aset b 0 b) (aset b 1 b)
      (qa181-both a b)) '(nil nil))
(qa (let ((a (cons nil 1)) (b (cons nil 1)))
      (setcar a a) (setcar b b) (qa181-both a b)) '(t t))
(qa (let ((a (cons nil 1)) (b (cons nil 2)))
      (setcar a a) (setcar b b) (qa181-both a b)) '(nil nil))
(qa (let ((a (vector nil 1)) (b (vector nil 1)))
      (aset a 0 a) (aset b 0 b) (qa181-both a b)) '(t t))
(qa (let ((a (vector nil 1)) (b (vector nil 2)))
      (aset a 0 a) (aset b 0 b) (qa181-both a b)) '(nil nil))
(qa (let ((a (vector nil)) (b (vector nil)) (c (vector nil)))
      (aset a 0 a) (aset b 0 c) (aset c 0 b) (qa181-both a b)) '(t t))
(qa (let ((a (record 'qa181 nil 1)) (b (record 'qa181 nil 1)))
      (aset a 1 a) (aset b 1 b) (qa181-both a b)) '(t t))
(qa (let ((a (record 'qa181 nil 1)) (b (record 'qa181 nil 2)))
      (aset a 1 a) (aset b 1 b) (qa181-both a b)) '(nil nil))
(qa (let* ((a (list nil 1)) (b (list nil 1))
           (av (vector a)) (bv (vector b)))
      (setcar a av) (setcar b bv) (qa181-both a b)) '(t t))
(qa (let* ((a (list nil 1)) (b (list nil 2))
           (av (vector a)) (bv (vector b)))
      (setcar a av) (setcar b bv) (qa181-both a b)) '(nil nil))
(qa (let ((a (qa181-cycle '(1))) (b (qa181-cycle '(1))))
      (qa181-both (vector a 1) (vector b 2))) '(nil nil))
(qa (let ((a (qa181-cycle '(1))) (b (qa181-cycle '(1))))
      (qa181-both (list a 1) (list b 2))) '(nil nil))

;; Long finite tails and relatively prime periods must stay iterative;
;; comparing the pair's whole period would take ~1,000,000 steps here.
(qa (qa181-both (make-list 10000 1) (make-list 10000 1)) '(t t))
(qa (qa181-both (make-list 10000 1)
               (append (make-list 9999 1) '(2))) '(nil nil))
(qa (qa181-both (qa181-cycle (make-list 1009 1))
               (qa181-cycle (make-list 1013 1))) '(t t))
(qa (qa181-both (qa181-cycle (make-list 1009 1))
               (qa181-cycle (append (make-list 1012 1) '(2)))) '(nil nil))
(princ (qa--done)) (terpri)
