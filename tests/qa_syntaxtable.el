;; syntaxtable — syntax descriptor codes/flags, syntax-table objects and
;; their buffer-local installation, scanning edges (forward-symbol,
;; scan-sexps/scan-lists, parse-partial-sexp stop args), two-character
;; and generic comment/string delimiters, and case-table internals.
;; Complements qa_syntax.el (basic codes/motion) and qa_case.el (case
;; commands/comparison).
;;
;; GNU 31.1 facts pinned here:
;; - `string-to-syntax' packs flags as bits 16..22 on top of the class
;;   code; the char right after the code letter is the matching char.
;; - `syntax-after' returns a raw (CLASS . MATCH) descriptor or nil
;;   outside the accessible range.
;; - temp buffers share `standard-syntax-table' until `set-syntax-table'.
;; - `forward-sexp' never signals: it returns nil at a boundary.
;;   `scan-sexps' signals `scan-error' on unbalanced input but returns
;;   nil when the count is simply unreachable.
;; - in `skip-chars-*' a backslash merely quotes the next char: "\\sw"
;;   skips literal `w', NOT word constituents.
;; - `parse-partial-sexp' STOPBEFORE stops before completing a sexp;
;;   COMMENTSTOP (arg 6) stops the scan at a comment start.
;; - `syntax-table-p' and `case-table-p' both accept a fresh char-table
;;   of the matching subtype.
;; - `make-case-table' and `invert-case' do not exist in GNU 31.1.

;; == string-to-syntax: flags and matching char ==
(qa (string-to-syntax "<") '(11))          ; comment start
(qa (string-to-syntax "/") '(10))          ; character quote
(qa (string-to-syntax "( ") '(4))          ; match char may be whitespace
(qa (string-to-syntax ") ") '(5))
(qa (string-to-syntax "wx") '(2 . 120))    ; match char stored for any class
(qa (string-to-syntax "w1") '(2 . 49))
(qa (string-to-syntax "$$") '(8 . 36))
(qa (string-to-syntax "\"\"") '(7 . 34))
(qa (string-to-syntax "|x") '(15 . 120))
(qa (string-to-syntax "@ x") nil)          ; "@": ignore rest, inherit
;; flags: 1=65536 2=131072 3=262144 4=524288 b=2097152 n=4194304 p=1048576
(qa (string-to-syntax "w p") '(1048578))
(qa (string-to-syntax ". 1") '(65537))
(qa (string-to-syntax ". 2") '(131073))
(qa (string-to-syntax ". 3") '(262145))
(qa (string-to-syntax ". 4") '(524289))
(qa (string-to-syntax ". b") '(2097153))
(qa (string-to-syntax ". n") '(4194305))
(qa (string-to-syntax ". p") '(1048577))
(qa (string-to-syntax "< 1") '(65547))
(qa (string-to-syntax "<12") '(131083 . 49))  ; match '1', flag '2'
(qa (string-to-syntax "< 12") '(196619))
(qa (string-to-syntax "> 34") '(786444))
(qa (string-to-syntax ". 14b") '(2686977))
(qa (string-to-syntax ". 124b") '(2818049))
(qa (string-to-syntax ". 23n") '(4587521))
(qa (string-to-syntax "(1pb") '(3145732 . 49))
(qa-err-data (string-to-syntax "") 'error
             '("Invalid syntax description letter: \0"))

;; == syntax-class / syntax-after ==
(qa (syntax-class (string-to-syntax ". 1")) 1)     ; flags masked out
(qa (syntax-class (string-to-syntax ". 124b")) 1)
(qa (syntax-class (string-to-syntax "< 12")) 11)
(qa (syntax-class nil) nil)                       ; no error on nil
(qa-err-data (syntax-class 2) 'wrong-type-argument '(listp 2))
(qa (with-temp-buffer (insert "(a) x") (syntax-after 1)) '(4 . 41))
(qa (with-temp-buffer (insert "(a) x") (syntax-after 2)) '(2))
(qa (with-temp-buffer (insert "(a) x") (syntax-after (point-max))) nil)
(qa (with-temp-buffer (syntax-after 1)) nil)
(qa (with-temp-buffer (insert "x") (syntax-after 0)) nil)
(qa (with-temp-buffer (insert "x") (syntax-after 99)) nil)
(qa (with-temp-buffer (char-syntax ?\n)) ? )
(qa (with-temp-buffer (char-syntax ?\t)) ? )
(qa-err (with-temp-buffer (char-syntax)) 'wrong-number-of-arguments)
(qa-err-data (with-temp-buffer (char-syntax "a")) 'wrong-type-argument
             '(characterp "a"))
(qa-err-data (with-temp-buffer (char-syntax -1)) 'wrong-type-argument
             '(characterp -1))
(qa (matching-paren ?\") nil)

;; == syntax-table objects ==
(qa (eq (standard-syntax-table) (standard-syntax-table)) t)
;; a temp buffer's syntax table IS the shared standard table object
(qa (with-temp-buffer (eq (syntax-table) (standard-syntax-table))) t)
(qa (let* ((b1 (generate-new-buffer " *qa st1*"))
           (b2 (generate-new-buffer " *qa st2*"))
           (same (with-current-buffer b1
                   (eq (syntax-table) (with-current-buffer b2 (syntax-table))))))
      (kill-buffer b1) (kill-buffer b2) same)
    t)
;; copying nil copies the current buffer's table — a fresh object
(qa (with-temp-buffer (eq (copy-syntax-table nil) (standard-syntax-table))) nil)
(qa (syntax-table-p (copy-syntax-table nil)) t)
(qa (not (eq (copy-syntax-table nil) (copy-syntax-table nil))) t)
(qa-err-data (copy-syntax-table 5) 'wrong-type-argument '(syntax-table-p 5))
(qa (char-table-subtype (standard-syntax-table)) 'syntax-table)
(qa (char-table-p (standard-syntax-table)) t)
(qa (char-table-parent (standard-syntax-table)) nil)
;; a fresh char-table of subtype syntax-table already qualifies
(qa (syntax-table-p (make-char-table 'syntax-table nil)) t)
(qa (char-table-range (standard-syntax-table) ?a) '(2))
(qa (char-table-range (standard-syntax-table) ?\() '(4 . 41))
(qa (char-table-range (standard-syntax-table) ?\)) '(5 . 40))
;; make-syntax-table parents to the standard table
(qa (eq (char-table-parent (make-syntax-table)) (standard-syntax-table)) t)
(qa (char-table-range (make-syntax-table) ?a) '(2))
(qa-err-data (make-syntax-table 5) 'wrong-type-argument '(char-table-p 5))
;; copy chains to the original as parent
(qa (let ((st (copy-syntax-table)))
      (eq (char-table-parent st) (standard-syntax-table)))
    t)
;; set-char-table-range writes a raw descriptor
(qa (let ((st (copy-syntax-table)))
      (set-char-table-range st ?a (list 3))
      (with-syntax-table st (char-syntax ?a)))
    ?_)

;; == set-syntax-table / with-syntax-table ==
;; set-syntax-table returns the table it installed
(qa (with-temp-buffer
      (let ((st (copy-syntax-table)))
        (eq (set-syntax-table st) st)))
    t)
(qa-err-data (with-temp-buffer (set-syntax-table 5))
             'wrong-type-argument '(syntax-table-p 5))
(qa-err (with-temp-buffer (set-syntax-table)) 'wrong-number-of-arguments)
(qa (with-syntax-table (make-syntax-table) 42) 42)   ; returns last body value
(qa-err-data (with-syntax-table nil (char-syntax ?a))
             'wrong-type-argument '(syntax-table-p nil))
;; with-syntax-table restores even on non-local exit
(qa (let ((old (syntax-table)))
      (ignore-errors (with-syntax-table (make-syntax-table) (error "boom")))
      (eq (syntax-table) old))
    t)
;; ... and on normal exit
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?x "_" st)
      (with-temp-buffer
        (with-syntax-table st (char-syntax ?x))
        (char-syntax ?x)))
    ?w)
;; a buffer-local table does not leak into other buffers
(qa (with-temp-buffer
      (let ((st (copy-syntax-table)))
        (modify-syntax-entry ?x "_" st)
        (set-syntax-table st)
        (with-temp-buffer (char-syntax ?x))))
    ?w)
(qa (with-temp-buffer
      (let ((st (copy-syntax-table)))
        (modify-syntax-entry ?x "_" st)
        (set-syntax-table st)
        (list (char-syntax ?x) (eq (syntax-table) st))))
    '(?_ t))

;; == modify-syntax-entry ==
(qa (modify-syntax-entry ?z "w" (copy-syntax-table)) nil)   ; return value
(qa-err (modify-syntax-entry ?a) 'wrong-number-of-arguments)
(qa-err-data (modify-syntax-entry ?a "z" (copy-syntax-table))
             'error '("Invalid syntax description letter: z"))
(qa-err-data (modify-syntax-entry ?a "" (copy-syntax-table))
             'error '("Invalid syntax description letter: \0"))
(qa-err-data (modify-syntax-entry ?a nil (copy-syntax-table))
             'wrong-type-argument '(stringp nil))
(qa-err-data (modify-syntax-entry 'sym "w" (copy-syntax-table))
             'wrong-type-argument '(characterp sym))
(qa-err-data (modify-syntax-entry ?a "w" 'not-table)
             'wrong-type-argument '(syntax-table-p not-table))
(qa-err-data (modify-syntax-entry ?x ?w (copy-syntax-table))
             'wrong-type-argument '(stringp 119))
;; CHAR may be a (MIN . MAX) range; modify-syntax-entry returns nil
(qa (let ((st (copy-syntax-table)))
      (list (modify-syntax-entry '(?0 . ?9) "_" st)
            (with-syntax-table st (char-syntax ?5))))
    '(nil ?_))
;; "@ " entry means: inherit from the standard table
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?x "_" st)
      (modify-syntax-entry ?x "@ " st)
      (with-syntax-table st (char-syntax ?x)))
    ?w)
;; after a flag-string change, reading char-table-range shows raw entry
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?/ ". 14b" st)
      (char-table-range st ?/))
    '(2686977))

;; == char-equal ==
(qa (let ((case-fold-search t)) (char-equal ?a ?A)) t)
(qa (let ((case-fold-search t)) (char-equal ?\u00e9 ?\u00c9)) t)
(qa (let ((case-fold-search nil)) (char-equal ?\u00e9 ?\u00c9)) nil)
(qa (let ((case-fold-search nil)) (char-equal ?a ?a)) t)
(qa-err-data (char-equal "a" ?a) 'wrong-type-argument '(characterp "a"))
(qa-err (char-equal ?a) 'wrong-number-of-arguments)
;; char-equal does NOT consult custom case-table pairs
(qa (let ((ct (copy-case-table (standard-case-table))))
      (set-case-syntax-pair ?x ?Q ct)
      (with-case-table ct
        (let ((case-fold-search t)) (char-equal ?x ?q))))
    nil)

;; == skip-chars: backslash quotes the next char literally ==
;; "\\sw" is NOT a syntax class in skip-chars-*: it matches only `w'.
(qa (with-temp-buffer (insert "abc_ x") (goto-char 1)
                      (list (skip-chars-forward "\\sw") (point)))
    '(0 1))
(qa (with-temp-buffer (insert "wab") (goto-char 1)
                      (list (skip-chars-forward "\\sw") (point)))
    '(1 2))
(qa (with-temp-buffer (insert "  ab") (goto-char 1)
                      (list (skip-chars-forward "\\s ") (point)))
    '(2 3))
(qa (with-temp-buffer (insert "\tab") (goto-char 1)
                      (list (skip-chars-forward "\\s ") (point)))
    '(0 1))
(qa (with-temp-buffer (insert "__a") (goto-char 1)
                      (list (skip-chars-forward "\\s_") (point)))
    '(2 3))
(qa (with-temp-buffer (insert "((ab") (goto-char 1)
                      (list (skip-chars-forward "\\s(") (point)))
    '(2 3))
(qa (with-temp-buffer (insert "ab_ c") (goto-char 4)
                      (list (skip-chars-backward "\\sw") (point)))
    '(0 4))

;; == skip-syntax corners ==
(qa (with-temp-buffer (insert "a\\b") (goto-char 1)
                      (list (skip-syntax-forward "w") (point)))
    '(1 2))
(qa (with-temp-buffer (insert "foo") (goto-char 2)
                      (list (skip-syntax-forward "w" -1) (point)))
    '(0 2))
(qa (with-temp-buffer (insert "x") (goto-char 1) (skip-syntax-forward "z")) 0)
(qa (with-temp-buffer (insert "ab   cd") (goto-char 8)
                      (list (skip-syntax-backward "-" 5) (point)))
    '(0 8))

;; == word / symbol / sexp motion edges ==
;; forward-word returns t and treats nil COUNT as 1
(qa (with-temp-buffer (insert "ab c") (goto-char 1)
                      (list (forward-word nil) (point)))
    '(t 3))
;; digits and multibyte letters are word constituents
(qa (with-temp-buffer (insert "été x") (goto-char 1)
                      (list (forward-word) (point)))
    '(t 4))
(qa (with-temp-buffer (insert "ab 日本語") (goto-char 1)
                      (list (forward-word 2) (point)))
    '(t 7))
(qa (with-temp-buffer (insert "日本語 test") (goto-char 1)
                      (list (forward-word) (point)))
    '(t 4))
(qa (with-temp-buffer (insert "a b c") (goto-char 6)
                      (list (backward-word 2) (point)))
    '(t 3))
;; forward-symbol skips word AND symbol-constituent chars; its return
;; value is the new point going forward, nil going backward
(qa (with-temp-buffer (insert "foo_bar baz") (goto-char 1)
                      (list (forward-symbol 1) (point)))
    '(8 8))
(qa (with-temp-buffer (insert "foo123-bar") (goto-char 1)
                      (list (forward-symbol 1) (point)))
    '(11 11))
(qa (with-temp-buffer (insert "foo bar") (goto-char 7)
                      (list (forward-symbol -1) (point)))
    '(nil 5))
(qa (with-temp-buffer (insert "foo") (goto-char 1)
                      (list (forward-symbol 5) (point)))
    '(nil 4))
(qa (with-temp-buffer (insert "  foo") (goto-char 1)
                      (list (forward-symbol 1) (point)))
    '(6 6))
;; `_' is symbol-constituent, not word: word stops, sexp doesn't
(qa (with-temp-buffer (insert "a_b c") (goto-char 1)
                      (list (forward-sexp) (point)))
    '(nil 4))
;; forward-sexp returns nil even on success; never signals at boundary
(qa (with-temp-buffer (insert "(a) b") (goto-char 1) (forward-sexp)) nil)
(qa (with-temp-buffer (insert "a b") (goto-char 1)
                      (list (forward-sexp 5) (point)))
    '(nil 4))
(qa (with-temp-buffer (insert "a b") (goto-char 1)
                      (list (forward-sexp 0) (point)))
    '(nil 1))
(qa (with-temp-buffer (insert "a b") (goto-char 4)
                      (list (backward-sexp 5) (point)))
    '(nil 1))
;; a paren inside a string is not a delimiter
(qa (with-temp-buffer (insert "\"a ( b\" x") (goto-char 1)
                      (list (forward-sexp) (point)))
    '(nil 8))
;; narrowing bounds the scan
(qa (with-temp-buffer
      (insert "ab (cd) ef")
      (narrow-to-region 4 8)
      (goto-char (point-min))
      (list (forward-sexp) (point)))
    '(nil 8))
(qa-err-data (with-temp-buffer
               (insert "(ab") (narrow-to-region 1 3)
               (goto-char 1) (forward-list))
             'scan-error '("Unbalanced parentheses" 1 3))
;; forward-list/backward-list at top level step over one sexp and
;; return the new point
(qa (with-temp-buffer (insert "x") (goto-char 1)
                      (list (forward-list) (point)))
    '(2 2))
(qa (with-temp-buffer (insert "x") (goto-char 2)
                      (list (backward-list) (point)))
    '(1 1))
(qa (default-value 'open-paren-in-column-0-is-defun-start) t)

;; == scan-sexps / scan-lists edges ==
(qa (with-temp-buffer (insert "(a b) c") (scan-sexps 1 5)) nil)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-sexps 4 1)) 7)
;; backward scan reports the position before the sexp start
(qa (with-temp-buffer (insert "a (b) c") (scan-sexps 5 -1)) 4)
(qa (with-temp-buffer (insert "(a) b") (scan-sexps 5 -5)) nil)
(qa-err-data (with-temp-buffer (insert "(a b") (scan-sexps 1 1))
             'scan-error '("Unbalanced parentheses" 1 5))
(qa-err-data (with-temp-buffer (scan-sexps "a" 1))
             'wrong-type-argument '(fixnump "a"))
(qa-err-data (with-temp-buffer (insert "(a (b) c)") (scan-lists 4 1 2))
             'scan-error '("Unbalanced parentheses" 4 10))
(qa (with-temp-buffer (insert "(a") (scan-lists 1 -1 0)) nil)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-lists 1 1 -1)) 2)
(qa (get 'scan-error 'error-message) "Scan error")
(qa (get 'scan-error 'error-conditions) '(scan-error error))

;; == parse-partial-sexp stop arguments ==
;; STOPBEFORE: stop before completing a sexp — here the list ends at 5
(qa (with-temp-buffer (insert "(a b) (c d)")
                      (parse-partial-sexp 1 6 nil t))
    '(0 nil nil nil nil nil 0 nil nil nil nil))
;; COMMENTSTOP (arg 6): scan stops at the comment start, elt 4 = t
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (parse-partial-sexp 1 6 nil nil nil 'syntax-table))
    '(0 nil 1 nil t nil 0 nil 5 nil nil))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (parse-partial-sexp 1 6 nil nil nil t))
    '(0 nil 1 nil t nil 0 nil 5 nil nil))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a ;; hi\nb)")
      (parse-partial-sexp 1 5 nil nil nil 'syntax-table))
    '(1 1 2 nil t nil 0 nil 4 (1) nil))
;; OLDSTATE must be a list (or nil)
(qa-err-data (with-temp-buffer (insert "(a)")
                               (parse-partial-sexp 1 4 nil nil 'foo))
             'wrong-type-argument '(listp foo))
(qa-err-data (with-temp-buffer (insert "(a)")
                               (parse-partial-sexp 1 4 nil nil 5))
             'wrong-type-argument '(listp 5))
(qa-err-data (with-temp-buffer (insert "abcd") (parse-partial-sexp 3 1))
             'error '("End position is smaller than start position"))
(qa-err (with-temp-buffer (insert "ab") (parse-partial-sexp 1 99))
        'args-out-of-range)
(qa-err (with-temp-buffer (insert "ab") (syntax-ppss 99)) 'args-out-of-range)

;; == syntax-ppss extras ==
(qa (fboundp 'syntax-ppss-depth) t)
(qa (fboundp 'syntax-ppss-context) t)
(qa (fboundp 'syntax-ppss-toplevel-pos) t)
(qa (fboundp 'syntax-ppss-flush-cache) t)
(qa (with-temp-buffer (insert "(a (b c) d)") (syntax-ppss-depth (syntax-ppss 6))) 2)
(qa (with-temp-buffer (insert "(a (b c) d)")
                      (syntax-ppss-toplevel-pos (syntax-ppss 6)))
    1)
(qa (with-temp-buffer (insert "\"ab\" c") (syntax-ppss-context (syntax-ppss 3))) 'string)
(qa (with-temp-buffer
      (emacs-lisp-mode) (insert "a ;; b\nc")
      (syntax-ppss-context (syntax-ppss 7)))
    'comment)
;; unterminated lists: elt 9 lists the open positions innermost-last
(qa (with-temp-buffer (insert "(a (b) c") (syntax-ppss 9))
    '(1 1 8 nil nil nil 0 nil nil (1) nil))
;; elt 10 non-nil while inside a prefix character position
(qa (with-temp-buffer (emacs-lisp-mode) (insert "#'(a)") (syntax-ppss 5))
    '(1 3 4 nil nil nil 0 nil nil (3) nil))
;; parse-sexp-ignore-comments: nil = comments are sexps
(qa (default-value 'parse-sexp-ignore-comments) nil)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "a ;; c\nb")
      (goto-char 1)
      (let ((parse-sexp-ignore-comments t))
        (list (forward-sexp 2) (point))))
    '(nil 9))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "a ;; c\nb")
      (goto-char 1)
      (let ((parse-sexp-ignore-comments nil))
        (list (forward-sexp 2) (point))))
    '(nil 7))

;; == forward-comment extras ==
(qa (with-temp-buffer
      (emacs-lisp-mode) (insert ";; c\nx") (goto-char 1)
      (list (forward-comment 0) (point)))
    '(t 1))
(qa (with-temp-buffer
      (emacs-lisp-mode) (insert ";; a\n;; b\nx") (goto-char 1)
      (list (forward-comment 2) (point)))
    '(t 11))
(qa (with-temp-buffer
      (emacs-lisp-mode) (insert "x ; c\ny") (goto-char 7)
      (list (forward-comment -1) (point)))
    '(t 3))
(qa (with-temp-buffer
      (emacs-lisp-mode) (insert "x") (goto-char 2)
      (list (forward-comment 1) (point)))
    '(nil 2))

;; == two-char / generic delimiters via custom tables ==
;; C-style /* */ needs flags: '/' gets ". 14b", '*' gets ". 23"
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?/ ". 14b" st)
      (modify-syntax-entry ?* ". 23" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "/* hi */ x")
        (goto-char 1)
        (list (forward-comment 1) (point))))
    '(t 9))
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?/ ". 14b" st)
      (modify-syntax-entry ?* ". 23" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "/* c */ x")
        (syntax-ppss 4)))
    '(0 nil nil nil t nil 0 nil 1 nil nil))
;; 'n' flag makes the b-style comment nestable
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?/ ". 14b" st)
      (modify-syntax-entry ?* ". 23bn" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "/* a /* b */ c */ x")
        (goto-char 1)
        (list (forward-comment 1) (point))))
    '(t 18))
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?/ ". 14b" st)
      (modify-syntax-entry ?* ". 23b" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "/* a /* b */ c */ x")
        (goto-char 1)
        (list (forward-comment 1) (point))))
    '(t 13))
;; generic comment delimiter '!': comment runs to EOL; elt 7 is
;; 'syntax-table (not a style number)
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?! "!" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "a! rest")
        (syntax-ppss 5)))
    '(0 nil 1 nil t nil 0 syntax-table 2 nil nil))
;; generic string delimiter '|': elt 3 is t, not the delimiter char
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?| "|" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "|str| x")
        (list (syntax-ppss 3)
              (progn (goto-char 1) (list (forward-sexp) (point))))))
    '((0 nil nil t nil nil 0 nil 1 nil nil) (nil 6)))
;; 'p' flag on punctuation makes it an expression prefix: "#(...)"
;; scans as one sexp even with '#' punctuation
(qa (let ((st (copy-syntax-table)))
      (modify-syntax-entry ?# ". p" st)
      (with-temp-buffer
        (set-syntax-table st)
        (insert "#(a b) x")
        (goto-char 1)
        (list (forward-sexp) (point))))
    '(nil 7))

;; == case tables ==
(qa (char-table-subtype (standard-case-table)) 'case-table)
(qa (char-table-p (standard-case-table)) t)
(qa (char-table-parent (standard-case-table)) nil)
;; a fresh case-table subtype char-table qualifies too
(qa (case-table-p (make-char-table 'case-table nil)) t)
;; char-table-range on a case table reads the DOWNCASE mapping:
;; both 'a' and 'A' map to 97
(qa (char-table-range (standard-case-table) ?a) ?a)
(qa (char-table-range (standard-case-table) ?A) ?a)
(qa-err-data (with-temp-buffer (set-case-table 5))
             'wrong-type-argument '(case-table-p 5))
(qa-err (with-temp-buffer (set-case-table)) 'wrong-number-of-arguments)
(qa (case-table-p (with-temp-buffer
                    (set-case-table (copy-case-table (standard-case-table)))))
    t)
(qa (with-case-table (standard-case-table) 7) 7)
;; set-case-syntax-pair UC LC: upcase(LC)=UC, downcase(UC)=LC
(qa (let ((ct (copy-case-table (standard-case-table))))
      (set-case-syntax-pair ?x ?Q ct)
      (with-case-table ct
        (list (upcase ?Q) (downcase ?x) (upcase ?x) (downcase ?Q))))
    '(?x ?Q ?x ?Q))
;; set-case-syntax: char becomes case-invariant
(qa (let ((ct (copy-case-table (standard-case-table))))
      (set-case-syntax ?z "_" ct)
      (with-case-table ct (list (upcase ?z) (downcase ?z) (upcase "z"))))
    '(?z ?z "z"))
;; set-case-syntax-delims: paired chars with no case conversion
(qa (let ((ct (copy-case-table (standard-case-table))))
      (set-case-syntax-delims ?\[ ?\] ct)
      (with-case-table ct (list (upcase ?\[) (downcase ?\[) (upcase "a[b]"))))
    '(?\[ ?\[ "A[B]"))
;; set-standard-case-table affects fresh buffers' current-case-table
(qa (let ((orig (copy-case-table (standard-case-table)))
          (ct (copy-case-table (standard-case-table))))
      (set-case-syntax-pair ?x ?Q ct)
      (set-standard-case-table ct)
      (unwind-protect
          (list (with-temp-buffer (eq (current-case-table) ct))
                (with-temp-buffer (upcase ?Q))
                (with-temp-buffer (downcase ?x)))
        (set-standard-case-table orig)))
    '(t ?x ?Q))
;; buffer-local set-case-table doesn't change the standard table
(qa (with-temp-buffer
      (let ((ct (copy-case-table (standard-case-table))))
        (set-case-syntax-pair ?z ?Z ct)
        (set-case-table ct)
        (eq (current-case-table) ct)))
    t)
(qa (with-temp-buffer (insert "hello world")
                      (upcase-initials-region 1 12) (buffer-string))
    "Hello World")
;; functions that do not exist in GNU 31.1
(qa (fboundp 'make-case-table) nil)
(qa (fboundp 'invert-case) nil)

(princ (qa--done))
