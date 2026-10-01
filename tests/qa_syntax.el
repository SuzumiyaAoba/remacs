;; syntax — syntax tables, descriptors, char skipping, sexp/list/word
;; motion, defun motion, scan state (syntax-ppss), forward-comment.
;; NOTE: a temp buffer's default syntax table is `standard-syntax-table',
;; where `;', `#' and `' are punctuation; comment/prefix syntax needs
;; emacs-lisp-mode or a modified table.

;; == string-to-syntax ==
(qa (string-to-syntax "-") '(0))          ; whitespace (also " ")
(qa (string-to-syntax " ") '(0))
(qa (string-to-syntax ".") '(1))          ; punctuation
(qa (string-to-syntax "w") '(2))          ; word
(qa (string-to-syntax "_") '(3))          ; symbol constituent
(qa (string-to-syntax "(") '(4))          ; open, no matching char
(qa (string-to-syntax ")") '(5))          ; close
(qa (string-to-syntax "'") '(6))          ; expression prefix
(qa (string-to-syntax "\"") '(7))         ; string quote
(qa (string-to-syntax "$") '(8))          ; paired delimiter
(qa (string-to-syntax "\\") '(9))         ; escape
(qa (string-to-syntax "-") '(0))          ; whitespace
(qa (string-to-syntax ".") '(1))          ; punctuation
(qa (string-to-syntax ">") '(12))         ; comment end
(qa (string-to-syntax "!") '(14))         ; generic comment delimiter
(qa (string-to-syntax "|") '(15))         ; generic string delimiter
(qa (string-to-syntax "()") '(4 . 41))    ; open with matching char
(qa (string-to-syntax "(1") '(4 . 49))
(qa (string-to-syntax ")4") '(5 . 52))
(qa (string-to-syntax "(n") '(4 . 110))   ; matching char needn't be bracket
(qa (string-to-syntax "@") nil)           ; inherit alone => nil
(qa-err-data (string-to-syntax "z") 'error
             '("Invalid syntax description letter: z"))
(qa-err-data (string-to-syntax "#") 'error
             '("Invalid syntax description letter: #"))
(qa-err-data (string-to-syntax "&") 'error
             '("Invalid syntax description letter: &"))
(qa (syntax-class (string-to-syntax "w")) 2)
(qa (syntax-class (string-to-syntax "(1")) 4)
(qa (syntax-class (string-to-syntax "-")) 0)

;; == char-syntax / standard-syntax-table ==
;; In a temp buffer the default table is `standard-syntax-table'.
(qa (with-temp-buffer (char-syntax ?a)) ?w)
(qa (with-temp-buffer (char-syntax ?0)) ?w)
(qa (with-temp-buffer (char-syntax ? )) ? )
(qa (with-temp-buffer (char-syntax ?_)) ?_)
(qa (with-temp-buffer (char-syntax ?.)) ?.)
(qa (with-temp-buffer (char-syntax ?\( )) ?\()
(qa (with-temp-buffer (char-syntax ?\) )) ?\))
(qa (with-temp-buffer (char-syntax ?\" )) ?\")
(qa (with-temp-buffer (char-syntax ?\\)) ?\\)
;; `;', `#' and `' are punctuation in the standard table (unlike
;; emacs-lisp-mode where `;' starts comments and `' is a prefix).
(qa (with-temp-buffer (char-syntax ?\;)) ?.)
(qa (with-temp-buffer (char-syntax ?#)) ?.)
(qa (with-temp-buffer (char-syntax ?')) ?.)
(with-temp-buffer
  (emacs-lisp-mode)
  (qa (char-syntax ?\;) ?<)
  (qa (char-syntax ?\n) ?>)
  (qa (char-syntax ?') ?\' )
  (qa (eq (syntax-table) emacs-lisp-mode-syntax-table) t))
(qa (with-temp-buffer (char-to-string (char-syntax ?a))) "w")
(qa (with-temp-buffer (char-to-string (char-syntax ? ))) " ")
(qa (with-temp-buffer (char-to-string (char-syntax ?_))) "_")
(qa (matching-paren ?\( ) ?\) )
(qa (matching-paren ?\) ) ?\( )
(qa (matching-paren ?a) nil)

;; == table constructors & predicates ==
(qa (syntax-table-p (standard-syntax-table)) t)
(qa (syntax-table-p (make-syntax-table)) t)
(qa (syntax-table-p (with-temp-buffer (syntax-table))) t)
(qa (syntax-table-p nil) nil)
(qa (syntax-table-p 5) nil)
;; make-syntax-table inherits standard-syntax-table when arg is nil.
(qa (with-syntax-table (make-syntax-table) (char-syntax ?a)) ?w)
(qa (with-syntax-table (make-syntax-table nil) (char-syntax ?a)) ?w)
(qa (with-syntax-table (make-syntax-table (standard-syntax-table))
      (char-syntax ?a))
    ?w)
;; modify-syntax-entry returns nil; the table arg is required for
;; non-current tables.
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (list (modify-syntax-entry ?0 "w" st)
            (with-syntax-table st (char-syntax ?0))))
    '(nil 119))
;; modifying a copy does not alter the standard table
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?_ "w" st)
      (list (with-syntax-table st (char-syntax ?_))
            (with-syntax-table (standard-syntax-table) (char-syntax ?_))))
    '(119 95))
;; assorted descriptor letters through modify-syntax-entry
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?a "_" st)
      (with-syntax-table st (char-syntax ?a)))
    ?_)
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?! "'" st)
      (with-syntax-table st (char-syntax ?!)))
    ?')
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?/ "\\" st)
      (with-syntax-table st (char-syntax ?/)))
    ?\\)
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?| "\"" st)
      (with-syntax-table st (char-syntax ?|)))
    ?\")
;; "@ " inherits from standard-syntax-table; `#' is punctuation there.
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?# "w" st)
      (modify-syntax-entry ?# "@ " st)
      (with-syntax-table st (char-syntax ?#)))
    ?.)
;; CHAR may be a (MIN . MAX) range
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry '(?0 . ?9) "_" st)
      (with-syntax-table st (list (char-syntax ?0) (char-syntax ?9))))
    '(?_ ?_))
;; custom matching pair: make <> parens
(qa (let ((st (copy-syntax-table (standard-syntax-table))))
      (modify-syntax-entry ?< "(>" st)
      (modify-syntax-entry ?> ")<" st)
      (with-syntax-table st
        (with-temp-buffer
          (insert "<x>")
          (goto-char 1)
          (forward-sexp)
          (point))))
    4)
;; set-syntax-table / with-syntax-table
(qa (with-temp-buffer
      (let ((st (copy-syntax-table (standard-syntax-table))))
        (set-syntax-table st)
        (eq (syntax-table) st)))
    t)
(qa (let ((old (syntax-table)))
      (with-syntax-table (copy-syntax-table (standard-syntax-table))
        (char-syntax ?a))
      (eq (syntax-table) old))
    t)

;; == skip-chars-forward / skip-chars-backward ==
(qa (with-temp-buffer (insert "aaabbb") (goto-char 1)
                      (skip-chars-forward "a"))
    3)
(qa (with-temp-buffer (insert "aaabbb") (goto-char 1)
                      (skip-chars-forward "a") (point))
    4)
(qa (with-temp-buffer (insert "aaabbb") (goto-char 1)
                      (skip-chars-forward "a-z"))
    6)
(qa (with-temp-buffer (insert "abc123") (goto-char 1)
                      (skip-chars-forward "^0-9"))
    3)
(qa (with-temp-buffer (insert "aaaabbb") (goto-char 1)
                      (list (skip-chars-forward "a" 3) (point)))
    '(2 3))
(qa (with-temp-buffer (insert "a1b2") (goto-char 1)
                      (skip-chars-forward "a-z0-9"))
    4)
(qa (with-temp-buffer (insert "abc  ") (goto-char 5)
                      (list (skip-chars-backward " ") (point)))
    '(-1 4))
(qa (with-temp-buffer (insert "a-z") (goto-char 1)
                      (skip-chars-forward "\\s-"))
    0)

;; == skip-syntax-forward / skip-syntax-backward ==
(qa (with-temp-buffer (insert "abc 123") (goto-char 1)
                      (list (skip-syntax-forward "w") (point)))
    '(3 4))
(qa (with-temp-buffer (insert "abc_12") (goto-char 1)
                      (list (skip-syntax-forward "w") (point)))
    '(3 4))
(qa (with-temp-buffer (insert "abc_12") (goto-char 1)
                      (list (skip-syntax-forward "w_") (point)))
    '(6 7))
(qa (with-temp-buffer (insert "ab c_x") (goto-char 1)
                      (list (skip-syntax-forward "w ") (point)))
    '(4 5))
(qa (with-temp-buffer (insert "   abc") (goto-char 7)
                      (list (skip-syntax-backward "w") (point)))
    '(-3 4))
(qa (with-temp-buffer (insert "abc   ") (goto-char 7)
                      (list (skip-syntax-backward "-") (point)))
    '(-3 4))
(qa (with-temp-buffer (insert "abc") (goto-char 1)
                      (list (skip-syntax-forward "^w") (point)))
    '(0 1))
(qa (with-temp-buffer (insert "xabc") (goto-char 1)
                      (list (skip-syntax-forward "w" 2) (point)))
    '(1 2))
(qa (with-temp-buffer (insert "a;;b") (goto-char 2)
                      (list (skip-syntax-forward ".") (point)))
    '(2 4))

;; == word motion ==
(qa (with-temp-buffer (insert "foo_bar baz") (goto-char 1)
                      (forward-word) (point))
    4)
(qa (with-temp-buffer (insert "foo_bar baz") (goto-char 1)
                      (let ((st (copy-syntax-table (standard-syntax-table))))
                        (with-syntax-table st
                          (modify-syntax-entry ?_ "w" st)
                          (forward-word)
                          (point))))
    8)
(qa (with-temp-buffer (insert "abc123 xyz") (goto-char 1)
                      (let ((st (copy-syntax-table (standard-syntax-table))))
                        (with-syntax-table st
                          (modify-syntax-entry '(?0 . ?9) "w" st)
                          (forward-word)
                          (point))))
    7)
(qa (with-temp-buffer (insert "foo bar") (goto-char (point-max))
                      (backward-word) (point))
    5)
(qa (with-temp-buffer (insert "foo bar") (goto-char 1)
                      (forward-word))
    t)
(qa (with-temp-buffer (insert "abc") (goto-char 1)
                      (list (forward-word 2) (point)))
    '(nil 4))
;; forward-word returns nil when there is no word left
(qa (with-temp-buffer (insert "abc") (goto-char 1)
                      (forward-word 3))
    nil)
(qa (with-temp-buffer (insert "a b") (goto-char 1)
                      (list (forward-word 0) (point)))
    '(t 1))
(qa (with-temp-buffer (insert "ab  cd") (goto-char 1)
                      (backward-word) (point))
    1)

;; == sexp/list motion ==
(qa (with-temp-buffer (insert "(a b) c") (goto-char 1)
                      (list (forward-sexp) (point)))
    '(nil 6))
(qa (with-temp-buffer (insert "foo bar") (goto-char 1)
                      (forward-sexp) (point))
    4)
(qa (with-temp-buffer (insert "\"str\" x") (goto-char 1)
                      (forward-sexp) (point))
    6)
(qa (with-temp-buffer (insert "(a b) c") (goto-char 6)
                      (list (backward-sexp) (point)))
    '(nil 1))
;; `#' is punctuation in the standard table: forward-sexp at `#' skips
;; it plus the following balanced list
(qa (with-temp-buffer (insert "#(a b) x") (goto-char 1)
                      (list (forward-sexp) (point)))
    '(nil 7))
(qa-err-data (with-temp-buffer (insert "(a b") (goto-char 1) (forward-sexp))
             'scan-error '("Unbalanced parentheses" 1 5))
(qa-err-data (with-temp-buffer (insert "a b)") (goto-char 1) (forward-sexp 3))
             'scan-error '("Containing expression ends prematurely" 4 5))
;; scan-sexps: position of end after COUNT sexps, nil if impossible
(qa (with-temp-buffer (insert "(a (b c) d) e") (scan-sexps 1 1)) 12)
(qa (with-temp-buffer (insert "(a (b c) d) e") (scan-sexps 1 2)) 14)
(qa (with-temp-buffer (insert "(a (b c) d) e") (scan-sexps 13 -1)) 1)
(qa (with-temp-buffer (insert "foo bar") (scan-sexps 1 1)) 4)
;; scan-lists (FROM COUNT DEPTH)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-lists 1 1 0)) 10)
(qa (with-temp-buffer (insert "(a (b) c) (d)") (scan-lists 1 1 0)) 10)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-lists 1 2 0)) nil)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-lists 4 1 1)) 10)
(qa (with-temp-buffer (insert "(a (b) c)") (scan-lists 4 -1 1)) 1)
(qa-err-data (with-temp-buffer (insert "(a (b) c)") (scan-lists 1 1 1))
             'scan-error '("Unbalanced parentheses" 1 10))
;; up-list / backward-up-list / down-list / forward-list / backward-list
(qa (with-temp-buffer (insert "(a (b c) d)") (goto-char 5)
                      (list (up-list) (point)))
    '(nil 9))
(qa (with-temp-buffer (insert "(a (b c) d)") (goto-char 5)
                      (backward-up-list) (point))
    4)
(qa (with-temp-buffer (insert "(a (b c) d)") (goto-char 1)
                      (list (down-list) (point)))
    '(nil 2))
(qa (with-temp-buffer (insert "(a (b c) d)") (goto-char 4)
                      (down-list) (point))
    5)
(qa (with-temp-buffer (insert "(a) (b)") (goto-char 1)
                      (forward-list) (point))
    4)
(qa (with-temp-buffer (insert "(a) (b)") (goto-char 8)
                      (backward-list) (point))
    5)
(qa-err-data (with-temp-buffer (insert "abc") (goto-char 2) (up-list))
             'scan-error '("Unbalanced parentheses" 2 4))
(qa-err-data (with-temp-buffer (insert "(a (b c) d)") (goto-char 1) (up-list -1))
             'scan-error '("Unbalanced parentheses" 1 1))
;; down-list at top level with no list ahead returns nil
(qa (with-temp-buffer (insert "x (a) y") (goto-char 1) (down-list)) nil)
(qa-err-data (with-temp-buffer (insert "x (a) y") (goto-char 5) (down-list))
             'scan-error '("Containing expression ends prematurely" 5 6))
;; up-list from inside a string is not allowed out
(qa-err-data (with-temp-buffer (insert "(a \"x)\" b)") (goto-char 7) (up-list))
             'scan-error '("Unbalanced parentheses" 7 11))

;; == defun motion ==
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char (point-max))
      (list (beginning-of-defun) (point)))
    '(t 17))
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char (point-max))
      (beginning-of-defun)
      (beginning-of-defun)
      (point))
    1)
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char 20)
      (beginning-of-defun)
      (point))
    17)
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char 1)
      (end-of-defun)
      (point))
    16)
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char 20)
      (end-of-defun)
      (point))
    32)
;; negative arg = forward
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char 1)
      (list (beginning-of-defun -1) (point)))
    '(t 17))
(qa (with-temp-buffer
      (insert "(defun a () 1)\n\n(defun b () 2)\n")
      (goto-char 1)
      (beginning-of-defun -2))
    nil)

;; == forward-comment (needs comment syntax: emacs-lisp-mode) ==
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (goto-char 4)
      (list (forward-comment 1) (point)))
    '(t 11))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert ";; hi\n(b)")
      (goto-char 1)
      (list (forward-comment 1) (point)))
    '(t 7))
;; a non-comment character at point stops the scan: nil, unmoved
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (goto-char 1)
      (list (forward-comment 1) (point)))
    '(nil 1))
;; same with the standard table, where `;' is punctuation
(qa (with-temp-buffer
      (insert "(a) ;; hi\n(b)")
      (goto-char 4)
      (list (forward-comment 1) (point)))
    '(nil 5))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (goto-char 11)
      (list (forward-comment -1) (point)))
    '(t 5))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert ";; a\n ;; b\nx")
      (goto-char 1)
      (list (forward-comment 1) (point)))
    '(t 6))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a) ;; hi\n(b)")
      (goto-char 4)
      (forward-comment 0))
    t)

;; == backward-prefix-chars ==
;; `' is an expression prefix in emacs-lisp-mode, punctuation otherwise.
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "''x")
      (goto-char 3)
      (list (backward-prefix-chars) (point)))
    '(nil 1))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "x'y")
      (goto-char 3)
      (backward-prefix-chars)
      (point))
    2)
(qa (with-temp-buffer
      (insert "''x")
      (goto-char 3)
      (backward-prefix-chars)
      (point))
    3)

;; == syntax-ppss / parse-partial-sexp ==
;; state is an 11-element list in 31.1
(qa (with-temp-buffer (insert "") (syntax-ppss))
    '(0 nil nil nil nil nil 0 nil nil nil nil))
(qa (with-temp-buffer (insert "(a \"str\") ;; c\n(d)")
                      (goto-char 6)
                      (syntax-ppss))
    '(1 1 2 34 nil nil 0 nil 4 (1) nil))
(qa (with-temp-buffer (insert "(a (b c) d)")
                      (goto-char 6)
                      (syntax-ppss))
    '(2 4 5 nil nil nil 0 nil nil (1 4) nil))
(qa (with-temp-buffer (insert "'(a b)")
                      (goto-char 3)
                      (syntax-ppss))
    '(1 2 nil nil nil nil 0 nil nil (2) nil))
;; inside a string: elt 3 = delimiter char, elt 8 = string start
(qa (with-temp-buffer (insert "(a \"str\")")
                      (goto-char 8)
                      (let ((s (syntax-ppss)))
                        (list (nth 3 s) (nth 8 s) (nth 9 s) (length s))))
    '(34 4 (1) 11))
;; inside a comment (emacs-lisp-mode): elt 4 = t, elt 8 = comment start
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a)\n;; hi\n(b)")
      (syntax-ppss 7))
    '(0 nil 1 nil t nil 0 nil 5 nil nil))
;; `;' is not a comment starter in the standard table
(qa (with-temp-buffer (insert "(a) ; hi") (syntax-ppss 9))
    '(0 nil 7 nil nil nil 0 nil nil nil nil))
(qa (with-temp-buffer (insert "(a \"str\") ;; c\n(d)")
                      (parse-partial-sexp 1 9))
    '(1 1 4 nil nil nil 0 nil nil (1) nil))
(qa (with-temp-buffer (insert "(a b) c")
                      (parse-partial-sexp 1 6))
    '(0 nil 1 nil nil nil 0 nil nil nil nil))
(qa (with-temp-buffer (insert "ab cd")
                      (parse-partial-sexp 1 6))
    '(0 nil 4 nil nil nil 0 nil nil nil nil))
;; parse-partial-sexp state equals syntax-ppss at the same point
(qa (with-temp-buffer (insert "(a (b c) d)")
                      (equal (parse-partial-sexp 1 6) (syntax-ppss 6)))
    t)
;; resume from a saved state
(qa (with-temp-buffer (insert "(a (b c) d)")
                      (let ((s1 (parse-partial-sexp 1 4)))
                        (parse-partial-sexp 4 11 nil nil s1)))
    '(1 1 10 nil nil nil 1 nil nil (1) nil))
;; TARGETDEPTH stops the scan early
(qa (with-temp-buffer (insert "(a (b) c)")
                      (parse-partial-sexp 1 10 1))
    '(1 1 nil nil nil nil 0 nil nil (1) nil))
;; ppss after an escape char: elt 5 = t
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "\"a\\nb\"")
      (syntax-ppss 4))
    '(0 nil nil 34 nil t 0 nil 1 nil 9))

(princ (qa--done))
