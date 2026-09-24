//! The evaluator: `Interp` state, `eval`, `apply`, and special forms.
//!
//! Scoping: elisp is dynamically scoped by default. Dynamic `let` is
//! implemented exactly like Emacs's specbind: save the symbol's current
//! value cell, overwrite, restore on unwind — so lookup stays O(1).
//! When `lexical-binding` is on, `let` binds in a captured lexical env
//! chain instead (unless the symbol is `defvar`'d special).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::error::{EvalResult, Flow};
use super::obarray::Obarray;
use super::obarray::sym;
use super::reader::Reader;
use super::value::{Arity, Lambda, Marker, SymId, Value};

/// A saved dynamic binding (specbind entry).
struct SpecBind {
    sym: SymId,
    /// `Some(buf)` = binding is buffer-local in `buf`; `None` = global cell.
    buf: Option<usize>,
    /// Previous value (`None` = no local binding existed).
    old: Option<Value>,
}

/// A lexical environment frame (chained).
pub struct LexFrame {
    pub vars: RefCell<HashMap<SymId, Value>>,
    pub parent: Option<Rc<LexFrame>>,
}

pub type LexEnv = Option<Rc<LexFrame>>;

pub fn lexenv_lookup(mut env: &LexEnv, sym: SymId) -> Option<Value> {
    while let Some(frame) = env {
        if let Some(v) = frame.vars.borrow().get(&sym) {
            return Some(v.clone());
        }
        env = &frame.parent;
    }
    None
}

/// Where output from `princ`/`print`/`message` goes. The editor installs a
/// real sink; tests use the buffer.
pub enum OutputSink {
    /// Accumulate into a string (tests, `with-output-to-string`).
    Buffer(Rc<RefCell<String>>),
    /// Call a Lisp function with each printed string.
    Function(Value),
    /// Write to process stdout (batch mode).
    Stdout,
}

/// The interpreter: all global Lisp state. Lives on the eval thread.
pub struct Interp {
    pub obarray: Obarray,
    specbind: Vec<SpecBind>,
    /// Current lexical environment (non-nil only under lexical-binding).
    pub lexenv: LexEnv,
    /// The `lexical-binding` default (buffer-local in Emacs; we keep a
    /// default plus per-buffer locals).
    pub buffers: crate::buffer::BufferSet,
    pub current_buffer: usize,
    /// Symbols provided via `provide` (the `features` variable).
    pub features: Vec<SymId>,
    /// Output destination for princ/print/message.
    pub output: Option<OutputSink>,
    /// Last thing shown in the echo area (tests inspect this).
    pub echo_message: String,
    /// Read position for `read` from a buffer/stream — kept simple.
    pub max_lisp_eval_depth: usize,
    pub eval_depth: usize,
    /// Depth of explicit `(eval ...)' calls — lambdas built inside get a
    /// `nil' environment marker like Emacs.
    pub explicit_eval_depth: usize,
    /// C-g sets this; eval checks it between forms.
    pub quit_flag: bool,
    /// Values pushed by `throw` for debugging — not needed.
    pub catch_tags: Vec<Value>,
    /// Set by `append-next-kill': the next kill appends to the last
    /// kill-ring entry.
    pub append_next_kill: bool,
    /// `inhibit-read-only` dynamic override.
    pub standard_output_sym: SymId,
    /// SymId of `undo-inhibit-record-point' — its dynamic value is
    /// mirrored into `buffers.undo_inhibit' so `record_point' (which
    /// runs without interpreter access) can consult it.
    pub undo_inhibit_sym: SymId,
    /// Command-loop state used by `interactive` specs.
    pub command_args: Vec<Value>,
    /// The symbol currently being defined by `defun` for recursion.
    pub defining_symbol: Option<SymId>,
    /// History list for `M-x` etc.
    pub read_expression_history: Value,
    /// Eval depth counters for `recursive-edit`.
    pub recursion_depth: usize,
    /// `overriding-terminal-local-map` equivalent — keymap override.
    pub executing_kbd_macro: bool,
    /// Records most-recent error for `error-message-string` debugging.
    pub last_error: Option<(Value, Value)>,
    /// Accumulator while `with-output-to-string` captures output.
    pub output_buffer: String,
    /// When true, `write_output` appends to `output_buffer`.
    pub capture_output: bool,
    /// Registers from the last search (`match-data`).
    pub match_data: Option<MatchData>,
    /// True in `--batch`: `princ` goes to stdout, `message` to stderr.
    pub noninteractive: bool,
    /// GNU's `noninteractive_need_newline`: set when batch stdout was
    /// written, so the next stderr message is preceded by a newline.
    pub stderr_need_newline: bool,
    /// Last character written to the real output sink; used by
    /// `(terpri nil t)'-style BOL checks (GNU's print_position).
    pub out_last_char: Option<char>,
    /// All frames (the first is the initial tty frame).
    pub frames: Vec<crate::editor::FrameRef>,
    /// The selected frame.
    pub selected_frame: Option<crate::editor::FrameRef>,
    /// Set by `kill-emacs` to exit the command loop.
    pub quit_editor: bool,
    /// Interactive input hook installed by the terminal front-end.
    /// Called as (interp, prompt, single_key) -> MinibufInput.
    /// `single_key` reads one event (y-or-n-p, read-char).
    pub minibuf_reader:
        Option<std::rc::Rc<dyn Fn(&mut Interp, &str, bool) -> Result<MinibufInput, Flow>>>,
    /// Nesting depth of active minibuffer reads (`minibuffer-depth').
    pub minibuf_level: i32,
    /// User-defined faces: name → plist of attribute keywords.
    /// Built-in faces (default, bold, italic, …) live in a static table.
    pub face_table: Vec<(String, Value)>,
    /// Live process objects (`Value::Process`), including finished ones
    /// until `delete-process`.
    pub processes: Vec<crate::lisp::value::ProcessRef>,
    /// Charset name → plist (`define-charset` / `set-charset-plist`).
    pub charsets: Vec<(String, Value)>,
    /// Charset alias → canonical name (`define-charset-alias`).
    pub charset_aliases: Vec<(String, String)>,
    /// Coding systems defined via `define-coding-system-internal`.
    pub extra_coding_systems: Vec<String>,
    /// Current `terminal-coding-system' value (nil after set-nil).
    pub terminal_coding: Value,
    /// Current `keyboard-coding-system' value (no-conversion after
    /// set-nil).
    pub keyboard_coding: Value,
    /// Key events queued by `execute-kbd-macro`; the key loop replays
    /// them ahead of real input.
    pub macro_replay: std::collections::VecDeque<i128>,
    /// Whether the key currently being dispatched came from macro
    /// replay (replayed keys are not re-recorded).
    pub macro_replaying: bool,
    /// Events recorded for the keyboard macro being defined.
    pub kbd_macro_events: Vec<Value>,
    /// `kbd_macro_events` length at the last command boundary; used by
    /// `cancel-kbd-macro-events`.
    pub kbd_macro_mark: usize,
    /// char-code property name → per-char entries (`put-char-code-property`).
    pub char_code_props: Vec<(String, Vec<(i64, Value)>)>,
    /// char-code property name → char-table backing store.
    pub char_code_prop_tables: Vec<(String, Value)>,
    /// Cycle phase for `move-to-window-line-top-bottom' repeats.
    pub mtwlb_phase: u8,
    /// Path of the dribble file opened by `open-dribble-file'.
    pub dribble_file: Option<String>,
    /// `handler-bind' dynamic chain: (CONDITIONS . HANDLER) pairs,
    /// innermost last. Consulted when a signal is raised uncaught.
    pub handler_bindings: Vec<(Value, Value)>,
    /// Condition lists of `condition-case' forms currently evaluating
    /// their bodies — a signal matching any of them is "caught".
    pub case_handlers: Vec<Value>,
    /// `add-variable-watcher' registry: (SYM . FUNCTION) pairs.
    pub var_watchers: Vec<(SymId, Value)>,
    /// All thread objects ever created (`all-threads'); element 0 is
    /// the main thread.
    pub threads: Vec<crate::lisp::value::ThreadRef>,
    /// Index into `threads` of the currently running thread.
    pub current_thread: usize,
    /// `thread-last-error' state: the (sym . data) condition of the
    /// most recent thread function failure.
    pub thread_last_error: Value,
    /// The standard case table (`standard-case-table'), built lazily.
    pub standard_case_table: Option<Value>,
    /// The standard category table (`standard-category-table'), built
    /// lazily with GNU's ASCII defaults.
    pub standard_category_table: Option<Value>,
    /// `register-ccl-program' registration counter.
    pub ccl_program_count: usize,
    /// `register-code-conversion-map' registration counter.
    pub code_conv_map_count: usize,
    /// `profiler-memory-running-p' state — our memory profiler is a
    /// bookkeeping-only stub, but start/stop toggle it like GNU.
    pub memory_profiler: bool,
    /// `advice-add' registry: SYM → ordered (WHERE . (FUN . NAME))
    /// entries. Consulted by `apply' when calling SYM.
    pub advices: Vec<(SymId, Vec<(SymId, Value, Value)>)>,
    /// Generated trampolines for advice composition: index →
    /// (WHERE . (ADVICE-FUN . NEXT-CALLABLE)) as a flat triple.
    pub advice_links: Vec<(Value, Value, Value)>,
    /// `set-char-table-parent' registry: record identity → parent table.
    /// Char-table parents live outside the record so existing record
    /// layouts are untouched.
    pub char_table_parents: Vec<(usize, Value)>,
    /// Char-table default values, same registry style as parents
    /// (record identity → defalt).  GNU has no Lisp accessor for the
    /// defalt; it shows in `#^[...]' printing and feeds
    /// `char-table-range' misses.
    pub char_table_defalts: Vec<(usize, Value)>,
    /// Fingerprint seen by the last `frame-or-buffer-changed-p' call.
    pub frame_state_seen: Option<u64>,
    /// Live Lisp call frames `(FUNCTION . ARGS)', outermost first.
    /// Pushed by `apply' so `mapbacktrace'/backtrace internals can
    /// walk the stack like GNU's specpdl entries do.
    pub lisp_stack: Vec<(Value, Vec<Value>)>,
    /// `profiler-cpu-running-p' state flag (no real sampler).
    pub cpu_profiler: bool,
    /// The sole terminal object (`terminal-list', `frame-terminal'):
    /// a lazily-created record `#s(terminal 0 "initial_terminal")'.
    pub terminal: Option<Value>,
    /// `terminal-parameter'/`set-terminal-parameter' alist, seeded
    /// with GNU's tty defaults.
    pub terminal_params: Vec<(Value, Value)>,
    /// Set once `x-open-connection'/`x-close-connection' ran; GNU's
    /// `xw-*'/`x-color-values' then work even though no real X
    /// display exists.
    pub x_display_attempted: bool,
    /// `tty-color-values' was called — GNU's tty color database also
    /// initializes `x-color-values' (but not the `xw-*' functions).
    pub color_db_init: bool,
    /// StrRef identity → liveness for strings produced by unibyte
    /// encoders (`encode-coding-char'/`-string').  `prin1' prints
    /// byte-chars ≥0x80 of marked strings as `\NNN' octal escapes
    /// like GNU's unibyte strings.  The Weak guards against address
    /// reuse: a dead entry never upgrades, so a fresh allocation at
    /// the same address is never mis-marked.
    pub unibyte_strings:
        std::collections::HashMap<usize, std::rc::Weak<std::cell::RefCell<String>>>,
    /// Strings produced by multibyte decoders — `multibyte-string-p'
    /// is t for them even when their contents are pure ASCII.
    pub multibyte_strings:
        std::collections::HashMap<usize, std::rc::Weak<std::cell::RefCell<String>>>,
    /// Text properties attached to string objects: pointer identity →
    /// (weak backref, sorted disjoint intervals (start, end, plist)).
    /// GNU keeps dead interval boundaries after property removal, so
    /// a true interval model is needed (not last-write-wins).  The
    /// Weak guards against address reuse like the unibyte map.
    pub string_props: std::collections::HashMap<
        usize,
        (
            std::rc::Weak<std::cell::RefCell<String>>,
            Vec<(usize, usize, Vec<Value>)>,
        ),
    >,
}

/// Result of a minibuffer read from the front-end.
pub enum MinibufInput {
    /// A completed input line.
    Text(String),
    /// A single raw key event code (modifier bits included).
    Key(i128),
}

/// `<share>/emacs/<ver>/etc/` if it contains a DOC file.
fn doc_dir_under(emacs_share: &std::path::Path) -> Option<String> {
    let rd = std::fs::read_dir(emacs_share).ok()?;
    for e in rd.flatten() {
        let etc = e.path().join("etc");
        if etc.join("DOC").is_file() {
            return Some(format!("{}/", etc.display()));
        }
    }
    None
}

/// Find `<prefix>/share/emacs/<ver>/etc/` (containing DOC) for the
/// emacs binary at EXE. Wrapper scripts (`exec /real/emacs`, as in
/// Nixpkgs' emacsWithPackages) are unwrapped up to DEPTH hops.
fn doc_dir_of_emacs(exe: &std::path::Path, depth: u8) -> Option<String> {
    let exe = std::fs::canonicalize(exe).ok()?;
    if let Some(prefix) = exe.parent().and_then(|p| p.parent()) {
        if let Some(hit) = doc_dir_under(&prefix.join("share/emacs")) {
            return Some(hit);
        }
        // Nixpkgs with-packages wrappers symlink share/* into the real
        // emacs package — follow any of them to the real prefix.
        if let Ok(rd) = std::fs::read_dir(prefix.join("share")) {
            for e in rd.flatten() {
                if let Ok(real) = std::fs::canonicalize(e.path()) {
                    if let Some(share) = real.parent() {
                        if let Some(hit) =
                            doc_dir_under(&share.join("emacs"))
                        {
                            return Some(hit);
                        }
                    }
                }
            }
        }
    }
    if depth < 5 {
        if let Ok(src) = std::fs::read_to_string(&exe) {
            for line in src.lines() {
                let Some(rest) = line.trim_start().strip_prefix("exec ") else {
                    continue;
                };
                let Some(target) = rest.split_whitespace().next() else {
                    continue;
                };
                let cand = std::path::PathBuf::from(target);
                if cand.file_name().map(|n| n == "emacs").unwrap_or(false) {
                    if let Some(hit) = doc_dir_of_emacs(&cand, depth + 1) {
                        return Some(hit);
                    }
                }
            }
        }
    }
    None
}

impl Interp {
    pub fn new() -> Interp {
        let mut obarray = Obarray::new();
        let standard_output_sym = obarray.intern("standard-output");
        let emacs_sym = obarray.intern("emacs");
        // GNU batch's feature list (NS tty build ordering);
        // `provide' appends to this.
        let feature_syms: Vec<SymId> = [
            "japan-util", "rmc", "iso-transl", "tooltip", "cconv", "eldoc",
            "paren", "electric", "uniquify", "ediff-hook", "vc-hooks",
            "lisp-float-type", "elisp-mode", "mwheel", "term/ns-win", "ns-win",
            "ucs-normalize", "mule-util", "term/common-win", "tool-bar", "dnd",
            "fontset", "image", "regexp-opt", "fringe", "tabulated-list",
            "replace", "newcomment", "text-mode", "lisp-mode", "prog-mode",
            "register", "page", "tab-bar", "menu-bar", "rfn-eshadow", "isearch",
            "easymenu", "timer", "select", "scroll-bar", "mouse", "jit-lock",
            "font-lock", "syntax", "font-core", "term/tty-colors", "frame",
            "minibuffer", "nadvice", "seq", "simple", "cl-generic",
            "indonesian", "philippine", "cham", "georgian", "utf-8-lang",
            "misc-lang", "vietnamese", "tibetan", "thai", "tai-viet", "lao",
            "korean", "japanese", "eucjp-ms", "cp51932", "hebrew", "greek",
            "romanian", "slovak", "czech", "european", "ethiopic", "indian",
            "cyrillic", "chinese", "composite", "emoji-zwj", "charscript",
            "charprop", "case-table", "epa-hook", "jka-cmpr-hook", "help",
            "abbrev", "obarray", "oclosure", "cl-preloaded", "button",
            "loaddefs", "theme-loaddefs", "faces", "cus-face", "macroexp",
            "files", "window", "text-properties", "overlay", "sha1", "md5",
            "base64", "format", "env", "code-pages", "mule", "custom",
            "widget", "keymap", "hashtable-print-readable", "backquote",
            "threads", "kqueue", "cocoa", "ns", "multi-tty",
            "make-network-process", "tty-child-frames", "native-compile",
        ]
        .iter()
        .map(|s| obarray.intern(s))
        .chain(std::iter::once(emacs_sym))
        .collect();
        let dump_features = feature_syms.clone();
        let mut interp = Interp {
            obarray,
            specbind: Vec::new(),
            lexenv: None,
            buffers: crate::buffer::BufferSet::new(),
            current_buffer: 0,
            features: feature_syms,
            output: None,
            echo_message: String::new(),
            max_lisp_eval_depth: 1600,
            eval_depth: 0,
            explicit_eval_depth: 0,
            quit_flag: false,
            catch_tags: Vec::new(),

            append_next_kill: false,
            standard_output_sym,
            undo_inhibit_sym: 0,
            command_args: Vec::new(),
            defining_symbol: None,
            read_expression_history: Value::Nil,
            recursion_depth: 0,
            executing_kbd_macro: false,
            last_error: None,
            output_buffer: String::new(),
            capture_output: false,
            match_data: None,
            noninteractive: false,
            stderr_need_newline: false,
            out_last_char: None,
            frames: Vec::new(),
            selected_frame: None,
            quit_editor: false,
            minibuf_reader: None,
            minibuf_level: 0,
            face_table: Vec::new(),
            processes: Vec::new(),
            charsets: Vec::new(),
            charset_aliases: Vec::new(),
            extra_coding_systems: Vec::new(),
            terminal_coding: Value::Nil,
            keyboard_coding: Value::Nil,
            macro_replay: std::collections::VecDeque::new(),
            macro_replaying: false,
            kbd_macro_events: Vec::new(),
            kbd_macro_mark: 0,
            char_code_props: Vec::new(),
            char_code_prop_tables: Vec::new(),
            mtwlb_phase: 0,
            dribble_file: None,
            handler_bindings: Vec::new(),
            case_handlers: Vec::new(),
            var_watchers: Vec::new(),
            threads: vec![std::rc::Rc::new(std::cell::RefCell::new(
                crate::lisp::value::Thread {
                    name: None,
                    alive: true,
                    result: None,
                    last_error: None,
                    finished: false,
                },
            ))],
            current_thread: 0,
            thread_last_error: Value::Nil,
            standard_case_table: None,
            standard_category_table: None,
            ccl_program_count: 0,
            code_conv_map_count: 0,
            memory_profiler: false,
            advices: Vec::new(),
            advice_links: Vec::new(),
            char_table_parents: Vec::new(),
            char_table_defalts: Vec::new(),
            frame_state_seen: None,
            lisp_stack: Vec::new(),
            cpu_profiler: false,
            terminal: None,
            terminal_params: Vec::new(),
            x_display_attempted: false,
            color_db_init: false,
            unibyte_strings: std::collections::HashMap::new(),
            multibyte_strings: std::collections::HashMap::new(),
            string_props: std::collections::HashMap::new(),
        };
        crate::lisp::builtins::install(&mut interp);
        crate::buffer::install_primitives(&mut interp);
        interp.define_error_conditions();
        interp.define_special_variables();
        // Undo recording writes into the `buffer-undo-list' buffer-local
        // variable; buffers need the symbol's id before creation.
        let us = interp.intern("buffer-undo-list");
        interp.buffers.set_undo_sym(us);
        interp.undo_inhibit_sym = interp.intern("undo-inhibit-record-point");
        // The initial buffers every Emacs session has, in GNU's
        // buffer-list order: (scratch Minibuf-0 Messages load
        // Warnings). New buffers append at the end of the order.
        let scratch = interp.buffers.create_exact("*scratch*");
        interp.current_buffer = scratch;
        let minibuf = interp.buffers.create_exact(" *Minibuf-0*");
        let messages = interp.buffers.create_exact("*Messages*");
        interp.buffers.create_exact(" *load*");
        interp.buffers.create_exact("*Warnings*");
        // GNU seeds the startup buffers' buffer-local major modes.
        let mm = interp.intern("major-mode");
        for (id, mode) in [
            (minibuf, "minibuffer-inactive-mode"),
            (messages, "messages-buffer-mode"),
        ] {
            if let Some(b) = interp.buffers.get(id) {
                b.borrow_mut()
                    .locals
                    .insert(mm, Value::Sym(interp.intern(mode)));
            }
        }
        crate::editor::install_primitives(&mut interp);
        // GNU's startup `*scratch*' gets its syntax-table chain from
        // `lisp-interaction-mode' (run by the prelude below):
        // lisp-interaction-mode-syntax-table →
        // emacs-lisp-mode-syntax-table → lisp-data-mode-syntax-table
        // → prog-mode-syntax-table → standard-syntax-table.  Every
        // other buffer resolves to `standard-syntax-table'.
        // Locale-derived tty defaults (GNU: utf-8 with unix EOL).
        let u8u = interp.intern("utf-8-unix");
        interp.terminal_coding = Value::Sym(u8u);
        interp.keyboard_coding = Value::Sym(u8u);
        // Load the Lisp prelude (subr.el subset). Errors here indicate a
        // broken prelude, but don't abort startup.
        let prelude_max: usize = std::env::var("PRELUDE_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(usize::MAX);
        if std::env::var("REMACS_NO_PRELUDE").is_ok() {
            // Debug escape: skip prelude evaluation entirely.
        } else if std::env::var("PRELUDE_TRACE").is_ok() || prelude_max != usize::MAX {
            let src = crate::lisp::prelude::PRELUDE;
            let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
            let mut pos = 0usize;
            let mut k = 0;
            loop {
                if k >= prelude_max {
                    break;
                }
                let next = {
                    let mut reader =
                        crate::lisp::reader::Reader::with_chars(&mut interp, chars.clone());
                    reader.set_position(pos);
                    match reader.read() {
                        Ok(f) => f.map(|v| (v, reader.position())),
                        Err(_) => None,
                    }
                };
                match next {
                    Some((form, end)) => {
                        pos = end;
                        k += 1;
                        eprintln!(
                            "prelude form {k} @{pos}: {}",
                            interp
                                .prin1_to_string(&form)
                                .chars()
                                .take(120)
                                .collect::<String>()
                        );
                        if std::env::var("PRELUDE_TRACE_MX").is_ok() {
                            if let Ok(x) = interp.macroexpand(&form) {
                                eprintln!(
                                    "  expands to: {}",
                                    interp
                                        .prin1_to_string(&x)
                                        .chars()
                                        .take(800)
                                        .collect::<String>()
                                );
                            }
                        }
                        let _ = interp.eval(&form);
                    }
                    None => break,
                }
            }
        } else {
            let _ = interp.eval_str(crate::lisp::prelude::PRELUDE);
        }
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            // GNU records every dumped library in `load-history'; do the
            // same for the embedded prelude so `symbol-file' and the
            // find-func family can locate prelude-defined symbols.
            crate::lisp::load::record_prelude_load_history(
                &mut interp,
                concat!(env!("CARGO_MANIFEST_DIR"), "/src/lisp/prelude.el"),
                crate::lisp::prelude::PRELUDE,
            );
        }
        // GNU's `global-eldoc-mode' installs eldoc functions
        // buffer-locally on the command hooks; in `-Q --batch' only
        // `*scratch*' carries those local bindings (verified), which is
        // why `local-variable-p' reports t there.  The default value of
        // `pre-command-hook' is (tooltip-hide) in batch.
        let pch = interp.intern("post-command-hook");
        let eldoc = interp.intern("eldoc-schedule-timer");
        let prech = interp.intern("pre-command-hook");
        let eldoc_pre = interp.intern("eldoc-pre-command-refresh-echo-area");
        if let Some(b) = interp.buffers.get(scratch) {
            let mut br = b.borrow_mut();
            br.locals
                .insert(pch, Value::list(vec![Value::Sym(eldoc), Value::t()]));
            br.locals.insert(
                prech,
                Value::list(vec![Value::Sym(eldoc_pre), Value::t()]),
            );
        }
        let tooltip_hide = interp.intern("tooltip-hide");
        interp.obarray.symbol_mut(prech).value =
            Value::list(vec![Value::Sym(tooltip_hide)]);
        // Boot-time autoloads (easy-mmode & co.) correspond to GNU's
        // dumped loadup; the user-visible `features' list must match the
        // post-dump set.
        interp.features = dump_features;
        // `icons' faces created by boot-time loads are likewise
        // hidden until a real `load' triggers them.
        interp
            .face_table
            .retain(|(n, _)| n != "icon" && n != "icon-button");
        let flist = Value::list(
            interp.features.iter().map(|s| Value::Sym(*s)).collect(),
        );
        let fid = interp.intern("features");
        interp.obarray.symbol_mut(fid).value = flist;
        interp
    }

    // ---------- symbol helpers ----------

    pub fn intern(&mut self, name: &str) -> SymId {
        self.obarray.intern(name)
    }

    pub fn intern_soft(&self, name: &str) -> Option<SymId> {
        self.obarray.intern_soft(name)
    }

    pub fn make_symbol(&mut self, name: &str) -> SymId {
        self.obarray.make_symbol(name)
    }

    /// `Value::Sym`, normalized so id 0 becomes `Value::Nil`.
    pub fn sym(&self, id: SymId) -> Value {
        if id == sym::NIL {
            Value::Nil
        } else {
            Value::Sym(id)
        }
    }

    pub fn symbol_name(&self, id: SymId) -> String {
        self.obarray.name(id).to_string()
    }

    pub fn sym_is(&self, v: &Value, id: SymId) -> bool {
        match v {
            Value::Sym(s) => *s == id,
            Value::Nil => id == sym::NIL,
            _ => false,
        }
    }

    /// The symbol id of a `Value::Sym`/`Value::Nil`, if it is one.
    pub fn sym_id(&self, v: &Value) -> Option<SymId> {
        match v {
            Value::Sym(s) => Some(*s),
            Value::Nil => Some(sym::NIL),
            _ => None,
        }
    }

    /// Global (default) value of a symbol.
    /// Follow `defvaralias' chains: return the ultimate base symbol.
    pub fn var_alias_target(&self, id: SymId) -> SymId {
        let prop = self.obarray.intern_soft("variable-alias").unwrap_or(0);
        let mut cur = id;
        for _ in 0..64 {
            match self.get_prop(cur, prop) {
                Value::Sym(next) => cur = next,
                _ => return cur,
            }
        }
        cur
    }

    pub fn symbol_value(&self, id: SymId) -> Value {
        let id = self.var_alias_target(id);
        // Buffer-local binding in current buffer wins. `try_borrow`:
        // primitives that hold the buffer mutably borrowed may still
        // consult variables (they see the global binding).
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    return v.clone();
                }
            }
        }
        let v = &self.obarray.symbol(id).value;
        if let Value::Sym(s) = v {
            if *s == sym::UNBOUND {
                return Value::Sym(sym::UNBOUND);
            }
        }
        v.clone()
    }

    /// Mark a string as unibyte (encoder output); `prin1' escapes
    /// its ≥0x80 byte-chars as `\NNN' octal like GNU.
    pub fn mark_unibyte(&mut self, s: &crate::lisp::value::StrRef) {
        self.unibyte_strings.insert(
            std::rc::Rc::as_ptr(s) as usize,
            std::rc::Rc::downgrade(s),
        );
    }

    /// Is this string a marked unibyte string?  A stale map entry
    /// (dead Weak, address since reused) never reports true.
    pub fn is_unibyte_str(&self, s: &crate::lisp::value::StrRef) -> bool {
        self.unibyte_strings
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .and_then(|w| w.upgrade())
            .is_some()
    }

    /// Mark a string as multibyte (decoder output).
    pub fn mark_multibyte(&mut self, s: &crate::lisp::value::StrRef) {
        self.multibyte_strings.insert(
            std::rc::Rc::as_ptr(s) as usize,
            std::rc::Rc::downgrade(s),
        );
    }

    /// Is this string a marked multibyte string?
    pub fn is_multibyte_str(&self, s: &crate::lisp::value::StrRef) -> bool {
        self.multibyte_strings
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .and_then(|w| w.upgrade())
            .is_some()
    }

    /// Text-prop interval list for a string object (empty when none).
    pub fn str_props(&self, s: &crate::lisp::value::StrRef) -> &[(usize, usize, Vec<Value>)] {
        match self.string_props.get(&(std::rc::Rc::as_ptr(s) as usize)) {
            Some((w, v)) if w.upgrade().is_some() => v.as_slice(),
            _ => &[],
        }
    }

    /// Mutable text-prop interval list for a string (creates entry).
    pub fn str_props_mut(
        &mut self,
        s: &crate::lisp::value::StrRef,
    ) -> &mut Vec<(usize, usize, Vec<Value>)> {
        &mut self
            .string_props
            .entry(std::rc::Rc::as_ptr(s) as usize)
            .or_insert_with(|| (std::rc::Rc::downgrade(s), Vec::new()))
            .1
    }

    /// Does a prop interval tree exist for this string?  GNU's
    /// `object-intervals' returns nil before the first prop op but a
    /// full nil-plist cover afterwards, so presence must be tracked
    /// separately from the interval list.
    pub fn has_str_props(&self, s: &crate::lisp::value::StrRef) -> bool {
        self.string_props
            .get(&(std::rc::Rc::as_ptr(s) as usize))
            .map(|(w, _)| w.upgrade().is_some())
            .unwrap_or(false)
    }

    /// Replace a string's prop intervals wholesale (copy ops).  An
    /// empty list still materializes the interval tree (GNU keeps a
    /// single nil-plist interval covering the string).
    pub fn set_str_props(&mut self, s: &crate::lisp::value::StrRef, v: Vec<(usize, usize, Vec<Value>)>) {
        self.string_props
            .insert(std::rc::Rc::as_ptr(s) as usize, (std::rc::Rc::downgrade(s), v));
    }

    /// Follow a symbol's function-alias chain; return the final
    /// non-symbol value (or UNBOUND sym).
    pub fn indirect_function_value(&self, v: &Value) -> Value {
        let mut cur = v.clone();
        for _ in 0..64 {
            match cur {
                Value::Sym(id) => {
                    let f = self.symbol_function(id);
                    match f {
                        Value::Sym(next) if next != sym::UNBOUND => cur = Value::Sym(next),
                        _ => return f,
                    }
                }
                other => return other,
            }
        }
        cur
    }

    /// `boundp`: is the effective value non-void?
    pub fn bound_p(&self, id: SymId) -> bool {
        let id = self.var_alias_target(id);
        // Constants (nil, t, keywords) are always bound.
        if self.obarray.symbol(id).constant {
            return true;
        }
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    // A void local binding (make-local-variable on an
                    // unbound variable) counts as unbound.
                    return !matches!(v, Value::Sym(s) if *s == sym::UNBOUND);
                }
            }
        }
        !matches!(
            self.obarray.symbol(id).value,
            Value::Sym(s) if s == sym::UNBOUND
        )
    }

    /// Set a variable the way `set`/`setq` does: local binding if the
    /// current buffer has one or the symbol is "automatically local".
    pub fn set_symbol(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let id = self.var_alias_target(id);
        let constant = self.obarray.symbol(id).constant;
        if constant && id != sym::NIL && id != sym::T {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        if constant {
            // nil/t can't be set at all.
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let sym = self.obarray.symbol(id);
        let is_auto_local = sym.make_local_if_set || sym.always_local;
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                if is_auto_local || bb.locals.contains_key(&id) {
                    bb.locals.insert(id, val.clone());
                    drop(bb);
                    return self.fire_var_watchers(id, &val, "set", Some(self.current_buffer));
                }
            }
        }
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        self.fire_var_watchers(id, &val, "set", None)
    }

    /// Mirror `undo-inhibit-record-point' writes into the shared cell
    /// consulted by `Buffer::record_point'.
    fn sync_undo_inhibit(&mut self, id: SymId, val: &Value) {
        if self.undo_inhibit_sym != 0 && id == self.undo_inhibit_sym {
            self.buffers
                .undo_inhibit_cell()
                .set(!val.is_nil());
        }
    }

    /// Set the global (default) value regardless of buffer-local bindings.
    pub fn set_symbol_default(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        self.fire_var_watchers(id, &val, "set", None)
    }

    /// `functionp' on a value: subrs, lambdas, `(lambda ...)' conses,
    /// and symbols whose function cell holds a non-macro definition.
    pub fn function_p(&self, v: &Value) -> bool {
        match v {
            Value::Subr(_) | Value::Lambda(_) => true,
            Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
            Value::Sym(id) => match self.symbol_function(*id) {
                Value::Subr(s) => self
                    .intern_soft(s.name)
                    .and_then(|x| crate::lisp::special::special_form(x))
                    .is_none(),
                Value::Lambda(l) => !l.is_macro,
                Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
                _ => false,
            },
            _ => false,
        }
    }

    pub fn symbol_function(&self, id: SymId) -> Value {
        self.obarray.symbol(id).function.clone()
    }

    pub fn fbound_p(&self, id: SymId) -> bool {
        // Emacs: special forms are fbound too.
        if super::special::special_form(id).is_some() {
            return true;
        }
        !matches!(
            self.obarray.symbol(id).function,
            Value::Sym(s) if s == sym::UNBOUND
        )
    }

    pub fn fset(&mut self, id: SymId, def: Value) {
        self.obarray.symbol_mut(id).function = def;
    }

    /// `get` — symbol property.
    pub fn get_prop(&self, id: SymId, prop: SymId) -> Value {
        let plist = &self.obarray.symbol(id).plist;
        plist_get(plist, prop)
    }

    /// `put` — set symbol property, returns value.
    pub fn put_prop(&mut self, id: SymId, prop: SymId, val: Value) {
        let plist = self.obarray.symbol(id).plist.clone();
        let new_plist = plist_put(&plist, prop, val);
        self.obarray.symbol_mut(id).plist = new_plist;
    }

    // ---------- specbind (dynamic let) ----------

    /// Push a dynamic binding for `sym` to `val`.
    pub fn specbind(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let sym = self.obarray.symbol(id);
        let is_auto_local = sym.make_local_if_set || sym.always_local;
        let mut bound_buf = None;
        if let Some(b) = self.buffers.get(self.current_buffer) {
            let mut bb = b.borrow_mut();
            if is_auto_local || bb.locals.contains_key(&id) {
                let old = bb.locals.insert(id, val.clone());
                drop(bb);
                self.specbind.push(SpecBind {
                    sym: id,
                    buf: Some(self.current_buffer),
                    old,
                });
                bound_buf = Some(self.current_buffer);
            }
        }
        if bound_buf.is_none() {
            let old = self.obarray.symbol(id).value.clone();
            self.obarray.symbol_mut(id).value = val.clone();
            self.sync_undo_inhibit(id, &val);
            self.specbind.push(SpecBind {
                sym: id,
                buf: None,
                old: Some(old),
            });
        }
        self.fire_var_watchers(id, &val, "let", bound_buf)
    }

    /// Pop `n` specbind entries, restoring values.
    pub fn unbind(&mut self, n: usize) -> Result<(), Flow> {
        for _ in 0..n {
            let Some(sb) = self.specbind.pop() else {
                return Ok(());
            };
            let mut restored = Value::Nil;
            match sb.buf {
                Some(buf_id) => {
                    if let Some(b) = self.buffers.get(buf_id) {
                        let mut bb = b.borrow_mut();
                        match sb.old {
                            Some(v) => {
                                restored = v.clone();
                                bb.locals.insert(sb.sym, v)
                            }
                            None => bb.locals.remove(&sb.sym),
                        };
                    }
                }
                None => {
                    if let Some(v) = sb.old {
                        restored = v.clone();
                        self.obarray.symbol_mut(sb.sym).value = v.clone();
                        self.sync_undo_inhibit(sb.sym, &v);
                    }
                }
            }
            // GNU fires 'unlet watchers with the restored value.
            self.fire_var_watchers(sb.sym, &restored, "unlet", sb.buf)?;
        }
        Ok(())
    }

    pub fn specbind_depth(&self) -> usize {
        self.specbind.len()
    }

    /// `default-toplevel-value': the global binding ignoring `let' frames.
    /// The outermost specbind frame for the symbol saved the toplevel value.
    /// Returns None when the symbol's default is void.
    pub fn default_toplevel_value(&self, id: SymId) -> Option<Value> {
        for sb in &self.specbind {
            if sb.sym == id && sb.buf.is_none() {
                return sb.old.clone();
            }
        }
        match &self.obarray.symbol(id).value {
            Value::Sym(s) if *s == sym::UNBOUND => None,
            v => Some(v.clone()),
        }
    }

    /// `buffer-local-toplevel-value': the toplevel buffer-local binding in
    /// `buf`, ignoring `let' frames. None when no toplevel local exists.
    pub fn buffer_local_toplevel_value(&self, id: SymId, buf: usize) -> Option<Value> {
        for sb in &self.specbind {
            if sb.sym == id && sb.buf == Some(buf) {
                return sb
                    .old
                    .clone()
                    .filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND));
            }
        }
        let v = self
            .buffers
            .get(buf)
            .and_then(|b| b.try_borrow().ok().map(|bb| bb.locals.get(&id).cloned()))
            .flatten();
        v.filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND))
    }

    /// `set-buffer-local-toplevel-value': set the toplevel buffer-local
    /// binding in `buf` without disturbing in-flight `let' bindings.
    pub fn set_buffer_local_toplevel_value(&mut self, id: SymId, buf: usize, val: Value) {
        for sb in &mut self.specbind {
            if sb.sym == id && sb.buf == Some(buf) {
                sb.old = Some(val);
                return;
            }
        }
        if let Some(b) = self.buffers.get(buf) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                bb.locals.insert(id, val);
            }
        }
    }

    // ---------- errors ----------

    /// `(signal sym (data...))` where data is already a list.
    pub fn signal(&self, sym_id: SymId, data: Value) -> Flow {
        Flow::Signal(Value::Sym(sym_id), data, false)
    }

    /// `(signal sym data-list-from-vec)`.
    pub fn signal_data(&self, sym_id: SymId, data: Vec<Value>) -> Flow {
        Flow::Signal(Value::Sym(sym_id), Value::list(data), false)
    }

    /// `(error "fmt" args...)` — signals `error` with a formatted message.
    pub fn error(&self, msg: impl Into<String>) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg.into())])
    }

    /// `wrong-type-argument` signal: pred, value.
    pub fn wrong_type_mut(&mut self, pred: &str, val: &Value) -> Flow {
        let pred_id = self.intern(pred);
        self.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred_id), val.clone()],
        )
    }

    /// `wrong-number-of-arguments' — Emacs signals `(FUN ARGC)'.
    pub fn wrong_number_of_args(&self, fun: &Value, argc: i128) -> Flow {
        self.signal_data(
            sym::WRONG_NUMBER_OF_ARGUMENTS,
            vec![fun.clone(), Value::Int(argc)],
        )
    }

    /// Does signal `sig` match any condition name in `handlers`?
    /// `handlers` is a list of condition symbols (or t).
    pub fn signal_matches(&self, sig: &Value, handlers: &Value) -> bool {
        // A handler's car is either a single condition name or a list
        // of names; `t` matches everything, `nil` (debug) nothing.
        if let Value::Sym(s) = handlers {
            if *s == sym::NIL {
                return false;
            }
            return self.signal_matches(sig, &Value::list(vec![handlers.clone()]));
        }
        let mut found = false;
        handlers.each_car(|h| {
            if found {
                return;
            }
            match h {
                Value::Sym(s) if *s == sym::T => found = true,
                Value::Sym(cond) => {
                    // A signal matches if `cond` is the signal symbol or
                    // appears in its `error-conditions` property.
                    if let Value::Sym(sig_id) = sig {
                        if sig_id == cond || self.condition_has(*sig_id, *cond) {
                            found = true;
                        }
                    }
                }
                Value::Cons(_) => {
                    // (cond ...) group? Emacs allows a list of symbols too.
                    h.each_car(|c| {
                        if let (Value::Sym(c_id), Value::Sym(sig_id)) = (c, sig) {
                            if *sig_id == *c_id || self.condition_has(*sig_id, *c_id) {
                                found = true;
                            }
                        }
                    });
                }
                _ => {}
            }
        });
        found
    }

    /// Does error symbol `sig` have `cond` in its `error-conditions`?
    fn condition_has(&self, sig: SymId, cond: SymId) -> bool {
        let conds = self.get_prop(sig, {
            self.obarray.intern_soft("error-conditions").unwrap_or(0)
        });
        let mut found = false;
        conds.each_car(|c| {
            if let Value::Sym(c_id) = c {
                if *c_id == cond {
                    found = true;
                }
            }
        });
        found
    }

    // ---------- entry points ----------

    /// Read and evaluate all top-level forms in `src`.
    /// Returns the last value.
    pub fn eval_str(&mut self, src: &str) -> EvalResult {
        // The reader borrows `self`, so create/drop it per form — but
        // share one collected char buffer across all reads.
        let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            let next = {
                let mut reader = Reader::with_chars(self, chars.clone());
                reader.set_position(pos);
                match reader.read()? {
                    Some(f) => Some((f, reader.position())),
                    None => None,
                }
            };
            match next {
                Some((form, end)) => {
                    pos = end;
                    if std::env::var_os("REMACS_TRACE_EVAL").is_some() {
                        eprintln!("[eval@{}] {}", end, self.princ_to_string(&form).chars().take(80).collect::<String>());
                    }
                    match self.eval(&form) {
                        Ok(v) => last = v,
                        // An uncaught throw is a `no-catch' error.
                        Err(Flow::Throw(tag, val)) => {
                            let nc = self.intern("no-catch");
                            return Err(self.signal_data(nc, vec![tag, val]));
                        }
                        Err(f) => return Err(f),
                    }
                }
                None => return Ok(last),
            }
        }
    }

    /// `read-from-string` core: read one object, return it + end position.
    pub fn read_from_string(&mut self, src: &str, start: usize) -> Result<(Value, usize), Flow> {
        self.read_from_string_pos(src, start, None)
    }

    /// `read_from_string' with `read-positioning-symbols' mode: symbol
    /// tokens become `symbol-with-pos' records carrying `base + token
    /// char index' as their position.
    pub fn read_from_string_pos(
        &mut self,
        src: &str,
        start: usize,
        pos_base: Option<i128>,
    ) -> Result<(Value, usize), Flow> {
        let mut reader = Reader::new(self, src);
        reader.set_position(start);
        reader.annotate_pos = pos_base;
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }

    // ---------- eval ----------

    pub fn eval(&mut self, form: &Value) -> EvalResult {
        self.eval_depth += 1;
        if self.eval_depth > self.max_lisp_eval_depth {
            self.eval_depth -= 1;
            return Err(self.error("Lisp nesting exceeds `max-lisp-eval-depth'"));
        }
        let result = self.eval_inner(form);
        self.eval_depth -= 1;
        match result {
            Err(Flow::Signal(sig, data, false)) => match self.offer_signal(&sig, &data) {
                Ok(()) => Err(Flow::Signal(sig, data, true)),
                Err(f) => Err(f),
            },
            other => other,
        }
    }

    /// Offer a freshly-raised signal to `signal-hook-function' and to
    /// `handler-bind' handlers. Emacs runs these at raise time, in the
    /// signaling dynamic context, once per signal — the `offered' flag
    /// in `Flow::Signal' marks it after the innermost eval frame sees it.
    /// Handler nonlocal exits supersede the original signal.
    fn offer_signal(&mut self, sig: &Value, data: &Value) -> Result<(), Flow> {
        if let Some(id) = self.intern_soft("signal-hook-function") {
            if self.bound_p(id) {
                let hook = self.symbol_value(id);
                if hook.truthy() {
                    self.apply(&hook, vec![sig.clone(), data.clone()])?;
                }
            }
        }
        if self.handler_bindings.is_empty() {
            return Ok(());
        }
        // Emacs runs handler-bind handlers only for signals no
        // condition-case will claim (the debugger-entry point).
        for conds in self.case_handlers.iter().rev() {
            if self.signal_matches(sig, conds) {
                return Ok(());
            }
        }
        let cond = Value::cons(sig.clone(), data.clone());
        for idx in (0..self.handler_bindings.len()).rev() {
            let (conds, handler) = self.handler_bindings[idx].clone();
            if self.signal_matches(sig, &conds) {
                // While a handler runs only strictly-outer bindings are
                // visible — a handler's own signal can't re-enter it.
                let tail = self.handler_bindings.split_off(idx);
                let r = self.apply(&handler, vec![cond.clone()]);
                self.handler_bindings.extend(tail);
                r?;
            }
        }
        Ok(())
    }

    /// Run `add-variable-watcher' functions for `id' after a change.
    /// GNU calls each watcher as (SYM NEWVAL OPERATION WHERE).
    pub(crate) fn fire_var_watchers(
        &mut self,
        id: SymId,
        newval: &Value,
        op: &str,
        buf: Option<usize>,
    ) -> Result<(), Flow> {
        if self.var_watchers.is_empty() || !self.var_watchers.iter().any(|(s, _)| *s == id) {
            return Ok(());
        }
        if let Some(inh) = self.intern_soft("inhibit-variable-watchers") {
            if self.bound_p(inh) && self.symbol_value(inh).truthy() {
                return Ok(());
            }
        }
        let watchers: Vec<Value> = self
            .var_watchers
            .iter()
            .filter(|(s, _)| *s == id)
            .map(|(_, f)| f.clone())
            .collect();
        let where_ = buf.and_then(|b| self.buffer_value(b)).unwrap_or(Value::Nil);
        let op_id = self.intern(op);
        let op_sym = self.sym(op_id);
        for f in watchers {
            self.apply(
                &f,
                vec![self.sym(id), newval.clone(), op_sym.clone(), where_.clone()],
            )?;
        }
        Ok(())
    }

    fn eval_inner(&mut self, form: &Value) -> EvalResult {
        match form {
            Value::Nil => Ok(Value::Nil),
            Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Vec(_)
            | Value::Hash(_)
            | Value::Subr(_)
            | Value::Lambda(_)
            | Value::Buffer(_)
            | Value::Record(_)
            | Value::Marker(_)
            | Value::Window(_)
            | Value::Frame(_)
            | Value::Process(_)
            | Value::Thread(_)
            | Value::Mutex(_)
            | Value::CondVar(_)
            | Value::Finalizer(_) => Ok(form.clone()),
            Value::Sym(id) => self.eval_symbol(*id),
            Value::Cons(_) => self.eval_form(form),
        }
    }

    fn eval_symbol(&mut self, id: SymId) -> EvalResult {
        if id == sym::NIL {
            return Ok(Value::Nil);
        }
        // Lexical lookup first (only when the symbol isn't special).
        if !self.obarray.symbol(id).special {
            if let Some(v) = lexenv_lookup(&self.lexenv, id) {
                return Ok(v);
            }
        }
        let v = self.symbol_value(id);
        if let Value::Sym(s) = &v {
            if *s == sym::UNBOUND {
                if std::env::var("DBG_VOID").is_ok() {
                    let name = self.obarray.symbol(id).name.clone();
                    let mut depth = 0;
                    let mut env = &self.lexenv;
                    let mut found = false;
                    while let Some(f) = env {
                        depth += 1;
                        if f.vars.borrow().contains_key(&id) {
                            found = true;
                        }
                        env = &f.parent;
                    }
                    eprintln!(
                        "DBG void-var {} special={} lexdepth={} lexhas={} specbinds={} case_handlers={}",
                        name,
                        self.obarray.symbol(id).special,
                        depth,
                        found,
                        self.specbind
                            .iter()
                            .filter(|s| s.sym == id)
                            .count(),
                        self.case_handlers.len(),
                    );
                }
                return Err(self.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
            }
        }
        Ok(v)
    }

    /// Evaluate a compound form `(fn . args)`.
    fn eval_form(&mut self, form: &Value) -> EvalResult {
        if self.quit_flag {
            self.quit_flag = false;
            return Err(Flow::Quit);
        }
        let cons = match form {
            Value::Cons(c) => c,
            _ => unreachable!(),
        };
        let (head, args) = {
            let b = cons.borrow();
            (b.car.clone(), b.cdr.clone())
        };

        match &head {
            Value::Sym(id) => {
                let id = *id;
                // Special form?
                if let Some(sf) = super::special::special_form(id) {
                    return sf(self, args);
                }
                // Function cell.
                let fun = self.symbol_function(id);
                if let Value::Sym(s) = &fun {
                    if *s == sym::UNBOUND {
                        return Err(self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(id)]));
                    }
                }
                if self.advices.iter().any(|(s, a)| *s == id && !a.is_empty()) {
                    let argv = self.eval_args(&args)?;
                    return self.apply_adviced(id, &fun, argv);
                }
                self.call_function(&fun, &args, Some(id))
            }
            Value::Cons(_) => {
                // `((lambda (x) ...) a b)` — function position lambda is
                // used directly, not evaluated as a call.
                let b = match &head {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => unreachable!(),
                };
                if self.sym_is(&b, sym::LAMBDA) {
                    let lambda = self.lambda_from_form(&head, None)?;
                    let argv = self.eval_args(&args)?;
                    return self.apply(&Value::Lambda(Rc::new(lambda)), argv);
                }
                let fun = self.eval(&head)?;
                self.call_function(&fun, &args, None)
            }
            _ => {
                let fun = self.eval(&head)?;
                self.call_function(&fun, &args, None)
            }
        }
    }

    /// Evaluate each element of a list form.
    pub fn eval_args(&mut self, args: &Value) -> Result<Vec<Value>, Flow> {
        let mut out = Vec::new();
        let mut cur = args.clone();
        loop {
            match cur {
                Value::Nil => return Ok(out),
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    out.push(self.eval(&car)?);
                    cur = next;
                }
                _ => {
                    return Err(self.error("Invalid function call form (dotted)"));
                }
            }
        }
    }

    /// Call `fun` with raw unevaluated `args` (a list). Evaluates args
    /// unless `fun` is a macro or special.
    pub fn call_function(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<SymId>,
    ) -> EvalResult {
        match fun {
            Value::Sym(id) => {
                // Function alias chain: chase.
                let mut cur = *id;
                let mut advised = None;
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    if advised.is_none()
                        && self.advices.iter().any(|(s, a)| *s == cur && !a.is_empty())
                    {
                        advised = Some(cur);
                    }
                    let f = self.symbol_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            cur = next;
                        }
                        other => {
                            if let Some(s) = advised {
                                let argv = self.eval_args(args)?;
                                return self.apply_adviced(s, &other, argv);
                            }
                            // Emacs reports the originally called symbol
                            // in arity errors, not the resolved one.
                            return self.call_function(&other, args, sym_name);
                        }
                    }
                }
            }
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => (s.func)(self, vec![args.clone()]),
                _ => {
                    let argv = self.eval_args(args)?;
                    self.check_arity_subr(s, &argv, sym_name)?;
                    (s.func)(self, argv)
                }
            },
            Value::Lambda(_) => {
                // Macro: expand then eval.
                if fun.as_lambda().map(|l| l.is_macro).unwrap_or(false) {
                    let expansion = self.macro_expand_call(fun, args)?;
                    return self.eval(&expansion);
                }
                let argv = self.eval_args(args)?;
                let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                self.apply_resolved(fun, argv, shown)
            }
            Value::Cons(_) => {
                // A cons as function: `(lambda ...)` form or `(macro . f)`.
                let (car, cdr) = {
                    let c = match fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    // (macro . lambda) — expand with cdr as function.
                    let expansion = self.macro_expand_call(&cdr, args)?;
                    return self.eval(&expansion);
                }
                if self.sym_is(&car, sym::LAMBDA) || self.sym_is(&car, sym::QUOTE_FUNCTION) {
                    let lambda = Rc::new(self.lambda_from_form(fun, sym_name)?);
                    let argv = self.eval_args(args)?;
                    let shown = sym_name
                        .map(Value::Sym)
                        .unwrap_or_else(|| Value::Lambda(lambda.clone()));
                    return self.apply_resolved(&Value::Lambda(lambda), argv, shown);
                }
                let auto_id = self.intern("autoload");
                if self.sym_is(&car, auto_id) {
                    // (autoload FILE ...) — load, then re-dispatch on
                    // the real definition (macro autoloads expand).
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    return self.call_function(&newdef, args, sym_name);
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()])),
        }
    }

    fn check_arity_subr(
        &self,
        s: &'static super::value::Subr,
        argv: &[Value],
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        // Emacs reports the calling symbol for eval'd calls, the subr
        // object itself for `funcall'/`apply'.
        let who = name.map_or(Value::Subr(s), Value::Sym);
        let n = argv.len() as i128;
        let (min, max) = match s.arity {
            Arity::Range { min, max } => (min as i128, max as i128),
            Arity::Many { min } => {
                if n < min as i128 {
                    return Err(self.wrong_number_of_args(&who, n));
                }
                return Ok(());
            }
            Arity::Unevalled => return Ok(()),
        };
        if n < min || n > max {
            return Err(self.wrong_number_of_args(&who, n));
        }
        Ok(())
    }

    /// `apply`/`funcall`: call `fun` with already-evaluated `argv`.
    pub fn apply(&mut self, fun: &Value, argv: Vec<Value>) -> EvalResult {
        match fun {
            Value::Sym(id) => {
                let mut cur = *id;
                let mut advised = None;
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    if advised.is_none()
                        && self.advices.iter().any(|(s, a)| *s == cur && !a.is_empty())
                    {
                        advised = Some(cur);
                    }
                    let f = self.symbol_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            cur = next;
                        }
                        other => match advised {
                            Some(s) => return self.apply_adviced(s, &other, argv),
                            None => {
                                return self.apply_resolved(&other, argv, Value::Sym(*id))
                            }
                        },
                    }
                }
            }
            _ => self.apply_resolved(fun, argv, fun.clone()),
        }
    }

    /// Dispatch a resolved (non-symbol) function, recording the call on
    /// `lisp_stack' so `mapbacktrace' can walk live frames.  `shown' is
    /// what backtraces display as the callee — the symbol when the call
    /// came through a symbol's function cell, else the function itself.
    fn apply_resolved(&mut self, fun: &Value, argv: Vec<Value>, shown: Value) -> EvalResult {
        if std::env::var_os("REMACS_TRACE_CALL").is_some() {
            eprintln!(
                "[call] {}",
                self.princ_to_string(&shown).chars().take(90).collect::<String>()
            );
        }
        match fun {
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => {
                    // GNU: special forms cannot be funcalled/applied.
                    Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
                }
                _ => {
                    self.check_arity_subr(s, &argv, None)?;
                    self.lisp_stack.push((shown, argv.clone()));
                    let r = (s.func)(self, argv);
                    self.lisp_stack.pop();
                    r
                }
            },
            Value::Lambda(l) => {
                self.lisp_stack.push((shown.clone(), argv.clone()));
                let r = self.call_lambda(l, argv, &shown);
                self.lisp_stack.pop();
                r
            }
            Value::Cons(_) => {
                let (car, _) = {
                    let c = match fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::LAMBDA) {
                    let lambda = self.lambda_from_form(fun, None)?;
                    self.lisp_stack.push((shown.clone(), argv.clone()));
                    let r = self.call_lambda(&Rc::new(lambda), argv, &shown);
                    self.lisp_stack.pop();
                    return r;
                }
                let auto_id = self.intern("autoload");
                if self.sym_is(&car, auto_id) {
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    return self.apply(&newdef, argv);
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()])),
        }
    }

    /// Copy of SYM's `advice-add' entries: (WHERE FUN . NAME) triples.
    pub fn advice_list(&self, sym: SymId) -> Vec<(SymId, Value, Value)> {
        self.advices
            .iter()
            .find(|(s, _)| *s == sym)
            .map(|(_, a)| a.clone())
            .unwrap_or_default()
    }

    /// Synthesize `(lambda (&rest a) (apply 'SUBR IDX a))'.
    fn advice_trampoline(&mut self, idx: usize) -> EvalResult {
        let src = format!(
            "(lambda (&rest cl--args) (apply (function cl--advice--apply) {} cl--args))",
            idx
        );
        let (form, _) = self.read_from_string(&src, 0)?;
        let lam = self.lambda_from_form(&form, None)?;
        Ok(Value::Lambda(Rc::new(lam)))
    }

    /// Call SYM's adviced function.  GNU's nadvice composes advices in
    /// reverse add order — the most recently added piece is outermost —
    /// each wrapping the inner thunk per its WHERE class.
    fn apply_adviced(&mut self, sym: SymId, base: &Value, argv: Vec<Value>) -> EvalResult {
        let advs = self.advice_list(sym);
        if advs.is_empty() {
            return self.apply(base, argv);
        }
        let mut next = base.clone();
        for (w, f, _) in &advs {
            let idx = self.advice_links.len();
            self.advice_links.push((Value::Sym(*w), f.clone(), next));
            next = self.advice_trampoline(idx)?;
        }
        self.apply(&next, argv)
    }

    /// Call an interpreted lambda with evaluated args.  `shown' is what
    /// `wrong-number-of-arguments' reports as the function — GNU prints
    /// the called symbol when invoked by name.
    fn call_lambda(&mut self, l: &Rc<Lambda>, argv: Vec<Value>, shown: &Value) -> EvalResult {
        // Arity.
        let (min, max_ok) = (l.required.len(), l.rest.is_some());
        if argv.len() < min || (!max_ok && argv.len() > min + l.optional.len()) {
            return Err(self.wrong_number_of_args(shown, argv.len() as i128));
        }

        // Dynamic (non-macro) functions with extended `(var init)'
        // parameters are invalid to call, like Emacs's interpreted
        // functions — the arity check above still runs first.
        if l.bad_arglist && !l.is_macro {
            return Err(self.signal_data(sym::INVALID_FUNCTION, vec![Value::Lambda(l.clone())]));
        }

        if l.env.is_some() {
            // Lexical closure: extend captured env.  Parameters that are
            // `defvar'd special are bound dynamically (GNU specbind);
            // lookups for them bypass the lexical env.
            let vars = RefCell::new(HashMap::new());
            let mark = self.specbind_depth();
            let bind_result = self.bind_lambda_args_lexical(&l.clone(), &argv, &vars);
            // `&optional` defaults may need evaluation in the new env;
            // evaluate them after the frame exists.
            let frame = Rc::new(LexFrame {
                vars,
                parent: l.env.clone(),
            });
            let saved = std::mem::replace(&mut self.lexenv, Some(frame.clone()));
            let r = match bind_result {
                Ok(()) => self.fill_optional_defaults(l, &argv, &frame),
                Err(e) => Err(e),
            };
            let result = match r {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved;
            self.unbind_to(mark)?;
            result
        } else {
            // Dynamic: specbind each parameter, and make sure no lexical
            // env leaks in — dynamic functions can't see callers' lexvars.
            let saved_lex = std::mem::replace(&mut self.lexenv, None);
            let mark = self.specbind_depth();
            let bind_result = self.bind_lambda_args_result(&l.clone(), &argv);
            let result = match bind_result {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved_lex;
            self.unbind_to(mark)?;
            result
        }
    }

    /// For lexical closures: evaluate `&optional` defaults inside the
    /// already-created frame for params not supplied.
    fn fill_optional_defaults(
        &mut self,
        l: &Rc<Lambda>,
        argv: &[Value],
        frame: &Rc<LexFrame>,
    ) -> Result<(), Flow> {
        let supplied = argv.len().min(l.required.len() + l.optional.len());
        let mut i = l.required.len();
        for opt in &l.optional {
            if i >= supplied {
                let v = match &opt.default {
                    Some(d) => self.eval(d)?,
                    None => Value::Nil,
                };
                if self.obarray.symbol(opt.sym).special {
                    self.specbind(opt.sym, v)?;
                } else {
                    frame.vars.borrow_mut().insert(opt.sym, v);
                }
            }
            i += 1;
        }
        Ok(())
    }

    /// Lexical-closure argument binding: params go into the new frame's
    /// `vars`, except `defvar'd specials which get dynamic specbinds.
    fn bind_lambda_args_lexical(
        &mut self,
        l: &Rc<Lambda>,
        argv: &[Value],
        vars: &RefCell<HashMap<SymId, Value>>,
    ) -> Result<(), Flow> {
        let bind = |this: &mut Self, sym: SymId, val: Value| -> Result<(), Flow> {
            if this.obarray.symbol(sym).special {
                this.specbind(sym, val)
            } else {
                vars.borrow_mut().insert(sym, val);
                Ok(())
            }
        };
        let mut i = 0;
        for s in l.required.clone() {
            bind(self, s, argv[i].clone())?;
            i += 1;
        }
        for opt in l.optional.clone() {
            let given = i < argv.len();
            let v = if given {
                argv[i].clone()
            } else {
                opt.default.clone().unwrap_or(Value::Nil)
            };
            bind(self, opt.sym, v)?;
            if let Some(sp) = opt.supplied {
                bind(self, sp, Value::from_bool(given))?;
            }
            i += 1;
        }
        if let Some(rest) = l.rest {
            let tail = if i < argv.len() {
                Value::list(argv[i..].to_vec())
            } else {
                Value::Nil
            };
            bind(self, rest, tail)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn bind_lambda_args(&self, l: &Rc<Lambda>, argv: &[Value], mut f: impl FnMut(SymId, Value)) {
        let mut i = 0;
        for s in &l.required {
            f(*s, argv[i].clone());
            i += 1;
        }
        for opt in &l.optional {
            let given = i < argv.len();
            let v = if given {
                argv[i].clone()
            } else {
                // Evaluate default at call time — needs eval; handled by
                // bind_lambda_args_result path.
                opt.default.clone().unwrap_or(Value::Nil)
            };
            f(opt.sym, v);
            if let Some(sp) = opt.supplied {
                f(sp, Value::from_bool(given));
            }
            i += 1;
        }
        if let Some(rest) = l.rest {
            let tail = if i < argv.len() {
                Value::list(argv[i..].to_vec())
            } else {
                Value::Nil
            };
            f(rest, tail);
        }
    }

    /// Specbind version that evaluates `&optional` defaults properly.
    fn bind_lambda_args_result(&mut self, l: &Rc<Lambda>, argv: &[Value]) -> Result<(), Flow> {
        let mut i = 0;
        for s in &l.required {
            self.specbind(*s, argv[i].clone())?;
            i += 1;
        }
        for opt in &l.optional {
            let given = i < argv.len();
            let v = if given {
                argv[i].clone()
            } else {
                match &opt.default {
                    Some(d) => self.eval(d)?,
                    None => Value::Nil,
                }
            };
            self.specbind(opt.sym, v)?;
            if let Some(sp) = opt.supplied {
                self.specbind(sp, Value::from_bool(given))?;
            }
            i += 1;
        }
        if let Some(rest) = l.rest {
            let tail = if i < argv.len() {
                Value::list(argv[i..].to_vec())
            } else {
                Value::Nil
            };
            self.specbind(rest, tail)?;
        }
        Ok(())
    }

    pub fn unbind_to(&mut self, mark: usize) -> Result<(), Flow> {
        let n = self.specbind.len() - mark;
        self.unbind(n)
    }

    /// Evaluate a body (progn), returning the last value.
    pub fn eval_body(&mut self, body: &[Value]) -> EvalResult {
        let mut last = Value::Nil;
        for f in body {
            last = self.eval(f)?;
        }
        Ok(last)
    }

    /// Evaluate a list of forms (a `progn`-style list, not a Vec).
    pub fn eval_progn(&mut self, forms: &Value) -> EvalResult {
        let mut last = Value::Nil;
        let mut cur = forms.clone();
        loop {
            match cur {
                Value::Nil => return Ok(last),
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    last = self.eval(&car)?;
                    cur = next;
                }
                _ => return Err(self.error("dotted body")),
            }
        }
    }

    /// Expand a macro call: call the macro's function on raw args.
    pub fn macro_expand_call(&mut self, mac: &Value, args: &Value) -> EvalResult {
        if std::env::var("PRELUDE_TRACE").is_ok() {
            eprintln!(
                "mxcall {} <- {}",
                self.prin1_to_string(mac).chars().take(90).collect::<String>(),
                self.prin1_to_string(args).chars().take(90).collect::<String>()
            );
        }
        let argv = match args.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.error("bad macro args")),
        };
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) if l.is_macro => self.call_lambda(l, argv, mac)?,
            Value::Lambda(l) => self.call_lambda(l, argv, mac)?,
            Value::Cons(_) => {
                let (car, _) = {
                    let c = match mac {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    let cdr = match mac {
                        Value::Cons(c) => c.borrow().cdr.clone(),
                        _ => unreachable!(),
                    };
                    return self.macro_expand_call(&cdr, args);
                }
                self.apply(mac, argv)?
            }
            _ => self.apply(mac, argv)?,
        };
        Ok(result)
    }

    /// `macroexpand`: repeatedly expand while the form is a macro call.
    pub fn macroexpand(&mut self, form: &Value) -> EvalResult {
        let mut cur = form.clone();
        let mut iters = 0;
        loop {
            iters += 1;
            if std::env::var("PRELUDE_TRACE").is_ok() && iters > 500 {
                eprintln!("macroexpand iter {iters}: {}", self.prin1_to_string(&cur).chars().take(200).collect::<String>());
            }
            let next = match &cur {
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match car {
                        Value::Sym(id) => {
                            let mut f = self.symbol_function(id);
                            // Autoload cell: resolve macro autoloads
                            // (TYPE non-nil); others stop expansion.
                            let auto_id = self.intern("autoload");
                            let is_auto = match &f {
                                Value::Cons(cc) => self.sym_is(&cc.borrow().car, auto_id),
                                _ => false,
                            };
                            if is_auto {
                                f = crate::lisp::builtins::evalfn::autoload_do_load(self, f, true)?;
                                let still_auto = match &f {
                                    Value::Cons(cc) => self.sym_is(&cc.borrow().car, auto_id),
                                    _ => false,
                                };
                                if still_auto {
                                    return Ok(cur);
                                }
                            }
                            let is_mac = match &f {
                                Value::Lambda(l) => l.is_macro,
                                Value::Cons(cc) => {
                                    let b = cc.borrow();
                                    self.sym_is(&b.car, sym::MACRO)
                                }
                                _ => false,
                            };
                            if is_mac {
                                self.macro_expand_call(&f, &cdr)?
                            } else {
                                return Ok(cur);
                            }
                        }
                        _ => return Ok(cur),
                    }
                }
                _ => return Ok(cur),
            };
            cur = next;
        }
    }

    /// Parse a `(lambda (params) body...)` cons form into a `Lambda`.
    pub fn lambda_from_form(&mut self, form: &Value, name: Option<SymId>) -> Result<Lambda, Flow> {
        let items = match form.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.signal_data(sym::INVALID_FUNCTION, vec![form.clone()])),
        };
        // items[0] = lambda, items[1] = param list, rest = body
        if items.len() < 2 {
            return Err(self.signal_data(sym::INVALID_FUNCTION, vec![form.clone()]));
        }
        self.parse_lambda(&items[1], &items[2..], name)
    }

    /// Build a `Lambda` from a param list and body forms.
    pub fn parse_lambda(
        &mut self,
        params: &Value,
        body: &[Value],
        name: Option<SymId>,
    ) -> Result<Lambda, Flow> {
        let mut required = Vec::new();
        let mut optional = Vec::new();
        let mut rest = None;
        let mut bad_arglist = false;
        let mut mode = 0u8; // 0 = required, 1 = optional, 2 = rest done
        let plist = params;
        let mut cur = plist.clone();
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match self.sym_id(&car) {
                        Some(s) if s == sym::OPTIONAL => mode = 1,
                        Some(s) if s == sym::REST => {
                            mode = 2;
                        }
                        _ => match mode {
                            0 => {
                                if let Some(s) = self.sym_id(&car) {
                                    required.push(s);
                                } else {
                                    // `(var init)' in required position:
                                    // Emacs counts it toward the arity but
                                    // calling the function signals
                                    // invalid-function.
                                    required.push(self.intern("&bad-param"));
                                    bad_arglist = true;
                                }
                            }
                            1 => {
                                // `sym` or `(sym default)` — the extended
                                // form only works via macros/cl- arglists;
                                // calling a plain function with it signals
                                // invalid-function.
                                match self.sym_id(&car) {
                                    Some(s) => optional.push(super::value::OptParam {
                                        sym: s,
                                        default: None,
                                        supplied: None,
                                    }),
                                    None => {
                                        if let Value::Cons(_) = car {
                                            let pair = car.list_to_vec().unwrap_or_default();
                                            if let Some(s) =
                                                pair.first().and_then(|v| self.sym_id(v))
                                            {
                                                optional.push(super::value::OptParam {
                                                    sym: s,
                                                    default: pair.get(1).cloned(),
                                                    supplied: pair
                                                        .get(2)
                                                        .and_then(|v| self.sym_id(v)),
                                                });
                                                bad_arglist = true;
                                            } else {
                                                return Err(self.error("bad &optional parameter"));
                                            }
                                        } else {
                                            return Err(self.error("bad &optional parameter"));
                                        }
                                    }
                                }
                            }
                            _ => {
                                if rest.is_none() {
                                    if let Some(s) = self.sym_id(&car) {
                                        rest = Some(s);
                                    } else {
                                        return Err(self.error("bad &rest parameter"));
                                    }
                                } else {
                                    return Err(self.error("multiple &rest parameters"));
                                }
                            }
                        },
                    }
                    cur = next;
                }
                _ => return Err(self.error("dotted lambda list")),
            }
        }

        // Extract docstring and interactive spec from the body front.
        let mut doc = None;
        let mut interactive = None;
        let mut start = 0;
        if let Some(Value::Str(s)) = body.first() {
            if body.len() > 1 {
                doc = Some(s.borrow().clone());
                start = 1;
            }
        }
        for (i, f) in body.iter().enumerate().skip(start) {
            if let Value::Cons(c) = f {
                let b = c.borrow();
                if self.sym_is(&b.car, sym::INTERACTIVE) {
                    interactive = Some(f.clone());
                    start = i + 1;
                    break;
                }
                if self.sym_is(&b.car, self.intern_soft("declare").unwrap_or(SymId::MAX)) {
                    start = i + 1;
                    continue;
                }
            }
            break;
        }

        Ok(Lambda {
            is_macro: false,
            required,
            optional,
            rest,
            body: body[start..].to_vec(),
            env: self.lambda_env(),
            doc,
            interactive,
            name: name.map(|s| self.symbol_name(s)),
            bad_arglist,
            arglist: Some(params.clone()),
            plain: self.explicit_eval_depth > 0,
        })
    }

    fn define_error_conditions(&mut self) {
        let ec = self.intern("error-conditions");
        let put = |interp: &mut Interp, name: &str, conds: &[&str]| {
            let s = interp.intern(name);
            let list = Value::list(conds.iter().map(|c| Value::Sym(interp.intern(c))).collect());
            interp.put_prop(s, ec, list);
        };
        put(self, "error", &["error"]);
        put(self, "quit", &["quit"]);
        put(self, "user-error", &["user-error", "error"]);
        put(self, "arith-error", &["arith-error", "error"]);
        put(
            self,
            "range-error",
            &["range-error", "arith-error", "error"],
        );
        put(
            self,
            "domain-error",
            &["domain-error", "arith-error", "error"],
        );
        put(
            self,
            "overflow-error",
            &["overflow-error", "arith-error", "error"],
        );
        put(
            self,
            "underflow-error",
            &["underflow-error", "arith-error", "error"],
        );
        put(
            self,
            "wrong-type-argument",
            &["wrong-type-argument", "error"],
        );
        put(
            self,
            "wrong-number-of-arguments",
            &["wrong-number-of-arguments", "error"],
        );
        put(self, "args-out-of-range", &["args-out-of-range", "error"]);
        put(
            self,
            "wrong-length-argument",
            &["wrong-length-argument", "error"],
        );
        put(self, "void-function", &["void-function", "error"]);
        put(self, "void-variable", &["void-variable", "error"]);
        put(self, "setting-constant", &["setting-constant", "error"]);
        put(self, "invalid-function", &["invalid-function", "error"]);
        put(
            self,
            "invalid-read-syntax",
            &["invalid-read-syntax", "error"],
        );
        put(self, "circular-list", &["circular-list", "error"]);
        put(
            self,
            "beginning-of-buffer",
            &["beginning-of-buffer", "error"],
        );
        put(self, "end-of-buffer", &["end-of-buffer", "error"]);
        put(self, "buffer-read-only", &["buffer-read-only", "error"]);
        put(
            self,
            "text-read-only",
            &["text-read-only", "buffer-read-only", "error"],
        );
        put(self, "mark-inactive", &["mark-inactive", "error"]);
        put(self, "file-error", &["file-error", "error"]);
        put(
            self,
            "file-missing",
            &["file-missing", "file-error", "error"],
        );
        put(self, "end-of-file", &["end-of-file", "error"]);
        put(self, "search-failed", &["search-failed", "error"]);
        put(self, "no-catch", &["no-catch", "error"]);
        put(self, "scan-error", &["scan-error", "error"]);
        put(self, "invalid-regexp", &["invalid-regexp", "error"]);
        put(self, "recursion-error", &["recursion-error", "error"]);
        put(
            self,
            "unknown-image-type",
            &["unknown-image-type", "error"],
        );
        put(
            self,
            "file-already-exists",
            &["file-already-exists", "file-error", "error"],
        );
        put(
            self,
            "file-supersession",
            &["file-supersession", "file-error", "error"],
        );
        put(
            self,
            "permission-denied",
            &["permission-denied", "file-error", "error"],
        );
        // filelock.c
        put(
            self,
            "file-locked",
            &["file-locked", "file-error", "error"],
        );
        // cl-macs.el `cl-assert'
        put(
            self,
            "cl-assertion-failed",
            &["cl-assertion-failed", "error"],
        );
        put(
            self,
            "coding-system-error",
            &["coding-system-error", "error"],
        );
        put(
            self,
            "coding-conversion-error",
            &["coding-conversion-error", "error"],
        );
        // JSON errors (json.c): a `json-error' parent under `error'.
        put(self, "json-error", &["json-error", "error"]);
        put(
            self,
            "json-parse-error",
            &["json-parse-error", "json-error", "error"],
        );
        put(
            self,
            "json-end-of-file",
            &[
                "json-end-of-file",
                "json-parse-error",
                "json-error",
                "error",
            ],
        );
        put(self, "mark-set", &["mark-set"]);
        put(self, "mark-active", &["mark-active"]);

        // `error-message` property strings (as in Emacs's data.c put_error).
        let em = self.intern("error-message");
        let msgs: &[(&str, &str)] = &[
            ("error", "error"),
            ("quit", "Quit"),
            ("user-error", ""),
            ("arith-error", "Arithmetic error"),
            ("range-error", "Arithmetic range error"),
            ("domain-error", "Arithmetic domain error"),
            ("overflow-error", "Arithmetic overflow error"),
            ("underflow-error", "Arithmetic underflow error"),
            ("wrong-type-argument", "Wrong type argument"),
            ("wrong-number-of-arguments", "Wrong number of arguments"),
            ("args-out-of-range", "Args out of range"),
            ("wrong-length-argument", "Wrong length argument"),
            ("void-function", "Symbol's function definition is void"),
            ("void-variable", "Symbol's value as variable is void"),
            ("setting-constant", "Attempt to set a constant symbol"),
            ("invalid-function", "Invalid function"),
            ("invalid-read-syntax", "Invalid read syntax"),
            ("circular-list", "List contains a loop"),
            ("beginning-of-buffer", "Beginning of buffer"),
            ("end-of-buffer", "End of buffer"),
            ("buffer-read-only", "Buffer is read-only"),
            ("text-read-only", "Text is read-only"),
            ("mark-inactive", "The mark is not active now"),
            ("file-error", "File error"),
            ("file-missing", "File is missing"),
            ("end-of-file", "End of file during parsing"),
            ("search-failed", "Search failed"),
            ("no-catch", "No catch for tag"),
            ("scan-error", "Scan error"),
            ("invalid-regexp", "Invalid regexp"),
            ("coding-system-error", "Invalid coding system"),
            ("coding-conversion-error", "Coding conversion error"),
            ("file-already-exists", "File already exists"),
            ("file-supersession", "File is already being edited"),
            ("permission-denied", "Permission denied"),
            ("file-locked", "File is locked"),
            ("cl-assertion-failed", "Assertion failed"),
            (
                "recursion-error",
                "Variable binding depth exceeds max-specpdl-size",
            ),
            ("unknown-image-type", "Cannot determine image type"),
        ];
        for (name, msg) in msgs {
            let s = self.intern(name);
            self.put_prop(s, em, Value::string(*msg));
        }
    }

    fn define_special_variables(&mut self) {
        // Variables that are always dynamically bound even under
        // lexical-binding. Real Emacs has hundreds; these cover startup.
        let specials = [
            "standard-output",
            "standard-input",
            "lexical-binding",
            "inhibit-read-only",
            "load-path",
            "features",
            "command-line-args",
            "noninteractive",
            "emacs-version",
            "system-type",
            "debug-on-error",
            "max-lisp-eval-depth",
            "max-specpdl-size",
            "gc-cons-threshold",
            "command-history",
            "values",
            "obarray",
            "deactivate-mark",
            "transient-mark-mode",
            "kill-ring",
            "kill-ring-yank-pointer",
            "kill-ring-max",
            "last-command",
            "this-command",
            "current-prefix-arg",
            "prefix-arg",
            "minibuffer-history",
            "buffer-name-history",
            "read-expression-history",
            "command-line-args-left",
            "window-system",
            "global-map",
            "minibuffer-local-map",
            "overriding-local-map",
            "current-load-list",
            "load-in-progress",
            "load-file-name",
            "user-init-file",
            "print-level",
            "print-length",
            "print-circle",
            "most-positive-fixnum",
            "most-negative-fixnum",
            "before-change-functions",
            "after-change-functions",
            "first-change-hook",
            "post-self-insert-hook",
            "pre-command-hook",
            "post-command-hook",
            "kill-emacs-hook",
            "before-init-hook",
            "after-init-hook",
            "emacs-startup-hook",
            "delay-mode-hooks",
            "executing-kbd-macro",
            "defining-kbd-macro",
            "last-kbd-macro",
            "system-configuration",
            "system-name",
            "emacs-major-version",
            "emacs-minor-version",
            "doc-directory",
            "exec-directory",
            "exec-path",
            "process-environment",
            "path-separator",
            "null-device",
            "invocation-name",
            "invocation-directory",
            "history-length",
            "history-delete-duplicates",
            "history-add-new-input",
            "minibuffer-completion-table",
            "minibuffer-completion-predicate",
            "minibuffer-completion-confirm",
            "minibuffer-completing-file-name",
            "completion-ignore-case",
            "completion-styles",
            "read-circle",
            "find-file-hook",
            "find-file-not-found-hook",
            "write-file-functions",
            "write-contents-functions",
            "after-save-hook",
            "before-save-hook",
            "save-buffer-coding-system",
            "buffer-file-coding-system",
            "coding-system-for-write",
            "coding-system-for-read",
            "auto-mode-alist",
            "interpreter-mode-alist",
            "magic-mode-alist",
            "file-name-handler-alist",
            "completion-ignored-extensions",
            "buffer-offer-save",
            "enable-local-variables",
            "enable-local-eval",
            "safe-local-variable-values",
            "file-local-variables-alist",
            "permanent-local-variables",
            "change-major-mode-hook",
            "after-change-major-mode-hook",
            "make-backup-files",
            "backup-by-copying",
            "version-control",
            "kept-new-versions",
            "kept-old-versions",
            "delete-old-versions",
            "create-lockfiles",
            "temporary-file-directory",
            "revert-buffer-function",
            "auto-save-default",
            "auto-save-interval",
            "auto-save-timeout",
            "auto-save-file-name-transforms",
            "delete-auto-save-files",
            "require-final-newline",
            "sort-fold-case",
            "sort-numeric-base",
            "kill-buffer-hook",
            "kill-buffer-query-functions",
            "buffer-list-update-hook",
            "indent-tabs-mode",
            "tab-width",
            "fill-column",
            "standard-indent",
            "left-margin",
            "goal-column",
            "next-screen-context-lines",
            "scroll-conservatively",
            "scroll-margin",
            "scroll-up-aggressively",
            "scroll-down-aggressively",
            "scroll-preserve-screen-position",
            "scroll-error-top-bottom",
            "echo-keystrokes",
            "visible-bell",
            "inhibit-startup-screen",
            "inhibit-startup-message",
            "initial-major-mode",
            "initial-scratch-message",
            "user-full-name",
            "user-login-name",
            "user-mail-address",
            "user-uid",
            "kill-read-only-ok",
            "yank-excluded-properties",
            "set-mark-command-repeat-pop",
            "mark-even-if-inactive",
            "regexp-search-ring",
            "search-ring",
            "search-ring-max",
            "regexp-search-ring-max",
            "search-upper-case",
            "search-invisible",
            "search-whitespace-regexp",
            "case-replace",
            "case-fold-search",
            "isearch-forward",
            "isearch-regexp",
            "register-alist",
            "killed-rectangle",
            "undo-limit",
            "undo-strong-limit",
            "undo-outer-limit",
            "mark-ring-max",
            "global-mark-ring-max",
            "window-min-height",
            "window-min-width",
            "split-height-threshold",
            "split-width-threshold",
            "split-window-preferred-direction",
            "tooltip-mode",
            "image-type-file-name-regexps",
            "resize-mini-windows",
            "max-mini-window-height",
            "enable-recursive-minibuffers",
            "minibuffer-message-timeout",
            "read-buffer-function",
            "read-buffer-completion-ignore-case",
            "read-file-name-completion-ignore-case",
            "truncate-lines",
            "comment-start",
            "comment-end",
            "comment-start-skip",
            "comment-end-skip",
            "comment-column",
            "comment-padding",
            "comment-multi-line",
            "comment-empty-lines",
            "paragraph-start",
            "paragraph-separate",
            "paragraph-ignore-fill-prefix",
            "page-delimiter",
            "sentence-end",
            "sentence-end-double-space",
            "adaptive-fill-mode",
            "adaptive-fill-regexp",
            "adaptive-fill-function",
            "fill-prefix",
            "fill-paragraph-function",
            "fill-nobreak-predicate",
            "abbrev-mode",
            "save-abbrevs",
            "abbrev-file-name",
            "only-global-abbrevs",
            "double-click-time",
            "double-click-fuzz",
            "shell-file-name",
            "explicit-shell-file-name",
            "auto-save-hook",
            "delete-exited-processes",
            "process-connection-type",
            "undo-in-region",
            "undo-in-progress",
            "undo-no-redo",
            "undo-no-pull",
            "pending-undo-list",
            "undo-inhibit-record-point",
            "shift-select-mode",
            "delete-active-region",
            "yank-handled-properties",
            "query-replace-history",
            "auto-mode-case-fold",
            "use-dialog-box",
            "menu-prompting",
            "delayed-warnings-list",
            "delayed-warnings-hook",
            "minibuffer-prompt-properties",
            "eval-expression-print-level",
            "eval-expression-print-length",
            "indent-line-function",
            "comment-indent-function",
            "major-mode",
            "mode-name",
            "minor-mode-alist",
            "minor-mode-map-alist",
            "emulation-mode-map-alists",
            "global-minor-modes",
            "buffer-read-only",
            "default-directory",
            "buffer-file-name",
            "buffer-file-truename",
            "buffer-undo-list",
            "mark-ring",
            "mark-active",
            "mark-even-if-inactive",
            "use-empty-active-region",
            "exchange-point-and-mark-highlight-region",
            "global-mark-ring",
            "local-keymap",
            "list-buffers-directory",
            "buffer-saved-size",
            "buffer-display-table",
            "buffer-invisibility-spec",
            "selective-display",
            "overwrite-mode",
            "local-abbrev-table",
            "bidi-display-reordering",
            "header-line-format",
            "mode-line-format",
            "default-text-properties",
            "char-property-alias-alist",
            "inhibit-point-motion-hooks",
            "inhibit-field-text-motion",
            "show-trailing-whitespace",
            "indicate-empty-lines",
            "indicate-buffer-boundaries",
            "fringes-outside-margins",
            "word-wrap",
            "wrap-prefix",
            "line-prefix",
            "cache-long-line-scans",
            "cache-long-scans",
            "display-line-numbers",
            "line-spacing",
            "cursor-type",
            "scroll-bar-width",
            "left-fringe-width",
            "right-fringe-width",
            "left-margin-width",
            "right-margin-width",
            "print-gensym",
            "print-escape-newlines",
            "print-quoted",
            "print-unreadable",
            "print-gensym-alist",
            "print-continuous-numbering",
            "print-number-table",
            "filter-buffer-substring-function",
        ];

        for name in &specials {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).special = true;
        }

        // Printer variables with their Emacs defaults.
        for name in ["print-quoted", "print-unreadable"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::t();
        }
        // defvar'd to nil in GNU (simple.el).
        let id = self.intern("filter-buffer-substring-function");
        self.obarray.symbol_mut(id).value = Value::Nil;
        for name in ["print-gensym", "print-escape-newlines", "print-circle"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::Nil;
        }

        // Variables that are automatically buffer-local when set
        // (`make-variable-buffer-local' / DEFVAR_PER_BUFFER semantics in
        // GNU).  Set verified against GNU 31.1 `-Q --batch' via
        // `local-variable-if-set-p' in a fresh buffer.
        let auto_locals = [
            "tab-width",
            "fill-column",
            "indent-tabs-mode",
            "truncate-lines",
            "word-wrap",
            "case-fold-search",
            "mark-ring",
            "comment-column",
            "fill-prefix",
            "local-abbrev-table",
            "window-size-fixed",
            "abbrev-mode",
            "overwrite-mode",
            "buffer-display-table",
            "selective-display",
            "selective-display-ellipses",
            "indicate-empty-lines",
            "indicate-buffer-boundaries",
            "left-margin",
            "goal-column",
            "scroll-up-aggressively",
            "scroll-down-aggressively",
            "left-fringe-width",
            "right-fringe-width",
            "fringes-outside-margins",
            "scroll-bar-width",
            "scroll-bar-height",
            "vertical-scroll-bar",
            "horizontal-scroll-bar",
            "line-spacing",
            "left-margin-width",
            "right-margin-width",
            "cursor-type",
            "cursor-in-non-selected-windows",
            "mode-line-format",
            "header-line-format",
            "tab-line-format",
            "display-line-numbers",
            "display-line-numbers-offset",
            "display-line-numbers-width",
            "display-line-numbers-widen",
            "wrap-prefix",
            "line-prefix",
            "display-fill-column-indicator",
            "display-fill-column-indicator-column",
            "display-fill-column-indicator-character",
            "show-trailing-whitespace",
            "bidi-paragraph-direction",
            "bidi-paragraph-start-re",
            "bidi-display-reordering",
            "buffer-file-coding-system",
            "save-buffer-coding-system",
            "deactivate-mark",
            "file-local-variables-alist",
            "lexical-binding",
            "buffer-offer-save",
            "font-lock-defaults",
            "current-input-method",
            "text-property-default-nonsticky",
        ];
        for name in &auto_locals {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).make_local_if_set = true;
            self.obarray.symbol_mut(id).special = true;
        }

        // GNU `DEFVAR_PER_BUFFER' variables: a real slot in every
        // buffer, so `local-variable-p' is t even before any `setq'
        // (set verified against GNU 31.1 in a fresh buffer).
        let per_buffer: &[&str] = &[
            "buffer-auto-save-file-name",
            "buffer-backed-up",
            "buffer-display-count",
            "buffer-file-format",
            "buffer-file-name",
            "buffer-file-truename",
            "buffer-invisibility-spec",
            "buffer-read-only",
            "buffer-saved-size",
            "buffer-undo-list",
            "default-directory",
            "enable-multibyte-characters",
            "major-mode",
            "mark-active",
            "mode-name",
        ];
        for name in per_buffer {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).always_local = true;
            self.obarray.symbol_mut(id).special = true;
        }

        // GNU's C-backed variables: `makunbound' refuses on them
        // (the "Built-in variable may not be unbound" set, verified
        // against GNU 31.1).
        let builtin_locals: &[&str] = &[
            "abbrev-mode",
            "after-change-functions",
            "before-change-functions",
            "bidi-display-reordering",
            "bidi-inhibit-bpa",
            "bidi-paragraph-direction",
            "bidi-paragraph-start-re",
            "buffer-auto-save-file-name",
            "buffer-backed-up",
            "buffer-display-table",
            "buffer-file-coding-system",
            "buffer-file-name",
            "buffer-file-truename",
            "buffer-invisibility-spec",
            "buffer-read-only",
            "buffer-saved-size",
            "buffer-undo-list",
            "cache-long-scans",
            "case-fold-search",
            "change-major-mode-hook",
            "char-property-alias-alist",
            "coding-system-for-read",
            "coding-system-for-write",
            "cursor-in-non-selected-windows",
            "cursor-type",
            "deactivate-mark",
            "default-directory",
            "default-text-properties",
            "delayed-warnings-list",
            "delete-exited-processes",
            "display-fill-column-indicator",
            "display-fill-column-indicator-character",
            "display-fill-column-indicator-column",
            "display-line-numbers",
            "display-line-numbers-current-absolute",
            "display-line-numbers-major-tick",
            "display-line-numbers-minor-tick",
            "display-line-numbers-offset",
            "display-line-numbers-widen",
            "display-line-numbers-width",
            "double-click-fuzz",
            "double-click-time",
            "emulation-mode-map-alists",
            "fill-column",
            "first-change-hook",
            "fringes-outside-margins",
            "header-line-format",
            "horizontal-scroll-bar",
            "indent-tabs-mode",
            "indicate-buffer-boundaries",
            "indicate-empty-lines",
            "inhibit-field-text-motion",
            "inhibit-point-motion-hooks",
            "left-fringe-width",
            "left-margin",
            "left-margin-width",
            "lexical-binding",
            "line-prefix",
            "line-spacing",
            "local-abbrev-table",
            "major-mode",
            "mark-active",
            "menu-prompting",
            "minibuffer-prompt-properties",
            "minor-mode-map-alist",
            "mode-line-format",
            "mode-name",
            "next-screen-context-lines",
            "overwrite-mode",
            "post-command-hook",
            "post-self-insert-hook",
            "pre-command-hook",
            "print-continuous-numbering",
            "print-escape-newlines",
            "print-gensym",
            "print-number-table",
            "print-quoted",
            "process-connection-type",
            "right-fringe-width",
            "right-margin-width",
            "scroll-bar-height",
            "scroll-bar-width",
            "scroll-conservatively",
            "scroll-down-aggressively",
            "scroll-margin",
            "scroll-preserve-screen-position",
            "scroll-up-aggressively",
            "selective-display",
            "selective-display-ellipses",
            "shell-file-name",
            "show-trailing-whitespace",
            "tab-line-format",
            "tab-width",
            "transient-mark-mode",
            "truncate-lines",
            "use-dialog-box",
            "vertical-scroll-bar",
            "word-wrap",
            "wrap-prefix",
        ];
        for name in builtin_locals {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).builtin_variable = true;
        }

        // Initial values.
        let defs: &[(&str, Value)] = &[
            ("emacs-major-version", Value::Int(31)),
            ("emacs-minor-version", Value::Int(1)),
            ("emacs-version", Value::string("31.1.0 (remacs)")),
            ("system-type", Value::Sym(self.intern("darwin"))),
            (
                "system-configuration",
                Value::string("aarch64-apple-darwin"),
            ),
            ("fill-column", Value::Int(70)),
            ("tab-width", Value::Int(8)),
            ("standard-indent", Value::Int(4)),
            ("indent-tabs-mode", Value::t()),
            ("case-fold-search", Value::t()),
            ("case-replace", Value::t()),
            ("noninteractive", Value::Nil),
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
            ("history-length", Value::Int(60)),
            ("kill-ring-max", Value::Int(120)),
            ("mark-ring-max", Value::Int(16)),
            ("global-mark-ring-max", Value::Int(16)),
            ("scroll-margin", Value::Int(0)),
            ("scroll-conservatively", Value::Int(0)),
            ("next-screen-context-lines", Value::Int(2)),
            ("undo-limit", Value::Int(160_000)),
            ("undo-strong-limit", Value::Int(240_000)),
            ("undo-outer-limit", Value::Int(24_000_000)),
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
                Value::Sym(self.intern("longest")),
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
                    .map(|(re, ty)| {
                        Value::cons(
                            Value::string(*re),
                            Value::Sym(self.intern(ty)),
                        )
                    })
                    .collect(),
                ),
            ),
            ("completion-ignore-case", Value::Nil),
            ("read-buffer-completion-ignore-case", Value::Nil),
            ("read-file-name-completion-ignore-case", Value::Nil),
            ("history-delete-duplicates", Value::Nil),
            ("minibuffer-history", Value::Nil),
            ("buffer-name-history", Value::Nil),
            ("kill-ring", Value::Nil),
            ("values", Value::Nil),
            ("print-level", Value::Nil),
            ("print-length", Value::Nil),
            ("print-circle", Value::Nil),
            ("load-path", Value::Nil),
            // GNU: `load-file-name' is a C variable bound to nil
            // outside of `load' (Vload_file_name), so it is always
            // readable (e.g. by `custom-current-group').
            ("load-file-name", Value::Nil),
            (
                "default-directory",
                Value::string({
                    let mut d = std::env::current_dir()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "/".to_string());
                    if !d.ends_with('/') {
                        d.push('/');
                    }
                    d
                }),
            ),
            ("buffer-undo-list", Value::Nil),
            ("mark-ring", Value::Nil),
            ("text-quoting-style", Value::Nil),
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
            (
                "obarray",
                Value::Record(std::rc::Rc::new(std::cell::RefCell::new(vec![
                    Value::Sym(self.intern("obarray")),
                    Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
                        Value::Nil;
                        4
                    ]))),
                ]))),
            ),
            (
                "features",
                // Kept in sync with `self.features' by `provide'.
                Value::list(
                    self.features
                        .iter()
                        .map(|s| Value::Sym(*s))
                        .collect(),
                ),
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
            ("lexical-binding", Value::Sym(sym::T)),
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
            ("current-prefix-arg", Value::Nil),
            ("prefix-arg", Value::Nil),
            ("unread-command-events", Value::Nil),
            ("unread-post-input-method-events", Value::Nil),
            ("quit-flag", Value::Nil),
            ("inhibit-quit", Value::Nil),
            ("throw-on-input", Value::Nil),
            ("mark-active", Value::Nil),
            ("minibuffer-inactive-mode", Value::Nil),
            // Hook/option variables with GNU defaults.
            ("before-change-functions", Value::Nil),
            ("after-change-functions", Value::Nil),
            ("inhibit-modification-hooks", Value::Nil),
            ("inhibit-point-motion-hooks", Value::Nil),
            ("inhibit-field-text-motion", Value::Nil),
            ("focus-in-hook", Value::Nil),
            ("focus-out-hook", Value::Nil),
            ("after-focus-change-function", Value::Nil),
            ("minor-mode-map-alist", Value::Nil),
            ("emulation-mode-map-alists", Value::Nil),
            ("overriding-terminal-local-map", Value::Nil),
            ("char-property-alias-alist", Value::Nil),
            ("default-text-properties", Value::Nil),
            ("mode-line-format", Value::Nil),
            ("header-line-format", Value::Nil),
            ("tab-line-format", Value::Nil),
            ("truncate-partial-width-windows", Value::t()),
            ("word-wrap", Value::Nil),
            ("scroll-step", Value::Int(0)),
            ("scroll-error-top-bottom", Value::Nil),
            ("scroll-preserve-screen-position", Value::Sym(sym::T)),
            ("scroll-up-aggressively", Value::Nil),
            ("scroll-down-aggressively", Value::Nil),
            ("scroll-minibuffer-conservatively", Value::Sym(sym::T)),
            ("hscroll-margin", Value::Int(5)),
            ("hscroll-step", Value::Int(0)),
            ("auto-hscroll-mode", Value::Sym(sym::T)),
            ("polling-period", Value::float(0.5)),
            ("visible-bell", Value::Nil),
            ("ring-bell-function", Value::Nil),
            ("cursor-type", Value::Sym(sym::T)),
            ("cursor-in-non-selected-windows", Value::Sym(sym::T)),
            ("blink-matching-paren", Value::Sym(sym::T)),
            ("show-paren-mode", Value::Nil),
            ("sentence-end", Value::Nil),
            ("recenter-redisplay", Value::Sym(self.intern("tty"))),
            ("redisplay-dont-pause", Value::Sym(sym::T)),
            ("fast-but-imprecise-scrolling", Value::Nil),
            ("print-escape-multibyte", Value::Nil),
            ("print-escape-nonascii", Value::Nil),
            ("print-gensym", Value::Nil),
            ("print-continuous-numbering", Value::Nil),
            ("print-number-table", Value::Nil),
            ("print-readably", Value::Sym(sym::T)),
            ("eval-expression-print-length", Value::Nil),
            ("eval-expression-print-level", Value::Nil),
            ("font-lock-maximum-decoration", Value::Sym(sym::T)),
            ("font-lock-maximum-size", Value::Nil),
            ("jit-lock-stealth-time", Value::Nil),
            ("jit-lock-stealth-load", Value::Int(200)),
            ("jit-lock-defer-time", Value::Nil),
            ("jit-lock-context-time", Value::float(0.5)),
            ("jit-lock-functions", Value::Nil),
            (
                "jit-lock-contextually",
                Value::Sym(self.intern("syntax-driven")),
            ),
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
                Value::Sym(self.intern("after-completion")),
            ),
            ("ctl-arrow", Value::Sym(sym::T)),
            ("current-input-method", Value::Nil),
            ("cursor-in-echo-area", Value::Nil),
            ("debug-on-quit", Value::Nil),
            ("default-justification", Value::Sym(self.intern("left"))),
            ("delete-old-versions", Value::Nil),
            ("enable-recursive-minibuffers", Value::Nil),
            ("eval-expression-debug-on-error", Value::Sym(sym::T)),
            ("find-file-existing-other-name", Value::Sym(sym::T)),
            ("find-file-visit-truename", Value::Nil),
            ("gc-cons-percentage", Value::float(1.0)),
            ("idle-update-delay", Value::float(0.5)),
            ("input-method-function", Value::Nil),
            ("insert-default-directory", Value::Sym(sym::T)),
            ("isearch-allow-scroll", Value::Nil),
            ("isearch-case-fold-search", Value::Nil),
            ("isearch-lazy-highlight", Value::Sym(sym::T)),
            ("isearch-regexp", Value::Nil),
            ("left-margin-width", Value::Int(0)),
            ("line-number-display-limit", Value::Nil),
            ("list-buffers-directory", Value::Nil),
            (
                "locale-coding-system",
                Value::Sym(self.intern("utf-8-unix")),
            ),
            ("make-backup-files", Value::Sym(sym::T)),
            ("message-log-max", Value::Int(1000)),
            ("mode-require-final-newline", Value::Sym(sym::T)),
            ("next-error-highlight", Value::float(0.5)),
            ("no-redraw-on-reenter", Value::Nil),
            (
                "normal-erase-is-backspace",
                Value::Sym(self.intern("maybe")),
            ),
            ("paragraph-separate", Value::string("[ \t\x0c]*$")),
            ("paragraph-start", Value::string("\x0c\\|[ \t]*$")),
            ("page-delimiter", Value::string("^\x0c")),
            ("parse-sexp-ignore-comments", Value::Nil),
            ("require-final-newline", Value::Sym(sym::T)),
            ("sort-numeric-base", Value::Int(10)),
            ("sort-fold-case", Value::Nil),
            ("resize-mini-windows", Value::Sym(self.intern("grow-only"))),
            ("select-enable-clipboard", Value::Sym(sym::T)),
            ("selection-coding-system", Value::Nil),
            (
                "send-mail-function",
                Value::Sym(self.intern("sendmail-query-once")),
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
            ("overflow-newline-into-fringe", Value::Nil),
            (
                "uniquify-buffer-name-style",
                Value::Sym(self.intern("post-forward-angle-brackets")),
            ),
            ("use-file-dialog", Value::Sym(sym::T)),
            ("version-control", Value::Nil),
            ("visible-cursor", Value::Sym(sym::T)),
            ("write-contents-hooks", Value::Nil),
            (
                "yank-excluded-properties",
                Value::list(vec![
                    Value::Sym(self.intern("category")),
                    Value::Sym(self.intern("field")),
                    Value::Sym(self.intern("follow-link")),
                    Value::Sym(self.intern("fontified")),
                    Value::Sym(self.intern("font-lock-face")),
                    Value::Sym(self.intern("help-echo")),
                    Value::Sym(self.intern("intangible")),
                    Value::Sym(self.intern("invisible")),
                    Value::Sym(self.intern("keymap")),
                    Value::Sym(self.intern("local-map")),
                    Value::Sym(self.intern("mouse-face")),
                    Value::Sym(self.intern("read-only")),
                    Value::Sym(self.intern("yank-handler")),
                ]),
            ),
            ("kill-ring-yank-pointer", Value::Nil),
            ("buffer-auto-save-file-name", Value::Nil),
            (
                "buffer-file-coding-system",
                Value::Sym(self.intern("utf-8-unix")),
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
                Value::Sym(self.intern("command-error-default-function")),
            ),
            ("post-command-hook", Value::Nil),
            ("pre-command-hook", Value::Nil),
            ("minibuffer-setup-hook", Value::Nil),
            ("minibuffer-exit-hook", Value::Nil),
            ("minibuffer-help-form", Value::Nil),
            ("minibuffer-scroll-window", Value::Nil),
            ("enable-local-variables", Value::Sym(sym::T)),
            ("enable-local-eval", Value::Sym(self.intern("maybe"))),
            ("buffer-access-fontify-functions", Value::Nil),
            ("redisplay--variables", Value::Nil),
            ("buffer-display-count", Value::Int(0)),
            ("buffer-display-time", Value::Nil),
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
                Value::Sym(self.intern("lisp-indent-function")),
            ),
            (
                "indent-line-function",
                Value::Sym(self.intern("indent-relative")),
            ),
            ("electric-indent-mode", Value::Sym(sym::T)),
            ("comment-multi-line", Value::Nil),
            ("comment-empty-lines", Value::Sym(sym::T)),
            ("comment-fill-column", Value::Nil),
            ("comment-use-syntax", Value::Sym(self.intern("undecided"))),
            ("fill-nospace-between-words", Value::Sym(sym::T)),
            ("auto-fill-function", Value::Nil),
            ("normal-auto-fill-function", Value::Nil),
            ("adaptive-fill-mode", Value::Sym(sym::T)),
            (
                "adaptive-fill-regexp",
                Value::string("[-–!|#%;>*·•‣⁃◦ \t]*"),
            ),
            ("adaptive-fill-function", Value::Nil),
            (
                "adaptive-fill-first-line-regexp",
                Value::string("\\`[ \t]*\\'"),
            ),
            ("kill-do-not-save-duplicates", Value::Nil),
            ("kill-read-only-ok", Value::Sym(sym::T)),
            ("kill-transform-function", Value::Nil),
            ("yank-handled-properties", Value::Nil),
            ("set-mark-command-repeat-pop", Value::Nil),
            ("yank-pop-change-selection", Value::Nil),
            ("x-select-request-type", Value::Nil),
            ("search-invisible", Value::Sym(self.intern("open"))),
            ("search-spaces-regexp", Value::Nil),
            ("search-whitespace-regexp", Value::string("[ \t\r\n]+")),
            ("search-nonincremental-instead", Value::Sym(sym::T)),
            ("isearch-hide-immediately", Value::Sym(sym::T)),
            ("isearch-resume-in-command-history", Value::Nil),
            ("isearch-allow-prefix", Value::Nil),
            ("isearch-push-state-function", Value::Nil),
            ("lazy-highlight-initial-delay", Value::float(0.25)),
            ("lazy-highlight-interval", Value::Int(0)),
            ("lazy-highlight-max-at-a-time", Value::Int(20)),
            ("lazy-highlight-cleanup", Value::Sym(sym::T)),
            ("lazy-highlight-no-delay-length", Value::Int(3)),
            ("query-replace-from-to-separator", Value::Nil),
            ("query-replace-lazy-highlight", Value::Sym(sym::T)),
            ("query-replace-show-replacement", Value::Sym(sym::T)),
            ("query-replace-skip-read-only", Value::Nil),
            ("query-replace-highlight", Value::Sym(sym::T)),
            (
                "recenter-positions",
                Value::list(vec![
                    Value::Sym(self.intern("middle")),
                    Value::Sym(self.intern("top")),
                    Value::Sym(self.intern("bottom")),
                ]),
            ),
            ("handle-shift-selection", Value::Nil),
            ("read-buffer-function", Value::Nil),
            ("read-expression-map", Value::Nil),
            ("read-circle", Value::Sym(sym::T)),
            ("debugger", Value::Sym(self.intern("debug"))),
            ("debug-ignored-errors", Value::Nil),
            ("signal-hook-function", Value::Nil),
            ("delayed-warnings-list", Value::Nil),
            ("delayed-warnings-hook", Value::Nil),
            ("garbage-collection-messages", Value::Nil),
            ("post-gc-hook", Value::Nil),
            ("gc-message-percentage", Value::Nil),
            ("user-emacs-directory", Value::string("~/.emacs.d/")),
            ("native-comp-driver-options", Value::Nil),
            ("module-file-suffix", Value::string(".so")),
            ("dynamic-library-alist", Value::Nil),
            ("load-prefer-newer", Value::Nil),
            ("load-read-function", Value::Sym(self.intern("read"))),
            ("obarray-size", Value::Int(15121)),
            ("max-image-size", Value::float(10.0)),
            ("image-scaling-factor", Value::string("auto")),
            ("use-short-answers", Value::Nil),
            ("yes-or-no-prompt", Value::Nil),
            ("async-shell-command-display-buffer", Value::Sym(sym::T)),
            (
                "shell-command-default-error-buffer",
                Value::string("*Shell Command Error*"),
            ),
            ("shell-command-prompt-show-cwd", Value::Sym(sym::T)),
            (
                "exec-suffixes",
                Value::list(vec![
                    Value::string(".exec"),
                    Value::string(".exe"),
                    Value::string(".com"),
                    Value::string(".bat"),
                    Value::string(".cmd"),
                    Value::string(".btm"),
                    Value::string(""),
                ]),
            ),
            ("process-connection-type", Value::Sym(sym::T)),
            ("delete-exited-processes", Value::Sym(sym::T)),
            ("process-adaptive-read-buffering", Value::Sym(sym::T)),
            ("process-error-pause-time", Value::Int(0)),
            ("network-security-level", Value::Sym(self.intern("medium"))),
            ("mail-host-address", Value::Nil),
            (
                "system-configuration",
                Value::string("aarch64-apple-darwin"),
            ),
            (
                "emacs-copyright",
                Value::string("Copyright (C) 2025 Free Software Foundation, Inc."),
            ),
            ("emacs-build-time", Value::Nil),
            ("emacs-build-system", Value::Nil),
            ("emacs-repository-version", Value::string("emacs-31.1")),
            ("emacs-repository-branch", Value::Nil),
            ("internal-initialization-file", Value::Nil),
            ("site-run-file", Value::string("site-start")),
            ("installation-directory", Value::Nil),
            ("invocation-name", Value::string("remacs")),
            ("invocation-directory", Value::Nil),
            ("source-directory", Value::Nil),
            ("emacs-program-version", Value::string("31.1")),
            ("emacs-major-version", Value::Int(31)),
            ("keyboard-type", Value::Sym(self.intern("pc"))),
            ("window-system", Value::Nil),
            ("initial-window-system", Value::Nil),
            ("daemon-socket", Value::Nil),
            ("glyph-table", Value::Nil),
            (
                "charset-list",
                Value::list(
                    [
                        "ascii",
                        "unicode",
                        "emacs",
                        "eight-bit",
                        "ucs",
                        "iso-8859-1",
                        "latin-iso8859-1",
                        "eight-bit-control",
                        "eight-bit-graphic",
                        "control-1",
                        "mule-unicode-0100-24ff",
                        "mule-unicode-2500-33ff",
                        "mule-unicode-e000-ffff",
                    ]
                    .iter()
                    .map(|n| Value::Sym(self.intern(n)))
                    .collect(),
                ),
            ),
            ("charset-map-path", Value::Nil),
            ("char-code-property-alist", Value::Nil),
            ("unicode-category-table", Value::Nil),
            ("current-language-environment", Value::string("English")),
            ("default-input-method", Value::Nil),
            (
                "input-method-verbose-flag",
                Value::Sym(self.intern("complex-only")),
            ),
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
                Value::Sym(self.intern("utf-8-hfs-unix")),
            ),
            (
                "default-file-name-coding-system",
                Value::Sym(self.intern("utf-8-unix")),
            ),
            (
                "default-terminal-coding-system",
                Value::Sym(self.intern("utf-8-unix")),
            ),
            ("file-name-shadow-mode", Value::Sym(sym::T)),
            (
                "show-help-function",
                Value::Sym(self.intern("tooltip-show-help")),
            ),
            ("delete-trailing-lines", Value::Sym(sym::T)),
            (
                "keyboard-coding-system",
                Value::Sym(self.intern("utf-8-unix")),
            ),
            (
                "default-process-coding-system",
                Value::cons(
                    Value::Sym(self.intern("utf-8-unix")),
                    Value::Sym(self.intern("utf-8-unix")),
                ),
            ),
            ("emacs-basic-display", Value::Nil),
            ("auto-coding-alist", Value::Nil),
            ("auto-coding-functions", Value::Nil),
            ("auto-coding-regexp-alist", Value::Nil),
            ("set-auto-coding-function", Value::Nil),
            ("coding-system-list", Value::Nil),
            ("coding-system-alist", Value::Nil),
            ("coding-category-list", Value::Nil),
            ("translation-table-for-input", Value::Nil),
            ("file-name-handler-alist", Value::Nil),
            ("directory-listing-before-filename-regexp", Value::Nil),
            ("directory-free-space-program", Value::string("df")),
            ("directory-free-space-args", Value::string("-k")),
            ("directory-sep-char", Value::Int(47)),
            ("insert-directory-program", Value::string("ls")),
            ("auto-save-list-file-prefix", Value::Nil),
            ("auto-save-visited-file-name", Value::Nil),
            ("auto-save-mode", Value::Nil),
            ("delete-auto-save-files", Value::Sym(sym::T)),
            ("auto-save-include-big-deletions", Value::Nil),
            ("buffer-offer-save", Value::Nil),
            ("kept-new-versions", Value::Int(2)),
            ("kept-old-versions", Value::Int(2)),
            ("vc-handled-backends", Value::Nil),
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
            (
                "ad-default-compilation-action",
                Value::Sym(self.intern("maybe")),
            ),
            ("read-symbol-positions-list", Value::Nil),
            ("eval-expression-print-maximum-character", Value::Int(127)),
            ("print-integers-as-characters", Value::Sym(sym::T)),
            ("print-escape-control-characters", Value::Nil),
            (
                "print-charset-text-property",
                Value::Sym(self.intern("default")),
            ),
            ("print-unreadable-function", Value::Nil),
            ("eval-depth", Value::Int(0)),
            ("internal-interpreter-environment", Value::Nil),
            ("internal-make-interpreted-closure-function", Value::Nil),
            ("internal--label-uninterned-symbols", Value::Nil),
            ("internal--forge-builtin-symbols", Value::Nil),
            ("macroexp--debug-eager", Value::Nil),
            ("macroexpand-all-environment", Value::Nil),
            ("internal-macroexpand-for-load", Value::Nil),
            ("internal-cons-cell-stats", Value::Nil),
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
            ("command-line-default-directory", Value::Nil),
            ("command-line-args-left", Value::Nil),
            ("command-line-normalized-file-name", Value::Nil),
            ("emacs-startup-hook", Value::Nil),
            ("term-setup-hook", Value::Nil),
            ("window-setup-hook", Value::Nil),
            ("before-init-hook", Value::Nil),
            ("after-init-hook", Value::Nil),
            ("init-file-debug", Value::Nil),
            ("init-file-user", Value::Nil),
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
            ("scroll-bar-mode", Value::Nil),
            ("horizontal-scroll-bar-mode", Value::Nil),
            ("default-frame-alist", Value::Nil),
            ("initial-frame-alist", Value::Nil),
            ("minibuffer-frame-alist", Value::Nil),
            (
                "default-frame-scroll-bars",
                Value::Sym(self.intern("right")),
            ),
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
            (
                "mouse-1-click-follows-link",
                Value::Sym(self.intern("double")),
            ),
            ("mouse-1-click-in-non-selected-windows", Value::Sym(sym::T)),
            ("mouse-wheel-scroll-amount", Value::Nil),
            ("mouse-wheel-scroll-amount-horizontal", Value::Nil),
            ("mouse-wheel-tilt-scroll", Value::Nil),
            ("mouse-wheel-flip-direction", Value::Nil),
            ("mouse-wheel-progressive-speed", Value::Sym(sym::T)),
            ("mouse-wheel-follow-mouse", Value::Sym(sym::T)),
            ("mouse-wheel-mode", Value::Sym(sym::T)),
            ("scroll-bar-adjust-thumb-portion", Value::Nil),
            ("x-stretch-cursor", Value::Nil),
            ("x-use-underline-position-properties", Value::Nil),
            ("x-underline-at-descent-line", Value::Nil),
            ("x-mouse-clip-rectangular", Value::Sym(sym::T)),
            ("shell-file-name", Value::string("/bin/sh")),
            ("path-separator", Value::string(":")),
            ("null-device", Value::string("/dev/null")),
            (
                "temporary-file-directory",
                Value::string({
                    let d = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
                    if d.ends_with('/') { d } else { format!("{d}/") }
                }),
            ),
            ("exec-directory", Value::string("/usr/local/bin/")),
            (
                "doc-directory",
                // Locate the host GNU Emacs's etc/ dir (which holds the
                // DOC file) via the `emacs' on PATH; fall back to the
                // conventional share path.
                Value::string(
                    std::env::var_os("PATH")
                        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
                        .unwrap_or_default()
                        .into_iter()
                        .chain(
                            [
                                "/run/current-system/sw/bin",
                                "/usr/bin",
                                "/usr/local/bin",
                                "/opt/homebrew/bin",
                            ]
                            .into_iter()
                            .map(std::path::PathBuf::from),
                        )
                        .find_map(|dir| doc_dir_of_emacs(&dir.join("emacs"), 0))
                        .unwrap_or_else(|| "/usr/share/emacs/".into()),
                ),
            ),
            (
                "initial-major-mode",
                Value::Sym(self.intern("lisp-interaction-mode")),
            ),
            (
                "initial-scratch-message",
                Value::string(
                    ";; This buffer is for text that is not saved, and for Lisp evaluation.\n;; To create a file, visit it with C-x C-f and enter text in its buffer.\n\n",
                ),
            ),
            ("inhibit-startup-screen", Value::Nil),
            ("user-full-name", Value::string("user")),
            ("user-login-name", Value::string("user")),
            ("user-mail-address", Value::string("user@localhost")),
            ("user-real-login-name", Value::string("user")),
            ("user-uid", Value::Int(1000)),
            ("user-real-uid", Value::Int(1000)),
            ("history-add-new-input", Value::t()),
            ("save-abbrevs", Value::t()),
            ("debug-on-error", Value::Nil),
            ("sentence-end-double-space", Value::t()),
            ("inhibit-startup-message", Value::t()),
            ("use-dialog-box", Value::Nil),
            ("menu-prompting", Value::Nil),
            ("minibuffer-prompt-properties", Value::Nil),
            (
                "completion-styles",
                Value::list(vec![Value::Sym(self.intern("basic"))]),
            ),
        ];
        for (name, val) in defs {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = val.clone();
        }
    }

    // ---------- buffers ----------

    /// The current buffer's shared handle.
    pub fn current_buffer_ref(&self) -> Option<Rc<RefCell<crate::buffer::Buffer>>> {
        self.buffers.get(self.current_buffer)
    }

    /// Buffer id for a `Value::Buffer` or `Value::Str` (name).
    pub fn buffer_id_of(&self, v: &Value) -> Option<usize> {
        match v {
            Value::Buffer(b) => Some(b.borrow().id),
            Value::Str(s) => self.buffers.by_name(&s.borrow()),
            _ => None,
        }
    }

    /// `Value` for a live buffer id.
    pub fn buffer_value(&self, id: usize) -> Option<Value> {
        self.buffers.get(id).map(|r| Value::Buffer(r.clone()))
    }

    pub fn buffer_name(&self, id: usize) -> Option<String> {
        self.buffers.get(id).map(|b| b.borrow().name.clone())
    }

    pub fn set_current_buffer(&mut self, id: usize) {
        // Emacs: `set-buffer'/`with-current-buffer' do not update
        // buffer-list recency — only window selection/display does.
        if self.buffers.get(id).is_some() {
            self.current_buffer = id;
        }
    }

    /// With `set-buffer` semantics — no error for dead/missing.
    pub fn current_buffer_is_live(&self) -> bool {
        self.buffers.get(self.current_buffer).is_some()
    }

    /// `current-case-table': the buffer's local case table if set,
    /// else `standard-case-table'.
    pub fn current_case_table(&mut self) -> Value {
        let local = self
            .buffers
            .get(self.current_buffer)
            .and_then(|b| b.try_borrow().ok().and_then(|bb| bb.case_table.clone()));
        match local {
            Some(t) => t,
            None => self.standard_case_table(),
        }
    }

    /// `standard-case-table': the shared case-table char-table.
    /// GNU's layout: contents = downcase map, extra slots =
    /// {upcase, canonicalize, equivalency} char-tables.  All four are
    /// replayed from GNU Emacs 31's printed tries (see ctdata.rs),
    /// giving exact >255 contents and trie structure.
    pub fn standard_case_table(&mut self) -> Value {
        if let Some(v) = &self.standard_case_table {
            return v.clone();
        }
        use crate::lisp::builtins::misc::{ct_replay, make_ct};
        let case_tag = Value::Sym(self.intern("case-table"));
        // GNU's case tables physically carry 3 extra slots each
        // (`char-table-extra-slots' = 3); most stay nil.
        let build = |i: &mut Self, ops: &str| {
            let t = make_ct(
                i,
                case_tag.clone(),
                Value::Nil,
                vec![Value::Nil, Value::Nil, Value::Nil],
            );
            ct_replay(i, &t, ops);
            t
        };
        use crate::lisp::ctdata::*;
        let up = build(self, CASE_UP_OPS);
        let canon = build(self, CASE_CANON_OPS);
        let equiv = build(self, CASE_EQUIV_OPS);
        let down = build(self, CASE_OPS);
        if let (Value::Record(d), Value::Record(c)) = (&down, &canon) {
            let mut rr = d.borrow_mut();
            rr[3] = up;
            rr[4] = canon.clone();
            rr[5] = equiv.clone();
            // GNU's canon table shares the equivalencies table in its
            // own third extra slot.
            c.borrow_mut()[5] = equiv;
        }
        self.standard_case_table = Some(down.clone());
        down
    }

    /// Char-table parent accessor (record identity → parent value).
    pub fn char_table_parent(&self, table: &Value) -> Value {
        let id = match table {
            Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
            _ => return Value::Nil,
        };
        self.char_table_parents
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's parent (side-table; record layout untouched).
    pub fn set_char_table_parent(&mut self, table: &Value, parent: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_parents.retain(|(k, _)| *k != id);
            if !parent.is_nil() {
                self.char_table_parents.push((id, parent));
            }
        }
    }

    /// Char-table defalt accessor (record identity → defalt value).
    pub fn char_table_defalt(&self, table: &Value) -> Value {
        let id = match table {
            Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
            _ => return Value::Nil,
        };
        self.char_table_defalts
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's defalt (side-table; record layout untouched).
    pub fn set_char_table_defalt(&mut self, table: &Value, defalt: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_defalts.retain(|(k, _)| *k != id);
            if !defalt.is_nil() {
                self.char_table_defalts.push((id, defalt));
            }
        }
    }

    /// `standard-category-table': the shared category table, with
    /// GNU's ASCII membership and label docstrings.
    pub fn standard_category_table(&mut self) -> Value {
        if let Some(v) = &self.standard_category_table {
            return v.clone();
        }
        let t = crate::lisp::builtins::misc::make_category_table_value(self, true);
        self.standard_category_table = Some(t.clone());
        t
    }

    /// GNU `BUFFER_LIVE_P` on a buffer id.
    pub fn buffer_live(&self, id: usize) -> bool {
        self.buffers
            .get(id)
            .map(|b| b.borrow().live)
            .unwrap_or(false)
    }

    // ---------- excursions ----------

    /// Snapshot for `save-excursion`: buffer + point saved as a *marker*
    /// (like GNU), so edits before point move the restored position.
    /// `save_mark` additionally records the mark (`save-mark-and-excursion`).
    pub fn save_excursion_state(&mut self, save_mark: bool) -> ExcursionState {
        let buf = self.current_buffer;
        let ma = self.intern_soft("mark-active").unwrap_or(0);
        let (point, mark, mark_active) = self
            .buffers
            .get(buf)
            .map(|r| {
                let bb = r.borrow();
                (
                    bb.point,
                    bb.mark,
                    bb.locals.get(&ma).map(|v| v.truthy()).unwrap_or(false),
                )
            })
            .unwrap_or((0, None, false));
        let mut mk = |pos| {
            let m = Rc::new(RefCell::new(Marker {
                buffer: Some(buf),
                position: pos,
                insertion_type: false,
            }));
            if let Some(b) = self.buffers.get(buf) {
                b.borrow_mut().register_marker(&m);
            }
            m
        };
        ExcursionState {
            buffer: buf,
            point: mk(point),
            mark: if save_mark { mark.map(&mut mk) } else { None },
            mark_active,
        }
    }

    pub fn restore_excursion_state(&mut self, s: ExcursionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            {
                let ma = self.intern_soft("mark-active").unwrap_or(0);
                let mut bb = b.borrow_mut();
                bb.set_point(s.point.borrow().position);
                if let Some(m) = &s.mark {
                    let p = m.borrow().position;
                    bb.mark = Some(p.min(bb.text.len()));
                    bb.locals.insert(
                        ma,
                        if s.mark_active {
                            Value::t()
                        } else {
                            Value::Nil
                        },
                    );
                }
            }
            self.set_current_buffer(s.buffer);
        }
    }

    /// Snapshot for `save-restriction` (narrowing bounds). GNU records the
    /// bounds as markers so edits inside the restriction move them.
    pub fn save_restriction_state(&mut self) -> RestrictionState {
        let buf = self.current_buffer;
        let (begv, zv) = self
            .buffers
            .get(buf)
            .map(|r| {
                let bb = r.borrow();
                (bb.begv, bb.zv)
            })
            .unwrap_or((0, 0));
        let mk = |pos, itype| {
            let m = Rc::new(RefCell::new(Marker {
                buffer: Some(buf),
                position: pos,
                insertion_type: itype,
            }));
            if let Some(b) = self.buffers.get(buf) {
                b.borrow_mut().register_marker(&m);
            }
            m
        };
        // GNU records ZV with insertion_type = t: text inserted exactly at
        // the end of the restriction stays inside it.
        RestrictionState {
            buffer: buf,
            begv: mk(begv, false),
            zv: mk(zv, true),
        }
    }

    pub fn restore_restriction_state(&mut self, s: RestrictionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            let mut bb = b.borrow_mut();
            bb.begv = s.begv.borrow().position.min(bb.text.len());
            bb.zv = s.zv.borrow().position.max(bb.begv).min(bb.text.len());
        }
    }

    // ---------- output / echo ----------

    /// Send printed output to the current destination
    /// (`standard-output`, capture buffer, or the editor's sink).
    pub fn write_output(&mut self, s: &str) -> Result<(), Flow> {
        self.write_output_to(s, &Value::Nil)
    }

    /// Is the print destination STREAM currently at the beginning of a
    /// line?  Buffers/markers peek at the preceding character; the real
    /// output sink uses `out_last_char` (GNU's print_position).
    pub fn output_at_bol(&mut self, stream: &Value) -> bool {
        if self.capture_output {
            return self.output_buffer.ends_with('\n');
        }
        let dest = match stream {
            Value::Nil => self.symbol_value(self.standard_output_sym),
            v => v.clone(),
        };
        match dest {
            Value::Buffer(b) => {
                let bb = b.borrow();
                let p = bb.point();
                p <= bb.begv || bb.text.char_at(p - 1) == '\n'
            }
            Value::Marker(m) => {
                let mm = m.borrow();
                match mm.buffer.and_then(|id| self.buffers.get(id)) {
                    Some(b) => {
                        let bb = b.borrow();
                        mm.position <= bb.begv || bb.text.char_at(mm.position - 1) == '\n'
                    }
                    None => false,
                }
            }
            // Function streams carry no position state — GNU treats
            // them as never at BOL.
            Value::Lambda(_) | Value::Subr(_) => false,
            Value::Sym(sid) if sid != sym::T => false,
            _ => self.out_last_char == Some('\n'),
        }
    }

    /// Send printed output to STREAM (nil → `standard-output', t → the
    /// real output sink, buffer/marker → insert, function → call).
    pub fn write_output_to(&mut self, s: &str, stream: &Value) -> Result<(), Flow> {
        if let Some(c) = s.chars().last() {
            self.out_last_char = Some(c);
        }
        if self.capture_output {
            self.output_buffer.push_str(s);
            return Ok(());
        }
        // `standard-output` may name a buffer, a marker, a function, or t.
        let dest = match stream {
            Value::Nil => self.symbol_value(self.standard_output_sym),
            v => v.clone(),
        };
        match dest {
            Value::Buffer(b) => {
                b.borrow_mut().insert(s);
            }
            Value::Marker(m) => {
                // GNU inserts before the marker, then advances it past
                // the inserted text.
                let mm = m.borrow();
                if let Some(buf_id) = mm.buffer {
                    let pos = mm.position;
                    drop(mm);
                    if let Some(b) = self.buffers.get(buf_id) {
                        b.borrow_mut().insert_at(pos, s);
                    }
                    let newpos = pos + s.chars().count();
                    m.borrow_mut().position = newpos;
                }
            }
            // GNU calls a function print stream once per character.
            Value::Lambda(_) | Value::Subr(_) => {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(ch as i128)])?;
                }
            }
            Value::Cons(ref c) if {
                let cb = c.borrow();
                matches!(&cb.car, Value::Sym(s) if *s == self.intern("lambda") || *s == self.intern("closure"))
            } => {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(ch as i128)])?;
                }
            }
            Value::Sym(sid) if sid != sym::T => {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(ch as i128)])?;
                }
            }
            _ => {
                // nil or t: the real print destination — echo area
                // interactively, stdout in batch.
                match &self.output {
                    Some(OutputSink::Buffer(buf)) => buf.borrow_mut().push_str(s),
                    Some(OutputSink::Function(f)) => {
                        let f = f.clone();
                        let arg = Value::string(s);
                        let _ = self.apply(&f, vec![arg]);
                    }
                    Some(OutputSink::Stdout) => {
                        print!("{}", s);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        self.stderr_need_newline = true;
                    }
                    None => self.echo_message.push_str(s),
                }
            }
        }
        Ok(())
    }

    /// `message` — show a string in the echo area (and log to *Messages*).
    pub fn message(&mut self, s: &str) {
        if self.noninteractive {
            if self.stderr_need_newline {
                eprintln!();
            }
            self.stderr_need_newline = false;
            eprintln!("{}", s);
            return;
        }
        self.echo_message = s.to_string();
        if let Some(mb) = self.buffers.by_name(" *Messages*") {
            if let Some(b) = self.buffers.get(mb) {
                let mut bb = b.borrow_mut();
                let tl = bb.text.len();
                let n = s.chars().count();
                bb.text.insert(tl, s);
                bb.adjust_markers_insert(tl, n, false);
                bb.text.insert(tl + n, "\n");
                bb.adjust_markers_insert(tl + n, 1, false);
            }
        }
        if let Some(OutputSink::Buffer(buf)) = &self.output {
            buf.borrow_mut().push_str(s);
            buf.borrow_mut().push('\n');
        }
    }

    /// Read minibuffer input via the front-end hook. `single` reads one
    /// raw key event instead of a full line.
    pub fn minibuf_input(&mut self, prompt: &str, single: bool) -> Result<MinibufInput, Flow> {
        let reader = self.minibuf_reader.clone();
        match reader {
            Some(r) => r(self, prompt, single),
            None => Err(self.error("minibuffer input unavailable")),
        }
    }

    /// Read a full input line via the front-end hook.
    pub fn minibuf_line(&mut self, prompt: &str) -> Result<String, Flow> {
        self.minibuf_level += 1;
        let r = self.minibuf_input(prompt, false);
        self.minibuf_level -= 1;
        match r? {
            MinibufInput::Text(t) => Ok(t),
            MinibufInput::Key(k) => Ok(char::from_u32(k as u32)
                .map(|c| c.to_string())
                .unwrap_or_default()),
        }
    }

    /// Value of `current-prefix-arg` (nil when unset).
    pub fn prefix_arg(&self) -> Value {
        self.intern_soft("current-prefix-arg")
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil)
    }

    /// `wrong-type-argument` where pred is a value (used by builtins that
    /// pass arbitrary predicates).
    pub fn error_obj(&self, msg: &str, v: &Value) -> Flow {
        self.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "{}: {}",
                msg,
                self.princ_to_string(v)
            ))],
        )
    }

    /// Execute a command by name (M-x dispatch entry point).
    /// The editor's command loop calls this with the command symbol.
    pub fn command_execute(&mut self, cmd: &Value) -> EvalResult {
        // Resolve to function, then call with interactive args.
        let fun = match cmd {
            Value::Sym(id) => self.symbol_function(*id),
            other => other.clone(),
        };
        // Interactive spec: evaluate it to get args (simplified — the
        // editor drives real prompting; batch mode uses defaults).
        if let Some(l) = fun.as_lambda() {
            if let Some(spec) = &l.interactive {
                let argv = self.eval_interactive_spec(spec)?;
                return self.apply(&fun, argv);
            }
        }
        // Subrs: honor the declared interactive spec, if any.
        if let Value::Subr(s) = &fun {
            if let Some(spec) = subr_interactive(s.name) {
                let isym = self.intern("interactive");
                let spec_form = Value::list(vec![Value::Sym(isym), Value::string(spec)]);
                let argv = self.eval_interactive_spec(&spec_form)?;
                return self.apply(&fun, argv);
            }
        }
        self.apply(&fun, vec![])
    }

    /// Evaluate an `(interactive ...)` spec to an argv.
    /// Non-interactive support: `interactive` with no string → no args;
    /// with a string spec we honor the simple codes when input is
    /// pre-supplied in `command_args`.
    pub fn eval_interactive_spec(&mut self, spec_form: &Value) -> Result<Vec<Value>, Flow> {
        let items = spec_form.list_to_vec().unwrap_or_default();
        let spec = items.get(1).cloned().unwrap_or(Value::Nil);
        match spec {
            Value::Nil => Ok(Vec::new()),
            Value::Str(s) => {
                // Parse letter codes. A code's prompt is the text after
                // the code up to the next newline (Emacs spec syntax).
                let chars: Vec<char> = s.borrow().chars().collect();
                let mut out = Vec::new();
                let mut pos = 0usize;
                while pos < chars.len() {
                    let c = chars[pos];
                    pos += 1;
                    match c {
                        // Prefix modifiers and prompt separators produce
                        // no argument.
                        '*' | '^' | '@' | '\n' => {}
                        'r' => {
                            let (p, m) = self
                                .current_buffer_ref()
                                .map(|b| {
                                    let bb = b.borrow();
                                    (bb.point, bb.mark.unwrap_or(bb.point))
                                })
                                .unwrap_or((0, 0));
                            // `r' yields region-beginning/region-end
                            // (ordered) like Emacs.
                            let (beg, end) = if p <= m { (p, m) } else { (m, p) };
                            out.push(Value::Int(beg as i128 + 1));
                            out.push(Value::Int(end as i128 + 1));
                        }
                        'd' => {
                            let p = self
                                .current_buffer_ref()
                                .map(|b| b.borrow().point)
                                .unwrap_or(0);
                            out.push(Value::Int(p as i128 + 1));
                        }
                        'm' => {
                            let m = self
                                .current_buffer_ref()
                                .and_then(|b| b.borrow().mark)
                                .unwrap_or(0);
                            out.push(Value::Int(m as i128 + 1));
                        }
                        'p' | 'P' => {
                            let pa = self.prefix_arg();
                            let numeric = c == 'p';
                            if pa.is_nil() {
                                let fb = self.command_args.first().cloned();
                                out.push(match fb {
                                    Some(v) => v,
                                    None => {
                                        if numeric {
                                            Value::Int(1)
                                        } else {
                                            Value::Nil
                                        }
                                    }
                                });
                            } else if numeric {
                                out.push(prefix_numeric(self, &pa));
                            } else {
                                out.push(pa);
                            }
                            skip_prompt(&chars, &mut pos);
                        }
                        'n' | 'N' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            let pa = self.prefix_arg();
                            if !pa.is_nil() {
                                out.push(prefix_numeric(self, &pa));
                            } else if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                let n = s.trim().parse::<i128>().unwrap_or(0);
                                out.push(Value::Int(n));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        's' | 'B' | 'b' | 'F' | 'f' | 'D' | 'z' | 'Z' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                // `b' defaults to the current buffer on
                                // empty input (Emacs spec semantics).
                                if s.is_empty() && c == 'b' {
                                    let n = self
                                        .current_buffer_ref()
                                        .map(|b| b.borrow().name.clone())
                                        .unwrap_or_default();
                                    out.push(Value::string(n));
                                } else {
                                    out.push(Value::string(s));
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'a' | 'C' | 'S' | 'v' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                out.push(Value::Sym(self.intern(&s)));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'k' | 'K' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                out.push(Value::string(s));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'x' | 'X' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                match self.read_from_string(&s, 0) {
                                    Ok((form, _)) => {
                                        let v = self.eval(&form)?;
                                        if c == 'X' {
                                            // 'X' also prints the result.
                                            let pr = self.prin1_to_string(&v);
                                            self.message(&pr);
                                        }
                                        out.push(v);
                                    }
                                    Err(_) => out.push(Value::Nil),
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'c' | 'e' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                match self.minibuf_input(&prompt, true)? {
                                    MinibufInput::Key(k) => out.push(Value::Int(k)),
                                    MinibufInput::Text(t) => {
                                        let n = t.chars().next().map(|c| c as i128).unwrap_or(0);
                                        out.push(Value::Int(n));
                                    }
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        _ => {
                            // Unknown/prompting code → nil in batch.
                            skip_prompt(&chars, &mut pos);
                            out.push(Value::Nil);
                        }
                    }
                }
                Ok(out)
            }
            _ => {
                // (interactive (list (read-string ...) ...)) — evaluate.
                let v = self.eval(&spec)?;
                Ok(v.list_to_vec().unwrap_or_default())
            }
        }
    }
}

/// Interactive specs for subrs that Emacs declares `interactive'.
/// `commandp`/`command-execute` consult this for primitives.
pub(crate) fn subr_interactive(name: &str) -> Option<&'static str> {
    const T: &[(&str, &str)] = &[
        ("self-insert-command", "p"),
        ("forward-char", "p"),
        ("backward-char", "p"),
        ("delete-char", "p\nP"),
        ("delete-backward-char", "p\nP"),
        ("move-beginning-of-line", "p"),
        ("move-end-of-line", "p"),
        ("forward-word", "p"),
        ("backward-word", "p"),
        ("forward-sexp", "p"),
        ("backward-sexp", "p"),
        ("forward-line", "p"),
        ("newline", "p\nP"),
        ("open-line", "p\nP"),
        ("indent-line-to", "p"),
        ("indent-rigidly", "r\nP"),
        ("transpose-chars", "p"),
        ("kill-line", "P\np"),
        ("kill-region", "r"),
        ("kill-whole-line", "p"),
        ("kill-word", "p"),
        ("backward-kill-word", "p"),
        ("yank", "P"),
        ("yank-pop", "p"),
        ("undo", "p"),
        ("scroll-up-command", "P"),
        ("scroll-down-command", "P"),
        ("scroll-other-window", "p"),
        ("upcase-word", "p"),
        ("downcase-word", "p"),
        ("capitalize-word", "p"),
        ("upcase-region", "r"),
        ("downcase-region", "r"),
        ("zap-to-char", "p\ncZap to char: "),
        ("just-one-space", "p"),
        ("delete-horizontal-space", "p"),
        ("delete-indentation", "p"),
        ("digit-argument", "p"),
        ("negative-argument", "p"),
        ("universal-argument", ""),
        ("abort-recursive-edit", ""),
        ("suspend-emacs", ""),
        ("kill-emacs", "P"),
        ("save-buffer", "p"),
        ("write-file", "FWrite file: "),
        ("find-file", "FFind file: "),
        ("other-window", "p\np"),
        ("delete-window", "p"),
        ("delete-other-windows", "p"),
        ("split-window-below", "P"),
        ("split-window-right", "p"),
        ("narrow-to-region", "r"),
        ("narrow-to-page", "r"),
        ("widen", ""),
        ("beginning-of-defun", "p"),
        ("end-of-defun", "p"),
        ("mark-defun", ""),
        ("narrow-to-defun", ""),
        ("what-cursor-position", "P"),
        ("insert-char", "p\nP"),
        ("erase-buffer", ""),
        ("bury-buffer", "bBury buffer: "),
        ("kill-buffer", "bKill buffer: "),
        ("move-to-window-line", "P"),
        ("recenter", "P"),
        ("count-words-region", ""),
        ("eval-expression", "xEval: "),
        ("execute-extended-command", "P"),
        ("mark-page", "p"),
        ("count-lines-page", "p"),
    ];
    T.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Numeric value of a prefix-arg value: (4)→4, (16)→16, (-)→-1, nil→1.
fn prefix_numeric(i: &Interp, v: &Value) -> Value {
    match v {
        Value::Nil => Value::Int(1),
        Value::Int(n) => Value::Int(*n),
        Value::Cons(c) => match &c.borrow().car {
            Value::Int(n) => Value::Int(*n),
            _ => Value::Int(-1),
        },
        Value::Sym(_) => {
            // A bare `-` prefix means -1 (Emacs `prefix-numeric-value`).
            if i.sym_is(v, i.intern_soft("-").unwrap_or(SymId::MAX)) {
                Value::Int(-1)
            } else {
                Value::Int(1)
            }
        }
        _ => Value::Int(1),
    }
}

/// Consume an interactive-spec prompt (text until `\n` or end).
fn take_prompt(chars: &[char], pos: &mut usize) -> String {
    let mut p = String::new();
    while *pos < chars.len() && chars[*pos] != '\n' {
        p.push(chars[*pos]);
        *pos += 1;
    }
    if *pos < chars.len() {
        *pos += 1;
    }
    p
}

/// Skip an interactive-spec prompt without capturing it.
fn skip_prompt(chars: &[char], pos: &mut usize) {
    let _ = take_prompt(chars, pos);
}

/// Saved point/buffer for `save-excursion` (marker-based, like GNU).
pub struct ExcursionState {
    pub buffer: usize,
    pub point: Rc<RefCell<Marker>>,
    /// Only `Some` for `save-mark-and-excursion`.
    pub mark: Option<Rc<RefCell<Marker>>>,
    pub mark_active: bool,
}

/// Saved narrowing bounds for `save-restriction` (markers like GNU).
pub struct RestrictionState {
    pub buffer: usize,
    pub begv: Rc<RefCell<Marker>>,
    pub zv: Rc<RefCell<Marker>>,
}

/// `match-data` contents after a successful search.
#[derive(Clone)]
pub struct MatchData {
    /// Group start/end pairs (0-based char offsets in the searched text).
    pub regs: Vec<Option<usize>>,
    /// True if the match was against a buffer (positions are 1-based
    /// Emacs positions); false for strings (0-based indices).
    pub in_buffer: bool,
    /// Base offset of the searched text in the buffer (0 for strings).
    pub base: usize,
}

// `Symbol` extension: make-local-if-set flag stored in obarray? We need the
// field on Symbol; it's added in obarray.rs.
pub fn plist_get(plist: &Value, prop: SymId) -> Value {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        // Value is cadr.
                        if let Value::Cons(c2) = &next {
                            return c2.borrow().car.clone();
                        }
                        return Value::Nil;
                    }
                }
                // advance two
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return Value::Nil,
                }
                n += 1;
                if n > 10000 {
                    return Value::Nil;
                }
            }
            _ => return Value::Nil,
        }
    }
}

/// Presence-aware variant of `plist_get': `Some(v)' iff the key
/// exists, even when its value is nil (GNU `lookup_char_property'
/// returns the slot value directly when found).
pub fn plist_lookup(plist: &Value, prop: SymId) -> Option<Value> {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        if let Value::Cons(c2) = &next {
                            return Some(c2.borrow().car.clone());
                        }
                        return Some(Value::Nil);
                    }
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return None,
                }
                n += 1;
                if n > 10000 {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

/// `put` on a plist value — returns a new plist with prop set.
/// Mutates in place when possible (Emacs mutates the plist).
pub fn plist_put(plist: &Value, prop: SymId, val: Value) -> Value {
    let mut cur = plist.clone();
    // Tracks the last cons cell reached while walking pairs — the
    // value cell of the final pair — so new props append at the end.
    let mut last_pair_end: Option<super::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        if let Value::Cons(c2) = &next {
                            c2.borrow_mut().car = val;
                            return plist.clone();
                        }
                        return plist.clone();
                    }
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        last_pair_end = Some(c2);
                        cur = n2;
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    // Not found: append (prop val) at the END of the plist,
    // matching Emacs's put/plist-put ordering.
    match last_pair_end {
        Some(cell) => {
            cell.borrow_mut().cdr = Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil));
            plist.clone()
        }
        None => Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil)),
    }
}

impl Value {
    pub fn as_lambda(&self) -> Option<&Rc<Lambda>> {
        match self {
            Value::Lambda(l) => Some(l),
            _ => None,
        }
    }
}
