//! Eager default-value construction. Do not install entries one at a time:
//! symbol interning and table creation here must finish before the default
//! names are interned/assigned by define_special_variables.

use super::environment;
use crate::lisp::{
    eval::Interp,
    obarray::sym,
    value::{Value, VecRef},
};

impl Interp {
    pub(super) fn collect_variable_defaults(
        &mut self,
        default_obarray_rec: &VecRef,
    ) -> Vec<(&'static str, Value)> {
        vec![
            ("emacs-major-version", Value::Int(31)),
            ("emacs-minor-version", Value::Int(1)),
            // GNU's `emacs-version' variable is just "31.1" (the
            // remacs identity lives in the `emacs-version' *function*).
            // With " (remacs)" here, `version-to-list' errored on
            // "Invalid version syntax" and broke ol/tramp-compat/xref.
            ("emacs-version", Value::string("31.1")),
            // GNU's NS build binds this in nsterm.m (AppKit version);
            // `emacs-version' reads it under (featurep 'ns).
            (
                "ns-version-string",
                Value::string("appkit-2685.60 Version 26.5.2 (Build 25F84)"),
            ),
            ("system-type", Value::Sym(self.q("darwin"))),
            (
                "system-configuration",
                Value::string(env!("REMACS_SYSTEM_CONFIGURATION")),
            ),
            ("fill-column", Value::Int(70)),
            ("tab-width", Value::Int(8)),
            ("standard-indent", Value::Int(4)),
            ("indent-tabs-mode", Value::t()),
            ("case-fold-search", Value::t()),
            ("case-replace", Value::t()),
            (
                "noninteractive",
                if self.noninteractive {
                    Value::t()
                } else {
                    Value::Nil
                },
            ),
            ("standard-output", Value::t()),
            ("standard-input", Value::t()),
            (
                "most-positive-fixnum",
                Value::Int(crate::lisp::value::FIXNUM_MAX),
            ),
            (
                "most-negative-fixnum",
                Value::Int(crate::lisp::value::FIXNUM_MIN),
            ),
            ("max-lisp-eval-depth", Value::Int(1600)),
            ("max-specpdl-size", Value::Int(2500)),
            ("gc-cons-threshold", Value::Int(800_000)),
            // GNU's Vgc_elapsed/Vgcs_done (alloc.c): GC statistics
            // variables.  Remacs has no tracing GC, so they stay at 0 —
            // `benchmark-run' reads them.
            ("gc-elapsed", Value::float(0.0)),
            ("gcs-done", Value::Int(0)),
            ("history-length", Value::Int(100)),
            ("kill-ring-max", Value::Int(120)),
            ("mark-ring-max", Value::Int(16)),
            ("global-mark-ring-max", Value::Int(16)),
            ("scroll-margin", Value::Int(0)),
            ("scroll-conservatively", Value::Int(0)),
            ("next-screen-context-lines", Value::Int(2)),
            ("undo-limit", Value::Int(160_000)),
            ("undo-strong-limit", Value::Int(240_000)),
            // GNU 31: nil means "no outer limit" (cus-start choice).
            ("undo-outer-limit", Value::Nil),
            ("pending-undo-list", Value::Nil),
            ("undo-in-region", Value::Nil),
            ("undo-no-redo", Value::Nil),
            ("undo-inhibit-record-point", Value::Nil),
            ("echo-keystrokes", Value::Int(1)),
            ("auto-save-interval", Value::Int(300)),
            ("auto-save-timeout", Value::Int(30)),
            ("double-click-time", Value::Int(500)),
            ("double-click-fuzz", Value::Int(3)),
            ("minibuffer-message-timeout", Value::Int(2)),
            ("read-process-output-max", Value::Int(65536)),
            // split-string can run during prelude evaluation, before
            // GNU subr.el's defconst has been loaded.
            (
                "split-string-default-separators",
                Value::string("[ \u{0c}\t\n\r\u{0b}]+"),
            ),
            ("max-mini-window-height", Value::float(0.25)),
            ("window-min-height", Value::Int(4)),
            ("window-min-width", Value::Int(10)),
            ("window-safe-min-height", Value::Int(1)),
            ("window-safe-min-width", Value::Int(2)),
            ("standard-display-table", Value::Nil),
            ("display-mm-dimensions-alist", Value::Nil),
            ("buffer-display-table", Value::Nil),
            ("window-size-fixed", Value::Nil),
            ("tooltip-mode", Value::t()),
            // GNU -Q batch starts with blink-cursor-mode off.
            ("blink-cursor-mode", Value::Nil),
            (
                "split-window-preferred-direction",
                Value::Sym(self.q("longest")),
            ),
            ("split-height-threshold", Value::Int(80)),
            ("split-width-threshold", Value::Int(160)),
            // GNU image.el: file-name → image-type mapping.
            (
                "image-type-file-name-regexps",
                Value::list(
                    [
                        ("\\.png\\'", "png"),
                        ("\\.gif\\'", "gif"),
                        ("\\.jpe?g\\'", "jpeg"),
                        ("\\.webp\\'", "webp"),
                        ("\\.bmp\\'", "bmp"),
                        ("\\.xpm\\'", "xpm"),
                        ("\\.pbm\\'", "pbm"),
                        ("\\.xbm\\'", "xbm"),
                        ("\\.ps\\'", "postscript"),
                        ("\\.tiff?\\'", "tiff"),
                        ("\\.svgz?\\'", "svg"),
                        ("\\.hei[cf]s?\\'", "heic"),
                    ]
                    .iter()
                    .map(|(re, ty)| Value::cons(Value::string(*re), Value::Sym(self.intern(ty))))
                    .collect(),
                ),
            ),
            ("completion-ignore-case", Value::Nil),
            ("read-buffer-completion-ignore-case", Value::Nil),
            // GNU defaults to t on case-insensitive filesystems
            // (darwin): minibuffer.el's defcustom probes the fs.
            ("read-file-name-completion-ignore-case", Value::Sym(sym::T)),
            ("history-delete-duplicates", Value::Nil),
            ("minibuffer-history", Value::Nil),
            ("buffer-name-history", Value::Nil),
            ("kill-ring", Value::Nil),
            ("values", Value::Nil),
            ("float-output-format", Value::Nil),
            ("print-level", Value::Nil),
            ("print-length", Value::Nil),
            ("print-circle", Value::Nil),
            // GNU initializes `load-path' to the directories holding
            // the bundled Lisp libraries (the installation's lisp/ +
            // subdirs).  remacs resolves bundled libs through
            // EMBEDDED_LISP plus `load::builtin_dirs'; expose the
            // existing on-disk dirs so `load'/`locate-file' behave
            // the same for files on disk.
            (
                "load-path",
                Value::list(
                    crate::lisp::load::builtin_dirs()
                        .iter()
                        .filter(|d| {
                            let p = std::path::Path::new(d);
                            p.is_dir()
                        })
                        .filter_map(|d| {
                            std::fs::canonicalize(d)
                                .ok()
                                .map(|p| Value::string(crate::lisp::text::EmacsText::from_os_str(p.as_os_str())))
                        })
                        .collect(),
                ),
            ),
            // GNU: `load-file-name' is a C variable bound to nil
            // outside of `load' (Vload_file_name), so it is always
            // readable (e.g. by `custom-current-group').
            ("load-file-name", Value::Nil),
            // GNU lread.c DEFVAR_LISPs `load-true-file-name' to nil —
            // bound (readable) at -Q even outside `load'.
            ("load-true-file-name", Value::Nil),
            (
                "default-directory",
                Value::string({
                    // GNU `emacs_wd' (sysdep.c): the logical cwd —
                    // $PWD wins when it stat-matches getcwd, else
                    // physical getcwd.  So `cd /tmp' (a symlink on
                    // macOS) reports /tmp, not /private/tmp.
                    let mut d = crate::editor::emacs_wd();
                    if !d.ends_with('/') {
                        d.push('/');
                    }
                    d
                }),
            ),
            ("buffer-undo-list", Value::Nil),
            ("mark-ring", Value::Nil),
            ("text-quoting-style", Value::Nil),
            // GNU main() seeds this from `using_utf8 ()' (the
            // LC_ALL > LC_CTYPE > LANG codeset); startup.el forces it
            // to t for interactive sessions.  nil makes a nil
            // `text-quoting-style' behave as `grave'.
            (
                "internal--text-quoting-flag",
                // GNU main() seeds this from `using_utf8 ()' — the
                // LC_ALL > LC_CTYPE > LANG codeset — and startup.el
                // itself forces t for interactive sessions, so batch
                // under a non-UTF-8 locale must stay nil (grave).
                Value::from_bool({
                    let loc = ["LC_ALL", "LC_CTYPE", "LANG"]
                        .iter()
                        .filter_map(|k| std::env::var(k).ok())
                        .find(|v| !v.is_empty())
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    loc.contains("utf-8") || loc.contains("utf8")
                }),
            ),
            ("transient-mark-mode", Value::Nil),
            ("mark-even-if-inactive", Value::t()),
            ("use-empty-active-region", Value::Nil),
            ("exchange-point-and-mark-highlight-region", Value::t()),
            ("global-mark-ring", Value::Nil),
            ("shift-select-mode", Value::t()),
            ("delete-active-region", Value::t()),
            ("inhibit-read-only", Value::Nil),
            ("deactivate-mark", Value::Nil),
            ("buffer-read-only", Value::Nil),
            ("truncate-lines", Value::Nil),
            ("enable-multibyte-characters", Value::t()),
            ("obarray", Value::Record(default_obarray_rec.clone())),
            (
                "features",
                // Kept in sync with `self.features' by `provide'.
                Value::list(self.features.iter().map(|s| Value::Sym(*s)).collect()),
            ),
            ("current-load-list", Value::Nil),
            ("load-in-progress", Value::Nil),
            ("command-history", Value::Nil),
            ("regexp-search-ring", Value::Nil),
            ("search-ring", Value::Nil),
            ("register-alist", Value::Nil),
            ("killed-rectangle", Value::Nil),
            ("global-map", Value::Nil), // set up by editor init
            ("minibuffer-local-map", Value::Nil),
            // GNU's `lexical-binding' default is nil; `*scratch*' gets
            // a buffer-local t (seeded below).  Reading it at eval
            // entry picks up the lexical env for --eval/-l.
            ("lexical-binding", Value::Nil),
            ("overriding-local-map", Value::Nil),
            // auto-mode-alist / interpreter-mode-alist / magic-mode-alist
            // get their real GNU defaults from prelude defvars.
            // GNU: exec-path = $PATH dirs + exec-directory.
            (
                "exec-path",
                Value::list(
                    std::env::var("PATH")
                        .unwrap_or_default()
                        .split(':')
                        .filter(|s| !s.is_empty())
                        .map(|d| {
                            Value::string(
                                d.trim_end_matches('/').to_string()
                                    + if d.trim_end_matches('/').is_empty() {
                                        "/"
                                    } else {
                                        ""
                                    },
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "process-environment",
                Value::list(
                    std::env::vars()
                        .map(|(k, v)| Value::string(format!("{k}={v}")))
                        .collect(),
                ),
            ),
            // Command-loop state variables (set per command interactively).
            ("last-command", Value::Nil),
            ("this-command", Value::Nil),
            ("real-last-command", Value::Nil),
            ("last-command-event", Value::Nil),
            ("last-input-event", Value::Nil),
            ("last-event-frame", Value::Nil),
            ("last-event-device", Value::Nil),
            ("current-prefix-arg", Value::Nil),
            ("prefix-arg", Value::Nil),
            ("unread-command-events", Value::Nil),
            ("unread-post-input-method-events", Value::Nil),
            ("quit-flag", Value::Nil),
            ("native-comp-enable-subr-trampolines", Value::t()),
            ("inhibit-quit", Value::Nil),
            ("inhibit-interaction", Value::Nil),
            ("throw-on-input", Value::Nil),
            ("mark-active", Value::Nil),
            ("minibuffer-inactive-mode", Value::Nil),
            ("read-hide-char", Value::Nil),
            ("read-expression-history", Value::Nil),
            // Hook/option variables with GNU defaults.
            ("before-change-functions", Value::Nil),
            ("after-change-functions", Value::Nil),
            ("inhibit-modification-hooks", Value::Nil),
            ("inhibit-point-motion-hooks", Value::Sym(sym::T)),
            ("inhibit-field-text-motion", Value::Nil),
            ("focus-in-hook", Value::Nil),
            ("focus-out-hook", Value::Nil),
            ("after-focus-change-function", Value::Sym(self.q("ignore"))),
            ("minor-mode-map-alist", Value::Nil),
            ("emulation-mode-map-alists", Value::Nil),
            ("overriding-terminal-local-map", Value::Nil),
            ("char-property-alias-alist", Value::Nil),
            ("default-text-properties", Value::Nil),
            // GNU bindings.el's standard format.  A nil placeholder suppresses
            // the mode line when computing window minimum heights.
            (
                "mode-line-format",
                Value::list(vec![
                    Value::string("%e"),
                    Value::Sym(self.q("mode-line-front-space")),
                    Value::Sym(self.q("mode-line-mule-info")),
                    Value::Sym(self.q("mode-line-client")),
                    Value::Sym(self.q("mode-line-modified")),
                    Value::Sym(self.q("mode-line-remote")),
                    Value::Sym(self.q("mode-line-window-dedicated")),
                    Value::Sym(self.q("mode-line-frame-identification")),
                    Value::Sym(self.q("mode-line-buffer-identification")),
                    Value::string("   "),
                    Value::Sym(self.q("mode-line-position")),
                    Value::list(vec![
                        Value::Sym(self.q("project-mode-line")),
                        Value::Sym(self.q("project-mode-line-format")),
                    ]),
                    Value::list(vec![
                        Value::Sym(self.q("vc-mode")),
                        Value::Sym(self.q("vc-mode")),
                    ]),
                    Value::string("  "),
                    Value::Sym(self.q("mode-line-modes")),
                    Value::Sym(self.q("mode-line-misc-info")),
                    Value::Sym(self.q("mode-line-end-spaces")),
                ]),
            ),
            ("mode-name", Value::Nil),
            ("header-line-format", Value::Nil),
            ("tab-line-format", Value::Nil),
            ("truncate-partial-width-windows", Value::Int(50)),
            ("word-wrap", Value::Nil),
            ("scroll-step", Value::Int(0)),
            ("scroll-error-top-bottom", Value::Nil),
            ("scroll-preserve-screen-position", Value::Nil),
            ("scroll-up-aggressively", Value::Nil),
            ("scroll-down-aggressively", Value::Nil),
            ("scroll-minibuffer-conservatively", Value::Sym(sym::T)),
            ("hscroll-margin", Value::Int(5)),
            ("hscroll-step", Value::Int(0)),
            ("auto-hscroll-mode", Value::Sym(sym::T)),
            ("polling-period", Value::float(2.0)),
            ("visible-bell", Value::Nil),
            ("ring-bell-function", Value::Nil),
            ("cursor-type", Value::Sym(sym::T)),
            ("cursor-in-non-selected-windows", Value::Sym(sym::T)),
            ("blink-matching-paren", Value::Sym(sym::T)),
            ("show-paren-mode", Value::Nil),
            ("sentence-end", Value::Nil),
            ("recenter-redisplay", Value::Sym(self.q("tty"))),
            ("redisplay-dont-pause", Value::Sym(sym::T)),
            ("fast-but-imprecise-scrolling", Value::Nil),
            ("print-escape-multibyte", Value::Nil),
            ("print-escape-nonascii", Value::Nil),
            ("print-gensym", Value::Nil),
            ("print-continuous-numbering", Value::Nil),
            ("print-number-table", Value::Nil),
            ("print-readably", Value::Sym(sym::T)),
            ("eval-expression-print-length", Value::Int(12)),
            ("eval-expression-print-level", Value::Int(4)),
            ("font-lock-maximum-decoration", Value::Sym(sym::T)),
            ("font-lock-maximum-size", Value::Nil),
            ("jit-lock-stealth-time", Value::Nil),
            ("jit-lock-stealth-load", Value::Int(200)),
            ("jit-lock-defer-time", Value::Nil),
            ("jit-lock-context-time", Value::float(0.5)),
            ("jit-lock-functions", Value::Nil),
            ("jit-lock-contextually", Value::Sym(self.q("syntax-driven"))),
            ("font-lock-mode", Value::Nil),
            ("scroll-lock-mode", Value::Nil),
            ("pixel-scroll-mode", Value::Nil),
            ("pixel-scroll-precision-mode", Value::Nil),
            ("overwrite-mode", Value::Nil),
            // Editor options (GNU batch defaults).
            ("abbrev-mode", Value::Nil),
            ("abbrev-all-caps", Value::Nil),
            ("auto-save-default", Value::Sym(sym::T)),
            ("auto-mode-case-fold", Value::Sym(sym::T)),
            ("backup-by-copying", Value::Nil),
            ("baud-rate", Value::Int(0)),
            ("blink-matching-delay", Value::Int(1)),
            ("coding-system-for-read", Value::Nil),
            ("coding-system-for-write", Value::Nil),
            ("comment-column", Value::Int(32)),
            ("comment-end", Value::string("")),
            ("completion-auto-help", Value::Sym(sym::T)),
            ("completions-detailed", Value::Nil),
            (
                "confirm-nonexistent-file-or-buffer",
                Value::Sym(self.q("after-completion")),
            ),
            ("ctl-arrow", Value::Sym(sym::T)),
            ("current-input-method", Value::Nil),
            ("cursor-in-echo-area", Value::Nil),
            ("debug-on-quit", Value::Nil),
            ("default-justification", Value::Sym(self.q("left"))),
            ("delete-old-versions", Value::Nil),
            ("enable-recursive-minibuffers", Value::Nil),
            ("eval-expression-debug-on-error", Value::Sym(sym::T)),
            ("find-file-existing-other-name", Value::Sym(sym::T)),
            ("find-file-visit-truename", Value::Nil),
            ("gc-cons-percentage", Value::float(1.0)),
            ("idle-update-delay", Value::float(0.5)),
            ("input-method-function", Value::Sym(self.q("list"))),
            ("insert-default-directory", Value::Sym(sym::T)),
            ("isearch-allow-scroll", Value::Nil),
            ("isearch-case-fold-search", Value::Nil),
            ("isearch-lazy-highlight", Value::Sym(sym::T)),
            ("isearch-regexp", Value::Nil),
            ("left-margin-width", Value::Int(0)),
            ("line-number-display-limit", Value::Nil),
            ("list-buffers-directory", Value::Nil),
            ("locale-coding-system", Value::Sym(self.q("utf-8-unix"))),
            ("make-backup-files", Value::Sym(sym::T)),
            ("message-log-max", Value::Int(1000)),
            ("mode-require-final-newline", Value::Sym(sym::T)),
            ("next-error-highlight", Value::float(0.5)),
            ("no-redraw-on-reenter", Value::Nil),
            ("normal-erase-is-backspace", Value::Sym(self.q("maybe"))),
            ("paragraph-separate", Value::string("[ \t\x0c]*$")),
            ("paragraph-start", Value::string("\x0c\\|[ \t]*$")),
            ("page-delimiter", Value::string("^\x0c")),
            ("parse-sexp-ignore-comments", Value::Nil),
            // GNU: files.el's defcustom makes the default nil; the
            // *scratch* buffer-local value (t) is seeded at init.
            ("require-final-newline", Value::Nil),
            ("sort-numeric-base", Value::Int(10)),
            // GNU -Q leaves `sort-fold-case' unbound: its defcustom
            // lives in sort.el, which is not in the dump.
            ("resize-mini-windows", Value::Sym(self.q("grow-only"))),
            ("select-enable-clipboard", Value::Sym(sym::T)),
            ("selection-coding-system", Value::Nil),
            (
                "send-mail-function",
                Value::Sym(self.q("sendmail-query-once")),
            ),
            ("shell-command-switch", Value::string("-c")),
            ("tab-always-indent", Value::Sym(sym::T)),
            ("track-eol", Value::Nil),
            ("goal-column", Value::Nil),
            ("temporary-goal-column", Value::Int(0)),
            ("next-line-add-newlines", Value::Nil),
            ("auto-window-vscroll", Value::Sym(sym::T)),
            ("line-move-visual", Value::Sym(sym::T)),
            ("line-move-ignore-invisible", Value::Sym(sym::T)),
            ("overflow-newline-into-fringe", Value::Sym(sym::T)),
            (
                "uniquify-buffer-name-style",
                Value::Sym(self.q("post-forward-angle-brackets")),
            ),
            ("use-file-dialog", Value::Sym(sym::T)),
            ("version-control", Value::Nil),
            ("visible-cursor", Value::Sym(sym::T)),
            ("write-contents-hooks", Value::Nil),
            (
                "yank-excluded-properties",
                Value::list(vec![
                    Value::Sym(self.q("category")),
                    Value::Sym(self.q("field")),
                    Value::Sym(self.q("follow-link")),
                    Value::Sym(self.q("fontified")),
                    Value::Sym(self.q("font-lock-face")),
                    Value::Sym(self.q("help-echo")),
                    Value::Sym(self.q("intangible")),
                    Value::Sym(self.q("invisible")),
                    Value::Sym(self.q("keymap")),
                    Value::Sym(self.q("local-map")),
                    Value::Sym(self.q("mouse-face")),
                    Value::Sym(self.q("read-only")),
                    Value::Sym(self.q("yank-handler")),
                ]),
            ),
            ("kill-ring-yank-pointer", Value::Nil),
            ("buffer-auto-save-file-name", Value::Nil),
            (
                "buffer-file-coding-system",
                Value::Sym(self.q("utf-8-unix")),
            ),
            ("buffer-file-format", Value::Nil),
            ("buffer-file-truename", Value::Nil),
            ("buffer-backed-up", Value::Nil),
            ("abbrev-file-name", Value::string("~/.emacs.d/abbrev_defs")),
            ("add-log-full-name", Value::Nil),
            ("add-log-mailing-address", Value::Nil),
            ("dired-directory", Value::Nil),
            ("unread-input-method-events", Value::Nil),
            ("executing-kbd-macro", Value::Nil),
            ("defining-kbd-macro", Value::Nil),
            ("last-kbd-macro", Value::Nil),
            ("kbd-macro-termination-hook", Value::Nil),
            ("activate-mark-hook", Value::Nil),
            ("deactivate-mark-hook", Value::Nil),
            (
                "command-error-function",
                Value::Sym(self.q("command-error-default-function")),
            ),
            ("post-command-hook", Value::Nil),
            ("pre-command-hook", Value::Nil),
            ("minibuffer-setup-hook", Value::Nil),
            ("minibuffer-exit-hook", Value::Nil),
            ("minibuffer-help-form", Value::Nil),
            ("minibuffer-scroll-window", Value::Nil),
            ("enable-local-variables", Value::Sym(sym::T)),
            ("enable-local-eval", Value::Sym(self.q("maybe"))),
            ("buffer-access-fontify-functions", Value::Nil),
            ("redisplay--variables", Value::Nil),
            ("buffer-display-count", Value::Int(0)),
            ("buffer-display-time", Value::Nil),
            ("buffer-auto-save-file-format", Value::t()),
            ("point-before-scroll", Value::Nil),
            ("local-minor-modes", Value::Nil),
            ("first-change-hook", Value::Nil),
            ("window-configuration-change-hook", Value::Nil),
            ("window-size-change-functions", Value::Nil),
            ("window-scroll-functions", Value::Nil),
            ("window-text-change-functions", Value::Nil),
            ("window-selection-change-functions", Value::Nil),
            ("change-major-mode-hook", Value::Nil),
            ("after-load-alist", Value::Nil),
            ("delay-mode-hooks", Value::Nil),
            (
                "lisp-indent-function",
                Value::Sym(self.q("lisp-indent-function")),
            ),
            (
                "indent-line-function",
                Value::Sym(self.q("indent-relative")),
            ),
            ("electric-indent-mode", Value::Sym(sym::T)),
            ("comment-multi-line", Value::Nil),
            ("comment-empty-lines", Value::Nil),
            ("comment-fill-column", Value::Nil),
            ("comment-use-syntax", Value::Sym(self.q("undecided"))),
            ("fill-nospace-between-words", Value::Sym(sym::T)),
            ("auto-fill-function", Value::Nil),
            (
                "normal-auto-fill-function",
                Value::Sym(self.q("do-auto-fill")),
            ),
            ("adaptive-fill-mode", Value::Sym(sym::T)),
            (
                "adaptive-fill-regexp",
                Value::string("[-–!|#%;>*·•‣⁃◦ \t]*"),
            ),
            // GNU fill.el: `adaptive-fill-function' defaults to `ignore'.
            ("adaptive-fill-function", Value::Sym(self.q("ignore"))),
            (
                "adaptive-fill-first-line-regexp",
                Value::string("\\`[ \t]*\\'"),
            ),
            ("kill-do-not-save-duplicates", Value::Nil),
            ("kill-read-only-ok", Value::Nil),
            ("kill-transform-function", Value::Nil),
            ("yank-handled-properties", Value::Nil),
            ("set-mark-command-repeat-pop", Value::Nil),
            ("yank-pop-change-selection", Value::Nil),
            ("x-select-request-type", Value::Nil),
            ("search-invisible", Value::Sym(self.q("open"))),
            ("search-whitespace-regexp", Value::string("[ \t]+")),
            ("search-nonincremental-instead", Value::Sym(sym::T)),
            ("isearch-hide-immediately", Value::Sym(sym::T)),
            ("isearch-resume-in-command-history", Value::Nil),
            ("isearch-allow-prefix", Value::Sym(sym::T)),
            ("isearch-push-state-function", Value::Nil),
            ("lazy-highlight-initial-delay", Value::float(0.25)),
            ("lazy-highlight-interval", Value::Int(0)),
            ("lazy-highlight-max-at-a-time", Value::Nil),
            ("lazy-highlight-cleanup", Value::Sym(sym::T)),
            ("lazy-highlight-no-delay-length", Value::Int(3)),
            // GNU replace.el: " → " (U+2192 with spaces).
            ("query-replace-from-to-separator", Value::string(" → ")),
            ("query-replace-lazy-highlight", Value::Sym(sym::T)),
            ("query-replace-show-replacement", Value::Sym(sym::T)),
            ("query-replace-skip-read-only", Value::Nil),
            ("query-replace-highlight", Value::Sym(sym::T)),
            (
                "recenter-positions",
                Value::list(vec![
                    Value::Sym(self.q("middle")),
                    Value::Sym(self.q("top")),
                    Value::Sym(self.q("bottom")),
                ]),
            ),
            ("handle-shift-selection", Value::Nil),
            ("read-buffer-function", Value::Nil),
            ("read-expression-map", Value::Nil),
            ("read-circle", Value::Sym(sym::T)),
            ("debugger", Value::Sym(self.q("debug"))),
            ("debug-ignored-errors", Value::Nil),
            ("signal-hook-function", Value::Nil),
            ("delayed-warnings-list", Value::Nil),
            // GNU subr.el defvar: defaults to the two warning helpers.
            (
                "delayed-warnings-hook",
                Value::list(vec![
                    Value::Sym(self.q("collapse-delayed-warnings")),
                    Value::Sym(self.q("display-delayed-warnings")),
                ]),
            ),
            ("garbage-collection-messages", Value::Nil),
            ("post-gc-hook", Value::Nil),
            ("gc-message-percentage", Value::Nil),
            ("user-emacs-directory", Value::string("~/.emacs.d/")),
            ("native-comp-driver-options", Value::Nil),
            (
                "module-file-suffix",
                Value::string(if cfg!(target_os = "macos") {
                    ".dylib"
                } else {
                    ".so"
                }),
            ),
            ("dynamic-library-alist", Value::Nil),
            // GNU xdisp.c/xfaces.c defaults verified -Q: t /
            // `completions-highlight' (not nil / t).
            ("inhibit-x-resources", Value::Sym(sym::T)),
            (
                "completions-highlight-face",
                Value::Sym(self.q("completions-highlight")),
            ),
            // GNU's pre-redisplay-functions -Q value (window.el installs
            // this hook function in its dump).
            (
                "pre-redisplay-functions",
                Value::list(vec![Value::Sym(
                    self.q("redisplay--update-region-highlight"),
                )]),
            ),
            ("load-prefer-newer", Value::Nil),
            ("load-read-function", Value::Sym(self.q("read"))),
            (
                "bytecomp-version-regexp",
                Value::string(r"^;;;.\(in Emacs version\|bytecomp version FSF\)"),
            ),
            ("obarray-size", Value::Int(15121)),
            ("max-image-size", Value::float(10.0)),
            ("image-scaling-factor", Value::Sym(self.q("auto"))),
            ("use-short-answers", Value::Nil),
            ("yes-or-no-prompt", Value::string("(yes or no) ")),
            ("async-shell-command-display-buffer", Value::Sym(sym::T)),
            (
                "shell-command-default-error-buffer",
                Value::string("*Shell Command Error*"),
            ),
            ("shell-command-prompt-show-cwd", Value::Nil),
            (
                "exec-suffixes",
                // GNU: '("") on non-DOS/Windows systems.
                Value::list(vec![Value::string("")]),
            ),
            ("process-connection-type", Value::Sym(sym::T)),
            ("delete-exited-processes", Value::Sym(sym::T)),
            ("process-adaptive-read-buffering", Value::Sym(sym::T)),
            ("process-error-pause-time", Value::Int(0)),
            ("network-security-level", Value::Sym(self.q("medium"))),
            ("mail-host-address", Value::Nil),
            (
                "emacs-copyright",
                Value::string("Copyright (C) 2025 Free Software Foundation, Inc."),
            ),
            ("emacs-build-time", Value::Nil),
            ("emacs-build-system", Value::Nil),
            ("emacs-repository-version", Value::string("emacs-31.1")),
            ("emacs-repository-branch", Value::string("master")),
            ("site-run-file", Value::Nil),
            ("installation-directory", Value::Nil),
            ("invocation-name", Value::string("remacs")),
            ("invocation-directory", Value::Nil),
            (
                "source-directory",
                Value::string(env!("REMACS_SOURCE_ROOT")),
            ),
            ("emacs-program-version", Value::string("31.1")),
            ("emacs-major-version", Value::Int(31)),
            ("keyboard-type", Value::Sym(self.q("pc"))),
            ("window-system", Value::Nil),
            ("initial-window-system", Value::Nil),
            ("daemon-socket", Value::Nil),
            ("internal--daemon-sockname", Value::Nil),
            ("glyph-table", Value::Nil),
            (
                "charset-list",
                // GNU `Vcharset_list' is Fcons-prepend order: only the
                // five C builtins live here at init, oldest-first at
                // the tail (charset.c init order: ascii, iso-8859-1,
                // unicode, emacs, eight-bit); every later
                // `define-charset-internal' prepends.
                Value::list(
                    ["eight-bit", "emacs", "unicode", "iso-8859-1", "ascii"]
                        .iter()
                        .map(|n| Value::Sym(self.intern(n)))
                        .collect(),
                ),
            ),
            ("charset-map-path", Value::Nil),
            (
                "char-code-property-alist",
                self.seed_char_code_property_alist(),
            ),
            ("unicode-category-table", Value::Nil),
            ("current-language-environment", Value::string("English")),
            ("default-input-method", Value::Nil),
            ("input-method-verbose-flag", Value::Sym(self.q("default"))),
            ("input-method-highlight-flag", Value::Sym(sym::T)),
            ("input-method-activate-hook", Value::Nil),
            ("input-method-inactivate-hook", Value::Nil),
            ("input-method-use-echo-area", Value::Nil),
            ("find-file-hook", Value::Nil),
            ("write-file-functions", Value::Nil),
            ("after-save-hook", Value::Nil),
            ("before-save-hook", Value::Nil),
            ("save-buffer-coding-system", Value::Nil),
            ("file-coding-system-alist", Value::Nil),
            ("process-coding-system-alist", Value::Nil),
            ("network-coding-system-alist", Value::Nil),
            (
                "file-name-coding-system",
                Value::Sym(self.q("utf-8-hfs-unix")),
            ),
            (
                "default-file-name-coding-system",
                Value::Sym(self.q("utf-8-unix")),
            ),
            (
                "default-terminal-coding-system",
                Value::Sym(self.q("utf-8-unix")),
            ),
            ("file-name-shadow-mode", Value::Sym(sym::T)),
            (
                "show-help-function",
                Value::Sym(self.q("tooltip-show-help")),
            ),
            ("delete-trailing-lines", Value::Sym(sym::T)),
            // GNU -Q: `keyboard-coding-system' is nil (the *terminal's*
            // kbd coding — `keyboard-coding-system' the subr — is what
            // carries no-conversion/utf-8-unix).  Seeding the var with
            // utf-8-unix made mule.el's defcustom :set run
            // `set-keyboard-coding-system' on the seed at load, which
            // overrode the locale-derived no-conversion under LC_ALL=C.
            // `set-locale-environment' assigns the real value at boot.
            ("keyboard-coding-system", Value::Nil),
            (
                "default-process-coding-system",
                Value::cons(
                    Value::Sym(self.q("utf-8-unix")),
                    Value::Sym(self.q("utf-8-unix")),
                ),
            ),
            ("emacs-basic-display", Value::Nil),
            // GNU's files.el/mule.el initial contents (verified at -Q).
            ("auto-coding-alist", Value::Nil), // set from Lisp below
            ("auto-coding-functions", Value::Nil),
            ("auto-coding-regexp-alist", Value::Nil), // set below
            ("set-auto-coding-function", Value::Nil),
            // GNU's coding.c + mule/mule-conf populate
            // Vcoding_system_list with every defined name INCLUDING
            // `-mac'/`-dos'/`-unix' subsidiaries and aliases, in dump
            // order; `coding-system-list' (the defun) iterates it.
            // Seeded from a GNU 31.1 -Q dump verbatim (see csl_table.rs).
            (
                "coding-system-list",
                Value::list(
                    crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED
                        .iter()
                        .map(|n| Value::Sym(self.intern(n)))
                        .collect(),
                ),
            ),
            ("coding-category-list", Value::Nil),
            // GNU's coding.c: `Vlatin_extra_code_table =
            // make_nil_vector (256)'; mule-conf.el `aset's into it.
            (
                "latin-extra-code-table",
                Value::vector(vec![Value::Nil; 256]),
            ),
            ("translation-table-for-input", Value::Nil),
            ("file-name-handler-alist", Value::Nil),
            ("directory-free-space-program", Value::string("df")),
            ("directory-free-space-args", Value::string("-k")),
            // GNU 31 removed `directory-sep-char' — `void-variable' at -Q.
            ("insert-directory-program", Value::string("ls")),
            ("auto-save-list-file-prefix", Value::Nil),
            ("auto-save-visited-file-name", Value::Nil),
            ("auto-save-mode", Value::Nil),
            ("delete-auto-save-files", Value::Sym(sym::T)),
            ("auto-save-include-big-deletions", Value::Nil),
            ("buffer-offer-save", Value::Nil),
            ("kept-new-versions", Value::Int(2)),
            ("kept-old-versions", Value::Int(2)),
            ("dired-kept-versions", Value::Int(2)),
            ("remote-file-name-inhibit-cache", Value::Int(10)),
            ("file-precious-flag", Value::Nil),
            ("view-read-only", Value::Nil),
            ("ask-about-buffer-names", Value::Nil),
            ("large-file-warning-threshold", Value::Int(10000000)),
            ("write-region-inhibit-fsync", Value::Sym(sym::T)),
            ("write-region-annotate-functions", Value::Nil),
            ("write-region-post-annotation-function", Value::Nil),
            ("write-region-annotations-so-far", Value::Nil),
            ("after-insert-file-set-coding-functions", Value::Nil),
            ("before-load-history", Value::Nil),
            ("after-load-functions", Value::Nil),
            ("load-force-doc-strings", Value::Nil),
            ("load-convert-to-unibyte", Value::Nil),
            ("load-dangerous-libraries", Value::Nil),
            ("byte-compile-warnings", Value::Sym(sym::T)),
            ("byte-compile-dynamic", Value::Nil),
            ("byte-compile-dynamic-docstrings", Value::Sym(sym::T)),
            ("byte-compile-verbose", Value::Nil),
            ("byte-compile-optimize", Value::Sym(sym::T)),
            ("byte-compile-delete-errors", Value::Nil),
            ("byte-compile-generate-call-tree", Value::Nil),
            ("byte-compile-call-tree-sort", Value::Nil),
            ("byte-compile-debug", Value::Nil),
            ("byte-compile-error-on-warn", Value::Nil),
            ("byte-compile-docstring-max-column", Value::Int(80)),
            ("byte-compile-cond-use-jump-table", Value::Sym(sym::T)),
            ("ad-default-compilation-action", Value::Sym(self.q("maybe"))),
            ("read-symbol-positions-list", Value::Nil),
            ("eval-expression-print-maximum-character", Value::Int(127)),
            ("print-integers-as-characters", Value::Nil),
            ("print-escape-control-characters", Value::Nil),
            ("print-charset-text-property", Value::Sym(self.q("default"))),
            ("print-unreadable-function", Value::Nil),
            ("eval-depth", Value::Int(0)),
            (
                // Bootstrap without closure optimization; loadup installs
                // cconv's constructor after the dumped libraries are ready.
                "internal-make-interpreted-closure-function",
                Value::Nil,
            ),
            ("macroexp--debug-eager", Value::Nil),
            ("macroexpand-all-environment", Value::Nil),
            ("functions-exhausted", Value::Nil),
            ("fontification-functions", Value::Nil),
            ("fontification-error", Value::Nil),
            ("fontification-missing", Value::Nil),
            ("inhibit-changing-match-data", Value::Nil),
            ("search-slow-speed", Value::Int(1200)),
            ("search-slow-window-lines", Value::Int(1)),
            ("regexp-search-ring-max", Value::Int(16)),
            ("search-ring-max", Value::Int(16)),
            ("search-ring-yank-pointer", Value::Nil),
            ("regexp-search-ring-yank-pointer", Value::Nil),
            ("search-exit-option", Value::Sym(sym::T)),
            ("command-line-args", Value::Nil),
            ("command-line-processed", Value::Sym(sym::T)),
            ("command-switch-alist", Value::Nil),
            ("command-line-functions", Value::Nil),
            // GNU init_cmdargs: `Vcommand_line_default_directory' is
            // the cwd at startup (with trailing slash) — same value
            // `default-directory' is seeded with.
            (
                "command-line-default-directory",
                Value::string({
                    // Same logical-cwd rule as `default-directory'
                    // (`emacs_wd' prefers $PWD when stat-equal).
                    let mut d = crate::editor::emacs_wd();
                    if !d.ends_with('/') {
                        d.push('/');
                    }
                    d
                }),
            ),
            ("command-line-args-left", Value::Nil),
            ("command-line-normalized-file-name", Value::Nil),
            ("emacs-startup-hook", Value::Nil),
            ("term-setup-hook", Value::Nil),
            ("window-setup-hook", Value::Nil),
            ("before-init-hook", Value::Nil),
            ("after-init-hook", Value::Nil),
            ("init-file-debug", Value::Nil),
            ("init-file-user", Value::Nil),
            ("user-init-file", Value::Nil),
            ("inhibit-startup-echo-area-message", Value::Nil),
            ("inhibit-default-init", Value::Nil),
            ("inhibit-startup-buffer-menu", Value::Nil),
            ("initial-buffer-choice", Value::Nil),
            ("initial-scratch-message", Value::Nil),
            ("fancy-startup-text", Value::Nil),
            ("fancy-about-text", Value::Nil),
            ("fancy-splash-image", Value::Nil),
            ("fancy-startup-tail", Value::Nil),
            ("fancy-about-tail", Value::Nil),
            ("menu-bar-mode", Value::Sym(sym::T)),
            ("tool-bar-mode", Value::Nil),
            ("tab-bar-mode", Value::Nil),
            ("scroll-bar-mode", Value::Sym(self.q("right"))),
            ("horizontal-scroll-bar-mode", Value::Nil),
            ("default-frame-alist", Value::Nil),
            ("initial-frame-alist", Value::Nil),
            ("minibuffer-frame-alist", Value::Nil),
            ("default-frame-scroll-bars", Value::Sym(self.q("right"))),
            ("frame-inherited-parameters", Value::Nil),
            ("frame-initial-frame", Value::Nil),
            ("frame-initial-frame-alist", Value::Nil),
            ("frame-initial-geometry-arguments", Value::Nil),
            ("frame-title-format", Value::Nil),
            ("icon-title-format", Value::Nil),
            ("multiple-frames", Value::Nil),
            ("delete-frame-functions", Value::Nil),
            ("after-make-frame-functions", Value::Nil),
            ("before-make-frame-hook", Value::Nil),
            ("menu-bar-final-items", Value::Nil),
            ("menu-bar-update-hook", Value::Nil),
            ("menu-bar-select-buffer-function", Value::Nil),
            ("mouse-position-function", Value::Nil),
            ("mouse-highlight", Value::Sym(sym::T)),
            ("mouse-autoselect-window", Value::Nil),
            ("mouse-drag-copy-region", Value::Nil),
            ("mouse-1-click-follows-link", Value::Sym(self.q("double"))),
            ("mouse-1-click-in-non-selected-windows", Value::Sym(sym::T)),
            ("mouse-wheel-scroll-amount", Value::Nil),
            ("mouse-wheel-scroll-amount-horizontal", Value::Nil),
            ("mouse-wheel-tilt-scroll", Value::Nil),
            ("mouse-wheel-flip-direction", Value::Nil),
            ("mouse-wheel-progressive-speed", Value::Sym(sym::T)),
            ("mouse-wheel-follow-mouse", Value::Sym(sym::T)),
            // `mouse-wheel-mode' itself is NOT preset: GNU reaches t via
            // mwheel.el's `define-minor-mode' (:init-value t) when loadup
            // loads the library — remacs loads it after fontset below.
            // Presetting t here made every late `(require 'subr-x)' /
            // mwheel load recurse: defcustom `:set' saw t, called the
            // still-autoloaded `mouse-wheel-mode', which pulled mwheel
            // while it was already loading.
            ("scroll-bar-adjust-thumb-portion", Value::Sym(sym::T)),
            ("x-stretch-cursor", Value::Nil),
            ("x-use-underline-position-properties", Value::Nil),
            ("x-underline-at-descent-line", Value::Nil),
            ("x-mouse-clip-rectangular", Value::Sym(sym::T)),
            (
                "shell-file-name",
                // GNU init_callproc uses SHELL, including an empty value.
                // ESHELL selects an interactive shell, not this default.
                environment::shell(),
            ),
            ("path-separator", Value::string(":")),
            ("null-device", Value::string("/dev/null")),
            (
                "temporary-file-directory",
                environment::temporary_directory(),
            ),
            ("exec-directory", Value::string("/usr/local/bin/")),
            (
                "doc-directory",
                // Locate the host GNU Emacs's etc/ dir (which holds the
                // DOC file) via the `emacs' on PATH; fall back to the
                // conventional share path.
                environment::documentation_directory(),
            ),
            (
                "configure-info-directory",
                // GNU: PATH_INFO from configure (`<prefix>/share/info').
                environment::info_directory(),
            ),
            (
                "initial-major-mode",
                Value::Sym(self.q("lisp-interaction-mode")),
            ),
            (
                "initial-scratch-message",
                // GNU startup.el stores the RAW `\[find-file]' form;
                // `get-scratch-buffer-create' runs it through
                // substitute-command-keys at insertion time, which adds
                // the `...' quoting and help-key-binding text props.
                Value::string(
                    ";; This buffer is for text that is not saved, and for Lisp evaluation.\n;; To create a file, visit it with `\\[find-file]' and enter text in its buffer.\n\n",
                ),
            ),
            ("inhibit-startup-screen", Value::Nil),
            (
                // GNU init_editfns: getenv("NAME") if set, else the
                // GECOS full name of the effective user (getpwuid when
                // the claimed env name matches the real one).
                "user-full-name",
                Value::string(crate::lisp::builtins::misc::boot_user_full_name()),
            ),
            (
                // GNU init_editfns: LOGNAME, then USER, then
                // getpwuid(geteuid()).
                "user-login-name",
                Value::string(crate::lisp::builtins::misc::boot_user_login_name()),
            ),
            ("user-mail-address", Value::string("user@localhost")),
            (
                // getpwuid(getuid()) — environment ignored.
                "user-real-login-name",
                Value::string(crate::lisp::builtins::misc::boot_user_real_login_name()),
            ),
            (
                "user-uid",
                Value::Int(crate::lisp::builtins::misc::boot_geteuid()),
            ),
            (
                "user-real-uid",
                Value::Int(crate::lisp::builtins::misc::boot_getuid()),
            ),
            ("history-add-new-input", Value::t()),
            ("save-abbrevs", Value::t()),
            ("debug-on-error", Value::Nil),
            ("sentence-end-double-space", Value::t()),
            ("inhibit-startup-message", Value::t()),
            // GNU's defcustom default is t (cus-start.el).
            ("use-dialog-box", Value::Sym(sym::T)),
            ("menu-prompting", Value::Nil),
            (
                "minibuffer-prompt-properties",
                Value::list(vec![
                    Value::Sym(self.q("read-only")),
                    Value::Sym(sym::T),
                    Value::Sym(self.q("face")),
                    Value::Sym(self.q("minibuffer-prompt")),
                ]),
            ),
            (
                "completion-styles",
                Value::list(vec![Value::Sym(self.q("basic"))]),
            ),
        ]
    }
}
