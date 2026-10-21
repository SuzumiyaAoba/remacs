;; GNU-verified coverage of the buffer-search domain beyond qa_match.el
;; (match-data internals, BOUND/NOERROR/COUNT), qa_case.el (case-fold),
;; qa_regex.el (regexp syntax), qa_syntax*.el (skip-chars basics,
;; scan-sexps/lists, defun motion) and qa_replace.el (region-bounded
;; count-matches, perform-replace bodies).
;; Pinned GNU 31.1 semantics:
;;  * NEITHER `search-forward' NOR `word-search-forward' takes a NOCASE
;;    argument: a 5th argument signals `wrong-number-of-arguments' (the
;;    data is (FN . ARGCOUNT) as a cons — prints (fn . 5)).
;;  * `search-whitespace-regexp' affects ONLY isearch: Lisp-level
;;    search-forward AND re-search-forward ignore it entirely.
;;  * `char-fold-search' does not exist in batch -Q.
;;  * word-search turns spaces into "\\W+" and anchors both ends with
;;    "\\<"/"\\>" (strict); the lax right boundary lives only in
;;    word-search-forward-lax.  IGNORE-PUNCT is not an argument.
;;  * `re-search-backward'/`search-backward' return the match START.
;;    A backward match may END at point but never begin at point.
;;  * `forward-sentence' returns the new point; with the default
;;    `sentence-end-double-space' a ". " single-space is NOT a
;;    boundary — the sentence runs on.  `sentence-end' requires two
;;    spaces, a tab, or EOL after the closer.  At eob it signals
;;    `end-of-buffer'; `backward-sentence' at bob returns 1 (no error).
;;  * `forward-paragraph' returns the count of paragraphs NOT moved
;;    (0 when fulfilled); `backward-paragraph' bizarrely returns -1 on
;;    a fulfilled move (pinned, GNU-true).  Both land on the START of
;;    the separating blank line.
;;  * `count-matches'/`how-many' with nil bounds count from POINT to
;;    point-max (NOT the whole buffer); swapped bounds are sorted; an
;;    END past point-max is silently clamped, START 0 clamped to 1.
;;  * `inhibit-changing-match-data' non-nil suppresses match-data
;;    recording for buffer searches.
;;  * skip-chars LIM is a destination position, clipped into the
;;    accessible range; skip-chars-backward returns the NEGATIVE
;;    distance moved.  `^' is magic only as first char; `]' is always
;;    literal; a trailing/leading `-' is literal.
;; Buffer positions below are 1-based inside each with-temp-buffer.

;; == no NOCASE argument exists (GNU 31.1) ==
;; error data is the proper list (FUNCTION ARGCOUNT)
(qa-err-data (with-temp-buffer
               (insert "ABC") (goto-char 1)
               (search-forward "abc" nil nil 1 t))
             'wrong-number-of-arguments '(search-forward 5))
(qa-err-data (with-temp-buffer
               (insert "ABC") (goto-char 1)
               (search-backward "abc" nil nil 1 t))
             'wrong-number-of-arguments '(search-backward 5))
(qa-err-data (with-temp-buffer
               (insert "cat, dog") (goto-char 1)
               (word-search-forward "cat dog" nil t 1 t))
             'wrong-number-of-arguments '(word-search-forward 5))
(qa-err-data (with-temp-buffer
               (insert "a") (goto-char 1)
               (re-search-forward "a" nil nil 1 t))
             'wrong-number-of-arguments '(re-search-forward 5))
;; case comes only from case-fold-search
(qa (with-temp-buffer
      (insert "ABC")
      (goto-char 1)
      (let ((case-fold-search nil))
        (search-forward "abc" nil t)))
    nil)

;; == search-whitespace-regexp: isearch-only, ignored by Lisp calls ==
(qa (with-temp-buffer
      (insert "a    b")
      (goto-char 1)
      (let ((search-whitespace-regexp " +"))
        (list (search-forward "a b" nil t) (point))))
    '(nil 1))
(qa (with-temp-buffer
      (insert "a    b")
      (goto-char 1)
      (let ((search-whitespace-regexp " +"))
        (list (re-search-forward "a b" nil t) (point))))
    '(nil 1))
;; the failure is a real search-failed without NOERROR
(qa-err-data (with-temp-buffer
               (insert "a    b")
               (goto-char 1)
               (let ((search-whitespace-regexp " +"))
                 (search-forward "a b")))
             'search-failed '("a b"))
;; char folding variable absent in batch -Q
(qa (boundp 'char-fold-search) nil)

;; == search-forward/-backward point & return contracts ==
;; forward: point and return both land at match END
(qa (with-temp-buffer
      (insert "xabcy")
      (goto-char 1)
      (list (search-forward "abc") (point)))
    '(5 5))
;; backward: point and return land at match START; match-data records
;; the full extent
(qa (with-temp-buffer
      (insert "xabcy")
      (goto-char (point-max))
      (list (search-backward "abc") (point)
            (match-beginning 0) (match-end 0)))
    '(2 2 2 5))
;; a backward match may END at point but may not START at point
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 4)
      (list (search-backward "c" nil t) (point)))
    '(3 3))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 3)
      (list (search-backward "c" nil t) (point)))
    '(nil 3))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 4)
      (list (re-search-backward "bc" nil t) (point)))
    '(2 2))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 3)
      (list (re-search-backward "bc" nil t) (point)))
    '(nil 3))
;; BOUND is half-open on the far side: a forward match must end
;; strictly BEFORE bound (end == bound misses), backward must begin
;; strictly after it
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (search-forward "bc" 3 t) (point)))
    '(nil 1))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 4)
      (list (search-backward "bc" 2 t) (point)))
    '(2 2))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 4)
      (list (search-backward "bc" 3 t) (point)))
    '(nil 4))
;; BOUND accepts a marker
(qa (with-temp-buffer
      (insert "abcabc")
      (goto-char 1)
      (list (search-forward "c" (copy-marker 4) t) (point)))
    '(4 4))
;; NOERROR non-t non-nil moves to BOUND (covered in qa_match) — here:
;; bound nil moves to the limit of the accessible region
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (re-search-forward "zz" 2 'x) (point)))
    '(nil 2))
;; COUNT > occurrences + NOERROR non-t: to the far limit, nil
(qa (with-temp-buffer
      (insert "abab")
      (goto-char 1)
      (list (search-forward "ab" nil 'x 5) (point)))
    '(nil 5))
;; search-backward COUNT>1 picks the COUNTth previous occurrence
(qa (with-temp-buffer
      (insert "axax")
      (goto-char (point-max))
      (list (search-backward "ax" nil nil 2) (point)))
    '(1 1))
;; empty STRING matches at point
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 2)
      (list (search-forward "") (point)))
    '(2 2))
;; empty buffer: nil with noerror, and "" still matches at 1
(qa (with-temp-buffer (search-forward "a" nil t)) nil)
(qa (with-temp-buffer (list (re-search-forward "" nil t) (point))) '(1 1))
;; non-string STRING arg
(qa-err-data (search-forward 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (re-search-forward 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (word-search-forward 5) 'wrong-type-argument '(stringp 5))
;; search-failed carries the sought string
(qa-err-data (with-temp-buffer (insert "abc") (goto-char 1)
               (re-search-forward "zz"))
             'search-failed '("zz"))
;; narrowing bounds the search; positions stay absolute
(qa (with-temp-buffer
      (insert "a1 b1 a1")
      (goto-char 1)
      (narrow-to-region 7 9)
      (goto-char 7)
      (list (search-forward "a") (point)))
    '(8 8))
;; a point outside the narrowed region is clamped into it, so the
;; match inside is still found
(qa (with-temp-buffer
      (insert "a1 b1 a1")
      (goto-char 1)
      (narrow-to-region 7 9)
      (list (search-forward "a" nil t) (point)))
    '(8 8))
;; buffer literal search sets match data too (ints under the accessor)
(qa (with-temp-buffer
      (insert "a12b")
      (goto-char 1)
      (search-forward "12")
      (list (match-beginning 0) (match-end 0)))
    '(2 4))
;; re-search-forward records groups relative to buffer positions
(qa (with-temp-buffer
      (insert "a12b")
      (goto-char 1)
      (re-search-forward "\\([0-9]+\\)")
      (list (point) (match-beginning 1) (match-end 1)))
    '(4 2 4))

;; == aliases ==
(qa (eq (symbol-function 'search-forward-regexp)
        (symbol-function 're-search-forward))
    t)
(qa (eq (symbol-function 'search-backward-regexp)
        (symbol-function 're-search-backward))
    t)

;; == looking-at / looking-back never move point ==
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 2)
      (list (looking-at "zz") (point)))
    '(nil 2))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 2)
      (list (looking-back "zz" nil t) (point)))
    '(nil 2))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (posix-looking-at "ab") (point)))
    '(t 1))
;; empty regexp matches looking-at even in an empty buffer
(qa (with-temp-buffer (looking-at "")) t)

;; == inhibit-changing-match-data ==
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (set-match-data nil)
      (let ((inhibit-changing-match-data t))
        (re-search-forward "b"))
      (match-data))
    nil)

;; == word-search: own whitespace folding + strict boundaries ==
;; spaces in STRING become \\W+ — any non-word run separates words
(qa (word-search-regexp "a b") "\\<a\\W+b\\>")
(qa (word-search-regexp "a b" t) "\\<a\\W+b")
(qa (word-search-regexp "a\nb") "\\<a\\W+b\\>")
;; \\W+ needs real non-word chars between the words, so "axb"/"xcb"
;; never match "a b" — search-failed carries the CONSTRUCTED regexp
(qa-err-data (with-temp-buffer
               (insert "axb xcb axb")
               (goto-char 1)
               (word-search-forward "a b"))
             'search-failed '("\\<a\\W+b\\>"))
;; "cat, dog" : the comma is skipped by \\W+
(qa (with-temp-buffer
      (insert "cat, dog")
      (goto-char 1)
      (list (word-search-forward "cat dog" nil t) (point)))
    '(9 9))
;; strict boundaries both ends: no match inside a longer word
(qa (with-temp-buffer
      (insert "scatter cat")
      (goto-char 1)
      (list (word-search-forward "cat" nil t) (point)))
    '(12 12))
(qa (with-temp-buffer
      (insert "cat scatter cat!")
      (goto-char 1)
      (list (word-search-forward "cat" nil nil 1) (point)))
    '(4 4))
;; backward variant: lands on match start, strict left boundary
(qa (with-temp-buffer
      (insert "cat dog cat")
      (goto-char (point-max))
      (list (word-search-backward "cat") (point)))
    '(9 9))
(qa (with-temp-buffer
      (insert "scat")
      (goto-char (point-max))
      (list (word-search-backward "cat" nil t) (point)))
    '(nil 5))
(qa (with-temp-buffer
      (insert "cat xcat")
      (goto-char (point-max))
      (list (word-search-backward "cat" nil t) (point)))
    '(1 1))
;; word-search failure contracts mirror search-forward
(qa (with-temp-buffer
      (insert "cat")
      (goto-char 1)
      (list (word-search-forward "dog" nil 'x) (point)))
    '(nil 4))
(qa-err-data (with-temp-buffer
               (insert "cat")
               (goto-char 1)
               (word-search-forward "dog"))
             'search-failed '("\\<dog\\>"))
;; COUNT and BOUND work too
(qa (with-temp-buffer
      (insert "cat cat cat")
      (goto-char 1)
      (list (word-search-forward "cat" nil nil 2) (point)))
    '(8 8))
(qa (with-temp-buffer
      (insert "cat cat")
      (goto-char (point-max))
      (list (word-search-forward "cat" nil nil -1) (point)))
    '(5 5))
(qa (with-temp-buffer
      (insert "cat")
      (goto-char 1)
      (list (word-search-forward "cat" 3 t) (point)))
    '(nil 1))
;; empty word-search string matches at point
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (word-search-forward "" nil t) (point)))
    '(1 1))
;; word-search honors case-fold-search
(qa (with-temp-buffer
      (insert "CAT")
      (goto-char 1)
      (let ((case-fold-search t))
        (list (word-search-forward "cat") (point))))
    '(4 4))

;; == sentence motion ==
;; default sentence-end-double-space: "One. Two." is ONE sentence
(qa (with-temp-buffer
      (insert "One. Two. Three.")
      (goto-char 1)
      (list (forward-sentence) (point)))
    '(17 17))
(qa (with-temp-buffer
      (insert "One. Two. Three.")
      (goto-char (point-max))
      (list (backward-sentence) (point)))
    '(1 1))
;; two spaces (or tab/EOL) make the boundary; point lands right after
;; the closer, before the trailing whitespace
(qa (with-temp-buffer
      (insert "One.  Two.")
      (goto-char 1)
      (list (forward-sentence) (point)))
    '(5 5))
(qa (with-temp-buffer
      (insert "One. Two.\nThree.")
      (goto-char 1)
      (list (forward-sentence) (point)))
    '(10 10))
;; sentence-end-double-space nil: single space suffices
(qa (with-temp-buffer
      (insert "One. Two. Three.")
      (goto-char 1)
      (let ((sentence-end-double-space nil))
        (list (forward-sentence) (point))))
    '(5 5))
;; returns the new point
(qa (with-temp-buffer
      (insert "A.  B.")
      (goto-char 1)
      (forward-sentence))
    3)
(qa (with-temp-buffer
      (insert "A.  B.")
      (goto-char (point-max))
      (backward-sentence))
    5)
;; backward lands at the sentence's first char
(qa (with-temp-buffer
      (insert "One.  Two.  X")
      (goto-char (point-max))
      (list (backward-sentence) (point)))
    '(13 13))
;; boundaries: eob signals end-of-buffer, bob quietly returns 1
(qa-err (with-temp-buffer
          (insert "A.  B.")
          (goto-char (point-max))
          (forward-sentence))
        'end-of-buffer)
(qa (with-temp-buffer
      (insert "A.  B.")
      (goto-char 1)
      (backward-sentence))
    1)
;; sentences do not cross a paragraph (blank-line) boundary
(qa (with-temp-buffer
      (insert "One. Two\n\nThree.")
      (goto-char 1)
      (list (forward-sentence) (point)))
    '(9 9))
;; the machinery is a Lisp variable + default function
(qa (boundp 'forward-sentence-function) t)
(qa (fboundp 'sentence-end) t)
;; the default regexp: closer + two-space/tab/EOL terminator
(qa (stringp (sentence-end)) t)
(qa (let ((sentence-end "\\.\\'")) (sentence-end)) "\\.\\'")

;; == paragraph motion ==
;; lands on the START of the separating blank line, returns
;; paragraphs-left (0 when the move is fulfilled)
(qa (with-temp-buffer
      (insert "para one\n\npara two\n")
      (goto-char 1)
      (list (forward-paragraph) (point)))
    '(0 10))
(qa (with-temp-buffer
      (insert "para one\n\n\npara two\n")
      (goto-char 1)
      (list (forward-paragraph) (point)))
    '(0 10))
(qa (with-temp-buffer
      (insert "para one\n\npara two\n")
      (goto-char (point-max))
      (list (backward-paragraph) (point)))
    '(0 10))
;; overshooting returns the unfulfilled count and clamps point
(qa (with-temp-buffer
      (insert "a\n\nb\n\nc\n")
      (goto-char 1)
      (list (forward-paragraph 5) (point)))
    '(2 9))
(qa (with-temp-buffer
      (insert "a\n\nb\n\nc\n")
      (goto-char 1)
      (list (forward-paragraph 2) (point)))
    '(0 6))
(qa (with-temp-buffer
      (insert "a\n\nb\n\nc\n")
      (goto-char (point-max))
      (list (backward-paragraph 2) (point)))
    '(0 3))
;; GNU quirk: a fulfilled backward-paragraph returns -1, not 0
(qa (with-temp-buffer
      (insert "aa bb cc")
      (goto-char 9)
      (list (backward-paragraph) (point)))
    '(-1 1))
(qa (with-temp-buffer
      (insert "aa bb cc")
      (goto-char 3)
      (list (backward-paragraph) (point)))
    '(-1 1))
;; forward-paragraph -1 behaves like backward-paragraph
(qa (with-temp-buffer
      (insert "aa bb cc")
      (goto-char 1)
      (list (forward-paragraph -1) (point)))
    '(-1 1))
;; no blank lines: forward goes to eob
(qa (with-temp-buffer
      (insert "aa bb cc")
      (goto-char 1)
      (list (forward-paragraph) (point)))
    '(0 9))
;; paragraph-start customizes what separates paragraphs
(qa (with-temp-buffer
      (insert "a\nX line\nb\n")
      (goto-char (point-max))
      (let ((paragraph-start "^X"))
        (list (backward-paragraph) (point))))
    '(0 3))

;; == count-matches / how-many bounds semantics ==
;; nil bounds = point..point-max, NOT the whole buffer
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (goto-char 5)
      (count-matches "[0-9]"))
    2)
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (goto-char 5)
      (how-many "[0-9]"))
    2)
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (goto-char 1)
      (list (count-matches "[0-9]") (how-many "[0-9]")))
    '(3 3))
;; swapped bounds are sorted rather than an error
(qa (with-temp-buffer
      (insert "a1b2")
      (count-matches "[0-9]" 5 1))
    2)
(qa (with-temp-buffer
      (insert "abc")
      (count-matches "a" 3 2))
    0)
;; out-of-range bounds are clamped, not an error
(qa (with-temp-buffer
      (insert "abc")
      (count-matches "a" 1 99))
    1)
(qa (with-temp-buffer
      (insert "abc")
      (count-matches "a" 0 2))
    1)
;; not a subr (it lives in replace.el)
(qa (subrp (symbol-function 'count-matches)) nil)
(qa (subrp (symbol-function 'skip-chars-forward)) t)
(qa (subrp (symbol-function 'search-forward)) t)
(qa (subrp (symbol-function 'forward-paragraph)) t)

;; == skip-chars: char-class syntax edges ==
;; `^' is magic only as the FIRST character
(qa (with-temp-buffer
      (insert "a^b")
      (goto-char 1)
      (list (skip-chars-forward "a^") (point)))
    '(2 3))
(qa (with-temp-buffer
      (insert "^a^b")
      (goto-char 1)
      (list (skip-chars-forward "a^") (point)))
    '(3 4))
;; a bare "^" complements the empty set — skips EVERYTHING
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (skip-chars-forward "^") (point)))
    '(3 4))
;; `-' is literal at either end; `]' is always literal
(qa (with-temp-buffer
      (insert "--abc")
      (goto-char 1)
      (list (skip-chars-forward "-a") (point)))
    '(3 4))
(qa (with-temp-buffer
      (insert "-abc")
      (goto-char 1)
      (list (skip-chars-forward "a-") (point)))
    '(2 3))
(qa (with-temp-buffer
      (insert "]ab]")
      (goto-char 1)
      (list (skip-chars-forward "]a") (point)))
    '(2 3))
(qa (with-temp-buffer
      (insert "]x]x")
      (goto-char 1)
      (list (skip-chars-forward "]x") (point)))
    '(4 5))
;; ranges compose; complement + range compose
(qa (with-temp-buffer
      (insert "aA1b")
      (goto-char 1)
      (list (skip-chars-forward "a-zA-Z0-9") (point)))
    '(4 5))
;; empty spec skips nothing
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (skip-chars-forward "") (point)))
    '(0 1))
;; return value = signed distance moved
(qa (with-temp-buffer
      (insert "abc  ")
      (goto-char 6)
      (skip-chars-backward " "))
    -2)
;; LIM is a destination position, clipped into the accessible range
(qa (with-temp-buffer
      (insert "aaaa")
      (goto-char 1)
      (list (skip-chars-forward "a" 99) (point)))
    '(4 5))
(qa (with-temp-buffer
      (insert "aaaa")
      (goto-char 4)
      (list (skip-chars-backward "a" 2) (point)))
    '(-2 2))
(qa (with-temp-buffer
      (insert "aaaa")
      (goto-char 4)
      (list (skip-chars-backward "a" 0) (point)))
    '(-3 1))
;; LIM on the wrong side means no movement at all
(qa (with-temp-buffer
      (insert "aaaa")
      (goto-char 1)
      (list (skip-chars-forward "a" -5) (point)))
    '(0 1))
;; LIM stops the skip mid-class
(qa (with-temp-buffer
      (insert "  abc")
      (goto-char 5)
      (list (skip-chars-backward "abc" 4) (point)))
    '(-1 4))
;; non-string spec is a type error
(qa-err-data (skip-chars-forward 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (skip-chars-backward 'x) 'wrong-type-argument '(stringp x))

;; == skip-syntax: string of syntax codes, same LIM contract ==
;; return = signed distance; " " is whitespace syntax
(qa (with-temp-buffer
      (insert "ab 12")
      (goto-char 3)
      (list (skip-syntax-forward " ") (point)))
    '(1 4))
(qa (with-temp-buffer
      (insert "ab 12")
      (goto-char 4)
      (list (skip-syntax-backward " ") (point)))
    '(-1 3))
;; unknown code letters simply match nothing — no error
(qa (with-temp-buffer
      (insert "zzq")
      (goto-char 1)
      (list (skip-syntax-forward "z") (point)))
    '(0 1))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (list (skip-syntax-forward "xyz") (point)))
    '(0 1))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (skip-syntax-forward ""))
    0)
;; LIM is a destination position, clipped
(qa (with-temp-buffer
      (insert "  abc")
      (goto-char 5)
      (list (skip-syntax-backward " " 3) (point)))
    '(0 5))
(qa (with-temp-buffer
      (insert "abcd")
      (goto-char 1)
      (list (skip-syntax-forward "w" 3) (point)))
    '(2 3))
(qa (with-temp-buffer
      (insert "abcd")
      (goto-char 5)
      (list (skip-syntax-backward "w" 3) (point)))
    '(-2 3))
;; negative LIM does not move backward either; GNU clips it (returns 0)
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 3)
      (list (skip-syntax-forward "w" -1) (point)))
    '(0 3))
(qa-err-data (skip-syntax-forward 5) 'wrong-type-argument '(stringp 5))

;; == regexp-opt extras (qa_regex pins the basic shapes) ==
(qa (regexp-opt '("foo" "bar") 'words) "\\<\\(bar\\|foo\\)\\>")
(qa (regexp-opt '("foo") 'words) "\\<\\(foo\\)\\>")
;; 'symbols uses \_< \_> symbol boundaries
(qa (regexp-opt '("foo" "bar") 'symbols) "\\_<\\(bar\\|foo\\)\\_>")
;; special chars inside strings are quoted
(qa (regexp-opt '("a.c" "b")) "\\(?:a\\.c\\|b\\)")
;; the produced regexp actually matches
(qa (string-match (regexp-opt '("foo" "bar") 'words) "a bar b") 2)
(qa (progn
      (string-match (regexp-opt '("cat" "car") 'words) "a car")
      (match-string 0 "a car"))
    "car")

;; == regexp-quote extras ==
(qa (regexp-quote "λ.ö") "λ\\.ö")
(qa (string-match (regexp-quote "^x$") "^x$") 0)

;; == perform-replace: return value is nil ==
(qa (with-temp-buffer
      (insert "foo foo")
      (goto-char 1)
      (perform-replace "foo" "X" nil t nil))
    nil)

;; == scan-sexps / list motion complements ==
;; scan-sexps stops inside a comment? comments need a mode table;
;; in fundamental-mode ";" is a word char — pinned: scans it as word
(qa (with-temp-buffer
      (insert ";a b")
      (scan-sexps 1 2))
    5)
;; forward-list/backward-list return the resulting point; on failure
;; point just stops at the limit (no nil, no error at top level)
(qa (with-temp-buffer
      (insert "(a) b")
      (goto-char 5)
      (list (forward-list) (point)))
    '(6 6))
(qa (with-temp-buffer
      (insert "a (b)")
      (goto-char 1)
      (list (backward-list) (point)))
    '(1 1))

;; == save-match-data / string-match / match-data shapes ==
;; save-match-data restores match data after the inner forms run —
;; inside, the NEW match is visible; outside, the old one returns.
(qa (let ((s "a1b2"))
      (string-match "\\([0-9]\\)" s)
      (list (match-string 0 s)
            (save-match-data (string-match "b" s) (match-string 0 s))
            (match-string 0 s)))
    '("1" "b" "1"))
;; string-match stores 0-based positions; match-data is flat pairs
;; (b0 e0 b1 e1 …) with no trailing buffer for string matches
(qa (save-match-data
      (string-match "\\(a\\)\\(b\\)" "xab")
      (match-data))
    '(1 3 1 2 2 3))
;; buffer matches record MARKERS plus the buffer as the last element
(qa (with-temp-buffer
         (insert "xab")
         (goto-char 1)
         (re-search-forward "a\\(b\\)")
         (list (match-beginning 0) (match-end 0)
               (match-beginning 1) (match-end 1)
               (markerp (car (match-data)))))
       '(2 4 3 4 t))
;; (match-data 'integers) swaps markers for ints; buffer still trails
(qa (with-temp-buffer
      (insert "xy")
      (goto-char 1) (re-search-forward "y")
      (let ((md (match-data 'integers)))
        (list (car md) (cadr md)
              (eq (car (last md)) (current-buffer)))))
    '(2 3 t))
;; string-match START arg is a 0-based position
(qa (string-match "b" "abcb" 2) 3)
(qa (string-match "b" "abcb" 3) 3)
(qa (string-match "z" "abc") nil)
(qa (string-match "^\\([a-z]+\\)\\([0-9]+\\)" "abc12") 0)
;; a failed re-search does NOT clobber prior match data
(qa (with-temp-buffer
      (insert "a1")
      (goto-char 1)
      (re-search-forward "[0-9]")
      (list (match-string 0)
            (progn (re-search-forward "zzz" nil t) (match-string 0))))
    '("1" "1"))
;; set-match-data with plain integers re-seats the match
(qa (with-temp-buffer
      (insert "abcd")
      (set-match-data '(2 4))
      (list (match-beginning 0) (match-end 0) (match-string 0)))
    '(2 4 "bc"))
;; group numbers beyond the match return nil, not an error
(qa (save-match-data (string-match "a" "a") (match-beginning 5)) nil)
;; looking-at does not move point
(qa (with-temp-buffer (insert "abc") (goto-char 1)
      (list (looking-at "ab") (point)))
    '(t 1))
;; looking-at-p / string-match-p exist
(qa (list (fboundp 'looking-at-p) (fboundp 'string-match-p)) '(t t))
;; match-string-no-properties mirrors match-string on unpropertized text
(qa (save-match-data
      (string-match "a" "a")
      (match-string-no-properties 0 "a"))
    "a")
;; match-string with no STRING reads from the current buffer
(qa (with-temp-buffer (insert "abc") (goto-char 1)
      (re-search-forward "b")
      (match-string 0))
    "b")
;; COUNT on re-search-forward/-backward steps N matches
(qa (with-temp-buffer (insert "a a a") (goto-char 1)
      (list (re-search-forward "a" nil nil 2) (point)))
    '(4 4))
(qa (with-temp-buffer (insert "a a a") (goto-char (point-max))
      (list (re-search-backward "a" nil nil 2) (point)))
    '(3 3))

;; NOERROR non-t in a narrowed buffer: point lands at ZV/BEGV of the
;; narrowed region — the limit is the *absolute* end index, not
;; region length (a `begv' slip here broke `forward-paragraph's
;; `(goto-char start)' inside erc's narrowed docstring buffers).
(qa (with-temp-buffer
      (insert "aaa bbb ccc")
      (narrow-to-region 5 11)
      (goto-char (point-min))
      (list (re-search-forward "zzz" nil 1) (point)))
    '(nil 11))
(qa (with-temp-buffer
      (insert "aaa bbb ccc")
      (narrow-to-region 5 11)
      (goto-char (point-max))
      (list (re-search-backward "zzz" nil 1) (point)))
    '(nil 5))

;; Issue #272: a partial backward match must not satisfy the full count.
(defun qa272-run (function pattern count noerror &optional narrowed)
 (with-temp-buffer
  (when narrowed (insert "hidden "))
  (let ((beg (point)))
   (insert "a")
   (when narrowed (narrow-to-region beg (point-max)))
   (goto-char (point-max))
   (list (condition-case e (funcall function pattern nil noerror count) (error e)) (point)))))
(qa
(qa272-run 'search-backward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 'search-backward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 'search-backward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 'search-backward "a" 1 nil)
'(1 1)
)

(qa
(qa272-run 'search-backward "a" 1 t)
'(1 1)
)

(qa
(qa272-run 'search-backward "a" 1 'move)
'(1 1)
)

(qa
(qa272-run 'search-backward "a" 2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'search-backward "a" 2 t)
'(nil 2)
)

(qa
(qa272-run 'search-backward "a" 2 'move)
'(nil 1)
)

(qa
(qa272-run 'search-backward "a" 3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'search-backward "a" 3 t)
'(nil 2)
)

(qa
(qa272-run 'search-backward "a" 3 'move)
'(nil 1)
)

(qa
(qa272-run 're-search-backward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 're-search-backward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 're-search-backward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 're-search-backward "a" 1 nil)
'(1 1)
)

(qa
(qa272-run 're-search-backward "a" 1 t)
'(1 1)
)

(qa
(qa272-run 're-search-backward "a" 1 'move)
'(1 1)
)

(qa
(qa272-run 're-search-backward "a" 2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 're-search-backward "a" 2 t)
'(nil 2)
)

(qa
(qa272-run 're-search-backward "a" 2 'move)
'(nil 1)
)

(qa
(qa272-run 're-search-backward "a" 3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 're-search-backward "a" 3 t)
'(nil 2)
)

(qa
(qa272-run 're-search-backward "a" 3 'move)
'(nil 1)
)

(qa
(qa272-run 'posix-search-backward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "a" 1 nil)
'(1 1)
)

(qa
(qa272-run 'posix-search-backward "a" 1 t)
'(1 1)
)

(qa
(qa272-run 'posix-search-backward "a" 1 'move)
'(1 1)
)

(qa
(qa272-run 'posix-search-backward "a" 2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'posix-search-backward "a" 2 t)
'(nil 2)
)

(qa
(qa272-run 'posix-search-backward "a" 2 'move)
'(nil 1)
)

(qa
(qa272-run 'posix-search-backward "a" 3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'posix-search-backward "a" 3 t)
'(nil 2)
)

(qa
(qa272-run 'posix-search-backward "a" 3 'move)
'(nil 1)
)

(qa
(qa272-run 'search-forward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 'search-forward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 'search-forward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 'search-forward "a" -1 nil)
'(1 1)
)

(qa
(qa272-run 'search-forward "a" -1 t)
'(1 1)
)

(qa
(qa272-run 'search-forward "a" -1 'move)
'(1 1)
)

(qa
(qa272-run 'search-forward "a" -2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'search-forward "a" -2 t)
'(nil 2)
)

(qa
(qa272-run 'search-forward "a" -2 'move)
'(nil 1)
)

(qa
(qa272-run 'search-forward "a" -3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'search-forward "a" -3 t)
'(nil 2)
)

(qa
(qa272-run 'search-forward "a" -3 'move)
'(nil 1)
)

(qa
(qa272-run 're-search-forward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 're-search-forward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 're-search-forward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 're-search-forward "a" -1 nil)
'(1 1)
)

(qa
(qa272-run 're-search-forward "a" -1 t)
'(1 1)
)

(qa
(qa272-run 're-search-forward "a" -1 'move)
'(1 1)
)

(qa
(qa272-run 're-search-forward "a" -2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 're-search-forward "a" -2 t)
'(nil 2)
)

(qa
(qa272-run 're-search-forward "a" -2 'move)
'(nil 1)
)

(qa
(qa272-run 're-search-forward "a" -3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 're-search-forward "a" -3 t)
'(nil 2)
)

(qa
(qa272-run 're-search-forward "a" -3 'move)
'(nil 1)
)

(qa
(qa272-run 'posix-search-forward "a" 0 nil)
'(2 2)
)

(qa
(qa272-run 'posix-search-forward "a" 0 t)
'(2 2)
)

(qa
(qa272-run 'posix-search-forward "a" 0 'move)
'(2 2)
)

(qa
(qa272-run 'posix-search-forward "a" -1 nil)
'(1 1)
)

(qa
(qa272-run 'posix-search-forward "a" -1 t)
'(1 1)
)

(qa
(qa272-run 'posix-search-forward "a" -1 'move)
'(1 1)
)

(qa
(qa272-run 'posix-search-forward "a" -2 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'posix-search-forward "a" -2 t)
'(nil 2)
)

(qa
(qa272-run 'posix-search-forward "a" -2 'move)
'(nil 1)
)

(qa
(qa272-run 'posix-search-forward "a" -3 nil)
'((search-failed "a") 2)
)

(qa
(qa272-run 'posix-search-forward "a" -3 t)
'(nil 2)
)

(qa
(qa272-run 'posix-search-forward "a" -3 'move)
'(nil 1)
)

(qa
(qa272-run 'search-backward "" 1 t)
'(2 2)
)

(qa
(qa272-run 'search-backward "" 2 t)
'(2 2)
)

(qa
(qa272-run 'search-backward "" 3 t)
'(2 2)
)

(qa
(qa272-run 'search-backward "a" 2 nil t)
'((search-failed "a") 9)
)

(qa
(qa272-run 'search-backward "a" 2 t t)
'(nil 9)
)

(qa
(qa272-run 'search-backward "a" 2 'move t)
'(nil 8)
)

(qa
(qa272-run 're-search-backward "" 1 t)
'(2 2)
)

(qa
(qa272-run 're-search-backward "" 2 t)
'(2 2)
)

(qa
(qa272-run 're-search-backward "" 3 t)
'(2 2)
)

(qa
(qa272-run 're-search-backward "a" 2 nil t)
'((search-failed "a") 9)
)

(qa
(qa272-run 're-search-backward "a" 2 t t)
'(nil 9)
)

(qa
(qa272-run 're-search-backward "a" 2 'move t)
'(nil 8)
)

(qa
(qa272-run 'posix-search-backward "" 1 t)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "" 2 t)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "" 3 t)
'(2 2)
)

(qa
(qa272-run 'posix-search-backward "a" 2 nil t)
'((search-failed "a") 9)
)

(qa
(qa272-run 'posix-search-backward "a" 2 t t)
'(nil 9)
)

(qa
(qa272-run 'posix-search-backward "a" 2 'move t)
'(nil 8)
)

(qa
(qa272-run 're-search-backward "^" 3 t)
'(1 1)
)

(qa
(qa272-run 're-search-backward "a\\|^" 3 t)
'(1 1)
)

(qa
(qa272-run 're-search-backward "\\(a\\)" 3 t)
'(nil 2)
)

(qa
(qa272-run 'posix-search-backward "^" 3 t)
'(1 1)
)

(qa
(qa272-run 'posix-search-backward "a\\|^" 3 t)
'(1 1)
)

(qa
(qa272-run 'posix-search-backward "\\(a\\)" 3 t)
'(nil 2)
)


(princ (qa--done))
