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
'toggle' toggles explicitly."
  (let* ((init-value (plist-get args :init-value))
         (lighter (plist-get args :lighter))
         (keymap (plist-get args :keymap))
         (global (plist-get args :global))
         (variable (or (plist-get args :variable) mode))
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
       ',mode)))

(provide 'easy-mmode)
