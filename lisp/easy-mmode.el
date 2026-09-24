;;; easy-mmode.el --- easy minor mode definition -*- lexical-binding: t -*-

;; A subset of GNU's easy-mmode: `define-minor-mode' covers the common
;; keywords (:init-value :lighter :keymap :global :variable :group) and
;; generates the same observable behavior — a bound mode variable, a
;; toggle function, a `MODE-hook' run, and a `minor-mode-alist' entry.

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
           (variable-spec (or (plist-get kw :variable) mode))
           ;; GNU's :variable can be (VAR . SETTER) where SETTER
           ;; takes the new value (e.g. setq-local).  GET may itself
           ;; be a form like (get-scroll-bar-mode); then it is not a
           ;; variable to defvar, just the state expression.
           (variable (if (consp variable-spec) (car variable-spec) variable-spec))
           (variable-setter (and (consp variable-spec) (cdr variable-spec)))
           (variable-defvar (symbolp variable))
           (name (symbol-name mode))
           (pretty (capitalize
                    (if (string-suffix-p "-mode" name)
                        (substring name 0 -5)
                      name)))
           (lighter-val (or lighter (concat " " pretty)))
           (hook (intern (concat name "-hook")))
           (map-sym (intern (concat name "-map"))))
      `(progn
         ,@(when variable-defvar
             `((defvar ,variable ,init-value ,doc)))
         ,@(when keymap `((defvar ,map-sym ,keymap)))
         (defun ,mode (&optional arg)
           ,doc
           (interactive (list (or current-prefix-arg 'toggle)))
           ,(if variable-setter
                `(funcall ,variable-setter
                          (cond ((eq arg 'toggle) (not ,variable))
                                ((null arg) t)
                                ((and (consp arg)
                                      (eq (car arg) 'toggle))
                                 (not ,variable))
                                (t (> (prefix-numeric-value arg) 0))))
              `(setq ,variable
                     (cond ((eq arg 'toggle) (not ,variable))
                           ((null arg) t)
                           ((and (consp arg)
                                 (eq (car arg) 'toggle))
                            (not ,variable))
                           (t (> (prefix-numeric-value arg) 0)))))
           ,@body
           (run-hooks ',hook)
           (when (called-interactively-p 'interactive)
             (message "%s mode %s" ,pretty
                      (if ,variable "enabled" "disabled")))
           ,variable)
         (defvar ,hook nil)
         ,@(when global `((put ',mode 'global-minor-mode t)))
         ;; Register the lighter on minor-mode-alist.  GNU stores the
         ;; lighter spec verbatim: a string as-is, a form unevaluated
         ;; (it's a mode-line construct evaluated at display time).
         ;; For a GET-form :variable there is no variable key to file
         ;; under (GNU shows no alist entry for `scroll-bar-mode').
         ,@(when variable-defvar
             `((let ((cell (assq ',variable minor-mode-alist)))
                 (if cell
                     (setcdr cell ,(if (stringp lighter)
                                       `(list ,lighter-val)
                                     `(list ',lighter)))
                   (setq minor-mode-alist
                         (cons ,(if (stringp lighter)
                                    `(list ',variable ,lighter-val)
                                  `(list ',variable ',lighter))
                               minor-mode-alist))))))
         ;; GNU's `add-minor-mode' also records the keymap on
         ;; `minor-mode-map-alist' (front for new entries, in-place
         ;; cdr update for existing ones).
         ,@(when (and keymap variable-defvar)
             `((let ((cell (assq ',variable minor-mode-map-alist)))
                 (if cell
                     (setcdr cell ,map-sym)
                   (setq minor-mode-map-alist
                         (cons (cons ',variable ,map-sym)
                               minor-mode-map-alist))))))
         ',mode))))

(defmacro define-globalized-minor-mode (global-mode mode turn-on &rest keys)
  "Define a global minor mode GLOBAL-MODE corresponding to buffer-local MODE.
TURN-ON is a function or form run in each buffer to enable MODE."
  `(define-minor-mode ,global-mode
     ,(format "Toggle %s in all buffers." global-mode)
     :global t ,@keys
     (dolist (buf (buffer-list))
       (with-current-buffer buf
         (if ,global-mode
             ,(if (symbolp turn-on)
                  (list 'funcall (list 'quote turn-on))
                (list 'funcall turn-on))
           (,mode -1))))))

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
