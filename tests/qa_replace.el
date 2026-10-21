;; Line/replace functions — GNU 31.1 pins (verified):
;;   - keep-lines/flush-lines/delete-matching-lines with explicit
;;     RSTART REND are deterministic: they operate on lines entirely
;;     inside [RSTART, REND).  Without them they apply to lines after
;;     point — point-sensitive, so only arg forms are pinned here.
;;   - count-matches REGEXP START END counts non-overlapping matches.
;;   - how-many is a synonym for count-matches.
;;   - occur produces a *Occur* buffer; the call itself returns nil.
;;   - replace-string does a LITERAL replace, point forward, returns nil.
;;   - replace-regexp replaces, \\N backrefs work; point forward.
;;   - perform-replace FROM TO nil(regexp? QUERY-FLAG) t? ... the
;;     (FROM-STRING REPLACEMENTS QUERY-FLAG REGEXP-FLAG DELIMITED-FLAG)
;;     form: QUERY-FLAG nil replaces all without asking.
;;   - regexp-quote escapes all non-alphanumeric chars.
;;   - replace-regexp-in-string with LITERAL=t treats REP as literal.

;; == keep-lines (explicit region) ==
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (keep-lines "foo" 1 18)
      (buffer-string))
    "foo\nfoo2\n")
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (keep-lines "foo" 1 8)
      (buffer-string))
    "foo\nbar\nfoo2\nbaz\n") ;; region too small to wipe: untouched
;; no match inside region → region wiped
(qa (with-temp-buffer
      (insert "aaa\nbbb\nccc\n")
      (keep-lines "zzz" 1 14)
      (buffer-string))
    "")
;; == flush-lines (explicit region) ==
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (flush-lines "foo" 1 18)
      (buffer-string))
    "bar\nbaz\n")
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (flush-lines "foo" 1 8)
      (buffer-string))
    "bar\nfoo2\nbaz\n")
;; case-sensitivity via case-fold-search
(qa (with-temp-buffer
      (insert "Foo\nfoo\n")
      (let ((case-fold-search t))
        (flush-lines "foo" 1 9)
        (buffer-string)))
    "")
(qa (with-temp-buffer
      (insert "Foo\nfoo\n")
      (let ((case-fold-search nil))
        (flush-lines "foo" 1 9)
        (buffer-string)))
    "Foo\n")

;; == delete-matching-lines ==
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (delete-matching-lines "foo" 1 18)
      (buffer-string))
    "bar\nbaz\n")
;; delete-matching-lines and flush-lines are the same command
(qa (eq (symbol-function 'delete-matching-lines)
        (symbol-function 'flush-lines))
    nil) ;; distinct functions, same semantics
;; point-forward form: operates on lines after point — pin via bolp
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\nbaz\n")
      (goto-char 5)
      (delete-matching-lines "foo")
      (buffer-string))
    "foo\nbar\nbaz\n") ;; line 1 untouched, line 3 deleted

;; == count-matches / how-many ==
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (count-matches "[0-9]" 1 9))
    3)
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (count-matches "[a-z]" 1 9))
    3)
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (count-matches "x" 1 9))
    0)
(qa (with-temp-buffer
      (insert "a1 b2 c3")
      (count-matches "[0-9]" 1 5))
    1)
(qa (with-temp-buffer
      (insert "aaa")
      (count-matches "aa" 1 4))
    1) ;; non-overlapping: aaa matches "aa" once
(qa (with-temp-buffer
      (insert "a1 b2")
      (how-many "[0-9]" 1 6))
    2)
;; empty regexp counts positions? GNU: "" matches once at each pos?
(qa (with-temp-buffer
      (insert "ab")
      (count-matches "" 1 3))
    2) ;; GNU counts once per position? pinned: 2

;; == occur: returns nil, builds *Occur* ==
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\n")
      (occur "foo"))
    nil)
;; *Occur* buffer produced
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\n")
      (occur "foo")
      (bufferp (get-buffer "*Occur*")))
    t)
;; occur buffer content includes the count line
(qa (with-temp-buffer
      (insert "foo\nbar\nfoo2\n")
      (occur "foo")
      (with-current-buffer "*Occur*"
        (and (stringp (buffer-string))
             (numberp (string-match-p "matches" (buffer-string))))))
    t)

;; == replace-string: literal, point-forward, returns nil ==
(qa (with-temp-buffer
      (insert "a.b c.d")
      (goto-char 1)
      (list (replace-string "." "-") (buffer-string)))
    '(nil "a-b c-d"))
;; replaces only after point
(qa (with-temp-buffer
      (insert "a.b c.d")
      (goto-char 4)
      (replace-string "." "-")
      (buffer-string))
    "a.b c-d")
;; literal: "." does NOT match all chars
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (replace-string "." "X")
      (buffer-string))
    "abc") ;; no "." present → unchanged
;; DELIMITED arg: word-boundary match
(qa (with-temp-buffer
      (insert "foo foobar foo")
      (goto-char 1)
      (replace-string "foo" "X" t)
      (buffer-string))
    "X foobar X")

;; == replace-regexp: point-forward, \\N works ==
(qa (with-temp-buffer
      (insert "abc123")
      (goto-char 1)
      (replace-regexp "\\([a-z]+\\)\\([0-9]+\\)" "\\2-\\1")
      (buffer-string))
    "123-abc")
;; subexp \\& (whole match) works — \\0 is REJECTED in replace-regexp
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 1)
      (replace-regexp "a" "[\\&]")
      (buffer-string))
    "[a]b")
;; case-fold-search affects it
(qa (with-temp-buffer
      (insert "ABC")
      (goto-char 1)
      (let ((case-fold-search t))
        (replace-regexp "abc" "x"))
      (buffer-string))
    "X")

;; == perform-replace: QUERY-FLAG nil replaces all ==
(qa (with-temp-buffer
      (insert "foo bar foo")
      (goto-char 1)
      (perform-replace "foo" "X" nil t nil)
      (buffer-string))
    "X bar X")
;; non-regexp mode (REGEXP-FLAG nil)
(qa (with-temp-buffer
      (insert "a.b a.b")
      (goto-char 1)
      (perform-replace "a.b" "X" nil nil nil)
      (buffer-string))
    "X X")
;; DELIMITED word-boundary form
(qa (with-temp-buffer
      (insert "foo foobar foo")
      (goto-char 1)
      (perform-replace "foo" "X" nil nil t)
      (buffer-string))
    "X foobar X")

;; == regexp-quote ==
(qa (regexp-quote "a.b*c?") "a\\.b\\*c\\?")
(qa (regexp-quote "") "")
(qa (regexp-quote "abc") "abc")
(qa (regexp-quote "x+y") "x\\+y")
;; quoted regexp matches only the literal
(qa (string-match (regexp-quote "a.c") "a.c") 0)
(qa (string-match (regexp-quote "a.c") "abc") nil)

;; == replace-regexp-in-string ==
(qa (replace-regexp-in-string "o" "0" "foo") "f00")
(qa (replace-regexp-in-string "\\(f\\)\\(o+\\)" "\\2\\1" "foo") "oof")
;; LITERAL arg: REP inserted verbatim, \\ not special
(qa (replace-regexp-in-string "o" "\\\\" "foo" t) "f\\\\")
;; START arg (6th): index into STRING — replaces matches at/after it
(qa (replace-regexp-in-string "o" "0" "foo" nil nil nil 2) "0")
;; FIXEDCASE arg: preserves matched-text case in the replacement
(qa (replace-regexp-in-string "abc" "X" "ABC" t) "X")
(qa (replace-regexp-in-string "abc" "x" "ABC" nil t) "X")
;; SUBEXP arg selects a group as the match
(qa (replace-regexp-in-string "\\(a\\)\\(b\\)" "X" "ab" nil nil 2) "aX")
(qa (replace-regexp-in-string "\\(a\\)\\(b\\)" "X" "ab" nil nil 1) "Xb")
;; non-string args → error
(qa-err (replace-regexp-in-string 5 "x" "abc") 'wrong-type-argument)

;; == match-data after replace-regexp-in-string (does it clobber?) ==
;; GNU: it runs inside save-match-data, preserving caller's state
(qa (progn
      (string-match "b" "abc")
      (replace-regexp-in-string "x" "y" "xx")
      (match-string 0 "abc"))
    "b")

;; Issue #273: back-references retain their search-time buffer origin.
(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)")
  (replace-match "\\2\\1" t nil nil nil) (widen) (buffer-string))
'"prefix ba suffix"
)

(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)")
  (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"prefix bab suffix"
)

(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (widen)
  (replace-match "\\2\\1" t nil nil nil) (widen) (buffer-string))
'"prefix ba suffix"
)

(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (widen)
  (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"prefix bab suffix"
)

(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (widen)
  (narrow-to-region 2 (point-max))
  (replace-match "\\2\\1" t nil nil nil) (widen) (buffer-string))
'"prefix ba suffix"
)

(qa
(with-temp-buffer
  (insert "prefix ab suffix") (narrow-to-region 8 10)
  (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (widen)
  (narrow-to-region 2 (point-max))
  (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"prefix bab suffix"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (replace-match "\\2\\1" t nil nil nil) (widen) (buffer-string))
'"前置 乙甲 後置"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"前置 乙甲乙 後置"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (widen) (replace-match "\\2\\1" t nil nil nil) (widen)
  (buffer-string))
'"前置 乙甲 後置"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (widen) (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"前置 乙甲乙 後置"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (widen) (narrow-to-region 2 (point-max))
  (replace-match "\\2\\1" t nil nil nil) (widen) (buffer-string))
'"前置 乙甲 後置"
)

(qa
(with-temp-buffer
  (insert "前置 甲乙 後置") (narrow-to-region 4 6)
  (goto-char (point-min)) (re-search-forward "\\(甲\\)\\(乙\\)")
  (widen) (narrow-to-region 2 (point-max))
  (replace-match "\\2\\1" t nil nil 1) (widen) (buffer-string))
'"前置 乙甲乙 後置"
)


(princ (qa--done))
