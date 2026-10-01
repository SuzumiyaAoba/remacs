;; Events / mouse / command vars — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - event-* / posn-* are fns; mouse fns exist in batch.
;;   - event-basic-type on C-a → 97 (the base char).
;;   - event-modifiers on C-a → '(control).
;;   - event-convert-list returns an event integer.
;;   - command vars are bound; single-command-keys/raw-keys are NOT bound.
;;   - Hooks (pre-command-hook etc.) are bound vars.
;;   - recent-keys exists as a var.
;;   - unread-command-events exists.

;; == event fns ==
(qa (fboundp 'event-start) t)
(qa (fboundp 'event-end) t)
(qa (fboundp 'event-basic-type) t)
(qa (fboundp 'event-modifiers) t)
(qa (fboundp 'event-convert-list) t)

;; == posn-* (mouse position accessors) ==
(qa (fboundp 'posn-window) t)
(qa (fboundp 'posn-point) t)
(qa (fboundp 'posn-col-row) t)
(qa (fboundp 'posn-actual-col-row) t)
(qa (fboundp 'posn-image) t)
(qa (fboundp 'posn-object) t)
(qa (fboundp 'posn-string) t)
(qa (fboundp 'posn-area) t)
(qa (fboundp 'posn-x-y) t)
(qa (fboundp 'posn-object-x-y) t)
(qa (fboundp 'posn-object-width-height) t)
(qa (fboundp 'posn-timestamp) t)

;; == mouse fns ==
(qa (fboundp 'mouse-pixel-position) t)
(qa (fboundp 'mouse-position) t)
(qa (fboundp 'set-mouse-position) t)
(qa (fboundp 'set-mouse-pixel-position) t)
(qa (fboundp 'track-mouse) t)

;; == command vars ==
(qa (boundp 'this-command) t)
(qa (boundp 'last-command) t)
(qa (boundp 'last-command-event) t)
(qa (boundp 'last-nonmenu-event) t)
(qa (boundp 'last-input-event) t)
(qa (boundp 'last-event-frame) t)
(qa (boundp 'real-last-command) t)
(qa (boundp 'this-single-command-keys) nil)
(qa (boundp 'this-single-command-raw-keys) nil)
(qa (boundp 'this-command-keys) nil)
(qa (boundp 'this-command-keys-vector) nil)
(qa (boundp 'recent-keys) nil)
(qa (boundp 'unread-command-events) t)
(qa (boundp 'unread-input-method-events) t)
(qa (boundp 'unread-post-input-method-events) t)
(qa (boundp 'keyboard-coding-system) t)
(qa (boundp 'keyboard-translate-table) t)
(qa (boundp 'num-input-keys) t)
(qa (boundp 'num-nonmacro-input-events) t)

;; == hooks (bound vars) ==
(qa (boundp 'pre-command-hook) t)
(qa (boundp 'post-command-hook) t)
(qa (boundp 'post-gc-hook) t)
(qa (boundp 'before-init-hook) t)
(qa (boundp 'after-init-hook) t)
(qa (boundp 'emacs-startup-hook) t)
(qa (boundp 'term-setup-hook) t)
(qa (boundp 'window-setup-hook) t)
(qa (boundp 'auto-save-hook) t)
(qa (boundp 'suspend-hook) t)
(qa (boundp 'suspend-resume-hook) t)
(qa (boundp 'kill-emacs-hook) t)
(qa (boundp 'kill-buffer-hook) t)
(qa (boundp 'change-major-mode-hook) t)
(qa (boundp 'first-change-hook) t)
(qa (boundp 'buffer-list-update-hook) t)

;; == event-modifiers / event-basic-type ==
;; on a real key event
(qa (event-modifiers (aref [?\C-a] 0)) '(control))
(qa (event-basic-type (aref [?\C-a] 0)) 97)
;; on a symbol event
(qa (event-modifiers 'control) nil)
(qa (event-basic-type 'control) 'control)
;; M-x has meta modifier
(qa (event-modifiers (aref [?\M-x] 0)) '(meta))

;; == event-convert-list ==
;; (control ?a) → C-a char (1)
(qa (event-convert-list '(control ?a)) 1)
;; (meta ?x) → M-x char
(qa (event-convert-list '(meta ?x)) ?\M-x)

;; == this-command-keys in batch ==
(qa (this-command-keys) "")
(qa (this-command-keys-vector) [])

(princ (qa--done))
