;; read-* surface — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - Most read-* are fns; read-file-name-default and
;;     read-non-nil-coding-system are NOT.
;;   - multi-query-replace-regexp is not a fn.
;;   - read-buffer-function / read-buffer-completion-ignore-case /
;;     read-file-name-function / read-file-name-completion-ignore-case /
;;     read-regexp-defaults-function are bound vars.
;;   - read-file-name-predicate is NOT bound.

;; == read fns ==
(qa (fboundp 'read-string) t)
(qa (fboundp 'read-from-minibuffer) t)
(qa (fboundp 'read-regexp) t)
(qa (fboundp 'read-regexp-suggestions) t)
(qa (fboundp 'read-passwd) t)
(qa (fboundp 'read-number) t)
(qa (fboundp 'read-char) t)
(qa (fboundp 'read-char-choice) t)
(qa (fboundp 'read-char-exclusive) t)
(qa (fboundp 'read-char-from-minibuffer) t)
(qa (fboundp 'read-charset) t)
(qa (fboundp 'read-event) t)
(qa (fboundp 'read-key) t)
(qa (fboundp 'read-key-sequence) t)
(qa (fboundp 'read-key-sequence-vector) t)
(qa (fboundp 'read-answer) t)
(qa (fboundp 'read-answer-short) nil)
(qa (fboundp 'read-command) t)
(qa (fboundp 'read-variable) t)
(qa (fboundp 'read-function) nil)
(qa (fboundp 'read-buffer) t)
(qa (fboundp 'read-buffer-to-switch) t)
(qa (fboundp 'read-file-name) t)
(qa (fboundp 'read-file-name-default) t)
(qa (fboundp 'read-directory-name) t)
(qa (fboundp 'read-shell-command) t)
(qa (fboundp 'read-color) t)
(qa (fboundp 'read-fontset) nil)
(qa (fboundp 'read-coding-system) t)
(qa (fboundp 'read-non-nil-coding-system) t)

;; == read vars ==
(qa (boundp 'read-buffer-function) t)
(qa (boundp 'read-buffer-completion-ignore-case) t)
(qa (boundp 'read-file-name-function) t)
(qa (boundp 'read-file-name-completion-ignore-case) t)
(qa (boundp 'read-file-name-predicate) nil)
(qa (boundp 'minibuffer-allow-text-properties) t)
(qa (boundp 'minibuffer-auto-raise) t)
(qa (boundp 'minibuffer-prompt-properties) t)
(qa (boundp 'read-regexp-defaults-function) t)

;; == y-or-n / yes-or-no / map ==
(qa (fboundp 'y-or-n-p) t)
(qa (fboundp 'yes-or-no-p) t)
(qa (fboundp 'map-y-or-n-p) t)
(qa (fboundp 'multi-query-replace-regexp) nil)

;; == minibuffer-window-active-p / -window ==
(qa (fboundp 'minibuffer-window-active-p) t)

;; == substitute-command-keys bonus ==
;; \\[cmd] → "M-x cmd" if cmd isn't bound
(qa (substitute-command-keys "\\[no-such-command-xyz]") "M-x no-such-command-xyz")

;; == key-description on several shapes ==
(qa (key-description [?\t]) "TAB")
(qa (key-description [?\s]) "SPC")
(qa (key-description [?\r]) "RET")
(qa (key-description [?\e]) "ESC")
;; key-description on a single char → just that char
(qa (key-description [?a]) "a")

;; == current-active-maps / read-key vs key-binding ==
;; these are about the actual keybinding maps
(qa (fboundp 'current-active-maps) t)
(qa (listp (current-active-maps)) t)

;; == key-binding resolves through the global map ==
(qa (commandp (key-binding "\C-x\C-f")) t)
(qa (key-binding "\C-x\C-f") 'find-file)
;; unbound sequence → nil
(qa (key-binding "\C-cz") nil)
;; GNU returns all keybindings: [open] for menu, [24 6] for C-x C-f,
;; [menu-bar file new-file] for the menu item. Order may vary.
(let ((r (where-is-internal 'find-file)))
  ;; vectors need member (equal), not memq (eq)
  (qa (if (member [24 6] r) t nil) t)
  (qa (if (member [open] r) t nil) t)
  (qa (if (member [menu-bar file new-file] r) t nil) t))
;; == lookup-key / local-key-binding / minor-mode-key-binding ==
(qa (fboundp 'lookup-key) t)
(qa (fboundp 'local-key-binding) t)
(qa (fboundp 'global-key-binding) t)
(qa (fboundp 'minor-mode-key-binding) t)
;; minor-mode-key-binding returns an alist
(qa (listp (minor-mode-key-binding "a")) t)


(princ (qa--done))
