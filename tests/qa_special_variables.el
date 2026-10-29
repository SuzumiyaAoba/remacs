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

;; Alias chains are not limited to a fixed number of links.  In a malformed
;; cycle installed through `put', GNU indirect-variable returns its input.
(qa-lex
 (let* ((base (intern "qasp-alias-depth-0"))
        (last base))
   (set base 99)
   (dotimes (n 80)
     (setq last (intern (format "qasp-alias-depth-%d" (1+ n))))
     (defvaralias last (intern (format "qasp-alias-depth-%d" n))))
   (list (eq (indirect-variable last) base) (symbol-value last)))
 '(t 99))
(qa-lex
 (let* ((x (make-symbol "qasp-alias-cycle-x"))
        (a (make-symbol "qasp-alias-cycle-a"))
        (b (make-symbol "qasp-alias-cycle-b")))
   (put x 'variable-alias a)
   (put a 'variable-alias b)
   (put b 'variable-alias a)
   (list (eq (indirect-variable x) x)
         (eq (indirect-variable a) a)
         (eq (indirect-variable b) b)
         (boundp x)
         (condition-case nil (symbol-value x) (void-variable t))))
 '(t t t nil t))

;; GNU intentionally leaves `features' available for lexical binding.
(qa (special-variable-p 'features) nil)
(qa-lex (let ((features nil))
          (list features (featurep 'emacs)))
        '(nil t))
;; Default bindings of automatically buffer-local variables (#158).
(defvar-local qa158-variable 70)
(defvar qa158-events nil)
(defun qa158-watcher (symbol value operation where)
  (push (list value operation (and where t) (symbol-value symbol)) qa158-events))
(qa (let ((fill-column 10)) (with-temp-buffer (current-fill-column))) 10)
(qa (let ((fill-column 10))
      (list fill-column (default-value 'fill-column) (local-variable-p 'fill-column)
            (with-temp-buffer (list fill-column (current-fill-column) (local-variable-p 'fill-column)))))
    '(10 10 nil (10 10 nil)))
(qa (let ((qa158-variable 10))
      (list (default-value 'qa158-variable) (with-temp-buffer qa158-variable))) '(10 10))
(qa (with-temp-buffer
      (setq-local fill-column 20)
      (list (let ((fill-column 10))
              (list fill-column (default-value 'fill-column) (with-temp-buffer fill-column)))
            fill-column)) '((10 70 70) 20))
(qa (with-temp-buffer
      (let ((result (let ((fill-column 10))
                      (setq fill-column 11)
                      (list fill-column (default-value 'fill-column)
                            (with-temp-buffer fill-column) (local-variable-p 'fill-column)))))
        (list result fill-column (default-value 'fill-column) (local-variable-p 'fill-column))))
    '((11 11 11 nil) 70 70 nil))
(qa (let ((fill-column 10))
      (set 'fill-column 12)
      (list fill-column (default-value 'fill-column) (with-temp-buffer fill-column))) '(12 12 12))
(qa (let ((fill-column 10))
      (set-default 'fill-column 13)
      (list fill-column (default-value 'fill-column) (with-temp-buffer fill-column))) '(13 13 13))
(qa (let ((fill-column 10))
      (list (with-temp-buffer
              (setq fill-column 11)
              (list fill-column (default-value 'fill-column) (local-variable-p 'fill-column)))
            fill-column (default-value 'fill-column))) '((11 10 t) 10 10))
(qa (with-temp-buffer
      (let ((other (current-buffer)))
        (setq-local fill-column 20)
        (with-temp-buffer
          (let ((fill-column 10))
            (list (with-current-buffer other (setq fill-column 21) fill-column)
                  fill-column (default-value 'fill-column)))))) '(21 10 10))
(qa (with-temp-buffer
      (let ((inside (let ((fill-column 10))
                      (make-local-variable 'fill-column)
                      (setq fill-column 30)
                      (list fill-column (default-value 'fill-column)
                            (with-temp-buffer fill-column)))))
        (list inside fill-column (default-value 'fill-column)))) '((30 10 10) 30 70))
(qa (let ((fill-column 10))
      (list fill-column (let ((fill-column 20)) (with-temp-buffer fill-column))
            (with-temp-buffer fill-column))) '(10 20 10))
(qa (let ((fill-column 10))
      (with-temp-buffer
        (setq-local fill-column 30)
        (list (let ((fill-column 40))
                (list fill-column (default-value 'fill-column) (with-temp-buffer fill-column)))
              fill-column))) '((40 10 10) 30))
(qa (progn (catch 'qa158-exit (let ((fill-column 10)) (throw 'qa158-exit nil)))
           (list fill-column (default-value 'fill-column) (local-variable-p 'fill-column))) '(70 70 nil))
(qa (progn (condition-case nil (let ((fill-column 10)) (error "exit")) (error nil))
           (list fill-column (default-value 'fill-column))) '(70 70))
(defvaralias 'qa158-alias 'qa158-variable)
(qa (let ((qa158-alias 12))
      (list qa158-variable (default-value 'qa158-variable)
            (with-temp-buffer qa158-variable))) '(12 12 12))
(qa-lex (let ((qa158-variable 10)) (with-temp-buffer qa158-variable)) 10)
(qa (let ((major-mode 'qa158-mode) (buffer-read-only t))
      (with-temp-buffer (list major-mode buffer-read-only))) '(fundamental-mode nil))
(qa (let ((buffer-file-name "qa158-file"))
      (with-temp-buffer buffer-file-name)) nil)
(qa (let ((qa158-events nil))
      (unwind-protect
          (progn (add-variable-watcher 'fill-column 'qa158-watcher)
                 (let ((fill-column 10)) (setq fill-column 11))
                 (nreverse qa158-events))
        (remove-variable-watcher 'fill-column 'qa158-watcher)))
    '((10 set nil 70) (11 set t 10) (11 set nil 10) (70 set nil 11)))
(qa (let ((qa158-events nil))
      (unwind-protect
          (progn (add-variable-watcher 'qa158-variable 'qa158-watcher)
                 (let ((qa158-variable 10)) (setq qa158-variable 11))
                 (nreverse qa158-events))
        (remove-variable-watcher 'qa158-variable 'qa158-watcher)))
    '((10 let t 70) (11 set t 10) (70 set nil 11)))
(defvar-local qa158-changing 70)
(defun qa158-change-default (symbol value operation _where)
  (when (and (eq operation 'let) (eq value 10)) (set-default symbol 77)))
(qa (unwind-protect
        (progn (add-variable-watcher 'qa158-changing 'qa158-change-default)
               (list (let ((qa158-changing 10)) qa158-changing)
                     (default-value 'qa158-changing)))
      (remove-variable-watcher 'qa158-changing 'qa158-change-default)) '(10 70))
(defun qa158-fail-default (symbol value operation _where)
  (when (and (eq operation 'let) (eq value 10))
    (set-default symbol 77) (error "binding aborted")))
(qa (unwind-protect
        (progn (add-variable-watcher 'qa158-changing 'qa158-fail-default)
               (condition-case nil (let ((qa158-changing 10)) nil) (error nil))
               (list (default-value 'qa158-changing)
                     (with-temp-buffer qa158-changing)))
      (remove-variable-watcher 'qa158-changing 'qa158-fail-default)) '(70 70))
(princ (qa--done))
