//! Ordered interpreter construction and GNU dump compatibility.

use super::{Interp, MatchData, lexenv_root};

mod defaults;
mod history;
mod paths;
mod properties;
mod rollback;
mod snapshot;
mod symbols;
use snapshot::{SymbolSnapshot, SymbolState};

use symbols::{GNU_VOID_FNS, GNU_VOID_FNS_LATE, GNU_VOID_VARS};

/// State retained across library loading and delayed custom initialization.
#[derive(Default)]
struct BootRollback {
    symbols: Vec<(SymId, Value, Value, Value)>,
    properties: Vec<(SymId, SymId)>,
}
use crate::lisp::{
    obarray::{Obarray, sym},
    value::{SymId, Value},
};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::cell::RefCell;
use std::rc::Rc;

impl Interp {
    /// Storage used while the live runtime is owned by another coroutine.
    /// It is never evaluated; all live state travels through owned boxes.
    pub(crate) fn vacant() -> Self {
        Self::uninitialized(Obarray::empty(), 0, Vec::new(), false)
    }

    fn uninitialized(
        obarray: Obarray,
        standard_output_sym: SymId,
        feature_syms: Vec<SymId>,
        noninteractive: bool,
    ) -> Self {
        let mut interp = Interp {
            symbol_position_flag: std::cell::Cell::new(None),
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
            overlay_handles: HashMap::default(),
            overlay_cleanup_at: 256,
            special_records: Vec::new(),
            current_buffer: 0,
            features: feature_syms,
            output: None,
            echo_message: crate::lisp::text::EmacsText::new(),
            echo_message_props: Vec::new(),
            gui_tip: None,
            gui_popup_menu: None,
            gui_mouse_position: None,
            gui_mouse_absolute_position: None,
            gui_mouse_frame_position: None,
            gui_mouse_frame_pixel_position: None,
            gui_image_cache_size: std::cell::Cell::new(0),
            max_lisp_eval_depth: 1600,
            eval_depth: 0,
            explicit_eval_depth: 0,
            quit_flag: false,
            bc_quit_counter: None,
            unwind_quit: None,
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
            signaling_function: Value::Nil,
            output_buffer: crate::lisp::text::EmacsText::new(),
            capture_output: false,
            match_data: None,
            running_asynch_code: false,
            saved_async_match_data: None,
            pending_insert_behind: Vec::new(),
            pending_insert_front: Vec::new(),
            pending_overlay_hooks: Vec::new(),
            combine_after_change_list: Vec::new(),
            combine_after_change_buffer: None,
            // `main' argv-scans --batch/--script before `Interp::new'
            // (GNU does the same in its C main), so startup-time Lisp
            // already sees the batch state.
            noninteractive,
            runtime_started: std::time::Instant::now(),
            default_obarray: None,
            loads_in_progress: Vec::new(),
            load_stream: None,
            load_stream_symbol: None,
            loading_dumped: false,
            dumped_call_depth: 0,
            reading_elc: false,
            boot_loading: true,
            bc_depth: 0,
            macroexp_call_depth: 0,
            current_lambda_dumped: false,
            current_body_compiled: false,
            compiled_macro_expansion: false,
            expand_unadvised_depth: 0,
            loading_compiled_library: false,
            macro_cache: HashMap::default(),
            macro_gen: 0,
            macroexpand_all_cache: std::cell::RefCell::new((0, HashMap::default())),
            stderr_need_newline: false,
            out_last_char: None,
            tz_rule: None,
            frames: Vec::new(),
            next_frame_name: 2,
            selected_frame: None,
            window_select_count: 0,
            quit_editor: false,
            exit_status: None,
            minibuf_reader: None,
            clipboard_backend: None,
            bell_backend: None,
            about_panel_backend: None,
            dialog_backend: None,
            file_dialog_backend: None,
            font_selection_backend: None,
            frame_window_backend: None,
            color_panel_backend: None,
            drag_backend: None,
            application_visibility_backend: None,
            apple_script_backend: None,
            native_service_backend: None,
            nsgui_selections: HashMap::default(),
            unread_switch_frame: Value::Nil,
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
            coding_detection_members: HashMap::default(),
            terminal_coding: Value::Nil,
            keyboard_coding: Value::Nil,
            macro_replay: std::collections::VecDeque::new(),
            macro_replaying: false,
            macro_pending_flow: None,
            macro_strict: false,
            kbd_macro_events: Vec::new(),
            kbd_macro_mark: 0,
            command_key_events: Vec::new(),
            command_key_start: 0,
            command_raw_key_events: Vec::new(),
            recent_key_events: Vec::new(),
            command_key_capture_active: false,
            char_code_props: Vec::new(),
            char_code_prop_tables: Vec::new(),
            mtwlb_phase: 0,
            dribble_file: None,
            handler_bindings: Vec::new(),
            case_handlers: Vec::new(),
            var_watchers: Vec::new(),
            threads: Vec::new(),
            current_thread: 0,
            thread_scheduler: Default::default(),
            thread_yielder: None,
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
            byte_op_subrs_by_name: None,
            char_table_parents: Vec::new(),
            char_table_defalts: Vec::new(),
            frame_state_seen: None,
            lisp_stack: Vec::new(),
            definition_history_enabled: false,
            native_trampolines: HashSet::default(),
            native_lisp_literals: HashMap::default(),
            fixed_symbols: HashMap::default(),
            pending_frames: Vec::new(),
            compiled_fn_names: std::cell::RefCell::new(Vec::new()),
            subr_prof: std::collections::HashMap::new(),
            cur_head_form: Value::Nil,
            last_error_stack: std::cell::RefCell::new(Vec::new()),
            cpu_profiler: false,
            profiling: Default::default(),
            loaded_dump: None,
            terminal: None,
            terminal_params: Vec::new(),
            x_display_attempted: false,
            color_db_init: false,
            unibyte_strings: HashMap::default(),
            multibyte_strings: HashMap::default(),
            string_data_tick: 0,
            string_props: HashMap::default(),
            zero_vector: Value::vector(Vec::new()),
            zero_unibyte_string: Value::Nil,
            zero_multibyte_string: Value::Nil,
        };
        let unibyte = crate::lisp::value::Value::string_ref_with_kind("", false);
        interp.mark_unibyte(&unibyte);
        interp.zero_unibyte_string = Value::Str(unibyte);
        let multibyte = crate::lisp::value::Value::string_ref_with_kind("", true);
        interp.mark_multibyte(&multibyte);
        interp.zero_multibyte_string = Value::Str(multibyte);
        interp
    }

    pub fn new() -> Interp {
        Self::new_with_noninteractive(std::env::var_os("REMACS_NONINTERACTIVE").is_some())
    }

    /// Set the batch condition before initializing Lisp and loading libraries.
    /// Callers can select it without changing the process-wide environment.
    pub fn new_with_noninteractive(noninteractive: bool) -> Interp {
        let mut interp = Self::bootstrap_storage(noninteractive);
        let dump_features = interp.features.clone();
        let _init_t0 = std::time::Instant::now();
        let scratch = Self::install_boot_primitives(&mut interp, _init_t0);
        let rollback = Self::load_boot_image(&mut interp, _init_t0, &dump_features);
        Self::start_boot_session(&mut interp, scratch, dump_features, rollback);
        if std::env::var_os("REMACS_BOOT_PROF").is_some() {
            eprintln!(
                "[init] total Interp::new: {}ms",
                _init_t0.elapsed().as_millis()
            );
        }
        interp
    }

    fn bootstrap_storage(noninteractive: bool) -> Self {
        let mut obarray = Obarray::new();
        let standard_output_sym = obarray.intern("standard-output");
        let emacs_sym = obarray.intern("emacs");
        // GNU batch's feature list (NS tty build ordering);
        // `provide' appends to this.
        let feature_syms: Vec<SymId> = [
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
        let mut interp =
            Self::uninitialized(obarray, standard_output_sym, feature_syms, noninteractive);
        interp.threads.push(crate::lisp::memory::allocate(
            crate::lisp::value::Thread {
                name: None,
                alive: true,
                result: None,
                last_error: None,
                pending_signal: None,
                blocker: None,
                finished: false,
            },
            crate::lisp::Value::Thread,
        ));
        interp
    }

    fn install_boot_primitives(interp: &mut Self, _init_t0: std::time::Instant) -> usize {
        crate::lisp::builtins::install(interp);
        if std::env::var_os("REMACS_BOOT_PROF").is_some() {
            eprintln!(
                "[init] builtins::install: {}ms",
                _init_t0.elapsed().as_millis()
            );
        }
        crate::buffer::install_primitives(interp);
        interp.define_error_conditions();
        interp.define_special_variables();
        // Undo recording writes into the `buffer-undo-list' buffer-local
        // variable; buffers need the symbol's id before creation.
        let us = interp.q("buffer-undo-list");
        interp.buffers.set_undo_sym(us);
        interp.undo_inhibit_sym = interp.q("undo-inhibit-record-point");
        interp.bfn_sym = interp.q("buffer-file-name");
        interp.var_alias_prop = interp.q("variable-alias");
        // The initial buffers every Emacs session has, in GNU's
        // buffer-list order: (scratch Minibuf-0 Messages load
        // Warnings). New buffers append at the end of the order.
        let scratch = interp.create_buffer_nolast("*scratch*");
        interp.set_current_buffer(scratch);
        {
            // GNU's `*scratch*' carries buffer-local `lexical-binding'
            // = t (set up by startup.el's `eval-buffer' on scratch);
            // this also makes `eval_str' (which samples the buffer-
            // local value) evaluate the prelude lexically.
            let lb = interp.q("lexical-binding");
            interp
                .buffers
                .get(scratch)
                .unwrap()
                .borrow_mut()
                .locals
                .insert(lb, Value::t());
        }
        let minibuf = interp.create_buffer_exact(" *Minibuf-0*");
        let messages = interp.create_buffer_exact("*Messages*");
        // No ` *load*' buffer: GNU streams loads off the file descriptor
        // (`readevalloop'), so `buffer-list' at -Q holds only
        // scratch/Minibuf-0/Messages.
        // GNU does not create `*Warnings*' at startup; `display-warning'
        // makes it lazily (with messages-buffer-mode) on first warning.
        // GNU seeds the startup buffers' buffer-local major modes; its
        // `*Messages*' is read-only and carries a stale modified flag.
        let mm = interp.q("major-mode");
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
            let ro = interp.q("buffer-read-only");
            br.locals.insert(ro, Value::t());
            br.note_modified(true);
        }
        let event_kind = interp.q("event-kind");
        let event_elements = interp.q("event-symbol-elements");
        // keyboard.c syms_of_keyboard's head_table, before keymap setup.
        for (name, kind) in [
            ("mouse-movement", "mouse-movement"),
            ("scroll-bar-movement", "mouse-movement"),
            ("switch-frame", "switch-frame"),
            ("focus-in", "focus-in"),
            ("focus-out", "focus-out"),
            ("move-frame", "move-frame"),
            ("delete-frame", "delete-frame"),
            ("iconify-frame", "iconify-frame"),
            ("make-frame-visible", "make-frame-visible"),
            ("select-window", "switch-frame"),
            ("touchscreen-begin", "touchscreen"),
            ("touchscreen-end", "touchscreen"),
        ] {
            let symbol = interp.intern(name);
            let kind = Value::Sym(interp.intern(kind));
            interp.put_prop(symbol, event_kind, kind);
            interp.put_prop(
                symbol,
                event_elements,
                Value::list(vec![Value::Sym(symbol)]),
            );
        }
        crate::editor::install_primitives(interp);
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
        scratch
    }

    fn load_boot_image(
        interp: &mut Self,
        _init_t0: std::time::Instant,
        dump_features: &[SymId],
    ) -> BootRollback {
        // Load the Lisp prelude (subr.el subset). Errors here indicate a
        // broken prelude, but don't abort startup.
        let prelude_max: usize = std::env::var("PRELUDE_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(usize::MAX);
        // gv.el's post-rollback cells/props for the tail re-application
        // (populated right before the boot-rollback block ends; used at
        // the very end of `new' — the custom-reevaluate pass re-loads
        // gv.el via `advice-remove''s `(setf ...)' expansion).
        let mut rollback = BootRollback::default();
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
        // GNU's DEFVAR_INT("gensym-counter") binds the counter at C
        // init — always bound when any Lisp runs.  The dump-parity
        // reset below re-seeds it to 0 after boot; seed it here too so
        // prelude/boot gensyms don't hit `void-variable'.
        {
            let gc = interp.q("gensym-counter");
            interp.obarray.symbol_mut(gc).value = Value::Int(0);
        }

        interp.loading_dumped = true;
        if std::env::var("REMACS_NO_PRELUDE").is_ok() {
            // Debug escape: skip prelude evaluation entirely.
        } else if std::env::var("PRELUDE_TRACE").is_ok() || prelude_max != usize::MAX {
            interp.lexenv = lexenv_root();
            let src = crate::lisp::prelude::PRELUDE;
            let chars: Rc<Vec<crate::lisp::text::EmacsChar>> =
                Rc::new(src.chars().map(Into::into).collect());
            let mut pos = 0usize;
            let mut k = 0;
            loop {
                if k >= prelude_max {
                    break;
                }
                let next = {
                    let mut reader = crate::lisp::reader::Reader::with_chars(interp, chars.clone());
                    reader.set_position(pos);
                    match reader.read() {
                        Ok(f) => f.map(|v| (v, reader.position())),
                        Err(f) => {
                            eprintln!(
                                "[prelude] read error @{pos}: {:?} {}",
                                f,
                                match &f {
                                    crate::lisp::Flow::Signal(signal) =>
                                        interp.print_to_string(&signal.condition),
                                    _ => crate::lisp::text::EmacsText::new(),
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
                                .collect::<crate::lisp::text::EmacsText>()
                        );
                        if std::env::var("PRELUDE_TRACE_MX").is_ok() {
                            if let Ok(x) = interp.macroexpand(&form) {
                                eprintln!(
                                    "  expands to: {}",
                                    interp
                                        .prin1_to_string(&x)
                                        .chars()
                                        .take(800)
                                        .collect::<crate::lisp::text::EmacsText>()
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
                                        crate::lisp::Flow::Signal(signal) =>
                                            signal.condition.clone(),
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
            Self::evaluate_boot_prelude(interp, _init_t0);
            // Snapshot every symbol's state after the prelude but before
            // the boot-lib evals: interpreted loads fire
            // `eval-when-compile'/autoload requires that GNU's dumped
            // .elc files compiled away (cl-generic's `(eval-when-compile
            // (require 'cl-lib))' pulls cl-macs + cl-loaddefs + gv;
            // mouse pulls rect/send-to/map).  GNU's -Q image never ran
            // those files, so everything they touch — function cells,
            // value cells, plists — is restored from this snapshot after
            // the loads finish below.
            let pre_boot_syms: SymbolSnapshot = interp
                .obarray
                .all_ids()
                .iter()
                .map(|&id| (id, SymbolState::capture(interp.obarray.symbol(id))))
                .collect();
            Self::load_boot_libraries(interp);
            rollback =
                rollback::restore(interp, &pre_boot_syms, &pre_prelude_plists, dump_features);
        }
        rollback
    }

    fn start_boot_session(
        interp: &mut Self,
        scratch: usize,
        dump_features: Vec<SymId>,
        rollback: BootRollback,
    ) {
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
                interp,
                concat!(
                    env!("REMACS_SOURCE_ROOT"),
                    "/crates/runtime/src/lisp/prelude.el"
                ),
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
        let pch = interp.q("post-command-hook");
        let eldoc = interp.q("eldoc-schedule-timer");
        let prech = interp.q("pre-command-hook");
        // GNU `init_buffer' runs on every buffer creation (and pdump
        // restart re-creates buffers): `characters.el' cleared the
        // standard case table's canon/eqv extras mid-loadup, so the
        // next buffer init re-derives them via `set_case_table'.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            crate::lisp::builtins::misc::heal_standard_case_table(interp);
        }
        let eldoc_pre = interp.q("eldoc-pre-command-refresh-echo-area");
        if let Some(b) = interp.buffers.get(scratch) {
            let mut br = b.borrow_mut();
            br.locals
                .insert(pch, Value::list(vec![Value::Sym(eldoc), Value::t()]));
            br.locals
                .insert(prech, Value::list(vec![Value::Sym(eldoc_pre), Value::t()]));
            // GNU's *scratch* has `require-final-newline' t (captured
            // before files.el's defcustom nils the default).
            let rfn = interp.q("require-final-newline");
            br.locals.insert(rfn, Value::t());
            // ...and `eldoc-mode' on (set up by `lisp-interaction-mode'
            // via `global-eldoc-mode''s turn-on in GNU's startup),
            // which also pushes it onto `local-minor-modes'.
            let eld = interp.q("eldoc-mode");
            let lmm = interp.q("local-minor-modes");
            br.locals.insert(eld, Value::t());
            br.locals.insert(lmm, Value::list(vec![Value::Sym(eld)]));
        }
        let tooltip_hide = interp.q("tooltip-hide");
        interp.obarray.symbol_mut(prech).value = Value::list(vec![Value::Sym(tooltip_hide)]);
        // Boot-time autoloads (easy-mmode & co.) correspond to GNU's
        // dumped loadup; the user-visible `features' list must match the
        // post-dump set.
        interp.features = dump_features;
        // External-library features are registered at runtime (dlopen):
        // re-check after the dump restore so `featurep' sees e.g.
        // `lcms2'/`kqueue' when the libraries resolve.
        crate::lisp::builtins::register_extlib_features(interp);
        // `icons' faces created by boot-time loads are likewise
        // hidden until a real `load' triggers them.
        let icon_faces: Vec<_> = ["icon", "icon-button"].iter()
            .filter_map(|name| interp.intern_soft(name)).collect();
        interp.face_table.retain(|(face, _)| !icon_faces.contains(face));
        interp.face_t_table.retain(|(face, _)| !icon_faces.contains(face));
        // Interactive boot can evaluate the real deffaces.  Roll their
        // declarations and registration back with the realized faces,
        // otherwise custom-declare-face skips recreating them on require.
        for name in ["icon", "icon-button"] {
            let face = interp.intern(name);
            for property in ["face", "face-defface-spec", "face-override-spec"] {
                let property = interp.intern(property);
                interp.put_prop(face, property, Value::Nil);
            }
            if let Some(defaults) = interp.q_soft("face--new-frame-defaults") {
                if let Value::Hash(table) = interp.symbol_value(defaults) {
                    let key = crate::lisp::builtins::hashfn::hash_key_for(
                        &interp,
                        &Value::Sym(face),
                        crate::lisp::HashTest::Eq,
                    );
                    let mut table = table.borrow_mut();
                    table.map.remove(&key);
                    table.remove_key(&key);
                }
            }
        }
        // GNU xfaces.c seeds Vface_new_frame_defaults during startup
        // face realization; mirror it: every known face gets an
        // (id . lface) entry so `face-list'/`frame-face-alist' see
        // the same faces at -Q.
        {
            let mut names: Vec<_> = crate::editor::KNOWN_FACES.iter().map(|name| interp.q(name)).collect();
            names.extend(interp.face_table.iter().map(|(face, _)| *face));
            for n in &names {
                crate::editor::face_register_global(interp, n);
            }
        }
        let flist = Value::list(interp.features.iter().map(|s| Value::Sym(*s)).collect());
        let fid = interp.q("features");
        interp.obarray.symbol_mut(fid).value = flist;
        // GNU resets `gensym-counter' to 0 when the dumped image starts
        // (pdumper boot), so dump-time gensyms don't leak into the
        // session.  Our boot-time library loads play the dump's role;
        // reset last so boot's own macroexpansion gensyms don't count.
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            let gc = interp.q("gensym-counter");
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
                let iname = interp.q("invocation-name");
                let _ = interp.set_symbol(iname, Value::string(name));
                let idir = interp.q("invocation-directory");
                let _ = interp.set_symbol(idir, dir.map(Value::string).unwrap_or(Value::Nil));
                let cla = interp.q("command-line-args");
                let _ = interp.set_symbol(
                    cla,
                    Value::list(argv.iter().map(|a| Value::string(a.clone())).collect()),
                );
            }
        }
        Self::normalize_boot_session(interp, rollback);
    }

    fn evaluate_boot_prelude(interp: &mut Self, _init_t0: std::time::Instant) {
        let t_prelude = std::time::Instant::now();
        if std::env::var_os("REMACS_BOOT_PROF").is_some() {
            eprintln!("[init] pre-prelude: {}ms", _init_t0.elapsed().as_millis());
        }
        // Definitions during the prelude attach to
        // `current-load-list' (GNU's LOADHIST_ATTACH) — bind it so
        // those pushes die with the unwind; the real history entry
        // comes from `record_prelude_load_history' below.
        {
            let cll = interp.q("current-load-list");
            let mark = interp.specbind_depth();
            let _ = interp.specbind(cll, Value::Nil);
            if let Err(f) = interp.eval_str(crate::lisp::prelude::PRELUDE) {
                // An error aborts `eval_str' mid-file, silently dropping
                // every definition after the failing form — surface it.
                let detail = match &f {
                    crate::lisp::Flow::Signal(signal) => format!(
                        "signal {} {}",
                        interp.print_to_string(&signal.symbol()),
                        interp.print_to_string(&signal.data())
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
            let pk = interp.q("remacs--dump-fn");
            let auto_id = interp.q("autoload");
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
            let id = interp.q("cl-member");
            eprintln!(
                "[post-prelude] cl-member fn={:?} plist={:?}",
                interp.obarray.symbol(id).function,
                interp.obarray.symbol(id).plist
            );
        }
    }

    fn load_boot_libraries(interp: &mut Self) {
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
        let _ = boot_lib(interp, "byte-run");

        // GNU's loadup.el `(load "subr")' runs right after
        // byte-run (line 125): its defmacros install the
        // `(macro . closure)' cells that `macrop',
        // `symbol-function' and the census surface report.  The
        // prelude's folded definitions still win at eval time via
        // `special_form', so this only corrects the cell shape.
        let _ = boot_lib(interp, "subr");
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
        let _ = boot_lib(interp, "oclosure");
        // GNU loadup.el:199-200 dumps obarray.el then abbrev.el
        // right after cl-preloaded/oclosure and before help.
        // obarray.el makes `obarray-size' & co ordinary functions;
        // abbrev.el provides `make-abbrev-table' whose absence
        // aborts cl-generic's eager `define-abbrev-table'
        // expansion (and, under it, later boot files).
        let _ = boot_lib(interp, "obarray");
        let _ = boot_lib(interp, "abbrev");
        // easymenu.el is in GNU's dump (loadup.el:83 — before
        // register/textmodes): since remacs expands macros lazily
        // at load, every dumped library that calls
        // `easy-menu-define'/`easy-menu-add' (cl-generic, subr-x,
        // replace, newcomment, isearch, menu-bar) needs its macros
        // already real.
        let _ = boot_lib(interp, "easymenu");
        // help.el is dumped by GNU's loadup.el at line 202 — before
        // cl-generic (250).  `cl-defgeneric'/`cl-defmethod' forms in
        // cl-generic, seq, subr-x and register call
        // `help-add-fundoc-usage' → `help--docstring-quote' during
        // eager macro-expansion; loading help first keeps those
        // expansions bound like GNU's dump order.
        let _ = boot_lib(interp, "help");
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
        let _ = boot_lib(interp, "cl-generic");
        // seq.el is in GNU's dump too (loadup.el line 252, right
        // after cl-generic — its `.elc' calls cl-generic-define):
        // its `seq-let' defmacro replaces the raw-subr placeholder
        // so `(macrop 'seq-let)' reports t like GNU.
        let _ = boot_lib(interp, "seq");
        // env.el is preloaded into GNU's dump (loadup.el): its
        // feature is already registered, but the definitions must
        // exist too — `(require 'env)' short-circuits on the
        // feature mark, so evaluate the embedded source now.
        let _ = boot_lib(interp, "env");
        // jka-cmpr-hook.el is in GNU's dump too (loadup.el): the
        // feature mark would make `require' skip the definitions —
        // evaluate it so `jka-compr-installed-p' & co. exist and
        // `auto-compression-mode' installs its file-name handler.
        let _ = boot_lib(interp, "jka-cmpr-hook");
        // tabulated-list.el is likewise in GNU's dump (loadup.el):
        // same reasoning as env — the feature mark alone would
        // make `require' skip the definitions.
        let _ = boot_lib(interp, "tabulated-list");
        // register.el before subr-x.el: subr-x's compiled
        // `cl-defmethod register-val-jump-to ((val registerv) ...)'
        // errors "Unknown specializer registerv" unless the
        // `cl-defstruct' here has already registered the type.
        // (GNU dodges this by never dumping subr-x — it's pure
        // autoloads; we need its functions at -Q, so order wins.)
        let _ = boot_lib(interp, "register");
        // subr-x.el is also in GNU's dump: it holds the accumulated
        // GNU-verbatim compatibility definitions (string trim/pad,
        // fringe helpers, paren/select/vc/dnd support, …).
        let _ = boot_lib(interp, "subr-x");
        // Keep GNU's load-expansion hook, but use the Rust deep
        // walker behind its full-pass helper.  Interpreting the
        // amalgam's pcase-heavy walker exceeds Lisp depth at loadup.
        if let Some(subr) = crate::lisp::builtins::evalfn::SUBRS
            .iter()
            .find(|s| s.name == "macroexpand--all-toplevel")
        {
            let id = interp.intern(subr.name);
            interp.fset(id, Value::Subr(subr));
        }
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
        // GNU menu-bar.el owns its constructors. The serialized prelude
        // Edit snapshot dropped function-valued properties to nil.
        let _ = boot_lib(interp, "menu-bar");
        let _ = boot_lib(interp, "tool-bar");
        // widget.el is dumped too (loadup.el loads "widget" for
        // `define-widget' & co. used by cus-edit and friends).
        let _ = boot_lib(interp, "widget");
        // version.el is loaded by loadup.el without a `provide':
        // evaluating it defines `emacs-repository-*' helpers while
        // `(require 'version)' still fails exactly like GNU.
        let _ = boot_lib(interp, "version");
        // Match GNU's reproducible (--disable-build-details) image.
        // version.el initializes these from the running process; those
        // are not the machine or time at which this binary was built.
        for name in ["emacs-build-system", "emacs-build-time"] {
            let id = interp.intern(name);
            let _ = interp.set_symbol(id, Value::Nil);
        }
        // rx.el is *not* loaded in GNU's dump: `rx' and friends
        // are loaddefs autoloads at -Q, so the prelude-folded
        // definitions get stashed+autoloaded below rather than the
        // file being eagerly loaded here.
        // vc-hooks.el is in GNU's dump (loadup.el): its defcustoms
        // (`vc-handled-backends', `vc-ignore-dir-regexp', ...) and
        // the vc-file-* property machinery are bound at -Q.
        let _ = boot_lib(interp, "vc-hooks");
        // uniquify.el is dumped too; GNU's loadup order runs it
        // after vc-hooks, so `uniquify-kill-buffer-function'
        // prepends ahead of `vc-kill-buffer-hook'.
        let _ = boot_lib(interp, "uniquify");
        // tooltip.el is dumped on window-system builds too: GNU has
        // `tooltip-delay', `tooltip-mode' & co. bound at -Q, and the
        // `tooltip' feature mark alone would make `require' skip
        // the definitions.
        let _ = boot_lib(interp, "tooltip");
        // float-sup.el is dumped too (`lisp-float-type' feature):
        // `float-pi', `degrees-to-radians' & co. are bound at -Q.
        let _ = boot_lib(interp, "float-sup");
        // map-ynp.el is in GNU's dump (loadup.el): `map-y-or-n-p'
        // is bound at -Q, used by files.el's `save-some-buffers'.
        let _ = boot_lib(interp, "map-ynp");
        // debug-early.el is in GNU's dump too (loadup.el): it has
        // no `provide', so the feature stays nil while
        // `debug-early'/`debug-early-backtrace' are bound at -Q.
        let _ = boot_lib(interp, "debug-early");
        // Snapshot the real definitions of GNU_VOID_FNS before
        // `loaddefs' replaces some of them with (autoload ...) cells
        // (e.g. `cl-loop' => "cl-macs"): the `remacs--dump-fn' stash
        // must hold the true dumped definition, or cl-macs's restore
        // would reinstall the autoload and recurse into its own load.
        {
            let pk = interp.q("remacs--dump-fn");
            let auto_id = interp.q("autoload");
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
        let _ = boot_lib(interp, "nadvice");
        let _ = boot_lib(interp, "easymenu");
        // custom.el + cus-start.el are in GNU's dump (loadup.el
        // loads them right after mule): `defcustom'/`defgroup'/
        // `defface' must be real functions before later dumped
        // libraries (image/tab-bar/ns-win/mwheel) expand their
        // calls, and cus-start's `define-minor-mode' uses need
        // custom.el first.
        let _ = boot_lib(interp, "custom");
        // GNU loadup only seeds defaults and safety metadata here.
        // Full customization types/groups are installed on demand.
        let preload = interp.q("cus-start--preload");
        let mark = interp.specbind_depth();
        let _ = interp.specbind(preload, Value::t());
        let _ = boot_lib(interp, "cus-start");
        let _ = interp.unbind_to(mark);
        // loaddefs.el is loaded by loadup.el right after subr.el:
        // it installs the (autoload ...) cells for every preloaded
        // library's entry points and provides the `loaddefs'
        // feature, both visible at -Q.
        let _ = boot_lib(interp, "loaddefs");
        // tab-bar.el is in GNU's dump (loadup.el): tab-bar-mode,
        // tab-new, tab-switcher & co. are bound at -Q.  It needs
        // loaddefs's `frameset-filter-alist' defvar, hence the order.
        let _ = boot_lib(interp, "tab-bar");
        // image.el is in GNU's dump too (loadup.el): image-mode,
        // `image-load-path', `insert-image' & co. are bound at -Q.
        let _ = boot_lib(interp, "image");
        // buff-menu.el is in GNU's dump as well: `Buffer-menu-mode'
        // and friends are bound at -Q while the `buff-menu' feature
        // stays nil (the file has no `provide').
        let _ = boot_lib(interp, "buff-menu");
        // electric.el is in GNU's dump (loadup.el):
        // `electric-indent-mode'/`electric-quote-mode' are bound
        // at -Q and elec-pair.el needs `electric-quote-chars'.
        let _ = boot_lib(interp, "electric");
        // format.el and composite.el are in GNU's dump (loadup.el):
        // `format-alist', `format-decode' & co. and the
        // composition machinery are bound at -Q.
        let _ = boot_lib(interp, "format");
        let _ = boot_lib(interp, "composite");
        // iso-transl.el, mule-util.el and epa-hook.el are also in
        // GNU's dump (loadup.el): `iso-transl-set-language' & co.
        // are bound at -Q.
        let _ = boot_lib(interp, "iso-transl");
        let _ = boot_lib(interp, "mule-util");
        // mule.el and mule-conf.el are in GNU's dump (loadup.el):
        // `define-charset', `load-with-code-conversion' & co. and
        // the charsets (chinese-gb2312, japanese-jisx0213-*, ...)
        // are registered at -Q.
        let _ = boot_lib(interp, "mule");
        let _ = boot_lib(interp, "mule-conf");
        // mule-cmds/case-table/charprop/characters are in GNU's
        // dump (loadup.el:208-215, between mule-conf and the
        // language files).  `characters' populates the standard
        // category table (`modify-category-entry' ranges and
        // `map-charset-chars'), which `\\cX' regexes (e.g. `\\cj'
        // in ja-dic-cnv) rely on.
        let _ = boot_lib(interp, "mule-cmds");
        let _ = boot_lib(interp, "case-table");
        let _ = boot_lib(interp, "charprop");
        // GNU's temacs runs loadup.el with `dump-mode' non-nil,
        // so characters.el takes its `(load
        // "international/charscript")' branch and the script &
        // category side effects really execute.  Here the
        // `charscript'/`emoji-zwj' features are already
        // registered, so `(require ...)' would be a no-op —
        // pretend dump-mode for this one library.
        let dump_mode = interp.q("dump-mode");
        let _ = interp.set_symbol(dump_mode, Value::string("dump"));
        let _ = boot_lib(interp, "characters");
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
            let _ = boot_lib(interp, lib);
        }
        let _ = boot_lib(interp, "epa-hook");
        // paren.el is in GNU's dump (loadup.el): `show-paren-mode'
        // and the `paren' feature are bound at -Q.
        let _ = boot_lib(interp, "paren");
        // rmc.el is in GNU's dump too (loadup.el loads it early so
        // `read-multiple-choice' is available during startup).
        let _ = boot_lib(interp, "rmc");
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
            let _ = boot_lib(interp, lib);
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
        let _ = boot_lib(interp, "ucs-normalize");
        let _ = boot_lib(interp, "ns-win");
        let _ = boot_lib(interp, "common-win");
        let _ = boot_lib(interp, "fontset");
        // mwheel.el is in GNU's dump (loadup.el:366, after
        // term/ns-win, gated on `(fboundp 'x-create-frame)' which
        // holds on NS builds): `mouse-wheel-mode' must be the real
        // `define-minor-mode' function and the variable gets t via
        // :init-value, matching GNU -Q where the mode is on.
        let _ = boot_lib(interp, "mwheel");
        // GNU's dump includes leim/leim-list.el (loadup.el): its
        // `register-input-method' calls populate
        // `input-method-alist' so `activate-input-method' /
        // `read-multilingual-string' find the quail methods.
        let _ = boot_lib(interp, "leim-list");
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
    }

    fn normalize_boot_session(interp: &mut Self, rollback: BootRollback) {
        let BootRollback {
            symbols: gv_late_syms,
            properties: gv_late_props,
        } = rollback;
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
                    let pvk = interp.q("remacs--dump-var");
                    interp.put_prop(sid, pvk, old);
                }
                {
                    let s = interp.obarray.symbol_mut(sid);
                    s.value = Value::Sym(sym::UNBOUND);
                }
                for bid in interp.buffers.list() {
                    if let Some(b) = interp.buffers.get(bid) {
                        b.borrow_mut().locals.shift_remove(&sid);
                    }
                }
            }
            let pk = interp.q("remacs--dump-fn");
            let auto_id = interp.q("autoload");
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
                let comp_dash = interp.q("comp--");
                {
                    let s = interp.obarray.symbol_mut(comp_dash);
                    s.value = Value::Sym(sym::UNBOUND);
                    s.special = false;
                }
                let comp_hash = interp.q("comp--#$");
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
                    crate::buffer::primitives::delete_preserving_undo(interp, &mut bb, 0, z, false);
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
                let id = interp.q("search-emacs-glossary");
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
        crate::lisp::builtins::backquote::install_macros(interp);
        // GNU loadup installs the optimizer only after cconv and the other
        // dumped libraries are ready. Earlier bootstrap closures stay raw.
        let constructor = interp.q("internal-make-interpreted-closure-function");
        let optimizer = interp.q("cconv-make-interpreted-closure");
        let _ = interp.set_symbol(constructor, Value::Sym(optimizer));
        if std::env::var("REMACS_NO_PRELUDE").is_err() {
            // A compatibility wrapper, not user advice: dumped macro
            // expansion intentionally strips advice from called helpers.
            let dumped = interp.loading_dumped;
            interp.loading_dumped = true;
            let _ = interp.eval_str(
                "(eval \
                   '(let ((original (symbol-function 'cl-struct-define))) \
                      (fset 'cl-struct-define \
                        (lambda (name docstring parent type named slots children-sym tag print) \
                          (prog1 \
                            (funcall original name docstring parent type named slots children-sym tag print) \
                            (remacs--eieio-compiled-accessors name))))) \
                   t)",
            );
            interp.loading_dumped = dumped;
        }
        // GNU -Q starts a session with `gensym-counter' at 0 and no
        // interned `g<N>' symbols (its .elc boot never reads the let
        // bindings named g0.. that remacs's source boot interns).
        interp.obarray.unintern_boot_gensyms();
        // GNU -Q never evaluated gv.el: loadup's `(require 'gv)' is
        // gated on `add-hook' being interpreted, which is false for
        // the distributed (compiled) build — so no `gv-expander' props
        // and gv's entry points are autoload cells.  Remacs loads gv.el
        // once mid-boot (rolled back above) and AGAIN near the end via
        // `custom-reevaluate-setting' → electric-indent-mode →
        // `advice-remove''s interpreted `(setf ...)' expansion, which
        // lands past the rollback.  Reapply the rollback outcome here:
        // gv.el's own definitions restore their pre-boot (prelude
        // autoload) cells, and the prop keys its top-level forms wrote
        // (`gv-expander' on car/if/nth/..., edebug specs, the `eq'
        // obsolescence mark) are stripped.  `setf' still works: the
        // autoload cell loads gv.el on first use, same as GNU.
        {
            // Reapply the post-rollback gv state captured above.
            for (sid, f, v, p) in gv_late_syms.iter().cloned() {
                let s = interp.obarray.symbol_mut(sid);
                s.function = f;
                s.value = v;
                s.plist = p;
            }
            for (sid, key) in gv_late_props.iter().copied() {
                if key == sym::NIL {
                    continue;
                }
                let plist = interp.obarray.symbol(sid).plist.clone();
                interp.obarray.symbol_mut(sid).plist = properties::without(&plist, key);
            }
            let gv_id = interp.q("gv");
            interp.features.retain(|f| *f != gv_id);
            let fid = interp.q("features");
            if let Value::Cons(_) = interp.symbol_value(fid) {
                let items = interp.symbol_value(fid).list_to_vec().unwrap_or_default();
                let kept: Vec<Value> = items
                    .into_iter()
                    .filter(|v| !matches!(interp.sym_id(v), Some(id) if id == gv_id))
                    .collect();
                let _ = interp.set_symbol(fid, Value::list(kept));
            }
            let lh = interp.q("load-history");
            let hist = interp.symbol_value(lh);
            let mut items: Vec<Value> = Vec::new();
            let mut cur = hist;
            while let Value::Cons(c) = cur {
                items.push(c.borrow().car.clone());
                cur = c.borrow().cdr.clone();
            }
            items.retain(|entry| {
                let file = match entry {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => return true,
                };
                match &file {
                    Value::Str(s) => {
                        let f = s.borrow();
                        !f.ends_with("/gv.el") && !f.ends_with("/gv.elc")
                    }
                    _ => true,
                }
            });
            // Rolling compile-time libraries out of the dump must also
            // remove the dependency records they left in kept files.
            // A successful runtime require cannot depend on an absent
            // feature. Keep definitions/provides for symbol-file and all
            // requirements on features that really remain in the dump.
            let boot_features: HashSet<SymId> = interp
                .symbol_value(fid)
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .filter_map(|feature| interp.sym_id(feature))
                .collect();
            let require_id = interp.q("require");
            for entry in &items {
                if let Value::Cons(cell) = entry {
                    let mut entries = cell.borrow().cdr.list_to_vec().unwrap_or_default();
                    entries.retain(|entry| {
                        !matches!(entry, Value::Cons(dependency) if {
                            let dependency = dependency.borrow();
                            interp.sym_id(&dependency.car) == Some(require_id)
                                && interp.sym_id(&dependency.cdr)
                                    .is_some_and(|feature| !boot_features.contains(&feature))
                        })
                    });
                    cell.borrow_mut().cdr = Value::list(entries);
                }
            }
            let _ = interp.set_symbol(lh, Value::list(items));
        }
        // Snapshot GNU's post-loadup forwarding state, independently of
        // optional DOC data. Do not intern absent platform variables.
        for entry in include_str!("../forwarded_variables.txt").lines() {
            let (kind, name) = entry.split_once(' ').unwrap();
            if let Some(id) = interp.obarray.intern_soft(name) {
                if interp.bound_p(id) {
                    let symbol = interp.obarray.symbol_mut(id);
                    symbol.builtin_variable = true;
                    symbol.forwarded_variable = true;
                    symbol.make_local_if_set |= kind == "B";
                }
            }
        }
        crate::lisp::builtins::misc::install_variable_docs(interp);
        if std::env::var_os("REMACS_NO_PRELUDE").is_none()
            && std::env::var_os("PRELUDE_MAX").is_none()
        {
            if let Err(error) = crate::lisp::dump_history::restore(interp) {
                eprintln!("[boot-history] {error:?}");
            }
        }
        crate::lisp::load::finish_load_path(interp);
        // Native lookup keeps Qnumeric even if user code later uninterns it.
        interp.q("numeric");
        for symbol in interp.obarray.all_ids() {
            let name = interp.symbol_name(symbol);
            interp.fixed_symbols.entry(name).or_insert(symbol);
        }
        interp.definition_history_enabled = true;
    }
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
            crate::lisp::Flow::Signal(signal) => {
                let s = signal.symbol();
                let data = signal.data();
                format!("{} {}", i.prin1_to_string(&s), i.prin1_to_string(&data))
            }
            other => format!("{other:?}"),
        };
        eprintln!(
            "[boot-err] {name}: {msg} | head={}",
            i.princ_to_string(&i.cur_head_form)
                .chars()
                .take(200)
                .collect::<crate::lisp::text::EmacsText>()
        );
    }
    let _ = r;
}
