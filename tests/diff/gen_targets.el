;;; Emit the differential test target list on stdout, one per line:
;;;   NAME-PRIN1|MIN|MAX        (MAX is "many" for rest-args subrs)
;;; Run under GNU Emacs -Q --batch -l this-file -f d--emit.
;;; Only subrs that are NOT special forms are listed; special forms
;;; cannot be applied.
(defvar d--emit-skip
  '(
    ;; Process exit / terminal death / hangs
    kill-emacs save-buffers-kill-emacs save-buffers-kill-terminal
    abort-recursive-edit top-level throw--to-debug
    recursive-edit sleep-for sit-for
    read-event read-char read-char-exclusive read-key
    read-from-minibuffer read-string read-password
    y-or-n-p yes-or-no-p map-y-or-n-p
    x-popup-menu x-menu-bar-open-internal
    read-file-name completing-read read-buffer
    ;; External processes / filesystem mutation
    call-process call-process-region start-process start-file-process
    make-process make-network-process make-pipe-process
    open-network-stream set-network-process-option
    process-send-string process-send-region process-send-eof
    accept-process-output
    delete-file rename-file copy-file add-name-to-file make-symbolic-link
    make-directory delete-directory set-file-modes set-file-times
    set-file-selinux-context copy-directory move-file-to-trash
    write-region append-to-file write-abbrev-file
    lock-buffer unlock-file file-modes
    ;; System interaction / environment
    suspend-emacs suspend-tty resume-tty set-time-zone-rule
    browse-url x-disown-selection-internal x-own-selection-internal
    x-store-cut-buffer-internal x-get-cut-buffer-internal
    x-rotate-cut-buffers-internal x-get-selection-internal
    x-register-dnd-atom x-begin-drag x-send-client-message
    emacs-session-restore emacs-session-save
    ;; Timing / nondeterminism
    current-time current-time-string current-time-zone
    encode-time decode-time float-time time-convert
    emacs-uptime emacs-init-time emacs-build-time emacs-version
    recent-keys random
    ;; Global mutation that corrupts the driver itself
    kill-buffer bury-buffer fset defalias defvaralias
    put setplist set symbol-function
    fmakunbound makunbound unintern
    define-key keymap-set
    use-global-map use-local-map
    provide require load autoload autoload-do-load
    ;; Dumped state
    dump-emacs pdumper-stats
    ;; Interactive machinery
    execute-extended-command call-interactively
    command-execute keyboard-quit minibuffer-quit-recursive-edit
    exit-recursive-edit abort-if-not-in-minibuffer
    ;; Debugger / diagnostics that diverge by design
    debug backtrace backtrace-eval backtrace-frame backtrace-debug
    mapbacktrace error-message-string
    ;; UI surface that can hang without a display
    x-create-frame make-frame make-terminal-frame
    Electric-command-loop Electric-pop-up-window
    open-termscript send-string-to-terminal ding
    ;; External libs (env-dependent)
    dbus--init dbus-call-method dbus-register-signal
    ;; GNU aborts (SIGABRT) on malformed args to these internals
    comp--register-lambda comp--register-subr comp--late-register-subr
    comp--install-trampoline comp--compile-ctxt-to-file0
    dump-emacs-portable dump-emacs-portable--sort-predicate
    get-variable-watchers internal--define-uninitialized-variable
    window-discard-buffer-from-window
    ;; Block indefinitely (event loop / stdin even with /dev/null)
    cl--defmethod-doc-pos event-apply-alt-modifier
    tool-bar-event-apply-alt-modifier lisp--match-hidden-arg
    modifier-bar-button quoted-insert restart-emacs
    save-some-buffers help-command-error-confusable-suggestions
    Snarf-documentation
    ;; GNU aborts or hangs mid-slice (sweep killers observed in v3):
    ;; command-error-default-function aborts (SIGABRT) when called
    ;; with random args; isearch/help/mouse readers block on input.
    command-error-default-function custom-initialize-after-file-load
    isearch-describe-key help--read-key-sequence mark-defun
    mouse-drag-secondary read--potential-mouse-event
    scroll-bar-horizontal-drag tool-bar-event-apply-control-modifier
    event-apply-control-modifier indent-accumulate-tab-stops
    isearch-char-by-name window-size-fixed-p
    ;; stderr/stdout redirection clobbers the marker protocol
    redirect-debugging-output
    ;; GNU aborts on missing eln paths / palette without a display
    comp-el-to-eln-filename ns-do-show-character-palette
    event-apply-shift-modifier help-for-help isearch-quote-char
    read-quoted-char tool-bar-event-apply-hyper-modifier
    electric-indent-just-newline
    ;; number-sequence materializes a huge list on big ranges —
    ;; both sides grind to OOM/timeout, so corpus calls are useless.
    number-sequence
    ))

(defun d--emit ()
  (let (names)
    (mapatoms
     (lambda (s)
       (when (and (fboundp s)
                  (subrp (symbol-function s))
                  (not (special-form-p s))
                  (not (memq s d--emit-skip)))
         (push s names))))
    (dolist (s (sort names (lambda (a b) (string< (symbol-name a)
                                                  (symbol-name b)))))
      (let* ((ar (subr-arity (indirect-function s)))
             (min (car ar))
             (max (cdr ar)))
        (princ (format "%S|%s|%s\n" (symbol-name s) min
                       (if (eq max 'many) "many" (format "%s" max))))))
    (terpri)))

(provide 'diff-gen-targets)
