;; String library surface — GNU 31.1 -Q --batch pins:
;;   - `string-distance' = Levenshtein character distance.
;;   - `string-lines' splits on \n: signature (STRING &optional OMIT-EMPTY
;;     KEEP-NEWLINES); trailing \n is dropped; "\n" gives (""); nil → nil.
;;   - `string-pad' pads, never truncates; negative LENGTH errors.
;;   - `string-limit' TRUNCATES to N characters.
;;   - `string-clean-whitespace' collapses whitespace runs to one space,
;;     trims both ends.
;;   - `string-join' SEPARATOR is optional — nil just concatenates.
;;   - `string-chop-newline' removes exactly ONE trailing newline.
;;   - `truncate-string-to-width' takes (STR END-COLUMN &optional
;;     START-COLUMN PADDING ELLIPSIS ELLIPSIS-TEXT-PROPERTY): START-COLUMN
;;     selects a column window, PADDING pads short strings, ELLIPSIS (a
;;     string, or t for truncate-string-ellipsis) replaces the tail.
;;   - `string-blank-p' returns 0 (a NUMBER, not t) for blank input.
;;   - `string-as-unibyte'/`string-to-unibyte' return unibyte strings;
;;     `multibyte-string-p' is nil on a plain "abc" under -Q --batch.
;;   - `unibyte-string' builds a raw unibyte string from byte values.
;;   - `string-remove-prefix' returns the original string on no match.
;;   - `string-equal' IGNORES case-fold-search (always case-sensitive).
(require 'subr-x) ;; string-join, string-remove-prefix, string-chop-newline

;; == string-distance ==
(qa (string-distance "abc" "abc") 0)
(qa (string-distance "abc" "abd") 1)
(qa (string-distance "abc" "") 3)
(qa (string-distance "" "abc") 3)
(qa (string-distance "kitten" "sitting") 3)
(qa (string-distance "abc" "abcd") 1)
(qa (string-distance "abc" "ac") 1)
(qa (string-distance "abc" "xyz") 3)
(qa (string-distance "" "") 0)
(qa (string-distance "ABC" "abc") 3)
;; optional BYTECOMPARE flag counts bytes
(qa (string-distance "abc" "abd" nil) 1)
(qa (string-distance "abc" "abd" t) 1)

;; == string-lines ==
(qa (string-lines "a\nb\nc") '("a" "b" "c"))
(qa (string-lines "a\nb\nc\n") '("a" "b" "c"))
(qa (string-lines "a\n\nb") '("a" "" "b"))
(qa (string-lines "") '(""))
(qa (string-lines "a") '("a"))
(qa (string-lines "\n") '(""))
(qa (string-lines "a\n") '("a"))
;; OMIT-EMPTY drops empty elements
(qa (string-lines "a\n\nb" t) '("a" "b"))
(qa (string-lines "\n" t) nil)
;; KEEP-NEWLINES keeps \n on every non-final line
(qa (string-lines "a\nb" nil t) '("a\n" "b"))
(qa (string-lines "a\n\nb" t t) '("a\n" "b"))
;; nil input → nil (no error)
(qa (string-lines nil) nil)

;; == string-pad ==
(qa (string-pad "abc" 5) "abc  ")
(qa (string-pad "abc" 5 ?*) "abc**")
(qa (string-pad "abc" 5 ?* t) "**abc")
(qa (string-pad "abc" 3) "abc")
(qa (string-pad "abc" 2) "abc") ;; never truncates
(qa (string-pad "" 3 ?x) "xxx")
(qa (string-pad "abc" 0) "abc")
(qa-err (string-pad "abc" -1) 'wrong-type-argument)

;; == string-limit ==
(qa (string-limit "abcdef" 3) "abc")
(qa (string-limit "abc" 10) "abc")
(qa (string-limit "abc" 3) "abc")
(qa (string-limit "" 5) "")
(qa (string-limit "abcdef" 0) "")
(qa (string-limit "abcdef" 6) "abcdef")

;; == string-clean-whitespace ==
(qa (string-clean-whitespace "  a  b  c  ") "a b c")
(qa (string-clean-whitespace "a\tb\nc") "a b c")
(qa (string-clean-whitespace "  x  ") "x")
(qa (string-clean-whitespace "") "")
(qa (string-clean-whitespace "   ") "")
(qa (string-clean-whitespace "a") "a")
(qa (string-clean-whitespace "a b") "a b")
(qa (string-clean-whitespace "a\t\tb\n\nc") "a b c")

;; == string-join ==
(qa (string-join '("a" "b" "c") "-") "a-b-c")
(qa (string-join '("a") "-") "a")
(qa (string-join nil "-") "")
(qa (string-join '("a" "b")) "ab")
(qa (string-join '("a" "b") ", ") "a, b")
(qa (string-join '("" "") "-") "-")

;; == string-remove-prefix ==
(qa (string-remove-prefix "foo" "foobar") "bar")
(qa (string-remove-prefix "x" "foobar") "foobar")
(qa (string-remove-prefix "" "abc") "abc")
(qa (string-remove-prefix "foobar" "foo") "foo")
(qa (string-remove-prefix "Foo" "foobar") "foobar") ;; case-sensitive

;; == string-chop-newline ==
(qa (string-chop-newline "abc\n") "abc")
(qa (string-chop-newline "abc") "abc")
(qa (string-chop-newline "abc\n\n") "abc\n") ;; only ONE removed
(qa (string-chop-newline "") "")
(qa (string-chop-newline "\n") "")

;; == truncate-string-to-width ==
;; (STR END-COLUMN &optional START-COLUMN PADDING ELLIPSIS …)
(qa (truncate-string-to-width "abcdef" 3) "abc")
(qa (truncate-string-to-width "abc" 10) "abc")
;; ELLIPSIS replaces the tail and consumes part of END-COLUMN's budget.
(qa (truncate-string-to-width "abcdef" 4 0 nil "...") "a...")
(let ((char-width-table (copy-sequence char-width-table)))
  ;; Pin ambiguous-width ellipsis explicitly for every environment.
  (set-char-table-range char-width-table ?… 2)
  (qa (truncate-string-to-width "abcdef" 4 0 nil "…") "ab…"))
;; ELLIPSIS non-nil non-string → "…" (truncate-string-ellipsis is nil -Q)
(let ((char-width-table (copy-sequence char-width-table)))
  (set-char-table-range char-width-table ?… 2)
  (qa (truncate-string-to-width "abcdef" 5 0 nil t) "abc…")
  (qa (truncate-string-to-width "abcdef" 4 0 nil t) "ab…"))
;; ELLIPSIS ignored when nothing is truncated
(qa (truncate-string-to-width "abc" 5 0 nil "...") "abc")

;; == unibyte / multibyte ==
(qa (string-as-unibyte "abc") "abc")
(qa (string-as-multibyte "abc") "abc")
(qa (multibyte-string-p "abc") nil)
(qa (multibyte-string-p (string-as-unibyte "abc")) nil)
(qa (multibyte-string-p (string-as-multibyte (string-as-unibyte "abc"))) t)
(qa (string-to-unibyte "abc") "abc")
(qa (multibyte-string-p (string-to-unibyte "abc")) nil)
(qa (string-to-multibyte (string-to-unibyte "abc")) "abc")

;; == unibyte-string ==
(qa (unibyte-string 97 98 99) "abc")
(qa (multibyte-string-p (unibyte-string 97 98 99)) nil)
(qa (unibyte-string) "")
(qa (length (unibyte-string 1 2 3 4)) 4)

;; == string-equal-ignore-case ==
(qa (string-equal-ignore-case "A" "a") t)
(qa (string-equal-ignore-case "AbC" "aBc") t)
(qa (string-equal-ignore-case "a" "b") nil)
(qa (string-equal-ignore-case "" "") t)
(qa-err (string-equal-ignore-case "a" "A" t) 'wrong-number-of-arguments)

;; == assoc-string ==
(qa (assoc-string "B" '(("b" . 2)) t) '("b" . 2))
(qa (assoc-string "B" '(("b" . 2))) nil)
(qa (car (assoc-string "B" '(("b" . 2)) t)) "b")

;; == string-lessp ==
;; case-fold-search DOES affect string-lessp? — GNU: no; string-lessp is
;; char-code order regardless of case-fold-search
(qa (let ((case-fold-search t)) (string-lessp "B" "a")) t)   ;; B(66) < a(97)
(qa (let ((case-fold-search nil)) (string-lessp "B" "a")) t)
(qa (string-lessp "a" "b") t)
(qa (string-lessp "b" "a") nil)
(qa (string-lessp "a" "a") nil)
(qa (string-lessp "ab" "abc") t)  ;; prefix sorts first
(qa (string-lessp "" "a") t)

;; == string> ==
(qa (string> "b" "a") t)
(qa (string> "a" "b") nil)
(qa (string> "a" "a") nil)

;; == string-prefix-p / string-suffix-p / string-match-p ==
(qa (string-prefix-p "foo" "foobar") t)
(qa (string-prefix-p "bar" "foobar") nil)
(qa (string-prefix-p "" "abc") t)
(qa (string-prefix-p "abc" "") nil)
(qa (string-prefix-p "foo" "foo") t)
(qa (string-prefix-p "FOO" "foobar" t) t) ;; IGNORE-CASE
(qa (string-suffix-p "bar" "foobar") t)
(qa (string-suffix-p "foo" "foobar") nil)
(qa (string-suffix-p "" "abc") t)
(qa (string-suffix-p "bar" "bar") t)
(qa (string-suffix-p "BAR" "foobar" t) t)
;; string-match-p: like string-match but no match-data clobber
(qa (string-match-p "b+" "aaabbb") 3)
(qa (string-match-p "z" "abc") nil)
(qa (let ((case-fold-search t)) (string-match-p "B" "abc")) 1)
(qa (let ((case-fold-search nil)) (string-match-p "B" "abc")) nil)

;; == string-blank-p — returns 0, not t, on blank input ==
(qa (string-blank-p "") 0)
(qa (string-blank-p "   ") 0)
(qa (string-blank-p " \t\n ") 0)
(qa (string-blank-p " a ") nil)
(qa (string-blank-p "a") nil)

;; == string-fill exists (display-width justification) ==
(qa (fboundp 'string-fill) t)

;; == string-pad rejects a string PADDING ==
(qa-err (string-pad "abc" 5 "xy") 'wrong-type-argument)

;; == string-equal ignores case-fold-search ==
(qa (let ((case-fold-search t)) (string-equal "A" "a")) nil)
(qa (let ((case-fold-search nil)) (string-equal "A" "a")) nil)
(qa (string-equal "abc" "abc") t)

;; == compare-strings ==
(qa (compare-strings "abc" 0 nil "abc" 0 nil) t)
(qa (compare-strings "abc" 0 nil "abd" 0 nil) -3)
(qa (compare-strings "abd" 0 nil "abc" 0 nil) 3)
(qa (compare-strings "abc" 1 nil "xbc" 1 nil) t)
(qa (compare-strings "ABC" 0 nil "abc" 0 nil t) t)

;; == string-replace (subr-x) ==
(qa (string-replace "o" "0" "foo") "f00")
(qa (string-replace "z" "0" "foo") "foo")
;; empty FROM signals wrong-length-argument (FROM must be non-empty)
(qa-err (string-replace "" "-" "ab") 'wrong-length-argument)
(qa (string-replace "aa" "b" "aaaa") "bb")

;; == string-trim ==
(qa (string-trim "  abc  ") "abc")
(qa (string-trim "abc") "abc")
(qa (string-trim "xxabcxx" "x*" "x*") "abc")
(qa (string-trim-left "  abc") "abc")
(qa (string-trim-right "abc  ") "abc")

;; == string-width / string-pixel-width availability ==
(qa (string-width "abc") 3)
(qa (string-width "") 0)

;; == case-fold-search affects regexp search, not literal comparisons ==
(qa (let ((case-fold-search t)) (string-match "B" "abc")) 1)
(qa (let ((case-fold-search nil)) (string-match "B" "abc")) nil)

(princ (qa--done))
