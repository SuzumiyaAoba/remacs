;; Misc machinery — GNU 31.1 pins:
;;   - `obarray-make' builds an opaque obarray (NOT a vector: vectorp → nil);
;;     size rounds up to the next Mersenne-ish prime (7 → 7, 0/nil → 4).
;;   - `obarray-get' returns the interned SYMBOL or nil; `obarray-remove'
;;     uninterns and returns t.
;;   - `format-seconds' skips zero-valued units: "%y, %d, %h, %m, %s, %z"
;;     prints only the nonzero components, so 0 gives "0, 0, 0, 0, 0,"
;;     (%z is never emitted for integer input) and 3661 gives "1, 1, 1,".
;;   - `cl-letf' rebinds a PLACE for the dynamic extent: symbol-function,
;;     symbol-value, and plain places like (car l) all restore afterwards.
;;   - `macroexpand-all' expands macros recursively inside the form.
;;   - `equal-including-properties' compares text properties too;
;;     `copy-sequence' PRESERVES text properties on strings.
;;   - bool-vectors: `bool-vector-count-population' is 1-arg (count of t);
;;     set ops return NEW vectors (#&4"" prints as the packed bytes).
;;   - char-tables: default range value nil; range nil in set-char-table-
;;     range means "all characters"; subtype recorded from the ctor.
;;   - `fillarray' fills in place and returns the array object.
(defvar qa-ob (obarray-make 7))
(qa (vectorp qa-ob) nil) ;; obarray is an opaque type, not a plain vector
;; GNU 31.1: obarray-make ignores SIZE and always returns a size-4 table
(qa (obarray-size qa-ob) 4)
(qa (obarray-size (obarray-make 0)) 4)
(qa (obarray-size (obarray-make)) 4)
(qa (obarray-get qa-ob "zz") nil)
(progn (intern "zz" qa-ob) (intern "aa" qa-ob))
;; obarray-get returns the symbol interned INTO THIS obarray — a distinct
;; object from the global `zz', so equal/eq against 'zz both fail
(qa (equal (obarray-get qa-ob "zz") 'zz) nil)
(qa (obarray-get qa-ob "missing") nil)
(qa (let (acc) (mapatoms (lambda (s) (push (symbol-name s) acc)) qa-ob)
      (sort acc #'string<))
    '("aa" "zz"))
(qa (obarray-remove qa-ob "zz") t)
(qa (obarray-get qa-ob "zz") nil)
;; intern into a custom obarray yields a DIFFERENT symbol than global
(qa (let ((ob (obarray-make)))
      (eq (intern "car" ob) 'car))
    nil)
;; unintern with a single arg errors in 31.1 (OBARRAY is required)
(qa-err (unintern 'car) 'wrong-number-of-arguments)

;; == format-seconds ==
(qa (format-seconds "%y, %d, %h, %m, %s, %z" 0) "0, 0, 0, 0, 0,")
(qa (format-seconds "%y, %d, %h, %m, %s, %z" 3661) "1, 1, 1,")
(qa (format-seconds "%y %d %h %m %s" 93784) "0 1 2 3 4")
(qa (format-seconds "%hh %mm %ss" 3661) "1h 1m 1s")
;; literal text passes through (SECONDS required)
(qa (format-seconds "just text" 0) "just text")
;; %m counts TOTAL minutes (61), not the remainder
(qa (format-seconds "%m minutes %s seconds" 3661) "61 minutes 1 seconds")

;; == cl-letf ==
(defun qa--clletf-fn () 'orig)
(qa (cl-letf (((symbol-function 'qa--clletf-fn) (lambda () 'temp)))
      (qa--clletf-fn))
    'temp)
(qa (qa--clletf-fn) 'orig)
(defvar qa-dyn-x 1)
(qa (cl-letf (((symbol-value 'qa-dyn-x) 99)) qa-dyn-x) 99)
(qa qa-dyn-x 1)
;; (car l) as a cl-letf place is NOT supported by GNU 31.1 — it expands
;; to a plain progn (no rebinding), so l keeps its original car both
;; inside the dynamic extent and after
(qa (let ((l (list 1 2 3)))
      (list (cl-letf (((car l) 99)) l) l))
    '((1 2 3) (1 2 3)))
(qa (let ((l (list 1)))
      (list (catch 'out
              (cl-letf (((car l) 'boom))
                (throw 'out l))
              'no)
            l))
    '((1) (1)))

;; == macroexpand-all ==
(qa (macroexpand-all '(when a b c)) '(if a (progn b c)))
(qa (macroexpand-all '(list 1 2)) '(list 1 2))
(qa (let* ((a (macroexpand-all '(let ((x 1)) (when x (dotimes (i 2) i)))))
           (ifform (caddr a))
           (innerlet (cadr (caddr ifform)))
           (binds (cadr innerlet)))
      (list (car ifform)
            (car innerlet)
            (eq (caar binds) 'upper-bound)
            (symbol-name (caar binds))))
    '(if let nil "upper-bound"))
;; plain forms are returned untouched
(qa (macroexpand-all '(progn a b)) '(progn a b))
(qa (macroexpand-all '(quote (a b))) '(quote (a b)))
(qa (macroexpand-all 'foo) 'foo)
(qa (macroexpand-all 42) 42)

;; == equal-including-properties ==
(qa (let ((s (copy-sequence "ab")))
      (put-text-property 0 1 'face 'bold s)
      (list (equal s "ab")
            (equal-including-properties s "ab")
            (equal-including-properties s (copy-sequence s))
            (equal-including-properties s s)))
    '(t nil t t))
;; two strings with different props differ
(qa (let ((s1 (copy-sequence "ab")) (s2 (copy-sequence "ab")))
      (put-text-property 0 1 'face 'bold s1)
      (list (equal s1 s2) (equal-including-properties s1 s2)))
    '(t nil))
;; non-strings behave like equal
(qa (equal-including-properties '(1 2 3) '(1 2 3)) t)
(qa (equal-including-properties 1.5 1.5) t)

;; == bool-vectors ==
(qa (bool-vector-p (make-bool-vector 4 t)) t)
(qa (bool-vector-p "x") nil)
(qa (bool-vector-p [t t]) nil)
(qa (length (make-bool-vector 4 t)) 4)
(qa (make-bool-vector 4 t) #&4"\x0f")
(qa (aref (make-bool-vector 4 t) 0) t)
(qa (let ((bv (make-bool-vector 4 t))) (aset bv 1 nil) (aref bv 1)) nil)
(qa-err (make-bool-vector -1 t) 'wrong-type-argument)
(qa (subr-arity (symbol-function 'bool-vector-count-population)) '(1 . 1))
(qa (let ((bv (make-bool-vector 4 t))) (aset bv 1 nil)
      (bool-vector-count-population bv))
    3)
(qa (let ((bv (make-bool-vector 4 t))) (aset bv 1 nil)
      (list (bool-vector-count-consecutive bv t 0)
            (bool-vector-count-consecutive bv t 2)
            (bool-vector-count-consecutive bv nil 1)
            (bool-vector-count-consecutive bv nil 0)))
    '(1 2 1 0))
;; set ops return new vectors
(qa (let ((a (make-bool-vector 4 t)) (b (make-bool-vector 4 nil)))
      (aset a 1 nil) (aset b 1 t)
      (list (bool-vector-union a b)
            (bool-vector-intersection a b)
            (bool-vector-not a)
            (bool-vector-exclusive-or a b)
            (bool-vector-subsetp a b)
            (bool-vector-subsetp b a)
            (bool-vector-subsetp a a)))
    '(#&4"\x0f" #&4"\x00" #&4"\x02" #&4"\x0f" nil nil t))

;; == char-tables ==
(qa (let ((ct (make-char-table 'test nil)))
      (set-char-table-range ct ?a 1)
      (list (char-table-p ct)
            (char-table-range ct ?a)
            (char-table-range ct ?b)
            (char-table-subtype ct)))
    '(t 1 nil test))
;; range nil in set-char-table-range covers all chars
(qa (let ((ct (make-char-table 'test nil)))
      (set-char-table-range ct nil 'all)
      (char-table-range ct ?b))
    'all)
;; init value applies everywhere
(qa (char-table-range (make-char-table 'test 99) ?q) 99)
;; a parent supplies defaults for unset ranges
(qa (let ((ct (make-char-table 'test nil))
          (p (make-char-table 'test 'parent)))
      (set-char-table-parent ct p)
      (list (char-table-range ct ?c) (char-table-p (char-table-parent ct))))
    '(parent t))
;; extra slots
(qa-err (let ((ct (make-char-table 'test nil)))
          (set-char-table-extra-slot ct 0 'x))
        'args-out-of-range)
;; map-char-table visits explicit ranges — GNU emits (97 va) plus the two
;; (98 . 4194303) 'dft range entries (ASCII + multibyte halves) for the
;; default entry, so three calls in total.
(qa (let ((ct (make-char-table 'test nil)) (acc 0))
      (set-char-table-range ct ?a 'va)
      (set-char-table-range ct nil 'dft)
      (map-char-table (lambda (k v) (setq acc (1+ acc))) ct)
      acc)
    3)

;; == fillarray ==
(qa (let ((v (vector 1 2 3))) (eq (fillarray v 'x) v)) t)
(qa (let ((v (vector 1 2 3))) (fillarray v 'x) v) [x x x])
(qa (let ((s (copy-sequence "ab"))) (fillarray s ?z) s) "zz")
(qa (let ((b (make-bool-vector 3 nil))) (fillarray b t) (aref b 0)) t)
(qa-err (fillarray 5 'x) 'wrong-type-argument)
(qa-err (fillarray (list 1 2) 'x) 'wrong-type-argument)

;; == macroexpand vs macroexpand-all ==
(qa (macroexpand '(when a b)) '(if a (progn b)))
(qa (macroexpand '(when a (when b c)))
    '(if a (progn (when b c)))) ;; single level only
(qa (macroexpand-all '(when a (when b c)))
    '(if a (progn (if b (progn c)))))

;; == unload-feature ==
;; a provide with no load-history file record can't be unloaded — GNU
;; 31.1 errors inside file-provides on the nil file name
(qa-err (progn (provide 'qa-nonexistent-feature)
              (unload-feature 'qa-nonexistent-feature))
        'wrong-type-argument)

;; == sort's hidden cousin: sort-lines on a single-line buffer is a no-op
(qa (with-temp-buffer (insert "only\n") (sort-lines nil (point-min) (point-max))
      (buffer-string))
    "only\n")

;; == string-equal vs case-fold-search ==
;; string-equal is always case-SENSITIVE under -Q: it ignores
;; case-fold-search entirely
(qa (let ((case-fold-search t)) (string-equal "A" "a")) nil)
(qa (let ((case-fold-search nil)) (string-equal "A" "a")) nil)
(qa (string-equal "A" "a") nil)
(qa (string= "A" "a") nil)
(qa (string-equal "abc" "abc") t)

;; == compare-strings ==
(qa (compare-strings "abc" 0 nil "abc" 0 nil) t)
(qa (compare-strings "abc" 0 nil "abd" 0 nil) -3)
(qa (compare-strings "abd" 0 nil "abc" 0 nil) 3)
(qa (compare-strings "abc" 1 nil "xbc" 1 nil) t)
;; ignore-case arg
(qa (compare-strings "ABC" 0 nil "abc" 0 nil t) t)

;; == assoc-string ==
(qa (assoc-string "b" '(("a" . 1) ("b" . 2))) '("b" . 2))
(qa (assoc-string "b" '(("A" . 1) ("b" . 2)) t) '("b" . 2))
(qa (assoc-string "B" '(("b" . 2))) nil)
(qa (assoc-string "B" '(("b" . 2)) t) '("b" . 2))

;; == string-equal-ignore-case exists in 31.1 and folds ASCII case ==
(qa (fboundp 'string-equal-ignore-case) t)
(qa (string-equal-ignore-case "A" "a") t)
(qa (string-equal-ignore-case "AbC" "aBc") t)
(qa (string-equal-ignore-case "a" "b") nil)
(princ (qa--done))
