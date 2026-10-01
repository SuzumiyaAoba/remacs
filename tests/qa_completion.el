;; Completion machinery: try-/all-/test-completion over list/alist/obarray/
;; hash collections, hand-rolled function tables (all five actions),
;; curried table combinators, styles, and case-folding variables.
;; GNU-verified: `dynamic-completion-table' (old name) does not exist in
;; 31.1; `completion-table-apply'/`-quote'/`-refresh' don't either.

;; == try-completion on list collections ==
;; Returns longest common prefix; t iff sole possible completion; nil none.
(qa (try-completion "fo" '(("foo" . 1) ("foobar" . 2))) "foo")
(qa (try-completion "foo" '(("foo" . 1) ("foobar" . 2))) "foo")
(qa (try-completion "foox" '(("foo" . 1) ("foobar" . 2))) nil)
(qa (try-completion "" '(("foo" . 1) ("foobar" . 2))) "foo")
(qa (try-completion "foo" '(("foo" . 1))) t)     ; sole candidate → t
(qa (try-completion "foo" '("foo")) t)
(qa (try-completion "foo" '("foo" "foobar")) "foo") ; exact but not sole → string
(qa (try-completion "f" '("foo" "fie" "fum")) "f")
(qa (try-completion "z" '("foo" "fie")) nil)
(qa (try-completion "f" nil) nil)
(qa (try-completion "" '()) nil)
;; Symbol-keyed alist and plain symbol list are accepted; result is a string.
(qa (try-completion "fo" '((foo . 1) (foobar . 2))) "foo")
(qa (try-completion "f" '(foo fie)) "f")
(qa (try-completion "foo" '(foo)) t)
;; Non-string, non-symbol alist keys are ignored.
(qa (try-completion "f" '((foo . 1) (5 . 2) ("fie" . 3))) "f")
;; PREDICATE filters before the prefix is computed.
(qa (try-completion "fo" '(("foo" . 1) ("foobar" . 2))
                    (lambda (x) (= (cdr x) 1)))
    "foo")
(qa (try-completion "fo" '(("foo" . 1) ("foobar" . 2))
                    (lambda (x) (= (cdr x) 2)))
    "foobar")
(qa (try-completion "f" '("foo" "fie")
                    (lambda (s) (equal s "fie")))
    "fie")
(qa (try-completion "f" '("foo" "fie") (lambda (s) (> (length s) 3))) nil)

;; == case folding ==
(qa (try-completion "FOO" '(("foo" . 1))) nil)   ; case-sensitive by default
(qa (let ((completion-ignore-case t))
      (try-completion "FOO" '(("foo" . 1))))
    "foo")
(qa (let ((completion-ignore-case t))
      (try-completion "FO" '(("foo" . 1) ("FOO" . 2))))
    "FOO")
(qa (all-completions "F" '("foo")) nil)
(qa (let ((completion-ignore-case t))
      (all-completions "F" '("foo" "fie")))
    '("foo" "fie"))

;; == completion-regexp-list ==
(qa (let ((completion-regexp-list '("^fo")))
      (try-completion "f" '("foo" "fie" "bar")))
    "foo")
(qa (let ((completion-regexp-list '("^fo")))
      (all-completions "" '("foo" "fie" "bar")))
    '("foo"))
;; Multiple regexps are AND-ed.
(qa (let ((completion-regexp-list '("^f" "o$")))
      (all-completions "" '("foo" "fo" "fie" "bar")))
    '("foo" "fo"))
(qa (let ((completion-regexp-list '("^z")))
      (try-completion "" '("foo")))
    nil)

;; == all-completions ==
;; Order = collection order; candidates returned as strings even for symbols.
(qa (all-completions "f" '("foo" "fie" "fum")) '("foo" "fie" "fum"))
(qa (all-completions "" '("foo" "fie" "fum")) '("foo" "fie" "fum"))
(qa (all-completions "z" '("foo" "fie")) nil)
(qa (all-completions "" nil) nil)
(qa (all-completions "fo" '(("foo" . 1) ("foobar" . 2) ("bar" . 3)))
    '("foo" "foobar"))
(qa (all-completions "f" '(("foo" . 1) (fie . 2) ("fum" . 3)))
    '("foo" "fie" "fum"))
(qa (all-completions "f" '(foo fie)) '("foo" "fie"))
(qa (all-completions "" '((foo . 1) (5 . 2) ("fie" . 3)))
    '("foo" "fie"))
;; Predicate receives the collection element (cons cell for alists).
(qa (all-completions "f" '(("foo" . 1) ("foobar" . 2))
                     (lambda (x) (= (cdr x) 2)))
    '("foobar"))
(qa (all-completions "f" '((foo . 1) (fie . 2))
                     (lambda (x) (= (cdr x) 2)))
    '("fie"))
(qa (all-completions "f" '("foo" "fie") (lambda (s) (> (length s) 3))) nil)
(qa (all-completions "f" '(foo fie) #'symbolp) '("foo" "fie"))
;; Text properties on candidate strings are preserved.
(qa (all-completions "f" '(#("foo" 0 3 (face bold))))
    '(#("foo" 0 3 (face bold))))

;; == test-completion ==
(qa (test-completion "foo" '(("foo" . 1) ("foobar" . 2))) t)
(qa (test-completion "fo" '(("foo" . 1) ("foobar" . 2))) nil)
(qa (test-completion "bar" '("foo")) nil)
(qa (test-completion "foo" '(("foo" . 1)) (lambda (x) (= (cdr x) 2))) nil)
(qa (test-completion "" nil) nil)

;; == obarray collection ==
(let ((qacmp-ob (make-vector 8 0)))
  (intern "alpha" qacmp-ob)
  (intern "alpine" qacmp-ob)
  (intern "beta" qacmp-ob)
  (qa (sort (all-completions "al" qacmp-ob) #'string<) '("alpha" "alpine"))
  (qa (try-completion "al" qacmp-ob) "alp")
  (qa (test-completion "alpha" qacmp-ob) t))

;; == hash-table collection ==
;; Predicate gets two arguments: KEY and VALUE.
(let ((qacmp-h (make-hash-table :test 'equal)))
  (puthash "ax" 1 qacmp-h)
  (puthash "ay" 2 qacmp-h)
  (puthash "bx" 3 qacmp-h)
  (qa (sort (all-completions "a" qacmp-h) #'string<) '("ax" "ay"))
  (qa (try-completion "a" qacmp-h) "a")
  (qa (try-completion "ax" qacmp-h) t)
  (qa (test-completion "ax" qacmp-h) t)
  (qa (test-completion "az" qacmp-h) nil)
  (qa (sort (all-completions "a" qacmp-h (lambda (k v) (= v 2))) #'string<)
      '("ay"))
  (qa (try-completion "a" qacmp-h (lambda (k v) (= v 2))) "ay")
  (qa (test-completion "ax" qacmp-h (lambda (k v) (= v 1))) t))

;; == errors on bad collection types ==
;; A number is treated as a function table and then fails funcall.
(qa-err (try-completion "f" 5) 'invalid-function)
(qa-err (all-completions "f" 5) 'invalid-function)
;; A vector is treated as an obarray.
(qa-err (all-completions "f" [a b]) 'wrong-type-argument)
;; Non-string input string.
(qa-err (try-completion 5 '("foo")) 'wrong-type-argument)
(qa-err (try-completion 'sym '("a")) 'wrong-type-argument)

;; == hand-rolled function table: all five actions ==
(defun qacmp-table (string pred action)
  "Completion table: candidates \"foo\", \"foobar\", \"bar\"."
  (pcase action
    ('nil (cond ((equal string "foo") t)
                ((string-prefix-p string "foobar") "foobar")
                (t nil)))
    ('t (delq nil (mapcar (lambda (s) (and (string-prefix-p string s) s))
                          '("foo" "foobar" "bar"))))
    ('lambda (equal string "foo"))
    (`(boundaries . ,suffix) `(boundaries 0 . ,(length suffix)))
    ('metadata '(metadata (category . qacmp-cat)
                          (display-sort-function . identity)))))

(qa (try-completion "fo" #'qacmp-table) "foobar")
(qa (try-completion "foo" #'qacmp-table) t)
(qa (try-completion "zz" #'qacmp-table) nil)
(qa (all-completions "f" #'qacmp-table) '("foo" "foobar"))
(qa (all-completions "" #'qacmp-table) '("foo" "foobar" "bar"))
(qa (test-completion "foo" #'qacmp-table) t)
(qa (test-completion "fo" #'qacmp-table) nil)
;; (boundaries . SUFFIX) action → (START . END) for the completion field.
(qa (completion-boundaries "pre" #'qacmp-table nil "suf") '(0 . 3))
(qa (completion-boundaries "pre" #'qacmp-table nil "") '(0 . 0))
(qa (completion-metadata "x" #'qacmp-table nil)
    '(metadata (category . qacmp-cat) (display-sort-function . identity)))
(qa (completion-metadata-get
     '(metadata (category . qacmp-cat)) 'category)
    'qacmp-cat)
(qa (completion-metadata-get '(metadata) 'category) nil)

;; Table that omits the boundaries/metadata actions: defaults apply.
(defun qacmp-plain (s p a)
  (cond ((eq a 'lambda) (equal s "foo"))
        ((eq a nil) (try-completion s '("foo" "fie")))
        ((eq a t) (all-completions s '("foo" "fie")))))

(qa (completion-boundaries "f" #'qacmp-plain nil "xy") '(0 . 2))
(qa (completion-metadata "x" #'qacmp-plain nil) '(metadata))
(qa (completion-metadata "x" '("a" "b") nil) '(metadata))
;; Plain list table boundaries: always (0 . (length SUFFIX)).
(qa (completion-boundaries "foo" '("foo" "bar") nil "suf") '(0 . 3))
(qa (completion-boundaries "foo" '("foo" "bar") nil "") '(0 . 0))
;; File-name table: boundary delimited at the last directory separator.
(qa (completion-boundaries "x/y" #'completion-file-name-table nil "")
    '(2 . 0))

;; == completion-table-dynamic / -with-cache ==
;; Dynamic calls FUN on each lookup (FUN takes the input string).  FUNs
;; are byte-compiled wrappers, so the counter must be a special (defvar'd)
;; variable — a lexical `let' binding goes void inside them.
(defvar qacmp-n 0)
(let ((qacmp-tab (completion-table-dynamic
                  (lambda (s) (setq qacmp-n (1+ qacmp-n)) '("da" "db")))))
  (setq qacmp-n 0)
  (qa (all-completions "d" qacmp-tab) '("da" "db"))
  (qa (try-completion "d" qacmp-tab) "d")
  (qa (all-completions "d" qacmp-tab) '("da" "db"))
  ;; try-completion (not just all-completions) also consults FUN.
  (qa qacmp-n 3))
;; Cached variant: same input (or extension of it) reuses one FUN call.
(let ((qacmp-tab (completion-table-with-cache
                  (lambda (s) (setq qacmp-n (1+ qacmp-n)) '("xa" "xb")))))
  (setq qacmp-n 0)
  (qa (all-completions "x" qacmp-tab) '("xa" "xb"))
  (qa (all-completions "x" qacmp-tab) '("xa" "xb"))
  (qa (all-completions "xa" qacmp-tab) '("xa"))
  (qa qacmp-n 1))
;; lazy-completion-table (obsolete in 31.1 but still functional): FUN is
;; called on first use and the result stored in VAR.
(let ((qacmp-lazy nil))
  (setq qacmp-lazy (lazy-completion-table qacmp-lazy (lambda () '("la" "lb"))))
  (qa (try-completion "l" qacmp-lazy) "l")
  (qa qacmp-lazy '("la" "lb")))

;; == curried combinators ==
;; with-terminator appends TERMINATOR when completion is complete.
(let ((qacmp-tab (apply-partially #'completion-table-with-terminator
                                  "|" '("foo" "fie"))))
  (qa (try-completion "foo" qacmp-tab) "foo|")
  (qa (try-completion "f" qacmp-tab) "f")
  (qa (all-completions "f" qacmp-tab) '("foo" "fie")))
;; with-predicate: STRICT nil falls back to unfiltered result when the
;; predicate would reduce the completion set to nothing.
(let ((qacmp-tab (apply-partially #'completion-table-with-predicate
                                  '("foo" "fie" "bar")
                                  (lambda (s) (> (length s) 3)) t)))
  (qa (all-completions "" qacmp-tab) nil))
(let ((qacmp-tab (apply-partially #'completion-table-with-predicate
                                  '("foo" "fie")
                                  (lambda (s) (> (length s) 100)) nil)))
  (qa (all-completions "" qacmp-tab) '("foo" "fie")))
;; case-fold: independent of completion-ignore-case.
(let ((qacmp-tab (completion-table-case-fold '("FOO" "FIe"))))
  (qa (try-completion "f" qacmp-tab) "F")
  (qa (all-completions "f" qacmp-tab) '("FOO" "FIe")))
(let ((qacmp-tab (completion-table-case-fold '("FOO" "FIe") t)))
  (qa (try-completion "f" qacmp-tab) nil))
;; subvert: completes "(concat S1 S)" as table would "(concat S2 S)".
(let ((qacmp-tab (completion-table-subvert '("foo" "bar") "pre-" "")))
  (qa (all-completions "pre-f" qacmp-tab) '("foo"))
  (qa (try-completion "pre-f" qacmp-tab) "pre-foo"))
(let ((qacmp-tab (completion-table-subvert '("ax" "ay") "q-" "z-")))
  (qa (completion-boundaries "q-a" qacmp-tab nil "") '(2 . 0)))
;; with-context: PREFIX is stripped for the table but kept in boundaries.
(let ((qacmp-tab (apply-partially #'completion-table-with-context
                                  "ctx:" '("foo" "fie"))))
  (qa (all-completions "f" qacmp-tab) '("foo" "fie"))
  (qa (completion-boundaries "ctx:f" qacmp-tab nil "") '(4 . 0)))

;; == style-dispatch layer (completion-try/all-completions) ==
;; Returns (NEWSTRING . NEWPOINT) instead of the bare string.
(qa (completion-try-completion "f" '("foo" "fie") nil 1) '("f" . 1))
(qa (completion-try-completion "fo" '("foo" "fie") nil 1) '("foo" . 3))
(qa (completion-try-completion "foo" '("foo" "fie") nil 3) t)
;; all-completions: propertized strings; dotted tail holds base-size 0.
(qa-prin1 (completion-all-completions "f" '("foo" "fie") nil 1)
          "(#(\"foo\" 0 1 (face completions-common-part) 1 2 (face completions-first-difference)) #(\"fie\" 0 1 (face completions-common-part) 1 2 (face completions-first-difference)) . 0)")
(qa-prin1 (completion-all-completions "" '("foo" "fie") nil 0)
          "(\"foo\" \"fie\" . 0)")
;; completion-styles applies only through this layer, not to the
;; primitive try-/all-completions.
(let ((completion-styles '(substring)))
  (qa (all-completions "oo" '("foo" "bar")) nil)
  (qa (try-completion "oo" '("foo" "bar")) nil))
(let ((completion-styles '(substring)))
  (qa (completion-try-completion "oo" '("foo" "zoo") nil 2) '("oo" . 2))
  (qa-prin1 (completion-all-completions "oo" '("foo" "bar") nil 2)
            "(#(\"foo\" 1 3 (face completions-common-part)) . 0)"))
(let ((completion-styles '(flex)))
  (qa (let ((qacmp-l (completion-all-completions
                      "fb" '("foo-bar" "fie-bum" "bar") nil 2))
            (qacmp-r nil))
        (while (consp qacmp-l)
          (push (substring-no-properties (car qacmp-l)) qacmp-r)
          (setq qacmp-l (cdr qacmp-l)))
        (nreverse qacmp-r))
      '("foo-bar" "fie-bum")))
(let ((completion-styles '(partial-completion)))
  (qa (let ((qacmp-l (completion-all-completions
                      "f-b" '("foo-bar" "fie-bum" "bar") nil 3))
            (qacmp-r nil))
        (while (consp qacmp-l)
          (push (substring-no-properties (car qacmp-l)) qacmp-r)
          (setq qacmp-l (cdr qacmp-l)))
        (nreverse qacmp-r))
      '("foo-bar" "fie-bum")))

;; == styles alist and variable defaults ==
(qa (mapcar #'car completion-styles-alist)
    '(emacs21 emacs22 basic partial-completion substring flex initials
              shorthand))
(qa (consp (assq 'basic completion-styles-alist)) t)
(qa (consp (assq 'substring completion-styles-alist)) t)
(qa (consp (assq 'flex completion-styles-alist)) t)
(qa completion-styles '(basic partial-completion emacs22))
(qa completion-ignore-case nil)
(qa completion-regexp-list nil)
(qa minibuffer-completion-table nil)
(qa minibuffer-completion-predicate nil)
(qa completion-category-defaults
    '((buffer (styles basic substring))
      (unicode-name (styles basic substring))
      (project-file (styles substring))
      (xref-location (styles substring))
      (info-menu (styles basic substring))
      (symbol-help (styles basic shorthand substring))))
(qa completion-category-overrides nil)
(qa completion-extra-properties nil)

(princ (qa--done))
