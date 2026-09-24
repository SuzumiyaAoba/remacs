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


(defconst env--substitute-vars-regexp
  "\\$\\(?:\\(?1:[[:alnum:]_]+\\)\\|{\\(?1:[^{}]+\\)}\\|\\$\\)")

(defun substitute-env-vars (string &optional when-undefined)
  "Substitute environment variables referred to in STRING.
`$FOO' where FOO is an environment variable name means to substitute
the value of that variable.  The variable name should be terminated
with a character not a letter, digit or underscore; otherwise, enclose
the entire variable name in braces.  For instance, in `ab$cd-x',
`$cd' is treated as an environment variable.

If WHEN-UNDEFINED is omitted or nil, references to undefined environment
variables are replaced by the empty string; if it is a function, the
function is called with the variable's name as argument, and should return
the text with which to replace it, or nil to leave it unchanged.
If it is non-nil and not a function, references to undefined variables are
left unchanged.

Use `$$' to insert a single dollar sign."
  (declare (important-return-value t))
  (let ((start 0))
    (while (string-match env--substitute-vars-regexp string start)
      (cond ((match-beginning 1)
	     (let* ((var (match-string 1 string))
                    (value (getenv var)))
               (if (and (null value)
                        (if (functionp when-undefined)
                            (null (setq value (funcall when-undefined var)))
                          when-undefined))
                   (setq start (match-end 0))
                 (setq string (replace-match (or value "") t t string)
                       start (+ (match-beginning 0) (length value))))))
	    (t
	     (setq string (replace-match "$" t t string)
		   start (+ (match-beginning 0) 1)))))
    string))

(defun substitute-env-in-file-name (filename)
  (declare (important-return-value t))
  (substitute-env-vars filename
                       ;; How 'bout we lookup other tables than the env?
                       ;; E.g. we could accept bookmark names as well!
                       (if (memq system-type '(windows-nt ms-dos))
                           (lambda (var) (getenv (upcase var)))
                         t)))

(defun pwd (&optional insert)
  "Show the current default directory.
With prefix argument INSERT, insert the current default directory
at point instead."
  (interactive "P")
  (if insert
      (insert default-directory)
    (message "Directory %s" default-directory)))

(defvar cd-path nil
  "Value of the CDPATH environment variable, as a list.
Not actually set up until the first time you use it.")

(defun parse-colon-path (search-path)
  "Explode a search path into a list of directory names.
Directories are separated by `path-separator' (which is colon in
GNU and Unix systems).  Substitute environment variables into the
resulting list of directory names.  For an empty path element (i.e.,
a leading or trailing separator, or two adjacent separators), return
nil (meaning `default-directory') as the associated list element."
  (declare (ftype (function (string) list)))
  (when (stringp search-path)
    (let ((spath (substitute-env-vars search-path))
          (double-slash-special-p
           (memq system-type '(windows-nt cygwin ms-dos))))
      (mapcar (lambda (f)
                (if (equal "" f) nil
                  (let ((dir (file-name-as-directory f)))
                    ;; Previous implementation used `substitute-in-file-name'
                    ;; which collapses multiple "/" in front, while
                    ;; preserving double slash where it matters.  Do
                    ;; the same for backward compatibility.
                    (if (string-match "\\`//+" dir)
                        (substring dir (- (match-end 0)
                                          (if double-slash-special-p 2 1)))
                      dir))))
              (split-string spath path-separator)))))

(defun cd-absolute (dir)
  "Change current directory to given absolute file name DIR."
  ;; Put the name into directory syntax now,
  ;; because otherwise expand-file-name may give some bad results.
  (setq dir (file-name-as-directory dir))
  ;; We used to additionally call abbreviate-file-name here, for an
  ;; unknown reason.  Problem is that most buffers are setup
  ;; without going through cd-absolute and don't call
  ;; abbreviate-file-name on their default-directory, so the few that
  ;; do end up using a superficially different directory.
  (setq dir (expand-file-name dir))
  (if (not (file-directory-p dir))
      (error (if (file-exists-p dir)
	         "%s is not a directory"
               "%s: no such directory")
             dir)
    (unless (file-accessible-directory-p dir)
      (error "Cannot cd to %s:  Permission denied" dir))
    (setq default-directory dir)
    (setq list-buffers-directory dir)))

(defun cd (dir)
  "Make DIR become the current buffer's default directory.
If your environment includes a `CDPATH' variable, try each one of
that list of directories (separated by occurrences of
`path-separator') when resolving a relative directory name.
The path separator is colon in GNU and GNU-like systems."
  (interactive
   (list
    ;; FIXME: There's a subtle bug in the completion below.  Seems linked
    ;; to a fundamental difficulty of implementing `predicate' correctly.
    ;; The manifestation is that TAB may list non-directories in the case where
    ;; those files also correspond to valid directories (if your cd-path is (A/
    ;; B/) and you have A/a a file and B/a a directory, then both `a' and `a/'
    ;; will be listed as valid completions).
    ;; This is because `a' (listed because of A/a) is indeed a valid choice
    ;; (which will lead to the use of B/a).
    (minibuffer-with-setup-hook
        (lambda ()
          (setq-local minibuffer-completion-table
		      (apply-partially #'locate-file-completion-table
				       cd-path nil))
          (setq-local minibuffer-completion-predicate
		      (lambda (dir)
			(locate-file dir cd-path nil
				     (lambda (f) (and (file-directory-p f) 'dir-ok))))))
      (unless cd-path
        (setq cd-path (or (parse-colon-path (getenv "CDPATH"))
                          (list "./"))))
      (read-directory-name "Change default directory: "
                           default-directory default-directory
                           t))))
  (unless cd-path
    (setq cd-path (or (parse-colon-path (getenv "CDPATH"))
                      (list "./"))))
  (cd-absolute
   (or
    ;; locate-file doesn't support remote file names, so detect them
    ;; and support them here by hand.
    (and (file-remote-p (expand-file-name dir))
         (file-accessible-directory-p (expand-file-name dir))
         (expand-file-name dir))
    (locate-file dir cd-path nil
                 (lambda (f) (and (file-directory-p f) 'dir-ok)))
    (if (getenv "CDPATH")
        (error "No such directory found via CDPATH environment variable: %s" dir)
      (error "No such directory: %s" dir)))))



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

;; GNU initializes this variable in C (insdel.c); remacs leaves it nil
;; there, so set its standard value here where the handlers live.
(unless yank-handled-properties
  (setq yank-handled-properties
        '((font-lock-face . yank-handle-font-lock-face-property)
          (category . yank-handle-category-property))))

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

;;;; simple.el members.

(define-obsolete-function-alias 'set-temporary-overlay-map
  #'set-transient-map "24.4")

(defun insert-into-buffer (buffer &optional start end)
  "Insert the contents of the current buffer into BUFFER.
If START/END, only insert that region from the current buffer.
Point in BUFFER will be placed after the inserted text."
  (let ((current (current-buffer)))
    (with-current-buffer buffer
      (insert-buffer-substring current start end))))

(defun insert-buffer-substring-no-properties (buffer &optional start end)
  "Insert before point a substring of BUFFER, without text properties.
BUFFER may be a buffer or a buffer name.
Arguments START and END are character positions specifying the substring.
They default to the values of (point-min) and (point-max) in BUFFER."
  (let ((opoint (point)))
    (insert-buffer-substring buffer start end)
    (let ((inhibit-read-only t))
      (set-text-properties opoint (point) nil))))

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
          (replace-region-contents (match-beginning 0) (match-end 0)
                                   replacement 0)
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

(defun kill-backward-chars (arg)
  (if (listp arg) (setq arg (car arg)))
  (if (eq arg '-) (setq arg -1))
  (kill-region (point) (- (point) arg)))

(defun end-of-visible-line ()
  "Move to end of current visible line."
  (end-of-line)
  ;; If the following character is currently invisible,
  ;; skip all characters with that same `invisible' property value,
  ;; then find the next newline.
  (while (and (not (eobp))
	      (save-excursion
		(skip-chars-forward "^\n")
		(invisible-p (point))))
    (skip-chars-forward "^\n")
    (if (get-text-property (point) 'invisible)
	(goto-char (or (next-single-property-change (point) 'invisible)
		       (point-max)))
      (goto-char (next-overlay-change (point))))
    (end-of-line)))

(defun insert-buffer (buffer)
  "Insert after point the contents of BUFFER.
Puts mark after the inserted text.
BUFFER may be a buffer or a buffer name."
  (declare (interactive-only insert-buffer-substring))
  (interactive
   (list
    (progn
      (barf-if-buffer-read-only)
      (read-buffer "Insert buffer: "
		   (if (eq (selected-window) (next-window))
		       (other-buffer (current-buffer))
		     (window-buffer (next-window)))
		   t))))
  (push-mark
   (save-excursion
     (insert-buffer-substring (get-buffer buffer)))))

(defun forward-unix-word (n &optional delim)
  "Move forward N Unix-words.
A Unix-word is whitespace-delimited.
A negative N means go backwards to the beginning of Unix-words.

Unix-words differ from Emacs words in that they are always delimited by
whitespace, regardless of the buffer's syntax table.  This function
emulates how C-w at the Unix terminal or shell identifies words.

Optional argument DELIM specifies what characters are considered
whitespace.  It is a string as might be passed to `skip-chars-forward'.
The default is \"\\s\\f\\n\\r\\t\\v\".  Do not prefix a `^' character."
  (when (string-prefix-p "^" delim)
    (error "DELIM argument must not begin with `^'"))
  (unless (zerop n)
    ;; We do skip over newlines by default because `backward-word' does.
    (let* ((delim (or delim "\s\f\n\r\t\v"))
           (ndelim (format "^%s" delim))
           (start (point))
           (fun (if (> n 0)
                    #'skip-chars-forward
                  #'skip-chars-backward)))
      (dotimes (_ (abs n))
        (funcall fun delim)
        (funcall fun ndelim))
      (constrain-to-field nil start))))

(defun unix-word-rubout (arg)
  "Kill ARG Unix-words backwards.
A Unix-word is whitespace-delimited.
Interactively, ARG is the numeric prefix argument, defaulting to 1.
A negative ARG means to kill forwards.

Unix-words differ from Emacs words in that they are always delimited by
whitespace, regardless of the buffer's syntax table.
Thus, this command emulates C-w at the Unix terminal or shell.
See also this command's namesake in Info node
`(readline)Commands For Killing'."
  (interactive "^p")
  (let ((start (point)))
    (forward-unix-word (- arg))
    (kill-region start (point))))

(defun unix-filename-rubout (arg)
  "Kill ARG Unix-words backwards, also treating slashes as word delimiters.
A Unix-word is whitespace-delimited.
Interactively, ARG is the numeric prefix argument, defaulting to 1.
A negative ARG means to kill forwards.

This is like `unix-word-rubout' (which see), but `/' and `\\' are also
treated as delimiting words.  See this command's namesake in Info node
`(readline)Commands For Killing'."
  (interactive "^p")
  (let ((start (point)))
    (forward-unix-word (- arg) "\\\\/\s\f\n\r\t\v")
    (kill-region start (point))))

(defun file-user-uid ()
  "Return the connection-local effective uid.
This is similar to `user-uid', but may invoke a file name handler
based on `default-directory'.  See Info node `(elisp)Magic File
Names'.

If a file name handler is unable to retrieve the effective uid,
this function will instead return -1."
  (if-let* ((handler (find-file-name-handler default-directory 'file-user-uid)))
      (funcall handler 'file-user-uid)
    (user-uid)))

(defun file-group-gid ()
  "Return the connection-local effective gid.
This is similar to `group-gid', but may invoke a file name handler
based on `default-directory'.  See Info node `(elisp)Magic File
Names'.

If a file name handler is unable to retrieve the effective gid,
this function will instead return -1."
  (if-let* ((handler (find-file-name-handler default-directory 'file-group-gid)))
      (funcall handler 'file-group-gid)
    (group-gid)))

(defun upcase-dwim (arg)
  "Upcase words in the region, if active; if not, upcase word at point.
If the region is active, this function calls `upcase-region'.
Otherwise, it calls `upcase-word', with prefix argument passed to it
to upcase ARG words."
  (interactive "*p")
  (if (use-region-p)
      (upcase-region (region-beginning) (region-end) (region-noncontiguous-p))
    (upcase-word arg)))

(defun downcase-dwim (arg)
    "Downcase words in the region, if active; if not, downcase word at point.
If the region is active, this function calls `downcase-region'.
Otherwise, it calls `downcase-word', with prefix argument passed to it
to downcase ARG words."
  (interactive "*p")
  (if (use-region-p)
      (downcase-region (region-beginning) (region-end) (region-noncontiguous-p))
    (downcase-word arg)))

(defun get-scratch-buffer-create ()
  "Return the *scratch* buffer, creating a new one if needed."
  (or (get-buffer "*scratch*")
      (let ((scratch (get-buffer-create "*scratch*")))
        ;; Don't touch the buffer contents or mode unless we know that
        ;; we just created it.
        (with-current-buffer scratch
          (when initial-scratch-message
            (insert (substitute-command-keys initial-scratch-message))
            (set-buffer-modified-p nil))
          (funcall initial-major-mode)
          (when (eq initial-major-mode 'lisp-interaction-mode)
            (setq-local trusted-content :all)))
        scratch)))

(defvar goto-line-history nil
  "History of values entered with `goto-line'.")

(defcustom goto-line-history-local nil
  "Non-nil means use a local history for `goto-line'."
  :type 'boolean
  :group 'convenience)

(defun goto-line-read-args (&optional relative)
  "Read arguments for `goto-line' related commands."
  (if (and current-prefix-arg (not (consp current-prefix-arg)))
      (list (prefix-numeric-value current-prefix-arg))
    ;; Look for a default, a number in the buffer at point.
    (let* ((number (number-at-point))
           (default (and (natnump number) number))
           ;; Decide if we're switching buffers.
           (buffer
            (if (consp current-prefix-arg)
                (other-buffer (current-buffer) t)))
           (buffer-prompt
            (if buffer
                (concat " in " (buffer-name buffer))
              "")))
      ;; Has the buffer locality of `goto-line-history' changed?
      (cond ((and goto-line-history-local (not (local-variable-p 'goto-line-history)))
             (make-local-variable 'goto-line-history))
            ((and (not goto-line-history-local) (local-variable-p 'goto-line-history))
             (kill-local-variable 'goto-line-history)))
      ;; Read the argument, offering that number (if any) as default.
      (list (read-number (format "Goto%s line%s: "
                                 (if (buffer-narrowed-p)
                                     (if relative " relative" " absolute")
                                   "")
                                 buffer-prompt)
                         (list default (if (or relative (not (buffer-narrowed-p)))
                                           (line-number-at-pos)
                                         (save-restriction
                                           (widen)
                                           (line-number-at-pos))))
                         'goto-line-history)
            buffer))))

(defun goto-line (line &optional buffer relative interactive)
  "Go to LINE, counting from line 1 at beginning of buffer.
If called interactively, a numeric prefix argument specifies
LINE; without a numeric prefix argument, read LINE from the
minibuffer.

If called interactively with \\[universal-argument], switch to the
most recently selected other buffer and move to line LINE there.

If optional argument RELATIVE is non-nil, counting starts at the beginning
of the accessible portion of the (potentially narrowed) buffer.

If the variable `widen-automatically' is non-nil, cancel narrowing and
leave all lines accessible.  If `widen-automatically' is nil, just move
point to the edge of visible portion and don't change the buffer bounds.

Prior to moving point, this function sets the mark (without
activating it), unless Transient Mark mode is enabled and the
mark is already active.

A non-nil INTERACTIVE argument pushes the mark and switches the buffer
if optional argument BUFFER is non-nil.

This function is usually the wrong thing to use in a Lisp program.
What you probably want instead is something like:
  (goto-char (point-min))
  (forward-line (1- N))
If at all possible, an even better solution is to use char counts
rather than line counts."
  (interactive (append (goto-line-read-args) '(nil t)))
  ;; Switch to the desired buffer, one way or another.
  (when interactive
    (when buffer
      (let ((window (get-buffer-window buffer)))
	(if window (select-window window)
	  (switch-to-buffer-other-window buffer))))
    ;; Leave mark at previous position
    (or (region-active-p) (push-mark)))
  ;; Move to the specified line number in that buffer.
  (let ((pos (save-restriction
               (unless relative (widen))
               (goto-char (point-min))
               (if (eq selective-display t)
                   (re-search-forward "[\n\C-m]" nil 'end (1- line))
                 (forward-line (1- line)))
               (point))))
    (when (and (not relative)
               (buffer-narrowed-p)
               widen-automatically
               ;; Position is outside narrowed part of buffer
               (or (> (point-min) pos) (> pos (point-max))))
      (widen))
    (goto-char pos)))

(defun goto-line-relative (line &optional buffer interactive)
  "Go to LINE, counting from line at (point-min).
The line number is relative to the accessible portion of the narrowed
buffer.  The arguments BUFFER and INTERACTIVE are the same as in the
function `goto-line'."
  (interactive (append (goto-line-read-args t) t))
  (goto-line line buffer t interactive))

;;;; indent.el members.

(defun insert-tab (&optional arg)
  (let ((count (prefix-numeric-value arg)))
    (if (and abbrev-mode
	     (eq (char-syntax (preceding-char)) ?w))
	(expand-abbrev))
    (if indent-tabs-mode
	(insert-char ?\t count)
      (indent-to (* tab-width (+ count (/ (current-column) tab-width)))))))

(defun alter-text-property (from to prop func &optional object)
  "Programmatically change value of a text-property.
For each region between FROM and TO that has a single value for PROPERTY,
apply FUNCTION to that value and sets the property to the function's result.
Optional fifth argument OBJECT specifies the string or buffer to operate on."
  (let ((begin from)
	end val)
    (while (setq val (get-text-property begin prop object)
		 end (text-property-not-all begin to prop val object))
      (put-text-property begin end prop (funcall func val) object)
      (setq begin end))
    (if (< begin to)
	(put-text-property begin to prop (funcall func val) object))))

(defun set-left-margin (from to width)
  "Set the left margin of the region to WIDTH.
If `auto-fill-mode' is active, re-fill the region to fit the new margin.

Interactively, WIDTH is the prefix argument, if specified.
Without prefix argument, the command prompts for WIDTH."
  (interactive "r\nNSet left margin to column: ")
  (save-excursion
    ;; If inside indentation, start from BOL.
    (goto-char from)
    (skip-chars-backward " \t")
    (if (bolp) (setq from (point)))
    ;; Place end after whitespace
    (goto-char to)
    (skip-chars-forward " \t")
    (setq to (point-marker)))
  ;; Delete margin indentation first, but keep paragraph indentation.
  (delete-to-left-margin from to)
  (put-text-property from to 'left-margin width)
  (indent-rigidly from to width)
  (if auto-fill-function (save-excursion (fill-region from to nil t t)))
  (move-marker to nil))

(defun set-right-margin (from to width)
  "Set the right margin of the region to WIDTH.
If `auto-fill-mode' is active, re-fill the region to fit the new margin.

Interactively, WIDTH is the prefix argument, if specified.
Without prefix argument, the command prompts for WIDTH."
  (interactive "r\nNSet right margin to width: ")
  (save-excursion
    (goto-char from)
    (skip-chars-backward " \t")
    (if (bolp) (setq from (point))))
  (put-text-property from to 'right-margin width)
  (if auto-fill-function (save-excursion (fill-region from to nil t t))))

(defun increase-left-margin (from to inc)
  "Increase or decrease the `left-margin' of the region.
With no prefix argument, this adds `standard-indent' of indentation.
A prefix arg (optional third arg INC noninteractively) specifies the amount
to change the margin by, in characters.
If `auto-fill-mode' is active, re-fill the region to fit the new margin."
  (interactive "*r\nP")
  (setq inc (if inc (prefix-numeric-value inc) standard-indent))
  (save-excursion
    (goto-char from)
    (skip-chars-backward " \t")
    (if (bolp) (setq from (point)))
    (goto-char to)
    (setq to (point-marker)))
  (alter-text-property from to 'left-margin
		       (lambda (v) (max (- left-margin) (+ inc (or v 0)))))
  (indent-rigidly from to inc)
  (if auto-fill-function (save-excursion (fill-region from to nil t t)))
  (move-marker to nil))

(defun decrease-left-margin (from to inc)
  "Make the left margin of the region smaller.
With no prefix argument, decrease the indentation by `standard-indent'.
A prefix arg (optional third arg INC noninteractively) specifies the amount
to change the margin by, in characters.
If `auto-fill-mode' is active, re-fill the region to fit the new margin."
  (interactive "*r\nP")
  (setq inc (if inc (prefix-numeric-value inc) standard-indent))
  (increase-left-margin from to (- inc)))

(defun increase-right-margin (from to inc)
  "Increase the right-margin of the region.
With no prefix argument, increase the right margin by `standard-indent'.
A prefix arg (optional third arg INC noninteractively) specifies the amount
to change the margin by, in characters.  A negative argument decreases
the right margin width.
If `auto-fill-mode' is active, re-fill the region to fit the new margin."
  (interactive "r\nP")
  (setq inc (if inc (prefix-numeric-value inc) standard-indent))
  (save-excursion
    (alter-text-property from to 'right-margin
			 (lambda (v) (+ inc (or v 0))))
    (if auto-fill-function
	(fill-region from to nil t t))))

(defun decrease-right-margin (from to inc)
  "Make the right margin of the region smaller.
With no prefix argument, decrease the right margin by `standard-indent'.
A prefix arg (optional third arg INC noninteractively) specifies the amount
of width to remove, in characters.  A negative argument increases
the right margin width.
If `auto-fill-mode' is active, re-fills region to fit in new margin."
  (interactive "*r\nP")
  (setq inc (if inc (prefix-numeric-value inc) standard-indent))
  (increase-right-margin from to (- inc)))

;;;; bindings.el members.

(defun ignore-preserving-kill-region (&rest _)
  "Like `ignore', but don't overwrite `last-event' if it's `kill-region'."
  (declare (completion ignore))
  (interactive)
  (when (eq last-command 'kill-region)
    (setq this-command 'kill-region))
  nil)

(defun make-mode-line-mouse-map (mouse function)
  "Return a keymap with single entry for mouse key MOUSE on the mode line.
MOUSE is defined to run function FUNCTION with no args in the buffer
corresponding to the mode line clicked."
  (let ((map (make-sparse-keymap)))
    (define-key map (vector 'mode-line mouse) function)
    map))

(defun mode-line-toggle-read-only (event)
  "Like toggling `read-only-mode', for the mode-line."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (read-only-mode 'toggle)))

(defun mode-line-toggle-modified (event)
  "Toggle the buffer-modified flag from the mode-line."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (set-buffer-modified-p (not (buffer-modified-p)))
    (force-mode-line-update)))

(defun mode-line-widen (event)
  "Widen a buffer from the mode-line."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (widen)
    (force-mode-line-update)))

;; mule.el: eol mnemonics (C-side variables in GNU).
(defvar eol-mnemonic-unix ":")
(defvar eol-mnemonic-dos "(DOS)")
(defvar eol-mnemonic-mac "(Mac)")
(defvar eol-mnemonic-undecided ":")

(defun coding-system-eol-type-mnemonic (coding-system)
  "Return the string indicating end-of-line format of CODING-SYSTEM."
  (let* ((eol-type (coding-system-eol-type coding-system))
	 (val (cond ((eq eol-type 0) eol-mnemonic-unix)
		    ((eq eol-type 1) eol-mnemonic-dos)
		    ((eq eol-type 2) eol-mnemonic-mac)
		    (t eol-mnemonic-undecided))))
    (if (stringp val)
	val
      (char-to-string val))))

(defvar mode-line-eol-desc-cache nil)

(defun mode-line-eol-desc ()
  (let* ((eol (coding-system-eol-type buffer-file-coding-system))
	 (mnemonic (coding-system-eol-type-mnemonic buffer-file-coding-system))
	 (desc (assoc eol mode-line-eol-desc-cache)))
    (if (and desc (eq (cadr desc) mnemonic))
	(cddr desc)
      (if desc (setq mode-line-eol-desc-cache nil)) ;Flush the cache if stale.
      (setq desc
	    (propertize
	     mnemonic
	     'help-echo (format "End-of-line style: %s\nmouse-1: Cycle"
				(if (eq eol 0) "Unix-style LF"
				  (if (eq eol 1) "DOS-style CRLF"
				    (if (eq eol 2) "Mac-style CR"
				      "Undecided"))))
	     'keymap
	     (let ((map (make-sparse-keymap)))
	       (define-key map [mode-line mouse-1] 'mode-line-change-eol)
	       map)
	     'mouse-face 'mode-line-highlight))
      (push (cons eol (cons mnemonic desc)) mode-line-eol-desc-cache)
      desc)))

(defun mode-line-change-eol (event)
  "Cycle through the various possible kinds of end-of-line styles."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (let ((eol (coding-system-eol-type buffer-file-coding-system)))
      (set-buffer-file-coding-system
       (cond ((eq eol 0) 'dos) ((eq eol 1) 'mac) (t 'unix))))))

(defun mode-line-default-help-echo (window)
  "Return default help echo text for WINDOW's mode line."
  (let* ((frame (window-frame window))
         (line-1a
          ;; Show text to select window only if the window is not
          ;; selected.
          (not (eq window (frame-selected-window frame))))
         (line-1b
          ;; Show text to drag mode line if either the window is not
          ;; at the bottom of its frame or the minibuffer window of
          ;; this frame can be resized.  This matches a corresponding
          ;; check in `mouse-drag-mode-line'.
          (or (not (window-at-side-p window 'bottom))
              (let ((mini-window (minibuffer-window frame)))
                (and (eq frame (window-frame mini-window))
                     (or (minibuffer-window-active-p mini-window)
                         (not resize-mini-windows))))))
         (line-2
          ;; Show text make window occupy the whole frame
          ;; only if it doesn't already do that.
          (not (eq window (frame-root-window frame))))
         (line-3
          ;; Show text to delete window only if that's possible.
          (not (eq window (frame-root-window frame)))))
    (when (or line-1a line-1b line-2 line-3)
      (concat
       (when (or line-1a line-1b)
         (concat
          "mouse-1: "
          (when line-1a "Select window")
          (when line-1b
            (if line-1a " (drag to resize)" "Drag to resize"))
          (when (or line-2 line-3) "\n")))
       (when line-2
         (concat
          "mouse-2: Make window occupy whole frame"
          (when line-3 "\n")))
       (when line-3
         "mouse-3: Remove window from frame")))))

(defcustom mode-line-default-help-echo #'mode-line-default-help-echo
  "Default help text for the mode line.
If the value is a string, it specifies the tooltip or echo area
message to display when the mouse is moved over the mode line.
If the value is a function, call that function with one argument
- the window whose mode line to display.  If the text at the
mouse position has a `help-echo' text property, that overrides
this variable."
  :type '(choice
          (const :tag "No help" :value nil)
          function
          (string :value "mouse-1: Select (drag to resize)\n\
mouse-2: Make current window occupy the whole frame\n\
mouse-3: Remove current window from display"))
  :version "27.1"
  :group 'mode-line)

(defun mode-line-mule-info-help-echo (window _object _point)
  (with-current-buffer (window-buffer window)
    (if buffer-file-coding-system
	(format "Buffer coding system (%s): %s
mouse-1: Describe coding system
mouse-3: Set coding system"
		(if enable-multibyte-characters "multi-byte" "unibyte")
		(symbol-name buffer-file-coding-system))
      "Buffer coding system: none specified")))

(defun mode-line-read-only-help-echo (window _object _point)
  (format "Buffer is %s\nmouse-1: Toggle"
	  (if (buffer-local-value 'buffer-read-only (window-buffer window))
	      "read-only" "writable")))

(defun mode-line-modified-help-echo (window _object _point)
  (format "Buffer is %smodified\nmouse-1: Toggle modification state"
	  (if (buffer-modified-p (window-buffer window)) "" "not ")))

(defun mode-line-frame-control ()
  "Compute mode line construct for frame identification.
Value is used for `mode-line-frame-identification', which see."
  (if (or (null window-system)
	  (eq window-system 'pc))
      " %F  "
    "  "))

(defvar mode-line-frame-identification '(:eval (mode-line-frame-control))
  "Mode line construct to describe the current frame.")

(defvar-keymap mode-line-window-dedicated-keymap
  :doc "Keymap for what is displayed by `mode-line-window-dedicated'."
  "<mode-line> <mouse-1>" #'toggle-window-dedicated)

(defun mode-line-window-control ()
  "Compute mode line construct for window dedicated state.
Value is used for `mode-line-window-dedicated', which see."
  (cond
   ((eq (window-dedicated-p) t)
    (propertize
     "D"
     'help-echo "Window strongly dedicated to its buffer\nmouse-1: Toggle"
     'local-map mode-line-window-dedicated-keymap
     'mouse-face 'mode-line-highlight))
   ((window-dedicated-p)
    (propertize
     "d"
     'help-echo "Window dedicated to its buffer\nmouse-1: Toggle"
     'local-map mode-line-window-dedicated-keymap
     'mouse-face 'mode-line-highlight))
   (t "")))

(defvar mode-line-window-dedicated '(:eval (mode-line-window-control))
  "Mode line construct to describe the current window.")

(defun mode-line-bury-buffer (event)
  "Like `bury-buffer', but temporarily select EVENT's window."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (bury-buffer)))

(defun mode-line-unbury-buffer (event)
  "Call `unbury-buffer' in this window."
  (interactive "e")
  (with-selected-window (posn-window (event-start event))
    (unbury-buffer)))

(defun mode-line-other-buffer ()
  "Switch to the most recently selected buffer other than the current one."
  (interactive)
  (switch-to-buffer (other-buffer) nil t))

(defun mode-line-window-selected-p ()
  "Return non-nil if we're updating the mode line for the selected window.
This function is meant to be called in `:eval' mode line
constructs to allow altering the look of the mode line depending
on whether the mode line belongs to the currently selected window
or not."
  (let ((window (selected-window)))
    (or (eq window (old-selected-window))
	(and (minibuffer-window-active-p (minibuffer-window))
	     (with-selected-window (minibuffer-window)
	       (eq window (minibuffer-selected-window)))))))

(defvar-keymap mode-line-buffer-identification-keymap
  :doc "Keymap for what is displayed by `mode-line-buffer-identification'."
  ;; Add menu of buffer operations to the buffer identification part
  ;; of the mode line.or header line.
  ;; Bind down- events so that the global keymap won't ``shine
  ;; through''.
  "<mode-line> <mouse-1>"        #'mode-line-previous-buffer
  "<header-line> <down-mouse-1>" #'ignore
  "<header-line> <mouse-1>"      #'mode-line-previous-buffer
  "<mode-line> <mouse-3>"        #'mode-line-next-buffer
  "<header-line> <down-mouse-3>" #'ignore
  "<header-line> <mouse-3>"      #'mode-line-next-buffer)

(defun propertized-buffer-identification (fmt)
  "Return a list suitable for `mode-line-buffer-identification'.
FMT is a format specifier such as \"%12b\".  This function adds
text properties for face, help-echo, and local-map to it."
  (list (propertize fmt
		    'face 'mode-line-buffer-id
		    'help-echo
                    "Buffer name
mouse-1: Previous buffer\nmouse-3: Next buffer"
		    'mouse-face 'mode-line-highlight
		    'local-map mode-line-buffer-identification-keymap)))

(defvar-local mode-line-buffer-identification
  (propertized-buffer-identification "%12b")
  "Mode line construct for identifying the buffer being displayed.")

;;;; subr.el: progress reporters.

;; We could use a defstruct for the progress reporter, but that would
;; make `progress-reporter-update' slower because accessing the slots
;; via accessor functions is slower than `car'/`aref'.
;; `(car reporter)' gives the next update time or the pulse index.

(defvar progress-reporter-update-functions (list #'progress-reporter-echo-area)
  "Special hook run on progress-reporter updates.
Each function is called with three arguments:
REPORTER is the result of a call to `make-progress-reporter'.
STATE can be one of:
- A float representing the percentage complete in the range 0.0-1.0
for a numeric reporter.
- A monotonically increasing integer for a pulsing reporter.
- The symbol `done' to indicate that the progress reporter is complete.
UPDATE-TEXT is a string that a progress-reporter back-end might display
as a result of this update.  A typical use is as the \"step\" of the
progress reporting process.")

(defsubst progress-reporter-update (reporter &optional value update-text)
  "Report progress of an operation, by default, in the echo area.
REPORTER should be the result of a call to `make-progress-reporter'.
If REPORTER is a numerical progress reporter---i.e. if it was
made using non-nil MIN-VALUE and MAX-VALUE arguments to
`make-progress-reporter'---then VALUE should be a number between
MIN-VALUE and MAX-VALUE.
Optional argument UPDATE-TEXT is a string that a progress-reporter
back-end might display as a result of this update.  A typical use is as
the \"step\" of the progress reporting process.  If REPORTER is a
non-numerical reporter, then VALUE should be nil, or a string to use
instead of UPDATE-TEXT.
See `progress-reporter-update-functions' for the list of functions
called on each update.
This function is relatively inexpensive.  If the change since
last update is too small or insufficient time has passed, it does
nothing."
  (when (or (not (numberp value))      ; For pulsing reporter
	    (>= value (car reporter))) ; For numerical reporter
    (progress-reporter-do-update reporter value update-text)))

(defun make-progress-reporter (message &optional min-value max-value
				       current-value min-change min-time
                                       context)
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
effectively rounded up.
Optional CONTEXT can be nil or `async'.  It is consulted by back ends before
showing progress updates.  For example, when CONTEXT is `async',
the echo area progress reports may be muted if the echo area is busy."
  (when (string-match "[[:alnum:]]\\'" message)
    (setq message (concat message "...")))
  (unless min-time
    (setq min-time 0.2))
  (let ((reporter
	 (cons (or min-value 0)
	       ;; FIXME: Use defstruct.
	       (vector (if (>= min-time 0.02)
			   (float-time) nil)
		       min-value
		       max-value
		       message
		       (if min-change (max (min min-change 50) 1) 1)
                       min-time
                       ;; Unused (formerly SUFFIX).
                       nil
                       ;;
                       context))))
    ;; Force a call to `message' now.
    (progress-reporter-update reporter (or current-value min-value))
    reporter))

(defalias 'progress-reporter-make #'make-progress-reporter)

(defun progress-reporter-text (reporter)
  "Return REPORTER's text."
  (aref (cdr reporter) 3))

(defun progress-reporter-context (reporter)
  "Return REPORTER's context."
  (aref (cdr reporter) 7))

(defun progress-reporter-force-update (reporter &optional
                                                value new-message update-text)
  "Report progress of an operation in the echo area unconditionally.
REPORTER, VALUE, and UPDATE-TEXT are the same as in
`progress-reporter-update'.
NEW-MESSAGE, if non-nil, sets a new message for the reporter."
  (let ((parameters (cdr reporter)))
    (when new-message
      (aset parameters 3 new-message))
    (when (aref parameters 0)
      (aset parameters 0 (float-time)))
    (progress-reporter-do-update reporter value update-text)))

(defvar progress-reporter--pulse-characters ["-" "\\" "|" "/"]
  "Characters to use for pulsing progress reporters.")

(defun progress-reporter-echo-area (reporter state update-text)
  "Progress reporter echo area update function.
REPORTER, STATE, and UPDATE-TEXT are the same as in
`progress-reporter-update-functions'.
Do not emit a message if the reporter context is `async' and the echo
area is busy with something else."
  (let ((text (progress-reporter-text reporter)))
    (unless (and (eq (progress-reporter-context reporter) 'async)
                 (current-message)
                 (not (string-prefix-p text (current-message))))
      (setq update-text (if update-text (format " %s" update-text) ""))
      (pcase state
        ((pred floatp)
         (if (plusp state)
             (message "%s%d%%%s" text (* state 100.0) update-text)
           (message "%s%s" text update-text)))
        ((pred integerp)
         (let ((message-log-max nil)
               (pulse-char
                (aref progress-reporter--pulse-characters
                      (mod state (length progress-reporter--pulse-characters)))))
           (message "%s %s%s" text pulse-char update-text)))
        ('done
         (message "%sdone" text))))))

(defun progress-reporter-do-update (reporter value &optional update-text)
  (let* ((parameters      (cdr reporter))
	 (update-time     (aref parameters 0))
	 (min-value       (aref parameters 1))
	 (max-value       (aref parameters 2))
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
	       (run-hook-with-args 'progress-reporter-update-functions
				   reporter
				   (/ percentage 100.0)
				   update-text))))
	  ;; Pulsing indicator
	  (enough-time-passed
           (let ((index (1+ (car reporter))))
	     (setcar reporter index)
             (run-hook-with-args 'progress-reporter-update-functions
                                 reporter
                                 index
                                 (or update-text value)))))))

(defun progress-reporter-done (reporter)
  "Print reporter's message followed by word \"done\" in echo area.
Call the functions on `progress-reporter-update-functions`."
  (run-hook-with-args 'progress-reporter-update-functions
                      reporter
                      'done
                      nil))

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

;;;; indent.el: remaining members.

(defvar-keymap indent-rigidly-map
  :doc   "Transient keymap for adjusting indentation interactively.
It is activated by calling `indent-rigidly' interactively."
  "TAB"       #'indent-rigidly-right
  "<left>"    #'indent-rigidly-left
  "<right>"   #'indent-rigidly-right
  "S-<left>"  #'indent-rigidly-left-to-tab-stop
  "S-<right>" #'indent-rigidly-right-to-tab-stop)

(put 'indent-rigidly-right :advertised-binding (kbd "<right>"))

(defun indent-rigidly (start end arg &optional interactive)
  "Indent all lines starting in the region.
If called interactively with no prefix argument, activate a
transient mode in which the indentation can be adjusted interactively
by typing \\<indent-rigidly-map>\\[indent-rigidly-left], \\<indent-rigidly-map>\\[indent-rigidly-right], \\<indent-rigidly-map>\\[indent-rigidly-left-to-tab-stop], or \\<indent-rigidly-map>\\[indent-rigidly-right-to-tab-stop].
In addition, \\`TAB' is also bound (and calls `indent-rigidly-right').

Typing any other key exits this mode, and this key is then
acted upon as normally.  If `transient-mark-mode' is enabled,
exiting also deactivates the mark.

If called from a program, or interactively with prefix ARG,
indent all lines starting in the region forward by ARG columns.
If called from a program, START and END specify the beginning and
end of the text to act on, in place of the region.

Negative values of ARG indent backward, so you can remove all
indentation by specifying a large negative ARG."
  (interactive "r\nP\np")
  (if (and (not arg) interactive)
      (set-transient-map indent-rigidly-map t #'deactivate-mark
                         "Type %k to indent region interactively")
    (save-excursion
      (goto-char end)
      (setq end (point-marker))
      (goto-char start)
      (or (bolp) (forward-line 1))
      (while (< (point) end)
        (let ((indent (current-indentation))
              eol-flag)
          (save-excursion
            (skip-chars-forward " \t")
            (setq eol-flag (eolp)))
          (or eol-flag
              (indent-to (max 0 (+ indent (prefix-numeric-value arg))) 0))
          (delete-region (point) (progn (skip-chars-forward " \t") (point))))
        (forward-line 1))
      (move-marker end nil)
      ;; Keep the active region in transient mode.
      (when (eq (cadr overriding-terminal-local-map) indent-rigidly-map)
	(setq deactivate-mark nil)))))

(defun indent-rigidly--pop-undo ()
  (and (memq last-command '(indent-rigidly-left indent-rigidly-right
			    indent-rigidly-left-to-tab-stop
			    indent-rigidly-right-to-tab-stop))
       (consp buffer-undo-list)
       (eq (car buffer-undo-list) nil)
       (pop buffer-undo-list)))

(defun indent-rigidly-left (beg end)
  "Indent all lines between BEG and END leftward by one space."
  (interactive "r")
  (indent-rigidly--pop-undo)
  (indent-rigidly
   beg end
   (if (eq (current-bidi-paragraph-direction) 'right-to-left) 1 -1)))

(defun indent-rigidly-right (beg end)
  "Indent all lines between BEG and END rightward by one space."
  (interactive "r")
  (indent-rigidly--pop-undo)
  (indent-rigidly
   beg end
   (if (eq (current-bidi-paragraph-direction) 'right-to-left) -1 1)))

(defun indent-rigidly--current-indentation (beg end)
  "Return the smallest indentation in range from BEG to END.
Blank lines are ignored."
  (save-excursion
    (save-match-data
      (let ((beg (progn (goto-char beg) (line-beginning-position)))
            indent)
        (goto-char beg)
        (while (re-search-forward "^\\s-*[[:print:]]" end t)
          (setq indent (min (or indent (current-indentation))
                            (current-indentation))))
        indent))))

(defun indent-rigidly-left-to-tab-stop (beg end)
  "Indent all lines between BEG and END leftward to a tab stop."
  (interactive "r")
  (indent-rigidly--pop-undo)
  (let* ((current (indent-rigidly--current-indentation beg end))
	 (rtl (eq (current-bidi-paragraph-direction) 'right-to-left))
	 (next (indent-next-tab-stop current (if rtl nil 'prev))))
    (indent-rigidly beg end (- next current))))

(defun indent-rigidly-right-to-tab-stop (beg end)
  "Indent all lines between BEG and END rightward to a tab stop."
  (interactive "r")
  (indent-rigidly--pop-undo)
  (let* ((current (indent-rigidly--current-indentation beg end))
	 (rtl (eq (current-bidi-paragraph-direction) 'right-to-left))
	 (next (indent-next-tab-stop current (if rtl 'prev))))
    (indent-rigidly beg end (- next current))))

(defun indent--funcall-widened (func)
  (save-restriction
    (widen)
    (funcall func)))

(defun indent--default-inside-comment ()
  (unless (or (> (current-column) (current-indentation))
              (eq this-command last-command))
    (let ((ppss (syntax-ppss)))
      (when (nth 4 ppss)
        (indent-line-to
         (save-excursion
           (forward-line -1)
           (skip-chars-forward " \t")
           (when (< (1- (point)) (nth 8 ppss) (line-end-position))
             (goto-char (nth 8 ppss))
             (when (looking-at comment-start-skip)
               (goto-char (match-end 0))))
           (current-column)))
        t))))

(defun indent-region-line-by-line (start end)
  (save-excursion
    (setq end (copy-marker end))
    (goto-char start)
    (let ((pr (unless (minibufferp)
                (make-progress-reporter "Indenting region..." (point)
                                        (marker-position end)))))
      (while (< (point) end)
        (or (and (bolp) (eolp))
            (indent-according-to-mode t))
        (forward-line 1)
        (and pr (progress-reporter-update pr (point))))
      (and pr (progress-reporter-done pr))
      (move-marker end nil))))

(defun indent-accumulate-tab-stops (limit)
  (let ((tab 0) (tab-stops))
    (while (<= (setq tab (indent-next-tab-stop tab)) limit)
      (push tab tab-stops))
    (nreverse tab-stops)))

(defvar-keymap edit-tab-stops-map
  :doc "Keymap used in `edit-tab-stops'."
  "C-x C-s" #'edit-tab-stops-note-changes
  "C-c C-c" #'edit-tab-stops-note-changes)

(defvar edit-tab-stops-buffer nil
  "The buffer whose tab stops are being edited.
This matters if the variable `tab-stop-list' is local in that buffer.")

(defun edit-tab-stops ()
  "Edit the tab stops used by `tab-to-tab-stop'.
Creates a buffer *Tab Stops* containing text describing the tab stops.
A colon indicates a column where there is a tab stop.
You can add or remove colons and then do \\<edit-tab-stops-map>\\[edit-tab-stops-note-changes] to make changes take effect."
  (interactive)
  (setq edit-tab-stops-buffer (current-buffer))
  (switch-to-buffer (get-buffer-create "*Tab Stops*"))
  (use-local-map edit-tab-stops-map)
  (setq-local indent-tabs-mode nil)
  (overwrite-mode 1)
  (setq truncate-lines t)
  (erase-buffer)
  (let ((tabs tab-stop-list))
    (while tabs
      (indent-to (car tabs) 0)
      (insert ?:)
      (setq tabs (cdr tabs))))
  (let ((count 0))
    (insert ?\n)
    (while (< count 8)
      (insert (+ count ?0))
      (insert "         ")
      (setq count (1+ count)))
    (insert ?\n)
    (while (> count 0)
      (insert "0123456789")
      (setq count (1- count))))
  (insert (substitute-command-keys
           (concat "\nTo install changes, type \\<edit-tab-stops-map>"
                   "\\[edit-tab-stops-note-changes]")))
  (goto-char (point-min)))

(defun edit-tab-stops-note-changes ()
  "Put edited tab stops into effect."
  (interactive)
    (let (tabs)
      (save-excursion
	(goto-char 1)
	(end-of-line)
	(while (search-backward ":" nil t)
	  (setq tabs (cons (current-column) tabs))))
      (bury-buffer (prog1 (current-buffer)
			  (switch-to-buffer edit-tab-stops-buffer)))
      (setq tab-stop-list tabs))
  (message "Tab stops installed"))

;;; simple.el members (verbatim ports from GNU Emacs 31.1).

(defun set-hard-newline-properties (from to)
  (let ((sticky (get-text-property from 'rear-nonsticky)))
    (put-text-property from to 'hard 't)
    ;; If rear-nonsticky is not "t", add 'hard to rear-nonsticky list
    (if (and (listp sticky) (not (memq 'hard sticky)))
	(put-text-property from (point) 'rear-nonsticky
			   (cons 'hard sticky)))))

(defun make-separator-line (&optional length)
  "Make a string appropriate for usage as a visual separator line.
This uses the `separator-line' face.

If LENGTH is nil, use the window width."
  (if (or (display-graphic-p)
          (display-supports-face-attributes-p '(:underline t)))
      (if length
          (concat (propertize (make-string length ?\s) 'face 'separator-line)
                  "\n")
        (propertize "\n" 'face '(:inherit separator-line :extend t)))
    ;; In terminals (that don't support underline), use a line of dashes.
    (concat (propertize (make-string (or length (1- (window-width))) ?-)
                        'face 'separator-line)
            "\n")))

(defun region-modifiable-p (start end)
  "Return non-nil if the region contains no read-only text."
  (and (not (get-text-property start 'read-only))
       (eq end (next-single-property-change start 'read-only nil end))))

(defcustom copy-region-blink-delay 1
  "Time in seconds to delay after showing the other end of the region.
It's used by the command `kill-ring-save' and the function
`indicate-copied-region' to blink the cursor between point and mark.
The value 0 disables blinking."
  :type 'number
  :group 'killing
  :version "28.1")

(defcustom copy-region-blink-predicate #'region-indistinguishable-p
  "Whether the cursor must be blinked after a copy.
When this condition holds, and the copied region fits in the
current window, `kill-ring-save' will blink the cursor between
point and mark for `copy-region-blink-delay' seconds."
  :type '(radio (function-item region-indistinguishable-p)
                (function-item :doc "Always blink point and mark." always)
                (function-item :doc "Never blink point and mark." ignore)
                (function :tag "Other predicate function"))
  :group 'killing
  :version "29.1")

(defun region-indistinguishable-p ()
  "Whether the current region is not denoted visually.
This holds when the region is inactive, or when the `region' face
cannot be distinguished from the `default' face."
  (not (and (region-active-p)
            (face-differs-from-default-p 'region))))

(defun indicate-copied-region (&optional message-len)
  "Indicate that the region text has been copied interactively.
If the mark is visible in the selected window, blink the cursor between
point and mark if there is currently no active region highlighting.
The option `copy-region-blink-delay' can disable blinking.

If the mark lies outside the selected window, display an
informative message containing a sample of the copied text.  The
optional argument MESSAGE-LEN, if non-nil, specifies the length
of this sample text; it defaults to 40."
  (let ((mark (mark t))
	(point (point))
	;; Inhibit quitting so we can make a quit here
	;; look like a C-g typed as a command.
	(inhibit-quit t))
    (if (pos-visible-in-window-p mark (selected-window))
	;; Swap point-and-mark quickly so as to show the region that
	;; was selected.  Don't do it if the region is highlighted.
	(when (and (numberp copy-region-blink-delay)
		   (> copy-region-blink-delay 0)
		   (funcall copy-region-blink-predicate))
	  ;; Swap point and mark.
	  (set-marker (mark-marker) (point) (current-buffer))
	  (goto-char mark)
	  (sit-for copy-region-blink-delay)
	  ;; Swap back.
	  (set-marker (mark-marker) mark (current-buffer))
	  (goto-char point)
	  ;; If user quit, deactivate the mark
	  ;; as C-g would as a command.
	  (and quit-flag (region-active-p)
	       (deactivate-mark)))
      (let ((len (min (abs (- mark point))
		      (or message-len 40))))
	(if (< point mark)
	    ;; Don't say "killed" or "saved"; that is misleading.
	    (message "Copied text until \"%s\""
		     ;; Don't show newlines literally
		     (query-replace-descr
		      (buffer-substring-no-properties (- mark len) mark)))
	  (message "Copied text from \"%s\""
		   (query-replace-descr
		    (buffer-substring-no-properties mark (+ mark len)))))))))

(defun kill-forward-chars (arg)
  (if (listp arg) (setq arg (car arg)))
  (if (eq arg '-) (setq arg -1))
  (kill-region (point) (+ (point) arg)))

(defun kill-current-buffer ()
  "Kill the current buffer.
When called in the minibuffer, get out of the minibuffer
using `abort-recursive-edit'.

This is like `kill-this-buffer', but it doesn't have to be invoked
via the menu bar, and pays no attention to the menu-bar's frame."
  (interactive)
  (let ((frame (selected-frame)))
    (if (and (frame-live-p frame)
             (not (window-minibuffer-p (frame-selected-window frame))))
        (kill-buffer (current-buffer))
      (abort-recursive-edit))))

(defun pop-global-mark ()
  "Pop off global mark ring and jump to the top location."
  (interactive)
  ;; Pop entries that refer to non-existent buffers.
  (while (and global-mark-ring (not (marker-buffer (car global-mark-ring))))
    (setq global-mark-ring (cdr global-mark-ring)))
  (or global-mark-ring
      (error "No global mark set"))
  (let* ((marker (car global-mark-ring))
	 (buffer (marker-buffer marker))
	 (position (marker-position marker)))
    (setq global-mark-ring (nconc (cdr global-mark-ring)
				  (list (car global-mark-ring))))
    (set-buffer buffer)
    (or (and (>= position (point-min))
	     (<= position (point-max)))
	(if widen-automatically
	    (widen)
	  (error "Global mark position is outside accessible part of buffer %s"
                 (buffer-name buffer))))
    (goto-char position)
    (switch-to-buffer buffer)))

(defun turn-on-visual-line-mode ()
  (visual-line-mode 1))

(define-globalized-minor-mode global-visual-line-mode
  visual-line-mode turn-on-visual-line-mode)

(defun set-selective-display (arg)
  "Set `selective-display' to ARG; clear it if no arg.
When the value of `selective-display' is a number > 0,
lines whose indentation is >= that value are not displayed.
The variable `selective-display' has a separate value for each buffer."
  (interactive "P")
  (if (eq selective-display t)
      (error "selective-display already in use for marked lines"))
  (let ((current-vpos
	 (save-restriction
	   (narrow-to-region (point-min) (point))
	   (goto-char (window-start))
	   (vertical-motion (window-height)))))
    (setq selective-display
	  (and arg (prefix-numeric-value arg)))
    (recenter current-vpos))
  (set-window-start (selected-window) (window-start))
  (princ "selective-display set to " t)
  (prin1 selective-display t)
  (princ "." t))

(defun toggle-truncate-lines (&optional arg)
  "Toggle truncating of long lines for the current buffer.
When truncating is off, long lines are folded.
With prefix argument ARG, truncate long lines if ARG is positive,
otherwise fold them.  Note that in side-by-side windows, this
command has no effect if `truncate-partial-width-windows' is
non-nil."
  (interactive "P")
  (setq truncate-lines
	(if (null arg)
	    (not truncate-lines)
	  (> (prefix-numeric-value arg) 0)))
  (force-mode-line-update)
  (unless truncate-lines
    (let ((buffer (current-buffer)))
      (walk-windows (lambda (window)
		      (if (eq buffer (window-buffer window))
			  (set-window-hscroll window 0)))
		    nil t)))
  (message "Truncate long lines %s%s"
	   (if truncate-lines "enabled" "disabled")
           (if (and truncate-lines visual-line-mode)
               (progn
                 (visual-line-mode -1)
                 (setq truncate-lines t)
                 (format-message " and `visual-line-mode' disabled"))
             "")))

(defun toggle-word-wrap (&optional arg)
  "Toggle whether to use word-wrapping for continuation lines.
With prefix argument ARG, wrap continuation lines at word boundaries
if ARG is positive, otherwise wrap them at the right screen edge.
This command toggles the value of `word-wrap'.  It has no effect
if long lines are truncated."
  (interactive "P")
  (setq word-wrap
	(if (null arg)
	    (not word-wrap)
	  (> (prefix-numeric-value arg) 0)))
  (force-mode-line-update)
  (message "Word wrapping %s"
	   (if word-wrap "enabled" "disabled")))

(defcustom mail-user-agent 'message-user-agent
  "Your preference for a mail composition package.
Various Emacs Lisp packages (e.g. Reporter) require you to compose an
outgoing email message.  This variable lets you specify which
mail-sending package you prefer.

Valid values include:

  `message-user-agent'  -- use the Message package.
                           See Info node `(message)'.
  `sendmail-user-agent' -- use the Mail package.
                           See Info node `(emacs)Sending Mail'.
  `mh-e-user-agent'     -- use the Emacs interface to the MH mail system.
                           See Info node `(mh-e)'.
  `gnus-user-agent'     -- like `message-user-agent', but with Gnus
                           paraphernalia if Gnus is running, particularly
                           the Gcc: header for archiving.

Additional valid symbols may be available; check in the manual of
your mail user agent package for details.  You may also define
your own symbol to be used as value for this variable using
`define-mail-user-agent'.

See also `read-mail-command' concerning reading mail."
  :type '(radio (function-item :tag "Message package"
			       :format "%t\n"
			       message-user-agent)
		(function-item :tag "Mail package"
			       :format "%t\n"
			       sendmail-user-agent)
		(function-item :tag "Emacs interface to MH"
			       :format "%t\n"
			       mh-e-user-agent)
		(function-item :tag "Message with full Gnus features"
			       :format "%t\n"
			       gnus-user-agent)
		(symbol :tag "Other"))
  :version "23.2"                       ; sendmail->message
  :group 'mail)

(defcustom compose-mail-user-agent-warnings t
  "If non-nil, `compose-mail' warns about changes in `mail-user-agent'.
If the value of `mail-user-agent' is the default, and the user
appears to have customizations applying to the old default,
`compose-mail' issues a warning."
  :type 'boolean
  :version "23.2"
  :group 'mail)

(defun rfc822-goto-eoh ()
  "If the buffer starts with a mail header, move point to the header's end.
Otherwise, moves to `point-min'.
The end of the header is the start of the next line, if there is one,
else the end of the last line.  This function obeys RFC 822 (or later)."
  (goto-char (point-min))
  (when (re-search-forward
	 "^\\([:\n]\\|[^: \t\n]+[ \t\n]\\)" nil 'move)
    (goto-char (match-beginning 0))))

(defvar mail-encode-mml nil
  "If non-nil, mail-user-agent's `sendfunc' command should mml-encode
the outgoing message before sending it.")

(defun compose-mail (&optional to subject other-headers continue
		     switch-function yank-action send-actions
		     return-action)
  "Start composing a mail message to send.
This uses the user's chosen mail composition package
as selected with the variable `mail-user-agent'.
The optional arguments TO and SUBJECT specify recipients
and the initial Subject field, respectively.

OTHER-HEADERS is an alist specifying additional
header fields.  Elements look like (HEADER . VALUE) where both
HEADER and VALUE are strings.

By default, if an unsent message is already being composed, this
command will ask whether to erase the unsent message, and will not
start a new message if the user doesn't allow erasing.  However, if
CONTINUE is non-nil, it means to continue editing a message already
being composed without asking.  Interactively, CONTINUE is the prefix
argument.

SWITCH-FUNCTION, if non-nil, is a function to use to
switch to and display the buffer used for mail composition.

YANK-ACTION, if non-nil, is an action to perform, if and when necessary,
to insert the raw text of the message being replied to.
It has the form (FUNCTION . ARGS).  The user agent will apply
FUNCTION to ARGS, to insert the raw text of the original message.
\(The user agent will also run `mail-citation-hook', *after* the
original text has been inserted in this way.)

SEND-ACTIONS is a list of actions to call when the message is sent.
Each action has the form (FUNCTION . ARGS).

RETURN-ACTION, if non-nil, is an action for returning to the
caller.  It has the form (FUNCTION . ARGS).  The function is
called after the mail has been sent or put aside, and the mail
buffer buried."
  (interactive
   (list nil nil nil current-prefix-arg))

  ;; In Emacs 23.2, the default value of `mail-user-agent' changed
  ;; from sendmail-user-agent to message-user-agent.  Some users may
  ;; encounter incompatibilities.  This hack tries to detect problems
  ;; and warn about them.
  (and compose-mail-user-agent-warnings
       (eq mail-user-agent 'message-user-agent)
       (let (warn-vars)
	 (dolist (var '(mail-mode-hook mail-send-hook mail-setup-hook
			mail-citation-hook mail-archive-file-name
			mail-default-reply-to mail-mailing-lists
			mail-self-blind))
	   (and (boundp var)
		(symbol-value var)
		(push var warn-vars)))
	 (when warn-vars
	   (display-warning 'mail
			    (format-message "\
The default mail mode is now Message mode.
You have the following Mail mode variable%s customized:
\n  %s\n\nTo use Mail mode, set `mail-user-agent' to sendmail-user-agent.
To disable this warning, set `compose-mail-user-agent-warnings' to nil."
				    (if (> (length warn-vars) 1) "s" "")
				    (mapconcat 'symbol-name
					       warn-vars " "))))))

  (let ((function (get mail-user-agent 'composefunc)))
    (unless function
      (error "Invalid value for `mail-user-agent'"))
    (funcall function to subject other-headers continue switch-function
	     yank-action send-actions return-action)))

(defun compose-mail-other-window (&optional to subject other-headers continue
					    yank-action send-actions
					    return-action)
  "Like \\[compose-mail], but edit the outgoing message in another window.
If this command needs to split the current window, it by default obeys
the user options `split-height-threshold' and `split-width-threshold',
when it decides whether to split the window horizontally or vertically."
  (interactive (list nil nil nil current-prefix-arg))
  (compose-mail to subject other-headers continue
		'switch-to-buffer-other-window yank-action send-actions
		return-action))

(defvar mouse-leave-buffer-hook nil
  "Hook run when the user mouse-clicks in a window.")
(defvar completion-setup-hook nil
  "Normal hook run at the end of setting up a completion list buffer.")
(defvar completions-highlight-face nil
  "Face used for highlighting the current completion candidate.")

(defvar completion-list-mode-map
  (let ((map (make-sparse-keymap)))
    (set-keymap-parent map special-mode-map)
    (define-key map "g" nil) ;; There's nothing to revert from.
    (define-key map [mouse-2] 'choose-completion)
    (define-key map [follow-link] 'mouse-face)
    (define-key map [down-mouse-2] nil)
    (define-key map "\C-m" 'choose-completion)
    (define-key map "\e\e\e" 'delete-completion-window)
    (define-key map [remap keyboard-quit] #'delete-completion-window)
    (define-key map [up] 'previous-line-completion)
    (define-key map [down] 'next-line-completion)
    (define-key map [left] 'previous-column-completion)
    (define-key map [right] 'next-column-completion)
    (define-key map [?\t] 'next-completion)
    (define-key map [backtab] 'previous-completion)
    (define-key map [M-up] 'minibuffer-previous-completion)
    (define-key map [M-down] 'minibuffer-next-completion)
    (define-key map "\M-\r" 'minibuffer-choose-completion)
    (define-key map "z" 'kill-current-buffer)
    (define-key map "n" 'next-completion)
    (define-key map "p" 'previous-completion)
    (define-key map "\M-g\M-c" 'switch-to-minibuffer)
    map)
  "Local map for completion list buffers.")
;; Completion mode is suitable only for specially formatted data.
(put 'completion-list-mode 'mode-class 'special)
(defvar completion-reference-buffer nil
  "Record the buffer that was current when the completion list was requested.
This is a local variable in the completion list buffer.
Initial value is nil to avoid some compiler warnings.")
(defvar completion-no-auto-exit nil
  "Non-nil means `choose-completion-string' should never exit the minibuffer.
This also applies to other functions such as `choose-completion'.")
(defvar completion-base-position nil
  "Position of the base of the text corresponding to the shown completions.
This variable is used in the *Completions* buffers.
Its value is a list of the form (START END) where START is the place
where the completion should be inserted and END (if non-nil) is the end
of the text to replace.  If END is nil, point is used instead.")
(defvar completion-list-insert-choice-function #'completion--replace
  "Function to use to insert the text chosen in *Completions*.
Called with three arguments (BEG END TEXT), it should replace the text
between BEG and END with TEXT.  Expected to be set buffer-locally
in the *Completions* buffer.")
(defun delete-completion-window ()
  "Delete the completion list window.
Go to the window from which completion was requested."
  (interactive)
  (let ((buf completion-reference-buffer))
    (if (one-window-p t)
	(if (window-dedicated-p) (delete-frame))
      (delete-window (selected-window))
      (if (get-buffer-window buf)
	  (select-window (get-buffer-window buf))))))
(defcustom completion-auto-wrap t
  "Non-nil means to wrap around when selecting completion candidates.
This affects the commands `next-completion', `previous-completion',
`next-line-completion' and `previous-line-completion'.
When `completion-auto-select' is t, it wraps through the minibuffer
for the commands bound to the TAB key."
  :type 'boolean
  :version "29.1"
  :group 'completion)
(defcustom completion-auto-select nil
  "If non-nil, automatically select the window showing the *Completions* buffer.
When the value is t, pressing TAB will switch to the completion list
buffer when Emacs pops up a window showing that buffer.
If the value is `second-tab', then the first TAB will pop up the
window showing the completions list buffer, and the next TAB will
select that window.
See `completion-auto-help' for controlling when the window showing
the completions is popped up and down."
  :type '(choice (const :tag "Don't auto-select completions window" nil)
                 (const :tag "Select completions window on first TAB" t)
                 (const :tag "Select completions window on second TAB"
                        second-tab))
  :version "29.1"
  :group 'completion)
(defun first-completion ()
  "Move to the first item in the completions buffer."
  (interactive)
  (goto-char (point-min))
  (if (get-text-property (point) 'mouse-face)
      (unless (get-text-property (point) 'first-completion)
        (let ((inhibit-read-only t))
          (add-text-properties (point) (min (1+ (point)) (point-max))
                               '(first-completion t))))
    (when-let* ((pos (next-single-property-change (point) 'mouse-face)))
      (goto-char pos))))
(defun last-completion ()
  "Move to the last item in the completions buffer."
  (interactive)
  ;; Move to the last item in horizontal or one-column format.
  (goto-char (previous-single-property-change
              (point-max) 'mouse-face nil (point-min)))
  ;; Move to the start of the item.
  (unless (get-text-property (point) 'mouse-face)
    (when-let* ((pos (previous-single-property-change (point) 'mouse-face)))
      (goto-char pos)))
  ;; In vertical format the last item is in the last column even if its
  ;; line number is less than that of the last item in earlier columns.
  (when (eq completions-format 'vertical)
    (let ((pt (point))
          (col (current-column))
          (last-col (progn
                      (first-completion)
                      (goto-char (pos-eol))
                      ;; Go to the beginning of the candidate.  We loop
                      ;; to move past any completion annotations.
                      (while (not (get-text-property (point) 'mouse-face))
                        (goto-char
                         (previous-single-property-change (point) 'mouse-face)))
                      (current-column))))
      (if (zerop last-col)
          ;; If there is only one column of completions, the last
          ;; completion in vertical format is the same as in horizontal
          ;; format, so go there now.
          (goto-char pt)
        ;; Otherwise, we set `pt' to the beginning of first item in last
        ;; column here because if the last column contains only one
        ;; item, `pt' will not be set below.)
        (setq pt (point))
        ;; If all columns contain the same number of items, `col' (which
        ;; specifies the column of the last item in horizontal format)
        ;; equals `last-col', so the test must be with `>=', not `>'.
        (when (>= last-col col)
          (while (= (current-column) last-col)
            (forward-line)
            (unless (eobp)
              (goto-char (pos-eol))
              (move-to-column last-col)
              (when (= (current-column) last-col)
                (setq pt (point))))))
        (goto-char pt)))))
(defun previous-column-completion (n)
  "Move to the item in the previous column of the completions buffer.
With prefix argument N, move back N columns (negative N means move
forward).

Also see the `completion-auto-wrap' variable."
  (interactive "p")
  (next-column-completion (- n)))
(defun completion--move-to-candidate-start ()
  "If in a completion candidate, move point to its start."
  (when (and (get-text-property (point) 'mouse-face)
             (not (bobp))
             (get-text-property (1- (point)) 'mouse-face))
    (goto-char (previous-single-property-change (point) 'mouse-face))))
(defun completion--move-to-candidate-end ()
  "If in a completion candidate, move point to its end.
More precisely, point moves the the position immediately after the last
character of the completion candidate."
  (when (get-text-property (point) 'mouse-face)
    (goto-char (or (next-single-property-change (point) 'mouse-face)
                   (point-max)))))
(defun next-column-completion (n)
  "Move to the item in the next column of the completions buffer.
With prefix argument N, move N columns (negative N means move
backward).

Also see the `completion-auto-wrap' variable."
  (interactive "p")
  (let ((tabcommand (member (this-command-keys) '("\t" [backtab])))
        (one-col (save-excursion
                   (first-completion)
                   (completion--move-to-candidate-end)
                   (eolp)))
        pos line last first)
    (catch 'bound
      (when (and (bobp)
                 (> n 0)
                 (get-text-property (point) 'mouse-face)
                 (not (get-text-property (point) 'first-completion)))
        (let ((inhibit-read-only t))
          (add-text-properties (point) (1+ (point)) '(first-completion t)))
        (setq n (1- n)))

      (while (> n 0)
        (setq pos (point) line (line-number-at-pos)
              last (if one-col
                       (save-excursion (and (forward-line) (eobp)))
                     (save-excursion (completion--move-to-candidate-end) (eolp))))
        ;; If in a completion, move to the end of it.
        (when (get-text-property pos 'mouse-face)
          (setq pos (next-single-property-change pos 'mouse-face)))
        (when pos (setq pos (next-single-property-change pos 'mouse-face)))
        (if (and pos
                 (if last
                     (not (eq completions-format 'vertical))
                   t))
            ;; Move to the start of next one.
            (goto-char pos)
          ;; If at the last completion option, wrap or skip
          ;; to the minibuffer, if requested.
          (when (and completion-auto-wrap
                     (or one-col
                         (not (eq completions-format 'vertical))))
            (if (and (eq completion-auto-select t) tabcommand
                     (minibufferp completion-reference-buffer))
                (progn
                  (completions--clear-selection)
                  (throw 'bound nil))
              (first-completion))))
        (when (and (eq completions-format 'vertical)
                   (or last
                       (= (point) (save-excursion (first-completion) (point)))))
          (if (> (line-number-at-pos) line)
              (forward-line -1)
            (when completion-auto-wrap
              (goto-char (pos-bol))
              (completion--move-to-candidate-start))))
        (setq n (1- n)))

      (while (< n 0)
        (setq pos (point) line (line-number-at-pos)
              first (if one-col
                        (save-excursion
                          (forward-line -1)
                          (not (get-text-property (point) 'mouse-face)))
                      (save-excursion (completion--move-to-candidate-start)
                                      (bolp))))
        ;; If in a completion, move to the start of it.
        (when (and (get-text-property pos 'mouse-face)
                   (not (bobp))
                   (get-text-property (1- pos) 'mouse-face))
          (setq pos (previous-single-property-change pos 'mouse-face)))
        (when pos (setq pos (previous-single-property-change pos 'mouse-face)))
        (if (and pos
                 (not (and completion-auto-wrap
                           (eq completions-format 'vertical)
                           (not one-col)
                           (bolp)))
                 (if first
                     (not (eq completions-format 'vertical))
                   t))
            (progn
              (goto-char pos)
              ;; Move to the start of that one.
              (unless (get-text-property (point) 'mouse-face)
                (goto-char (previous-single-property-change
                            (point) 'mouse-face nil (point-min)))))
          ;; If at the first completion option, wrap or skip
          ;; to the minibuffer, if requested.
          (when completion-auto-wrap
            (cond ((and (eq completions-format 'vertical)
                        (not one-col)
                        (or first (not pos)))
                   (when (> line (line-number-at-pos))
                     (forward-line))
                   (goto-char (1- (pos-eol)))
                   (completion--move-to-candidate-start))
                  ((and (eq completion-auto-select t) tabcommand
                        (minibufferp completion-reference-buffer))
                   (completions--clear-selection)
                   (throw 'bound nil))
                  (t
                   (last-completion)))))
        (setq n (1+ n))))

    (when (/= 0 n)
      (switch-to-minibuffer))))
(defun previous-line-completion (&optional n)
  "Move to completion candidate on the previous line in the completions buffer.
With prefix argument N, move back N lines (negative N means move
forward).  In vertical format (see user option `completions-format')
this command moves line-wise through all columns in the completions
buffer, in horizontal format movement is confined to the current column
of completions.

Also see the `completion-auto-wrap' variable."
  (interactive "p")
  (next-line-completion (- n)))
(defun next-line-completion (&optional n)
  "Move to completion candidate on the next line in the completions buffer.
With prefix argument N, move N lines forward (negative N means move
backward).  In vertical format (see user option `completions-format')
this command moves line-wise through all columns in the completions
buffer, in horizontal format movement is confined to the current column
of completions.

Also see the `completion-auto-wrap' variable."
  (interactive "p")
  (let (line column pos found last first)
    (when (and (bobp)
               (> n 0)
               (get-text-property (point) 'mouse-face)
               (not (get-text-property (point) 'first-completion)))
      (let ((inhibit-read-only t))
        (add-text-properties (point) (1+ (point)) '(first-completion t)))
      (setq n (1- n)))

    (if (get-text-property (point) 'mouse-face)
        ;; If in a completion, move to the start of it.
        (completion--move-to-candidate-start)
      ;; Try to move to the previous completion.
      (setq pos (previous-single-property-change (point) 'mouse-face))
      (if pos
          ;; Move to the start of the previous completion.
          (progn
            (goto-char pos)
            (unless (get-text-property (point) 'mouse-face)
              (goto-char (previous-single-property-change
                          (point) 'mouse-face nil (point-min)))))
        (cond ((> n 0) (setq n (1- n)) (first-completion))
              ((< n 0) (first-completion)))))

    (while (> n 0)
      (setq found nil pos (point) column (current-column)
            line (line-number-at-pos)
            last (= (point) (save-excursion (last-completion) (point))))
      (if (and (eq completions-format 'vertical)
               completion-auto-wrap last)
          (first-completion)            ; Wrap from last to first item.
        (completion--move-to-candidate-end)
        (while (and (not found)
                    (eq (forward-line 1) 0)
                    (not (eobp))
                    (move-to-column column))
          (when (get-text-property (point) 'mouse-face)
            (setq found t)))
        (when (not found)
          (if (and (not completion-auto-wrap)
                   (if (eq completions-format 'vertical)
                       (and (or last (get-text-property (point) 'mouse-face))
                            (last-completion))
                     (goto-char pos)))
              t
            (save-excursion
              (setq pos nil)
              (goto-char (point-min))
              (when (and (eq (move-to-column column) column)
                         (get-text-property (point) 'mouse-face))
                (setq pos (point)))
              (while (and (not pos) (> line (line-number-at-pos)))
                (forward-line 1)
                (when (and (eq (move-to-column column) column)
                           (get-text-property (point) 'mouse-face))
                  (setq pos (point)))))
            (if pos (goto-char pos))
            (when (eq completions-format 'vertical)
              (next-column-completion 1)))))   ; Move to next column.
      (setq n (1- n)))

    (while (< n 0)
      (setq found nil pos (point) column (current-column)
            line (line-number-at-pos)
            first (= (point) (save-excursion (first-completion) (point))))
      (if (and (eq completions-format 'vertical)
               completion-auto-wrap first)
          (last-completion)             ; Wrap from first to last item.
        (completion--move-to-candidate-start)
        (while (and (not found)
                    (eq (forward-line -1) 0)
                    (move-to-column column))
          (when (get-text-property (point) 'mouse-face)
            (setq found t)))
        (when (not found)
          (if (and (not completion-auto-wrap)
                   (if (eq completions-format 'vertical)
                       (and (or last first
                                (get-text-property (point) 'mouse-face))
                            first (first-completion))
                     (goto-char pos)))
              t
            (save-excursion
              (setq pos nil)
              (goto-char (point-max))
              (when (and (eq (move-to-column column) column)
                         (get-text-property (point) 'mouse-face))
                (setq pos (point)))
              (while (and (not pos) (< line (line-number-at-pos)))
                (forward-line -1)
                (when (and (eq (move-to-column column) column)
                           (get-text-property (point) 'mouse-face))
                  (setq pos (point)))))
            (if pos (goto-char pos))
            (when (eq completions-format 'vertical)
              (previous-column-completion 1)   ; Move to previous column.
              (setq column (current-column))
              ;; Move to last item in this column (previous column may
              ;; have fewer items).
              (while (not (eobp))
                (move-to-column column)
                (setq pos (point))
                (forward-line))
              (goto-char pos)))))
      (setq n (1+ n)))))
(defun next-completion (&optional n)
  "Move according to `completions-format' to next completion item.
In horizontal format movement is between columns within the same line,
in vertical format between lines within the same column.  With non-nil
`completion-auto-wrap', movement continues to the next line or column,
respectively."
  (interactive "p")
  (pcase completions-format
    ('vertical (next-line-completion n))
    (_ (next-column-completion n))))
(defun previous-completion (&optional n)
  "Move according to `completions-format' to previous completion item.
In horizontal format movement is between columns within the same line,
in vertical format between lines within the same column.  With non-nil
`completion-auto-wrap', movement continues to the next line or column,
respectively."
  (interactive "p")
  (pcase completions-format
    ('vertical (previous-line-completion n))
    (_ (previous-column-completion n))))
(defvar choose-completion-deselect-if-after nil
  "If non-nil, don't choose a completion candidate if point is right after it.

This makes `completions--deselect' effective.")
(defun completion-list-candidate-at-point (&optional pt)
  "Candidate string and bounds at PT in completions buffer.
The return value has the format (STR BEG END).
The optional argument PT defaults to (point)."
  (setq pt (or pt (point)))
  (when (cond
         ((and (/= pt (point-max))
               (get-text-property pt 'completion--string))
          (incf pt))
         ((and (/= pt (point-min))
               (get-text-property (1- pt) 'completion--string))))
    (setq pt (or (previous-single-property-change pt 'completion--string) pt))
    (list (get-text-property pt 'completion--string) pt
          (or (next-single-property-change pt 'completion--string)
              (point-max)))))
(defun choose-completion (&optional event no-exit no-quit)
  "Choose the completion at point.
If EVENT, use EVENT's position to determine the starting position.
With prefix argument NO-EXIT, insert the completion at point to the
minibuffer, but don't exit the minibuffer.  When the prefix argument
is not provided, then whether to exit the minibuffer depends on the value
of `completion-no-auto-exit'.
If NO-QUIT is non-nil, insert the completion at point to the
minibuffer, but don't quit the completions window."
  (interactive (list last-nonmenu-event current-prefix-arg))
  ;; In case this is run via the mouse, give temporary modes such as
  ;; isearch a chance to turn off.
  (run-hooks 'mouse-leave-buffer-hook)
  (with-current-buffer (window-buffer (posn-window (event-start event)))
    (let ((buffer completion-reference-buffer)
          (base-position completion-base-position)
          (insert-function completion-list-insert-choice-function)
          (completion-no-auto-exit (if no-exit t completion-no-auto-exit))
          (choice
           (if choose-completion-deselect-if-after
               (or (get-text-property (posn-point (event-start event))
                                      'completion--string)
                   (error "No completion here"))
             (or (car (completion-list-candidate-at-point
                       (posn-point (event-start event))))
                 (error "No completion here")))))

      (unless (buffer-live-p buffer)
        (error "Destination buffer is dead"))
      (unless no-quit
        (quit-window nil (posn-window (event-start event))))

      (with-current-buffer buffer
        (choose-completion-string
         choice buffer
         (or base-position
             ;; If all else fails, just guess.
             (list (choose-completion-guess-base-position choice)))
         insert-function)))))
;; Delete the longest partial match for STRING
;; that can be found before POINT.
(defun choose-completion-guess-base-position (string)
  (save-excursion
    (let ((opoint (point))
          len)
      ;; Try moving back by the length of the string.
      (goto-char (max (- (point) (length string))
                      (minibuffer-prompt-end)))
      ;; See how far back we were actually able to move.  That is the
      ;; upper bound on how much we can match and delete.
      (setq len (- opoint (point)))
      (if completion-ignore-case
          (setq string (downcase string)))
      (while (and (> len 0)
                  (let ((tail (buffer-substring (point) opoint)))
                    (if completion-ignore-case
                        (setq tail (downcase tail)))
                    (not (string= tail (substring string 0 len)))))
        (setq len (1- len))
        (forward-char 1))
      (point))))
(defvar choose-completion-string-functions nil
  "Functions that may override the normal insertion of a completion choice.
These functions are called in order with three arguments:
CHOICE - the string to insert in the buffer,
BUFFER - the buffer in which the choice should be inserted,
BASE-POSITION - where to insert the completion.

Functions should also accept and ignore a potential fourth
argument, passed for backwards compatibility.

If a function in the list returns non-nil, that function is supposed
to have inserted the CHOICE in the BUFFER, and possibly exited
the minibuffer; no further functions will be called.

If all functions in the list return nil, that means to use
the default method of inserting the completion in BUFFER.")
(defun choose-completion-string (choice &optional
                                        buffer base-position insert-function)
  "Switch to BUFFER and insert the completion choice CHOICE.
BASE-POSITION says where to insert the completion.
INSERT-FUNCTION says how to insert the completion and falls
back on `completion-list-insert-choice-function' when nil."

  ;; If BUFFER is the minibuffer, exit the minibuffer
  ;; unless it is reading a file name and CHOICE is a directory,
  ;; or completion-no-auto-exit is non-nil.

  (let* ((buffer (or buffer completion-reference-buffer))
	 (mini-p (minibufferp buffer)))
    ;; If BUFFER is a minibuffer, barf unless it's the currently
    ;; active minibuffer.
    (if (and mini-p
             (not (and (active-minibuffer-window)
                       (equal buffer
			     (window-buffer (active-minibuffer-window))))))
	(error "Minibuffer is not active for completion")
      ;; Set buffer so buffer-local choose-completion-string-functions works.
      (set-buffer buffer)
      (unless (run-hook-with-args-until-success
	       'choose-completion-string-functions
               ;; The fourth arg used to be `mini-p' but was useless
               ;; (since minibufferp can be used on the `buffer' arg)
               ;; and indeed unused.  The last used to be `base-size', so we
               ;; keep it to try and avoid breaking old code.
	       choice buffer base-position nil)
        ;; This remove-text-properties should be unnecessary since `choice'
        ;; comes from buffer-substring-no-properties.
        ;;(remove-text-properties 0 (length choice) '(mouse-face nil) choice)
	;; Insert the completion into the buffer where it was requested.
        (funcall (or insert-function completion-list-insert-choice-function)
                 (or (car base-position) (point))
                 (or (cadr base-position) (point))
                 choice)
        ;; Update point in the window that BUFFER is showing in.
	(let ((window (get-buffer-window buffer t)))
	  (set-window-point window (point)))
	;; If completing for the minibuffer, exit it with this choice.
	(and (not completion-no-auto-exit)
             (minibufferp buffer)
	     minibuffer-completion-table
	     ;; If this is reading a file name, and the file name chosen
	     ;; is a directory, don't exit the minibuffer.
             (let* ((result (buffer-substring (field-beginning) (point)))
                    (bounds
                     (completion-boundaries result minibuffer-completion-table
                                            minibuffer-completion-predicate
                                            "")))
               (if (eq (car bounds) (length result))
                   ;; The completion chosen leads to a new set of completions
                   ;; (e.g. it's a directory): don't exit the minibuffer yet.
                   (let ((mini (active-minibuffer-window)))
                     (select-window mini)
                     (when minibuffer-auto-raise
                       (raise-frame (window-frame mini))))
                 (exit-minibuffer))))))))
(define-derived-mode completion-list-mode nil "Completion List"
  "Major mode for buffers showing lists of possible completions.
Type \\<completion-list-mode-map>\\[choose-completion] in the completion list\
 to select the completion near point.
Or click to select one with the mouse.

See the `completions-format' user option to control how this
buffer is formatted.

\\{completion-list-mode-map}")
(defun completion-list-mode-finish ()
  "Finish setup of the completions buffer.
Called from `temp-buffer-show-hook'."
  (when (eq major-mode 'completion-list-mode)
    (setq buffer-read-only t)))
(add-hook 'temp-buffer-show-hook 'completion-list-mode-finish)
;; Variables and faces used in `completion-setup-function'.
(defcustom completion-show-help t
  "Non-nil means show help message in *Completions* buffer."
  :type 'boolean
  :version "22.1"
  :group 'completion)
(defvar minibuffer-visible-completions--always-bind)
;; This function goes in completion-setup-hook, so that it is called
;; after the text of the completion list buffer is written.
(defun completion-setup-function ()
  (let* ((mainbuf (current-buffer))
         (base-dir
          ;; FIXME: This is a bad hack.  We try to set the default-directory
          ;; in the *Completions* buffer so that the relative file names
          ;; displayed there can be treated as valid file names, independently
          ;; from the completion context.  But this suffers from many problems:
          ;; - It's not clear when the completions are file names.  With some
          ;;   completion tables (e.g. bzr revision specs), the listed
          ;;   completions can mix file names and other things.
          ;; - It doesn't pay attention to possible quoting.
          ;; - With fancy completion styles, the code below will not always
          ;;   find the right base directory.
          (if minibuffer-completing-file-name
              (file-name-directory
               (expand-file-name
                (buffer-substring (minibuffer-prompt-end) (point)))))))
    (with-current-buffer standard-output
      (let ((base-position completion-base-position)
            (insert-fun completion-list-insert-choice-function)
            (lazy-button completions--lazy-insert-button))
        (completion-list-mode)
        (when completions-highlight-face
          (setq-local cursor-face-highlight-nonselected-window t))
        (setq-local completions--lazy-insert-button lazy-button)
        (setq-local completion-base-position base-position)
        (setq-local completion-list-insert-choice-function insert-fun))
      (setq-local completion-reference-buffer mainbuf)
      (if base-dir (setq default-directory base-dir))
      (when completion-tab-width
        (setq tab-width completion-tab-width))
      (add-hook 'window-scroll-functions
                'completion--lazy-insert-strings-on-scroll nil t)
      ;; Maybe enable cursor completions-highlight.
      (when completions-highlight-face
        (cursor-face-highlight-mode 1))
      ;; Maybe insert help string.
      (when completion-show-help
	(goto-char (point-min))
        (let ((helps
               (with-current-buffer (window-buffer (active-minibuffer-window))
                 (let ((minibuffer-visible-completions--always-bind t))
                   (list
                    (substitute-command-keys
	             (if (display-mouse-p)
	                 "Click or type \\[minibuffer-choose-completion] on a completion to select it.\n"
                       "Type \\[minibuffer-choose-completion] on a completion to select it.\n"))
                    (if (eq minibuffer-visible-completions t)
                        (substitute-command-keys
		         "Type \\[minibuffer-next-completion], \\[minibuffer-previous-completion], \
\\[minibuffer-next-line-completion], \\[minibuffer-previous-line-completion] \
to move point between completions.\n\n")
                      (substitute-command-keys
		       "Type \\[minibuffer-next-completion] or \\[minibuffer-previous-completion] \
to move point between completions.\n\n")))))))
          (dolist (help helps)
            (insert help)))))))
(add-hook 'completion-setup-hook #'completion-setup-function)
(defun switch-to-completions ()
  "Select the completion list window."
  (interactive)
  (when-let* ((window (or (minibuffer--completions-visible)
		          ;; Make sure we have a completions window.
                          (progn (minibuffer-completion-help)
                                 (minibuffer--completions-visible)))))
    (select-window window)
    (completion--lazy-insert-strings)
    (when (bobp)
      (cond
       ((and (memq this-command '(completion-at-point minibuffer-complete))
             (equal (this-command-keys) [backtab]))
        (goto-char (point-max))
        (last-completion))
       (t (first-completion))))))
(defun read-expression-switch-to-completions ()
  "Select the completion list window while reading an expression."
  (interactive)
  (completion-help-at-point)
  (switch-to-completions))
(defun switch-to-minibuffer ()
  "Select the minibuffer window."
  (interactive)
  (when (active-minibuffer-window)
    (select-window (active-minibuffer-window))))
;;; Support keyboard commands to turn on various modifiers.
;; These functions -- which are not commands -- each add one modifier
;; to the following event.
(defun event-apply-alt-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Alt modifier to the following event.
For example, type \\[event-apply-alt-modifier] & to enter Alt-&."
  (vector (event-apply-modifier (read-event) 'alt 22 "A-")))
(defun event-apply-super-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Super modifier to the following event.
For example, type \\[event-apply-super-modifier] & to enter Super-&."
  (vector (event-apply-modifier (read-event) 'super 23 "s-")))
(defun event-apply-hyper-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Hyper modifier to the following event.
For example, type \\[event-apply-hyper-modifier] & to enter Hyper-&."
  (vector (event-apply-modifier (read-event) 'hyper 24 "H-")))
(defun event-apply-shift-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Shift modifier to the following event.
For example, type \\[event-apply-shift-modifier] & to enter Shift-&."
  (vector (event-apply-modifier (read-event) 'shift 25 "S-")))
(defun event-apply-control-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Ctrl modifier to the following event.
For example, type \\[event-apply-control-modifier] & to enter Ctrl-&."
  (vector (event-apply-modifier (read-event) 'control 26 "C-")))
(defun event-apply-meta-modifier (_ignore-prompt)
  "\\<function-key-map>Add the Meta modifier to the following event.
For example, type \\[event-apply-meta-modifier] & to enter Meta-&."
  (vector (event-apply-modifier (read-event) 'meta 27 "M-")))
(defun event-apply-modifier (event symbol lshiftby prefix)
  "Apply a modifier flag to event EVENT.
SYMBOL is the name of this modifier, as a symbol.
LSHIFTBY is the numeric value of this modifier, in keyboard events.
PREFIX is the string that represents this modifier in an event type symbol."
  (if (numberp event)
      ;; Use the base event to determine how the control and shift
      ;; modifiers should be applied.
      (let* ((base-event (event-basic-type event)))
        (cond ((eq symbol 'control)
	       (if (<= 64 (upcase base-event) 95)
                   ;; Apply the control modifier...
		   (logior (- (upcase base-event) 64)
                           ;; ... and any additional modifiers
                           ;; specified in the original event...
                           (logand event (logior ?\M-\0 ?\C-\0 ?\S-\0
					         ?\H-\0 ?\s-\0 ?\A-\0))
                           ;; ... including any shift modifier that
                           ;; `event-basic-type' may have removed.
                           (if (<= ?A event ?Z) ?\S-\0 0))
	         (logior (ash 1 lshiftby) event)))
	      ((eq symbol 'shift)
               ;; FIXME: Should we also apply this "upcase" behavior of shift
               ;; to non-ascii letters?
	       (if (<= ?a base-event ?z)
                   ;; Apply the Shift modifier.
		   (logior (upcase base-event)
                           ;; ... and any additional modifiers
                           ;; specified in the original event.
                           (logand event (logior ?\M-\0 ?\C-\0 ?\S-\0
					         ?\H-\0 ?\s-\0 ?\A-\0)))
	         (logior (ash 1 lshiftby) event)))
	      (t
	       (logior (ash 1 lshiftby) event))))
    (if (memq symbol (event-modifiers event))
	event
      (let ((event-type (if (symbolp event) event (car event))))
	(setq event-type (intern (concat prefix (symbol-name event-type))))
	(if (symbolp event)
	    event-type
	  (cons event-type (cdr event)))))))
;; This is what makes "C-x @" followed by [hsmaSc] work even though
;; you won't find any (define-key ctl-x-map "@" ...) binding.
(define-key function-key-map [?\C-x ?@ ?h] 'event-apply-hyper-modifier)
(define-key function-key-map [?\C-x ?@ ?s] 'event-apply-super-modifier)
(define-key function-key-map [?\C-x ?@ ?m] 'event-apply-meta-modifier)
(define-key function-key-map [?\C-x ?@ ?a] 'event-apply-alt-modifier)
(define-key function-key-map [?\C-x ?@ ?S] 'event-apply-shift-modifier)
(define-key function-key-map [?\C-x ?@ ?c] 'event-apply-control-modifier)

(defun compose-mail-other-frame (&optional to subject other-headers continue
					    yank-action send-actions
					    return-action)
  "Like \\[compose-mail], but edit the outgoing message in another frame."
  (interactive (list nil nil nil current-prefix-arg))
  (compose-mail to subject other-headers continue
		'switch-to-buffer-other-frame yank-action send-actions
		return-action))

(defvar set-variable-value-history nil
  "History of values entered with `set-variable'.

Maximum length of the history list is determined by the value
of `history-length', which see.")

(defun set-variable (variable value &optional make-local)
  "Set VARIABLE to VALUE.  VALUE is a Lisp object.
VARIABLE should be a user option variable name, a Lisp variable
meant to be customized by users.  You should enter VALUE in Lisp syntax,
so if you want VALUE to be a string, you must surround it with doublequotes.
VALUE is used literally, not evaluated.

If VARIABLE has a `variable-interactive' property, that is used as if
it were the arg to `interactive' (which see) to interactively read VALUE.

If VARIABLE has been defined with `defcustom', then the type information
in the definition is used to check that VALUE is valid.

Note that this function is at heart equivalent to the basic `set' function.
For a variable defined with `defcustom', it does not pay attention to
any :set property that the variable might have (if you want that, use
\\[customize-set-variable] instead).

With a prefix argument, set VARIABLE to VALUE buffer-locally.

When called interactively, the user is prompted for VARIABLE and
then VALUE.  The current value of VARIABLE will be put in the
minibuffer history so that it can be accessed with \\`M-n', which
makes it easier to edit it."
  (interactive
   (let* ((default-var (variable-at-point))
          (var (if (custom-variable-p default-var)
		   (read-variable (format-prompt "Set variable" default-var)
				  default-var)
		 (read-variable "Set variable: ")))
	  (minibuffer-help-form `(describe-variable ',var))
	  (prop (get var 'variable-interactive))
          (obsolete (car (get var 'byte-obsolete-variable)))
	  (prompt (format "Set %s %s to value: " var
			  (cond ((local-variable-p var)
				 "(buffer-local)")
				((or current-prefix-arg
				     (local-variable-if-set-p var))
				 "buffer-locally")
				(t "globally"))))
	  (val (progn
                 (when obsolete
                   (message (concat "`%S' is obsolete; "
                                    (if (symbolp obsolete) "use `%S' instead" "%s"))
                            var obsolete)
                   (sit-for 3))
                 (if prop
                     ;; Use VAR's `variable-interactive' property
                     ;; as an interactive spec for prompting.
                     (call-interactively `(lambda (arg)
                                            (interactive ,prop)
                                            arg))
                   (read-from-minibuffer prompt nil
                                         read-expression-map t
                                         'set-variable-value-history
                                         (format "%S" (symbol-value var)))))))
     (list var val current-prefix-arg)))

  (and (custom-variable-p variable)
       (not (get variable 'custom-type))
       (custom-load-symbol variable))
  (let ((type (get variable 'custom-type)))
    (when type
      ;; Match with custom type.
      (when (fboundp 'widget-convert)
        (require 'cus-edit)
        (setq type (widget-convert type))
        (unless (widget-apply type :match value)
	  (user-error "Value `%S' does not match type %S of %S"
		      value (car type) variable)))))

  (if make-local
      (make-local-variable variable))

  (set variable value)

  ;; Force a thorough redisplay for the case that the variable
  ;; has an effect on the display, like `tab-width' has.
  (force-mode-line-update))

(defcustom completions-format 'horizontal
  "Define the appearance and sorting of completions.
If the value is `vertical', display completions sorted vertically
in columns in the *Completions* buffer.
If the value is `horizontal', display completions sorted in columns
horizontally in alphabetical order, rather than down the screen.
If the value is `one-column', display completions down the screen
in one column."
  :type '(choice (const horizontal) (const vertical) (const one-column))
  :version "23.2")

(define-minor-mode visible-mode
  "Toggle making all invisible text temporarily visible (Visible mode).

This mode works by saving the value of `buffer-invisibility-spec'
and setting it to nil."
  :lighter " Vis"
  :group 'editing-basics
  (when (local-variable-p 'vis-mode-saved-buffer-invisibility-spec)
    (setq buffer-invisibility-spec vis-mode-saved-buffer-invisibility-spec)
    (kill-local-variable 'vis-mode-saved-buffer-invisibility-spec))
  (when visible-mode
    (setq-local vis-mode-saved-buffer-invisibility-spec
                buffer-invisibility-spec)
    (setq buffer-invisibility-spec nil)))

(define-derived-mode messages-buffer-mode special-mode "Messages"
  "Major mode used in the \"*Messages*\" buffer."
  ;; Make it easy to do like "tail -f".
  (setq-local window-point-insertion-type t))

(defun messages-buffer ()
  "Return the \"*Messages*\" buffer.
If it does not exist, create it and switch it to `messages-buffer-mode'."
  (or (get-buffer "*Messages*")
      (with-current-buffer (get-buffer-create "*Messages*")
        (messages-buffer-mode)
        (current-buffer))))

(defun scratch-buffer ()
  "Switch to the *scratch* buffer.
If the buffer doesn't exist, create it first."
  (interactive)
  (pop-to-buffer-same-window (get-scratch-buffer-create)))

;;; simple.el/help.el/newcomment.el members (verbatim ports from GNU Emacs 31.1).

(defalias 'set-comment-column 'comment-set-column)

(defvar extended-command-history nil)
(defvar execute-extended-command--last-typed nil)

(defvar-keymap read--expression-map
  :doc "Keymap used by `read--expression'."
  :parent read-expression-map
  "RET" #'read--expression-try-read
  "C-j" #'read--expression-try-read)

(defun read--expression-try-read ()
  "Try to read an Emacs Lisp expression in the minibuffer.

Exit the minibuffer if successful, else report the error to the
user and move point to the location of the error.  If point is
not already at the location of the error, push a mark before
moving point."
  (interactive)
  (unless (> (minibuffer-depth) 0)
    (error "Minibuffer must be active"))
  (if (let* ((contents (minibuffer-contents))
             (error-point nil))
        (with-temp-buffer
          (condition-case err
              (progn
                (insert contents)
                (goto-char (point-min))
                ;; `read' will signal errors like "End of file during
                ;; parsing" and "Invalid read syntax".
                (read (current-buffer))
                ;; Since `read' does not signal the "Trailing garbage
                ;; following expression" error, we check for trailing
                ;; garbage ourselves.
                (or (progn
                      ;; This check is similar to what `string_to_object'
                      ;; does in minibuf.c.
                      (skip-chars-forward " \t\n")
                      (= (point) (point-max)))
                    (error "Trailing garbage following expression")))
            (error
             (setq error-point (+ (length (minibuffer-prompt)) (point)))
             (with-current-buffer (window-buffer (minibuffer-window))
               (unless (= (point) error-point)
                 (push-mark))
               (goto-char error-point)
               (minibuffer-message (error-message-string err)))
             nil))))
      (exit-minibuffer)))

(defun eval-expression-get-print-arguments (prefix-argument)
  "Get arguments for commands that print an expression result.
Returns a list (INSERT-VALUE NO-TRUNCATE CHAR-PRINT-LIMIT) based
on PREFIX-ARGUMENT.  This function determines the interpretation
of the prefix argument for `eval-expression' and
`eval-last-sexp'."
  (let ((num (prefix-numeric-value prefix-argument)))
    (list (not (memq prefix-argument '(- nil)))
          (= num 0)
          (cond ((not (memq prefix-argument '(0 -1 - nil))) nil)
                ((= num -1) most-positive-fixnum)
                (t eval-expression-print-maximum-character)))))

(defun repeat-complex-command (arg)
  "Edit and re-evaluate last complex command, or ARGth from last.
A complex command is one that used the minibuffer.
The command is placed in the minibuffer as a Lisp form for editing.
The result is executed, repeating the command as changed.
If the command has been changed or is not the most recent previous
command it is added to the front of the command history.
You can use the minibuffer history commands \
\\<minibuffer-local-map>\\[next-history-element] and \\[previous-history-element]
to get different commands to edit and resubmit."
  (interactive "p")
  (let ((elt (nth (1- arg) command-history))
	newcmd)
    (if elt
	(progn
	  (setq newcmd
		(let ((print-level nil)
		      (minibuffer-history-position arg)
		      (minibuffer-history-sexp-flag (1+ (minibuffer-depth))))
		  (unwind-protect
		      (read-from-minibuffer
		       "Redo: " (prin1-to-string elt) read-expression-map t
		       (cons 'command-history arg))

		    ;; If command was added to command-history as a
		    ;; string, get rid of that.  We want only
		    ;; evaluable expressions there.
                    (when (stringp (car command-history))
                      (pop command-history)))))

          (add-to-history 'command-history newcmd)
          (apply #'funcall-interactively
		 (car newcmd)
		 (mapcar (lambda (e) (eval e t)) (cdr newcmd))))
      (if command-history
	  (error "Argument %d is beyond length of command history" arg)
	(error "There are no previous complex commands to repeat")))))

(defun help--key-description-fontified (keys &optional prefix)
  "Like `key-description' but add face for \"*Help*\" buffers.
KEYS is the return value of `(where-is-internal \\='foo-cmd nil t)'.
Return nil if KEYS is nil."
  (when keys
    ;; We add both the `font-lock-face' and `face' properties here, as this
    ;; seems to be the only way to get this to work reliably in any
    ;; buffer.
    (propertize (key-description keys prefix)
                'font-lock-face 'help-key-binding
                'face 'help-key-binding)))

(defun where-is (definition &optional insert)
  "Print message listing key sequences that invoke the command DEFINITION.
Argument is a command definition, usually a symbol with a function definition.
If INSERT (the prefix arg) is non-nil, insert the message in the buffer."
  (interactive
   (let ((fn (function-called-at-point))
	 (enable-recursive-minibuffers t)
	 val)
     (setq val (completing-read (format-prompt "Where is command" fn)
		                obarray #'commandp t nil nil
		                (and fn (symbol-name fn))))
     (list (unless (equal val "") (intern val))
	   current-prefix-arg)))
  (unless definition (error "No command"))
  (let ((func (indirect-function definition))
        (defs nil)
        (standard-output (if insert (current-buffer) standard-output)))
    ;; In DEFS, find all symbols that are aliases for DEFINITION.
    (mapatoms (lambda (symbol)
		(and (fboundp symbol)
		     (not (eq symbol definition))
		     (eq func (condition-case ()
				  (indirect-function symbol)
				(error symbol)))
		     (push symbol defs))))
    ;; Look at all the symbols--first DEFINITION,
    ;; then its aliases.
    (dolist (symbol (cons definition defs))
      (let* ((remapped (command-remapping symbol))
	     (keys (where-is-internal
		    symbol overriding-local-map nil nil remapped))
             (keys (mapconcat #'help--key-description-fontified
                              keys ", "))
	     string)
	(setq string
	      (if insert
		  (if (> (length keys) 0)
		      (if remapped
			  (format "%s, remapped to %s (%s)"
                                  symbol remapped keys)
			(format "%s (%s)" symbol keys))
		    (format "M-x %s RET" symbol))
		(if (> (length keys) 0)
		    (if remapped
			(if (eq symbol (symbol-function definition))
			    (format
                             "%s, which is remapped to %s, which is on %s"
			     symbol remapped keys)
			  (format "%s is remapped to %s, which is on %s"
				  symbol remapped keys))
		      (if (eq symbol (symbol-function definition))
			  (format "%s, which is on %s" symbol keys)
			(format "%s is on %s" symbol keys)))
		  ;; If this is the command the user asked about,
		  ;; and it is not on any key, say so.
		  ;; For other symbols, its aliases, say nothing
		  ;; about them unless they are on keys.
		  (if (eq symbol definition)
		      (format "%s is not on any key" symbol)))))
	(when string
	  (unless (eq symbol definition)
	    (if (eq definition (symbol-function symbol))
		(princ ";\n its alias ")
	      (princ ";\n it's an alias for ")))
	  (princ string)))))
  nil)

;;; simple.el/subr.el members batch C (verbatim ports from GNU Emacs 31.1).

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
	 end)

    ;; FIXME: This throws away any yank-undo-function set by previous calls
    ;; to insert-for-yank-1 within the loop of insert-for-yank!
    (setq yank-undo-function t)
    (if (nth 0 handler) ; FUNCTION
	(funcall (car handler) param)
      (insert param))
    (setq end (point))

    (with-silent-modifications
      (unless (nth 2 handler)           ; NOEXCLUDE
        (remove-yank-excluded-properties opoint end))

      ;; If last inserted char has properties, mark them as rear-nonsticky.
      (if (and (> end opoint)
	       (text-properties-at (1- end)))
	  (put-text-property (1- end) end 'rear-nonsticky t)))

    (if (eq yank-undo-function t)		   ; not set by FUNCTION
	(setq yank-undo-function (nth 3 handler))) ; UNDO
    (if (nth 4 handler)				   ; COMMAND
	(setq this-command (nth 4 handler)))))

(defun delete-trailing-whitespace-if-possible ()
  "Call `delete-trailing-whitespace' unless the buffer is read-only."
  (unless buffer-read-only (delete-trailing-whitespace)))

(defvar cycle-spacing--context nil
  "Stored context used in consecutive calls to `cycle-spacing' command.
The value is a property list with the following elements:
- `:orig-pos'    The original position of point when starting the
                 sequence.
- `:whitespace-string' All whitespace characters around point
                       including newlines.
- `:n'            The prefix arg given to the initial invocation
                  which is reused for all actions in this cycle.
- `:last-action'  The last action performed in the cycle.")

(defcustom cycle-spacing-actions
  '( just-one-space
     delete-all-space
     restore)
  "List of actions cycled through by `cycle-spacing'.
Supported values are:
- `just-one-space'      Delete all but N (prefix arg) spaces.
                        See that command's docstring for details.
- `delete-space-after'  Delete spaces after point keeping only N.
- `delete-space-before' Delete spaces before point keeping only N.
- `delete-all-space'    Delete all spaces around point.
- `restore'             Restore the original spacing.

All actions make use of the prefix arg given to `cycle-spacing'
in the initial invocation, i.e., `just-one-space' keeps this
amount of spaces deleting surplus ones.  `just-one-space' and all
other actions have the contract that a positive prefix arg (or
zero) only deletes tabs and spaces whereas a negative prefix arg
also deletes newlines.

The `delete-space-before' and `delete-space-after' actions handle
the prefix arg \\[negative-argument] without a number provided
specially: all spaces before/after point are deleted (as if N was
0) including newlines (as if N was negative).

In addition to the predefined actions listed above, any function
which accepts one argument is allowed.  It receives the raw
prefix arg of this cycle.

In addition, an action may take the form (ACTION ARG) where
ACTION is one of the predefined actions (except for `restore')
and ARG is either
- an integer with the meaning that ACTION should always use this
  fixed integer instead of the actual prefix arg or
- the symbol `inverted-arg' with the meaning that ACTION should
  be performed with the inverted actual prefix arg.
- the symbol `-' with the meaning that ACTION should include
  newlines but it's up to the ACTION to decide how to interpret
  it as a number, e.g., `delete-space-before' and
  `delete-space-after' treat it like 0 whereas `just-one-space'
  treats it like -1 as is usual."
  :group 'editing-basics
  :type (let ((actions
               '((const :tag "Just N (prefix arg) spaces" just-one-space)
                 (const :tag "Delete spaces after point" delete-space-after)
                 (const :tag "Delete spaces before point" delete-space-before)
                 (const :tag "Delete all spaces around point" delete-all-space)
                 (function :tag "Function receiving a numeric arg"))))
          `(repeat
            (choice
             ,@actions
             (list :tag "Action with modified arg"
                   (choice ,@actions)
                   (choice (const :tag "Inverted prefix arg" inverted-arg)
                           (integer :tag "Fixed numeric arg")
                           (const :tag "Negative arg" -)))
             (const :tag "Restore the original spacing" restore))))
  :version "29.1")

(defun cycle-spacing (&optional n)
  "Manipulate whitespace around point in a smart way.
Repeated calls perform the actions in `cycle-spacing-actions' one
after the other, wrapping around after the last one.

All actions are amendable using a prefix arg N.  In general, a
zero or positive prefix arg allows only for deletion of tabs and
spaces whereas a negative prefix arg also allows for deleting
newlines.

The prefix arg given at the first invocation starting a cycle is
provided to all following actions, i.e.,
    \\[negative-argument] \\[cycle-spacing] \\[cycle-spacing] \\[cycle-spacing]
is equivalent to
    \\[negative-argument] \\[cycle-spacing] \\[negative-argument] \\[cycle-spacing] \\[negative-argument] \\[cycle-spacing].

A new sequence can be started by providing a different prefix arg
than provided at the initial invocation (except for 1), or by
doing any other command before the next \\[cycle-spacing]."
  (interactive "*P")
  ;; Initialize `cycle-spacing--context' if needed.
  (when (or (not (equal last-command this-command))
            (not cycle-spacing--context)
            ;; With M-5 M-SPC M-SPC... we pass the prefix arg 5 to
            ;; each action and only start a new cycle when a different
            ;; prefix arg is given and which is not the default value
            ;; 1.
            (and n (not (equal (plist-get cycle-spacing--context :n)
                               n))))
    (let ((orig-pos (point))
          (skip-characters " \t\n\r"))
      (save-excursion
        (skip-chars-backward skip-characters)
        (constrain-to-field nil orig-pos)
        (let ((start (point))
              (end   (progn
                       (skip-chars-forward skip-characters)
                       (constrain-to-field nil orig-pos t))))
          (setq cycle-spacing--context  ;; Save for later.
                (list :orig-pos orig-pos
                      :whitespace-string (buffer-substring start end)
                      :n n
                      :last-action nil))))))

  ;; Cycle through the actions in `cycle-spacing-actions'.
  (when cycle-spacing--context
    (cl-labels ((next-action ()
                  (let* ((l cycle-spacing-actions)
                         (elt (plist-get cycle-spacing--context
                                         :last-action)))
                    (if (null elt)
                        (car cycle-spacing-actions)
                      (catch 'found
                        (while l
                          (cond
                           ((null (cdr l))
                            (throw 'found
                                   (when (eq elt (car l))
                                     (car cycle-spacing-actions))))
                           ((and (eq elt (car l))
                                 (cdr l))
                            (throw 'found (cadr l)))
                           (t (setq l (cdr l)))))))))
                (skip-chars (chars max-dist direction)
                  (if (eq direction 'forward)
                      (skip-chars-forward
                       chars
                       (and max-dist (+ (point) max-dist)))
                    (skip-chars-backward
                     chars
                     (and max-dist (- (point) max-dist)))))
                (delete-space (n include-newlines direction)
                  (let ((orig-point (point))
                        (chars (if include-newlines
                                   " \t\r\n"
                                 " \t")))
                    (when (or (zerop n)
                              (= n (abs (skip-chars chars n direction))))
                      (let ((start (point))
                            (end (progn
                                   (skip-chars chars nil direction)
                                   (point))))
                        (unless (= start end)
                          (delete-region start end))
                        (goto-char (if (eq direction 'forward)
                                       orig-point
                                     (+ n end)))))))
                (restore ()
                  (delete-all-space)
                  (insert (plist-get cycle-spacing--context
                                     :whitespace-string))
                  (goto-char (plist-get cycle-spacing--context
                                        :orig-pos))))
      (let ((action (next-action)))
        (atomic-change-group
          (restore)
          (unless (eq action 'restore)
            ;; action can be some-action or (some-action <arg>) where
            ;; arg is either an integer, the arg to be always used for
            ;; this action or - to use the inverted context n for this
            ;; action.
            (let* ((actual-action (if (listp action)
                                      (car action)
                                    action))
                   (arg (when (listp action)
                          (nth 1 action)))
                   (context-n (plist-get cycle-spacing--context :n))
                   (actual-n (cond
                              ((integerp arg) arg)
                              ((eq 'inverted-arg arg)
                               (* -1 (prefix-numeric-value context-n)))
                              ((eq '- arg) '-)
                              (t context-n)))
                   (numeric-n (prefix-numeric-value actual-n))
                   (include-newlines (or (eq actual-n '-)
                                         (and (integerp actual-n)
                                              (< actual-n 0)))))
              (cond
               ((eq actual-action 'just-one-space)
                (just-one-space numeric-n))
               ((eq actual-action 'delete-space-after)
                (delete-space (if (eq actual-n '-) 0 (abs numeric-n))
                              include-newlines 'forward))
               ((eq actual-action 'delete-space-before)
                (delete-space (if (eq actual-n '-) 0 (abs numeric-n))
                              include-newlines 'backward))
               ((eq actual-action 'delete-all-space)
                (if include-newlines
                    (delete-all-space)
                  (delete-horizontal-space)))
               ((functionp actual-action)
                (funcall actual-action actual-n))
               (t
                (error "Don't know how to handle action %S" action)))))
          (setf (plist-get cycle-spacing--context :last-action)
                action))))))

(defun edit-and-eval-command (prompt command)
  "Prompting with PROMPT, let user edit COMMAND and eval result.
COMMAND is a Lisp expression.  Let user edit that expression in
the minibuffer, then read and evaluate the result."
  (let ((command
	 (let ((print-level nil)
	       (minibuffer-history-sexp-flag (1+ (minibuffer-depth))))
	   (unwind-protect
	       (read-from-minibuffer prompt
				     (prin1-to-string command)
				     read-expression-map t
				     'command-history)
	     ;; If command was added to command-history as a string,
	     ;; get rid of that.  We want only evaluable expressions there.
             (when (stringp (car command-history))
               (pop command-history))))))

    (add-to-history 'command-history command)
    (eval command)))

(defvar bidi-directional-controls-chars "\x202a-\x202e\x2066-\x2069"
  "Character set that matches bidirectional formatting control characters.")

(defvar bidi-directional-non-controls-chars "^\x202a-\x202e\x2066-\x2069"
  "Character set that matches any character except bidirectional controls.")

(defun bidi--char-in-category-p (char category)
  "Whether CHAR belongs to the bidi category denoted by CATEGORY.
CATEGORY is a regexp like \"\\\\CR+\" or \"\\\\CL+\" as used by
`squeeze-bidi-context'.  remacs regexps do not support the \\c
category syntax, so the membership test is done via the char's
`bidi-class' property instead."
  (let ((bc (get-char-code-property char 'bidi-class)))
    (if (string-equal category "\\CR+")
        ;; GNU category R: strong RTL and RTL embedding/override controls.
        (memq bc '(R AL RLE RLO))
      ;; GNU category L: strong LTR and LTR embedding/override controls.
      (memq bc '(L LRE LRO)))))

(defun squeeze-bidi-context-1 (from to category replacement)
  "A subroutine of `squeeze-bidi-context'.
FROM and TO should be markers, CATEGORY and REPLACEMENT should be strings."
  (let ((pt (copy-marker from))
	(limit (copy-marker to))
	(old-pt 0)
	lim1)
    (setq lim1 limit)
    (goto-char pt)
    (while (< pt limit)
      (if (> pt old-pt)
	  (move-marker lim1
		       (save-excursion
			 ;; L and R categories include embedding and
			 ;; override controls, but we don't want to
			 ;; replace them, because that might change
			 ;; the visual order.  Likewise with PDF and
			 ;; isolate controls.
			 (+ pt (skip-chars-forward
				bidi-directional-non-controls-chars
				limit)))))
      ;; Replace any run of characters not in CATEGORY by a single mark.
      ;; (remacs regexps lack the \c category syntax; scan manually.)
      (let ((scan pt)
	    run-start)
	(while (and (< scan lim1)
		    (bidi--char-in-category-p (char-after scan) category))
	  (setq scan (1+ scan)))
	(if (>= scan lim1)
	    ;; No more characters outside CATEGORY, we are done.
	    (setq pt limit)
	  (setq run-start scan)
	  (while (and (< scan lim1)
		      (not (bidi--char-in-category-p (char-after scan)
						     category)))
	    (setq scan (1+ scan)))
	  (delete-region run-start scan)
	  (goto-char run-start)
	  (insert replacement)
	  (move-marker pt (point))))
      (setq old-pt pt)
      ;; Skip directional controls, if any.
      (move-marker
       pt (+ pt (skip-chars-forward bidi-directional-controls-chars limit))))))

(defun squeeze-bidi-context (from to)
  "Replace characters between FROM and TO while keeping bidi context.

This function replaces the region of text with as few characters
as possible, while preserving the effect that region will have on
bidirectional display before and after the region."
  (let ((start (set-marker (make-marker)
			   (if (> from 0) from (+ (point-max) from))))
	(end (set-marker (make-marker) to))
	;; This is for when they copy text with read-only text
	;; properties.
	(inhibit-read-only t))
    (if (null (marker-position end))
	(setq end (point-max-marker)))
    ;; Replace each run of non-RTL characters with a single LRM.
    (squeeze-bidi-context-1 start end "\\CR+" "\x200e")
    ;; Replace each run of non-LTR characters with a single RLM.  Note
    ;; that the \cR category includes both the Arabic Letter (AL) and
    ;; R characters; here we ignore the distinction between them,
    ;; because that distinction affects only Arabic Number (AN)
    ;; characters, which are weak and don't affect the reordering.
    (squeeze-bidi-context-1 start end "\\CL+" "\x200f")))

(defun line-substring-with-bidi-context (start end &optional no-properties)
  "Return buffer text between START and END with its bidi context.

START and END are assumed to belong to the same physical line
of buffer text.  This function prepends and appends to the text
between START and END bidi control characters that preserve the
visual order of that text when it is inserted at some other place."
  (if (or (< start (point-min))
	  (> end (point-max)))
      (signal 'args-out-of-range (list (current-buffer) start end)))
  (let ((buf (current-buffer))
	substr para-dir from to)
    (save-excursion
      (goto-char start)
      (setq para-dir (current-bidi-paragraph-direction))
      (setq from (line-beginning-position)
	    to (line-end-position))
      (goto-char from)
      ;; If we don't have any mixed directional characters in the
      ;; entire line, we can just copy the substring without adding
      ;; any context.
      (if (or (looking-at-p "\\CR*$")
	      (looking-at-p "\\CL*$"))
	  (setq substr (if no-properties
			   (buffer-substring-no-properties start end)
			 (buffer-substring start end)))
	(setq substr
	      (with-temp-buffer
		(if no-properties
		    (insert-buffer-substring-no-properties buf from to)
		  (insert-buffer-substring buf from to))
		(squeeze-bidi-context 1 (1+ (- start from)))
		(squeeze-bidi-context (- end to) nil)
		(buffer-substring 1 (point-max)))))

      ;; Wrap the string in LRI/RLI..PDI pair to achieve 2 effects:
      ;; (1) force the string to have the same base embedding
      ;; direction as the paragraph direction at the source, no matter
      ;; what is the paragraph direction at destination; and (2) avoid
      ;; affecting the visual order of the surrounding text at
      ;; destination if there are characters of different
      ;; directionality there.
      (concat (if (eq para-dir 'left-to-right) "\x2066" "\x2067")
	      substr "\x2069"))))

(defun yank-in-context (&optional arg)
  "Insert the last stretch of killed text while preserving syntax.
In particular, if point is inside a string, any quote characters
in the killed text will be quoted, so that the string remains a
valid string.

If point is inside a comment, ensure that the inserted text is
also marked as a comment.

This command otherwise behaves as `yank'.  See that command for
explanation of ARG.

This function uses the `escaped-string-quote' buffer-local
variable to determine how strings should be escaped."
  (interactive "*P")
  (let ((yank-transform-functions (cons #'yank-in-context--transform
                                        yank-transform-functions)))
    (yank arg)))

(defun yank-in-context--transform (string)
  (let ((ppss (syntax-ppss)))
    (cond
     ;; We're in a string.
     ((ppss-string-terminator ppss)
      (string-replace
       (string (ppss-string-terminator ppss))
       (concat (if (functionp escaped-string-quote)
                   (funcall escaped-string-quote
                            (ppss-string-terminator ppss))
                 escaped-string-quote)
               (string (ppss-string-terminator ppss)))
       string))
     ;; We're in a comment.
     ((or (ppss-comment-depth ppss)
          (and (bolp)
               (not (eobp))
               ;; If we're in the middle of a bunch of commented text,
               ;; we probably want to be commented.  This is quite DWIM.
               (or (bobp)
                   (save-excursion
                     (forward-line -1)
                     (forward-char 1)
                     (ppss-comment-depth (syntax-ppss))))
               (ppss-comment-depth
                (setq ppss (save-excursion
                             (forward-char 1)
                             (syntax-ppss))))))
      (cond
       ((and (eq (ppss-comment-depth ppss) t)
             (> (length comment-end) 0)
             (string-search comment-end string))
        (user-error "Can't insert a string containing a comment terminator in a comment"))
       ;; If this is a comment syntax that has an explicit end, then
       ;; we can just insert as is.
       ((> (length comment-end) 0) string)
       ;; Line-based comment formats.
       ((or (string-search "\n" string)
            (bolp))
        (let ((mode major-mode)
              (bolp (bolp))
              (eolp (eolp))
              (comment-style 'plain))
          (with-temp-buffer
            (funcall mode)
            (insert string)
            (when (string-match-p "\n\\'" string)
              (cond
               ((not eolp) (delete-char -1))
               (bolp (insert "\n"))))
            (comment-normalize-vars)
            (comment-region-default-1
             (if bolp
                 (point-min)
               (save-excursion
                 (goto-char (point-min))
                 (forward-line 1)
                 (point)))
             (point-max))
            (buffer-string))))
       (t string)))
     (t string))))

(defun read-from-kill-ring (prompt)
  "Read a `kill-ring' entry using completion and minibuffer history.
PROMPT is a string to prompt with.
Return the entry as a string."
  ;; `current-kill' updates `kill-ring' with a possible interprogram-paste
  (current-kill 0)
  (let* ((history-add-new-input nil)
         (history-pos (when yank-from-kill-ring-rotate
                        (- (length kill-ring)
                           (length kill-ring-yank-pointer))))
         (ellipsis (if (char-displayable-p ?…) "…" "..."))
         ;; Remove keymaps from text properties of copied string,
         ;; because typing RET in the minibuffer might call
         ;; an irrelevant command from the map of copied string.
         (read-from-kill-ring-history
          (mapcar (lambda (s)
                    (remove-list-of-text-properties
                     0 (length s)
                     '(
                       keymap local-map action mouse-action
                       read-only button category help-args)
                     s)
                    s)
                  kill-ring))
         (completions
          (mapcar (lambda (s)
                    (let* ((s (query-replace-descr s))
                           (b 0)
                           (limit (frame-text-cols)))
                      ;; Add ellipsis on leading whitespace
                      (when (string-match "\\`[[:space:]]+" s)
                        (setq b (match-end 0))
                        (add-text-properties 0 b `(display ,ellipsis) s))
                      ;; Add ellipsis at the end of a long string
                      (when (> (length s) (+ limit b))
                        (add-text-properties
                         (min (+ limit b) (length s)) (length s)
                         `(display ,ellipsis) s))
                      s))
                  read-from-kill-ring-history)))
    (minibuffer-with-setup-hook
        (lambda ()
          ;; Allow ‘SPC’ to be self-inserting
          (use-local-map
           (let ((map (make-sparse-keymap)))
             (set-keymap-parent map (current-local-map))
             (define-key map " " nil)
             (define-key map "?" nil)
             map)))
      (completing-read
       prompt
       ;; Keep sorted by recency
       (completion-table-with-metadata
        completions '((display-sort-function . identity)))
       nil nil nil
       (if history-pos
           (cons 'read-from-kill-ring-history
                 (if (zerop history-pos) history-pos (1+ history-pos)))
         'read-from-kill-ring-history)))))

(defun yank-from-kill-ring (string &optional arg)
  "Select a stretch of previously killed text and insert (\"paste\") it.
This command allows you to select one of the stretches of text
killed or yanked by previous commands, which are recorded in
`kill-ring', and reinsert the chosen kill at point.

This command prompts for a previously-killed text in the minibuffer.
Use the minibuffer history and search commands, or the minibuffer
completion commands, to select a previously-killed text.  In
particular, typing \\<minibuffer-local-completion-map>\\[minibuffer-complete] at the prompt will pop up a buffer showing
all the previously-killed stretches of text from which you can
choose the one you want to reinsert.
Once you select the text you want to reinsert, type \\<minibuffer-local-map>\\[exit-minibuffer] to actually
insert it and exit the minibuffer.
You can also edit the selected text in the minibuffer before
inserting it.

With \\[universal-argument] as argument, this command puts point at
beginning of the inserted text and mark at the end, like `yank' does.

When called from Lisp, insert STRING like `insert-for-yank' does."
  (interactive (list (read-from-kill-ring "Yank from kill-ring: ")
                     current-prefix-arg))
  (setq yank-window-start (window-start))
  (push-mark)
  (insert-for-yank string)
  (when yank-from-kill-ring-rotate
    (let ((pos (seq-position kill-ring string)))
      (if pos
          (setq kill-ring-yank-pointer (nthcdr pos kill-ring))
        (kill-new string))))
  (if (consp arg)
      ;; Swap point and mark like in `yank' and `yank-pop'.
      (goto-char (prog1 (mark t)
                   (set-marker (mark-marker) (point) (current-buffer))))))

(defun handle-shift-selection ()
  "Activate/deactivate mark depending on invocation thru shift translation.
This function is called by `call-interactively' when a command
with a `^' character in its `interactive' spec is invoked, before
running the command itself.

If `shift-select-mode' is enabled and the command was invoked
through shift translation, set the mark and activate the region
temporarily, unless it was already set in this way.  See
`this-command-keys-shift-translated' for the meaning of shift
translation.

Otherwise, if the region has been activated temporarily,
deactivate it, and restore the variable `transient-mark-mode' to
its earlier value."
  (cond ((and (eq shift-select-mode 'permanent)
              this-command-keys-shift-translated)
         (unless mark-active
           (push-mark nil nil t)))
        ((and shift-select-mode
              this-command-keys-shift-translated)
         (unless (and mark-active
		      (eq (car-safe transient-mark-mode) 'only))
	   (setq-local transient-mark-mode
                       (cons 'only
                             (unless (eq transient-mark-mode 'lambda)
                               transient-mark-mode)))
           (push-mark nil nil t)))
        ((eq (car-safe transient-mark-mode) 'only)
         (setq transient-mark-mode (cdr transient-mark-mode))
         (if (eq transient-mark-mode (default-value 'transient-mark-mode))
             (kill-local-variable 'transient-mark-mode))
         (deactivate-mark))))

(defun default-line-height ()
  "Return the pixel height of current buffer's default-face text line.

The value includes `line-spacing', if any, defined for the buffer
or the frame.
This function uses the definition of the default face for the currently
selected frame."
  (let ((dfh (default-font-height))
	(lsp (if (display-graphic-p)
		 (total-line-spacing (or (if (local-variable-p 'line-spacing)
                                             line-spacing
		                           (default-value 'line-spacing))
		                         (frame-parameter nil 'line-spacing)
		                         0))
	       0)))
    (if (floatp lsp)
	(setq lsp (truncate (* (frame-char-height) lsp))))
    (+ dfh lsp)))

(defun bad-package-check (package)
  "Run a check using the element from `bad-packages-alist' matching PACKAGE."
  (declare (obsolete nil "29.1"))
  (condition-case nil
      (let* ((list (assoc package bad-packages-alist))
             (symbol (nth 1 list)))
        (and list
             (boundp symbol)
             (or (eq symbol t)
                 (and (stringp (setq symbol (symbol-value symbol)))
                      (string-match-p (nth 2 list) symbol)))
             (display-warning package (nth 3 list) :warning)))
    (error nil)))

(defmacro define-alternatives (command &rest customizations)
  "Define a new generic COMMAND which can have several implementations.

The argument `COMMAND' should be an unquoted symbol.

Running `\\[execute-extended-command] COMMAND RET' for \
the first time prompts for the
alternative implementation to use and records the selected alternative.
Thereafter, `\\[execute-extended-command] COMMAND RET' will \
automatically invoke the recorded selection.

Running `\\[universal-argument] \\[execute-extended-command] COMMAND RET' \
again prompts for an alternative
and overwrites the previous selection.

The macro creates a `defcustom' named `COMMAND-alternatives'.
CUSTOMIZATIONS, if non-nil, should be pairs of `defcustom'
keywords and values to add to the definition of that `defcustom';
typically, these keywords will be :group and :version with the
appropriate values.

To be useful, the value of `COMMAND-alternatives' should be an
alist describing the alternative implementations of COMMAND.
The elements of this alist should be of the form
  (ALTERNATIVE-NAME . FUNCTION)
where ALTERNATIVE-NAME is the name of the alternative to be shown
to the user as a selectable alternative, and FUNCTION is the
interactive function to call which implements that alternative.
The variable could be populated with associations describing the
alternatives either before or after invoking `define-alternatives';
if the variable is not defined when `define-alternatives' is invoked,
the macro will create it with a nil value, and your Lisp program
should then populate it."
  (declare (indent defun))
  (let* ((command-name (symbol-name command))
         (varalt-name (concat command-name "-alternatives"))
         (varalt-sym (intern varalt-name))
         (varimp-sym (intern (concat command-name "--implementation"))))
    `(progn

       (defcustom ,varalt-sym nil
         ,(format "Alist of alternative implementations for the `%s' command.

Each entry must be a pair (ALTNAME . ALTFUN), where:
ALTNAME - The name shown at user to describe the alternative implementation.
ALTFUN  - The function called to implement this alternative."
                  command-name)
         :type '(alist :key-type string :value-type function)
         ,@customizations)

       (put ',varalt-sym 'definition-name ',command)
       (defvar ,varimp-sym nil "Internal use only.")

       (defun ,command (&optional arg)
         ,(format "Run generic command `%s'.
If used for the first time, or with interactive ARG, ask the user which
implementation to use for `%s'.  The variable `%s'
contains the list of implementations currently supported for this command."
                  command-name command-name varalt-name)
         (interactive "P")
         (when (or arg (null ,varimp-sym))
           (let ((val (completing-read
		       ,(format-message
                         "Select implementation for command `%s': "
                         command-name)
		       ,varalt-sym nil t)))
             (unless (string-equal val "")
	       (when (null ,varimp-sym)
		 (message
		  "Use `C-u M-x %s RET' to select another implementation"
		  ,command-name)
		 (sit-for 3))
	       (customize-save-variable ',varimp-sym
					(cdr (assoc-string val ,varalt-sym))))))
         (if ,varimp-sym
             (call-interactively ,varimp-sym)
           (message "%s" ,(format-message
                           "No implementation selected for command `%s'"
                           command-name)))))))

(defun kill-buffer--possibly-save (buffer)
  "Ask the user to confirm killing of a modified BUFFER.

If the user confirms, optionally save BUFFER that is about to be
killed."
  (let ((response
         (cadr
          (read-multiple-choice
           (format "Buffer %s modified; kill anyway?"
                   (buffer-name))
           '((?y "yes" "kill buffer without saving")
             (?n "no" "exit without doing anything")
             (?s "save and then kill" "save the buffer and then kill it"))
           nil nil (and (not use-short-answers)
                        (not (use-dialog-box-p)))))))
    (if (equal response "no")
        nil
      (unless (equal response "yes")
        (with-current-buffer buffer
          (save-buffer)))
      t)))

(defun read-signal-name ()
  "Read a signal number or name.
Return the signal number, if the user entered a number, otherwise
the signal symbol."
  (let ((value
         (completing-read "Signal code or name: "
                          (signal-names)
                          nil
                          (lambda (value)
                            (or (string-match "\\`[0-9]+\\'" value)
                                (member value (signal-names)))))))
    (if (string-match "\\`[0-9]+\\'" value)
        (string-to-number value)
      (intern (concat "sig" (downcase value))))))

(defun use-dialog-box-p (&rest _args)
  "Non-nil if input events are invoked via mouse or pointer gestures.
remacs has no GUI dialog boxes, so this always returns nil."
  nil)

(defvar line-spacing nil
  "Additional space between lines of text, in pixels.")

;;; File-mode and size helpers (GNU files.el).

(defun file-modes-char-to-who (char)
  "Convert CHAR to a numeric bit-mask for extracting mode bits.
CHAR is in [ugoa] and represents the category of users (Owner, Group,
Others, or All) for whom to produce the mask.
The bit-mask that is returned extracts from mode bits the access rights
for the specified category of users."
  (cond ((eq char ?u) #o4700)
	((eq char ?g) #o2070)
	((eq char ?o) #o1007)
	((eq char ?a) #o7777)
        (t (error "%c: Bad `who' character" char))))

(defun file-modes-char-to-right (char &optional from)
  "Convert CHAR to a numeric value of mode bits.
CHAR is in [rwxXstugo] and represents symbolic access permissions.
If CHAR is in [Xugo], the value is taken from FROM (or 0 if omitted)."
  (or from (setq from 0))
  (cond ((eq char ?r) #o0444)
	((eq char ?w) #o0222)
	((eq char ?x) #o0111)
	((eq char ?s) #o6000)
	((eq char ?t) #o1000)
	;; Rights relative to the previous file modes.
	((eq char ?X) (if (= (logand from #o111) 0) 0 #o0111))
	((eq char ?u) (let ((uright (logand #o4700 from)))
		        ;; FIXME: These divisions/shifts seem to be right
                        ;; for the `7' part of the #o4700 mask, but not
                        ;; for the `4' part.  Same below for `g' and `o'.
		        (+ uright (/ uright #o10) (/ uright #o100))))
	((eq char ?g) (let ((gright (logand #o2070 from)))
		        (+ gright (/ gright #o10) (* gright #o10))))
	((eq char ?o) (let ((oright (logand #o1007 from)))
		        (+ oright (* oright #o10) (* oright #o100))))
        (t (error "%c: Bad right character" char))))

(defun file-modes-rights-to-number (rights who-mask &optional from)
  "Convert a symbolic mode string specification to an equivalent number.
RIGHTS is the symbolic mode spec, it should match \"([+=-][rwxXstugo]*)+\".
WHO-MASK is the bit-mask specifying the category of users to which to
apply the access permissions.  See `file-modes-char-to-who'.
FROM (or 0 if nil) gives the mode bits on which to base permissions if
RIGHTS request to add, remove, or set permissions based on existing ones,
as in \"og+rX-w\"."
  (let* ((num-rights (or from 0))
	 (list-rights (string-to-list rights))
	 (op (pop list-rights)))
    (while (memq op '(?+ ?- ?=))
      (let ((num-right 0)
	    char-right)
	(while (memq (setq char-right (pop list-rights))
		     '(?r ?w ?x ?X ?s ?t ?u ?g ?o))
	  (setq num-right
		(logior num-right
			(file-modes-char-to-right char-right num-rights))))
	(setq num-right (logand who-mask num-right)
	      num-rights
	      (cond ((= op ?+) (logior num-rights num-right))
		    ((= op ?-) (logand num-rights (lognot num-right)))
		    (t (logior (logand num-rights (lognot who-mask)) num-right)))
	      op char-right)))
    num-rights))

(defun file-modes-number-to-symbolic (mode &optional filetype)
  "Return a description of a file's MODE as a string of 10 letters and dashes.
The returned string is like the mode description produced by \"ls -l\".
For instance, if MODE is #o700, then it produces `-rwx------'.
Note that this is NOT the same as the \"chmod\" style symbolic description
accepted by `file-modes-symbolic-to-number'.
FILETYPE, if provided, should be a character denoting the type of file,
such as `?d' for a directory, or `?l' for a symbolic link, and will override
the leading `-' character."
  (string
   (or filetype
       (pcase (ash mode -12)
         ;; POSIX specifies that the file type is included in st_mode
         ;; and provides names for the file types but values only for
         ;; the permissions (e.g., S_IWOTH=2).

         ;; (#o017 ??) ;; #define S_IFMT  00170000
         (#o014 ?s)    ;; #define S_IFSOCK 0140000
         (#o012 ?l)    ;; #define S_IFLNK  0120000
         ;; (8  ??)    ;; #define S_IFREG  0100000
         (#o006  ?b)   ;; #define S_IFBLK  0060000
         (#o004  ?d)   ;; #define S_IFDIR  0040000
         (#o002  ?c)   ;; #define S_IFCHR  0020000
         (#o001  ?p)   ;; #define S_IFIFO  0010000
         (_ ?-)))
   (if (zerop (logand   256 mode)) ?- ?r)
   (if (zerop (logand   128 mode)) ?- ?w)
   (if (zerop (logand    64 mode))
       (if (zerop (logand  2048 mode)) ?- ?S)
     (if (zerop (logand  2048 mode)) ?x ?s))
   (if (zerop (logand    32 mode)) ?- ?r)
   (if (zerop (logand    16 mode)) ?- ?w)
   (if (zerop (logand     8 mode))
       (if (zerop (logand  1024 mode)) ?- ?S)
     (if (zerop (logand  1024 mode)) ?x ?s))
   (if (zerop (logand     4 mode)) ?- ?r)
   (if (zerop (logand     2 mode)) ?- ?w)
   (if (zerop (logand 512 mode))
       (if (zerop (logand   1 mode)) ?- ?x)
     (if (zerop (logand   1 mode)) ?T ?t))))

(defun file-size-human-readable (file-size &optional flavor space unit)
  "Produce a string showing FILE-SIZE in human-readable form.

Optional second argument FLAVOR controls the units and the display format:

 If FLAVOR is nil or omitted, each kilobyte is 1024 bytes and the produced
    suffixes are \"k\", \"M\", \"G\", \"T\", etc.
 If FLAVOR is `si', each kilobyte is 1000 bytes and the produced suffixes
    are \"k\", \"M\", \"G\", \"T\", etc.
 If FLAVOR is `iec', each kilobyte is 1024 bytes and the produced suffixes
    are \"KiB\", \"MiB\", \"GiB\", \"TiB\", etc.

Optional third argument SPACE is a string put between the number and unit.
It defaults to the empty string.  We recommend a single space or
non-breaking space, unless other constraints prohibit a space in that
position.

Optional fourth argument UNIT is the unit to use.  It defaults to \"B\"
when FLAVOR is `iec' and the empty string otherwise.  We recommend \"B\"
in all cases, since that is the standard symbol for byte."
  (let ((power (if (or (null flavor) (eq flavor 'iec))
		   1024.0
		 1000.0))
	(prefixes '("" "k" "M" "G" "T" "P" "E" "Z" "Y" "R" "Q")))
    (while (and (>= file-size power) (cdr prefixes))
      (setq file-size (/ file-size power)
	    prefixes (cdr prefixes)))
    (let* ((prefix (car prefixes))
           (prefixed-unit (if (eq flavor 'iec)
                              (concat
                               (if (string= prefix "k") "K" prefix)
                               (if (string= prefix "") "" "i")
                               (or unit "B"))
                            (concat prefix unit))))
      ;; Mimic what GNU "ls -lh" does:
      ;; If the formatted size will have just one digit before the decimal...
      (format (if (and (< file-size 10)
                       ;; ...and its fractional part is not too small...
                       (>= (mod file-size 1.0) 0.05)
                       (< (mod file-size 1.0) 0.95))
                  ;; ...then emit one digit after the decimal.
		  "%.1f%s%s"
	        "%.0f%s%s")
	      file-size
              (if (string= prefixed-unit "") "" (or space ""))
              prefixed-unit))))

(defun file-size-human-readable-iec (size)
  "Human-readable string for SIZE bytes, using IEC prefixes."
  (file-size-human-readable size 'iec " "))

;;; File-name and attribute helpers (GNU files.el).

(defun file-name-split (filename)
  "Return a list of all the components of FILENAME.
On most systems, this will be true:

  (equal (string-join (file-name-split filename) \"/\") filename)"
  (let ((components nil))
    ;; If this is a directory file name, then we have a null file name
    ;; at the end.
    (when (directory-name-p filename)
      (push "" components)
      (setq filename (directory-file-name filename)))
    ;; Loop, chopping off components.
    (while (length> filename 0)
      (push (file-name-nondirectory filename) components)
      (let ((dir (file-name-directory filename)))
        (setq filename (and dir (directory-file-name dir)))
        ;; If there's nothing left to peel off, we're at the root and
        ;; we can stop.
        (when (and dir (equal dir filename))
          (push (if (equal dir "") ""
                  ;; On Windows, the first component might be "c:" or
                  ;; the like.
                  (substring dir 0 -1))
                components)
          (setq filename nil))))
    components))

(defun file-nlinks (filename)
  "Return number of names file FILENAME has."
  (car (cdr (file-attributes filename))))

(defun auto-save-file-name-p (filename)
  "Return non-nil if FILENAME can be yielded by `make-auto-save-file-name'.
FILENAME should lack slashes.
See also `make-auto-save-file-name'."
  (string-match "\\`#.*#\\'" filename))

(defun directory-empty-p (dir)
  "Return t if DIR names an existing directory containing no other files.
Return nil if DIR does not name a directory, or if there was
trouble determining whether DIR is a directory or empty.

Symbolic links to directories count as directories.
See `file-symlink-p' to distinguish symlinks."
  (and (file-directory-p dir)
       (null (directory-files dir nil directory-files-no-dot-files-regexp t 1))))

(defvar tramp-mode t
  "Non-nil means handle file name handlers for remote files.")

(defun directory-files-recursively (dir regexp
                                        &optional include-directories predicate
                                        follow-symlinks)
  "Return list of all files under directory DIR whose names match REGEXP.
This function works recursively.  Files are returned in \"depth
first\" order, and files from each directory are sorted in
alphabetical order.  Each file name appears in the returned list
in its absolute form.

By default, the returned list excludes directories, but if
optional argument INCLUDE-DIRECTORIES is non-nil, they are
included.

PREDICATE can be either nil (which means that all subdirectories
of DIR are descended into), t (which means that subdirectories that
can't be read are ignored), or a function (which is called with
the name of each subdirectory, and should return non-nil if the
subdirectory is to be descended into).

If FOLLOW-SYMLINKS is non-nil, symbolic links that point to
directories are followed.  Note that this can lead to infinite
recursion."
  (let* ((result nil)
	 (files nil)
         (dir (directory-file-name dir))
	 ;; When DIR is "/", remote file names like "/method:" could
	 ;; also be offered.  We shall suppress them.
	 (tramp-mode (and tramp-mode (file-remote-p (expand-file-name dir)))))
    (dolist (file (sort (file-name-all-completions "" dir)
			'string<))
      (unless (member file '("./" "../"))
	(if (directory-name-p file)
	    (let* ((leaf (substring file 0 (1- (length file))))
		   (full-file (concat dir "/" leaf)))
	      ;; Don't follow symlinks to other directories.
	      (when (and (or (not (file-symlink-p full-file))
                             (and (file-symlink-p full-file)
                                  follow-symlinks))
                         ;; Allow filtering subdirectories.
                         (or (eq predicate nil)
                             (eq predicate t)
                             (funcall predicate full-file)))
                (let ((sub-files
                       (if (eq predicate t)
                           (ignore-error file-error
                             (directory-files-recursively
			      full-file regexp include-directories
                              predicate follow-symlinks))
                         (directory-files-recursively
			  full-file regexp include-directories
                          predicate follow-symlinks))))
		  (setq result (nconc result sub-files))))
	      (when (and include-directories
			 (string-match regexp leaf))
		(setq result (nconc result (list full-file)))))
	  (when (string-match regexp file)
	    (push (concat dir "/" file) files)))))
    (nconc result (nreverse files))))

(defcustom directory-abbrev-alist
  nil
  "Alist of abbreviations for file directories.
A list of elements of the form (FROM . TO), each meaning to replace
a match for FROM with TO when a directory name matches FROM.  This
replacement is done when setting up the default directory of a
newly visited file buffer.

FROM is a regexp that is matched against directory names anchored at
the first character, so it should start with a \"\\\\\\=`\", or, if
directory names cannot have embedded newlines, with a \"^\".

FROM and TO should be equivalent names, which refer to the
same directory.  TO should be an absolute directory name.
Do not use `~' in the TO strings.

Use this feature when you have directories that you normally refer to
via absolute symbolic links.  Make TO the name of the link, and FROM
a regexp matching the name it is linked to."
  :type '(repeat (cons :format "%v"
		       (regexp :tag "From")
		       (string :tag "To")))
  :group 'find-file)

(defun directory-abbrev-make-regexp (directory)
  "Create a regexp to match DIRECTORY for `directory-abbrev-alist'."
  (let ((regexp
         ;; We include a slash at the end, to avoid spurious
         ;; matches such as `/usr/foobar' when the home dir is
         ;; `/usr/foo'.
         (concat "\\`" (regexp-quote directory) "\\(/\\|\\'\\)")))
    ;; The value of regexp could be multibyte or unibyte.  In the
    ;; latter case, we need to decode it.
    (if (multibyte-string-p regexp)
        regexp
      (decode-coding-string regexp
                            (if (eq system-type 'windows-nt)
                                'utf-8
                              locale-coding-system)))))

(defun directory-abbrev-apply (filename)
  "Apply the abbreviations in `directory-abbrev-alist' to FILENAME.
Note that when calling this, you should set `case-fold-search' as
appropriate for the filesystem used for FILENAME."
  (dolist (dir-abbrev directory-abbrev-alist filename)
    (when (string-match (car dir-abbrev) filename)
         (setq filename (concat (cdr dir-abbrev)
                                (substring filename (match-end 0)))))))

;;; file-attribute accessors (GNU files.el).

(defsubst file-attribute-type (attributes)
  "The type field in ATTRIBUTES returned by `file-attributes'.
The value is either t for directory, string (name linked to) for
symbolic link, or nil."
  (nth 0 attributes))

(defsubst file-attribute-link-number (attributes)
  "Return the number of links in ATTRIBUTES returned by `file-attributes'."
  (nth 1 attributes))

(defsubst file-attribute-user-id (attributes)
  "The UID field in ATTRIBUTES returned by `file-attributes'.
This is either a string or a number.  If a string value cannot be
looked up, a numeric value, either an integer or a float, is
returned."
  (nth 2 attributes))

(defsubst file-attribute-group-id (attributes)
  "The GID field in ATTRIBUTES returned by `file-attributes'.
This is either a string or a number.  If a string value cannot be
looked up, a numeric value, either an integer or a float, is
returned."
  (nth 3 attributes))

(defsubst file-attribute-access-time (attributes)
  "The last access time in ATTRIBUTES returned by `file-attributes'.
This a Lisp timestamp in the style of `current-time'."
  (nth 4 attributes))

(defsubst file-attribute-status-change-time (attributes)
  "The status modification time in ATTRIBUTES returned by `file-attributes'.
This is the time of last change to the file's attributes: owner
and group, access mode bits, etc., and is a Lisp timestamp in the
style of `current-time'."
  (nth 6 attributes))

(unless (fboundp 'file-attribute-modes)
  (defsubst file-attribute-modes (attributes)
    "The file modes in ATTRIBUTES returned by `file-attributes'.
This is a string of ten letters or dashes as in ls -l."
    (nth 8 attributes)))

(defsubst file-attribute-inode-number (attributes)
  "The inode number in ATTRIBUTES returned by `file-attributes'.
It is a nonnegative integer."
  (nth 10 attributes))

(defsubst file-attribute-device-number (attributes)
  "The file system device number in ATTRIBUTES returned by `file-attributes'.
It is an integer or a cons cell of integers."
  (nth 11 attributes))

(defsubst file-attribute-file-identifier (attributes)
  "The inode and device numbers in ATTRIBUTES returned by `file-attributes'.
The value is a list of the form (INODENUM DEVICE), where DEVICE could be
either a single number or a cons cell of two numbers.
This tuple of numbers uniquely identifies the file."
  (nthcdr 10 attributes))

(defun file-attribute-collect (attributes &rest attr-names)
  "Return a sublist of ATTRIBUTES returned by `file-attributes'.
ATTR-NAMES are symbols with the selected attribute names.

Valid attribute names are: type, link-number, user-id, group-id,
access-time, modification-time, status-change-time, size, modes,
inode-number, device-number and file-number."
  (let ((all '(type link-number user-id group-id access-time
               modification-time status-change-time
               size modes inode-number device-number file-number))
        result)
    (while attr-names
      (let ((attr (pop attr-names)))
        (if (memq attr all)
            (push (funcall
                   (intern (format "file-attribute-%s" (symbol-name attr)))
                   attributes)
                  result)
          (error "Wrong attribute name '%S'" attr))))
    (nreverse result)))

;;; Misc files.el functions.

(defun file-ownership-preserved-p (file &optional group)
  "Return t if deleting FILE and rewriting it would preserve the owner.
Return also t if FILE does not exist.  If GROUP is non-nil, check whether
the group would be preserved too."
  (let ((handler (find-file-name-handler file 'file-ownership-preserved-p)))
    (if handler
	(funcall handler 'file-ownership-preserved-p file group)
      (let ((attributes (file-attributes file 'integer)))
	;; Return t if the file doesn't exist, since it's true that no
	;; information would be lost by an (attempted) delete and create.
	(or (null attributes)
	    (and (or (= (file-attribute-user-id attributes) (user-uid))
		     ;; Files created on Windows by Administrator (RID=500)
		     ;; have the Administrators group (RID=544) recorded as
		     ;; their owner.  Rewriting them will still preserve the
		     ;; owner.
		     (and (eq system-type 'windows-nt)
			  (= (user-uid) 500)
			  (= (file-attribute-user-id attributes) 544)))
		 (or (not group)
		     ;; On BSD-derived systems files always inherit the parent
		     ;; directory's group, so skip the group-gid test.
		     (memq system-type '(berkeley-unix darwin gnu/kfreebsd))
		     (= (file-attribute-group-id attributes) (group-gid)))
		 (let* ((parent (or (file-name-directory file) "."))
			(parent-attributes (file-attributes parent 'integer)))
		   (and parent-attributes
			;; On some systems, a file created in a setuid directory
			;; inherits that directory's owner.
			(or
			 (= (file-attribute-user-id parent-attributes)
			    (user-uid))
			 (string-match
			  "^...[^sS]"
			  (file-attribute-modes parent-attributes)))
			;; On many systems, a file created in a setgid directory
			;; inherits that directory's group.  On some systems
			;; this happens even if the setgid bit is not set.
			(or (not group)
			    (= (file-attribute-group-id parent-attributes)
			       (file-attribute-group-id attributes)))))))))))

(defvar file-has-changed-p--hash-table (make-hash-table :test #'equal)
  "Internal variable used by `file-has-changed-p'.")

(defun file-has-changed-p (file &optional tag)
  "Return non-nil if FILE has changed.
The size and modification time of FILE are compared to the size
and modification time of the same FILE during a previous
invocation of `file-has-changed-p'.  Thus, the first invocation
of `file-has-changed-p' always returns non-nil when FILE exists.
The optional argument TAG, which must be a symbol, can be used to
limit the comparison to invocations with identical tags; it can be
the symbol of the calling function, for example."
  (let* ((file (directory-file-name (expand-file-name file)))
         (remote-file-name-inhibit-cache t)
         (fileattr (file-attributes file 'integer))
	 (attr (and fileattr
                    (cons (file-attribute-size fileattr)
		          (file-attribute-modification-time fileattr))))
	 (sym (concat (symbol-name tag) "@" file))
	 (cachedattr (gethash sym file-has-changed-p--hash-table)))
     (when (not (equal attr cachedattr))
       (puthash sym attr file-has-changed-p--hash-table))))

(defun buffer-stale--default-function (&optional _noconfirm)
  "Default function to use for `buffer-stale-function'.
This function ignores its argument.
This returns non-nil if the current buffer is visiting a readable file
whose modification time does not match that of the buffer.

This function handles only buffers that are visiting files.
Non-file buffers need a custom function."
  (and buffer-file-name
       (file-readable-p buffer-file-name)
       (not (buffer-modified-p (current-buffer)))
       (not (verify-visited-file-modtime (current-buffer)))))

(defvar buffer-stale-function #'buffer-stale--default-function
  "Function to check whether a buffer needs reverting.
This should be a function with one optional argument NOCONFIRM.
Auto Revert Mode passes t for NOCONFIRM.  The function should return
non-nil if the buffer needs reverting.")

(defun confirm-nonexistent-file-or-buffer ()
  "Whether to request confirmation before visiting a new file or buffer.
The variable `confirm-nonexistent-file-or-buffer' determines the
return value, which may be passed as the REQUIRE-MATCH arg to
`read-buffer' or `find-file-read-args'."
  (cond ((eq confirm-nonexistent-file-or-buffer 'after-completion)
	 'confirm-after-completion)
	(confirm-nonexistent-file-or-buffer
	 'confirm)
	(t nil)))

(defvar inhibit-file-name-handlers nil)
(defvar inhibit-file-name-operation nil)

(defun files--name-absolute-system-p (file)
  "Return non-nil if FILE is an absolute name to the operating system.
This is like `file-name-absolute-p', except that it returns nil for
names beginning with `~'."
  (and (file-name-absolute-p file)
       (not (eq (aref file 0) ?~))))

(defun files--splice-dirname-file (dirname file)
  "Splice DIRNAME to FILE like the operating system would.
If FILE is relative, return DIRNAME concatenated to FILE.
Otherwise return FILE, quoted as needed if DIRNAME and FILE have
different file name handlers; although this quoting is dubious if
DIRNAME is magic, it is not clear what would be better.  This
function differs from `expand-file-name' in that DIRNAME must be
a directory name and leading `~' and `/:' are not special in
FILE."
  (let ((unquoted (if (files--name-absolute-system-p file)
		      file
		    (concat dirname file))))
    (if (eq (find-file-name-handler dirname 'file-symlink-p)
	    (find-file-name-handler unquoted 'file-symlink-p))
	unquoted
      (let (file-name-handler-alist) (file-name-quote unquoted)))))

(defun files--transform-file-name (filename transforms prefix suffix)
  "Transform FILENAME according to TRANSFORMS.
See `auto-save-file-name-transforms' for the format of
TRANSFORMS.  PREFIX is prepended to the non-directory portion of
the resulting file name, and SUFFIX is appended."
  (save-match-data
    (let (result uniq)
      ;; Apply user-specified translations to the file name.
      (while (and transforms (not result))
        (if (string-match (car (car transforms)) filename)
	    (setq result (replace-match (cadr (car transforms)) t nil
				        filename)
		  uniq (car (cddr (car transforms)))))
        (setq transforms (cdr transforms)))
      (when result
        (setq filename
              (cond
               ((memq uniq (secure-hash-algorithms))
                (concat
                 (file-name-directory result)
                 (secure-hash uniq filename)))
               (uniq
                (concat
	         (file-name-directory result)
	         (subst-char-in-string
		  ?/ ?!
		  (string-replace
                   "!" "!!" filename))))
	       (t result))))
      (setq result
	    (if (and (eq system-type 'ms-dos)
		     (not (msdos-long-file-names)))
	        ;; We truncate the file name to DOS 8+3 limits before
	        ;; doing anything else, because the regexp passed to
	        ;; string-match below cannot handle extensions longer
	        ;; than 3 characters, multiple dots, and other
	        ;; atrocities.
	        (let ((fn (dos-8+3-filename
			   (file-name-nondirectory buffer-file-name))))
		  (string-match
		   "\\`\\([^.]+\\)\\(\\.\\(..?\\)?.?\\|\\)\\'"
		   fn)
		  (concat (file-name-directory buffer-file-name)
			  prefix (match-string 1 fn)
			  "." (match-string 3 fn) suffix))
	      (concat (file-name-directory filename)
		      prefix
		      (file-name-nondirectory filename)
		      suffix)))
      ;; Make sure auto-save file names don't contain characters
      ;; invalid for the underlying filesystem.
      (expand-file-name
       (if (and (memq system-type '(ms-dos windows-nt cygwin))
	        ;; Don't modify remote filenames
                (not (file-remote-p result)))
	   (convert-standard-filename result)
         result)))))

(defun file-name-non-special (operation &rest arguments)
  (let ((inhibit-file-name-handlers
         (cons 'file-name-non-special
               (and (eq inhibit-file-name-operation operation)
                    inhibit-file-name-handlers)))
        (inhibit-file-name-operation operation)
        ;; Some operations respect file name handlers in
        ;; `default-directory'.  Because core function like
        ;; `call-process' don't care about file name handlers in
        ;; `default-directory', we here have to resolve the directory
        ;; into a local one.  For `process-file',
        ;; `start-file-process', and `shell-command', this fixes
        ;; Bug#25949.
        (default-directory
	  (if (memq operation
                    '(insert-directory process-file start-file-process
                                       make-process shell-command
                                       temporary-file-directory))
	      (directory-file-name
	       (expand-file-name
		(unhandled-file-name-directory default-directory)))
	    default-directory))
	;; Get a list of the indices of the args that are file names.
	(file-arg-indices
	 (cdr (or (assq operation
			'(;; The first seven are special because they
			  ;; return a file name.  We want to include
			  ;; the /: in the return value.  So just
			  ;; avoid stripping it in the first place.
                          (abbreviate-file-name)
                          (directory-file-name)
                          (file-name-as-directory)
                          (file-name-directory)
                          (file-name-sans-versions)
                          (file-remote-p)
                          (find-backup-file-name)
	                  ;; `identity' means just return the first
			  ;; arg not stripped of its quoting.
			  (substitute-in-file-name identity)
                          ;; `expand-file-name' shall do special case
                          ;; for the first argument starting with
                          ;; "/:~".  (Bug#65685)
                          (expand-file-name expand-file-name)
			  ;; `add' means add "/:" to the result.
			  (file-truename add 0)
                          ;;`insert-file-contents' needs special handling.
			  (insert-file-contents insert-file-contents 0)
			  ;; `unquote-then-quote' means set buffer-file-name
			  ;; temporarily to unquoted filename.
			  (verify-visited-file-modtime unquote-then-quote)
                          ;; Unquote `buffer-file-name' temporarily.
                          (make-auto-save-file-name buffer-file-name)
                          (set-visited-file-modtime buffer-file-name)
                          ;; Use a temporary local copy.
			  (copy-file local-copy)
			  (rename-file local-copy)
                          (copy-directory local-copy)
			  ;; List the arguments that are filenames.
			  (file-name-completion 0 1)
			  (file-name-all-completions 0 1)
                          (file-equal-p 0 1)
                          (file-newer-than-file-p 0 1)
			  (write-region 2 5)
			  (file-in-directory-p 0 1)
			  (make-symbolic-link 0 1)
			  (add-name-to-file 0 1)
                          ;; These file-notify-* operations take a
                          ;; descriptor.
                          (file-notify-rm-watch)
                          (file-notify-valid-p)
                          ;; `make-process' uses keyword arguments and
                          ;; doesn't mangle its filenames in any way.
                          ;; It already strips /: from the binary
                          ;; filename, so we don't have to do this
                          ;; here.
                          (make-process)))
		  ;; For all other operations, treat the first
		  ;; argument only as the file name.
		  '(nil 0))))
	method
	;; Copy ARGUMENTS so we can replace elements in it.
	(arguments (copy-sequence arguments)))
    (if (symbolp (car file-arg-indices))
	(setq method (pop file-arg-indices)))
    ;; Strip off the /: from the file names that have it.
    (save-match-data                    ;FIXME: Why?
      (while (consp file-arg-indices)
	(let ((pair (nthcdr (car file-arg-indices) arguments)))
	  (when (car pair)
	    (setcar pair (file-name-unquote (car pair) t))))
	(setq file-arg-indices (cdr file-arg-indices))))
    ;; In general, we don't want any file name handler, see Bug#47625,
    ;; Bug#48349.  For some few cases, operations with two file name
    ;; arguments which might be bound to different file name handlers,
    ;; we still need this.
    (let ((tramp-mode (and tramp-mode (eq method 'local-copy))))
      (pcase method
        ('identity (car arguments))
        ('expand-file-name
         (when (string-prefix-p "/:~" (car arguments))
           (setcar arguments (file-name-unquote (car arguments) t)))
         (apply operation arguments))
        ('add (file-name-quote (apply operation arguments) t))
        ('buffer-file-name
         (let ((buffer-file-name (file-name-unquote buffer-file-name t)))
           (apply operation arguments)))
        ('insert-file-contents
         (let ((visit (nth 1 arguments)))
           (unwind-protect
               (apply operation arguments)
             (when (and visit buffer-file-name)
               (setq buffer-file-name (file-name-quote buffer-file-name t))))))
        ('unquote-then-quote
         ;; We can't use `cl-letf' with `(buffer-local-value)' here
         ;; because it wouldn't work during bootstrapping.
         (let ((buffer (current-buffer)))
           ;; `unquote-then-quote' is used only for the
           ;; `verify-visited-file-modtime' action, which takes a
           ;; buffer as only optional argument.
           (with-current-buffer (or (car arguments) buffer)
             (let ((buffer-file-name (file-name-unquote buffer-file-name t)))
               ;; Make sure to hide the temporary buffer change from
               ;; the underlying operation.
               (with-current-buffer buffer
                 (apply operation arguments))))))
        ('local-copy
         (let ((source (car arguments))
               (target (car (cdr arguments)))
               (prefix (expand-file-name
                        "file-name-non-special" temporary-file-directory))
               tmpfile)
           (cond
            ;; If source is remote, we must create a local copy.
            ((file-remote-p source)
             (setq tmpfile (make-temp-name prefix))
             (apply operation source tmpfile (cddr arguments))
             (setq source tmpfile))
            ;; If source is quoted, and the unquoted source looks
            ;; remote, we must create a local copy.
            ((file-name-quoted-p source t)
             (setq source (file-name-unquote source t))
             (when (file-remote-p source)
               (setq tmpfile (make-temp-name prefix))
               (let (file-name-handler-alist)
                 (apply operation source tmpfile (cddr arguments)))
               (setq source tmpfile))))
           ;; If target is quoted, and the unquoted target looks
           ;; remote, we must disable the file name handler.
           (when (file-name-quoted-p target t)
             (setq target (file-name-unquote target t))
             (when (file-remote-p target)
               (setq file-name-handler-alist nil)))
           ;; Do it.
           (setcar arguments source)
           (setcar (cdr arguments) target)
           (apply operation arguments)
           ;; Cleanup.
           (when (and tmpfile (file-exists-p tmpfile))
             (if (file-directory-p tmpfile)
                 (delete-directory tmpfile 'recursive) (delete-file tmpfile)))))
        (_
         (apply operation arguments))))))

(defvar lock-file-name-transforms nil
  "Transforms to apply to a file name before making a lock file name.
See `files--transform-file-name' for the format.")

(defun make-lock-file-name (filename)
  "Make a lock file name for FILENAME.
By default, this just prepends \".#\" to the non-directory part
of FILENAME, but the transforms in `lock-file-name-transforms'
are done first."
  (let ((handler (find-file-name-handler filename 'make-lock-file-name)))
    (if handler
	(funcall handler 'make-lock-file-name filename)
      (files--transform-file-name filename lock-file-name-transforms ".#" ""))))


;;; More GNU files.el helpers.

(defcustom out-of-memory-warning-percentage nil
  "Warn if file size exceeds this percentage of available free memory.
When nil, never issue warning.  Beware: This probably doesn't do what you
think it does, because "free" is pretty hard to define in practice."
  :group 'files
  :group 'find-file
  :version "25.1"
  :type '(choice integer (const :tag "Never issue warning" nil)))

(defun files--ensure-directory (dir)
  "Make directory DIR if it is not already a directory.
Return non-nil if DIR is already a directory."
  (condition-case err
      (make-directory-internal dir)
    (error
     (or (file-directory-p dir)
	 (signal err)))))

(defun make-empty-file (filename &optional parents)
  "Create an empty file FILENAME.
Optional arg PARENTS, if non-nil then creates parent dirs as needed.

If called interactively, then PARENTS is non-nil."
  (interactive
   (let ((filename (read-file-name "Create empty file: ")))
     (list filename t)))
  (when (and (file-exists-p filename) (null parents))
    (signal 'file-already-exists `("File exists" ,filename)))
  (let ((paren-dir (file-name-directory filename)))
    (when (and paren-dir (not (file-exists-p paren-dir)))
      (make-directory paren-dir parents)))
  (write-region "" nil filename nil 0))

(defun files--force (no-such fn &rest args)
  "Use NO-SUCH to affect behavior of function FN applied to list ARGS.
This acts like (apply FN ARGS) except it returns NO-SUCH if it is
non-nil and if FN fails due to a missing file or directory."
  (condition-case err
      (apply fn args)
    (file-missing (or no-such (signal err)))))

(defvar save-silently nil)

(defun files--message (format &rest args)
  "Like `message', except sometimes don't show the message text.
If the variable `save-silently' is non-nil, the message will not
be visible in the echo area."
  (apply #'message format args)
  (when save-silently (message nil)))

(defun files--make-magic-temp-file (absolute-prefix
                                    &optional dir-flag suffix text)
  "Implement (make-temp-file ABSOLUTE-PREFIX DIR-FLAG SUFFIX TEXT).
This implementation works on magic file names."
  ;; Create temp files with strict access rights.  It's easy to
  ;; loosen them later, whereas it's impossible to close the
  ;; time-window of loose permissions otherwise.
  (with-file-modes ?\700
    (let ((contents (if (stringp text) text ""))
          file)
      (while (condition-case ()
		 (progn
		   (setq file (make-temp-name absolute-prefix))
		   (if suffix
		       (setq file (concat file suffix)))
		   (if dir-flag
		       (make-directory file)
		     (write-region contents nil file nil 'silent nil 'excl))
		   nil)
	       (file-already-exists t))
	;; the file was somehow created by someone else between
	;; `make-temp-name' and `write-region', let's try again.
	nil)
      file)))

(defun make-nearby-temp-file (prefix &optional dir-flag suffix)
  "Create a temporary file as close as possible to `default-directory'.
Return the absolute file name of the created file.
If PREFIX is a relative file name, and `default-directory' is a
remote file name or located on a mounted file systems, the
temporary file is created in the directory returned by the
function `temporary-file-directory'.  Otherwise, the function
`make-temp-file' is used.  PREFIX, DIR-FLAG and SUFFIX have the
same meaning as in `make-temp-file'."
  (let ((handler (find-file-name-handler
                  default-directory 'make-nearby-temp-file)))
    (if (and handler (not (file-name-absolute-p default-directory)))
	(funcall handler 'make-nearby-temp-file prefix dir-flag suffix)
      (let ((temporary-file-directory (temporary-file-directory)))
        (make-temp-file prefix dir-flag suffix)))))

(defun prune-directory-list (dirs &optional keep reject)
  "Return a copy of DIRS with all non-existent directories removed.
The optional argument KEEP is a list of directories to retain even if
they don't exist, and REJECT is a list of directories to remove from
DIRS, even if they exist; REJECT takes precedence over KEEP.

Note that membership in REJECT and KEEP is checked using simple string
comparison."
  (apply #'nconc
	 (mapcar (lambda (dir)
		   (and (not (member dir reject))
			(or (member dir keep) (file-directory-p dir))
			(list dir)))
		 dirs)))

(defun files--ask-user-about-large-file-help-text (op-type size)
  "Format the text that explains the options to open large files in Emacs.
OP-TYPE contains the kind of file operation that will be
performed.  SIZE is the size of the large file."
  (format
   "The file that you want to %s is large (%s), which exceeds the
 threshold above which Emacs asks for confirmation (%s).

 Large files may be slow to edit or navigate so Emacs asks you
 before you try to %s such files.

 You can press:
 'y' to %s the file.
 'n' to abort, and not %s the file.
 'l' (the letter ell) to %s the file literally, which means that
 Emacs will %s the file without doing any format or character code
 conversion and in Fundamental mode, without loading any potentially
 expensive features.

 You can customize the option `large-file-warning-threshold' to be the
 file size, in bytes, from which Emacs will ask for confirmation.  Set
 it to nil to never request confirmation."
   op-type
   size
   (funcall byte-count-to-string-function large-file-warning-threshold)
   op-type
   op-type
   op-type
   op-type
   op-type))

(defun files--ask-user-about-large-file (size op-type filename offer-raw)
  "Query the user about what to do with large files.
Files are \"large\" if file SIZE is larger than `large-file-warning-threshold'.

OP-TYPE specifies the file operation being performed on FILENAME.

If OFFER-RAW is true, give user the additional option to open the
file literally."
  (let ((prompt (format "File %s is large (%s), really %s?"
		        (file-name-nondirectory filename)
		        (funcall byte-count-to-string-function size) op-type)))
    (if (not offer-raw)
        (if (y-or-n-p prompt) nil 'abort)
      (let ((choice
             (car
              (read-multiple-choice
               prompt '((?y "yes")
                        (?n "no")
                        (?l "literally"))
               (files--ask-user-about-large-file-help-text
                op-type (funcall byte-count-to-string-function size))))))
        (cond ((eq choice ?y) nil)
              ((eq choice ?l) 'raw)
              (t 'abort))))))

(defun abort-if-file-too-large (size op-type filename &optional offer-raw)
  "If file SIZE larger than `large-file-warning-threshold', allow user to abort.
OP-TYPE specifies the file operation being performed (for message
to user).  If OFFER-RAW is true, give user the additional option
to open the file literally.  If the user chooses this option,
`abort-if-file-too-large' returns the symbol `raw'.  Otherwise,
it returns nil or exits non-locally."
  (let ((choice (and large-file-warning-threshold size
	             (> size large-file-warning-threshold)
                     ;; No point in warning if we can't read it.
                     (file-readable-p filename)
                     (files--ask-user-about-large-file
                      size op-type filename offer-raw))))
    (when (eq choice 'abort)
      (user-error "Aborted"))
    choice))

(defun warn-maybe-out-of-memory (size)
  "Warn if an attempt to open file of SIZE bytes may run out of memory."
  (when (and (numberp size) (not (zerop size))
	     (integerp out-of-memory-warning-percentage))
    (let* ((default-directory temporary-file-directory)
           (meminfo (memory-info)))
      (when (consp meminfo)
	(let ((total-free-memory (float (+ (nth 1 meminfo) (nth 3 meminfo)))))
	  (when (> (/ size 1024)
		   (/ (* total-free-memory out-of-memory-warning-percentage)
		      100.0))
	    (warn
	     "You are trying to open a file whose size (%s)
exceeds the %S%% of currently available free memory (%s).
If that fails, try to open it with `find-file-literally'
\(but note that some characters might be displayed incorrectly)."
	     (funcall byte-count-to-string-function size)
	     out-of-memory-warning-percentage
	     (funcall byte-count-to-string-function
                      (* total-free-memory 1024)))))))))

(defun kill-buffer-ask (buffer)
  "Kill BUFFER if confirmed."
  (when (yes-or-no-p (format "Buffer %s %s.  Kill? "
			     (buffer-name buffer)
			     (if (buffer-modified-p buffer)
				 "HAS BEEN EDITED" "is unmodified")))
    (kill-buffer buffer)))

(defun kill-some-buffers (&optional list)
  "Kill some buffers.  Asks the user whether to kill each one of them.
Non-interactively, if optional argument LIST is non-nil, it
specifies the list of buffers to kill, asking for approval for each one."
  (interactive)
  (if (null list)
      (setq list (buffer-list)))
  (while list
    (let* ((buffer (car list))
	   (name (buffer-name buffer)))
      (and name				; Can be nil for an indirect buffer
					; if we killed the base buffer.
	   (not (string-equal name ""))
	   (/= (aref name 0) ?\s)
	   (kill-buffer-ask buffer)))
    (setq list (cdr list))))

(defun kill-matching-buffers (regexp &optional internal-too no-ask)
  "Kill buffers whose names match the regular expression REGEXP.
Interactively, prompt for REGEXP.
Ignores buffers whose names start with a space, unless optional
prefix argument INTERNAL-TOO(interactively, the prefix argument)
is non-nil.  Asks before killing each buffer, unless NO-ASK is non-nil."
  (interactive "sKill buffers matching this regular expression: \nP")
  (dolist (buffer (buffer-list))
    (let ((name (buffer-name buffer)))
      (when (and name (not (string-equal name ""))
                 (or internal-too (/= (aref name 0) ?\s))
                 (string-match regexp name))
        (funcall (if no-ask 'kill-buffer 'kill-buffer-ask) buffer)))))

(defun kill-matching-buffers-no-ask (regexp &optional internal-too)
  "Kill buffers whose names match the regular expression REGEXP.
Interactively, prompt for REGEXP.
Like `kill-matching-buffers', but doesn't ask for confirmation
before killing each buffer.
Ignores buffers whose names start with a space, unless the
optional argument INTERNAL-TOO (interactively, the prefix argument)
is non-nil."
  (interactive "sKill buffers matching this regular expression: \nP")
  (kill-matching-buffers regexp internal-too t))

(defun rename-auto-save-file ()
  "Adjust current buffer's auto save file name for current conditions.
Also rename any existing auto save file, if it was made in this session."
  (let ((osave buffer-auto-save-file-name))
    (setq buffer-auto-save-file-name
	  (make-auto-save-file-name))
    (if (and osave buffer-auto-save-file-name
	     (not (string= buffer-auto-save-file-name buffer-file-name))
	     (not (string= buffer-auto-save-file-name osave))
	     (file-exists-p osave)
	     (recent-auto-save-p))
	(rename-file osave buffer-auto-save-file-name t))))

(defun save-some-buffers-root ()
  "A predicate to check whether the buffer is under the project root directory.
Can be used as a value of `save-some-buffers-default-predicate'
to save buffers only under the project root or in subdirectories
of the directory that was default during command invocation."
  (let ((root (or (and (featurep 'project) (project-current)
                       (fboundp 'project-root)
                       (project-root (project-current)))
                  default-directory)))
    (lambda () (file-in-directory-p default-directory root))))
(put 'save-some-buffers-root 'save-some-buffers-function t)

(defun files--buffers-needing-to-be-saved (pred)
  "Return a list of buffers to save according to PRED.
See `save-some-buffers' for PRED values."
  (let ((buffers
         (mapcar (lambda (buffer)
                   (if
                       ;; Note that killing some buffers may kill others via
                       ;; hooks (e.g. Rmail and its viewing buffer).
                       (and (buffer-live-p buffer)
	                    (buffer-modified-p buffer)
                            (not (buffer-base-buffer buffer))
                            (or
                             (buffer-file-name buffer)
                             (with-current-buffer buffer
                               (or (eq buffer-offer-save 'always)
                                   (and pred buffer-offer-save
                                        (> (buffer-size) 0)))))
                            (or (not (functionp pred))
                                (with-current-buffer buffer
                                  (funcall pred))))
                       buffer))
                 (buffer-list))))
    (delq nil buffers)))

(defun rename-visited-file (new-location)
  "Rename the file visited by the current buffer to NEW-LOCATION.
This command also sets the visited file name.  If the buffer
isn't visiting any file, that's all it does.

Interactively, this prompts for NEW-LOCATION."
  (interactive
   (list (if buffer-file-name
             (read-file-name "Rename visited file to: ")
           (read-file-name "Set visited file name: "
                           default-directory
                           (expand-file-name
                            (file-name-nondirectory (buffer-name))
                            default-directory)))))
  ;; If the user has given a directory name, the file should be moved
  ;; there (under the same file name).
  (when (file-directory-p new-location)
    (unless buffer-file-name
      (user-error "Can't rename buffer to a directory file name"))
    (setq new-location (expand-file-name
                        (file-name-nondirectory buffer-file-name)
                        new-location)))
  (when (and buffer-file-name
             (file-exists-p buffer-file-name))
    (rename-file buffer-file-name new-location))
  (set-visited-file-name new-location nil t))

(defcustom find-sibling-rules nil
  "Rules for finding \"sibling\" files.
This is used by the `find-sibling-file' command.

The value of this variable should a list (RULE1 RULE2 ...), where each
RULE has the form (MATCH EXPANSION...).

MATCH is a regular expression that should match a file name which might
have a sibling.  It can contain sub-expressions that will be used in
each EXPANSION as \\N and \\& replacements.

Each EXPANSION is a string that matches names of files that are to be
considered siblings of a file whose name matches MATCH."
  :type '(alist :key-type (regexp :tag "Match")
                :value-type (repeat (string :tag "Expansion")))
  :version "29.1")

(defun find-sibling-file (file)
  "Visit a \"sibling\" file of FILE.
When called interactively, FILE is the currently visited file.

The \"sibling\" file is defined by the `find-sibling-rules' variable."
  (interactive (progn
                 (unless buffer-file-name
                   (user-error "Not visiting a file"))
                 (list buffer-file-name)))
  (unless find-sibling-rules
    (user-error "The `find-sibling-rules' variable has not been configured"))
  (let ((siblings (find-sibling-file-search (expand-file-name file)
                                            find-sibling-rules)))
    (cond
     ((null siblings)
      (user-error "Couldn't find any sibling files"))
     ((length= siblings 1)
      (find-file (car siblings)))
     (t
      (let ((relatives (mapcar (lambda (sibling)
                                 (file-relative-name
                                  sibling (file-name-directory file)))
                               siblings)))
        (find-file
         (completing-read (format-prompt "Find file" (car relatives))
                          relatives nil t nil nil (car relatives))))))))

(defun find-sibling-file-search (file &optional rules)
  "Return a list of FILE's \"siblings\".
RULES should be a list on the form defined by `find-sibling-rules' (which
see), and if nil, defaults to `find-sibling-rules'."
  (let ((results nil))
    (pcase-dolist (`(,match . ,expansions) (or rules find-sibling-rules))
      ;; Go through the list and find matches.
      (when (string-match match file)
        (let ((match-data (match-data)))
          (dolist (expansion expansions)
            (let ((start 0))
              ;; Expand \\1 forms in the expansions.
              (while (string-match "\\\\\\([&0-9]+\\)" expansion start)
                (let* ((index (string-to-number (match-string 1 expansion)))
                       (value (substring file
                                         (elt match-data (* index 2))
                                         (elt match-data (1+ (* index 2))))))
                  (setq start (+ (match-beginning 0) (length value))
                        expansion (replace-match value t t expansion)))))
            ;; Then see which files we have that are matching.  (And
            ;; expand from the end of the file's match, since we might
            ;; be doing a relative match.)
            (let ((default-directory (substring file 0 (car match-data))))
              ;; Keep the first matches first.
              (setq results
                    (nconc
                     results
                     (mapcar #'expand-file-name
                             (file-expand-wildcards expansion nil t)))))))))
    ;; Delete the file itself (in case it matched), and remove
    ;; duplicates, in case we have several expansions and some match
    ;; the same subsets of files.
    (delete file (delete-dups results))))

(defun make-auto-save-file-name ()
  "Return file name to use for auto-saves of current buffer.
Does not consider `auto-save-visited-file-name' as that variable is checked
before calling this function.
See also `auto-save-file-name-p'."
  (if buffer-file-name
      (let ((handler (find-file-name-handler
                      buffer-file-name 'make-auto-save-file-name)))
	(if handler
	    (funcall handler 'make-auto-save-file-name)
          (files--transform-file-name
           buffer-file-name auto-save-file-name-transforms
                                          "#" "#")))
    ;; Deal with buffers that don't have any associated files.  (Mail
    ;; mode tends to create a good number of these.)
    (let ((buffer-name (buffer-name))
	  (limit 0)
	  file-name)
      ;; Restrict the characters used in the file name to those that
      ;; are known to be safe on all filesystems, url-encoding the
      ;; rest.
      ;; We do this on all platforms, because even if we are not
      ;; running on DOS/Windows, the current directory may be on a
      ;; mounted VFAT filesystem, such as a USB memory stick.
      (while (string-match "[^A-Za-z0-9_.~#+-]" buffer-name limit)
	(let* ((character (aref buffer-name (match-beginning 0)))
	       (replacement
                ;; For multibyte characters, this will produce more than
                ;; 2 hex digits, so is not true URL encoding.
                (format "%%%02X" character)))
	  (setq buffer-name (replace-match replacement t t buffer-name))
	  (setq limit (1+ (match-end 0)))))
      ;; Generate the file name.
      (setq file-name
	    (make-temp-file
	     (let ((fname
		    (expand-file-name
		     (format "#%s#" buffer-name)
		     ;; Try a few alternative directories, to get one we can
		     ;; write it.
		     (cond
		      ((file-writable-p default-directory) default-directory)
		      ((file-writable-p "/var/tmp/") "/var/tmp/")
		      ("~/")))))
	       (if (and (memq system-type '(ms-dos windows-nt cygwin))
			;; Don't modify remote filenames
			(not (file-remote-p fname)))
		   ;; The call to convert-standard-filename is in case
		   ;; buffer-name includes characters not allowed by the
		   ;; DOS/Windows filesystems.  make-temp-file writes to the
		   ;; file it creates, so we must fix the file name _before_
		   ;; make-temp-file is called.
		   (convert-standard-filename fname)
		 fname))
	     nil "#"))
      ;; make-temp-file creates the file,
      ;; but we don't want it to exist until we do an auto-save.
      (condition-case ()
	  (delete-file file-name)
	(file-error nil))
      file-name)))

(defcustom auto-save-file-name-transforms
  `(("\\`/[^/]*:\\([^/]*/\\)*\\([^/]*\\)\\'"
     ;; Don't put "\\2" inside expand-file-name, since it will be
     ;; transformed to "/2" on DOS/Windows.
     ,(concat temporary-file-directory "\\2") t))
  "Transforms to apply to buffer file name before making auto-save file name."
  :type '(repeat (list (regexp :tag "Regexp")
                       (string :tag "Replacement")
                       (boolean :tag "Uniquify")))
  :group 'auto-save
  :version "21.1")

(defun insert-file-1 (filename insert-func)
  (if (file-directory-p filename)
      (signal 'file-error (list "Opening input file" "Is a directory"
                                filename)))
  ;; Check whether the file is uncommonly large
  (abort-if-file-too-large (file-attribute-size (file-attributes filename))
			   "insert" filename)
  (let* ((buffer (find-buffer-visiting (abbreviate-file-name (file-truename filename))
                                       #'buffer-modified-p))
         (tem (funcall insert-func filename)))
    (push-mark (+ (point) (car (cdr tem))))
    (when buffer
      (message "File %s already visited and modified in buffer %s"
               filename (buffer-name buffer)))))

(defun insert-file-literally (filename)
  "Insert contents of file FILENAME into buffer after point with no conversion.

This function is meant for the user to run interactively.
Don't call it from programs!  Use `insert-file-contents-literally' instead.
\(Its calling sequence is different; see its documentation)."
  (declare (interactive-only insert-file-contents-literally))
  (interactive "*fInsert file literally: ")
  (insert-file-1 filename #'insert-file-contents-literally))

;;; GNU subr.el additions.

(defvar undo--combining-change-calls nil
  "Non-nil when `combine-change-calls-1' is running.")

(defun internal--compiler-macro-cXXr (form x)
  (let* ((head (car form))
         (n (symbol-name head))
         (i (- (length n) 2)))
    (if (not (string-match "c[ad]+r\\'" n))
        (if (and (fboundp head) (symbolp (symbol-function head)))
            (internal--compiler-macro-cXXr
             (cons (symbol-function head) (cdr form)) x)
          (error "Compiler macro for cXXr applied to non-cXXr form"))
      (while (> i (match-beginning 0))
        (setq x (list (if (eq (aref n i) ?a) 'car 'cdr) x))
        (setq i (1- i)))
      x)))

(defun internal--effect-free-fun-arg-p (x)
  ;; FIXME: Rename it to `macroexp-FOO-p' and give it a proper docstring
  ;; which explains the finer difference with `macroexp-copyable-p'
  ;; (and maybe adjust the docstring of `macroexp-copyable-p' accordingly).
  (or (closurep x) (memq (car-safe x) '(function quote))))

(defun package--description-file (dir)
  "Return package description file name for package DIR."
  (concat (let ((subdir (file-name-nondirectory
                         (directory-file-name dir))))
            ;; This needs to match only the version strings that can be
            ;; generated by `package-version-join'.
            (if (string-match "\\([^.].*?\\)-\\([0-9]+\\(?:[.][0-9]+\\|\\(?:pre\\|beta\\|alpha\\|snapshot\\)[0-9]*\\)*\\)\\'" subdir)
                (match-string 1 subdir) subdir))
          "-pkg.el"))

(defun total-line-spacing (&optional line-spacing-param)
  "Return numeric value of line-spacing, summing it if it's a cons.
   When LINE-SPACING-PARAM is provided, calculate from it instead."
  (let ((v (or line-spacing-param line-spacing)))
    (pcase v
      ((pred numberp) v)
      (`(,above . ,below) (+ above below)))))

(defalias 'kill-comment 'comment-kill)

(define-obsolete-function-alias 'compare-window-configurations
  #'window-configuration-equal-p "29.1")

(define-obsolete-function-alias 'fetch-bytecode #'ignore "30.1")

(provide 'subr-x)

;;; subr-x.el ends here
