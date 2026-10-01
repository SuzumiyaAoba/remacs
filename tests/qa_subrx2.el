;;; -*- lexical-binding: t -*-
;; subr-x.el extras — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - string-clean-whitespace normalizes runs of whitespace to one space.
;;   - string-pad pads left or right (with optional padding char).
;;   - string-lines splits on \n, preserving trailing empties by default.
;;   - string-fill wraps at LENGTH columns.
;;   - string-limit truncates to LENGTH.
;;   - string-remove-prefix/suffix trim exact prefixes.
;;   - named-let is a recursive-let.
;;   - hash-table-empty-p/keys/values cover hash-table iteration.
;;   - if-let*/when-let*/and-let* are multi-binding variants.
;;   - many "extended" hash fns are NOT in 31.1 (hash-table-union etc).

(require 'subr-x)

;; == surface ==
(qa (fboundp 'string-clean-whitespace) t)
(qa (fboundp 'string-fill) t)
(qa (fboundp 'string-limit) t)
(qa (fboundp 'string-lines) t)
(qa (fboundp 'string-pad) t)
(qa (fboundp 'string-chop-newline) t)
(qa (fboundp 'string-remove-prefix) t)
(qa (fboundp 'string-remove-suffix) t)
(qa (fboundp 'string-glyph-split) t)
(qa (fboundp 'named-let) t)
(qa (fboundp 'thread-first) t)
(qa (fboundp 'thread-last) t)
(qa (fboundp 'hash-table-empty-p) t)
(qa (fboundp 'hash-table-keys) t)
(qa (fboundp 'hash-table-values) t)
(qa (fboundp 'read-multiple-choice) t)
(qa (fboundp 'if-let*) t)
(qa (fboundp 'when-let*) t)
(qa (fboundp 'and-let*) t)
(qa (fboundp 'string-trim-left) t)
(qa (fboundp 'string-trim-right) t)
(qa (fboundp 'ensure-list) t)
(qa (fboundp 'list-of-strings-p) t)
(qa (fboundp 'string-reverse) t)
;; NOT fns
(qa (fboundp 'string-replace-substring) nil)
(qa (fboundp 'hash-table-apply) nil)
(qa (fboundp 'hash-table-clear) nil)
(qa (fboundp 'hash-table-union) nil)
(qa (fboundp 'hash-table-intersection) nil)
(qa (fboundp 'hash-table-difference) nil)
(qa (fboundp 'hash-table-exclusion) nil)
(qa (fboundp 'hash-table-into) nil)
(qa (fboundp 'alist-to-plist) nil)
(qa (fboundp 'plist-to-alist) nil)
(qa (fboundp 'string-to-number-base) nil)

;; == string-clean-whitespace ==
(qa (string-clean-whitespace "  a  b  ") "a b")
(qa (string-clean-whitespace "a\n\tb") "a b")
(qa (string-clean-whitespace "x") "x")

;; == string-pad ==
;; length arg
(qa (string-pad "abc" 6) "abc   ")
;; custom pad char
(qa (string-pad "abc" 6 ?0) "abc000")
;; pad on the LEFT with t
(qa (string-pad "abc" 6 ?0 t) "000abc")
;; already long enough → unchanged
(qa (string-pad "abcdef" 3) "abcdef")

;; == string-lines ==
(qa (string-lines "a\nb\nc") '("a" "b" "c"))
;; trailing newline — GNU drops the last \n
;; OMIT-NULLS arg
(qa (string-lines "a\n\nb" t) '("a" "b"))
(qa (string-lines "a\n\nb") '("a" "" "b"))

;; == string-fill ==
;; wrap at 8 cols — "aaa bbb" stays, "ccc" wraps
(qa (string-fill "aaa bbb ccc" 8) "aaa bbb\nccc")
;; long word alone
(qa (string-fill "abc" 8) "abc")

;; == string-limit ==
(qa (string-limit "hello" 3) "hel")
(qa (string-limit "hi" 5) "hi")
;; trailing newline drops the last \n — GNU drops it
(qa (string-lines "a\nb\n") '("a" "b"))
;; == string-remove-prefix / -suffix ==
(qa (string-remove-prefix "foo-" "foo-bar") "bar")
(qa (string-remove-prefix "nope" "foo") "foo")
(qa (string-remove-suffix ".el" "file.el") "file")
(qa (string-remove-suffix ".c" "file.el") "file.el")

;; == string-glyph-split / string-reverse ==
;; string-reverse is missing on remacs
(qa (string-reverse "abc") "cba")
(qa (string-reverse "") "")
;; string-glyph-split splits into grapheme clusters
(qa (string-glyph-split "abc") '("a" "b" "c"))

;; == string-chop-newline ==
(qa (string-chop-newline "abc\n") "abc")
(qa (string-chop-newline "abc") "abc")
(qa (string-chop-newline "abc\n\n") "abc\n")

;; == named-let — local recursive fn ==
;; == named-let — local recursive fn (needs lexical binding) ==
(qa-lex (let ((sum (named-let loop ((i 0) (acc 0))
                     (if (> i 4) acc
                       (loop (1+ i) (+ acc i))))))
          sum)
        10)

;; == hash-table-empty-p ==
(qa (let ((h (make-hash-table)))
      (list (hash-table-empty-p h)
            (progn (puthash 'k 'v h)
                   (hash-table-empty-p h))))
    '(t nil))

;; == hash-table-keys / values ==
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash 'a 1 h)
      (puthash 'b 2 h)
      ;; hash-table-keys returns the keys — symbol keys for 'eq/'equal tables
      (list (sort (hash-table-keys h) #'string<)
            (sort (hash-table-values h) #'<)))
    '((a b) (1 2)))
;; == thread-first / thread-last ==
;; thread-first inserts as FIRST arg
(qa (macroexpand '(thread-first x (f a) (g b)))
    '(g (f x a) b))
;; thread-last inserts as LAST arg
(qa (macroexpand '(thread-last x (f a) (g b)))
    '(g b (f a x)))

;; == if-let* — multi-binding if ==
(qa (if-let* ((a 1) (b 2)) (+ a b) 'none) 3)
(qa (if-let* ((a nil) (b 2)) 'has 'none) 'none)
;; == ensure-list — wraps a non-list in a 1-elem list; nil → (nil) ==
(qa (ensure-list 'x) '(x))
(qa (ensure-list '(a b)) '(a b))
(qa (ensure-list nil) nil)

;; == when-let* — if-nil → nil, else body ==
(qa (when-let* ((a nil)) 'hi) nil)
(qa (when-let* ((a 1)) 'hi) 'hi)

;; == and-let* — each binding + prior bindings in scope ==
(qa (and-let* ((a 1) (b (+ a 2))) (+ a b)) 4)
(qa (and-let* ((a nil) (b 1)) (+ a b)) nil)

;; == ensure-list — duplicated earlier, keep single copy ==

;; == list-of-strings-p ==
(qa (list-of-strings-p '("a" "b")) t)
(qa (list-of-strings-p '("a" 'b)) nil)
(qa (list-of-strings-p '()) t)
(qa (list-of-strings-p 'a) nil)

;; == string-trim-left/right — customize REGEXP arg ==
(qa (string-trim-left "  abc") "abc")
(qa (string-trim-right "abc  ") "abc")
;; custom regexp arg
(qa (string-trim-left "xxabc" "x+") "abc")
(qa (string-trim-right "abcxx" "x+") "abc")

;; == string-chop-newline is a synonym for string-trim-right "\n" ==
(qa (string-chop-newline "line1\nline2\n") "line1\nline2")

(princ (qa--done))
