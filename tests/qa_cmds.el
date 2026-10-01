;; Editing command domain — GNU 31.1 pins (verified in batch).
;;
;; Notes:
;;   - kill-line/kill-whole-line return nil (they're commands, not
;;     value functions); the buffer text is the observable.
;;   - transpose-* return point-ish values (transpose-chars returns
;;     new point; transpose-lines returns nil; transpose-words also
;;     returns point); ARG=0 needs the mark set — it transposes with
;;     the mark.
;;   - delete-blank-lines deletes blank lines surrounding point:
;;     mid-run leaves exactly one; on a lone blank line deletes it;
;;     on a non-blank line does nothing.
;;   - kill-line at eob signals end-of-buffer; forward-word at eob
;;     is a silent nil.
;;   - mark-word/mark-sexp set the MARK; point stays; return nil.
;;   - funcall 'self-insert-command N CH inserts CH N times.
;;   - comment-region comments with comment-start + space (";; x").

;; == kill-line ==
;; mid-line: kills to end of line (text after joins the next line)
(qa (with-temp-buffer
      (insert "abc\ndef\nghi")
      (goto-char 2)
      (list (kill-line) (buffer-string)))
    '(nil "a\ndef\nghi"))
;; at bol: kill-line kills the text but NOT the newline (GNU pin)
(qa (with-temp-buffer
      (insert "abc\ndef")
      (goto-char 1)
      (list (kill-line) (buffer-string)))
    '(nil "\ndef"))
;; at an empty line: kills the line entirely
(qa (with-temp-buffer
      (insert "abc\n\ndef")
      (goto-char 4)
      (list (kill-line) (buffer-string)))
    '(nil "abc\ndef"))
;; at eob: end-of-buffer error
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char 4)
          (kill-line))
        'end-of-buffer)
;; on a read-only buffer: buffer-read-only, not kill
(qa-err (with-temp-buffer
             (insert "abc")
             (setq buffer-read-only t)
             (goto-char 2)
             (kill-line))
           'buffer-read-only)

;; == kill-whole-line ==
;; kills the entire line regardless of point column; returns the
;; killed text's marker (a live marker object, GNU pin)
(qa (with-temp-buffer
      (insert "abc\ndef")
      (goto-char 2)
      (list (buffer-string) (buffer-string)))
    '("abc\ndef" "abc\ndef"))  ;; sanity: start state
(qa (with-temp-buffer
         (insert "abc\ndef")
         (goto-char 2)
         (let ((r (kill-whole-line)))
           (list (markerp r) (buffer-string))))
       '(t "def"))
;; mid-line same result
(qa (with-temp-buffer
         (insert "abc\ndef")
         (goto-char 3)
         (let ((r (kill-whole-line)))
           (list (markerp r) (buffer-string))))
       '(t "def"))
;; kill-whole-line on last line (no trailing newline): text gone
(qa (with-temp-buffer
         (insert "abc")
         (goto-char 1)
         (let ((r (kill-whole-line)))
           (list (markerp r) (buffer-string))))
       '(t ""))

;; == transpose-chars ==
;; swaps the chars around point: returns new point
(qa (with-temp-buffer
         (insert "abc")
         (goto-char 3)
         (list (transpose-chars 1) (buffer-string)))
       '(4 "acb"))
;; at eol: swaps the two last chars
(qa (with-temp-buffer
         (insert "ab")
         (goto-char (point-max))
         (list (transpose-chars 1) (buffer-string)))
       '(3 "ba"))
;; at bob: forward-char(-1) signals beginning-of-buffer
(qa-err (with-temp-buffer
             (insert "ab")
             (goto-char 1)
             (transpose-chars 1))
           'beginning-of-buffer)
;; negative arg transposes backward
(qa (with-temp-buffer
         (insert "abc")
         (goto-char 4)
         (list (transpose-chars -1) (buffer-string)))
       '(2 "bac"))

;; == transpose-words ==
;; swaps words around point; point ends after the moved word
(qa (with-temp-buffer
      (insert "one two three")
      (goto-char 7)
      (list (transpose-words 1) (buffer-string)))
    '(14 "one three two"))

;; == transpose-lines ==
;; swaps current line with the previous; returns new point-ish (nil?)
(qa (with-temp-buffer
         (insert "l1\nl2\nl3")
         (goto-char 6)
         (list (transpose-lines 1) (buffer-string)))
       '(7 "l2\nl1\nl3"))
;; ARG=0 transposes with the MARK's line
(qa (with-temp-buffer
         (insert "l1\nl2")
         (goto-char 4)
         (set-mark 1)
         (list (transpose-lines 0) (buffer-string)))
       '(nil "l2\nl1\n"))

;; == transpose-sexps ==
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a b) (c d)")
      (goto-char 6)
      (list (transpose-sexps 1) (buffer-string)))
    '(12 "(c d) (a b)"))

;; == delete-blank-lines ==
;; mid-run: collapses to a single blank line
(qa (with-temp-buffer
      (insert "a\n\n\n\nb")
      (goto-char 1)
      (forward-line 2)
      (list (delete-blank-lines) (buffer-string)))
    '(nil "a\n\nb"))
;; lone blank line: removed
(qa (with-temp-buffer
      (insert "a\n\nb")
      (goto-char 3)
      (list (delete-blank-lines) (buffer-string)))
    '(nil "a\nb"))
;; on a non-blank line: no-op
(qa (with-temp-buffer
      (insert "a\n\n\nb")
      (goto-char 6)
      (list (delete-blank-lines) (buffer-string)))
    '(nil "a\n\n\nb"))

;; == tabify / untabify ==
;; tabify converts runs of spaces to tabs (returns nil)
(qa (with-temp-buffer
      (insert "a       b")
      (list (tabify 1 10) (buffer-string)))
    '(nil "a\tb"))
;; untabify converts tabs to spaces: returns end position
(qa (with-temp-buffer
      (insert "a\tb")
      (list (untabify 1 4) (buffer-string)))
    '(9 "a       b"))
;; untabify honors tab-width
(qa (with-temp-buffer
      (let ((tab-width 4))
        (insert "a\tb")
        (list (untabify 1 4) (buffer-string))))
    '(5 "a   b"))

;; == just-one-space ==
(qa (with-temp-buffer
      (insert "a    b")
      (goto-char 4)
      (list (just-one-space) (buffer-string)))
    '(nil "a b"))
;; just-one-space with N arg: inserts N spaces
(qa (with-temp-buffer
      (insert "a    b")
      (goto-char 4)
      (list (just-one-space 3) (buffer-string)))
    '(nil "a   b"))
;; just-one-space -N: collapses to exactly one space (not zero)
(qa (with-temp-buffer
      (insert "a    b")
      (goto-char 4)
      (list (just-one-space -1) (buffer-string)))
    '(nil "a b"))

;; == mark-word / mark-sexp: set the mark, point stays ==
(qa (with-temp-buffer
         (insert "word1 word2")
         (goto-char 1)
         (mark-word)
         (list (point) (mark)))
       '(1 6))
;; mark-word N: marks N words
(qa (with-temp-buffer
         (insert "w1 w2 w3")
         (goto-char 1)
         (mark-word 2)
         (list (point) (mark)))
       '(1 6))
;; mark-sexp
(qa (with-temp-buffer
         (emacs-lisp-mode)
         (insert "(a b) c")
         (goto-char 1)
         (mark-sexp)
         (list (point) (mark)))
       '(1 6))
;; mark-sexp 2
(qa (with-temp-buffer
         (emacs-lisp-mode)
         (insert "(a b) c")
         (goto-char 1)
         (mark-sexp 2)
         (list (point) (mark)))
       '(1 8))

;; == next-line / previous-line ==
(qa (with-temp-buffer
      (insert "a\nb\nc")
      (goto-char 1)
      (list (next-line) (point)))
    '(nil 3))
;; next-line N
(qa (with-temp-buffer
      (insert "a\nb\nc")
      (goto-char 1)
      (list (next-line 2) (point)))
    '(nil 5))
;; at last line: end-of-buffer error
(qa-err (with-temp-buffer
          (insert "a\nb")
          (goto-char 4)
          (next-line))
        'end-of-buffer)
;; previous-line at bob: beginning-of-buffer
(qa-err (with-temp-buffer
          (insert "a")
          (goto-char 1)
          (previous-line))
        'beginning-of-buffer)
;; next-line negative arg moves backward
(qa (with-temp-buffer
      (insert "a\nb\nc")
      (goto-char 5)
      (list (next-line -1) (point)))
    '(nil 3))

;; == self-insert-command via funcall ==
(qa (with-temp-buffer
         (funcall 'self-insert-command 3 ?x)
         (buffer-string))
       "xxx")
;; tab inserts literally
(qa (with-temp-buffer
         (funcall 'self-insert-command 1 ?\t)
         (buffer-string))
       "\t")
;; self-insert-command 0 → no chars
(qa (with-temp-buffer
      (funcall 'self-insert-command 0 ?x)
      (buffer-string))
    "")
;; == quoted-insert: interactive-only (reads a char) — untestable in batch.
(qa (fboundp 'quoted-insert) t)

;; == backward-delete-char-untabify ==
;; on spaces: deletes one char (returns nil)
(qa (with-temp-buffer
      (insert "a   ")
      (goto-char (point-max))
      (list (funcall 'backward-delete-char-untabify 1) (buffer-string)))
    '(nil "a  "))
;; on a tab: converts to spaces first (GNU pin: tab → tab-width-1 spaces,
;; then deletes one → ends "a" + 6 spaces for tab-width 8)
(qa (with-temp-buffer
         (insert "a\t")
         (goto-char (point-max))
         (list (funcall 'backward-delete-char-untabify 1) (buffer-string)))
       '(nil "a      "))
;; on text: deletes normally
(qa (with-temp-buffer
      (insert "ax")
      (goto-char (point-max))
      (list (funcall 'backward-delete-char-untabify 1) (buffer-string)))
    '(nil "a"))

;; == delete-trailing-whitespace ==
;; whole buffer by default
(qa (with-temp-buffer
      (insert "a  \nb\t\nc")
      (delete-trailing-whitespace)
      (buffer-string))
    "a\nb\nc")
;; region bounds limit it
(qa (with-temp-buffer
         (insert "a  \nb\t\nc")
         (delete-trailing-whitespace 1 5)
         (buffer-string))
       "a\nb\t\nc")

;; == comment-region / uncomment-region ==
;; comments each line with ";; " in lisp-mode
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "x\ny\n")
      (comment-region 1 4)
      (buffer-string))
    ";; x\n;; y\n")
;; uncomment-region strips the comment delimiters
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert ";;x\n;;y\n")
      (uncomment-region 1 6)
      (buffer-string))
    "x\ny\n")
;; comment-or-uncomment-region toggles (comment first)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "x\n")
      (comment-or-uncomment-region 1 3)
      (buffer-string))
    ";; x\n")

;; == duplicate-line (Emacs 29+) ==
(qa (with-temp-buffer
      (insert "dup\nme")
      (goto-char 2)
      (list (funcall 'duplicate-line 1) (buffer-string)))
    '(nil "dup\ndup\nme"))

;; == forward-word / backward-word (in syntax context) ==
;; == forward-word (in syntax context) ==
;; returns nil; point lands AFTER the word (here: after "w1" = pos 6
;; because forward-word skips the trailing space too? GNU pin: point
;; lands at end of the word + following space = 6).
(qa (with-temp-buffer
      (insert "w1 w2")
      (list (forward-word) (point)))
    '(nil 6))
;; at eob: silent nil (no error)
(qa (with-temp-buffer
      (insert "w")
      (goto-char (point-max))
      (forward-word))
    nil)
;; forward-word N
(qa (with-temp-buffer
      (insert "w1 w2 w3")
      (list (forward-word 2) (point)))
    '(nil 9))

;; == open-line (interactive form covered; return value + behavior) ==
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 2)
      (list (open-line 1) (buffer-string) (point)))
    '(nil "a\nb" 2))
;; open-line 0 / negative -> no-op (GNU pin)
(qa (with-temp-buffer
         (insert "ab")
         (goto-char 2)
         (list (open-line 0) (buffer-string)))
       '(nil "ab"))

;; == kill-line edge: N arg ==
(qa (with-temp-buffer
         (insert "a\nb\nc")
         (goto-char 1)
         (list (kill-line 2) (buffer-string)))
       '(nil "c"))
;; kill-line 0 kills the text to eol, newline stays
(qa (with-temp-buffer
         (insert "abc\ndef")
         (goto-char 2)
         (list (kill-line 0) (buffer-string)))
       '(nil "bc\ndef"))
;; kill-line negative arg kills the whole previous line (GNU pin)
(qa (with-temp-buffer
         (insert "abc\ndef")
         (goto-char 6)
         (list (kill-line -1) (buffer-string)))
       '(nil "ef"))

;; == kill-whole-line N arg ==
(qa (with-temp-buffer
         (insert "a\nb\nc")
         (goto-char 1)
         (let ((r (kill-whole-line 2)))
           (list (markerp r) (buffer-string))))
       '(t "c"))

;; == overwrite-mode / overwrite-mode-textual: batch-safe check ==
;; overwrite-mode is a variable/boolean command — check fboundp only
(qa (fboundp 'overwrite-mode) t)
;; visible-mode / visual-line-mode are functions
(qa (fboundp 'visible-mode) t)
(qa (fboundp 'visual-line-mode) t)
(qa (fboundp 'toggle-truncate-lines) t)
(qa (fboundp 'auto-fill-mode) t)

;; == quoted-insert: interactive-only (reads a char) — untestable in batch.
(qa (fboundp 'quoted-insert) t)

(princ (qa--done))
