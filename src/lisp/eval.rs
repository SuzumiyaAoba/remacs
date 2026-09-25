//! The evaluator: `Interp` state, `eval`, `apply`, and special forms.
//!
//! Scoping: elisp is dynamically scoped by default. Dynamic `let` is
//! implemented exactly like Emacs's specbind: save the symbol's current
//! value cell, overwrite, restore on unwind — so lookup stays O(1).
//! When `lexical-binding` is on, `let` binds in a captured lexical env
//! chain instead (unless the symbol is `defvar`'d special).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    /// Scoped `defvar' declarations — GNU's bare-symbol elements of
    /// `internal-interpreter-environment': a name listed here is bound
    /// dynamically by `let'/`let*' for the rest of this scope's extent,
    /// then the declaration unwinds with the frame.
    pub declared: RefCell<HashSet<SymId>>,
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

/// GNU's `Fmemq(var, Vinternal_interpreter_environment)': true when SYM
/// is declared dynamically scoped by a bare `defvar' anywhere in ENV.
/// Scoped declarations only affect binding decisions — lookups go
/// through `lexenv_lookup'/the dynamic cell like GNU's `assq'.
pub fn lexenv_declared(mut env: &LexEnv, sym: SymId) -> bool {
    while let Some(frame) = env {
        if frame.declared.borrow().contains(&sym) {
            return true;
        }
        env = &frame.parent;
    }
    false
}

/// Record a scoped `defvar' declaration in the innermost frame of ENV
/// (GNU pushes the bare symbol onto `internal-interpreter-environment').
pub fn lexenv_declare(env: &LexEnv, sym: SymId) {
    if let Some(frame) = env {
        frame.declared.borrow_mut().insert(sym);
    }
}

/// A fresh empty lexical root — GNU's `(t)' sentinel environment.
pub fn lexenv_root() -> LexEnv {
    Some(Rc::new(LexFrame {
        vars: RefCell::new(HashMap::new()),
        declared: RefCell::new(HashSet::new()),
        parent: None,
    }))
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
    /// The `obarray' variable's record value; its Rc identity marks the
    /// default symbol table, so `intern'/`mapatoms'/completion over
    /// `obarray' route to `self.obarray' instead of a (stub) record vec.
    pub default_obarray: Option<crate::lisp::value::VecRef>,
    /// True while the startup prelude or an embedded (`builtin:') library
    /// is being evaluated — lambdas defined then get `dumped_doc' (their
    /// docstrings behave like GNU's .elc/DOC-file entries).
    pub loading_dumped: bool,
    /// Number of currently-active dumped definitions.  GNU byte-compiles
    /// its dump, so macros called there need not remain visible at -Q;
    /// remacs stashes those cells and resolves hidden macros only while
    /// a dumped lambda is running.
    pub dumped_call_depth: usize,
    /// Number of currently-active macro expanders.  Hidden dump-time
    /// helpers remain callable only in this expansion-time context; a
    /// dumped function's ordinary runtime calls still see GNU's void
    /// function cells.
    pub macroexp_call_depth: usize,
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
    /// Buffered stdin for noninteractive minibuffer reads (GNU
    /// `read_minibuf_noninteractive'); buffered so line and char reads
    /// share the stream the way `getchar' does.
    pub batch_stdin: Option<std::io::BufReader<std::io::Stdin>>,
    /// Nesting depth of active minibuffer reads (`minibuffer-depth').
    pub minibuf_level: i32,
    /// ` *Minibuf-N*' buffer ids by depth — GNU's `Vminibuffer_list'.
    /// Index 0 (` *Minibuf-0*') is the never-active null minibuffer;
    /// a `usize::MAX` slot is a not-yet-created buffer.
    pub minibuf_list: Vec<usize>,
    /// Prompt strings of in-progress minibuffer reads, innermost
    /// last — GNU's `minibuf_prompt' (saved per level in
    /// `minibuf_save_list').
    pub minibuf_prompts: Vec<String>,
    /// Non-nil while `minibuf_read' has an `exit' catch installed
    /// (GNU's `internal_catch (Qexit, ...)') — the front-end command
    /// loop turns `exit-minibuffer''s throw into this flag instead of
    /// reporting an uncaught throw.
    pub minibuf_catching_exit: bool,
    /// Set by the front-end loop when `exit-minibuffer' fired during
    /// the active read.
    pub minibuf_exited: bool,
    /// A quit/throw flow raised by a command inside the minibuffer
    /// read — handed to the front-end loop so it can abort
    /// `minibuf_input' with it (io::Result can't carry Flow).
    pub minibuf_pending_flow: Option<crate::lisp::error::Flow>,
    /// A function thrown through the `exit' catch — the front-end
    /// loop returns it as `MinibufInput::Call' so `minibuf_read' can
    /// call it after the read unwinds (GNU `recursive_edit_1').
    pub minibuf_exit_fn: Option<Value>,
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
    /// entries; the oldest is innermost, like GNU's oclosure chain.
    pub advices: Vec<(SymId, Vec<(SymId, Value, Value)>)>,
    /// The unadvised function-cell value captured when SYM first gains
    /// advice — `fset' clears it, `defalias' substitutes a new one.
    pub advice_bases: Vec<(SymId, Value)>,
    /// Generated trampolines for advice composition: index →
    /// (WHERE, ADVICE-FUN, NEXT-CALLABLE, NAME) — NAME feeds the
    /// `advice--props' alist `(name . N)'.
    pub advice_links: Vec<(Value, Value, Value, Value)>,
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
    /// `lisp_stack' snapshot taken when the last signal was raised —
    /// the frames GNU's batch debugger prints after `Error:' (the live
    /// stack itself has already unwound by the time main sees it).
    pub last_error_stack: std::cell::RefCell<Vec<(Value, Vec<Value>)>>,
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
    /// A function thrown through the `exit' catch (GNU calls it
    /// after `recursive_edit_1' unwinds — `minibuffer-quit-recursive-
    /// edit' uses this to signal `minibuffer-quit' post-cleanup).
    Call(Value),
}

/// Per-read arguments mirroring GNU `read_minibuf' (minibuf.c).
pub struct MinibufArgs {
    /// `read-from-minibuffer' HISTORY: nil/`t'/symbol/(HISTVAR . HISTPOS).
    pub hist: Value,
    /// DEFAULT-VALUE → specbound `minibuffer-default' and the
    /// empty-input `histstring'.
    pub defalt: Value,
    /// INITIAL-CONTENTS: nil, string, or (STRING . POS).
    pub initial: Value,
    /// KEYMAP override; nil substitutes `minibuffer-local-map'.
    pub keymap: Value,
}

impl Default for MinibufArgs {
    fn default() -> Self {
        MinibufArgs {
            hist: Value::Nil,
            defalt: Value::Nil,
            initial: Value::Nil,
            keymap: Value::Nil,
        }
    }
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
                        if let Some(hit) = doc_dir_under(&share.join("emacs")) {
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
            "japan-util",
            "rmc",
            "iso-transl",
            "tooltip",
            "cconv",
            "eldoc",
            "paren",
            "electric",
            "uniquify",
            "ediff-hook",
            "vc-hooks",
            "lisp-float-type",
            "elisp-mode",
            "mwheel",
            "term/ns-win",
            "ns-win",
            "ucs-normalize",
            "mule-util",
            "term/common-win",
            "tool-bar",
            "dnd",
            "fontset",
            "image",
            "regexp-opt",
            "fringe",
            "tabulated-list",
            "replace",
            "newcomment",
            "text-mode",
            "lisp-mode",
            "prog-mode",
            "register",
            "page",
            "tab-bar",
            "menu-bar",
            "rfn-eshadow",
            "isearch",
            "easymenu",
            "timer",
            "select",
            "scroll-bar",
            "mouse",
            "jit-lock",
            "font-lock",
            "syntax",
            "font-core",
            "term/tty-colors",
            "frame",
            "minibuffer",
            "nadvice",
            "seq",
            "simple",
            "cl-generic",
            "indonesian",
            "philippine",
            "cham",
            "georgian",
            "utf-8-lang",
            "misc-lang",
            "vietnamese",
            "tibetan",
            "thai",
            "tai-viet",
            "lao",
            "korean",
            "japanese",
            "eucjp-ms",
            "cp51932",
            "hebrew",
            "greek",
            "romanian",
            "slovak",
            "czech",
            "european",
            "ethiopic",
            "indian",
            "cyrillic",
            "chinese",
            "composite",
            "emoji-zwj",
            "charscript",
            "charprop",
            "case-table",
            "epa-hook",
            "jka-cmpr-hook",
            "help",
            "abbrev",
            "obarray",
            "oclosure",
            "cl-preloaded",
            "button",
            "loaddefs",
            "theme-loaddefs",
            "faces",
            "cus-face",
            "macroexp",
            "files",
            "window",
            "text-properties",
            "overlay",
            "sha1",
            "md5",
            "base64",
            "format",
            "env",
            "code-pages",
            "mule",
            "custom",
            "widget",
            "keymap",
            "hashtable-print-readable",
            "backquote",
            "threads",
            "kqueue",
            "cocoa",
            "ns",
            "multi-tty",
            "make-network-process",
            "tty-child-frames",
            "native-compile",
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
            default_obarray: None,
            loading_dumped: false,
            dumped_call_depth: 0,
            macroexp_call_depth: 0,
            stderr_need_newline: false,
            out_last_char: None,
            frames: Vec::new(),
            selected_frame: None,
            quit_editor: false,
            minibuf_reader: None,
            batch_stdin: None,
            minibuf_level: 0,
            minibuf_list: Vec::new(),
            minibuf_prompts: Vec::new(),
            minibuf_catching_exit: false,
            minibuf_exited: false,
            minibuf_pending_flow: None,
            minibuf_exit_fn: None,
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
            advice_bases: Vec::new(),
            advice_links: Vec::new(),
            char_table_parents: Vec::new(),
            char_table_defalts: Vec::new(),
            frame_state_seen: None,
            lisp_stack: Vec::new(),
            last_error_stack: std::cell::RefCell::new(Vec::new()),
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
        // GNU does not create `*Warnings*' at startup; `display-warning'
        // makes it lazily (with messages-buffer-mode) on first warning.
        // GNU seeds the startup buffers' buffer-local major modes; its
        // `*Messages*' is read-only and carries a stale modified flag.
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
        if let Some(b) = interp.buffers.get(messages) {
            let mut br = b.borrow_mut();
            let ro = interp.intern("buffer-read-only");
            br.locals.insert(ro, Value::t());
            br.note_modified(true);
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
        interp.loading_dumped = true;
        if std::env::var("REMACS_NO_PRELUDE").is_ok() {
            // Debug escape: skip prelude evaluation entirely.
        } else if std::env::var("PRELUDE_TRACE").is_ok() || prelude_max != usize::MAX {
            interp.lexenv = lexenv_root();
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
            // The prelude is a concatenation of GNU's lexical-binding
            // sources (subr.el, minibuffer.el, ...); `lexical-binding'
            // defaults to t, so `eval_str' installs a `(t)' root env
            // and it evals lexically like GNU's dump does.
            let _ = interp.eval_str(crate::lisp::prelude::PRELUDE);
            // env.el is preloaded into GNU's dump (loadup.el): its
            // feature is already registered, but the definitions must
            // exist too — `(require 'env)' short-circuits on the
            // feature mark, so evaluate the embedded source now.
            let _ = crate::lisp::load::load_library(&mut interp, "env");
            // jka-cmpr-hook.el is in GNU's dump too (loadup.el): the
            // feature mark would make `require' skip the definitions —
            // evaluate it so `jka-compr-installed-p' & co. exist and
            // `auto-compression-mode' installs its file-name handler.
            let _ = crate::lisp::load::load_library(&mut interp, "jka-cmpr-hook");
            // tabulated-list.el is likewise in GNU's dump (loadup.el):
            // same reasoning as env — the feature mark alone would
            // make `require' skip the definitions.
            let _ = crate::lisp::load::load_library(&mut interp, "tabulated-list");
            // subr-x.el is also in GNU's dump: it holds the accumulated
            // GNU-verbatim compatibility definitions (string trim/pad,
            // fringe helpers, paren/select/vc/dnd support, …).
            let _ = crate::lisp::load::load_library(&mut interp, "subr-x");
            // GNU's subr.el defvar (mostly populated by loaddefs.el
            // autoload cookies); compat.el's cookie pushes
            // `(compat MAJOR MINOR 9999)'.
            let _ = interp.eval_str(
                "(unless (boundp 'package--builtin-versions) \
                   (defvar package--builtin-versions \
                     (list (list 'emacs emacs-major-version emacs-minor-version)) \
                     \"Alist giving the version of each versioned builtin package.\")) \
                 (push (list 'compat emacs-major-version emacs-minor-version 9999) \
                       package--builtin-versions)",
            );
            // tool-bar.el is in GNU's dump on window-system builds
            // (loadup.el): same reasoning — the feature mark alone
            // would make `require' skip the definitions.
            let _ = crate::lisp::load::load_library(&mut interp, "tool-bar");
            // widget.el is dumped too (loadup.el loads "widget" for
            // `define-widget' & co. used by cus-edit and friends).
            let _ = crate::lisp::load::load_library(&mut interp, "widget");
            // version.el is loaded by loadup.el without a `provide':
            // evaluating it defines `emacs-repository-*' helpers while
            // `(require 'version)' still fails exactly like GNU.
            let _ = crate::lisp::load::load_library(&mut interp, "version");
            // rx.el is likewise loaded by loadup.el: GNU has the `rx'
            // macro bound at -Q but leaves `rx' unprovided
            // (`(featurep 'rx)' is nil there too), so libraries whose
            // top-level forms use `rx' can expand at load.
            let _ = crate::lisp::load::load_library(&mut interp, "rx");
            // vc-hooks.el is in GNU's dump (loadup.el): its defcustoms
            // (`vc-handled-backends', `vc-ignore-dir-regexp', ...) and
            // the vc-file-* property machinery are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "vc-hooks");
            // uniquify.el is dumped too; GNU's loadup order runs it
            // after vc-hooks, so `uniquify-kill-buffer-function'
            // prepends ahead of `vc-kill-buffer-hook'.
            let _ = crate::lisp::load::load_library(&mut interp, "uniquify");
            // tooltip.el is dumped on window-system builds too: GNU has
            // `tooltip-delay', `tooltip-mode' & co. bound at -Q, and the
            // `tooltip' feature mark alone would make `require' skip
            // the definitions.
            let _ = crate::lisp::load::load_library(&mut interp, "tooltip");
            // float-sup.el is dumped too (`lisp-float-type' feature):
            // `float-pi', `degrees-to-radians' & co. are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "float-sup");
            // map-ynp.el is in GNU's dump (loadup.el): `map-y-or-n-p'
            // is bound at -Q, used by files.el's `save-some-buffers'.
            let _ = crate::lisp::load::load_library(&mut interp, "map-ynp");
            // debug-early.el is in GNU's dump too (loadup.el): it has
            // no `provide', so the feature stays nil while
            // `debug-early'/`debug-early-backtrace' are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "debug-early");
            // loaddefs.el is loaded by loadup.el right after subr.el:
            // it installs the (autoload ...) cells for every preloaded
            // library's entry points and provides the `loaddefs'
            // feature, both visible at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "loaddefs");
            // tab-bar.el is in GNU's dump (loadup.el): tab-bar-mode,
            // tab-new, tab-switcher & co. are bound at -Q.  It needs
            // loaddefs's `frameset-filter-alist' defvar, hence the order.
            let _ = crate::lisp::load::load_library(&mut interp, "tab-bar");
            // image.el is in GNU's dump too (loadup.el): image-mode,
            // `image-load-path', `insert-image' & co. are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "image");
            // buff-menu.el is in GNU's dump as well: `Buffer-menu-mode'
            // and friends are bound at -Q while the `buff-menu' feature
            // stays nil (the file has no `provide').
            let _ = crate::lisp::load::load_library(&mut interp, "buff-menu");
            // electric.el is in GNU's dump (loadup.el):
            // `electric-indent-mode'/`electric-quote-mode' are bound
            // at -Q and elec-pair.el needs `electric-quote-chars'.
            let _ = crate::lisp::load::load_library(&mut interp, "electric");
            // format.el and composite.el are in GNU's dump (loadup.el):
            // `format-alist', `format-decode' & co. and the
            // composition machinery are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "format");
            let _ = crate::lisp::load::load_library(&mut interp, "composite");
            // iso-transl.el, mule-util.el and epa-hook.el are also in
            // GNU's dump (loadup.el): `iso-transl-set-language' & co.
            // are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "iso-transl");
            let _ = crate::lisp::load::load_library(&mut interp, "mule-util");
            let _ = crate::lisp::load::load_library(&mut interp, "epa-hook");
            // paren.el is in GNU's dump (loadup.el): `show-paren-mode'
            // and the `paren' feature are bound at -Q.
            let _ = crate::lisp::load::load_library(&mut interp, "paren");
            // rmc.el is in GNU's dump too (loadup.el loads it early so
            // `read-multiple-choice' is available during startup).
            let _ = crate::lisp::load::load_library(&mut interp, "rmc");
            // Thirteen more GNU-dumped libraries (loadup.el): their
            // features and real definitions exist at -Q.
            for lib in [
                "abbrev",
                "cconv",
                "cus-face",
                "ediff-hook",
                "eldoc",
                "mouse",
                "prog-mode",
                "regexp-opt",
                "register",
                "replace",
                "scroll-bar",
                "text-mode",
                "timer",
                // GNU's dump also has newcomment.el, image.el and
                // tab-bar.el (loadup.el).  json.el is a plain library.
                "newcomment",
                "image",
                "tab-bar",
            ] {
                let _ = crate::lisp::load::load_library(&mut interp, lib);
            }
            // Loading cconv.el interpretively expands its
            // `define-inline' call, which pulls in inline.el — GNU's
            // dump had it compiled away, so -Q keeps `define-inline'
            // as a loaddefs autoload cell and leaves the rest of
            // inline.el unbound.  Restore that state.
            let _ = interp.eval_str(
                "(progn \
                   (put 'define-inline 'remacs--dump-fn \
                        (symbol-function 'define-inline)) \
                   (dolist (s '(inline-quote inline-const-p inline-const-val \
                                inline-error inline--leteval inline--letlisteval \
                                inline-letevals inline--do-quote inline--dont-quote \
                                inline--do-leteval inline--dont-leteval \
                                inline--do-letlisteval inline--dont-letlisteval \
                                inline--testconst-p inline--alwaysconst-p \
                                inline--getconst-val inline--alwaysconst-val \
                                inline--error inline--warning)) \
                     (put s 'remacs--dump-fn (symbol-function s)) \
                     (fmakunbound s)) \
                   (fset 'define-inline \
                         '(autoload \"inline\" \
                           \"Define an inline function NAME with arguments ARGS and body in BODY.\\nThis is halfway between `defmacro' and `defun'.  BODY is used as a blueprint\\nboth for the body of the function and for the body of the compiler-macro\\nused to generate the code inlined at each call site.\\nSee Info node `(elisp)Inline Functions' for more details.\\n\\nA (noinline t) in the `declare' form prevents the definition of the\\ncompiler macro.  This is for the rare case in which you want to use this\\nmacro to define a function that should not be inlined.\\n\\n(fn NAME ARGS &rest BODY)\" \
                           nil t)))",
            );
            // GNU -Q leaves `define-derived-mode'/`define-generic-mode'
            // as loaddefs autoload cells: GNU's dumped mode definitions
            // were byte-compiled, so the macros expanded at build time
            // and the cells stay autoloads at startup.  Our prelude's
            // local subset `define-derived-mode' served the same early
            // mode definitions (and the dumped libraries' calls), so
            // restore the autoload cells only now — `autoloadp',
            // `featurep', and lazy loading of derived.el/generic.el
            // then match GNU.
            let _ = interp.eval_str(
                "(progn \
                   (put 'define-derived-mode 'remacs--dump-fn \
                        (symbol-function 'define-derived-mode)) \
                   (fset 'define-derived-mode \
                         '(autoload \"derived\" \
                           \"Create a new mode CHILD which is a variant of an existing mode PARENT.\n\n\\(fn CHILD PARENT NAME [DOCSTRING] [KEYWORD-ARGS...] &rest BODY)\" \
                           nil t)) \
                   (fset 'define-generic-mode \
                         '(autoload \"generic\" \
                           \"Create a new generic mode MODE.\n\n\\(fn MODE COMMENT-LIST KEYWORD-LIST FONT-LOCK-LIST AUTO-MODE-LIST\n     FUNCTION-LIST &optional DOCSTRING)\" \
                           nil t)) \
                   ;; Same for entry points the prelude stubs for early
                   ;; use: GNU keeps them as loaddefs autoload cells at -Q.
                   (fset 'electric-pair-mode \
                         '(autoload \"elec-pair\" \
                           \"Toggle automatic pairing of delimiters (Electric Pair mode).\" \
                           t nil)) \
                   (fset 'dcl-mode \
                         '(autoload \"dcl-mode\" \
                           \"Major mode for editing DCL-files.\" \
                           t nil)) \
                   (fset 'find-function \
                         '(autoload \"find-func\" \
                           \"Find the definition of the Emacs Lisp FUNCTION near point.\" \
                           t nil)) \
                   ;; Mode stubs the prelude generated for
                   ;; `auto-mode-alist' entries: GNU keeps these as
                   ;; loaddefs autoload cells at -Q.
                   (fset 'dns-mode '(autoload \"dns-mode\" \"Major mode for DNS master files.\" t nil)) \
                   (fset 'icon-mode '(autoload \"icon\" \"Major mode for editing Icon code.\" t nil)) \
                   (fset 'mixal-mode '(autoload \"mixal-mode\" \"Major mode for the mixasm code.\" t nil)) \
                   (fset 'opascal-mode '(autoload \"opascal\" \"Major mode for editing OPascal code.\" t nil)) \
                   (fset 'pascal-mode '(autoload \"pascal\" \"Major mode for editing Pascal code.\" t nil)) \
                   (fset 'sieve-mode '(autoload \"sieve-mode\" \"Major mode for Sieve scripts.\" t nil)) \
                   (fset 'simula-mode '(autoload \"simula\" \"Major mode for Simula.\" t nil)) \
                   (fset 'metafont-mode '(autoload \"meta-mode\" \"Major mode for editing Metafont sources.\" t nil)) \
                   (fset 'metapost-mode '(autoload \"meta-mode\" \"Major mode for editing MetaPost sources.\" t nil)) \
                   ;; More prelude mode stubs whose real libraries are
                   ;; now embedded: restore GNU's autoload cells.
                   (fset 'perl-mode '(autoload \"perl-mode\" \"Major mode for editing Perl code.\" t nil)) \
                   (fset 'scheme-mode '(autoload \"scheme\" \"Major mode for editing Scheme code.\" t nil)) \
                   (fset 'nroff-mode '(autoload \"nroff-mode\" \"Major mode for editing nroff text.\" t nil)) \
                   (fset 'mail-mode '(autoload \"sendmail\" \"Major mode for editing mail.\" t nil)) \
                   (fset 'makefile-mode '(autoload \"make-mode\" \"Major mode for editing Makefiles.\" t nil)) \
                   (fset 'makefile-automake-mode '(autoload \"make-mode\" \"Major mode for Automake Makefiles.\" t nil)) \
                   (fset 'makefile-gmake-mode '(autoload \"make-mode\" \"Major mode for GNU makefiles.\" t nil)) \
                   (fset 'makefile-makepp-mode '(autoload \"make-mode\" \"Major mode for Makeppfiles.\" t nil)) \
                   (fset 'makefile-bsdmake-mode '(autoload \"make-mode\" \"Major mode for BSD makefiles.\" t nil)) \
                   (fset 'makefile-imake-mode '(autoload \"make-mode\" \"Major mode for Imakefiles.\" t nil)) \
                   (fset 'change-log-mode '(autoload \"add-log\" \"Major mode for editing change logs.\" t nil))) \
                   (fset '2C-associate-buffer '(autoload \"two-column\" \"Associate another BUFFER with this one in two-column minor mode.\\nCan also be used to associate a just previously visited file, by\\naccepting the proposed default buffer.\\n\\n(See  \\\\[describe-mode] .)\\n\\n(fn BUFFER)\" t nil)) \
                   (fset '2C-command '(autoload \"two-column\" nil t keymap)) \
                   (fset '2C-split '(autoload \"two-column\" \"Split a two-column text at point, into two buffers in two-column minor mode.\\nPoint becomes the local value of `2C-window-width'.  Only lines that\\nhave the ARG same preceding characters at that column get split.  The\\nARG preceding characters without any leading whitespace become the local\\nvalue for `2C-separator'.  This way lines that continue across both\\ncolumns remain untouched in the first buffer.\\n\\nThis function can be used with a prototype line, to set up things.  You\\nwrite the first line of each column and then split that line.  E.g.:\\n\\nFirst column's text    sSs  Second column's text\\n		       \\\\___/\\\\\\n			/    \\\\\\n   5 character Separator      You type  M-5 \\\\[2C-split]  with the point here.\\n\\n(See  \\\\[describe-mode] .)\\n\\n(fn ARG)\" t nil)) \
                   (fset '2C-two-columns '(autoload \"two-column\" \"Split current window vertically for two-column editing.\\n\\\\<global-map>When called the first time, associates a buffer with the current\\nbuffer in two-column minor mode (use \\\\[describe-mode] once in the mode,\\nfor details.).  It runs `2C-other-buffer-hook' in the new buffer.\\nWhen called again, restores the screen layout with the current buffer\\nfirst and the associated buffer to its right.\\n\\n(fn &optional BUFFER)\" t nil)) \
                   (fset 'archive-mode '(autoload \"arc-mode\" \"Major mode for viewing an archive file in a dired-like way.\\nYou can move around using the usual cursor motion commands.\\nLetters no longer insert themselves.\\\\<archive-mode-map>\\nType \\\\[archive-extract] to pull a file out of the archive and into its own buffer;\\nor click mouse-2 on the file's line in the archive mode buffer.\\n\\nIf you edit a sub-file of this archive (as with the \\\\[archive-extract] command) and\\nsave it, the contents of that buffer will be saved back into the\\narchive.\\n\\n\\\\{archive-mode-map}\\n\\n(fn &optional FORCE)\" nil nil)) \
                   (fset 'diff-latest-backup-file '(autoload \"diff\" \"Return the latest existing backup of file FN, or nil.\\n\\n(fn FN)\" nil nil)) \
                   (fset 'dsssl-mode '(autoload \"scheme\" \"Major mode for editing DSSSL code.\\nEditing commands are similar to those of `lisp-mode'.\\n\\nCommands:\\nDelete converts tabs to spaces as it moves back.\\nBlank lines separate paragraphs.  Semicolons start comments.\\n\\\\{scheme-mode-map}\\nEntering this mode runs the hooks `scheme-mode-hook' and then\\n`dsssl-mode-hook' and inserts the value of `dsssl-sgml-declaration' if\\nthat variable's value is a string.\" t nil)) \
                   (fset 'help-buffer '(autoload \"help-mode\" nil nil nil)) \
                   (fset 'help-insert-xref-button '(autoload \"help-mode\" \"Insert STRING and make a hyperlink from cross-reference text on it.\\nTYPE is the type of button to use.  Any remaining arguments are passed\\nto the button's help-function when it is invoked.\\nSee `help-make-xrefs'.\\n\\n(fn STRING TYPE &rest ARGS)\" nil nil)) \
                   (fset 'help-make-xrefs '(autoload \"help-mode\" \"Parse and hyperlink documentation cross-references in the given BUFFER.\\n\\nFind cross-reference information in a buffer and activate such cross\\nreferences for selection with `help-follow-symbol'.  Cross-references have\\nthe canonical form `...'  and the type of reference may be\\ndisambiguated by the preceding word(s) used in\\n`help-xref-symbol-regexp'.  Faces only get cross-referenced if\\npreceded or followed by the word `face'.  Variables without\\nvariable documentation do not get cross-referenced, unless\\npreceded by the word `variable' or `option'.\\n\\nIf the variable `help-xref-mule-regexp' is non-nil, find also\\ncross-reference information related to multilingual environment\\n(e.g., coding-systems).  This variable is also used to disambiguate\\nthe type of reference as the same way as `help-xref-symbol-regexp'.\\n\\nA special reference `back' is made to return back through a stack of\\nhelp buffers.  Variable `help-back-label' specifies the text for\\nthat.\\n\\n(fn &optional BUFFER)\" t nil)) \
                   (fset 'help-mode '(autoload \"help-mode\" \"Major mode for viewing help text and navigating references in it.\\nAlso see the `help-enable-variable-value-editing' variable.\\n\\nCommands:\\n\\\\{help-mode-map}\\n\\nIn addition to any hooks its parent mode `special-mode' might have\\nrun, this mode runs the hook `help-mode-hook', as the final or\\npenultimate step during initialization.\" t nil)) \
                   (fset 'help-mode--add-function-link '(autoload \"help-mode\" \"\\n\\n(fn STR FUN)\" nil nil)) \
                   (fset 'help-mode-finish '(autoload \"help-mode\" \"Finalize Help mode setup in current buffer.\" nil nil)) \
                   (fset 'help-mode-setup '(autoload \"help-mode\" \"Enter Help mode in the current buffer.\" nil nil)) \
                   (fset 'help-setup-xref '(autoload \"help-mode\" \"Invoked from commands using the \\\"*Help*\\\" buffer to install some xref info.\\n\\nITEM is a (FUNCTION . ARGS) pair appropriate for recreating the help\\nbuffer after following a reference.  INTERACTIVE-P is non-nil if the\\ncalling command was invoked interactively.  In this case the stack of\\nitems for help buffer \\\"back\\\" buttons is cleared.\\n\\nThis function also re-enables the major mode of the buffer, thus\\nresetting local variables to the values set by the mode and running the\\nmode hooks.\\n\\nSo this should be called very early, before the output buffer is\\ncleared, also because we want to record the \\\"previous\\\" position of\\npoint so we can restore it properly when going back.\\n\\n(fn ITEM INTERACTIVE-P)\" nil nil)) \
                   (fset 'help-xref-button '(autoload \"help-mode\" \"Make a hyperlink for cross-reference text previously matched.\\nMATCH-NUMBER is the subexpression of interest in the last matched\\nregexp.  TYPE is the type of button to use.  Any remaining arguments are\\npassed to the button's help-function when it is invoked.\\nSee `help-make-xrefs'.\\n\\nThis function removes quotes surrounding the match if the\\nvariable `help-clean-buttons' is non-nil.\\n\\n(fn MATCH-NUMBER TYPE &rest ARGS)\" nil nil)) \
                   (fset 'help-xref-on-pp '(autoload \"help-mode\" \"Add xrefs for symbols in `pp's output between FROM and TO.\\n\\n(fn FROM TO)\" nil nil)) \
                   (fset 'pixel-scroll-mode '(autoload \"pixel-scroll\" \"A minor mode to scroll text pixel-by-pixel.\\n\\nThis is a global minor mode.  If called interactively, toggle the\\n`Pixel-Scroll mode' mode.  If the prefix argument is positive, enable\\nthe mode, and if it is zero or negative, disable the mode.\\n\\nIf called from Lisp, toggle the mode if ARG is `toggle'.  Enable the\\nmode if ARG is nil, omitted, or is a positive number.  Disable the mode\\nif ARG is a negative number.\\n\\nTo check whether the minor mode is enabled in the current buffer,\\nevaluate `(default-value \\\\='pixel-scroll-mode)'.\\n\\nThe mode's hook is called both when the mode is enabled and when it is\\ndisabled.\\n\\n(fn &optional ARG)\" t nil)) \
                   (fset 'pixel-scroll-precision-mode '(autoload \"pixel-scroll\" \"Toggle pixel scrolling.\\n\\nWhen enabled, this minor mode allows you to scroll the display\\nprecisely, according to the turning of the mouse wheel.\\n\\nThis is a global minor mode.  If called interactively, toggle the\\n`Pixel-Scroll-Precision mode' mode.  If the prefix argument is positive,\\nenable the mode, and if it is zero or negative, disable the mode.\\n\\nIf called from Lisp, toggle the mode if ARG is `toggle'.  Enable the\\nmode if ARG is nil, omitted, or is a positive number.  Disable the mode\\nif ARG is a negative number.\\n\\nTo check whether the minor mode is enabled in the current buffer,\\nevaluate `(default-value \\\\='pixel-scroll-precision-mode)'.\\n\\nThe mode's hook is called both when the mode is enabled and when it is\\ndisabled.\\n\\n(fn &optional ARG)\" t nil)) \
                   (fset 'snmp-mode '(autoload \"snmp-mode\" \"Major mode for editing SNMP MIBs.\\nExpression and list commands understand all C brackets.\\nTab indents for C code.\\nComments start with -- and end with newline or another --.\\nDelete converts tabs to spaces as it moves back.\\n\\\\{snmp-mode-map}\\nTurning on `snmp-mode' runs the hooks in `snmp-common-mode-hook', then\\n`snmp-mode-hook'.\" t nil)) \
                   (fset 'snmpv2-mode '(autoload \"snmp-mode\" \"Major mode for editing SNMPv2 MIBs.\\nExpression and list commands understand all C brackets.\\nTab indents for C code.\\nComments start with -- and end with newline or another --.\\nDelete converts tabs to spaces as it moves back.\\n\\\\{snmp-mode-map}\\nTurning on `snmp-mode' runs the hooks in `snmp-common-mode-hook',\\nthen `snmpv2-mode-hook'.\" t nil)) \
                   (fset 'delete-duplicate-lines '(autoload \"sort\" \"Delete all but one copy of any identical lines in the region.\\nNon-interactively, arguments BEG and END delimit the region.\\nNormally it searches forwards, keeping the first instance of\\neach identical line.  If REVERSE is non-nil (interactively, with\\na \\\\[universal-argument] prefix), it searches backwards and keeps the last instance of\\neach repeated line.\\n\\nIdentical lines need not be adjacent, unless the argument\\nADJACENT is non-nil (interactively, with a \\\\[universal-argument] \\\\[universal-argument] prefix).\\nThis is a more efficient mode of operation, and may be useful\\non large regions that have already been sorted.\\n\\nIf the argument KEEP-BLANKS is non-nil (interactively, with a\\n\\\\[universal-argument] \\\\[universal-argument] \\\\[universal-argument] prefix), it retains repeated blank lines.\\n\\nReturns the number of deleted lines.  Interactively, or if INTERACTIVE\\nis non-nil, it also prints a message describing the number of deletions.\\n\\n(fn BEG END &optional REVERSE ADJACENT KEEP-BLANKS INTERACTIVE)\" t nil)) \
                   (fset 'describe-function '(autoload \"help-fns\" \"Display the full documentation of FUNCTION (a symbol).\\nWhen called from Lisp, FUNCTION may also be a function object.\\n\\nSee the `help-enable-symbol-autoload' variable for special\\nhandling of autoloaded functions.\\n\\n(fn FUNCTION)\" t nil)) \
                   (fset 'describe-mode '(autoload \"help-fns\" \"Display documentation of current major mode and minor modes.\\nA brief summary of the minor modes comes first, followed by the\\nmajor mode description.  This is followed by detailed\\ndescriptions of the minor modes, each on a separate page.\\n\\nFor this to work correctly for a minor mode, the mode's indicator\\nvariable (listed in `minor-mode-alist') must also be a function\\nwhose documentation describes the minor mode.\\n\\nIf called from Lisp with a non-nil BUFFER argument, display\\ndocumentation for the major and minor modes of that buffer.\\n\\nWhen `describe-mode-outline' is non-nil, Outline minor mode\\nis enabled in the Help buffer.\\n\\n(fn &optional BUFFER)\" t nil)) \
                   (fset 'describe-syntax '(autoload \"help-fns\" \"Describe the syntax specifications in the syntax table of BUFFER.\\nThe descriptions are inserted in a help buffer, which is then displayed.\\nBUFFER defaults to the current buffer.\\n\\n(fn &optional BUFFER)\" t nil)) \
                   (fset 'describe-variable '(autoload \"help-fns\" \"Display the full documentation of VARIABLE (a symbol).\\nReturns the documentation as a string, also.\\nIf VARIABLE has a buffer-local value in BUFFER or FRAME\\n(default to the current buffer and current frame),\\nit is displayed along with the global value.\\n\\n(fn VARIABLE &optional BUFFER FRAME)\" t nil)) \
                   (fset 'find-lisp-object-file-name '(autoload \"help-fns\" \"Guess the file that defined the Lisp object OBJECT, of type TYPE.\\nOBJECT should be a symbol associated with a function, variable, or face;\\n  alternatively, it can be a function definition.\\nIf TYPE is `defvar', search for a variable definition.\\nIf TYPE is `defface', search for a face definition.\\nIf TYPE is not a symbol, search for a function definition.\\n\\nThe return value is the absolute name of a readable file where OBJECT is\\ndefined.  If several such files exist, preference is given to a file\\nfound via `load-path'.  The return value can also be `C-source', which\\nmeans that OBJECT is a function or variable defined in C, but\\nit's currently unknown where.  If no suitable file is found,\\nreturn nil.\\n\\nIf ALSO-C-SOURCE is non-nil, instead of returning `C-source',\\nthis function will attempt to locate the definition of OBJECT in\\nthe C sources, too.\\n\\n(fn OBJECT TYPE &optional ALSO-C-SOURCE)\" nil nil)) \
                   (fset 'help-C-file-name '(autoload \"help-fns\" \"Return the name of the C file where SUBR-OR-VAR is defined.\\nKIND should be `var' for a variable or `subr' for a subroutine.\\nIf we can't find the file name, nil is returned.\\n\\n(fn SUBR-OR-VAR KIND)\" nil nil)) \
                   (fset 'reverse-region '(autoload \"sort\" \"Reverse the order of lines in a region.\\nWhen called from Lisp, takes two point or marker arguments, BEG and END.\\nIf BEG is not at the beginning of a line, the first line of those\\nto be reversed is the line starting after BEG.\\nIf END is not at the end of a line, the last line to be reversed\\nis the one that ends before END.\\n\\n(fn BEG END)\" t nil)) \
                   (fset 'sort-columns '(autoload \"sort\" \"Sort lines in region alphabetically by a certain range of columns.\\nFor the purpose of this command, the region BEG...END includes\\nthe entire line that point is in and the entire line the mark is in.\\nThe column positions of point and mark bound the range of columns to sort on.\\nA prefix argument means sort into REVERSE order.\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\nNote that `sort-columns' rejects text that contains tabs,\\nbecause tabs could be split across the specified columns\\nand it doesn't know how to handle that.  Also, when possible,\\nit uses the `sort' utility program, which doesn't understand tabs.\\nUse \\\\[untabify] to convert tabs to spaces before sorting.\\n\\n(fn REVERSE &optional BEG END)\" t nil)) \
                   (fset 'sort-fields '(autoload \"sort\" \"Sort lines in region lexicographically by the ARGth field of each line.\\nFields are separated by whitespace and numbered from 1 up.\\nWith a negative arg, sorts by the ARGth field counted from the right.\\nCalled from a program, there are three arguments:\\nFIELD, BEG and END.  BEG and END specify region to sort.\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\n(fn FIELD BEG END)\" t nil)) \
                   (fset 'sort-lines '(autoload \"sort\" \"Sort lines in region alphabetically; REVERSE non-nil means descending order.\\nInteractively, REVERSE is the prefix argument, and BEG and END are the region.\\nCalled from a program, there are three arguments:\\nREVERSE (non-nil means reverse order), BEG and END (region to sort).\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\n(fn REVERSE BEG END)\" t nil)) \
                   (fset 'sort-numeric-fields '(autoload \"sort\" \"Sort lines in region numerically by the ARGth field of each line.\\nFields are separated by whitespace and numbered from 1 up.\\nSpecified field must contain a number in each line of the region,\\nwhich may begin with \\\"0x\\\" or \\\"0\\\" for hexadecimal and octal values.\\nOtherwise, the number is interpreted according to sort-numeric-base.\\nWith a negative arg, sorts by the ARGth field counted from the right.\\nCalled from a program, there are three arguments:\\nFIELD, BEG and END.  BEG and END specify region to sort.\\n\\n(fn FIELD BEG END)\" t nil)) \
                   (fset 'sort-pages '(autoload \"sort\" \"Sort pages in region alphabetically; argument means descending order.\\nCalled from a program, there are three arguments:\\nREVERSE (non-nil means reverse order), BEG and END (region to sort).\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\n(fn REVERSE BEG END)\" t nil)) \
                   (fset 'sort-paragraphs '(autoload \"sort\" \"Sort paragraphs in region alphabetically; argument means descending order.\\nCalled from a program, there are three arguments:\\nREVERSE (non-nil means reverse order), BEG and END (region to sort).\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\n(fn REVERSE BEG END)\" t nil)) \
                   (fset 'sort-regexp-fields '(autoload \"sort\" \"Sort the text in the region lexicographically.\\nIf called interactively, prompt for two regular expressions,\\nRECORD-REGEXP and KEY-REGEXP.\\n\\nRECORD-REGEXP specifies the textual units to be sorted.\\n  For example, to sort lines, RECORD-REGEXP would be \\\"^.*$\\\".\\n\\nKEY-REGEXP specifies the part of each record (i.e. each match for\\n  RECORD-REGEXP) to be used for sorting.\\n  If it is \\\"\\\\\\\\digit\\\", use the digit'th \\\"\\\\\\\\(...\\\\\\\\)\\\"\\n  match field specified by RECORD-REGEXP.\\n  If it is \\\"\\\\\\\\&\\\", use the whole record.\\n  Otherwise, KEY-REGEXP should be a regular expression with which\\n  to search within the record.  If a match for KEY-REGEXP is not\\n  found within a record, that record is ignored.\\n\\nWith a negative prefix arg, sort in reverse order.\\n\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\nFor example: to sort lines in the region by the first word on each line\\n starting with the letter \\\"f\\\",\\n RECORD-REGEXP would be \\\"^.*$\\\" and KEY would be \\\"\\\\\\\\=\\\\<f\\\\\\\\w*\\\\\\\\>\\\"\\n\\n(fn REVERSE RECORD-REGEXP KEY-REGEXP BEG END)\" t nil)) \
                   (fset 'sort-subr '(autoload \"sort\" \"General text sorting routine to divide buffer into records and sort them.\\n\\nWe divide the accessible portion of the buffer into disjoint pieces\\ncalled sort records.  A portion of each sort record (perhaps all of\\nit) is designated as the sort key.  The records are rearranged in the\\nbuffer in order by their sort keys.  The records may or may not be\\ncontiguous.\\n\\nUsually the records are rearranged in order of ascending sort key.\\nIf REVERSE is non-nil, they are rearranged in order of descending sort key.\\nThe variable `sort-fold-case' determines whether alphabetic case affects\\nthe sort order.\\n\\nThe next four arguments are functions to be called to move point\\nacross a sort record.  They will be called many times from within `sort-subr'.\\n\\nNEXTRECFUN is called with point at the end of the previous record.\\nIt moves point to the start of the next record.\\nIt should move point to the end of the buffer if there are no more records.\\nThe first record is assumed to start at the position of point when `sort-subr'\\nis called.\\n\\nENDRECFUN is called with point within the record.\\nIt should move point to the end of the record.\\n\\nSTARTKEYFUN moves from the start of the record to the start of the key.\\nIt may return either a non-nil value to be used as the key, or\\nelse the key is the substring between the values of point after\\nSTARTKEYFUN and ENDKEYFUN are called.  If STARTKEYFUN is nil, the key\\nstarts at the beginning of the record.\\n\\nENDKEYFUN moves from the start of the sort key to the end of the sort key.\\nENDKEYFUN may be nil if STARTKEYFUN returns a value or if it would be the\\nsame as ENDRECFUN.\\n\\nPREDICATE, if non-nil, is the predicate function for comparing\\nkeys; it is called with two arguments, the keys to compare, and\\nshould return non-nil if the first key should sort before the\\nsecond key.  If PREDICATE is nil, comparison is done with `<' if\\nthe keys are numbers, with `compare-buffer-substrings' if the\\nkeys are cons cells (the car and cdr of each cons cell are taken\\nas start and end positions), and with `string<' otherwise.\\n\\n(fn REVERSE NEXTRECFUN ENDRECFUN &optional STARTKEYFUN ENDKEYFUN PREDICATE)\" nil nil)) \
                   (fset 'tar-mode '(autoload \"tar-mode\" \"Major mode for viewing a tar file as a dired-like listing of its contents.\\nYou can move around using the usual cursor motion commands.\\nLetters no longer insert themselves.\\\\<tar-mode-map>\\nType \\\\[tar-extract] to pull a file out of the tar file and into its own buffer;\\nor click mouse-2 on the file's line in the Tar mode buffer.\\nType \\\\[tar-copy] to copy an entry from the tar file into another file on disk.\\n\\nIf you edit a sub-file of this archive (as with the \\\\[tar-extract] command) and\\nsave it with \\\\[save-buffer], the contents of that buffer will be\\nsaved back into the tar-file buffer; in this way you can edit a file\\ninside of a tar archive without extracting it and re-archiving it.\\n\\nSee also: variables `tar-update-datestamp' and `tar-anal-blocksize'.\\n\\\\{tar-mode-map}\\n\\nIn addition to any hooks its parent mode `special-mode' might have\\nrun, this mode runs the hook `tar-mode-hook', as the final or\\npenultimate step during initialization.\" t nil)) \
                   (fset 'variable-at-point '(autoload \"help-fns\" \"Return the bound variable symbol found at or before point.\\nReturn 0 if there is no such symbol.\\nIf ANY-SYMBOL is non-nil, don't insist the symbol be bound.\\n\\n(fn &optional ANY-SYMBOL)\" nil nil))))",
            );
            // Roll back the `eval-when-compile' requires fired during
            // the dumped libraries' interpreted loads: mouse.el's
            // `(require 'rect)'/`(require 'send-to)' (which pulls in
            // map.el) don't fire in GNU's dumped .elc, so -Q keeps
            // those purely autoloaded.  Unbind every definition the
            // interpreted loads made, then reinstall the entry
            // points' loaddefs autoload cells.
            let rolled_back = interp.eval_str(
                "(let (fns vars) \
                   (dolist (lib '(\"rect\" \"send-to\" \"map\")) \
                     (let ((entry (assoc (concat \"lisp/\" lib \".el\") load-history))) \
                       (when entry \
                         (dolist (item (cdr entry)) \
                           (cond \
                            ((and (consp item) (memq (car item) '(defun defmacro))) \
                             (push (cdr item) fns)) \
                            ((and (consp item) (memq (car item) '(defface require provide autoload))) \
                             nil) \
                            ((symbolp item) (push item vars)) \
                            ((and (consp item) (symbolp (cdr item))) \
                             (push (cdr item) vars)))) \
                         (setq load-history (delq entry load-history))))) \
                   (list fns vars))",
            );
            if let Ok(rolled) = rolled_back {
                if let Ok(groups) = rolled.list_to_vec() {
                    // Unbind in Rust rather than via `makunbound':
                    // that builtin voids an always-buffer-local var by
                    // planting a voided local binding, which would
                    // shadow the default if the library later loads
                    // for real — a plain "never loaded" state needs
                    // the locals entry and the auto-local flag gone.
                    let fns = groups.first().and_then(|v| v.list_to_vec().ok());
                    let vars = groups.get(1).and_then(|v| v.list_to_vec().ok());
                    for v in fns.into_iter().flatten() {
                        if let Value::Sym(sid) = v {
                            interp.obarray.symbol_mut(sid).function = Value::Sym(sym::UNBOUND);
                        }
                    }
                    for v in vars.into_iter().flatten() {
                        if let Value::Sym(sid) = v {
                            {
                                let s = interp.obarray.symbol_mut(sid);
                                s.value = Value::Sym(sym::UNBOUND);
                                s.special = false;
                                s.constant = false;
                                s.make_local_if_set = false;
                                s.variable_documentation = None;
                            }
                            for bid in interp.buffers.list() {
                                if let Some(b) = interp.buffers.get(bid) {
                                    b.borrow_mut().locals.remove(&sid);
                                }
                            }
                        }
                    }
                }
            }
            let _ = interp.eval_str(
                "(progn
  (fset 'clear-rectangle '(autoload \"rect\" \"Blank out the region-rectangle.
The text previously in the region is overwritten with blanks.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill with blanks the parts of the
rectangle which were empty.

(fn START END &optional FILL)\" t nil))
  (fset 'copy-rectangle-as-kill '(autoload \"rect\" \"Copy the region-rectangle and save it as the last killed one.

(fn START END)\" t nil))
  (fset 'delete-extract-rectangle '(autoload \"rect\" \"Delete the contents of the rectangle with corners at START and END.
Return it as a list of strings, one for each line of the rectangle.

When called from a program the rectangle's corners are START and END.
With an optional FILL argument, also fill lines where nothing has to be
deleted.

(fn START END &optional FILL)\" nil nil))
  (fset 'delete-rectangle '(autoload \"rect\" \"Delete (don't save) text in the region-rectangle.
The same range of columns is deleted in each line starting with the
line where the region begins and ending with the line where the region
ends.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill lines where nothing has
to be deleted.

(fn START END &optional FILL)\" t nil))
  (fset 'delete-whitespace-rectangle '(autoload \"rect\" \"Delete all whitespace following a specified column in each line.
The left edge of the rectangle specifies the position in each line
at which whitespace deletion should begin.  On each line in the
rectangle, all contiguous whitespace starting at that column is deleted.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, also fill too short lines.

(fn START END &optional FILL)\" t nil))
  (fset 'extract-rectangle '(autoload \"rect\" \"Return the contents of the rectangle with corners at START and END.
Return it as a list of strings, one for each line of the rectangle.

(fn START END)\" nil nil))
  (fset 'insert-rectangle '(autoload \"rect\" \"Insert text of RECTANGLE with upper left corner at point.
RECTANGLE's first line is inserted at point, its second
line is inserted at a point vertically under point, etc.
RECTANGLE should be a list of strings.
After this command, the mark is at the upper left corner
and point is at the lower right corner.

(fn RECTANGLE)\" nil nil))
  (fset 'kill-rectangle '(autoload \"rect\" \"Delete the region-rectangle and save it as the last killed one.

When called from a program the rectangle's corners are START and END.
You might prefer to use `delete-extract-rectangle' from a program.

With a prefix (or a FILL) argument, also fill lines where nothing has to be
deleted.

If the buffer is read-only, Emacs will beep and refrain from deleting
the rectangle, but put it in `killed-rectangle' anyway.  This means that
you can use this command to copy text from a read-only buffer.
(If the variable `kill-read-only-ok' is non-nil, then this won't
even beep.)

(fn START END &optional FILL)\" t nil))
  (fset 'open-rectangle '(autoload \"rect\" \"Blank out the region-rectangle, shifting text right.

The text previously in the region is not overwritten by the blanks,
but instead winds up to the right of the rectangle.

When called from a program the rectangle's corners are START and END.
With a prefix (or a FILL) argument, fill with blanks even if there is
no text on the right side of the rectangle.

(fn START END &optional FILL)\" t nil))
  (fset 'rectangle-mark-mode '(autoload \"rect\" \"Toggle the region as rectangular.

Activates the region if it's inactive and Transient Mark mode is
on.  Only lasts until the region is next deactivated.

This is a minor mode.  If called interactively, toggle the
`Rectangle-Mark mode' mode.  If the prefix argument is positive, enable
the mode, and if it is zero or negative, disable the mode.

If called from Lisp, toggle the mode if ARG is `toggle'.  Enable the
mode if ARG is nil, omitted, or is a positive number.  Disable the mode
if ARG is a negative number.

To check whether the minor mode is enabled in the current buffer,
evaluate the variable `rectangle-mark-mode'.

The mode's hook is called both when the mode is enabled and when it is
disabled.

\\\\{rectangle-mark-mode-map}

(fn &optional ARG)\" t nil))
  (fset 'rectangle-number-lines '(autoload \"rect\" \"Insert numbers in front of the region-rectangle.

START-AT, if non-nil, should be a number from which to begin
counting.  FORMAT, if non-nil, should be a format string to pass
to `format' along with the line count.  When called interactively
with a prefix argument, prompt for START-AT and FORMAT.

(fn START END START-AT &optional FORMAT)\" t nil))
  (fset 'string-insert-rectangle '(autoload \"rect\" \"Insert STRING on each line of region-rectangle, shifting text right.

When called from a program, the rectangle's corners are START and END.
The left edge of the rectangle specifies the column for insertion.
This command does not delete or overwrite any existing text.

(fn START END STRING)\" t nil))
  (fset 'string-rectangle '(autoload \"rect\" \"Replace rectangle contents with STRING on each line.
The length of STRING need not be the same as the rectangle width.

When called interactively and option `rectangle-preview' is
non-nil, display the result as the user enters the string into
the minibuffer.

Called from a program, takes three args; START, END and STRING.

(fn START END STRING)\" t nil))
  (fset 'yank-rectangle '(autoload \"rect\" \"Yank the last killed rectangle with upper left corner at point.\" t nil))
  (fset 'send-to--resolve-handler '(autoload \"send-to\" nil nil nil))
  (fset 'send-to-supported-p '(autoload \"send-to\" \"Return non-nil for platforms where `send-to' is supported.\" nil nil))
  (fset 'send-to '(autoload \"send-to\" \"Send file(s) or region text to (non-Emacs) applications or services.

Sending is handled by the first supported handler from `send-to-handlers'.

ITEMS list is also populated by the resolved handler, but can be
explicitly overridden.

(fn &optional ITEMS)\" t nil)) \
  (fset 'close-rectangle 'delete-whitespace-rectangle) \
  (fset 'replace-rectangle 'string-rectangle))",
            );
            // rect.el's `rectangle-preview' defface must stay
            // unloaded too.
            interp.face_table.retain(|(n, _)| n != "rectangle-preview");

            // The prelude amalgamates helpers that GNU 31.1 keeps void
            // at -Q: they live in libraries that aren't dumped
            // (subr-x.el, help.el, pcase/rx internals, ...) or simply
            // don't exist upstream (`second', `copy-seq', ...).  A
            // static call-graph over the dumped defs shows no kept
            // (GNU-bound) definition calls these, so void their
            // function cells to match GNU's boot state.  The real
            // lisp/*.el files still rebind them on `require'.
            const GNU_VOID_FNS: &[&str] = &[
                "advice--make-how-alist",
                "append-to-list",
                "bool-vector-length",
                "buffer-name-as-string",
                "buffer-substring-with-properties",
                "buffer-word-at-point",
                "byte-compile-warn-x",
                "car-or-marker-p",
                "char-table",
                "cl--advice--apply",
                "cl--find-class",
                "cl--old-struct-type-of",
                "cl-struct--pcase-macroexpander",
                // GNU installs no cl-seq/cl-extra autoload cells at -Q;
                // the real libraries rebind these on require.
                "cl--adjoin",
                "cl--compiler-macro-adjoin",
                "cl--derived-type-generalizers",
                "cl--do-remf",
                "cl--map-intervals",
                "cl--map-overlays",
                "cl--mapcar-many",
                "cl--optimize",
                "cl--set-frame-visible-p",
                "cl--set-getf",
                "cl-assoc",
                "cl-assoc-if",
                "cl-assoc-if-not",
                "cl-ceiling",
                "cl-coerce",
                "cl-compiler-macroexpand",
                "cl-concatenate",
                "cl-count",
                "cl-count-if",
                "cl-count-if-not",
                "cl-define-compiler-macro",
                "cl-defsubst",
                "cl-deftype",
                "cl-delete",
                "cl-delete-duplicates",
                "cl-delete-if",
                "cl-delete-if-not",
                "cl-describe-type",
                "cl-endp",
                "cl-equalp",
                "cl-every",
                "cl-fill",
                "cl-find",
                "cl-find-class",
                "cl-find-if",
                "cl-find-if-not",
                "cl-float-limits",
                "cl-floor",
                "cl-fresh-line",
                "cl-gcd",
                "cl-get",
                "cl-getf",
                "cl-intersection",
                "cl-isqrt",
                "cl-iter-defun",
                "cl-lcm",
                "cl-list-length",
                "cl-make-random-state",
                "cl-mapc",
                "cl-mapcan",
                "cl-mapcon",
                "cl-mapl",
                "cl-maplist",
                "cl-member",
                "cl-member-if",
                "cl-member-if-not",
                "cl-merge",
                "cl-mismatch",
                "cl-mod",
                "cl-nintersection",
                "cl-nset-difference",
                "cl-nset-exclusive-or",
                "cl-nsublis",
                "cl-nsubst",
                "cl-nsubst-if",
                "cl-nsubst-if-not",
                "cl-nsubstitute",
                "cl-nsubstitute-if",
                "cl-nsubstitute-if-not",
                "cl-nunion",
                "cl-parse-integer",
                "cl-position",
                "cl-position-if",
                "cl-position-if-not",
                "cl-prettyexpand",
                "cl-random",
                "cl-random-state-p",
                "cl-rassoc",
                "cl-rassoc-if",
                "cl-rassoc-if-not",
                "cl-reduce",
                "cl-rem",
                "cl-remove",
                "cl-remove-duplicates",
                "cl-remove-if",
                "cl-remove-if-not",
                "cl-remprop",
                "cl-replace",
                "cl-round",
                "cl-search",
                "cl-set-difference",
                "cl-set-exclusive-or",
                "cl-signum",
                "cl-some",
                "cl-sort",
                "cl-stable-sort",
                "cl-struct-sequence-type",
                "cl-struct-slot-info",
                "cl-struct-slot-offset",
                "cl-sublis",
                "cl-subseq",
                "cl-subsetp",
                "cl-subst-if",
                "cl-subst-if-not",
                "cl-substitute",
                "cl-substitute-if",
                "cl-substitute-if-not",
                "cl-tailp",
                "cl-tree-equal",
                "cl-truncate",
                "cl-type--pcase-macroexpander",
                "cl-union",
                "clear-vector",
                "connection-local-criteria-for-default-directory",
                "connection-local-get-profile-variables",
                "connection-local-get-profiles",
                "connection-local-normalize-criteria",
                "connection-local-profile-name-for-criteria",
                "copy-seq",
                "custom-face-state",
                "custom-face-tag",
                "custom-group-list",
                "custom-group-tag",
                "custom-theme-load-themes",
                "custom-variable-state",
                "custom-variable-tag",
                "declare-functionp",
                "dir-locals-to-string",
                "emacs-build-time",
                "emacs-etc--hide-local-variables",
                "face-attrs--make-indirect-safe",
                "face-remap--clear-remappings",
                "face-remap--remap-face",
                "face-remap-remove-relative",
                "feature-file",
                "feature-symbols",
                "file-dependents",
                "file-loadhist-lookup",
                "file-provides",
                "file-requires",
                "file-set-intersect",
                "find-function--defface",
                "forward-line-command",
                "frame--list-z-order",
                "handle-change-group",
                "hash-table-empty-p",
                "hash-table-keys",
                "hash-table-values",
                "help-button-action",
                "help-customize",
                "help-do-xref",
                "help-follow",
                "help-follow-mouse",
                "help-follow-symbol",
                "help-function-def--button-function",
                "help-go-back",
                "help-go-forward",
                "help-goto-info",
                "help-goto-lispref-info",
                "help-goto-next-page",
                "help-goto-previous-page",
                "help-insert-string",
                "help-mode-context-menu",
                "help-mode-menu",
                "help-mode-revert-buffer",
                "help-view-source",
                "help-xref-go-back",
                "help-xref-go-forward",
                "internal--thread-argument",
                "internal-doc-string-p",
                "kmacro-end-or-call-macro-repeat",
                "list-length",
                "make-obsolete-generalized-variable",
                "map--plist-p",
                "member-if-not",
                "modify-dir-local-variable",
                "modify-file-local-variable",
                "modify-file-local-variable-message",
                "modify-file-local-variable-prop-line",
                "output-switches",
                "overlays-at-point",
                "prop-match-p",
                "read-dir-locals-file",
                "read-feature",
                "read-file-local-variable",
                "read-file-local-variable-mode",
                "read-file-local-variable-value",
                "same-names-p",
                "scribe-mode",
                "seq-last",
                "set-translation-table",
                "shell-command-mode",
                "string-aref",
                "string-compare",
                "string-remove-prefix",
                "string-remove-suffix",
                "string-to-sequence",
                "text-property--find-end-backward",
                "text-property-search-backward",
                "text-scale--refresh",
                "thread-first",
                "thread-last",
                "timer-p",
                "toggle-read-only",
                "unload--set-major-mode",
                "window-has-parameters",
                "window-inside-absolute-body-pixel-edges",
                "window-left-char",
                "window-line",
                // Remaining dump-time helpers GNU 31.1 leaves void at -Q.
                "(setf accessor--slot)",
                "(setf accessor--type)",
                "(setf built-in-class--non-abstract-supertype)",
                "(setf cl--class-docstring)",
                "(setf cl--class-index-table)",
                "(setf cl--class-name)",
                "(setf cl--class-parents)",
                "(setf cl--class-slots)",
                "(setf cl--generic)",
                "(setf cl--generic-dispatches)",
                "(setf cl--generic-generalizer-name)",
                "(setf cl--generic-generalizer-priority)",
                "(setf cl--generic-generalizer-specializers-function)",
                "(setf cl--generic-generalizer-tagcode-function)",
                "(setf cl--generic-lazy-function)",
                "(setf cl--generic-method-call-con)",
                "(setf cl--generic-method-function)",
                "(setf cl--generic-method-qualifiers)",
                "(setf cl--generic-method-specializers)",
                "(setf cl--generic-method-table)",
                "(setf cl--generic-name)",
                "(setf cl--generic-options)",
                "(setf cl--slot-descriptor-initform)",
                "(setf cl--slot-descriptor-name)",
                "(setf cl--slot-descriptor-props)",
                "(setf cl--slot-descriptor-type)",
                "(setf cl--struct-class-children-sym)",
                "(setf cl--struct-class-docstring)",
                "(setf cl--struct-class-index-table)",
                "(setf cl--struct-class-name)",
                "(setf cl--struct-class-named)",
                "(setf cl--struct-class-parents)",
                "(setf cl--struct-class-print)",
                "(setf cl--struct-class-slots)",
                "(setf cl--struct-class-tag)",
                "(setf cl--struct-class-type)",
                "(setf decoded-time-day)",
                "(setf decoded-time-dst)",
                "(setf decoded-time-hour)",
                "(setf decoded-time-minute)",
                "(setf decoded-time-month)",
                "(setf decoded-time-second)",
                "(setf decoded-time-weekday)",
                "(setf decoded-time-year)",
                "(setf decoded-time-zone)",
                "(setf lisp-indent-state-ppss)",
                "(setf lisp-indent-state-ppss-point)",
                "(setf lisp-indent-state-stack)",
                "(setf oclosure--class-allparents)",
                "(setf oclosure--class-docstring)",
                "(setf oclosure--class-index-table)",
                "(setf oclosure--class-name)",
                "(setf oclosure--class-parents)",
                "(setf oclosure--class-slots)",
                "(setf oclosure-accessor--index)",
                "(setf oclosure-accessor--slot)",
                "(setf oclosure-accessor--type)",
                "(setf registerv-data)",
                "(setf registerv-insert-func)",
                "(setf registerv-jump-func)",
                "(setf registerv-print-func)",
                "(setf timer--args)",
                "(setf timer--function)",
                "(setf timer--high-seconds)",
                "(setf timer--idle-delay)",
                "(setf timer--integral-multiple)",
                "(setf timer--low-seconds)",
                "(setf timer--psecs)",
                "(setf timer--repeat-delay)",
                "(setf timer--time)",
                "(setf timer--triggered)",
                "(setf timer--usecs)",
                "(setf uniquify-item-dirname)",
                "(setf uniquify-item-proposed)",
                "(setf xref-elisp-location-file)",
                "(setf xref-elisp-location-symbol)",
                "(setf xref-elisp-location-type)",
                "`--pcase-macroexpander",
                "abbrev--expand-body",
                "abbrev--expand-wrapped",
                "add-remove--display-text-property",
                "apply-on-rectangle",
                "bidi--char-in-category-p",
                "buffer-face-mode-invoke",
                "cl--add-function",
                "cl--advice--link",
                "cl--advice-place-code",
                "cl--block-throw",
                "cl--block-wrapper",
                "cl--check-keys",
                "cl--compile-time-too",
                "cl--compiler-macro-cXXr",
                "cl--compiler-macro-list*",
                "cl--compiling-file",
                "cl--defalias",
                "cl--defun-1",
                "cl--do-proclaim",
                "cl--do-subst",
                "cl--expand-do-loop",
                "cl--expr-contains",
                "cl--expr-contains-any",
                "cl--expr-depends-p",
                "cl--generic-dispatch",
                "cl--keyfn",
                "cl--labels-convert",
                "cl--loop-action",
                "cl--loop-cond",
                "cl--loop-destruct",
                "cl--loop-destruct-accessors",
                "cl--loop-expand",
                "cl--loop-hash-pairs",
                "cl--method-fn",
                "cl--method-more-specific-p",
                "cl--methods-with-qual",
                "cl--prog",
                "cl--remove-function",
                "cl--safe-expr-p",
                "cl--set-buffer-substring",
                "cl--set-substring",
                "cl--simple-expr-p",
                "cl--simple-exprs-p",
                "cl--sm-subst",
                "cl--spec-applicable-p",
                "cl--spec-more-specific-p",
                "cl--take",
                "cl--thread-expand",
                "cl--type-parents",
                "cl-acons",
                "cl-adjoin",
                "cl-assert",
                "cl-block",
                "cl-caaaar",
                "cl-caaadr",
                "cl-caaar",
                "cl-caadar",
                "cl-caaddr",
                "cl-caadr",
                "cl-cadaar",
                "cl-cadadr",
                "cl-cadar",
                "cl-caddar",
                "cl-cadddr",
                "cl-caddr",
                "cl-callf",
                "cl-callf2",
                "cl-case",
                "cl-cdaaar",
                "cl-cdaadr",
                "cl-cdaar",
                "cl-cdadar",
                "cl-cdaddr",
                "cl-cdadr",
                "cl-cddaar",
                "cl-cddadr",
                "cl-cddar",
                "cl-cdddar",
                "cl-cddddr",
                "cl-cdddr",
                "cl-check-type",
                "cl-constantly",
                "cl-copy-list",
                "cl-copy-seq",
                "cl-decf",
                "cl-declaim",
                "cl-declare",
                "cl-defmacro",
                "cl-defstruct",
                "cl-defun",
                "cl-destructuring-bind",
                "cl-digit-char-p",
                "cl-do",
                "cl-do*",
                "cl-do-all-symbols",
                "cl-do-symbols",
                "cl-dolist",
                "cl-dotimes",
                "cl-ecase",
                "cl-eighth",
                "cl-etypecase",
                "cl-eval-when",
                "cl-evenp",
                "cl-fifth",
                "cl-first",
                "cl-flet",
                "cl-flet*",
                "cl-floatp-safe",
                "cl-fourth",
                "cl-function",
                "cl-gensym",
                "cl-gentemp",
                "cl-labels",
                "cl-ldiff",
                "cl-letf",
                "cl-letf*",
                "cl-list*",
                "cl-load-time-value",
                "cl-locally",
                "cl-loop",
                "cl-macrolet",
                "cl-map",
                "cl-mapcar",
                "cl-minus",
                "cl-minusp",
                "cl-multiple-value-apply",
                "cl-multiple-value-bind",
                "cl-multiple-value-call",
                "cl-multiple-value-list",
                "cl-multiple-value-setq",
                "cl-ninth",
                "cl-notany",
                "cl-notevery",
                "cl-nreconc",
                "cl-nth-value",
                "cl-oddp",
                "cl-once-only",
                "cl-pairlis",
                "cl-plus",
                "cl-plusp",
                "cl-proclaim",
                "cl-prog",
                "cl-prog*",
                "cl-progv",
                "cl-psetf",
                "cl-psetq",
                "cl-pushnew",
                "cl-remf",
                "cl-rest",
                "cl-return",
                "cl-return-from",
                "cl-revappend",
                "cl-rotatef",
                "cl-second",
                "cl-seventh",
                "cl-shiftf",
                "cl-sixth",
                "cl-subst",
                "cl-svref",
                "cl-symbol-macrolet",
                "cl-tagbody",
                "cl-tenth",
                "cl-the",
                "cl-third",
                "cl-times",
                "cl-typecase",
                "cl-typep",
                "cl-values",
                "cl-values-list",
                "cl-with-accessors",
                "cl-with-gensyms",
                "clear-rectangle-line",
                "custom--settings-delete",
                "custom--theme-entry-delete",
                "custom-unlispify-menu-entry",
                "decoded-time--defslot",
                "define-icon",
                "delete-extract-rectangle-line",
                "delete-rectangle-line",
                "delete-whitespace-rectangle-line",
                "display-buffer-mark-dedicated",
                "easy-mmode--next",
                "easy-mmode--prev",
                "easy-mmode-define-navigation",
                "epa-file",
                "event-apply--modifier",
                "extract-rectangle-bounds",
                "extract-rectangle-line",
                "face-attrs-more-relative-p",
                "face-remap-order",
                "find-function--any-subform-p",
                "find-function--search-by-expanding-macros",
                "find-function--try-macroexpand",
                "find-function-C-source",
                "find-function-advised-original",
                "find-function-do-it",
                "find-function-library",
                "find-function-on-key-do-it",
                "find-function-read",
                "find-library--from-load-history",
                "find-library--load-name",
                "find-library-name",
                "find-library-suffixes",
                "gv--defsetter",
                "gv-delay-error",
                "gv-deref",
                "gv-setter",
                "gv-synthetic-place",
                "gv-synthetic-place--anon-cmacro",
                "help-xref--navigation-buttons",
                "icon-complete-spec",
                "icon-documentation",
                "icon-elements",
                "icon-spec-keywords",
                "icon-spec-values",
                "icon-string",
                "iconp",
                "icons--copy-spec",
                "icons--create",
                "icons--describe-spec",
                "icons--merge-spec",
                "icons--register",
                "icons--spec",
                "insert-directory-adj-pos",
                "internal--set-subr-doc",
                "internal-make-interpreted-closure-function",
                "jka-compr",
                "let--pcase-macroexpander",
                "list-tail",
                "make-help-screen",
                "make-prop-match",
                "open-rectangle-line",
                "operate-on-rectangle",
                "pcase--and",
                "pcase--app-subst-match",
                "pcase--app-subst-rest",
                "pcase--edebug-match-pat-args",
                "pcase--eval",
                "pcase--expand",
                "pcase--expand-`",
                "pcase--flip",
                "pcase--funcall",
                "pcase--get-macroexpander",
                "pcase--if",
                "pcase--let*",
                "pcase--macroexpand",
                "pcase--mark-used",
                "pcase--match",
                "pcase--mutually-exclusive-p",
                "pcase--self-quoting-p",
                "pcase--small-branch-p",
                "pcase--split-equal",
                "pcase--split-match",
                "pcase--split-member",
                "pcase--split-pred",
                "pcase--split-rest",
                "pcase--subtype-bitsets",
                "pcase--trivial-upat-p",
                "pcase--u",
                "pcase--u1",
                "pcase-compile-patterns",
                "prop-match-value",
                "pushnew",
                "read-library-name--find-files",
                "rectangle--*-char",
                "rectangle--col-pos",
                "rectangle--crutches",
                "rectangle--default-line-number-format",
                "rectangle--extract-region",
                "rectangle--insert-for-yank",
                "rectangle--insert-region",
                "rectangle--point-col",
                "rectangle--pos-cols",
                "rectangle--region-beginning",
                "rectangle--region-end",
                "rectangle--reset-crutches",
                "rectangle--reset-point-crutches",
                "rectangle--space-to",
                "rectangle--string-erase-preview",
                "rectangle--string-flush-preview",
                "rectangle--string-preview",
                "rectangle-backward-char",
                "rectangle-dimensions",
                "rectangle-exchange-point-and-mark",
                "rectangle-forward-char",
                "rectangle-intersect-p",
                "rectangle-left-char",
                "rectangle-next-line",
                "rectangle-number-line-callback",
                "rectangle-position-as-coordinates",
                "rectangle-previous-line",
                "rectangle-right-char",
                "remacs--char-width-table",
                "remacs--dir-locals-merge",
                "remacs--dir-locals-merge-vars",
                "repos-count-screen-lines",
                "repos-count-screen-lines-signed",
                "rx--all-string-branches-p",
                "rx--atomic-regexp",
                "rx--bracket",
                "rx--char-alt-union",
                "rx--check-repeat-arg",
                "rx--collect-or-strings",
                "rx--condense-intervals",
                "rx--control-greedy",
                "rx--empty",
                "rx--enclose",
                "rx--expand-def-form",
                "rx--expand-def-symbol",
                "rx--expand-eval",
                "rx--expand-template",
                "rx--extend-local-defs",
                "rx--foldl",
                "rx--generate-alt",
                "rx--human-readable",
                "rx--intersection-intervals",
                "rx--interval-set-complement",
                "rx--interval-set-intersection",
                "rx--interval-set-union",
                "rx--lookup-def",
                "rx--make-binding",
                "rx--make-named-binding",
                "rx--normalize-char-pattern",
                "rx--optimize-or-args",
                "rx--parse-any",
                "rx--pcase-transform",
                "rx--reduce-right",
                "rx--reduce-to-char-alt",
                "rx--sequence",
                "rx--string-to-intervals",
                "rx--substitute",
                "rx--to-expr",
                "rx--translate",
                "rx--translate-**",
                "rx--translate-=",
                "rx--translate->=",
                "rx--translate-any",
                "rx--translate-backref",
                "rx--translate-bounded-repetition",
                "rx--translate-category",
                "rx--translate-char-alt",
                "rx--translate-compat-form",
                "rx--translate-compat-form-entry",
                "rx--translate-compat-symbol-entry",
                "rx--translate-counted-repetition",
                "rx--translate-eval",
                "rx--translate-form",
                "rx--translate-group",
                "rx--translate-group-n",
                "rx--translate-intersection",
                "rx--translate-literal",
                "rx--translate-not",
                "rx--translate-or",
                "rx--translate-regexp",
                "rx--translate-rep",
                "rx--translate-repeat",
                "rx--translate-seq",
                "rx--translate-symbol",
                "rx--translate-syntax",
                "rx-submatch-n",
                "seq",
                "sequence",
                "spaces-string",
                "string-rectangle-line",
                "text-property--find-end-forward",
                "text-property--match-p",
                "text-scale-max-amount",
                "text-scale-min-amount",
                "text-scale-mode",
                "timer--defslot",
                "unsafep--check",
                "values",
                "with-buffer-unmodified-if-unchanged",
                "work-buffer--get",
                "work-buffer--prepare-pixelwise",
            ];
            for name in GNU_VOID_FNS {
                let sid = interp.intern(name);
                let old = interp.obarray.symbol(sid).function.clone();
                if *name == "cl--advice--apply" {
                    // `advice-add' trampolines call this dispatcher; GNU
                    // voids the name at -Q, so stash the subr on the
                    // symbol's plist where Lisp code can't reach it via
                    // `symbol-function'.
                    let pk = interp.intern("cl--advice--apply--fn");
                    interp.put_prop(sid, pk, old.clone());
                }
                if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) {
                    let pk = interp.intern("remacs--dump-fn");
                    interp.put_prop(sid, pk, old);
                }
                interp.obarray.symbol_mut(sid).function = Value::Sym(sym::UNBOUND);
            }
            // GNU's dump-time `autoload' is a plain defalias: every
            // `(autoload ...)' form in loaddefs.elc installs its cell
            // unconditionally, even over definitions the dump itself
            // made (e.g. the `pcase' macro).  Reinstall each loaddefs
            // autoload cell over whatever the amalgamated prelude
            // bound so that first use loads the real library — and
            // its expansion-time internals — exactly like GNU.
            if let Some(src) = crate::lisp::load::embedded("loaddefs") {
                let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
                let auto_id = interp.intern("autoload");
                let macro_id = interp.intern("macro");
                let void_names: HashSet<SymId> = GNU_VOID_FNS
                    .iter()
                    .map(|name| interp.intern(name))
                    .collect();
                let mut pos = 0usize;
                loop {
                    let form = {
                        let mut reader = Reader::with_chars(&mut interp, chars.clone());
                        reader.set_position(pos);
                        match reader.read() {
                            Ok(Some(f)) => {
                                pos = reader.position();
                                f
                            }
                            _ => break,
                        }
                    };
                    // `(autoload 'name FILE DOC INTERACTIVE TYPE)'
                    let items = match &form {
                        Value::Cons(_) => form.list_to_vec().ok(),
                        _ => None,
                    };
                    let Some(items) = items else { continue };
                    if items.len() < 2 {
                        continue;
                    }
                    if !matches!(&items[0], Value::Sym(h) if *h == auto_id) {
                        continue;
                    }
                    // items[1] is (quote name).
                    let name = match &items[1] {
                        Value::Cons(q) => {
                            let qb = q.borrow();
                            let rest = match &qb.cdr {
                                Value::Cons(r) => r.borrow().car.clone(),
                                _ => continue,
                            };
                            match rest {
                                Value::Sym(id) => id,
                                _ => continue,
                            }
                        }
                        _ => continue,
                    };
                    if void_names.contains(&name) {
                        continue;
                    }
                    // Leave cells alone when the autoload's own file
                    // hosts expansion machinery: interpreting that
                    // file needs the expander/helper bound, so the
                    // autoload would recurse into the same load.  GNU
                    // avoids this because .elc files are preexpanded.
                    let target_selfhosts = match &items[2] {
                        Value::Str(s) => {
                            let f = s.borrow();
                            matches!(
                                f.as_str(),
                                "cl-macs"
                                    | "cl-preloaded"
                                    | "cl-generic"
                                    | "cl-lib"
                                    | "pcase"
                                    | "rx"
                                    | "gv"
                                    | "easy-mmode"
                                    | "map"
                                    | "subr-x"
                                    | "inline"
                                    | "macroexp"
                                    | "byte-run"
                                    | "oclosure"
                                    | "nadvice"
                                    | "rect"
                                    | "send-to"
                                    | "icons"
                                    | "warnings"
                            )
                        }
                        _ => false,
                    };
                    if target_selfhosts {
                        continue;
                    }
                    // Macros in any other file are just as un-satisfiable.
                    if matches!(
                        &interp.obarray.symbol(name).function,
                        Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(m) if *m == macro_id)
                    ) {
                        continue;
                    }
                    let mut cell = vec![Value::Sym(auto_id)];
                    cell.extend_from_slice(&items[2..]);
                    interp.fset(name, Value::list(cell));
                }
            }
        }
        interp.loading_dumped = false;
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            // startup.el processes variables whose defcustom used
            // `custom-initialize-delay' via `custom-reevaluate-setting',
            // then sets the list to a non-list so later :initialize calls
            // (e.g. from `require') initialize immediately.
            let _ = interp.eval_str(
                "(progn (mapc #'custom-reevaluate-setting \
                              (nreverse custom-delayed-init-variables)) \
                         (setq custom-delayed-init-variables t))",
            );
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
            br.locals
                .insert(prech, Value::list(vec![Value::Sym(eldoc_pre), Value::t()]));
        }
        let tooltip_hide = interp.intern("tooltip-hide");
        interp.obarray.symbol_mut(prech).value = Value::list(vec![Value::Sym(tooltip_hide)]);
        // Boot-time autoloads (easy-mmode & co.) correspond to GNU's
        // dumped loadup; the user-visible `features' list must match the
        // post-dump set.
        interp.features = dump_features;
        // `icons' faces created by boot-time loads are likewise
        // hidden until a real `load' triggers them.
        interp
            .face_table
            .retain(|(n, _)| n != "icon" && n != "icon-button");
        let flist = Value::list(interp.features.iter().map(|s| Value::Sym(*s)).collect());
        let fid = interp.intern("features");
        interp.obarray.symbol_mut(fid).value = flist;
        // GNU resets `gensym-counter' to 0 when the dumped image starts
        // (pdumper boot), so dump-time gensyms don't leak into the
        // session.  Our boot-time library loads play the dump's role;
        // reset last so boot's own macroexpansion gensyms don't count.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            let gc = interp.intern("gensym-counter");
            interp.obarray.symbol_mut(gc).value = Value::Int(0);
        }
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
        self.unibyte_strings
            .insert(std::rc::Rc::as_ptr(s) as usize, std::rc::Rc::downgrade(s));
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
        self.multibyte_strings
            .insert(std::rc::Rc::as_ptr(s) as usize, std::rc::Rc::downgrade(s));
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
    pub fn set_str_props(
        &mut self,
        s: &crate::lisp::value::StrRef,
        v: Vec<(usize, usize, Vec<Value>)>,
    ) {
        self.string_props.insert(
            std::rc::Rc::as_ptr(s) as usize,
            (std::rc::Rc::downgrade(s), v),
        );
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
            self.buffers.undo_inhibit_cell().set(!val.is_nil());
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

    /// Function cell used by ordinary calls.  Hidden dump-time helpers
    /// are reachable only while a macro expander is running; runtime calls
    /// from dumped definitions still observe GNU's void -Q cells.
    fn callable_function(&mut self, id: SymId) -> Value {
        let f = self.symbol_function(id);
        if !matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            return f;
        }
        if self.macroexp_call_depth > 0 {
            return self.dumped_function(id).unwrap_or(f);
        }
        // Dump-internal calls: GNU's dump keeps every definition it
        // loaded reachable from other dumped code even when the public
        // cell is void at -Q.  Our stash on `remacs--dump-fn' plays that
        // role — anything defined during the prelude/dump load stays
        // callable from other dumped functions (e.g. `rx-to-string'
        // reaching `rx--translate').  `dumped_runtime_helper' remains as
        // documentation of the cases that motivated the mechanism.
        if self.dumped_call_depth > 0 || self.loading_dumped {
            return self.dumped_function(id).unwrap_or(f);
        }
        f
    }

    fn dumped_runtime_helper(&self, id: SymId) -> bool {
        let name = self.obarray.name(id);
        name.starts_with("(setf ")
            || name.starts_with("rx--")
            || matches!(
                name,
                "cl--generic-dispatch"
                    | "cl--advice--apply"
                    | "cl--advice--link"
                    | "cl--add-function"
                    | "cl--check-keys"
                    | "cl--method-fn"
                    | "cl--method-more-specific-p"
                    | "cl--methods-with-qual"
                    | "cl--spec-applicable-p"
                    | "cl--spec-more-specific-p"
                    | "cl--type-parents"
                    | "cl-typep"
                    | "gv-deref"
            )
    }

    /// Hidden dump-time definition for SYM, if one was stashed while its
    /// public function cell was voided for GNU -Q compatibility.
    fn dumped_function(&mut self, id: SymId) -> Option<Value> {
        let prop = self.intern("remacs--dump-fn");
        let hidden = self.get_prop(id, prop);
        if hidden.is_nil() || matches!(hidden, Value::Sym(s) if s == sym::UNBOUND) {
            None
        } else {
            Some(hidden)
        }
    }

    /// Function cell for the head of a form.  In addition to normal
    /// expansion-time access, a dumped definition may still call the
    /// macros GNU expanded away at dump time.  Ordinary hidden functions
    /// are not exposed here.
    fn form_function(&mut self, id: SymId) -> Value {
        let f = self.symbol_function(id);
        if !matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            return f;
        }
        if self.macroexp_call_depth > 0 {
            return self.dumped_function(id).unwrap_or(f);
        }
        if self.dumped_call_depth == 0 && !self.loading_dumped {
            return f;
        }
        match self.dumped_function(id) {
            Some(v) if self.is_macro_function(&v) || self.dumped_runtime_helper(id) => v,
            _ => f,
        }
    }

    fn is_macro_function(&self, v: &Value) -> bool {
        match v {
            Value::Lambda(l) => l.is_macro,
            Value::Cons(c) => self.sym_is(&c.borrow().car, sym::MACRO),
            _ => false,
        }
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
        *self.last_error_stack.borrow_mut() = self.lisp_stack.clone();
        Flow::Signal(Value::Sym(sym_id), data, false)
    }

    /// `(signal sym data-list-from-vec)`.
    pub fn signal_data(&self, sym_id: SymId, data: Vec<Value>) -> Flow {
        *self.last_error_stack.borrow_mut() = self.lisp_stack.clone();
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
        // Like GNU's `eval_buffer': `lexical-binding' is sampled once at
        // entry; truthy → a fresh `(t)' lexical env, else nil (dynamic).
        // Scoped `defvar' declarations then unwind with the eval.
        let lex_on = self
            .obarray
            .intern_soft("lexical-binding")
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false);
        let saved_lexenv =
            std::mem::replace(&mut self.lexenv, if lex_on { lexenv_root() } else { None });
        let r = self.eval_str_inner(src);
        self.lexenv = saved_lexenv;
        r
    }

    fn eval_str_inner(&mut self, src: &str) -> EvalResult {
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
                        eprintln!(
                            "[eval@{}] {}",
                            end,
                            self.princ_to_string(&form)
                                .chars()
                                .take(80)
                                .collect::<String>()
                        );
                    }
                    match self.eval(&form) {
                        Ok(v) => last = v,
                        // An uncaught throw is a `no-catch' error.
                        Err(Flow::Throw(tag, val)) => {
                            let nc = self.intern("no-catch");
                            return Err(self.signal_data(nc, vec![tag, val]));
                        }
                        Err(f) => {
                            if std::env::var_os("REMACS_TRACE_ERR").is_some() {
                                eprintln!(
                                    "[eval-err@{}] {} => {:?}",
                                    end,
                                    self.princ_to_string(&form)
                                        .chars()
                                        .take(120)
                                        .collect::<String>(),
                                    f
                                );
                            }
                            return Err(f);
                        }
                    }
                }
                None => return Ok(last),
            }
        }
    }

    /// `--eval` semantics: GNU's `command-line-1' reads ONE object from
    /// the argument and evaluates it; trailing forms are ignored.  GNU
    /// runs `(let ((lexical-binding t)) (eval FORM t))' — the eval is
    /// always lexical, with `lexical-binding' bound to t inside.
    pub fn eval_first_form(&mut self, src: &str) -> EvalResult {
        let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
        let form = {
            let mut reader = Reader::with_chars(self, chars);
            reader.read()?
        };
        match form {
            Some(f) => {
                let mark = self.specbind_depth();
                let lb = self.intern("lexical-binding");
                let _ = self.specbind(lb, Value::t());
                let saved_lexenv = std::mem::replace(&mut self.lexenv, lexenv_root());
                let r = self.eval(&f);
                self.lexenv = saved_lexenv;
                self.unbind_to(mark)?;
                match r {
                    Ok(v) => Ok(v),
                    Err(Flow::Throw(tag, val)) => {
                        let nc = self.intern("no-catch");
                        Err(self.signal_data(nc, vec![tag, val]))
                    }
                    Err(f) => Err(f),
                }
            }
            None => Ok(Value::Nil),
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
        if std::env::var_os("REMACS_TRACE_DEPTH").is_some()
            && self.eval_depth > 0
            && self.eval_depth % 200 == 0
        {
            eprintln!(
                "[depth={}] {}",
                self.eval_depth,
                self.princ_to_string(form).chars().take(80).collect::<String>()
            );
        }
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
                        self.specbind.iter().filter(|s| s.sym == id).count(),
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
                let fun = self.form_function(id);
                if let Value::Sym(s) = &fun {
                    if *s == sym::UNBOUND {
                        return Err(self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(id)]));
                    }
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
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    // GNU resolves defalias chains in `eval' before
                    // dispatching, so an alias to a special form
                    // (e.g. `inline' -> `progn') is called as a
                    // special form with unevaluated arguments.
                    if let Some(sf) = super::special::special_form(cur) {
                        return sf(self, args.clone());
                    }
                    let f = self.callable_function(cur);
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
                    // GNU checks a subr's arity before evaluating the
                    // argument forms, so `(car BAD EXTRA)' reports
                    // `wrong-number-of-arguments' rather than whatever
                    // BAD would signal.
                    let n = self.raw_list_length(args)?;
                    self.check_arity_subr_n(s, n, sym_name)?;
                    let argv = self.eval_args(args)?;
                    // Record the frame like apply_resolved does — GNU's
                    // specpdl holds every call, so backtraces show the
                    // innermost subr (`car(5)') too.
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    self.lisp_stack.push((shown, argv.clone()));
                    let r = (s.func)(self, argv);
                    self.lisp_stack.pop();
                    r
                }
            },
            Value::Lambda(l) => {
                // Macro: expand then eval.
                if l.is_macro {
                    let expansion = self.macro_expand_call(fun, args)?;
                    return self.eval(&expansion);
                }
                // GNU byte-compiles its dumped defuns, and compiled
                // functions check arity before argument forms are
                // evaluated (interpreted lambdas do not).  `dumped_doc'
                // marks our dumped-equivalent definitions.
                if l.dumped_doc {
                    let n = self.raw_list_length(args)?;
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    self.check_arity_lambda_n(l, n, &shown)?;
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

    /// Length of a raw (unevaluated) argument list: the number of cons
    /// cells in its spine.  A dotted tail is not counted; a circular
    /// list signals `circular-list' like GNU's `Flength'.
    fn raw_list_length(&mut self, args: &Value) -> Result<i128, Flow> {
        let mut n = 0i128;
        let mut cur = args.clone();
        let mut hare = Some(args.clone());
        loop {
            let next = match &cur {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => break,
            };
            n += 1;
            cur = next;
            // Hare advances two cells per step while it can; if it
            // meets `cur' the spine loops.
            if let Some(h) = hare.take() {
                let step = |v: &Value| match v {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                let h1 = step(&h);
                if matches!(h1, Value::Cons(_)) {
                    hare = Some(step(&h1));
                    if let (Value::Cons(a), Some(Value::Cons(b))) = (&cur, hare.as_ref()) {
                        if Rc::ptr_eq(a, b) {
                            return Err(crate::lisp::builtins::listfn::err_circular(self));
                        }
                    }
                }
            }
        }
        Ok(n)
    }

    fn check_arity_subr(
        &self,
        s: &'static super::value::Subr,
        argv: &[Value],
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        self.check_arity_subr_n(s, argv.len() as i128, name)
    }

    fn check_arity_subr_n(
        &self,
        s: &'static super::value::Subr,
        n: i128,
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        // Emacs reports the calling symbol for eval'd calls, the subr
        // object itself for `funcall'/`apply'.
        let who = name.map_or(Value::Subr(s), Value::Sym);
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

    /// Same arity test `call_lambda' performs, but against the raw
    /// argument count — used so dumped (GNU-compiled-equivalent)
    /// functions reject a bad call before arguments are evaluated.
    fn check_arity_lambda_n(&self, l: &Lambda, n: i128, who: &Value) -> Result<(), Flow> {
        let min = l.required.len() as i128;
        if n < min || (l.rest.is_none() && n > min + l.optional.len() as i128) {
            return Err(self.wrong_number_of_args(who, n));
        }
        Ok(())
    }

    /// `apply`/`funcall`: call `fun` with already-evaluated `argv`.
    pub fn apply(&mut self, fun: &Value, argv: Vec<Value>) -> EvalResult {
        match fun {
            Value::Sym(id) => {
                let mut cur = *id;
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    let f = self.callable_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            cur = next;
                        }
                        other => return self.apply_resolved(&other, argv, Value::Sym(*id)),
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
                self.princ_to_string(&shown)
                    .chars()
                    .take(90)
                    .collect::<String>()
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

    /// Synthesize `(lambda (&rest a) (apply SUBR IDX a))'.
    /// The dispatcher subr is embedded directly rather than looked up
    /// through `cl--advice--apply' — GNU keeps that name void at -Q.
    fn advice_trampoline(&mut self, idx: usize) -> EvalResult {
        let fn_sym = self.intern("cl--advice--apply");
        let prop = self.intern("cl--advice--apply--fn");
        let mut subr = self.get_prop(fn_sym, prop);
        if !matches!(subr, Value::Subr(_)) {
            // Before the -Q voiding pass the dispatcher still lives in
            // the public function cell (e.g. add-function calls while
            // loading dumped libraries).
            subr = self.symbol_function(fn_sym);
        }
        if !matches!(subr, Value::Subr(_)) {
            return Err(self.error("internal: advice dispatcher missing"));
        }
        let cl_args = self.intern("cl--args");
        let form = Value::list(vec![
            Value::Sym(self.intern("lambda")),
            Value::list(vec![Value::Sym(self.intern("&rest")), Value::Sym(cl_args)]),
            Value::list(vec![
                Value::Sym(self.intern("apply")),
                subr,
                Value::Int(idx as i128),
                Value::Sym(cl_args),
            ]),
        ]);
        let mut lam = self.lambda_from_form(&form, None)?;
        lam.advice_link = Some(idx);
        Ok(Value::Lambda(Rc::new(lam)))
    }

    /// Compose KEY's advice entries into the function-cell value: each
    /// entry wraps the previous layer, most recently added outermost —
    /// like GNU's nested `advice' oclosures.  The innermost `next' is
    /// KEY's `advice_bases' entry.  A `(macro . X)' base composes inside
    /// the macro wrapper so the advice runs at expansion, as in GNU.
    pub(crate) fn compose_advice(&mut self, key: SymId) -> EvalResult {
        let base = self
            .advice_bases
            .iter()
            .find(|(s, _)| *s == key)
            .map(|(_, b)| b.clone())
            .unwrap_or_else(|| self.symbol_function(key));
        let mut next = base;
        let mut macrop = false;
        match &next {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    macrop = true;
                    next = cdr;
                }
            }
            Value::Lambda(l) if l.is_macro => macrop = true,
            _ => {}
        }
        for (w, f, n) in self.advice_list(key) {
            let idx = self.advice_links.len();
            self.advice_links.push((Value::Sym(w), f, next, n));
            next = self.advice_trampoline(idx)?;
        }
        if macrop {
            next = Value::cons(Value::Sym(sym::MACRO), next);
        }
        Ok(next)
    }

    /// Recompute KEY's function cell after its advice entries changed:
    /// empty → restore the captured base; otherwise → recomposed chain.
    pub(crate) fn recompose_advice(&mut self, key: SymId) -> Result<(), Flow> {
        if self
            .advices
            .iter()
            .find(|(s, _)| *s == key)
            .map(|(_, a)| a.is_empty())
            .unwrap_or(true)
        {
            if let Some(pos) = self.advice_bases.iter().position(|(s, _)| *s == key) {
                let base = self.advice_bases.remove(pos).1;
                self.fset(key, base);
            }
            return Ok(());
        }
        let composed = self.compose_advice(key)?;
        self.fset(key, composed);
        Ok(())
    }

    /// If V is an advice trampoline, its `(WHERE FUN NEXT NAME)'.
    pub fn advice_link_entry(&self, v: &Value) -> Option<(Value, Value, Value, Value)> {
        if let Value::Lambda(l) = v {
            if let Some(idx) = l.advice_link {
                return self.advice_links.get(idx).cloned();
            }
        }
        None
    }

    /// Peel advice layers: innermost `next' of a trampoline chain —
    /// the base definition GNU reaches via `advice--cd*r'.
    pub fn advice_base_value(&self, v: &Value) -> Value {
        let mut cur = v.clone();
        while let Some((_, _, next, _)) = self.advice_link_entry(&cur) {
            cur = next;
        }
        cur
    }

    /// `defalias'-level write to SYM's function cell: when SYM carries
    /// advice, the new definition substitutes the chain's base (GNU's
    /// `advice--defalias-fset'); otherwise a plain `fset'.
    pub fn fset_defalias(&mut self, id: SymId, def: Value) -> Result<(), Flow> {
        if self.advice_bases.iter().any(|(s, _)| *s == id) {
            if let Some(pos) = self.advice_bases.iter().position(|(s, _)| *s == id) {
                self.advice_bases[pos].1 = def;
            }
            self.recompose_advice(id)
        } else {
            self.fset(id, def);
            Ok(())
        }
    }

    /// Call an interpreted lambda with evaluated args.  `shown' is what
    /// `wrong-number-of-arguments' reports as the function — GNU prints
    /// the called symbol when invoked by name.
    fn call_lambda(&mut self, l: &Rc<Lambda>, argv: Vec<Value>, shown: &Value) -> EvalResult {
        if l.dumped_doc {
            self.dumped_call_depth += 1;
        }
        let r = self.call_lambda_inner(l, argv, shown);
        if l.dumped_doc {
            self.dumped_call_depth -= 1;
        }
        r
    }

    fn call_lambda_inner(&mut self, l: &Rc<Lambda>, argv: Vec<Value>, shown: &Value) -> EvalResult {
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
            // Lexical closure: extend captured env.  GNU's
            // `funcall_lambda' binds every parameter lexically — even
            // `defvar'd specials and scoped-declared names — since
            // `internal-interpreter-environment' is not consulted here.
            let vars = RefCell::new(HashMap::new());
            let mark = self.specbind_depth();
            let bind_result = self.bind_lambda_args_lexical(&l.clone(), &argv, &vars);
            // `&optional` defaults may need evaluation in the new env;
            // evaluate them after the frame exists.
            let frame = Rc::new(LexFrame {
                vars,
                declared: RefCell::new(HashSet::new()),
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

    /// Lexical-closure argument binding: every param goes into the new
    /// frame's `vars' — like GNU's `funcall_lambda', which pushes
    /// `(param . arg)' pairs unconditionally when the closure is lexical.
    fn bind_lambda_args_lexical(
        &mut self,
        l: &Rc<Lambda>,
        argv: &[Value],
        vars: &RefCell<HashMap<SymId, Value>>,
    ) -> Result<(), Flow> {
        let bind = |i: &mut Self, sym: SymId, val: Value| -> Result<(), Flow> {
            // Like `let': `defvar'd specials specbind dynamically even
            // in lexical functions (GNU `funcall_lambda').
            if i.obarray.symbol(sym).special {
                i.specbind(sym, val)
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
                self.prin1_to_string(mac)
                    .chars()
                    .take(90)
                    .collect::<String>(),
                self.prin1_to_string(args)
                    .chars()
                    .take(90)
                    .collect::<String>()
            );
        }
        let argv = match args.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.error("bad macro args")),
        };
        self.macroexp_call_depth += 1;
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) if l.is_macro => self.call_lambda(l, argv, mac),
            Value::Lambda(l) => self.call_lambda(l, argv, mac),
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
                    self.macroexp_call_depth -= 1;
                    return self.macro_expand_call(&cdr, args);
                }
                self.apply(mac, argv)
            }
            _ => self.apply(mac, argv),
        };
        self.macroexp_call_depth -= 1;
        result
    }

    /// `macroexpand`: repeatedly expand while the form is a macro call.
    pub fn macroexpand(&mut self, form: &Value) -> EvalResult {
        let mut cur = form.clone();
        let mut iters = 0;
        loop {
            iters += 1;
            if std::env::var("PRELUDE_TRACE").is_ok() && iters > 500 {
                eprintln!(
                    "macroexpand iter {iters}: {}",
                    self.prin1_to_string(&cur)
                        .chars()
                        .take(200)
                        .collect::<String>()
                );
            }
            let next = match &cur {
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match car {
                        Value::Sym(id) => {
                            // GNU `macroexpand-1' consults the ENVIRONMENT
                            // argument first; here that is the dynamically
                            // bound `macroexpand-all-environment'.  An entry
                            // (SYM . DEF) shadows SYM's global definition:
                            // a nil DEF stops expansion, otherwise DEF is
                            // applied to the form's argument list (this is
                            // how GNU `cl-flet'/`rx-let' rewrite calls).
                            let env_id = self.intern("macroexpand-all-environment");
                            let env = self.symbol_value(env_id);
                            let mut env_hit = false;
                            let mut env_def = Value::Nil;
                            let mut tail = env;
                            loop {
                                match tail {
                                    Value::Cons(cc) => {
                                        let (a, d) = {
                                            let b = cc.borrow();
                                            (b.car.clone(), b.cdr.clone())
                                        };
                                        if let Value::Cons(e) = &a {
                                            let (ek, ev) = {
                                                let b = e.borrow();
                                                (b.car.clone(), b.cdr.clone())
                                            };
                                            if let Value::Sym(eid) = ek {
                                                if eid == id {
                                                    env_hit = true;
                                                    env_def = ev;
                                                }
                                            }
                                        }
                                        tail = d;
                                    }
                                    _ => break,
                                }
                            }
                            if env_hit {
                                if env_def.is_nil() {
                                    return Ok(cur);
                                }
                                let argl = crate::lisp::builtins::want_list(self, &cdr)?;
                                let new = self.apply(&env_def, argl)?;
                                // GNU macroexpand-1 stops when the expander
                                // returns the identical object (the
                                // `cl--labels-convert' cache relies on it).
                                if crate::lisp::builtins::eq_values(&new, &cur) {
                                    return Ok(cur);
                                }
                                cur = new;
                                continue;
                            }
                            let mut f = self.form_function(id);
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
        // `&body' is GNU's `&rest' synonym, legal in macro arglists
        // (defmacro/cl-defmacro) — treat it identically here.
        let body_kw = self.intern("&body");
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
                        Some(s) if s == sym::REST || s == body_kw => {
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
                // A dotted tail `(a . b)' names the rest parameter —
                // GNU accepts `(lambda a ...)' and `(lambda (a . b) ...)'
                // alike, binding the full/remaining arg list to it.
                Value::Sym(s) => {
                    if rest.is_none() {
                        rest = Some(s);
                        break;
                    }
                    return Err(self.error("multiple &rest parameters"));
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
            dumped_doc: self.loading_dumped,
            advice_link: None,
            bc_items: None,
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
        put(self, "unknown-image-type", &["unknown-image-type", "error"]);
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
        put(self, "file-locked", &["file-locked", "file-error", "error"]);
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
            "inhibit-modification-hooks",
            "inhibit-quit",
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
            "last-command-event",
            "current-prefix-arg",
            "prefix-arg",
            "minibuffer-history",
            "read-hide-char",
            "yes-or-no-prompt",
            "use-short-answers",
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
            "activate-mark-hook",
            "ad-default-compilation-action",
            "adaptive-fill-first-line-regexp",
            "add-log-full-name",
            "add-log-mailing-address",
            "after-load-functions",
            "after-make-frame-functions",
            "auto-coding-alist",
            "auto-coding-functions",
            "auto-coding-regexp-alist",
            "auto-hscroll-mode",
            "auto-save-include-big-deletions",
            "auto-save-list-file-prefix",
            "auto-save-visited-file-name",
            "baud-rate",
            "before-make-frame-hook",
            "blink-matching-delay",
            "blink-matching-paren",
            "buffer-access-fontify-functions",
            "buffer-display-time",
            "char-code-property-alist",
            "charset-list",
            "charset-map-path",
            "coding-category-list",
            "coding-system-alist",
            "coding-system-list",
            "command-error-function",
            "command-line-default-directory",
            "command-line-functions",
            "command-line-processed",
            "command-switch-alist",
            "completions-detailed",
            "confirm-nonexistent-file-or-buffer",
            "ctl-arrow",
            "current-language-environment",
            "cursor-in-echo-area",
            "deactivate-mark-hook",
            "debug-ignored-errors",
            "debug-on-quit",
            "debugger",
            "default-file-name-coding-system",
            "default-frame-alist",
            "default-input-method",
            "default-justification",
            "default-process-coding-system",
            "default-terminal-coding-system",
            "delete-frame-functions",
            "delete-trailing-lines",
            "directory-free-space-args",
            "directory-free-space-program",
            "dired-directory",
            "dired-kept-versions",
            "dynamic-library-alist",
            "emacs-basic-display",
            "emacs-build-system",
            "emacs-build-time",
            "emacs-copyright",
            "emacs-repository-branch",
            "emacs-repository-version",
            "eval-expression-debug-on-error",
            "eval-expression-print-maximum-character",
            "exec-suffixes",
            "fancy-about-text",
            "fancy-splash-image",
            "fancy-startup-text",
            "fast-but-imprecise-scrolling",
            "file-coding-system-alist",
            "file-name-coding-system",
            "file-precious-flag",
            "find-file-existing-other-name",
            "find-file-visit-truename",
            "fontification-functions",
            "frame-inherited-parameters",
            "frame-initial-frame",
            "frame-initial-geometry-arguments",
            "frame-title-format",
            "garbage-collection-messages",
            "gc-cons-percentage",
            "glyph-table",
            "hscroll-margin",
            "hscroll-step",
            "icon-title-format",
            "idle-update-delay",
            "image-scaling-factor",
            "inhibit-changing-match-data",
            "inhibit-default-init",
            "inhibit-startup-buffer-menu",
            "inhibit-startup-echo-area-message",
            "init-file-debug",
            "init-file-user",
            "initial-buffer-choice",
            "initial-frame-alist",
            "initial-window-system",
            "input-method-activate-hook",
            "input-method-function",
            "input-method-highlight-flag",
            "input-method-use-echo-area",
            "input-method-verbose-flag",
            "insert-default-directory",
            "installation-directory",
            "internal-make-interpreted-closure-function",
            "isearch-allow-prefix",
            "isearch-allow-scroll",
            "isearch-case-fold-search",
            "isearch-hide-immediately",
            "isearch-lazy-highlight",
            "isearch-resume-in-command-history",
            "kbd-macro-termination-hook",
            "keyboard-coding-system",
            "kill-do-not-save-duplicates",
            "kill-transform-function",
            "large-file-warning-threshold",
            "lazy-highlight-cleanup",
            "lazy-highlight-initial-delay",
            "lazy-highlight-interval",
            "lazy-highlight-max-at-a-time",
            "lazy-highlight-no-delay-length",
            "load-dangerous-libraries",
            "load-force-doc-strings",
            "load-prefer-newer",
            "load-read-function",
            "locale-coding-system",
            "macroexp--debug-eager",
            "macroexpand-all-environment",
            "mail-host-address",
            "max-image-size",
            "menu-bar-final-items",
            "menu-bar-select-buffer-function",
            "menu-bar-update-hook",
            "message-log-max",
            "minibuffer-exit-hook",
            "minibuffer-frame-alist",
            "minibuffer-help-form",
            "minibuffer-setup-hook",
            "mode-require-final-newline",
            "module-file-suffix",
            "mouse-1-click-follows-link",
            "mouse-1-click-in-non-selected-windows",
            "mouse-autoselect-window",
            "mouse-drag-copy-region",
            "mouse-highlight",
            "mouse-position-function",
            "multiple-frames",
            "network-coding-system-alist",
            "no-redraw-on-reenter",
            "normal-erase-is-backspace",
            "pixel-scroll-mode",
            "pixel-scroll-precision-mode",
            "polling-period",
            "post-gc-hook",
            "print-charset-text-property",
            "print-escape-control-characters",
            "print-escape-multibyte",
            "print-escape-nonascii",
            "print-integers-as-characters",
            "print-unreadable-function",
            "process-adaptive-read-buffering",
            "process-coding-system-alist",
            "process-error-pause-time",
            "query-replace-from-to-separator",
            "query-replace-highlight",
            "query-replace-lazy-highlight",
            "query-replace-show-replacement",
            "query-replace-skip-read-only",
            "read-expression-map",
            "recenter-positions",
            "recenter-redisplay",
            "regexp-search-ring-yank-pointer",
            "remote-file-name-inhibit-cache",
            "ring-bell-function",
            "scroll-bar-adjust-thumb-portion",
            "scroll-minibuffer-conservatively",
            "search-exit-option",
            "search-nonincremental-instead",
            "search-ring-yank-pointer",
            "search-slow-speed",
            "search-slow-window-lines",
            "selection-coding-system",
            "send-mail-function",
            "set-auto-coding-function",
            "shell-command-default-error-buffer",
            "shell-command-switch",
            "show-help-function",
            "site-run-file",
            "source-directory",
            "tab-always-indent",
            "tab-bar-mode",
            "term-setup-hook",
            "translation-table-for-input",
            "unicode-category-table",
            "unread-input-method-events",
            "use-file-dialog",
            "user-emacs-directory",
            "user-real-login-name",
            "vc-handled-backends",
            "view-read-only",
            "visible-cursor",
            "window-configuration-change-hook",
            "window-scroll-functions",
            "window-selection-change-functions",
            "window-setup-hook",
            "window-size-change-functions",
            "write-contents-hooks",
            "write-region-annotate-functions",
            "write-region-annotations-so-far",
            "write-region-inhibit-fsync",
            "write-region-post-annotation-function",
            "x-stretch-cursor",
            "x-underline-at-descent-line",
            "x-use-underline-position-properties",
            "yank-pop-change-selection",
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

        // The `obarray' variable's record; its Rc identity is stored on
        // the interpreter so primitives can tell the default symbol
        // table from a custom `obarray-make' record.
        let default_obarray_rec = std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Sym(self.intern("obarray")),
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
                Value::Nil;
                4
            ]))),
        ]));
        self.default_obarray = Some(default_obarray_rec.clone());

        // Initial values.
        let defs: &[(&str, Value)] = &[
            ("emacs-major-version", Value::Int(31)),
            ("emacs-minor-version", Value::Int(1)),
            ("emacs-version", Value::string("31.1.0 (remacs)")),
            // GNU's NS build binds this in nsterm.m (AppKit version);
            // `emacs-version' reads it under (featurep 'ns).
            (
                "ns-version-string",
                Value::string("appkit-2685.60 Version 26.5.2 (Build 25F84)"),
            ),
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
            // GNU's Vgc_elapsed/Vgcs_done (alloc.c): GC statistics
            // variables.  Remacs has no tracing GC, so they stay at 0 —
            // `benchmark-run' reads them.
            ("gc-elapsed", Value::float(0.0)),
            ("gcs-done", Value::Int(0)),
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
                    .map(|(re, ty)| Value::cons(Value::string(*re), Value::Sym(self.intern(ty))))
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
            ("read-hide-char", Value::Nil),
            ("read-expression-history", Value::Nil),
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
            ("mode-name", Value::Nil),
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
            (
                "normal-auto-fill-function",
                Value::Sym(self.intern("do-auto-fill")),
            ),
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
            ("search-whitespace-regexp", Value::string("[ \t]+")),
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
            ("yes-or-no-prompt", Value::string("(yes or no) ")),
            ("async-shell-command-display-buffer", Value::Sym(sym::T)),
            (
                "shell-command-default-error-buffer",
                Value::string("*Shell Command Error*"),
            ),
            ("shell-command-prompt-show-cwd", Value::Sym(sym::T)),
            (
                "exec-suffixes",
                // GNU: '("") on non-DOS/Windows systems.
                Value::list(vec![Value::string("")]),
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
            ("internal--daemon-sockname", Value::Nil),
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
            (
                "char-code-property-alist",
                self.seed_char_code_property_alist(),
            ),
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
            ("scroll-bar-mode", Value::Sym(self.intern("right"))),
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
            ("scroll-bar-adjust-thumb-portion", Value::Sym(sym::T)),
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

    /// `char-code-property-alist': GNU dumps 20 entries at build time —
    /// each property symbol consed onto its char-table, except `name'
    /// which lazily loads from "uni-name.el".  Tables carry GNU's three
    /// extra slots: (PROP MAPPER INDEX), where MAPPER is a unidata-gen
    /// function for computed properties (we substitute `identity',
    /// which satisfies functionp/fboundp like GNU's opaque bytecode)
    /// and INDEX a small unidata table index or nil.
    pub fn seed_char_code_property_alist(&mut self) -> Value {
        use crate::lisp::builtins::misc::make_ct;
        // (prop slot1 slot2) in GNU's dumped order; "fn" = mapper fn.
        let specs: &[(&str, &str, &str)] = &[
            ("bracket-type", "0", "1"),
            ("paired-bracket", "nil", "0"),
            ("special-titlecase", "nil", "nil"),
            ("special-lowercase", "nil", "nil"),
            ("special-uppercase", "nil", "nil"),
            ("titlecase", "nil", "0"),
            ("lowercase", "nil", "0"),
            ("uppercase", "nil", "0"),
            ("iso-10646-comment", "fn", "fn"),
            ("old-name", "fn", "fn"),
            ("mirroring", "nil", "0"),
            ("mirrored", "0", "1"),
            ("numeric-value", "0", "2"),
            ("digit-value", "0", "1"),
            ("decimal-digit-value", "0", "1"),
            ("decomposition", "fn", "fn"),
            ("bidi-class", "0", "1"),
            ("canonical-combining-class", "0", "1"),
            ("general-category", "0", "1"),
            ("name", "fn", "fn"),
        ];
        let tag = Value::Sym(self.intern("char-code-property-table"));
        let slot = |i: &mut Self, tok: &str| match tok {
            "fn" => Value::Sym(i.intern("identity")),
            "nil" => Value::Nil,
            n => Value::Int(n.parse::<i128>().unwrap()),
        };
        let mut alist = Vec::with_capacity(specs.len());
        for &(prop, s1, s2) in specs {
            let psym = Value::Sym(self.intern(prop));
            if prop == "name" {
                alist.push(Value::cons(psym, Value::string("uni-name.el")));
                continue;
            }
            let extras = vec![psym.clone(), slot(self, s1), slot(self, s2)];
            let t = make_ct(self, tag.clone(), Value::Nil, extras);
            self.char_code_prop_tables
                .push((prop.to_string(), t.clone()));
            alist.push(Value::cons(psym, t));
        }
        Value::list(alist)
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
                let bid = b.borrow().id;
                crate::buffer::primitives::chg_with_buffer(self, bid, |i| {
                    crate::buffer::primitives::chg_insert_pt(i, s, false)
                })?;
            }
            Value::Marker(m) => {
                // GNU inserts before the marker, then advances it past
                // the inserted text.
                let mm = m.borrow();
                if let Some(buf_id) = mm.buffer {
                    let pos = mm.position;
                    drop(mm);
                    if self.buffers.get(buf_id).is_some() {
                        crate::buffer::primitives::chg_with_buffer(self, buf_id, |i| {
                            crate::buffer::primitives::chg_insert(i, pos, s)
                        })?;
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
            Value::Cons(ref c)
                if {
                    let cb = c.borrow();
                    matches!(&cb.car, Value::Sym(s) if *s == self.intern("lambda") || *s == self.intern("closure"))
                } =>
            {
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
        if let Some(mb) = self.buffers.by_name("*Messages*") {
            if self.buffers.get(mb).is_some() {
                let _ = crate::buffer::primitives::chg_with_buffer(self, mb, |i| {
                    crate::buffer::primitives::chg_insert_pt(i, &format!("{}\n", s), false)
                });
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

    /// Buffer text after the prompt field of the active minibuffer —
    /// what a finished `read-from-minibuffer' returns.
    pub fn minibuf_contents(&self) -> String {
        let Some(b) = self.buffers.get(self.current_buffer) else {
            return String::new();
        };
        let bb = b.borrow();
        let end = bb.text_len();
        let start = self
            .minibuf_prompts
            .last()
            .map(|p| p.chars().count())
            .unwrap_or(0)
            .min(end);
        bb.text.substring(start, end)
    }

    /// GNU `get_minibuffer' (minibuf.c): the ` *Minibuf-{depth}*'
    /// buffer for DEPTH, (re)creating dead or missing entries.
    /// Depth 0 is ` *Minibuf-0*', the never-active null minibuffer.
    pub fn get_minibuffer(&mut self, depth: usize) -> usize {
        while self.minibuf_list.len() <= depth {
            self.minibuf_list.push(usize::MAX);
        }
        let mut id = self.minibuf_list[depth];
        if id == usize::MAX || !self.buffer_live(id) {
            let name = format!(" *Minibuf-{depth}*");
            id = self
                .buffers
                .by_name(&name)
                .unwrap_or_else(|| self.buffers.create(&name));
            self.minibuf_list[depth] = id;
        }
        id
    }

    /// Is `id` a real minibuffer (on `Vminibuffer_list')?
    pub fn is_minibuffer(&self, id: usize) -> bool {
        self.minibuf_list.iter().any(|&x| x == id)
    }

    /// Point `buf`'s frame minibuffer window at buffer `id`
    /// (GNU `set_window_buffer (minibuf_window, buf)').
    pub fn set_minibuf_window_buffer(&mut self, id: usize) {
        if let Some(f) = &self.selected_frame {
            if let Some(w) = &f.borrow().minibuffer {
                w.borrow_mut().buffer = id;
            }
        }
    }

    /// GNU `read_minibuf' (minibuf.c), interactive path only:
    /// switch to ` *Minibuf-{depth}*', install the prompt with its
    /// `field'/`front-sticky'/`rear-nonsticky' and
    /// `minibuffer-prompt-properties', run `minibuffer-setup-hook',
    /// read input, then run `minibuffer-exit-hook' (safe) and unwind.
    /// GNU runs the setup hook via `run_hook', so an error in it
    /// aborts the read — but the exit hook and the state restoration
    /// still run, exactly like the C unwind-protect chain.
    pub fn minibuf_read(&mut self, prompt: &str, args: MinibufArgs) -> Result<String, Flow> {
        // Fread_from_minibuffer's HIST handling: symbol → (sym . 0),
        // cons → (HISTVAR . HISTPOS), nil HISTVAR → `minibuffer-history'.
        let (histvar, histpos) = match &args.hist {
            Value::Cons(c) => {
                let (hv, hp) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.int().unwrap_or(0))
                };
                (hv, hp)
            }
            v => (v.clone(), 0),
        };
        let histvar = if histvar.is_nil() {
            Value::Sym(self.intern("minibuffer-history"))
        } else {
            histvar
        };

        // GNU: `enable-recursive-minibuffers' nil + already inside a
        // minibuffer → `user-error' in the minibuffer window, else
        // `throw 'exit' with the message.
        if self.minibuf_level > 0 {
            let erm = self
                .intern_soft("enable-recursive-minibuffers")
                .map(|id| self.symbol_value(id).truthy())
                .unwrap_or(false);
            if !erm {
                let msg = "Command attempted to use minibuffer while in minibuffer";
                if self.is_minibuffer(self.current_buffer) {
                    let ue = self.intern("user-error");
                    return Err(self.signal_data(ue, vec![Value::string(msg)]));
                }
                return Err(Flow::Throw(
                    Value::Sym(self.intern("exit")),
                    Value::string(msg),
                ));
            }
        }

        let mark = self.specbind_depth();
        // specbind minibuffer-default ← DEFALT, inhibit-read-only ← nil.
        let mdef = self.intern("minibuffer-default");
        let _ = self.specbind(mdef, args.defalt.clone());
        let iro = self.intern("inhibit-read-only");
        let _ = self.specbind(iro, Value::Nil);
        // specbind minibuffer-completing-file-name ← current value;
        // a `lambda' on entry (t from an outer read) normalizes to nil.
        let cf = self.intern("minibuffer-completing-file-name");
        let cfv = self.symbol_value(cf);
        let _ = self.specbind(cf, cfv.clone());
        if matches!(cfv, Value::Sym(s) if self.symbol_name(s) == "lambda") {
            let _ = self.set_symbol(cf, Value::Nil);
        }

        // GNU creates the depth-N minibuffer before bumping the level.
        let depth = self.minibuf_level.max(0) as usize + 1;
        let mb_id = self.get_minibuffer(depth);
        self.minibuf_level += 1;
        let saved_buf = self.current_buffer;

        // minibuf_save_list vars (per-level dynamic state).
        let mhp = self.intern("minibuffer-history-position");
        let _ = self.specbind(mhp, Value::Int(histpos));
        let mhv = self.intern("minibuffer-history-variable");
        let _ = self.specbind(mhv, histvar.clone());
        let hform = self
            .intern_soft("minibuffer-help-form")
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil);
        let hf = self.intern("help-form");
        let _ = self.specbind(hf, hform);
        let cpa = self.intern("current-prefix-arg");
        let cpav = self.symbol_value(cpa);
        let _ = self.specbind(cpa, cpav);

        // Non-nil completing-file-name becomes `lambda' for this read
        // ("t here, nil for nested reads").
        if self.symbol_value(cf).truthy() {
            let lam = Value::Sym(self.intern("lambda"));
            let _ = self.set_symbol(cf, lam);
        }
        // GNU: an unbound history variable is set to nil on entry.
        if let Value::Sym(hsid) = &histvar {
            if matches!(self.obarray.symbol(*hsid).value, Value::Sym(s) if s == sym::UNBOUND)
                && !self.obarray.symbol(*hsid).constant
            {
                let _ = self.set_symbol(*hsid, Value::Nil);
            }
        }

        self.minibuf_prompts.push(prompt.to_string());

        // ---- enter the minibuffer ----
        self.set_current_buffer(mb_id);
        // GNU set_minibuffer_mode: `minibuffer-mode' if fbound (it
        // resets locals and runs its mode hook).
        let mm = self.intern("minibuffer-mode");
        let mode_r = if self.fbound_p(mm) {
            self.apply(&Value::Sym(mm), vec![])
        } else {
            Ok(Value::Nil)
        };
        // bset_truncate_lines (current_buffer, Qnil).
        let tl = self.intern("truncate-lines");
        if let Some(b) = self.buffers.get(mb_id) {
            b.borrow_mut().locals.insert(tl, Value::Nil);
        }
        // Display the minibuffer in the mini window.
        self.set_minibuf_window_buffer(mb_id);
        // GNU `Fselect_window (minibuf_window, Qnil)': the read runs
        // with the minibuffer window selected (window-minibuffer-p,
        // minibuffer-selected-window & friends observe it).
        let saved_window = self
            .selected_frame
            .as_ref()
            .map(|f| f.borrow().selected.clone());
        if let Some(f) = &self.selected_frame {
            let mbw = f.borrow().minibuffer.clone();
            if let Some(w) = mbw {
                f.borrow_mut().selected = w;
            }
        }

        // Erase, insert prompt + initial input — GNU binds
        // inhibit-read-only and inhibit-modification-hooks around it.
        let input_r = mode_r.and_then(|_| {
            let m2 = self.specbind_depth();
            let iro = self.intern("inhibit-read-only");
            let imh = self.intern("inhibit-modification-hooks");
            let _ = self.specbind(iro, Value::t());
            let _ = self.specbind(imh, Value::t());
            let r = self.minibuf_install(prompt, &args.initial, mb_id);
            let _ = self.unbind_to(m2);
            r
        });

        // Local keymap: KEYMAP arg or `minibuffer-local-map' (GNU
        // substitutes it for nil before bset_keymap).
        let input_r = input_r.and_then(|_| {
            let map = if args.keymap.is_nil() {
                self.intern_soft("minibuffer-local-map")
                    .map(|id| self.symbol_value(id))
                    .unwrap_or(Value::Nil)
            } else {
                args.keymap.clone()
            };
            let km = self.intern("local-keymap");
            if let Some(b) = self.buffers.get(mb_id) {
                b.borrow_mut().locals.insert(km, map);
            }
            // GNU: no undo past this point.
            if let Some(b) = self.buffers.get(mb_id) {
                b.borrow_mut().set_undo_list(Value::Nil);
            }
            // `minibuffer-setup-hook' — GNU `run_hook' (not safe):
            // an error aborts the read after unwinding.
            match crate::lisp::builtins::evalfn::call_hook(self, "minibuffer-setup-hook") {
                Ok(_) => {
                    // GNU wraps the command loop in
                    // `internal_catch (Qexit, ...)': `exit-minibuffer'
                    // throws `exit', ending the read.  The front-end
                    // loop reports the throw via `minibuf_exited'
                    // (a raw Flow::Throw can't cross its io boundary).
                    let exit_sym = Value::Sym(self.intern("exit"));
                    self.catch_tags.push(exit_sym.clone());
                    let prev_catching = self.minibuf_catching_exit;
                    self.minibuf_catching_exit = true;
                    self.minibuf_exited = false;
                    let r = self.minibuf_input(prompt, false);
                    self.minibuf_catching_exit = prev_catching;
                    self.catch_tags.pop();
                    match r {
                        // An `exit' throw that bypassed the front-end
                        // flag (test readers, throws from Lisp): GNU
                        // signals `error' for a thrown string, else
                        // ends the read with the buffer contents.
                        Err(Flow::Throw(tag, v)) if crate::lisp::eq_values(&tag, &exit_sym) => {
                            match &v {
                                Value::Str(_) => {
                                    let es = Value::Sym(self.intern("error"));
                                    Err(Flow::Signal(es, Value::list(vec![v.clone()]), false))
                                }
                                Value::Sym(_) if v.truthy() => Err(Flow::Quit),
                                _ => Ok(MinibufInput::Text(self.minibuf_contents())),
                            }
                        }
                        // GNU `recursive_edit_1' calls a function
                        // thrown through `exit' before unbinding —
                        // `minibuffer-quit-recursive-edit' throws a
                        // lambda that signals `minibuffer-quit'.
                        Ok(MinibufInput::Call(f)) => match self.apply(&f, vec![]) {
                            Ok(_) => Ok(MinibufInput::Text(self.minibuf_contents())),
                            Err(e) => Err(e),
                        },
                        other => other,
                    }
                }
                Err(f) => Err(f),
            }
        });

        // ---- unwind: run_exit_minibuf_hook, then read_minibuf_unwind.
        if self.buffer_live(mb_id) {
            self.set_current_buffer(mb_id);
        }
        let hook_r = crate::lisp::builtins::evalfn::safe_call_hook(self, "minibuffer-exit-hook");
        self.minibuf_level -= 1;
        self.minibuf_prompts.pop();
        let _ = self.unbind_to(mark);
        // GNU calls `minibuffer-inactive-mode' in the expired
        // minibuffer after restoring the per-level vars.
        if self.buffer_live(mb_id) {
            self.set_current_buffer(mb_id);
            let im = self.intern("minibuffer-inactive-mode");
            if self.fbound_p(im) {
                let _ = self.apply(&Value::Sym(im), vec![]);
            }
        }
        // The mini window shows the next-less-nested (null) minibuffer.
        let idle = self.get_minibuffer(0);
        self.set_minibuf_window_buffer(idle);
        // GNU's window-configuration unwind reselects the prior window.
        if let (Some(f), Some(sw)) = (&self.selected_frame, saved_window) {
            f.borrow_mut().selected = sw;
        }
        if self.buffer_live(saved_buf) {
            self.set_current_buffer(saved_buf);
        }
        if let Err(f) = hook_r {
            return Err(f);
        }

        let input = input_r?;

        // History push — after restoring the calling buffer (GNU:
        // "in case the history variable is buffer-local").
        if let MinibufInput::Text(text) = &input {
            let han = self
                .intern_soft("history-add-new-input")
                .map(|id| self.symbol_value(id).truthy())
                .unwrap_or(true);
            let histstring = if !text.is_empty() {
                Some(text.clone())
            } else {
                match &args.defalt {
                    Value::Str(s) => Some(s.borrow().clone()),
                    Value::Cons(c) => {
                        let h = c.borrow().car.clone();
                        if let Value::Str(s) = h {
                            Some(s.borrow().clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };
            if han && histstring.is_some() {
                let ah = self.intern("add-to-history");
                if self.fbound_p(ah) {
                    let _ = self.apply(
                        &Value::Sym(ah),
                        vec![histvar.clone(), Value::string(histstring.unwrap())],
                    );
                }
            }
        }

        match input {
            MinibufInput::Text(t) => Ok(t),
            MinibufInput::Key(k) => Ok(char::from_u32(k as u32)
                .map(|c| c.to_string())
                .unwrap_or_default()),
            // Handled in the input phase (applied before unwinding);
            // unreachable here.
            MinibufInput::Call(_) => Ok(String::new()),
        }
    }

    /// Read a full input line via the front-end hook.
    pub fn minibuf_line(&mut self, prompt: &str) -> Result<String, Flow> {
        self.minibuf_read(prompt, MinibufArgs::default())
    }

    /// The minibuffer-enter half of GNU `read_minibuf': erase the
    /// buffer, insert the prompt with `field'/`front-sticky'/
    /// `rear-nonsticky' plus `minibuffer-prompt-properties' (faces
    /// appended, not overwriting), then insert INITIAL-CONTENTS and
    /// position point.
    fn minibuf_install(&mut self, prompt: &str, initial: &Value, mb_id: usize) -> EvalResult {
        if let Some(b) = self.buffers.get(mb_id) {
            let mut bb = b.borrow_mut();
            bb.overlays.clear();
            let len = bb.text_len();
            if len > 0 {
                bb.delete_region(0, len);
            }
            bb.set_point(0);
            if !prompt.is_empty() {
                bb.insert(prompt);
            }
        }
        let pend = prompt.chars().count();
        // front-sticky t, rear-nonsticky t, field t over the prompt.
        if pend > 0 {
            for (key, val) in [
                ("front-sticky", Value::t()),
                ("rear-nonsticky", Value::t()),
                ("field", Value::t()),
            ] {
                let ptp = self.intern("put-text-property");
                let key_sym = self.intern(key);
                let _ = self.apply(
                    &Value::Sym(ptp),
                    vec![
                        Value::Int(1),
                        Value::Int(pend as i128 + 1),
                        Value::Sym(key_sym),
                        val,
                    ],
                );
            }
            // minibuffer-prompt-properties plist: `face' is appended
            // via add-face-text-property, others via put-text-property.
            let mpp = self
                .intern_soft("minibuffer-prompt-properties")
                .map(|id| self.symbol_value(id))
                .unwrap_or(Value::Nil);
            if let Value::Cons(_) = &mpp {
                let mut list = mpp.clone();
                while let Value::Cons(c) = list {
                    let (key, rest) = {
                        let cc = c.borrow();
                        (cc.car.clone(), cc.cdr.clone())
                    };
                    let (val, rest) = match &rest {
                        Value::Cons(c2) => {
                            let cc2 = c2.borrow();
                            (cc2.car.clone(), cc2.cdr.clone())
                        }
                        _ => (Value::Nil, Value::Nil),
                    };
                    let is_face = matches!(&key, Value::Sym(s) if self.symbol_name(*s) == "face");
                    let f = if is_face {
                        self.intern("add-face-text-property")
                    } else {
                        self.intern("put-text-property")
                    };
                    let mut call = vec![Value::Int(1), Value::Int(pend as i128 + 1)];
                    if is_face {
                        call.push(val);
                        call.push(Value::t());
                    } else {
                        call.push(key);
                        call.push(val);
                    }
                    let _ = self.apply(&Value::Sym(f), call);
                    list = rest;
                }
            }
        }
        // Initial input: string, or (STRING . POS) where POS is a
        // 1-based offset converted to distance-from-end.
        let (init, pos) = match initial {
            Value::Cons(c) => {
                let (s, n) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                let slen = match &s {
                    Value::Str(t) => t.borrow().chars().count() as i128,
                    _ => 0,
                };
                let p = match n.int() {
                    Some(x) if x < 1 => -slen,
                    Some(x) => x - 1 - slen,
                    None => 0,
                };
                (s, p)
            }
            v => (v.clone(), 0),
        };
        if let Value::Str(s) = &init {
            let s = s.borrow().clone();
            if let Some(b) = self.buffers.get(mb_id) {
                let mut bb = b.borrow_mut();
                bb.insert(&s);
                let len = bb.text_len();
                let target = (bb.point() as i128 + pos).clamp(pend as i128, len as i128);
                bb.set_point(target as usize);
            }
        }
        Ok(Value::Nil)
    }

    /// `(end-of-file "Error reading from stdin")' — the signal GNU
    /// raises when noninteractive minibuffer input hits EOF.
    pub fn batch_eof_flow(&mut self) -> Flow {
        let eof = self.intern("end-of-file");
        self.signal_data(eof, vec![Value::string("Error reading from stdin")])
    }

    /// GNU `read_minibuf_noninteractive': echo PROMPT on stdout and
    /// read from stdin until `\n', `\r', or EOF.  A `\r' does not
    /// consume a following `\n' (GNU's getchar loop stops at `\r').
    /// EOF before any character signals `end-of-file'; a partial
    /// line at EOF is still returned.  When `read-hide-char' is a
    /// character, that many masking glyphs plus a newline are echoed.
    pub fn batch_read_line(&mut self, prompt: &str) -> Result<String, Flow> {
        use std::io::Write;
        print!("{prompt}");
        let _ = std::io::stdout().flush();
        let hide = self
            .intern_soft("read-hide-char")
            .map(|id| self.symbol_value(id))
            .and_then(|v| match v {
                Value::Int(n) if (0..=0x10ffff).contains(&n) => char::from_u32(n as u32),
                _ => None,
            });
        let mut line: Vec<u8> = Vec::new();
        let mut got_eof = false;
        {
            use std::io::Read;
            let stdin = self
                .batch_stdin
                .get_or_insert_with(|| std::io::BufReader::new(std::io::stdin()));
            let mut b = [0u8; 1];
            loop {
                match stdin.read(&mut b) {
                    Ok(0) | Err(_) => {
                        got_eof = true;
                        break;
                    }
                    Ok(_) if b[0] == b'\n' || b[0] == b'\r' => break,
                    Ok(_) => line.push(b[0]),
                }
            }
        }
        if let Some(h) = hide {
            let mut out = String::new();
            for _ in 0..line.len() {
                out.push(h);
            }
            out.push('\n');
            print!("{out}");
            let _ = std::io::stdout().flush();
        }
        // GNU `read_minibuf_noninteractive' bypasses the minibuffer
        // entirely — `minibuffer-depth' stays 0, no hooks run.
        if got_eof && line.is_empty() {
            Err(self.batch_eof_flow())
        } else {
            Ok(String::from_utf8_lossy(&line).into_owned())
        }
    }

    /// Read a single character from batch stdin (the way GNU's
    /// `read-char' and `y-or-n-p' consume the same `getchar' stream).
    /// EOF signals `end-of-file'.
    pub fn batch_read_char(&mut self) -> Result<i128, Flow> {
        use std::io::Read;
        let mut b = [0u8; 1];
        let r = {
            let stdin = self
                .batch_stdin
                .get_or_insert_with(|| std::io::BufReader::new(std::io::stdin()));
            stdin.read(&mut b)
        };
        match r {
            Ok(0) | Err(_) => Err(self.batch_eof_flow()),
            Ok(_) => Ok(b[0] as i128),
        }
    }

    /// Minibuffer line input in batch: read one line from stdin when
    /// running noninteractively.  Returns `None' when a front-end
    /// reader should handle it instead.
    pub fn batch_minibuf_line(&mut self, prompt: &str) -> Result<Option<String>, Flow> {
        if self.minibuf_reader.is_none() && self.noninteractive {
            return self.batch_read_line(prompt).map(Some);
        }
        Ok(None)
    }

    /// True when the selected window is the minibuffer window
    /// (GNU `BASE_EQ (selected_window, minibuf_window)').
    pub fn selected_window_is_minibuffer(&self) -> bool {
        let Some(sel) = crate::editor::sel_window(self) else {
            return false;
        };
        let Some(frame) = &self.selected_frame else {
            return false;
        };
        match &frame.borrow().minibuffer {
            Some(mb) => Rc::ptr_eq(&sel, mb),
            None => false,
        }
    }

    /// GNU `string_to_object' (minibuf.c): read a Lisp object from
    /// VAL; an empty string falls back to DEFALT's string (or first
    /// string element).  Trailing non-whitespace signals
    /// `invalid-read-syntax'.
    pub fn string_to_object(&mut self, val: &Value, defalt: &Value) -> EvalResult {
        let mut text = match val {
            Value::Str(s) => s.borrow().clone(),
            _ => String::new(),
        };
        if text.is_empty() {
            let d = match defalt {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => defalt.clone(),
            };
            if let Value::Str(s) = &d {
                text = s.borrow().clone();
            }
        }
        match self.read_from_string(&text, 0) {
            Ok((form, pos)) => {
                // Only trailing whitespace may follow the form.
                if text
                    .chars()
                    .skip(pos)
                    .any(|c| !matches!(c, ' ' | '\t' | '\n'))
                {
                    let irs = self.intern("invalid-read-syntax");
                    return Err(self.signal_data(
                        irs,
                        vec![Value::string("Trailing garbage following expression")],
                    ));
                }
                Ok(form)
            }
            Err(_) => {
                let eof = self.intern("end-of-file");
                Err(self.signal_data(eof, vec![Value::string("End of file during parsing")]))
            }
        }
    }

    /// Interactive-spec line input: front-end reader when available,
    /// else batch stdin (GNU `read_minibuf_noninteractive'), else None.
    fn spec_line(&mut self, prompt: &str) -> Result<Option<String>, Flow> {
        if self.minibuf_reader.is_some() {
            return self.minibuf_line(prompt).map(Some);
        }
        self.batch_minibuf_line(prompt)
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
        // GNU `command-execute' (simple.el): a command symbol whose
        // `disabled' property is non-nil is not run; instead
        // `disabled-command-function' (a hook var naming a function,
        // or a list of functions) handles it.  The `(query ...)'
        // property form is handled by `command-execute--query' — not
        // yet ported; such commands run normally for now.
        if let Value::Sym(id) = cmd {
            let dis_id = self.intern("disabled");
            let dis = self.get_prop(*id, dis_id);
            if !dis.is_nil() {
                let q_id = self.intern("query");
                let query = matches!(
                    &dis,
                    Value::Cons(c)
                        if matches!(c.borrow().car, Value::Sym(s) if s == q_id)
                );
                if !query {
                    let dcf = self.intern("disabled-command-function");
                    if self.bound_p(dcf) && self.symbol_value(dcf).truthy() {
                        return crate::lisp::builtins::evalfn::call_hook(
                            self,
                            "disabled-command-function",
                        );
                    }
                }
            }
        }
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
            if let Some(spec_form) = subr_interactive_form(self, s.name) {
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
                            // `N' uses a supplied prefix; `n' always
                            // prompts (GNU callint.c).
                            if c == 'N' && !pa.is_nil() {
                                out.push(prefix_numeric(self, &pa));
                            } else if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                let rn = self.intern("read-number");
                                out.push(self.apply(&Value::Sym(rn), vec![Value::string(prompt)])?);
                            }
                        }
                        'b' | 'B' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                // GNU callint.c: 'b' defaults to
                                // other-buffer only when the minibuffer
                                // window is selected; 'B' always uses
                                // other-buffer ('b' => require-match t).
                                let cur = self
                                    .current_buffer_ref()
                                    .map(|b| Value::Buffer(b.clone()))
                                    .unwrap_or(Value::Nil);
                                let def = if c == 'B' || self.selected_window_is_minibuffer() {
                                    let ob = self.intern("other-buffer");
                                    self.apply(&Value::Sym(ob), vec![cur])?
                                } else {
                                    cur
                                };
                                let rb = self.intern("read-buffer");
                                let mut argv = vec![Value::string(prompt), def];
                                if c == 'b' {
                                    argv.push(Value::t());
                                }
                                out.push(self.apply(&Value::Sym(rb), argv)?);
                            }
                        }
                        's' | 'M' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                let rs = self.intern("read-string");
                                out.push(self.apply(&Value::Sym(rs), vec![Value::string(prompt)])?);
                            }
                        }
                        'F' | 'f' | 'D' | 'G' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                let rfn = self.intern("read-file-name");
                                out.push(
                                    self.apply(&Value::Sym(rfn), vec![Value::string(prompt)])?,
                                );
                            }
                        }
                        'z' | 'Z' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                let rcs = self.intern("read-coding-system");
                                out.push(
                                    self.apply(&Value::Sym(rcs), vec![Value::string(prompt)])?,
                                );
                            }
                        }
                        'a' | 'C' | 'S' | 'v' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if let Some(s) = self.spec_line(&prompt)? {
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
                                match self.minibuf_input(&prompt, true)? {
                                    MinibufInput::Call(_) => {
                                        return Err(self.error("exit function in key read"));
                                    }
                                    MinibufInput::Key(k) => {
                                        if c == 'K' {
                                            out.push(Value::Vec(Rc::new(RefCell::new(vec![
                                                Value::Int(k),
                                            ]))));
                                        } else if k < 128 {
                                            out.push(Value::string(
                                                char::from_u32(k as u32).unwrap_or(' ').to_string(),
                                            ));
                                        } else {
                                            out.push(Value::Vec(Rc::new(RefCell::new(vec![
                                                Value::Int(k),
                                            ]))));
                                        }
                                    }
                                    MinibufInput::Text(t) => {
                                        out.push(Value::string(t));
                                    }
                                }
                            } else if self.noninteractive {
                                // GNU batch (threadless builds): one
                                // getchar yields a one-key sequence.
                                let k = self.batch_read_char()?;
                                if c == 'K' {
                                    out.push(Value::Vec(Rc::new(RefCell::new(vec![Value::Int(
                                        k,
                                    )]))));
                                } else {
                                    out.push(Value::string(
                                        char::from_u32(k as u32).unwrap_or(' ').to_string(),
                                    ));
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'x' | 'X' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                // `x' reads a form (read-minibuffer);
                                // `X' reads and evals (eval-minibuffer).
                                let fname = if c == 'x' {
                                    "read-minibuffer"
                                } else {
                                    "eval-minibuffer"
                                };
                                let f = self.intern(fname);
                                out.push(self.apply(&Value::Sym(f), vec![Value::string(prompt)])?);
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
                                    MinibufInput::Call(_) => {
                                        return Err(self.error("exit function in key read"));
                                    }
                                }
                            } else if self.noninteractive {
                                // GNU batch `read-char' consumes one
                                // stdin character.
                                let k = self.batch_read_char()?;
                                out.push(Value::Int(k));
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
                // GNU requires the result to be a list (callint.c).
                let v = self.eval(&spec)?;
                match v.list_to_vec() {
                    Ok(argv) => Ok(argv),
                    Err(_) => Err(self.wrong_type_mut("listp", &v)),
                }
            }
        }
    }
}

/// Return `(interactive SPEC)' for a primitive.  GNU primitive specs are
/// stored as strings; a leading `(' means the string is an expression to
/// read and evaluate rather than callint letter codes.
pub(crate) fn subr_interactive_form(i: &mut Interp, name: &str) -> Option<Value> {
    let spec = subr_interactive(name)?;
    // "\u{1}nil" is a sentinel for GNU's `(interactive)' (no argument),
    // which `interactive-form' renders as `(interactive nil)'.
    let spec = match spec {
        "\u{1}nil" => Value::Nil,
        s if s.trim_start().starts_with('(') => i
            .read_from_string(s, 0)
            .ok()
            .map(|(form, _)| form)
            .unwrap_or_else(|| Value::string(s)),
        s => Value::string(s),
    };
    Some(Value::list(vec![Value::Sym(i.intern("interactive")), spec]))
}

/// Interactive specs for subrs that Emacs declares `interactive'.
/// `commandp`/`command-execute` consult this for primitives.
pub(crate) fn subr_interactive(name: &str) -> Option<&'static str> {
    const T: &[(&str, &str)] = &[
        ("Buffer-menu-delete", "p"),
        ("Buffer-menu-delete-backwards", "p"),
        ("Buffer-menu-mouse-select", "e"),
        ("Buffer-menu-not-modified", "P"),
        ("Buffer-menu-toggle-files-only", "P"),
        ("Buffer-menu-toggle-internal", "P"),
        ("Buffer-menu-unmark", "P"),
        (
            "Buffer-menu-unmark-all-buffers",
            "cRemove marks (RET means all):",
        ),
        ("abbrev-prefix-mark", "P"),
        ("abort-minibuffers", ""),
        ("abort-recursive-edit", ""),
        ("activate-transient-input-method", "P\np"),
        ("add-global-abbrev", "P"),
        ("add-mode-abbrev", "P"),
        (
            "add-name-to-file",
            "fAdd name to file: \nGName to add to %s: \np",
        ),
        ("append-next-kill", "p"),
        ("append-to-file", "r\nFAppend to file: "),
        ("back-to-indentation", "^"),
        ("backward-button", "p\nd\nd"),
        ("backward-char", "^p"),
        ("backward-delete-char-untabify", "*p\nP"),
        ("backward-kill-paragraph", "p"),
        ("backward-kill-sentence", "p"),
        ("backward-kill-sexp", "p\nd"),
        ("backward-kill-word", "p"),
        ("backward-list", "^p\nd"),
        ("backward-page", "p"),
        ("backward-paragraph", "^p"),
        ("backward-sentence", "^p"),
        ("backward-sexp", "^p\nd"),
        ("backward-to-indentation", "^p"),
        ("backward-up-list", "^p\nd\nd"),
        ("backward-word", "^p"),
        ("base64-decode-region", "r"),
        ("base64-encode-region", "r"),
        ("base64url-encode-region", "r"),
        ("beginning-of-buffer", "^P"),
        ("beginning-of-buffer-other-window", "P"),
        ("beginning-of-defun", "^p"),
        ("beginning-of-defun-comments", "^p"),
        ("beginning-of-defun-raw", "^p"),
        ("beginning-of-line", "^p"),
        ("beginning-of-line-text", "^p"),
        ("beginning-of-visual-line", "^p"),
        ("buffer-enable-undo", ""),
        ("buffer-menu", "P"),
        ("buffer-menu-other-window", "P"),
        ("button-describe", "d"),
        ("call-last-kbd-macro", "p"),
        ("cancel-function-timers", "aCancel timers of function: "),
        ("canonically-space-region", "*r"),
        ("capitalize-dwim", "*p"),
        ("capitalize-word", "p"),
        ("center-line", "P"),
        ("center-region", "r"),
        ("clipboard-kill-region", "r\np"),
        ("clipboard-kill-ring-save", "r\np"),
        ("clipboard-yank", "*"),
        ("comment-box", "*r\np"),
        ("comment-dwim", "*P"),
        ("comment-indent", "*"),
        ("comment-kill", "P"),
        ("comment-line", "p"),
        ("comment-or-uncomment-region", "*r\nP"),
        ("comment-region", "*r\nP"),
        ("comment-set-column", "P"),
        ("complete-symbol", "P"),
        ("compose-last-chars", "e"),
        ("compose-region", "r"),
        ("copy-file", "fCopy file: \nGCopy %s to file: \np\nP"),
        ("copy-to-buffer", "BCopy to buffer: \nr"),
        ("cycle-spacing", "*P"),
        ("debugger-trap", ""),
        ("decode-coding-region", "r\nzCoding system: "),
        ("decompose-region", "r"),
        ("decrease-left-margin", "*r\nP"),
        ("decrease-right-margin", "*r\nP"),
        ("define-abbrevs", "P"),
        (
            "define-global-abbrev",
            "sDefine global abbrev: \nsExpansion for %s: ",
        ),
        (
            "define-mode-abbrev",
            "sDefine mode abbrev: \nsExpansion for %s: ",
        ),
        ("defining-kbd-macro", "P"),
        ("delete-all-space", "*P"),
        ("delete-backward-char", "p\nP"),
        ("delete-blank-lines", "*"),
        ("delete-char", "p\nP"),
        ("delete-forward-char", "p\nP"),
        ("delete-frame", ""),
        ("delete-horizontal-space", "*P"),
        ("delete-other-frames", "i\nP"),
        ("delete-other-windows", "i\np"),
        ("delete-other-windows-internal", ""),
        ("delete-pair", "P"),
        ("delete-region", "r"),
        ("digit-argument", "P"),
        (
            "display-buffer-other-frame",
            "bDisplay buffer in other frame: ",
        ),
        ("do-auto-save", ""),
        ("down-list", "^p\nd"),
        ("downcase-dwim", "*p"),
        ("downcase-word", "p"),
        ("electric-indent-just-newline", "*P"),
        ("electric-newline-and-maybe-indent", "*"),
        ("elisp-byte-compile-buffer", "P"),
        ("elisp-byte-compile-file", "P"),
        ("elisp-enable-lexical-binding", "@p"),
        ("elisp-last-sexp-toggle-display", "P"),
        ("emacs-version", "P"),
        ("encode-coding-region", "r\nzCoding system: "),
        ("end-kbd-macro", "p"),
        ("end-of-buffer", "^P"),
        ("end-of-buffer-other-window", "P"),
        ("end-of-defun", "^p\nd"),
        ("end-of-line", "^p"),
        ("end-of-visual-line", "^p"),
        ("enlarge-window", "p"),
        ("enlarge-window-horizontally", "p"),
        ("ensure-empty-lines", "p"),
        ("erase-buffer", "*"),
        ("eval-buffer", ""),
        ("eval-defun", "P"),
        (
            "eval-expression",
            "(cons (read--expression \"Eval: \") (eval-expression-get-print-arguments current-prefix-arg))",
        ),
        ("eval-last-sexp", "P"),
        ("eval-print-last-sexp", "P"),
        ("eval-region", "r"),
        ("exchange-point-and-mark", "P"),
        ("execute-extended-command", "(list current-prefix-arg)"),
        ("exit-recursive-edit", ""),
        ("expand-region-abbrevs", "r\nP"),
        ("first-error", "p"),
        ("fixup-whitespace", "*"),
        ("font-lock-fontify-block", "P"),
        ("font-lock-fontify-buffer", "p"),
        ("font-lock-update", "P"),
        ("forward-button", "p\nd\nd"),
        ("forward-char", "^p"),
        ("forward-line", "^p"),
        ("forward-list", "^p\nd"),
        ("forward-page", "p"),
        ("forward-paragraph", "^p"),
        ("forward-same-syntax", "^p"),
        ("forward-sentence", "^p"),
        ("forward-sexp", "^p\nd"),
        ("forward-symbol", "^p"),
        ("forward-to-indentation", "^p"),
        ("forward-whitespace", "^p"),
        ("forward-word", "^p"),
        ("fullwidth-region", "r"),
        ("fullwidth-word", "p"),
        ("garbage-collect", ""),
        ("global-unset-key", "kUnset key globally: "),
        ("goto-history-element", "p"),
        ("halfwidth-region", "r"),
        ("halfwidth-word", "p"),
        ("handle-delete-frame", "e"),
        ("handle-focus-in", "e"),
        ("handle-focus-out", "e"),
        ("handle-move-frame", "e"),
        ("handle-select-window", "^e"),
        ("handle-switch-frame", "^e"),
        ("iconify-frame", ""),
        ("image-decrease-size", "P"),
        ("image-increase-size", "P"),
        ("image-mouse-decrease-size", "e"),
        ("image-mouse-increase-size", "e"),
        ("increase-left-margin", "*r\nP"),
        ("increase-right-margin", "r\nP"),
        ("indent-code-rigidly", "r\np"),
        ("indent-for-tab-command", "P"),
        ("indent-pp-sexp", "P"),
        ("indent-region", "r\nP"),
        ("indent-relative", "P"),
        ("indent-rigidly", "r\nP\np"),
        ("indent-rigidly-left", "r"),
        ("indent-rigidly-left-to-tab-stop", "r"),
        ("indent-rigidly-right", "r"),
        ("indent-rigidly-right-to-tab-stop", "r"),
        ("indent-to", "NIndent to column: "),
        ("insert-file", "*fInsert file: "),
        ("insert-file-literally", "*fInsert file literally: "),
        ("insert-pair", "P"),
        ("insert-parentheses", "P"),
        ("inverse-add-global-abbrev", "p"),
        ("inverse-add-mode-abbrev", "p"),
        ("isearch-backward", "P\np"),
        ("isearch-backward-regexp", "P\np"),
        ("isearch-beginning-of-buffer", "p"),
        ("isearch-char-by-name", "p"),
        ("isearch-del-char", "p"),
        ("isearch-emoji-by-name", "p"),
        ("isearch-end-of-buffer", "p"),
        ("isearch-forward", "P\np"),
        ("isearch-forward-regexp", "P\np"),
        ("isearch-forward-symbol", "P\np"),
        ("isearch-forward-symbol-at-point", "P"),
        ("isearch-forward-word", "P\np"),
        ("isearch-mouse-2", "e"),
        ("isearch-quote-char", "p"),
        ("isearch-repeat-backward", "P"),
        ("isearch-repeat-forward", "P"),
        ("isearch-xterm-paste", "e"),
        ("isearch-yank-char", "p"),
        ("isearch-yank-char-in-minibuffer", "p"),
        ("isearch-yank-line", "p"),
        ("isearch-yank-pop-only", "P"),
        ("isearch-yank-symbol-or-char", "p"),
        ("isearch-yank-until-char", "cYank until character: \np"),
        ("isearch-yank-word", "p"),
        ("isearch-yank-word-or-char", "p"),
        ("japanese-hankaku-region", "r\nP"),
        ("japanese-hiragana-region", "r"),
        ("japanese-katakana-region", "r\nP"),
        ("japanese-zenkaku-region", "r\nP"),
        ("just-one-space", "*p"),
        ("justify-current-line", "*"),
        (
            "keymap-global-set",
            "KSet key globally: \nCSet key %s globally to command: \np",
        ),
        (
            "keymap-local-set",
            "KSet key locally: \nCSet key %s locally to command: \np",
        ),
        ("kill-backward-up-list", "*p"),
        ("kill-buffer", "bKill buffer: "),
        ("kill-emacs", "P"),
        ("kill-line", "P"),
        ("kill-local-variable", "vKill Local Variable: "),
        (
            "kill-matching-buffers",
            "sKill buffers matching this regular expression: \nP",
        ),
        (
            "kill-matching-buffers-no-ask",
            "sKill buffers matching this regular expression: \nP",
        ),
        ("kill-paragraph", "p"),
        (
            "kill-region",
            "(progn (let ((beg (mark)) (end (point))) (if (not (and beg end)) (user-error \"The mark is not set now, so there is no region\") (list beg end))))",
        ),
        ("kill-sentence", "p"),
        ("kill-sexp", "p\nd"),
        ("kill-visual-line", "P"),
        ("kill-whole-line", "p"),
        ("kill-word", "p"),
        ("left-char", "^p"),
        ("left-word", "^p"),
        ("lisp-fill-paragraph", "P"),
        ("list-abbrevs", "P"),
        ("list-buffers", "P"),
        (
            "local-set-key",
            "KSet key locally: \nCSet key %s locally to command: ",
        ),
        ("local-unset-key", "kUnset key locally: "),
        ("lower-frame", ""),
        ("make-frame-invisible", ""),
        ("make-frame-visible", ""),
        (
            "make-indirect-buffer",
            "bMake indirect buffer (to buffer): \nBName of indirect buffer: ",
        ),
        ("make-local-variable", "vMake Local Variable: "),
        (
            "make-symbolic-link",
            "FMake symbolic link to file: \nGMake symbolic link to file %s: \np",
        ),
        (
            "make-variable-buffer-local",
            "vMake Variable Buffer Local: ",
        ),
        ("mark-defun", "p\nd"),
        ("mark-end-of-sentence", "p"),
        ("mark-page", "P"),
        ("mark-paragraph", "p\np"),
        ("mark-sexp", "P\np"),
        ("mark-word", "P\np"),
        ("menu-bar-open-mouse", "e"),
        ("menu-bar-select-yank", "*"),
        ("minibuffer-beginning-of-buffer", "^P"),
        ("minibuffer-choose-completion", "P"),
        ("minibuffer-choose-completion-or-exit", "P"),
        ("minibuffer-complete-and-exit", "P"),
        ("minibuffer-completion-exit", "P"),
        ("minibuffer-next-column-completion", "p"),
        ("minibuffer-next-completion", "p"),
        ("minibuffer-next-line-completion", "p"),
        ("minibuffer-previous-column-completion", "p"),
        ("minibuffer-previous-completion", "p"),
        ("minibuffer-previous-line-completion", "p"),
        ("minibuffer-recenter-top-bottom", "P"),
        ("minibuffer-scroll-down-command", "^P"),
        ("minibuffer-scroll-other-window", "P"),
        ("minibuffer-scroll-other-window-down", "^P"),
        ("minibuffer-scroll-up-command", "^P"),
        ("mode-line-bury-buffer", "e"),
        ("mode-line-change-eol", "e"),
        ("mode-line-minor-mode-help", "@e"),
        ("mode-line-next-buffer", "e"),
        ("mode-line-previous-buffer", "e"),
        ("mode-line-toggle-modified", "e"),
        ("mode-line-toggle-read-only", "e"),
        ("mode-line-unbury-buffer", "e"),
        ("mode-line-widen", "e"),
        (
            "modify-syntax-entry",
            "cSet syntax for character: \nsSet syntax for %s to: ",
        ),
        ("mouse-appearance-menu", "@e"),
        ("mouse-buffer-menu", "e"),
        ("mouse-delete-other-windows", "e"),
        ("mouse-delete-window", "e"),
        ("mouse-drag-and-drop-region", "e"),
        ("mouse-drag-bottom-edge", "e"),
        ("mouse-drag-bottom-left-corner", "e"),
        ("mouse-drag-bottom-right-corner", "e"),
        ("mouse-drag-header-line", "e"),
        ("mouse-drag-left-edge", "e"),
        ("mouse-drag-mode-line", "e"),
        ("mouse-drag-region", "e"),
        ("mouse-drag-region-rectangle", "e"),
        ("mouse-drag-region-shift-adjust", "e"),
        ("mouse-drag-right-edge", "e"),
        ("mouse-drag-secondary", "e"),
        ("mouse-drag-tab-line", "e"),
        ("mouse-drag-top-edge", "e"),
        ("mouse-drag-top-left-corner", "e"),
        ("mouse-drag-top-right-corner", "e"),
        ("mouse-drag-vertical-line", "e"),
        ("mouse-kill", "e"),
        ("mouse-kill-ring-save", "e"),
        ("mouse-minor-mode-menu", "@e"),
        ("mouse-save-then-kill", "e"),
        ("mouse-secondary-save-then-kill", "e"),
        ("mouse-select-window", "e"),
        ("mouse-set-mark", "e"),
        ("mouse-set-point", "e\np"),
        ("mouse-set-region", "e"),
        ("mouse-set-secondary", "e"),
        ("mouse-split-window-horizontally", "@e"),
        ("mouse-split-window-vertically", "@e"),
        ("mouse-start-secondary", "e"),
        ("mouse-yank-at-click", "e\nP"),
        ("mouse-yank-secondary", "e"),
        ("move-beginning-of-line", "^p"),
        ("move-end-of-line", "^p"),
        ("move-file-to-trash", "fMove file to trash: "),
        ("move-to-column", "NMove to column: "),
        ("move-to-window-line", "P"),
        ("move-to-window-line-top-bottom", "P"),
        ("narrow-to-page", "P"),
        ("narrow-to-region", "r"),
        ("negative-argument", "P"),
        ("newline", "*P\np"),
        ("newline-and-indent", "*p"),
        ("next-buffer", "p\np"),
        ("next-column-completion", "p"),
        ("next-complete-history-element", "p"),
        ("next-completion", "p"),
        ("next-error", "P"),
        ("next-error-no-select", "p"),
        ("next-error-this-buffer-no-select", "p"),
        ("next-history-element", "p"),
        ("next-line", "^p\np"),
        ("next-line-completion", "p"),
        ("next-line-or-history-element", "^p"),
        ("next-logical-line", "^p\np"),
        ("nonincremental-re-search-backward", "sSearch for regexp: "),
        ("nonincremental-re-search-forward", "sSearch for regexp: "),
        (
            "nonincremental-search-backward",
            "sSearch backwards for string: ",
        ),
        ("nonincremental-search-forward", "sSearch for string: "),
        ("not-modified", "P"),
        ("ns-drag-n-drop", "e"),
        ("ns-popup-color-panel", ""),
        ("occur-next", "p"),
        ("occur-next-error", "p"),
        ("occur-prev", "p"),
        ("occur-rename-buffer", "P\np"),
        ("occur-symbol-at-mouse", "e"),
        ("occur-word-at-mouse", "e"),
        ("open-dribble-file", "FOpen dribble file: "),
        ("open-line", "*p"),
        ("open-termscript", "FOpen termscript file: "),
        ("other-frame", "p"),
        ("other-window", "p\ni\np"),
        ("other-window-backward", "p\ni\np"),
        ("play-sound-file", "fPlay sound file: "),
        ("posix-search-backward", "sPosix search backward: "),
        ("posix-search-forward", "sPosix search: "),
        ("prefer-coding-system", "zPrefer coding system: "),
        ("prepend-to-buffer", "BPrepend to buffer: \nr"),
        ("previous-buffer", "p\np"),
        ("previous-column-completion", "p"),
        ("previous-complete-history-element", "p"),
        ("previous-completion", "p"),
        ("previous-error", "p"),
        ("previous-error-no-select", "p"),
        ("previous-error-this-buffer-no-select", "p"),
        ("previous-history-element", "p"),
        ("previous-line", "^p\np"),
        ("previous-line-completion", "p"),
        ("previous-line-or-history-element", "^p"),
        ("previous-logical-line", "^p\np"),
        ("prog-fill-reindent-defun", "P"),
        ("prog-fill-reindent-defun-default", "P"),
        ("prog-indent-sexp", "P"),
        ("push-mark-command", "P"),
        ("pwd", "P"),
        ("quit-window", "P"),
        ("quit-windows-on", "bQuit windows on (buffer):\nP"),
        ("quoted-insert", "*p"),
        ("raise-frame", ""),
        ("raise-sexp", "p"),
        ("re-search-backward", "sRE search backward: "),
        ("re-search-forward", "sRE search: "),
        ("read-color", "i\np\ni\np"),
        ("recenter", "P\np"),
        ("recenter-current-error", "P"),
        ("recenter-other-window", "P"),
        ("recenter-top-bottom", "P"),
        ("recover-file", "FRecover file: "),
        ("recursive-edit", ""),
        ("redirect-debugging-output", "FDebug output file: \nP"),
        ("redraw-display", ""),
        ("reindent-then-newline-and-indent", "*"),
        ("rename-file", "fRename file: \nGRename %s to file: \np"),
        ("repeat-complex-command", "p"),
        ("replace-buffer-contents", "bSource buffer: "),
        ("replace-buffer-in-windows", "bBuffer to replace: "),
        ("repunctuate-sentences", "i\nR"),
        ("revert-buffer-quick", "P"),
        (
            "revert-buffer-with-coding-system",
            "zCoding system for visited file (default nil): \nP",
        ),
        ("right-char", "^p"),
        ("right-word", "^p"),
        ("rotate-yank-pointer", "p"),
        (
            "run-at-time",
            "sRun at time: \nNRepeat interval: \naFunction: ",
        ),
        (
            "run-with-timer",
            "sRun after delay (seconds): \nNRepeat interval: \naFunction: ",
        ),
        ("save-buffer", "p"),
        ("save-buffers-kill-emacs", "P"),
        ("save-buffers-kill-terminal", "P"),
        ("save-some-buffers", "P"),
        ("scroll-bar-drag", "e"),
        ("scroll-bar-horizontal-drag", "e"),
        ("scroll-bar-maybe-set-window-start", "e"),
        ("scroll-bar-scroll-down", "e"),
        ("scroll-bar-scroll-up", "e"),
        ("scroll-bar-set-window-start", "e"),
        ("scroll-bar-toolkit-horizontal-scroll", "e"),
        ("scroll-bar-toolkit-scroll", "e"),
        ("scroll-down", "^P"),
        ("scroll-down-command", "^P"),
        ("scroll-down-line", "p"),
        ("scroll-left", "^P\np"),
        ("scroll-other-window", "P"),
        ("scroll-other-window-down", "P"),
        ("scroll-right", "^P\np"),
        ("scroll-up", "^P"),
        ("scroll-up-command", "^P"),
        ("scroll-up-line", "p"),
        ("search-backward", "MSearch backward: "),
        ("search-backward-regexp", "sRE search backward: "),
        ("search-forward", "MSearch: "),
        ("search-forward-regexp", "sRE search: "),
        ("select-frame", "e"),
        (
            "self-insert-command",
            "(list (prefix-numeric-value current-prefix-arg) last-command-event)",
        ),
        (
            "set-buffer-process-coding-system",
            "zCoding-system for output from the process: \nzCoding-system for input to the process: ",
        ),
        (
            "set-file-name-coding-system",
            "zCoding system for file names (default nil): ",
        ),
        ("set-fill-prefix", "P"),
        ("set-goal-column", "P"),
        ("set-left-margin", "r\nNSet left margin to column: "),
        ("set-mark-command", "P"),
        ("set-right-margin", "r\nNSet right margin to width: "),
        (
            "set-selection-coding-system",
            "zCoding system for X selection: ",
        ),
        ("set-selective-display", "P"),
        ("set-visited-file-name", "FSet visited file name: "),
        ("shrink-window", "p"),
        ("shrink-window-horizontally", "p"),
        ("split-line", "*P"),
        ("start-kbd-macro", "P"),
        ("suspend-emacs", ""),
        ("tab-bar-close-tab", "P"),
        ("tab-bar-duplicate-tab", "P"),
        ("tab-bar-menu-bar", "e"),
        ("tab-bar-merge-tabs", "i\ni\nP"),
        ("tab-bar-mouse-1", "e"),
        ("tab-bar-mouse-close-tab", "e"),
        ("tab-bar-mouse-context-menu", "e"),
        ("tab-bar-mouse-down-1", "e"),
        ("tab-bar-mouse-move-tab", "e"),
        ("tab-bar-move-tab", "p"),
        ("tab-bar-move-tab-backward", "p"),
        ("tab-bar-move-tab-to", "P"),
        ("tab-bar-move-tab-to-frame", "P"),
        ("tab-bar-new-tab", "P"),
        ("tab-bar-new-tab-to", "P"),
        ("tab-bar-select-tab", "P"),
        ("tab-bar-split-tab", "i\nP"),
        ("tab-bar-switch-to-last-tab", "p"),
        ("tab-bar-switch-to-next-tab", "p"),
        ("tab-bar-switch-to-prev-tab", "p"),
        ("tab-bar-switch-to-recent-tab", "p"),
        ("tab-bar-touchscreen-begin", "e"),
        ("tab-switcher-delete", "p"),
        ("tab-switcher-delete-backwards", "p"),
        ("tab-switcher-mouse-select", "e"),
        ("tab-switcher-next-line", "p"),
        ("tab-switcher-prev-line", "p"),
        ("tab-switcher-unmark", "P"),
        ("tabulated-list-col-sort", "e"),
        ("tabulated-list-narrow-current-column", "p"),
        ("tabulated-list-next-column", "p"),
        ("tabulated-list-previous-column", "p"),
        ("tabulated-list-sort", "P"),
        ("tabulated-list-widen-current-column", "p"),
        ("toggle-enable-multibyte-characters", "P"),
        ("toggle-horizontal-scroll-bar", "P"),
        ("toggle-input-method", "P\np"),
        ("toggle-scroll-bar", "P"),
        ("toggle-truncate-lines", "P"),
        ("toggle-window-dedicated", "i\nP\np"),
        ("toggle-word-wrap", "P"),
        ("top-level", ""),
        ("transpose-chars", "*p"),
        ("transpose-lines", "*p"),
        ("transpose-paragraphs", "*p"),
        ("transpose-sentences", "*p"),
        ("transpose-sexps", "*p\nd"),
        ("transpose-words", "*p"),
        ("ucs-normalize-HFS-NFC-region", "r"),
        ("ucs-normalize-HFS-NFD-region", "r"),
        ("ucs-normalize-NFC-region", "r"),
        ("ucs-normalize-NFD-region", "r"),
        ("ucs-normalize-NFKC-region", "r"),
        ("ucs-normalize-NFKD-region", "r"),
        ("uncomment-region", "*r\nP"),
        ("undelete-frame", "P"),
        ("undo", "*P"),
        ("undo-ignore-read-only", "P"),
        ("undo-only", "*p"),
        ("undo-redo", "*p"),
        ("unfill-paragraph", "P\nR"),
        ("universal-argument", "\u{1}nil"),
        ("universal-argument-more", "P"),
        ("unix-filename-rubout", "^p"),
        ("unix-sync", ""),
        ("unix-word-rubout", "^p"),
        ("up-list", "^p\nd\nd"),
        ("upcase-dwim", "*p"),
        ("upcase-word", "p"),
        ("view-emacs-news", "P"),
        ("view-emacs-todo", "P"),
        ("view-lossage", "P"),
        ("what-cursor-position", "P"),
        ("widen", ""),
        ("word-search-backward", "sWord search backward: "),
        ("word-search-backward-lax", "sWord search backward: "),
        ("word-search-forward", "sWord search: "),
        ("word-search-forward-lax", "sWord search: "),
        ("write-region", "r\nFWrite region to file: \ni\ni\ni\np"),
        ("yank", "*P"),
        ("yank-in-context", "*P"),
        ("yank-pop", "p"),
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
