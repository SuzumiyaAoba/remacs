;; indentfill — indentation, tab stops, filling and paragraph motion.
;; GNU-31.1 quirks pinned below: `indent-to' returns the column actually
;; reached (a no-op when already past COLUMN; the second arg is a minimum
;; number of COLUMNS to add, not a minimum resulting column); negative
;; columns are clamped to 0 rather than signalling; `indent-line-to'
;; returns the column only when it INDENTED (nil when it dedented or was
;; already right); `indent-region' returns t even though docstrings say
;; nil; `forward-paragraph' returns the number of paragraphs left to
;; move (like `forward-line') so hitting a bound returns a nonzero
;; leftover rather than signalling; `tab-to-tab-stop' extrapolates past
;; the last entry of `tab-stop-list' by the last spacing; the fill
;; functions return "" (or the fill prefix string) instead of a useful
;; value; `unfill-paragraph' requires its ARG under -Q and `justify',
;; `unjustify', `unjustify-region'-relative `unfill-region', and
;; `text-indent' do not exist; `delete-indentation' returns nil and is a
;; no-op on the first line; `unjustify-region' only strips text
;; properties and leaves the spacing untouched.

;; == default indentation/fill variables ==
(qa (default-value 'fill-column) 70)
(qa (default-value 'fill-prefix) nil)
(qa (default-value 'tab-stop-list) nil)
(qa (default-value 'indent-tabs-mode) t)
(qa (default-value 'left-margin) 0)
(qa (default-value 'indent-line-function) 'indent-relative)
(qa (default-value 'indent-region-function) 'indent-region-line-by-line)
(qa (default-value 'adaptive-fill-mode) t)
(qa (default-value 'adaptive-fill-function) 'ignore)
(qa (default-value 'adaptive-fill-regexp) "[-–!|#%;>*·•‣⁃◦ \t]*")
(qa (default-value 'adaptive-fill-first-line-regexp) "\\`[ \t]*\\'")
(qa (default-value 'paragraph-start) "\f\\|[ \t]*$")
(qa (default-value 'paragraph-separate) "[ \t\f]*$")
;; names that are unbound under GNU -Q (kept honest if they appear later)
(qa (fboundp 'justify) nil)
(qa (fboundp 'unjustify) nil)
(qa (fboundp 'unfill-region) nil)
(qa (fboundp 'text-indent) nil)
(qa (fboundp 'adaptive-fill-first-line-regexp) nil) ; a variable, not a fn
(qa (fboundp 'unfill-paragraph) t)
(qa (fboundp 'justify-current-line) t)
(qa (fboundp 'set-justification) t)
(qa (fboundp 'indent-to-column) t)
(qa (fboundp 'join-line) t)
(qa (fboundp 'current-fill-column) t)

;; == current-column / current-indentation ==
(qa (with-temp-buffer (current-column)) 0)
(qa (with-temp-buffer (insert "\t\t") (goto-char (point-max))
      (current-column))
    16)
(qa (with-temp-buffer
      (insert "abc\n   indented\n\tx\n\n")
      (goto-char (point-min))
      (list (current-indentation)
            (progn (forward-line 1) (current-indentation))
            (progn (forward-line 1) (current-indentation))
            (progn (forward-line 1) (current-indentation))))
    '(0 3 8 0))                      ; blank line counts as 0 indentation
;; a tab past column 8 counts as its full stop
(qa (with-temp-buffer (insert "a\tb") (goto-char (point-min))
      (current-indentation))
    0)

;; == move-to-column ==
;; returns the column actually reached; no insertion without FORCE
(qa (with-temp-buffer
      (insert "abcdef")
      (goto-char (point-min))
      (list (move-to-column 4) (point)))
    '(4 5))
;; stops on the far side of a tab rather than splitting it (no FORCE)
(qa (with-temp-buffer
      (insert "a\tb")
      (goto-char (point-min))
      (list (move-to-column 4) (point) (current-column)))
    '(8 3 8))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char (point-min))
      (list (move-to-column 6) (current-column) (point) (buffer-string)))
    '(2 2 3 "ab"))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char (point-min))
      (list (move-to-column 6 t) (current-column) (buffer-string)))
    '(6 6 "ab    "))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 2)
      (list (move-to-column 0) (point)))
    '(0 1))
(qa-err (with-temp-buffer (move-to-column -1)) 'wrong-type-argument)

;; == indent-to / indent-to-column ==
;; returns the resulting column; uses tabs when indent-tabs-mode is on
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (indent-to 5) (current-column) (buffer-string)))
    '(5 5 "x    "))
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (let ((indent-tabs-mode nil)) (indent-to 5)) (buffer-string)))
    '(5 "x    "))
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (indent-to 12) (buffer-string)))
    '(12 "x	    "))
;; already past COLUMN: a no-op returning the CURRENT column
(qa (with-temp-buffer
      (insert "123456789")
      (goto-char (point-max))
      (list (indent-to 3) (buffer-string)))
    '(9 "123456789"))
;; MINIMUM is a minimum number of columns to ADD, not a result floor:
;; col 9 + at least 12 more -> col 21
(qa (with-temp-buffer
      (insert "123456789")
      (goto-char (point-max))
      (list (indent-to 3 12) (buffer-string)))
    '(21 "123456789	     "))
;; negative column clamps to 0 and does nothing (no error)
(qa (with-temp-buffer (indent-to -2)) 0)
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-min))
      (list (indent-to 0) (point)))
    '(0 1))
;; indent-to-column is the same operation
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (indent-to-column 4) (buffer-string)))
    '(4 "x   "))

;; == indent-line-to ==
;; returns the column only when the line was INDENTED to it
(qa (with-temp-buffer (insert "aa") (indent-line-to 4)) 4)
(qa (with-temp-buffer (insert "  aa") (indent-line-to 2)) nil)
;; dedenting returns nil even though the buffer changed
(qa (with-temp-buffer
      (insert "    aa")
      (list (indent-line-to 1) (buffer-string)))
    '(nil " aa"))
(qa (with-temp-buffer
      (insert "\taa")
      (list (indent-line-to 3) (buffer-string)))
    '(nil "   aa"))

;; == indent-rigidly ==
(qa (with-temp-buffer
      (insert "aa\nbb\ncc\n")
      (list (indent-rigidly (point-min) (point-max) 2) (buffer-string)))
    '(nil "  aa\n  bb\n  cc\n"))
(qa (with-temp-buffer
      (insert "  aa\n   bb\ncc\n")
      (indent-rigidly (point-min) (point-max) -1)
      (buffer-string))
    " aa\n  bb\ncc\n")
;; a wholly-blank line is left alone
(qa (with-temp-buffer
      (insert "aa\n\nbb\n")
      (indent-rigidly (point-min) (point-max) 2)
      (buffer-string))
    "  aa\n\n  bb\n")
;; a line is shifted only when the region covers its BEGINNING:
;; region 5..8 spans the middle of line 2 and the start of line 3
(qa (with-temp-buffer
      (insert "aa\nbb\ncc\n")
      (indent-rigidly 5 8 2)
      (buffer-string))
    "aa\nbb\n  cc\n")
;; negative count clamps to column 0, never errors
(qa (with-temp-buffer
      (insert " aa\nbb\n")
      (indent-rigidly (point-min) (point-max) -5)
      (buffer-string))
    "aa\nbb\n")

;; == indent-region ==
;; returns t despite the docstring saying nil
(qa (with-temp-buffer (insert "x\ny\n") (indent-region 1 4 2)) t)
(qa (with-temp-buffer (insert "x\ny\n") (indent-region 1 4)) t)
;; COLUMN gives every line that indentation
(qa (with-temp-buffer
      (insert "aa\nbb\ncc\n")
      (indent-region (point-min) (point-max) 3)
      (buffer-string))
    "   aa\n   bb\n   cc\n")
(qa (with-temp-buffer
      (insert "aa\nbb\ncc\n")
      (indent-region (point-min) (point-max) 0)
      (buffer-string))
    "aa\nbb\ncc\n")
;; empty region on an empty buffer is fine
(qa (with-temp-buffer (indent-region (point-min) (point-max))) t)
;; indent-line-function is honored line by line
(qa (with-temp-buffer
      (insert "a\nb\n")
      (let ((indent-line-function (lambda () (indent-to 5))))
        (indent-region (point-min) (point-max))
        (buffer-string)))
    "     a\n     b\n")

;; == tab-to-tab-stop ==
;; returns the column landed on (default stops every 8)
(qa (with-temp-buffer
      (list (tab-to-tab-stop) (current-column) (buffer-string)))
    '(8 8 "	"))
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (let ((tab-stop-list '(3 7))) (tab-to-tab-stop))
            (current-column)))
    '(3 3))
(qa (with-temp-buffer
      (insert "1234")
      (goto-char (point-max))
      (let ((tab-stop-list '(3 7)))
        (tab-to-tab-stop)
        (current-column)))
    7)
;; past the last stop the spacing repeats: 7 + (7-3) = 11
(qa (with-temp-buffer
      (insert "12345678")
      (goto-char (point-max))
      (let ((tab-stop-list '(3 7)))
        (tab-to-tab-stop)
        (current-column)))
    11)

;; == back-to-indentation ==
(qa (with-temp-buffer
      (insert "   abc\n   \nxyz")
      (goto-char 1)
      (back-to-indentation)
      (list (point) (progn (forward-line 1)
                           (back-to-indentation)
                           (point))
            (progn (forward-line 1)
                   (back-to-indentation)
                   (point))))
    ;; whitespace-only line -> point lands on the newline
    '(4 11 12))

;; == delete-indentation / join-line ==
;; returns nil; joins the current line onto the previous one
(qa (with-temp-buffer
      (insert "foo\n   bar\nbaz")
      (goto-char 4)
      (forward-line 1)
      (list (delete-indentation) (buffer-string)))
    '(nil "foo bar\nbaz"))
;; JOIN-FOLLOWING merges the next line instead
(qa (with-temp-buffer
      (insert "foo\n   bar\nbaz")
      (goto-char 1)
      (list (delete-indentation t) (buffer-string)))
    '(nil "foo bar\nbaz"))
;; no space is added after an opening paren / before a closing one
(qa (with-temp-buffer
      (insert "(\n   foo")
      (goto-char 4)
      (list (delete-indentation) (buffer-string)))
    '(nil "(foo"))
(qa (with-temp-buffer
      (insert "foo\n   )")
      (goto-char 5)
      (list (delete-indentation) (buffer-string)))
    '(nil "foo)"))
;; joining after "." keeps a single space here
(qa (with-temp-buffer
      (insert "end.\n   next")
      (goto-char 6)
      (delete-indentation)
      (buffer-string))
    "end. next")
;; on the first line there is nothing to join: no-op, returns nil
(qa (with-temp-buffer
      (insert "  foo")
      (goto-char (point-min))
      (list (delete-indentation) (buffer-string)))
    '(nil "  foo"))
;; join-line joins current line onto the previous one
(qa (with-temp-buffer
      (insert "a\n  b")
      (goto-char 5)
      (list (join-line) (buffer-string)))
    '(nil "a b"))

;; == newline / newline-and-indent / open-line ==
(qa (with-temp-buffer (insert "ab") (list (newline) (buffer-string)))
    '(nil "ab\n"))
(qa (with-temp-buffer (insert "ab") (list (newline 2) (buffer-string)))
    '(nil "ab\n\n"))
;; newline-and-indent works non-interactively and indents like the
;; previous line (default indent-line-function is indent-relative)
(qa (with-temp-buffer
      (insert "    aa")
      (goto-char (point-max))
      (list (funcall 'newline-and-indent) (buffer-string) (current-column)))
    '(nil "    aa\n    " 4))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 1)
      (funcall 'newline-and-indent)
      (list (buffer-string) (point)))
    '("\nab" 2))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 2)
      (list (open-line 2) (buffer-string) (point)))
    '(nil "a\n\nb" 2))

;; == indent-relative / indent-relative-maybe ==
;; returns a marker into the previous line
(qa (with-temp-buffer
      (insert "  first\n")
      (goto-char (point-max))
      (list (markerp (indent-relative)) (buffer-string)))
    '(t "  first\n  "))
(qa (with-temp-buffer
      (insert "  first line\n")
      (goto-char (point-max))
      (list (markerp (indent-relative-maybe)) (buffer-string)))
    '(t "  first line\n  "))
;; from an existing indent it hops to the NEXT indentation point
(qa (with-temp-buffer
      (insert "  x  y\n")
      (goto-char (point-max))
      (insert "  ")
      (list (markerp (indent-relative)) (buffer-string)))
    '(t "  x  y\n     "))

;; == forward-paragraph / backward-paragraph ==
;; returns the number of paragraphs left to move (0 on full success),
;; the same convention as forward-line -- bounds do NOT signal
(qa (with-temp-buffer
      (insert "para1 line1\npara1 line2\n\npara2 line1\npara2 line2\n\n\npara3")
      (goto-char 1)
      (list (forward-paragraph) (point)
            (forward-paragraph) (point)
            (forward-paragraph) (point)
            (backward-paragraph) (point)
            (backward-paragraph 2) (point)
            (backward-paragraph 1) (point)))
    '(0 25 0 50 0 57 0 51 0 1 -1 1))
(qa (with-temp-buffer
      (insert "abc")
      (goto-char (point-max))
      (forward-paragraph))
    1)                              ; leftover count, not an error
(qa (with-temp-buffer
      (insert "abc")
      (goto-char 1)
      (backward-paragraph))
    -1)
(qa (with-temp-buffer
      (insert "a\n\nb")
      (goto-char 1)
      (list (forward-paragraph 5) (point)))
    '(3 5))                         ; only 2 paragraphs available
;; a whitespace-only line ends the paragraph (paragraph-separate)
(qa (with-temp-buffer
      (insert "abc\n   \ndef")
      (goto-char 1)
      (forward-paragraph)
      (point))
    5)
;; custom paragraph-start matching only buffer start makes one
;; big paragraph
(qa (with-temp-buffer
      (let ((paragraph-start "\\`\\|\f"))
        (insert "a\nb\nc")
        (goto-char 1)
        (forward-paragraph)
        (point)))
    6)
;; multi-line count and negative arg
(qa (with-temp-buffer
      (insert "a\n\nb\n\nc")
      (goto-char 1)
      (list (progn (forward-paragraph 2) (point))
            (progn (forward-paragraph -1) (point))))
    '(6 3))

;; == forward/backward-to-indentation ==
;; GNU behavior: skips to the next line's first nonblank character and
;; returns the indentation column found, or 0 when a buffer bound
;; stops the search
(qa (with-temp-buffer
      (insert "ab\n  cd")
      (goto-char 1)
      (list (funcall 'forward-to-indentation) (point)
            (funcall 'forward-to-indentation) (point)
            (funcall 'backward-to-indentation) (point)))
    '(2 6 0 8 0 1))

;; == delete-to-left-margin ==
;; deletes the margin inside (FROM . TO) when left-margin is nonzero
(qa (with-temp-buffer
      (insert "    ab")
      (let ((left-margin 4))
        (list (markerp (funcall 'delete-to-left-margin 1 5))
              (buffer-string))))
    '(t "ab"))
(qa (with-temp-buffer
      (insert "    ab\n    cd\n")
      (let ((left-margin 4))
        (delete-to-left-margin (point-min) (point-max))
        (buffer-string)))
    "ab\ncd\n")
;; with left-margin 0 and no region it changes nothing
(qa (with-temp-buffer
      (insert "   ab")
      (list (markerp (funcall 'delete-to-left-margin)) (buffer-string)))
    '(t "   ab"))

;; == line-number-at-pos absolute arg ==
;; narrowing makes counts relative unless ABSOLUTE is non-nil
(qa (with-temp-buffer
      (insert "a\nb\nc\nd")
      (narrow-to-region 5 8)
      (goto-char 6)
      (list (line-number-at-pos) (line-number-at-pos nil t)))
    '(1 3))

;; == fill-paragraph ==
;; returns "" (the empty fill prefix) on success
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aaa bbb ccc ddd eee fff ggg")
        (list (fill-paragraph) (buffer-string))))
    '("" "aaa bbb ccc ddd eee\nfff ggg"))
;; point may sit on any line of the paragraph
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aaa bbb ccc ddd\neee fff ggg hhh")
        (goto-char 20)
        (fill-paragraph)
        (buffer-string)))
    "aaa bbb ccc ddd eee\nfff ggg hhh")
;; only the paragraph containing point is filled
(qa (with-temp-buffer
      (let ((fill-column 15))
        (insert "one two three.\n\nfive six seven eight.")
        (goto-char (point-max))
        (list (fill-paragraph) (buffer-string))))
    '("" "one two three.\n\nfive six seven\neight."))
;; text that already fits is untouched
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aa bb cc dd ee")
        (fill-paragraph 'full)
        (buffer-string)))
    "aa bb cc dd ee")
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aa bb cc dd ee ff")
        (fill-paragraph 'left)
        (buffer-string)))
    "aa bb cc dd ee ff")

;; == fill-region / fill-region-as-paragraph ==
;; both return "" as well
(qa (with-temp-buffer
      (let ((fill-column 15))
        (insert "one two three four.\n\nfive six seven eight.")
        (list (fill-region (point-min) (point-max)) (buffer-string))))
    '("" "one two three\nfour.\n\nfive six seven\neight."))
(qa (with-temp-buffer (fill-region 1 1)) nil) ; empty region -> nil
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aa bb cc dd ee ff gg hh ii")
        (list (fill-region-as-paragraph (point-min) (point-max))
              (buffer-string))))
    '("" "aa bb cc dd ee ff gg\nhh ii"))
;; multiple spaces are squeezed unless NOSQUEEZE
(qa (with-temp-buffer
      (let ((fill-column 30))
        (insert "aa   bb   cc   dd ee ff")
        (fill-region-as-paragraph (point-min) (point-max))
        (buffer-string)))
    "aa bb cc dd ee ff")
(qa (with-temp-buffer
      (let ((fill-column 30))
        (insert "aa   bb   cc")
        (fill-region-as-paragraph (point-min) (point-max) nil t)
        (buffer-string)))
    "aa   bb   cc")
;; leading indentation of the region is preserved on every line
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "   aa bb cc dd ee ff gg")
        (fill-region-as-paragraph (point-min) (point-max))
        (buffer-string)))
    "   aa bb cc dd ee ff\n   gg")
;; JUSTIFY accepts the usual symbols
(qa (with-temp-buffer
      (let ((fill-column 30))
        (insert "aa bb cc dd ee")
        (fill-region-as-paragraph (point-min) (point-max) 'full)
        (buffer-string)))
    "aa bb cc dd ee")
;; fill-individual-paragraphs returns nil and fills each "paragraph"
;; (here indentation-separated groups share the fill)
(qa (with-temp-buffer
      (let ((fill-column 15))
        (insert "  aa bb cc dd ee\nff gg hh ii jj")
        (list (fill-individual-paragraphs (point-min) (point-max))
              (buffer-string))))
    '(nil "  aa bb cc dd\nee ff gg hh ii\njj"))
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert "aa bb cc\ndd ee ff")
        (fill-individual-paragraphs (point-min) (point-max) 'full)
        (buffer-string)))
    "aa bb cc dd ee ff")

;; == justify-current-line / unjustify-region ==
;; returns nil; justification is stored, padding via tabs under -Q
(qa (with-temp-buffer
      (insert "aa bb cc")
      (let ((fill-column 20))
        (list (justify-current-line 'none) (buffer-string))))
    '(nil "aa bb cc"))
(qa (with-temp-buffer
      (insert "aa bb cc")
      (let ((fill-column 20))
        (justify-current-line 'full)
        (buffer-string)))
    "aa       bb       cc")
;; 'none' on an unjustified line is a no-op
(qa (with-temp-buffer
      (insert "aa bb cc")
      (let ((fill-column 20))
        (justify-current-line 'none)
        (buffer-string)))
    "aa bb cc")
;; unjustify-region only strips properties, text is left alone
(qa (with-temp-buffer
      (insert "aa   bb")
      (list (unjustify-region 1 8) (buffer-string)))
    '(nil "aa   bb"))

;; == unfill-paragraph (ARG required under -Q) ==
(qa (with-temp-buffer
      (insert "aa bb\ncc dd\n\nxy zw")
      (goto-char 3)
      (list (unfill-paragraph nil) (buffer-string)))
    '("" "aa bb cc dd\n\nxy zw"))
(qa (with-temp-buffer
      (insert "aa bb\ncc dd")
      (list (unfill-paragraph nil (point-min) (point-max))
            (buffer-string)))
    '("" "aa bb cc dd"))

;; == fill-prefix / adaptive fill ==
;; an explicit fill-prefix is repeated on every line and returned
(qa (with-temp-buffer
      (let ((fill-column 20)
            (fill-prefix "> "))
        (insert "> aa bb cc dd ee ff")
        (list (fill-paragraph) (buffer-string))))
    '("> " "> aa bb cc dd ee ff"))
;; adaptive fill picks up "> " automatically
(qa (with-temp-buffer
      (let ((fill-column 25))
        (insert "> aa bb cc dd ee ff gg hh ii jj")
        (fill-paragraph)
        (buffer-string)))
    "> aa bb cc dd ee ff gg hh\n  ii jj")
;; adaptive-fill-mode nil disables prefix detection
(qa (with-temp-buffer
      (let ((fill-column 25)
            (adaptive-fill-mode nil))
        (insert "> aa bb cc dd ee ff gg hh ii jj")
        (fill-paragraph)
        (buffer-string)))
    "> aa bb cc dd ee ff gg hh\nii jj")
;; adaptive-fill-function overrides detection entirely
(qa (with-temp-buffer
      (let ((fill-column 20)
            (adaptive-fill-function (lambda () "!! ")))
        (insert "aa bb cc dd ee ff")
        (fill-paragraph)
        (buffer-string)))
    "aa bb cc dd ee ff")
;; a long prefix that leaves no room stays as one line
(qa (with-temp-buffer
      (let ((fill-column 20))
        (insert ">> aa bb cc dd ee ff")
        (fill-paragraph)
        (buffer-string)))
    ">> aa bb cc dd ee ff")
;; set-fill-prefix takes the text before point on the line and
;; returns the confirmation message string
(qa (with-temp-buffer
      (insert ">> text")
      (goto-char 4)
      (let ((fill-prefix nil))
        (list (funcall 'set-fill-prefix) fill-prefix)))
    '("fill-prefix: \">> \"" ">> "))

;; == misc fill helpers ==
;; canonically-space-region: single space, but two after "."
;; (sentence-end-double-space is t by default)
(qa (with-temp-buffer
      (insert "a   b.    c    d")
      (list (canonically-space-region (point-min) (point-max))
            (buffer-string)))
    '(nil "a b.  c d"))
(qa (with-temp-buffer
      (insert "ab")
      (let ((fill-column 10))
        (list (funcall 'center-line) (buffer-string))))
    '(nil "    ab"))
(qa (string-fill "aaa bbb ccc ddd" 6) "aaa\nbbb\nccc\nddd")
(qa (string-fill "a" 6) "a")
;; current-fill-column sees the dynamic binding
(qa (with-temp-buffer (current-fill-column)) 70)
(qa (with-temp-buffer (let ((fill-column 30)) (current-fill-column))) 30)
(qa (with-temp-buffer
      (list (funcall 'set-fill-column 33) fill-column))
    '(33 33))

(princ (qa--done))
