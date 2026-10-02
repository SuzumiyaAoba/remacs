;;; tests/diff/driver.el --- differential corpus driver -*- lexical-binding: t -*-
;;
;; Same file is loaded by `emacs -Q --batch' and `remacs --batch' inside a
;; scratch-file sandbox (cwd).  For every (NAME . ARITY) in dif--targets
;; (loaded from $DIFF_DIR/targets.el) it emits one stdout row per call:
;;
;;   DIF|<name>|<seq>|<ok|err|unbound>|<normalized payload>
;;
;; and a stderr marker `DMARK <name> <seq>' before each call so a hang or
;; crash can be attributed.  stdout pollution from the callee (princ &co)
;; is tolerated: the report tool searches each line for `DIF|'.

(defvar dif--targets nil)
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
    "early-init-file" "init-file-user"))

(defvar dif--skip-prefix
  '("w32-" "cygwin-" "msdos-" "haiku-" "android-" "pgtk-" "haiku"
    "dbus-" "sqlite" "native-comp-" "comp-" "xwidget" "profiler-"
    "file-notify-" "debug-early" "internal-module" "module-"
    "electric-" "Electric-"))
;; Result normalized to (type-of ...) symbol only — inherently
;; environment/time/pid-dependent or self-describing.
(defvar dif--norm-type
  '("current-time" "current-time-string" "current-time-zone"
    "decode-time" "emacs-uptime" "emacs-init-time" "emacs-pid"
    "float-time" "emacs-version" "emacs-build-time"
    "system-configuration" "system-configuration-options"
    "system-configuration-features" "invocation-name"
    "invocation-directory" "features"
    "garbage-collect" "memory-report" "memory-info" "memory-limit"
    "malloc-info" "list-system-processes" "process-attributes"
    "user-real-uid" "user-uid" "user-full-name" "user-login-name"
    "group-gid" "group-real-gid" "system-name" "system-groups"
    "system-users" "file-attributes" "directory-files-and-attributes"
    "file-system-info" "file-system-info-...")) ;; file-system-info kept? value depends on fs usage — normalize
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
      (aset v i (condition-case nil (eval (nth i dif--forms))
                  (error (list 'dif--corpus-err i)))))
    v))

;;; ---- value normalization ----------------------------------------------

(defvar dif--have-terminalp (fboundp 'terminalp))
(defvar dif--have-recordp (fboundp 'recordp))
(defvar dif--have-categoryp (fboundp 'category-table-p))
(defvar dif--have-oclosurep (fboundp 'oclosurep))
(defvar dif--have-autoloadp (fboundp 'autoloadp))

(defun dif--norm (v)
  "Reduce V to a deterministically-comparable print string."
  (condition-case e
      (cond
       ((markerp v)
        (list 'marker (marker-position v) (not (not (marker-buffer v)))))
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
       ((syntax-table-p v) '(syntax-table))
       ((case-table-p v) '(case-table))
       ((keymapp v) '(keymap))
       ((subrp v) (list 'subr (ignore-errors (subr-name v))))
       ((and dif--have-oclosurep (oclosurep v)) '(oclosure))
       ((and dif--have-recordp (recordp v)) '(record))
       ((byte-code-function-p v) '(bytecode))
       ((window-configuration-p v) '(winconf))
       ((frame-configuration-p v) '(frameconf))
       ((and (boundp 'dif--have-autoloadp) dif--have-autoloadp (autoloadp v)) '(autoload))
       ((macrop v) '(macro))
       ((functionp v) '(fn))
       (t (let ((print-circle t)
                (print-escape-newlines t)
                (print-escape-control-characters t)
                (print-escape-nonascii t)
                (print-level 6)
                (print-length 60))
            (let ((s (prin1-to-string v)))
              (setq s (replace-regexp-in-string "#<[^>\n]*" "#<X" s))
              (if (> (length s) 300) (concat (substring s 0 300) "…") s)))))
    (error (format "normerr:%s" (car e)))))

;;; ---- rows --------------------------------------------------------------

(defun dif--emit (name seq tag val)
  ;; stderr via `message': unbuffered on both sides, and immune to the
  ;; stdout pollution that callees (princ &co) may emit.
  (message "DIF|%s|%s|%s|%s" name seq tag val))

(defun dif--call (name seq f args)
  (condition-case e
      (dif--emit name seq 'ok
                 (if (member name dif--norm-type)
                     (list 'type (type-of (apply f args)))
                   (dif--norm (apply f args))))
    (error (dif--emit name seq 'err
                      (dif--norm (cons (car e) (cdr e)))))))

(defun dif--call-buf (name seq f args)
  "Call inside the fixture buffer, point mid-text."
  (with-current-buffer dif--fxbuf
    (save-excursion
       (save-restriction
        (widen)
        (goto-char 9)
        (dif--call name seq f args)))))

(defun dif--skip-p (name)
  (or (member name dif--skip)
      (let ((hit nil))
        (dolist (p dif--skip-prefix)
          (when (and (<= (length p) (length name))
                     (equal p (substring name 0 (length p))))
            (setq hit t)))
        hit)))

(defun dif--args (n seed corpus)
  (let ((m (length corpus)) (i 0) (out nil))
    (while (< i n)
      (push (aref corpus (% (+ (* seed 29) (* i 17) i 3) m)) out)
      (setq i (1+ i)))
    (nreverse out)))

;;; ---- main loop ---------------------------------------------------------

(progn
  (setq gc-cons-threshold 800000000)
  (random "difftest-seed")          ;; deterministic PRNG
  (load (expand-file-name "targets.el"
                          (or (getenv "DIFF_DIR") ".")))
  (setq dif--fxbuf (get-buffer-create " dfx"))
  (with-current-buffer dif--fxbuf
    (insert "Alpha beta gamma.\nDelta epsilon zeta.\nEta theta.\n"))
  (let ((corpus (dif--mk))
        (only (getenv "DIFF_ONLY"))
        (from (getenv "DIFF_FROM"))
        (auto (getenv "DIFF_AUTOSKIP"))
        (total 0))
    (when (and auto (not (equal auto "")))
      (dolist (n (split-string auto ","))
        (push n dif--skip)))
    (dolist (tgt dif--targets)
      (let ((name (car tgt))
            (arity (cadr tgt))
            (seq 0))
        (when (and (or (null only) (equal only "")
                       (member name (split-string only ",")))
                   (or (null from) (equal from "") (not (string< name from)))
                   (not (dif--skip-p name)))
          (let* ((mn (if (consp arity) (car arity) 0))
                 (mx (if (consp arity) (cdr arity) 0))
                 (maxf (if (numberp mx) mx mn))
                 (ns (list 0 mn (max 0 (1- mn))))
                 (ns (append ns
                             (if (numberp mx)
                                 (list (min 10 mx) (min 11 (1+ mx)))
                               (list (min 5 (+ mn 2)) (min 6 (+ mn 4))))))
                 (seen nil) (runs nil))
            (dolist (n ns)
              (unless (or (member n seen) (< n 0))
                (push n seen)
                (push (cons n nil) runs)))
            ;; one buffer-context call at minimum arity
            (push (cons mn 'buf) runs)
            (dolist (r (nreverse runs))
              (let ((n (car r)) (buf (cdr r)))
                (when (or (> n 0) (not (member name dif--skip-noarg)))
                  (setq seq (1+ seq) total (1+ total))
                  (message "DMARK %s %s %s" name seq (if buf "buf" n))
                  (let ((sym (intern-soft name))
                        (args (dif--args n seq corpus)))
                    (when (getenv "DIFF_DEBUG")
                      (dif--emit name seq 'args
                                 (dif--norm (cons n args))))
                    (cond
                     ((or (null sym) (not (fboundp sym)))
                      (dif--emit name seq 'unbound nil))
                     (t
                      (let ((f (condition-case nil
                                   (indirect-function sym t)
                                 (error nil))))
                        (if buf (dif--call-buf name seq f args)
                          (dif--call name seq f args)))))))))))))
    (message "DIFDONE %d rows" total)
    (message "DIF|TOTAL|%d" total)))

;;; driver.el ends here
