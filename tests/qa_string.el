;; String functions — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - concat rejects bare integers and symbols (wrong-type-argument);
;;     accepts strings, char lists, vectors of chars, nil.
;;   - (split-string "" " ") => (""), NOT nil.
;;   - string-blank-p returns the match index 0, not t.
;;   - string< / string-lessp are case-sensitive: "a" < "B" is nil,
;;     "A" < "a" is t.  string-equal-ignore-case exists.
;;   - replace-regexp-in-string REPLACEMENT may be a function.
;;   - upcase-initials vs capitalize differ on mid-word caps.
;;   - substring accepts negative indices (from end); out-of-range
;;     signals args-out-of-range.

;; == concat ==
(qa (concat "foo" "bar") "foobar")
(qa (concat) "")
(qa (concat "a" nil "b") "ab")
(qa (concat "a" '() [98] "c") "abc")
(qa (concat "a" '(98 99)) "abc")
(qa (concat [97 98]) "ab")
(qa-err (concat 97) 'wrong-type-argument)
(qa-err (concat 'a "b") 'wrong-type-argument)

;; == substring ==
(qa (substring "hello" 1 3) "el")
(qa (substring "hello" 2) "llo")
(qa (substring "abc" nil nil) "abc")
(qa (substring "hello" 0 -1) "hell")
(qa (substring "hello" -3) "llo")
(qa-err (substring "hi" 0 5) 'args-out-of-range)
(qa (substring-no-properties "abc" 1) "bc")
(qa (let ((s (copy-sequence "abc")))
      (put-text-property 0 1 'face 'bold s)
      (get-text-property 0 'face (substring-no-properties s 0 2)))
    nil)
(qa (let ((s (copy-sequence "abc")))
      (put-text-property 0 1 'face 'bold s)
      (get-text-property 0 'face (substring s 0 2)))
    'bold)

;; == comparison ==
(qa (string= "abc" "abc") t)
(qa (string= "abc" "abd") nil)
(qa (string= 'a "a") t)
(qa (string-equal "ABC" "abc") nil)
(qa (string-equal-ignore-case "ABC" "abc") t)
(qa (string< "a" "ab") t)
(qa (string< "ab" "a") nil)
(qa (string< "A" "a") t)
(qa (string< "a" "B") nil)
(qa (string< 'a 'b) t)
(qa (string-lessp "abc" "abd") t)
(qa (string-greaterp "b" "a") t)
(qa (string-version-lessp "1.9" "1.10") t)
(qa (string-version-lessp "1.10" "1.9") nil)
(qa (compare-strings "abc" nil nil "abc" nil nil) t)
(qa (compare-strings "abc" nil nil "abd" nil nil) -3)
(qa (string-empty-p "") t)
(qa (string-empty-p "x") nil)
(qa (string-blank-p " \t") 0)
(qa (string-blank-p " x ") nil)
(qa-err (string-blank-p nil) 'wrong-type-argument)

;; == split / join ==
(qa (split-string "a,b,c" ",") '("a" "b" "c"))
(qa (split-string "a1b22c" "[0-9]+") '("a" "b" "c"))
(qa (split-string "a b  c") '("a" "b" "c"))
(qa (split-string "" " ") '(""))
(qa (split-string "  a b  " " " t) '("a" "b"))
(qa (split-string "xaXbbYc" "[XY]" nil "x+") '("" "a" "bb" "c"))
(qa (string-join '("a" "b" "c") "-") "a-b-c")
(qa (string-join '() ",") "")
(qa (string-lines "a\nb\n") '("a" "b"))
(qa (string-lines "a\n\nb\n" t) '("a" "b"))
(qa (string-chop-newline "a\n") "a")
(qa (string-remove-prefix "foo" "foobar") "bar")
(qa (string-remove-prefix "x" "foobar") "foobar")
(qa (string-remove-suffix "bar" "foobar") "foo")

;; == trim ==
(qa (string-trim "  hi  ") "hi")
(qa (string-trim "--hi--" "-+" "-+") "hi")
(qa (string-trim-left "  hi  ") "hi  ")
(qa (string-trim-right "  hi  ") "  hi")
(qa (string-trim "xxhixx" "x+") "hixx")

;; == replace-regexp-in-string ==
(qa (replace-regexp-in-string "a" "X" "banana") "bXnXnX")
(qa (replace-regexp-in-string "\\(a\\)" "\\1\\1" "aba") "aabaa")
(qa (replace-regexp-in-string "a" "\\&" "aba" t t) "\\&b\\&")
(qa (replace-regexp-in-string "a" (lambda (m) (upcase m)) "banana")
    "bAnAnA")
(qa (replace-regexp-in-string "A" "x" "aAa" nil t) "xXx")
(qa (replace-regexp-in-string "a" "\\\\" "aXa" t) "\\X\\")
(qa (string-match "b+c" "abbcd") 1)
(qa (string-match "b+c" "abbcd" 2) 2)
(qa (string-match "xyz" "abc") nil)

;; == case ==
(qa (upcase "abc") "ABC")
(qa (upcase "a1b") "A1B")
(qa (upcase ?a) 65)
(qa (downcase "AbC") "abc")
(qa (downcase ?A) 97)
(qa (capitalize "hELLO tHERE") "Hello There")
(qa (upcase-initials "hELLO tHERE") "HELLO THERE")
(qa (capitalize ?a) 65)
(qa (capitalize "1abc def") "1abc Def")

;; == construction / conversion ==
(qa (string ?a ?b ?c) "abc")
(qa (make-string 3 ?x) "xxx")
(qa (string-pad "ab" 5) "ab   ")
(qa (string-pad "ab" 5 ?\s t) "   ab")
(qa (string-to-vector "ab") [97 98])
(qa (string-to-char "abc") 97)
(qa (string-to-char "") 0)
(qa (char-to-string ?x) "x")
(qa (reverse "abc") "cba")
(qa (length "hello") 5)
(qa (elt "abc" 1) 98)
(qa (aref "abc" 1) 98)
(qa (string-width "abc") 3)
(qa (string-distance "kitten" "sitting") 3)
(qa (string-glyph-split "abc") '("a" "b" "c"))
(qa (multibyte-string-p "abc") nil)
(qa (assoc-string "b" '(("a" . 1) ("b" . 2))) '("b" . 2))
(qa (assoc-string "B" '(("a" . 1) ("b" . 2)) t) '("b" . 2))
(qa (assoc-string "B" '(("a" . 1) ("b" . 2))) nil)

(princ (qa--done))
