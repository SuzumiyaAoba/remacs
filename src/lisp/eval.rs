//! The evaluator: `Interp` state, `eval`, `apply`, and special forms.
//!
//! Scoping: elisp is dynamically scoped by default. Dynamic `let` is
//! implemented exactly like Emacs's specbind: save the symbol's current
//! value cell, overwrite, restore on unwind — so lookup stays O(1).
//! When `lexical-binding` is on, `let` binds in a captured lexical env
//! chain instead (unless the symbol is `defvar`'d special).

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::cell::RefCell;

use std::rc::Rc;

use super::error::{EvalResult, Flow};
use super::obarray::Obarray;
use super::obarray::sym;
use super::reader::Reader;
use super::value::{Argv, Arity, Cons, Lambda, Marker, SymId, Value};

/// A saved dynamic binding (specbind entry).
struct SpecBind {
    sym: SymId,
    /// `Some(buf)` = binding is buffer-local in `buf`; `None` = global cell.
    buf: Option<usize>,
    /// Previous value (`None` = no local binding existed).
    old: Option<Value>,
}

/// A lexical environment frame (chained).
///
/// Bindings are stored in a `Vec`, not a `HashMap`: `let' frames and
/// lambda calls typically bind a handful of variables, so a linear
/// scan beats hashing and we skip two allocations per frame.  The
/// vector's order doubles as GNU's `internal-interpreter-environment'
/// order — `lexenv_as_value' walks it in reverse (newest first).
pub struct LexFrame {
    /// `(sym . value)' pairs in binding order (oldest first).
    pub vars: RefCell<Vec<(SymId, Value)>>,
    /// Scoped `defvar' declarations — GNU's bare-symbol elements of
    /// `internal-interpreter-environment': a name listed here is bound
    /// dynamically by `let'/`let*' for the rest of this scope's extent,
    /// then the declaration unwinds with the frame.
    pub declared: RefCell<Vec<SymId>>,
    pub parent: Option<Rc<LexFrame>>,
}

/// Record a binding in a lexical frame, tracking GNU's env order.
/// A re-bind keeps the name's original position (GNU's `assq' hits the
/// first env entry, same observable order).
pub fn lexframe_bind(frame: &LexFrame, sym: SymId, val: Value) {
    let mut vars = frame.vars.borrow_mut();
    if let Some(e) = vars.iter_mut().find(|(s, _)| *s == sym) {
        e.1 = val;
    } else {
        vars.push((sym, val));
    }
}

/// GNU's `internal-interpreter-environment' as a Lisp value: a flat
/// alist of `(sym . val)' conses, innermost bindings first (specbind
/// prepends).  Bare `defvar' markers appear as bare symbols.  An
/// entirely empty chain prints as `(t)' — our root-env convention.
pub fn lexenv_as_value(env: &LexEnv) -> Value {
    let mut items: Vec<Value> = Vec::new();
    let mut cur = env;
    while let Some(frame) = cur {
        for s in frame.declared.borrow().iter() {
            items.push(Value::Sym(*s));
        }
        for (s, v) in frame.vars.borrow().iter().rev() {
            items.push(Value::cons(Value::Sym(*s), v.clone()));
        }
        cur = &frame.parent;
    }
    if items.is_empty() {
        Value::list(vec![Value::Sym(crate::lisp::obarray::sym::T)])
    } else {
        Value::list(items)
    }
}

pub type LexEnv = Option<Rc<LexFrame>>;

pub fn lexenv_lookup(mut env: &LexEnv, sym: SymId) -> Option<Value> {
    while let Some(frame) = env {
        if let Some((_, v)) = frame.vars.borrow().iter().find(|(s, _)| *s == sym) {
            return Some(v.clone());
        }
        env = &frame.parent;
    }
    None
}

/// Read a cons cell's `cdr` without borrowing the `RefCell` or cloning
/// the `Value`.  Spine walks only ever follow `cdr` chains — the cells
/// stay alive through the `Rc` links reachable from the list head, and
/// nothing mutates them while these loops run — so a raw pointer read
/// is safe and skips a borrow + two `Rc` refcount ops per step.  The
/// returned `&'static` is a scoped lie: callers must only use it while
/// the owning list value is alive and unmutated.
pub(crate) unsafe fn cdr_ptr(c: &Rc<RefCell<Cons>>) -> &'static Value {
    unsafe { &(*RefCell::as_ptr(c)).cdr }
}

/// Cons-cell pointer of a `Value::Cons`, or null.
pub(crate) fn cons_ptr(v: &Value) -> *const RefCell<Cons> {
    match v {
        Value::Cons(c) => Rc::as_ptr(c),
        _ => std::ptr::null(),
    }
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
        let mut d = frame.declared.borrow_mut();
        if !d.contains(&sym) {
            d.push(sym);
        }
    }
}

/// A fresh empty lexical root — GNU's `(t)' sentinel environment.
pub fn lexenv_root() -> LexEnv {
    Some(Rc::new(LexFrame {
        vars: RefCell::new(Vec::new()),
        declared: RefCell::new(Vec::new()),
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
    /// Names a bare `(defvar X)' declared at file top-level — GNU's
    /// scoped `defvar' marker unwinds with the load, but the same file
    /// byte-compiled would emit a permanent `specbind' for X, so
    /// `let's see it dynamically forever after.  Root-frame
    /// `declared' sets are merged here when each file finishes
    /// loading, approximating the compiled effect for interpreted
    /// code (e.g. rx.el's `rx--pcase-vars' feeding pcase expansion
    /// during other libraries' loads).
    pub file_declared: RefCell<HashSet<SymId>>,
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
    /// `inhibit-read-only` dynamic override.
    pub standard_output_sym: SymId,
    /// SymId of `undo-inhibit-record-point' — its dynamic value is
    /// mirrored into `buffers.undo_inhibit' so `record_point' (which
    /// runs without interpreter access) can consult it.
    pub undo_inhibit_sym: SymId,
    /// SymId of `buffer-file-name' — in GNU the variable IS the
    /// `buffer->name_' slot (`BVAR'), so Lisp-side `setq' writes must
    /// update `Buffer::file_name' too (plain `setq' otherwise leaves
    /// `(buffer-file-name)' and file primitives reading a stale slot).
    pub bfn_sym: SymId,
    /// Command-loop state used by `interactive` specs.
    pub command_args: Vec<Value>,
    /// Depth of in-progress `call-interactively'/command-loop
    /// invocations — `called-interactively-p' reports non-nil inside
    /// one.  (GNU tracks a per-frame flag on the call stack.)
    pub interactive_call_depth: u32,
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
    /// True while the reader is consuming an embedded/on-disk `.elc'
    /// stream (`;ELC' magic).  `#[...]' byte-code objects read in this
    /// window are dumped compiled functions: they get `compiled' set so
    /// `subrp'/`type-of' report GNU's nativecomp surface, exactly like
    /// the interpreted `compiled' lambdas they replace.
    pub reading_elc: bool,
    /// True while `Interp::new' is loading the boot libraries — the
    /// AOT `lisp-elc' twins apply only during this window (post-boot
    /// `load'/'require' of the same names resolves to the `.el'
    /// source, keeping user-visible `symbol-file'/`load-history'
    /// semantics and avoiding compiled-twin staleness for files whose
    /// compiled forms assume GNU-only machinery).
    pub boot_loading: bool,
    /// Nesting depth of `bytecode::exec' frames — only for the
    /// REMACS_BC_DEPTH debug escape hatch.
    pub bc_depth: usize,
    /// Number of in-flight expander calls running in a
    /// compiled-equivalent context (a `dumped'/bundled lambda or a
    /// bundled file load).  GNU byte-compiles these libraries, so their
    /// macroexpansion ran at compile time — in a world without advice.
    /// While nonzero, `callable_function'/`form_function' resolve
    /// symbols to their unadvised base so expander code like
    /// `letrec''s (mapcar #'car ...) doesn't see advice added at
    /// runtime (which would corrupt expansions — e.g. advice on `car'
    /// turning the binder list into ((ad x) ...)).
    pub expand_unadvised_depth: usize,
    /// Macro expansion cache: call-site cons pointer -> (generation,
    /// fn cell, expansion, pin).  GNU re-expands macro forms on every
    /// interpreted evaluation, which is fine when the code is actually
    /// byte-compiled; remacs interprets the dumped libraries, so a
    /// `cl-typep'/`pcase' inside a hot function (eieio-core's
    /// `eieio-oref-default') re-expanded per call makes `defclass'
    /// take seconds.  The pin holds the call-site cons alive so its
    /// `Rc' pointer stays unique.  Entries validate on the current
    /// `macro_gen' plus `eq' identity of the resolved fn cell.
    pub macro_cache: HashMap<usize, (u64, Value, Value, Value)>,
    /// Generation counter bumped by `fset'/`put_prop'/`setplist'/
    /// `fmakunbound' — any global change that could alter expansion
    /// output (expanders consult symbol properties like
    /// `pcase-defmacro', `cl-deftype-satisfies', compiler macros).
    /// Cheap coarse invalidation: entries only re-expand once per
    /// generation.
    pub macro_gen: u64,
    /// Memoized `macroexpand_all' results keyed by input-cons identity.
    /// `pcase' duplicates subforms into its expansion, so eager
    /// `macroexpand-all' on a large file re-expands shared structure
    /// exponentially; caching each cons's expansion collapses that.
    /// Cleared when `macro_gen' bumps (any definition/property change).
    /// Each entry stores the input form so its `Rc' keeps the cons —
    /// and its address — alive, preventing a recycled cons pointer
    /// from matching a stale entry.
    pub macroexpand_all_cache: std::cell::RefCell<(u64, HashMap<usize, (Value, Value)>)>,
    /// GNU's `noninteractive_need_newline`: set when batch stdout was
    /// written, so the next stderr message is preceded by a newline.
    pub stderr_need_newline: bool,
    /// Last character written to the real output sink; used by
    /// `(terpri nil t)'-style BOL checks (GNU's print_position).
    pub out_last_char: Option<char>,
    /// The interpreter's time-zone rule, set by `set-time-zone-rule'
    /// (GNU's `environ'/"TZ" flip).  We keep it per-`Interp' instead of
    /// touching the process `TZ' so parallel interpreters (tests) see
    /// independent wall clocks — `None' means the ambient system zone.
    pub tz_rule: Option<String>,
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
    /// Command-loop level (remacs: `recursion_depth') recorded when the
    /// minibuffer at each depth was opened — GNU's
    /// `Vcommand_loop_level_list'.  `minibuffer-innermost-command-loop-p'
    /// compares entry N against the current level.
    pub minibuf_cll: Vec<usize>,
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
    /// GNU's "new-frame defaults" lfaces — attrs set via
    /// `(set-face-attribute FACE t ...)'; read by `face-attribute'/`
    /// internal-get-lisp-face-attribute' with FRAME=t.
    pub face_t_table: Vec<(String, Value)>,
    /// Live process objects (`Value::Process`), including finished ones
    /// until `delete-process`.
    pub processes: Vec<crate::lisp::value::ProcessRef>,
    /// Tree-sitter parsers/nodes/queries/languages (`treesit-*').
    pub treesit: crate::lisp::builtins::treesit::TsState,
    /// File-notification (kqueue) watch table — GNU's `watch_list'
    /// from `src/kqueue.c'.
    pub filenotify: crate::lisp::builtins::filenotify::FnState,
    /// GNU's `Vcharset_ordered_list`/`Vcharset_non_preferred_head':
    /// (ORDERED-NAMES, NON-PREFERRED-INDEX).  Empty ORDERED-NAMES means
    /// the built-in -Q default (`GNU_CHARSET_PRIORITY`, boundary 16).
    /// `set-charset-priority' rebuilds both; `define-charset-internal'
    /// appends non-supplementary charsets before supplementary ones.
    pub charset_order: std::cell::RefCell<(Vec<String>, usize)>,
    /// Memoized `char-charset' winners for the default order
    /// (invalidated when the order or any charset definition changes).
    pub char_charset_cache: std::cell::RefCell<HashMap<i64, String>>,
    /// Interned id of `variable-alias' — caches `Symbol::var_alias'
    /// updates in `put_prop'/`f_put'/`f_setplist' so `symbol_value'
    /// skips the intern lookup and the plist walk.
    pub var_alias_prop: SymId,
    /// D-Bus bus connections and registered fds — GNU's
    /// `xd_registered_buses' / `xd_registered_fds' from `src/dbusbind.c'.
    pub dbus: crate::lisp::builtins::dbus::DbusState,
    /// Charset name → plist (`define-charset` / `set-charset-plist`).
    pub charsets: Vec<(String, Value)>,
    /// Charset alias → canonical name (`define-charset-alias`).
    pub charset_aliases: Vec<(String, String)>,
    /// GNU `iso_charset_table' overrides installed by `set-iso-charset'
    /// — (dimension, chars-96-flag, final-char) → charset name.
    pub iso_charset_overrides: Vec<(i64, bool, i64, String)>,
    /// Lazily parsed `etc/charsets/*.map' tables and maps installed
    /// by `unify-charset' (key `charset:NAME'); see builtins/charset.rs.
    pub(crate) charset_maps: std::cell::RefCell<
        HashMap<String, std::rc::Rc<crate::lisp::builtins::charset::CharsetTable>>,
    >,
    /// Coding systems defined via `define-coding-system-internal`.
    pub extra_coding_systems: Vec<String>,
    /// Coding-system aliases added via `define-coding-system-alias'
    /// at runtime: (ALIAS, CANONICAL-NAME) pairs; `coding-system-base'
    /// resolves ALIAS to CANONICAL's group head.
    pub extra_coding_aliases: Vec<(String, String)>,
    /// Current `terminal-coding-system' value (nil after set-nil).
    pub terminal_coding: Value,
    /// Current `keyboard-coding-system' value (no-conversion after
    /// set-nil).
    pub keyboard_coding: Value,
    /// Key events queued by `execute-kbd-macro'; the key loop replays
    /// them ahead of real input.
    pub macro_replay: std::collections::VecDeque<i128>,
    /// Whether the key currently being dispatched came from macro
    /// replay (replayed keys are not re-recorded).
    pub macro_replaying: bool,
    /// A command error while replaying a macro aborts the macro with
    /// the error (GNU `command_loop_2' propagates the signal out of
    /// `execute-kbd-macro').  `dispatch_key_core' stashes the flow
    /// here; the drainer (or the front-end loop) then propagates it.
    pub macro_pending_flow: Option<crate::lisp::error::Flow>,
    /// True while `f_execute_kbd_macro' is draining the replay queue
    /// synchronously (batch + nested calls).  Errors are stashed to
    /// `macro_pending_flow' instead of only echoed.
    pub macro_strict: bool,
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
    /// Canonical subr for each symbol that has a dedicated GNU
    /// byte-code opcode (car/cdr/list/concat/...).  Compiled code's
    /// `(car x)' never touches `symbol-function', so advising such a
    /// primitive can't recurse into the advice machinery itself —
    /// interpreted remacs library code (which stands in for GNU's
    /// .elc bodies) calls these via the same direct dispatch when
    /// `dumped_call_depth' is nonzero.
    pub byte_op_subrs: Option<HashMap<SymId, &'static crate::lisp::value::Subr>>,
    /// `set-char-table-parent' registry: record identity → parent table.
    /// Char-table parents live outside the record so existing record
    /// layouts are untouched.  The Weak guards against a dead record's
    /// address being reused by a different char-table (stale entries
    /// must not alias onto the new allocation).
    pub char_table_parents: Vec<(usize, std::rc::Weak<std::cell::RefCell<Vec<Value>>>, Value)>,
    /// Char-table default values, same registry style as parents
    /// (record identity → defalt).  GNU has no Lisp accessor for the
    /// defalt; it shows in `#^[...]' printing and feeds
    /// `char-table-range' misses.
    pub char_table_defalts: Vec<(usize, std::rc::Weak<std::cell::RefCell<Vec<Value>>>, Value)>,
    /// State snapshot from the last nil-arg `frame-or-buffer-changed-p'
    /// call (GNU `frame_and_buffer_state'): frame+buffer identity/flags,
    /// not a tick hash — see `frame_buffer_state' in builtins/misc.rs.
    pub frame_state_seen: Option<Vec<Value>>,
    /// Cached debug env vars — reading them per eval/call showed up
    /// hot (getenv does a linear environ scan + OsString alloc).
    /// Checked once at `Interp::new'.
    pub dbg_flags: u32,
    /// Cached REMACS_BT_AT limit (0 = unset).
    pub bt_at_limit: usize,
    /// Per-subr wall-time accumulator, enabled by REMACS_SUBR_PROF.
    /// (name → (calls, total ns)).
    pub subr_prof: std::collections::HashMap<&'static str, (u64, u128)>,
    /// Live Lisp call frames `(FUNCTION . ARGS)', outermost first.
    /// Pushed by `apply' so `mapbacktrace'/backtrace internals can
    /// walk the stack like GNU's specpdl entries do.
    pub lisp_stack: Vec<(Value, Argv)>,
    /// Bound names for compiled lambdas that were `defalias'd while
    /// nameless (`#[...]' literals from `.elc' files).  Entries pair
    /// a `Weak' with the name: `Weak::upgrade' failing means the
    /// pointer was freed and reused, so stale names never reattach
    /// to a different lambda.
    pub compiled_fn_names: std::cell::RefCell<Vec<(std::rc::Weak<Lambda>, String)>>,
    /// Debug: the raw form whose head symbol call is in progress.
    pub cur_head_form: Value,
    /// `lisp_stack' snapshot taken when the last signal was raised —
    /// the frames GNU's batch debugger prints after `Error:' (the live
    /// stack itself has already unwound by the time main sees it).
    pub last_error_stack: std::cell::RefCell<Vec<(Value, Argv)>>,
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
    pub unibyte_strings: HashMap<usize, std::rc::Weak<std::cell::RefCell<String>>>,
    /// Strings produced by multibyte decoders — `multibyte-string-p'
    /// is t for them even when their contents are pure ASCII.
    pub multibyte_strings: HashMap<usize, std::rc::Weak<std::cell::RefCell<String>>>,
    /// Text properties attached to string objects: pointer identity →
    /// (weak backref, sorted disjoint intervals (start, end, plist)).
    /// GNU keeps dead interval boundaries after property removal, so
    /// a true interval model is needed (not last-write-wins).  The
    /// Weak guards against address reuse like the unibyte map.
    pub string_props: HashMap<
        usize,
        (
            std::rc::Weak<std::cell::RefCell<String>>,
            Vec<(usize, usize, Vec<Value>)>,
        ),
    >,
    /// GNU's `zero_vector': the single shared 0-length vector —
    /// `(vector)', `[]' and `(make-vector 0)' are all `eq' to it.
    pub zero_vector: Value,
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

/// Find something under `<prefix>/share/` for the emacs binary at EXE.
/// CHECK is called with each candidate share dir. Wrapper scripts
/// (`exec /real/emacs`, as in Nixpkgs' emacsWithPackages) are unwrapped
/// up to DEPTH hops.
fn find_under_emacs_share(
    exe: &std::path::Path,
    depth: u8,
    check: &dyn Fn(&std::path::Path) -> Option<String>,
) -> Option<String> {
    let exe = std::fs::canonicalize(exe).ok()?;
    if let Some(prefix) = exe.parent().and_then(|p| p.parent()) {
        if let Some(hit) = check(&prefix.join("share")) {
            return Some(hit);
        }
        // Nixpkgs with-packages wrappers symlink share/* into the real
        // emacs package — follow any of them to the real prefix.
        if let Ok(rd) = std::fs::read_dir(prefix.join("share")) {
            for e in rd.flatten() {
                if let Ok(real) = std::fs::canonicalize(e.path()) {
                    if let Some(share) = real.parent() {
                        if let Some(hit) = check(share) {
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
                    if let Some(hit) = find_under_emacs_share(&cand, depth + 1, check) {
                        return Some(hit);
                    }
                }
            }
        }
    }
    None
}

/// Find `<prefix>/share/emacs/<ver>/etc/` (containing DOC) for the
/// emacs binary at EXE.
fn doc_dir_of_emacs(exe: &std::path::Path, depth: u8) -> Option<String> {
    find_under_emacs_share(exe, depth, &|share| doc_dir_under(&share.join("emacs")))
}

/// Find `<prefix>/share/info/` for the emacs binary at EXE — GNU's
/// configure-time PATH_INFO.
fn info_dir_of_emacs(exe: &std::path::Path, depth: u8) -> Option<String> {
    find_under_emacs_share(exe, depth, &|share| {
        let dir = share.join("info");
        dir.is_dir().then(|| format!("{}/", dir.display()))
    })
}

/// Scan PATH (plus conventional dirs) for an `emacs` binary and apply F.
fn find_emacs_on_path(f: &dyn Fn(&std::path::Path) -> Option<String>) -> Option<String> {
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
        .find_map(|dir| f(&dir.join("emacs")))
}

/// `load_library` wrapper for the init sequence: with REMACS_BOOT_PROF
/// set, prints per-library wall time to stderr so the startup profile
/// can be decomposed.
fn boot_lib(i: &mut Interp, name: &str) {
    let prof = std::env::var_os("REMACS_BOOT_PROF").is_some();
    let t = prof.then(std::time::Instant::now);
    let r = crate::lisp::load::load_library(i, name);
    if let Some(t0) = t {
        eprintln!("[boot] {name}: {}ms", t0.elapsed().as_millis());
    }
    if let Err(e) = &r {
        // Print symbol names, not SymIds, so boot failures are legible.
        let msg = match e {
            crate::lisp::Flow::Signal(s, data, _) => {
                format!("{} {}", i.prin1_to_string(s), i.prin1_to_string(data))
            }
            other => format!("{other:?}"),
        };
        eprintln!(
            "[boot-err] {name}: {msg} | head={}",
            i.princ_to_string(&i.cur_head_form)
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
    let _ = r;
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
            // `subr-x' is deliberately absent: GNU -Q reports
            // `(featurep 'subr-x)' => nil (subr-x.el isn't in loadup),
            // even though the amalgamated file's dump-side defs are
            // loaded via `boot_lib' below.  Registering the feature
            // would make `(require 'subr-x)' a no-op and leave the
            // real subr-x names (hash-table-keys, thread-first, ...)
            // void — qa_subrx2's GNU-verified checks.
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
            catch_tags: Vec::new(),
            lexenv: None,
            file_declared: RefCell::new(HashSet::default()),
            dbg_flags: {
                let mut f = 0u32;
                for (var, bit) in [
                    ("DBG_FUNCALL", 0x01u32),
                    ("REMACS_TRACE_DEPTH", 0x02),
                    ("REMACS_TRACE_CALL", 0x04),
                    ("REMACS_EVAL_WATCH", 0x08),
                    ("DBG_VOID", 0x10),
                    ("WHILE_WATCH", 0x20),
                    ("REMACS_TRACE_BC", 0x40),
                    ("DBG_DOTTED", 0x80),
                    ("DBG_SETCONST", 0x100),
                    ("REMACS_FORM_PROF", 0x200),
                    ("REMACS_MCE_PROF", 0x400),
                    ("REMACS_TRACE_AUTOLOAD", 0x800),
                    ("REMACS_TRACE_ERR", 0x1000),
                    ("WATCH_ROLLBACK", 0x2000),
                    ("WATCH_MAKUNBOUND", 0x4000),
                    ("REMACS_LOAD_POS", 0x8000),
                    ("REMACS_TRACE_EVAL", 0x10000),
                    ("REMACS_LOAD_DEBUG", 0x20000),
                    ("WATCH_TERM_CODING", 0x40000),
                    ("REMACS_TRACE_DECLARE", 0x80000),
                    ("REMACS_UNIPROP_DEBUG", 0x100000),
                    ("PRELUDE_TRACE", 0x200000),
                    ("REMACS_BT_EOF", 0x400000),
                    ("REMACS_BT_ERR", 0x800000),
                ] {
                    if std::env::var_os(var).is_some() {
                        f |= bit;
                    }
                }
                f
            },
            bt_at_limit: std::env::var("REMACS_BT_AT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            buffers: crate::buffer::BufferSet::new(),
            current_buffer: 0,
            features: feature_syms,
            output: None,
            echo_message: String::new(),
            max_lisp_eval_depth: 1600,
            eval_depth: 0,
            explicit_eval_depth: 0,
            quit_flag: false,
            standard_output_sym,
            undo_inhibit_sym: 0,
            bfn_sym: 0,
            command_args: Vec::new(),
            interactive_call_depth: 0,
            defining_symbol: None,
            read_expression_history: Value::Nil,
            recursion_depth: 0,
            executing_kbd_macro: false,
            last_error: None,
            output_buffer: String::new(),
            capture_output: false,
            match_data: None,
            // `main' argv-scans --batch/--script before `Interp::new'
            // (GNU does the same in its C main), so startup-time Lisp
            // already sees the batch state.
            noninteractive: std::env::var_os("REMACS_NONINTERACTIVE").is_some(),
            default_obarray: None,
            loading_dumped: false,
            dumped_call_depth: 0,
            reading_elc: false,
            boot_loading: true,
            bc_depth: 0,
            macroexp_call_depth: 0,
            expand_unadvised_depth: 0,
            macro_cache: HashMap::default(),
            macro_gen: 0,
            macroexpand_all_cache: std::cell::RefCell::new((0, HashMap::default())),
            stderr_need_newline: false,
            out_last_char: None,
            tz_rule: None,
            frames: Vec::new(),
            selected_frame: None,
            quit_editor: false,
            minibuf_reader: None,
            batch_stdin: None,
            minibuf_level: 0,
            minibuf_list: Vec::new(),
            minibuf_cll: Vec::new(),
            minibuf_prompts: Vec::new(),
            minibuf_catching_exit: false,
            minibuf_exited: false,
            minibuf_pending_flow: None,
            minibuf_exit_fn: None,
            face_table: Vec::new(),
            face_t_table: Vec::new(),
            processes: Vec::new(),
            treesit: crate::lisp::builtins::treesit::TsState::new(),
            filenotify: crate::lisp::builtins::filenotify::FnState::new(),
            dbus: crate::lisp::builtins::dbus::DbusState::new(),
            charsets: Vec::new(),
            charset_order: std::cell::RefCell::new((Vec::new(), 16)),
            char_charset_cache: std::cell::RefCell::new(HashMap::default()),
            var_alias_prop: 0,
            charset_aliases: Vec::new(),
            iso_charset_overrides: Vec::new(),
            charset_maps: std::cell::RefCell::new(HashMap::default()),
            extra_coding_systems: Vec::new(),
            extra_coding_aliases: Vec::new(),
            terminal_coding: Value::Nil,
            keyboard_coding: Value::Nil,
            macro_replay: std::collections::VecDeque::new(),
            macro_replaying: false,
            macro_pending_flow: None,
            macro_strict: false,
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
            byte_op_subrs: None,
            char_table_parents: Vec::new(),
            char_table_defalts: Vec::new(),
            frame_state_seen: None,
            lisp_stack: Vec::new(),
            compiled_fn_names: std::cell::RefCell::new(Vec::new()),
            subr_prof: std::collections::HashMap::new(),
            cur_head_form: Value::Nil,
            last_error_stack: std::cell::RefCell::new(Vec::new()),
            cpu_profiler: false,
            terminal: None,
            terminal_params: Vec::new(),
            x_display_attempted: false,
            color_db_init: false,
            unibyte_strings: HashMap::default(),
            multibyte_strings: HashMap::default(),
            string_props: HashMap::default(),
            zero_vector: Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))),
        };
        let _init_t0 = std::time::Instant::now();
        crate::lisp::builtins::install(&mut interp);
        if std::env::var_os("REMACS_BOOT_PROF").is_some() {
            eprintln!(
                "[init] builtins::install: {}ms",
                _init_t0.elapsed().as_millis()
            );
        }
        crate::buffer::install_primitives(&mut interp);
        interp.define_error_conditions();
        interp.define_special_variables();
        // Undo recording writes into the `buffer-undo-list' buffer-local
        // variable; buffers need the symbol's id before creation.
        let us = interp.intern("buffer-undo-list");
        interp.buffers.set_undo_sym(us);
        interp.undo_inhibit_sym = interp.intern("undo-inhibit-record-point");
        interp.bfn_sym = interp.intern("buffer-file-name");
        interp.var_alias_prop = interp.intern("variable-alias");
        // The initial buffers every Emacs session has, in GNU's
        // buffer-list order: (scratch Minibuf-0 Messages load
        // Warnings). New buffers append at the end of the order.
        let scratch = interp.buffers.create_exact_nolast("*scratch*");
        interp.set_current_buffer(scratch);
        {
            // GNU's `*scratch*' carries buffer-local `lexical-binding'
            // = t (set up by startup.el's `eval-buffer' on scratch);
            // this also makes `eval_str' (which samples the buffer-
            // local value) evaluate the prelude lexically.
            let lb = interp.intern("lexical-binding");
            interp
                .buffers
                .get(scratch)
                .unwrap()
                .borrow_mut()
                .locals
                .insert(lb, Value::t());
        }
        let minibuf = interp.buffers.create_exact(" *Minibuf-0*");
        let messages = interp.buffers.create_exact("*Messages*");
        // No ` *load*' buffer: GNU streams loads off the file descriptor
        // (`readevalloop'), so `buffer-list' at -Q holds only
        // scratch/Minibuf-0/Messages.
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
        // Locale-derived tty defaults: UTF-8 locales → utf-8-unix,
        // C/POSIX → no-conversion (mirrors GNU's locale coding init —
        // the diff harness runs under LC_ALL=C).
        let locale = std::env::var("LC_ALL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("LC_CTYPE").ok().filter(|s| !s.is_empty()))
            .or_else(|| std::env::var("LANG").ok())
            .unwrap_or_default();
        let kbd = if locale.to_ascii_lowercase().contains("utf") || locale.is_empty() {
            "utf-8-unix"
        } else {
            "no-conversion"
        };
        let u8u = interp.intern(kbd);
        // GNU: a non-UTF-8 tty defaults to `undecided', which
        // `terminal-coding-system' reports as nil (coding.c:10736).
        let tcd = interp.intern(if kbd == "utf-8-unix" {
            "utf-8-unix"
        } else {
            "undecided"
        });
        interp.terminal_coding = Value::Sym(tcd);
        interp.keyboard_coding = Value::Sym(u8u);
        // Load the Lisp prelude (subr.el subset). Errors here indicate a
        // broken prelude, but don't abort startup.
        let prelude_max: usize = std::env::var("PRELUDE_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(usize::MAX);
        // Prelude-folded vars for libraries GNU never dumps — the
        // `*-mode-map'/`*-mode-hook'/`*-mode-syntax-table'/
        // `*-mode-abbrev-table' quads from folded
        // `define-derived-mode' calls, face-remap leftovers and a
        // few function-name symbols that got stray value cells.
        // GNU -Q reports all of these unbound; the final pass in
        // `new' voids their value cells (and buffer-local copies).
        const GNU_VOID_VARS: &[&str] = &[
            "after-insert-file-set-coding-functions",
            "antlr-mode-abbrev-table",
            "antlr-mode-hook",
            "antlr-mode-map",
            "antlr-mode-syntax-table",
            "antlr-v4-mode-abbrev-table",
            "antlr-v4-mode-hook",
            "antlr-v4-mode-map",
            "antlr-v4-mode-syntax-table",
            "archive-mode-abbrev-table",
            "archive-mode-hook",
            "archive-mode-map",
            "archive-mode-syntax-table",
            "ask-about-buffer-names",
            "authinfo-mode-abbrev-table",
            "authinfo-mode-hook",
            "authinfo-mode-map",
            "authinfo-mode-syntax-table",
            "auto-save-mode",
            "awk-mode-abbrev-table",
            "awk-mode-hook",
            "awk-mode-map",
            "awk-mode-syntax-table",
            "before-load-history",
            "bibtex-mode-abbrev-table",
            "bibtex-mode-hook",
            "bibtex-mode-map",
            "bibtex-mode-syntax-table",
            "bovine-grammar-mode-abbrev-table",
            "bovine-grammar-mode-hook",
            "bovine-grammar-mode-map",
            "bovine-grammar-mode-syntax-table",
            "buffer-face-mode",
            "cl-print-object",
            "cl-old-struct-compat-mode-hook",
            "internal-lisp-face-attributes",
            "buffer-face-mode-face",
            // Prelude-folded cells from files GNU doesn't run at
            // dump time (cl-macs/cl-lib constants, pcase caches,
            // rect vars) — GNU -Q leaves their value cells void.
            "cl--active-block-names",
            "cl--cnm",
            "cl--cnm-args",
            "cl--cnm-name",
            "cl--cnm-ok",
            "cl--labels-convert-cache",
            "cl--labels-magic",
            "cl--lambda-list-keywords",
            "cl--loop--into-vars",
            "cl--loop-keywords",
            "cl--safe-funcs",
            "cl--simple-funcs",
            "cl-float-epsilon",
            "cl-float-negative-epsilon",
            "cl-least-negative-float",
            "cl-least-negative-normalized-float",
            "cl-least-positive-float",
            "cl-least-positive-normalized-float",
            "cl-most-negative-float",
            "cl-most-positive-float",
            "cl-struct-built-in-class",
            "cl-struct-cl--class",
            "cl-struct-cl--generic",
            "cl-struct-cl--generic-generalizer",
            "cl-struct-cl--generic-method",
            "cl-struct-cl-derived-type-class",
            "cl-struct-cl-slot-descriptor",
            "cl-struct-cl-structure-class",
            "cl-struct-cl-structure-object",
            "cl-struct-oclosure--class",
            "cl-struct-registerv",
            "cl-struct-timer",
            "cl-struct-xref-elisp-location",
            "pcase--dontcare-upats",
            "pcase--dontwarn-upats",
            "pcase--find-macro-def-regexp",
            "pcase--memoize",
            "pcase--subtype-bitsets",
            "rectangle--mark-crutches",
            "rectangle-mark-mode-hook",
            "rectangle-mark-mode-map",
            "rectangle-mark-mode",
            "buffer-face-mode-hook",
            "buffer-face-mode-remapping",
            "button-mode-map",
            "byte-compile-call-tree-sort",
            "byte-compile-cond-use-jump-table",
            "byte-compile-debug",
            "byte-compile-delete-errors",
            "byte-compile-docstring-max-column",
            "byte-compile-dynamic",
            "byte-compile-dynamic-docstrings",
            "byte-compile-error-on-warn",
            "byte-compile-generate-call-tree",
            "byte-compile-optimize",
            "byte-compile-verbose",
            "byte-compile-warnings",
            "c++-mode-abbrev-table",
            "c++-mode-hook",
            "c++-mode-map",
            "c++-mode-syntax-table",
            "c-mode-abbrev-table",
            "c-mode-hook",
            "c-mode-map",
            "c-mode-syntax-table",
            "calculate-lisp-indent-last-sexp",
            "change-log-mode-abbrev-table",
            "change-log-mode-hook",
            "change-log-mode-map",
            "change-log-mode-syntax-table",
            "command-args",
            "command-line-normalized-file-name",
            "compilation-context-lines",
            "compilation-current-error",
            "compilation-mode-abbrev-table",
            "compilation-mode-map",
            "compilation-mode-syntax-table",
            "connection-local-criteria",
            "connection-local-criteria-alist",
            "connection-local-default-application",
            "connection-local-profile-alist",
            "connection-local-profile-name-for-setq",
            "connection-local-variables-alist",
            "csharp-mode-abbrev-table",
            "csharp-mode-hook",
            "csharp-mode-map",
            "csharp-mode-syntax-table",
            "css-mode-abbrev-table",
            "css-mode-hook",
            "css-mode-map",
            "css-mode-syntax-table",
            "daemon-socket",
            "dcl-mode-abbrev-table",
            "dcl-mode-hook",
            "dcl-mode-map",
            "dcl-mode-syntax-table",
            "delete-selection-mode-hook",
            "describe-symbol-backends",
            "diff-mode-abbrev-table",
            "diff-mode-hook",
            "diff-mode-map",
            "diff-mode-syntax-table",
            "display-battery-mode-hook",
            "display-time-mode-hook",
            "dns-mode-abbrev-table",
            "dns-mode-hook",
            "dns-mode-map",
            "dns-mode-syntax-table",
            "doc-view-mode-abbrev-table",
            "doc-view-mode-hook",
            "doc-view-mode-map",
            "doc-view-mode-syntax-table",
            "doctex-mode-abbrev-table",
            "doctex-mode-hook",
            "doctex-mode-map",
            "doctex-mode-syntax-table",
            "dsssl-mode-abbrev-table",
            "dsssl-mode-hook",
            "dsssl-mode-map",
            "dsssl-mode-syntax-table",
            "ebrowse-tree-mode-abbrev-table",
            "ebrowse-tree-mode-hook",
            "ebrowse-tree-mode-map",
            "ebrowse-tree-mode-syntax-table",
            "emacs--start-time",
            "emacs-program-version",
            "erts-mode-abbrev-table",
            "erts-mode-hook",
            "erts-mode-map",
            "erts-mode-syntax-table",
            "eval-depth",
            "f90-mode-abbrev-table",
            "f90-mode-hook",
            "f90-mode-map",
            "f90-mode-syntax-table",
            "fancy-about-tail",
            "fancy-startup-tail",
            "fill-nospace-between-words",
            "find-alias-regexp",
            "find-face-regexp",
            "find-feature-regexp",
            "find-function--read-history-face",
            "find-function--read-history-function",
            "find-function--read-history-library",
            "find-function--read-history-variable",
            "find-function-C-source-directory",
            "find-function-after-hook",
            "find-function-recenter-line",
            "find-function-regexp",
            "find-function-regexp-alist",
            "find-function-space-re",
            "find-library-include-other-files",
            "find-library-source-path",
            "find-variable-regexp",
            "font-lock-maximum-size",
            "fontification-error",
            "fontification-missing",
            "fortran-mode-abbrev-table",
            "fortran-mode-hook",
            "fortran-mode-map",
            "fortran-mode-syntax-table",
            "frame-initial-frame-alist",
            "functions-exhausted",
            "gc-message-percentage",
            "gdb-script-mode-abbrev-table",
            "gdb-script-mode-hook",
            "gdb-script-mode-map",
            "gdb-script-mode-syntax-table",
            "global-text-scale-adjust--default-height",
            "global-text-scale-adjust--increment-factor",
            "global-text-scale-adjust-limits",
            "global-text-scale-adjust-resizes-frames",
            "handle-shift-selection",
            "help-back-label",
            "help-clean-buttons",
            "help-for-help-use-variable-pitch",
            "help-forward-label",
            "help-mode--current-data",
            "help-mode-abbrev-table",
            "help-mode-hook",
            "help-mode-map",
            "help-mode-menu",
            "help-mode-syntax-table",
            "help-mode-tool-bar-map",
            "help-xref-customization-group-regexp",
            "help-xref-following",
            "help-xref-forward-stack",
            "help-xref-forward-stack-item",
            "help-xref-info-regexp",
            "help-xref-man-regexp",
            "help-xref-mule-regexp",
            "help-xref-stack",
            "help-xref-stack-forward-item",
            "help-xref-stack-item",
            "help-xref-symbol-regexp",
            "help-xref-url-regexp",
            "icon-mode-abbrev-table",
            "icon-mode-hook",
            "icon-mode-map",
            "icon-mode-syntax-table",
            "icon-preference",
            "idl-mode-abbrev-table",
            "idl-mode-hook",
            "idl-mode-map",
            "idl-mode-syntax-table",
            "ido-ignore-item-temp-list",
            "image-mode-abbrev-table",
            "image-mode-hook",
            "image-mode-map",
            "image-mode-syntax-table",
            "inhibit-variable-watchers",
            "input-method-inactivate-hook",
            "java-mode-abbrev-table",
            "java-mode-hook",
            "java-mode-map",
            "java-mode-syntax-table",
            "javascript-mode-abbrev-table",
            "javascript-mode-hook",
            "javascript-mode-map",
            "javascript-mode-syntax-table",
            "js-json-mode-abbrev-table",
            "js-json-mode-hook",
            "js-json-mode-map",
            "js-json-mode-syntax-table",
            "js-mode-abbrev-table",
            "js-mode-hook",
            "js-mode-map",
            "js-mode-syntax-table",
            "latex-mode-abbrev-table",
            "latex-mode-hook",
            "latex-mode-map",
            "latex-mode-syntax-table",
            "less-css-mode-abbrev-table",
            "less-css-mode-hook",
            "less-css-mode-map",
            "less-css-mode-syntax-table",
            "load--fixup-all-elns",
            "load-convert-to-unibyte",
            "loadhist-unload-filename",
            "lua-mode-abbrev-table",
            "lua-mode-hook",
            "lua-mode-map",
            "lua-mode-syntax-table",
            "m2-mode-abbrev-table",
            "m2-mode-hook",
            "m2-mode-map",
            "m2-mode-syntax-table",
            "mail-citation-prefix",
            "mail-mode-abbrev-table",
            "mail-mode-hook",
            "mail-mode-map",
            "mail-mode-syntax-table",
            "mail-send-hook",
            "makefile-automake-mode-abbrev-table",
            "makefile-automake-mode-hook",
            "makefile-automake-mode-map",
            "makefile-automake-mode-syntax-table",
            "makefile-bsdmake-mode-abbrev-table",
            "makefile-bsdmake-mode-hook",
            "makefile-bsdmake-mode-map",
            "makefile-bsdmake-mode-syntax-table",
            "makefile-gmake-mode-abbrev-table",
            "makefile-gmake-mode-hook",
            "makefile-gmake-mode-map",
            "makefile-gmake-mode-syntax-table",
            "makefile-imake-mode-abbrev-table",
            "makefile-imake-mode-hook",
            "makefile-imake-mode-map",
            "makefile-imake-mode-syntax-table",
            "makefile-makepp-mode-abbrev-table",
            "makefile-makepp-mode-hook",
            "makefile-makepp-mode-map",
            "makefile-makepp-mode-syntax-table",
            "makefile-mode-abbrev-table",
            "makefile-mode-hook",
            "makefile-mode-map",
            "makefile-mode-syntax-table",
            "metafont-mode-abbrev-table",
            "metafont-mode-hook",
            "metafont-mode-map",
            "metafont-mode-syntax-table",
            "metapost-mode-abbrev-table",
            "metapost-mode-hook",
            "metapost-mode-map",
            "metapost-mode-syntax-table",
            "mhtml-mode-abbrev-table",
            "mhtml-mode-hook",
            "mhtml-mode-map",
            "mhtml-mode-syntax-table",
            "minibuffer-inactive-mode",
            "mixal-mode-abbrev-table",
            "mixal-mode-hook",
            "mixal-mode-map",
            "mixal-mode-syntax-table",
            "mouse-wheel-down-alternate-event",
            "mouse-wheel-left-alternate-event",
            "mouse-wheel-right-alternate-event",
            "mouse-wheel-up-alternate-event",
            "native-comp-driver-options",
            "network-security-level",
            "nroff-mode-abbrev-table",
            "nroff-mode-hook",
            "nroff-mode-map",
            "nroff-mode-syntax-table",
            "obarray-size",
            "objc-mode-abbrev-table",
            "objc-mode-hook",
            "objc-mode-map",
            "objc-mode-syntax-table",
            "octave-mode-abbrev-table",
            "octave-mode-hook",
            "octave-mode-map",
            "octave-mode-syntax-table",
            "opascal-mode-abbrev-table",
            "opascal-mode-hook",
            "opascal-mode-map",
            "opascal-mode-syntax-table",
            "org-mode-abbrev-table",
            "org-mode-hook",
            "org-mode-map",
            "org-mode-syntax-table",
            "overriding-text-conversion-style",
            "pascal-mode-abbrev-table",
            "pascal-mode-hook",
            "pascal-mode-map",
            "pascal-mode-syntax-table",
            "perl-mode-abbrev-table",
            "perl-mode-hook",
            "perl-mode-map",
            "perl-mode-syntax-table",
            "pike-mode-abbrev-table",
            "pike-mode-hook",
            "pike-mode-map",
            "pike-mode-syntax-table",
            "plain-tex-mode-abbrev-table",
            "plain-tex-mode-hook",
            "plain-tex-mode-map",
            "plain-tex-mode-syntax-table",
            "print-readably",
            "print-unreadable",
            "prolog-mode-abbrev-table",
            "prolog-mode-hook",
            "prolog-mode-map",
            "prolog-mode-syntax-table",
            "ps-mode-abbrev-table",
            "ps-mode-hook",
            "ps-mode-map",
            "ps-mode-syntax-table",
            "python-mode-abbrev-table",
            "python-mode-hook",
            "python-mode-map",
            "python-mode-syntax-table",
            "read-symbol-positions-list",
            "redisplay--variables",
            "redisplay-dont-pause",
            "revert-buffer-preserve-modes",
            "rst-mode-abbrev-table",
            "rst-mode-hook",
            "rst-mode-map",
            "rst-mode-syntax-table",
            "ruby-mode-abbrev-table",
            "ruby-mode-hook",
            "ruby-mode-map",
            "ruby-mode-syntax-table",
            "scheme-mode-abbrev-table",
            "scheme-mode-hook",
            "scheme-mode-map",
            "scheme-mode-syntax-table",
            "scribe-mode-abbrev-table",
            "scribe-mode-hook",
            "scribe-mode-map",
            "scribe-mode-syntax-table",
            "scroll-lock-mode",
            "scss-mode-abbrev-table",
            "scss-mode-hook",
            "scss-mode-map",
            "scss-mode-syntax-table",
            "seconds-to-string",
            "ses-mode-abbrev-table",
            "ses-mode-hook",
            "ses-mode-map",
            "ses-mode-syntax-table",
            "sgml-mode-abbrev-table",
            "sgml-mode-hook",
            "sgml-mode-map",
            "sgml-mode-syntax-table",
            "sh-mode-abbrev-table",
            "sh-mode-hook",
            "sh-mode-map",
            "sh-mode-syntax-table",
            "shell-command-mode-hook",
            "sieve-mode-abbrev-table",
            "sieve-mode-hook",
            "sieve-mode-map",
            "sieve-mode-syntax-table",
            "simula-mode-abbrev-table",
            "simula-mode-hook",
            "simula-mode-map",
            "simula-mode-syntax-table",
            "slitex-mode-abbrev-table",
            "slitex-mode-hook",
            "slitex-mode-map",
            "slitex-mode-syntax-table",
            "snmp-mode-abbrev-table",
            "snmp-mode-hook",
            "snmp-mode-map",
            "snmp-mode-syntax-table",
            "snmpv2-mode-abbrev-table",
            "snmpv2-mode-hook",
            "snmpv2-mode-map",
            "snmpv2-mode-syntax-table",
            "sort-numeric-base",
            "sql-mode-abbrev-table",
            "sql-mode-hook",
            "sql-mode-map",
            "sql-mode-syntax-table",
            "srecode-template-mode-abbrev-table",
            "srecode-template-mode-hook",
            "srecode-template-mode-map",
            "srecode-template-mode-syntax-table",
            "tabify-regexp",
            "tar-mode-abbrev-table",
            "tar-mode-hook",
            "tar-mode-map",
            "tar-mode-syntax-table",
            "tcl-mode-abbrev-table",
            "tcl-mode-hook",
            "tcl-mode-map",
            "tcl-mode-syntax-table",
            "tex-mode-abbrev-table",
            "tex-mode-hook",
            "tex-mode-map",
            "tex-mode-syntax-table",
            "texinfo-mode-abbrev-table",
            "texinfo-mode-hook",
            "texinfo-mode-map",
            "texinfo-mode-syntax-table",
            "text-scale--pinch-start-scale",
            "text-scale-mode",
            "text-scale-mode-amount",
            "text-scale-mode-hook",
            "text-scale-mode-lighter",
            "text-scale-mode-remapping",
            "text-scale-mode-step",
            "text-scale-remap-header-line",
            "unload-feature-special-hooks",
            "unload-function-defs-list",
            "unsafep--safe-functions",
            "user-real-uid",
            "user-uid",
            "vc-mode-line-hook",
            "vera-mode-abbrev-table",
            "vera-mode-hook",
            "vera-mode-map",
            "vera-mode-syntax-table",
            "verilog-mode-abbrev-table",
            "verilog-mode-hook",
            "verilog-mode-map",
            "verilog-mode-syntax-table",
            "vhdl-mode-abbrev-table",
            "vhdl-mode-hook",
            "vhdl-mode-map",
            "vhdl-mode-syntax-table",
            "window-text-change-functions",
            "wisent-grammar-mode-abbrev-table",
            "wisent-grammar-mode-hook",
            "wisent-grammar-mode-map",
            "wisent-grammar-mode-syntax-table",
            "x-create-frame",
            "x-family-fonts",
            "x-load-color-file",
            "x-mouse-clip-rectangular",
            "x-select-font",
            "xml-mode-abbrev-table",
            "xml-mode-hook",
            "xml-mode-map",
            "xml-mode-syntax-table",
            // ucs-normalize.el's `eval-when-compile' tables/vars:
            // GNU's dump computes them at compile time, so the
            // non-`ucs-normalize-' helper bindings are void at -Q
            // (the `ucs-normalize-*' globals are real defvars —
            // they stay bound).
            "check-range",
            "combining-chars",
            "decomposition-pair-to-composition",
            "hfs-nfd-alist",
            "hfs-nfd-quick-check-list",
            "hfs-nfc-quick-check-list",
            "nfd-alist",
            "nfd-quick-check-list",
            "nfc-quick-check-list",
            "nfkc-quick-check-list",
            "nfkd-alist",
            "nfkd-quick-check-list",
            "non-starter-decompositions",
            "ucs-normalize-composition-exclusions",
        ];
        // Same treatment for prelude-folded *functions* GNU
        // leaves void at -Q (they live in dumped loaddefs
        // autoloads or real files — rebind on `require').
        const GNU_VOID_FNS_LATE: &[&str] = &[
            "`--pcase-macroexpander",
            "abbrev--expand-body",
            "abbrev--expand-wrapped",
            "add-remove--display-text-property",
            "advice--make-how-alist",
            "append-to-list",
            "apply-on-rectangle",
            "bidi--char-in-category-p",
            "bool-vector-length",
            "buffer-face-mode-invoke",
            "buffer-name-as-string",
            "buffer-substring-with-properties",
            "buffer-word-at-point",
            "byte-compile-warn-x",
            "car-or-marker-p",
            "char-table",
            "cl--add-function",
            "cl--adjoin",
            "cl--advice--apply",
            "cl--advice--link",
            "build-default-fontset-data",
            "cl--advice-place-code",
            "cl--alist-to-plist",
            "cl--arglist-args",
            // cl-macs inline helpers folded into the prelude —
            // GNU keeps them void at -Q (cl-macs isn't dumped).
            "cl--const-expr-p",
            "cl--const-expr-val",
            "cl--do-&aux",
            "cl--do-arglist",
            "cl--loop-intervals",
            "cl--loop-keymap-pairs",
            "cl--loop-keymap-seqs",
            "cl--loop-obarray-symbols",
            "cl--loop-overlays",
            "cl--pop2",
            "cl--sm-exp",
            "cl--sm-shadow",
            "cl--block-throw",
            "cl--block-throw--cmacro",
            "cl--block-wrapper",
            "cl--block-wrapper--cmacro",
            "cl--check-key",
            "cl--check-keys",
            "cl--check-match",
            "cl--check-test",
            "cl--check-test-nokey",
            "cl--compile-time-too",
            "cl--compiler-macro-adjoin",
            "cl--compiler-macro-assoc",
            "cl--compiler-macro-cXXr",
            "cl--compiler-macro-list*",
            "cl--compiler-macro-member",
            "cl--compiling-file",
            "cl--defalias",
            "cl--defun-1",
            "cl--delete-duplicates",
            "cl--derived-type-generalizers",
            "cl--do-proclaim",
            "cl--do-remf",
            "cl--do-subst",
            "cl--expand-do-loop",
            "cl--expr-contains",
            "cl--expr-contains-any",
            "cl--expr-depends-p",
            "cl--find-class",
            "cl--generic-dispatch",
            "cl--generic-prefill-dispatchers",
            "cl--keyfn",
            "cl--labels-convert",
            "cl--loop-action",
            "cl--loop-cond",
            "cl--loop-destruct",
            "cl--loop-destruct-accessors",
            "cl--loop-expand",
            "cl--loop-hash-pairs",
            "cl--map-intervals",
            "cl--map-overlays",
            "cl--mapcar-many",
            "cl--method-fn",
            "cl--method-more-specific-p",
            "cl--methods-with-qual",
            "cl--nsublis-rec",
            "cl--old-struct-type-of",
            "cl--optimize",
            "cl--parsing-keywords",
            "cl--position",
            "cl--prog",
            "cl--remove-function",
            "cl--safe-expr-p",
            "cl--self-tco",
            "cl--self-tco-on-form",
            "cl--set-buffer-substring",
            "cl--set-frame-visible-p",
            "cl--set-getf",
            "cl--set-substring",
            "cl--simple-expr-p",
            "cl--simple-exprs-p",
            "cl--sm-subst",
            "cl--spec-applicable-p",
            "cl--spec-more-specific-p",
            "cl--struct-register-class",
            "cl--struct-register-tag",
            "cl--struct-tag-var",
            "cl--sublis-rec",
            "cl--take",
            "cl--thread-expand",
            "cl--tree-equal-rec",
            "cl--type-parents",
            "cl-acons",
            "cl-adjoin",
            "cl-assert",
            "cl-assoc",
            "cl-assoc-if",
            "cl-assoc-if-not",
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
            "cl-ceiling",
            "cl-check-type",
            "cl-coerce",
            "cl-compiler-macroexpand",
            "cl-concatenate",
            "cl-constantly",
            "cl-copy-list",
            "cl-copy-seq",
            "cl-count",
            "cl-count-if",
            "cl-count-if-not",
            "cl-decf",
            "cl-declaim",
            "cl-declare",
            "cl-define-compiler-macro",
            "cl-defmacro",
            "cl-defstruct",
            "cl-defsubst",
            "cl-deftype",
            "cl-defun",
            "cl-delete",
            "cl-delete-duplicates",
            "cl-delete-if",
            "cl-delete-if-not",
            "cl-describe-type",
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
            "cl-endp",
            "cl-equalp",
            "cl-etypecase",
            "cl-eval-when",
            "cl-evenp",
            "cl-every",
            "cl-fifth",
            "cl-fill",
            "cl-find",
            "cl-find-class",
            "cl-find-if",
            "cl-find-if-not",
            "cl-first",
            "cl-flet",
            "cl-flet*",
            "cl-float-limits",
            "cl-floatp-safe",
            "cl-floor",
            "cl-fourth",
            "cl-fresh-line",
            "cl-function",
            "cl-gcd",
            "cl-gensym",
            "cl-gentemp",
            "cl-get",
            "cl-getf",
            "cl-intersection",
            "cl-isqrt",
            "cl-iter-defun",
            "cl-labels",
            "cl-lcm",
            "cl-ldiff",
            "cl-letf",
            "cl-letf*",
            "cl-list*",
            "cl-list-length",
            "cl-load-time-value",
            "cl-locally",
            "cl-loop",
            "cl-macrolet",
            "cl-make-random-state",
            "cl-map",
            "cl-mapc",
            "cl-mapcan",
            "cl-mapcar",
            "cl-mapcon",
            "cl-mapl",
            "cl-maplist",
            "cl-member",
            "cl-member-if",
            "cl-member-if-not",
            "cl-merge",
            "cl-minus",
            "cl-minusp",
            "cl-mismatch",
            "cl-mod",
            "cl-multiple-value-apply",
            "cl-multiple-value-bind",
            "cl-multiple-value-call",
            "cl-multiple-value-list",
            "cl-multiple-value-setq",
            "cl-nintersection",
            "cl-ninth",
            "cl-notany",
            "cl-notevery",
            "cl-nreconc",
            "cl-nset-difference",
            "cl-nset-exclusive-or",
            "cl-nsublis",
            "cl-nsubst",
            "cl-nsubst-if",
            "cl-nsubst-if-not",
            "cl-nsubstitute",
            "cl-nsubstitute-if",
            "cl-nsubstitute-if-not",
            "cl-nth-value",
            "cl-nunion",
            "cl-oddp",
            "cl-once-only",
            "cl-pairlis",
            "cl-parse-integer",
            "cl-plus",
            "cl-plusp",
            "cl-position",
            "cl-position-if",
            "cl-position-if-not",
            "cl-prettyexpand",
            "cl-proclaim",
            "cl-prog",
            "cl-prog*",
            "cl-progv",
            "cl-psetf",
            "cl-psetq",
            "cl-pushnew",
            "cl-random",
            "cl-random-state-p",
            "cl-rassoc",
            "cl-rassoc-if",
            "cl-rassoc-if-not",
            "cl-reduce",
            "cl-rem",
            "cl-remf",
            "cl-remove",
            "cl-remove-duplicates",
            "cl-remove-if",
            "cl-remove-if-not",
            "cl-remprop",
            "cl-replace",
            "cl-rest",
            "cl-return",
            "cl-return-from",
            "cl-revappend",
            "cl-rotatef",
            "cl-round",
            "cl-search",
            "cl-second",
            "cl-set-difference",
            "cl-set-exclusive-or",
            "cl-seventh",
            "cl-shiftf",
            "cl-signum",
            "cl-sixth",
            "cl-some",
            "cl-sort",
            "cl-stable-sort",
            "cl-struct--pcase-macroexpander",
            "cl-struct-built-in-class",
            "cl-struct-cl--class",
            "cl-struct-cl--generic",
            "cl-struct-cl--generic-generalizer",
            "cl-struct-cl--generic-method",
            "cl-struct-cl-derived-type-class",
            "cl-struct-cl-slot-descriptor",
            "cl-struct-cl-structure-class",
            "cl-struct-cl-structure-object",
            "cl-struct-oclosure--class",
            "cl-struct-registerv",
            "cl-struct-sequence-type",
            "cl-struct-set-slot-value",
            "cl-struct-slot-info",
            "cl-struct-slot-offset",
            "cl-struct-slot-value",
            "cl-struct-timer",
            "cl-struct-xref-elisp-location",
            "cl-sublis",
            "cl-subseq",
            "cl-subsetp",
            "cl-subst",
            "cl-subst-if",
            "cl-subst-if-not",
            "cl-substitute",
            "cl-substitute-if",
            "cl-substitute-if-not",
            "cl-svref",
            "cl-symbol-macrolet",
            "cl-tagbody",
            "cl-tailp",
            "cl-tenth",
            "cl-the",
            "cl-third",
            "cl-times",
            "cl-tree-equal",
            "cl-truncate",
            "cl-type--pcase-macroexpander",
            "cl-typecase",
            "cl-typep",
            "cl-union",
            "cl-values",
            "cl-values-list",
            "cl-with-accessors",
            "cl-with-gensyms",
            "clear-rectangle-line",
            "clear-vector",
            "connection-local-criteria-for-default-directory",
            "connection-local-get-profile-variables",
            "connection-local-get-profiles",
            "connection-local-normalize-criteria",
            "connection-local-profile-name-for-criteria",
            "copy-seq",
            "custom--settings-delete",
            "custom--theme-entry-delete",
            "custom-face-state",
            "custom-face-tag",
            "custom-group-list",
            "custom-group-tag",
            "custom-theme-load-themes",
            "custom-unlispify-menu-entry",
            "custom-variable-state",
            "custom-variable-tag",
            "declare-functionp",
            "decoded-time--defslot",
            "define-icon",
            "delete-extract-rectangle-line",
            "delete-rectangle-line",
            "delete-whitespace-rectangle-line",
            "dir-locals-to-string",
            "display-buffer-mark-dedicated",
            "easy-mmode--next",
            "easy-mmode--prev",
            "easy-mmode--globalized-predicate-p",
            "easy-mmode-define-navigation",
            "easy-mmode-pretty-mode-name",
            "emacs-build-time",
            "emacs-etc--hide-local-variables",
            "epa-file",
            "event-apply--modifier",
            "extract-rectangle-bounds",
            "extract-rectangle-line",
            "face-attrs--make-indirect-safe",
            "face-attrs-more-relative-p",
            "face-remap--clear-remappings",
            "face-remap--remap-face",
            "face-remap-order",
            "face-remap-remove-relative",
            "feature-file",
            "feature-symbols",
            "file-dependents",
            "file-loadhist-lookup",
            "file-provides",
            "file-requires",
            "file-set-intersect",
            "find-function--any-subform-p",
            "find-function--defface",
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
            "forward-line-command",
            "frame--list-z-order",
            "gv--defsetter",
            "gv-delay-error",
            "gv-deref",
            "gv-setter",
            "gv-synthetic-place",
            "gv-synthetic-place--anon-cmacro",
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
            "help-xref--navigation-buttons",
            "help-xref-go-back",
            "help-xref-go-forward",
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
            "jka-compr",
            "kmacro-end-or-call-macro-repeat",
            "let--pcase-macroexpander",
            "list-length",
            "list-tail",
            "make-help-screen",
            "make-obsolete-generalized-variable",
            "make-prop-match",
            "map--pcase-macroexpander",
            "map--pcase-map-elt",
            "map--plist-p",
            "map-let",
            "map-put",
            "member-if-not",
            "modify-dir-local-variable",
            "modify-file-local-variable",
            "modify-file-local-variable-message",
            "modify-file-local-variable-prop-line",
            "open-rectangle-line",
            "operate-on-rectangle",
            "output-switches",
            "overlays-at-point",
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
            "progv",
            "prop-match-p",
            "prop-match-value",
            "pushnew",
            "read-dir-locals-file",
            "read-feature",
            "read-file-local-variable",
            "read-file-local-variable-mode",
            "read-file-local-variable-value",
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
            "same-names-p",
            "scribe-mode",
            "seq",
            "seq-last",
            "sequence",
            "set-translation-table",
            "shell-command-mode",
            "spaces-string",
            "string-aref",
            "string-compare",
            "string-rectangle-line",
            "string-remove-prefix",
            "string-remove-suffix",
            "string-to-sequence",
            "text-property--find-end-backward",
            "text-property--find-end-forward",
            "text-property--match-p",
            "text-property-search-backward",
            "text-scale--refresh",
            "text-scale-max-amount",
            "text-scale-min-amount",
            "text-scale-mode",
            "thread-first",
            "thread-last",
            "timer--defslot",
            "timer-p",
            "toggle-read-only",
            "unload--set-major-mode",
            "unsafep--check",
            "values",
            "window-has-parameters",
            "window-inside-absolute-body-pixel-edges",
            "window-left-char",
            "window-line",
            "with-buffer-unmodified-if-unchanged",
            "work-buffer--get",
            "work-buffer--prepare-pixelwise",
            // GNU leaves these internal cells void at -Q: the
            // subr/`defun' here is a remacs convenience, so the
            // definition stays reachable via `remacs--dump-fn'.
            "internal-doc-string-p",
            "internal--set-subr-doc",
            "internal--thread-argument",
            "internal-make-interpreted-closure-function",
            "ucs-normalize--ensure-loaded",
            // The interpreted `(setf cl--find-class)' fallback in
            // cl-preloaded.el — GNU -Q has no such function.
            "(setf cl--find-class)",
            // ucs-normalize.el's `eval-when-compile'-only helpers:
            // GNU's dump runs them at compile time only, so they are
            // void at -Q; our source-level load defines them.
            "alist-list-to-vector",
            "decomposition-char-recursively",
            "decomposition-translation-alist",
            "hfs-nfd",
            "nfd",
            "nfkd",
            "quick-check-composition-list-to-regexp",
            "quick-check-decomposition-list-to-regexp",
            "quick-check-list",
            "quick-check-list-to-regexp",
        ];

        // Plist snapshot from before ANY Lisp runs: the cl chain
        // (gv→cl-lib→cl-loaddefs→cl-macs) loads *inside* the prelude
        // eval via pcase.el's expansion, so the post-prelude snapshot
        // below already carries cl-loaddefs'/gv's prop writes.  Rolled
        // files' own definitions restore their plist from this
        // (absent → nil); the `remacs--dump-fn' stash pass preserves
        // the prelude-folded definition on the same plist.
        let pre_prelude_plists: HashMap<SymId, Value> = interp
            .obarray
            .all_ids()
            .iter()
            .map(|&id| (id, interp.obarray.symbol(id).plist.clone()))
            .collect();
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
                        Err(f) => {
                            eprintln!(
                                "[prelude] read error @{pos}: {:?} {}",
                                f,
                                match &f {
                                    crate::lisp::Flow::Signal(s, d, _) =>
                                        interp.print_to_string(&Value::cons(s.clone(), d.clone())),
                                    _ => String::new(),
                                }
                            );
                            None
                        }
                    }
                };
                match next {
                    Some((form, end)) => {
                        pos = end;
                        k += 1;
                        let tf = std::time::Instant::now();
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
                        match interp.eval(&form) {
                            Ok(_) => eprintln!("  [{}ms]", tf.elapsed().as_millis()),
                            Err(f) => {
                                eprintln!(
                                    "  [{}ms] ERR {:?}",
                                    tf.elapsed().as_millis(),
                                    interp.print_to_string(&match &f {
                                        crate::lisp::Flow::Signal(s, d, _) =>
                                            Value::cons(s.clone(), d.clone(),),
                                        _ => Value::Nil,
                                    })
                                );
                            }
                        }
                    }
                    None => break,
                }
            }
        } else {
            // The prelude is a concatenation of GNU's lexical-binding
            // sources (subr.el, minibuffer.el, ...); `lexical-binding'
            // defaults to t, so `eval_str' installs a `(t)' root env
            // and it evals lexically like GNU's dump does.
            let t_prelude = std::time::Instant::now();
            if std::env::var_os("REMACS_BOOT_PROF").is_some() {
                eprintln!("[init] pre-prelude: {}ms", _init_t0.elapsed().as_millis());
            }
            // Definitions during the prelude attach to
            // `current-load-list' (GNU's LOADHIST_ATTACH) — bind it so
            // those pushes die with the unwind; the real history entry
            // comes from `record_prelude_load_history' below.
            {
                let cll = interp.intern("current-load-list");
                let mark = interp.specbind_depth();
                let _ = interp.specbind(cll, Value::Nil);
                if let Err(f) = interp.eval_str(crate::lisp::prelude::PRELUDE) {
                    // An error aborts `eval_str' mid-file, silently dropping
                    // every definition after the failing form — surface it.
                    let detail = match &f {
                        crate::lisp::Flow::Signal(sym, data, _) => format!(
                            "signal {} {}",
                            interp.print_to_string(sym),
                            interp.print_to_string(data)
                        ),
                        _ => format!("{:?}", f),
                    };
                    eprintln!("[prelude] eval aborted: {}", detail);
                }
                let _ = interp.unbind_to(mark);
            }
            if std::env::var_os("REMACS_BOOT_PROF").is_some() {
                eprintln!("[boot] prelude eval: {}ms", t_prelude.elapsed().as_millis());
            }
            if std::env::var_os("WATCH_TERM_CODING").is_some() {
                let _ = interp.eval_str(
                    "(add-variable-watcher 'default-terminal-coding-system \
                       (lambda (s v op w) \
                         (princ (format \"[WATCH d-t-c-s] op=%s v=%s in-load=%s file=%s\\n\" \
                                op v load-in-progress load-file-name))))",
                );
            }
            // First stash snapshot, right after the prelude: GNU_VOID_FNS
            // members the prelude defines (`cl-loop', `cl-defstruct', ...)
            // hold their real definitions now.  Libraries loaded below may
            // pull `cl-loaddefs' via `(eval-when-compile (require 'cl-lib))'
            // (tabulated-list, vc-hooks), which replaces those cells with
            // (autoload ...) forms before the later snapshot can see them —
            // and that snapshot skips autoload cells, so the stash would
            // end up empty.
            {
                let pk = interp.intern("remacs--dump-fn");
                let auto_id = interp.intern("autoload");
                for name in GNU_VOID_FNS.iter().chain(GNU_VOID_FNS_LATE.iter()) {
                    let sid = interp.intern(name);
                    let old = interp.obarray.symbol(sid).function.clone();
                    let is_autoload = matches!(
                        &old,
                        Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                    );
                    if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload {
                        interp.put_prop(sid, pk, old);
                    }
                }
            }
            if interp.dbg_flags & 0x800 != 0 {
                let id = interp.intern("cl-member");
                eprintln!(
                    "[post-prelude] cl-member fn={:?} plist={:?}",
                    interp.obarray.symbol(id).function,
                    interp.obarray.symbol(id).plist
                );
            }
            // Snapshot every symbol's state after the prelude but before
            // the boot-lib evals: interpreted loads fire
            // `eval-when-compile'/autoload requires that GNU's dumped
            // .elc files compiled away (cl-generic's `(eval-when-compile
            // (require 'cl-lib))' pulls cl-macs + cl-loaddefs + gv;
            // mouse pulls rect/send-to/map).  GNU's -Q image never ran
            // those files, so everything they touch — function cells,
            // value cells, plists — is restored from this snapshot after
            // the loads finish below.
            let pre_boot_syms: HashMap<SymId, (Value, Value, Value, bool, bool, bool)> = interp
                .obarray
                .all_ids()
                .iter()
                .map(|&id| {
                    let s = interp.obarray.symbol(id);
                    (
                        id,
                        (
                            s.function.clone(),
                            s.value.clone(),
                            s.plist.clone(),
                            s.special,
                            s.constant,
                            s.make_local_if_set,
                        ),
                    )
                })
                .collect();
            // Seed `defun-declarations-alist'/`macro-declarations-alist'
            // with byte-run.el's values *before* loading it.  The prelude
            // binds both to nil (so its early `(declare (gv-expander ...))'
            // pushes have a list to grow), which makes byte-run.el's own
            // `(defvar ... (nconc ...))' skip its initvals — GNU defvar
            // never overwrites a bound variable.  Without this, every
            // `declare' spec processed between here and subr-x.el's
            // repair `setq' warns `Unknown macro/defun property' and the
            // property (indent, obsolete, side-effect-free, doc-string, ...)
            // is silently dropped.  `#'byte-run--set-*' read before the
            // defaliases exist yields the bare symbol, which `apply'
            // resolves at call time.  `byte-run--dont-autoload' isn't
            // bound yet either, so its wrapper closure is spelled out.
            let _ = interp.eval_str(
                "(progn \
                   (setq defun-declarations-alist \
                         (list \
                          (list 'advertised-calling-convention #'byte-run--set-advertised-calling-convention) \
                          (list 'obsolete #'byte-run--set-obsolete) \
                          (list 'interactive-only #'byte-run--set-interactive-only) \
                          (list 'pure #'byte-run--set-pure) \
                          (list 'side-effect-free #'byte-run--set-side-effect-free) \
                          (list 'important-return-value #'byte-run--set-important-return-value) \
                          (list 'compiler-macro #'byte-run--set-compiler-macro) \
                          (list 'doc-string #'byte-run--set-doc-string) \
                          (list 'indent #'byte-run--set-indent) \
                          (list 'speed #'byte-run--set-speed) \
                          (list 'safety #'byte-run--set-safety) \
                          (list 'completion #'byte-run--set-completion) \
                          (list 'modes #'byte-run--set-modes) \
                          (list 'interactive-args #'byte-run--set-interactive-args) \
                          (list 'ftype #'byte-run--set-function-type))) \
                   (setq macro-declarations-alist \
                         (nconc \
                          (list \
                           (list 'debug #'byte-run--set-debug) \
                           (list 'autoload-macro #'byte-run--set-autoload-macro) \
                           (list 'indent (let ((f #'byte-run--set-indent)) \
                                           #'(lambda (&rest args) \
                                               (let ((code (apply f args))) \
                                                 (list 'progn ':autoload-end code))))) \
                           (list 'no-font-lock-keyword #'byte-run--set-no-font-lock-keyword)) \
                          defun-declarations-alist)) \
                   ;; gv.el's autoload cookies push these; the prelude pushed
                   ;; them early, so restore GNU order after the rebuild.
                   (let ((x (list 'gv-expander #'gv--expander-defun-declaration))) \
                     (or (assq 'gv-expander defun-declarations-alist) \
                         (progn (push x macro-declarations-alist) \
                                (push x defun-declarations-alist)))) \
                   (or (assq 'gv-setter defun-declarations-alist) \
                       (push (list 'gv-setter #'gv--setter-defun-declaration) \
                             defun-declarations-alist)))",
            );
            // byte-run.el is loaded by GNU's loadup.el near the top
            // (right after backquote/macroexp, both folded into the
            // prelude): it defines the `defmacro', `defun', `defsubst'
            // and `eval-when-compile'/`eval-and-compile' *macros* —
            // GNU expands `(defun ...)' to `(defalias ...)' at
            // macroexpand time, which `byte-compile-preprocess'
            // relies on to route definitions through the
            // `byte-hunk-handler' machinery.  The file has no
            // `provide', matching GNU's `featurep' => nil at -Q.
            let _ = boot_lib(&mut interp, "byte-run");

            // GNU's loadup.el `(load "subr")' runs right after
            // byte-run (line 125): its defmacros install the
            // `(macro . closure)' cells that `macrop',
            // `symbol-function' and the census surface report.  The
            // prelude's folded definitions still win at eval time via
            // `special_form', so this only corrects the cell shape.
            let _ = boot_lib(&mut interp, "subr");
            // subr.el's Lisp `defun's overwrite Rust subrs carrying
            // parity tweaks (gensym's interned-name skip, oddp's marker
            // rejection, apply-partially's #[128] byte-code shape,
            // locate-library's suffix rules, ...).  GNU's nativecomp
            // build reports `subrp' t for all of them, so restoring the
            // Rust cells is the more faithful surface.
            for s in crate::lisp::builtins::collect() {
                if let Some(id) = interp.intern_soft(s.name) {
                    let f = interp.symbol_function(id);
                    // Never clobber a macro cell: several prelude
                    // defmacros share names with helper subrs
                    // (`with-output-to-string' binds a real buffer for
                    // `standard-output'; the subr is just the runtime
                    // sink).  Macros are the faithful GNU cell shape
                    // anyway (subr.el's defmacro/defsubst beats the
                    // special-form subr for `macrop' parity).
                    let is_macro = matches!(&f, Value::Lambda(l) if l.is_macro)
                        || matches!(&f, Value::Cons(c)
                            if interp.sym_is(&c.borrow().car, sym::MACRO));
                    // A symbol-valued cell is a `defalias' shim (GNU's
                    // `not'/`beep'/`rplaca'/... aliases): don't replace
                    // it with our Rust subr — GNU's -Q shows the alias
                    // chain, not a direct subr.
                    let is_alias = matches!(&f, Value::Sym(s) if *s != sym::UNBOUND);
                    if !is_macro && !is_alias && !matches!(f, Value::Subr(_)) {
                        interp.fset(id, Value::Subr(s));
                    }
                }
            }
            // oclosure.el is in GNU's dump (loadup.el:198, right after
            // cl-preloaded — "Used by cl-generic"): interpreted closures
            // now carry GNU's docstring/env slot view, so the real file
            // provides `oclosure-define', `oclosure-lambda',
            // `oclosure--copy' & the `accessor--*' OClosure accessors
            // that cl-generic's `cl--generic-nnm' relies on.
            let _ = boot_lib(&mut interp, "oclosure");
            // help.el is dumped by GNU's loadup.el at line 202 — before
            // cl-generic (250).  `cl-defgeneric'/`cl-defmethod' forms in
            // cl-generic, seq, subr-x and register call
            // `help-add-fundoc-usage' → `help--docstring-quote' during
            // eager macro-expansion; loading help first keeps those
            // expansions bound like GNU's dump order.
            let _ = boot_lib(&mut interp, "help");
            // cl-generic.el is in GNU's dump (the `cl-generic' feature
            // is pre-registered): its `cl-defmethod' forms install the
            // real generalizer methods (`cl-generic-generalizers' for
            // `t', `eql', `head', `typeof', ...) that the vendored
            // machinery in subr-x needs to register any specialized
            // method.  Loading it here — after the dump-fn snapshot so
            // the cl-loaddefs autoloads pulled by its `(require
            // 'cl-lib)' cannot corrupt the stash — mirrors GNU's dump
            // order (loadup.el: cl-generic before seq/nadvice; its
            // `(require 'subr-x)' is eval-when-compile only, so neither
            // the .el nor the .elc needs subr-x loaded), and keeps
            // later cl-defmethod uses (icons, map, register, send-to)
            // from hitting an empty method table.
            let _ = boot_lib(&mut interp, "cl-generic");
            // seq.el is in GNU's dump too (loadup.el line 252, right
            // after cl-generic — its `.elc' calls cl-generic-define):
            // its `seq-let' defmacro replaces the raw-subr placeholder
            // so `(macrop 'seq-let)' reports t like GNU.
            let _ = boot_lib(&mut interp, "seq");
            // env.el is preloaded into GNU's dump (loadup.el): its
            // feature is already registered, but the definitions must
            // exist too — `(require 'env)' short-circuits on the
            // feature mark, so evaluate the embedded source now.
            let _ = boot_lib(&mut interp, "env");
            // jka-cmpr-hook.el is in GNU's dump too (loadup.el): the
            // feature mark would make `require' skip the definitions —
            // evaluate it so `jka-compr-installed-p' & co. exist and
            // `auto-compression-mode' installs its file-name handler.
            let _ = boot_lib(&mut interp, "jka-cmpr-hook");
            // tabulated-list.el is likewise in GNU's dump (loadup.el):
            // same reasoning as env — the feature mark alone would
            // make `require' skip the definitions.
            let _ = boot_lib(&mut interp, "tabulated-list");
            // register.el before subr-x.el: subr-x's compiled
            // `cl-defmethod register-val-jump-to ((val registerv) ...)'
            // errors "Unknown specializer registerv" unless the
            // `cl-defstruct' here has already registered the type.
            // (GNU dodges this by never dumping subr-x — it's pure
            // autoloads; we need its functions at -Q, so order wins.)
            let _ = boot_lib(&mut interp, "register");
            // subr-x.el is also in GNU's dump: it holds the accumulated
            // GNU-verbatim compatibility definitions (string trim/pad,
            // fringe helpers, paren/select/vc/dnd support, …).
            let _ = boot_lib(&mut interp, "subr-x");
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
            let _ = boot_lib(&mut interp, "tool-bar");
            // widget.el is dumped too (loadup.el loads "widget" for
            // `define-widget' & co. used by cus-edit and friends).
            let _ = boot_lib(&mut interp, "widget");
            // version.el is loaded by loadup.el without a `provide':
            // evaluating it defines `emacs-repository-*' helpers while
            // `(require 'version)' still fails exactly like GNU.
            let _ = boot_lib(&mut interp, "version");
            // rx.el is *not* loaded in GNU's dump: `rx' and friends
            // are loaddefs autoloads at -Q, so the prelude-folded
            // definitions get stashed+autoloaded below rather than the
            // file being eagerly loaded here.
            // vc-hooks.el is in GNU's dump (loadup.el): its defcustoms
            // (`vc-handled-backends', `vc-ignore-dir-regexp', ...) and
            // the vc-file-* property machinery are bound at -Q.
            let _ = boot_lib(&mut interp, "vc-hooks");
            // uniquify.el is dumped too; GNU's loadup order runs it
            // after vc-hooks, so `uniquify-kill-buffer-function'
            // prepends ahead of `vc-kill-buffer-hook'.
            let _ = boot_lib(&mut interp, "uniquify");
            // tooltip.el is dumped on window-system builds too: GNU has
            // `tooltip-delay', `tooltip-mode' & co. bound at -Q, and the
            // `tooltip' feature mark alone would make `require' skip
            // the definitions.
            let _ = boot_lib(&mut interp, "tooltip");
            // float-sup.el is dumped too (`lisp-float-type' feature):
            // `float-pi', `degrees-to-radians' & co. are bound at -Q.
            let _ = boot_lib(&mut interp, "float-sup");
            // map-ynp.el is in GNU's dump (loadup.el): `map-y-or-n-p'
            // is bound at -Q, used by files.el's `save-some-buffers'.
            let _ = boot_lib(&mut interp, "map-ynp");
            // debug-early.el is in GNU's dump too (loadup.el): it has
            // no `provide', so the feature stays nil while
            // `debug-early'/`debug-early-backtrace' are bound at -Q.
            let _ = boot_lib(&mut interp, "debug-early");
            // Snapshot the real definitions of GNU_VOID_FNS before
            // `loaddefs' replaces some of them with (autoload ...) cells
            // (e.g. `cl-loop' => "cl-macs"): the `remacs--dump-fn' stash
            // must hold the true dumped definition, or cl-macs's restore
            // would reinstall the autoload and recurse into its own load.
            {
                let pk = interp.intern("remacs--dump-fn");
                let auto_id = interp.intern("autoload");
                for name in GNU_VOID_FNS {
                    let sid = interp.intern(name);
                    let old = interp.obarray.symbol(sid).function.clone();
                    let is_autoload = matches!(
                        &old,
                        Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                    );
                    if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload {
                        interp.put_prop(sid, pk, old);
                    }
                }
            }
            // nadvice.el is dumped by loadup.el right after cl-generic
            // (loadup.el:253): its `oclosure-define' builds the real
            // `advice' OClosure accessors (`advice--car',
            // `advice--copy', ...) and the `advice--how-alist'
            // prototypes — required so later `advice-add'/
            // `add-function' uses (org's `org-macs' etc.) create real
            // 5-slot oclosures, and its `cl-defmethod
            // oclosure-interactive-form' needs cl-generic's method
            // table already installed.
            let _ = boot_lib(&mut interp, "nadvice");
            // easymenu.el is in GNU's dump (loadup.el, right after
            // widget): every dumped library that calls
            // `easy-menu-define'/`easy-menu-add' (prog-mode,
            // replace, text-mode, newcomment, isearch, menu-bar)
            // needs its macros already real.
            let _ = boot_lib(&mut interp, "easymenu");
            // custom.el + cus-start.el are in GNU's dump (loadup.el
            // loads them right after mule): `defcustom'/`defgroup'/
            // `defface' must be real functions before later dumped
            // libraries (image/tab-bar/ns-win/mwheel) expand their
            // calls, and cus-start's `define-minor-mode' uses need
            // custom.el first.
            let _ = boot_lib(&mut interp, "custom");
            let _ = boot_lib(&mut interp, "cus-start");
            // loaddefs.el is loaded by loadup.el right after subr.el:
            // it installs the (autoload ...) cells for every preloaded
            // library's entry points and provides the `loaddefs'
            // feature, both visible at -Q.
            let _ = boot_lib(&mut interp, "loaddefs");
            // tab-bar.el is in GNU's dump (loadup.el): tab-bar-mode,
            // tab-new, tab-switcher & co. are bound at -Q.  It needs
            // loaddefs's `frameset-filter-alist' defvar, hence the order.
            let _ = boot_lib(&mut interp, "tab-bar");
            // image.el is in GNU's dump too (loadup.el): image-mode,
            // `image-load-path', `insert-image' & co. are bound at -Q.
            let _ = boot_lib(&mut interp, "image");
            // buff-menu.el is in GNU's dump as well: `Buffer-menu-mode'
            // and friends are bound at -Q while the `buff-menu' feature
            // stays nil (the file has no `provide').
            let _ = boot_lib(&mut interp, "buff-menu");
            // electric.el is in GNU's dump (loadup.el):
            // `electric-indent-mode'/`electric-quote-mode' are bound
            // at -Q and elec-pair.el needs `electric-quote-chars'.
            let _ = boot_lib(&mut interp, "electric");
            // format.el and composite.el are in GNU's dump (loadup.el):
            // `format-alist', `format-decode' & co. and the
            // composition machinery are bound at -Q.
            let _ = boot_lib(&mut interp, "format");
            let _ = boot_lib(&mut interp, "composite");
            // iso-transl.el, mule-util.el and epa-hook.el are also in
            // GNU's dump (loadup.el): `iso-transl-set-language' & co.
            // are bound at -Q.
            let _ = boot_lib(&mut interp, "iso-transl");
            let _ = boot_lib(&mut interp, "mule-util");
            // mule.el and mule-conf.el are in GNU's dump (loadup.el):
            // `define-charset', `load-with-code-conversion' & co. and
            // the charsets (chinese-gb2312, japanese-jisx0213-*, ...)
            // are registered at -Q.
            let _ = boot_lib(&mut interp, "mule");
            let _ = boot_lib(&mut interp, "mule-conf");
            // mule-cmds/case-table/charprop/characters are in GNU's
            // dump (loadup.el:208-215, between mule-conf and the
            // language files).  `characters' populates the standard
            // category table (`modify-category-entry' ranges and
            // `map-charset-chars'), which `\\cX' regexes (e.g. `\\cj'
            // in ja-dic-cnv) rely on.
            let _ = boot_lib(&mut interp, "mule-cmds");
            let _ = boot_lib(&mut interp, "case-table");
            let _ = boot_lib(&mut interp, "charprop");
            // GNU's temacs runs loadup.el with `dump-mode' non-nil,
            // so characters.el takes its `(load
            // "international/charscript")' branch and the script &
            // category side effects really execute.  Here the
            // `charscript'/`emoji-zwj' features are already
            // registered, so `(require ...)' would be a no-op —
            // pretend dump-mode for this one library.
            let dump_mode = interp.intern("dump-mode");
            let _ = interp.set_symbol(dump_mode, Value::string("dump"));
            let _ = boot_lib(&mut interp, "characters");
            let _ = interp.set_symbol(dump_mode, Value::Nil);
            // The language-specific files are all in GNU's dump
            // (loadup.el:219-247, after mule-cmds/charprop/composite):
            // their charset registration, `set-language-environment'
            // calls and transcription alists (e.g.
            // `tibetan-vowel-transcription-alist', needed by quail's
            // tibetan.el) are live at -Q.
            for lib in [
                "chinese",
                "cyrillic",
                "indian",
                "sinhala",
                "english",
                "ethiopic",
                "european",
                "czech",
                "slovak",
                "romanian",
                "greek",
                "hebrew",
                "cp51932",
                "eucjp-ms",
                "japanese",
                "korean",
                "lao",
                "tai-viet",
                "thai",
                "tibetan",
                "vietnamese",
                "misc-lang",
                "utf-8-lang",
                "georgian",
                "khmer",
                "burmese",
                "cham",
                "philippine",
                "indonesian",
            ] {
                let _ = boot_lib(&mut interp, lib);
            }
            let _ = boot_lib(&mut interp, "epa-hook");
            // paren.el is in GNU's dump (loadup.el): `show-paren-mode'
            // and the `paren' feature are bound at -Q.
            let _ = boot_lib(&mut interp, "paren");
            // rmc.el is in GNU's dump too (loadup.el loads it early so
            // `read-multiple-choice' is available during startup).
            let _ = boot_lib(&mut interp, "rmc");
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
                // "register" moved before "subr-x" earlier: its
                // `cl-defstruct registerv' must exist before
                // subr-x.elc's `cl-defmethod ... ((val registerv))'
                // registers — a second load here would just re-run.
                // minibuffer.el is dumped too (loadup.el): binds
                // `minibuffer-beginning-of-buffer' & friends at -Q.
                "minibuffer",
                "replace",
                "scroll-bar",
                "text-mode",
                "timer",
                // GNU's dump also has newcomment.el, image.el and
                // tab-bar.el (loadup.el).  json.el is a plain library.
                "newcomment",
                "image",
                "tab-bar",
                // mwheel/t-mouse/windmove are NOT dumped by GNU's
                // loadup: their `:init-value t' modes reach
                // `global-minor-modes' via the `add-to-list' autoload
                // cookies in loaddefs.el, leaving the real files
                // autoload-only.
            ] {
                let _ = boot_lib(&mut interp, lib);
            }
            // term/ns-win.el, term/common-win.el and fontset.el are in
            // GNU's dump too (loadup.el): the `ns-win'/`common-win'/
            // `fontset' feature marks are pre-registered, so evaluate
            // the embedded sources for the real definitions —
            // `ns-handle-nxopen', `ns-parse-geometry',
            // `x-handle-args', `x-decompose-font-name' & co. must be
            // bound at -Q.
            // ucs-normalize.el is in GNU's dump (loadup.el, only when
            // uni-*.el exist — they do): its `define-translation-table'
            // calls register ucs-normalize-{nfd,nfkd,hfs-nfd}-table in
            // `translation-table-vector'.  The `ucs-normalize' feature
            // is pre-registered, so `(require 'ucs-normalize)' inside
            // ns-win would skip it — load explicitly like ns-win.
            let _ = boot_lib(&mut interp, "ucs-normalize");
            let _ = boot_lib(&mut interp, "ns-win");
            let _ = boot_lib(&mut interp, "common-win");
            let _ = boot_lib(&mut interp, "fontset");
            // mwheel.el is in GNU's dump (loadup.el:366, after
            // term/ns-win, gated on `(fboundp 'x-create-frame)' which
            // holds on NS builds): `mouse-wheel-mode' must be the real
            // `define-minor-mode' function and the variable gets t via
            // :init-value, matching GNU -Q where the mode is on.
            let _ = boot_lib(&mut interp, "mwheel");
            // GNU's dump includes leim/leim-list.el (loadup.el): its
            // `register-input-method' calls populate
            // `input-method-alist' so `activate-input-method' /
            // `read-multilingual-string' find the quail methods.
            let _ = boot_lib(&mut interp, "leim-list");
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
                   (fset 'change-log-mode '(autoload \"add-log\" \"Major mode for editing change logs.\" t nil)) \
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
                   (fset 'variable-at-point '(autoload \"help-fns\" \"Return the bound variable symbol found at or before point.\\nReturn 0 if there is no such symbol.\\nIf ANY-SYMBOL is non-nil, don't insist the symbol be bound.\\n\\n(fn &optional ANY-SYMBOL)\" nil nil)))",
            );
            // Roll back the `eval-when-compile' requires fired during
            // the dumped libraries' interpreted loads: mouse.el's
            // `(require 'rect)'/`(require 'send-to)' (which pulls in
            // map.el) don't fire in GNU's dumped .elc, so -Q keeps
            // those purely autoloaded.  Unbind every definition the
            // interpreted loads made, then reinstall the entry
            // points' loaddefs autoload cells.
            // Roll back the `eval-when-compile' requires fired during
            // the dumped libraries' interpreted loads: GNU's dumped
            // .elc files compiled those requires away, so -Q keeps
            // these libraries purely autoloaded.  mouse.el's
            // `(require 'rect)'/`(require 'send-to)' pull in map.el;
            // cl-generic's `(eval-when-compile (require 'cl-lib))'
            // pulls cl-lib (remacs-compat + cl-loaddefs), cl-macs,
            // pcase/subr-x no-ops — and `cl-defmethod (setf ...)'
            // expansion pulls gv.  History records only the entry's
            // own definitions, while cl-loaddefs' real work is `put'/
            // `autoload' side effects that leave no history items, so
            // the rolled-back state can't be reconstructed from
            // history alone: restore every symbol those files mention
            // (parsed from source) plus every symbol their history
            // items name, from the pre-load `pre_boot_syms' snapshot —
            // except symbols a KEPT library's history touches, which
            // legitimately changed post-snapshot (e.g. a dumped file
            // defining a `cl-*' name).
            // NOTE: `subr-x' is NOT here: GNU -Q leaves its feature
            // unregistered AND voids its own defs (handled by the
            // GNU_VOID lists), but its amalgamated dump-file copies
            // (bindings.el's `make-mode-line-mouse-map', leaf subr.el
            // helpers) stay bound at -Q — mass-rolling the file would
            // void those.  `pcase', `gv', `help-macro' ARE rolled:
            // none are dumped, so their defs/props (`pcase--memoize',
            // `progn's `gv-expander') are void at -Q — but see below:
            // prelude-subtracted defs still restore from the snapshot.
            // NOTE: `gv' is deliberately absent: GNU's loadup.el:195
            // `(require 'gv)' runs before the dump, so gv.el's defs
            // (`gv-get', `gv--defsetter', the `cond'/`if' gv-expander
            // props) survive at -Q even though `featurep' is nil
            // (its `provide' is dropped with the features reset at
            // the end of `new').  Rolling it voided `gv--defsetter'
            // while keeping `gv-get' bound (loaddefs history kept),
            // producing `((setf cond) ...)' expansions that signal
            // invalid-function — killing `advice-remove' and thus
            // `electric-indent-mode' init.
            const ROLLED_LIBS: &[&str] = &[
                "rect",
                "send-to",
                "map",
                "cl-macs",
                "cl-lib",
                "cl-loaddefs",
                "pcase",
                "help-macro",
                "remacs-compat",
                "inline",
            ];
            // Symbols every kept library's load-history items mention —
            // restoring these would clobber post-snapshot work.
            let mut kept_syms: HashSet<SymId> = HashSet::default();
            let mut touched: HashSet<SymId> = HashSet::default();
            let lh = interp.intern("load-history");
            let mut hist = interp.symbol_value(lh);
            let mut hist_items: Vec<Value> = Vec::new();
            while let Value::Cons(c) = hist {
                let b = c.borrow();
                hist_items.push(b.car.clone());
                hist = b.cdr.clone();
            }
            // One pass collects both sets: an entry belongs to a
            // rolled-back library when its path tail-matches one of
            // ROLLED_LIBS.
            for entry in &hist_items {
                let file = match entry {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => continue,
                };
                let rolled = match &file {
                    Value::Str(s) => {
                        let f = s.borrow();
                        ROLLED_LIBS.iter().any(|l| {
                            f.ends_with(&format!("/{}.el", l))
                                || f.ends_with(&format!("/{}.elc", l))
                        })
                    }
                    _ => false,
                };
                if let Value::Cons(c) = entry {
                    let mut items = c.borrow().cdr.clone();
                    while let Value::Cons(ic) = items {
                        let item = ic.borrow().car.clone();
                        items = ic.borrow().cdr.clone();
                        // Both rolled and kept entries record
                        // `(TYPE . NAME)' / `(TYPE NAME ...)' items
                        // where TYPE is defun/defmacro/autoload/...
                        // and bare symbols for defvar'd names.  Only
                        // NAME is a mutated symbol — collecting the
                        // TYPE token itself (as a deep walk does) pulls
                        // `defun' into the restore set and voids the
                        // macro byte-run just defined.
                        let mut push_sym = |v: &Value| {
                            if let Value::Sym(sid) = v {
                                if rolled {
                                    touched.insert(*sid);
                                } else {
                                    kept_syms.insert(*sid);
                                }
                            }
                        };
                        match &item {
                            Value::Cons(ic2) => {
                                let ib = ic2.borrow();
                                match &ib.car {
                                    Value::Sym(t)
                                        if matches!(
                                            interp.obarray.name(*t),
                                            "defun"
                                                | "defmacro"
                                                | "cl-defgeneric"
                                                | "autoload"
                                                | "t"
                                                | "defvar"
                                                | "defconst"
                                                | "defcustom"
                                                | "defalias"
                                                | "defsubst"
                                                | "defvar-local"
                                                | "defface"
                                                | "defgroup"
                                                | "cl-defun"
                                                | "cl-defmacro"
                                                | "cl-defsubst"
                                                | "cl-deftype"
                                                | "cl-defstruct"
                                                | "defstruct"
                                        ) =>
                                    {
                                        push_sym(&ib.cdr);
                                    }
                                    Value::Sym(t) if interp.obarray.name(*t) == "cl-defmethod" => {
                                        if let Value::Cons(d) = &ib.cdr {
                                            push_sym(&d.borrow().car);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => push_sym(&item),
                        }
                    }
                }
            }
            // Add every symbol literally targeted in the rolled-back
            // sources (autoload/'put/function-put/define-obsolete
            // side effects don't appear in load-history).  These
            // restore unconditionally — a rolled entry's own
            // `(defun . s)' history records for its autoload forms
            // would otherwise shield them as "kept".
            let mut prop_strips: HashSet<(SymId, SymId)> = HashSet::default();
            let mut rolled_defs: HashSet<SymId> = HashSet::default();
            let mut file_syms: HashSet<SymId> = HashSet::default();
            for lib in ROLLED_LIBS {
                if let Some(src) = crate::lisp::load::embedded(lib) {
                    collect_source_syms(
                        &mut interp,
                        src,
                        &mut file_syms,
                        &mut prop_strips,
                        &mut rolled_defs,
                    );
                    if std::env::var_os("REMACS_TRACE_COLLECT").is_some()
                        && rolled_defs.contains(&interp.intern("`"))
                    {
                        eprintln!("[rolled-defs got ` from {lib}]");
                    }
                }
            }
            // The prelude itself writes some of the same (SYM . KEY)
            // pairs a rolled file does (e.g. `(put 'cl--block-wrapper
            // 'compiler-macro ...)'): those stay — GNU's dump carries
            // them.  Scan PRELUDE with the same collector and subtract.
            {
                let mut _o = HashSet::default();
                let mut prelude_pairs: HashSet<(SymId, SymId)> = HashSet::default();
                let mut prelude_defs: HashSet<SymId> = HashSet::default();
                collect_source_syms(
                    &mut interp,
                    crate::lisp::prelude::PRELUDE,
                    &mut _o,
                    &mut prelude_pairs,
                    &mut prelude_defs,
                );
                prop_strips.retain(|pair| !prelude_pairs.contains(pair));
                // Names the prelude itself defines must not force-void
                // on rollback: prelude defs model GNU's dumped Lisp, so
                // GNU -Q binds them even when a rolled file carries a
                // copy (pcase.el's `define-symbol-prop' shim).
                rolled_defs.retain(|s| !prelude_defs.contains(s));
            }
            let mut restore_set: HashSet<SymId> = HashSet::default();
            // History-named symbols of rolled entries restore, and
            // every symbol the rolled sources target — EXCEPT names a
            // kept library's history records: a kept file wrote those
            // cells *after* the snapshot (byte-run.el's defalias for
            // `byte-run--parse-body'), so restoring the pre-snapshot
            // cell would void a definition GNU keeps.  Names GNU voids
            // anyway (cl-*) are re-voided by GNU_VOID_FNS_LATE below.
            for s in touched.iter().chain(file_syms.iter()) {
                if !kept_syms.contains(s) {
                    restore_set.insert(*s);
                }
            }
            if interp.dbg_flags & 0x800 != 0 {
                let id = interp.intern("cl-member");
                eprintln!(
                    "[pre-restore] cl-member in_file={} in_touched={} kept={} fn={:?} plist={:?}",
                    file_syms.contains(&id),
                    touched.contains(&id),
                    kept_syms.contains(&id),
                    interp.obarray.symbol(id).function,
                    interp.obarray.symbol(id).plist
                );
            }
            // Restore the snapshot state — value, function, plist and
            // binding flags together, plus every buffer's local cell.
            // A name the rolled file itself *defined* (defun/defmacro/
            // defvar/defalias) is voided even when the snapshot holds a
            // prelude-folded copy: GNU -Q has no definition for it at
            if interp.dbg_flags & 0x800 != 0 {
                let pb = interp.intern("byte-run--parse-body");
                let bq = interp.intern("`");
                let md = interp.intern("macroexp--dynvars");
                eprintln!(
                    "[pre-restore] pb kept={} file={} tch={} | ` kept={} file={} tch={} rst={} | mdyn kept={} file={} tch={} rst={} snapv={:?}",
                    kept_syms.contains(&pb),
                    file_syms.contains(&pb),
                    touched.contains(&pb),
                    kept_syms.contains(&bq),
                    file_syms.contains(&bq),
                    touched.contains(&bq),
                    restore_set.contains(&bq),
                    kept_syms.contains(&md),
                    file_syms.contains(&md),
                    touched.contains(&md),
                    restore_set.contains(&md),
                    pre_boot_syms.get(&md).map(|t| t.1.clone())
                );
            }
            if interp.dbg_flags & 0x800 != 0 {
                for nm in [
                    "pcase--memoize",
                    "killed-rectangle",
                    "cl--do-arglist",
                    "cl--lambda-list-keywords",
                    "cl-float-epsilon",
                    "cl-struct-timer",
                    "rectangle-mark-mode-map",
                ] {
                    let q = interp.intern(nm);
                    eprintln!(
                        "[pre-restore] {nm} defs={} kept={} file={} tch={} rst={} snapfn={:?}",
                        rolled_defs.contains(&q),
                        kept_syms.contains(&q),
                        file_syms.contains(&q),
                        touched.contains(&q),
                        restore_set.contains(&q),
                        pre_boot_syms.get(&q).map(|t| t.0.clone())
                    );
                }
            }
            // all.  Its plist likewise came from cl-loaddefs/declare
            // writes inside the cl chain — restore the pre-prelude
            // plist.  The `remacs--dump-fn' stash below keeps the
            // folded def reachable for `(require 'cl-lib)'/self-hosting.
            for sid in &restore_set {
                let sid = *sid;
                let fresh = || {
                    (
                        Value::Sym(sym::UNBOUND),
                        Value::Sym(sym::UNBOUND),
                        Value::Nil,
                        false,
                        false,
                        false,
                    )
                };
                let (f, v, p, sp, cn, ml) = pre_boot_syms.get(&sid).cloned().unwrap_or_else(fresh);
                // A name the rolled file *defined* voids unless a kept
                // file also defines it: prelude's own defs were already
                // subtracted from `rolled_defs' above, so what remains
                // here is never part of GNU's -Q dump state — even when
                // the snapshot holds a cell from the file's own
                // eval-when-compile run during the prelude
                // (pcase--memoize).  Names GNU voids that prelude folds
                // (cl-*) are re-voided by GNU_VOID_FNS_LATE after this.
                let force_void = rolled_defs.contains(&sid) && !kept_syms.contains(&sid);
                {
                    let s = interp.obarray.symbol_mut(sid);
                    s.function = if force_void {
                        Value::Sym(sym::UNBOUND)
                    } else {
                        f
                    };
                    s.value = if force_void {
                        Value::Sym(sym::UNBOUND)
                    } else {
                        v
                    };
                    s.plist = if force_void {
                        pre_prelude_plists.get(&sid).cloned().unwrap_or(Value::Nil)
                    } else {
                        p
                    };
                    s.special = if force_void { false } else { sp };
                    s.constant = if force_void { false } else { cn };
                    s.make_local_if_set = if force_void { false } else { ml };
                    s.variable_documentation = None;
                    s.var_alias = None;
                }
                if force_void {
                    // Preserve the dump-time stash the earlier pass
                    // planted: self-hosting loads (cl-macs expanding
                    // `cl-defmacro', pcase.el expanding `pcase')
                    // resolve it via `autoload_do_load'.
                    let dfk = interp.intern("remacs--dump-fn");
                    let stashed = match pre_boot_syms.get(&sid) {
                        Some((_, _, p, _, _, _)) => {
                            let mut cur = p.clone();
                            let mut found = Value::Nil;
                            while let Value::Cons(c) = cur {
                                let b = c.borrow();
                                if matches!(&b.car, Value::Sym(s) if *s == dfk) {
                                    found = match &b.cdr {
                                        Value::Cons(c2) => c2.borrow().car.clone(),
                                        _ => Value::Nil,
                                    };
                                    break;
                                }
                                cur = match &b.cdr {
                                    Value::Cons(c2) => c2.borrow().cdr.clone(),
                                    _ => Value::Nil,
                                };
                            }
                            found
                        }
                        None => Value::Nil,
                    };
                    let stash_live = match &stashed {
                        Value::Nil => false,
                        Value::Sym(s) => *s != sym::UNBOUND,
                        _ => true,
                    };
                    if stash_live {
                        interp.put_prop(sid, dfk, stashed);
                    }
                }
                for bid in interp.buffers.list() {
                    if let Some(b) = interp.buffers.get(bid) {
                        b.borrow_mut().locals.remove(&sid);
                    }
                }
            }
            // Strip the exact prop keys the rolled sources wrote
            // (`gv-expander' on `nth'/`if'/..., `compiler-macro' on
            // `cl-member'/..., obsolete/error metadata): the load ran
            // during the *prelude* for the cl chain (pcase's
            // eval-when-compile pulls gv→cl-lib→cl-loaddefs), so the
            // post-prelude snapshot already carries them and the
            // cell/plist restore above cannot remove them.  A NIL key
            // sentinel restores the whole plist from the snapshot.
            for (sym_id, key_id) in &prop_strips {
                let sym_id = *sym_id;
                let key_id = *key_id;
                if key_id == sym::NIL {
                    if let Some((_, _, p, _, _, _)) = pre_boot_syms.get(&sym_id) {
                        interp.obarray.symbol_mut(sym_id).plist = p.clone();
                    }
                } else {
                    // `remprop': walk key/value pairs; drop each pair
                    // whose key matches, preserving order.
                    let pl = interp.obarray.symbol(sym_id).plist.clone();
                    let mut kept: Vec<Value> = Vec::new();
                    let mut cur = pl;
                    while let Value::Cons(c) = cur {
                        let (k, after_k) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        // Read the value cons that follows the key.
                        let (v, after_v) = match after_k {
                            Value::Cons(c2) => {
                                let b = c2.borrow();
                                (b.car.clone(), b.cdr.clone())
                            }
                            _ => (Value::Nil, Value::Nil),
                        };
                        let is_target = matches!(&k, Value::Sym(s) if *s == key_id);
                        if !is_target {
                            kept.push(k);
                            kept.push(v);
                        }
                        cur = after_v;
                    }
                    let mut tail = Value::Nil;
                    for v in kept.iter().rev() {
                        tail = Value::cons(v.clone(), tail);
                    }
                    interp.obarray.symbol_mut(sym_id).plist = tail;
                }
            }
            interp.macro_gen += 1;
            // Drop the rolled entries themselves from load-history.
            let new_hist: Vec<Value> = hist_items
                .iter()
                .filter(|entry| {
                    let file = match entry {
                        Value::Cons(c) => c.borrow().car.clone(),
                        _ => return true,
                    };
                    match &file {
                        Value::Str(s) => {
                            let f = s.borrow();
                            !ROLLED_LIBS.iter().any(|l| {
                                f.ends_with(&format!("/{}.el", l))
                                    || f.ends_with(&format!("/{}.elc", l))
                            })
                        }
                        _ => true,
                    }
                })
                .cloned()
                .collect();
            let _ = interp.set_symbol(lh, Value::list(new_hist));
            // Purge the rolled features from `interp.features' AND the
            // `features' Lisp var (rebuilt from `interp.features' only
            // at the very end of `new'): while they linger,
            // `autoload_do_load''s feature-present shortcut resolves a
            // gv/cl-macs autoload through the dump-time stash without
            // loading the file, leaving side effects like `cond''s
            // `gv-expander' unregistered (boot-reeval of
            // `electric-indent-mode' broke this way).
            {
                let rolled_ids: HashSet<SymId> =
                    ROLLED_LIBS.iter().map(|l| interp.intern(l)).collect();
                interp.features.retain(|f| !rolled_ids.contains(f));
                let fid = interp.intern("features");
                if let Value::Cons(_) = interp.symbol_value(fid) {
                    let items = interp.symbol_value(fid).list_to_vec().unwrap_or_default();
                    let kept: Vec<Value> = items
                        .into_iter()
                        .filter(|v| {
                            !matches!(
                                interp.sym_id(v),
                                Some(id) if rolled_ids.contains(&id)
                            )
                        })
                        .collect();
                    let _ = interp.set_symbol(fid, Value::list(kept));
                }
            }
            // The rolled-back loads also pushed `minor-mode-map-alist'
            // entries (rect.el's `define-minor-mode' has a :keymap);
            // GNU's -Q alist doesn't have them.
            let _ = interp.eval_str(
                "(progn \
                   (set-default 'minor-mode-map-alist \
                       (delq (assq 'rectangle-mark-mode \
                                   (default-value 'minor-mode-map-alist)) \
                             (default-value 'minor-mode-map-alist))) \
                   (set-default 'minor-mode-alist \
                       (delq (assq 'rectangle-mark-mode \
                                   (default-value 'minor-mode-alist)) \
                             (default-value 'minor-mode-alist))) \
                   (setq minor-mode-map-alist \
                         (delq (assq 'rectangle-mark-mode \
                                     minor-mode-map-alist) \
                               minor-mode-map-alist) \
                         minor-mode-alist \
                         (delq (assq 'rectangle-mark-mode \
                                     minor-mode-alist) \
                               minor-mode-alist)))",
            );
            // GNU's -Q ordering of the minor-mode registries reflects
            // loadup evaluation order; our folded prelude registers
            // them in a different sequence, and prelude folds of
            // non-dumped files (face-remap, elec-pair, delsel,
            // battery, time) left extra `minor-mode-list' entries
            // while hand-written defuns (read-only-mode,
            // transient-mark-mode, use-hard-newlines) never pushed
            // theirs.  GNU's -Q list additionally carries
            // `cl-old-struct-compat-mode' at the head (pushed when
            // cl-lib's define-minor-mode ran during the dump; its
            // function cell is still the cl-lib autoload), so it
            // heads the literal list below.
            // Reproduce GNU's canonical dump state exactly: reorder
            // the alists (entry order = keymap priority) and set
            // `minor-mode-list' to GNU's literal -Q value.  The alists
            // are always-local, so fix both default and current
            // bindings.
            let _ = interp.eval_str(
                "(progn
                   (let ((order '(tab-bar-history-mode tab-bar-mode
                                  mouse-shift-adjust-mode context-menu-mode
                                  visual-line-mode read-extended-command-mode
                                  button-mode isearch-mode))
                         (reorder (lambda (list order)
                                    (let ((head (delq nil
                                                 (mapcar (lambda (s) (assq s list))
                                                         order)))
                                          (tail nil))
                                      (dolist (e list)
                                        (unless (memq (car e) order)
                                          (push e tail)))
                                      (nconc head (nreverse tail))))))
                     (set-default 'minor-mode-map-alist
                         (funcall reorder (default-value 'minor-mode-map-alist)
                                  order))
                     (setq minor-mode-map-alist
                           (funcall reorder minor-mode-map-alist order)))
                   (let ((order '(eldoc-mode center-line-mode
                                  isearch-fold-quotes-mode visible-mode
                                  visual-line-mode next-error-follow-minor-mode
                                  abbrev-mode overwrite-mode
                                  auto-fill-function defining-kbd-macro
                                  isearch-mode))
                         (reorder (lambda (list order)
                                    (let ((head (delq nil
                                                 (mapcar (lambda (s) (assq s list))
                                                         order)))
                                          (tail nil))
                                      (dolist (e list)
                                        (unless (memq (car e) order)
                                          (push e tail)))
                                      (nconc head (nreverse tail))))))
                     (set-default 'minor-mode-alist
                         (funcall reorder (default-value 'minor-mode-alist)
                                  order))
                     (setq minor-mode-alist
                           (funcall reorder minor-mode-alist order)))
                   (let ((order '(tooltip-mode global-eldoc-mode
                                  show-paren-mode electric-indent-mode
                                  file-name-shadow-mode
                                  minibuffer-regexp-mode
                                  minibuffer-nonselected-mode line-number-mode
                                  auto-encryption-mode auto-compression-mode
                                  windmove-mode gpm-mouse-mode
                                  mouse-wheel-mode))
                         (tail nil))
                     (setq global-minor-modes
                           (nconc (delq nil
                                   (mapcar (lambda (s)
                                             (and (memq s global-minor-modes) s))
                                           order))
                                  (progn
                                    (dolist (s global-minor-modes)
                                      (unless (memq s order) (push s tail)))
                                    (nreverse tail)))))
                   (setq minor-mode-list
                         '(cl-old-struct-compat-mode
                           tooltip-mode global-eldoc-mode eldoc-mode
                           show-paren-mode electric-quote-mode
                           electric-layout-mode electric-indent-mode
                           vc-auto-revert-mode mouse-wheel-mode
                           modifier-bar-mode tool-bar-mode center-line-mode
                           paragraph-indent-minor-mode
                           global-prettify-symbols-mode prettify-symbols-mode
                           use-hard-newlines tab-bar-history-mode tab-bar-mode
                           menu-bar-mode file-name-shadow-mode
                           isearch-fold-quotes-mode lost-selection-mode
                           horizontal-scroll-bar-mode mouse-shift-adjust-mode
                           context-menu-mode jit-lock-debug-mode
                           global-font-lock-mode font-lock-mode
                           blink-cursor-mode window-divider-mode
                           undelete-frame-mode minibuffer-nonselected-mode
                           minibuffer-regexp-mode completion-in-region-mode
                           visible-mode buffer-read-only
                           mode-line-invisible-mode size-indication-mode
                           column-number-mode line-number-mode
                           auto-fill-function global-visual-line-mode
                           visual-line-mode indent-tabs-mode
                           transient-mark-mode cursor-face-highlight-mode
                           read-extended-command-mode
                           delete-trailing-whitespace-mode
                           next-error-follow-minor-mode auto-composition-mode
                           auto-encryption-mode auto-compression-mode
                           temp-buffer-resize-mode button-mode lock-file-mode
                           auto-save-visited-mode auto-save-mode
                           auto-fill-mode abbrev-mode overwrite-mode
                           view-mode hs-minor-mode)))",
            );
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
            const GNU_VOID_FNS: &[&str] = &["cl--advice--apply"];
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
                // Stash only a genuine pre-void definition: a cell that
                // already holds an (autoload ...) form (installed by
                // loaddefs after the real def was made) must not be
                // recorded as the "dumped" definition, and an earlier
                // snapshot must not be overwritten either.
                let pk = interp.intern("remacs--dump-fn");
                let auto_id = interp.intern("autoload");
                let is_autoload = matches!(
                    &old,
                    Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                );
                let have_stash = !matches!(
                    interp.get_prop(sid, pk),
                    v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
                );
                if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload && !have_stash
                {
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
            // its expansion-time internals — exactly like GNU.  The
            // displaced definition goes to `remacs--dump-fn': while a
            // self-hosting file is being loaded (pcase.el expanding
            // `pcase', cl-macs.el expanding `cl-defmacro', ...),
            // `autoload_do_load' resolves the stash instead of
            // recursively loading the same file.
            if let Some(src) = crate::lisp::load::embedded("loaddefs") {
                let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
                let auto_id = interp.intern("autoload");
                let pk = interp.intern("remacs--dump-fn");
                let void_names: HashSet<SymId> = GNU_VOID_FNS
                    .iter()
                    .chain(GNU_VOID_FNS_LATE.iter())
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
                            other => {
                                if interp.dbg_flags & 0x800 != 0 {
                                    eprintln!(
                                        "[loaddefs-reinstall] reader stopped at {pos}: {other:?}"
                                    );
                                }
                                break;
                            }
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
                    let trace = interp.dbg_flags & 0x800 != 0
                        && matches!(interp.obarray.name(name), "pcase" | "rx" | "map-let");
                    if void_names.contains(&name) {
                        if trace {
                            eprintln!("[skip {}: void_names]", interp.symbol_name(name));
                        }
                        continue;
                    }
                    // GNU's loaddefs.el never carries autoloads for
                    // cl-macs/cl-seq/cl-extra — those cookies are scraped
                    // into cl-loaddefs.el instead, which only evaluates on
                    // the first `(require 'cl-lib)'.  Skip them here so
                    // -Q keeps the cl-* cells void like GNU; cl-lib.el's
                    // own `(load "cl-loaddefs")' installs them on demand.
                    let cl_deferred = match &items[2] {
                        Value::Str(s) => {
                            let f = s.borrow();
                            f.as_str() == "cl-macs"
                                || f.as_str() == "cl-seq"
                                || f.as_str() == "cl-extra"
                        }
                        _ => false,
                    };
                    if cl_deferred {
                        continue;
                    }
                    // A file that provides a loaded feature (the
                    // dumped libraries above) was already evaluated:
                    // in GNU's dump order its real definitions came
                    // after the loaddefs autoload cell and win.
                    let loaded_feature = match &items[2] {
                        Value::Str(s) => match interp.intern_soft(&s.borrow()) {
                            // Compare against the dump-time feature set —
                            // `interp.features' is polluted mid-init by
                            // libraries that get evaluated and rolled
                            // back, and is only reset to `dump_features'
                            // at the end of `new'.
                            Some(fid) => dump_features.contains(&fid),
                            None => false,
                        },
                        _ => false,
                    };
                    if loaded_feature {
                        if trace {
                            eprintln!("[skip {}: loaded_feature]", interp.symbol_name(name));
                        }
                        continue;
                    }
                    // A symbol re-defined *after* loaddefs by a later
                    // dump-time file (nadvice's `(cl-defmethod
                    // cl-print-object ...)' installs the generic
                    // dispatcher via `cl-generic-define-method') keeps
                    // that definition — GNU's dump order is loaddefs
                    // first.  Detect it by comparing the live cell
                    // with the pre-boot snapshot: `eq' (Rc pointer
                    // identity) means the prelude-folded def is still
                    // in place and GNU's autoload replaces it.
                    let live = interp.obarray.symbol(name).function.clone();
                    // Only a Lisp-level definition counts as "re-defined
                    // after loaddefs": a Subr cell predates loaddefs
                    // (builtins register before the prelude), so GNU's
                    // autoload still shadows it (format-spec).
                    // Any non-autoload Lisp-level cell — Lambda,
                    // bytecode Vec, or a Cons shape ((macro . λ),
                    // closures, advice chains) — counts as "re-defined
                    // after loaddefs".  A Subr cell predates loaddefs
                    // entirely (builtins register before the prelude),
                    // so GNU's autoload still shadows it (format-spec).
                    let live_is_lisp_def = match &live {
                        Value::Lambda(_) | Value::Vec(_) | Value::Cons(_) => true,
                        _ => false,
                    };
                    let value_is = |a: &Value, b: &Value| -> bool {
                        match (a, b) {
                            (Value::Sym(x), Value::Sym(y)) => x == y,
                            (Value::Cons(x), Value::Cons(y)) => Rc::ptr_eq(x, y),
                            (Value::Lambda(x), Value::Lambda(y)) => Rc::ptr_eq(x, y),
                            (Value::Nil, Value::Nil) => true,
                            _ => false,
                        }
                    };
                    let post_loaddefs_def = match pre_boot_syms.get(&name) {
                        Some((snap_f, _, _, _, _, _)) => {
                            // A snapshot cell that's itself an autoload
                            // (prelude's `(autoload 'gv-letplace "gv")'
                            // forms) IS the GNU -Q state — reinstalling
                            // restores it over the prelude-pulled real
                            // def (gv.el loaded mid-prelude via `push''s
                            // `gv-letplace' expansion).
                            let snap_is_autoload = matches!(
                                snap_f,
                                Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                            );
                            !snap_is_autoload && !value_is(&live, snap_f)
                        }
                        None => !matches!(live, Value::Sym(s) if s == sym::UNBOUND),
                    };
                    if live_is_lisp_def && post_loaddefs_def {
                        continue;
                    }
                    if trace {
                        eprintln!("[reinstall {}]", interp.symbol_name(name));
                    }
                    // Stash the prelude-folded definition being
                    // displaced: code inside the autoloaded file itself
                    // can still expand through it via the
                    // `loading_same' path in `autoload_do_load', which
                    // is what makes self-hosting files (pcase, rx,
                    // cl-macs, ...) loadable without .elc bodies.
                    let old = interp.obarray.symbol(name).function.clone();
                    let was_autoload = matches!(
                        &old,
                        Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                    );
                    let have_stash = !matches!(
                        interp.get_prop(name, pk),
                        v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
                    );
                    if !matches!(old, Value::Sym(s) if s == sym::UNBOUND)
                        && !was_autoload
                        && !have_stash
                    {
                        interp.put_prop(name, pk, old);
                    }
                    if interp.dbg_flags & 0x800 != 0
                        && matches!(interp.obarray.name(name), "pcase" | "rx" | "map-let")
                    {
                        eprintln!(
                            "[reinstall {} -> {:?}]",
                            interp.obarray.name(name),
                            interp.symbol_name(name)
                        );
                    }
                    // The read form's tail args are still source
                    // syntax: `nil'/'macro' read as (quote macro)
                    // etc.  Evaluate them the way `autoload' would —
                    // quoted args unquote, literals self-eval — so
                    // the cell ends with `t'/`macro', never
                    // `(quote t)'.
                    let quote_id = sym::QUOTE;
                    let mut cell = vec![Value::Sym(auto_id)];
                    for arg in &items[2..] {
                        let evaluated = match arg {
                            Value::Cons(c) => {
                                let b = c.borrow();
                                if matches!(&b.car, Value::Sym(s) if *s == quote_id) {
                                    match &b.cdr {
                                        Value::Cons(tail) => tail.borrow().car.clone(),
                                        _ => arg.clone(),
                                    }
                                } else {
                                    arg.clone()
                                }
                            }
                            _ => arg.clone(),
                        };
                        cell.push(evaluated);
                    }
                    interp.fset(name, Value::list(cell));
                }
            }
        }
        interp.loading_dumped = false;
        // Boot library loading is finished — `.elc' twins stop
        // applying so `(require …)' later reads the real source.
        interp.boot_loading = false;
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            // startup.el processes variables whose defcustom used
            // `custom-initialize-delay' via `custom-reevaluate-setting',
            // then sets the list to a non-list so later :initialize calls
            // (e.g. from `require') initialize immediately.
            let _ = interp.eval_str(
                "(progn (dolist (v (nreverse custom-delayed-init-variables)) \
                         (let ((r (condition-case e \
                                    (progn (custom-reevaluate-setting v) 'ok) \
                                    (t (list 'qa-error e))))) \
                           (when (and (consp r) (eq (car r) 'qa-error)) \
                             (message \"[boot-reeval] %s => %s\" v (cdr r))))) \
                         (setq custom-delayed-init-variables t))",
            );
            // GNU's `command-line' runs `set-locale-environment', which
            // calls `prefer-coding-system' for UTF-8 locales — yielding
            // default-terminal-coding-system, default buffer-file
            // coding and process-coding of utf-8-unix at -Q.  Mirror
            // the UTF-8 case (the only one remacs supports anyway).
            let utf8_locale = match ["LC_ALL", "LC_CTYPE", "LANG"]
                .iter()
                .filter_map(|v| std::env::var(v).ok())
                .find(|v| !v.is_empty())
            {
                // GNU honours the first non-empty locale var.
                Some(v) => {
                    let v = v.to_ascii_lowercase();
                    v.contains("utf-8") || v.contains("utf8")
                }
                // No locale vars at all: treat as UTF-8 (the platform
                // default on macOS is a UTF-8 system).
                None => true,
            };
            // GNU's `command-line' → `set-locale-environment nil'
            // resolves LANG/LC_* into a language environment and runs
            // `set-language-environment', which applies the
            // environment's `set-charset-priority' order (e.g. JIS
            // charsets first under ja_JP) and seeds the default
            // coding systems.  Fall back to a bare UTF-8 preference
            // if the Lisp call fails.
            let r = interp.eval_str(
                "(condition-case nil (progn (set-locale-environment nil) t) (error nil))",
            );
            let ran_locale = matches!(r, Ok(v) if v.truthy());
            if !ran_locale && utf8_locale {
                let _ = interp.eval_str("(prefer-coding-system 'utf-8-unix)");
                // `set-language-environment' also seeds
                // default-sendmail-coding-system from the locale.
                let _ = interp.eval_str("(setq default-sendmail-coding-system 'utf-8-unix)");
            }
            // GNU's `native-compile'/`native-compile-async' live in
            // comp.el/comp-run.el over the LIMPLE→libgccjit pipeline.
            // Remacs compiles through its own cc-driven emitter (see
            // builtins/comp.rs); bind the public entry points to
            // wrappers over `comp--remacs-*' subrs so the loaddefs
            // autoloads for comp.el never pull the GNU pipeline in.
            let _ = interp.eval_str(
                "(progn \
                   (defun native-compile (function-or-file &optional output) \
                     \"Compile FUNCTION-OR-FILE into native code.\" \
                     (comp--remacs-native-compile function-or-file output)) \
                   (defun native-compile-async \
                          (files &optional recursively load selector) \
                     \"Compile FILES asynchronously (synchronous in remacs).\" \
                     (comp--remacs-native-compile-async \
                      files recursively load selector)) \
                   (defun native-compile-directory (dir) \
                     \"Native-compile all .el files under DIR recursively.\" \
                     (comp--remacs-native-compile-async \
                      (list dir) t nil nil)) \
                   (defun batch-native-compile () \
                     \"Perform batch native compilation of remaining \
command-line arguments.\" \
                     (mapc #'native-compile command-line-args-left) \
                     (kill-emacs)) \
                   (defun batch-byte+native-compile () \
                     \"Like `batch-native-compile', but for bootstrap.\" \
                     (batch-native-compile)) \
                   (defun native-compile-prune-cache () \
                     \"Remove *.eln files unusable by this build.\" \
                     (interactive) \
                     nil) \
                   (defun emacs-lisp-native-compile () \
                     \"Native-compile the file visited in the current buffer.\" \
                     (interactive) \
                     (native-compile (buffer-file-name))) \
                   (defun emacs-lisp-native-compile-and-load () \
                     \"Native-compile the current buffer's file and load it.\" \
                     (interactive) \
                     (native-elisp-load \
                      (native-compile (buffer-file-name)))))",
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
        // GNU startup.el runs `lisp-interaction-mode' on *scratch*
        // AFTER loadup: characters.el's bare `modify-syntax-entry'
        // calls must hit the standard table first (remacs moved the
        // prelude's version of this to here for that reason).
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            let _ = interp.eval_str(
                "(when (get-buffer \"*scratch*\") \
                   (with-current-buffer \"*scratch*\" \
                     (lisp-interaction-mode)))",
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
        // GNU `init_buffer' runs on every buffer creation (and pdump
        // restart re-creates buffers): `characters.el' cleared the
        // standard case table's canon/eqv extras mid-loadup, so the
        // next buffer init re-derives them via `set_case_table'.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            crate::lisp::builtins::misc::heal_standard_case_table(&mut interp);
        }
        let eldoc_pre = interp.intern("eldoc-pre-command-refresh-echo-area");
        if let Some(b) = interp.buffers.get(scratch) {
            let mut br = b.borrow_mut();
            br.locals
                .insert(pch, Value::list(vec![Value::Sym(eldoc), Value::t()]));
            br.locals
                .insert(prech, Value::list(vec![Value::Sym(eldoc_pre), Value::t()]));
            // GNU's *scratch* has `require-final-newline' t (captured
            // before files.el's defcustom nils the default).
            let rfn = interp.intern("require-final-newline");
            br.locals.insert(rfn, Value::t());
            // ...and `eldoc-mode' on (set up by `lisp-interaction-mode'
            // via `global-eldoc-mode''s turn-on in GNU's startup),
            // which also pushes it onto `local-minor-modes'.
            let eld = interp.intern("eldoc-mode");
            let lmm = interp.intern("local-minor-modes");
            br.locals.insert(eld, Value::t());
            br.locals.insert(lmm, Value::list(vec![Value::Sym(eld)]));
        }
        let tooltip_hide = interp.intern("tooltip-hide");
        interp.obarray.symbol_mut(prech).value = Value::list(vec![Value::Sym(tooltip_hide)]);
        // Boot-time autoloads (easy-mmode & co.) correspond to GNU's
        // dumped loadup; the user-visible `features' list must match the
        // post-dump set.
        interp.features = dump_features;
        // External-library features are registered at runtime (dlopen):
        // re-check after the dump restore so `featurep' sees e.g.
        // `lcms2'/`kqueue' when the libraries resolve.
        crate::lisp::builtins::register_extlib_features(&mut interp);
        // `icons' faces created by boot-time loads are likewise
        // hidden until a real `load' triggers them.
        interp
            .face_table
            .retain(|(n, _)| n != "icon" && n != "icon-button");
        // GNU xfaces.c seeds Vface_new_frame_defaults during startup
        // face realization; mirror it: every known face gets an
        // (id . lface) entry so `face-list'/`frame-face-alist' see
        // the same faces at -Q.
        {
            let names: Vec<String> = crate::editor::KNOWN_FACES
                .iter()
                .map(|s| s.to_string())
                .chain(interp.face_table.iter().map(|(n, _)| n.clone()))
                .collect();
            for n in &names {
                crate::editor::face_register_global(&mut interp, n);
            }
        }
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
        // GNU seeds `command-line-args'/`invocation-name'/
        // `invocation-directory' in init_cmdargs — i.e. for EVERY
        // Interp, not just real_main's (tests build Interp directly).
        {
            let argv: Vec<String> = std::env::args().collect();
            if let Some(argv0) = argv.first() {
                let p = std::path::Path::new(argv0);
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| argv0.clone());
                let dir = p.parent().map(|d| {
                    let mut s = d.to_string_lossy().into_owned();
                    if !s.ends_with('/') {
                        s.push('/');
                    }
                    s
                });
                let iname = interp.intern("invocation-name");
                let _ = interp.set_symbol(iname, Value::string(name));
                let idir = interp.intern("invocation-directory");
                let _ = interp.set_symbol(idir, dir.map(Value::string).unwrap_or(Value::Nil));
                let cla = interp.intern("command-line-args");
                let _ = interp.set_symbol(
                    cla,
                    Value::list(argv.iter().map(|a| Value::string(a.clone())).collect()),
                );
            }
        }
        // Final -Q normalization.  The folded prelude evaluates
        // `define-derived-mode' calls and defvars for libraries GNU
        // never dumps (cc/progmodes modes, face-remap, ...), which
        // leaves `*-mode-map'/`*-mode-hook'/`*-mode-syntax-table'/
        // `*-mode-abbrev-table' and assorted toggle vars bound where
        // GNU -Q reports them unbound — and likewise leftover function
        // cells for undumped internals.  Void the cells so `boundp'/
        // `fboundp' match GNU: value cells are cleared outright,
        // function cells keep their `remacs--dump-fn' stash so direct
        // calls still reach the dump-time definition.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            for name in GNU_VOID_VARS {
                let sid = interp.intern(name);
                // Stash the dump value under `remacs--dump-var':
                // expansion-time machinery (cl-labels' convert cache,
                // pcase's memoize table, cl-macs's float limits, ...)
                // reads through it via `symbol_value' while `boundp'
                // stays nil like GNU -Q.
                let old = interp.obarray.symbol(sid).value.clone();
                if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) {
                    let pvk = interp.intern("remacs--dump-var");
                    interp.put_prop(sid, pvk, old);
                }
                {
                    let s = interp.obarray.symbol_mut(sid);
                    s.value = Value::Sym(sym::UNBOUND);
                }
                for bid in interp.buffers.list() {
                    if let Some(b) = interp.buffers.get(bid) {
                        b.borrow_mut().locals.remove(&sid);
                    }
                }
            }
            let pk = interp.intern("remacs--dump-fn");
            let auto_id = interp.intern("autoload");
            for name in GNU_VOID_FNS_LATE {
                let sid = interp.intern(name);
                let old = interp.obarray.symbol(sid).function.clone();
                let is_autoload = matches!(
                    &old,
                    Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                );
                let have_stash = !matches!(
                    interp.get_prop(sid, pk),
                    v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
                );
                if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload && !have_stash
                {
                    interp.put_prop(sid, pk, old);
                }
                interp.obarray.symbol_mut(sid).function = Value::Sym(sym::UNBOUND);
            }
            // `(setf NAME)' accessor cells: GNU -Q interns exactly the
            // eleven setters its dumped `cl-defstruct's emit as real
            // functions (built-in-class, cl-derived-type-class) plus
            // `(setf seq-elt)'; every other slot setter is a
            // compiler-macro/`gv-setter' plumbing artifact, so the
            // symbol is never interned.  Our interpreted prelude
            // spells them out as `(fset (intern "(setf …)") …)'
            // instead — void those cells to match.  Definitions stay
            // callable through `remacs--dump-fn' (callable_function),
            // which `dumped_runtime_helper' already claims for the
            // "(setf " prefix.
            const GNU_SETF_FNS: &[&str] = &[
                "(setf built-in-class--docstring)",
                "(setf built-in-class--index-table)",
                "(setf built-in-class--name)",
                "(setf built-in-class--parents)",
                "(setf built-in-class--slots)",
                "(setf cl-derived-type-class-docstring)",
                "(setf cl-derived-type-class-index-table)",
                "(setf cl-derived-type-class-name)",
                "(setf cl-derived-type-class-parents)",
                "(setf cl-derived-type-class-slots)",
                "(setf seq-elt)",
            ];
            let setf_ids: Vec<SymId> = interp
                .obarray
                .all_ids()
                .iter()
                .copied()
                .filter(|&sid| {
                    let n = interp.obarray.name(sid);
                    n.starts_with("(setf ") && !GNU_SETF_FNS.contains(&n)
                })
                .collect();
            for sid in setf_ids {
                let old = interp.obarray.symbol(sid).function.clone();
                let is_autoload = matches!(
                    &old,
                    Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
                );
                let have_stash = !matches!(
                    interp.get_prop(sid, pk),
                    v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
                );
                if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload && !have_stash
                {
                    interp.put_prop(sid, pk, old);
                }
                interp.obarray.symbol_mut(sid).function = Value::Sym(sym::UNBOUND);
            }
            // GNU -Q keeps two autoload cells that our dump fold can't
            // reproduce in-place: map-ynp.el's `(autoload
            // 'cl--map-keymap-recursively "cl-extra")' is a no-op at
            // boot_lib time (rolled cl-loaddefs installs the cell
            // first, so `autoload' early-outs on `fboundp' and the
            // rollback restore then voids it), and cl-preloaded.el's
            // `(autoload 'cl--defsubst-expand "cl-macs")' never runs
            // at all (the feature is pre-registered, its file never
            // loaded).  Reinstall both so `fboundp'/`symbol-function'
            // match GNU.
            for (sym_name, file) in [
                ("cl--map-keymap-recursively", "cl-extra"),
                ("cl--defsubst-expand", "cl-macs"),
            ] {
                let sid = interp.intern(sym_name);
                if matches!(
                    interp.obarray.symbol(sid).function,
                    Value::Sym(s) if s == sym::UNBOUND
                ) {
                    interp.obarray.symbol_mut(sid).function = Value::list(vec![
                        Value::Sym(auto_id),
                        Value::string(file),
                        Value::Nil,
                        Value::Nil,
                        Value::Nil,
                    ]);
                }
            }
            // `comp--#$': comp.el's `(defvar comp--#$ "#$")' reads in
            // *source* form as `comp--' + `#$' (the load-file-name
            // reader macro) — GNU's dump ran the .elc, where the symbol
            // name survives verbatim, so -Q binds `comp--#$' to "#$"
            // (special) and leaves `comp--' void.  Our interpreted
            // subr-x boot instead binds `comp--' to the load path;
            // repair the split here.
            {
                let comp_dash = interp.intern("comp--");
                {
                    let s = interp.obarray.symbol_mut(comp_dash);
                    s.value = Value::Sym(sym::UNBOUND);
                    s.special = false;
                }
                let comp_hash = interp.intern("comp--#$");
                {
                    let s = interp.obarray.symbol_mut(comp_hash);
                    s.value = Value::string("#$");
                    s.special = true;
                }
            }
        }
        // GNU's pdumper discards whatever the loadup wrote into
        // *Messages* — a -Q image starts with the buffer absent and
        // `message' recreates it.  Our boot loads (which stand in for
        // loadup) would otherwise leave their "Loading ..." lines
        // behind; drop them so `(buffer-size *Messages*)' starts at 0.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            if let Some(mb) = interp.buffers.by_name("*Messages*") {
                if let Some(b) = interp.buffers.get(mb) {
                    let mut bb = b.borrow_mut();
                    let z = bb.text_len();
                    bb.delete_region(0, z);
                    bb.set_point(0);
                    bb.set_undo_list(Value::t());
                }
            }
        }
        // GNU parity fixups that must land after every boot library:
        //
        // * `registerv-data' & friends are cl-defstruct accessors —
        //   GNU's compiled accessors check `registerv-p' first and
        //   signal `wrong-type-argument (registerv X)'; our subset
        //   generator emits bare `(aref v N)' which reports
        //   `args-out-of-range'.  Redefine them checked.
        // * `search-emacs-glossary' is a native subr in GNU 31's
        //   dump, so its wrong-number-of-arguments datum is the
        //   symbol, not our byte-code `(0 . 0)' pair — rebind it to
        //   the Rust subr (its body still routes through `info').
        // * GNU's dumped image carries a stale search_regs leftover
        //   `(0 3)' — `(match-data)', `match-substitute-replacement'
        //   and `prettify-symbols--compose-symbol' on a fresh -Q all
        //   see it; prime ours identically.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            let _ = interp.eval_str(
                "(progn \
                   (defun registerv-data (v) \
                     (if (registerv-p v) (aref v 1) \
                       (signal 'wrong-type-argument (list 'registerv v)))) \
                   (defun registerv-print-func (v) \
                     (if (registerv-p v) (aref v 2) \
                       (signal 'wrong-type-argument (list 'registerv v)))) \
                   (defun registerv-jump-func (v) \
                     (if (registerv-p v) (aref v 3) \
                       (signal 'wrong-type-argument (list 'registerv v)))) \
                   (defun registerv-insert-func (v) \
                     (if (registerv-p v) (aref v 4) \
                       (signal 'wrong-type-argument (list 'registerv v)))))",
            );
            if let Some(s) = crate::editor::winxtra::SUBRS
                .iter()
                .find(|s| s.name == "search-emacs-glossary")
            {
                let id = interp.intern("search-emacs-glossary");
                interp.fset(id, Value::Subr(s));
            }
            interp.match_data = Some(MatchData {
                regs: vec![Some(0), Some(3)],
                in_buffer: false,
                base: 0,
                buffer: None,
            });
        }
        // The backquote machinery must be fmakunbound-immune exactly like
        // GNU's bytecompiled backquote.el (a user `(fmakunbound 'car)'
        // can't break `Bcar' opcodes): bind `backquote'/`backquote-list*'
        // and the helper defuns to Rust subrs after every library load,
        // so prelude/Lisp redefinitions can't re-expose the cells.
        crate::lisp::builtins::backquote::install_macros(&mut interp);
        if std::env::var_os("REMACS_BOOT_PROF").is_some() {
            eprintln!(
                "[init] total Interp::new: {}ms",
                _init_t0.elapsed().as_millis()
            );
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
        // SymId::MAX = an `intern_soft' miss; an absent symbol's name
        // resolves to "" (never interned, never equal to a real name).
        if id == u32::MAX {
            return String::new();
        }
        self.obarray.name(id).to_string()
    }

    pub fn sym_is(&self, v: &Value, id: SymId) -> bool {
        match v {
            Value::Sym(s) => *s == id,
            Value::Nil => id == sym::NIL,
            _ => crate::lisp::builtins::misc::as_sympos(v)
                .map(|(s, _)| s == id)
                .unwrap_or(false),
        }
    }

    /// The symbol id of a `Value::Sym`/`Value::Nil`/enabled
    /// `symbols-with-pos-enabled' object, if it is one.  GNU's enabled
    /// sympos are symbols for every primitive (`symbol-name', `get',
    /// `fboundp', ...), so they unwrap to their bare id here.
    pub fn sym_id(&self, v: &Value) -> Option<SymId> {
        match v {
            Value::Sym(s) => Some(*s),
            Value::Nil => Some(sym::NIL),
            _ => crate::lisp::builtins::misc::as_sympos(v).map(|(s, _)| s),
        }
    }

    /// Global (default) value of a symbol.
    /// Follow `defvaralias' chains: return the ultimate base symbol.
    pub fn var_alias_target(&self, id: SymId) -> SymId {
        if id == u32::MAX {
            return id; // intern_soft miss sentinel: no alias chain
        }
        let mut cur = id;
        for _ in 0..64 {
            match self.obarray.symbol(cur).var_alias {
                Some(next) => cur = next,
                None => return cur,
            }
        }
        cur
    }

    pub fn symbol_value(&self, id: SymId) -> Value {
        let id = self.var_alias_target(id);
        // Buffer-local binding in current buffer wins. `try_borrow`:
        // primitives that hold the buffer mutably borrowed may still
        // consult variables (they see the global binding).
        if id == u32::MAX {
            return Value::Nil; // intern_soft miss — unbound is nil
        }
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    return v.clone();
                }
                // `buffer-file-name' is a BVAR: the real buffer slot is
                // the binding; no Lisp-level entry means the slot.
                if self.bfn_sym != 0 && id == self.bfn_sym {
                    return match &bb.file_name {
                        Some(f) => Value::string(f.clone()),
                        None => Value::Nil,
                    };
                }
            }
        }
        let v = &self.obarray.symbol(id).value;
        if let Value::Sym(s) = v {
            if *s == sym::UNBOUND {
                // A voided dump-time variable (GNU_VOID_VARS) still
                // answers reads with its stashed dump value —
                // expansion-time machinery (cl-labels' convert cache,
                // pcase's memoize table, cl-macs's float limits, ...)
                // keeps working while `boundp' stays nil like GNU -Q.
                if let Some(stashed) = self.dumped_variable(id) {
                    return stashed;
                }
                return Value::Sym(sym::UNBOUND);
            }
        }
        v.clone()
    }

    /// Hidden dump-time value for SYM, if it was stashed while the
    /// public value cell was voided for GNU -Q compatibility.
    fn dumped_variable(&self, id: SymId) -> Option<Value> {
        let prop = self.intern_soft("remacs--dump-var")?;
        // `nil' is a real stashed value (cl--labels-convert-cache's
        // cache starts nil): test plist membership, not truthiness.
        let mut cur = self.obarray.symbol(id).plist.clone();
        while let Value::Cons(c) = cur {
            let (car, next) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(car, Value::Sym(s) if s == prop) {
                return match &next {
                    Value::Cons(c2) => Some(c2.borrow().car.clone()),
                    _ => Some(Value::Nil),
                };
            }
            cur = match next {
                Value::Cons(c2) => c2.borrow().cdr.clone(),
                _ => break,
            };
        }
        None
    }

    /// Mark a string as unibyte (encoder output); `prin1' escapes
    /// its ≥0x80 byte-chars as `\NNN' octal like GNU.
    pub fn mark_unibyte(&mut self, s: &crate::lisp::value::StrRef) {
        // Normalize contents: a unibyte string's 0x80-0xFF chars are
        // raw bytes — hold them as eight-bit PUA proxies so `aref',
        // printing, and output transcode see byte semantics.
        {
            let mut text = s.borrow_mut();
            if text.chars().any(|c| (0x80..=0xFF).contains(&(c as u32))) {
                let conv: String = text
                    .chars()
                    .map(|c| {
                        if (0x80..=0xFF).contains(&(c as u32)) {
                            char::from_u32(crate::lisp::value::EIGHT_BIT_BASE + c as u32)
                                .unwrap_or(c)
                        } else {
                            c
                        }
                    })
                    .collect();
                *text = conv;
            }
        }
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
        let mut cur = crate::lisp::builtins::misc::unpos(v);
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
        if self.dbg_flags & 0x100 != 0 && constant {
            eprintln!(
                "[setconst {} = {}] caller={:?}",
                self.symbol_name(id),
                self.princ_to_string(&val)
                    .chars()
                    .take(80)
                    .collect::<String>(),
                std::panic::Location::caller()
            );
            for (f, a) in self.lisp_stack.iter().rev().take(20).rev() {
                eprintln!(
                    "  {} {}",
                    self.princ_to_string(f)
                        .chars()
                        .take(100)
                        .collect::<String>(),
                    self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                        .chars()
                        .take(160)
                        .collect::<String>()
                );
            }
        }
        if constant && id != sym::NIL && id != sym::T {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        if constant {
            // nil/t can't be set at all.
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let sym = self.obarray.symbol(id);
        let is_auto_local = sym.make_local_if_set || sym.always_local;
        let local = is_auto_local
            || self
                .buffers
                .get(self.current_buffer)
                .and_then(|b| {
                    b.try_borrow()
                        .ok()
                        .map(|bb| bb.locals.contains_key(&id))
                })
                .unwrap_or(false);
        // GNU's set_internal fires variable watchers BEFORE the store:
        // a signalling watcher aborts the write entirely and observers
        // inside the watcher still see the old value.
        self.fire_var_watchers(id, &val, "set", local.then_some(self.current_buffer))?;
        if local {
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // Mirror the BVAR write into the buffer slot.
                        bb.file_name = match &val {
                            Value::Str(s) => Some(s.borrow().clone()),
                            _ => None,
                        };
                    }
                }
            }
            return Ok(());
        }
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        Ok(())
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
        // GNU `Fset_default' walks the variable-alias chain first:
        // `(set-default 'alias v)' writes BASE's default cell, leaving
        // the alias's own cell a legacy remnant readers ignore.
        let id = self.var_alias_target(id);
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        if self.dbg_flags & 0x40000 != 0 && self.symbol_name(id) == "default-terminal-coding-system"
        {
            eprintln!(
                "[WATCH set-default] -> {} \n{}",
                self.prin1_to_string(&val),
                std::backtrace::Backtrace::capture()
            );
        }
        // GNU's set_internal fires variable watchers BEFORE the store:
        // a signalling watcher leaves the default cell untouched.
        self.fire_var_watchers(id, &val, "set", None)?;
        self.obarray.symbol_mut(id).value = val.clone();
        self.sync_undo_inhibit(id, &val);
        Ok(())
    }

    /// `functionp' on a value: subrs, lambdas, `(lambda ...)' conses,
    /// and symbols whose function cell holds a non-macro definition.
    pub fn function_p(&self, v: &Value) -> bool {
        match v {
            Value::Subr(_) | Value::Lambda(_) => true,
            Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
            Value::Sym(id) => {
                // GNU resolves `defalias' chains: the alias's final
                // target decides.
                let mut f = self.symbol_function(*id);
                let mut hops = 0usize;
                while let Value::Sym(next) = f {
                    if next == sym::UNBOUND || hops >= 64 {
                        return false;
                    }
                    hops += 1;
                    f = self.symbol_function(next);
                }
                match f {
                    Value::Subr(s) => self
                        .intern_soft(s.name)
                        .and_then(|x| crate::lisp::special::special_form(x))
                        .is_none(),
                    Value::Lambda(l) => !l.is_macro,
                    Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn symbol_function(&self, id: SymId) -> Value {
        self.obarray.symbol(id).function.clone()
    }

    /// Function cell used by ordinary calls.  Hidden dump-time
    /// definitions stay reachable via the `remacs--dump-fn' stash —
    /// GNU voids these cells at -Q, but keeping calls working is a
    /// deliberate superset (see `form_function').
    fn callable_function(&mut self, id: SymId) -> Value {
        let f = self.symbol_function(id);
        // A `nil' cell (fset to nil, or a cell that holds nil rather
        // than `unbound') is a void function in GNU terms: `funcall
        // nil' reports `void-function', not `invalid-function'.
        let f = if matches!(f, Value::Nil) {
            Value::Sym(sym::UNBOUND)
        } else if !matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            f
        } else {
            self.dumped_function(id).unwrap_or(f)
        };
        // Inside a macro expander running for compiled-equivalent
        // (dumped/bundled) code, resolve to the unadvised base: GNU
        // expanded these call sites at byte-compile time, before any
        // advice could exist.
        if self.expand_unadvised_depth > 0 {
            self.unadvised_base(&f)
        } else {
            f
        }
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
    /// expansion-time access, a dumped definition stays callable: GNU
    /// voids these cells at -Q, but every name we void has its real
    /// dumped definition stashed on `remacs--dump-fn', so direct calls
    /// still run the dump-time semantics (strictly more capable than
    /// GNU's void-function, while `symbol-function' stays nil).
    fn form_function(&mut self, id: SymId) -> Value {
        let f = self.symbol_function(id);
        let f = if !matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            f
        } else {
            self.dumped_function(id).unwrap_or(f)
        };
        // Same unadvised-base resolution as `callable_function': code
        // evaluated inside a dumped-code macro expander sees the
        // compile-time function cells.
        if self.expand_unadvised_depth > 0 {
            self.unadvised_base(&f)
        } else {
            f
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
        // GNU fboundp: a nil or unbound cell is void.  (defalias 'x nil)
        // stores nil, dropping fboundp but leaving symbol-function nil.
        !matches!(self.obarray.symbol(id).function, Value::Nil)
            && !matches!(
                self.obarray.symbol(id).function,
                Value::Sym(s) if s == sym::UNBOUND
            )
    }
    /// GNU permits function definitions on t and keywords, but nil must
    /// retain a nil function cell. Check before replacing definitions or advice.
    pub(crate) fn check_function_assignment(&mut self, id: SymId, def: &Value) -> Result<(), Flow> {
        if id == sym::NIL && !def.is_nil() {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![Value::Nil]));
        }
        Ok(())
    }
    pub fn fset(&mut self, id: SymId, def: Value) {
        // GNU stores `(macro . fn)' as the literal function cell, so
        // `symbol-function' returns that cons — a stable identity that
        // nadvice's `(cdr nf)' gv place mutates in place.  Call paths
        // unwrap the cons at dispatch.
        let def = match &def {
            Value::Lambda(l) if l.is_macro => Value::cons(Value::Sym(sym::MACRO), def.clone()),
            _ => def,
        };
        self.macro_gen += 1;
        self.obarray.symbol_mut(id).function = def;
    }

    /// `get` — symbol property.
    pub fn get_prop(&self, id: SymId, prop: SymId) -> Value {
        // `intern_soft` misses surface as SymId::MAX at ~40 call sites —
        // an absent symbol has no properties.
        if id == u32::MAX || prop == u32::MAX {
            return Value::Nil;
        }
        let plist = &self.obarray.symbol(id).plist;
        plist_get(plist, prop)
    }

    /// `put` — set symbol property, returns value.
    pub fn put_prop(&mut self, id: SymId, prop: SymId, val: Value) {
        if id == u32::MAX || prop == u32::MAX {
            return; // see get_prop: intern_soft miss sentinel
        }
        self.macro_gen += 1;
        if prop == self.var_alias_prop {
            self.obarray.symbol_mut(id).var_alias = match val {
                Value::Sym(s) => Some(s),
                _ => None,
            };
        }
        let plist = self.obarray.symbol(id).plist.clone();
        let new_plist = plist_put(&plist, prop, val);
        self.obarray.symbol_mut(id).plist = new_plist;
    }

    // ---------- specbind (dynamic let) ----------

    /// Push a dynamic binding for `sym` to `val`.
    #[track_caller]
    pub fn specbind(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        if self.obarray.symbol(id).constant {
            if self.dbg_flags & 0x100 != 0 {
                eprintln!(
                    "[bindconst {}] head_form={} caller={:?}",
                    self.symbol_name(id),
                    self.princ_to_string(&self.cur_head_form)
                        .chars()
                        .take(400)
                        .collect::<String>(),
                    std::panic::Location::caller()
                );
                for (f, a) in self.lisp_stack.iter().rev().take(20).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f)
                            .chars()
                            .take(100)
                            .collect::<String>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(160)
                            .collect::<String>()
                    );
                }
            }
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let sym = self.obarray.symbol(id);
        let is_auto_local = sym.make_local_if_set || sym.always_local;
        let mut bound_buf = None;
        // The current buffer's RefCell may already be borrowed when
        // specbind runs inside eval-driven macroexpansion (e.g. a
        // `with-temp-buffer' wrapper holding the buffer).  GNU binds
        // the local slot; when we can't borrow we must not panic —
        // fall back to the global binding for the duration.
        let local_here = is_auto_local
            || self
                .buffers
                .get(self.current_buffer)
                .and_then(|b| {
                    b.try_borrow()
                        .ok()
                        .map(|bb| bb.locals.contains_key(&id))
                })
                .unwrap_or(false);
        // GNU's set_internal fires watchers BEFORE the store: a
        // signalling watcher aborts the binding push entirely.  The
        // specpdl entry still goes on first — an erroring watcher
        // leaves the binding popped by unwind, same as GNU.
        self.fire_var_watchers(id, &val, "let", local_here.then_some(self.current_buffer))?;
        if local_here {
            if let Some(b) = self.buffers.get(self.current_buffer) {
                if let Ok(mut bb) = b.try_borrow_mut() {
                    let old = bb.locals.insert(id, val.clone());
                    if id == self.bfn_sym {
                        // BVAR: the buffer slot is the binding.
                        bb.file_name = match &val {
                            Value::Str(s) => Some(s.borrow().clone()),
                            _ => None,
                        };
                    }
                    drop(bb);
                    self.specbind.push(SpecBind {
                        sym: id,
                        buf: Some(self.current_buffer),
                        old,
                    });
                    bound_buf = Some(self.current_buffer);
                }
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
        Ok(())
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
                        if sb.sym == self.bfn_sym {
                            bb.file_name = match bb.locals.get(&sb.sym) {
                                Some(Value::Str(s)) => Some(s.borrow().clone()),
                                _ => None,
                            };
                        }
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

    /// GNU `default_toplevel_binding': the first (outermost) global
    /// specbind entry for `id`.  Used by `defvar' to detect a `let'
    /// shadowing a still-void default binding.
    pub fn specbind_outermost_unbound(&self, id: SymId) -> bool {
        self.specbind.iter().any(|sb| {
            sb.sym == id
                && sb.buf.is_none()
                && matches!(&sb.old, Some(Value::Sym(s)) if *s == sym::UNBOUND)
        })
    }

    /// GNU `set_specpdl_old_value' on the outermost global specbind
    /// for `id` — installing a value that surfaces as the default when
    /// the shadowing `let' unwinds.
    pub fn set_specbind_outermost_old(&mut self, id: SymId, val: Value) {
        if let Some(sb) = self
            .specbind
            .iter_mut()
            .find(|sb| sb.sym == id && sb.buf.is_none())
        {
            sb.old = Some(val);
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
        if std::env::var_os("QUITDBG").is_some() && sym_id == sym::QUIT {
            let names: Vec<String> = self
                .lisp_stack
                .iter()
                .rev()
                .take(24)
                .map(|(f, _)| self.prin1_to_string(f))
                .collect();
            eprintln!("SIGQUIT stack={names:?}");
            eprintln!("{}", std::backtrace::Backtrace::capture());
        }
        Flow::Signal(Value::Sym(sym_id), data, false)
    }

    /// `(signal sym data-list-from-vec)`.
    pub fn signal_data(&self, sym_id: SymId, data: Vec<Value>) -> Flow {
        *self.last_error_stack.borrow_mut() = self.lisp_stack.clone();
        if std::env::var_os("QUITDBG").is_some() && sym_id == sym::QUIT {
            let names: Vec<String> = self
                .lisp_stack
                .iter()
                .rev()
                .take(24)
                .map(|(f, _)| self.prin1_to_string(f))
                .collect();
            eprintln!("SIGQUIT(stack)={names:?}");
            eprintln!("{}", std::backtrace::Backtrace::capture());
        }
        if let Some(filter) = (self.dbg_flags & 0x800000 != 0)
            .then(|| std::env::var("REMACS_BT_ERR").unwrap_or_default())
        {
            let rendered = self
                .prin1_to_string(&Value::list(data.clone()))
                .chars()
                .take(160)
                .collect::<String>();
            let shown = format!("{} {}", self.symbol_name(sym_id), rendered);
            if filter == "1" || shown.contains(&filter) {
                eprintln!("=== SIGNAL {shown} ===");
                for (f, a) in self.lisp_stack.iter().rev().take(30).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f).chars().take(80).collect::<String>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(100)
                            .collect::<String>()
                    );
                }
                if self.lisp_stack.is_empty() {
                    eprintln!("{}", std::backtrace::Backtrace::capture());
                }
            }
        }
        Flow::Signal(Value::Sym(sym_id), Value::list(data), false)
    }

    /// `(error "fmt" args...)` — signals `error` with a formatted message.
    pub fn error(&self, msg: impl Into<String>) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg.into())])
    }

    /// GNU C `error (FMT, ...)': doprnt translates FMT's `like this'
    /// quotes per `text-quoting-style' — never the `%s' arguments.
    pub fn error_q(&self, fmt: &str, args: &[&str]) -> Flow {
        let fmt = crate::lisp::builtins::evalfn::translate_message_quotes(self, fmt);
        let mut args = args.iter();
        let mut out = String::with_capacity(fmt.len());
        let mut rest = fmt.as_str();
        while let Some(p) = rest.find("%s") {
            out.push_str(&rest[..p]);
            out.push_str(args.next().copied().unwrap_or_default());
            rest = &rest[p + 2..];
        }
        out.push_str(rest);
        self.error(out)
    }

    /// `wrong-type-argument` signal: pred, value.
    pub fn wrong_type_mut(&mut self, pred: &str, val: &Value) -> Flow {
        if std::env::var_os("REMACS_TRACE_WT").is_some() {
            let top: Vec<String> = self
                .lisp_stack
                .iter()
                .rev()
                .take(14)
                .map(|(f, a)| {
                    format!(
                        "{}({})",
                        self.princ_to_string(f).chars().take(60).collect::<String>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(80)
                            .collect::<String>()
                    )
                })
                .collect();
            eprintln!("=== wt {pred} on {:?}; top: {}", val, top.join(" <- "));
        }
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
        if let Some(s) = self.sym_id(handlers) {
            if s == sym::NIL {
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
                _ if self.sym_id(h).is_some() => {
                    // A signal matches if `cond` is the signal symbol or
                    // appears in its `error-conditions` property.
                    // Handler names may be positioned symbols (records)
                    // when `symbols-with-pos-enabled' is on.
                    let cond = self.sym_id(h).unwrap();
                    if let Value::Sym(sig_id) = sig {
                        if *sig_id == cond || self.condition_has(*sig_id, cond) {
                            found = true;
                        }
                    }
                }
                Value::Cons(_) => {
                    // (cond ...) group? Emacs allows a list of symbols too.
                    h.each_car(|c| {
                        if let (Some(c_id), Value::Sym(sig_id)) = (self.sym_id(c), sig) {
                            if *sig_id == c_id || self.condition_has(*sig_id, c_id) {
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
        let r = self.eval_str_inner(src, false);
        self.lexenv = saved_lexenv;
        r
    }

    /// `eval-buffer'/`eval-region' source: read errors then carry GNU's
    /// buffer-source (LINE COLUMN) on `invalid-read-syntax'.
    pub fn eval_str_buf(&mut self, src: &str) -> EvalResult {
        let lex_on = self
            .obarray
            .intern_soft("lexical-binding")
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false);
        let saved_lexenv =
            std::mem::replace(&mut self.lexenv, if lex_on { lexenv_root() } else { None });
        let r = self.eval_str_inner(src, true);
        self.lexenv = saved_lexenv;
        r
    }

    fn eval_str_inner(&mut self, src: &str, from_buffer: bool) -> EvalResult {
        // The reader borrows `self`, so create/drop it per form — but
        // share one collected char buffer across all reads.
        let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            let next = {
                let mut reader = Reader::with_chars(self, chars.clone());
                reader.set_position(pos);
                reader.from_buffer = from_buffer;
                match reader.read()? {
                    Some(f) => Some((f, reader.position())),
                    None => None,
                }
            };
            match next {
                Some((form, end)) => {
                    pos = end;
                    if self.dbg_flags & 0x10000 != 0 {
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
                            if self.dbg_flags & 0x1000 != 0 {
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
            let mut reader = Reader::with_chars(self, chars.clone());
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

    /// Evaluate every top-level form in SRC, returning the last
    /// form's value — GNU `--eval' processes the whole string
    /// (command-line-1 → readevalloop-style iteration).
    pub fn eval_forms(&mut self, src: &str) -> EvalResult {
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            let (form, end) = match self.read_from_string(src, pos) {
                Ok(v) => v,
                Err(Flow::Signal(sym, data, r)) => {
                    // A read error past the last real form behaves
                    // like EOF: trailing whitespace/comments only.
                    let is_eof = matches!(&sym, Value::Sym(id) if {
                        let n = self.symbol_name(*id);
                        n == "end-of-file"
                    });
                    if is_eof {
                        break;
                    }
                    return Err(Flow::Signal(sym, data, r));
                }
                Err(f) => return Err(f),
            };
            if end <= pos {
                break; // reader made no progress — done
            }
            pos = end;
            let mark = self.specbind_depth();
            let lb = self.intern("lexical-binding");
            let _ = self.specbind(lb, Value::t());
            let saved_lexenv = std::mem::replace(&mut self.lexenv, lexenv_root());
            let r = self.eval(&form);
            self.lexenv = saved_lexenv;
            self.unbind_to(mark)?;
            match r {
                Ok(v) => last = v,
                Err(Flow::Throw(tag, val)) => {
                    let nc = self.intern("no-catch");
                    return Err(self.signal_data(nc, vec![tag, val]));
                }
                Err(f) => return Err(f),
            }
        }
        Ok(last)
    }

    /// `read-from-string` core: read one object, return it + end position.
    pub fn read_from_string(&mut self, src: &str, start: usize) -> Result<(Value, usize), Flow> {
        self.read_from_string_pos(src, start, None)
    }

    /// `read_from_string' with `read-positioning-symbols' mode: symbol
    /// tokens become `symbol-with-pos' records carrying `base + token
    /// char index' as their position.  Under a non-nil
    /// `symbols-with-pos-enabled' the reader emits GNU's enabled sympos
    /// instead (real symbols for all primitives; `eq' unwraps them).
    /// The flag also activates position emission for plain `read' calls
    /// (GNU `readevalloop' parity).
    pub fn read_from_string_pos(
        &mut self,
        src: &str,
        start: usize,
        pos_base: Option<i128>,
    ) -> Result<(Value, usize), Flow> {
        let enabled = crate::lisp::builtins::misc::sympos_enabled_p(self);
        let mut reader = Reader::new(self, src);
        reader.set_position(start);
        // GNU parity: `locate_syms' is a per-read flag set only by
        // `read-positioning-symbols' (our explicit pos_base).  Plain
        // `read'/`read-from-string' never position symbols, no matter
        // how `symbols-with-pos-enabled' is bound.
        reader.annotate_pos = pos_base;
        reader.sympos_enabled = enabled;
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }

    /// Buffer-source read: like `read_from_string_pos' but read errors
    /// carry GNU's `(MSG LINE COLUMN)' invalid-read-syntax data.
    pub fn read_from_string_buf(
        &mut self,
        src: &str,
        start: usize,
    ) -> Result<(Value, usize), Flow> {
        let enabled = crate::lisp::builtins::misc::sympos_enabled_p(self);
        let mut reader = Reader::new(self, src);
        reader.set_position(start);
        reader.sympos_enabled = enabled;
        reader.from_buffer = true;
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }

    // ---------- eval ----------

    pub fn eval(&mut self, form: &Value) -> EvalResult {
        self.eval_depth += 1;
        if self.dbg_flags & 0x02 != 0 && self.eval_depth > 0 && self.eval_depth % 200 == 0 {
            eprintln!(
                "[depth={}] {}",
                self.eval_depth,
                self.princ_to_string(form)
                    .chars()
                    .take(80)
                    .collect::<String>()
            );
        }
        if self.eval_depth > self.max_lisp_eval_depth {
            // GNU reads Vmax_lisp_eval_depth on every entry: `setq' and
            // dynamic `let' both change the live limit, so resync the
            // cached field from the variable before failing.
            let vid = self.intern("max-lisp-eval-depth");
            if let Value::Int(d) = self.symbol_value(vid) {
                if d > 0 {
                    self.max_lisp_eval_depth = d as usize;
                }
            }
            if self.eval_depth > self.max_lisp_eval_depth {
                self.eval_depth -= 1;
                // GNU eval.c: the depth cap signals `excessive-lisp-nesting'
                // (subordinate to `recursion-error'), not a generic `error'.
                let nsym = self.intern("excessive-lisp-nesting");
                let msg = Value::string("Lisp nesting exceeds `max-lisp-eval-depth'");
                return Err(self.signal_data(nsym, vec![msg]));
            }
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
        // GNU signal_or_quit runs `handler-bind' handlers for every
        // raised signal, whether or not a condition-case will claim it
        // — they observe at raise time; a normal return still lets
        // signaling continue.  Handler nonlocal exits supersede the
        // original signal.
        if !self.handler_bindings.is_empty() {
            let cond = Value::cons(sig.clone(), data.clone());
            for idx in (0..self.handler_bindings.len()).rev() {
                let (conds, handler) = self.handler_bindings[idx].clone();
                if self.signal_matches(sig, &conds) {
                    // While a handler runs only strictly-outer bindings
                    // are visible — a handler's own signal can't
                    // re-enter it.
                    let tail = self.handler_bindings.split_off(idx);
                    let r = self.apply(&handler, vec![cond.clone()]);
                    self.handler_bindings.extend(tail);
                    r?;
                }
            }
        }
        // `signal-hook-function' is GNU's debugger-entry hook: it runs
        // only when no condition-case will claim the signal.
        for conds in self.case_handlers.iter().rev() {
            if self.signal_matches(sig, conds) {
                return Ok(());
            }
        }
        if let Some(id) = self.intern_soft("signal-hook-function") {
            if self.bound_p(id) {
                let hook = self.symbol_value(id);
                if hook.truthy() {
                    self.apply(&hook, vec![sig.clone(), data.clone()])?;
                }
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
        if cfg!(debug_assertions) && self.dbg_flags & 0x08 != 0 {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n > 0 && n % 20_000_000 == 0 {
                let names: Vec<String> = self
                    .lisp_stack
                    .iter()
                    .rev()
                    .take(30)
                    .map(|(f, _)| self.prin1_to_string(f))
                    .collect();
                eprintln!("EVAL-WATCH n={n} stack={names:?}");
                eprintln!("EVAL-WATCH form={}", self.prin1_to_string(form));
            }
        }
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
            | Value::Marker(_)
            | Value::Window(_)
            | Value::Frame(_)
            | Value::Process(_)
            | Value::Thread(_)
            | Value::Mutex(_)
            | Value::CondVar(_)
            | Value::Finalizer(_) => Ok(form.clone()),
            Value::Sym(id) => self.eval_symbol(*id),
            Value::Record(_) => {
                // `symbols-with-pos-enabled' objects evaluate like
                // their bare symbol (GNU: they are real symbols).
                match crate::lisp::builtins::misc::as_sympos(form) {
                    Some((s, _)) => self.eval_symbol(s),
                    None => Ok(form.clone()),
                }
            }
            Value::Cons(_) => self.eval_form(form),
        }
    }

    fn eval_symbol(&mut self, id: SymId) -> EvalResult {
        if id == sym::NIL {
            return Ok(Value::Nil);
        }
        // Lexical lookup first — GNU `eval_sub' does `Fassq (form,
        // internal-interpreter-environment)' unconditionally ("we do not
        // pay attention to the declared_special flag here"): a closure
        // parameter that became special AFTER capture (e.g. via
        // `internal--define-uninitialized-variable', as
        // `custom-initialize-after-file-load' does) still reads the
        // lexical binding, matching byte-compiled GNU code where the
        // parameter is a stack slot.
        if let Some(v) = lexenv_lookup(&self.lexenv, id) {
            return Ok(v);
        }
        let v = self.symbol_value(id);
        if let Value::Sym(s) = &v {
            if *s == sym::UNBOUND {
                if self.dbg_flags & 0x10 != 0 {
                    let name = self.obarray.symbol(id).name.clone();
                    let mut depth = 0;
                    let mut env = &self.lexenv;
                    let mut found = false;
                    while let Some(f) = env {
                        depth += 1;
                        if f.vars.borrow().iter().any(|(s, _)| *s == id) {
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
                    for (f, a) in self.lisp_stack.iter().rev().take(25) {
                        eprintln!("   bt {} nargs={}", self.prin1_to_string(f), a.len());
                    }
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
            if std::env::var_os("QUITDBG").is_some() {
                let names: Vec<String> = self
                    .lisp_stack
                    .iter()
                    .rev()
                    .take(24)
                    .map(|(f, _)| self.prin1_to_string(f))
                    .collect();
                eprintln!("QUIT-AT stack={names:?}");
            }
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
        // `symbols-with-pos-enabled' symbols in operator position are
        // real symbols (GNU parity): dispatch on the bare symbol.
        let head = crate::lisp::builtins::misc::unpos(&head);
        // GNU `eval_sub' runs CHECK_LIST on the arg spine before
        // dispatch (and `list_length' once fun is known), so a dotted
        // call `(f a . b)' signals `wrong-type-argument listp b'
        // before the function — even before a special form's body
        // or `if''s test is evaluated.  Circular spines stay legal:
        // GNU's FOR_EACH_TAIL halts on a repeat, so only a truly
        // non-cons tail is an error.
        let spine_len = {
            // Raw-pointer tortoise/hare over the cdr chain: identical
            // semantics to the Value-clone walk it replaces (nil tail
            // ends the spine, a non-cons tail is `listp' error, a
            // pointer meet means circular and legal).
            let mut slow = &args as *const Value;
            let mut fast = &args as *const Value;
            let mut guard = true;
            let mut spine_len: i128 = 0;
            loop {
                let sv = unsafe { &*slow };
                match sv {
                    Value::Cons(c) => {
                        slow = unsafe { cdr_ptr(c) };
                        spine_len += 1;
                    }
                    Value::Nil => break,
                    other => return Err(self.wrong_type_mut("listp", other)),
                }
                if !guard {
                    continue;
                }
                match unsafe { &*fast } {
                    Value::Cons(c) => {
                        let n1 = unsafe { cdr_ptr(c) };
                        match n1 {
                            Value::Cons(c2) => fast = unsafe { cdr_ptr(c2) },
                            _ => {
                                fast = n1;
                                guard = false;
                            }
                        }
                    }
                    _ => guard = false,
                }
                if cons_ptr(unsafe { &*slow }) == cons_ptr(unsafe { &*fast })
                    && matches!(unsafe { &*slow }, Value::Cons(_))
                {
                    // Circular spine: length is unusable.
                    spine_len = -1;
                    break;
                }
            }
            spine_len
        };

        if self.dbg_flags & 0x80 != 0 {
            eprintln!(
                "[eval_form head={} args={}]",
                self.princ_to_string(&head),
                self.princ_to_string(&args)
            );
        }
        match &head {
            Value::Sym(id) => {
                let id = *id;
                // Special form?  GNU checks each special form's own
                // minimum arity inside the evaluator (Fif needs 2,
                // Fsetq needs pairs, ...): report `(SYM N)'.
                if let Some(sf) = super::special::special_form(id) {
                    let min = super::special::special_form_min_args(id) as i128;
                    let n = match spine_len {
                        n if n >= 0 => n,
                        _ => self.raw_list_length(&args)?,
                    };
                    if n < min {
                        // GNU's `defun'/`defmacro' are byte-coded
                        // macros: their too-few-args error carries the
                        // trampoline arity `(2 . 2)' as the function
                        // datum, not the symbol.
                        let fun = if id == sym::DEFUN || id == sym::DEFMACRO {
                            Value::cons(Value::Int(2), Value::Int(2))
                        } else {
                            Value::Sym(id)
                        };
                        return Err(self.wrong_number_of_args(&fun, n));
                    }
                    return sf(self, args);
                }
                if self.dbg_flags & 0x01 != 0 {
                    self.cur_head_form = form.clone();
                }
                // Function cell.
                let fun = self.form_function(id);
                if let Value::Sym(s) = &fun {
                    if *s == sym::UNBOUND {
                        if std::env::var_os("REMACS_TRACE_SETF").is_some() {
                            eprintln!(
                                "void head: {} in form {}",
                                self.symbol_name(id),
                                self.prin1_to_string(&form)
                                    .chars()
                                    .take(200)
                                    .collect::<String>()
                            );
                        }
                        return Err(self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(id)]));
                    }
                }
                // Macro call: GNU's evaluator re-expands the form on
                // every call, but its dumped libraries run
                // byte-compiled so expansion happens once at compile
                // time.  remacs interprets them — caching the
                // expansion on the call-site cons cell (invalidated
                // by `macro_gen' and the fn cell's identity) keeps
                // interpreted cl-*/pcase/eieio code usable.
                // Inside a `compiled' (dumped-library-equivalent)
                // function, a symbol-head call to a byte-op primitive
                // dispatches to its canonical subr — GNU's compiled
                // `(car x)' emits Bcar, which never reads
                // `symbol-function', so advising `car' can't make the
                // advice machinery's own calls recurse.
                if self.dumped_call_depth > 0 {
                    if let Some(s) = self.byte_op_subr(id) {
                        return self.call_function(&Value::Subr(s), &args, Some(id));
                    }
                }
                if self.is_macro_function(&fun) {
                    let key = Rc::as_ptr(cons) as usize;
                    // Inside dumped-library (compiled-equivalent) lambdas
                    // macro calls were expanded at "compile" time — i.e.
                    // against the pre-advice cell.  Strip any advice
                    // trampolines so interpreted nadvice/gv code behaves
                    // like GNU's .elc where `when' is an unadvised `if'.
                    let fun = if self.dumped_call_depth > 0 {
                        self.unadvised_base(&fun)
                    } else {
                        fun
                    };
                    if let Some((stamp, fncell, expansion, _pin)) = self.macro_cache.get(&key) {
                        // `(macro . f)' cells are mutated in place by
                        // nadvice's gv machinery, so compare the cdr
                        // too, not just the cons identity.
                        let same_fn = match (&fun, fncell) {
                            (Value::Cons(a), Value::Cons(b)) => {
                                let (aa, bb) = (a.borrow(), b.borrow());
                                crate::lisp::builtins::eq_values(&aa.car, &bb.car)
                                    && crate::lisp::builtins::eq_values(&aa.cdr, &bb.cdr)
                            }
                            _ => crate::lisp::builtins::eq_values(&fun, fncell),
                        };
                        if *stamp == self.macro_gen && same_fn {
                            let expansion = expansion.clone();
                            return self.eval(&expansion);
                        }
                    }
                    let expansion = self.eval_macro_expand(&fun, &args)?;
                    if self.macro_cache.len() >= 262144 {
                        self.macro_cache.clear();
                    }
                    self.macro_cache
                        .insert(key, (self.macro_gen, fun, expansion.clone(), form.clone()));
                    return self.eval(&expansion);
                }
                self.call_function_len(&fun, &args, Some(id), spine_len)
            }
            Value::Cons(_) => {
                // `((lambda (x) ...) a b)` — function position lambda is
                // used directly, not evaluated as a call.
                let b = match &head {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => unreachable!(),
                };
                if self.sym_is(&b, sym::LAMBDA) {
                    // GNU `eval_sub': `((lambda ...) ARGS)' calls
                    // `funcall_lambda' on the RAW form — dynamic
                    // binding, no closure conversion, `invalid-
                    // function'/`wna' data is the form itself.
                    let argv = self.eval_args_len(&args, spine_len)?;
                    return self.call_lambda_form(&head, argv);
                }
                // GNU uses the cons HEAD itself as the function object:
                // `(lambda ...)'/`(closure ...)'/`(macro . f)' are
                // callable, anything else signals invalid-function with
                // the cons (e.g. (('q)) → invalid-function (quote q)).
                self.call_function_len(&head, &args, None, spine_len)
            }
            _ => {
                let fun = self.eval(&head)?;
                self.call_function_len(&fun, &args, None, spine_len)
            }
        }
    }

    /// Evaluate each element of a list form, using PRE_LEN when the
    /// caller already counted the spine (eval_form's check walk).
    pub fn eval_args_len(&mut self, args: &Value, pre_len: i128) -> Result<Argv, Flow> {
        // Raw-pointer spine walk: GNU's `eval_sub' reads the cdr AFTER
        // evaluating the car, so a `setcdr' by an argument is seen.
        // Cells stay alive through `args' while the walk runs.
        let cap = if pre_len >= 0 {
            pre_len as usize
        } else {
            self.raw_list_length(args).unwrap_or(4) as usize
        };
        let mut out = Vec::with_capacity(cap);
        let mut cur = args as *const Value;
        loop {
            match unsafe { &*cur } {
                Value::Nil => return Ok(Argv::new(out)),
                Value::Cons(c) => {
                    let cell = RefCell::as_ptr(c);
                    out.push(self.eval(unsafe { &(*cell).car })?);
                    cur = unsafe { &(*cell).cdr };
                }
                _ => {
                    return Err(self.error("Invalid function call form (dotted)"));
                }
            }
        }
    }

    /// Evaluate each element of a list form.
    pub fn eval_args(&mut self, args: &Value) -> Result<Vec<Value>, Flow> {
        // Raw-pointer spine walk: GNU's `eval_sub' reads the cdr AFTER
        // evaluating the car, so a `setcdr' by an argument is seen.
        // Cells stay alive through `args' while the walk runs.
        let mut out = Vec::with_capacity(self.raw_list_length(args).unwrap_or(4) as usize);
        let mut cur = args as *const Value;
        loop {
            match unsafe { &*cur } {
                Value::Nil => return Ok(out),
                Value::Cons(c) => {
                    let cell = RefCell::as_ptr(c);
                    out.push(self.eval(unsafe { &(*cell).car })?);
                    cur = unsafe { &(*cell).cdr };
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
        self.call_function_len(fun, args, sym_name, -1)
    }

    /// `call_function` with a precomputed spine length (-1 = unknown).
    /// eval_form passes the count its arg-check walk already computed.
    fn call_function_len(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<SymId>,
        spine_len: i128,
    ) -> EvalResult {
        let fun = crate::lisp::builtins::misc::unpos(fun);
        match &fun {
            Value::Sym(id) => {
                // Function alias chain: chase.  GNU's
                // `indirect_function' signals `cyclic-function-indirection'
                // with the symbol whose alias points back into the chain.
                // Most calls resolve in one hop — the `seen' Vec only
                // allocates once an actual alias step happens.
                let mut cur = *id;
                let mut seen: Option<Vec<SymId>> = None;
                loop {
                    // GNU resolves defalias chains in `eval' before
                    // dispatching, so an alias to a special form
                    // (e.g. `inline' -> `progn') is called as a
                    // special form with unevaluated arguments.
                    if let Some(sf) = super::special::special_form(cur) {
                        let min = super::special::special_form_min_args(cur) as i128;
                        let n = if spine_len >= 0 {
                            spine_len
                        } else {
                            self.raw_list_length(&args)?
                        };
                        if n < min {
                            return Err(self.wrong_number_of_args(&Value::Sym(cur), n));
                        }
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
                            if seen.is_none() {
                                seen = Some(vec![cur]);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.intern("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![Value::Sym(cur)]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().push(cur);
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
                Arity::Unevalled => (s.func)(self, Argv::new(vec![args.clone()])),
                _ => {
                    // GNU checks a subr's arity before evaluating the
                    // argument forms, so `(car BAD EXTRA)' reports
                    // `wrong-number-of-arguments' rather than whatever
                    // BAD would signal.
                    let n = if spine_len >= 0 {
                        spine_len
                    } else {
                        self.raw_list_length(args)?
                    };
                    self.check_arity_subr_n(s, n, sym_name)?;
                    let argv = self.eval_args_len(args, spine_len)?;
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
                    let expansion = self.eval_macro_expand(&fun, args)?;
                    return self.eval(&expansion);
                }
                // GNU byte-compiles its dumped defuns, and compiled
                // functions check arity before argument forms are
                // evaluated (interpreted lambdas do not).  `dumped_doc'
                // marks our dumped-equivalent definitions.
                if l.dumped_doc {
                    let n = if spine_len >= 0 {
                        spine_len
                    } else {
                        self.raw_list_length(args)?
                    };
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    self.check_arity_lambda_n(l, n, &shown)?;
                }
                // Dumped `#[ARGDESC ...]' twins (compiled, no parsed
                // arglist) get the same early check — GNU's nativecomp
                // signal data is the called symbol, while apply/funcall
                // truncate excess args instead (exec() does that).
                if !l.dumped_doc && l.compiled {
                    if let Some(items) = &l.bc_items {
                        if super::bytecode::is_byte_code(&items.borrow()) {
                            let n = if spine_len >= 0 {
                                spine_len
                            } else {
                                self.raw_list_length(args)?
                            };
                            let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                            super::bytecode::check_argdesc(
                                self,
                                &items.borrow(),
                                n.max(0) as usize,
                                &shown,
                            )?;
                        }
                    }
                }
                let argv = self.eval_args_len(args, spine_len)?;
                let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                self.apply_resolved(&fun, argv, shown)
            }
            Value::Cons(_) => {
                // A cons as function: `(lambda ...)` form or `(macro . f)`.
                let (car, cdr) = {
                    let c = match &fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    // (macro . lambda) — expand with cdr as function.
                    let expansion = self.eval_macro_expand(&cdr, args)?;
                    return self.eval(&expansion);
                }
                if self.sym_is(&car, sym::LAMBDA) {
                    // Symbol-function indirection's cons form: GNU
                    // `funcall_lambda' on `(lambda ...)'.  `((function
                    // (lambda ...)))' (QUOTE_FUNCTION head) is NOT a
                    // lambda call — `function' is no macro here, so GNU
                    // `invalid-function' on the cons.
                    let argv = self.eval_args_len(args, spine_len)?;
                    return self.call_lambda_form(&fun, argv);
                }
                let auto_id = self.intern("autoload");
                if self.sym_is(&car, auto_id) {
                    // (autoload FILE ...) — load, then re-dispatch on
                    // the real definition (macro autoloads expand).
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    return self.call_function(&newdef, args, sym_name);
                }
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name.map(|s| self.symbol_name(s)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => {
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun_ {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name.map(|s| self.symbol_name(s)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
        }
    }

    /// Length of a raw (unevaluated) argument list: the number of cons
    /// cells in its spine.  A dotted tail is not counted; a circular
    /// list signals `circular-list' like GNU's `Flength'.
    pub(crate) fn raw_list_length(&mut self, args: &Value) -> Result<i128, Flow> {
        // Raw-pointer walk — same tortoise/hare logic as before, minus
        // a `Value` clone + drop per spine cell.
        let mut n = 0i128;
        let mut cur = args as *const Value;
        let mut hare: Option<*const Value> = Some(args);
        loop {
            let next = match unsafe { &*cur } {
                Value::Cons(c) => unsafe { cdr_ptr(c) },
                _ => break,
            };
            n += 1;
            cur = next;
            // Hare advances two cells per step while it can; if it
            // meets `cur' the spine loops.
            if let Some(h) = hare {
                let step = |v: *const Value| -> *const Value {
                    match unsafe { &*v } {
                        Value::Cons(c) => unsafe { cdr_ptr(c) },
                        _ => v,
                    }
                };
                let h1 = step(h);
                if matches!(unsafe { &*h1 }, Value::Cons(_)) {
                    hare = Some(step(h1));
                    if let (Value::Cons(_), Some(hp)) = (unsafe { &*cur }, hare) {
                        if cons_ptr(unsafe { &*cur }) == cons_ptr(unsafe { &*hp }) {
                            return Err(crate::lisp::builtins::listfn::err_circular(self));
                        }
                    }
                } else {
                    hare = None;
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
    pub fn apply(&mut self, fun: &Value, argv: impl Into<Argv>) -> EvalResult {
        if self.dbg_flags & 0x01 != 0 {
            if let Value::Cons(c) = fun {
                let car = c.borrow().car.clone();
                if !(self.sym_is(&car, sym::LAMBDA) || self.sym_is(&car, sym::MACRO)) {
                    eprintln!(
                        "[apply-raw {}]\n{}",
                        self.princ_to_string(fun)
                            .chars()
                            .take(150)
                            .collect::<String>(),
                        std::backtrace::Backtrace::capture()
                    );
                }
            }
        }
        let argv = argv.into();
        let fun = crate::lisp::builtins::misc::unpos(fun);
        match &fun {
            Value::Sym(id) => {
                // Most calls resolve in one hop — the `seen' Vec only
                // allocates once an actual alias step happens.
                let mut cur = *id;
                let mut seen: Option<Vec<SymId>> = None;
                loop {
                    let f = self.callable_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                // GNU `Ffuncall' reports the called
                                // symbol, not the alias chain's tail.
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(*id)])
                                );
                            }
                            if seen.is_none() {
                                seen = Some(vec![cur]);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.intern("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![Value::Sym(*id)]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().push(cur);
                        }
                        other => return self.apply_resolved(&other, argv, Value::Sym(*id)),
                    }
                }
            }
            _ => self.apply_resolved(&fun, argv, fun.clone()),
        }
    }

    /// Dispatch a resolved (non-symbol) function, recording the call on
    /// `lisp_stack' so `mapbacktrace' can walk live frames.  `shown' is
    /// what backtraces display as the callee — the symbol when the call
    /// came through a symbol's function cell, else the function itself.
    fn apply_resolved(&mut self, fun: &Value, argv: Argv, shown: Value) -> EvalResult {
        if self.dbg_flags & 0x04 != 0 {
            eprintln!(
                "[call] {}",
                self.princ_to_string(&shown)
                    .chars()
                    .take(90)
                    .collect::<String>()
            );
        }
        if self.bt_at_limit > 0 {
            let limit: usize = self.bt_at_limit;
            if self.lisp_stack.len() >= limit {
                eprintln!("=== REMACS_BT_AT hit, last 40 frames ===");
                for (f, a) in self.lisp_stack.iter().rev().take(40).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f).chars().take(80).collect::<String>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(120)
                            .collect::<String>()
                    );
                }
                std::process::exit(3);
            }
        }
        match fun {
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => {
                    // GNU: special forms cannot be funcalled/applied;
                    // the error data is the #<subr> object, not the
                    // calling symbol.
                    Err(self.signal_data(sym::INVALID_FUNCTION, vec![Value::Subr(s)]))
                }
                _ => {
                    self.check_arity_subr(s, &argv, None)?;
                    let prof = self.dbg_flags & 0x1000000 != 0;
                    let t0 = prof.then(std::time::Instant::now);
                    self.lisp_stack.push((shown, argv.clone()));
                    let r = (s.func)(self, argv);
                    self.lisp_stack.pop();
                    if let Some(t0) = t0 {
                        let e = self.subr_prof.entry(s.name).or_default();
                        e.0 += 1;
                        e.1 += t0.elapsed().as_nanos();
                    }
                    r
                }
            },
            Value::Lambda(l) => {
                // Dumped definitions stand in for GNU's nativecomp'd
                // subrs, and `funcall_subr' hands one with more than 8
                // parameters to its n-adic entry without an upper-bound
                // check: surplus args are dropped.  Call forms already
                // checked arity (`call_function_len', like `eval_sub');
                // bytecode twins get this in `bytecode::exec'.
                let max = l.required.len() + l.optional.len();
                let argv = if l.dumped_doc
                    && !l.is_macro
                    && !l.bad_arglist
                    && !l.is_bytecode()
                    && l.rest.is_none()
                    && max > 8
                    && argv.len() > max
                {
                    Argv::new(argv[..max].to_vec())
                } else {
                    argv
                };
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
                    // GNU `Ffuncall' calls the raw cons directly via
                    // `funcall_lambda' — no closure conversion, dynamic
                    // binding, `invalid-function' datum is the form.
                    self.lisp_stack.push((shown.clone(), argv.clone()));
                    let r = self.call_lambda_form(fun, argv);
                    self.lisp_stack.pop();
                    return r;
                }
                let auto_id = self.intern("autoload");
                if self.sym_is(&car, auto_id) {
                    // GNU `Ffuncall': a bare `(autoload ...)' cons as
                    // the function signals `wrong-type-argument'
                    // `symbolp' — autoload indirection only happens
                    // through a symbol's function cell.
                    if !matches!(shown, Value::Sym(_)) {
                        return Err(self.wrong_type_mut("symbolp", fun));
                    }
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    return self.apply(&newdef, argv);
                }
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[apply-in {} shown={} callform={}]",
                        self.princ_to_string(&fun),
                        self.princ_to_string(&shown),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown.clone()]))
            }
            _ => {
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[applyin_ {} shown={} callform={}]",
                        self.princ_to_string(&fun),
                        self.princ_to_string(&shown),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    );
                }
                // GNU `Ffuncall' signals `void-function' for nil (its
                // "function cell" reads as void); other atoms are
                // `invalid-function'.
                if fun.is_nil() {
                    // The void report is `shown' (the symbol that was
                    // called), not the nil cell.
                    return Err(self.signal_data(sym::VOID_FUNCTION, vec![shown.clone()]));
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown.clone()]))
            }
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

    /// Strip advice trampolines off F, returning the innermost
    /// (pre-advice) function — keeping a `(macro . F)' wrapper if F
    /// itself was macro-wrapped.  Follows `advice_links' entries via
    /// `Lambda.advice_link' and nadvice oclosure layers via their
    /// `nf' env binding.
    pub(crate) fn unadvised_base(&self, f: &Value) -> Value {
        let mut v = f.clone();
        let mut was_macro_cons = false;
        loop {
            match &v {
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if self.sym_is(&car, sym::MACRO) {
                        was_macro_cons = true;
                        v = cdr;
                        continue;
                    }
                    break;
                }
                Value::Lambda(l) => {
                    if let Some(idx) = l.advice_link {
                        if let Some((_, _, next, _)) = self.advice_links.get(idx) {
                            v = next.clone();
                            continue;
                        }
                    }
                    // nadvice's `advice--make' layers bind `nf' (next
                    // function) and `how' in the layer's env — hop
                    // through `nf' to reach the base.  Require `how'
                    // to hold a keyword so unrelated closures that
                    // capture `nf' aren't stripped.
                    if let Some(next) = self.advice_layer_next(l) {
                        v = next;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        if was_macro_cons {
            Value::cons(Value::Sym(sym::MACRO), v)
        } else {
            v
        }
    }

    /// If L is a nadvice `advice' oclosure layer (type tag `advice'
    /// in the closure's docstring slot, `cdr' bound to the next
    /// layer, `how' a keyword), return the next layer.
    fn advice_layer_next(&self, l: &crate::lisp::value::Lambda) -> Option<Value> {
        let advice_tag = self.intern_soft("advice")?;
        match &l.doc_value {
            Some(Value::Sym(s)) if *s == advice_tag => {}
            _ => return None,
        }
        let cdr = self.intern_soft("cdr")?;
        let how = self.intern_soft("how")?;
        let how_v = lexenv_lookup(&l.env, how);
        let is_layer = matches!(&how_v, Some(Value::Sym(s)) if {
            let n = self.symbol_name(*s);
            n.starts_with(':')
        });
        if !is_layer {
            return None;
        }
        lexenv_lookup(&l.env, cdr)
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

    /// The canonical subr for SYM when SYM names a dedicated GNU
    /// byte-code opcode — `car', `cdr', `list', `concat', `eq', the
    /// arithmetic ops, ... — or None.  GNU's compiler emits a
    /// dedicated opcode for each of these calls, so compiled code
    /// invokes the primitive directly and `symbol-function' (and any
    /// advice layered on it) is never consulted; remacs routes the
    /// equivalent calls through this table while a `compiled'
    /// (shipped-library-equivalent) function is running.
    fn byte_op_subr(&mut self, id: SymId) -> Option<&'static crate::lisp::value::Subr> {
        if self.byte_op_subrs.is_none() {
            let mut m = HashMap::default();
            // Names mirrored from GNU's bytecomp.el `byte-defop' table
            // (opcodes 56-168): each compiled call to one of these is
            // a dedicated opcode, not a `byte-call' through
            // `symbol-function'.
            for s in crate::lisp::builtins::collect() {
                match s.name {
                    "car"
                    | "cdr"
                    | "cons"
                    | "list"
                    | "nth"
                    | "nthcdr"
                    | "memq"
                    | "member"
                    | "assq"
                    | "elt"
                    | "length"
                    | "aref"
                    | "aset"
                    | "nreverse"
                    | "setcar"
                    | "setcdr"
                    | "nconc"
                    | "concat"
                    | "substring"
                    | "get"
                    | "set"
                    | "fset"
                    | "symbolp"
                    | "consp"
                    | "stringp"
                    | "listp"
                    | "not"
                    | "null"
                    | "eq"
                    | "equal"
                    | "symbol-value"
                    | "symbol-function"
                    | "symbol-name"
                    | "eqlsign"
                    | "gtr"
                    | "lss"
                    | "leq"
                    | "geq"
                    | "+"
                    | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "max"
                    | "min"
                    | "="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | "point"
                    | "point-max"
                    | "point-min"
                    | "goto-char"
                    | "insert"
                    | "char-after"
                    | "char-before"
                    | "following-char"
                    | "preceding-char"
                    | "current-column"
                    | "indent-to"
                    | "eolp"
                    | "eobp"
                    | "bolp"
                    | "bobp"
                    | "current-buffer"
                    | "set-buffer"
                    | "forward-char"
                    | "forward-word"
                    | "skip-chars-forward"
                    | "skip-chars-backward"
                    | "forward-line"
                    | "char-syntax"
                    | "buffer-substring"
                    | "delete-region"
                    | "narrow-to-region"
                    | "widen"
                    | "end-of-line"
                    | "set-marker"
                    | "match-beginning"
                    | "match-end"
                    | "upcase"
                    | "downcase"
                    | "string="
                    | "string<"
                    | "numberp"
                    | "integerp"
                    | "car-safe"
                    | "cdr-safe"
                    | "interactive-p" => {
                        let id = self.intern(s.name);
                        m.insert(id, s);
                    }
                    _ => {}
                }
            }
            self.byte_op_subrs = Some(m);
        }
        self.byte_op_subrs.as_ref().unwrap().get(&id).copied()
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
    /// Record the bound name for a compiled lambda that doesn't carry
    /// one — `#[...]' literals read from `.elc' files arrive nameless,
    /// and `defalias' is the point where GNU's nativecomp loads learn
    /// the bound symbol (`#<subr NAME>' print + wna datum).  Keyed on
    /// the Rc pointer since `Lambda' is not Clone.
    fn name_compiled_lambda(&mut self, id: SymId, def: &Value) {
        if let Value::Lambda(l) = def {
            if l.name.is_none() && l.compiled {
                self.compiled_fn_names
                    .borrow_mut()
                    .push((Rc::downgrade(l), self.symbol_name(id)));
            }
        }
    }

    /// Effective display name of a lambda: `l.name' plus the
    /// `defalias'-stamped name for `.elc' twins.
    pub(crate) fn lambda_name(&self, l: &Rc<Lambda>) -> Option<String> {
        if let Some(n) = &l.name {
            return Some(n.clone());
        }
        let ptr = Rc::as_ptr(l);
        let names = self.compiled_fn_names.borrow();
        names.iter().find_map(|(w, n)| {
            (w.upgrade().is_some_and(|r| Rc::as_ptr(&r) == ptr)).then(|| n.clone())
        })
    }

    pub fn fset_defalias(&mut self, id: SymId, def: Value) -> Result<(), Flow> {
        self.check_function_assignment(id, &def)?;
        self.name_compiled_lambda(id, &def);
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
    fn call_lambda(&mut self, l: &Rc<Lambda>, argv: Argv, shown: &Value) -> EvalResult {
        // `compiled' lambdas stand in for GNU's byte-compiled bodies:
        // while one runs, symbol-head calls to byte-op primitives
        // dispatch directly (see `byte_op_subr'), matching GNU's
        // compiled code where `(car x)' emits Bcar rather than
        // consulting `symbol-function'.
        let bump = l.dumped_doc || l.compiled;
        if bump {
            self.dumped_call_depth += 1;
        }
        let r = self.call_lambda_inner(l, argv, shown);
        if bump {
            self.dumped_call_depth -= 1;
        }
        r
    }

    /// Dispatch for `#[ARGDESC BYTE-CODE CONSTANTS DEPTH FILE]' literals
    /// read from the dumped uni-*.el char-table extra slots.  GNU runs
    /// the embedded byte-code (unidata-gen.el's `unidata-get-*'/
    /// `unidata-put-*'/describe helpers); we route each of the three
    /// emitted shapes to the native equivalents in builtins::charset.
    /// Returns None for any unrecognized shape so ordinary arity
    /// checking reports the usual error.
    fn unidata_bc_dispatch(&mut self, l: &Rc<Lambda>, argv: &[Value]) -> Option<EvalResult> {
        // Only bare `#[...]' literals read from the dumped uni-*.el data
        // route here.  `make-closure' products splice their captured env
        // into consts[0..]; a mutated box can masquerade as an alist and
        // would be short-circuited into Nil, skipping the real bytecode
        // (seq-reduce's acc cell hit exactly this).
        if l.bc_closure {
            return None;
        }
        if !matches!(&l.arglist, Some(Value::Int(_))) {
            return None;
        }
        let items = l.bc_items.as_ref()?.borrow();
        // Only the uni-*.el literals: argdesc 257 (1 arg) or 771 (3
        // args).  A generic guard — without it any `#[...]' closure
        // whose consts happen to look like an alist (e.g.
        // `case-table-get-table's ((up . 0) ...) table) gets its
        // 1-arg calls hijacked into `assq' lookups instead of the
        // real byte code / arity error.
        let argdesc_n = match items.first() {
            Some(Value::Int(n)) => *n,
            _ => return None,
        };
        if argdesc_n != 257 && argdesc_n != 771 {
            return None;
        }
        let consts = match items.get(2) {
            Some(Value::Vec(v)) => v.borrow().clone(),
            _ => return None,
        };
        let cts = self.intern("char-table-extra-slot");
        if argv.len() == 3 && argdesc_n == 771 {
            let (ch, raw, tbl) = (&argv[0], &argv[1], &argv[2]);
            let Value::Int(n) = ch else { return None };
            if !crate::lisp::builtins::misc::is_char_table(self, tbl) {
                return None;
            }
            if consts.len() == 3
                && matches!(consts[0], Value::Int(0))
                && matches!(&consts[1], Value::Sym(s) if *s == cts)
                && matches!(consts[2], Value::Int(1))
            {
                // `unidata-put-decomposition'/`unidata-put-name': flush
                // the compressed block through the slot-1 getter first,
                // then store VAL verbatim.  Their initial aref validates
                // the original integer before any native narrowing.
                if !ch.fixnump() {
                    return Some(Err(self.wrong_type_mut("fixnump", ch)));
                }
                if !(0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(n) {
                    return Some(Err(self.wrong_type_mut("characterp", ch)));
                }
                let cur =
                    crate::lisp::builtins::misc::char_table_ref(self, tbl, (*n).max(0) as usize);
                if let Value::Str(s) = &cur {
                    if s.borrow().chars().next() == Some('\0') {
                        let _ = crate::lisp::builtins::charset::uniprop_lookup(
                            self,
                            tbl,
                            (*n).max(0) as u32,
                            cur.clone(),
                        );
                    }
                }
                crate::lisp::builtins::misc::ct_set(self, tbl, (*n).max(0) as u32, raw.clone());
                return Some(Ok(raw.clone()));
            }
            // `unidata-get-decomposition'/`unidata-get-name' & kin:
            // decode RAW through the table's slot-1 machinery (keyed on
            // the property name in slot 0).
            if consts
                .iter()
                .any(|c| matches!(c, Value::Sym(s) if *s == cts))
            {
                return Some(Ok(crate::lisp::builtins::charset::uniprop_lookup(
                    self,
                    tbl,
                    (*n).max(0) as u32,
                    raw.clone(),
                )));
            }
            return None;
        }
        if argv.len() == 1 && argdesc_n == 257 {
            if consts.is_empty() {
                return None;
            }
            let mconcat = self.intern("mapconcat");
            if matches!(&consts[0], Value::Sym(s) if *s == mconcat) {
                // `unidata-describe-decomposition': (mapconcat INNER
                // VAL " ") where INNER renders symbols by name and
                // characters as 'C' (compose-string display aside).
                let parts: Vec<String> = argv[0]
                    .list_to_vec()
                    .unwrap_or_default()
                    .iter()
                    .map(|x| match x {
                        Value::Sym(s) => self.symbol_name(*s),
                        Value::Int(n) => char::from_u32((*n).max(0) as u32)
                            .map(|c| format!("'{}'", c))
                            .unwrap_or_default(),
                        _ => self.prin1_to_string(x),
                    })
                    .collect();
                return Some(Ok(Value::string(parts.join(" "))));
            }
            // `unidata-describe-*' alist lookups: (cdr (assq VAL ALIST)).
            let alist = consts.first()?;
            if matches!(alist, Value::Cons(_) | Value::Vec(_)) {
                let v = argv[0].clone();
                let elems: Vec<Value> = match alist {
                    Value::Cons(_) => alist.list_to_vec().unwrap_or_default(),
                    Value::Vec(vv) => vv.borrow().clone(),
                    _ => Vec::new(),
                };
                // Require a unidata-style alist: every element a cons
                // pair `(KEY . STRING)' — uni-*.el tables map property
                // keys to description strings.  `make-closure'
                // products splice closure vars into consts[0..] and
                // arbitrary `#[257]' literals (byte-compiled lambdas
                // whose consts[0] happens to be a list, e.g. the
                // `oclosure--lambda' mapcar helper with consts[0] =
                // ((progn nil))) must fall through to the real VM.
                let is_unidata_alist = elems.iter().all(
                    |e| matches!(e, Value::Cons(p) if matches!(&p.borrow().cdr, Value::Str(_))),
                );
                if !is_unidata_alist {
                    return None;
                }
                for e in elems {
                    if let Value::Cons(p) = &e {
                        if crate::lisp::builtins::eq_values(&p.borrow().car, &v) {
                            return Some(Ok(p.borrow().cdr.clone()));
                        }
                    }
                }
                return Some(Ok(Value::Nil));
            }
        }
        None
    }

    /// GNU `funcall_lambda' on a raw `(lambda ...)' cons — the
    /// `funcall'/`apply' path, which never converts the form to a
    /// closure.  Args bind dynamically (`lexenv' is nil for raw forms);
    /// `(lambda)', `(lambda . X)', `(lambda a ...)' and dotted/non-
    /// symbol arglist elements all signal `invalid-function FORM'
    /// (datum is the form itself, not a closure).  The body runs as
    /// GNU's `Fprogn (XCDR (XCDR fun))': cons elements evaluate, a
    /// non-cons tail ends evaluation silently.
    fn call_lambda_form(&mut self, fun: &Value, argv: Argv) -> EvalResult {
        let cdr = match fun {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => unreachable!(),
        };
        let arglist = match &cdr {
            Value::Cons(c) => c.borrow().car.clone(),
            _ => {
                return Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]));
            }
        };
        let body = match &cdr {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => Value::Nil,
        };
        // Dynamic binding, no lexical env (GNU: `lexenv = Qnil').
        let saved_lex = std::mem::replace(&mut self.lexenv, None);
        let mark = self.specbind_depth();
        let bind_result = self.bind_lambda_walk(&arglist, &argv, fun, |i, s, v| i.specbind(s, v));
        let result = match bind_result {
            Ok(()) => {
                // `Fprogn' on the raw body: walk conses only; a dotted
                // or atom tail just ends the body (returns last/nil).
                let mut last = Value::Nil;
                let mut cur = body.clone();
                loop {
                    match cur.clone() {
                        Value::Cons(c) => {
                            let (form, next) = {
                                let b = c.borrow();
                                (b.car.clone(), b.cdr.clone())
                            };
                            match self.eval(&form) {
                                Ok(v) => last = v,
                                Err(e) => {
                                    self.lexenv = saved_lex;
                                    self.unbind_to(mark)?;
                                    return Err(e);
                                }
                            }
                            cur = next;
                        }
                        _ => break,
                    }
                }
                Ok(last)
            }
            Err(e) => Err(e),
        };
        self.lexenv = saved_lex;
        self.unbind_to(mark)?;
        result
    }

    /// GNU `funcall_lambda's binding loop over the RAW arglist:
    /// `&optional'/`&rest' are the only keywords; each element must be
    /// a bare symbol (`invalid-function FUN' otherwise — FUN is the
    /// closure for closures, the `(lambda ...)' cons for raw forms);
    /// a second `&optional', `&optional' after `&rest', a second
    /// `&rest', a bare trailing keyword, or a non-nil dotted tail all
    /// signal `invalid-function FUN'; too few required args signal
    /// `wna FUN N' mid-walk, too many after it.  `bind' is either
    /// specbind (dynamic) or frame binding (lexical — GNU binds EVERY
    /// param into the closure env, specials included).
    fn bind_lambda_walk(
        &mut self,
        arglist: &Value,
        argv: &[Value],
        fun: &Value,
        mut bind: impl FnMut(&mut Interp, SymId, Value) -> Result<(), Flow>,
    ) -> Result<(), Flow> {
        let inv = |i: &Interp| -> Flow { i.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]) };
        let wna = |i: &Interp, n: usize| -> Flow {
            i.signal_data(
                sym::WRONG_NUMBER_OF_ARGUMENTS,
                vec![fun.clone(), Value::Int(n as i128)],
            )
        };
        let opt_kw = sym::OPTIONAL;
        let rest_kw = sym::REST;
        let mut syms = arglist.clone();
        let mut arg_i = 0usize;
        let mut optional = false;
        let mut rest = false;
        let mut previous_rest = false;
        loop {
            let (elt, next) = match &syms {
                Value::Cons(c) => {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                }
                _ => break,
            };
            let s = match self.sym_id(&elt) {
                Some(s) => s,
                None => return Err(inv(self)),
            };
            if s == rest_kw {
                if rest || previous_rest {
                    return Err(inv(self));
                }
                rest = true;
                previous_rest = true;
            } else if s == opt_kw {
                if optional || rest || previous_rest {
                    return Err(inv(self));
                }
                optional = true;
            } else {
                let arg = if rest {
                    // GNU `Flist (nargs - i, &arg_vector[i])'.
                    let v = Value::list(argv[arg_i..].to_vec());
                    arg_i = argv.len();
                    v
                } else if arg_i < argv.len() {
                    let v = argv[arg_i].clone();
                    arg_i += 1;
                    v
                } else if !optional {
                    return Err(wna(self, argv.len()));
                } else {
                    Value::Nil
                };
                bind(self, s, arg)?;
                previous_rest = false;
            }
            syms = next;
        }
        if !syms.is_nil() || previous_rest {
            return Err(inv(self));
        }
        if arg_i < argv.len() {
            return Err(wna(self, argv.len()));
        }
        Ok(())
    }

    fn call_lambda_inner(&mut self, l: &Rc<Lambda>, argv: Argv, _shown: &Value) -> EvalResult {
        if let Some(items) = &l.bc_items {
            if let Some(r) = self.unidata_bc_dispatch(l, &argv) {
                return r;
            }
            // `#[ARGDESC ...]' shape → the byte-code interpreter (also
            // reached by `#[...]' literals read from `.elc' files).
            // GNU exec_byte_code reports `(MIN . NONREST)' in wna data
            // — byte-code twins keep `who' nil so exec emits the argdesc
            // pair; only compiled *non-bytecode* twins (native-subr
            // stand-ins) report the shown function like GNU's
            // `#<subr>' wna datum.
            if super::bytecode::is_byte_code(&items.borrow()) {
                // Named compiled twins are GNU nativecomp's subrs —
                // report `shown' (the lambda object from funcall, the
                // calling symbol from eval) as the wna datum.  Only
                // nameless `#[...]' literals keep `None' so exec emits
                // the `(MIN . NONREST)' pair.
                // Datum rule (nativecomp): a *named* twin reports the
                // `#<subr NAME>' object (funcall) / called symbol
                // (eval path already emitted); an anonymous `#[...]'
                // byte-code literal (macro expanders included) reports
                // its `(MIN . NONREST)' argdesc pair.  `who' carries
                // the *resolved* object so funcall shows `#<subr>'.
                let named = self.lambda_name(l).is_some() || (l.compiled && !l.is_bytecode());
                let funv = Value::Lambda(l.clone());
                let who = if named && self.macroexp_call_depth == 0 {
                    Some(&funv)
                } else {
                    None
                };
                return super::bytecode::exec(self, l, argv, who, named);
            }
        }
        // GNU's wna datum is `(MIN . NONREST)' for byte-code calls
        // (exec_byte_code) and the function object for `funcall_lambda'.
        // Under nativecomp, named dumped functions are native subrs:
        // their wna datum is the `#<subr NAME>' object (funcall) or the
        // calling symbol (eval) — so a *named* compiled twin takes that
        // arm regardless of its bytecode twin's argdesc; the
        // `(MIN . NONREST)' pair is only for anonymous `#[...]' literals.
        // GNU's wna datum is `(MIN . NONREST)' for byte-code calls
        // (exec_byte_code) and the function object for `funcall_lambda'.
        // Under nativecomp, named dumped functions are native subrs:
        // `funcall' shows the `#<subr NAME>' object — our compiled
        // Lambda prints that way (see print.rs) — while the eval path
        // already reported the calling symbol in check_argdesc.
        // The `(MIN . NONREST)' pair stays for anonymous `#[...]'
        // literals only.
        let fun = if self.lambda_name(l).is_some() || (l.compiled && !l.is_bytecode()) {
            Value::Lambda(l.clone())
        } else {
            match &l.bc_items {
                Some(items) if l.is_bytecode() => match items.borrow().first() {
                    Some(Value::Int(desc)) => {
                        let (req, max, _rest, _) =
                            super::bytecode::parse_argdesc(self, &Value::Int(*desc));
                        Value::cons(Value::Int(req as i128), Value::Int(max as i128))
                    }
                    _ => Value::Lambda(l.clone()),
                },
                _ => Value::Lambda(l.clone()),
            }
        };
        let arglist = l.arglist.clone().unwrap_or(Value::Nil);

        if l.env.is_some() {
            // Lexical closure: GNU's `funcall_lambda' pushes EVERY
            // `(param . arg)' pair onto the closure env — `defvar'd
            // specials, scoped-declared names, even nil/t — since
            // `internal-interpreter-environment' is not consulted here.
            let frame = Rc::new(LexFrame {
                vars: RefCell::new(Vec::new()),
                declared: RefCell::new(Vec::new()),
                parent: l.env.clone(),
            });
            let saved = std::mem::replace(&mut self.lexenv, Some(frame.clone()));
            let bind_result = self.bind_lambda_walk(&arglist, &argv, &fun, |_i, s, v| {
                lexframe_bind(&frame, s, v);
                Ok(())
            });
            let r = match bind_result {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved;
            r
        } else {
            // Dynamic: specbind each parameter, and make sure no lexical
            // env leaks in — dynamic functions can't see callers' lexvars.
            let saved_lex = std::mem::replace(&mut self.lexenv, None);
            let mark = self.specbind_depth();
            let bind_result =
                self.bind_lambda_walk(&arglist, &argv, &fun, |i, s, v| i.specbind(s, v));
            let result = match bind_result {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved_lex;
            self.unbind_to(mark)?;
            result
        }
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
        // Same raw-pointer walk as `eval_args': zero `Value' clones per
        // form, and the next cdr is read after the form runs (GNU's
        // `prog_ignore' order).
        let mut cur = forms as *const Value;
        loop {
            match unsafe { &*cur } {
                Value::Nil => return Ok(last),
                Value::Cons(c) => {
                    let cell = RefCell::as_ptr(c);
                    last = self.eval(unsafe { &(*cell).car })?;
                    cur = unsafe { &(*cell).cdr };
                }
                // GNU `prog_ignore' signals wrong-type-argument listp
                // with the non-list tail as data.
                _ => return Err(self.wrong_type_mut("listp", unsafe { &*cur })),
            }
        }
    }

    /// Expand a macro call in eval position.  GNU `eval_sub' specbinds
    /// `lexical-binding' around the expander call: `t' when
    /// `internal-interpreter-environment' is non-nil (lexical context),
    /// nil otherwise — so `named-let' & friends see whether the emitted
    /// code will run lexically.  Only the EVAL-driven path binds this;
    /// explicit `macroexpand'/`macroexpand-all' calls leave the
    /// variable alone (verified on GNU 31.1).
    pub fn eval_macro_expand(&mut self, mac: &Value, args: &Value) -> EvalResult {
        let mark = self.specbind_depth();
        let lb = self.intern("lexical-binding");
        let lex_val = if self.lexenv.is_some() {
            Value::Sym(sym::T)
        } else {
            Value::Nil
        };
        if let Err(e) = self.specbind(lb, lex_val) {
            return Err(e);
        }
        let r = self.macro_expand_call(mac, args);
        self.unbind_to(mark)?;
        r
    }

    /// Expand a macro call: call the macro's function on raw args.
    pub fn macro_expand_call(&mut self, mac: &Value, args: &Value) -> EvalResult {
        if self.dbg_flags & 0x200000 != 0 {
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
        // GNU byte-compiles its bundled libraries, so macro expansion
        // of their call sites ran at byte-compile time — before any
        // runtime advice could be installed.  remacs interprets them
        // and re-expands lazily, so when the expansion happens inside
        // compiled-equivalent code (a `compiled'/dumped lambda or a
        // bundled load), run the expander with advice stripped from
        // symbol resolution: otherwise advice added earlier in the
        // session (e.g. (advice-add 'car ...)) corrupts expansions
        // whose expanders call the advised name — `letrec''s
        // (mapcar #'car binders) producing ((ad done) ...) binders.
        let unadvise = self.dumped_call_depth > 0 || self.loading_dumped;
        if unadvise {
            self.expand_unadvised_depth += 1;
        }
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) if l.is_macro => self.call_lambda(l, argv.into(), mac),
            Value::Lambda(l) => self.call_lambda(l, argv.into(), mac),
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
                    if unadvise {
                        self.expand_unadvised_depth -= 1;
                    }
                    self.macroexp_call_depth -= 1;
                    return self.macro_expand_call(&cdr, args);
                }
                self.apply(mac, argv).map_err(|f| self.macroexp_wna(mac, f))
            }
            _ => self.apply(mac, argv).map_err(|f| self.macroexp_wna(mac, f)),
        };
        if unadvise {
            self.expand_unadvised_depth -= 1;
        }
        self.macroexp_call_depth -= 1;
        result
    }

    /// `macro_expand_call' wna datum: GNU's expander is a byte-code
    /// object, so `exec_byte_code' reports `(MIN . NONREST)' — not the
    /// function.  remacs stores dumped expanders as `Subr's; translate
    /// a `wrong-number-of-arguments' from the expander call to the same
    /// cons datum (`when' expands to ((1 . 1) 0) like GNU's 0x181
    /// argdesc — `Arity::Many' has no nonrest, so min serves).
    fn macroexp_wna(&mut self, mac: &Value, f: Flow) -> Flow {
        if let Flow::Signal(sig, data, _) = &f {
            if matches!(sig, Value::Sym(s) if *s == sym::WRONG_NUMBER_OF_ARGUMENTS) {
                if let Some((min, max)) = match mac {
                    Value::Subr(s) => match s.arity {
                        Arity::Range { min, max } => Some((min, max)),
                        Arity::Many { min } => Some((min, min)),
                        Arity::Unevalled => None,
                    },
                    // `#[ARGDESC ...]' byte-code expander (`.elc' twin):
                    // GNU exec_byte_code reports `(MIN . NONREST)' —
                    // `when' → ((1 . 1) 0) from argdesc 385.
                    Value::Lambda(l) => match &l.bc_items {
                        Some(items) if super::bytecode::is_byte_code(&items.borrow()) => {
                            let items = items.borrow();
                            match items.first() {
                                Some(argdesc) => {
                                    let (req, max, _rest, _) =
                                        super::bytecode::parse_argdesc(self, argdesc);
                                    Some((req as u16, max as u16))
                                }
                                None => None,
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                } {
                    let n = data
                        .list_to_vec()
                        .ok()
                        .and_then(|v| v.get(1).cloned())
                        .unwrap_or(Value::Int(0));
                    return self.signal_data(
                        sym::WRONG_NUMBER_OF_ARGUMENTS,
                        vec![
                            Value::cons(Value::Int(min as i128), Value::Int(max as i128)),
                            n,
                        ],
                    );
                }
            }
        }
        f
    }

    /// `macroexpand`: repeatedly expand while the form is a macro call.
    pub fn macroexpand(&mut self, form: &Value) -> EvalResult {
        self.macroexpand_env(form, &Value::Nil)
    }

    /// GNU `macroexpand' (eval.c): ENV is an explicit alist of local
    /// macro definitions shadowing the global ones.  Unlike
    /// `macroexpand-all', it does not consult the dynamically bound
    /// `macroexpand-all-environment' variable.
    pub fn macroexpand_env(&mut self, form: &Value, env: &Value) -> EvalResult {
        let mut cur = form.clone();
        let mut iters = 0;
        loop {
            iters += 1;
            if self.dbg_flags & 0x200000 != 0 && iters > 500 {
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
                    // GNU `macroexpand-1' (eval.c): a `(lambda ...)'
                    // form is a function expression whose expansion is
                    // `(function (lambda ...))'.  Since that result is a
                    // fixpoint for `macroexpand', return it directly.
                    if self.sym_is(&car, sym::LAMBDA) {
                        return Ok(Value::list(vec![Value::Sym(sym::FUNCTION), cur.clone()]));
                    }
                    // Enabled `symbols-with-pos' cars expand through
                    // their bare symbol's function cell.
                    let car = crate::lisp::builtins::misc::unpos(&car);
                    match car {
                        Value::Sym(id) => {
                            // GNU `macroexpand-1' consults the ENVIRONMENT
                            // argument first.  An entry (SYM . DEF) shadows
                            // SYM's global definition: a nil DEF stops
                            // expansion, otherwise DEF is applied to the
                            // form's argument list (this is how GNU
                            // `cl-flet'/`rx-let' rewrite calls).
                            let mut env_hit = false;
                            let mut env_def = Value::Nil;
                            let mut tail = env.clone();
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
                                            // Enabled sympos keys unwrap to
                                            // their bare symbol id (GNU's
                                            // `assq' via `macroexpand-1'
                                            // compares with `eq').  `assq'
                                            // semantics: the FIRST match
                                            // wins — inner bindings are
                                            // consed in front of outer ones
                                            // (cl-flet shadowing a
                                            // cl-macrolet binding relies on
                                            // it).
                                            if self.sym_id(&ek) == Some(id) {
                                                env_hit = true;
                                                env_def = ev;
                                                break;
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

    /// Convert the cdr of a `(lambda CDR...)' form into a `Lambda`,
    /// mirroring GNU's `Ffunction'/`Fmake_interpreted_closure':
    ///
    /// - ARGLIST is `Fcar (cdr)' — a non-cons non-nil atom signals
    ///   `wrong-type-argument (listp ATOM)' (`(lambda a)' under
    ///   dynamic eval, `(function (lambda . 5))' alike).
    /// - A leading string is a docstring only when more forms follow
    ///   (else it's the sole body form); `(:documentation FORM)' is
    ///   evaluated at closure-construction time like GNU.
    /// - `(interactive ...)' — any cons starting `interactive' — is the
    ///   interactive spec; `(interactive . X)' signals listp X via
    ///   GNU's `CHECK_LIST (iform)'.
    /// - An empty body normalizes to `(nil)' ("make sure the body is
    ///   never empty").
    /// - With a lexical env GNU routes through `internal-make-closure'
    ///   (cconv.el): `cl-assert (consp body)' then `(listp args)',
    ///   yielding `cl-assertion-failed' errors; the dynamic
    ///   `Fmake_interpreted_closure' signals `wta consp'/`listp' on the
    ///   same operands.
    pub fn lambda_from_function_form(&mut self, cdr: &Value) -> Result<Lambda, Flow> {
        // `cdr' is FCDR of the `(lambda ...)' form — the spine check in
        // eval_form already ensured it's a proper list or atom.
        let arglist = match cdr {
            Value::Nil => Value::Nil,
            Value::Cons(c) => c.borrow().car.clone(),
            // GNU `Fcar' on an atom signals listp ATOM (the `5' of
            // `(lambda . 5)').
            other => return Err(self.wrong_type_mut("listp", other)),
        };
        let mut body = match cdr {
            Value::Nil | Value::Cons(_) => match cdr {
                Value::Nil => Value::Nil,
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => unreachable!(),
            },
            _ => Value::Nil,
        };
        let mut docstring = Value::Nil;
        let mut iform = Value::Nil;
        // GNU: a leading string is the docstring iff a body follows.
        if let Value::Cons(c) = &body {
            let (first, rest) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            match &first {
                Value::Str(s) => {
                    if !rest.is_nil() {
                        docstring = first.clone();
                        body = rest;
                    } else {
                        let _ = s;
                    }
                }
                Value::Cons(cc) => {
                    // `(:documentation FORM)': evaluate FORM now; nil or
                    // a non-consp cdr means it isn't a docstring at all.
                    let is_doc = {
                        let b = cc.borrow();
                        matches!(&b.car, Value::Sym(s)
                            if self.symbol_name(*s) == ":documentation")
                    };
                    if is_doc {
                        let form = {
                            let b = cc.borrow();
                            match &b.cdr {
                                Value::Cons(a) => a.borrow().car.clone(),
                                _ => Value::Nil,
                            }
                        };
                        let v = self.eval(&form)?;
                        if !v.is_nil() {
                            docstring = v;
                            body = rest;
                        }
                    }
                }
                _ => {}
            }
        }
        // `(interactive ...)' in the (post-docstring) front slot.
        if let Value::Cons(c) = &body {
            let (first, rest) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let is_interactive = match &first {
                Value::Cons(cc) => {
                    let b = cc.borrow();
                    self.sym_is(&b.car, sym::INTERACTIVE)
                }
                _ => false,
            };
            if is_interactive {
                iform = first.clone();
                body = rest;
            }
        }
        if body.is_nil() {
            body = Value::list(vec![Value::Nil]);
        }
        // The make-interpreted-closure checks.  GNU's dynamic path
        // (Fmake_interpreted_closure) uses CHECK_CONS/CHECK_LIST; the
        // lexical path goes through `internal-make-closure' whose
        // `cl-assert's produce `cl-assertion-failed' instead.
        if self.lexenv.is_some() {
            if !matches!(body, Value::Cons(_)) {
                let assert_id = self.intern("cl-assertion-failed");
                let consp_id = self.intern("consp");
                let body_id = self.intern("body");
                return Err(self.signal_data(
                    assert_id,
                    vec![Value::list(vec![Value::Sym(consp_id), Value::Sym(body_id)])],
                ));
            }
            let arglist_ok = matches!(arglist, Value::Cons(_) | Value::Nil);
            if !arglist_ok {
                let assert_id = self.intern("cl-assertion-failed");
                let listp_id = self.intern("listp");
                let args_id = self.intern("args");
                return Err(self.signal_data(
                    assert_id,
                    vec![Value::list(vec![Value::Sym(listp_id), Value::Sym(args_id)])],
                ));
            }
        } else {
            if !matches!(body, Value::Cons(_)) {
                return Err(self.wrong_type_mut("consp", &body));
            }
            if !matches!(arglist, Value::Cons(_) | Value::Nil) {
                return Err(self.wrong_type_mut("listp", &arglist));
            }
        }
        // CHECK_LIST (iform): a dotted `(interactive . X)' tails out.
        let mut tail = iform.clone();
        while let Value::Cons(c) = &tail {
            let next = c.borrow().cdr.clone();
            tail = next;
        }
        if !tail.is_nil() {
            return Err(self.wrong_type_mut("listp", &tail));
        }
        let body_vec = match body.list_to_vec() {
            Ok(v) => v,
            Err(crate::lisp::value::ListError::Dotted(t)) => {
                return Err(self.wrong_type_mut("listp", &t));
            }
            Err(_) => return Err(self.wrong_type_mut("listp", &body)),
        };
        let mut lambda = self.parse_lambda(&arglist, &body_vec, None)?;
        lambda.env = self.lambda_env();
        lambda.doc_value = if docstring.is_nil() {
            None
        } else {
            Some(docstring.clone())
        };
        lambda.doc = match &docstring {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => lambda.doc.clone(),
        };
        if !iform.is_nil() {
            lambda.interactive = Some(iform);
        }
        Ok(lambda)
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
        let mut bad_arglist = false;
        let mut rest = None;
        // GNU's `funcall_lambda' rest/optional state: `&rest' seen but
        // its parameter not yet bound (`rest_seen && rest.is_none()' =
        // GNU's `previous_rest').
        let mut rest_seen = false;
        let mut mode = 0u8; // 0 = required, 1 = optional, 2 = post-&rest
        // GNU's `funcall_lambda' only knows `&optional'/`&rest' — every
        // other `&'-prefixed name (`&body', `&key', `&whole', ...) is an
        // ordinary parameter symbol.  A second keyword, `&optional'
        // after `&rest', or a bare `&rest'/`&optional' at the tail makes
        // the arglist call-invalid rather than a parse error.
        let plist = params;
        let mut cur = plist.clone();
        loop {
            // Clone for the match: the `other' arm compares `cur' to
            // `plist' after the match scrutinee consumes the clone.
            match cur.clone() {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match self.sym_id(&car) {
                        Some(s) if s == sym::OPTIONAL => {
                            // GNU: `&optional' after `&rest' or a
                            // second `&optional' → call-invalid.
                            if mode != 0 {
                                bad_arglist = true;
                            }
                            mode = 1;
                        }
                        Some(s) if s == sym::REST => {
                            // GNU: a second `&rest' → call-invalid.
                            // `rest_seen' latches on the keyword itself
                            // so a bare trailing `&rest' is invalid.
                            if rest_seen {
                                bad_arglist = true;
                            }
                            rest_seen = true;
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
                                // GNU `funcall_lambda': after `&rest'
                                // consumed the arg list (i = nargs),
                                // further symbols bind nil; a non-symbol
                                // element is call-invalid.
                                if rest.is_none() {
                                    if let Some(s) = self.sym_id(&car) {
                                        rest = Some(s);
                                    } else {
                                        // `(&rest 5)': invalid at call.
                                        bad_arglist = true;
                                    }
                                } else if self.sym_id(&car).is_none() {
                                    // `(&rest a 5)': invalid at call.
                                    bad_arglist = true;
                                }
                                // Post-`&rest' symbols need no slot:
                                // GNU binds them nil at call time
                                // (`i = nargs' fast path), which the
                                // raw-arglist call walk reproduces.
                            }
                        },
                    }
                    cur = next;
                }
                // A dotted tail: GNU's `funcall_lambda' rejects ANY
                // non-nil tail at call time (`!NILP (syms_left)' →
                // invalid-function), symbol tails like `(a . b)'
                // included.  The closure still builds; the arglist
                // prints verbatim.
                Value::Sym(_) => {
                    bad_arglist = true;
                    break;
                }
                _ => {
                    // `cur == plist' only when the ARGLIST itself is
                    // this atom: internal-make-closure asserts
                    // `(listp args)' → `cl-assertion-failed'.
                    if crate::lisp::builtins::eq_values(&cur, plist) {
                        let assert_id = self.intern("cl-assertion-failed");
                        let listp_id = self.intern("listp");
                        let args_id = self.intern("args");
                        return Err(self.signal_data(
                            assert_id,
                            vec![Value::list(vec![Value::Sym(listp_id), Value::Sym(args_id)])],
                        ));
                    }
                    // Mid-list non-symbol tail `(a . 2)': call-invalid.
                    bad_arglist = true;
                    break;
                }
            }
        }
        // GNU's `previous_rest' end state: a `&rest' keyword with no
        // following parameter name is call-invalid.
        if rest_seen && rest.is_none() {
            bad_arglist = true;
        }

        // Extract docstring and interactive spec from the body front.
        let mut doc = None;
        let mut doc_value = None;
        let mut interactive = None;
        let mut start = 0;
        if let Some(Value::Str(s)) = body.first() {
            if body.len() > 1 {
                doc = Some(s.borrow().clone());
                doc_value = Some(body.first().unwrap().clone());
                start = 1;
            }
        } else if let Some(Value::Cons(c)) = body.first() {
            // GNU `Flambda' handles `(:documentation <form>)' as a
            // computed docstring: the form is evaluated at lambda
            // construction and consumed (eval.c).  The result is the
            // closure's docstring slot — any value (oclosure.el stores
            // its type symbol there).
            let is_doc_decl = {
                let b = c.borrow();
                matches!(&b.car, Value::Sym(s)
                    if self.symbol_name(*s) == ":documentation")
            };
            if is_doc_decl {
                let b = c.borrow();
                let form = match &b.cdr {
                    Value::Cons(args) => args.borrow().car.clone(),
                    _ => Value::Nil,
                };
                drop(b);
                let v = self.eval(&form)?;
                if let Value::Str(s) = &v {
                    doc = Some(s.borrow().clone());
                }
                doc_value = Some(v);
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
                // `(declare ...)' specs are a `defun'/`defmacro'
                // facility — `macroexp-parse-body' strips them there.
                // A bare `lambda' has no declaration handling (GNU
                // `Ffunction' keeps them in the body).
                if name.is_some()
                    && self.sym_is(&b.car, self.intern_soft("declare").unwrap_or(SymId::MAX))
                {
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
            compiled: self.loading_dumped || self.dumped_call_depth > 0,
            advice_link: None,
            bc_items: None,
            bc_closure: false,
            bc_prog: std::cell::RefCell::new(None),
            doc_value,
            env_value: None,
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
            &["overflow-error", "range-error", "arith-error", "error"],
        );
        put(
            self,
            "underflow-error",
            &["underflow-error", "range-error", "arith-error", "error"],
        );
        put(
            self,
            "singularity-error",
            &["singularity-error", "domain-error", "arith-error", "error"],
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
        put(
            self,
            "cyclic-function-indirection",
            &["cyclic-function-indirection", "error"],
        );
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
            "excessive-lisp-nesting",
            &["excessive-lisp-nesting", "recursion-error", "error"],
        );
        put(
            self,
            "excessive-variable-binding",
            &["excessive-variable-binding", "recursion-error", "error"],
        );
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
        put(
            self,
            "json-escape-sequence-error",
            &[
                "json-escape-sequence-error",
                "json-parse-error",
                "json-error",
                "error",
            ],
        );
        put(
            self,
            "json-trailing-content",
            &[
                "json-trailing-content",
                "json-parse-error",
                "json-error",
                "error",
            ],
        );
        put(
            self,
            "json-invalid-surrogate-error",
            &["json-invalid-surrogate-error", "json-error", "error"],
        );
        put(
            self,
            "json-number-out-of-range-error",
            &["json-number-out-of-range-error", "json-error", "error"],
        );
        put(
            self,
            "json-object-too-deep",
            &["json-object-too-deep", "json-error", "error"],
        );
        put(
            self,
            "json-out-of-memory",
            &["json-out-of-memory", "json-error", "error"],
        );
        put(
            self,
            "json-utf8-decode-error",
            &["json-utf8-decode-error", "json-error", "error"],
        );
        put(self, "mark-set", &["mark-set"]);
        put(self, "mark-active", &["mark-active"]);

        // Remaining GNU error symbols (fileio.c, keyboard.c, module.c,
        // comp.c, alloc.c, thread.c, xdisp.c, search.c, eval.c, ...).
        for (name, conds) in [
            (
                "cyclic-variable-indirection",
                &["cyclic-variable-indirection", "error"][..],
            ),
            (
                "file-date-error",
                &["file-date-error", "file-error", "error"],
            ),
            (
                "file-notify-error",
                &["file-notify-error", "file-error", "error"],
            ),
            (
                "remote-file-error",
                &["remote-file-error", "file-error", "error"],
            ),
            ("inhibited-interaction", &["inhibited-interaction", "error"]),
            ("invalid-arity", &["invalid-arity", "error"]),
            (
                "malformed-keyword-arg-list",
                &["malformed-keyword-arg-list", "error"],
            ),
            (
                "memory-buffer-too-small",
                &["memory-buffer-too-small", "error"],
            ),
            ("minibuffer-quit", &["minibuffer-quit", "quit"]),
            ("module-load-failed", &["module-load-failed", "error"]),
            (
                "missing-module-init-function",
                &[
                    "missing-module-init-function",
                    "module-load-failed",
                    "error",
                ],
            ),
            (
                "module-init-failed",
                &["module-init-failed", "module-load-failed", "error"],
            ),
            (
                "module-not-gpl-compatible",
                &["module-not-gpl-compatible", "module-load-failed", "error"],
            ),
            (
                "module-open-failed",
                &["module-open-failed", "module-load-failed", "error"],
            ),
            ("module-out-of-memory", &["module-out-of-memory", "error"]),
            ("native-compiler-error", &["native-compiler-error", "error"]),
            (
                "native-ice",
                &["native-ice", "native-compiler-error", "error"],
            ),
            (
                "native-lisp-load-failed",
                &["native-lisp-load-failed", "error"],
            ),
            (
                "native-lisp-file-inconsistent",
                &[
                    "native-lisp-file-inconsistent",
                    "native-lisp-load-failed",
                    "error",
                ],
            ),
            (
                "native-lisp-wrong-reloc",
                &[
                    "native-lisp-wrong-reloc",
                    "native-lisp-load-failed",
                    "error",
                ],
            ),
            (
                "wrong-register-subr-call",
                &[
                    "wrong-register-subr-call",
                    "native-lisp-load-failed",
                    "error",
                ],
            ),
            ("comp-sanitizer-error", &["comp-sanitizer-error", "error"]),
            ("protected-field", &["protected-field", "error"]),
            ("thread-buffer-killed", &["thread-buffer-killed", "error"]),
            ("trapping-constant", &["trapping-constant", "error"]),
            ("type-mismatch", &["type-mismatch", "error"]),
            (
                "user-search-failed",
                &["user-search-failed", "user-error", "search-failed", "error"],
            ),
            (
                "treesit-buffer_changed",
                &["treesit-buffer_changed", "treesit-error", "error"],
            ),
        ] {
            put(self, name, conds);
        }
        // `target-idx' properties from syms_of_coding (coding.c):
        // `find-operation-coding-system' picks its TARGET argument via
        // the operation symbol's `target-idx' plist entry.
        {
            let tidx = self.intern("target-idx");
            for (op, n) in [
                ("insert-file-contents", 0i128),
                ("write-region", 2),
                ("call-process", 0),
                ("call-process-region", 2),
                ("start-process", 2),
                ("open-network-stream", 3),
            ] {
                let s = self.intern(op);
                self.put_prop(s, tidx, Value::Int(n));
            }
        }
        // Tree-sitter errors (treesit.c): a `treesit-error' parent so
        // `ignore-errors'/`condition-case error' catches them — GNU's
        // `treesit-query-valid-p' relies on that to probe grammars.
        let em = self.intern("error-message");
        put(self, "treesit-error", &["treesit-error", "error"]);
        let te = self.intern("treesit-error");
        self.put_prop(te, em, Value::string("Generic tree-sitter error"));
        for (name, msg) in [
            ("treesit-query-error", "Query pattern is malformed"),
            ("treesit-parse-error", "Parse failed"),
            (
                "treesit-range-invalid",
                "RANGES are invalid: they have to be ordered and should not overlap",
            ),
            ("treesit-buffer-too-large", "Buffer too large (> 4GiB)"),
            (
                "treesit-load-language-error",
                "Cannot load language definition",
            ),
            (
                "treesit-node-outdated",
                "This node is outdated, please retrieve a new one",
            ),
            (
                "treesit-buffer-changed",
                "Buffer content changed, please don't edit buffer in predicate function, etc",
            ),
            (
                "treesit-node-buffer-killed",
                "The buffer associated with this node is killed",
            ),
            (
                "treesit-parser-deleted",
                "This parser is deleted and cannot be used",
            ),
            (
                "treesit-invalid-predicate",
                "Invalid predicate, see `treesit-thing-settings' for valid forms for a predicate",
            ),
        ] {
            put(self, name, &[name, "treesit-error", "error"]);
            let s = self.intern(name);
            self.put_prop(s, em, Value::string(msg));
        }
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
            (
                "cyclic-function-indirection",
                "Symbol's chain of function indirections contains a loop",
            ),
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
            ("permission-denied", "Cannot access file or directory"),
            ("file-locked", "File is locked"),
            ("cl-assertion-failed", "Assertion failed"),
            ("recursion-error", "Excessive recursive calling error"),
            (
                "excessive-lisp-nesting",
                "Lisp nesting exceeds `max-lisp-eval-depth'",
            ),
            (
                "excessive-variable-binding",
                "Variable binding depth exceeds max-specpdl-size",
            ),
            ("singularity-error", "Arithmetic singularity error"),
            ("unknown-image-type", "Cannot determine image type"),
            ("json-error", "generic JSON error"),
            ("json-parse-error", "could not parse JSON stream"),
            ("json-end-of-file", "end of JSON stream"),
            ("json-escape-sequence-error", "invalid escape sequence"),
            (
                "json-trailing-content",
                "trailing content after JSON stream",
            ),
            ("json-invalid-surrogate-error", "invalid surrogate pair"),
            ("json-number-out-of-range-error", "number out of range"),
            (
                "json-object-too-deep",
                "object cyclic or Lisp evaluation too deep",
            ),
            (
                "json-out-of-memory",
                "not enough memory for creating JSON object",
            ),
            ("json-utf8-decode-error", "invalid utf-8 encoding"),
            (
                "cyclic-variable-indirection",
                "Symbol's chain of variable indirections contains a loop",
            ),
            ("file-date-error", "Cannot set file date"),
            ("file-notify-error", "File notification error"),
            ("remote-file-error", "Remote file error"),
            ("inhibited-interaction", "User interaction while inhibited"),
            ("invalid-arity", "Invalid function arity"),
            (
                "malformed-keyword-arg-list",
                "Keyword lacks a corresponding value",
            ),
            ("memory-buffer-too-small", "Memory buffer too small"),
            ("minibuffer-quit", "Quit"),
            ("module-load-failed", "Module load failed"),
            (
                "missing-module-init-function",
                "Module does not export an initialization function",
            ),
            ("module-init-failed", "Module initialization failed"),
            ("module-not-gpl-compatible", "Module is not GPL compatible"),
            ("module-open-failed", "Module could not be opened"),
            ("module-out-of-memory", "Module out of memory"),
            ("native-compiler-error", "Native compiler error"),
            ("native-ice", "Internal native compiler error"),
            ("native-lisp-load-failed", "Native elisp load failed"),
            (
                "native-lisp-file-inconsistent",
                "eln file inconsistent with current runtime configuration, please recompile",
            ),
            (
                "native-lisp-wrong-reloc",
                "Primitive redefined or wrong relocation",
            ),
            (
                "wrong-register-subr-call",
                "comp--register-subr can only be called during native lisp load phase.",
            ),
            (
                "comp-sanitizer-error",
                "Native code sanitizer runtime error",
            ),
            ("protected-field", "Attempt to modify a protected field"),
            ("thread-buffer-killed", "Thread's current buffer killed"),
            (
                "trapping-constant",
                "Attempt to trap writes to a constant symbol",
            ),
            ("type-mismatch", "Types do not match"),
            ("user-search-failed", "Search failed"),
            ("sqlite-error", "Database error"),
            (
                "treesit-buffer_changed",
                "Buffer content changed, please don't edit buffer in predicate function, etc",
            ),
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
            // `features' is deliberately NOT special in GNU 31.1
            // (lexical `let' must not shadow what `featurep' sees;
            // only `dlet' dynamic bindings consult it).
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
            "configure-info-directory",
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
            "unread-command-events",
            "unread-input-method-events",
            "unread-post-input-method-events",
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
        // GNU simple.el defvars these to function names.
        for (name, val) in [
            (
                "filter-buffer-substring-function",
                "buffer-substring--filter",
            ),
            ("revert-buffer-function", "revert-buffer--default"),
            ("forward-sexp-function", "forward-sexp-default-function"),
            (
                "hack-read-symbol-shorthands-function",
                "hack-read-symbol-shorthands",
            ),
        ] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::Sym(self.intern(val));
        }
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
            // GNU marks `fontification-functions' local-if-set:
            // `local-variable-p' stays nil until set, but
            // `local-variable-if-set-p' is t, so `add-hook LOCAL' does
            // not prepend `t' to the fresh local value.
            "fontification-functions",
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
            // DEFVAR_PER_BUFFER: GNU reports `local-variable-p' t in
            // every buffer (e.g. `require-final-newline').
            "require-final-newline",
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
            (
                "noninteractive",
                if std::env::var_os("REMACS_NONINTERACTIVE").is_some() {
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
            // GNU defaults to t on case-insensitive filesystems
            // (darwin): minibuffer.el's defcustom probes the fs.
            ("read-file-name-completion-ignore-case", Value::Sym(sym::T)),
            ("history-delete-duplicates", Value::Nil),
            ("minibuffer-history", Value::Nil),
            ("buffer-name-history", Value::Nil),
            ("kill-ring", Value::Nil),
            ("values", Value::Nil),
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
                                .map(|p| Value::string(p.to_string_lossy().into_owned()))
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
            ("inhibit-point-motion-hooks", Value::Sym(sym::T)),
            ("inhibit-field-text-motion", Value::Nil),
            ("focus-in-hook", Value::Nil),
            ("focus-out-hook", Value::Nil),
            (
                "after-focus-change-function",
                Value::Sym(self.intern("ignore")),
            ),
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
            ("recenter-redisplay", Value::Sym(self.intern("tty"))),
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
            ("input-method-function", Value::Sym(self.intern("list"))),
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
            // GNU: files.el's defcustom makes the default nil; the
            // *scratch* buffer-local value (t) is seeded at init.
            ("require-final-newline", Value::Nil),
            ("sort-numeric-base", Value::Int(10)),
            // GNU -Q leaves `sort-fold-case' unbound: its defcustom
            // lives in sort.el, which is not in the dump.
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
            ("overflow-newline-into-fringe", Value::Sym(sym::T)),
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
            // GNU fill.el: `adaptive-fill-function' defaults to `ignore'.
            ("adaptive-fill-function", Value::Sym(self.intern("ignore"))),
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
            // GNU subr.el defvar: defaults to the two warning helpers.
            (
                "delayed-warnings-hook",
                Value::list(vec![
                    Value::Sym(self.intern("collapse-delayed-warnings")),
                    Value::Sym(self.intern("display-delayed-warnings")),
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
                Value::Sym(self.intern("completions-highlight")),
            ),
            // GNU's pre-redisplay-functions -Q value (window.el installs
            // this hook function in its dump).
            (
                "pre-redisplay-functions",
                Value::list(vec![Value::Sym(
                    self.intern("redisplay--update-region-highlight"),
                )]),
            ),
            ("load-prefer-newer", Value::Nil),
            ("load-read-function", Value::Sym(self.intern("read"))),
            ("obarray-size", Value::Int(15121)),
            ("max-image-size", Value::float(10.0)),
            ("image-scaling-factor", Value::Sym(self.intern("auto"))),
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
            ("emacs-repository-branch", Value::string("master")),
            ("site-run-file", Value::Nil),
            ("installation-directory", Value::Nil),
            ("invocation-name", Value::string("remacs")),
            ("invocation-directory", Value::Nil),
            (
                "source-directory",
                Value::string(env!("CARGO_MANIFEST_DIR")),
            ),
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
            (
                "input-method-verbose-flag",
                Value::Sym(self.intern("default")),
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
                    Value::Sym(self.intern("utf-8-unix")),
                    Value::Sym(self.intern("utf-8-unix")),
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
                Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
                    Value::Nil;
                    256
                ]))),
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
            (
                "ad-default-compilation-action",
                Value::Sym(self.intern("maybe")),
            ),
            ("read-symbol-positions-list", Value::Nil),
            ("eval-expression-print-maximum-character", Value::Int(127)),
            ("print-integers-as-characters", Value::Nil),
            ("print-escape-control-characters", Value::Nil),
            (
                "print-charset-text-property",
                Value::Sym(self.intern("default")),
            ),
            ("print-unreadable-function", Value::Nil),
            ("eval-depth", Value::Int(0)),
            (
                // GNU's loadup sets this to
                // `#'cconv-make-interpreted-closure' once cconv is
                // dumped (lisp/loadup.el:387); mirror that state.
                "internal-make-interpreted-closure-function",
                Value::Sym(self.intern("cconv-make-interpreted-closure")),
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
                    find_emacs_on_path(&|exe| doc_dir_of_emacs(exe, 0))
                        .unwrap_or_else(|| "/usr/share/emacs/".into()),
                ),
            ),
            (
                "configure-info-directory",
                // GNU: PATH_INFO from configure (`<prefix>/share/info').
                Value::string(
                    find_emacs_on_path(&|exe| info_dir_of_emacs(exe, 0))
                        .unwrap_or_else(|| "/usr/local/share/info/".into()),
                ),
            ),
            (
                "initial-major-mode",
                Value::Sym(self.intern("lisp-interaction-mode")),
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
                    Value::Sym(self.intern("read-only")),
                    Value::Sym(sym::T),
                    Value::Sym(self.intern("face")),
                    Value::Sym(self.intern("minibuffer-prompt")),
                ]),
            ),
            (
                "completion-styles",
                Value::list(vec![Value::Sym(self.intern("basic"))]),
            ),
        ];
        for (name, val) in defs {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = val.clone();
        }

        // GNU binds `main-thread' to the main thread object at -Q
        // (threads.c); our thread table's slot 0 is that object.
        {
            let id = self.intern("main-thread");
            self.obarray.symbol_mut(id).value =
                Value::Thread(self.threads[self.current_thread].clone());
        }
        // GNU binds `keyboard-translate-table' to a fresh char-table in
        // keyboard.c's syms_of_keyboard; it starts nil.
        {
            let id = self.intern("keyboard-translate-table");
            let ktt =
                crate::lisp::builtins::seq::make_char_table_raw(self, Value::Sym(id), Value::Nil);
            self.obarray.symbol_mut(id).value = ktt;
        }

        // GNU seeds auto-coding tables at dump (files.el/mule.el).
        // Quoted literals — identical to a -Q build's contents.
        let _ = self.eval_str(
            "(progn \
               (setq auto-coding-functions \
                     '(sgml-xml-auto-coding-function \
                       sgml-html-meta-auto-coding-function)) \
               (setq auto-coding-alist \
                     '((\"\\\\.\\\\(arc\\\\|zip\\\\|lzh\\\\|lha\\\\|zoo\\\\|[jew]ar\\\\|xpi\\\\|rar\\\\|7z\\\\|squashfs\\\\|ARC\\\\|ZIP\\\\|LZH\\\\|LHA\\\\|ZOO\\\\|[JEW]AR\\\\|XPI\\\\|RAR\\\\|7Z\\\\|SQUASHFS\\\\)\\\\'\" . no-conversion-multibyte) \
                       (\"\\\\.\\\\(exe\\\\|EXE\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(sx[dmicw]\\\\|odt\\\\|tar\\\\|t[bg]z\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(gz\\\\|Z\\\\|bz\\\\|bz2\\\\|xz\\\\|gpg\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(jpe?g\\\\|png\\\\|gif\\\\|tiff?\\\\|p[bpgn]m\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.pdf\\\\'\" . no-conversion) \
                       (\"/\\\\.editorconfig\\\\'\" . utf-8) \
                       (\"/#[^/]+#\\\\'\" . utf-8-emacs-unix))) \
               (setq auto-coding-regexp-alist \
                     '((\"\\\\`BABYL OPTIONS:[ \t]*-\\\\*-[ \t]*rmail[ \t]*-\\\\*-\" . no-conversion) \
                       (\"\\\\`\\376\\377\" . utf-16be-with-signature) \
                       (\"\\\\`\\377\\376\" . utf-16le-with-signature) \
                       (\"\\\\`\\357\\273\\277\" . utf-8-with-signature) \
                       (\"\\\\`;ELC\\024\\000\\000\\000\" . emacs-mule))))",
        );
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
        if self.buffers.get(id).is_none() {
            return;
        }
        // GNU set_buffer_internal_2 (Bug#18180): the undo list is
        // parked in the base buffer — an indirect buffer's record
        // moves back to its base when we switch away, and a switch
        // INTO an indirect buffer picks the base's list up.
        if let Some(old) = self.buffers.get(self.current_buffer) {
            let (base, list) = {
                let ob = old.borrow();
                (ob.base_buffer.clone(), ob.undo_list())
            };
            if let Some(base) = base {
                base.borrow_mut().set_undo_list(list);
            }
        }
        self.current_buffer = id;
        if let Some(new) = self.buffers.get(id) {
            let base = new.borrow().base_buffer.clone();
            if let Some(base) = base {
                let shared = base.borrow().undo_list();
                new.borrow_mut().set_undo_list(shared);
            }
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
        let r = match table {
            Value::Record(r) => r,
            _ => return Value::Nil,
        };
        let id = std::rc::Rc::as_ptr(r) as usize;
        self.char_table_parents
            .iter()
            .find(|(k, w, _)| *k == id && w.upgrade().is_some_and(|u| std::rc::Rc::ptr_eq(&u, r)))
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's parent (side-table; record layout untouched).
    pub fn set_char_table_parent(&mut self, table: &Value, parent: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_parents
                .retain(|(k, w, _)| *k != id && w.upgrade().is_some());
            if !parent.is_nil() {
                self.char_table_parents
                    .push((id, std::rc::Rc::downgrade(r), parent));
            }
        }
    }

    /// Char-table defalt accessor (record identity → defalt value).
    pub fn char_table_defalt(&self, table: &Value) -> Value {
        let r = match table {
            Value::Record(r) => r,
            _ => return Value::Nil,
        };
        let id = std::rc::Rc::as_ptr(r) as usize;
        self.char_table_defalts
            .iter()
            .find(|(k, w, _)| *k == id && w.upgrade().is_some_and(|u| std::rc::Rc::ptr_eq(&u, r)))
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's defalt (side-table; record layout untouched).
    pub fn set_char_table_defalt(&mut self, table: &Value, defalt: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_defalts
                .retain(|(k, w, _)| *k != id && w.upgrade().is_some());
            if !defalt.is_nil() {
                self.char_table_defalts
                    .push((id, std::rc::Rc::downgrade(r), defalt));
            }
        }
    }

    /// `char-code-property-alist': GNU's `charprop.el' registers each
    /// property with its data file name; the file lazily loads into a
    /// real char-table on first access (GNU's dump materializes twelve
    /// of them — our `characters.el' run does the same through
    /// `unicode-property-table-internal').
    pub fn seed_char_code_property_alist(&mut self) -> Value {
        // (prop . FILE) in GNU's dumped order.
        let specs: &[(&str, &str)] = &[
            ("bracket-type", "uni-brackets.el"),
            ("paired-bracket", "uni-brackets.el"),
            ("special-titlecase", "uni-special-titlecase.el"),
            ("special-lowercase", "uni-special-lowercase.el"),
            ("special-uppercase", "uni-special-uppercase.el"),
            ("titlecase", "uni-titlecase.el"),
            ("lowercase", "uni-lowercase.el"),
            ("uppercase", "uni-uppercase.el"),
            ("iso-10646-comment", "uni-comment.el"),
            ("old-name", "uni-old-name.el"),
            ("mirroring", "uni-mirrored.el"),
            ("mirrored", "uni-mirrored.el"),
            ("numeric-value", "uni-numeric.el"),
            ("digit-value", "uni-digit.el"),
            ("decimal-digit-value", "uni-decimal.el"),
            ("decomposition", "uni-decomposition.el"),
            ("bidi-class", "uni-bidi.el"),
            ("canonical-combining-class", "uni-combining.el"),
            ("general-category", "uni-category.el"),
            ("name", "uni-name.el"),
        ];
        let mut alist = Vec::with_capacity(specs.len());
        for &(prop, file) in specs {
            let psym = Value::Sym(self.intern(prop));
            let backing = Value::string(file);
            self.char_code_prop_tables
                .push((prop.to_string(), backing.clone()));
            alist.push(Value::cons(psym, backing));
        }
        Value::list(alist)
    }

    /// `standard-category-table': the shared category table.  Like
    /// GNU's temacs it starts empty — `international/characters.el'
    /// (loaded during init, loadup.el:215) fills in the category
    /// docstrings and per-char membership via `define-category' /
    /// `modify-category-entry'.
    pub fn standard_category_table(&mut self) -> Value {
        if let Some(v) = &self.standard_category_table {
            return v.clone();
        }
        let t = crate::lisp::builtins::misc::make_category_table_value(self, false);
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
                    bb.mark_position(),
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
                    let len = bb.text.len();
                    bb.set_mark(p.min(len));
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

    /// Rewrite printer byte-chars for a TEXT sink (buffer, marker,
    /// captured string): unibyte byte-chars print as `\NNN' octal
    /// escapes, matching GNU's printchar into text accumulators
    /// (whereas stdout gets overlong bytes and function streams get
    /// char codes).
    pub(crate) fn sink_text_mb(s: &str, mb: bool) -> std::borrow::Cow<'_, str> {
        use crate::lisp::value::{
            EIGHT_BIT_BASE, eight_bit_byte, print_eight_bit_byte, raw_byte_marker,
            write_char_eight_bit_byte,
        };
        if !s.chars().any(|c| {
            eight_bit_byte(c).is_some()
                || print_eight_bit_byte(c).is_some()
                || raw_byte_marker(c).is_some()
                || write_char_eight_bit_byte(c).is_some()
        }) {
            return std::borrow::Cow::Borrowed(s);
        }
        let mut out = String::with_capacity(s.len() + 8);
        for c in s.chars() {
            // write-char's sentinel stores the eight-bit CHARACTER
            // (GNU inserts the char, not `\NNN' text).
            if let Some(b) = write_char_eight_bit_byte(c) {
                if let Some(p) = char::from_u32(EIGHT_BIT_BASE + b as u32) {
                    out.push(p);
                }
                continue;
            }
            // print/prin1 byte-chars: `\NNN' escapes into multibyte
            // text; PRINC's plain `eight_bit_byte'/`raw_byte_marker'
            // chars stay as the eight-bit char itself (GNU princ →
            // the char, prin1 → the escape), so they don't double-
            // escape when the result is later printed.
            if let Some(b) = print_eight_bit_byte(c) {
                if mb {
                    let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\{:03o}", b));
                } else if let Some(p) = char::from_u32(EIGHT_BIT_BASE + b as u32) {
                    // Unibyte sink: downgrade the sentinel to the
                    // eight-bit char (0xF0080+b), not the sentinel.
                    out.push(p);
                }
                continue;
            }
            // PRINC emitted the byte-char itself; keep the char —
            // rewriting to `\NNN' text here double-escapes it when
            // the message string is printed again (GNU stores bytes).
            if let Some(_b) = eight_bit_byte(c).or_else(|| raw_byte_marker(c)) {
                out.push(c);
                continue;
            }
            out.push(c);
        }
        std::borrow::Cow::Owned(out)
    }

    /// `sink_text' for the always-multibyte accumulators
    /// (capture-output / echo area / string results).
    pub(crate) fn sink_text(s: &str) -> std::borrow::Cow<'_, str> {
        Self::sink_text_mb(s, true)
    }

    /// GNU printchar's function-stream argument: princ of a unibyte
    /// byte-char passes the raw byte, prin1 and write-char pass the
    /// eight-bit char code (0x3FFF00+byte, so byte 0x80 → 0x3FFF80);
    /// everything else is the char's own code.
    fn fn_stream_code(c: char) -> i128 {
        use crate::lisp::value::{
            eight_bit_byte, print_eight_bit_byte, raw_byte_marker, write_char_eight_bit_byte,
        };
        if let Some(b) = write_char_eight_bit_byte(c).or_else(|| print_eight_bit_byte(c)) {
            0x3FFF00 + b as i128
        } else if let Some(b) = eight_bit_byte(c).or_else(|| raw_byte_marker(c)) {
            b as i128
        } else {
            crate::lisp::value::lisp_char_code(c)
        }
    }

    /// Send printed output to STREAM (nil → `standard-output', t → the
    /// real output sink, buffer/marker → insert, function → call).
    pub fn write_output_to(&mut self, s: &str, stream: &Value) -> Result<(), Flow> {
        if let Some(c) = s.chars().last() {
            self.out_last_char = Some(c);
        }
        if self.capture_output {
            self.output_buffer.push_str(&Self::sink_text(s));
            return Ok(());
        }
        // `standard-output` may name a buffer, a marker, a function, or t.
        let dest = match stream {
            Value::Nil => self.symbol_value(self.standard_output_sym),
            v => v.clone(),
        };
        match &dest {
            Value::Buffer(b) => {
                let bb = b.borrow();
                let bid = bb.id;
                let ems = self.intern("enable-multibyte-characters");
                let mb = bb
                    .locals
                    .get(&ems)
                    .map(|v| v.truthy())
                    .unwrap_or_else(|| self.symbol_value(ems).truthy());
                drop(bb);
                let text = Self::sink_text_mb(s, mb);
                crate::buffer::primitives::chg_with_buffer(self, bid, |i| {
                    crate::buffer::primitives::chg_insert_pt(i, &text, false)
                })?;
            }
            Value::Marker(m) => {
                // GNU inserts before the marker, then advances it past
                // the inserted text (escapes are inserted as text).
                let mm = m.borrow();
                if let Some(buf_id) = mm.buffer {
                    let pos = mm.position;
                    drop(mm);
                    let ems = self.intern("enable-multibyte-characters");
                    let mb = self
                        .buffers
                        .get(buf_id)
                        .map(|b| {
                            let bb = b.borrow();
                            bb.locals
                                .get(&ems)
                                .map(|v| v.truthy())
                                .unwrap_or_else(|| self.symbol_value(ems).truthy())
                        })
                        .unwrap_or(true);
                    let text = Self::sink_text_mb(s, mb);
                    if self.buffers.get(buf_id).is_some() {
                        crate::buffer::primitives::chg_with_buffer(self, buf_id, |i| {
                            crate::buffer::primitives::chg_insert(i, pos, &text)
                        })?;
                    }
                    let newpos = pos + text.chars().count();
                    m.borrow_mut().position = newpos;
                }
            }
            // GNU calls a function print stream once per character;
            // unibyte byte-chars arrive as raw bytes (princ) or the
            // eight-bit char code (prin1), per fn_stream_code.
            Value::Lambda(_) | Value::Subr(_) => {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            Value::Cons(c)
                if {
                    let cb = c.borrow();
                    matches!(&cb.car, Value::Sym(s) if *s == self.intern("lambda") || *s == self.intern("closure"))
                } =>
            {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            Value::Sym(sid) if *sid != sym::T => {
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            // GNU `printcharfun': any non-nil/t, non-buffer, non-marker,
            // non-function value is treated as a FUNCTION name — ints,
            // strings, vectors, non-function lists, hash-tables all die
            // in funcall with (invalid-function VALUE).
            Value::Nil | Value::Sym(_) => {
                // nil or t: the real print destination — echo area
                // interactively, stdout in batch.
                match &self.output {
                    Some(OutputSink::Buffer(buf)) => buf.borrow_mut().push_str(&Self::sink_text(s)),
                    Some(OutputSink::Function(f)) => {
                        let f = f.clone();
                        let arg = Value::string(Self::sink_text(s).as_ref());
                        let _ = self.apply(&f, vec![arg]);
                    }
                    Some(OutputSink::Stdout) => {
                        // GNU's output encoding renders an eight-bit
                        // char (0x80-0xFF) in its internal two-byte
                        // form: 0xC0+((b>>6)&1), 0x80+(b&0x3F); raw-byte
                        // marker chars (mule codes >0x10FFFF) write
                        // their literal byte.
                        if s.chars().any(|c| {
                            crate::lisp::value::eight_bit_byte(c).is_some()
                                || crate::lisp::value::write_char_eight_bit_byte(c).is_some()
                                || crate::lisp::value::print_eight_bit_byte(c).is_some()
                                || crate::lisp::value::raw_byte_marker(c).is_some()
                                || crate::lisp::value::mule_seq_code(c).is_some()
                        }) {
                            let mut bytes = Vec::with_capacity(s.len());
                            for c in s.chars() {
                                if let Some(b) = crate::lisp::value::raw_byte_marker(c) {
                                    bytes.push(b);
                                } else if let Some(code) = crate::lisp::value::mule_seq_code(c) {
                                    bytes.extend_from_slice(&crate::lisp::value::mule_high_bytes(
                                        code,
                                    ));
                                } else {
                                    let b = crate::lisp::value::eight_bit_byte(c)
                                        .or_else(|| crate::lisp::value::print_eight_bit_byte(c))
                                        .or_else(|| {
                                            crate::lisp::value::write_char_eight_bit_byte(c)
                                        });
                                    match b {
                                        // every eight-bit char class
                                        // goes out as GNU's overlong
                                        // two-byte encoding
                                        Some(b) => {
                                            bytes.push(0xC0 + ((b >> 6) & 1));
                                            bytes.push(0x80 + (b & 0x3F));
                                        }
                                        None => {
                                            let mut tmp = [0u8; 4];
                                            bytes.extend_from_slice(
                                                c.encode_utf8(&mut tmp).as_bytes(),
                                            );
                                        }
                                    }
                                }
                            }
                            use std::io::Write;
                            let _ = std::io::stdout().write_all(&bytes);
                        } else {
                            print!("{}", s);
                        }
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        self.stderr_need_newline = true;
                    }
                    None => self.echo_message.push_str(&Self::sink_text(s)),
                }
            }
            _ => {
                // Non-nil/t, non-buffer/marker/function values are
                // function NAMES — apply yields the GNU errors
                // ((invalid-function 5) / ((invalid-function (1)) etc).
                for ch in s.chars() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
        }
        Ok(())
    }

    /// `message` — show a string in the echo area and log it to
    /// *Messages*.  GNU's `message_dolog' logs in batch too: the
    /// buffer is created on demand (undo disabled), consecutive
    /// duplicate lines collapse into `msg [N times]', and
    /// `message-log-max' bounds the line count from the top.
    /// Insertions bypass read-only/change-hook machinery (GNU's
    /// `insert_1_both' with PREPARE=false).
    pub fn message(&mut self, s: &str) {
        if self.noninteractive {
            if self.stderr_need_newline {
                eprintln!();
            }
            self.stderr_need_newline = false;
            eprintln!("{}", s);
        } else {
            self.echo_message = s.to_string();
        }
        self.message_log(s);
        // GNU routes `message' to stderr in batch and the echo area
        // interactively — never to the `princ'/stdout stream.
    }

    /// `message_dolog' body: append `s\n' to *Messages*, folding a run
    /// of identical messages into `msg [N times]'.  All positions are
    /// 0-based char indices.
    fn message_log(&mut self, s: &str) {
        let mb = match self.buffers.by_name("*Messages*") {
            Some(id) => id,
            None => {
                let id = self.buffers.create("*Messages*");
                // GNU message_dolog sets buffer-undo-list=t on a fresh
                // *Messages* (it "doesn't have undo").
                if let Some(b) = self.buffers.get(id) {
                    b.borrow_mut().set_undo_list(Value::t());
                }
                id
            }
        };
        let Some(b) = self.buffers.get(mb) else {
            return;
        };
        let mut bb = b.borrow_mut();
        let z = bb.text_len();
        // GNU appends `s\n' then message_log_check_duplicate compares
        // it with the previous line; equal lines fold.
        let prev_bol = {
            let mut i = z;
            if i > 0 && bb.text.char_at(i - 1) == '\n' {
                i -= 1;
            }
            while i > 0 && bb.text.char_at(i - 1) != '\n' {
                i -= 1;
            }
            i
        };
        if prev_bol < z {
            let mut eol = prev_bol;
            while eol < z && bb.text.char_at(eol) != '\n' {
                eol += 1;
            }
            let prev: String = (prev_bol..eol).map(|k| bb.text.char_at(k)).collect();
            // Strip a trailing " [N times]" suffix for the base text.
            let (base, count) = match prev.rfind(" [") {
                Some(pos) => {
                    let suf = &prev[pos + 2..];
                    let digits = suf.strip_suffix(" times]");
                    match digits {
                        Some(d) if !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()) => {
                            (prev[..pos].to_string(), d.parse::<usize>().unwrap_or(1))
                        }
                        _ => (prev.clone(), 1),
                    }
                }
                None => (prev.clone(), 1),
            };
            if base == s {
                let n = count + 1;
                // GNU del_range_both(prev_bol, this_bol): drop the old
                // line (incl. newline), then re-append the suffix.
                bb.delete_region(prev_bol, eol + 1);
                let z3 = bb.text_len();
                bb.raw_insert_at(z3, &format!("{}\n", s));
                let z2 = bb.text_len();
                bb.raw_insert_at(z2.saturating_sub(1), &format!(" [{} times]", n));
                let z4 = bb.text_len();
                bb.set_point(z4);
                drop(bb);
                self.trim_message_log(mb);
                return;
            }
        }
        let at_end = bb.point() >= z;
        bb.raw_insert_at(z, &format!("{}\n", s));
        if at_end {
            let z5 = bb.text_len();
            bb.set_point(z5);
        }
        drop(bb);
        self.trim_message_log(mb);
    }

    /// `message-log-max': keep at most that many trailing lines in
    /// *Messages* (GNU del_range_both(BEG, BOL-of-line N-before-end)).
    fn trim_message_log(&mut self, mb: usize) {
        let ml = self.intern("message-log-max");
        let Value::Int(maxl) = self.symbol_value(ml) else {
            return;
        };
        if maxl <= 0 {
            return;
        }
        let Some(b) = self.buffers.get(mb) else {
            return;
        };
        let mut bb = b.borrow_mut();
        let z = bb.text_len();
        let mut lines = 0usize;
        let mut cut = 0usize;
        for k in (0..z).rev() {
            if bb.text.char_at(k) == '\n' {
                lines += 1;
                if lines > maxl as usize {
                    cut = k + 1;
                    break;
                }
            }
        }
        if cut > 0 {
            bb.delete_region(0, cut);
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
        // GNU `get_minibuffer': `Vcommand_loop_level_list[depth]' ←
        // `command_loop_level' (remacs `recursion_depth').  Rewritten
        // on every read, not just when the buffer is created.
        while self.minibuf_cll.len() <= depth {
            self.minibuf_cll.push(0);
        }
        self.minibuf_cll[depth] = self.recursion_depth;
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
                    // A scripted `MinibufInput::Key' is dispatched
                    // through the minibuffer's real keymaps (the
                    // front-end's `minibuf_loop' behavior): remap
                    // bindings like y-or-n-p-map's `y-or-n-p-insert-*'
                    // run as commands instead of inserting raw text.
                    let mut keys: Vec<i128> = Vec::new();
                    let mut arg_mode = false;
                    let r = loop {
                        match self.minibuf_input(prompt, false) {
                            Ok(MinibufInput::Key(k)) => {
                                if let Err(e) = crate::term::minibuf_dispatch_key(
                                    self,
                                    k,
                                    &mut keys,
                                    &mut arg_mode,
                                ) {
                                    break Err(e);
                                }
                                if let Some(f) = self.minibuf_exit_fn.take() {
                                    break Ok(MinibufInput::Call(f));
                                }
                                if self.minibuf_exited {
                                    break Ok(MinibufInput::Text(self.minibuf_contents()));
                                }
                                if let Some(f) = self.minibuf_pending_flow.take() {
                                    break Err(f);
                                }
                                continue;
                            }
                            other => break other,
                        }
                    };
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

    /// `(error "MSG" DATA)' — GNU signal2(Qerror, build_string(MSG),
    /// DATA): the object travels as error data, NOT concatenated into
    /// the message string.
    pub fn error_obj(&self, msg: &str, v: &Value) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg), v.clone()])
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
        // GNU `command-execute' → `call-interactively' resolves the
        // symbol's cell through `indirect_function', which loads
        // autoload cells; applying the raw `(autoload ...)' cons would
        // hit apply's bare-cons `symbolp' check ("No file chosen" path
        // in dired's mouse command, M-x on autoloaded commands).
        // GNU command_execute: string/vector commands are keyboard
        // macros — `Fexecute_kbd_macro' on them (batch runs it too:
        // `(command-execute "a")' self-inserts `a' like GNU).
        if matches!(cmd, Value::Str(_) | Value::Vec(_)) {
            return crate::editor::f_execute_kbd_macro_pub(self, vec![cmd.clone()].into());
        }
        let fun = match cmd {
            Value::Sym(id) => {
                let mut f = self.symbol_function(*id);
                let auto_id = self.intern("autoload");
                let is_auto = matches!(&f, Value::Cons(c) if self.sym_is(&c.borrow().car, auto_id));
                if is_auto {
                    f = crate::lisp::builtins::evalfn::autoload_do_load(self, f, false)?;
                }
                f
            }
            other => other.clone(),
        };
        self.interactive_call_depth += 1;
        let res = (|| {
            // Interactive spec: GNU Fcall_interactively uses
            // `interactive-form' for every function kind — lambdas,
            // `#[...]' byte-code objects, oclosures (via
            // `oclosure-interactive-form') and subrs — so a single
            // `f_interactive_form' dispatch covers them all.
            let spec =
                crate::lisp::builtins::data::f_interactive_form(self, vec![fun.clone()].into())?;
            if !spec.is_nil() {
                let argv = self.eval_interactive_spec(&spec)?;
                return self.apply(&fun, argv);
            }
            self.apply(&fun, vec![])
        })();
        self.interactive_call_depth = self.interactive_call_depth.saturating_sub(1);
        res
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
                // GNU callint.c: each spec line's prompt text is
                // Fformat_message'd with the args collected so far —
                // BEFORE the control letter is dispatched.  A `%'
                // sequence with too few args signals
                // "Not enough arguments for format string" even when
                // the letter itself would be invalid or the prompt
                // text is unused ('r'/'d'/'p' &co).
                let take_fmt = |i: &mut Interp,
                                chars: &[char],
                                pos: &mut usize,
                                out: &[Value]|
                 -> Result<String, Flow> {
                    let prompt = take_prompt(chars, pos);
                    let mut argv = vec![Value::string(prompt)];
                    argv.extend_from_slice(out);
                    match crate::lisp::builtins::strfn::f_format_message(
                        i,
                        crate::lisp::Argv::from(argv),
                    )? {
                        Value::Str(fs) => Ok(fs.borrow().clone()),
                        _ => Ok(String::new()),
                    }
                };
                // GNU callint.c: leading `+' (reserved), `*'
                // (read-only barf), `-' (ignored), `@'/`^' (mouse
                // selection hooks — no-ops in batch) are stripped at
                // offset 0 only; elsewhere they fall through to the
                // invalid-letter error.
                while pos < chars.len() {
                    if pos == 0 {
                        match chars[pos] {
                            '+' => {
                                return Err(self.error(
                                    "\u{2018}+\u{2019} is not used in \u{2018}interactive\u{2019} for ordinary commands",
                                ))
                            }
                            '*' => {
                                crate::buffer::primitives::check_writable(self)?;
                                pos += 1;
                                continue;
                            }
                            '-' | '@' | '^' => {
                                pos += 1;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    let c = chars[pos];
                    pos += 1;
                    // GNU callint.c formats the prompt text BEFORE
                    // dispatching the letter — format errors (and the
                    // prompt's consumption) precede even an
                    // invalid-letter report.
                    let prompt = take_fmt(self, &chars, &mut pos, &out)?;
                    match c {
                        'r' => {
                            // GNU callint 'r' = region-beginning +
                            // region-end: no mark set → "The mark is
                            // not set now, so there is no region".
                            let (p, m) = self
                                .current_buffer_ref()
                                .map(|b| {
                                    let bb = b.borrow();
                                    (bb.point, bb.mark_position())
                                })
                                .unwrap_or((0, None));
                            let m = match m {
                                Some(m) => m,
                                None => {
                                    return Err(self
                                        .error("The mark is not set now, so there is no region"));
                                }
                            };
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
                                .and_then(|b| b.borrow().mark_position())
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
                        }
                        'n' | 'N' => {
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
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else {
                                let rs = self.intern("read-string");
                                out.push(self.apply(&Value::Sym(rs), vec![Value::string(prompt)])?);
                            }
                        }
                        'F' | 'f' | 'D' | 'G' => {
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
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if let Some(s) = self.spec_line(&prompt)? {
                                out.push(Value::Sym(self.intern(&s)));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'k' | 'K' => {
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
                        // GNU callint.c: the up-event a `k'/`K' read
                        // stored after a mouse down-event, else nil —
                        // our key reads never split down/up pairs.
                        'U' => out.push(Value::Nil),
                        _ => {
                            // GNU callint.c `default:': any unknown
                            // letter (including `+') signals —
                            // silently skipping it makes bogus specs
                            // like "~" return nil.
                            let letter = c as i128;
                            return Err(self.error_q(&format!(
                                "Invalid control letter \u{2018}{}\u{2019} (#o{:03o}, #x{:04x}) in interactive calling string",
                                c, letter, letter
                            ), &[]));
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

/// Deep-collect every interned symbol reachable inside a `load-history'
/// item (or any Lisp object): the pre-boot rollback restores by symbol
/// name, and side-effect forms (`put', `autoload', `defalias') never
/// produce history items, so the set must be broader than
/// `(defun . name)' pairs.  Cycles and shared tails are bounded by a
/// visited-cons counter.
fn collect_item_syms(_i: &Interp, v: &Value, out: &mut HashSet<SymId>) {
    let mut work = vec![v.clone()];
    let mut steps = 0usize;
    while let Some(x) = work.pop() {
        steps += 1;
        if steps > 200_000 {
            break;
        }
        match &x {
            Value::Sym(s) => {
                out.insert(*s);
            }
            Value::Cons(c) => {
                let b = c.borrow();
                work.push(b.car.clone());
                work.push(b.cdr.clone());
            }
            Value::Vec(vec) => {
                for e in vec.borrow().iter() {
                    work.push(e.clone());
                }
            }
            Value::Record(r) => {
                let mut k = 0;
                while let Some(e) = r.borrow().get(k) {
                    work.push(e.clone());
                    k += 1;
                }
            }
            _ => {}
        }
    }
}

/// Read every top-level form of an embedded Lisp source and collect the
/// symbols its evaluation could have mutated — the defined name of
/// `defun'/`defmacro'/`defvar'/`cl-defmethod'/... forms and the target
/// arguments of side-effect calls (`put', `autoload', `defalias',
/// `define-error', `make-obsolete*', `gv-define-*', `fset', `set',
/// `declare-function', `custom-declare-*').  Symbols merely *mentioned*
/// in code (docstring tails, loop variables, called functions) must NOT
/// be collected: the pre-boot rollback restores every collected symbol
/// to its pre-prelude state, which unbound `dolist'/`when' & co. when
/// the set was the union of every symbol in the file.
fn collect_source_syms(
    i: &mut Interp,
    src: &str,
    out: &mut HashSet<SymId>,
    prop_out: &mut HashSet<(SymId, SymId)>,
    defs_out: &mut HashSet<SymId>,
) {
    let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
    let mut pos = 0usize;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(pos);
            match reader.read() {
                Ok(f) => f.map(|v| (v, reader.position())),
                Err(_) => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                collect_form_targets(i, &form, out, prop_out, defs_out);
            }
            None => break,
        }
    }
}

/// Push the symbol interned by `(quote SYM)' or `#'SYM' into `out';
/// non-literal arguments (computed names) are ignored — they cannot be
/// restored by name anyway.
fn collect_quoted(i: &Interp, v: &Value, out: &mut HashSet<SymId>) {
    if let Value::Cons(c) = v {
        let b = c.borrow();
        let is_quote = matches!(
            &b.car,
            Value::Sym(s) if matches!(i.obarray.name(*s), "quote" | "function")
        );
        if is_quote {
            if let Value::Cons(t) = &b.cdr {
                if let Value::Sym(s) = t.borrow().car {
                    out.insert(s);
                }
            }
        }
    }
}

/// Push the literal symbol in `v' (a bare `Value::Sym') into `out'.
fn collect_sym_arg(v: &Value, out: &mut HashSet<SymId>) {
    if let Value::Sym(s) = v {
        out.insert(*s);
    }
}

/// Collect mutation targets from one top-level (or nested container)
/// form.
///
/// Three disjoint output sets:
///   * `out` — symbols whose whole state (function cell, value cell,
///     plist, binding flags) is restored from the pre-boot snapshot:
///     defined names plus non-defining cell writes (`fset', `autoload',
///     `define-obsolete-function-alias', ...).
///   * `prop_out` — `(symbol, prop-name)' pairs: literal `put'/
///     `function-put' targets and the known prop keys written by
///     `gv-define-*'/`make-obsolete*'/`define-error'/advertised-calling-
///     convention.  Only that plist key is removed, preserving props the
///     prelude/dump legitimately installed on the same symbol (unlike a
///     whole-plist restore, which voided `if' itself when gv.el's
///     `(put 'if 'gv-expander ...)' rolled back).
///   * `defs_out` — top-level defining heads only (defun/defmacro/
///     defvar/defconst/defcustom/defalias/defsubst).  A rolled file's
///     own definition must be erased even when the prelude folded the
///     same name (the snapshot then holds a real cell, not void): GNU
///     -Q binds it only via the dump or not at all.
fn collect_form_targets(
    i: &mut Interp,
    v: &Value,
    out: &mut HashSet<SymId>,
    prop_out: &mut HashSet<(SymId, SymId)>,
    defs_out: &mut HashSet<SymId>,
) {
    let mut work = vec![v.clone()];
    while let Some(x) = work.pop() {
        let items = match &x {
            Value::Cons(_) => {
                let mut v = Vec::new();
                let mut cur = x.clone();
                while let Value::Cons(cc) = cur {
                    let bb = cc.borrow();
                    v.push(bb.car.clone());
                    cur = bb.cdr.clone();
                }
                v
            }
            _ => continue,
        };
        if items.is_empty() {
            continue;
        }
        let head = match &items[0] {
            Value::Sym(s) => i.obarray.name(*s).to_string(),
            _ => {
                // Non-symbol head (e.g. `((lambda ...) args)'): scan
                // subforms for nested defining forms.
                for it in &items {
                    if let Value::Cons(_) = it {
                        work.push(it.clone());
                    }
                }
                continue;
            }
        };
        let h = head.as_str();
        // Defining forms: the name is the first argument.  This covers
        // `defun'/`defmacro'/`defvar'/`defcustom'/`defalias'-style heads
        // including `cl-defun'/`cl-defmethod' (name is items[1];
        // `(setf SYM)' method names yield the `(setf ...)' list, so
        // also collect the inner symbol) and `defstruct'/`defclass'
        // (whose accessor/constructor symbols aren't literal in the
        // form — collect the whole form deeply for those).
        let name_is_sym = matches!(&items.get(1), Some(Value::Sym(_)));
        let name_is_quote = matches!(&items.get(1), Some(Value::Cons(_)));
        if h.starts_with("def") || h.starts_with("define-") || h.starts_with("cl-def") {
            if h == "defstruct" || h == "defclass" || h == "cl-defstruct" {
                collect_item_syms(i, &x, out);
                if std::env::var_os("REMACS_TRACE_COLLECT").is_some() {
                    let nm = match items.get(1) {
                        Some(Value::Sym(s)) => i.obarray.name(*s).to_string(),
                        _ => "?".to_string(),
                    };
                    eprintln!(
                        "[defs {} <- {}]",
                        nm,
                        i.princ_to_string(&x).chars().take(80).collect::<String>()
                    );
                }
            } else if h == "defvar" || h == "defcustom" || h == "defconst" {
                // Rolled-file value defs force-void too: GNU never ran
                // the file, so `boundp' is nil at -Q (pcase--memoize,
                // killed-rectangle, the cl--loop-* caches, ...).  Names
                // the prelude itself defines (macroexp--dynvars, also
                // defvar'd by the rolled subr-x copy) are subtracted
                // from rolled_defs before the restore loop, so their
                // snapshot binding survives.
                if let Value::Sym(s) = items[1] {
                    out.insert(s);
                    defs_out.insert(s);
                }
            } else if name_is_sym {
                if let Value::Sym(s) = items[1] {
                    out.insert(s);
                    defs_out.insert(s);
                    if std::env::var_os("REMACS_TRACE_COLLECT").is_some() {
                        eprintln!(
                            "[defs {} <- {}]",
                            i.obarray.name(s),
                            i.princ_to_string(&x).chars().take(90).collect::<String>()
                        );
                    }
                }
            } else if name_is_quote {
                collect_quoted(i, &items[1], out);
                // `(setf SYM)' heads and `(defalias 'f (..))' — scan
                // the arg for a literal symbol too.
                if let Value::Cons(c) = &items[1] {
                    let b = c.borrow();
                    let mut inner = b.cdr.clone();
                    while let Value::Cons(t) = inner {
                        let tb = t.borrow();
                        collect_sym_arg(&tb.car, out);
                        inner = tb.cdr.clone();
                    }
                }
            }
            continue;
        }
        // Resolve the literal symbol arg shared by every side-effect
        // head below: `(head 'SYM ...)' / `(head SYM ...)'.
        let sym_arg = |at: usize| -> Option<SymId> {
            let a = items.get(at)?;
            if let Value::Sym(s) = a {
                return Some(*s);
            }
            if let Value::Cons(c) = a {
                let b = c.borrow();
                let quoted = matches!(
                    &b.car,
                    Value::Sym(s) if matches!(i.obarray.name(*s), "quote" | "function")
                );
                if quoted {
                    if let Value::Cons(t) = &b.cdr {
                        if let Value::Sym(s) = t.borrow().car {
                            return Some(s);
                        }
                    }
                }
            }
            None
        };
        match h {
            // (quote SYM) / #'SYM appearing as a *form*: irrelevant.
            "quote" | "function" | "provide" | "require" | "declare" | "declare-function" => {}
            // Literal `(put 'SYM 'KEY ...)'/`function-put': strip just
            // that prop key (a nil value is `remprop' — removing is
            // still correct).  A non-literal key forces a whole-plist
            // restore — encoded with a NIL key sentinel.
            "put" | "function-put" | "define-symbol-prop" => {
                if let Some(sym_id) = sym_arg(1) {
                    match sym_arg(2) {
                        Some(key_id) => {
                            prop_out.insert((sym_id, key_id));
                        }
                        None => {
                            prop_out.insert((sym_id, sym::NIL));
                        }
                    }
                }
            }
            // Known fixed prop keys for non-put heads.
            "make-obsolete" | "define-obsolete-function-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("byte-obsolete-info")));
                }
            }
            "make-obsolete-variable" | "define-obsolete-variable-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("byte-obsolete-variable")));
                }
            }
            "make-obsolete-face" | "define-obsolete-face-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("obsolete-face")));
                }
            }
            "make-obsolete-generalized-variable" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("byte-obsolete-generalized-variable")));
                }
            }
            // All three write the `gv-expander' prop.
            "gv-define-setter" | "gv-define-simple-setter" | "gv-define-expander" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("gv-expander")));
                }
            }
            "define-error" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("error-conditions")));
                    prop_out.insert((s, i.intern("error-message")));
                }
            }
            "set-advertised-calling-convention" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("advertised-calling-convention")));
                }
            }
            // `pcase-defmacro' writes NAME's `pcase-macroexpander'
            // prop — not a function cell (e.g. `(pcase-defmacro \`
            // (qpat) ...)' must not void the backquote macro).
            "pcase-defmacro" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.intern("pcase-macroexpander")));
                }
            }
            // Whole-plist overwrite: restore the snapshot plist.
            "setplist" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, sym::NIL));
                }
            }
            // Function/value-cell writes without a definition form:
            // `fset'/`autoload' leave an (autoload ...) cell; `set'
            // assigns the value cell.  The whole symbol still needs
            // restoring so the written cell goes back to its pre-load
            // state.
            "fset" | "set" | "defvaralias" | "autoload" => {
                if let Some(a) = items.get(1) {
                    collect_sym_arg(a, out);
                    collect_quoted(i, a, out);
                }
            }
            // Container/macro heads and everything else: recurse into
            // every element so nested definitions inside `progn'/
            // `eval-when-compile'/`unless' are still collected.  Bare
            // symbols drop out (collect_form_targets ignores non-conses).
            _ => {
                for it in &items {
                    if let Value::Cons(_) = it {
                        work.push(it.clone());
                    }
                }
            }
        }
    }
}

/// Return `(interactive SPEC)' for a primitive.  GNU primitive specs are
/// stored as strings; a leading `(' means the string is an expression to
/// read and evaluate rather than callint letter codes.
pub(crate) fn subr_interactive_form(i: &mut Interp, name: &str) -> Option<Value> {
    if let Some(f) = crate::lisp::builtins::comp::native_iform_by_name(name) {
        return Some(f);
    }
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
        ("search-emacs-glossary", "\u{1}nil"),
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
        (
            "append-to-buffer",
            "(list (read-buffer \"Append to buffer: \" (other-buffer (current-buffer) t)) (region-beginning) (region-end))",
        ),
        ("balance-windows", "\u{1}nil"),
        ("bury-buffer", "\u{1}nil"),
        (
            "capitalize-region",
            "(list (region-beginning) (region-end) (region-noncontiguous-p))",
        ),
        (
            "clone-buffer",
            "(progn (if buffer-file-name (error \"Cannot clone a file-visiting buffer\")) (if (get major-mode 'no-clone) (error \"Cannot clone a buffer in %s mode\" mode-name)) (list (if current-prefix-arg (read-buffer \"Name of new cloned buffer: \" (current-buffer))) t))",
        ),
        (
            "clone-indirect-buffer",
            "(progn (if (get major-mode 'no-clone-indirect) (error \"Cannot indirectly clone a buffer in %s mode\" mode-name)) (list (if current-prefix-arg (read-buffer \"Name of indirect buffer: \" (current-buffer))) t))",
        ),
        ("copy-region-as-kill", "(list (mark) (point) 'region)"),
        (
            "delete-directory",
            "(let* ((trashing (and delete-by-moving-to-trash (null current-prefix-arg))) (dir (expand-file-name (read-directory-name (if trashing \"Move directory to trash: \" \"Delete directory: \") default-directory default-directory nil nil)))) (list dir (if (directory-files dir nil directory-files-no-dot-files-regexp) (y-or-n-p (format-message \"Directory `%s' is not empty, really %s? \" dir (if trashing \"trash\" \"delete\"))) nil) (null current-prefix-arg)))",
        ),
        (
            "delete-file",
            "(list (read-file-name (if (and delete-by-moving-to-trash (null current-prefix-arg)) \"Move file to trash: \" \"Delete file: \") nil default-directory (confirm-nonexistent-file-or-buffer)) (null current-prefix-arg))",
        ),
        (
            "delete-indentation",
            "(progn (barf-if-buffer-read-only) (cons current-prefix-arg (and (use-region-p) (list (region-beginning) (region-end)))))",
        ),
        ("delete-minibuffer-contents", "\u{1}nil"),
        ("delete-process", "(list 'message)"),
        (
            "delete-trailing-whitespace",
            "(progn (barf-if-buffer-read-only) (if (use-region-p) (list (region-beginning) (region-end)) (list nil nil)))",
        ),
        ("delete-window", "\u{1}nil"),
        (
            "delete-windows-on",
            "(let ((frame (cond ((and (numberp current-prefix-arg) (zerop current-prefix-arg)) 0) (current-prefix-arg t)))) (list (read-buffer \"Delete windows on (buffer): \" nil nil (lambda (buf) (get-buffer-window (if (consp buf) (car buf) buf) (cond ((null frame) t) ((numberp frame) frame))))) frame))",
        ),
        (
            "display-buffer",
            "(list (read-buffer \"Display buffer: \" (other-buffer)) (if current-prefix-arg t))",
        ),
        (
            "downcase-region",
            "(list (region-beginning) (region-end) (region-noncontiguous-p))",
        ),
        ("exit-minibuffer", "\u{1}nil"),
        (
            "find-file",
            "(find-file-read-args \"Find file: \" (confirm-nonexistent-file-or-buffer))",
        ),
        ("fit-window-to-buffer", "\u{1}nil"),
        ("fundamental-mode", "\u{1}nil"),
        (
            "global-set-key",
            "(let* ((menu-prompting nil) (key (read-key-sequence \"Set key globally: \" nil t))) (list key (read-command (format \"Set key %s to command: \" (key-description key)))))",
        ),
        (
            "goto-char",
            "(goto-char--read-natnum-interactive \"Go to char: \")",
        ),
        ("ignore", "\u{1}nil"),
        (
            "insert-char",
            "(list (read-char-by-name \"Insert character (Unicode name or hex): \") (prefix-numeric-value current-prefix-arg) t)",
        ),
        (
            "invert-face",
            "(list (read-face-name \"Invert face\" (face-at-point t)))",
        ),
        (
            "join-line",
            "(progn (barf-if-buffer-read-only) (cons current-prefix-arg (and (use-region-p) (list (region-beginning) (region-end)))))",
        ),
        ("kill-buffer-and-window", "\u{1}nil"),
        (
            "kill-process",
            "(list (read-process-name \"Kill process\"))",
        ),
        ("kill-ring-save", "(list (mark) (point) 'region)"),
        ("list-processes", "\u{1}nil"),
        (
            "load-file",
            "(list (let ((completion-ignored-extensions (remove module-file-suffix (remove \".elc\" completion-ignored-extensions)))) (read-file-name \"Load file: \" nil nil 'lambda)))",
        ),
        ("locate-library", "(list (read-library-name) nil nil t)"),
        (
            "lossage-size",
            "(list (read-number \"Set maximum keystrokes to: \" (lossage-size)))",
        ),
        (
            "make-directory",
            "(list (read-file-name \"Make directory: \" default-directory default-directory nil nil) t)",
        ),
        (
            "make-face",
            "(list (read-from-minibuffer \"Make face: \" nil nil t 'face-name-history))",
        ),
        ("make-frame", "\u{1}nil"),
        ("maximize-window", "\u{1}nil"),
        ("minimize-window", "\u{1}nil"),
        (
            "remove-hook",
            "(let* ((default (and (symbolp (variable-at-point)) (symbol-name (variable-at-point)))) (hook (intern (completing-read (format-prompt \"Hook variable\" default) obarray #'boundp t nil nil default))) (local (and (local-variable-p hook) (symbol-value hook) (or (not (default-value hook)) (y-or-n-p (format \"%s has a buffer-local binding, use that? \" hook))))) (fn-alist (mapcar (lambda (x) (cons (with-output-to-string (prin1 x)) x)) (if local (symbol-value hook) (default-value hook)))) (function (alist-get (completing-read (format \"%s hook to remove: \" (if local \"Buffer-local\" \"Global\")) fn-alist nil t nil 'set-variable-value-history) fn-alist nil nil #'string=))) (list hook function local))",
        ),
        (
            "rename-buffer",
            "(list (read-string \"Rename buffer (to new name): \" nil 'buffer-name-history (buffer-name (current-buffer))) current-prefix-arg)",
        ),
        (
            "replace-region-contents",
            "(list (if (use-region-p) (region-beginning) (point-min)) (if (use-region-p) (region-end) (point-max)) (get-buffer (read-buffer-to-switch \"Source buffer: \")))",
        ),
        ("self-insert-and-exit", "\u{1}nil"),
        (
            "set-face-background",
            "(read-face-and-attribute :background)",
        ),
        (
            "set-face-extend",
            "(let ((list (read-face-and-attribute :extend))) (list (car list) (if (cadr list) t)))",
        ),
        (
            "set-file-modes",
            "(let ((file (read-file-name \"File: \"))) (list file (read-file-modes nil file)))",
        ),
        (
            "set-frame-font",
            "(let* ((completion-ignore-case t) (default (frame-parameter nil 'font)) (font (completing-read (format-prompt \"Font name\" default) (x-list-fonts \"*\" nil (selected-frame)) nil nil nil nil default))) (list font current-prefix-arg nil))",
        ),
        (
            "set-frame-height",
            "(set-frame-property--interactive \"Frame height: \" (frame-height))",
        ),
        (
            "set-frame-width",
            "(set-frame-property--interactive \"Frame width: \" (frame-width))",
        ),
        ("shrink-window-if-larger-than-buffer", "\u{1}nil"),
        (
            "signal-process",
            "(list (read-string \"Process (name or number): \") (read-signal-name))",
        ),
        (
            "split-window-below",
            "(list (when current-prefix-arg (prefix-numeric-value current-prefix-arg)) (selected-window))",
        ),
        (
            "split-window-horizontally",
            "(list (when current-prefix-arg (prefix-numeric-value current-prefix-arg)) (selected-window))",
        ),
        (
            "split-window-right",
            "(list (when current-prefix-arg (prefix-numeric-value current-prefix-arg)) (selected-window))",
        ),
        (
            "split-window-vertically",
            "(list (when current-prefix-arg (prefix-numeric-value current-prefix-arg)) (selected-window))",
        ),
        (
            "switch-to-buffer-other-frame",
            "(list (read-buffer-to-switch \"Switch to buffer in other frame: \"))",
        ),
        (
            "switch-to-buffer-other-window",
            "(list (read-buffer-to-switch \"Switch to buffer in other window: \"))",
        ),
        (
            "transpose-regions",
            "(if (< (length mark-ring) 2) (error \"Other region must be marked before transposing two regions\") (let* ((num (if current-prefix-arg (prefix-numeric-value current-prefix-arg) 0)) (ring-length (length mark-ring)) (eltnum (mod num ring-length)) (eltnum2 (mod (1+ num) ring-length))) (list (point) (mark) (elt mark-ring eltnum) (elt mark-ring eltnum2))))",
        ),
        ("unbury-buffer", "\u{1}nil"),
        (
            "upcase-initials-region",
            "(list (region-beginning) (region-end) (region-noncontiguous-p))",
        ),
        (
            "upcase-region",
            "(list (region-beginning) (region-end) (region-noncontiguous-p))",
        ),
        ("what-line", "\u{1}nil"),
        (
            "write-file",
            "(list (if buffer-file-name (read-file-name \"Write file: \" nil nil nil nil) (read-file-name \"Write file: \" default-directory (expand-file-name (file-name-nondirectory (buffer-name)) default-directory) nil nil)) (not current-prefix-arg))",
        ),
        (
            "zap-to-char",
            "(list (prefix-numeric-value current-prefix-arg) (read-char-from-minibuffer \"Zap to char: \" nil 'read-char-history) t)",
        ),
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
    /// Buffer the match ran in (for `match-data' markers); None for
    /// string searches or data installed via `set-match-data'.
    pub buffer: Option<usize>,
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

/// `get`/`put` accept ANY Lisp object as PROPNAME (eq-compared) —
/// GNU's symbol plist isn't restricted to symbol keys.
pub fn plist_get_v(plist: &Value, prop: &Value) -> Value {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        return c2.borrow().car.clone();
                    }
                    return Value::Nil;
                }
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

/// `plist_lookup' with an arbitrary (eq-compared) key — overlay
/// plists aren't restricted to symbol keys either.
pub fn plist_lookup_v(plist: &Value, prop: &Value) -> Option<Value> {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        return Some(c2.borrow().car.clone());
                    }
                    return Some(Value::Nil);
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

pub fn plist_put_v(plist: &Value, prop: &Value, val: Value) -> Value {
    let mut cur = plist.clone();
    let mut last_pair_end: Option<super::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&car, prop) {
                    if let Value::Cons(c2) = &next {
                        c2.borrow_mut().car = val;
                        return plist.clone();
                    }
                    return plist.clone();
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
    match last_pair_end {
        Some(cell) => {
            cell.borrow_mut().cdr = Value::cons(prop.clone(), Value::cons(val, Value::Nil));
            plist.clone()
        }
        None => Value::cons(prop.clone(), Value::cons(val, Value::Nil)),
    }
}

/// `overlay-put`-style plist write: existing prop updates in place;
/// a NEW prop conses onto the FRONT (GNU Foverlay_put, buffer.c).
pub fn plist_put_front(plist: &Value, prop: &Value, val: Value) -> Value {
    let mut cur = plist.clone();
    while let Value::Cons(c) = cur {
        let (car, next) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if crate::lisp::builtins::eq_values(&car, prop) {
            if let Value::Cons(vc) = &next {
                vc.borrow_mut().car = val;
            }
            return plist.clone();
        }
        if let Value::Cons(pair) = next {
            cur = pair.borrow().cdr.clone();
        } else {
            break;
        }
    }
    Value::cons(prop.clone(), Value::cons(val, plist.clone()))
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
