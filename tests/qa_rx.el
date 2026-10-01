;; rx — GNU-verified 31.1.  `group' is CAPTURING (\(...\)), `or' factors
;; common suffixes and merges single-char alternatives into […], named
;; classes print as [[:name:]], `anything' prints as "[^z-a]",
;; `unmatchable'/empty `or' print as "\\`a\\`".  `rx-let-eval' does not
;; define bindings during load-time macroexpansion (skipped).
(require 'rx)

;; Return the text matched by regexp RE in string S, or nil.
(defun qarx-m (re s)
  (and (string-match re s) (match-string 0 s)))

;; == literals ==
(qa (rx "a.b") "a\\.b")
(qa (rx "a*b+c?") "a\\*b\\+c\\?")
(qa (rx "[x]") "\\[x]") ;; ] not escaped by rx in 31.1
(qa (rx "^x") "\\^x")
(qa (rx "") "")
(qa (rx ?a) "a")
;; A bare integer is a character code: (rx 3) is a literal \003 char
;; (prints as "" without print-escape-control-characters — use qa-prin1
;; semantics via concat to force visibility).
(qa (rx 3) "\3")
(qa (rx (seq "a" 3)) "a\3")
(qa-err-data (eval '(rx (literal 97))) 'wrong-type-argument '(stringp 97))
;; `literal' quotes specials; it requires a string argument.
(qa (rx (literal "a.b")) "a\\.b")
(qa (rx (literal "^$*")) "\\^\\$\\*")
(qa (rx (literal "")) "")
;; `literal' defers the type error to eval time: macroexpand succeeds
(qa (macroexpand '(rx (literal 97))) '(regexp-quote 97))
(qa-err (macroexpand '(rx (literal "a" "b"))) 'error)
;; Quoted forms are not rx literals.
(qa-err-data (macroexpand '(rx 'a)) 'error '("Unknown rx form ‘quote’"))

;; == single-character forms ==
(qa (rx any) ".")
(qa (rx nonl) ".")
(qa (rx not-newline) ".")
(qa (rx anything) "[^z-a]")
(qa (qarx-m (rx (seq "a" any "b")) "axb") "axb")
(qa (string-match (rx (seq "a" any "b")) "a\nb") nil)
(qa (string-match (rx anything) "\n") 0)

;; == sequences and alternatives ==
(qa (rx (seq "a" "b")) "ab")
(qa (rx (sequence "a" "b")) "ab")
(qa (rx (: "a" "b")) "ab")
(qa (rx (and "a" "b")) "ab")
(qa (rx (seq)) "")
(qa (rx (or "a" "b")) "[ab]")
(qa (rx (| "a" "b")) "[ab]")
(qa (rx (or "a")) "a")
(qa (rx (or)) "\\`a\\`")
(qa (rx (or "ab" "cd")) "\\(?:ab\\|cd\\)")
(qa (rx (or "a" (seq "b" "c"))) "a\\|bc")
(qa (rx (seq (or "ab" "cd") "e")) "\\(?:ab\\|cd\\)e")
;; `or' refactors shared suffixes and sorts charset members last.
(qa (rx (or "ab" "a")) "\\(?:ab?\\)")
(qa (rx (or "a" "b" "cd")) "\\(?:cd\\|[ab]\\)")
(qa (rx (or "cd" "a" "b")) "\\(?:cd\\|[ab]\\)")
;; Nested `or' alternatives merge into one charset.
(qa (rx (or (or "a" "b") (or "c" "d"))) "[a-d]")
(qa (rx (or "a" (or "b" "c"))) "[abc]")
(qa (qarx-m (rx (or "a" "ab")) "ab") "ab")

;; == repetition ==
(qa (rx (zero-or-more "a")) "a*")
(qa (rx (* "a")) "a*")
(qa (rx (0+ "a")) "a*")
(qa (rx (one-or-more "a")) "a+")
(qa (rx (+ "a")) "a+")
(qa (rx (1+ "a")) "a+")
(qa (rx (zero-or-one "a")) "a?")
(qa (rx (optional "a")) "a?")
(qa (rx (opt "a")) "a?")
;; Multi-element bodies are wrapped in a shy group.
(qa (rx (+ "ab")) "\\(?:ab\\)+")
(qa (rx (one-or-more "a" "b")) "\\(?:ab\\)+")
(qa (rx (* (syntax word))) "\\w*")
(qa (rx (zero-or-more (or "ab" "cd"))) "\\(?:ab\\|cd\\)*")
(qa (rx (opt (group "ab"))) "\\(ab\\)?")
;; Non-greedy operators.
(qa (rx (*? "a")) "a*?")
(qa (rx (+? "a")) "a+?")
(qa (rx (?? "a")) "a??")
(qa (rx (+? (any "ab"))) "[ab]+?")
;; `minimal-match' rewrites single-element bodies only.
(qa (rx (minimal-match (one-or-more "a"))) "a+?")
(qa (rx (minimal-match (zero-or-more (any "ab")))) "[ab]*?")
(qa (rx (minimal-match (zero-or-one "a"))) "a??")
(qa (rx (minimal-match (* (seq "a" "b")))) "\\(?:ab\\)*")
(qa (rx (maximal-match (minimal-match (+ "a")))) "a+")
(qa (rx (maximal-match (*? "a"))) "a*?")
;; Exact and ranged counts.
(qa (rx (= 3 "a")) "a\\{3\\}")
(qa (rx (= 2 "ab")) "\\(?:ab\\)\\{2\\}")
(qa (rx (= 0 "a")) "a\\{0\\}")
(qa (rx (>= 2 "ab")) "\\(?:ab\\)\\{2,\\}")
(qa (rx (>= 1 (or "ab" "cd"))) "\\(?:ab\\|cd\\)\\{1,\\}")
(qa (rx (repeat 2 4 "a")) "a\\{2,4\\}")
(qa (rx (** 2 4 "a")) "a\\{2,4\\}")
(qa (rx (repeat 3 "ab")) "\\(?:ab\\)\\{3\\}")
(qa (rx (repeat 2 2 "a")) "a\\{2\\}")
(qa (rx (repeat 0 0 "a")) "a\\{0\\}")
(qa-err-data (macroexpand '(rx (repeat 2))) 'error
             '("rx ‘repeat’ requires at least 2 arguments"))
(qa-err-data (macroexpand '(rx (repeat 2 "a" "b"))) 'error
             '("rx ‘repeat’ range error"))
(qa (qarx-m (rx (repeat 2 3 "a")) "aaaa") "aaa")
(qa (qarx-m (rx (>= 2 "a")) "aaa") "aaa")
(qa (qarx-m (rx (minimal-match (one-or-more "a"))) "aaa") "a")
(qa (qarx-m (rx (one-or-more "a")) "aaa") "aaa")

;; == char sets ==
(qa (rx (any "a-c")) "[a-c]")
(qa (rx (any "a-c" "x")) "[a-cx]")
(qa (rx (any "a-z" "0-9")) "[0-9a-z]")
(qa (rx (any (?a . ?z))) "[a-z]")
(qa (rx (any ?a ?b)) "[ab]")
(qa (rx (in "abc")) "[a-c]")
(qa (rx (char "abc")) "[a-c]")
(qa (rx (any alpha digit)) "[[:alpha:][:digit:]]")
(qa (rx (any "a-b" "c-d" "e")) "[a-e]")
;; A lone "-" needs no brackets; "]", "^", "-" are reordered to stay valid.
(qa (rx (any "-")) "-")
(qa (rx (any ?^ ?- ?\])) "[]^-]")
(qa (rx (not (any ?^ ?- ?\]))) "[^]^-]")
(qa-err-data (macroexpand '(rx (any "^-]"))) 'error
             '("Invalid rx ‘any’ range: ^-]"))
(qa-err (macroexpand '(rx (any space-for-indent))) 'error)

;; == named classes ==
(qa (rx alpha) "[[:alpha:]]")
(qa (rx alphabetic) "[[:alpha:]]")
(qa (rx letter) "[[:alpha:]]")
(qa (rx digit) "[[:digit:]]")
(qa (rx numeric) "[[:digit:]]")
(qa (rx num) "[[:digit:]]")
(qa (rx alnum) "[[:alnum:]]")
(qa (rx alphanumeric) "[[:alnum:]]")
(qa (rx upper) "[[:upper:]]")
(qa (rx upper-case) "[[:upper:]]")
(qa (rx lower) "[[:lower:]]")
(qa (rx lower-case) "[[:lower:]]")
(qa (rx space) "[[:space:]]")
(qa (rx whitespace) "[[:space:]]")
(qa (rx white) "[[:space:]]")
(qa (rx blank) "[[:blank:]]")
(qa (rx punct) "[[:punct:]]")
(qa (rx punctuation) "[[:punct:]]")
(qa (rx xdigit) "[[:xdigit:]]")
(qa (rx hex-digit) "[[:xdigit:]]")
(qa (rx hex) "[[:xdigit:]]")
(qa (rx cntrl) "[[:cntrl:]]")
(qa (rx control) "[[:cntrl:]]")
(qa (rx print) "[[:print:]]")
(qa (rx printing) "[[:print:]]")
(qa (rx graph) "[[:graph:]]")
(qa (rx graphic) "[[:graph:]]")
(qa (rx ascii) "[[:ascii:]]")
(qa (rx nonascii) "[[:nonascii:]]")
(qa (rx word) "[[:word:]]")
(qa (rx wordchar) "[[:word:]]")
(qa (rx not-wordchar) "[^[:word:]]")
(qa-err (macroexpand '(rx symbol)) 'error)
(qa-err (macroexpand '(rx word-constituent)) 'error)
(qa (qarx-m (rx (+ digit)) "ab123") "123")
;; case-fold-search defaults t → char classes fold too
(qa (qarx-m (rx (+ (any "a-z"))) "abcABC") "abcABC")
(qa (qarx-m (rx (+ word)) "a_b!") "a")

;; == not ==
(qa (rx (not (any "a"))) "[^a]")
(qa (rx (not "a")) "[^a]")
(qa (rx (not ?a)) "[^a]")
(qa (rx (not alpha)) "[^[:alpha:]]")
(qa (rx (not digit)) "[^[:digit:]]")
(qa (rx (not (in "abc"))) "[^a-c]")
(qa (rx (not (or "a" "b"))) "[^ab]")
(qa (rx (not (any "a" "b" "c"))) "[^a-c]")
(qa (rx (not (syntax word))) "\\W")
(qa (rx (not (category ascii))) "\\Ca")
(qa-err-data (macroexpand '(rx (not "ab"))) 'error
             '("Illegal argument to rx ‘not’: \"ab\""))
(qa-err (macroexpand '(rx (not line-start))) 'error)
(qa-err (macroexpand '(rx (not (seq "a")))) 'error)

;; == anchors and boundaries ==
(qa (rx line-start) "^")
(qa (rx line-end) "$")
(qa (rx bol) "^")
(qa (rx eol) "$")
(qa (rx string-start) "\\`")
(qa (rx string-end) "\\'")
(qa (rx buffer-start) "\\`")
(qa (rx buffer-end) "\\'")
(qa (rx bos) "\\`")
(qa (rx eos) "\\'")
(qa (rx bot) "\\`")
(qa (rx eot) "\\'")
(qa (rx point) "\\=")
(qa (rx (seq "a" point "b")) "a\\=b")
(qa (rx word-start) "\\<")
(qa (rx word-end) "\\>")
(qa (rx bow) "\\<")
(qa (rx eow) "\\>")
(qa (rx word-boundary) "\\b")
(qa (rx not-word-boundary) "\\B")
(qa (rx symbol-start) "\\_<")
(qa (rx symbol-end) "\\_>")
(qa (rx (seq line-start "a" line-end)) "^a$")
(qa (rx (seq string-start "a")) "\\`a")
(qa (rx (or line-start "a")) "^\\|a")
;; "^" matches after an embedded newline; "\\`" only at position 0.
(qa (string-match (rx (seq line-start "a")) "ba\na") 3)
(qa (string-match (rx (seq string-start "a")) "ba") nil)
(qa (string-match (rx (seq word-boundary "cat")) "a cat") 2)
(qa (string-match (rx (seq not-word-boundary "at")) "cat") 1)
(qa (qarx-m (rx (seq symbol-start "foo" symbol-end)) "(foo)") "foo")
(qa (string-match (rx (seq symbol-start "foo" symbol-end)) "a_foo") nil)
;; "_" is not a word char, so "\<foo" can match inside "a_foo".
(qa (qarx-m (rx (seq word-start "foo" word-end)) "a_foo") "foo")

;; == syntax and category ==
(qa (rx (syntax word)) "\\w")
(qa (rx (syntax symbol)) "\\s_")
(qa (rx (syntax whitespace)) "\\s-")
(qa (rx (syntax open-parenthesis)) "\\s(")
(qa (rx (syntax close-parenthesis)) "\\s)")
(qa (rx (syntax punctuation)) "\\s.")
(qa (rx (syntax string-quote)) "\\s\"")
(qa (rx (syntax escape)) "\\s\\")
(qa (rx (syntax comment-start)) "\\s<")
(qa (rx (syntax paired-delimiter)) "\\s$")
(qa (rx (syntax expression-prefix)) "\\s'")
(qa (rx (syntax character-quote)) "\\s/")
(qa-err-data (macroexpand '(rx (syntax invalid-name))) 'error
             '("Unknown rx syntax name ‘invalid-name’"))
(qa (rx (category ascii)) "\\ca")
(qa (rx (category chinese)) "\\cc")
(qa (rx (category space-for-indent)) "\\c ")
(qa (qarx-m (rx (+ (syntax word))) "a_b ") "a")

;; == groups, numbered groups, backrefs ==
(qa (rx (group "a")) "\\(a\\)")
(qa (rx (submatch "a")) "\\(a\\)")
(qa (rx (group "a" "b")) "\\(ab\\)")
(qa (rx (group)) "\\(\\)")
(qa (rx (group-n 1 "a")) "\\(?1:a\\)")
(qa (rx (submatch-n 1 "a")) "\\(?1:a\\)")
(qa (rx (group-n 2 "a" "b")) "\\(?2:ab\\)")
(qa (rx (group-n 10 "x")) "\\(?10:x\\)")
(qa (rx (backref 1)) "\\1")
(qa (rx (seq (group "a") (backref 1))) "\\(a\\)\\1")
(qa (rx (seq (group-n 1 "a") (backref 1))) "\\(?1:a\\)\\1")
(qa-err-data (macroexpand '(rx (backref 0))) 'error
             '("rx ‘backref’ requires an argument in the range 1..9"))
(qa-err (macroexpand '(rx (backref 12))) 'error)
(qa-err (macroexpand '(rx (submatch-n 0 "x"))) 'error)
(qa (string-match (rx (seq (group "a") (backref 1))) "aa") 0)
(qa (string-match (rx (seq (group "a") (backref 1))) "ab") nil)
;; Numbered groups are numbered by their explicit number, not position.
(let ((s "ba"))
  (qa (and (string-match (rx (seq (group-n 2 "b") (group-n 1 "a"))) s)
           (list (match-string 1 s) (match-string 2 s)))
      '("a" "b")))
(let ((s "xy"))
  (qa (and (string-match (rx (seq (group "x") (group-n 4 "y"))) s)
           (list (match-string 1 s) (match-string 2 s) (match-string 4 s)))
      '("x" nil "y")))
(qa (qarx-m (rx (seq (group (or "a" "b")) "c")) "bc") "bc")
(qa (rx (+ (group "a"))) "\\(a\\)+")

;; == regex / regexp / eval / unmatchable ==
(qa (rx (regex "a\\|b")) "a\\|b")
(qa (rx (regexp "a\\|b")) "a\\|b")
(qa (rx (seq (regex "a\\|b") "c")) "\\(?:a\\|b\\)c")
(qa (string-match (rx (regex "a\\|b")) "xby") 1)
;; `eval' evaluates the form during expansion; a string splices, an
;; integer expands like a bare integer (to "").
(qa (rx (eval "ab")) "ab")
(qa (rx (eval ?a)) "a")
(qa (rx (seq "x" (eval "y") "z")) "xyz")
(qa (rx (eval (+ 1 2))) "\3") ;; eval'd INTEGER is a char code
(qa (rx unmatchable) "\\`a\\`")
(qa (rx (seq unmatchable "a")) "\\`a\\`a")
(qa (string-match (rx unmatchable) "anything") nil)

;; == intersection ==
(qa (rx (intersection (any "a-z") (any "b-z"))) "[b-z]")
(qa (rx (intersection (any "a-f") (any "c-z") (any "b-e"))) "[c-e]")
(qa (rx (not (intersection (any "a-f") (any "c-z")))) "[^c-f]")
(qa-err-data (macroexpand '(rx (intersection "a-f" "c-e"))) 'error
             '("Cannot be used in rx intersection: \"a-f\""))
(qa-err (macroexpand '(rx (intersection (any "a-z") digit))) 'error)

;; == rx-to-string ==
;; Wraps the form in a shy group unless NO-GROUP or the form is atomic.
(qa (rx-to-string '(: "a" "b")) "\\(?:ab\\)")
(qa (rx-to-string '(: "a" "b") t) "ab")
(qa (rx-to-string '(or "a" "b")) "[ab]")
(qa (rx-to-string '(or "a" "b") t) "[ab]")
(qa (rx-to-string '(or "ab" "cd")) "\\(?:ab\\|cd\\)")
(qa (rx-to-string '(or "ab" "cd") t) "\\(?:ab\\|cd\\)")
(qa (rx-to-string '(group "a")) "\\(a\\)")
(qa (rx-to-string "a") "a")

;; == rx-define / rx-let ==
(rx-define qarx-word (seq alpha (+ (any "_-" alnum))))
(rx-define qarx-pair (x) (seq x ":" x))
(qa (rx qarx-word) "[[:alpha:]][_[:alnum:]-]+")
(qa (qarx-m (rx qarx-word) "foo-bar_9") "foo-bar_9")
(qa (rx (qarx-pair "ab")) "ab:ab")
;; rx-let splices the binding body without adding a group.
(qa (rx-let ((qarxv (any "aeiou"))) (rx (seq qarxv "x"))) "[aeiou]x")
(qa (rx-let ((qarxw (+ (any "a-z")))) (rx (seq qarxw ":" qarxw)))
    "[a-z]+:[a-z]+")

;; == case sensitivity ==
;; rx output is case-insensitive only under case-fold-search (t by default).
(qa (let ((case-fold-search t)) (string-match (rx "ABC") "xabc")) 1)
(qa (let ((case-fold-search nil)) (string-match (rx "ABC") "xabc")) nil)
;; Case fold also extends negated charsets.
(qa (let ((case-fold-search t))
      (string-match (rx (not (any "a-z"))) "abcABC"))
    nil)
(qa (let ((case-fold-search nil))
      (string-match (rx (not (any "a-z"))) "abcABC"))
    3)

;; == composite ==
(qa (rx (seq (+ (or "ab" "cd")) (opt (any "0-9")))) "\\(?:ab\\|cd\\)+[0-9]?")
(qa (qarx-m (rx (seq string-start (group (+ word)) "@" (group (+ digit))
                    string-end))
            "user@42")
    "user@42")

(princ (qa--done))
