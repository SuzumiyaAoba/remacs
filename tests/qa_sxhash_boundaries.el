;;; -*- lexical-binding: t; -*-
;; Issue #91: pin deterministic GNU hashes. Identity-based addresses
;; may differ across sessions; the signed fixnum endpoints do not.
(defvar qa91-case)
(dolist (qa91-case
         '((-2305843009213693953 -1729382256910270443 -1729382256910270443)
           (-2305843009213693952 -1729382256910270464 -1729382256910270462)
           (-2305843009213693951 -1729382256910270463 -1729382256910270458)
           (-1 -1152921504606846976 -1152921504606846969)
           (0 0 2) (1 1 6) (5 4 3)
           (2305843009213693950 1729382256910270465 1729382256910270467)
           (2305843009213693951 1729382256910270464 1729382256910270471)
           (2305843009213693952 -1729382256910270464 -1729382256910270464)))
  (qa (sxhash-equal (car qa91-case)) (nth 1 qa91-case))
  (qa (sxhash-eql (car qa91-case)) (nth 2 qa91-case)))
(qa (= (sxhash-eq most-negative-fixnum) (sxhash-eql most-negative-fixnum)) t)
(qa (mapcar #'sxhash-eq '(0 1 5 -1 123)) '(2 6 3 -1152921504606846969 98))
(qa (mapcar #'sxhash-equal '(0 1 5 -1 123)) '(0 1 4 -1152921504606846976 101))
(qa (list (sxhash-equal "abc") (sxhash "abc")) '(8059383 8059383))
(qa (mapcar (lambda (symbol) (list (sxhash-eq symbol) (sxhash-equal symbol)))
            '(nil t car)) '((0 0) (15 15) (5790 5790)))

;; Hash equality follows the associated comparison; distinct identity
;; hashes are allowed to collide, so their absolute values are not pinned.
(qa (let ((symbol (make-symbol "abc")))
      (list (= (sxhash-eq symbol) (sxhash-equal symbol))
            (= (sxhash-eq symbol) (sxhash-eql symbol)))) '(t t))
(qa (let ((a (copy-sequence "abc")) (b (copy-sequence "abc")))
      (list (equal a b) (= (sxhash-equal a) (sxhash-equal b))
            (integerp (sxhash-eq a)) (integerp (sxhash-eq b)))) '(t t t t))
(qa (let* ((string (copy-sequence "abc")) (identity-hash (sxhash-eq string)))
      (aset string 0 ?z)
      (list (= identity-hash (sxhash-eq string))
            (= (sxhash-equal string) (sxhash-equal "zbc")))) '(t t))
(qa (let ((list '(1 2 3)))
      (list (= (sxhash-eq list) (sxhash-eql list))
            (= (sxhash-equal list) (sxhash-equal (copy-sequence list))))) '(t t))

(princ (qa--done)) (terpri)
