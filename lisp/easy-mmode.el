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
           (variable (or (plist-get kw :variable) mode))
           (name (symbol-name mode))
           (pretty (capitalize
                    (if (string-suffix-p "-mode" name)
                        (substring name 0 -5)
                      name)))
           (lighter-val (or lighter (concat " " pretty)))
           (hook (intern (concat name "-hook")))
           (map-sym (intern (concat name "-map"))))
      `(progn
         (defvar ,variable ,init-value ,doc)
         (defvar ,hook nil)
         ,@(when keymap `((defvar ,map-sym ,keymap)))
         (defun ,mode (&optional arg)
           ,doc
           (interactive (list (or current-prefix-arg 'toggle)))
           (setq ,variable
                 (cond ((eq arg 'toggle) (not ,variable))
                       ((null arg) t)
                       ((and (consp arg)
                             (eq (car arg) 'toggle))
                        (not ,variable))
                       (t (> (prefix-numeric-value arg) 0))))
           ,@body
           (run-hooks ',hook)
           (when (called-interactively-p 'interactive)
             (message "%s mode %s" ,pretty
                      (if ,variable "enabled" "disabled")))
           ,variable)
         ,@(when global `((put ',mode 'global-minor-mode t)))
         ;; Register the lighter on minor-mode-alist.
         (let ((cell (assq ',variable minor-mode-alist)))
           (if cell
               (setcdr cell (list ,lighter-val))
             (setq minor-mode-alist
                   (cons (list ',variable ,lighter-val) minor-mode-alist))))
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

(provide 'easy-mmode)
