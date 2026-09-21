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

(defmacro save-current-buffer (&rest body)
  "Save the current buffer; execute BODY; restore the current buffer."
  (let ((old-buf (make-symbol "old-buffer")))
    (list 'let (list (list old-buf '(current-buffer)))
          (list 'unwind-protect
                (cons 'progn body)
                (list 'if (list 'buffer-live-p old-buf)
                      (list 'set-buffer old-buf))))))

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

(defun move-to-window-line (arg)
  "Position point relative to the window (approximate)."
  (interactive "P")
  arg
  (beginning-of-line))

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

;; ---------- mode keymaps ----------
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
