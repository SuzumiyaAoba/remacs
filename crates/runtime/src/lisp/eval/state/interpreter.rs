//! Interpreter storage. Field layout and visibility are kept compatible;
//! lifecycle operations and scoped snapshots live in the parent state module.

use crate::lisp::eval::{
    LexEnv, MatchData, MinibufInput, OutputSink, SpecialRecordKind, bindings::SpecBind, threads,
};
use crate::lisp::{
    error::Flow,
    obarray::Obarray,
    value::{Argv, Lambda, SymId, Value},
};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::{cell::RefCell, rc::Rc};

/// The interpreter: all global Lisp state. Lives on the eval thread.
pub struct Interp {
    pub(in crate::lisp::eval) symbol_position_flag: std::cell::Cell<Option<SymId>>,
    pub obarray: Obarray,
    pub(in crate::lisp::eval) specbind: Vec<SpecBind>,
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
    // Overlay handles currently use vectors; keep their type by identity,
    // independently of buffer lifetime, attachment, or vector contents.
    pub(in crate::lisp::eval) overlay_handles: HashMap<usize, std::rc::Weak<RefCell<Vec<Value>>>>,
    pub(in crate::lisp::eval) overlay_cleanup_at: usize,
    /// Record-backed objects whose GNU type is not `record'. Keep this by
    /// object identity because ordinary records may use the same tag.
    pub(crate) special_records: Vec<(std::rc::Weak<RefCell<Vec<Value>>>, SpecialRecordKind)>,
    pub current_buffer: usize,
    /// Symbols provided via `provide` (the `features` variable).
    pub features: Vec<SymId>,
    /// Output destination for princ/print/message.
    pub output: Option<OutputSink>,
    /// Last thing shown in the echo area (tests inspect this).
    pub echo_message: crate::lisp::text::EmacsText,
    /// Active graphical tooltip requested by `x-show-tip'.
    pub gui_tip: Option<String>,
    /// Popup menu requested by `x-popup-menu' for the GUI frontend.
    pub gui_popup_menu: Option<(Value, i128, i128)>,
    /// Last pointer position in window-relative pixels during mouse dispatch.
    pub gui_mouse_position: Option<(i128, i128)>,
    /// Read position for `read` from a buffer/stream — kept simple.
    pub max_lisp_eval_depth: usize,
    pub eval_depth: usize,
    /// Depth of explicit `(eval ...)' calls — lambdas built inside get a
    /// `nil' environment marker like Emacs.
    pub explicit_eval_depth: usize,
    /// C-g sets this; eval checks it between forms.
    /// Incoming host request, synchronized into the Lisp quit-flag cell.
    pub quit_flag: bool,
    pub(crate) bc_quit_counter: Option<Rc<std::cell::Cell<u8>>>,
    pub(crate) unwind_quit: Option<Value>,
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
    /// GNU's private Vsignaling_function, passed to command-error-function.
    pub(crate) signaling_function: Value,
    /// Accumulator while `with-output-to-string` captures output.
    pub output_buffer: crate::lisp::text::EmacsText,
    /// When true, `write_output` appends to `output_buffer`.
    pub capture_output: bool,
    /// Registers from the last search (`match-data`).
    pub match_data: Option<MatchData>,
    /// A process filter or sentinel is running (GNU's running_asynch_code).
    pub(crate) running_asynch_code: bool,
    /// GNU search.c saves interrupted search registers lazily on first use.
    /// The outer Option distinguishes unsaved from saved empty match data.
    pub(crate) saved_async_match_data: Option<Option<MatchData>>,
    /// Pending `insert-behind-hooks' / `insert-in-front-hooks' lists
    /// collected by `verify_interval_modification' (inside
    /// `signal_before_change') and run by `report_interval_modification'
    /// after the insertion lands — GNU's `interval_insert_*_hooks'
    /// globals.
    pub pending_insert_behind: Vec<Value>,
    pub pending_insert_front: Vec<Value>,
    /// `(hooklist . overlay-handle)` pairs collected during the before-
    /// phase and replayed after — GNU's `last_overlay_modification_hooks'.
    pub pending_overlay_hooks: Vec<(Value, Value)>,
    /// `combine_after_change_list': deferred `(beg0, end0, delta)'
    /// triples while `combine-after-change-calls' is bound.
    pub combine_after_change_list: Vec<(usize, usize, i128)>,
    /// `combine_after_change_buffer': which buffer the deferred changes
    /// belong to.
    pub combine_after_change_buffer: Option<usize>,
    /// True in `--batch`: `princ` goes to stdout, `message` to stderr.
    pub noninteractive: bool,
    /// Monotonic process start used by `get-internal-run-time'.
    pub(crate) runtime_started: std::time::Instant,
    /// The `obarray' variable's record value; its Rc identity marks the
    /// default symbol table, so `intern'/`mapatoms'/completion over
    /// `obarray' route to `self.obarray' instead of a (stub) record vec.
    pub default_obarray: Option<crate::lisp::value::VecRef>,
    /// True while the startup prelude or an embedded (`builtin:') library
    /// is being evaluated — lambdas defined then get `dumped_doc' (their
    /// docstrings behave like GNU's .elc/DOC-file entries).
    pub loading_dumped: bool,
    /// GNU's `Vloads_in_progress': resolved path identity of every
    /// `load' currently evaluating (the `builtin:NAME' virtual path
    /// for embedded libraries, `found' for disk files), outermost
    /// last.  `autoload_do_load' compares its resolved target against
    /// this — GNU's "Recursive load" check and the recursion that
    /// would spin forever on self-autoloads.
    pub loads_in_progress: Vec<super::super::super::text::EmacsText>,
    pub(crate) load_stream: Option<Rc<RefCell<crate::lisp::load::LoadStream>>>,
    pub(crate) load_stream_symbol: Option<SymId>,
    /// Number of currently-active dumped definitions.  GNU byte-compiles
    /// its dump, so macros called there need not remain visible at -Q;
    /// remacs stashes those cells and resolves hidden macros only while
    /// a dumped lambda is running.
    pub dumped_call_depth: usize,
    /// Number of active macro expanders, used to distinguish compiler
    /// requirements from runtime library dependencies.
    pub macroexp_call_depth: usize,
    /// The currently executing Lisp function belongs to dumped code.
    /// Unlike dumped_call_depth, this does not include ancestor frames.
    pub current_lambda_dumped: bool,
    /// Byte-compiled bodies have control flow, but no source special-form frames.
    pub(crate) current_body_compiled: bool,
    /// Provenance of forms being inspected by macroexpand, independent
    /// of the dumped implementation of the macroexpansion machinery.
    pub compiled_macro_expansion: bool,
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
    /// The current library load stands in for a shipped compiled file.
    /// Explicit source loads and user overrides retain source semantics.
    pub loading_compiled_library: bool,
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
    pub out_last_char: Option<crate::lisp::text::EmacsChar>,
    /// The interpreter's time-zone rule, set by `set-time-zone-rule'
    /// (GNU's `environ'/"TZ" flip).  We keep it per-`Interp' instead of
    /// touching the process `TZ' so parallel interpreters (tests) see
    /// independent wall clocks — `None' means the ambient system zone.
    pub tz_rule: Option<String>,
    /// All frames (the first is the initial tty frame).
    pub frames: Vec<crate::editor::FrameRef>,
    /// Next unused automatic terminal frame name (the initial frame is F1).
    pub(crate) next_frame_name: u64,
    /// The selected frame.
    pub selected_frame: Option<crate::editor::FrameRef>,
    /// GNU window_select_count, including selections of deleted windows.
    pub(crate) window_select_count: u64,
    /// Set by `kill-emacs` to exit the command loop.
    pub quit_editor: bool,
    /// A completed shutdown request is irreversible. Shared by Lisp threads;
    /// Rust stack restoration must never execute more Lisp or replace it.
    pub(crate) exit_status: Option<i128>,
    /// Interactive input hook installed by the terminal front-end.
    /// Called as (interp, prompt, single_key) -> MinibufInput.
    /// `single_key` reads one event (y-or-n-p, read-char).
    pub minibuf_reader: Option<
        std::rc::Rc<
            dyn Fn(&mut Interp, &crate::lisp::text::EmacsStr, bool) -> Result<MinibufInput, Flow>,
        >,
    >,
    /// Host clipboard bridge, installed by a GUI front-end.
    pub(crate) clipboard_backend: Option<Rc<dyn crate::frontend::ClipboardBackend>>,
    /// NS selection values retained for local selection reads and ownership checks.
    pub(crate) nsgui_selections: HashMap<SymId, Value>,
    /// Keyboard.c keeps a deferred switch-frame outside the public queues.
    pub(crate) unread_switch_frame: Value,
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
    pub minibuf_prompts: Vec<crate::lisp::text::EmacsText>,
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
    pub face_table: Vec<(SymId, Value)>,
    /// GNU's "new-frame defaults" lfaces — attrs set via
    /// `(set-face-attribute FACE t ...)'; read by `face-attribute'/`
    /// internal-get-lisp-face-attribute' with FRAME=t.
    pub face_t_table: Vec<(SymId, Value)>,
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
    /// Exact selected category members for detection; the public priority
    /// list exposes base names instead (aliases retain distinct IDs in GNU).
    pub coding_detection_members: HashMap<String, String>,
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
    /// `handler-bind' dynamic chain: conditions, handler, and the active
    /// condition-case count at registration. Innermost binding is last.
    pub handler_bindings: Vec<(Value, Value, usize)>,
    /// Condition lists of active interpreted or byte-code condition-case
    /// bodies — a signal matching any of them is caught.
    pub(crate) case_handlers: Vec<crate::lisp::error::CaseHandler>,
    /// `add-variable-watcher' registry: (SYM . FUNCTION) pairs.
    pub var_watchers: Vec<(SymId, Value)>,
    /// All thread objects ever created (`all-threads'); element 0 is
    /// the main thread.
    pub threads: Vec<crate::lisp::value::ThreadRef>,
    /// Index into `threads` of the currently running thread.
    pub current_thread: usize,
    pub(crate) thread_scheduler: threads::Scheduler,
    pub(in crate::lisp::eval) thread_yielder: Option<std::ptr::NonNull<threads::ThreadYielder>>,
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
    /// Memory profiler running state; log data lives in `profiling`.
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
    /// Name-keyed mirror for dedicated byte-code opcodes.  These calls
    /// are tied to the canonical primitive even if Lisp uninterns its
    /// public symbol and a later `intern' creates a new symbol ID.
    pub byte_op_subrs_by_name: Option<HashMap<&'static str, &'static crate::lisp::value::Subr>>,
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
    /// Track runtime redefinitions, not artificial bootstrap cell shadows.
    pub(crate) definition_history_enabled: bool,
    pub(crate) native_trampolines: HashSet<String>,
    /// Constants belonging to Rust implementations of dumped Lisp functions.
    /// Keep their object identity and mutations, as native unit literals do.
    pub(crate) native_lisp_literals: HashMap<String, Value>,
    /// Native code's Q-symbol references survive removal from an obarray.
    pub(crate) fixed_symbols: HashMap<String, SymId>,
    pub(crate) pending_frames: Vec<crate::lisp::backtrace::PendingFrame>,
    /// Bound names for compiled lambdas that were `defalias'd while
    /// nameless (`#[...]' literals from `.elc' files).  Entries pair
    /// a `Weak' with the name: `Weak::upgrade' failing means the
    /// pointer was freed and reused, so stale names never reattach
    /// to a different lambda.
    pub compiled_fn_names:
        std::cell::RefCell<Vec<(std::rc::Weak<Lambda>, crate::lisp::text::EmacsText)>>,
    /// Debug: the raw form whose head symbol call is in progress.
    pub cur_head_form: Value,
    /// `lisp_stack' snapshot taken when the last signal was raised —
    /// the frames GNU's batch debugger prints after `Error:' (the live
    /// stack itself has already unwound by the time main sees it).
    pub last_error_stack: std::cell::RefCell<Vec<crate::lisp::backtrace::Frame>>,
    /// CPU-time profiler running state.
    pub cpu_profiler: bool,
    pub(crate) profiling: crate::lisp::profiler::State,
    pub(crate) loaded_dump: Option<crate::lisp::dump::LoadedDump>,
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
        HashMap<usize, std::rc::Weak<std::cell::RefCell<crate::lisp::text::EmacsText>>>,
    /// Strings produced by multibyte decoders — `multibyte-string-p'
    /// is t for them even when their contents are pure ASCII.
    pub multibyte_strings:
        HashMap<usize, std::rc::Weak<std::cell::RefCell<crate::lisp::text::EmacsText>>>,
    /// Destructive string-data changes; byte-code dispatch uses this to
    /// notice instruction edits made by Lisp during a running call.
    pub(crate) string_data_tick: u64,
    /// Text properties attached to string objects: pointer identity →
    /// (weak backref, sorted disjoint intervals (start, end, plist)).
    /// GNU keeps dead interval boundaries after property removal, so
    /// a true interval model is needed (not last-write-wins).  The
    /// Weak guards against address reuse like the unibyte map.
    pub string_props: HashMap<
        usize,
        (
            std::rc::Weak<std::cell::RefCell<crate::lisp::text::EmacsText>>,
            crate::lisp::memory::Intervals,
        ),
    >,
    /// GNU's `zero_vector': the single shared 0-length vector —
    /// `(vector)', `[]' and `(make-vector 0)' are all `eq' to it.
    pub zero_vector: Value,
    /// GNU shares empty strings by byte representation. Keep the canonical
    /// unibyte and multibyte values separate, like the zero-length vectors.
    pub zero_unibyte_string: Value,
    pub zero_multibyte_string: Value,
}
