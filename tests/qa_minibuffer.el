;; Minibuffer vars / keymaps / window — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - minibuffer-completion-table/-predicate/-confirm/-history are bound
;;     vars; minibuffer-completion-contents / -style / -auto-help are not
;;     in the dump (bound lazily).
;;   - minibuffer-local-filename-must-match-map is unbound in 31.1.
;;   - minibuffer-local-filename-completion-map is a keymap.
;;   - minibufferp returns t inside the minibuffer.
;;   - minibuffer-window is a live window.

;; == minibuffer vars (bound) ==
(qa (boundp 'minibuffer-completion-table) t)
(qa (boundp 'minibuffer-completion-predicate) t)
(qa (boundp 'minibuffer-completion-confirm) t)
(qa (boundp 'minibuffer-history) t)
(qa (boundp 'minibuffer-history-position) t)
(qa (boundp 'minibuffer-history-variable) t)
(qa (boundp 'minibuffer-history-sexp-flag) t)
(qa (boundp 'minibuffer-text-before-history) t)
(qa (boundp 'minibuffer-default) t)
(qa (boundp 'minibuffer-scroll-window) t)
(qa (boundp 'minibuffer-setup-hook) t)
(qa (boundp 'minibuffer-exit-hook) t)
(qa (boundp 'minibuffer-help-form) t)
(qa (boundp 'minibuffer-completion-auto-help) nil)
(qa (boundp 'minibuffer-message-min-height) nil)
(qa (boundp 'minibuffer-depth-indicate-mode) t)
(qa (boundp 'minibuffer-completion-contents) nil)
;; == minibuffer keymap vars ==
(qa (boundp 'minibuffer-local-map) t)
(qa (boundp 'minibuffer-local-completion-map) t)
(qa (boundp 'minibuffer-local-must-match-map) t)
(qa (boundp 'minibuffer-local-filename-completion-map) t)
(qa (boundp 'minibuffer-local-shell-command-map) t)
(qa (boundp 'minibuffer-local-ns-map) t)
;; minibuffer-local-filename-must-match-map is unbound
(qa (boundp 'minibuffer-local-filename-must-match-map) nil)

;; each bound map is a keymap
(qa (keymapp minibuffer-local-map) t)
(qa (keymapp minibuffer-local-completion-map) t)
(qa (keymapp minibuffer-local-must-match-map) t)
(qa (keymapp minibuffer-local-filename-completion-map) t)
(qa (keymapp minibuffer-local-shell-command-map) t)
(qa (keymapp minibuffer-local-isearch-map) t)

;; == minibufferp ==
(qa (fboundp 'minibufferp) t)
(qa (fboundp 'minibuffer-window) t)
(qa (fboundp 'minibuffer-selected-window) t)
(qa (fboundp 'minibuffer-contents) t)
(qa (fboundp 'minibuffer-contents-no-properties) t)
(qa (fboundp 'minibuffer-prompt) t)
(qa (fboundp 'minibuffer-prompt-end) t)
(qa (fboundp 'minibuffer-completion-contents) t)
;; nil for non-minibuffer
(with-temp-buffer
  (qa (minibufferp) nil))
;; t inside a minibuffer buffer
(let ((mb (get-buffer " *Minibuf-0*")))
  (qa (if mb (minibufferp mb) t) t))

;; == minibuffer-window ==
(qa (windowp (minibuffer-window)) t)
(qa (window-live-p (minibuffer-window)) t)
;; minibuffer-window-active-p fn
(qa (fboundp 'minibuffer-window-active-p) t)
;; minibuffer-depth
(qa (fboundp 'minibuffer-depth) t)
(qa (minibuffer-depth) 0)
;; minibuffer-window-buffer fn? (not in 31.1)
(qa (fboundp 'minibuffer-window-buffer) nil)

;; == minibuffer-prompt-width ==
(qa (fboundp 'minibuffer-prompt-width) t)

(princ (qa--done))
