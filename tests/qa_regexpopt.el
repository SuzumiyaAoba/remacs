;; regexp-opt.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - regexp-opt builds a non-capturing group regexp by default.
;;   - PAREN arg t → capture-group \(...\); 'words → \<...\>.
;;   - regexp-opt-depth counts CAPTURING groups only (0 for \(?:\)).
;;   - regexp-opt-quote is NOT a fn; use regexp-quote.
;;   - regexp-opt-charset is a fn for charset construction.

(require 'regexp-opt)

;; == surface ==
(qa (fboundp 'regexp-opt) t)
(qa (fboundp 'regexp-opt-depth) t)
(qa (fboundp 'regexp-opt-charset) t)
(qa (fboundp 'regexp-opt-group) t)
(qa (fboundp 'regexp-opt-quote) nil)
(qa (fboundp 'regexp-opt-time) nil)
(qa (fboundp 'regexp-opt-try-suffix) nil)

;; == regexp-opt builds non-capturing group by default ==
(qa (regexp-opt '("foo" "bar")) "\\(?:bar\\|foo\\)")
(qa (regexp-opt '("foo" "bar") t) "\\(bar\\|foo\\)")
(qa (regexp-opt '("foo" "bar") 'words) "\\<\\(bar\\|foo\\)\\>")
;; special chars escaped
(qa (regexp-opt '("a.b" "c*d")) "\\(?:a\\.b\\|c\\*d\\)")
;; single-element list → just the (escaped) string in group
(qa (regexp-opt '("hello")) "\\(?:hello\\)")
;; empty list → matches nothing (or empty group)
(qa (regexp-opt '()) "\\(?:\\`a\\`\\)")
;; single-char strings collapse into a sorted charset
(qa (regexp-opt '("b" "a")) "[ab]")

;; == regexp-opt-depth ==
;; counts CAPTURING \(...\) groups only
(qa (regexp-opt-depth "abc") 0)
(qa (regexp-opt-depth "\\(a\\)") 1)
(qa (regexp-opt-depth "\\(a\\|b\\)") 1)
(qa (regexp-opt-depth "\\(?:a\\|b\\)") 0)
(qa (regexp-opt-depth "\\(\\(a\\)\\)") 2)
;; regexp-opt output itself is non-capturing
(qa (regexp-opt-depth (regexp-opt '("foo" "bar"))) 0)
;; with PAREN → 1 capturing group
(qa (regexp-opt-depth (regexp-opt '("foo" "bar") t)) 1)
;; escaped parens don't count
(qa (regexp-opt-depth "a\\(b\\)") 1)
(qa (regexp-opt-depth "a\\\\(b\\\\)") 0)
;; character-class brackets don't count
(qa (regexp-opt-depth "[abc]") 0)

;; == regexp-opt-quote — not a fn; regexp-quote is ==
(qa (fboundp 'regexp-quote) t)
(qa (regexp-quote "a.b*c") "a\\.b\\*c")
(qa (regexp-quote "abc") "abc")

;; == regexp-opt-charset ==
;; builds a character-class regexp for a set of chars
(qa (regexp-opt-charset '(?a ?b ?c)) "[abc]")
;; range gets merged
(qa (regexp-opt-charset '(?a ?b ?c ?d ?e)) "[a-e]")
;; with specials — they get escaped properly
(let ((r (regexp-opt-charset '(?a ?b ?\] ?\\))))
  ;; result is a string starting with [ and ending with ]
  (qa (string-match-p "\\`\\[.*\\]\\'" r) 0))

;; == common real-world use ==
;; match any keyword
(let ((kw-re (regexp-opt '("if" "else" "while") 'words)))
  (qa (string-match-p kw-re "if x then") 0)
  (qa (string-match-p kw-re "while loop") 0)
  (qa (string-match-p kw-re "iffy") nil))  ; 'words' prevents partial match

;; without 'words, partial matches
(let ((kw-re (regexp-opt '("if" "else" "while"))))
  (qa (string-match-p kw-re "iffy") 0))

;; == regexp-opt-group ==
;; single-char strings collapse into a charset — no group needed
(let ((g (regexp-opt-group '("a" "b") t)))
  (qa g "\\(?:[ab]\\)"))
;; default: bare charset
(let ((g (regexp-opt-group '("a" "b"))))
  (qa g "[ab]"))
;; for strings → alternation with explicit \\(?:...)\\) wrapper
(let ((g (regexp-opt-group '("foo" "bar") t)))
  (qa g "\\(?:foo\\|bar\\)"))
(let ((g (regexp-opt-group '("foo" "bar"))))
  (qa g "foo\\|bar"))

(princ (qa--done))
