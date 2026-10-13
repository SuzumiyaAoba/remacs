;;; -*- lexical-binding: t -*-
;;; tests/diff/common.el --- shared helpers for differential drivers
;; Loaded by driver.el and driver-sf.el.  Same file must run under both
;; GNU `emacs -Q --batch' and `remacs --batch'.

(defvar dif--targets nil)
(defvar dif--corpus nil
  "The corpus vector in use — needed by `dif--restore-cells' inside
`dif--call', which can't see the driver's lexical bindings.")
(defvar dif--forms nil)
(defvar dif--fxbuf nil)

;;; ---- tunables ---------------------------------------------------------

;; Names never called (exit/hang/side-effectful outside the sandbox or
;; environment-dependent by definition).
(defvar dif--skip
  '("throw" "top-level" "exit-minibuffer" "minibuffer-quit-recursive-edit"
    "recursive-edit" "kill-emacs" "save-buffers-kill-emacs"
    "save-buffers-kill-terminal" "suspend-emacs" "suspend-tty" "resume-tty"
    "dump-emacs" "pdumper-stats"
    ;; Wall-clock payloads: (TIME GC GCTIME) results can't match.
    ;; remacs' compiled-lambda trampolines also skip executing the body,
    ;; so voids/errors GNU hits inside the callee never fire.
    "benchmark" "benchmark-run" "benchmark-run-compiled" "benchmark-progn"
    "benchmark-elapse" "benchmark-call"
    "debug" "edebug" "debug-on-entry" "cancel-debug-on-entry"
    "debug-on-message" "backtrace" "backtrace--frames" "backtrace-debug"
    "backtrace-frame" "mapbacktrace" "internal--backtrace-*"
    "sleep-for" "sit-for" "accept-process-output" "wait-for"
    "read-char" "read-char-exclusive" "read-event" "read-key"
    "read-key-sequence" "read-key-sequence-vector" "read-quoted-char"
    "read-from-minibuffer" "read-no-blanks-input" "read-string"
    "read-buffer" "read-file-name" "read-directory-name" "read-color"
    "read-coding-system" "read-command" "read-function" "read-variable"
    "read-symbol" "read-charset" "read-char-by-name" "read-face-name"
    "read-face-font" "read-envvar-name" "read-file-modes" "read-number"
    "read-passwd" "read-regexp" "read-regexp-suggestions"
    "read-shell-command" "read-extended-command" "read-multilingual-string"
    "sleep-for" "sit-for" "accept-process-output" "wait-for"
    "normal-top-level" "normal-top-level-1" "command-line"
    "command-line-1" "handle-args-interface" "emacs-session-restore"
    "test-completion" "kbd"
    "start-process" "start-process-shell-command" "start-file-process"
    "start-file-process-shell-command" "make-process" "pipe-process"
    "make-network-process" "open-network-stream" "make-serial-process"
    "serial-process-configure" "server-eval-at" "server-eval-and-print"
    "make-temp-file" "make-temp-name" "file-local-copy"
    "open-termscript" "open-dribble-file" "set-terminal-local-value"
    "fset" "defalias" "fmakunbound" "makunbound" "setplist" "unintern"
    "make-closure" "gensym" "gentemp"
    "make-thread" "thread-join" "thread-yield" "thread-signal"
    "thread-last-error" "thread--blocker" "thread-alive-p"
    "main-thread" "current-thread" "all-threads" "thread-name"
    "make-mutex" "mutex-lock" "mutex-unlock" "condition-wait"
    "condition-notify" "make-condition-variable" "with-mutex"
    "native-compile" "native-compile-async" "native-compile-directory"
    "native-compile-prune-cache" "batch-native-compile"
    "batch-byte-compile" "byte-compile" "compile-defun"
    "comp--install-trampoline" "comp--compile-ctxt-to-file"
    "garbage-collect-messages"
    ;; process signals can reach pids/groups — corpus ints are small, but
    ;; don't risk it.
    "signal-process"
    ;; tty/terminal writers with side effects outside stdout
    "send-string-to-terminal"
    ;; debugger entry points that abort batch runs
    "early-init-file" "init-file-user"
    ;; Registry mutators: watchers persist globally and fire on every
    ;; later specbind/set of the target symbol — the firing itself is
    ;; correct, but registry state can't be restored cleanly mid-sweep.
    "add-variable-watcher" "remove-variable-watcher"
    "get-variable-watchers"
    ;; Lisp subrs that hang in --batch on the corpus (verified by
    ;; timeout on BOTH sides — GNU hangs identically, so these stay
    ;; consistent but burn the sweep's time budget).
    "cl--defmethod-doc-pos" "command-error-default-function"
    "help--read-key-sequence" "help-read-key-sequence"
    "internal-echo-keystrokes-prefix"
    ;; External side effects: launch the user's browser / mail client
    ;; / network fetch on BOTH sides.  Must never run unattended.
    "describe-gnu-project" "browse-url-at-point" "browse-url-at-mouse"
    "browse-url-of-buffer" "browse-url-of-region" "browse-url-of-file"
    "browse-url-of-dired-file" "browse-url-button-open-url"
    "dnd-handle-one-url" "dnd-handle-multiple-urls"
    "dnd-open-remote-url" "dnd-open-local-file" "dnd-open-file"
    "dnd-begin-drag-files" "dnd-begin-file-drag" "dnd-begin-text-drag"
    "dnd-handle-movement" "dnd-insert-text" "dnd-direct-save"
    "dnd-remove-last-dragged-remote-file"
    "eww" "eww-browse-url" "eww-search-words" "eww-follow-link"
    "define-mail-user-agent" "menu-bar-read-mail"
    "report-emacs-bug" "compose-mail" "compose-mail-other-window"
    "compose-mail-other-frame" "mail" "mail-other-window"
    "mail-other-frame" "send-mail-function" "url-retrieve"
    "url-retrieve-synchronously" "url-store-in-cache"
    "url-copy-file" "url-insert-file-contents" "url-download"
    "url-queue-retrieve" "webjump" "google-maps"
    "zone" "doctor" "life" "gomoku" "tetris" "pong" "snake"
    "bubble" "bubbles" "hanoi" "landmark" "blackbox" "solitaire"
    "multiplication" "animate-birthday-present" "butterfly"
    ;; Hang both sides in --batch on corpus args (read keys/prompt,
    ;; display tty frames, spawn subprocesses, exit).  Collected by
    ;; autoskip under GNU — consistent, so no signal lost.
    "Buffer-menu-isearch-buffers" "Buffer-menu-isearch-buffers-regexp"
    "Buffer-menu-mode" "buffer-menu" "buffer-menu-other-window"
    "bury-buffer" "completion-list-mode-finish" "describe-copying"
    "describe-distribution" "describe-no-warranty"
    "display-about-screen" "dump-emacs-portable--sort-predicate"
    "elisp-index-search" "emacs-index-search" "eval-defun"
    "event-apply-alt-modifier" "event-apply-control-modifier"
    "event-apply-hyper-modifier" "event-apply-meta-modifier"
    "frame--z-order-lessp" "help-command-error-confusable-suggestions"
    "help-for-help" "internal-face-x-get-resource" "kill-buffer"
    "kill-buffer-and-window" "kill-current-buffer" "menu-bar-read-mail"
    "menu-bar-select-buffer" "messages-buffer-mode"
    "mode-line-other-buffer" "mode-line-toggle-read-only"
    "modifier-bar-button" "mouse-drag-secondary" "next-buffer"
    "normal-splash-screen" "ns-do-show-character-palette"
    "ns-frame-edges" "ns-frame-geometry"
    "ns-mouse-absolute-pixel-position" "ns-open-file-service"
    "ns-set-mouse-absolute-pixel-position" "ns-show-character-palette"
    "occur-mode" "process-menu-mode" "query-replace"
    "quit-restore-window" "quit-window" "quit-windows-on"
    "replace-buffer-in-windows" "restart-emacs"
    ;; Writes to the user's real abbrev_defs file (outside the sandbox)
    ;; — GNU hits a file-lock error, remacs silently writes.  Real
    ;; side effect; skip.
    "abbrev-edit-save-buffer" "write-abbrev-file" "read-abbrev-file"
    "quietly-read-abbrev-file" "define-abbrevs"
    ;; the row is stable but uninformative.
    "save-some-buffers" "save-buffers-kill-terminal"
    ;; compiler-macro cells are stored as `*--cmacro' functions and
    ;; take (CL-WHOLE-ARG NAME...) — they are only callable through
    ;; the compiler-macro expansion path, not plain apply.
    "cl--make-slot-descriptor--cmacro" "cl--generic-make--cmacro"
    "cl--generic-make-method--cmacro" "cl-generic-make-generalizer--cmacro"
    "cl--struct-new-class--cmacro" "make-ppss--cmacro"
    "make-xref-elisp-location--cmacro" "make-byte-code--cmacro"
    "oclosure-lambda--cmacro" "eieio-oref--cmacro" "eieio-oset--cmacro"))

(defvar dif--skip-prefix
  '("w32-" "cygwin-" "msdos-" "haiku-" "android-" "pgtk-" "haiku"
    "dbus-" "sqlite" "native-comp-" "comp-" "xwidget" "profiler-"
    "file-notify-" "debug-early" "internal-module" "module-"
    "electric-" "Electric-"
    ;; batch input readers that hang on /dev/null stdin
    "isearch-" "ediff-" "epa--" "auth-source-pass"
    ;; argv-consumers: command-line-* mutate command-line-args-left /
    ;; re-enter startup and can quietly terminate the sweep.
    "command-line"))

(defvar dif--skip-input
  '("y-or-n-p" "yes-or-no-p" "read-char-choice" "map-y-or-n-p"
    "read-char-from-minibuffer" "read-answer" "info-lookup-interactive-arguments"
    ;; interactive char readers block on the tty/stdin in batch on
    ;; BOTH sides (verified: GNU `quoted-insert' timed out at 300 s)
    "quoted-insert"
    ;; batch stdin is /dev/null — GNU's `read' consumes the driver input
    ;; or hits eof mid-sweep; input-dependent by construction
    "read" "read--expression"))
;; Result normalized to (type-of ...) symbol only — inherently
;; environment/time/pid-dependent or self-describing.
(defvar dif--norm-type
  '("current-time" "current-time-string" "current-time-zone"
    "decode-time" "emacs-uptime" "emacs-init-time" "emacs-pid"
    "float-time" "emacs-version" "emacs-build-time"
    "system-configuration" "system-configuration-options"
    "system-configuration-features" "invocation-name"
    "invocation-directory" "features" "exec-path"
    "garbage-collect" "memory-report" "memory-info" "memory-limit"
    "malloc-info" "list-system-processes" "process-attributes"
    "user-real-uid" "user-uid" "user-full-name" "user-login-name"
    "group-gid" "group-real-gid" "system-name" "system-groups"
    "system-users" "file-attributes" "directory-files-and-attributes"
    "file-system-info" "file-system-info-..."
    ;; Wall-clock values.
    "frame-use-time" "backtrace-frames" "backtrace-frame"
    ;; Buffer-local alist payloads differ legitimately: GNU pre-seeds
    ;; mode vars (menu/case-fold/imenu/electric pairs, font-lock, ...)
    ;; while remacs tracks a smaller set and a different order.  Type
    ;; only.
    "buffer-local-variables" "buffer-local-toplevel-value"
    ;; `(random t)' reseeds from entropy — nondeterministic by design.
    ;; Seeded calls (string/int args) stay exact-compared.
    "random"))
;; note: file-system-info IS normalized by name above (harmless dup rule)

;; Skip only the zero-argument call for these (arg-ful forms may be
;; deterministic).
(defvar dif--skip-noarg
  '("current-time" "decode-time" "encode-time" "float-time"
    "current-time-string" "current-time-zone" "format-time-string"
    "emacs-uptime" "emacs-init-time" "emacs-pid" "recent-keys"
    "this-command-keys" "this-command-keys-vector" "this-single-command-keys"
    "this-single-command-raw-keys" "key-description" "random" "garbage-collect"
    "buffer-list" "process-list" "frame-list" "terminal-list"
    "coding-system-list" "charset-priority-list" "font-family-list"
    "defined-colors" "face-list" "tool-bar-lines-needed"))

;;; ---- corpus -----------------------------------------------------------

(defun dif--mk ()
  "Freshly evaluate the corpus forms, returning a vector of values."
  (setq dif--forms
        '(-1 0 1 2 3 7 10 42 65 97 255 256
          511 1000 1024 2048 4096 65535 65536 999999
          1073741823 1073741824 2147483647 -2147483648
          4294967295 4294967296 2147483648
          2305843009213693951 -2305843009213693952
          4611686018427387903 -4611686018427387904
          0.0 -0.0 1.5 -2.25 2.5e-1 1e10 100.0 -100.5
          3.14e300 1.0e-300 1.0e+INF -1.0e+INF 0.0e+NaN
          (most-positive-fixnum) (most-negative-fixnum) (max-char)
          ?a ?A ?0 ?\s ?\t ?\n ?\\ ?\x7f ?\x80 ?\xff ?あ ?\u00e9 ?\U0001F600
          ;; strings
          "" "a" "abc" "Hello" "hello world" "a b c" "a  b"
          "日本語" "日本語テスト" "中文测试" "한국어" "αβγ" "ΑΒΓ"
          "ｱｲｳ" "Ångström" "ﬁle" "العربية" "é"
          "\\" "a\\b" "a|b" "a\nb" "tab\there" "ctrl\x01\x02"
          "\x80\x81\xff" "\xc0\xc1" "\xe3\x81" "\xef\xbb\xbfutf8"
          "%s" "%s %d %x" "%-10s|%05.2f" "%d%%" "%3$c"
          "[a-z]+" "\\`" "\\'" "^$" "(" ")" "[" "]" ".*" "\\w+" "\\s-"
          "\\bfoo\\b" "^\\s-*" "[^a-z]" "\\([a-z]\\)*"
          ;; sandbox-relative filenames (fixture populated by difftest.py)
          "file-a.txt" "./file-a.txt" "sub" "./sub" "./sub/b.el"
          "link-a" "empty.bin" "bin.dat" "./nofile" ".." "." "~"
          "file-a.txt.gz" "dir/.hidden"
          ;; symbols
          t nil
          (quote car) (quote cdr) (quote cons) (quote mapcar)
          (quote ignore) (quote +) (quote -) (quote list)
          (quote vector) (quote append) (quote length) (quote nth)
          (quote format) (quote princ) (quote eval) (quote apply)
          (quote funcall) (quote symbol-value) (quote boundp)
          (quote fboundp) (quote setq) (quote let) (quote progn)
          (quote quote) (quote function) (quote lambda) (quote nil-nil)
          (quote error) (quote user-error) (quote search-failed)
          (quote arith-error) (quote file-error)
          (quote void-function) (quote void-variable)
          (quote wrong-type-argument) (quote end-of-buffer)
          (quote beginning-of-buffer) (quote buffer-read-only)
          (quote mark-inactive) (quote quit-flag)
          (quote a-symbol) (quote DIFF--sym) (quote test.foo)
          (quote x|y) (quote |\x01|) (quote bold) (quote italic)
          (quote default) (quote fixed-pitch) (quote variable-pitch)
          (quote mode-line) (quote minibuffer-prompt)
          (quote eq) (quote eql) (quote equal)
          (quote read) (quote write) (quote insert)
          (quote delete-char) (quote open-file) (quote write-file)
          (quote rename-file) (quote copy-file)
          (quote forward-char) (quote backward-char)
          (quote beginning-of-line) (quote end-of-line)
          (quote l) (quote r) (quote o) (quote m) (quote in)
          (quote strict)
          (quote utf-8) (quote utf-8-unix) (quote utf-8-dos)
          (quote utf-16) (quote utf-16le) (quote utf-16be)
          (quote iso-8859-1) (quote latin-1) (quote us-ascii)
          (quote ascii) (quote binary) (quote raw-text)
          (quote no-conversion) (quote emacs-mule)
          (quote japanese-cp932) (quote chinese-gb18030)
          (quote undecided) (quote undecided-unix) (quote japanese)
          (quote chinese-gb2312) (quote latin-2) (quote unicode)
          (quote eight-bit) (quote compound-text)
          (quote japanese-shift-jis)
          (quote prefix) (quote fill-prefix) (quote tab-width)
          :size :test :weakness :rehash-size :name :file :coding
          :type :eof :coding-system :mustbenew :visit :directory
          ;; conses (quoted — never eval the payload)
          (quote (a)) (quote (a b)) (quote (a b c)) (quote (1 2 3))
          (quote ("x" "y" "z")) (quote (a . b)) (quote (1 . 2))
          (quote ((a . 1) (b . 2) (c . 3))) (quote ((x . "1") (y . "2")))
          (quote (nil)) (quote (t)) (quote ((a))) (quote (((b))))
          (quote (a (b c) d)) (quote (0 1 2 3 4 5 6 7 8 9))
          (quote (:k v)) (quote (k v k2 v2)) (quote (a 1 b 2 c 3))
          (quote (nil nil)) (quote (t t))
          ;; vectors
          (vector) (vector (quote a)) (vector 1 2 3) (vector "x" "y")
          (vector 1 "two" (quote three) (list 4) (vector 5))
          ;; bool vectors / tables / keymaps / objects
          (make-bool-vector 5 t) (make-bool-vector 3 nil)
          (bool-vector t nil t) (make-hash-table)
          (make-hash-table :test (quote equal))
          (make-hash-table :test (quote eq) :size 65)
          (let ((h (make-hash-table :test (quote equal))))
            (puthash "a" 1 h) (puthash "b" 2 h) (puthash (quote s) 3 h) h)
          (get-buffer-create " dfx") (current-buffer) dif--fxbuf
          (make-marker) (with-current-buffer dif--fxbuf (copy-marker 1))
          (with-current-buffer dif--fxbuf (copy-marker 10))
          (with-current-buffer dif--fxbuf (copy-marker 25 t))
          (with-current-buffer dif--fxbuf (copy-marker 40))
          (make-keymap) (make-sparse-keymap)
          (copy-keymap (current-global-map)) (current-global-map)
          (standard-syntax-table) (copy-syntax-table)
          (standard-case-table) (make-char-table (quote difcat) nil)
          (make-char-table (quote difcat2) 7)
          (condition-case nil (standard-category-table) (error nil))
          (list (quote lambda) (quote (x)) (quote (+ x 1)))
          (function (lambda (x) (+ x 1)))
          (symbol-function (quote car)) (symbol-function (quote +))
          (selected-window) (selected-frame) (minibuffer-window)))
  (let* ((n (length dif--forms))
         (v (make-vector n nil)))
    (dotimes (i n)
      (aset v i (condition-case nil (dif--fresh (eval (nth i dif--forms)))
                  (error (list 'dif--corpus-err i))))
      ;; Emit each element's normalized form so corpus parity itself is
      ;; a comparable row (index k => corpus[k] identical on both sides).
      (dif--emit "ZZcorpus" i 'ok (dif--norm (aref v i))))
    v))

(defun dif--fresh (v)
  "Deep-copy the string/cons/vector skeleton of V.
Corpus literals are shared with this file's code, so a destructive
callee would otherwise mutate them for every later target."
  (cond ((stringp v) (copy-sequence v))
        ((consp v) (cons (dif--fresh (car v)) (dif--fresh (cdr v))))
        ((vectorp v) (apply #'vector (mapcar #'dif--fresh v)))
        (t v)))

;;; ---- value normalization ----------------------------------------------

(defvar dif--have-terminalp (fboundp 'terminalp))
(defvar dif--have-recordp (fboundp 'recordp))
(defvar dif--have-categoryp (fboundp 'category-table-p))
(defvar dif--have-oclosurep (fboundp 'oclosurep))
(defvar dif--have-autoloadp (fboundp 'autoloadp))

;;; ---- value normalization ----------------------------------------------

(defun dif--walk (v depth)
  "Deep-normalize V: opaque objects collapse to tags, containers
recurse, everything else is returned as-is for prin1."
  (cond
   ((>= depth 8) (if (atom v) v '…))
   ((markerp v) (list 'marker (marker-position v) (not (not (marker-buffer v)))))
   ((bufferp v) (list 'buf (buffer-name v)))
   ((overlayp v) (list 'ov (overlay-start v) (overlay-end v)))
   ((processp v) (list 'proc (process-name v)))
   ((windowp v) (list 'window (not (not (window-buffer v)))))
   ((framep v) '(frame))
   ((and dif--have-terminalp (terminalp v)) '(terminal))
   ((hash-table-p v) (list 'hash (hash-table-test v)
                           (hash-table-count v)))
   ((bool-vector-p v) (list 'boolvec (length v)))
   ((char-table-p v) (list 'chartab (char-table-subtype v)))
   ((and dif--have-categoryp (category-table-p v)) '(cattab))
   ((and (fboundp 'abbrev-table-p) (abbrev-table-p v)) '(abbrev-table))
   ((case-table-p v) '(case-table))
   ((keymapp v) '(keymap))
   ((subrp v)
    (let ((n (ignore-errors (subr-name v))))
      (if n (list 'subr n) '(fn))))
   ((and dif--have-oclosurep (oclosurep v)) '(oclosure))
   ;; GNU's `window-configuration'/`frame-configuration' objects also
   ;; satisfy `recordp', so they must be classified before `recordp'
   ;; collapses them to (record).
   ((window-configuration-p v) '(winconf))
   ((frame-configuration-p v) '(frameconf))
   ((and dif--have-recordp (recordp v)) '(record))
   ((byte-code-function-p v) '(fn))
   ((and dif--have-autoloadp (autoloadp v)) '(autoload))
   ((macrop v) '(macro))
   ;; `functionp' is t for symbols naming functions — error-data
   ;; predicate symbols must stay printable, so exclude them.
   ((symbolp v) v)
   ((and (functionp v) (not (symbolp v))) '(fn))
   ((consp v)
    (cons (dif--walk (car v) (1+ depth))
          (if (consp (cdr v)) (dif--walk (cdr v) (1+ depth))
            (dif--walk (cdr v) (1+ depth)))))
   ((vectorp v)
    (apply #'vector (mapcar (lambda (x) (dif--walk x (1+ depth))) v)))
   (t v)))

;; GNU's byte-compiled-function arity errors carry the function object
;; (nativecomp build: #<subr>), ours carry the (min . max) cons —
;; compare only the reported argc.
(defun dif--norm-err (v)
  (if (eq (car-safe v) 'wrong-number-of-arguments)
      ;; GNU prints the function object (#<subr …>); remacs prints an
      ;; arity cons.  Only the reported argc is comparable.
      (list 'wna (nth 2 v))
    (dif--norm v)))

(defun dif--norm (v)
  "Reduce V to a deterministically-comparable print string."
  (condition-case e
      (let ((print-circle t)
            (print-escape-newlines t)
            (print-escape-control-characters t)
            (print-escape-nonascii t)
            (print-level 12)
            (print-length 120))
        (let ((s (prin1-to-string (dif--walk v 0))))
          (setq s (replace-regexp-in-string "#<[^>\n]*" "#<X" s))
          ;; Sandbox cwd names differ between shards and sides —
          ;; normalize any absolute tests/diff/sandbox* path to a
          ;; fixed token so error payloads stay comparable.
          (setq s (replace-regexp-in-string
                   "/[^\n \"]*tests/diff/sandbox[^/\n\"]*" "/SB" s))
          ;; Buffer names embedding the sandbox dir (create-file-buffer,
          ;; tooltip-event-buffer): "a<sandbox-gnu-7>" vs
          ;; "a<sandbox-remacs-7>" are the same buffer.
          (setq s (replace-regexp-in-string
                   "[a-z0-9-]*sandbox-[a-z]+-[0-9]+" "SB" s))
          ;; GNU generates space-leading buffer names with a random
          ;; -NNNNNN suffix (unseeded `emacs_random' → per-process);
          ;; the suffix is inherently non-comparable.
          (setq s (replace-regexp-in-string
                   "\"\\( [^\"\n]*\\)-[0-9]+\"" "\"\\1-N\"" s))
          ;; Timer vectors embed wall-clock stamps: [nil HI US PS ...]
          ;; — normalize the stamp cells to T.
          (setq s (replace-regexp-in-string
                   "\\[nil [0-9]+ [0-9]+ [0-9]+" "[nil T T T" s))
          ;; Large floats (time stamps, progress reporters).
          (setq s (replace-regexp-in-string
                   "[0-9]\\{7,\\}\\.[0-9]+" "TS" s))
          ;; (n . 1000000) cpu-time conses / (HI US PS 0) time lists.
          (setq s (replace-regexp-in-string
                   "([0-9]+ \\.[0-9]+)\\. 1000000" "(T . T)" s))
          (setq s (replace-regexp-in-string
                   "([0-9]+ [0-9]+ [0-9]+ 0)" "(T T T 0)" s))
          ;; (hash eq NNN) — bucket count depends on table size.
          (setq s (replace-regexp-in-string
                   "(hash \\sw+ [0-9]+)" "(hash \\1 N)" s))
          ;; invocation-name prefix in diagnostics.
          (setq s (replace-regexp-in-string
                   "\"\\(emacs\\|remacs\\): " "\"X: " s))
          ;; Temp-file random suffixes: /TMP/XXXXXX and #...#NAME#
          ;; (GNU seeds its RNG per process).
          (setq s (replace-regexp-in-string
                   "T/[A-Za-z0-9_.-]*[A-Z]\\{6\\}[^\" \n]*" "T/TMP" s))
          (setq s (replace-regexp-in-string
                   "#[A-Za-z0-9]\\{6\\}#" "#RND#" s))
          ;; Network interface hardware addresses are host-dependent.
          (setq s (replace-regexp-in-string
                   "\\[[0-9 ]*\\]" "[ADDR]" s))
          (if (> (length s) 300) (concat (substring s 0 300) "…") s)))
    (error (format "normerr:%s" (car e)))))
;;; ---- rows --------------------------------------------------------------

(defun dif--emit (name seq tag val)
  ;; stderr via `message': unbuffered on both sides, and immune to the
  ;; stdout pollution that callees (princ &co) may emit.
  (message "DIF|%s|%s|%s|%s" name seq tag val))

(defun dif--call (name seq f args)
  ;; Restore point + current buffer + window layout after the call so
  ;; a target's side effects can't drift the next target's raw context
  ;; on only one side of the pair.
  (let ((dif--cb (current-buffer)))
    (save-window-excursion
      (save-excursion
        (unwind-protect
            (catch 'dif--threw
              (condition-case e
                  (dif--emit name seq 'ok
                             (if (member name dif--norm-type)
                                 (list 'type (type-of (apply f args)))
                               (dif--norm (apply f args))))
                ;; `quit' isn't under `error' — handle explicitly so a
                ;; `keyboard-quit'-style subr can't abort the sweep.
                (quit (dif--emit name seq 'err '(quit)))
                (error (dif--emit name seq 'err
                                 (dif--norm-err (cons (car e) (cdr e)))))))
          (dif--restore-vars)
          (dif--restore-guards)
          (dif--probe-keymap name)
          ;; Symbol cells mutated mid-call (define-prefix-command,
          ;; autoload, fset-via-set) cascade through dif--walk's
          ;; keymapp check and poison later payloads; restore so each
          ;; target sees the same global cell state on both sides.
          (dif--restore-cells dif--corpus)
          (when (buffer-live-p dif--cb)
            (set-buffer dif--cb))
          ;; State probe: a target that leaves *scratch* text/point or
          ;; the current-buffer selection different on one side shows
          ;; up here, attributing the leak to THIS target rather than
          ;; surfacing 1000 rows later inside point-max's payload.
          (dif--emit "ZZstate" seq 'ok
                     (dif--norm
                      (list
                       ;; Window-buffer liveness: a target that leaves
                       ;; the selected window showing a dead buffer id
                       ;; poisons every window-* row after it.
                       (not (not (window-buffer (selected-window))))
                       (let ((b (get-buffer "*scratch*")))
                         (and b (with-current-buffer b
                                  (list (point-max) (point) (mark t)
                                        ;; Tail text — identifies WHICH
                                        ;; target wrote into scratch.
                                        (buffer-substring
                                         (max 1 (- (point-max) 60))
                                         (point-max))))))
                       (buffer-name (current-buffer))
                       ;; Buffer-local leaks a corpus restore can't
                       ;; reach: default-directory/major-mode in
                       ;; *scratch*, mode-line vars.  nil-state
                       ;; asymmetry shows up as `(nil . nil)' → `(x . t)'.
                       (let ((b (get-buffer "*scratch*")))
                         (and b (with-current-buffer b
                                  (list (not (null default-directory))
                                        (not (null major-mode)))))))))
          ;; `a' liveness tracker — set DIFF_TRACK_A=1 to attribute
          ;; the target that kills (or recreates) buffer `a'.
          ;; `a' liveness + minibuf window tracker — DIFF_TRACK_A=1.
          (when (getenv "DIFF_TRACK_A")
            (dif--emit "ZZa" seq 'ok
                       (dif--norm (list (buffer-live-p (get-buffer "a"))
                                        (and (get-buffer "a")
                                             (buffer-local-value
                                              'buffer-read-only
                                              (get-buffer "a")))
                                        (buffer-name (current-buffer))
                                        (buffer-name (window-buffer
                                                      (minibuffer-window)))
                                        (and (get-buffer "a")
                                             (buffer-base-buffer
                                              (get-buffer "a")))
                                        (mapcar #'buffer-name
                                                (buffer-list)))))))))))

          ;; `function'/`test.foo' cell tracker — DIFF_WATCH=1.
          ;; Attributes whichever target leaks a stale binding (the
          ;; add-hook/defconst/remove-hook chain reads `function''s
          ;; value cell).
          (when (getenv "DIFF_WATCH")
            (dif--emit "ZZwatch" seq 'ok
                       (list name
                             (boundp 'function)
                             (and (boundp 'function)
                                  (dif--norm (symbol-value 'function)))
                             (boundp 'test.foo)
                             (and (boundp 'test.foo)
                                  (dif--norm (symbol-value 'test.foo)))
                             (get 'function 'variable-alias)
                             (get 'test.foo 'variable-alias))))


(defconst dif--guard-fns
  '(funcall apply eval fset fdefalias defalias
    symbol-function fboundp boundp set plist-member)
  "Non-corpus engine symbols whose function cells must survive each
target's mutations — `fset'-family targets can clobber `funcall'
itself (the (invalid-function #<subr function>) cascade), and corpus
snapshotting doesn't cover symbols absent from dif--corpus.")

(defvar dif--guard-snapshot nil
  "Alist (SYM . FCELL) for `dif--guard-fns', captured at sweep start.")

(defun dif--snapshot-guards ()
  (setq dif--guard-snapshot
        (mapcar (lambda (s)
                  (cons s (if (fboundp s) (symbol-function s) dif--void)))
                dif--guard-fns)))

(defun dif--restore-guards ()
  "Restore `dif--guard-fns' cells changed since snapshot."
  (dolist (p dif--guard-snapshot)
    (let ((x (car p)) (saved (cdr p)))
      (let ((fc (if (fboundp x) (symbol-function x) dif--void)))
        (unless (equal fc saved)
          (condition-case nil
              (if (eq saved dif--void)
                  (fmakunbound x)
                (fset x saved))
            (error nil)))))))

(defun dif--probe-keymap (name)
  "Emit a probe row if FUNCTION's function cell became keymapp.
Attributes the first offender when a callee mutates global cells."
  (when (keymapp 'function)
    (message "PROBE|keymapp-function|%s|%s" name
             (prin1-to-string (symbol-function 'function)))))

(defvar dif--cell-snapshot nil
  "Vector parallel to the corpus: (FCELL . VCELL) per symbol slot.
Built once in `dif--mk''s caller; `dif--restore-cells' reverts any
callee mutation so global symbol state can't cascade between targets
(e.g. `define-prefix-command' fsets its argument).")

(defconst dif--void (list 'dif--void)
  "Sentinel for unbound cells in `dif--cell-snapshot' — a bound nil
and an absent binding must restore differently (makunbound on a
built-in like `buffer-read-only' signals, and fmakunbound on
subrs' symbols kills the subr).")

(defun dif--snapshot-cells (corpus)
  "Capture each corpus symbol's function and value cells."
  (let ((v (make-vector (length corpus) nil)))
    (dotimes (k (length corpus))
      (let ((x (aref corpus k)))
        (when (and (symbolp x) (not (memq x '(t nil))))
          ;; Also snapshot the `face' property: `make-face' /
          ;; `custom-declare-face' et al register the name on the face
          ;; table (reflected as the symbol's `face' prop), and the
          ;; face table isn't reachable from fboundp/boundp, so a
          ;; leak here makes every later `face-id'-style call
          ;; diverge.
          (aset v k (list (if (fboundp x) (symbol-function x) dif--void)
                          (if (boundp x) (symbol-value x) dif--void)
                          (if (and (fboundp 'internal-lisp-face-p)
                                   (internal-lisp-face-p x))
                              t
                            dif--void))))))
    v))

(defun dif--restore-cells (corpus)
  "Revert function/value cells of corpus symbols to the snapshot."
  (when dif--cell-snapshot
    (dotimes (k (length corpus))
      (let* ((x (aref corpus k))
             (snap (and (symbolp x) (aref dif--cell-snapshot k))))
        (when snap
          (let ((fc (if (fboundp x) (symbol-function x) dif--void)))
            (unless (equal fc (car snap))
              (condition-case nil
                  (if (eq (car snap) dif--void)
                      (fmakunbound x)
                    (fset x (car snap)))
                (error nil))))
          (let ((vc (if (boundp x) (symbol-value x) dif--void)))
            (unless (equal vc (cadr snap))
              (condition-case nil
                  (if (eq (cadr snap) dif--void)
                      (makunbound x)
                    (set x (cadr snap)))
                (error nil))))
          ;; Restore the `face' property: remacs keeps a global face
          ;; table (faces aren't per-frame) so a corpus symbol made a
          ;; face by `make-face'/`custom-declare-face' must be
          ;; un-registered when it wasn't a face at snapshot time.
          (let ((fc (if (and (fboundp 'internal-lisp-face-p)
                             (internal-lisp-face-p x))
                        t dif--void)))
            (unless (eq fc (nth 2 snap))
              (condition-case nil
                  (if (eq (nth 2 snap) dif--void)
                      (when (and (fboundp 'internal-lisp-face-p)
                                 (internal-lisp-face-p x))
                        (remprop x 'face))
                    (unless (and (fboundp 'internal-lisp-face-p)
                                 (internal-lisp-face-p x))
                      (make-face x)))
                (error nil)))))))))
(defvar dif--var-snapshot nil
  "Alist (SYM . SAVED) of global variables whose *value* mutations
would poison later targets — `cl-struct-*-tags' lists (pushed to by
`cl--struct-register-tag' at any time) and the `dif--guard-vars'
globals (`features', `exec-path' & kin) mutated by `require'/`push'
targets.")

(defconst dif--guard-vars
  '(features load-history exec-path load-path process-environment
    exec-suffixes shell-file-name)
  "Global variables snapshotted once per sweep; mutation during one
call is reverted before the next.")

(defun dif--snapshot-vars ()
  "Capture the guard variable set once; each side keeps its own
baseline (remacs `features' legitimately differs from GNU's)."
  (setq dif--var-snapshot nil)
  (dolist (s dif--guard-vars)
    (when (boundp s)
      (push (cons s (copy-sequence (symbol-value s))) dif--var-snapshot)))
  (mapatoms
   (lambda (x)
     (when (and (string-match-p "\\`cl-struct-.*-tags\\'" (symbol-name x))
                (boundp x))
       (push (cons x (copy-sequence (symbol-value x)))
             dif--var-snapshot)))))

(defun dif--restore-vars ()
  (dolist (p dif--var-snapshot)
    (let ((x (car p)) (saved (cdr p)))
      (unless (equal (and (boundp x) (symbol-value x)) saved)
        (condition-case nil (set x (copy-sequence saved)) (error nil))))))

(defun dif--reset-fixture ()
  "Recreate the \" dfx\" fixture buffer with pristine text."
  (let ((kill-buffer-hook nil) (kill-buffer-query-functions nil)
        (old (get-buffer " dfx")))
    (when old (kill-buffer old)))
  (setq dif--fxbuf (get-buffer-create " dfx"))
  (with-current-buffer dif--fxbuf
    (insert "Alpha beta gamma.\nDelta epsilon zeta.\nEta theta.\n")))

(defun dif--call-buf (name seq f args)
  "Call inside a fresh fixture buffer, point mid-text.  A fresh
buffer per call keeps one target's inserts from leaking into the
next — both sides must see identical fixture state."
  (with-temp-buffer
    (insert "Alpha beta gamma.\nDelta epsilon zeta.\nEta theta.\n")
    (goto-char 9)
    (dif--call name seq f args)))

(defun dif--skip-p (name)
  (or (member name dif--skip)
      (member name dif--skip-input)
      (let ((hit nil))
        (dolist (p dif--skip-prefix)
          (when (and (<= (length p) (length name))
                     (equal p (substring name 0 (length p))))
            (setq hit t)))
        hit)))

(defun dif--args (n seed corpus)
  (let ((m (length corpus)) (i 0) (out nil))
    (while (< i n)
      ;; Fresh copy per call: callees may mutate args in place
      ;; (put-text-property, setcar, aset); without this, one side's
      ;; mutation leaks into later targets and shows up as a bogus
      ;; payload divergence.
      (push (dif--fresh (aref corpus (% (+ (* seed 29) (* i 17) i 3) m)))
            out)
      (setq i (1+ i)))
    (nreverse out)))

