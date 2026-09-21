//! Startup Lisp: the subr.el subset remacs loads at interpreter boot.
//! These are real Lisp macros/functions (as in Emacs), not Rust code.

/// Source evaluated once per `Interp::new`.
pub const PRELUDE: &str = r#"
;; -*- lexical-binding: nil -*-

;; ---------- control-flow macros ----------

(defmacro when (cond &rest body)
  "If COND yields non-nil, do BODY, else return nil."
  (list 'if cond (cons 'progn body)))

(defmacro unless (cond &rest body)
  "If COND yields nil, do BODY."
  (cons 'if (cons cond (cons nil body))))

(defmacro dolist (spec &rest body)
  "Loop over a list.
Evaluate BODY with VAR bound to each element of LIST, in turn.
Then evaluate RESULT (default nil) with VAR bound to nil."
  (let ((tail (make-symbol "tail")))
    (append
     (list 'let (list (list tail (nth 1 spec)))
           (list 'while tail
                 (append (list 'let (list (list (nth 0 spec)
                                              (list 'car tail))))
                         body
                         (list (list 'setq tail (list 'cdr tail))))))
     (cdr (cdr spec)))))

(defmacro dotimes (spec &rest body)
  "Loop a certain number of times.
Evaluate BODY with VAR bound to successive integers from 0,
inclusive, to COUNT, exclusive."
  (let ((count (make-symbol "dotimes")))
    (list 'let (list (list count (nth 1 spec))
                     (list (nth 0 spec) 0))
          (cons 'while
                (cons (list '< (nth 0 spec) count)
                      (append body
                              (list (list 'setq (nth 0 spec)
                                          (list '1+ (nth 0 spec)))))))
          (nth 2 spec))))

(defmacro ignore-errors (&rest body)
  "Execute BODY; if an error occurs, return nil."
  (list 'condition-case nil (cons 'progn body) '(error nil)))

;; ---------- buffer macros ----------

(defmacro with-temp-buffer (&rest body)
  "Create a temporary buffer, and evaluate BODY there like `progn'."
  (let ((temp-buffer (make-symbol "temp-buffer")))
    (list 'let (list (list temp-buffer '(generate-new-buffer " *temp*")))
          (list 'with-current-buffer temp-buffer
                (list 'unwind-protect
                      (cons 'progn body)
                      (list 'and
                            (list 'buffer-name temp-buffer)
                            (list 'kill-buffer temp-buffer)))))))

(defmacro with-temp-file (file &rest body)
  "Create a temporary buffer, evaluate BODY, write it to FILE."
  (let ((temp-buffer (make-symbol "temp-buffer"))
        (temp-file (make-symbol "temp-file")))
    (list 'let (list (list temp-file file)
                     (list temp-buffer '(generate-new-buffer " *temp file*")))
          (list 'with-current-buffer temp-buffer
                (list 'unwind-protect
                      (list 'prog1 (cons 'progn body)
                            (list 'write-region nil nil temp-file nil 0))
                      (list 'and
                            (list 'buffer-name temp-buffer)
                            (list 'kill-buffer temp-buffer)))))))

;; `save-current-buffer' is a primitive special form (as in GNU), not a
;; macro — see special.rs.

(defmacro push (newelt place)
  "Add NEWELT to the list stored in symbol PLACE."
  (let ((v (if (and (consp place) (eq (car place) 'quote))
               (cadr place)
             place)))
    (list 'setq v (list 'cons newelt v))))

(defmacro pop (place)
  "Return and remove the first element of the list in PLACE."
  (let ((v (if (and (consp place) (eq (car place) 'quote))
               (cadr place)
             place)))
    (list 'prog1 (list 'car v) (list 'setq v (list 'cdr v)))))

(defmacro setq-local (var val)
  "Make variable VAR buffer-local and set it to VAL."
  (list 'set (list 'make-local-variable (list 'quote var)) val))

;; ---------- small functions ----------

(defun number-sequence (from &optional to inc)
  "Return list of numbers FROM to TO by INC.
If TO is nil or equal to FROM, return (FROM)."
  (if (or (not to) (= from to))
      (list from)
    (or inc (setq inc 1))
    (let ((res nil) (n from))
      (if (> inc 0)
          (while (<= n to)
            (setq res (cons n res))
            (setq n (+ n inc)))
        (while (>= n to)
          (setq res (cons n res))
          (setq n (+ n inc))))
      (nreverse res))))

(defmacro with-output-to-string (&rest body)
  "Execute BODY with `standard-output' bound to a temporary buffer,
returning that buffer's contents as a string."
  (let ((buf (make-symbol "out-buffer")))
    (list 'let (list (list buf '(generate-new-buffer " *string-output*")))
          (list 'unwind-protect
                (list 'let (list (list 'standard-output buf))
                      (cons 'progn body)
                      (list 'with-current-buffer buf '(buffer-string)))
                (list 'and
                      (list 'buffer-name buf)
                      (list 'kill-buffer buf))))))

;; ---------- string predicates ----------
;; GNU: string-equal/string-lessp/string-greaterp are primitives that accept
;; strings or symbols; the =/</> spellings are Lisp-level aliases.
;; string-empty-p et al. are Lisp defuns in subr.el.

(defalias 'string= 'string-equal)
(defalias 'string< 'string-lessp)
(defalias 'string> 'string-greaterp)

(defun string-greaterp (string1 string2)
  "Return non-nil if STRING1 is greater than STRING2 in lexicographic order.
Symbols are also allowed; their print names are used instead."
  (string-lessp string2 string1))

(defun string-empty-p (string)
  "Check whether STRING is empty."
  (string= string ""))

(defun string-blank-p (string)
  "Check whether STRING is either empty or only whitespace."
  (string-match-p "\\`[ \11\n\15]*\\'" string))

(defun string-prefix-p (prefix string &optional ignore-case)
  "Return non-nil if STRING begins with PREFIX."
  (let ((prefix-length (length prefix)))
    (if (> prefix-length (length string)) nil
      (eq t (compare-strings prefix 0 prefix-length string
                             0 prefix-length ignore-case)))))

(defun string-suffix-p (suffix string &optional ignore-case)
  "Return non-nil if STRING ends with SUFFIX."
  (let ((start-pos (- (length string) (length suffix))))
    (and (>= start-pos 0)
         (eq t (compare-strings suffix nil nil
                                string start-pos nil ignore-case)))))

;; ---------- cl-lib list accessors ----------
;; These are plain Lisp defaliases in cl-lib.el / cl-macs.el.

(defalias 'cl-first 'car)
(defalias 'cl-second 'cadr)
(defalias 'cl-third 'caddr)
(defalias 'cl-fourth 'cadddr)
(defalias 'cl-fifth 'fifth)
(defalias 'cl-sixth 'sixth)
(defalias 'cl-seventh 'seventh)
(defalias 'cl-eighth 'eighth)
(defalias 'cl-ninth 'ninth)
(defalias 'cl-tenth 'tenth)
(defalias 'cl-rest 'cdr)
(defalias 'cl-endp 'null)

(defun cl-list-length (x)
  "Return the length of list X, or nil for a circular/dotted list."
  (let ((n 0) (tail x))
    (while (consp tail)
      (setq tail (cdr tail))
      (setq n (1+ n)))
    (and (null tail) n)))

;; ---------- more subr.el-style helpers ----------

(defmacro with-silent-modifications (&rest body)
  "Execute BODY, suppressing modification hooks (simplified)."
  (cons 'progn body))

(defun member-ignore-case (elt list)
  "Like `member', but ignore string case differences."
  (let ((tail list))
    (while (and tail
                (not (if (and (stringp (car tail)) (stringp elt))
                         (eq t (compare-strings (car tail) 0 nil elt 0 nil t))
                       (equal (car tail) elt))))
      (setq tail (cdr tail)))
    tail))

(defun delete-dups (list)
  "Destructively remove duplicate `equal' elements from LIST."
  (let ((tail list))
    (while tail
      (setcdr tail (delete (car tail) (cdr tail)))
      (setq tail (cdr tail))))
  list)

;; ---------- simple.el-style interactive commands ----------

(defun next-line (&optional arg try-vscroll)
  "Move cursor vertically down ARG lines."
  (interactive "^p\nP")
  (forward-line (or arg 1)))

(defun previous-line (&optional arg try-vscroll)
  "Move cursor vertically up ARG lines."
  (interactive "^p\nP")
  (forward-line (- (or arg 1))))

(defun beginning-of-buffer (&optional arg)
  "Move point to the beginning of the buffer."
  (interactive "^P")
  (goto-char (point-min)))

(defun end-of-buffer (&optional arg)
  "Move point to the end of the buffer."
  (interactive "^P")
  (goto-char (point-max)))

(defun mark-whole-buffer ()
  "Put point at beginning and mark at end of buffer."
  (interactive)
  (push-mark (point))
  (push-mark (point-max) nil t)
  (goto-char (point-min)))

(defun set-mark-command (arg)
  "Set the mark at point, or jump to the mark with a prefix argument."
  (interactive "P")
  (if arg
      (when (mark)
        (goto-char (mark))
        (deactivate-mark))
    (push-mark)))

(defun keyboard-quit ()
  "Signal a `quit' condition (C-g)."
  (interactive)
  (signal 'quit nil))

(defun keyboard-escape-quit ()
  "Abort the current operation (ESC ESC ESC)."
  (interactive)
  (signal 'quit nil))

(defun mark-word (arg)
  "Set mark ARG words from point."
  (interactive "P")
  (set-mark (point))
  (forward-word (prefix-numeric-value arg)))

(defun mark-sexp (arg)
  "Set mark ARG sexps from point."
  (interactive "P")
  (set-mark (point))
  (forward-sexp (prefix-numeric-value arg)))

(defun mark-paragraph (&optional arg)
  "Put mark at end of this paragraph, point at beginning."
  (interactive "P")
  (forward-paragraph (or arg 1))
  (push-mark nil t t)
  (backward-paragraph))

(defun back-to-indentation ()
  "Move point to the first non-whitespace character on this line."
  (interactive "^")
  (beginning-of-line)
  (skip-chars-forward " \t"))

(defun goto-line (line)
  "Go to LINE, counting from line 1 at beginning of buffer."
  (interactive "NGoto line: ")
  (goto-char (point-min))
  (forward-line (1- line)))

(defun recenter-top-bottom (&optional arg)
  "Center point in window; with ARG, cycle positions."
  (interactive "P")
  (recenter arg))

(defun quoted-insert (arg)
  "Read next input character and insert it ARG times."
  (interactive "*p")
  (insert-char (read-char) arg))

(defun indent-for-tab-command (&optional arg)
  "Indent the current line (inserts a tab in fundamental mode)."
  (interactive "P")
  (insert "\t"))

(defun indent-relative (&optional unindented-ok)
  "Space out to under next indent point in previous nonblank line."
  (interactive "P")
  (insert "\t"))

(defun newline-and-indent ()
  "Insert a newline, then indent."
  (interactive "*")
  (newline)
  (insert "\t"))

(defun transpose-words (arg)
  "Interchange the word at point with the previous word ARG times."
  (interactive "*p")
  (transpose-subr 'forward-word arg))

(defun transpose-sexps-default-function (arg)
  "Default method to locate a pair of points for `transpose-sexps'."
  (if (if (> arg 0)
          (looking-at "\\sw\\|\\s_")
        (and (not (bobp))
             (save-excursion
               (forward-char -1)
               (looking-at "\\sw\\|\\s_"))))
      ;; Jumping over a symbol.  We might be inside it, mind you.
      (progn (funcall (if (> arg 0)
                          #'skip-syntax-backward #'skip-syntax-forward)
                      "w_")
             (cons (save-excursion (forward-sexp arg) (point)) (point)))
    ;; Otherwise, we're between sexps.  Take a step back before jumping
    ;; to make sure we'll obey the same precedence no matter which
    ;; direction we're going.
    (funcall (if (> arg 0) #'skip-syntax-backward #'skip-syntax-forward)
             " .")
    (cons (save-excursion (forward-sexp arg) (point))
          (progn (while (or (forward-comment (if (> arg 0) 1 -1))
                            (not (zerop (funcall (if (> arg 0)
                                                     #'skip-syntax-forward
                                                   #'skip-syntax-backward)
                                                 ".")))))
                 (point)))))

(defun transpose-sexps (arg)
  "Like \\[transpose-chars] (`transpose-chars'), but applies to sexps."
  (interactive "*p")
  (transpose-subr 'transpose-sexps-default-function arg 'special))

(defun transpose-subr-1 (pos1 pos2)
  (unless (and pos1 pos2)
    (error "Don't have two things to transpose"))
  (when (> (car pos1) (cdr pos1)) (setq pos1 (cons (cdr pos1) (car pos1))))
  (when (> (car pos2) (cdr pos2)) (setq pos2 (cons (cdr pos2) (car pos2))))
  (when (> (car pos1) (car pos2))
    (let ((swap pos1))
      (setq pos1 pos2 pos2 swap)))
  (if (> (cdr pos1) (car pos2)) (error "Don't have two things to transpose"))
  (let* ((a (buffer-substring (car pos1) (cdr pos1)))
         (m (buffer-substring (cdr pos1) (car pos2)))
         (b (buffer-substring (car pos2) (cdr pos2))))
    (delete-region (car pos1) (cdr pos2))
    (goto-char (car pos1))
    (insert b m a)))

(defun transpose-subr (mover arg &optional special)
  "Subroutine to do the work of transposing objects."
  (let ((aux (if special mover
               (lambda (x)
                 (cons (progn (funcall mover x) (point))
                       (progn (funcall mover (- x)) (point))))))
        pos1 pos2)
    (cond
     ((= arg 0)
      (save-excursion
        (setq pos1 (funcall aux 1))
        (goto-char (or (mark) (error "No mark set in this buffer")))
        (setq pos2 (funcall aux 1))
        (transpose-subr-1 pos1 pos2))
      (exchange-point-and-mark))
     ((> arg 0)
      (setq pos1 (funcall aux -1))
      (setq pos2 (funcall aux arg))
      (transpose-subr-1 pos1 pos2)
      (goto-char (car pos2)))
     (t
      (setq pos1 (funcall aux -1))
      (goto-char (car pos1))
      (setq pos2 (funcall aux arg))
      (transpose-subr-1 pos1 pos2)
      (goto-char (+ (car pos2) (- (cdr pos1) (car pos1))))))))

(defun undo-only (&optional arg)
  "Undo some previous changes (no redo)."
  (interactive "p")
  (undo arg))

(defun undo-redo (&optional arg)
  "Redo some previously undone changes."
  (interactive "p")
  (undo arg))

(defun revert-buffer (&rest _ignore)
  "Replace the buffer text with the contents of the visited file."
  (interactive)
  (when buffer-file-name
    (erase-buffer)
    (insert-file-contents buffer-file-name)))

(defun insert-file (filename)
  "Insert the contents of FILENAME into the buffer after point."
  (interactive "*fInsert file: ")
  (insert-file-contents filename))

(defun read-only-mode (&optional arg)
  "Toggle whether the buffer is read-only."
  (interactive "P")
  (setq buffer-read-only (if arg (> (prefix-numeric-value arg) 0)
                           (not buffer-read-only))))

(defun toggle-read-only (&optional arg)
  "Change whether this buffer is read-only."
  (interactive "P")
  (read-only-mode arg))

(defun kill-sexp (&optional arg interactive)
  "Kill the sexp (balanced expression) following point.
With ARG, kill that many sexps after point.
Negative arg -N means kill N sexps before point.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "p\nd")
  (if interactive
      (condition-case _
          (kill-sexp arg nil)
        (scan-error (user-error (if (> arg 0)
                                    "No next sexp"
                                  "No previous sexp"))))
    (let ((opoint (point)))
      (forward-sexp (or arg 1))
      (kill-region opoint (point)))))

(defun backward-kill-sexp (&optional arg interactive)
  "Kill the sexp (balanced expression) preceding point.
With ARG, kill that many sexps before point.
Negative arg -N means kill N sexps after point.
This command assumes point is not in a string or comment.
If INTERACTIVE is non-nil, as it is interactively,
report errors as appropriate for this kind of usage."
  (interactive "p\nd")
  (kill-sexp (- (or arg 1)) interactive))

(defun kill-sentence (&optional arg)
  "Kill from point to end of sentence."
  (interactive "p")
  (kill-region (point) (save-excursion (forward-sentence arg) (point))))

(defun backward-kill-sentence (&optional arg)
  "Kill back from point to start of sentence."
  (interactive "p")
  (kill-region (point) (save-excursion (backward-sentence arg) (point))))

(defun kill-paragraph (arg)
  "Kill forward to end of paragraph.
With ARG N, kill forward to Nth end of paragraph;
negative ARG -N means kill backward to Nth start of paragraph."
  (interactive "p")
  (kill-region (point) (progn (forward-paragraph arg) (point))))

(defun backward-kill-paragraph (arg)
  "Kill back to start of paragraph.
With ARG N, kill back to Nth start of paragraph;
negative ARG -N means kill forward to Nth end of paragraph."
  (interactive "p")
  (kill-region (point) (progn (backward-paragraph arg) (point))))

(defun transpose-paragraphs (arg)
  "Interchange the current paragraph with the next one.
With prefix argument ARG a non-zero integer, moves the current
paragraph past ARG paragraphs, leaving point after the current paragraph.
If ARG is positive, moves the current paragraph forwards, if
ARG is negative moves it backwards.  If ARG is zero, exchanges
the current paragraph with the one containing the mark."
  (interactive "*p")
  (transpose-subr 'forward-paragraph arg))

(defun transpose-sentences (arg)
  "Interchange the current sentence with the next one.
With prefix argument ARG a non-zero integer, moves the current
sentence past ARG sentences, leaving point after the current sentence.
If ARG is positive, moves the current sentence forwards, if
ARG is negative moves it backwards.  If ARG is zero, exchanges
the current sentence with the one containing the mark."
  (interactive "*p")
  (transpose-subr 'forward-sentence arg))

(defun start-of-paragraph-text ()
  "Move to the start of the current paragraph."
  (let ((opoint (point)) npoint)
    (forward-paragraph -1)
    (setq npoint (point))
    (skip-chars-forward " \t\n")
    ;; If the range of blank lines found spans the original start point,
    ;; try again from the beginning of it.
    ;; Must be careful to avoid infinite loop
    ;; when following a single return at start of buffer.
    (if (and (>= (point) opoint) (< npoint opoint))
	(progn
	  (goto-char npoint)
	  (if (> npoint (point-min))
	      (start-of-paragraph-text))))))

(defun end-of-paragraph-text ()
  "Move to the end of the current paragraph."
  (let ((opoint (point)))
    (forward-paragraph 1)
    (if (eq (preceding-char) ?\n) (forward-char -1))
    (if (<= (point) opoint)
	(progn
	  (forward-char 1)
	  (if (< (point) (point-max))
	      (end-of-paragraph-text))))))

(defun mark-end-of-sentence (arg)
  "Put mark at end of sentence.
ARG works as in `forward-sentence'.  If this command is repeated,
it marks the next ARG sentences after the ones already marked."
  (interactive "p")
  (push-mark
   (save-excursion
     (if (and (eq last-command this-command) (mark t))
	 (goto-char (mark)))
     (forward-sentence arg)
     (point))
   nil t))

(defun forward-paragraph (&optional arg)
  "Move forward to end of paragraph.
With argument ARG, do it ARG times;
a negative argument ARG = -N means move backward N paragraphs.

A line which `paragraph-start' matches either separates paragraphs
\(if `paragraph-separate' matches it also) or is the first line of a paragraph.
A paragraph end is the beginning of a line which is not part of the paragraph
to which the end of the previous line belongs, or the end of the buffer.
Returns the count of paragraphs left to move."
  (interactive "^p")
  (or arg (setq arg 1))
  (let* ((opoint (point))
	 (fill-prefix-regexp
	  (and (boundp 'fill-prefix)
	       fill-prefix (not (equal fill-prefix ""))
	       (not paragraph-ignore-fill-prefix)
	       (regexp-quote fill-prefix)))
	 ;; Remove ^ from paragraph-start and paragraph-sep if they are there.
	 ;; These regexps shouldn't be anchored, because we look for them
	 ;; starting at the left-margin.  This allows paragraph commands to
	 ;; work normally with indented text.
	 (parstart (if (and (not (equal "" paragraph-start))
			    (equal ?^ (aref paragraph-start 0)))
		       (substring paragraph-start 1)
		     paragraph-start))
	 (parsep (if (and (not (equal "" paragraph-separate))
			  (equal ?^ (aref paragraph-separate 0)))
		     (substring paragraph-separate 1)
		   paragraph-separate))
	 (parsep
	  (if fill-prefix-regexp
	      (concat parsep "\\|"
		      fill-prefix-regexp "[ \t]*$")
	    parsep))
	 ;; This is used for searching.
	 (sp-parstart (concat "^[ \t]*\\(?:" parstart "\\|" parsep "\\)"))
	 start found-start)
    (while (and (< arg 0) (not (bobp)))
      (if (and (not (looking-at parsep))
	       (re-search-backward "^\n" (max (1- (point)) (point-min)) t)
	       (looking-at parsep))
	  (setq arg (1+ arg))
	(setq start (point))
	;; Move back over paragraph-separating lines.
	(forward-char -1) (beginning-of-line)
	(while (and (not (bobp))
		    (progn (move-to-left-margin)
			   (looking-at parsep)))
	  (forward-line -1))
	(if (bobp)
	    nil
	  (setq arg (1+ arg))
	  ;; Go to end of the previous (non-separating) line.
	  (end-of-line)
	  ;; Search back for line that starts or separates paragraphs.
	  (if (if fill-prefix-regexp
		  ;; There is a fill prefix; it overrides parstart.
		  (progn
		    (while (and (progn (beginning-of-line) (not (bobp)))
				(progn (move-to-left-margin)
				       (not (looking-at parsep)))
				(looking-at fill-prefix-regexp))
		      (forward-line -1))
		    (move-to-left-margin)
		    (not (bobp)))
		(while (and (re-search-backward sp-parstart nil 1)
			    (setq found-start t)
			    ;; Found a candidate, but need to check if it is a
			    ;; REAL parstart.
			    (progn (setq start (point))
				   (move-to-left-margin)
				   (not (looking-at parsep)))
			    (not (and (looking-at parstart)
				      (or (not use-hard-newlines)
					  (bobp)
					  (get-text-property
					   (1- start) 'hard)))))
		  (setq found-start nil)
		  (goto-char start))
		found-start)
	      ;; Found one.
	      (progn
		;; Move forward over paragraph separators.
		;; We know this cannot reach the place we started
		;; because we know we moved back over a non-separator.
		(while (and (not (eobp))
			    (progn (move-to-left-margin)
				   (looking-at parsep)))
		  (forward-line 1))
		;; If line before paragraph is just margin, back up to there.
		(end-of-line 0)
		(if (> (current-column) (current-left-margin))
		    (forward-char 1)
		  (skip-chars-backward " \t")
		  (if (not (bolp))
		      (forward-line 1))))
	    ;; No starter or separator line => use buffer beg.
	    (goto-char (point-min))))))

    (while (and (> arg 0) (not (eobp)))
      ;; Move forward over separator lines...
      (while (and (not (eobp))
		  (progn (move-to-left-margin) (not (eobp)))
		  (looking-at parsep))
	(forward-line 1))
      (unless (eobp) (setq arg (1- arg)))
      ;; ... and one more line.
      (forward-line 1)
      (if fill-prefix-regexp
	  ;; There is a fill prefix; it overrides parstart.
	  (while (and (not (eobp))
		      (progn (move-to-left-margin) (not (eobp)))
		      (not (looking-at parsep))
		      (looking-at fill-prefix-regexp))
	    (forward-line 1))
	(while (and (re-search-forward sp-parstart nil 1)
		    (progn (setq start (match-beginning 0))
			   (goto-char start)
			   (not (eobp)))
		    (progn (move-to-left-margin)
			   (not (looking-at parsep)))
		    (or (not (looking-at parstart))
			(and use-hard-newlines
			     (not (get-text-property (1- start) 'hard)))))
	  (forward-char 1))
	(if (< (point) (point-max))
	    (goto-char start))))
    (constrain-to-field nil opoint t)
    ;; Return the number of steps that could not be done.
    arg))

(defun backward-paragraph (&optional arg)
  "Move backward to start of paragraph.
With argument ARG, do it ARG times;
a negative argument ARG = -N means move forward N paragraphs.

A paragraph start is the beginning of a line which is a
`paragraph-start' or which is ordinary text and follows a
`paragraph-separate'ing line; except: if the first real line of a
paragraph is preceded by a blank line, the paragraph starts at that
blank line.

See `forward-paragraph' for more information."
  (interactive "^p")
  (or arg (setq arg 1))
  (forward-paragraph (- arg)))

(defvar sentence-end-double-space t
  "Non-nil means a single space does not end a sentence.")

(defvar sentence-end-without-period nil
  "Non-nil means a sentence will end without a period.")

(defvar sentence-end-without-space "。．？！"
  "String of characters that end sentence without following spaces.")

(defvar sentence-end-base "[.?!…‽][]\"'”’)}»›]*"
  "Regexp matching the basic end of a sentence, not including following space.")

(defun sentence-end ()
  "Return the regexp describing the end of a sentence.

This function returns either the value of the variable `sentence-end'
if it is non-nil, or the default value constructed from the
variables `sentence-end-base', `sentence-end-double-space',
`sentence-end-without-period' and `sentence-end-without-space'."
  (or sentence-end
      ;; We accept non-break space along with space.
      (concat (if sentence-end-without-period "\\w[ \u00a0][ \u00a0]\\|")
	      "\\("
	      sentence-end-base
              (if sentence-end-double-space
                  "\\($\\|[ \u00a0]$\\|\t\\|[ \u00a0][ \u00a0]\\)" "\\($\\|[\t \u00a0]\\)")
              "\\|[" sentence-end-without-space "]+"
	      "\\)"
              "[ \u00a0\t\n]*")))

(defun forward-sentence-default-function (&optional arg)
  "Move forward to next end of sentence.  With argument, repeat.
When ARG is negative, move backward repeatedly to start of sentence.

The variable `sentence-end' is a regular expression that matches ends of
sentences.  Also, every paragraph boundary terminates sentences as well."
  (or arg (setq arg 1))
  (let ((opoint (point))
        (sentence-end (sentence-end)))
    (while (< arg 0)
      (let ((pos (point))
	    par-beg par-text-beg)
	(save-excursion
	  (start-of-paragraph-text)
	  ;; Start of real text in the paragraph.
	  ;; We move back to here if we don't see a sentence-end.
	  (setq par-text-beg (point))
	  ;; Start of the first line of the paragraph.
	  ;; We use this as the search limit
	  ;; to allow sentence-end to match if it is anchored at
	  ;; BOL and the paragraph starts indented.
	  (beginning-of-line)
	  (setq par-beg (point)))
	(if (and (re-search-backward sentence-end par-beg t)
		 (or (< (match-end 0) pos)
		     (re-search-backward sentence-end par-beg t)))
	    (goto-char (match-end 0))
	  (goto-char par-text-beg)))
      (setq arg (1+ arg)))
    (while (> arg 0)
      (let ((par-end (save-excursion (end-of-paragraph-text) (point))))
	(if (re-search-forward sentence-end par-end t)
	    (skip-chars-backward " \t\n")
	  (goto-char par-end)))
      (setq arg (1- arg)))
    (constrain-to-field nil opoint t)))

(defvar forward-sentence-function #'forward-sentence-default-function
  "Function to be used to calculate sentence movements.
See `forward-sentence' for a description of its behavior.")

(defun forward-sentence (&optional arg)
  "Move forward to next end of sentence.  With argument ARG, repeat.
If ARG is negative, move backward repeatedly to start of
sentence.  Delegates its work to `forward-sentence-function'."
  (interactive "^p")
  (or arg (setq arg 1))
  (funcall forward-sentence-function arg))

(defun backward-sentence (&optional arg)
  "Move backward to start of sentence.  With argument, repeat.
With negative argument, move forward repeatedly to end of sentence.
See `forward-sentence' for more information."
  (interactive "^p")
  (or arg (setq arg 1))
  (forward-sentence (- arg)))

(defun count-sentences (start end)
  "Count sentences in current buffer from START to END."
  (let ((sentences 0)
        (inhibit-field-text-motion t))
    (save-excursion
      (save-restriction
        (narrow-to-region start end)
        (goto-char (point-min))
        (let* ((prev (point))
               (next (forward-sentence)))
          (while (and (not (null next))
                      (not (= prev next)))
            (setq prev next
                  next (ignore-errors (forward-sentence))
                  sentences (1+ sentences))))
        ;; Remove last possibly empty sentence
        (when (/= (skip-chars-backward " \t\n") 0)
          (setq sentences (1- sentences)))
	sentences))))

(defun forward-page (&optional count)
  "Move forward to page boundary."
  (interactive "^p")
  (skip-chars-forward "\n")
  (if (re-search-forward page-delimiter nil t (or count 1))
      (goto-char (match-end 0))
    (goto-char (point-max))))

(defun backward-page (&optional count)
  "Move backward to page boundary."
  (interactive "^p")
  (if (re-search-backward "\f" nil t (or count 1))
      (goto-char (match-end 0))
    (goto-char (point-min))))

(defun mark-page (&optional page)
  "Put mark at end of page, point at beginning."
  (interactive "p")
  (forward-page page)
  (push-mark nil t t)
  (backward-page))

(defun center-line (&optional nlines)
  "Center the line point is on (approximate: no-op when unsupported)."
  (interactive "P")
  nlines)

;; `move-to-window-line' is a real subr now (see builtins/misc.rs).

;; ---------- help commands ----------

(defun help-buffer ()
  "Return the *Help* buffer."
  (get-buffer-create "*Help*"))

(defun pop-to-buffer (buffer &optional _action _norecord)
  "Select BUFFER in some window, preferring the current one."
  (interactive "bPop to buffer: ")
  (let ((b (get-buffer-create (if (stringp buffer) buffer
                                (buffer-name buffer)))))
    (set-window-buffer (selected-window) b)
    (set-buffer b)
    b))

(defun switch-to-buffer (buffer)
  "Select BUFFER in the current window."
  (interactive "BSwitch to buffer: ")
  (let ((b (get-buffer-create (if (stringp buffer) buffer
                                (buffer-name buffer)))))
    (set-window-buffer (selected-window) b)
    (set-buffer b)
    b))

(defun list-buffers (&optional files-only)
  "Display a list of existing buffers."
  (interactive "P")
  (let ((b (get-buffer-create "*Buffer List*")))
    (with-current-buffer b
      (erase-buffer)
      (dolist (buf (buffer-list))
        (unless (and files-only (not (buffer-local-value 'buffer-file-name buf)))
          (insert (format "%-24s %s\n"
                          (buffer-name buf)
                          (or (buffer-local-value 'buffer-file-name buf) ""))))))
    (pop-to-buffer b)))

(defun describe-function (function)
  "Display the documentation of FUNCTION."
  (interactive "aDescribe function: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert (symbol-name function) " is "
              (cond ((commandp function) "an interactive command")
                    ((subrp function) "a built-in function")
                    (t "a function"))
              ".\n\n"
              (or (documentation function) "Not documented.")
              "\n"))
    (pop-to-buffer b)))

(defun describe-variable (variable)
  "Display the documentation and value of VARIABLE."
  (interactive "vDescribe variable: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert (symbol-name variable) " is "
              (if (boundp variable)
                  (concat "bound; its value is\n"
                          (prin1-to-string (symbol-value variable)))
                "void")
              "\n"))
    (pop-to-buffer b)))

(defun describe-key (key)
  "Display the command bound to KEY."
  (interactive "kDescribe key: ")
  (let* ((b (help-buffer))
         (def (key-binding key)))
    (with-current-buffer b
      (erase-buffer)
      (insert (key-description key) " runs the command "
              (prin1-to-string def) "\n"))
    (pop-to-buffer b)))

(defun describe-bindings (&optional prefix buffer)
  "Display a list of key bindings."
  (interactive "P")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (insert "key             binding\n---             -------\n")
      (dolist (cell (cdr (current-global-map)))
        (when (consp cell)
          (let ((k (car cell)))
            (insert (if (eq k t) "default" (single-key-description k))
                    "\t\t"
                    (prin1-to-string (cdr cell)) "\n")))))
    (pop-to-buffer b)))

(defun describe-mode (&optional buffer)
  "Display the current buffer's mode."
  (interactive)
  (message "Mode: %s" (or major-mode "Fundamental")))

(defun apropos-command (pattern)
  "Show commands whose names match PATTERN."
  (interactive "sApropos command: ")
  (let ((b (help-buffer)))
    (with-current-buffer b
      (erase-buffer)
      (dolist (s (apropos-internal pattern t))
        (when (commandp s)
          (insert (symbol-name s) "\n"))))
    (pop-to-buffer b)))

(defun eval-last-sexp (&optional eval-last-sexp-arg-internal)
  "Evaluate the sexp before point."
  (interactive "P")
  (save-excursion
    (backward-sexp)
    (let ((v (eval (read (current-buffer)))))
      (message "%s" v)
      v)))

(defun save-some-buffers (&optional arg)
  "Save some modified file-visiting buffers."
  (interactive "P")
  (mapc (lambda (b)
          (with-current-buffer b
            (when (and buffer-file-name (buffer-modified-p))
              (save-buffer))))
        (buffer-list))
  t)

(defun save-buffers-kill-emacs (&optional arg)
  "Offer to save each buffer, then kill Emacs."
  (interactive "P")
  (save-some-buffers arg)
  (kill-emacs))

(defun isearch-forward (regexp-p string)
  "Search forward; prompts for a string (line-input fallback)."
  (interactive "P\nsI-search: ")
  (when (> (length string) 0)
    (if regexp-p
        (re-search-forward string nil t)
      (search-forward string nil t)))
  string)

(defun isearch-backward (regexp-p string)
  "Search backward; prompts for a string (line-input fallback)."
  (interactive "P\nsI-search backward: ")
  (when (> (length string) 0)
    (if regexp-p
        (re-search-backward string nil t)
      (search-backward string nil t)))
  string)

(defun isearch-forward-regexp (string)
  "Incremental regexp search forward (line-input fallback)."
  (interactive "sI-search regexp: ")
  (when (> (length string) 0)
    (re-search-forward string nil t))
  string)

(defun isearch-backward-regexp (string)
  "Incremental regexp search backward (line-input fallback)."
  (interactive "sI-search backward regexp: ")
  (when (> (length string) 0)
    (re-search-backward string nil t))
  string)

(defun query-replace (from-string to-string &optional delimited)
  "Replace occurrences of FROM-STRING with TO-STRING, asking."
  (interactive "sQuery replace: \nsQuery replace %s with: ")
  (let ((n 0))
    (while (search-forward from-string nil t)
      (when (y-or-n-p (format "Replace with %s? " to-string))
        (replace-match to-string)
        (setq n (1+ n))))
    (message "Replaced %d occurrence(s)" n)
    n))

(defun suspend-frame ()
  "Suspend the current frame (alias for suspend-emacs)."
  (interactive)
  (suspend-emacs))

(defun iconify-or-deiconify-frame ()
  "Iconify the selected frame (no-op on tty)."
  (interactive)
  nil)

;; ---------- command-loop variables ----------

(defvar this-command nil
  "The command now being executed.")
(defvar last-command nil
  "The last command executed.")
(defvar current-prefix-arg nil
  "The value of the prefix argument for this command.")
(defvar transient-mark-mode nil
  "Non-nil if Transient Mark mode is enabled.")
(defvar major-mode 'fundamental-mode
  "The major mode of the current buffer.")
(defvar buffer-read-only nil
  "Non-nil if the current buffer is read-only.")
(defvar command-args nil
  "Arguments supplied to the current command interactively.")
(defvar buffer-file-name nil
  "Name of file visited in the current buffer.")
(defvar left-margin 0
  "Column for the default `indent-line-function' to indent to.
Linefeed-indented lines are indented to this column.")

(defvar fill-prefix nil
  "Text for `fill-region' to put at the beginning of each line, or nil.")

(defvar use-hard-newlines nil
  "Non-nil means to distinguish between soft and hard newlines.")

(defvar paragraph-ignore-fill-prefix nil
  "Non-nil means the paragraph commands are not affected by `fill-prefix'.")

(defvar comment-column 32
  "Column to indent right-margin comments to.")
(defvar comment-start nil
  "String to insert to start a new comment, or nil if none.")
(defvar tab-stop-list '(8 16 24 32 40 48 56 64 72 80 88 96 104 112 120)
  "List of tab stop positions used by `tab-to-tab-stop'.")
(defvar indent-line-function 'indent-relative-first-indent-point
  "Function to be used to indent the current line.")

;; ---------- indentation commands ----------

(defun indent-to-column (col &optional minimum)
  "Indent to column COL, or to MINIMUM if already past COL."
  (interactive "NIndent to column: ")
  (indent-to col minimum))

(defun indent-rigidly-left (start end &optional count)
  "Indent all lines between START and END leftward by COUNT spaces."
  (interactive "r\nP")
  (indent-rigidly start end (- (or count 4))))

(defun indent-rigidly-right (start end &optional count)
  "Indent all lines between START and END rightward by COUNT spaces."
  (interactive "r\nP")
  (indent-rigidly start end (or count 4)))

(defun indent-code-rigidly (start end arg &optional nochange-regexp)
  "Indent the region between START and END rigidly by ARG columns."
  (interactive "r\nP")
  (indent-rigidly start end (prefix-numeric-value arg)))

(defun indent-relative (&optional unindented-ok first-only)
  "Space out to under next indent point in previous nonblank line."
  (interactive "P")
  (let ((stops (save-excursion
                 (beginning-of-line)
                 (when (re-search-backward "^[^\n]" nil t)
                   (end-of-line)
                   (let ((eol (point)) (pos nil) (in-run nil))
                     (beginning-of-line)
                     (while (< (point) eol)
                       (let ((c (char-after)))
                         (cond ((memq c '(?\s ?\t))
                                (setq in-run t))
                               (in-run
                                (push (current-column) pos)
                                (setq in-run nil))))
                       (forward-char 1))
                     (nreverse pos))))))
    (let* ((col (current-column))
           (beyond (delq nil (mapcar (lambda (c) (and (> c col) c)) stops)))
           (target (or (car beyond)
                       (and stops (if first-only (car stops) (car stops))))))
      (cond (target (indent-to target))
            (unindented-ok (indent-to 0))
            (t (tab-to-tab-stop))))))

(defun indent-relative-first-indent-point ()
  "Indent to the first indent stop of the previous nonblank line."
  (interactive)
  (indent-relative nil t))

(defun indent-relative-maybe ()
  "Indent like `indent-relative', defaulting to unindented."
  (interactive)
  (indent-relative t))

(defun indent-according-to-mode ()
  "Indent line in proper way for current major mode."
  (interactive)
  (funcall indent-line-function))

(defun indent-region (start end &optional column)
  "Indent each nonblank line in the region using `indent-line-function'."
  (interactive "r")
  (save-excursion
    (goto-char start)
    (setq end (copy-marker end))
    (while (< (point) end)
      (or (and (bolp) (eolp))
          (if column
              (indent-to-column column)
            (indent-according-to-mode)))
      (forward-line 1))))

(defun indent-sexp (&optional endpos)
  "Indent each line of the list starting just after point."
  (interactive "P")
  (let ((e (or endpos (save-excursion (forward-sexp 1) (point)))))
    (indent-region (point) e)))

(defun current-left-margin ()
  "Return the left margin to use for this line.
This is the value of the buffer-local variable `left-margin' plus the value
of the `left-margin' text-property at the start of the line."
  (save-excursion
    (back-to-indentation)
    (max 0
	 (+ left-margin (or (get-text-property
			     (if (and (eobp) (not (bobp)))
				 (1- (point)) (point))
			     'left-margin) 0)))))

(defun move-to-left-margin (&optional n force)
  "Move to the left margin of the current line.
With optional argument, move forward N-1 lines first.
The column moved to is the one given by the `current-left-margin' function.
If the line's indentation appears to be wrong, and this command is called
interactively or with optional argument FORCE, it will be fixed."
  (interactive (list (prefix-numeric-value current-prefix-arg) t))
  (beginning-of-line n)
  (skip-chars-forward " \t")
  (if (minibufferp (current-buffer))
      (if (save-excursion (beginning-of-line) (bobp))
	  (goto-char (minibuffer-prompt-end))
	(beginning-of-line))
    (let ((lm (current-left-margin))
	  (cc (current-column)))
      (cond ((> cc lm)
	     (if (> (move-to-column lm force) lm)
		 ;; If lm is in a tab and we are not forcing, move before tab
		 (backward-char 1)))
	    ((and force (< cc lm))
	     (indent-to-left-margin))))))

(defun indent-to-left-margin ()
  "Indent current line to the column given by `current-left-margin'."
  (save-excursion (indent-line-to (current-left-margin)))
  ;; If we are within the indentation, move past it.
  (when (save-excursion
	  (skip-chars-backward " \t")
	  (bolp))
    (skip-chars-forward " \t")))

(defun delete-to-left-margin (&optional from to)
  "Delete left margin indentation of each line between FROM and TO."
  (interactive "r")
  (let ((from (or from (point)))
        (to (or to (point))))
    (save-excursion
      (goto-char from)
      (while (< (point) to)
        (beginning-of-line)
        (let ((col (current-indentation)))
          (when (> col left-margin)
            (let ((beg (point)))
              (move-to-column left-margin t)
              (delete-region beg (point)))))
        (forward-line 1)))))

(defun center-region (from to &optional nlates)
  "Center each nonblank line between FROM and TO."
  (interactive "r\nP")
  (save-excursion
    (goto-char from)
    (while (< (point) to)
      (unless (and (bolp) (eolp))
        (center-line nlates))
      (forward-line 1))))

(defun indent-for-comment (&optional insert)
  "Indent this line's comment to `comment-column', or insert a comment."
  (interactive)
  (end-of-line)
  (or (eq (preceding-char) ?\s)
      (insert " "))
  (let ((comment-start (or comment-start ";")))
    (delete-horizontal-space)
    (indent-to comment-column 1)
    (insert comment-start)))

(defun insert-parentheses (&optional arg)
  "Enclose following ARG sexps in parentheses."
  (interactive "P")
  (or arg (setq arg 0))
  (insert ?\()
  (save-excursion
    (or (eq arg 0)
        (forward-sexp arg))
    (insert ?\))))

(defun beginning-of-line-text (&optional n)
  "Move to the beginning of the text on this line.
This is like `beginning-of-line', but skips past a comment prefix."
  (interactive "^p")
  (beginning-of-line n)
  (skip-chars-forward " \t"))

(defun fixup-whitespace ()
  "Fixup white space between objects around point.
Leave one space or none, according to the context."
  (interactive "*")
  (save-excursion
    (delete-horizontal-space)
    (if (or (looking-at "^\\|\\s)")
            (save-excursion (forward-char -1)
                            (looking-at "\\\|$\\|\\s(\\|\\s'")))
        nil
      (insert ?\s))))

;; ---------- named-function-key commands ----------
(defun left-char (&optional n)
  "Move point N characters to the left (to the right if N is negative)."
  (interactive "^p")
  (forward-char (- (prefix-numeric-value n))))

(defun right-char (&optional n)
  "Move point N characters to the right (to the left if N is negative)."
  (interactive "^p")
  (forward-char (prefix-numeric-value n)))

(defun delete-forward-char (&optional n killflag)
  "Delete the following N characters (previous if N is negative)."
  (interactive "p\nP")
  (delete-char (prefix-numeric-value n) killflag))

(defun overwrite-mode (&optional arg)
  "Placeholder for Emacs compatibility; no-op in this editor."
  (interactive "P")
  nil)

(defun help-command (key &optional flag)
  "Placeholder help dispatcher."
  (interactive "KHelp: \np")
  nil)

(defun menu-bar-open (&optional frame)
  "Placeholder menu-bar opener."
  (interactive "i\nF")
  nil)

(defun mouse-set-point (event &optional promote-to-region)
  "Move point to the position clicked on with the mouse."
  (interactive "e\np")
  nil)

(defun mouse-set-region (click)
  "Set the region to the interval dragged over."
  (interactive "e")
  nil)

(defun mwheel-scroll (event &optional arg)
  "Scroll up or down according to the EVENT."
  (interactive "e\nP")
  nil)

;; ---------- default global-map bindings ----------

(let ((m (current-global-map)))
  (define-key m [t] 'self-insert-command)
  (define-key m (kbd "C-@") 'set-mark-command)
  (define-key m (kbd "C-a") 'move-beginning-of-line)
  (define-key m (kbd "C-b") 'backward-char)
  (define-key m (kbd "C-d") 'delete-char)
  (define-key m (kbd "C-e") 'move-end-of-line)
  (define-key m (kbd "C-f") 'forward-char)
  (define-key m (kbd "C-g") 'keyboard-quit)
  (define-key m (kbd "C-j") 'newline-and-indent)
  (define-key m (kbd "C-k") 'kill-line)
  (define-key m (kbd "C-l") 'recenter-top-bottom)
  (define-key m (kbd "RET") 'newline)
  (define-key m (kbd "C-n") 'next-line)
  (define-key m (kbd "C-o") 'open-line)
  (define-key m (kbd "C-p") 'previous-line)
  (define-key m (kbd "C-q") 'quoted-insert)
  (define-key m (kbd "C-r") 'isearch-backward)
  (define-key m (kbd "C-s") 'isearch-forward)
  (define-key m (kbd "C-M-s") 'isearch-forward-regexp)
  (define-key m (kbd "C-M-r") 'isearch-backward-regexp)
  (define-key m (kbd "C-t") 'transpose-chars)
  (define-key m (kbd "C-u") 'universal-argument)
  (define-key m (kbd "C-v") 'scroll-up-command)
  (define-key m (kbd "C-w") 'kill-region)
  (define-key m (kbd "C-y") 'yank)
  (define-key m (kbd "C-z") 'suspend-emacs)
  (define-key m (kbd "TAB") 'indent-for-tab-command)
  (define-key m (kbd "DEL") 'delete-backward-char)
  (define-key m [down] 'next-line)
  (define-key m [up] 'previous-line)
  (define-key m [left] 'left-char)
  (define-key m [right] 'right-char)
  (define-key m [prior] 'scroll-down-command)
  (define-key m [next] 'scroll-up-command)
  (define-key m [home] 'beginning-of-buffer)
  (define-key m [end] 'end-of-buffer)
  (define-key m [deletechar] 'delete-forward-char)
  (define-key m [insert] 'overwrite-mode)
  (define-key m [insertchar] 'overwrite-mode)
  (define-key m [mouse-1] 'mouse-set-point)
  (define-key m [drag-mouse-1] 'mouse-set-region)
  (define-key m [double-mouse-1] 'mouse-set-region)
  (define-key m [triple-mouse-1] 'mouse-set-region)
  (define-key m [wheel-up] 'mwheel-scroll)
  (define-key m [wheel-down] 'mwheel-scroll)
  (define-key m [f1] 'help-command)
  (define-key m [f10] 'menu-bar-open)
  (define-key m (kbd "C-_") 'undo)
  (define-key m (kbd "C-/") 'undo)
  ;; ESC-prefix map (ESC x = M-x).
  (let ((esc (make-sparse-keymap)))
    (define-key m (kbd "ESC") esc)
    (define-key esc "x" 'execute-extended-command)
    (define-key esc "f" 'forward-word)
    (define-key esc "b" 'backward-word)
    (define-key esc "<" 'beginning-of-buffer)
    (define-key esc ">" 'end-of-buffer)
    (define-key esc "v" 'scroll-down-command)
    (define-key esc "%" 'query-replace)
    (define-key esc "w" 'kill-ring-save)
    (define-key esc "y" 'yank-pop)
    (define-key esc "d" 'kill-word)
    (define-key esc "c" 'capitalize-word)
    (define-key esc "l" 'downcase-word)
    (define-key esc "u" 'upcase-word)
    (define-key esc "-" 'negative-argument)
    (define-key esc "t" 'transpose-words)
    (define-key esc "\\" 'delete-indentation)
    (define-key esc " " 'just-one-space)
    (define-key esc "m" 'back-to-indentation)
    (define-key esc "z" 'zap-to-char)
    (define-key esc "q" 'fill-paragraph)
    (define-key esc ":" 'eval-expression)
    (define-key esc "^" 'delete-indentation)
    (define-key esc "{" 'backward-paragraph)
    (define-key esc "}" 'forward-paragraph)
    (define-key esc "a" 'backward-sentence)
    (define-key esc "e" 'forward-sentence)
    (define-key esc "h" 'mark-paragraph)
    (define-key esc "k" 'kill-sexp)
    (define-key esc "s" 'center-line)
    (define-key esc "r" 'move-to-window-line)
    (define-key esc "g" 'facemenu-keymap)
    (define-key esc "0" 'digit-argument)
    (define-key esc "1" 'digit-argument)
    (define-key esc "2" 'digit-argument)
    (define-key esc "3" 'digit-argument)
    (define-key esc "4" 'digit-argument)
    (define-key esc "5" 'digit-argument)
    (define-key esc "6" 'digit-argument)
    (define-key esc "7" 'digit-argument)
    (define-key esc "8" 'digit-argument)
    (define-key esc "9" 'digit-argument)
    (define-key esc (kbd "C-f") 'forward-sexp)
    (define-key esc (kbd "C-b") 'backward-sexp)
    (define-key esc (kbd "C-k") 'kill-sexp)
    (define-key esc (kbd "C-@") 'mark-sexp)
    (define-key esc (kbd "C-y") 'yank-pop)
    (define-key esc (kbd "DEL") 'backward-kill-word)
    (define-key esc (kbd "C-]") 'abort-recursive-edit))
  ;; C-x prefix map.
  (let ((cx (make-sparse-keymap)))
    (define-key m (kbd "C-x") cx)
    (define-key cx "b" 'switch-to-buffer)
    (define-key cx "k" 'kill-buffer)
    (define-key cx "o" 'other-window)
    (define-key cx "0" 'delete-window)
    (define-key cx "1" 'delete-other-windows)
    (define-key cx "2" 'split-window-below)
    (define-key cx "3" 'split-window-right)
    (define-key cx "h" 'mark-whole-buffer)
    (define-key cx "u" 'undo)
    (define-key cx "i" 'insert-file)
    (define-key cx "e" 'call-last-kbd-macro)
    (define-key cx "(" 'kmacro-start-macro)
    (define-key cx ")" 'kmacro-end-macro)
    (define-key cx "[" 'backward-page)
    (define-key cx "]" 'forward-page)
    (define-key cx (kbd "C-p") 'mark-page)
    (define-key cx (kbd "C-x") 'exchange-point-and-mark)
    (define-key cx (kbd "C-b") 'list-buffers)
    (define-key cx (kbd "C-c") 'save-buffers-kill-emacs)
    (define-key cx (kbd "C-e") 'eval-last-sexp)
    (define-key cx (kbd "C-f") 'find-file)
    (define-key cx (kbd "C-s") 'save-buffer)
    (define-key cx (kbd "C-w") 'write-file)
    (define-key cx (kbd "C-l") 'downcase-region)
    (define-key cx (kbd "C-u") 'upcase-region)
    (define-key cx (kbd "C-q") 'read-only-mode)
    (define-key cx (kbd "=") 'what-cursor-position)
    (define-key cx (kbd "ESC") 'keyboard-escape-quit)
    (define-key cx "z" 'repeat)
    (define-key cx (kbd "C-t") 'transpose-lines)
    (define-key cx (kbd "C-v") 'find-alternate-file)
    (define-key cx (kbd "C-n") 'set-goal-column)
    (define-key cx (kbd "DEL") 'backward-kill-sentence)
    ;; C-x n — narrowing prefix.
    (let ((n (make-sparse-keymap)))
      (define-key cx "n" n)
      (define-key n "n" 'narrow-to-region)
      (define-key n "w" 'widen)
      (define-key n "p" 'narrow-to-page)))
  ;; C-h help map.
  (let ((h (make-sparse-keymap)))
    (define-key m (kbd "C-h") h)
    (define-key h "f" 'describe-function)
    (define-key h "v" 'describe-variable)
    (define-key h "k" 'describe-key)
    (define-key h "b" 'describe-bindings)
    (define-key h "m" 'describe-mode)
    (define-key h "a" 'apropos-command))
  ;; M-x and other M- bindings in the global map proper.
  (define-key m (kbd "M-x") 'execute-extended-command)
  (define-key m (kbd "M-f") 'forward-word)
  (define-key m (kbd "M-b") 'backward-word)
  (define-key m (kbd "M-<") 'beginning-of-buffer)
  (define-key m (kbd "M->") 'end-of-buffer)
  (define-key m (kbd "M-v") 'scroll-down-command)
  (define-key m (kbd "M-%") 'query-replace)
  (define-key m (kbd "M-w") 'kill-ring-save)
  (define-key m (kbd "M-y") 'yank-pop)
  (define-key m (kbd "M-d") 'kill-word)
  (define-key m (kbd "M-DEL") 'backward-kill-word)
  (define-key m (kbd "M-c") 'capitalize-word)
  (define-key m (kbd "M-l") 'downcase-word)
  (define-key m (kbd "M-u") 'upcase-word)
  (define-key m (kbd "M-t") 'transpose-words)
  (define-key m (kbd "M-\\") 'delete-indentation)
  (define-key m (kbd "M-SPC") 'just-one-space)
  (define-key m (kbd "M-m") 'back-to-indentation)
  (define-key m (kbd "M-z") 'zap-to-char)
  (define-key m (kbd "M-q") 'fill-paragraph)
  (define-key m (kbd "M-:") 'eval-expression)
  (define-key m (kbd "M-a") 'backward-sentence)
  (define-key m (kbd "M-e") 'forward-sentence)
  (define-key m (kbd "M-h") 'mark-paragraph)
  (define-key m (kbd "M-k") 'kill-sentence)
  (define-key m (kbd "M-{") 'backward-paragraph)
  (define-key m (kbd "M-}") 'forward-paragraph)
  (define-key m (kbd "M-r") 'move-to-window-line)
  (define-key m (kbd "M--") 'negative-argument)
  (define-key m (kbd "M-0") 'digit-argument)
  (define-key m (kbd "M-1") 'digit-argument)
  (define-key m (kbd "M-2") 'digit-argument)
  (define-key m (kbd "M-3") 'digit-argument)
  (define-key m (kbd "M-4") 'digit-argument)
  (define-key m (kbd "M-5") 'digit-argument)
  (define-key m (kbd "M-6") 'digit-argument)
  (define-key m (kbd "M-7") 'digit-argument)
  (define-key m (kbd "M-8") 'digit-argument)
  (define-key m (kbd "M-9") 'digit-argument)
  (define-key m (kbd "C-M-f") 'forward-sexp)
  (define-key m (kbd "C-M-b") 'backward-sexp)
  (define-key m (kbd "C-M-k") 'kill-sexp)
  (define-key m (kbd "C-M-@") 'mark-sexp)
  (define-key m (kbd "C-M-a") 'beginning-of-defun)
  (define-key m (kbd "C-M-e") 'end-of-defun)
  (define-key m (kbd "C-M-h") 'mark-defun)
  (define-key m (kbd "C-M-t") 'transpose-sexps)
  (define-key m (kbd "C-M-u") 'backward-up-list)
  (define-key m (kbd "C-M-d") 'down-list)
  (define-key m (kbd "C-M-n") 'forward-list)
  (define-key m (kbd "C-M-p") 'backward-list)
  (define-key m (kbd "C-M-v") 'scroll-other-window)
  (define-key m (kbd "C-M-w") 'append-next-kill)
  (define-key m (kbd "M-=") 'count-words-region)
  (define-key m (kbd "M-^") 'delete-indentation)
  ;; M-g goto map.
  (let ((g (make-sparse-keymap)))
    (define-key m (kbd "M-g") g)
    (define-key g "g" 'goto-line)
    (define-key g (kbd "M-g") 'goto-line))
  ;; M-s search map.
  (let ((s (make-sparse-keymap)))
    (define-key m (kbd "M-s") s)
    (define-key s "o" 'occur))
  (define-key m (kbd "ESC ESC ESC") 'keyboard-escape-quit))

;; ---------- interactive commands ----------

(defun repeat (&optional arg)
  "Re-execute the last command, like Emacs's C-x z."
  (interactive "P")
  (let ((cmd last-command))
    (when (and cmd (not (eq cmd 'repeat)))
      (command-execute cmd))))

(defun find-alternate-file (filename)
  "Visit FILENAME, replacing the current buffer's contents (C-x C-v)."
  (interactive "fFind alternate file: ")
  (kill-buffer (current-buffer))
  (find-file filename))

(defun occur (regexp &optional nlines)
  "Show all lines in the current buffer matching REGEXP in *Occur*."
  (interactive "sList lines matching: \nP")
  (let ((src (current-buffer))
        (hits '()))
    (save-excursion
      (goto-char (point-min))
      (let ((ln 1))
        (while (not (eobp))
          (let ((line (buffer-substring (line-beginning-position)
                                        (line-end-position))))
            (when (string-match regexp line)
              (push (format "%7d:%s" ln line) hits)))
          (forward-line 1)
          (setq ln (1+ ln)))))
    (let ((ob (get-buffer-create "*Occur*"))
          (n (length hits)))
      (with-current-buffer ob
        (erase-buffer)
        (insert (format "%d %s for \"%s\" in buffer: %s\n"
                        n (if (= n 1) "match" "matches") regexp
                        (buffer-name src)))
        (dolist (l (nreverse hits))
          (insert l "\n")))
      (message "Searched 1 buffer; %d %s for \"%s\""
               n (if (= n 1) "match" "matches") regexp)
      (display-buffer ob))))

(defun display-buffer (buffer &optional action)
  "Make BUFFER visible in a window without selecting it."
  (let ((w (or (get-buffer-window buffer) (selected-window))))
    (set-window-buffer w buffer))
  buffer)

;; ---------- subr.el-level utilities ----------
(defalias 'cl-subseq #'seq-subseq)

(defun add-to-list (list-var element &optional append compare-fn)
  "Add ELEMENT to the list value of LIST-VAR if not already present."
  (let ((lst (symbol-value list-var)))
    (if (if compare-fn
            (let ((found nil) (rest lst))
              (while (and rest (not found))
                (if (funcall compare-fn element (car rest))
                    (setq found t))
                (setq rest (cdr rest)))
              found)
          (member element lst))
        lst
      (set list-var
           (if append (append lst (list element)) (cons element lst))))))

(defmacro bound-and-true-p (var)
  "Return the value of symbol VAR if bound and non-nil."
  (list 'and (list 'boundp (list 'quote var)) var))

(defmacro save-match-data (&rest body)
  "Execute BODY, restoring the match data afterwards."
  (list 'let (list (list 'match-data '(match-data)))
        (list 'unwind-protect (cons 'progn body)
              '(set-match-data match-data))))

(defmacro with-local-quit (&rest body)
  "Execute BODY with quits allowed."
  (cons 'let (cons '((inhibit-quit nil)) body)))

(defvar signal-hook-function nil
  "If non-nil, `signal' calls this function (with the same arguments)
before doing anything else.")

(defvar inhibit-variable-watchers nil
  "If non-nil, variable watchers are not called.")

(defmacro handler-bind (handlers &rest body)
  "Execute BODY with condition handlers bound.
Each element of HANDLERS is (CONDITIONS HANDLER) where CONDITIONS is a
condition name or list of names; HANDLER is called with the condition
object (CONDITION-NAME . DATA) when a matching signal is raised
uncaught (at debugger-entry time, in the raising dynamic context)."
  (cons 'handler-bind-1
        (cons (cons 'lambda (cons nil body))
              (apply #'append
                     (mapcar (lambda (b)
                               (list (list 'quote
                                           (let ((c (car b)))
                                             (if (consp c) c (list c))))
                                     (car (cdr b))))
                             handlers)))))

(defmacro condition-case-unless-debug (var bodyform &rest handlers)
  "Like `condition-case' (we have no debugger, so equivalent here)."
  (cons 'condition-case
        (cons var (cons bodyform handlers))))

;; GNU implements these three as Lisp macros (their `symbol-function'
;; is `(macro . ...)', not a subr); eval still dispatches them via the
;; special-form table, so the cells exist for `macroexpand'/introspection.
(defmacro save-mark-and-excursion (&rest body)
  "Like `save-excursion', but also save and restore the mark state."
  (list 'let (list (list 'saved-marker '(save-mark-and-excursion--save)))
        (list 'unwind-protect (cons 'save-excursion body)
              '(save-mark-and-excursion--restore saved-marker))))

(defmacro track-mouse (&rest body)
  "Evaluate BODY with mouse movement events enabled."
  (list 'internal--track-mouse (cons 'lambda (cons nil body))))

;; ---------- backquote (port of GNU emacs-lisp/backquote.el) ----------
;; `backquote-process' returns (TAG . FORM): TAG 0 => constant, 1 =>
;; evaluates to the structure, 2 => produces a list to be spliced in.

(defvar backquote-backquote-symbol (intern "`"))
(defvar backquote-unquote-symbol (intern ","))
(defvar backquote-splice-symbol (intern ",@"))

(defun backquote-list*-function (first &rest list)
  "Like `list' but the last argument is the tail of the new list."
  (if list (cons first (apply #'backquote-list*-function list)) first))

;; GNU defines this via `backquote-list*-macro': one expansion step
;; folds the whole spine into a cons chain (list* semantics).
(defmacro backquote-list* (first &rest list)
  "Like `list' but the last argument is the tail of the new list."
  (let ((r (car (last (cons first list)))))
    (dolist (x (cdr (nreverse (cons first list))))
      (setq r (list 'cons x r)))
    r))

(defun backquote-delay-process (s level)
  "Process a (un|back|splice)quote inside a backquote.
This simply recurses through the body."
  (let ((exp (backquote-listify (list (cons 0 (list 'quote (car s))))
                                (backquote-process (cdr s) level))))
    (cons (if (eq (car-safe exp) 'quote) 0 1) exp)))

(defun backquote-process (s &optional level)
  "Process the body of a backquote.
S is the body.  Returns a cons cell whose cdr is piece of code which
is the macro-expansion of S, and whose car is a small integer whose value
can either indicate that the code is constant (0), or not (1), or returns
a list which should be spliced into its environment (2).
LEVEL is only used internally and indicates the nesting level:
0 (the default) is for the toplevel nested inside a single backquote."
  (unless level (setq level 0))
  (cond
   ((vectorp s)
    (let ((n (backquote-process (append s nil) level)))
      (if (= (car n) 0)
	  (cons 0 s)
	(cons 1 (cond
		 ((not (listp (cdr n)))
		  (list 'vconcat (cdr n)))
		 ((eq (nth 1 n) 'list)
		  (cons 'vector (nthcdr 2 n)))
		 ((eq (nth 1 n) 'append)
		  (cons 'vconcat (nthcdr 2 n)))
		 (t
		  (list 'apply '(function vector) (cdr n))))))))
   ((atom s)
    (cons 0 (if (or (null s) (eq s t) (not (symbolp s)))
		s
	      (list 'quote s))))
   ((eq (car s) backquote-unquote-symbol)
    (if (<= level 0)
        (cond
         ((> (length s) 2)
          (error "Multiple args to , are not supported: %S" s))
         (t (cons (if (eq (car-safe (nth 1 s)) 'quote) 0 1)
                  (nth 1 s))))
      (backquote-delay-process s (1- level))))
   ((eq (car s) backquote-splice-symbol)
    (if (<= level 0)
        (if (> (length s) 2)
            (error "Multiple args to ,@ are not supported: %S" s)
          (cons 2 (nth 1 s)))
      (backquote-delay-process s (1- level))))
   ((eq (car s) backquote-backquote-symbol)
      (backquote-delay-process s (1+ level)))
   (t
    (let ((rest s)
	  item firstlist list lists expression)
      ;; Scan this list-level, setting LISTS to a list of forms,
      ;; each of which produces a list of elements
      ;; that should go in this level.
      ;; The order of LISTS is backwards.
      ;; If there are non-splicing elements (constant or variable)
      ;; at the beginning, put them in FIRSTLIST,
      ;; as a list of tagged values (TAG . FORM).
      ;; If there are any at the end, they go in LIST, likewise.
      (while (and (consp rest)
                  ;; Stop if the cdr is an expression inside a backquote or
                  ;; unquote since this needs to go recursively through
                  ;; backquote-process.
                  (not (or (eq (car rest) backquote-unquote-symbol)
                           (eq (car rest) backquote-backquote-symbol))))
	(setq item (backquote-process (car rest) level))
	(cond
	 ((= (car item) 2)
	  ;; Put the nonspliced items before the first spliced item
	  ;; into FIRSTLIST.
	  (if (null lists)
	      (setq firstlist list
		    list nil))
	  ;; Otherwise, put any preceding nonspliced items into LISTS.
	  (if list
	      (push (backquote-listify list '(0 . nil)) lists))
	  (push (cdr item) lists)
	  (setq list nil))
	 (t
	  (setq list (cons item list))))
	(setq rest (cdr rest)))
      ;; Handle nonsplicing final elements, and the tail of the list
      ;; (which remains in REST).
      (if (or rest list)
	  (push (backquote-listify list (backquote-process rest level))
                lists))
      ;; Turn LISTS into a form that produces the combined list.
      (setq expression
	    (if (or (cdr lists)
		    (eq (car-safe (car lists)) backquote-splice-symbol))
		(cons 'append (nreverse lists))
	      (car lists)))
      ;; Tack on any initial elements.
      (if firstlist
	  (setq expression (backquote-listify firstlist (cons 1 expression))))
      (cons (if (eq (car-safe expression) 'quote) 0 1) expression)))))

;; backquote-listify takes (tag . structure) pairs from backquote-process
;; and decides between append, list, backquote-list*, and cons depending
;; on which tags are in the list.

(defun backquote-listify (list old-tail)
  (let ((heads nil) (tail (cdr old-tail)) (list-tail list) (item nil))
    (if (= (car old-tail) 0)
	(setq tail (eval tail)
	      old-tail nil))
    (while (consp list-tail)
      (setq item (car list-tail))
      (setq list-tail (cdr list-tail))
      (if (or heads old-tail (/= (car item) 0))
	  (setq heads (cons (cdr item) heads))
	(setq tail (cons (eval (cdr item)) tail))))
    (cond
     (tail
      (if (null old-tail)
	  (setq tail (list 'quote tail)))
      (if heads
	  (let ((use-list* (or (cdr heads)
			       (and (consp (car heads))
				    (eq (car (car heads))
					backquote-splice-symbol)))))
	    (cons (if use-list* 'backquote-list* 'cons)
		  (append heads (list tail))))
	tail))
     (t (cons 'list heads)))))

(defmacro backquote (structure)
  "Argument STRUCTURE describes a template to build.
The whole structure acts as if it were quoted except for certain
places where expressions are evaluated and inserted or spliced in."
  (cdr (backquote-process structure)))

;; The reader produces (` STRUCTURE) — GNU binds ` to the same macro.
(fset (intern "`") (symbol-function 'backquote))



(defvar minor-mode-alist nil
  "Alist of (MODE . LIGHTER-STRINGS) for minor modes.")

;; GNU registers these as autoload cells; calling them loads the
;; library from lisp/ (see load-path handling in load.rs).
(autoload 'define-minor-mode "easy-mmode"
  "Define a new minor mode MODE." nil t)
;; loaddefs.el registers this obsolete alias eagerly.
(defalias 'easy-mmode-define-minor-mode 'define-minor-mode)
(autoload 'kbd-macro-query "macros"
  "Query user during kbd macro execution." t)
(autoload 'insert-kbd-macro "macros"
  "Insert in buffer the Lisp definition of kbd macro MACRONAME." t)
(autoload 'kmacro-start-macro "kmacro"
  "Record subsequent keyboard input, defining a keyboard macro." t)
(autoload 'kmacro-end-macro "kmacro"
  "Finish defining a keyboard macro." t)
(autoload 'kmacro-start-macro-or-insert-counter "kmacro" nil t)
(autoload 'kmacro-end-or-call-macro "kmacro" nil t)
(autoload 'kmacro-end-or-call-macro-repeat "kmacro" nil t)
(autoload 'kmacro-name-last-macro "kmacro"
  "Assign a name to the last keyboard macro defined." t)
(autoload 'gv-get "gv"
  "Build the code that applies DO to PLACE." nil t)
(autoload 'gv-letplace "gv"
  "Build the code manipulating the generalized variable PLACE." nil t)
(autoload 'gv-define-expander "gv"
  "Attach HANDLER as the gv-expander of NAME." nil t)
(autoload 'gv-define-setter "gv"
  "Define a setter method for generalized variable NAME." nil t)
(autoload 'gv-define-simple-setter "gv"
  "Define a simple setter method for generalized variable NAME." nil t)
(autoload 'defclass "eieio"
  "Define NAME as a class." nil t)
(autoload 'make-instance "eieio"
  "Create an instance of CLASS." nil nil)
(autoload 'gv-ref "gv"
  "Return a reference to PLACE." nil t)

;; GNU aliases (resolve immediately, before the library loads).
(defalias 'kmacro-exec-ring-item 'funcall)
(defalias 'name-last-kbd-macro 'kmacro-name-last-macro)

;; ---------- nadvice place forms ----------

;; `add-function'/`remove-function' are GNU macros over generalized
;; places; PLACE is normalized to code evaluating to (KIND . ARGS)
;; and `cl--add-function'/`cl--remove-function' do the wrap/dispatch.
(defun cl--advice-place-code (place)
  (cond
   ((symbolp place) (list 'list ''var (list 'quote place)))
   ((eq (car-safe place) 'local) (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'var) (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'function) (list 'list ''function (nth 1 place)))
   ((eq (car-safe place) 'symbol-function)
    (list 'list ''function (nth 1 place)))
   ((eq (car-safe place) 'default-value)
    (list 'list ''var (nth 1 place)))
   ((eq (car-safe place) 'get)
    (list 'list ''get (nth 1 place) (nth 2 place)))
   ;; GNU: a quoted place reaches a `(setf quote)' setter and fails.
   ((eq (car-safe place) 'quote) '(quote (setf-quote)))
   (t (list 'list ''bad (list 'quote place)))))

(defmacro add-function (how place function &optional props)
  "Add FUNCTION to the function stored in the generalized PLACE.
HOW is one of the `advice-add' locations; PROPS is an alist that may
contain `name' and `depth'."
  (list 'cl--add-function how (cl--advice-place-code place)
        function props))

(defmacro remove-function (place function)
  "Remove FUNCTION (or the named advice) from the function in PLACE."
  (list 'cl--remove-function (cl--advice-place-code place) function))

(defmacro define-advice (symbol args &rest body)
  "Define an advice and add it to the function named SYMBOL."
  (or (listp args) (signal 'wrong-type-argument (list 'listp args)))
  (or (<= 2 (length args) 4)
      (signal 'wrong-number-of-arguments (list 2 4 (length args))))
  (let* ((how (nth 0 args))
         (lambda-list (nth 1 args))
         (name (nth 2 args))
         (depth (nth 3 args))
         (props (append (and depth (list (cons 'depth depth)))
                        (and name (list (cons 'name name)))))
         (advice (cond ((null name) (cons 'lambda (cons lambda-list body)))
                       ((or (stringp name) (symbolp name))
                        (intern (format "%s@%s" symbol name)))
                       (t (error "Unrecognized name spec `%S'" name)))))
    (append '(prog1)
            (and (symbolp advice)
                 (list (cons 'defun (cons advice (cons lambda-list body)))))
            (list (list 'advice-add (list 'quote symbol) how
                        (list 'function advice)
                        (and props (list 'quote props)))))))

(defun advice-mapc (fun symbol)
  "Apply FUN to each advice added to SYMBOL.
FUN is called with the advice function and its property alist."
  (advice-function-mapc fun symbol))

;; ---------- mode keymaps ----------




;;; -*- Compatibility support for fill.el / newcomment.el ports -*-

(defmacro defcustom (var value &optional doc &rest _keys)
  (list 'defvar var value doc))

(defmacro defvar-local (var value &optional docstring)
  (list 'progn
        (list 'defvar var value docstring)
        (list 'make-variable-buffer-local (list 'quote var))))

(defmacro defsubst (name arglist &rest body)
  (cons 'defun (cons name (cons arglist body))))

;; ---------- subr.el / subr-x.el cluster (GNU-dumped) ----------

(defsubst xor (cond1 cond2)
  "Return the non-nil argument if exactly one of COND1, COND2 is non-nil."
  (if cond1 (unless cond2 cond1) cond2))

(defmacro if-let* (varlist then &rest else)
  "Bind each VAR in VARLIST to VAL; eval THEN when all non-nil, else ELSE.
Each binding spec is (VAR VAL), (VAR) (binds nil), or a bare VAR
\(tests VAR's current value)."
  (if (null varlist)
      `(progn ,then)
    (let ((spec (car varlist)))
      (cond
       ((symbolp spec) (setq spec (list spec spec)))
       ((null (cdr spec)) (setq spec (list (car spec) nil))))
      `(let* (,spec)
         (if ,(car spec)
             (if-let* ,(cdr varlist) ,then ,@else)
           ,@(if else `((progn ,@else)) '(nil)))))))

(defmacro when-let* (varlist &rest body)
  "Bind each VAR in VARLIST; when all non-nil, eval BODY."
  `(if-let* ,varlist (progn ,@body)))

;; The non-* variants take a single (VAR VAL) spec or legacy [VAR VAL].
(defmacro if-let (varlist then &rest else)
  "Like `if-let*' for a single binding (obsolete shape)."
  (let ((spec (if (and (consp varlist) (cdr varlist)
                       (symbolp (car varlist)) (not (consp (car varlist))))
                  (list varlist)          ; legacy (var val)
                varlist)))
    `(if-let* ,spec ,then ,@else)))

(defmacro when-let (varlist &rest body)
  "Like `when-let*' for a single binding (obsolete shape)."
  `(if-let ,varlist (progn ,@body)))

(defmacro and-let* (varlist &rest body)
  "Bind (VAR VAL) specs in sequence; eval BODY when all yield non-nil.
A (FORM) spec evaluates FORM for truth without binding; a bare
VAR spec tests VAR's current value."
  (if (null varlist)
      `(progn ,@body)
    (let ((spec (car varlist))
          (rest (cdr varlist)))
      (cond
       ((and (consp spec) (symbolp (car spec)) (cdr spec))
        `(let* ((,(car spec) (and t ,(cadr spec))))
           (and ,(car spec) (and-let* ,rest ,@body))))
       ((consp spec)
        (let ((tmp (gensym)))
          `(let* ((,tmp (and t ,(car spec))))
             (and ,tmp (and-let* ,rest ,@body)))))
       (t
        `(and ,spec (and-let* ,rest ,@body)))))))

(defun cl--thread-expand (x forms last)
  (if (null forms)
      x
    (let ((f (car forms)))
      (cl--thread-expand
       (if (consp f)
           (if last
               `(,(car f) ,@(cdr f) ,x)
             `(,(car f) ,x ,@(cdr f)))
         (list f x))
       (cdr forms) last))))

(defmacro thread-first (&rest forms)
  "Thread X through FORMS as the first argument."
  (cl--thread-expand (car forms) (cdr forms) nil))

(defmacro thread-last (&rest forms)
  "Thread X through FORMS as the last argument."
  (cl--thread-expand (car forms) (cdr forms) t))

(defmacro dlet (spec &rest body)
  "Like `let*' with dynamic binding (ours is already dynamic)."
  `(let* ,spec ,@body))

(defun define-error (name message &optional parent)
  "Define NAME as an error with MESSAGE inheriting from PARENT."
  (let* ((parent (or parent 'error))
         (conds (cons name (or (get parent 'error-conditions)
                               (list 'error)))))
    (put name 'error-conditions conds)
    (put name 'error-message message)))

(defvar after-load-alist nil
  "Alist of (FILE . FORMS) to eval after FILE is loaded.")

(defun eval-after-load (file form)
  "Arrange that FORM is evaluated after FILE is loaded.
String FILEs become a regexp matching the file name at the end of
any path, with optional .so/.dylib/.elc/.el/.gz extension."
  (let ((key (if (stringp file)
                 (concat "\\(\\`\\|/\\)" (regexp-quote file)
                         "\\(\\.so\\|\\.dylib\\|\\.elc\\|\\.el\\)?\\(\\.gz\\)?\\'")
               file)))
    (setq after-load-alist
          (cons (list key `(lambda () ,form)) after-load-alist)))
  nil)

(defmacro with-eval-after-load (file &rest body)
  "Arrange that BODY runs after FILE is loaded."
  `(eval-after-load ,file (function (lambda () ,@body))))

(defun load-library (library)
  "Load the Emacs Lisp library named LIBRARY."
  (interactive "sLoad library: ")
  (load library))

(defun delete-consecutive-dups (list &optional circular)
  "Destructively remove consecutive `equal' duplicates from LIST."
  (when (and circular (consp list) (consp (cdr list))
             (equal (car list) (car (last list))))
    (setcdr (nthcdr (- (length list) 2) list) nil))
  (let ((tail list))
    (while (cdr-safe tail)
      (if (equal (car tail) (cadr tail))
          (setcdr tail (cddr tail))
        (setq tail (cdr tail)))))
  list)

(defun assoc-delete-all (key alist &optional testfn)
  "Delete from ALIST all elements whose car matches KEY via TESTFN."
  (let ((test (or testfn #'equal)))
    (while (and alist (funcall test key (caar alist)))
      (setq alist (cdr alist)))
    (let ((tail alist))
      (while (cdr tail)
        (if (funcall test key (car (cadr tail)))
            (setcdr tail (cddr tail))
          (setq tail (cdr tail)))))
    alist))

(defun string-limit (string &optional length coding-system)
  "Return STRING truncated to LENGTH characters."
  (let ((n (length string)))
    (if (or (null length) (>= length n))
        string
      (substring string 0 (max 0 length)))))

(defun string-clean-whitespace (string)
  "Collapse whitespace runs in STRING to single spaces; trim ends."
  (string-trim
   (if (fboundp 'replace-regexp-in-string)
       (replace-regexp-in-string "[\\s-]+" " " string)
     (string-replace "\n" " " string))))

(defun forward-thing (thing &optional n)
  "Move point forward N THINGs."
  (or n (setq n 1))
  (cond
   ((memq thing '(symbol)) (forward-symbol n))
   ((memq thing '(word)) (forward-word n))
   ((memq thing '(sexp)) (forward-sexp n))
   ((memq thing '(list)) (forward-list n))
   ((memq thing '(line)) (forward-line n))
   ((memq thing '(sentence)) (forward-sentence n))
   ((memq thing '(paragraph)) (forward-paragraph n))
   ((memq thing '(defun)) (beginning-of-defun (- n)))
   ((memq thing '(char)) (forward-char n))
   (t (error "Unknown thing: %s" thing))))

(defun locate-file (filename path &optional suffixes predicate)
  "Search PATH (a directory or list of directories) for FILENAME,
trying SUFFIXES; PREDICATE (default `file-exists-p') must pass."
  (let ((dirs (if (listp path) path (list path)))
        (sufs (or suffixes '("")))
        (pred (or predicate #'file-exists-p))
        (found nil))
    (while (and dirs (not found))
      (let ((ss sufs))
        (while ss
          (let ((f (expand-file-name (concat filename (car ss)) (car dirs))))
            (when (funcall pred f)
              (setq found f ss nil)))
          (setq ss (cdr ss))))
      (setq dirs (cdr dirs)))
    found))

(defun file-name-parent-directory (file)
  "Return the parent directory of directory FILE, or nil."
  (let ((dir (directory-file-name file)))
    (file-name-directory dir)))

(defun file-name-with-extension (file extension)
  "Return FILE with its extension changed to EXTENSION."
  (concat (file-name-sans-extension file)
          (if (string-prefix-p "." extension)
              extension
            (concat "." extension))))

(defun keymap-set (keymap key def)
  "In KEYMAP, bind KEY (a `kbd' string or vector) to DEF."
  (define-key keymap (if (stringp key) (kbd key) key) def))

(defun keymap-global-set (key def)
  "Bind KEY globally to DEF."
  (global-set-key (if (stringp key) (kbd key) key) def))

(defun keymap-local-set (key def)
  "Bind KEY in the current buffer's local map to DEF."
  (local-set-key (if (stringp key) (kbd key) key) def))

(defun keymap-unset (keymap key &optional remove)
  "Remove KEY's binding from KEYMAP."
  (define-key keymap (if (stringp key) (kbd key) key)
              (if remove 'remove nil)))

(defmacro define-keymap (&rest pairs)
  "Define a new keymap; PAIRS is :option vals then alternating keys/defs."
  (let ((parent nil) (name nil) (full nil) (dense nil)
        (defs '()) (suppress nil))
    (while pairs
      (let ((p (car pairs)))
        (if (keywordp p)
            (progn
              (setq pairs (cdr pairs))
              (cond
               ((eq p :parent) (setq parent (car pairs) pairs (cdr pairs)))
               ((eq p :name) (setq name (car pairs) pairs (cdr pairs)))
               ((eq p :doc) (setq pairs (cdr pairs)))
               ((eq p :full) (setq full (car pairs) pairs (cdr pairs)))
               ((eq p :dense) (setq dense (car pairs) pairs (cdr pairs)))
               ((eq p :suppress) (setq suppress (car pairs) pairs (cdr pairs)))
               (t (setq pairs (cdr pairs)))))
          (push (list p (cadr pairs)) defs)
          (setq pairs (cddr pairs)))))
    `(let ((m (,(if (or full (not dense)) 'make-keymap 'make-sparse-keymap))))
       ,@(when parent `((set-keymap-parent m ,parent)))
       ,@(mapcar (lambda (kv) `(keymap-set m ,(car kv) ,(cadr kv)))
                 (nreverse defs))
       m)))

(defmacro defvar-keymap (name &rest pairs)
  "Define NAME as a keymap variable."
  (let ((doc (when (and pairs (keywordp (car pairs)) (eq (car pairs) :doc))
               (prog1 (cadr pairs) (setq pairs (cddr pairs))))))
    `(progn
       (defvar ,name nil ,doc)
       (setq ,name (define-keymap ,@pairs))
       ,name)))

;; ---------- minimal setf/generalized-place machinery ----------

(defun cl--setf-pair (place val)
  "Return a form evaluating to `(setf PLACE VAL)' for common places."
  (cond
   ((symbolp place) (list 'setq place val))
   ((consp place)
    (let ((op (car place)))
      (cond
       ;; Expand macro-headed places (GNU gv does this) so e.g.
       ;; `(oref o s)' exposes its `(setf eieio-oref)'-able form.
       ((and (symbolp op)
             (not (memq op '(car cdr caar cadr cddr nth elt nthcdr aref
                             get gethash symbol-value symbol-function
                             symbol-plist plist-get alist-get gv-deref)))
             (macrop (symbol-function op)))
        (cl--setf-pair (macroexpand-1 place) val))
       ((eq op 'car) `(setcar ,(cadr place) ,val))
       ((eq op 'cdr) `(setcdr ,(cadr place) ,val))
       ((eq op 'caar) `(setcar (car ,(cadr place)) ,val))
       ((eq op 'cadr) `(setcar (cdr ,(cadr place)) ,val))
       ((eq op 'cddr) `(setcdr (cdr ,(cadr place)) ,val))
       ((eq op 'nth) `(setcar (nthcdr ,(cadr place) ,(caddr place)) ,val))
       ((eq op 'elt) `(setcar (nthcdr ,(caddr place) ,(cadr place)) ,val))
       ((eq op 'nthcdr)
        `(setcdr (nthcdr ,(caddr place) ,(cadr place)) ,val))
       ((eq op 'aref) `(aset ,(cadr place) ,(caddr place) ,val))
       ((eq op 'get) `(put ,(cadr place) ,(caddr place) ,val))
       ((eq op 'gethash) `(puthash ,(caddr place) ,val ,(cadr place)))
       ((eq op 'symbol-value) `(set ,(cadr place) ,val))
       ((eq op 'symbol-function) `(fset ,(cadr place) ,val))
       ((eq op 'symbol-plist) `(setplist ,(cadr place) ,val))
       ((eq op 'plist-get)
        `(progn (setq ,(cadr place)
                      (plist-put ,(cadr place) ,(caddr place) ,val))
                ,(caddr place)))
       ((eq op 'alist-get)
        `(setf (cdr (assoc ,(caddr place) ,(cadr place))) ,val))
       ((eq op 'gv-deref) `(funcall (cdr ,(cadr place)) ,val))
       ;; Fallback like GNU's gv-setter: call the `(setf OP)' function.
       ((symbolp op)
        `(funcall ',(intern (format "(setf %s)" op)) ,val
                  ,@(cdr place)))
       (t (error "setf: unsupported place %s" place)))))
   (t (error "setf: unsupported place %s" place))))

(defmacro setf (&rest args)
  "Set each generalized PLACE to VALUE.  Supports symbol, car, cdr,
nth, elt, aref, get, gethash, plist-get, symbol-* places."
  ;; GNU autoloads `setf' itself from gv.el, so expanding a `setf' form
  ;; loads that file and defines gv-ref/gv-letplace/gv-get.  Mirror the
  ;; observable autoload timing.
  (when (autoloadp (symbol-function 'gv-get))
    (load "gv"))
  (cons 'progn
        (let ((out nil) (rest args))
          (while rest
            (push (cl--setf-pair (car rest) (cadr rest)) out)
            (setq rest (cddr rest)))
          (nreverse out))))

(defmacro psetf (&rest args)
  "Like `setf' but evaluate all values before assigning."
  ;; GNU autoloads `psetf' from gv.el — expanding it loads gv.
  (when (autoloadp (symbol-function 'gv-get))
    (load "gv"))
  (let ((temps nil) (sets nil) (rest args))
    (while rest
      (let ((tmp (gensym)))
        (push (list tmp (cadr rest)) temps)
        (push (cl--setf-pair (car rest) tmp) sets))
      (setq rest (cddr rest)))
    `(let ,(nreverse temps) ,@(nreverse sets))))

(defalias 'cl-psetf 'psetf)

(defmacro incf (place &optional delta)
  "Increment PLACE by DELTA (default 1)."
  `(setf ,place (+ ,place ,(or delta 1))))

(defmacro decf (place &optional delta)
  "Decrement PLACE by DELTA (default 1)."
  `(setf ,place (- ,place ,(or delta 1))))

(defalias 'cl-incf 'incf)
(defalias 'cl-decf 'decf)

(defmacro cl-pushnew (val place &rest keys)
  "Push VAL onto PLACE's list unless already `eql' to a member."
  `(let ((v ,val))
     (unless (apply #'cl-member v ,place (list ,@keys))
       (setf ,place (cons v ,place)))))

(defmacro cl-remf (place item)
  "Remove the first element `eql' to ITEM from the list in PLACE."
  `(setf ,place (cl--do-remf ,place ,item)))

(defun cl--do-remf (list item)
  (if (and (consp list) (eql (car list) item))
      (cdr list)
    (let ((tail list))
      (while (and (cdr tail) (not (eql (cadr tail) item)))
        (setq tail (cdr tail)))
      (when (cdr tail) (setcdr tail (cddr tail)))
      list)))

(defmacro cl-rotatef (&rest args)
  "Rotate the values of ARGS leftward."
  (let ((tmps (mapcar (lambda (_) (gensym)) args))
        (sets nil) (i 0) (n (length args)))
    (while (< i n)
      (push (cl--setf-pair (nth (mod (1+ i) n) args) (nth i tmps)) sets)
      (setq i (1+ i)))
    `(let ,(cl--zip tmps args) ,@(nreverse sets) nil)))

(defmacro cl-shiftf (&rest args)
  "Shift each ARG left: (cl-shiftf a b ... v) sets a←b ... returns old a."
  (let ((tmps (mapcar (lambda (_) (gensym)) args))
        (sets nil) (i 0) (n (length args)))
    (while (< (1+ i) n)
      (push (cl--setf-pair (nth i args) (nth (1+ i) tmps)) sets)
      (setq i (1+ i)))
    `(let ,(cl--zip tmps args) ,@(nreverse sets) ,(car tmps))))

;; ---------- simple.el / timer.el / subr.el additions ----------

(defun transient-mark-mode (&optional arg)
  "Toggle Transient Mark mode.
With positive numeric ARG, enable; with non-positive, disable;
with no ARG (or 'toggle), toggle."
  (interactive (list (or current-prefix-arg 'toggle)))
  (setq transient-mark-mode
        (cond ((eq arg 'toggle) (not transient-mark-mode))
              ((null arg) t)
              (t (> (prefix-numeric-value arg) 0))))
  nil)

(defun read-minibuffer (prompt &optional initial-contents)
  "Return a Lisp object read using the minibuffer, unevaluated."
  (read-from-minibuffer prompt initial-contents minibuffer-local-map
                        t 'minibuffer-history))

(defun eval-minibuffer (prompt &optional initial-contents)
  "Return value of Lisp expression read using the minibuffer."
  (eval (read-minibuffer prompt initial-contents) t))

;; GNU: obsolete alias of `run-with-timer' since 26.1.
(defalias 'add-timeout 'run-with-timer)

(defun values--store-value (value)
  "Store VALUE in the list `values'."
  (push value values))

(autoload 'pp "pp" "Output pretty-printed representation of OBJECT." t)
(autoload 'pp-to-string "pp"
  "Return a string containing the pretty-printed representation of OBJECT.")
(autoload 'pp-buffer "pp" "Prettify the current buffer." t)
(autoload 'pp-eval-expression "pp"
  "Evaluate EXPRESSION and pretty-print its value." t)
(autoload 'pp-eval-last-sexp "pp"
  "Run `pp-eval-expression' on sexp before point." t)
(autoload 'pp-macroexpand-expression "pp"
  "Macroexpand EXPRESSION and pretty-print its value." t)
(autoload 'pp-macroexpand-last-sexp "pp"
  "Run `pp-macroexpand-expression' on sexp before point." t)
(autoload 'pp-display-expression "pp"
  "Prettify and display EXPRESSION in an appropriate way.")

(defmacro with-output-to-temp-buffer (bufname &rest body)
  "Bind `standard-output' to buffer BUFNAME, then run BODY."
  `(let ((standard-output (get-buffer-create ,bufname)))
     (with-current-buffer standard-output
       (let ((inhibit-read-only t)) (erase-buffer))
       (run-hooks 'temp-buffer-setup-hook))
     (prog1 (progn ,@body)
       (with-current-buffer standard-output
         ;; GNU's temp-buffer display (help-mode setup) ensures the
         ;; contents end with a newline and makes the buffer read-only.
         (let ((inhibit-read-only t))
           (goto-char (point-max))
           (unless (or (bobp) (eq (char-before) ?\n))
             (insert "\n")))
         (setq buffer-read-only t))
       (ignore-errors (display-buffer standard-output)))))

;; ---------- cl-generic subset ----------

(define-error 'cl-no-applicable-method "No applicable method" 'error)
(define-error 'cl-no-next-method "No next method" 'error)

;; A generic function's methods live on its `cl--methods' plist entry:
;; a list of (SPECIALIZERS QUALIFIER . FUNCTION), where SPECIALIZERS is
;; a list of `t', a type symbol, or (eql FORM).

(defvar cl--cnm nil
  "Dynamically bound chain of remaining applicable methods.")
(defvar cl--cnm-args nil)

(defvar cl--cnm-name nil
  "Dynamically bound name of the generic function being dispatched.")

(defun cl-call-next-method (&rest args)
  "Call the next most specific applicable method."
  (unless cl--cnm-ok
    (error "cl-call-next-method only allowed inside primary and around methods"))
  (unless (consp cl--cnm)
    (signal 'cl-no-next-method (cons cl--cnm-name (or args cl--cnm-args))))
  (let ((m (car cl--cnm)))
    (setq cl--cnm (cdr cl--cnm))
    (apply (cl--method-fn m) (or args cl--cnm-args))))

(defvar cl--cnm-ok nil
  "Dynamically bound non-nil while executing a method body.")

(defun cl-next-method-p ()
  "Return non-nil if a next method is available."
  (unless cl--cnm-ok
    (error "cl-next-method-p only allowed inside primary and around methods"))
  (consp cl--cnm))

(defun cl--type-parents (type)
  "Ancestors of TYPE for method specificity ordering."
  (cdr (assq type '((integer number) (fixnum integer) (bignum integer)
                    (number t) (string t) (cons list) (list t)
                    (symbol t) (keyword symbol) (float number)
                    (vector sequence) (list sequence)
                    (sequence t) (function t) (buffer t)
                    (marker t) (window t) (frame t) (process t)
                    (hash-table t) (atom t) (null symbol)
                    (boolean symbol) (plist list)))))

(defun cl--spec-applicable-p (spec arg)
  (cond
   ((eq spec t) t)
   ((and (consp spec) (eq (car spec) 'eql)) (eql arg (cadr spec)))
   ((symbolp spec)
    (or (and (fboundp (intern (format "%sp" spec)))
             (funcall (intern (format "%sp" spec)) arg))
        ;; An EIEIO class name specializes on instances of it.
        (and (fboundp 'eieio--class-p)
             (funcall 'eieio--class-p spec)
             (fboundp 'object-of-class-p)
             (funcall 'object-of-class-p arg spec))
        (eq spec 't)))
   (t nil)))

(defun cl--spec-more-specific-p (a b)
  "Non-nil if specializer A is more specific than B."
  (cond
   ((equal a b) nil)
   ((and (consp a) (eq (car a) 'eql)) t)
   ((consp b) nil)
   ((eq b t) (not (eq a t)))
   ((eq a t) nil)
   (t (or (memq b (cl--type-parents a))
          ;; EIEIO subclass specializers are more specific than parents.
          (and (fboundp 'eieio--class-p)
               (funcall 'eieio--class-p a)
               (fboundp 'child-of-class-p)
               (funcall 'child-of-class-p a b))))))

(defun cl--method-more-specific-p (a b)
  "Compare method specs lexicographically."
  (let ((sa (car a)) (sb (car b)) (more nil))
    (while (and sa sb (not more) (equal (car sa) (car sb)))
      (setq sa (cdr sa) sb (cdr sb)))
    (when (and sa sb)
      (setq more (cl--spec-more-specific-p (car sa) (car sb))))
    more))

(defun cl--method-fn (m) (nth 2 m))

(defmacro cl-defgeneric (name args &rest body)
  "Define a generic function NAME with arglist ARGS.
BODY may contain a docstring, declarations, and options (subset)."
  (let ((doc (and (stringp (car body)) (car body))))
    `(progn
       (put ',name 'cl--methods nil)
       (defun ,name (&rest cl--args)
         ,@(and doc (list doc))
         (cl--generic-dispatch ',name cl--args))
       ',name)))

(defun cl--generic-dispatch (name args)
  (let* ((methods (get name 'cl--methods))
         (applicable
          (let (out)
            (dolist (m methods)
              (let ((specs (car m)) (ok t) (as args))
                (while (and specs as ok)
                  (unless (cl--spec-applicable-p (car specs) (car as))
                    (setq ok nil))
                  (setq specs (cdr specs) as (cdr as)))
                (when ok (push m out))))
            ;; Stable sort: most specific first.
            (sort (nreverse out)
                  (lambda (a b) (cl--method-more-specific-p a b)))))
         (around (cl--methods-with-qual applicable :around))
         (before (cl--methods-with-qual applicable :before))
         (primary (cl--methods-with-qual applicable nil))
         (after (nreverse (cl--methods-with-qual applicable :after))))
    (unless (or around primary)
      (signal 'cl-no-applicable-method (cons name args)))
    (let ((cl--cnm-ok t) (cl--cnm-name name))
      (dolist (m before) (apply (cl--method-fn m) args))
      (let* ((cl--cnm-args args)
             (cl--cnm (append (cdr around) primary))
             (fn (cl--method-fn (car (or around primary))))
             (result (apply fn args)))
        (dolist (m after) (apply (cl--method-fn m) args))
        result))))

(defun cl--methods-with-qual (methods qual)
  (let (out)
    (dolist (m methods)
      (when (eq (cadr m) qual) (push m out)))
    (nreverse out)))

(defmacro cl-defmethod (name &rest args)
  "Define a method for generic function NAME.
ARGS is [QUALIFIER] ARGLIST BODY where ARGLIST elements may be
VAR, (VAR TYPE), or (VAR (eql FORM))."
  (let ((qual nil))
    (when (and (car args) (not (listp (car args))))
      (setq qual (car args) args (cdr args)))
    (let* ((arglist (car args))
           (mbody (cdr args))
           (doc (and (stringp (car mbody)) (pop mbody)))
           (params nil) (specs nil))
      (dolist (a arglist)
        (cond
         ((symbolp a) (push a params) (push t specs))
         ((and (consp a) (eq (car a) '&rest)) (push a params)
          (push t specs))
         ((memq (car-safe a) '(&optional &aux &key))
          (push a params) (push t specs))
         ((consp a)
          (push (car a) params)
          (push (if (and (consp (cadr a)) (eq (caadr a) 'eql))
                    (list 'eql (eval (cadr (cadr a))))
                  (cadr a))
                specs))
         (t (push a params) (push t specs))))
      (let ((specs (nreverse specs))
            (params (nreverse params)))
        `(progn
           (put ',name 'cl--methods
                (cons (list ',specs ',qual
                            (lambda ,params
                              ,@(and doc (list doc)) ,@mbody))
                      (let ((old (get ',name 'cl--methods)) (out nil))
                        ;; Replace a method with same specs+qualifier.
                        (dolist (m old)
                          (unless (and (equal (car m) ',specs)
                                       (eq (cadr m) ',qual))
                            (push m out)))
                        (nreverse out))))
           ',name)))))

;; ---------- cl-lib / cl-seq subset ----------

(defun cl-gensym (&optional prefix)
  "Generate a new uninterned symbol with PREFIX (default \"G\")."
  (gensym (or prefix "G")))

(defun cl-gentemp (&optional prefix)
  "Generate a new interned symbol with PREFIX (default \"t\")."
  (let ((n 0) sym)
    (while (progn
             (setq sym (intern (format "%s%d" (or prefix "t")
                                       (setq n (1+ n)))))
             (fboundp sym)))
    sym))

(defsubst cl-minusp (n) "Return t if N is negative." (< n 0))
(defsubst cl-plusp (n) "Return t if N is positive." (> n 0))
(defsubst cl-oddp (n) "Return t if N is odd." (eq (logand n 1) 1))
(defsubst cl-evenp (n) "Return t if N is even." (eq (logand n 1) 0))

(defun cl-signum (n) "Return 1, 0 or -1 depending on N's sign."
  (cond ((> n 0) 1) ((< n 0) -1) (t 0)))

(defun cl-gcd (&rest args)
  "Greatest common divisor of ARGS."
  (let ((a 0))
    (dolist (x args (abs a))
      (setq x (abs x))
      (while (not (zerop x))
        (let ((r (% a x))) (setq a x x r))))))

(defun cl-lcm (&rest args)
  "Least common multiple of ARGS."
  (let ((l 1))
    (dolist (x args (abs l))
      (setq l (if (or (zerop l) (zerop x)) 0
                (/ (* l (abs x)) (cl-gcd l x)))))))

(defun cl-isqrt (n)
  "Integer square root of N."
  (if (fboundp 'isqrt)
      (isqrt n)
    (floor (sqrt (float n)))))

(defvar cl-most-positive-float 1.7976931348623157e308)
(defvar cl-most-negative-float -1.7976931348623157e308)
(defvar cl-least-positive-float 5e-324)
(defvar cl-least-negative-float -5e-324)
(defvar cl-least-positive-normalized-float 2.2250738585072014e-308)
(defvar cl-least-negative-normalized-float -2.2250738585072014e-308)
(defvar cl-float-epsilon 2.220446049250313e-16)
(defvar cl-float-negative-epsilon 1.1102230246251565e-16)

(defun cl-random (lim &optional state)
  "Random number < LIM (STATE ignored)."
  (random lim))

(defun cl-equalp (x y)
  "Like `equal' but case-insensitive for strings/chars and number
equivalence ignoring int/float distinction."
  (cond
   ((and (numberp x) (numberp y)) (= x y))
   ((and (stringp x) (stringp y)) (equal (downcase x) (downcase y)))
   ((and (characterp x) (characterp y))
    (= (downcase x) (downcase y)))
   (t (equal x y))))

(defun cl-copy-list (list) "Return a copy of LIST's spine."
  (copy-sequence list))

(defun cl-tailp (sublist list)
  "Return t if SUBLIST is a tail (eq) of LIST."
  (let ((tail list))
    (while (and (consp tail) (not (eq tail sublist)))
      (setq tail (cdr tail)))
    (eq tail sublist)))

(defun cl-ldiff (list sublist)
  "Return a copy of the part of LIST before SUBLIST."
  (let ((res nil) (tail list))
    (while (and (consp tail) (not (eq tail sublist)))
      (push (car tail) res)
      (setq tail (cdr tail)))
    (nreverse res)))

(defun cl-pairlis (keys values &optional alist)
  "Prepend (KEY . VALUE) pairs to ALIST."
  (while keys
    (push (cons (pop keys) (pop values)) alist))
  alist)

(defun cl--keyfn (keys)
  "The :key function from KEYS (default `identity')."
  (or (plist-get keys :key) #'identity))

(defun cl-member (item list &rest keys)
  "Like `member' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys)))
    (while (and list
                (not (if test
                         (funcall test item (funcall key (car list)))
                       (equal item (funcall key (car list))))))
      (setq list (cdr list)))
    list))

(defun cl-assoc (item alist &rest keys)
  "Like `assoc' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail alist))
    (while (and tail
                (let ((pair (car tail)))
                  (not (and (consp pair)
                            (if test
                                (funcall test item
                                         (funcall key (car pair)))
                              (eql item (funcall key (car pair))))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-rassoc (item alist &rest keys)
  "Like `rassoc' with :test/:key support."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail alist))
    (while (and tail
                (let ((pair (car tail)))
                  (not (and (consp pair)
                            (if test
                                (funcall test item
                                         (funcall key (cdr pair)))
                              (eql item (funcall key (cdr pair))))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-adjoin (item list &rest keys)
  "Add ITEM to LIST unless already `equal' to a member."
  (if (apply #'cl-member item list keys) list (cons item list)))

(defun cl-union (list1 list2 &rest keys)
  "Elements of LIST2 not already in LIST1 prepended to LIST1."
  (dolist (x list2 list1)
    (unless (apply #'cl-member x list1 keys)
      (push x list1))))

(defun cl-intersection (list1 list2 &rest keys)
  "Elements of LIST1 that are also in LIST2."
  (let ((res nil))
    (dolist (x list1 res)
      (when (apply #'cl-member x list2 keys)
        (push x res)))))

(defun cl-set-difference (list1 list2 &rest keys)
  "Elements of LIST1 not in LIST2."
  (let ((res nil))
    (dolist (x list1 (nreverse res))
      (unless (apply #'cl-member x list2 keys)
        (push x res)))))

(defun cl-subsetp (list1 list2 &rest keys)
  "Return t if every element of LIST1 is in LIST2."
  (let ((ok t))
    (dolist (x list1 ok)
      (unless (apply #'cl-member x list2 keys)
        (setq ok nil)))))

(defun cl-position (item seq &rest keys)
  "Position of ITEM in SEQ, or nil."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (from-end (plist-get keys :from-end))
        (l (append seq nil))
        (i 0) (found nil))
    (if from-end
        (let ((n (length l)) (r (nreverse (copy-sequence l))))
          (catch 'done
            (dolist (x r)
              (setq n (1- n))
              (when (if test
                        (funcall test item (funcall key x))
                      (eql item (funcall key x)))
                (throw 'done n)))))
      (while (and l (not found))
        (if (if test
                (funcall test item (funcall key (car l)))
              (eql item (funcall key (car l))))
            (setq found i)
          (setq l (cdr l) i (1+ i))))
      found)))

(defun cl-count (item seq &rest keys)
  "Number of elements equal to ITEM in SEQ."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (n 0))
    (dolist (x (append seq nil) n)
      (when (if test (funcall test item (funcall key x))
              (eql item (funcall key x)))
        (setq n (1+ n))))))

(defun cl-find (item seq &rest keys)
  "First element of SEQ equal to ITEM, or nil."
  (let ((test (plist-get keys :test))
        (key (cl--keyfn keys))
        (tail (append seq nil)))
    (while (and tail
                (not (if test
                         (funcall test item (funcall key (car tail)))
                       (eql item (funcall key (car tail))))))
      (setq tail (cdr tail)))
    (car tail)))

(defun cl-remove (item seq &rest keys)
  "Copy of SEQ with elements equal to ITEM removed."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (res nil))
    (dolist (x (append seq nil) (nreverse res))
      (unless (if test (funcall test item (funcall key x))
                (eql item (funcall key x)))
        (push x res)))))

(defun cl-delete (item seq &rest keys)
  "Like `cl-remove' (non-destructive for our lists)."
  (apply #'cl-remove item seq keys))

(defun cl-remove-if (pred seq &rest _keys)
  "Copy of SEQ with elements satisfying PRED removed."
  (let ((res nil))
    (dolist (x (append seq nil) (nreverse res))
      (unless (funcall pred x) (push x res)))))

(defun cl-delete-if (pred seq &rest keys)
  (apply #'cl-remove-if pred seq keys))

(defun cl-substitute (new old seq &rest keys)
  "Copy of SEQ with OLD replaced by NEW."
  (let ((test (plist-get keys :test)) (key (cl--keyfn keys)) (res nil))
    (dolist (x (append seq nil) (nreverse res))
      (push (if (if test (funcall test old (funcall key x))
                  (eql old (funcall key x)))
                new x)
            res))))

(defun cl-nsubstitute (new old seq &rest keys)
  (apply #'cl-substitute new old seq keys))

(defun cl-reduce (func seq &rest keys)
  "Reduce SEQ by FUNC (left-associative); empty SEQ → (func)."
  (let* ((from-end (plist-get keys :from-end))
         (l (append seq nil))
         (l (if from-end (nreverse l) l))
         (init (plist-get keys :initial-value)))
    (cond
     ((null l) (if init init (funcall func)))
     (t
      (let ((acc (if init (funcall func init (car l))
                   (pop l))))
        (while l
          (setq acc (if from-end
                        (funcall func (car l) acc)
                      (funcall func acc (car l))))
          (setq l (cdr l)))
        acc)))))

(defun cl-coerce (seq type)
  "Convert SEQ to TYPE (list, vector, string)."
  (cond
   ((eq type 'list) (append seq nil))
   ((eq type 'vector) (vconcat seq))
   ((eq type 'string)
    (cond ((stringp seq) seq)
          ((or (listp seq) (vectorp seq)) (concat seq))))
   (t (error "cl-coerce: unsupported type %s" type))))

(defun cl-typep (val type)
  "Return t if VAL is of TYPE (subset of CL type specifiers)."
  (cond
   ((consp type)
    (let ((op (car type)))
      (cond
       ((eq op 'or) (cl-some (lambda (tp) (cl-typep val tp)) (cdr type)))
       ((eq op 'and) (cl-every (lambda (tp) (cl-typep val tp)) (cdr type)))
       ((eq op 'not) (not (cl-typep val (cadr type))))
       ((eq op 'member) (memql val (cdr type)))
       (t (funcall op val)))))
   (t
    (funcall
     (or (cdr (assq type '((integer . integerp) (number . numberp)
                           (float . floatp) (string . stringp)
                           (symbol . symbolp) (cons . consp)
                           (list . listp) (vector . vectorp)
                           (hash-table . hash-table-p) (function . functionp)
                           (character . characterp) (boolean . booleanp)
                           (sequence . sequencep) (array . arrayp)
                           (atom . atom) (keyword . keywordp)
                           (fixnum . fixnump) (buffer . bufferp)
                           (window . windowp) (process . processp)
                           (frame . framep) (marker . markerp))))
         (error "cl-typep: unknown type %s" type))
     val))))

(defun cl-some (pred seq &rest _keys)
  "First non-nil (PRED X) for X in SEQ."
  (let ((res nil) (tail (append seq nil)))
    (while (and tail (not res))
      (let ((r (funcall pred (car tail))))
        (when r (setq res r)))
      (setq tail (cdr tail)))
    res))

(defun cl-every (pred seq)
  "t if PRED holds for every element of SEQ."
  (let ((ok t))
    (dolist (x (append seq nil) ok)
      (unless (funcall pred x) (setq ok nil)))))

(defun cl-check-type (val type &optional string)
  "Signal `wrong-type-argument' unless VAL is of TYPE."
  (unless (cl-typep val type)
    (signal 'wrong-type-argument (list type val string))))

(defmacro cl-assert (form &optional show-args string &rest args)
  "Signal an error unless FORM is non-nil."
  `(or ,form
       (signal 'cl-assertion-failed
               (list ',form ,show-args ,string ,@args))))

;; ---------- cl-macs subset ----------

(defun cl--zip (xs ys)
  "Zip XS and YS into a list of two-element lists."
  (let ((res nil))
    (while xs
      (push (list (pop xs) (pop ys)) res))
    (nreverse res)))

(defmacro cl-defun (name args &rest body)
  "Like `defun' with CL arglist support (subset: &key handled)."
  (cl--defun-1 'defun name args body))

(defmacro cl-defmacro (name args &rest body)
  "Like `defmacro' with CL arglist support (subset)."
  (cl--defun-1 'defmacro name args body))

(defun cl--defun-1 (kind name args body)
  (if (not (memq '&key args))
      `(,kind ,name ,args ,@body)
    ;; Convert &key params into a &rest plist extraction.
    (let ((plain nil) (keys nil) (rest nil) (state 'req)
          (kws nil))
      (dolist (a args)
        (cond
         ((eq a '&key) (setq state 'key))
         ((eq a '&rest) (setq state 'rest))
         ((eq a '&optional) (setq state 'opt))
         ((eq a '&aux) (setq state 'aux))
         ((eq a '&allow-other-keys) nil)
         ((eq state 'key)
          (let* ((v (if (consp a) (car a) a))
                 (def (if (consp a) (cadr a) nil))
                 (kw (intern (concat ":" (symbol-name v)))))
            (push kw kws)
            (push `(,v (car (cdr (or (plist-member cl--keys ,kw)
                                     (list nil ,def)))))
                  keys)))
         ((eq state 'rest) (push a rest) (setq state 'done))
         ((eq state 'aux)
          (push (if (consp a) a (list a nil)) keys))
         (t (push a plain))))
      `(,kind ,name
              ,(append (nreverse plain)
                       '(&rest cl--keys))
              (let ,(nreverse keys)
                (cl--check-keys cl--keys ',(nreverse kws))
                ,@body)))))

(defun cl--check-keys (plist allowed)
  "Validate keyword PLIST against ALLOWED keyword list."
  (let ((ks plist))
    (while ks
      (unless (memq (car ks) allowed)
        (error "Keyword argument %S not one of %s" (car ks)
               allowed))
      (setq ks (cddr ks)))))

(defmacro cl-case (expr &rest clauses)
  "Evaluate CLAUSES matching EXPR (each (KEYS . BODY))."
  (let ((v (gensym)) (default nil) (cases nil))
    (dolist (cl clauses)
      (if (memq (car cl) '(t otherwise))
          (setq default `(progn ,@(cdr cl)))
        (let ((k (car cl)))
          (push (list (if (consp k) `(memql ,v ',k) `(eql ,v ',k))
                      `(progn ,@(cdr cl)))
                cases))))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases) (t ,default)))))

(defmacro cl-ecase (expr &rest clauses)
  "Like `cl-case' but signals an error when no clause matches."
  (let ((v (gensym)) (cases nil) (allkeys nil))
    (dolist (cl clauses)
      (let ((k (car cl)))
        (setq allkeys (append allkeys (if (consp k) k (list k))))
        (push (list (if (consp k) `(memql ,v ',k) `(eql ,v ',k))
                    `(progn ,@(cdr cl)))
              cases)))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases)
             (t (error "cl-ecase failed: %s, %s" ,v
                       ',allkeys))))))

;; ---------- seq additions ----------

(defun seq-sort (predicate sequence)
  "Return SEQUENCE sorted by PREDICATE (non-destructive)."
  (let ((l (append sequence nil)))
    (setq l (sort l predicate))
    (if (vectorp sequence) (vconcat l) l)))

(defun seq-partition (seq n)
  "Return a list of N-element subsequences of SEQ."
  (let ((l (append seq nil)) (res nil) part)
    (while l
      (setq part nil)
      (dotimes (_ n) (when l (push (pop l) part)))
      (push (nreverse part) res))
    (nreverse res)))

(defun seq-group-by (function sequence)
  "Alist grouping SEQUENCE elements by (FUNCTION elt), in first-seen order."
  (let ((res nil))
    (dolist (x (append sequence nil) (nreverse res))
      (let* ((k (funcall function x))
             (cell (cl-assoc k res)))
        (if cell
            (setcdr cell (append (cdr cell) (list x)))
          (push (list k x) res))))))

(defun seq-mapn (function seq &rest seqs)
  "Map FUNCTION over SEQ and SEQS in parallel."
  (let ((lists (cons (append seq nil)
                     (mapcar (lambda (s) (append s nil)) seqs)))
        (res nil))
    (while (cl-every #'consp lists)
      (push (apply function (mapcar #'car lists)) res)
      (setq lists (mapcar #'cdr lists)))
    (nreverse res)))

(defalias 'cl-dolist 'dolist)
(defalias 'cl-dotimes 'dotimes)

(defmacro cl-typecase (expr &rest clauses)
  "Evaluate CLAUSES matching the type of EXPR (each (TYPE . BODY))."
  (let ((v (gensym)) (default nil) (cases nil))
    (dolist (cl clauses)
      (if (memq (car cl) '(t otherwise))
          (setq default `(progn ,@(cdr cl)))
        (push (list `(cl-typep ,v ',(car cl))
                    `(progn ,@(cdr cl)))
              cases)))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases) (t ,default)))))

(defmacro cl-etypecase (expr &rest clauses)
  "Like `cl-typecase' but signals an error when no clause matches."
  (let ((v (gensym)) (cases nil) (types nil))
    (dolist (cl clauses)
      (push (car cl) types)
      (push (list `(cl-typep ,v ',(car cl)) `(progn ,@(cdr cl))) cases))
    `(let ((,v ,expr))
       (cond ,@(nreverse cases)
             (t (error "cl-etypecase failed: %s, %s" ,v
                       ',(nreverse types)))))))

(defmacro cl-destructuring-bind (args expr &rest body)
  "Bind ARGS (a list pattern) to elements of EXPR.
Subset: flat patterns with &optional/&rest support."
  (let ((vals (gensym)) (binds nil) (rest-sym nil)
        (i 0) (state 'req))
    (dolist (a args)
      (cond
       ((eq a '&optional) (setq state 'opt))
       ((eq a '&rest) (setq state 'rest))
       ((memq a '(&key &aux &allow-other-keys)) (setq state 'skip))
       ((eq state 'rest)
        (setq rest-sym a state 'done))
       ((eq state 'skip) nil)
       (t
        (let ((v (if (consp a) (car a) a))
              (def (and (consp a) (cadr a))))
          (push (list v `(or (nth ,i ,vals) ,def)) binds)
          (setq i (1+ i))))))
    (when rest-sym
      (push (list rest-sym `(nthcdr ,i ,vals)) binds))
    `(let ((,vals ,expr))
       (let ,(nreverse binds) ,@body))))

(defmacro cl-letf (bindings &rest body)
  "Bind generalized PLACEs temporarily (subset: symbols and
\=(symbol-function SYM) places)."
  (let ((saves nil) (sets nil) (rests nil))
    (dolist (b bindings)
      (let ((place (car b)) (val (cadr b)) (tmp (gensym)))
        (cond
         ((and (consp place) (eq (car place) 'symbol-function))
          (push (list tmp `(symbol-function ,(cadr place))) saves)
          (push `(fset ,(cadr place) ,tmp) rests)
          (push `(fset ,(cadr place) ,val) sets))
         ((symbolp place)
          (push (list tmp place) saves)
          (push `(setq ,place ,tmp) rests)
          (push `(setq ,place ,val) sets))
         (t (error "cl-letf: unsupported place %s" place)))))
    `(let ,(nreverse saves)
       (unwind-protect
           (progn ,@(nreverse sets) ,@body)
         ,@(nreverse rests)))))

(defmacro cl-letf* (bindings &rest body)
  "Like `cl-letf' but bindings are made sequentially."
  (if (null bindings)
      `(progn ,@body)
    `(cl-letf (,(car bindings))
       (cl-letf* ,(cdr bindings) ,@body))))

(defmacro cl-flet (bindings &rest body)
  "Bind function names locally (dynamic extent)."
  (let ((lets nil))
    (dolist (b bindings)
      (push (list `(symbol-function ',(car b))
                  `(lambda ,@(cdr b)))
            lets))
    `(cl-letf ,(nreverse lets) ,@body)))

(defmacro cl-labels (bindings &rest body)
  "Like `cl-flet' (labels are dynamically scoped in this dialect)."
  `(cl-flet ,bindings ,@body))

(defmacro cl-macrolet (bindings &rest body)
  "Bind macro names locally."
  (let ((lets nil))
    (dolist (b bindings)
      (push (list `(symbol-function ',(car b))
                  `(cons 'macro (lambda ,@(cdr b))))
            lets))
    `(cl-letf ,(nreverse lets) ,@body)))

(defmacro cl-defstruct (name &rest slots)
  "Define a structure type NAME with SLOTS (subset: no options).
Creates make-NAME, NAME-p, and NAME-SLOT accessors; objects are
records whose first element is NAME."
  (let* ((n (if (consp name) (car name) name))
         (ctor (intern (concat "make-" (symbol-name n))))
         (pred (intern (concat (symbol-name n) "-p")))
         (slots (mapcar (lambda (x) (if (consp x) (car x) x)) slots))
         (defs nil) (i 1))
    (push `(defun ,ctor (&rest cl--keys)
             (apply #'record ',n
                    (mapcar (lambda (s)
                              (plist-get cl--keys
                                         (intern (concat ":"
                                                         (symbol-name s)))))
                            ',slots)))
          defs)
    (push `(defun ,pred (ob)
             (and (recordp ob) (eq (aref ob 0) ',n)))
          defs)
    (dolist (slot slots)
      (let* ((sn (if (consp slot) (car slot) slot))
             (acc (intern (concat (symbol-name n) "-"
                                  (symbol-name sn)))))
        (push `(defun ,acc (ob) (aref ob ,i)) defs))
      (setq i (1+ i)))
    `(progn ,@(nreverse defs) ',n)))

;; ---------- registers / misc ----------

(defun register-read-with-preview (prompt)
  "Read a register name, showing PROMPT."
  (read-char prompt))

(defun append-to-register (register start end &optional delete-flag)
  "Append region text to REGISTER."
  (let ((text (buffer-substring start end)))
    (set-register register
                  (concat (or (get-register register) "") text))
    (when delete-flag (delete-region start end))))

(defun prepend-to-register (register start end &optional delete-flag)
  "Prepend region text to REGISTER."
  (let ((text (buffer-substring start end)))
    (set-register register
                  (concat text (or (get-register register) "")))
    (when delete-flag (delete-region start end))))

(defun set-file-extended-attributes (filename attributes)
  "Set extended ATTRIBUTES on FILENAME (best effort; returns nil)."
  nil)

(defmacro with-suppressed-warnings (_warnings &rest body)
  "Eval BODY with byte-compile WARNINGS suppressed."
  `(progn ,@body))

(defun exec-path ()
  "Return `exec-path'."
  exec-path)

(defun path-separator ()
  "Return `path-separator'."
  path-separator)

(defun cl-float-limits ()
  "Initialize the cl-float-* variables (already set)."
  nil)

;; ---------- cl-loop (subset) ----------

(defmacro cl-loop (&rest clauses)
  "Common Lisp `loop' macro subset: for/in/on/across/=,/from..to,
with, while, until, repeat, if/when/unless, do, collect, append,
nconc, sum, count, maximize, minimize, return, initially, finally."
  (cl--loop-expand clauses))

(defconst cl--loop-keywords
  '(for as with if when unless else end do doing collect collecting
    append appending nconc nconcing sum counting count maximize
    maximizing minimize minimizing return while until repeat
    initially finally from to upto below downto above upfrom
    downfrom in on across by = then and it being the elements
    hash-key hash-keys hash-value hash-values of each))

(defun cl--loop-action (clauses i)
  "Parse one action clause at index I; return (FORMS KINDS NEW-I).
Accumulation refers to the `cl--loop-list-acc' and
`cl--loop-num-acc' variables bound by the generated code."
  (let ((kw (nth i clauses)) (forms nil) (kinds nil))
    (cond
     ((memq kw '(do doing))
      (setq i (1+ i))
      (while (and (< i (length clauses))
                  (not (memq (nth i clauses) cl--loop-keywords)))
        (push (nth i clauses) forms)
        (setq i (1+ i)))
      (setq forms (list (cons 'progn (nreverse forms)))))
     ((memq kw '(collect collecting append appending nconc nconcing
                 sum counting count maximize maximizing minimize
                 minimizing))
      (let* ((e (nth (1+ i) clauses))
             (kind (cond ((memq kw '(collect collecting)) 'collect)
                         ((memq kw '(append appending)) 'append)
                         ((memq kw '(nconc nconcing)) 'nconc)
                         ((memq kw '(sum counting)) 'sum)
                         ((eq kw 'count) 'count)
                         ((memq kw '(maximize maximizing)) 'max)
                         (t 'min))))
        (setq i (+ i 2))
        (when (eq (nth i clauses) 'into) (setq i (+ i 2)))
        (push kind kinds)
        (push
         (cond
          ((eq kind 'collect) `(push ,e cl--loop-list-acc))
          ((eq kind 'append)
           `(setq cl--loop-list-acc
                  (nconc cl--loop-list-acc (append ,e nil))))
          ((eq kind 'nconc)
           `(setq cl--loop-list-acc (nconc cl--loop-list-acc ,e)))
          ((eq kind 'sum)
           `(setq cl--loop-num-acc (+ cl--loop-num-acc ,e)))
          ((eq kind 'count)
           `(when ,e (setq cl--loop-num-acc (1+ cl--loop-num-acc))))
          (t `(setq cl--loop-ext-acc
                    (if cl--loop-ext-acc
                        (,(if (eq kind 'max) 'max 'min)
                         cl--loop-ext-acc ,e)
                      ,e))))
         forms)))
     ((eq kw 'return)
      (push `(throw 'cl--loop ,(nth (1+ i) clauses)) forms)
      (setq i (+ i 2)))
     (t (error "cl-loop: bad action clause %s" kw)))
    (list forms kinds i)))

(defun cl--loop-expand (clauses)
  (let ((inits nil) (initially nil) (pretests nil) (pre nil)
        (steps nil) (body nil) (finally nil) (finret nil)
        (kinds nil) (i 0) (n (length clauses)))
    (while (< i n)
      (let ((kw (nth i clauses)))
        (cond
         ((memq kw '(for as))
          ;; for VAR <iter> [and VAR <iter>]*
          (let ((var nil))
            (setq i (1+ i))
            (let ((more t))
              (while more
                (setq var (nth i clauses) i (1+ i))
                (let ((op (nth i clauses)))
                  (setq i (1+ i))
                  (cond
                   ((memq op '(in on))
                    (let ((tl (gensym)) (src (nth i clauses)) (by nil))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'by)
                        (setq by (nth (1+ i) clauses) i (+ i 2)))
                      (setq inits (append inits
                                          (list (list tl src)
                                                (list var nil))))
                      (push `(consp ,tl) pretests)
                      (push (if (eq op 'on) `(setq ,var ,tl)
                              `(setq ,var (car ,tl)))
                            pre)
                      (push (if by `(setq ,tl (funcall ,by ,tl))
                              `(setq ,tl (cdr ,tl)))
                            steps)))
                   ((eq op 'across)
                    (let ((v (gensym)) (ix (gensym)))
                      (setq inits (append inits
                                          (list (list v (nth i clauses))
                                                (list ix 0)
                                                (list var nil))))
                      (push `(< ,ix (length ,v)) pretests)
                      (push `(setq ,var (aref ,v ,ix)) pre)
                      (push `(setq ,ix (1+ ,ix)) steps)
                      (setq i (1+ i))))
                   ((eq op '=)
                    (let ((e (nth i clauses)) (then nil))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'then)
                        (setq then (nth (1+ i) clauses) i (+ i 2)))
                      (setq inits (append inits (list (list var e))))
                      (push `(setq ,var ,(or then e)) steps)))
                   ((eq op 'being)
                    (when (eq (nth i clauses) 'the) (setq i (1+ i)))
                    (when (memq (nth i clauses) '(elements element))
                      (setq i (1+ i))
                      (when (eq (nth i clauses) 'of) (setq i (1+ i)))
                      (let ((v (gensym)) (ix (gensym)))
                        (setq inits
                              (append inits (list (list v (nth i clauses))
                                                  (list ix 0)
                                                  (list var nil))))
                        (push `(< ,ix (length ,v)) pretests)
                        (push `(setq ,var (aref ,v ,ix)) pre)
                        (push `(setq ,ix (1+ ,ix)) steps)
                        (setq i (1+ i)))))
                   ((memq op '(from upfrom downfrom below above
                                  to upto downto))
                    (let ((down (memq op '(downfrom downto)))
                          (init-e (if (memq op '(from upfrom downfrom))
                                      (prog1 (nth i clauses)
                                        (setq i (1+ i)))
                                    (if (memq op '(below to upto)) 0
                                      0)))
                          (bound nil) (cmp nil) (step 1))
                      (when (memq op '(below above to upto downto))
                        ;; Bound was consumed as the "op" itself.
                        (let ((e2 (nth i clauses)))
                          (setq i (1+ i))
                          (cond
                           ((eq op 'below) (setq bound e2 cmp '<))
                           ((eq op 'above) (setq bound e2 cmp '> down t))
                           ((memq op '(to upto)) (setq bound e2 cmp '<=))
                           ((eq op 'downto)
                            (setq bound e2 cmp '>= down t init-e
                                  e2)))))
                      (while (memq (nth i clauses)
                                   '(to upto below downto above by))
                        (let ((k2 (nth i clauses))
                              (e2 (nth (1+ i) clauses)))
                          (cond
                           ((memq k2 '(to upto))
                            (setq bound e2 cmp '<=))
                           ((eq k2 'below)
                            (setq bound e2 cmp '<))
                           ((eq k2 'downto)
                            (setq bound e2 cmp '>= down t))
                           ((eq k2 'above)
                            (setq bound e2 cmp '> down t))
                           ((eq k2 'by) (setq step e2)))
                          (setq i (+ i 2))))
                      (setq inits
                            (append inits (list (list var init-e))))
                      (when bound
                        (push (list cmp var bound) pretests))
                      (push `(setq ,var (,(if down '- '+) ,var ,step))
                            steps)))
                   (t (error "cl-loop: bad for clause %s" op))))
                (setq more (eq (nth i clauses) 'and))
                (when more (setq i (1+ i)))))))
         ((eq kw 'with)
          (let ((more t))
            (setq i (1+ i))
            (while more
              (let ((v (nth i clauses)) (e nil))
                (setq i (1+ i))
                (when (eq (nth i clauses) '=)
                  (setq e (nth (1+ i) clauses) i (+ i 2)))
                (setq inits (append inits (list (list v e))))
                (setq more (eq (nth i clauses) 'and))
                (when more (setq i (1+ i)))))))
         ((eq kw 'repeat)
          (let ((c (gensym)))
            (setq inits
                  (append inits (list (list c (nth (1+ i) clauses)))))
            (push `(> ,c 0) pretests)
            (push `(setq ,c (1- ,c)) steps)
            (setq i (+ i 2))))
         ((eq kw 'while)
          (push (nth (1+ i) clauses) pretests) (setq i (+ i 2)))
         ((eq kw 'until)
          (push `(not ,(nth (1+ i) clauses)) pretests) (setq i (+ i 2)))
         ((memq kw '(if when unless))
          (let* ((raw (nth (1+ i) clauses))
                 (cnd (if (eq kw 'unless) `(not ,raw) raw))
                 (a (cl--loop-action clauses (+ i 2)))
                 (then-forms (car a)) (j (nth 2 a)) (else-forms nil))
            (setq kinds (append kinds (cadr a)))
            (when (eq (nth j clauses) 'else)
              (let ((b (cl--loop-action clauses (1+ j))))
                (setq else-forms (car b) j (nth 2 b)
                      kinds (append kinds (cadr b)))))
            (when (eq (nth j clauses) 'end) (setq j (1+ j)))
            (push `(if ,cnd (progn ,@then-forms)
                     ,@(when else-forms `((progn ,@else-forms))))
                  body)
            (setq i j)))
         ((eq kw 'initially)
          (setq i (1+ i))
          (while (and (< i n)
                      (not (memq (nth i clauses) cl--loop-keywords)))
            (push (nth i clauses) initially)
            (setq i (1+ i))))
         ((eq kw 'finally)
          (setq i (1+ i))
          (when (memq (nth i clauses) '(do doing)) (setq i (1+ i)))
          (if (eq (nth i clauses) 'return)
              (setq finret (nth (1+ i) clauses) i (+ i 2))
            (while (and (< i n)
                        (not (memq (nth i clauses) cl--loop-keywords)))
              (push (nth i clauses) finally)
              (setq i (1+ i)))))
         ((memq kw '(do doing collect collecting append appending
                    nconc nconcing sum counting count maximize
                    maximizing minimize minimizing return))
          (let ((a (cl--loop-action clauses i)))
            (setq body (append body (car a))
                  kinds (append kinds (cadr a))
                  i (nth 2 a))))
         (t (error "cl-loop: unknown clause %s" kw)))))
    `(let ,(append (nreverse inits)
                   '((cl--loop-list-acc nil) (cl--loop-num-acc 0)
                     (cl--loop-ext-acc nil)))
       (catch 'cl--loop
         ,@(nreverse initially)
         (while (and ,@(nreverse pretests))
           ,@(nreverse pre)
           ,@body
           ,@(nreverse steps))
         ,@(nreverse finally)
         ,(or finret
              (cond
               ((memq 'collect kinds) '(nreverse cl--loop-list-acc))
               ((or (memq 'append kinds) (memq 'nconc kinds))
                'cl--loop-list-acc)
               ((or (memq 'sum kinds) (memq 'count kinds))
                'cl--loop-num-acc)
               ((or (memq 'max kinds) (memq 'min kinds))
                'cl--loop-ext-acc)
               (t nil)))))))

(defun cl--sm-subst (form bindings)
  "Substitute symbol-macrolet BINDINGS ((SYM FORM)...) in FORM tree.
`setq' on a bound symbol becomes `setf' on its expansion, like GNU's
`cl--sm-macroexpand'."
  (cond
   ((symbolp form)
    (let ((b (assq form bindings)))
      (if b (cadr b) form)))
   ((consp form)
    (cond
     ((memq (car form) '(quote function))
      form)
     ((memq (car form) '(setq setq-default))
      ;; (setq SYM VAL ...) → (setf EXPANSION VAL ...) for bound syms.
      (let ((args (cdr form)) (out nil))
        (while args
          (let* ((s (car args)) (v (cadr args))
                 (b (assq s bindings)))
            (if b
                (push (list 'setf (cl--sm-subst (cadr b) bindings)
                            (cl--sm-subst v bindings))
                      out)
              (push (list (car form) s
                          (cl--sm-subst v bindings))
                    out)))
          (setq args (cddr args)))
        (if (cdr out) (cons 'progn (nreverse out)) (car out))))
     (t
      (cons (cl--sm-subst (car form) bindings)
            (cl--sm-subst (cdr form) bindings)))))
   (t form)))

(defmacro cl-symbol-macrolet (bindings &rest body)
  "Bind symbols as macros expanding to their forms (subset)."
  `(progn ,@(cl--sm-subst body bindings)))

;; ---------- occur / replace subset ----------

(defun list-matching-lines (regexp &optional nlines buffer)
  "Show lines matching REGEXP in BUFFER (subset: *Occur* buffer)."
  (interactive "sList lines matching: ")
  (let ((buf (or buffer (current-buffer))) (matches nil))
    (with-current-buffer buf
      (save-excursion
        (goto-char (point-min))
        (while (re-search-forward regexp nil t)
          (push (buffer-substring (line-beginning-position)
                                  (line-end-position))
                matches)
          (forward-line 1))))
    (setq matches (nreverse matches))
    (if (null matches)
        (message "Searched %d buffer%s; no matches for \"%s\""
                 1 "" regexp)
      (let ((obuf (get-buffer-create "*Occur*")))
        (with-current-buffer obuf
          (erase-buffer)
          (insert (format "%d match%s for \"%s\" in buffer: %s\n"
                          (length matches)
                          (if (= (length matches) 1) "" "es")
                          regexp (buffer-name buf)))
          (dolist (m matches) (insert m "\n")))
        (display-buffer obuf)))
    t))

(defun perform-replace (from-string replacements query-flag
                        regexp-flag delimited-flag
                        &optional repeat-count map start end
                        backward region-noncontiguous-p)
  "Replace FROM-STRING with REPLACEMENTS (subset: non-query only)."
  (save-excursion
    (goto-char (or start (point-min)))
    (let ((count 0) (limit (or repeat-count most-positive-fixnum)))
      (while (and (< count limit)
                  (if regexp-flag
                      (re-search-forward from-string end t)
                    (search-forward from-string end t)))
        (replace-match replacements (not regexp-flag)
                       (not regexp-flag))
        (setq count (1+ count)))
      count)))

;; ---------- more subr.el / simple.el helpers ----------

(defun alist-get (key alist &optional default remove testfn)
  "Return the value associated with KEY in ALIST."
  (let ((x (if testfn
               (cl-assoc key alist :test testfn)
             (assoc key alist))))
    (if x (cdr x) default)))

(defun flatten-list (list)
  "Flatten LIST: return a list of all non-nil atoms."
  (let ((res nil) (stack (list list)))
    (while stack
      (let ((x (pop stack)))
        (cond
         ((null x) nil)
         ((consp x)
          (push (cdr x) stack)
          (push (car x) stack))
         (t (push x res)))))
    (nreverse res)))

(defun buffer-narrowed-p (&optional buffer)
  "Return t if BUFFER is narrowed."
  (with-current-buffer (or buffer (current-buffer))
    (or (/= (point-min) 1)
        (/= (point-max) (1+ (buffer-size))))))

(defun shell-quote-argument (arg)
  "Quote ARG for use as a shell argument (POSIX style)."
  (if (equal arg "")
      "''"
    (let ((res nil))
      (dolist (c (append arg nil))
        (push (if (string-match-p "[-a-zA-Z0-9_@%+=:,./~]"
                                  (char-to-string c))
                  (char-to-string c)
                (concat "\\" (char-to-string c)))
              res))
      (apply #'concat (nreverse res)))))

(defun combine-and-quote-strings (strings &optional separator)
  "Join STRINGS with SEPARATOR (default space), quoting strings
containing whitespace or quotes with double quotes."
  (mapconcat
   (lambda (st)
     (if (string-match "[\"\\ 	
]" st)
         (concat "\"" (string-replace "\"" "\\\"" st) "\"")
       st))
   strings
   (or separator " ")))

(defun split-string-and-unquote (string &optional separator)
  "Split STRING at SEPARATOR (default whitespace) respecting
double-quoted spans; the quotes are removed."
  (let ((seps (if separator (append separator nil)
                '(?\s ?\t ?\n ?\r ?\f ?\v)))
        (res nil) (cur nil) (inq nil) (i 0) (n (length string)))
    (while (< i n)
      (let ((c (aref string i)))
        (cond
         ((and (not inq) (memq c seps))
          (when cur (push (apply #'concat (nreverse cur)) res))
          (setq cur nil i (1+ i)))
         ((eq c ?\") (setq inq (not inq) i (1+ i)))
         (t (push (char-to-string c) cur) (setq i (1+ i))))))
    (when cur (push (apply #'concat (nreverse cur)) res))
    (nreverse res)))

(defun time-to-seconds (&optional time)
  "Convert TIME (a Lisp time value) to seconds."
  (float-time time))

(defvar emacs--start-time (float-time))

(defun emacs-init-time ()
  "Return a string describing the Emacs startup time."
  (format "%.1f seconds" (- (float-time) emacs--start-time)))

(defun function-alias-p (func &optional _noerror)
  "Return non-nil if FUNC's function cell is another symbol."
  (let ((d (and (symbolp func) (symbol-function func))))
    (and (symbolp d) (list d))))

(defun symbol-file (symbol &optional type)
  "Return the file where SYMBOL was defined (nil if unknown)."
  nil)

(defun find-lisp-object-file-name (object &optional type)
  "Return the file where OBJECT was defined (nil if unknown)."
  nil)

(defun pop-to-buffer-same-window (buffer &optional norecord)
  "Display BUFFER in the selected window."
  (pop-to-buffer buffer nil norecord))

(defun field-at-pos (pos)
  "Return the `field' property at POS."
  (get-char-property pos 'field))

(defun completion-try-completion (string table pred point
                                         &optional metadata)
  "Try to complete STRING using TABLE."
  (let ((r (try-completion string table pred)))
    (if (stringp r) (cons r (or point (length r))) r)))

(defun completion-all-completions (string table pred point
                                          &optional metadata)
  "All completions of STRING in TABLE."
  (all-completions string table pred))

(defun completion--action (action table string pred)
  "Perform completion ACTION on TABLE for STRING and PRED."
  (cond
   ((functionp table) (funcall table string pred action))
   ((or (eq action 'metadata) (eq action 'boundaries)
        (eq (car-safe action) 'boundaries)) nil)
   ((eq action nil) (try-completion string table pred))
   ((eq action 'lambda) (test-completion string table pred))
   (t (all-completions string table pred))))

(defun completion-table-dynamic (fun &optional switch-buffer)
  "Use FUN as a dynamic completion table: FUN is called with the
string to complete and returns the list of completions."
  ;; `',fun': dynamic scope means no closures — embed the value.
  (list 'lambda '(string pred action)
        (list 'completion--action 'action
              (list 'funcall (list 'quote fun) 'string)
              'string 'pred)))

(defun completion-table-merge (&rest tables)
  "Return a completion table merging the candidates of all TABLES."
  (list 'lambda '(string pred action)
        `(if (eq action 'lambda)
             (cl-some
              (lambda (tab) (completion--action action tab string pred))
              ',tables)
           (completion--action
            action
            (apply #'append
                   (mapcar (lambda (tab) (all-completions string tab pred))
                           ',tables))
            string pred))))

(defun completion-table-in-turn (&rest tables)
  "Return a completion table trying each of TABLES in turn."
  (list 'lambda '(string pred action)
        `(if (eq (car-safe action) 'boundaries)
             (completion--action action (car ',tables) string pred)
           (cl-some
            (lambda (tab) (completion--action action tab string pred))
            ',tables))))

(defun completion-table-with-cache (fun &optional bound)
  "Return a dynamic completion table caching FUN's last result.
The cache is reused while STRING still starts with the string the
cache was computed for."
  (let ((cell (cons nil nil)))
    (list 'lambda '(string pred action)
          `(let ((cache ',cell))
             (unless (and (car cache)
                          (string-prefix-p (car cache) string)
                          (or (null ,bound)
                              (<= (length string)
                                  (+ ,bound (length (car cache))))))
               (setcar cache string)
               (setcdr cache (funcall ',fun string)))
             (completion--action action (cdr cache) string pred)))))

(defun completion-table-with-context (prefix table string pred action)
  "Complete STRING in TABLE as if PREFIX preceded it.
For action nil the returned completion includes PREFIX; for t the
candidates are returned without it."
  (cond
   ((or (eq action 'metadata) (eq action 'boundaries)
        (eq (car-safe action) 'boundaries)) nil)
   (t
    (let ((comp (completion--action action table string pred)))
      (if (and (eq action nil) (stringp comp))
          (concat prefix comp)
        comp)))))

(defmacro macroexp-quote (v)
  "Return the argument V converted to a form that will \"quote\" it."
  (if (or (consp v)
          (and (symbolp v) (not (keywordp v))))
      (list 'quote v)
    v))

(defun substitute-key-definition (olddef newdef keymap
                                  &optional oldmap prefix)
  "In KEYMAP, rebind every key bound to OLDDEF in OLDMAP to NEWDEF."
  (map-keymap
   (lambda (key def)
     (when (eq def olddef)
       (define-key keymap (vector key) newdef)))
   (or oldmap (current-global-map))))

(defun add-to-ordered-list (list-var element &optional order)
  "Add ELEMENT to the value of LIST-VAR if it isn't there yet.
The test for presence of ELEMENT is done with `eq'.  Numeric ORDER
is recorded as the element's rank; elements with ranks sort before
those without.  LIST-VAR cannot refer to a lexical variable."
  (let ((ordering (get list-var 'list-order)))
    (unless ordering
      (put list-var 'list-order
           (setq ordering (make-hash-table :weakness 'key :test 'eq))))
    (when order
      (puthash element (and (numberp order) order) ordering))
    (unless (memq element (symbol-value list-var))
      (set list-var (cons element (symbol-value list-var))))
    (set list-var
         (sort (symbol-value list-var)
               (lambda (a b)
                 (let ((oa (gethash a ordering))
                       (ob (gethash b ordering)))
                   (if (and oa ob) (< oa ob) oa)))))
    (symbol-value list-var)))

(defvar history-length 60)
(defvar history-delete-duplicates nil
  "Non-nil means `add-to-history' removes duplicate entries.")

(defun add-to-history (history-var newelt &optional maxelt keep-all)
  "Add NEWELT to the history list stored in the variable HISTORY-VAR.
MAXELT bounds the length (default: the `history-length' property of
HISTORY-VAR, else the `history-length' variable).  Empty strings and
entries equal to the most recent element are skipped unless KEEP-ALL.
HISTORY-VAR cannot refer to a lexical variable."
  (unless maxelt
    (setq maxelt (or (get history-var 'history-length)
                     history-length)))
  (let ((history (symbol-value history-var))
        tail)
    (when (and (listp history)
               (or keep-all (not (stringp newelt))
                   (plusp (length newelt)))
               (or keep-all (not (equal (car history) newelt))))
      (if history-delete-duplicates
          (setq history (delete newelt history)))
      (setq history (cons newelt history))
      (when (integerp maxelt)
        (if (>= 0 maxelt)
            (setq history nil)
          (setq tail (nthcdr (1- maxelt) history))
          (when (consp tail)
            (setcdr tail nil)))))
    (set history-var history)))

(defun cl--take (n list)
  (let ((res nil))
    (while (and (> n 0) list)
      (push (pop list) res) (setq n (1- n)))
    (nreverse res)))

(defvar buffer-invisibility-spec t
  "If t, all invisible text is invisible; if a list, only listed
symbols (and (sym . t) entries) make text invisible.")

(defun add-to-invisibility-spec (element)
  "Add ELEMENT to `buffer-invisibility-spec'.
See documentation for `buffer-invisibility-spec' for the kind of
elements that can be added."
  (if (eq buffer-invisibility-spec t)
      (setq buffer-invisibility-spec (list t)))
  (add-to-list 'buffer-invisibility-spec element))

(defun remove-from-invisibility-spec (element)
  "Remove ELEMENT from `buffer-invisibility-spec'."
  (if (consp buffer-invisibility-spec)
      (setq buffer-invisibility-spec
            (delete element buffer-invisibility-spec))))

(defun add-minor-mode (toggle name &optional keymap after lighter)
  "Register a minor mode in `minor-mode-alist'."
  (let ((existing (assq toggle minor-mode-alist))
        (entry (list toggle (or lighter name))))
    (if existing
        (setcdr existing (cdr entry))
      (setq minor-mode-alist (cons entry minor-mode-alist)))
    (when keymap
      (setq minor-mode-map-alist
            (cons (cons toggle keymap) minor-mode-map-alist)))))

(defun event--posn-at-point ()
  (if (fboundp 'posn-at-point) (posn-at-point)))

(defun event-start (event)
  "Return the starting position of EVENT, a click or drag event.
If EVENT is nil, the value of `posn-at-point' is used instead."
  (if (and (consp event)
           (memq (car event) '(touchscreen-begin touchscreen-end)))
      (cdr (car-safe (cdr event)))
    (or (and (consp event)
             (not (eq (car event) 'touchscreen-update))
             (nth 1 event))
        (event--posn-at-point))))

(defun event-end (event)
  "Return the ending position of EVENT.
See `event-start' for a description of the value returned."
  (if (and (consp event)
           (memq (car event) '(touchscreen-begin touchscreen-end)))
      (cdr (car-safe (cdr event)))
    (or (and (consp event)
             (not (eq (car event) 'touchscreen-update))
             (nth (if (consp (nth 2 event)) 2 1) event))
        (event--posn-at-point))))

(defsubst event-click-count (event)
  "Return the multi-click count of EVENT, a click or drag event."
  (if (and (consp event) (integerp (nth 2 event))) (nth 2 event) 1))

(defsubst event-line-count (event)
  "Return the line count of EVENT, a mousewheel event."
  (if (and (consp event) (integerp (nth 3 event))) (nth 3 event) 1))

(defun posnp (obj)
  "Return non-nil if OBJ appears to be a valid posn object."
  (and (windowp (car-safe obj))
       (atom (car-safe (car-safe (cdr obj))))
       (integerp (car-safe (car-safe (cdr (cdr obj)))))
       (integerp (car-safe (cdr (cdr (cdr obj)))))))

(defun posn-window (position) "Return the window in POSITION."
  (nth 0 position))

(defun posn-area (position)
  "Return the window area recorded in POSITION, or nil for the text area."
  (let ((area (if (consp (nth 1 position))
                  (car (nth 1 position))
                (nth 1 position))))
    (and (symbolp area) area)))

(defun posn-point (position)
  "Return the buffer location in POSITION."
  (or (nth 5 position)
      (let ((pt (nth 1 position)))
        (or (car-safe pt)
            (if (integerp pt) pt)))))

(defsubst posn-x-y (position)
  "Return the x and y coordinates in POSITION as (X . Y)."
  (nth 2 position))

(defun posn-actual-col-row (position)
  "Return the window row number and character number in POSITION."
  (nth 6 position))

(defun posn-col-row (position &optional use-window)
  "Return the nominal column and row in POSITION, in characters."
  (let* ((pair (posn-x-y position))
         (frame-or-window (posn-window position))
         (window (and (windowp frame-or-window) frame-or-window))
         (area (posn-area position)))
    (cond
     ((null frame-or-window) '(0 . 0))
     ((eq area 'vertical-scroll-bar)
      (cons 0 (scroll-bar-scale pair (1- (window-height window)))))
     ((eq area 'horizontal-scroll-bar)
      (cons (scroll-bar-scale pair (window-width window)) 0))
     (t (if use-window
            (cons (/ (car pair) (window-font-width window))
                  (/ (cdr pair) (window-font-height window)))
          (cons (/ (car pair)
                   (frame-char-width
                    (if (framep frame-or-window) frame-or-window
                      (window-frame frame-or-window))))
                (/ (cdr pair)
                   (frame-char-height
                    (if (framep frame-or-window) frame-or-window
                      (window-frame frame-or-window))))))))))

(defsubst posn-timestamp (position)
  "Return the timestamp of POSITION."
  (nth 3 position))

(defun posn-string (position)
  "Return the string object of POSITION: a cons (STRING . POS) or nil."
  (let ((x (nth 4 position)))
    (when (consp x) x)))

(defsubst posn-image (position)
  "Return the image object of POSITION, or nil if not an image."
  (nth 7 position))

(defsubst posn-object (position)
  "Return the object (image or string) of POSITION."
  (or (posn-image position) (posn-string position)))

(defsubst posn-object-x-y (position)
  "Return the (DX . DY) offset of the object glyph in POSITION."
  (nth 8 position))

(defsubst posn-object-width-height (position)
  "Return the (WIDTH . HEIGHT) of the object glyph in POSITION."
  (nth 9 position))

(defun posn-set-point (position)
  "Move point to POSITION; select the corresponding window."
  (if (framep (posn-window position))
      (progn
        (unless (windowp (frame-selected-window (posn-window position)))
          (error "Position not in text area of window"))
        (select-window (frame-selected-window (posn-window position))))
    (unless (windowp (posn-window position))
      (error "Position not in text area of window"))
    (select-window (posn-window position)))
  (if (numberp (posn-point position))
      (goto-char (posn-point position))))

;; ---------- mode machinery ----------

(defmacro define-derived-mode (variant parent name &optional docstring
                                       &rest body)
  "Define VARIANT as a major mode derived from PARENT (subset)."
  (let* ((map-sym (intern (concat (symbol-name variant) "-map")))
         (hook-sym (intern (concat (symbol-name variant) "-hook")))
         (syntax-sym (intern (concat (symbol-name variant)
                                     "-syntax-table"))))
    `(progn
       (defvar ,map-sym
               ,(if parent
                    `(let ((m (make-sparse-keymap))
                           (pmsym ',(intern
                                     (concat (symbol-name parent)
                                             "-map"))))
                       (when (boundp pmsym)
                         (set-keymap-parent m (symbol-value pmsym)))
                       m)
                  '(make-sparse-keymap))
               ,(concat "Keymap for `" (symbol-name variant) "'."))
       (defvar ,syntax-sym (copy-syntax-table))
       (defvar ,hook-sym nil)
       (defun ,variant ()
         ,@(when docstring (list docstring))
         (interactive)
         ,@(when parent `((when (fboundp ',parent) (,parent))))
         (kill-all-local-variables)
         (setq major-mode ',variant
               mode-name ,name)
         (use-local-map ,map-sym)
         ,@body
         (run-mode-hooks ',hook-sym))
       ,@(when parent
           `((derived-mode-set-parent ',variant ',parent)))
       ',variant)))

(defun merge-ordered-lists (lists &optional error-function)
  "Merge LISTS in a consistent order (C3 linearization).
Equality is tested with `eql'.  On inconsistency, ERROR-FUNCTION is
called with the remaining lists; by default the head of the first
list is used."
  (let ((result nil))
    (setq lists (remq nil lists))
    (while (cdr lists)
      (let* ((find-next
              (lambda (lists)
                (let ((next nil)
                      (tail lists))
                  (while tail
                    (let ((candidate (caar tail))
                          (other-lists lists))
                      (while other-lists
                        (if (not (memql candidate (cdr (car other-lists))))
                            (setq other-lists (cdr other-lists))
                          (setq candidate nil)
                          (setq other-lists nil)))
                      (if (not candidate)
                          (setq tail (cdr tail))
                        (setq next candidate)
                        (setq tail nil))))
                  next)))
             (next (funcall find-next lists)))
        (unless next
          (let ((tail lists))
            (while (and (cdr tail) (null (funcall find-next (cdr tail))))
              (setq tail (cdr tail)))
            (setq next
                  (funcall (or error-function
                               (lambda (remaining-lists)
                                 (message "Inconsistent hierarchy: %S"
                                          remaining-lists)
                                 (caar remaining-lists)))
                           tail))
            (unless (assoc next lists #'eql)
              (error "Invalid candidate returned by error-function: %S"
                     next))
            (dolist (list lists) (setcdr list (remq next (cdr list))))))
        (push next result)
        (setq lists
              (delq nil
                    (mapcar (lambda (l) (if (eql (car l) next) (cdr l) l))
                            lists)))))
    (if (null result) (car lists)
      (append (nreverse result) (car lists)))))

(defun derived-mode-all-parents (mode &optional known-children)
  "Return all the parents of MODE, starting with MODE."
  (let ((ps (get mode 'derived-mode--all-parents)))
    (cond
     (ps ps)
     ((memq mode known-children)
      (memq mode (reverse known-children)))
     (t
      (let* ((new-children (cons mode known-children))
             (parent (or (get mode 'derived-mode-parent)
                         (let ((alias (symbol-function mode)))
                           (and (symbolp alias) alias))))
             (extras (get mode 'derived-mode-extra-parents))
             (all-parents
              (merge-ordered-lists
               (cons (if (and parent (not (memq parent extras)))
                         (derived-mode-all-parents parent new-children))
                     (mapcar (lambda (p)
                               (derived-mode-all-parents p new-children))
                             extras)))))
        (if (and (memq mode all-parents) known-children)
            (cons mode (remq mode all-parents))
          (put mode 'derived-mode--all-parents (cons mode all-parents))))))))

(defun provided-mode-derived-p (mode &optional modes &rest old-modes)
  "Non-nil if MODE is derived from a member of MODES."
  (cond
   (old-modes (setq modes (cons modes old-modes)))
   ((not (listp modes)) (setq modes (list modes))))
  (let ((ps (derived-mode-all-parents mode)))
    (while (and modes (not (memq (car modes) ps)))
      (setq modes (cdr modes)))
    (car modes)))

(defun derived-mode-p (&optional modes &rest old-modes)
  "Return non-nil if the current major mode is derived from one of MODES."
  (provided-mode-derived-p major-mode (if old-modes (cons modes old-modes)
                                        modes)))

(defun derived-mode--flush (mode)
  (put mode 'derived-mode--all-parents nil)
  (let ((followers (get mode 'derived-mode--followers)))
    (when followers
      (put mode 'derived-mode--followers nil)
      (mapc #'derived-mode--flush followers))))

(defun derived-mode-set-parent (mode parent)
  "Declare PARENT to be the parent of MODE."
  (put mode 'derived-mode-parent parent)
  (derived-mode--flush mode))

(defun derived-mode-add-parents (mode extra-parents)
  "Add EXTRA-PARENTS to the parents of MODE."
  (put mode 'derived-mode-extra-parents extra-parents)
  (derived-mode--flush mode))

(defun modify-face (face &optional foreground background stipple bold-p
                         italic-p underline-p inverse-p frame)
  "Change the display attributes of FACE.
Obsolete: use `set-face-attribute' instead."
  (declare (obsolete set-face-attribute "22.1"))
  (unless (memq face (face-list))
    (signal 'error (list 'Invalid 'face face)))
  (when foreground
    (set-face-attribute face frame :foreground foreground))
  (when background
    (set-face-attribute face frame :background background))
  (when stipple
    (set-face-attribute face frame :stipple stipple))
  (when bold-p
    (set-face-attribute face frame :weight (if bold-p 'bold 'normal)))
  (when italic-p
    (set-face-attribute face frame :slant (if italic-p 'italic 'normal)))
  (when underline-p
    (set-face-attribute face frame :underline underline-p))
  (when inverse-p
    (set-face-attribute face frame :inverse-video inverse-p)))

(defmacro defface (face spec doc &rest args)
  "Define FACE (subset: registers the name and applies SPEC's
`default'/`t' entry attributes)."
  (declare (indent 1))
  (let* ((specv (if (and (consp spec) (eq (car spec) 'quote))
                    (cadr spec)
                  spec))
         (plist (cadr (or (assq t specv) (car specv)))))
    `(progn
       (set-face-attribute ',face nil
         ,@(mapcan (lambda (kw)
                     (let ((v (plist-get plist kw)))
                       (if v (list kw (list 'quote v)) nil)))
                   '(:foreground :background :weight :slant
                                 :underline :inverse-video
                                 :stipple :height)))
       ',face)))

(defmacro define-generic-mode (&rest args)
  "Define a generic mode (subset: aliases define-derived-mode)."
  (declare (indent 1))
  `(define-derived-mode ,(car args) fundamental-mode ,(cadr args)
                        ,(caddr args)))

;; ---------- eval-after-load plumbing is in load.rs ----------

(defmacro with-buffer-unmodified-if-unchanged (&rest body)
  (let ((doc (if (stringp (car body)) (pop body))))
    `(let ((modp (buffer-modified-p))
           (buffer-undo-list buffer-undo-list))
       (with-silent-modifications
         ,doc
         ,@body
         (restore-buffer-modified-p modp)))))

;; Character categories are implemented as bool-vector sets in
;; category tables (see `make-category-table').

(defvar text-property-default-nonsticky nil)

;;; ---- newcomment.el cluster ----
(defun comment-string-strip (str beforep afterp)
  "Strip STR of any leading (if BEFOREP) and/or trailing (if AFTERP) space."
  (string-match (concat "\\`" (if beforep "\\s-*")
			"\\(.*?\\)" (if afterp "\\s-*\n?")
			"\\'") str)
  (match-string 1 str))

(defun comment-string-reverse (s)
  "Return the mirror image of string S, without any trailing space."
  (comment-string-strip (concat (nreverse (string-to-list s))) nil t))

(defun comment-normalize-vars (&optional noerror)
  "Check and set up variables needed by other commenting functions.
All the `comment-*' commands call this function to set up various
variables, like `comment-start', to ensure that the commenting
functions work correctly.  Lisp callers of any other `comment-*'
function should first call this function explicitly."
  (funcall comment-setup-function)
  (unless (and (not comment-start) noerror)
    (unless comment-start
      (let ((cs (read-string "No comment syntax is defined.  Use: ")))
	(if (zerop (length cs))
	    (error "No comment syntax defined")
          (setq-local comment-start cs)
          (setq-local comment-start-skip cs))))
    ;; comment-use-syntax
    (when (eq comment-use-syntax 'undecided)
      (setq-local comment-use-syntax
                  (let ((st (syntax-table))
                        (cs comment-start)
                        (ce (if (string= "" comment-end) "\n" comment-end)))
                    ;; Try to skip over a comment using forward-comment
                    ;; to see if the syntax tables properly recognize it.
                    (with-temp-buffer
                      (set-syntax-table st)
                      (insert cs " hello " ce)
                      (goto-char (point-min))
                      (and (forward-comment 1) (eobp))))))
    ;; comment-padding
    (unless comment-padding (setq comment-padding 0))
    (when (integerp comment-padding)
      (setq comment-padding (make-string comment-padding ? )))
    ;; comment markers
    ;;(setq comment-start (comment-string-strip comment-start t nil))
    ;;(setq comment-end (comment-string-strip comment-end nil t))
    ;; comment-continue
    (unless (or comment-continue (string= comment-end ""))
      (setq-local comment-continue
                  (concat (if (string-match "\\S-\\S-" comment-start) " " "|")
                          (substring comment-start 1)))
      ;; Hasn't been necessary yet.
      ;; (unless (string-match comment-start-skip comment-continue)
      ;;	(kill-local-variable 'comment-continue))
      )
    ;; comment-skip regexps
    (unless (and comment-start-skip
		 ;; In case comment-start has changed since last time.
		 (string-match comment-start-skip comment-start))
      (setq-local comment-start-skip
                  (concat (unless (eq comment-use-syntax t)
                            ;; `syntax-ppss' will detect escaping.
                            "\\(\\(^\\|[^\\\n]\\)\\(\\\\\\\\\\)*\\)")
                          "\\(?:\\s<+\\|"
                          (regexp-quote (comment-string-strip comment-start t t))
                          ;; Let's not allow any \s- but only [ \t] since \n
                          ;; might be both a comment-end marker and \s-.
                          "+\\)[ \t]*")))
    (unless (and comment-end-skip
		 ;; In case comment-end has changed since last time.
		 (string-match comment-end-skip
                               (if (string= "" comment-end) "\n" comment-end)))
      (let ((ce (if (string= "" comment-end) "\n"
		  (comment-string-strip comment-end t t))))
        (setq-local comment-end-skip
                    ;; We use [ \t] rather than \s- because we don't want to
                    ;; remove ^L in C mode when uncommenting.
                    (concat "[ \t]*\\(\\s>" (if comment-quote-nested "" "+")
                            "\\|" (regexp-quote (substring ce 0 1))
                            (if (and comment-quote-nested (<= (length ce) 1)) "" "+")
                            (regexp-quote (substring ce 1))
                            "\\)"))))))

(defun comment-quote-re (str unp)
  (concat (regexp-quote (substring str 0 1))
	  "\\\\" (if unp "+" "*")
	  (regexp-quote (substring str 1))))

(defvar comment-quote-nested t
  "Non-nil if nested comments should be quoted.
This should be locally set by each major mode if needed.")

(defun comment-quote-nested (cs ce unp)
  "Quote or unquote nested comments.
If UNP is non-nil, unquote nested comment markers."
  (setq cs (comment-string-strip cs t t))
  (setq ce (comment-string-strip ce t t))
  (when (and comment-quote-nested
	     (> (length ce) 0))
    (funcall comment-quote-nested-function cs ce unp)))

(defun comment-quote-nested-default (cs ce unp)
  "Quote comment delimiters in the buffer.
It expects to be called with the buffer narrowed to a single comment.
It is used as a default for `comment-quote-nested-function'.

The arguments CS and CE are strings matching comment starting and
ending delimiters respectively.

If UNP is non-nil, comments are unquoted instead.

To quote the delimiters, a \\ is inserted after the first
character of CS or CE.  If CE is a single character it will
change CE into !CS."
  (let ((re (concat (comment-quote-re ce unp)
		    "\\|" (comment-quote-re cs unp))))
    (goto-char (point-min))
    (while (re-search-forward re nil t)
      (goto-char (match-beginning 0))
      (forward-char 1)
      (if unp (delete-char 1) (insert "\\"))
      (when (= (length ce) 1)
	;; If the comment-end is a single char, adding a \ after that
	;; "first" char won't deactivate it, so we turn such a CE
	;; into !CS.  I.e. for pascal, we turn } into !{
	(if (not unp)
	    (when (string= (match-string 0) ce)
	      (replace-match (concat "!" cs) t t))
	  (when (and (< (point-min) (match-beginning 0))
		     (string= (buffer-substring (1- (match-beginning 0))
						(1- (match-end 0)))
			      (concat "!" cs)))
	    (backward-char 2)
	    (delete-char (- (match-end 0) (match-beginning 0)))
	    (insert ce)))))))

(defun comment-search-forward (limit &optional noerror)
  "Find a comment start between point and LIMIT.
Moves point to inside the comment and returns the position of the
comment-starter.  If no comment is found, moves point to LIMIT
and raises an error or returns nil if NOERROR is non-nil.

Ensure that `comment-normalize-vars' has been called before you use this."
  (if (not comment-use-syntax)
      (if (re-search-forward comment-start-skip limit noerror)
	  (or (match-end 1) (match-beginning 0))
	(goto-char limit)
	(unless noerror (error "No comment")))
    (let* ((pt (point))
	   ;; Assume (at first) that pt is outside of any string.
	   (s (parse-partial-sexp pt (or limit (point-max)) nil nil
				  (if comment-use-global-state (syntax-ppss pt))
				  t)))
      (when (and (nth 8 s) (nth 3 s) (not comment-use-global-state))
	;; The search ended at eol inside a string.  Try to see if it
	;; works better when we assume that pt is inside a string.
	(setq s (parse-partial-sexp
		 pt (or limit (point-max)) nil nil
		 (list nil nil nil (nth 3 s) nil nil nil nil)
		 t)))
      (if (or (not (and (nth 8 s) (not (nth 3 s))))
	      ;; Make sure the comment starts after PT.
	      (< (nth 8 s) pt))
	  (unless noerror (error "No comment"))
	;; We found the comment.
	(let ((pos (point))
	      (start (nth 8 s))
	      (bol (line-beginning-position))
	      (end nil))
	  (while (and (null end) (>= (point) bol))
	    (if (looking-at comment-start-skip)
		(setq end (min (or limit (point-max)) (match-end 0)))
	      (backward-char)))
	  (goto-char (or end pos))
	  start)))))

(defun comment-search-backward (&optional limit noerror)
  "Find a comment start between LIMIT and point.
Moves point to inside the comment and returns the position of the
comment-starter.  If no comment is found, moves point to LIMIT
and raises an error or returns nil if NOERROR is non-nil.

Ensure that `comment-normalize-vars' has been called before you use this."
  ;; FIXME: If a comment-start appears inside a comment, we may erroneously
  ;; stop there.  This can be rather bad in general, but since
  ;; comment-search-backward is only used to find the comment-column (in
  ;; comment-set-column) and to find the comment-start string (via
  ;; comment-beginning) in indent-new-comment-line, it should be harmless.
  (if (not (re-search-backward comment-start-skip limit 'move))
      (unless noerror (error "No comment"))
    (beginning-of-line)
    (let* ((end (match-end 0))
	   (cs (comment-search-forward end t))
	   (pt (point)))
      (if (not cs)
	  (progn (beginning-of-line)
		 (comment-search-backward limit noerror))
	(while (progn (goto-char cs)
		      (comment-forward)
		      (and (< (point) end)
			   (setq cs (comment-search-forward end t))))
	  (setq pt (point)))
	(goto-char pt)
	cs))))

(defun comment-beginning ()
  "Find the beginning of the enclosing comment.
Returns nil if not inside a comment, else moves point and returns
the same as `comment-search-backward'."
  (if (and comment-use-syntax comment-use-global-state)
      (let ((state (syntax-ppss)))
        (when (nth 4 state)
          (goto-char (nth 8 state))
          (prog1 (point)
            (when (save-restriction
                    ;; `comment-start-skip' sometimes checks that the
                    ;; comment char is not escaped.  (Bug#16971)
                    (narrow-to-region (point) (point-max))
                    (looking-at comment-start-skip))
              (goto-char (match-end 0))))))
    ;; Can't rely on the syntax table, let's guess based on font-lock.
    (unless (eq (get-text-property (point) 'face) 'font-lock-string-face)
      (let ((pt (point))
            (cs (comment-search-backward nil t)))
        (when cs
          (if (save-excursion
                (goto-char cs)
                (and
                 ;; For modes where comment-start and comment-end are the same,
                 ;; the search above may have found a `ce' rather than a `cs'.
                 (or (if comment-end-skip (not (looking-at comment-end-skip)))
                     ;; Maybe font-lock knows that it's a `cs'?
                     (eq (get-text-property (match-end 0) 'face)
                         'font-lock-comment-face)
                     (unless (eq (get-text-property (point) 'face)
                                 'font-lock-comment-face)
                       ;; Let's assume it's a `cs' if we're on the same line.
                       (>= (line-end-position) pt)))
                 ;; Make sure that PT is not past the end of the comment.
                 (if (comment-forward 1) (> (point) pt) (eobp))))
              cs
            (goto-char pt)
            nil))))))

(defun comment-forward (&optional n)
  "Skip forward over N comments.
Just like `forward-comment' but only for positive N
and can use regexps instead of syntax."
  (setq n (or n 1))
  (if (< n 0) (error "No comment-backward")
    (if comment-use-syntax (forward-comment n)
      (while (> n 0)
	(setq n
	      (if (or (forward-comment 1)
		      (and (looking-at comment-start-skip)
			   (goto-char (match-end 0))
			   (re-search-forward comment-end-skip nil 'move)))
		  (1- n) -1)))
      (= n 0))))

(defun comment-enter-backward ()
  "Move from the end of a comment to the end of its content.
Point is assumed to be just at the end of a comment."
  (if (bolp)
      ;; comment-end = ""
      (progn (backward-char) (skip-syntax-backward " "))
    (cond
     ((save-excursion
        (save-restriction
          (narrow-to-region (line-beginning-position) (point))
          (goto-char (point-min))
          (re-search-forward (concat comment-end-skip "\\'") nil t)))
      (goto-char (match-beginning 0)))
     ;; comment-end-skip not found probably because it was not set
     ;; right.  Since \\s> should catch the single-char case, let's
     ;; check that we're looking at a two-char comment ender.
     ((not (or (<= (- (point-max) (line-beginning-position)) 1)
               (zerop (logand (car (syntax-after (- (point) 1)))
                              ;; Here we take advantage of the fact that
                              ;; the syntax class " " is encoded to 0,
                              ;; so "  4" gives us just the 4 bit.
                              (car (string-to-syntax "  4"))))
               (zerop (logand (car (syntax-after (- (point) 2)))
                              (car (string-to-syntax "  3"))))))
      (backward-char 2)
      (skip-chars-backward (string (char-after)))
      (skip-syntax-backward " "))
     ;; No clue what's going on: maybe we're really not right after the
     ;; end of a comment.  Maybe we're at the "end" because of EOB rather
     ;; than because of a marker.
     (t (skip-syntax-backward " ")))))

(defun comment-indent-default ()
  "Default for `comment-indent-function'."
  (if (and (looking-at "\\s<\\s<\\(\\s<\\)?")
	   (or (match-end 1) (/= (current-column) (current-indentation))))
      0
    (when (or (/= (current-column) (current-indentation))
	      (and (> comment-add 0) (looking-at "\\s<\\(\\S<\\|\\'\\)")))
      comment-column)))

(defun comment-choose-indent (&optional indent)
  "Choose the indentation to use for a right-hand-side comment.
The criteria are (in this order):
- try to keep the comment's text within `comment-fill-column'.
- try to align with surrounding comments.
- prefer INDENT (or `comment-column' if nil).
Point is expected to be at the start of the comment."
  (unless indent (setq indent comment-column))
  (let ((other nil)
        min max)
    (if (consp indent)
        (progn (setq min (car indent)) (setq max (cdr indent))
               (setq indent comment-column))
      ;; Avoid moving comments past the fill-column.
      (setq max (+ (current-column)
                   (- (or comment-fill-column fill-column)
                      (save-excursion (end-of-line) (current-column)))))
      (setq min (save-excursion
                  (skip-chars-backward " \t")
                  ;; Leave at least `comment-inline-offset' space after
                  ;; other nonwhite text on the line.
                  (if (bolp) 0 (+ comment-inline-offset (current-column))))))
    ;; Fix up the range.
    (if (< max min) (setq max min))
    ;; Don't move past the fill column.
    (if (<= max indent) (setq indent max))
    ;; We can choose anywhere between min..max.
    ;; Let's try to align to a comment on the previous line.
    (save-excursion
      (when (and (zerop (forward-line -1))
                 (setq other (comment-search-forward
                              (line-end-position) t)))
        (goto-char other) (setq other (current-column))))
    (if (and other (<= other max) (>= other min))
        ;; There is a comment and it's in the range: bingo!
        other
      ;; Can't align to a previous comment: let's try to align to comments
      ;; on the following lines, then.  These have not been re-indented yet,
      ;; so we can't directly align ourselves with them.  All we do is to try
      ;; and choose an indentation point with which they will be able to
      ;; align themselves.
      (save-excursion
        (while (and (zerop (forward-line 1))
                    (setq other (comment-search-forward
                                 (line-end-position) t)))
          (goto-char other)
          (let ((omax (+ (current-column)
                         (- (or comment-fill-column fill-column)
                            (save-excursion (end-of-line) (current-column)))))
                (omin (save-excursion (skip-chars-backward " \t")
                                      (1+ (current-column)))))
            (if (and (>= omax min) (<= omin max))
                (progn (setq min (max omin min))
                       (setq max (min omax max)))
              ;; Can't align with this anyway, so exit the loop.
              (goto-char (point-max))))))
      ;; Return the closest point to indent within min..max.
      (max min (min max indent)))))

(defun comment-indent (&optional continue)
  "Indent this line's comment to `comment-column', or insert an empty comment.
If CONTINUE is non-nil, use the `comment-continue' markers if any."
  (interactive "*")
  (comment-normalize-vars)
  (beginning-of-line)
  (let ((starter (or (and continue comment-continue)
                     comment-start
                     (error "No comment syntax defined")))
	(ender (or (and continue comment-continue "")
                   comment-end))
	(begpos (comment-search-forward (line-end-position) t))
	cpos indent)
    (cond
     ;; If we couldn't find a comment *starting* on this line, see if we
     ;; are already within a multiline comment at BOL (bug#78003).
     ((and (not begpos) (not continue)
           comment-use-syntax comment-use-global-state
           (save-excursion (nth 4 (syntax-ppss (line-beginning-position)))))
      ;; We don't know anything about the nature of the multiline
      ;; construct, so immediately delegate to the mode.
      (indent-according-to-mode))
     ((and (not begpos) comment-insert-comment-function)
      ;; If no comment and c-i-c-f is set, let it do everything.
      (funcall comment-insert-comment-function))
     (t
      ;; An existing comment?
      (if begpos
	  (progn
	    (if (and (not (looking-at "[\t\n ]"))
		     (looking-at comment-end-skip))
		;; The comment is empty and we have skipped all its space
		;; and landed right before the comment-ender:
		;; Go back to the middle of the space.
		(forward-char (/ (skip-chars-backward " \t") -2)))
	    (setq cpos (point-marker)))
	;; If none, insert one.
	(save-excursion
	  ;; Some `comment-indent-function's insist on not moving
	  ;; comments that are in column 0, so we first go to the
	  ;; likely target column.
	  (indent-to comment-column)
	  ;; Ensure there's a space before the comment for things
	  ;; like sh where it matters (as well as being neater).
	  (unless (memq (char-before) '(nil ?\n ?\t ?\s))
	    (insert ?\s))
	  (setq begpos (point))
	  (insert starter)
	  (setq cpos (point-marker))
	  (insert ender)))
      (goto-char begpos)
      ;; Compute desired indent.
      (setq indent (save-excursion (funcall comment-indent-function)))
      ;; If `indent' is nil and there's code before the comment, we can't
      ;; use `indent-according-to-mode', so we default to comment-column.
      (unless (or indent (save-excursion (skip-chars-backward " \t") (bolp)))
	(setq indent comment-column))
      (if (not indent)
	  ;; comment-indent-function refuses: delegate to line-indent.
	  (indent-according-to-mode)
	;; If the comment is at the right of code, adjust the indentation.
	(unless (save-excursion (skip-chars-backward " \t") (bolp))
	  (setq indent (comment-choose-indent indent)))
	;; If that's different from comment's current position, change it.
	(unless (= (current-column) indent)
	  (delete-region (point) (progn (skip-chars-backward " \t") (point)))
	  (indent-to indent)))
      (goto-char cpos)
      (set-marker cpos nil)))))

(defun comment-set-column (arg)
  "Set the comment column based on point.
With no ARG, set the comment column to the current column.
With just minus as arg, kill any comment on this line.
With any other arg, set comment column to indentation of the previous comment
 and then align or create a comment on this line at that column."
  (interactive "P")
  (cond
   ((eq arg '-) (comment-kill nil))
   (arg
    (comment-normalize-vars)
    (save-excursion
      (beginning-of-line)
      (comment-search-backward)
      (beginning-of-line)
      (goto-char (comment-search-forward (line-end-position)))
      (setq comment-column (current-column))
      (message "Comment column set to %d" comment-column))
    (comment-indent))
   (t (setq comment-column (current-column))
      (message "Comment column set to %d" comment-column))))

(defun comment-kill (arg)
  "Kill the first comment on this line, if any.
With prefix ARG, kill comments on that many lines starting with this one."
  (interactive "P")
  (comment-normalize-vars)
  (dotimes (_i (prefix-numeric-value arg))
    (save-excursion
      (beginning-of-line)
      (let ((cs (comment-search-forward (line-end-position) t)))
	(when cs
	  (goto-char cs)
	  (skip-syntax-backward " ")
	  (setq cs (point))
	  (comment-forward)
	  (kill-region cs (if (bolp) (1- (point)) (point)))
	  (indent-according-to-mode))))
    (if arg (forward-line 1))))

(defun comment-padright (str &optional n)
  "Construct a string composed of STR plus `comment-padding'.
It also adds N copies of the last non-whitespace chars of STR.
If STR already contains padding, the corresponding amount is
ignored from `comment-padding'.
N defaults to 0.
If N is `re', a regexp is returned instead, that would match
the string for any N.

Ensure that `comment-normalize-vars' has been called before you use this."
  (setq n (or n 0))
  (when (and (stringp str) (string-match "\\S-" str))
    ;; Separate the actual string from any leading/trailing padding
    (string-match "\\`\\s-*\\(.*?\\)\\s-*\\'" str)
    (let ((s (match-string 1 str))                     ;actual string
	  (lpad (substring str 0 (match-beginning 1))) ;left padding
	  (rpad (concat
                 (substring str (match-end 1)) ;original right padding
                 (if (numberp comment-padding)
                     (make-string (min comment-padding
                                       (- (match-end 0) (match-end 1)))
                                  ?\s)
                   (if (not (string-match-p "\\`\\s-" comment-padding))
                       ;; If the padding isn't spaces, then don't
                       ;; shorten the padding.
                       comment-padding
		     (substring comment-padding ;additional right padding
			        (min (- (match-end 0) (match-end 1))
				     (length comment-padding)))))))
	  ;; We can only duplicate C if the comment-end has multiple chars
	  ;; or if comments can be nested, else the comment-end `}' would
	  ;; be turned into `}}}' where only the first ends the comment
	  ;; and the rest becomes bogus junk.
	  (multi (not (and comment-quote-nested
			   ;; comment-end is a single char
			   (string-match "\\`\\s-*\\S-\\s-*\\'" comment-end)))))
      (if (not (symbolp n))
	  (concat lpad s (when multi (make-string n (aref str (1- (match-end 1))))) rpad)
	;; construct a regexp that would match anything from just S
	;; to any possible output of this function for any N.
	(concat (mapconcat (lambda (c) (concat (regexp-quote (string c)) "?"))
			   lpad "")	;padding is not required
		(regexp-quote s)
		(when multi "+") ;the last char of S might be repeated
		(mapconcat (lambda (c) (concat (regexp-quote (string c)) "?"))
			   rpad ""))))))

(defun comment-padleft (str &optional n)
  "Construct a string composed of `comment-padding' plus STR.
It also adds N copies of the first non-whitespace chars of STR.
If STR already contains padding, the corresponding amount is
ignored from `comment-padding'.
N defaults to 0.
If N is `re', a regexp is returned instead, that would match the
string for any N.

Ensure that `comment-normalize-vars' has been called before you use this."
  (setq n (or n 0))
  (when (and (stringp str) (not (string= "" str)))
    ;; Only separate the left pad because we assume there is no right pad.
    (string-match "\\`\\s-*" str)
    (let ((s (substring str (match-end 0)))
	  (pad (concat (if (not (string-match-p "\\`\\s-" comment-padding))
                           ;; If the padding isn't spaces, then don't
                           ;; shorten the padding.
                           comment-padding
                         (substring comment-padding
				    (min (- (match-end 0) (match-beginning 0))
				         (length comment-padding))))
		       (match-string 0 str)))
	  (c (aref str (match-end 0)))	;the first non-space char of STR
	  ;; We can only duplicate C if the comment-end has multiple chars
	  ;; or if comments can be nested, else the comment-end `}' would
	  ;; be turned into `}}}' where only the first ends the comment
	  ;; and the rest becomes bogus junk.
	  (multi (not (and comment-quote-nested
			   ;; comment-end is a single char
			   (string-match "\\`\\s-*\\S-\\s-*\\'" comment-end)))))
      (if (not (symbolp n))
	  (concat pad (when multi (make-string n c)) s)
	;; Construct a regexp that would match anything from just S
	;; to any possible output of this function for any N.
	;; We match any number of leading spaces because this regexp will
	;; be used for uncommenting where we might want to remove
	;; uncomment markers with arbitrary leading space (because
	;; they were aligned).
	(concat "\\s-*"
		(if multi (concat (regexp-quote (string c)) "*"))
		(regexp-quote s))))))

(defun uncomment-region (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment delimiters."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (when (> beg end) (setq beg (prog1 end (setq end beg))))
  ;; Bind `comment-use-global-state' to nil.  While uncommenting a region
  ;; (which works a line at a time), a comment can appear to be
  ;; included in a multi-line string, but it is actually not.
  (let ((comment-use-global-state nil))
    (save-excursion
      (funcall uncomment-region-function beg end arg))))

(defun uncomment-region-default-1 (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment delimiters.
This function is the default value of `uncomment-region-function'."
  (goto-char beg)
  (setq end (copy-marker end))
  (let* ((numarg (prefix-numeric-value arg))
	 (ccs comment-continue)
	 (srei (or (comment-padright ccs 're)
		   (and (stringp comment-continue) comment-continue)))
	 (csre (comment-padright comment-start 're))
	 (sre (and srei (concat "^\\s-*?\\(" srei "\\)")))
	 spt)
    (while (and (< (point) end)
		(setq spt (comment-search-forward end t)))
      (let ((ipt (point))
	    ;; Find the end of the comment.
	    (ept (progn
		   (goto-char spt)
		   (unless (or (comment-forward)
			       ;; Allow non-terminated comments.
			       (eobp))
		     (error "Can't find the comment end"))
		   (point)))
	    (box nil)
	    (box-equal nil))	   ;Whether we might be using `=' for boxes.
	(save-restriction
	  (narrow-to-region spt ept)

	  ;; Remove the comment-start.
	  (goto-char ipt)
	  (skip-syntax-backward " ")
	  ;; A box-comment starts with a looong comment-start marker.
	  (when (and (or (and (= (- (point) (point-min)) 1)
			      (setq box-equal t)
			      (looking-at "=\\{7\\}")
			      (not (eq (char-before (point-max)) ?\n))
			      (skip-chars-forward "="))
			 (> (- (point) (point-min) (length comment-start)) 7))
		     (> (count-lines (point-min) (point-max)) 2))
	    (setq box t))
	  ;; Skip the padding.  Padding can come from comment-padding and/or
	  ;; from comment-start, so we first check comment-start.
	  (if (or (save-excursion (goto-char (point-min)) (looking-at csre))
		  (looking-at (regexp-quote comment-padding)))
	      (goto-char (match-end 0)))
	  (when (and sre (looking-at (concat "\\s-*\n\\s-*" srei)))
	    (goto-char (match-end 0)))
	  (if (null arg) (delete-region (point-min) (point))
            (let ((opoint (point-marker)))
              (skip-syntax-backward " ")
              (delete-char (- numarg))
              (unless (and (not (bobp))
                           (save-excursion (goto-char (point-min))
                                           (looking-at comment-start-skip)))
                ;; If there's something left but it doesn't look like
                ;; a comment-start any more, just remove it.
                (delete-region (point-min) opoint))))

	  ;; Remove the end-comment (and leading padding and such).
	  (goto-char (point-max)) (comment-enter-backward)
	  ;; Check for special `=' used sometimes in comment-box.
	  (when (and box-equal (not (eq (char-before (point-max)) ?\n)))
	    (let ((pos (point)))
	      ;; skip `=' but only if there are at least 7.
	      (when (> (skip-chars-backward "=") -7) (goto-char pos))))
	  (unless (looking-at "\\(\n\\|\\s-\\)*\\'")
	    (when (and (bolp) (not (bobp))) (backward-char))
	    (if (null arg) (delete-region (point) (point-max))
	      (skip-syntax-forward " ")
	      (delete-char numarg)
	      (unless (or (eobp) (looking-at comment-end-skip))
		;; If there's something left but it doesn't look like
		;; a comment-end any more, just remove it.
		(delete-region (point) (point-max)))))

	  ;; Unquote any nested end-comment.
	  (comment-quote-nested comment-start comment-end t)

	  ;; Eliminate continuation markers as well.
	  (when sre
	    (let* ((cce (comment-string-reverse (or comment-continue
						    comment-start)))
		   (erei (and box (comment-padleft cce 're)))
		   (ere (and erei (concat "\\(" erei "\\)\\s-*$"))))
	      (goto-char (point-min))
	      (while (progn
		       (if (and ere (re-search-forward
				     ere (line-end-position) t))
			   (replace-match "" t t nil (if (match-end 2) 2 1))
			 (setq ere nil))
		       (forward-line 1)
		       (re-search-forward sre (line-end-position) t))
		(replace-match "" t t nil (if (match-end 2) 2 1)))))
	  ;; Go to the end for the next comment.
	  (goto-char (point-max)))
        ;; Remove any obtrusive spaces left preceding a tab at `spt'.
        (when (and (eq (char-after spt) ?\t) (eq (char-before spt) ? )
                   (> tab-width 0))
          (save-excursion
            (goto-char spt)
            (let* ((fcol (current-column))
                   (slim (- (point) (mod fcol tab-width))))
              (delete-char (- (skip-chars-backward " " slim)))))))))
  (set-marker end nil))

(defun uncomment-region-default (beg end &optional arg)
  "Uncomment each line in the BEG .. END region.
The numeric prefix ARG can specify a number of chars to remove from the
comment markers."
  (if comment-combine-change-calls
      (combine-change-calls beg end (uncomment-region-default-1 beg end arg))
    (uncomment-region-default-1 beg end arg)))

(defun comment-make-bol-ws (len)
  "Make a white-space string of width LEN for use at BOL.
When `indent-tabs-mode' is non-nil, tab characters will be used."
  (if (and indent-tabs-mode (> tab-width 0))
      (concat (make-string (/ len tab-width) ?\t)
	      (make-string (% len tab-width) ? ))
    (make-string len ? )))

(defun comment-make-extra-lines (cs ce ccs cce min-indent max-indent &optional block)
  "Make the leading and trailing extra lines.
This is used for `extra-line' style (or `box' style if BLOCK is specified)."
  (let ((eindent 0))
    (if (not block)
	;; Try to match CS and CE's content so they align aesthetically.
	(progn
	  (setq ce (comment-string-strip ce t t))
	  (when (string-match "\\(.+\\).*\n\\(.*?\\)\\1" (concat ce "\n" cs))
	    (setq eindent
		  (max (- (match-end 2) (match-beginning 2) (match-beginning 0))
		       0))))
      ;; box comment
      (let* ((width (- max-indent min-indent))
	     (s (concat cs "a=m" cce))
	     (e (concat ccs "a=m" ce))
	     (c (if (string-match ".*\\S-\\S-" cs)
		    (aref cs (1- (match-end 0)))
		  (if (and (equal comment-end "") (string-match ".*\\S-" cs))
		      (aref cs (1- (match-end 0))) ?=)))
	     (re "\\s-*a=m\\s-*")
	     (_ (string-match re s))
	     (lcs (length cs))
	     (fill
	      (make-string (+ width (- (match-end 0)
				       (match-beginning 0) lcs 3)) c)))
	(setq cs (replace-match fill t t s))
	(when (and (not (string-match comment-start-skip cs))
		   (string-match "a=m" s))
	  ;; The whitespace around CS cannot be ignored: put it back.
	  (setq re "a=m")
	  (setq fill (make-string (- width lcs) c))
	  (setq cs (replace-match fill t t s)))
	(string-match re e)
	(setq ce (replace-match fill t t e))))
    (cons (concat cs "\n" (comment-make-bol-ws min-indent) ccs)
	  (concat cce "\n" (comment-make-bol-ws (+ min-indent eindent)) ce))))

(defmacro comment-with-narrowing (beg end &rest body)
  "Execute BODY with BEG..END narrowing.
Space is added (and then removed) at the beginning for the text's
indentation to be kept as it was before narrowing."
  (declare (debug t) (indent 2))
  (let ((bindent (make-symbol "bindent")))
    `(let ((,bindent (save-excursion (goto-char ,beg) (current-column))))
       (save-restriction
	 (narrow-to-region ,beg ,end)
	 (goto-char (point-min))
	 (insert (make-string ,bindent ? ))
	 (prog1
	     (progn ,@body)
	   ;; remove the bindent
	   (save-excursion
	     (goto-char (point-min))
	     (when (looking-at " *")
	       (let ((n (min (- (match-end 0) (match-beginning 0)) ,bindent)))
		 (delete-char n)
		 (setq ,bindent (- ,bindent n))))
	     (end-of-line)
	     (let ((e (point)))
	       (beginning-of-line)
	       (while (and (> ,bindent 0) (re-search-forward "   *" e t))
		 (let ((n (min ,bindent (- (match-end 0) (match-beginning 0) 1))))
		   (goto-char (match-beginning 0))
		   (delete-char n)
		   (setq ,bindent (- ,bindent n)))))))))))

(defvar comment-add 0
  "How many more comment chars should be inserted by `comment-region'.
This determines the default value of the numeric argument of `comment-region'.
The `plain' comment style doubles this value.

This should generally stay 0, except for a few modes like Lisp where
it is 1 so that regions are commented with two or three semi-colons.")

(defun comment-add (arg)
  "Compute the number of extra comment starter characters.
\(Extra semicolons in Lisp mode, extra stars in C mode, etc.)
If ARG is non-nil, just follow ARG.
If the comment starter is multi-char, just follow ARG.
Otherwise obey `comment-add'."
  (if (and (null arg) (= (string-match "[ \t]*\\'" comment-start) 1))
      (* comment-add 1)
    (1- (prefix-numeric-value arg))))

(defun comment-region-internal (beg end cs ce
                                &optional ccs cce block lines indent)
  "Comment region BEG .. END.
CS and CE are the comment start string and comment end string,
respectively.  CCS and CCE are the comment continuation strings
for the start and end of lines, respectively (default to CS and CE).
BLOCK indicates that end of lines should be marked with either CCE,
CE or CS \(if CE is empty) and that those markers should be aligned.
LINES indicates that an extra lines will be used at the beginning
and end of the region for CE and CS.
INDENT indicates to put CS and CCS at the current indentation of
the region rather than at left margin."
  ;;(assert (< beg end))
  (let ((no-empty (not (or (eq comment-empty-lines t)
			   (and comment-empty-lines (zerop (length ce))))))
	ce-sanitized)
    ;; Sanitize CE and CCE.
    (if (and (stringp ce) (string= "" ce)) (setq ce nil))
    (setq ce-sanitized ce)
    (if (and (stringp cce) (string= "" cce)) (setq cce nil))
    ;; If CE is empty, multiline cannot be used.
    (unless ce (setq ccs nil cce nil))
    ;; Should we mark empty lines as well ?
    (if (or ccs block lines) (setq no-empty nil))
    ;; Make sure we have end-markers for BLOCK mode.
    (when block (unless ce (setq ce (comment-string-reverse cs))))
    ;; If BLOCK is not requested, we don't need CCE.
    (unless block (setq cce nil))
    ;; Continuation defaults to the same as CS and CE.
    (unless ccs (setq ccs cs cce ce))

    (save-excursion
      (goto-char end)
      ;; If the end is not at the end of a line and the comment-end
      ;; is implicit (i.e. a newline), explicitly insert a newline.
      (unless (or ce-sanitized (eolp)) (insert "\n") (indent-according-to-mode))
      (comment-with-narrowing beg end
	(let ((min-indent (point-max))
	      (max-indent 0))
	  (goto-char (point-min))
	  ;; Quote any nested comment marker
	  (comment-quote-nested comment-start comment-end nil)

	  ;; Loop over all lines to find the needed indentations.
	  (goto-char (point-min))
	  (while
	      (progn
		(unless (looking-at "[ \t]*$")
		  (setq min-indent (min min-indent (current-indentation))))
		(end-of-line)
		(setq max-indent (max max-indent (current-column)))
		(not (or (eobp) (progn (forward-line) nil)))))

	  (setq max-indent
		(+ max-indent (max (length cs) (length ccs))
                   ;; Inserting ccs can change max-indent by (1- tab-width)
                   ;; but only if there are TABs in the boxed text, of course.
                   (if (save-excursion (goto-char beg)
                                       (search-forward "\t" end t))
                       (1- tab-width) 0)))
	  (unless indent (setq min-indent 0))

	  ;; make the leading and trailing lines if requested
	  (when lines
            ;; Trim trailing whitespace from cs if there's some.
            (setq cs (string-trim-right cs))

	    (let ((csce
		   (comment-make-extra-lines
		    cs ce ccs cce min-indent max-indent block)))
	      (setq cs (car csce))
	      (setq ce (cdr csce))))

	  (goto-char (point-min))
	  ;; Loop over all lines from BEG to END.
	  (while
	      (progn
		(unless (and no-empty (looking-at "[ \t]*$"))
		  (move-to-column min-indent t)
		  (insert cs) (setq cs ccs) ;switch to CCS after the first line
		  (end-of-line)
		  (if (eobp) (setq cce ce))
		  (when cce
		    (when block (move-to-column max-indent t))
		    (insert cce)))
		(end-of-line)
		(not (or (eobp) (progn (forward-line) nil))))))))))

(defun comment-region (beg end &optional arg)
  "Comment or uncomment each line in the region.
With just \\[universal-argument] prefix arg, uncomment each line in region BEG .. END.
Numeric prefix ARG means use ARG comment characters.
If ARG is negative, delete that many comment characters instead.

The strings used as comment starts are built from `comment-start'
and `comment-padding'; the strings used as comment ends are built
from `comment-end' and `comment-padding'.

By default, the `comment-start' markers are inserted at the
current indentation of the region, and comments are terminated on
each line (even for syntaxes in which newline does not end the
comment and blank lines do not get comments).  This can be
changed with `comment-style'."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (if (> beg end) (let (mid) (setq mid beg beg end end mid)))
  (save-excursion
    ;; FIXME: maybe we should call uncomment depending on ARG.
    (funcall comment-region-function beg end arg)))

(defun comment-region-default-1 (beg end &optional arg)
  (let* ((numarg (prefix-numeric-value arg))
	 (style (cdr (assoc comment-style comment-styles)))
	 (lines (nth 2 style))
	 (block (nth 1 style))
	 (multi (nth 0 style)))

    ;; We use `chars' instead of `syntax' because `\n' might be
    ;; of end-comment syntax rather than of whitespace syntax.
    ;; sanitize BEG and END
    (goto-char beg) (skip-chars-forward " \t\n\r") (beginning-of-line)
    (setq beg (max beg (point)))
    (goto-char end) (skip-chars-backward " \t\n\r") (end-of-line)
    (setq end (min end (point)))
    (if (>= beg end) (error "Nothing to comment"))

    ;; sanitize LINES
    (setq lines
	  (and
	   lines ;; multi
	   (progn (goto-char beg) (beginning-of-line)
		  (skip-syntax-forward " ")
		  (>= (point) beg))
	   (progn (goto-char end) (end-of-line) (skip-syntax-backward " ")
		  (<= (point) end))
	   (or block (not (string= "" comment-end)))
           (or block (progn (goto-char beg) (re-search-forward "$" end t)))))

    ;; don't add end-markers just because the user asked for `block'
    (unless (or lines (string= "" comment-end)) (setq block nil))

    (cond
     ((consp arg) (uncomment-region beg end))
     ((< numarg 0) (uncomment-region beg end (- numarg)))
     (t
      (let ((multi-char (/= (string-match "[ \t]*\\'" comment-start) 1))
	    indent triple)
	(if (eq (nth 3 style) 'multi-char)
	    (save-excursion
	      (goto-char beg)
	      (setq indent multi-char
		    ;; Triple if we will put the comment starter at the margin
		    ;; and the first line of the region isn't indented
		    ;; at least two spaces.
		    triple (and (not multi-char) (looking-at "\t\\|  "))))
	  (setq indent (nth 3 style)))

	;; In Lisp and similar modes with one-character comment starters,
	;; double it by default if `comment-add' says so.
	;; If it isn't indented, triple it.
	(if (and (null arg) (not multi-char))
	    (setq numarg (* comment-add (if triple 2 1)))
	  (setq numarg (1- (prefix-numeric-value arg))))

	(comment-region-internal
	 beg end
	 (let ((s (comment-padright comment-start numarg)))
	   (if (string-match comment-start-skip s) s
	     (comment-padright comment-start)))
	 (let ((s (comment-padleft comment-end numarg)))
	   (and s (if (string-match comment-end-skip s) s
		    (comment-padright comment-end))))
	 (if multi
             (or (comment-padright comment-continue numarg)
                 ;; `comment-padright' returns nil when
                 ;; `comment-continue' contains only whitespace
                 (and (stringp comment-continue) comment-continue)))
	 (if multi
	     (comment-padleft (comment-string-reverse comment-continue) numarg))
	 block
	 lines
	 indent))))))

(defun comment-region-default (beg end &optional arg)
  (if comment-combine-change-calls
      (combine-change-calls beg
          ;; A new line might get inserted and whitespace deleted
          ;; after END for line comments.  Ensure the next argument is
          ;; after any and all changes.
          (save-excursion
            (goto-char end)
            (forward-line)
            (point))
        (comment-region-default-1 beg end arg))
    (comment-region-default-1 beg end arg)))

(defun comment-box (beg end &optional arg)
  "Comment out the BEG .. END region, putting it inside a box.
The numeric prefix ARG specifies how many characters to add to begin- and
end- comment markers additionally to what variable `comment-add' already
specifies."
  (interactive "*r\np")
  (comment-normalize-vars)
  (let ((comment-style (if (cadr (assoc comment-style comment-styles))
			   'box-multi 'box)))
    (comment-region beg end (+ comment-add arg))))

(defun comment-only-p (beg end)
  "Return non-nil if the text between BEG and END is all comments."
  (save-excursion
    (goto-char beg)
    (comment-forward (point-max))
    (<= end (point))))

(defun comment-or-uncomment-region (beg end &optional arg)
  "Call `comment-region', unless the region only consists of comments,
in which case call `uncomment-region'.  If a prefix arg is given, it
is passed on to the respective function."
  (interactive "*r\nP")
  (comment-normalize-vars)
  (funcall (if (comment-only-p beg end)
	       'uncomment-region 'comment-region)
	   beg end arg))

(defun comment-dwim (arg)
  "Call the comment command you want (Do What I Mean).
If the region is active and `transient-mark-mode' is on, call
`comment-region' (unless it only consists of comments, in which
case it calls `uncomment-region'); in this case, prefix numeric
argument ARG specifies how many characters to remove from each
comment delimiter (so don't specify a prefix argument whose value
is greater than the total length of the comment delimiters).
Else, if the current line is empty, call `comment-insert-comment-function'
if it is defined, otherwise insert a comment and indent it.
Else, if a prefix ARG is specified, call `comment-kill'; in this
case, prefix numeric argument ARG specifies on how many lines to kill
the comments.
Else, call `comment-indent'.
You can configure `comment-style' to change the way regions are commented."
  (interactive "*P")
  (comment-normalize-vars)
  (if (use-region-p)
      (comment-or-uncomment-region (region-beginning) (region-end) arg)
    (if (save-excursion (beginning-of-line) (not (looking-at "\\s-*$")))
	;; FIXME: If there's no comment to kill on this line and ARG is
	;; specified, calling comment-kill is not very clever.
	(if arg (comment-kill (and (integerp arg) arg)) (comment-indent))
      ;; Inserting a comment on a blank line. comment-indent calls
      ;; c-i-c-f if needed in the non-blank case.
      (if comment-insert-comment-function
          (funcall comment-insert-comment-function)
        (let ((add (comment-add arg)))
          ;; Some modes insist on keeping column 0 comment in column 0
          ;; so we need to move away from it before inserting the comment.
          (indent-according-to-mode)
          (insert (comment-padright comment-start add))
          (save-excursion
            (unless (string= "" comment-end)
              (insert (comment-padleft comment-end add)))
            (indent-according-to-mode)))))))

(defun comment-valid-prefix-p (prefix compos)
    "Check that the adaptive fill prefix is consistent with the context.
PREFIX is the prefix (presumably guessed by `adaptive-fill-mode').
COMPOS is the position of the beginning of the comment we're in, or nil
if we're not inside a comment."
  ;; This consistency checking is mostly needed to workaround the limitation
  ;; of auto-fill-mode whose paragraph-determination doesn't pay attention
  ;; to comment boundaries.
  (if (null compos)
      ;; We're not inside a comment: the prefix shouldn't match
      ;; a comment-starter.
      (not (and comment-start comment-start-skip
                (string-match comment-start-skip prefix)))
    (or
     ;; Accept any prefix if the current comment is not EOL-terminated.
     (save-excursion (goto-char compos) (comment-forward) (not (bolp)))
     ;; Accept any prefix that starts with the same comment-start marker
     ;; as the current one.
     (when (string-match (concat "\\`[ \t]*\\(?:" comment-start-skip "\\)")
                         prefix)
       (let ((prefix-com (comment-string-strip (match-string 0 prefix) nil t)))
         (string-match "\\`[ \t]*" prefix-com)
         (let* ((prefix-space (match-string 0 prefix-com))
                (prefix-indent (string-width prefix-space))
                (prefix-comstart (substring prefix-com (match-end 0))))
           (save-excursion
             (goto-char compos)
             ;; The comstart marker is the same.
             (and (looking-at (regexp-quote prefix-comstart))
                  ;; The indentation as well.
                  (or (= prefix-indent
                         (- (current-column) (current-left-margin)))
                      ;; Check the indentation in two different ways, just
                      ;; to try and avoid most of the potential funny cases.
                      (equal prefix-space
                             (buffer-substring (point)
                                               (progn (move-to-left-margin)
                                                      (point)))))))))))))

(defun comment-indent-new-line (&optional soft)
  "Break line at point and indent, continuing comment if within one.
This indents the body of the continued comment
under the previous comment line.

This command is intended for styles where you write a comment per line,
starting a new comment (and terminating it if necessary) on each line.
If you want to continue one comment across several lines, use \\[newline-and-indent].

If a fill column is specified, it overrides the use of the comment column
or comment indentation.

The inserted newline is marked hard if variable `use-hard-newlines' is true,
unless optional argument SOFT is non-nil."
  (interactive)
  (comment-normalize-vars t)
  (let (compos comin)
    ;; If we are not inside a comment and we only auto-fill comments,
    ;; don't do anything (unless no comment syntax is defined).
    (unless (and comment-start
		 comment-auto-fill-only-comments
		 (not (called-interactively-p 'interactive))
		 (not (save-excursion
			(prog1 (setq compos (comment-beginning))
			  (setq comin (point))))))

      ;; Now we know we should auto-fill.
      ;; Insert the newline before removing empty space so that markers
      ;; get preserved better.
      (if soft (insert-and-inherit ?\n) (newline 1))
      (save-excursion (forward-char -1) (delete-horizontal-space))
      (delete-horizontal-space)

      (if (and fill-prefix (not adaptive-fill-mode))
	  ;; Blindly trust a non-adaptive fill-prefix.
	  (progn
	    (indent-to-left-margin)
	    (insert-before-markers-and-inherit fill-prefix))

	;; If necessary check whether we're inside a comment.
	(unless (or compos (null comment-start))
	  (save-excursion
	    (backward-char)
	    (setq compos (comment-beginning))
	    (setq comin (point))))

	(cond
	 ;; If there's an adaptive prefix, use it unless we're inside
	 ;; a comment and the prefix is not a comment starter.
	 ((and fill-prefix
               (comment-valid-prefix-p fill-prefix compos))
	  (indent-to-left-margin)
	  (insert-and-inherit fill-prefix))
	 ;; If we're not inside a comment, just try to indent.
	 ((not compos) (indent-according-to-mode))
	 (t
	  (let* ((comstart (buffer-substring compos comin))
		 (normalp
		  (string-match (regexp-quote (comment-string-strip
					       comment-start t t))
				comstart))
		 (comend
		  (if normalp comment-end
		    ;; The comment starter is not the normal comment-start
		    ;; so we can't just use comment-end.
		    (save-excursion
		      (goto-char compos)
		      (if (not (comment-forward)) comment-end
			(comment-string-strip
			 (buffer-substring
			  (save-excursion (comment-enter-backward) (point))
			  (point))
			 nil t))))))
	    (if (and comment-multi-line (> (length comend) 0))
		(indent-according-to-mode)
	      (insert-and-inherit ?\n)
	      (forward-char -1)
              (let* ((comment-column
                      ;; The continuation indentation should be somewhere
                      ;; between the current line's indentation (plus 2 for
                      ;; good measure) and the current comment's indentation,
                      ;; with a preference for comment-column.
                      (save-excursion
                        ;; FIXME: use prev line's info rather than first
                        ;; line's.
                        (goto-char compos)
                        (min (current-column)
                             (max comment-column
                                  (+ 2 (current-indentation))))))
                     (comment-indent-function
                      ;; If the previous comment is on its own line, then
                      ;; reuse its indentation unconditionally.
                      ;; Important for modes like Python/Haskell where
                      ;; auto-indentation is unreliable.
                      (if (save-excursion (goto-char compos)
                                          (skip-chars-backward " \t")
                                          (bolp))
                          (lambda () comment-column) comment-indent-function))
                     (comment-start comstart)
                     (comment-end comend)
                     (continuep (or comment-multi-line
                                    (cadr (assoc comment-style
                                                 comment-styles))))
                     ;; Recreate comment-continue from comment-start.
                     ;; FIXME: wrong if comment-continue was set explicitly!
                     ;; FIXME: use prev line's continuation if available.
                     (comment-continue nil))
                (comment-indent continuep))
	      (save-excursion
		(let ((pt (point)))
		  (end-of-line)
		  (let ((comend (buffer-substring pt (point))))
		    ;; The 1+ is to make sure we delete the \n inserted above.
		    (delete-region pt (1+ (point)))
		    (end-of-line 0)
		    (insert comend))))))))))))

(defun comment-line (n)
  "Comment or uncomment current line and leave point after it.
With positive prefix, apply to N lines including current one.
With negative prefix, apply to -N lines above.  Also, further
consecutive invocations of this command will inherit the negative
argument.

If region is active, comment lines in active region instead.
Unlike `comment-dwim', this always comments whole lines."
  (interactive "p")
  (if (use-region-p)
      (comment-or-uncomment-region
       (save-excursion
         (goto-char (region-beginning))
         (line-beginning-position))
       (save-excursion
         (goto-char (region-end))
         (line-end-position)))
    (when (and (eq last-command 'comment-line-backward)
               (natnump n))
      (setq n (- n)))
    (let ((range
           (list (line-beginning-position)
                 (goto-char (line-end-position n)))))
      (comment-or-uncomment-region
       (apply #'min range)
       (apply #'max range)))
    (forward-line 1)
    (back-to-indentation)
    (unless (natnump n) (setq this-command 'comment-line-backward))))

(defvar comment-use-syntax 'undecided
  "Non-nil if syntax-tables can be used instead of regexps.
Can also be `undecided' which means that a somewhat expensive test will
be used to try to determine whether syntax-tables should be trusted
to understand comments or not in the given buffer.
Major modes should set this variable.")

(defvar comment-fill-column nil
  "Column to use for `comment-indent'.  If nil, use `fill-column' instead."
  :type '(choice (const nil) integer)
  :group 'comment)

(defvar comment-end-skip nil
  "Regexp to match the end of a comment plus everything back to its body.")

(defvar comment-indent-function 'comment-indent-default
  "Function to compute desired indentation for a comment.
This function is called with no args with point at the beginning
of the comment's starting delimiter and should return either the
desired column indentation, a range of acceptable
indentation (MIN . MAX), or nil.
If nil is returned, indentation is delegated to `indent-according-to-mode'.")

(defvar comment-insert-comment-function nil
  "Function to insert a comment when a line doesn't contain one.
The function has no args.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar comment-region-function 'comment-region-default
  "Function to comment a region.
Its args are the same as those of `comment-region', but BEG and END are
guaranteed to be correctly ordered.  It is called within `save-excursion'.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar uncomment-region-function 'uncomment-region-default
  "Function to uncomment a region.
Its args are the same as those of `uncomment-region', but BEG and END are
guaranteed to be correctly ordered.  It is called within `save-excursion'.

Applicable at least in modes for languages like fixed-format Fortran where
comments always start in column zero.")

(defvar comment-quote-nested-function #'comment-quote-nested-default
  "Function to quote nested comments in a region.
It takes the same arguments as `comment-quote-nested-default',
and is called with the buffer narrowed to a single comment.")

(defvar comment-continue nil
  "Continuation string to insert for multiline comments.
This string will be added at the beginning of each line except the very
first one when commenting a region with a commenting style that allows
comments to span several lines.
It should generally have the same length as `comment-start' in order to
preserve indentation.
If it is nil a value will be automatically derived from `comment-start'
by replacing its first character with a space.")

(defconst comment-styles
  '((plain      nil nil nil nil
                "Start in column 0 (do not indent), as in Emacs-20")
    (indent-or-triple nil nil nil multi-char
              "Start in column 0, but only for single-char starters")
    (indent     nil nil nil t
                "Full comment per line, ends not aligned")
    (aligned	nil t   nil t
                "Full comment per line, ends aligned")
    (box	nil t   t   t
                "Full comment per line, ends aligned, + top and bottom")
    (extra-line	t   nil t   t
                "One comment for all lines, end on a line by itself")
    (multi-line	t   nil nil t
                "One comment for all lines, end on last commented line")
    (box-multi	t   t   t   t
                "One comment for all lines, + top and bottom"))
  "Comment region style definitions.
Each style is defined with a form (STYLE . (MULTI ALIGN EXTRA INDENT DOC)).
DOC should succinctly describe the style.
STYLE should be a mnemonic symbol.
MULTI specifies that comments are allowed to span multiple lines.
  e.g. in C it comments regions as
     /* blabla
      * bli */
  rather than
     /* blabla */
     /* bli */
  if `comment-end' is empty, this has no effect.

ALIGN specifies that the `comment-end' markers should be aligned.
  e.g. in C it comments regions as
     /* blabla */
     /* bli    */
  rather than
     /* blabla */
     /* bli */
  if `comment-end' is empty, this has no effect, unless EXTRA is also set,
  in which case the comment gets wrapped in a box.

EXTRA specifies that an extra line should be used before and after the
  region to comment (to put the `comment-end' and `comment-start').
  e.g. in C it comments regions as
     /*
      * blabla
      * bli
      */
  rather than
     /* blabla
      * bli */
  if the comment style is not multi line, this has no effect, unless ALIGN
  is also set, in which case the comment gets wrapped in a box.

INDENT specifies that the `comment-start' markers should not be put at the
  left margin but at the current indentation of the region to comment.
If INDENT is `multi-char', that means indent multi-character
  comment starters, but not one-character comment starters.")

(defvar comment-style 'indent
  "Style to be used for `comment-region'.
See `comment-styles' for a list of available styles."
  :type (if (boundp 'comment-styles)
	    `(choice
              ,@(mapcar (lambda (s)
                          `(const :tag ,(format "%s: %s" (car s) (nth 5 s))
                                  ,(car s)))
                        comment-styles))
	  'symbol)
  :version "23.1"
  :group 'comment)

(defvar comment-padding " "
  "Padding string that `comment-region' puts between comment chars and text.
Can also be an integer which will be automatically turned into a string
of the corresponding number of spaces.

Extra spacing between the comment characters and the comment text
makes the comment easier to read.  Default is 1.  nil means 0."
  :type '(choice string integer (const nil))
  :group 'comment)

(defvar comment-inline-offset 1
  "Inline comments have to be preceded by at least this many spaces.
This is useful when style-conventions require a certain minimal offset.
Python's PEP8 for example recommends two spaces, so you could do:

\(add-hook \\='python-mode-hook
   (lambda () (setq-local comment-inline-offset 2)))

See `comment-padding' for whole-line comments."
  :version "24.3"
  :type 'integer
  :group 'comment)

(defvar comment-multi-line nil
  "Non-nil means `comment-indent-new-line' continues comments.
That is, it inserts no new terminator or starter.
This affects `auto-fill-mode', which is the main reason to
customize this variable.

It also affects \\[indent-new-comment-line].  However, if you want this
behavior for explicit filling, you might as well use \\[newline-and-indent]."
  :type 'boolean
  :safe #'booleanp
  :group 'comment)

(defvar comment-empty-lines nil
  "If nil, `comment-region' does not comment out empty lines.
If t, it always comments out empty lines.
If `eol', it only comments out empty lines if comments are
terminated by the end of line (i.e., `comment-end' is empty)."
  :type '(choice (const :tag "Never" nil)
                 (const :tag "Always" t)
                 (const :tag "EOL-terminated" eol))
  :group 'comment)

(defvar comment-setup-function #'ignore
  "Function to set up variables needed by commenting functions.")

(defvar comment-use-global-state t
  "Non-nil means that the global syntactic context is used.
More specifically, it means that `syntax-ppss' is used to find out whether
point is within a string or not.  Major modes whose syntax is not faithfully
described by the syntax-tables (or where `font-lock-syntax-table' is radically
different from the main syntax table) can set this to nil,
then `syntax-ppss' cache won't be used in comment-related routines.")

(defvar comment-auto-fill-only-comments nil
  "Non-nil means to only auto-fill inside comments.
This has no effect in modes that do not define a comment syntax."
  :type 'boolean
  :group 'comment)

(defvar comment-combine-change-calls t
  "If non-nil (the default), use `combine-change-calls' around
calls of `comment-region-function' and
`uncomment-region-function'.  This Substitutes a single call to
each of the hooks `before-change-functions' and
`after-change-functions' in place of those hooks being called
for each individual buffer change.")

(defun set-fill-prefix (&optional arg)
  "Set the fill prefix to the current line up to point.
Filling expects lines to start with the fill prefix and
reinserts the fill prefix in each resulting line.
With a prefix argument, cancel the fill prefix."
  (interactive "P")
  (if arg
      (setq fill-prefix nil)
    (let ((left-margin-pos (save-excursion (move-to-left-margin) (point))))
      (if (> (point) left-margin-pos)
	  (progn
	    (setq fill-prefix (buffer-substring left-margin-pos (point)))
	    (when (equal fill-prefix "")
	      (setq fill-prefix nil)))
        (setq fill-prefix nil))))
  (if fill-prefix
      (message "fill-prefix: \"%s\"" fill-prefix)
    (message "fill-prefix cancelled")))

(defun current-fill-column ()
  "Return the fill-column to use for this line.
The fill-column to use for a buffer is stored in the variable `fill-column',
but can be locally modified by the `right-margin' text property, which is
subtracted from `fill-column'.

The fill column to use for a line is the first column at which the column
number equals or exceeds the local fill-column - right-margin difference."
  (save-excursion
    (if fill-column
	(let* ((here (line-beginning-position))
	       (here-col 0)
	       (eol (progn (end-of-line) (point)))
	       margin fill-col change col)
	  ;; Look separately at each region of line with a different
	  ;; right-margin.
	  (while (and (setq margin (get-text-property here 'right-margin)
			    fill-col (- fill-column (or margin 0))
			    change (text-property-not-all
				    here eol 'right-margin margin))
		      (progn (goto-char (1- change))
			     (setq col (current-column))
			     (< col fill-col)))
	    (setq here change
		  here-col col))
	  (max here-col fill-col))
      ;; This warning was added in 28.1.  It should be removed later,
      ;; and this function changed to never return nil.
      (unless current-fill-column--has-warned
        (lwarn '(fill-column) :warning
               "Setting this variable to nil is obsolete; use `(auto-fill-mode -1)' instead")
        (setq current-fill-column--has-warned t))
      most-positive-fixnum)))

(defun canonically-space-region (beg end)
  "Remove extra spaces between words in region.
Leave one space between words, two at end of sentences or after colons
\(depending on values of `sentence-end-double-space', `colon-double-space',
and `sentence-end-without-period').
Remove indentation from each line."
  (interactive "*r")
  ;; Ideally, we'd want to scan the text from the end, so that changes to
  ;; text don't affect the boundary, but the regexp we match against does
  ;; not match as eagerly when matching backward, so we instead use
  ;; a marker.
  (unless (markerp end) (setq end (copy-marker end t)))
  (let ((end-spc-re (concat "\\(" (sentence-end) "\\) *\\|  +")))
    (save-excursion
      (goto-char beg)
      ;; Nuke tabs; they get screwed up in a fill.
      ;; This is quick, but loses when a tab follows the end of a sentence.
      ;; Actually, it is difficult to tell that from "Mr.\tSmith".
      ;; Blame the typist.
      (subst-char-in-region beg end ?\t ?\s)
      (while (and (< (point) end)
		  (re-search-forward end-spc-re end t))
	(delete-region
	 (cond
	  ;; `sentence-end' matched and did not match all spaces.
	  ;; I.e. it only matched the number of spaces it needs: drop the rest.
	  ((and (match-end 1) (> (match-end 0) (match-end 1)))  (match-end 1))
	  ;; `sentence-end' matched but with nothing left.  Either that means
	  ;; nothing should be removed, or it means it's the "old-style"
	  ;; sentence-end which matches all it can.  Keep only 2 spaces.
	  ;; We probably don't even need to check `sentence-end-double-space'.
	  ((match-end 1)
	   (min (match-end 0)
		(+ (if sentence-end-double-space 2 1)
		   (save-excursion (goto-char (match-end 0))
				   (skip-chars-backward " ")
				   (point)))))
	  (t ;; It's not an end of sentence.
	   (+ (match-beginning 0)
	      ;; Determine number of spaces to leave:
	      (save-excursion
		(skip-chars-backward " ]})\"'")
		(cond ((and sentence-end-double-space
			    (or (memq (preceding-char) '(?. ?? ?!))
				(and sentence-end-without-period
				     (= (char-syntax (preceding-char)) ?w)))) 2)
		      ((and colon-double-space
			    (= (preceding-char) ?:))  2)
		      ((char-equal (preceding-char) ?\n)  0)
		      (t 1))))))
	 (match-end 0))))))

(defun fill-common-string-prefix (s1 s2)
  "Return the longest common prefix of strings S1 and S2, or nil if none."
  (let ((cmp (compare-strings s1 nil nil s2 nil nil)))
    (if (eq cmp t)
	s1
      (setq cmp (1- (abs cmp)))
      (unless (zerop cmp)
	(substring s1 0 cmp)))))

(defun fill-match-adaptive-prefix ()
  (let ((str (or
              (and adaptive-fill-function (funcall adaptive-fill-function))
              (and adaptive-fill-regexp (looking-at adaptive-fill-regexp)
                   (match-string 0)))))
    (if (>= (+ (current-left-margin) (length str)) (current-fill-column))
        ;; Death to insanely long prefixes.
        nil
      str)))

(defun fill-context-prefix (from to &optional first-line-regexp)
  "Compute a fill prefix from the text between FROM and TO.
This uses the variables `adaptive-fill-regexp' and `adaptive-fill-function'
and `adaptive-fill-first-line-regexp'.  `paragraph-start' also plays a role;
we reject a prefix based on a one-line paragraph if that prefix would
act as a paragraph-separator."
  (or first-line-regexp
      (setq first-line-regexp adaptive-fill-first-line-regexp))
  (save-excursion
    (goto-char from)
    (if (eolp) (forward-line 1))
    ;; Move to the second line unless there is just one.
    (move-to-left-margin)
    (let (first-line-prefix
	  ;; Non-nil if we are on the second line.
	  second-line-prefix)
      (setq first-line-prefix
	    ;; We don't need to consider `paragraph-start' here since it
	    ;; will be explicitly checked later on.
	    ;; Also setting first-line-prefix to nil prevents
	    ;; second-line-prefix from being used.
	    ;; ((looking-at paragraph-start) nil)
	    (fill-match-adaptive-prefix))
      (forward-line 1)
      (if (< (point) to)
          (progn
            (move-to-left-margin)
            (setq second-line-prefix
                  (cond ((looking-at paragraph-start) nil) ;Can it happen? -Stef
                        (t (fill-match-adaptive-prefix))))
            ;; If we get a fill prefix from the second line,
            ;; make sure it or something compatible is on the first line too.
            (when second-line-prefix
              (unless first-line-prefix (setq first-line-prefix ""))
              ;; If the non-whitespace chars match the first line,
              ;; just use it (this subsumes the 2 checks used previously).
              ;; Used when first line is `/* ...' and second-line is
              ;; ` * ...'.
              (let ((tmp second-line-prefix)
                    (re "\\`"))
                (while (string-match "\\`[ \t]*\\([^ \t]+\\)" tmp)
                  (setq re (concat re ".*" (regexp-quote (match-string 1 tmp))))
                  (setq tmp (substring tmp (match-end 0))))
                ;; (assert (string-match "\\`[ \t]*\\'" tmp))

                (if (string-match re first-line-prefix)
                    second-line-prefix

                  ;; Use the longest common substring of both prefixes,
                  ;; if there is one.
                  (fill-common-string-prefix first-line-prefix
                                             second-line-prefix)))))
	;; If we get a fill prefix from a one-line paragraph,
	;; maybe change it to whitespace,
	;; and check that it isn't a paragraph starter.
	(if first-line-prefix
	    (let ((result
		   ;; If first-line-prefix comes from the first line,
		   ;; see if it seems reasonable to use for all lines.
		   ;; If not, replace it with whitespace.
		   (if (or (and first-line-regexp
				(string-match first-line-regexp
					      first-line-prefix))
			   (and comment-start-skip
				(string-match comment-start-skip
					      first-line-prefix)))
		       first-line-prefix
		     (make-string (string-width first-line-prefix) ?\s))))
	      ;; But either way, reject it if it indicates the start
	      ;; of a paragraph when text follows it.
	      (if (not (eq 0 (string-match paragraph-start
					   (concat result "a"))))
		  result)))))))

(defun fill-single-word-nobreak-p ()
  "Don't break a line after the first or before the last word of a sentence."
  ;; Actually, allow breaking before the last word of a sentence, so long as
  ;; it's not the last word of the paragraph.
  (or (looking-at (concat "[ \t]*\\sw+" "\\(?:" (sentence-end) "\\)[ \t]*$"))
      (save-excursion
	(skip-chars-backward " \t")
	(and (/= (skip-syntax-backward "w") 0)
	     (/= (skip-chars-backward " \t") 0)
	     (/= (skip-chars-backward ".?!:") 0)
	     (looking-at (sentence-end))))))

(defun fill-french-nobreak-p ()
  "Return nil if French style allows breaking the line at point.
This is used in `fill-nobreak-predicate' to prevent breaking lines just
after an opening paren or just before a closing paren or a punctuation
mark such as `?' or `:'.  It is common in French writing to put a space
at such places, which would normally allow breaking the line at those
places."
  (or (looking-at "[ \t]*[])}»?!;:-]")
      (save-excursion
	(skip-chars-backward " \t")
	(unless (bolp)
	  (backward-char 1)
	  (or (looking-at "[([{«]")
	      ;; Don't cut right after a single-letter word.
	      (and (memq (preceding-char) '(?\t ?\s))
		   (eq (char-syntax (following-char)) ?w)))))))

(defun fill-polish-nobreak-p ()
  "Return nil if Polish style allows breaking the line at point.
This function may be used in the `fill-nobreak-predicate' hook.
It is almost the same as `fill-single-char-nobreak-p', with the
exception that it does not require the one-letter word to be
preceded by a space.  This blocks line-breaking in cases like
\"(a jednak)\"."
  (save-excursion
    (skip-chars-backward " \t")
    (backward-char 2)
    (looking-at "[^[:alpha:]]\\cl")))

(defun fill-single-char-nobreak-p ()
  "Return non-nil if a one-letter word is before point.
This function is suitable for adding to the hook `fill-nobreak-predicate',
to prevent the breaking of a line just after a one-letter word,
which is an error according to some typographical conventions."
  (save-excursion
    (skip-chars-backward " \t")
    (backward-char 2)
    (looking-at "[[:space:]][[:alpha:]]")))

(defun fill-nobreak-p ()
  "Return nil if breaking the line at point is allowed.
Can be customized with the variables `fill-nobreak-predicate'
and `fill-nobreak-invisible'."
  (or
   (and fill-nobreak-invisible (invisible-p (point)))
   (unless (bolp)
    (or
     ;; Don't break after a period followed by just one space.
     ;; Move back to the previous place to break.
     ;; The reason is that if a period ends up at the end of a
     ;; line, further fills will assume it ends a sentence.
     ;; If we now know it does not end a sentence, avoid putting
     ;; it at the end of the line.
     (and sentence-end-double-space
	  (save-excursion
	    (skip-chars-backward " ")
	    (and (eq (preceding-char) ?.)
                 ;; There's something more after the space.
		 (looking-at " [^ \n]"))))
     ;; Don't split a line if the rest would look like a new paragraph.
     (unless use-hard-newlines
       (save-excursion
	 (skip-chars-forward " \t")
	 ;; If this break point is at the end of the line,
	 ;; which can occur for auto-fill, don't consider the newline
	 ;; which follows as a reason to return t.
	 (and (not (eolp))
	      (looking-at paragraph-start))))
     (run-hook-with-args-until-success 'fill-nobreak-predicate)))))

(defun fill-find-break-point (limit)
  "Move point to a proper line breaking position of the current line.
Don't move back past the buffer position LIMIT.

This function is called when we are going to break the current line
after or before a non-ASCII character.  If the charset of the
character has the property `fill-find-break-point-function', this
function calls the property value as a function with one arg LIMIT.
If the charset has no such property, do nothing."
  (let ((func (or
	       (aref fill-find-break-point-function-table (following-char))
	       (aref fill-find-break-point-function-table (preceding-char)))))
    (if (and func (fboundp func))
	(funcall func limit))))

(defun fill-delete-prefix (from to prefix)
  "Delete the fill prefix from every line except the first.
The first line may not even have a fill prefix.
Point is moved to just past the fill prefix on the first line."
  (let ((fpre (if (and prefix (not (string-match "\\`[ \t]*\\'" prefix)))
		  (concat "[ \t]*\\("
			  (replace-regexp-in-string
			   "[ \t]+" "[ \t]*"
			   (regexp-quote prefix))
			  "\\)?[ \t]*")
		"[ \t]*")))
    (goto-char from)
    ;; Why signal an error here?  The problem needs to be caught elsewhere.
    ;; (if (>= (+ (current-left-margin) (length prefix))
    ;;         (current-fill-column))
    ;;     (error "fill-prefix too long for specified width"))
    (forward-line 1)
    (while (< (point) to)
      (if (looking-at fpre)
          (delete-region (point) (match-end 0)))
      (forward-line 1))
    (goto-char from)
    (if (looking-at fpre)
	(goto-char (match-end 0)))
    (point)))

(defun fill-delete-newlines (from to justify nosqueeze squeeze-after)
  (goto-char from)
  ;; Make sure sentences ending at end of line get an extra space.
  ;; loses on split abbrevs ("Mr.\nSmith")
  (let ((eol-double-space-re
	 (cond
	  ((not colon-double-space) (concat (sentence-end) "$"))
	  ;; Try to add the : inside the `sentence-end' regexp.
	  ((string-match "\\[[^][]*\\(\\.\\)[^][]*\\]" (sentence-end))
	   (concat (replace-match ".:" nil nil (sentence-end) 1) "$"))
	  ;; Can't find the right spot to insert the colon.
	  (t "[.?!:][])}\"']*$")))
	(sentence-end-without-space-list
	 (string-to-list sentence-end-without-space)))
    (while (re-search-forward eol-double-space-re to t)
      (or (>= (point) to) (memq (char-before) '(?\t ?\s))
	  (memq (char-after (match-beginning 0))
		sentence-end-without-space-list)
	  (insert-and-inherit ?\s))))

  (goto-char from)
  (if enable-multibyte-characters
      ;; Delete unnecessary newlines surrounded by words.  The
      ;; character category `|' means that we can break a line at the
      ;; character.  And, char-table
      ;; `fill-nospace-between-words-table' tells how to concatenate
      ;; words.  If a character has non-nil value in the table, never
      ;; put spaces between words, thus delete a newline between them.
      ;; Otherwise, delete a newline only when a character preceding a
      ;; newline has non-nil value in that table.
      (while (search-forward "\n" to t)
	(if (get-text-property (match-beginning 0) 'fill-space)
	    (replace-match (get-text-property (match-beginning 0) 'fill-space))
	  (let ((prev (char-before (match-beginning 0)))
		(next (following-char)))
	    (if (and (if fill-separate-heterogeneous-words-with-space
			 (and (aref (char-category-set next) ?|)
			      (aref (char-category-set prev) ?|))
		       (or (aref (char-category-set next) ?|)
			   (aref (char-category-set prev) ?|)))
		     (or (aref fill-nospace-between-words-table next)
			 (aref fill-nospace-between-words-table prev)))
		(delete-char -1))))))

  (goto-char from)
  (skip-chars-forward " \t")
  ;; Then change all newlines to spaces.
  (subst-char-in-region from to ?\n ?\s)
  (if (and nosqueeze (not (eq justify 'full)))
      nil
    (canonically-space-region (or squeeze-after (point)) to)
    ;; Remove trailing whitespace.
    ;; Maybe canonically-space-region should do that.
    (goto-char to) (delete-char (- (skip-chars-backward " \t"))))
  (goto-char from))

(defun fill-move-to-break-point (linebeg)
  "Move to the position where the line should be broken.
The break position will be always after LINEBEG and generally before point."
  ;; If the fill column is before linebeg, move to linebeg.
  (if (> linebeg (point)) (goto-char linebeg))
  ;; Move back to the point where we can break the line
  ;; at.  We break the line between word or after/before
  ;; the character which has character category `|'.  We
  ;; search space, \c| followed by a character, or \c|
  ;; following a character.  If not found, place
  ;; the point at linebeg.
  (while
      (when (re-search-backward "[ \t]\\|\\c|.\\|.\\c|" linebeg 0)
	;; In case of space, we place the point at next to
	;; the point where the break occurs actually,
	;; because we don't want to change the following
	;; logic of original Emacs.  In case of \c|, the
	;; point is at the place where the break occurs.
	(forward-char 1)
	(when (fill-nobreak-p) (skip-chars-backward " \t" linebeg))))

  ;; Move back over the single space between the words.
  (skip-chars-backward " \t")

  ;; If the left margin and fill prefix by themselves
  ;; pass the fill-column. or if they are zero
  ;; but we have no room for even one word,
  ;; keep at least one word or a character which has
  ;; category `|' anyway.
  (if (>= linebeg (point))
      ;; Ok, skip at least one word or one \c| character.
      ;; Meanwhile, don't stop at a period followed by one space.
      (let ((to (line-end-position))
	    (first t))
	(goto-char linebeg)
	(while (and (< (point) to) (or first (fill-nobreak-p)))
	  ;; Find a breakable point while ignoring the
	  ;; following spaces.
	  (skip-chars-forward " \t")
	  (if (looking-at "\\c|")
	      (forward-char 1)
	    (let ((pos (save-excursion
			 (skip-chars-forward "^ \n\t")
			 (point))))
	      (if (re-search-forward "\\c|" pos t)
		  (forward-char -1)
		(goto-char pos))))
	  (setq first nil)))

    (if enable-multibyte-characters
	;; If we are going to break the line after or
	;; before a non-ascii character, we may have to
	;; run a special function for the charset of the
	;; character to find the correct break point.
	(if (not (and (eq (charset-after (1- (point))) 'ascii)
		      (eq (charset-after (point)) 'ascii)))
	    ;; Make sure we take SOMETHING after the fill prefix if any.
	    (fill-find-break-point linebeg)))))

(defun fill-text-properties-at (pos)
  (let ((l (text-properties-at pos))
	prop-list)
    (while l
      (unless (eq (car l) 'composition)
	(setq prop-list
	      (cons (car l) (cons (cadr l) prop-list))))
      (setq l (cddr l)))
    prop-list))

(defun fill-newline ()
  ;; Replace whitespace here with one newline, then
  ;; indent to left margin.
  (skip-chars-backward " \t")
  (insert ?\n)
  ;; Give newline the properties of the space(s) it replaces
  (set-text-properties (1- (point)) (point)
		       (fill-text-properties-at (point)))
  (and (looking-at "\\( [ \t]*\\)\\(\\c|\\)?")
       (or (aref (char-category-set (or (char-before (1- (point))) ?\000)) ?|)
	   (match-end 2))
       ;; When refilling later on, this newline would normally not be replaced
       ;; by a space, so we need to mark it specially to re-install the space
       ;; when we unfill.
       (put-text-property (1- (point)) (point) 'fill-space (match-string 1)))
  ;; If we don't want breaks in invisible text, don't insert
  ;; an invisible newline.
  (if fill-nobreak-invisible
      (remove-text-properties (1- (point)) (point)
			      '(invisible t)))
  (if (or fill-prefix
	  (not fill-indent-according-to-mode))
      (fill-indent-to-left-margin)
    (indent-according-to-mode))
  ;; Insert the fill prefix after indentation.
  (and fill-prefix (not (equal fill-prefix ""))
       ;; Markers that were after the whitespace are now at point: insert
       ;; before them so they don't get stuck before the prefix.
       (insert-before-markers-and-inherit fill-prefix)))

(defun fill-indent-to-left-margin ()
  "Indent current line to the column given by `current-left-margin'."
  (let ((beg (point)))
    (indent-line-to (current-left-margin))
    (put-text-property beg (point) 'face 'default)))

(defun fill-region-as-paragraph-default (from to &optional justify
				              nosqueeze squeeze-after)
  "Fill the region as if it were a single paragraph.
This command removes any paragraph breaks in the region and
extra newlines at the end, and indents and fills lines between the
margins given by the `current-left-margin' and `current-fill-column'
functions.  (In most cases, the variable `fill-column' controls the
width.)  It leaves point at the beginning of the line following the
region.

Note that how paragraph breaks are removed in text that includes
characters from different scripts is affected by the value
of `fill-separate-heterogeneous-words-with-space', which see.

Normally, the command performs justification according to
the `current-justification' function, but with a prefix arg, it
does full justification instead.

When called from Lisp, optional third arg JUSTIFY can specify any
type of justification; see `default-justification' for the possible
values.
Optional fourth arg NOSQUEEZE non-nil means not to make spaces
between words canonical before filling.
Fifth arg SQUEEZE-AFTER, if non-nil, should be a buffer position; it
means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization
of spaces.

Return the `fill-prefix' used for filling.

If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (unless (memq justify '(t nil none full center left right))
    (setq justify 'full))

  ;; Make sure "to" is the endpoint.
  (goto-char (min from to))
  (setq to   (max from to))
  ;; Ignore blank lines at beginning of region.
  (skip-chars-forward " \t\n")

  (let ((from-plus-indent (point))
	(oneleft nil))

    (beginning-of-line)
    ;; We used to round up to whole line, but that prevents us from
    ;; correctly handling filling of mixed code-and-comment where we do want
    ;; to fill the comment but not the code.  So only use (point) if it's
    ;; further than `from', which means that `from' is followed by some
    ;; number of empty lines.
    (setq from (max (point) from))

    ;; Delete all but one soft newline at end of region.
    ;; And leave TO before that one.
    (goto-char to)
    (while (and (> (point) from) (eq ?\n (char-after (1- (point)))))
      (if (and oneleft
	       (not (and use-hard-newlines
			 (get-text-property (1- (point)) 'hard))))
	  (delete-char -1)
	(backward-char 1)
	(setq oneleft t)))
    (setq to (copy-marker (point) t))
    ;; ;; If there was no newline, and there is text in the paragraph, then
    ;; ;; create a newline.
    ;; (if (and (not oneleft) (> to from-plus-indent))
    ;; 	(newline))
    (goto-char from-plus-indent))

  (if (not (> to (point)))
      ;; There is no paragraph, only whitespace: exit now.
      (progn
        (set-marker to nil)
        nil)

    (or justify (setq justify (current-justification)))

    ;; Don't let Adaptive Fill mode alter the fill prefix permanently.
    (let ((fill-prefix fill-prefix))
      ;; Figure out how this paragraph is indented, if desired.
      (when (and adaptive-fill-mode
		 (or (null fill-prefix) (string= fill-prefix "")))
	(setq fill-prefix (fill-context-prefix from to))
	;; Ignore a white-space only fill-prefix
	;; if we indent-according-to-mode.
	(when (and fill-prefix fill-indent-according-to-mode
		   (string-match "\\`[ \t]*\\'" fill-prefix))
	  (setq fill-prefix nil)))

      (goto-char from)
      (beginning-of-line)

      (if (not justify)     ; filling disabled: just check indentation
	  (progn
	    (goto-char from)
	    (while (< (point) to)
	      (if (and (not (eolp))
		       (< (current-indentation) (current-left-margin)))
		  (fill-indent-to-left-margin))
	      (forward-line 1)))

	(if use-hard-newlines
	    (remove-list-of-text-properties from to '(hard)))
	;; Make sure first line is indented (at least) to left margin...
	(if (or (memq justify '(right center))
		(< (current-indentation) (current-left-margin)))
	    (fill-indent-to-left-margin))
	;; Delete the fill-prefix from every line.
	(fill-delete-prefix from to fill-prefix)
	(setq from (point))

	;; FROM, and point, are now before the text to fill,
	;; but after any fill prefix on the first line.

	(fill-delete-newlines from to justify nosqueeze squeeze-after)

	;; This is the actual filling loop.
	(goto-char from)
	(let (linebeg)
          (while (< (point) to)
	    (setq linebeg (point))
	    (move-to-column (current-fill-column))
	    (if (when (and (< (point) to) (< linebeg to))
		  ;; Find the position where we'll break the line.
		  ;; Use an immediately following space, if any.
		  ;; However, note that `move-to-column' may overshoot
		  ;; if there are wide characters (Bug#3234).
		  (unless (> (current-column) (current-fill-column))
		    (forward-char 1))
		  (fill-move-to-break-point linebeg)
		  ;; Check again to see if we got to the end of
		  ;; the paragraph.
		  (skip-chars-forward " \t")
		  (< (point) to))
		;; Found a place to cut.
		(progn
		  (fill-newline)
		  (when justify
		    ;; Justify the line just ended, if desired.
		    (save-excursion
		      (forward-line -1)
		      (justify-current-line justify nil t))))

	      (goto-char to)
	      ;; Justify this last line, if desired.
	      (if justify (justify-current-line justify t t))))))
      ;; Leave point after final newline.
      (goto-char to)
      (unless (eobp) (forward-char 1))
      (set-marker to nil)
      ;; Return the fill-prefix we used
      fill-prefix)))

(defun fill-region-as-paragraph (from to &optional justify
				      nosqueeze squeeze-after)
  "Fill the region as if it were a single paragraph.
The behavior of this command is controlled by the variable
`fill-region-as-paragraph-function', with the default implementation
being `fill-region-as-paragraph-default'.

The arguments FROM and TO define the boundaries of the region.

The optional third argument JUSTIFY, when called interactively with a
prefix arg, is assigned the value `full'.
When called from Lisp, JUSTIFY can specify any type of justification;
see `default-justification' for the possible values.
Optional fourth arg NOSQUEEZE non-nil means not to make spaces between
words canonical before filling.
Fifth arg SQUEEZE-AFTER, if non-nil, should be a buffer position; it
means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization of
spaces.

It returns the `fill-prefix' used for filling."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (funcall fill-region-as-paragraph-function
           from to justify nosqueeze squeeze-after))

(defun skip-line-prefix (prefix)
  "If point is inside the string PREFIX at the beginning of line, move past it."
  (when (and prefix
	     (< (- (point) (line-beginning-position)) (length prefix))
	     (save-excursion
	       (beginning-of-line)
	       (looking-at (regexp-quote prefix))))
    (goto-char (match-end 0))))

(defun fill-minibuffer-function (arg)
  "Fill a paragraph in the minibuffer, ignoring the prompt."
  (save-restriction
    (narrow-to-region (minibuffer-prompt-end) (point-max))
    (fill-paragraph arg)))

(defun fill-forward-paragraph (arg)
  (funcall fill-forward-paragraph-function arg))

(defun fill-paragraph (&optional justify region)
  "Fill paragraph at or after point.

If JUSTIFY is non-nil (interactively, with prefix argument), justify as well.
If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there.
The variable `fill-column' controls the width for filling.

If `fill-paragraph-function' is non-nil, we call it (passing our
argument to it), and if it returns non-nil, we simply return its value.

If `fill-paragraph-function' is nil, return the `fill-prefix' used for filling.

The REGION argument is non-nil if called interactively; in that
case, if Transient Mark mode is enabled and the mark is active,
call `fill-region' to fill each of the paragraphs in the active
region, instead of just filling the current paragraph."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (if current-prefix-arg 'full) t)))
  (with-buffer-unmodified-if-unchanged
    (or
     ;; 1. Fill the region if it is active when called interactively.
     (and region transient-mark-mode mark-active
          (not (eq (region-beginning) (region-end)))
          (or (fill-region (region-beginning) (region-end) justify) t))
     ;; 2. Try fill-paragraph-function.
     (and (not (eq fill-paragraph-function t))
          (or fill-paragraph-function
              (and (minibufferp (current-buffer))
                   (= 1 (point-min))))
          (let ((function (or fill-paragraph-function
                              ;; In the minibuffer, don't count
                              ;; the width of the prompt.
                              'fill-minibuffer-function))
                ;; If fill-paragraph-function is set, it probably
                ;; takes care of comments and stuff.  If not, it
                ;; will have to set fill-paragraph-handle-comment
                ;; back to t explicitly or return nil.
                (fill-paragraph-handle-comment nil)
                (fill-paragraph-function t))
            (funcall function justify)))
     ;; 3. Try our syntax-aware filling code.
     (and fill-paragraph-handle-comment
          ;; Our code only handles \n-terminated comments right now.
          comment-start (equal comment-end "")
          (let ((fill-paragraph-handle-comment nil))
            (fill-comment-paragraph justify)))
     ;; 4. If it all fails, default to the good ol' text paragraph filling.
     (let ((before (point))
           (paragraph-start-orig paragraph-start)
           (paragraph-start paragraph-start)
           ;; Fill prefix used for filling the paragraph.
           fill-pfx)
       ;; Try to prevent code sections and comment sections from being
       ;; filled together.
       (when (and fill-paragraph-handle-comment comment-start-skip)
         (setq paragraph-start
               (concat paragraph-start "\\|[ \t]*\\(?:"
                       comment-start-skip "\\)")))
       (save-excursion
         ;; To make sure the return value of forward-paragraph is
         ;; meaningful, we have to start from the beginning of
         ;; line, otherwise skipping past the last few chars of a
         ;; paragraph-separator would count as a paragraph (and
         ;; not skipping any chars at EOB would not count as a
         ;; paragraph even if it is).
         (move-to-left-margin)
         (if (not (zerop (fill-forward-paragraph 1)))
             ;; There's no paragraph at or after point: give up.
             (setq fill-pfx "")
           (let ((end (point))
                 (beg (progn (fill-forward-paragraph -1) (point))))
             ;; If the paragraph starts with a comment line preceding point
             ;; on a non-comment line, skip such comment lines, so they
             ;; are not filled together (bug#80449).
             (when (and fill-paragraph-handle-comment comment-start-skip
                        (< beg before))
               (save-excursion
                 (goto-char beg)
                 (when (looking-at paragraph-start-orig)
                   (goto-char (1+ (match-end 0))))
                 (when (looking-at comment-start-skip)
                   (forward-line 1)
                   (setq beg (point)))))
             (goto-char before)
             (setq fill-pfx
                   (if use-hard-newlines
                       ;; Can't use fill-region-as-paragraph, since this
                       ;; paragraph may still contain hard newlines.  See
                       ;; fill-region.
                       (fill-region beg end justify)
                     (fill-region-as-paragraph beg end justify))))))
       fill-pfx))))

(defun unfill-paragraph (arg &optional beg end)
  "Join lines of this paragraph and fix up whitespace at joins.
Interactively, if the region is active, join lines of each paragraph in
the region.  A numeric prefix argument means join the lines of the
following ARG paragraphs.  In this case an active region is ignored.

With an active region and no prefix argument this is roughly the same as
`delete-indentation' with that active region, except that this command
only joins lines within paragraphs, preserving the paragraphs
themselves.

When called from Lisp, ARG is the number of following paragraphs to join
lines within, or if ARG is nil, optional arguments BEG and END non-nil
means to join the lines of each paragraph in the region delimited by BEG
and END."
  (interactive "P\nR")
  (when (or arg (not beg))
    (let ((arg (prefix-numeric-value arg)))
      (when (zerop arg)
        (user-error "Invalid numeric argument to `unfill-paragraph'"))
      (save-excursion
        (fill-forward-paragraph 1)
        (fill-forward-paragraph -1)
        (setq beg (point))
        (fill-forward-paragraph arg)
        (setq end (point)))))
  ;; FIXME: It would be better to use
  ;;
  ;;    (let ((fill-column (* (max 2 tab-width) (point-max))))
  ;;      (fill-region beg end))
  ;;
  ;; multiplying by at least 2 to account for any wide characters in the
  ;; region to be filled and by at least `tab-width' to account for any
  ;; tab characters in the region to be filled.  Then we can easily
  ;; prove that filling the region will actually unfill it.
  ;; However, `fill-region' fails if `fill-column' is not a fixnum.
  (let ((fill-column most-positive-fixnum))
    (fill-region beg end)))

(defun fill-comment-paragraph (&optional justify)
  "Fill current comment.
If we're not in a comment, just return nil so that the caller
can take care of filling.  JUSTIFY is used as in `fill-paragraph'."
  (comment-normalize-vars)
  (let (has-code-and-comment ; Non-nil if it contains code and a comment.
	comin comstart)
    ;; Figure out what kind of comment we are looking at.
    (save-excursion
      (beginning-of-line)
      (when (setq comstart (comment-search-forward (line-end-position) t))
	(setq comin (point))
	(goto-char comstart) (skip-chars-backward " \t")
	(setq has-code-and-comment (not (bolp)))))

    (if (not (and comstart
                  ;; Make sure the comment-start mark we found is accepted by
                  ;; comment-start-skip.  If not, all bets are off, and
                  ;; we'd better not mess with it.
                  (string-match comment-start-skip
                                (buffer-substring comstart comin))))

	;; Return nil, so the normal filling will take place.
	nil

      ;; Narrow to include only the comment, and then fill the region.
      (let* ((fill-prefix fill-prefix)
	     (commark
	      (comment-string-strip (buffer-substring comstart comin) nil t))
	     (comment-re
              ;; A regexp more specialized than comment-start-skip, that only
              ;; matches the current commark rather than any valid commark.
              ;;
              ;; The specialized regexp only works for "normal" comment
              ;; syntax, not for Texinfo's "@c" (which can't be immediately
              ;; followed by word-chars) or Fortran's "C" (which needs to be
              ;; at bol), so check that comment-start-skip indeed allows the
              ;; commark to appear in the middle of the line and followed by
              ;; word chars.  The choice of "\0" and "a" is mostly arbitrary.
              (if (string-match comment-start-skip (concat "\0" commark "a"))
                  (concat "[ \t]*" (regexp-quote commark)
                          ;; Make sure we only match comments that
                          ;; use the exact same comment marker.
                          "[^" (substring commark -1) "]")
                (concat "[ \t]*\\(?:" comment-start-skip "\\)")))
             (comment-fill-prefix	; Compute a fill prefix.
	      (save-excursion
		(goto-char comstart)
		(if has-code-and-comment
		    (concat
		     (if (not indent-tabs-mode)
			 (make-string (current-column) ?\s)
		       (concat
			(make-string (/ (current-column) tab-width) ?\t)
			(make-string (% (current-column) tab-width) ?\s)))
		     (buffer-substring (point) comin))
		  (buffer-substring (line-beginning-position) comin))))
	     beg end)
	(save-excursion
	  (save-restriction
	    (beginning-of-line)
	    (narrow-to-region
	     ;; Find the first line we should include in the region to fill.
	     (if has-code-and-comment
		 (line-beginning-position)
	       (save-excursion
		 (while (and (zerop (forward-line -1))
			     (looking-at comment-re)))
		 ;; We may have gone too far.  Go forward again.
		 (line-beginning-position
		  (if (progn
			(goto-char
			 (or (comment-search-forward (line-end-position) t)
			     (point)))
			(looking-at comment-re))
		      (progn (setq comstart (point)) 1)
		    (progn (setq comstart (point)) 2)))))
	     ;; Find the beginning of the first line past the region to fill.
	     (save-excursion
	       (while (progn (forward-line 1)
			     (looking-at comment-re)))
	       (point)))
	    ;; Obey paragraph starters and boundaries within comments.
	    (let* ((paragraph-separate
		    ;; Use the default values since they correspond to
		    ;; the values to use for plain text.
		    (concat paragraph-separate "\\|[ \t]*\\(?:"
			    comment-start-skip "\\)\\(?:"
			    (default-value 'paragraph-separate) "\\)"))
		   (paragraph-start
		    (concat paragraph-start "\\|[ \t]*\\(?:"
			    comment-start-skip "\\)\\(?:"
			    (default-value 'paragraph-start) "\\)"))
		   ;; We used to rely on fill-prefix to break paragraph at
		   ;; comment-starter changes, but it did not work for the
		   ;; first line (mixed comment&code).
		   ;; We now use comment-re instead to "manually" make sure
		   ;; we treat comment-marker changes as paragraph boundaries.
		   ;; (paragraph-ignore-fill-prefix nil)
		   ;; (fill-prefix comment-fill-prefix)
		   (after-line (if has-code-and-comment
				   (line-beginning-position 2))))
	      (setq end (progn (forward-paragraph) (point)))
	      ;; If this comment starts on a line with code,
	      ;; include that line in the filling.
	      (setq beg (progn (backward-paragraph)
			       (if (eq (point) after-line)
				   (forward-line -1))
			       (point)))))

	  ;; Find the fill-prefix to use.
	  (cond
	   (fill-prefix)	  ; Use the user-provided fill prefix.
	   ((and adaptive-fill-mode	; Try adaptive fill mode.
		 (setq fill-prefix (fill-context-prefix beg end))
		 (string-match comment-start-skip fill-prefix)))
	   (t
	    (setq fill-prefix comment-fill-prefix)))

	  ;; Don't fill with narrowing.
	  (or
	   (fill-region-as-paragraph
	    (max comstart beg) end justify nil
	    ;; Don't canonicalize spaces within the code just before
	    ;; the comment.
	    (save-excursion
	      (goto-char beg)
	      (if (looking-at fill-prefix)
		  nil
		(re-search-forward comment-start-skip))))
	   ;; Make sure we don't return nil.
	   t))))))

(defun fill-region (from to &optional justify nosqueeze to-eop)
  "Fill each of the paragraphs in the region.
A prefix arg means justify as well.
The `fill-column' variable controls the width.

Noninteractively, the third argument JUSTIFY specifies which
kind of justification to do: `full', `left', `right', `center',
or `none' (equivalent to nil).  A value of t means handle each
paragraph as specified by its text properties.

The fourth arg NOSQUEEZE non-nil means to leave whitespace other
than line breaks untouched, and fifth arg TO-EOP non-nil means
to keep filling to the end of the paragraph (or next hard newline,
if variable `use-hard-newlines' is on).

Return the `fill-prefix' used for filling the last paragraph.

If `sentence-end-double-space' is non-nil, then period followed by one
space does not end a sentence, so don't break a line there.

The variable `fill-region-as-paragraph-function' can be used to override
how paragraphs are filled."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (unless (memq justify '(t nil none full center left right))
    (setq justify 'full))
  (let ((start-point (point-marker))
	max beg fill-pfx)
    (goto-char (max from to))
    (when to-eop
      (skip-chars-backward "\n")
      (fill-forward-paragraph 1))
    (setq max (copy-marker (point) t))
    (goto-char (setq beg (min from to)))
    (beginning-of-line)
    (while (< (point) max)
      (let ((initial (point))
	    end)
	;; If using hard newlines, break at every one for filling
	;; purposes rather than using paragraph breaks.
	(if use-hard-newlines
	    (progn
	      (while (and (setq end (text-property-any (point) max
						       'hard t))
			  (not (= ?\n (char-after end)))
			  (not (>= end max)))
		(goto-char (1+ end)))
	      (setq end (if end (min max (1+ end)) max))
	      (goto-char initial))
	  (fill-forward-paragraph 1)
	  (setq end (min max (point)))
	  (fill-forward-paragraph -1))
	(if (< (point) beg)
	    (goto-char beg))
	(if (and (>= (point) initial) (< (point) end))
	    (setq fill-pfx
		  (fill-region-as-paragraph (point) end justify nosqueeze))
	  (goto-char end))))
    (goto-char start-point)
    (set-marker start-point nil)
    fill-pfx))

(defun current-justification ()
  "How should we justify this line?
This returns the value of the text-property `justification',
or the variable `default-justification' if there is no text-property.
However, it returns nil rather than `none' to mean \"don't justify\"."
  (let ((j (or (get-text-property
		;; Make sure we're looking at paragraph body.
		(save-excursion (skip-chars-forward " \t")
				(if (and (eobp) (not (bobp)))
				    (1- (point)) (point)))
		'justification)
	       default-justification)))
    (if (eq 'none j)
	nil
      j)))

(defun set-justification (begin end style &optional whole-par)
  "Set the region's justification style to STYLE.
This commands prompts for the kind of justification to use.
See `default-justification' for the possible values and their meaning.
If the mark is not active, this command operates on the current paragraph.
If the mark is active, it operates on the region.  However, if the
beginning and end of the region are not at paragraph breaks, they are
moved to the beginning and end \(respectively) of the paragraphs they
are in.

If variable `use-hard-newlines' is true, all hard newlines are
taken to be paragraph breaks.

When calling from a program, operates just on region between BEGIN and END,
unless optional fourth arg WHOLE-PAR is non-nil.  In that case bounds are
extended to include entire paragraphs as in the interactive command."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))
		     (let ((s (completing-read
			       "Set justification to: "
			       '(("left") ("right") ("full")
				 ("center") ("none"))
			       nil t)))
		       (if (equal s "") (error ""))
		       (intern s))
		     t))
  (save-excursion
    (save-restriction
      (if whole-par
	  (let ((paragraph-start (if use-hard-newlines "." paragraph-start))
		(paragraph-ignore-fill-prefix (if use-hard-newlines t
						paragraph-ignore-fill-prefix)))
	    (goto-char begin)
	    (while (and (bolp) (not (eobp))) (forward-char 1))
	    (backward-paragraph)
	    (setq begin (point))
	    (goto-char end)
	    (skip-chars-backward " \t\n" begin)
	    (forward-paragraph)
	    (setq end (point))))

      (narrow-to-region (point-min) end)
      (unjustify-region begin (point-max))
      (put-text-property begin (point-max) 'justification style)
      (fill-region begin (point-max) nil t))))

(defun set-justification-none (b e)
  "Disable automatic filling for paragraphs in the region.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'none t))

(defun set-justification-left (b e)
  "Make paragraphs in the region left-justified.
This means lines are flush (lined up) at the left margin and ragged
on the right.
This is usually the default, but see the variable `default-justification'.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'left t))

(defun set-justification-right (b e)
  "Make paragraphs in the region right-justified.
This means lines are flush (lined up) at the right margin and ragged
on the left.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'right t))

(defun set-justification-full (b e)
  "Make paragraphs in the region fully justified.
This makes lines be lined up on both margins by inserting spaces between words.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'full t))

(defun set-justification-center (b e)
  "Make paragraphs in the region centered.
If the mark is not active, this applies to the current paragraph."
  (interactive (list (if mark-active (region-beginning) (point))
		     (if mark-active (region-end) (point))))
  (set-justification b e 'center t))

(defun justify-current-line (&optional how eop nosqueeze)
  "Do some kind of justification on this line.
Normally does full justification: adds spaces to the line to make it end at
the column given by `current-fill-column'.
Optional first argument HOW specifies alternate type of justification:
it can be `left', `right', `full', `center', or `none'; for their
meaning, see `default-justification'.
If HOW is t, will justify however the `current-justification' function says to.
If HOW is nil or missing, full justification is done by default.
Second arg EOP non-nil means that this is the last line of the paragraph, so
it will not be stretched by full justification.
Third arg NOSQUEEZE non-nil means to leave interior whitespace unchanged,
otherwise it is made canonical."
  (interactive "*")
  (if (eq t how) (setq how (or (current-justification) 'none))
    (if (null how) (setq how 'full)
      (or (memq how '(none left right center))
	  (setq how 'full))))
  (or (memq how '(none left))  ; No action required for these.
      (let ((fc (current-fill-column))
	    (pos (point-marker))
	    fp-end			; point at end of fill prefix
	    beg				; point at beginning of line's text
	    end				; point at end of line's text
	    indent			; column of `beg'
	    endcol			; column of `end'
	    ncols			; new indent point or offset
	    (nspaces 0)			; number of spaces between words
					; in line (not space characters)
	    (curr-fracspace 0)		; current fractional space amount
	    count)
	(end-of-line)
	;; Check if this is the last line of the paragraph.
	(if (and use-hard-newlines (null eop)
		 (get-text-property (point) 'hard))
	    (setq eop t))
	(skip-chars-backward " \t")
	;; Quick exit if it appears to be properly justified already
	;; or there is no text.
	(if (or (bolp)
		(and (memq how '(full right))
		     (= (current-column) fc)))
	    nil
	  (setq end (point))
	  (beginning-of-line)
	  (skip-chars-forward " \t")
	  ;; Skip over fill-prefix.
	  (if (and fill-prefix
		   (not (string-equal fill-prefix ""))
		   (equal fill-prefix
			  (buffer-substring
			   (point) (min (point-max) (+ (length fill-prefix)
						       (point))))))
	      (forward-char (length fill-prefix))
	    (if (and adaptive-fill-mode
		     (looking-at adaptive-fill-regexp))
		(goto-char (match-end 0))))
	  (setq fp-end (point))
	  (skip-chars-forward " \t")
	  ;; This is beginning of the line's text.
	  (setq indent (current-column))
	  (setq beg (point))
	  (goto-char end)
	  (setq endcol (current-column))

	  ;; HOW can't be null or left--we would have exited already
	  (cond ((eq 'right how)
		 (setq ncols (- fc endcol))
		 (if (< ncols 0)
		     ;; Need to remove some indentation
		     (delete-region
		      (progn (goto-char fp-end)
			     (if (< (current-column) (+ indent ncols))
				 (move-to-column (+ indent ncols) t))
			     (point))
		      (progn (move-to-column indent) (point)))
		   ;; Need to add some
		   (goto-char beg)
		   (indent-to (+ indent ncols))
		   ;; If point was at beginning of text, keep it there.
		   (if (= beg pos)
		       (move-marker pos (point)))))

		((eq 'center how)
		 ;; Figure out how much indentation is needed
		 (setq ncols (+ (current-left-margin)
				(/ (- fc (current-left-margin) ;avail. space
				      (- endcol indent)) ;text width
				   2)))
		 (if (< ncols indent)
		     ;; Have too much indentation - remove some
		     (delete-region
		      (progn (goto-char fp-end)
			     (if (< (current-column) ncols)
				 (move-to-column ncols t))
			     (point))
		      (progn (move-to-column indent) (point)))
		   ;; Have too little - add some
		   (goto-char beg)
		   (indent-to ncols)
		   ;; If point was at beginning of text, keep it there.
		   (if (= beg pos)
		       (move-marker pos (point)))))

		((eq 'full how)
		 ;; Insert extra spaces between words to justify line
		 (save-restriction
		   (narrow-to-region beg end)
		   (or nosqueeze
		       (canonically-space-region beg end))
		   (goto-char (point-max))
		   ;; count word spaces in line
		   (while (search-backward " " nil t)
		     (setq nspaces (1+ nspaces))
		     (skip-chars-backward " "))
		   (setq ncols (- fc endcol))
		   ;; Ncols is number of additional space chars needed
		   (when (and (> ncols 0) (> nspaces 0) (not eop))
                     (setq curr-fracspace (+ ncols (/ nspaces 2))
                           count nspaces)
                     (while (> count 0)
                       (skip-chars-forward " ")
                       (insert-char ?\s (/ curr-fracspace nspaces) t)
                       (search-forward " " nil t)
                       (setq count (1- count)
                             curr-fracspace
                             (+ (% curr-fracspace nspaces) ncols))))))
		(t (error "Unknown justification value"))))
	(goto-char pos)
	(move-marker pos nil)))
  nil)

(defun unjustify-current-line ()
  "Remove justification whitespace from current line.
If the line is centered or right-justified, this function removes any
indentation past the left margin.  If the line is full-justified, it removes
extra spaces between words.  It does nothing in other justification modes."
  (let ((justify (current-justification)))
    (cond ((eq 'left justify) nil)
	  ((eq  nil  justify) nil)
	  ((eq 'full justify)		; full justify: remove extra spaces
	   (beginning-of-line-text)
	   (canonically-space-region (point) (line-end-position)))
	  ((memq justify '(center right))
	   (save-excursion
	     (move-to-left-margin nil t)
	     ;; Position ourselves after any fill-prefix.
	     (if (and fill-prefix
		      (not (string-equal fill-prefix ""))
		      (equal fill-prefix
			     (buffer-substring
			      (point) (min (point-max) (+ (length fill-prefix)
							  (point))))))
		 (forward-char (length fill-prefix)))
	     (delete-region (point) (progn (skip-chars-forward " \t")
					   (point))))))))

(defun unjustify-region (&optional begin end)
  "Remove justification whitespace from region.
For centered or right-justified regions, this function removes any indentation
past the left margin from each line.  For full-justified lines, it removes
extra spaces between words.  It does nothing in other justification modes.
Arguments BEGIN and END are optional; default is the whole buffer."
  (save-excursion
    (save-restriction
      (if end (narrow-to-region (point-min) end))
      (goto-char (or begin (point-min)))
      (while (not (eobp))
	(unjustify-current-line)
	(forward-line 1)))))

(defun fill-nonuniform-paragraphs (min max &optional justifyp citation-regexp)
  "Fill paragraphs within the region, allowing varying indentation within each.
This command divides the region into \"paragraphs\",
only at paragraph-separator lines, then fills each paragraph
using as the fill prefix the smallest indentation of any line
in the paragraph.

When calling from a program, pass range to fill as first two arguments.

Optional third and fourth arguments JUSTIFYP and CITATION-REGEXP:
JUSTIFYP to justify paragraphs (prefix arg).
When filling a mail message, pass a regexp for CITATION-REGEXP
which will match the prefix of a line which is a citation marker
plus whitespace, but no other kind of prefix.
Also, if CITATION-REGEXP is non-nil, don't fill header lines."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (let ((fill-individual-varying-indent t))
    (fill-individual-paragraphs min max justifyp citation-regexp)))

(defun fill-individual-paragraphs (min max &optional justify citation-regexp)
  "Fill paragraphs of uniform indentation within the region.
This command divides the region into \"paragraphs\",
treating every change in indentation level or prefix as a paragraph boundary,
then fills each paragraph using its indentation level as the fill prefix.

There is one special case where a change in indentation does not start
a new paragraph.  This is for text of this form:

   foo>    This line with extra indentation starts
   foo> a paragraph that continues on more lines.

These lines are filled together.

When calling from a program, pass the range to fill
as the first two arguments.

Optional third and fourth arguments JUSTIFY and CITATION-REGEXP:
JUSTIFY to justify paragraphs (prefix arg).
When filling a mail message, pass a regexp for CITATION-REGEXP
which will match the prefix of a line which is a citation marker
plus whitespace, but no other kind of prefix.
Also, if CITATION-REGEXP is non-nil, don't fill header lines."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning) (region-end)
		       (if current-prefix-arg 'full))))
  (save-restriction
    (save-excursion
      (goto-char min)
      (beginning-of-line)
      (narrow-to-region (point) max)
      (if citation-regexp
	  (while (and (not (eobp))
		      (or (looking-at "[ \t]*[^ \t\n]+:")
			  (looking-at "[ \t]*$")))
	    (if (looking-at "[ \t]*[^ \t\n]+:")
		(search-forward "\n\n" nil 'move)
	      (forward-line 1))))
      (narrow-to-region (point) max)
      ;; Loop over paragraphs.
      (while (progn
	       ;; Skip over all paragraph-separating lines
	       ;; so as to not include them in any paragraph.
               (while (and (not (eobp))
			   (progn (move-to-left-margin)
				  (and (not (eobp))
				       (looking-at paragraph-separate))))
                 (forward-line 1))
               (skip-chars-forward " \t\n") (not (eobp)))
	(move-to-left-margin)
	(let ((start (point))
	      fill-prefix fill-prefix-regexp)
	  ;; Find end of paragraph, and compute the smallest fill-prefix
	  ;; that fits all the lines in this paragraph.
	  (while (progn
		   ;; Update the fill-prefix on the first line
		   ;; and whenever the prefix good so far is too long.
		   (if (not (and fill-prefix
				 (looking-at fill-prefix-regexp)))
		       (setq fill-prefix
			     (fill-individual-paragraphs-prefix
			      citation-regexp)
			     fill-prefix-regexp (regexp-quote fill-prefix)))
		   (forward-line 1)
		   (if (bolp)
		       ;; If forward-line went past a newline,
		       ;; move further to the left margin.
		       (move-to-left-margin))
		   ;; Now stop the loop if end of paragraph.
		   (and (not (eobp))
			(if fill-individual-varying-indent
			    ;; If this line is a separator line, with or
			    ;; without prefix, end the paragraph.
			    (and
			     (not (looking-at paragraph-separate))
			     (save-excursion
			       (not (and (looking-at fill-prefix-regexp)
					 (progn (forward-char
						 (length fill-prefix))
						(looking-at
						 paragraph-separate))))))
			  ;; If this line has more or less indent
			  ;; than the fill prefix wants, end the paragraph.
			  (and (looking-at fill-prefix-regexp)
			       ;; If fill prefix is shorter than a new
			       ;; fill prefix computed here, end paragraph.
 			       (let ((this-line-fill-prefix
				      (fill-individual-paragraphs-prefix
				       citation-regexp)))
 				 (>= (length fill-prefix)
 				     (length this-line-fill-prefix)))
			       (save-excursion
				 (not (progn (forward-char
					      (length fill-prefix))
					     (or (looking-at "[ \t]")
						 (looking-at paragraph-separate)
						 (looking-at paragraph-start)))))
			       (not (and (equal fill-prefix "")
					 citation-regexp
					 (looking-at citation-regexp))))))))
	  ;; Fill this paragraph, but don't add a newline at the end.
	  (let ((had-newline (bolp)))
	    (fill-region-as-paragraph start (point) justify)
	    (if (and (bolp) (not had-newline))
		(delete-char -1))))))))

(defun fill-individual-paragraphs-prefix (citation-regexp)
  (let* ((adaptive-fill-first-line-regexp ".*")
	 (just-one-line-prefix
	  ;; Accept any prefix rather than just the ones matched by
	  ;; adaptive-fill-first-line-regexp.
	  (fill-context-prefix (point) (line-beginning-position 2)))
	 (two-lines-prefix
	  (fill-context-prefix (point) (line-beginning-position 3))))
    (if (not just-one-line-prefix)
	(buffer-substring
	 (point) (save-excursion (skip-chars-forward " \t") (point)))
	;; See if the citation part of JUST-ONE-LINE-PREFIX
	;; is the same as that of TWO-LINES-PREFIX,
	;; except perhaps with longer whitespace.
      (if (and just-one-line-prefix two-lines-prefix
	       (let* ((one-line-citation-part
		       (fill-individual-paragraphs-citation
			just-one-line-prefix citation-regexp))
		      (two-lines-citation-part
		       (fill-individual-paragraphs-citation
			two-lines-prefix citation-regexp))
		      (adjusted-two-lines-citation-part
		       (substring two-lines-citation-part 0
				  (string-match "[ \t]*\\'"
						two-lines-citation-part))))
		 (and
		 (string-match (concat "\\`"
				       (regexp-quote
					adjusted-two-lines-citation-part)
				       "[ \t]*\\'")
			       one-line-citation-part)
		 (>= (string-width one-line-citation-part)
		      (string-width two-lines-citation-part)))))
	    two-lines-prefix
	just-one-line-prefix))))

(defun fill-individual-paragraphs-citation (string citation-regexp)
  (if citation-regexp
      (if (string-match citation-regexp string)
	  (match-string 0 string)
	"")
    string))

(defun fill-region-as-paragraph-semlf (from to &optional justify
                                            nosqueeze squeeze-after)
  "Fill the region using semantic linefeeds as if it were a single paragraph.
This command removes any paragraph breaks in the region and extra
newlines at the end, and fills lines within the region.  Text is
refilled putting a newline character after each sentence, calling
`forward-sentence' to find the ends of sentences.  If
`sentence-end-double-space' is non-nil, period followed by one space is
not the end of a sentence.

If JUSTIFY is non-nil (interactively, with prefix argument), justify as
well.  If NOSQUEEZE is non-nil, do not to make spaces between words
canonical before filling.  SQUEEZE-AFTER, if non-nil, should be a buffer
position; it means canonicalize spaces only starting from that position.
See `canonically-space-region' for the meaning of canonicalization of
spaces.  The variable `fill-column' controls the width for filling.

Return the `fill-prefix' used for filling.

This function can be assigned to `fill-region-as-paragraph-function' to
override how functions like `fill-paragraph' and `fill-region' fill
text.

For more details about semantic linefeeds, see URL `https://sembr.org/'
and URL `https://rhodesmill.org/brandon/2012/one-sentence-per-line/'."
  (interactive (progn
		 (barf-if-buffer-read-only)
		 (list (region-beginning)
                       (region-end)
		       (if current-prefix-arg 'full))))

  (let ((from (min from to))
        (to (copy-marker (max from to) t))
        pfx)
    (goto-char from)
    (let ((fill-column most-positive-fixnum))
      (setq pfx (or (save-excursion
                      (fill-region-as-paragraph-default (point)
                                                        to
                                                        nil
                                                        nosqueeze
                                                        squeeze-after))
                    "")))
    (while (< (point) to)
      (let ((fill-to (copy-marker
                      (min to
                           (save-excursion
                             (forward-sentence)
                             (point)))
                      t))
            (fill-prefix pfx))
	(fill-region-as-paragraph-default (point)
				          fill-to
				          justify
                                          t)
        (goto-char fill-to))
      (when (and (> (point) (line-beginning-position))
		 (< (point) (line-end-position))
                 (< (point) to))
	(delete-horizontal-space)
	(insert "\n")
	(insert pfx)))
    pfx))

(defvar comment-start-skip nil
  "Regexp to match the start of a comment plus everything up to its body.")

(defvar mail-citation-prefix nil)

(defvar fill-individual-varying-indent nil)
(defvar colon-double-space nil)
(defvar fill-separate-heterogeneous-words-with-space nil)
(defvar fill-paragraph-function nil)
(defvar fill-paragraph-handle-comment t)
(defvar enable-kinsoku t)
(defvar fill-indent-according-to-mode nil)
(defvar current-fill-column--has-warned nil)
(defvar fill-nobreak-predicate nil)
(defvar fill-nobreak-invisible nil)
(defvar fill-find-break-point-function-table (make-char-table nil))
(defvar fill-nospace-between-words-table (make-char-table nil))
(defvar fill-region-as-paragraph-function #'fill-region-as-paragraph-default)
(defvar fill-forward-paragraph-function 'forward-paragraph)


(defvar lisp-interaction-mode-map
  (let ((m (make-sparse-keymap))
        (menu (make-sparse-keymap)))
    (define-key m [menu-bar] menu)
    (define-key menu [lisp-interaction]
      (list 'menu-item "Lisp-Interaction" (make-sparse-keymap)))
    (define-key m "\C-j" 'eval-print-last-sexp)
    m)
  "Keymap for Lisp Interaction mode.")

(defvar emacs-lisp-mode-map
  (let ((m (make-sparse-keymap)))
    (define-key m "\C-j" 'eval-print-last-sexp)
    m)
  "Keymap for Emacs Lisp mode.")

;; *scratch* starts in lisp-interaction-mode.
(when (get-buffer "*scratch*")
  (with-current-buffer "*scratch*"
    (use-local-map lisp-interaction-mode-map)))
"#;
