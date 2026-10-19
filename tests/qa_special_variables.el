;;; -*- lexical-binding: t; -*-
;; Issue #1: C/dump defaults and obsolete aliases remain dynamically scoped.
(qa (delq nil
          (mapcar (lambda (s) (unless (special-variable-p s) s))
                  '(after-focus-change-function clear-message-function
                    clone-indirect-buffer-hook comp-enable-subr-trampolines
                    display-mm-dimensions-alist edit-abbrevs-map
                    eldoc-documentation-function emacs-bzr-version
                    even-window-heights find-file-hooks focus-in-hook
                    focus-out-hook fringe-cursor-alist fringe-indicator-alist
                    help-enable-auto-load indicate-unused-lines
                    inhibit-nul-byte-detection inhibit-splash-screen
                    internal--get-default-lexical-binding-function
                    isearch-lazy-highlight-word isearch-new-word isearch-word
                    jit-lock-defer-contextually last-input-event
                    latin-extra-code-table lisp-font-lock-keywords
                    lisp-font-lock-keywords-1 lisp-font-lock-keywords-2
                    messages-buffer-max-lines minibuffer-mode-map
                    native-comp-deferred-compilation ns-version-string
                    overriding-terminal-local-map quit-flag
                    read-process-output-max real-last-command
                    region-extract-function revert-buffer-in-progress-p
                    scroll-step set-message-function standard-display-table
                    text-quoting-style throw-on-input
                    truncate-partial-width-windows
                    uniquify-after-kill-buffer-p uniquify-trailing-separator-p
                    window-safe-min-height window-safe-min-width
                    write-file-hooks)))
    nil)

(qa-lex (let ((scroll-step 5)) (symbol-value 'scroll-step)) 5)
(qa-lex (let ((read-process-output-max 1234))
          (symbol-value 'read-process-output-max))
        1234)
(qa-lex (let ((text-quoting-style 'grave)) (format-message "`foo'"))
        "`foo'")
(qa-lex (let ((find-file-hooks '(qa-special-hook)))
          (symbol-value 'find-file-hook))
        '(qa-special-hook))
(qa-lex (let ((write-file-hooks '(qa-special-hook)))
          (symbol-value 'write-file-functions))
        '(qa-special-hook))

;; Alias creation marks both previously undeclared names special.
(defvaralias 'qa-special-alias 'qa-special-base)
(qa (list (special-variable-p 'qa-special-alias)
          (special-variable-p 'qa-special-base))
    '(t t))
(setq qa-special-base 'outer)
(qa-lex (let ((qa-special-alias 'inner)) qa-special-base) 'inner)
(qa qa-special-base 'outer)
(qa (let ((qa-special-alias 'dynamic)) (symbol-value 'qa-special-base))
    'dynamic)
(qa-lex (catch 'qa-special-exit
          (let ((qa-special-alias 'inner))
            (throw 'qa-special-exit qa-special-base)))
        'inner)
(qa qa-special-base 'outer)
(defvaralias 'qa-special-tab-alias 'tab-width)
(qa-lex (with-temp-buffer
          (setq-local tab-width 3)
          (list (let ((qa-special-tab-alias 7)) tab-width)
                tab-width))
        '(7 3))
(qa-lex (let* ((qa-special-alias 'inner)
               (qa-special-base 'innermost))
          (list qa-special-alias (symbol-value 'qa-special-base)))
        '(innermost innermost))
(qa qa-special-base 'outer)

;; GNU intentionally leaves `features' available for lexical binding.
(qa (special-variable-p 'features) nil)
(qa-lex (let ((features nil))
          (list features (featurep 'emacs)))
        '(nil t))
(princ (qa--done))
