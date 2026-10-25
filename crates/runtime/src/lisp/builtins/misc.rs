//! Miscellaneous subrs: symbols, time values, hashing/crypto, file
//! attributes, environment, and small editor glue that doesn't belong
//! to a larger category module.

use std::cell::RefCell;
use std::rc::Rc;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};

use super::{S, arg, want_int, want_list, want_string, want_sym};
use crate::lisp::Interp;
use crate::lisp::eq_values;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Arity, Lambda, StrRef, Subr, SymId, Value};

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
        f_subr_arity,
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
        5,
        f_make_interpreted_closure,
        "Build a closure from args/env/body."
    ),
    // `obarray-clear' is registered in data.rs; a stub here previously
    // shadowed it.
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
    S!("map-keymap-internal", 2, 2, f_map_keymap_internal, ""),
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
    S!("encode-time", many 1,
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
        2,
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
        2,
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
        3,
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
        f_remove_pos_from_symbol,
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
        f_user_real_login_name,
        "Real login name."
    ),
    S!("user-full-name", 0, 1, f_user_full_name, "Full name."),
    S!("user-uid", 0, 0, f_user_uid, "Effective uid."),
    S!("user-real-uid", 0, 0, f_user_real_uid, "Real uid."),
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
    // `version-listp' does not exist in GNU Emacs 31.
    // ---------- predicates ----------
    S!("string-or-null-p", 1, 1, f_string_or_null_p, ""),
    S!("vector-or-char-table-p", 1, 1, f_vector_or_char_table_p, ""),
    S!("subr-native-elisp-p", 1, 1, f_subr_native_elisp_p, ""),
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
        "t if THREAD is alive (not yet exited)."
    ),
    S!(
        "make-thread",
        1,
        3,
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
    S!("thread-yield", 0, 0, f_thread_yield, "Yield to other threads."),
    S!(
        "thread-last-error",
        0,
        1,
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
    S!("condition-name", 1, 1, f_condition_name, "Name of CONDVAR."),
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
        1,
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
    S!(
        "bitmap-spec-p",
        1,
        1,
        f_bitmap_spec_p,
        "t if OBJECT is a bitmap spec."
    ),
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
        f_delete_other_windows_internal,
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
        5,
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
    S!(
        "zlib-available-p",
        0,
        0,
        f_zlib_available_p,
        "t if zlib decompression is available."
    ),
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
    S!(
        "insert-byte",
        2,
        3,
        f_insert_byte,
        "Insert COUNT copies of BYTE."
    ),
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
        f_newline_cache_check,
        "Check the newline cache for sanity."
    ),
    S!(
        "tab-bar-height",
        0,
        2,
        f_tab_bar_height,
        "Height of the tab bar."
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
    S!("debugger-trap", 0, 0, f_nil, "Trap into the debugger."),
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
    S!("record", many 1, f_record, "Create a record of TYPE with SLOTS."),
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
    S!(
        "command-error-default-function",
        3,
        3,
        f_command_error_default,
        ""
    ),
    S!("command-line", 0, 0, f_command_line, ""),
    S!("minibuffer-depth", 0, 0, f_minibuffer_depth, ""),
    S!("detect-coding-string", 1, 2, f_detect_coding_string, ""),
    S!("detect-coding-region", 2, 3, f_detect_coding_region, ""),
    S!("coding-system-list", 0, 1, f_coding_system_list, ""),
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
        f_coding_system_priority_list,
        ""
    ),
    S!("terminal-coding-system", 0, 1, f_terminal_coding_system, ""),
    S!("keyboard-coding-system", 0, 1, f_keyboard_coding_system, ""),
    // GNU 31: `file-name-coding-system' and `default-terminal-coding-system'
    // exist only as variables, not functions.
    S!("encode-coding-string", 2, 4, f_encode_coding_string, ""),
    S!("decode-coding-string", 2, 4, f_decode_coding_string, ""),
    S!("encode-coding-char", 2, 3, f_encode_coding_char, ""),
    S!("decode-coding-region", 3, 4, f_decode_coding_region, ""),
    S!("encode-coding-region", 3, 4, f_encode_coding_region, ""),
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
    S!("set-mouse-position", 3, 3, f_frame_live_nil, ""),
    S!("set-mouse-pixel-position", 3, 3, f_frame_live_nil, ""),
    S!("display-images-p", 0, 1, f_display_false, ""),
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
    S!("x-open-connection", 1, 3, f_x_open_connection, ""),
    S!("x-close-connection", 1, 1, f_x_close_connection, ""),
    S!("x-display-list", 0, 0, f_nil, ""),
    S!("xw-display-color-p", 0, 1, f_ns_display, ""),
    S!("xw-color-defined-p", 1, 2, f_xw_color_defined_p, ""),
    S!("color-gray-p", 1, 2, f_color_gray_p, ""),
    S!("color-supported-p", 1, 3, f_color_defined_p, ""),
    S!("invert-face", 1, 2, f_invert_face, ""),
    S!("clear-face-cache", 0, 1, f_nil, ""),
    // ---------- windows ----------
    S!("minibuffer-selected-window", 0, 0, f_nil, ""),
    // GNU 31: `window-min-height'/`window-min-width' are Lisp functions in
    // window.el (not loaded at -Q); the variables exist.
    S!("window-sizable", 2, 5, f_window_sizable, ""),
    S!("window-fixed-size-p", 0, 2, f_windowp_nil, ""),
    S!("fit-window-to-buffer", 0, 6, f_fit_window_to_buffer, ""),
    S!(
        "shrink-window-if-larger-than-buffer",
        0,
        1,
        f_shrink_if_larger,
        ""
    ),
    S!("window-safely-shrinkable-p", 0, 1, f_safely_shrinkable, ""),
    S!("window--display-buffer", 3, 4, f_window_display_buffer, ""),
    S!("window-max-chars-per-line", 0, 2, f_window_max_chars, ""),
    S!("window-preserve-size", 0, 3, f_window_preserve_size, ""),
    S!("window-left-column", 0, 1, f_window_left_column, ""),
    S!("pos-visible-in-window-group-p", 0, 3, f_pos_visible, ""),
    S!("window-line", 0, 1, f_window_line, ""),
    S!("window-normalize-window", 1, 2, f_window_normalize, ""),
    S!("window-normalize-buffer", 1, 1, f_window_norm_buffer, ""),
    S!("window-normalize-frame", 1, 1, f_window_norm_frame, ""),
    S!("delete-windows-on", 0, 2, f_delete_windows_on, ""),
    // `split-window-sensibly' is Lisp (GNU window.el) — see prelude.
    S!("window-child", 1, 1, f_window_child, ""),
    S!("window-child-count", 1, 1, f_window_child_count, ""),
    S!("window-combined-p", 0, 2, f_window_combined_p, ""),
    // `window-leftmost-p'/`-rightmost-p'/`-topmost-p'/`-bottommost-p'
    // do not exist in GNU.
    S!("window-at-side-p", 0, 2, f_window_at_side_p, ""),
    S!(
        "window-in-direction",
        1,
        6,
        f_window_in_direction,
        "Return window in DIRECTION as seen from WINDOW."
    ),
    S!("window-main-window", 0, 1, f_window_main_window, ""),
    // `get-mru-window' lives in editor::winxtra with the real
    // use-time scan.
    S!("get-window-with-predicate", 1, 4, f_get_window_pred, ""),
    // ---------- keymap ops ----------
    S!("suppress-keymap", 1, 2, f_suppress_keymap, ""),
    S!("make-composed-keymap", 1, 2, f_make_composed_keymap, ""),
    S!("current-active-maps", 0, 2, f_current_active_maps, ""),
    S!("keymap-canonicalize", 1, 1, f_keymap_canonicalize, ""),
    // `set-transient-map' is a Lisp defun (GNU subr.el → our
    // lisp/subr-x.el) so KEEP-PRED/ON-EXIT/MESSAGE/TIMEOUT all work.
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
    S!(
        "remacs--char-width-table",
        0,
        0,
        f_remacs_char_width_table,
        ""
    ),
    S!("set-char-table-range", 3, 3, f_set_char_table_range, ""),
    S!("char-table-parent", 1, 1, f_char_table_parent, ""),
    S!("set-char-table-parent", 2, 2, f_set_char_table_parent, ""),
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
        5,
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
    S!("setenv-internal", 4, 4, f_setenv_internal, ""),
    S!(
        "read--expression",
        1,
        2,
        f_read_expression,
        "Read one form."
    ),
    S!(
        "read-positioning-symbols",
        0,
        1,
        f_read_positioning_symbols,
        ""
    ),
    S!("describe-vector", 1, 2, f_describe_vector, ""),
    S!("locale-info", 1, 1, f_locale_info, "Locale data for ITEM."),
    S!("locale-translate", 1, 1, f_identity, ""),
    S!("mapbacktrace", 1, 2, f_mapbacktrace, ""),
    // `internal-timer-start-idle' is Lisp (prelude timer.el port).
    S!(
        "internal-describe-syntax-value",
        1,
        1,
        f_internal_describe_syntax_value,
        "Insert a description of the internal syntax description SYNTAX at point."
    ),
    S!(
        "internal-copy-lisp-face",
        4,
        4,
        f_internal_copy_lisp_face,
        ""
    ),
    S!(
        "internal-make-lisp-face",
        1,
        2,
        f_internal_make_lisp_face,
        ""
    ),
    S!(
        "frame-or-buffer-changed-p",
        0,
        1,
        f_frame_or_buffer_changed_p,
        ""
    ),
    S!("scroll-bar-scale", 2, 2, f_scroll_bar_scale, ""),
    S!("popup-menu", 1, 4, f_popup_menu, ""),
    S!("set-frame-font", 1, 4, f_set_frame_font, ""),
    S!(
        "set-keyboard-coding-system",
        1,
        2,
        f_set_keyboard_coding_system,
        ""
    ),
    S!(
        "set-terminal-coding-system",
        1,
        2,
        f_set_terminal_coding_system,
        ""
    ),
    S!("set-mouse-absolute-pixel-position", 2, 2, f_nil, ""),
    S!("tooltip-mode", 0, 1, f_tooltip_mode, ""),
    // `keymap-of' does not exist in GNU Emacs 31.
    // ---------- display/font/image stubs (no GUI) ----------
    S!("default-font-width", 0, 0, f_one, "Char cell width."),
    S!("default-font-height", 0, 0, f_one, "Char cell height."),
    S!(
        "window-font-width",
        0,
        2,
        f_window_font_metric,
        "Char cell width."
    ),
    S!(
        "window-font-height",
        0,
        2,
        f_window_font_metric,
        "Char cell height."
    ),
    S!("color-distance", 2, 4, f_color_distance, "RGB distance."),
    S!(
        "frame-geometry",
        0,
        1,
        f_frame_geometry,
        "Frame geometry; nil on a tty."
    ),
    S!("frame-inner-width", 0, 1, f_frame_width_val, ""),
    S!("frame-inner-height", 0, 1, f_frame_height_val, ""),
    S!("frame-outer-width", 0, 1, f_frame_outer_dim, ""),
    S!("frame-outer-height", 0, 1, f_frame_outer_dim, ""),
    S!("glyph-char", 1, 1, f_glyph_char, ""),
    S!("glyph-face", 1, 1, f_glyph_face, ""),
    S!("make-glyph-code", 1, 2, f_make_glyph_code, ""),
    S!("font-at", 1, 3, f_font_at, ""),
    S!("font-get-glyphs", 3, 4, f_font_object_stub, ""),
    S!("font-info", 1, 2, f_font_info, ""),
    S!("font-match-p", 2, 2, f_font_match_p, ""),
    S!("font-family-list", 0, 1, crate::editor::f_nil_frame_arg, ""),
    S!("font-face-attributes", 1, 2, f_font_face_attributes, ""),
    S!("font-spec", many 0, f_font_spec, ""),
    S!("face-name", 1, 1, f_face_name, ""),
    S!("face-font", 1, 3, f_face_font, ""),
    S!("face-documentation", 1, 1, f_face_documentation, ""),
    S!(
        "face-attributes-as-vector",
        1,
        1,
        f_face_attributes_as_vector,
        ""
    ),
    S!("image-flush", 1, 2, f_image_flush, ""),
    S!("image-mask-p", 1, 2, f_image_spec_check, ""),
    S!("image-metadata", 1, 2, f_image_metadata, ""),
    S!("image-size", 1, 3, f_image_spec_check, ""),
    S!("image-transforms-p", 0, 1, f_image_transforms_p, ""),
    S!("image-type", 1, 3, f_image_type, ""),
    S!("image-type-available-p", 1, 2, f_image_type_available_p, ""),
    S!("init-image-library", 1, 1, f_init_image_library, ""),
    S!("put-image", 2, 4, f_put_image, ""),
    S!("remove-images", 2, 3, f_remove_images, ""),
    S!("display-popup-menus-p", 0, 1, f_display_popup_menus_p, ""),
    S!("display-screens", 0, 1, f_display_screens, ""),
    S!("display-selections-p", 0, 1, f_display_pred_false, ""),
    // ---------- X stubs (no X) ----------
    S!("gui-get-selection", 0, 2, f_gui_get_selection, ""),
    S!("gui-set-selection", 2, 2, f_arg1, ""),
    S!(
        "x-create-frame",
        1,
        1,
        f_ns_frame_stub_err,
        "Create an X frame (no X toolkit)."
    ),
    S!("x-family-fonts", 0, 2, f_nil, "List X font families."),
    S!(
        "x-load-color-file",
        1,
        1,
        f_nil,
        "Load an X color name file."
    ),
    S!(
        "x-select-font",
        0,
        2,
        f_select_font_err,
        "Select an X font (no X toolkit)."
    ),
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
    S!("x-get-resource", 2, 4, f_x_get_resource, ""),
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
    S!("x-show-tip", 1, 6, f_x_show_tip, ""),
    // ---------- optional-library availability ----------
    S!("gnutls-available-p", 0, 0, f_gnutls_available_p, ""),
    S!("sqlite-available-p", 0, 0, f_t, ""),
    S!("libxml-available-p", 0, 0, f_t, ""),
    S!("treesit-available-p", 0, 0, f_t, ""),
    S!("imagep", 1, 1, f_imagep, ""),
    S!("long-line-optimizations-p", 0, 0, f_nil, ""),
    // ---------- input/display mode internals ----------
    S!("current-input-mode", 0, 0, f_current_input_mode, ""),
    S!("set-input-mode", 3, 4, f_nil, ""),
    S!("set-input-interrupt-mode", 1, 1, f_nil, ""),
    S!("set-input-meta-mode", 1, 2, f_set_input_meta_mode, ""),
    S!(
        "current-bidi-paragraph-direction",
        0,
        1,
        f_current_bidi_paragraph_direction,
        ""
    ),
    S!(
        "bidi-string-mark-left-to-right",
        1,
        1,
        f_bidi_string_mark_left_to_right,
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
    S!(
        "mouse-position-in-root-frame",
        0,
        0,
        f_mouse_position_root,
        ""
    ),
    S!("window-scroll-bar-width", 0, 1, f_zero_winlive, ""),
    S!("window-scroll-bar-height", 0, 1, f_zero_winlive, ""),
    S!("line-number-display-width", 0, 1, f_zero, ""),
    // `move-to-window-line' is a Lisp-level defun in GNU, not a subr.
    S!(
        "window-configuration-equal-p",
        2,
        2,
        crate::editor::f_window_configuration_equal_p,
        ""
    ),
    S!(
        "window-configuration-frame",
        1,
        1,
        crate::editor::f_window_configuration_frame,
        ""
    ),
    S!("internal-stack-stats", 0, 0, f_nil, ""),
    S!("pdumper-stats", 0, 0, crate::lisp::dump::stats, ""),
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
        f_set_file_selinux_context,
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
        f_garbage_collect_maybe,
        "GC if allocation count warrants it."
    ),
    S!(
        "make-closure",
        many 1,
        f_make_closure,
        "Wrap a byte-code prototype into a closure."
    ),
    S!("do-auto-save", 0, 2, f_nil, "Auto-save all buffers."),
    S!(
        "sqlitep",
        1,
        1,
        super::sqlite::f_sqlitep,
        "t if OBJECT is a SQLite handle."
    ),
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
        f_composition_get_gstring,
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
        f_find_composition_internal,
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
        f_set_safe_terminal_coding,
        "Set safe terminal coding system."
    ),
    S!(
        "window-cursor-info",
        0,
        1,
        f_window_cursor_info,
        "Cursor info for WINDOW."
    ),
    S!(
        "profiler-cpu-log",
        0,
        0,
        crate::lisp::profiler::cpu_log,
        "CPU profiler log."
    ),
    S!(
        "profiler-cpu-stop",
        0,
        0,
        crate::lisp::profiler::stop_cpu,
        "Stop CPU profiler."
    ),
    S!(
        "profiler-memory-log",
        0,
        0,
        crate::lisp::profiler::memory_log,
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
        crate::lisp::profiler::start_memory,
        "Start memory profiler."
    ),
    S!(
        "profiler-memory-stop",
        0,
        0,
        crate::lisp::profiler::stop_memory,
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
        "dump-emacs-portable",
        1,
        2,
        crate::lisp::dump::save,
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
        f_backtrace_debug,
        "Enter debugger for backtrace frame."
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
    S!(
        "profiler-cpu-start",
        1,
        1,
        crate::lisp::profiler::start_cpu,
        ""
    ),
    S!(
        "backtrace-eval",
        2,
        3,
        f_backtrace_eval,
        "Evaluate FORM in backtrace frame."
    ),
    // ---------- thread/process internals ----------
    S!(
        "thread-buffer-disposition",
        1,
        1,
        f_thread_buffer_disposition,
        ""
    ),
    S!(
        "thread-set-buffer-disposition",
        2,
        2,
        f_thread_set_buffer_disposition,
        ""
    ),
    S!("set-network-process-option", 3, 4, f_process_arg_err, ""),
    S!("set-process-thread", 2, 2, f_process_arg_err, ""),
    S!("process-thread", 1, 1, f_process_arg_err, ""),
    // ---------- reader/printer/composition internals ----------
    S!("lread--substitute-object-in-subtree", 3, 3, f_nil, ""),
    S!(
        "print--preprocess",
        1,
        1,
        crate::lisp::print::f_print_preprocess,
        ""
    ),
    S!("clear-composition-cache", 0, 0, f_nil, ""),
    S!("help--describe-vector", 7, 7, f_help_describe_vector, ""),
    S!("re--describe-compiled", 1, 2, f_re_describe_compiled, ""),
    S!("system-move-file-to-trash", 1, 1, f_move_file_to_trash, ""),
    // ---------- display/font internals (no GUI) ----------
    S!("get-display-property", 2, 4, f_get_display_property, ""),
    S!("lookup-image-map", 3, 3, f_lookup_image_map, ""),
    S!("clear-image-cache", 0, 2, f_clear_image_cache, ""),
    S!("image-cache-size", 0, 0, f_zero, ""),
    S!("display--line-is-continued-p", 0, 0, f_nil, ""),
    S!("display--update-for-mouse-movement", 3, 3, f_nil, ""),
    S!(
        "internal-handle-focus-in",
        1,
        1,
        f_internal_handle_focus_in,
        ""
    ),
    S!("internal-face-x-get-resource", 2, 3, f_nil, ""),
    S!(
        "internal-set-alternative-font-family-alist",
        1,
        1,
        f_set_alt_font_family_alist,
        ""
    ),
    S!(
        "internal-set-alternative-font-registry-alist",
        1,
        1,
        f_set_alt_font_registry_alist,
        ""
    ),
    S!(
        "internal-set-font-selection-order",
        1,
        1,
        f_set_font_selection_order,
        ""
    ),
    S!(
        "internal-set-lisp-face-attribute-from-resource",
        3,
        4,
        f_set_lisp_face_attr_resource,
        ""
    ),
    S!("close-font", 1, 2, f_close_font, ""),
    S!("font-has-char-p", 2, 3, f_font_has_char_p, ""),
    S!("font-shape-gstring", 2, 2, f_font_shape_gstring, ""),
    S!("font-variation-glyphs", 2, 2, f_font_object_stub, ""),
    S!("query-fontset", 1, 2, f_query_fontset, ""),
    S!("define-fringe-bitmap", 2, 5, f_define_fringe_bitmap, ""),
    S!("destroy-fringe-bitmap", 1, 1, f_destroy_fringe_bitmap, ""),
    S!("set-fringe-bitmap-face", 1, 2, f_set_fringe_bitmap_face, ""),
];

// ---------- symbols / functions ----------

fn gensym_literal(i: &mut Interp, key: &str, text: &str) -> Value {
    if let Some(value) = i.native_lisp_literals.get(key) { return value.clone(); }
    let string = Value::string_ref(text);
    i.mark_unibyte(&string);
    let value = Value::Str(string);
    i.native_lisp_literals.insert(key.to_owned(), value.clone());
    value
}

fn f_gensym(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // subr.el: update the counter before formatting. GNU's dumped native
    // function reads the old counter once and inlines the 1+ operation.
    let counter = match i.native_lisp_literals.get("gensym/counter") {
        Some(Value::Sym(symbol)) => *symbol,
        _ => {
            let symbol = i.q("gensym-counter");
            i.native_lisp_literals.insert("gensym/counter".into(), Value::Sym(symbol));
            symbol
        }
    };
    let number = super::call_native_builtin(i, "symbol-value", vec![Value::Sym(counter)])?;
    let next = super::call_canonical_builtin(i, "1+", vec![number.clone()])?;
    i.set_symbol(counter, next)?;
    let prefix = arg(&args, 0);
    let prefix = if prefix.is_nil() { gensym_literal(i, "gensym/prefix", "g") } else { prefix };
    let format = gensym_literal(i, "gensym/format", "%s%d");
    let name = super::call_native_builtin(i, "format", vec![format, prefix, number])?;
    super::call_native_builtin(i, "make-symbol", vec![name])
}

/// GNU `lisp_func_arity' walk over a raw interpreted arglist:
/// non-symbol elements and a bare `. tail' (not after `&rest') make
/// the arglist malformed → None → invalid-function.
pub(crate) fn raw_arglist_arity(i: &mut Interp, arglist_v: &Value) -> Option<Arity> {
    let opt_sym = i.q("&optional");
    let rest_sym = i.q("&rest");
    let (mut min, mut max, mut many, mut mode) = (0u16, 0u16, false, 0);
    let mut cur = arglist_v.clone();
    let mut dotted = Value::Nil;
    loop {
        match cur {
            Value::Cons(c) => {
                let (hd, tl) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let Some(id) = i.sym_id(&hd) else {
                    return None;
                };
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
                cur = tl;
            }
            Value::Nil => break,
            other => {
                dotted = other;
                break;
            }
        }
    }
    // `(a . r)' is malformed; `(a &rest . r)' binds r as rest-arg —
    // GNU reads it through `&rest', and the tail must name a symbol.
    if !dotted.is_nil() && (!many || i.sym_id(&dotted).is_none()) {
        return None;
    }
    Some(if many {
        Arity::Many { min }
    } else {
        Arity::Range { min, max }
    })
}

/// A Lambda's `Arity': parsed arg fields for source closures, the
/// packed/list argdesc in `bc_items[0]' for `#[...]' byte-code objects.
fn lambda_arity(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Option<Arity> {
    // GNU `func_arity' consults the raw arglist for interpreted
    // functions — `(b 1)' optional specs or a non-`&rest' dot make
    // it malformed → invalid-function (only callable lambdas carry
    // a parsed arity).
    if let Some(al) = &l.arglist {
        if al.is_nil() || matches!(al, Value::Cons(_)) {
            return raw_arglist_arity(i, al);
        }
    }
    if l.bad_arglist {
        return None;
    }
    Some(lambda_arity_raw(i, l))
}

fn lambda_arity_raw(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Arity {
    if let Some(items) = &l.bc_items {
        match items.borrow().first() {
            Some(Value::Int(n)) => {
                let req = (*n & 0x7f) as u16;
                let rest = (*n >> 7) & 1 != 0;
                let max = ((*n >> 8) & 0x7f) as u16;
                if rest {
                    Arity::Many { min: req }
                } else {
                    Arity::Range { min: req, max }
                }
            }
            Some(list @ (Value::Cons(_) | Value::Nil)) => {
                let opt = i.q("&optional");
                let rst = i.q("&rest");
                let (mut min, mut max, mut rest, mut mode) = (0u16, 0u16, false, 0);
                let mut cur = list.clone();
                while let Value::Cons(c) = cur {
                    let (hd, tl) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match i.sym_id(&hd) {
                        Some(s) if s == opt => mode = 1,
                        Some(s) if s == rst => {
                            rest = true;
                            mode = 2;
                        }
                        Some(_) => {
                            min += (mode == 0) as u16;
                            max += (mode <= 1) as u16;
                        }
                        None => {}
                    }
                    cur = tl;
                }
                if rest {
                    Arity::Many { min }
                } else {
                    Arity::Range { min, max }
                }
            }
            _ => l.arity(),
        }
    } else {
        l.arity()
    }
}
fn f_func_arity(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU signals void-function for unbound symbols (indirect-function).
    if let Some(id) = i.sym_id(&args[0]) {
        if matches!(i.symbol_function(id), Value::Sym(s) if s == sym::UNBOUND) {
            return Err(i.signal_data(sym::VOID_FUNCTION, vec![args[0].clone()]));
        }
    }
    let fun = i.indirect_function_value(&args[0]);
    // GNU eval.c: (macro . X) is unwrapped one level before arity.
    let fun = match &fun {
        Value::Cons(c) if matches!(i.sym_id(&c.borrow().car), Some(s) if s == i.q("macro")) => {
            c.borrow().cdr.clone()
        }
        _ => fun,
    };
    // GNU `func_arity': a bare `(autoload ...)' cons that isn't reached
    // through a symbol's function cell signals (symbolp OBJ).
    let auto_id = i.q("autoload");
    if let Value::Cons(c) = &fun {
        if !matches!(args[0], Value::Sym(_))
            && matches!(i.sym_id(&c.borrow().car), Some(s) if s == auto_id)
        {
            return Err(i.wrong_type_mut("symbolp", &args[0]));
        }
    }
    // GNU: `func-arity' on an autoloaded symbol LOADS the file first
    // (Ffunc_arity calls autoload_do_load).
    let fun = match &fun {
        Value::Cons(c) if matches!(i.sym_id(&c.borrow().car), Some(s) if s == auto_id) => {
            super::evalfn::autoload_do_load(i, fun.clone(), false)?
        }
        _ => fun,
    };
    // A `(lambda ARGLIST ...)' list.
    let mut list_arity = |v: &Value| -> Option<Arity> {
        let cells = v.list_to_vec().ok()?;
        let head = i.sym_id(cells.first()?)?;
        let name = i.symbol_name(head);
        // GNU 31.1: only `(lambda ...)' is a valid list function —
        // old-style `(closure ENV ...)' lists are invalid-function.
        if name != "lambda" {
            return None;
        }
        raw_arglist_arity(i, cells.get(1).unwrap_or(&Value::Nil))
    };

    let arity = match &fun {
        Value::Subr(s) => Some(s.arity),
        Value::Lambda(l) => lambda_arity(i, l),
        v => list_arity(v),
    };
    let arity = match arity {
        Some(a) => a,
        None => {
            return Err(i.signal_data(sym::INVALID_FUNCTION, vec![args[0].clone()]));
        }
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min, Value::Int(max as i128)),
        Arity::Many { min } => (min, Value::Sym(i.q("many"))),
        Arity::Unevalled => {
            // `(2 . unevalled)' for `if' — min from the special-form table.
            let min = match &args[0] {
                Value::Sym(id) => crate::lisp::special::special_form_min_args(*id),
                Value::Subr(s) => crate::lisp::special::special_form_min_args(i.intern(s.name)),
                _ => 0,
            };
            (min, Value::Sym(i.q("unevalled")))
        }
    };
    Ok(Value::cons(Value::Int(min as i128), max))
}

fn f_subr_arity(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU requires the subr object itself (not a symbol naming one).
    let arity = match &args[0] {
        Value::Subr(s) => s.arity,
        // GNU 31.1 nativecomp: Lisp-defined functions report `subrp' t
        // and `subr-arity' returns their arglist-derived range.  remacs
        // marks dumped definitions `compiled' (`f_subrp'), so a compiled
        // non-macro lambda must be accepted here too — and that INCLUDES
        // `.elc'-twin `#[...]' objects, which GNU native-compiles to
        // real subrs (their `subrp' is already t, so `subr-arity' must
        // accept them or it contradicts `subrp').
        // Only RAW `#[...]' literals (`compiled' false — e.g. what
        // `apply-partially' returns or a reader byte-code vector) stay
        // non-subrp and signal `wrong-type-argument' like GNU.
        Value::Lambda(l) if l.compiled && !l.is_macro => lambda_arity_raw(i, l),
        other => return Err(i.wrong_type_mut("subrp", other)),
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min as i128, Value::Int(max as i128)),
        Arity::Many { min } => (min as i128, Value::Sym(i.q("many"))),
        // Unevalled only ever comes from a Subr (lambdas can't be
        // unevalled), so the special-form table lookup stays on its name.
        Arity::Unevalled => (
            crate::lisp::special::special_form_min_args(match &args[0] {
                Value::Subr(s) => i.intern(s.name),
                _ => unreachable!("unevalled arity on non-subr"),
            }) as i128,
            Value::Sym(i.q("unevalled")),
        ),
    };
    Ok(Value::cons(Value::Int(min), max))
}

fn f_closurep(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `closurep' is t for interpreted closures AND byte-code
    // objects alike (both are `function' cells that close over state);
    // only subrs/primitives and non-functions answer nil.
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

fn f_interpreted_function_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&args[0], Value::Lambda(l) if !l.is_bytecode()),
    ))
}

pub(crate) fn f_make_interpreted_closure(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-interpreted-closure ARGS BODY ENV &optional DOCSTRING IFORM)
    // — GNU eval.c: slots [args body env nil docstring iform], length
    // 3/5/6 by trailing non-nil slots.  IFORM is `(interactive X)'.
    // GNU's checks: `consp' on BODY first (a nil body is an error), then
    // `listp' on ARGS; ENV is never checked.
    if !matches!(args[1], Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &args[1]));
    }
    // CHECK_LIST checks the outer shape only.  The binding walk diagnoses
    // a dotted argument tail when the resulting closure is called.
    if !matches!(args[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &args[0]));
    }
    let doc = args.get(3).cloned().filter(|v| !v.is_nil());
    let iform = args.get(4).cloned().filter(|v| !v.is_nil());
    let v = make_interpreted_closure(i, &args[0], &args[1], &args[2], false, None);
    let Value::Lambda(l) = &v else {
        return Ok(v);
    };
    let doc_str = doc.as_ref().and_then(|d| match d {
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    });
    Ok(Value::Lambda(Rc::new(Lambda {
        doc_value: doc,
        env_value: Some(args[2].clone()),
        interactive: iform.or_else(|| l.interactive.clone()),
        is_macro: l.is_macro,
        required: l.required.clone(),
        optional: l.optional.clone(),
        rest: l.rest,
        body: l.body.clone(),
        env: l.env.clone(),
        doc: doc_str.or_else(|| l.doc.clone()),
        name: l.name.clone(),
        bad_arglist: l.bad_arglist,
        arglist: l.arglist.clone(),
        plain: l.plain,
        dumped_doc: l.dumped_doc,
        compiled: l.compiled,
        advice_link: l.advice_link,
        bc_items: l.bc_items.clone(),
        bc_closure: l.bc_closure,
        bc_prog: std::cell::RefCell::new(None),
    }))
    .tracked())
}

/// Build an interpreted-closure Lambda from ARGLIST, BODY (a list of
/// body forms) and ENV (nil → dynamic, `(t)' → dynamic top-level,
/// otherwise an alist lexical frame).  `plain' mirrors `Lambda.plain':
/// the printer shows `nil' for the env of plain lambdas, `(t)' for
/// defun-produced ones — matching Emacs 31's `#[args body env]' repr.
/// `bc_items' carries the raw `#[...]' elements for `arrayp'/`aref'
/// when the object was read as a byte-code literal.
pub(crate) fn make_interpreted_closure(
    i: &mut Interp,
    arglist_v: &Value,
    body_v: &Value,
    env_v: &Value,
    plain: bool,
    bc_items: Option<Vec<Value>>,
) -> Value {
    // Parse ARGS (a list arglist) into required/optional/rest.
    let arglist = arglist_v.list_to_vec().unwrap_or_default();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut mode = 0; // 0 req, 1 opt, 2 rest
    let opt_sym = i.q("&optional");
    let rest_sym = i.q("&rest");
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
    let body = body_v.list_to_vec().unwrap_or_default();
    let env = crate::lisp::eval::lexenv_from_value(env_v);
    // A `#[ARGDESC STR CONSTS DEPTH DOC ISPEC]' literal read while
    // evaluating a `.elc' dump blob stands in for GNU's byte-compiled
    // (or, on nativecomp builds, subr-shaped) function object: mark it
    // `compiled' so `subrp'/`type-of' report the nativecomp surface,
    // and pull the doc/interactive slots into the Lambda like
    // `parse_lambda' does for source definitions.  Plain `#[...]' reads
    // (user code, `make-byte-code') stay `compiled' = nil.
    let elc_compiled = i.loading_dumped
        && bc_items
            .as_ref()
            .map(|items| crate::lisp::bytecode::is_byte_code(items))
            .unwrap_or(false);
    let mut bc_doc = None;
    let mut bc_doc_str = None;
    let mut bc_interactive = None;
    if elc_compiled {
        let items = bc_items.as_ref().unwrap();
        if let Some(dv) = items.get(4).filter(|v| !v.is_nil()) {
            if let Value::Str(s) = dv {
                bc_doc_str = Some(s.borrow().clone());
            }
            bc_doc = Some(dv.clone());
        }
        if let Some(spec) = items.get(5).filter(|v| !v.is_nil()) {
            bc_interactive = Some(Value::list(vec![
                Value::Sym(i.q("interactive")),
                spec.clone(),
            ]));
        }
    }
    Value::Lambda(Rc::new(Lambda {
        is_macro: false,
        required,
        optional,
        rest,
        body,
        env,
        doc: bc_doc_str,
        interactive: bc_interactive,
        name: None,
        bad_arglist: false,
        arglist: Some(arglist_v.clone()),
        plain,
        dumped_doc: false,
        compiled: elc_compiled,
        advice_link: None,
        bc_items: bc_items.map(|v| crate::lisp::Value::vector_ref(v)),
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: bc_doc,
        env_value: None,
    }))
    .tracked()
}

/// GNU's closure element view for `aref'/`length'/`copy-sequence':
/// `[ARGS BODY ENV nil DOCSTRING IFORM]' truncated to 3, 5, or 6
/// slots — 6 when interactive, 5 when a docstring/type slot is
/// present, else 3 (eval.c `Fmake_interpreted_closure').  Byte-code
/// `#[...]' literals keep their literal elements.
pub(crate) fn lambda_slot_values(l: &Lambda) -> Vec<Value> {
    if let Some(items) = &l.bc_items {
        return items.borrow().clone();
    }
    // GNU's iform slot holds the spec *inside* `(interactive ...)':
    // its single form, or a vector when several forms follow.
    let iform_val = l.interactive.as_ref().and_then(|f| {
        let inner = match f {
            Value::Cons(c) => c.borrow().cdr.clone(),
            other => other.clone(),
        };
        let forms = inner.list_to_vec().unwrap_or_default();
        match forms.len() {
            1 => Some(forms.into_iter().next().unwrap()),
            0 => None,
            _ => Some(Value::vector(forms)),
        }
    });
    // GNU sizes by `!NILP (iform)' — `(interactive)' alone still
    // produces a 6-slot closure with a nil slot 5.
    let has_if = l.interactive.is_some();
    let env = l.env_value.clone().unwrap_or_else(|| {
        if l.env.is_none() {
            Value::Nil
        } else {
            crate::lisp::eval::lexenv_as_value(&l.env)
        }
    });
    let mut slots = vec![
        l.arglist.clone().unwrap_or(Value::Nil),
        Value::list(l.body.clone()),
        env,
        Value::Nil,
        l.doc_value.clone().unwrap_or(Value::Nil),
        iform_val.unwrap_or(Value::Nil),
    ];
    slots.truncate(if has_if {
        6
    } else if l.doc_value.is_some() {
        5
    } else {
        3
    });
    slots
}

fn f_getenv_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = want_string(i, &args[0])?;
    // GNU consults `process-environment' only (it is initialized from
    // the OS environment at startup); shared with filename expansion.
    match crate::editor::lisp_getenv(i, &name) {
        Some(v) => Ok(Value::string(v)),
        None => Ok(Value::Nil),
    }
}

fn f_command_modes(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let _ = args;
    Ok(Value::Nil)
}

fn f_abort_minibuffers(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fabort_minibuffers: not in a minibuffer → error; otherwise
    // `minibuffer-quit-recursive-edit' throws `exit' (a function that
    // signals `minibuffer-quit' after the read unwinds).
    if i.minibuf_level <= 0 {
        return Err(i.error("Not in a minibuffer"));
    }
    let qre = Value::Sym(i.q("minibuffer-quit-recursive-edit"));
    i.apply(&qre, vec![])
}

fn f_keymap_canonicalize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el's algorithm: walk MAP (inheritance flattened), gather
    // (KEY . DEF) pairs — char-range keys ((LO . HI)) go into a fresh
    // char-table via define-key; everything else lands in the alist
    // (duplicate keys merged by `keymap--merge-bindings').
    if keymap_of(i, &a[0])?.is_none() {
        return Ok(Value::list(vec![Value::Sym(i.q("keymap"))]));
    }
    let src = a[0].clone();
    let prompt = {
        let kp = Value::Sym(i.q("keymap-prompt"));
        i.apply(&kp, vec![src.clone()]).unwrap_or(Value::Nil)
    };
    let mut bindings: Vec<Value> = Vec::new();
    let mut ranges: Vec<(i128, i128, Value)> = Vec::new();
    if let Some(pairs) = keymap_of(i, &src)? {
        for (k, def) in pairs {
            if let Value::Cons(r) = &k {
                let rb = r.borrow();
                if let (Value::Int(lo), Value::Int(hi)) = (&rb.car, &rb.cdr) {
                    let (lo, hi) = (*lo, *hi);
                    drop(rb);
                    if hi == lo + 1 {
                        // Two-char range → two single bindings (GNU).
                        bindings.push(Value::cons(Value::Int(lo), def.clone()));
                        bindings.push(Value::cons(Value::Int(hi), def));
                    } else {
                        ranges.push((lo, hi, def));
                    }
                    continue;
                }
            }
            bindings.push(Value::cons(k, def));
        }
    }
    let mk = Value::Sym(i.intern(if ranges.is_empty() {
        "make-sparse-keymap"
    } else {
        "make-keymap"
    }));
    let map = i.apply(
        &mk,
        if prompt.is_nil() {
            vec![]
        } else {
            vec![prompt]
        },
    )?;
    let dk = Value::Sym(i.q("define-key"));
    for (lo, hi, def) in ranges {
        let rk = Value::cons(Value::Int(lo), Value::Int(hi));
        let kv = Value::vector(vec![rk]);
        i.apply(&dk, vec![map.clone(), kv, def])?;
    }
    // Merge duplicate keys; GNU pushes each binding then nconcs, so the
    // result's alist is reversed (later bindings come first).
    let merge = Value::Sym(i.q("keymap--merge-bindings"));
    let mut out: Vec<Value> = Vec::new();
    for b in bindings {
        let key = match &b {
            Value::Cons(c) => c.borrow().car.clone(),
            _ => continue,
        };
        let dup = out
            .iter()
            .position(|e| matches!(e, Value::Cons(c) if super::eq_values(&c.borrow().car, &key)));
        match dup {
            None => out.push(b),
            Some(pos) => {
                let olddef = match &out[pos] {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                let newdef = match &b {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                let merged = i.apply(&merge, vec![newdef, olddef])?;
                out[pos] = Value::cons(key, merged);
            }
        }
    }
    // nconc: splice the alist onto the map tail, reversed like GNU's
    // (push (cons k v) bindings) ordering.
    let mut last = map.clone();
    loop {
        let next = match &last {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => break,
        };
        if matches!(next, Value::Cons(_)) {
            last = next;
        } else {
            break;
        }
    }
    if let Value::Cons(c) = &last {
        let tail: Vec<Value> = out.into_iter().rev().collect();
        if !tail.is_empty() {
            c.borrow_mut().cdr = Value::list(tail);
        }
    }
    Ok(map)
}

fn keymap_of(i: &mut Interp, v: &Value) -> Result<Option<Vec<(Value, Value)>>, Flow> {
    // GNU's C `map_keymap' traversal order: char-table contents
    // (compressed ranges) at the table's spine position, then alist
    // pairs; embedded keymaps expand in place and the parent tail's
    // elements follow.  Symbols resolve via the function cell.
    let v = crate::editor::keymap_def(i, v.clone())?;
    if !crate::editor::is_keymap(i, &v) {
        return Ok(None);
    }
    Ok(Some(crate::editor::keymap_all_bindings(i, &v)))
}

fn f_accessible_keymaps(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Faccessible_keymaps: breadth-first walk of nested keymaps.
    // Entries are (PREFIX-VECTOR . MAP); the map itself is under PREFIX
    // ([] when nil). Maps reached via the meta-prefix (ESC=27) binding
    // are listed under meta-bit keys: [27] + key K => [K | CHAR_META].
    let prefix_elems: Vec<Value> = match arg(&args, 1) {
        Value::Nil => Vec::new(),
        Value::Vec(v) => v.borrow().clone(),
        Value::Str(s) => s.borrow().chars().map(|c| Value::Int(c as i128)).collect(),
        Value::Cons(_) => match arg(&args, 1).list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(i.wrong_type_mut("sequencep", &args[1])),
        },
        // GNU Faccessible_keymaps validates PREFIX with CHECK_SEQUENCE:
        // a non-nil non-sequence signals wrong-type-argument.
        _ => return Err(i.wrong_type_mut("sequencep", &args[1])),
    };
    let prefixlen = prefix_elems.len();
    // GNU resolves the map arg through get_keymap (symbols OK).
    let map0 = crate::editor::keymap_def(i, args[0].clone())?;
    if !crate::editor::is_keymap(i, &map0) {
        return Err(i.wrong_type_mut("keymapp", &args[0]));
    }
    // (prefix-elems, map) queue, processed breadth-first. With a
    // PREFIX, GNU starts at the submap bound by that prefix.
    let mut maps: Vec<(Vec<Value>, Value)> = if prefixlen > 0 {
        let mut keys = Vec::new();
        for value in &prefix_elems {
            if let Some(code) = crate::editor::key_seq_elt(i, value)? { keys.push(code); }
        }
        let mut km = map0.clone();
        let mut ok = true;
        for &k in &keys {
            let raw = crate::editor::lookup_in_keymap(i, &km, k, false)?;
            let d = crate::editor::keymap_def(i, raw)?;
            if crate::editor::is_keymap(i, &d) {
                km = d;
            } else {
                ok = false;
                break;
            }
        }
        if !ok {
            return Ok(Value::Nil);
        }
        vec![(prefix_elems.clone(), km)]
    } else {
        vec![(prefix_elems, map0)]
    };

    /// get_keyelt + get_keymap(cmd, 0, 0): scan the binding's cons
    /// spine for a keymap — the cons itself, a `keymap`-tagged
    /// element, a menu-item's real def, or a symbol tail resolving
    /// via its function cell. Autoload forms are NOT loaded here
    /// (GNU "can't run lisp code" in this traversal).
    fn find_keymap(i: &mut Interp, def: Value) -> Option<Value> {
        let mut d = def;
        loop {
            match d.clone() {
                Value::Cons(c) => {
                    if crate::editor::is_keymap(i, &d) {
                        return Some(d);
                    }
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if crate::editor::is_keymap(i, &car) {
                        return Some(car);
                    }
                    // (menu-item NAME DEF . PROPS): real def is DEF.
                    if i.sym_is(&car, i.q_soft("menu-item").unwrap_or(u32::MAX)) {
                        if let Some(dd) = d.list_to_vec().ok().and_then(|v| v.get(2).cloned()) {
                            d = dd;
                            continue;
                        }
                    }
                    d = cdr;
                }
                Value::Sym(s) => {
                    let f = i.symbol_function(s);
                    if crate::editor::is_keymap(i, &f) {
                        return Some(f);
                    }
                    // Autoload keymap: GNU lists the SYMBOL as the map
                    // (it is never scanned further — not a cons).
                    if crate::editor::is_autoload_keymap(i, &f) {
                        return Some(d);
                    }
                    return None;
                }
                _ => return None,
            }
        }
    }

    /// GNU cycle check: skip CMD if it already appears in MAPS under
    /// a prefix that is a prefix of THISSEQ.
    fn seen_as_prefix(maps: &[(Vec<Value>, Value)], cmd: &Value, thisseq: &[Value]) -> bool {
        for (pfx, m) in maps {
            if !super::eq_values(m, cmd) {
                continue;
            }
            if pfx.len() <= thisseq.len()
                && pfx
                    .iter()
                    .zip(thisseq.iter())
                    .all(|(a, b)| super::eq_values(a, b))
            {
                return true;
            }
        }
        false
    }

    let mut qi = 0usize;
    while qi < maps.len() {
        let (thisseq, thismap) = maps[qi].clone();
        // is_metized: last elem is the meta-prefix char (27) and not
        // part of a user-supplied PREFIX.
        let is_metized = thisseq
            .last()
            .map(|v| matches!(v, Value::Int(27)))
            .unwrap_or(false)
            && thisseq.len() - 1 >= prefixlen;
        // Only cons maps are scanned (autoload-keymap symbols are
        // leaf entries, as in GNU's `CONSP (thismap)` guard).
        if !matches!(thismap, Value::Cons(_)) {
            qi += 1;
            continue;
        }
        if let Some(pairs) = keymap_of(i, &thismap)? {
            for (k, def) in pairs {
                if let Some(m) = find_keymap(i, def) {
                    if seen_as_prefix(&maps, &m, &thisseq) {
                        continue;
                    }
                    let pfx = if is_metized {
                        // Replace trailing 27 with K | CHAR_META.
                        if let Value::Int(kc) = k {
                            let mut s = thisseq.clone();
                            let n = s.len() - 1;
                            s[n] = Value::Int(kc | crate::editor::META_BIT);
                            s
                        } else {
                            let mut s = thisseq.clone();
                            s.push(k.clone());
                            s
                        }
                    } else {
                        let mut s = thisseq.clone();
                        s.push(k.clone());
                        s
                    };
                    // Metized entries go right after the current node;
                    // plain entries append at the queue tail (GNU).
                    let pos = if is_metized { qi + 1 } else { maps.len() };
                    maps.insert(pos, (pfx, m));
                }
            }
        }
        qi += 1;
    }
    let out: Vec<Value> = maps
        .into_iter()
        .map(|(pfx, m)| Value::cons(Value::vector(pfx), m))
        .collect();
    Ok(Value::list(out))
}

fn map_keymap_impl(i: &mut Interp, args: &[Value], own_only: bool) -> EvalResult {
    let v = crate::editor::keymap_def(i, args[1].clone())?;
    if !crate::editor::is_keymap(i, &v) {
        return Err(i.wrong_type_mut("keymapp", &args[1]));
    }
    // GNU `map-keymap-internal' stops at the parent tail and at
    // embedded keymaps; `map-keymap' traverses them.
    let pairs = if own_only {
        crate::editor::keymap_own_bindings(i, &v)
    } else {
        crate::editor::keymap_all_bindings(i, &v)
    };
    for (k, def) in pairs {
        if matches!(&k, Value::Sym(s) if i.symbol_name(*s) == "keymap") {
            continue;
        }
        let fnv = args[0].clone();
        i.apply(&fnv, vec![k, def])?;
    }
    Ok(Value::Nil)
}

fn f_map_keymap(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    map_keymap_impl(i, &args, false)
}

fn f_map_keymap_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    map_keymap_impl(i, &args, true)
}

fn f_keymap_get_keyelt(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fkeymap__get_keyelt = get_keyelt(object, autoload): unwrap
    // `menu-item' forms and (STRING . DEFN) menu-name prefixes.
    Ok(crate::editor::keyelt_value(i, &args[0]))
}

fn f_describe_bindings(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdescribe_buffer_bindings
    // (keymap.c): drive `help--describe-map-tree' over translation maps,
    // overriding/minor/major/local/global maps, then the input-decode maps.
    // Rendering lives in the Lisp helper; the subr only walks the sources.
    let buffer = crate::buffer::primitives::buf_of(i, &a[0])?;
    let prefix = a.get(1).cloned().unwrap_or(Value::Nil);
    let nomenu = if a.get(2).cloned().unwrap_or(Value::Nil).is_nil() {
        Value::t()
    } else {
        Value::Nil
    };
    let qt = Value::t();
    let qnil = Value::Nil;
    let bufv = Value::Buffer(buffer.clone());
    macro_rules! call_tree {
        ($map:expr, $partial:expr, $shadow:expr, $title:expr, $transl:expr, $always:expr, $mention:expr) => {{
            let fun = Value::Sym(i.q("help--describe-map-tree"));
            i.apply(
                &fun,
                vec![
                    $map,
                    $partial,
                    $shadow,
                    prefix.clone(),
                    Value::string($title),
                    nomenu.clone(),
                    $transl,
                    $always,
                    $mention,
                    bufv.clone(),
                ],
            )?;
        }};
    }
    // Key translations map.
    let ktm = i.symbol_value(i.q_soft("key-translation-map").unwrap_or(u32::MAX));
    if crate::editor::is_keymap(i, &ktm) {
        call_tree!(
            ktm,
            qnil.clone(),
            qnil.clone(),
            "Key translations",
            qt.clone(),
            qt.clone(),
            qnil.clone()
        );
    }
    // Overriding maps.
    let mut shadow = Value::Nil;
    let otlm = i.symbol_value(
        i.q_soft("overriding-terminal-local-map")
            .unwrap_or(u32::MAX),
    );
    let olm = i.symbol_value(i.q_soft("overriding-local-map").unwrap_or(u32::MAX));
    for start1 in [otlm, olm] {
        if crate::editor::is_keymap(i, &start1) {
            call_tree!(
                start1.clone(),
                qt.clone(),
                shadow.clone(),
                "\x0c\nOverriding Bindings",
                qnil.clone(),
                qnil.clone(),
                qnil.clone()
            );
            shadow = Value::cons(start1, shadow);
        }
    }
    if shadow.is_nil() {
        // Minor-mode maps (evaluated with BUFFER current so minor-mode
        // predicates resolve there), then the buffer's local map.
        let bid = buffer.borrow().id;
        let save = i.current_buffer;
        i.set_current_buffer(bid);
        let minors = crate::editor::current_minor_maps(i);
        i.set_current_buffer(save);
        if let Ok(minors) = minors {
            for (mode, map) in minors {
                if !crate::editor::is_keymap(i, &map) {
                    continue;
                }
                let name = match &mode {
                    Value::Sym(s) => i.obarray.name(*s).to_string(),
                    _ => continue,
                };
                call_tree!(
                    map.clone(),
                    qt.clone(),
                    shadow.clone(),
                    format!("\x0c\n‘{}’ Minor Mode Bindings", name),
                    qnil.clone(),
                    qnil.clone(),
                    qnil.clone()
                );
                shadow = Value::cons(map, shadow);
            }
        }
        let lk = {
            let bb = buffer.borrow();
            bb.locals
                .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
                .cloned()
                .unwrap_or(Value::Nil)
        };
        if crate::editor::is_keymap(i, &lk) {
            let major = {
                let bb = buffer.borrow();
                bb.locals
                    .get(&i.q_soft("major-mode").unwrap_or(u32::MAX))
                    .cloned()
                    .unwrap_or(Value::Nil)
            };
            let name = match &major {
                Value::Sym(s) => i.obarray.name(*s).to_string(),
                _ => "fundamental-mode".to_string(),
            };
            call_tree!(
                lk.clone(),
                qt.clone(),
                shadow.clone(),
                format!("\x0c\n‘{}’ Major Mode Bindings", name),
                qnil.clone(),
                qnil.clone(),
                qnil.clone()
            );
            shadow = Value::cons(lk, shadow);
        }
    }
    // Global map.
    let gm = i.symbol_value(i.q_soft("global-map").unwrap_or(u32::MAX));
    if crate::editor::is_keymap(i, &gm) {
        call_tree!(
            gm,
            qt.clone(),
            shadow.clone(),
            "\x0c\nGlobal Bindings",
            qnil.clone(),
            qt.clone(),
            qnil.clone()
        );
    }
    // Function key map + input decode map translations.
    for (var, title) in [
        (
            "local-function-key-map",
            "\x0c\nFunction key map translations",
        ),
        ("input-decode-map", "\x0c\nInput decoding map translations"),
    ] {
        let m = i.symbol_value(i.intern_soft(var).unwrap_or(u32::MAX));
        if crate::editor::is_keymap(i, &m) {
            call_tree!(
                m,
                qnil.clone(),
                qnil.clone(),
                title,
                qt.clone(),
                qnil.clone(),
                qnil.clone()
            );
        }
    }
    Ok(Value::Nil)
}

fn f_set_this_command_keys(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_documentation_stringp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU VALID_DOC_STRING_P (doc.h): a string, a fixnum (position in
    // DOC file), or a lazy (FILENAME . POS) cons from an .elc reader.
    let ok = match &args[0] {
        Value::Str(_) | Value::Int(_) => true,
        Value::Cons(c) => {
            let b = c.borrow();
            matches!(b.car, Value::Str(_)) && matches!(b.cdr, Value::Int(_))
        }
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

fn f_error_message_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // Mirrors print_error_message in print.c: OBJ must be a list
    // (signal data); nil yields nil, other non-cons signals
    // wrong-type-argument listp.
    if !args[0].is_nil() && !matches!(args[0], Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &args[0]));
    }
    // nil falls through to error_message → "peculiar error" like GNU.
    // GNU print.c Ferror_message_string: the CAR must be a symbol
    // (error name); anything else signals wrong-type-argument symbolp.
    if let Value::Cons(c) = &args[0] {
        let car = c.borrow().car.clone();
        if !matches!(car, Value::Sym(_)) && !car.is_nil() {
            return Err(i.wrong_type_mut("symbolp", &car));
        }
    }
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
    // GNU print.c's FOR_EACH_TAIL prints every cons in the data, then
    // ignores a dotted atom tail.  Keep the existing cycle guard: only
    // use each_car after list_to_vec has proved the list is acyclic.
    fn data_items(data: &Value) -> Vec<Value> {
        match data.list_to_vec() {
            Ok(items) => items,
            Err(crate::lisp::value::ListError::Dotted(_)) => {
                let mut items = Vec::new();
                data.each_car(|item| items.push(item.clone()));
                items
            }
            Err(crate::lisp::value::ListError::Circular(_)) => Vec::new(),
        }
    }
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
        // For `error`, the first data item is the message — GNU takes
        // XCAR(data) even when DATA is a dotted pair, and the tail is
        // XCDR walked as a list (non-list tail contributes nothing).
        errmsg = match &data {
            Value::Cons(c) => c.borrow().car.clone(),
            _ => Value::Nil,
        };
        file_error = false;
        tail_list = match &data {
            Value::Cons(c) => data_items(&c.borrow().cdr),
            _ => Vec::new(),
        };
    } else {
        let mut msg = Value::Nil;
        let mut ferror = false;
        if let Some(id) = errname_id {
            let plist = i.obarray.symbol(id).plist.clone();
            let em_id = i.q("error-message");
            msg = crate::lisp::eval::plist_get(&plist, em_id);
            let ec_id = i.q("error-conditions");
            let conds = crate::lisp::eval::plist_get(&plist, ec_id);
            let fe_id = i.q("file-error");
            ferror = conds
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .any(|c| i.sym_id(c) == Some(fe_id));
        }
        errmsg = msg;
        file_error = ferror;
        tail_list = data_items(&data);
    }
    let (errmsg, tail, from_prop): (Value, &[Value], bool) = if file_error && !tail_list.is_empty()
    {
        // file-error: first data item is the message string.
        (tail_list[0].clone(), &tail_list[1..], false)
    } else {
        (errmsg, &tail_list[..], errname_id != Some(sym::ERROR))
    };
    // GNU print_error_message runs only the symbol's `error-message'
    // property through `substitute-command-keys' (quote translation);
    // an `error' message or file-error data string prints verbatim —
    // it was already translated when signaled (format-message/doprnt).
    let msg_text = match &errmsg {
        Value::Str(s) if from_prop => Some(super::evalfn::translate_message_quotes(i, &s.borrow())),
        Value::Str(s) => Some(s.borrow().clone()),
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

fn f_external_debugging_output(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fexternal_debugging_output: CHECK_FIXNUM first —
    // anything else signals `fixnump' before the write.
    let c = match args[0].int() {
        Some(c) => c,
        None => return Err(i.wrong_type_mut("fixnump", &args[0])),
    };
    // GNU Fexternal_debugging_output → printchar_to_stream: the fixnum
    // is truncated to `unsigned int', modifier bits (CHAR_MODIFIER_MASK)
    // are stripped, and the residue must satisfy CHAR_VALID_P
    // (≤ MAX_CHAR = 0x3FFFFF) — else "Invalid character: %x".
    let ch32 = c as u32;
    let stripped = if ch32 & 0x0FC0_0000 != 0 {
        ch32 & !0x0FC0_0000
    } else {
        ch32
    };
    if stripped > 0x3FFFFF {
        return Err(i.error(&format!("Invalid character: {:x}", stripped)));
    }
    eprint!("{}", char::from_u32(stripped).unwrap_or('?'));
    Ok(args[0].clone())
}

fn f_open_dribble_file(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_open_termscript(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_send_string_to_terminal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    crate::stdio::write(s.as_bytes());
    crate::stdio::flush();
    Ok(Value::Nil)
}

fn f_flush_stdout(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    crate::stdio::flush();
    Ok(Value::Nil)
}

fn f_display_screens(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: one screen (the initial terminal).
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Int(1))
}

fn f_command_line(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU recurses into the top-level loop; in batch that ends in
    // excessive-lisp-nesting. Signal a plain error instead.
    Err(i.error("command-line is for interactive use"))
}

/// GNU `describe_vector' (keymap.c): iterate VECTOR (a vector or
/// char-table), compress runs sharing a definition, and insert
/// "KEY[.. KEY]" + DESCRIBER output per run.  When BASIC is nil the
/// describer is wrapped like GNU's `describe_vector_princ' (indent-to
/// 16 + terpri).  Char-table runs never straddle the 0x3FFF80
/// boundary and group by `eq'-equal raw/defalt values like GNU's
/// char_table_ref_and_range.
fn describe_vector(
    i: &mut Interp,
    vector: &Value,
    prefix: &Value,
    describer: &Value,
    partial: bool,
    shadow: &Value,
    entire_map: &Value,
    keymap_p: bool,
    // nil ⇒ wrap DESCRIBER like GNU's `describe_vector_princ'
    // (indent-to 16 + terpri); t ⇒ call it directly (basic).
    basic: bool,
    mention_shadow: bool,
) -> Result<(), Flow> {
    let is_ct = is_char_table(i, vector);
    // ELT_PREFIX/"C-x " header for non-keymap vectors; PREFIX then
    // resets to nil like GNU.
    let mut elt_prefix = Value::Nil;
    let mut prefix = prefix.clone();
    if !keymap_p {
        if !prefix.is_nil() {
            let plen = match &prefix {
                Value::Str(s) => s.borrow().chars().count(),
                Value::Vec(v) => v.borrow().len(),
                Value::Cons(_) => prefix.list_to_vec().map(|v| v.len()).unwrap_or(0),
                _ => 0,
            };
            if plen > 0 {
                let kd = Value::Sym(i.q("key-description"));
                let d = i.apply(&kd, vec![prefix.clone(), Value::Nil])?;
                elt_prefix = Value::string(format!("{} ", i.princ_to_string(&d)));
            }
        }
        prefix = Value::Nil;
    }
    let suppress = if partial {
        Value::Sym(i.q("suppress-keymap"))
    } else {
        Value::Nil
    };
    let check_ranges = i
        .intern_soft("describe-bindings-check-shadowing-in-ranges")
        .map(|s| i.symbol_value(s))
        .unwrap_or(Value::Nil);
    let self_insert = i.q("self-insert-command");
    let key_desc = Value::Sym(i.q("key-description"));
    let indent_to = Value::Sym(i.q("indent-to"));
    let shadow_maps = shadow.list_to_vec().unwrap_or_default();
    let kludge = |k: i128| Value::vector(vec![Value::Int(k)]);
    // STOP/TO: GNU never lets a char-table run cross 0x3FFF80.
    let (stop0, to): (i128, i128) = if is_ct {
        (0x3FFF80, CT_MAX_CHAR as i128 + 1)
    } else {
        let n = match vector {
            Value::Vec(v) => v.borrow().len() as i128,
            _ => 0,
        };
        (n, n)
    };
    // Raw/defalt read + run extension by `eq' (GNU compares raw
    // contents values, not resolved definitions).
    // Raw/defalt read helper as a macro so it borrows `i' only for
    // the call (a closure would pin an immutable borrow of `i').
    macro_rules! cellval {
        ($k:expr) => {{
            if is_ct {
                ct_ref_defalt(i, vector, $k as usize)
            } else {
                match vector {
                    Value::Vec(vv) => vv.borrow()[$k as usize].clone(),
                    _ => Value::Nil,
                }
            }
        }};
    }
    let fontify = |i: &mut Interp, k: i128| -> Result<String, Flow> {
        // describe_key_maybe_fontify: key-description + font-lock-face
        // `help-key-binding' when inside a keymap.
        let d = match i.apply(&key_desc, vec![kludge(k), prefix.clone()])? {
            s @ Value::Str(_) => s,
            other => {
                let t = i.princ_to_string(&other);
                Value::string(t)
            }
        };
        if keymap_p {
            let len = match &d {
                Value::Str(s) => s.borrow().chars().count() as i128,
                _ => 0,
            };
            let props = Value::list(vec![
                Value::Sym(i.q("font-lock-face")),
                Value::Sym(i.q("help-key-binding")),
            ]);
            let _ = crate::buffer::primitives::f_add_text_properties(
                i,
                vec![Value::Int(0), Value::Int(len), props, d.clone()].into(),
            );
        }
        Ok(match d {
            Value::Str(s) => s.borrow().clone(),
            _ => i.princ_to_string(&d),
        })
    };
    let mut first = true;
    let mut c: i128 = 0;
    while c < to {
        let starting = c;
        let stop = if c < stop0 { stop0 } else { to };
        // Group [starting, c) sharing the same cell value (GNU's
        // ref_and_range for char-tables; plain eq for vectors).
        let val = cellval!(starting);
        let range_beg = starting;
        if is_ct {
            let mut k = starting + 1;
            while k < stop && crate::lisp::builtins::eq_values(&cellval!(k), &val) {
                k += 1;
            }
            c = k;
            while range_beg > 0 {
                // GNU also extends the range backwards; callers always
                // pass from=i so this only matters at i=0 — skip it:
                // the backward walk can only shrink the range.
                break;
            }
        } else {
            c = starting + 1;
        }
        let _ = range_beg;
        let defn = crate::editor::keyelt_value(i, &val);
        if defn.is_nil() {
            continue;
        }
        if let Value::Sym(_) = defn {
            if partial {
                // Fget(definition, 'suppress-keymap): suppressed
                // commands are not mentioned in partial maps.
                let get = Value::Sym(i.q("get"));
                let tem = i.apply(&get, vec![defn.clone(), suppress.clone()])?;
                if tem.truthy() {
                    continue;
                }
            }
        }
        // Shadowed by an earlier map?
        let mut this_shadowed = false;
        let mut shadowed_by = Value::Nil;
        if !shadow.is_nil() {
            shadowed_by =
                crate::editor::shadow_lookup_ad(i, &shadow_maps, &kludge(starting), true, false)?;
            if !shadowed_by.is_nil() && !crate::lisp::builtins::eq_values(&shadowed_by, &defn) {
                if mention_shadow {
                    this_shadowed = true;
                } else {
                    continue;
                }
            }
        }
        if !entire_map.is_nil() {
            // Flookup_key(entire_map, [k], t): GNU compares the
            // whole-map definition with this vector's definition.
            let evs = crate::editor::seq_events(&kludge(starting));
            let tem = crate::editor::lookup_seq(i, entire_map, &evs, true)?;
            if !crate::lisp::builtins::eq_values(&tem, &defn) {
                continue;
            }
        }
        // Vector-only run extension by equal definitions.
        if !is_ct {
            while c < stop {
                let d2 = crate::editor::keyelt_value(i, &cellval!(c));
                if d2.is_nil() || !crate::lisp::builtins::equal_values(i, &d2, &defn)? {
                    break;
                }
                c += 1;
            }
        }
        // GNU's optional per-char shadow check inside char-table
        // ranges (describe-bindings-check-shadowing-in-ranges).
        let mut range_end = c - 1;
        if check_ranges.truthy() && is_ct && range_end != starting {
            let is_si =
                matches!(&check_ranges, Value::Sym(s) if *s == i.q("ignore-self-insert"));
            let self_def = matches!(&defn, Value::Sym(s) if *s == self_insert);
            if !(is_si && self_def) {
                let mut j = starting + 1;
                while j <= range_end {
                    let tem =
                        crate::editor::shadow_lookup_ad(i, &shadow_maps, &kludge(j), true, false)?;
                    if !crate::lisp::builtins::equal_values(i, &tem, &shadowed_by)? {
                        range_end = j - 1;
                        c = j;
                        break;
                    }
                    j += 1;
                }
            }
        }
        if first {
            i.write_output_to("\n", &Value::Nil)?;
            first = false;
        }
        if !elt_prefix.is_nil() {
            {
                let t = i.princ_to_string_cb(&elt_prefix)?;
                i.write_output_to(&t, &Value::Nil)?;
            }
        }
        let s = fontify(i, starting)?;
        i.write_output_to(&s, &Value::Nil)?;
        if range_end != starting {
            i.write_output_to(" .. ", &Value::Nil)?;
            if !elt_prefix.is_nil() {
                {
                    let t = i.princ_to_string_cb(&elt_prefix)?;
                    i.write_output_to(&t, &Value::Nil)?;
                }
            }
            let s = fontify(i, range_end)?;
            i.write_output_to(&s, &Value::Nil)?;
        }
        if basic {
            i.apply(describer, vec![defn.clone()])?;
        } else {
            // describe_vector_princ: indent-to 16, call, terpri.
            i.apply(&indent_to, vec![Value::Int(16), Value::Int(1)])?;
            i.apply(describer, vec![defn.clone()])?;
            {
                let t = Value::Sym(i.q("terpri"));
                i.apply(&t, vec![])?;
            }
        }
        if this_shadowed {
            // GNU backs point up one char, inserts the note, moves on.
            let pt = i
                .current_buffer_ref()
                .map(|b| b.borrow().point())
                .unwrap_or(1);
            let note = match &shadowed_by {
                Value::Sym(s) => format!("  (currently shadowed by ‘{}’)", i.symbol_name(*s)),
                _ => "  (currently shadowed)".to_string(),
            };
            crate::buffer::primitives::chg_insert(i, pt.saturating_sub(1), &note)?;
            if let Some(b) = i.current_buffer_ref() {
                b.borrow_mut().set_point(pt);
            }
        }
    }
    if is_ct {
        let defalt = i.char_table_defalt(vector);
        if !defalt.is_nil() {
            if !elt_prefix.is_nil() {
                {
                    let t = i.princ_to_string_cb(&elt_prefix)?;
                    i.write_output_to(&t, &Value::Nil)?;
                }
            }
            i.write_output_to("default", &Value::Nil)?;
            if basic {
                i.apply(describer, vec![defalt])?;
            } else {
                i.apply(&indent_to, vec![Value::Int(16), Value::Int(1)])?;
                i.apply(describer, vec![defalt])?;
                {
                    let t = Value::Sym(i.q("terpri"));
                    i.apply(&t, vec![])?;
                }
            }
        }
    }
    Ok(())
}

fn f_describe_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let describer = match a.get(1) {
        Some(v) if !v.is_nil() => v.clone(),
        _ => Value::Sym(i.q("princ")),
    };
    // GNU specbinds `standard-output' to the current buffer, then
    // checks VECTOR-OR-CHAR-TABLE.
    let depth = i.specbind_depth();
    let curbuf = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
    i.specbind(i.standard_output_sym, curbuf)?;
    let result = if !matches!(&a[0], Value::Vec(_)) && !is_char_table(i, &a[0]) {
        Err(i.wrong_type_mut("vector-or-char-table-p", &a[0]))
    } else {
        describe_vector(
            i,
            &a[0],
            &Value::Nil,
            &describer,
            false,
            &Value::Nil,
            &Value::Nil,
            false,
            false,
            false,
        )
    };
    let _ = i.unbind_to(depth);
    result.map(|_| Value::Nil)
}

/// GNU `Finternal_describe_syntax_value' (syntax.c): insert the
/// human-readable description of a syntax-table entry at point —
/// the code letter, matching char, flag letters, then a phrase.
fn f_internal_describe_syntax_value(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const SPEC: [char; 16] = [
        ' ', '.', 'w', '_', '(', ')', '\'', '"', '$', '\\', '/', '<', '>', '@', '!', '|',
    ];
    const NAMES: [&str; 16] = [
        "whitespace",
        "punctuation",
        "word",
        "symbol",
        "open",
        "close",
        "prefix",
        "string",
        "math",
        "escape",
        "charquote",
        "comment",
        "endcomment",
        "inherit",
        "comment fence",
        "string fence",
    ];
    let value = a[0].clone();
    let mut out = String::new();
    if value.is_nil() {
        out.push_str("default");
    } else if is_char_table(i, &value) {
        out.push_str("deeper char-table ...");
    } else if let Value::Cons(cell) = &value {
        let (first, match_lisp) = {
            let b = cell.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        let match_char = match &match_lisp {
            Value::Nil => None,
            Value::Int(m) if *m >= 0 && *m <= CT_MAX_CHAR as i128 => Some(*m as u32),
            _ => None,
        };
        let valid = matches!(first, Value::Int(_)) && (match_char.is_some() || match_lisp.is_nil());
        if !valid {
            out.push_str("invalid");
        } else {
            let Value::Int(syntax_code) = first else {
                unreachable!()
            };
            let code = (syntax_code & 0o377) as usize;
            if code >= 16 {
                out.push_str("invalid");
            } else {
                let start1 = syntax_code & (1 << 16) != 0;
                let start2 = syntax_code & (1 << 17) != 0;
                let end1 = syntax_code & (1 << 18) != 0;
                let end2 = syntax_code & (1 << 19) != 0;
                let prefix = syntax_code & (1 << 20) != 0;
                let comstyleb = syntax_code & (1 << 21) != 0;
                let comnested = syntax_code & (1 << 22) != 0;
                let comstylec = syntax_code & (1 << 23) != 0;
                out.push(SPEC[code]);
                match match_char {
                    None => out.push(' '),
                    Some(m) => out.push(char::from_u32(m).unwrap_or('\u{FFFD}')),
                }
                if start1 {
                    out.push('1');
                }
                if start2 {
                    out.push('2');
                }
                if end1 {
                    out.push('3');
                }
                if end2 {
                    out.push('4');
                }
                if prefix {
                    out.push('p');
                }
                if comstyleb {
                    out.push('b');
                }
                if comstylec {
                    out.push('c');
                }
                if comnested {
                    out.push('n');
                }
                out.push_str("\twhich means: ");
                out.push_str(NAMES[code]);
                if let Some(m) = match_char {
                    out.push_str(", matches ");
                    out.push(char::from_u32(m).unwrap_or('\u{FFFD}'));
                }
                if start1 {
                    out.push_str(",\n\t  is the first character of a comment-start sequence");
                }
                if start2 {
                    out.push_str(",\n\t  is the second character of a comment-start sequence");
                }
                if end1 {
                    out.push_str(",\n\t  is the first character of a comment-end sequence");
                }
                if end2 {
                    out.push_str(",\n\t  is the second character of a comment-end sequence");
                }
                if comstyleb {
                    out.push_str(" (comment style b)");
                }
                if comstylec {
                    out.push_str(" (comment style c)");
                }
                if comnested {
                    out.push_str(" (nestable)");
                }
                if prefix {
                    // GNU inserts `substitute-command-keys' of the
                    // annotation (quote substitution turns `...' into
                    // '...' or grave/curly quoting).
                    let doc =
                        Value::string(",\n\t  is a prefix character for `backward-prefix-chars'");
                    let sck = Value::Sym(i.q("substitute-command-keys"));
                    let sub = i.apply(&sck, vec![doc])?;
                    out.push_str(&i.princ_to_string(&sub));
                }
            }
        }
    } else {
        out.push_str("invalid");
    }
    // GNU's insert_string writes to the current buffer, not
    // `standard-output': the row must not pollute stdout.
    crate::buffer::primitives::chg_insert_pt(i, &out, false)?;
    Ok(value)
}

/// GNU's optional FRAME argument check for face/window readers:
/// nil ok, live frame ok, else `wrong-type-argument frame-live-p'
/// (decode_live_frame).
fn want_opt_frame(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    match v {
        Value::Nil => Ok(()),
        Value::Frame(f) if !f.borrow().dead => Ok(()),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// Whether V names a face (symbol or string), like GNU's face lookup.
fn face_exists(i: &Interp, v: &Value) -> bool {
    let mut name = match v {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        _ => return false,
    };
    // GNU: `face-alias' chains resolve to the target face.
    for _ in 0..16 {
        if crate::editor::face_known(i, &name) {
            return true;
        }
        let alias = i.get_prop(
            i.intern_soft(&name).unwrap_or(u32::MAX),
            i.q_soft("face-alias").unwrap_or(u32::MAX),
        );
        match i.sym_id(&alias).map(|s| i.symbol_name(s)) {
            Some(t) => name = t,
            None => return false,
        }
    }
    false
}

/// `face-name` — GNU faces.el returns the face's name as a *string*
/// (`symbol-name`), rejects string args with `wrong-type-argument
/// (symbolp ...)', and errors "Not a face: X" on anything else that
/// doesn't name an existing face.
fn f_face_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: strings (and other non-symbols) signal "Not a face: X".
    if let Some(s) = i.sym_id(&a[0]) {
        if face_exists(i, &a[0]) {
            return Ok(Value::string(i.symbol_name(s)));
        }
    }
    Err(i.error(format!("Not a face: {}", i.princ_to_string(&a[0]))))
}

fn f_face_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_opt_frame(i, a.get(1).unwrap_or(&Value::Nil))?;
    if !face_exists(i, &a[0]) {
        return Err(crate::editor::invalid_face(i, &a[0]));
    }
    // No fonts in batch.
    Ok(Value::Nil)
}

fn f_invert_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_opt_frame(i, a.get(1).unwrap_or(&Value::Nil))?;
    if !face_exists(i, &a[0]) {
        return Err(crate::editor::invalid_face(i, &a[0]));
    }
    // GNU internal-set-lisp-face-attribute path: swap the stored
    // :foreground/:background spec values (`unspecified' is legal and
    // stays unspecified on both sides).
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s),
        Value::Str(s) => s.borrow().clone(),
        _ => unreachable!(),
    };
    let fg_kw = i.q(":foreground");
    let bg_kw = i.q(":background");
    let _unsp = i.q("unspecified");
    let getv = |t: &Vec<(String, Value)>, n: &str, kw, miss: &str| -> Value {
        t.iter()
            .find(|(f, _)| f == n)
            .map(|(_, p)| crate::lisp::eval::plist_get(p, kw))
            .map(|v| if v.is_nil() { Value::string(miss) } else { v })
            .unwrap_or_else(|| Value::string(miss))
    };
    // GNU's tty realizes unset fg/bg as the "unspecified-fg"/
    // "unspecified-bg" pseudo-strings; unset slots swap those.
    let fg = getv(&i.face_table, &name, fg_kw, "unspecified-fg");
    let bg = getv(&i.face_table, &name, bg_kw, "unspecified-bg");
    if let Some((_, plist)) = i.face_table.iter_mut().find(|(n, _)| *n == name) {
        *plist = crate::lisp::eval::plist_put(plist, fg_kw, bg);
        *plist = crate::lisp::eval::plist_put(plist, bg_kw, fg);
    }
    Ok(a[0].clone())
}

/// GNU Fframe_or_buffer_changed_p (dispnew.c): the state snapshot
/// records every frame (object + name) and every LIVE buffer whose
/// name doesn't start with a space (object + read_only +
/// `buffer-modified-p' flag — NOT the modtick, so edits to an
/// already-dirty buffer don't count).  Space-named scratch buffers
/// like the driver's " *temp*" are skipped entirely.
fn frame_buffer_state(i: &mut Interp) -> Vec<Value> {
    let mut v: Vec<Value> = Vec::new();
    for f in &i.frames {
        let name = f.borrow().name.clone();
        v.push(Value::Frame(f.clone()));
        v.push(Value::Str(name));
    }
    for id in i.buffers.list() {
        let Some(b) = i.buffers.get(id) else {
            continue;
        };
        let (name, ro, modified) = {
            let bb = b.borrow();
            let ro = bb
                .locals
                .get(&i.q("buffer-read-only"))
                .cloned()
                .unwrap_or(Value::Nil);
            (bb.name.clone(), ro, bb.is_modified())
        };
        if name.starts_with(' ') {
            continue;
        }
        v.push(i.buffer_value(id).unwrap_or(Value::Nil));
        v.push(ro);
        v.push(Value::from_bool(modified));
    }
    v
}

fn f_frame_or_buffer_changed_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: VARIABLE is a symbol naming a state vector.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Sym(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("symbolp", other)),
    }
    let cur = frame_buffer_state(i);
    let (old, var) = match a.first() {
        Some(Value::Sym(id)) => {
            let id = *id;
            if id != sym::NIL {
                // GNU Fsymbol_value: unbound VARIABLE is void-variable.
                if !i.bound_p(id) {
                    return Err(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
                }
                let old = match i.symbol_value(id) {
                    Value::Vec(v) => Some(v.borrow().clone()),
                    _ => None,
                };
                (old, Some(id))
            } else {
                (i.frame_state_seen.clone(), None)
            }
        }
        _ => (i.frame_state_seen.clone(), None),
    };
    let changed = match &old {
        Some(o) => o.len() != cur.len() || !o.iter().zip(cur.iter()).all(|(x, y)| eq_values(x, y)),
        None => true,
    };
    if changed {
        if let Some(id) = var {
            i.set_symbol(id, Value::vector(cur))?;
        } else {
            i.frame_state_seen = Some(cur);
        }
    }
    Ok(Value::from_bool(changed))
}

fn f_image_transforms_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => Ok(Value::Nil),
        Some(Value::Frame(f)) if !f.borrow().dead => Ok(Value::Nil),
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

fn f_t(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

fn f_nil(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_newline_cache_check(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fnewline_cache_check:
    // optional BUFFER is CHECK_BUFFER; a buffer with no newline cache
    // (cache-long-scans nil — the default, and we never maintain one)
    // answers nil.
    match arg(&a, 0) {
        Value::Nil => {}
        Value::Buffer(_) => {}
        other => return Err(i.wrong_type_mut("bufferp", &other)),
    }
    Ok(Value::Nil)
}

/// `on_hot_spot_p' (xdisp.c) — does AREA contain pixel (X, Y)?
/// AREA is (rect . ((x0 . y0) . (x1 . y1))), (circle . ((x0 . y0) . r))
/// or (poly . [x0 y0 x1 y1 ...]).
fn on_hot_spot_p(i: &Interp, area: &Value, x: i128, y: i128) -> bool {
    let Value::Cons(hs) = area else {
        return false;
    };
    let (tag, body) = {
        let b = hs.borrow();
        (b.car.clone(), b.cdr.clone())
    };
    let Value::Sym(tag_id) = tag else {
        return false;
    };
    let cons_parts = |v: &Value| -> Option<(Value, Value)> {
        if let Value::Cons(c) = v {
            let b = c.borrow();
            Some((b.car.clone(), b.cdr.clone()))
        } else {
            None
        }
    };
    let fixnum = |v: &Value| -> Option<i128> {
        if let Value::Int(n) = v {
            Some(*n)
        } else {
            None
        }
    };
    match i.symbol_name(tag_id).as_str() {
        "rect" => {
            // ((x0 . y0) . (x1 . y1))
            let Some((tl, br)) = cons_parts(&body) else {
                return false;
            };
            let Some((x0v, y0v)) = cons_parts(&tl) else {
                return false;
            };
            let Some((x1v, y1v)) = cons_parts(&br) else {
                return false;
            };
            let (Some(x0), Some(y0), Some(x1), Some(y1)) =
                (fixnum(&x0v), fixnum(&y0v), fixnum(&x1v), fixnum(&y1v))
            else {
                return false;
            };
            x >= x0 && y >= y0 && x <= x1 && y <= y1
        }
        "circle" => {
            // ((x0 . y0) . r)
            let Some((ctr, rv)) = cons_parts(&body) else {
                return false;
            };
            let Some((x0v, y0v)) = cons_parts(&ctr) else {
                return false;
            };
            let (Some(x0), Some(y0)) = (fixnum(&x0v), fixnum(&y0v)) else {
                return false;
            };
            let r = match &rv {
                Value::Int(n) => *n as f64,
                Value::Float(f) => **f,
                _ => return false,
            };
            let dx = (x0 - x) as f64;
            let dy = (y0 - y) as f64;
            dx * dx + dy * dy <= r * r
        }
        "poly" => {
            // [x0 y0 x1 y1 x2 y2 ...]
            let Value::Vec(v) = &body else {
                return false;
            };
            let poly = v.borrow();
            let n = poly.len();
            if n < 6 || n % 2 != 0 {
                return false;
            }
            let mut inside = false;
            let (Some(mut x0), Some(mut y0)) = (fixnum(&poly[n - 2]), fixnum(&poly[n - 1])) else {
                return false;
            };
            for k in (0..n).step_by(2) {
                let x1 = x0;
                let y1 = y0;
                let (Some(nx), Some(ny)) = (fixnum(&poly[k]), fixnum(&poly[k + 1])) else {
                    return false;
                };
                x0 = nx;
                y0 = ny;
                // Does this segment cross the X line?
                if x0 >= x {
                    if x1 >= x {
                        continue;
                    }
                } else if x1 < x {
                    continue;
                }
                if y > y0 && y > y1 {
                    continue;
                }
                if y < y0 + ((y1 - y0) * (x - x0)) / (x1 - x0) {
                    inside = !inside;
                }
            }
            inside
        }
        _ => false,
    }
}

/// `lookup-image-map' — first map entry whose AREA contains (X, Y).
fn f_lookup_image_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if a[0].is_nil() {
        return Ok(Value::Nil);
    }
    let (x, y) = match (&a[1], &a[2]) {
        (Value::Int(x), Value::Int(y)) => (*x, *y),
        (other, _) if !matches!(other, Value::Int(_)) => {
            return Err(i.wrong_type_mut("fixnump", other));
        }
        (_, other) => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (entry, rest) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        cur = rest;
        if let Value::Cons(e) = &entry {
            let area = e.borrow().car.clone();
            if on_hot_spot_p(i, &area, x, y) {
                return Ok(entry);
            }
        }
    }
    Ok(Value::Nil)
}

fn f_false(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

/// Tag head of a `symbols-with-pos-enabled' symbol-with-position.
/// GNU's enabled sympos are a different object flavor from the
/// `symbol-with-pos' PVEC: `type-of' reports `symbol', `symbolp' is
/// t, symbol primitives work on them, and `eq' unwraps to the bare
/// symbol.  We encode them as `[Int(MAGIC) BARE POS]' records — the
/// magic Int head keeps detection `Interp'-free (`eq_values', hash
/// keys) and cannot be produced by any Lisp-visible record.
pub(crate) const SYMPOS_MAGIC: i128 = 0x5EED_CAFE_1;

/// `symbols-with-pos-enabled' parts: (bare symbol, position).
pub(crate) fn as_sympos(v: &Value) -> Option<(SymId, i128)> {
    if let Value::Record(r) = v {
        let rr = r.borrow();
        if let [Value::Int(tag), Value::Sym(s), Value::Int(p)] = rr.as_slice() {
            if *tag == SYMPOS_MAGIC {
                return Some((*s, *p));
            }
        }
    }
    None
}

/// Build an enabled `symbols-with-pos-enabled' sympos for BARE at POS.
pub(crate) fn make_sympos(bare: SymId, pos: i128) -> Value {
    Value::record(vec![
        Value::Int(SYMPOS_MAGIC),
        Value::Sym(bare),
        Value::Int(pos),
    ])
}

/// Strip an enabled sympos to its bare symbol; everything else is
/// returned unchanged.  Cheap because `as_sympos' borrows only when V
/// is a record.
pub(crate) fn unpos(v: &Value) -> Value {
    match as_sympos(v) {
        Some((s, _)) => Value::Sym(s),
        None => v.clone(),
    }
}

/// Is `symbols-with-pos-enabled' dynamically bound to a non-nil value?
pub(crate) fn sympos_enabled_p(i: &mut Interp) -> bool {
    let Some(id) = i.q_soft("symbols-with-pos-enabled") else {
        return false;
    };
    let v = i.symbol_value(id);
    !matches!(v, Value::Nil) && !matches!(v, Value::Sym(s) if s == crate::lisp::sym::UNBOUND)
}

/// Build a `symbol-with-pos' object for SYM at POS.
pub(crate) fn make_symbol_with_pos(i: &mut Interp, sym: SymId, pos: i128) -> Value {
    Value::record(vec![
        Value::Sym(i.q("symbol-with-pos")),
        Value::Sym(sym),
        Value::Int(pos),
    ])
}

fn f_bare_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if let Some((s, _)) = sym_pos_parts(i, &args[0]).or_else(|| as_sympos(&args[0])) {
        return Ok(i.sym(s));
    }
    match &args[0] {
        Value::Sym(_) | Value::Nil => Ok(args[0].clone()),
        other => {
            // GNU `Fbare_symbol' uses CHECK_SYMBOL_OR_SYMBOL_WITH_POS —
            // the signaled predicate is the two-element list.
            let swp = i.q("symbol-with-pos-p");
            let symbolp = i.q("symbolp");
            Err(i.signal_data(
                sym::WRONG_TYPE_ARGUMENT,
                vec![
                    Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                    other.clone(),
                ],
            ))
        }
    }
}

fn f_bare_symbol_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // t only for a plain symbol — a positioned symbol is not bare.
    Ok(Value::from_bool(
        matches!(&args[0], Value::Sym(_) | Value::Nil) && sym_pos_parts(i, &args[0]).is_none(),
    ))
}

fn f_position_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Fposition_symbol'
    // CHECK_SYMBOL_OR_SYMBOL_WITH_POS's SYM (the signaled predicate is
    // the two-element list) and accepts a fixnum or a positioned symbol
    // for POS (`fixnum-or-symbol-with-pos-p').
    let s = if let Some((s, _)) = sym_pos_parts(i, &args[0]).or_else(|| as_sympos(&args[0])) {
        s
    } else {
        match i.sym_id(&args[0]) {
            Some(s) => s,
            None => {
                let swp = i.q("symbol-with-pos-p");
                let symbolp = i.q("symbolp");
                return Err(i.signal_data(
                    sym::WRONG_TYPE_ARGUMENT,
                    vec![
                        Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                        args[0].clone(),
                    ],
                ));
            }
        }
    };
    let pos = match &args[1] {
        Value::Int(n) => *n,
        v => match sym_pos_parts(i, v).or_else(|| as_sympos(v)) {
            Some((_, p)) => p,
            None => return Err(i.wrong_type_mut("fixnum-or-symbol-with-pos-p", &args[1])),
        },
    };
    // GNU `Fposition_symbol': under `symbols-with-pos-enabled' the
    // result is a real positioned symbol; otherwise a
    // `symbol-with-pos' record.
    if sympos_enabled_p(i) {
        Ok(make_sympos(s, pos))
    } else {
        Ok(make_symbol_with_pos(i, s, pos))
    }
}

fn f_remove_pos_from_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `remove-pos-from-symbol' unwraps a positioned
    // symbol; any other object passes through unchanged (unlike
    // `bare-symbol' which type-checks).
    if let Some((s, _)) = sym_pos_parts(i, &args[0]).or_else(|| as_sympos(&args[0])) {
        return Ok(Value::Sym(s));
    }
    Ok(args[0].clone())
}

fn f_remove_images(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU checks START/END as integer-or-marker (BUFFER arg unused
    // here since image display props are ignored) and returns nil.
    let _ = crate::lisp::builtins::want_int_or_mark(i, &args[0])?;
    let _ = crate::lisp::builtins::want_int_or_mark(i, &args[1])?;
    Ok(Value::Nil)
}

fn f_symbol_with_pos_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        sym_pos_parts(i, &args[0]).is_some() || as_sympos(&args[0]).is_some(),
    ))
}

fn f_symbol_with_pos_pos(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match sym_pos_parts(i, &args[0]).or_else(|| as_sympos(&args[0])) {
        Some((_, p)) => Ok(Value::Int(p)),
        None => Err(i.wrong_type_mut("symbol-with-pos-p", &args[0])),
    }
}

fn f_make_var_non_special(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Finternal_make_var_non_special: CHECK_SYMBOL
    // on VARIABLE before clearing its special flag.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = false;
    Ok(Value::Nil)
}

fn f_special_variable_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match i.sym_id(&args[0]) {
        Some(id) => {
            // GNU: every interned keyword is self-quoting/defconst'd,
            // hence special.
            if i.symbol_name(id).starts_with(':') {
                return Ok(Value::t());
            }
            Ok(Value::from_bool(i.obarray.symbol(id).special))
        }
        None => Err(i.wrong_type_mut("symbolp", &args[0])),
    }
}

// ---------- time values ----------
//
// Lisp time is (HIGH LOW MICRO PICO) or an integer seconds/ticks. We
// convert to microseconds (i128) internally.

pub(crate) fn lisp_time_to_us(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000)
}

/// Like `lisp_time_to_us`, but nanosecond precision.
pub(crate) fn lisp_time_to_ns(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000_000)
}

fn scaled_lisp_time(i: &mut Interp, v: &Value, scale: i128) -> Result<i128, Flow> {
    let (ticks, hz) = decode_big_ticks_hz(i, v)?;
    (ticks * scale)
        .div_floor(&hz)
        .to_i128()
        .ok_or_else(|| i.error("Specified time is not representable"))
}

/// GNU CFORM_SECS_ONLY ignores fractional fields in old list timestamps.
pub(crate) fn lisp_time_seconds(i: &mut Interp, v: &Value) -> Result<i64, Flow> {
    let (ticks, hz) = decode_time_parts_big(i, v, true)?;
    ticks
        .div_floor(&hz)
        .to_i64()
        .ok_or_else(|| i.error("Specified time is not representable"))
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

/// Like `ns_to_lisp_time` but honors `current-time-list' — GNU's
/// `make_lisp_time' renders (TICKS . HZ) with ns ticks when the
/// variable is nil.
pub(crate) fn ns_to_lisp_time_i(i: &mut Interp, ns: i128) -> Value {
    if !current_time_list(i) {
        return Value::cons(Value::Int(ns), Value::Int(1_000_000_000));
    }
    ns_to_lisp_time(ns)
}

/// Like `lisp_time_to_ns`, but picosecond precision — GNU's native
/// tick unit (hz = 10^12).
pub(crate) fn lisp_time_to_ps(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    scaled_lisp_time(i, v, 1_000_000_000_000)
}

/// Minimal POSIX tm for `localtime_r` (macOS/Linux layout).
#[repr(C)]
#[derive(Clone, Copy)]
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
    pub(crate) fn gmtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    fn tzset();
    fn mktime(tm: *mut Tm) -> i64;
}

/// Process-global TZ lock: `tz_local_tm' and `mktime_secs' flip TZ +
/// call `tzset', and ambient readers (`local_tm', `mktime' with no
/// zone) depend on the CURRENT TZ.  GNU Emacs is effectively
/// single-threaded here; parallel Rust test threads race, so
/// serialize every TZ-sensitive section (holders call
/// `local_tm_locked' instead of `local_tm' — no recursion).
/// `set-time-zone-rule' itself never touches process state: it stores
/// the rule on the Interp (`tz_rule') and `interp_local_tm' applies it.
pub(crate) fn tz_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}
/// Local-time breakdown for a unix-second timestamp.
pub(crate) fn local_tm(secs: i64) -> Tm {
    let _g = tz_lock();
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { localtime_r(&secs, &mut tm) };
    tm
}

/// Same as `local_tm' but for callers already holding `tz_lock'.
fn local_tm_locked(secs: i64) -> Tm {
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { localtime_r(&secs, &mut tm) };
    tm
}

/// Local-time breakdown honoring the interpreter's `tz_rule'
/// (set by `set-time-zone-rule'); `None' falls back to the ambient
/// zone.  This is the single entry point every "local zone" reader
/// should use so one `Interp's rule can't leak into another's.
pub(crate) fn interp_local_tm(i: &Interp, secs: i64) -> Tm {
    interp_local_tm_named(i, secs).0
}

/// `interp_local_tm' plus the resolved zone abbreviation — under a
/// rule the breakdown comes from `tz_local_tm', whose restore
/// `tzset()' invalidates `tm_zone', so the name must come from the
/// pre-restore copy, not the returned `Tm' (which reads as garbage
/// like "LMT" on macOS).
pub(crate) fn interp_local_tm_named(i: &Interp, secs: i64) -> (Tm, String) {
    match &i.tz_rule {
        Some(z) => tz_local_tm(secs, z),
        None => {
            let t = local_tm(secs);
            let name = if t.tm_zone.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(t.tm_zone as *const i8) }
                    .to_string_lossy()
                    .into_owned()
            };
            (t, name)
        }
    }
}

/// UTC breakdown for a unix-second timestamp.
pub(crate) fn gmt_tm(secs: i64) -> Tm {
    let mut tm = unsafe { std::mem::zeroed::<Tm>() };
    unsafe { gmtime_r(&secs, &mut tm) };
    tm
}

/// Run one `strftime` conversion against a broken-down time.  Returns the
/// locale-formatted text (e.g. `%a` -> weekday abbrev, `%p` -> AM/PM).
/// The C library's locale is initialized once from the environment, like
/// GNU does at startup; an explicit LOCALE (`system-time-locale')
/// temporarily overrides it — GNU's Fformat_time_string does exactly
/// this pair of setlocale calls, falling back to "C" for unknown
/// locale names.
pub(crate) fn strftime_spec(fmt: &str, tm: &Tm, locale: Option<&str>) -> Option<String> {
    // setlocale/strftime share process-global C state — serialize with
    // the TZ lock so parallel threads can't observe a half-switched
    // locale (or vice versa).
    let _g = tz_lock();
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        libc::setlocale(libc::LC_TIME, c"".as_ptr());
    });
    // Only portable, unadorned locale directives reach libc. Field widths,
    // flags, numeric conversions and GNU extensions live in timefmt.rs.
    // Match gnulib's underlying_strftime buffer and sentinel: locale
    // composites can include a long caller-supplied zone abbreviation, and
    // a leading space distinguishes valid empty locale output from failure.
    let mut cfmt = format!(" {fmt}").into_bytes();
    cfmt.push(0);
    let mut buf = vec![0u8; 1024];
    // Our `Tm' mirrors the POSIX `struct tm' layout used by strftime.
    let n = if let Some(loc) = locale {
        unsafe {
            let cloc = std::ffi::CString::new(loc).unwrap_or_default();
            if libc::setlocale(libc::LC_TIME, cloc.as_ptr()).is_null() {
                // GNU: an unusable locale name falls back to "C".
                libc::setlocale(libc::LC_TIME, c"C".as_ptr());
            }
            let n = libc::strftime(
                buf.as_mut_ptr() as *mut i8,
                buf.len(),
                cfmt.as_ptr() as *const i8,
                tm as *const Tm as *const libc::tm,
            );
            libc::setlocale(libc::LC_TIME, c"".as_ptr());
            n
        }
    } else {
        unsafe {
            libc::strftime(
                buf.as_mut_ptr() as *mut i8,
                buf.len(),
                cfmt.as_ptr() as *const i8,
                tm as *const Tm as *const libc::tm,
            )
        }
    };
    if n == 0 {
        None
    } else {
        Some(String::from_utf8_lossy(&buf[1..n]).into_owned())
    }
}

/// Local-time breakdown under an explicit TZ spec (e.g. "Europe/Paris",
/// "UTC"), restoring the process TZ afterwards.  Also returns the
/// zone's abbreviation (`tm_zone`) — the pointer is copied BEFORE the
/// restore `tzset()` because on macOS it aliases libc-internal zone
/// state that `tzset` invalidates (reads as stale "LMT" garbage).
pub(crate) fn tz_local_tm(secs: i64, zone: &str) -> (Tm, String) {
    // GNU passes the TZ rule as a C string: ignore the suffix after NUL.
    let zone = zone.split('\0').next().unwrap_or("");
    let _g = tz_lock();
    let saved = std::env::var_os("TZ");
    unsafe {
        std::env::set_var("TZ", zone);
        tzset();
    }
    let tm = local_tm_locked(secs);
    let name = if tm.tm_zone.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(tm.tm_zone as *const i8) }
            .to_string_lossy()
            .into_owned()
    };
    unsafe {
        match saved {
            Some(v) => std::env::set_var("TZ", v),
            None => std::env::remove_var("TZ"),
        }
        tzset();
    }
    (tm, name)
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

/// POSIX mktime: interpret a (possibly out-of-range) broken-down time
/// in the given TZ spec — None means the ambient local zone — honoring
/// tm_isdst (-1 guess / 0 standard / >0 DST).  Returns epoch seconds.
fn mktime_secs(
    sec: i64,
    min: i64,
    hour: i64,
    day: i64,
    mon: i64,
    year: i64,
    isdst: i32,
    tz: Option<&str>,
) -> i64 {
    let _g = tz_lock();
    let mut tm: Tm = unsafe { std::mem::zeroed() };
    tm.tm_sec = sec as i32;
    tm.tm_min = min as i32;
    tm.tm_hour = hour as i32;
    tm.tm_mday = day as i32;
    tm.tm_mon = mon as i32 - 1;
    tm.tm_year = year as i32 - 1900;
    tm.tm_isdst = isdst;
    // GNU's gnulib mktime normalizes a forced tm_isdst to the zone's
    // real flag: an isdst=1 request in a non-DST zone (e.g. JST) must
    // produce the standard-offset instant, not the blindly-shifted
    // result libc mktime returns.  Pass 1 asks with the requested flag;
    // pass 2 re-runs with the flag mktime resolved to, which GNU
    // guarantees matches the zone's rule.
    // Run `f' with TZ=Z (restored after).  Named-zone mktime and the
    // roundtrip's localtime must share the same zone, so the TZ flip
    // brackets both passes.
    let with_tz = |z: Option<&str>, f: &mut dyn FnMut() -> i64| -> i64 {
        if let Some(z) = z {
            // NUL inside the spec would panic std::env::set_var; a NUL
            // string never names a real zone — fall back to UTC like
            // tzset does for unparseable specs.
            let z = if z.contains('\0') { "UTC0" } else { z };
            let saved = std::env::var_os("TZ");
            unsafe {
                std::env::set_var("TZ", z);
                tzset();
            }
            let r = f();
            unsafe {
                match saved {
                    Some(v) => std::env::set_var("TZ", v),
                    None => std::env::remove_var("TZ"),
                }
                tzset();
            }
            r
        } else {
            f()
        }
    };
    // GNU's gnulib mktime (lib/mktime.c): libc mktime on a forced
    // isdst=1 blind-shifts -1h even when the zone never observes DST
    // (e.g. Asia/Tokyo).  GNU resolves the request by probing
    // localtime around the result: when the resolved isdst differs
    // from the request, it scans ±1yr for an instant whose resolved
    // flag matches and extrapolates back; none found → keep the
    // zone's resolved interpretation.  Emulate: first pass asks libc,
    // then a localtime roundtrip normalizes fields under the resolved
    // flag; when the request is still unmet and the zone never shows
    // DST at the probed instants, the resolved result stands.
    let r = with_tz(tz, &mut || unsafe { mktime(&mut tm) });
    if isdst >= 0 && (tm.tm_isdst > 0) != (isdst > 0) {
        // Resolved flag disagrees with the request.  Find a nearby
        // instant (gnulib probes ±1yr, stride 601200s) whose resolved
        // flag matches the request; extrapolating its offset back is
        // exactly what libc already did on pass 1, so KEEP r when a
        // matching probe exists — the blind shift is the honored
        // interpretation.  Otherwise re-run under the resolved flag.
        let mut found = false;
        for delta in (601200i64..366 * 86400 + 601200).step_by(601200) {
            for dir in [-1i64, 1] {
                let ot = r.saturating_add(delta.saturating_mul(dir));
                let resolved = with_tz(tz, &mut || {
                    let mut otm: Tm = unsafe { std::mem::zeroed() };
                    unsafe { localtime_r(&ot, &mut otm) };
                    otm.tm_isdst as i64
                });
                if (resolved > 0) == (isdst > 0) {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            // Zone never shows the requested flag: return the
            // zone-resolved interpretation — re-run with the flag
            // libc resolved (fields were normalized in place, so
            // rebuild the tm from the request first).
            let mut tm2: Tm = unsafe { std::mem::zeroed() };
            tm2.tm_sec = sec as i32;
            tm2.tm_min = min as i32;
            tm2.tm_hour = hour as i32;
            tm2.tm_mday = day as i32;
            tm2.tm_mon = mon as i32 - 1;
            tm2.tm_year = year as i32 - 1900;
            tm2.tm_isdst = if tm.tm_isdst > 0 { 1 } else { 0 };
            return with_tz(tz, &mut || unsafe { mktime(&mut tm2) });
        }
    }
    r
}

/// How `encode-time' interprets ZONE: a fixed seconds-east-of-UTC
/// offset (t, "UTC", integer) uses pure calendar arithmetic; nil uses
/// the interpreter's zone rule (or ambient when unset), `wall' is the
/// startup wall clock (GNU's `wall_tz', immune to `set-time-zone-rule'),
/// other strings name a TZ spec.
enum ZoneKind {
    Fixed(i128),
    /// nil: the interpreter's current rule, or ambient zone.
    Local,
    /// `wall': ambient system zone regardless of `set-time-zone-rule'.
    Wall,
    Named(String),
}

fn decode_zone(i: &mut Interp, zone: &Value) -> Result<ZoneKind, Flow> {
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    // GNU decode_time_zone: t = UTC, "UTC"/"t" = UTC, integer = fixed
    // offset, a cons uses the car as a fixed offset, nil and `wall'
    // mean local; anything else signals "Invalid time zone
    // specification".
    Ok(match zone {
        Value::Int(off) if zone.fixnump() => ZoneKind::Fixed(*off),
        Value::Sym(s) if *s == t_sym => ZoneKind::Fixed(0),
        Value::Sym(s) if *s == wall_sym => ZoneKind::Wall,
        Value::Str(z) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                ZoneKind::Fixed(0)
            } else {
                ZoneKind::Named(z)
            }
        }
        Value::Cons(c) if c.borrow().car.fixnump() && matches!(c.borrow().cdr, Value::Cons(_)) => {
            let c = c.borrow();
            let Value::Int(offset) = c.car else {
                unreachable!()
            };
            let Value::Cons(tail) = &c.cdr else {
                unreachable!()
            };
            let abbreviation = tail.borrow().car.clone();
            if !matches!(abbreviation, Value::Str(_)) {
                return Err(i.wrong_type_mut("stringp", &abbreviation));
            }
            ZoneKind::Fixed(offset)
        }
        Value::Nil => ZoneKind::Local,
        v => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    })
}

pub(crate) fn time_tm(i: &mut Interp, secs: i64, zone: &Value) -> Result<Tm, Flow> {
    Ok(match decode_zone(i, zone)? {
        ZoneKind::Fixed(offset) => {
            let adjusted = (secs as i128)
                .checked_add(offset)
                .and_then(|n| i64::try_from(n).ok())
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            gmt_tm(adjusted)
        }
        ZoneKind::Local => interp_local_tm(i, secs),
        ZoneKind::Wall => local_tm(secs),
        ZoneKind::Named(zone) => tz_local_tm(secs, &zone).0,
    })
}

fn f_encode_time(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (encode-time TIME) where TIME is a list, or the obsolete
    // (encode-time SECOND MINUTE HOUR DAY MONTH YEAR &rest) form.
    // GNU signals `wrong-type-argument (consp X)' for a lone
    // non-list argument.
    // GNU: arities 2..5 signal `wrong-number-of-arguments (encode-time N)'
    // — only 1-arg (list) or ≥6-arg calls are valid.
    if args.len() >= 2 && args.len() <= 5 {
        let f = Value::Sym(i.q("encode-time"));
        return Err(i.wrong_number_of_args(&f, args.len() as i128));
    }
    let items: Vec<Value> = if args.len() == 1 {
        let mut items = Vec::new();
        let mut tail = args[0].clone();
        for _ in 0..6 {
            let Value::Cons(cell) = tail else {
                return Err(i.wrong_type_mut("consp", &tail));
            };
            let cell = cell.borrow();
            items.push(cell.car.clone());
            tail = cell.cdr.clone();
        }
        if !tail.is_nil() {
            for _ in 0..3 {
                let Value::Cons(cell) = tail else {
                    return Err(i.wrong_type_mut("consp", &tail));
                };
                let cell = cell.borrow();
                items.push(cell.car.clone());
                tail = cell.cdr.clone();
            }
        }
        items
    } else {
        args.as_slice().to_vec()
    };
    // GNU's Fencode_time CHECK_FIXNUMs the five fields after SEC —
    // nil (missing) elements signal `wrong-type-argument (fixnump nil)'.
    let get = |i: &mut Interp, k: usize| -> Result<i128, Flow> {
        match items.get(k) {
            Some(Value::Int(n)) => Ok(*n),
            other => Err(i.wrong_type_mut("fixnump", &other.cloned().unwrap_or(Value::Nil))),
        }
    };
    // SEC may be any Lisp time value — a fixnum, (HI LO), or
    // (TICKS . HZ); whole seconds add to the wall time and the
    // fraction becomes the sub-second part of the result.
    let (sec_whole, sub_ps) = match items.first() {
        Some(v @ (Value::Cons(_) | Value::Int(_) | Value::Float(_))) => {
            let (ticks, hz) = decode_ticks_hz(i, v)?;
            if hz <= 0 {
                return Err(i.error("Invalid time specification"));
            }
            (
                ticks.div_euclid(hz),
                ticks.rem_euclid(hz) * 1_000_000_000_000 / hz,
            )
        }
        other => return Err(i.wrong_type_mut("fixnump", &other.cloned().unwrap_or(Value::Nil))),
    };
    let (min, hour, day, mon, year) = (get(i, 1)?, get(i, 2)?, get(i, 3)?, get(i, 4)?, get(i, 5)?);
    // GNU's obsolescent calling convention (the call wasn't the
    // single-list form): when more than 6 args are given the LAST
    // argument is ZONE — middle extras are ignored — and DST is -1.
    // The one-list form reads ZONE at index 8 and DST at index 7.
    let (zone, isdst) = if args.len() > 1 {
        let zone = if items.len() > 6 {
            items.last().cloned().unwrap_or(Value::Nil)
        } else {
            Value::Nil
        };
        (zone, -1i32)
    } else {
        let zone = items.get(8).cloned().unwrap_or(Value::Nil);
        let t_sym = i.q("t");
        let isdst = match items.get(7) {
            Some(Value::Int(n)) => (*n).clamp(-1, 1) as i32,
            Some(Value::Sym(s)) if *s == t_sym => 1,
            // GNU list form: DST nil means "not in effect" (0), -1 asks
            // mktime to guess; absent fields guess too.
            Some(Value::Nil) => 0,
            _ => -1,
        };
        (zone, isdst)
    };
    let secs = match decode_zone(i, &zone)? {
        ZoneKind::Fixed(off) => {
            // Arithmetic path: normalize overflowing fields like
            // mktime does (Jan 32 -> Feb 1, hour 24 -> next day).
            let total_m = year * 12 + (mon - 1);
            let (ny, nm0) = (total_m.div_euclid(12), total_m.rem_euclid(12) + 1);
            let days = days_from_civil(ny, nm0, day);
            days * 86400 + hour * 3600 + min * 60 + sec_whole - off
        }
        ZoneKind::Local => mktime_secs(
            sec_whole as i64,
            min as i64,
            hour as i64,
            day as i64,
            mon as i64,
            year as i64,
            isdst,
            // The interpreter's `set-time-zone-rule', when set, IS the
            // local zone — don't read the process `TZ'.
            i.tz_rule.as_deref(),
        ) as i128,
        ZoneKind::Named(z) => mktime_secs(
            sec_whole as i64,
            min as i64,
            hour as i64,
            day as i64,
            mon as i64,
            year as i64,
            isdst,
            Some(&z),
        ) as i128,
        ZoneKind::Wall => mktime_secs(
            sec_whole as i64,
            min as i64,
            hour as i64,
            day as i64,
            mon as i64,
            year as i64,
            isdst,
            None,
        ) as i128,
    };
    // Emacs returns (HIGH LOW), plus (HIGH LOW USEC PSEC) when the
    // SEC argument carried a sub-second fraction.
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    if sub_ps == 0 {
        Ok(Value::list(vec![Value::Int(hi), Value::Int(lo)]))
    } else {
        Ok(Value::list(vec![
            Value::Int(hi),
            Value::Int(lo),
            Value::Int(sub_ps / 1_000_000),
            Value::Int(sub_ps % 1_000_000),
        ]))
    }
}

fn f_decode_time(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (ticks, hz) = if matches!(arg(&args, 2), Value::Sym(sym::T)) {
        decode_ticks_hz(i, &arg(&args, 0))?
    } else {
        (lisp_time_seconds(i, &arg(&args, 0))? as i128, 1)
    };
    let secs = i64::try_from(ticks.div_euclid(hz))
        .map_err(|_| i.error("Specified time is not representable"))?;
    // GNU decode_time_zone: nil/`wall' = local wall clock, t and
    // "UTC"/"Universal" = UTC, integer = fixed seconds-east offset,
    // other strings name a TZ spec, a (OFFSET . NAME) cons uses car.
    // Anything else signals "Invalid time zone specification".
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    let (tm, zone) = match args.get(1) {
        Some(Value::Int(off)) => (gmt_tm(secs + *off as i64), *off),
        Some(Value::Sym(s)) if *s == t_sym => (gmt_tm(secs), 0),
        Some(Value::Sym(s)) if *s == wall_sym => {
            // `wall' is the system wall clock — GNU keeps a separate
            // `wall_tz' allocated at startup, so `set-time-zone-rule'
            // does NOT affect it; use the ambient zone, not tz_rule.
            let t = local_tm(secs);
            let off = t.tm_gmtoff as i128;
            (t, off)
        }
        Some(Value::Str(z)) => {
            let z = z.borrow().clone();
            if z == "UTC" || z == "t" {
                (gmt_tm(secs), 0)
            } else {
                let (t, _zone_name) = tz_local_tm(secs, &z);
                let off = t.tm_gmtoff as i128;
                (t, off)
            }
        }
        Some(Value::Cons(c)) => {
            let off = match &c.borrow().car {
                Value::Int(n) => *n as i64,
                _ => 0,
            };
            (gmt_tm(secs + off), off as i128)
        }
        Some(Value::Nil) | None => {
            let t = interp_local_tm(i, secs);
            let off = t.tm_gmtoff as i128;
            (t, off)
        }
        Some(v) => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    Ok(Value::list(vec![
        if hz == 1 {
            Value::Int(tm.tm_sec as i128)
        } else {
            let seconds = (tm.tm_sec as i128)
                .checked_mul(hz)
                .and_then(|n| n.checked_add(ticks.rem_euclid(hz)))
                .ok_or_else(|| i.error("Specified time is not representable"))?;
            Value::cons(Value::Int(seconds), Value::Int(hz))
        },
        Value::Int(tm.tm_min as i128),
        Value::Int(tm.tm_hour as i128),
        Value::Int(tm.tm_mday as i128),
        Value::Int(tm.tm_mon as i128 + 1),
        Value::Int(tm.tm_year as i128 + 1900),
        Value::Int(tm.tm_wday as i128),
        Value::from_bool(tm.tm_isdst > 0),
        Value::Int(zone),
    ]))
}

/// `current-time-list': GNU defaults it to t; when the variable is
/// unbound (or bound non-nil) timestamps render in (HI LO US PS)
/// list form rather than (TICKS . HZ).
pub(crate) fn current_time_list(i: &mut Interp) -> bool {
    let id = i.q("current-time-list");
    !i.bound_p(id) || i.symbol_value(id).truthy()
}

/// Current timestamp for C-style callers such as window display.
/// GNU Fcurrent_time honors current-time-list and retains nanoseconds.
pub(crate) fn current_time_value(i: &mut Interp) -> Value {
    let ticks = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i128,
        Err(error) => -(error.duration().as_nanos() as i128),
    };
    if current_time_list(i) {
        ticks_hz_list4(ticks, 1_000_000_000)
    } else {
        Value::cons(Value::Int(ticks), Value::Int(1_000_000_000))
    }
}

/// Decode a Lisp time value to GNU's (TICKS . HZ) pair, per
/// decode_lisp_time in timefns.c (CFORM_TICKS_HZ):
///   int N      -> (N . 1)
///   float F    -> (F * 1e12 . 1e12)
///   (T . HZ)   -> (T . HZ), HZ > 0 required
///   (HI LO)    -> (HI*65536 + LO . 1)
///   (HI LO US) / (HI LO . US)  -> (*1e6 . 1e6)
///   (HI LO US PS ...)          -> (*1e12 . 1e12)
pub(crate) fn decode_ticks_hz(i: &mut Interp, v: &Value) -> Result<(i128, i128), Flow> {
    decode_time_parts(i, v, false)
}

/// Exact integer tick representation used by the Lisp time operations.
pub(crate) fn decode_big_ticks_hz(i: &mut Interp, value: &Value) -> Result<(BigInt, BigInt), Flow> {
    decode_time_parts_big(i, value, false)
}
fn decode_time_parts_big(
    i: &mut Interp,
    value: &Value,
    seconds_only: bool,
) -> Result<(BigInt, BigInt), Flow> {
    if let Some(n) = value.as_bigint() {
        return Ok((n, BigInt::from(1)));
    }
    match value {
        Value::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok((BigInt::from(now.as_nanos()), BigInt::from(1_000_000_000)))
        }
        Value::Float(n) => {
            let n = **n;
            if n.is_nan() {
                return Err(i.error("Invalid time specification"));
            }
            if !n.is_finite() {
                return Err(i.error("Specified time is not representable"));
            }
            if n == 0.0 {
                return Ok((BigInt::from(0), BigInt::from(1)));
            }
            // GNU double_integer_scale retains trailing mantissa zeros.
            let bits = n.to_bits();
            let exponent = ((bits >> 52) & 0x7ff) as i32;
            let fraction = bits & ((1u64 << 52) - 1);
            let (mantissa, exponent) = if exponent == 0 {
                (fraction, -1074)
            } else {
                (fraction | (1u64 << 52), exponent - 1075)
            };
            let mut ticks = BigInt::from(mantissa);
            let hz = if exponent < 0 {
                BigInt::from(1) << (-exponent) as usize
            } else {
                ticks <<= exponent as usize;
                BigInt::from(1)
            };
            if n.is_sign_negative() {
                ticks = -ticks;
            }
            Ok((ticks, hz))
        }
        Value::Cons(first) => {
            let (high, tail) = {
                let first = first.borrow();
                (first.car.clone(), first.cdr.clone())
            };
            let high = high
                .as_bigint()
                .ok_or_else(|| i.error("Invalid time specification"))?;
            let Value::Cons(second) = tail else {
                let hz = tail
                    .as_bigint()
                    .filter(|n| n.sign() == num_bigint::Sign::Plus)
                    .ok_or_else(|| i.error("Invalid time specification"))?;
                return Ok((high, hz));
            };
            let (low, tail) = {
                let second = second.borrow();
                (second.car.clone(), second.cdr.clone())
            };
            let low = low
                .as_bigint()
                .ok_or_else(|| i.error("Invalid time specification"))?;
            let seconds = high * 65536 + low;
            if seconds_only || tail.is_nil() {
                return Ok((seconds, BigInt::from(1)));
            }
            let (usec, psec, hz) = match tail {
                Value::Cons(third) => {
                    let third = third.borrow();
                    match &third.cdr {
                        Value::Cons(fourth) => (
                            third.car.clone(),
                            fourth.borrow().car.clone(),
                            1_000_000_000_000i128,
                        ),
                        _ => (third.car.clone(), Value::Int(0), 1_000_000),
                    }
                }
                usec => (usec, Value::Int(0), 1_000_000),
            };
            if !usec.fixnump() || !psec.fixnump() {
                return Err(i.error("Invalid time specification"));
            }
            let ticks = seconds * hz + usec.int().unwrap() * (hz / 1_000_000) + psec.int().unwrap();
            Ok((ticks, BigInt::from(hz)))
        }
        _ => Err(i.error("Invalid time specification")),
    }
}

/// Bounded projection for platform date/time interfaces. Keep the exact
/// frequency when it fits; otherwise retain the picoseconds they can use.
fn decode_time_parts(
    i: &mut Interp,
    value: &Value,
    seconds_only: bool,
) -> Result<(i128, i128), Flow> {
    let (ticks, hz) = decode_time_parts_big(i, value, seconds_only)?;
    if let (Some(ticks), Some(hz)) = (ticks.to_i128(), hz.to_i128()) {
        return Ok((ticks, hz));
    }
    let common = ticks.gcd(&hz);
    let (ticks, hz) = (ticks / &common, hz / common);
    if let (Some(ticks), Some(hz)) = (ticks.to_i128(), hz.to_i128()) {
        return Ok((ticks, hz));
    }
    let ps: BigInt = (ticks * 1_000_000_000_000i128).div_floor(&hz);
    ps.to_i128()
        .map(|ps| (ps, 1_000_000_000_000))
        .ok_or_else(|| i.error("Specified time is not representable"))
}
fn ticks_hz_list4_big(ticks: BigInt, hz: BigInt) -> Value {
    let trillion = BigInt::from(1_000_000_000_000i128);
    let ps = (ticks * &trillion).div_floor(&hz);
    let (seconds, fraction) = ps.div_mod_floor(&trillion);
    let (high, low) = seconds.div_mod_floor(&BigInt::from(65536));
    let (micro, pico) = fraction.div_mod_floor(&BigInt::from(1_000_000));
    Value::list(vec![
        Value::from_bigint(high),
        Value::from_bigint(low),
        Value::from_bigint(micro),
        Value::from_bigint(pico),
    ])
}
fn ticks_hz_list4(ticks: i128, hz: i128) -> Value {
    ticks_hz_list4_big(ticks.into(), hz.into())
}

/// True for a (TICKS . HZ) cons — a cons whose cdr is not a cons.
fn is_ticks_hz_form(v: &Value) -> bool {
    match v {
        Value::Cons(c) => !matches!(c.borrow().cdr, Value::Cons(_)),
        _ => false,
    }
}

/// GNU timefns.c `time_arith': decode both args to (TICKS . HZ), add or
/// subtract ticks (normalizing via lcm when the frequencies differ, and
/// never returning a resolution coarser than either input), then render:
/// hz == 1 yields an integer; a (TICKS . HZ) input, `current-time-list'
/// nil, or an hz that does not divide 1e12 yields (TICKS . HZ);
/// otherwise the (HI LO US PS) list form.
fn time_arith(i: &mut Interp, a: &[Value], sub: bool) -> EvalResult {
    let (ta, ha) = decode_big_ticks_hz(i, &a[0])?;
    let (tb, hb) = decode_big_ticks_hz(i, &a[1])?;
    let minimum = ha.clone().min(hb.clone());
    let same_frequency = ha == hb;
    let common = ha.gcd(&hb);
    let ma = &hb / &common;
    let mb = &ha / &common;
    let mut hz = ha * &ma;
    let mut ticks = if sub {
        ta * ma - tb * mb
    } else {
        ta * ma + tb * mb
    };
    if !same_frequency {
        // GNU reduces the result, then rescales to at least the smaller
        // input frequency. Keeping the raw lcm can change the output form.
        let reduction = ticks.gcd(&hz);
        if reduction > BigInt::from(1) {
            ticks /= &reduction;
            hz /= reduction;
            if hz < minimum {
                let scale = minimum.div_ceil(&hz);
                ticks *= &scale;
                hz *= scale;
            }
        }
    }
    if hz == BigInt::from(1) {
        return Ok(Value::from_bigint(ticks));
    }
    if is_ticks_hz_form(&a[0])
        || is_ticks_hz_form(&a[1])
        || !current_time_list(i)
        || !(BigInt::from(1_000_000_000_000i128) % &hz).is_zero()
    {
        Ok(Value::cons(
            Value::from_bigint(ticks),
            Value::from_bigint(hz),
        ))
    } else {
        Ok(ticks_hz_list4_big(ticks, hz))
    }
}

fn f_time_add(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    time_arith(i, &args, false)
}

fn f_time_subtract(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (time-subtract X X) takes the BASE_EQ shortcut and returns
    // zero in make_lisp_time form — (0 0 0 0) under current-time-list.
    if super::eq_values(&args[0], &args[1]) {
        return Ok(if current_time_list(i) {
            Value::list(vec![
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
                Value::Int(0),
            ])
        } else {
            Value::cons(Value::Int(0), Value::Int(1_000_000_000))
        });
    }
    time_arith(i, &args, true)
}

fn f_time_less_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (a, b) = (&args[0], &args[1]);
    if eq_values(a, b) {
        return Ok(Value::from_bool(false));
    }
    if let (Value::Cons(a), Value::Cons(b)) = (a, b) {
        let (a, b) = (a.borrow(), b.borrow());
        if eq_values(&a.cdr, &b.cdr) {
            if let (Some(a), Some(b)) = (a.car.as_bigint(), b.car.as_bigint()) {
                return Ok(Value::from_bool(a < b));
            }
        }
    }
    let (a, ah) = decode_big_ticks_hz(i, &args[0])?;
    let (b, bh) = decode_big_ticks_hz(i, &args[1])?;
    Ok(Value::from_bool(a * bh < b * ah))
}

fn f_time_equal_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (a, b) = (&args[0], &args[1]);
    if eq_values(a, b) {
        return Ok(Value::from_bool(true));
    }
    if let (Value::Cons(a), Value::Cons(b)) = (a, b) {
        let (a, b) = (a.borrow(), b.borrow());
        if eq_values(&a.cdr, &b.cdr) {
            if let (Some(a), Some(b)) = (a.car.as_bigint(), b.car.as_bigint()) {
                return Ok(Value::from_bool(a == b));
            }
        }
    }
    let (a, ah) = decode_big_ticks_hz(i, &args[0])?;
    let (b, bh) = decode_big_ticks_hz(i, &args[1])?;
    Ok(Value::from_bool(a * bh == b * ah))
}

fn f_time_convert(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let (ticks, hz) = decode_big_ticks_hz(i, &args[0])?;
    let form = args.get(1);
    let name = form
        .and_then(|v| i.sym_id(v))
        .map(|id| i.symbol_name(id))
        .unwrap_or_default();
    let default = form.is_none_or(Value::is_nil);
    if name == "list" || (default && current_time_list(i)) {
        return Ok(ticks_hz_list4_big(ticks, hz));
    }
    if name == "integer" {
        if args[0].integerp() {
            return Ok(args[0].clone());
        }
        return Ok(Value::from_bigint(ticks.div_floor(&hz)));
    }
    if name == "t" || default {
        if let Value::Cons(pair) = &args[0] {
            let pair = pair.borrow();
            if pair.car.integerp() && pair.cdr.integerp() {
                return Ok(args[0].clone());
            }
        }
        if args[0].integerp() {
            return Ok(Value::cons(args[0].clone(), Value::Int(1)));
        }
        return Ok(Value::cons(
            Value::from_bigint(ticks),
            Value::from_bigint(hz),
        ));
    }
    let form = form.unwrap();
    if let Some(frequency) = form
        .as_bigint()
        .filter(|n| n.sign() == num_bigint::Sign::Plus)
    {
        if let Value::Cons(pair) = &args[0] {
            if eq_values(form, &pair.borrow().cdr) {
                return Ok(args[0].clone());
            }
        }
        let converted = (ticks * frequency).div_floor(&hz);
        return Ok(Value::cons(Value::from_bigint(converted), form.clone()));
    }
    Err(i.signal_data(
        sym::ERROR,
        vec![Value::string("Invalid time frequency"), form.clone()],
    ))
}


fn f_load_average(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // getloadavg(3): 1/5/15-minute averages; GNU USE-FLOATS t
    // returns floats, otherwise scaled integers (*100).
    unsafe extern "C" {
        fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32;
    }
    let use_floats = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let mut v = [0.0f64; 3];
    let n = unsafe { getloadavg(v.as_mut_ptr(), 3) };
    let items: Vec<Value> = (0..n.max(0) as usize)
        .map(|k| {
            if use_floats {
                Value::float(v[k])
            } else {
                Value::Int((v[k] * 100.0) as i128)
            }
        })
        .collect();
    Ok(Value::list(items))
}

fn f_invocation_dir(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_build_binding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (VAR VALUE &optional BUFFER) — GNU's printed binding shape is
    // (VAR (and VALUE VAR)).  GNU specbinds VAR from a proper-list or
    // symbol binding cell; other atoms signal `(listp ARG)'.
    match &a[0] {
        Value::Sym(_) => {}
        Value::Nil | Value::Cons(_) => {
            // Cycle-safe proper-list check via `list_to_vec' (Dotted
            // carries the offending tail for the error data).
            if let Err(e) = a[0].list_to_vec() {
                let tail = match e {
                    crate::lisp::value::ListError::Dotted(t) => t,
                    crate::lisp::value::ListError::Circular(_) => Value::Nil,
                };
                return Err(i.wrong_type_mut("listp", if tail.is_nil() { &a[0] } else { &tail }));
            }
        }
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let and = Value::Sym(i.q("and"));
    Ok(Value::list(vec![
        a[0].clone(),
        Value::list(vec![and, a[1].clone(), a[0].clone()]),
    ]))
}

fn f_emacs_uptime(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let start = *START;
    let secs = start.elapsed().as_secs() as i128;
    if let Some(fmt) = a.get(0) {
        if fmt.truthy() {
            // GNU applies `format-seconds' to the uptime in seconds
            // (autoloaded from time-date).
            let fs = i.q("format-seconds");
            if !i.fbound_p(fs) {
                let q = |v: Value| Value::list(vec![Value::Sym(sym::QUOTE), v]);
                let req = Value::list(vec![
                    Value::Sym(i.q("require")),
                    q(Value::Sym(i.q("time-date"))),
                ]);
                let _ = i.eval(&req);
            }
            if i.fbound_p(fs) {
                let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
                let args = Value::list(vec![q(fmt), q(&Value::Int(secs))]);
                return i.call_function(&Value::Sym(fs), &args, None);
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

fn f_current_cpu_time(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU sysdep.c `get_current_time' for CPU time: on POSIX it is
    // `clock_gettime(CLOCK_PROCESS_CPUTIME_ID)' with microsecond ticks;
    // 31.1 returns (TICKS . HZ) with HZ = 1000000.
    let us = unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) == 0 {
            (ts.tv_sec as i128) * 1_000_000 + (ts.tv_nsec as i128) / 1_000
        } else {
            // getrusage fallback (user + sys).
            let mut ru: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
                (ru.ru_utime.tv_sec as i128 + ru.ru_stime.tv_sec as i128) * 1_000_000
                    + (ru.ru_utime.tv_usec as i128 + ru.ru_stime.tv_usec as i128)
            } else {
                0
            }
        }
    };
    Ok(Value::cons(Value::Int(us), Value::Int(1_000_000)))
}

static START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// `set-time-zone-rule' — GNU's set_time_zone_rule: adopt TZ as the
/// process-local zone (like `(setenv "TZ" TZ)' minus the
/// `process-environment' entry).  nil/`wall' restores system wall
/// clock (TZ unset); t = "UTC0"; an integer offset or a (OFFSET NAME)
/// cons uses a fabricated POSIX spec.  Returns nil.
fn f_set_time_zone(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _g = tz_lock();
    let t_sym = i.q("t");
    let wall_sym = i.q("wall");
    /// GNU's name for an integer zone: sign plus HH, then MM/SS only
    /// when non-zero ("+01", "+0130", "+010101").
    fn offset_name(off: i64) -> String {
        let sign = if off < 0 { '-' } else { '+' };
        let a = off.abs();
        let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
        if s != 0 {
            format!("{}{:02}{:02}{:02}", sign, h, m, s)
        } else if m != 0 {
            format!("{}{:02}{:02}", sign, h, m)
        } else {
            format!("{}{:02}", sign, h)
        }
    }
    /// POSIX TZ field for a fixed offset: POSIX inverts the sign
    /// (the field is what to subtract from local time to get UTC).
    fn posix_off(off: i64) -> String {
        let sign = if off > 0 { '-' } else { '+' };
        let a = off.abs();
        let (h, m, s) = (a / 3600, (a % 3600) / 60, a % 60);
        if s != 0 {
            format!("{}{}:{:02}:{:02}", sign, h, m, s)
        } else if m != 0 {
            format!("{}{}:{:02}", sign, h, m)
        } else {
            format!("{}{}", sign, h)
        }
    }
    /// Fabricate "NAME±H[:M[:S]]"; POSIX needs >=3 chars or <...>
    /// quoting for NAME, and macOS honors "<+01>" but not "<X>", so
    /// short/empty names use the offset's own "+HH" rendering.
    fn fixed_spec(name: &str, off: i64) -> String {
        let name = if name.len() >= 3 {
            name.to_string()
        } else {
            offset_name(off)
        };
        format!("<{}>{}", name, posix_off(off))
    }
    let spec: Option<String> = match &a[0] {
        Value::Str(z) => Some(z.borrow().clone()),
        Value::Sym(s) if *s == t_sym => Some("UTC0".to_string()),
        Value::Sym(s) if *s == wall_sym => None,
        Value::Int(off) => Some(fixed_spec("", *off as i64)),
        Value::Cons(c) => {
            let (off, name) = {
                let b = c.borrow();
                let off = match &b.car {
                    Value::Int(n) => *n as i64,
                    _ => 0,
                };
                let name = match &b.cdr {
                    Value::Cons(c2) => match &c2.borrow().car {
                        Value::Str(s) => s.borrow().clone(),
                        _ => String::new(),
                    },
                    Value::Str(s) => s.borrow().clone(),
                    _ => String::new(),
                };
                (off, name)
            };
            Some(fixed_spec(&name, off))
        }
        Value::Nil => None,
        v => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Invalid time zone specification"), v.clone()],
            ));
        }
    };
    // Unlike GNU we do NOT set the process `TZ'/call `tzset': the rule
    // lives on the Interp so parallel interpreters never see each
    // other's zone (libtest runs them in the same process).
    i.tz_rule = spec;
    Ok(Value::Nil)
}

// ---------- hashing / base64 ----------

fn hash_bytes(algo: &str, bytes: &[u8]) -> Result<Vec<u8>, Flow> {
    use sha2::Digest;
    let out = match algo {
        "md5" => md5::Md5::digest(bytes).to_vec(),
        "sha1" => sha1::Sha1::digest(bytes).to_vec(),
        "sha224" => sha2::Sha224::digest(bytes).to_vec(),
        "sha256" => sha2::Sha256::digest(bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(bytes).to_vec(),
        "sha512" => sha2::Sha512::digest(bytes).to_vec(),
        "sha3-224" => sha3::Sha3_224::digest(bytes).to_vec(),
        "sha3-256" => sha3::Sha3_256::digest(bytes).to_vec(),
        "sha3-384" => sha3::Sha3_384::digest(bytes).to_vec(),
        "sha3-512" => sha3::Sha3_512::digest(bytes).to_vec(),
        _ => return Err(Flow::signal(Value::Nil, Value::Nil)),
    };
    Ok(out)
}

fn digest_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hash_hex(algo: &str, bytes: &[u8]) -> Result<String, Flow> {
    hash_bytes(algo, bytes).map(|digest| digest_hex(&digest))
}

fn secure_hash_bytes(i: &mut Interp, args: &[Value]) -> Result<Vec<u8>, Flow> {
    // GNU Fsecure_hash: CHECK_SYMBOL first — a non-symbol algorithm
    // signals `wrong-type-argument symbolp'.
    let algo_sym = match i.sym_id(&args[0]) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &args[0])),
    };
    let algo = i.symbol_name(algo_sym);
    let obj = &args[1];
    // GNU hashes the string's byte representation: unibyte strings
    // hash their raw bytes, multibyte their internal multibyte form
    // (eight-bit chars as single 0x80–0xFF bytes).  Raw UTF-8 of the
    // PUA proxy is WRONG — `lisp_string_bytes' knows the mapping.
    let mut bytes: Vec<u8> = match obj {
        Value::Str(_) => lisp_string_bytes(i, obj),
        Value::Buffer(b) => {
            // Same byte mapping as lisp_string_bytes but over buffer
            // text (buffer may hold eight-bit proxy chars).
            let mut out = Vec::new();
            for c in b.borrow().text.text().chars() {
                if let Some(byte) = crate::lisp::value::eight_bit_byte(c) {
                    out.push(byte);
                } else {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
            }
            out
        }
        Value::Nil => {
            // GNU Fsecure_hash: nil isn't the current buffer — it
            // falls past STRINGP/BUFFERP/OVERLAYP into
            // (error "Invalid object argument" "nil").
            return Err(i.signal_data(
                sym::ERROR,
                vec![
                    Value::string("Invalid object argument"),
                    Value::string("nil"),
                ],
            ));
        }
        _ => {
            // GNU signal_error builds data = cons(message, OBJECT):
            // a LIST object splices into the data tail, an ATOM
            // stays as the single last element:
            // `(sha1 '(a b))' → ("Invalid object argument" a b),
            // `(sha1 'a)' → ("Invalid object argument" a).
            let tail = match &obj {
                Value::Cons(_) => obj.clone(),
                other => Value::cons((*other).clone(), Value::Nil),
            };
            return Err(i.signal(
                sym::ERROR,
                Value::cons(Value::string("Invalid object argument"), tail),
            ));
        }
    };
    // GNU: (secure-hash ALGORITHM OBJECT &optional START END BINARY) —
    // hash only OBJECT[START..END] (byte offsets); nil means default.
    if let Some(st) = args.get(2) {
        // GNU validate_subarray uses CHECK_INTEGER on START/END —
        // `integerp' (floats, conses all rejected).
        let start = match st {
            Value::Nil => 0usize,
            Value::Int(n) => (*n).max(0) as usize,
            other => return Err(i.wrong_type_mut("integerp", other)),
        };
        let end = match args.get(3) {
            Some(Value::Nil) | None => bytes.len(),
            Some(Value::Int(n)) => (*n).max(0) as usize,
            Some(v) => return Err(i.wrong_type_mut("integerp", v)),
        };
        if start > bytes.len() || end > bytes.len() || start > end {
            return Err(i.signal_data(
                sym::ARGS_OUT_OF_RANGE,
                vec![
                    obj.clone(),
                    Value::Int(start as i128),
                    Value::Int(end as i128),
                ],
            ));
        }
        bytes = bytes[start..end].to_vec();
    }
    hash_bytes(&algo, &bytes).map_err(|_| {
        i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Invalid algorithm arg: {}", algo))],
        )
    })
}

fn f_secure_hash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let digest = secure_hash_bytes(i, &args)?;
    if arg(&args, 4).truthy() {
        // Digest octets are unibyte characters, including NUL and 0x80..0xff.
        let value = Value::string(digest.iter().map(|&byte| byte as char).collect::<String>());
        if let Value::Str(string) = &value {
            i.mark_unibyte(string);
        }
        Ok(value)
    } else {
        Ok(Value::string(digest_hex(&digest)))
    }
}

fn f_md5(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let mut a = vec![Value::Sym(i.q("md5"))];
    a.extend_from_slice(args.as_slice());
    // MD5's optional coding-system argument must not become BINARY.
    Ok(Value::string(digest_hex(&secure_hash_bytes(i, &a)?)))
}

/// The byte sequence a Lisp string stands for: unibyte strings use
/// their chars' values directly; multibyte strings encode each char
/// as UTF-8 (eight-bit chars emit their raw byte).
pub(crate) fn lisp_string_bytes(i: &Interp, v: &Value) -> Vec<u8> {
    let unibyte = match v {
        Value::Str(r) => i.is_unibyte_str(r),
        _ => false,
    };
    let s = match v {
        Value::Str(r) => r.borrow().clone(),
        _ => String::new(),
    };
    if unibyte {
        return s.chars().map(|c| c as u8).collect();
    }
    let mut out = Vec::new();
    for c in s.chars() {
        if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
            out.push(b);
        } else {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    out
}

fn f_b64_encode_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let _s = want_string(i, &args[0])?;
    let no_break = arg(&args, 1).truthy();
    // GNU pads unconditionally; NO-LINE-BREAK only suppresses the
    // 76-column line wrapping.
    let enc = base64::engine::general_purpose::STANDARD.encode(lisp_string_bytes(i, &args[0]));
    let out = if no_break || enc.len() <= 76 {
        enc
    } else {
        let mut wrapped = String::with_capacity(enc.len() + enc.len() / 76);
        for chunk in enc.as_bytes().chunks(76) {
            wrapped.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            wrapped.push('\n');
        }
        wrapped.pop();
        wrapped
    };
    Ok(Value::string(out))
}

fn f_b64_decode_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU base64_decode_1 (lib/base64.c via fns.c):
    // - optional BASE64URL switches to the RFC 4648 URL alphabet,
    // - optional IGNORE-INVALID drops non-alphabet chars instead of
    //   erroring,
    // - whitespace is always skipped,
    // - '=' padding ends the input; a trailing partial group of 2 or
    //   3 chars still decodes (1 leftover char signals), and padding
    //   is *not required* — GNU pads implicitly.
    let s = want_string(i, &args[0])?;
    let url = arg(&args, 1).truthy();
    let ignore_invalid = arg(&args, 2).truthy();
    // Alphabet lookup: index of c or 0x100/0x101 sentinel.
    let b64val = |c: u8| -> u32 {
        if url {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'-' => 62,
                b'_' => 63,
                _ => 0x100,
            }
        } else {
            match c {
                b'A'..=b'Z' => (c - b'A') as u32,
                b'a'..=b'z' => (c - b'a' + 26) as u32,
                b'0'..=b'9' => (c - b'0' + 52) as u32,
                b'+' => 62,
                b'/' => 63,
                _ => 0x100,
            }
        }
    };
    let mut out: Vec<u8> = Vec::new();
    let mut quad: [u32; 4] = [0; 4];
    let mut n = 0usize;
    let invalid =
        || -> Flow { i.signal_data(sym::ERROR, vec![Value::string("Invalid base64 data")]) };
    let mut pad_await = false; // '=' seen at quad pos 2: await closing '='
    for &c in s.as_bytes() {
        // Whitespace is always ignored.
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) {
            continue;
        }
        if c == b'=' {
            // GNU pad rules: '=' is legal only inside an open quad.
            // At position 3 it emits 2 bytes and closes the quad; at
            // position 2 it must be followed by one more '=' (the
            // canonical "…==" tail) which then emits 1 byte.  A bare
            // '=' with no pending chars — or data arriving while a pad
            // is awaited — is invalid.
            if pad_await {
                let w = (quad[0] << 18) | (quad[1] << 12);
                out.push((w >> 16) as u8);
                pad_await = false;
                n = 0;
            } else if n == 3 {
                let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6);
                out.push((w >> 16) as u8);
                out.push((w >> 8) as u8);
                n = 0;
            } else if n == 2 {
                pad_await = true;
            } else {
                return Err(invalid());
            }
            continue;
        }
        if pad_await {
            // Inside "XX==" only the closing '=' is legal.
            return Err(invalid());
        }
        let v = b64val(c);
        if v == 0x100 {
            if ignore_invalid {
                continue;
            }
            return Err(invalid());
        }
        quad[n] = v;
        n += 1;
        if n == 4 {
            let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6) | quad[3];
            out.push((w >> 16) as u8);
            out.push((w >> 8) as u8);
            out.push(w as u8);
            n = 0;
        }
    }
    // EOF: an awaited-but-unclosed "XX=" pad is invalid, as is a
    // leftover single char; a 2/3-char tail is invalid in strict mode
    // but GNU emits its partial bytes under IGNORE-INVALID.
    if pad_await || n == 1 || (n > 0 && !ignore_invalid) {
        return Err(invalid());
    }
    match n {
        2 => {
            let w = (quad[0] << 18) | (quad[1] << 12);
            out.push((w >> 16) as u8);
        }
        3 => {
            let w = (quad[0] << 18) | (quad[1] << 12) | (quad[2] << 6);
            out.push((w >> 16) as u8);
            out.push((w >> 8) as u8);
        }
        _ => {}
    }
    let v = Value::string(out.iter().map(|&b| b as char).collect::<String>());
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}

fn f_buffer_hash(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (buffer-hash BUFFER) = sha1 hex of the buffer's text.
    let b = match args.get(0) {
        // GNU decodes via Fget_buffer: nil -> current buffer, a name
        // resolves or signals `No buffer named X'.  Non-string non-buffer
        // args signal wrong-type-argument STRINGP (Fget_buffer's check).
        None | Some(Value::Nil) => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("No current buffer"))?,
        Some(Value::Buffer(b)) => b.clone(),
        Some(Value::Str(name)) => {
            let name = name.borrow().clone();
            match i.buffers.by_name(&name).and_then(|id| i.buffers.get(id)) {
                Some(b) => b,
                None => return Err(i.error(format!("No buffer named {}", name))),
            }
        }
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let text = b.borrow().text.text();
    match hash_hex("sha1", text.as_bytes()) {
        Ok(h) => Ok(Value::string(h)),
        Err(_) => unreachable!(),
    }
}

// ---------- files ----------

fn f_make_temp_file_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    // GNU Fmake_temp_file_internal: PREFIX is used VERBATIM — never
    // expanded against temporary-file-directory — so a relative prefix
    // resolves in the working directory (make-temp-file wraps it to
    // expand first).  DIR-FLAG selects create-file (nil), name-only
    // verification (0) or mkdir (other non-nil).  "X"*6 is filled from
    // the base-62 alphabet; files get mode 0600, dirs 0700.
    let prefix = want_string(i, &args[0])?;
    let dir_flag = arg(&args, 1);
    let suffix = match arg(&args, 2) {
        Value::Str(s) => s.borrow().clone(),
        v => return Err(i.wrong_type_mut("stringp", &v)),
    };
    let text = arg(&args, 3);
    enum Kind {
        File,
        NoCreate,
        Dir,
    }
    let kind = if dir_flag.is_nil() {
        Kind::File
    } else if matches!(dir_flag, Value::Int(0)) {
        Kind::NoCreate
    } else {
        Kind::Dir
    };
    let kind_message = match kind {
        Kind::File => "Creating file with prefix",
        Kind::Dir => "Creating directory with prefix",
        Kind::NoCreate => "Creating file name with prefix",
    };
    let mut counter = 0u64;
    for _ in 0..62 * 62 * 62 {
        counter = counter.wrapping_add(1);
        let cand = format!("{}{}{}", prefix, tempname_suffix(counter), suffix);
        let res: Result<Option<std::fs::File>, std::io::Error> = match kind {
            Kind::Dir => std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&cand)
                .map(|_| None),
            Kind::NoCreate => match std::fs::symlink_metadata(&cand) {
                Ok(_) => Err(std::io::Error::from_raw_os_error(libc::EEXIST)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            },
            Kind::File => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&cand)
                .map(Some),
        };
        match res {
            Ok(mut f) => {
                if let (Kind::File, Some(fh)) = (&kind, &mut f) {
                    if matches!(&text, Value::Str(string) if !string.borrow().is_empty()) {
                        use std::io::Write;
                        let bytes = crate::editor::temp_file_text_bytes(i, &cand, &text)?;
                        if let Err(e) = fh.write_all(&bytes) {
                            let _ = std::fs::remove_file(&cand);
                            return Err(crate::editor::report_file_errno(
                                i,
                                "Write error",
                                vec![Value::string(cand)],
                                &e,
                            ));
                        }
                    }
                }
                return Ok(Value::string(cand));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                // GNU report_file_error: the *unexpanded* PREFIX is the
                // reported name, errno picks the symbol.
                return Err(crate::editor::report_file_errno(
                    i,
                    kind_message,
                    vec![args[0].clone()],
                    &e,
                ));
            }
        }
    }
    Err(crate::editor::report_file_errno(
        i,
        kind_message,
        vec![args[0].clone()],
        &std::io::Error::from_raw_os_error(libc::EEXIST),
    ))
}

/// gnulib `gen_tempname' fills "X"*6 from this base-62 alphabet.
fn tempname_suffix(seed: u64) -> String {
    const LETTERS: &[u8; 62] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
        ^ ((std::process::id() as u64) << 16)
        ^ seed.wrapping_mul(0x9E3779B97F4A7C15);
    let mut v = n ^ n.rotate_left(32);
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        out.push(LETTERS[(v % 62) as usize] as char);
        v /= 62;
    }
    out
}

fn f_make_symbolic_link(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_symbolic_link: TARGET stays verbatim (only an integer
    // OK-IF-ALREADY-EXISTS expands a leading `~'), while LINKNAME is
    // expand_cp_target'd — expanded, and a trailing-slash LINKNAME means
    // "TARGET's basename under that directory".
    let target = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut target = target;
    let ok_if_exists = args.get(2).cloned().unwrap_or(Value::Nil);
    if matches!(ok_if_exists, Value::Int(_)) && target.starts_with('~') {
        target = crate::editor::expand_file_name_str(i, &target);
    }
    let name_raw = match &args[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let name = if name_raw.ends_with('/') {
        // expand_cp_target: LINKNAME a directory name → LINKNAME/basename(TARGET).
        let base = target
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&target);
        crate::editor::expand_file_name_str(i, &format!("{}{}", name_raw, base))
    } else {
        crate::editor::expand_file_name_str(i, &name_raw)
    };
    match std::os::unix::fs::symlink(&target, &name) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            match &ok_if_exists {
                // GNU barf_or_query_if_file_exists: nil →
                // file-already-exists; an integer would ask (batch → EOF).
                Value::Nil => {
                    let s = i.q("file-already-exists");
                    Err(i.signal_data(
                        s,
                        vec![Value::string("File already exists"), Value::string(name)],
                    ))
                }
                Value::Int(_) => {
                    let s = i.q("end-of-file");
                    Err(i.signal_data(s, vec![Value::string("Error reading from stdin")]))
                }
                // Other non-nil: unlink and retry, like GNU.
                _ => {
                    let _ = std::fs::remove_file(&name);
                    match std::os::unix::fs::symlink(&target, &name) {
                        Ok(()) => Ok(Value::Nil),
                        Err(e2) => Err(crate::editor::report_file_errno(
                            i,
                            "Making symbolic link",
                            vec![Value::string(target), Value::string(name)],
                            &e2,
                        )),
                    }
                }
            }
        }
        // GNU report_file_error: ("Making symbolic link" strerror TARGET
        // LINKNAME) — errno picks file-missing/permission-denied/file-error.
        Err(e) => Err(crate::editor::report_file_errno(
            i,
            "Making symbolic link",
            vec![Value::string(target), Value::string(name)],
            &e,
        )),
    }
}

fn f_directory_name_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::from_bool(s.ends_with('/')))
}

pub(crate) fn f_file_name_case_insensitive_p(
    i: &mut Interp,
    args: crate::lisp::Argv,
) -> EvalResult {
    let path = want_string(i, &args[0])?;
    // GNU fileio.c `check_case_insensitive': query the FILESYSTEM of
    // the parent directory (expand-file-name + directory-name), so a
    // nonexistent file under an existing dir still answers t on APFS.
    let expanded = crate::editor::expand_file_name_str(i, &path);
    let dir = {
        let t = expanded.trim_end_matches('/');
        match t.rfind('/') {
            Some(0) => "/".to_string(),
            Some(k) => t[..k].to_string(),
            None => ".".to_string(),
        }
    };
    #[cfg(unix)]
    {
        use std::ffi::CString;
        if let Ok(c) = CString::new(dir) {
            // _PC_CASE_SENSITIVE (11): 0 → case-insensitive FS → t;
            // 1 → sensitive → nil; -1/EINVAL → unknown → nil.
            let r = unsafe { libc::pathconf(c.as_ptr(), 11) };
            if r == 0 {
                return Ok(Value::t());
            }
            if r == 1 {
                return Ok(Value::Nil);
            }
            // Fall through to the probe on error.
        }
    }
    // Fallback (or non-unix): probe by statting a case-flipped name.
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

fn f_file_attributes_lessp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_LIST on both attribute lists.
    for v in args.iter().take(2) {
        match v {
            Value::Nil | Value::Cons(_) => {}
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
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

fn f_set_file_times(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_file_times order: symlink_nofollow_flag (FLAG), then
    // lisp_time_argument (TIMESTAMP), then expand FILENAME — so a bogus
    // TIMESTAMP signals "Invalid time specification" before FILENAME is
    // checked.
    // GNU symlink_nofollow_flag treats every non-nil value as nofollow.
    let nofollow = args.get(2).is_some_and(Value::truthy);
    let time = args.get(1).cloned().unwrap_or(Value::Nil);
    let ns = lisp_time_to_ns(i, &time)?;
    let name = {
        let f = want_string(i, &args[0])?;
        crate::editor::expand_file_name_str(i, &f)
    };
    if let Some((handler, op)) = crate::editor::file_handler(i, "set-file-times", &[&name]) {
        return i.apply(
            &handler,
            vec![
                op,
                crate::editor::file_name_argument(&args[0], &name),
                time,
                args.get(2).cloned().unwrap_or(Value::Nil),
            ],
        );
    }
    let secs = ns.div_euclid(1_000_000_000) as i64;
    let nsec = ns.rem_euclid(1_000_000_000) as i64;
    #[cfg(unix)]
    {
        let c = std::ffi::CString::new(name.clone())
            .map_err(|_| i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")]))?;
        let ts = libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: nsec as _,
        };
        let times = [ts, ts];
        let flag = if nofollow {
            libc::AT_SYMLINK_NOFOLLOW
        } else {
            0
        };
        let r = unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), flag) };
        if r != 0 {
            // GNU report_file_error: ("Setting file times" strerror ABSNAME),
            // errno choosing file-missing/permission-denied/file-error.
            let e = std::io::Error::last_os_error();
            return Err(crate::editor::report_file_errno(
                i,
                "Setting file times",
                vec![Value::string(name)],
                &e,
            ));
        }
    }
    Ok(Value::t())
}

fn f_get_file_buffer(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let predicate = args.get(1).cloned();
    // GNU files.el `find-buffer-visiting' (our find-buffer-visiting
    // shares this entry point): get-file-buffer first, then the
    // truename lookup — a buffer is eligible only when PREDICATE (if
    // any) passes on it.
    let canonical = crate::editor::expand_file_name_str(i, &name);
    let mut check = |i: &mut Interp, id: usize| -> EvalResult {
        match &predicate {
            None | Some(Value::Nil) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
            Some(p) => {
                let bv = i.buffer_value(id).unwrap_or(Value::Nil);
                match i.apply(p, vec![bv]) {
                    Err(fl) => Err(fl),
                    Ok(v) if v.truthy() => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
                    _ => Ok(Value::Nil),
                }
            }
        }
    };
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let matches = {
                let bb = b.borrow();
                bb.file_name.as_deref() == Some(canonical.as_str())
            };
            if matches {
                let r = check(i, id)?;
                if r.truthy() {
                    return Ok(r);
                }
            }
        }
    }
    if predicate.is_none() {
        return Ok(Value::Nil);
    }
    // (get-truename-buffer truename): buffer-file-truename equal.
    let truename = crate::buffer::file_truename(&canonical);
    let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let matches = {
                let bb = b.borrow();
                match bb.locals.get(&bft) {
                    Some(Value::Str(t)) => *t.borrow() == truename,
                    _ => false,
                }
            };
            if matches {
                let r = check(i, id)?;
                if r.truthy() {
                    return Ok(r);
                }
            }
        }
    }
    Ok(Value::Nil)
}

// ---------- environment / user ----------

fn f_setenv(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let val = match args.get(1) {
        Some(Value::Nil) | None => None,
        Some(v) => Some(want_string(i, v)?),
    };
    // GNU `setenv' only modifies `process-environment', not the OS
    // environment.
    let pe = i.q("process-environment");
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

/// GNU `setenv-internal': (ENV VARIABLE VALUE KEEP-EMPTY) — set
/// VARIABLE to VALUE in the list ENV by side effect; the matching
/// element ("VAR=..." or a bare "VAR") is replaced in place, else
/// "VAR=VALUE" is prepended.  Returns the new list head.
fn f_setenv_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let env = arg(&args, 0);
    let var = want_string(i, &args[1])?;
    let val = want_string(i, &args[2])?;
    let new_entry = format!("{}={}", var, val);
    let mut cur = env.clone();
    while let Value::Cons(c) = &cur {
        let cc = c.clone();
        let hit = match &cc.borrow().car {
            Value::Str(s) => {
                let s = s.borrow();
                *s == var || s.starts_with(&format!("{}=", var))
            }
            _ => false,
        };
        if hit {
            cc.borrow_mut().car = Value::string(new_entry);
            return Ok(env);
        }
        cur = cc.borrow().cdr.clone();
    }
    Ok(Value::cons(Value::string(new_entry), env))
}

fn f_user_login_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_login_name:
    //   if (NILP (uid)) return Vuser_login_name;
    //   CONS_TO_INTEGER (uid, uid_t, id); pw = getpwuid (id);
    if arg(&a, 0).is_nil() {
        let uln = i.q("user-login-name");
        return Ok(i.symbol_value(uln));
    }
    let uid = match cons_to_uid(&a[0]) {
        Some(u) => u,
        None => {
            return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
        }
    };
    Ok(match pw_login_name(uid) {
        Some(n) => Value::string(n),
        None => Value::Nil,
    })
}

/// GNU `CONS_TO_INTEGER' into `uid_t': an integer, an integral
/// float, or a bignum-shaped cons `(HIGH . LOW16)' / `(HIGH MID24
/// . LOW16)'.  Plain integers/floats are range-checked against
/// uid_t; the cons form folds parts then truncates into uid_t.
fn cons_to_uid(v: &Value) -> Option<u32> {
    let n: i128 = match v {
        Value::Int(n) => *n,
        Value::Float(f) => {
            let d = **f;
            if d.fract() != 0.0 || !d.is_finite() {
                return None;
            }
            d as i128
        }
        Value::Cons(c) => {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let hi = match car {
                Value::Int(n) if n >= 0 => n,
                _ => return None,
            };
            let n = match cdr {
                Value::Int(lo) if (0..=0xFFFF).contains(&lo) => (hi << 16) | lo,
                Value::Cons(r) => {
                    // Bignum literal: proper 2-list (HI MID LOW).
                    let (mid, midrest) = {
                        let rb = r.borrow();
                        (rb.car.clone(), rb.cdr.clone())
                    };
                    let low = match &midrest {
                        Value::Cons(r2) => {
                            let rb2 = r2.borrow();
                            match (&rb2.car, &rb2.cdr) {
                                (l @ Value::Int(_), Value::Nil) => l.clone(),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    match (mid, low) {
                        (Value::Int(m), Value::Int(l))
                            if (0..=0xFF_FFFF).contains(&m) && (0..=0xFFFF).contains(&l) =>
                        {
                            (hi << 40) | (m << 16) | l
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            };
            return Some(n as u32);
        }
        _ => return None,
    };
    if !(0..=u32::MAX as i128).contains(&n) {
        return None;
    }
    Some(n as u32)
}

/// `getpwuid (uid)' -> `pw_name'.
fn pw_login_name(uid: u32) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() {
            return Some(
                std::ffi::CStr::from_ptr((*pw).pw_name)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    let _ = uid;
    None
}

/// The GECOS full-name field (before the first comma) of a uid.
fn gecos_full_name(uid: u32) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() {
            let gecos = std::ffi::CStr::from_ptr((*pw).pw_gecos)
                .to_string_lossy()
                .to_string();
            let name = gecos.split(',').next().unwrap_or("").to_string();
            if !name.is_empty() {
                return Some(name);
            }
            let login = std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .to_string();
            return if login.is_empty() { None } else { Some(login) };
        }
    }
    let _ = uid;
    None
}
fn f_user_real_login_name(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_real_login_name:
    // `getpwuid (getuid ())' — ignores USER/LOGNAME.
    #[cfg(unix)]
    {
        let uid = unsafe { libc::getuid() };
        return Ok(match pw_login_name(uid) {
            Some(n) => Value::string(n),
            None => Value::Nil,
        });
    }
    #[allow(unreachable_code)]
    Ok(Value::Nil)
}

fn f_user_real_uid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_real_uid: getuid ().
    #[cfg(unix)]
    {
        return Ok(Value::Int(unsafe { libc::getuid() } as i128));
    }
    #[allow(unreachable_code)]
    Ok(Value::Int(0))
}

/// GECOS full name for a login *name* (getpwnam path).
fn gecos_full_name_by_login(name: &str) -> Option<String> {
    #[cfg(unix)]
    unsafe {
        let c = std::ffi::CString::new(name).ok()?;
        let pw = libc::getpwnam(c.as_ptr());
        if !pw.is_null() {
            let gecos = std::ffi::CStr::from_ptr((*pw).pw_gecos)
                .to_string_lossy()
                .to_string();
            let name2 = gecos.split(',').next().unwrap_or("").to_string();
            if !name2.is_empty() {
                return Some(name2);
            }
            let login = std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
            return Some(login);
        }
    }
    let _ = name;
    None
}

/// Boot-time `user-login-name' (GNU init_editfns): LOGNAME, then
/// USER, then getpwuid(geteuid()).
pub fn boot_user_login_name() -> String {
    if let Ok(n) = std::env::var("LOGNAME") {
        if !n.is_empty() {
            return n;
        }
    }
    if let Ok(n) = std::env::var("USER") {
        if !n.is_empty() {
            return n;
        }
    }
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(libc::geteuid());
        if !pw.is_null() {
            return std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
        }
    }
    "unknown".to_string()
}

/// Boot-time `user-real-login-name': getpwuid(getuid()).
pub fn boot_user_real_login_name() -> String {
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() {
            return std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned();
        }
    }
    "unknown".to_string()
}

/// Boot-time `user-full-name' (GNU init_editfns): NAME env, else
/// Fuser_full_name(login-name-or-euid), else "unknown".
pub fn boot_user_full_name() -> String {
    if let Ok(n) = std::env::var("NAME") {
        return n;
    }
    // `tem = Fuser_login_name (euid)' when the claimed env name
    // differs from the real login name, else the claimed name.
    let login = boot_user_login_name();
    let real = boot_user_real_login_name();
    let full = if login == real {
        gecos_full_name_by_login(&login)
    } else {
        #[cfg(unix)]
        {
            gecos_full_name(unsafe { libc::geteuid() })
        }
        #[cfg(not(unix))]
        {
            None
        }
    };
    full.unwrap_or_else(|| "unknown".to_string())
}

/// Effective uid for the boot `user-uid' binding.
pub fn boot_geteuid() -> i128 {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() as i128 }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Real uid for the boot `user-real-uid' binding.
pub fn boot_getuid() -> i128 {
    #[cfg(unix)]
    {
        unsafe { libc::getuid() as i128 }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// `user-full-name' — GNU Fuser_full_name: nil → Vuser_full_name;
/// NUMBERP → getpwuid(uid)'s GECOS; STRINGP → getpwnam's GECOS;
/// else "Invalid UID specification".
fn f_user_full_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let arg0 = arg(&a, 0);
    if arg0.is_nil() {
        let ufn = i.q("user-full-name");
        return Ok(i.symbol_value(ufn));
    }
    match &arg0 {
        // GNU's NUMBERP test: a cons UID fails it, hitting
        // "Invalid UID specification".
        Value::Int(_) | Value::Float(_) => match cons_to_uid(&arg0) {
            Some(uid) => match gecos_full_name(uid) {
                Some(n) => return Ok(Value::string(n)),
                None => return Ok(Value::Nil),
            },
            None => {
                return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
            }
        },
        Value::Str(s) => {
            let name = s.borrow().clone();
            match gecos_full_name_by_login(&name) {
                Some(n) => return Ok(Value::string(n)),
                None => return Ok(Value::Nil),
            }
        }
        _ => return Err(i.error("Invalid UID specification")),
    }
}

fn f_user_uid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU Fuser_uid: geteuid ().
    #[cfg(unix)]
    {
        return Ok(Value::Int(unsafe { libc::geteuid() } as i128));
    }
    #[allow(unreachable_code)]
    Ok(Value::Int(0))
}

fn f_system_groups(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU enumerates the whole group database (getgrent), not just
    // the groups the current user belongs to.
    #[cfg(unix)]
    unsafe {
        let mut out = Vec::new();
        libc::setgrent();
        loop {
            let grp = libc::getgrent();
            if grp.is_null() {
                break;
            }
            out.push(Value::string(
                std::ffi::CStr::from_ptr((*grp).gr_name)
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
        libc::endgrent();
        return Ok(Value::list(out));
    }
    #[allow(unreachable_code)]
    Ok(Value::Nil)
}

fn f_group_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU editfns.c Fgroup_name: non-number
    // non-cons → bare (error "Invalid GID specification"); CONS_TO_INTEGER
    // range/shape failures → the "Not an in-range integer, integral
    // float, or cons of integers" error.
    match &a[0] {
        Value::Int(_) | Value::Float(_) | Value::Cons(_) => {}
        _ => return Err(i.error("Invalid GID specification")),
    }
    let gid = match cons_to_uid(&a[0]) {
        Some(g) => g,
        None => {
            return Err(i.error("Not an in-range integer, integral float, or cons of integers"));
        }
    };
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

fn f_get_truename_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil for any arg (truename cache unused in batch).
    let _ = (i, &a[0]);
    Ok(Value::Nil)
}

fn f_unencodable_char_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Funencodable_char_position (coding.c):
    // check_coding_system runs FIRST (non-symbol → `symbolp', unknown →
    // `coding-system-error'); then STRING non-nil switches to string
    // mode (substring-style bounds), else buffer mode
    // (`integer-or-marker-p'). Nil coding selects undecided.
    if !a[2].is_nil() {
        check_coding(i, &a[2])?;
    }
    let coding = if a[2].is_nil() {
        Value::Sym(i.q("undecided"))
    } else {
        a[2].clone()
    };
    // Raw-text coding accepts every character, before validating bounds.
    if eol_split(&coding_canonical(i, &coding)).0 == "raw-text" {
        return Ok(Value::Nil);
    }
    // A char encodes iff some charset of the coding system encodes it;
    // an empty charset list (no-conversion) accepts everything.
    let cslist = super::charset::coding_system_charset_names(i, &coding);
    let csnames: Vec<String> = cslist
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Sym(s) => Some(i.symbol_name(*s)),
            _ => None,
        })
        .collect();
    let unencodable = |i: &Interp, ch: char| {
        !csnames.is_empty()
            && !csnames
                .iter()
                .any(|n| super::charset::charset_encode(i, n, ch as i64).is_some())
    };
    let mut hits: Vec<i128> = Vec::new();
    match a.get(4) {
        Some(Value::Str(s)) => {
            let sv: Vec<char> = s.borrow().chars().collect();
            let (pos, end) = super::strfn::validate_subarray(i, &a[4], &a[0], &a[1], sv.len())?;
            if i.is_unibyte_str(s) {
                return Ok(Value::Nil);
            }
            for p in pos..end {
                if unencodable(i, sv[p as usize]) {
                    hits.push(p as i128);
                }
            }
        }
        Some(Value::Nil) | None => {
            let start = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
            let end = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
            let Some(b) = i.current_buffer_ref() else {
                return Ok(Value::Nil);
            };
            let bb = b.borrow();
            let (lo, hi) = (bb.begv as i128 + 1, bb.zv.min(bb.text_len()) as i128 + 1);
            if start < lo || start > end || end > hi {
                let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
                drop(bb);
                return Err(
                    i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![bv, a[0].clone(), a[1].clone()])
                );
            }
            for p in start..end {
                if unencodable(i, bb.text.char_at((p - 1) as usize)) {
                    hits.push(p);
                }
            }
        }
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    }
    // COUNT is checked lazily: nil → the first position (or nil);
    // non-nil forces a list, natnum → at most N entries (0 → all),
    // anything else signals `wholenump' — but only when a hit exists.
    if hits.is_empty() {
        return Ok(Value::Nil);
    }
    match &arg(&a, 3) {
        Value::Nil => Ok(Value::Int(hits[0])),
        Value::Int(n) if *n >= 0 => {
            let take = if *n == 0 { usize::MAX } else { *n as usize };
            Ok(Value::list(
                hits.iter().take(take).map(|p| Value::Int(*p)).collect(),
            ))
        }
        other => Err(i.wrong_type_mut("wholenump", other)),
    }
}

fn f_compose_region_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (start, end) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let (begv, zv, bufv) = i
        .current_buffer_ref()
        .map(|b| {
            let (bb, bv) = (b.borrow(), Value::Buffer(b.clone()));
            (bb.begv as i128 + 1, bb.zv as i128 + 1, bv)
        })
        .unwrap_or((1, 1, Value::Nil));
    if start < begv || end > zv || start > end {
        // GNU's args-out-of-range data is (BUFFER START END).
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![bufv, a[0].clone(), a[1].clone()],
        ));
    }
    // GNU Fcompose_region_internal type-checks COMPONENTS
    // (fixnum/cons/string pass, other non-nil → `vectorp'), then
    // compose_text installs `composition' = ((WIDTH . COMPONENTS)
    // . MODIFICATION-FUNC) over the range.
    if let Some(v) = a.get(2) {
        if v.truthy()
            && !matches!(
                v,
                Value::Int(_) | Value::Cons(_) | Value::Str(_) | Value::Vec(_)
            )
        {
            return Err(i.wrong_type_mut("vectorp", v));
        }
    }
    let width = Value::Int(end - start);
    let head = match a.get(2) {
        Some(v) if v.truthy() => Value::cons(width, v.clone()),
        _ => width,
    };
    let prop_val = match a.get(3) {
        Some(v) if v.truthy() => Value::cons(head, v.clone()),
        _ => Value::list(vec![head]),
    };
    let comp_sym = Value::Sym(i.q("composition"));
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[0].clone(), a[1].clone(), comp_sym, prop_val, Value::Nil].into(),
    )?;
    Ok(Value::Nil)
}

fn f_compose_string_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let (start, end) = (want_int(i, &a[1])?, want_int(i, &a[2])?);
    if start < 0 || end > s.chars().count() as i128 || start > end {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), a[1].clone(), a[2].clone()],
        ));
    }
    // GNU Fcompose_string_internal installs the `composition' property
    // over [START,END): value ((WIDTH . COMPONENTS) . MODIFICATION-STRING)
    // where WIDTH = END-START and COMPONENTS is the raw COMPONENTS arg
    // (absent → just (WIDTH)); MODIFICATION-STRING extends the head.
    let width = Value::Int(end - start);
    // WIDTH . COMPONENTS is a single cons even when COMPONENTS is nil.
    let head = Value::cons(width, a.get(3).cloned().unwrap_or(Value::Nil));
    let prop_val = match a.get(4) {
        // GNU: modification-string cdrs the (WIDTH . COMPONENTS) entry —
        // no list wrap; absent → the entry is a one-element list.
        Some(v) if v.truthy() => Value::cons(head, v.clone()),
        _ => Value::list(vec![head]),
    };
    let comp_sym = Value::Sym(i.q("composition"));
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[1].clone(), a[2].clone(), comp_sym, prop_val, a[0].clone()].into(),
    )?;
    Ok(a[0].clone())
}

fn f_set_buffer_redisplay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xdisp.c: marks the current buffer for redisplay and returns
    // nil — the (SYMBOL NEWVAL OPERATION WHERE) args are unused, never
    // type-checked.
    let _ = (i, a);
    Ok(Value::Nil)
}

fn f_register_ccl_program(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    let elems = match &a[1] {
        Value::Vec(v) => v.borrow().clone(),
        other => return Err(i.wrong_type_mut("vectorp", other)),
    };
    if elems.len() < 3 || !elems.iter().all(|e| matches!(e, Value::Int(_))) {
        // GNU ccl.c Fregister_ccl_program: "Error in CCL program"
        // (the execute path's "Invalid CCL program" is different).
        return Err(i.signal_data(sym::ERROR, vec![Value::string("Error in CCL program")]));
    }
    let prop = i.q("ccl-program");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.ccl_program_count;
    i.ccl_program_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

fn f_ccl_program_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil for non-symbols
    // too — only a symbol with a `ccl-program' property counts.
    let Some(name) = i.sym_id(&a[0]) else {
        return Ok(Value::Nil);
    };
    let prop = i.q("ccl-program");
    Ok(if i.get_prop(name, prop).is_nil() {
        Value::Nil
    } else {
        Value::t()
    })
}

fn ccl_prog_of(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    // GNU: PROGRAM is a symbol whose `ccl-program' prop is a vector,
    // or a vector itself; anything else signals the plain error.
    let prog = match v {
        Value::Sym(s) => {
            let prop = i.q("ccl-program");
            i.get_prop(*s, prop)
        }
        other => other.clone(),
    };
    if matches!(prog, Value::Vec(_)) {
        Ok(())
    } else {
        Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid CCL program")]))
    }
}

fn f_ccl_execute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    ccl_prog_of(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_ccl_execute_on_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates the CCL program first (Fccl_execute_on_string ->
    // ccl_driver before CHECK_STRING lands via the status register);
    // invalid program reports the plain "Invalid CCL program" error.
    ccl_prog_of(i, &a[0])?;
    let s = want_string(i, &a[1])?;
    // STATUS vector: 10 slots of int 0.
    if let Value::Vec(v) = &a[2] {
        for x in v.borrow_mut().iter_mut().take(10) {
            *x = Value::Int(0);
        }
    }
    Ok(Value::string(s))
}

fn f_register_code_conversion_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    if !matches!(a[1], Value::Vec(_)) {
        return Err(i.wrong_type_mut("vectorp", &a[1]));
    }
    let prop = i.q("code-conversion-map");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.code_conv_map_count;
    i.code_conv_map_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

fn f_zlib_available_p(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

fn f_zlib_decompress_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use std::io::Read;
    let (start, end) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let buf = match i.current_buffer_ref() {
        Some(b) => b,
        None => return Err(i.signal_data(sym::ERROR, vec![Value::string("No buffer")])),
    };
    let bytes = {
        let bb = buf.borrow();
        let (begv, zv) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if start < begv || end > zv || start > end {
            return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone(), a[1].clone()]));
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
            let lo = (start - 1).max(0) as usize;
            let hi = (end - 1).max(lo as i128) as usize;
            crate::buffer::primitives::chg_delete(i, lo, hi)?;
            crate::buffer::primitives::chg_insert(i, lo, &text)?;
            Ok(Value::t())
        }
        None => Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Malformed or misplaced compressed data")],
        )),
    }
}

fn f_find_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
        if super::equal_values(i, &v, &a[1])? {
            if let Some(b) = i.buffers.get(id) {
                return Ok(Value::Buffer(b));
            }
        }
    }
    Ok(Value::Nil)
}

fn f_insert_byte(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let byte = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    // GNU insert_byte: CHECK_FIXNUM_RANGE(byte, 0, 255) — range checked
    // BEFORE COUNT is coerced; datum carries BYTE MIN MAX.
    if !(0..=255).contains(&byte) {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), Value::Int(0), Value::Int(255)],
        ));
    }
    let count = crate::lisp::builtins::want_fixnum(i, &a[1])?;
    // GNU insert_byte (insdel.c): in a multibyte buffer bytes ≥0x80 are
    // inserted as the raw-byte escape char BYTE8_TO_CHAR(byte) =
    // byte - 0x80 + 0x3FFF80 = 0x3FFF00 + byte (eight-bit-graphic
    // charset; `lisp_char' returns the proxy char); bytes <0x80 are
    // literal.  Unibyte buffers store the byte as-is (char byte).
    let ems = i.q("enable-multibyte-characters");
    let mb = i.symbol_value(ems).truthy();
    let code = if mb && byte >= 0x80 {
        byte as u32 + 0x3FFF00
    } else {
        byte as u32
    };
    let ch = crate::lisp::value::lisp_char(code).unwrap_or('\u{fffd}');
    let s: String = std::iter::repeat_n(ch, count.max(0) as usize).collect();
    if a.get(2).is_some_and(Value::truthy) {
        crate::buffer::primitives::chg_insert_and_inherit_pt(i, &s)?;
    } else {
        crate::buffer::primitives::chg_insert_pt(i, &s, false)?;
    }
    Ok(Value::Nil)
}

fn f_set_binary_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    if !matches!(name.as_str(), "stdin" | "stdout" | "stderr") {
        // GNU fileio.c: xsignal2 'error ("unsupported stream" STREAM) —
        // the datum carries the stream object, not its name.
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("unsupported stream"), a[0].clone()],
        ));
    }
    // POSIX: always binary; value is the previous mode (non-nil).
    Ok(Value::t())
}

fn f_set_output_flow_control(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (FLOW &optional TERMINAL). No terminal objects in this port;
    // GNU signals terminal-live-p when TERMINAL is given and not a
    // live terminal.
    if let Some(term) = a.get(1) {
        let pred = i.q("terminal-live-p");
        return Err(i.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred), term.clone()],
        ));
    }
    Ok(Value::Nil)
}

fn f_tab_bar_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU tab-bar.el:
    // `(frame-parameter frame 'tab-bar-height)' — decode_any_frame:
    // wta `framep' on non-frames; a tty frame reports 0.
    let _ = crate::editor::frame_any(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

fn f_buffer_text_pixel_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xdisp.c Fbuffer_text_pixel_size (BUFFER-OR-NAME WINDOW
    // X-LIMIT Y-LIMIT): decode_live_window(WINDOW) FIRST — wta
    // `window-live-p' on non-windows — then Fget_buffer on
    // BUFFER-OR-NAME (wta `stringp' on non-buffer non-string; a missing
    // name aborts in GNU — we signal cleanly).  X-LIMIT/Y-LIMIT are
    // never type-checked.
    let w = crate::editor::win_live(i, &arg(&a, 1))?;
    let buf = match &arg(&a, 0) {
        Value::Nil => i.current_buffer_ref(),
        Value::Buffer(b) => Some(b.clone()),
        Value::Str(name) => {
            let n = name.borrow().clone();
            match i.buffers.by_name(&n).and_then(|id| i.buffers.get(id)) {
                Some(b) => Some(b),
                None => return Err(i.error(format!("No such buffer: ‘{}’", n))),
            }
        }
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // GNU window_text_pixel_size on the batch tty (a column and a line
    // are one pixel each): an X-LIMIT fixnum in [0, INT_MAX] wraps
    // screen lines at that column, any other non-nil X-LIMIT means no
    // limit, and nil wraps at the window's text width (its last column
    // holds the continuation glyph).  Y-LIMIT nil is unlimited, a
    // fixnum in [0, INT_MAX] caps the height, and any other value
    // leaves GNU's max_y at 0.  With a non-nil X-LIMIT the iterator
    // also moves across the screen line starting at max_y, so that
    // line still counts toward the width.
    const INT_MAX: i128 = i32::MAX as i128;
    let wrap = match arg(&a, 2) {
        Value::Int(n) if (0..=INT_MAX).contains(&n) => Some(n as usize),
        Value::Nil => Some(w.borrow().width.saturating_sub(1).max(1)),
        _ => None,
    };
    let max_y = match arg(&a, 3) {
        Value::Nil => usize::MAX,
        Value::Int(n) if (0..=INT_MAX).contains(&n) => n as usize,
        _ => 0,
    };
    let measure_y = max_y.saturating_add(usize::from(arg(&a, 2).truthy()));
    let Some(b) = buf else {
        return Ok(Value::cons(Value::Int(0), Value::Int(0)));
    };
    let (text, tab_width, ctl_arrow) = {
        let bb = b.borrow();
        let local = |name: &str| {
            let id = i.intern_soft(name)?;
            match bb.locals.get(&id) {
                Some(v) if !matches!(v, Value::Sym(s) if *s == sym::UNBOUND) => Some(v.clone()),
                _ => Some(i.obarray.symbol(id).value.clone()),
            }
        };
        // GNU SANE_TAB_WIDTH: a fixnum in 1..=1000, else 8.
        let tab_width = match local("tab-width") {
            Some(Value::Int(n)) if (1..=1000).contains(&n) => n as usize,
            _ => 8,
        };
        let ctl_arrow = local("ctl-arrow").map_or(true, |v| v.truthy());
        (
            bb.text.substring(bb.begv, bb.text_len()),
            tab_width,
            ctl_arrow,
        )
    };
    // Zero-width chars with a `glyphless-char-display' method draw as
    // that method on the tty: `thin-space' (the default for format
    // controls and variation selectors) takes one column.  A cons
    // entry is (GRAPHICAL . TEXT).
    let glyphless = i
        .intern_soft("glyphless-char-display")
        .map(|id| i.obarray.symbol(id).value.clone())
        .filter(|t| is_char_table(i, t));
    let thin_space = i.q_soft("thin-space");
    let glyph_width = |c: char| -> usize {
        let u = c as u32;
        if crate::lisp::value::eight_bit_byte(c).is_some() || (0x80..=0x9F).contains(&u) {
            return 4;
        }
        if u < 0x20 || u == 0x7F {
            return if ctl_arrow { 2 } else { 4 };
        }
        let cw = crate::buffer::primitives::char_width(c).max(0) as usize;
        if cw == 0 {
            if let Some(t) = &glyphless {
                let method = match char_table_ref(i, t, u as usize) {
                    Value::Cons(m) => m.borrow().cdr.clone(),
                    m => m,
                };
                if matches!(method, Value::Sym(s) if Some(s) == thin_space) {
                    return 1;
                }
            }
        }
        cw
    };
    let (mut width, mut lines) = (0usize, 0usize);
    let segs: Vec<&str> = text.split('\n').collect();
    // The empty line after a final newline takes no screen line.
    let n_logical = segs.len() - usize::from(segs.last().map_or(true, |s| s.is_empty()));
    'lines: for seg in &segs[..n_logical] {
        let (mut x, mut line_x) = (0usize, 0usize);
        for c in seg.chars() {
            let cw = |x: usize| {
                if c == '\t' {
                    tab_width - x % tab_width
                } else {
                    glyph_width(c)
                }
            };
            let mut adv = cw(x);
            if let Some(wr) = wrap {
                if x > 0 && x + adv > wr {
                    // Continued screen line: it spans the full wrap width.
                    if lines >= measure_y {
                        break 'lines;
                    }
                    lines += 1;
                    width = width.max(wr);
                    x = 0;
                    line_x = 0;
                    adv = cw(0);
                }
                if x == 0 && c == '\t' && adv > wr && wr > 0 {
                    // A tab wider than the limit at a line start runs
                    // past the edge; the next glyph lands at its
                    // column modulo the limit, on the same screen line.
                    line_x = line_x.max(adv);
                    x = adv % wr;
                    continue;
                }
            }
            x += adv;
            line_x = line_x.max(x);
        }
        if lines >= measure_y {
            break;
        }
        lines += 1;
        width = width.max(wrap.map_or(line_x, |wr| line_x.min(wr)));
    }
    Ok(Value::cons(
        Value::Int(width as i128),
        Value::Int(lines.min(max_y) as i128),
    ))
}

fn f_format_mode_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fformat_mode_line:
    // WINDOW (arg3) is CHECK_WINDOW (windowp) when non-nil; batch GNU
    // yields "" for everything else.
    match a.get(2) {
        Some(v) if v.truthy() && !matches!(v, Value::Window(_)) => {
            return Err(i.wrong_type_mut("windowp", v));
        }
        _ => {}
    }
    Ok(Value::string(""))
}

// ---------- category tables ----------
// Record layout: [char-table, 'category-table, Vec contents,
// Vec docstrings(95), category-set hash].

/// Build a category table value. `standard` populates GNU's ASCII
/// membership and label docstrings.
pub(crate) fn make_category_table_value(i: &mut Interp, standard: bool) -> Value {
    // GNU extra slot 0: docstring vector of 95 (indexed by cat - 32);
    // extra slot 1 starts nil and becomes an `equal' hash-table
    // hash-consing category sets on the first `modify-category-entry'.
    let docs = crate::lisp::Value::vector_ref(vec![Value::Nil; 95]);
    let tag = Value::Sym(i.q("category-table"));
    let t = make_ct(
        i,
        tag,
        Value::Nil,
        vec![Value::Vec(docs.clone()), Value::Nil],
    );
    // GNU init_category_once leaves raw entries nil, with a distinct
    // empty category set as their inherited default.
    let empty = make_bool_vector(i, vec![false; 128]);
    i.set_char_table_defalt(&t, empty);
    if standard {
        // GNU's standard-category-table ASCII defaults.
        const DOCS: &[(usize, &str)] = &[
            (
                32,
                "space for indent\nThis character counts as a space for indentation purposes.",
            ),
            (49, "base vowel\nBase (independent) vowel"),
            (
                50,
                "upper diacritic\nUpper diacritical mark (including upper vowel)",
            ),
            (
                51,
                "lower diacritic\nLower diacritical mark (including lower vowel)",
            ),
            (52, "combining tone\nCombining tone mark"),
            (53, "symbol"),
            (54, "digit"),
            (55, "vowel diacritic\nVowel-modifying diacritical mark"),
            (56, "vowel-signs"),
            (57, "semivowel lower"),
            (
                60,
                "Not at eol\nA character which can't be placed at end of line.",
            ),
            (
                62,
                "Not at bol\nA character which can't be placed at beginning of line.",
            ),
            (
                65,
                "2-byte alnum\nAlphanumeric characters of 2-byte character sets",
            ),
            (
                67,
                "2-byte han\nChinese (Han) characters of 2-byte character sets",
            ),
            (
                71,
                "2-byte Greek\nGreek characters of 2-byte character sets",
            ),
            (
                72,
                "2-byte Hiragana\nJapanese Hiragana characters of 2-byte character sets",
            ),
            (73, "Indian Glyphs"),
            (
                75,
                "2-byte Katakana\nJapanese Katakana characters of 2-byte character sets",
            ),
            (
                76,
                "Strong L2R\nCharacters with \"strong\" left-to-right directionality, i.e.\nwith L, LRE, or LRO Unicode bidi character type.",
            ),
            (
                78,
                "2-byte Korean\nKorean Hangul characters of 2-byte character sets",
            ),
            (
                82,
                "Strong R2L\nCharacters with \"strong\" right-to-left directionality, i.e.\nwith R, AL, RLE, or RLO Unicode bidi character type.",
            ),
            (
                89,
                "2-byte Cyrillic\nCyrillic characters of 2-byte character sets",
            ),
            (
                94,
                "Combining\nCombining diacritic or mark (Unicode General Category M)",
            ),
            (
                97,
                "ASCII\nASCII graphic characters 32-126 (ISO646 IRV:1983[4/0])",
            ),
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
            (
                124,
                "line breakable\nWhile filling, we can break a line at this character.",
            ),
        ];
        {
            let mut d = docs.borrow_mut();
            for &(label, doc) in DOCS {
                d[label - 32] = Value::string(doc);
            }
        }
        for ch in 32u32..=126 {
            let mut bits = vec![false; 128];
            for b in [46usize, 97, 108] {
                bits[b] = true;
            }
            if ch != 32 && ch != 92 && ch != 126 {
                bits[114] = true;
            }
            let c = char::from_u32(ch).unwrap();
            if c.is_ascii_alphabetic() {
                bits[76] = true;
            }
            if c.is_ascii_digit() {
                bits[54] = true;
            }
            let bv = make_bool_vector(i, bits);
            ct_set(i, &t, ch, bv);
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
        Value::Nil => Ok(
            match i
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
            },
        ),
        Value::Record(r) if is_category_table(i, v) => Ok(r.clone()),
        other => Err(i.wrong_type_mut("category-table-p", other)),
    }
}

/// The docstring vec of a category table (record slot 3).
fn cat_docs(v: &Rc<RefCell<Vec<Value>>>) -> Rc<RefCell<Vec<Value>>> {
    let rr = v.borrow();
    match rr.get(3) {
        Some(Value::Vec(d)) => d.clone(),
        _ => crate::lisp::Value::vector_ref(vec![Value::Nil; 95]),
    }
}

fn f_make_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let table = make_category_table_value(i, false);
    let contents = char_table_vec(&table).unwrap();
    // Unlike the standard table, GNU fills each top-level block with
    // its own mutable set.  Its ASCII cache remains nil initially.
    for slot in 1..=64 {
        contents.borrow_mut()[slot] = make_bool_vector(i, vec![false; 128]);
    }
    Ok(table)
}

fn f_category_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_category_table(i, &a[0])))
}

fn f_standard_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.standard_category_table())
}

fn f_category_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let v = i
        .current_buffer_ref()
        .and_then(|b| b.borrow().category_table.clone())
        .unwrap_or_else(|| i.standard_category_table());
    Ok(v)
}

fn f_set_category_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_category_table(i, &a[0]) {
        return Err(i.wrong_type_mut("category-table-p", &a[0]));
    }
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().category_table = Some(a[0].clone());
    }
    Ok(a[0].clone())
}

fn f_copy_category_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let source = match a.first() {
        None | Some(Value::Nil) => i.standard_category_table(),
        Some(value) => Value::Record(want_category_table(i, value)?),
    };
    let table = ct_copy(i, &source);
    // GNU copies mutable category sets and the documentation vector,
    // but leaves the category-set interning hash shared.
    let default = i.char_table_defalt(&table);
    if !default.is_nil() {
        let default = super::seq::f_copy_sequence(i, vec![default].into())?;
        i.set_char_table_defalt(&table, default);
    }
    let docs = super::seq::f_copy_sequence(i, vec![ct_extra(&table, 0)].into())?;
    ct_set_extra(&table, 0, docs);
    for (from, to, value) in ct_effective_runs(i, &table, true) {
        if !value.is_nil() {
            let copied = super::seq::f_copy_sequence(i, vec![value].into())?;
            ct_set_range(i, &table, from, to, copied);
        }
    }
    Ok(table)
}

fn f_define_category(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `CHECK_CATEGORY': CATEGORY must be a printable-ASCII fixnum
    // (32..=126); anything else is `(categoryp CATEGORY)'.
    let cat = match &a[0] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[0])),
    };
    let doc = want_string(i, &a[1])?;
    let t = want_category_table(i, &arg(&a, 2))?;
    let docs = cat_docs(&t);
    if !docs.borrow()[cat as usize - 32].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Category ‘{}’ is already defined",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    docs.borrow_mut()[cat as usize - 32] = Value::string(doc);
    Ok(Value::Nil)
}

fn f_category_docstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let cat = match &a[0] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[0])),
    };
    let t = want_category_table(i, &arg(&a, 1))?;
    Ok(cat_docs(&t).borrow()[cat as usize - 32].clone())
}

fn f_get_unused_category(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_category_table(i, &arg(&a, 0))?;
    let docs = cat_docs(&t);
    let dd = docs.borrow();
    for c in 32usize..=126 {
        if dd[c - 32].is_nil() {
            return Ok(Value::Int(c as i128));
        }
    }
    Ok(Value::Nil)
}
/// Extra slot 1 of a category table: GNU's `equal' hash-table
/// hash-consing category sets (created lazily by
/// `modify-category-entry'), so equal sets are `eq'.
fn cat_set_hash(t: &Rc<RefCell<Vec<Value>>>) -> Rc<RefCell<crate::lisp::value::LispHash>> {
    {
        let b = t.borrow();
        if let Some(Value::Hash(h)) = b.get(4) {
            return h.clone();
        }
    }
    let h = crate::lisp::memory::allocate(
        crate::lisp::value::LispHash::new(crate::lisp::value::HashTest::Equal),
        crate::lisp::Value::Hash,
    );
    if t.borrow().len() > 4 {
        t.borrow_mut()[4] = Value::Hash(h.clone());
    }
    h
}

fn f_modify_category_entry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // CHAR may be a char or a (MIN . MAX) cons.  GNU checks `consp'
    // only after the FIXNATP fast path: any other type is
    // `(consp CHAR)', while ints and cons elements out of character
    // range are `(characterp ...)'.
    let _tt = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    let (lo, hi) = match &a[0] {
        Value::Int(c) if (0..=CT_MAX_CHAR as i128).contains(c) => (*c, *c),
        Value::Int(_) => return Err(i.wrong_type_mut("characterp", &a[0])),
        Value::Cons(c) => {
            let (car, cdr) = {
                let r = c.borrow();
                (r.car.clone(), r.cdr.clone())
            };
            let l = match &car {
                Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
                _ => return Err(i.wrong_type_mut("characterp", &car)),
            };
            let h = match &cdr {
                Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
                _ => return Err(i.wrong_type_mut("characterp", &cdr)),
            };
            (l, h)
        }
        other => return Err(i.wrong_type_mut("consp", other)),
    };
    let cat = match &a[1] {
        Value::Int(c) if (32..=126).contains(c) => *c,
        _ => return Err(i.wrong_type_mut("categoryp", &a[1])),
    };
    let _tc = _tt.is_some().then(std::time::Instant::now);
    let reset = !arg(&a, 3).is_nil();
    let t = want_category_table(i, &arg(&a, 2))?;
    if cat_docs(&t).borrow()[cat as usize - 32].is_nil() {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Undefined category: {}",
                char::from_u32(cat as u32).unwrap_or('?')
            ))],
        ));
    }
    if let Some(tc) = _tc {
        eprintln!("[mce-prolog] {}us", tc.elapsed().as_micros());
    }
    if lo > hi {
        // GNU: a reversed range is a silent no-op.
        return Ok(Value::Nil);
    }
    // Fast path: the common `(modify-category-entry RANGE CAT)' form
    // from characters.el / map-charset-chars — parentless table, plain
    // scalar defalt.  Fuse the old-value walk and the write into one
    // descent: each covered slot's new set derives from its old
    // category-set via a per-object cache (`mce_new_set'), so no
    // intermediate run list or per-run trie re-descent is needed.
    if arg(&a, 2).is_nil() {
        let defalt = i.char_table_defalt(&Value::Record(t.clone()));
        let no_parent = i.char_table_parent(&Value::Record(t.clone())).is_nil();
        if no_parent && !is_sub_ct(i, &defalt) {
            let _tf = _tt.is_some().then(std::time::Instant::now);
            let mut mce_cache: rustc_hash::FxHashMap<usize, Value> = Default::default();
            mce_fused(
                i,
                &t,
                lo as u32,
                hi as u32,
                cat as usize,
                reset,
                &defalt,
                &mut mce_cache,
            );
            if let Some(tf) = _tf {
                eprintln!("[mce-fused] {}us", tf.elapsed().as_micros());
            }
            return Ok(Value::Nil);
        }
    }
    let table = Value::Record(t.clone());
    // Group the range by runs of equal old category-set: one
    // bool-vector per run shared over the whole run.  Run
    // boundaries come from `ct_runs_in' — leaf blocks of the
    // char-table trie — instead of a per-character `char_table_ref'
    // walk (multi-GB worth of lookups on CJK-wide categories).
    // Adjacent runs carrying the SAME `old' set yield the same
    // `set' — reuse it without re-hashing, and merge the writes
    // into one `ct_set_range' covering the whole span.
    let mut pending: Option<(u32, u32, Value)> = None; // (start,end,set)
    // Cross-run cache keyed on the old object's identity (Rc pointer;
    // nil/other immediates share one slot): identical `old' values
    // recur non-adjacently too, so a single last-entry cache misses
    // on alternating patterns.
    let mut old_cache: rustc_hash::FxHashMap<usize, Value> = Default::default();
    let _t0 = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    let _runs_v = ct_runs_in(i, &table, lo as u32, hi as u32);
    if let Some(t0) = _t0 {
        eprintln!(
            "[mce] runs_in {} runs {}us",
            _runs_v.len(),
            t0.elapsed().as_micros()
        );
    }
    let _post_t = (i.dbg_flags & 0x400 != 0).then(std::time::Instant::now);
    for (c, e, old) in _runs_v {
        // New set = old bits with bit CAT = !RESET (an unset cell
        // starts from all-zero — GNU stores that empty set, never
        // leaves nil behind), then hash-consed through extra slot 1
        // so equal sets share one bool-vector object.
        let key = match &old {
            Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
            Value::Vec(v) => std::rc::Rc::as_ptr(v) as usize,
            Value::Cons(v) => std::rc::Rc::as_ptr(v) as usize,
            _ => usize::MAX,
        };
        let set = match old_cache.get(&key) {
            Some(s) => s.clone(),
            None => {
                let mut bits = vec![false; 128];
                if is_bool_vector(i, &old) {
                    if let Ok(old_bits) = bool_vec_of(i, &old) {
                        for (k, v) in old_bits.iter().enumerate() {
                            if k < 128 {
                                bits[k] = *v;
                            }
                        }
                    }
                }
                bits[cat as usize] = !reset;
                let newv = make_bool_vector(i, bits);
                let hk = crate::lisp::builtins::hashfn::hash_key_for(
                    i,
                    &newv,
                    crate::lisp::value::HashTest::Equal,
                );
                let h = cat_set_hash(&t);
                let s = {
                    let mut hm = h.borrow_mut();
                    match hm.map.get(&hk) {
                        Some(v) => v.clone(),
                        None => {
                            hm.map.insert(hk.clone(), newv.clone());
                            hm.put_key(hk, newv.clone());
                            newv.clone()
                        }
                    }
                };
                old_cache.insert(key, s.clone());
                s
            }
        };
        match pending {
            Some((pc, pe, ref ps)) if crate::lisp::builtins::eq_values(ps, &set) && pe + 1 == c => {
                pending = Some((pc, e, set));
            }
            Some((pc, pe, ps)) => {
                if pe == pc {
                    ct_set(i, &table, pc, ps);
                } else {
                    ct_set_range(i, &table, pc, pe, ps);
                }
                pending = Some((c, e, set));
            }
            None => pending = Some((c, e, set)),
        }
    }
    if let Some(t0) = _post_t {
        eprintln!("[mce] body {}us", t0.elapsed().as_micros());
    }
    if let Some((pc, pe, ps)) = pending {
        if pe == pc {
            ct_set(i, &table, pc, ps);
        } else {
            ct_set_range(i, &table, pc, pe, ps);
        }
    }
    if let Some(tt) = _tt {
        eprintln!("[mce-total] {}us", tt.elapsed().as_micros());
    }
    Ok(Value::Nil)
}

/// Pre-resolved `modify-category-entry' fast call: the standard
/// category table + CAT + RESET validated once, then every (lo..=hi)
/// range writes through `mce_fused' with a shared old→new cache.
/// `mce_fast_for' returns None for anything needing the full
/// `f_modify_category_entry' semantics (explicit TABLE arg, parented
/// table, invalid CAT) so the caller falls back to `apply'.
pub(crate) struct MceFast {
    t: std::rc::Rc<RefCell<Vec<Value>>>,
    cat: usize,
    defalt: Value,
    cache: rustc_hash::FxHashMap<usize, Value>,
}

pub(crate) fn mce_fast_for(i: &mut Interp, function: &Value, arg: &Value) -> Option<MceFast> {
    let Value::Subr(s) = function else {
        return None;
    };
    if s.name != "modify-category-entry" {
        return None;
    }
    let Value::Int(c) = arg else {
        return None;
    };
    if !(32..=126).contains(c) {
        return None; // let the real call raise `categoryp'
    }
    let t = want_category_table(i, &Value::Nil).ok()?;
    if cat_docs(&t).borrow()[*c as usize - 32].is_nil() {
        return None; // undefined category must still signal
    }
    let tv = Value::Record(t.clone());
    if !i.char_table_parent(&tv).is_nil() {
        return None;
    }
    let defalt = i.char_table_defalt(&tv);
    if is_sub_ct(i, &defalt) {
        return None;
    }
    Some(MceFast {
        t,
        cat: *c as usize,
        defalt,
        cache: Default::default(),
    })
}

impl MceFast {
    /// `(modify-category-entry (LO . HI) CAT)' — RESET always nil here
    /// since `map-charset-chars' passes exactly two arguments.
    pub(crate) fn emit(&mut self, i: &mut Interp, lo: i64, hi: i64) {
        if lo > hi {
            return;
        }
        mce_fused(
            i,
            &self.t,
            lo as u32,
            hi as u32,
            self.cat,
            false,
            &self.defalt,
            &mut self.cache,
        );
    }
}

/// New category set for `cat' derived from the slot's old value OLD:
/// copy bits, set CAT = !RESET, hash-cons through the table's extra
/// slot so equal sets share one bool-vector (same rule as
/// `f_modify_category_entry').  Results are cached by the old
/// object's identity in CACHE — identical sets recur heavily across
/// a call (nil leaves, alternating bool-vectors).
fn mce_new_set(
    i: &mut Interp,
    t: &Rc<RefCell<Vec<Value>>>,
    old: &Value,
    cat: usize,
    reset: bool,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) -> Value {
    let key = match old {
        Value::Record(r) => std::rc::Rc::as_ptr(r) as usize,
        Value::Vec(v) => std::rc::Rc::as_ptr(v) as usize,
        Value::Cons(v) => std::rc::Rc::as_ptr(v) as usize,
        _ => usize::MAX,
    };
    if let Some(s) = cache.get(&key) {
        return s.clone();
    }
    let mut bits = vec![false; 128];
    if is_bool_vector(i, old) {
        if let Ok(old_bits) = bool_vec_of(i, old) {
            for (k, v) in old_bits.iter().enumerate() {
                if k < 128 {
                    bits[k] = *v;
                }
            }
        }
    }
    bits[cat] = !reset;
    let newv = make_bool_vector(i, bits);
    let hk =
        crate::lisp::builtins::hashfn::hash_key_for(i, &newv, crate::lisp::value::HashTest::Equal);
    let h = cat_set_hash(t);
    let s = {
        let mut hm = h.borrow_mut();
        match hm.map.get(&hk) {
            Some(v) => v.clone(),
            None => {
                hm.map.insert(hk.clone(), newv.clone());
                hm.put_key(hk, newv.clone());
                newv.clone()
            }
        }
    };
    cache.insert(key, s.clone());
    s
}

/// Recursive worker for `mce_fused': write the transformed category
/// set into every slot of SUB (a sub-char-table Value) intersecting
/// [FROM,TO].  Fully covered scalar slots collapse to the new value;
/// partially covered scalars materialize a deeper sub-table first —
/// GNU `sub_char_table_set_range' shape, except the written value is
/// computed per slot rather than constant.
/// Recursive worker for `mce_fused'.  Transforms every slot of SUB
/// intersecting [FROM,TO].  Returns Some(V) when the whole sub-table
/// (not just the covered span) resolves to the single scalar V — the
/// caller then collapses SUB's slot to V, matching GNU's
/// `sub_char_table_set_range' block overwrite.  A partially covered
/// child always makes the parent non-uniform (uncovered content
/// stays mixed), so it yields None.
/// Effective old value of a whole slot block when the subtree below V
/// is *uniform* — nil slots read as DEFALT (GNU `char_table_ref' chain
/// stops at this table's defalt for `mce_fused' purposes).  Returns
/// None for mixed content: the caller must descend slot by slot.
/// Purely read-only — lets covered blocks fold without materializing
/// the (usually nil) leaf chain first.
fn mce_fold_value(i: &Interp, v: &Value, defalt: &Value) -> Option<Value> {
    if !is_sub_ct(i, v) {
        return Some(if v.is_nil() {
            defalt.clone()
        } else {
            v.clone()
        });
    }
    let (depth, _min, r) = sub_ct_parts(v)?;
    let b = r.borrow();
    let mut uni: Option<Value> = None;
    for k in 0..CHARTAB_SIZE[depth] {
        let child = b.get(3 + k).cloned().unwrap_or(Value::Nil);
        let vv = mce_fold_value(i, &child, defalt)?;
        match &uni {
            Some(u) if crate::lisp::builtins::eq_values(u, &vv) => {}
            Some(_) => return None,
            None => uni = Some(vv),
        }
    }
    uni
}

fn mce_sub(
    i: &mut Interp,
    sub: &Value,
    from: u32,
    to: u32,
    cat: usize,
    reset: bool,
    t: &Rc<RefCell<Vec<Value>>>,
    defalt: &Value,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) -> Option<Value> {
    let (depth, min, r) = sub_ct_parts(sub)?;
    let chars = 1u32 << CHARTAB_BITS[depth];
    let lim = CHARTAB_SIZE[depth];
    let lo = from.max(min);
    let mut idx = (lo.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    let mut c = min + chars * idx as u32;
    // UNIFORM tracks one foldable value; starts unset and becomes
    // Some(v) once a covered slot resolves — any later covered slot
    // with a different value, or any uncovered child that keeps mixed
    // content, breaks uniformity.
    let mut uniform: Option<Value> = None;
    let mut broke = false;
    if depth == 3 {
        // Leaf level: one slot = one character.  One borrow to read the
        // old values, then one borrow_mut for the whole covered write —
        // the transform cost is a per-object cache lookup (identical
        // `old' objects share one entry), not per-character work.
        let base = c;
        let mut k = idx;
        let mut cc = c;
        let olds: Vec<Value> = {
            let b = r.borrow();
            let mut v: Vec<Value> = Vec::with_capacity(lim - k);
            while k < lim && cc <= to {
                v.push(b.get(3 + k).cloned().unwrap_or(Value::Nil));
                k += 1;
                cc += 1;
            }
            v
        };
        let end = k;
        let mut sets: Vec<Value> = Vec::with_capacity(end - idx);
        for (j, old) in olds.iter().enumerate() {
            if from <= base + j as u32 {
                let old_eff = if old.is_nil() {
                    defalt.clone()
                } else {
                    old.clone()
                };
                sets.push(mce_new_set(i, t, &old_eff, cat, reset, cache));
            } else {
                sets.push(old.clone());
            }
        }
        {
            let mut b = r.borrow_mut();
            for (j, set) in sets.iter().enumerate() {
                if from <= base + j as u32 {
                    b[3 + idx + j] = set.clone();
                    match &uniform {
                        Some(u) if crate::lisp::builtins::eq_values(u, set) => {}
                        Some(_) => broke = true,
                        None => uniform = Some(set.clone()),
                    }
                }
            }
        }
        idx = k;
        c = cc;
    }
    while idx < lim && c <= to {
        let covered = from <= c && c + chars - 1 <= to;
        if !covered {
            // Uncovered content remains inside SUB: the parent can't
            // fold this sub-table to a single scalar.
            if depth < 3 {
                let child = r.borrow()[3 + idx].clone();
                if is_sub_ct(i, &child) {
                    mce_sub(i, &child, from, to, cat, reset, t, defalt, cache);
                } else {
                    let n = make_sub_ct(i, depth + 1, c, child);
                    r.borrow_mut()[3 + idx] = n.clone();
                    mce_sub(i, &n, from, to, cat, reset, t, defalt, cache);
                }
            }
            broke = true;
        } else {
            // Covered slot: try the read-only fold first — a uniform
            // subtree collapses to one scalar write WITHOUT touching
            // (or materializing) its leaves.  Only mixed content
            // descends and writes per-slot.
            let child = r.borrow()[3 + idx].clone();
            let folded = mce_fold_value(i, &child, defalt)
                .map(|old_eff| mce_new_set(i, t, &old_eff, cat, reset, cache));
            match folded {
                Some(v) => {
                    r.borrow_mut()[3 + idx] = v.clone();
                    match &uniform {
                        Some(u) if crate::lisp::builtins::eq_values(u, &v) => {}
                        Some(_) => broke = true,
                        None => uniform = Some(v),
                    }
                }
                None => {
                    let v = if is_sub_ct(i, &child) {
                        mce_sub(i, &child, from, to, cat, reset, t, defalt, cache)
                    } else {
                        let old_eff = if child.is_nil() {
                            defalt.clone()
                        } else {
                            child
                        };
                        Some(mce_new_set(i, t, &old_eff, cat, reset, cache))
                    };
                    match v {
                        Some(v) => {
                            r.borrow_mut()[3 + idx] = v.clone();
                            match &uniform {
                                Some(u) if crate::lisp::builtins::eq_values(u, &v) => {}
                                Some(_) => broke = true,
                                None => uniform = Some(v),
                            }
                        }
                        None => broke = true,
                    }
                }
            }
        }
        idx += 1;
        c += chars;
    }
    // This sub-table folds only if the covered span hit every slot
    // (whole block) AND all resolved to one scalar.
    if broke || idx < lim || lo > min {
        return None;
    }
    uniform
}

/// Fused `modify-category-entry' for a parentless category table:
/// one descent writes [LO,HI] (inclusive) at all levels.
fn mce_fused(
    i: &mut Interp,
    t: &Rc<RefCell<Vec<Value>>>,
    lo: u32,
    hi: u32,
    cat: usize,
    reset: bool,
    defalt: &Value,
    cache: &mut rustc_hash::FxHashMap<usize, Value>,
) {
    // T is the char-table Record; its contents vector is slot 2.
    let contents = match t.borrow().get(2).cloned() {
        Some(Value::Vec(v)) => v,
        _ => return,
    };
    let mut ti = (lo >> 16) as usize;
    let lim = (hi.min(CT_MAX_CHAR) >> 16) as usize;
    while ti <= lim {
        let c = (ti as u32) << 16;
        if c > hi {
            break;
        }
        let slot = {
            let b = contents.borrow();
            b.get(1 + ti).cloned().unwrap_or(Value::Nil)
        };
        let covered = lo <= c && c + 65535 <= hi;
        // Covered 64k block: fold the slot's effective value — uniform
        // content (a nil slot inheriting DEFALT included) collapses to
        // one scalar write without materializing the leaf chain.
        let folded = if covered {
            mce_fold_value(i, &slot, defalt)
        } else {
            None
        };
        if let Some(old_eff) = folded {
            let set = mce_new_set(i, t, &old_eff, cat, reset, cache);
            contents.borrow_mut()[1 + ti] = set;
        } else {
            let sub = if is_sub_ct(i, &slot) {
                slot
            } else {
                let n = make_sub_ct(i, 1, c, slot);
                contents.borrow_mut()[1 + ti] = n.clone();
                n
            };
            if let Some(v) = mce_sub(i, &sub, lo, hi, cat, reset, t, defalt, cache) {
                // Whole 64k block resolved to one scalar — collapse.
                contents.borrow_mut()[1 + ti] = v;
            }
        }
        ti += 1;
    }
    if lo < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

fn f_char_category_set(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `CHECK_CHARACTER' — non-chars are `(characterp CH)'.
    let ch = match &a[0] {
        Value::Int(n) if (0..=CT_MAX_CHAR as i128).contains(n) => *n,
        _ => return Err(i.wrong_type_mut("characterp", &a[0])),
    };
    let t = want_category_table(i, &Value::Nil)?;
    let table = Value::Record(t);
    Ok(match char_table_ref(i, &table, ch as usize) {
        v if !v.is_nil() => v,
        _ => make_bool_vector(i, vec![false; 128]),
    })
}

fn f_category_set_mnemonics(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    // GNU `CHECK_CATEGORY_SET': a bool-vector of exactly 128 bits.
    let bits = if is_bool_vector(i, v) {
        bool_vec_of(i, v)?
    } else {
        return Err(i.wrong_type_mut("categorysetp", v));
    };
    if bits.len() != 128 {
        return Err(i.wrong_type_mut("categorysetp", v));
    }
    Ok(bits)
}

fn f_make_category_set(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: STRING must be unibyte ("Multibyte string in
    // `make-category-set'"), and every byte must name a category
    // (printable ASCII 32..126 — `(categoryp BYTE)').
    let Value::Str(sref) = &a[0] else {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    };
    let chars: Vec<char> = sref.borrow().chars().collect();
    let multibyte = if i.is_unibyte_str(sref) {
        false
    } else if i.is_multibyte_str(sref) {
        true
    } else {
        // Unmarked strings are multibyte when any char exceeds the
        // byte range (same storage rule as `multibyte-string-p');
        // eight-bit proxies in a still-unmarked string count as
        // bytes, not multibyte chars.
        chars
            .iter()
            .any(|&c| (c as u32) > 0xFF && crate::lisp::value::eight_bit_byte(c).is_none())
    };
    if multibyte {
        return Err(i.error_q("Multibyte string in `make-category-set'", &[]));
    }
    let mut bits = vec![false; 128];
    for c in chars {
        let k = crate::lisp::value::eight_bit_byte(c)
            .map(|b| b as usize)
            .unwrap_or(c as usize);
        if !(32..=126).contains(&k) {
            return Err(i.wrong_type_mut("categoryp", &Value::Int(k as i128)));
        }
        bits[k] = true;
    }
    Ok(make_bool_vector(i, bits))
}

fn f_invocation_name(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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
    let s = match v {
        Value::Str(s) => s.borrow().clone(),
        _ => return Err(i.signal_data(sym::ERROR, vec![Value::string("Version must be a string")])),
    };
    if !s
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        // GNU subr.el's `error' runs `format-message', which
        // translates `` `' to curve quotes under the default
        // text-quoting-style.
        return Err(i.error_q(
            "Invalid version syntax: `%s' (must start with a number)",
            &[&s],
        ));
    }
    let mut out = Vec::new();
    for part in s.split('.') {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        out.push(digits.parse().unwrap_or(0));
    }
    Ok(out)
}

fn f_version_to_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    // `version</<=/=' take *strings* ("Version must be a string").
    Ok(version_cmp(
        &version_list_of(i, &a[0])?,
        &version_list_of(i, &a[1])?,
    ))
}

fn f_version_list_cmp(i: &mut Interp, a: &[Value]) -> Result<std::cmp::Ordering, Flow> {
    // `version-list-*' take int lists: listp on the arg, integerp
    // on elements.
    fn ints(i: &mut Interp, v: &Value) -> Result<Vec<i128>, Flow> {
        let items = match v {
            Value::Nil => Vec::new(),
            Value::Cons(_) => want_list(i, v)?,
            other => return Err(i.wrong_type_mut("listp", other)),
        };
        items
            .iter()
            .map(|e| match e {
                Value::Int(n) => Ok(*n),
                other => Err(i.wrong_type_mut("integerp", other)),
            })
            .collect()
    }
    Ok(version_cmp(&ints(i, &a[0])?, &ints(i, &a[1])?))
}

fn f_version_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Less,
    ))
}
fn f_version_le(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? != std::cmp::Ordering::Greater,
    ))
}
fn f_version_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_cmp(i, &a)? == std::cmp::Ordering::Equal,
    ))
}
fn f_version_list_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? == std::cmp::Ordering::Less,
    ))
}
fn f_version_list_le(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? != std::cmp::Ordering::Greater,
    ))
}
fn f_version_list_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        f_version_list_cmp(i, &a)? == std::cmp::Ordering::Equal,
    ))
}

// ---------- predicates ----------

fn f_string_or_null_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(
        &a[0],
        Value::Str(_) | Value::Nil
    )))
}

fn f_vector_or_char_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's VECTOR_OR_CHAR_TABLE_P covers vectors and char-tables.
    Ok(Value::from_bool(
        matches!(&a[0], Value::Vec(_)) || is_char_table(i, &a[0]),
    ))
}

// ---------- threads ----------

fn want_thread(i: &mut Interp, v: &Value) -> Result<crate::lisp::value::ThreadRef, Flow> {
    match v {
        Value::Thread(t) => Ok(t.clone()),
        other => Err(i.wrong_type_mut("threadp", other)),
    }
}

fn f_threadp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Thread(_))))
}

fn f_all_threads(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_current_thread(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Thread(i.threads[i.current_thread].clone()))
}

fn f_thread_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(t.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_thread_live_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(Value::from_bool(t.borrow().alive))
}

fn f_make_thread(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fun = a[0].clone();
    let name = match a.get(1) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let t = crate::lisp::memory::allocate(
        crate::lisp::value::Thread {
            name,
            alive: true,
            result: None,
            last_error: None,
            pending_signal: None,
            blocker: None,
            finished: false,
        },
        crate::lisp::Value::Thread,
    );
    i.threads.push(t.clone());
    if let Err(error) = crate::lisp::eval::threads::spawn(i, fun, t.clone()) {
        i.threads.pop();
        return Err(error);
    }
    Ok(Value::Thread(t))
}

fn f_thread_join(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    if std::rc::Rc::ptr_eq(&t, &i.threads[i.current_thread]) {
        return Err(i.error("Cannot join current thread"));
    }
    // GNU captures a target's pending signal before waiting, even if the
    // target subsequently handles it. A completed target retains it.
    let pending = t.borrow().pending_signal.clone();
    if !t.borrow().finished {
        crate::lisp::eval::threads::wait(
            i,
            crate::lisp::eval::threads::Wait::Join(t.clone()),
            Some(Value::Thread(t.clone())),
        )?;
    }
    let result = t.borrow().result.clone();
    if let Some((symbol, data)) = pending {
        if !symbol.is_nil() {
            return super::evalfn::f_signal(i, vec![symbol, data].into());
        }
    }
    Ok(result.unwrap_or(Value::Nil))
}

fn f_thread_yield(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::eval::threads::yield_now(i)?;
    Ok(Value::Nil)
}

fn f_thread_last_error(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let error = i.thread_last_error.clone();
    if arg(&a, 0).truthy() {
        i.thread_last_error = Value::Nil;
    }
    Ok(error)
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

fn f_mutexp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Mutex(_))))
}

fn f_make_mutex(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match a.first() {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(Value::Nil) | None => None,
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::Mutex(std::rc::Rc::new(std::cell::RefCell::new(
        crate::lisp::value::Mutex {
            name,
            owner: None,
            count: 0,
        },
    )))
    .tracked())
}

fn f_mutex_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    Ok(m.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_mutex_lock(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    loop {
        let current = Value::Thread(i.threads[i.current_thread].clone());
        let available = {
            let mut mutex = m.borrow_mut();
            if mutex
                .owner
                .as_ref()
                .is_none_or(|owner| super::eq_values(owner, &current))
            {
                mutex.owner = Some(current);
                mutex.count += 1;
                true
            } else {
                false
            }
        };
        if available {
            return Ok(Value::Nil);
        }
        crate::lisp::eval::threads::wait(
            i,
            crate::lisp::eval::threads::Wait::Mutex(m.clone()),
            Some(a[0].clone()),
        )?;
    }
}

fn f_mutex_unlock(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let m = want_mutex(i, &a[0])?;
    let current = Value::Thread(i.threads[i.current_thread].clone());
    let mut mutex = m.borrow_mut();
    if !mutex
        .owner
        .as_ref()
        .is_some_and(|owner| super::eq_values(owner, &current))
    {
        drop(mutex);
        return Err(i.error("Cannot unlock mutex owned by another thread"));
    }
    mutex.count -= 1;
    if mutex.count == 0 {
        mutex.owner = None;
    }
    Ok(Value::Nil)
}

fn f_condition_variable_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::CondVar(_))))
}

fn f_make_condition_variable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
            waiters: Vec::new(),
        },
    )))
    .tracked())
}

fn f_condition_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow()
        .name
        .as_ref()
        .map(|n| Value::string(n.clone()))
        .unwrap_or(Value::Nil))
}

fn f_condition_mutex(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    Ok(c.borrow().mutex.clone())
}

fn condition_mutex_held(i: &Interp, c: &crate::lisp::value::CondVarRef) -> bool {
    let cb = c.borrow();
    let Value::Mutex(m) = &cb.mutex else {
        return false;
    };
    let mb = m.borrow();
    match &mb.owner {
        Some(Value::Thread(t)) => std::rc::Rc::ptr_eq(t, &i.threads[i.current_thread]),
        _ => false,
    }
}

fn f_condition_wait(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error_q(
            "Condition variable's mutex is not held by current thread",
            &[],
        ));
    }
    let mutex = c.borrow().mutex.clone();
    let Value::Mutex(m) = &mutex else {
        unreachable!()
    };
    let count = {
        let mut held = m.borrow_mut();
        let count = held.count;
        held.owner = None;
        held.count = 0;
        count
    };
    let notified = std::rc::Rc::new(std::cell::Cell::new(false));
    c.borrow_mut()
        .waiters
        .push(std::rc::Rc::downgrade(&notified));
    let result = crate::lisp::eval::threads::wait(
        i,
        crate::lisp::eval::threads::Wait::Condition(notified.clone()),
        Some(a[0].clone()),
    );
    c.borrow_mut().waiters.retain(|w| {
        w.upgrade()
            .is_some_and(|w| !std::rc::Rc::ptr_eq(&w, &notified))
    });
    // A signaled waiter still reacquires its mutex before unwinding.
    f_mutex_lock(i, vec![mutex.clone()].into())?;
    m.borrow_mut().count = count;
    result?;
    Ok(Value::Nil)
}

fn f_condition_notify(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let c = want_condvar(i, &a[0])?;
    if !condition_mutex_held(i, &c) {
        return Err(i.error_q(
            "Condition variable's mutex is not held by current thread",
            &[],
        ));
    }
    let all = arg(&a, 1).truthy();
    let mut notified = false;
    c.borrow_mut().waiters.retain(|weak| {
        let Some(waiter) = weak.upgrade() else {
            return false;
        };
        if !waiter.get() && (all || !notified) {
            waiter.set(true);
            notified = true;
        }
        true
    });
    // GNU briefly releases every recursive level while notifying, then
    // reacquires the mutex before returning to Lisp.
    let mutex = c.borrow().mutex.clone();
    let Value::Mutex(m) = &mutex else {
        unreachable!()
    };
    let count = {
        let mut held = m.borrow_mut();
        let count = held.count;
        held.owner = None;
        held.count = 0;
        count
    };
    let result = crate::lisp::eval::threads::yield_now(i);
    f_mutex_lock(i, vec![mutex.clone()].into())?;
    m.borrow_mut().count = count;
    result?;
    Ok(Value::Nil)
}

fn f_make_finalizer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_FUNCTION's the
    // FUNCTION argument → `functionp'.  The function is retained but
    // never invoked (no GC hooks needed for observable behavior).
    if !super::data::functionp_val(i, &a[0]) {
        return Err(i.wrong_type_mut("functionp", &a[0]));
    }
    Ok(Value::Finalizer(std::rc::Rc::new(std::cell::RefCell::new(a[0].clone()))).tracked())
}

fn f_byte_to_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    match u32::try_from(n).ok().and_then(char::from_u32) {
        Some(c) => Ok(Value::string(c.to_string())),
        None => Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone()])),
    }
}

fn f_get_load_suffixes(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // macOS module suffixes first, like GNU.
    Ok(Value::list(
        [".so", ".dylib", ".elc", ".elc.gz", ".el", ".el.gz"]
            .iter()
            .map(|s| Value::string(*s))
            .collect(),
    ))
}

fn f_num_processors(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let n = std::thread::available_parallelism()
        .map(|n| n.get() as i128)
        .unwrap_or(1);
    Ok(Value::Int(n))
}

fn f_daemon_initialized(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("This function can only be called if emacs is run as a daemon"))
}

fn f_signal_names(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // POSIX names in reverse signal-number order, as GNU's `signal-names'.
    const NAMES: &[&str] = &[
        "USR2", "USR1", "INFO", "WINCH", "PROF", "VTALRM", "XFSZ", "XCPU", "IO", "TTOU", "TTIN",
        "CHLD", "CONT", "TSTP", "STOP", "URG", "TERM", "ALRM", "PIPE", "SYS", "SEGV", "BUS",
        "KILL", "FPE", "EMT", "ABRT", "TRAP", "ILL", "QUIT", "INT", "HUP", "EXIT",
    ];
    Ok(Value::list(
        NAMES.iter().map(|s| Value::string(*s)).collect(),
    ))
}

fn f_cl_type_of(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Int(n) if (crate::lisp::value::FIXNUM_MIN..=crate::lisp::value::FIXNUM_MAX).contains(n) => "fixnum",
        Value::Int(_) | Value::BigInt(_) => "bignum",
        Value::Float(_) => "float",
        Value::Sym(_) => "symbol",
        // GNU `cl-type-of' reports `null' for nil and a record's type
        // field (slot 0) when it is a symbol — like `type-of'.
        Value::Nil => "null",
        Value::Cons(_) => "cons",
        Value::Str(_) => "string",
        Value::Vec(_) => "vector",
        Value::Record(r) => {
            let rr = r.borrow();
            match rr.first() {
                Some(Value::Sym(tag)) => return Ok(Value::Sym(*tag)),
                // GNU: when slot 0 is itself a record with size > 1 it
                // is a class object (EIEIO instances mid-`initialize-instance')
                // and the reported type is that object's name field (slot 1).
                Some(Value::Record(t)) if t.borrow().len() > 1 => {
                    return Ok(t.borrow()[1].clone());
                }
                _ => {}
            }
            "record"
        }
        Value::Hash(_) => "hash-table",
        Value::Subr(s) => {
            if matches!(s.arity, Arity::Unevalled) {
                "special-form"
            } else if crate::lisp::builtins::comp::is_native(s).is_some() {
                "native-comp-function"
            } else {
                "primitive-function"
            }
        }
        Value::Lambda(l) => {
            if l.is_bytecode() {
                "byte-code-function"
            } else {
                "interpreted-function"
            }
        }
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
    let data = Value::vector(
        bits.into_iter()
            .map(|b| Value::Int(if b { 1 } else { 0 }))
            .collect(),
    );
    Value::record(vec![Value::Sym(i.q("bool-vector")), data])
}

fn f_bool_vector_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_bool_vector(i, &a[0])))
}

fn f_make_bool_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_bool_vector: LENGTH is CHECK_FIXNAT — non-integers signal
    // `wholenump' (not `integerp').
    let n = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let init = !arg(&a, 1).is_nil();
    // GNU Fmake_bool_vector CHECK_FIXNATs LENGTH → `wholenump', not
    // args-out-of-range.
    if n < 0 {
        return Err(i.wrong_type_mut("wholenump", &a[0]));
    }
    if n > 0x2000_0000 * 8 {
        return Err(i.error("Memory exhausted--use C-x s then exit and restart Emacs"));
    }
    Ok(make_bool_vector(i, vec![init; n as usize]))
}

fn f_bool_vector_length(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(b.len() as i128))
}

fn f_bool_vector_subsetp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let y = bool_vec_of(i, &a[1])?;
    if x.len() != y.len() {
        // GNU Fbool_vector_subsetp: (wrong-length-argument ALEN BLEN
        // BLEN) — the trailing datum is B's length (where the walk
        // stopped), not the vector objects.
        let s = i.q("wrong-length-argument");
        return Err(i.signal_data(
            s,
            vec![
                Value::Int(x.len() as i128),
                Value::Int(y.len() as i128),
                Value::Int(y.len() as i128),
            ],
        ));
    }
    Ok(Value::from_bool(
        x.iter().zip(&y).all(|(p, q)| !(*p && !*q)),
    ))
}

fn f_bool_vector_not(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
                    let s = i.q("wrong-length-argument");
                    return Err(i.signal_data(
                        s,
                        vec![Value::Int(out.len() as i128), Value::Int(bb.len() as i128)],
                    ));
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
    let target = arg(a, 2);
    let dest = if target.is_nil() {
        None
    } else {
        Some(bool_vec_of(i, &target)?)
    };
    if x.len() != y.len() || dest.as_ref().is_some_and(|d| d.len() != x.len()) {
        // GNU includes DEST's length only when a destination was supplied.
        let mut lengths = vec![Value::Int(x.len() as i128), Value::Int(y.len() as i128)];
        if let Some(dest) = &dest {
            lengths.push(Value::Int(dest.len() as i128));
        }
        let symbol = i.q("wrong-length-argument");
        return Err(i.signal_data(symbol, lengths));
    }
    let out: Vec<bool> = x.iter().zip(&y).map(|(p, q)| f(*p, *q)).collect();
    if let Some(dest) = dest {
        // A supplied destination is returned only when its contents change.
        // Snapshots of both sources also make aliasing DEST with either safe.
        if dest == out {
            return Ok(Value::Nil);
        }
        if let Value::Record(r) = &target {
            let rr = r.borrow();
            if let Some(Value::Vec(b)) = rr.get(1) {
                for (bit, value) in b.borrow_mut().iter_mut().zip(out) {
                    *bit = Value::Int(i128::from(value));
                }
            }
        }
        return Ok(target);
    }
    Ok(make_bool_vector(i, out))
}

fn f_bool_vector_bin(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p != q)
}
fn f_bool_vector_union(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p || q)
}
fn f_bool_vector_inter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p && q)
}
fn f_bool_vector_diff(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    bv_binop(i, &a, |p, q| p && !q)
}

fn f_bool_vector_count(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    Ok(Value::Int(x.iter().filter(|b| **b).count() as i128))
}

fn f_bool_vector_consec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let x = bool_vec_of(i, &a[0])?;
    let b = !a[1].is_nil();
    let at = match &a[2] {
        Value::Int(n) if (0..=crate::lisp::value::FIXNUM_MAX).contains(n) => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    // GNU allows the index just past the last bit, yielding zero.
    if at > x.len() {
        let symbol = i.q("args-out-of-range");
        return Err(i.signal_data(symbol, vec![a[0].clone(), a[2].clone()]));
    }
    Ok(Value::Int(
        x.iter().skip(at).take_while(|v| **v == b).count() as i128,
    ))
}

// ---------- events ----------

use crate::editor::err_sym;
use crate::editor::{
    CHAR_ALT, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER, WindowRef, is_keymap,
    sel_frame, sel_window, w_parent,
};

fn f_eventp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let ok = match &a[0] {
        Value::Cons(c) => matches!(c.borrow().car, Value::Sym(_) | Value::Int(_)),
        Value::Int(_) | Value::Sym(_) => true,
        _ => false,
    };
    Ok(Value::from_bool(ok))
}

/// GNU's canonical order for `event-modifiers' output:
/// (meta control shift hyper super alt triple double down drag click).
fn mod_list_rank(m: &str) -> usize {
    [
        "meta", "control", "shift", "hyper", "super", "alt", "triple", "double", "down", "drag",
        "click",
    ]
    .iter()
    .position(|x| *x == m)
    .unwrap_or(usize::MAX)
}

/// Parse an event symbol NAME into (char-mods, click-mods, base-name):
/// prefixes `A- C- H- M- S- s-' then `down- drag- double- triple-'.
/// (`click-' is not a name prefix in GNU.)
fn parse_event_symbol(name: &str) -> (Vec<String>, Vec<String>, String) {
    let mut cmods = Vec::new();
    let mut kmods = Vec::new();
    let mut name = name.to_string();
    // GNU parse_modifiers_uncached scans all modifier prefixes in ONE
    // pass, in the order written (drag-M-x parses both).
    loop {
        let mut hit = false;
        for (p, m) in [
            ("A-", "alt"),
            ("C-", "control"),
            ("H-", "hyper"),
            ("M-", "meta"),
            ("S-", "shift"),
            ("s-", "super"),
            ("down-", "down"),
            ("drag-", "drag"),
            ("double-", "double"),
            ("triple-", "triple"),
            ("up-", "up"),
        ] {
            if let Some(r) = name.strip_prefix(p) {
                if p.len() == 2 {
                    cmods.push(m.to_string());
                } else {
                    kmods.push(m.to_string());
                }
                name = r.to_string();
                hit = true;
                break;
            }
        }
        if !hit {
            break;
        }
    }
    (cmods, kmods, name)
}

/// `internal-event-symbol-parse-modifiers' (keyboard.c
/// Fevent_symbol_parse_modifiers): parse SYMBOL like a key lookup
/// does — caches `event-symbol-elements'/`-mask' and returns
/// (BASE . MOD-SYMS).
pub(crate) fn f_internal_event_symbol_parse(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(symbol) = i.sym_id(&a[0]) else { return Err(i.wrong_type_mut("symbolp", &a[0])); };
    parse_event_symbol_cached(i, symbol)?;
    let property = i.q("event-symbol-elements");
    Ok(i.get_prop(symbol, property))
}

fn event_modifier_bit(i: &mut Interp, event: &Value, mask: i128) -> Result<bool, Flow> {
    let bits = super::call_native_builtin(i, "logand", vec![event.clone(), Value::Int(mask)])?;
    let zero = super::call_canonical_builtin(i, "zerop", vec![bits])?;
    Ok(zero.is_nil())
}

fn f_event_modifiers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let event = a[0].clone();
    if super::call_native_builtin(i, "stringp", vec![event.clone()])?.truthy() {
        return Ok(Value::Nil);
    }
    let mut event_type = event;
    if super::call_native_builtin(i, "listp", vec![event_type.clone()])?.truthy() {
        event_type = super::call_canonical_builtin(i, "car", vec![event_type])?;
    }
    if super::call_native_builtin(i, "symbolp", vec![event_type.clone()])?.truthy() {
        let parsed = super::call_native_builtin(i, "internal-event-symbol-parse-modifiers", vec![event_type])?;
        return super::call_canonical_builtin(i, "cdr", vec![parsed]);
    }
    let mask = CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
    let character = super::call_native_builtin(i, "logand", vec![event_type.clone(), Value::Int(!mask)])?;
    let mut modifiers = Vec::new();
    if event_modifier_bit(i, &event_type, CHAR_META)? { modifiers.push("meta"); }
    if event_modifier_bit(i, &event_type, CHAR_CTL)?
        || super::call_native_builtin(i, "<", vec![character.clone(), Value::Int(32)])?.truthy() {
        modifiers.push("control");
    }
    let shifted = if event_modifier_bit(i, &event_type, CHAR_SHIFT)? { true } else {
        let lower = super::call_native_builtin(i, "downcase", vec![character.clone()])?;
        super::call_canonical_builtin(i, "/=", vec![character, lower])?.truthy()
    };
    if shifted { modifiers.push("shift"); }
    for (bit, name) in [(CHAR_HYPER, "hyper"), (CHAR_SUPER, "super"), (CHAR_ALT, "alt")] {
        if event_modifier_bit(i, &event_type, bit)? { modifiers.push(name); }
    }
    modifiers.reverse();
    Ok(Value::list(modifiers.into_iter().map(|name| Value::Sym(i.intern(name))).collect()))
}

fn event_basic(i: &mut Interp, event: &Value) -> EvalResult {
    if super::call_native_builtin(i, "stringp", vec![event.clone()])?.truthy() {
        return Ok(Value::Nil);
    }
    let mut event = event.clone();
    if super::call_native_builtin(i, "consp", vec![event.clone()])?.truthy() {
        event = super::call_canonical_builtin(i, "car", vec![event])?;
    }
    if super::call_native_builtin(i, "symbolp", vec![event.clone()])?.truthy() {
        let property = Value::Sym(i.q("event-symbol-elements"));
        let parsed = super::call_native_builtin(i, "get", vec![event, property])?;
        return super::call_canonical_builtin(i, "car", vec![parsed]);
    }
    let base = super::call_native_builtin(i, "logand", vec![event, Value::Int(CHAR_ALT - 1)])?;
    let uncontrolled = if super::call_native_builtin(i, "<", vec![base.clone(), Value::Int(32)])?.truthy() {
        super::call_native_builtin(i, "logior", vec![base, Value::Int(64)])?
    } else { base };
    // The dumped Lisp body catches errors from downcase only. Offer signals
    // with that scope active so observers and clause selection match GNU.
    let depth = i.case_handlers.len();
    i.case_handlers.push(crate::lisp::error::CaseHandler::single(Value::Sym(sym::ERROR)));
    let result = super::call_native_builtin(i, "downcase", vec![uncontrolled.clone()]);
    let result = i.offer_flow(result);
    i.case_handlers.truncate(depth);
    match result {
        Err(Flow::Signal(signal)) if signal.claims_frame(depth) => {
            i.finish_quit_unwind();
            Ok(uncontrolled)
        }
        Err(flow) => { i.rebase_quit_unwind(); Err(flow) }
        result => result,
    }
}
/// GNU parse_modifiers caches the parsing result, including an empty base.
/// Existing cons caches are authoritative and parsing does not apply modifiers.
fn parse_event_symbol_cached(i: &mut Interp, s: SymId) -> EvalResult {
    let esem = i.q("event-symbol-element-mask");
    let cached = i.get_prop(s, esem);
    if matches!(cached, Value::Cons(_)) { return Ok(cached); }
    let name = i.symbol_name(s);
    let (cmods, kmods, base) = parse_event_symbol(&name);
    const ORDER: &[(&str, i128)] = &[
        ("alt", CHAR_ALT), ("control", CHAR_CTL), ("hyper", CHAR_HYPER),
        ("meta", CHAR_META), ("shift", CHAR_SHIFT), ("super", CHAR_SUPER),
        ("double", 1 << 4), ("triple", 1 << 5), ("up", 1 << 0),
        ("down", 1 << 1), ("drag", 1 << 2),
    ];
    let mut mask: i128 = 0;
    let mut lispy: Vec<String> = Vec::new();
    for (modname, bit) in ORDER {
        if cmods.iter().any(|m| m == modname) || kmods.iter().any(|m| m == modname) {
            mask |= bit;
            lispy.push(modname.to_string());
        }
    }
    // GNU caches the parse even with zero modifiers (f1 → (f1 . nil)).
    // click modifier rules from parse_modifiers_uncached: `mouse-D'
    // (single digit) unless a down/drag/double/triple modifier is
    // present; `wheel-…' unless double/triple.
    let no_dir = !kmods
        .iter()
        .any(|m| matches!(m.as_str(), "down" | "drag" | "double" | "triple"));
    let is_mouse_d =
        base.len() == 7 && base.starts_with("mouse-") && base.as_bytes()[6].is_ascii_digit();
    let is_wheel = base.len() > 6
        && base.starts_with("wheel-")
        && !kmods
            .iter()
            .any(|m| matches!(m.as_str(), "double" | "triple"));
    if (no_dir && is_mouse_d) || is_wheel {
        mask |= 1 << 3;
        lispy.push("click".to_string());
    }
    // GNU's lispy list order: meta control shift hyper super alt
    // triple double down drag click (lispy_modifier_list walks the
    // bit table descending).
    lispy.sort_by_key(|m| mod_list_rank(m));
    let base_value = super::data::f_intern(i, vec![Value::string(base)].into())?;
    let ese = i.q("event-symbol-elements");
    let mod_syms: Vec<Value> = lispy.iter().map(|m| Value::Sym(i.intern(m))).collect();
    let result = Value::list(vec![base_value.clone(), Value::Int(mask)]);
    i.put_prop(s, esem, result.clone());
    i.put_prop(
        s,
        ese,
        Value::cons(base_value.clone(), Value::list(mod_syms)),
    );
    Ok(result)
}

fn event_cache_car(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().car.clone()),
        value => Err(i.wrong_type_mut("listp", value)),
    }
}

fn event_cache_cdr(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().cdr.clone()),
        value => Err(i.wrong_type_mut("listp", value)),
    }
}

pub(crate) fn reorder_event_symbol(i: &mut Interp, symbol: SymId) -> EvalResult {
    let parsed = parse_event_symbol_cached(i, symbol)?;
    let base = event_cache_car(i, &parsed)?;
    if i.sym_id(&base).is_none() { return Err(i.wrong_type_mut("symbolp", &base)); }
    let tail = event_cache_cdr(i, &parsed)?;
    let mask = event_cache_car(i, &tail)?;
    if !mask.fixnump() { return Err(i.wrong_type_mut("fixnump", &mask)); }
    apply_event_modifiers(i, mask.int().unwrap(), base)
}

fn f_event_basic_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    event_basic(i, &a[0])
}

pub(crate) fn f_event_convert_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut base = Value::Nil;
    let mut mods = 0i128;
    let mut tail = a[0].clone();
    // GNU FOR_EACH_TAIL_SAFE uses Brent's walk and stops on a cycle
    // without signalling. Preserve its detection timing: a second base
    // can signal before the cycle is detected.
    let mut tortoise = tail.clone();
    let mut span = 2usize;
    let mut blocks = 0isize;
    let mut steps = 2u16;
    while let Value::Cons(cell) = tail {
        let (event, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        // Only a symbol followed by another element can be a modifier.
        let modifier = if matches!(next, Value::Cons(_)) {
            i.sym_id(&event)
                .map(|id| match i.symbol_name(id).as_str() {
                    "A" | "alt" => CHAR_ALT,
                    "C" | "ctrl" | "control" => CHAR_CTL,
                    "H" | "hyper" => CHAR_HYPER,
                    "M" | "meta" => CHAR_META,
                    "S" | "shift" => CHAR_SHIFT,
                    "s" | "super" => CHAR_SUPER,
                    "up" => 1,
                    "down" => 2,
                    "drag" => 4,
                    "click" => 8,
                    "double" => 16,
                    "triple" => 32,
                    _ => 0,
                })
                .unwrap_or(0)
        } else {
            0
        };
        if modifier != 0 {
            mods |= modifier;
        } else if !base.is_nil() {
            return Err(i.error("Two bases given in one event"));
        } else {
            base = event;
        }
        tail = next;
        steps = steps.wrapping_sub(1);
        let check_cycle = if steps != 0 {
            true
        } else {
            blocks -= 1;
            if blocks > 0 {
                true
            } else {
                span = span.saturating_mul(2);
                steps = span as u16;
                blocks = (span >> 16) as isize;
                tortoise = tail.clone();
                false
            }
        };
        if check_cycle && eq_values(&tail, &tortoise) {
            break;
        }
    }
    if let Some(id) = i.sym_id(&base) {
        let name = i.symbol_name(id);
        let mut chars = name.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            base = Value::Int(crate::lisp::value::lisp_char_code(ch));
        }
    }
    if let Value::Int(code) = &base {
        if base.fixnump() {
            return Ok(Value::Int(crate::editor::apply_mods_ev(*code, mods)));
        }
    }
    let Some(symbol) = i.sym_id(&base) else {
        return Err(i.error("Invalid base event"));
    };
    let base = i.sym(symbol);
    apply_event_modifiers(i, mods, base)
}

/// keyboard.c apply_modifiers: cache and kind inheritance are distinct from
/// parse_modifiers, which must not populate this cache itself.
fn apply_event_modifiers(i: &mut Interp, modifiers: i128, base: Value) -> EvalResult {
    let mods = i128::from(modifiers as i32);
    if base.fixnump() { return Ok(Value::Int(base.int().unwrap() | mods)); }
    let Some(symbol) = i.sym_id(&base) else { return Err(i.wrong_type_mut("symbolp", &base)); };
    // GNU apply_modifiers keeps the original base name, including any
    // existing prefixes. Click is absent from both the name and cache key.
    let cache_property = i.q("modifier-cache");
    let cache = i.get_prop(symbol, cache_property);
    let index = mods & !8;
    let mut entries = cache.clone();
    let mut tortoise = entries.clone();
    let mut span = 1usize;
    let mut steps = 0usize;
    let mut converted = None;
    while let Value::Cons(entry) = entries {
        let pair = entry.borrow().car.clone();
        entries = entry.borrow().cdr.clone();
        if let Value::Cons(pair) = pair {
            if matches!(pair.borrow().car, Value::Int(n) if n == index) {
                converted = Some(pair.borrow().cdr.clone());
                break;
            }
        }
        steps += 1;
        if eq_values(&entries, &tortoise) { break; }
        if steps == span {
            tortoise = entries.clone();
            span = span.saturating_mul(2);
            steps = 0;
        }
    }
    let converted = if let Some(value) = converted { value } else {
      let mut name = String::new();
    for (bit, prefix) in [
        (CHAR_ALT, "A-"),
        (CHAR_CTL, "C-"),
        (CHAR_HYPER, "H-"),
        (CHAR_META, "M-"),
        (CHAR_SHIFT, "S-"),
        (CHAR_SUPER, "s-"),
        (16, "double-"),
        (32, "triple-"),
        (1, "up-"),
        (2, "down-"),
        (4, "drag-"),
    ] {
        if mods & bit != 0 {
            name.push_str(prefix);
        }
    }
    name.push_str(&i.symbol_name(symbol));
    let name = Value::string(name);
    if let Value::Str(string) = &name { i.mark_multibyte(string); }
    let converted = super::data::f_intern(i, vec![name].into())?;
    let entry = Value::cons(Value::Int(index), converted.clone());
    i.put_prop(symbol, cache_property, Value::cons(entry, cache));
    converted
    };
    let Some(converted_symbol) = i.sym_id(&converted) else { return Err(i.wrong_type_mut("symbolp", &converted)); };
    let kind = i.q("event-kind");
    if i.get_prop(converted_symbol, kind).is_nil() {
        let value = i.get_prop(symbol, kind);
        if !value.is_nil() { i.put_prop(converted_symbol, kind, value); }
    }
    Ok(converted)
}

fn f_listify_key_sequence(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut events = super::seq::seq_to_vec(i, &a[0])?;
    // GNU subr.el preserves vector and multibyte-string elements.
    if !matches!(a[0], Value::Vec(_)) && !super::strfn::string_multibyte_p(i, &a[0]) {
        for event in &mut events {
            if super::arith::f_gt(i, vec![event.clone(), Value::Int(127)].into())?.truthy() {
                let modifier = i.q("listify-key-sequence-1");
                let modifier = i.eval(&Value::Sym(modifier))?;
                *event = super::arith::f_logxor(i, vec![event.clone(), modifier].into())?;
            }
        }
    }
    Ok(Value::list(events))
}

fn f_key_valid_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    // GNU's top-level shape: `\`[^ ]+\( [^ ]+\)*\'' — non-empty words
    // joined by SINGLE spaces (double spaces or leading/trailing are
    // invalid).
    if s.is_empty() || s.split(' ').any(|t| t.is_empty()) {
        return Ok(Value::Nil);
    }
    for tok in s.split(' ').filter(|t| !t.is_empty()) {
        // GNU `key-valid-p': modifiers may only be given in the fixed
        // order A-C-H-M-S-s — each optional, but no repeats or
        // reordering.
        let mut rest = tok;
        for m in ["A-", "C-", "H-", "M-", "S-", "s-"] {
            if let Some(r) = rest.strip_prefix(m) {
                rest = r;
            }
        }
        let chs: Vec<char> = rest.chars().collect();
        let ok = if chs.len() == 1 {
            // Single char: printable only (GNU rejects control chars
            // and unibyte 128..255 high bytes).
            let c = chs[0] as u32;
            c >= 32 && !(128..=255).contains(&c)
        } else if rest.starts_with('<') && rest.ends_with('>') {
            // <NAME>: NAME is [-_A-Za-z0-9]+ with NO inner modifier
            // prefixes (GNU rejects `<M-C-down>').
            let inner = &rest[1..rest.len() - 1];
            !inner.is_empty()
                && inner
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                && !["A-", "C-", "H-", "M-", "S-", "s-"]
                    .iter()
                    .any(|m| inner.starts_with(m))
        } else {
            // Shorthand names NUL RET TAB LFD ESC SPC DEL, as the whole
            // word or after a `-X' boundary (GNU's `\<…$').
            ["NUL", "RET", "TAB", "LFD", "ESC", "SPC", "DEL"]
                .iter()
                .any(|n| {
                    rest == *n
                        || (rest.ends_with(n)
                            && rest.len() > n.len()
                            && rest.as_bytes()[rest.len() - n.len() - 1] == b'-')
                })
        };
        if !ok {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
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

fn f_days_between(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn month_from_name(tok: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let mut t = tok.to_ascii_lowercase();
    t.truncate(3);
    MONTHS.iter().position(|m| t == *m).map(|p| p as i64 + 1)
}

/// Parse a numeric zone token like `+0900`, `-05:30` into offset seconds.
fn parse_zone_offset(tok: &str) -> Option<i64> {
    let (sign, rest) = match tok.as_bytes().first()? {
        b'+' => (1i64, &tok[1..]),
        b'-' => (-1i64, &tok[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let n: i64 = digits.parse().ok()?;
    if rest.contains(':') || digits.len() <= 2 {
        Some(sign * (n * 3600 + 0))
    } else {
        Some(sign * ((n / 100) * 3600 + (n % 100) * 60))
    }
}

/// Parse the date+time forms GNU `parse-time-string' accepts in practice:
/// ISO `YYYY-MM-DD[T]HH:MM[:SS][Z|±ZZZZ]`, ctime `[Www] Mmm DD HH:MM:SS
/// YYYY`, RFC-822 `DD Mmm YYYY HH:MM:SS ZZZZ`.  Returns the broken-down
/// fields and an explicit zone offset in seconds when the string carried
/// one.
fn parse_date_hms(s: &str) -> Option<(i64, i64, i64, i64, i64, i64, Option<i64>)> {
    let mut year = None;
    let mut month = None;
    let mut day = None;
    let (mut hh, mut mm, mut ss) = (0i64, 0i64, 0i64);
    let mut zone = None;
    for raw in s.split(|c: char| c.is_whitespace() || c == ',') {
        let mut tok = raw;
        if tok.is_empty() {
            continue;
        }
        // ISO date `YYYY-MM-DD' or `YYYY/MM/DD', possibly glued to the
        // time via `T'.
        if tok.len() >= 8
            && tok[..4].chars().all(|c| c.is_ascii_digit())
            && matches!(tok.as_bytes()[4], b'-' | b'/')
        {
            let (dtok, rest) = match tok.find('T') {
                Some(p) => (&tok[..p], &tok[p + 1..]),
                None => (tok, ""),
            };
            let dpart: Vec<&str> = dtok.split(['-', '/']).collect();
            if dpart.len() >= 3 {
                year = dpart[0].parse().ok();
                month = dpart[1].parse().ok();
                day = dpart[2].parse().ok();
            }
            if rest.is_empty() {
                continue;
            }
            tok = rest;
        }
        // HH:MM[:SS][.frac][Z|±ZZZZ]
        if tok.contains(':') && tok.as_bytes()[0].is_ascii_digit() {
            let (hmain, zpart) = match tok.find(|c| c == '+' || c == 'Z' || c == 'z') {
                Some(p) => (&tok[..p], Some(&tok[p..])),
                None => (tok, None),
            };
            let p: Vec<&str> = hmain.split(':').collect();
            if p.len() >= 2 && p[0].chars().all(|c| c.is_ascii_digit()) {
                hh = p[0].parse().ok()?;
                mm = p[1].parse().ok()?;
                if p.len() >= 3 {
                    ss = p[2].split('.').next().unwrap_or("0").parse().unwrap_or(0);
                }
            }
            if let Some(z) = zpart {
                zone = Some(if z.eq_ignore_ascii_case("z") {
                    0
                } else {
                    parse_zone_offset(z).unwrap_or(0)
                });
            }
            continue;
        }
        if let Some(z) = parse_zone_offset(tok) {
            zone = Some(z);
            continue;
        }
        if matches!(tok, "Z" | "z" | "UT" | "UTC" | "GMT" | "ut" | "utc" | "gmt") {
            zone = Some(0);
            continue;
        }
        if let Some(m) = month_from_name(tok) {
            month = Some(m);
            continue;
        }
        if tok.chars().all(|c| c.is_ascii_digit()) {
            match tok.len() {
                4 => year = year.or(tok.parse().ok()),
                1 | 2 => day = day.or(tok.parse().ok()),
                _ => {}
            }
        }
    }
    Some((year?, month?, day?, hh, mm, ss, zone))
}

fn f_date_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    if let Some((y, m, d, hh, mm, ss, zone)) = parse_date_hms(&s) {
        let mut secs = days_from_civil(y as i128, m as i128, d as i128) * 86400
            + hh as i128 * 3600
            + mm as i128 * 60
            + ss as i128;
        // Broken-down fields are local wall time unless a zone was given.
        secs -= match zone {
            Some(z) => z as i128,
            None => interp_local_tm(i, secs as i64).tm_gmtoff as i128,
        };
        let hi = secs.div_euclid(65536);
        let lo = secs.rem_euclid(65536);
        return Ok(Value::list(vec![
            Value::Int(hi as i128),
            Value::Int(lo as i128),
        ]));
    }
    // GNU signals (error "Invalid date: DATE") on unparseable input;
    // `safe-date-to-time' relies on catching it.
    Err(i.error(format!("Invalid date: {}", s)))
}

fn f_memory_limit(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el: (cdr (assq 'vsize (process-attributes (emacs-pid))))
    // — own process virtual size in KiB via ps(1).
    let pid = std::process::id();
    if let Ok(o) = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "vsz="])
        .output()
    {
        let text = String::from_utf8_lossy(&o.stdout);
        if let Some(v) = text
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<i128>().ok())
        {
            return Ok(Value::Int(v));
        }
    }
    Ok(Value::Int(0))
}

fn f_help_function_arglist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // A (lambda PARAMS . BODY) form: extract PARAMS directly.
    let mut v = i.indirect_function_value(&a[0]);
    if let Value::Cons(_) = v {
        if let Ok(items) = v.list_to_vec() {
            let head_lam = matches!(items.first(), Some(Value::Sym(s))
            if *s == i.q("lambda") || *s == i.q("closure"));
            if head_lam {
                if let Ok(l) = i.lambda_from_form(&v, None) {
                    v = Value::Lambda(std::rc::Rc::new(l)).tracked();
                }
            }
        }
    }
    match v {
        Value::Lambda(l) => {
            let mut v: Vec<Value> = l.required.iter().map(|s| Value::Sym(*s)).collect();
            if !l.optional.is_empty() {
                v.push(Value::Sym(i.q("&optional")));
                for o in &l.optional {
                    v.push(Value::Sym(o.sym));
                }
            }
            if let Some(r) = l.rest {
                v.push(Value::Sym(i.q("&rest")));
                v.push(Value::Sym(r));
            }
            Ok(Value::list(v))
        }
        Value::Subr(s) => {
            // PRESERVE-NAMES: GNU recovers the real arg names from the
            // docstring's `(fn ARGLIST)' trailer, lowercased.
            if a.get(1).map(|v| v.truthy()).unwrap_or(false) {
                let doc = doc_text(i, &format!("F{}", s.name)).unwrap_or_else(|| s.doc.to_string());
                if let Some(list) = doc_arglist(i, &doc) {
                    return Ok(list);
                }
            }
            Ok(match s.arity {
                Arity::Range { min, max } => {
                    let mut v = Vec::new();
                    for k in 0..min {
                        v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                    }
                    if max > min {
                        v.push(Value::Sym(i.q("&optional")));
                        for k in min..max {
                            v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                        }
                    }
                    Value::list(v)
                }
                Arity::Many { min } => {
                    let mut v: Vec<Value> = (0..min)
                        .map(|k| Value::Sym(i.intern(&format!("arg{}", k + 1))))
                        .collect();
                    v.push(Value::Sym(i.q("&rest")));
                    v.push(Value::Sym(i.q("rest")));
                    Value::list(v)
                }
                // Special forms: GNU `func-arity' reports
                // (MIN . unevalled); the arglist is `arg1..argN &rest
                // rest' — e.g. `if' → (arg1 arg2 &rest rest).
                Arity::Unevalled => {
                    let min = crate::lisp::special::special_form_min_args(i.intern(s.name));
                    let mut v: Vec<Value> = (0..min)
                        .map(|k| Value::Sym(i.intern(&format!("arg{}", k + 1))))
                        .collect();
                    v.push(Value::Sym(i.q("&rest")));
                    v.push(Value::Sym(i.q("rest")));
                    Value::list(v)
                }
            })
        }
        _ => Ok(Value::t()),
    }
}

/// Parse the `(fn ARGS...)' trailer of a subr docstring into a list of
/// lowercased arg-name symbols (GNU's PRESERVE-NAMES path).
fn doc_arglist(i: &mut Interp, doc: &str) -> Option<Value> {
    let start = doc.rfind("(fn ")?;
    let inner = &doc[start + 4..];
    let mut depth = 1i32;
    let mut end = inner.len();
    for (off, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = off;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let names: Vec<Value> = inner[..end]
        .split_whitespace()
        .map(|t| Value::Sym(i.intern(&t.to_lowercase())))
        .collect();
    Some(Value::list(names))
}

// ---------- GNU DOC file ----------

/// Lazily-parsed index of the GNU `etc/DOC' file named by
/// `doc-directory': "Fname"/"Vname" -> byte range of the doc text.
/// Shared across interpreters via thread-local cache keyed by path.
fn doc_index(
    i: &mut Interp,
) -> Option<std::rc::Rc<(Vec<u8>, std::collections::HashMap<String, usize>)>> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    thread_local! {
        static CACHE: RefCell<HashMap<String, Rc<(Vec<u8>, HashMap<String, usize>)>>> =
            RefCell::new(HashMap::new());
    }
    let dir_sym = i.q("doc-directory");
    let dir = match i.symbol_value(dir_sym) {
        Value::Str(s) => s.borrow().clone(),
        _ => return None,
    };
    let path = format!("{dir}DOC");
    CACHE.with(|c| {
        if let Some(hit) = c.borrow().get(&path) {
            return Some(hit.clone());
        }
        let bytes = std::fs::read(&path).ok()?;
        // Entries: 0x1F KIND NAME '\n' TEXT ... until next 0x1F.
        let mut map = HashMap::new();
        let mut p = 0;
        while p < bytes.len() {
            if bytes[p] != 0x1f || p + 2 >= bytes.len() {
                p += 1;
                continue;
            }
            let kind = bytes[p + 1];
            if !matches!(kind, b'F' | b'V' | b'S') {
                p += 1;
                continue;
            }
            let name_start = p + 1;
            let nl = match bytes[name_start..].iter().position(|&b| b == b'\n') {
                Some(o) => name_start + o,
                None => break,
            };
            let key = String::from_utf8_lossy(&bytes[name_start..nl]).into_owned();
            // GNU Snarf-documentation ignores placeholder entries, including
            // later duplicates, rather than replacing a real doc with SKIP.
            if !bytes[nl + 1..].starts_with(b"SKIP") {
                map.insert(key, nl + 1);
            }
            p = nl + 1;
        }
        let rc = Rc::new((bytes, map));
        c.borrow_mut().insert(path, rc.clone());
        Some(rc)
    })
}

/// Install GNU's variable documentation offsets after the startup libraries
/// have supplied their definitions, like `Snarf-documentation' in doc.c.
/// Do not intern absent platform-specific variables or document void ones.
pub(crate) fn install_variable_docs(i: &mut Interp) {
    let Some(index) = doc_index(i) else {
        return;
    };
    let prop = i.q("variable-documentation");
    for (key, &offset) in &index.1 {
        let Some(name) = key.strip_prefix('V') else {
            continue;
        };
        let Some(id) = i.obarray.intern_soft(name) else {
            continue;
        };
        if i.bound_p(id) {
            i.put_prop(id, prop, Value::Int(offset as i128));
        }
    }
}

/// DOC-entry text for KEY ("Fcar"): bytes from the recorded offset to
/// the next 0x1F separator.
pub(crate) fn doc_text(i: &mut Interp, key: &str) -> Option<String> {
    let rc = doc_index(i)?;
    let (bytes, map) = &*rc;
    let start = *map.get(key)?;
    let end = bytes[start..]
        .iter()
        .position(|&b| b == 0x1f)
        .map(|o| start + o)
        .unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
}

/// Byte offset of KEY's doc text (GNU's `function-documentation'
/// returns this integer for DOC-backed builtins).
pub(crate) fn doc_offset(i: &mut Interp, key: &str) -> Option<usize> {
    let rc = doc_index(i)?;
    rc.1.get(key).copied()
}

/// DOC-entry text starting at byte offset OFF (GNU `get_doc_string'
/// for `variable-documentation'/function cells that store raw
/// positions).
pub(crate) fn doc_text_at(i: &mut Interp, off: usize) -> Option<String> {
    let rc = doc_index(i)?;
    let (bytes, _map) = &*rc;
    if off < 2 || off >= bytes.len() || bytes[off - 1] != b'\n' {
        return None;
    }
    // GNU rejects stale DOC references rather than returning a suffix
    // from arbitrary file contents: a header is US + NAME + newline.
    let mut header = off - 2;
    while header > 0 && bytes[header] > b' ' {
        header -= 1;
    }
    if bytes[header] != 31 {
        return None;
    }
    let end = bytes[off..]
        .iter()
        .position(|&b| b == 0x1f)
        .map(|o| off + o)
        .unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[off..end]).into_owned())
}

fn f_function_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match i.indirect_function_value(&a[0]) {
        Value::Subr(s) => {
            // GNU's doc slot for a dumped builtin is the DOC offset.
            if let Some(off) = doc_offset(i, &format!("F{}", s.name)) {
                return Ok(Value::Int(off as i128));
            }
            if !s.doc.is_empty() {
                return Ok(Value::string(s.doc));
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

// ---------- coding systems ----------

/// Coding systems defined by GNU Emacs batch startup.
pub(crate) const CODING_SYSTEMS: &[&str] = &[
    "binary",
    "no-conversion",
    "undecided",
    "prefer-utf-8",
    "raw-text",
    "no-conversion-multibyte",
    "latin-1",
    "iso-8859-1",
    "iso-latin-1",
    "emacs-mule",
    "cp65001",
    "mule-utf-8",
    "utf-8",
    "utf-8-with-signature",
    "utf-8-auto",
    "utf-8-emacs",
    "utf-16le",
    "utf-16be",
    "utf-16-le",
    "utf-16le-with-signature",
    "utf-16-be",
    "utf-16be-with-signature",
    "utf-16",
    "iso-2022-7bit",
    "iso-2022-7bit-ss2",
    "iso-2022-int-1",
    "iso-2022-7bit-lock",
    "iso-2022-cjk",
    "iso-2022-7bit-lock-ss2",
    "iso-2022-8bit-ss2",
    "ctext",
    "x-ctext",
    "compound-text",
    "ctext-no-compositions",
    "ctext-with-extensions",
    "x-ctext-with-extensions",
    "compound-text-with-extensions",
    "ascii",
    "iso-safe",
    "us-ascii",
    "utf-7",
    "utf-7-imap",
    "chinese-iso-7bit",
    "iso-2022-cn",
    "iso-2022-cn-ext",
    "gb2312",
    "cn-gb",
    "euc-cn",
    "euc-china",
    "cn-gb-2312",
    "chinese-iso-8bit",
    "hz",
    "hz-gb-2312",
    "chinese-hz",
    "cp950",
    "cn-big5",
    "big5",
    "chinese-big5",
    "cn-big5-hkscs",
    "big5-hkscs",
    "chinese-big5-hkscs",
    "euc-taiwan",
    "euc-tw",
    "windows-936",
    "cp936",
    "gbk",
    "chinese-gbk",
    "gb18030",
    "chinese-gb18030",
    "iso-8859-5",
    "cyrillic-iso-8bit",
    "cp878",
    "koi8",
    "koi8-r",
    "cyrillic-koi8",
    "koi8-u",
    "alternativnyj",
    "cyrillic-alternativnyj",
    "cp866",
    "koi8-t",
    "cp1251",
    "windows-1251",
    "cp866u",
    "ruscii",
    "cp1125",
    "ibm855",
    "cp855",
    "mik",
    "pt154",
    "devanagari",
    "in-is13194-devanagari",
    "ebcdic-us",
    "ebcdic-uk",
    "cp1047",
    "ibm1047",
    "cp038",
    "ebcdic-int",
    "ibm038",
    "latin-2",
    "iso-8859-2",
    "iso-latin-2",
    "latin-3",
    "iso-8859-3",
    "iso-latin-3",
    "latin-4",
    "iso-8859-4",
    "iso-latin-4",
    "latin-5",
    "iso-8859-9",
    "iso-latin-5",
    "latin-6",
    "iso-8859-10",
    "iso-latin-6",
    "latin-7",
    "iso-8859-13",
    "iso-latin-7",
    "latin-8",
    "iso-8859-14",
    "iso-latin-8",
    "latin-0",
    "latin-9",
    "iso-8859-15",
    "iso-latin-9",
    "cp1250",
    "windows-1250",
    "cp1252",
    "windows-1252",
    "cp1254",
    "windows-1254",
    "cp1257",
    "windows-1257",
    "cp256",
    "ebcdic-int1",
    "ibm256",
    "cp273",
    "ibm273",
    "cp274",
    "ebcdic-be",
    "ibm274",
    "cp275",
    "ebcdic-br",
    "ibm275",
    "cp277",
    "ebcdic-cp-no",
    "ebcdic-cp-dk",
    "ibm277",
    "cp278",
    "ebcdic-cp-se",
    "ebcdic-cp-fi",
    "ibm278",
    "cp280",
    "ebcdic-cp-it",
    "ibm280",
    "cp284",
    "ebcdic-cp-es",
    "ibm284",
    "cp285",
    "ebcdic-cp-gb",
    "ibm285",
    "cp297",
    "ebcdic-cp-fr",
    "ibm297",
    "ibm775",
    "cp775",
    "ibm850",
    "cp850",
    "ibm852",
    "cp852",
    "ibm857",
    "cp857",
    "cp858",
    "ibm860",
    "cp860",
    "ibm861",
    "cp861",
    "ibm863",
    "cp863",
    "ibm865",
    "cp865",
    "ibm437",
    "cp437",
    "macintosh",
    "mac-roman",
    "next",
    "roman8",
    "hp-roman8",
    "adobe-standard-encoding",
    "latin-10",
    "iso-8859-16",
    "iso-latin-10",
    "iso-8859-7",
    "greek-iso-8bit",
    "cp1253",
    "windows-1253",
    "cp737",
    "ibm851",
    "cp851",
    "ibm869",
    "cp869",
    "iso-8859-8-i",
    "iso-8859-8-e",
    "iso-8859-8",
    "hebrew-iso-8bit",
    "cp1255",
    "windows-1255",
    "ibm862",
    "cp862",
    "junet",
    "iso-2022-jp",
    "iso-2022-jp-2",
    "sjis",
    "shift_jis",
    "japanese-shift-jis",
    "cp932",
    "japanese-cp932",
    "old-jis",
    "iso-2022-jp-1978-irv",
    "japanese-iso-7bit-1978-irv",
    "euc-jp",
    "euc-japan",
    "euc-japan-1990",
    "japanese-iso-8bit",
    "eucjp-ms",
    "iso-2022-jp-3",
    "iso-2022-jp-2004",
    "euc-jisx0213",
    "euc-jis-2004",
    "shift_jis-2004",
    "japanese-shift-jis-2004",
    "cp281",
    "ebcdic-jp-e",
    "ibm281",
    "cp290",
    "ebcdic-jp-kana",
    "ibm290",
    "ks_c_5601-1987",
    "euc-korea",
    "euc-kr",
    "korean-iso-8bit",
    "korean-iso-7bit-lock",
    "iso-2022-kr",
    "cp949",
    "korean-cp949",
    "lao",
    "tis-620",
    "tis620",
    "th-tis620",
    "thai-tis620",
    "ibm874",
    "cp874",
    "iso-8859-11",
    "tibetan",
    "tibetan-iso-8bit",
    "viscii",
    "vietnamese-viscii",
    "tcvn-5712",
    "tcvn",
    "vietnamese-tcvn",
    "vscii",
    "vietnamese-vscii",
    "viqr",
    "vietnamese-viqr",
    "cp1258",
    "windows-1258",
    "iso-8859-6",
    "cp1256",
    "windows-1256",
    "georgian-ps",
    "georgian-academy",
    "utf-8-nfd",
    "utf-8-hfs",
];

pub(crate) const CODING_ALIASES: &[(&str, &[&str])] = &[
    ("binary", &["no-conversion", "binary"]),
    ("no-conversion", &["no-conversion", "binary"]),
    ("undecided", &["undecided"]),
    ("prefer-utf-8", &["prefer-utf-8"]),
    ("raw-text", &["raw-text"]),
    ("no-conversion-multibyte", &["no-conversion-multibyte"]),
    ("latin-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("iso-8859-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("iso-latin-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("emacs-mule", &["emacs-mule"]),
    ("cp65001", &["utf-8", "mule-utf-8", "cp65001"]),
    ("mule-utf-8", &["utf-8", "mule-utf-8", "cp65001"]),
    ("utf-8", &["utf-8", "mule-utf-8", "cp65001"]),
    ("utf-8-with-signature", &["utf-8-with-signature"]),
    ("utf-8-auto", &["utf-8-auto"]),
    ("utf-8-emacs", &["utf-8-emacs"]),
    ("utf-16le", &["utf-16le"]),
    ("utf-16be", &["utf-16be"]),
    ("utf-16-le", &["utf-16le-with-signature", "utf-16-le"]),
    (
        "utf-16le-with-signature",
        &["utf-16le-with-signature", "utf-16-le"],
    ),
    ("utf-16-be", &["utf-16be-with-signature", "utf-16-be"]),
    (
        "utf-16be-with-signature",
        &["utf-16be-with-signature", "utf-16-be"],
    ),
    ("utf-16", &["utf-16"]),
    ("iso-2022-7bit", &["iso-2022-7bit"]),
    ("iso-2022-7bit-ss2", &["iso-2022-7bit-ss2"]),
    ("iso-2022-int-1", &["iso-2022-7bit-lock", "iso-2022-int-1"]),
    (
        "iso-2022-7bit-lock",
        &["iso-2022-7bit-lock", "iso-2022-int-1"],
    ),
    ("iso-2022-cjk", &["iso-2022-7bit-lock-ss2", "iso-2022-cjk"]),
    (
        "iso-2022-7bit-lock-ss2",
        &["iso-2022-7bit-lock-ss2", "iso-2022-cjk"],
    ),
    ("iso-2022-8bit-ss2", &["iso-2022-8bit-ss2"]),
    ("ctext", &["compound-text", "x-ctext", "ctext"]),
    ("x-ctext", &["compound-text", "x-ctext", "ctext"]),
    ("compound-text", &["compound-text", "x-ctext", "ctext"]),
    ("ctext-no-compositions", &["ctext-no-compositions"]),
    (
        "ctext-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    (
        "x-ctext-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    (
        "compound-text-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    ("ascii", &["us-ascii", "iso-safe", "ascii"]),
    ("iso-safe", &["us-ascii", "iso-safe", "ascii"]),
    ("us-ascii", &["us-ascii", "iso-safe", "ascii"]),
    ("utf-7", &["utf-7"]),
    ("utf-7-imap", &["utf-7-imap"]),
    ("chinese-iso-7bit", &["iso-2022-cn", "chinese-iso-7bit"]),
    ("iso-2022-cn", &["iso-2022-cn", "chinese-iso-7bit"]),
    ("iso-2022-cn-ext", &["iso-2022-cn-ext"]),
    (
        "gb2312",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "cn-gb",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "euc-cn",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "euc-china",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "cn-gb-2312",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "chinese-iso-8bit",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    ("hz", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("hz-gb-2312", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("chinese-hz", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("cp950", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    ("cn-big5", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    ("big5", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    (
        "chinese-big5",
        &["chinese-big5", "big5", "cn-big5", "cp950"],
    ),
    (
        "cn-big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    (
        "big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    (
        "chinese-big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    ("euc-taiwan", &["euc-tw", "euc-taiwan"]),
    ("euc-tw", &["euc-tw", "euc-taiwan"]),
    (
        "windows-936",
        &["chinese-gbk", "gbk", "cp936", "windows-936"],
    ),
    ("cp936", &["chinese-gbk", "gbk", "cp936", "windows-936"]),
    ("gbk", &["chinese-gbk", "gbk", "cp936", "windows-936"]),
    (
        "chinese-gbk",
        &["chinese-gbk", "gbk", "cp936", "windows-936"],
    ),
    ("gb18030", &["chinese-gb18030", "gb18030"]),
    ("chinese-gb18030", &["chinese-gb18030", "gb18030"]),
    ("iso-8859-5", &["cyrillic-iso-8bit", "iso-8859-5"]),
    ("cyrillic-iso-8bit", &["cyrillic-iso-8bit", "iso-8859-5"]),
    ("cp878", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    ("koi8", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    ("koi8-r", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    (
        "cyrillic-koi8",
        &["cyrillic-koi8", "koi8-r", "koi8", "cp878"],
    ),
    ("koi8-u", &["koi8-u"]),
    (
        "alternativnyj",
        &["cyrillic-alternativnyj", "alternativnyj"],
    ),
    (
        "cyrillic-alternativnyj",
        &["cyrillic-alternativnyj", "alternativnyj"],
    ),
    ("cp866", &["cp866"]),
    ("koi8-t", &["koi8-t"]),
    ("cp1251", &["windows-1251", "cp1251"]),
    ("windows-1251", &["windows-1251", "cp1251"]),
    ("cp866u", &["cp1125", "ruscii", "cp866u"]),
    ("ruscii", &["cp1125", "ruscii", "cp866u"]),
    ("cp1125", &["cp1125", "ruscii", "cp866u"]),
    ("ibm855", &["cp855", "ibm855"]),
    ("cp855", &["cp855", "ibm855"]),
    ("mik", &["mik"]),
    ("pt154", &["pt154"]),
    ("devanagari", &["in-is13194-devanagari", "devanagari"]),
    (
        "in-is13194-devanagari",
        &["in-is13194-devanagari", "devanagari"],
    ),
    ("ebcdic-us", &["ebcdic-us"]),
    ("ebcdic-uk", &["ebcdic-uk"]),
    ("cp1047", &["ibm1047", "cp1047"]),
    ("ibm1047", &["ibm1047", "cp1047"]),
    ("cp038", &["ibm038", "ebcdic-int", "cp038"]),
    ("ebcdic-int", &["ibm038", "ebcdic-int", "cp038"]),
    ("ibm038", &["ibm038", "ebcdic-int", "cp038"]),
    ("latin-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("iso-8859-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("iso-latin-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("latin-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("iso-8859-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("iso-latin-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("latin-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("iso-8859-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("iso-latin-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("latin-5", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("iso-8859-9", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("iso-latin-5", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("latin-6", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("iso-8859-10", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("iso-latin-6", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("latin-7", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("iso-8859-13", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("iso-latin-7", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("latin-8", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    ("iso-8859-14", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    ("iso-latin-8", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    (
        "latin-0",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "latin-9",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "iso-8859-15",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "iso-latin-9",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    ("cp1250", &["windows-1250", "cp1250"]),
    ("windows-1250", &["windows-1250", "cp1250"]),
    ("cp1252", &["windows-1252", "cp1252"]),
    ("windows-1252", &["windows-1252", "cp1252"]),
    ("cp1254", &["windows-1254", "cp1254"]),
    ("windows-1254", &["windows-1254", "cp1254"]),
    ("cp1257", &["windows-1257", "cp1257"]),
    ("windows-1257", &["windows-1257", "cp1257"]),
    ("cp256", &["ibm256", "ebcdic-int1", "cp256"]),
    ("ebcdic-int1", &["ibm256", "ebcdic-int1", "cp256"]),
    ("ibm256", &["ibm256", "ebcdic-int1", "cp256"]),
    ("cp273", &["ibm273", "cp273"]),
    ("ibm273", &["ibm273", "cp273"]),
    ("cp274", &["ibm274", "ebcdic-be", "cp274"]),
    ("ebcdic-be", &["ibm274", "ebcdic-be", "cp274"]),
    ("ibm274", &["ibm274", "ebcdic-be", "cp274"]),
    ("cp275", &["ibm275", "ebcdic-br", "cp275"]),
    ("ebcdic-br", &["ibm275", "ebcdic-br", "cp275"]),
    ("ibm275", &["ibm275", "ebcdic-br", "cp275"]),
    (
        "cp277",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ebcdic-cp-no",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ebcdic-cp-dk",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ibm277",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "cp278",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ebcdic-cp-se",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ebcdic-cp-fi",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ibm278",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    ("cp280", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("ebcdic-cp-it", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("ibm280", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("cp284", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("ebcdic-cp-es", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("ibm284", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("cp285", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("ebcdic-cp-gb", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("ibm285", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("cp297", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ebcdic-cp-fr", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ibm297", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ibm775", &["cp775", "ibm775"]),
    ("cp775", &["cp775", "ibm775"]),
    ("ibm850", &["cp850", "ibm850"]),
    ("cp850", &["cp850", "ibm850"]),
    ("ibm852", &["cp852", "ibm852"]),
    ("cp852", &["cp852", "ibm852"]),
    ("ibm857", &["cp857", "ibm857"]),
    ("cp857", &["cp857", "ibm857"]),
    ("cp858", &["cp858"]),
    ("ibm860", &["cp860", "ibm860"]),
    ("cp860", &["cp860", "ibm860"]),
    ("ibm861", &["cp861", "ibm861"]),
    ("cp861", &["cp861", "ibm861"]),
    ("ibm863", &["cp863", "ibm863"]),
    ("cp863", &["cp863", "ibm863"]),
    ("ibm865", &["cp865", "ibm865"]),
    ("cp865", &["cp865", "ibm865"]),
    ("ibm437", &["cp437", "ibm437"]),
    ("cp437", &["cp437", "ibm437"]),
    ("macintosh", &["mac-roman", "macintosh"]),
    ("mac-roman", &["mac-roman", "macintosh"]),
    ("next", &["next"]),
    ("roman8", &["hp-roman8", "roman8"]),
    ("hp-roman8", &["hp-roman8", "roman8"]),
    ("adobe-standard-encoding", &["adobe-standard-encoding"]),
    ("latin-10", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-8859-16", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-latin-10", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-8859-7", &["greek-iso-8bit", "iso-8859-7"]),
    ("greek-iso-8bit", &["greek-iso-8bit", "iso-8859-7"]),
    ("cp1253", &["windows-1253", "cp1253"]),
    ("windows-1253", &["windows-1253", "cp1253"]),
    ("cp737", &["cp737"]),
    ("ibm851", &["cp851", "ibm851"]),
    ("cp851", &["cp851", "ibm851"]),
    ("ibm869", &["cp869", "ibm869"]),
    ("cp869", &["cp869", "ibm869"]),
    (
        "iso-8859-8-i",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "iso-8859-8-e",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "iso-8859-8",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "hebrew-iso-8bit",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    ("cp1255", &["windows-1255", "cp1255"]),
    ("windows-1255", &["windows-1255", "cp1255"]),
    ("ibm862", &["cp862", "ibm862"]),
    ("cp862", &["cp862", "ibm862"]),
    ("junet", &["iso-2022-jp", "junet"]),
    ("iso-2022-jp", &["iso-2022-jp", "junet"]),
    ("iso-2022-jp-2", &["iso-2022-jp-2"]),
    ("sjis", &["japanese-shift-jis", "shift_jis", "sjis"]),
    ("shift_jis", &["japanese-shift-jis", "shift_jis", "sjis"]),
    (
        "japanese-shift-jis",
        &["japanese-shift-jis", "shift_jis", "sjis"],
    ),
    ("cp932", &["japanese-cp932", "cp932"]),
    ("japanese-cp932", &["japanese-cp932", "cp932"]),
    (
        "old-jis",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "iso-2022-jp-1978-irv",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "japanese-iso-7bit-1978-irv",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "euc-jp",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "euc-japan",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "euc-japan-1990",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "japanese-iso-8bit",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    ("eucjp-ms", &["eucjp-ms"]),
    ("iso-2022-jp-3", &["iso-2022-jp-2004", "iso-2022-jp-3"]),
    ("iso-2022-jp-2004", &["iso-2022-jp-2004", "iso-2022-jp-3"]),
    ("euc-jisx0213", &["euc-jis-2004", "euc-jisx0213"]),
    ("euc-jis-2004", &["euc-jis-2004", "euc-jisx0213"]),
    (
        "shift_jis-2004",
        &["japanese-shift-jis-2004", "shift_jis-2004"],
    ),
    (
        "japanese-shift-jis-2004",
        &["japanese-shift-jis-2004", "shift_jis-2004"],
    ),
    ("cp281", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("ebcdic-jp-e", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("ibm281", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("cp290", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    ("ebcdic-jp-kana", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    ("ibm290", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    (
        "ks_c_5601-1987",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "euc-korea",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "euc-kr",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "korean-iso-8bit",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "korean-iso-7bit-lock",
        &["iso-2022-kr", "korean-iso-7bit-lock"],
    ),
    ("iso-2022-kr", &["iso-2022-kr", "korean-iso-7bit-lock"]),
    ("cp949", &["korean-cp949", "cp949"]),
    ("korean-cp949", &["korean-cp949", "cp949"]),
    ("lao", &["lao"]),
    (
        "tis-620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    ("tis620", &["thai-tis620", "th-tis620", "tis620", "tis-620"]),
    (
        "th-tis620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    (
        "thai-tis620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    ("ibm874", &["cp874", "ibm874"]),
    ("cp874", &["cp874", "ibm874"]),
    ("iso-8859-11", &["iso-8859-11"]),
    ("tibetan", &["tibetan-iso-8bit", "tibetan"]),
    ("tibetan-iso-8bit", &["tibetan-iso-8bit", "tibetan"]),
    ("viscii", &["vietnamese-viscii", "viscii"]),
    ("vietnamese-viscii", &["vietnamese-viscii", "viscii"]),
    (
        "tcvn-5712",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "tcvn",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vietnamese-tcvn",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vscii",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vietnamese-vscii",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    ("viqr", &["vietnamese-viqr", "viqr"]),
    ("vietnamese-viqr", &["vietnamese-viqr", "viqr"]),
    ("cp1258", &["windows-1258", "cp1258"]),
    ("windows-1258", &["windows-1258", "cp1258"]),
    ("iso-8859-6", &["iso-8859-6"]),
    ("cp1256", &["windows-1256", "cp1256"]),
    ("windows-1256", &["windows-1256", "cp1256"]),
    ("georgian-ps", &["georgian-ps"]),
    ("georgian-academy", &["georgian-academy"]),
    ("utf-8-nfd", &["utf-8-hfs", "utf-8-nfd"]),
    ("utf-8-hfs", &["utf-8-hfs", "utf-8-nfd"]),
    // GNU: `define-coding-system-alias 'emacs-internal 'utf-8-emacs-unix'
    // — a pure alias, so absent from `coding-system-list' but known to
    // `coding-system-p'.
    ("emacs-internal", &["utf-8-emacs-unix", "emacs-internal"]),
];

pub(crate) fn coding_known(i: &Interp, v: &Value) -> Option<String> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    // Registered subsidiaries remain valid even if a parent alias is
    // later redirected to a coding system with a fixed EOL type.
    if crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&name.as_str()) {
        return Some(name);
    }
    // Explicit alias names are arbitrary symbols, even when their spelling
    // ends in an EOL suffix whose apparent base is not a coding system.
    if i.extra_coding_aliases.iter().any(|(alias, _)| *alias == name) {
        return Some(name);
    }
    let base = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    // Only automatic-EOL parents own subsidiaries. Aliases of a fixed
    // subsidiary (emacs-internal -> utf-8-emacs-unix) do not gain them.
    let mut parent = base.to_string();
    for _ in 0..32 {
        if let Some((_, target)) = i.extra_coding_aliases.iter().find(|(n, _)| *n == parent) {
            parent = target.clone();
        } else {
            break;
        }
    }
    if let Some((_, group)) = CODING_ALIASES.iter().find(|(n, _)| *n == parent) {
        parent = group[0].to_string();
    }
    let eol_ok = base == name.as_str()
        || (eol_split(&parent).1.is_none()
            && !matches!(parent.as_str(), "no-conversion" | "unix" | "dos" | "mac")
            && (CODING_SYSTEMS.contains(&base)
                || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&base)
                || i.extra_coding_systems.iter().any(|n| *n == base)));
    if eol_ok
        && (CODING_SYSTEMS.contains(&name.as_str())
            || CODING_SYSTEMS.contains(&base)
            || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&name.as_str())
            || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&base)
            || CODING_ALIASES.iter().any(|&(n, _)| n == name)
            || i.extra_coding_systems.iter().any(|n| *n == name)
            || i.extra_coding_aliases.iter().any(|(n, _)| *n == name))
    {
        Some(name)
    } else {
        None
    }
}

/// GNU Fcheck_coding_system's two-stage check used by every
/// `coding-system-*` subr: a non-symbol argument signals
/// `wrong-type-argument symbolp'; a symbol naming no coding system
/// signals `coding-system-error'.  Returns the canonical name.
pub(crate) fn check_coding(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Sym(_) => match coding_known(i, v) {
            Some(n) => Ok(n),
            None => {
                let cs = i.q("coding-system-error");
                Err(i.signal_data(cs, vec![v.clone()]))
            }
        },
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_coding_system_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU (mule.el): iterate the `coding-system-list' VARIABLE, keep
    // members that are their own base, and (unless BASE-ONLY) expand
    // each through `coding-system-aliases'; `cons'ing accumulates the
    // reverse of the variable order.  The Lisp defun shadows this subr
    // once mule.el loads — keep both implementations consistent.
    let base_only = arg(&a, 0).truthy();
    let csl = i.q("coding-system-list");
    let members = i.symbol_value(csl).list_to_vec().unwrap_or_default();
    let mut codings: Vec<Value> = Vec::new();
    for member in members {
        let Value::Sym(_) = member else { continue };
        let base = f_coding_system_base(i, vec![member.clone()].into())?;
        if !eq_values(&base, &member) {
            continue;
        }
        if base_only {
            codings.push(member);
            continue;
        }
        match coding_alias_group(i, &member) {
            Some(group) if !group.is_empty() => {
                for n in group {
                    codings.push(Value::Sym(i.intern(&n)));
                }
            }
            _ => codings.push(member),
        }
    }
    codings.reverse();
    Ok(Value::list(codings))
}
fn f_coding_system_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        a[0].is_nil() || coding_known(i, &a[0]).is_some(),
    ))
}

fn f_coding_system_priority_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU builds this from the per-category priority table; we keep
    // the same list in `remacs-coding-system-priorities', seeded to
    // GNU's -Q order and updated by `set-coding-system-priority'.
    let var = i.q("remacs-coding-system-priorities");
    let list = i.symbol_value(var);
    if arg(&a, 0).truthy() {
        if let Value::Cons(c) = &list {
            return Ok(c.borrow().car.clone());
        }
    }
    Ok(list)
}

fn f_check_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil is accepted (returned as-is), non-symbols signal
    // `wrong-type-argument symbolp', unknown symbols signal
    // `coding-system-error' (verified against 31.1).
    match &a[0] {
        Value::Nil => return Ok(Value::Nil),
        Value::Sym(_) => {}
        v => return Err(i.wrong_type_mut("symbolp", v)),
    }
    match coding_known(i, &a[0]) {
        Some(_) => Ok(a[0].clone()),
        None => {
            let s = i.q("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

/// Strip a `-unix'/`-dos'/`-mac' EOL suffix, returning the base name
/// and variant index (0/1/2); None when there's no suffix.
fn eol_split(name: &str) -> (&str, Option<usize>) {
    for (suf, idx) in [("-unix", 0usize), ("-dos", 1), ("-mac", 2)] {
        if let Some(b) = name.strip_suffix(suf) {
            return (b, Some(idx));
        }
    }
    (name, None)
}

/// The alias group (canonical name first) for a known coding-system
/// name, resolving EOL variants to their base first; runtime aliases
/// defined via `define-coding-system-alias' join their target's group
/// appended (GNU extends the group's alias list).
pub(crate) fn coding_alias_group(i: &Interp, v: &Value) -> Option<Vec<String>> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    // GNU's model: every coding system X owns its `aliases' attribute
    // — a list whose head is X itself.  `define-coding-system-alias'
    // appends the alias to the *resolved* system's list, so `unix'
    // (→ undecided-unix) lands in `undecided-unix's group, never in
    // `undecided's.
    let resolve_target = |i: &Interp, n: &str| -> String {
        let mut cur = n.to_string();
        for _ in 0..16 {
            match i.extra_coding_aliases.iter().find(|(x, _)| *x == cur) {
                Some((_, t)) => cur = t.clone(),
                None => break,
            }
        }
        cur
    };
    // The canonical system NAME resolves to (base-group member or
    // custom system), plus the group that head owns.
    let name = resolve_target(i, &name);
    let head = {
        let (nb, idx) = eol_split(&name);
        let base_res = resolve_target(i, nb);
        match idx {
            Some(_) => format!("{base_res}-{}", ["unix", "dos", "mac"][idx.unwrap()]),
            None => {
                // Canonical-first: an alias that is itself a group
                // head (no-conversion) wins over appearing inside
                // another group's member list (binary's row).
                if let Some(&(_, group)) = CODING_ALIASES
                    .iter()
                    .find(|&&(n, _)| n == base_res)
                    .or_else(|| {
                        CODING_ALIASES
                            .iter()
                            .find(|&&(_, grp)| grp.iter().any(|m| *m == base_res))
                    })
                {
                    group[0].to_string()
                } else {
                    base_res
                }
            }
        }
    };
    let (hbase, hidx) = eol_split(&head);
    let group: Vec<String> = match hidx {
        // Subsidiary: base's own group with this eol suffix applied,
        // e.g. utf-8-unix → (utf-8-unix mule-utf-8-unix cp65001-unix).
        Some(e) => {
            let hg = CODING_ALIASES
                .iter()
                .find(|&&(n, grp)| n == hbase || grp.iter().any(|m| *m == hbase));
            let suf = ["unix", "dos", "mac"][e];
            match hg {
                // `no-conversion'/`binary' (eol-type nil) have no
                // eol variants at all — `binary-dos' etc. error.
                Some(&(_, base_group)) if hbase != "no-conversion" => {
                    base_group.iter().map(|m| format!("{m}-{suf}")).collect()
                }
                _ => {
                    if i.extra_coding_systems.iter().any(|n| *n == hbase) {
                        vec![head.clone()]
                    } else {
                        return None;
                    }
                }
            }
        }
        None => {
            if let Some(&(_, g)) = CODING_ALIASES
                .iter()
                .find(|&&(n, _)| n == head)
                .or_else(|| {
                    CODING_ALIASES
                        .iter()
                        .find(|&&(_, grp)| grp.iter().any(|m| *m == head))
                })
            {
                g.iter().map(|s| s.to_string()).collect()
            } else if i.extra_coding_systems.iter().any(|n| *n == head) {
                vec![head.clone()]
            } else {
                return None;
            }
        }
    };
    // Fold in runtime aliases that resolve to this exact head —
    // GNU appends them to the resolved system's `aliases' list.
    let mut g = group;
    for (a, t) in &i.extra_coding_aliases {
        if resolve_target(i, t) == head && !g.iter().any(|s| s == a) {
            g.push(a.clone());
        }
    }
    Some(g)
}

fn f_coding_system_eol_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil means the default (unix ⇒ 0); a non-coding-system or
    // non-symbol argument returns nil rather than signaling.
    match &a[0] {
        Value::Nil => return Ok(Value::Int(0)),
        Value::Sym(_) => {}
        _ => return Ok(Value::Nil),
    };
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    if coding_known(i, &a[0]).is_none() {
        return Ok(Value::Nil);
    }
    if let Some(idx) = coding_fixed_eol(i, &a[0]) {
        return Ok(Value::Int(idx as i128));
    }
    let base = coding_canonical(i, &a[0]);
    // Base system: vector of the three EOL variants.
    Ok(Value::vector(
        ["unix", "dos", "mac"]
            .iter()
            .map(|s| Value::Sym(i.intern(&format!("{base}-{s}"))))
            .collect(),
    ))
}
fn f_coding_system_aliases(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks CHECK_SYMBOL
    // before consulting the coding-system table.
    // GNU: nil means the no-conversion coding system.
    if a[0].is_nil() {
        return Ok(Value::list(vec![
            Value::Sym(i.q("no-conversion")),
            Value::Sym(i.q("binary")),
        ]));
    }
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    match coding_alias_group(i, &a[0]) {
        Some(group) => Ok(Value::list(
            group.iter().map(|n| Value::Sym(i.intern(n))).collect(),
        )),
        // GNU signals `coding-system-error' on unknown names.
        None => {
            let s = i.q("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

fn coding_base_extra(i: &Interp, base: &str) -> String {
    // Runtime aliases (`define-coding-system-alias') resolve through
    // their target, recursively, to the static group head or a custom
    // system's own name.
    let mut cur = base.to_string();
    for _ in 0..16 {
        match i.extra_coding_aliases.iter().find(|(x, _)| *x == cur) {
            Some((_, t)) => {
                let (tb, _) = eol_split(t);
                cur = tb.to_string();
            }
            None => break,
        }
    }
    cur
}

fn f_coding_system_base(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        // GNU treats nil as `no-conversion'.
        Value::Nil => "no-conversion".to_string(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    // Validate the original name before stripping a suffix or resolving
    // an alias group, which could otherwise make an unknown name valid.
    if !a[0].is_nil() && coding_known(i, &a[0]).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    if i.extra_coding_aliases.iter().any(|(alias, _)| *alias == name) {
        let base = coding_canonical(i, &a[0]);
        return Ok(Value::Sym(i.intern(&base)));
    }
    // GNU: the bare EOL names `unix'/`dos'/`mac' are subsidiaries of
    // `undecided' (its eol-type vector), not self-based.
    if matches!(name.as_str(), "unix" | "dos" | "mac") {
        return Ok(Value::Sym(i.q("undecided")));
    }
    // Subsidiaries resolve to the base coding system, and alias names
    // resolve to their group's canonical head: GNU maps `cp1256-dos' →
    // `windows-1256', `iso-8859-1' → `iso-latin-1', `mule-utf-8' → `utf-8'.
    let stripped = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    let resolved = coding_base_extra(i, stripped);
    let base: &str = &resolved;
    if let Some(&(_, group)) = CODING_ALIASES
        .iter()
        .find(|&&(n, grp)| n == base || grp.iter().any(|m| *m == base))
    {
        return Ok(Value::Sym(i.intern(group[0])));
    }
    Ok(Value::Sym(i.intern(base)))
}

pub(crate) fn f_coding_system_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU stores per-coding-system metadata on the coding-system
    // symbol's plist; we mirror it via the generated
    // `remacs-coding-system-plists' alist keyed by the queried name.
    if i.sym_id(&a[0]).is_none() {
        // GNU treats nil as `no-conversion'.
        if !a[0].is_nil() {
            return Err(i.wrong_type_mut("symbolp", &a[0]));
        }
    }
    let q = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    let Some(_) = coding_known(i, &q) else {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    };
    let table_id = i.q("remacs-coding-system-plists");
    if let Value::Cons(_) = i.symbol_value(table_id) {
        // EOL variants share the base system's plist — GNU's
        // coding-system objects for utf-8-unix/-dos/-mac all carry
        // utf-8's attributes.
        let base = coding_canonical(i, &q);
        let key = Value::Sym(i.intern(&base));
        if let Some(entry) = assq(&i.symbol_value(table_id), &key) {
            if let Value::Cons(pair) = &entry {
                let (_, val) = {
                    let pb = pair.borrow();
                    (pb.car.clone(), pb.cdr.clone())
                };
                return Ok(val);
            }
        }
        return Ok(Value::Nil);
    }
    Ok(Value::Nil)
}

/// `(assq KEY ALIST)' on raw Values.
fn assq(alist: &Value, key: &Value) -> Option<Value> {
    let mut cur = alist.clone();
    while let Value::Cons(c) = cur {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(pair) = &car {
            let (k, _) = {
                let b = pair.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let (Value::Sym(a), Value::Sym(b)) = (&k, key) {
                if a == b {
                    return Some(car.clone());
                }
            }
        }
        cur = cdr;
    }
    None
}

fn f_coding_system_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prop = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return Ok(Value::Nil),
    };
    let cs = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    match (coding_known(i, &cs), prop.as_str()) {
        (Some(n), ":name") => Ok(Value::Sym(i.intern(&n))),
        (Some(n), ":coding-type") => Ok(Value::Sym(i.intern(if n.starts_with("utf-8") {
            "utf-8"
        } else {
            "charset"
        }))),
        (Some(_), ":eol-type") => f_coding_system_eol_type(i, a),
        (Some(_), _) => {
            let plist = f_coding_system_plist(i, vec![a[0].clone()].into())?;
            let mut cur = plist;
            while let Value::Cons(c) = cur {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let (Value::Sym(k), Some(q)) = (&k, i.sym_id(&a[1])) {
                    if *k == q {
                        if let Value::Cons(v) = rest {
                            return Ok(v.borrow().car.clone());
                        }
                        return Ok(Value::Nil);
                    }
                }
                cur = match rest {
                    Value::Cons(c2) => c2.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

/// GNU's `add-to-coding-system-list' (mule.el): sorted insert of NAME
/// into the `coding-system-list' variable using `coding-system-lessp'
/// (mnemonic-char ordering with no-conversion/emacs-mule/undecided
/// special-cased), inserting AFTER equal keys — subsidiaries land in
/// parent, mac, dos, unix order when registered that way.  Falls back
/// to a plain append while the Lisp defun isn't bound yet (pre-mule
/// init) or for names without a mnemonic.  No-op when already a member.
pub(crate) fn add_to_coding_system_list(i: &mut Interp, name: &str) -> EvalResult {
    let sid = i.intern(name);
    let csl = i.q("coding-system-list");
    let cur = i.symbol_value(csl).list_to_vec().unwrap_or_default();
    if cur.iter().any(|v| matches!(v, Value::Sym(s) if *s == sid)) {
        return Ok(Value::Nil);
    }
    // Prefer the real Lisp function (mule.el loads it early): it knows
    // mnemonics through `coding-system-plist'.
    let add = i.q("add-to-coding-system-list");
    let fbound = !matches!(
        i.symbol_function(add),
        Value::Sym(s) if s == crate::lisp::sym::UNBOUND
    );
    if fbound {
        i.apply(&Value::Sym(add), vec![Value::Sym(sid)])?;
        return Ok(Value::Nil);
    }
    let mut items = cur;
    items.push(Value::Sym(sid));
    i.set_symbol(csl, Value::list(items))?;
    Ok(Value::Nil)
}

fn f_coding_system_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: SYMBOLP checks CS then PROP; unknown CS → coding-system-error;
    // Existing properties update in place; new properties append.
    let cs = match &a[0] {
        Value::Sym(_) => a[0].clone(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if i.sym_id(&a[1]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
    if coding_known(i, &cs).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![cs]));
    }
    let table_id = i.q("remacs-coding-system-plists");
    let key = {
        let sid = i.sym_id(&a[0]).unwrap();
        let resolved = coding_base_extra(i, &i.symbol_name(sid).to_string());
        let (base, _) = eol_split(&resolved);
        Value::Sym(i.intern(base))
    };
    let mut cur = i.symbol_value(table_id);
    loop {
        let (car, cdr) = match &cur {
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            _ => break,
        };
        if let Value::Cons(pair) = &car {
            let (pk, pl) = {
                let pb = pair.borrow();
                (pb.car.clone(), pb.cdr.clone())
            };
            if eq_values(&pk, &key) {
                pair.borrow_mut().cdr = crate::lisp::eval::plist_put_v(&pl, &a[1], a[2].clone());
                return Ok(a[2].clone());
            }
        }
        cur = cdr;
    }
    // Known coding system with no plist entry yet — create one.
    let entry = Value::cons(
        key,
        Value::cons(a[1].clone(), Value::cons(a[2].clone(), Value::Nil)),
    );
    let new_alist = Value::cons(entry, i.symbol_value(table_id));
    i.set_symbol(table_id, new_alist)?;
    Ok(a[2].clone())
}

fn f_terminal_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Optional TERMINAL is terminal-live-p-checked.
    if let Some(v) = a.first() {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(i.terminal_coding.clone())
}

fn f_keyboard_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(i.keyboard_coding.clone())
}

/// `window-at-side-p' — GNU's plain "N is not a valid window" error
/// for a bad window, then SIDE nil/left/top/right/bottom selects the
/// edge; anything else fails GNU's side decode with
/// `wrong-type-argument integerp nil'.
fn f_window_at_side_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el: WINDOW's SIDE edge equals the same edge of the
    // frame's root window.  Our flat model has no root window object;
    // the root extent is (0,0,frame-width,minibuffer-top) — i.e. the
    // frame minus the echo area.
    let w = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i).ok_or_else(|| i.error("no selected window"))?,
        Value::Window(w) => w,
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let side = match arg(&a, 1) {
        Value::Nil => 3,
        Value::Sym(s) => match i.symbol_name(s).as_str() {
            "left" => 0,
            "top" => 1,
            "right" => 2,
            "bottom" => 3,
            _ => return Err(i.wrong_type_mut("integerp", &Value::Nil)),
        },
        _ => return Err(i.wrong_type_mut("integerp", &Value::Nil)),
    };
    let wid = w.borrow().id;
    let frame = i.frames.iter().find(|f| {
        f.borrow().windows.iter().any(|w2| w2.borrow().id == wid)
            || f.borrow()
                .minibuffer
                .as_ref()
                .map(|m| m.borrow().id == wid)
                .unwrap_or(false)
    });
    let Some(frame) = frame.cloned() else {
        return Ok(Value::Nil);
    };
    let (root_l, root_t, root_r, root_b) = {
        let f = frame.borrow();
        let mini_top = f
            .minibuffer
            .as_ref()
            .map(|m| m.borrow().top as i128)
            .unwrap_or(f.height as i128);
        // Root top = topmost content edge (0 unsplit, 1 after GNU's
        // menu-bar row materializes on the first split).
        let top = f
            .windows
            .iter()
            .map(|w| w.borrow().top as i128)
            .min()
            .unwrap_or(0);
        (0i128, top, f.width as i128, mini_top)
    };
    let (wl, wt, wr, wb) = {
        let b = w.borrow();
        (
            b.left as i128,
            b.top as i128,
            (b.left + b.width) as i128,
            (b.top + b.height) as i128,
        )
    };
    let at = match side {
        0 => wl == root_l,
        1 => wt == root_t,
        2 => wr == root_r,
        _ => wb == root_b,
    };
    Ok(Value::from_bool(at))
}

/// `window-font-width' / `window-font-height' — 1 (char cells) on a
/// tty; GNU uses the plain "N is not a live window" error here.
fn f_window_font_metric(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Int(1)),
        other => {
            let shown = i.princ_to_string(&other);
            Err(i.error(format!("{shown} is not a live window")))
        }
    }
}

/// `window-cursor-info' — nil on tty; a typed window-live-p check.
fn f_window_cursor_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("window-live-p", &other)),
    }
}

/// `set-input-meta-mode' — META arg is ignored on our model; GNU
/// checks the optional TERMINAL with terminal-live-p.
fn f_set_input_meta_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 1) {
        Value::Nil => Ok(Value::Nil),
        w if crate::editor::is_terminal(i, &w) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
}

/// Shared body for `set-terminal-coding-system' /
/// `set-keyboard-coding-system'.  GNU `set-terminal-coding-system'
/// decodes the optional TERMINAL arg FIRST (terminal-live-p on a bad
/// arg fires before the CODING-SYSTEM checks); the keyboard variant
/// checks its CODING-SYSTEM arg first.
fn set_coding_system(i: &mut Interp, a: crate::lisp::Argv, ret_name: bool) -> EvalResult {
    if !ret_name {
        match arg(&a, 1) {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, &w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
        }
    }
    match &a[0] {
        // GNU: nil resets — terminal to nil, keyboard to no-conversion;
        // both return nil.
        Value::Nil => {
            if ret_name {
                i.keyboard_coding = Value::Sym(i.q("no-conversion"));
            } else {
                i.terminal_coding = Value::Nil;
            }
            return Ok(Value::Nil);
        }
        Value::Sym(_) => {
            if i.sym_id(&a[0]).is_none() {
                return Err(i.wrong_type_mut("symbolp", &a[0]));
            }
            if coding_known(i, &a[0]).is_none() {
                let s = i.q("coding-system-error");
                return Err(i.signal_data(s, vec![a[0].clone()]));
            }
        }
        other => return Err(i.wrong_type_mut("symbolp", other)),
    }
    match arg(&a, 1) {
        Value::Nil => {}
        w if crate::editor::is_terminal(i, &w) => {}
        other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
    if !ret_name {
        // `set-terminal-coding-system' stores the given name and
        // returns nil.
        i.terminal_coding = a[0].clone();
        return Ok(Value::Nil);
    }
    // GNU's keyboard coding canonicalizes aliases and gains the
    // platform EOL suffix.
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => unreachable!(),
    };
    let canon = coding_alias_group(i, &a[0])
        .and_then(|g| g.first().cloned())
        .unwrap_or(name);
    let bare = canon
        .strip_suffix("-unix")
        .or_else(|| canon.strip_suffix("-dos"))
        .or_else(|| canon.strip_suffix("-mac"))
        .is_some();
    let out = if bare { canon } else { format!("{canon}-unix") };
    let sym = Value::Sym(i.intern(&out));
    i.keyboard_coding = sym.clone();
    Ok(sym)
}

/// `gui-get-selection' — GNU's Lisp definition calls
/// `gui-selection-exists-p', which is void in batch; the
/// void-function error propagates.
fn f_gui_get_selection(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let f = Value::Sym(i.q("gui-selection-exists-p"));
    i.call_function(&f, &Value::Nil, None)
}

/// `x-create-frame' on an NS build: GNU's nsterm reports that
/// Nextstep windows are unavailable.
fn f_ns_frame_stub_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `x-create-frame-with-faces' CHECK_LIST(PARMS) before the
    // NS window-system check fires, so a non-list PARMS reports
    // `wrong-type-argument listp' rather than the Nextstep error.
    let _ = want_list(i, &a[0])?;
    Err(i.error("Nextstep windows are not in use or not initialized"))
}

/// `x-select-font' on an NS build: GNU decodes the optional FRAME
/// with decode_live_frame first (frame-live-p), then the NS font
/// panel requires a window-system frame, so batch signals
/// `window-system-frame'.
fn f_select_font_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live(i, &arg(&a, 0))?;
    Err(i.error("Window system frame should be used"))
}

fn f_set_terminal_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    set_coding_system(i, a, false)
}

fn f_set_keyboard_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's mule.el Lisp defun: check the coding type for suitability,
    // save/restore the meta-mode flag around it, change the EOL
    // conversion to unix, then run the internal setter and mirror the
    // result into the `keyboard-coding-system' variable.
    if let Some(v) = a.get(1) {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    if !matches!(a[0], Value::Sym(_) | Value::Nil) {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    // nil means "disable decoding" → raw-text type → accept-8-bit.
    let cs_val = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    if coding_known(i, &cs_val).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![cs_val.clone()]));
    }
    let coding_type = crate::lisp::builtins::charset::f_coding_system_type(
        i,
        vec![cs_val.clone()].into(),
    )?;
    let type_name = match &coding_type {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => String::new(),
    };
    let accept_8_bit = if [
        "raw-text", "charset", "utf-8", "shift-jis", "big5", "ccl",
    ]
    .contains(&type_name.as_str())
    {
        true
    } else if type_name == "iso-2022" {
        // `(or (memq '7-bit flags) (setq accept-8-bit t))'
        let flags_prop = i.q(":flags");
        let flags = f_coding_system_get(
            i,
            vec![cs_val.clone(), Value::Sym(flags_prop)].into(),
        )?;
        !flags
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .any(|v| matches!(v, Value::Sym(s) if i.symbol_name(*s) == "7-bit"))
    } else {
        let name = match &a[0] {
            Value::Sym(s) => i.symbol_name(*s).to_string(),
            _ => "no-conversion".to_string(),
        };
        return Err(i.error(format!(
            "Unsupported coding system for keyboard: {name}"
        )));
    };
    let pkey = i.q("keyboard-coding-saved-meta-mode");
    let saved_meta = i
        .terminal_params
        .iter()
        .find(|(k, _)| matches!(k, Value::Sym(s) if *s == pkey))
        .map(|(_, v)| v.clone())
        .unwrap_or(Value::Nil);
    if accept_8_bit {
        if saved_meta.is_nil() {
            // Save `(nth 2 (current-input-mode))' — the meta flag —
            // under the saved-meta parameter; set meta-mode 8.
            let meta_flag = Value::t();
            i.terminal_params
                .retain(|(k, _)| !matches!(k, Value::Sym(s) if *s == pkey));
            i.terminal_params
                .push((Value::Sym(pkey), Value::list(vec![meta_flag])));
        }
        let _ = f_set_input_meta_mode(i, vec![Value::Int(8)].into());
    } else if saved_meta.truthy() {
        let meta = saved_meta
            .list_to_vec()
            .ok()
            .and_then(|v| v.first().cloned())
            .unwrap_or(Value::Nil);
        let _ = f_set_input_meta_mode(i, vec![meta].into());
        i.terminal_params
            .retain(|(k, _)| !matches!(k, Value::Sym(s) if *s == pkey));
    }
    // EOL: `(coding-system-change-eol-conversion CS 'unix)' — on
    // variant-less systems this returns CS unchanged (no bogus
    // `-unix' names).
    let f = i.q("coding-system-change-eol-conversion");
    let unix_id = i.q("unix");
    let set = i.apply(
        &Value::Sym(f),
        vec![cs_val, Value::Sym(unix_id)],
    )?;
    i.keyboard_coding = set.clone();
    let var = i.q("keyboard-coding-system");
    let _ = i.set_symbol_default(var, set.clone());
    Ok(set)
}

/// `delete-other-windows-internal' — GNU decodes WINDOW with
/// decode_valid_window (`window-valid-p'); our single window is
/// never deleted.
fn f_delete_other_windows_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_valid(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// `fit-window-to-buffer' — GNU window.el: `(window-normalize-window
/// WINDOW t)' gives `(error "%s is not a live window")' for
/// non-window args.
fn f_fit_window_to_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        Value::Window(w) if !w.borrow().dead => Ok(Value::Nil),
        other => Err(i.error(format!(
            "{} is not a live window",
            i.princ_to_string(&other)
        ))),
    }
}

/// `delete-windows-on' — resolves BUFFER-OR-NAME (GNU's "No such
/// buffer" error for the unresolvable); our single window is never
/// deleted, so nil.
fn f_delete_windows_on(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU
    // window.el `window-normalize-buffer': a name naming no live
    // buffer is "No such live buffer %s"; other non-buffer objects
    // are "No such buffer %s".  Our single window is never deleted.
    match arg(&a, 0) {
        Value::Nil => Ok(Value::Nil),
        Value::Buffer(b) => {
            if !i.buffer_live(b.borrow().id) {
                return Err(i.error(format!("No such live buffer {}", i.princ_to_string(&a[0]))));
            }
            Ok(Value::Nil)
        }
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.buffers.by_name(&name) {
                Some(_) => Ok(Value::Nil),
                None => Err(i.error(format!("No such live buffer {name}"))),
            }
        }
        other => {
            let shown = i.princ_to_string(&other);
            Err(i.error(format!("No such buffer {shown}")))
        }
    }
}

/// `face-documentation' — GNU reads the symbol's `face-documentation'
/// property; nil for unknown faces (no facep check).
fn f_face_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &a[0])?;
    let prop = i.q("face-documentation");
    Ok(i.get_prop(id, prop))
}

/// `x-open-connection' — never connects on a tty batch (nil), but
/// `xw-*' color functions consult the color database instead of
/// erroring.
fn f_x_open_connection(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU xfns.c Fx_open_connection: CHECK_STRING(DISPLAY) runs before
    // the connection attempt — a bad arg does NOT count as "attempted"
    // for the xw-* color functions.
    if !matches!(&a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    i.x_display_attempted = true;
    Ok(Value::Nil)
}

fn f_x_close_connection(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU nsfns.m: check_ns_display_info(terminal) — a string looks up
    // the display name and fails (no multi-display NS), nil reads the
    // selected frame's terminal (errors in batch), a live frame/terminal
    // that's not NS reports "not a Nextstep display", and other atoms
    // need frame-live-p.
    match a.first() {
        Some(Value::Str(_)) => {
            Err(i.error("Emacs for Nextstep does not yet support multi-display"))
        }
        Some(Value::Nil) | None => {
            Err(i.error("Nextstep windows are not in use or not initialized"))
        }
        Some(v) if matches!(v, Value::Frame(_)) || crate::editor::is_terminal(i, v) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `xw-color-defined-p' — errors until an X connection was tried,
/// then answers from the standard color table.
fn f_xw_color_defined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !i.x_display_attempted {
        return Err(i.error("Window system is not in use or not initialized"));
    }
    match &a[0] {
        Value::Str(_) => Ok(Value::from_bool(parse_color_16(&a[0]).is_some())),
        _ => Ok(Value::Nil),
    }
}

/// `window--display-buffer' — GNU's Lisp defun (nativecomp build:
/// `subrp' t), replayed in Rust.  `(when (and (buffer-live-p BUFFER)
/// (window-live-p WINDOW)) ...)': a dead buffer or a dead/minibuffer
/// window quietly returns nil — NO type check on either arg (a number
/// `buffer' arg is nil-live too).  On success: when the window isn't
/// already showing BUFFER, `dedicated' clears and the buffer slot is
/// set (the shown buffer remembered in `old_buffer'); a non-nil ALIST
/// `bump-use-time' bumps use time; the dedicated-entry policy applies
/// only when not already dedicated and TYPE isn't plain `reuse' on a
/// non-side window; TYPE ∈ (window frame tab) drops prev-buffers.
fn f_window_display_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let bid = match &a[0] {
        Value::Buffer(b) => {
            let id = b.borrow().id;
            if i.buffer_live(id) { Some(id) } else { None }
        }
        // GNU's `(and (buffer-live-p buffer) (window-live-p window))':
        // `buffer-live-p' is `(bufferp x) && live' — a name STRING is
        // not a buffer object, so `"a" → nil, no lookup.  (`display-buffer'
        // callers normalize names via `window-normalize-buffer' first.)
        _ => None, // (buffer-live-p 42)/"a" → nil, no signal
    };
    let Some(bid) = bid else {
        return Ok(Value::Nil);
    };
    let w = match &a[1] {
        Value::Window(w) => w.clone(),
        _ => return Ok(Value::Nil), // (window-live-p x) → nil
    };
    {
        let wb = w.borrow();
        if wb.dead || wb.minibuffer {
            return Ok(Value::Nil);
        }
    }
    if w.borrow().buffer != bid {
        // GNU set-window-buffer: swap `buffer'; `old_buffer' is only
        // written when a buffer leaves a window (unshow_window), so this
        // path must NOT touch it.
        let point = i.buffers.get(bid).map_or(0, |buffer| buffer.borrow().point());
        let mut wb = w.borrow_mut();
        wb.dedicated = Value::Nil;
        wb.buffer = bid;
        crate::editor::set_window_old_point(i, &mut wb, point);
        // A brand-new window's `next-buffers' entry.
        if !wb.next_buffers.is_nil() {
            wb.next_buffers = Value::Nil;
        }
        drop(wb);
        crate::editor::record_buffer_display(i, bid);
    }
    // (window-bump-use-time WINDOW) when ALIST has `bump-use-time'.
    {
        let alist = arg(&a, 3);
        // NOTE: collect the interesting ALIST entries eagerly so the
        // closure borrow of `i' doesn't fight the mutable intern below.
        let bump = {
            let items = alist.list_to_vec().unwrap_or_default();
            items.iter().any(|e| {
                if let Value::Cons(c) = e {
                    let cb = c.borrow();
                    matches!(&cb.car, Value::Sym(s) if i.symbol_name(*s) == "bump-use-time")
                        && cb.cdr.truthy()
                } else {
                    false
                }
            })
        };
        let dedicated_entry = {
            let items = alist.list_to_vec().unwrap_or_default();
            items.iter().find_map(|e| {
                if let Value::Cons(c) = e {
                    let cb = c.borrow();
                    if matches!(&cb.car, Value::Sym(s) if i.symbol_name(*s) == "dedicated") {
                        return Some(cb.cdr.clone());
                    }
                }
                None
            })
        };
        if bump {
            let bump = Value::Sym(i.q("window-bump-use-time"));
            i.apply(&bump, vec![Value::Window(w.clone())])?;
        }
        // Dedicated policy: skip if already dedicated, or TYPE is `reuse'
        // on a non-side window; else ALIST's `dedicated' wins, else
        // `display-buffer-mark-dedicated'.
        let ty_name = i.sym_id(&arg(&a, 2)).map(|s| i.symbol_name(s));
        let win_side = {
            let side_sym = i.q("window-side");
            let wb = w.borrow();
            crate::lisp::eval::plist_get(&wb.params, side_sym).truthy()
        };
        let dedicated = w.borrow().dedicated.truthy();
        if dedicated || (ty_name.as_deref() == Some("reuse") && !win_side) {
            // leave alone
        } else if let Some(v) = dedicated_entry {
            w.borrow_mut().dedicated = v;
        } else {
            let flag = i
                .obarray
                .intern_soft("display-buffer-mark-dedicated")
                .map(|id| i.symbol_value(id))
                .unwrap_or(Value::Nil);
            w.borrow_mut().dedicated = flag;
        }
        // (memq type '(window frame tab)) → (set-window-prev-buffers w nil)
        if matches!(
            ty_name.as_deref(),
            Some("window") | Some("frame") | Some("tab")
        ) {
            w.borrow_mut().prev_buffers = Value::Nil;
        }
    }
    Ok(Value::Window(w))
}

/// `internal-make-lisp-face' — creates the face (GNU registers it in
/// the face table) and returns the fresh face vector
/// `[face unspecified ...]' with 19 attribute slots.
fn f_internal_make_lisp_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_sym(i, &a[0])?;
    let name = i.symbol_name(id).to_string();
    if !i.face_table.iter().any(|(n, _)| *n == name) {
        // GNU registers the NAME but creates no lface — so
        // `internal-lisp-face-empty-p' (lface_from_face_name) still
        // errors "Invalid face" on it, unlike `make-face' faces.
        // The `remacs--raw-face' sentinel distinguishes the two.
        let sentinel = Value::Sym(i.q("remacs--raw-face"));
        i.face_table.push((name.clone(), sentinel));
    }
    crate::editor::face_register_global(i, &name);
    let mut v = vec![Value::Sym(i.q("face"))];
    let unspec = Value::Sym(i.q("unspecified"));
    for _ in 0..19 {
        v.push(unspec.clone());
    }
    Ok(Value::vector(v))
}

/// `internal-copy-lisp-face' — GNU Finternal_copy_lisp_face order:
/// CHECK_SYMBOL(FROM), CHECK_SYMBOL(TO), then FRAME: `t' copies the
/// global definition; otherwise CHECK_LIVE_FRAME(FRAME) and
/// CHECK_LIVE_FRAME(NEW-FRAME, defaulting to FRAME) — nil is NOT a live
/// frame here.  Unknown FROM signals (error "Invalid face" FROM)
/// before TO is created.
fn f_internal_copy_lisp_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = want_sym(i, &a[0])?;
    let to = want_sym(i, &a[1])?;
    let frame = arg(&a, 2);
    let global = matches!(frame, Value::Sym(s) if s == sym::T);
    if !global {
        let nf = arg(&a, 3);
        let nf = if nf.is_nil() { frame.clone() } else { nf };
        match &frame {
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
        match &nf {
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    let from_name = i.symbol_name(from).to_string();
    if !crate::editor::face_known(i, &from_name) {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face"), a[0].clone()],
        ));
    }
    let to_name = i.symbol_name(to).to_string();
    if !i.face_table.iter().any(|(n, _)| *n == to_name) {
        i.face_table.push((to_name.clone(), Value::Nil));
    }
    // GNU Finternal_copy_lisp_face: TO's frame face receives FROM's
    // attributes (our spec/realized layer = face_table plist).
    crate::editor::face_register_global(i, &to_name);
    let src = i
        .face_table
        .iter()
        .find(|(n, _)| *n == from_name)
        .map(|(_, p)| p.clone())
        .unwrap_or(Value::Nil);
    if let Some((_, dst)) = i.face_table.iter_mut().find(|(n, _)| *n == to_name) {
        *dst = src;
    }
    Ok(a[1].clone())
}

/// `font-at' — GNU checks that WINDOW (default selected) displays
/// the current buffer, then bounds-checks POSITION against the
/// buffer (1-based) or STRING (0-based).  tty text has no font.
fn f_font_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c: decode_live_window(WINDOW) first (wta window-live-p),
    // then the STRING branch: CHECK_FIXNUM(position) + CHECK_STRING;
    // else the window's buffer must BE the current buffer, and
    // fix_position(pos) accepts ints/markers (integer-or-marker-p).
    let w = crate::editor::win_live(i, &arg(&a, 1))?;
    let aoor = |i: &mut Interp, v: Vec<Value>| {
        let s = i.q("args-out-of-range");
        i.signal_data(s, v)
    };
    if !arg(&a, 2).is_nil() {
        let pos = crate::lisp::builtins::want_fixnum(i, &a[0])?;
        let s = want_string(i, &a[2])?;
        let len = s.chars().count() as i128;
        if pos < 0 || pos >= len {
            return Err(aoor(i, vec![Value::string(s.clone()), Value::Int(pos)]));
        }
        return Ok(Value::Nil);
    }
    let wid = w.borrow().buffer;
    let cur = i.buffers.get(i.current_buffer);
    if cur.as_ref().map(|b| b.borrow().id) != Some(wid) {
        return Err(i.error("Specified window is not displaying the current buffer"));
    }
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let (begv, zv) = match &cur {
        Some(b) => {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.text_len() as i128 + 1)
        }
        None => (1, 1),
    };
    if pos < begv || pos >= zv {
        return Err(aoor(
            i,
            vec![a[0].clone(), Value::Int(begv), Value::Int(zv)],
        ));
    }
    Ok(Value::Nil)
}

/// `font-info' — needs a window-system frame; GNU always errors on
/// a tty.
fn f_font_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c Ffont_info:
    // CHECK_STRING(NAME) first, then the optional FRAME decodes via
    // frame-live-p — both before the batch winsys error.
    let _ = want_string(i, &a[0])?;
    if let Some(v) = a.get(1) {
        match v {
            Value::Nil => {}
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        }
    }
    Err(i.error("Window system frame should be used"))
}

/// `font-match-p' — both args are font-specs in GNU (typed check);
/// matching two arbitrary specs is trivially true for us.
fn f_font_match_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    for v in &a[..2] {
        if !matches!(v, Value::Record(_)) {
            return Err(i.wrong_type_mut("font-spec", v));
        }
    }
    Ok(Value::t())
}

/// `font-face-attributes' — GNU font.c accepts a font name string
/// (mapped to `(:family NAME)'), a font-spec (its core properties are
/// reported, :size → :height ×10), and other objects signal
/// "Invalid font object OBJ".
fn invalid_font_object(i: &mut Interp, v: &Value) -> Flow {
    // GNU font.c: (error "Invalid font object" OBJ) — the object is a
    // separate signal datum, not part of the format string.
    let err = i.q("error");
    i.signal_data(err, vec![Value::string("Invalid font object"), v.clone()])
}

fn f_font_face_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Ffont_face_attributes decodes FRAME (arg2) via
    // decode_live_frame before CHECK_FONTing arg1.
    if a.len() > 1 {
        match &a[1] {
            Value::Nil => {}
            Value::Frame(f) if !f.borrow().dead => {}
            other => return Err(i.wrong_type_mut("frame-live-p", &other)),
        }
    }
    match &a[0] {
        Value::Str(_) => {
            return Ok(Value::list(vec![
                Value::Sym(i.q(":family")),
                a[0].clone(),
            ]));
        }
        Value::Record(r) => {
            let fields = r.borrow().clone();
            // Our font-spec stores its property plist in slot 1.
            let spec = i.q("font-spec");
            let is_spec = matches!(fields.first(), Some(Value::Sym(s)) if *s == spec);
            if !is_spec {
                return Err(invalid_font_object(i, &a[0]));
            }
            let plist = fields.get(1).cloned().unwrap_or(Value::Nil);
            let mut out = Vec::new();
            let mut cur = plist;
            while let Value::Cons(c) = cur {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Cons(c2) = rest {
                    let (v, next) = {
                        let b = c2.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let kname = match &k {
                        Value::Sym(s) => i.symbol_name(*s).to_string(),
                        _ => String::new(),
                    };
                    let mapped = match kname.as_str() {
                        ":family" => Some((":family", v)),
                        ":weight" => Some((":weight", v)),
                        ":slant" => Some((":slant", v)),
                        ":width" => Some((":width", v)),
                        ":size" => match &v {
                            Value::Int(n) => Some((":height", Value::Int(n * 10))),
                            Value::Float(f) => Some((":height", Value::float(**f * 10.0))),
                            _ => None,
                        },
                        ":foundry" => Some((":foundry", v)),
                        ":registry" => Some((":registry", v)),
                        _ => None,
                    };
                    if let Some((k2, v2)) = mapped {
                        // GNU interns string values of the name slots into
                        // symbols... no: :family stays a string, as in
                        // `(:family "a")'.  Symbols pass through.
                        out.push(Value::Sym(i.intern(k2)));
                        out.push(v2);
                    }
                    cur = next;
                } else {
                    break;
                }
            }
            return Ok(Value::list(out));
        }
        _ => {}
    }
    Err(invalid_font_object(i, &a[0]))
}

/// `font-has-char-p' — typed `font' check, then nil (no fonts on a
/// tty).
fn f_font_has_char_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Record(_)) {
        return Err(i.wrong_type_mut("font", &a[0]));
    }
    Ok(Value::Nil)
}

/// `x-show-tip' — GNU xfns.c Fx_show_tip: CHECK_STRING(STRING),
/// then decode_live_frame(FRAME), only then the window-system check
/// ("Window system frame should be used" on a tty frame).
fn f_x_show_tip(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(&a[0], Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    let _ = crate::editor::frame_live(i, &arg(&a, 1))?;
    Err(i.error("Window system frame should be used"))
}

/// `image-size' / `image-mask-p' — GNU validates the spec first and
/// signals "Invalid image specification" for anything else; a valid
/// spec then hits `decode_window_system_frame', which on a tty is the
/// "Window system frame should be used" error.
fn f_image_spec_check(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    Err(i.error("Window system frame should be used"))
}

/// `image-metadata' — nil for an invalid spec; a valid spec hits the
/// same window-system error as `image-size'.
fn f_image_metadata(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Ok(Value::Nil);
    }
    Err(i.error("Window system frame should be used"))
}

/// Detect the byte sequence represented by a Lisp string, rather than the
/// UTF-8 backing storage used by Rust.
fn f_detect_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let unibyte = matches!(&a[0], Value::Str(r) if i.is_unibyte_str(r));
    let input: Vec<i32> = s
        .chars()
        .map(|c| {
            if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                b as i32
            } else if unibyte || c.is_ascii() {
                c as i32
            } else {
                -(crate::lisp::value::lisp_char_code(c) as i32)
            }
        })
        .collect();
    super::coding_detection::detect(i, &input, !unibyte, arg(&a, 1).truthy())
}

fn f_arg1(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(a[1].clone())
}

/// `x-display-*'/`x-server-*'/`xw-*' on a non-Nextstep display: GNU
/// checks the arg (frame-live-p), then signals the NS error — with a
/// terminal object the message names the terminal.
fn f_ns_display(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        None | Some(Value::Nil) => {
            Err(i.error("Nextstep windows are not in use or not initialized"))
        }
        // GNU (nsfns.m): a string is a display name — the NS backend is
        // single-display, so it always errors this way.
        Some(Value::Str(_)) => {
            Err(i.error("Emacs for Nextstep does not yet support multi-display"))
        }
        Some(v) if crate::editor::is_terminal(i, v) => {
            Err(i.error("Terminal 0 is not a Nextstep display"))
        }
        Some(Value::Frame(_)) => Err(i.error("Terminal 0 is not a Nextstep display")),
        Some(other) => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `image-type' — GNU maps a file name's extension via
/// `image-type-file-name-regexps'; unknown extension signals
/// `unknown-image-type', a non-string signals a plain error.
fn f_image_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Str(s) = &a[0] else {
        let shown = i.princ_to_string(&a[0]);
        return Err(i.error_q("Invalid image file name `%s'", &[&shown]));
    };
    let name = s.borrow().clone();
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
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
            let unk = i.q("unknown-image-type");
            Err(i.signal_data(unk, vec![Value::string("Cannot determine image type")]))
        }
    }
}

/// The image types remacs claims (GNU `image_types[]' parity):
/// `init-image-library'/`image-type-available-p' share the set.
fn known_image_type(i: &Interp, v: &Value) -> bool {
    let Value::Sym(s) = v else {
        return false;
    };
    matches!(
        i.symbol_name(*s).as_str(),
        "png"
            | "gif"
            | "jpeg"
            | "webp"
            | "bmp"
            | "xpm"
            | "pbm"
            | "xbm"
            | "postscript"
            | "tiff"
            | "svg"
            | "heic"
    )
}

/// `image-type-available-p' — the types our (fake) image support
/// claims: GNU batch reports all built-ins available.
fn f_image_type_available_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(known_image_type(i, &a[0])))
}

/// `init-image-library' — GNU's `lookup_image_type': t iff TYPE is a
/// registered (built-in) image type symbol.
fn f_init_image_library(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(known_image_type(i, &a[0])))
}

/// `imagep' — GNU's valid_image_p: a list headed `image' whose plist
/// has a known :type plus a :file or :data source.
fn valid_image_spec(i: &Interp, v: &Value) -> bool {
    let Value::Cons(c) = v else {
        return false;
    };
    if !matches!(&c.borrow().car, Value::Sym(s) if i.symbol_name(*s) == "image") {
        return false;
    }
    let mut ty = false;
    let mut src = false;
    let mut cur = c.borrow().cdr.clone();
    while let Value::Cons(p) = cur {
        let (k, rest) = (p.borrow().car.clone(), p.borrow().cdr.clone());
        if let Value::Sym(id) = &k {
            let n = i.symbol_name(*id).to_string();
            let val = match &rest {
                Value::Cons(vc) => vc.borrow().car.clone(),
                _ => Value::Nil,
            };
            match n.as_str() {
                ":type" => {
                    if known_image_type(i, &val) {
                        ty = true;
                    }
                }
                ":file" | ":data" => {
                    if matches!(val, Value::Str(_)) {
                        src = true;
                    }
                }
                _ => {}
            }
        }
        match rest {
            Value::Cons(r) => cur = r.borrow().cdr.clone(),
            _ => break,
        }
    }
    ty && src
}

fn f_imagep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(valid_image_spec(i, &a[0])))
}

/// `image-flush' — invalid spec signals a plain error; a valid spec
/// then hits the no-window-system error like GNU on a tty.
fn f_image_flush(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !valid_image_spec(i, &a[0]) {
        return Err(i.error("Invalid image specification"));
    }
    Err(i.error("Window system frame should be used"))
}


// ---- popup-menu position decode (GNU menu.c x_popup_menu_1) ----
//
// Shared between `popup-menu' (which applies menu-bar.el's
// `popup-menu-normalize-position' first) and `x-popup-menu' (raw).

fn popup_car_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}
fn popup_fcar(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}
fn popup_fcdr(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// menu.c x_popup_menu_1 position decode.  Errors match GNU's
/// `wrong-type-argument' outcomes for every malformed shape.
pub(crate) fn decode_popup_position(i: &mut Interp, pos: &Value) -> Result<(), Flow> {
    if matches!(pos, Value::Nil) {
        return Ok(()); // obsolete precalculate call
    }
    if matches!(pos, Value::Sym(s) if *s == sym::T) {
        return Ok(()); // current mouse position
    }
    let Value::Cons(pc) = pos else {
        return Err(i.wrong_type_mut("listp", pos));
    };
    let (pcar, pcdr) = {
        let b = pc.borrow();
        (b.car.clone(), b.cdr.clone())
    };
    if matches!(&pcar, Value::Sym(s) if matches!(&*i.symbol_name(*s), "menu-bar" | "tab-bar" | "tool-bar"))
    {
        return Ok(());
    }
    if let Value::Cons(c2) = &pcdr {
        if matches!(&c2.borrow().car, Value::Sym(s) if &*i.symbol_name(*s) == "tab-bar") {
            return Ok(());
        }
    }
    let (window, x, y);
    if let Value::Cons(xyc) = &pcar {
        // ((X Y) WINDOW): window = Fcar(Fcdr pos) — Fcar on an
        // atomic tail signals listp on the tail.
        window = popup_fcar(i, &pcdr)?;
        x = xyc.borrow().car.clone();
        // y = Fcar(XCDR(xy)): raw cdr then Fcar — atom → listp.
        y = popup_fcar(i, &xyc.borrow().cdr)?;
    } else {
        // EVENT: tem = CAR_SAFE(CDR_SAFE pos) = cadr (atomic cdr
        // → nil); then Fcar/Fcdr chains signal listp on atoms.
        let tem = popup_car_safe(&pcdr);
        window = popup_fcar(i, &tem)?;
        let t1 = popup_fcdr(i, &tem)?;
        let _tem2 = popup_fcar(i, &t1)?;
        let t2 = popup_fcdr(i, &t1)?;
        let tem3 = popup_fcar(i, &t2)?;
        x = popup_fcar(i, &tem3)?;
        y = popup_fcdr(i, &tem3)?;
    }
    if matches!(x, Value::Nil) && matches!(y, Value::Nil) {
        return Ok(()); // get_current_pos_p
    }
    match window {
        Value::Frame(_) | Value::Window(_) => {}
        w => return Err(i.wrong_type_mut("windowp", &w)),
    }
    for v in [&x, &y] {
        if !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("integerp", v));
        }
    }
    Ok(())
}
/// `popup-menu' — GNU's Lisp def (menu-bar.el): MENU resolves via
///   (keymapp menu)                    → x-popup-menu → nil in batch
///   (listp menu && keymapp (car))     → list of keymaps → nil
///   (not (listp menu))                → map is nil → nil
///   else easy-menu-create-menu on (car menu) / (cdr menu), whose
///   per-item converter signals "Invalid menu item in easymenu".
/// In --batch x-popup-menu always returns nil so a successfully
/// validated menu still finishes nil without popping anything.
fn f_popup_menu(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    fn aref_check(i: &mut Interp, v: &Value, idx: usize) -> Result<(), Flow> {
        let len = match v {
            Value::Vec(rv) => rv.borrow().len(),
            _ => 0,
        };
        if idx >= len {
            let s = i.q("args-out-of-range");
            return Err(i.signal_data(s, vec![v.clone(), Value::Int(idx as i128)]));
        }
        Ok(())
    }
    fn keywordp(i: &Interp, v: &Value) -> bool {
        matches!(v, Value::Sym(id) if i.symbol_name(*id).starts_with(':'))
    }
    // easy-menu-convert-item-1: validate one menu item.
    fn check_item(i: &mut Interp, item: &Value) -> Result<(), Flow> {
        match item {
            Value::Str(_) => Ok(()),
            Value::Cons(c) => {
                let cdr = c.borrow().cdr.clone();
                // (NAME . COMMAND): COMMAND is used verbatim when it is a
                // keymap, else it is the submenu's item list.
                if crate::editor::is_keymap(i, &cdr) {
                    return Ok(());
                }
                check_items(i, &cdr)
            }
            Value::Vec(_) => {
                // (aref item 1) is evaluated before any keyword walking.
                aref_check(i, item, 1)?;
                let (len, active) = match item {
                    Value::Vec(rv) => {
                        let b = rv.borrow();
                        (
                            b.len(),
                            if b.len() > 2 {
                                b[2].clone()
                            } else {
                                Value::t()
                            },
                        )
                    }
                    _ => unreachable!(),
                };
                if keywordp(i, &active) {
                    // Keyword pairs start at index 2; both aref calls run
                    // while (1+ count) < len is still unchecked.
                    let mut count = 2usize;
                    while len > count {
                        aref_check(i, item, count)?;
                        aref_check(i, item, count + 1)?;
                        count += 2;
                    }
                }
                Ok(())
            }
            _ => Err(i.error("Invalid menu item in easymenu")),
        }
    }
    // easy-menu-create-menu: leading KEYWORD ARG pairs are consumed first
    // (any keyword counts; unknown ones are ignored by the pcase), then a
    // mapcar walks the remaining items in order — an atom tail signals
    // listp only after the items before it pass.
    fn check_items(i: &mut Interp, items: &Value) -> Result<(), Flow> {
        let mut cur = items.clone();
        loop {
            match &cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
                    // (and items (cdr items) (keywordp (car items)))
                    match &cdr {
                        Value::Nil => break,
                        Value::Cons(cc) => {
                            if keywordp(i, &car) {
                                cur = cc.borrow().cdr.clone();
                                continue;
                            }
                            break;
                        }
                        // (cdr items) is a non-nil atom and CAR is a
                        // keyword → (cddr items) errors on the atom.
                        other => {
                            if keywordp(i, &car) {
                                return Err(i.wrong_type_mut("listp", other));
                            }
                            break;
                        }
                    }
                }
                // (cdr items) on an atom head: the while condition's first
                // access already signals listp.
                other => return Err(i.wrong_type_mut("listp", other)),
            }
        }
        while let Value::Cons(c) = cur {
            let (item, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            check_item(i, &item)?;
            cur = rest;
        }
        match cur {
            Value::Nil => Ok(()),
            other => Err(i.wrong_type_mut("listp", &other)),
        }
    }
    // GNU `popup-menu' is Lisp (menu-bar.el): POSITION first goes
    // through `popup-menu-normalize-position', then the normalized
    // value is decoded by `x-popup-menu' (menu.c x_popup_menu_1).
    //
    //   normalize:
    //     nil                       → posn-at-point → decodes nil
    //     posnp                     → ((X Y) WINDOW): decode checks
    //                                 windowp + integerp on the
    //                                 posn-x-y pair
    //     car = touchscreen-begin/  → posn-at-point → decodes nil
    //       end
    //     eventp                    → normalize (event-end position)
    //       recursively.  event-end = nth(if consp(nth 2) 2 1);
    //       nthcdr errors listp on the WHOLE list (dotted tail),
    //       nth's Fcar on an atomic tail errors listp on the atom.
    //       A nil event-end falls back to posn-at-point.
    //     else                      → decode as-is
    //
    //   decode (menu.c x_popup_menu_1):
    //     t / car ∈ {menu-bar,tab-bar,tool-bar} / cadr = tab-bar
    //                               → get_current_pos → nil
    //     atom                      → Fcar → listp on the atom
    //     cons, car cons            → ((X Y) W): window = cadr;
    //                                 x = XCAR(car), y = car(cdr(car))
    //     cons, other               → EVENT: tem = cadr-or-nil;
    //                                 window=Fcar(tem), tem2=Fcar(Fcdr
    //                                 tem), tem3=Fcar(Fcdr(Fcdr tem)),
    //                                 x=Fcar(tem3), y=Fcdr(tem3) —
    //                                 Fcar/Fcdr on atoms signal listp
    //     x & y nil                 → get_current_pos → nil
    //     else                      → windowp|framep window, then
    //                                 integerp x, integerp y.

    // fns.c Fnthcdr+Fnth: counting down, a non-nil non-cons tail
    // signals listp on the ORIGINAL list; a count that lands on a tail
    // returns it as-is (atom included) and nth Fcar's it (atom →
    // listp on the atom).
    fn nth(i: &mut Interp, n: i128, list: &Value) -> Result<Value, Flow> {
        let mut tail = list.clone();
        for _ in 0..n.max(0) {
            match tail {
                Value::Cons(c) => tail = c.borrow().cdr.clone(),
                Value::Nil => return Ok(Value::Nil),
                _ => return Err(i.wrong_type_mut("listp", list)),
            }
        }
        match tail {
            Value::Cons(c) => Ok(c.borrow().car.clone()),
            Value::Nil => Ok(Value::Nil),
            atom => Err(i.wrong_type_mut("listp", &atom)),
        }
    }
    use crate::lisp::builtins::misc::{
        popup_car_safe as car_safe, popup_fcar as fcar, popup_fcdr as fcdr,
    };
    // subr.el posnp — the cdr walks use plain `cdr' (signals on an
    // atom), the heads use `car-safe'.
    fn posnp_check(i: &mut Interp, v: &Value) -> Result<bool, Flow> {
        if !matches!(car_safe(v), Value::Window(_)) {
            return Ok(false);
        }
        let obj = fcdr(i, v)?;
        if matches!(car_safe(&obj), Value::Cons(_)) {
            return Ok(false); // AREA-OR-POS must satisfy `atom'
        }
        let obj = fcdr(i, &obj)?;
        if !matches!(car_safe(&car_safe(&obj)), Value::Int(_)) {
            return Ok(false); // XOFFSET
        }
        let d3 = fcdr(i, &obj)?;
        Ok(matches!(car_safe(&d3), Value::Int(_))) // TIMESTAMP
    }
    fn eventp(i: &Interp, v: &Value) -> bool {
        let head = match v {
            Value::Int(_) => return true,
            Value::Sym(_) => v.clone(),
            Value::Cons(c) => c.borrow().car.clone(),
            _ => return false,
        };
        match &head {
            Value::Sym(s) => !i.symbol_name(*s).starts_with(':'),
            _ => false,
        }
    }

    let mut pos = arg(&a, 1).clone();
    let decoded = 'norm: loop {
        match &pos {
            Value::Nil => break 'norm None, // → posn-at-point → nil
            _ => {}
        }
        if posnp_check(i, &pos)? {
            // normalize → ((X Y) WINDOW): windowp is already proven;
            // the x/y decode checks integerp on the posn-x-y pair.
            let xy = nth(i, 2, &pos)?; // posn-x-y
            let x = fcar(i, &xy)?;
            let y = fcdr(i, &xy)?;
            for v in [&x, &y] {
                if !matches!(v, Value::Int(_)) {
                    return Err(i.wrong_type_mut("integerp", v));
                }
            }
            break 'norm None;
        }
        if let Value::Cons(c) = &pos {
            if matches!(&c.borrow().car, Value::Sym(s) if matches!(&*i.symbol_name(*s), "touchscreen-begin" | "touchscreen-end"))
            {
                break 'norm None; // → posn-at-point
            }
        }
        if eventp(i, &pos) {
            match &pos {
                Value::Cons(_) => {
                    let inner = nth(i, 2, &pos)?;
                    let n = if matches!(inner, Value::Cons(_)) { 2 } else { 1 };
                    match nth(i, n, &pos)? {
                        Value::Nil => break 'norm None, // → posn-at-point
                        end => {
                            pos = end;
                            continue 'norm;
                        }
                    }
                }
                _ => break 'norm None, // bare atom event → posn-at-point
            }
        }
        break 'norm Some(pos.clone()); // (t position)
    };
    if let Some(p) = decoded {
        decode_popup_position(i, &p)?;
    }
    match &a[0] {
        Value::Nil => Ok(Value::Nil),
        menu if crate::editor::is_keymap(i, menu) => Ok(Value::Nil),
        Value::Cons(c) => {
            let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            if crate::editor::is_keymap(i, &car) {
                return Ok(Value::Nil);
            }
            check_items(i, &cdr)?;
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

/// `scroll-bar-scale' — first arg is a cons (LISTP check), returns 0
/// when there are no scroll bars.
fn f_scroll_bar_scale(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Cons(_) | Value::Nil => Ok(Value::Int(0)),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// `get-display-property' — GNU Fget_display_property checks that
/// POSITION is inside OBJECT's text (signal `args-out-of-range' with
/// `(POS POS)') and that OBJECT satisfies `buffer-or-string-p'.
/// Buffer positions are 1-based [BEGV, ZV]; string positions are
/// 0-based [0, len].  The lookup always finds no display prop in
/// batch, so a valid call returns nil.
fn f_get_display_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match &a[0] {
        Value::Int(n) => *n,
        // GNU marker_position: 0-based bufpos + 1.
        Value::Marker(m) => match m.borrow().buffer {
            Some(_) => m.borrow().position as i128 + 1,
            None => return Err(i.error("Marker does not point anywhere")),
        },
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    // Internal begv/zv are 0-based; add 1 for the Emacs position.
    let (lo, hi) = match a.get(2) {
        None | Some(Value::Nil) => match i.current_buffer_ref() {
            Some(b) => {
                let bb = b.borrow();
                (bb.begv as i128 + 1, bb.zv as i128 + 1)
            }
            None => return Ok(Value::Nil),
        },
        Some(Value::Buffer(b)) => {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.zv as i128 + 1)
        }
        Some(Value::Str(s)) => {
            let len = s.borrow().chars().count() as i128;
            (0, len)
        }
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    };
    if pos < lo || pos > hi {
        return Err(i.signal(
            sym::ARGS_OUT_OF_RANGE,
            Value::list(vec![a[0].clone(), a[0].clone()]),
        ));
    }
    Ok(Value::Nil)
}

/// `gnutls-available-p' — GNU returns the GnuTLS capability list.
fn f_gnutls_available_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    const CAPS: &[&str] = &[
        "Key Share",
        "Post Handshake Auth",
        "PSK Key Exchange Modes",
        "Cookie",
        "Supported Versions",
        "Early Data",
        "Pre Shared Key",
        "Session Ticket",
        "Record Size Limit",
        "Compress Certificate",
        "Extended Master Secret",
        "Encrypt-then-MAC",
        "Server Certificate Type",
        "Client Certificate Type",
        "ALPN",
        "SRTP",
        "Signature Algorithms",
        "Supported EC Point Formats",
        "Supported Groups",
        "OCSP Status Request",
        "Maximum Record Size",
        "Server Name Indication",
        "macs",
        "AEAD-ciphers",
        "ciphers",
        "digests",
        "gnutls3",
        "ClientHello Padding",
        "gnutls",
    ];
    let vals: Vec<Value> = CAPS.iter().map(|c| Value::Sym(i.intern(c))).collect();
    Ok(Value::list(vals))
}

/// `help--describe-vector' (keymap.c): specbind `standard-output' to
/// the current buffer, then run `describe_vector' with keymap_p=t —
/// describe-map's lambda is the raw BASIC describer.
fn f_help_describe_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let depth = i.specbind_depth();
    let curbuf = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
    i.specbind(i.standard_output_sym, curbuf)?;
    let result = if !matches!(&a[0], Value::Vec(_)) && !is_char_table(i, &a[0]) {
        Err(i.wrong_type_mut("vector-or-char-table-p", &a[0]))
    } else {
        describe_vector(
            i,
            &a[0],
            &a[1],
            &a[2],
            a[3].truthy(),
            &a[4],
            &a[5],
            true,
            true,
            a[6].truthy(),
        )
    };
    let _ = i.unbind_to(depth);
    result.map(|_| Value::Nil)
}

/// `font-get-glyphs' / `font-variation-glyphs' — GNU requires a real
/// font object (font-spec does not qualify).
fn f_font_object_stub(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fo = i.q("font-object");
    let ok = match &a[0] {
        Value::Record(r) => i.sym_is(&r.borrow()[0], fo),
        _ => false,
    };
    if !ok {
        return Err(i.wrong_type_mut("font-object", &a[0]));
    }
    Ok(Value::Nil)
}

/// `font-shape-gstring' — GNU checks a glyph-string (vector).
fn f_font_shape_gstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU font.c requires a real gstring — a vector of LGLYPH records
    // produced by composition-get-gstring.  At -Q no lgstring can be
    // built, so every arg is invalid (a plain vector errors too).
    Err(i.signal_data(
        sym::ERROR,
        vec![
            Value::string("Invalid glyph-string: ".to_string()),
            a[0].clone(),
        ],
    ))
}

/// `x-get-resource' / `x-list-fonts' — no X display in batch.
fn f_x_get_resource(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Err(i.error("Window system is not in use or not initialized"))
}

/// `set-mouse-position' / `set-mouse-pixel-position' — frame-live-p
/// check on FRAME; nil on a tty.
fn f_frame_live_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", &other)),
    }
}

/// `set-frame-font' — GNU's third argument (FRAMES) is a list.
fn f_set_frame_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 2) {
        Value::Nil | Value::Cons(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", &other)),
    }
}

/// `destroy-fringe-bitmap' — the name must be a symbol.
fn f_destroy_fringe_bitmap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Sym(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

/// `set-fringe-bitmap-face' — symbolp name, then GNU fails because
/// no fringe bitmaps are defined.
fn f_set_fringe_bitmap_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Sym(_) => Err(i.error("Undefined fringe bitmap")),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

/// `internal-set-alternative-font-family-alist' — each element's car
/// must be a string (GNU stores family-name → alternative list).
fn f_set_alt_font_family_alist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_LIST first — non-nil non-cons signals listp.
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (elt, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
        let car = match &elt {
            Value::Cons(e) => e.borrow().car.clone(),
            other => other.clone(),
        };
        if !matches!(car, Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", &car));
        }
        cur = rest;
    }
    Ok(Value::Nil)
}

/// `internal-set-alternative-font-registry-alist' — each element's
/// car is char-or-string-p.
fn f_set_alt_font_registry_alist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    let mut cur = a[0].clone();
    while let Value::Cons(c) = cur {
        let (elt, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
        let car = match &elt {
            Value::Cons(e) => e.borrow().car.clone(),
            other => other.clone(),
        };
        if !matches!(car, Value::Str(_) | Value::Int(_)) {
            return Err(i.wrong_type_mut("char-or-string-p", &car));
        }
        cur = rest;
    }
    Ok(Value::Nil)
}

/// `internal-set-lisp-face-attribute-from-resource' — GNU xfaces.c
/// arg order: CHECK_SYMBOL(FACE), CHECK_SYMBOL(ATTR), CHECK_STRING(VALUE),
/// then the resource value is decoded per ATTR name (unknown attrs pass
/// straight through to `internal-set-lisp-face-attribute', which is
/// where "Invalid face attribute name" comes from).
fn f_set_lisp_face_attr_resource(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const ATTRS: &[&str] = &[
        ":family",
        ":foundry",
        ":width",
        ":height",
        ":weight",
        ":slant",
        ":underline",
        ":inverse-video",
        ":foreground",
        ":background",
        ":stipple",
        ":overline",
        ":strike-through",
        ":box",
        ":font",
        ":inherit",
        ":fontset",
        ":distant-foreground",
        ":extend",
        ":bold",
        ":italic",
    ];
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    // GNU CHECK_SYMBOLs ATTR, then CHECK_STRINGs VALUE — both before
    // the attribute-name test inside internal-set-lisp-face-attribute.
    if i.sym_id(&a[1]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
    let _v = want_string(i, &a[2])?;
    let attr_id = i.sym_id(&a[1]).unwrap();
    if !ATTRS.contains(&i.symbol_name(attr_id).as_str()) {
        // GNU signals (error "Invalid face attribute name" ATTR) —
        // structured signal data, not a message-formatted string.
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Invalid face attribute name"), a[1].clone()],
        ));
    }
    Ok(a[0].clone())
}

/// `read-positioning-symbols' — like `read', but every symbol token
/// becomes a `symbol-with-pos' object.  GNU reports 1-based absolute
/// positions for buffers and 0-based read-relative ones for strings
/// and markers. Both entry points share callable-stream and minibuffer
/// dispatch; the minibuffer path returns ordinary objects.
fn f_read_positioning_symbols(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    super::readfn::read_source(i, arg(&a, 0), true)
}

/// `tooltip-mode' — minor-mode semantics: no arg reports, 'toggle
/// flips, numeric <= 0 disables, else enables.  State lives in the
/// `tooltip-mode' variable (seeded t like GNU).
fn f_tooltip_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = i.q("tooltip-mode");
    let cur = i.symbol_value(sid).truthy();
    let on = match arg(&a, 0) {
        // GNU minor-mode commands called from Lisp treat nil as ON.
        Value::Nil => true,
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
fn f_command_error_default(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcommand_error_default_function
    // CHECK_LISTs DATA before formatting — a non-list arg signals
    // wrong-type-argument listp rather than printing "peculiar error".
    if !matches!(&a[0], Value::Cons(_) | Value::Nil) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    if let Value::Cons(condition) = &a[0] {
        let name = condition.borrow().car.clone();
        if i.sym_id(&name).is_none() {
            return Err(i.wrong_type_mut("symbolp", &name));
        }
    }
    let context = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if let Value::Cons(condition) = &a[0] {
        let minibuffer_quit = Value::Sym(i.q("minibuffer-quit"));
        if i.signal_matches(&condition.borrow().car, &minibuffer_quit) {
            super::evalfn::f_ding(i, vec![Value::t()].into())?;
            let msg = error_message(i, &a[0]);
            if i.noninteractive {
                i.write_output_to(&format!("{context}{msg}"), &Value::t())?;
            } else {
                i.message(&format!("{context}{msg}"));
            }
            return Ok(Value::Nil);
        }
    }
    let msg = error_message(i, &a[0]);
    if i.noninteractive {
        eprintln!("{context}{msg}");
        return super::evalfn::f_kill_emacs(i, vec![Value::Int(-1)].into());
    }
    i.message(&format!("{context}{msg}"));
    Ok(Value::Nil)
}

/// `shrink-window-if-larger-than-buffer' — GNU window.el: live-window
/// gate via `window-normalize-window w t' (plain "N is not a live
/// window" error), then only shrinks when the window is taller than
/// its buffer — nil in batch.
fn f_shrink_if_larger(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_err_live(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// `window-preserve-size' — GNU window.el: `window-normalize-window
/// window t' gates with the plain "N is not a live window" error,
/// then reports (BUFFER HSIZE WSIZE) — nil sizes in batch.
fn f_window_preserve_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => crate::editor::sel_window(i),
        Value::Window(w) if !w.borrow().dead => Some(w),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live window")));
        }
    };
    let buf = match w {
        Some(w) => i.buffer_value(w.borrow().buffer).unwrap_or(Value::Nil),
        None => Value::Nil,
    };
    Ok(Value::list(vec![buf, Value::Nil, Value::Nil]))
}

/// GNU delivers a signal to the current thread synchronously, with
/// exactly the same validation and unwind behavior as `signal'.
fn f_thread_signal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let thread = want_thread(i, &a[0])?;
    if std::rc::Rc::ptr_eq(&thread, &i.threads[i.current_thread]) {
        return super::evalfn::f_signal(i, a[1..].to_vec().into());
    }
    if std::rc::Rc::ptr_eq(&thread, &i.threads[0]) {
        crate::lisp::eval::threads::signal_main(i, a[1].clone(), a[2].clone());
        return Ok(Value::Nil);
    }
    // Live targets receive this at their next scheduling boundary. For
    // completed targets GNU retains it for later (including repeat) joins.
    thread.borrow_mut().pending_signal = Some((a[1].clone(), a[2].clone()));
    Ok(Value::Nil)
}

fn coding_region_source(i: &mut Interp, a: &[Value]) -> Result<(Vec<i128>, Value), Flow> {
    let mut positions = Vec::new();
    for v in &a[..2] {
        let position = match v {
            Value::Int(n) if v.fixnump() => *n,
            Value::Int(_) | Value::BigInt(_) => {
                let buffer = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![buffer, a[0].clone(), a[1].clone()],
                ));
            }
            Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
            Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
            v => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
        };
        positions.push(position);
    }
    positions.sort();
    let source = crate::buffer::primitives::f_buffer_substring(
        i,
        positions
            .iter()
            .map(|p| Value::Int(*p))
            .collect::<Vec<_>>()
            .into(),
    )?;
    Ok((positions, source))
}

fn f_detect_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (_, source) = coding_region_source(i, &a)?;
    f_detect_coding_string(i, vec![source, arg(&a, 2)].into())
}

/// Canonical coding-system name for the arg (alias-group head, EOL
/// suffix stripped).
pub(crate) fn coding_canonical(i: &Interp, v: &Value) -> String {
    let mut name = match v {
        Value::Nil => "no-conversion".to_string(),
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return String::new(),
    };
    for _ in 0..32 {
        let base = eol_split(&name).0.to_string();
        if let Some((_, target)) = i
            .extra_coding_aliases
            .iter()
            .find(|(n, _)| *n == name || *n == base)
        {
            name = target.clone();
            continue;
        }
        return CODING_ALIASES
            .iter()
            .find(|&&(n, _)| n == base)
            .or_else(|| {
                CODING_ALIASES
                    .iter()
                    .find(|&&(_, group)| group.contains(&base.as_str()))
            })
            .map(|&(_, group)| eol_split(group[0]).0.to_string())
            .unwrap_or(base);
    }
    eol_split(&name).0.to_string()
}

/// Follow aliases before selecting an EOL subsidiary; a bare base uses
/// automatic EOL detection when decoding and LF when encoding.
pub(crate) fn coding_fixed_eol(i: &Interp, v: &Value) -> Option<usize> {
    let Value::Sym(s) = v else { return Some(0) };
    let mut name = i.symbol_name(*s).to_string();
    for _ in 0..32 {
        if let Some((_, target)) = i.extra_coding_aliases.iter().find(|(n, _)| *n == name) {
            name = target.clone();
        } else {
            if let Some(eol) = eol_split(&name).1 {
                return Some(eol);
            }
            return match name.as_str() {
                "unix" | "no-conversion" | "binary" => Some(0),
                "dos" => Some(1),
                "mac" => Some(2),
                _ => None,
            };
        }
    }
    None
}

fn encode_coding_text(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    encode_process_text(i, a, None, false)
}

/// Pure conversion shared by strings and process encoders. Process callers
/// keep ISO designation state and suppress a signature already emitted.
pub(crate) fn encode_process_text(
    i: &mut Interp,
    a: crate::lisp::Argv,
    iso: Option<&mut super::coding_iso2022::EncoderState>,
    signature_written: bool,
) -> EvalResult {
    let text = want_string(i, &a[0])?;
    f_check_coding_system(i, vec![a[1].clone()].into())?;
    let canonical = coding_canonical(i, &a[1]);
    let coding = Value::Sym(i.intern(&canonical));
    let inhibit = i.q("inhibit-eol-conversion");
    let eol = if i.symbol_value(inhibit).truthy() {
        0
    } else {
        coding_fixed_eol(i, &a[1]).unwrap_or(0)
    };
    let unibyte = matches!(&a[0], Value::Str(r) if i.is_unibyte_str(r));
    let intervals = match &a[0] {
        Value::Str(r) => i.str_props(r).to_vec(),
        _ => Vec::new(),
    };
    let charset = i.q("charset");
    let mut preferred = Vec::new();
    let mut input = String::new();
    for (pos, ch) in text.chars().enumerate() {
        let preference = intervals
            .iter()
            .filter(|(start, end, _)| *start <= pos && pos < *end)
            .find_map(|(_, _, pl)| {
                pl.chunks_exact(2).find_map(|pair| {
                    if matches!(pair[0], Value::Sym(id) if id == charset) {
                        i.sym_id(&pair[1]).map(|id| i.symbol_name(id).to_string())
                    } else {
                        None
                    }
                })
            });
        preferred.push(preference.clone());
        if ch == '\n' && eol != 0 {
            input.push('\r');
            if eol == 1 {
                input.push('\n');
                preferred.push(preference);
            }
        } else if unibyte && (128..=255).contains(&(ch as u32)) {
            input.push(crate::lisp::value::lisp_char(0x3fff00 + ch as u32).unwrap());
        } else {
            input.push(ch);
        }
    }
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let default_key = i.q(":default-char");
    let default = crate::lisp::eval::plist_get(&plist, default_key)
        .int()
        .unwrap_or(32) as u32;
    let iso = if let Some(state) = iso {
        super::coding_iso2022::encode_stream(i, &coding, &input, false, &preferred, state)
    } else {
        super::coding_iso2022::encode_with_preferences(i, &coding, &input, true, &preferred)
    };
    let bytes = if let Some(mut bytes) = super::coding_unicode::encode(&canonical, &input, default)
    {
        if signature_written {
            if (canonical == "utf-16" || canonical.ends_with("-with-signature"))
                && canonical.starts_with("utf-16")
            {
                bytes.drain(..2);
            } else if matches!(canonical.as_str(), "utf-8-auto" | "utf-8-with-signature") {
                bytes.drain(..3);
            }
        }
        bytes
    } else if canonical == "chinese-hz" {
        encode_hz(i, &input)
    } else if let Some(bytes) = iso {
        bytes
    } else if let Some(bytes) = super::coding_legacy::encode(i, &coding, &input, true) {
        bytes
    } else {
        let plist = f_coding_system_plist(i, vec![coding].into())?;
        let dc = crate::lisp::eval::plist_get(&plist, i.q(":default-char"));
        let default = dc.int().unwrap_or(32) as u8;
        input
            .chars()
            .flat_map(|ch| {
                if let Some(b) = crate::lisp::value::eight_bit_byte(ch) {
                    vec![b]
                } else {
                    encode_one_char(&canonical, ch as u32).unwrap_or_else(|| vec![default])
                }
            })
            .collect()
    };
    let value = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &value {
        i.mark_unibyte(r);
    }
    Ok(value)
}

fn encode_hz(i: &Interp, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut shifted = false;
    for ch in text.chars() {
        let c = crate::lisp::value::lisp_char_code(ch);
        let code = if c >= 128 {
            super::charset::charset_encode(i, "chinese-gb2312", c as i64)
        } else {
            None
        };
        if let Some(code) = code {
            if !shifted {
                out.extend_from_slice(b"~{");
                shifted = true;
            }
            out.extend_from_slice(&[(code >> 8) as u8, code as u8]);
        } else {
            if shifted {
                out.extend_from_slice(b"~}");
                shifted = false;
            }
            if c == 126 {
                out.extend_from_slice(b"~~");
            } else if c < 128 {
                out.push(c as u8);
            } else {
                out.extend_from_slice(
                    format!(
                        "{}{:0width$X}",
                        if c < 65536 { "\\u" } else { "\\U" },
                        c,
                        width = if c < 65536 { 4 } else { 8 }
                    )
                    .as_bytes(),
                );
            }
        }
    }
    if shifted {
        out.extend_from_slice(b"~}");
    }
    out
}

fn decode_hz(i: &mut Interp, bytes: &[u8]) -> Result<(String, Vec<(usize, usize, String)>), Flow> {
    // HZ's Lisp post-read conversion operates after UTF-8 decoding, and
    // only converts complete contiguous GB pairs following a ~{ marker.
    let chars: Vec<char> = super::coding_legacy::internal_decode(bytes)
        .chars()
        .collect();
    let mut out = String::new();
    let mut spans = Vec::new();
    let mut pos = 0;
    let mut at = 0;
    let continuation = i
        .intern_soft("decode-hz-line-continuation")
        .is_some_and(|id| i.symbol_value(id).truthy());
    while at < chars.len() {
        if chars[at] == '~' && at + 1 == chars.len() {
            return Err(i.wrong_type_mut("wholenump", &Value::Nil));
        }
        if chars[at] == '~' && at + 1 < chars.len() {
            match chars[at + 1] {
                '{' => {
                    at += 2;
                    let start = pos;
                    while at + 1 < chars.len()
                        && ('!'..='}').contains(&chars[at])
                        && ('!'..='~').contains(&chars[at + 1])
                    {
                        let code = ((chars[at] as i64) << 8) | chars[at + 1] as i64;
                        if let Some(c) = super::charset::charset_decode(i, "chinese-gb2312", code) {
                            out.push(
                                crate::lisp::value::lisp_char(c as u32)
                                    .unwrap_or_else(|| crate::lisp::value::mule_seq_char(c as u32)),
                            );
                            pos += 1;
                            at += 2;
                        } else {
                            break;
                        }
                    }
                    if pos > start {
                        spans.push((start, pos, "chinese-gb2312".to_string()));
                    }
                    if chars.get(at) == Some(&'~') && chars.get(at + 1) == Some(&'}') {
                        at += 2;
                    }
                    continue;
                }
                '~' => {
                    out.push('~');
                    pos += 1;
                    at += 2;
                    continue;
                }
                '\n' if continuation => {
                    at += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[at]);
        pos += 1;
        at += 1;
    }
    Ok((out, spans))
}

fn decode_coding_bytes(
    i: &mut Interp,
    coding: &Value,
    canonical: &str,
    bytes: &[u8],
    force_conversion: bool,
) -> Result<Value, Flow> {
    super::coding_legacy::validate_decode(i, coding, bytes)?;
    let converted = if canonical.starts_with("utf-16") {
        let units: Vec<i32> = bytes.iter().map(|b| *b as i32).collect();
        let name = super::coding_unicode::resolve(canonical, &units, false);
        let text = super::coding_unicode::decode_utf16(&name, &units);
        Some((
            super::coding_unicode::normalize_eol(&text, coding_fixed_eol(i, coding)).0,
            Vec::new(),
        ))
    } else if matches!(canonical, "utf-8-with-signature" | "utf-8-auto") {
        let bytes = bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes);
        Some((decode_file_utf8(bytes), Vec::new()))
    } else if canonical == "chinese-hz" {
        Some(decode_hz(i, bytes)?)
    } else if let Some(decoded) = super::coding_iso2022::decode(i, coding, bytes) {
        Some(decoded)
    } else {
        super::coding_legacy::decode(i, coding, bytes, force_conversion)
    };
    let (text, spans, metadata) = match converted {
        Some((text, spans)) => (text, spans, true),
        None => (
            if matches!(canonical, "utf-8" | "utf-8-auto" | "undecided") {
                decode_file_utf8(bytes)
            } else {
                decode_bytes(canonical, bytes)
            },
            Vec::new(),
            false,
        ),
    };
    let value = Value::string(text);
    if let Value::Str(r) = &value {
        i.mark_multibyte(r);
        if metadata {
            let charset = i.q("charset");
            let props = spans
                .into_iter()
                .map(|(start, end, name)| {
                    (
                        start,
                        end,
                        vec![Value::Sym(charset), Value::Sym(i.intern(&name))],
                    )
                })
                .collect();
            i.set_str_props(r, props);
        } else {
            attach_decode_charset_props(i, r, canonical);
        }
    }
    Ok(value)
}

/// Decode bytes to chars for CANONICAL coding names — the inverse of
/// `encode_one_char' using the same generated tables.
fn decode_bytes(canonical: &str, bytes: &[u8]) -> String {
    use super::enc_tables::*;
    let utf8ish = matches!(
        canonical,
        "utf-8"
            | "mule-utf-8"
            | "cp65001"
            | "utf-8-auto"
            | "utf-8-emacs"
            | "utf-8-hfs"
            | "utf-8-nfd"
            | "utf-8-with-signature"
            | "prefer-utf-8"
            | "no-conversion"
            | "raw-text"
            | "binary"
            | "no-conversion-multibyte"
            | "undecided"
    );
    if utf8ish {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if matches!(canonical, "iso-latin-1") {
        // Latin-1 is the identity byte→char map.
        return bytes.iter().map(|&b| b as char).collect();
    }
    let single: Option<&[(u32, u64)]> = match canonical {
        "iso-latin-2" | "iso-latin-3" | "iso-latin-4" | "iso-latin-5" | "iso-latin-9"
        | "iso-latin-10" | "us-ascii" => Some(&[]),
        "cyrillic-koi8" => Some(ENC_KOI8_R),
        "windows-1251" => Some(ENC_WINDOWS_1251),
        "mac-roman" => Some(ENC_MAC_ROMAN),
        _ => None,
    };
    if let Some(t) = single {
        return bytes
            .iter()
            .map(|&b| {
                if b < 0x80 {
                    b as char
                } else {
                    t.iter()
                        .find(|(_, p)| (p >> 56) == 1 && ((p >> 48) & 0xFF) == b as u64)
                        .and_then(|(u, _)| char::from_u32(*u))
                        .unwrap_or('?')
                }
            })
            .collect();
    }
    let multi: Option<&[(u32, u64)]> = match canonical {
        "japanese-shift-jis" | "japanese-cp932" => Some(ENC_SHIFT_JIS),
        "japanese-iso-8bit" => Some(ENC_EUC_JP),
        "chinese-big5" => Some(ENC_BIG5),
        "chinese-iso-8bit" | "chinese-gbk" => Some(ENC_GB2312),
        _ => None,
    };
    if let Some(t) = multi {
        let mut out = String::new();
        let mut ix = 0;
        while ix < bytes.len() {
            let b = bytes[ix];
            if b < 0x80 {
                out.push(b as char);
                ix += 1;
                continue;
            }
            // Try the longest sequence first (up to 4 bytes).
            let mut matched = false;
            for len in (2..=4usize).rev() {
                if ix + len > bytes.len() {
                    continue;
                }
                let seq = &bytes[ix..ix + len];
                if let Some((u, _)) = t.iter().find(|(_, p)| {
                    let n = (p >> 56) as usize;
                    n == len
                        && seq
                            .iter()
                            .enumerate()
                            .all(|(k, &bb)| ((p >> (48 - 8 * k)) & 0xFF) == bb as u64)
                }) {
                    if let Some(c) = char::from_u32(*u) {
                        out.push(c);
                        matched = true;
                    }
                    ix += len;
                    break;
                }
            }
            if !matched {
                out.push('?');
                ix += 1;
            }
        }
        return out;
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// GNU records the decoding charset of each produced char as a
/// `charset' text property.  Single-charset coding systems mark every
/// char (incl. ASCII) once any non-ASCII char exists; multi-charset
/// systems (gb2312, euc-jp, sjis, big5) mark a char with the charset
/// that decoded it, and ASCII chars inherit the "current" charset
/// (unmarked before the first non-ASCII char).  utf-8 et al. record
/// nothing.
fn attach_decode_charset_props(i: &mut Interp, r: &StrRef, canonical: &str) {
    enum Mode {
        Single(&'static str),
        Multi(&'static [&'static str]),
    }
    let mode = match canonical {
        "iso-latin-1" => Mode::Single("iso-8859-1"),
        "iso-latin-2" => Mode::Single("iso-8859-2"),
        "iso-latin-3" => Mode::Single("iso-8859-3"),
        "iso-latin-4" => Mode::Single("iso-8859-4"),
        "iso-latin-5" => Mode::Single("iso-8859-9"),
        "iso-latin-9" => Mode::Single("iso-8859-15"),
        "iso-latin-10" => Mode::Single("iso-8859-16"),
        "cyrillic-koi8" => Mode::Single("koi8-r"),
        "windows-1251" => Mode::Single("windows-1251"),
        "mac-roman" => Mode::Single("mac-roman"),
        "japanese-shift-jis" | "japanese-cp932" => {
            Mode::Multi(&["ascii", "katakana-jisx0201", "japanese-jisx0208"])
        }
        "japanese-iso-8bit" => Mode::Multi(&[
            "ascii",
            "latin-jisx0201",
            "japanese-jisx0208",
            "katakana-jisx0201",
            "japanese-jisx0212",
            "japanese-jisx0208-1978",
        ]),
        "chinese-big5" => Mode::Multi(&["ascii", "big5"]),
        "chinese-iso-8bit" | "chinese-gbk" => Mode::Multi(&["ascii", "chinese-gb2312"]),
        _ => return,
    };
    let chars: Vec<char> = r.borrow().chars().collect();
    if !chars.iter().any(|&c| (c as u32) >= 0x80) {
        return;
    }
    let charset_id = i.q("charset");
    let mk = |i: &mut Interp, name: &str| -> Vec<Value> {
        vec![Value::Sym(charset_id), Value::Sym(i.intern(name))]
    };
    match mode {
        Mode::Single(name) => {
            let pl = mk(i, name);
            i.set_str_props(r, vec![(0, chars.len(), pl)]);
        }
        Mode::Multi(restriction) => {
            // Runs of (start, end, charset-name); merged at the end.
            let mut runs: Vec<(usize, usize, &'static str)> = Vec::new();
            let mut cur: Option<&'static str> = None;
            for (ix, &c) in chars.iter().enumerate() {
                let u = c as u32;
                if u >= 0x80 {
                    if let Some(name) =
                        crate::lisp::builtins::strfn::char_charset_in(u as i128, restriction)
                    {
                        cur = Some(name);
                    }
                }
                if let Some(name) = cur {
                    if let Some(last) = runs.last_mut() {
                        if last.1 == ix && last.2 == name {
                            last.1 = ix + 1;
                            continue;
                        }
                    }
                    runs.push((ix, ix + 1, name));
                }
            }
            if !runs.is_empty() {
                let ivs: Vec<(usize, usize, Vec<Value>)> = runs
                    .into_iter()
                    .map(|(s, e, name)| (s, e, mk(i, name)))
                    .collect();
                i.set_str_props(r, ivs);
            }
        }
    }
}

/// File decoding keeps invalid input bytes as GNU eight-bit characters.
/// Rust's lossy UTF-8 conversion would replace them irreversibly with U+FFFD.
fn decode_file_utf8(mut bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                out.push_str(std::str::from_utf8(&bytes[..valid]).unwrap());
                let invalid = error.error_len().unwrap_or(bytes.len() - valid);
                for &byte in &bytes[valid..valid + invalid] {
                    out.push(crate::lisp::value::lisp_char(0x3FFF00 + byte as u32).unwrap());
                }
                bytes = &bytes[valid + invalid..];
            }
        }
    }
    out
}

/// Apply the user's file coding detector to a raw, unibyte sample. GNU
/// samples the first KiB and last three KiB without modifying the target.
pub(crate) fn auto_file_coding(i: &mut Interp, path: &str, bytes: &[u8]) -> EvalResult {
    let function = i.q("set-auto-coding-function");
    let function = i.symbol_value(function);
    if function.is_nil() {
        return Ok(Value::Nil);
    }
    let directory = i.q("default-directory");
    let directory_value = i.symbol_value(directory);
    let sample = if bytes.len() > 4096 {
        [&bytes[..1024], &bytes[bytes.len() - 3072..]].concat()
    } else {
        bytes.to_vec()
    };
    let source = Value::string(sample.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &source {
        i.mark_unibyte(r);
    }
    let saved_match = i.match_data.clone();
    let result = with_coding_buffer(i, &source, false, |i| {
        i.set_symbol(directory, directory_value)?;
        crate::buffer::primitives::cur(i).borrow_mut().set_point(0);
        i.apply(
            &function,
            vec![Value::string(path), Value::Int(sample.len() as i128)],
        )
    });
    i.match_data = saved_match;
    result
}

pub(crate) fn coding_inherit_file_eol(
    i: &mut Interp,
    coding: &Value,
    parent: &Value,
) -> EvalResult {
    let coding = if coding.is_nil() {
        Value::Sym(i.q("raw-text"))
    } else {
        coding.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    if coding_fixed_eol(i, &coding).is_some() {
        return Ok(coding);
    }
    if !parent.is_nil() {
        f_check_coding_system(i, vec![parent.clone()].into())?;
    }
    let eol = if parent.is_nil() {
        0
    } else {
        coding_fixed_eol(i, parent).unwrap_or(0)
    };
    let base = coding_canonical(i, &coding);
    Ok(Value::Sym(i.intern(&format!(
        "{}-{}",
        base,
        ["unix", "dos", "mac"][eol]
    ))))
}

pub(crate) fn raw_file_coding(i: &mut Interp, coding: &Value) -> EvalResult {
    let base = coding_canonical(i, coding);
    if matches!(base.as_str(), "no-conversion" | "raw-text") {
        return Ok(coding.clone());
    }
    let name = match coding_fixed_eol(i, coding) {
        Some(n) => format!("raw-text-{}", ["unix", "dos", "mac"][n]),
        None => "raw-text".into(),
    };
    Ok(Value::Sym(i.intern(&name)))
}

/// Decode external file bytes with the common codecs. File detection
/// keeps incomplete terminal sequences as candidates until EOF is known.
pub(crate) fn decode_file_bytes(
    i: &mut Interp,
    bytes: &[u8],
    requested: &Value,
    literal: bool,
) -> Result<(Value, Value), Flow> {
    let multibyte = i.q("enable-multibyte-characters");
    let multibyte = i.symbol_value(multibyte).truthy();
    let mut coding = if literal {
        Value::Sym(i.q("no-conversion"))
    } else if requested.is_nil() {
        Value::Sym(i.q("undecided"))
    } else if i
        .sym_id(requested)
        .is_some_and(|s| i.symbol_name(s) == "auto-save-coding")
    {
        Value::Sym(i.q("utf-8-emacs-unix"))
    } else {
        requested.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    if !multibyte {
        coding = raw_file_coding(i, &coding)?;
    }
    if coding_canonical(i, &coding) == "undecided" {
        let fixed = coding_fixed_eol(i, &coding);
        coding = if bytes.starts_with(&[255, 254]) {
            Value::Sym(i.q("utf-16le-with-signature"))
        } else if bytes.starts_with(&[254, 255]) {
            Value::Sym(i.q("utf-16be-with-signature"))
        } else {
            super::coding_detection::detect_file(
                i,
                &bytes.iter().map(|b| *b as i32).collect::<Vec<_>>(),
            )?
        };
        if let Some(eol) = fixed {
            if coding_canonical(i, &coding) != "no-conversion" {
                let base = coding_canonical(i, &coding);
                coding = Value::Sym(i.intern(&format!("{}-{}", base, ["unix", "dos", "mac"][eol])));
            }
        }
    }
    let source = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &source {
        i.mark_unibyte(r);
    }
    let (decoded, used, _) = decode_region_text(i, &source, &coding)?;
    if !multibyte {
        if let Value::Str(r) = &decoded {
            i.mark_unibyte(r);
        }
    }
    Ok((decoded, used))
}

pub(crate) fn process_encode_source(i: &mut Interp, source: Value, coding: &Value) -> EvalResult {
    let hook = region_conversion_hook(i, coding, true)?;
    if hook.is_nil() {
        return Ok(source);
    }
    with_coding_buffer(i, &source, true, |i| {
        let len = crate::buffer::primitives::cur(i).borrow().text_len();
        coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
        coding_buffer_string(i)
    })
}

pub(crate) fn process_decode_bytes(
    i: &mut Interp,
    bytes: &[u8],
    coding: &Value,
    iso: &mut super::coding_iso2022::DecoderState,
    first: bool,
    finish: bool,
) -> Result<(Value, Value), Flow> {
    let source = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &source {
        i.mark_unibyte(r);
    }
    let coding = if coding.is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        coding.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let (converted, used, raw) =
        decode_region_text_stream(i, &source, &coding, Some(iso), first, finish)?;
    Ok((coding_string_result(i, &converted, raw)?, used))
}

pub(crate) fn process_decode_result(i: &mut Interp, value: Value, coding: &Value) -> EvalResult {
    let hook = region_conversion_hook(i, coding, false)?;
    if hook.is_nil() {
        return Ok(value);
    }
    let count = want_string(i, &value)?.chars().count();
    with_coding_buffer(i, &value, false, |i| {
        region_post_read(i, &hook, 0, count)?;
        coding_buffer_string(i)
    })
}

pub(crate) fn f_encode_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_string(i, a, true)
}

pub(crate) fn f_decode_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_string(i, a, false)
}

/// Encode one Unicode scalar to bytes for CANONICAL (alias-group
/// head) coding-system names, mirroring GNU's `encode-coding-char'.
/// Returns None when the char is not encodable.
fn encode_one_char(canonical: &str, c: u32) -> Option<Vec<u8>> {
    use super::enc_tables::*;
    if c < 0x80 {
        return Some(vec![c as u8]);
    }
    let table: Option<&[(u32, u64)]> = match canonical {
        "japanese-shift-jis" | "japanese-cp932" => Some(ENC_SHIFT_JIS),
        "japanese-iso-8bit" => Some(ENC_EUC_JP),
        "cyrillic-koi8" => Some(ENC_KOI8_R),
        "windows-1251" => Some(ENC_WINDOWS_1251),
        "mac-roman" => Some(ENC_MAC_ROMAN),
        "chinese-big5" => Some(ENC_BIG5),
        "chinese-iso-8bit" | "chinese-gbk" => Some(ENC_GB2312),
        _ => None,
    };
    if let Some(t) = table {
        return t.binary_search_by_key(&c, |&(u, _)| u).ok().map(|ix| {
            let p = t[ix].1;
            let n = (p >> 56) as usize;
            p.to_be_bytes()[1..1 + n].to_vec()
        });
    }
    match canonical {
        "utf-8"
        | "mule-utf-8"
        | "cp65001"
        | "utf-8-auto"
        | "utf-8-emacs"
        | "utf-8-hfs"
        | "utf-8-nfd"
        | "utf-8-with-signature"
        | "prefer-utf-8"
        | "no-conversion"
        | "raw-text"
        | "binary"
        | "no-conversion-multibyte" => char::from_u32(c).map(|ch| ch.to_string().into_bytes()),
        "iso-latin-1" | "iso-latin-2" | "iso-latin-3" | "iso-latin-4" | "iso-latin-5"
        | "iso-latin-9" | "iso-latin-10" => {
            if c <= 0xFF {
                Some(vec![c as u8])
            } else {
                None
            }
        }
        "utf-16" | "utf-16be" | "utf-16be-with-signature" => {
            Some(vec![(c >> 8) as u8, (c & 0xFF) as u8])
        }
        "utf-16le" | "utf-16le-with-signature" => Some(vec![(c & 0xFF) as u8, (c >> 8) as u8]),
        _ => None,
    }
}

fn f_encode_coding_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (encode-coding-char CHAR CODING-SYSTEM &optional CHARSET) —
    // GNU returns a unibyte string of the encoded bytes.
    let c = match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= 0x3FFFFF => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let _ = f_check_coding_system(i, vec![a[1].clone()].into())?;
    let canonical = coding_canonical(i, &a[1]);
    Ok(match encode_one_char(&canonical, c) {
        Some(bs) => {
            let sv = Value::string(bs.iter().map(|&b| b as char).collect::<String>());
            if let Value::Str(r) = &sv {
                i.mark_unibyte(r);
            }
            sv
        }
        None => Value::Nil,
    })
}

/// Decode a region as external octets plus already decoded native
/// characters (GNU's negative charbuf entries). Charset annotations
/// survive native entries; ISO-2022 also resynchronizes graphic plane 0.
fn decode_region_text(
    i: &mut Interp,
    source: &Value,
    requested: &Value,
) -> Result<(Value, Value, bool), Flow> {
    decode_region_text_stream(i, source, requested, None, true, true)
}

fn decode_region_text_stream(
    i: &mut Interp,
    source: &Value,
    requested: &Value,
    mut iso: Option<&mut super::coding_iso2022::DecoderState>,
    first: bool,
    finish: bool,
) -> Result<(Value, Value, bool), Flow> {
    let text = want_string(i, source)?;
    let unibyte = matches!(source, Value::Str(r) if i.is_unibyte_str(r));
    let mut bytes = Vec::new();
    let mut native = Vec::new();
    let mut storage_bytes = 0;
    for c in text.chars() {
        let raw = crate::lisp::value::eight_bit_byte(c);
        let decoded = !unibyte && raw.is_none() && c as u32 >= 128;
        bytes.push(if decoded { 0 } else { raw.unwrap_or(c as u8) });
        native.push(decoded.then_some(c));
        storage_bytes += if unibyte || c.is_ascii() {
            1
        } else if raw.is_some() {
            2
        } else {
            super::coding_legacy::internal_encode(crate::lisp::value::lisp_char_code(c) as u32)
                .len()
        };
    }
    let original = coding_canonical(i, requested);
    let fixed = coding_fixed_eol(i, requested);
    let inhibit = i.q("inhibit-eol-conversion");
    let inhibit = i.symbol_value(inhibit).truthy();
    let mut canonical = original.clone();
    if original == "undecided" {
        let coding = f_detect_coding_string(i, vec![source.clone(), Value::t()].into())?;
        canonical = coding_canonical(i, &coding);
    }
    let units: Vec<i32> = bytes
        .iter()
        .enumerate()
        .map(|(p, b)| {
            native[p]
                .map(|c| -(crate::lisp::value::lisp_char_code(c) as i32))
                .unwrap_or(*b as i32)
        })
        .collect();
    canonical = super::coding_unicode::resolve(&canonical, &units, !unibyte);
    if canonical.starts_with("utf-16") {
        let decode_name = if !first {
            canonical
                .strip_suffix("-with-signature")
                .unwrap_or(&canonical)
        } else {
            &canonical
        };
        let text = super::coding_unicode::decode_utf16(decode_name, &units);
        let (text, detected) =
            super::coding_unicode::normalize_eol(&text, if inhibit { Some(0) } else { fixed });
        let eol = if inhibit { None } else { detected };
        let used = if canonical == original && (fixed.is_some() || eol.is_none()) {
            requested.clone()
        } else {
            Value::Sym(i.intern(&match eol {
                Some(n) => format!("{}-{}", canonical, ["unix", "dos", "mac"][n]),
                None => canonical,
            }))
        };
        let value = Value::string(text);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
        }
        return Ok((value, used, false));
    }
    let mut seen = [false; 3];
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\r' if bytes.get(at + 1) == Some(&b'\n') => {
                seen[1] = true;
                at += 1;
            }
            b'\r' => seen[2] = true,
            b'\n' => seen[0] = true,
            _ => {}
        }
        at += 1;
    }
    let detected = match seen.iter().filter(|v| **v).count() {
        0 => None,
        1 => seen.iter().position(|v| *v),
        _ => Some(0),
    };
    let eol = if inhibit { None } else { fixed.or(detected) };
    let used = if canonical == original && (fixed.is_some() || eol.is_none()) {
        requested.clone()
    } else {
        let name = match eol {
            Some(n) => format!("{}-{}", canonical, ["unix", "dos", "mac"][n]),
            None => canonical.clone(),
        };
        Value::Sym(i.intern(&name))
    };
    let force = bytes.contains(&b'\r');
    if !canonical.starts_with("utf-16") {
        let mut input = Vec::new();
        let mut natives = Vec::new();
        at = 0;
        while at < bytes.len() {
            let crlf = eol == Some(1) && bytes[at..].starts_with(b"\r\n");
            input.push(if crlf || eol == Some(2) && bytes[at] == b'\r' {
                b'\n'
            } else {
                bytes[at]
            });
            natives.push(native[at]);
            at += if crlf { 2 } else { 1 };
        }
        bytes = input;
        native = natives;
    }
    let raw = matches!(canonical.as_str(), "raw-text" | "no-conversion");
    let codec = Value::Sym(i.intern(&canonical));
    let converted = if raw {
        Some((
            bytes
                .iter()
                .enumerate()
                .map(|(p, b)| {
                    native[p].unwrap_or_else(|| {
                        if *b < 128 {
                            *b as char
                        } else {
                            crate::lisp::value::lisp_char(0x3fff00 + *b as u32).unwrap()
                        }
                    })
                })
                .collect(),
            Vec::new(),
        ))
    } else if let Some(result) = if let Some(state) = iso.as_deref_mut() {
        super::coding_iso2022::decode_stream(i, &codec, &bytes, state, finish)
    } else {
        super::coding_iso2022::decode_with_native(i, &codec, &bytes, &native, storage_bytes)
    } {
        Some(result)
    } else if native.iter().any(Option::is_some)
        && !matches!(canonical.as_str(), "utf-8-emacs" | "utf-7" | "utf-7-imap")
    {
        super::coding_legacy::decode_with_native(i, &codec, &bytes, &native, force)
    } else {
        None
    };
    let value = if let Some((text, spans)) = converted {
        let value = Value::string(text);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
            let charset = i.q("charset");
            let props = spans
                .into_iter()
                .map(|(s, e, name)| (s, e, vec![Value::Sym(charset), Value::Sym(i.intern(&name))]))
                .collect();
            i.set_str_props(r, props);
        }
        value
    } else {
        // Byte decoders restart at native characters; these are never
        // reinterpreted using the external coding system.
        let mut out = String::new();
        let mut props = Vec::new();
        let mut offset = 0;
        at = 0;
        while at < bytes.len() {
            if let Some(c) = native[at] {
                out.push(c);
                offset += 1;
                at += 1;
                continue;
            }
            let start = at;
            while at < bytes.len() && native[at].is_none() {
                at += 1;
            }
            let part = &bytes[start..at];
            let segment_canonical = if (start > 0 || !first)
                && matches!(canonical.as_str(), "utf-8-auto" | "utf-8-with-signature")
            {
                "utf-8"
            } else {
                &canonical
            };
            let decoded = decode_coding_bytes(i, &codec, segment_canonical, part, force)?;
            if let Value::Str(r) = decoded {
                props.extend(
                    i.str_props(&r)
                        .iter()
                        .map(|(s, e, pl)| (s + offset, e + offset, pl.clone())),
                );
                offset += r.borrow().chars().count();
                out.push_str(&r.borrow());
            }
        }
        let value = Value::string(out);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
            i.set_str_props(r, props);
        }
        value
    };
    Ok((value, used, raw))
}

/// Run Lisp conversion hooks in a disposable buffer, as coding.c does.
/// Its text properties and byte mode are part of the conversion input.
fn with_coding_buffer(
    i: &mut Interp,
    source: &Value,
    encode: bool,
    f: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    let old = i.current_buffer;
    let id = i.create_buffer(" *code-conversion-work*");
    i.set_current_buffer(id);
    let mb = matches!(source, Value::Str(r) if !i.is_unibyte_str(r)
        && (!encode || r.borrow().chars().any(|c| !c.is_ascii())));
    let multibyte = i.q("enable-multibyte-characters");
    i.set_buffer_local_toplevel_value(multibyte, id, if mb { Value::t() } else { Value::Nil });
    let buffer = crate::buffer::primitives::cur(i);
    buffer.borrow_mut().inhibit_buffer_hooks = true;
    if let Value::Str(r) = source {
        let storage = if mb {
            r.borrow().clone()
        } else {
            lisp_string_bytes(i, source)
                .into_iter()
                .map(|b| b as char)
                .collect()
        };
        let mut b = buffer.borrow_mut();
        let undo = b.undo_sym;
        b.locals.insert(undo, Value::t());
        b.raw_insert_at(0, &storage);
        b.text.props_write().graft(0, i.str_props(r), false);
    }
    let result = f(i);
    i.set_current_buffer(old);
    let _ = crate::buffer::primitives::kill_buffer_keep_current(i, id);
    result
}

fn coding_buffer_string(i: &mut Interp) -> EvalResult {
    let buffer = crate::buffer::primitives::cur(i);
    let b = buffer.borrow();
    let value = Value::string(b.text.substring(0, b.text_len()));
    if let Value::Str(r) = &value {
        i.set_str_props(r, b.text.props.borrow().slice(0, b.text_len()));
        let multibyte = i.q("enable-multibyte-characters");
        if i.symbol_value(multibyte).truthy() {
            i.mark_multibyte(r);
        } else {
            i.mark_unibyte(r);
        }
    }
    Ok(value)
}

fn region_conversion_hook(i: &mut Interp, coding: &Value, encode: bool) -> Result<Value, Flow> {
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let property = i.intern(if encode {
        ":pre-write-conversion"
    } else {
        ":post-read-conversion"
    });
    let hook = crate::lisp::eval::plist_get(&plist, property);
    // These Lisp hooks are already implemented by the shared byte codecs.
    let implemented = i.sym_id(&hook).is_some_and(|s| {
        matches!(
            i.symbol_name(s).as_str(),
            "utf-7-pre-write-conversion"
                | "utf-7-post-read-conversion"
                | "utf-7-imap-pre-write-conversion"
                | "utf-7-imap-post-read-conversion"
                | "post-read-decode-hz"
                | "pre-write-encode-hz"
        )
    });
    Ok(if implemented { Value::Nil } else { hook })
}

fn coding_safe_call(i: &mut Interp, hook: &Value, args: Vec<Value>) -> EvalResult {
    match i.apply(hook, args) {
        Err(Flow::Signal(..) | Flow::Quit) => Ok(Value::Nil),
        result => result,
    }
}

fn region_post_read(i: &mut Interp, hook: &Value, start: usize, count: usize) -> EvalResult {
    post_read_conversion(i, hook, start, count, true)
}

pub(crate) fn post_file_conversion(
    i: &mut Interp,
    coding: &Value,
    start: usize,
    count: usize,
) -> EvalResult {
    let hook = region_conversion_hook(i, coding, false)?;
    post_read_conversion(i, &hook, start, count, false)
}

fn post_read_conversion(
    i: &mut Interp,
    hook: &Value,
    start: usize,
    count: usize,
    safe: bool,
) -> EvalResult {
    if hook.is_nil() {
        return Ok(Value::Int(count as i128));
    }
    let buffer = crate::buffer::primitives::cur(i);
    let before = buffer.borrow().text_len();
    let undo = buffer.borrow().undo_sym;
    let saved_undo = buffer.borrow_mut().locals.insert(undo, Value::t());
    buffer.borrow_mut().set_point(start);
    let result = if safe {
        coding_safe_call(i, hook, vec![Value::Int(count as i128)])
    } else {
        i.apply(hook, vec![Value::Int(count as i128)])
    };
    if let Some(saved) = saved_undo {
        buffer.borrow_mut().locals.insert(undo, saved);
    } else {
        buffer.borrow_mut().locals.shift_remove(&undo);
    }
    let value = result?;
    if !value.fixnump() || value.int().unwrap_or(-1) < 0 {
        return Err(i.wrong_type_mut("wholenump", &value));
    }
    Ok(Value::Int(
        count as i128 + buffer.borrow().text_len() as i128 - before as i128,
    ))
}

fn coding_storage_text(
    i: &mut Interp,
    converted: &Value,
    encode: bool,
    raw: bool,
) -> Result<String, Flow> {
    let text = want_string(i, converted)?;
    let unibyte_id = i.q("enable-multibyte-characters");
    let unibyte = !i.symbol_value(unibyte_id).truthy();
    Ok(if encode {
        lisp_string_bytes(i, converted)
            .into_iter()
            .map(|b| {
                if b < 128 || unibyte {
                    b as char
                } else {
                    crate::lisp::value::lisp_char(0x3fff00 + b as u32).unwrap()
                }
            })
            .collect::<String>()
    } else if unibyte {
        text.chars()
            .flat_map(|c| {
                if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                    vec![b]
                } else if raw && !c.is_ascii() {
                    vec![(-crate::lisp::value::lisp_char_code(c)) as u8]
                } else {
                    super::coding_legacy::internal_encode(
                        crate::lisp::value::lisp_char_code(c) as u32
                    )
                }
            })
            .map(|b| b as char)
            .collect()
    } else {
        text.clone()
    })
}

fn coding_string_result(i: &mut Interp, converted: &Value, raw: bool) -> Result<Value, Flow> {
    if !raw {
        return Ok(converted.clone());
    }
    let text = want_string(i, converted)?;
    let value = Value::string(
        text.chars()
            .map(|c| {
                crate::lisp::value::eight_bit_byte(c).unwrap_or_else(|| {
                    if c.is_ascii() {
                        c as u8
                    } else {
                        (-crate::lisp::value::lisp_char_code(c)) as u8
                    }
                }) as char
            })
            .collect::<String>(),
    );
    if let Value::Str(r) = &value {
        i.mark_unibyte(r);
    }
    Ok(value)
}

fn copy_coding_string(i: &mut Interp, source: &Value) -> EvalResult {
    let Value::Str(r) = source else {
        unreachable!()
    };
    let value = Value::string(r.borrow().clone());
    if let Value::Str(copy) = &value {
        i.set_str_props(copy, i.str_props(r).to_vec());
        if i.is_unibyte_str(r) {
            i.mark_unibyte(copy);
        } else if i.is_multibyte_str(r) {
            i.mark_multibyte(copy);
        }
    }
    Ok(value)
}

fn code_convert_string(i: &mut Interp, a: crate::lisp::Argv, encode: bool) -> EvalResult {
    let text = want_string(i, &a[0])?;
    let destination = arg(&a, 3);
    let last = i.q("last-coding-system-used");
    if a[1].is_nil() && destination.is_nil() {
        let no_conversion = Value::Sym(i.q("no-conversion"));
        i.set_symbol(last, no_conversion)?;
        return if arg(&a, 2).truthy() {
            Ok(a[0].clone())
        } else {
            copy_coding_string(i, &a[0])
        };
    }
    let coding = if a[1].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[1].clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let target = match &destination {
        Value::Nil | Value::Sym(sym::T) => None,
        Value::Buffer(b) => Some(b.borrow().id),
        v => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let ascii_key = i.q(":ascii-compatible-p");
    let inhibit = i.q("inhibit-eol-conversion");
    if target.is_none()
        && text.is_ascii()
        && crate::lisp::eval::plist_get(&plist, ascii_key).truthy()
        && (coding_fixed_eol(i, &coding) == Some(0)
            || i.symbol_value(inhibit).truthy()
            || !text.contains(if encode { '\n' } else { '\r' }))
    {
        i.set_symbol(last, coding)?;
        if arg(&a, 2).truthy() {
            return Ok(a[0].clone());
        }
        let value = Value::string(text);
        if let Value::Str(r) = &value {
            if encode {
                i.mark_unibyte(r);
            } else {
                i.mark_multibyte(r);
            }
        }
        return Ok(value);
    }
    let hook = region_conversion_hook(i, &coding, encode)?;
    let mut source = a[0].clone();
    if encode && !hook.is_nil() {
        source = with_coding_buffer(i, &source, true, |i| {
            let len = crate::buffer::primitives::cur(i).borrow().text_len();
            coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
            coding_buffer_string(i)
        })?;
    }
    let (converted, used, raw) = if encode {
        (
            encode_coding_text(i, vec![source, coding.clone()].into())?,
            coding,
            false,
        )
    } else {
        decode_region_text(i, &source, &coding)?
    };
    let count = want_string(i, &converted)?.chars().count();
    let result = if let Some(target) = target {
        crate::buffer::primitives::chg_with_buffer(i, target, |i| {
            let storage = coding_storage_text(i, &converted, encode, raw)?;
            let point = crate::buffer::primitives::cur(i).borrow().point();
            crate::buffer::primitives::chg_coding_region(
                i,
                point,
                point,
                &storage,
                &converted,
                false,
                encode,
                |i| {
                    if encode {
                        Ok(Value::Int(count as i128))
                    } else {
                        region_post_read(i, &hook, point, count)
                    }
                },
            )
        })?
    } else {
        let result = coding_string_result(i, &converted, raw)?;
        if !encode && !hook.is_nil() {
            with_coding_buffer(i, &result, false, |i| {
                region_post_read(i, &hook, 0, count)?;
                coding_buffer_string(i)
            })?
        } else {
            result
        }
    };
    i.set_symbol(last, used)?;
    Ok(result)
}

fn code_convert_region(i: &mut Interp, a: crate::lisp::Argv, encode: bool) -> EvalResult {
    let deactivate = i.q("deactivate-mark");
    let saved = i.symbol_value(deactivate);
    let result = code_convert_region_inner(i, a, encode);
    let _ = i.set_symbol(deactivate, saved);
    result
}

fn code_convert_region_inner(i: &mut Interp, a: crate::lisp::Argv, encode: bool) -> EvalResult {
    use crate::buffer::primitives::{chg_coding_region, chg_with_buffer};
    let coding = if a[2].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[2].clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let destination = arg(&a, 3);
    let string_result = matches!(destination, Value::Sym(sym::T));
    let source_buffer = i.current_buffer;
    let destination_buffer = match &destination {
        Value::Nil => source_buffer,
        Value::Sym(sym::T) => source_buffer,
        Value::Buffer(b) => b.borrow().id,
        v => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let (positions, mut source) = coding_region_source(i, &a)?;
    let hook = region_conversion_hook(i, &coding, encode)?;
    if encode && !hook.is_nil() {
        source = with_coding_buffer(i, &source, true, |i| {
            let len = crate::buffer::primitives::cur(i).borrow().text_len();
            coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
            coding_buffer_string(i)
        })?;
    }
    let (converted, used, raw) = if encode {
        (
            encode_coding_text(i, vec![source.clone(), coding.clone()].into())?,
            coding,
            false,
        )
    } else {
        decode_region_text(i, &source, &coding)?
    };
    let text = want_string(i, &converted)?;
    let count = text.chars().count();
    let mut result;
    if string_result {
        result = coding_string_result(i, &converted, raw)?;
        if !encode && !hook.is_nil() {
            result = with_coding_buffer(i, &result, false, |i| {
                region_post_read(i, &hook, 0, count)?;
                coding_buffer_string(i)
            })?;
        }
    } else {
        let same = source_buffer == destination_buffer;
        result = chg_with_buffer(i, destination_buffer, |i| {
            let storage = coding_storage_text(i, &converted, encode, raw)?;
            let b = crate::buffer::primitives::cur(i);
            let (start, end) = if same {
                (positions[0] as usize - 1, positions[1] as usize - 1)
            } else {
                let point = b.borrow().point();
                (point, point)
            };
            chg_coding_region(i, start, end, &storage, &converted, same, encode, |i| {
                if encode {
                    Ok(Value::Int(count as i128))
                } else {
                    region_post_read(i, &hook, start, count)
                }
            })
        })?;
    }
    let last = i.q("last-coding-system-used");
    i.set_symbol(last, used)?;
    Ok(result)
}

fn f_decode_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_region(i, a, false)
}

fn f_encode_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_region(i, a, true)
}

// ---------- multibyte ----------

fn f_identity(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(a[0].clone())
}

// ---------- display / frames ----------

fn f_not_useful(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fdisplay_save_under/Fdisplay_backing_store run
    // decode_display first: bad DISPLAY args die before the answer.
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Sym(i.q("not-useful")))
}
fn f_zero(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(0))
}
/// window-scroll-bar-*: decode_live_window → `window-live-p', 0 in batch.
fn f_zero_winlive(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::win_live(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}
fn f_minibuffer_depth(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU: make_fixnum (minibuf_level).
    Ok(Value::Int(i.minibuf_level.max(0) as i128))
}

fn f_frame_configuration_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fc = i.q("frame-configuration");
    Ok(Value::from_bool(match &a[0] {
        Value::Cons(c) => i.sym_is(&c.borrow().car, fc),
        _ => false,
    }))
}

/// `current-frame-configuration' — GNU returns a list headed
/// `frame-configuration' of (FRAME PARAMS WINDOW-CONFIG) triples.
fn f_current_frame_configuration(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => {
            return Ok(Value::list(vec![Value::Sym(
                i.q("frame-configuration"),
            )]));
        }
    };
    let params = crate::editor::f_frame_parameters(i, vec![fr.clone()].into())?;
    let wc = crate::editor::f_current_window_configuration(i, vec![fr.clone()].into())?;
    let fc = i.q("frame-configuration");
    Ok(Value::list(vec![
        Value::Sym(fc),
        Value::list(vec![fr, params, wc]),
    ]))
}

fn f_mouse_position(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => Value::Nil,
    };
    // Emacs: (FRAME nil) on a tty with no mouse.
    Ok(Value::list(vec![fr, Value::Nil]))
}

fn f_display_pixel_width(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // TTY: "pixels" are char cells (Emacs reports frame width).
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    let w = sel_frame(i).map(|f| f.borrow().width).unwrap_or(80);
    Ok(Value::Int(w as i128))
}

fn f_display_pixel_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    let h = sel_frame(i).map(|f| f.borrow().height).unwrap_or(24);
    Ok(Value::Int(h as i128))
}

fn f_display_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Nil)
}

fn f_display_pred_false(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::Nil)
}

/// GNU `decode_display' (xfns.c): nil → selected frame, a live frame
/// → it, a string must name an open display (only ":0"/""-style TTY
/// names resolve) — anything else signals
/// `(error "Display %s does not exist")'.
pub(crate) fn decode_display(i: &mut Interp, v: &Value) -> Result<crate::editor::FrameRef, Flow> {
    match v {
        // GNU get-device-terminal: nil → selected frame's terminal;
        // live frame → its terminal.
        Value::Nil | Value::Frame(_) => crate::editor::frame_live(i, v),
        Value::Str(s) => Err(i.error(format!("Display {} does not exist", s.borrow()))),
        // Any other object: "Invalid argument X in ‘get-device-terminal’"
        // (GNU keeps the curly-quote style even in batch).
        other => Err(i.error(format!(
            "Invalid argument {} in ‘get-device-terminal’",
            i.princ_to_string(other)
        ))),
    }
}

fn f_display_mm(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

fn f_display_visual_class(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Sym(i.q("static-gray")))
}

fn f_display_planes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Int(3))
}

fn f_display_color_cells(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Int(0))
}

/// `display-popup-menus-p' — GNU Fdisplay_popup_menus_p: decode_frame
/// first (arg must be a frame when non-nil), then the tty result nil.
fn f_display_popup_menus_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = decode_display(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}

/// `(tty-color-desc COLOR FRAME)' — the prelude Lisp entry; returns
/// (NAME INDEX R G B) or nil.  `frame' is the already-validated frame
/// arg (nil = selected), forwarded verbatim like GNU's C callers do.
pub(crate) fn tty_color_desc(i: &mut Interp, color: &Value, frame: &Value) -> Result<Value, Flow> {
    let f = Value::Sym(i.q("tty-color-desc"));
    i.apply(&f, vec![color.clone(), frame.clone()])
}

/// The three ints in (cddr (tty-color-desc …)), or None.
fn desc_rgb3(desc: &Value) -> Option<(i128, i128, i128)> {
    let tail = crate::lisp::builtins::listfn::nthcdr_of(desc, 2);
    let v = tail.list_to_vec().ok()?;
    match v.as_slice() {
        [Value::Int(r), Value::Int(g), Value::Int(b), ..] => Some((*r, *g, *b)),
        _ => None,
    }
}

/// GNU xfaces.c `face_color_supported_p' on a tty frame:
/// `tty_defined_color' — "" succeeds vacuously, the unspecified-fg/-bg
/// names map to the terminal defaults, everything else needs a
/// fixnum INDEX from `tty-color-desc'.
fn tty_color_supported(i: &mut Interp, color: &str, frame: &Value) -> Result<bool, Flow> {
    // GNU tty_defined_color checks `*color_name' — the C string's
    // first byte, so a Lisp "\0xxx" reads as empty: the default
    // black pixel is installed and status comes back true.
    if color.is_empty()
        || color.starts_with('\0')
        || color == "unspecified-fg"
        || color == "unspecified-bg"
    {
        return Ok(true);
    }
    let desc = tty_color_desc(i, &Value::string(color), frame)?;
    let idx = match crate::lisp::builtins::listfn::nthcdr_of(&desc, 1) {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    Ok(matches!(idx, Value::Int(_)))
}

/// `color-supported-p' — GNU xfaces.c: CHECK_STRING (COLOR), then
/// decode_any_frame (FRAME), then the tty lookup.
fn f_color_defined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let frame_v = arg(&a, 1);
    let _ = crate::editor::frame_any(i, &frame_v)?;
    Ok(Value::from_bool(tty_color_supported(i, &s, &frame_v)?))
}

/// `color-gray-p' — GNU xfaces.c `face_color_gray_p': CHECK_STRING,
/// decode_any_frame, then the defined-color's RGB distance from the
/// gray diagonal (any near-black color counts too).
fn f_color_gray_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let frame_v = arg(&a, 1);
    let _ = crate::editor::frame_any(i, &frame_v)?;
    // tty_defined_color on "" leaves {0,0,0} → gray; otherwise the
    // description's RGB comes from tty-color-desc (nil → not gray).
    let rgb = if s.is_empty() || s.starts_with('\0') {
        // GNU tty_defined_color reads `*color_name' — a leading NUL
        // truncates the C view, so "\0xxx" lands on the {0,0,0}
        // default like "" does.
        Some((0, 0, 0))
    } else {
        let desc = tty_color_desc(i, &a[0], &frame_v)?;
        desc_rgb3(&desc)
    };
    let ok = match rgb {
        Some((r, g, b)) => {
            (r < 5000 && g < 5000 && b < 5000)
                || ((r - g).abs() < std::cmp::max(r, g) / 20
                    && (g - b).abs() < std::cmp::max(g, b) / 20
                    && (b - r).abs() < std::cmp::max(b, r) / 20)
        }
        None => false,
    };
    Ok(Value::from_bool(ok))
}

// ---------- windows ----------

/// Window dims helper: nil/omitted → selected window; a non-window
/// value is a `wrong-type-argument' under GNU's CHECK_WINDOW
/// convention — callers needing GNU's "not a valid window" plain
/// error use `win_any' instead.
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

/// Character-unit minimum used by resizing and sizable queries.
/// Keep these callers consistent with state serialization and window-min-size.
pub(crate) fn win_min_size(i: &mut Interp, wv: &Value, horiz: bool, ignore: &Value) -> i128 {
    let window = match wv {
        Value::Window(w) => Some(w.clone()),
        _ => crate::editor::sel_window(i),
    };
    window.map_or(0, |w| {
        crate::editor::window_minimum(i, &w, horiz, ignore, false)
    })
}

/// `window-sizable' — GNU: normalize WINDOW (plain "N is not a valid
/// window" error), DELTA must satisfy number-or-marker-p.  Negative
/// DELTA clamps at the window's minimum size; positive DELTA returns
/// DELTA unless the window is size-fixed.
fn f_window_sizable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let wv = match &a[0] {
        Value::Window(_) => a[0].clone(),
        Value::Nil => f_selected_window(i, vec![].into())?,
        other => {
            let shown = i.princ_to_string(other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let delta = match &a[1] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let horiz = !arg(&a, 2).is_nil();
    let ignore = arg(&a, 3);
    if delta < 0 {
        let min = win_min_size(i, &wv, horiz, &ignore);
        let (w, h) = win_dims(i, &a[0]);
        let size = if horiz { w } else { h } as i128;
        if size <= min {
            Ok(Value::Int(0))
        } else {
            Ok(Value::Int((min - size).max(delta)))
        }
    } else if delta > 0 {
        // `window-size-fixed-p' is nil on our model.
        Ok(Value::Int(delta))
    } else {
        Ok(Value::Int(0))
    }
}

fn f_window_max_chars(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `window-max-chars-per-line'
    // is Lisp (window.el): `window-normalize-window window t' raises the
    // plain "N is not a live window" error on non-live args; a live tty
    // window reserves the rightmost column for the continuation glyph
    // (80-col → 79).
    let w = crate::editor::win_err_live(i, &arg(&a, 0))?;
    let width = w.borrow().width;
    Ok(Value::Int(width.saturating_sub(1) as i128))
}

/// `window-left-column' — GNU decodes a VALID window (returns `left').
fn f_window_left_column(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = crate::editor::win_valid(i, &arg(&a, 0))?;
    Ok(Value::Int(w.borrow().left as i128))
}

fn f_pos_visible(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fpos_visible_in_window_group_p: POS is
    // CHECK_NUMBER_COERCE_MARKER, WINDOW is CHECK_LIVE_WINDOW.
    // POS omitted/nil → point in WINDOW (the Lisp wrapper defaults it).
    let w = match arg(&a, 1) {
        Value::Nil => sel_window(i),
        Value::Window(w) => Some(w),
        other => return Err(i.wrong_type_mut("window-live-p", &other)),
    };
    // POS omitted/nil → point in WINDOW.
    let pos = match arg(&a, 0) {
        Value::Nil => w.as_ref().map(|w| w.borrow().point).unwrap_or(1),
        Value::Int(n) => n as usize,
        Value::Marker(m) => m.borrow().position,
        ref other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    let ok = match w {
        Some(w) => {
            // GNU computes visibility from the window's glyph
            // matrix — in batch there is no display, so positions
            // are never visible.
            if i.noninteractive {
                false
            } else {
                let w = w.borrow();
                pos >= w.start
            }
        }
        None => true,
    };
    Ok(Value::from_bool(ok))
}

/// `window-line' — display-line index of point in WINDOW (0-based
/// line count from window-start to point).
fn f_window_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Window(w) => w,
        _ => match sel_window(i) {
            Some(w) => w,
            None => return Ok(Value::Nil),
        },
    };
    let (start, bid) = {
        let b = w.borrow();
        (b.start, b.buffer)
    };
    let buf = match i.buffers.get(bid) {
        Some(b) => b,
        None => return Ok(Value::Nil),
    };
    let bb = buf.borrow();
    let point = bb.point();
    let s0 = start.min(bb.text.len());
    let e0 = point.min(bb.text.len());
    let (lo, hi) = if s0 <= e0 { (s0, e0) } else { (e0, s0) };
    let mut n = 0i128;
    for k in lo..hi {
        if bb.text.char_at(k) == '\n' {
            n += 1;
        }
    }
    Ok(Value::Int(if s0 <= e0 { n } else { -n }))
}

fn f_selected_window(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    match sel_window(i) {
        Some(w) => Ok(Value::Window(w)),
        None => Ok(Value::Nil),
    }
}

fn f_window_in_direction(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el
    // `window-in-direction': `(window-normalize-window window t)' runs
    // FIRST — the plain "N is not a live window" check precedes the
    // `Wrong direction' error.
    enum Dir {
        Above,
        Below,
        Left,
        Right,
    }
    let window = match arg(&a, 1) {
        Value::Nil => crate::editor::sel_window(i).ok_or_else(|| i.error("no selected window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => {
            return Err(i.error(format!(
                "{} is not a live window",
                i.princ_to_string(&other)
            )));
        }
    };
    let dir = match i.sym_id(&a[0]).map(|s| i.symbol_name(s)).as_deref() {
        Some("up") | Some("above") => Dir::Above,
        Some("down") | Some("below") => Dir::Below,
        Some("left") => Dir::Left,
        Some("right") => Dir::Right,
        _ => return Err(i.error(format!("Wrong direction {}", i.princ_to_string(&a[0])))),
    };
    let ignore = arg(&a, 2).truthy();
    let sign = arg(&a, 3).int().unwrap_or(0);
    let wrap = arg(&a, 4).truthy();
    let minibuf = arg(&a, 5);

    let wid = window.borrow().id;
    let frame = i
        .frames
        .iter()
        .find(|f| {
            f.borrow().windows.iter().any(|w| w.borrow().id == wid)
                || f.borrow()
                    .minibuffer
                    .as_ref()
                    .map(|m| m.borrow().id == wid)
                    .unwrap_or(false)
        })
        .cloned();
    let Some(frame) = frame else {
        return Ok(Value::Nil);
    };

    let (wtop, wleft, wwid, whgt, wpt) = {
        let w = window.borrow();
        (
            w.top as i128,
            w.left as i128,
            w.width as i128,
            w.height as i128,
            w.point,
        )
    };
    let hor = matches!(dir, Dir::Left | Dir::Right);
    let first = if hor { wleft } else { wtop };
    let last = first + if hor { wwid } else { whgt };
    let posn = if sign < 0 {
        if hor {
            wtop + whgt - 1
        } else {
            wleft + wwid - 1
        }
    } else if sign > 0 {
        if hor { wtop } else { wleft }
    } else {
        // GNU takes (nth 2 (posn-at-point ...)) = (COL . ROW); nil in
        // batch → the (or .. 1) fallbacks.
        let pap_sym = i.q("posn-at-point");
        let pap = i.apply(
            &Value::Sym(pap_sym),
            vec![Value::Int(wpt as i128 + 1), Value::Window(window.clone())],
        )?;
        let posn_cons = pap
            .list_to_vec()
            .ok()
            .and_then(|v| v.get(2).cloned())
            .unwrap_or(Value::Nil);
        let (pcar, pcdr) = match &posn_cons {
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.int(), b.cdr.int())
            }
            _ => (None, None),
        };
        if hor {
            pcdr.unwrap_or(1) + wtop
        } else {
            pcar.unwrap_or(1) + wleft
        }
    };

    let (fwidth, frame_bot, root_top, mini_top) = {
        let f = frame.borrow();
        let (mini_t, mini_b) = f
            .minibuffer
            .as_ref()
            .map(|m| {
                let b = m.borrow();
                (b.top as i128, (b.top + b.height) as i128)
            })
            .unwrap_or((f.height as i128, f.height as i128));
        // `frame-pixel-height' = frame bottom including the echo
        // area; `root_top' = topmost content edge (0 unsplit, 1
        // once the menu-bar row materialized).
        (
            f.width as i128,
            mini_b,
            f.windows
                .iter()
                .map(|w| w.borrow().top as i128)
                .min()
                .unwrap_or(0),
            mini_t,
        )
    };
    let mut best_edge = match dir {
        Dir::Below => frame_bot,
        Dir::Right => fwidth,
        _ => -1,
    };
    let mut best_edge_2 = best_edge;
    let mut best_diff_2 = if hor { frame_bot } else { fwidth };
    let mut best: Option<WindowRef> = None;
    let mut best_2: Option<WindowRef> = None;

    // walk-window-tree's minibuf arg: t → always include FRAME's
    // minibuffer window; nil → only when active; other → never.
    let mut cands: Vec<WindowRef> = frame.borrow().windows.clone();
    if let Some(mini) = frame.borrow().minibuffer.clone() {
        let include = match &minibuf {
            Value::Sym(s) if i.symbol_name(*s) == "t" => true,
            Value::Nil => i.minibuf_level > 0,
            _ => false,
        };
        if include && !mini.borrow().dead {
            cands.push(mini);
        }
    }
    let iwp = i
        .symbol_value(i.q_soft("ignore-window-parameters").unwrap_or(0))
        .truthy();
    let no_ow = i.q("no-other-window");
    let active_mini = i.minibuf_level > 0;

    // window-at-side-p against the root extent (0,root_top,fwidth,
    // mini_top).
    let at_side = |w: &WindowRef, s: usize| -> bool {
        let b = w.borrow();
        let (l, t, r, bt) = (
            b.left as i128,
            b.top as i128,
            (b.left + b.width) as i128,
            (b.top + b.height) as i128,
        );
        match s {
            0 => l == 0,
            1 => t == root_top,
            2 => r == fwidth,
            _ => bt == mini_top,
        }
    };

    for w in cands {
        if w.borrow().id == wid || w.borrow().dead {
            continue;
        }
        if !ignore && !iwp {
            let no_other = crate::lisp::eval::plist_get(&w.borrow().params, no_ow);
            if no_other.truthy() {
                continue;
            }
        }
        let (wt, wl, ww, wh) = {
            let b = w.borrow();
            (
                b.top as i128,
                b.left as i128,
                b.width as i128,
                b.height as i128,
            )
        };
        if hor {
            if wt <= posn && posn < wt + wh {
                // W covers the reference row.
                let ok = match dir {
                    Dir::Left => {
                        (wl <= first && wl > best_edge)
                            || (wrap && at_side(&window, 0) && at_side(&w, 2))
                    }
                    Dir::Right => {
                        (wl >= last && wl < best_edge)
                            || (wrap && at_side(&window, 2) && at_side(&w, 0))
                    }
                    _ => false,
                };
                if ok {
                    best_edge = wl;
                    best = Some(w.clone());
                }
            } else if (matches!(dir, Dir::Left) && wl + ww <= first)
                || (matches!(dir, Dir::Right) && last <= wl)
            {
                // W is on the right side axis but doesn't cover posn.
                let diff = if wt > posn { wt - posn } else { posn - wt - wh };
                if diff < best_diff_2
                    || (diff == best_diff_2
                        && match dir {
                            Dir::Left => wl > best_edge_2,
                            _ => wl < best_edge_2,
                        })
                {
                    best_edge_2 = wl;
                    best_diff_2 = diff;
                    best_2 = Some(w.clone());
                }
            }
        } else {
            if wl <= posn && posn < wl + ww {
                // W covers the reference column.
                let ok = match dir {
                    Dir::Above => {
                        (wt <= first && wt > best_edge)
                            || (wrap
                                && at_side(&window, 1)
                                && if active_mini {
                                    w.borrow().minibuffer && i.minibuf_level > 0
                                } else {
                                    at_side(&w, 3)
                                })
                    }
                    Dir::Below => {
                        (wt >= first && wt < best_edge)
                            || (wrap
                                && if active_mini {
                                    window.borrow().minibuffer && i.minibuf_level > 0
                                } else {
                                    at_side(&window, 3)
                                }
                                && at_side(&w, 1))
                    }
                    _ => false,
                };
                if ok {
                    best_edge = wt;
                    best = Some(w.clone());
                }
            } else if (matches!(dir, Dir::Above) && wt + wh <= first)
                || (matches!(dir, Dir::Below) && last <= wt)
            {
                let diff = if wl > posn { wl - posn } else { posn - wl - ww };
                if diff < best_diff_2
                    || (diff == best_diff_2
                        && match dir {
                            Dir::Above => wt > best_edge_2,
                            _ => wt < best_edge_2,
                        })
                {
                    best_edge_2 = wt;
                    best_diff_2 = diff;
                    best_2 = Some(w.clone());
                }
            }
        }
    }
    Ok(best.or(best_2).map(Value::Window).unwrap_or(Value::Nil))
}

fn f_window_normalize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el defun: nil → selected-window; LIVE-ONLY →
    // `window-live-p' gate; otherwise `window-valid-p' (a *dead* window
    // object is invalid, internal windows are fine).  The prelude defun
    // matches GNU textually; this subr shadows it for GNU's nativecomp
    // `subrp' surface, so it must carry the same 1+1 arity.
    match &a[0] {
        Value::Nil => f_selected_window(i, vec![].into()),
        Value::Window(w) => {
            let w = w.borrow();
            let live_only = arg(&a, 1).truthy();
            let ok = if live_only {
                !w.dead && w.horiz.is_none()
            } else {
                !w.dead
            };
            if ok {
                drop(w);
                Ok(a[0].clone())
            } else {
                Err(i.error(if live_only {
                    format!("{} is not a live window", i.princ_to_string(&a[0]))
                } else {
                    format!("{} is not a valid window", i.princ_to_string(&a[0]))
                }))
            }
        }
        other => {
            let shown = i.princ_to_string(other);
            Err(i.error(if arg(&a, 1).truthy() {
                format!("{shown} is not a live window")
            } else {
                format!("{shown} is not a valid window")
            }))
        }
    }
}

fn f_window_norm_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el `window-normalize-buffer': nil →
    // current buffer; a live buffer object returns it; a string must name
    // a live buffer — anything else is `(error "No such live buffer %s")';
    // non-string atoms pass through for the caller's bufferp check.
    match &a[0] {
        Value::Nil => Ok(i
            .current_buffer_ref()
            .map(Value::Buffer)
            .unwrap_or(Value::Nil)),
        Value::Buffer(b) => {
            if i.buffer_live(b.borrow().id) {
                Ok(a[0].clone())
            } else {
                Err(i.error(format!("No such live buffer {}", i.princ_to_string(&a[0]))))
            }
        }
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.buffers.by_name(&name) {
                Some(id) if i.buffer_live(id) => match i.buffers.get(id) {
                    Some(b) => Ok(Value::Buffer(b)),
                    None => Err(i.error(format!("No such live buffer {name}"))),
                },
                _ => Err(i.error(format!("No such live buffer {name}"))),
            }
        }
        // Non-buffer non-string atoms: GNU falls to "No such buffer %s".
        other => Err(i.error(format!("No such buffer {}", i.princ_to_string(other)))),
    }
}

fn f_window_norm_frame(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el defun: nil -> selected frame; non-frame and
    // dead frames signal `(error "X is not a live frame")'.
    match &a[0] {
        Value::Frame(f) if !f.borrow().dead => Ok(Value::Frame(f.clone())),
        Value::Nil => match sel_frame(i) {
            Some(f) => Ok(Value::Frame(f)),
            None => Ok(Value::Nil),
        },
        other => {
            let shown = i.princ_to_string(other);
            Err(i.error(format!("{shown} is not a live frame")))
        }
    }
}

fn f_get_window_pred(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pred = a[0].clone();
    if let Some(f) = sel_frame(i) {
        let wins: Vec<WindowRef> = f.borrow().windows.clone();
        for w in wins {
            let wv = Value::Window(w.clone());
            // GNU funcall semantics: a special-form name signals
            // `invalid-function' (call_function would *call* it).
            let r = i.apply(&pred, vec![wv.clone()])?;
            if !r.is_nil() {
                return Ok(wv);
            }
        }
    }
    Ok(Value::Nil)
}

// ---------- keymaps ----------

fn f_make_composed_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el:
    // `(keymap ,@(if (keymapp maps) (list maps) maps) ,@parent)' — MAPS
    // is spliced ELEMENT BY ELEMENT (strings and vectors yield their
    // elements, e.g. "a" → 97) unless it resolves to a keymap itself;
    // PARENT is appended verbatim as the (possibly dotted) tail.
    fn splice_seq(i: &mut Interp, v: &Value, into: &mut Vec<Value>) -> EvalResult {
        match v {
            Value::Str(s) => {
                // GNU string_to_char_array: a unibyte string yields its
                // raw bytes as chars 0-255; a multibyte string yields
                // real codepoints (eight-bit chars = 0x3FFF80+b).  The
                // PUA proxy chars map back via `eight_bit_byte' (byte
                // value) or `lisp_char_code' (full eight-bit code).
                if i.is_unibyte_str(s) {
                    for ch in s.borrow().chars() {
                        into.push(Value::Int(
                            crate::lisp::value::eight_bit_byte(ch)
                                .map(|b| b as i128)
                                .unwrap_or_else(|| ch as i128),
                        ));
                    }
                } else {
                    for ch in s.borrow().chars() {
                        into.push(Value::Int(crate::lisp::value::lisp_char_code(ch)));
                    }
                }
            }
            Value::Vec(vec) => {
                into.extend(vec.borrow().iter().cloned());
            }
            Value::Nil => {}
            // GNU `mapcar' over MAPS: non-list atoms (symbols, ints,
            // floats, buffers…) signal `sequencep'.
            Value::Int(_) | Value::Float(_) | Value::Sym(_) => {
                return Err(i.wrong_type_mut("sequencep", v));
            }
            other => match other.list_to_vec() {
                Ok(items) => into.extend(items),
                Err(crate::lisp::value::ListError::Circular(_)) => {
                    return Err(i.error("List contains a loop"));
                }
                Err(crate::lisp::value::ListError::Dotted(tail)) => {
                    return Err(i.wrong_type_mut("listp", &tail));
                }
            },
        }
        Ok(Value::Nil)
    }
    let m0 = crate::editor::keymap_def(i, a[0].clone())?;
    let mut elems = Vec::new();
    if is_keymap(i, &m0) {
        elems.push(m0);
    } else {
        splice_seq(i, &m0, &mut elems)?;
    }
    let mut tail = arg(&a, 1).clone();
    for m in elems.iter().rev() {
        tail = Value::cons(m.clone(), tail);
    }
    Ok(Value::cons(Value::Sym(i.q("keymap")), tail))
}

fn f_current_active_maps(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcurrent_active_maps: (otlp) (keymap-prop) minors local global;
    // with OLP set and no otlp, overriding-local-map REPLACES all of
    // the buffer-dependent maps.  POSITION goes through keymap.c's
    // `click_position': an integer/marker must sit inside BEGV..ZV of the
    // current buffer — out of range signals
    // (args-out-of-range BUFFER POSITION); other objects use PT.
    let olp = a.first().map(|v| v.truthy()).unwrap_or(false);
    if let Some(pos) = a.get(1) {
        let pt: Option<i128> = match pos {
            Value::Int(n) => Some(*n),
            Value::Marker(m) => {
                let m = m.borrow();
                if m.buffer.is_none() {
                    return Err(i.error("Marker does not point anywhere"));
                }
                Some(m.position as i128 + 1)
            }
            _ => None,
        };
        if let Some(p) = pt {
            let cur = crate::editor::cur(i);
            let bb = cur.borrow();
            if p < bb.begv as i128 + 1 || p > bb.zv as i128 + 1 {
                drop(bb);
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![Value::Buffer(cur), pos.clone()],
                ));
            }
        }
    }
    let otlp = i.symbol_value(i.q_soft("overriding-terminal-local-map").unwrap_or(0));
    let olmap = i.symbol_value(i.q_soft("overriding-local-map").unwrap_or(0));
    let gmap = i.symbol_value(i.q_soft("global-map").unwrap_or(0));

    let mut maps = Vec::new();
    if is_keymap(i, &gmap) {
        maps.push(gmap);
    }
    if olp && !is_keymap(i, &otlp) && is_keymap(i, &olmap) {
        maps.insert(0, olmap);
        return Ok(Value::list(maps));
    }

    let local = {
        let b = crate::editor::cur(i);
        let lb = b.borrow();
        lb.locals
            .get(&i.q_soft("local-keymap").unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    if is_keymap(i, &local) {
        maps.insert(0, local);
    }
    // Minor-mode maps precede the local map.
    let minors = crate::editor::current_minor_maps(i)?
        .into_iter()
        .filter(|(_, m)| is_keymap(i, m))
        .map(|(_, m)| m)
        .collect::<Vec<_>>();
    for m in minors.into_iter().rev() {
        maps.insert(0, m);
    }
    if olp && is_keymap(i, &otlp) {
        maps.insert(0, otlp);
    }
    Ok(Value::list(maps))
}

// ---------- char tables (GNU chartab.c trie: 64/16/32/128)

/// Number of characters covered by one slot of each trie level:
/// top-level slots cover 65536, `#^^[1' slots 4096, `#^^[2' slots
/// 128, `#^^[3' leaves one char per slot.
const CHARTAB_BITS: [u32; 4] = [16, 12, 7, 0];
/// Slot count of each trie level (the top level is indexed
/// separately — a depth-0 "table" is the char-table's own contents).
const CHARTAB_SIZE: [usize; 4] = [64, 16, 32, 128];
/// `MAX_CHAR'.
pub(crate) const CT_MAX_CHAR: u32 = 0x3f_ffff;

/// The contents vector of a char-table: 65 slots where [0] is the
/// ASCII cache slot (a scalar, or the `#^^[3' leaf covering chars
/// 0-127) and [1..=64] are the top-level slots covering 65536
/// characters each.  Sub-tables are Records of the form
/// [sub-char-table DEPTH MIN-CHAR S0 .. SN].
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

/// Build a char-table: Record [char-table SUBTYPE CONTENTS65 EXTRAS..]
/// where CONTENTS65 is the 65-slot trie root ([0] ASCII cache,
/// [1..=64] top blocks), every slot = INIT.  GNU's `make_vector'
/// also initializes the defalt slot to INIT.
pub(crate) fn make_ct(i: &mut Interp, subtype: Value, init: Value, extras: Vec<Value>) -> Value {
    let vec = Value::vector(vec![init.clone(); 65]);
    let mut rec = vec![Value::Sym(i.q("char-table")), subtype, vec];
    rec.extend(extras);
    let t = Value::record(rec);
    if !init.is_nil() {
        i.set_char_table_defalt(&t, init);
    }
    t
}

/// A sub-char-table (`#^^[DEPTH MIN-CHAR S0 .. SN]') is stored as a
/// Record headed by the `sub-char-table' symbol.  Users can fake the
/// tag, but GNU is similarly permissive about stored values.
fn is_sub_ct(i: &Interp, v: &Value) -> bool {
    match i.q_soft("sub-char-table") {
        Some(tag) => is_sub_ct_tag(tag, v),
        None => false,
    }
}

/// `is_sub_ct' with a precomputed tag id (no `&Interp' needed).
pub(crate) fn is_sub_ct_tag(tag: SymId, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let b = r.borrow();
            matches!(b.first(), Some(Value::Sym(s)) if *s == tag)
                && matches!(b.get(1), Some(Value::Int(d)) if (1..=3).contains(d))
                && matches!(b.get(2), Some(Value::Int(_)))
        }
        _ => false,
    }
}

/// (depth, min_char, record) of a sub-char-table value.
fn sub_ct_parts(v: &Value) -> Option<(usize, u32, Rc<RefCell<Vec<Value>>>)> {
    let Value::Record(r) = v else {
        return None;
    };
    let b = r.borrow();
    let d = match b.get(1) {
        Some(Value::Int(n)) => *n as usize,
        _ => return None,
    };
    let m = match b.get(2) {
        Some(Value::Int(n)) => *n as u32,
        _ => return None,
    };
    Some((d, m, r.clone()))
}

fn make_sub_ct(i: &mut Interp, depth: usize, min_char: u32, fill: Value) -> Value {
    let mut v = Vec::with_capacity(3 + CHARTAB_SIZE[depth]);
    v.push(Value::Sym(i.q("sub-char-table")));
    v.push(Value::Int(depth as i128));
    v.push(Value::Int(min_char as i128));
    v.extend(std::iter::repeat_n(fill, CHARTAB_SIZE[depth]));
    Value::record(v)
}

/// Raw value stored for C in the 65-slot contents vec CB — no
/// defalt or parent inheritance (GNU's raw `contents' walk).
fn ct_raw(i: &Interp, cb: &[Value], c: u32, uniprop: bool) -> Value {
    ct_raw_tag(i.q_soft("sub-char-table"), cb, c, uniprop)
}

/// GNU `UNIPROP_TABLE_P': purpose `char-code-property-table' with
/// exactly five extra slots.
pub(crate) fn is_uniprop_table(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Record(r) => {
            let rr = r.borrow();
            rr.len() == 8
                && matches!(rr.first(), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-table")
                && matches!(rr.get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "char-code-property-table")
                && matches!(rr.get(2), Some(Value::Vec(_)))
        }
        _ => false,
    }
}

/// GNU `UNIPROP_COMPRESSED_FORM_P': a string starting with character
/// 1 or 2 encodes 128 per-character values in a depth-2 leaf slot.
fn uniprop_compressed(v: &Value) -> bool {
    match v {
        Value::Str(s) => matches!(s.borrow().chars().next(), Some('\u{1}') | Some('\u{2}')),
        _ => false,
    }
}

/// GNU `uniprop_table_uncompress': expand a compressed leaf string in
/// slot SLOT of VEC into a real depth-3 sub-table covering MIN3..MIN3+127,
/// storing it back into VEC.  Format 1 is a plain sequence (0 = nil);
/// format 2 is run-length (a char >= 128 repeats the previous value).
fn uniprop_uncompress(tag: SymId, vec: &Rc<RefCell<Vec<Value>>>, slot: usize, min3: u32) -> Value {
    let val = vec.borrow().get(slot).cloned().unwrap_or(Value::Nil);
    let mut rec = Vec::with_capacity(3 + 128);
    rec.push(Value::Sym(tag));
    rec.push(Value::Int(3));
    rec.push(Value::Int(min3 as i128));
    rec.extend(std::iter::repeat_n(Value::Nil, 128));
    if let Value::Str(s) = &val {
        let chars: Vec<char> = s.borrow().chars().collect();
        match chars.first() {
            Some('\u{1}') => {
                let mut k = chars.get(1).map(|c| *c as usize).unwrap_or(0);
                for c in chars.iter().skip(2) {
                    if k >= 128 {
                        break;
                    }
                    rec[3 + k] = if *c == '\0' {
                        Value::Nil
                    } else {
                        Value::Int(*c as i128)
                    };
                    k += 1;
                }
            }
            Some('\u{2}') => {
                let (mut k, mut j) = (0usize, 1usize);
                while j < chars.len() && k < 128 {
                    let v = chars[j] as u32;
                    j += 1;
                    let mut count = 1usize;
                    if j < chars.len() && (chars[j] as u32) >= 128 {
                        count = chars[j] as usize - 128;
                        j += 1;
                    }
                    for _ in 0..count {
                        if k >= 128 {
                            break;
                        }
                        rec[3 + k] = Value::Int(v as i128);
                        k += 1;
                    }
                }
            }
            _ => {}
        }
    }
    let sub = Value::record(rec);
    if let Some(s) = vec.borrow_mut().get_mut(slot) {
        *s = sub.clone();
    }
    sub
}

/// `ct_raw' with the `sub-char-table' tag id (None = no sub-tables
/// can exist, so every slot is scalar).  Usable from `Syn' snapshots
/// and other `&Interp'-free contexts.  UNIPROP mirrors GNU's
/// `sub_char_table_ref' uniprop flag: compressed leaf strings are
/// expanded in place.
pub(crate) fn ct_raw_tag(tag: Option<SymId>, cb: &[Value], c: u32, uniprop: bool) -> Value {
    let is_sub = |v: &Value| tag.is_some_and(|t| is_sub_ct_tag(t, v));
    if c > CT_MAX_CHAR {
        return Value::Nil;
    }
    if c < 128 {
        return match cb.first() {
            Some(s) if is_sub(s) => sub_ct_parts(s)
                .and_then(|(_, _, r)| r.borrow().get(3 + c as usize).cloned())
                .unwrap_or(Value::Nil),
            Some(v) => v.clone(),
            None => Value::Nil,
        };
    }
    let mut cur = cb
        .get(1 + (c >> 16) as usize)
        .cloned()
        .unwrap_or(Value::Nil);
    loop {
        match cur {
            v if is_sub(&v) => {
                let Some((d, m, r)) = sub_ct_parts(&v) else {
                    return Value::Nil;
                };
                let idx = (c.saturating_sub(m) >> CHARTAB_BITS[d]) as usize;
                let next = match r.borrow().get(3 + idx) {
                    Some(x) => x.clone(),
                    None => return Value::Nil,
                };
                cur = if uniprop && uniprop_compressed(&next) {
                    match tag {
                        Some(t) => uniprop_uncompress(t, &r, 3 + idx, m + 128 * idx as u32),
                        None => next,
                    }
                } else {
                    next
                };
            }
            v => return v,
        }
    }
}

/// `char_table_ascii': the value of chars 0-127, derived by walking
/// contents[0]'s 0-index chain (scalar, or the `#^^[3' leaf).
fn ct_ascii(i: &Interp, cb: &[Value]) -> Value {
    let s1 = cb.get(1).cloned().unwrap_or(Value::Nil);
    if !is_sub_ct(i, &s1) {
        return s1;
    }
    let s2 = sub_ct_parts(&s1)
        .and_then(|(_, _, r)| r.borrow().get(3).cloned())
        .unwrap_or(Value::Nil);
    if !is_sub_ct(i, &s2) {
        return s2;
    }
    sub_ct_parts(&s2)
        .and_then(|(_, _, r)| r.borrow().get(3).cloned())
        .unwrap_or(Value::Nil)
}

/// Store `ct_ascii' into the contents vector's ASCII cache slot.
fn ct_refresh_ascii(i: &Interp, contents: &Rc<RefCell<Vec<Value>>>) {
    let v = {
        let cb = contents.borrow();
        ct_ascii(i, &cb)
    };
    contents.borrow_mut()[0] = v;
}

/// GNU `sub_char_table_set': write VAL for C inside sub-table SUB,
/// lazily materializing deeper levels.  UNIPROP mirrors GNU: a
/// compressed leaf string expands before the write descends.
fn sub_ct_set(i: &mut Interp, sub: &Value, c: u32, val: Value, uniprop: bool) {
    let Some((depth, min, r)) = sub_ct_parts(sub) else {
        return;
    };
    let idx = (c.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    if depth == 3 {
        r.borrow_mut()[3 + idx] = val;
        return;
    }
    let child = r.borrow().get(3 + idx).cloned().unwrap_or(Value::Nil);
    let child = if is_sub_ct(i, &child) {
        child
    } else if uniprop && uniprop_compressed(&child) {
        match i.q_soft("sub-char-table") {
            Some(t) => uniprop_uncompress(t, &r, 3 + idx, min + (idx as u32) * 128),
            None => child,
        }
    } else {
        let n = make_sub_ct(
            i,
            depth + 1,
            min + (idx as u32) * (1 << CHARTAB_BITS[depth]),
            child,
        );
        r.borrow_mut()[3 + idx] = n.clone();
        n
    };
    sub_ct_set(i, &child, c, val, uniprop);
}

/// GNU `char_table_set': write VAL for C, splitting the path and
/// refreshing the ASCII cache as needed.
pub(crate) fn ct_set(i: &mut Interp, table: &Value, c: u32, val: Value) {
    let Some(contents) = char_table_vec(table) else {
        return;
    };
    if c > CT_MAX_CHAR {
        return;
    }
    if c < 128 {
        let hit = {
            let cb = contents.borrow();
            cb.first().map(|v| is_sub_ct(i, v)).unwrap_or(false)
        };
        if hit {
            if let Some(a) = contents.borrow().first().cloned() {
                if let Some((_, _, r)) = sub_ct_parts(&a) {
                    r.borrow_mut()[3 + c as usize] = val;
                    return;
                }
            }
        }
    }
    let ti = 1 + (c >> 16) as usize;
    let sub = contents.borrow().get(ti).cloned().unwrap_or(Value::Nil);
    let sub = if is_sub_ct(i, &sub) {
        sub
    } else {
        let n = make_sub_ct(i, 1, (c >> 16) << 16, sub);
        contents.borrow_mut()[ti] = n.clone();
        n
    };
    sub_ct_set(i, &sub, c, val, is_uniprop_table(i, table));
    if c < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

/// GNU `sub_char_table_set_range': block-optimized range write —
/// fully covered slots are overwritten with the scalar (collapsing
/// any sub-table), partial slots are materialized and descended.
/// UNIPROP mirrors GNU: compressed leaf strings expand first.
fn sub_ct_set_range(i: &mut Interp, sub: &Value, from: u32, to: u32, val: &Value, uniprop: bool) {
    let Some((depth, min, r)) = sub_ct_parts(sub) else {
        return;
    };
    let chars = 1u32 << CHARTAB_BITS[depth];
    let lim = CHARTAB_SIZE[depth];
    let from = from.max(min);
    let mut idx = (from.saturating_sub(min) >> CHARTAB_BITS[depth]) as usize;
    let mut c = min + chars * idx as u32;
    while idx < lim && c <= to {
        if from <= c && c + chars - 1 <= to {
            r.borrow_mut()[3 + idx] = val.clone();
        } else {
            let child = r.borrow()[3 + idx].clone();
            let child = if is_sub_ct(i, &child) {
                child
            } else if uniprop && uniprop_compressed(&child) {
                match i.q_soft("sub-char-table") {
                    Some(t) => uniprop_uncompress(t, &r, 3 + idx, min + (idx as u32) * 128),
                    None => child,
                }
            } else {
                let n = make_sub_ct(i, depth + 1, c, child);
                r.borrow_mut()[3 + idx] = n.clone();
                n
            };
            sub_ct_set_range(i, &child, from, to, val, uniprop);
        }
        idx += 1;
        c += chars;
    }
}

/// GNU `char_table_set_range': range write with whole-block
/// collapsing (FROM and TO inclusive).
pub(crate) fn ct_set_range(i: &mut Interp, table: &Value, from: u32, to: u32, val: Value) {
    if from == to {
        ct_set(i, table, from, val);
        return;
    }
    let Some(contents) = char_table_vec(table) else {
        return;
    };
    let to = to.min(CT_MAX_CHAR);
    let uniprop = is_uniprop_table(i, table);
    let mut ti = (from >> 16) as usize;
    let lim = (to >> 16) as usize;
    while ti <= lim {
        let c = (ti as u32) << 16;
        if c > to {
            break;
        }
        if from <= c && c + 65535 <= to {
            contents.borrow_mut()[1 + ti] = val.clone();
        } else {
            let sub = contents.borrow()[1 + ti].clone();
            let sub = if is_sub_ct(i, &sub) {
                sub
            } else {
                let n = make_sub_ct(i, 1, c, sub);
                contents.borrow_mut()[1 + ti] = n.clone();
                n
            };
            sub_ct_set_range(i, &sub, from, to, &val, uniprop);
        }
        ti += 1;
    }
    if from < 128 {
        ct_refresh_ascii(i, &contents);
    }
}

/// GNU's `char-width-table' char-table: subtype nil, nil defalt, and
/// a parent holding all the width data — replayed from
/// CHAR_WIDTH_RANGES (extracted from Emacs 31's effective `aref'
/// values).
fn f_remacs_char_width_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU's `char-width-table' is an empty char-table whose parent
    // holds all the width data (charset.c `syms_of_charset_for_lisp').
    let parent = make_ct(i, Value::Nil, Value::Nil, vec![]);
    for &(s, e, w) in crate::lisp::ctdata::CHAR_WIDTH_RANGES {
        ct_set_range(i, &parent, s, e, Value::Int(w as i128));
    }
    let t = make_ct(i, Value::Nil, Value::Nil, vec![]);
    i.set_char_table_parent(&t, parent);
    Ok(t)
}

/// Deep copy of a sub-char-table value (`copy_sub_char_table').
fn sub_ct_copy(i: &mut Interp, v: &Value) -> Value {
    let Some((depth, min, r)) = sub_ct_parts(v) else {
        return v.clone();
    };
    let n = CHARTAB_SIZE[depth];
    let mut rec = Vec::with_capacity(3 + n);
    rec.push(Value::Sym(i.q("sub-char-table")));
    rec.push(Value::Int(depth as i128));
    rec.push(Value::Int(min as i128));
    {
        let b = r.borrow();
        for slot in b.iter().skip(3).take(n) {
            rec.push(if is_sub_ct(i, slot) {
                sub_ct_copy(i, slot)
            } else {
                slot.clone()
            });
        }
    }
    Value::record(rec)
}

/// `copy_char_table': deep copy of TABLE's contents (recursive
/// sub-tables), defalt, parent, purpose and extra slots.  The ASCII
/// slot is re-derived from the copied trie.
pub(crate) fn ct_copy(i: &mut Interp, table: &Value) -> Value {
    let Value::Record(r) = table else {
        return table.clone();
    };
    let (subtype, contents, extras) = {
        let b = r.borrow();
        let subtype = b.get(1).cloned().unwrap_or(Value::Nil);
        let contents = match b.get(2) {
            Some(Value::Vec(v)) => Some(v.clone()),
            _ => None,
        };
        let extras: Vec<Value> = b.iter().skip(3).cloned().collect();
        (subtype, contents, extras)
    };
    let Some(contents) = contents else {
        return table.clone();
    };
    let copied: Vec<Value> = {
        let b = contents.borrow();
        b.iter()
            .skip(1)
            .map(|v| {
                if is_sub_ct(i, v) {
                    sub_ct_copy(i, v)
                } else {
                    v.clone()
                }
            })
            .collect()
    };
    let mut nv = Vec::with_capacity(65);
    nv.push(Value::Nil);
    nv.extend(copied);
    let contents_rc = crate::lisp::Value::vector_ref(nv);
    ct_refresh_ascii(i, &contents_rc);
    let mut rec = vec![
        Value::Sym(i.q("char-table")),
        subtype,
        Value::Vec(contents_rc),
    ];
    rec.extend(extras);
    let t = Value::record(rec);
    let defalt = i.char_table_defalt(table);
    if !defalt.is_nil() {
        i.set_char_table_defalt(&t, defalt);
    }
    let parent = i.char_table_parent(table);
    if !parent.is_nil() {
        i.set_char_table_parent(&t, parent);
    }
    t
}

/// Replay GNU-extracted `(R LO HI VAL)'/`(L OFF VALS)' ops (see
/// `ctdata.rs') into TABLE — reproduces GNU's exact trie structure
/// and contents.  Range ops first is not required: every slot GNU
/// shows gets its own op.
pub(crate) fn ct_replay(i: &mut Interp, table: &Value, ops_src: &str) {
    if i.dbg_flags & 0x10000 != 0 {
        eprintln!("[ct_replay] start");
    }
    let mut r = crate::lisp::reader::Reader::new(i, ops_src);
    let ops = match r.read() {
        Ok(Some(v)) => v,
        _ => return,
    };
    if i.dbg_flags & 0x10000 != 0 {
        eprintln!("[ct_replay] read done");
    }
    let mut n = 0usize;
    ops.each_car(|op| {
        n += 1;
        if i.dbg_flags & 0x10000 != 0 {
            eprintln!("[ct_replay] op {}", n);
        }
        let items = op.list_to_vec().unwrap_or_default();
        let tag = match items.first() {
            Some(Value::Sym(s)) => i.symbol_name(*s).to_string(),
            _ => return,
        };
        match tag.as_str() {
            "R" => {
                if let (Some(Value::Int(lo)), Some(Value::Int(hi)), Some(v)) =
                    (items.get(1), items.get(2), items.get(3))
                {
                    ct_set_range(i, table, *lo as u32, *hi as u32, v.clone());
                }
            }
            "L" => {
                if let (Some(Value::Int(off)), Some(vals)) = (items.get(1), items.get(2)) {
                    if let Some(vs) = vals.list_to_vec().ok() {
                        for (k, v) in vs.iter().enumerate() {
                            ct_set(i, table, *off as u32 + k as u32, v.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

/// `optimize_sub_char_table': collapse a sub-table whose slots all
/// compare equal (default `equal'; `eq' optimized).  Children are
/// optimized bottom-up first.
fn ct_same(
    i: &mut Interp,
    a: &Value,
    b: &Value,
    test: &Value,
    eq_test: bool,
) -> Result<bool, Flow> {
    if eq_test {
        Ok(super::eq_values(a, b))
    } else if test.is_nil() {
        super::equal_values(i, a, b)
    } else {
        // A/B are values: quote them so `call_function's
        // unevaluated-arg protocol doesn't re-eval a symbol like
        // `cdr' as a variable (void-variable).
        let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
        i.call_function(test, &Value::list(vec![q(a), q(b)]), None)
            .map(|v| !v.is_nil())
    }
}

fn sub_ct_optimize(i: &mut Interp, sub: &Value, test: &Value) -> EvalResult {
    let Some((depth, _, r)) = sub_ct_parts(sub) else {
        return Ok(sub.clone());
    };
    let n = CHARTAB_SIZE[depth];
    let eq_test = matches!(test, Value::Sym(s) if i.symbol_name(*s) == "eq");
    let test_fn = test.clone();
    let mut first = {
        let b = r.borrow();
        b.get(3).cloned().unwrap_or(Value::Nil)
    };
    if is_sub_ct(i, &first) {
        first = sub_ct_optimize(i, &first, &test_fn)?;
        r.borrow_mut()[3] = first.clone();
    }
    let mut optimizable = !is_sub_ct(i, &first);
    for k in 1..n {
        let mut this = {
            let b = r.borrow();
            b.get(3 + k).cloned().unwrap_or(Value::Nil)
        };
        if is_sub_ct(i, &this) {
            this = sub_ct_optimize(i, &this, &test_fn)?;
            r.borrow_mut()[3 + k] = this.clone();
        }
        if optimizable && !ct_same(i, &this, &first, &test_fn, eq_test)? {
            optimizable = false;
        }
    }
    Ok(if optimizable { first } else { sub.clone() })
}

/// `Foptimize_char_table' inner: optimize each top-level slot and
/// refresh the ASCII cache.
fn ct_optimize(i: &mut Interp, table: &Value, test: &Value) -> Result<(), Flow> {
    let Some(contents) = char_table_vec(table) else {
        return Ok(());
    };
    for ti in 1..=64usize {
        let sub = contents.borrow()[ti].clone();
        if is_sub_ct(i, &sub) {
            let newv = sub_ct_optimize(i, &sub, test)?;
            contents.borrow_mut()[ti] = newv;
        }
    }
    ct_refresh_ascii(i, &contents);
    Ok(())
}

/// GNU `char_table_ascii' for a uniprop table: a compressed string in
/// the ASCII cache slot (chars 0-127) expands into a depth-3 leaf.
fn ct_uncompress_ascii(i: &Interp, v: &Value, vec: &Rc<RefCell<Vec<Value>>>) {
    let Some(tag) = i.q_soft("sub-char-table") else {
        return;
    };
    let cur = vec.borrow().first().cloned().unwrap_or(Value::Nil);
    if is_uniprop_table(i, v) && uniprop_compressed(&cur) {
        uniprop_uncompress(tag, vec, 0, 0);
    }
}

/// Raw contents of TABLE for char C — no defalt/parent inheritance.
pub(crate) fn char_table_raw(i: &Interp, v: &Value, idx: usize) -> Value {
    match char_table_vec(v) {
        Some(vec) => {
            if idx < 128 {
                ct_uncompress_ascii(i, v, &vec);
            }
            let b = vec.borrow();
            // Flat legacy tables (bare vectors / old-style 256-vec
            // contents) index directly.
            if b.len() == 256 {
                return b.get(idx).cloned().unwrap_or(Value::Nil);
            }
            ct_raw(i, &b, idx as u32, is_uniprop_table(i, v))
        }
        None => Value::Nil,
    }
}

/// `char_table_ref_simple' top-level step: raw value for C, falling
/// back to this table's own defalt (never the parent — GNU's
/// cons-range `char-table-range' semantics).
fn ct_ref_defalt(i: &Interp, table: &Value, idx: usize) -> Value {
    let raw = char_table_raw(i, table, idx);
    if !raw.is_nil() {
        return raw;
    }
    i.char_table_defalt(table)
}

/// GNU `char_table_ref': raw content slot → this table's defalt →
/// parent chain (defalt checked at each level before following the
/// parent link).  Used by `aref'/`elt' and syntax lookups; NOT by
/// `char-table-range' with a cons range (which skips the parent).
/// Read-only access to a char-table chain without retaining an Interp
/// borrow.  Regexp matching uses this while its buffer is borrowed.
pub(crate) struct CharTableSnapshot {
    tag: Option<SymId>,
    levels: Vec<(Rc<RefCell<Vec<Value>>>, Value, bool)>,
}

impl CharTableSnapshot {
    pub(crate) fn new(i: &Interp, table: &Value) -> Option<Self> {
        if !is_char_table(i, table) {
            return None;
        }
        let mut levels = Vec::new();
        let mut current = table.clone();
        for _ in 0..64 {
            let contents = char_table_vec(&current)?;
            ct_uncompress_ascii(i, &current, &contents);
            levels.push((
                contents,
                i.char_table_defalt(&current),
                is_uniprop_table(i, &current),
            ));
            current = i.char_table_parent(&current);
            if current.is_nil() {
                break;
            }
        }
        Some(Self {
            tag: i.q_soft("sub-char-table"),
            levels,
        })
    }

    pub(crate) fn get(&self, code: u32) -> Value {
        for (contents, default, uniprop) in &self.levels {
            let contents = contents.borrow();
            let value = if contents.len() == 256 {
                contents.get(code as usize).cloned().unwrap_or(Value::Nil)
            } else {
                ct_raw_tag(self.tag, &contents, code, *uniprop)
            };
            if !value.is_nil() {
                return value;
            }
            if !default.is_nil() {
                return default.clone();
            }
        }
        Value::Nil
    }
}

pub(crate) fn char_table_ref(i: &Interp, table: &Value, idx: usize) -> Value {
    let mut cur = table.clone();
    for _ in 0..64 {
        let val = match char_table_vec(&cur) {
            Some(v) => {
                if idx < 128 {
                    ct_uncompress_ascii(i, &cur, &v);
                }
                let b = v.borrow();
                if b.len() == 256 {
                    b.get(idx).cloned().unwrap_or(Value::Nil)
                } else {
                    ct_raw(i, &b, idx as u32, is_uniprop_table(i, &cur))
                }
            }
            None => return Value::Nil,
        };
        if !val.is_nil() {
            return val;
        }
        let defalt = i.char_table_defalt(&cur);
        if !defalt.is_nil() {
            return defalt;
        }
        let parent = i.char_table_parent(&cur);
        if parent.is_nil() {
            return Value::Nil;
        }
        cur = parent;
    }
    Value::Nil
}

fn f_char_table_range(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fchar_table_range': nil RANGE reads the table's own
    // default; a character RANGE resolves via `char_table_ref'
    // (raw → defalt → parent); a cons (FROM . TO) returns the value
    // for FROM via `char_table_ref_and_range' (raw → defalt, no
    // parent).  Other ranges are an error.
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    match &a[1] {
        Value::Nil => Ok(i.char_table_defalt(&a[0])),
        Value::Int(n) => {
            // GNU: an out-of-character-range fixnum falls through to
            // the generic "Invalid RANGE" error, not `characterp'.
            if !(0..=CT_MAX_CHAR as i128).contains(n) {
                return Err(i.error_q("Invalid RANGE argument to `char-table-range'", &[]));
            }
            Ok(char_table_ref(i, &a[0], *n as usize))
        }
        Value::Cons(c) => {
            let (from, to) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            let from = match &from {
                Value::Int(f) if (0..=CT_MAX_CHAR as i128).contains(f) => *f as usize,
                _ => return Err(i.wrong_type_mut("characterp", &from)),
            };
            match &to {
                Value::Int(t) if (0..=CT_MAX_CHAR as i128).contains(t) => {}
                _ => return Err(i.wrong_type_mut("characterp", &to)),
            }
            Ok(ct_ref_defalt(i, &a[0], from))
        }
        _ => Err(i.error_q("Invalid RANGE argument to `char-table-range'", &[])),
    }
}

fn f_char_table_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    Ok(i.char_table_parent(&a[0]))
}

fn f_set_char_table_parent(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    if !a[1].is_nil() && !is_char_table(i, &a[1]) {
        return Err(i.wrong_type_mut("char-table-p", &a[1]));
    }
    // GNU: "Attempt to make a chartable be its own parent".
    let mut temp = a[1].clone();
    for _ in 0..64 {
        if temp.is_nil() {
            break;
        }
        if super::eq_values(&temp, &a[0]) {
            return Err(i.error("Attempt to make a chartable be its own parent"));
        }
        temp = i.char_table_parent(&temp);
    }
    i.set_char_table_parent(&a[0], a[1].clone());
    Ok(a[1].clone())
}

fn f_set_char_table_range(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fset_char_table-range': RANGE t sets the ASCII slot and
    // all 64 top-level slots (defalt untouched); nil sets the defalt;
    // a char splits the trie path; a cons does a block-optimized
    // range write.  Returns VALUE.
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let val = a[2].clone();
    match &a[1] {
        Value::Sym(s) if i.symbol_name(*s) == "t" => {
            if let Some(v) = char_table_vec(&a[0]) {
                let mut vv = v.borrow_mut();
                for slot in vv.iter_mut() {
                    *slot = val.clone();
                }
            }
        }
        Value::Nil => {
            i.set_char_table_defalt(&a[0], val.clone());
        }
        Value::Int(n) => {
            if !(0..=CT_MAX_CHAR as i128).contains(n) {
                return Err(i.error_q("Invalid RANGE argument to `set-char-table-range'", &[]));
            }
            ct_set(i, &a[0], *n as u32, val.clone());
        }
        Value::Cons(c) => {
            let (from, to) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            let from = match &from {
                Value::Int(f) if (0..=CT_MAX_CHAR as i128).contains(f) => *f as u32,
                _ => return Err(i.wrong_type_mut("characterp", &from)),
            };
            let to = match &to {
                Value::Int(t) if (0..=CT_MAX_CHAR as i128).contains(t) => *t as u32,
                _ => return Err(i.wrong_type_mut("characterp", &to)),
            };
            // GNU: a reversed (FROM > TO) cons silently sets nothing.
            if from <= to {
                ct_set_range(i, &a[0], from, to, val.clone());
            }
        }
        _ => return Err(i.error_q("Invalid RANGE argument to `set-char-table-range'", &[])),
    }
    Ok(val)
}

/// Walk the raw trie in char order, producing (FROM TO VAL) spans —
/// every character is covered by exactly one span (scalar slots
/// report their whole block).  Used by `map-char-table' and the
/// keymap iterators.  UNIPROP mirrors GNU's `map_sub_char_table':
/// compressed leaf strings expand into their 128 slots in place.
fn ct_walk(
    i: &Interp,
    v: &Value,
    from: u32,
    to: u32,
    uniprop: bool,
    out: &mut Vec<(u32, u32, Value)>,
) {
    if is_sub_ct(i, v) {
        if let Some((depth, min, r)) = sub_ct_parts(v) {
            for k in 0..CHARTAB_SIZE[depth] {
                let lo = min + (k as u32) * (1 << CHARTAB_BITS[depth]);
                let hi = lo + (1 << CHARTAB_BITS[depth]) - 1;
                let child = {
                    let b = r.borrow();
                    b.get(3 + k).cloned().unwrap_or(Value::Nil)
                };
                let child = if uniprop && uniprop_compressed(&child) {
                    match i.q_soft("sub-char-table") {
                        Some(t) => uniprop_uncompress(t, &r, 3 + k, lo),
                        None => child,
                    }
                } else {
                    child
                };
                ct_walk(i, &child, lo, hi.min(to), uniprop, out);
            }
        }
        return;
    }
    out.push((from, to, v.clone()));
}

/// `ct_walk' over the whole table — 64 top slots of 65536 chars.
fn ct_spans(i: &Interp, table: &Value, uniprop: bool) -> Vec<(u32, u32, Value)> {
    let mut out = Vec::new();
    if let Some(contents) = char_table_vec(table) {
        let b = contents.borrow();
        for k in 0..64usize {
            let lo = (k as u32) << 16;
            let slot = b.get(1 + k).cloned().unwrap_or(Value::Nil);
            ct_walk(i, &slot, lo, lo + 65535, uniprop, &mut out);
        }
    }
    out
}

/// Effective-value runs clipped to `[LO, HI]`: descend only into
/// sub-tables overlapping the range, so untouched space costs
/// nothing per character.  Each emitted leaf is resolved through
/// `char_table_ref' — raw → own defalt → parent chain.  Adjacent
/// runs with `eq' values merge via `merge_runs'.  Used by
/// `modify-category-entry' to find run boundaries in O(leaves)
/// instead of O(characters).
fn ct_runs_in(i: &Interp, table: &Value, lo: u32, hi: u32) -> Vec<(u32, u32, Value)> {
    fn walk(
        i: &Interp,
        v: &Value,
        from: u32,
        to: u32,
        lo: u32,
        hi: u32,
        table: &Value,
        out: &mut Vec<(u32, u32, Value)>,
    ) {
        if from > hi || to < lo {
            return;
        }
        if is_sub_ct(i, v) {
            if let Some((depth, min, r)) = sub_ct_parts(v) {
                let chars = 1u32 << CHARTAB_BITS[depth];
                for k in 0..CHARTAB_SIZE[depth] {
                    let c = min + (k as u32) * chars;
                    if c > hi || c + chars - 1 < lo {
                        continue;
                    }
                    let child = {
                        let b = r.borrow();
                        b.get(3 + k).cloned().unwrap_or(Value::Nil)
                    };
                    walk(i, &child, c, c + chars - 1, lo, hi, table, out);
                }
            }
            return;
        }
        let f = from.max(lo);
        let t = to.min(hi);
        // A scalar slot's value is uniform over its whole block.
        // For a nil leaf the effective value comes from this
        // table's defalt first (uniform for the span); only when
        // the defalt is also nil do we follow the parent — whose
        // runs may vary inside the span, so they are spliced in
        // with a clipped recursive walk.
        if !v.is_nil() {
            out.push((f, t, v.clone()));
            return;
        }
        let defalt = i.char_table_defalt(table);
        if !defalt.is_nil() {
            out.push((f, t, defalt));
            return;
        }
        let parent = i.char_table_parent(table);
        if parent.is_nil() {
            out.push((f, t, Value::Nil));
        } else {
            out.extend(ct_runs_in(i, &parent, f, t));
        }
    }
    let mut out = Vec::new();
    if let Some(contents) = char_table_vec(table) {
        let b = contents.borrow();
        for k in 0..64usize {
            let base = (k as u32) << 16;
            if base > hi || base + 65535 < lo {
                continue;
            }
            let slot = b.get(1 + k).cloned().unwrap_or(Value::Nil);
            walk(i, &slot, base, base + 65535, lo, hi, table, &mut out);
        }
    }
    merge_runs(out)
}

/// Merge adjacent spans with `eq' values (GNU's range coalescing).
fn merge_runs(spans: Vec<(u32, u32, Value)>) -> Vec<(u32, u32, Value)> {
    let mut out: Vec<(u32, u32, Value)> = Vec::with_capacity(spans.len());
    for (f, t, v) in spans {
        if let Some((_, pt, pv)) = out.last_mut() {
            if super::eq_values(pv, &v) && *pt + 1 == f {
                *pt = t;
                continue;
            }
        }
        out.push((f, t, v));
    }
    out
}

/// Effective (raw → defalt) runs of TABLE; nil-value regions are
/// spliced with the parent's own runs when `with_parent' — GNU's
/// `map_char_table' semantics (the parent's own parent is excluded).
fn ct_effective_runs(i: &Interp, table: &Value, with_parent: bool) -> Vec<(u32, u32, Value)> {
    let defalt = i.char_table_defalt(table);
    let spans = ct_spans(i, table, is_uniprop_table(i, table))
        .into_iter()
        .map(|(f, t, v)| (f, t, if v.is_nil() { defalt.clone() } else { v }))
        .collect();
    let runs = merge_runs(spans);
    let parent = i.char_table_parent(table);
    if with_parent && !parent.is_nil() {
        let parent_runs = ct_effective_runs(i, &parent, false);
        let mut out = Vec::new();
        for (f, t, v) in runs {
            if v.is_nil() {
                // Splice parent's own runs clipped to [f, t].
                let mut any = false;
                for (pf, pt, pv) in &parent_runs {
                    let (lo, hi) = ((*pf).max(f), (*pt).min(t));
                    if lo <= hi {
                        out.push((lo, hi, pv.clone()));
                        any = true;
                    }
                }
                if !any {
                    out.push((f, t, Value::Nil));
                }
            } else {
                out.push((f, t, v));
            }
        }
        merge_runs(out)
    } else {
        runs
    }
}

/// Raw non-nil runs of TABLE — for keymap iteration, which treats
/// nil slots as unbound.
pub(crate) fn ct_collect(i: &Interp, table: &Value) -> Vec<(u32, u32, Value)> {
    merge_runs(ct_spans(i, table, false))
        .into_iter()
        .filter(|(_, _, v)| !v.is_nil())
        .collect()
}

fn f_map_char_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[1]) {
        return Err(i.wrong_type_mut("char-table-p", &a[1]));
    }
    // GNU's `uniprop_get_decoder': emitted run values are decoded only
    // when extra slot 1 is the fixnum decoder index (0 = run-length
    // through the extra-slot-4 value vector); a Lisp-function slot
    // (word-list tables) leaves the raw value untouched.
    let decode = matches!(&a[1], Value::Record(r) if matches!(r.borrow().get(4), Some(Value::Int(0))))
        && is_uniprop_table(i, &a[1]);
    let runs = ct_effective_runs(i, &a[1], true);
    // GNU's `map_char_table' only calls the function for ranges whose
    // effective value is non-nil — unset regions (and nil stores,
    // which revert to the default) are skipped entirely.  GNU passes
    // ONE shared RANGE cons it mutates between calls (a lone
    // character is passed as a fixnum) — retained keys all end up
    // showing the final mutation; mirror that.
    let range = Value::cons(Value::Int(0), Value::Int(0));
    let mut last: Option<(u32, u32)> = None;
    for (from, to, val) in runs.into_iter().filter(|(_, _, v)| !v.is_nil()) {
        last = Some((from, to));
        // GNU updates the shared range even when this callback receives
        // a fixnum. A previously retained cons must already show the
        // singleton bounds while that callback is running (or exits).
        if let Value::Cons(c) = &range {
            let mut cc = c.borrow_mut();
            cc.car = Value::Int(from as i128);
            cc.cdr = Value::Int(to as i128);
        }
        let key = if from == to {
            Value::Int(from as i128)
        } else {
            range.clone()
        };
        let val = if decode {
            super::charset::uniprop_decode_run_length(&a[1], val)
        } else {
            val
        };
        i.apply(&a[0], vec![key, val])?;
    }
    // GNU leaves its shared RANGE cons at `(FROM . TO)' where FROM is
    // the block start after the last emitted run (i.e. last-to + 1,
    // or the run's own `from' when it reached MAX_CHAR) and TO is
    // MAX_CHAR — a callback that retains the cons sees this state.
    if let Some((from, to)) = last {
        if let Value::Cons(c) = &range {
            let mut cc = c.borrow_mut();
            cc.car = Value::Int(if to < CT_MAX_CHAR { to + 1 } else { from } as i128);
            cc.cdr = Value::Int(CT_MAX_CHAR as i128);
        }
    }
    Ok(Value::Nil)
}

/// Record slot 3+N of a char-table (the extra slots).
pub(crate) fn ct_extra(table: &Value, n: usize) -> Value {
    match table {
        Value::Record(r) => r.borrow().get(3 + n).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

fn ct_set_extra(table: &Value, n: usize, v: Value) {
    if let Value::Record(r) = table {
        if r.borrow().len() > 3 + n {
            r.borrow_mut()[3 + n] = v;
        }
    }
}

/// GNU casetab.c `set_case_table' (the shared body of `set-case-table'
/// and `set-standard-case-table'): derive the UPCASE/CANON/EQV extra
/// slots of TABLE when they are nil.  GNU builds each table with
/// `map_char_table' over the *effective* entries of the source table:
///
///   up:    identity pass, then `shuffle' pass cycling each class
///   canon: canon[range] = down[up[down[c]]]   (FIXNATP elts only)
///   eqv:   identity + shuffle over the canon table's runs
///
/// and finally stashes `canon.extras[2] = eqv' so regexp's
/// set_image_of_range_1 can find it.
pub(crate) fn set_case_table_fill(i: &mut Interp, table: &Value) {
    let case_table_sym = Value::Sym(i.q("case-table"));
    let up = ct_extra(table, 0);
    let canon = ct_extra(table, 1);
    let eqv = ct_extra(table, 2);

    let up = if up.is_nil() {
        let u = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        // set_identity pass over TABLE's runs, then shuffle pass.
        // GNU's callbacks act only on FIXNATP elts — nil-valued runs
        // (and strings/symbols) contribute nothing to either pass.
        let runs: Vec<(u32, u32, u32)> = ct_effective_runs(i, table, true)
            .into_iter()
            .filter_map(|(f, t, v)| match v {
                Value::Int(e) if e >= 0 => Some((f, t, e as u32)),
                _ => None,
            })
            .collect();
        for (from, to, _) in &runs {
            for c in *from..=*to {
                ct_set(i, &u, c, Value::Int(c as i128));
            }
        }
        for (from, to, e) in &runs {
            for c in *from..=*to {
                let tem = char_table_ref(i, &u, *e as usize);
                ct_set(i, &u, *e, Value::Int(c as i128));
                ct_set(i, &u, c, tem.clone());
            }
        }
        ct_set_extra(table, 0, u.clone());
        u
    } else {
        up
    };

    let canon = if canon.is_nil() {
        let cn = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        ct_set_extra(table, 1, cn.clone());
        // set_canon: canon[range] = table[up[elt]].
        for (from, to, elt) in ct_effective_runs(i, table, true) {
            let Value::Int(e) = elt else { continue };
            if e < 0 {
                continue;
            }
            let uv = char_table_ref(i, &up, e as usize);
            let dv = match uv {
                Value::Int(n) if n >= 0 => char_table_ref(i, table, n as usize),
                _ => Value::Nil,
            };
            ct_set_range(i, &cn, from, to, dv);
        }
        cn
    } else {
        canon
    };

    if eqv.is_nil() {
        let eq = make_ct(
            i,
            case_table_sym.clone(),
            Value::Nil,
            vec![Value::Nil, Value::Nil, Value::Nil],
        );
        let runs: Vec<(u32, u32, u32)> = ct_effective_runs(i, &canon, true)
            .into_iter()
            .filter_map(|(f, t, v)| match v {
                Value::Int(e) if e >= 0 => Some((f, t, e as u32)),
                _ => None,
            })
            .collect();
        for (from, to, _) in &runs {
            for c in *from..=*to {
                ct_set(i, &eq, c, Value::Int(c as i128));
            }
        }
        for (from, to, e) in &runs {
            for c in *from..=*to {
                let tem = char_table_ref(i, &eq, *e as usize);
                ct_set(i, &eq, *e, Value::Int(c as i128));
                ct_set(i, &eq, c, tem.clone());
            }
        }
        ct_set_extra(table, 2, eq.clone());
        // canon.extras[2] = eqv (regex's set_image_of_range_1).
        ct_set_extra(&canon, 2, eq);
    }
}

/// GNU `buffer.c:1150' (`init_buffer'): if the standard case table's
/// extras aren't all char-tables, re-run `set_case_table' on it so
/// the extras recompute.  Called wherever GNU would create a buffer.
pub(crate) fn heal_standard_case_table(i: &mut Interp) {
    let Some(st) = i.standard_case_table.clone() else {
        return;
    };
    let ok = (0..3).all(|n| is_char_table(i, &ct_extra(&st, n)));
    if !ok {
        set_case_table_fill(i, &st);
    }
}

fn f_suppress_keymap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_keymap(i, &a[0]) {
        return Err(i.wrong_type_mut("keymapp", &a[0]));
    }
    // GNU remaps self-insert-command, preserving explicit bindings.
    let nodigits = arg(&a, 1).truthy();
    let define_key = Value::Sym(i.q("define-key"));
    let keys = Value::vector(vec![
        Value::Sym(i.q("remap")),
        Value::Sym(i.q("self-insert-command")),
    ]);
    let undefined = Value::Sym(i.q("undefined"));
    i.apply(&define_key, vec![a[0].clone(), keys, undefined])?;
    if !nodigits {
        let dig = Value::Sym(i.q("digit-argument"));
        for code in '0' as i128..='9' as i128 {
            crate::editor::set_binding(i, &a[0], code, dig.clone());
        }
        let neg = Value::Sym(i.q("negative-argument"));
        crate::editor::set_binding(i, &a[0], '-' as i128, neg);
    }
    // GNU returns the NODIGITS argument (nil by default).
    Ok(arg(&a, 1))
}

/// Record slot 1 of a char-table holds its subtype symbol.
fn char_table_subtype_of(v: &Value) -> Value {
    match v {
        Value::Record(r) => r.borrow().get(1).cloned().unwrap_or(Value::Nil),
        _ => Value::Nil,
    }
}

fn f_char_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_char_table(i, &a[0])))
}

fn f_char_table_subtype(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    Ok(char_table_subtype_of(&a[0]))
}

fn f_char_table_extra_slot(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Fchar_table_extra_slot': CHECK_CHAR_TABLE, then N is
    // `fixnump' (non-fixnums are `(fixnump N)'; negatives fall to
    // `args-out-of-range').
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let Value::Record(r) = &a[0] else {
        unreachable!()
    };
    let Value::Int(n) = &a[1] else {
        return Err(i.wrong_type_mut("fixnump", &a[1]));
    };
    let slots = r.borrow().len().saturating_sub(3);
    let n = usize::try_from(*n).ok().filter(|u| *u < slots);
    let Some(n) = n else {
        let s = i.q("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    };
    Ok(r.borrow()[3 + n].clone())
}

fn f_set_char_table_extra_slot(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let Value::Record(r) = &a[0] else {
        unreachable!()
    };
    let Value::Int(n) = &a[1] else {
        return Err(i.wrong_type_mut("fixnump", &a[1]));
    };
    let mut rr = r.borrow_mut();
    let slots = rr.len().saturating_sub(3);
    let n = usize::try_from(*n).ok().filter(|u| *u < slots);
    let Some(n) = n else {
        let s = i.q("args-out-of-range");
        return Err(i.signal_data(s, vec![a[0].clone(), a[1].clone()]));
    };
    rr[3 + n] = a[2].clone();
    Ok(a[2].clone())
}

fn f_record(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::record(a.as_slice().to_vec()))
}

fn f_recordp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // `symbols-with-pos-enabled' objects are symbols, not records;
    // obarrays are their own pseudovec type in GNU (PVEC_OBARRAY,
    // distinct from PVEC_RECORD) so `recordp' is nil on them.
    // A font-spec is stored as a record internally but is a
    // pseudovector (PVEC_FONT), not a record, in GNU — `recordp' is
    // nil on it (type-of reports `font-spec').
    let font_spec = match &a[0] {
        Value::Record(r) => {
            let rb = r.borrow();
            matches!(
                rb.first(),
                Some(Value::Sym(s)) if i.symbol_name(*s) == "font-spec"
            )
        }
        _ => false,
    };
    Ok(Value::from_bool(
        matches!(a[0], Value::Record(_))
            && !crate::editor::window_configuration_object(&a[0])
            && !font_spec
            && as_sympos(&a[0]).is_none()
            && !crate::lisp::builtins::data::is_obarray(i, &a[0]),
    ))
}

// ---------- added GNU compat subrs ----------

/// Conses reachable from LIST's head for length=/length</length>:
/// GNU counts elements, stopping silently at a non-list tail (dotted
/// lists don't error), and a circular list is unbounded (reported as
/// i128::MAX so < is false, > is true).
fn cons_seq_len(v: &Value) -> i128 {
    let mut n = 0i128;
    let mut cur = v.clone();
    let mut tortoise = v.clone();
    loop {
        match &cur {
            Value::Cons(c) => {
                let next = c.borrow().cdr.clone();
                cur = next;
                n += 1;
                if n % 2 == 0 {
                    if let Value::Cons(tc) = &tortoise {
                        let t = tc.borrow().cdr.clone();
                        tortoise = t;
                    }
                }
                if n > 1 && eq_values(&cur, &tortoise) {
                    return i128::MAX;
                }
            }
            _ => return n,
        }
    }
}

fn seq_len(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    Ok(match v {
        Value::Nil => 0,
        Value::Cons(_) => cons_seq_len(v),
        Value::Str(s) => s.borrow().chars().count() as i128,
        Value::Vec(x) => x.borrow().len() as i128,
        // Bool-vectors are sequences of their bit count; char-tables
        // cover the whole internal char space (MAX_CHAR + 1).
        Value::Record(_) if is_bool_vector(i, v) => bool_vec_of(i, v)?.len() as i128,
        Value::Record(_) if is_char_table(i, v) => CT_MAX_CHAR as i128 + 1,
        Value::Record(x) => x.borrow().len() as i128,
        // Hash tables aren't sequences (GNU signals sequencep).
        _ => return Err(i.wrong_type_mut("sequencep", v)),
    })
}

fn length_cmp(i: &mut Interp, a: &[Value], cmp: i8) -> EvalResult {
    // GNU Flength_less/eq/greater checks LENGTH (arg2, CHECK_FIXNUM)
    // before SEQUENCE (arg1) — `(length< 's 'x)' → fixnump x.
    let len = match &a[1] {
        Value::Int(x) => *x,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let n = seq_len(i, &a[0])?;
    Ok(Value::from_bool(match cmp {
        -1 => n < len,
        1 => n > len,
        _ => n == len,
    }))
}

fn f_length_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, -1)
}
fn f_length_gt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, 1)
}
fn f_length_eq(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    length_cmp(i, &a, 0)
}

/// GNU `value<': compares only within the same broad type —
/// numbers numerically, strings lexically, symbols by name,
/// conses/vectors elementwise; any type mismatch signals
/// (error 'type-mismatch), and otherwise-comparable pairs with no
/// ordering answer yield nil.
fn value_lt_impl(i: &mut Interp, x: &Value, y: &Value, depth: u32) -> Result<bool, Flow> {
    if depth > 64 {
        return Err(i.error("Circular object"));
    }
    fn mism(i: &mut Interp) -> Flow {
        let s = i.q("type-mismatch");
        i.signal_data(s, vec![])
    }
    Ok(match (x, y) {
        (Value::Int(p), Value::Int(q)) => p < q,
        (Value::Int(p), Value::Float(q)) => (*p as f64) < **q,
        (Value::Float(p), Value::Int(q)) => **p < (*q as f64),
        (Value::Float(p), Value::Float(q)) => p < q,
        (Value::Int(_) | Value::Float(_), _) => return Err(mism(i)),
        (Value::Sym(a), Value::Sym(b)) => {
            if a == b {
                false
            } else if x.is_nil() {
                true
            } else if y.is_nil() {
                false
            } else {
                i.symbol_name(*a) < i.symbol_name(*b)
            }
        }
        (Value::Sym(_), _) => return Err(mism(i)),
        (Value::Str(p), Value::Str(q)) => *p.borrow() < *q.borrow(),
        (Value::Str(_), _) => return Err(mism(i)),
        (Value::Cons(_), Value::Cons(_)) => {
            let xs = match want_list(i, x) {
                Ok(v) => v,
                Err(f) => return Err(f),
            };
            let ys = match want_list(i, y) {
                Ok(v) => v,
                Err(f) => return Err(f),
            };
            let mut lt = false;
            for k in 0..xs.len().max(ys.len()) {
                match (xs.get(k), ys.get(k)) {
                    (None, Some(_)) => {
                        lt = true;
                        break;
                    }
                    (None, None) => break,
                    (Some(_), None) => {
                        lt = false;
                        break;
                    }
                    (Some(a), Some(b)) => {
                        if !crate::lisp::builtins::equal_values(i, a, b)? {
                            lt = value_lt_impl(i, a, b, depth + 1)?;
                            break;
                        }
                    }
                }
            }
            lt
        }
        (Value::Cons(_), _) => return Err(mism(i)),
        (Value::Vec(p), Value::Vec(q)) => {
            let xs = p.borrow().clone();
            let ys = q.borrow().clone();
            let mut lt = false;
            for k in 0..xs.len().max(ys.len()) {
                match (xs.get(k), ys.get(k)) {
                    (None, Some(_)) => {
                        lt = true;
                        break;
                    }
                    (None, None) => break,
                    (Some(_), None) => {
                        lt = false;
                        break;
                    }
                    (Some(a), Some(b)) => {
                        if !crate::lisp::builtins::equal_values(i, a, b)? {
                            lt = value_lt_impl(i, a, b, depth + 1)?;
                            break;
                        }
                    }
                }
            }
            lt
        }
        (Value::Vec(_), _) => return Err(mism(i)),
        // Remaining same-type pairs (hash tables, buffers, subrs,
        // markers...) are unorderable → nil; mismatched types → error.
        _ => {
            if std::mem::discriminant(x) == std::mem::discriminant(y) {
                false
            } else {
                return Err(mism(i));
            }
        }
    })
}

pub(super) fn f_value_lt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (x, y) = (&a[0], &a[1]);
    Ok(Value::from_bool(value_lt_impl(i, x, y, 0)?))
}

fn f_seconds_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let us = lisp_time_to_us(i, &a[0])?;
    Ok(us_to_lisp_time(us))
}

fn f_time_since(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = lisp_time_to_us(i, &a[0])?;
    let now = lisp_time_to_us(i, &Value::Nil)?;
    // GNU's nil second arg defers to `current-time-list' (t => `list');
    // `list' yields the (HI LO US PS) form, otherwise (TICKS . HZ).
    if !current_time_list(i) {
        return Ok(Value::cons(
            Value::Int((now - from) * 1000),
            Value::Int(1_000_000_000),
        ));
    }
    Ok(us_to_lisp_time(now - from))
}

fn f_time_to_days(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let secs = lisp_time_seconds(i, &a[0])?;
    let tm = interp_local_tm(i, secs);
    // GNU time-date.el uses the local calendar date and truncating
    // division in its pseudo-Gregorian day-number formula.
    let previous_year = tm.tm_year as i128 + 1899;
    Ok(Value::Int(
        tm.tm_yday as i128 + 1 + 365 * previous_year + previous_year / 4 - previous_year / 100
            + previous_year / 400,
    ))
}

fn f_time_to_day_in_year(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let secs = lisp_time_seconds(i, &a[0])?;
    let tm = interp_local_tm(i, secs);
    Ok(Value::Int(tm.tm_yday as i128 + 1))
}

fn f_days_to_time(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let days = match &a[0] {
        Value::Int(n) => *n,
        Value::Float(f) => **f as i128,
        other => return Err(i.wrong_type_mut("numberp", other)),
    };
    // GNU returns the (HIGH LOW) seconds form, not a full time value.
    let secs = days * 86400;
    Ok(Value::list(vec![
        Value::Int(secs.div_euclid(65536)),
        Value::Int(secs.rem_euclid(65536)),
    ]))
}

fn f_date_leap_year_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let y = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    Ok(Value::from_bool(
        y % 4 == 0 && (y % 100 != 0 || y % 400 == 0),
    ))
}

fn f_version_list_not_zero(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: listp arg, first non-zero elt or 0.
    let items = match &a[0] {
        Value::Nil => Vec::new(),
        Value::Cons(_) => want_list(i, &a[0])?,
        other => return Err(i.wrong_type_mut("listp", other)),
    };
    Ok(items
        .into_iter()
        .find(|v| !matches!(v, Value::Int(0)))
        .unwrap_or(Value::Int(0)))
}

fn f_memory_use_counts(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let counts = crate::lisp::memory::counts();
    Ok(Value::list(
        counts.into_iter().map(|n| Value::Int(n as i128)).collect(),
    ))
}

fn f_group_gid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    unsafe extern "C" {
        fn getegid() -> u32;
    }
    Ok(Value::Int(unsafe { getegid() } as i128))
}

fn f_group_real_gid(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    unsafe extern "C" {
        fn getgid() -> u32;
    }
    Ok(Value::Int(unsafe { getgid() } as i128))
}

fn f_system_users(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
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

fn f_bufferpos_to_filepos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's Lisp (mule-util.el)
    // runs `coding-system-eol-type'/`coding-system-type'/`coding-system-base'
    // on CODING-SYSTEM before POSITION is coerced — each wants a symbol.
    if let Some(q) = a.get(2) {
        if !matches!(q, Value::Sym(_)) {
            return Err(i.wrong_type_mut("symbolp", q));
        }
    }
    if let Some(q) = a.get(1) {
        if !matches!(q, Value::Sym(_)) {
            return Err(i.wrong_type_mut("symbolp", q));
        }
    }
    let pos = match &a[0] {
        // GNU CHECK_NUMBER_COERCE_MARKER accepts only ints and markers —
        // floats land on the same integer-or-marker-p error.
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128,
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    // Unibyte buffers: file byte = (clamped position) - 1.
    let zv = i
        .current_buffer_ref()
        .map(|b| b.borrow().text_len())
        .unwrap_or(0) as i128
        + 1;
    Ok(Value::Int((pos.clamp(1, zv)) - 1))
}

fn f_filepos_to_bufferpos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match &a[0] {
        // GNU CHECK_NUMBER_COERCE_MARKER: marker → position; other
        // non-numbers signal number-or-marker-p.
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
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

fn f_set_buffer_multibyte(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_buffer_multibyte: forbidden on indirect buffers — their
    // multibyte flag belongs to the shared text's owner (base).
    if let Some(b) = i.current_buffer_ref() {
        if b.borrow().base_buffer.is_some() {
            return Err(i.error_q(
                "Cannot do `set-buffer-multibyte' on an indirect buffer",
                &[],
            ));
        }
    }
    // GNU Fset_buffer_multibyte flips the current buffer's
    // `enable-multibyte-characters' local and returns the flag.
    let sym = i.q("enable-multibyte-characters");
    i.set_symbol(sym, a[0].clone())?;
    if let Some(buffer) = i.current_buffer_ref() {
        buffer.borrow().text.set_multibyte(a[0].truthy());
    }
    Ok(a[0].clone())
}

fn f_window_with_parameter(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el:
    // `walk-window-tree' decodes FRAME (arg 2) via `frame-root-window' —
    // nil -> selected, non-frame -> `frame-live-p' wta.
    let _ = crate::editor::frame_live(i, &arg(&a, 2))?;
    // Windows carry no `window-side' etc. in this build → nil.
    Ok(Value::Nil)
}

fn f_message_box(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::builtins::evalfn::f_message(i, a)
}

fn f_secure_hash_algorithms(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let names = [
        "md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-224", "sha3-256", "sha3-384",
        "sha3-512",
    ];
    Ok(Value::list(
        names.iter().map(|n| Value::Sym(i.intern(n))).collect(),
    ))
}

fn f_primitive_function_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(matches!(a[0], Value::Subr(_))))
}

fn f_read_expression(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (read--expression PROMPT &optional INITIAL-CONTENTS) — read a
    // string from the minibuffer, then `read' one form from it.
    let prompt = match a.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => String::new(),
    };
    let input = if i.minibuf_reader.is_some() {
        // GNU read--expression → read-from-minibuffer with
        // `read-expression-history' as HISTVAR.
        let args = crate::lisp::eval::MinibufArgs {
            hist: Value::Sym(i.q("read-expression-history")),
            initial: a.get(1).cloned().unwrap_or(Value::Nil),
            ..Default::default()
        };
        Value::string(i.minibuf_read(&prompt, args)?)
    } else if i.noninteractive {
        // GNU `read_minibuf_noninteractive' reads a line from stdin.
        i.batch_read_line(&prompt)?
    } else {
        // GNU reads an answer from the minibuffer; in batch that is
        // stdin and signals end-of-file with this message.
        let eof = i.q("end-of-file");
        return Err(i.signal_data(eof, vec![Value::string("Error reading from stdin")]));
    };
    // GNU `string_to_object': parse the line; empty input has no
    // default here, so it signals `end-of-file'.
    i.string_to_object(&input, &Value::Nil)
}

fn f_one(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(1))
}

/// Parse a color to 16-bit (0-65535) RGB like GNU: "#rgb"/"#rrggbb"
/// strings, the 8 standard color names, or a (R G B) list.
pub(crate) fn parse_color_16(v: &Value) -> Option<(i64, i64, i64)> {
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
        // tty-defined colors: like tty_defined_color, a leading NUL
        // truncates the C view of the name to "" — the default
        // black is returned, never "Invalid color".
        if t.starts_with('\0') {
            return Some((0, 0, 0));
        }
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

fn f_color_distance(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Riemersma's "colour metric" on 16-bit components, as in GNU.
    // GNU checks FRAME with `frame-live-p' FIRST (Fcolor_distance calls
    // get_frame which errors on dead/non-frame args), then parses colors.
    if a.len() > 2 && !a[2].is_nil() {
        let live = match &a[2] {
            Value::Frame(f) => !f.borrow().dead,
            other => return Err(i.wrong_type_mut("frame-live-p", other)),
        };
        if !live {
            return Err(i.wrong_type_mut("frame-live-p", &a[2]));
        }
    }
    let (c1, c2) = match (parse_color_16(&a[0]), parse_color_16(&a[1])) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            // GNU reports the first INVALID color (COLOR1 before COLOR2).
            let bad = if parse_color_16(&a[0]).is_none() {
                &a[0]
            } else {
                &a[1]
            };
            let err = i.q("error");
            return Err(i.signal_data(err, vec![Value::string("Invalid color"), bad.clone()]));
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
            Value::Sym(i.q("mm-size")),
            Value::Nil,
            Value::Nil,
        ]),
        Value::list(vec![Value::Sym(i.q("frames")), fr]),
    ])
}

fn f_frame_monitor_attributes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(monitor_attributes(i))
}

fn f_display_monitor_attributes_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.get(0).cloned().unwrap_or(Value::Nil);
    let _ = decode_display(i, &v)?;
    Ok(Value::list(vec![monitor_attributes(i)]))
}

fn f_locale_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
        "days" => Ok(Value::vector(
            (0..7)
                .map(|d| lang(day1 + d).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )),
        "months" => Ok(Value::vector(
            (0..12)
                .map(|m| lang(mon1 + m).map(Value::string).unwrap_or(Value::Nil))
                .collect(),
        )),
        _ => Ok(Value::Nil),
    }
}

/// `frame-inner-width'/`frame-inner-height' — GNU's decode errors
/// plainly (`"X is not a live frame"') for non-frames, then the tty
/// text geometry is the frame's own width/height.
/// `frame-geometry' — GNU frame.el: `window-normalize-frame' gates
/// with the plain "X is not a live frame" error, then tty → nil.
fn f_frame_geometry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Nil)
}
fn f_frame_width_val(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Int(sel_frame_dims(i).0))
}
fn f_frame_height_val(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Ok(Value::Int(sel_frame_dims(i).1))
}

/// `frame-outer-width'/`frame-outer-height' — GNU consults
/// `frame-edges', which is nil on a tty batch frame; the arithmetic
/// then signals `wrong-type-argument number-or-marker-p nil'.
fn f_frame_outer_dim(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = crate::editor::frame_live_plain(i, &arg(&a, 0))?;
    Err(i.wrong_type_mut("number-or-marker-p", &Value::Nil))
}

fn f_x_parse_geometry(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_current_input_mode(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: (INTERRUPT FLOW META QUIT) = (t nil t 7).
    Ok(Value::list(vec![
        Value::t(),
        Value::Nil,
        Value::t(),
        Value::Int(7),
    ]))
}

fn f_current_bidi_paragraph_direction(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcurrent_bidi_paragraph_direction: nil →
    // current buffer; a non-buffer arg is `bufferp'.
    match a.first() {
        None | Some(Value::Nil) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("bufferp", other)),
    }
    Ok(Value::Sym(i.q("left-to-right")))
}

fn f_bidi_string_mark_left_to_right(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    // GNU's bidi.c: if STR contains a character of strong R or AL
    // bidi type, return STR followed by U+200E LEFT-TO-RIGHT MARK so
    // subsequent text resolves LTR; otherwise return STR unchanged.
    let rtl = s.chars().any(|ch| {
        let u = ch as u32;
        super::bidi_table::BIDI_CLASS_TABLE
            .binary_search_by(|&(lo, hi, _)| {
                if u < lo {
                    std::cmp::Ordering::Greater
                } else if u > hi {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .map(|idx| matches!(super::bidi_table::BIDI_CLASS_TABLE[idx].2, "R" | "AL"))
            .unwrap_or(false)
    });
    Ok(if rtl {
        let mut marked = s;
        marked.push('\u{200e}');
        Value::string(marked)
    } else {
        a[0].clone()
    })
}

fn f_read_non_nil_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fread_non_nil_coding_system (coding.c):
    // completing-read over `coding-system-alist' (REQUIRE-MATCH t,
    // `coding-system-history', no default) — retries while the answer is
    // empty.  Batch reads stdin; EOF propagates `end-of-file'.
    let cr = Value::Sym(i.q("completing-read"));
    let alist = {
        let id = i.q("coding-system-alist");
        if i.bound_p(id) {
            i.symbol_value(id)
        } else {
            Value::Nil
        }
    };
    let hist = Value::Sym(i.q("coding-system-history"));
    loop {
        let val = i.apply(
            &cr,
            vec![
                a[0].clone(),
                alist.clone(),
                Value::Nil,
                Value::t(),
                Value::Nil,
                hist.clone(),
                Value::Nil,
                Value::Nil,
            ],
        )?;
        match &val {
            Value::Str(s) if !s.borrow().is_empty() => {
                let name = s.borrow().clone();
                return Ok(Value::Sym(i.intern(&name)));
            }
            _ => {}
        }
    }
}

fn f_find_operation_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Ffind_operation_coding_system (coding.c): TARGET is
    // args[1 + target-idx] where `target-idx' is a plist property put on
    // the operation symbol at startup; the value comes from scanning the
    // operation's coding-system alist (file/process/network).
    if a.len() < 2 {
        return Err(i.error("Too few arguments"));
    }
    let Some(op_id) = i.sym_id(&a[0]) else {
        return Err(i.error("Invalid first argument"));
    };
    let tidx_sym = i.q("target-idx");
    let target_idx = match i.get_prop(op_id, tidx_sym) {
        Value::Int(n) if n >= 0 => n as usize,
        _ => return Err(i.error("Invalid first argument")),
    };
    if a.len() <= 1 + target_idx {
        return Err(i.error_q(
            "Too few arguments for operation `%s'",
            &[&i.symbol_name(op_id)],
        ));
    }
    let target = a[1 + target_idx].clone();
    let op_name = i.symbol_name(op_id).to_string();
    let ifc = op_name == "insert-file-contents";
    let ons = op_name == "open-network-stream";
    let target_ok = matches!(target, Value::Str(_))
        || (ifc
            && matches!(&target, Value::Cons(c)
            if matches!(&c.borrow().car, Value::Str(_))
                && matches!(&c.borrow().cdr, Value::Buffer(_))))
        || (ons
            && (matches!(target, Value::Int(_)) || matches!(target, Value::Sym(s) if s == sym::T)));
    if !target_ok {
        return Err(i.error_q(
            "Invalid argument %s of operation `%s'",
            &[&(target_idx + 1).to_string(), &op_name],
        ));
    }
    let target = if let Value::Cons(c) = target {
        c.borrow().car.clone()
    } else {
        target
    };
    let var_name = if ifc || op_name == "write-region" {
        "file-coding-system-alist"
    } else if ons {
        "network-coding-system-alist"
    } else {
        "process-coding-system-alist"
    };
    let chain = {
        let vs = i.intern(var_name);
        i.symbol_value(vs)
    };
    let mut cur = chain;
    while let Value::Cons(c) = cur.clone() {
        let b = c.borrow();
        cur = b.cdr.clone();
        let Value::Cons(elt) = &b.car else { continue };
        let (pat, val) = (elt.borrow().car.clone(), elt.borrow().cdr.clone());
        let hit = match (&pat, &target) {
            (Value::Str(pat), Value::Str(t)) => {
                // fast_string_match: case-sensitive; a bad regexp just
                // misses the element.
                let chars: Vec<char> = t.borrow().chars().collect();
                crate::lisp::regexp::compile_case(&pat.borrow(), false)
                    .ok()
                    .map(|re| {
                        crate::lisp::regexp::search_full(
                            &re,
                            &chars,
                            0,
                            &crate::editor::re_syntax(i),
                            &crate::editor::re_category(i),
                            crate::lisp::regexp::NO_POINT,
                            false,
                        )
                        .is_some()
                    })
                    .unwrap_or(false)
            }
            (Value::Int(p), Value::Int(t)) => p == t,
            _ => false,
        };
        if !hit {
            continue;
        }
        // val is a coding-system cons, a coding-system symbol, or a
        // function symbol called with the arg list.
        if matches!(val, Value::Cons(_)) {
            return Ok(val);
        }
        if i.sym_id(&val).is_none() {
            return Ok(Value::Nil);
        }
        if coding_known(i, &val).is_some() {
            return Ok(Value::cons(val.clone(), val));
        }
        let fid = i.sym_id(&val).unwrap();
        let f = i.symbol_function(fid);
        if matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            return Ok(Value::Nil);
        }
        let arglist = Value::list(a.to_vec());
        let val = i.apply(&f, vec![arglist])?;
        if matches!(val, Value::Cons(_)) {
            return Ok(val);
        }
        if coding_known(i, &val).is_some() {
            return Ok(Value::cons(val.clone(), val));
        }
        return Ok(Value::Nil);
    }
    Ok(Value::Nil)
}
fn f_lossage_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        if v.truthy() {
            match v {
                // GNU keyboard.c Flossage_size: FIXNATP required, then
                // MIN_NUM_RECENT_KEYS (100) .. MAX_NUM_RECENT_KEYS
                // (INT_MAX/EMACS_INT_WIDTH = 33554431 on 64-bit),
                // both reported via user-error.
                Value::Int(n) if *n >= 0 => {
                    if *n < 100 {
                        return Err(i.signal_data(
                            sym::USER_ERROR,
                            vec![Value::string("Value must be >= 100")],
                        ));
                    }
                    if *n > 33_554_431 {
                        return Err(i.signal_data(
                            sym::USER_ERROR,
                            vec![Value::string("Value must be <= 33554431")],
                        ));
                    }
                }
                _ => {
                    return Err(i.signal_data(
                        sym::USER_ERROR,
                        vec![Value::string("Value must be a positive integer")],
                    ));
                }
            }
        }
    }
    Ok(Value::Int(300))
}

fn f_mouse_position_root(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::cons(Value::Int(0), Value::Int(0)))
}

fn f_define_coding_system_alias(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let alias_id = want_sym(i, &a[0])?;
    let target_id = want_sym(i, &a[1])?;
    if coding_known(i, &a[1]).is_none() {
        // GNU signals `coding-system-error' with the CODING-SYSTEM arg
        // only — not (ALIAS CODING-SYSTEM).
        let cs_err = i.q("coding-system-error");
        return Err(i.signal_data(cs_err, vec![a[1].clone()]));
    }
    // GNU: `define-coding-system-alias' adds ALIAS to
    // `coding-system-list' (sorted via `add-to-coding-system-list')
    // and resolves to CODING-SYSTEM's base for `coding-system-base'.
    let alias = i.symbol_name(alias_id);
    // GNU aliases capture the target's coding object, not a live symbol
    // indirection. Freeze its canonical name (including any EOL suffix)
    // before overwriting ALIAS, so retargeting another alias cannot change
    // existing aliases and defining an alias to itself cannot create a cycle.
    let target = coding_alias_group(i, &a[1])
        .and_then(|group| group.into_iter().next())
        .unwrap_or_else(|| i.symbol_name(target_id));
    if alias == target {
        return add_to_coding_system_list(i, &alias);
    }
    if let Some((_, previous)) = i.extra_coding_aliases.iter_mut().find(|(x, _)| *x == alias) {
        *previous = target;
    } else {
        i.extra_coding_aliases.push((alias.clone(), target));
    }
    add_to_coding_system_list(i, &alias)
}

fn f_thread_buffer_disposition(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        Some(Value::Thread(_)) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("threadp", other.unwrap_or(&Value::Nil))),
    }
}

fn f_thread_set_buffer_disposition(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

/// The mutex, condition variable or thread this thread is waiting for.
fn f_thread_blocker(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let t = want_thread(i, &a[0])?;
    Ok(t.borrow().blocker.clone().unwrap_or(Value::Nil))
}

/// `optimize-char-table' — collapse uniform sub-tables bottom-up
/// (TEST defaults to `equal'; `eq' uses identity).
fn f_optimize_char_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if !is_char_table(i, &a[0]) {
        return Err(i.wrong_type_mut("char-table-p", &a[0]));
    }
    let test = arg(&a, 1);
    ct_optimize(i, &a[0].clone(), &test)?;
    Ok(Value::Nil)
}

/// `glyph-char`/`glyph-face` — GNU Lisp accessors over glyph codes:
/// a plain char maps to itself / no face.
fn f_glyph_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        v @ Value::Int(_) => Ok(v.clone()),
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

fn f_glyph_face(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(_) => Ok(Value::Nil),
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        other => Err(i.wrong_type_mut("numberp", other)),
    }
}

/// `make-glyph-code` — GNU returns CHAR alone for no face, else a
/// (CHAR . FACE-ID) cons where FACE-ID is the face's index in the
/// face table (GNU's `lookup_named_face' result).
fn f_make_glyph_code(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(_) => {}
        other => return Err(i.wrong_type_mut("characterp", other)),
    }
    match a.get(1) {
        None | Some(Value::Nil) => Ok(a[0].clone()),
        Some(face) => {
            let name = match face {
                Value::Sym(s) => i.symbol_name(*s).to_string(),
                _ => return Err(i.wrong_type_mut("symbolp", face)),
            };
            match i.face_table.iter().position(|(n, _)| *n == name) {
                Some(id) => Ok(Value::cons(a[0].clone(), Value::Int(id as i128))),
                // Unknown face: GNU falls back to the bare char.
                None => Ok(a[0].clone()),
            }
        }
    }
}

/// `font-spec` — build a font-spec record `#s(font-spec PROPS)'.
/// GNU validates the restricted properties: :weight/:slant/:width
/// must name a member of their enum, :size/:dpi a number.
fn f_font_spec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const WEIGHTS: &[&str] = &[
        "thin",
        "ultra-light",
        "extra-light",
        "light",
        "semi-light",
        "book",
        "normal",
        "medium",
        "semi-bold",
        "demi-bold",
        "bold",
        "extra-bold",
        "ultra-bold",
    ];
    const SLANTS: &[&str] = &[
        "normal",
        "roman",
        "italic",
        "oblique",
        "reverse-italic",
        "reverse-oblique",
    ];
    const WIDTHS: &[&str] = &[
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "narrow",
        "normal",
        "regular",
        "medium",
        "semi-expanded",
        "expanded",
        "extra-expanded",
        "ultra-expanded",
    ];
    let bad = |i: &mut Interp, p: &Value, v: &Value| -> Flow {
        let err = i.q("error");
        let msg = Value::string("invalid font property");
        let datum = Value::cons(p.clone(), v.clone());
        i.signal_data(err, vec![msg, datum])
    };
    let mut idx = 0;
    while idx < a.len() {
        let prop = &a[idx];
        // GNU: CHECK_SYMBOL(key) — non-symbol keys signal `symbolp'
        // (the key itself is the datum), checked before the missing-value
        // arity error for the LAST key.
        let Some(id) = i.sym_id(prop) else {
            return Err(i.wrong_type_mut("symbolp", prop));
        };
        if idx + 1 >= a.len() {
            return Err(i.error(&format!("No value for key ‘{}’", i.symbol_name(id))));
        }
        let val = &a[idx + 1];
        let name = i.symbol_name(id).to_string();
        let invalid = match name.as_str() {
            ":weight" => {
                !i.sym_id(val)
                    .map(|s| WEIGHTS.contains(&i.symbol_name(s).as_str()))
                    .unwrap_or(false)
                    && !matches!(val, Value::Int(_))
            }
            ":slant" => !i
                .sym_id(val)
                .map(|s| SLANTS.contains(&i.symbol_name(s).as_str()))
                .unwrap_or(false),
            ":width" => {
                !i.sym_id(val)
                    .map(|s| WIDTHS.contains(&i.symbol_name(s).as_str()))
                    .unwrap_or(false)
                    && !matches!(val, Value::Int(_))
            }
            ":size" | ":dpi" => !matches!(val, Value::Int(_) | Value::Float(_)),
            _ => false,
        };
        if invalid {
            return Err(bad(i, prop, val));
        }
        idx += 2;
    }
    Ok(Value::record(vec![
        Value::Sym(i.q("font-spec")),
        Value::list(a.as_slice().to_vec()),
    ]))
}

/// `put-image` — GNU requires an image spec (list headed `image') and
/// returns a fresh overlay at POS.
fn f_put_image(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let image_sym = i.q("image");
    let ok = match &a[0] {
        Value::Cons(c) => i.sym_is(&c.borrow().car, image_sym),
        _ => false,
    };
    if !ok {
        let shown = i.prin1_to_string(&a[0]);
        return Err(i.error(format!("Not an image: {shown}")));
    }
    let pos = a[1].clone();
    crate::editor::f_make_overlay(i, vec![pos.clone(), pos, Value::Nil].into())
}

/// `set-file-acl` — best-effort like GNU: validate strings, no ACL
/// support → nil.
fn f_set_file_acl(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_string(i, &a[0])?;
    let _ = want_string(i, &a[1])?;
    Ok(Value::Nil)
}

/// `mapbacktrace` — GNU calls FUNCTION with (EVALD FUNC ARGS FLAGS)
/// for each live frame, innermost first, then returns nil.
fn f_mapbacktrace(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fun = a[0].clone();
    let frames: Vec<_> = i.backtrace_snapshot().into_iter().rev().collect();
    // GNU mapbacktrace: when FUNCTION is non-nil, frames below its
    // nearest activation are discarded — the BASE frame itself is the
    // first one delivered.  If BASE never appears on the stack, nothing
    // is mapped at all.
    let start = match a.get(1) {
        Some(v) if v.truthy() => {
            // GNU `get_backtrace_starting_at' compares BASE through
            // `Findirect_function' on BOTH sides — the stack holds the
            // resolved function value for direct calls but the symbol
            // for `apply'-driven ones.
            let base_fn = i.indirect_function_value(v);
            match frames.iter().position(|frame| {
                crate::lisp::builtins::eq_values(&i.indirect_function_value(&frame.function), &base_fn)
            }) {
                Some(p) => p,
                None => return Ok(Value::Nil),
            }
        }
        _ => 0,
    };
    for frame in &frames[start..] {
        i.apply(
            &fun,
            vec![
                Value::from_bool(frame.evaluated),
                frame.function.clone(),
                frame.arguments.clone(),
                Value::Nil,
            ],
        )?;
    }
    Ok(Value::Nil)
}

fn f_process_arg_err(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let v = a.first().cloned().unwrap_or(Value::Nil);
    Err(i.wrong_type_mut("processp", &v))
}

fn f_internal_handle_focus_in(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_set_font_selection_order(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU requires a proper list of font-driver symbols.
    // GNU: CHECK_LIST first — non-lists signal `listp'; then each member
    // must be a font-driver symbol else "Invalid font sort order".
    if !matches!(a[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
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

fn f_clear_image_cache(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU image.c
    // Fclear_image_cache: non-nil ANIMATION-FILTER is CHECK_CONS'd FIRST
    // (wta `consp') and returns nil early; only nil/frame FILTER reaches
    // decode_window_system_frame → "Window system frame should be used".
    // A non-nil non-frame FILTER (e.g. a filename string) just clears
    // matching cache entries — no error even without a window system.
    if arg(&a, 1).truthy() {
        if !matches!(arg(&a, 1), Value::Cons(_)) {
            return Err(i.wrong_type_mut("consp", &arg(&a, 1)));
        }
        return Ok(Value::Nil);
    }
    match &arg(&a, 0) {
        Value::Nil | Value::Frame(_) => Err(i.error("Window system frame should be used")),
        _ => Ok(Value::Nil),
    }
}

fn f_query_fontset(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: fontsets need a window system.
    Err(i.error("Window system is not in use or not initialized"))
}

fn f_close_font(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // No font objects exist; GNU type-checks arg0 as font-object.
    Err(i.wrong_type_mut("font-object", &a[0]))
}

fn f_define_fringe_bitmap(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU fringe.c: CHECK_SYMBOL(BITMAP) then CHECK_ARRAY(BITS,
    // Qstringp) — a string bits arg is allowed, nil is not.
    match &a[0] {
        Value::Sym(_) => {}
        other => return Err(i.wrong_type_mut("symbolp", other)),
    }
    match &a[1] {
        Value::Str(_) | Value::Vec(_) => {}
        other => return Err(i.wrong_type_mut("arrayp", other)),
    }
    Ok(a[0].clone())
}

fn f_re_describe_compiled(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(_) | Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_move_file_to_trash(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU nsfns.m Fsystem_move_file_to_trash: a non-symlink directory
    // switches the operation to delete-directory with the directory-file
    // name, then FILENAME is expand-file-name'd *fully qualified* before
    // the trash attempt; failure signals report_file_error
    // "Removing old name" with the errno-picked symbol.
    let raw = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut name = raw.clone();
    let meta = std::fs::metadata(&crate::editor::expand_file_name_str(i, &name)).ok();
    let is_link = std::fs::symlink_metadata(&crate::editor::expand_file_name_str(i, &name))
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) && !is_link {
        // Qdelete_directory path: directory_file_name strips the slash.
        while name.len() > 1 && name.ends_with('/') {
            name.pop();
        }
    }
    let path = crate::editor::expand_file_name_str(i, &name);
    let src = std::path::Path::new(&path);
    // GNU's NS trash call fails on a missing path with ENOENT →
    // (file-missing "Removing old name" strerror ABSNAME).
    if let Err(e) = std::fs::symlink_metadata(src) {
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path)],
            &e,
        ));
    }
    // Moving $HOME or / into Trash via rename would succeed where GNU's
    // NS trash refuses with EPERM — and would be destructive.  Refuse the
    // same way: file-error "Removing old name" EPERM ABSNAME.
    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());
    if path == "/" || (!home.is_empty() && path == home) {
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path)],
            &std::io::Error::from_raw_os_error(libc::EPERM),
        ));
    }
    // freedesktop trash: ~/.local/share/Trash/{files,info}
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
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path.clone())],
            &e,
        ));
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
/// even type-checking ARG — but only when the selected window shows
/// the current buffer; a stray buffer context signals "called from
/// unrelated buffer" (GNU Lisp signal text, isearch.el relies on it).
fn f_move_to_window_line(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let w = sel_window(i);
    let wb_buf = w.and_then(|w| w.borrow().buffer.into());
    let cur_id = crate::editor::cur(i).borrow().id;
    match wb_buf {
        Some(id) if id == cur_id => Ok(Value::Int(0)),
        _ => Err(i.error("move-to-window-line called from unrelated buffer")),
    }
}

/// `network-lookup-address-info` — real getaddrinfo; GNU shape:
/// IPv4 → [A B C D 0], IPv6 → [S0..S7 0]; nil on lookup failure.
fn f_network_lookup_address_info(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let host = want_string(i, &a[0])?;
    let mut want4 = true;
    let mut want6 = true;
    if let Some(fam) = a.get(1) {
        if !fam.is_nil() {
            match i
                .sym_id(fam)
                .map(|s| i.symbol_name(s).to_string())
                .as_deref()
            {
                Some("ipv4") => want6 = false,
                Some("ipv6") => want4 = false,
                _ => {
                    let e = i.q("error");
                    return Err(i.signal_data(e, vec![Value::string("Unsupported family")]));
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
                out.push(Value::vector(vec![
                    Value::Int(o[0] as i128),
                    Value::Int(o[1] as i128),
                    Value::Int(o[2] as i128),
                    Value::Int(o[3] as i128),
                    Value::Int(0),
                ]));
            }
            std::net::SocketAddr::V6(v6) if want6 => {
                let mut elts: Vec<Value> = v6
                    .ip()
                    .segments()
                    .iter()
                    .map(|s| Value::Int(*s as i128))
                    .collect();
                elts.push(Value::Int(0));
                out.push(Value::vector(elts));
            }
            _ => {}
        }
    }
    Ok(Value::list(out))
}

/// `color-values-from-color-spec` — parse #RGB/#RRGGBB/#RRRGGGBBB/
/// #RRRRGGGGBBBB into 16-bit channel values; named colors need a
/// display (nil in batch), non-strings get stringp.
fn f_color_values_from_color_spec(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let hex = s.strip_prefix('#').unwrap_or("");
    if !s.starts_with('#')
        || ![3, 4, 6, 9, 12].contains(&hex.len())
        || !hex.chars().all(|c| c.is_ascii_hexdigit())
    {
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
fn f_file_selinux_context(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_string(i, &a[0])?;
    Ok(Value::list(vec![
        Value::Nil,
        Value::Nil,
        Value::Nil,
        Value::Nil,
    ]))
}

/// GNU `set-file-selinux-context': CHECK_STRING on FILENAME (a bare
/// `stringp' — GNU's Fexpand_file_name path isn't reached on this
/// no-libselinux build, so "~" isn't expanded and CONTEXT is never
/// type-checked), then nil.
fn f_set_file_selinux_context(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_string(i, &a[0])?;
    Ok(Value::Nil)
}

/// GNU's FACTOR must be a nonnegative fixnum. Zero never collects.
fn f_garbage_collect_maybe(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let factor = match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= crate::lisp::value::FIXNUM_MAX => *n as u64,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    if factor == 0 {
        return Ok(Value::Nil);
    }
    let threshold = i.q("gc-cons-threshold");
    let mut threshold = match i.symbol_value(threshold) {
        Value::Int(n) => n.max(0) as u64,
        _ => 800_000,
    };
    let percentage = i.q("gc-cons-percentage");
    if let Value::Float(percentage) = i.symbol_value(percentage) {
        if *percentage > 0.0 {
            threshold = threshold
                .max((crate::lisp::memory::heap_stats().bytes as f64 * *percentage) as u64);
        }
    }
    if crate::lisp::memory::allocated_since_gc() > threshold / factor {
        return Ok(if crate::lisp::memory::collect_interp(i) {
            Value::t()
        } else {
            Value::Nil
        });
    }
    Ok(Value::Nil)
}

fn f_gc_heapsize(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(crate::lisp::memory::report(i))
}

/// `make-closure` — GNU's `Fmake_closure': splice the runtime
/// ENV values over the `V0..Vn' placeholder slots at the head of the
/// prototype's constant vector (see `byte-compile-make-closure'),
/// producing a real byte-code closure.
fn f_make_closure(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Lambda(l) = &a[0] else {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    };
    let Some(items) = &l.bc_items else {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    };
    let mut new_items = items.borrow().clone();
    let env = &a[1..];
    let Value::Vec(consts) = &new_items[2] else {
        return Err(i.wrong_type_mut("vectorp", &new_items[2]));
    };
    let old = consts.borrow();
    let mut spliced: Vec<Value> = env.to_vec();
    spliced.extend_from_slice(&old[env.len().min(old.len())..]);
    drop(old);
    new_items[2] = Value::vector(spliced);
    Ok(Value::Lambda(Rc::new(Lambda {
        is_macro: l.is_macro,
        required: l.required.clone(),
        optional: l.optional.clone(),
        rest: l.rest,
        body: l.body.clone(),
        env: l.env.clone(),
        doc: l.doc.clone(),
        interactive: l.interactive.clone(),
        name: l.name.clone(),
        bad_arglist: l.bad_arglist,
        arglist: l.arglist.clone(),
        plain: l.plain,
        dumped_doc: l.dumped_doc,
        compiled: l.compiled,
        advice_link: l.advice_link.clone(),
        bc_items: Some(crate::lisp::Value::vector_ref(new_items)),
        bc_closure: true,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: l.doc_value.clone(),
        env_value: l.env_value.clone(),
    }))
    .tracked())
}

/// `bidi-find-overridden-directionality` (FROM TO OBJECT &optional BASE-DIR):
/// GNU decodes OBJECT first — buffer/window accepted, anything else
/// non-nil is CHECK_STRING — then validate_interval_range/s u b a r r a y
/// bounds-checks FROM/TO as positions.  No bidi → nil.
fn f_bidi_find_overridden(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let object_is_str = match &a[2] {
        Value::Nil | Value::Buffer(_) | Value::Window(_) => false,
        Value::Str(_) => true,
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if object_is_str {
        let Value::Str(s) = &a[2] else { unreachable!() };
        // GNU bidi.c: unibyte strings are always strong LTR — nil is
        // returned BEFORE validate_subarray, so bad FROM/TO never
        // get type-checked on a unibyte object.
        if !i.is_multibyte_str(s) {
            return Ok(Value::Nil);
        }
        // validate_subarray: FROM/TO 0-based positions in the string.
        let len = s.borrow().chars().count() as i128;
        let from = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
        let to = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
        if from < 0 || to < from || to > len {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        return Ok(Value::Nil);
    }
    // Buffer path: FROM/TO are 1-based integer-or-marker positions.
    let from = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let to = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
    let (begv, zv) = {
        let b = match &a[2] {
            Value::Buffer(buf) => buf.clone(),
            _ => crate::buffer::primitives::cur(i),
        };
        let bb = b.borrow();
        (bb.begv as i128, bb.zv as i128)
    };
    if from < begv + 1 || from > zv + 1 || to < from || to > zv + 1 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    Ok(Value::Nil)
}

/// `bidi-resolved-levels` — GNU fixnump-checks its arg; nil without bidi.
fn f_bidi_resolved_levels(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        if !v.is_nil() && !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("fixnump", v));
        }
    }
    Ok(Value::Nil)
}

/// `composition-sort-rules` — GNU listp-checks RULES; nil.
/// `composition-get-gstring' — GNU composite.c: FONT-OBJECT decodes
/// a terminal first (`terminal-live-p' on non-nil non-font-object
/// values), then FROM/TO must be integer-or-marker; the string path
/// additionally bounds-checks via args-out-of-range.  Returns a
/// shaped lgstring vector whose lglyph arrays carry (FROM TO CHAR
/// CODE ...) tuples — enough structure for `auto-compose-chars' to
/// call compose-gstring-for-terminal without crashing.
fn f_composition_get_gstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes FONT-OBJECT's terminal before FROM/TO.
    if !matches!(a[2], Value::Nil) {
        let ok = match &a[2] {
            Value::Record(r) | Value::Vec(r) => {
                let r = r.borrow();
                r.first()
                    .and_then(|v| match v {
                        Value::Sym(s) => Some(i.symbol_name(*s)),
                        _ => None,
                    })
                    .map(|n| n == "font-object")
                    .unwrap_or(false)
            }
            _ => false,
        };
        if !ok {
            return Err(i.wrong_type_mut("terminal-live-p", &a[2]));
        }
    }
    let want_int = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let from = want_int(i, &a[0])?;
    let to = want_int(i, &a[1])?;
    if to < from {
        return Err(crate::buffer::primitives::err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    if to == from {
        return Err(crate::buffer::primitives::err_sym(
            i,
            "error",
            vec![Value::string("Attempt to shape zero-length text")],
        ));
    }
    // String path: bounds against the string length; buffer path:
    // against BEGV..ZV of the current buffer.
    let chars: Vec<u32> = match &a[3] {
        Value::Str(s) => {
            let v: Vec<u32> = s.borrow().chars().map(|c| c as u32).collect();
            if from < 0 || to as usize > v.len() || from as usize > v.len() {
                return Err(crate::buffer::primitives::err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[3].clone(), Value::Int(from), Value::Int(to)],
                ));
            }
            v[from.max(0) as usize..(to.max(0) as usize).min(v.len())].to_vec()
        }
        _ => {
            let Some(b) = i.current_buffer_ref() else {
                return Err(crate::buffer::primitives::err_sym(i, "error", vec![]));
            };
            let bb = b.borrow();
            let text = bb.text.text();
            let zv = bb.text_len();
            if from < 1 || to > zv as i128 {
                return Err(crate::buffer::primitives::err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[0].clone(), a[1].clone()],
                ));
            }
            text.chars()
                .skip((from - 1) as usize)
                .take((to - from) as usize)
                .map(|c| c as u32)
                .collect()
        }
    };
    // GNU lgstring layout: header [coding-system char...] then nil,
    // then one lglyph per character: [from to char code width lborder
    // rborder ascent descent adjustment ...] = [0 0 ch ch 1 0 1 1 0
    // nil] for unshaped glyphs.
    let mut lglyphs = Vec::new();
    for (_idx, ch) in chars.iter().enumerate() {
        lglyphs.push(Value::vector(vec![
            Value::Int(0),
            Value::Int(0),
            Value::Int(*ch as i128),
            Value::Int(*ch as i128),
            Value::Int(1),
            Value::Int(0),
            Value::Int(1),
            Value::Int(1),
            Value::Int(0),
            Value::Nil,
        ]));
    }
    let utf8_unix = i.q("utf-8-unix");
    let cs = Value::Sym(utf8_unix);
    let mut hdr = vec![cs];
    for ch in &chars {
        hdr.push(Value::Int(*ch as i128));
    }
    let mut v = vec![Value::vector(hdr), Value::Nil];
    v.extend(lglyphs);
    while v.len() < 10 {
        v.push(Value::Nil);
    }
    Ok(Value::vector(v))
}

fn f_composition_sort_rules(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_list(i, &a[0])?;
    Ok(Value::Nil)
}

/// `find-composition-internal` — GNU decodes POS/LIMIT with
/// integer_or_marker checks. Static string properties are available without
/// a display composition cache; automatic/detailed compositions still need it.
pub(crate) fn f_find_composition_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (POSITION LIMIT STRING DETAIL-P)
    // GNU Ffind_composition_internal: POS via fix_position (markers
    // coerce, detached → "Marker does not point anywhere"), LIMIT
    // nil → -1 else fix_position clipped to ZV, STRING non-nil must be
    // a string and POS must lie in 0..SCHARS — else
    // `args-out-of-range (STRING POS)'; a nil STRING bounds POS to
    // BEGV..ZV with `args-out-of-range (BUFFER POS)'.
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let limit = match a.get(1) {
        Some(lim) if lim.truthy() => crate::lisp::builtins::want_int_or_mark(i, lim)?,
        _ => -1,
    };
    match a.get(2) {
        Some(Value::Str(s)) => {
            let n = s.borrow().chars().count() as i128;
            if !(0..=n).contains(&pos) {
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[2].clone(), a[0].clone()],
                ));
            }
        }
        Some(other) if other.truthy() => {
            return Err(i.wrong_type_mut("stringp", other));
        }
        _ => {
            let bb = crate::buffer::primitives::cur(i);
            let bx = bb.borrow();
            let lo = bx.begv as i128 + 1;
            let hi = bx.zv as i128 + 1;
            if !(lo..=hi).contains(&pos) {
                let buf = Value::Buffer(crate::buffer::primitives::cur(i));
                return Err(err_sym(i, "args-out-of-range", vec![buf, a[0].clone()]));
            }
        }
    }
    if let Some(Value::Str(src)) = a.get(2) {
        if !arg(&a, 3).truthy() {
            let composition = Value::Sym(i.q("composition"));
            let intervals = i.str_props(src).to_vec();
            let mut ranges: Vec<(usize, usize, Value)> = Vec::new();
            for (from, to, _) in intervals {
                let value = crate::buffer::primitives::f_get_text_property(
                    i,
                    vec![Value::Int(from as i128), composition.clone(), a[2].clone()].into(),
                )?;
                if value.is_nil() {
                    continue;
                }
                if let Some((_, end, previous)) = ranges.last_mut() {
                    if *end == from && super::eq_values(previous, &value) {
                        *end = to;
                        continue;
                    }
                }
                ranges.push((from, to, value));
            }
            let found = ranges
                .iter()
                .find(|(from, to, _)| *from as i128 <= pos && pos < *to as i128)
                .or_else(|| {
                    if limit > pos {
                        ranges
                            .iter()
                            .find(|(from, _, _)| pos < *from as i128 && (*from as i128) < limit)
                    } else if limit >= 0 && limit < pos {
                        ranges
                            .iter()
                            .rev()
                            .find(|(_, to, _)| limit < *to as i128 && (*to as i128) <= pos)
                    } else {
                        None
                    }
                });
            if let Some((from, to, property)) = found {
                // GNU composition_valid_p accepts an unregistered
                // ((LENGTH . COMPONENTS) . MOD-FUNC) with matching length.
                let valid = if let Value::Cons(prop) = property {
                    if let Value::Cons(head) = &prop.borrow().car {
                        let head = head.borrow();
                        head.car.int() == Some((to - from) as i128)
                            && matches!(
                                head.cdr,
                                Value::Nil
                                    | Value::Int(_)
                                    | Value::Str(_)
                                    | Value::Vec(_)
                                    | Value::Cons(_)
                            )
                    } else {
                        false
                    }
                } else {
                    false
                };
                return Ok(Value::list(vec![
                    Value::Int(*from as i128),
                    Value::Int(*to as i128),
                    Value::from_bool(valid),
                ]));
            }
        }
    }
    Ok(Value::Nil)
}

/// `remember-mouse-glyph` — GNU checks arg 0: nil selects the current
/// frame then fails the window-system check; a live frame → nil.
fn f_remember_mouse_glyph(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => Err(i.error("Window system frame should be used")),
        Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}

/// `set-terminal-coding-system-internal` — GNU decodes TERMINAL
/// (arg 2) with terminal-live-p FIRST, then CHECK_SYMBOLs CODING,
/// then requires a known coding system (coding-system-error).
fn f_set_terminal_coding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 1) {
        Value::Nil | Value::Frame(_) => {}
        w if crate::editor::is_terminal(i, &w) => {}
        other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    if !a[0].is_nil() && coding_known(i, &a[0]).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    i.terminal_coding = a[0].clone();
    Ok(Value::Nil)
}
/// `set-safe-terminal-coding-system-internal` — GNU CHECK_SYMBOLs its
/// CODING-SYSTEM arg; the safe-coding result is a no-op in batch.
/// CODING-SYSTEM arg; the safe-coding result is a no-op in batch.
fn f_set_safe_terminal_coding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_sym(i, &a[0])?;
    Ok(Value::Nil)
}

/// `module-load` — no dynamic-module support; GNU signals error with
/// the dlopen message as data.
fn f_module_load(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = want_string(i, &a[0])?;
    let e = i.q("error");
    Err(i.signal_data(
        e,
        vec![
            Value::string(f.clone()),
            Value::string(format!("dlopen({}): module support not in this build", f)),
        ],
    ))
}

/// `backtrace--frames-from-thread` — threadp-checks its arg; we keep
/// no suspended frames, so nil for a real thread.
fn f_backtrace_frames_from_thread(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_thread(i, &a[0])?;
    Ok(Value::Nil)
}

/// `backtrace--locals` — wholenump-checks arg 0; nil (no frame info).
fn f_backtrace_locals(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(n) if *n >= 0 => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("wholenump", other)),
    }
}

/// `backtrace-eval' — GNU checks NFRAMES (arg 1) with wholenump and
/// THREAD (arg 2) with threadp, then answers nil when the frame is
/// out of range (we keep no suspended frames at all).
fn f_backtrace_eval(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[1] {
        Value::Int(n) if *n >= 0 => {}
        other => return Err(i.wrong_type_mut("wholenump", other)),
    }
    if let Some(t) = a.get(2) {
        if !t.is_nil() {
            want_thread(i, t)?;
        }
    }
    Ok(Value::Nil)
}

/// `backtrace-frame--internal` — call FUNCTION on the stack frame
/// NFRAMES past BASE's nearest activation, returning its result; nil
/// when no such frame exists.  GNU eval.c `get_backtrace_starting_at':
/// BASE may be `(OFFSET . FUNCTION)', and FUNCTION is compared after
/// `indirect-function' on both sides.  remacs's `lisp_stack' holds
/// `(func, args)` innermost-last; GNU's `backtrace_top' sees the
/// innermost live frame (this subr itself), and the BASE scan skips
/// through the `backtrace-frame' Lisp frame.
fn f_backtrace_debug(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU backtrace.c Fbacktrace_debug: CHECK_FIXNUM then
    // CHECK_WHOLENUMP on LEVEL before debugger dispatch; batch mode
    // no-ops to t.
    match &a[0] {
        Value::Int(n) if *n >= 0 => Ok(Value::from_bool(true)),
        Value::Int(_) => Err(i.wrong_type_mut("wholenump", &a[0])),
        other => Err(i.wrong_type_mut("fixnump", other)),
    }
}

fn f_backtrace_frame_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fun = a[0].clone();
    let nframes = match &a[1] {
        Value::Int(n) if *n >= 0 => *n as usize,
        other => return Err(i.wrong_type_mut("wholenump", other)),
    };
    let mut base = a[2].clone();
    let mut offset: usize = 0;
    if let Value::Cons(c) = &base {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Int(n) = car {
            offset = n.max(0) as usize;
            base = cdr;
        }
    }
    // Walk innermost→outermost over the live call stack.
    let stack: Vec<_> = i.backtrace_snapshot().into_iter().rev().collect();
    let mut idx = 0usize;
    if !base.is_nil() {
        let base_fn = i.indirect_function_value(&base);
        let mut found = None;
        for (k, frame) in stack.iter().enumerate() {
            let ffn = i.indirect_function_value(&frame.function);
            if crate::lisp::builtins::eq_values(&ffn, &base_fn) {
                found = Some(k);
                break;
            }
        }
        match found {
            Some(k) => idx = k + offset,
            None => return Ok(Value::Nil),
        }
    }
    idx += nframes;
    let Some(frame) = stack.get(idx) else {
        return Ok(Value::Nil);
    };
    let frame = frame.clone();
    i.apply(
        &fun,
        vec![
            Value::from_bool(frame.evaluated),
            frame.function,
            frame.arguments,
            Value::Nil,
        ],
    )
}

/// `completion--flex-cost-gotoh` — GNU's affine-gap flex cost:
/// (COST POS1 POS2 ...) or nil when NEEDLE isn't a subsequence of
/// STRING. Empirical model: 5 if the match doesn't start at 0, plus
/// 9 + gap-len per interior gap; trailing text is free.
fn f_flex_cost_gotoh(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

fn f_profiler_cpu_running_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(if i.cpu_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}

/// `check-coding-systems-region` — GNU Fcheck_coding_systems_region:
/// START may itself be a string (checked directly → nil); otherwise
/// START/END go through CHECK_NUMBER_COERCE_MARKER, so floats and
/// other non-integers signal integer-or-marker-p.  Valid integer
/// positions are range-checked against BEGV..ZV.
fn f_check_coding_systems_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if matches!(a[0], Value::Str(_)) {
        return Ok(Value::Nil);
    }
    let coerce = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::BigInt(_) => crate::lisp::builtins::want_int(i, v),
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let s = coerce(i, &a[0])?;
    let e = coerce(i, &a[1])?;
    let (lo, hi) = i
        .current_buffer_ref()
        .map(|b| {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.zv as i128 + 1)
        })
        .unwrap_or((1, 1));
    if s < lo || e > hi || s > e {
        let sym = i.q("args-out-of-range");
        return Err(i.signal_data(sym, vec![a[0].clone(), a[1].clone()]));
    }
    Ok(Value::Nil)
}

/// `window-child' — first child of an internal window; nil on leaves.
fn f_window_child(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil => {
            let w = sel_window(i).ok_or_else(|| i.error("No window"))?;
            Ok(w.borrow()
                .children
                .first()
                .map(|k| Value::Window(k.clone()))
                .unwrap_or(Value::Nil))
        }
        Value::Window(w) => Ok(w
            .borrow()
            .children
            .first()
            .map(|k| Value::Window(k.clone()))
            .unwrap_or(Value::Nil)),
        ref other => Err(i.wrong_type_mut("window-valid-p", other)),
    }
}

/// `window-child-count' — GNU window.el: `(and (windowp window)
/// (window-child window))' walk — non-window args yield 0, not an
/// error.  Children only exist on internal windows.
fn f_window_child_count(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Window(w) => Ok(Value::Int(w.borrow().children.len() as i128)),
        _ => Ok(Value::Int(0)),
    }
}

/// `window-combined-p` — WINDOW is part of a combination in direction
/// HORIZONTAL (nil = vertical): returns the parent's first child (the
/// top/left window of the combination), which for the first child is
/// WINDOW itself; nil when the parent splits the other axis.
/// GNU signals a plain `error' for non-windows.
fn f_window_combined_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let w = match arg(&a, 0) {
        Value::Nil => sel_window(i).ok_or_else(|| i.error("No window"))?,
        Value::Window(w) if !w.borrow().dead => w.clone(),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a valid window")));
        }
    };
    let horiz = arg(&a, 1).truthy();
    let Some(p) = w_parent(&w) else {
        return Ok(Value::Nil);
    };
    let pb = p.borrow();
    if pb.horiz != Some(horiz) {
        return Ok(Value::Nil);
    }
    Ok(pb
        .children
        .first()
        .map(|k| Value::Window(k.clone()))
        .unwrap_or(Value::Nil))
}

/// `window-main-window` — GNU returns the frame's root window object
/// (a leaf while unsplit, the internal vc/hc window after splits).
fn f_window_main_window(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let f = match arg(&a, 0) {
        Value::Nil => sel_frame(i),
        Value::Frame(fr) => Some(fr),
        other => {
            let shown = i.princ_to_string(&other);
            return Err(i.error(format!("{shown} is not a live frame")));
        }
    };
    let Some(f) = f else {
        return f_selected_window(i, vec![].into());
    };
    let fb = f.borrow();
    Ok(fb
        .root
        .as_ref()
        .map(|r| Value::Window(r.clone()))
        .unwrap_or(Value::Nil))
}

/// `window-fixed-size-p` — GNU windowp-checks WINDOW; nil (nothing
/// fixed in our model).
fn f_windowp_nil(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("windowp", &other)),
    }
}

/// `window-safely-shrinkable-p` — batch tty: always safe → t.
fn f_safely_shrinkable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 0) {
        Value::Nil | Value::Window(_) => Ok(Value::t()),
        other => Err(i.wrong_type_mut("window-valid-p", &other)),
    }
}

/// `face-attributes-as-vector` — GNU maps a plist of `:attr value'
/// onto the 20-slot lface vector; anything else → all unspecified.
fn f_face_attributes_as_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    const ATTRS: [&str; 20] = [
        "face",
        "family",
        "foundry",
        "width",
        "height",
        "weight",
        "slant",
        "underline",
        "inverse-video",
        "foreground",
        "background",
        "stipple",
        "overline",
        "strike-through",
        "box",
        "font",
        "inherit",
        "fontset",
        "distant-foreground",
        "extend",
    ];
    let unspec = i.q("unspecified");
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
    Ok(Value::vector(slots))
}

/// `profiler-memory-running-p` — whether the memory profiler is on.
fn f_profiler_memory_running_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(if i.memory_profiler {
        Value::t()
    } else {
        Value::Nil
    })
}

/// `subr-native-elisp-p' — t when SUBR is a native-compiled Lisp
/// function (our cc-emitted code), nil for built-in primitives.
fn f_subr_native_elisp_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Subr(s) => Ok(Value::from_bool(
            crate::lisp::builtins::comp::is_native(s).is_some(),
        )),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}

fn f_bitmap_spec_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU treats a string as a bitmap spec (file name / data form).
    Ok(Value::from_bool(matches!(&a[0], Value::Str(_))))
}
