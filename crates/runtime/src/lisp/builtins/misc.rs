//! Miscellaneous subrs: symbols, time values, hashing/crypto, file
//! attributes, environment, and small editor glue that doesn't belong
//! to a larger category module.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, SymId, Value};

mod functions;
use functions::{f_closurep, f_func_arity, f_gensym, f_interpreted_function_p, f_subr_arity};
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use functions::{
    f_make_interpreted_closure, lambda_slot_values, make_interpreted_closure, raw_arglist_arity,
};
mod time;
// Preserve the pre-existing misc::* compatibility surface, including currently unused helpers.
#[allow(unused_imports)]
pub(crate) use time::{
    MAX_FIXED_ZONE_OFFSET, Tm, checked_time_zone_cons, current_time_list, current_time_value,
    decode_big_ticks_hz, decode_ticks_hz, gmt_tm, gmtime_r, interp_local_tm, interp_local_tm_named,
    invalid_time_zone, lisp_time_seconds, lisp_time_to_ns, lisp_time_to_ps, lisp_time_to_us,
    local_tm, localtime_r, ns_to_lisp_time, ns_to_lisp_time_i, strftime_spec, time_tm, tz_local_tm,
    tz_lock, uptime_text, us_to_lisp_time, valid_fixed_zone_offset,
};
use time::{
    f_build_binding, f_current_cpu_time, f_decode_time, f_emacs_uptime, f_encode_time,
    f_invocation_dir, f_load_average, f_set_time_zone, f_time_add, f_time_convert, f_time_equal_p,
    f_time_less_p, f_time_subtract,
};
mod crypto;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use crypto::lisp_string_bytes;
use crypto::{f_b64_decode_string, f_b64_encode_string, f_buffer_hash, f_md5, f_secure_hash};
mod files;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use files::f_file_name_case_insensitive_p;
use files::{
    f_directory_name_p, f_file_attributes_lessp, f_get_file_buffer, f_make_symbolic_link,
    f_make_temp_file_internal, f_set_file_times,
};
mod environment;
pub use environment::{
    boot_geteuid, boot_getuid, boot_user_full_name, boot_user_login_name, boot_user_real_login_name,
};
use environment::{
    f_group_name, f_setenv, f_setenv_internal, f_system_groups, f_user_full_name,
    f_user_login_name, f_user_real_login_name, f_user_real_uid, f_user_uid,
};
mod categories;
#[allow(unused_imports)]
pub(crate) use categories::{MceFast, make_category_table_value, mce_fast_for};
use categories::{
    f_category_docstring, f_category_set_mnemonics, f_category_table, f_category_table_p,
    f_char_category_set, f_copy_category_table, f_define_category, f_get_unused_category,
    f_make_category_set, f_make_category_table, f_modify_category_entry, f_set_category_table,
    f_standard_category_table,
};
mod versions;
use versions::{
    f_version_eq, f_version_le, f_version_list_eq, f_version_list_le, f_version_list_lt,
    f_version_lt, f_version_to_list,
};
mod threads;
use threads::{
    f_all_threads, f_condition_mutex, f_condition_name, f_condition_notify, f_condition_variable_p,
    f_condition_wait, f_current_thread, f_make_condition_variable, f_make_mutex, f_make_thread,
    f_mutex_lock, f_mutex_name, f_mutex_unlock, f_mutexp, f_thread_join, f_thread_last_error,
    f_thread_live_p, f_thread_name, f_thread_yield, f_threadp,
};
mod bool_vectors;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use bool_vectors::{bool_vec_of, is_bool_vector, make_bool_vector};
use bool_vectors::{
    f_bool_vector_bin, f_bool_vector_consec, f_bool_vector_count, f_bool_vector_diff,
    f_bool_vector_inter, f_bool_vector_length, f_bool_vector_not, f_bool_vector_p,
    f_bool_vector_subsetp, f_bool_vector_union, f_make_bool_vector,
};

use crate::editor::err_sym;
use crate::editor::is_keymap;
mod keymaps;
use keymaps::{
    f_accessible_keymaps, f_current_active_maps, f_describe_bindings, f_keymap_canonicalize,
    f_keymap_get_keyelt, f_make_composed_keymap, f_map_keymap, f_map_keymap_internal,
};
mod chartables;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use chartables::{
    CT_MAX_CHAR, CharTableSnapshot, char_table_raw, char_table_ref, char_table_vec, ct_collect,
    ct_copy, ct_effective_runs, ct_extra, ct_raw_tag, ct_refresh_ascii, ct_replay, ct_set,
    ct_set_range, heal_standard_case_table, is_char_table, is_sub_ct_tag, is_uniprop_table,
    make_ct, set_case_table_fill,
};
use chartables::{
    f_char_table_extra_slot, f_char_table_p, f_char_table_parent, f_char_table_range,
    f_char_table_subtype, f_map_char_table, f_remacs_char_width_table, f_set_char_table_extra_slot,
    f_set_char_table_parent, f_set_char_table_range,
};
mod diagnostics;
pub use diagnostics::error_message;
use diagnostics::f_error_message_string;
mod documentation;
use documentation::f_function_documentation;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use documentation::{doc_offset, doc_text, doc_text_at, install_variable_docs};
mod events;
use events::{
    f_event_basic_type, f_event_modifiers, f_eventp, f_key_valid_p, f_listify_key_sequence,
};
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use events::{
    f_event_convert_list, f_internal_event_symbol_parse, reorder_event_symbol,
};

mod objects;
use keymaps::f_suppress_keymap;
use objects::{f_record, f_recordp};
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use objects::{has_opaque_array_storage, is_record, record_array_accessible};

use environment::f_invocation_name;

use time::{f_date_to_time, f_days_between};
mod positions;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use positions::{
    SYMPOS_MAGIC, as_sympos, make_symbol_with_pos, make_sympos, raw_symbol_position, sym_pos_parts,
    symbol_position_for, sympos_enabled_p, unpos,
};
use positions::{
    f_bare_symbol, f_bare_symbol_p, f_position_symbol, f_remove_pos_from_symbol,
    f_symbol_with_pos_p, f_symbol_with_pos_pos,
};
mod coding;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use coding::{
    CODING_ALIASES, CODING_SYSTEMS, add_to_coding_system_list, check_coding, coding_alias_group,
    coding_known, f_coding_system_plist,
};
use coding::{
    f_check_coding_system, f_coding_system_aliases, f_coding_system_base, f_coding_system_eol_type,
    f_coding_system_get, f_coding_system_list, f_coding_system_p, f_coding_system_priority_list,
    f_coding_system_put, f_keyboard_coding_system, f_terminal_coding_system,
};
mod introspection;
use introspection::f_help_function_arglist;

use environment::{
    f_getenv_internal, f_group_gid, f_group_real_gid, f_locale_info, f_num_processors,
    f_system_users,
};
mod compat;
use compat::{
    f_bidi_find_overridden, f_bidi_resolved_levels, f_bidi_string_mark_left_to_right,
    f_byte_to_string, f_command_line, f_command_modes, f_current_bidi_paragraph_direction,
    f_daemon_initialized, f_flex_cost_gotoh, f_flush_stdout, f_format_mode_line,
    f_get_load_suffixes, f_glyph_char, f_gnutls_available_p, f_gui_get_selection,
    f_internal_handle_focus_in, f_make_glyph_code, f_message_box, f_module_load,
    f_network_lookup_address_info, f_process_arg_err, f_profiler_cpu_running_p,
    f_re_describe_compiled, f_send_string_to_terminal, f_set_output_flow_control, f_signal_names,
    f_version_list_not_zero, f_x_close_connection, f_x_get_resource, f_x_hide_tip, f_x_open_connection,
    f_x_parse_geometry, f_x_show_tip,
};
mod buffers;
use buffers::{
    f_abort_minibuffers, f_buffer_text_pixel_size, f_bufferpos_to_filepos, f_filepos_to_bufferpos,
    f_find_buffer, f_get_truename_buffer, f_minibuffer_depth, f_newline_cache_check,
    f_safely_shrinkable, f_set_buffer_multibyte, f_shrink_if_larger,
};
use diagnostics::{
    f_backtrace_debug, f_backtrace_eval, f_backtrace_frame_internal, f_backtrace_locals,
    f_command_error_default, f_describe_vector, f_documentation_stringp,
    f_external_debugging_output, f_help_describe_vector, f_internal_describe_syntax_value,
    f_mapbacktrace, f_open_dribble_file, f_open_termscript,
};
use keymaps::f_set_this_command_keys;
mod display;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use display::{decode_display, mouse_coordinate, warp_mouse_absolute, win_min_size};
use display::{
    f_current_frame_configuration, f_delete_other_windows_internal, f_delete_windows_on,
    f_display_false, f_display_mm, f_display_monitor_attributes_list, f_display_pixel_height,
    f_display_pixel_width, f_display_planes, f_display_pred_false, f_display_screens,
    f_display_visual_class, f_fit_window_to_buffer, f_frame_configuration_p, f_frame_geometry,
    f_frame_height_val, f_frame_monitor_attributes, f_frame_or_buffer_changed_p,
    f_frame_outer_dim, f_frame_width_val, f_get_display_property, f_get_window_pred,
    f_move_to_window_line, f_ns_display, f_ns_frame_stub_err, f_pos_visible, f_scroll_bar_scale,
    f_set_buffer_redisplay, f_set_mouse_absolute_pixel_position, f_set_mouse_pixel_position,
    f_set_mouse_position, f_tab_bar_height, f_window_at_side_p, f_window_child,
    f_window_child_count, f_window_combined_p, f_window_cursor_info, f_window_display_buffer,
    f_window_in_direction, f_window_left_column, f_window_line, f_window_main_window,
    f_window_max_chars, f_window_norm_buffer, f_window_norm_frame, f_window_normalize,
    f_window_preserve_size, f_window_sizable, f_window_with_parameter, f_windowp_nil,
    f_xw_display_color_p,
};
mod faces;
use faces::{
    f_close_font, f_color_defined_p, f_color_distance, f_color_gray_p,
    f_color_values_from_color_spec, f_define_fringe_bitmap, f_destroy_fringe_bitmap,
    f_display_color_cells, f_face_attributes_as_vector, f_face_documentation, f_face_font,
    f_face_name, f_font_at, f_font_face_attributes, f_font_has_char_p, f_font_info, f_font_match_p,
    f_font_object_stub, f_font_shape_gstring, f_font_spec, f_glyph_face, f_internal_copy_lisp_face,
    f_internal_make_lisp_face, f_invert_face, f_query_fontset, f_select_font,
    f_set_alt_font_family_alist, f_set_alt_font_registry_alist, f_set_font_selection_order,
    f_set_frame_font, f_set_fringe_bitmap_face, f_set_lisp_face_attr_resource,
    f_window_font_height, f_window_font_width, f_xw_color_defined_p,
};
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use faces::{parse_color_16, tty_color_desc};
mod images;
use chartables::f_optimize_char_table;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use coding::{
    coding_canonical, coding_fixed_eol, coding_inherit_file_eol, decode_file_bytes,
    encode_process_text, f_decode_coding_string, f_encode_coding_string,
    f_find_composition_internal, post_file_conversion, process_decode_bytes, process_decode_result,
    process_encode_source, raw_file_coding,
};
use coding::{
    f_ccl_execute, f_ccl_execute_on_string, f_ccl_program_p, f_check_coding_systems_region,
    f_compose_region_internal, f_compose_string_internal, f_composition_get_gstring,
    f_composition_sort_rules, f_decode_coding_region, f_define_coding_system_alias,
    f_detect_coding_region, f_detect_coding_string, f_encode_coding_char, f_encode_coding_region,
    f_find_operation_coding_system, f_insert_byte, f_read_non_nil_coding_system,
    f_register_ccl_program, f_register_code_conversion_map, f_set_binary_mode,
    f_set_input_meta_mode, f_set_keyboard_coding_system, f_set_safe_terminal_coding,
    f_set_terminal_coding, f_set_terminal_coding_system, f_unencodable_char_position,
    f_zlib_available_p, f_zlib_decompress_region, with_coding_buffer,
};
use crypto::f_secure_hash_algorithms;
#[allow(unused_imports)] // misc::* compatibility re-exports
pub(crate) use events::decode_popup_position;
use events::{
    f_current_input_mode, f_display_popup_menus_p, f_lossage_size, f_mouse_pixel_position,
    f_mouse_position,
    f_mouse_position_root, f_popup_menu, f_remember_mouse_glyph, f_tooltip_mode, popup_car_safe,
    popup_fcar, popup_fcdr,
};
use files::{
    f_file_selinux_context, f_move_file_to_trash, f_set_file_acl, f_set_file_selinux_context,
};
use functions::{
    f_make_closure, f_make_var_non_special, f_primitive_function_p, f_read_expression,
    f_special_variable_p, f_subr_native_elisp_p,
};
use images::{
    f_bitmap_spec_p, f_clear_image_cache, f_image_flush, f_image_metadata, f_image_spec_check,
    f_image_transforms_p, f_image_type, f_image_type_available_p, f_imagep, f_init_image_library,
    f_lookup_image_map, f_put_image, f_remove_images,
};
use objects::{
    f_cl_type_of, f_garbage_collect_maybe, f_gc_heapsize, f_length_eq, f_length_gt, f_length_lt,
    f_make_finalizer, f_memory_limit, f_memory_use_counts, f_profiler_memory_running_p,
    f_string_or_null_p, f_vector_or_char_table_p,
};
use positions::f_read_positioning_symbols;
use threads::{
    f_backtrace_frames_from_thread, f_thread_blocker, f_thread_buffer_disposition,
    f_thread_set_buffer_disposition, f_thread_signal,
};
use time::{
    f_date_leap_year_p, f_days_to_time, f_seconds_to_time, f_time_since, f_time_to_day_in_year,
    f_time_to_days,
};

#[allow(unused_imports)] // misc::* compatibility re-exports
pub(super) use objects::f_value_lt;
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
    S!(
        "thread-yield",
        0,
        0,
        f_thread_yield,
        "Yield to other threads."
    ),
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
    S!("mouse-pixel-position", 0, 0, f_mouse_pixel_position, ""),
    S!("set-mouse-position", 3, 3, f_set_mouse_position, ""),
    S!(
        "set-mouse-pixel-position",
        3,
        3,
        f_set_mouse_pixel_position,
        ""
    ),
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
    S!("xw-display-color-p", 0, 1, f_xw_display_color_p, ""),
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
    S!(
        "set-mouse-absolute-pixel-position",
        2,
        2,
        f_set_mouse_absolute_pixel_position,
        ""
    ),
    S!("tooltip-mode", 0, 1, f_tooltip_mode, ""),
    // `keymap-of' does not exist in GNU Emacs 31.
    // ---------- display/font/image stubs (no GUI) ----------
    S!("default-font-width", 0, 0, f_default_font_width, "Char cell width."),
    S!("default-font-height", 0, 0, f_default_font_height, "Char cell height."),
    S!(
        "window-font-width",
        0,
        2,
        f_window_font_width,
        "Char cell width."
    ),
    S!(
        "window-font-height",
        0,
        2,
        f_window_font_height,
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
        f_select_font,
        "Choose a font using the graphical font panel."
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
    S!("x-hide-tip", 0, 0, f_x_hide_tip, ""),
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

fn f_t(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

fn f_nil(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

fn f_false(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

// ---------- predicates ----------

/// Wrap a Value in (quote v) for `call_function`'s unevaluated args.
fn quoted(v: Value) -> Value {
    Value::list(vec![Value::Sym(sym::QUOTE), v])
}

// ---------- misc ----------

fn f_arg1(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(a[1].clone())
}

// ---- popup-menu position decode (GNU menu.c x_popup_menu_1) ----
//
// Shared between `popup-menu' (which applies menu-bar.el's
// `popup-menu-normalize-position' first) and `x-popup-menu' (raw).

/// Apply the user's file coding detector to a raw, unibyte sample. GNU
/// samples the first KiB and last three KiB without modifying the target.
pub(crate) fn auto_file_coding<T: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    path: &T,
    bytes: &[u8],
) -> EvalResult {
    path.with_text(|path| {
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
    })
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

// ---------- windows ----------

// ---------- added GNU compat subrs ----------

/// GNU `value<': compares only within the same broad type —
/// numbers numerically, strings lexically, symbols by name,
/// conses/vectors elementwise; any type mismatch signals
/// (error 'type-mismatch), and otherwise-comparable pairs with no
/// ordering answer yield nil.
// GNU fns.c value_cmp: ordering-equivalence is distinct from `equal'.

fn f_default_font_width(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    Ok(Value::Int(
        crate::editor::frame_char_width(i, &frame) as i128,
    ))
}

fn f_default_font_height(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let frame = crate::editor::sel_frame(i).ok_or_else(|| i.error("No frame"))?;
    Ok(Value::Int(
        crate::editor::frame_char_height(i, &frame) as i128,
    ))
}

// ---------- internals & platform stubs (probe-matched arities) ----------

// Compatibility names for interpreter callers during the representation migration.
pub(crate) fn any_sympos_parts(i: &Interp, value: &Value) -> Option<(SymId, i128)> {
    sym_pos_parts(i, value)
}
pub(crate) fn active_sympos_parts(i: &Interp, value: &Value) -> Option<(SymId, i128)> {
    symbol_position_for(i, value)
}
pub(crate) fn recordp(i: &Interp, value: &Value) -> bool {
    is_record(i, value)
}
pub(crate) fn is_opaque_record(_i: &Interp, value: &Value) -> bool {
    matches!(
        value.record_kind(),
        Some(crate::lisp::value::RecordKind::Obarray | crate::lisp::value::RecordKind::FontSpec)
    )
}
