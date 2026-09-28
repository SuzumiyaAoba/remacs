;;; easy-mmode.el --- easy minor mode definition -*- lexical-binding: t -*-

;; A subset of GNU's easy-mmode: `define-minor-mode' covers the common
;; keywords (:init-value :lighter :keymap :global :variable :group) and
;; generates the same observable behavior — a bound mode variable, a
;; toggle function, a `MODE-hook' run, and a `minor-mode-alist' entry.

;; GNU keeps these on subr-x/simple; they must exist before any
;; `define-minor-mode' expansion runs, so define them here.
(defvar local-minor-modes nil
  "A list of currently active minor mode commands for the current buffer.")
(defvar global-minor-modes nil
  "A list of currently active global minor mode commands.")

;; GNU-verbatim easy-mmode.el.
(defun easy-mmode-pretty-mode-name (mode &optional lighter)
  "Turn the symbol MODE into a string intended for the user.
If provided, LIGHTER will be used to help choose capitalization by,
replacing its case-insensitive matches with the literal string in LIGHTER."
  (let* ((case-fold-search t)
	 ;; Produce "Foo-Bar minor mode" from foo-bar-minor-mode.
	 (name (concat (replace-regexp-in-string
			;; If the original mode name included "-minor" (some
			;; of them don't, e.g. auto-revert-mode), then
			;; replace it with " minor".
			"-Minor" " minor"
			;; "foo-bar-minor" -> "Foo-Bar-Minor"
			(capitalize (replace-regexp-in-string
				     ;; "foo-bar-minor-mode" -> "foo-bar-minor"
				     "toggle-\\|-mode\\'" ""
                                     (symbol-name mode))))
		       " mode")))
    (setq name (replace-regexp-in-string "\\`Global-" "Global " name))
    (if (not (stringp lighter)) name
      ;; Strip leading and trailing whitespace from LIGHTER.
      (setq lighter (replace-regexp-in-string "\\`\\s-+\\|\\s-+\\'" ""
					      lighter))
      ;; Replace any (case-insensitive) matches for LIGHTER in NAME
      ;; with a literal LIGHTER.  E.g., if NAME is "Iimage mode" and
      ;; LIGHTER is " iImag", then this will produce "iImage mode".
      ;; (LIGHTER normally comes from the mode-line string passed to
      ;; define-minor-mode, and normally includes at least one leading
      ;; space.)
      (replace-regexp-in-string (regexp-quote lighter) lighter name t t))))

(defmacro define-minor-mode (mode doc &rest args)
  "Define a new minor mode MODE.
The following keyword arguments are supported: :init-value, :lighter,
:keymap, :global, :variable, :group.  MODE is toggled with (MODE),
forced on/off with positive/nonpositive numeric arguments, and
'toggle' toggles explicitly.  Forms following the keyword arguments
are the mode's body, run on each toggle."
  ;; Split ARGS into the keyword/value prefix and the body suffix.
  (let ((kw nil) (rest args))
    (while (and rest (keywordp (car rest)))
      (setq kw (append kw (list (car rest) (cadr rest)))
            rest (cddr rest)))
    (let* ((body rest)
           (init-value (plist-get kw :init-value))
           (lighter (plist-get kw :lighter))
           (keymap (plist-get kw :keymap))
           (global (plist-get kw :global))
           (has-variable (plist-member kw :variable))
           (variable-spec (plist-get kw :variable))
           ;; GNU easy-mmode treats :variable as the PLACE where the
           ;; state is stored: either a (GET . SET-FN) cons pair or a
           ;; generalized-variable form (e.g. (default-value 'VAR)).
           ;; Without :variable the place is the variable MODE itself.
           (pair-setter
            (and (consp variable-spec)
                 (cdr-safe variable-spec)
                 (or (symbolp (cdr variable-spec))
                     (functionp (cdr variable-spec)))
                 (cdr variable-spec)))
           (getter (cond (pair-setter (car variable-spec))
                         (has-variable variable-spec)
                         (t mode)))
           ;; GNU never defvars a :variable place ("the var will be
           ;; declared elsewhere"); it defvars MODE only when no
           ;; :variable was given.
           (variable-defvar (not has-variable))
           ;; The alist key GNU's `add-minor-mode' registers under:
           ;; (default-value 'VAR) unfolds to VAR.
           (modevar (if (and (consp getter)
                             (eq (car getter) 'default-value)
                             (consp (cdr getter))
                             (consp (cadr getter))
                             (eq (car (cadr getter)) 'quote))
                        (cadr (cadr getter))
                      getter))
           (name (symbol-name mode))
           (pretty (easy-mmode-pretty-mode-name mode))
           (hook (intern (concat name "-hook")))
           (map-sym (intern (concat name "-map"))))
      `(progn
         ;; GNU: local modes get `defvar-local', global modes a
         ;; `defcustom' whose :set is `custom-set-minor-mode' (so
         ;; setq/customize actually toggle the mode).
         ,@(when variable-defvar
             (if global
                 `((defcustom ,mode ,init-value ,doc
                     ,@(list :set (or (plist-get kw :set)
                                      '#'custom-set-minor-mode))
                     ,@(list :initialize (or (plist-get kw :initialize)
                                             '#'custom-initialize-default))
                     :type 'boolean
                     ;; GNU: ,@(nreverse extra-keywords) — any other
                     ;; keywords (:group, :require, :version, user keys)
                     ;; pass through to the defcustom.
                     ,@(let ((extra nil) (rest kw))
                         (while rest
                           (unless (memq (car rest)
                                         '(:init-value :lighter :keymap
                                           :global :variable :set
                                           :initialize :type))
                             (push (car rest) extra)
                             (push (cadr rest) extra))
                           (setq rest (cddr rest)))
                         (nreverse extra)))
                   ;; GNU: a non-nil :init-value records the mode on
                   ;; `global-minor-modes' at definition time.
                   ,@(when init-value
                       `((when (bound-and-true-p ,mode)
                           (add-to-list 'global-minor-modes ',mode)))))
               `((defvar-local ,mode ,init-value ,doc))))
         ,@(when keymap `((defvar ,map-sym ,keymap)))
         (defun ,mode (&optional arg)
           ,doc
           (interactive (list (or current-prefix-arg 'toggle)))
           ,(let ((newval
                   `(cond ((eq arg 'toggle) (not ,getter))
                           ((null arg) t)
                           ((and (consp arg)
                                 (eq (car arg) 'toggle))
                            (not ,getter))
                           (t (> (prefix-numeric-value arg) 0)))))
              (cond
               (pair-setter `(funcall #',pair-setter ,newval))
               ;; A :variable place: GNU emits `(setf PLACE VAL)'
               ;; — (default-value 'x) → set-default; a bare symbol
               ;; gets plain setq even for :global modes.
               (has-variable `(setf ,getter ,newval))
               ;; GNU uses `setq-default' for :global mode vars.
               (global `(setq-default ,getter ,newval))
               (t `(setq ,getter ,newval))))
           ,@(when global
               `((when (boundp 'global-minor-modes)
                   (if ,getter
                       (add-to-list 'global-minor-modes ',mode)
                     (setq global-minor-modes
                           (delq ',mode global-minor-modes))))))
           ,@body
           (run-hooks ',hook)
           (when (called-interactively-p 'interactive)
             (message "%s mode %s" ,pretty
                      (if ,getter "enabled" "disabled")))
           ,getter)
         (defvar ,hook nil)
         ,@(when global `((put ',mode 'global-minor-mode t)))
         ;; GNU emits a single `add-minor-mode' call: it pushes MODEVAR
         ;; onto `minor-mode-list' unconditionally, files the raw
         ;; `:lighter' on `minor-mode-alist' only when non-nil, and
         ;; registers the keymap on `minor-mode-map-alist'.  For a
         ;; non-symbol getter place there is no variable key.
         ,@(when (symbolp modevar)
             `((add-minor-mode ',modevar ',lighter
                               ,(if keymap map-sym
                                  `(if (boundp ',map-sym) ,map-sym)))))
         ',mode))))

(defmacro define-globalized-minor-mode (global-mode mode turn-on &rest keys)
  "Define a global minor mode GLOBAL-MODE corresponding to buffer-local MODE.
TURN-ON is a function or form run in each buffer to enable MODE."
  (let* ((mode-name (symbol-name mode))
         (set-explicitly (intern (concat mode-name "--set-explicitly")))
         (suppress-set-explicitly
          (intern (concat mode-name "--suppress-set-explicitly")))
         (enable-in-buffer
          (intern (concat (symbol-name global-mode) "-enable-in-buffer")))
         (mode-hook (intern (concat mode-name "-hook"))))
    `(progn
       (define-minor-mode ,global-mode
         ,(format "Toggle %s in all buffers." global-mode)
         :global t ,@keys
         (dolist (buf (buffer-list))
           (with-current-buffer buf
             (if ,global-mode
                 (,enable-in-buffer)
               (,mode -1)))))
       ;; GNU's `define-globalized-minor-mode' records explicit user
       ;; toggles in `MODE--set-explicitly' (cleared by
       ;; `kill-all-local-variables' since it is `defvar-local'); a
       ;; globalized enable skips buffers where it is set.
       (defvar-local ,set-explicitly nil)
       (defvar ,suppress-set-explicitly nil)
       (defun ,set-explicitly ()
         (unless ,suppress-set-explicitly
           (setq ,set-explicitly t)))
       (put ',set-explicitly 'definition-name ',global-mode)
       (add-hook ',mode-hook #',set-explicitly)
       (defun ,enable-in-buffer ()
         (unless ,set-explicitly
           (let ((,suppress-set-explicitly t))
             ,(if (symbolp turn-on)
                  (list 'funcall (list 'quote turn-on))
                (list 'funcall turn-on)))))
       (put ',enable-in-buffer 'definition-name ',global-mode))))

;; GNU: `easy-mmode-define-minor-mode' is the pre-30.1 name, and
;; `define-global-minor-mode' the pre-29 name of the globalized macro.
(defalias 'easy-mmode-define-minor-mode #'define-minor-mode)
(defalias 'define-global-minor-mode #'define-globalized-minor-mode)


(defun easy-mmode--prev (re name count &optional endfun narrowfun)
  "Go to the COUNT'th previous occurrence of RE.

If none, error with NAME.

ENDFUN and NARROWFUN are treated like in `easy-mmode-define-navigation'."
  (unless count (setq count 1))
  (if (< count 0) (easy-mmode--next re name (- count) endfun narrowfun)
    (let ((re-narrow (and narrowfun (prog1 (buffer-narrowed-p) (widen)))))
      ;; If point is inside a match for RE, move to its beginning like
      ;; `backward-sexp' and other movement commands.
      (when (and (not (zerop count))
                 (save-excursion
                   ;; Make sure we're out of the current match if any.
                   (goto-char (if (re-search-backward re nil t 1)
                                  (match-end 0) (point-min)))
                   (re-search-forward re nil t 1))
                 (< (match-beginning 0) (point) (match-end 0)))
        (goto-char (match-beginning 0))
        (setq count (1- count)))
      (unless (re-search-backward re nil t count)
        (user-error "No previous %s" name))
      (when re-narrow (funcall narrowfun)))))

(defun easy-mmode--next (re name count &optional endfun narrowfun)
  "Go to the next COUNT'th occurrence of RE.

If none, error with NAME.

ENDFUN and NARROWFUN are treated like in `easy-mmode-define-navigation'."
  (unless count (setq count 1))
  (if (< count 0) (easy-mmode--prev re name (- count) endfun narrowfun)
    (if (looking-at re) (setq count (1+ count)))
    (let ((re-narrow (and narrowfun (prog1 (buffer-narrowed-p) (widen)))))
      (if (not (re-search-forward re nil t count))
          (if (looking-at re)
              (goto-char (or (if endfun (funcall endfun)) (point-max)))
            (user-error "No next %s" name))
        (goto-char (match-beginning 0))
        (when (and (eq (current-buffer) (window-buffer))
                   (called-interactively-p 'interactive))
          (let ((endpt (or (save-excursion
                             (if endfun (funcall endfun)
                               (re-search-forward re nil t 2)))
                           (point-max))))
            (unless (pos-visible-in-window-p endpt nil t)
              (let ((ws (window-start)))
                (recenter '(0))
                (if (< (window-start) ws)
                    ;; recenter scrolled in the wrong direction!
                    (set-window-start nil ws)))))))
      (when re-narrow (funcall narrowfun)))))

(defmacro easy-mmode-define-navigation (base re &optional name endfun narrowfun
                                             &rest body)
  "Define BASE-next and BASE-prev to navigate in the buffer.
RE determines the places the commands should move point to.
NAME should describe the entities matched by RE.  It is used to build
  the docstrings of the two functions.
BASE-next also tries to make sure that the whole entry is visible by
  searching for its end (by calling ENDFUN if provided or by looking for
  the next entry) and recentering if necessary.
ENDFUN should return the end position (with or without moving point).
NARROWFUN non-nil means to check for narrowing before moving, and if
found, do `widen' first and then call NARROWFUN with no args after moving.
BODY is executed after moving to the destination location."
  (declare (indent 5) (debug (exp exp exp def-form def-form def-body)))
  (let* ((base-name (symbol-name base))
	 (prev-sym (intern (concat base-name "-prev")))
	 (next-sym (intern (concat base-name "-next")))
         (endfun (when endfun `#',endfun))
         (narrowfun (when narrowfun `#',narrowfun)))
    (unless name (setq name base-name))
    `(progn
       (defun ,next-sym (&optional count)
	 ,(format "Go to the next COUNT'th %s.
Interactively, COUNT is the prefix numeric argument, and defaults to 1." name)
	 (interactive "p")
         (easy-mmode--next ,re ,name count ,endfun ,narrowfun)
         ,@body)
       (put ',next-sym 'definition-name ',base)
       (defun ,prev-sym (&optional count)
	 ,(format "Go to the previous COUNT'th %s.
Interactively, COUNT is the prefix numeric argument, and defaults to 1." name)
	 (interactive "p")
         (easy-mmode--prev ,re ,name count ,endfun ,narrowfun)
         ,@body)
       (put ',prev-sym 'definition-name ',base))))

(provide 'easy-mmode)
