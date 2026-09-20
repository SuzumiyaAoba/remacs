//! Miscellaneous subrs: symbols, time values, hashing/crypto, file
//! attributes, environment, and small editor glue that doesn't belong
//! to a larger category module.

use std::cell::RefCell;
use std::rc::Rc;

use super::{S, arg, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Arity, Lambda, Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("gensym", 0, 1, f_gensym, "New uninterned symbol gN."),
    S!(
        "func-arity",
        1,
        1,
        f_func_arity,
        "Return (MIN . MAX) arity of FUNCTION."
    ),
    S!(
        "subr-arity",
        1,
        1,
        f_func_arity,
        "Return (MIN . MAX) arity of subr."
    ),
    S!("closurep", 1, 1, f_closurep, "t if OBJECT is a closure."),
    S!(
        "interpreted-function-p",
        1,
        1,
        f_interpreted_function_p,
        "t if FUNCTION is interpreted."
    ),
    S!(
        "make-interpreted-closure",
        3,
        3,
        f_make_interpreted_closure,
        "Build a closure from args/env/body."
    ),
    S!(
        "obarray-clear",
        0,
        1,
        f_obarray_clear,
        "Empty the obarray (keeps core syms)."
    ),
    S!(
        "getenv-internal",
        1,
        2,
        f_getenv_internal,
        "Environment variable value."
    ),
    S!(
        "command-modes",
        1,
        1,
        f_command_modes,
        "Modes a command applies to (nil)."
    ),
    S!(
        "abort-minibuffers",
        0,
        0,
        f_abort_minibuffers,
        "Abort any active minibuffer."
    ),
    S!(
        "accessible-keymaps",
        1,
        2,
        f_accessible_keymaps,
        "List (PREFIX . KEYMAP) reachable from MAP."
    ),
    S!(
        "map-keymap",
        2,
        3,
        f_map_keymap,
        "Call FUNCTION on each binding in KEYMAP."
    ),
    S!("map-keymap-internal", 2, 2, f_map_keymap, ""),
    S!(
        "keymap--get-keyelt",
        2,
        2,
        f_keymap_get_keyelt,
        "Return (BINDING . DEF) for OBJECT."
    ),
    S!(
        "describe-buffer-bindings",
        1,
        3,
        f_describe_bindings,
        "Print key bindings of BUFFER."
    ),
    S!(
        "set--this-command-keys",
        1,
        1,
        f_set_this_command_keys,
        "Set this-command-keys (stub)."
    ),
    S!(
        "documentation-stringp",
        1,
        1,
        f_documentation_stringp,
        "t if OBJECT is a docstring."
    ),
    S!(
        "error-message-string",
        1,
        1,
        f_error_message_string,
        "Format an error data list."
    ),
    S!(
        "external-debugging-output",
        1,
        1,
        f_external_debugging_output,
        "Write CHAR to stderr."
    ),
    S!(
        "open-dribble-file",
        1,
        1,
        f_open_dribble_file,
        "Record keystrokes to FILE (stub)."
    ),
    S!(
        "open-termscript",
        1,
        1,
        f_open_termscript,
        "Record terminal output to FILE (stub)."
    ),
    S!(
        "send-string-to-terminal",
        1,
        2,
        f_send_string_to_terminal,
        "Send STRING to the terminal."
    ),
    S!(
        "flush-standard-output",
        0,
        0,
        f_flush_stdout,
        "Flush stdout."
    ),
    S!(
        "encode-time",
        0,
        9,
        f_encode_time,
        "Convert time components to Lisp time."
    ),
    S!(
        "decode-time",
        0,
        3,
        f_decode_time,
        "Decompose Lisp time into components."
    ),
    S!("time-add", 2, 2, f_time_add, "Add two Lisp time values."),
    S!(
        "time-subtract",
        2,
        2,
        f_time_subtract,
        "Subtract two Lisp time values."
    ),
    S!("time-less-p", 2, 2, f_time_less_p, "t if TIME1 < TIME2."),
    S!("time-equal-p", 2, 2, f_time_equal_p, "t if TIME1 == TIME2."),
    S!(
        "time-convert",
        1,
        3,
        f_time_convert,
        "Convert TIME to FORM ticks."
    ),
    S!("emacs-uptime", 0, 1, f_emacs_uptime, "Process uptime."),
    S!(
        "current-cpu-time",
        0,
        0,
        f_current_cpu_time,
        "CPU time used by this process."
    ),
    S!(
        "set-time-zone-rule",
        1,
        1,
        f_set_time_zone,
        "Set TZ (returns t)."
    ),
    S!(
        "secure-hash",
        2,
        5,
        f_secure_hash,
        "Cryptographic hash of OBJECT."
    ),
    S!("md5", 1, 5, f_md5, "MD5 hash of OBJECT."),
    S!(
        "base64-encode-string",
        1,
        2,
        f_b64_encode_string,
        "Base64 encode STRING."
    ),
    S!(
        "base64-decode-string",
        1,
        2,
        f_b64_decode_string,
        "Base64 decode STRING."
    ),
    S!(
        "buffer-hash",
        0,
        1,
        f_buffer_hash,
        "Hash of buffer contents."
    ),
    S!(
        "make-temp-file-internal",
        4,
        4,
        f_make_temp_file_internal,
        "Create a temp file."
    ),
    S!(
        "make-symbolic-link",
        2,
        3,
        f_make_symbolic_link,
        "Create symlink FILENAME -> TARGET."
    ),
    S!(
        "directory-name-p",
        1,
        1,
        f_directory_name_p,
        "t if NAME ends in a slash."
    ),
    S!(
        "file-name-case-insensitive-p",
        1,
        1,
        f_file_name_case_insensitive_p,
        "t on case-insensitive FS."
    ),
    S!(
        "file-attributes-lessp",
        2,
        2,
        f_file_attributes_lessp,
        "t if ATTRS1 < ATTRS2 (mtime)."
    ),
    S!(
        "set-file-times",
        1,
        3,
        f_set_file_times,
        "Set file times (stub)."
    ),
    S!("file-acl", 1, 1, f_nil, "ACL list (unsupported)."),
    S!(
        "get-file-buffer",
        1,
        1,
        f_get_file_buffer,
        "Buffer visiting FILENAME."
    ),
    S!("unlock-file", 1, 1, f_nil, "Unlock FILE (no-op)."),
    S!("lock-file", 1, 1, f_nil, "Lock FILE (no-op)."),
    S!(
        "bare-symbol",
        1,
        1,
        f_bare_symbol,
        "Symbol without position info."
    ),
    S!(
        "bare-symbol-p",
        1,
        1,
        f_bare_symbol_p,
        "t if OBJECT is a symbol without position."
    ),
    S!(
        "position-symbol",
        2,
        2,
        f_position_symbol,
        "Symbol with position (ignored)."
    ),
    S!(
        "remove-pos-from-symbol",
        1,
        1,
        f_bare_symbol,
        "Strip position (ignored)."
    ),
    S!(
        "symbol-with-pos-p",
        1,
        1,
        f_false,
        "t if OBJECT is a positioned symbol."
    ),
    S!(
        "symbol-with-pos-pos",
        1,
        1,
        f_nil,
        "Position of a positioned symbol."
    ),
    S!(
        "internal-make-var-non-special",
        1,
        1,
        f_make_var_non_special,
        "Clear SPECIAL flag."
    ),
    S!(
        "special-variable-p",
        1,
        1,
        f_special_variable_p,
        "t if SYMBOL is special."
    ),
    // ---------- environment / user ----------
    S!(
        "getenv",
        1,
        2,
        f_getenv_internal,
        "Value of environment VARIABLE."
    ),
    S!(
        "setenv",
        1,
        3,
        f_setenv,
        "Set environment VARIABLE to VALUE."
    ),
    S!("user-login-name", 0, 1, f_user_login_name, "Login name."),
    S!(
        "user-real-login-name",
        0,
        0,
        f_user_login_name,
        "Real login name."
    ),
    S!("user-full-name", 0, 1, f_user_full_name, "Full name."),
    S!("user-uid", 0, 0, f_user_uid, "Effective uid."),
    S!("user-real-uid", 0, 0, f_user_uid, "Real uid."),
    S!("system-groups", 0, 0, f_system_groups, "Group names."),
    S!(
        "invocation-name",
        0,
        0,
        f_invocation_name,
        "Program invocation name."
    ),
    // ---------- version ----------
    S!(
        "version-to-list",
        1,
        1,
        f_version_to_list,
        "Version string to int list."
    ),
    S!("version<", 2, 2, f_version_lt, "t if V1 < V2."),
    S!("version<=", 2, 2, f_version_le, "t if V1 <= V2."),
    S!("version=", 2, 2, f_version_eq, "t if V1 == V2."),
    S!("version-list-<", 2, 2, f_version_list_lt, ""),
    S!("version-list-<=", 2, 2, f_version_list_le, ""),
    S!("version-list-=", 2, 2, f_version_list_eq, ""),
    S!("version-listp", 1, 1, f_version_listp, ""),
    // ---------- predicates ----------
    S!("string-or-null-p", 1, 1, f_string_or_null_p, ""),
    S!("vector-or-char-table-p", 1, 1, f_vector_or_char_table_p, ""),
    S!("subr-native-elisp-p", 1, 1, f_false, ""),
    S!("threadp", 1, 1, f_false, ""),
    S!("mutexp", 1, 1, f_false, ""),
    S!("condition-variable-p", 1, 1, f_false, ""),
    S!("cl-type-of", 1, 1, f_cl_type_of, ""),
    S!("bool-vector-p", 1, 1, f_bool_vector_p, ""),
    S!("record", many 0, f_record, "Create a record of TYPE with SLOTS."),
    S!("recordp", 1, 1, f_recordp, "t if OBJECT is a record."),
    S!("make-bool-vector", 2, 2, f_make_bool_vector, ""),
    S!("bool-vector-length", 1, 1, f_bool_vector_length, ""),
    S!("bool-vector-subsetp", 2, 2, f_bool_vector_subsetp, ""),
    S!("bool-vector-not", 1, 2, f_bool_vector_not, ""),
    S!("bool-vector-exclusive-or", 2, 3, f_bool_vector_bin, ""),
    S!("bool-vector-union", 2, 3, f_bool_vector_union, ""),
    S!("bool-vector-intersection", 2, 3, f_bool_vector_inter, ""),
    S!("bool-vector-set-difference", 2, 3, f_bool_vector_diff, ""),
    S!(
        "bool-vector-count-population",
        1,
        2,
        f_bool_vector_count,
        ""
    ),
    S!(
        "bool-vector-count-consecutive",
        2,
        2,
        f_bool_vector_consec,
        ""
    ),
    // ---------- events ----------
    S!("eventp", 1, 1, f_eventp, ""),
    S!("event-basic-type", 1, 1, f_event_basic_type, ""),
    S!("event-modifiers", 1, 1, f_event_modifiers, ""),
    S!("event-convert-list", 1, 1, f_event_convert_list, ""),
    S!("listify-key-sequence", 1, 1, f_listify_key_sequence, ""),
    S!("key-valid-p", 1, 1, f_key_valid_p, ""),
    S!("key-parse", 1, 1, f_key_parse, ""),
    // ---------- misc ----------
    S!(
        "days-between",
        2,
        2,
        f_days_between,
        "Days between two dates."
    ),
    S!(
        "date-to-time",
        1,
        1,
        f_date_to_time,
        "Parse an RFC822-ish date."
    ),
    S!(
        "memory-limit",
        0,
        0,
        f_memory_limit,
        "Most-positive-fixnum."
    ),
    S!("help-function-arglist", 1, 1, f_help_function_arglist, ""),
    S!("function-documentation", 1, 1, f_function_documentation, ""),
    S!("command-error-default-function", 3, 3, f_nil, ""),
    S!("detect-coding-string", 1, 2, f_detect_coding_string, ""),
    S!("detect-coding-region", 1, 3, f_detect_coding_region, ""),
    S!("coding-system-list", 0, 0, f_coding_system_list, ""),
    S!("coding-system-p", 1, 1, f_coding_system_p, ""),
    S!("check-coding-system", 1, 1, f_check_coding_system, ""),
    S!("coding-system-eol-type", 1, 1, f_coding_system_eol_type, ""),
    S!("coding-system-aliases", 1, 1, f_coding_system_aliases, ""),
    S!("coding-system-base", 1, 1, f_coding_system_base, ""),
    S!("coding-system-plist", 1, 1, f_coding_system_plist, ""),
    S!("coding-system-get", 2, 2, f_coding_system_get, ""),
    S!("coding-system-put", 3, 3, f_coding_system_put, ""),
    S!(
        "coding-system-priority-list",
        0,
        1,
        f_coding_system_list,
        ""
    ),
    S!("terminal-coding-system", 0, 1, f_terminal_coding_system, ""),
    S!("keyboard-coding-system", 0, 1, f_terminal_coding_system, ""),
    S!(
        "file-name-coding-system",
        0,
        0,
        f_terminal_coding_system,
        ""
    ),
    S!(
        "default-terminal-coding-system",
        0,
        0,
        f_terminal_coding_system,
        ""
    ),
    S!("encode-coding-string", 2, 4, f_encode_coding_string, ""),
    S!("decode-coding-string", 2, 4, f_decode_coding_string, ""),
    S!("encode-coding-char", 1, 2, f_encode_coding_char, ""),
    S!("decode-coding-region", 2, 4, f_decode_coding_region, ""),
    S!("encode-coding-region", 2, 4, f_encode_coding_region, ""),
    S!("check-coding-systems-region", 3, 3, f_nil, ""),
    // ---------- multibyte ----------
    // ---------- display/frame ----------
    S!("frame-configuration-p", 1, 1, f_frame_configuration_p, ""),
    S!(
        "current-frame-configuration",
        0,
        0,
        f_current_frame_configuration,
        ""
    ),
    S!("mouse-position", 0, 0, f_mouse_position, ""),
    S!("mouse-pixel-position", 0, 0, f_mouse_position, ""),
    S!("set-mouse-position", 3, 3, f_nil, ""),
    S!("set-mouse-pixel-position", 3, 3, f_nil, ""),
    S!("display-images-p", 0, 1, f_false, ""),
    S!("display-pixel-width", 0, 1, f_display_pixel_width, ""),
    S!("display-pixel-height", 0, 1, f_display_pixel_height, ""),
    S!("display-mm-width", 0, 1, f_display_mm, ""),
    S!("display-mm-height", 0, 1, f_display_mm, ""),
    S!("display-backing-store", 0, 1, f_not_useful, ""),
    S!("display-visual-class", 0, 1, f_display_visual_class, ""),
    S!("display-planes", 0, 1, f_display_planes, ""),
    S!("display-color-cells", 0, 1, f_display_color_cells, ""),
    S!("display-save-under", 0, 1, f_not_useful, ""),
    S!("display-monitor-attributes-list", 0, 1, f_nil, ""),
    S!("tool-bar-height", 0, 2, f_zero, ""),
    S!("tool-bar-pixel-width", 0, 1, f_zero, ""),
    S!("frame-monitor-attributes", 0, 1, f_nil, ""),
    S!("display-mm-dimensions-alist", 0, 1, f_nil, ""),
    S!("x-synchronize", 0, 2, f_nil, ""),
    S!("x-open-connection", 1, 2, f_nil, ""),
    S!("x-close-connection", 1, 1, f_nil, ""),
    S!("x-display-list", 0, 0, f_nil, ""),
    S!("xw-display-color-p", 0, 1, f_false, ""),
    S!("xw-color-defined-p", 1, 2, f_false, ""),
    S!("color-gray-p", 1, 2, f_color_gray_p, ""),
    S!("color-supported-p", 1, 2, f_color_defined_p, ""),
    S!("invert-face", 1, 2, f_nil, ""),
    S!("clear-face-cache", 0, 1, f_nil, ""),
    // ---------- windows ----------
    S!("minibuffer-selected-window", 0, 0, f_nil, ""),
    S!("window-min-height", 0, 0, f_window_min_height, ""),
    S!("window-min-width", 0, 0, f_window_min_width, ""),
    S!("window-sizable", 1, 3, f_window_sizable, ""),
    S!("window-fixed-size-p", 1, 2, f_nil, ""),
    S!("fit-window-to-buffer", 0, 4, f_nil, ""),
    S!("shrink-window-if-larger-than-buffer", 0, 1, f_nil, ""),
    S!("window-safely-shrinkable-p", 0, 2, f_nil, ""),
    S!("window--display-buffer", 3, 4, f_nil, ""),
    S!("window-max-chars-per-line", 0, 2, f_window_max_chars, ""),
    S!("window-preserve-size", 0, 3, f_nil, ""),
    S!("window-left-column", 0, 1, f_zero, ""),
    S!("pos-visible-in-window-group-p", 0, 3, f_pos_visible, ""),
    S!("window-line", 0, 1, f_window_line, ""),
    S!("window-normalize-window", 1, 1, f_window_normalize, ""),
    S!("window-normalize-buffer", 1, 1, f_window_norm_buffer, ""),
    S!("window-normalize-frame", 0, 1, f_window_norm_frame, ""),
    S!("delete-windows-on", 0, 3, f_nil, ""),
    S!("split-window-sensibly", 0, 1, f_nil, ""),
    S!("window-child", 1, 1, f_nil, ""),
    S!("window-child-count", 1, 1, f_zero, ""),
    S!("window-combined-p", 0, 2, f_nil, ""),
    S!("window-leftmost-p", 1, 1, f_t, ""),
    S!("window-rightmost-p", 1, 1, f_t, ""),
    S!("window-topmost-p", 1, 1, f_t, ""),
    S!("window-bottommost-p", 1, 1, f_t, ""),
    S!("window-at-side-p", 1, 2, f_t, ""),
    S!("window-in-direction", 1, 5, f_window_in_direction, ""),
    S!("window-has-dark-scroll-bar", 0, 0, f_nil, ""),
    S!("window-group", 0, 1, f_nil, ""),
    S!("window-main-window", 0, 1, f_nil, ""),
    S!("get-mru-window", 0, 2, f_selected_window, ""),
    S!("get-window-with-predicate", 1, 3, f_get_window_pred, ""),
    // ---------- keymap ops ----------
    S!("suppress-keymap", 1, 2, f_suppress_keymap, ""),
    S!("make-composed-keymap", 1, 2, f_make_composed_keymap, ""),
    S!("current-active-maps", 0, 2, f_current_active_maps, ""),
    S!("keymap--mergable", 1, 1, f_t, ""),
    S!("keymap-canonicalize", 1, 1, f_identity, ""),
    S!("set-transient-map", 1, 3, f_set_transient_map, ""),
    S!("text-mode-map", 0, 0, f_nil, ""),
    // ---------- tables ----------
    S!("buffer-display-table", 0, 0, f_nil, ""),
    S!("char-table-extra-slot", 3, 3, f_nil, ""),
    S!("set-char-table-extra-slot", 4, 4, f_nil, ""),
    S!("char-table-range", 2, 2, f_char_table_range, ""),
    S!("set-char-table-range", 3, 3, f_set_char_table_range, ""),
    S!("char-table-parent", 1, 1, f_nil, ""),
    S!("set-char-table-parent", 2, 2, f_nil, ""),
    S!("map-char-table", 2, 2, f_map_char_table, ""),
    S!("optimize-char-table", 1, 2, f_nil, ""),
    S!("char-table-subtype", 1, 1, f_char_table_subtype, ""),
    S!("char-table-p", 1, 1, f_char_table_p, ""),
];

// ---------- symbols / functions ----------

fn f_gensym(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: PREFIX is a string or number (a number becomes the
    // numeric prefix of the name, e.g. (gensym 5) -> symbol "5N").
    let prefix = match args.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(Value::Int(n)) => n.to_string(),
        _ => "g".to_string(),
    };
    Ok(Value::Sym(i.obarray.gensym(&prefix)))
}

fn f_func_arity(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = i.indirect_function_value(&args[0]);
    let arity = match &fun {
        Value::Subr(s) => s.arity,
        Value::Lambda(l) => l.arity(),
        _ => return Err(i.signal_data(sym::VOID_FUNCTION, vec![args[0].clone()])),
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min, Value::Int(max as i128)),
        Arity::Many { min } => (min, Value::Sym(i.intern("many"))),
        Arity::Unevalled => {
            // `(2 . unevalled)' for `if' — min from the special-form table.
            let min = match &args[0] {
                Value::Sym(id) => crate::lisp::special::special_form_min_args(*id),
                Value::Subr(s) => crate::lisp::special::special_form_min_args(i.intern(s.name)),
                _ => 0,
            };
            (min, Value::Sym(i.intern("unevalled")))
        }
    };
    Ok(Value::cons(Value::Int(min as i128), max))
}

fn f_closurep(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

fn f_interpreted_function_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

fn f_make_interpreted_closure(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (make-interpreted-closure ARGS BODY ENV) → Lambda value.
    // Parse ARGS (a list arglist) into required/optional/rest.
    let arglist = args[0].list_to_vec().unwrap_or_default();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut mode = 0; // 0 req, 1 opt, 2 rest
    let opt_sym = i.intern("&optional");
    let rest_sym = i.intern("&rest");
    for a in &arglist {
        if let Some(id) = i.sym_id(a) {
            if id == opt_sym {
                mode = 1;
                continue;
            }
            if id == rest_sym {
                mode = 2;
                continue;
            }
            if mode == 2 {
                rest = Some(id);
                continue;
            }
            if mode == 0 {
                required.push(id);
            } else {
                optional.push(crate::lisp::value::OptParam {
                    sym: id,
                    default: None,
                });
            }
        }
    }
    let body = args[1].list_to_vec().unwrap_or_default();
    let env = match &args[2] {
        Value::Nil => None,
        // An env value is a list of binding alists — model as a flat
        // alist lexical frame.
        alist => {
            let mut vars = std::collections::HashMap::new();
            let mut cur = alist.clone();
            // Peel off a possible outer context list.
            loop {
                let next = match &cur {
                    Value::Cons(c) => {
                        let b = c.borrow();
                        let elem = b.car.clone();
                        let rest = b.cdr.clone();
                        // Elements may be (sym . val) conses or bare syms.
                        if let Value::Cons(p) = &elem {
                            let pb = p.borrow();
                            if let Some(sid) = i.sym_id(&pb.car) {
                                vars.insert(sid, pb.cdr.clone());
                            }
                        }
                        rest
                    }
                    _ => Value::Nil,
                };
                if matches!(next, Value::Nil) {
                    break;
                }
                cur = next;
            }
            Some(Rc::new(crate::lisp::LexFrame {
                vars: RefCell::new(vars),
                parent: None,
            }))
        }
    };
    Ok(Value::Lambda(Rc::new(Lambda {
        is_macro: false,
        required,
        optional,
        rest,
        body,
        env,
        doc: None,
        interactive: None,
        name: None,
        bad_arglist: false,
        arglist: None,
        plain: false,
    })))
}

fn f_obarray_clear(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Symbol ids are positional across the whole runtime (buffer locals,
    // specbind), so actually clearing the obarray would corrupt state.
    // Treat as a no-op that returns its argument.
    Ok(arg(&args, 0))
}

fn f_getenv_internal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    // Emacs looks up `process-environment' first.
    let pe = i.intern("process-environment");
    let proc_env = i.symbol_value(pe);
    if let Value::Cons(_) = &proc_env {
        let prefix = format!("{}=", name);
        let mut hit: Option<String> = None;
        proc_env.each_car(|v| {
            if let Value::Str(s) = v {
                let s = s.borrow();
                if s.starts_with(&prefix) {
                    hit = Some(s[prefix.len()..].to_string());
                }
            }
        });
        if let Some(v) = hit {
            return Ok(Value::string(v));
        }
    }
    match std::env::var(&name) {
        Ok(v) => Ok(Value::string(v)),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_command_modes(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = args;
    Ok(Value::Nil)
}

fn f_abort_minibuffers(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn keymap_of(i: &mut Interp, v: &Value) -> Result<Option<Vec<Value>>, Flow> {
    // Our keymaps are lists whose car is the symbol `keymap` and whose
    // cdr is an alist of (KEY . DEF) or sparse vectors. Return pairs.
    match v {
        Value::Cons(c) => {
            let (head, tail) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if i.sym_id(&head) != i.intern_soft("keymap") {
                return Ok(None);
            }
            let mut pairs = Vec::new();
            let mut cur = tail;
            loop {
                match cur {
                    Value::Nil => break,
                    Value::Cons(cc) => {
                        let (elem, next) = {
                            let b = cc.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        pairs.push(elem);
                        cur = next;
                    }
                    _ => break,
                }
            }
            Ok(Some(pairs))
        }
        _ => Ok(None),
    }
}

fn f_accessible_keymaps(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let prefix = arg(&args, 1);
    let _ = prefix;
    match keymap_of(i, &args[0])? {
        Some(pairs) => {
            let out: Vec<Value> = pairs
                .into_iter()
                .map(|p| Value::cons(Value::Nil, p))
                .collect();
            Ok(Value::list(out))
        }
        None => Err(i.wrong_type_mut("keymapp", &args[0])),
    }
}

fn f_map_keymap(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match keymap_of(i, &args[1])? {
        Some(pairs) => {
            for p in pairs {
                // Parent slots (keymap values / keymap lists) aren't
                // bindings.
                if is_keymap(i, &p) {
                    continue;
                }
                let (k, def) = match &p {
                    Value::Cons(c) => {
                        let b = c.borrow();
                        if matches!(&b.car, Value::Cons(_)) {
                            continue;
                        }
                        (b.car.clone(), b.cdr.clone())
                    }
                    _ => continue,
                };
                if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == "keymap") {
                    continue;
                }
                let fnv = args[0].clone();
                i.apply(&fnv, vec![k, def])?;
            }
            Ok(Value::Nil)
        }
        None => Err(i.wrong_type_mut("keymapp", &args[1])),
    }
}

fn f_keymap_get_keyelt(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => Ok(args[0].clone()),
        _ => Ok(Value::Nil),
    }
}

fn f_describe_bindings(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.write_output("Key bindings not implemented\n");
    Ok(Value::Nil)
}

fn f_set_this_command_keys(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_documentation_stringp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Str(_))))
}

fn f_error_message_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Mirrors print_error_message in print.c.
    // Fast path: (error STRING) → STRING.
    if let Value::Cons(c) = &args[0] {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(d) = &cdr {
            let (d1, drest) = {
                let b = d.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if i.sym_id(&car) == Some(sym::ERROR) && matches!(d1, Value::Str(_)) && drest.is_nil() {
                return Ok(d1);
            }
        }
    }
    Ok(Value::string(error_message(i, &args[0])))
}

/// Format an error object `(SYMBOL . DATA)` the way Emacs's
/// `print_error_message` does.
pub fn error_message(i: &mut Interp, obj: &Value) -> String {
    let (errname, data) = match obj {
        Value::Cons(c) => {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        }
        _ => (obj.clone(), Value::Nil),
    };
    let errname_id = i.sym_id(&errname);
    let (errmsg, file_error, tail_list): (Value, bool, Vec<Value>);
    if errname_id == Some(sym::ERROR) {
        // For `error`, the first data item is the message.
        let items = data.list_to_vec().unwrap_or_default();
        errmsg = items.first().cloned().unwrap_or(Value::Nil);
        file_error = false;
        tail_list = items.get(1..).map(|s| s.to_vec()).unwrap_or_default();
    } else {
        let mut msg = Value::Nil;
        let mut ferror = false;
        if let Some(id) = errname_id {
            let plist = i.obarray.symbol(id).plist.clone();
            let em_id = i.intern("error-message");
            msg = crate::lisp::eval::plist_get(&plist, em_id);
            let ec_id = i.intern("error-conditions");
            let conds = crate::lisp::eval::plist_get(&plist, ec_id);
            let fe_id = i.intern("file-error");
            ferror = conds
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .any(|c| i.sym_id(c) == Some(fe_id));
        }
        errmsg = msg;
        file_error = ferror;
        tail_list = data.list_to_vec().unwrap_or_default();
    }
    let (errmsg, tail): (Value, &[Value]) = if file_error && !tail_list.is_empty() {
        // file-error: first data item is the message string.
        (tail_list[0].clone(), &tail_list[1..])
    } else {
        (errmsg, &tail_list[..])
    };
    // quote-curve the message text (substitute-command-keys applies
    // text-quoting-style 'curve to doc/error strings).
    let msg_text = match &errmsg {
        Value::Str(s) => Some(curve_quotes(&s.borrow())),
        _ => None,
    };
    let mut out = String::new();
    let mut sep: Option<&str> = Some(": ");
    match &msg_text {
        None => out.push_str("peculiar error"),
        Some(t) if t.is_empty() => sep = None,
        Some(t) => out.push_str(t),
    }
    let princ_mode =
        file_error || errname_id == Some(sym::END_OF_FILE) || errname_id == Some(sym::USER_ERROR);
    for item in tail {
        if let Some(s) = sep {
            out.push_str(s);
        }
        sep = Some(", ");
        if princ_mode {
            out.push_str(&i.princ_to_string(item));
        } else {
            out.push_str(&i.prin1_to_string(item));
        }
    }
    out
}

/// Apply Emacs's default `text-quoting-style` (curve) to a message
/// template: `'` after a word char → `’`, before a word char → `‘`;
/// `` ` `` before a word char → `‘`.
fn curve_quotes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (k, &c) in chars.iter().enumerate() {
        match c {
            '\'' => {
                let prev = if k > 0 {
                    chars.get(k - 1).copied()
                } else {
                    None
                };
                out.push(
                    if prev
                        .map(|p| p.is_alphanumeric() || p == '\'')
                        .unwrap_or(false)
                    {
                        '\u{2019}'
                    } else {
                        '\u{2018}'
                    },
                );
            }
            '`' => out.push('\u{2018}'),
            _ => out.push(c),
        }
    }
    out
}

fn f_external_debugging_output(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Some(c) = args[0].int() {
        eprint!("{}", char::from_u32(c as u32).unwrap_or('?'));
    }
    Ok(args[0].clone())
}

fn f_open_dribble_file(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_open_termscript(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_send_string_to_terminal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    print!("{}", s);
    use std::io::Write;
    let _ = std::io::stdout().flush();
    Ok(Value::Nil)
}

fn f_flush_stdout(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    Ok(Value::Nil)
}

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_false(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_bare_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(_) => Ok(args[0].clone()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_bare_symbol_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Sym(_))))
}

fn f_position_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(_) => Ok(args[0].clone()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_make_var_non_special(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Some(id) = i.sym_id(&args[0]) {
        i.obarray.symbol_mut(id).special = false;
    }
    Ok(Value::Nil)
}

fn f_special_variable_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match i.sym_id(&args[0]) {
        Some(id) => Ok(Value::from_bool(i.obarray.symbol(id).special)),
        None => Err(i.wrong_type_mut("symbolp", &args[0])),
    }
}

// ---------- time values ----------
//
// Lisp time is (HIGH LOW MICRO PICO) or an integer seconds/ticks. We
// convert to microseconds (i128) internally.

pub(crate) fn lisp_time_to_us(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok(now.as_micros() as i128)
        }
        Value::Int(n) => Ok(*n as i128 * 1_000_000),
        Value::Float(f) => Ok((*f * 1e6) as i128),
        Value::Cons(_) => {
            // Walk the conses — a dotted tail means (TICKS . HZ).
            let mut elems: Vec<i128> = Vec::new();
            let mut tail = v.clone();
            let mut hz: Option<i128> = None;
            loop {
                let step = match &tail {
                    Value::Cons(c) => {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Int(n) = car {
                            elems.push(n as i128);
                        }
                        Some(cdr)
                    }
                    Value::Int(n) => {
                        hz = Some(*n as i128);
                        None
                    }
                    _ => None,
                };
                match step {
                    Some(cdr) => tail = cdr,
                    None => break,
                }
                if elems.len() > 4 {
                    break;
                }
            }
            if let Some(hz) = hz {
                let ticks = elems.first().copied().unwrap_or(0);
                return Ok(ticks * 1_000_000 / hz.max(1));
            }
            let n = |k: usize| elems.get(k).copied().unwrap_or(0);
            let ticks = match elems.len() {
                0 => 0,
                1 => n(0),
                _ => n(0) * 65536 + n(1),
            };
            Ok(ticks * 1_000_000 + n(2))
        }
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

pub(crate) fn us_to_lisp_time(us: i128) -> Value {
    let secs = us.div_euclid(1_000_000);
    let micro = us.rem_euclid(1_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(micro as i128),
        Value::Int(0),
    ])
}

/// Minimal POSIX tm for `localtime_r` (macOS/Linux layout).
#[repr(C)]
pub(crate) struct Tm {
    pub tm_sec: i32,
    pub tm_min: i32,
    pub tm_hour: i32,
    pub tm_mday: i32,
    pub tm_mon: i32,
    pub tm_year: i32,
    pub tm_wday: i32,
    pub tm_yday: i32,
    pub tm_isdst: i32,
    pub tm_gmtoff: i64,
    pub tm_zone: *const u8,
}

unsafe extern "C" {
    pub(crate) fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
}

/// Local-time breakdown for a unix-second timestamp.
pub(crate) fn local_tm(secs: i64) -> Tm {
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { localtime_r(&secs, &mut tm) };
    tm
}

/// Days since 1970-01-01 for (y, m, d) — Howard Hinnant's algorithm.
fn days_from_civil(y: i128, m: i128, d: i128) -> i128 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn f_encode_time(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (encode-time SECOND MINUTE HOUR DAY MONTH YEAR &rest) or a list.
    let items: Vec<Value> = if args.len() == 1 {
        match args[0].list_to_vec() {
            Ok(v) => v,
            Err(_) => args.clone(),
        }
    } else {
        args.clone()
    };
    let get = |k: usize| -> i128 { items.get(k).and_then(|v| v.int()).unwrap_or(0) };
    let (sec, min, hour, day, mon, year) = (get(0), get(1), get(2), get(3), get(4), get(5));
    let days = days_from_civil(year, mon.max(1).min(12), day.max(1));
    let mut secs = days * 86400 + hour * 3600 + min * 60 + sec;
    // ZONE (index 8) may give an explicit offset in seconds.
    if let Some(Value::Int(off)) = items.get(8) {
        secs -= off;
    } else {
        // Interpret as local time: find the local UTC offset at this
        // approximate instant via localtime_r.
        secs -= local_tm(secs as i64).tm_gmtoff as i128;
    }
    // Emacs returns (HIGH LOW) — seconds split at 2^16.
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Ok(Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
    ]))
}

fn f_decode_time(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let t = match args.get(0) {
        Some(v) => lisp_time_to_us(i, v)?,
        None => lisp_time_to_us(i, &Value::Nil)?,
    };
    let secs = (t / 1_000_000) as i64;
    let tm = local_tm(secs);
    Ok(Value::list(vec![
        Value::Int(tm.tm_sec as i128),
        Value::Int(tm.tm_min as i128),
        Value::Int(tm.tm_hour as i128),
        Value::Int(tm.tm_mday as i128),
        Value::Int(tm.tm_mon as i128 + 1),
        Value::Int(tm.tm_year as i128 + 1900),
        Value::Int(tm.tm_wday as i128),
        if tm.tm_isdst > 0 {
            Value::t()
        } else {
            Value::Nil
        },
        Value::Int(tm.tm_gmtoff as i128),
    ]))
}

/// `time-add`/`time-subtract`: integer args give an integer result.
fn time_arith(i: &mut Interp, a: &[Value], sub: bool) -> EvalResult {
    if let (Value::Int(x), Value::Int(y)) = (&a[0], &a[1]) {
        return Ok(Value::Int(if sub { x - y } else { x + y }));
    }
    let x = lisp_time_to_us(i, &a[0])?;
    let y = lisp_time_to_us(i, &a[1])?;
    Ok(us_to_lisp_time(if sub { x - y } else { x + y }))
}

fn f_time_add(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    time_arith(i, &args, false)
}

fn f_time_subtract(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    time_arith(i, &args, true)
}

fn f_time_less_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(Value::from_bool(a < b))
}

fn f_time_equal_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}

fn f_time_convert(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let us = lisp_time_to_us(i, &args[0])?;
    let form = args.get(1);
    let hz = args.get(2).and_then(|v| v.int()).unwrap_or(1_000_000);
    match form {
        Some(Value::Sym(_)) => {
            let name = i.symbol_name(i.sym_id(&args[1]).unwrap_or(0));
            if name == "integer" {
                Ok(Value::Int((us * hz as i128 / 1_000_000) as i128))
            } else {
                Ok(us_to_lisp_time(us))
            }
        }
        Some(Value::Int(h)) => Ok(Value::Int((us * *h as i128 / 1_000_000) as i128)),
        _ => Ok(us_to_lisp_time(us)),
    }
}

fn f_emacs_uptime(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let start = START.get_or_init(std::time::Instant::now);
    let secs = start.elapsed().as_secs() as i128;
    if let Some(fmt) = a.get(0) {
        if fmt.truthy() {
            // (emacs-uptime FORMAT) formats via format-time-string.
            let fts = i.intern("format-time-string");
            if i.fbound_p(fts) {
                let t = us_to_lisp_time(start.elapsed().as_micros() as i128);
                let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
                let args = Value::list(vec![q(fmt), q(&t)]);
                return i.call_function(&Value::Sym(fts), &args, None);
            }
        }
    }
    let days = secs / 86400;
    let s = if days <= 0 {
        format!("{} second{}", secs, if secs == 1 { "" } else { "s" })
    } else {
        let rem = secs % 86400;
        format!(
            "{} day{}, {:02}:{:02}:{:02}",
            days,
            if days == 1 { "" } else { "s" },
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    };
    Ok(Value::string(s))
}

fn f_current_cpu_time(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let start = START.get_or_init(std::time::Instant::now);
    Ok(us_to_lisp_time(start.elapsed().as_micros() as i128))
}

static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn f_set_time_zone(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

// ---------- hashing / base64 ----------

fn hash_hex(algo: &str, bytes: &[u8]) -> Result<String, Flow> {
    use sha2::Digest;
    let out = match algo {
        "md5" => md5::Md5::digest(bytes).to_vec(),
        "sha1" => sha1::Sha1::digest(bytes).to_vec(),
        "sha224" => sha2::Sha224::digest(bytes).to_vec(),
        "sha256" => sha2::Sha256::digest(bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(bytes).to_vec(),
        "sha512" => sha2::Sha512::digest(bytes).to_vec(),
        _ => return Err(Flow::Signal(Value::Nil, Value::Nil)),
    };
    Ok(out.iter().map(|b| format!("{:02x}", b)).collect())
}

fn secure_hash_str(i: &mut Interp, args: &[Value]) -> Result<String, Flow> {
    let algo_sym = i.sym_id(&args[0]).unwrap_or(u32::MAX);
    let algo = i.symbol_name(algo_sym);
    let obj = &args[1];
    let bytes: Vec<u8> = match obj {
        Value::Str(s) => s.borrow().as_bytes().to_vec(),
        _ => {
            // Buffer text (current buffer or 4th arg).
            let b = i
                .current_buffer_ref()
                .ok_or_else(|| i.error("No current buffer"))?;
            let bb = b.borrow();
            bb.text.text().into_bytes()
        }
    };
    hash_hex(&algo, &bytes).map_err(|_| {
        i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Unknown hash algorithm {}", algo))],
        )
    })
}

fn f_secure_hash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::string(secure_hash_str(i, &args)?))
}

fn f_md5(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut a = vec![Value::Sym(i.intern("md5"))];
    a.extend(args);
    f_secure_hash(i, a)
}

fn f_b64_encode_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let s = want_string(i, &args[0])?;
    let no_break = arg(&args, 1).truthy();
    let out = if no_break {
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(s.as_bytes())
    } else {
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    };
    Ok(Value::string(out))
}

fn f_b64_decode_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let s = want_string(i, &args[0])?;
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    match base64::engine::general_purpose::STANDARD.decode(cleaned.as_bytes()) {
        Ok(bytes) => Ok(Value::string(String::from_utf8_lossy(&bytes).to_string())),
        Err(_) => Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid base64 data")])),
    }
}

fn f_buffer_hash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let b = match args.get(0) {
        Some(Value::Buffer(b)) => b.clone(),
        _ => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("No current buffer"))?,
    };
    let text = b.borrow().text.text();
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    Ok(Value::Int(h.finish() as i128))
}

// ---------- files ----------

fn f_make_temp_file_internal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let prefix = want_string(i, &args[0])?;
    let dir = std::env::temp_dir();
    for n in 0..1000u32 {
        let cand = dir.join(format!(
            "{}{}",
            prefix,
            if n == 0 {
                random_suffix()
            } else {
                format!("{}{}", random_suffix(), n)
            }
        ));
        match std::fs::File::create_new(&cand) {
            Ok(_) => return Ok(Value::string(cand.to_string_lossy().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(i.signal_data(
                    sym::FILE_ERROR,
                    vec![
                        Value::string(format!("Creating temp file: {}", e)),
                        Value::string(cand.to_string_lossy().to_string()),
                    ],
                ));
            }
        }
    }
    Err(i.signal_data(
        sym::FILE_ERROR,
        vec![Value::string("Cannot create temp file")],
    ))
}

fn random_suffix() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        ^ (std::process::id() << 16);
    format!("{:x}", n)
}

fn f_make_symbolic_link(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let target = want_string(i, &args[0])?;
    let name = want_string(i, &args[1])?;
    match std::os::unix::fs::symlink(&target, &name) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Making symbolic link: {}", e)),
                Value::string(name),
            ],
        )),
    }
}

fn f_directory_name_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    let s = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    Ok(Value::from_bool(s.ends_with('/')))
}

fn f_file_name_case_insensitive_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let path = want_string(i, &args[0])?;
    // macOS default FS is case-insensitive; probe by trying to stat a
    // case-flipped name.
    let flipped: String = path
        .chars()
        .map(|c| {
            if c.is_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    if flipped == path {
        return Ok(Value::from_bool(std::path::Path::new(&path).exists()));
    }
    Ok(Value::from_bool(
        std::path::Path::new(&flipped).exists() && std::path::Path::new(&path).exists(),
    ))
}

fn f_file_attributes_lessp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Compare mtime (index 5 in the attributes list) — simplified:
    // compare nth 5 element if present, else nil.
    let get_mtime = |v: &Value| -> Option<i128> {
        let items = v.list_to_vec().ok()?;
        items.get(5).and_then(|x| x.int())
    };
    match (get_mtime(&args[0]), get_mtime(&args[1])) {
        (Some(a), Some(b)) => Ok(Value::from_bool(a < b)),
        _ => Ok(Value::Nil),
    }
}

fn f_set_file_times(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    if !std::path::Path::new(&name).exists() {
        return Err(i.signal_data(
            sym::FILE_MISSING,
            vec![Value::string(format!(
                "Setting file times: no such file {}",
                name
            ))],
        ));
    }
    Ok(Value::t())
}

fn f_get_file_buffer(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let canonical = std::fs::canonicalize(&name)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| name.clone());
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let bb = b.borrow();
            if let Some(f) = &bb.file_name {
                let fc = std::fs::canonicalize(f)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| f.clone());
                if fc == canonical {
                    return Ok(i.buffer_value(id).unwrap_or(Value::Nil));
                }
            }
        }
    }
    Ok(Value::Nil)
}

// ---------- environment / user ----------

fn f_setenv(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let val = match args.get(1) {
        Some(Value::Nil) | None => None,
        Some(v) => Some(want_string(i, v)?),
    };
    match &val {
        Some(v) => unsafe { std::env::set_var(&name, v) },
        None => unsafe { std::env::remove_var(&name) },
    }
    // Mirror into `process-environment'.
    let pe = i.intern("process-environment");
    let cur = i.symbol_value(pe);
    let mut items = cur.list_to_vec().unwrap_or_default();
    let prefix = format!("{}=", name);
    items.retain(|v| match v {
        Value::Str(s) => !s.borrow().starts_with(&prefix),
        _ => true,
    });
    if let Some(v) = &val {
        items.insert(0, Value::string(format!("{}={}", name, v)));
    }
    let _ = i.set_symbol(pe, Value::list(items));
    Ok(match val {
        Some(v) => Value::string(v),
        None => Value::Nil,
    })
}

fn f_user_login_name(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let name = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    Ok(Value::string(name))
}

fn f_user_full_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = a;
    Ok(match std::env::var("NAME") {
        Ok(n) if !n.is_empty() => Value::string(n),
        _ => f_user_login_name(i, vec![])?,
    })
}

fn f_user_uid(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // No libc dep: the executable's owner uid is the effective uid in
    // the overwhelmingly common case; fall back to `id -u'.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(m) = std::fs::metadata(&exe) {
                return Ok(Value::Int(m.uid() as i128));
            }
        }
    }
    Ok(Value::Int(0))
}

fn f_system_groups(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match std::process::Command::new("id").arg("-Gn").output() {
        Ok(o) if o.status.success() => Ok(Value::list(
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .map(Value::string)
                .collect(),
        )),
        _ => Ok(Value::Nil),
    }
}

fn f_invocation_name(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let name = std::env::args()
        .next()
        .map(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(p)
        })
        .unwrap_or_else(|| "remacs".to_string());
    Ok(Value::string(name))
}

// ---------- version strings ----------

fn version_list_of(i: &mut Interp, v: &Value) -> Result<Vec<i128>, Flow> {
    let s = want_string(i, v)?;
    // Split on '.'; strip non-digit suffixes (e.g. "31.1.50" or "3.0rc1").
    let mut out = Vec::new();
    for part in s.split('.') {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        out.push(digits.parse().unwrap_or(0));
    }
    Ok(out)
}

fn f_version_to_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let v = version_list_of(i, &a[0])?;
    Ok(Value::list(v.into_iter().map(Value::Int).collect()))
}

fn version_cmp(a: &[i128], b: &[i128]) -> std::cmp::Ordering {
    let n = a.len().max(b.len());
    for k in 0..n {
        let x = a.get(k).copied().unwrap_or(0);
        let y = b.get(k).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

fn f_version_cmp(i: &mut Interp, a: &[Value]) -> Result<std::cmp::Ordering, Flow> {
    let (x, y) = match (&a[0], &a[1]) {
        (Value::Str(_), Value::Str(_)) => (version_list_of(i, &a[0])?, version_list_of(i, &a[1])?),
        _ => (
            a[0].list_to_vec()
                .unwrap_or_default()
                .iter()
                .map(|v| match v {
                    Value::Int(n) => *n,
                    _ => 0,
                })
                .collect(),
            a[1].list_to_vec()
                .unwrap_or_default()
                .iter()
                .map(|v| match v {
                    Value::Int(n) => *n,
                    _ => 0,
                })
                .collect(),
        ),
    };
    Ok(version_cmp(&x, &y))
}

fn f_version_lt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Less,
    ))
}
fn f_version_le(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? != std::cmp::Ordering::Greater,
    ))
}
fn f_version_eq(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Equal,
    ))
}
fn f_version_list_lt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_version_lt(i, a)
}
fn f_version_list_le(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_version_le(i, a)
}
fn f_version_list_eq(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_version_eq(i, a)
}
fn f_version_listp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ok = match &a[0] {
        Value::Nil => true,
        Value::Cons(_) => a[0]
            .list_to_vec()
            .map(|v| v.iter().all(|x| matches!(x, Value::Int(_))))
            .unwrap_or(false),
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

// ---------- predicates ----------

fn f_string_or_null_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &a[0],
        Value::Str(_) | Value::Nil
    )))
}

fn f_vector_or_char_table_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Vec(_))))
}

fn f_cl_type_of(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Int(_) => "fixnum",
        Value::Float(_) => "float",
        Value::Sym(_) | Value::Nil => "symbol",
        Value::Cons(_) => "cons",
        Value::Str(_) => "string",
        Value::Vec(_) => "vector",
        Value::Record(_) => "record",
        Value::Hash(_) => "hash-table",
        Value::Subr(_) => "subr",
        Value::Lambda(_) => "interpreted-function",
        Value::Buffer(_) => "buffer",
        Value::Marker(_) => "marker",
        Value::Window(_) => "window",
        Value::Frame(_) => "frame",
    };
    Ok(Value::Sym(i.intern(name)))
}

/// Wrap a Value in (quote v) for `call_function`'s unevaluated args.
fn quoted(v: Value) -> Value {
    Value::list(vec![Value::Sym(sym::QUOTE), v])
}

// ---------- bool vectors ----------
// Represented as a Record `#s(bool-vector [bits])' so `bool-vector-p'
// is exact while element access stays cheap.

pub(crate) fn is_bool_vector(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            matches!(rr.first(), Some(Value::Sym(t)) if i.symbol_name(*t) == "bool-vector")
                && matches!(rr.get(1), Some(Value::Vec(_)))
        }
        _ => false,
    }
}

pub(crate) fn bool_vec_of(i: &mut Interp, v: &Value) -> Result<Vec<bool>, Flow> {
    if !is_bool_vector(i, v) {
        return Err(i.wrong_type_mut("bool-vector-p", v));
    }
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let Some(Value::Vec(b)) = rr.get(1) {
            return Ok(b
                .borrow()
                .iter()
                .map(|x| matches!(x, Value::Int(n) if *n != 0))
                .collect());
        }
    }
    Err(i.wrong_type_mut("bool-vector-p", v))
}

pub(crate) fn make_bool_vector(i: &mut Interp, bits: Vec<bool>) -> Value {
    let data = Value::Vec(Rc::new(RefCell::new(
        bits.into_iter()
            .map(|b| Value::Int(if b { 1 } else { 0 }))
            .collect(),
    )));
    Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("bool-vector")),
        data,
    ])))
}

fn f_bool_vector_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_bool_vector(i, &a[0])))
}

fn f_make_bool_vector(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = super::want_int(i, &a[0])?;
    let init = !arg(&a, 1).is_nil();
    if n < 0 {
        let s = i.intern("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    Ok(make_bool_vector(i, vec![init; n as usize]))
}

fn f_bool_vector_length(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(b.len() as i128))
}

fn f_bool_vector_subsetp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let y = bool_vec_of(i, &a[1])?;
    if x.len() != y.len() {
        let s = i.intern("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    }
    Ok(Value::from_bool(
        x.iter().zip(&y).all(|(p, q)| !(*p && !*q)),
    ))
}

fn f_bool_vector_not(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let out: Vec<bool> = x.iter().map(|b| !*b).collect();
    let target = arg(&a, 1);
    if !target.is_nil() {
        if is_bool_vector(i, &target) {
            if let Value::Record(r) = &target {
                let rr = r.borrow();
                if let Some(Value::Vec(b)) = rr.get(1) {
                    let mut bb = b.borrow_mut();
                    if bb.len() == out.len() {
                        for (k, v) in bb.iter_mut().zip(&out) {
                            *k = Value::Int(if *v { 1 } else { 0 });
                        }
                        return Ok(target.clone());
                    }
                }
            }
        }
    }
    Ok(make_bool_vector(i, out))
}

fn bv_binop(i: &mut Interp, a: &[Value], f: fn(bool, bool) -> bool) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let y = bool_vec_of(i, &a[1])?;
    if x.len() != y.len() {
        let s = i.intern("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    }
    let out: Vec<bool> = x.iter().zip(&y).map(|(p, q)| f(*p, *q)).collect();
    let target = arg(a, 2);
    if !target.is_nil() && is_bool_vector(i, &target) {
        if let Value::Record(r) = &target {
            let rr = r.borrow();
            if let Some(Value::Vec(b)) = rr.get(1) {
                let mut bb = b.borrow_mut();
                if bb.len() == out.len() {
                    for (k, v) in bb.iter_mut().zip(&out) {
                        *k = Value::Int(if *v { 1 } else { 0 });
                    }
                    return Ok(target.clone());
                }
            }
        }
    }
    Ok(make_bool_vector(i, out))
}

fn f_bool_vector_bin(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    bv_binop(i, &a, |p, q| p != q)
}
fn f_bool_vector_union(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    bv_binop(i, &a, |p, q| p || q)
}
fn f_bool_vector_inter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    bv_binop(i, &a, |p, q| p && q)
}
fn f_bool_vector_diff(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    bv_binop(i, &a, |p, q| p && !q)
}

fn f_bool_vector_count(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(x.iter().filter(|b| **b).count() as i128))
}

fn f_bool_vector_consec(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let b = !a[1].is_nil();
    Ok(Value::Int(x.iter().take_while(|v| **v == b).count() as i128))
}

// ---------- events ----------

use crate::editor::{
    CHAR_ALT, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER, WindowRef, apply_mods,
    is_keymap, key_seq, parse_key_token, sel_frame, sel_window,
};

fn f_eventp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ok = match &a[0] {
        Value::Cons(c) => matches!(c.borrow().car, Value::Sym(_) | Value::Int(_)),
        Value::Int(_) | Value::Sym(_) => true,
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

/// Modifier symbols of an event: (click mouse-1), (control ?a), or
/// a symbol like `M-left` / int with modifier bits.
fn event_mod_list(i: &Interp, ev: &Value) -> Vec<String> {
    let mut mods = Vec::new();
    match ev {
        Value::Cons(c) => {
            let items = Value::Cons(c.clone()).list_to_vec().unwrap_or_default();
            // List event form is (EVENT-SYMBOL position-info...);
            // the car must carry event-symbol-elements.
            if let Some(Value::Sym(s)) = items.first() {
                if eventish(&i.symbol_name(*s)) {
                    for m in event_mod_list(i, &Value::Sym(*s)) {
                        mods.push(m);
                    }
                }
            }
        }
        Value::Int(n) => {
            for (bit, name) in [
                (CHAR_ALT, "alt"),
                (CHAR_CTL, "control"),
                (CHAR_HYPER, "hyper"),
                (CHAR_META, "meta"),
                (CHAR_SHIFT, "shift"),
                (CHAR_SUPER, "super"),
            ] {
                if n & bit != 0 {
                    mods.push(name.to_string());
                }
            }
            // Control chars 0-31 carry an implicit control modifier.
            if (0..=31).contains(n) {
                mods.push("control".to_string());
            }
        }
        Value::Sym(s) => {
            let mut name = i.symbol_name(*s).to_string();
            loop {
                let mut hit = false;
                for (p, m) in [
                    ("A-", "alt"),
                    ("C-", "control"),
                    ("H-", "hyper"),
                    ("M-", "meta"),
                    ("S-", "shift"),
                    ("s-", "super"),
                ] {
                    if let Some(r) = name.strip_prefix(p) {
                        mods.push(m.to_string());
                        name = r.to_string();
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    break;
                }
            }
            // Click-kind prefixes and bare mouse-N contribute kinds.
            let pre_len = mods.len();
            loop {
                let mut hit = false;
                for p in ["down-", "drag-", "double-", "triple-", "click-"] {
                    if let Some(r) = name.strip_prefix(p) {
                        mods.push(p.trim_end_matches('-').to_string());
                        name = r.to_string();
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    break;
                }
            }
            if name.starts_with("mouse-") && mods.len() == pre_len {
                mods.push("click".to_string());
            }
        }
        _ => {}
    }
    mods
}

fn f_event_modifiers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let mods = event_mod_list(i, &a[0]);
    Ok(Value::list(
        mods.iter().map(|m| Value::Sym(i.intern(m))).collect(),
    ))
}

fn event_basic(i: &mut Interp, ev: &Value) -> Value {
    match ev {
        Value::Cons(c) => {
            let items = Value::Cons(c.clone()).list_to_vec().unwrap_or_default();
            match items.first() {
                Some(Value::Sym(s)) if eventish(&i.symbol_name(*s)) => {
                    event_basic(i, &Value::Sym(*s))
                }
                _ => Value::Nil,
            }
        }
        Value::Int(n) => {
            let c = n & !(CHAR_ALT | CHAR_CTL | CHAR_HYPER | CHAR_META | CHAR_SHIFT | CHAR_SUPER);
            Value::Int(if (1..=26).contains(&c) {
                c + 96
            } else if (0..=31).contains(&c) {
                c + 64
            } else {
                c
            })
        }
        Value::Sym(s) => {
            let mut name = i.symbol_name(*s).to_string();
            loop {
                let mut hit = false;
                for p in [
                    "A-", "C-", "H-", "M-", "S-", "s-", "down-", "drag-", "double-", "triple-",
                    "click-",
                ] {
                    if let Some(r) = name.strip_prefix(p) {
                        name = r.to_string();
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    break;
                }
            }
            let stripped = name != i.symbol_name(*s);
            if stripped || eventish(&name) {
                Value::Sym(i.intern(&name))
            } else {
                Value::Nil
            }
        }
        _ => ev.clone(),
    }
}

/// Is NAME a key-event symbol (has event-symbol-elements in Emacs)?
fn eventish(name: &str) -> bool {
    const NAMED: &[&str] = &[
        "return",
        "tab",
        "escape",
        "space",
        "backspace",
        "delete",
        "deletechar",
        "home",
        "end",
        "left",
        "right",
        "up",
        "down",
        "prior",
        "next",
        "insert",
        "menu",
        "kanji",
        "redo",
        "undo",
        "clear",
        "insertchar",
        "deleteline",
        "insertline",
        "select",
        "print",
        "find",
        "execute",
        "help",
        "menu",
        "begin",
        "break",
        "pause",
        "printscreen",
        "scrollock",
        "numlock",
        "capslock",
    ];
    NAMED.contains(&name)
        || name.starts_with("mouse-")
        || name.starts_with("wheel-")
        || name.starts_with("kp-")
        || name.starts_with("iso-")
        || (name.starts_with('f')
            && name[1..].chars().all(|c| c.is_ascii_digit())
            && name.len() > 1)
}

fn f_event_basic_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(event_basic(i, &a[0]))
}

fn f_event_convert_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let items = a[0].list_to_vec().unwrap_or_default();
    if items.is_empty() {
        return Ok(Value::Nil);
    }
    let basic = event_basic(i, items.last().unwrap());
    let mut mods = 0i128;
    for m in &items[..items.len() - 1] {
        if let Value::Sym(s) = m {
            match i.symbol_name(*s).as_str() {
                "control" => mods |= CHAR_CTL,
                "meta" => mods |= CHAR_META,
                "shift" => mods |= CHAR_SHIFT,
                "hyper" => mods |= CHAR_HYPER,
                "super" => mods |= CHAR_SUPER,
                "alt" => mods |= CHAR_ALT,
                _ => {}
            }
        }
    }
    Ok(match basic {
        Value::Int(c) => Value::Int(apply_mods(c, mods)),
        Value::Sym(s) => {
            let mut prefix = String::new();
            for (bit, name) in [
                (CHAR_ALT, "A-"),
                (CHAR_CTL, "C-"),
                (CHAR_HYPER, "H-"),
                (CHAR_META, "M-"),
                (CHAR_SHIFT, "S-"),
                (CHAR_SUPER, "s-"),
            ] {
                if mods & bit != 0 {
                    prefix.push_str(name);
                }
            }
            Value::Sym(i.intern(&format!("{}{}", prefix, i.symbol_name(s))))
        }
        other => other,
    })
}

fn f_listify_key_sequence(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let keys = key_seq(i, &a[0])?;
    Ok(Value::list(keys.into_iter().map(Value::Int).collect()))
}

fn f_key_valid_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    if s.trim().is_empty() {
        return Ok(Value::Nil);
    }
    const NAMED: &[&str] = &[
        "ret",
        "return",
        "tab",
        "lfd",
        "spc",
        "space",
        "esc",
        "escape",
        "del",
        "nul",
        "backspace",
        "delete",
        "delchar",
        "deletechar",
        "home",
        "end",
        "left",
        "right",
        "up",
        "down",
        "prior",
        "pageup",
        "next",
        "pagedown",
        "insert",
    ];
    for tok in s.split(' ').filter(|t| !t.is_empty()) {
        // Strip modifiers.
        let mut rest = tok;
        loop {
            let r = rest
                .strip_prefix("C-")
                .or_else(|| rest.strip_prefix("M-"))
                .or_else(|| rest.strip_prefix("S-"))
                .or_else(|| rest.strip_prefix("H-"))
                .or_else(|| rest.strip_prefix("s-"))
                .or_else(|| rest.strip_prefix("A-"));
            match r {
                Some(r) => rest = r,
                None => break,
            }
        }
        let ok = rest.chars().count() == 1
            || (rest.starts_with('<') && rest.ends_with('>') && rest.len() > 2)
            || NAMED.contains(&rest.to_ascii_lowercase().as_str());
        if !ok || parse_key_token(i, tok).is_empty() {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

fn f_key_parse(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let mut out = Vec::new();
    for tok in s.split(' ').filter(|t| !t.is_empty()) {
        out.extend(parse_key_token(i, tok));
    }
    Ok(Value::Vec(Rc::new(RefCell::new(out))))
}

// ---------- misc ----------

fn parse_date_ymd(s: &str) -> Option<(i64, i64, i64)> {
    // Accept "YYYY-MM-DD" or "YYYY/MM/DD" (with optional trailing time).
    let date = s.split([' ', 'T']).next()?;
    let parts: Vec<&str> = date.split(['-', '/']).collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

fn f_days_between(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s1 = want_string(i, &a[0])?;
    let s2 = want_string(i, &a[1])?;
    match (parse_date_ymd(&s1), parse_date_ymd(&s2)) {
        (Some((y1, m1, d1)), Some((y2, m2, d2))) => Ok(Value::Int(
            (days_from_civil(y1 as i128, m1 as i128, d1 as i128)
                - days_from_civil(y2 as i128, m2 as i128, d2 as i128)) as i128,
        )),
        _ => Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid date")])),
    }
}

fn f_date_to_time(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[0])?;
    // Parse ISO date; RFC822 forms get a day-level approximation.
    if let Some((y, m, d)) = parse_date_ymd(&s) {
        let mut secs = days_from_civil(y as i128, m as i128, d as i128) * 86400;
        // Local midnight, like encode-time.
        secs -= local_tm(secs as i64).tm_gmtoff as i128;
        let hi = secs.div_euclid(65536);
        let lo = secs.rem_euclid(65536);
        return Ok(Value::list(vec![
            Value::Int(hi as i128),
            Value::Int(lo as i128),
        ]));
    }
    Ok(Value::Nil)
}

fn f_memory_limit(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Emacs fixnum range on 64-bit: 2^61 - 1.
    Ok(Value::Int((1i128 << 61) - 1))
}

fn f_help_function_arglist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match i.indirect_function_value(&a[0]) {
        Value::Lambda(l) => {
            let mut v: Vec<Value> = l.required.iter().map(|s| Value::Sym(*s)).collect();
            if !l.optional.is_empty() {
                v.push(Value::Sym(i.intern("&optional")));
                for o in &l.optional {
                    v.push(Value::Sym(o.sym));
                }
            }
            if let Some(r) = l.rest {
                v.push(Value::Sym(i.intern("&rest")));
                v.push(Value::Sym(r));
            }
            Ok(Value::list(v))
        }
        Value::Subr(s) => Ok(match s.arity {
            Arity::Range { min, max } => {
                let mut v = Vec::new();
                for k in 0..min {
                    v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                }
                if max > min {
                    v.push(Value::Sym(i.intern("&optional")));
                    for k in min..max {
                        v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                    }
                }
                Value::list(v)
            }
            Arity::Many { .. } | Arity::Unevalled => Value::list(vec![
                Value::Sym(i.intern("&rest")),
                Value::Sym(i.intern("args")),
            ]),
        }),
        _ => Ok(Value::Nil),
    }
}

fn f_function_documentation(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match i.indirect_function_value(&a[0]) {
        Value::Subr(s) if !s.doc.is_empty() => Ok(Value::string(s.doc)),
        _ => Ok(Value::Nil),
    }
}

// ---------- coding systems ----------

const CODING_SYSTEMS: &[&str] = &[
    "utf-8",
    "utf-8-unix",
    "utf-8-dos",
    "utf-8-mac",
    "no-conversion",
    "undecided",
    "undecided-unix",
    "raw-text",
    "iso-latin-1",
    "us-ascii",
];

fn coding_known(i: &Interp, v: &Value) -> Option<String> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    let base = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    if CODING_SYSTEMS.contains(&name.as_str()) || CODING_SYSTEMS.contains(&base) {
        Some(name)
    } else {
        None
    }
}

fn f_coding_system_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(
        CODING_SYSTEMS
            .iter()
            .map(|n| Value::Sym(i.intern(n)))
            .collect(),
    ))
}

fn f_coding_system_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(coding_known(i, &a[0]).is_some()))
}

fn f_check_coding_system(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match coding_known(i, &a[0]) {
        Some(_) => Ok(a[0].clone()),
        None => {
            let s = i.intern("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

fn f_coding_system_eol_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return Ok(Value::Int(0)),
    };
    Ok(Value::Int(if n.ends_with("-dos") {
        1
    } else if n.ends_with("-mac") {
        2
    } else {
        0
    }))
}

fn f_coding_system_aliases(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match coding_known(i, &a[0]) {
        Some(n) => Ok(Value::list(vec![Value::Sym(i.intern(&n))])),
        None => Ok(Value::Nil),
    }
}

fn f_coding_system_base(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match coding_known(i, &a[0]) {
        Some(n) => {
            let base = n
                .strip_suffix("-unix")
                .or_else(|| n.strip_suffix("-dos"))
                .or_else(|| n.strip_suffix("-mac"))
                .unwrap_or(&n);
            Ok(Value::Sym(i.intern(base)))
        }
        None => Ok(a[0].clone()),
    }
}

fn f_coding_system_plist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match coding_known(i, &a[0]) {
        Some(n) => Ok(Value::list(vec![
            Value::Sym(i.intern(":name")),
            Value::Sym(i.intern(&n)),
            Value::Sym(i.intern(":coding-type")),
            Value::Sym(i.intern(if n.starts_with("utf-8") {
                "utf-8"
            } else {
                "charset"
            })),
        ])),
        None => {
            let s = i.intern("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

fn f_coding_system_get(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prop = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return Ok(Value::Nil),
    };
    match (coding_known(i, &a[0]), prop.as_str()) {
        (Some(n), ":name") => Ok(Value::Sym(i.intern(&n))),
        (Some(n), ":coding-type") => Ok(Value::Sym(i.intern(if n.starts_with("utf-8") {
            "utf-8"
        } else {
            "charset"
        }))),
        (Some(_), ":eol-type") => f_coding_system_eol_type(i, a),
        _ => Ok(Value::Nil),
    }
}

fn f_coding_system_put(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(a[2].clone())
}

fn f_terminal_coding_system(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("utf-8")))
}

fn f_detect_coding_string(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![Value::Sym(i.intern("undecided"))]))
}

fn f_detect_coding_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_detect_coding_string(i, a)
}

fn f_encode_coding_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_check_coding_system(i, vec![a[0].clone()])?;
    Ok(a[1].clone())
}

fn f_decode_coding_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(a[1].clone())
}

fn f_encode_coding_char(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_decode_coding_region(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_encode_coding_region(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

// ---------- multibyte ----------

fn f_identity(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a[0].clone())
}

// ---------- display / frames ----------

fn f_t(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}
fn f_not_useful(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("not-useful")))
}
fn f_zero(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

fn f_frame_configuration_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Cons(_))))
}

fn f_current_frame_configuration(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => Value::Nil,
    };
    Ok(Value::list(vec![fr]))
}

fn f_mouse_position(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => Value::Nil,
    };
    // Emacs: (FRAME nil) on a tty with no mouse.
    Ok(Value::list(vec![fr, Value::Nil]))
}

fn f_display_pixel_width(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // TTY: "pixels" are char cells (Emacs reports frame width).
    let w = sel_frame(i).map(|f| f.borrow().width).unwrap_or(80);
    Ok(Value::Int(w as i128))
}

fn f_display_pixel_height(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let h = sel_frame(i).map(|f| f.borrow().height).unwrap_or(24);
    Ok(Value::Int(h as i128))
}

fn f_display_mm(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_display_visual_class(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("static-gray")))
}

fn f_display_planes(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(3))
}

fn f_display_color_cells(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

fn f_color_defined_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(matches!(&a[0], Value::Str(_))))
}

fn f_color_gray_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    let ok = match &a[0] {
        Value::Str(s) => {
            let s = s.borrow().to_ascii_lowercase();
            s == "gray" || s == "grey" || s == "black" || s == "white"
        }
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

// ---------- windows ----------

fn win_dims(i: &Interp, v: &Value) -> (usize, usize) {
    let w = match v {
        Value::Window(w) => Some(w.clone()),
        _ => sel_window(i),
    };
    w.map(|w| {
        let w = w.borrow();
        (w.width, w.height)
    })
    .unwrap_or((80, 24))
}

fn f_window_min_height(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(4))
}
fn f_window_min_width(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(10))
}

fn f_window_sizable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let delta = match arg(&a, 1) {
        Value::Int(n) => n,
        _ => return Ok(Value::Nil),
    };
    let horiz = !arg(&a, 2).is_nil();
    let (w, h) = win_dims(i, &a[0]);
    let lim = if horiz { 10 } else { 4 };
    let dim = if horiz { w } else { h };
    Ok(Value::from_bool((dim as i128 + delta) >= lim as i128))
}

fn f_window_max_chars(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (w, _) = win_dims(i, &a[0]);
    Ok(Value::Int(w as i128))
}

fn f_pos_visible(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = match arg(&a, 0) {
        Value::Int(n) => n as usize,
        _ => return Ok(Value::Nil),
    };
    let w = match arg(&a, 1) {
        Value::Window(w) => Some(w),
        _ => sel_window(i),
    };
    let ok = match w {
        Some(w) => {
            let w = w.borrow();
            pos >= w.start
        }
        None => true,
    };
    Ok(Value::from_bool(ok))
}

fn f_window_line(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

fn f_selected_window(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_window_in_direction(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = &a;
    f_selected_window(i, vec![])
}

fn f_window_normalize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Window(_) => Ok(a[0].clone()),
        Value::Nil => f_selected_window(i, vec![]),
        _ => Err(i.wrong_type_mut("window-live-p", &a[0])),
    }
}

fn f_window_norm_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil => Ok(i
            .current_buffer_ref()
            .map(Value::Buffer)
            .unwrap_or(Value::Nil)),
        other => Ok(other.clone()),
    }
}

fn f_window_norm_frame(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Frame(_) => Ok(a[0].clone()),
        _ => match sel_frame(i) {
            Some(f) => Ok(Value::Frame(f)),
            None => Ok(Value::Nil),
        },
    }
}

fn f_get_window_pred(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pred = a[0].clone();
    if let Some(f) = sel_frame(i) {
        let wins: Vec<WindowRef> = f.borrow().windows.clone();
        for w in wins {
            let wv = Value::Window(w.clone());
            let r = i.call_function(&pred, &Value::list(vec![quoted(wv.clone())]), None)?;
            if !r.is_nil() {
                return Ok(wv);
            }
        }
    }
    Ok(Value::Nil)
}

// ---------- keymaps ----------

fn f_make_composed_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Args may be keymaps or a single list of keymaps.
    let mut parents = Vec::new();
    for v in &a {
        if is_keymap(i, v) {
            parents.push(v.clone());
        } else if let Value::Cons(_) = v {
            for e in v.list_to_vec().unwrap_or_default() {
                if is_keymap(i, &e) {
                    parents.push(e);
                }
            }
        }
    }
    if parents.is_empty() {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // Emacs shape: (keymap P1 . P2-chain) — a two-parent composed map
    // prints as (keymap (keymap) keymap) when both are empty.
    let mut tail = Value::Nil;
    for p in parents.iter().skip(1).rev() {
        tail = p.clone();
        break;
    }
    let _ = tail;
    let cdr = if parents.len() > 1 {
        Value::cons(parents[0].clone(), parents[1].clone())
    } else {
        Value::list(vec![parents[0].clone()])
    };
    Ok(Value::cons(Value::Sym(i.intern("keymap")), cdr))
}

fn f_current_active_maps(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let mut maps = Vec::new();
    let gid = i.intern("global-map");
    let global = i.symbol_value(gid);
    if is_keymap(i, &global) {
        maps.push(global);
    }
    Ok(Value::list(maps))
}

fn f_set_transient_map(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    let sid = i.intern("overriding-terminal-local-map");
    let _ = i.set_symbol(sid, a[0].clone());
    Ok(Value::Nil)
}

// ---------- char tables (vec-approximated like seq::make-char-table)

/// The slot vector of a char-table (Record form or legacy bare Vec).
fn char_table_vec(v: &Value) -> Option<Rc<RefCell<Vec<Value>>>> {
    match v {
        Value::Vec(v) => Some(v.clone()),
        Value::Record(r) => {
            let rr = r.borrow();
            match rr.get(2) {
                Some(Value::Vec(v)) => Some(v.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_char_table(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            matches!(rr.first(), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-table")
                && matches!(rr.get(2), Some(Value::Vec(_)))
        }
        _ => false,
    }
}

fn char_table_subtype_of(v: &Value) -> Value {
    match v {
        Value::Record(r) => r.borrow().get(1).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

fn f_char_table_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_char_table(i, &a[0])))
}

fn f_char_table_range(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = char_table_vec(&a[0]) {
        let idx = match &a[1] {
            Value::Int(n) if *n >= 0 => *n as usize,
            Value::Nil => 0,
            _ => return Ok(v.borrow().first().cloned().unwrap_or(Value::Nil)),
        };
        Ok(v.borrow().get(idx).cloned().unwrap_or(Value::Nil))
    } else {
        Ok(Value::Nil)
    }
}

fn f_set_char_table_range(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = char_table_vec(&a[0]) {
        let (lo, hi) = match &a[1] {
            Value::Int(n) => (*n as usize, *n as usize),
            Value::Nil | Value::Cons(_) => (0, 255),
            Value::Sym(s) if i.symbol_name(*s) == "t" => (0, 255),
            _ => return Ok(Value::Nil),
        };
        let mut vv = v.borrow_mut();
        for k in lo..=hi.min(vv.len().saturating_sub(1)) {
            if k < vv.len() {
                vv[k] = a[2].clone();
            }
        }
    }
    Ok(Value::Nil)
}

fn f_map_char_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = char_table_vec(&a[1]) {
        let items: Vec<Value> = v.borrow().clone();
        for (k, val) in items.iter().enumerate() {
            i.call_function(
                &a[0],
                &Value::list(vec![quoted(Value::Int(k as i128)), quoted(val.clone())]),
                None,
            )?;
        }
    }
    Ok(Value::Nil)
}

fn f_suppress_keymap(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // Emacs returns nil after rebinding printable chars to `undefined'.
    Ok(Value::Nil)
}

fn f_char_table_subtype(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(char_table_subtype_of(&a[0]))
}

fn f_record(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::Record(Rc::new(RefCell::new(a))))
}

fn f_recordp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(a[0], Value::Record(_))))
}
