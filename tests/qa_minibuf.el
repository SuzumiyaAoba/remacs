;; Minibuffer / read-family — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - minibuffer-depth at top level → 0; active-minibuffer-window → nil.
;;   - format-mode-line returns "" for ANY spec in batch.
;;   - minibuffer-local-map is a VAR (not a fn).
;;   - read-function / minibuffer-keyboard-quit are NOT fns in 31.1.
;;   - minibuffer-completing-symbol is NOT a var; -file-name is.
;;   - read-string-completion-ignore-case / minibuffer-regexp-predicates
;;     are NOT vars.
;;   - completing-read-function default = 'completing-read-default.
;;   - try-completion "" nil → nil; "al" on (alpha alps) → "alp" (LCP).
;;   - completion-boundaries "al" nil "x" → (0 . 1).

;; == fboundp inventory ==
(qa (fboundp 'read-from-minibuffer) t)
(qa (fboundp 'read-string) t)
(qa (fboundp 'read-buffer) t)
(qa (fboundp 'read-variable) t)
(qa (fboundp 'read-command) t)
(qa (fboundp 'read-function) nil)
(qa (fboundp 'completing-read) t)
(qa (fboundp 'minibuffer-contents) t)
(qa (fboundp 'minibuffer-contents-no-properties) t)
(qa (fboundp 'minibuffer-prompt-end) t)
(qa (fboundp 'minibuffer-prompt-width) t)
(qa (fboundp 'minibuffer-message) t)
(qa (fboundp 'minibuffer-quit-recursive-edit) t)
(qa (fboundp 'abort-recursive-edit) t)
(qa (fboundp 'top-level) t)
(qa (fboundp 'minibufferp) t)
(qa (fboundp 'active-minibuffer-window) t)
(qa (fboundp 'minibuffer-window) t)
(qa (fboundp 'minibuffer-selected-window) t)
(qa (fboundp 'minibuffer-prompt) t)
(qa (fboundp 'minibuffer-mode) t)
(qa (fboundp 'minibuffer-inactive-mode) t)
(qa (fboundp 'internal-complete-buffer) t)
(qa (fboundp 'window-minibuffer-p) t)
(qa (fboundp 'minibuffer-window-active-p) t)
(qa (fboundp 'minibuffer-with-setup-hook) t)
(qa (fboundp 'minibuffer-local-map) nil)
(qa (boundp 'minibuffer-local-map) t)

;; == minibuffer-completion-* vars ==
(qa (boundp 'minibuffer-completion-table) t)
(qa (boundp 'minibuffer-completion-predicate) t)
(qa (boundp 'minibuffer-completion-confirm) t)
(qa (boundp 'minibuffer-completing-file-name) t)
(qa (boundp 'minibuffer-completing-symbol) nil)

;; == minibuffer-depth ==
(qa (minibuffer-depth) 0)
(qa (active-minibuffer-window) nil)

;; == format-mode-line in batch → "" ==
(qa (format-mode-line "abc") "")
(qa (format-mode-line mode-line-format) "")
(qa (format-mode-line 'mode-name) "")

;; == completion-metadata ==
(qa (completion-metadata "ab" '(("abc") ("abd")) nil) '(metadata))
(qa (completion-metadata-get nil 'category) nil)
(qa (completion-metadata-get '(metadata (category . foo)) 'category) 'foo)

;; == try-completion ==
(qa (try-completion "x" nil) nil)
(qa (try-completion "" nil) nil)
(qa (try-completion "car" obarray) "car")
;; ambiguous prefix → longest common string
(qa (try-completion "al" '(("alpha") ("alps"))) "alp")
(qa (try-completion "" '(("alpha") ("alps"))) "alp")
(qa (try-completion "a" '(("alpha") ("beta"))) "alpha")
(qa (try-completion "z" '(("alpha") ("beta"))) nil)

;; == minibuffer-local-map keymaps ==
(qa (keymapp minibuffer-local-map) t)
(qa (keymapp minibuffer-local-completion-map) t)
(qa (keymapp minibuffer-local-must-match-map) t)
(qa (keymapp minibuffer-local-filename-completion-map) t)
(qa (keymapp minibuffer-local-ns-map) t)

;; == completing-read-function default ==
(qa (boundp 'completing-read-function) t)
(qa completing-read-function 'completing-read-default)

;; == minibuffer history vars ==
(qa (boundp 'minibuffer-history) t)
(qa (listp minibuffer-history) t)
(qa (boundp 'minibuffer-history-variable) t)
(qa (boundp 'minibuffer-history-position) t)

;; == minibuffer command fns ==
(qa (fboundp 'exit-minibuffer) t)
(qa (fboundp 'minibuffer-complete) t)
(qa (fboundp 'minibuffer-complete-word) t)
(qa (fboundp 'minibuffer-completion-help) t)
(qa (fboundp 'minibuffer-complete-and-exit) t)
(qa (fboundp 'self-insert-and-exit) t)
(qa (fboundp 'minibuffer-keyboard-quit) nil)
;; remacs: hash-table completion not implemented
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "alpha" 1 h)
      (test-completion "alpha" h))
    t)
(qa (let ((h (make-hash-table :test 'equal)))
      (puthash "alpha" 1 h)
      (test-completion "beta" h))
    nil)
(qa (boundp 'read-file-name-function) t)
(qa (boundp 'read-string-completion-ignore-case) nil)

;; == test-completion ==
(qa (test-completion "car" obarray) t)
(qa (test-completion "qa-no-such" obarray) nil)

;; remacs: all-completions on a sub-obarray returns nil
(qa (let ((o (make-vector 3 0)))
      (intern "abc" o)
      (intern "abd" o)
      (intern "xy" o)
      (sort (all-completions "a" o) #'string<))
    '("abc" "abd"))
(qa (let ((o (make-vector 3 0)))
      (intern "abc" o)
      (intern "abd" o)
      (intern "xy" o)
      (all-completions "x" o))
    '("xy"))

;; == completion-boundaries ==
(qa (completion-boundaries "alpha" '(("alpha")) nil "")
    '(0 . 0))
(qa (completion-boundaries "al" '(("alpha")) nil "x")
    '(0 . 1))

;; == minibuffer prop/regexp vars ==
(qa (boundp 'minibuffer-prompt-properties) t)
(qa (boundp 'minibuffer-allow-text-properties) t)
(qa (boundp 'minibuffer-auto-raise) t)
(qa (boundp 'minibuffer-regexp-predicates) nil)
(qa (boundp 'minibuffer-default-prompt-format) t)
(qa (boundp 'history-length) t)
(qa (boundp 'history-delete-duplicates) t)
(qa (boundp 'minibuffer-local-filename-syntax) t)
(qa (boundp 'minibuffer-local-shell-command-map) t)
(qa (boundp 'minibuffer-local-isearch-map) t)

;; == read-answer / read-char-choice / minibuffer-read fns ==
(qa (fboundp 'read-answer) t)
(qa (fboundp 'read-char-choice) t)
(qa (fboundp 'read-char-from-minibuffer) t)
(qa (fboundp 'y-or-n-p) t)
(qa (fboundp 'yes-or-no-p) t)
(qa (fboundp 'read-key) t)
(qa (fboundp 'read-key-sequence) t)
(qa (fboundp 'read-key-sequence-vector) t)
(qa (fboundp 'read-event) t)
(qa (fboundp 'read-char) t)
(qa (fboundp 'read-char-exclusive) t)

(princ (qa--done))
