//! GNU-compat QA suite runner: each test loads `qa_framework.el`
//! (assertion harness) plus one `qa_*.el` file of `(qa FORM EXPECTED)`
//! checks and asserts the summary "N checks passed".  Every expected
//! value is verified against GNU Emacs 31.1 (`emacs -Q --batch`).
//!
//! The qa_*.el files are also directly loadable:
//!   emacs -Q --batch -l tests/qa_framework.el -l tests/qa_<dom>.el
//!   remacs --batch --load tests/qa_framework.el --load tests/qa_<dom>.el
#![recursion_limit = "4096"]
mod common;

use common::*;

/// Load framework + test file through `load' so the file sees the same
/// environment as the GNU `-l' run (noninteractive=t, load context).
fn run(file: &'static str) {
    let src = format!(
        "(progn (load {:?}) (load {:?}) nil)",
        format!("{}/tests/qa_framework.el", env!("CARGO_MANIFEST_DIR")),
        format!("{}/tests/{}", env!("CARGO_MANIFEST_DIR"), file)
    );
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let (mut i, out) = batch_interp();
            match i.eval_str(&src) {
                Ok(_) => {
                    let o = out.borrow();
                    assert!(o.contains("checks passed"), "qa {file} summary: {o}");
                }
                Err(_f) => panic!("qa {file} eval error; captured output:\n{}", out.borrow()),
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

macro_rules! qa {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            run($file);
        }
    };
}

qa!(qa_abbrev, "qa_abbrev.el");
qa!(qa_abbrev2, "qa_abbrev2.el");
qa!(qa_absolute_name, "qa_absolute_name.el");
qa!(qa_advice, "qa_advice.el");
qa!(qa_interactive_ignore, "qa_interactive_ignore.el");
qa!(qa_apply, "qa_apply.el");
qa!(qa_apply2, "qa_apply2.el");
qa!(qa_arith, "qa_arith.el");
qa!(qa_array, "qa_array.el");
qa!(qa_backquote, "qa_backquote.el");
qa!(qa_backtrace_frames, "qa_backtrace_frames.el");
qa!(qa_backup, "qa_backup.el");
qa!(qa_buffer, "qa_buffer.el");
qa!(qa_buffer_history, "qa_buffer_history.el");
qa!(qa_before_markers, "qa_before_markers.el");
qa!(qa_float_output, "qa_float_output.el");
qa!(qa_process_defaults, "qa_process_defaults.el");
qa!(qa_process_callback_context, "qa_process_callback_context.el");
qa!(qa_process_callback_errors, "qa_process_callback_errors.el");
qa!(qa_process_buffer_identity, "qa_process_buffer_identity.el");
qa!(qa_process_window_size, "qa_process_window_size.el");
qa!(qa_process_names, "qa_process_names.el");
qa!(qa_network_datagram, "qa_network_datagram.el");
qa!(qa_network_nowait, "qa_network_nowait.el");
qa!(qa_network_random_service, "qa_network_random_service.el");
qa!(qa_fresh_buffer_locals, "qa_fresh_buffer_locals.el");
qa!(qa_buffer_local_value_type, "qa_buffer_local_value_type.el");
qa!(qa_buffer2, "qa_buffer2.el");
qa!(qa_buffer_transfer, "qa_buffer_transfer.el");
qa!(qa_append_to_buffer, "qa_append_to_buffer.el");
qa!(qa_bufferfile, "qa_bufferfile.el");
qa!(qa_buffmenu, "qa_buffmenu.el");
qa!(qa_display_fallback, "qa_display_fallback.el");
qa!(qa_case, "qa_case.el");
qa!(qa_case_name_properties, "qa_case_name_properties.el");
qa!(qa_char, "qa_char.el");
qa!(qa_char_to_string, "qa_char_to_string.el");
qa!(qa_char_markers, "qa_char_markers.el");
qa!(qa_substitute_chars, "qa_substitute_chars.el");
qa!(qa_charset, "qa_charset.el");
qa!(qa_character_width_table, "qa_character_width_table.el");
qa!(
    qa_decomposition_description,
    "qa_decomposition_description.el"
);
qa!(qa_chartables, "qa_chartables.el");
qa!(qa_char_table_range_alias, "qa_char_table_range_alias.el");
qa!(qa_char_table_reader, "qa_char_table_reader.el");
qa!(qa_category_defaults, "qa_category_defaults.el");
qa!(qa_clloop, "qa_clloop.el");
qa!(qa_clloop2, "qa_clloop2.el");
qa!(qa_clloop3, "qa_clloop3.el");
qa!(qa_clloop4, "qa_clloop4.el");
qa!(qa_clloop5, "qa_clloop5.el");
qa!(qa_clloop6, "qa_clloop6.el");
qa!(qa_clloop7, "qa_clloop7.el");
qa!(qa_clmisc, "qa_clmisc.el");
qa!(qa_clprint, "qa_clprint.el");
qa!(qa_clseq, "qa_clseq.el");
qa!(qa_clseq2, "qa_clseq2.el");
qa!(qa_cmds, "qa_cmds.el");
qa!(qa_whitespace_narrowing, "qa_whitespace_narrowing.el");
qa!(qa_blank_line_characters, "qa_blank_line_characters.el");
qa!(qa_legacy_codecs, "qa_legacy_codecs.el");
qa!(qa_coding, "qa_coding.el");
qa!(qa_coding_region, "qa_coding_region.el");
qa!(qa_unicode_codecs, "qa_unicode_codecs.el");
qa!(qa_coding2, "qa_coding2.el");
qa!(qa_coding_detection, "qa_coding_detection.el");
qa!(qa_completion, "qa_completion.el");
qa!(qa_ctl, "qa_ctl.el");
qa!(qa_custom, "qa_custom.el");
qa!(qa_custom2, "qa_custom2.el");
qa!(qa_descr, "qa_descr.el");
qa!(qa_doc, "qa_doc.el");
qa!(qa_documentation_property, "qa_documentation_property.el");
qa!(qa_text_char_description, "qa_text_char_description.el");
qa!(qa_function_alias_docs, "qa_function_alias_docs.el");
qa!(qa_variable_documentation, "qa_variable_documentation.el");
qa!(qa_vconcat_unibyte, "qa_vconcat_unibyte.el");
qa!(qa_eieio, "qa_eieio.el");
qa!(qa_equal_cycles, "qa_equal_cycles.el");
qa!(qa_equal_depth, "qa_equal_depth.el");
qa!(qa_equal_callers, "qa_equal_callers.el");
qa!(qa_equal_runtime, "qa_equal_runtime.el");
qa!(qa_eieio2, "qa_eieio2.el");
qa!(qa_edge_semantics, "qa_edge_semantics.el");
qa!(qa_transpose_regions, "qa_transpose_regions.el");
qa!(qa_transpose_properties, "qa_transpose_properties.el");
qa!(qa_err2, "qa_err2.el");
qa!(qa_errors, "qa_errors.el");
qa!(qa_dotted_error_data, "qa_dotted_error_data.el");
qa!(qa_eval, "qa_eval.el");
qa!(qa_eval_narrowing, "qa_eval_narrowing.el");
qa!(qa_eval_lexical, "qa_eval_lexical.el");
qa!(qa_eval_lexenv, "qa_eval_lexenv.el");
qa!(qa_closure_constructor, "qa_closure_constructor.el");
qa!(qa_cl_boot_surface, "qa_cl_boot_surface.el");
qa!(qa_obsolete_pushnew, "qa_obsolete_pushnew.el");
qa!(qa_menu_keys, "qa_menu_keys.el");
qa!(qa_basic_faces, "qa_basic_faces.el");
qa!(qa_face, "qa_face.el");
qa!(qa_face3, "qa_face3.el");
qa!(qa_faces2, "qa_faces2.el");
qa!(qa_face_remap_autoload, "qa_face_remap_autoload.el");
qa!(qa_face_remap_require, "qa_face_remap_require.el");
qa!(qa_json_sentinels, "qa_json_sentinels.el");
qa!(qa_json_narrow, "qa_json_narrow.el");
qa!(qa_json_utf8, "qa_json_utf8.el");
qa!(qa_json_output, "qa_json_output.el");
qa!(qa_json_insert_bytes, "qa_json_insert_bytes.el");
qa!(qa_json_number_range, "qa_json_number_range.el");
qa!(qa_json_cycles, "qa_json_cycles.el");
qa!(qa_fileattr, "qa_fileattr.el");
qa!(qa_file_modes, "qa_file_modes.el");
qa!(qa_extension_period, "qa_extension_period.el");
qa!(qa_file_completion_prefix, "qa_file_completion_prefix.el");
qa!(qa_directory_order, "qa_directory_order.el");
qa!(qa_filelocal, "qa_filelocal.el");
qa!(qa_filesys, "qa_filesys.el");
qa!(qa_file_directory_handlers, "qa_file_directory_handlers.el");
qa!(qa_file_truename_roots, "qa_file_truename_roots.el");
qa!(qa_copy_file_times, "qa_copy_file_times.el");
qa!(qa_copy_file_self, "qa_copy_file_self.el");
qa!(qa_file_broken_symlinks, "qa_file_broken_symlinks.el");
qa!(qa_file_primitive_handlers, "qa_file_primitive_handlers.el");
qa!(qa_file_handler_io, "qa_file_handler_io.el");
qa!(qa_file_remote, "qa_file_remote.el");
qa!(qa_fill, "qa_fill.el");
qa!(qa_fontlock, "qa_fontlock.el");
qa!(qa_docstring_escape, "qa_docstring_escape.el");
qa!(qa_skip_classes, "qa_skip_classes.el");
qa!(qa_tab_indent, "qa_tab_indent.el");
qa!(qa_format, "qa_format.el");
qa!(qa_format_conversions, "qa_format_conversions.el");
qa!(qa_globdir, "qa_globdir.el");
qa!(qa_gv, "qa_gv.el");
qa!(qa_hash, "qa_hash.el");
qa!(qa_weak_hash, "qa_weak_hash.el");
qa!(qa_sxhash_boundaries, "qa_sxhash_boundaries.el");
qa!(qa_hash_float_identity, "qa_hash_float_identity.el");
qa!(qa_hash_markers, "qa_hash_markers.el");
qa!(qa_hash_custom, "qa_hash_custom.el");
qa!(qa_hash_cycles, "qa_hash_cycles.el");
qa!(qa_hash2, "qa_hash2.el");
qa!(qa_help, "qa_help.el");
qa!(qa_hook, "qa_hook.el");
qa!(qa_hooks2, "qa_hooks2.el");
qa!(qa_hook_traversal, "qa_hook_traversal.el");
qa!(qa_load_lexical_cookie, "qa_load_lexical_cookie.el");
qa!(qa_signal_hook, "qa_signal_hook.el");
qa!(qa_signal_room, "qa_signal_room.el");
qa!(qa_quit_flag, "qa_quit_flag.el");
// Exact nesting budgets depend on the entry context: use GNU's real -l
// path, rather than the in-process runner's extra (progn (load ...)) call.
#[test]
fn qa_call_depth() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    for source in [false, true] {
        let root = env!("CARGO_MANIFEST_DIR");
        let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
        command.args(["-Q", "--batch", "-l", &format!("{root}/tests/qa_framework.el"),
                      "-l", &format!("{root}/tests/qa_call_depth.el")])
            .env_remove("EMACSLOADPATH")
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if source { command.env("REMACS_NO_ELC", "1"); }
        else { command.env_remove("REMACS_NO_ELC"); }
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() { break; }
            if started.elapsed() > Duration::from_secs(180) {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("call-depth watchdog source={source}: {}\n{}",
                       String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "call-depth source={source}: {}\n{}",
                String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("checks passed"));
    }
}
qa!(qa_modhooks, "qa_modhooks.el");
qa!(qa_readonly, "qa_readonly.el");
qa!(qa_indent, "qa_indent.el");
qa!(qa_indentcmds, "qa_indentcmds.el");
qa!(qa_indentfill, "qa_indentfill.el");
qa!(qa_input, "qa_input.el");
qa!(qa_event_conversion, "qa_event_conversion.el");
qa!(qa_char_resolve, "qa_char_resolve.el");
qa!(qa_read_events, "qa_read_events.el");
qa!(qa_crm, "qa_crm.el");
qa!(qa_json, "qa_json.el");
qa!(qa_kbd, "qa_kbd.el");
qa!(qa_define_key_type, "qa_define_key_type.el");
qa!(qa_keymap, "qa_keymap.el");
qa!(qa_key_parse, "qa_key_parse.el");
qa!(qa_modified_named_keys, "qa_modified_named_keys.el");
qa!(qa_keymap_cycles, "qa_keymap_cycles.el");
qa!(qa_keymap2, "qa_keymap2.el");
qa!(qa_keymap3, "qa_keymap3.el");
qa!(qa_kmacro, "qa_kmacro.el");
qa!(qa_kbd_macro_repeat, "qa_kbd_macro_repeat.el");
qa!(qa_list, "qa_list.el");
qa!(qa_lists, "qa_lists.el");
qa!(qa_length_functions, "qa_length_functions.el");
qa!(qa_length_nonsequence, "qa_length_nonsequence.el");
qa!(qa_lists2, "qa_lists2.el");
qa!(qa_load, "qa_load.el");
qa!(qa_autoload_dispatch, "qa_autoload_dispatch.el");
qa!(qa_load_read_stream, "qa_load_read_stream.el");
qa!(qa_unload_dependencies, "qa_unload_dependencies.el");
qa!(qa_locate_embedded, "qa_locate_embedded.el");
qa!(qa_load_macroexpand, "qa_load_macroexpand.el");
qa!(qa_load_buffer, "qa_load_buffer.el");
qa!(qa_killed_match_markers, "qa_killed_match_markers.el");
qa!(qa_logb, "qa_logb.el");
qa!(qa_locvar, "qa_locvar.el");
qa!(qa_variable_protection, "qa_variable_protection.el");
qa!(qa_frame_parameters, "qa_frame_parameters.el");
qa!(qa_frame_buffer_changes, "qa_frame_buffer_changes.el");
qa!(qa_feature_capabilities, "qa_feature_capabilities.el");
qa!(qa_macro, "qa_macro.el");
qa!(qa_macro_environment, "qa_macro_environment.el");
qa!(qa_map, "qa_map.el");
qa!(qa_map2, "qa_map2.el");
qa!(qa_mapring, "qa_mapring.el");
qa!(qa_mark, "qa_mark.el");
qa!(qa_marker, "qa_marker.el");
qa!(qa_markers, "qa_markers.el");
qa!(qa_mark_excursion, "qa_mark_excursion.el");
qa!(qa_match, "qa_match.el");
qa!(qa_math, "qa_math.el");
qa!(qa_minibuf, "qa_minibuf.el");
qa!(qa_minibuffer, "qa_minibuffer.el");
qa!(qa_set_minibuffer_window, "qa_set_minibuffer_window.el");
qa!(qa_misc2, "qa_misc2.el");
qa!(qa_misc3, "qa_misc3.el");
qa!(qa_miscfn, "qa_miscfn.el");
qa!(qa_memory_statistics, "qa_memory_statistics.el");
qa!(qa_profiler, "qa_profiler.el");
qa!(qa_pdumper, "qa_pdumper.el");
qa!(qa_count_lines, "qa_count_lines.el");
qa!(qa_gap, "qa_gap.el");
qa!(qa_misc_differences, "qa_misc_differences.el");
qa!(qa_modes, "qa_modes.el");
qa!(qa_narrow, "qa_narrow.el");
#[test]
fn qa_native_error() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native error tests: no C compiler available");
        return;
    }
    run("qa_native_error.el");
}
#[test]
fn qa_native_variables() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native variable tests: no C compiler available");
        return;
    }
    run("qa_native_variables.el");
}
#[test]
fn qa_native_closures() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native closure tests: no C compiler available");
        return;
    }
    run("qa_native_closures.el");
}
#[test]
fn qa_native_letstar() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native let* tests: no C compiler available");
        return;
    }
    run("qa_native_letstar.el");
}
#[test]
fn qa_native_toplevel() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native top-level tests: no C compiler available");
        return;
    }
    run("qa_native_toplevel.el");
}
#[test]
fn qa_native_boolean() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native boolean tests: no C compiler available");
        return;
    }
    run("qa_native_boolean.el");
}
#[test]
fn qa_native_constants() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native constant tests: no C compiler available");
        return;
    }
    run("qa_native_constants.el");
}
#[test]
fn qa_native_strings() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native string tests: no C compiler available");
        return;
    }
    run("qa_native_strings.el");
}
#[test]
fn qa_native_parse_errors() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native parse-error tests: no C compiler available");
        return;
    }
    run("qa_native_parse_errors.el");
}
qa!(qa_numbers, "qa_numbers.el");
qa!(qa_arbitrary_precision, "qa_arbitrary_precision.el");
qa!(qa_obarray, "qa_obarray.el");
qa!(qa_unintern_identity, "qa_unintern_identity.el");
qa!(qa_overlay, "qa_overlay.el");
qa!(qa_overlays, "qa_overlays.el");
qa!(qa_overlay_equality, "qa_overlay_equality.el");
qa!(qa_current_kill, "qa_current_kill.el");
qa!(qa_kill_new, "qa_kill_new.el");
qa!(qa_pcase, "qa_pcase.el");
qa!(qa_plists, "qa_plists.el");
qa!(qa_point, "qa_point.el");
qa!(qa_pp, "qa_pp.el");
qa!(qa_printvars, "qa_printvars.el");
qa!(qa_print_overrides, "qa_print_overrides.el");
qa!(qa_write_char, "qa_write_char.el");
qa!(qa_output_streams, "qa_output_streams.el");
qa!(qa_print_variables, "qa_print_variables.el");
qa!(qa_quoting_scope, "qa_quoting_scope.el");
qa!(qa_symbol_print_escape, "qa_symbol_print_escape.el");
qa!(qa_process, "qa_process.el");
qa!(qa_accept_process_output, "qa_accept_process_output.el");
qa!(qa_accept_process_wait, "qa_accept_process_wait.el");
qa!(qa_process_connection, "qa_process_connection.el");
qa!(qa_process_coding, "qa_process_coding.el");
qa!(qa_process_utf8_chunks, "qa_process_utf8_chunks.el");
qa!(qa_process_environment, "qa_process_environment.el");
qa!(qa_process_directory, "qa_process_directory.el");
qa!(qa_process_exec_path, "qa_process_exec_path.el");
qa!(qa_process_filter_pause, "qa_process_filter_pause.el");
qa!(qa_process_send_region, "qa_process_send_region.el");
qa!(qa_process_timeout, "qa_process_timeout.el");
qa!(qa_process_network_eof, "qa_process_network_eof.el");
qa!(qa_network_listener_delete, "qa_network_listener_delete.el");
qa!(qa_network_accept_callbacks, "qa_network_accept_callbacks.el");
qa!(qa_network_contacts, "qa_network_contacts.el");
qa!(qa_network_stop, "qa_network_stop.el");
qa!(qa_treesit_edit, "qa_treesit_edit.el");
qa!(qa_treesit_node_generation, "qa_treesit_node_generation.el");
qa!(qa_treesit_indirect, "qa_treesit_indirect.el");
qa!(qa_treesit_notifiers, "qa_treesit_notifiers.el");
qa!(qa_treesit_query_case, "qa_treesit_query_case.el");
qa!(qa_treesit_repeat_capture, "qa_treesit_repeat_capture.el");
qa!(qa_treesit_ranges, "qa_treesit_ranges.el");
qa!(qa_process_stderr, "qa_process_stderr.el");
qa!(qa_radix, "qa_radix.el");
qa!(qa_read, "qa_read.el");
qa!(qa_readprint, "qa_readprint.el");
qa!(qa_reader_function_stream, "qa_reader_function_stream.el");
qa!(qa_reader_string_kind, "qa_reader_string_kind.el");
qa!(qa_reader_symbol_source, "qa_reader_symbol_source.el");
qa!(qa_reader_source_dispatch, "qa_reader_source_dispatch.el");
qa!(qa_obarray_defaults, "qa_obarray_defaults.el");
qa!(qa_symbol_shorthands, "qa_symbol_shorthands.el");
qa!(qa_load_shorthands, "qa_load_shorthands.el");
qa!(qa_shorthand_characters, "qa_shorthand_characters.el");
qa!(qa_reader_error_context, "qa_reader_error_context.el");
qa!(qa_reader_circles, "qa_reader_circles.el");
qa!(qa_rect, "qa_rect.el");
qa!(qa_regex, "qa_regex.el");
qa!(qa_regex_properties, "qa_regex_properties.el");
qa!(qa_regexp_classes, "qa_regexp_classes.el");
qa!(qa_regexp_errors, "qa_regexp_errors.el");
qa!(qa_regexpopt, "qa_regexpopt.el");
qa!(qa_replace, "qa_replace.el");
qa!(qa_replace_metadata, "qa_replace_metadata.el");
qa!(qa_replace_region_contents, "qa_replace_region_contents.el");
qa!(qa_replace_region_narrowing, "qa_replace_region_narrowing.el");
qa!(qa_replace_region_properties, "qa_replace_region_properties.el");
qa!(qa_replace_bounds, "qa_replace_bounds.el");
qa!(qa_replace_case, "qa_replace_case.el");
qa!(qa_ring, "qa_ring.el");
qa!(qa_rx, "qa_rx.el");
qa!(qa_search, "qa_search.el");
qa!(qa_secure_hash_binary, "qa_secure_hash_binary.el");
qa!(qa_seq, "qa_seq.el");
qa!(qa_seq2, "qa_seq2.el");
qa!(qa_sort_callbacks, "qa_sort_callbacks.el");
qa!(qa_sort, "qa_sort.el");
qa!(qa_special, "qa_special.el");
qa!(qa_special_variables, "qa_special_variables.el");
qa!(qa_special2, "qa_special2.el");
qa!(qa_string, "qa_string.el");
qa!(qa_string_number, "qa_string_number.el");
qa!(qa_split_string, "qa_split_string.el");
qa!(qa_string_search, "qa_string_search.el");
qa!(qa_strings, "qa_strings.el");
qa!(qa_string_make_multibyte, "qa_string_make_multibyte.el");
qa!(qa_string_to_list, "qa_string_to_list.el");
qa!(qa_string_to_vector, "qa_string_to_vector.el");
qa!(qa_string_order, "qa_string_order.el");
qa!(qa_concat_mule_chars, "qa_concat_mule_chars.el");
qa!(qa_string_replace_mule, "qa_string_replace_mule.el");
qa!(qa_string_trim_mule, "qa_string_trim_mule.el");
qa!(qa_string_trim_invalid_regexp, "qa_string_trim_invalid_regexp.el");
qa!(qa_subst_char_mule, "qa_subst_char_mule.el");
qa!(qa_store_substring_mule, "qa_store_substring_mule.el");
qa!(qa_case_table_mule, "qa_case_table_mule.el");
qa!(qa_truncate_string_width_edges, "qa_truncate_string_width_edges.el");
qa!(qa_truncate_width_mule_padding, "qa_truncate_width_mule_padding.el");
qa!(qa_string_pad_properties, "qa_string_pad_properties.el");
qa!(qa_string_chop_newline_properties, "qa_string_chop_newline_properties.el");
qa!(qa_string_join_properties, "qa_string_join_properties.el");
qa!(qa_truncate_width_float, "qa_truncate_width_float.el");
qa!(qa_zero_vector_identity, "qa_zero_vector_identity.el");
qa!(qa_string_replace_properties, "qa_string_replace_properties.el");
qa!(qa_string_trim_properties, "qa_string_trim_properties.el");
qa!(qa_string_fill_properties, "qa_string_fill_properties.el");
qa!(qa_string_reverse_byte_kind, "qa_string_reverse_byte_kind.el");
qa!(qa_string_join_byte_kind, "qa_string_join_byte_kind.el");
qa!(qa_string_pad_byte_kind, "qa_string_pad_byte_kind.el");
qa!(qa_string_trim_byte_kind, "qa_string_trim_byte_kind.el");
qa!(qa_string_chop_newline_byte_kind, "qa_string_chop_newline_byte_kind.el");
qa!(qa_string_replace_byte_kind, "qa_string_replace_byte_kind.el");
qa!(qa_subst_char_byte_kind, "qa_subst_char_byte_kind.el");
qa!(qa_delete_identity, "qa_delete_identity.el");
qa!(qa_delete_string_mule, "qa_delete_string_mule.el");
qa!(qa_string_truncate_left_properties, "qa_string_truncate_left_properties.el");
qa!(qa_string_truncate_left_sequences, "qa_string_truncate_left_sequences.el");
qa!(qa_string_truncate_left_float, "qa_string_truncate_left_float.el");
qa!(qa_string_truncate_left_empty_negative, "qa_string_truncate_left_empty_negative.el");
qa!(qa_string_pad_argument_order, "qa_string_pad_argument_order.el");
qa!(qa_string_pad_unused_padding, "qa_string_pad_unused_padding.el");
qa!(qa_string_pad_mule_padding, "qa_string_pad_mule_padding.el");
qa!(qa_string_fill_character_input, "qa_string_fill_character_input.el");
qa!(qa_string_replace_element_type, "qa_string_replace_element_type.el");
qa!(qa_string_replace_invalid_code, "qa_string_replace_invalid_code.el");
qa!(qa_string_fill_trailing_newline, "qa_string_fill_trailing_newline.el");
qa!(qa_string_fill_empty_width, "qa_string_fill_empty_width.el");
qa!(qa_string_reverse_sequences, "qa_string_reverse_sequences.el");
qa!(qa_string_lines_properties, "qa_string_lines_properties.el");
qa!(qa_string_lines_byte_kind, "qa_string_lines_byte_kind.el");
qa!(qa_string_lines_bool_vector_error, "qa_string_lines_bool_vector_error.el");
qa!(qa_string_lines_empty_vector, "qa_string_lines_empty_vector.el");
qa!(qa_string_lines_nil, "qa_string_lines_nil.el");
qa!(qa_string_byte_conversion_identity, "qa_string_byte_conversion_identity.el");
qa!(qa_string_fill_trailing_newlines, "qa_string_fill_trailing_newlines.el");
qa!(qa_string_fill_tab_width, "qa_string_fill_tab_width.el");
qa!(qa_string_fill_sentence_spacing, "qa_string_fill_sentence_spacing.el");
qa!(qa_string_version_nil, "qa_string_version_nil.el");
qa!(qa_string_version_file_order, "qa_string_version_file_order.el");
qa!(qa_byte_to_string_range, "qa_byte_to_string_range.el");
qa!(qa_byte_to_string_byte_kind, "qa_byte_to_string_byte_kind.el");
qa!(qa_string_as_multibyte_extended_utf8, "qa_string_as_multibyte_extended_utf8.el");
qa!(qa_truncate_width_float_start, "qa_truncate_width_float_start.el");
qa!(qa_truncate_width_head_padding, "qa_truncate_width_head_padding.el");
qa!(qa_case_table_fold, "qa_case_table_fold.el");
qa!(qa_byte_character_conversion, "qa_byte_character_conversion.el");
qa!(qa_bytecode_errors, "qa_bytecode_errors.el");
qa!(qa_bytecode_mutation, "qa_bytecode_mutation.el");
qa!(qa_bytecode_slots, "qa_bytecode_slots.el");
qa!(qa_dump_history, "qa_dump_history.el");
qa!(qa_load_elc_header, "qa_load_elc_header.el");
qa!(qa_strings2, "qa_strings2.el");
qa!(qa_subrinfo, "qa_subrinfo.el");
qa!(qa_subrx, "qa_subrx.el");
qa!(qa_subrx2, "qa_subrx2.el");
qa!(qa_subrx3, "qa_subrx3.el");
qa!(qa_symbol, "qa_symbol.el");
qa!(qa_variable_watchers, "qa_variable_watchers.el");
qa!(qa_syntax, "qa_syntax.el");
qa!(qa_syntax_lookup, "qa_syntax_lookup.el");
qa!(qa_syntax_class_bounds, "qa_syntax_class_bounds.el");
qa!(qa_syntax2, "qa_syntax2.el");
qa!(qa_syntaxtable, "qa_syntaxtable.el");
qa!(qa_system, "qa_system.el");
qa!(qa_build_identity, "qa_build_identity.el");
qa!(qa_tablist, "qa_tablist.el");
qa!(qa_textprops, "qa_textprops.el");
qa!(qa_object_intervals, "qa_object_intervals.el");
qa!(
    qa_property_argument_checks,
    "qa_property_argument_checks.el"
);
qa!(qa_thing, "qa_thing.el");
qa!(qa_thing2, "qa_thing2.el");
qa!(qa_diffface, "qa_diffface.el");
qa!(qa_time, "qa_time.el");
qa!(qa_timefmt, "qa_timefmt.el");
qa!(qa_timer, "qa_timer.el");
qa!(qa_types2, "qa_types2.el");
qa!(qa_ucs, "qa_ucs.el");
qa!(qa_japanese, "qa_japanese.el");
qa!(qa_undo, "qa_undo.el");
qa!(qa_undo_properties, "qa_undo_properties.el");
qa!(qa_propertize_bytes, "qa_propertize_bytes.el");
qa!(qa_vector, "qa_vector.el");
qa!(qa_fillarray_strings, "qa_fillarray_strings.el");
qa!(qa_clear_string, "qa_clear_string.el");
qa!(qa_reverse_strings, "qa_reverse_strings.el");
qa!(qa_string_width_tabs, "qa_string_width_tabs.el");
qa!(qa_string_glyph_split, "qa_string_glyph_split.el");
qa!(qa_window, "qa_window.el");
qa!(qa_window_configuration, "qa_window_configuration.el");
qa!(qa_window_scroll_hooks, "qa_window_scroll_hooks.el");
qa!(qa_winstate, "qa_winstate.el");
qa!(qa_window_state_minimum, "qa_window_state_minimum.el");
qa!(qa_xref, "qa_xref.el");

qa!(qa_buffer_name_validation, "qa_buffer_name_validation.el");

qa!(qa_window_dedication, "qa_window_dedication.el");

qa!(qa_overlay_type, "qa_overlay_type.el");
qa!(qa_overlay_lifetime, "qa_overlay_lifetime.el");
qa!(qa_overlay_arguments, "qa_overlay_arguments.el");
qa!(qa_overlay_priority, "qa_overlay_priority.el");
qa!(qa_overlay_evaporation, "qa_overlay_evaporation.el");
qa!(qa_overlay_target_buffer, "qa_overlay_target_buffer.el");
qa!(qa_overlay_category, "qa_overlay_category.el");
qa!(qa_overlay_window, "qa_overlay_window.el");
qa!(qa_indent_inheritance, "qa_indent_inheritance.el");
qa!(qa_insert_primitive_inherit, "qa_insert_primitive_inherit.el");
qa!(qa_newline_inheritance, "qa_newline_inheritance.el");
qa!(qa_insert_char_validation, "qa_insert_char_validation.el");
qa!(qa_empty_readonly, "qa_empty_readonly.el");
qa!(qa_completion_function_table, "qa_completion_function_table.el");
qa!(qa_completion_regexp_errors, "qa_completion_regexp_errors.el");
qa!(qa_completion_string_flags, "qa_completion_string_flags.el");
qa!(qa_delete_killed_target, "qa_delete_killed_target.el");
qa!(qa_delete_relocation, "qa_delete_relocation.el");
qa!(qa_extract_metadata, "qa_extract_metadata.el");
qa!(qa_temp_private_modes, "qa_temp_private_modes.el");
qa!(qa_temp_text_bytes, "qa_temp_text_bytes.el");
qa!(qa_file_handler_metadata, "qa_file_handler_metadata.el");
qa!(qa_set_file_times_flags, "qa_set_file_times_flags.el");
qa!(qa_temp_directory, "qa_temp_directory.el");
qa!(qa_temp_nonstring_text, "qa_temp_nonstring_text.el");
qa!(qa_temp_write_modes, "qa_temp_write_modes.el");
qa!(qa_change_major_mode_hook, "qa_change_major_mode_hook.el");
qa!(qa_permanent_local_hooks, "qa_permanent_local_hooks.el");
qa!(qa_erase_buffer_widen, "qa_erase_buffer_widen.el");
qa!(qa_local_dynamic_unwind, "qa_local_dynamic_unwind.el");
qa!(qa_handler_dynamic_order, "qa_handler_dynamic_order.el");
qa!(qa_signal_objects, "qa_signal_objects.el");
qa!(qa_vm_unwind, "qa_vm_unwind.el");
qa!(qa_unwind_legacy, "qa_unwind_legacy.el");
qa!(qa_progn_traversal, "qa_progn_traversal.el");
qa!(qa_signal_targets, "qa_signal_targets.el");
#[test]
fn qa_signal_native() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native signal tests: no C compiler available");
        return;
    }
    run("qa_signal_native.el");
}
#[test]
fn qa_native_comp_load() {
    if ev("(native-comp-available-p)") != "t" {
        eprintln!("skipping native compiler load tests: no C compiler available");
        return;
    }
    run("qa_native_comp_load.el");
}

#[test]
fn qa_native_call_depth() {
    if ev("(native-comp-available-p)") != "t" {
        return;
    }
    run("qa_native_call_depth.el");
}

#[test]
fn qa_native_bindings() {
    if ev("(native-comp-available-p)") != "t" {
        return;
    }
    run("qa_native_bindings.el");
}
qa!(qa_called_interactively_frames, "qa_called_interactively_frames.el");
qa!(qa_map_live_sequence, "qa_map_live_sequence.el");
qa!(qa_lisp_mode_init, "qa_lisp_mode_init.el");
qa!(qa_lisp_mode_reload, "qa_lisp_mode_reload.el");
qa!(qa_org_deferred, "qa_org_deferred.el");
qa!(qa_org_mode_startup, "qa_org_mode_startup.el");
qa!(qa_closure_dotted_args, "qa_closure_dotted_args.el");
qa!(qa_sqlite_directory, "qa_sqlite_directory.el");
qa!(qa_sqlite_text, "qa_sqlite_text.el");
qa!(qa_shortdoc_data, "qa_shortdoc_data.el");
qa!(qa_shortdoc_source_first, "qa_shortdoc_source_first.el");

qa!(qa_gensym_counter, "qa_gensym_counter.el");
qa!(qa_gensym_semantics, "qa_gensym_semantics.el");
qa!(qa_format_string_kind, "qa_format_string_kind.el");

qa!(qa_delete_char_bounds, "qa_delete_char_bounds.el");

qa!(qa_time_spec_validation, "qa_time_spec_validation.el");

qa!(qa_time_zero_zone, "qa_time_zero_zone.el");
qa!(qa_time_modifiers, "qa_time_modifiers.el");

qa!(qa_thread_bookkeeping, "qa_thread_bookkeeping.el");
qa!(qa_thread_scheduling, "qa_thread_scheduling.el");
qa!(qa_thread_isolation, "qa_thread_isolation.el");
qa!(qa_mutex_ownership, "qa_mutex_ownership.el");

qa!(qa_file_replace, "qa_file_replace.el");

qa!(qa_split_window_limits, "qa_split_window_limits.el");

qa!(qa_buffer_string_flags, "qa_buffer_string_flags.el");

qa!(qa_unreadable_callable, "qa_unreadable_callable.el");

// Remacs-only logical frame lifecycle; GNU batch cannot create extra frames.
#[test]
fn qa_buffer_history_frames() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let mut i = remacs::lisp::Interp::new();
        i.noninteractive = false;
        let source = r#"
(progn
 (setq qf-one (selected-frame)
       qf-a (get-buffer-create "qf-a")
       qf-b (get-buffer-create "qf-b"))
 (switch-to-buffer qf-a)
 (setq qf-two (make-frame) qf-two-window (frame-selected-window qf-two))
 (select-frame qf-two t)
 (switch-to-buffer qf-b)
 (unless (eq (car (frame-parameter qf-two 'buffer-list)) qf-b) (error "second history"))
 (unless (eq (car (frame-parameter qf-one 'buffer-list)) qf-a) (error "first history"))
 (select-frame qf-one t)
 (bury-buffer qf-b)
 (unless (eq (car (frame-parameter qf-one 'buried-buffer-list)) qf-b) (error "buried first"))
 (unless (eq (car (frame-parameter qf-two 'buffer-list)) qf-b) (error "burial leaked"))
 (kill-buffer qf-b)
 (when (memq qf-b (frame-parameter qf-one 'buried-buffer-list)) (error "kill buried cleanup"))
 (when (memq qf-b (frame-parameter qf-two 'buffer-list)) (error "kill visited cleanup"))
 (delete-frame qf-two)
 (when (frame-live-p qf-two) (error "dead frame live"))
 (when (window-live-p qf-two-window) (error "dead frame window live"))
 (when (frame-parameter qf-two 'buffer-list) (error "dead frame history"))
 (unless (condition-case nil (progn (other-buffer nil nil qf-two) nil) (wrong-type-argument t)) (error "dead frame accepted"))
 (setq qf-two (make-frame))
 (select-frame qf-two)
 (delete-frame qf-two)
 (unless (eq (selected-frame) qf-one) (error "selected deletion replacement"))
 (unless (eq (current-buffer) (window-buffer)) (error "selected deletion buffer"))
 (unless (condition-case nil (progn (delete-frame qf-one) nil) (error t)) (error "last frame deleted"))
 'ok)
"#;
        let result = i.eval_str(source);
        match result {
            Ok(v) => assert_eq!(i.prin1_to_string(&v), "ok"),
            Err(_) => panic!("frame lifecycle assertion failed"),
        }
    }).unwrap().join().unwrap();
}

qa!(qa_elisp_mode_name, "qa_elisp_mode_name.el");

qa!(qa_unibyte_bounds, "qa_unibyte_bounds.el");

qa!(qa_sqlite_blob, "qa_sqlite_blob.el");
qa!(qa_sqlite_parameters, "qa_sqlite_parameters.el");
qa!(qa_sqlite_lazy_sets, "qa_sqlite_lazy_sets.el");
qa!(qa_sqlite_errors, "qa_sqlite_errors.el");
qa!(qa_sqlite_validation, "qa_sqlite_validation.el");

qa!(qa_process_large_send, "qa_process_large_send.el");

#[test]
fn qa_output_stream_state() {
    on_big_stack(|| {
        let (mut i, output) = interp();
        let streams = [
            "(let ((b (generate-new-buffer \" *qa-dead-output*\"))) (kill-buffer b) b)",
            "(make-marker)",
            "(let* ((b (generate-new-buffer \" *qa-dead-marker*\")) (m (with-current-buffer b (point-marker)))) (kill-buffer b) m)",
        ];
        for source in streams {
            let stream = i
                .eval_str(source)
                .unwrap_or_else(|_| panic!("stream fixture failed"));
            for capture in [false, true] {
                i.capture_output = capture;
                i.output_buffer = "unchanged\n".into();
                i.out_last_char = Some('\n');
                let old_output = output.borrow().clone();
                assert!(i.write_output_to("payload", &stream).is_err());
                assert_eq!(i.output_buffer, "unchanged\n");
                assert_eq!(i.out_last_char, Some('\n'));
                assert_eq!(*output.borrow(), old_output);
                let terpri = i.intern("terpri");
                assert!(
                    i.apply(
                        &remacs::lisp::Value::Sym(terpri),
                        vec![stream.clone(), remacs::lisp::Value::t()]
                    )
                    .is_err()
                );
                assert_eq!(i.output_buffer, "unchanged\n");
                assert_eq!(i.out_last_char, Some('\n'));
                assert_eq!(*output.borrow(), old_output);
                i.capture_output = false;
            }
        }
    });
}

qa!(qa_xml_normalization, "qa_xml_normalization.el");

qa!(qa_xml_namespace_attrs, "qa_xml_namespace_attrs.el");

qa!(qa_xml_well_formed, "qa_xml_well_formed.el");

qa!(qa_xml_dtd_entities, "qa_xml_dtd_entities.el");

qa!(qa_html_comments, "qa_html_comments.el");

qa!(qa_html_head, "qa_html_head.el");

qa!(qa_html_tbody, "qa_html_tbody.el");

qa!(qa_html_repair, "qa_html_repair.el");
