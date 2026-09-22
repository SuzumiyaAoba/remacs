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

(provide 'subr-x)

;;; subr-x.el ends here
