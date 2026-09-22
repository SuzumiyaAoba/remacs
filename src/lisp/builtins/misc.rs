//! Miscellaneous subrs: symbols, time values, hashing/crypto, file
//! attributes, environment, and small editor glue that doesn't belong
//! to a larger category module.

use std::cell::RefCell;
use std::rc::Rc;

use super::{S, arg, want_int, want_list, want_string, want_sym};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Arity, Lambda, Subr, SymId, Value};

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
        "load-average",
        0,
        1,
        f_load_average,
        "System load averages."
    ),
    S!("daemonp", 0, 0, f_nil, "t when running as a daemon."),
    S!("invocation-name", 0, 0, f_invocation_name, "Program name."),
    S!(
        "invocation-directory",
        0,
        0,
        f_invocation_dir,
        "Program directory."
    ),
    S!(
        "internal--build-binding",
        2,
        3,
        f_build_binding,
        "Make a binding object."
    ),
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
    // `file-acl', `lock-file', `unlock-file' have real
    // implementations in buffer/primitives.rs (registered later).
    S!(
        "get-file-buffer",
        1,
        1,
        f_get_file_buffer,
        "Buffer visiting FILENAME."
    ),
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
        f_symbol_with_pos_p,
        "t if OBJECT is a positioned symbol."
    ),
    S!(
        "symbol-with-pos-pos",
        1,
        1,
        f_symbol_with_pos_pos,
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
    S!("threadp", 1, 1, f_threadp, "t if OBJECT is a thread."),
    S!("all-threads", 0, 0, f_all_threads, "List of all threads."),
    S!(
        "current-thread",
        0,
        0,
        f_current_thread,
        "The currently running thread."
    ),
    S!("thread-name", 1, 1, f_thread_name, "Name of THREAD."),
    S!(
        "thread-live-p",
        1,
        1,
        f_thread_live_p,
        "t if THREAD is alive (not yet joined)."
    ),
    S!(
        "make-thread",
        1,
        2,
        f_make_thread,
        "Run FUNCTION in a new thread named NAME."
    ),
    S!(
        "thread-join",
        1,
        1,
        f_thread_join,
        "Wait for THREAD and return its result."
    ),
    S!("thread-yield", 0, 0, f_nil, "Yield to other threads."),
    S!(
        "thread-last-error",
        0,
        0,
        f_thread_last_error,
        "Last error form recorded by a thread."
    ),
    S!("thread--blocker", 1, 1, f_thread_blocker, ""),
    S!("thread-signal", 3, 3, f_thread_signal, ""),
    S!("mutexp", 1, 1, f_mutexp, "t if OBJECT is a mutex."),
    S!("make-mutex", 0, 1, f_make_mutex, "Create a mutex."),
    S!("mutex-name", 1, 1, f_mutex_name, "Name of MUTEX."),
    S!("mutex-lock", 1, 1, f_mutex_lock, "Lock MUTEX."),
    S!("mutex-unlock", 1, 1, f_mutex_unlock, "Unlock MUTEX."),
    S!(
        "condition-variable-p",
        1,
        1,
        f_condition_variable_p,
        "t if OBJECT is a condition variable."
    ),
    S!(
        "make-condition-variable",
        1,
        2,
        f_make_condition_variable,
        "Create a condition variable on MUTEX."
    ),
    S!(
        "condition-name",
        1,
        1,
        f_condition_name,
        "Name of CONDVAR."
    ),
    S!(
        "condition-mutex",
        1,
        1,
        f_condition_mutex,
        "Mutex associated with CONDVAR."
    ),
    S!(
        "condition-wait",
        1,
        1,
        f_condition_wait,
        "Wait on CONDVAR (cooperative: returns immediately)."
    ),
    S!(
        "condition-notify",
        1,
        2,
        f_condition_notify,
        "Notify waiters on CONDVAR."
    ),
    S!(
        "make-finalizer",
        1,
        1,
        f_make_finalizer,
        "Create a finalizer calling FUNCTION."
    ),
    S!("byte-to-string", 1, 1, f_byte_to_string, "Byte to string."),
    S!(
        "get-load-suffixes",
        0,
        0,
        f_get_load_suffixes,
        "Suffixes tried by `load'."
    ),
    S!(
        "num-processors",
        0,
        0,
        f_num_processors,
        "Number of available processors."
    ),
    S!(
        "daemon-initialized",
        0,
        0,
        f_daemon_initialized,
        "Error unless running as a daemon."
    ),
    S!("signal-names", 0, 0, f_signal_names, "POSIX signal names."),
    S!("user-ptrp", 1, 1, f_false, "t if OBJECT is a user pointer."),
    S!("group-name", 1, 1, f_group_name, "Group name for GID."),
    S!(
        "waiting-for-user-input-p",
        0,
        0,
        f_nil,
        "t while waiting for user input."
    ),
    S!("bitmap-spec-p", 1, 1, f_false, "t if OBJECT is a bitmap spec."),
    S!(
        "get-truename-buffer",
        1,
        1,
        f_get_truename_buffer,
        "Return the buffer visiting the truename of FILENAME."
    ),
    S!(
        "unencodable-char-position",
        3,
        5,
        f_unencodable_char_position,
        "Position of first unencodable char in a region."
    ),
    S!(
        "compose-region-internal",
        2,
        4,
        f_compose_region_internal,
        "Internal function for `compose-region'."
    ),
    S!(
        "compose-string-internal",
        3,
        5,
        f_compose_string_internal,
        "Internal function for `compose-string'."
    ),
    S!(
        "set-buffer-redisplay",
        4,
        4,
        f_set_buffer_redisplay,
        "Set redisplay flags for BUFFER's region."
    ),
    S!(
        "delete-other-windows-internal",
        0,
        2,
        f_nil,
        "Delete all windows except WINDOW in ROOT."
    ),
    S!(
        "register-ccl-program",
        2,
        2,
        f_register_ccl_program,
        "Register CCL program CCL-PROG as NAME."
    ),
    S!(
        "ccl-program-p",
        1,
        1,
        f_ccl_program_p,
        "t if NAME is a registered CCL program."
    ),
    S!(
        "ccl-execute",
        2,
        2,
        f_ccl_execute,
        "Execute registered CCL-PROG with registers STATUS."
    ),
    S!(
        "ccl-execute-on-string",
        3,
        4,
        f_ccl_execute_on_string,
        "Execute CCL-PROG on STRING with registers STATUS."
    ),
    S!(
        "register-code-conversion-map",
        2,
        2,
        f_register_code_conversion_map,
        "Register MAP as code conversion map NAME."
    ),
    S!("zlib-available-p", 0, 0, f_zlib_available_p, "t if zlib decompression is available."),
    S!(
        "zlib-decompress-region",
        2,
        3,
        f_zlib_decompress_region,
        "Decompress the region as gzip or zlib data."
    ),
    S!(
        "find-buffer",
        2,
        2,
        f_find_buffer,
        "Return the buffer with buffer-local VARIABLE `equal' to VALUE."
    ),
    S!("insert-byte", 2, 3, f_insert_byte, "Insert COUNT copies of BYTE."),
    S!("set-quit-char", 1, 1, f_nil, "Set terminal quit char."),
    S!(
        "set-binary-mode",
        2,
        2,
        f_set_binary_mode,
        "Switch STREAM into binary or text MODE."
    ),
    S!(
        "set-output-flow-control",
        1,
        2,
        f_set_output_flow_control,
        "Enable flow control on TERMINAL."
    ),
    S!(
        "newline-cache-check",
        0,
        1,
        f_nil,
        "Check the newline cache for sanity."
    ),
    S!("tab-bar-height", 0, 2, f_tab_bar_height, "Height of the tab bar."),
    S!(
        "insert-special-event",
        1,
        1,
        f_insert_special_event,
        "Insert EVENT into the input queue."
    ),
    S!(
        "buffer-text-pixel-size",
        0,
        4,
        f_buffer_text_pixel_size,
        "Size of the buffer text in pixels."
    ),
    S!(
        "format-mode-line",
        1,
        4,
        f_format_mode_line,
        "Format a string using the mode line format."
    ),
    S!(
        "debugger-trap",
        0,
        0,
        f_nil,
        "Trap into the debugger."
    ),
    S!(
        "make-category-table",
        0,
        0,
        f_make_category_table,
        "Create a fresh category table."
    ),
    S!(
        "category-table-p",
        1,
        1,
        f_category_table_p,
        "t if OBJECT is a category table."
    ),
    S!(
        "standard-category-table",
        0,
        0,
        f_standard_category_table,
        "Return the standard category table."
    ),
    S!(
        "category-table",
        0,
        0,
        f_category_table,
        "Return the current buffer's category table."
    ),
    S!(
        "set-category-table",
        1,
        1,
        f_set_category_table,
        "Select TABLE as the current buffer's category table."
    ),
    S!(
        "copy-category-table",
        0,
        1,
        f_copy_category_table,
        "Copy TABLE (default: current) and return the copy."
    ),
    S!(
        "define-category",
        2,
        3,
        f_define_category,
        "Define CATEGORY as a category with DOCSTRING in TABLE."
    ),
    S!(
        "category-docstring",
        1,
        2,
        f_category_docstring,
        "Return the docstring of CATEGORY in TABLE."
    ),
    S!(
        "get-unused-category",
        0,
        1,
        f_get_unused_category,
        "Return a still-unused category label in TABLE."
    ),
    S!(
        "modify-category-entry",
        2,
        4,
        f_modify_category_entry,
        "Add CATEGORY to the category set of CHAR in TABLE."
    ),
    S!(
        "char-category-set",
        1,
        1,
        f_char_category_set,
        "Return the category set of CH in the current table."
    ),
    S!(
        "category-set-mnemonics",
        1,
        1,
        f_category_set_mnemonics,
        "Return a string of category labels present in CATEGORY-SET."
    ),
    S!(
        "make-category-set",
        1,
        1,
        f_make_category_set,
        "Make a category set from a mnemonic string."
    ),
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
        1,
        f_bool_vector_count,
        ""
    ),
    S!(
        "bool-vector-count-consecutive",
        3,
        3,
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
    S!("help-function-arglist", 1, 2, f_help_function_arglist, ""),
    S!("function-documentation", 1, 1, f_function_documentation, ""),
    S!("command-error-default-function", 3, 3, f_command_error_default, ""),
    S!("command-line", 0, 0, f_command_line, ""),
    S!("recursion-depth", 0, 0, f_zero, ""),
    S!("minibuffer-depth", 0, 0, f_zero, ""),
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
    S!(
        "check-coding-systems-region",
        3,
        3,
        f_check_coding_systems_region,
        ""
    ),
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
    S!(
        "display-monitor-attributes-list",
        0,
        1,
        f_display_monitor_attributes_list,
        ""
    ),
    S!("tool-bar-height", 0, 2, f_zero, ""),
    S!("tool-bar-pixel-width", 0, 1, f_zero, ""),
    S!(
        "frame-monitor-attributes",
        0,
        1,
        f_frame_monitor_attributes,
        ""
    ),
    // `display-mm-dimensions-alist' is a variable in GNU (nil in batch).
    S!("x-open-connection", 1, 2, f_nil, ""),
    S!("x-close-connection", 1, 1, f_nil, ""),
    S!("x-display-list", 0, 0, f_nil, ""),
    S!("xw-display-color-p", 0, 1, f_ns_display, ""),
    S!("xw-color-defined-p", 1, 2, f_false, ""),
    S!("color-gray-p", 1, 2, f_color_gray_p, ""),
    S!("color-supported-p", 1, 2, f_color_defined_p, ""),
    S!("invert-face", 1, 2, f_invert_face, ""),
    S!("clear-face-cache", 0, 1, f_nil, ""),
    // ---------- windows ----------
    S!("minibuffer-selected-window", 0, 0, f_nil, ""),
    S!("window-min-height", 0, 0, f_window_min_height, ""),
    S!("window-min-width", 0, 0, f_window_min_width, ""),
    S!("window-sizable", 1, 3, f_window_sizable, ""),
    S!("window-fixed-size-p", 0, 2, f_windowp_nil, ""),
    S!("fit-window-to-buffer", 0, 4, f_nil, ""),
    S!("shrink-window-if-larger-than-buffer", 0, 1, f_nil, ""),
    S!("window-safely-shrinkable-p", 0, 1, f_safely_shrinkable, ""),
    S!("window--display-buffer", 3, 4, f_nil, ""),
    S!("window-max-chars-per-line", 0, 2, f_window_max_chars, ""),
    S!("window-preserve-size", 0, 3, f_window_preserve_size, ""),
    S!("window-left-column", 0, 1, f_zero, ""),
    S!("pos-visible-in-window-group-p", 0, 3, f_pos_visible, ""),
    S!("window-line", 0, 1, f_window_line, ""),
    S!("window-normalize-window", 1, 1, f_window_normalize, ""),
    S!("window-normalize-buffer", 1, 1, f_window_norm_buffer, ""),
    S!("window-normalize-frame", 0, 1, f_window_norm_frame, ""),
    S!("delete-windows-on", 0, 3, f_nil, ""),
    // `split-window-sensibly' is Lisp (GNU window.el) — see prelude.
    S!("window-child", 1, 1, f_window_valid_nil, ""),
    S!("window-child-count", 1, 1, f_window_valid_zero, ""),
    S!("window-combined-p", 0, 2, f_window_combined_p, ""),
    // `window-leftmost-p'/`-rightmost-p'/`-topmost-p'/`-bottommost-p'
    // do not exist in GNU.
    S!("window-at-side-p", 1, 2, f_t, ""),
    S!("window-in-direction", 1, 5, f_window_in_direction, ""),
    S!("window-main-window", 0, 1, f_window_main_window, ""),
    S!("get-mru-window", 0, 2, f_selected_window, ""),
    S!("get-window-with-predicate", 1, 3, f_get_window_pred, ""),
    // ---------- keymap ops ----------
    S!("suppress-keymap", 1, 2, f_suppress_keymap, ""),
    S!("make-composed-keymap", 1, 2, f_make_composed_keymap, ""),
    S!("current-active-maps", 0, 2, f_current_active_maps, ""),
    S!("keymap-canonicalize", 1, 1, f_keymap_canonicalize, ""),
    S!("set-transient-map", 1, 3, f_set_transient_map, ""),
    // `text-mode-map' is a variable (keymap) in GNU, not a subr.
    // ---------- tables ----------
    // `buffer-display-table' is a buffer-local variable in GNU.
    S!("char-table-extra-slot", 2, 2, f_char_table_extra_slot, ""),
    S!(
        "set-char-table-extra-slot",
        3,
        3,
        f_set_char_table_extra_slot,
        ""
    ),
    S!("char-table-range", 2, 2, f_char_table_range, ""),
    S!("set-char-table-range", 3, 3, f_set_char_table_range, ""),
    S!("char-table-parent", 1, 1, f_char_table_parent, ""),
    S!(
        "set-char-table-parent",
        2,
        2,
        f_set_char_table_parent,
        ""
    ),
    S!("map-char-table", 2, 2, f_map_char_table, ""),
    S!("optimize-char-table", 1, 2, f_optimize_char_table, ""),
    S!("char-table-subtype", 1, 1, f_char_table_subtype, ""),
    S!("char-table-p", 1, 1, f_char_table_p, ""),
    // ---------- GNU subrs present on a terminal build ----------
    S!(
        "length<",
        2,
        2,
        f_length_lt,
        "Is SEQUENCE shorter than LENGTH?"
    ),
    S!(
        "length>",
        2,
        2,
        f_length_gt,
        "Is SEQUENCE longer than LENGTH?"
    ),
    S!("length=", 2, 2, f_length_eq, "Is SEQUENCE exactly LENGTH?"),
    S!(
        "value<",
        2,
        2,
        f_value_lt,
        "Is A less than B (internal order)?"
    ),
    S!(
        "seconds-to-time",
        1,
        1,
        f_seconds_to_time,
        "SECS as a time value."
    ),
    S!(
        "time-since",
        1,
        1,
        f_time_since,
        "Seconds elapsed since TIME."
    ),
    S!(
        "time-to-days",
        1,
        1,
        f_time_to_days,
        "Days since epoch of TIME."
    ),
    S!(
        "time-to-day-in-year",
        1,
        1,
        f_time_to_day_in_year,
        "Day of year of TIME."
    ),
    S!(
        "days-to-time",
        1,
        1,
        f_days_to_time,
        "DAYS as a time value."
    ),
    S!(
        "date-leap-year-p",
        1,
        1,
        f_date_leap_year_p,
        "Is YEAR a leap year?"
    ),
    S!(
        "version-list-not-zero",
        1,
        1,
        f_version_list_not_zero,
        "Drop leading zero components."
    ),
    S!(
        "memory-use-counts",
        0,
        0,
        f_memory_use_counts,
        "Object counts."
    ),
    S!("group-gid", 0, 0, f_group_gid, "Effective group id."),
    S!("group-real-gid", 0, 0, f_group_real_gid, "Real group id."),
    S!("system-users", 0, 0, f_system_users, "List of user names."),
    S!(
        "bufferpos-to-filepos",
        1,
        2,
        f_bufferpos_to_filepos,
        "Char POSITION to byte offset."
    ),
    S!(
        "filepos-to-bufferpos",
        1,
        2,
        f_filepos_to_bufferpos,
        "Byte offset to char position."
    ),
    S!(
        "find-buffer-visiting",
        1,
        2,
        f_get_file_buffer,
        "Buffer visiting FILENAME."
    ),
    S!(
        "set-buffer-multibyte",
        1,
        1,
        f_set_buffer_multibyte,
        "Set the multibyte flag."
    ),
    S!(
        "window-with-parameter",
        1,
        3,
        f_window_with_parameter,
        "Window whose PARAMETER is VALUE."
    ),
    S!("message-box", many 1, f_message_box, "Like `message'."),
    S!("message-or-box", many 1, f_message_box, "Like `message'."),
    S!(
        "secure-hash-algorithms",
        0,
        0,
        f_secure_hash_algorithms,
        "List of hash algorithm names."
    ),
    S!("primitive-function-p", 1, 1, f_primitive_function_p, ""),
    S!("setenv-internal", 2, 3, f_setenv, ""),
    S!(
        "read--expression",
        0,
        2,
        f_read_expression,
        "Read one form."
    ),
    S!("read-positioning-symbols", 0, 1, f_read_positioning_symbols, ""),
    S!("describe-vector", 1, 2, f_describe_vector, ""),
    S!("locale-info", 1, 1, f_locale_info, "Locale data for ITEM."),
    S!("locale-translate", 1, 1, f_identity, ""),
    S!("mapbacktrace", 1, 2, f_mapbacktrace, ""),
    // `internal-timer-start-idle' is Lisp (prelude timer.el port).
    S!("internal-describe-syntax-value", 1, 1, f_identity, ""),
    S!("internal-copy-lisp-face", 4, 4, f_nil, ""),
    S!("internal-make-lisp-face", 1, 2, f_nil, ""),
    S!(
        "frame-or-buffer-changed-p",
        0,
        1,
        f_frame_or_buffer_changed_p,
        ""
    ),
    S!("scroll-bar-scale", 2, 2, f_nil, ""),
    S!("popup-menu", 1, 2, f_nil, ""),
    S!("set-frame-font", 1, 3, f_nil, ""),
    S!("set-keyboard-coding-system", 1, 2, f_nil, ""),
    S!("set-terminal-coding-system", 1, 2, f_nil, ""),
    S!("set-mouse-absolute-pixel-position", 2, 2, f_nil, ""),
    S!("tooltip-mode", 0, 1, f_tooltip_mode, ""),
    S!("keymap-of", 1, 1, f_keymap_of, ""),
    // ---------- display/font/image stubs (no GUI) ----------
    S!("default-font-width", 0, 0, f_one, "Char cell width."),
    S!("default-font-height", 0, 0, f_one, "Char cell height."),
    S!("window-font-width", 0, 1, f_one, "Char cell width."),
    S!("window-font-height", 0, 1, f_one, "Char cell height."),
    S!("color-distance", 2, 4, f_color_distance, "RGB distance."),
    S!(
        "frame-geometry",
        0,
        1,
        f_nil,
        "Frame geometry; nil on a tty."
    ),
    S!("frame-inner-width", 0, 1, f_frame_width_val, ""),
    S!("frame-inner-height", 0, 1, f_frame_height_val, ""),
    S!("frame-outer-width", 0, 1, f_frame_width_val, ""),
    S!("frame-outer-height", 0, 1, f_frame_height_val, ""),
    S!("glyph-char", 1, 1, f_glyph_char, ""),
    S!("glyph-face", 1, 1, f_glyph_face, ""),
    S!("font-at", 1, 3, f_nil, ""),
    S!("font-get-glyphs", 3, 4, f_nil, ""),
    S!("font-info", 1, 2, f_nil, ""),
    S!("font-match-p", 2, 2, f_nil, ""),
    S!("font-family-list", 0, 1, f_nil, ""),
    S!("font-face-attributes", 1, 2, f_nil, ""),
    S!("font-spec", many 0, f_font_spec, ""),
    S!("face-font", 1, 2, f_face_font, ""),
    S!("face-documentation", 1, 1, f_nil, ""),
    S!("face-attributes-as-vector", 1, 1, f_face_attributes_as_vector, ""),
    S!("image-flush", 1, 2, f_nil, ""),
    S!("image-mask-p", 1, 2, f_nil, ""),
    S!("image-metadata", 1, 2, f_nil, ""),
    S!("image-size", 1, 3, f_nil, ""),
    S!("image-transforms-p", 0, 1, f_image_transforms_p, ""),
    S!("image-type", 1, 3, f_image_type, ""),
    S!("image-type-available-p", 1, 2, f_image_type_available_p, ""),
    S!("init-image-library", 1, 1, f_nil, ""),
    S!("put-image", 2, 4, f_put_image, ""),
    S!("remove-images", 2, 3, f_nil, ""),
    S!("display-popup-menus-p", 0, 1, f_nil, ""),
    S!("display-screens", 0, 1, f_display_screens, ""),
    S!("display-selections-p", 0, 1, f_nil, ""),
    // ---------- X stubs (no X) ----------
    S!("gui-get-selection", 0, 3, f_nil, ""),
    S!("gui-set-selection", 2, 2, f_arg1, ""),
    S!("x-begin-drag", 1, 4, f_nil, ""),
    S!("x-display-backing-store", 0, 1, f_ns_display, ""),
    S!("x-display-color-cells", 0, 1, f_ns_display, ""),
    S!("x-display-grayscale-p", 0, 1, f_ns_display, ""),
    S!("x-display-mm-height", 0, 1, f_ns_display, ""),
    S!("x-display-mm-width", 0, 1, f_ns_display, ""),
    S!("x-display-pixel-height", 0, 1, f_ns_display, ""),
    S!("x-display-pixel-width", 0, 1, f_ns_display, ""),
    S!("x-display-planes", 0, 1, f_ns_display, ""),
    S!("x-display-save-under", 0, 1, f_ns_display, ""),
    S!("x-display-screens", 0, 1, f_ns_display, ""),
    S!("x-display-visual-class", 0, 1, f_ns_display, ""),
    S!("x-get-clipboard", 0, 0, f_nil, ""),
    S!("x-get-resource", 2, 4, f_nil, ""),
    S!("x-get-selection", 0, 2, f_nil, ""),
    S!("x-hide-tip", 0, 0, f_nil, ""),
    S!(
        "x-parse-geometry",
        1,
        1,
        f_x_parse_geometry,
        "Parse GEOMETRY."
    ),
    S!("x-server-max-request-size", 0, 1, f_ns_display, ""),
    S!("x-server-vendor", 0, 1, f_ns_display, ""),
    S!("x-server-version", 0, 1, f_ns_display, ""),
    S!("x-set-selection", 2, 2, f_arg1, ""),
    S!("x-show-tip", 1, 6, f_nil, ""),
    // ---------- optional-library availability ----------
    S!("gnutls-available-p", 0, 0, f_nil, ""),
    S!("sqlite-available-p", 0, 0, f_nil, ""),
    S!("libxml-available-p", 0, 0, f_nil, ""),
    S!("treesit-available-p", 0, 0, f_nil, ""),
    S!("imagep", 1, 1, f_nil, ""),
    S!("long-line-optimizations-p", 0, 0, f_nil, ""),
    // ---------- input/display mode internals ----------
    S!("current-input-mode", 0, 0, f_current_input_mode, ""),
    S!("set-input-mode", 3, 4, f_nil, ""),
    S!("set-input-interrupt-mode", 1, 1, f_nil, ""),
    S!("set-input-meta-mode", 1, 2, f_nil, ""),
    S!(
        "current-bidi-paragraph-direction",
        0,
        1,
        f_current_bidi_paragraph_direction,
        ""
    ),
    S!(
        "read-non-nil-coding-system",
        1,
        1,
        f_read_non_nil_coding_system,
        ""
    ),
    S!(
        "find-operation-coding-system",
        many 1,
        f_find_operation_coding_system,
        ""
    ),
    S!(
        "define-coding-system-alias",
        2,
        2,
        f_define_coding_system_alias,
        ""
    ),
    S!("next-read-file-uses-dialog-p", 0, 0, f_nil, ""),
    S!("lossage-size", 0, 1, f_lossage_size, ""),
    S!("mouse-position-in-root-frame", 0, 0, f_mouse_position_root, ""),
    S!("window-scroll-bar-width", 0, 1, f_zero, ""),
    S!("window-scroll-bar-height", 0, 1, f_zero, ""),
    S!("line-number-display-width", 0, 1, f_zero, ""),
    // `move-to-window-line' is a Lisp-level defun in GNU, not a subr.
    S!(
        "window-configuration-equal-p",
        2,
        2,
        f_window_config_pred_err,
        ""
    ),
    S!(
        "window-configuration-frame",
        1,
        1,
        f_window_config_pred_err,
        ""
    ),
    S!("internal-stack-stats", 0, 0, f_nil, ""),
    S!("pdumper-stats", 0, 0, f_pdumper_stats, ""),
    S!("profiler-cpu-running-p", 0, 0, f_profiler_cpu_running_p, ""),
    S!(
        "move-to-window-line",
        1,
        1,
        f_move_to_window_line,
        "Position point relative to window (no window system: 0)."
    ),
    S!(
        "network-lookup-address-info",
        1,
        3,
        f_network_lookup_address_info,
        "Look up IP addresses for HOST via getaddrinfo."
    ),
    S!(
        "color-values-from-color-spec",
        1,
        1,
        f_color_values_from_color_spec,
        "Parse a color spec into (R G B) 16-bit values."
    ),
    S!(
        "file-selinux-context",
        1,
        1,
        f_file_selinux_context,
        "Return SELinux context of FILE."
    ),
    S!(
        "set-file-selinux-context",
        2,
        2,
        f_nil,
        "Set SELinux context of FILE."
    ),
    S!("set-file-acl", 2, 2, f_set_file_acl, "Set ACL of FILE."),
    S!(
        "garbage-collect-heapsize",
        0,
        0,
        f_gc_heapsize,
        "Return heap size statistics."
    ),
    S!(
        "garbage-collect-maybe",
        1,
        1,
        f_nil,
        "GC if allocation count warrants it."
    ),
    S!(
        "make-closure",
        many 1,
        f_make_closure,
        "Wrap a byte-code prototype into a closure."
    ),
    S!("do-auto-save", 0, 2, f_nil, "Auto-save all buffers."),
    S!("sqlitep", 1, 1, f_nil, "t if OBJECT is a SQLite handle."),
    S!(
        "bidi-find-overridden-directionality",
        3,
        4,
        f_bidi_find_overridden,
        "Find overridden directionality in STRING."
    ),
    S!(
        "bidi-resolved-levels",
        0,
        1,
        f_bidi_resolved_levels,
        "Return resolved bidi levels."
    ),
    S!(
        "composition-get-gstring",
        4,
        4,
        f_nil,
        "Get gstring for composition."
    ),
    S!(
        "composition-sort-rules",
        1,
        1,
        f_composition_sort_rules,
        "Sort composition rules."
    ),
    S!(
        "find-composition-internal",
        4,
        4,
        f_nil,
        "Find composition at position."
    ),
    S!(
        "remember-mouse-glyph",
        3,
        3,
        f_remember_mouse_glyph,
        "Record glyph under mouse."
    ),
    S!(
        "set-terminal-coding-system-internal",
        1,
        2,
        f_set_terminal_coding,
        "Set terminal coding system."
    ),
    S!(
        "set-safe-terminal-coding-system-internal",
        1,
        1,
        f_nil,
        "Set safe terminal coding system."
    ),
    S!("window-cursor-info", 0, 1, f_nil, "Cursor info for WINDOW."),
    S!("profiler-cpu-log", 0, 0, f_nil, "CPU profiler log."),
    S!(
        "profiler-cpu-stop",
        0,
        0,
        f_profiler_cpu_stop,
        "Stop CPU profiler."
    ),
    S!(
        "profiler-memory-log",
        0,
        0,
        f_profiler_memory_log,
        "Memory profiler log."
    ),
    S!(
        "profiler-memory-running-p",
        0,
        0,
        f_profiler_memory_running_p,
        "t if memory profiler is running."
    ),
    S!(
        "profiler-memory-start",
        0,
        0,
        f_profiler_memory_start,
        "Start memory profiler."
    ),
    S!(
        "profiler-memory-stop",
        0,
        0,
        f_profiler_memory_stop,
        "Stop memory profiler."
    ),
    S!(
        "module-load",
        1,
        1,
        f_module_load,
        "Load a dynamic module FILE."
    ),
    S!(
        "native-elisp-load",
        1,
        2,
        f_native_elisp_load,
        "Load a native-compiled .eln FILE."
    ),
    S!(
        "dump-emacs-portable",
        1,
        2,
        f_nil,
        "Dump a portable Emacs image."
    ),
    S!(
        "dump-emacs-portable--sort-predicate",
        2,
        2,
        f_nil,
        "Dump-time ordering predicate."
    ),
    S!(
        "dump-emacs-portable--sort-predicate-copied",
        2,
        2,
        f_nil,
        "Dump-time ordering predicate for copied objects."
    ),
    S!(
        "backtrace--frames-from-thread",
        1,
        1,
        f_backtrace_frames_from_thread,
        "Backtrace frames of THREAD."
    ),
    S!(
        "backtrace--locals",
        1,
        2,
        f_backtrace_locals,
        "Locals of backtrace frame N."
    ),
    S!(
        "backtrace-debug",
        2,
        3,
        f_nil,
        "Enter debugger for backtrace frame."
    ),
    S!(
        "backtrace-eval",
        2,
        3,
        f_nil,
        "Evaluate FORM in backtrace frame."
    ),
    S!(
        "backtrace-frame--internal",
        3,
        3,
        f_backtrace_frame_internal,
        "Describe backtrace frame N of THREAD."
    ),
    S!(
        "completion--flex-cost-gotoh",
        2,
        2,
        f_flex_cost_gotoh,
        "Flex completion cost via Gotoh alignment."
    ),
    S!("profiler-cpu-start", 1, 1, f_profiler_cpu_start, ""),
    S!("redirect-debugging-output", 1, 2, f_nil, ""),
    S!("make-terminal-frame", 1, 1, f_make_terminal_frame, ""),
    S!("tty-frame-edges", 0, 2, f_nil, ""),
    S!("tty-frame-geometry", 0, 1, f_nil, ""),
    // ---------- native compilation / module stubs ----------
    S!("comp-libgccjit-version", 0, 0, f_nil, ""),
    S!("subr-native-comp-unit", 1, 1, f_subr_native_comp_unit, ""),
    S!("native-comp-function-p", 1, 1, f_nil, ""),
    S!("module-function-p", 1, 1, f_nil, ""),
    S!(
        "comp-el-to-eln-filename",
        1,
        2,
        f_comp_el_to_eln_filename,
        ""
    ),
    // ---------- thread/process internals ----------
    S!("thread-buffer-disposition", 1, 1, f_thread_buffer_disposition, ""),
    S!(
        "thread-set-buffer-disposition",
        2,
        2,
        f_thread_set_buffer_disposition,
        ""
    ),
    S!(
        "internal-default-signal-process",
        2,
        3,
        f_internal_default_signal_process,
        ""
    ),
    S!(
        "internal-default-interrupt-process",
        0,
        2,
        f_internal_default_interrupt,
        ""
    ),
    S!("set-network-process-option", 3, 4, f_process_arg_err, ""),
    S!("set-process-thread", 2, 2, f_process_arg_err, ""),
    S!("process-thread", 1, 1, f_process_arg_err, ""),
    // ---------- reader/printer/composition internals ----------
    S!("lread--substitute-object-in-subtree", 3, 3, f_nil, ""),
    S!("print--preprocess", 1, 1, f_arg0, ""),
    S!("clear-composition-cache", 0, 0, f_nil, ""),
    S!("help--describe-vector", 7, 7, f_nil, ""),
    S!("re--describe-compiled", 1, 2, f_re_describe_compiled, ""),
    S!("system-move-file-to-trash", 1, 1, f_move_file_to_trash, ""),
    // ---------- display/font internals (no GUI) ----------
    S!("get-display-property", 2, 4, f_nil, ""),
    S!("lookup-image-map", 3, 3, f_nil, ""),
    S!("clear-image-cache", 0, 2, f_clear_image_cache, ""),
    S!("image-cache-size", 0, 0, f_zero, ""),
    S!("display--line-is-continued-p", 0, 0, f_nil, ""),
    S!("display--update-for-mouse-movement", 3, 3, f_nil, ""),
    S!("internal-handle-focus-in", 1, 1, f_internal_handle_focus_in, ""),
    S!("internal-face-x-get-resource", 2, 3, f_nil, ""),
    S!("internal-set-alternative-font-family-alist", 1, 1, f_nil, ""),
    S!("internal-set-alternative-font-registry-alist", 1, 1, f_nil, ""),
    S!(
        "internal-set-font-selection-order",
        1,
        1,
        f_set_font_selection_order,
        ""
    ),
    S!("internal-set-lisp-face-attribute-from-resource", 3, 4, f_nil, ""),
    S!("close-font", 1, 2, f_close_font, ""),
    S!("font-has-char-p", 2, 3, f_nil, ""),
    S!("font-shape-gstring", 2, 2, f_nil, ""),
    S!("font-variation-glyphs", 2, 2, f_nil, ""),
    S!("query-fontset", 1, 2, f_query_fontset, ""),
    S!("define-fringe-bitmap", 2, 5, f_define_fringe_bitmap, ""),
    S!("destroy-fringe-bitmap", 1, 1, f_nil, ""),
    S!("set-fringe-bitmap-face", 1, 2, f_nil, ""),
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
    // A `(lambda ARGLIST ...)' or `(closure ENV ARGLIST ...)' list.
    let mut list_arity = |v: &Value| -> Option<Arity> {
        let cells = v.list_to_vec().ok()?;
        let head = i.sym_id(cells.first()?)?;
        let name = i.symbol_name(head);
        if name != "lambda" && name != "closure" {
            return None;
        }
        let arglist_idx = if name == "closure" { 2 } else { 1 };
        let mut min = 0u16;
        let mut max = 0u16;
        let mut many = false;
        let mut mode = 0;
        let opt_sym = i.intern("&optional");
        let rest_sym = i.intern("&rest");
        for a in cells
            .get(arglist_idx)
            .map(|v| v.list_to_vec().unwrap_or_default())
            .unwrap_or_default()
        {
            let Some(id) = i.sym_id(&a) else { continue };
            if id == opt_sym {
                mode = 1;
            } else if id == rest_sym {
                many = true;
                mode = 2;
            } else if mode == 0 {
                min += 1;
                max += 1;
            } else if mode == 1 {
                max += 1;
            }
        }
        Some(if many {
            Arity::Many { min }
        } else {
            Arity::Range { min, max }
        })
    };
    let arity = match &fun {
        Value::Subr(s) => s.arity,
        Value::Lambda(l) => l.arity(),
        v => match list_arity(v) {
            Some(a) => a,
            None => return Err(i.signal_data(sym::VOID_FUNCTION, vec![args[0].clone()])),
        },
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
                    supplied: None,
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

fn f_keymap_canonicalize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: a keymap returns itself; anything else becomes a fresh `(keymap)`.
    match keymap_of(i, &a[0])? {
        Some(_) => Ok(a[0].clone()),
        None => Ok(Value::list(vec![Value::Sym(i.intern("keymap"))])),
    }
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

fn f_display_screens(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU batch: one screen (the initial terminal).
    Ok(Value::Int(1))
}

fn f_command_line(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU recurses into the top-level loop; in batch that ends in
    // excessive-lisp-nesting. Signal a plain error instead.
    Err(i.error("command-line is for interactive use"))
}

fn f_describe_vector(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Vec(_) => Ok(Value::Nil),
        v if is_char_table(i, v) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("vector-or-char-table-p", other)),
    }
}

/// GNU's optional FRAME argument check: nil ok, live frame ok, else
/// `wrong-type-argument framep'.
fn want_opt_frame(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Nil => Ok(()),
        Value::Frame(f) if !f.borrow().dead => Ok(()),
        other => Err(i.wrong_type_mut("framep", other)),
    }
}

/// Whether V names a face (symbol or string), like GNU's face lookup.
fn face_exists(i: &Interp, v: &Value) -> bool {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        _ => return false,
    };
    crate::editor::face_known(i, &name)
}

fn f_face_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_opt_frame(i, a.get(1).unwrap_or(&Value::Nil))?;
    if !face_exists(i, &a[0]) {
        return Err(i.error("Invalid face"));
    }
    // No fonts in batch.
    Ok(Value::Nil)
}

fn f_invert_face(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_opt_frame(i, a.get(1).unwrap_or(&Value::Nil))?;
    if !face_exists(i, &a[0]) {
        return Err(i.error("Invalid face"));
    }
    // GNU returns the face.
    Ok(a[0].clone())
}

fn f_frame_or_buffer_changed_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU's STATE arg is a symbol naming a state vector.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Sym(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("symbolp", other)),
    }
    // Stateful like GNU: t when frames/buffers changed since last call.
    let mut fp = i.frames.len() as u64;
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            fp = fp.wrapping_mul(31).wrapping_add(b.borrow().mod_tick);
        }
    }
    let changed = i.frame_state_seen != Some(fp);
    i.frame_state_seen = Some(fp);
    Ok(Value::from_bool(changed))
}

fn f_image_transforms_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Optional FRAME arg must be a live frame; no transforms in batch.
    match a.first() {
        None | Some(Value::Nil) => Ok(Value::Nil),
        Some(Value::Frame(f)) if !f.borrow().dead => Ok(Value::Nil),
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_false(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

/// `symbol-with-pos' objects print `#<symbol NAME at POS>' in GNU.
/// Ours are records `#s(symbol-with-pos BARE POS)'; this checks the
/// tag and returns (symbol, position).
pub(crate) fn sym_pos_parts(i: &Interp, v: &Value) -> Option<(SymId, i128)> {
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let [Value::Sym(tag), Value::Sym(s), Value::Int(p)] = rr.as_slice() {
            if i.symbol_name(*tag) == "symbol-with-pos" {
                return Some((*s, *p));
            }
        }
    }
    None
}

/// Build a `symbol-with-pos' object for SYM at POS.
pub(crate) fn make_symbol_with_pos(i: &mut Interp, sym: SymId, pos: i128) -> Value {
    Value::Record(std::rc::Rc::new(std::cell::RefCell::new(vec![
        Value::Sym(i.intern("symbol-with-pos")),
        Value::Sym(sym),
        Value::Int(pos),
    ])))
}

fn f_bare_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Some((s, _)) = sym_pos_parts(i, &args[0]) {
        return Ok(Value::Sym(s));
    }
    match &args[0] {
        Value::Sym(_) => Ok(args[0].clone()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_bare_symbol_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // t only for a plain symbol — a positioned symbol is not bare.
    Ok(Value::from_bool(
        matches!(&args[0], Value::Sym(_)) && sym_pos_parts(i, &args[0]).is_none(),
    ))
}

fn f_position_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = match &args[0] {
        Value::Sym(s) => *s,
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    let pos = args.get(1).and_then(|v| v.int()).unwrap_or(0);
    Ok(make_symbol_with_pos(i, s, pos))
}

fn f_symbol_with_pos_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(sym_pos_parts(i, &args[0]).is_some()))
}

fn f_symbol_with_pos_pos(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match sym_pos_parts(i, &args[0]) {
        Some((_, p)) => Ok(Value::Int(p)),
        None => Err(i.wrong_type_mut("symbol-with-pos-p", &args[0])),
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

/// Like `lisp_time_to_us`, but nanosecond precision (ps field / 1000).
pub(crate) fn lisp_time_to_ns(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Cons(_) => {
            // (hi lo us ps) or (TICKS . HZ) — reuse the µs walk for the
            // (TICKS . HZ) case; for the 4-list take ps into account.
            let mut elems: Vec<i128> = Vec::new();
            let mut tail = v.clone();
            let mut dotted_hz = false;
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
                        elems.push(*n as i128);
                        dotted_hz = true;
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
            if dotted_hz && elems.len() == 2 {
                let ticks = elems[0];
                let hz = elems[1];
                return Ok(ticks * 1_000_000_000 / hz.max(1));
            }
            let n = |k: usize| elems.get(k).copied().unwrap_or(0);
            let ticks = match elems.len() {
                0 => 0,
                1 => n(0),
                _ => n(0) * 65536 + n(1),
            };
            Ok(ticks * 1_000_000_000 + n(2) * 1000 + n(3) / 1000)
        }
        _ => Ok(lisp_time_to_us(i, v)? * 1000),
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

/// GNU (hi lo us ps) timestamp from nanoseconds since the epoch.
pub(crate) fn ns_to_lisp_time(ns: i128) -> Value {
    let secs = ns.div_euclid(1_000_000_000);
    let nano = ns.rem_euclid(1_000_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(nano / 1000),
        Value::Int((nano % 1000) * 1000),
    ])
}

/// Like `lisp_time_to_ns`, but picosecond precision — GNU's native
/// tick unit (hz = 10^12).
pub(crate) fn lisp_time_to_ps(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Cons(_) => {
            let mut elems: Vec<i128> = Vec::new();
            let mut tail = v.clone();
            let mut dotted_hz = false;
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
                        elems.push(*n as i128);
                        dotted_hz = true;
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
            if dotted_hz && elems.len() == 2 {
                let ticks = elems[0];
                let hz = elems[1];
                return Ok(ticks * 1_000_000_000_000 / hz.max(1));
            }
            let n = |k: usize| elems.get(k).copied().unwrap_or(0);
            let ticks = match elems.len() {
                0 => 0,
                1 => n(0),
                _ => n(0) * 65536 + n(1),
            };
            Ok(ticks * 1_000_000_000_000 + n(2) * 1_000_000 + n(3))
        }
        Value::Float(f) => Ok((*f * 1e12) as i128),
        _ => Ok(lisp_time_to_us(i, v)? * 1_000_000),
    }
}

/// GNU (hi lo us ps) timestamp from picoseconds since the epoch.
pub(crate) fn ps_to_lisp_time(ps: i128) -> Value {
    let secs = ps.div_euclid(1_000_000_000_000);
    let rem = ps.rem_euclid(1_000_000_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(rem / 1_000_000),
        Value::Int(rem % 1_000_000),
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
    let x = lisp_time_to_ps(i, &a[0])?;
    let y = lisp_time_to_ps(i, &a[1])?;
    Ok(ps_to_lisp_time(if sub { x - y } else { x + y }))
}

fn f_time_add(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    time_arith(i, &args, false)
}

fn f_time_subtract(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    time_arith(i, &args, true)
}

fn f_time_less_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_ps(i, &args[0])?;
    let b = lisp_time_to_ps(i, &args[1])?;
    Ok(Value::from_bool(a < b))
}

fn f_time_equal_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_ps(i, &args[0])?;
    let b = lisp_time_to_ps(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}

fn f_time_convert(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU timefns.c: internal representation is (TICKS . HZ); `t'
    // yields ps ticks (hz = 10^12), an integer FORM yields
    // (TICKS . FORM), `integer' truncates to whole seconds, `list'
    // (the default) yields (HI LO US PS).
    let ps = lisp_time_to_ps(i, &args[0])?;
    match args.get(1) {
        Some(Value::Int(hz)) if *hz > 0 => Ok(Value::cons(
            Value::Int(ps * *hz / 1_000_000_000_000),
            Value::Int(*hz),
        )),
        Some(Value::Sym(_)) => {
            let name = i.symbol_name(i.sym_id(&args[1]).unwrap_or(0));
            if name == "integer" {
                Ok(Value::Int(ps / 1_000_000_000_000))
            } else if name == "t" {
                Ok(Value::cons(Value::Int(ps), Value::Int(1_000_000_000_000)))
            } else {
                Ok(ps_to_lisp_time(ps))
            }
        }
        _ => Ok(ps_to_lisp_time(ps)),
    }
}

fn f_load_average(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // getloadavg(3): 1/5/15-minute averages, scaled like Emacs (*100).
    unsafe extern "C" {
        fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
    }
    let mut v = [0.0f64; 3];
    let n = unsafe { getloadavg(v.as_mut_ptr(), 3) };
    let items: Vec<Value> = (0..n.max(0) as usize)
        .map(|k| Value::Float((v[k] * 100.0) as i64 as f64))
        .collect();
    Ok(Value::list(items))
}

fn f_invocation_dir(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let dir = std::env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .parent()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let _ = i;
    Ok(Value::string(dir))
}

fn f_build_binding(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (VAR VALUE &optional BUFFER) — lexical binding object.
    Ok(Value::cons(a[0].clone(), a[1].clone()))
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
    Ok(Value::string(uptime_text(secs)))
}

/// `emacs-uptime` default format: "N seconds" / "N days, HH:MM:SS".
pub(crate) fn uptime_text(secs: i128) -> String {
    let days = secs / 86400;
    if days <= 0 {
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
    }
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
        _ => return Err(Flow::Signal(Value::Nil, Value::Nil, false)),
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
    let dir_flag = !arg(&args, 1).is_nil();
    let suffix = match arg(&args, 2) {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let text = match arg(&args, 3) {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let dir = std::env::temp_dir();
    for n in 0..1000u32 {
        let cand = dir.join(format!(
            "{}{}{}",
            prefix,
            if n == 0 {
                random_suffix()
            } else {
                format!("{}{}", random_suffix(), n)
            },
            suffix
        ));
        let res = if dir_flag {
            std::fs::create_dir(&cand)
        } else {
            std::fs::File::create_new(&cand).map(|mut f| {
                use std::io::Write;
                let _ = f.write_all(text.as_bytes());
            })
        };
        match res {
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
    let ok_if_exists = args.get(2).cloned().unwrap_or(Value::Nil);
    if ok_if_exists.truthy() && !matches!(ok_if_exists, Value::Int(_)) {
        // Non-nil non-integer means overwrite silently (GNU fileio.c).
        let _ = std::fs::remove_file(&name);
    }
    match std::os::unix::fs::symlink(&target, &name) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let s = i.intern("file-already-exists");
            Err(i.signal_data(
                s,
                vec![
                    Value::string("File already exists"),
                    Value::string(name),
                ],
            ))
        }
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
    let time = args.get(1).cloned().unwrap_or(Value::Nil);
    let nofollow = match args.get(2) {
        Some(Value::Sym(s)) => i.symbol_name(*s) == "nofollow",
        Some(v) => v.truthy(),
        None => false,
    };
    let ns = lisp_time_to_ns(i, &time)?;
    let secs = ns.div_euclid(1_000_000_000) as i64;
    let nsec = ns.rem_euclid(1_000_000_000) as i64;
    #[cfg(unix)]
    {
        let c = std::ffi::CString::new(name.clone()).map_err(|_| {
            i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")])
        })?;
        let ts = libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: nsec as _,
        };
        let times = [ts, ts];
        let flag = if nofollow { libc::AT_SYMLINK_NOFOLLOW } else { 0 };
        let r = unsafe {
            libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), flag)
        };
        if r != 0 {
            let e = std::io::Error::last_os_error();
            let (s, msg) = if e.kind() == std::io::ErrorKind::NotFound {
                (sym::FILE_MISSING, "Setting file times: no such file")
            } else {
                (sym::FILE_ERROR, "Setting file times")
            };
            return Err(i.signal_data(
                s,
                vec![
                    Value::string(format!("{}: {}", msg, e)),
                    Value::string(name),
                ],
            ));
        }
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

fn f_group_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let gid = want_int(i, &a[0])?;
    #[cfg(unix)]
    unsafe {
        let grp = libc::getgrgid(gid as libc::gid_t);
        if !grp.is_null() {
            let name = std::ffi::CStr::from_ptr((*grp).gr_name).to_string_lossy();
            return Ok(Value::string(name.into_owned()));
        }
    }
    Ok(Value::Nil)
}

fn f_get_truename_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_string(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_unencodable_char_position(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_compose_region_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (start, end) = (want_int(i, &a[0])?, want_int(i, &a[1])?);
    let (begv, zv) = i
        .current_buffer_ref()
        .map(|b| {
            let b = b.borrow();
            (b.begv as i128 + 1, b.zv as i128 + 1)
        })
        .unwrap_or((1, 1));
    if start < begv || end > zv || start > end {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    Ok(Value::Nil)
}

fn f_compose_string_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let (start, end) = (want_int(i, &a[1])?, want_int(i, &a[2])?);
    if start < 0 || end > s.chars().count() as i128 || start > end {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), a[1].clone(), a[2].clone()],
        ));
    }
    Ok(Value::string(s))
}

fn f_set_buffer_redisplay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !matches!(a[0], Value::Buffer(_)) && !a[0].is_nil() {
        return Err(i.wrong_type_mut("bufferp", &a[0]));
    }
    want_int(i, &a[1])?;
    want_int(i, &a[2])?;
    Ok(Value::Nil)
}

fn f_register_ccl_program(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    let elems = match &a[1] {
        Value::Vec(v) => v.borrow().clone(),
        other => return Err(i.wrong_type_mut("vectorp", other)),
    };
    if elems.len() < 3 || !elems.iter().all(|e| matches!(e, Value::Int(_))) {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid CCL program")],
        ));
    }
    let prop = i.intern("ccl-program");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.ccl_program_count;
    i.ccl_program_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

fn f_ccl_program_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    let prop = i.intern("ccl-program");
    Ok(if i.get_prop(name, prop).is_nil() {
        Value::Nil
    } else {
        Value::t()
    })
}

fn f_ccl_execute(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    let prop = i.intern("ccl-program");
    if i.get_prop(name, prop).is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("CCL program is not registered")],
        ));
    }
    Ok(Value::Nil)
}

fn f_ccl_execute_on_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[1])?;
    f_ccl_execute(i, vec![a[0].clone(), a[2].clone()])?;
    Ok(Value::string(s))
}

fn f_register_code_conversion_map(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    if !matches!(a[1], Value::Vec(_)) {
        return Err(i.wrong_type_mut("vectorp", &a[1]));
    }
    let prop = i.intern("code-conversion-map");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.code_conv_map_count;
    i.code_conv_map_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

fn f_zlib_available_p(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

fn f_zlib_decompress_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use std::io::Read;
    let (start, end) = (want_int(i, &a[0])?, want_int(i, &a[1])?);
    let buf = match i.current_buffer_ref() {
        Some(b) => b,
        None => return Err(i.signal_data(sym::ERROR, vec![Value::string("No buffer")])),
    };
    let bytes = {
        let bb = buf.borrow();
        let (begv, zv) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if start < begv || end > zv || start > end {
            return Err(i.signal_data(
                sym::ARGS_OUT_OF_RANGE,
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        let t = bb.text.text();
        let chars: Vec<char> = t.chars().collect();
        let lo = (start - 1).max(0) as usize;
        let hi = (end - 1).min(chars.len() as i128) as usize;
        // Approximate unibyte storage: Latin-1 chars encode as their
        // single byte; anything else uses UTF-8.
        let mut bytes = Vec::new();
        let mut tmp = [0u8; 4];
        for &c in &chars[lo..hi] {
            if (c as u32) < 256 {
                bytes.push(c as u8);
            } else {
                bytes.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
        bytes
    };
    // Try gzip first, then raw zlib.
    let mut decoded: Option<Vec<u8>> = None;
    {
        let mut d = flate2::read::GzDecoder::new(&bytes[..]);
        let mut out = Vec::new();
        if d.read_to_end(&mut out).is_ok() {
            decoded = Some(out);
        }
    }
    if decoded.is_none() {
        let mut d = flate2::read::ZlibDecoder::new(&bytes[..]);
        let mut out = Vec::new();
        if d.read_to_end(&mut out).is_ok() {
            decoded = Some(out);
        }
    }
    match decoded {
        Some(out) => {
            let text = String::from_utf8_lossy(&out).into_owned();
            let mut bb = buf.borrow_mut();
            let lo = (start - 1).max(0) as usize;
            let hi = (end - 1).max(lo as i128) as usize;
            bb.delete_region(lo, hi);
            bb.insert_at(lo, &text);
            Ok(Value::t())
        }
        None => Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Malformed or misplaced compressed data")],
        )),
    }
}

fn f_find_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    for id in i.buffers.list() {
        let v = match i.buffers.get(id) {
            Some(b) => match b.borrow().locals.get(&sid) {
                Some(v) => v.clone(),
                None => i.obarray.symbol(sid).value.clone(),
            },
            None => continue,
        };
        if matches!(v, Value::Sym(s) if s == sym::UNBOUND) {
            return Err(i.signal_data(sym::VOID_VARIABLE, vec![a[0].clone()]));
        }
        if super::equal_values(i, &v, &a[1]) {
            if let Some(b) = i.buffers.get(id) {
                return Ok(Value::Buffer(b));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_insert_byte(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let byte = want_int(i, &a[0])?;
    let count = want_int(i, &a[1])?;
    if !(0..=255).contains(&byte) {
        return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone()]));
    }
    let ch = char::from_u32(byte as u32).unwrap_or('\u{fffd}');
    let s: String = std::iter::repeat_n(ch, count.max(0) as usize).collect();
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().insert(&s);
    }
    Ok(Value::Nil)
}

fn f_set_binary_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    if !matches!(name.as_str(), "stdin" | "stdout" | "stderr") {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Bad stream {}", name))],
        ));
    }
    // POSIX: always binary; value is the previous mode (non-nil).
    Ok(Value::t())
}

fn f_set_output_flow_control(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (FLOW &optional TERMINAL). No terminal objects in this port;
    // GNU signals terminal-live-p when TERMINAL is given and not a
    // live terminal.
    if let Some(term) = a.get(1) {
        let pred = i.intern("terminal-live-p");
        return Err(i.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred), term.clone()],
        ));
    }
    Ok(Value::Nil)
}

fn f_tab_bar_height(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

fn f_insert_special_event(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !matches!(a[0], Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_buffer_text_pixel_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let buf = match a.first() {
        None | Some(Value::Nil) => i.current_buffer_ref(),
        Some(Value::Buffer(b)) => Some(b.clone()),
        Some(other) => return Err(i.wrong_type_mut("window-live-p", other)),
    };
    let (width, height) = match buf {
        Some(b) => {
            let bb = b.borrow();
            let text = bb.text.text();
            let chars: Vec<char> = text.chars().collect();
            let (mut from, mut to) = (1i128, chars.len() as i128 + 1);
            if let Some(v) = a.get(1) {
                if !v.is_nil() {
                    from = want_int(i, v)?;
                }
            }
            if let Some(v) = a.get(2) {
                if !v.is_nil() {
                    to = want_int(i, v)?;
                }
            }
            let lo = (from - 1).clamp(0, chars.len() as i128) as usize;
            let hi = (to - 1).clamp(lo as i128, chars.len() as i128) as usize;
            let region: String = chars[lo..hi].iter().collect();
            let mut w = 0usize;
            let mut h = region.matches('\n').count();
            for line in region.split('\n') {
                w = w.max(line.chars().count());
            }
            if !region.ends_with('\n') && !region.is_empty() {
                h += 1;
            }
            (w, h)
        }
        None => (0, 0),
    };
    Ok(Value::cons(
        Value::Int(width as i128),
        Value::Int(height as i128),
    ))
}

fn f_format_mode_line(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // No mode-line machinery; batch GNU likewise yields "".
    Ok(Value::string(""))
}

// ---------- category tables ----------
// Record layout: [char-table, 'category-table, Vec contents, Vec
// docstrings(128)]. GNU stores one extra slot (the docstring table);
// ours is a plain Vec for simplicity.

/// Build a category table value. `standard` populates GNU's ASCII
/// membership and label docstrings.
pub(crate) fn make_category_table_value(i: &mut Interp, standard: bool) -> Value {
    let slots = Rc::new(RefCell::new(vec![Value::Nil; 256]));
    let docs = Rc::new(RefCell::new(vec![Value::Nil; 128]));
    let t = Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("char-table")),
        Value::Sym(i.intern("category-table")),
        Value::Vec(slots.clone()),
        Value::Vec(docs.clone()),
    ])));
    if standard {
        // GNU's standard-category-table ASCII defaults.
        const DOCS: &[(usize, &str)] = &[
            (32, "space for indent\nThis character counts as a space for indentation purposes."),
            (46, "Base\nBase characters (Unicode General Category L,N,P,S,Zs)"),
            (48, "consonant"),
            (49, "base vowel\nBase (independent) vowel"),
            (50, "upper diacritic\nUpper diacritical mark (including upper vowel)"),
            (51, "lower diacritic\nLower diacritical mark (including lower vowel)"),
            (52, "combining tone\nCombining tone mark"),
            (53, "symbol"),
            (54, "digit"),
            (55, "vowel diacritic\nVowel-modifying diacritical mark"),
            (56, "vowel-signs"),
            (57, "semivowel lower"),
            (60, "Not at eol\nA character which can't be placed at end of line."),
            (62, "Not at bol\nA character which can't be placed at beginning of line."),
            (65, "2-byte alnum\nAlphanumeric characters of 2-byte character sets"),
            (67, "2-byte han\nChinese (Han) characters of 2-byte character sets"),
            (71, "2-byte Greek\nGreek characters of 2-byte character sets"),
            (72, "2-byte Hiragana\nJapanese Hiragana characters of 2-byte character sets"),
            (73, "Indian Glyphs"),
            (75, "2-byte Katakana\nJapanese Katakana characters of 2-byte character sets"),
            (76, "Strong L2R\nCharacters with \"strong\" left-to-right directionality, i.e.\nwith L, LRE, or LRO Unicode bidi character type."),
            (78, "2-byte Korean\nKorean Hangul characters of 2-byte character sets"),
            (82, "Strong R2L\nCharacters with \"strong\" right-to-left directionality, i.e.\nwith R, AL, RLE, or RLO Unicode bidi character type."),
            (89, "2-byte Cyrillic\nCyrillic characters of 2-byte character sets"),
            (94, "Combining\nCombining diacritic or mark (Unicode General Category M)"),
            (97, "ASCII\nASCII graphic characters 32-126 (ISO646 IRV:1983[4/0])"),
            (98, "Arabic"),
            (99, "Chinese"),
            (101, "Ethiopic\nEthiopic (Ge'ez)"),
            (103, "Greek"),
            (104, "Korean"),
            (105, "Indian"),
            (106, "Japanese"),
            (107, "Katakana\nJapanese katakana"),
            (111, "Lao"),
            (113, "Tibetan"),
            (114, "Roman\nJapanese roman"),
            (116, "Thai"),
            (118, "Viet\nVietnamese"),
            (119, "Hebrew"),
            (121, "Cyrillic"),
            (124, "line breakable\nWhile filling, we can break a line at this character."),
        ];
        {
            let mut d = docs.borrow_mut();
            for &(label, doc) in DOCS {
                d[label] = Value::string(doc);
            }
        }
        let mut sv = slots.borrow_mut();
        for ch in 32usize..=126 {
            let mut bits = vec![false; 128];
            for b in [46usize, 97, 108] {
                bits[b] = true;
            }
            if ch != 32 && ch != 92 && ch != 126 {
                bits[114] = true;
            }
            let c = char::from_u32(ch as u32).unwrap();
            if c.is_ascii_alphabetic() {
                bits[76] = true;
            }
            if c.is_ascii_digit() {
                bits[54] = true;
            }
            sv[ch] = make_bool_vector(i, bits);
        }
    }
    t
}

fn is_category_table(i: &Interp, v: &Value) -> bool {
    is_char_table(i, v)
        && matches!(char_table_subtype_of(v), Value::Sym(s) if i.symbol_name(s) == "category-table")
}

fn want_category_table(i: &mut Interp, v: &Value) -> Result<Rc<RefCell<Vec<Value>>>, Flow> {
    match v {
        Value::Nil => Ok(match i
            .current_buffer_ref()
            .and_then(|b| b.borrow().category_table.clone())
        {
            Some(t) => match t {
                Value::Record(r) => r,
                _ => return Err(i.wrong_type_mut("category-table-p", &Value::Nil)),
            },
            None => match i.standard_category_table() {
                Value::Record(r) => r,
                _ => unreachable!(),
            },
        }),
        Value::Record(r) if is_category_table(i, v) => Ok(r.clone()),
        other => Err(i.wrong_type_mut("category-table-p", other)),
    }
}

/// The docstring vec of a category table (record slot 3).
fn cat_docs(v: &Rc<RefCell<Vec<Value>>>) -> Rc<RefCell<Vec<Value>>> {
    let rr = v.borrow();
    match rr.get(3) {
        Some(Value::Vec(d)) => d.clone(),
        _ => Rc::new(RefCell::new(vec![Value::Nil; 128])),
    }
}

fn f_make_category_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(make_category_table_value(i, false))
}

fn f_category_table_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_category_table(i, &a[0])))
}

fn f_standard_category_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(i.standard_category_table())
}

fn f_category_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let v = i
        .current_buffer_ref()
        .and_then(|b| b.borrow().category_table.clone())
        .unwrap_or_else(|| i.standard_category_table());
    Ok(v)
}

fn f_set_category_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_category_table(i, &a[0]) {
        return Err(i.wrong_type_mut("category-table-p", &a[0]));
    }
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().category_table = Some(a[0].clone());
    }
    Ok(a[0].clone())
}

fn f_copy_category_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let src = want_category_table(i, &arg(&a, 0))?;
    let rr = src.borrow();
    let contents = match rr.get(2) {
        Some(Value::Vec(v)) => v.clone(),
        _ => Rc::new(RefCell::new(vec![Value::Nil; 256])),
    };
    let docs = cat_docs(&src);
    drop(rr);
    // Fcopy_sequence-style: share the per-char category sets and the
    // docstring table, copy the top-level vec.
    Ok(Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("char-table")),
        Value::Sym(i.intern("category-table")),
        Value::Vec(Rc::new(RefCell::new(contents.borrow().clone()))),
        Value::Vec(docs),
    ]))))
}

fn f_define_category(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let cat = want_int(i, &a[0])?;
    if !(32..=126).contains(&cat) {
        return Err(i.wrong_type_mut("categoryp", &a[0]));
    }
    let doc = want_string(i, &a[1])?;
    let t = want_category_table(i, &arg(&a, 2))?;
    let docs = cat_docs(&t);
    if !docs.borrow()[cat as usize].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Category `{}' is already defined",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    docs.borrow_mut()[cat as usize] = Value::string(doc);
    Ok(Value::Nil)
}

fn f_category_docstring(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let cat = want_int(i, &a[0])?;
    if !(32..=126).contains(&cat) {
        return Err(i.wrong_type_mut("categoryp", &a[0]));
    }
    let t = want_category_table(i, &arg(&a, 1))?;
    Ok(cat_docs(&t).borrow()[cat as usize].clone())
}

fn f_get_unused_category(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let t = want_category_table(i, &arg(&a, 0))?;
    let docs = cat_docs(&t);
    let dd = docs.borrow();
    for c in 32usize..=126 {
        if dd[c].is_nil() {
            return Ok(Value::Int(c as i128));
        }
    }
    Ok(Value::Nil)
}

fn f_modify_category_entry(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // CHAR may be a char or a (MIN . MAX) cons.
    let (lo, hi) = match &a[0] {
        Value::Int(c) => (*c, *c),
        Value::Cons(c) => {
            let r = c.borrow();
            match (&r.car, &r.cdr) {
                (Value::Int(l), Value::Int(h)) => (*l, *h),
                _ => return Err(i.wrong_type_mut("characterp", &a[0])),
            }
        }
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let cat = want_int(i, &a[1])?;
    if !(32..=126).contains(&cat) {
        return Err(i.wrong_type_mut("categoryp", &a[1]));
    }
    let reset = !arg(&a, 3).is_nil();
    let t = want_category_table(i, &arg(&a, 2))?;
    if cat_docs(&t).borrow()[cat as usize].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Undefined category: {}",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    let contents = match char_table_vec(&Value::Record(t.clone())) {
        Some(v) => v,
        None => return Ok(Value::Nil),
    };
    for ch in lo..=hi {
        if !(0..256).contains(&ch) {
            continue;
        }
        let mut bits = vec![false; 128];
        let mut cv = contents.borrow_mut();
        if !reset {
            if let Some(old) = cv.get(ch as usize) {
                if let Ok(b) = bool_vec_of(i, old) {
                    for (k, v) in b.iter().enumerate() {
                        if k < 128 {
                            bits[k] = *v;
                        }
                    }
                }
            }
        }
        bits[cat as usize] = true;
        cv[ch as usize] = make_bool_vector(i, bits);
    }
    Ok(Value::Nil)
}

fn f_char_category_set(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ch = want_int(i, &a[0])?;
    let t = want_category_table(i, &Value::Nil)?;
    let contents = match char_table_vec(&Value::Record(t)) {
        Some(v) => v,
        None => return Ok(Value::Nil),
    };
    let cv = contents.borrow();
    Ok(match cv.get(ch as usize) {
        Some(v) if !v.is_nil() => v.clone(),
        _ => make_bool_vector(i, vec![false; 128]),
    })
}

fn f_category_set_mnemonics(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let bits = bool_vec_of_cat(i, &a[0])?;
    let s: String = bits
        .iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .filter_map(|(k, _)| char::from_u32(k as u32))
        .collect();
    Ok(Value::string(s))
}

fn bool_vec_of_cat(i: &mut Interp, v: &Value) -> Result<Vec<bool>, Flow> {
    if !is_bool_vector(i, v) {
        return Err(i.wrong_type_mut("categorysetp", v));
    }
    bool_vec_of(i, v)
}

fn f_make_category_set(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let mut bits = vec![false; 128];
    for c in s.chars() {
        let k = c as usize;
        if k < 128 {
            bits[k] = true;
        }
    }
    Ok(make_bool_vector(i, bits))
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

// ---------- threads ----------

fn want_thread(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::ThreadRef, Flow> {
    match v {
        Value::Thread(t) => Ok(t.clone()),
        other => Err(i.wrong_type_mut("threadp", other)),
    }
}

fn f_threadp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Thread(_))))
}

fn f_all_threads(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU lists only threads that haven't finished (a finished thread
    // drops out even before `thread-join' reaps it).
    let ts: Vec<Value> = i
        .threads
        .iter()
        .filter(|t| !t.borrow().finished)
        .map(|t| Value::Thread(t.clone()))
        .collect();
    Ok(Value::list(ts))
}

fn f_current_thread(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Thread(i.threads[i.current_thread].clone()))
}

fn f_thread_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(t.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_thread_live_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(Value::from_bool(t.borrow().alive))
}

fn f_make_thread(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (make-thread FUNCTION &optional NAME) — cooperative model: run
    // the function now; the thread stays a zombie (live) until joined.
    let fun = a[0].clone();
    let name = match a.get(1) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let t = std::rc::Rc::new(std::cell::RefCell::new(crate::lisp::value::Thread {
        name,
        alive: true,
        result: None,
        last_error: None,
        finished: false,
    }));
    i.threads.push(t.clone());
    let idx = i.threads.len() - 1;
    let saved = i.current_thread;
    i.current_thread = idx;
    let r = i.apply(&fun, vec![]);
    i.current_thread = saved;
    {
        let mut tb = t.borrow_mut();
        tb.finished = true;
        match r {
            Ok(v) => tb.result = Some(v),
            Err(Flow::Signal(s, d, _)) => {
                let cond = Value::cons(s.clone(), d.clone());
                tb.last_error = Some(cond.clone());
                i.thread_last_error = cond;
            }
            Err(e) => {
                tb.last_error = Some(Value::Nil);
                return Err(e);
            }
        }
    }
    Ok(Value::Thread(t))
}

fn f_thread_join(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: reaps the thread; returns the function result, or nil when
    // it died with an error.
    let t = want_thread(i, &a[0])?;
    let mut tb = t.borrow_mut();
    tb.alive = false;
    Ok(tb.result.clone().unwrap_or(Value::Nil))
}

fn f_thread_last_error(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(i.thread_last_error.clone())
}

// ---------- mutexes and condition variables (cooperative model) ----------

fn want_mutex(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::MutexRef, Flow> {
    match v {
        Value::Mutex(m) => Ok(m.clone()),
        other => Err(i.wrong_type_mut("mutexp", other)),
    }
}

fn want_condvar(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::CondVarRef, Flow> {
    match v {
        Value::CondVar(c) => Ok(c.clone()),
        other => Err(i.wrong_type_mut("condition-variable-p", other)),
    }
}

fn f_mutexp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Mutex(_))))
}

fn f_make_mutex(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match a.first() {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::Mutex(std::rc::Rc::new(std::cell::RefCell::new(
        crate::lisp::value::Mutex {
            name,
            owner: None,
        },
    ))))
}

fn f_mutex_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    Ok(m.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_mutex_lock(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    m.borrow_mut().owner = Some(Value::Thread(i.threads[i.current_thread].clone()));
    Ok(Value::Nil)
}

fn f_mutex_unlock(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    if m.borrow().owner.is_none() {
        return Err(i.error("Cannot unlock mutex owned by another thread"));
    }
    m.borrow_mut().owner = None;
    Ok(Value::Nil)
}

fn f_condition_variable_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::CondVar(_))))
}

fn f_make_condition_variable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    let name = match a.get(1) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::CondVar(std::rc::Rc::new(std::cell::RefCell::new(
        crate::lisp::value::CondVar {
            name,
            mutex: Value::Mutex(m),
        },
    ))))
}

fn f_condition_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_condition_mutex(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow().mutex.clone())
}

fn condition_mutex_held(i: &Interp, c: &crate::lisp::value::CondVarRef) -> bool {
    let cb = c.borrow();
    let Value::Mutex(m) = &cb.mutex else { return false };
    let mb = m.borrow();
    match &mb.owner {
        Some(Value::Thread(t)) => std::rc::Rc::ptr_eq(t, &i.threads[i.current_thread]),
        _ => false,
    }
}

fn f_condition_wait(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU signals when the condvar's mutex isn't held by the current
    // thread. With the cooperative model a held mutex means "wait"
    // returns immediately — there is no other thread to wake us.
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error("Condition variable’s mutex is not held by current thread"));
    }
    Ok(Value::Nil)
}

fn f_condition_notify(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error("Condition variable’s mutex is not held by current thread"));
    }
    Ok(Value::Nil)
}

fn f_make_finalizer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // The function is retained but never invoked (no GC hooks needed
    // for observable behavior).
    let _ = i;
    Ok(Value::Finalizer(std::rc::Rc::new(std::cell::RefCell::new(
        a[0].clone(),
    ))))
}

fn f_byte_to_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?;
    match u32::try_from(n).ok().and_then(char::from_u32) {
        Some(c) => Ok(Value::string(c.to_string())),
        None => Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone()])),
    }
}

fn f_get_load_suffixes(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // macOS module suffixes first, like GNU.
    Ok(Value::list(
        [".so", ".dylib", ".elc", ".elc.gz", ".el", ".el.gz"]
            .iter()
            .map(|s| Value::string(*s))
            .collect(),
    ))
}

fn f_num_processors(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let n = std::thread::available_parallelism()
        .map(|n| n.get() as i128)
        .unwrap_or(1);
    Ok(Value::Int(n))
}

fn f_daemon_initialized(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("This function can only be called if emacs is run as a daemon"))
}

fn f_signal_names(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // POSIX names in reverse signal-number order, as GNU's `signal-names'.
    const NAMES: &[&str] = &[
        "USR2", "USR1", "INFO", "WINCH", "PROF", "VTALRM", "XFSZ", "XCPU", "IO", "TTOU", "TTIN",
        "CHLD", "CONT", "TSTP", "STOP", "URG", "TERM", "ALRM", "PIPE", "SYS", "SEGV", "BUS",
        "KILL", "FPE", "EMT", "ABRT", "TRAP", "ILL", "QUIT", "INT", "HUP", "EXIT",
    ];
    Ok(Value::list(
        NAMES.iter().map(|s| Value::Sym(i.intern(s))).collect(),
    ))
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
        Value::Process(_) => "process",
        Value::Thread(_) => "thread",
        Value::Mutex(_) => "mutex",
        Value::CondVar(_) => "condition-variable",
        Value::Finalizer(_) => "finalizer",
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
        // GNU: B must be a bool-vector of the same length.
        if !is_bool_vector(i, &target) {
            return Err(i.wrong_type_mut("bool-vector-p", &target));
        }
        if let Value::Record(r) = &target {
            let rr = r.borrow();
            if let Some(Value::Vec(b)) = rr.get(1) {
                let mut bb = b.borrow_mut();
                if bb.len() != out.len() {
                    let s = i.intern("args-out-of-range");
                    return Err(i.signal_data(s, vec![target.clone()]));
                }
                for (k, v) in bb.iter_mut().zip(&out) {
                    *k = Value::Int(if *v { 1 } else { 0 });
                }
                return Ok(target.clone());
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
    let Value::Int(at) = a[1] else {
        return Err(i.wrong_type_mut("integerp", &a[1]));
    };
    let b = !a[2].is_nil();
    Ok(Value::Int(
        x.iter()
            .skip(at.max(0) as usize)
            .take_while(|v| **v == b)
            .count() as i128,
    ))
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
    // A (lambda PARAMS . BODY) form: extract PARAMS directly.
    let mut v = i.indirect_function_value(&a[0]);
    if let Value::Cons(_) = v {
        if let Ok(items) = v.list_to_vec() {
            let head_lam = matches!(items.first(), Some(Value::Sym(s))
                if *s == i.intern("lambda") || *s == i.intern("closure"));
            if head_lam {
                if let Ok(l) = i.lambda_from_form(&v, None) {
                    v = Value::Lambda(std::rc::Rc::new(l));
                }
            }
        }
    }
    match v {
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

/// Coding systems defined by GNU Emacs batch startup.
pub(crate) const CODING_SYSTEMS: &[&str] = &[
    "adobe-standard-encoding",
    "alternativnyj",
    "ascii",
    "big5",
    "big5-hkscs",
    "binary",
    "chinese-big5",
    "chinese-big5-hkscs",
    "chinese-gb18030",
    "chinese-gbk",
    "chinese-hz",
    "chinese-iso-7bit",
    "chinese-iso-8bit",
    "cn-big5",
    "cn-big5-hkscs",
    "cn-gb",
    "cn-gb-2312",
    "compound-text",
    "compound-text-with-extensions",
    "cp038",
    "cp1047",
    "cp1125",
    "cp1250",
    "cp1251",
    "cp1252",
    "cp1253",
    "cp1254",
    "cp1255",
    "cp1256",
    "cp1257",
    "cp1258",
    "cp256",
    "cp273",
    "cp274",
    "cp275",
    "cp277",
    "cp278",
    "cp280",
    "cp281",
    "cp284",
    "cp285",
    "cp290",
    "cp297",
    "cp437",
    "cp65001",
    "cp737",
    "cp775",
    "cp850",
    "cp851",
    "cp852",
    "cp855",
    "cp857",
    "cp858",
    "cp860",
    "cp861",
    "cp862",
    "cp863",
    "cp865",
    "cp866",
    "cp866u",
    "cp869",
    "cp874",
    "cp878",
    "cp932",
    "cp936",
    "cp949",
    "cp950",
    "ctext",
    "ctext-no-compositions",
    "ctext-with-extensions",
    "cyrillic-alternativnyj",
    "cyrillic-iso-8bit",
    "cyrillic-koi8",
    "devanagari",
    "ebcdic-be",
    "ebcdic-br",
    "ebcdic-cp-dk",
    "ebcdic-cp-es",
    "ebcdic-cp-fi",
    "ebcdic-cp-fr",
    "ebcdic-cp-gb",
    "ebcdic-cp-it",
    "ebcdic-cp-no",
    "ebcdic-cp-se",
    "ebcdic-int",
    "ebcdic-int1",
    "ebcdic-jp-e",
    "ebcdic-jp-kana",
    "ebcdic-uk",
    "ebcdic-us",
    "emacs-mule",
    "euc-china",
    "euc-cn",
    "euc-japan",
    "euc-japan-1990",
    "euc-jis-2004",
    "euc-jisx0213",
    "euc-jp",
    "euc-korea",
    "euc-kr",
    "euc-taiwan",
    "euc-tw",
    "eucjp-ms",
    "gb18030",
    "gb2312",
    "gbk",
    "georgian-academy",
    "georgian-ps",
    "greek-iso-8bit",
    "hebrew-iso-8bit",
    "hp-roman8",
    "hz",
    "hz-gb-2312",
    "ibm038",
    "ibm1047",
    "ibm256",
    "ibm273",
    "ibm274",
    "ibm275",
    "ibm277",
    "ibm278",
    "ibm280",
    "ibm281",
    "ibm284",
    "ibm285",
    "ibm290",
    "ibm297",
    "ibm437",
    "ibm775",
    "ibm850",
    "ibm851",
    "ibm852",
    "ibm855",
    "ibm857",
    "ibm860",
    "ibm861",
    "ibm862",
    "ibm863",
    "ibm865",
    "ibm869",
    "ibm874",
    "in-is13194-devanagari",
    "iso-2022-7bit",
    "iso-2022-7bit-lock",
    "iso-2022-7bit-lock-ss2",
    "iso-2022-7bit-ss2",
    "iso-2022-8bit-ss2",
    "iso-2022-cjk",
    "iso-2022-cn",
    "iso-2022-cn-ext",
    "iso-2022-int-1",
    "iso-2022-jp",
    "iso-2022-jp-1978-irv",
    "iso-2022-jp-2",
    "iso-2022-jp-2004",
    "iso-2022-jp-3",
    "iso-2022-kr",
    "iso-8859-1",
    "iso-8859-10",
    "iso-8859-11",
    "iso-8859-13",
    "iso-8859-14",
    "iso-8859-15",
    "iso-8859-16",
    "iso-8859-2",
    "iso-8859-3",
    "iso-8859-4",
    "iso-8859-5",
    "iso-8859-6",
    "iso-8859-7",
    "iso-8859-8",
    "iso-8859-8-e",
    "iso-8859-8-i",
    "iso-8859-9",
    "iso-latin-1",
    "iso-latin-10",
    "iso-latin-2",
    "iso-latin-3",
    "iso-latin-4",
    "iso-latin-5",
    "iso-latin-6",
    "iso-latin-7",
    "iso-latin-8",
    "iso-latin-9",
    "iso-safe",
    "japanese-cp932",
    "japanese-iso-7bit-1978-irv",
    "japanese-iso-8bit",
    "japanese-shift-jis",
    "japanese-shift-jis-2004",
    "junet",
    "koi8",
    "koi8-r",
    "koi8-t",
    "koi8-u",
    "korean-cp949",
    "korean-iso-7bit-lock",
    "korean-iso-8bit",
    "ks_c_5601-1987",
    "lao",
    "latin-0",
    "latin-1",
    "latin-10",
    "latin-2",
    "latin-3",
    "latin-4",
    "latin-5",
    "latin-6",
    "latin-7",
    "latin-8",
    "latin-9",
    "mac-roman",
    "macintosh",
    "mik",
    "mule-utf-8",
    "next",
    "no-conversion",
    "no-conversion-multibyte",
    "old-jis",
    "prefer-utf-8",
    "pt154",
    "raw-text",
    "roman8",
    "ruscii",
    "shift_jis",
    "shift_jis-2004",
    "sjis",
    "tcvn",
    "tcvn-5712",
    "th-tis620",
    "thai-tis620",
    "tibetan",
    "tibetan-iso-8bit",
    "tis-620",
    "tis620",
    "undecided",
    "us-ascii",
    "utf-16",
    "utf-16-be",
    "utf-16-le",
    "utf-16be",
    "utf-16be-with-signature",
    "utf-16le",
    "utf-16le-with-signature",
    "utf-7",
    "utf-7-imap",
    "utf-8",
    "utf-8-auto",
    "utf-8-emacs",
    "utf-8-hfs",
    "utf-8-nfd",
    "utf-8-with-signature",
    "vietnamese-tcvn",
    "vietnamese-viqr",
    "vietnamese-viscii",
    "vietnamese-vscii",
    "viqr",
    "viscii",
    "vscii",
    "windows-1250",
    "windows-1251",
    "windows-1252",
    "windows-1253",
    "windows-1254",
    "windows-1255",
    "windows-1256",
    "windows-1257",
    "windows-1258",
    "windows-936",
    "x-ctext",
    "x-ctext-with-extensions",
];

pub(crate) fn coding_known(i: &Interp, v: &Value) -> Option<String> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    let base = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    if CODING_SYSTEMS.contains(&name.as_str())
        || CODING_SYSTEMS.contains(&base)
        || i.extra_coding_systems.iter().any(|n| *n == name)
    {
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

fn f_terminal_coding_system(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU tty default: utf-8 with unix EOL.  Optional TERMINAL is
    // terminal-live-p-checked.
    if let Some(v) = a.first() {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(Value::Sym(i.intern("utf-8-unix")))
}

fn f_detect_coding_string(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![Value::Sym(i.intern("undecided"))]))
}

fn f_arg0(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a[0].clone())
}

fn f_arg1(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a[1].clone())
}

/// `x-display-*'/`x-server-*'/`xw-*' on a non-Nextstep display: GNU
/// checks the arg (frame-live-p), then signals the NS error — with a
/// terminal object the message names the terminal.
fn f_ns_display(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => {
            Err(i.error("Nextstep windows are not in use or not initialized"))
        }
        Some(v) if crate::editor::is_terminal(i, v) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(Value::Frame(_)) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `image-type' — GNU maps a file name's extension via
/// `image-type-file-name-regexps'; unknown extension signals
/// `unknown-image-type', a non-string signals a plain error.
fn f_image_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Str(s) = &a[0] else {
        let shown = i.princ_to_string(&a[0]);
        return Err(i.error(format!("Invalid image file name ‘{shown}’")));
    };
    let name = s.borrow().clone();
    let ext = name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let has_dot = name.contains('.');
    let ty = if has_dot {
        match ext.as_str() {
            "png" => Some("png"),
            "gif" => Some("gif"),
            "jpg" | "jpeg" => Some("jpeg"),
            "webp" => Some("webp"),
            "bmp" => Some("bmp"),
            "xpm" => Some("xpm"),
            "pbm" => Some("pbm"),
            "xbm" => Some("xbm"),
            "ps" => Some("postscript"),
            "tif" | "tiff" => Some("tiff"),
            "svg" | "svgz" => Some("svg"),
            "heic" | "heif" | "heics" => Some("heic"),
            _ => None,
        }
    } else {
        None
    };
    match ty {
        Some(t) => Ok(Value::Sym(i.intern(t))),
        None => {
            let unk = i.intern("unknown-image-type");
            Err(i.signal_data(unk, vec![Value::string("Cannot determine image type")]))
        }
    }
}

/// `image-type-available-p' — the types our (fake) image support
/// claims: GNU batch reports all built-ins available.
fn f_image_type_available_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let ok = match &a[0] {
        Value::Sym(s) => {
            let n = i.symbol_name(*s);
            matches!(
                n.as_str(),
                "png" | "gif" | "jpeg" | "webp" | "bmp" | "xpm" | "pbm" | "xbm"
                    | "postscript" | "tiff" | "svg" | "heic"
            )
        }
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

/// `read-positioning-symbols' — like `read', but every symbol token
/// becomes a `symbol-with-pos' object.  GNU reports 1-based absolute
/// positions for buffers and 0-based read-relative ones for strings
/// and markers; non-stream args signal `invalid-function'.
fn f_read_positioning_symbols(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => {
            let Some(b) = i.current_buffer_ref() else {
                return Err(i.signal(crate::lisp::sym::END_OF_FILE, Value::Nil));
            };
            let (src, pos) = {
                let bb = b.borrow();
                (bb.text.text(), bb.point)
            };
            let r = i.read_from_string_pos(&src, pos, Some(pos as i128 + 1));
            match r {
                Ok((v, end)) => {
                    b.borrow_mut().set_point(end);
                    Ok(v)
                }
                // GNU's end-of-file signal data is the input stream.
                Err(e) => Err(eof_with_stream(e, &Value::Buffer(b))),
            }
        }
        Value::Buffer(b) => {
            let (src, pos) = {
                let bb = b.borrow();
                (bb.text.text(), bb.point)
            };
            let r = i.read_from_string_pos(&src, pos, Some(pos as i128 + 1));
            match r {
                Ok((v, end)) => {
                    b.borrow_mut().set_point(end);
                    Ok(v)
                }
                Err(e) => Err(eof_with_stream(e, &Value::Buffer(b))),
            }
        }
        Value::Str(s) => {
            let src = s.borrow().clone();
            let r = i.read_from_string_pos(&src, 0, Some(0));
            match r {
                Ok((v, _)) => Ok(v),
                Err(e) => Err(eof_with_stream(e, &Value::Str(s))),
            }
        }
        Value::Marker(m) => {
            let (buf, pos) = {
                let mm = m.borrow();
                (mm.buffer, mm.position)
            };
            match buf.and_then(|id| i.buffers.get(id)) {
                Some(b) => {
                    let src = b.borrow().text.text();
                    // GNU reports marker-read positions relative to the
                    // marker, zero-based.
                    let r = i.read_from_string_pos(&src, pos, Some(-(pos as i128)));
                    match r {
                        Ok((v, end)) => {
                            m.borrow_mut().position = end;
                            Ok(v)
                        }
                        Err(e) => Err(eof_with_stream(e, &Value::Marker(m))),
                    }
                }
                None => Err(i.signal_data(crate::lisp::sym::END_OF_FILE, vec![])),
            }
        }
        other => Err(i.signal_data(crate::lisp::sym::INVALID_FUNCTION, vec![other])),
    }
}

/// GNU's `end-of-file' signal carries the input stream as its datum.
fn eof_with_stream(e: Flow, stream: &Value) -> Flow {
    match e {
        Flow::Signal(Value::Sym(s), _, seen) if s == crate::lisp::sym::END_OF_FILE => {
            Flow::Signal(Value::Sym(s), Value::list(vec![stream.clone()]), seen)
        }
        other => other,
    }
}

/// `tooltip-mode' — minor-mode semantics: no arg reports, 'toggle
/// flips, numeric <= 0 disables, else enables.  State lives in the
/// `tooltip-mode' variable (seeded t like GNU).
fn f_tooltip_mode(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = i.intern("tooltip-mode");
    let cur = i.symbol_value(sid).truthy();
    let on = match arg(&a, 0) {
        Value::Nil => cur,
        Value::Sym(s) if i.symbol_name(s) == "toggle" => !cur,
        Value::Int(n) => n > 0,
        _ => true,
    };
    let v = Value::from_bool(on);
    let _ = i.set_symbol(sid, v.clone());
    Ok(v)
}

/// `command-error-default-function' — print "CONTEXT<msg>" for the
/// error data, like GNU's batch error report.  In a noninteractive
/// session this is the toplevel death path: GNU kills the batch job
/// (exit status 255, i.e. kill-emacs -1).
fn f_command_error_default(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let msg = error_message(i, &a[0]);
    let context = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    eprintln!("{context}{msg}");
    if i.noninteractive {
        return Err(crate::lisp::error::Flow::Exit(-1));
    }
    Ok(Value::Nil)
}

/// `window-preserve-size' — returns (BUFFER HSIZE WSIZE).
fn f_window_preserve_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i),
        Value::Window(w) => Some(w),
        other => return Err(i.wrong_type_mut("window-live-p", &other)),
    };
    let buf = match w {
        Some(w) => i
            .buffer_value(w.borrow().buffer)
            .unwrap_or(Value::Nil),
        None => Value::Nil,
    };
    Ok(Value::list(vec![buf, Value::Nil, Value::Nil]))
}

/// `thread-signal' — threadp-check THREAD; nothing to deliver in our
/// single-threaded evaluator.
fn f_thread_signal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_thread(i, &a[0])?;
    Ok(Value::Nil)
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
pub(crate) fn char_table_vec(v: &Value) -> Option<Rc<RefCell<Vec<Value>>>> {
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

pub(crate) fn is_char_table(i: &Interp, v: &Value) -> bool {
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

fn f_char_table_range(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: when a slot is nil, the parent chain supplies the value.
    let mut cur = a[0].clone();
    let mut guard = 0;
    loop {
        if let Some(v) = char_table_vec(&cur) {
            let idx = match &a[1] {
                Value::Int(n) if *n >= 0 => *n as usize,
                Value::Nil => 0,
                _ => return Ok(v.borrow().first().cloned().unwrap_or(Value::Nil)),
            };
            let got = v.borrow().get(idx).cloned().unwrap_or(Value::Nil);
            if !got.is_nil() {
                return Ok(got);
            }
        } else {
            return Ok(Value::Nil);
        }
        guard += 1;
        if guard > 32 {
            return Ok(Value::Nil);
        }
        let id = match &cur {
            Value::Record(r) => Rc::as_ptr(r) as usize,
            Value::Vec(r) => Rc::as_ptr(r) as usize,
            _ => return Ok(Value::Nil),
        };
        match i.char_table_parents.iter().find(|(k, _)| *k == id) {
            Some((_, p)) => cur = p.clone(),
            None => return Ok(Value::Nil),
        }
    }
}

fn f_char_table_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let id = match &a[0] {
        Value::Record(r) => Rc::as_ptr(r) as usize,
        _ => return Ok(Value::Nil),
    };
    Ok(i
        .char_table_parents
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| v.clone())
        .unwrap_or(Value::Nil))
}

fn f_set_char_table_parent(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    if !a[1].is_nil() && !is_char_table(i, &a[1]) {
        return Err(i.wrong_type_mut("char-table-p", &a[1]));
    }
    let id = match &a[0] {
        Value::Record(r) => Rc::as_ptr(r) as usize,
        _ => return Ok(a[1].clone()),
    };
    i.char_table_parents.retain(|(k, _)| *k != id);
    if !a[1].is_nil() {
        i.char_table_parents.push((id, a[1].clone()));
    }
    Ok(a[1].clone())
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

fn f_char_table_extra_slot(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Record(r) = &a[0] else {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    };
    let n = match &a[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    Ok(r.borrow().get(3 + n).cloned().unwrap_or(Value::Nil))
}

fn f_set_char_table_extra_slot(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let Value::Record(r) = &a[0] else {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    };
    let n = match &a[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let mut rr = r.borrow_mut();
    while rr.len() <= 3 + n {
        rr.push(Value::Nil);
    }
    rr[3 + n] = a[2].clone();
    Ok(a[2].clone())
}

fn f_record(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::Record(Rc::new(RefCell::new(a))))
}

fn f_recordp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(a[0], Value::Record(_))))
}

// ---------- added GNU compat subrs ----------

fn seq_len(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    Ok(match v {
        Value::Nil => 0,
        Value::Cons(_) => v.list_to_vec().map(|x| x.len() as i128).unwrap_or(-1),
        Value::Str(s) => s.borrow().chars().count() as i128,
        Value::Vec(x) | Value::Record(x) => x.borrow().len() as i128,
        Value::Hash(h) => h.borrow().map.len() as i128,
        _ => return Err(i.wrong_type_mut("sequencep", v)),
    })
}

fn length_cmp(i: &mut Interp, a: &[Value], cmp: i8) -> EvalResult {
    let n = seq_len(i, &a[0])?;
    let len = match &a[1] {
        Value::Int(x) => *x,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    Ok(Value::from_bool(match cmp {
        -1 => n < len,
        1 => n > len,
        _ => n == len,
    }))
}

fn f_length_lt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    length_cmp(i, &a, -1)
}
fn f_length_gt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    length_cmp(i, &a, 1)
}
fn f_length_eq(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    length_cmp(i, &a, 0)
}

/// Type rank for `value<'.
fn value_rank(v: &Value) -> u8 {
    match v {
        Value::Int(_) | Value::Float(_) => 0,
        Value::Sym(_) => 1,
        Value::Str(_) => 2,
        Value::Cons(_) => 3,
        Value::Vec(_) | Value::Record(_) => 4,
        Value::Hash(_) => 5,
        _ => 6,
    }
}

fn f_value_lt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (x, y) = (&a[0], &a[1]);
    let (rx, ry) = (value_rank(x), value_rank(y));
    let lt = if rx != ry {
        rx < ry
    } else {
        match (x, y) {
            (Value::Int(p), Value::Int(q)) => p < q,
            (Value::Int(p), Value::Float(q)) => (*p as f64) < *q,
            (Value::Float(p), Value::Int(q)) => *p < (*q as f64),
            (Value::Float(p), Value::Float(q)) => p < q,
            (Value::Sym(p), Value::Sym(q)) => i.symbol_name(*p) < i.symbol_name(*q),
            (Value::Str(p), Value::Str(q)) => *p.borrow() < *q.borrow(),
            _ => false,
        }
    };
    Ok(Value::from_bool(lt))
}

fn f_seconds_to_time(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let us = lisp_time_to_us(i, &a[0])?;
    Ok(us_to_lisp_time(us))
}

fn f_time_since(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = lisp_time_to_us(i, &a[0])?;
    let now = lisp_time_to_us(i, &Value::Nil)?;
    Ok(us_to_lisp_time(now - from))
}

fn f_time_to_days(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let us = lisp_time_to_us(i, &a[0])?;
    // Absolute date: days since 1 Jan 1 AD. 719163 is the day number
    // of the Unix epoch (1970-01-01).
    Ok(Value::Int((us / 1_000_000) / 86400 + 719163))
}

fn f_time_to_day_in_year(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let us = lisp_time_to_us(i, &a[0])?;
    let tm = local_tm((us / 1_000_000) as i64);
    Ok(Value::Int(tm.tm_yday as i128 + 1))
}

fn f_days_to_time(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let days = match &a[0] {
        Value::Int(n) => *n,
        Value::Float(f) => *f as i128,
        other => return Err(i.wrong_type_mut("numberp", other)),
    };
    // GNU returns the (HIGH LOW) seconds form, not a full time value.
    let secs = days * 86400;
    Ok(Value::list(vec![
        Value::Int(secs.div_euclid(65536)),
        Value::Int(secs.rem_euclid(65536)),
    ]))
}

fn f_date_leap_year_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let y = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    Ok(Value::from_bool(
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0),
    ))
}

fn f_version_list_not_zero(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU returns the first non-zero element, or 0 when all are zero.
    Ok(a[0]
        .list_to_vec()
        .unwrap_or_default()
        .into_iter()
        .find(|v| !matches!(v, Value::Int(0)))
        .unwrap_or(Value::Int(0)))
}

fn f_memory_use_counts(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // (CONSES FLOATS VECTOR-CELLS SYMBOLS STRING-CHARS INTERVALS STRINGS)
    let bufs = i.buffers.list().len() as i128;
    Ok(Value::list(vec![
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
        Value::Int(bufs),
    ]))
}

fn f_group_gid(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    unsafe extern "C" {
        fn getegid() -> u32;
    }
    Ok(Value::Int(unsafe { getegid() } as i128))
}

fn f_group_real_gid(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    unsafe extern "C" {
        fn getgid() -> u32;
    }
    Ok(Value::Int(unsafe { getgid() } as i128))
}

fn f_system_users(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // User names from the passwd database (like GNU's getpwent walk).
    let names: Vec<Value> = std::fs::read_to_string("/etc/passwd")
        .map(|txt| {
            txt.lines()
                .filter_map(|l| l.split(':').next())
                .filter(|n| !n.is_empty() && !n.starts_with('#'))
                .map(Value::string)
                .collect()
        })
        .unwrap_or_default();
    Ok(Value::list(names))
}

fn f_bufferpos_to_filepos(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    // Unibyte buffers: file byte = (clamped position) - 1.
    let zv = i
        .current_buffer_ref()
        .map(|b| b.borrow().text_len())
        .unwrap_or(0) as i128
        + 1;
    Ok(Value::Int((pos.clamp(1, zv)) - 1))
}

fn f_filepos_to_bufferpos(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    let zv = i
        .current_buffer_ref()
        .map(|b| b.borrow().text_len())
        .unwrap_or(0) as i128
        + 1;
    Ok(if pos + 1 <= zv {
        Value::Int(pos + 1)
    } else {
        Value::Nil
    })
}

fn f_set_buffer_multibyte(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Buffers are unibyte-capable; return the flag like Emacs does.
    Ok(a[0].clone())
}

fn f_window_with_parameter(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = (i, a);
    // Windows carry no parameters in this build.
    Ok(Value::Nil)
}

fn f_message_box(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    crate::lisp::builtins::evalfn::f_message(i, a)
}

fn f_secure_hash_algorithms(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let names = ["md5", "sha1", "sha224", "sha256", "sha384", "sha512"];
    Ok(Value::list(
        names.iter().map(|n| Value::Sym(i.intern(n))).collect(),
    ))
}

fn f_primitive_function_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = i.indirect_function_value(&a[0]);
    Ok(Value::from_bool(matches!(f, Value::Subr(_))))
}

fn f_read_expression(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (read--expression PROMPT &optional INITIAL-CONTENTS) — read a
    // string from the minibuffer, then `read' one form from it.
    let prompt = match a.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => String::new(),
    };
    let input = if i.minibuf_reader.is_some() {
        i.minibuf_line(&prompt)?
    } else {
        match a.get(1) {
            Some(Value::Str(s)) => s.borrow().clone(),
            _ => {
                let eof = i.intern("end-of-file");
                return Err(i.signal_data(eof, vec![Value::string("End of file during parsing")]));
            }
        }
    };
    let mut r = crate::lisp::reader::Reader::new(i, &input);
    match r.read() {
        Ok(Some(v)) => Ok(v),
        Ok(None) => {
            let eof = i.intern("end-of-file");
            Err(i.signal_data(eof, vec![Value::string("End of file during parsing")]))
        }
        Err(f) => Err(f),
    }
}

fn f_keymap_of(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if is_keymap(i, &a[0]) {
        Ok(a[0].clone())
    } else {
        Ok(Value::Nil)
    }
}

fn f_one(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(1))
}

/// Parse a color to 16-bit (0-65535) RGB like GNU: "#rgb"/"#rrggbb"
/// strings, the 8 standard color names, or a (R G B) list.
fn parse_color_16(v: &Value) -> Option<(i64, i64, i64)> {
    if let Value::Str(s) = v {
        let t = s.borrow().clone();
        let h = t.trim_start_matches('#');
        if t.starts_with('#') && h.len() == 6 {
            let r = i64::from_str_radix(&h[0..2], 16).ok()? * 257;
            let g = i64::from_str_radix(&h[2..4], 16).ok()? * 257;
            let b = i64::from_str_radix(&h[4..6], 16).ok()? * 257;
            return Some((r, g, b));
        }
        if t.starts_with('#') && h.len() == 3 {
            let mut it = h.chars().filter_map(|c| c.to_digit(16));
            let (r, g, b) = (it.next()?, it.next()?, it.next()?);
            return Some((
                (r * 65535 / 15) as i64,
                (g * 65535 / 15) as i64,
                (b * 65535 / 15) as i64,
            ));
        }
        // Standard color names (tty-color-standard-values).
        let rgb = match t.as_str() {
            "black" => (0, 0, 0),
            "red" => (65535, 0, 0),
            "green" => (0, 65535, 0),
            "yellow" => (65535, 65535, 0),
            "blue" => (0, 0, 65535),
            "magenta" => (65535, 0, 65535),
            "cyan" => (0, 65535, 65535),
            "white" => (65535, 65535, 65535),
            _ => return None,
        };
        return Some(rgb);
    }
    if let Value::Cons(_) = v {
        let items = v.list_to_vec().ok()?;
        if items.len() == 3 {
            let mut out = [0i64; 3];
            for (k, item) in items.iter().enumerate() {
                out[k] = match item {
                    Value::Int(n) => (*n).clamp(0, 65535) as i64,
                    _ => return None,
                };
            }
            return Some((out[0], out[1], out[2]));
        }
    }
    None
}

fn f_color_distance(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Riemersma's "colour metric" on 16-bit components, as in GNU.
    let (c1, c2) = match (parse_color_16(&a[0]), parse_color_16(&a[1])) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            let err = i.intern("error");
            return Err(i.signal_data(err, vec![Value::string("Invalid color"), a[0].clone()]));
        }
    };
    let r = c1.0 - c2.0;
    let g = c1.1 - c2.1;
    let b = c1.2 - c2.2;
    let r_mean = (c1.0 + c2.0) >> 1;
    let d = ((((2 * 65536 + r_mean) * r * r) >> 16)
        + 4 * g * g
        + (((2 * 65536 + 65535 - r_mean) * b * b) >> 16))
        >> 16;
    Ok(Value::Int(d as i128))
}

fn sel_frame_dims(i: &Interp) -> (i128, i128) {
    match &i.selected_frame {
        Some(f) => {
            let ff = f.borrow();
            (ff.width as i128, ff.height as i128)
        }
        None => (80, 25),
    }
}

/// One monitor's attribute alist, shaped like GNU's tty result.
fn monitor_attributes(i: &mut Interp) -> Value {
    let (w, h) = sel_frame_dims(i);
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => Value::Nil,
    };
    let mut rect = |k: &str| -> Value {
        Value::list(vec![
            Value::Sym(i.intern(k)),
            Value::Int(0),
            Value::Int(0),
            Value::Int(w),
            Value::Int(h),
        ])
    };
    Value::list(vec![
        rect("geometry"),
        rect("workarea"),
        Value::list(vec![
            Value::Sym(i.intern("mm-size")),
            Value::Nil,
            Value::Nil,
        ]),
        Value::list(vec![Value::Sym(i.intern("frames")), fr]),
    ])
}

fn f_frame_monitor_attributes(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(monitor_attributes(i))
}

fn f_display_monitor_attributes_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![monitor_attributes(i)]))
}

fn f_locale_info(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let item = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        _ => return Ok(Value::Nil),
    };
    let lang = |n: i32| -> Option<String> {
        unsafe extern "C" {
            fn nl_langinfo(item: i32) -> *const std::ffi::c_char;
            fn setlocale(category: i32, locale: *const std::ffi::c_char)
            -> *const std::ffi::c_char;
        }
        // GNU calls setlocale(LC_ALL, "") at startup; do it lazily here.
        // LC_ALL is 6 on glibc, 0 on BSD/macOS.
        static ONCE: std::sync::Once = std::sync::Once::new();
        let lc_all: i32 = if cfg!(target_os = "linux") { 6 } else { 0 };
        ONCE.call_once(|| unsafe {
            setlocale(lc_all, c"".as_ptr());
        });
        let p = unsafe { nl_langinfo(n) };
        if p.is_null() {
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        if s.is_empty() { None } else { Some(s) }
    };
    // nl_item constants differ between glibc and BSD/macOS.
    let codeset: i32 = if cfg!(target_os = "linux") { 14 } else { 0 };
    let day1: i32 = if cfg!(target_os = "linux") {
        0x20007
    } else {
        7
    };
    let mon1: i32 = if cfg!(target_os = "linux") {
        0x2000e
    } else {
        21
    };
    match item.as_str() {
        "codeset" => Ok(lang(codeset).map(Value::string).unwrap_or(Value::Nil)),
        "days" => Ok(Value::Vec(Rc::new(RefCell::new(
            (0..7)
                .map(|d| lang(day1 + d).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )))),
        "months" => Ok(Value::Vec(Rc::new(RefCell::new(
            (0..12)
                .map(|m| lang(mon1 + m).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )))),
        _ => Ok(Value::Nil),
    }
}

fn f_frame_width_val(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(sel_frame_dims(i).0))
}
fn f_frame_height_val(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(sel_frame_dims(i).1))
}

fn f_x_parse_geometry(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU returns an alist ((height . H) (width . W) (top . Y) (left . X)).
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut left = None;
    let mut top = None;
    let mut w = None;
    let mut h = None;
    // The first sign starts the position part; sizes are digits and 'x'.
    let split_at = s.find(['+', '-']).unwrap_or(s.len());
    let (size, pos) = s.split_at(split_at);
    let mut it = size.split('x');
    if let Some(t) = it.next() {
        w = t.parse().ok();
    }
    if let Some(t) = it.next() {
        h = t.parse().ok();
    }
    // Position part is [+-]N[+-]N — left then top.
    let bytes = pos.as_bytes();
    let mut idx = 0;
    let mut k = 0;
    while idx < bytes.len() && k < 2 {
        let sign = match bytes[idx] {
            b'-' => -1i32,
            _ => 1i32,
        };
        if matches!(bytes[idx], b'+' | b'-') {
            idx += 1;
        }
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if let Ok(n) = pos[start..idx].parse::<i32>() {
            if k == 0 {
                left = Some(sign * n);
            } else {
                top = Some(sign * n);
            }
            k += 1;
        }
    }
    let mut items: Vec<Value> = Vec::new();
    for (name, v) in [("height", h), ("width", w), ("top", top), ("left", left)] {
        if let Some(n) = v {
            items.push(Value::cons(
                Value::Sym(i.intern(name)),
                Value::Int(n as i128),
            ));
        }
    }
    Ok(Value::list(items))
}

// ---------- internals & platform stubs (probe-matched arities) ----------

fn f_current_input_mode(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU batch: (INTERRUPT FLOW META QUIT) = (t nil t 7).
    Ok(Value::list(vec![
        Value::t(),
        Value::Nil,
        Value::t(),
        Value::Int(7),
    ]))
}

fn f_current_bidi_paragraph_direction(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Sym(i.intern("left-to-right")))
}

fn f_read_non_nil_coding_system(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Batch read returns the default coding system.
    Ok(Value::Sym(i.intern("utf-8")))
}

fn f_find_operation_coding_system(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU batch resolves to (undecided) for generic operations.
    Ok(Value::list(vec![Value::Sym(i.intern("undecided"))]))
}

fn f_lossage_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Int(n) if *n >= 100 => {}
            _ => {
                return Err(i.signal_data(
                    sym::USER_ERROR,
                    vec![Value::string("Value must be >= 100")],
                ));
            }
        }
    }
    Ok(Value::Int(300))
}

fn f_mouse_position_root(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::cons(Value::Int(0), Value::Int(0)))
}

fn f_window_config_pred_err(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // We have no window-configuration objects; every argument fails the
    // type check like GNU's window-configuration-p.
    let v = a.first().cloned().unwrap_or(Value::Nil);
    Err(i.wrong_type_mut("window-configuration-p", &v))
}

fn f_make_terminal_frame(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Err(i.error("Don't know how to create a terminal frame"))
}

fn f_subr_native_comp_unit(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: arg must satisfy `subrp'; a C subr without a comp unit → nil.
    match &a[0] {
        Value::Subr(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}

fn f_define_coding_system_alias(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_sym(i, &a[0])?;
    let _ = want_sym(i, &a[1])?;
    if coding_known(i, &a[1]).is_none() {
        let cs_err = i.intern("coding-system-error");
        return Err(i.signal_data(cs_err, vec![a[0].clone(), a[1].clone()]));
    }
    Ok(Value::Nil)
}

fn f_comp_el_to_eln_filename(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_string(i, &a[0])?;
    if !std::path::Path::new(&path).exists() {
        return Err(i.signal_data(
            sym::FILE_MISSING,
            vec![
                Value::string("Applying native-compiler to missing file"),
                a[0].clone(),
            ],
        ));
    }
    let base = std::path::Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("anon.el")
        .trim_end_matches(".el")
        .to_string();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    Ok(Value::string(format!(
        "{}/.emacs.d/eln-cache/remacs/{}-{:x}.eln",
        home,
        base,
        path.len()
    )))
}

fn f_thread_buffer_disposition(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.first() {
        Some(Value::Thread(_)) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("threadp", other.unwrap_or(&Value::Nil))),
    }
}

fn f_thread_set_buffer_disposition(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match a.first() {
        Some(Value::Thread(_)) => {}
        other => {
            return Err(i.wrong_type_mut("threadp", other.unwrap_or(&Value::Nil)));
        }
    }
    // GNU only accepts nil as the disposition.
    if !a[1].is_nil() {
        return Err(i.wrong_type_mut("null", &a[1]));
    }
    Ok(Value::Nil)
}

fn f_internal_default_signal_process(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Accepts a pid (int); nothing to signal here → -1 like GNU.
    let _ = want_int(i, &a[0])?;
    Ok(Value::Int(-1))
}

/// `thread--blocker` — GNU checks THREADP; nil (no blocker tracked).
fn f_thread_blocker(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_thread(i, &a[0])?;
    Ok(Value::Nil)
}

/// `optimize-char-table` — GNU char-table-p-checks TABLE; ours are
/// already flat, so nil.
fn f_optimize_char_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    Ok(Value::Nil)
}

/// `glyph-char`/`glyph-face` — GNU Lisp accessors over glyph codes:
/// a plain char maps to itself / no face.
fn f_glyph_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        v @ Value::Int(_) => Ok(v.clone()),
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

fn f_glyph_face(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Int(_) => Ok(Value::Nil),
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

/// `font-spec` — build a font-spec record `#s(font-spec PROPS)'.
fn f_font_spec(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::Record(Rc::new(RefCell::new(vec![
        Value::Sym(i.intern("font-spec")),
        Value::list(a),
    ]))))
}

/// `put-image` — GNU requires an image spec (list headed `image') and
/// returns a fresh overlay at POS.
fn f_put_image(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let image_sym = i.intern("image");
    let ok = match &a[0] {
        Value::Cons(c) => i.sym_is(&c.borrow().car, image_sym),
        _ => false,
    };
    if !ok {
        let shown = i.prin1_to_string(&a[0]);
        return Err(i.error(format!("Not an image: {shown}")));
    }
    let pos = a[1].clone();
    crate::editor::f_make_overlay(i, vec![pos.clone(), pos, Value::Nil])
}

/// `pdumper-stats` — GNU returns an alist; we were never dumped.
fn f_pdumper_stats(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::list(vec![
        Value::cons(
            Value::Sym(i.intern("dumped-with-pdumper")),
            Value::Nil,
        ),
        Value::cons(Value::Sym(i.intern("load-time")), Value::Float(0.0)),
        Value::cons(Value::Sym(i.intern("dump-file-name")), Value::Nil),
    ]))
}

/// `set-file-acl` — best-effort like GNU: validate strings, no ACL
/// support → nil.
fn f_set_file_acl(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = want_string(i, &a[0])?;
    let _ = want_string(i, &a[1])?;
    Ok(Value::Nil)
}

/// `mapbacktrace` — GNU calls FUNCTION with (EVALD FUNC ARGS FLAGS)
/// for each live frame, innermost first, then returns nil.
fn f_mapbacktrace(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fun = a[0].clone();
    let t = Value::Sym(sym::T);
    let frames: Vec<(Value, Vec<Value>)> = i.lisp_stack.iter().rev().cloned().collect();
    for (fval, args) in frames {
        i.apply(&fun, vec![t.clone(), fval, Value::list(args), Value::Nil])?;
    }
    Ok(Value::Nil)
}

fn f_internal_default_interrupt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: nil arg means "the process of the current buffer".
    match a.first() {
        Some(Value::Process(_)) => Ok(Value::Nil),
        _ => {
            let name = i
                .current_buffer_ref()
                .map(|b| b.borrow().name.clone())
                .unwrap_or_else(|| "*scratch*".into());
            Err(i.error(format!("Buffer {name} has no process")))
        }
    }
}

fn f_process_arg_err(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let v = a.first().cloned().unwrap_or(Value::Nil);
    Err(i.wrong_type_mut("processp", &v))
}

fn f_internal_handle_focus_in(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU validates EVENT is a (focus-in/out LIVE-FRAME) cons.
    let ok = match &a[0] {
        Value::Cons(c) => {
            let (car, cadr) = {
                let cc = c.borrow();
                let cadr = match &cc.cdr {
                    Value::Cons(d) => d.borrow().car.clone(),
                    _ => Value::Nil,
                };
                (cc.car.clone(), cadr)
            };
            let head_ok = match &car {
                Value::Sym(s) => {
                    let n = i.symbol_name(*s);
                    n == "focus-in" || n == "focus-out"
                }
                _ => false,
            };
            head_ok && matches!(cadr, Value::Frame(_))
        }
        _ => false,
    };
    if ok {
        Ok(Value::Nil)
    } else {
        Err(i.error("Invalid focus-in event"))
    }
}

fn f_set_font_selection_order(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU requires a proper list of font-driver symbols.
    let ok = match a[0].list_to_vec() {
        Ok(items) => !items.is_empty() && items.iter().all(|v| matches!(v, Value::Sym(_))),
        Err(_) => false,
    };
    if ok {
        Ok(Value::Nil)
    } else {
        Err(i.error("Invalid font sort order"))
    }
}

fn f_clear_image_cache(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU requires a window-system frame; our batch/tty has none.
    Err(i.error("Window system frame should be used"))
}

fn f_query_fontset(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU batch: fontsets need a window system.
    Err(i.error("Window system is not in use or not initialized"))
}

fn f_close_font(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // No font objects exist; GNU type-checks arg0 as font-object.
    Err(i.wrong_type_mut("font-object", &a[0]))
}

fn f_define_fringe_bitmap(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU returns the bitmap name symbol.
    Ok(a[0].clone())
}

fn f_re_describe_compiled(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Str(_) | Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_move_file_to_trash(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let path = want_string(i, &a[0])?;
    let src = std::path::Path::new(&path);
    if !src.exists() {
        return Err(i.signal_data(
            sym::FILE_MISSING,
            vec![Value::string("Removing old name"), a[0].clone()],
        ));
    }
    // freedesktop trash: ~/.local/share/Trash/{files,info}
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let trash = std::path::Path::new(&home).join(".local/share/Trash");
    let files = trash.join("files");
    let info = trash.join("info");
    let _ = std::fs::create_dir_all(&files);
    let _ = std::fs::create_dir_all(&info);
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed");
    let mut dest = files.join(name);
    let mut n = 1;
    while dest.exists() {
        dest = files.join(format!("{}.{}", name, n));
        n += 1;
    }
    if let Err(e) = std::fs::rename(src, &dest) {
        return Err(i.error(format!("Trashing {}: {}", path, e)));
    }
    if let Some(stem) = dest.file_name().and_then(|s| s.to_str()) {
        let _ = std::fs::write(
            info.join(format!("{}.trashinfo", stem)),
            format!("[Trash Info]\nPath={}\nDeletionDate=0\n", path),
        );
    }
    Ok(Value::Nil)
}

/// `move-to-window-line` — with no window system GNU returns 0 without
/// even type-checking ARG (observed: `(move-to-window-line 'x)` → 0).
fn f_move_to_window_line(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}

/// `network-lookup-address-info` — real getaddrinfo; GNU shape:
/// IPv4 → [A B C D 0], IPv6 → [S0..S7 0]; nil on lookup failure.
fn f_network_lookup_address_info(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let host = want_string(i, &a[0])?;
    let mut want4 = true;
    let mut want6 = true;
    if let Some(fam) = a.get(1) {
        if !fam.is_nil() {
            match i.sym_id(fam).map(|s| i.symbol_name(s).to_string()).as_deref() {
                Some("ipv4") => want6 = false,
                Some("ipv6") => want4 = false,
                _ => {
                    let e = i.intern("error");
                    return Err(i.signal_data(
                        e,
                        vec![Value::string("Unsupported family")],
                    ));
                }
            }
        }
    }
    use std::net::ToSocketAddrs;
    let addrs: Vec<std::net::SocketAddr> = (host.as_str(), 0u16)
        .to_socket_addrs()
        .map(|it| it.collect())
        .unwrap_or_default();
    // GNU dedupes; keep first occurrences in order.
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for sa in addrs {
        if !seen.insert(sa) {
            continue;
        }
        match sa {
            std::net::SocketAddr::V4(v4) if want4 => {
                let o = v4.ip().octets();
                out.push(Value::Vec(Rc::new(RefCell::new(vec![
                    Value::Int(o[0] as i128),
                    Value::Int(o[1] as i128),
                    Value::Int(o[2] as i128),
                    Value::Int(o[3] as i128),
                    Value::Int(0),
                ]))));
            }
            std::net::SocketAddr::V6(v6) if want6 => {
                let mut elts: Vec<Value> = v6
                    .ip()
                    .segments()
                    .iter()
                    .map(|s| Value::Int(*s as i128))
                    .collect();
                elts.push(Value::Int(0));
                out.push(Value::Vec(Rc::new(RefCell::new(elts))));
            }
            _ => {}
        }
    }
    Ok(Value::list(out))
}

/// `color-values-from-color-spec` — parse #RGB/#RRGGBB/#RRRGGGBBB/
/// #RRRRGGGGBBBB into 16-bit channel values; named colors need a
/// display (nil in batch), non-strings get stringp.
fn f_color_values_from_color_spec(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let hex = s.strip_prefix('#').unwrap_or("");
    if !s.starts_with('#') || ![3, 4, 6, 9, 12].contains(&hex.len()) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(Value::Nil);
    }
    let n = hex.len() / 3;
    let scale = |chunk: &str| -> i128 {
        let v = i128::from_str_radix(chunk, 16).unwrap_or(0);
        // Scale N-digit channel to 16 bits: v * 65535 / (16^n - 1).
        v * 65535 / ((1i128 << (4 * n)) - 1)
    };
    Ok(Value::list(vec![
        Value::Int(scale(&hex[0..n])),
        Value::Int(scale(&hex[n..2 * n])),
        Value::Int(scale(&hex[2 * n..3 * n])),
    ]))
}

/// `file-selinux-context` — no SELinux: GNU shape (nil nil nil nil).
fn f_file_selinux_context(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_string(i, &a[0])?;
    Ok(Value::list(vec![
        Value::Nil,
        Value::Nil,
        Value::Nil,
        Value::Nil,
    ]))
}

/// `garbage-collect-heapsize` — GNU-shaped stats alist; we don't track
/// per-type counts, so report what we know and zeros elsewhere.
fn f_gc_heapsize(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let syms = i.obarray.all_ids().len() as i128;
    let bufs = i.buffers.list().len() as i128;
    let names: Vec<crate::lisp::value::SymId> = [
        "conses",
        "symbols",
        "strings",
        "string-bytes",
        "vectors",
        "vector-slots",
        "floats",
        "intervals",
        "buffers",
    ]
    .iter()
    .map(|n| i.intern(n))
    .collect();
    let mk = |idx: usize, unit: i128, used: i128| {
        Value::list(vec![
            Value::Sym(names[idx]),
            Value::Int(unit),
            Value::Int(used),
            Value::Int(0),
        ])
    };
    Ok(Value::list(vec![
        mk(0, 16, 0),
        mk(1, 48, syms),
        mk(2, 32, 0),
        Value::list(vec![Value::Sym(names[3]), Value::Int(1), Value::Int(0)]),
        mk(4, 16, 0),
        mk(5, 8, 0),
        mk(6, 8, 0),
        mk(7, 56, 0),
        Value::list(vec![Value::Sym(names[8]), Value::Int(1064), Value::Int(bufs)]),
    ]))
}

/// `make-closure` — only valid on byte-code prototypes, which we don't
/// have; GNU signals wrong-type-argument byte-code-function-p.
fn f_make_closure(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Err(i.wrong_type_mut("byte-code-function-p", &a[0]))
}

/// `bidi-find-overridden-directionality` — STRING is arg 2 (GNU
/// stringp-checks it); no bidi support → nil.
fn f_bidi_find_overridden(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_string(i, &a[2])?;
    Ok(Value::Nil)
}

/// `bidi-resolved-levels` — GNU fixnump-checks its arg; nil without bidi.
fn f_bidi_resolved_levels(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(v) = a.first() {
        if !v.is_nil() && !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("fixnump", v));
        }
    }
    Ok(Value::Nil)
}

/// `composition-sort-rules` — GNU listp-checks RULES; nil.
fn f_composition_sort_rules(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_list(i, &a[0])?;
    Ok(Value::Nil)
}

/// `remember-mouse-glyph` — GNU checks arg 0: nil selects the current
/// frame then fails the window-system check; a live frame → nil.
fn f_remember_mouse_glyph(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Nil => Err(i.error("Window system frame should be used")),
        Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `set-terminal-coding-system-internal` — GNU terminal-live_p-checks
/// the optional TERMINAL arg.
fn f_set_terminal_coding(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(t) = a.get(1) {
        match t {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(Value::Nil)
}

/// `module-load` — no dynamic-module support; GNU signals error with
/// the dlopen message as data.
fn f_module_load(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = want_string(i, &a[0])?;
    let e = i.intern("error");
    Err(i.signal_data(
        e,
        vec![
            Value::string(f.clone()),
            Value::string(format!("dlopen({}): module support not in this build", f)),
        ],
    ))
}

/// `native-elisp-load` — no native compiler; GNU errors when the file
/// is absent (message literally says "does not exists").
fn f_native_elisp_load(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = want_string(i, &a[0])?;
    let e = i.intern("error");
    if !std::path::Path::new(&f).exists() {
        return Err(i.signal_data(
            e,
            vec![Value::string("file does not exists"), Value::string(f)],
        ));
    }
    Err(i.signal_data(
        e,
        vec![Value::string("native compilation not in this build")],
    ))
}

/// `backtrace--frames-from-thread` — threadp-checks its arg; we keep
/// no suspended frames, so nil for a real thread.
fn f_backtrace_frames_from_thread(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_thread(i, &a[0])?;
    Ok(Value::Nil)
}

/// `backtrace--locals` — wholenump-checks arg 0; nil (no frame info).
fn f_backtrace_locals(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Int(n) if *n >= 0 => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("wholenump", other)),
    }
}

/// `backtrace-frame--internal` — GNU errors when no such frame exists.
fn f_backtrace_frame_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let e = i.intern("error");
    Err(i.signal_data(e, vec![a[0].clone()]))
}

/// `completion--flex-cost-gotoh` — GNU's affine-gap flex cost:
/// (COST POS1 POS2 ...) or nil when NEEDLE isn't a subsequence of
/// STRING. Empirical model: 5 if the match doesn't start at 0, plus
/// 9 + gap-len per interior gap; trailing text is free.
fn f_flex_cost_gotoh(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let needle = want_string(i, &a[0])?;
    let hay = want_string(i, &a[1])?;
    if needle.is_empty() || hay.is_empty() {
        return Ok(Value::Nil);
    }
    let hc: Vec<char> = hay.chars().collect();
    let mut pos: Vec<usize> = Vec::new();
    let mut from = 0usize;
    for nc in needle.chars() {
        match hc[from..].iter().position(|c| *c == nc) {
            Some(off) => {
                pos.push(from + off);
                from += off + 1;
            }
            None => return Ok(Value::Nil),
        }
    }
    let mut cost: i128 = if pos[0] > 0 { 5 } else { 0 };
    for w in pos.windows(2) {
        let gap = w[1] - w[0] - 1;
        if gap > 0 {
            cost += 9 + gap as i128;
        }
    }
    let mut out = vec![Value::Int(cost)];
    out.extend(pos.iter().map(|p| Value::Int(*p as i128)));
    Ok(Value::list(out))
}

/// `profiler-cpu-start` — flag only, no real sampler; GNU returns t.
fn f_profiler_cpu_start(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.cpu_profiler = true;
    Ok(Value::t())
}

fn f_profiler_cpu_running_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(if i.cpu_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}

/// `profiler-cpu-stop` — GNU returns whether it was running.
fn f_profiler_cpu_stop(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let was = i.cpu_profiler;
    i.cpu_profiler = false;
    Ok(if was { Value::t() } else { Value::Nil })
}

/// `check-coding-systems-region` — GNU validates START/END against
/// the current buffer's accessible range, then reports codings (nil).
fn f_check_coding_systems_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_int(i, &a[0])?;
    let e = want_int(i, &a[1])?;
    // Lisp positions are 1-based over the accessible region.
    let (lo, hi) = i
        .current_buffer_ref()
        .map(|b| {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.zv as i128 + 1)
        })
        .unwrap_or((1, 1));
    if s < lo || e > hi || s > e {
        let sym = i.intern("args-out-of-range");
        return Err(i.signal_data(sym, vec![a[0].clone(), a[1].clone()]));
    }
    Ok(Value::Nil)
}

/// `window-valid-p' check → nil.
fn f_window_valid_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}

fn f_window_valid_zero(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Int(0)),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}

/// `window-combined-p` — GNU signals a plain `error' for non-windows.
fn f_window_combined_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => {
            let shown = i.princ_to_string(&other);
            Err(i.error(format!("{shown} is not a valid window")))
        }
    }
}

/// `window-fixed-size-p` — GNU windowp-checks WINDOW; nil (nothing
/// fixed in our model).
fn f_windowp_nil(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("windowp", &other)),
    }
}

/// `window-safely-shrinkable-p` — batch tty: always safe → t.
fn f_safely_shrinkable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}

/// `window-main-window` — frame arg check, then the frame's main
/// (non-minibuffer) window; single-window frames → that window.
fn f_window_main_window(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let f = match arg(&a, 0) {
        Value::Nil => sel_frame(i),
        Value::Frame(fr) => Some(fr),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live frame")));
        }
    };
    let Some(f) = f else {
        return f_selected_window(i, vec![]);
    };
    let fb = f.borrow();
    let w = fb
        .windows
        .iter()
        .find(|w| !w.borrow().minibuffer)
        .or_else(|| fb.windows.first())
        .cloned();
    drop(fb);
    Ok(w.map(Value::Window).unwrap_or(Value::Nil))
}

/// `face-attributes-as-vector` — GNU maps a plist of `:attr value'
/// onto the 20-slot lface vector; anything else → all unspecified.
fn f_face_attributes_as_vector(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    const ATTRS: [&str; 20] = [
        "face", "family", "foundry", "width", "height", "weight", "slant",
        "underline", "inverse-video", "foreground", "background", "stipple",
        "overline", "strike-through", "box", "font", "inherit", "fontset",
        "distant-foreground", "extend",
    ];
    let unspec = i.intern("unspecified");
    let mut slots = vec![Value::Sym(unspec); 20];
    // Iterate plist pairs: (KEY VAL KEY VAL ...).
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (k, rest) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let (v, next) = match rest {
            Value::Cons(c2) => {
                let b = c2.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            _ => break,
        };
        if let Value::Sym(s) = &k {
            let name = i.symbol_name(*s);
            let bare = name.strip_prefix(':').unwrap_or(&name);
            if let Some(idx) = ATTRS.iter().position(|x| *x == bare) {
                if idx > 0 {
                    slots[idx] = if idx == 7 && v.truthy() && !v.is_nil() {
                        // GNU normalizes :underline to t/nil/plist.
                        Value::t()
                    } else {
                        v
                    };
                }
            }
        }
        cur = next;
    }
    Ok(Value::Vec(Rc::new(RefCell::new(slots))))
}

fn f_profiler_memory_start(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.memory_profiler = true;
    Ok(Value::t())
}

/// `profiler-memory-running-p` — whether the memory profiler is on.
fn f_profiler_memory_running_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(if i.memory_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}

/// `profiler-memory-stop` — GNU returns whether it was running.
fn f_profiler_memory_stop(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let was = i.memory_profiler;
    i.memory_profiler = false;
    Ok(if was { Value::t() } else { Value::Nil })
}

/// `profiler-memory-log` — nil when not running.
fn f_profiler_memory_log(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Nil)
}
