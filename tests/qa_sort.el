;; Sorting semantics — GNU 31.1 pins:
;;   - `sort' takes (1 . many) args, sorts lists AND vectors in place and
;;     returns the same object (eq).  Strings signal wrong-type-argument.
;;   - `cl-sort' == `sort' for lists/vectors; for strings it converts to a
;;     list of chars, sorts, and returns a NEW string (s is unchanged).
;;   - `cl-stable-sort' is a separate defalias, not eq to `sort' or
;;     `cl-sort'; all three are stable mergesorts.
;;   - The predicate is called as (PRED a b): non-nil => a precedes b; a
;;     non-function predicate signals invalid-function.
;;   - `cl-merge' produces a NEW sequence of the requested type; it is NOT
;;     destructive on the input lists.
;;   - Buffer-level sorts: sort-lines/-fields/-numeric-fields/-columns/
;;     -paragraphs/-pages/-regexp-fields rewrite the region in place.
;;     `sort-fields' compares whole fields as strings; `sort-numeric-fields'
;;     reads a NUMBER prefix inside the chosen field (sign allowed);
;;     `sort-columns' compares the text inside the column window.
(require 'cl-lib)

;; == sort core ==
(qa (sort (list 3 1 2) #'<) '(1 2 3))
(qa (sort (vector 3 1 2) #'<) [1 2 3])
(qa (sort (list 1 2 3) #'>) '(3 2 1))
(qa (sort nil #'<) nil)
(qa (sort (list 9) #'<) '(9))
;; sort on a string signals wrong-type-argument (list-or-vector-p)
(qa-err (sort (copy-sequence "cba") #'<) 'wrong-type-argument)
;; predicates can be symbols, lambdas, #'-quoted
(qa (sort (list 3 1 2) '<) '(1 2 3))
(qa (sort (list 3 1 2) (lambda (a b) (< a b))) '(1 2 3))
(qa (sort (list 3 1 2) (lambda (a b) (> a b))) '(3 2 1))
;; in-place: result is eq to the input
(qa (let ((l (list 3 1 2))) (eq (sort l #'<) l)) t)
(qa (let ((v (vector 3 1 2))) (eq (sort v #'<) v)) t)
;; …and the input binding still points at the sorted structure
(qa (let ((l (list 3 1 2))) (sort l #'<) l) '(1 2 3))
(qa (let ((v (vector 3 1 2))) (sort v #'<) v) [1 2 3])
;; stability: equal keys keep input order (c . 0) first, a/b/d keep order
(qa (sort (list (cons 'a 1) (cons 'b 1) (cons 'c 0) (cons 'd 1))
          (lambda (x y) (< (cdr x) (cdr y))))
    '((c . 0) (a . 1) (b . 1) (d . 1)))
;; arity: (1 . many)
(qa (subr-arity (symbol-function 'sort)) '(1 . many))
;; error on non-sequence; a non-function predicate signals invalid-function
(qa-err (sort 5 #'<) 'wrong-type-argument)
(qa-err (sort (list 1 2) 5) 'invalid-function)

;; == cl-sort / cl-stable-sort ==
(qa (cl-sort (list 3 1 2) #'<) '(1 2 3))
(qa (cl-sort (vector 3 1 2) #'<) [1 2 3])
;; strings: cl-sort returns a NEW sorted string; the input is intact
(qa (cl-sort (copy-sequence "cba") #'<) "abc")
(qa (let ((s (copy-sequence "cba")))
      (list (cl-sort s #'<) s (eq (cl-sort s #'<) s)))
    '("abc" "cba" nil))
(qa (cl-sort (copy-sequence "") #'<) "")
(qa (cl-sort (list (cons 'a 1) (cons 'b 1) (cons 'c 0))
             (lambda (x y) (< (cdr x) (cdr y))))
    '((c . 0) (a . 1) (b . 1)))
;; cl-stable-sort is also stable, and is NOT eq to sort
(qa (cl-stable-sort (list 3 1 2) #'<) '(1 2 3))
(qa (eq (symbol-function 'cl-sort) (symbol-function 'sort)) nil)
(qa (eq (symbol-function 'cl-stable-sort) (symbol-function 'sort)) nil)

;; == cl-merge ==
(qa (cl-merge 'list (list 1 3) (list 2 4) #'<) '(1 2 3 4))
(qa (cl-merge 'vector (list 1 3) (list 2 4) #'<) [1 2 3 4])
;; mixing input sequence types
(qa (cl-merge 'list (vector 1 3) "bd" #'<) '(1 3 98 100))
(qa (cl-merge 'string (list ?a ?c) (list ?b ?d) #'<) "abcd")
(qa (cl-merge 'list nil (list 1 2) #'<) '(1 2))
(qa (cl-merge 'list (list 1 2) nil #'<) '(1 2))
(qa (cl-merge 'list nil nil #'<) nil)
;; merge keeps equal-key stability within each input
(qa (cl-merge 'list (list (cons 'a 1) (cons 'c 2))
              (list (cons 'b 1) (cons 'd 2))
              (lambda (x y) (< (cdr x) (cdr y))))
    '((a . 1) (b . 1) (c . 2) (d . 2)))

;; == buffer sort commands ==
(qa (with-temp-buffer
      (insert "banana\napple\ncherry\n")
      (sort-lines nil (point-min) (point-max))
      (buffer-string))
    "apple\nbanana\ncherry\n")
(qa (with-temp-buffer
      (insert "b\na\n")
      (sort-lines nil (point-min) (point-max))
      (buffer-string))
    "a\nb\n")
;; REVERSE non-nil sorts descending
(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (sort-lines 'reverse (point-min) (point-max))
      (buffer-string))
    "c\nb\na\n")
;; sort-fields: FIELD counts whitespace-separated fields, 1-based
(qa (with-temp-buffer
      (insert "b 2\na 10\nc 1\n")
      (sort-fields 1 (point-min) (point-max))
      (buffer-string))
    "a 10\nb 2\nc 1\n")
(qa (with-temp-buffer
      (insert "x c\ny a\nz b\n")
      (sort-fields 2 (point-min) (point-max))
      (buffer-string))
    "y a\nz b\nx c\n")
;; sort-numeric-fields: number prefix inside the field, sign tolerated
(qa (with-temp-buffer
      (insert "b 2\na 10\nc 1\n")
      (sort-numeric-fields 2 (point-min) (point-max))
      (buffer-string))
    "c 1\nb 2\na 10\n")
(qa (with-temp-buffer
      (insert "a -5\nb -10\nc 0\n")
      (sort-numeric-fields 2 (point-min) (point-max))
      (buffer-string))
    "b -10\na -5\nc 0\n")
;; no number → 0
(qa (with-temp-buffer
      (insert "x foo\ny 3\nz 1\n")
      (sort-numeric-fields 2 (point-min) (point-max))
      (buffer-string))
    "x foo\nz 1\ny 3\n")
;; sort-columns: compares inside the column window (default = whole line)
(qa (with-temp-buffer
      (insert "cc 10 zz\naa 30 yy\nbb 20 xx\n")
      (sort-columns nil (point-min) (point-max))
      (buffer-string))
    "aa 30 yy\nbb 20 xx\ncc 10 zz\n")
(qa (with-temp-buffer
      (insert "c b a\na c b\nb a c\n")
      (sort-columns 'reverse (point-min) (point-max))
      (buffer-string))
    "c b a\nb a c\na c b\n")
;; sort-paragraphs: blank-line-separated paragraphs
(qa (with-temp-buffer
      (insert "zebra\n\napple\n\n")
      (sort-paragraphs nil (point-min) (point-max))
      (buffer-string))
    "apple\n\nzebra\n\n")
;; sort-pages: formfeed-separated pages
(qa (with-temp-buffer
      (insert "b\n\f\na\n\f\nc\n")
      (sort-pages nil (point-min) (point-max))
      (buffer-string))
    "a\n\f\nb\n\f\nc\n")
;; reverse-region
(qa (with-temp-buffer
      (insert "1\n2\n3\n")
      (reverse-region (point-min) (point-max))
      (buffer-string))
    "3\n2\n1\n")
(qa (with-temp-buffer
      (insert "x")
      (reverse-region (point-min) (point-max))
      (buffer-string))
    "x")
;; sort-regexp-fields: KEY selects the sort key inside each record —
;; "\\2" is group 2 of RECORD-REGEXP, "\\&" is the whole record, or a
;; regexp matched against the record text.  A numeric KEY errors.
(qa (with-temp-buffer
      (insert "b: 2\na: 10\nc: 1\n")
      (sort-regexp-fields nil "^\\(.*\\): \\(.*\\)$" "\\2"
                          (point-min) (point-max))
      (buffer-string))
    "c: 1\na: 10\nb: 2\n")
(qa (with-temp-buffer
      (insert "b: 2\na: 10\nc: 1\n")
      (sort-regexp-fields nil "^\\(.*\\): \\(.*\\)$" "\\&"
                          (point-min) (point-max))
      (buffer-string))
    "a: 10\nb: 2\nc: 1\n")
(qa-err (with-temp-buffer
          (insert "b: 2\na: 10\nc: 1\n")
          (sort-regexp-fields nil "^\\(.*\\): \\(.*\\)$" 2
                              (point-min) (point-max)))
        'wrong-type-argument)

;; == case-fold-search interaction with string predicates ==
;; `string-lessp' compares case-insensitively (this GNU build's
;; string-lessp applies case folding regardless of case-fold-search)
(qa (sort (list "b" "A" "c") #'string-lessp) '("A" "b" "c"))
;; a literal symbol predicate is resolved like a function
(qa (sort (list "b" "a") 'string<) '("a" "b"))

(princ (qa--done))
