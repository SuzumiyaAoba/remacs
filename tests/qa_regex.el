;; GNU-verified regexp coverage: string-match anchors/classes/repetition,
;; match data, replace-*, buffer searching, regexp-quote/opt, split-string.
;; Pinned GNU 31.1 quirks: `case-fold-search' defaults t (affects literal
;; AND char-class case); `\,'/`\#' in replace-regexp-in-string signal plain
;; `error' in batch; `^'/`$' match around embedded newlines while
;; \`/\' anchor the whole string; posix-* prefer longest match;
;; string-match-p/looking-at-p do NOT touch match-data;
;; `regexp-unmatchable' = "\\`a\\`".

;; == string-match basics / START arg ==
(qa (string-match "b" "abc") 1)
(qa (string-match "z" "abc") nil)
(qa (string-match "" "abc") 0)
(qa (string-match "" "abc") 0)
(qa (string-match "b" "abcabc" 2) 4)
(qa (progn (string-match "b" "abcabc" 2) (match-data)) '(4 5))
(qa (string-match "a" "abc" 3) nil)
(qa (string-match "a" "abc" -1) nil)
(qa-err-data (string-match "a" "abc" 4) 'args-out-of-range '("abc" 4))
(qa-err-data (string-match "a" 5) 'wrong-type-argument '(stringp 5))
(qa-err (string-match nil "abc") 'wrong-type-argument)

;; == anchors: `^'/`$' vs \`/\' with newlines ==
(qa (string-match "^b" "a\nb") 2)          ; ^ matches after \n
(qa (string-match "^b" "ba\nb") 0)
(qa (string-match "a$" "a\nb") 0)          ; $ matches before \n
(qa (string-match "a$" "ba\n") 1)
(qa (string-match "$" "ab\n") 2)
(qa (string-match "\\`a" "a\nb") 0)
(qa (string-match "\\`b" "a\nb") nil)
(qa (string-match "b\\'" "a\nb") 2)
(qa (string-match "b\\'" "a\nb\n") nil)
(qa (string-match "\\'" "a\nb") 3)
(qa (string-match "\\`\\'" "") 0)
(qa (string-match "\\`\\'" "a") nil)
;; \= is the point anchor: always nil under string-match (no point)
(qa (string-match "\\=" "abc") nil)
(qa (string-match "\\=a" "abc") nil)

;; == . never matches newline; bracket does ==
(qa (string-match "a.b" "a\nb") nil)
(qa (string-match "a.b" "axb") 0)

;; == alternation / groups / shy groups ==
(qa (string-match "foo\\|bar" "xbar") 1)
(qa (string-match "a\\|" "ba") 0)
(qa (progn (string-match "a\\|" "ba") (match-string 0 "ba")) "")
(qa (string-match "\\(a\\)\\(b\\)" "ab") 0)
(qa (progn (string-match "\\(a\\)\\(b\\)" "ab") (match-data)) '(0 2 0 1 1 2))
(qa (string-match "\\(a\\(b\\)\\)\\(c\\)" "abc") 0)
(qa (progn (string-match "\\(a\\(b\\)\\)\\(c\\)" "abc") (match-data))
    '(0 3 0 2 1 2 2 3))
;; shy groups don't consume a group number
(qa (progn (string-match "\\(?:a\\)\\(b\\)" "ab") (match-string 1 "ab")) "b")
(qa (progn (string-match "\\(?:x\\|ab\\)" "ab") (match-string 0 "ab")) "ab")
;; explicitly numbered groups
(qa (progn (string-match "\\(?1:a\\)\\(?2:b\\)" "ab") (match-string 2 "ab")) "b")
(qa (regexp-opt-depth "\\(a\\)") 1)
(qa (regexp-opt-depth "\\(?:a\\)") 0)
(qa (regexp-opt-depth "\\(a\\(b\\)\\)") 2)
;; unmatched subgroup leaves holes in match-data
(qa (progn (string-match "a\\(b\\)?" "xa") (match-data)) '(1 2))
(qa (progn (string-match "a\\(b\\)?" "xa") (match-beginning 1)) nil)
(qa (string-match "a[^x]b" "a\nb") 0)      ; [^x] DOES match newline

;; == repetition ==
(qa (progn (string-match "a*" "aaab") (match-string 0 "aaab")) "aaa")
(qa (string-match "x*" "abc") 0)
(qa (progn (string-match "x*" "abc") (match-string 0 "abc")) "")
(qa (progn (string-match "a+b" "aaab") (match-string 0 "aaab")) "aaab")
(qa (string-match "a?" "bbb") 0)
(qa (progn (string-match "a\\{2\\}" "aaa") (match-string 0 "aaa")) "aa")
(qa (progn (string-match "a\\{2,\\}" "aaaa") (match-string 0 "aaaa")) "aaaa")
(qa (progn (string-match "a\\{2,3\\}" "aaaa") (match-string 0 "aaaa")) "aaa")
(qa (string-match "a\\{2,3\\}" "a") nil)
(qa (progn (string-match "a\\{,3\\}" "aaa") (match-string 0 "aaa")) "aaa")
;; lazy operators prefer the shortest match
(qa (progn (string-match "a*?" "aaa") (match-string 0 "aaa")) "")
(qa (progn (string-match "a+?" "aaa") (match-string 0 "aaa")) "a")
(qa (progn (string-match "a??" "aaa") (match-string 0 "aaa")) "")
(qa (progn (string-match "a\\{1\\}?" "aa") (match-string 0 "aa")) "a")
(qa (progn (string-match "\\(a+?\\)" "aaa") (match-string 1 "aaa")) "a")
;; group repetition: subexp records the LAST iteration
(qa (progn (string-match "\\(a\\)+b" "aab") (match-string 1 "aab")) "a")
(qa (progn (string-match "\\(a\\)\\{2\\}" "aa") (match-string 1 "aa")) "a")

;; == backreferences ==
(qa (string-match "\\(a+\\)b\\1" "aabaa") 0)
(qa (progn (string-match "\\(a+\\)b\\1" "aabaa") (match-data)) '(0 5 0 2))
(qa (string-match "\\(a\\|b\\)\\1" "ab") nil)
(qa (progn (string-match "\\(a\\|b\\)\\1" "abba") (match-string 0 "abba")) "bb")
(qa-err (string-match "\\1" "aa") 'invalid-regexp)

;; == char classes ==
(qa (progn (string-match "[a-z]+" "12abcXY") (match-string 0 "12abcXY")) "abcXY")
(qa (progn (string-match "[^a-z]+" "ab12cd") (match-string 0 "ab12cd")) "12")
(qa (progn (string-match "[-a]+" "b-a-") (match-string 0 "b-a-")) "-a-")
(qa (progn (string-match "[]a]+" "b]a") (match-string 0 "b]a")) "]a")
(qa (progn (string-match "[a^]+" "b^a") (match-string 0 "b^a")) "^a")
(qa (string-match "\\[a\\]" "[a]") 0)
(qa (string-match "a\\*b" "a*b") 0)
;; POSIX classes (case-fold-search is t by default: [:upper:]/[:lower:]
;; fold case; pin both foldings)
(qa (progn (string-match "[[:alpha:]]+" "_ab12") (match-string 0 "_ab12")) "ab")
(qa (progn (string-match "[[:digit:]]+" "ab123") (match-string 0 "ab123")) "123")
(qa (progn (string-match "[[:alnum:]]+" "!ab12_") (match-string 0 "!ab12_")) "ab12")
(qa (progn (string-match "[[:word:]]+" "-ab_") (match-string 0 "-ab_")) "ab")
(qa (progn (string-match "[[:xdigit:]]+" "g1fZ") (match-string 0 "g1fZ")) "1f")
(qa (progn (string-match "[[:punct:]]+" "a!b") (match-string 0 "a!b")) "!")
(qa (progn (string-match "[[:blank:]]+" "a \tb") (match-string 0 "a \tb")) " \t")
(qa (progn (string-match "[[:ascii:]]+" "ab") (match-string 0 "ab")) "ab")
(qa (progn (string-match "[[:upper:]]+" "abAB") (match-string 0 "abAB")) "abAB")
(qa (let ((case-fold-search nil))
        (string-match "[[:upper:]]+" "abAB"))
    2)
(qa (let ((case-fold-search nil))
        (progn (string-match "[[:lower:]]+" "ABab") (match-string 0 "ABab")))
    "ab")
(qa-err (string-match "[[:foo:]]" "x") 'invalid-regexp)
(qa-err (string-match "[" "x") 'invalid-regexp)
(qa-err (string-match "\\(" "x") 'invalid-regexp)
(qa-err (string-match "a\\{2,1\\}" "x") 'invalid-regexp)

;; == case-fold-search ==
(qa (string-match "B" "abc") 1)            ; fold t by default
(qa (let ((case-fold-search nil)) (string-match "B" "abc")) nil)
(qa (let ((case-fold-search nil))
        (progn (string-match "[a-z]+" "abAB") (match-string 0 "abAB")))
    "ab")

;; == syntax classes / boundaries (standard syntax table) ==
(qa (string-match "\\sw" "a-b") 0)
(qa (string-match "\\s_" "a-b") 1)         ; `-' is symbol constituent
(qa (string-match "\\s-" "a b") 1)
(qa (string-match "\\s'" "a'b") 1)
(qa (string-match "\\s." "a.b") nil)       ; `.' is not punctuation class
(qa (string-match "\\Sw" "-") 0)
(qa (progn (string-match "\\w+" "foo-bar") (match-string 0 "foo-bar")) "foo")
(qa (string-match "\\ca" "a") 0)
(qa (string-match "\\ca" "λ") nil)
(qa (string-match "\\cc" "λ") 0)
(qa (string-match "\\Cc" "λ") nil)
(qa (string-match "\\bbar" "foo bar") 4)
(qa (string-match "\\boo" "foo bar") nil)
(qa (string-match "o\\B" "foo bar") 1)
(qa (progn (string-match "\\b" "abc") (match-string 0 "abc")) "")
(qa (progn (string-match "\\B" "abc") (match-beginning 0)) 1)
(qa (string-match "\\<bar" "foo bar") 4)
(qa (string-match "\\<oo" "foo bar") nil)
(qa (string-match "o\\>" "foo") 2)
(qa (progn (string-match "\\<\\w+\\>" "foo-bar") (match-string 0 "foo-bar")) "foo")
;; symbol boundaries: `_' is a symbol constituent, so `a_b' is one symbol
(qa (string-match "\\_<foo" "foo_bar") 0)
(qa (string-match "foo\\_>" "foo_bar") nil)
(qa (string-match "\\_<a\\_>" "a-b a") 4)
(qa (string-match "\\_<a\\_>" "a_b a") 4)
(qa (string-match "\\_<\\w+\\_>" "a1_b") nil)

;; == match-data accessors ==
(qa (progn (string-match "\\(a\\)b" "xabc") (match-data)) '(1 3 1 2))
(qa (progn (string-match "\\(a\\)b" "xabc") (match-beginning 0)) 1)
(qa (progn (string-match "\\(a\\)b" "xabc") (match-end 1)) 2)
(qa (progn (string-match "\\(a\\)b" "xabc") (match-string 0 "xabc")) "ab")
(qa (progn (string-match "\\(a\\)b" "xabc")
           (match-string-no-properties 1 "xabc"))
    "a")
;; failed match leaves previous match-data untouched
(qa (progn (string-match "\\(a\\)" "ba") (string-match "q" "abc")
           (match-data))
    '(1 2 1 2))
(qa (progn (string-match "\\(b\\)" "abc") (string-match "q" "abc")
           (match-data))
    '(1 2 1 2))
(qa (progn (string-match "\\(a\\)" "a") (match-beginning 5)) nil)
(qa-err (progn (string-match "\\(a\\)" "a") (match-beginning -1))
        'args-out-of-range)
;; match-string without STRING arg after a string match errors (it would
;; look in the current buffer)
(qa-err (progn (string-match "b" "abc") (match-string 0)) 'args-out-of-range)
;; string-match-p / looking-at-p do not alter match-data
(qa (progn (set-match-data '(5 6))
           (list (string-match-p "b" "abc") (match-beginning 0)))
    '(1 5))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (set-match-data nil)
      (list (looking-at-p "ab") (match-beginning 0)))
    '(t nil))
;; save-match-data restores match data afterwards
(qa (progn (set-match-data '(5 6))
           (list (save-match-data
                   (string-match "c" "abc")
                   (match-beginning 0))
                 (match-beginning 0)))
    '(2 5))
(qa (progn (set-match-data nil) (match-end 0)) nil)

;; == match-substitute-replacement (STRING arg required after string-match) ==
(qa (progn (string-match "\\(b\\)" "abc")
           (match-substitute-replacement "X" nil nil "abc"))
    "X")
(qa (progn (string-match "\\(b\\)" "abc")
           (match-substitute-replacement "<\\&>" nil nil "abc"))
    "<b>")
(qa (progn (string-match "\\(b\\)" "abc")
           (match-substitute-replacement "<\\1>" nil nil "abc"))
    "<b>")
(qa (progn (string-match "\\(b\\)" "abc")
           (match-substitute-replacement "x" nil t "abc"))
    "x")
;; \# and \, are not expanded in batch replace paths
(qa-err (progn (string-match "\\(b\\)" "abc")
               (match-substitute-replacement "\\#" nil nil "abc"))
        'error)
(qa-err (progn (string-match "\\(b\\)" "abc")
               (match-substitute-replacement "\\,(upcase \\1)" nil nil "abc"))
        'error)

;; == replace-match ==
(qa (progn (string-match "\\(b\\)" "abc")
           (replace-match "<\\1>" nil nil "abc"))
    "a<b>c")
(qa (progn (string-match "\\(b\\)c" "abc")
           (replace-match "X" nil nil "abc" 1))
    "aXc")
;; case transference: replacement inherits matched case unless FIXEDCASE
(qa (let ((case-fold-search t))
      (string-match "def" "aDEFb")
      (replace-match "xyz" nil nil "aDEFb"))
    "aXYZb")
(qa (let ((case-fold-search t))
      (string-match "def" "aDEFb")
      (replace-match "xyz" t nil "aDEFb"))
    "axyzb")
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (re-search-forward "\\(b\\)")
      (list (replace-match "<\\1>") (buffer-string)))
    '(nil "a<b>c"))

;; == replace-regexp-in-string ==
(qa (replace-regexp-in-string "b" "X" "abc") "aXc")
(qa (replace-regexp-in-string "\\(b\\)" "<\\1>" "abc") "a<b>c")
(qa (replace-regexp-in-string "b" "\\&\\&" "abc") "abbc")
(qa (replace-regexp-in-string "b" "x\\&y" "abc" nil t) "ax\\&yc") ; LITERAL
(qa (replace-regexp-in-string "b" "x\\&y" "abc" t nil) "axbyc")   ; FIXEDCASE
(qa (replace-regexp-in-string "a" #'upcase "banana") "bAnAnA")
(qa (replace-regexp-in-string "\\(a\\)" (lambda (m) (concat "<" m ">")) "banana")
    "b<a>n<a>n<a>")
(qa (replace-regexp-in-string "B" "X" "abc") "aXc")
(qa (let ((case-fold-search nil))
      (replace-regexp-in-string "B" "X" "abc"))
    "abc")
;; START trims the result to the substring from START
(qa (replace-regexp-in-string "a" "X" "banana" nil nil nil 3) "XnX")
(qa (replace-regexp-in-string "\\(a\\)" "<\\1>" "banana" nil nil nil 3)
    "<a>n<a>")
(qa (replace-regexp-in-string "^" "|" "abc") "|abc")
(qa (replace-regexp-in-string "\\`" "|" "abc") "|abc")
(qa (replace-regexp-in-string "$" "|" "a\nb") "a|\nb|")
(qa (replace-regexp-in-string "x*" "-" "abc") "-a-b-c")
(qa (replace-regexp-in-string "b*" "-" "abc") "-a--c")
(qa (replace-regexp-in-string "\\\\" "\\\\" "a\\b") "a\\b")
(qa-err (replace-regexp-in-string "a" "\\#" "banana") 'error)
(qa-err (replace-regexp-in-string "a" "\\,(upcase \\&)" "banana") 'error)
(qa-err (replace-regexp-in-string "b" '("X") "abc") 'invalid-function)

;; == buffer: replace-regexp ==
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (let ((inhibit-message t)) (replace-regexp "b" "X"))
      (buffer-string))
    "aXc aXc")
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (let ((inhibit-message t)) (replace-regexp "b" "X"))))
    '(nil))

;; == buffer: looking-at / looking-back / \= ==
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (list (looking-at "ab") (looking-at "c")))
    '(t nil))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (looking-at "a\\(b\\)")
      (list (match-beginning 0) (match-end 0)
            (match-beginning 1) (match-end 1)))
    '(1 3 2 3))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (looking-at "ab")
      (markerp (car (match-data))))
    t)
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (looking-at "ab")
      (mapcar #'marker-position (match-data)))
    '(1 3))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (looking-at "a")
      (match-string 0))
    "a")
(qa (with-temp-buffer
      (insert "ax")
      (goto-char (point-min))
      (looking-at "a\\(b\\)?x")
      (list (match-beginning 1) (match-string-no-properties 1)))
    '(nil nil))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (list (looking-at "\\=") (looking-at "\\=b" )))
    '(t nil))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 2)
      (list (looking-at "\\=b") (looking-at "a\\=")))
    '(t nil))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 3)
      (list (looking-back "b" nil nil) (looking-back "z" nil nil)))
    '(t nil))
(qa (with-temp-buffer
      (insert "xabc")
      (goto-char (point-max))
      (looking-back "xabc" 3 nil))
    nil)
;; looking-back: default = shortest match, GREEDY = longest
(qa (with-temp-buffer
      (insert "abbc")
      (goto-char (point-max))
      (list (looking-back "b*c" nil nil) (match-beginning 0)))
    '(t 4))
(qa (with-temp-buffer
      (insert "abbc")
      (goto-char (point-max))
      (list (looking-back "b*c" nil t) (match-beginning 0)))
    '(t 2))
(qa-err (with-temp-buffer
          (insert "x")
          (goto-char (point-min))
          (looking-at "\\("))
        'invalid-regexp)

;; == buffer: re-search-forward/-backward ==
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (list (re-search-forward "b") (point)))
    '(3 3))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (re-search-forward "c" nil nil 2) (point)))
    '(8 8))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (re-search-forward "bc" 4 t) (point)))
    '(4 4))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (re-search-forward "abc" 3 t) (point)))
    '(nil 1))
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char (point-min))
          (re-search-forward "zzz"))
        'search-failed)
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (re-search-forward "zzz" nil t))
    nil)
(qa (with-temp-buffer
      (insert "ab")
      (goto-char (point-min))
      (list (re-search-forward "") (point)))
    '(1 1))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char (point-min))
      (list (re-search-forward "a*") (point)))
    '(2 2))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-max))
      (list (re-search-backward "b" nil t) (point)))
    '(2 2))
;; Emacs takes the FIRST matching alternative; POSIX takes the LONGEST
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (re-search-forward "b\\|bc")
      (match-string 0))
    "b")
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (posix-search-forward "b\\|bc")
      (match-string 0))
    "bc")
(qa (with-temp-buffer
      (insert "axbyc")
      (goto-char (point-max))
      (posix-search-backward "b\\|ab"))
    3)

;; == posix-string-match ==
(qa (progn (posix-string-match "a\\|ab" "ab") (match-string 0 "ab")) "ab")
(qa (progn (string-match "a\\|ab" "ab") (match-string 0 "ab")) "a")
(qa (progn (posix-string-match "\\(a\\|ab\\)\\(c\\|bcd\\)" "zabcd")
           (match-data))
    '(1 5 1 2 2 5))
(qa (posix-string-match "B" "abc") 1)
(qa (let ((case-fold-search nil)) (posix-string-match "B" "abc")) nil)

;; == buffer: literal search-forward/-backward ==
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (search-forward "bc") (point)))
    '(4 4))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (search-forward "c" nil nil 2) (point)))
    '(8 8))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-min))
      (search-forward "zz" nil t))
    nil)
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char (point-min))
          (search-forward "zz"))
        'search-failed)
;; BOUND limits where the match may END
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (search-forward "bc" 4 t) (point)))
    '(4 4))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (search-forward "abc" 3 t) (point)))
    '(nil 1))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (search-forward "a" nil t -1) (point)))
    '(nil 1))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char (point-min))
      (list (search-forward "") (point)))
    '(1 1))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-max))
      (list (search-backward "a") (point)))
    '(1 1))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-max))
      (list (search-backward "a" 5 t) (point)))
    '(5 5))
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char (point-min))
          (search-backward "a"))
        'search-failed)

;; == word-search-forward ==
(qa (with-temp-buffer
      (insert "axb xcb")
      (goto-char (point-min))
      (word-search-forward "axb xcb"))
    8)
(qa (with-temp-buffer
      (insert "a b c")
      (goto-char (point-min))
      (list (word-search-forward "b c") (point)))
    '(6 6))
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char (point-min))
          (word-search-forward "x"))
        'search-failed)

;; == skip-chars-forward/-backward ==
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (list (skip-chars-forward "a-c") (point)))
    '(3 4))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char (point-min))
      (skip-chars-forward "a-c")
      (list (skip-chars-forward "^a-c") (point)))
    '(1 5))
(qa (with-temp-buffer
      (insert "abc abc")
      (goto-char 5)
      (list (skip-chars-backward "a-c") (point)))
    '(0 5))
(qa (with-temp-buffer
      (insert "aaabbb")
      (goto-char (point-min))
      (list (skip-chars-forward "a" 2) (point)))
    '(1 2))
;; CHAR-SET has no \sX syntax classes: "\\sw" is just chars s and w
(qa (with-temp-buffer
      (insert "a-b")
      (goto-char (point-min))
      (skip-chars-forward "\\sw"))
    0)

;; == regexp-quote / regexp-opt / regexp-unmatchable ==
(qa (regexp-quote "a.b*c?\n") "a\\.b\\*c\\?\n")
(qa (regexp-quote "^$[x]|+") "\\^\\$\\[x]|\\+")
(qa (regexp-quote "") "")
(qa (string-match (regexp-quote "a.b") "a.b") 0)
(qa (string-match (regexp-quote "a.b") "axb") nil)
(qa (regexp-opt '("foo" "foobar")) "\\(?:foo\\(?:bar\\)?\\)")
(qa (regexp-opt '("foo" "bar")) "\\(?:bar\\|foo\\)")
(qa (regexp-opt '("foo" "foobar") 'words) "\\<\\(foo\\(?:bar\\)?\\)\\>")
(qa (regexp-opt '("a") t) "\\(a\\)")
(qa (regexp-opt nil) "\\(?:\\`a\\`\\)")
(qa regexp-unmatchable "\\`a\\`")
(qa (string-match regexp-unmatchable "a") nil)

;; == split-string ==
(qa (split-string "a,,b" ",") '("a" "" "b"))
(qa (split-string "a,,b" "," t) '("a" "b"))
(qa (split-string ",a," ",") '("" "a" ""))
(qa (split-string ",a," "," t) '("a"))
(qa (split-string "a b  c") '("a" "b" "c"))
(qa (split-string "a1b22c" "[0-9]+") '("a" "b" "c"))
(qa (split-string "aabba" "b") '("aa" "" "a"))
(qa (split-string "" ",") '(""))
(qa (split-string "abc" "") '("" "a" "b" "c" ""))
(qa (split-string ".a.b." "\\.") '("" "a" "b" ""))
(qa (split-string ".a.b." "\\." t) '("a" "b"))
;; TRIM regexp on separators here leaves edge empties in GNU 31.1
(qa (split-string ",a," "," nil "\\`") '("" "a" ""))
(qa (split-string "aXbXc" "X" nil "\\`") '("a" "b" "c"))

;; == quoting roundtrips ==
(qa (split-string-and-unquote "\"a b\" c") '("a b" "c"))
(qa (split-string-and-unquote "a b c" "[ \t\n]+") '("a" "b" "c"))
(qa (combine-and-quote-strings '("a b" "c d")) "\"a b\" \"c d\"")
(qa (combine-and-quote-strings '("a\"b" "c")) "\"a\\\"b\" c")
(qa (combine-and-quote-strings nil) "")
(qa (split-string-and-unquote (combine-and-quote-strings '("a b" "c d")))
    '("a b" "c d"))
(qa (shell-quote-argument "a b") "a\\ b")
(qa (shell-quote-argument "a'b") "a\\'b")
(qa (shell-quote-argument "a*b") "a\\*b")
(qa (shell-quote-argument "a$b") "a\\$b")
(qa (shell-quote-argument "normal") "normal")
(qa (shell-quote-argument "") "''")

(princ (qa--done))
