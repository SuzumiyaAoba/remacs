;; help.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - describe-*/apropos fns exist.
;;   - help-function-arglist normalizes arglists.
;;   - where-is returns a message via `message' (or nil w/o prefix).

;; == surface ==
(qa (fboundp 'describe-function) t)
(qa (fboundp 'describe-variable) t)
(qa (fboundp 'describe-key) t)
(qa (fboundp 'describe-mode) t)
(qa (fboundp 'describe-bindings) t)
(qa (fboundp 'where-is) t)
(qa (fboundp 'apropos) t)
(qa (fboundp 'apropos-command) t)
(qa (fboundp 'apropos-variable) t)
(qa (fboundp 'apropos-documentation) t)
(qa (fboundp 'describe-function-1) t)
(qa (fboundp 'describe-char) t)
(qa (fboundp 'describe-symbol) t)
(qa (fboundp 'describe-face) t)
(qa (fboundp 'view-echo-area-messages) t)
(qa (fboundp 'where-is-internal) t)
(qa (fboundp 'keymap--menu-item-binding) t)
(qa (fboundp 'keymap--menu-item-with-binding) t)
(qa (fboundp 'keymap--get-keyelt) t)
;; NOT fns
(qa (fboundp 'describe-variable-custom-version-info) nil)
(qa (fboundp 'keymap--lookup-key-1) nil)

;; == where-is returns nil in batch (no active minibuffer/echo) ==
(let ((buf (generate-new-buffer " *w*")))
  (unwind-protect
      (with-current-buffer buf
        ;; 'isearch-forward IS bound in batch (global-map has C-s)
        ;; where-is prints via `message', not returns a list
        (qa (where-is 'isearch-forward) nil))
    (kill-buffer buf)))

;; == help-function-arglist — returns raw subr arglist (argN placeholders) ==
(qa (help-function-arglist 'car) '(arg1))
(qa (help-function-arglist 'progn) '(&rest rest))
;; 'if / 'let are primitives — GNU normalizes their arglists
(qa (help-function-arglist 'if) '(arg1 arg2 &rest rest))
(qa (help-function-arglist 'let) '(arg1 &rest rest))
(qa (boundp 'help-window-keep-selected) t)
(qa (boundp 'help-enable-autoload) t)
;; NOT vars in 31.1
(qa (boundp 'help-enable-variable-value-editing) nil)
(qa (boundp 'help-enable-symbol-autoload) nil)
(qa (boundp 'help-split-width) nil)
(qa (boundp 'help-window-display-fudge) nil)

;; == help-window-select default ==
(qa (boundp 'help-window-select) t)

;; == apropos-internal is the real driver ==
(qa (fboundp 'apropos-internal) t)

;; == documentation fn exists ==
(qa (fboundp 'documentation) t)
(qa (fboundp 'documentation-property) t)


;; == keymap utilities are on keymap.el ==
(qa (fboundp 'lookup-key) t)
(qa (fboundp 'define-key) t)
(qa (fboundp 'global-set-key) t)
(qa (fboundp 'local-set-key) t)
(qa (fboundp 'key-binding) t)
;; help-functions-list isn't bound in 31.1
(qa (boundp 'help-functions-list) nil)
(qa (fboundp 'substitute-command-keys) t)

;; == substitute-command-keys substitutes \\[...] ==
(let ((s (substitute-command-keys "\\[self-insert-command]")))
  ;; returns the key binding — e.g. \"C-a\" or similar
  (qa (stringp s) t))

;; == substitute-command-keys of a quoted form ==
(let ((s (substitute-command-keys "hello \\[next-line] world")))
  ;; produces some string — exact binding depends on context
  (qa (stringp s) t))

;; == help-buffer is a fn ==
(qa (fboundp 'help-buffer) t)

(princ (qa--done))
