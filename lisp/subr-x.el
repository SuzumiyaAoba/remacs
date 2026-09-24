;;; subr-x.el --- subr-x definitions missing from the prelude -*- lexical-binding: t -*-

;; Copyright (C) 2013-2025 Free Software Foundation, Inc.

;; This file is part of GNU Emacs.

;; GNU Emacs is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; GNU Emacs is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; remacs: the prelude already defines the bulk of subr-x (if-let*,
;; when-let*, string-join, string-trim, etc.).  This file contains the
;; remaining subr-x definitions, copied from GNU Emacs 31's
;; lisp/emacs-lisp/subr-x.el, plus a few leaf helpers from subr.el,
;; minibuffer.el and mule-util.el that they depend on.

;;; Code:

(require 'pcase)

;;; Leaf helpers (subr.el / minibuffer.el / mule-util.el).

(defun process-live-p (process)
  "Return non-nil if PROCESS is alive.
A process is considered alive if its status is `run', `open',
`listen', `connect' or `stop'.  Value is nil if PROCESS is not a
process."
  (and (processp process)
       (memq (process-status process)
	     '(run open listen connect stop))))

(defvar minibuffer-default-prompt-format " (default %s)"
  "Format string for the default part of the minibuffer prompt.")

(defun format-prompt (prompt default &rest format-args)
  "Format PROMPT with DEFAULT according to `minibuffer-default-prompt-format'.
If FORMAT-ARGS is nil, PROMPT is used as a plain string.  If
FORMAT-ARGS is non-nil, PROMPT is used as a format control
string, and FORMAT-ARGS are the arguments to be substituted into
it.  See `format' for details.

If DEFAULT is a list, the first element is used as the default.
If not, the element is used as is.

If DEFAULT is nil or an empty string, no \"default value\" string
is included in the return value."
  (concat
   (if (null format-args)
       (substitute-command-keys prompt)
     (apply #'format (substitute-command-keys prompt) format-args))
   (and default
        (or (not (stringp default))
            (length> default 0))
        (format (substitute-command-keys minibuffer-default-prompt-format)
                (if (consp default)
                    (car default)
                  default)))
   ": "))

(defvar truncate-string-ellipsis nil
  "String to use to indicate truncation.
Serves as default value of ELLIPSIS argument to `truncate-string-to-width'
returned by the function `truncate-string-ellipsis'.")

(defun char-displayable-p (char)
  "Return non-nil if we should be able to display CHAR.
remacs approximation: ASCII and printable-width characters."
  (and (characterp char)
       (or (< char 128)
           (> (char-width char) 0))))

(defun truncate-string-ellipsis ()
  "Return the string used to indicate truncation.
Use the value of the variable `truncate-string-ellipsis' when it's non-nil.
Otherwise, return the Unicode character U+2026 \"HORIZONTAL ELLIPSIS\"
when it's displayable on the selected frame, or `...'.  This function
needs to be called on every use of `truncate-string-to-width' to
decide whether the selected frame can display that Unicode character."
  (cond
   (truncate-string-ellipsis)
   ((char-displayable-p ?…) "…")
   ("...")))

(defun unrecord-window-buffer (window buffer &optional all-frames)
  "Mark WINDOW's association with BUFFER as forgotten.
remacs stub: window buffer history is not tracked, so this is a no-op."
  nil)

;;; Threading.

(defmacro internal--thread-argument (first? &rest forms)
  "Internal implementation for `thread-first' and `thread-last'.
When Argument FIRST? is non-nil argument is threaded first, else
last.  FORMS are the expressions to be threaded."
  (pcase forms
    (`(,x (,f . ,args) . ,rest)
     `(internal--thread-argument
       ,first? ,(if first? `(,f ,x ,@args) `(,f ,@args ,x)) ,@rest))
    (`(,x ,f . ,rest) `(internal--thread-argument ,first? (,f ,x) ,@rest))
    (_ (car forms))))

;;; Hash tables.

(defsubst hash-table-empty-p (hash-table)
  "Check whether HASH-TABLE is empty (has 0 elements)."
  (declare (side-effect-free t))
  (zerop (hash-table-count hash-table)))

;;; Strings.

(defsubst string-remove-prefix (prefix string)
  "Remove PREFIX from STRING if present."
  (declare (pure t) (side-effect-free t))
  (if (string-prefix-p prefix string)
      (substring string (length prefix))
    string))

(defsubst string-remove-suffix (suffix string)
  "Remove SUFFIX from STRING if present."
  (declare (pure t) (side-effect-free t))
  (if (string-suffix-p suffix string)
      (substring string 0 (- (length string) (length suffix)))
    string))

(defun string-truncate-left (string length)
  "If STRING is longer than LENGTH, return a truncated version.
When truncating, \"...\" is always prepended to the string, so
the resulting string may be longer than the original if LENGTH is
3 or smaller."
  (declare (pure t) (side-effect-free t))
  (let ((strlen (length string)))
    (if (<= strlen length)
	string
      (setq length (max 0 (- length 3)))
      (concat "..." (substring string (min (1- strlen)
                                           (max 0 (- strlen length))))))))

;;; `named-let'.

(defmacro named-let (name bindings &rest body)
  "Looping construct taken from Scheme.
Like `let', bind variables in BINDINGS and then evaluate BODY,
but with the twist that BODY can evaluate itself recursively by
calling NAME, where the arguments passed to NAME are used
as the new values of the bound variables in the recursive invocation.

This construct can only be used with lexical binding."
  (declare (indent 2) (debug (symbolp (&rest (symbolp form)) body)))
  (require 'cl-lib)
  (unless lexical-binding
    (error "`named-let' requires lexical binding"))
  (let ((fargs (mapcar (lambda (b) (if (consp b) (car b) b)) bindings))
        (aargs (mapcar (lambda (b) (if (consp b) (cadr b))) bindings)))
    ;; remacs: `cl-labels' is a dynamic-extent (fset) shim, so the bound
    ;; function value cannot escape its extent via #'name the way GNU's
    ;; `(funcall (cl-labels ...) #'name)' does.  Instead the initial call
    ;; is placed inside the cl-labels body, where direct calls to NAME
    ;; (including recursive ones) resolve through the function cell.
    `(cl-labels ((,name ,fargs . ,body))
       (,name . ,aargs))))

;;; Work buffers.

(defvar work-buffer--list nil)
(defvar work-buffer-limit 10
  "Maximum number of reusable work buffers.
When this limit is exceeded, newly allocated work buffers are
automatically killed, which means that in a such case
`with-work-buffer' becomes equivalent to `with-temp-buffer'.")

(defsubst work-buffer--get ()
  "Get a work buffer."
  (let ((buffer (pop work-buffer--list)))
    (if (buffer-live-p buffer)
        buffer
      ;; GNU passes t for INHIBIT-BUFFER-HOOKS; remacs's
      ;; `generate-new-buffer' does not take that argument yet.
      (generate-new-buffer " *work*"))))

(defun work-buffer--release (buffer)
  "Release work BUFFER."
  (if (buffer-live-p buffer)
      (with-current-buffer buffer
        ;; Flush BUFFER before making it available again, i.e. clear
        ;; its contents, remove all overlays and buffer-local
        ;; variables.  Is it enough to safely reuse the buffer?
        (let ((inhibit-read-only t))
          (erase-buffer)
          (delete-all-overlays))
        (let (change-major-mode-hook)
          ;; `kill-all-local-variables' does not kill permanent locals
          ;; like `buffer-read-only'.  (GNU passes t for KILL-PERMANENT;
          ;; remacs's version takes no argument.)
          (setq buffer-read-only nil)
          (kill-all-local-variables))
        ;; Make the buffer available again.
        (push buffer work-buffer--list)))
  ;; If the maximum number of reusable work buffers is exceeded, kill
  ;; work buffer in excess, taking into account that the limit could
  ;; have been let-bound to temporarily increase its value.
  (when (> (length work-buffer--list) work-buffer-limit)
    (mapc #'kill-buffer (nthcdr work-buffer-limit work-buffer--list))
    (setq work-buffer--list (ntake work-buffer-limit work-buffer--list))))

(defmacro with-work-buffer (&rest body)
  "Create a work buffer, and evaluate BODY there like `progn'.
Like `with-temp-buffer', but reuse an already created temporary
buffer when possible, instead of creating a new one on each call.
Avoid retaining state referring to a work buffer, and kill
any indirect buffers you create that use a work buffer as a base."
  (declare (indent 0) (debug t))
  (let ((work-buffer (make-symbol "work-buffer")))
    `(let ((,work-buffer (work-buffer--get)))
       (with-current-buffer ,work-buffer
         (unwind-protect
             (progn ,@body)
           (work-buffer--release ,work-buffer))))))

(defun work-buffer--prepare-pixelwise (string buffer)
  "Set up the current buffer to correctly compute STRING's pixel width.
Call this with a work buffer as the current buffer.
BUFFER is the originating buffer and if non-nil, make the current
buffer's (work buffer) face remappings match it."
  (when buffer
    (dolist (v '(face-remapping-alist
                 char-property-alias-alist
                 default-text-properties))
      (if (local-variable-p v buffer)
          (set (make-local-variable v)
               (buffer-local-value v buffer)))))
  ;; Avoid deactivating the region as side effect.
  (let (deactivate-mark)
    (insert string))
  ;; If `display-line-numbers' is enabled in internal
  ;; buffers (e.g. globally), it breaks width calculation
  ;; (bug#59311).  Disable `line-prefix' and `wrap-prefix',
  ;; for the same reason.
  (add-text-properties
   (point-min) (point-max)
   '(display-line-numbers-disable t line-prefix "" wrap-prefix "")))

(defun truncate-string-pixelwise (string max-pixels &optional buffer
                                  ellipsis ellipsis-pixels)
  "Return STRING truncated to fit within MAX-PIXELS.
If BUFFER is non-nil, use the face remappings, alternative and default
properties from that buffer when determining the width.
If you call this function to measure pixel width of a string
with embedded newlines, it returns the width of the widest
substring that does not include newlines.

If ELLIPSIS is non-nil, it should be a string which will replace the end
of STRING if it extends beyond MAX-PIXELS, unless the pixel width of
STRING is equal to or less than the pixel width of ELLIPSIS.  If it is
non-nil and not a string, then ELLIPSIS defaults to
`truncate-string-ellipsis', or to three dots when it's nil.

If ELLIPSIS-PIXELS is non-nil, it is the pixel width of ELLIPSIS, and
can be used to avoid the cost of recomputing this for multiple calls to
this function using the same ELLIPSIS."
  (declare (important-return-value t))
  (if (zerop (length string))
      string
    ;; Keeping a work buffer around is more efficient than creating a
    ;; new temporary buffer.
    (let* ((window (selected-window))
           (original-buffer (window-buffer window))
           (window-dedication (window-dedicated-p window))
           (buffer-list-update-hook)
           (window-scroll-functions)
           (window-configuration-change-hook))
      (with-work-buffer
        ;; Use a binary search to prune the number of calls to
        ;; `window-text-pixel-size'.
        ;; These are 1-based buffer indexes.
        (unwind-protect
            (let* ((low 1)
                   (high (1+ (length string)))
                   mid)
              (work-buffer--prepare-pixelwise string buffer)
              (set-window-dedicated-p window nil)
              (set-window-buffer window (current-buffer) 'keep-margins)
              (when (> (car (window-text-pixel-size nil 1 high)) max-pixels)
                (when (and ellipsis (not (stringp ellipsis)))
                  (setq ellipsis (truncate-string-ellipsis)))
                (setq ellipsis-pixels (if ellipsis
                                          (if ellipsis-pixels
                                              ellipsis-pixels
                                            (string-pixel-width ellipsis buffer))
                                        0))
                (let ((adjusted-pixels
                       (if (> max-pixels ellipsis-pixels)
                           (- max-pixels ellipsis-pixels)
                         max-pixels)))
                  (while (<= low high)
                    (setq mid (floor (+ low high) 2))
                    (if (<= (car (window-text-pixel-size nil 1 mid))
                            adjusted-pixels)
                        (setq low (1+ mid))
                      (setq high (1- mid))))))
              (if mid
                  ;; Binary search ran.
                  (if (and ellipsis (> max-pixels ellipsis-pixels))
                      (concat (substring string 0 (1- high)) ellipsis)
                    (substring string 0 (1- high)))
                ;; Fast path.
                string))
          (set-window-buffer window original-buffer 'keep-margins)
          (set-window-dedicated-p window window-dedication)
          (unrecord-window-buffer window (current-buffer) t))))))

;;; Display text properties.

(defun add-remove--display-text-property (start end spec value
                                                &optional object remove)
  (let ((sub-start start)
        (sub-end 0)
        (limit (if (stringp object)
                   (min (length object) end)
                 (min end (point-max))))
        disp)
    (while (< sub-end end)
      (setq sub-end (next-single-property-change sub-start 'display object
                                                 limit))
      (if (not (setq disp (get-text-property sub-start 'display object)))
          ;; No old properties in this range.
          (unless remove
            (put-text-property sub-start sub-end 'display (list spec value)
                               object))
        ;; We have old properties.
        (let ((changed nil)
              type)
          ;; Make disp into a list.
          (setq disp
                (cond
                 ((vectorp disp)
                  (setq type 'vector)
                  (seq-into disp 'list))
                 ((or (not (consp (car-safe disp)))
                      ;; If disp looks like ((margin ...) ...), that's
                      ;; still a single display specification.
                      (eq (caar disp) 'margin))
                  (setq type 'scalar)
                  (list disp))
                 (t
                  (setq type 'list)
                  disp)))
          ;; Remove any old instances.
          (when-let* ((old (assoc spec disp)))
            ;; If the property value was a list, don't modify the
            ;; original value in place; it could be used by other
            ;; regions of text.
            (setq disp (if (eq type 'list)
                           (remove old disp)
                         (delete old disp))
                  changed t))
          (unless remove
            (setq disp (cons (list spec value) disp)
                  changed t))
          (when changed
            (if (not disp)
                (remove-text-properties sub-start sub-end '(display nil) object)
              (when (eq type 'vector)
                (setq disp (seq-into disp 'vector)))
              ;; Finally update the range.
              (put-text-property sub-start sub-end 'display disp object)))))
      (setq sub-start sub-end))))

(defun add-display-text-property (start end spec value &optional object)
  "Add the display specification (SPEC VALUE) to the text from START to END.
If any text in the region has a non-nil `display' property, the existing
display specifications are retained.

OBJECT is either a string or a buffer to add the specification to.
If omitted, OBJECT defaults to the current buffer."
  (add-remove--display-text-property start end spec value object))

(defun remove-display-text-property (start end spec &optional object)
  "Remove the display specification SPEC from the text from START to END.
SPEC is the car of the display specification to remove, e.g. `height'.
If any text in the region has other display specifications, those specs
are retained.

OBJECT is either a string or a buffer to remove the specification from.
If omitted, OBJECT defaults to the current buffer."
  (add-remove--display-text-property start end spec nil object 'remove))

;;; Misc.

(defun read-process-name (prompt)
  "Query the user for a process and return the process object."
  ;; Currently supports only the PROCESS argument.
  ;; Must either return a list containing a process, or signal an error.
  ;; (Returning nil would mean the current buffer's process.)
  (unless (fboundp 'process-list)
    (error "Asynchronous subprocesses are not supported on this system"))
  ;; Local function to return cons of a complete-able name, and the
  ;; associated process object, for use with `completing-read'.
  (cl-flet ((procitem
             (p) (when (process-live-p p)
                   (let ((pid (process-id p))
                         (procname (process-name p))
                         (procbuf (process-buffer p)))
                     (and (eq (process-type p) 'real)
                          (cons (if procbuf
                                    (format "%s (%s) in buffer %s"
                                            procname pid
                                            (buffer-name procbuf))
                                  (format "%s (%s)" procname pid))
                                p))))))
    ;; Perform `completing-read' for a process.
    (let* ((currproc (get-buffer-process (current-buffer)))
           (proclist (or (process-list)
                         (error "No processes found")))
           (collection (delq nil (mapcar #'procitem proclist)))
           (selection (completing-read
                       (format-prompt prompt
                                      (and currproc
                                           (eq (process-type currproc) 'real)
                                           (procitem currproc)))
                       collection nil :require-match nil nil
                       (car (seq-find (lambda (proc)
                                        (eq currproc (cdr proc)))
                                      collection))))
           (process (and selection
                         (cdr (assoc selection collection)))))
      (unless process
        (error "No process selected"))
      process)))

(defun emacs-etc--hide-local-variables ()
  "Hide local variables.
Used by `emacs-authors-mode' and `emacs-news-mode'."
  (narrow-to-region (point-min)
                    (save-excursion
                      (goto-char (point-max))
                      ;; Obfuscate to avoid this being interpreted
                      ;; as a local variable section itself.
                      (if (re-search-backward "^Local\sVariables:$" nil t)
                          (progn (forward-line -1) (point))
                        (point-max)))))

(defun take-while (pred list)
  "Return the successive elements of LIST for which PRED returns non-nil.
PRED is called with each element of LIST, in order, until PRED
returns nil, at which point the elements gathered so far are returned."
  (let (result)
    (while (and list (funcall pred (car list)))
      (push (pop list) result))
    (nreverse result)))

(defmacro and-let* (varlist &rest body)
  "Bind variables according to VARLIST and conditionally evaluate BODY.
Like `when-let*', except that BODY is only evaluated when every
binding's value is non-nil.

Each element of VARLIST is a list (SYMBOL VALUEFORM) which binds
SYMBOL to the value of VALUEFORM.  An element can additionally be
of the form (VALUEFORM), which is evaluated and checked for nil;
i.e. SYMBOL can be omitted if only the test result is of interest.
It can also be of the form (SYMBOL), which is a shorthand
for (SYMBOL SYMBOL)."
  (declare (indent 1))
  (if (null varlist)
      (cons 'progn body)
    (let* ((binding (car varlist))
           (spec (if (consp binding) binding (list binding binding)))
           (var (car spec))
           (val (if (cdr spec) (cadr spec) var)))
      (if (symbolp var)
          `(let ((,var ,val))
             (when ,var
               (and-let* ,(cdr varlist) ,@body)))
        `(when ,var
           (and-let* ,(cdr varlist) ,@body))))))

(defalias 'string-split #'split-string)

;;; Motion helpers (GNU subr.el/simple.el).

(defvar word-move-empty-char-table nil
  "Used in `forward-word-strictly' and `backward-word-strictly'
to countermand the effect of `find-word-boundary-function-table'.")

(defvar find-word-boundary-function-table nil
  "Char-table of functions to search for a word boundary.")

(defun forward-word-strictly (&optional arg)
  "Move point forward ARG words (backward if ARG is negative).
If ARG is omitted or nil, move point forward one word.
Normally returns t.
If an edge of the buffer or a field boundary is reached, point is left there
and the function returns nil.  Field boundaries are not noticed if
`inhibit-field-text-motion' is non-nil.

This function is like `forward-word', but it is not affected
by `find-word-boundary-function-table'.  It is also not interactive."
  (let ((find-word-boundary-function-table
         (if (char-table-p word-move-empty-char-table)
             word-move-empty-char-table
           (setq word-move-empty-char-table (make-char-table nil)))))
    (forward-word (or arg 1))))

(defun backward-word-strictly (&optional arg)
  "Move point backward ARG words (forward if ARG is negative).
If ARG is omitted or nil, move point backward one word.
Normally returns t.
If an edge of the buffer or a field boundary is reached, point is left there
and the function returns nil.  Field boundaries are not noticed if
`inhibit-field-text-motion' is non-nil.

This function is like `backward-word', but it is not affected
by `find-word-boundary-function-table'.  It is also not interactive."
  (forward-word-strictly (- (or arg 1))))

(defun forward-to-indentation (&optional arg)
  "Move forward ARG lines and position at first nonblank character.
Return the number of whitespace characters skipped."
  (interactive "^p")
  (forward-line (or arg 1))
  (skip-chars-forward " \t"))

(defun left-word (&optional n)
  "Move point N words to the left (to the right if N is negative).
Depending on the bidirectional context, this may move point backward
or forward in the buffer."
  (interactive "^p")
  (if (eq (current-bidi-paragraph-direction) 'left-to-right)
      (backward-word n)
    (forward-word n)))

(defun right-word (&optional n)
  "Move point N words to the right (to the left if N is negative).
Depending on the bidirectional context, this may move point forward
or backward in the buffer."
  (interactive "^p")
  (if (eq (current-bidi-paragraph-direction) 'left-to-right)
      (forward-word n)
    (backward-word n)))

;;; files.el/simple.el members.

(defun cd (dir)
  "Make DIR become the current buffer's default directory.
Return the name of the new default directory."
  (interactive "DChange default directory: ")
  (setq dir (expand-file-name dir))
  (unless (file-directory-p dir)
    (error "No such directory: %s" dir))
  (setq dir (file-name-as-directory dir)
        default-directory dir))

(define-minor-mode indent-tabs-mode
  "Toggle whether indentation can insert TAB characters."
  :variable indent-tabs-mode)

;;; Aliases and simple predicates (GNU subr.el).

(defalias 'drop #'nthcdr)
(defalias 'chmod #'set-file-modes)
(defalias 'mkdir #'make-directory)
(defalias 'backward-delete-char #'delete-backward-char)
(defalias 'any #'member-if)
(define-obsolete-function-alias 'user-original-login-name
  #'user-login-name "28.1")

(defsubst xor (cond1 cond2)
  "Return the boolean exclusive-or of COND1 and COND2.
If only one of the arguments is non-nil, return it; otherwise
return nil."
  (declare (pure t) (side-effect-free error-free))
  (cond ((not cond1) cond2)
        ((not cond2) cond1)))

(defun plistp (object)
  "Non-nil if and only if OBJECT is a valid plist."
  (declare (pure t) (side-effect-free error-free))
  (let ((len (proper-list-p object)))
    (and len (zerop (% len 2)))))

(defun integer-or-null-p (object)
  "Return t if OBJECT is either an integer or nil."
  (declare (pure t) (side-effect-free error-free))
  (or (integerp object)
      (null object)))

(defun log10 (x)
  "Return (log X 10), the log base 10 of X."
  (declare (side-effect-free t))
  (log x 10))

(defun hash-table-contains-p (key table)
  "Check whether TABLE contains an element with key KEY.
Note: this is different from `gethash', which returns nil both
when KEY is not in TABLE, and when KEY is in TABLE with a nil
value."
  (let ((sentinel (make-symbol "sentinel")))
    (not (eq sentinel (gethash key table sentinel)))))

(defun ensure-proper-list (object)
  "Return OBJECT as a list.
If OBJECT is already a proper list, return OBJECT itself.  If it's not a
proper list, return a one-element list containing OBJECT.

`ensure-list' is usually preferable because that function runs in
constant time, but this one has to traverse the whole of OBJECT."
  (if (proper-list-p object)
      object
    (list object)))

(defun set-local (symbol newval)
  "Make VARIABLE buffer local and set it to VALUE."
  (set (make-local-variable symbol) newval))

(defsubst subr-primitive-p (object)
  "Return t if OBJECT is a built-in primitive written in C.
Such objects can be functions or special forms."
  (declare (side-effect-free error-free))
  (and (subrp object)
       (not (and (fboundp 'native-comp-function-p)
                 (native-comp-function-p object)))))

(defsubst primitive-function-p (object)
  "Return t if OBJECT is a built-in primitive function.
This excludes special forms, since they are not functions."
  (declare (side-effect-free error-free))
  (and (subrp object)
       (not (or (and (fboundp 'native-comp-function-p)
                     (native-comp-function-p object))
                (eq (cdr (subr-arity object)) 'unevalled)))))

(defun function-alias-p (func &optional _noerror)
  "Return nil if FUNC is not a function alias.
If FUNC is a function alias, return the function alias chain."
  (declare (side-effect-free error-free))
  (let ((chain nil))
    (while (and (symbolp func)
                (setq func (symbol-function func))
                (symbolp func))
      (push func chain))
    (nreverse chain)))

;;; Conditional compilation macros (GNU subr.el).

(defmacro static-if (condition then-form &rest else-forms)
  "A conditional compilation macro.
Evaluate CONDITION at macro-expansion time.  If it is non-nil,
expand the macro to THEN-FORM.  Otherwise expand it to ELSE-FORMS
enclosed in a `progn' form.  ELSE-FORMS may be empty."
  (declare (indent 2)
           (debug (sexp sexp &rest sexp)))
  (if (eval condition lexical-binding)
      then-form
    (cons 'progn else-forms)))

(defmacro static-when (condition &rest body)
  "Expand BODY when CONDITION evaluates to non-nil at macro-expansion time."
  (declare (indent 1) (debug (sexp &rest sexp)))
  (and (eval condition lexical-binding)
       (cons 'progn body)))

(defmacro static-unless (condition &rest body)
  "Expand BODY when CONDITION evaluates to nil at macro-expansion time."
  (declare (indent 1) (debug (sexp &rest sexp)))
  (and (not (eval condition lexical-binding))
       (cons 'progn body)))

(defmacro noreturn (form)
  "Evaluate FORM, with the expectation that it does not return.
If FORM does return, signal an error."
  (declare (debug t))
  `(prog1 ,form
     (error "Form marked with `noreturn' did return")))

(defmacro 1value (form)
  "Evaluate FORM, expecting a constant return value.
This is the global do-nothing version.  There is also `testcover-1value'
that complains if FORM ever does return differing values."
  (declare (debug t))
  form)

(defmacro while-let (spec &rest body)
  "Bind variables according to SPEC and conditionally evaluate BODY.
Evaluate each binding in turn, stopping if a binding value is nil.
If all bindings are non-nil, eval BODY and repeat.
The variable list SPEC is the same as in `if-let*'."
  (declare (indent 1) (debug if-let))
  (let ((done (gensym "done")))
    `(catch ',done
       (while t
         (if-let* ,spec
             (progn
               ,@body)
           (throw ',done nil))))))

;;; Region/buffer manipulation (GNU subr.el).

(defmacro with-restriction (start end &rest rest)
  "Execute BODY with restrictions set to START and END.
The current restrictions, if any, are restored upon return.
When the optional LABEL argument, which is evaluated to get the
label to use and must yield a non-nil value, is present, inside
BODY, `narrow-to-region' and `widen' can be used only within the
START and END limits.  To gain access to other portions of the
buffer, use `without-restriction' with the same LABEL argument.
\(fn START END [:label LABEL] BODY)"
  (declare (indent 2) (debug t))
  (if (eq (car rest) :label)
      `(save-restriction
         ;; remacs: no `internal--labeled-narrow-to-region' yet;
         ;; the label is currently ignored.
         (progn ,(cadr rest)
                (narrow-to-region ,start ,end))
         ,@(cddr rest))
    `(save-restriction (narrow-to-region ,start ,end) ,@rest)))

(defmacro without-restriction (&rest rest)
  "Execute BODY without restrictions.
The current restrictions, if any, are restored upon return.
When the optional LABEL argument is present, the restrictions set
by `with-restriction' with the same LABEL argument are lifted.
\(fn [:label LABEL] BODY)"
  (declare (indent 0) (debug t))
  (if (eq (car rest) :label)
      `(save-restriction
         (progn ,(cadr rest)
                (widen))
         ,@(cddr rest))
    `(save-restriction (widen) ,@rest)))

(defmacro with-existing-directory (&rest body)
  "Execute BODY with `default-directory' bound to an existing directory.
If `default-directory' is already an existing directory, it's not changed."
  (declare (indent 0) (debug t))
  `(let ((default-directory (seq-find (lambda (dir)
                                        (and dir
                                             (file-exists-p dir)))
                                      (list default-directory
                                            (expand-file-name "~/")
                                            temporary-file-directory
                                            (getenv "TMPDIR")
                                            "/tmp/")
                                      "/")))
     ,@body))

(defmacro with-case-table (table &rest body)
  "Execute the forms in BODY with TABLE as the current case table.
The value returned is the value of the last form in BODY."
  (declare (indent 1) (debug t))
  (let ((old-case-table (make-symbol "table"))
	(old-buffer (make-symbol "buffer")))
    `(let ((,old-case-table (current-case-table))
	   (,old-buffer (current-buffer)))
       (unwind-protect
	   (progn (set-case-table ,table)
		  ,@body)
	 (with-current-buffer ,old-buffer
	   (set-case-table ,old-case-table))))))

(defmacro with-mutex (mutex &rest body)
  "Invoke BODY with MUTEX held, releasing MUTEX when done.
This is the simplest safe way to acquire and release a mutex."
  (declare (indent 1) (debug t))
  (let ((sym (make-symbol "mutex")))
    `(let ((,sym ,mutex))
       (mutex-lock ,sym)
       (unwind-protect
	   (progn ,@body)
	 (mutex-unlock ,sym)))))

(defmacro with-memoization (place &rest code)
  "Return the value of CODE and stash it in PLACE.
If PLACE's value is non-nil, then don't bother evaluating CODE
and return the value found in PLACE instead."
  (declare (indent 1) (debug (gv-place body)))
  ;; remacs: `or' + `setf' is equivalent to GNU's `gv-letplace'
  ;; expansion for the supported generalized variables.
  `(or ,place
       (setf ,place (progn ,@code))))

(defmacro with-delayed-message (args &rest body)
  "Like `progn', but display MESSAGE if BODY takes longer than TIMEOUT seconds.
The MESSAGE form will be evaluated immediately, but the resulting
string will be displayed only if BODY takes longer than TIMEOUT seconds.

\(fn (TIMEOUT MESSAGE) &rest BODY)"
  (declare (indent 1))
  `(funcall-with-delayed-message ,(car args) ,(cadr args)
                                 (lambda ()
                                   ,@body)))

(defmacro def-edebug-spec (symbol spec)
  "Set the `edebug-form-spec' property of SYMBOL according to SPEC."
  `(put ',symbol 'edebug-form-spec ',spec))

;;; Buffer/region functions (GNU subr.el).

(defun insert-buffer-substring-no-properties (buffer &optional start end)
  "Insert before point a substring of BUFFER, without text properties.
BUFFER may be a buffer or a buffer name.
Arguments START and END are character positions specifying the substring.
They default to the values of (point-min) and (point-max) in BUFFER."
  (let ((opoint (point)))
    (insert-buffer-substring buffer start end)
    (let ((inhibit-read-only t))
      (set-text-properties opoint (point) nil))))

(defun insert-into-buffer (buffer &optional start end)
  "Insert the contents of the current buffer into BUFFER.
If START/END, only insert that region from the current buffer.
Point in BUFFER will be placed after the inserted text."
  (let ((current (current-buffer)))
    (with-current-buffer buffer
      (insert-buffer-substring current start end))))

(defun remove-yank-excluded-properties (start end)
  "Process text properties between START and END, inserted for a `yank'.
Perform the handling specified by `yank-handled-properties', then
remove properties specified by `yank-excluded-properties'."
  (let ((inhibit-read-only t))
    (dolist (handler yank-handled-properties)
      (let ((prop (car handler))
            (fun  (cdr handler))
            (run-start start))
        (while (< run-start end)
          (let ((value (get-text-property run-start prop))
                (run-end (next-single-property-change
                          run-start prop nil end)))
            (funcall fun value run-start run-end)
            (setq run-start run-end)))))
    (if (eq yank-excluded-properties t)
        (set-text-properties start end nil)
      (remove-list-of-text-properties start end yank-excluded-properties))))

(defun insert-buffer-substring-as-yank (buffer &optional start end)
  "Insert before point a part of BUFFER, stripping some text properties.
BUFFER may be a buffer or a buffer name.
Arguments START and END are character positions specifying the substring.
They default to the values of (point-min) and (point-max) in BUFFER.
Before insertion, process text properties according to
`yank-handled-properties' and `yank-excluded-properties'."
  ;; Since the buffer text should not normally have yank-handler properties,
  ;; there is no need to handle them here.
  (let ((opoint (point)))
    (insert-buffer-substring buffer start end)
    (remove-yank-excluded-properties opoint (point))))

(defun replace-string-in-region (string replacement &optional start end)
  "Replace STRING with REPLACEMENT in the region from START to END.
The number of replaced occurrences are returned, or nil if STRING
doesn't exist in the region.
If START is nil, use the current point.  If END is nil, use `point-max'.
Comparisons and replacements are done with fixed case."
  (if start
      (when (< start (point-min))
        (error "Start before start of buffer"))
    (setq start (point)))
  (if end
      (when (> end (point-max))
        (error "End after end of buffer"))
    (setq end (point-max)))
  (save-excursion
    (goto-char start)
    (save-restriction
      (narrow-to-region start end)
      (let ((matches 0)
            (case-fold-search nil))
        (while (search-forward string nil t)
          (delete-region (match-beginning 0) (match-end 0))
          (insert replacement)
          (setq matches (1+ matches)))
        (and (not (zerop matches))
             matches)))))

(defun replace-regexp-in-region (regexp replacement &optional start end)
  "Replace REGEXP with REPLACEMENT in the region from START to END.
The number of replaced occurrences are returned, or nil if REGEXP
doesn't exist in the region.
If START is nil, use the current point.  If END is nil, use `point-max'.
Comparisons and replacements are done with fixed case.
REPLACEMENT can use the following special elements:
  `\\&' in NEWTEXT means substitute original matched text.
  `\\N' means substitute what matched the Nth `\\(...\\)'.
       If Nth parens didn't match, substitute nothing.
  `\\\\' means insert one `\\'.
  `\\?' is treated literally."
  (if start
      (when (< start (point-min))
        (error "Start before start of buffer"))
    (setq start (point)))
  (if end
      (when (> end (point-max))
        (error "End after end of buffer"))
    (setq end (point-max)))
  (save-excursion
    (goto-char start)
    (save-restriction
      (narrow-to-region start end)
      (let ((matches 0)
            (case-fold-search nil))
          (while (re-search-forward regexp nil t)
          (replace-match replacement t)
          (setq matches (1+ matches)))
        (and (not (zerop matches))
             matches)))))

(defun delete-line ()
  "Delete the current line."
  ;; remacs's `pos-bol' ignores its argument, so use
  ;; `line-beginning-position' for the end position.
  (delete-region (pos-bol) (line-beginning-position 2)))

;;; Motion/syntax functions (GNU subr.el).

(defun forward-whitespace (arg)
  "Move point to the end of the next sequence of whitespace chars.
Each such sequence may be a single newline, or a sequence of
consecutive space and/or tab characters.
With prefix argument ARG, do it ARG times if positive, or move
backwards ARG times if negative."
  (interactive "^p")
  (if (natnump arg)
      (re-search-forward "[ \t]+\\|\n" nil 'move arg)
    (while (< arg 0)
      (if (re-search-backward "[ \t]+\\|\n" nil 'move)
	  (or (eq (char-after (match-beginning 0)) ?\n)
	      (skip-chars-backward " \t")))
      (setq arg (1+ arg)))))

(defun forward-symbol (arg)
  "Move point to the next position that is the end of a symbol.
A symbol is any sequence of characters that are in either the
word constituent or symbol constituent syntax class.
With prefix argument ARG, do it ARG times if positive, or move
backwards ARG times if negative."
  (interactive "^p")
  (if (natnump arg)
      (re-search-forward "\\(\\sw\\|\\s_\\)+" nil 'move arg)
    (while (< arg 0)
      (if (re-search-backward "\\(\\sw\\|\\s_\\)+" nil 'move)
	  (skip-syntax-backward "w_"))
      (setq arg (1+ arg)))))

(defun forward-same-syntax (&optional arg)
  "Move point past all characters with the same syntax class.
With prefix argument ARG, do it ARG times if positive, or move
backwards ARG times if negative."
  (interactive "^p")
  (or arg (setq arg 1))
  (while (< arg 0)
    (skip-syntax-backward
     (char-to-string (char-syntax (char-before))))
    (setq arg (1+ arg)))
  (while (> arg 0)
    (skip-syntax-forward (char-to-string (char-syntax (char-after))))
    (setq arg (1- arg))))

(defun subregexp-context-p (regexp pos &optional start)
  "Return non-nil if POS is in a normal subregexp context in REGEXP.
A subregexp context is one where a sub-regexp can appear.
A non-subregexp context is for example within brackets, or within a
repetition bounds operator `\\=\\{...\\}', or right after a `\\'.
If START is non-nil, it should be a position in REGEXP, smaller
than POS, and known to be in a subregexp context."
  (declare (important-return-value t))
  (let ((prefix (substring regexp (or start 0) pos))
        (i 0)
        (bound nil))
    ;; remacs's regexp engine does not implement \{m,n\} bounds and
    ;; accepts a trailing unmatched "\{" silently, so detect an
    ;; unclosed bound ourselves before consulting the parser.
    (while (< i (length prefix))
      (let ((c (aref prefix i)))
        (if (and (eq c ?\\) (< (1+ i) (length prefix)))
            (let ((n (aref prefix (1+ i))))
              (cond ((eq n ?{) (setq bound t))
                    ((eq n ?}) (setq bound nil)))
              (setq i (+ i 2)))
          (setq i (1+ i)))))
    (and (not bound)
         (condition-case err
             (progn
               (string-match prefix "")
               t)
           (invalid-regexp
            ;; GNU signals "Unmatched [ or [^", "Unmatched \\{" and
            ;; "Trailing backslash"; remacs uses its own wording.
            (not (member (cadr err) '("Unmatched [ or [^"
                                      "Unmatched \\{"
                                      "Trailing backslash"
                                      "unterminated char class"
                                      "trailing backslash"))))))))

;;; Yank machinery (GNU subr.el).

(defvar yank-transform-functions nil
  "Functions that transform the string to be inserted for a `yank'.
Each function is called with a single argument, the string to be
inserted, and should return the transformed string.")

(defvar yank-undo-function nil
  "Function used to remove the text inserted by the last `yank' command.")

(defun insert-for-yank (string)
  "Insert STRING at point for the `yank' command.
This function is like `insert', except it honors the variables
`yank-handled-properties' and `yank-excluded-properties', and the
`yank-handler' text property, in the way that `yank' does.
It also runs the string through `yank-transform-functions'."
  ;; Allow altering the yank string.
  (run-hook-wrapped 'yank-transform-functions
                    (lambda (f) (setq string (funcall f string)) nil))
  (let (to)
    (while (setq to (next-single-property-change 0 'yank-handler string))
      (insert-for-yank-1 (substring string 0 to))
      (setq string (substring string to))))
  (insert-for-yank-1 string))

(defun insert-for-yank-1 (string)
  "Helper for `insert-for-yank', which see."
  (let* ((handler (and (stringp string)
		       (get-text-property 0 'yank-handler string)))
	 (param (or (nth 1 handler) string))
	 (opoint (point))
	 (inhibit-read-only inhibit-read-only)
	 end)

    ;; FIXME: This throws away any yank-undo-function set by previous calls
    ;; to insert-for-yank-1 within the loop of insert-for-yank!
    (setq yank-undo-function t)
    (if (nth 0 handler) ; FUNCTION
	(funcall (car handler) param)
      (insert param))
    (setq end (point))

    ;; Prevent read-only properties from interfering with the
    ;; following text property changes.
    (setq inhibit-read-only t)

    (unless (nth 2 handler) ; NOEXCLUDE
      (remove-yank-excluded-properties opoint end))

    ;; If last inserted char has properties, mark them as rear-nonsticky.
    (if (and (> end opoint)
	     (text-properties-at (1- end)))
	(put-text-property (1- end) end 'rear-nonsticky t))

    (if (eq yank-undo-function t)		   ; not set by FUNCTION
	(setq yank-undo-function (nth 3 handler))) ; UNDO
    (if (nth 4 handler)				   ; COMMAND
	(setq this-command (nth 4 handler)))))

;;; Delayed warnings (GNU subr.el).

(defun display-delayed-warnings ()
  "Display delayed warnings from `delayed-warnings-list'.
Used from `delayed-warnings-hook' (which see)."
  (dolist (warning (nreverse delayed-warnings-list))
    (apply #'display-warning warning))
  (setq delayed-warnings-list nil))

(defun collapse-delayed-warnings ()
  "Remove duplicates from `delayed-warnings-list'.
Collapse identical adjacent warnings into one (plus count).
Used from `delayed-warnings-hook' (which see)."
  (let ((count 1)
        collapsed warning)
    (while delayed-warnings-list
      (setq warning (pop delayed-warnings-list))
      (if (equal warning (car delayed-warnings-list))
          (setq count (1+ count))
        (when (> count 1)
          (setcdr warning (cons (format "%s [%d times]" (cadr warning) count)
                                (cddr warning)))
          (setq count 1))
        (push warning collapsed)))
    (setq delayed-warnings-list (nreverse collapsed))))

(defun delay-warning (type message &optional level buffer-name)
  "Display a delayed warning.
Aside from going through `delayed-warnings-list', this is equivalent
to `display-warning'."
  (push (list type message level buffer-name) delayed-warnings-list))

;;; Backtrace (GNU subr.el).

(defun backtrace-frames (&optional base)
  "Collect all frames of current backtrace into a list.
If non-nil, BASE should be a function, and frames before its
nearest activation frame are discarded."
  (let ((frames nil))
    (mapbacktrace (lambda (&rest frame) (push frame frames))
                  (or base #'backtrace-frames))
    (nreverse frames)))

(defun backtrace-frame (nframes &optional base)
  "Return the function and arguments NFRAMES up from current execution point.
If non-nil, BASE should be a function, and NFRAMES counts from its
nearest activation frame.  BASE can also be of the form (OFFSET . FUNCTION)
in which case OFFSET will be added to NFRAMES.
If the frame has not evaluated the arguments yet (or is a special form),
the value is (nil FUNCTION ARG-FORMS...).
If the frame has evaluated its arguments and called its function already,
the value is (t FUNCTION ARG-VALUES...).
A &rest arg is represented as the tail of the list ARG-VALUES.
FUNCTION is whatever was supplied as car of evaluated list,
or a lambda expression for macro calls.
If NFRAMES is more than the number of frames, the value is nil."
  (backtrace-frame--internal
   (lambda (evald func args _) `(,evald ,func ,@args))
   nframes (or base #'backtrace-frame)))

;;; Loading history (GNU subr.el).

;; remacs: `load-suffixes' and `jka-compr-load-suffixes' are defined in
;; C/jka-compr in GNU; provide equivalents here for `load-history-regexp'.
(defvar load-suffixes '(".so" ".dylib" ".elc" ".el")
  "List of suffixes for Emacs Lisp files and dynamic modules.")

(defvar jka-compr-load-suffixes '(".gz")
  "Additional suffixes for loading compressed files, set by jka-compr.")

(defun load-history-regexp (file)
  "Form a regexp to find FILE in `load-history'.
FILE, a string, is described in the function `eval-after-load'."
  (if (file-name-absolute-p file)
      (setq file (file-truename file)))
  (concat (if (file-name-absolute-p file) "\\`" "\\(\\`\\|/\\)")
	  (regexp-quote file)
	  (if (file-name-extension file)
	      ""
	    ;; Note: regexp-opt can't be used here, since we need to call
	    ;; this before Emacs has been fully started.  2006-05-21
	    (concat "\\(" (mapconcat #'regexp-quote load-suffixes "\\|") "\\)?"))
	  "\\(" (mapconcat #'regexp-quote jka-compr-load-suffixes "\\|")
	  "\\)?\\'"))

;;; Input helpers (GNU subr.el).

(defvar read-char-choice-use-read-key nil
  "Non-nil means `read-char-choice' uses `read-key' instead of the minibuffer.")

(defun read-char-choice (prompt chars &optional inhibit-keyboard-quit)
  "Read and return one of the characters in CHARS, prompting with PROMPT.
CHARS should be a list of single characters.
The function discards any input character that is not one of CHARS,
and by default shows a message to the effect that it is not one of
the expected characters.

By default, this function uses the minibuffer to read the key
non-modally (see `read-char-from-minibuffer'), and the optional
argument INHIBIT-KEYBOARD-QUIT is ignored.  However, if
`read-char-choice-use-read-key' is non-nil, the modal `read-key'
function is used instead (see `read-char-choice-with-read-key'),
and INHIBIT-KEYBOARD-QUIT is passed to it."
  (if (not read-char-choice-use-read-key)
      (read-char-from-minibuffer prompt chars)
    (read-char-choice-with-read-key prompt chars inhibit-keyboard-quit)))

(defun read-char-choice-with-read-key (prompt chars &optional inhibit-keyboard-quit)
  "Read and return one of the characters in CHARS, prompting with PROMPT.
CHARS should be a list of single characters.
Any input that is not one of CHARS is ignored.

If optional argument INHIBIT-KEYBOARD-QUIT is non-nil, ignore
`keyboard-quit' events while waiting for valid input.

If you bind the variable `help-form' to a non-nil value
while calling this function, then pressing `help-char'
causes it to evaluate `help-form' and display the result."
  (unless (consp chars)
    (error "Called `read-char-choice' without valid char choices"))
  (let (char done show-help (helpbuf " *Char Help*"))
    (let ((cursor-in-echo-area t)
          (executing-kbd-macro executing-kbd-macro)
	  (esc-flag nil))
      (save-window-excursion	      ; in case we call help-form-show
	(while (not done)
	  (unless (get-text-property 0 'face prompt)
	    (setq prompt (propertize prompt 'face 'minibuffer-prompt)))
          ;; Display the on screen keyboard if it exists.
          (frame-toggle-on-screen-keyboard (selected-frame) nil)
	  (setq char (let ((inhibit-quit inhibit-keyboard-quit))
		       (read-key prompt)))
	  (and show-help (buffer-live-p (get-buffer helpbuf))
	       (kill-buffer helpbuf))
	  (cond
	   ((not (numberp char)))
	   ;; If caller has set help-form, that's enough.
	   ;; They don't explicitly have to add help-char to chars.
	   ((and help-form
		 (eq char help-char)
		 (setq show-help t)
		 (help-form-show)))
	   ((memq char chars)
	    (setq done t))
	   ((not inhibit-keyboard-quit)
	    (cond
	     ((and (null esc-flag) (eq char ?\e))
	      (setq esc-flag t))
	     ((memq char '(?\C-g ?\e))
	      (keyboard-quit))))))))
    ;; Display the question with the answer.  But without cursor-in-echo-area.
    (message "%s%s" prompt (char-to-string char))
    char))

;;; y-or-n-p minibuffer commands (GNU subr.el).

(defun y-or-n-p-insert-y ()
  "Insert the answer \"y\" and exit the minibuffer of `y-or-n-p'.
Discard all previous input before inserting and exiting the minibuffer."
  (interactive)
  (when (minibufferp)
    (delete-minibuffer-contents)
    (insert "y")
    (exit-minibuffer)))

(defun y-or-n-p-insert-n ()
  "Insert the answer \"n\" and exit the minibuffer of `y-or-n-p'.
Discard all previous input before inserting and exiting the minibuffer."
  (interactive)
  (when (minibufferp)
    (delete-minibuffer-contents)
    (insert "n")
    (exit-minibuffer)))

(defun y-or-n-p-insert-other ()
  "Handle inserting of other answers in the minibuffer of `y-or-n-p'.
Display an error on trying to insert a disallowed character.
Also discard all previous input in the minibuffer."
  (interactive)
  (when (minibufferp)
    (delete-minibuffer-contents)
    (ding)
    (discard-input)
    (minibuffer-message "Please answer y or n")
    (sit-for 2)))

(defvar y-or-n-p-use-read-key nil
  "Use `read-key' when reading answers to \"y or n\" questions by `y-or-n-p'.
Otherwise, use the `read-from-minibuffer' to read the answers.")

(defvar from--tty-menu-p nil
  "Non-nil means the current command was invoked from a TTY menu.")

(defvar use-dialog-box-override nil
  "Whether `use-dialog-box-p' should always return t.")

;;; Documentation/fill helpers (GNU subr.el).

(defun internal--fill-string-single-line (str)
  "Fill string STR to `fill-column'.
This is intended for very simple filling while bootstrapping
Emacs itself, and does not support all the customization options
of fill.el (for example `fill-region')."
  (if (< (length str) fill-column)
      str
    (let* ((limit (min fill-column (length str)))
           (fst (substring str 0 limit))
           (lst (substring str limit)))
      (cond ((string-match "\\( \\)$" fst)
             (setq fst (replace-match "\n" nil nil fst 1)))
            ((string-match "^ \\(.*\\)" lst)
             (setq fst (concat fst "\n"))
             (setq lst (match-string 1 lst)))
            ((string-match ".*\\( \\(.+\\)\\)$" fst)
             (setq lst (concat (match-string 2 fst) lst))
             (setq fst (replace-match "\n" nil nil fst 1))))
      (concat fst (internal--fill-string-single-line lst)))))

(defun internal--format-docstring-line (string &rest objects)
  "Format a single line from a documentation string out of STRING and OBJECTS.
Signal an error if STRING contains a newline.
This is intended for internal use only.  Avoid using this for the
first line of a docstring; the first line should be a complete
sentence (see Info node `(elisp) Documentation Tips')."
  (when (string-match "\n" string)
    (error "Unable to fill string containing newline: %S" string))
  (internal--fill-string-single-line (apply #'format string objects)))

;;; Keymap/hook helpers (GNU subr.el).

(defun map-keymap-sorted (function keymap)
  "Implement `map-keymap' with sorting.
Don't call this function; it is for internal use only."
  (let (list)
    (map-keymap (lambda (a b) (push (cons a b) list))
                keymap)
    (setq list (sort list
                     (lambda (a b)
                       (setq a (car a) b (car b))
                       (if (integerp a)
                           (if (integerp b) (< a b)
                             t)
                         (if (integerp b) t
                           ;; string< also accepts symbols.
                           (string< a b))))))
    (dolist (p list)
      (funcall function (car p) (cdr p)))))

(defun run-hook-query-error-with-timeout (hook)
  "Run HOOK, catching errors, and querying the user about whether to continue.
If a function in HOOK signals an error, the user will be prompted
whether to continue or not.  If the user doesn't respond,
evaluation will continue if the user doesn't respond within five
seconds."
  (run-hook-wrapped
   hook
   (lambda (fun)
     (condition-case err
         (funcall fun)
       (error
        (unless (y-or-n-p-with-timeout (format "Error %s; continue?" err)
                                       5 t)
          (error err))))
     ;; Continue running.
     nil)))

;;; Misc functions (GNU subr.el).

(defun use-dialog-box-p ()
  "Return non-nil if the current command should prompt the user via a dialog box."
  ;; remacs: `use-dialog-box-override' is unbound; guard all the
  ;; event-state variables with `bound-and-true-p'.
  (or (bound-and-true-p use-dialog-box-override)
      (and (bound-and-true-p last-input-event)
           (or (and (boundp 'last-nonmenu-event)
                    (consp last-nonmenu-event))
               (and (boundp 'last-nonmenu-event)
                    (null last-nonmenu-event)
                    (consp last-input-event))
               (bound-and-true-p from--tty-menu-p))
           (bound-and-true-p use-dialog-box))))

(defun json-available-p ()
  "Return non-nil if Emacs has native JSON support."
  t)

(defun bidi-string-mark-left-to-right (str)
  "Return a string that can be safely inserted in left-to-right text.

If STR contains any RTL character, this function returns a string
consisting of STR followed by an invisible left-to-right mark
\(LRM) character.  Otherwise, it returns STR."
  (unless (stringp str)
    (signal 'wrong-type-argument (list 'stringp str)))
  ;; remacs: `\cR' regexp categories are unsupported, so detect RTL
  ;; characters by their codepoint ranges instead.
  (let ((i 0)
        (len (length str))
        rtl)
    (while (and (< i len) (not rtl))
      (let ((c (aref str i)))
        (setq rtl (or (and (>= c #x0590) (<= c #x08FF))
                      (and (>= c #xFB1D) (<= c #xFDFF))
                      (and (>= c #xFE70) (<= c #xFEFF))
                      (and (>= c #x10800) (<= c #x10FFF))))
        (setq i (1+ i))))
    (if rtl
        (concat str (propertize (string ?\u200e) 'invisible t))
      str)))

;;; Event predicates (GNU subr.el).

(defsubst mouse-movement-p (object)
  "Return non-nil if OBJECT is a mouse movement event."
  (declare (side-effect-free error-free))
  (eq (car-safe object) 'mouse-movement))

;;; Motion (GNU simple.el).

(defun backward-to-indentation (&optional arg)
  "Move backward ARG lines and position at first nonblank character."
  (interactive "^p")
  (forward-to-indentation (- (or arg 1))))

;;; Hash (GNU subr.el).

(defun sha1 (object &optional start end binary)
  "Return the SHA-1 (Secure Hash Algorithm) of an OBJECT.
OBJECT is either a string or a buffer.  Optional arguments START and
END are character positions specifying which portion of OBJECT for
computing the hash.  If BINARY is non-nil, return a 20-byte unibyte
string; otherwise return a 40-character string.

Note that SHA-1 is not collision resistant and should not be used
for anything security-related.  See `secure-hash' for
alternatives."
  (declare (side-effect-free t))
  (secure-hash 'sha1 object start end binary))

;;; Yank handlers (GNU subr.el).

(defun yank-handle-font-lock-face-property (face start end)
  "If `font-lock-defaults' is nil, apply FACE as a `face' property.
START and END denote the start and end of the text to act on.
Do nothing if FACE is nil."
  (and face
       (null font-lock-defaults)
       (put-text-property start end 'face face)))

;; This removes `mouse-face' properties in *Help* buffer buttons:
;; https://lists.gnu.org/r/emacs-devel/2002-04/msg00648.html
(defun yank-handle-category-property (category start end)
  "Apply property category CATEGORY's properties between START and END."
  (when category
    (let ((start2 start))
      (while (< start2 end)
	(let ((end2     (next-property-change start2 nil end))
	      (original (text-properties-at start2)))
	  (set-text-properties start2 end2 (symbol-plist category))
	  (add-text-properties start2 end2 original)
	  (setq start2 end2))))))

;;; Menu-item keymap helpers (GNU subr.el).

(defun keymap--menu-item-binding (val)
  "Return the binding part of a menu-item."
  (cond
   ((not (consp val)) val)              ;Not a menu-item.
   ((eq 'menu-item (car val))
    (let* ((binding (nth 2 val))
           (plist (nthcdr 3 val))
           (filter (plist-get plist :filter)))
      (if filter (funcall filter binding)
        binding)))
   ((and (consp (cdr val)) (stringp (cadr val)))
    (cddr val))
   ((stringp (car val))
    (cdr val))
   (t val)))                            ;Not a menu-item either.

(defun keymap--menu-item-with-binding (item binding)
  "Build a menu-item like ITEM but with its binding changed to BINDING."
  (cond
   ((not (consp item)) binding)		;Not a menu-item.
   ((eq 'menu-item (car item))
    (setq item (copy-sequence item))
    (let ((tail (nthcdr 2 item)))
      (setcar tail binding)
      ;; Remove any potential filter.
      (if (plist-get (cdr tail) :filter)
          (setcdr tail (plist-put (cdr tail) :filter nil))))
    item)
   ((and (consp (cdr item)) (stringp (cadr item)))
    (cons (car item) (cons (cadr item) binding)))
   (t (cons (car item) binding))))

(defun keymap--merge-bindings (val1 val2)
  "Merge bindings VAL1 and VAL2."
  (let ((map1 (keymap--menu-item-binding val1))
        (map2 (keymap--menu-item-binding val2)))
    (if (not (and (keymapp map1) (keymapp map2)))
        ;; There's nothing to merge: val1 takes precedence.
        val1
      (let ((map (list 'keymap map1 map2))
            (item (if (keymapp val1) (if (keymapp val2) nil val2) val1)))
        (keymap--menu-item-with-binding item map)))))

;;; with-selected-window internals (GNU subr.el).

(defun internal--before-with-selected-window (window)
  (let ((other-frame (window-frame window)))
    (list window (selected-window)
          ;; Selecting a window on another frame also changes that
          ;; frame's frame-selected-window.  We must save&restore it.
          (unless (eq (selected-frame) other-frame)
            (frame-selected-window other-frame))
          ;; Also remember the top-frame if on ttys.
          (unless (eq (selected-frame) other-frame)
            (tty-top-frame other-frame)))))

(defun internal--after-with-selected-window (state)
  ;; First reset frame-selected-window.
  (when (window-live-p (nth 2 state))
    ;; We don't use set-frame-selected-window because it does not
    ;; pass the `norecord' argument to Fselect_window.
    (select-window (nth 2 state) 'norecord)
    (and (frame-live-p (nth 3 state))
         (not (eq (tty-top-frame) (nth 3 state)))
         (select-frame (nth 3 state) 'norecord)))
  ;; Then reset the actual selected-window.
  (when (window-live-p (nth 1 state))
    (select-window (nth 1 state) 'norecord)))

;;; Keymap substitution internals (GNU subr.el).

(defvar key-substitution-in-progress nil
  "Used internally by `substitute-key-definition'.")

(defun substitute-key-definition-key (defn olddef newdef prefix keymap)
  (let (inner-def skipped menu-item)
    ;; Find the actual command name within the binding.
    (if (eq (car-safe defn) 'menu-item)
	(setq menu-item defn defn (nth 2 defn))
      ;; Skip past menu-prompt.
      (while (stringp (car-safe defn))
	(push (pop defn) skipped))
      ;; Skip past cached key-equivalence data for menu items.
      (if (consp (car-safe defn))
	  (setq defn (cdr defn))))
    (if (or (eq defn olddef)
	    ;; Compare with equal if definition is a key sequence.
	    ;; That is useful for operating on function-key-map.
	    (and (or (stringp defn) (vectorp defn))
		 (equal defn olddef)))
	(define-key keymap prefix
	  (if menu-item
	      (let ((copy (copy-sequence menu-item)))
		(setcar (nthcdr 2 copy) newdef)
		copy)
	    (nconc (nreverse skipped) newdef)))
      ;; Look past a symbol that names a keymap.
      (setq inner-def
	    (or (indirect-function defn) defn))
      ;; For nested keymaps, we use `inner-def' rather than `defn' so as to
      ;; avoid autoloading a keymap.  This is mostly done to preserve the
      ;; original non-autoloading behavior of pre-map-keymap times.
      (if (and (keymapp inner-def)
	       ;; Avoid recursively scanning
	       ;; where KEYMAP does not have a submap.
	       (let ((elt (lookup-key keymap prefix)))
		 (or (null elt) (natnump elt) (keymapp elt)))
	       ;; Avoid recursively rescanning keymap being scanned.
	       (not (memq inner-def key-substitution-in-progress)))
	  ;; If this one isn't being scanned already, scan it now.
	  (substitute-key-definition olddef newdef keymap inner-def prefix)))))

;;; Load history (GNU subr.el).

(defun load-history-filename-element (file-regexp)
  "Get the first elt of `load-history' whose car matches FILE-REGEXP.
Return nil if there isn't one."
  (let* ((loads load-history)
	 (load-elt (and loads (car loads))))
    (save-match-data
      (while (and loads
		  (not (and (car load-elt)
                            (string-match file-regexp (car load-elt)))))
	(setq loads (cdr loads)
	      load-elt (and loads (car loads)))))
    load-elt))

(defun do-after-load-evaluation (abs-file)
  "Evaluate all `eval-after-load' forms, if any, for ABS-FILE.
ABS-FILE, a string, should be the absolute true name of a file just loaded.
This function is called directly from the C code."
  ;; Run the relevant eval-after-load forms.
  (dolist (a-l-element after-load-alist)
    (when (and (stringp (car a-l-element))
               (string-match-p (car a-l-element) abs-file))
      ;; discard the file name regexp
      (mapc #'funcall (cdr a-l-element))))
  ;; Complain when the user uses obsolete files.
  (when (string-match-p "/obsolete/[^/]*\\'" abs-file)
    ;; Maybe we should just use display-warning?  This seems yucky...
    (let* ((file (file-name-nondirectory abs-file))
           (package (intern (substring file 0
			               (string-match "\\.elc?\\>" file))
                            obarray))
	   (msg (format "Package %s is deprecated" package))
	   (fun (lambda (msg) (message "%s" msg))))
      (when (or (not (fboundp 'byte-compile-warning-enabled-p))
                (byte-compile-warning-enabled-p 'obsolete package))
        (cond
	 ((bound-and-true-p byte-compile-current-file)
	  ;; Don't warn about obsolete files using other obsolete files.
	  (unless (and (stringp byte-compile-current-file)
		       (string-match-p "/obsolete/[^/]*\\'"
				       (expand-file-name
					byte-compile-current-file
					byte-compile-root-dir)))
	    (byte-compile-warn "%s" msg)))
         (noninteractive (funcall fun msg)) ;; No timer will be run!
	 (t (run-with-idle-timer 0 nil fun msg))))))

  ;; Finally, run any other hook.
  (run-hook-with-args 'after-load-functions abs-file))

;;; Function properties (GNU subr.el).

(defun function-get (f prop &optional autoload)
  "Return the value of property PROP of function F.
If AUTOLOAD is non-nil and F is autoloaded, try to load it
in the hope that it will set PROP.  If AUTOLOAD is `macro', do it only
if it's an autoloaded macro."
  (declare (important-return-value t))
  (let ((val nil))
    (while (and (symbolp f)
                (null (setq val (get f prop)))
                (fboundp f))
      (let ((fundef (symbol-function f)))
        (if (and autoload (autoloadp fundef)
                 (not (equal fundef
                             (autoload-do-load fundef f
                                               (if (eq autoload 'macro)
                                                   'macro)))))
            nil                         ;Re-try `get' on the same `f'.
          (setq f fundef))))
    val))

;;; Shell command helpers (GNU subr.el/files.el).

(defmacro with-connection-local-variables (&rest body)
  "Apply connection-local variables according to `default-directory'.
remacs: connection-local variables are not implemented, so this
simply executes BODY."
  (declare (indent 0) (debug t))
  `(progn ,@body))

(defun process-file-shell-command (command &optional infile buffer display
					   &rest args)
  "Process files synchronously in a separate process.
Similar to `call-process-shell-command', but calls `process-file'."
  (declare (advertised-calling-convention
            (command &optional infile buffer display) "24.5"))
  ;; On remote hosts, the local `shell-file-name' might be useless.
  (with-connection-local-variables
   (process-file
    shell-file-name infile buffer display shell-command-switch
    (mapconcat #'identity (cons command args) " "))))

(defun start-file-process-shell-command (name buffer command)
  "Start a program in a subprocess.  Return the process object for it.
Similar to `start-process-shell-command', but calls `start-file-process'."
  (with-connection-local-variables
   (start-file-process name buffer shell-file-name
                       shell-command-switch command)))

(defun call-shell-region (start end command &optional delete buffer)
  "Send text from START to END as input to an inferior shell running COMMAND.
Delete the text if fourth arg DELETE is non-nil.

Insert output in BUFFER before point; t means current buffer; nil for
 BUFFER means discard it; 0 means discard and don't wait; and `(:file
 FILE)', where FILE is a file name string, means that it should be
 written to that file (if the file already exists it is overwritten).
BUFFER can also have the form (REAL-BUFFER STDERR-FILE); in that case,
REAL-BUFFER says what to do with standard output, as above,
while STDERR-FILE says what to do with standard error in the child.
STDERR-FILE may be nil (discard standard error output),
t (mix it with ordinary output), or a file name string.

If BUFFER is 0, `call-shell-region' returns immediately with value nil.
Otherwise it waits for COMMAND to terminate
and returns a numeric exit status or a signal description string.
If you quit, the process is killed with SIGINT, or SIGKILL if you quit again."
  (call-process-region start end
                       shell-file-name delete buffer nil
                       shell-command-switch command))

;;; Process line helpers (GNU subr.el).

(defun process-lines-handling-status (program status-handler &rest args)
  "Execute PROGRAM with ARGS, returning its output as a list of lines.
If STATUS-HANDLER is non-nil, it must be a function with one
argument, which will be called with the exit status of the
program before the output is collected.  If STATUS-HANDLER is
nil, an error is signaled if the program returns with a non-zero
exit status."
  (declare (important-return-value t))
  (with-temp-buffer
    (let ((status (apply #'call-process program nil (current-buffer) nil args)))
      (if status-handler
	  (funcall status-handler status)
	(unless (eq status 0)
	  (error "%s exited with status %s" program status)))
      (goto-char (point-min))
      (let (lines)
	(while (not (eobp))
	  (setq lines (cons (buffer-substring-no-properties
			     (line-beginning-position)
			     (line-end-position))
			    lines))
	  (forward-line 1))
	(nreverse lines)))))

(defun process-kill-buffer-query-function ()
  "Ask before killing a buffer that has a running process."
  (let ((process (get-buffer-process (current-buffer))))
    (or (not process)
        (not (memq (process-status process) '(run stop open listen)))
        (not (process-query-on-exit-flag process))
        (yes-or-no-p
	 (format "Buffer %S has a running process; kill it? "
		 (buffer-name (current-buffer)))))))

;;; Progress reporters (GNU subr.el).

(defvar progress-reporter--pulse-characters ["-" "\\" "|" "/"]
  "Characters to use for pulsing progress reporters.")

(defsubst progress-reporter-update (reporter &optional value suffix)
  "Report progress of an operation in the echo area.
REPORTER should be the result of a call to `make-progress-reporter'.

If REPORTER is a numerical progress reporter---i.e. if it was
 made using non-nil MIN-VALUE and MAX-VALUE arguments to
 `make-progress-reporter'---then VALUE should be a number between
 MIN-VALUE and MAX-VALUE.

Optional argument SUFFIX is a string to be displayed after
REPORTER's main message and progress text.  If REPORTER is a
non-numerical reporter, then VALUE should be nil, or a string to
use instead of SUFFIX.

This function is relatively inexpensive.  If the change since
last update is too small or insufficient time has passed, it does
nothing."
  (when (or (not (numberp value))      ; For pulsing reporter
	    (>= value (car reporter))) ; For numerical reporter
    (progress-reporter-do-update reporter value suffix)))

(defun make-progress-reporter (message &optional min-value max-value
				       current-value min-change min-time)
  "Return progress reporter object for use with `progress-reporter-update'.

MESSAGE is shown in the echo area, with a status indicator
appended to the end.  When you call `progress-reporter-done', the
word \"done\" is printed after the MESSAGE.  You can change the
MESSAGE of an existing progress reporter by calling
`progress-reporter-force-update'.

MIN-VALUE and MAX-VALUE, if non-nil, are starting (0% complete)
and final (100% complete) states of operation; the latter should
be larger.  In this case, the status message shows the percentage
progress.

If MIN-VALUE and/or MAX-VALUE is omitted or nil, the status
message shows a \"spinning\", non-numeric indicator.

Optional CURRENT-VALUE is the initial progress; the default is
MIN-VALUE.
Optional MIN-CHANGE is the minimal change in percents to report;
the default is 1%.
CURRENT-VALUE and MIN-CHANGE do not have any effect if MIN-VALUE
and/or MAX-VALUE are nil.

Optional MIN-TIME specifies the minimum interval time between
echo area updates (default is 0.2 seconds.)  If the OS is not
capable of measuring fractions of seconds, this parameter is
effectively rounded up."
  (when (string-match "[[:alnum:]]\\'" message)
    (setq message (concat message "...")))
  (unless min-time
    (setq min-time 0.2))
  (let ((reporter
	 (cons (or min-value 0)
	       (vector (if (>= min-time 0.02)
			   (float-time) nil)
		       min-value
		       max-value
		       message
		       (if min-change (max (min min-change 50) 1) 1)
                       min-time
                       ;; SUFFIX
                       nil))))
    ;; Force a call to `message' now.
    (progress-reporter-update reporter (or current-value min-value))
    reporter))

(defalias 'progress-reporter-make #'make-progress-reporter)

(defun progress-reporter-force-update (reporter &optional value new-message suffix)
  "Report progress of an operation in the echo area unconditionally.

REPORTER, VALUE, and SUFFIX are the same as in `progress-reporter-update'.
NEW-MESSAGE, if non-nil, sets a new message for the reporter."
  (let ((parameters (cdr reporter)))
    (when new-message
      (aset parameters 3 new-message))
    (when (aref parameters 0)
      (aset parameters 0 (float-time)))
    (progress-reporter-do-update reporter value suffix)))

(defun progress-reporter-do-update (reporter value &optional suffix)
  (let* ((parameters   (cdr reporter))
	 (update-time  (aref parameters 0))
	 (min-value    (aref parameters 1))
	 (max-value    (aref parameters 2))
	 (text         (aref parameters 3))
	 (enough-time-passed
	  ;; See if enough time has passed since the last update.
	  (or (not update-time)
	      (when (time-less-p update-time nil)
		;; Calculate time for the next update
		(aset parameters 0 (+ update-time (aref parameters 5)))))))
    (cond ((and min-value max-value)
	   ;; Numerical indicator
	   (let* ((one-percent (/ (- max-value min-value) 100.0))
		  (percentage  (if (= max-value min-value)
				   0
				 (truncate (/ (- value min-value)
					      one-percent)))))
	     ;; Calculate NEXT-UPDATE-VALUE.  If we are not printing
	     ;; message because not enough time has passed, use 1
	     ;; instead of MIN-CHANGE.  This makes delays between echo
	     ;; area updates closer to MIN-TIME.
	     (setcar reporter
		     (min (+ min-value (* (+ percentage
					     (if enough-time-passed
						 ;; MIN-CHANGE
						 (aref parameters 4)
					       1))
					  one-percent))
			  max-value))
	     (when (integerp value)
	       (setcar reporter (ceiling (car reporter))))
	     ;; Print message only if enough time has passed
	     (when enough-time-passed
               (if suffix
                   (aset parameters 6 suffix)
                 (setq suffix (or (aref parameters 6) "")))
               (if (> percentage 0)
                   (message "%s%d%% %s" text percentage suffix)
                 (message "%s %s" text suffix)))))
	  ;; Pulsing indicator
	  (enough-time-passed
           (when (and value (not suffix))
             (setq suffix value))
           (if suffix
               (aset parameters 6 suffix)
             (setq suffix (or (aref parameters 6) "")))
           (let* ((index (mod (1+ (car reporter)) 4))
                  (message-log-max nil)
                  (pulse-char (aref progress-reporter--pulse-characters
                                    index)))
	     (setcar reporter index)
             (message "%s %s %s" text pulse-char suffix))))))

(defun progress-reporter-done (reporter)
  "Print reporter's message followed by word \"done\" in echo area."
  (message "%sdone" (aref (cdr reporter) 3)))

(defmacro dotimes-with-progress-reporter (spec reporter-or-message &rest body)
  "Loop a certain number of times and report progress in the echo area.
Evaluate BODY with VAR bound to successive integers running from
0, inclusive, to COUNT, exclusive.  Then evaluate RESULT to get
the return value (nil if RESULT is omitted).

REPORTER-OR-MESSAGE is a progress reporter object or a string.  In the latter
case, use this string to create a progress reporter.

At each iteration, print the reporter message followed by progress
percentage in the echo area.  After the loop is finished,
print the reporter message followed by the word \"done\".

This macro is a convenience wrapper around `make-progress-reporter' and friends.

\(fn (VAR COUNT [RESULT]) REPORTER-OR-MESSAGE BODY...)"
  (declare (indent 2) (debug ((symbolp form &optional form) form body)))
  (let ((prep (make-symbol "--dotimes-prep--"))
        (end (make-symbol "--dotimes-end--")))
    `(let ((,prep ,reporter-or-message)
           (,end ,(cadr spec)))
       (when (stringp ,prep)
         (setq ,prep (make-progress-reporter ,prep 0 ,end)))
       (dotimes (,(car spec) ,end)
         ,@body
         (progress-reporter-update ,prep (1+ ,(car spec))))
       (progress-reporter-done ,prep)
       (or ,@(cdr (cdr spec)) nil))))

(defmacro dolist-with-progress-reporter (spec reporter-or-message &rest body)
  "Loop over a list and report progress in the echo area.
Evaluate BODY with VAR bound to each car from LIST, in turn.
Then evaluate RESULT to get return value, default nil.

REPORTER-OR-MESSAGE is a progress reporter object or a string.  In the latter
case, use this string to create a progress reporter.

At each iteration, print the reporter message followed by progress
percentage in the echo area.  After the loop is finished,
print the reporter message followed by the word \"done\".

\(fn (VAR LIST [RESULT]) REPORTER-OR-MESSAGE BODY...)"
  (declare (indent 2) (debug ((symbolp form &optional form) form body)))
  (let ((prep (make-symbol "--dolist-progress-reporter--"))
        (count (make-symbol "--dolist-count--"))
        (list (make-symbol "--dolist-list--")))
    `(let ((,prep ,reporter-or-message)
           (,count 0)
           (,list ,(cadr spec)))
       (when (stringp ,prep)
         (setq ,prep (make-progress-reporter ,prep 0 (length ,list))))
       (dolist (,(car spec) ,list)
         ,@body
         (progress-reporter-update ,prep (setq ,count (1+ ,count))))
       (progress-reporter-done ,prep)
       (or ,@(cdr (cdr spec)) nil))))

;;; if-let*/when-let* internals (GNU subr.el).

(defun internal--build-bindings (bindings)
  "Check and build conditional value forms for BINDINGS."
  ;; remacs's `internal--build-binding' returns a different shape, so
  ;; the GNU binding-normalization logic is inlined here.
  (let ((prev-var t))
    (mapcar (lambda (binding)
              (let* ((norm (cond
                            ((symbolp binding)
                             (list binding binding))
                            ((null (cdr binding))
                             (list (make-symbol "s") (car binding)))
                            ((eq '_ (car binding))
                             (list (make-symbol "s") (cadr binding)))
                            (t binding)))
                     (built (progn
                              (when (> (length norm) 2)
                                (signal 'error
                                        (cons "`let' bindings can have only one value-form"
                                              binding)))
                              (list (car norm)
                                    (list 'and prev-var (cadr norm))))))
                (setq prev-var (car built))
                built))
            bindings)))

;;; Event reading internals (GNU subr.el).

(defvar xterm-mouse-mode nil
  "Non-nil if Xterm Mouse mode is enabled.")

(defvar touch-screen-events-received nil
  "Non-nil if touch screen events have been received.")

(defun read--potential-mouse-event ()
  "Read an event that might be a mouse event.

This function exists for backward compatibility in code packaged
with Emacs.  Do not call it directly in your own packages."
  ;; `xterm-mouse-mode' events must go through `read-key' as they
  ;; are decoded via `input-decode-map'.
  (if (or xterm-mouse-mode
          ;; If a touch screen is being employed, then mouse events
          ;; are subject to translation as well.
          touch-screen-events-received)
      (read-key nil
                ;; Normally `read-key' discards all mouse button
                ;; down events.  However, we want them here.
                t)
    (read-event)))

(defvar read-number-history nil
  "The default history for the `read-number' function.")

(defun goto-char--read-natnum-interactive (prompt)
  "Get a natural number argument, optionally prompting with PROMPT.
If there is a natural number at point, use it as default."
  (if (and current-prefix-arg (not (consp current-prefix-arg)))
      (list (prefix-numeric-value current-prefix-arg))
    (let* ((number (number-at-point))
           (default (and (natnump number) number)))
      (list (read-number prompt (list default (point)))))))

;;; Temp output buffer internals (GNU subr.el).

(defvar temp-buffer-show-function nil
  "Non-nil means call as function to display a help buffer.")

(defvar temp-buffer-show-hook nil
  "Normal hook run by `with-output-to-temp-buffer' and friends.")

(defvar minibuffer-scroll-window nil
  "Window to scroll when the minibuffer's contents are shown.")

(defun internal-temp-output-buffer-show (buffer)
  "Internal function for `with-output-to-temp-buffer'."
  (with-current-buffer buffer
    (set-buffer-modified-p nil)
    (goto-char (point-min)))

  (if temp-buffer-show-function
      (funcall temp-buffer-show-function buffer)
    (with-current-buffer buffer
      (let* ((window
	      (let ((window-combination-limit
		   ;; When `window-combination-limit' equals
		   ;; `temp-buffer' or `temp-buffer-resize' and
		   ;; `temp-buffer-resize-mode' is enabled in this
		   ;; buffer bind it to t so resizing steals space
		   ;; preferably from the window that was split.
		   (if (or (eq window-combination-limit 'temp-buffer)
			   (and (eq window-combination-limit
				    'temp-buffer-resize)
				temp-buffer-resize-mode))
		       t
		     window-combination-limit)))
		(display-buffer buffer)))
	     (frame (and window (window-frame window))))
	(when window
	  (unless (eq frame (selected-frame))
	    (make-frame-visible frame))
	  (setq minibuffer-scroll-window window)
	  (set-window-hscroll window 0)
	  ;; Don't try this with NOFORCE non-nil!
	  (set-window-start window (point-min) t)
	  ;; This should not be necessary.
	  (set-window-point window (point-min))
	  ;; Run `temp-buffer-show-hook', with the chosen window selected.
	  (with-selected-window window
	    (run-hooks 'temp-buffer-show-hook))))))
  ;; Return nil.
  nil)

;;; Text clones (GNU subr.el).

(defvar text-clone--maintaining nil)

(defun text-clone--maintain (ol1 after beg end &optional _len)
  "Propagate the changes made under the overlay OL1 to the other clones.
This is used on the `modification-hooks' property of text clones."
  (when (and after (not undo-in-progress)
             (not text-clone--maintaining)
             (overlay-start ol1))
    (let ((margin (if (overlay-get ol1 'text-clone-spreadp) 1 0)))
      (setq beg (max beg (+ (overlay-start ol1) margin)))
      (setq end (min end (- (overlay-end ol1) margin)))
      (when (<= beg end)
	(save-excursion
	  (when (overlay-get ol1 'text-clone-syntax)
	    ;; Check content of the clone's text.
	    (let ((cbeg (+ (overlay-start ol1) margin))
		  (cend (- (overlay-end ol1) margin)))
	      (goto-char cbeg)
	      (save-match-data
		(if (not (re-search-forward
			  (overlay-get ol1 'text-clone-syntax) cend t))
		    ;; Mark the overlay for deletion.
		    (setq end cbeg)
		  (when (< (match-end 0) cend)
		    ;; Shrink the clone at its end.
		    (setq end (min end (match-end 0)))
		    (move-overlay ol1 (overlay-start ol1)
				  (+ (match-end 0) margin)))
		  (when (> (match-beginning 0) cbeg)
		    ;; Shrink the clone at its beginning.
		    (setq beg (max (match-beginning 0) beg))
		    (move-overlay ol1 (- (match-beginning 0) margin)
				  (overlay-end ol1)))))))
	  ;; Now go ahead and update the clones.
	  (let ((head (- beg (overlay-start ol1)))
		(tail (- (overlay-end ol1) end))
		(str (buffer-substring beg end))
		(nothing-left t)
		(text-clone--maintaining t))
	    (dolist (ol2 (overlay-get ol1 'text-clones))
	      (let ((oe (overlay-end ol2)))
		(unless (or (eq ol1 ol2) (null oe))
		  (setq nothing-left nil)
		  (let ((mod-beg (+ (overlay-start ol2) head)))
		    ;;(overlay-put ol2 'modification-hooks nil)
		    (goto-char (- (overlay-end ol2) tail))
		    (unless (> mod-beg (point))
		      (save-excursion (insert str))
		      (delete-region mod-beg (point)))
		    ;;(overlay-put ol2 'modification-hooks '(text-clone--maintain))
		    ))))
	    (if nothing-left (delete-overlay ol1))))))))

(defun text-clone-create (start end &optional spreadp syntax)
  "Create a text clone of START...END at point.
Text clones are chunks of text that are automatically kept identical:
changes done to one of the clones will be immediately propagated to the other.

The buffer's content at point is assumed to be already identical to
the one between START and END.
If SYNTAX is provided it's a regexp that describes the possible text of
the clones; the clone will be shrunk or killed if necessary to ensure that
its text matches the regexp.
If SPREADP is non-nil it indicates that text inserted before/after the
clone should be incorporated in the clone."
  ;; To deal with SPREADP we can either use an overlay with `nil t' along
  ;; with insert-(behind|in-front-of)-hooks or use a slightly larger overlay
  ;; (with a one-char margin at each end) with `t nil'.
  ;; We opted for a larger overlay because it behaves better in the case
  ;; where the clone is reduced to the empty string (we want the overlay to
  ;; stay when the clone's content is the empty string and we want to use
  ;; `evaporate' to make sure those overlays get deleted when needed).
  ;;
  (let* ((pt-end (+ (point) (- end start)))
  	 (start-margin (if (or (not spreadp) (bobp) (<= start (point-min)))
			   0 1))
  	 (end-margin (if (or (not spreadp)
			     (>= pt-end (point-max))
  			     (>= start (point-max)))
  			 0 1))
         ;; FIXME: Reuse overlays at point to extend dups!
  	 (ol1 (make-overlay (- start start-margin) (+ end end-margin) nil t))
  	 (ol2 (make-overlay (- (point) start-margin) (+ pt-end end-margin) nil t))
	 (dups (list ol1 ol2)))
    (overlay-put ol1 'modification-hooks '(text-clone--maintain))
    (when spreadp (overlay-put ol1 'text-clone-spreadp t))
    (when syntax (overlay-put ol1 'text-clone-syntax syntax))
    ;;(overlay-put ol1 'face 'underline)
    (overlay-put ol1 'evaporate t)
    (overlay-put ol1 'text-clones dups)
    ;;
    (overlay-put ol2 'modification-hooks '(text-clone--maintain))
    (when spreadp (overlay-put ol2 'text-clone-spreadp t))
    (when syntax (overlay-put ol2 'text-clone-syntax syntax))
    ;;(overlay-put ol2 'face 'underline)
    (overlay-put ol2 'evaporate t)
    (overlay-put ol2 'text-clones dups)))

;;; Mail user agents (GNU subr.el).

(defvar mail-user-agent 'message-user-agent
  "Your preference for a mail-sending package.
The valid values include:
  message-user-agent -- use the Message package.
  sendmail-user-agent -- use the Mail package.")

(defvar mail-send-hook nil
  "Hook run before a mail message is actually sent.")

(defun define-mail-user-agent (symbol composefunc sendfunc
				      &optional abortfunc hookvar)
  "Define a symbol to identify a mail-sending package for `mail-user-agent'.

SYMBOL can be any Lisp symbol.  Its function definition and/or
value as a variable do not matter for this usage; we use only certain
properties on its property list, to encode the rest of the arguments.

The properties used on SYMBOL are `composefunc', `sendfunc',
`abortfunc', and `hookvar'."
  (declare (indent defun))
  (put symbol 'composefunc composefunc)
  (put symbol 'sendfunc sendfunc)
  (put symbol 'abortfunc (or abortfunc #'kill-buffer))
  (put symbol 'hookvar (or hookvar 'mail-send-hook)))

;;; Keymap stack internals (GNU subr.el).

(defun internal-push-keymap (keymap symbol)
  (let ((map (symbol-value symbol)))
    (unless (memq keymap map)
      (unless (memq 'add-keymap-witness (symbol-value symbol))
        (setq map (make-composed-keymap nil (symbol-value symbol)))
        (push 'add-keymap-witness (cdr map))
        (set symbol map))
      (push keymap (cdr map)))))

(defun internal-pop-keymap (keymap symbol)
  (let ((map (symbol-value symbol)))
    (when (memq keymap map)
      (setf (cdr map) (delq keymap (cdr map))))
    (let ((tail (cddr map)))
      (and (or (null tail) (keymapp tail))
           (eq 'add-keymap-witness (nth 1 map))
           (set symbol tail)))))

(define-obsolete-function-alias
  'set-temporary-overlay-map #'set-transient-map "24.4")

;;; Autoload prefix registry (GNU subr.el).

(defvar definition-prefixes (make-hash-table :test 'equal)
  "Hash table mapping prefixes to the files defining them.")

(defun register-definition-prefixes (file prefixes)
  "Register that FILE uses PREFIXES."
  (dolist (prefix prefixes)
    (puthash prefix (cons file (gethash prefix definition-prefixes))
             definition-prefixes)))

;;; Misc (GNU subr.el).

(defconst menu-bar-separator '("--")
  "Separator for menus.")

(defun unmsys--file-name (file)
  "Produce the canonical file name for FILE from its MSYS form.

On systems other than MS-Windows, just returns FILE.
On MS-Windows, converts /d/foo/bar form of file names
passed by MSYS Make into d:/foo/bar that Emacs can grok.

This function is called from lisp/Makefile and leim/Makefile."
  (when (and (eq system-type 'windows-nt)
	     (string-match "\\`/[a-zA-Z]/" file))
    (setq file (concat (substring file 1 2) ":" (substring file 2))))
  file)

(defvar undo-in-progress nil
  "Non-nil while `undo' is in progress.")

;;;; From simple.el.

(defcustom suggest-key-bindings t
  "Non-nil means show the equivalent keybinding when running a command."
  :group 'keyboard
  :type '(choice (const :tag "off" nil)
                 (natnum :tag "time" 2)
                 (other :tag "on" t)))

(defvar local-minor-modes nil
  "List of minor modes enabled in the current buffer.")

(defvar global-minor-modes nil
  "List of global minor modes enabled.")

(defvar clone-buffer-hook nil
  "Normal hook run in the new buffer at the end of `clone-buffer'.")

(defvar filter-buffer-substring-functions nil
  "This variable is a wrapper hook around `filter-buffer-substring'.
Its members should be functions that remove themselves from the hook
when done.")

(defun capitalize-dwim (arg)
  "Capitalize words in the region, if active; if not, capitalize word at point.
If the region is active, this function calls `capitalize-region'.
Otherwise, it calls `capitalize-word', with prefix argument passed to it
to capitalize ARG words."
  (interactive "*p")
  (if (use-region-p)
      (capitalize-region (region-beginning) (region-end) (region-noncontiguous-p))
    (capitalize-word arg)))

(defun clone-process (process &optional newname)
  "Create a twin copy of PROCESS.
If NEWNAME is nil, it defaults to PROCESS' name;
NEWNAME is modified by adding or incrementing <N> at the end as necessary.
If PROCESS is associated with a buffer, the new process will be associated
  with the current buffer instead.
Returns nil if PROCESS has already terminated."
  (setq newname (or newname (process-name process)))
  (if (string-match "<[0-9]+>\\'" newname)
      (setq newname (substring newname 0 (match-beginning 0))))
  (when (memq (process-status process) '(run stop open))
    (let* ((process-connection-type (process-tty-name process))
           (new-process
            (if (memq (process-status process) '(open))
                (let ((args (process-contact process t)))
                  (setq args (plist-put args :name newname))
                  (setq args (plist-put args :buffer
                                        (if (process-buffer process)
                                            (current-buffer))))
                  (apply 'make-network-process args))
              (apply 'start-process newname
                     (if (process-buffer process) (current-buffer))
                     (process-command process)))))
      (set-process-query-on-exit-flag
       new-process (process-query-on-exit-flag process))
      (set-process-inherit-coding-system-flag
       new-process (process-inherit-coding-system-flag process))
      (set-process-filter new-process (process-filter process))
      (set-process-sentinel new-process (process-sentinel process))
      (set-process-plist new-process (copy-sequence (process-plist process)))
      new-process)))

(defun buffer-substring--filter (beg end &optional delete)
  "Default function to use for `filter-buffer-substring-function'.
Its arguments and return value are as specified for `filter-buffer-substring'.
Also respects the obsolete wrapper hook `filter-buffer-substring-functions'
(see `with-wrapper-hook' for details about wrapper hooks).
No filtering is done unless a hook says to."
  (subr--with-wrapper-hook-no-warnings
    filter-buffer-substring-functions (beg end delete)
    (cond
     (delete
      (save-excursion
        (goto-char beg)
        (delete-and-extract-region beg end)))
     (t
      (buffer-substring beg end)))))

(defun command-completion-default-include-p (symbol buffer)
  "Say whether SYMBOL should be offered as a completion.
If there's a `completion-predicate' for SYMBOL, the result from
calling that predicate is called.  If there isn't one, this
predicate is true if the command SYMBOL is applicable to the
major mode in BUFFER, or any of the active minor modes in
BUFFER."
  (if (get symbol 'completion-predicate)
      ;; An explicit completion predicate takes precedence.
      (funcall (get symbol 'completion-predicate) symbol buffer)
    (or (null (command-modes symbol))
        (command-completion-using-modes-p symbol buffer))))

(defun command-completion-with-modes-p (modes buffer)
  "Say whether MODES are in action in BUFFER.
This is the case if either the major mode is derived from one of MODES,
or (if one of MODES is a minor mode), if it is switched on in BUFFER."
  (or (provided-mode-derived-p (buffer-local-value 'major-mode buffer) modes)
      ;; It's a minor mode.
      (seq-intersection modes
                        (buffer-local-value 'local-minor-modes buffer)
                        #'eq)
      (seq-intersection modes global-minor-modes #'eq)))

(defun command-completion-using-modes-p (symbol buffer)
  "Say whether SYMBOL has been marked as a mode-specific command in BUFFER."
  ;; Check the modes.
  (when-let ((modes (command-modes symbol)))
    ;; Common fast case: Just a single mode.
    (if (null (cdr modes))
        (or (provided-mode-derived-p
             (buffer-local-value 'major-mode buffer) (car modes))
            (memq (car modes)
                  (buffer-local-value 'local-minor-modes buffer))
            (memq (car modes) global-minor-modes))
      ;; Uncommon case: Multiple modes.
      (command-completion-with-modes-p modes buffer))))

(defun command-completion-using-modes-and-keymaps-p (symbol buffer)
  "Return non-nil if SYMBOL is marked for BUFFER's mode or bound in its keymaps."
  (with-current-buffer buffer
    (let ((keymaps
           ;; The major mode's keymap and any active minor modes.
           (nconc
            (and (current-local-map) (list (current-local-map)))
            (mapcar
             #'cdr
             (seq-filter
              (lambda (elem)
                (symbol-value (car elem)))
              minor-mode-map-alist)))))
      (or (command-completion-using-modes-p symbol buffer)
          ;; Include commands that are bound in a keymap in the
          ;; current buffer.
          (and (where-is-internal symbol keymaps)
               ;; But not if they have a command predicate that
               ;; says that they shouldn't.  (This is the case
               ;; for `ignore' and `undefined' and similar
               ;; commands commonly found in keymaps.)
               (or (null (get symbol 'completion-predicate))
                   (funcall (get symbol 'completion-predicate)
                            symbol buffer)))
          ;; Include customize-* commands (do we need a list of such
          ;; "always available" commands? customizable?)
          (string-match-p "customize-" (symbol-name symbol))))))

(defun command-completion-button-p (category buffer)
  "Return non-nil if there's a button of CATEGORY at point in BUFFER."
  (with-current-buffer buffer
    (and (get-text-property (point) 'button)
         (eq (get-text-property (point) 'category) category))))

(defun command-completion--command-for-this-buffer-function ()
  (let ((keymaps
         ;; The major mode's keymap and any active minor modes.
         (nconc
          (and (current-local-map) (list (current-local-map)))
          (mapcar
           #'cdr
           (seq-filter
            (lambda (elem)
              (symbol-value (car elem)))
            minor-mode-map-alist)))))
    (lambda (symbol buffer)
      (or (command-completion-using-modes-p symbol buffer)
          ;; Include commands that are bound in a keymap in the
          ;; current buffer.
          (and (where-is-internal symbol keymaps)
               ;; But not if they have a command predicate that
               ;; says that they shouldn't.  (This is the case
               ;; for `ignore' and `undefined' and similar
               ;; commands commonly found in keymaps.)
               (or (null (get symbol 'completion-predicate))
                   (funcall (get symbol 'completion-predicate)
                            symbol buffer)))))))

(defun read-extended-command--affixation (command-names)
  (with-selected-window (or (minibuffer-selected-window) (selected-window))
    (mapcar
     (lambda (command-name)
       (let* ((fun (and (stringp command-name) (intern-soft command-name)))
              (binding (where-is-internal fun overriding-local-map t))
              (obsolete (get fun 'byte-obsolete-info))
              (alias (symbol-function fun))
              (suffix (cond ((symbolp alias)
                             (format " (%s)" alias))
                            (obsolete
                             (format " (%s)" (car obsolete)))
                            ((and binding (not (stringp binding)))
                             (format " (%s)" (key-description binding)))
                            (t ""))))
         (put-text-property 0 (length suffix)
                            'face 'completions-annotations suffix)
         (list command-name "" suffix)))
     command-names)))

(defun function-documentation (function)
  "Extract the raw docstring info from FUNCTION.
FUNCTION is expected to be a function value rather than, say, a mere symbol."
  (let ((docstring-p (lambda (doc)
                       ;; A docstring can be either a string or a reference
                       ;; into either the `etc/DOC' or a `.elc' file.
                       (or (stringp doc)
                           (fixnump doc) (fixnump (cdr-safe doc))))))
    (pcase function
      ((pred closurep)
       (when (> (length function) 4)
         (let ((doc (aref function 4)))
           (when (funcall docstring-p doc) doc))))
      ((or (pred stringp) (pred vectorp)) "Keyboard macro.")
      (`(keymap . ,_)
       "Prefix command (definition is a keymap associating keystrokes with commands).")
      ((or `(lambda ,_args . ,body) `(autoload ,_file . ,body))
       (let ((doc (car body)))
         (when (funcall docstring-p doc)
           doc))))))

;;;; Mark commands (GNU simple.el).

(define-error 'mark-inactive "The mark is not active now")

(defcustom set-mark-command-repeat-pop nil
  "Non-nil means repeating \\[set-mark-command] after popping mark pops it again.
That means that \\[universal-argument] \\[set-mark-command] \\[set-mark-command]
will pop the mark twice, and
\\[universal-argument] \\[set-mark-command] \\[set-mark-command] \\[set-mark-command]
will pop the mark three times.

A value of nil means \\[set-mark-command]'s behavior does not change
after \\[universal-argument] \\[set-mark-command]."
  :type 'boolean
  :group 'editing-basics)

(defun pop-to-mark-command ()
  "Jump to mark, and pop a new position for mark off the ring.
\(Does not affect global mark ring)."
  (interactive)
  (if (null (mark t))
      (user-error "No mark set in this buffer")
    (if (= (point) (mark t))
        (message "Mark popped"))
    (goto-char (mark t))
    (pop-mark)))

(defun push-mark-command (arg &optional nomsg)
  "Set mark at where point is.
If no prefix ARG and mark is already set there, just activate it.
Display `Mark set' unless the optional second arg NOMSG is non-nil."
  (interactive "P")
  (let ((mark (mark t)))
    (if (or arg (null mark) (/= mark (point)))
        (push-mark nil nomsg t)
      (activate-mark 'no-tmm)
      (unless nomsg
        (message "Mark activated")))))

;;;; Quoted char reading (GNU simple.el).

(defcustom read-quoted-char-radix 8
  "Radix for \\[quoted-insert] and other uses of `read-quoted-char'.
Supported radix values are 8, 10 and 16."
  :type '(choice (const 8) (const 10) (const 16))
  :group 'editing-basics)

(defvar help-event-list '(help f1 ?\?)
  "List of input events that invoke the help system.")

(defun read-quoted-char (&optional prompt)
  "Like `read-char', but do not allow quitting.
Also, if the first character read is an octal digit,
we read any number of octal digits and return the
specified character code.  Any nondigit terminates the sequence.
If the terminator is RET, it is discarded;
any other terminator is used itself as input.

The optional argument PROMPT specifies a string to use to prompt the user.
The variable `read-quoted-char-radix' controls which radix to use
for numeric input."
  (let ((message-log-max nil)
        (help-events (delq nil (mapcar (lambda (c) (unless (characterp c) c))
                                       help-event-list)))
        done (first t) (code 0) char translated)
    (while (not done)
      (let ((inhibit-quit first)
            ;; Don't let C-h or other help chars get the help
            ;; message--only help function keys.  See bug#16617.
            (help-char nil)
            (help-event-list help-events)
            (help-form
             "Type the special character you want to use,
or the octal character code.
RET terminates the character code and is discarded;
any other non-digit terminates the character code and is then used as input."))
        (setq char (read-event (and prompt (format "%s-" prompt)) t))
        (if inhibit-quit (setq quit-flag nil)))
      ;; Translate TAB key into control-I ASCII character, and so on.
      (let ((translation (lookup-key local-function-key-map (vector char))))
        (setq translated (if (arrayp translation)
                             (aref translation 0)
                           char)))
      (if (integerp translated)
          (setq translated (char-resolve-modifiers translated)))
      (cond ((null translated))
            ((not (integerp translated))
             (setq unread-command-events (list char)
                   done t))
            ((/= (logand translated ?\M-\^@) 0)
             ;; Turn a meta-character into a character with the 0200 bit set.
             (setq code (logior (logand translated (lognot ?\M-\^@)) 128)
                   done t))
            ((and (<= ?0 translated)
                  (< translated (+ ?0 (min 10 read-quoted-char-radix))))
             (setq code (+ (* code read-quoted-char-radix) (- translated ?0)))
             (and prompt (setq prompt (message "%s %c" prompt translated))))
            ((and (<= ?a (downcase translated))
                  (< (downcase translated)
                     (+ ?a -10 (min 36 read-quoted-char-radix))))
             (setq code (+ (* code read-quoted-char-radix)
                           (+ 10 (- (downcase translated) ?a))))
             (and prompt (setq prompt (message "%s %c" prompt translated))))
            ((and (not first) (eq translated ?\C-m))
             (setq done t))
            ((not first)
             (setq unread-command-events (list char)
                   done t))
            (t (setq code translated
                     done t)))
      (setq first nil))
    code))

;;;; Buffer copy commands (GNU simple.el).

(defun prepend-to-buffer (buffer start end)
  "Prepend to specified BUFFER the text of the region.
The text is inserted into that buffer after its point.
BUFFER can be a buffer or the name of a buffer; this
function will create BUFFER if it doesn't already exist.

When calling from a program, give three arguments:
BUFFER (or buffer name), START and END.
START and END specify the portion of the current buffer to be copied."
  (interactive "BPrepend to buffer: \nr")
  (let ((oldbuf (current-buffer)))
    (with-current-buffer (get-buffer-create buffer)
      (barf-if-buffer-read-only)
      (save-excursion
        (insert-buffer-substring oldbuf start end)))))

(defun copy-to-buffer (buffer start end)
  "Copy to specified BUFFER the text of the region.
The text is inserted into that buffer, replacing existing text there.
BUFFER can be a buffer or the name of a buffer; this
function will create BUFFER if it doesn't already exist.

When calling from a program, give three arguments:
BUFFER (or buffer name), START and END.
START and END specify the portion of the current buffer to be copied."
  (interactive "BCopy to buffer: \nr")
  (let ((oldbuf (current-buffer)))
    (with-current-buffer (get-buffer-create buffer)
      (barf-if-buffer-read-only)
      (erase-buffer)
      (save-excursion
        (insert-buffer-substring oldbuf start end)))))

;;;; Completion (GNU bindings.el/simple.el).

(defun complete-symbol (arg)
  "Perform completion on the text around point.
The completion method is determined by `completion-at-point-functions'.

With a prefix argument, this command does completion within
the collection of symbols listed in the index of the manual for the
language you are using."
  (interactive "P")
  (if arg (info-complete-symbol) (completion-at-point)))

;;;; Undo (GNU subr.el).

(defun undo--wrap-and-run-primitive-undo (beg end list)
  "Call `primitive-undo' on the undo elements in LIST.

This function is intended to be called purely by `undo' as the
function in an \(apply DELTA BEG END FUNNAME . ARGS) undo
element.  It invokes `before-change-functions' and
`after-change-functions' once each for the entire region \(BEG
END) rather than once for each individual change.

Additionally the fresh \"redo\" elements which are generated on
`buffer-undo-list' will themselves be \"enclosed\" in
`undo--wrap-and-run-primitive-undo'.

Undo elements of this form are generated by the macro
`combine-change-calls'."
  (combine-change-calls beg end
                        (while list
                          (setq list (primitive-undo 1 list)))))

;;;; History element navigation (GNU simple.el).

(defun next-line-or-history-element (&optional arg)
  "Move cursor vertically down ARG lines, or to the next history element.
When point moves over the bottom line of multi-line minibuffer, puts ARGth
next element of the minibuffer history in the minibuffer."
  (interactive "^p")
  (or arg (setq arg 1))
  (let* ((old-point (point))
         ;; Don't add newlines if they have the mode enabled globally.
         (next-line-add-newlines nil)
         ;; Remember the original goal column of possibly multi-line input
         ;; excluding the length of the prompt on the first line.
         (prompt-end (minibuffer-prompt-end))
         (old-column (unless (and (eolp) (> (point) prompt-end))
                       (if (= (line-number-at-pos) 1)
                           (max (- (current-column)
                                   (save-excursion
                                     (goto-char (1- prompt-end))
                                     (current-column)))
                                0)
                         (current-column)))))
    (condition-case nil
        (with-no-warnings
          (next-line arg))
      (end-of-buffer
       ;; Restore old position since `line-move-visual' moves point to
       ;; the end of the line when it fails to go to the next line.
       (goto-char old-point)
       (next-history-element arg)
       ;; Reset `temporary-goal-column' because a correct value is not
       ;; calculated when `next-line' above fails by bumping against
       ;; the bottom of the minibuffer (bug#22544).
       (setq temporary-goal-column 0)
       ;; Restore the original goal column on the last line
       ;; of possibly multi-line input.
       (goto-char (point-max))
       (when old-column
         (if (= (line-number-at-pos) 1)
             (move-to-column (+ old-column
                                (save-excursion
                                  (goto-char (1- (minibuffer-prompt-end)))
                                  (current-column))))
           (move-to-column old-column)))))))

(defun previous-line-or-history-element (&optional arg)
  "Move cursor vertically up ARG lines, or to the previous history element.
When point moves over the top line of multi-line minibuffer, puts ARGth
previous element of the minibuffer history in the minibuffer."
  (interactive "^p")
  (or arg (setq arg 1))
  (let* ((old-point (point))
         ;; Remember the original goal column of possibly multi-line input
         ;; excluding the length of the prompt on the first line.
         (prompt-end (minibuffer-prompt-end))
         (old-column (unless (and (eolp) (> (point) prompt-end))
                       (if (= (line-number-at-pos) 1)
                           (max (- (current-column)
                                   (save-excursion
                                     (goto-char (1- prompt-end))
                                     (current-column)))
                                1)
                         (current-column)))))
    (condition-case nil
        (with-no-warnings
          (previous-line arg)
          ;; Avoid moving point to the prompt
          (when (< (point) (minibuffer-prompt-end))
            ;; If there is minibuffer contents on the same line
            (if (<= (minibuffer-prompt-end)
                    (save-excursion
                      (if (or truncate-lines (not line-move-visual))
                          (end-of-line)
                        (end-of-visual-line))
                      (point)))
                ;; Move to the beginning of minibuffer contents
                (goto-char (minibuffer-prompt-end))
              ;; Otherwise, go to the previous history element
              (signal 'beginning-of-buffer nil))))
      (beginning-of-buffer
       ;; Restore old position since `line-move-visual' moves point to
       ;; the beginning of the line when it fails to go to the previous line.
       (goto-char old-point)
       (previous-history-element arg)
       ;; Reset `temporary-goal-column' because a correct value is not
       ;; calculated when `previous-line' above fails by bumping against
       ;; the top of the minibuffer (bug#22544).
       (setq temporary-goal-column 0)
       ;; Restore the original goal column on the first line
       ;; of possibly multi-line input.
       (goto-char (minibuffer-prompt-end))
       (if old-column
           (if (= (line-number-at-pos) 1)
               (move-to-column (+ old-column
                                  (save-excursion
                                    (goto-char (1- (minibuffer-prompt-end)))
                                    (current-column))))
             (move-to-column old-column))
         (if (not line-move-visual) ; Handle logical lines (bug#42862)
             (end-of-line)
           ;; Put the cursor at the end of the visual line instead of the
           ;; logical line, so the next `previous-line-or-history-element'
           ;; would move to the previous history element, not to a possible upper
           ;; visual line from the end of logical line in `line-move-visual' mode.
           (end-of-visual-line)
           ;; Since `end-of-visual-line' puts the cursor at the beginning
           ;; of the next visual line, move it one char back to the end
           ;; of the first visual line (bug#22544).
           (unless (eolp) (backward-char 1))))))))

;;;; Region content accessors (GNU simple.el).

(setq region-extract-function
      (lambda (method)
        ;; This call either signals an error (if there is no region)
        ;; or returns a number.
        (let ((beg (region-beginning)))
          (cond
           ((eq method 'bounds)
            (list (cons beg (region-end))))
           ((eq method 'delete-only)
            (delete-region beg (region-end)))
           (t
            (filter-buffer-substring beg (region-end) method))))))

(defvar region-insert-function
  (lambda (lines)
    (let ((first t))
      (while lines
        (or first
            (insert ?\n))
        (insert-for-yank (car lines))
        (setq lines (cdr lines)
              first nil))))
  "Function to insert the region's content.
Called with one argument LINES.
Insert the region as a list of lines.")

;;;; M-x history and predicate (GNU simple.el).

(defvar extended-command-history nil)
(defvar execute-extended-command--last-typed nil)

(defcustom read-extended-command-predicate nil
  "Predicate to use to determine which commands to include when completing.
If it's nil, include all the commands.
If it's a function, it will be called with two parameters: the
symbol of the command and the current buffer.  The predicate should
return non-nil if the command should be considered as a completion
candidate for \\`M-x' in that buffer."
  :type '(choice (const :tag "All commands" nil)
                 (function-item
                  :tag "No commands for other modes" command-completion-default-include-p)
                 (function-item
                  :tag "Only commands for this mode" command-completion-using-modes-p)
                 (function-item
                  :tag "Commands for this mode or bound in its keymaps"
                  command-completion-using-modes-and-keymaps-p)
                 (function :tag "Other"))
  :group 'execute-extended-command
  :version "28.1")

;;;; Line number display limits (GNU C variables).

(defvar line-number-display-limit nil
  "Maximum buffer size for which line number should be displayed.
If the buffer is bigger than this, the line number does not appear
in the mode line.  A value of nil means no limit.")

(defvar line-number-display-limit-width 200
  "Maximum line width for which line number should be displayed.
If a line is wider than this, the line number does not appear
in the mode line.")

;;;; Word counting (GNU simple.el).

(defun count-words (start end &optional totals)
  "Count words between START and END.
If called interactively, START and END are normally the start and
end of the buffer; but if the region is active, START and END are
the start and end of the region.  Print a message reporting the
number of lines, sentences, words, and chars.  With prefix
argument, also include the data for the entire (un-narrowed)
buffer.

If called from Lisp, return the number of words between START and
END, without printing any message.  TOTALS is ignored when called
from Lisp."
  (interactive (list nil nil current-prefix-arg))
  ;; When called from Lisp, return the data.
  (if (not (called-interactively-p 'any))
      (let ((words 0)
            ;; Count across field boundaries. (Bug#41761)
            (inhibit-field-text-motion t))
        (save-excursion
          (save-restriction
            (narrow-to-region start end)
            (goto-char (point-min))
            (while (forward-word-strictly 1)
              (setq words (1+ words)))))
        words)
    ;; When called interactively, message the data.
    (let ((totals (if (and totals
                           (or (use-region-p)
                               (buffer-narrowed-p)))
                      (save-restriction
                        (widen)
                        (count-words--format "; buffer in total"
                                             (point-min) (point-max)))
                    "")))
      (if (use-region-p)
          (message "%s%s" (count-words--format
                           "Region" (region-beginning) (region-end))
                   totals)
        (message "%s%s" (count-words--buffer-format) totals)))))

(defun count-words-region (start end &optional arg)
  "Count the number of words in the region.
If called interactively, print a message reporting the number of
lines, words, and characters in the region (whether or not the
region is active); with prefix ARG, report for the entire buffer
rather than the region.

If called from Lisp, return the number of words between positions
START and END."
  (interactive (if current-prefix-arg
                   (list nil nil current-prefix-arg)
                 (list (region-beginning) (region-end) nil)))
  (cond ((not (called-interactively-p 'any))
         (count-words start end))
        (arg
         (message "%s" (count-words--buffer-format)))
        (t
         (message "%s" (count-words--format "Region" start end)))))

(defun count-words--buffer-format ()
  (count-words--format
   (if (buffer-narrowed-p) "Narrowed part of buffer" "Buffer")
   (point-min) (point-max)))

(defun count-words--format (str start end)
  (let ((lines (count-lines start end))
        (sentences (count-sentences start end))
        (words (count-words start end))
        (chars (- end start)))
    (format "%s has %d line%s, %d sentence%s, %d word%s, and %d character%s"
            str
            lines (if (= lines 1) "" "s")
            sentences (if (= sentences 1) "" "s")
            words (if (= words 1) "" "s")
            chars (if (= chars 1) "" "s"))))

;;;; Time formatting (GNU time-date.el).

(defvar seconds-to-string
  (list (list 1 "ms" 0.001)
        (list 100 "s" 1)
        (list (* 60 100) "m" 60.0)
        (list (* 3600 30) "h" 3600.0)
        (list (* 3600 24 400) "d" (* 3600.0 24.0))
        (list nil "y" (* 365.25 24 3600)))
  "Formatting used by the function `seconds-to-string'.")

(defun seconds-to-string (delay)
  ;; FIXME: There's a similar (tho fancier) function in mastodon.el!
  "Convert the time interval in seconds to a short string."
  (cond ((> 0 delay) (concat "-" (seconds-to-string (- delay))))
        ((= 0 delay) "0s")
        (t (let ((sts seconds-to-string) here)
             (while (and (car (setq here (pop sts)))
                         (<= (car here) delay)))
             (concat (format "%.2f" (/ delay (car (cddr here)))) (cadr here))))))

;;;; Buffer list (GNU window.el).

(defun get-next-valid-buffer (list &optional buffer visible-ok frame)
  "Search LIST for a valid buffer to display in FRAME.
Return nil when all buffers in LIST are undesirable for display,
otherwise return the first suitable buffer in LIST.

Buffers not visible in windows are preferred to visible buffers,
unless VISIBLE-OK is non-nil.
If the optional argument FRAME is nil, it defaults to the selected frame.
If BUFFER is non-nil, ignore occurrences of that buffer in LIST."
  ;; This logic is more or less copied from other-buffer.
  (setq frame (or frame (selected-frame)))
  (let ((pred (frame-parameter frame 'buffer-predicate))
        found buf)
    (while (and (not found) list)
      (setq buf (car list))
      (if (and (not (eq buffer buf))
               (buffer-live-p buf)
               (or (null pred) (funcall pred buf))
               (not (eq (aref (buffer-name buf) 0) ?\s))
               (or visible-ok (null (get-buffer-window buf 'visible))))
          (setq found buf)
        (setq list (cdr list))))
    (car list)))

(provide 'subr-x)

;;; subr-x.el ends here
