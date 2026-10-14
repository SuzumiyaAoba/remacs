//! Buffer primitives: the DEFUNs operating on buffers, point, mark,
//! markers, narrowing, buffer-local variables, search/match, text
//! properties, and undo.
//!
//! Position convention: Emacs positions are 1-based; internally the
//! `Buffer` uses 0-based char indices. `pt` = `bb.point + 1`.

use crate::buffer::{Buffer, TextProp, file_truename, lock_file_name, lock_owner_string};
use crate::lisp::Interp;
use crate::lisp::builtins::{want_int_or_mark, want_string};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::eval::MatchData;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, SymId, Value};
use std::cell::RefCell;
use std::rc::Rc;

macro_rules! S {
    ($name:literal, $min:expr, $max:expr, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Range {
                min: $min,
                max: $max,
            },
            func: $f,
            doc: $doc,
        }
    };
    ($name:literal, many $min:expr, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Many { min: $min },
            func: $f,
            doc: $doc,
        }
    };
    ($name:literal, raw, $f:expr, $doc:literal) => {
        crate::lisp::value::Subr {
            name: $name,
            arity: crate::lisp::value::Arity::Unevalled,
            func: $f,
            doc: $doc,
        }
    };
}

pub(crate) static SUBRS: &[Subr] = &[
    // --- buffer objects ---
    S!(
        "current-buffer",
        0,
        0,
        f_current_buffer,
        "Return the current buffer."
    ),
    S!("bufferp", 1, 1, f_bufferp, "t if OBJECT is a live buffer."),
    S!(
        "buffer-name",
        0,
        1,
        f_buffer_name,
        "Return the name of BUFFER."
    ),
    S!(
        "get-buffer",
        1,
        1,
        f_get_buffer,
        "Return buffer named NAME."
    ),
    S!(
        "get-buffer-create",
        1,
        2,
        f_get_buffer_create,
        "Return or create buffer NAME."
    ),
    S!(
        "generate-new-buffer",
        1,
        2,
        f_generate_new_buffer,
        "Create a fresh buffer."
    ),
    S!(
        "generate-new-buffer-name",
        1,
        2,
        f_generate_new_buffer_name,
        "Unique buffer name."
    ),
    S!(
        "buffer-live-p",
        1,
        1,
        f_buffer_live_p,
        "t if OBJECT is a live buffer."
    ),
    S!("kill-buffer", 0, 1, f_kill_buffer, "Kill BUFFER."),
    S!("buffer-list", 0, 1, f_buffer_list, "List of live buffers."),
    S!(
        "other-buffer",
        0,
        3,
        f_other_buffer,
        "Return another buffer."
    ),
    S!("set-buffer", 1, 1, f_set_buffer, "Make BUFFER current."),
    S!(
        "rename-buffer",
        1,
        2,
        f_rename_buffer,
        "Rename current buffer."
    ),
    S!(
        "bury-buffer",
        0,
        1,
        f_bury_buffer,
        "Move BUFFER to the end of the list."
    ),
    S!(
        "bury-buffer-internal",
        1,
        1,
        f_bury_buffer_internal,
        "Move BUFFER-OR-NAME to the end of the buffer list."
    ),
    S!(
        "unbury-buffer",
        0,
        0,
        f_unbury_buffer,
        "Last buffer in the list."
    ),
    S!(
        "buffer-size",
        0,
        1,
        f_buffer_size,
        "Number of chars in BUFFER."
    ),
    S!(
        "buffer-modified-p",
        0,
        1,
        f_buffer_modified_p,
        "t if BUFFER was modified."
    ),
    S!(
        "set-buffer-modified-p",
        1,
        1,
        f_set_buffer_modified_p,
        "Set modified flag."
    ),
    S!(
        "not-modified",
        0,
        1,
        f_not_modified,
        "Mark current buffer unmodified; with ARG mark it modified."
    ),
    S!(
        "buffer-file-name",
        0,
        1,
        f_buffer_file_name,
        "Visited file name."
    ),
    S!(
        "buffer-base-buffer",
        0,
        1,
        f_buffer_base_buffer,
        "Base buffer (nil)."
    ),
    S!(
        "make-indirect-buffer",
        2,
        4,
        f_make_indirect_buffer,
        "Create an indirect buffer sharing BASE-BUFFER's text."
    ),
    S!(
        "clone-indirect-buffer",
        2,
        3,
        f_clone_indirect_buffer,
        "Create an indirect buffer cloning the current buffer."
    ),
    S!(
        "clone-buffer",
        0,
        2,
        f_clone_buffer,
        "Create a clone of the current buffer."
    ),
    S!(
        "buffer-local-variables",
        0,
        1,
        f_buffer_local_variables,
        "Alist of local vars."
    ),
    S!(
        "buffer-local-value",
        2,
        2,
        f_buffer_local_value,
        "Value of SYMBOL in BUFFER."
    ),
    S!(
        "set-buffer-major-mode",
        1,
        1,
        f_set_buffer_major_mode,
        "Set an appropriate major mode for BUFFER.\nFor the *scratch* buffer, use `initial-major-mode', otherwise choose a mode\naccording to the default value of `major-mode'.\nUse this function before selecting the buffer, since it may need to inspect\nthe current buffer's major mode."
    ),
    S!(
        "buffer-local-boundp",
        2,
        2,
        f_buffer_local_boundp,
        "t if SYMBOL has a local binding in BUFFER."
    ),
    S!(
        "make-local-variable",
        1,
        1,
        f_make_local_variable,
        "Make VARIABLE buffer-local."
    ),
    S!(
        "make-variable-buffer-local",
        1,
        1,
        f_make_variable_buffer_local,
        "Make VARIABLE always local."
    ),
    S!(
        "kill-local-variable",
        1,
        1,
        f_kill_local_variable,
        "Remove local binding."
    ),
    S!(
        "kill-all-local-variables",
        0,
        1,
        f_kill_all_local_variables,
        "Remove all local bindings."
    ),
    S!(
        "local-variable-p",
        1,
        2,
        f_local_variable_p,
        "t if VARIABLE is local in BUFFER."
    ),
    S!(
        "local-variable-if-set-p",
        1,
        2,
        f_local_variable_if_set_p,
        "t if VARIABLE is auto-local."
    ),
    S!(
        "default-value",
        1,
        1,
        f_default_value,
        "Default value of SYMBOL."
    ),
    S!(
        "current-case-table",
        0,
        0,
        f_current_case_table,
        "Case table of the current buffer."
    ),
    S!(
        "standard-case-table",
        0,
        0,
        f_standard_case_table,
        "The standard case table."
    ),
    S!(
        "case-table-p",
        1,
        1,
        f_case_table_p,
        "t if OBJECT is a case table."
    ),
    S!(
        "copy-case-table",
        1,
        1,
        f_copy_case_table,
        "Return a new case table that is a copy of CASE-TABLE.\nIt copies the case-table itself and each of its extra-slot tables."
    ),
    S!(
        "set-case-table",
        1,
        1,
        f_set_case_table,
        "Set the current buffer's case table."
    ),
    S!(
        "set-standard-case-table",
        1,
        1,
        f_set_standard_case_table,
        "Set the standard case table."
    ),
    S!(
        "set-default",
        2,
        2,
        f_set_default,
        "Set default value of SYMBOL."
    ),
    S!(
        "set-default-toplevel-value",
        2,
        2,
        f_set_default_toplevel,
        "Set default value of SYMBOL, return nil (GNU)."
    ),
    S!(
        "default-boundp",
        1,
        1,
        f_default_boundp,
        "t if SYMBOL has a default value."
    ),
    S!(
        "buffer-disable-undo",
        0,
        1,
        f_buffer_disable_undo,
        "Stop recording undo."
    ),
    S!(
        "buffer-enable-undo",
        0,
        1,
        f_buffer_enable_undo,
        "Start recording undo."
    ),
    // --- point & motion ---
    S!("point", 0, 0, f_point, "Current point (1-based)."),
    S!(
        "point-min",
        0,
        0,
        f_point_min,
        "Minimum accessible position."
    ),
    S!(
        "point-max",
        0,
        0,
        f_point_max,
        "Maximum accessible position."
    ),
    S!("goto-char", 1, 1, f_goto_char, "Move point to POSITION."),
    S!(
        "forward-char",
        0,
        1,
        f_forward_char,
        "Move point N chars forward."
    ),
    S!(
        "backward-char",
        0,
        1,
        f_backward_char,
        "Move point N chars backward."
    ),
    S!(
        "forward-word",
        0,
        1,
        f_forward_word,
        "Move point N words forward."
    ),
    S!(
        "backward-word",
        0,
        1,
        f_backward_word,
        "Move point N words backward."
    ),
    S!(
        "forward-line",
        0,
        1,
        f_forward_line,
        "Move point N lines forward."
    ),
    S!(
        "beginning-of-line",
        0,
        1,
        f_beginning_of_line,
        "Move to start of line."
    ),
    S!("end-of-line", 0, 1, f_end_of_line, "Move to end of line."),
    S!("bobp", 0, 0, f_bobp, "t at beginning of accessible text."),
    S!("eobp", 0, 0, f_eobp, "t at end of accessible text."),
    S!("bolp", 0, 0, f_bolp, "t at beginning of line."),
    S!("eolp", 0, 0, f_eolp, "t at end of line."),
    S!("point-marker", 0, 0, f_point_marker, "Marker at point."),
    S!(
        "point-min-marker",
        0,
        0,
        f_point_min_marker,
        "Marker at point-min."
    ),
    S!(
        "point-max-marker",
        0,
        0,
        f_point_max_marker,
        "Marker at point-max."
    ),
    S!("char-after", 0, 1, f_char_after, "Char at POSITION."),
    S!("char-before", 0, 1, f_char_before, "Char before POSITION."),
    S!(
        "following-char",
        0,
        0,
        f_following_char,
        "Char after point."
    ),
    S!(
        "preceding-char",
        0,
        0,
        f_preceding_char,
        "Char before point."
    ),
    S!("pos-bol", 0, 1, f_pos_bol, "Line start of POSITION."),
    S!("pos-eol", 0, 1, f_pos_eol, "Line end of POSITION."),
    S!(
        "line-beginning-position",
        0,
        1,
        f_line_beginning_position,
        "Start of Nth line."
    ),
    S!(
        "line-end-position",
        0,
        1,
        f_line_end_position,
        "End of Nth line."
    ),
    S!(
        "point-at-bol",
        0,
        1,
        f_line_beginning_position,
        "Start of Nth line."
    ),
    S!(
        "point-at-eol",
        0,
        1,
        f_line_end_position,
        "End of Nth line."
    ),
    S!(
        "line-number-at-pos",
        0,
        2,
        f_line_number_at_pos,
        "Line number of POSITION."
    ),
    S!(
        "count-lines",
        2,
        3,
        f_count_lines,
        "Lines between START and END."
    ),
    S!("current-column", 0, 0, f_current_column, "Column of point."),
    S!(
        "move-to-column",
        1,
        2,
        f_move_to_column,
        "Move to COLUMN on this line."
    ),
    S!(
        "forward-comment",
        1,
        1,
        f_forward_comment,
        "Skip comments (approx: whitespace)."
    ),
    S!(
        "skip-chars-forward",
        1,
        2,
        f_skip_chars_forward,
        "Skip chars in SET."
    ),
    S!(
        "skip-chars-backward",
        1,
        2,
        f_skip_chars_backward,
        "Skip chars in SET backward."
    ),
    S!(
        "skip-syntax-forward",
        1,
        2,
        f_skip_syntax_forward,
        "Skip chars of syntax classes."
    ),
    S!(
        "skip-syntax-backward",
        1,
        2,
        f_skip_syntax_backward,
        "Backward syntax skip."
    ),
    // `forward-sexp', `backward-sexp', `forward-list', `backward-list',
    // `down-list', `up-list' and `backward-up-list' are Lisp-level
    // functions in GNU (lisp.el); our prelude defines them on top of
    // the `scan-lists'/`scan-sexps' subrs below.
    S!("scan-lists", 3, 3, f_scan_lists, "Scan lists."),
    S!(
        "scan-sexps",
        2,
        2,
        f_scan_sexps,
        "Scan COUNT sexps from FROM."
    ),
    S!(
        "looking-back",
        1,
        3,
        f_looking_back,
        "Match regexp before point."
    ),
    S!("last-buffer", 0, 3, f_last_buffer, "Last buffer in order."),
    // --- insertion & deletion ---
    S!("insert", many 0, f_insert, "Insert args (strings/chars) at point."),
    S!("insert-and-inherit", many 0, f_insert_and_inherit, "Insert with inherited props."),
    S!("insert-before-markers", many 0, f_insert_before_markers, "Insert before markers."),
    S!("insert-before-markers-and-inherit", many 0, f_insert_before_markers_and_inherit, ""),
    S!(
        "insert-char",
        1,
        3,
        f_insert_char,
        "Insert CHAR COUNT times."
    ),
    S!(
        "insert-buffer-substring",
        1,
        3,
        f_insert_buffer_substring,
        "Insert text from BUFFER."
    ),
    S!(
        "self-insert-command",
        1,
        2,
        f_self_insert_command,
        "Insert the last typed char N times."
    ),
    S!("newline", 0, 2, f_newline, "Insert a newline."),
    S!(
        "open-line",
        1,
        1,
        f_open_line,
        "Insert newline without moving point."
    ),
    S!(
        "delete-char",
        1,
        2,
        f_delete_char,
        "Delete N chars after point."
    ),
    S!(
        "delete-backward-char",
        1,
        2,
        f_delete_backward_char,
        "Delete N chars before point."
    ),
    // backward-delete-char-untabify is Lisp (prelude): GNU's
    // untabify-method dispatch needs insert-char/current-column.
    S!(
        "beginning-of-visual-line",
        0,
        1,
        f_beginning_of_visual_line,
        "Move to visual beginning of line."
    ),
    S!(
        "end-of-visual-line",
        0,
        1,
        f_end_of_visual_line,
        "Move to visual end of line."
    ),
    S!(
        "forward-visible-line",
        1,
        1,
        f_forward_visible_line,
        "Move N visible lines forward."
    ),
    S!(
        "delete-and-extract-region",
        2,
        2,
        f_delete_and_extract_region,
        "Delete region, return text."
    ),
    S!(
        "delete-region",
        2,
        2,
        f_delete_region,
        "Delete text between START and END."
    ),
    S!("erase-buffer", 0, 0, f_erase_buffer, "Delete all text."),
    S!(
        "kill-region",
        2,
        3,
        f_kill_region,
        "Kill text between START and END."
    ),
    S!(
        "append-next-kill",
        0,
        1,
        f_append_next_kill,
        "Make the next kill append to the last kill-ring entry."
    ),
    S!(
        "delete-blank-lines",
        0,
        0,
        f_delete_blank_lines,
        "Delete blank lines around point."
    ),
    S!("combine-after-change-calls", raw, f_progn_raw, ""),
    S!("combine-change-calls", raw, f_second_form_raw, ""),
    // --- buffer text access ---
    S!(
        "filter-buffer-substring",
        2,
        3,
        f_filter_buffer_substring,
        "buffer-substring with optional deletion + filter."
    ),
    S!(
        "buffer-substring",
        2,
        2,
        f_buffer_substring,
        "Text between START and END."
    ),
    S!(
        "buffer-substring-no-properties",
        2,
        2,
        f_buffer_substring_no_properties,
        "Text without props."
    ),
    S!(
        "buffer-substring-with-properties",
        2,
        2,
        f_buffer_substring,
        "Text between START and END, with properties."
    ),
    S!(
        "buffer-substring-with-bidi-context",
        2,
        3,
        f_buffer_substring_with_bidi_context,
        "Text between START and END."
    ),
    S!(
        "buffer-string",
        0,
        0,
        f_buffer_string,
        "Whole accessible text."
    ),
    S!("buffer-word-at-point", 0, 0, f_word_at_point, ""),
    S!("current-word", 0, 2, f_current_word, "Word at point."),
    // `thing-at-point', `bounds-of-thing-at-point', `symbol-at-point'
    // and `word-at-point' live in thingatpt.el (autoloaded in GNU);
    // registered as autoloads in the prelude.
    // --- mark & region ---
    S!("mark", 0, 1, f_mark, "The mark position (or nil)."),
    S!("set-mark", 1, 1, f_set_mark, "Set the mark to POSITION."),
    S!("mark-marker", 0, 0, f_mark_marker, "Marker at the mark."),
    S!("push-mark", 0, 3, f_push_mark, "Push mark onto mark-ring."),
    S!("pop-mark", 0, 0, f_pop_mark, "Pop mark-ring."),
    S!(
        "region-beginning",
        0,
        0,
        f_region_beginning,
        "Start of region."
    ),
    S!("region-end", 0, 0, f_region_end, "End of region."),
    S!(
        "region-active-p",
        0,
        0,
        f_region_active_p,
        "t if mark is active."
    ),
    S!(
        "deactivate-mark",
        0,
        1,
        f_deactivate_mark,
        "Deactivate the mark."
    ),
    S!(
        "set-register",
        2,
        2,
        f_set_register,
        "Set register REGISTER to VALUE."
    ),
    S!(
        "get-register",
        1,
        1,
        f_get_register,
        "Return the value of register REGISTER."
    ),
    S!(
        "number-to-register",
        2,
        2,
        f_number_to_register,
        "Store NUMBER in register REGISTER."
    ),
    S!(
        "increment-register",
        2,
        2,
        f_increment_register,
        "Add NUMBER to register REGISTER."
    ),
    S!(
        "point-to-register",
        1,
        2,
        f_point_to_register,
        "Store the location of point in register REGISTER."
    ),
    S!(
        "jump-to-register",
        1,
        2,
        f_jump_to_register,
        "Move point to the position stored in register REGISTER."
    ),
    S!(
        "insert-register",
        1,
        2,
        f_insert_register,
        "Insert the contents of register REGISTER."
    ),
    S!(
        "copy-to-register",
        3,
        5,
        f_copy_to_register,
        "Copy region into register REGISTER."
    ),
    S!(
        "window-configuration-to-register",
        1,
        2,
        f_window_configuration_to_register,
        "Store the window configuration in register REGISTER."
    ),
    S!(
        "frame-configuration-to-register",
        1,
        2,
        f_frame_configuration_to_register,
        "Store the frame configuration in register REGISTER."
    ),
    S!(
        "kill-rectangle",
        2,
        3,
        f_kill_rectangle,
        "Delete the region-rectangle, saving it as the last killed one."
    ),
    S!(
        "delete-rectangle",
        2,
        3,
        f_delete_rectangle,
        "Delete the text in the region-rectangle."
    ),
    S!(
        "clear-rectangle",
        2,
        3,
        f_clear_rectangle,
        "Blank out the region-rectangle."
    ),
    S!(
        "open-rectangle",
        2,
        3,
        f_open_rectangle,
        "Blank out the region-rectangle, shifting text right."
    ),
    S!(
        "string-rectangle",
        3,
        3,
        f_string_rectangle,
        "Replace rectangle contents with STRING on each line."
    ),
    S!(
        "string-insert-rectangle",
        3,
        3,
        f_string_insert_rectangle,
        "Insert STRING on each line of region-rectangle."
    ),
    S!(
        "copy-rectangle-as-kill",
        2,
        2,
        f_copy_rectangle_as_kill,
        "Copy the region-rectangle as the last killed one."
    ),
    S!(
        "extract-rectangle",
        2,
        2,
        f_extract_rectangle,
        "Return the contents of the rectangle as a list of strings."
    ),
    S!(
        "delete-extract-rectangle",
        2,
        3,
        f_delete_extract_rectangle,
        "Delete the rectangle, returning its contents as a list."
    ),
    S!(
        "yank-rectangle",
        0,
        0,
        f_yank_rectangle,
        "Yank the last killed rectangle at point."
    ),
    S!(
        "insert-rectangle",
        1,
        1,
        f_insert_rectangle,
        "Insert RECTANGLE (list of strings) with upper left corner at point."
    ),
    S!(
        "rectangle-number-lines",
        3,
        4,
        f_rectangle_number_lines,
        "Insert numbers in front of the region-rectangle."
    ),
    S!(
        "delete-whitespace-rectangle",
        2,
        3,
        f_delete_whitespace_rectangle,
        "Delete all whitespace following a column in each line."
    ),
    S!(
        "close-rectangle",
        2,
        3,
        f_delete_whitespace_rectangle,
        "Obsolete alias for `delete-whitespace-rectangle'."
    ),
    S!(
        "replace-rectangle",
        3,
        3,
        f_string_rectangle,
        "Obsolete alias for `string-rectangle'."
    ),
    S!(
        "spaces-string",
        1,
        1,
        f_spaces_string,
        "Return a string of N spaces."
    ),
    S!(
        "rectangle-dimensions",
        2,
        2,
        f_rectangle_dimensions,
        "Return (WIDTH . HEIGHT) of the rectangle with corners START END."
    ),
    S!(
        "rectangle-position-as-coordinates",
        1,
        1,
        f_rectangle_position_as_coordinates,
        "Return (COLUMN . LINE) of POSITION."
    ),
    S!(
        "rectangle-intersect-p",
        4,
        4,
        f_rectangle_intersect_p,
        "Return non-nil if two rectangles intersect."
    ),
    S!(
        "extract-rectangle-bounds",
        2,
        2,
        f_extract_rectangle_bounds,
        "Return (START . END) bounds for each line of the rectangle."
    ),
    S!(
        "apply-on-rectangle",
        many 3,
        f_apply_on_rectangle,
        "Call FUNCTION for each line of rectangle START..END."
    ),
    S!(
        "operate-on-rectangle",
        4,
        4,
        f_operate_on_rectangle,
        "Call FUNCTION for each line segment of rectangle START..END."
    ),
    S!("activate-mark", 0, 1, f_activate_mark, "Activate the mark."),
    S!(
        "exchange-point-and-mark",
        0,
        1,
        f_exchange_point_and_mark,
        "Swap point and mark."
    ),
    S!(
        "use-region-p",
        0,
        0,
        f_use_region_p,
        "t if the region is active."
    ),
    // --- narrowing ---
    S!(
        "narrow-to-region",
        2,
        2,
        f_narrow_to_region,
        "Restrict editing to START..END."
    ),
    S!("widen", 0, 0, f_widen, "Remove narrowing."),
    S!(
        "internal--labeled-narrow-to-region",
        3,
        3,
        f_labeled_narrow_to_region,
        "Internal: labeled narrow."
    ),
    S!(
        "internal--labeled-widen",
        1,
        1,
        f_labeled_widen,
        "Internal: widen matching LABEL."
    ),
    S!(
        "internal--set-buffer-modified-tick",
        1,
        2,
        f_set_buffer_modified_tick,
        "Internal: set buffer tick."
    ),
    S!(
        "recent-auto-save-p",
        0,
        0,
        f_nil,
        "t if recently auto-saved."
    ),
    S!(
        "set-buffer-auto-saved",
        0,
        0,
        f_nil,
        "Mark buffer auto-saved."
    ),
    S!(
        "clear-buffer-auto-save-failure",
        0,
        0,
        f_nil,
        "Clear auto-save failure."
    ),
    // --- markers ---
    S!("markerp", 1, 1, f_markerp, "t if OBJECT is a marker."),
    S!(
        "make-marker",
        0,
        0,
        f_make_marker,
        "Create a marker pointing nowhere."
    ),
    S!("copy-marker", 0, 2, f_copy_marker, "Copy MARKER."),
    S!(
        "set-marker",
        2,
        3,
        f_set_marker,
        "Point MARKER at POSITION in BUFFER."
    ),
    S!(
        "marker-position",
        1,
        1,
        f_marker_position,
        "Position of MARKER."
    ),
    S!("marker-buffer", 1, 1, f_marker_buffer, "Buffer of MARKER."),
    S!(
        "marker-insertion-type",
        1,
        1,
        f_marker_insertion_type,
        "Insertion type of MARKER."
    ),
    S!(
        "set-marker-insertion-type",
        2,
        2,
        f_set_marker_insertion_type,
        "Set insertion type."
    ),
    S!("move-marker", 2, 3, f_set_marker, "Move MARKER."),
    // --- searching ---
    S!(
        "looking-at",
        1,
        2,
        f_looking_at,
        "t if text at point matches REGEXP."
    ),
    S!("looking-at-p", 1, 1, f_looking_at_p, "Predicate version."),
    S!(
        "string-match",
        2,
        4,
        f_string_match,
        "Match REGEXP in STRING."
    ),
    S!(
        "string-match-p",
        2,
        4,
        f_string_match_p,
        "Predicate version."
    ),
    S!(
        "re-search-forward",
        1,
        4,
        f_re_search_forward,
        "Regexp search forward."
    ),
    S!(
        "re-search-backward",
        1,
        4,
        f_re_search_backward,
        "Regexp search backward."
    ),
    S!(
        "search-forward",
        1,
        4,
        f_search_forward,
        "Literal search forward."
    ),
    S!(
        "search-backward",
        1,
        4,
        f_search_backward,
        "Literal search backward."
    ),
    // GNU subr.el: search-forward-regexp / search-backward-regexp are
    // `defalias' copies of re-search-* (same subr object) — done in
    // prelude so `symbol-function' of both is `eq'.
    S!(
        "match-beginning",
        1,
        1,
        f_match_beginning,
        "Start of match group N."
    ),
    S!("match-end", 1, 1, f_match_end, "End of match group N."),
    S!(
        "match-data",
        0,
        3,
        f_match_data,
        "Match registers as a list."
    ),
    S!(
        "set-match-data",
        1,
        2,
        f_set_match_data,
        "Set match registers."
    ),
    S!(
        "match-string",
        1,
        2,
        f_match_string,
        "Matched text of group N."
    ),
    S!("match-string-no-properties", 1, 2, f_match_string, ""),
    S!(
        "replace-match",
        1,
        5,
        f_replace_match,
        "Replace match with NEWTEXT."
    ),
    S!(
        "match-substitute-replacement",
        1,
        5,
        f_match_substitute_replacement,
        "Return NEWTEXT with \\&/\\N escapes substituted from match data."
    ),
    S!(
        "regexp-opt",
        1,
        2,
        f_regexp_opt,
        "Optimal regexp matching any of STRINGS."
    ),
    S!(
        "regexp-opt-depth",
        1,
        1,
        f_regexp_opt_depth,
        "Number of parenthesized groups in REGEXP."
    ),
    S!(
        "regexp-quote",
        1,
        1,
        f_regexp_quote,
        "Quote STRING for literal regexp match."
    ),
    S!("posix-looking-at", 1, 2, f_posix_looking_at, ""),
    S!("posix-string-match", 2, 4, f_posix_string_match, ""),
    S!("posix-search-forward", 1, 4, f_posix_search_forward, ""),
    S!("posix-search-backward", 1, 4, f_posix_search_backward, ""),
    S!("word-search-forward", 1, 4, f_word_search_forward, ""),
    S!("word-search-backward", 1, 4, f_word_search_backward, ""),
    S!(
        "word-search-forward-lax",
        1,
        4,
        f_word_search_forward_lax,
        ""
    ),
    S!(
        "word-search-backward-lax",
        1,
        4,
        f_word_search_backward_lax,
        ""
    ),
    // --- text properties ---
    S!(
        "put-text-property",
        4,
        5,
        f_put_text_property,
        "Set PROPERTY to VALUE in region."
    ),
    S!(
        "add-text-properties",
        3,
        4,
        f_add_text_properties,
        "Add plist props to region."
    ),
    S!(
        "remove-text-properties",
        3,
        4,
        f_remove_text_properties,
        "Remove props in region."
    ),
    S!(
        "set-text-properties",
        3,
        4,
        f_set_text_properties,
        "Set plist props in region."
    ),
    S!(
        "get-text-property",
        2,
        3,
        f_get_text_property,
        "Get PROPERTY at POSITION."
    ),
    S!(
        "text-properties-at",
        1,
        2,
        f_text_properties_at,
        "Plist at POSITION."
    ),
    S!("get-char-property", 2, 3, f_get_char_property, ""),
    S!("get-pos-property", 2, 3, f_get_pos_property, ""),
    S!(
        "remove-list-of-text-properties",
        3,
        4,
        f_remove_list_of_text_properties,
        ""
    ),
    S!("text-property-any", 4, 5, f_text_property_any, ""),
    S!("text-property-not-all", 4, 5, f_text_property_not_all, ""),
    S!("add-face-text-property", 3, 5, f_add_face_text_property, ""),
    S!(
        "next-property-change",
        1,
        3,
        f_next_property_change,
        "Next pos with different props."
    ),
    S!(
        "next-single-property-change",
        2,
        4,
        f_next_single_property_change,
        "Next pos where PROP changes."
    ),
    S!(
        "next-single-char-property-change",
        2,
        4,
        f_next_single_char_property_change,
        "Next pos where PROP changes (incl. overlays)."
    ),
    S!("previous-property-change", 1, 3, f_prev_property_change, ""),
    S!(
        "previous-single-property-change",
        2,
        4,
        f_prev_single_property_change,
        ""
    ),
    S!(
        "previous-single-char-property-change",
        2,
        4,
        f_prev_single_char_property_change,
        ""
    ),
    S!("propertize", many 1, f_propertize, "Return a copy of STRING with properties."),
    // `text-props-copy' does not exist in GNU.
    S!("object-intervals", 1, 1, f_object_intervals, ""),
    // --- undo ---
    S!(
        "primitive-undo",
        2,
        2,
        f_primitive_undo,
        "Apply undo entries."
    ),
    S!("undo-auto-amalgamate", 0, 0, f_noop, ""),
    S!("cancel-change-group", 0, 0, f_noop, ""),
    S!("activate-change-group", 0, 0, f_noop, ""),
    S!("accept-change-group", 1, 1, f_accept_change_group, ""),
    S!("handle-change-group", 0, 0, f_noop, ""),
    S!(
        "undo-outer-limit-truncate",
        1,
        1,
        f_undo_outer_limit_truncate,
        ""
    ),
    // --- gap/position misc ---
    S!(
        "gap-position",
        0,
        0,
        f_gap_position,
        "Gap position (internal)."
    ),
    S!("gap-size", 0, 0, f_gap_size, "Gap size (internal)."),
    S!(
        "position-bytes",
        1,
        1,
        f_position_bytes,
        "Byte position (chars == bytes here)."
    ),
    S!(
        "byte-to-position",
        1,
        1,
        f_byte_to_position,
        "Char position from byte."
    ),
    S!("max-char", 0, 1, f_max_char, "Max character code."),
    S!(
        "barf-if-buffer-read-only",
        0,
        1,
        f_barf_if_buffer_read_only,
        "Signal if read-only."
    ),
    S!(
        "verify-visited-file-modtime",
        0,
        1,
        f_verify_visited_file_modtime,
        "t if last mod time of BUF's visited file matches what BUF records."
    ),
    S!(
        "clear-visited-file-modtime",
        0,
        0,
        f_clear_visited_file_modtime,
        "Clear out records of last mod time of visited file."
    ),
    S!(
        "visited-file-modtime",
        0,
        0,
        f_visited_file_modtime,
        "Current buffer's recorded visited file modification time."
    ),
    S!(
        "set-visited-file-modtime",
        0,
        1,
        f_set_visited_file_modtime,
        "Update buffer's recorded mod time from visited file's time."
    ),
    S!(
        "lock-buffer",
        0,
        1,
        f_lock_buffer,
        "Lock FILE, if current buffer is modified."
    ),
    S!(
        "unlock-buffer",
        0,
        0,
        f_unlock_buffer,
        "Unlock the file visited in the current buffer."
    ),
    S!(
        "file-locked-p",
        1,
        1,
        f_file_locked_p,
        "Return lock status of FILE."
    ),
    S!(
        "lock-file",
        1,
        1,
        f_lock_file,
        "Lock FILE, if current buffer is modified."
    ),
    S!("unlock-file", 1, 1, f_unlock_file, "Unlock FILE."),
    S!("file-acl", 1, 1, f_file_acl, "Return ACL entries of FILE."),
    S!(
        "ask-user-about-lock",
        2,
        3,
        f_ask_user_about_lock,
        "Ask user what to do when one wants to edit a file that is locked."
    ),
    S!(
        "compare-buffer-substrings",
        6,
        6,
        f_compare_buffer_substrings,
        "Compare two buffer substrings."
    ),
];

fn f_nil(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// Stat mtime of PATH as nanoseconds since the epoch.
fn file_mtime_ns(path: &str) -> Option<(i128, i128)> {
    let m = std::fs::metadata(path).ok()?;
    let t = m.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some((
        d.as_secs() as i128 * 1_000_000_000 + d.subsec_nanos() as i128,
        m.len() as i128,
    ))
}

pub(crate) fn f_verify_visited_file_modtime(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = match a.first() {
        None => cur(i),
        Some(v) => buf_of(i, v)?,
    };
    let bb = b.borrow();
    // GNU: no visited file or never recorded (-2) → t.
    let path = match &bb.file_name {
        None => return Ok(Value::t()),
        Some(p) => p.clone(),
    };
    let rec = bb.file_modtime_ns;
    if rec == -2 {
        return Ok(Value::t());
    }
    match file_mtime_ns(&path) {
        // File missing: verified iff the recorded flag said missing (-1).
        None => Ok(Value::from_bool(rec < 0)),
        Some((ns, size)) => {
            if rec < 0 {
                return Ok(Value::Nil);
            }
            Ok(Value::from_bool(
                ns == rec && (bb.file_modtime_size < 0 || size == bb.file_modtime_size),
            ))
        }
    }
}

pub(crate) fn f_clear_visited_file_modtime(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.file_modtime_ns = -2;
    bb.file_modtime_size = -1;
    Ok(Value::Nil)
}

fn f_visited_file_modtime(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: negative ns is a flag — visited-file-modtime returns
    // UNKNOWN_MODTIME_NSECS - ns (0 when unknown, -1 when file missing).
    if bb.file_modtime_ns < 0 {
        return Ok(Value::Int(-2 - bb.file_modtime_ns));
    }
    Ok(crate::lisp::builtins::misc::ns_to_lisp_time_i(
        i,
        bb.file_modtime_ns,
    ))
}

fn f_set_visited_file_modtime(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match a.first() {
        Some(Value::Nil) | None => {
            // Stat the visited file and record its modtime+size.
            let b = cur(i);
            let path = {
                let bb = b.borrow();
                if bb.base_buffer.is_some() {
                    return Err(i.error("An indirect buffer does not have a visited file"));
                }
                match &bb.file_name {
                    Some(p) => p.clone(),
                    // GNU stats the nil filename → wrong-type-argument.
                    None => return Err(i.wrong_type_mut("stringp", &Value::Nil)),
                }
            };
            let mut bb = b.borrow_mut();
            match file_mtime_ns(&path) {
                Some((ns, size)) => {
                    bb.file_modtime_ns = ns;
                    bb.file_modtime_size = size;
                }
                // GNU: a file name that doesn’t stat records the
                // "file nonexistent" flag rather than signaling.
                None => {
                    bb.file_modtime_ns = -1;
                    bb.file_modtime_size = -1;
                }
            }
            Ok(Value::Nil)
        }
        Some(v) => {
            // Explicit value: fixnum flag (-1/0) or a Lisp timestamp.
            let ns = match v {
                Value::Int(n) => {
                    if *n != -1 && *n != 0 {
                        return Err(i.signal_data(
                            sym::ARGS_OUT_OF_RANGE,
                            vec![v.clone(), Value::Int(-1), Value::Int(0)],
                        ));
                    }
                    -2 - *n
                }
                _ => {
                    let us = crate::lisp::builtins::misc::lisp_time_to_us(i, v)?;
                    us * 1000
                }
            };
            let b = cur(i);
            let mut bb = b.borrow_mut();
            bb.file_modtime_ns = ns;
            bb.file_modtime_size = -1;
            Ok(Value::Nil)
        }
    }
}

/// Parse a lock-file target `USER@HOST.PID[:BOOT]' into
/// (user, host, pid, boot); None if malformed.  A missing `:BOOT'
/// suffix parses as 0 (GNU filelock.c `current_lock_owner').
fn parse_lock_target(target: &str) -> Option<(String, String, u32, i64)> {
    let (user, rest) = target.split_once('@')?;
    let (host, pid) = rest.rsplit_once('.')?;
    let mut parts = pid.splitn(2, ':');
    let pid = parts.next()?.parse().ok()?;
    let boot = parts.next().and_then(|b| b.parse().ok()).unwrap_or(0);
    Some((user.to_string(), host.to_string(), pid, boot))
}

/// GNU `current_lock_owner' result.
enum LockOwner {
    /// The lock belongs to this Emacs process.
    Ours,
    /// Another live process holds it: `user' is the plain name
    /// (file-locked-p returns it) and `info' is GNU's
    /// `USER@HOST (pid N)' description used in `file-locked' errors.
    Foreign { user: String, info: String },
}

/// Parse the lock target and classify ownership.  Err = not locked or
/// stale (dead pid on this host, a boot-time mismatch meaning the
/// lock predates the last reboot, or an unparseable target).
fn lock_owner(target: &str) -> Result<LockOwner, ()> {
    let (user, host, pid, boot) = parse_lock_target(target).ok_or(())?;
    let same_host = host == crate::buffer::our_host_name();
    if same_host && pid == std::process::id() {
        return Ok(LockOwner::Ours);
    }
    if same_host {
        // GNU current_lock_owner: the lock is stale when its recorded
        // boot time differs from the system's (the lock was left by a
        // process from a previous boot), or when the pid is dead.
        let bt = crate::buffer::boot_time();
        if bt != 0 && (boot - bt).abs() > 1 {
            return Err(());
        }
        #[cfg(unix)]
        unsafe {
            if libc::kill(pid as i32, 0) != 0 && *libc::__error() == libc::ESRCH {
                return Err(()); // stale lock
            }
        }
    }
    Ok(LockOwner::Foreign {
        info: format!("{}@{} (pid {})", user, host, pid),
        user,
    })
}

/// Signal GNU's `file-locked' error, as `ask-user-about-lock' does in
/// batch mode: (FILE OWNER-INFO "Cannot resolve lock conflict in
/// batch mode").
fn signal_file_locked(i: &mut Interp, file: Value, owner_info: Value) -> Flow {
    let fl = i.intern("file-locked");
    i.signal_data(
        fl,
        vec![
            file,
            owner_info,
            Value::string("Cannot resolve lock conflict in batch mode"),
        ],
    )
}

/// GNU `prepare_to_modify_buffer' runs `lock_file' before the first
/// modification of a file-visiting buffer; a foreign lock makes the
/// modification itself signal `file-locked'.
pub(crate) fn barf_if_file_locked(i: &mut Interp) -> EvalResult {
    let b = cur(i);
    // Refresh the create-lockfiles snapshot so dynamic `let' bindings
    // established after visiting still take effect.
    if let Some(cl) = i.intern_soft("create-lockfiles") {
        let v = i.symbol_value(cl).truthy();
        b.borrow_mut().create_lockfiles = v;
    }
    let (modified, file) = {
        let bb = b.borrow();
        (bb.is_modified(), bb.file_name.clone())
    };
    if modified {
        return Ok(Value::Nil);
    }
    let f = match file {
        Some(f) => f,
        None => return Ok(Value::Nil),
    };
    let f = file_truename(&f);
    if let Ok(target) = std::fs::read_link(&lock_file_name(&f)) {
        if let Ok(LockOwner::Foreign { info, .. }) = lock_owner(&target.to_string_lossy()) {
            return Err(signal_file_locked(i, Value::string(f), Value::string(info)));
        }
    }
    Ok(Value::Nil)
}

fn f_lock_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU lock-buffer: "Lock FILE, if current buffer is modified."
    // The modified check precedes the FILE argument handling.
    {
        let b = cur(i);
        let bb = b.borrow();
        if !bb.is_modified() {
            return Ok(Value::Nil);
        }
    }
    let file = match a.first() {
        Some(Value::Nil) | None => {
            let b = cur(i);
            match b.borrow().file_name.clone() {
                Some(f) => f,
                None => return Ok(Value::Nil),
            }
        }
        Some(v) => want_string(i, v)?,
    };
    let cl = i.intern_soft("create-lockfiles");
    if let Some(id) = cl {
        if !i.symbol_value(id).truthy() {
            return Ok(Value::Nil);
        }
    }
    // GNU locks the truename of the visited file.
    let file = file_truename(&file);
    let lname = lock_file_name(&file);
    let owner = lock_owner_string();
    #[cfg(unix)]
    match std::os::unix::fs::symlink(&owner, &lname) {
        Ok(()) => {
            cur(i).borrow_mut().file_lock_name = Some(lname);
            Ok(Value::Nil)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Lock exists: a lock we own is a no-op; a foreign lock
            // goes through ask-user-about-lock, which signals
            // `file-locked' in batch.
            match std::fs::read_link(&lname) {
                Ok(target) => match lock_owner(&target.to_string_lossy()) {
                    Ok(LockOwner::Ours) => {
                        cur(i).borrow_mut().file_lock_name = Some(lname);
                        Ok(Value::Nil)
                    }
                    Ok(LockOwner::Foreign { info, .. }) => Err(signal_file_locked(
                        i,
                        Value::string(file),
                        Value::string(info),
                    )),
                    // Stale/unparseable: GNU breaks the lock.
                    Err(()) => {
                        let _ = std::fs::remove_file(&lname);
                        match std::os::unix::fs::symlink(&owner, &lname) {
                            Ok(()) => {
                                cur(i).borrow_mut().file_lock_name = Some(lname);
                                Ok(Value::Nil)
                            }
                            Err(e) => Err(i.signal_data(
                                sym::FILE_ERROR,
                                vec![
                                    Value::string(format!("Locking file: {}", e)),
                                    Value::string(file),
                                ],
                            )),
                        }
                    }
                },
                Err(_) => Err(i.signal_data(
                    sym::FILE_ERROR,
                    vec![Value::string("Locking file"), Value::string(file)],
                )),
            }
        }
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Locking file: {}", e)),
                Value::string(file),
            ],
        )),
    }
    #[cfg(not(unix))]
    Ok(Value::Nil)
}

fn f_unlock_buffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let recorded = b.borrow_mut().file_lock_name.take();
    if let Some(l) = recorded {
        let _ = std::fs::remove_file(&l);
        return Ok(Value::Nil);
    }
    // No lock recorded: GNU removes `.#FILE' only if we own it.
    let file = b.borrow().file_name.clone();
    if let Some(f) = file {
        let l = lock_file_name(&file_truename(&f));
        if let Ok(target) = std::fs::read_link(&l) {
            if matches!(lock_owner(&target.to_string_lossy()), Ok(LockOwner::Ours)) {
                let _ = std::fs::remove_file(&l);
            }
        }
    }
    Ok(Value::Nil)
}

fn f_file_locked_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_string(i, &a[0])?;
    let lname = lock_file_name(&file_truename(&file));
    match std::fs::read_link(&lname) {
        Err(_) => Ok(Value::Nil),
        Ok(target) => match lock_owner(&target.to_string_lossy()) {
            // GNU returns t for our own lock, else the owner's
            // user name.
            Ok(LockOwner::Ours) => Ok(Value::t()),
            Ok(LockOwner::Foreign { user, .. }) => Ok(Value::string(user)),
            Err(()) => Ok(Value::Nil),
        },
    }
}

/// GNU `lock-file': create FILE's `.#FILE' lock, best effort.
/// A foreign live lock goes through ask-user-about-lock (signals
/// `file-locked' in batch); ours or stale is replaced quietly.
fn f_lock_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_string(i, &a[0])?;
    if let Some(id) = i.intern_soft("create-lockfiles") {
        if !i.symbol_value(id).truthy() {
            return Ok(Value::Nil);
        }
    }
    let file = file_truename(&file);
    let lname = lock_file_name(&file);
    let owner = lock_owner_string();
    #[cfg(unix)]
    match std::os::unix::fs::symlink(&owner, &lname) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            match std::fs::read_link(&lname) {
                Ok(target) => match lock_owner(&target.to_string_lossy()) {
                    Ok(LockOwner::Ours) => {}
                    Ok(LockOwner::Foreign { info, .. }) => {
                        return Err(signal_file_locked(
                            i,
                            Value::string(file),
                            Value::string(info),
                        ));
                    }
                    Err(()) => {
                        let _ = std::fs::remove_file(&lname);
                        let _ = std::os::unix::fs::symlink(&owner, &lname);
                    }
                },
                Err(_) => {}
            }
        }
        Err(_) => {}
    }
    Ok(Value::Nil)
}

/// GNU `unlock-file': remove FILE's lock if it is ours or stale.
fn f_unlock_file(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_string(i, &a[0])?;
    let lname = lock_file_name(&file_truename(&file));
    if let Ok(target) = std::fs::read_link(&lname) {
        match lock_owner(&target.to_string_lossy()) {
            Ok(LockOwner::Ours) | Err(()) => {
                let _ = std::fs::remove_file(&lname);
            }
            Ok(LockOwner::Foreign { info, .. }) => {
                return Err(signal_file_locked(
                    i,
                    Value::string(file),
                    Value::string(info),
                ));
            }
        }
    }
    Ok(Value::Nil)
}

/// GNU `file-acl': returns the ACL text, or nil when none / unsupported.
/// macOS: acl_get_file + acl_to_text (Darwin ACL), same output as GNU's
/// HAVE_ACL path (`!#acl 1\nentry...`).
fn f_file_acl(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = want_string(i, &a[0])?;
    let path = crate::editor::expand_file_name_str(i, &file);
    Ok(match acl_text(&path) {
        Some(s) => Value::string(s),
        None => Value::Nil,
    })
}

/// ACL text for PATH in GNU's `acl_to_text' format; None when the
/// file carries no extended ACL or ACLs are unsupported.
#[cfg(target_os = "macos")]
fn acl_text(path: &str) -> Option<String> {
    use std::ffi::{CStr, CString};
    unsafe extern "C" {
        fn acl_get_file(path: *const std::os::raw::c_char, kind: u32) -> *mut std::os::raw::c_void;
        fn acl_to_text(
            acl: *const std::os::raw::c_void,
            len_p: *mut isize,
        ) -> *mut std::os::raw::c_char;
        fn acl_free(ptr: *mut std::os::raw::c_void) -> i32;
    }
    // macOS <sys/acl.h>: ACL_TYPE_EXTENDED.
    const ACL_TYPE_EXTENDED: u32 = 0x00000100;
    let cpath = CString::new(path).ok()?;
    unsafe {
        let acl = acl_get_file(cpath.as_ptr(), ACL_TYPE_EXTENDED);
        if acl.is_null() {
            return None;
        }
        // Empty ACLs (no entries) still allocate; acl_to_text gives
        // NULL / "" for them — GNU returns nil then.
        let txt = acl_to_text(acl, std::ptr::null_mut());
        let out = if txt.is_null() {
            None
        } else {
            let s = CStr::from_ptr(txt).to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        };
        if !txt.is_null() {
            acl_free(txt as *mut _);
        }
        acl_free(acl);
        out
    }
}

#[cfg(not(target_os = "macos"))]
fn acl_text(_path: &str) -> Option<String> {
    None
}

fn f_ask_user_about_lock(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU prompts interactively; in batch or when the file isn't
    // locked by another Emacs, this signals `file-locked'.
    let file = a[0].clone();
    let other = a.get(1).cloned().unwrap_or(Value::Nil);
    Err(signal_file_locked(i, file, other))
}

/// GNU: 0 when equal, else +/-(1 + number of matching leading chars).
/// Honors `case-fold-search`.
fn f_compare_buffer_substrings(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcompare_buffer_substrings resolves buffer NAMES via
    // decode_buffer → get_buffer, in order buf1 → start1/end1 (CHECK_NUMBER_
    // COERCE_MARKER → integer-or-marker-p) → buf2 → start2/end2.
    let iom = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            // GNU CHECK_NUMBER_COERCE_MARKER yields the marker's 1-based
            // charpos; Marker.position is the 0-based offset.
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let b1 = buf_or_name(i, &a[0])?;
    let s1 = {
        let bb1 = b1.borrow();
        let (b, e) = (
            pos_idx(bb1.text.len(), iom(i, &a[1])?),
            pos_idx(bb1.text.len(), iom(i, &a[2])?),
        );
        bb1.text.substring(b.min(e), b.max(e))
    };
    let b2 = buf_or_name(i, &a[3])?;
    let (s1, s2) = {
        let bb2 = b2.borrow();
        let (b2x, e2x) = (
            pos_idx(bb2.text.len(), iom(i, &a[4])?),
            pos_idx(bb2.text.len(), iom(i, &a[5])?),
        );
        (s1, bb2.text.substring(b2x.min(e2x), b2x.max(e2x)))
    };
    let fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let n = cmp_common_prefix(&s1, &s2, fold);
    let eq = n == s1.chars().count() && s1.chars().count() == s2.chars().count();
    if eq {
        return Ok(Value::Int(0));
    }
    let less = if n == s1.chars().count() {
        true
    } else if n == s2.chars().count() {
        false
    } else {
        let c1 = fold_char(s1.chars().nth(n).unwrap(), fold);
        let c2 = fold_char(s2.chars().nth(n).unwrap(), fold);
        c1 < c2
    };
    Ok(Value::Int(if less {
        -(n as i128) - 1
    } else {
        n as i128 + 1
    }))
}

fn fold_char(c: char, fold: bool) -> char {
    if fold {
        c.to_lowercase().next().unwrap_or(c)
    } else {
        c
    }
}

/// Length of the common leading character prefix of `a` and `b`.
fn cmp_common_prefix(a: &str, b: &str, fold: bool) -> usize {
    let mut n = 0;
    for (x, y) in a.chars().zip(b.chars()) {
        if fold_char(x, fold) != fold_char(y, fold) {
            break;
        }
        n += 1;
    }
    n
}
fn f_object_intervals(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            if !i.has_str_props(s) {
                return Ok(Value::Nil);
            }
            let len = str_len(s);
            let ivs = i.str_props(s);
            // GNU's tree covers the whole string; synthesize nil-plist
            // intervals over the gaps between stored intervals.
            let mut out: Vec<Value> = Vec::new();
            let mut p = 0usize;
            let mut emit = |a: usize, b: usize, pl: Vec<Value>| {
                if a < b {
                    out.push(Value::list(vec![
                        Value::Int(a as i128),
                        Value::Int(b as i128),
                        if pl.is_empty() {
                            Value::Nil
                        } else {
                            Value::list(pl)
                        },
                    ]));
                }
            };
            for (s0, e0, pl) in ivs {
                emit(p, *s0, Vec::new());
                emit(*s0, *e0, pl.clone());
                p = p.max(*e0);
            }
            emit(p, len, Vec::new());
            Ok(Value::list(out))
        }
        Value::Buffer(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
}

fn f_accept_change_group(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates the handle is a change-group cons; accept = drop it.
    if !matches!(a[0], Value::Cons(_)) && !matches!(a[0], Value::Nil) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    Ok(Value::Nil)
}

/// GNU simple.el `undo-outer-limit-truncate' — GC's undo-size gate:
/// builds the "undo info is/was N bytes" message first (a non-integer
/// SIZE dies inside `format-message' on `%d'), then either prompts
/// `yes-or-no-p' (undo-ask-before-discard) or `display-warning's and
/// clears `buffer-undo-list'.
fn f_undo_outer_limit_truncate(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let size = match &a[0] {
        Value::Int(n) => *n,
        _ => return Err(i.error_q("Format specifier doesn't match argument type", &[])),
    };
    let buf_name = cur(i).borrow().name.clone();
    let ask = {
        let s = i.intern("undo-ask-before-discard");
        i.symbol_value(s).truthy()
    };
    if ask {
        let extra = {
            let s = i.intern("undo-extra-outer-limit");
            i.symbol_value(s)
        };
        let ask_again = match &extra {
            Value::Int(e) => size > *e,
            _ => true,
        };
        if !ask_again {
            return Ok(Value::Nil);
        }
        {
            let s = i.intern("undo-extra-outer-limit");
            i.obarray.symbol_mut(s).value = Value::Int(size + 50000);
        }
        let msg = format!(
            "Buffer \u{2018}{}\u{2019} undo info is {} bytes long; discard it? ",
            buf_name, size
        );
        let ynp = Value::Sym(i.intern("yes-or-no-p"));
        let ans = i.apply(&ynp, vec![Value::string(msg)])?;
        if ans.truthy() {
            if let Some(b) = i.buffers.get(i.current_buffer) {
                b.borrow_mut().set_undo_list(Value::Nil);
            }
            {
                let s = i.intern("undo-extra-outer-limit");
                i.obarray.symbol_mut(s).value = Value::Nil;
            }
            return Ok(Value::t());
        }
        return Ok(Value::Nil);
    }
    // Only the first `format-message' fragment gets quote-translation;
    // the tail is a plain literal concat'd on, so its graves stay
    // ASCII (`undo-outer-limit' etc).
    let msg = format!(
        "Buffer \u{2018}{}\u{2019} undo info was {} bytes long.\n\
         The undo info was discarded because it exceeded `undo-outer-limit'.\n\n\
         This is normal if you executed a command that made a huge change\n\
         to the buffer.  In that case, to prevent similar problems in the\n\
         future, set `undo-outer-limit' to a value that is large enough to\n\
         cover the maximum size of normal changes you expect a single\n\
         command to make, but not so large that it might exceed the\n\
         maximum memory allotted to Emacs.\n\n\
         If you did not execute any such command, the situation is\n\
         probably due to a bug and you should report it.\n\n\
         You can disable the popping up of this buffer by adding the entry\n\
         (undo discard-info) to the user option `warning-suppress-types',\n\
         which is defined in the `warnings' library.\n",
        buf_name, size
    );
    let dw = Value::Sym(i.intern("display-warning"));
    let undo_s = i.intern("undo");
    let discard_s = i.intern("discard-info");
    let warning_s = i.intern("warning");
    i.apply(
        &dw,
        vec![
            Value::list(vec![Value::Sym(undo_s), Value::Sym(discard_s)]),
            Value::string(msg),
            Value::Sym(warning_s),
        ],
    )?;
    if let Some(b) = i.buffers.get(i.current_buffer) {
        b.borrow_mut().set_undo_list(Value::Nil);
    }
    Ok(Value::t())
}

fn f_noop(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}
fn f_progn_raw(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    i.eval_progn(a.first().unwrap_or(&Value::Nil))
}
fn f_second_form_raw(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (combine-change-calls BEG END . BODY) — eval BODY.
    let args = a.first().cloned().unwrap_or(Value::Nil);
    let mut cur = args;
    for _ in 0..2 {
        cur = match cur {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => Value::Nil,
        };
    }
    i.eval_progn(&cur)
}

// ---------- helpers ----------

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Nil)
}

fn want_int(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
        _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
    }
}
/// GNU buffer.c `fix_position': fixnum → value; marker → charpos
/// (detached signals "Marker does not point anywhere"); other
/// non-integers → `integer-or-marker-p'.  (Bignum saturation exists
/// in GNU but our inline ints are already fixnum-range.)
fn want_int_or_marker_pos(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Marker(m) => {
            let m = m.borrow();
            if m.buffer.is_none() {
                Err(i.error("Marker does not point anywhere"))
            } else {
                Ok(m.position as i128 + 1)
            }
        }
        _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
    }
}

pub(crate) fn want_sym(i: &mut Interp, v: &Value) -> Result<u32, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
}

/// Resolve optional buffer arg (Value::Buffer/Str/nil) to a shared ref.
/// Names resolve via `get-buffer'; nil selects the current buffer.
/// Only for functions whose GNU twin takes buffer-or-name
/// (`Fdecode_buffer` callers that funnel through `get_buffer'):
/// bury/unbury/kill/set-buffer/other-buffer/rename-buffer,
/// insert-buffer-substring, compare-buffer-substrings,
/// buffer-disable/enable-undo.
pub(crate) fn buf_or_name(
    i: &mut Interp,
    v: &Value,
) -> Result<Rc<RefCell<crate::buffer::Buffer>>, Flow> {
    match v {
        Value::Nil => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("Selecting deleted buffer")),
        Value::Buffer(b) => Ok(b.clone()),
        Value::Str(s) => {
            let name = s.borrow().clone();
            i.buffers
                .by_name(&name)
                .and_then(|id| i.buffers.get(id))
                .ok_or_else(|| {
                    i.signal_data(
                        sym::ERROR,
                        vec![Value::string(format!("No buffer named {}", name))],
                    )
                })
        }
        // GNU's Fget_buffer signals stringp for non-buffer/non-name args.
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

/// GNU `Fdecode_buffer`: nil → current buffer; anything else must be a
/// live buffer object — a string name is NOT accepted (CHECK_BUFFER).
/// This is the argument decoder for buffer-size, buffer-modified-p,
/// buffer-file-name, buffer-base-buffer, buffer-local-variables,
/// buffer-local-value, buffer-local-boundp, local-variable-p,
/// local-variable-if-set-p, verify-visited-file-modtime,
/// buffer-swap-text, buffer-modified-tick, and the text-property
/// functions' OBJECT argument when it is not a string.
pub(crate) fn buf_of(
    i: &mut Interp,
    v: &Value,
) -> Result<Rc<RefCell<crate::buffer::Buffer>>, Flow> {
    match v {
        Value::Nil => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("Selecting deleted buffer")),
        Value::Buffer(b) => Ok(b.clone()),
        other => Err(i.wrong_type_mut("bufferp", other)),
    }
}

pub(crate) fn cur(i: &Interp) -> Rc<RefCell<crate::buffer::Buffer>> {
    i.current_buffer_ref().unwrap()
}

/// GNU change-hook signaling around buffer text mutations.  The hook
/// calls run with the buffer borrow released so hook functions can
/// inspect (and even modify) the buffer; positions are 0-based here
/// and converted to the 1-based values hooks receive.
use crate::lisp::builtins::evalfn::{call_hook, signal_after_change, signal_before_change};

/// `insert_at' (POS 0-based) with before/after-change signals.
pub(crate) fn chg_insert(i: &mut Interp, pos: usize, s: &str) -> EvalResult {
    let n = s.chars().count();
    if n == 0 {
        return Ok(Value::Nil);
    }
    signal_before_change(i, pos + 1, pos + 1)?;
    cur(i).borrow_mut().insert_at(pos, s);
    signal_after_change(i, pos + 1, pos + 1 + n, 0)
}

/// `insert'/`insert-before-markers' at point (advances point).
pub(crate) fn chg_insert_pt(i: &mut Interp, s: &str, before_markers: bool) -> EvalResult {
    let n = s.chars().count();
    if n == 0 {
        return Ok(Value::Nil);
    }
    let p = cur(i).borrow().point();
    text_ro_insert(i, p)?;
    signal_before_change(i, p + 1, p + 1)?;
    let q = {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        if before_markers {
            bb.insert_before_markers(s);
        } else {
            bb.insert(s);
        }
        bb.point()
    };
    signal_after_change(i, q + 1 - n, q + 1, 0)
}

/// `delete_region' over [S0,E0) (0-based) with signals; returns the
/// removed text.
pub(crate) fn chg_delete(i: &mut Interp, s0: usize, e0: usize) -> Result<String, Flow> {
    if s0 >= e0 {
        return Ok(String::new());
    }
    // GNU barf-if-buffer-read-only: kill/delete in a read-only buffer
    // signals `buffer-read-only' before touching text.
    check_writable(i)?;
    text_ro_range(i, s0, e0)?;
    signal_before_change(i, s0 + 1, e0 + 1)?;
    let removed = cur(i).borrow_mut().delete_region(s0, e0);
    signal_after_change(i, s0 + 1, s0 + 1, (e0 - s0) as i128)?;
    Ok(removed)
}

/// GNU `replace_range': ONE before/after pair and one MODIFF bump of
/// elogb(del+ins)+1 — `replace-match', not the two pairs a
/// delete+insert would produce.  Signals (S0+1, E0+1) before and
/// (S0+1, S0+1+N_INS, N_DEL) after.
pub(crate) fn chg_replace(i: &mut Interp, s0: usize, e0: usize, s: &str) -> EvalResult {
    let n_ins = s.chars().count();
    if s0 < e0 {
        text_ro_range(i, s0, e0)?;
    }
    text_ro_insert(i, s0)?;
    signal_before_change(i, s0 + 1, e0 + 1)?;
    let n_del = {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let e = e0.min(bb.text.len());
        let d = e.saturating_sub(s0);
        bb.raw_delete_region(s0, e0);
        bb.raw_insert_at(s0, s);
        bb.note_text_change(d + n_ins);
        d
    };
    signal_after_change(i, s0 + 1, s0 + 1 + n_ins, n_del as i128)
}

/// Run F with BUF_IDX temporarily current — GNU change hooks fire in
/// the modified buffer, which is not always the selected one
/// (`copy-to-buffer', `append-to-buffer').
pub(crate) fn chg_with_buffer(
    i: &mut Interp,
    buf_idx: usize,
    f: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    let old = i.current_buffer;
    i.set_current_buffer(buf_idx);
    let r = f(i);
    i.set_current_buffer(old);
    r
}

/// Signal an error named `name` with data `data` (avoids nested borrows).
pub(crate) fn err_sym(i: &mut Interp, name: &str, data: Vec<Value>) -> Flow {
    let id = i.intern(name);
    i.signal_data(id, data)
}

/// Signal `buffer-read-only` if the current buffer is read-only.
pub(crate) fn check_writable(i: &mut Interp) -> Result<(), Flow> {
    let ro_id = i.intern("buffer-read-only");
    let ro = i.symbol_value(ro_id);
    let inh_id = i.intern("inhibit-read-only");
    let inhibit = i.symbol_value(inh_id);
    if ro.truthy() && !inhibit.truthy() {
        let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
        return Err(i.signal_data(sym::BUFFER_READ_ONLY, vec![bv]));
    }
    Ok(())
}

/// Does `rear-nonsticky' at POS cover PROP?  `t' (or any non-list
/// non-nil value, per GNU's stickiness rules) covers every property;
/// a list covers its members.
fn prop_list_covers(_i: &mut Interp, v: Value, prop: u32) -> bool {
    match &v {
        Value::Cons(_) => {
            let prop_sym = Value::Sym(prop);
            let mut c = v.clone();
            while let Value::Cons(cc) = &c {
                let (h, tl) = {
                    let b = cc.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if crate::lisp::builtins::eq_values(&h, &prop_sym) {
                    return true;
                }
                c = tl;
            }
            false
        }
        _ => v.truthy(),
    }
}

fn rear_nonsticky_covers_bb(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> bool {
    let ns = i.intern("rear-nonsticky");
    let v = prop_at_bb(i, bb, pos, ns);
    prop_list_covers(i, v, prop)
}

fn rear_nonsticky_covers(i: &mut Interp, pos: usize, prop: u32) -> bool {
    let ns = i.intern("rear-nonsticky");
    let v = prop_at(i, pos, ns);
    prop_list_covers(i, v, prop)
}

/// GNU `verify_interval_modification' read-only gate for insertion at
/// 0-based POS: insertion is blocked when `read-only' applies from
/// the rear (the char at POS-1, unless `rear-nonsticky' covers it) or
/// when the char at POS itself has front-sticky `read-only'.
/// Returns Err with the GNU `text_read_only' signal shape.
/// `inhibit-read-only': t (or any non-list) bypasses everything; a
/// list bypasses only the `read-only' VALUES it contains.
pub(crate) fn text_ro_insert(i: &mut Interp, pos: usize) -> Result<(), Flow> {
    let ro = i.intern("read-only");
    let inh_id = i.intern("inhibit-read-only");
    let cat = i.intern("category");
    let front = i.intern("front-sticky");
    let rear = i.intern("rear-nonsticky");
    let inh = i.symbol_value(inh_id);
    // GNU `!NILP(Vinhibit_read_only) && !CONSP' skips the checks.
    if inh.truthy() && !matches!(inh, Value::Cons(_)) {
        return Ok(());
    }
    // `memq'-style per-value bypass when inhibit-read-only is a list.
    let inh_memq = |v: &Value| -> bool {
        matches!(&inh, Value::Cons(_)) && inh
            .list_to_vec()
            .map(|l| l.iter().any(|e| crate::lisp::builtins::eq_values(e, v)))
            .unwrap_or(false)
    };
    let (begv, zv, props) = {
        let b = cur(i);
        let bb = b.borrow();
        // GNU: `if (!intervals) return' — a buffer that never had a
        // property interval skips all read-only checks, so
        // `default-text-properties' alone can't gate it.
        if !bb.text.had_intervals.get() {
            return Ok(());
        }
        (bb.begv, bb.zv, bb.text.props.borrow().clone())
    };
    // Raw `plist_get' on the flat records (GNU's plist_get).
    let direct = |pos: usize, prop: u32| -> Value {
        for tp in props.iter().rev() {
            if pos >= tp.start && pos < tp.end && tp.prop == prop {
                return tp.value.clone();
            }
        }
        Value::Nil
    };
    // GNU `textget' = lookup_char_property on the interval plist:
    // direct hit → `category' symbol's plist → alias alist →
    // `default-text-properties' (buffer lookups pass textprop=1).
    let textget = |i: &Interp, pos: usize, prop: u32| -> Value {
        let mut catval = Value::Nil;
        for tp in props.iter().rev() {
            if pos >= tp.start && pos < tp.end {
                if tp.prop == prop {
                    return tp.value.clone();
                }
                if tp.prop == cat && catval.is_nil() {
                    catval = tp.value.clone();
                }
            }
        }
        if let Value::Sym(cs) = catval {
            let v = i.get_prop(cs, prop);
            if !v.is_nil() {
                return v;
            }
        }
        // char-property-alias-alist: plist_get on the flat records.
        let alist = i.symbol_value(match i.intern_soft("char-property-alias-alist") {
            Some(s) => s,
            None => return Value::Nil,
        });
        if let Some(cell) = alist
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .find(|e| matches!(e, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(prop)))
        {
            if let Value::Cons(c) = cell {
                for a in c.borrow().cdr.clone().list_to_vec().unwrap_or_default() {
                    if let Some(aid) = i.sym_id(&a) {
                        let v = direct(pos, aid);
                        if !v.is_nil() {
                            return v;
                        }
                    }
                }
            }
        }
        // lookup_char_property's textprop=1 tail: default-text-properties.
        default_prop(i, prop)
    };
    // i == prev iff no property boundary at POS: GNU splits intervals
    // at every property edge, so one record touching POS makes the
    // before/after intervals differ.
    let has_left = pos > begv;
    let has_right = pos < zv;
    // GNU's i != prev: prev is NULL at BEGV, i is NULL at ZV, and a
    // property boundary at POS always splits the two intervals.
    let boundary = pos == begv
        || pos == zv
        || props.iter().any(|tp| tp.start == pos || tp.end == pos);
    if boundary {
        // i != prev: check AFTER (right char) front-sticky read-only,
        // then PREV (left char) rear-sticky read-only.
        if has_right {
            let after = textget(i, pos, ro);
            if after.truthy() && !inh_memq(&after) {
                let fst = textget(i, pos, front);
                let ro_direct = !direct(pos, ro).is_nil();
                if prop_list_covers(i, fst.clone(), ro)
                    || (!ro_direct && prop_list_covers(i, fst, cat))
                {
                    return Err(text_ro_error(i, &after));
                }
            }
        }
        if has_left {
            let before = textget(i, pos - 1, ro);
            if before.truthy() && !inh_memq(&before) {
                let rns = textget(i, pos - 1, rear);
                let ro_direct = !direct(pos - 1, ro).is_nil();
                if !prop_list_covers(i, rns.clone(), ro)
                    && (ro_direct || !prop_list_covers(i, rns, cat))
                {
                    return Err(text_ro_error(i, &before));
                }
            }
        }
    } else if has_left && has_right {
        // i == prev (both exist — not BEGV/ZV, no boundary): one
        // interval spans the point; after must be read-only AND
        // either front-sticky covers it or the rear isn't nonsticky.
        let after = textget(i, pos, ro);
        if after.truthy() && !inh_memq(&after) {
            let fst = textget(i, pos, front);
            if prop_list_covers(i, fst.clone(), ro)
                || (!direct(pos, ro).is_nil() || prop_list_covers(i, fst, cat))
            {
                return Err(text_ro_error(i, &after));
            }
            let rns = textget(i, pos - 1, rear);
            if !prop_list_covers(i, rns.clone(), ro)
                && (!direct(pos - 1, ro).is_nil() || !prop_list_covers(i, rns, cat))
            {
                return Err(text_ro_error(i, &after));
            }
        }
    }
    Ok(())
}

/// GNU `verify_interval_modification' range branch: each covering
/// interval signals `text-read-only' (or `buffer-read-only' when the
/// buffer is read-only and only implicitly writable).  Positions are
/// 0-based [S0,E0).
pub(crate) fn text_ro_range(i: &mut Interp, s0: usize, e0: usize) -> Result<(), Flow> {
    let ro = i.intern("read-only");
    let cat = i.intern("category");
    let iro_prop = i.intern("inhibit-read-only");
    let inh = i.symbol_value(iro_prop);
    // GNU `!NILP(Vinhibit_read_only) && !CONSP' skips read-only checks.
    let global_bypass = inh.truthy() && !matches!(inh, Value::Cons(_));
    let inh_memq = |v: &Value| -> bool {
        matches!(&inh, Value::Cons(_)) && inh
            .list_to_vec()
            .map(|l| l.iter().any(|e| crate::lisp::builtins::eq_values(e, v)))
            .unwrap_or(false)
    };
    // GNU reads BVAR (current_buffer, read_only) && NILP(Vinhibit_read_only).
    let bro = i.intern("buffer-read-only");
    let buffer_ro = i.symbol_value(bro).truthy() && inh.is_nil();
    let props = {
        let b = cur(i);
        let bb = b.borrow();
        // GNU `if (!intervals) return'.
        if !bb.text.had_intervals.get() {
            return Ok(());
        }
        bb.text.props.borrow().clone()
    };
    // GNU walks the interval set one INTERVAL at a time; on the flat
    // store an "interval" is a maximal span with constant record
    // coverage — boundaries at every tp.start/tp.end inside [s0,e0).
    let mut cuts: Vec<usize> = vec![s0];
    for tp in &props {
        if tp.start > s0 && tp.start < e0 {
            cuts.push(tp.start);
        }
        if tp.end > s0 && tp.end < e0 {
            cuts.push(tp.end);
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let direct = |pos: usize, prop: u32| -> Value {
        for tp in props.iter().rev() {
            if pos >= tp.start && pos < tp.end && tp.prop == prop {
                return tp.value.clone();
            }
        }
        Value::Nil
    };
    // textget: direct → category → alias → default-text-properties.
    let textget = |i: &Interp, pos: usize, prop: u32| -> Value {
        let mut catval = Value::Nil;
        for tp in props.iter().rev() {
            if pos >= tp.start && pos < tp.end {
                if tp.prop == prop {
                    return tp.value.clone();
                }
                if tp.prop == cat && catval.is_nil() {
                    catval = tp.value.clone();
                }
            }
        }
        if let Value::Sym(cs) = catval {
            let v = i.get_prop(cs, prop);
            if !v.is_nil() {
                return v;
            }
        }
        let alist = i.symbol_value(match i.intern_soft("char-property-alias-alist") {
            Some(s) => s,
            None => return default_prop(i, prop),
        });
        if let Some(cell) = alist
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .find(|e| matches!(e, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(prop)))
        {
            if let Value::Cons(c) = cell {
                for a in c.borrow().cdr.clone().list_to_vec().unwrap_or_default() {
                    if let Some(aid) = i.sym_id(&a) {
                        let v = direct(pos, aid);
                        if !v.is_nil() {
                            return v;
                        }
                    }
                }
            }
        }
        default_prop(i, prop)
    };
    let mut p = 0usize;
    for &seg in &cuts {
        // Segment [seg, next) — one GNU interval's coverage.
        let read_only = textget(i, seg, ro);
        let implied = read_only.is_nil() || global_bypass;
        let express = {
            let iro_v = direct(seg, iro_prop);
            iro_v.truthy() || (read_only.truthy() && inh_memq(&read_only))
        };
        if !implied && !express {
            return Err(text_ro_error(i, &read_only));
        } else if !express && buffer_ro {
            let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
            return Err(i.signal_data(sym::BUFFER_READ_ONLY, vec![bv]));
        }
        p = cuts
            .iter()
            .find(|&&c| c > seg)
            .copied()
            .unwrap_or(e0);
    }
    // GNU: when the last writable interval does not reach END and the
    // buffer is read-only.
    if p < e0 && buffer_ro {
        let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
        return Err(i.signal_data(sym::BUFFER_READ_ONLY, vec![bv]));
    }
    Ok(())
}

/// GNU `text_read_only': the error carries the read-only VALUE only
/// when it is a string.
pub(crate) fn text_ro_error(i: &mut Interp, propval: &Value) -> Flow {
    let s = i.intern("text-read-only");
    match propval {
        Value::Str(_) => i.signal_data(s, vec![propval.clone()]),
        _ => i.signal_data(s, vec![]),
    }
}

/// 0-based clamped index from an Emacs position arg.
pub(crate) fn pos_idx(len_or_zv: usize, p: i128) -> usize {
    (p.max(1) as usize - 1).min(len_or_zv)
}

/// GNU `validate_interval_range' on a START END pair: signal
/// `(args-out-of-range START END)' — the *raw* values, not swapped/clamped.  A reversed
/// START>END pair is legal (the scan is simply empty).
pub(crate) fn want_range_in_range(
    i: &mut Interp,
    s: &Value,
    e: &Value,
) -> Result<(i128, i128), Flow> {
    let sv = want_int(i, s)?;
    let ev = want_int(i, e)?;
    let bc = cur(i);
    let bb = bc.borrow();
    let lo = bb.begv as i128 + 1;
    let hi = bb.zv as i128 + 1;
    drop(bb);
    if sv < lo || sv > hi || ev < lo || ev > hi {
        return Err(err_sym(i, "args-out-of-range", vec![s.clone(), e.clone()]));
    }
    Ok((sv, ev))
}

/// `want_pos_in_range' against a specific buffer (OBJECT arg).  GNU
/// validate_interval_range measures against the object, not the
/// current buffer.
pub(crate) fn want_pos_in_buf(
    i: &mut Interp,
    v: &Value,
    bb: &crate::buffer::Buffer,
) -> Result<i128, Flow> {
    want_pos_in_buf_dup(i, v, bb, true)
}

/// `want_pos_in_buf' with GNU's two payload shapes: most textprop
/// entry points emit (POS POS); `get-char-property' / `field-*'
/// paths emit a bare (POS).
pub(crate) fn want_pos_in_buf_dup(
    i: &mut Interp,
    v: &Value,
    bb: &crate::buffer::Buffer,
    dup: bool,
) -> Result<i128, Flow> {
    // GNU POSITION args accept markers (integer-or-marker-p): the
    // marker's 0-based slot +1 is its 1-based char position.
    let p = crate::lisp::builtins::want_int_or_mark(i, v)?;
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    if p < lo || p > hi {
        let payload = if dup {
            vec![v.clone(), v.clone()]
        } else {
            vec![v.clone()]
        };
        return Err(err_sym(i, "args-out-of-range", payload));
    }
    Ok(p)
}

pub(crate) fn marker_value(m: Rc<RefCell<Marker>>) -> Value {
    Value::Marker(m)
}

// ---------- buffer objects ----------

fn f_current_buffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.buffer_value(i.current_buffer).unwrap_or(Value::Nil))
}

fn f_bufferp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Buffer(_))))
}

fn f_buffer_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fbuffer_name wants a real buffer object (bufferp check) —
    // `set-buffer' resolves names, `buffer-name' doesn’t.
    let b = match &arg(&a, 0) {
        Value::Nil => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("Selecting deleted buffer"))?,
        Value::Buffer(b) => b.clone(),
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    // GNU Fbuffer_name: a killed buffer's name is nil.
    Ok(if b.borrow().live {
        Value::string(b.borrow().name.clone())
    } else {
        Value::Nil
    })
}

fn f_get_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            match i.buffers.by_name(&name) {
                Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
                None => Ok(Value::Nil),
            }
        }
        Value::Buffer(b) => Ok(Value::Buffer(b.clone())),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_get_buffer_create(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            // GNU `Fmake_buffer': empty buffer names are rejected.
            if name.is_empty() {
                return Err(i.error("Empty string for buffer name is not allowed"));
            }
            let inhibit = a.get(1).map(|v| v.truthy()).unwrap_or(false);
            let id = match i.buffers.by_name(&name) {
                Some(id) => id,
                None => {
                    // GNU `init_buffer': buffer creation re-derives the
                    // standard case table's extras if they aren't all
                    // char-tables (characters.el clears them at loadup).
                    crate::lisp::builtins::misc::heal_standard_case_table(i);
                    let id = i.buffers.create(&name);
                    if inhibit {
                        if let Some(b) = i.buffers.get(id) {
                            b.borrow_mut().inhibit_buffer_hooks = true;
                        }
                    }
                    run_buffer_list_update(i, id);
                    id
                }
            };
            Ok(i.buffer_value(id).unwrap_or(Value::Nil))
        }
        Value::Buffer(b) => Ok(Value::Buffer(b.clone())),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_generate_new_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    // GNU `Fmake_buffer': empty buffer names are rejected.
    if name.is_empty() {
        return Err(i.error("Empty string for buffer name is not allowed"));
    }
    // GNU generate-new-buffer (buffer.c): get-buffer-create on
    // (generate-new-buffer-name NAME) — space names pick up the
    // `-<random>' suffix here, not in the interned buffer registry.
    let name = gen_unique_buffer_name(i, &name, None)?;
    crate::lisp::builtins::misc::heal_standard_case_table(i);
    let id = i.buffers.create_exact(&name);
    if a.get(1).map(|v| v.truthy()).unwrap_or(false) {
        if let Some(b) = i.buffers.get(id) {
            b.borrow_mut().inhibit_buffer_hooks = true;
        }
    }
    run_buffer_list_update(i, id);
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

/// GNU Fgenerate_new_buffer_name (buffer.c): if a buffer named NAME is
/// already live and IGNORE doesn’t cover it, names starting with a
/// space first get NAME-<random> (bug#57211), then NAME-<random><N>;
/// ordinary names go straight to the NAME<N> sequence.  Shared by
/// `generate-new-buffer-name', `clone-buffer' and
/// `make-indirect-buffer'.
pub(crate) fn gen_unique_buffer_name(
    i: &mut Interp,
    name: &str,
    ignore: Option<&Value>,
) -> Result<String, Flow> {
    // GNU: !NILP(ignore) routes through Fstring_equal, whose
    // CHECK_STRING signals on a non-string ignore before any buffer
    // lookup happens.
    if let Some(ig) = ignore.filter(|v| !v.is_nil()) {
        if !matches!(ig, Value::Str(_)) {
            return Err(i.wrong_type_mut("stringp", ig));
        }
    }
    let is_ignored = |cand: &str| match ignore {
        Some(Value::Str(s)) => *s.borrow() == cand,
        _ => false,
    };
    if is_ignored(name) || i.buffers.by_name(name).is_none() {
        return Ok(name.to_string());
    }
    let genbase = if name.starts_with(' ') {
        let r = crate::lisp::builtins::arith::get_random();
        let cand = format!("{}-{}", name, r % 1000000);
        if is_ignored(&cand) || i.buffers.by_name(&cand).is_none() {
            return Ok(cand);
        }
        cand
    } else {
        name.to_string()
    };
    let mut n: i64 = 2;
    loop {
        let cand = format!("{}<{}>", genbase, n);
        if is_ignored(&cand) || i.buffers.by_name(&cand).is_none() {
            return Ok(cand);
        }
        n += 1;
    }
}

fn f_generate_new_buffer_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::string(gen_unique_buffer_name(i, &name, a.get(1))?))
}

fn f_buffer_live_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(match &a[0] {
        Value::Buffer(b) => b.borrow().live,
        _ => false,
    }))
}

/// GNU `run_buffer_list_update_hook': run `buffer-list-update-hook'
/// unless the buffer was created with INHIBIT-BUFFER-HOOKS.
pub(crate) fn run_buffer_list_update(i: &mut Interp, id: usize) {
    let inhibited = i
        .buffers
        .get(id)
        .map(|b| b.borrow().inhibit_buffer_hooks)
        .unwrap_or(true);
    if inhibited {
        return;
    }
    let h = i.intern("buffer-list-update-hook");
    let rh = i.intern("run-hooks");
    let _ = i.apply(&Value::Sym(rh), vec![Value::Sym(h)]);
}

/// Teardown shared by `kill-buffer' and child kills: unlock the file
/// lock, drop the buffer from the registry, repair `current_buffer'
/// (GNU always has a live buffer — killing the last one yields a fresh
/// *scratch*), then run `buffer-list-update-hook'.
fn kill_buffer_teardown(i: &mut Interp, id: usize) -> bool {
    // GNU kill-buffer unlocks the killed buffer's file lock.
    if let Some(b) = i.buffers.get(id) {
        b.borrow_mut().release_lock_file();
    }
    let killed = {
        let obarray = &i.obarray;
        i.buffers.kill(id, |s| obarray.symbol(s).always_local)
    };
    if !killed {
        return false;
    }
    // GNU kill_buffer calls replace_buffer_in_windows: every window
    // still showing the dead buffer switches to its own prev-buffers
    // history first, then `other-buffer' (per-window choice, GNU's
    // replace_buffer_in_windows loop).
    let fallback = other_buffer_visible(i, id)
        .or_else(|| i.buffers.list().first().copied())
        .unwrap_or_else(|| i.buffers.create("*scratch*"));
    let mut sel_next: Option<usize> = None;
    for f in i.frames.clone() {
        let sel = f.borrow().selected.borrow().id;
        for w in &f.borrow().windows {
            if !w.borrow().dead && !w.borrow().minibuffer && w.borrow().buffer == id {
                let pick = window_prev_live(i, w, id).unwrap_or(fallback);
                w.borrow_mut().buffer = pick;
                if w.borrow().id == sel {
                    sel_next = Some(pick);
                }
            }
        }
    }
    // GNU kill_buffer → record_unwind/delete_from_list: every window's
    // prev-buffers/next-buffers/quit-restore params purge the dead
    // buffer.  Without this, `switch-to-prev-buffer' sees stale dead
    // entries and `other-buffer' ordering drifts (the
    // exit-splash-screen / generate-new-buffer-name cascade).
    for f in i.frames.clone() {
        for w in &f.borrow().windows {
            if w.borrow().dead {
                continue;
            }
            let mut wb = w.borrow_mut();
            for slot in ["prev_buffers", "next_buffers"] {
                let cur_list = if slot == "prev_buffers" {
                    wb.prev_buffers.clone()
                } else {
                    wb.next_buffers.clone()
                };
                if let Ok(items) = cur_list.list_to_vec() {
                    let kept: Vec<Value> = items
                        .into_iter()
                        .filter(|e| {
                            let bid = match e {
                                Value::Cons(c) => {
                                    if let Value::Buffer(b) = &c.borrow().car {
                                        Some(b.borrow().id)
                                    } else {
                                        None
                                    }
                                }
                                Value::Buffer(b) => Some(b.borrow().id),
                                _ => None,
                            };
                            bid != Some(id)
                        })
                        .collect();
                    let nl = Value::list(kept);
                    if slot == "prev_buffers" {
                        wb.prev_buffers = nl;
                    } else {
                        wb.next_buffers = nl;
                    }
                }
            }
        }
    }
    if i.current_buffer == id {
        // GNU set_buffer_internal in quit/kill paths: the selected
        // window's replacement buffer becomes current.
        i.set_current_buffer(sel_next.unwrap_or(fallback));
    }
    run_buffer_list_update(i, id);
    true
}

/// GNU `kill_buffer' minus the interactive/autosave questions:
/// query functions + `kill-buffer-hook' (with the doomed buffer
/// current), then — for a base buffer — recursively kill indirect
/// children, then teardown.  Returns Ok(false) when a query function
/// vetoes the kill.
fn kill_buffer_impl(i: &mut Interp, id: usize) -> Result<bool, Flow> {
    // GNU Fkill_buffer: killing an already-dead buffer is a no-op.
    match i.buffers.get(id) {
        Some(b) if b.borrow().live => {}
        _ => return Ok(false),
    }
    let inhibit = i
        .buffers
        .get(id)
        .map(|b| b.borrow().inhibit_buffer_hooks)
        .unwrap_or(false);
    // GNU Fkill_buffer: first run `kill-buffer-query-functions' (each
    // returning nil aborts the kill), then run the buffer-local
    // `kill-buffer-hook' with the doomed buffer current — all skipped
    // for buffers created with INHIBIT-BUFFER-HOOKS.
    let prev = i.current_buffer;
    i.set_current_buffer(id);
    let outcome = (|| -> Result<bool, Flow> {
        if inhibit {
            return Ok(true);
        }
        let q = i.intern("kill-buffer-query-functions");
        if matches!(i.symbol_value(q), Value::Cons(_)) {
            let runf = i.intern("run-hook-with-args-until-failure");
            let r = i.apply(&Value::Sym(runf), vec![Value::Sym(q)])?;
            if r.is_nil() {
                return Ok(false);
            }
        }
        let h = i.intern("kill-buffer-hook");
        let rh = i.intern("run-hooks");
        i.apply(&Value::Sym(rh), vec![Value::Sym(h)])?;
        Ok(true)
    })();
    if i.current_buffer == id && i.buffers.get(prev).is_some() {
        i.set_current_buffer(prev);
    }
    match outcome {
        Ok(true) => {}
        Ok(false) => {
            if std::env::var_os("REMACS_TRACE_KILL").is_some() {
                let n = i.buffers.get(id).map(|b| b.borrow().name.clone());
                eprintln!("[kill refusal query {:?}]", n);
            }
            return Ok(false);
        }
        Err(e) => return Err(e),
    }
    // A hook may have killed the buffer already.
    if !i.buffers.get(id).map(|b| b.borrow().live).unwrap_or(false) {
        return Ok(true);
    }
    // GNU kill_buffer: an ordinary buffer sharing its text with
    // indirect buffers kills them too — before the base's own
    // teardown, so their query functions/hooks still run; a child
    // that refuses to die aborts the whole kill with an error.
    let is_indirect = i
        .buffers
        .get(id)
        .map(|b| b.borrow().base_buffer.is_some())
        .unwrap_or(false);
    if !is_indirect {
        let children: Vec<usize> = i
            .buffers
            .list()
            .into_iter()
            .filter(|&cid| {
                i.buffers
                    .get(cid)
                    .and_then(|cb| cb.borrow().base_buffer.as_ref().map(|br| br.borrow().id))
                    == Some(id)
            })
            .collect();
        for cid in children {
            let cname = i
                .buffers
                .get(cid)
                .map(|cb| cb.borrow().name.clone())
                .unwrap_or_default();
            kill_buffer_impl(i, cid)?;
            if i.buffers.get(cid).is_some() {
                return Err(i.error(format!(
                    "Unable to kill buffer whose indirect buffer ‘{}’ cannot be killed",
                    cname
                )));
            }
        }
        // GNU: a child's hook may have killed the base (Bug#11665).
        if i.buffers.get(id).is_none() {
            return Ok(true);
        }
    }
    // GNU kill_buffer refusal path (buffer.c): a buffer shown in the
    // minibuffer window can't be killed; when the victim is current,
    // make another buffer current first — if there is nowhere to go
    // (`other_buffer' can only offer the victim itself), refuse.
    // `kill-matching-buffers' depends on this: the last non-hidden
    // buffer survives (GNU leaves *scratch*).
    let minibuf_buf = i
        .frames
        .iter()
        .find_map(|f| f.borrow().minibuffer.as_ref().map(|w| w.borrow().buffer));
    if minibuf_buf == Some(id) {
        if std::env::var_os("REMACS_TRACE_KILL").is_some() {
            let n = i.buffers.get(id).map(|b| b.borrow().name.clone());
            eprintln!("[kill refusal minibuf {:?}]", n);
        }
        return Ok(false);
    }
    if i.current_buffer == id {
        let other = other_buffer_visible(i, id)
            .or_else(|| i.buffers.other(id))
            .unwrap_or_else(|| i.buffers.create("*scratch*"));
        i.set_current_buffer(other);
        if i.current_buffer == id {
            if std::env::var_os("REMACS_TRACE_KILL").is_some() {
                let n = i.buffers.get(id).map(|b| b.borrow().name.clone());
                eprintln!("[kill refusal sole {:?}]", n);
            }
            return Ok(false);
        }
    }
    // GNU: if the buffer now current is the minibuffer's and the
    // victim is the sole remaining buffer, give up.
    if minibuf_buf == Some(i.current_buffer) && other_buffer_visible(i, id) == Some(id) {
        if std::env::var_os("REMACS_TRACE_KILL").is_some() {
            let n = i.buffers.get(id).map(|b| b.borrow().name.clone());
            eprintln!("[kill refusal minibuf-sole {:?}]", n);
        }
        return Ok(false);
    }
    Ok(kill_buffer_teardown(i, id))
}

/// Kill `id` and repair `current_buffer`: GNU always has a live
/// buffer, so killing the last one yields a fresh *scratch*.
pub(crate) fn kill_buffer_keep_current(i: &mut Interp, id: usize) -> Result<bool, Flow> {
    kill_buffer_impl(i, id)
}

fn f_kill_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let buffer = buf_or_name(i, &arg(&a, 0))?;
    let id = buffer.borrow().id;
    Ok(Value::from_bool(kill_buffer_impl(i, id)?))
}

fn f_buffer_list(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let ids = i.buffers.list();
    Ok(Value::list(
        ids.iter().filter_map(|id| i.buffer_value(*id)).collect(),
    ))
}

/// First live buffer in WINDOW's `prev_buffers' history (GNU
/// `switch_to_prev_or_next_buffer' prefers it over other_buffer).
/// Entries are (BUFFER [POS...]); dead/killed buffers are skipped.
pub(crate) fn window_prev_live(
    i: &Interp,
    w: &crate::editor::WindowRef,
    exclude: usize,
) -> Option<usize> {
    let prev = w.borrow().prev_buffers.clone();
    let items = prev.list_to_vec().ok()?;
    for e in items {
        let bv = match &e {
            Value::Cons(c) => c.borrow().car.clone(),
            other => other.clone(),
        };
        if let Some(id) = i.buffer_id_of(&bv) {
            if id != exclude && i.buffers.get(id).map(|b| b.borrow().live).unwrap_or(false) {
                return Some(id);
            }
        }
    }
    None
}

/// GNU `other_buffer' with VISIBLE-OK nil: first live non-space
/// buffer not shown in any window of the frame, else first live
/// non-space buffer.  Shared by kill/bury/replace selection paths.
pub(crate) fn other_buffer_visible(i: &Interp, exclude: usize) -> Option<usize> {
    let mut shown = std::collections::HashSet::new();
    for f in &i.frames {
        for w in &f.borrow().windows {
            shown.insert(w.borrow().buffer);
        }
    }
    i.buffers.other_visible_pref(exclude, &shown, false)
}

fn f_other_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fother_buffer
    // (BUFFER VISIBLE-OK FRAME): ignored BUFFER unless live; FRAME goes
    // through decode_live_frame — a string/int signals frame-live-p.
    match arg(&a, 2) {
        Value::Nil | Value::Frame(_) => {}
        other => return Err(i.wrong_type_mut("frame-live-p", &other)),
    }
    let visible_ok = arg(&a, 1).truthy();
    // GNU: BUFFER nil means NO exclusion (not the current buffer).
    let exclude = a
        .get(0)
        .and_then(|v| i.buffer_id_of(v))
        .unwrap_or(usize::MAX);
    let pick = if visible_ok {
        i.buffers.other(exclude)
    } else {
        other_buffer_visible(i, exclude)
    };
    match pick {
        Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}

fn f_set_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: Fget_buffer signals `stringp' for non-buffer/non-name args;
    // a missing name is a plain "No buffer named" error.
    if !matches!(a[0], Value::Buffer(_) | Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
    // GNU Fset_buffer: selecting a killed buffer signals a plain error.
    if let Value::Buffer(b) = &a[0] {
        if !b.borrow().live {
            return Err(i.error("Selecting deleted buffer"));
        }
    }
    let id = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[0]))))?;
    i.set_current_buffer(id);
    // GNU Fset_buffer returns the BUFFER object, not the argument —
    // a string name resolves to its buffer.
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

fn f_rename_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if name.is_empty() {
        return Err(i.error("Empty string is invalid as a buffer name"));
    }
    let unique = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let actual = if unique {
        i.buffers.rename(i.current_buffer, &name)
    } else {
        // GNU: renaming to the buffer's OWN name is fine — the clash
        // check excludes the buffer itself.
        match i.buffers.by_name(&name) {
            Some(other) if other != i.current_buffer => {
                return Err(i.error_q("Buffer name `%s' is in use", &[&name]));
            }
            _ => {}
        }
        i.buffers.rename(i.current_buffer, &name)
    };
    // GNU buffer.c: when `uniquify-buffer-name-style' is non-nil, the
    // subr hands the rename to `uniquify--rename-buffer-advice' (which
    // may rename the buffer again, adding directory components).  The
    // variable is preset non-nil here, so guard on fboundp too.
    let ustyle = i.intern_soft("uniquify-buffer-name-style");
    if ustyle.map(|s| i.symbol_value(s).truthy()).unwrap_or(false) {
        let adv = i.intern("uniquify--rename-buffer-advice");
        if i.fbound_p(adv) {
            i.apply(
                &Value::Sym(adv),
                vec![
                    Value::string(name.clone()),
                    if unique { Value::t() } else { Value::Nil },
                ],
            )?;
            // The advice may have renamed the buffer again.
            if let Some(cur) = i.buffers.get(i.current_buffer) {
                return Ok(Value::string(cur.borrow().name.clone()));
            }
            return Ok(Value::string(actual));
        }
    }
    Ok(Value::string(actual))
}

fn f_bury_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el `bury-buffer': bury-buffer-internal the target,
    // then — only when called with no arg AND the buffer is shown in
    // the selected window — (window--delete nil t), else
    // (switch-to-prev-buffer nil 'bury).  Batch windows can't be
    // deleted (window--delete fails on the last window), so the
    // switch always runs: prev_buffers first, `other-buffer'
    // fallback — GNU's replace_buffer_in_windows_safely/switch path.
    let (id, explicit) = match a.get(0) {
        Some(v) if v.truthy() => (
            // GNU window-normalize-buffer: a string naming no live
            // buffer or a dead buffer is "No such live buffer %s";
            // other objects are "No such buffer %s".
            match v {
                Value::Buffer(b) => {
                    let id = b.borrow().id;
                    if !i.buffer_live(id) {
                        return Err(
                            i.error(format!("No such live buffer {}", i.princ_to_string(v)))
                        );
                    }
                    id
                }
                Value::Str(s) => match i.buffers.by_name(&s.borrow()) {
                    Some(id) => id,
                    None => return Err(i.error(format!("No such live buffer {}", s.borrow()))),
                },
                other => {
                    return Err(i.error(format!("No such buffer {}", i.princ_to_string(other))));
                }
            },
            true,
        ),
        _ => (i.current_buffer, false),
    };
    i.buffers.bury(id);
    // GNU bury-buffer-internal runs `buffer-list-update-hook'.
    run_buffer_list_update(i, id);
    if !explicit {
        let shown = crate::editor::sel_window(i)
            .map(|w| w.borrow().buffer == id)
            .unwrap_or(id == i.current_buffer);
        if shown {
            if let Some(other) = other_buffer_visible(i, id) {
                let target = i.buffer_value(other).unwrap_or(Value::Nil);
                let _ = crate::editor::f_set_window_buffer(i, vec![Value::Nil, target].into());
                i.set_current_buffer(other);
            }
        }
    }
    Ok(Value::Nil)
}

fn f_bury_buffer_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_BUFFER: a buffer object, not a name — killed buffers
    // still qualify (bury is a no-op for them).
    let id = match &a[0] {
        Value::Buffer(b) => b.borrow().id,
        _ => return Err(i.wrong_type_mut("bufferp", &a[0])),
    };
    if i.buffer_live(id) {
        i.buffers.bury(id);
        run_buffer_list_update(i, id);
    }
    Ok(Value::Nil)
}

fn f_unbury_buffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU window.el: (switch-to-buffer (last-buffer)) where
    // `last-buffer' scans the reversed buffer list through
    // `get-next-valid-buffer': first live buffer whose name does not
    // start with a space, that is not the current buffer, and (unless
    // VISIBLE-OK) is not shown in any window.  The pick becomes
    // current and is shown in the selected window (which re-touches
    // it to the front).  With no candidate GNU falls back to
    // `get-scratch-buffer-create'.
    let cur = i.current_buffer;
    let win_buf = i
        .frames
        .iter()
        .flat_map(|f| f.borrow().windows.iter().cloned().collect::<Vec<_>>())
        .map(|w| w.borrow().buffer)
        .collect::<Vec<_>>();
    let pick = i
        .buffers
        .order
        .iter()
        .rev()
        .copied()
        .find(|&id| {
            if id == cur || win_buf.contains(&id) {
                return false;
            }
            i.buffers
                .get(id)
                .map(|b| {
                    let bb = b.borrow();
                    bb.live && !bb.name.starts_with(' ')
                })
                .unwrap_or(false)
        });
    match pick {
        Some(id) => {
            let target = i.buffer_value(id).unwrap_or(Value::Nil);
            let _ = crate::editor::f_set_window_buffer(i, vec![Value::Nil, target].into());
            i.set_current_buffer(id);
            Ok(i.buffer_value(id).unwrap_or(Value::Nil))
        }
        None => Ok(Value::Nil),
    }
}

fn f_buffer_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    // GNU Fbuffer_size returns Z - BEG: total chars, ignoring narrowing.
    let bb = b.borrow();
    Ok(Value::Int(bb.size() as i128))
}

fn f_buffer_modified_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    Ok(Value::from_bool(b.borrow().is_modified()))
}

pub(crate) fn f_set_buffer_modified_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let flag = a[0].truthy();
    if flag {
        barf_if_file_locked(i)?;
    }
    cur(i).borrow_mut().note_modified(flag);
    Ok(Value::Nil)
}

/// `restore-buffer-modified-p' — same effect, but GNU returns FLAG.
pub(crate) fn f_restore_buffer_modified_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let flag = a[0].truthy();
    if flag {
        barf_if_file_locked(i)?;
    }
    cur(i).borrow_mut().note_modified(flag);
    Ok(a[0].clone())
}

fn f_not_modified(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let flag = arg(&a, 0).truthy();
    if flag {
        barf_if_file_locked(i)?;
    }
    cur(i).borrow_mut().note_modified(flag);
    Ok(Value::Nil)
}

fn f_buffer_file_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let bb = b.borrow();
    // BVAR: a Lisp-level local binding shadows the slot (and is where
    // plain `setq' lands); no binding → the slot itself.
    if i.bfn_sym != 0 {
        if let Some(v) = bb.locals.get(&i.bfn_sym) {
            return Ok(v.clone());
        }
    }
    match &bb.file_name {
        Some(f) => Ok(Value::string(f.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_buffer_base_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    // GNU Fbuffer_base_buffer: returns the base buffer OBJECT — even
    // when it was killed (`#<killed buffer>' prints).  The Rc in
    // `base_buffer' keeps the dead object reachable.
    let base = b.borrow().base_buffer.clone();
    Ok(base.map(Value::Buffer).unwrap_or(Value::Nil))
}

/// (make-indirect-buffer BASE-BUFFER NAME &optional CLONE
/// INHIBIT-BUFFER-HOOKS) — create a buffer sharing BASE-BUFFER's text.
/// GNU order: CHECK_STRING name → name-in-use → resolve base →
/// dead-base error → empty-name error.  The new buffer always takes
/// the ultimate base (no double indirection) and always copies
/// PT/BEGV/ZV; CLONE additionally copies locals and the mark.
fn f_make_indirect_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks NAME first (CHECK_STRING), then its uniqueness.
    let name = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if i.buffers.by_name(&name).is_some() {
        return Err(i.error_q("Buffer name `%s' is in use", &[&name]));
    }
    let base = match &a[0] {
        Value::Buffer(b) => b.clone(),
        // GNU: a string names a LIVE buffer via get-buffer — nil →
        // plain `error' "No such buffer: ‘X’".
        Value::Str(s) => match i
            .buffers
            .by_name(&s.borrow())
            .and_then(|id| i.buffers.get(id))
        {
            Some(b) => b,
            None => return Err(i.error_q("No such buffer: `%s'", &[s.borrow().as_str()])),
        },
        // GNU resolves base through Fget_buffer: a non-buffer/non-name
        // errors `stringp' there.
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if !base.borrow().live {
        return Err(i.error("Base buffer has been killed"));
    }
    if name.is_empty() {
        return Err(i.error("Empty string for buffer name is not allowed"));
    }
    let clone = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let inhibit_hooks = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    // No double indirection: indirect-of-indirect takes the real base
    // (GNU `b->base_buffer = XBUFFER(base)->base_buffer ?: base').
    let base = {
        let bb = base.borrow();
        bb.base_buffer.clone().unwrap_or_else(|| base.clone())
    };
    crate::lisp::builtins::misc::heal_standard_case_table(i);
    let id = i.buffers.create_exact(&name);
    {
        let bb = base.borrow();
        let nb = i.buffers.get(id).unwrap();
        let mut n = nb.borrow_mut();
        n.text = bb.text.clone();
        n.text.add_viewer(&nb);
        n.base_buffer = Some(base.clone());
        // GNU copies PT/BEGV/ZV unconditionally — a fresh indirect
        // buffer opens at the base's point and narrowing.
        n.point = bb.point;
        n.begv = bb.begv;
        n.zv = bb.zv;
        n.inhibit_buffer_hooks = inhibit_hooks;
        // Bug#18180: the indirect buffer shares the base's undo list
        // (in GNU the value is copied at creation and re-synced on
        // every buffer switch — see set_current_buffer).
        n.set_undo_list(bb.undo_list());
        if clone {
            // GNU clone_per_buffer_values: locals travel, but the
            // indirect buffer cannot visit a file.
            if let Some(mp) = bb.mark_position() {
                n.set_mark(mp);
            }
            n.locals = bb.locals.clone();
            n.file_name = None;
            for s in [
                "buffer-file-name",
                "buffer-file-truename",
                "buffer-auto-save-file-name",
                "buffer-backed-up",
                "local-minor-modes",
                "buffer-save-without-query",
                "buffer-file-number",
            ] {
                n.locals.remove(&i.intern(s));
            }
            n.locals
                .insert(i.intern("buffer-display-count"), Value::Int(0));
        }
    }
    if clone {
        // GNU: cloned buffers run `clone-indirect-buffer-hook' so
        // e.g. faces can make indirect-safe copies.
        let h = i.intern("clone-indirect-buffer-hook");
        let nb = i.buffers.get(id).unwrap();
        if !nb.borrow().inhibit_buffer_hooks {
            let prev = i.current_buffer;
            i.set_current_buffer(id);
            let rh = i.intern("run-hooks");
            let r = i.apply(&Value::Sym(rh), vec![Value::Sym(h)]);
            if i.current_buffer == id && i.buffers.get(prev).is_some() {
                i.set_current_buffer(prev);
            }
            r?;
        }
    }
    run_buffer_list_update(i, id);
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

/// GNU `clone-indirect-buffer'/`clone-buffer': a buffer on the current
/// buffer with state cloned (locals, point, narrowing); the name is
/// uniquified like `generate-new-buffer-name'.  `share' selects text
/// aliasing (`clone-indirect-buffer') vs an independent copy
/// (`clone-buffer').
fn clone_buffer(i: &mut Interp, name: &str, share: bool) -> Result<Value, Flow> {
    // GNU clone_indirect_buffer → Fmake_indirect_buffer(BASE of the
    // current buffer, NAME, CLONE=t): when the current buffer is
    // itself indirect, per-buffer values are cloned from its BASE,
    // not from the indirect buffer.
    let base = {
        let cb = cur(i);
        let bb = cb.borrow();
        bb.base_buffer.clone().unwrap_or_else(|| cb.clone())
    };
    crate::lisp::builtins::misc::heal_standard_case_table(i);
    // Both GNU entry points (clone-buffer → generate-new-buffer,
    // clone-indirect-buffer → make-indirect-buffer) uniquify NAME
    // through generate-new-buffer-name — `-<rand>' for space names.
    let name = gen_unique_buffer_name(i, name, None)?;
    // `gen_unique_buffer_name' already resolved collisions —
    // `buffers.create' would uniquify a second time.
    let id = i.buffers.create_exact(&name);
    {
        let bb = base.borrow();
        let nb = i.buffers.get(id).unwrap();
        let mut n = nb.borrow_mut();
        n.text = if share {
            bb.text.clone()
        } else {
            bb.text.deep_clone()
        };
        n.text.add_viewer(&nb);
        // GNU: `buffer-base-buffer' is set only when text is shared —
        // `clone-buffer' (independent copy) reports nil.
        n.base_buffer = if share { Some(base.clone()) } else { None };
        n.point = bb.point;
        if let Some(mp) = bb.mark_position() {
            n.set_mark(mp);
        }
        n.begv = bb.begv;
        n.zv = bb.zv;
        n.locals = bb.locals.clone();
        n.set_undo_list(bb.undo_list());
        // An indirect buffer cannot visit a file.
        if share {
            n.file_name = None;
        } else {
            n.file_name = bb.file_name.clone();
        }
    }
    if share {
        let nb = i.buffers.get(id).unwrap();
        if !nb.borrow().inhibit_buffer_hooks {
            // GNU runs `clone-indirect-buffer-hook' in the new buffer.
            let h = i.intern("clone-indirect-buffer-hook");
            let prev = i.current_buffer;
            i.set_current_buffer(id);
            let rh = i.intern("run-hooks");
            let r = i.apply(&Value::Sym(rh), vec![Value::Sym(h)]);
            if i.current_buffer == id && i.buffers.get(prev).is_some() {
                i.set_current_buffer(prev);
            }
            r?;
        }
    }
    run_buffer_list_update(i, id);
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

fn f_clone_indirect_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: NAME nil →
    // the base buffer's own name (same default as clone-buffer);
    // uniquified by generate-new-buffer-name either way.
    let name = match a.first() {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(other) if !other.is_nil() => return Err(i.wrong_type_mut("stringp", other)),
        _ => cur(i).borrow().name.clone(),
    };
    clone_buffer(i, &name, true)
}

fn f_clone_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: NEWNAME defaults to the current buffer's name; the result
    // is uniquified in either case.
    let name = match a.first() {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(other) if !other.is_nil() => return Err(i.wrong_type_mut("stringp", other)),
        _ => cur(i).borrow().name.clone(),
    };
    clone_buffer(i, &name, false)
}
/// GNU `Fset_buffer_major_mode' (buffer.c): the mode function is chosen
/// from `initial-major-mode' for `*scratch*', else the global default
/// of `major-mode' — falling back to the current buffer's own value
/// when the default is nil and the current mode has no `mode-class'
/// property.  A nil function just returns nil (no fundamental run).
/// BUFFER goes through CHECK_BUFFER (strings rejected, unlike the old
/// prelude defun's get-buffer resolution).
fn f_set_buffer_major_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &a[0])?;
    if !b.borrow().live {
        return Err(i.error("Attempt to set major mode for a dead buffer"));
    }
    let mm = i.intern("major-mode");
    let is_scratch = b.borrow().name == "*scratch*";
    let function = if is_scratch {
        let init_id = i.intern("initial-major-mode");
        i.symbol_value(init_id)
    } else {
        let dflt = i.default_toplevel_value(mm).unwrap_or(Value::Nil);
        if dflt.is_nil() {
            let mc = i.intern("mode-class");
            let cur_mode = {
                let cb = cur(i);
                cb.borrow().locals.get(&mm).cloned().unwrap_or(Value::Nil)
            };
            match &cur_mode {
                Value::Sym(s) if i.get_prop(*s, mc).is_nil() => cur_mode,
                _ => Value::Nil,
            }
        } else {
            dflt
        }
    };
    if function.is_nil() {
        return Ok(Value::Nil);
    }
    // GNU: record_unwind_current_buffer + Fset_buffer(BUFFER) +
    // call0(function).  Restore the previous current buffer even when
    // the mode function exits non-locally.
    let prev = i.current_buffer;
    if let Some(id) = i.buffer_id_of(&a[0]) {
        i.set_current_buffer(id);
    }
    let res = i.apply(&function, vec![]);
    if i.current_buffer != prev && i.buffers.get(prev).is_some() {
        i.set_current_buffer(prev);
    }
    res?;
    Ok(Value::Nil)
}


fn f_buffer_local_variables(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let (locals, file_name) = {
        let bb = b.borrow();
        (bb.locals.clone(), bb.file_name.clone())
    };
    // GNU buffer.c: the DEFVAR_PER_BUFFER slots live in every buffer,
    // so `buffer-local-variables' reports them (with the slot's
    // effective value) even when Lisp never setq'd them. remacs marks
    // those symbols `always_local' — merge them into the reported
    // list, value = real local, else `buffer-file-name'/global default.
    let bfn = i.intern("buffer-file-name");
    let always: Vec<SymId> = i
        .obarray
        .all_ids()
        .into_iter()
        .filter(|id| {
            let s = i.obarray.symbol(*id);
            s.always_local && !locals.contains_key(id)
        })
        .collect();
    let mut pairs: Vec<Value> = locals
        .iter()
        .map(|(s, v)| Value::cons(i.sym(*s), v.clone()))
        .collect();
    for id in always {
        let val = if id == bfn {
            match &file_name {
                Some(f) => Value::string(f.clone()),
                None => Value::Nil,
            }
        } else {
            let v = i.obarray.symbol(id).value.clone();
            if matches!(v, Value::Sym(s) if s == sym::UNBOUND) {
                Value::Nil
            } else {
                v
            }
        };
        pairs.push(Value::cons(i.sym(id), val));
    }
    Ok(Value::list(pairs))
}

fn f_buffer_local_value(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // Unlike optional buffer arguments, nil is not the current buffer.
    if !matches!(a[1], Value::Buffer(_)) {
        return Err(i.wrong_type_mut("bufferp", &a[1]));
    }
    let b = buf_of(i, &a[1])?;
    if let Some(v) = b.borrow().locals.get(&sid) {
        if !matches!(v, Value::Sym(s) if *s == sym::UNBOUND) {
            return Ok(v.clone());
        }
    }
    let v = i.obarray.symbol(sid).value.clone();
    if matches!(v, Value::Sym(s) if s == sym::UNBOUND) {
        return Err(i.signal_data(sym::VOID_VARIABLE, vec![a[0].clone()]));
    }
    Ok(v)
}

fn f_buffer_local_boundp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    if !matches!(a[1], Value::Buffer(_)) {
        return Err(i.wrong_type_mut("bufferp", &a[1]));
    }
    let b = buf_of(i, &a[1])?;
    // GNU subr.el: `buffer-local-boundp' is (condition-case nil
    //   (buffer-local-value SYMBOL BUFFER) (:success t)
    //   (void-variable nil)) — it reports boundness in BUFFER, not
    //   whether the binding is local.  A `defvar'd variable is t even
    //   with no local binding.
    let v = b
        .borrow()
        .locals
        .get(&sid)
        .cloned()
        .unwrap_or_else(|| i.obarray.symbol(sid).value.clone());
    let bound = !matches!(v, Value::Sym(s) if s == sym::UNBOUND);
    Ok(Value::from_bool(bound))
}

fn f_make_local_variable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // Seed with the CURRENT effective value: for BVAR-style variables
    // (`buffer-file-name') the buffer slot is the binding even before
    // `locals' has an entry, so the default cell would seed wrongly.
    let seed = i.symbol_value(sid);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if !bb.locals.contains_key(&sid) {
        // The local binding starts with the default value, or void
        // (the unbound sentinel) when the variable is unbound.
        bb.locals.insert(sid, seed);
    }
    Ok(a[0].clone())
}

fn f_make_variable_buffer_local(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // GNU: the variable gains a buffer-local cell in every buffer
    // initialized to the default; an unbound default becomes nil, so
    // `boundp' is t right away (verified on 31.1).
    if matches!(
        i.obarray.symbol(sid).value,
        Value::Sym(s) if s == sym::UNBOUND
    ) {
        i.obarray.symbol_mut(sid).value = Value::Nil;
    }
    i.obarray.symbol_mut(sid).make_local_if_set = true;
    Ok(a[0].clone())
}

fn f_kill_local_variable(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // `buffer-file-name' is a BVAR: GNU's kill edits the buffer's
    // local-vars alist where the slot variable never appears — a
    // no-op (value AND `local-variable-p' survive; verified on 31.1).
    if sid != i.bfn_sym {
        cur(i).borrow_mut().locals.remove(&sid);
    }
    Ok(a[0].clone())
}

pub(crate) fn f_kill_all_local_variables(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU reset_buffer_local_variables also resets the buffer's
    // syntax-table, case-table and category-table fields to the
    // standard objects (buffer slots, not Lisp locals, so they
    // survive `locals' cleanup otherwise).
    // KILL-PERMANENT non-nil clears even `permanent-local' bindings.
    if a.first().map(Value::truthy).unwrap_or(false) {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        // `buffer-file-name' is a C buffer slot, not a resettable
        // local: GNU keeps it across even `kill-all-local-variables t'
        // (verified on 31.1).  Re-mirror the slot after clearing.
        let keep_bfn = bb.file_name.clone();
        bb.locals.clear();
        if i.bfn_sym != 0 {
            if let Some(f) = keep_bfn {
                bb.locals.insert(i.bfn_sym, Value::string(f));
            }
        }
        bb.syntax_table = None;
        bb.case_table = None;
        bb.category_table = None;
        return Ok(Value::Nil);
    }
    let keep: Vec<u32> = {
        let b = cur(i);
        let bb = b.borrow();
        bb.locals
            .keys()
            .copied()
            .filter(|s| {
                i.get_prop(*s, i.intern_soft("permanent-local").unwrap_or(u32::MAX))
                    .truthy()
            })
            .collect()
    };
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let mut keep_set: std::collections::HashSet<u32> = keep.into_iter().collect();
    // `buffer-file-name' is a C buffer slot (BVAR), not a real local:
    // GNU preserves it across `kill-all-local-variables' (verified on
    // 31.1 — value and `local-variable-p' both survive).
    if bb.file_name.is_some() && i.bfn_sym != 0 {
        keep_set.insert(i.bfn_sym);
    }
    bb.locals.retain(|s, _| keep_set.contains(s));
    bb.syntax_table = None;
    bb.case_table = None;
    bb.category_table = None;
    Ok(Value::Nil)
}

fn f_local_variable_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Flocal_variable_p' resolves BUFFER (arg1, live
    // buffer-or-nil) BEFORE checking SYMBOL — `(local-variable-p "a" "b")'
    // signals `bufferp', not `symbolp'.
    let b = buf_of(i, &arg(&a, 1))?;
    let sid = want_sym(i, &a[0])?;
    // GNU `DEFVAR_PER_BUFFER' variables are local in every buffer.
    // `make-variable-buffer-local' (blv `local_if_set') is NOT consulted:
    // only a concrete buffer-local binding (or per-buffer C var) counts.
    if i.obarray.symbol(sid).always_local {
        return Ok(Value::t());
    }
    Ok(Value::from_bool(b.borrow().locals.contains_key(&sid)))
}
fn f_local_variable_if_set_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `Flocal_variable_if_set_p': the
    // SYMBOL check runs FIRST and BUFFER is only decoded for symbols
    // that actually hold a buffer-local slot — `(local-variable-if-set-p
    // 'x 1)' returns nil without validating `1' (a PLAINVAL symbol
    // shortcuts before the buffer is touched).
    let orig = want_sym(i, &a[0])?;
    let sid = i.var_alias_target(orig);
    let sym = i.obarray.symbol(sid);
    if sym.make_local_if_set || sym.always_local {
        return Ok(Value::t());
    }
    // GNU SYMBOL_LOCALIZED check: only a symbol that can carry a
    // per-buffer binding reaches the buffer decode.  A plain variable
    // (no buffer-local bindings anywhere, not special, not an alias
    // target change) shortcuts to nil like GNU's SYMBOL_PLAINVAL.
    let localized = sym.special
        || sid != orig
        || i.buffers.list().iter().any(|bid| {
            i.buffers
                .get(*bid)
                .map(|b| b.borrow().locals.contains_key(&sid))
                .unwrap_or(false)
        });
    if !localized {
        return Ok(Value::Nil);
    }
    let b = buf_of(i, &arg(&a, 1))?;
    Ok(Value::from_bool(b.borrow().locals.contains_key(&sid)))
}

fn f_default_value(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    // Follow `defvaralias' chains like GNU's find_symbol_value.
    let sid = i.var_alias_target(sid);
    // GNU's `default-value' signals void-variable for a var without a
    // default binding — the internal UNBOUND marker must not escape.
    match i.obarray.symbol(sid).value {
        Value::Sym(s) if s == sym::UNBOUND => {
            Err(i.signal_data(sym::VOID_VARIABLE, vec![a[0].clone()]))
        }
        ref v => Ok(v.clone()),
    }
}

fn f_set_default_toplevel(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's data.c `Fset_default_toplevel_value' sets the default but
    // returns nil, unlike `set-default' which returns the value.
    f_set_default(i, a)?;
    Ok(Value::Nil)
}

fn f_set_default(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let sid = i.var_alias_target(sid);
    i.set_symbol_default(sid, a[1].clone())?;
    Ok(a[1].clone())
}

fn f_default_boundp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let sid = i.var_alias_target(sid);
    Ok(Value::from_bool(!matches!(
        i.obarray.symbol(sid).value,
        Value::Sym(s) if s == sym::UNBOUND
    )))
}

// ---------- case tables ----------

/// GNU `case-table-p': char-table with `case-table' purpose AND
/// consistent extras — up is nil-or-char-table; canon+eqv are either
/// both nil or canon is a char-table with eqv nil-or-char-table.
fn is_case_table(i: &Interp, v: &Value) -> bool {
    use crate::lisp::builtins::misc::{ct_extra, is_char_table};
    if !is_char_table(i, v)
        || !matches!(
            v,
            Value::Record(r)
                if matches!(r.borrow().get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "case-table")
        )
    {
        return false;
    }
    let up = ct_extra(v, 0);
    let canon = ct_extra(v, 1);
    let eqv = ct_extra(v, 2);
    (up.is_nil() || is_char_table(i, &up))
        && ((canon.is_nil() && eqv.is_nil())
            || (is_char_table(i, &canon) && (eqv.is_nil() || is_char_table(i, &eqv))))
}

fn want_case_table(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    if is_case_table(i, v) {
        Ok(())
    } else {
        Err(i.wrong_type_mut("case-table-p", v))
    }
}

fn f_current_case_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.current_case_table())
}

fn f_standard_case_table(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(i.standard_case_table())
}

fn f_case_table_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_case_table(i, &a[0])))
}

fn f_set_case_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_case_table(i, &a[0])?;
    // GNU `set_case_table': nil UPCASE/CANON/EQV extras are derived
    // from the downcase table here (casetab.c), not lazily later.
    crate::lisp::builtins::misc::set_case_table_fill(i, &a[0]);
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if let Ok(mut bb) = b.try_borrow_mut() {
            bb.case_table = Some(a[0].clone());
        }
    }
    Ok(a[0].clone())
}

fn f_set_standard_case_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_case_table(i, &a[0])?;
    crate::lisp::builtins::misc::set_case_table_fill(i, &a[0]);
    i.standard_case_table = Some(a[0].clone());
    Ok(a[0].clone())
}

fn f_copy_case_table(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr (Emacs 31): copy = copy-sequence(case-table); extras[0]
    // = copy-sequence(upcase); extras[1..2] cleared so they recompute
    // from the new downcase table (the case-table.el defun documents
    // the same algorithm).
    want_case_table(i, &a[0])?;
    let Value::Record(r) = &a[0] else {
        return Err(i.wrong_type_mut("case-table-p", &a[0]));
    };
    let up = r.borrow().get(3).cloned().unwrap_or(Value::Nil);
    let copy = crate::lisp::builtins::misc::ct_copy(i, &a[0]);
    if let Value::Record(cr) = &copy {
        let mut rr = cr.borrow_mut();
        if rr.len() > 5 {
            rr[4] = Value::Nil;
            rr[5] = Value::Nil;
            if !up.is_nil() {
                rr[3] = crate::lisp::builtins::misc::ct_copy(i, &up);
            }
        }
    }
    Ok(copy)
}

fn f_buffer_disable_undo(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU defun: (with-current-buffer buffer (setq buffer-undo-list t))
    let b = buf_or_name(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    bb.set_undo_list(Value::t());
    Ok(Value::t())
}

fn f_buffer_enable_undo(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr: reset buffer-undo-list to nil only when disabled (t).
    let b = buf_or_name(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    if bb.undo_disabled() {
        bb.set_undo_list(Value::Nil);
    }
    Ok(Value::Nil)
}

// ---------- point & motion ----------

fn f_point(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().point() as i128 + 1))
}

fn f_point_min(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().begv as i128 + 1))
}

fn f_point_max(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().text_len() as i128 + 1))
}

fn f_goto_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => {
            let (pos, buf) = {
                let mm = m.borrow();
                (mm.position, mm.buffer)
            };
            // GNU Fgoto_char: a detached marker errors; a marker in
            // another buffer contributes only its position.
            if buf.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            cur(i).borrow_mut().set_point(pos);
            // GNU returns the argument itself, marker object included.
            Ok(a[0].clone())
        }
        v => {
            let p = want_int(i, v)?;
            let b = cur(i);
            let mut bb = b.borrow_mut();
            let idx = pos_idx(bb.text.len(), p);
            // Emacs clamps to the accessible region.
            let idx = idx.max(bb.begv).min(bb.text_len());
            bb.point = idx;
            // GNU returns the ORIGINAL argument even when clamped.
            Ok(a[0].clone())
        }
    }
}

fn f_forward_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_fixnum(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let newp = bb.point() as i128 + n;
    let tlen = bb.text_len();
    if n > 0 && newp > tlen as i128 {
        bb.set_point(tlen);
        return Err(i.signal_data(sym::END_OF_BUFFER, vec![]));
    }
    if n < 0 && newp < bb.begv as i128 {
        let bv = bb.begv;
        bb.set_point(bv);
        return Err(i.signal_data(sym::BEGINNING_OF_BUFFER, vec![]));
    }
    let np = newp.max(bb.begv as i128) as usize;
    bb.set_point(np);
    Ok(Value::Nil)
}

fn f_backward_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_fixnum(i, &arg(&a, 0))?;
    f_forward_char(i, vec![Value::Int(-n)].into())
}

fn f_forward_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_fixnum(i, &arg(&a, 0))?;
    // GNU scan_words: word constituents = syntax class 'w' in the
    // buffer's syntax table.  Fetch the chain before borrowing the
    // buffer (the lookup itself touches the buffer).
    let syn = crate::editor::syntax_table_entries(i);
    let wordp = |c: char| crate::editor::syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // GNU scan_words: each iteration skips non-word constituents, then
    // scans a word.  If a boundary is reached before completing ARG
    // words, point still moves to the boundary and nil is returned;
    // t is returned only when all ARG words were traversed.
    let mut ok = true;
    if n >= 0 {
        let len = bb.text_len();
        for _ in 0..n {
            let mut p = bb.point();
            while p < len && !wordp(bb.text.char_at(p)) {
                p += 1;
            }
            if p == len {
                ok = false;
                bb.set_point(p);
                break;
            }
            while p < len && wordp(bb.text.char_at(p)) {
                p += 1;
            }
            bb.set_point(p);
        }
    } else {
        for _ in 0..-n {
            let mut p = bb.point();
            while p > bb.begv && !wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            if p == bb.begv {
                ok = false;
                bb.set_point(p);
                break;
            }
            while p > bb.begv && wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            bb.set_point(p);
        }
    }
    Ok(Value::from_bool(ok))
}

/// GNU count-arg decoders (editfns/cmds layer): nil -> 1, then the
/// per-command predicate.  `fixnump' rejects bignums AND markers;
/// `integerp' rejects markers; `number-or-marker-p' accepts markers.
/// nil -> 1 variants (GNU `&optional N' decoders that default BEFORE
/// checking): forward-word/forward-line/backward-word take nil as 1.
fn opt_fixnum(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    if matches!(v, Value::Nil) {
        return Ok(1);
    }
    crate::lisp::builtins::want_fixnum(i, v)
}
fn opt_integer(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    if matches!(v, Value::Nil) {
        return Ok(1);
    }
    crate::lisp::builtins::want_int(i, v)
}
fn opt_num_mark(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    if matches!(v, Value::Nil) {
        return Ok(1);
    }
    crate::editor::want_num_or_mark(i, v)
}
/// Strict variants: a nil argument signals the predicate, matching
/// GNU subrs whose optional arg is CHECK_*-decoded when present
/// (self-insert-command, delete-char, other-window, ...).
fn want_fixnum_nil_err(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    if matches!(v, Value::Nil) {
        return Err(i.wrong_type_mut("fixnump", v));
    }
    crate::lisp::builtins::want_fixnum(i, v)
}
fn want_num_mark_nil_err(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    crate::editor::want_num_or_mark(i, v)
}

fn f_backward_word(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_num_mark(i, &arg(&a, 0))?;
    f_forward_word(i, vec![Value::Int(-n)].into())
}

fn f_forward_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_integer(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.zv.max(bb.begv).min(bb.text_len());
    let mut pt = bb.point();
    let result: i128;
    if n == 0 {
        // GNU quirk: forward-line 0 moves to BOL of the current line.
        let line = bb.text.line_of_pos(pt);
        pt = bb.text.line_start(line);
        result = 0;
    } else if n > 0 {
        let mut remaining = n;
        while remaining > 0 && pt < len {
            let mut p = pt;
            let nl = loop {
                if p >= len {
                    break None;
                }
                if bb.text.char_at(p) == '\n' {
                    break Some(p);
                }
                p += 1;
            };
            match nl {
                Some(nl) => {
                    pt = nl + 1;
                    remaining -= 1;
                }
                // Reaching eob not preceded by a newline counts as a move.
                None => {
                    pt = len;
                    remaining -= 1;
                }
            }
        }
        result = remaining;
    } else {
        let cur_line = bb.text.line_of_pos(pt);
        let moved = cur_line.min((-n) as usize);
        pt = bb.text.line_start(cur_line - moved);
        result = n + moved as i128;
    }
    let begv = bb.begv;
    bb.set_point(pt.max(begv).min(len));
    Ok(Value::Int(result))
}

fn f_beginning_of_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Emacs: (beginning-of-line N) = forward-line(N-1) then BOL.
    // N <= 0 moves to the BOL of an earlier line (N=0 → previous line).
    let n = opt_fixnum(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n - 1).max(0) as usize;
    let p = bb.text.line_start(target).max(bb.begv);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_end_of_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: end-of-line N = forward-line(N-1) then end of line; for
    // N <= 0, if the backward move cannot complete, point stays at the
    // first line's BOL (no end-of-line).
    let n = opt_fixnum(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.zv.max(bb.begv).min(bb.text_len());
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = cur_line as i128 + n - 1;
    let p = if target < 0 {
        bb.text.line_start(0).max(bb.begv)
    } else {
        bb.text
            .line_end(bb.text.line_start(target as usize))
            .min(len)
    };
    let begv = bb.begv;
    bb.set_point(p.max(begv).min(len));
    Ok(Value::Nil)
}

fn f_bobp(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(bb.point() <= bb.begv))
}

fn f_eobp(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(bb.point() >= bb.text_len()))
}

fn f_bolp(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    Ok(Value::from_bool(
        p == 0 || bb.text.char_at(p.saturating_sub(1)) == '\n' || p <= bb.begv,
    ))
}

fn f_eolp(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    Ok(Value::from_bool(
        p >= bb.text_len() || bb.text.char_at(p) == '\n',
    ))
}

pub(crate) fn new_marker_at(i: &mut Interp, buf: usize, pos: usize) -> Value {
    let m = Rc::new(RefCell::new(Marker {
        buffer: Some(buf),
        position: pos,
        insertion_type: false,
    }));
    if let Some(b) = i.buffers.get(buf) {
        b.borrow_mut().register_marker(&m);
    }
    marker_value(m)
}

fn f_point_marker(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_point_min_marker(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.begv)
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_point_max_marker(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.text_len())
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_char_after(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU uses the RAW position (1-based int arg, 0-based marker pos,
    // current point): out-of-range answers nil, never a clamp.
    let p: i128 = match a.get(0) {
        Some(Value::Marker(m)) => m.borrow().position as i128,
        Some(Value::Nil) | None => bb.point() as i128,
        Some(v) => want_int(i, v)? - 1,
    };
    // nil outside the accessible portion (GNU checks BEGV <= p < ZV).
    if p < bb.begv as i128 || p >= bb.zv as i128 {
        return Ok(Value::Nil);
    }
    let p = p as usize;
    if p >= bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(
        bb.text.char_at(p),
    )))
}

fn f_char_before(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p: i128 = match a.get(0) {
        Some(Value::Marker(m)) => m.borrow().position as i128,
        Some(Value::Nil) | None => bb.point() as i128,
        Some(v) => want_int(i, v)? - 1,
    };
    if p <= bb.begv as i128 || p > bb.zv as i128 {
        return Ok(Value::Nil);
    }
    let p = (p - 1) as usize;
    if p >= bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(
        bb.text.char_at(p),
    )))
}

fn f_following_char(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    // GNU returns 0 (not nil) at the end of the accessible portion.
    if p >= bb.text_len() {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(
        bb.text.char_at(p),
    )))
}

fn f_preceding_char(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    if p <= bb.begv {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(
        bb.text.char_at(p - 1),
    )))
}

// GNU editfns.c Fpos_bol/Fpos_eol: N is a LINE COUNT (nil → current
// line), CHECK_INTEGER on non-fixnum args (markers/floats →
// integerp), overshoot clamps to BEGV/ZV.
fn line_count_arg(i: &mut Interp, a: crate::lisp::Argv, eol: bool) -> Result<i128, Flow> {
    match a.get(0) {
        None => Ok(if eol { 1 } else { 0 }),
        Some(v) if v.is_nil() => Ok(if eol { 1 } else { 0 }),
        Some(&Value::Int(n)) => Ok(if eol { n } else { n - 1 }),
        Some(other) => Err(i.wrong_type_mut("integerp", other)),
    }
}

fn f_pos_bol(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let count = line_count_arg(i, a, false)?;
    let bb = b.borrow();
    let line = bb.text.line_of_pos(bb.point()) as i128 + count;
    // scan_newline_from_point stops at BEGV/ZV; overshoot returns ZV/BEGV.
    if line < 0 {
        return Ok(Value::Int(1));
    }
    let last_line = bb.text.line_of_pos(bb.text.len()) as i128;
    if line > last_line {
        return Ok(Value::Int(bb.text.len() as i128 + 1));
    }
    Ok(Value::Int(bb.text.line_start(line as usize) as i128 + 1))
}

fn f_pos_eol(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let count = line_count_arg(i, a, true)?;
    // GNU eol: find_before_next_newline(PT, 0, count - (count <= 0)) —
    // eol of the line (count-1) lines forward (count<=0 goes backward).
    let bb = b.borrow();
    let line = bb.text.line_of_pos(bb.point()) as i128 + (count - 1);
    let last_line = bb.text.line_of_pos(bb.text.len()) as i128;
    if line < 0 {
        return Ok(Value::Int(1));
    }
    if line > last_line {
        return Ok(Value::Int(bb.text.len() as i128 + 1));
    }
    // End of line L = first newline at/after line_start(L) (len if none).
    Ok(Value::Int(
        bb.text.line_end(bb.text.line_start(line as usize)) as i128 + 1,
    ))
}

fn f_line_beginning_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fline_beginning_position: N is CHECK_NUMBER
    // (integerp — markers NOT coerced), unlike `beginning-of-line'.
    let n = match a.get(0) {
        Some(v) if v.truthy() => match v {
            Value::Int(n) => *n,
            _ => return Err(i.wrong_type_mut("integerp", v)),
        },
        _ => 1,
    };
    let saved = cur(i).borrow().point();
    let r = f_beginning_of_line(i, vec![Value::Int(n)].into());
    let p = cur(i).borrow().point();
    cur(i).borrow_mut().set_point(saved);
    r?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_line_end_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = match a.get(0) {
        Some(v) if v.truthy() => match v {
            Value::Int(n) => *n,
            _ => return Err(i.wrong_type_mut("integerp", v)),
        },
        _ => 1,
    };
    let saved = cur(i).borrow().point();
    let r = f_end_of_line(i, vec![Value::Int(n)].into());
    let p = cur(i).borrow().point();
    cur(i).borrow_mut().set_point(saved);
    r?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_line_number_at_pos(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let p = match a.get(0) {
        // GNU Fline_number_at_pos: markers take their position (detached
        // markers signal); nil → point; other non-fixnums signal fixnump.
        Some(Value::Marker(m)) => {
            let mm = m.borrow();
            if mm.buffer.is_none() {
                drop(bb);
                return Err(i.error("Marker does not point anywhere"));
            }
            mm.position
        }
        Some(v) if v.truthy() => {
            let n = crate::lisp::builtins::want_fixnum(i, v)?;
            // Whole-buffer bounds (BEG..Z), not BEGV..ZV — positions
            // outside signal args-out-of-range (POS BEG Z).
            if n < 1 || n > len as i128 + 1 {
                drop(bb);
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![v.clone(), Value::Int(1), Value::Int(len as i128 + 1)],
                ));
            }
            (n - 1) as usize
        }
        _ => bb.point(),
    };
    // GNU: ABSOLUTE nil clips pos_byte to BEGV..ZV; non-nil counts from
    // BEG_BYTE.
    let absolute = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let p2 = if absolute {
        p
    } else {
        p.max(bb.begv.min(len)).min(bb.begv + len)
    };
    let line = bb.text.line_of_pos(p2.min(len));
    let base = if absolute {
        0
    } else {
        bb.text.line_of_pos(bb.begv.min(len))
    };
    Ok(Value::Int((line - base) as i128 + 1))
}

fn f_count_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let (s_arg, e_arg) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    if s_arg < lo || s_arg > hi || e_arg < lo || e_arg > hi {
        drop(bb);
        let sym = i.intern("args-out-of-range");
        return Err(i.signal_data(sym, vec![a[0].clone(), a[1].clone()]));
    }
    let s = pos_idx(bb.text.len(), s_arg);
    let e = pos_idx(bb.text.len(), e_arg);
    let (s, e) = (s.min(e), s.max(e));
    let mut n = 0;
    for k in s..e {
        if bb.text.char_at(k) == '\n' {
            n += 1;
        }
    }
    // GNU: if the greater position is not at the start of a line,
    // the partial line counts too.
    if e > s && bb.text.char_at(e - 1) != '\n' {
        n += 1;
    }
    Ok(Value::Int(n))
}

fn f_current_column(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    let ls = bb.text.line_start(bb.text.line_of_pos(p));
    let mut col = 0i128;
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let mut k = ls;
    while k < p {
        match bb.text.char_at(k) {
            '\t' => col = (col / tab_width + 1) * tab_width,
            c => col += char_width(c),
        }
        k += 1;
    }
    Ok(Value::Int(col))
}

fn f_move_to_column(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU signals `wrong-type-argument' (wholenump COLUMN) — not
    // natnump — for negatives, floats, markers and other non-natnump
    // values alike.
    let goal = match &a[0] {
        Value::Int(n) if *n >= 0 => *n,
        other => {
            let wholenump = i.intern("wholenump");
            return Err(i.signal_data(
                sym::WRONG_TYPE_ARGUMENT,
                vec![Value::Sym(wholenump), other.clone()],
            ));
        }
    };
    let force = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    // Read buffer-local vars before borrow_mut: `symbol_value' can only
    // see buffer-locals while the buffer is not mutably borrowed.
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let tabs_on = i
        .symbol_value(i.intern_soft("indent-tabs-mode").unwrap_or(0))
        .truthy();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let ls = bb.text.line_start(bb.text.line_of_pos(bb.point()));
    let le = bb.text.line_end(bb.point());
    let mut col = 0i128;
    let mut k = ls;
    while k < le && col < goal {
        match bb.text.char_at(k) {
            '\t' => col = (col / tab_width + 1) * tab_width,
            c => col += char_width(c),
        }
        k += 1;
    }
    // GNU lands on the first char boundary where col >= goal (a char is
    // never split), then returns the column actually reached.
    if col < goal && force {
        // GNU calls `indent-to' (indent.c): pad with tabs to tab stops
        // then spaces when `indent-tabs-mode' is on, else spaces only.
        let mut s = String::new();
        let mut c = col;
        if tabs_on {
            loop {
                let next = (c / tab_width + 1) * tab_width;
                if next > goal {
                    break;
                }
                s.push('\t');
                c = next;
            }
        }
        while c < goal {
            s.push(' ');
            c += 1;
        }
        let at = bb.text.line_end(bb.point());
        let pad = s.chars().count();
        drop(bb);
        chg_insert(i, at, &s)?;
        cur(i).borrow_mut().set_point(at + pad);
        Ok(Value::Int(goal))
    } else {
        bb.set_point(k);
        Ok(Value::Int(col))
    }
}

// ---------- GNU scan engine (port of syntax.c) ----------

/// GNU `enum syntaxcode' class numbers.
mod sclass {
    pub const WHITESPACE: i128 = 0;
    pub const PUNCT: i128 = 1;
    pub const WORD: i128 = 2;
    pub const SYMBOL: i128 = 3;
    pub const OPEN: i128 = 4;
    pub const CLOSE: i128 = 5;
    pub const QUOTE: i128 = 6;
    pub const STRING: i128 = 7;
    pub const MATH: i128 = 8;
    pub const ESCAPE: i128 = 9;
    pub const CHARQUOTE: i128 = 10;
    pub const COMMENT: i128 = 11;
    pub const ENDCOMMENT: i128 = 12;
    pub const COMMENT_FENCE: i128 = 14;
    pub const STRING_FENCE: i128 = 15;
    /// GNU `Smax' — sentinel "no syntax" value (one past the last real
    /// class): all flag bits clear, class matches nothing.
    pub const SMAX: i128 = 16;
    /// Pseudo comment styles for fence-delimited constructs.
    pub const ST_COMMENT: i128 = 256 + 1;
    pub const ST_STRING: i128 = 256 + 2;
}
use sclass::*;

/// `SYNTAX_WITH_FLAGS' of TEXT[IDX].
#[inline]
fn wf(syn: &crate::editor::Syn, text: &[char], idx: usize) -> i128 {
    syn.with_flags_at(idx, text[idx])
}
#[inline]
fn fl_comstart_first(s: i128) -> bool {
    (s >> 16) & 1 == 1
}
#[inline]
fn fl_comstart_second(s: i128) -> bool {
    (s >> 17) & 1 == 1
}
#[inline]
fn fl_comend_first(s: i128) -> bool {
    (s >> 18) & 1 == 1
}
#[inline]
fn fl_comend_second(s: i128) -> bool {
    (s >> 19) & 1 == 1
}
#[inline]
fn fl_prefix(s: i128) -> bool {
    (s >> 20) & 1 == 1
}
#[inline]
fn fl_nested(s: i128) -> bool {
    (s >> 22) & 1 == 1
}
#[inline]
fn fl_style(syntax: i128, other: i128) -> i128 {
    crate::editor::Syn::comment_style(syntax, other)
}

/// GNU `char_quoted': is the char at index POS escaped by an odd run
/// of `\\' or `/'-class characters ending just before it?
fn char_quoted(syn: &crate::editor::Syn, text: &[char], pos: usize, beg: usize) -> bool {
    let mut q = pos;
    let mut quoted = false;
    while q > beg {
        let code = wf(syn, text, q - 1) & 0xff;
        if code == CHARQUOTE || code == ESCAPE {
            quoted = !quoted;
            q -= 1;
        } else {
            break;
        }
    }
    quoted
}

/// Port of GNU `forw_comment': scan forward over a comment whose body
/// starts at FROM (just past the starter).  STYLE is the comment style
/// (`fl_style' of the starter flags, or `ST_COMMENT' for a fence);
/// NESTING is the initial nesting level (>0 for nested comments, else
/// <=0).  Returns (index of the comment's last ender char, found,
/// last_syntax, residual_nesting): when FOUND is false the comment ran
/// to STOP and LAST_SYNTAX carries the syntax of the last char scanned
/// when it could begin a two-character construct (GNU `last_syntax_ptr').
#[allow(unused_assignments)]
fn forw_comment(
    syn: &crate::editor::Syn,
    text: &[char],
    mut from: usize,
    stop: usize,
    nesting: i64,
    style: i128,
    mut prev_syntax: i128,
) -> (usize, bool, i128, i64) {
    let end_escaped = syn.end_escaped;
    let mut nesting = if nesting <= 0 { -1 } else { nesting };
    let mut syntax = prev_syntax;
    let mut code = syntax & 0xff;
    // GNU enters mid-iteration to catch a two-char ender spanning the
    // start (PREV_SYNTAX holds the preceding char's syntax).
    let mut mid = syntax != 0;
    loop {
        if !mid {
            if from == stop {
                let last = if code == ESCAPE
                    || code == CHARQUOTE
                    || fl_comend_first(syntax)
                    || (nesting > 0 && fl_comstart_first(syntax))
                {
                    syntax
                } else {
                    SMAX
                };
                return (from, false, last, nesting);
            }
            prev_syntax = syntax;
            syntax = wf(syn, text, from);
            code = syntax & 0xff;
            if code == ENDCOMMENT
                && fl_style(syntax, 0) == style
                && (if fl_nested(syntax) {
                    nesting > 0 && {
                        nesting -= 1;
                        nesting == 0
                    }
                } else {
                    nesting < 0
                })
                && !(end_escaped
                    && (prev_syntax & 0xff == ESCAPE || prev_syntax & 0xff == CHARQUOTE))
            {
                break;
            }
            if code == COMMENT_FENCE && style == ST_COMMENT {
                break;
            }
            if nesting > 0 && code == COMMENT && fl_nested(syntax) && fl_style(syntax, 0) == style {
                nesting += 1;
            }
            if end_escaped && (code == ESCAPE || code == CHARQUOTE) {
                from += 1;
                if from == stop {
                    continue;
                }
                prev_syntax = syntax;
                syntax = SMAX;
                code = SMAX;
            }
            from += 1;
        }
        mid = false;
        // GNU's forw_incomment tail: detect two-char enders and nested
        // two-char starters.
        if from < stop && fl_comend_first(syntax) {
            let other = wf(syn, text, from);
            if fl_comend_second(other)
                && fl_style(syntax, other) == style
                && (if fl_nested(syntax) || fl_nested(other) {
                    nesting > 0
                } else {
                    nesting < 0
                })
            {
                syntax = SMAX;
                nesting -= 1;
                if nesting <= 0 {
                    break;
                }
                from += 1;
            }
        }
        if nesting > 0 && from < stop && fl_comstart_first(syntax) {
            let other = wf(syn, text, from);
            if fl_style(other, syntax) == style
                && fl_comstart_second(other)
                && (fl_nested(syntax) || fl_nested(other))
            {
                syntax = SMAX;
                from += 1;
                nesting += 1;
            }
        }
    }
    (from, true, SMAX, 0)
}

/// Port of GNU `back_comment': FROM is the index of the last char of a
/// comment ender (or first char of a two-char ender).  Scans back for
/// the matching starter, tracking string-quote parity.  Returns the
/// index of the comment's first starter char on success.
/// GNU `find_defun_start': a syntactically safe point at or before POS
/// from which a forward scan reproduces the correct comment/string
/// state.  With `comment-use-syntax-ppss' (GNU's default) it is the
/// start of the innermost enclosing comment/string per a fresh
/// `parse-partial-sexp' scan; else the last col-0 open paren when
/// `open-paren-in-column-0-is-defun-start'; else BEGV (STOP).
fn find_defun_start(syn: &crate::editor::Syn, text: &[char], stop: usize, pos: usize) -> usize {
    if syn.comment_use_ppss {
        let mut st = ParseState::fresh();
        scan_sexps_fwd(syn, text, stop, stop, pos, &mut st, i128::MIN, false, 0);
        if st.comstr_start >= 0 {
            return st.comstr_start as usize;
        }
        return pos;
    }
    if !syn.open_paren_defun {
        return stop;
    }
    // Scan backward for an open paren at column 0, requiring Sopen in
    // both the global and property-overridden tables (as GNU does).
    let mut p = pos.min(text.len());
    while p > stop && text[p - 1] != '\n' {
        p -= 1;
    }
    while p > stop {
        let c = text[p];
        if syn.with_flags(c) & 0xff == OPEN && syn.with_flags_at(p, c) & 0xff == OPEN {
            return p;
        }
        // Move to the start of the previous line.
        p -= 1;
        while p > stop && text[p - 1] != '\n' {
            p -= 1;
        }
    }
    stop
}

fn back_comment(
    syn: &crate::editor::Syn,
    text: &[char],
    mut from: usize,
    stop: usize,
    comnested: bool,
    comstyle: i128,
) -> (usize, bool) {
    let end_escaped = syn.end_escaped;
    let mut string_style: i128 = -1;
    let mut string_lossage = false;
    let mut comment_lossage = false;
    let comment_end = from;
    let mut comstart_pos: Option<usize> = None;
    // Position of the last col-0 open paren seen (only tracked when
    // `comment-use-syntax-ppss' is nil and the col-0 heuristic is on).
    let mut defun_start: Option<usize> = None;
    let mut lossage = false;
    let mut nesting: i64 = 1;
    let mut syntax: i128 = 0;
    while from != stop {
        from -= 1;
        let prev_syntax = syntax;
        let c = text[from];
        syntax = wf(syn, text, from);
        let mut code = syntax & 0xff;
        let com2start = fl_comstart_first(syntax)
            && fl_comstart_second(prev_syntax)
            && comstyle == fl_style(prev_syntax, syntax)
            && (fl_nested(prev_syntax) || fl_nested(syntax)) == comnested;
        let mut com2end = fl_comend_first(syntax) && fl_comend_second(prev_syntax);
        let comstart = com2start || code == COMMENT;
        // If a 2-char comment sequence partly overlaps with another,
        // fall back to a forward rescan (GNU's `lossage').
        if from > stop && (com2end || comstart) {
            let next_syntax = wf(syn, text, from - 1);
            if ((comstart || comnested) && fl_comend_second(syntax) && fl_comend_first(next_syntax))
                || ((com2end || comnested)
                    && fl_comstart_second(syntax)
                    && comstyle == fl_style(syntax, prev_syntax)
                    && fl_comstart_first(next_syntax))
            {
                lossage = true;
                break;
            }
        }
        if com2start && comstart_pos.is_none() {
            com2end = false;
        }
        if com2end {
            code = ENDCOMMENT;
        } else if com2start {
            code = COMMENT;
        } else if code == COMMENT
            && (comstyle != fl_style(syntax, 0) || fl_nested(syntax) != comnested)
        {
            continue;
        }
        // Quoted chars are skipped, but comment enders cannot be
        // quoted unless `comment-end-can-be-escaped'.
        if (end_escaped || code != ENDCOMMENT) && char_quoted(syn, text, from, stop) {
            continue;
        }
        match code {
            STRING_FENCE | COMMENT_FENCE => {
                let cc = if code == STRING_FENCE {
                    ST_STRING
                } else {
                    ST_COMMENT
                };
                if string_style == -1 {
                    string_style = cc;
                } else if string_style == cc {
                    string_style = -1;
                } else {
                    string_lossage = true;
                }
            }
            STRING => {
                if string_style == -1 {
                    string_style = c as i128;
                } else if string_style == c as i128 {
                    string_style = -1;
                } else {
                    string_lossage = true;
                }
            }
            COMMENT => {
                if string_style != -1 || comment_lossage || string_lossage {
                    // Odd string quotes involved — rescan forward.
                    lossage = true;
                    break;
                }
                if !comnested {
                    comstart_pos = Some(from);
                } else {
                    nesting -= 1;
                    if nesting <= 0 {
                        return (from, true);
                    }
                }
            }
            ENDCOMMENT => {
                if fl_style(syntax, 0) == comstyle
                    && (if com2end {
                        fl_nested(prev_syntax)
                    } else {
                        fl_nested(syntax)
                    }) == comnested
                {
                    if comnested {
                        nesting += 1;
                    } else {
                        // A same-style ender: anything earlier would
                        // match it rather than ours — stop looking.
                        break;
                    }
                } else if comstart_pos.is_some() || c != '\n' {
                    comment_lossage = true;
                }
            }
            OPEN => {
                // An open paren in column 0 is a defun start — a safe
                // place outside strings and comments (GNU's
                // defun_start heuristic; the loop simply stops).
                if syn.open_paren_defun
                    && !syn.comment_use_ppss
                    && (from == stop || text[from - 1] == '\n')
                {
                    defun_start = Some(from);
                    break;
                }
            }
            _ => {}
        }
    }
    if let Some(p) = comstart_pos {
        return (p, true);
    }
    if !lossage {
        return (comment_end, false);
    }
    // `lossage': mixed string delimiters or overlapping two-char
    // markers — decode by rescanning forward from a safe point with
    // the full state machine (GNU's back_comment lossage path).
    let mut ds = match defun_start {
        Some(d) => d,
        None => find_defun_start(syn, text, stop, comment_end),
    };
    loop {
        let mut st = ParseState::fresh();
        scan_sexps_fwd(
            syn,
            text,
            stop,
            ds,
            comment_end,
            &mut st,
            i128::MIN,
            false,
            0,
        );
        ds = comment_end;
        if st.incomment == if comnested { 1 } else { -1 } && st.comstyle == comstyle {
            from = st.comstr_start.max(0) as usize;
        } else {
            from = comment_end;
            if st.incomment != 0 {
                // comment_end sits inside some other comment — maybe
                // ours is nested; retry from within the outer one.
                ds = (st.comstr_start + 2) as usize;
            }
        }
        if ds >= comment_end {
            break;
        }
    }
    (from, from != comment_end)
}

/// What a failed `scan_lists' reports.
enum ScanErr {
    /// "Unbalanced parentheses" — ran out of text mid-object.
    Unbalanced(usize, usize),
    /// "Containing expression ends prematurely" — hit a mismatched
    /// delimiter before reaching the target level.
    Premature(usize, usize),
}

/// Port of GNU `scan_lists' over TEXT ([BEG,STOP) = narrowed bounds,
/// 0-based indices).  When SEXPFLAG, atoms and strings count as sexps
/// at level 0 (`scan-sexps'); otherwise only parens matter
/// (`scan-lists').  Returns Ok(Some(landing)) or Ok(None) when the
/// boundary is reached between objects (COUNT not used up).
fn scan_lists_gnu(
    syn: &crate::editor::Syn,
    text: &[char],
    beg: usize,
    stop: usize,
    from0: usize,
    count: i128,
    depth: i64,
    sexpflag: bool,
) -> Result<Option<usize>, ScanErr> {
    let comments = syn.comments_enabled();
    let min_depth = if depth > 0 { 0 } else { depth };
    let mut depth = depth;
    let mut last_good = from0;
    let mut from = from0.clamp(beg, stop);
    let mut count1 = count;
    let mut mathexit = false;
    while count1 > 0 {
        // `completed' marks GNU's `done' path: one object crossed.
        let mut completed = false;
        while from < stop {
            let syntax = wf(syn, text, from);
            let mut code = syntax & 0xff;
            let comstart_first = fl_comstart_first(syntax);
            let mut comnested = fl_nested(syntax);
            let mut comstyle = fl_style(syntax, 0);
            let prefix = fl_prefix(syntax);
            if depth == min_depth {
                last_good = from;
            }
            from += 1;
            if from < stop && comstart_first && comments {
                let other = wf(syn, text, from);
                if fl_comstart_second(other) {
                    code = COMMENT;
                    comstyle = fl_style(other, syntax);
                    comnested |= fl_nested(other);
                    from += 1;
                }
            }
            if prefix {
                continue;
            }
            match code {
                ESCAPE | CHARQUOTE | WORD | SYMBOL => {
                    if code == ESCAPE || code == CHARQUOTE {
                        if from == stop {
                            return Err(ScanErr::Unbalanced(last_good, from));
                        }
                        // The escaped char counts as a word constituent.
                        from += 1;
                    }
                    if depth != 0 || !sexpflag {
                        continue;
                    }
                    // This word counts as a sexp; finish the atom.
                    while from < stop {
                        let cd = wf(syn, text, from) & 0xff;
                        if cd == CHARQUOTE || cd == ESCAPE {
                            from += 1;
                            if from == stop {
                                return Err(ScanErr::Unbalanced(last_good, from));
                            }
                        } else if cd == WORD || cd == SYMBOL || cd == QUOTE {
                        } else {
                            break;
                        }
                        from += 1;
                    }
                    completed = true;
                    break;
                }
                COMMENT_FENCE | COMMENT => {
                    if code == COMMENT_FENCE {
                        comstyle = ST_COMMENT;
                    }
                    if !comments {
                        continue;
                    }
                    let (out, found, ..) = forw_comment(
                        syn,
                        text,
                        from,
                        stop,
                        if comnested { 1 } else { -1 },
                        comstyle,
                        0,
                    );
                    from = out;
                    if !found {
                        if depth == 0 {
                            completed = true;
                            break;
                        }
                        return Err(ScanErr::Unbalanced(last_good, from));
                    }
                    from += 1;
                }
                MATH => {
                    if !sexpflag {
                        continue;
                    }
                    if from != stop && text[from - 1] == text[from] {
                        from += 1;
                    }
                    if mathexit {
                        mathexit = false;
                        depth -= 1;
                        if depth == 0 {
                            completed = true;
                            break;
                        }
                        if depth < min_depth {
                            return Err(ScanErr::Premature(last_good, from));
                        }
                    } else {
                        mathexit = true;
                        depth += 1;
                        if depth == 0 {
                            completed = true;
                            break;
                        }
                    }
                }
                OPEN => {
                    depth += 1;
                    if depth == 0 {
                        completed = true;
                        break;
                    }
                }
                CLOSE => {
                    depth -= 1;
                    if depth == 0 {
                        completed = true;
                        break;
                    }
                    if depth < min_depth {
                        return Err(ScanErr::Premature(last_good, from));
                    }
                }
                STRING | STRING_FENCE => {
                    let stringterm = text[from - 1];
                    loop {
                        if from >= stop {
                            return Err(ScanErr::Unbalanced(last_good, from));
                        }
                        let cd = wf(syn, text, from) & 0xff;
                        let hit = if code == STRING {
                            text[from] == stringterm && cd == STRING
                        } else {
                            cd == STRING_FENCE
                        };
                        if hit {
                            break;
                        }
                        if cd == CHARQUOTE || cd == ESCAPE {
                            from += 1;
                        }
                        from += 1;
                    }
                    from += 1;
                    if depth == 0 && sexpflag {
                        completed = true;
                        break;
                    }
                }
                // Whitespace, punctuation, quote, comment-end: trivia.
                WHITESPACE | PUNCT | QUOTE | ENDCOMMENT => {}
                _ => {}
            }
        }
        // Reached the boundary mid-scan: error inside an object, nil
        // between objects.
        if !completed {
            if depth != 0 {
                return Err(ScanErr::Unbalanced(last_good, from));
            }
            return Ok(None);
        }
        count1 -= 1;
    }
    while count1 < 0 {
        let mut completed = false;
        while from > beg {
            from -= 1;
            let c = text[from];
            let mut syntax = wf(syn, text, from);
            let mut code = syntax & 0xff;
            if depth == min_depth {
                last_good = from;
            }
            let mut comstyle = 0;
            let mut comnested = fl_nested(syntax);
            if code == ENDCOMMENT {
                comstyle = fl_style(syntax, 0);
            }
            // Two-character comment ender: current char is the second,
            // preceded by a COMEND_FIRST char.
            if from > beg
                && fl_comend_second(syntax)
                && syn.comend_first(text[from - 1])
                && comments
            {
                from -= 1;
                code = ENDCOMMENT;
                let other = wf(syn, text, from);
                comstyle = fl_style(other, syntax);
                comnested |= fl_nested(other);
                syntax = other;
            }
            // Quoting turns anything except a comment-ender into a
            // word character.
            if code != ENDCOMMENT && char_quoted(syn, text, from, beg) {
                from -= 1;
                code = WORD;
            } else if fl_prefix(syntax) {
                continue;
            }
            match code {
                WORD | SYMBOL | ESCAPE | CHARQUOTE => {
                    if depth != 0 || !sexpflag {
                        continue;
                    }
                    // Backward over a word/atom: continue while the
                    // previous char is word/symbol/quote or quoted.
                    while from > beg {
                        if wf(syn, text, from - 1) & 0xff == ENDCOMMENT {
                            break;
                        }
                        if char_quoted(syn, text, from - 1, beg) {
                            // Quoted pair: consume both chars.
                            from -= 2;
                            continue;
                        }
                        let cd = wf(syn, text, from - 1) & 0xff;
                        if cd == WORD || cd == SYMBOL || cd == QUOTE {
                            from -= 1;
                        } else {
                            break;
                        }
                    }
                    completed = true;
                    break;
                }
                MATH => {
                    if !sexpflag {
                        continue;
                    }
                    if from > beg && c == text[from - 1] {
                        from -= 1;
                    }
                    if mathexit {
                        mathexit = false;
                        depth -= 1;
                        if depth == 0 {
                            completed = true;
                            break;
                        }
                        if depth < min_depth {
                            return Err(ScanErr::Premature(last_good, from));
                        }
                    } else {
                        mathexit = true;
                        depth += 1;
                        if depth == 0 {
                            completed = true;
                            break;
                        }
                    }
                }
                CLOSE => {
                    depth += 1;
                    if depth == 0 {
                        completed = true;
                        break;
                    }
                }
                OPEN => {
                    depth -= 1;
                    if depth == 0 {
                        completed = true;
                        break;
                    }
                    if depth < min_depth {
                        return Err(ScanErr::Premature(last_good, from));
                    }
                }
                ENDCOMMENT => {
                    if !comments {
                        continue;
                    }
                    let (out, found) = back_comment(syn, text, from, beg, comnested, comstyle);
                    if found {
                        from = out;
                    }
                }
                COMMENT_FENCE | STRING_FENCE => {
                    loop {
                        if from == beg {
                            return Err(ScanErr::Unbalanced(last_good, from));
                        }
                        from -= 1;
                        if !char_quoted(syn, text, from, beg) && wf(syn, text, from) & 0xff == code
                        {
                            break;
                        }
                    }
                    if code == STRING_FENCE && depth == 0 && sexpflag {
                        completed = true;
                        break;
                    }
                }
                STRING => {
                    let stringterm = c;
                    loop {
                        if from == beg {
                            return Err(ScanErr::Unbalanced(last_good, from));
                        }
                        from -= 1;
                        if !char_quoted(syn, text, from, beg)
                            && text[from] == stringterm
                            && wf(syn, text, from) & 0xff == STRING
                        {
                            break;
                        }
                    }
                    if depth == 0 && sexpflag {
                        completed = true;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !completed {
            if depth != 0 {
                return Err(ScanErr::Unbalanced(last_good, from));
            }
            return Ok(None);
        }
        count1 += 1;
    }
    Ok(Some(from))
}

/// Parse a skip-chars spec like " \t\n" or "^a-z" into a predicate set.
fn char_set_pred(spec: &str) -> (Vec<(char, char)>, Vec<char>, bool) {
    let mut ranges = Vec::new();
    let mut singles = Vec::new();
    let mut neg = false;
    let chars: Vec<char> = spec.chars().collect();
    let mut k = 0;
    if chars.get(0) == Some(&'^') {
        neg = true;
        k = 1;
    }
    while k < chars.len() {
        let c = chars[k];
        if k + 2 < chars.len() && chars[k + 1] == '-' {
            ranges.push((c, chars[k + 2]));
            k += 3;
        } else {
            singles.push(c);
            k += 1;
        }
    }
    (ranges, singles, neg)
}

fn char_set_contains(ranges: &[(char, char)], singles: &[char], neg: bool, c: char) -> bool {
    let in_ = singles.contains(&c) || ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi);
    in_ != neg
}

// ---------- parse-partial-sexp state machine (GNU scan_sexps_forward) ----------

/// Mirror of GNU's `lisp_parse_state'.  Positions are 0-based indices
/// internally; externalization adds 1.  `-1' marks "none" positions.
#[derive(Clone)]
pub(crate) struct ParseState {
    pub depth: i128,
    /// -1 outside strings; else the terminator char, or ST_STRING.
    pub instring: i128,
    /// 0 outside comments; -1 inside a non-nestable comment; else the
    /// nesting depth.
    pub incomment: i64,
    /// Comment style bits (0 = style a; ST_COMMENT for fence comments).
    pub comstyle: i128,
    pub quoted: bool,
    pub mindepth: i128,
    /// Start of the last complete sexp at the current level (-1 none).
    pub thislevelstart: i64,
    /// Start of the innermost containing list (-1 none).
    pub prevlevelstart: i64,
    /// Start of the innermost comment/string (-1 none).
    pub comstr_start: i64,
    /// Open-paren positions of enclosing lists, outermost first.
    pub levelstarts: Vec<i64>,
    /// Syntax-with-flags of the last scanned char that could begin a
    /// two-character construct; SMAX when it cannot.
    pub prev_syntax: i128,
    /// Where scanning stopped.
    pub location: usize,
}

impl ParseState {
    pub fn fresh() -> Self {
        ParseState {
            depth: 0,
            instring: -1,
            incomment: 0,
            comstyle: 0,
            quoted: false,
            mindepth: 0,
            thislevelstart: -1,
            prevlevelstart: -1,
            comstr_start: -1,
            levelstarts: Vec::new(),
            prev_syntax: SMAX,
            location: 0,
        }
    }

    /// GNU `internalize_parse_state': rebuild from the external list
    /// form (elements are 1-based charpos).
    pub fn internalize(old: &[Value]) -> Self {
        let mut st = ParseState::fresh();
        let get = |n: usize| old.get(n);
        st.depth = get(0).and_then(|v| v.int()).unwrap_or(0);
        st.instring = match get(3) {
            Some(Value::Int(n)) => *n,
            Some(v) if v.truthy() => ST_STRING,
            _ => -1,
        };
        st.incomment = match get(4) {
            Some(Value::Int(n)) => *n as i64,
            Some(v) if v.truthy() => -1,
            _ => 0,
        };
        st.quoted = get(5).is_some_and(|v| v.truthy());
        st.comstyle = match get(7) {
            Some(Value::Int(n)) if *n >= 0 && *n <= ST_COMMENT => *n,
            Some(v) if v.truthy() => ST_COMMENT,
            _ => 0,
        };
        st.comstr_start = get(8)
            .and_then(|v| v.int())
            .map(|n| n as i64 - 1)
            .unwrap_or(-1);
        if let Some(Value::Int(_)) | Some(_) = get(9) {
            if let Some(list) = get(9).and_then(|v| v.list_to_vec().ok()) {
                st.levelstarts = list
                    .iter()
                    .filter_map(|v| v.int())
                    .map(|n| n as i64 - 1)
                    .collect();
            }
        }
        st.prev_syntax = get(10).and_then(|v| v.int()).unwrap_or(SMAX);
        st
    }

    /// GNU's return list for `parse-partial-sexp' (1-based positions).
    pub fn externalize(&self, i: &mut Interp) -> Value {
        let pos = |p: i64| -> Value {
            if p < 0 {
                Value::Nil
            } else {
                Value::Int(p as i128 + 1)
            }
        };
        Value::list(vec![
            Value::Int(self.depth),
            pos(self.prevlevelstart),
            pos(self.thislevelstart),
            if self.instring >= 0 {
                if self.instring == ST_STRING {
                    Value::Sym(sym::T)
                } else {
                    Value::Int(self.instring)
                }
            } else {
                Value::Nil
            },
            if self.incomment < 0 {
                Value::Sym(sym::T)
            } else if self.incomment == 0 {
                Value::Nil
            } else {
                Value::Int(self.incomment as i128)
            },
            Value::from_bool(self.quoted),
            Value::Int(self.mindepth),
            if self.comstyle == 0 {
                Value::Nil
            } else if self.comstyle == ST_COMMENT {
                Value::Sym(i.intern("syntax-table"))
            } else {
                Value::Int(self.comstyle)
            },
            if self.incomment != 0 || self.instring >= 0 {
                pos(self.comstr_start)
            } else {
                Value::Nil
            },
            Value::list(self.levelstarts.iter().map(|p| pos(*p)).collect()),
            if self.prev_syntax == SMAX {
                Value::Nil
            } else {
                Value::Int(self.prev_syntax)
            },
        ])
    }
}

/// Whether the char at FROM-1 (syntax PREV_FROM_SYNTAX) plus the char
/// at FROM start a two-character comment; on match, fills STATE's
/// comment fields like GNU's `in_2char_comment_start'.
fn in_2char_comment_start(
    syn: &crate::editor::Syn,
    text: &[char],
    st: &mut ParseState,
    prev_from_syntax: i128,
    prev_from: usize,
    from: usize,
) -> bool {
    if fl_comstart_first(prev_from_syntax) {
        let syntax = wf(syn, text, from);
        if fl_comstart_second(syntax) {
            st.comstyle = fl_style(syntax, prev_from_syntax);
            st.incomment = if fl_nested(prev_from_syntax) || fl_nested(syntax) {
                1
            } else {
                -1
            };
            st.comstr_start = prev_from as i64;
            return true;
        }
    }
    false
}

/// Port of GNU `scan_sexps_forward': advance the parse state ST from
/// position FROM to END (0-based exclusive), under BEGV=BEG.
/// TARGETDEPTH stops early when DEPTH reaches it; STOPBEFORE stops at
/// the start of the next sexp; COMMENTSTOP is 0 (never), 1 (stop at
/// comment start), or -1 (also stop at comment/string boundaries).
#[allow(unused_assignments)]
pub(crate) fn scan_sexps_fwd(
    syn: &crate::editor::Syn,
    text: &[char],
    beg: usize,
    from0: usize,
    end: usize,
    st: &mut ParseState,
    targetdepth: i128,
    stopbefore: bool,
    commentstop: i32,
) {
    // GNU's fixed 100-deep `levelstart' stack.  GNU stores the open
    // paren of enclosing list K (outermost first) in level[K].last and
    // points curlevel at level[K+1] while inside it.
    const LEVELS: usize = 100;
    struct Level {
        last: i64,
        prev: i64,
    }
    let nlev = st.levelstarts.len().min(LEVELS - 1);
    let mut levels: Vec<Level> = Vec::with_capacity(nlev + 2);
    for p in st.levelstarts.iter().take(nlev) {
        levels.push(Level { last: *p, prev: -1 });
    }
    levels.push(Level { last: -1, prev: -1 });
    let mut cur: usize = nlev;

    let boundary_stop = commentstop == -1;
    let mut depth = st.depth;
    let mut mindepth = depth;
    let start_quoted = st.quoted;
    st.quoted = false;

    let mut from = from0;
    let mut prev_from = from;
    if from != beg {
        prev_from = from - 1;
    }
    let mut prev_from_syntax = st.prev_syntax;
    let mut prev_prev_from_syntax = SMAX;

    // INC_FROM: prev_from tracks the char just stepped over and
    // prev_from_syntax its flags.
    macro_rules! inc_from {
        () => {{
            prev_from = from;
            prev_prev_from_syntax = prev_from_syntax;
            prev_from_syntax = if prev_from < text.len() {
                wf(syn, text, prev_from)
            } else {
                SMAX
            };
            from += 1;
        }};
    }

    macro_rules! done {
        () => {{
            st.depth = depth;
            st.mindepth = mindepth;
            st.thislevelstart = levels[cur].prev;
            st.prevlevelstart = if cur == 0 { -1 } else { levels[cur - 1].last };
            st.location = from;
            st.levelstarts = levels[..cur].iter().map(|l| l.last).collect();
            st.prev_syntax = if (prev_from_syntax & 0x50000) != 0 || st.quoted {
                prev_from_syntax
            } else {
                SMAX
            };
            return;
        }};
    }

    macro_rules! endquoted {
        () => {{
            st.quoted = true;
            done!();
        }};
    }

    // GNU's `atcomment' label: ST holds the new comment's fields.
    macro_rules! atcomment {
        () => {{
            if commentstop != 0 || boundary_stop {
                done!();
            }
            let (out, found, last_syn, rem) = forw_comment(
                syn,
                text,
                from,
                end,
                st.incomment,
                st.comstyle,
                if from == beg { 0 } else { prev_from_syntax },
            );
            from = out;
            st.incomment = rem;
            prev_from_syntax = last_syn;
            if !found {
                if last_syn & 0xff == ESCAPE || last_syn & 0xff == CHARQUOTE {
                    endquoted!();
                }
                done!();
            }
            inc_from!();
            st.incomment = 0;
            st.comstyle = 0;
            prev_from_syntax = SMAX;
            if boundary_stop {
                done!();
            }
        }};
    }

    // GNU's `symstarted' inner loop: continue an atom after an escape
    // or word/symbol char; a 2-char comment start interrupts it and
    // skips `symdone'.
    macro_rules! symstarted {
        () => {{
            let mut interrupted = false;
            while from < end {
                if in_2char_comment_start(syn, text, st, prev_from_syntax, prev_from, from) {
                    inc_from!();
                    prev_from_syntax = SMAX;
                    atcomment!();
                    interrupted = true;
                    break;
                }
                match wf(syn, text, from) & 0xff {
                    CHARQUOTE | ESCAPE => {
                        inc_from!();
                        if from == end {
                            endquoted!();
                        }
                    }
                    WORD | SYMBOL | QUOTE => {}
                    _ => break,
                }
                inc_from!();
            }
            if !interrupted {
                levels[cur].prev = levels[cur].last;
            }
        }};
    }

    // Enter mid-construct per the resumed state.
    if st.incomment != 0 {
        // startincomment
        if from >= end {
            done!();
        }
        let (out, found, last_syn, rem) = forw_comment(
            syn,
            text,
            from,
            end,
            st.incomment,
            st.comstyle,
            if from == beg { 0 } else { prev_from_syntax },
        );
        from = out;
        st.incomment = rem;
        prev_from_syntax = last_syn;
        if !found {
            if last_syn & 0xff == ESCAPE || last_syn & 0xff == CHARQUOTE {
                endquoted!();
            }
            done!();
        }
        inc_from!();
        st.incomment = 0;
        st.comstyle = 0;
        prev_from_syntax = SMAX;
        if boundary_stop {
            done!();
        }
    } else if st.instring >= 0 {
        // startinstring (possibly resuming after a quote char).
        let nofence = st.instring != ST_STRING;
        if start_quoted {
            // startquotedinstring: `from' sits on the escaped char.
            if from >= end {
                endquoted!();
            }
            inc_from!();
        }
        'instring: loop {
            if from >= end {
                done!();
            }
            let c = text[from];
            let c_code = wf(syn, text, from) & 0xff;
            if nofence && c as i128 == st.instring && c_code == STRING {
                break;
            }
            match c_code {
                STRING_FENCE => {
                    if !nofence {
                        break 'instring;
                    }
                }
                CHARQUOTE | ESCAPE => {
                    inc_from!();
                    // startquotedinstring
                    if from >= end {
                        endquoted!();
                    }
                }
                _ => {}
            }
            inc_from!();
        }
        // string_end
        st.instring = -1;
        levels[cur].prev = levels[cur].last;
        inc_from!();
        if boundary_stop {
            done!();
        }
    } else if start_quoted {
        // startquoted
        if from == end {
            endquoted!();
        }
        inc_from!();
        symstarted!();
    } else if from < end && in_2char_comment_start(syn, text, st, prev_from_syntax, prev_from, from)
    {
        inc_from!();
        prev_from_syntax = SMAX;
        atcomment!();
    }

    while from < end {
        inc_from!();

        if from < end && in_2char_comment_start(syn, text, st, prev_from_syntax, prev_from, from) {
            inc_from!();
            prev_from_syntax = SMAX;
            atcomment!();
            continue;
        }
        if fl_prefix(prev_from_syntax) {
            continue;
        }
        match prev_from_syntax & 0xff {
            ESCAPE | CHARQUOTE => {
                if stopbefore {
                    from = prev_from;
                    prev_from_syntax = prev_prev_from_syntax;
                    done!();
                }
                levels[cur].last = prev_from as i64;
                // startquoted
                if from == end {
                    endquoted!();
                }
                inc_from!();
                symstarted!();
            }
            WORD | SYMBOL => {
                if stopbefore {
                    from = prev_from;
                    prev_from_syntax = prev_prev_from_syntax;
                    done!();
                }
                levels[cur].last = prev_from as i64;
                symstarted!();
            }
            COMMENT_FENCE | COMMENT => {
                if prev_from_syntax & 0xff == COMMENT_FENCE {
                    st.comstyle = ST_COMMENT;
                    st.incomment = -1;
                } else {
                    st.comstyle = fl_style(prev_from_syntax, 0);
                    st.incomment = if fl_nested(prev_from_syntax) { 1 } else { -1 };
                }
                st.comstr_start = prev_from as i64;
                atcomment!();
            }
            OPEN => {
                if stopbefore {
                    from = prev_from;
                    prev_from_syntax = prev_prev_from_syntax;
                    done!();
                }
                depth += 1;
                levels[cur].last = prev_from as i64;
                if cur + 1 < LEVELS {
                    cur += 1;
                    levels.push(Level { last: -1, prev: -1 });
                }
                // GNU clamps curlevel at endlevel without clearing.
                if targetdepth == depth {
                    done!();
                }
            }
            CLOSE => {
                depth -= 1;
                if depth < mindepth {
                    mindepth = depth;
                }
                if cur != 0 {
                    cur -= 1;
                }
                levels[cur].prev = levels[cur].last;
                if targetdepth == depth {
                    done!();
                }
            }
            STRING | STRING_FENCE => {
                st.comstr_start = (from - 1) as i64;
                if stopbefore {
                    from = prev_from;
                    prev_from_syntax = prev_prev_from_syntax;
                    done!();
                }
                levels[cur].last = prev_from as i64;
                st.instring = if prev_from_syntax & 0xff == STRING {
                    text[prev_from] as i128
                } else {
                    ST_STRING
                };
                if boundary_stop {
                    done!();
                }
                // startinstring
                let nofence = st.instring != ST_STRING;
                'instr: loop {
                    if from >= end {
                        done!();
                    }
                    let c = text[from];
                    let c_code = wf(syn, text, from) & 0xff;
                    if nofence && c as i128 == st.instring && c_code == STRING {
                        break;
                    }
                    match c_code {
                        STRING_FENCE => {
                            if !nofence {
                                break 'instr;
                            }
                        }
                        CHARQUOTE | ESCAPE => {
                            inc_from!();
                            if from >= end {
                                endquoted!();
                            }
                        }
                        _ => {}
                    }
                    inc_from!();
                }
                // string_end
                st.instring = -1;
                levels[cur].prev = levels[cur].last;
                inc_from!();
                if boundary_stop {
                    done!();
                }
            }
            // Smath, whitespace, punctuation, quote, endcomment: skip.
            _ => {}
        }
    }
    done!();
}

fn f_skip_chars_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let lim = match a.get(1) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.text_len(),
    };
    let (ranges, singles, neg) = char_set_pred(&spec);
    let mut p = bb.point();
    while p < lim.min(bb.text.len())
        && char_set_contains(&ranges, &singles, neg, bb.text.char_at(p))
    {
        p += 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

fn f_skip_chars_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let lim = match a.get(1) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.begv,
    };
    let (ranges, singles, neg) = char_set_pred(&spec);
    let mut p = bb.point();
    while p > lim && char_set_contains(&ranges, &singles, neg, bb.text.char_at(p - 1)) {
        p -= 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

fn f_skip_syntax_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let upto = a
        .get(1)
        .and_then(|v| v.int())
        .map(|n| n.max(1) as usize)
        .unwrap_or_else(|| cur(i).borrow().zv + 1);
    let syn = crate::editor::Syn::current(i, upto);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let lim = match a.get(1) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.text_len(),
    };
    let neg = spec.starts_with('^');
    // `-` is an alias for the whitespace class, like in regexps.
    let codes: Vec<u8> = spec
        .trim_start_matches('^')
        .bytes()
        .map(|c| if c == b'-' { b' ' } else { c })
        .collect();
    let mut p = bb.point();
    while p < lim.min(bb.text.len()) {
        let hit = codes.contains(&syn.code_at(p, bb.text.char_at(p)));
        if hit == neg {
            break;
        }
        p += 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

fn f_skip_syntax_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let upto = cur(i).borrow().point() + 1;
    let syn = crate::editor::Syn::current(i, upto);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let lim = match a.get(1) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.begv,
    };
    let neg = spec.starts_with('^');
    let codes: Vec<u8> = spec
        .trim_start_matches('^')
        .bytes()
        .map(|c| if c == b'-' { b' ' } else { c })
        .collect();
    let mut p = bb.point();
    while p > lim {
        let hit = codes.contains(&syn.code_at(p - 1, bb.text.char_at(p - 1)));
        if hit == neg {
            break;
        }
        p -= 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

/// Build the `scan-error' flow for a failed `scan_lists'.
/// GNU signals (scan-error MESSAGE LAST_GOOD FROM) with 1-based
/// character positions.
fn scan_err_flow(i: &mut Interp, e: ScanErr) -> Flow {
    let (msg, a, b) = match e {
        ScanErr::Unbalanced(l, f) => ("Unbalanced parentheses", l, f),
        ScanErr::Premature(l, f) => ("Containing expression ends prematurely", l, f),
    };
    let sym = i.intern("scan-error");
    i.signal_data(
        sym,
        vec![
            Value::string(msg),
            Value::Int(a as i128 + 1),
            Value::Int(b as i128 + 1),
        ],
    )
}

/// Port of GNU `Fforward_comment': move across COUNT comments, stopping
/// at the first non-comment non-whitespace char.  Returns
/// (new point, t-when-all-count-crossed).  Comment delimiters apply
/// unconditionally (unlike `scan_lists', which gates on
/// `parse-sexp-ignore-comments').
fn forward_comment_scan(
    syn: &crate::editor::Syn,
    text: &[char],
    beg: usize,
    stop: usize,
    mut from: usize,
    count: i128,
) -> (usize, bool) {
    let mut count1 = count;
    while count1 > 0 {
        // Skip whitespace (and newline comment-enders) to a starter.
        let mut code;
        let mut comnested;
        let mut comstyle;
        loop {
            if from == stop {
                return (from, false);
            }
            let c = text[from];
            let syntax = wf(syn, text, from);
            code = syntax & 0xff;
            let comstart_first = fl_comstart_first(syntax);
            comnested = fl_nested(syntax);
            comstyle = fl_style(syntax, 0);
            from += 1;
            if from < stop && comstart_first {
                let other = wf(syn, text, from);
                if fl_comstart_second(other) {
                    code = COMMENT;
                    comstyle = fl_style(other, syntax);
                    comnested |= fl_nested(other);
                    from += 1;
                }
            }
            if !(code == WHITESPACE || (code == ENDCOMMENT && c == '\n')) {
                break;
            }
        }
        if code == COMMENT_FENCE {
            comstyle = ST_COMMENT;
        } else if code != COMMENT {
            from -= 1;
            return (from, false);
        }
        let (out, found, ..) = forw_comment(
            syn,
            text,
            from,
            stop,
            if comnested { 1 } else { -1 },
            comstyle,
            0,
        );
        from = out;
        if !found {
            return (from, false);
        }
        from += 1;
        count1 -= 1;
    }
    while count1 < 0 {
        loop {
            if from <= beg {
                return (beg, false);
            }
            from -= 1;
            let quoted = char_quoted(syn, text, from, beg);
            let c = text[from];
            let mut syntax = wf(syn, text, from);
            let mut code = syntax & 0xff;
            let mut comstyle = 0;
            let mut comnested = fl_nested(syntax);
            if code == ENDCOMMENT {
                comstyle = fl_style(syntax, 0);
            }
            let mut two_char = false;
            if from > beg
                && fl_comend_second(syntax)
                && syn.comend_first(text[from - 1])
                && !char_quoted(syn, text, from - 1, beg)
            {
                from -= 1;
                two_char = true;
                code = ENDCOMMENT;
                let other = wf(syn, text, from);
                comstyle = fl_style(other, syntax);
                comnested |= fl_nested(other);
                syntax = other;
            }
            if code == COMMENT_FENCE {
                // Skip back to the first preceding unquoted fence.
                let ini = from;
                let mut found = false;
                while from > beg {
                    from -= 1;
                    if wf(syn, text, from) & 0xff == COMMENT_FENCE
                        && !char_quoted(syn, text, from, beg)
                    {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return (ini + 1, false);
                }
                break;
            } else if code == ENDCOMMENT {
                let (out, found) = if !quoted || !syn.end_escaped {
                    back_comment(syn, text, from, beg, comnested, comstyle)
                } else {
                    (from, false)
                };
                if !found {
                    if c == '\n' {
                        // An end-of-line that isn't an end-of-comment
                        // is treated like whitespace.
                        continue;
                    }
                    if two_char {
                        from += 1;
                    }
                    return (from + 1, false);
                }
                from = out;
                break;
            } else if code != WHITESPACE || quoted {
                return (from + 1, false);
            }
        }
        count1 += 1;
    }
    (from, true)
}

/// GNU `Fbackward_prefix_chars': skip back over unquoted chars whose
/// syntax is expression-prefix (`'') or carries the `p' flag.
pub(crate) fn backward_prefix_chars(
    syn: &crate::editor::Syn,
    text: &[char],
    beg: usize,
    mut pos: usize,
) -> usize {
    while pos > beg
        && !char_quoted(syn, text, pos - 1, beg)
        && (wf(syn, text, pos - 1) & 0xff == QUOTE || syn.is_prefix_flag(text[pos - 1]))
    {
        pos -= 1;
    }
    pos
}

fn f_forward_comment(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let count = want_fixnum_nil_err(i, &arg(&a, 0))?;
    // Resolve the syntax table before the buffer borrow (the borrow
    // would make the lookup silently fall back to the standard table).
    let stop1 = cur(i).borrow().zv + 1;
    let syn = crate::editor::Syn::current(i, stop1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let beg = bb.begv;
    let stop = bb.zv.min(bb.text_len());
    let pt = bb.point();
    let (np, ok) = {
        let text = bb.text.as_slice();
        forward_comment_scan(&syn, &text, beg, stop, pt, count)
    };
    bb.set_point(np);
    Ok(if ok { Value::Sym(sym::T) } else { Value::Nil })
}

/// GNU `scan-lists': scan COUNT lists from FROM starting at DEPTH.
/// Returns the landing charpos, nil at the boundary between objects.
fn f_scan_lists(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let from = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    let count = crate::lisp::builtins::want_fixnum(i, &a[1])?;
    let depth = crate::lisp::builtins::want_fixnum(i, &a[2])?;
    let upto = cur(i).borrow().zv + 1;
    let syn = crate::editor::Syn::current(i, upto);
    let b = i.buffers.get(i.current_buffer).unwrap().clone();
    let bb = b.borrow_mut();
    let (beg, stop) = (bb.begv, bb.zv.min(bb.text_len()));
    let pos = pos_idx(stop, from).max(beg);
    let scan = {
        let text = bb.text.as_slice();
        scan_lists_gnu(&syn, &text, beg, stop, pos, count, depth as i64, false)
    };
    match scan {
        Ok(Some(p)) => Ok(Value::Int(p as i128 + 1)),
        Ok(None) => Ok(Value::Nil),
        Err(e) => Err(scan_err_flow(i, e)),
    }
}

fn f_scan_sexps(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_FIXNUM on both args (`fixnump', not integer-or-marker).
    let from = match &a[0] {
        Value::Int(n) => *n,
        v => return Err(i.wrong_type_mut("fixnump", v)),
    };
    let count = match &a[1] {
        Value::Int(n) => *n,
        v => return Err(i.wrong_type_mut("fixnump", v)),
    };
    let upto = cur(i).borrow().zv + 1;
    let syn = crate::editor::Syn::current(i, upto);
    let b = i.buffers.get(i.current_buffer).unwrap().clone();
    let bb = b.borrow_mut();
    let (beg, stop) = (bb.begv, bb.zv.min(bb.text_len()));
    let pos = pos_idx(stop, from).max(beg);
    let scan = {
        let text = bb.text.as_slice();
        scan_lists_gnu(&syn, &text, beg, stop, pos, count, 0, true)
    };
    match scan {
        Ok(Some(p)) => Ok(Value::Int(p as i128 + 1)),
        Ok(None) => Ok(Value::Nil),
        Err(e) => Err(scan_err_flow(i, e)),
    }
}

/// GNU `syntax-after': (CLASS . MATCHING-CHAR) for the char at POS.
pub(crate) fn f_syntax_after(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = want_int(i, &a[0])?;
    let b = cur(i);
    // GNU: POS outside [BEGV, ZV] returns nil (pos 0 is before
    // position 1 — don't clamp).  Bounds + text index first, borrow
    // released before Syn::current which may eval
    // `internal--syntax-propertize' (Lisp → buffer mutation).
    {
        let bb = b.borrow();
        if pos < bb.begv as i128 + 1 || pos > bb.zv as i128 + 1 {
            return Ok(Value::Nil);
        }
        if pos_idx(bb.text_len(), pos) >= bb.text_len() {
            return Ok(Value::Nil);
        }
    }
    let syn = crate::editor::Syn::current(i, pos.max(1) as usize);
    let bb = b.borrow();
    let idx = pos_idx(bb.text_len(), pos);
    let c = bb.text.char_at(idx);
    let cls = syn.class_at(idx, c);
    let matching = syn.matching_at(idx, c);
    // GNU returns (CLASS . MATCHING-CHAR); a nil cdr prints as a
    // one-element list.
    Ok(Value::cons(
        Value::Int(cls),
        matching.map(Value::Int).unwrap_or(Value::Nil),
    ))
}

fn f_looking_back(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    let limit = a.get(1).and_then(|v| v.int());
    let greedy = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let (text, pos, base) = {
        let b = cur(i);
        let bb = b.borrow();
        (
            bb.text
                .substring(bb.begv, bb.text_len())
                .chars()
                .collect::<Vec<char>>(),
            bb.point() - bb.begv,
            bb.begv,
        )
    };
    let lo = limit
        .map(|l| (l.max(1) as usize - 1).saturating_sub(base))
        .unwrap_or(0);
    // GNU: candidates must end exactly at point; non-greedy picks the
    // smallest (nearest start), greedy the largest (earliest start).
    // `match_at_end' forces the match end so alternation order doesn’t
    // prematurely succeed on a shorter match.
    let order: Vec<usize> = if greedy {
        (lo..=pos).collect()
    } else {
        (lo..=pos).rev().collect()
    };
    let syn = crate::editor::re_syntax(i);
    for start in order {
        if let Some(regs) = crate::lisp::regexp::match_at_end(
            &re,
            &text,
            start,
            pos,
            &syn,
            &crate::editor::re_category(i),
            pos,
        ) {
            i.match_data = Some(MatchData {
                regs,
                in_buffer: true,
                base,
                buffer: Some(i.current_buffer),
            });
            return Ok(Value::t());
        }
    }
    Ok(Value::Nil)
}

fn f_last_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fother_buffer(skip=BUFFER, visible_ok=VISIBLE-OK, last=t):
    // last live buffer whose name does not start with a space, unless
    // VISIBLE-OK is non-nil.  A non-nil non-deleted BUFFER is excluded.
    let exclude = match a.first() {
        None | Some(Value::Nil) => None,
        Some(v) => i.buffer_id_of(v),
    };
    let visible_ok = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    // GNU Fother_buffer: the third arg is a FRAME — decode_valid_frame
    // signals `(framep ARG)' on non-frames.
    if let Some(v) = a.get(2) {
        match v {
            Value::Nil | Value::Frame(_) => {}
            other => return Err(i.wrong_type_mut("framep", other)),
        }
    }
    let id = i.buffers.order.iter().rev().copied().find(|&id| {
        if Some(id) == exclude {
            return false;
        }
        i.buffers
            .get(id)
            .map(|b| {
                let bb = b.borrow();
                bb.live && (visible_ok || !bb.name.starts_with(' '))
            })
            .unwrap_or(false)
    });
    match id {
        Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}

// ---------- insertion & deletion ----------

fn insert_str_at_point(i: &mut Interp, s: &str, before_markers: bool) -> Result<(), Flow> {
    check_writable(i)?;
    barf_if_file_locked(i)?;
    chg_insert_pt(i, s, before_markers)?;
    Ok(())
}

/// Copy a string arg's own text properties into the buffer for the
/// just-inserted range [base, base+len).  GNU `insert' copies the
/// string's properties verbatim (no inheritance from surrounding
/// text — that is `insert-and-inherit's job).
pub(crate) fn copy_str_props(i: &mut Interp, s: &crate::lisp::value::StrRef, base: usize) {
    if !i.has_str_props(s) {
        return;
    }
    let ivs: Vec<(usize, usize, Vec<Value>)> = i.str_props(s).to_vec();
    if ivs.is_empty() {
        return;
    }
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for (st, en, plist) in ivs {
        // `insert' preserves the source plist order; since the
        // buffer's emitted plist is newest-first (records scanned in
        // reverse), push the pairs back-to-front.
        let mut k = plist.len();
        while k >= 2 {
            if let Some(p) = i.sym_id(&plist[k - 2]) {
                bb.text.props_write().push(TextProp {
                    start: base + st,
                    end: base + en,
                    prop: p,
                    value: plist[k - 1].clone(),
                });
            }
            k -= 2;
        }
    }
    bb.note_prop_modified();
}

/// Text to insert as it lands in a unibyte buffer: raw bytes stay
/// chars ≤0xFF, eight-bit proxies decode to their byte, and anything
/// wider truncates to a byte (GNU `(insert 300)' → char 44).
fn unibyte_insert_text(s: &str) -> String {
    s.chars()
        .map(|c| {
            if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                char::from_u32(b as u32).unwrap()
            } else {
                char::from_u32((c as u32) & 0xFF).unwrap_or('\u{FFFD}')
            }
        })
        .collect()
}

/// Does `front-sticky' at POS cover PROP?  `t' covers every property;
/// a list covers its members; anything else means non-sticky.
fn front_sticky_list_covers(i: &Interp, v: Value, prop: u32) -> bool {
    match v {
        Value::Sym(s) if s == sym::T => true,
        Value::Cons(c) => {
            let mut c = Value::Cons(c);
            while let Value::Cons(cc) = &c {
                let (h, tl) = {
                    let b = cc.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if i.sym_id(&h) == Some(prop) {
                    return true;
                }
                c = tl;
            }
            false
        }
        _ => false,
    }
}

fn front_sticky_covers(i: &mut Interp, pos: usize, prop: u32) -> bool {
    let fs = i.intern("front-sticky");
    let v = prop_at(i, pos, fs);
    front_sticky_list_covers(i, v, prop)
}

fn front_sticky_covers_bb(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> bool {
    let fs = i.intern("front-sticky");
    let v = prop_at_bb(i, bb, pos, fs);
    front_sticky_list_covers(i, v, prop)
}

/// The plist `insert-and-inherit' would graft onto text inserted at
/// POS (0-based char index): the rear-sticky properties of the
/// character *before* POS plus the front-sticky properties of the
/// character *at* POS (front wins on ties, as in GNU's
/// `text_property_stickiness'/interval grafting).
fn inherit_plist_at(i: &mut Interp, pos: usize) -> Vec<(u32, Value)> {
    let mut m: std::collections::BTreeMap<u32, Value> = if pos > 0 {
        cur(i).borrow().plist_at(pos - 1)
    } else {
        Default::default()
    };
    m.retain(|p, _| !rear_nonsticky_covers(i, pos.saturating_sub(1), *p));
    if pos < cur(i).borrow().text.len() {
        for (p, v) in cur(i).borrow().plist_at(pos) {
            if front_sticky_covers(i, pos, p) {
                m.insert(p, v);
            }
        }
    }
    m.into_iter().collect()
}

fn insert_impl(i: &mut Interp, a: &[Value], before_markers: bool, inherit: bool) -> EvalResult {
    check_writable(i)?;
    let unibyte = buffer_is_unibyte(i);
    for v in a {
        match v {
            Value::Str(s) => {
                let mut t = s.borrow().clone();
                if unibyte {
                    t = unibyte_insert_text(&t);
                }
                let n = t.chars().count();
                let base = cur(i).borrow().point();
                let inherited = if inherit {
                    inherit_plist_at(i, base)
                } else {
                    Vec::new()
                };
                let string_ivs = i.str_props(s).to_vec();
                insert_str_at_point(i, &t, before_markers)?;
                // Inheritance fills only where the inserted string
                // carries no property of its own (GNU grafts onto the
                // interval surrounding the insertion point but the
                // string's own properties stay).
                if !inherited.is_empty() {
                    let mut spans: Vec<(usize, usize, u32, Value)> = Vec::new();
                    for (p, val) in &inherited {
                        // Coalesce maximal runs of positions where the
                        // string lacks prop P (present-but-nil counts
                        // as present, like GNU's textget).
                        let mut q = 0usize;
                        while q < n {
                            let has = str_plist_at(&string_ivs, q)
                                .chunks(2)
                                .any(|c| c.len() == 2 && i.sym_id(&c[0]) == Some(*p));
                            if has {
                                q += 1;
                                continue;
                            }
                            let mut e = q + 1;
                            while e < n
                                && !str_plist_at(&string_ivs, e)
                                    .chunks(2)
                                    .any(|c| c.len() == 2 && i.sym_id(&c[0]) == Some(*p))
                            {
                                e += 1;
                            }
                            spans.push((q, e, *p, val.clone()));
                            q = e;
                        }
                    }
                    if !spans.is_empty() {
                        let b = cur(i);
                        let mut bb = b.borrow_mut();
                        for (st, en, p, val) in spans {
                            bb.text.props_write().push(TextProp {
                                start: base + st,
                                end: base + en,
                                prop: p,
                                value: val,
                            });
                        }
                        bb.note_prop_modified();
                    }
                }
                copy_str_props(i, s, base);
                // After-change hooks may have moved point; later args
                // insert wherever it ended up, like GNU.
            }
            Value::Int(n) => {
                let code = if unibyte { *n & 0xFF } else { *n };
                if let Some(c) = crate::lisp::value::lisp_char(code as u32) {
                    let base = cur(i).borrow().point();
                    let inherited = if inherit {
                        inherit_plist_at(i, base)
                    } else {
                        Vec::new()
                    };
                    insert_str_at_point(i, &c.to_string(), before_markers)?;
                    if !inherited.is_empty() {
                        let b = cur(i);
                        let mut bb = b.borrow_mut();
                        for (p, val) in inherited {
                            bb.text.props_write().push(TextProp {
                                start: base,
                                end: base + 1,
                                prop: p,
                                value: val,
                            });
                        }
                        bb.note_prop_modified();
                    }
                } else {
                    return Err(i.wrong_type_mut("char-or-string-p", v));
                }
            }
            other => return Err(i.wrong_type_mut("char-or-string-p", other)),
        }
    }
    Ok(Value::Nil)
}

fn f_insert(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    insert_impl(i, &a, false, false)
}

fn f_insert_before_markers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    insert_impl(i, &a, true, false)
}

fn f_insert_and_inherit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    insert_impl(i, &a, false, true)
}

fn f_insert_before_markers_and_inherit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    insert_impl(i, &a, true, true)
}

fn f_insert_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let unibyte = buffer_is_unibyte(i);
    let c = match &a[0] {
        Value::Int(n) => {
            let code = if unibyte { *n & 0xFF } else { *n };
            crate::lisp::value::lisp_char(code as u32)
                .ok_or_else(|| i.wrong_type_mut("characterp", &a[0]))?
        }
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let count = a.get(1).and_then(|v| v.int()).unwrap_or(1).max(0);
    // GNU make_gap_larger: growth past BUF_BYTES_MAX (= MOST_POSITIVE_
    // FIXNUM - 1 on 64-bit) reports "Maximum buffer size exceeded".
    // Anything beyond our practical cap reports the same message — a
    // real multi-GB insert would exhaust memory anyway.
    if count.saturating_mul(c.len_utf8() as i128) > 0x7FFF_FFFF {
        return Err(i.error("Maximum buffer size exceeded"));
    }
    check_writable(i)?;
    let s: String = std::iter::repeat(c).take(count as usize).collect();
    insert_str_at_point(i, &s, false)?;
    Ok(Value::Nil)
}

fn f_insert_buffer_substring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Finsert_buffer_substring decodes BUFFER via
    // Fget_buffer: a buffer passes through, a string is looked up,
    // anything else (nil included) signals `stringp'.
    let src = match &a[0] {
        Value::Buffer(b) => b.clone(),
        Value::Str(_) => buf_or_name(i, &a[0])?,
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let (text, len) = {
        let bb = src.borrow();
        (bb.text.text(), bb.text.len())
    };
    // GNU decode_position_or_default semantics on START/END: nil/absent
    // -> point-min/point-max of the source buffer; present non-nil
    // non-number-or-marker signals integer-or-marker-p.
    let mut decpos = |v: Option<&Value>| -> Result<Option<usize>, Flow> {
        match v {
            Some(v) if !v.is_nil() => match v {
                Value::Int(n) => Ok(Some(pos_idx(len, *n))),
                Value::Marker(m) if m.borrow().buffer.is_some() => {
                    Ok(Some(pos_idx(len, m.borrow().position as i128 + 1)))
                }
                _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
            },
            _ => Ok(None),
        }
    };
    let s = decpos(a.get(1))?.unwrap_or(0);
    let e = decpos(a.get(2))?.unwrap_or(len);
    let chars: Vec<char> = text.chars().collect();
    let (lo, hi) = (s.min(e), e.max(s));
    let sub: String = chars[lo..hi].iter().collect();
    // GNU copies the source range's text properties along with the
    // characters.
    let props: Vec<TextProp> = src
        .borrow()
        .text
        .props
        .borrow()
        .iter()
        .filter(|tp| tp.start < hi && tp.end > lo)
        .map(|tp| TextProp {
            start: tp.start.max(lo) - lo,
            end: tp.end.min(hi) - lo,
            prop: tp.prop,
            value: tp.value.clone(),
        })
        .collect();
    check_writable(i)?;
    let base = cur(i).borrow().point();
    insert_str_at_point(i, &sub, false)?;
    if !props.is_empty() {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        for mut tp in props {
            tp.start += base;
            tp.end += base;
            bb.text.props_write().push(tp);
        }
        bb.note_prop_modified();
    }
    Ok(Value::Nil)
}

fn f_self_insert_command(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = want_fixnum_nil_err(i, &arg(&a, 0))?.max(0);
    // GNU `internal_self_insert' takes the char from arg 2 (explicit
    // C) or `last-command-event' when C is nil/absent.
    let c = match a.get(1) {
        Some(Value::Int(x)) => crate::lisp::value::lisp_char(*x as u32),
        Some(Value::Nil) | None => {
            match i.symbol_value(i.intern_soft("last-command-event").unwrap_or(0)) {
                Value::Int(x) => crate::lisp::value::lisp_char(x as u32),
                _ => None,
            }
        }
        Some(v) => return Err(i.wrong_type_mut("characterp", v)),
    };
    if n <= 0 {
        // GNU internal_self_insert returns before the hook on N<=0.
        return Ok(Value::Nil);
    }
    if let Some(c) = c {
        // GNU `internal_self_insert': before inserting a character
        // with non-word syntax, expand the abbrev before point when
        // `abbrev-mode' is on; an abbrev hook with a non-nil
        // `no-self-insert' property suppresses the insertion.
        if n > 0 && self_insert_expand_abbrev(i, c)? {
            return Ok(Value::Nil);
        }
        let s: String = std::iter::repeat(c).take(n as usize).collect();
        insert_str_at_point(i, &s, false)?;
        // GNU `internal_self_insert': when `auto-fill-function' is
        // non-nil and the `auto-fill-chars' table maps the inserted
        // char (non-table value falls back to SPC and \n), call
        // `internal-auto-fill' — for '\n', on the previous line.
        let aff = i
            .intern_soft("auto-fill-function")
            .map(|id| i.symbol_value(id))
            .unwrap_or(Value::Nil);
        if aff.truthy() && n > 0 {
            let chars = i
                .intern_soft("auto-fill-chars")
                .map(|id| i.symbol_value(id))
                .unwrap_or(Value::Nil);
            let trigger = if crate::lisp::builtins::misc::is_char_table(i, &chars) {
                crate::lisp::builtins::misc::char_table_ref(i, &chars, c as usize).truthy()
            } else {
                c == ' ' || c == '\n'
            };
            if trigger {
                let zv = cur(i).borrow().zv;
                if c == '\n' {
                    let b = cur(i);
                    let p = b.borrow().point();
                    if p > 0 {
                        b.borrow_mut().set_point(p - 1);
                    }
                }
                let iaf = Value::Sym(i.intern("internal-auto-fill"));
                let _ = i.apply(&iaf, vec![]);
                if c == '\n' {
                    let b = cur(i);
                    let p = b.borrow().point();
                    if p < zv {
                        b.borrow_mut().set_point(p + 1);
                    }
                }
            }
        }
        // GNU runs `post-self-insert-hook' after every insertion.
        call_hook(i, "post-self-insert-hook")?;
    }
    Ok(Value::Nil)
}

/// GNU's abbrev check in `internal_self_insert' (src/cmds.c): when
/// `abbrev-mode' is on, the buffer is writable, point is after BEGV,
/// the char being inserted has non-word syntax and the previous char
/// has word syntax, call `expand-abbrev'.  Returns true when the
/// inserted char should be suppressed.
fn self_insert_expand_abbrev(i: &mut Interp, c: char) -> Result<bool, Flow> {
    let amode = match i.intern_soft("abbrev-mode") {
        Some(id) => i.symbol_value(id),
        None => return Ok(false),
    };
    if amode.is_nil() || crate::editor::syntax_code_buf(i, c) == b'w' {
        return Ok(false);
    }
    {
        let b = cur(i);
        let bb = b.borrow();
        let ro = i
            .intern_soft("buffer-read-only")
            .map(|id| i.symbol_value(id))
            .unwrap_or(Value::Nil);
        if ro.truthy() || bb.point() <= bb.begv {
            return Ok(false);
        }
        match bb.text.char_at(bb.point() - 1) {
            prev if crate::editor::syntax_code_buf(i, prev) == b'w' => {}
            _ => return Ok(false),
        }
    }
    let expand = match i.intern_soft("expand-abbrev") {
        Some(id) => i.sym(id),
        None => return Ok(false),
    };
    let expanded = i.apply(&expand, vec![])?;
    if let Value::Sym(abbr) = expanded {
        let hook = i.symbol_function(abbr);
        if let Value::Sym(hsym) = hook {
            let pid = i.intern("no-self-insert");
            if i.get_prop(hsym, pid).truthy() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn f_newline(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    // GNU make_gap_larger: growth past BUF_BYTES_MAX reports
    // "Maximum buffer size exceeded".
    if n > 0x7FFF_FFFF {
        return Err(i.error("Maximum buffer size exceeded"));
    }
    insert_str_at_point(i, &"\n".repeat(n as usize), false)?;
    Ok(Value::Nil)
}
fn f_open_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = want_num_mark_nil_err(i, &arg(&a, 0))?.max(0);
    if n == 0 {
        // GNU Fopen_line: nonpositive arg is a no-op (no newline,
        // no modification hooks).
        return Ok(Value::Nil);
    }
    check_writable(i)?;
    if n > 0x7FFF_FFFF {
        return Err(i.error("Maximum buffer size exceeded"));
    }
    let p = cur(i).borrow().point();
    chg_insert(i, p, &"\n".repeat(n as usize))?;
    cur(i).borrow_mut().set_point(p);
    Ok(Value::Nil)
}

fn f_delete_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = want_fixnum_nil_err(i, &arg(&a, 0))?;
    check_writable(i)?;
    let (p, zv, begv) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.point(), bb.text_len(), bb.begv)
    };
    if n > 0 {
        if p >= zv {
            return Err(i.signal_data(sym::END_OF_BUFFER, vec![]));
        }
        let end = (p + n as usize).min(zv);
        chg_delete(i, p, end)?;
    } else if n < 0 {
        if p <= begv {
            return Err(i.signal_data(sym::BEGINNING_OF_BUFFER, vec![]));
        }
        let start = (p as i128 + n).max(begv as i128) as usize;
        chg_delete(i, start, p)?;
    }
    Ok(Value::Nil)
}

fn f_delete_backward_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU simple.el `delete-backward-char', ported:
    // integerp check first, then the active-region and overwrite-mode
    // untabify arms, then `delete-char (- n)'.
    let n = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("integerp", other)),
    };
    let killflag = a.get(1).cloned().unwrap_or(Value::Nil);
    {
        let b = cur(i);
        let bb = b.borrow();
        let dar = i.intern_soft("delete-active-region").unwrap_or(0);
        if region_active(i, &bb) && buf_var(i, &bb, dar).truthy() && n == 1 {
            // (if (eq delete-active-region 'kill)
            //     (kill-region (region-beginning) (region-end) 'region)
            //   (funcall region-extract-function 'delete-only))
            let darv = buf_var(i, &bb, dar);
            let kill = matches!(darv, Value::Sym(s) if s == i.intern_soft("kill").unwrap_or(0));
            drop(bb);
            if kill {
                let kr = i.intern("kill-region");
                let rb_sym = i.intern("region-beginning");
                let re_sym = i.intern("region-end");
                let reg = i.intern("region");
                let rb = i.apply(&Value::Sym(rb_sym), vec![])?;
                let re = i.apply(&Value::Sym(re_sym), vec![])?;
                i.apply(&Value::Sym(kr), vec![rb, re, Value::Sym(reg)])?;
            } else {
                let sid = i.intern("region-extract-function");
                let f = i.symbol_value(sid);
                let d = i.intern("delete-only");
                i.apply(&f, vec![Value::Sym(d)])?;
            }
            return Ok(Value::Nil);
        }
    }
    let untab = {
        let b = cur(i);
        let bb = b.borrow();
        let om = i.intern_soft("overwrite-mode").unwrap_or(0);
        // GNU: (memq (char-before) '(?\t ?\n)) and (eobp)/(char-after)
        // check on raw positions.
        let p = bb.point() as i128;
        let before = if p <= bb.begv as i128 || p > bb.zv as i128 {
            None
        } else {
            Some(crate::lisp::value::lisp_char_code(
                bb.text.char_at((p - 1) as usize),
            ))
        };
        let after = if p < bb.begv as i128 || p >= bb.zv as i128 {
            None
        } else {
            Some(crate::lisp::value::lisp_char_code(
                bb.text.char_at(p as usize),
            ))
        };
        buf_var(i, &bb, om).truthy()
            && n > 0
            && !matches!(before, Some(9) | Some(10))
            && after.is_some()
            && after != Some(10)
    };
    if untab {
        // Overwrite-mode untabify: delete backward, then insert spaces
        // to keep the column.
        let ocol = f_current_column(i, vec![].into())?;
        f_delete_char(i, vec![Value::Int(-n), killflag.clone()].into())?;
        let ncol = f_current_column(i, vec![].into())?;
        let gap = match (&ocol, &ncol) {
            (Value::Int(x), Value::Int(y)) => x - y,
            _ => 0,
        };
        if gap > 0 {
            f_insert_char(i, vec![Value::Int(' ' as i128), Value::Int(gap)].into())?;
        }
        Ok(Value::Nil)
    } else {
        f_delete_char(i, vec![Value::Int(-n), killflag].into())
    }
}

fn f_beginning_of_visual_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // tty: visual lines == logical lines.
    // GNU decodes N via `(/= n 1)' -> number-or-marker-p on non-numerics,
    // nil -> 1, and N targets "N - 1 lines forward".
    let n = opt_num_mark(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n - 1).max(0) as usize;
    let p = bb.text.line_start(target).max(bb.begv);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_end_of_visual_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = opt_num_mark(i, &arg(&a, 0))?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n - 1).max(0) as usize;
    let p = bb
        .text
        .line_end(bb.text.line_start(target))
        .min(bb.text_len());
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_forward_visible_line(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes with CHECK_NUMBER_COERCE_MARKER (number-or-marker-p).
    let n = want_num_mark_nil_err(i, &arg(&a, 0))?;
    let _ = f_forward_line(i, vec![Value::Int(n)].into())?;
    Ok(Value::Nil)
}

fn f_delete_and_extract_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    barf_if_file_locked(i)?;
    let s0 = want_int(i, &a[0])?;
    let e0 = want_int(i, &a[1])?;
    // GNU `validate_region' bounds-checks against the accessible
    // portion: out-of-[BEGV,ZV] args signal args-out-of-range with the
    // ORIGINAL argument values.
    let (s1, e1) = (s0.min(e0), s0.max(e0));
    {
        let b = cur(i);
        let bb = b.borrow();
        if s1 < bb.begv as i128 + 1 || e1 > bb.zv as i128 + 1 {
            // GNU validate_region: the payload leads with the buffer object.
            let bufv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![bufv, a[0].clone(), a[1].clone()],
            ));
        }
    }
    let (s, e) = ((s1 - 1) as usize, (e1 - 1) as usize);
    // GNU `delete_and_extract' copies the deleted text WITH its text
    // properties — grab them before the delete reshapes the table.
    let ivs = {
        let b = cur(i);
        let bb = b.borrow();
        buf_props_as_ivs(&bb.text.props.borrow(), s, e)
    };
    let text = chg_delete(i, s, e)?;
    let v = Value::string(text);
    if !ivs.is_empty() {
        if let Value::Str(sr) = &v {
            i.set_str_props(sr, ivs);
        }
    }
    Ok(v)
}

fn f_delete_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_delete_and_extract_region(i, a).map(|_| Value::Nil)
}

fn f_erase_buffer(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    barf_if_file_locked(i)?;
    let tlen = cur(i).borrow().text.len();
    chg_delete(i, 0, tlen)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.begv = 0;
    bb.zv = 0;
    bb.set_point(0);
    Ok(Value::Nil)
}

/// GNU's `<` argument order in `(kill-append string (< end beg))':
/// `end' is number-or-marker-checked first.  Used by the merge path
/// where REGION's non-nil value means BEG/END may hold arbitrary
/// junk — GNU still evaluates the comparison and signals.
fn num_or_mark_less(i: &mut Interp, end: &Value, beg: &Value) -> Result<bool, Flow> {
    let e = crate::editor::want_num_or_mark(i, end)?;
    let b = crate::editor::want_num_or_mark(i, beg)?;
    Ok(e < b)
}

/// GNU kill-append: prepend/append STRING to the kill-ring head
/// (pushing a fresh entry when the ring is empty), then feed the
/// merged head to `interprogram-cut-function'.
fn kill_append(i: &mut Interp, s: &str, before: bool) -> EvalResult {
    let kr = i.intern("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    match items.first_mut() {
        Some(Value::Str(top)) => {
            let mut t = top.borrow_mut();
            if before {
                let mut joined = s.to_string();
                joined.push_str(&t);
                *t = joined;
            } else {
                t.push_str(s);
            }
        }
        _ => items.insert(0, Value::string(s)),
    }
    i.obarray.symbol_mut(kr).value = Value::list(items);
    let ring = i.symbol_value(kr);
    let head = match &ring {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    interprogram_cut(i, head)
}

/// The `last-command'-driven part of GNU's `kill-region'/
/// `copy-region-as-kill' bodies: when the previous command was also a
/// kill, the new STRING merges into the ring head (`< end beg' picks
/// prepend vs append, evaluating END first); otherwise `kill-new'.
/// `word_kill' mirrors GNU's `(memq region '(unix-word emacs-word))'
/// exemption, which never merges.
pub(crate) fn kill_or_append(
    i: &mut Interp,
    s: &str,
    beg: &Value,
    end: &Value,
    word_kill: bool,
) -> EvalResult {
    let kr_sym = i.intern("kill-region");
    let lc = i.intern("last-command");
    let merging = !word_kill && matches!(&i.symbol_value(lc), Value::Sym(s) if *s == kr_sym);
    if merging {
        let before = num_or_mark_less(i, end, beg)?;
        kill_append(i, s, before)
    } else {
        push_kill_ring(i, s.to_string())
    }
}

/// GNU sets `this-command' to `kill-region' after a real kill when the
/// killed text was non-empty OR the previous command was a kill too.
fn set_kill_this_command(i: &mut Interp, s_is_some: bool, word_kill: bool) {
    let kr_sym = i.intern("kill-region");
    let lc = i.intern("last-command");
    let merging = !word_kill && matches!(&i.symbol_value(lc), Value::Sym(s) if *s == kr_sym);
    if s_is_some || merging {
        let tc = i.intern("this-command");
        i.obarray.symbol_mut(tc).value = Value::Sym(kr_sym);
    }
}

fn f_kill_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Record into kill-ring then delete.
    // GNU simple.el `kill-region': a non-nil REGION ignores BEG/END and
    // extracts the marked region via `region-extract-function' — which
    // signals "The mark is not set now, so there is no region" without a
    // mark.  `(memq region '(unix-word emacs-word))' uses a word-motion
    // variant that never merges and never marks this-command.
    let region = a.get(2).cloned().unwrap_or(Value::Nil);
    let uw = i.intern("unix-word");
    let ew = i.intern("emacs-word");
    let word_kill = match &region {
        Value::Sym(s) => *s == uw || *s == ew,
        _ => false,
    };
    let text = if region.truthy() {
        let (lo, hi) = mark_region_bounds(i)?;
        f_delete_and_extract_region(
            i,
            vec![Value::Int(lo as i128 + 1), Value::Int(hi as i128 + 1)].into(),
        )?
    } else {
        f_delete_and_extract_region(i, a.clone())?
    };
    // GNU: `(when string (merge-or-kill-new))' — the extracted string is
    // "nil if BEG = END"; ours is Some("") there, which still counts as a
    // killed string for GNU's `(< end beg)' evaluation.
    if let Value::Str(s) = &text {
        let s = s.borrow().clone();
        kill_or_append(
            i,
            &s,
            a.get(0).unwrap_or(&Value::Nil),
            a.get(1).unwrap_or(&Value::Nil),
            word_kill,
        )?;
    }
    set_kill_this_command(i, matches!(&text, Value::Str(_)), word_kill);
    let dm = i.intern("deactivate-mark");
    let _ = i.set_symbol(dm, Value::t());
    Ok(Value::Nil)
}

/// Cross-module access to `kill-region' (kill commands live in
/// `crate::editor').
pub(crate) fn f_kill_region_pub(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_kill_region(i, a)
}

/// GNU kill-new's `(if interprogram-cut-function (funcall ... string))':
/// every path that lands a string on `kill-ring' feeds it to the cut
/// function (gui-select-text on GUI/clipboard-capable builds).
pub(crate) fn interprogram_cut(i: &mut Interp, text: Value) -> EvalResult {
    if let Some(id) = i.intern_soft("interprogram-cut-function") {
        let f = i.symbol_value(id);
        if f.truthy() {
            i.apply(&f, vec![text])?;
        }
    }
    Ok(Value::Nil)
}

/// Push a string onto the kill-ring (a plain list var).
///
/// GNU never merges inside `kill-new': consecutive-kill merging is the
/// callers' business (`kill-region'/`copy-region-as-kill' call
/// `kill-append' when `last-command' is `kill-region'; the obsolete
/// `append-next-kill' command arranges the same via `last-command').
pub(crate) fn push_kill_ring(i: &mut Interp, s: String) -> EvalResult {
    let kr = i.intern("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    items.insert(0, Value::string(s.clone()));
    let max = i
        .symbol_value(i.intern_soft("kill-ring-max").unwrap_or(0))
        .int()
        .unwrap_or(120) as usize;
    items.truncate(max);
    i.obarray.symbol_mut(kr).value = Value::list(items);
    // GNU resets the yank pointer to the ring head on each kill.
    let ring = i.symbol_value(kr);
    let ptr = i.intern("kill-ring-yank-pointer");
    let _ = i.set_symbol(ptr, ring);
    interprogram_cut(i, Value::string(s))
}
fn f_append_next_kill(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU arranges the merge entirely through `this-command'/
    // `last-command' — the next kill sees `last-command = kill-region'
    // and kill-appends; no separate flag is needed (and a flag would
    // merge `kill-new' calls too, which GNU never does).
    let kr = Value::Sym(i.intern("kill-region"));
    if arg(&a, 0).truthy() {
        // GNU (simple.el): `(setq this-command 'kill-region)' then
        // `(message ...)' — the command returns the message string.
        let tc = i.intern("this-command");
        i.obarray.symbol_mut(tc).value = kr;
        let msg = "If the next command is a kill, it will append";
        i.message(msg);
        Ok(Value::string(msg))
    } else {
        // Non-interactive call: only `last-command' is set and returned.
        let lc = i.intern("last-command");
        i.obarray.symbol_mut(lc).value = kr.clone();
        Ok(kr)
    }
}

// ---------- buffer text access ----------

/// Build a (start, end, plist) interval list for buffer range [S,E),
/// rebased to 0 — the inverse of `copy_str_props': each maximal run of
/// positions sharing the same property set becomes one interval.
/// Reverse a flat plist's (PROP VALUE) pairs.  GNU's
/// `copy_text_properties' plputs each source pair in turn, so
/// string→string copies (substring, concat, mapconcat) yield plists
/// in reversed order; verbatim copies (insert, copy-sequence,
/// buffer-substring) preserve it.
pub(crate) fn plist_pairs_rev(pl: &[Value]) -> Vec<Value> {
    let mut out = Vec::with_capacity(pl.len());
    let mut k = pl.len();
    while k >= 2 {
        out.push(pl[k - 2].clone());
        out.push(pl[k - 1].clone());
        k -= 2;
    }
    out
}

fn buf_props_as_ivs(props: &[TextProp], s: usize, e: usize) -> Vec<(usize, usize, Vec<Value>)> {
    if !props.iter().any(|tp| tp.start < e && tp.end > s) {
        return Vec::new();
    }
    let mut cuts: Vec<usize> = vec![s, e];
    for tp in props {
        if tp.start < e && tp.end > s {
            cuts.push(tp.start.max(s));
            cuts.push(tp.end.min(e));
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut ivs = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b {
            continue;
        }
        // GNU's plist order is newest-first: `put-text-property'
        // prepends each property, so the effective plist lists the
        // most recently added props first.
        let mut plist: Vec<Value> = Vec::new();
        for tp in props.iter().rev() {
            if tp.start <= a && tp.end >= b {
                let psym = Value::Sym(tp.prop);
                let mut dup = false;
                let mut k = 0;
                while k + 1 < plist.len() {
                    if crate::lisp::builtins::eq_values(&plist[k], &psym) {
                        dup = true;
                        break;
                    }
                    k += 2;
                }
                if !dup {
                    plist.push(psym);
                    plist.push(tp.value.clone());
                }
            }
        }
        ivs.push((a - s, b - s, plist));
    }
    ivs
}

fn buffer_is_unibyte(i: &mut Interp) -> bool {
    let sym = i.intern("enable-multibyte-characters");
    !i.symbol_value(sym).truthy()
}
fn buffer_substring_impl(i: &mut Interp, a: &[Value], with_props: bool) -> EvalResult {
    let unibyte = buffer_is_unibyte(i);
    let b = cur(i);
    let bb = b.borrow();
    // GNU Fbuffer_substring validates against BEGV..ZV (narrowed),
    // and signals (args-out-of-range BUFFER START END) — the buffer
    // object first, like validate_interval_range.
    let len = bb.text_len();
    let s_raw = want_int(i, &a[0])?;
    let e_raw = want_int(i, &a[1])?;
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv.min(len) as i128 + 1);
    if s_raw < lo || s_raw > hi || e_raw < lo || e_raw > hi {
        let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
        drop(bb);
        return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![bv, a[0].clone(), a[1].clone()]));
    }
    let s = pos_idx(len, s_raw);
    let e = pos_idx(len, e_raw);
    let (s, e) = (s.min(e), s.max(e));
    let text = bb.text.substring(s, e);
    let ivs = if with_props {
        buf_props_as_ivs(&bb.text.props.borrow(), s, e)
    } else {
        Vec::new()
    };
    drop(bb);
    let v = Value::string(text);
    if unibyte {
        if let Value::Str(sr) = &v {
            i.mark_unibyte(sr);
        }
    }
    if !ivs.is_empty() {
        if let Value::Str(sr) = &v {
            i.set_str_props(sr, ivs);
        }
    }
    Ok(v)
}

pub(crate) fn f_buffer_substring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    buffer_substring_impl(i, &a, true)
}

pub(crate) fn f_buffer_substring_no_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    buffer_substring_impl(i, &a, false)
}

fn f_filter_buffer_substring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let delete = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    // `filter-buffer-substring-function' hook: GNU delegates the whole
    // operation — call3(FUN, BEG, END, DELETE) returns the substring.
    let f = i
        .intern_soft("filter-buffer-substring-function")
        .map(|h| i.symbol_value(h))
        .filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND))
        .unwrap_or(Value::Nil);
    if f.truthy() {
        // Hook receives already-evaluated VALUES — `apply' dispatches them
        // verbatim (`call_function' would re-evaluate a symbol arg like
        // `cdr' as a variable and signal void-variable).
        return i.apply(
            &f,
            vec![a[0].clone(), a[1].clone(), Value::from_bool(delete)],
        );
    }
    let text = f_buffer_substring(i, a[..2].to_vec().into())?;
    if delete {
        let (s, e) = {
            let b = cur(i);
            let bb = b.borrow();
            let len = bb.text.len();
            let s = pos_idx(len, want_int(i, &a[0])?);
            let e = pos_idx(len, want_int(i, &a[1])?);
            (s.min(e), s.max(e))
        };
        chg_delete(i, s, e)?;
    }
    Ok(text)
}

fn f_buffer_string(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let unibyte = buffer_is_unibyte(i);
    let b = cur(i);
    let bb = b.borrow();
    let (s, e) = (bb.begv, bb.text_len());
    let text = bb.text.substring(s, e);
    // GNU `buffer-string' copies the text properties like
    // `buffer-substring' does.
    let ivs = buf_props_as_ivs(&bb.text.props.borrow(), s, e);
    drop(bb);
    let v = Value::string(text);
    if unibyte {
        if let Value::Str(sr) = &v {
            i.mark_unibyte(sr);
        }
    }
    if !ivs.is_empty() {
        if let Value::Str(sr) = &v {
            i.set_str_props(sr, ivs);
        }
    }
    Ok(v)
}

fn f_buffer_substring_with_bidi_context(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = crate::editor::want_num_or_mark(i, &a[0])?;
    let e = crate::editor::want_num_or_mark(i, &a[1])?;
    let (s, e) = (s.min(e), s.max(e));
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text_len();
    if s < bb.begv as i128 + 1 || e > bb.zv.min(len) as i128 + 1 {
        let sym = i.intern("args-out-of-range");
        let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
        let args = vec![bv, a[0].clone(), a[1].clone()];
        drop(bb);
        return Err(i.signal_data(sym, args));
    }
    let (s0, e0) = ((s - 1) as usize, (e - 1) as usize);
    let text = bb.text.substring(s0, e0);
    let ivs = buf_props_as_ivs(&bb.text.props.borrow(), s0, e0);
    drop(bb);
    let v = Value::string(text);
    if !ivs.is_empty() {
        if let Value::Str(sr) = &v {
            i.set_str_props(sr, ivs);
        }
    }
    Ok(v)
}

fn thing_bounds(i: &mut Interp, pred: impl Fn(char) -> bool) -> Option<(usize, usize)> {
    let b = i.current_buffer_ref()?;
    let bb = b.borrow();
    let p = bb.point();
    let len = bb.text_len();
    if len == 0 {
        return None;
    }
    let mut s = p.min(len);
    let mut e = s;
    // If on a non-thing char, try char before.
    if (s >= len || !pred(bb.text.char_at(s))) && s > 0 && pred(bb.text.char_at(s - 1)) {
        s -= 1;
    }
    if s < len && !pred(bb.text.char_at(s)) {
        return None;
    }
    while s > 0 && pred(bb.text.char_at(s - 1)) {
        s -= 1;
    }
    while e < len && pred(bb.text.char_at(e)) {
        e += 1;
    }
    Some((s, e))
}

fn f_current_word(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU's `current-word' (simple.el) grabs syntaxes "w_" — word OR
    // symbol constituents — via the buffer's syntax table.
    let syn = crate::editor::syntax_table_entries(i);
    match thing_bounds(i, |c| {
        matches!(
            crate::editor::syntax_entry_code(syn.as_ref(), c),
            b'w' | b'_'
        )
    }) {
        Some((s, e)) => {
            let b = cur(i);
            Ok(Value::string(b.borrow().text.substring(s, e)))
        }
        None => Ok(Value::Nil),
    }
}

fn f_word_at_point(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_current_word(i, a)
}

// ---------- mark & region ----------

fn ma_id(i: &Interp) -> crate::lisp::value::SymId {
    i.intern_soft("mark-active").unwrap_or(0)
}

fn tmm_id(i: &Interp) -> crate::lisp::value::SymId {
    i.intern_soft("transient-mark-mode").unwrap_or(0)
}

/// Buffer-local binding if present, else the global value.  Works while
/// the buffer is already mutably borrowed (symbol_value would only see
/// the global binding then).
fn buf_var(i: &Interp, bb: &Buffer, id: crate::lisp::value::SymId) -> Value {
    match bb.locals.get(&id) {
        Some(v) => v.clone(),
        None => i.symbol_value(id),
    }
}

fn mark_active(i: &Interp, bb: &Buffer) -> bool {
    bb.locals
        .get(&ma_id(i))
        .map(|v| v.truthy())
        .unwrap_or(false)
}

/// GNU `region-active-p': `(and transient-mark-mode mark-active)'.
fn region_active(i: &Interp, bb: &Buffer) -> bool {
    buf_var(i, bb, tmm_id(i)).truthy() && mark_active(i, bb)
}

/// `(car-safe X)' is 'only'.
fn car_is_only(v: &Value, i: &Interp) -> bool {
    let only = i.intern_soft("only").unwrap_or(0);
    matches!(v, Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == only))
}

/// GNU `deactivate-mark': gated on (region-active-p) unless FORCE;
/// unwinds a temporary ('only . X) or 'lambda transient-mark-mode, then
/// clears mark-active.
fn deactivate_mark(i: &mut Interp, bb: &mut Buffer, force: bool) -> bool {
    if !(force || region_active(i, bb)) {
        return false;
    }
    let tmm = tmm_id(i);
    let tmmv = buf_var(i, bb, tmm);
    let lambda = i.intern_soft("lambda").unwrap_or(0);
    if car_is_only(&tmmv, i) {
        let nv = match &tmmv {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => Value::Nil,
        };
        // GNU's (setq tmm ...) writes the innermost binding: the
        // buffer-local one if present, else the dynamic/global value.
        if bb.locals.contains_key(&tmm) {
            bb.locals.insert(tmm, nv.clone());
        } else {
            i.obarray.symbol_mut(tmm).value = nv.clone();
        }
        // (if (eq tmm (default-value 'tmm)) (kill-local-variable 'tmm))
        let dv = i.obarray.symbol(tmm).value.clone();
        if crate::lisp::eq_values(&nv, &dv) {
            bb.locals.remove(&tmm);
        }
    } else if matches!(&tmmv, Value::Sym(s) if *s == lambda) {
        bb.locals.remove(&tmm);
    }
    bb.locals.insert(ma_id(i), Value::Nil);
    true
}

/// GNU `activate-mark' core: set mark-active; with NO-TMM leave
/// transient-mark-mode alone, else set it buffer-locally to 'lambda
/// when unset.  Only runs when the mark exists and region is inactive.
fn activate_mark(i: &Interp, bb: &mut Buffer, no_tmm: bool) -> bool {
    if bb.mark_position().is_none() || region_active(i, bb) {
        return false;
    }
    bb.locals.insert(ma_id(i), Value::t());
    let tmm = tmm_id(i);
    if !(buf_var(i, bb, tmm).truthy() || no_tmm) {
        let lambda = i.intern_soft("lambda").unwrap_or(0);
        bb.locals.insert(tmm, Value::Sym(lambda));
    }
    true
}

fn f_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: (or force (not tmm) mark-active mark-even-if-inactive) else
    // signal 'mark-inactive.
    let mei = i.intern_soft("mark-even-if-inactive").unwrap_or(0);
    let ok = a.get(0).map(|v| v.truthy()).unwrap_or(false)
        || !buf_var(i, &bb, tmm_id(i)).truthy()
        || mark_active(i, &bb)
        || buf_var(i, &bb, mei).truthy();
    match bb.mark_position() {
        _ if !ok => Err(err_sym(i, "mark-inactive", vec![])),
        Some(m) => Ok(Value::Int(m as i128 + 1)),
        None => Ok(Value::Nil),
    }
}

fn f_set_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if a[0].truthy() {
        let len = bb.text.len();
        let p = match &a[0] {
            Value::Marker(m) => m.borrow().position,
            v => pos_idx(len, want_int(i, v)?),
        };
        bb.set_mark(p);
        // (activate-mark 'no-tmm): mark-active on, tmm untouched —
        // its hook still runs when the mark newly activates.
        let fired = if !region_active(i, &bb) {
            bb.locals.insert(ma_id(i), Value::t());
            true
        } else {
            false
        };
        drop(bb);
        if fired {
            call_hook(i, "activate-mark-hook")?;
        }
        Ok(Value::Nil)
    } else {
        // GNU: (deactivate-mark t) then detach the mark marker
        // ((set-marker mark nil)); the marker object stays live and
        // `mark-marker' keeps returning it.  GNU returns the marker.
        let fired = deactivate_mark(i, &mut bb, true);
        bb.locals.insert(ma_id(i), Value::Nil);
        let m = bb.mark_marker();
        m.borrow_mut().buffer = None;
        drop(bb);
        if fired {
            call_hook(i, "deactivate-mark-hook")?;
        }
        Ok(marker_value(m))
    }
}

fn f_mark_marker(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    Ok(marker_value(bb.mark_marker()))
}

fn f_push_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let p = match a.get(0) {
        Some(v) if v.truthy() => pos_idx(len, want_int(i, v)?),
        _ => bb.point(),
    };
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    // GNU pushes the OLD mark onto mark-ring before installing the new
    // one, via (add-to-history 'mark-ring (copy-marker (mark-marker))
    // mark-ring-max t): cons a fresh head cell (sharing the existing
    // ring's conses), truncate by (setcdr (nthcdr (1- MAX) ring) nil)
    // in place, and detach the marker at index MAX — the element that
    // falls off a full ring.
    if let Some(old) = bb.mark_position() {
        let old_ring = bb
            .locals
            .get(&ring_sym)
            .cloned()
            .unwrap_or(Value::Nil);
        let max = buf_var(i, &bb, i.intern_soft("mark-ring-max").unwrap_or(0))
            .int()
            .unwrap_or(16) as usize;
        let m = Rc::new(RefCell::new(Marker {
            buffer: Some(bb.id),
            position: old,
            insertion_type: false,
        }));
        bb.register_marker(&m);
        let new_ring = Value::cons(marker_value(m), old_ring.clone());
        // GNU's push-mark grabs (nth mark-ring-max mark-ring) BEFORE
        // prepending and detaches it after: the element one past a
        // FULL old ring, i.e. the same marker truncation drops.
        let mut old_at_max = old_ring.clone();
        for _ in 0..max {
            let n = match &old_at_max {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => Value::Nil,
            };
            if let Value::Cons(_) = n {
                old_at_max = n;
            } else {
                old_at_max = Value::Nil;
                break;
            }
        }
        // add-to-history cuts the chain in place: (setcdr (nthcdr
        // (1- MAX) new-ring) nil), so conses shared with a captured
        // reference observe the truncation.
        if max > 0 {
            let mut cut = new_ring.clone();
            for _ in 0..max - 1 {
                let n = match &cut {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                if let Value::Cons(_) = n {
                    cut = n;
                } else {
                    break;
                }
            }
            if let Value::Cons(c) = &cut {
                if matches!(c.borrow().cdr, Value::Cons(_)) {
                    c.borrow_mut().cdr = Value::Nil;
                }
            }
            if let Value::Cons(c) = &old_at_max {
                if let Value::Marker(mm) = c.borrow().car.clone() {
                    mm.borrow_mut().buffer = None;
                }
            }
        }
        bb.locals.insert(ring_sym, new_ring);
    }
    bb.set_mark(p);
    // GNU pushes the new mark onto `global-mark-ring' unless its car is
    // already in this buffer.
    let gmr = i.intern_soft("global-mark-ring").unwrap_or(0);
    let gring = i.symbol_value(gmr).list_to_vec().unwrap_or_default();
    let same_buf = matches!(
        gring.first(),
        Some(Value::Marker(m)) if m.borrow().buffer == Some(bb.id)
    );
    if !same_buf {
        let m = Rc::new(RefCell::new(Marker {
            buffer: Some(bb.id),
            position: p,
            insertion_type: false,
        }));
        bb.register_marker(&m);
        let mut items = gring;
        items.insert(0, marker_value(m));
        let max = i
            .symbol_value(i.intern_soft("global-mark-ring-max").unwrap_or(0))
            .int()
            .unwrap_or(16) as usize;
        items.truncate(max);
        i.set_symbol(gmr, Value::list(items))?;
    }
    // (if (or activate (not transient-mark-mode)) (set-mark (mark t)))
    let activate = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    if activate || !buf_var(i, &bb, tmm_id(i)).truthy() {
        // set-mark on the same position: activates mark-active only.
        if !region_active(i, &bb) {
            bb.locals.insert(ma_id(i), Value::t());
        }
    }
    let nomsg = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let kmacro = buf_var(i, &bb, i.intern_soft("executing-kbd-macro").unwrap_or(0)).truthy();
    let in_mini = buf_var(i, &bb, i.intern_soft("minibuffer-depth").unwrap_or(0))
        .int()
        .unwrap_or(0)
        > 0;
    drop(bb);
    if !(nomsg || kmacro || in_mini) {
        i.message("Mark set");
    }
    Ok(Value::Nil)
}

fn f_pop_mark(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    let ring = bb
        .locals
        .get(&ring_sym)
        .cloned()
        .unwrap_or(Value::Nil);
    if let Value::Cons(_) = ring {
        // GNU: (nconc mark-ring (list (copy-marker (mark-marker))))
        // mutates the ring's LAST cons in place, so code holding a
        // reference to the ring's head cons sees the appended copy —
        // e.g. (prog1 (list mark-ring) (pop-mark)) prints it.
        if let Some(mpos) = bb.mark_position() {
            let m = Rc::new(RefCell::new(Marker {
                buffer: Some(bb.id),
                position: mpos,
                insertion_type: false,
            }));
            bb.register_marker(&m);
            let mut tail = ring.clone();
            loop {
                let next = match &tail {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => break,
                };
                match next {
                    Value::Cons(_) => tail = next,
                    _ => break,
                }
            }
            if let Value::Cons(c) = &tail {
                c.borrow_mut().cdr = Value::list(vec![marker_value(m)]);
            }
        }
        // (set-marker (mark-marker) (car mark-ring)): the ring's head
        // marker becomes the buffer's mark.
        let head_marker = match &ring {
            Value::Cons(c) => match c.borrow().car.clone() {
                Value::Marker(mm) => Some(mm),
                _ => None,
            },
            _ => None,
        };
        if let Some(mm) = &head_marker {
            let len = bb.text.len();
            bb.set_mark(mm.borrow().position.min(len));
        }
        // (set-marker (car mark-ring) nil): detach the popped marker —
        // captured references then print it "in no buffer" like GNU.
        if let Some(mm) = &head_marker {
            mm.borrow_mut().buffer = None;
        }
        // (pop mark-ring): rebind the variable to (cdr mark-ring); the
        // head cons itself is untouched (its cdr keeps the rest).
        let newring = match &ring {
            Value::Cons(c) => c.borrow().cdr.clone(),
            _ => Value::Nil,
        };
        bb.locals.insert(ring_sym, newring);
    }
    // GNU's pop-mark calls (deactivate-mark) unconditionally; it is
    // itself gated on (region-active-p).
    let fired = deactivate_mark(i, &mut bb, false);
    drop(bb);
    if fired {
        call_hook(i, "deactivate-mark-hook")?;
    }
    Ok(Value::Nil)
}

// --- Registers ---------------------------------------------------------
// `register-alist' holds (KEY . VALUE) cells; keys are compared with
// `equal' (conventionally character codes, but any object works).

fn reg_alist(i: &mut Interp) -> (crate::lisp::value::SymId, Value) {
    let sym = i.intern("register-alist");
    let v = i.symbol_value(sym);
    (sym, v)
}

fn reg_set(i: &mut Interp, key: Value, val: Value) -> Result<(), Flow> {
    let (sym, alist) = reg_alist(i);
    let mut items = alist.list_to_vec().unwrap_or_default();
    let mut found = false;
    for it in &mut items {
        if let Value::Cons(c) = it {
            if crate::lisp::builtins::equal_values(i, &c.borrow().car, &key) {
                c.borrow_mut().cdr = val.clone();
                found = true;
                break;
            }
        }
    }
    if !found {
        // GNU prepends new cells to register-alist.
        items.insert(0, Value::cons(key, val));
    }
    i.set_symbol(sym, Value::list(items))
}

fn reg_get(i: &mut Interp, key: &Value) -> Value {
    let (_, alist) = reg_alist(i);
    for it in alist.list_to_vec().unwrap_or_default() {
        if let Value::Cons(c) = &it {
            if crate::lisp::builtins::equal_values(i, &c.borrow().car, key) {
                return c.borrow().cdr.clone();
            }
        }
    }
    Value::Nil
}

fn f_set_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU keeps the (key) cell even when VALUE is nil.
    reg_set(i, a[0].clone(), a[1].clone())?;
    Ok(a[1].clone())
}

fn f_get_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(reg_get(i, &a[0]))
}

fn f_number_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(_) | Value::Float(_) => {}
        other => return Err(i.wrong_type_mut("numberp", other)),
    }
    reg_set(i, a[1].clone(), a[0].clone())?;
    Ok(a[0].clone())
}

/// The "mark is not set" error GNU signals when register commands need
/// the region but no mark exists.
fn no_mark_err(i: &mut Interp) -> Flow {
    i.signal_data(
        sym::ERROR,
        vec![Value::string(
            "The mark is not set now, so there is no region",
        )],
    )
}

pub(crate) fn mark_region_bounds(i: &mut Interp) -> Result<(usize, usize), Flow> {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark_position() {
        Some(m) => {
            let p = bb.point();
            Ok((m.min(p), m.max(p)))
        }
        None => Err(no_mark_err(i)),
    }
}

fn f_increment_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU rounds the increment up (42+0.5 → 43, 50+0.5 → 51).
    let nf = match &a[0] {
        Value::Int(x) => *x as f64,
        Value::Float(f) => f.ceil(),
        other => return Err(i.wrong_type_mut("numberp", other)),
    };
    let reg = a[1].clone();
    let val = reg_get(i, &reg);
    if let Value::Int(old) = &val {
        let nv = Value::Int(old + nf as i128);
        reg_set(i, reg, nv.clone())?;
        return Ok(nv);
    }
    if let Value::Float(old) = &val {
        let nv = Value::float(**old + nf);
        reg_set(i, reg, nv.clone())?;
        return Ok(nv);
    }
    // Anything else (string, nil, unset) appends the active region's
    // text to the register, erroring when no mark is set.
    let (lo, hi) = mark_region_bounds(i)?;
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.substring(lo, hi)
    };
    let prefix = match &val {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => String::new(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    reg_set(i, reg, Value::string(format!("{prefix}{text}")))?;
    Ok(Value::Nil)
}

fn f_point_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (buf, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    let m = new_marker_at(i, buf, pos);
    reg_set(i, a[0].clone(), m.clone())?;
    Ok(m)
}

fn f_jump_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let val = reg_get(i, &a[0]);
    match &val {
        // GNU register_val_jump: a marker whose buffer is dead falls
        // through to the user-error like any non-position value.
        Value::Marker(m) if m.borrow().buffer.is_some() => {
            let (pos, buf) = {
                let mm = m.borrow();
                (mm.position, mm.buffer)
            };
            if let Some(bid) = buf {
                i.set_current_buffer(bid);
            }
            cur(i).borrow_mut().set_point(pos);
            Ok(val.clone())
        }
        Value::Marker(_) => Err(reg_no_pos_err(i)),
        Value::Cons(c) => {
            let (head, rest) = {
                let cc = c.borrow();
                (cc.car.clone(), cc.cdr.clone())
            };
            let head_name = match &head {
                Value::Sym(s) => i.symbol_name(*s),
                _ => String::new(),
            };
            match head_name.as_str() {
                "window-configuration" | "frame-configuration" => {
                    // Register value is (head marker); GNU jumps to the
                    // marker and returns it.
                    let items = val.list_to_vec().unwrap_or_default();
                    Ok(items.into_iter().nth(1).unwrap_or(rest))
                }
                "buffer" => {
                    let name = match &rest {
                        Value::Str(s) => Value::Str(s.clone()),
                        v => v.clone(),
                    };
                    f_set_buffer(i, vec![name].into()).map(|_| val.clone())
                }
                "file" | "file-query" => {
                    let target = rest.list_to_vec().ok().and_then(|v| v.into_iter().next());
                    match target {
                        Some(name) => {
                            let find = Value::Sym(i.intern("find-file"));
                            i.apply(&find, vec![name])?;
                            Ok(val.clone())
                        }
                        None => Err(reg_no_pos_err(i)),
                    }
                }
                _ => Err(reg_no_pos_err(i)),
            }
        }
        _ => Err(reg_no_pos_err(i)),
    }
}

fn reg_no_pos_err(i: &mut Interp) -> Flow {
    err_sym(
        i,
        "user-error",
        vec![Value::string(
            "Register doesn’t contain a buffer position or configuration",
        )],
    )
}

fn f_insert_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let val = reg_get(i, &a[0]);
    let before = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    match &val {
        Value::Str(s) => {
            let t = s.borrow().clone();
            insert_str_at_point(i, &t, false)?;
            if before {
                let b = cur(i);
                let mut bb = b.borrow_mut();
                let p = bb.point();
                bb.set_point(p.saturating_sub(t.chars().count()));
            }
            Ok(Value::Nil)
        }
        Value::Int(_) | Value::Float(_) => {
            let t = i.princ_to_string(&val);
            insert_str_at_point(i, &t, false)?;
            if before {
                let b = cur(i);
                let mut bb = b.borrow_mut();
                let p = bb.point();
                bb.set_point(p.saturating_sub(t.chars().count()));
            }
            Ok(Value::Nil)
        }
        // GNU register-val-insert on a marker: princ its position;
        // dead markers (no buffer) fall to the user-error path.
        Value::Marker(m) if m.borrow().buffer.is_some() => {
            let pos = m.borrow().position;
            let t = format!("{}", pos);
            insert_str_at_point(i, &t, false)?;
            Ok(Value::Nil)
        }
        Value::Marker(_) => Err(err_sym(
            i,
            "user-error",
            vec![Value::string("Register does not contain text")],
        )),
        // GNU insert_register: non-textual register contents signal
        // `wrong-type-argument buffer-or-string-p' with the contents.
        other => Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
}

fn f_copy_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // REGION non-nil means START/END are ignored and the marked region
    // is used (GNU's 5th arg).
    let (lo, hi) = if a.get(4).map(|v| v.truthy()).unwrap_or(false) {
        mark_region_bounds(i)?
    } else {
        let b = cur(i);
        let bb = b.borrow();
        let len = bb.text.len();
        let s = pos_idx(len, want_int(i, &a[1])?);
        let e = pos_idx(len, want_int(i, &a[2])?);
        drop(bb);
        (s.min(e), s.max(e))
    };
    let delete = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let text = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.substring(lo, hi)
    };
    if delete {
        chg_delete(i, lo, hi)?;
    }
    reg_set(i, a[0].clone(), Value::string(text))?;
    Ok(Value::Nil)
}

fn f_window_configuration_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU stores (config marker); our config object is stubbed, so the
    // value is (window-configuration marker) — jump-to-register
    // dispatches on the head and returns the marker.
    let head = Value::Sym(i.intern("window-configuration"));
    let (buf, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    let cell = Value::list(vec![head, new_marker_at(i, buf, pos)]);
    reg_set(i, a[0].clone(), cell.clone())?;
    Ok(cell)
}

fn f_frame_configuration_to_register(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let head = Value::Sym(i.intern("frame-configuration"));
    let (buf, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    let cell = Value::list(vec![head, new_marker_at(i, buf, pos)]);
    reg_set(i, a[0].clone(), cell.clone())?;
    Ok(cell)
}

// --- Rectangles ----------------------------------------------------------
// Faithful port of GNU rect.el: rectangles live in *display* columns.
// move-to-column semantics by FORCE:
//   nil    — land on a char boundary (overshooting wide chars), no EOL pad
//   coerce — additionally split tabs (insert spaces before them);
//            still no EOL pad, still overshoots other wide chars
//   t      — like coerce, plus pads short lines with spaces
// The per-line functions combine these exactly like rect.el.

#[derive(Clone, Copy, PartialEq)]
enum RectForce {
    Nil,
    Coerce,
    T,
}

/// GNU's `char_width' (character.c): the display column width of one
/// buffer character.  Control chars (with `ctl-arrow' t) render as
/// `^X' = 2 columns; C1 chars render as octal `\NNN' = 4; everything
/// else comes from `char-width-table' (CHAR_WIDTH_RANGES), which also
/// covers zero-width (tag) chars and East-Asian double-width chars.
/// TAB and newline are handled by the caller.
pub(crate) fn char_width(c: char) -> i128 {
    // PUA proxies stand in for chars a Rust `char' can't hold:
    // eight-bit codes and ISO control chars render 4 columns
    // (GNU's default char-width-table), surrogates/high-mule 1.
    if crate::lisp::value::eight_bit_byte(c).is_some() {
        return 4;
    }
    if crate::lisp::value::surrogate_code(c).is_some()
        || crate::lisp::value::mule_seq_code(c).is_some()
        || crate::lisp::value::raw_byte_marker(c).is_some()
    {
        return 1;
    }
    let n = c as u32;
    if n < 0x20 || n == 0x7f {
        return 2;
    }
    if n < 0x80 {
        return 1;
    }
    let ranges = crate::lisp::ctdata::CHAR_WIDTH_RANGES;
    let mut lo = 0usize;
    let mut hi = ranges.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (s, e, _) = ranges[mid];
        if n < s {
            hi = mid;
        } else if n > e {
            lo = mid + 1;
        } else {
            return ranges[mid].2 as i128;
        }
    }
    // Characters above the table (0x110000+, unreachable in our
    // UTF-8 text) display one column.
    1
}

fn rect_char_width(c: char, tab: i128) -> i128 {
    let _ = tab;
    char_width(c)
}

fn rect_tab_width(i: &Interp) -> i128 {
    i.symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1)
}

/// Column of char index P on the line starting at LS.
pub(crate) fn rect_col_at(
    text: &crate::buffer::SharedText,
    ls: usize,
    p: usize,
    tab: i128,
) -> i128 {
    let mut col = 0i128;
    let mut k = ls;
    while k < p {
        let ch = text.char_at(k);
        col += if ch == '\t' {
            (col / tab + 1) * tab - col
        } else {
            rect_char_width(ch, tab)
        };
        k += 1;
    }
    col
}

/// (char index, column reached) for `move-to-column COL force` on the
/// line [LS, LE). With force this may mutate the buffer (split a tab /
/// pad the EOL); LE must be the current line end.
fn rect_move_to(
    bb: &mut Buffer,
    ls: usize,
    le: usize,
    col: i128,
    tab: i128,
    force: RectForce,
) -> (usize, i128) {
    let mut c = 0i128;
    let mut k = ls;
    while k < le {
        let ch = bb.text.char_at(k);
        let w = if ch == '\t' {
            (c / tab + 1) * tab - c
        } else {
            rect_char_width(ch, tab)
        };
        if c + w > col {
            if c == col {
                return (k, col);
            }
            if ch == '\t' && force != RectForce::Nil {
                // Split the tab: insert (col - c) spaces before it.
                let pad = (col - c) as usize;
                bb.insert_at(k, &" ".repeat(pad));
                return (k + pad, col);
            }
            return (k + 1, c + w); // overshoot: point lands after the char
        }
        c += w;
        k += 1;
    }
    if force == RectForce::T && c < col {
        let pad = (col - c) as usize;
        bb.insert_at(le, &" ".repeat(pad));
        return (le + pad, col);
    }
    (le, c)
}

/// (start-line, end-line, c0, c1, tab): normalized corners like
/// apply-on-rectangle (columns swapped when END is left of START).
fn rect_corners(i: &mut Interp, a: &[Value]) -> Result<(usize, usize, i128, i128, i128), Flow> {
    let tab = rect_tab_width(i);
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let sp = pos_idx(len, want_int(i, &a[0])?);
    let ep = pos_idx(len, want_int(i, &a[1])?);
    let sl = bb.text.line_of_pos(sp);
    let el = bb.text.line_of_pos(ep);
    let c0 = rect_col_at(&bb.text, bb.text.line_start(sl), sp, tab);
    let c1 = rect_col_at(&bb.text, bb.text.line_start(el), ep, tab);
    Ok(if c0 <= c1 {
        (sl, el, c0, c1, tab)
    } else {
        (sl, el, c1, c0, tab)
    })
}

/// Line numbers GNU's apply-on-rectangle visits: the start line always,
/// then following lines through END's line.
fn rect_line_range(i: &mut Interp, a: &[Value]) -> Result<(usize, usize, i128, i128, i128), Flow> {
    let (sl, el, c0, c1, tab) = rect_corners(i, a)?;
    let last = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.line_of_pos(bb.text.len())
    };
    let hi = if el > sl { el.min(last) } else { sl };
    Ok((sl, hi, c0, c1, tab))
}

/// Visit each rectangle line. Line bounds are recomputed per line since
/// per-line ops shift positions within their own line only.
fn rect_apply(
    i: &mut Interp,
    a: &[Value],
    mut f: impl FnMut(&mut Buffer, usize, usize, i128, i128, i128) -> Result<(), Flow>,
) -> EvalResult {
    check_writable(i)?;
    let (sl, hi, c0, c1, tab) = rect_line_range(i, a)?;
    let b = cur(i);
    let orig = b.borrow().point();
    let mut final_point = orig;
    for ln in sl..=hi {
        let (ls, le) = {
            let bb = b.borrow();
            let ls = bb.text.line_start(ln);
            (ls, bb.text.line_end(ls))
        };
        // GNU fires the change hooks per line edit; the [ls,le) span
        // covers the line's mutation batch (as `combine-change-calls'
        // would report it).
        signal_before_change(i, ls + 1, le + 1)?;
        {
            let mut bb = b.borrow_mut();
            f(&mut bb, ls, le, c0, c1, tab)?;
            final_point = bb.point();
        }
        let le2 = b.borrow().text.line_end(ls);
        signal_after_change(i, ls + 1, le2 + 1, (le - ls) as i128)?;
    }
    // GNU wraps apply-on-rectangle in save-excursion.
    let restore = orig.min(b.borrow().text.len());
    b.borrow_mut().set_point(restore);
    Ok(Value::Int(final_point as i128 + 1))
}

/// GNU delete-rectangle-line; returns the position of column SC.
fn rect_delete_line(
    bb: &mut Buffer,
    ls: usize,
    le: usize,
    sc: i128,
    ec: i128,
    fill: bool,
    tab: i128,
) -> usize {
    let (p0, reached) = rect_move_to(
        bb,
        ls,
        le,
        sc,
        tab,
        if fill {
            RectForce::T
        } else {
            RectForce::Coerce
        },
    );
    if reached >= sc {
        let le2 = bb.text.line_end(ls);
        let (p1, _) = rect_move_to(bb, ls, le2, ec, tab, RectForce::Coerce);
        if p1 > p0 {
            bb.delete_region(p0, p1);
        }
    }
    p0
}

/// GNU delete-extract-rectangle-line: kill the span, return the segment.
fn rect_extract_delete_line(
    bb: &mut Buffer,
    ls: usize,
    le: usize,
    sc: i128,
    ec: i128,
    fill: bool,
    tab: i128,
) -> String {
    let (p0, reached) = rect_move_to(
        bb,
        ls,
        le,
        sc,
        tab,
        if fill {
            RectForce::T
        } else {
            RectForce::Coerce
        },
    );
    if reached < sc {
        // Line ends before the rectangle's left edge: GNU stores blanks
        // and leaves the line untouched.
        return " ".repeat((ec - sc).max(0) as usize);
    }
    let le2 = bb.text.line_end(ls);
    let (p1, _) = rect_move_to(bb, ls, le2, ec, tab, RectForce::T);
    bb.delete_region(p0, p1)
}

/// GNU extract-rectangle-line: non-destructive, padded with spaces.
fn rect_extract_line(
    text: &crate::buffer::SharedText,
    ls: usize,
    le: usize,
    sc: i128,
    ec: i128,
    tab: i128,
) -> String {
    // move-to-column without force: no pad, no split.
    let reach = |col: i128| -> (usize, i128) {
        let mut c = 0i128;
        let mut k = ls;
        while k < le {
            let ch = text.char_at(k);
            let w = if ch == '\t' {
                (c / tab + 1) * tab - c
            } else {
                rect_char_width(ch, tab)
            };
            if c + w > col {
                return if c == col { (k, col) } else { (k + 1, c + w) };
            }
            c += w;
            k += 1;
        }
        (le, c)
    };
    let (p0, r0) = reach(sc);
    let (p1, r1) = reach(ec);
    // Tabs inside the span expand to their display width.
    let mut seg = String::new();
    let mut col = rect_col_at(text, ls, p0, tab);
    for k in p0..p1.min(le) {
        let ch = text.char_at(k);
        if ch == '\t' {
            let w = (col / tab + 1) * tab - col;
            seg.extend(std::iter::repeat(' ').take(w as usize));
            col += w;
        } else {
            seg.push(ch);
            col += rect_char_width(ch, tab);
        }
    }
    let mut begextra = r0 - sc;
    let mut endextra = ec - r1;
    if begextra < 0 {
        endextra += begextra;
        begextra = 0;
    }
    if endextra < 0 {
        endextra = 0;
    }
    format!(
        "{}{}{}",
        " ".repeat(begextra as usize),
        seg,
        " ".repeat(endextra as usize)
    )
}

fn rect_set_killed(i: &mut Interp, segs: Vec<String>) {
    let sym = i.intern("killed-rectangle");
    let _ = i.set_symbol(
        sym,
        Value::list(segs.into_iter().map(Value::string).collect()),
    );
}

fn rect_extract_span(
    i: &mut Interp,
    a: &[Value],
    delete: bool,
    fill: bool,
) -> Result<Vec<String>, Flow> {
    let (sl, hi, c0, c1, tab) = rect_line_range(i, a)?;
    let b = cur(i);
    let mut segs = Vec::new();
    for ln in sl..=hi {
        let (ls, le) = {
            let bb = b.borrow();
            let ls = bb.text.line_start(ln);
            (ls, bb.text.line_end(ls))
        };
        if delete {
            let mut bb = b.borrow_mut();
            segs.push(rect_extract_delete_line(&mut bb, ls, le, c0, c1, fill, tab));
        } else {
            let bb = b.borrow();
            segs.push(rect_extract_line(&bb.text, ls, le, c0, c1, tab));
        }
    }
    Ok(segs)
}

fn f_kill_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let segs = rect_extract_span(i, &a, true, fill)?;
    rect_set_killed(i, segs);
    Ok(Value::Nil)
}

fn f_delete_extract_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let segs = rect_extract_span(i, &a, true, fill)?;
    Ok(Value::list(segs.into_iter().map(Value::string).collect()))
}

fn f_extract_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let segs = rect_extract_span(i, &a, false, false)?;
    Ok(Value::list(segs.into_iter().map(Value::string).collect()))
}

fn f_copy_rectangle_as_kill(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let segs = rect_extract_span(i, &a, false, false)?;
    rect_set_killed(i, segs);
    Ok(Value::Nil)
}

fn f_delete_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    rect_apply(i, &a, |bb, ls, le, c0, c1, tab| {
        let p0 = rect_delete_line(bb, ls, le, c0, c1, fill, tab);
        bb.set_point(p0);
        Ok(())
    })
}

fn f_clear_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    rect_apply(i, &a, |bb, ls, le, c0, c1, tab| {
        // GNU clear-rectangle-line.
        let eol_col = rect_col_at(&bb.text, ls, le, tab);
        let (p0, reached) = rect_move_to(
            bb,
            ls,
            le,
            c0,
            tab,
            if fill {
                RectForce::T
            } else {
                RectForce::Coerce
            },
        );
        if reached == c0 {
            if !fill && eol_col <= c1 {
                bb.delete_region(p0, le);
                bb.set_point(p0);
            } else {
                let le2 = bb.text.line_end(ls);
                let (p1, _) = rect_move_to(bb, ls, le2, c1, tab, RectForce::T);
                bb.delete_region(p0, p1);
                let cur = rect_col_at(&bb.text, ls, p0, tab);
                if c1 > cur {
                    bb.insert_at(p0, &" ".repeat((c1 - cur) as usize));
                    bb.set_point(p0 + (c1 - cur) as usize);
                } else {
                    bb.set_point(p0);
                }
            }
        }
        Ok(())
    })
}

fn f_open_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    rect_apply(i, &a, |bb, ls, le, c0, c1, tab| {
        // GNU open-rectangle-line.
        let (p0, reached) = rect_move_to(
            bb,
            ls,
            le,
            c0,
            tab,
            if fill {
                RectForce::T
            } else {
                RectForce::Coerce
            },
        );
        if reached == c0 && (fill || p0 != bb.text.line_end(ls)) {
            let cur = rect_col_at(&bb.text, ls, p0, tab);
            if c1 > cur {
                bb.insert_at(p0, &" ".repeat((c1 - cur) as usize));
            }
            bb.set_point(p0 + (c1 - cur).max(0) as usize);
        }
        Ok(())
    })
}

fn f_delete_whitespace_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    rect_apply(i, &a, |bb, ls, le, c0, _c1, tab| {
        // GNU delete-whitespace-rectangle-line (close-rectangle).
        let (p0, reached) = rect_move_to(
            bb,
            ls,
            le,
            c0,
            tab,
            if fill {
                RectForce::T
            } else {
                RectForce::Coerce
            },
        );
        if reached == c0 && p0 != bb.text.line_end(ls) {
            let mut p1 = p0;
            while p1 < le && matches!(bb.text.char_at(p1), ' ' | '\t') {
                p1 += 1;
            }
            if p1 > p0 {
                bb.delete_region(p0, p1);
            }
            bb.set_point(p0);
        }
        Ok(())
    })
}

fn rect_string_lines(i: &mut Interp, a: &[Value], s: &str, delete: bool) -> EvalResult {
    rect_apply(i, a, |bb, ls, le, c0, c1, tab| {
        // GNU string-rectangle-line.
        rect_move_to(bb, ls, le, c0, tab, RectForce::T);
        let p0 = if delete {
            rect_delete_line(bb, ls, le, c0, c1, false, tab)
        } else {
            let le2 = bb.text.line_end(ls);
            rect_move_to(bb, ls, le2, c0, tab, RectForce::Nil).0
        };
        bb.insert_at(p0, s);
        bb.set_point(p0 + s.chars().count());
        Ok(())
    })
}

/// GNU accepts a char-or-string for the rectangle text (insert handles
/// both): an integer inserts as that character.
fn rect_string_arg(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Str(s) => Ok(s.borrow().clone()),
        Value::Int(n) => char::from_u32(*n as u32)
            .map(|c| c.to_string())
            .ok_or_else(|| i.wrong_type_mut("char-or-string-p", v)),
        other => Err(i.wrong_type_mut("char-or-string-p", other)),
    }
}

fn f_string_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = rect_string_arg(i, &a[2])?;
    rect_string_lines(i, &a, &s, true)
}

fn f_string_insert_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = rect_string_arg(i, &a[2])?;
    rect_string_lines(i, &a, &s, false)
}

/// Shared body of yank-rectangle / insert-rectangle: GNU's
/// insert-rectangle — first line at point, following lines at the same
/// column (coerced with `t`), creating lines at EOF.
fn rect_insert_segs(i: &mut Interp, segs: Vec<String>) -> EvalResult {
    check_writable(i)?;
    // GNU pushes the mark at the upper-left corner unconditionally.
    f_push_mark(i, vec![].into())?;
    if segs.is_empty() {
        return Ok(Value::Nil);
    }
    let tab = rect_tab_width(i);
    let p0 = cur(i).borrow().point();
    signal_before_change(i, p0 + 1, p0 + 1)?;
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        let c0 = {
            let p = bb.point();
            let ls = bb.text.line_start(bb.text.line_of_pos(p));
            rect_col_at(&bb.text, ls, p, tab)
        };
        let mut ls = bb.text.line_start(bb.text.line_of_pos(bb.point()));
        for (n, seg) in segs.iter().enumerate() {
            let ins_at = if n == 0 {
                bb.point()
            } else {
                // forward-line: next line start, creating the line at EOF.
                let le = bb.text.line_end(ls);
                ls = if le < bb.text.len() {
                    le + 1
                } else {
                    bb.insert_at(le, "\n");
                    le + 1
                };
                let le2 = bb.text.line_end(ls);
                rect_move_to(&mut bb, ls, le2, c0, tab, RectForce::T).0
            };
            bb.insert_at(ins_at, seg);
            bb.set_point(ins_at + seg.chars().count()); // lower-right corner
            if n == 0 {
                ls = bb.text.line_start(bb.text.line_of_pos(ins_at));
            }
        }
    }
    let p1 = cur(i).borrow().point();
    signal_after_change(i, p0 + 1, p1.max(p0) + 1, 0)?;
    Ok(Value::Nil)
}

fn f_yank_rectangle(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let sym = i.intern("killed-rectangle");
    let kr = i.symbol_value(sym);
    let segs: Vec<String> = kr
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => None,
        })
        .collect();
    rect_insert_segs(i, segs)
}

fn f_insert_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let segs: Vec<String> = a[0]
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Str(s) => Some(s.borrow().clone()),
            _ => None,
        })
        .collect();
    rect_insert_segs(i, segs)
}

/// Minimal %d / %Nd formatter for rectangle-number-lines.
fn rect_format(fmt: &str, n: i128) -> String {
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            let mut w = String::new();
            while let Some(&d) = it.peek() {
                if d.is_ascii_digit() {
                    w.push(d);
                    it.next();
                } else {
                    break;
                }
            }
            match it.next() {
                Some('d') => {
                    let s = n.to_string();
                    let width: usize = w.parse().unwrap_or(0);
                    if s.len() < width {
                        out.extend(std::iter::repeat(' ').take(width - s.len()));
                    }
                    out.push_str(&s);
                }
                Some('%') => out.push('%'),
                Some(other) => {
                    out.push('%');
                    out.push_str(&w);
                    out.push(other);
                }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn f_rectangle_number_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let start_at = match &a[2] {
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128 + 1,
        Value::Nil => 1,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let (sl, hi, c0, _c1, _tab) = rect_line_range(i, &a)?;
    let fmt = match a.get(3) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => {
            // GNU: "%Nd ", N = width of (count-lines start end) + start-at.
            // count-lines signals args-out-of-range on out-of-bounds pos.
            let nlines = match f_count_lines(i, vec![a[0].clone(), a[1].clone()].into())? {
                Value::Int(n) => n,
                _ => 0,
            };
            let w = (nlines + start_at).to_string().len();
            format!("%{}d ", w)
        }
    };
    let _ = (sl, hi);
    let mut n = start_at;
    rect_apply(i, &a, |bb, ls, le, c0, _c1, tab| {
        let (p0, _) = rect_move_to(bb, ls, le, c0, tab, RectForce::T);
        let s = rect_format(&fmt, n);
        n += 1;
        bb.insert_at(p0, &s);
        bb.set_point(p0 + s.len());
        Ok(())
    })?;
    let _ = (sl, hi, c0);
    Ok(Value::Nil)
}

fn f_spaces_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = match &a[0] {
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128 + 1,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    Ok(Value::string(" ".repeat(n.max(0) as usize)))
}

fn f_rectangle_dimensions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let sp = pos_idx(len, want_int(i, &a[0])?);
    let ep = pos_idx(len, want_int(i, &a[1])?);
    let (sl, el) = (bb.text.line_of_pos(sp), bb.text.line_of_pos(ep));
    let tab = rect_tab_width(i);
    let c0 = rect_col_at(&bb.text, bb.text.line_start(sl), sp, tab);
    let c1 = rect_col_at(&bb.text, bb.text.line_start(el), ep, tab);
    let width = (c1 - c0).abs();
    let height = (el as i128 - sl as i128).abs() + 1;
    Ok(Value::cons(Value::Int(width), Value::Int(height)))
}

fn f_rectangle_position_as_coordinates(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let p = pos_idx(len, want_int(i, &a[0])?);
    let line = bb.text.line_of_pos(p) as i128 + 1;
    let tab = rect_tab_width(i);
    let col = rect_col_at(&bb.text, bb.text.line_start(bb.text.line_of_pos(p)), p, tab);
    Ok(Value::cons(Value::Int(col), Value::Int(line)))
}

fn f_rectangle_intersect_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut get = |v: &Value, _what: &str| -> Result<(i128, i128), Flow> {
        match v {
            Value::Cons(c) => {
                let cb = c.borrow();
                match (cb.car.int(), cb.cdr.int()) {
                    (Some(x), Some(y)) => Ok((x, y)),
                    _ => Err(i.wrong_type_mut("number-or-marker-p", &cb.car)),
                }
            }
            other => Err(i.wrong_type_mut("listp", other)),
        }
    };
    let (x1, y1) = get(&a[0], "listp")?;
    let (w1, h1) = get(&a[1], "listp")?;
    let (x2, y2) = get(&a[2], "listp")?;
    let (w2, h2) = get(&a[3], "listp")?;
    Ok(Value::from_bool(
        !(x1 + w1 <= x2 || x2 + w2 <= x1 || y1 + h1 <= y2 || y2 + h2 <= y1),
    ))
}

fn f_extract_rectangle_bounds(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (sl, hi, c0, c1, tab) = rect_line_range(i, &a)?;
    let b = cur(i);
    let bb = b.borrow();
    let mut out = Vec::new();
    for ln in sl..=hi {
        let ls = bb.text.line_start(ln);
        let le = bb.text.line_end(ls);
        // move-to-column without force, like GNU's bounds extraction.
        let reach = |col: i128| -> usize {
            let mut c = 0i128;
            let mut k = ls;
            while k < le {
                let ch = bb.text.char_at(k);
                let w = if ch == '\t' {
                    (c / tab + 1) * tab - c
                } else {
                    rect_char_width(ch, tab)
                };
                if c + w > col {
                    return if c == col { k } else { k + 1 };
                }
                c += w;
                k += 1;
            }
            le
        };
        out.push(Value::cons(
            Value::Int(reach(c0) as i128 + 1),
            Value::Int(reach(c1) as i128 + 1),
        ));
    }
    Ok(Value::list(out))
}

/// GNU apply-on-rectangle: call FUNCTION (startcol endcol . ARGS) per
/// line with point at the line's beginning; return the final point.
fn f_apply_on_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let func = a[0].clone();
    let extra: Vec<Value> = a[3..].to_vec();
    let (sl, hi, c0, c1, _tab) = rect_line_range(i, &[a[1].clone(), a[2].clone()])?;
    let b = cur(i);
    for ln in sl..=hi {
        let ls = {
            let bb = b.borrow();
            bb.text.line_start(ln)
        };
        b.borrow_mut().set_point(ls);
        let mut argv = vec![Value::Int(c0), Value::Int(c1)];
        argv.extend(extra.iter().cloned());
        i.apply(&func, argv)?;
    }
    Ok(Value::Nil)
}

/// GNU operate-on-rectangle: call FUNCTION (startpos begextra endextra)
/// per line with point at the end of that line's segment.
fn f_operate_on_rectangle(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let func = a[0].clone();
    let coerce_tabs = a[3].truthy();
    let (sl, hi, c0, c1, tab) = rect_line_range(i, &[a[1].clone(), a[2].clone()])?;
    let b = cur(i);
    // GNU passes coerce-tabs straight to move-to-column: t pads + splits.
    let mode = if coerce_tabs {
        RectForce::T
    } else {
        RectForce::Nil
    };
    for ln in sl..=hi {
        let (ls, le) = {
            let bb = b.borrow();
            let ls = bb.text.line_start(ln);
            (ls, bb.text.line_end(ls))
        };
        let (p0, startpos, begextra, endextra) = {
            let mut bb = b.borrow_mut();
            let (p0, r0) = rect_move_to(&mut bb, ls, le, c0, tab, mode);
            let mut begextra = r0 - c0;
            let le2 = bb.text.line_end(ls);
            let (mut p1, mut r1) = rect_move_to(&mut bb, ls, le2, c1, tab, mode);
            if !coerce_tabs && r1 > c1 {
                // Overshot a wide char: step back so endextra is positive.
                p1 -= 1;
                r1 = rect_col_at(&bb.text, ls, p1, tab);
            }
            let mut endextra = c1 - r1;
            if begextra < 0 {
                endextra += begextra;
                begextra = 0;
            }
            bb.set_point(p1);
            (p0, p0 as i128 + 1, begextra, endextra)
        };
        let _ = p0;
        i.apply(
            &func,
            vec![
                Value::Int(startpos),
                Value::Int(begextra),
                Value::Int(endextra),
            ],
        )?;
    }
    Ok(Value::Nil)
}

/// GNU signals `(error "The mark is not set now, so there is no region")'
/// from region-beginning/region-end when the buffer has no mark.
fn no_region_err(i: &mut Interp) -> Flow {
    let e = i.intern("error");
    i.signal_data(
        e,
        vec![Value::string(
            "The mark is not set now, so there is no region",
        )],
    )
}

fn f_region_beginning(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark_position() {
        // GNU clips the raw mark position into the accessible region.
        Some(m) => Ok(Value::Int((m.min(bb.point()).max(bb.begv) + 1) as i128)),
        None => Err(no_region_err(i)),
    }
}

fn f_region_end(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark_position() {
        Some(m) => Ok(Value::Int((m.max(bb.point()).min(bb.zv) + 1) as i128)),
        None => Err(no_region_err(i)),
    }
}

fn f_region_active_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(region_active(i, &bb)))
}

fn f_deactivate_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let ran;
    {
        let mut bb = b.borrow_mut();
        let force = a.get(0).map(|v| v.truthy()).unwrap_or(false);
        ran = deactivate_mark(i, &mut bb, force);
    }
    // GNU simple.el: `deactivate-mark-hook' runs after the
    // deactivate work, outside the gate.
    if ran {
        call_hook(i, "deactivate-mark-hook")?;
    }
    Ok(Value::Nil)
}

fn f_activate_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let ran;
    {
        let mut bb = b.borrow_mut();
        let no_tmm = a.get(0).map(|v| v.truthy()).unwrap_or(false);
        ran = activate_mark(i, &mut bb, no_tmm);
    }
    if ran {
        call_hook(i, "activate-mark-hook")?;
    }
    Ok(Value::Nil)
}

fn f_exchange_point_and_mark(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // GNU's defun: (mark t) omark; (set-mark (point)); (goto-char omark);
    // then deactivate or activate per ARG and the region state.
    let Some(omark) = bb.mark_position() else {
        let ue = i.intern("user-error");
        return Err(i.signal_data(ue, vec![Value::string("No mark set in this buffer")]));
    };
    let was_active = region_active(i, &bb);
    let tmmv = buf_var(i, &bb, tmm_id(i));
    let temp_highlight = car_is_only(&tmmv, i);
    let p = bb.point();
    bb.set_mark(p);
    if !region_active(i, &bb) {
        bb.locals.insert(ma_id(i), Value::t());
    }
    bb.set_point(omark);
    if temp_highlight {
        return Ok(Value::Nil);
    }
    let hl = buf_var(
        i,
        &bb,
        i.intern_soft("exchange-point-and-mark-highlight-region")
            .unwrap_or(0),
    )
    .truthy();
    // (xor arg (if epamhr (not (region-active-p)) (not was-active)))
    let rhs = if hl {
        !region_active(i, &bb)
    } else {
        !was_active
    };
    let arg = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let (de, ac) = if arg != rhs {
        (deactivate_mark(i, &mut bb, false), false)
    } else {
        (false, activate_mark(i, &mut bb, false))
    };
    drop(bb);
    if de {
        call_hook(i, "deactivate-mark-hook")?;
    }
    if ac {
        call_hook(i, "activate-mark-hook")?;
    }
    Ok(Value::Nil)
}

fn f_use_region_p(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: (and (region-active-p) (or (/= beg end) (and
    // use-empty-active-region (not down-mouse-1) (not mouse-movement)))).
    if !region_active(i, &bb) {
        return Ok(Value::Nil);
    }
    // GNU calls (region-end)/(region-beginning) here, which signal when
    // mark-active is non-nil but the mark was never set.
    let Some(m) = bb.mark_position() else {
        return Err(no_region_err(i));
    };
    if m != bb.point() {
        return Ok(Value::t());
    }
    let uear = i.intern_soft("use-empty-active-region").unwrap_or(0);
    if !buf_var(i, &bb, uear).truthy() {
        return Ok(Value::Nil);
    }
    let lie = buf_var(i, &bb, i.intern_soft("last-input-event").unwrap_or(0));
    let dm1 = i.intern_soft("down-mouse-1").unwrap_or(0);
    let mm = i.intern_soft("mouse-movement").unwrap_or(0);
    let car = match &lie {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    };
    let ok =
        !matches!(&car, Value::Sym(s) if *s == dm1) && !matches!(&car, Value::Sym(s) if *s == mm);
    Ok(Value::from_bool(ok))
}

// ---------- narrowing ----------

fn f_narrow_to_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    // GNU Fnarrow_to_region uses CHECK_NUMBER_COERCE_MARKER —
    // integer-or-marker-p, live markers coerce to their position.
    let sl = want_int_or_mark(i, &a[0])?;
    let el = want_int_or_mark(i, &a[1])?;
    // GNU Fnarrow_to_region: BEG <= s <= e <= Z against the UNNARROWED
    // bounds (an earlier narrowing does not shrink the valid range),
    // args swap into order, `args-out-of-range' carries the originals.
    if !(1 <= sl.min(el) && sl.max(el) <= len as i128 + 1) {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    let (sl, el) = (sl.min(el), sl.max(el));
    let (s, e) = ((sl - 1) as usize, (el - 1) as usize);
    bb.begv = s;
    bb.zv = e;
    // GNU pulls an outlying point toward the nearer boundary:
    // below BEGV up to it, above ZV down to it.
    bb.point = bb.point.clamp(s, e);
    Ok(Value::Nil)
}

fn f_widen(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.begv = 0;
    bb.zv = bb.text.len();
    Ok(Value::Nil)
}

fn f_labeled_narrow_to_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (BEG END LABEL): narrow like narrow-to-region, recording the
    // previous bounds under LABEL for internal--labeled-widen.
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let sl = want_int(i, &a[0])?;
    let el = want_int(i, &a[1])?;
    // Same unnarrowed-bounds check as `narrow-to-region' (GNU
    // Finternal__labeled_narrow_to_region shares the BEG/Z test).
    if !(1 <= sl.min(el) && sl.max(el) <= len as i128 + 1) {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    let (sl, el) = (sl.min(el), sl.max(el));
    let (s, e) = ((sl - 1) as usize, (el - 1) as usize);
    let (pb, pz) = (bb.begv, bb.zv);
    bb.begv = s;
    bb.zv = e;
    // Same inward pull as `narrow-to-region'.
    bb.point = bb.point.clamp(s, e);
    bb.narrow_labels.push((pb, pz, a[2].clone()));
    Ok(Value::Nil)
}

fn f_labeled_widen(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Restore the bounds captured by the matching labeled narrow. A
    // non-matching label still widens (like `widen').
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if let Some(pos) = bb
        .narrow_labels
        .iter()
        .rposition(|(_, _, l)| crate::lisp::builtins::equal_values(i, l, &a[0]))
    {
        let (pb, pz, _) = bb.narrow_labels.remove(pos);
        bb.narrow_labels.truncate(pos);
        bb.begv = pb.min(bb.text.len());
        bb.zv = pz.min(bb.text.len());
        if bb.point < bb.begv || bb.point > bb.zv {
            bb.point = bb.begv;
        }
    } else {
        bb.begv = 0;
        bb.zv = bb.text.len();
    }
    Ok(Value::Nil)
}

fn f_set_buffer_modified_tick(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (TICK &optional BUFFER)
    // GNU's CHECK_FIXNUM rejects non-fixnums (`fixnump') but happily
    // stores negatives — `(buffer-modified-tick)' then reports -5.
    let tick = crate::lisp::builtins::want_fixnum(i, &a[0])? as i64;
    let b = buf_of(i, &crate::lisp::builtins::arg(&a, 1))?;
    b.borrow().text.mod_tick.set(tick as u64);
    Ok(Value::Nil)
}

// ---------- markers ----------
fn f_markerp(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Marker(_))))
}

fn f_make_marker(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(marker_value(Rc::new(RefCell::new(Marker {
        buffer: None,
        position: 0,
        insertion_type: false,
    }))))
}

fn f_copy_marker(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fcopy_marker: MARKER nil/omitted → a fresh marker with NO
    // buffer (`#<marker in no buffer>'; marker-position → nil).  It is
    // not the point marker — `(copy-marker nil)' is how Lisp makes a
    // detached marker for later `set-marker' use.
    if arg(&a, 0).is_nil() {
        let r = Rc::new(RefCell::new(Marker {
            buffer: None,
            position: 0,
            insertion_type: arg(&a, 1).truthy(),
        }));
        return Ok(marker_value(r));
    }
    match &a[0] {
        Value::Marker(m) => {
            let mm = m.borrow();
            let nm = Marker {
                buffer: mm.buffer,
                position: mm.position,
                // GNU Fcopy_marker: TYPE defaults to nil (advance), NOT
                // the source marker's insertion type.
                insertion_type: a.get(1).map(|v| v.truthy()).unwrap_or(false),
            };
            let r = Rc::new(RefCell::new(nm));
            if let Some(bid) = mm.buffer {
                if let Some(b) = i.buffers.get(bid) {
                    b.borrow_mut().register_marker(&r);
                }
            }
            Ok(marker_value(r))
        }
        // Emacs accepts an integer position (or nil → point).
        Value::Int(_) | Value::Nil => {
            let itype = a.get(1).map(|v| v.truthy()).unwrap_or(false);
            let bid = i.current_buffer;
            let len = i.buffers.get(bid).map(|b| b.borrow().size()).unwrap_or(0);
            let pos = match &a[0] {
                Value::Int(n) => pos_idx(len, *n),
                _ => i.buffers.get(bid).map(|b| b.borrow().point()).unwrap_or(0),
            };
            let r = Rc::new(RefCell::new(Marker {
                buffer: Some(bid),
                position: pos,
                insertion_type: itype,
            }));
            if let Some(b) = i.buffers.get(bid) {
                b.borrow_mut().register_marker(&r);
            }
            Ok(marker_value(r))
        }
        other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
}

fn f_set_marker(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_marker order: BUFFER arg decodes first (get_buffer →
    // wrong-type-argument bufferp), THEN CHECK_MARKER, then POSITION.
    // A dead buffer means "detach the marker" (unchain at the end).
    let mut detach = false;
    let buf_id = match a.get(2) {
        Some(v) if v.truthy() => match v {
            Value::Buffer(b) => {
                let (id, live) = {
                    let bb = b.borrow();
                    (bb.id, bb.live)
                };
                if !live {
                    detach = true;
                    usize::MAX
                } else {
                    id
                }
            }
            v => return Err(i.wrong_type_mut("bufferp", v)),
        },
        _ => match &a[1] {
            Value::Marker(mm) => mm.borrow().buffer.unwrap_or(i.current_buffer),
            _ => i.current_buffer,
        },
    };
    let m = match &a[0] {
        Value::Marker(m) => m.clone(),
        other => return Err(i.wrong_type_mut("markerp", other)),
    };
    if detach {
        m.borrow_mut().buffer = None;
        return Ok(a[0].clone());
    }
    // GNU unchain_marker cases: a detached marker as POSITION detaches
    // the target too (buffer/position both become nil).
    if let Value::Marker(mm) = &a[1] {
        if mm.borrow().buffer.is_none() {
            m.borrow_mut().buffer = None;
            return Ok(a[0].clone());
        }
    }
    let pos = match &a[1] {
        Value::Marker(mm) => mm.borrow().position,
        Value::Nil => {
            m.borrow_mut().buffer = None;
            return Ok(a[0].clone());
        }
        v => {
            let len = i
                .buffers
                .get(buf_id)
                .map(|b| b.borrow().text.len())
                .unwrap_or(0);
            pos_idx(len, want_int(i, v)?)
        }
    };
    {
        let mut mm = m.borrow_mut();
        mm.buffer = Some(buf_id);
        mm.position = pos;
    }
    if let Some(b) = i.buffers.get(buf_id) {
        b.borrow_mut().register_marker(&m);
    }
    Ok(a[0].clone())
}

fn f_marker_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => {
            let mm = m.borrow();
            if mm.buffer.is_none() {
                Ok(Value::Nil)
            } else {
                Ok(Value::Int(mm.position as i128 + 1))
            }
        }
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_marker_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => match m.borrow().buffer {
            Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
            None => Ok(Value::Nil),
        },
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_marker_insertion_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => Ok(Value::from_bool(m.borrow().insertion_type)),
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_set_marker_insertion_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => {
            m.borrow_mut().insertion_type = a[1].truthy();
            Ok(a[1].clone())
        }
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

// ---------- searching ----------

pub(crate) fn regexp_compile(
    i: &mut Interp,
    pattern: &Value,
) -> Result<crate::lisp::regexp::Regex, Flow> {
    let pat = match pattern {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))
}
fn f_looking_at_impl(i: &mut Interp, a: crate::lisp::Argv, posix: bool) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    let syn = crate::editor::re_syntax(i);
    let regs = {
        let b = cur(i);
        let bb = b.borrow_mut();
        let begv = bb.begv;
        let stop = bb.text_len();
        let pos = bb.point() - begv;
        let text = bb.text.as_slice();
        crate::lisp::regexp::looking_at(
            &re,
            &text[begv..stop],
            pos,
            &syn,
            &crate::editor::re_category(i),
            pos,
            posix,
        )
    };
    match regs {
        Some(regs) => {
            // GNU Flooking_at: non-nil INHIBIT leaves match data alone
            // (subr.el's `looking-at-p' passes t).
            let inhibit = a.get(1).map(|v| v.truthy()).unwrap_or(false);
            if !inhibit {
                i.match_data = Some(MatchData {
                    regs,
                    in_buffer: true,
                    base: cur(i).borrow().begv,
                    buffer: Some(i.current_buffer),
                });
            }
            Ok(Value::t())
        }
        None => Ok(Value::Nil),
    }
}

fn f_looking_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_looking_at_impl(i, a, false)
}

fn f_posix_looking_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_looking_at_impl(i, a, true)
}

/// GNU's `looking-at-p` does not change the match data.
fn f_looking_at_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let saved = i.match_data.clone();
    let r = f_looking_at_impl(i, a, false);
    i.match_data = saved;
    r
}

fn f_string_match_impl(i: &mut Interp, a: crate::lisp::Argv, posix: bool) -> EvalResult {
    let pat = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let text = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let chars: Vec<char> = text.chars().collect();
    // GNU: negative START counts back from the end of STRING
    // (search.c: `if (start < 0) start += SCHARS(string)`); out of
    // range signals args-out-of-range with the ORIGINAL value.
    let raw_start = match a.get(2) {
        None | Some(Value::Nil) => 0,
        // GNU `string-match' START is CHECK_FIXNUM — markers and floats
        // both signal `fixnump', not integer-or-marker-p.
        Some(Value::Int(n)) => *n,
        Some(v) => return Err(i.wrong_type_mut("fixnump", v)),
    };
    let start = if raw_start < 0 {
        raw_start + chars.len() as i128
    } else {
        raw_start
    };
    if start < 0 || start > chars.len() as i128 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![Value::string(text.clone()), Value::Int(raw_start)],
        ));
    }
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let syn = crate::editor::re_syntax(i);
    match crate::lisp::regexp::search_full(
        &re,
        &chars,
        start as usize,
        &syn,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
        posix,
    ) {
        Some(regs) => {
            // GNU Fstring_match: a non-nil INHIBIT-CHANGING-MATCH-DATA
            // leaves the match data untouched (subr.el's `string-match-p'
            // relies on this).
            let inhibit = a.get(3).map(|v| v.truthy()).unwrap_or(false);
            let start0 = regs[0].unwrap_or(0);
            if !inhibit {
                i.match_data = Some(MatchData {
                    regs,
                    in_buffer: false,
                    base: 0,
                    buffer: None,
                });
            }
            Ok(Value::Int(start0 as i128))
        }
        None => Ok(Value::Nil),
    }
}

fn f_string_match(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_string_match_impl(i, a, false)
}

fn f_posix_string_match(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_string_match_impl(i, a, true)
}

/// GNU's `string-match-p` does not change the match data.
fn f_string_match_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let saved = i.match_data.clone();
    let r = f_string_match_impl(i, a, false);
    i.match_data = saved;
    r
}

pub(crate) fn search_common(
    i: &mut Interp,
    a: &[Value],
    re: Option<&crate::lisp::regexp::Regex>,
    needle: Option<&str>,
    backward: bool,
    posix: bool,
) -> EvalResult {
    // GNU search_command order: CHECK_FIXNUM (count) comes BEFORE
    // CHECK_FIXNUM_COERCE_MARKER (bound).  A marker count is rejected
    // (fixnump); a detached marker bound signals "Marker does not point
    // anywhere"; a non-integer bound is integer-or-marker-p.
    let count = match a.get(3) {
        Some(Value::Nil) | None => 1,
        // GNU CHECK_FIXNUM: out-of-fixnum ints (≥2^61 or < -2^61)
        // are bignums to the type check.
        Some(v @ Value::Int(n)) if v.fixnump() => *n,
        Some(v) => return Err(i.wrong_type_mut("fixnump", v)),
    };
    let bound = match a.get(1) {
        Some(Value::Nil) | None => None,
        Some(Value::Int(n)) => Some(*n),
        Some(Value::Marker(m)) => {
            let mm = m.borrow();
            if mm.buffer.is_none() {
                return Err(i.error("Marker does not point anywhere"));
            }
            Some(mm.position as i128 + 1)
        }
        Some(v) => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
    };
    let noerror = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    // GNU: a negative COUNT reverses the search direction (and BOUND
    // then limits the flipped direction).
    let backward = backward != (count < 0);
    let (pos, begv) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.point() - bb.begv, bb.begv)
    };
    // GNU: COUNT = 0 performs no search and returns point.
    if count == 0 {
        return Ok(Value::Int((pos + begv + 1) as i128));
    }
    if let Some(lim) = bound {
        // GNU: bound on the wrong side of point signals an error.
        let pt = (pos + begv + 1) as i128;
        if (!backward && lim < pt) || (backward && lim > pt) {
            return Err(i.error("Invalid search bound (wrong side of point)"));
        }
    }
    let syn = crate::editor::re_syntax(i);
    // GNU Fsearch_forward: literal search folds through the buffer's
    // case table when `case-fold-search' is non-nil.
    let literal_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let found = {
        let b = cur(i);
        let bb = b.borrow_mut();
        let stop = bb.text_len();
        let text = &bb.text.as_slice()[begv..stop];
        let bound_idx = bound
            .map(|p| (p.max(1) as usize - 1).saturating_sub(begv))
            .unwrap_or(if backward { 0 } else { text.len() });
        let mut found: Option<crate::lisp::regexp::Regs> = None;
        let mut steps = count.abs().max(1);
        let mut cur_pos = pos;
        while steps > 0 {
            let hit = match (re, needle) {
                (Some(r), _) => {
                    if backward {
                        crate::lisp::regexp::search_backward_full(
                            r,
                            text,
                            cur_pos,
                            &syn,
                            &crate::editor::re_category(i),
                            pos,
                            posix,
                        )
                    } else {
                        crate::lisp::regexp::search_full(
                            r,
                            text,
                            cur_pos,
                            &syn,
                            &crate::editor::re_category(i),
                            pos,
                            posix,
                        )
                    }
                }
                (_, Some(n)) => literal_search(text, n, cur_pos, backward, literal_fold),
                _ => None,
            };
            match hit {
                Some(regs) => {
                    let s = regs[0].unwrap_or(0);
                    let e = regs[1].unwrap_or(0);
                    // bound check
                    if !backward && e > bound_idx {
                        found = None;
                        break;
                    }
                    if backward && s < bound_idx {
                        found = None;
                        break;
                    }
                    found = Some(regs);
                    cur_pos = if backward { s } else { e.max(s + 1) };
                    steps -= 1;
                    if backward {
                        if s == 0 {
                            break;
                        }
                    }
                }
                // GNU: COUNT is all-or-nothing — a miss on any
                // iteration fails the whole search (noerror moves to
                // the bound/eob, else search-failed).
                None => {
                    found = None;
                    break;
                }
            }
        }
        found
    };
    match found {
        Some(regs) => {
            let e = regs[1].unwrap_or(0);
            let s = regs[0].unwrap_or(0);
            let landing = if backward { s } else { e };
            cur(i).borrow_mut().set_point(landing + begv);
            // GNU: `inhibit-changing-match-data' suppresses the store;
            // the search itself still runs (leaving any previous
            // match data untouched).
            let inhibit = i
                .intern_soft("inhibit-changing-match-data")
                .map(|s| i.symbol_value(s).truthy())
                .unwrap_or(false);
            if !inhibit {
                i.match_data = Some(MatchData {
                    regs,
                    in_buffer: true,
                    base: begv,
                    buffer: Some(i.current_buffer),
                });
            }
            Ok(Value::Int(landing as i128 + begv as i128 + 1))
        }
        None => {
            if noerror {
                // GNU: noerror neither nil nor t moves point to the
                // search limit (BOUND, else eob/bob in search dir).
                let noerror_t = matches!(&a[2], Value::Sym(s) if i.sym_is(&Value::Sym(*s), sym::T));
                if !noerror_t {
                    let buf = cur(i);
                    let blen = buf.borrow().text_len();
                    let limit = match bound {
                        Some(b) => b.max(1) as usize - 1,
                        None if backward => 0,
                        // text_len() is already the absolute end index;
                        // `blen - begv' was a relative offset and left
                        // point short of point-max under narrowing
                        // (broke forward-paragraph inside erc's
                        // narrowed docstring temp buffers).
                        None => blen,
                    };
                    buf.borrow_mut().set_point(limit.min(blen));
                }
                Ok(Value::Nil)
            } else {
                Err(i.signal_data(sym::SEARCH_FAILED, vec![a[0].clone()]))
            }
        }
    }
}

/// Literal (non-regexp) search.  `case_fold` applies a simple
/// per-char lowercase fold (GNU folds through the buffer's case
/// table's canon; lowercase is equivalent for unicase chars).
pub(crate) fn literal_search(
    text: &[char],
    needle: &str,
    from: usize,
    backward: bool,
    case_fold: bool,
) -> Option<crate::lisp::regexp::Regs> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return Some(vec![Some(from.min(text.len())), Some(from.min(text.len()))]);
    }
    let at = |p: usize| -> bool {
        if case_fold {
            text[p..p + n.len()]
                .iter()
                .zip(n.iter())
                .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        } else {
            text[p..p + n.len()] == n[..]
        }
    };
    if backward {
        // GNU: the match must end at or before `from`.
        let mut p = match from.min(text.len()).checked_sub(n.len()) {
            Some(p) => p,
            None => return None,
        };
        loop {
            if at(p) {
                return Some(vec![Some(p), Some(p + n.len())]);
            }
            if p == 0 {
                return None;
            }
            p -= 1;
        }
    } else {
        let mut p = from;
        while p + n.len() <= text.len() {
            if at(p) {
                return Some(vec![Some(p), Some(p + n.len())]);
            }
            p += 1;
        }
        None
    }
}

fn f_re_search_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, false, false)
}

fn f_re_search_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, true, false)
}

fn f_posix_search_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, false, true)
}

fn f_posix_search_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, true, true)
}
/// GNU `word-search-regexp' (isearch.el): quote each word, join with
/// `\W+', bracket with `\<…\>' unless LAX (which drops the trailing
/// `\>'); leading/trailing whitespace in STRING becomes `\W+'.
fn word_search_regexp(i: &Interp, string: &str, lax: bool) -> String {
    if string.is_empty() {
        return String::new();
    }
    let is_word = |c: char| crate::editor::syntax_code_buf(i, c) == b'w';
    if string.chars().all(|c| !is_word(c)) {
        return "\\W+".to_string();
    }
    let mut out = String::new();
    let lead = string.chars().next().map(|c| !is_word(c)).unwrap_or(false);
    let trail = string.chars().last().map(|c| !is_word(c)).unwrap_or(false);
    out.push_str(if lead { "\\W+" } else { "\\<" });
    let quote = |w: &str| -> String {
        let mut q = String::with_capacity(w.len());
        for c in w.chars() {
            if ".*+?[^$\\".contains(c) {
                q.push('\\');
            }
            q.push(c);
        }
        q
    };
    let words: Vec<String> = string
        .split(|c: char| !is_word(c))
        .filter(|w| !w.is_empty())
        .map(|w| quote(w))
        .collect();
    out.push_str(&words.join("\\W+"));
    out.push_str(if trail {
        "\\W+"
    } else if lax {
        ""
    } else {
        "\\>"
    });
    out
}

fn word_search(i: &mut Interp, a: &[Value], backward: bool, lax: bool) -> EvalResult {
    let string = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let pat = word_search_regexp(i, &string, lax);
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    // GNU's Fword_search_forward calls search_command with the
    // GENERATED regexp, so `search-failed' carries that pattern
    // (e.g. "\\<foo\\W+bar\\>"), not the word string.
    let mut a2 = a.to_vec();
    a2[0] = Value::string(pat);
    search_common(i, &a2, Some(&re), None, backward, false)
}

fn f_word_search_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_search(i, &a, false, false)
}

fn f_word_search_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_search(i, &a, true, false)
}

fn f_word_search_forward_lax(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_search(i, &a, false, true)
}

fn f_word_search_backward_lax(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    word_search(i, &a, true, true)
}

fn f_search_forward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let needle = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    search_common(i, &a, None, Some(&needle), false, false)
}

fn f_search_backward(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let needle = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    search_common(i, &a, None, Some(&needle), true, false)
}

/// GNU `match_limit': negative SUBEXP signals `args-out-of-range'
/// with data (SUBEXP 0); too-large groups return nil.
fn match_reg_index(i: &mut Interp, v: &Value) -> Result<usize, Flow> {
    // GNU match_beginning uses CHECK_FIXNUM (fixnump, no markers).
    let n = crate::lisp::builtins::want_fixnum(i, v)?;
    if n < 0 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![Value::Int(n), Value::Int(0)],
        ));
    }
    Ok(n as usize)
}

fn f_match_beginning(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = match_reg_index(i, &a[0])?;
    match &i.match_data {
        Some(md) => match md.regs.get(2 * n).copied().flatten() {
            Some(p) => Ok(Value::Int(
                (p + md.base + if md.in_buffer { 1 } else { 0 }) as i128,
            )),
            None => Ok(Value::Nil),
        },
        None => Ok(Value::Nil),
    }
}

fn f_match_end(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = match_reg_index(i, &a[0])?;
    match &i.match_data {
        Some(md) => match md.regs.get(2 * n + 1).copied().flatten() {
            Some(p) => Ok(Value::Int(
                (p + md.base + if md.in_buffer { 1 } else { 0 }) as i128,
            )),
            None => Ok(Value::Nil),
        },
        None => Ok(Value::Nil),
    }
}

fn f_match_data(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let integers = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let _reuse = a.get(1);
    let _reseat = a.get(2);
    match &i.match_data {
        Some(md) => {
            // GNU: INTEGERS nil + a buffer match → real markers in the
            // searched buffer; INTEGERS or a string match → positions.
            let use_markers = !integers && md.in_buffer && md.buffer.is_some();
            let mut items = Vec::new();
            // GNU drops trailing pairs where both registers are nil
            // (a group that never participated); interior holes stay.
            let mut npairs = md.regs.len() / 2;
            while npairs > 0
                && md.regs.get(2 * npairs - 2).copied().flatten().is_none()
                && md.regs.get(2 * npairs - 1).copied().flatten().is_none()
            {
                npairs -= 1;
            }
            for k in 0..npairs {
                let s = md.regs.get(2 * k).copied().flatten();
                let e = md.regs.get(2 * k + 1).copied().flatten();
                let off = md.base + if md.in_buffer { 1 } else { 0 };
                for p in [s, e] {
                    let v = match p {
                        Some(p) if use_markers => {
                            let bid = md.buffer.unwrap();
                            let m = Rc::new(RefCell::new(Marker {
                                buffer: Some(bid),
                                position: p + off - 1,
                                insertion_type: false,
                            }));
                            if let Some(b) = i.buffers.get(bid) {
                                b.borrow_mut().register_marker(&m);
                            }
                            marker_value(m)
                        }
                        Some(p) => Value::Int((p + off) as i128),
                        None => Value::Nil,
                    };
                    items.push(v);
                }
            }
            // GNU search.c `match_data': with INTEGERS non-nil and a
            // buffer match, the searched buffer trails the positions.
            if integers && md.in_buffer {
                if let Some(bid) = md.buffer {
                    if let Some(v) = i.buffer_value(bid) {
                        items.push(v);
                    }
                }
            }
            Ok(Value::list(items))
        }
        None => Ok(Value::Nil),
    }
}

fn f_set_match_data(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU search.c Fset_match_data:
    // LIST arg is walked pairwise; a live marker or buffer in a START
    // (even-index) slot makes this buffer match data (`last_thing_searched'
    // = that buffer — end-slot markers don't), detached markers coerce to
    // 0, nil start kills the pair, RESEAT unchains the list's markers.
    let items = match &a[0] {
        Value::Cons(_) => a[0].list_to_vec().unwrap_or_default(),
        Value::Nil => Vec::new(),
        other => return Err(i.wrong_type_mut("listp", other)),
    };
    let reseat = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let mut last_thing: Option<usize> = None; // buffer id; None = string mode
    let mut regs: Vec<Option<usize>> = Vec::new();
    let mut k = 0;
    while k < items.len() {
        let start_el = &items[k];
        // A buffer object in a start slot ends the walk and names the buffer.
        if let Value::Buffer(bv) = start_el {
            last_thing = Some(bv.borrow().id);
            break;
        }
        let coerce = |v: &Value,
                      i: &mut Interp,
                      last: &mut Option<usize>,
                      is_start: bool|
         -> Result<Option<usize>, Flow> {
            match v {
                Value::Nil => Ok(None),
                Value::Int(n) if (-(1i128 << 61)..=((1i128 << 61) - 1)).contains(n) => {
                    // Stored raw (0-based internal) — buffer matches keep
                    // the 1-based→0-based shift applied at emit time.
                    Ok(Some((*n).max(0) as usize))
                }
                Value::Marker(m) => {
                    let mm = m.borrow();
                    match mm.buffer {
                        None => Ok(Some(0)), // detached marker → 0
                        Some(bid) => {
                            if is_start {
                                *last = Some(bid);
                            }
                            Ok(Some(mm.position))
                        }
                    }
                }
                other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
            }
        };
        let s = coerce(start_el, i, &mut last_thing, true)?;
        if reseat {
            if let Value::Marker(m) = start_el {
                m.borrow_mut().buffer = None;
            }
        }
        if s.is_none() {
            // nil start: whole pair invalid; consume end without storing.
            regs.push(None);
            regs.push(None);
            k += 2;
            continue;
        }
        let e = match items.get(k + 1) {
            Some(v) => coerce(v, i, &mut last_thing, false)?,
            None => None,
        };
        if reseat {
            if let Some(Value::Marker(m)) = items.get(k + 1) {
                m.borrow_mut().buffer = None;
            }
        }
        regs.push(s);
        regs.push(e);
        k += 2;
    }
    let in_buffer = last_thing.is_some();
    i.match_data = Some(MatchData {
        regs,
        in_buffer,
        base: 0,
        buffer: last_thing,
    });
    Ok(Value::Nil)
}

fn f_match_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let n = match_reg_index(i, &a[0])?;
    let md = match &i.match_data {
        Some(m) => m,
        None => return Ok(Value::Nil),
    };
    let (s, e) = match (
        md.regs.get(2 * n).copied().flatten(),
        md.regs.get(2 * n + 1).copied().flatten(),
    ) {
        (Some(s), Some(e)) => (s, e),
        _ => return Ok(Value::Nil),
    };
    if md.in_buffer {
        let b = cur(i);
        let bb = b.borrow();
        let start = md.base + s;
        let end = (md.base + e).min(bb.text.len());
        Ok(Value::string(bb.text.substring(start, end)))
    } else {
        match a.get(1) {
            Some(Value::Str(text)) => {
                let chars: Vec<char> = text.borrow().chars().collect();
                Ok(Value::string(
                    chars[s.min(chars.len())..e.min(chars.len())]
                        .iter()
                        .collect::<String>(),
                ))
            }
            // GNU requires STRING when the match data came from a
            // string match; otherwise it indexes the current buffer
            // and signals args-out-of-range (buffer S E) — S/E are
            // the raw register positions.
            _ => {
                let b = cur(i);
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![
                        Value::Buffer(b),
                        Value::Int(s as i128),
                        Value::Int(e as i128),
                    ],
                ));
            }
        }
    }
}

/// Expand `\&`, `\N`, `\\` escapes in a replacement string using `regs`
/// indexed into `src` (a char-indexed source text).
fn expand_replacement(
    i: &mut Interp,
    rep: &str,
    literal: bool,
    md: &MatchData,
    src: &[char],
) -> Result<String, Flow> {
    if literal {
        return Ok(rep.to_string());
    }
    let mut out = String::new();
    let mut it = rep.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('\\') => out.push('\\'),
            Some('&') => {
                out.push_str(&group_text(i, md, src, 0)?);
            }
            Some(d @ '1'..='9') => {
                let n = d as usize - '0' as usize;
                // GNU substitutes an empty string for a group that did
                // not match or does not exist.
                if let Ok(t) = group_text(i, md, src, n) {
                    out.push_str(&t);
                }
            }
            // GNU: `\?' is treated literally — both characters are
            // emitted (query-replace-regexp compat, search.c:2564).
            Some('?') => out.push_str("\\?"),
            // GNU errors "Invalid use of `\\' in replacement text" for a
            // trailing backslash or any other escaped char.
            _ => {
                return Err(err_sym(
                    i,
                    "error",
                    vec![Value::string("Invalid use of `\\' in replacement text")],
                ));
            }
        }
    }
    Ok(out)
}

fn group_text(i: &mut Interp, md: &MatchData, src: &[char], n: usize) -> Result<String, Flow> {
    match (
        md.regs.get(2 * n).copied().flatten(),
        md.regs.get(2 * n + 1).copied().flatten(),
    ) {
        (Some(s), Some(e)) => Ok(src[s.min(src.len())..e.min(src.len())].iter().collect()),
        _ => Err(err_sym(i, "args-out-of-range", vec![Value::Int(n as i128)])),
    }
}

fn f_replace_match(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Freplace_match:
    // CHECK_STRING(NEWTEXT), optional STRING is CHECK_STRING too, and
    // SUBEXP goes through check_integer_range(0, num_regs-1) — integerp
    // on non-numbers, args-out-of-range on int overflow.
    let newtext = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let fixedcase = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let literal = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let str_arg = match a.get(3) {
        None | Some(Value::Nil) => None,
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    };
    let md = match &i.match_data {
        Some(m) => m.clone(),
        None => {
            return Err(err_sym(
                i,
                "error",
                vec![Value::string(
                    "`replace-match' called before any match found",
                )],
            ));
        }
    };
    // GNU search_regs.num_regs is the fixed 30-entry register array,
    // not the group's pair count — subexp < 30 is always in-range and
    // hits the `does not exist' path when its group didn't match.
    let num_regs = 30i128;
    let subexp = match a.get(4) {
        None | Some(Value::Nil) => 0usize,
        Some(Value::Int(n)) if *n >= 0 && *n < num_regs => *n as usize,
        Some(v @ Value::Int(_)) => {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![v.clone(), Value::Int(0), Value::Int(num_regs - 1)],
            ));
        }
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    // STRING path: substitute within the given string's match registers.
    if let Some(text) = str_arg {
        let src: Vec<char> = text.chars().collect();
        // GNU applyes case transference (`case_action') in the string
        // path too: FIXEDCASE skips it, and the casing is taken from
        // the replaced group's text.
        let (s, e) = match (
            md.regs.get(2 * subexp).copied().flatten(),
            md.regs.get(2 * subexp + 1).copied().flatten(),
        ) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                return Err(err_sym(
                    i,
                    "error",
                    vec![
                        Value::string("replace-match subexpression does not exist"),
                        Value::Int(subexp as i128),
                    ],
                ));
            }
        };
        // GNU: registers outside the string's bounds → args-out-of-range.
        if e > src.len() {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![Value::Int(s as i128), Value::Int(e as i128)],
            ));
        }
        let mut rep = expand_replacement(i, &newtext, literal, &md, &src)?;
        if !fixedcase {
            let old: String = src[s.min(src.len())..e.min(src.len())].iter().collect();
            rep = match_case(&old, &rep);
        }
        let mut out: String = src[..s.min(src.len())].iter().collect();
        out.push_str(&rep);
        out.extend(src[e.min(src.len())..].iter());
        return Ok(Value::string(out));
    }
    // GNU Freplace_match (search.c): with nil STRING the registers are
    // always checked against the CURRENT buffer's BEGV..ZV — even when the
    // last match ran on a string (its 0-based indices then read as
    // BEGV-relative and usually fail the check).  A negative register
    // (group never matched) is the "does not exist" error; any other
    // out-of-window pair is args-out-of-range.
    let bb_borrow = cur(i);
    // Internal begv/zv are 0-based; registers converted to Emacs
    // positions (match-beginning numbers) are 1-based.
    let (begv, zv) = {
        let b = bb_borrow.borrow();
        (b.begv as i128 + 1, (b.text.len().min(b.zv)) as i128 + 1)
    };
    let (s_raw, e_raw) = match (
        md.regs.get(2 * subexp).copied().flatten(),
        md.regs.get(2 * subexp + 1).copied().flatten(),
    ) {
        (Some(s), Some(e)) => (
            s as i128 + md.base as i128 + if md.in_buffer { 1 } else { 0 },
            e as i128 + md.base as i128 + if md.in_buffer { 1 } else { 0 },
        ),
        _ => (-1, -1),
    };
    if s_raw < 0 {
        return Err(err_sym(
            i,
            "error",
            vec![
                Value::string("replace-match subexpression does not exist"),
                Value::Int(subexp as i128),
            ],
        ));
    }
    if s_raw < begv || e_raw > zv {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![Value::Int(s_raw), Value::Int(e_raw)],
        ));
    }
    // Registers as Emacs char positions → 0-based text offsets.
    let (start, end) = ((s_raw - 1) as usize, (e_raw - 1) as usize);
    // Buffer path: build replacement against buffer text.
    let b = cur(i);
    let bb = b.borrow_mut();
    let old = bb.text.substring(start, end.min(bb.text.len()));
    let src: Vec<char> = bb.text.substring(bb.begv, bb.text.len()).chars().collect();
    let mut rep = expand_replacement(i, &newtext, literal, &md, &src)?;
    if !fixedcase {
        rep = match_case(&old, &rep);
    }
    let tlen = bb.text.len();
    let rep_len = rep.chars().count();
    drop(bb);
    // GNU `replace_range': a single change pair, not delete+insert.
    chg_replace(i, start, end.min(tlen), &rep)?;
    cur(i).borrow_mut().set_point(start + rep_len);
    Ok(Value::Nil)
}

fn f_match_substitute_replacement(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let newtext = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let fixedcase = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let literal = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let src: Vec<char> = match a.get(3) {
        Some(Value::Str(s)) => s.borrow().chars().collect(),
        _ => {
            let b = cur(i);
            let bb = b.borrow();
            bb.text.substring(bb.begv, bb.text.len()).chars().collect()
        }
    };
    let md = match &i.match_data {
        // Without a STRING arg the match must have been against a
        // buffer; a string-match's data is args-out-of-range here.
        Some(m) if m.in_buffer || a.get(3).is_some() => m.clone(),
        _ => {
            // GNU reports the current match-0 bounds against the
            // buffer — string-match data counts too; no match data
            // at all gives (0 0).
            let (s0, e0) = i
                .match_data
                .as_ref()
                .and_then(|m| match (m.regs.first(), m.regs.get(1)) {
                    (Some(Some(s)), Some(Some(e))) => Some((*s, *e)),
                    _ => None,
                })
                .unwrap_or((0, 0));
            let b = cur(i);
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![
                    Value::Buffer(b),
                    Value::Int(s0 as i128),
                    Value::Int(e0 as i128),
                ],
            ));
        }
    };
    let mut rep = expand_replacement(i, &newtext, literal, &md, &src)?;
    // GNU `match_substitute_replacement' applies `case_action' too:
    // FIXEDCASE suppresses transfer from the matched text's case.
    if !fixedcase {
        if let (Some(s), Some(e)) = (
            md.regs.first().copied().flatten(),
            md.regs.get(1).copied().flatten(),
        ) {
            let old: String = src[s.min(src.len())..e.min(src.len())].iter().collect();
            rep = match_case(&old, &rep);
        }
    }
    Ok(Value::string(rep))
}

/// Case-matching for `replace-match`: if OLD is all-caps/capitalized,
/// apply the same to NEW.
pub(crate) fn match_case(old: &str, new: &str) -> String {
    let letters: Vec<char> = old.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return new.to_string();
    }
    let all_upper = letters.iter().all(|c| c.is_uppercase());
    let first_upper = letters[0].is_uppercase() && letters[1..].iter().all(|c| c.is_lowercase());
    if all_upper && letters.len() > 1 {
        return new.to_uppercase();
    }
    if first_upper {
        let mut cs = new.chars();
        match cs.next() {
            Some(c) => return c.to_uppercase().collect::<String>() + cs.as_str(),
            None => return new.to_string(),
        }
    }
    new.to_string()
}

fn f_regexp_quote(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        // Emacs does not escape ']' (an unmatched ] is already literal).
        if ".*+?[^$\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    Ok(Value::string(out))
}

// ---------- regexp-opt ----------

/// Trie node for `regexp-opt`.
#[derive(Default)]
struct OptTrie {
    terminal: bool,
    children: Vec<(char, OptTrie)>,
}

impl OptTrie {
    fn insert(&mut self, s: &[char]) {
        if s.is_empty() {
            self.terminal = true;
            return;
        }
        let c = s[0];
        let idx = match self.children.iter().position(|(k, _)| *k == c) {
            Some(p) => p,
            None => {
                self.children.push((c, OptTrie::default()));
                self.children.sort_by_key(|(k, _)| *k);
                self.children.len() - 1
            }
        };
        self.children[idx].1.insert(&s[1..]);
    }

    /// All children are single-char terminal leaves → emit `[chars]`.
    fn all_char_leaves(&self) -> bool {
        self.children.len() > 1
            && self
                .children
                .iter()
                .all(|(_, t)| t.terminal && t.children.is_empty())
    }

    fn emit(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (c, t) in &self.children {
            let mut p = String::new();
            push_regexp_quoted(*c, &mut p);
            p.push_str(&t.emit());
            parts.push(p);
        }
        if parts.is_empty() {
            return String::new();
        }
        if self.all_char_leaves() {
            let mut s = String::from("[");
            for (c, _) in &self.children {
                // Emacs sorts charset contents; escape class specials.
                match c {
                    ']' | '\\' | '^' | '-' => {
                        s.push('\\');
                        s.push(*c);
                    }
                    _ => s.push(*c),
                }
            }
            s.push(']');
            return s;
        }
        let inner = if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            format!("\\(?:{}\\)", parts.join("\\|"))
        };
        if self.terminal {
            // This node also ends a string → the whole remainder is optional.
            if parts_single_char(&inner) || inner.starts_with("\\(?") {
                format!("{}?", inner)
            } else {
                format!("\\(?:{}\\)?", inner)
            }
        } else {
            inner
        }
    }
}

fn parts_single_char(s: &str) -> bool {
    s.chars().count() == 1 || (s.len() == 2 && s.starts_with('\\'))
}

fn push_regexp_quoted(c: char, out: &mut String) {
    if ".*+?[^$\\".contains(c) {
        out.push('\\');
    }
    out.push(c);
}

fn f_regexp_opt(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let strings: Vec<String> = match a[0].list_to_vec() {
        Ok(items) => {
            let mut v = Vec::new();
            for it in items {
                match &it {
                    Value::Str(s) => v.push(s.borrow().clone()),
                    other => return Err(i.wrong_type_mut("stringp", other)),
                }
            }
            v
        }
        Err(_) => Vec::new(),
    };
    let paren = &a.get(1).cloned().unwrap_or(Value::Nil);
    if strings.is_empty() {
        // Emacs quirk: (regexp-opt nil) => "\\(?:\\`a\\`\\)".
        return Ok(Value::string("\\(?:\\`a\\`\\)"));
    }
    let mut sorted = strings.clone();
    sorted.sort();
    sorted.dedup();
    let mut trie = OptTrie::default();
    for s in &sorted {
        trie.insert(&s.chars().collect::<Vec<_>>());
    }
    // Emit the root without group wrapping so top-level alternatives join
    // with \| under the requested parens.
    let body = if trie.all_char_leaves() {
        trie.emit()
    } else {
        let mut parts: Vec<String> = Vec::new();
        for (c, t) in &trie.children {
            let mut p = String::new();
            push_regexp_quoted(*c, &mut p);
            p.push_str(&t.emit());
            parts.push(p);
        }
        let inner = parts.join("\\|");
        if trie.terminal {
            format!("\\(?:{}\\)?", inner)
        } else {
            inner
        }
    };
    let (open, close) = if paren.truthy() {
        ("\\(", "\\)")
    } else {
        ("\\(?:", "\\)")
    };
    // Wrap when the body has top-level alternation or an inner group, or
    // when parens were explicitly requested.
    let need_wrap = paren.truthy() || body.contains("\\|") || body.contains("\\(?");
    let out = if need_wrap && !body.starts_with("\\(?") {
        format!("{}{}{}", open, body, close)
    } else if need_wrap && paren.truthy() {
        format!("{}{}{}", open, body, close)
    } else {
        body
    };
    Ok(Value::string(out))
}

fn f_regexp_opt_depth(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pat = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let chars: Vec<char> = pat.chars().collect();
    let mut depth = 0i128;
    let mut k = 0;
    while k + 1 < chars.len() {
        if chars[k] == '\\' && chars[k + 1] == '(' {
            let is_shy = k + 2 < chars.len() && chars[k + 2] == '?';
            if !is_shy {
                depth += 1;
            }
        }
        k += if chars[k] == '\\' { 2 } else { 1 };
    }
    Ok(Value::Int(depth))
}

// ---------- text properties ----------

pub(crate) fn f_put_text_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(Value::Str(s)) = a.get(4) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        // GNU validate_interval_range signals (START END) on the
        // raw int args, not (pos object).
        if st.max(en) > len {
            return Err(i.signal_data(
                crate::lisp::obarray::sym::ARGS_OUT_OF_RANGE,
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        let (s0, e0) = (st.min(en), st.max(en));
        let prop = a[2].clone();
        let val = a[3].clone();
        let fill = if val.is_nil() {
            None
        } else {
            Some(vec![prop.clone(), val.clone()])
        };
        let mut ivs = std::mem::take(i.str_props_mut(&s));
        let ii: &Interp = i;
        iv_apply(
            &mut ivs,
            s0,
            e0,
            |pl| {
                if val.is_nil() {
                    if let Some(id) = ii.sym_id(&prop) {
                        str_plist_remove(pl, id, ii);
                    }
                } else {
                    str_plist_put(pl, &prop, &val);
                }
            },
            fill,
        );
        i.set_str_props(&s, ivs);
        return Ok(Value::Nil);
    }
    // OBJECT may be nil, a buffer, or a string — GNU gates this via
    // CHECK_STRING_OR_BUFFER, so windows and other objects are
    // `buffer-or-string-p' errors (not `stringp').
    match a.get(4) {
        None | Some(Value::Nil) | Some(Value::Buffer(_)) | Some(Value::Str(_)) => {}
        Some(v) => return Err(i.wrong_type_mut("buffer-or-string-p", v)),
    }
    let b = match a.get(4) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    // GNU `validate_region' bounds-checks against the accessible
    // portion: out-of-[BEGV,ZV] args signal args-out-of-range with the
    // ORIGINAL argument values.
    let (s0i, e0i) = (want_int(i, &a[0])?, want_int(i, &a[1])?);
    {
        let bb = b.borrow();
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if s0i.min(e0i) < lo || s0i.max(e0i) > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
    }
    let len = b.borrow().text.len();
    // GNU `validate_region' orders the region: callers may pass
    // (END . BEG) — swap so s <= e, else `e - s' underflows.
    let (s, e) = {
        let (s, e) = (pos_idx(len, s0i), pos_idx(len, e0i));
        (s.min(e), s.max(e))
    };
    let prop = want_sym(i, &a[2])?;
    if s == e {
        // GNU set_text_properties returns early on an empty range:
        // no interval, no modification hooks.
        return Ok(Value::Nil);
    }
    // GNU `set_text_properties_1': `modify_text_properties' (before-
    // change + MODIFF bump) and `signal_after_change' both fire
    // unconditionally on the buffer path — with OBJECT made current
    // (GNU `set_buffer_internal') so hooks and the read-only checks
    // see the right buffer.
    let bid = b.borrow().id;
    let prop_v = a[2].clone();
    let val = a[3].clone();
    // GNU add_text_properties_1: when every interval in the range
    // already has the pair with an `eq' value, modify_text_properties
    // (and thus the read-only checks) never runs — even in a
    // buffer-read-only buffer the call is a silent no-op.
    {
        let bb = b.borrow();
        if !buf_needs_prop_change(i, &bb, s, e, &[(prop_v.clone(), val.clone())]) {
            return Ok(Value::Nil);
        }
    }
    // GNU validate_interval_range(hard) creates the interval tree
    // BEFORE verify — defaults/aliases gate prop writes too.
    b.borrow().text.had_intervals.set(true);
    chg_with_buffer(i, bid, |i| {
        check_writable(i)?;
        text_ro_range(i, s, e)?;
        signal_before_change(i, s + 1, e + 1)?;
        {
            let mut bb = b.borrow_mut();
            buf_record_prop(&mut bb, s, e, &prop_v, prop, &val);
            push_prop(
                &mut bb.text.props_write(),
                TextProp {
                    start: s,
                    end: e,
                    prop,
                    value: val,
                },
            );
            bb.note_prop_modified();
        }
        signal_after_change(i, s + 1, e + 1, (e - s) as i128)
    })?;
    Ok(Value::Nil)
}

/// GNU `record_property_change' over [S,E) (0-based): push one
/// `(nil PROP OLD BEG . END)' undo entry per contiguous run of OLD
/// values that are not `eq' NEW.
fn buf_record_prop(bb: &mut Buffer, s: usize, e: usize, prop: &Value, pid: u32, new: &Value) {
    // GNU `record_property_change' runs per contiguous run of equal OLD
    // values: walk the effective runs once (a per-position lookup was
    // quadratic in buffer size).
    let runs = prop_runs(&bb.text.props.borrow(), s, e, pid);
    // Emit in position order; untouched stretches contribute nothing.
    let mut p = s;
    for (rs, re, rv) in runs {
        if rs > p {
            // gap: OLD is nil — record only when NEW differs.
            if !crate::lisp::builtins::eq_values(&Value::Nil, new) {
                bb.record_prop_change(prop.clone(), Value::Nil, p + 1, rs.min(e) + 1);
            }
        }
        if !crate::lisp::builtins::eq_values(&rv, new) {
            bb.record_prop_change(prop.clone(), rv, rs + 1, re.min(e) + 1);
        }
        p = re.max(p);
    }
    if p < e && !crate::lisp::builtins::eq_values(&Value::Nil, new) {
        bb.record_prop_change(prop.clone(), Value::Nil, p + 1, e + 1);
    }
}

// ---------- string-object text properties ----------
//
// GNU keeps real split intervals: put/add/remove cut boundaries at
// the touched range, and dead boundaries stay after removal.  String
// props live in Interp.string_props keyed by the Str's identity.

type StrIvs = Vec<(usize, usize, Vec<Value>)>;

/// plist-get over a flat (k v ...) vec; `id` is the SymId to find.
fn str_plist_get(pl: &[Value], id: u32, i: &Interp) -> Option<Value> {
    let mut k = 0;
    while k + 1 < pl.len() {
        if i.sym_id(&pl[k]) == Some(id) {
            return Some(pl[k + 1].clone());
        }
        k += 2;
    }
    None
}

/// GNU `lookup_char_property' fallback: when PROP isn't in the plist
/// directly, look it up on the `category' symbol's plist.
fn str_plist_get_cat(pl: &[Value], id: u32, i: &Interp) -> Option<Value> {
    if let Some(v) = str_plist_get(pl, id, i) {
        return Some(v);
    }
    if let Some(cat) = i.intern_soft("category") {
        if let Some(Value::Sym(c)) = str_plist_get(pl, cat, i) {
            match i.get_prop(c, id) {
                Value::Nil => {}
                v => return Some(v),
            }
        }
    }
    // GNU lookup_char_property: char-property-alias-alist aliases are
    // consulted via raw plist_get (strings pass textprop=0, so
    // default-text-properties is NOT checked here).
    let alist = i.symbol_value(i.intern_soft("char-property-alias-alist").map(|s| s).unwrap_or(u32::MAX));
    if let Some(cell) = alist
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .find(|e| matches!(e, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(id)))
    {
        if let Value::Cons(c) = cell {
            for a in c.borrow().cdr.clone().list_to_vec().unwrap_or_default() {
                if let Some(aid) = i.sym_id(&a) {
                    if let Some(v) = str_plist_get(pl, aid, i) {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

/// GNU plput: update in place when the prop exists, else prepend.
fn str_plist_put(pl: &mut Vec<Value>, sym: &Value, val: &Value) {
    let mut k = 0;
    while k + 1 < pl.len() {
        if crate::lisp::builtins::eq_values(&pl[k], sym) {
            pl[k + 1] = val.clone();
            return;
        }
        k += 2;
    }
    let mut nl = vec![sym.clone(), val.clone()];
    nl.extend(pl.iter().cloned());
    *pl = nl;
}

/// plist minus the named prop (by SymId).
fn str_plist_remove(pl: &mut Vec<Value>, id: u32, i: &Interp) {
    let mut k = 0;
    while k + 1 < pl.len() {
        if i.sym_id(&pl[k]) == Some(id) {
            pl.drain(k..k + 2);
        } else {
            k += 2;
        }
    }
}

/// The plist at char position P (0-based) — the interval containing P.
pub(crate) fn str_plist_at(ivs: &[(usize, usize, Vec<Value>)], p: usize) -> Vec<Value> {
    for (s, e, pl) in ivs {
        if p >= *s && p < *e {
            return pl.clone();
        }
    }
    Vec::new()
}

/// Cut interval boundaries at S and E so every interval lies wholly
/// inside or outside [S,E).
fn iv_cut(ivs: &mut StrIvs, s: usize, e: usize) {
    for p in [s, e] {
        let mut k = 0;
        while k < ivs.len() {
            if ivs[k].0 < p && p < ivs[k].1 {
                let tail = (p, ivs[k].1, ivs[k].2.clone());
                ivs[k].1 = p;
                ivs.insert(k + 1, tail);
                break;
            }
            k += 1;
        }
    }
}

/// True when [S,E) contains any point not covered by an interval —
/// i.e. where a `fill' plist in `iv_apply' would create a new one.
fn iv_gap_in(ivs: &StrIvs, s: usize, e: usize) -> bool {
    let mut p = s;
    for iv in ivs {
        if iv.0 > p && p < e {
            return true;
        }
        p = p.max(iv.1);
        if p >= e {
            return false;
        }
    }
    p < e
}

/// Apply `f` to the plists of every interval inside [S,E); `fill`
/// creates an interval on uncovered gaps.  Intervals whose plist
/// becomes empty are kept — GNU preserves dead interval boundaries
/// after property removal.
fn iv_apply(
    ivs: &mut StrIvs,
    s: usize,
    e: usize,
    mut f: impl FnMut(&mut Vec<Value>),
    fill: Option<Vec<Value>>,
) {
    iv_cut(ivs, s, e);
    let mut covered: Vec<(usize, usize)> = Vec::new();
    for iv in ivs.iter_mut() {
        if iv.0 >= s && iv.1 <= e {
            f(&mut iv.2);
            covered.push((iv.0, iv.1));
        }
    }
    if let Some(pl0) = fill {
        if !pl0.is_empty() {
            covered.sort();
            let mut p = s;
            for (a, b) in covered {
                if p < a {
                    ivs.push((p, a, pl0.clone()));
                }
                p = p.max(b);
            }
            if p < e {
                ivs.push((p, e, pl0));
            }
            ivs.sort_by_key(|iv| iv.0);
        }
    }
}

/// Char length of a string object.
fn str_len(s: &std::rc::Rc<std::cell::RefCell<String>>) -> usize {
    s.borrow().chars().count()
}

/// Check a 0-based string position index is inside [0,len].
fn str_pos_ok(i: &Interp, obj: &Value, p: usize, len: usize) -> Result<(), Flow> {
    if p > len {
        Err(i.signal_data(
            crate::lisp::obarray::sym::ARGS_OUT_OF_RANGE,
            vec![Value::Int(p as i128), obj.clone()],
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn f_add_text_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `add_text_properties_1': a nil PROPERTIES short-circuits
    // before ANY argument validation.
    if a[2].is_nil() {
        return Ok(Value::Nil);
    }
    // GNU Fadd_text_properties counts PAIRS by cons cells: a dotted
    // tail (or a bare non-list arg like `5') is ignored, but an odd
    // number of cons cells errors ("Odd length text property list")
    // before OBJECT/position validation.
    {
        let mut n = 0usize;
        let mut tail = a[2].clone();
        loop {
            let next = match &tail {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => break,
            };
            n += 1;
            tail = next;
        }
        if n % 2 == 1 {
            return Err(i.error("Odd length text property list"));
        }
    }
    // GNU `validate_interval_range' checks OBJECT
    // (buffer-or-string-p) before START/END.
    match a.get(3) {
        None | Some(Value::Nil) | Some(Value::Str(_)) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
    // String object path (0-based positions).
    if let Some(Value::Str(s)) = a.get(3) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        str_pos_ok(i, &a[3], st.max(en), len)?;
        let (s0, e0) = (st.min(en), st.max(en));
        let plist = a[2].list_to_vec().unwrap_or_default();
        let pairs: Vec<(Value, Value)> = plist
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect();
        let fill: Vec<Value> = pairs
            .iter()
            .rev()
            .filter(|(_, v)| !v.is_nil())
            .flat_map(|(k, v)| [k.clone(), v.clone()])
            .collect();
        let fill = if fill.is_empty() { None } else { Some(fill) };
        let mut ivs = std::mem::take(i.str_props_mut(&s));
        // GNU returns t only when a property value actually changed.
        let mut changed = fill.is_some() && iv_gap_in(&ivs, s0, e0);
        let ii: &Interp = i;
        // GNU plput prepends new props per pair.
        iv_apply(
            &mut ivs,
            s0,
            e0,
            |pl| {
                for (k, v) in &pairs {
                    let id = ii.sym_id(k);
                    if v.is_nil() {
                        if let Some(id) = id {
                            if str_plist_get(pl, id, ii).is_some() {
                                changed = true;
                            }
                            str_plist_remove(pl, id, ii);
                        }
                    } else {
                        let same = id
                            .and_then(|id| str_plist_get(pl, id, ii))
                            .map(|cur| crate::lisp::builtins::eq_values(&cur, v))
                            .unwrap_or(false);
                        if !same {
                            changed = true;
                            str_plist_put(pl, k, v);
                        }
                    }
                }
            },
            fill,
        );
        i.set_str_props(&s, ivs);
        return Ok(Value::from_bool(changed));
    }
    let b = match a.get(3) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let len;
    let s;
    let e;
    {
        let bb = b.borrow();
        len = bb.text.len();
        let sv = want_int(i, &a[0])?;
        let ev = want_int(i, &a[1])?;
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if sv < lo || sv > hi || ev < lo || ev > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        // GNU validate_region orders START/END (START > END is legal).
        let (sv2, ev2) = (sv.min(ev), sv.max(ev));
        s = pos_idx(len, sv2);
        e = pos_idx(len, ev2);
    }
    let plist = a[2].list_to_vec().unwrap_or_default();
    if s == e {
        // Empty range: GNU returns nil, no hooks, no intervals.
        return Ok(Value::Nil);
    }
    // GNU `interval_has_all_properties' gates modify_text_properties:
    // an add that would change nothing is a silent nil — no
    // read-only check, no hooks.
    {
        let bb = b.borrow();
        let pairs: Vec<(Value, Value)> = plist
            .chunks_exact(2)
            .map(|c| (c[0].clone(), c[1].clone()))
            .collect();
        if !buf_needs_prop_change(i, &bb, s, e, &pairs) {
            return Ok(Value::Nil);
        }
    }
    // GNU: unconditional before/after-change on the buffer path,
    // run with OBJECT current (GNU `set_buffer_internal').
    let bid = b.borrow().id;
    // GNU validate_interval_range(hard) creates the interval tree
    // BEFORE verify — defaults/aliases gate prop writes too.
    b.borrow().text.had_intervals.set(true);
    chg_with_buffer(i, bid, |i| {
        check_writable(i)?;
        text_ro_range(i, s, e)?;
        signal_before_change(i, s + 1, e + 1)?;
        let changed = {
            let mut bb = b.borrow_mut();
            let r = buf_add_props(&mut bb, i, s, e, &plist, true);
            if r.is_ok() {
                bb.note_prop_modified();
            }
            r
        };
        signal_after_change(i, s + 1, e + 1, (e - s) as i128)?;
        changed
    })
}

/// Buffer side of `add-text-properties': records undo entries for
/// each property pair (when RECORD), pushes the flat entries, and
/// returns t when a value actually changed.
fn buf_add_props(
    bb: &mut Buffer,
    i: &Interp,
    s: usize,
    e: usize,
    plist: &[Value],
    record: bool,
) -> EvalResult {
    let mut changed = false;
    let mut k = 0;
    while k + 1 < plist.len() {
        if let Some(p) = i.sym_id(&plist[k]) {
            // GNU: t only when the value actually changes somewhere.
            if !buf_prop_uniform(bb, s, e, p, &plist[k + 1]) {
                changed = true;
            }
            if record {
                buf_record_prop(bb, s, e, &plist[k], p, &plist[k + 1]);
            }
            push_prop(
                &mut bb.text.props_write(),
                TextProp {
                    start: s,
                    end: e,
                    prop: p,
                    value: plist[k + 1].clone(),
                },
            );
        }
        k += 2;
    }
    Ok(Value::from_bool(changed))
}

/// True when every position in [S,E) already carries PROP == VAL
/// (an absent PROP reads as nil).
/// GNU `interval_has_all_properties' over [S,E): does every segment
/// in the range hold each PLIST pair with an `eq' value?  When the
/// answer is yes for the whole range, GNU's add/put path skips
/// `modify_text_properties' entirely — meaning no read-only check and
/// no change hooks.
fn buf_needs_prop_change(
    i: &Interp,
    bb: &Buffer,
    s: usize,
    e: usize,
    pairs: &[(Value, Value)],
) -> bool {
    if s >= e {
        return false;
    }
    let props = bb.text.props.borrow();
    let mut bounds: Vec<usize> = vec![s];
    for tp in props.iter() {
        if tp.start > s && tp.start < e {
            bounds.push(tp.start);
        }
        if tp.end > s && tp.end < e {
            bounds.push(tp.end);
        }
    }
    bounds.sort_unstable();
    bounds.dedup();
    // Fail = "needs change": one segment lacking a pair or holding a
    // non-`eq' value.  Segments with no record at all count as
    // "property absent".
    'seg: for &b in &bounds {
        for (k, v) in pairs {
            let kid = match i.sym_id(k) {
                Some(id) => id,
                None => {
                    // A non-symbol key can never appear in records →
                    // the pair is missing → change needed (GNU would
                    // see it as absent on every interval too).
                    return true;
                }
            };
            let mut found = false;
            for tp in props.iter() {
                if tp.prop == kid && b >= tp.start && b < tp.end {
                    found = true;
                    // GNU compares with EQ.
                    if !crate::lisp::builtins::eq_values(&tp.value, v) {
                        return true;
                    }
                    break;
                }
            }
            if !found {
                return true;
            }
        }
        continue 'seg;
    }
    false
}

fn buf_prop_uniform(bb: &Buffer, s: usize, e: usize, prop: u32, val: &Value) -> bool {
    let absent_ok = val.is_nil();
    let mut p = s;
    for (rs, re, rv) in prop_runs(&bb.text.props.borrow(), s, e, prop) {
        if (rs > p && !absent_ok) || !crate::lisp::builtins::eq_values(&rv, val) {
            return false;
        }
        p = re;
    }
    p >= e || absent_ok
}

/// Effective runs of PROP over [S,E) (0-based): sorted, disjoint
/// `(start, end, value)` stretches where PROP is present, with
/// last-write-wins when entries overlap.  Writers keep same-prop
/// entries disjoint (`push_prop'), so the usual cost is one sort; the
/// overlap fallback splices in write order.
fn prop_runs(props: &[TextProp], s: usize, e: usize, prop: u32) -> Vec<(usize, usize, Value)> {
    let hits: Vec<&TextProp> = props
        .iter()
        .filter(|tp| tp.prop == prop && tp.start < e && tp.end > s)
        .collect();
    let mut runs: Vec<(usize, usize, Value)> = hits
        .iter()
        .map(|tp| (tp.start.max(s), tp.end.min(e), tp.value.clone()))
        .collect();
    runs.sort_by_key(|r| r.0);
    if runs.windows(2).all(|w| w[0].1 <= w[1].0) {
        return runs;
    }
    let mut runs: Vec<(usize, usize, Value)> = Vec::new();
    for tp in hits {
        let (os, oe) = (tp.start.max(s), tp.end.min(e));
        let mut out: Vec<(usize, usize, Value)> = Vec::with_capacity(runs.len() + 1);
        for (rs, re, rv) in runs.drain(..) {
            if re <= os || rs >= oe {
                out.push((rs, re, rv));
                continue;
            }
            if rs < os {
                out.push((rs, os, rv.clone()));
            }
            if re > oe {
                out.push((oe, re, rv));
            }
        }
        out.push((os, oe, tp.value.clone()));
        out.sort_by_key(|r| r.0);
        runs = out;
    }
    runs
}

/// Write one entry, first clipping the same-prop entries it covers so
/// those stay disjoint: the log then mirrors the live interval
/// structure instead of growing with every write (lookups scan it, so
/// repeated put/add over the same text went quadratic).
fn push_prop(v: &mut Vec<TextProp>, tp: TextProp) {
    let (s, e, p) = (tp.start, tp.end, tp.prop);
    if v.iter().any(|o| o.prop == p && o.start < e && o.end > s) {
        clip_props(v, s, e, |q| q == p);
    }
    v.push(tp);
}

/// GNU interval semantics: removing or replacing properties over
/// [S,E) splits the covering intervals at the boundaries — the parts
/// outside the range keep their properties.  PRED selects which prop
/// names are cleared.
fn clip_props(v: &mut Vec<TextProp>, s: usize, e: usize, pred: impl Fn(u32) -> bool) {
    let mut out = Vec::with_capacity(v.len());
    for tp in v.drain(..) {
        if !pred(tp.prop) || tp.end <= s || tp.start >= e {
            out.push(tp);
            continue;
        }
        if tp.start < s {
            let mut left = tp.clone();
            left.end = s;
            out.push(left);
        }
        if tp.end > e {
            let mut right = tp;
            right.start = e;
            out.push(right);
        }
    }
    *v = out;
}

/// Shared string-object branch of remove-text-properties and
/// remove-list-of-text-properties: NAMES is a list of prop symbols
/// (remove-text-properties takes a plist and uses its keys).
pub(crate) fn str_remove_props(
    i: &mut Interp,
    a: &[Value],
    names: Vec<Value>,
) -> Result<bool, Flow> {
    let Value::Str(s) = a.get(3).unwrap() else {
        return Ok(false);
    };
    let s = s.clone();
    let len = str_len(&s);
    let st = want_int(i, &a[0])?.max(0) as usize;
    let en = want_int(i, &a[1])?.max(0) as usize;
    str_pos_ok(i, &a[3], st.max(en), len)?;
    let (s0, e0) = (st.min(en), st.max(en));
    let ids: Vec<u32> = names.iter().filter_map(|v| i.sym_id(v)).collect();
    let mut ivs = std::mem::take(i.str_props_mut(&s));
    let mut removed = false;
    let ii: &Interp = i;
    iv_apply(
        &mut ivs,
        s0,
        e0,
        |pl| {
            for id in &ids {
                let before = pl.len();
                str_plist_remove(pl, *id, ii);
                removed |= pl.len() != before;
            }
        },
        None,
    );
    i.set_str_props(&s, ivs);
    Ok(removed)
}

pub(crate) fn f_remove_text_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `validate_interval_range' checks OBJECT
    // (buffer-or-string-p) before START/END.
    match a.get(3) {
        None | Some(Value::Nil) | Some(Value::Str(_)) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
    if let Some(Value::Str(_)) = a.get(3) {
        // PROPERTIES is a plist; remove the named keys.
        let plist = a[2].list_to_vec().unwrap_or_default();
        let names: Vec<Value> = plist.into_iter().step_by(2).collect();
        let removed = str_remove_props(i, &a, names)?;
        return Ok(Value::from_bool(removed));
    }
    let b = match a.get(3) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let len;
    let s;
    let e;
    {
        let bb = b.borrow();
        len = bb.text.len();
        let sv = want_int(i, &a[0])?;
        let ev = want_int(i, &a[1])?;
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if sv < lo || sv > hi || ev < lo || ev > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        // GNU validate_region orders START/END (START > END is legal).
        let (sv2, ev2) = (sv.min(ev), sv.max(ev));
        s = pos_idx(len, sv2);
        e = pos_idx(len, ev2);
    }
    let plist = a[2].list_to_vec().unwrap_or_default();
    if s == e {
        return Ok(Value::Nil);
    }
    let props: Vec<u32> = plist
        .iter()
        .step_by(2)
        .filter_map(|v| i.sym_id(v))
        .collect();
    // GNU removes lazily: `modify_text_properties' and
    // `signal_after_change' fire only when something is removed.
    let will_change = {
        let bb = b.borrow();
        bb.text
            .props
            .borrow()
            .iter()
            .any(|tp| props.contains(&tp.prop) && tp.start < e && tp.end > s)
    };
    if !will_change {
        return Ok(Value::Nil);
    }
    let bid = b.borrow().id;
    // GNU validate_interval_range(hard) creates the interval tree
    // BEFORE verify — defaults/aliases gate prop writes too.
    b.borrow().text.had_intervals.set(true);
    chg_with_buffer(i, bid, |i| {
        check_writable(i)?;
        text_ro_range(i, s, e)?;
        signal_before_change(i, s + 1, e + 1)?;
        {
            let mut bb = b.borrow_mut();
            for (n, &p) in props.iter().enumerate() {
                buf_record_prop(&mut bb, s, e, &plist[n * 2], p, &Value::Nil);
            }
            clip_props(&mut bb.text.props_write(), s, e, |p| {
                props.contains(&p)
            });
            bb.note_prop_modified();
        }
        signal_after_change(i, s + 1, e + 1, (e - s) as i128)
    })?;
    Ok(Value::t())
}

fn f_set_text_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `set_text_properties' runs validate_plist on PROPERTIES
    // BEFORE decoding OBJECT or START/END — an odd-length list signals
    // "Odd length text property list" ahead of any type/range error.
    // nil → nil; non-list → (PROP nil) wrapper; dotted list → same error.
    let props: Value = match &a[2] {
        Value::Nil => Value::Nil,
        Value::Cons(_) => match a[2].list_to_vec() {
            Ok(v) if v.len() % 2 == 1 => {
                return Err(i.error("Odd length text property list"));
            }
            Ok(_) => a[2].clone(),
            Err(_) => return Err(i.error("Odd length text property list")),
        },
        other => Value::list(vec![other.clone(), Value::Nil]),
    };
    match a.get(3) {
        None | Some(Value::Nil) | Some(Value::Str(_)) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
    if let Some(Value::Str(s)) = a.get(3) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        str_pos_ok(i, &a[3], st.max(en), len)?;
        let (s0, e0) = (st.min(en), st.max(en));
        let plist = props.list_to_vec().unwrap_or_default();
        let fill = if plist.is_empty() {
            None
        } else {
            Some(plist.clone())
        };
        let mut ivs = std::mem::take(i.str_props_mut(&s));
        // GNU replaces the interval's plist wholesale.
        iv_apply(&mut ivs, s0, e0, |pl| *pl = plist.clone(), fill);
        i.set_str_props(&s, ivs);
        return Ok(Value::t());
    }
    // Remove all props in range, then add the plist.
    let b = match a.get(3) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let len;
    let s;
    let e;
    {
        let bb = b.borrow();
        len = bb.text.len();
        let sv = want_int(i, &a[0])?;
        let ev = want_int(i, &a[1])?;
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if sv < lo || sv > hi || ev < lo || ev > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        // GNU validate_region orders START/END (START > END is legal).
        let (sv2, ev2) = (sv.min(ev), sv.max(ev));
        s = pos_idx(len, sv2);
        e = pos_idx(len, ev2);
    }
    let plist = props.list_to_vec().unwrap_or_default();
    if s == e {
        return Ok(Value::Nil);
    }
    // GNU set_text_properties: when PROPERTIES is nil and the buffer
    // has no interval tree, `validate_interval_range' soft-returns and
    // the function exits BEFORE prepare_to_modify_buffer — no
    // buffer-read-only check.  Any existing interval flips it.
    {
        let bb = b.borrow();
        if plist.is_empty() && !bb.text.had_intervals.get() {
            return Ok(Value::Nil);
        }
    }
    // GNU `set_text_properties_1': unconditional hooks + MODIFF bump,
    // with OBJECT made current (GNU `set_buffer_internal').
    let bid = b.borrow().id;
    // GNU validate_interval_range(hard) creates the interval tree
    // BEFORE verify — defaults/aliases gate prop writes too.
    b.borrow().text.had_intervals.set(true);
    chg_with_buffer(i, bid, |i| {
        check_writable(i)?;
        text_ro_range(i, s, e)?;
        signal_before_change(i, s + 1, e + 1)?;
        {
            let mut bb = b.borrow_mut();
            buf_record_set(&mut bb, i, s, e, &plist);
            clip_props(&mut bb.text.props_write(), s, e, |_| true);
            buf_add_props(&mut bb, i, s, e, &plist, false)?;
            bb.note_prop_modified();
        }
        signal_after_change(i, s + 1, e + 1, (e - s) as i128)
    })?;
    Ok(Value::t())
}

/// GNU `set_properties' undo recording for `set-text-properties':
/// over each contiguous run sharing an old plist, record `(nil SYM
/// OLD BEG . END)' for each old property missing-or-different in the
/// new plist, and `(nil SYM nil BEG . END)' for each new property
/// absent from the old one.
fn buf_record_set(bb: &mut Buffer, i: &Interp, s: usize, e: usize, plist: &[Value]) {
    let mut p = s;
    while p < e {
        let old = bb.plist_at(p);
        let mut q = p + 1;
        while q < e {
            let pl = bb.plist_at(q);
            if pl.len() != old.len()
                || !pl.iter().all(|(k, v)| {
                    old.get(k)
                        .is_some_and(|o| crate::lisp::builtins::eq_values(o, v))
                })
            {
                break;
            }
            q += 1;
        }
        for (pid, oldv) in &old {
            let newv = plist
                .chunks(2)
                .find(|c| c.len() == 2 && i.sym_id(&c[0]) == Some(*pid))
                .map(|c| c[1].clone())
                .unwrap_or(Value::Nil);
            if !crate::lisp::builtins::eq_values(&newv, oldv) {
                bb.record_prop_change(Value::Sym(*pid), oldv.clone(), p + 1, q + 1);
            }
        }
        for c in plist.chunks(2) {
            if c.len() != 2 {
                continue;
            }
            if let Some(pid) = i.sym_id(&c[0]) {
                if !old.contains_key(&pid) {
                    bb.record_prop_change(c[0].clone(), Value::Nil, p + 1, q + 1);
                }
            }
        }
        p = q;
    }
}

/// Raw (no category/alias/default) property lookup at 0-based POS —
/// the flat analogue of GNU's `plist_get' on an interval plist.
pub(crate) fn direct_prop_at_bb(
    _i: &Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> Value {
    for tp in bb.text.props.borrow().iter().rev() {
        if pos >= tp.start && pos < tp.end && tp.prop == prop {
            return tp.value.clone();
        }
    }
    Value::Nil
}

/// The tail of `char-property-alias-alist' for PROP: raw `plist_get'
/// lookups against the flat records (GNU calls plist_get on the
/// interval plist — no category recursion).
fn alias_prop_at_bb(
    i: &Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> Value {
    let alist = i.symbol_value(i.intern_soft("char-property-alias-alist").map(|s| s).unwrap_or(u32::MAX));
    let mut found = Value::Nil;
    if let Value::Cons(_) = alist {
        // Fassq on the alist, then scan its cdr aliases in order.
        if let Some(cell) = alist
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .find(|e| {
                matches!(e, Value::Cons(c) if i.sym_id(&c.borrow().car) == Some(prop))
            })
        {
            if let Value::Cons(c) = cell {
                let aliases = c.borrow().cdr.clone();
                for a in aliases.list_to_vec().unwrap_or_default() {
                    if let Some(id) = i.sym_id(&a) {
                        let v = direct_prop_at_bb(i, bb, pos, id);
                        if !v.is_nil() {
                            found = v;
                            break;
                        }
                    }
                }
            }
        }
    }
    found
}

/// `plist_get' against `default-text-properties' (GNU's last
/// lookup_char_property step, only for textprop lookups).
fn default_prop(i: &Interp, prop: u32) -> Value {
    let dtp = i.symbol_value(i.intern_soft("default-text-properties").unwrap_or(u32::MAX));
    let mut cur = dtp;
    while let Value::Cons(c) = &cur {
        let (k, rest) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(vc) = &rest {
            let (v, tl) = {
                let b = vc.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if i.sym_id(&k) == Some(prop) {
                return v;
            }
            cur = tl;
        } else {
            break;
        }
    }
    Value::Nil
}

/// Text property PROP at 0-based POS in buffer B — GNU's
/// `lookup_char_property' chain on the flat record store:
/// covering direct records (last write wins) → `category' symbol's
/// plist → `char-property-alias-alist' aliases →
/// `default-text-properties'.
pub(crate) fn prop_at_bb(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> Value {
    let cat = i.intern_soft("category");
    let mut catval = Value::Nil;
    {
        let props = bb.text.props.borrow();
        for tp in props.iter().rev() {
            if pos >= tp.start && pos < tp.end {
                if tp.prop == prop {
                    return tp.value.clone();
                }
                if Some(tp.prop) == cat && catval.is_nil() {
                    catval = tp.value.clone();
                }
            }
        }
    }
    if let Value::Sym(cs) = catval {
        let v = i.get_prop(cs, prop);
        if !v.is_nil() {
            return v;
        }
    }
    let v = alias_prop_at_bb(i, bb, pos, prop);
    if !v.is_nil() {
        return v;
    }
    default_prop(i, prop)
}

pub(crate) fn prop_at(i: &mut Interp, pos: usize, prop: u32) -> Value {
    let b = cur(i);
    let bb = b.borrow();
    prop_at_bb(i, &bb, pos, prop)
}

/// Resolve a `get-*-property' OBJECT argument to the buffer it
/// designates: a live window's buffer, a buffer itself, or the
/// current buffer for nil.  Strings are handled by callers first.
pub(crate) fn prop_object_buffer(
    i: &mut Interp,
    v: Option<&Value>,
) -> Result<Rc<RefCell<crate::buffer::Buffer>>, Flow> {
    match v {
        Some(Value::Window(w)) => {
            let wb = w.borrow();
            if wb.dead {
                let v = Value::Window(w.clone());
                return Err(i.wrong_type_mut("bufferp", &v));
            }
            let id = wb.buffer;
            drop(wb);
            match i.buffer_value(id) {
                Some(Value::Buffer(b)) => Ok(b),
                _ => Err(i.error("Selecting deleted buffer")),
            }
        }
        Some(v) => buf_of(i, v),
        None => Ok(cur(i)),
    }
}

/// GNU `CHECK_STRING_OR_BUFFER' — OBJECT may be a string, buffer,
/// or nil; windows and other objects signal `buffer-or-string-p'.
/// Used by the mutation/scan property functions (put/add/set/remove-
/// text-properties, *-property-change, text-properties-at...).
fn buf_or_string_ok(i: &mut Interp, v: Option<&Value>) -> Result<(), Flow> {
    match v {
        None | Some(Value::Nil) | Some(Value::Buffer(_)) | Some(Value::Str(_)) => Ok(()),
        Some(v) => Err(i.wrong_type_mut("buffer-or-string-p", v)),
    }
}

pub(crate) fn f_get_text_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Emacs arg order: (get-text-property POSITION PROP &optional OBJECT).
    // GNU Fget_text_property → Ftext_properties_at →
    // validate_interval_range(object,&position,&position,soft): OBJECT
    // resolves first — nil → current buffer, then buffer-or-string-p —
    // THEN the marker POSITION coerce (detached marker + string signals
    // "Marker does not point anywhere"; with a bad OBJECT the
    // buffer-or-string-p wins first).
    buf_or_string_ok(i, a.get(2))?;
    if let Some(Value::Marker(m)) = a.get(0) {
        if m.borrow().buffer.is_none() {
            return Err(i.error("Marker does not point anywhere"));
        }
    }
    if let Some(Value::Str(s)) = a.get(2) {
        let s = s.clone();
        // GNU fix_position coerces markers to their char position; the
        // marker's own buffer is irrelevant for string indexing.
        let pos_v = want_int_or_marker_pos(i, &a[0])?;
        // GNU `validate_interval_range' is called with BEG=END=POS:
        // positions outside [0, len] signal (args-out-of-range POS POS).
        if pos_v < 0 || pos_v > str_len(&s) as i128 {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[0].clone()],
            ));
        }
        let Some(prop) = i.sym_id(&a[1]) else {
            return Ok(Value::Nil);
        };
        let pl = str_plist_at(i.str_props(&s), pos_v as usize);
        return Ok(str_plist_get_cat(&pl, prop, i).unwrap_or(Value::Nil));
    }
    // OBJECT may be a buffer: GNU validates POSITION against that
    // buffer's bounds and reads its intervals.
    let b = match a.get(2) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len();
    let pos_v = want_int_or_marker_pos(i, &a[0])?;
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    if pos_v < lo || pos_v > hi {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_idx(len, pos_v);
    let Some(prop) = i.sym_id(&a[1]) else {
        return Ok(Value::Nil);
    };
    Ok(prop_at_bb(i, &bb, pos, prop))
}

/// GNU `get_char_property_and_overlay' order: among the overlays
/// covering POS, the winner is the one with the highest `priority',
/// then the smallest end, then the most recently created.  Returns
/// the effective value of PROP at 0-based POS (overlays first, then
/// the text property), plus the winning overlay's handle when the
/// value came from an overlay.
pub(crate) fn char_prop_at_overlay(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    pos: usize,
    prop: u32,
) -> (Value, Value) {
    let prio = i.intern("priority");
    let mut best: Option<(i128, std::cmp::Reverse<usize>, usize, Value, Value)> = None;
    for (idx, ov) in bb.overlays.iter().enumerate() {
        if ov.buffer != Some(bb.id) || pos < ov.start || pos >= ov.end {
            continue;
        }
        let v = crate::lisp::eval::plist_get(&ov.plist, prop);
        if v.is_nil() {
            continue;
        }
        let p = crate::lisp::eval::plist_get(&ov.plist, prio)
            .int()
            .unwrap_or(0);
        let key = (p, std::cmp::Reverse(ov.end), idx);
        let better = match &best {
            None => true,
            Some((bp, bend, bidx, _, _)) => key > (*bp, *bend, *bidx),
        };
        if better {
            best = Some((p, std::cmp::Reverse(ov.end), idx, v, ov.handle.clone()));
        }
    }
    if let Some((_, _, _, v, h)) = best {
        return (v, h);
    }
    (prop_at_bb(i, bb, pos, prop), Value::Nil)
}

/// `get-char-property' — text properties plus overlays.
pub(crate) fn f_get_char_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU get_char_property_and_overlay: fix_position FIRST (detached
    // markers signal "Marker does not point anywhere"; non-int non-marker
    // → integer-or-marker-p), then OBJECT resolves — nil → current
    // buffer, live window → its buffer, and buffer-or-string-p on the
    // rest (sym signals even when POSITION is in range).  A STRING object
    // skips overlays; GNU falls into Fget_text_property on the coerced
    // position, whose prop lookup on a string returns nil for markers.
    let pos_v = want_int_or_marker_pos(i, &a[0])?;
    if let Some(Value::Str(_)) = a.get(2) {
        // String objects have no overlays — same as get-text-property.
        return f_get_text_property(i, a);
    }
    let b = match a.get(2) {
        None | Some(Value::Nil) => cur(i),
        Some(Value::Buffer(_)) | Some(Value::Window(_)) => prop_object_buffer(i, a.get(2))?,
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    };
    let len = b.borrow().text.len();
    let pos = {
        let bb0 = b.borrow();
        // GNU validates the fix_position'd value against BEGV..ZV but the
        // error datum is the original POSITION arg.
        let (lo, hi) = (bb0.begv as i128 + 1, bb0.zv as i128 + 1);
        if pos_v < lo || pos_v > hi {
            return Err(err_sym(i, "args-out-of-range", vec![a[0].clone()]));
        }
        drop(bb0);
        pos_idx(len, pos_v)
    };
    let Some(prop) = i.sym_id(&a[1]) else {
        return Ok(Value::Nil);
    };
    let bb = b.borrow();
    Ok(char_prop_at_overlay(i, &bb, pos, prop).0)
}

/// `get-pos-property' — the property text inserted at POSITION would
/// inherit: overlays at POSITION win; otherwise the rear-sticky props
/// of the char before POSITION, then the front-sticky props of the
/// char at POSITION (GNU Fget_pos_property).
pub(crate) fn f_get_pos_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Same GNU check order as get-char-property: POSITION first, then
    // OBJECT — and a non-buffer/string object means current buffer.
    match &a[0] {
        Value::Int(_) | Value::Marker(_) => {}
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
    if let Some(Value::Str(_)) = a.get(2) {
        // On a string, GNU degenerates to get-char-property.
        return f_get_char_property(i, a);
    }
    let b = match a.get(2) {
        // GNU CHECK_BUFFER_OR_STRING: nil/omitted = current buffer;
        // windows map to their buffer; everything else is a type error.
        None | Some(Value::Nil) => cur(i),
        Some(Value::Buffer(_)) | Some(Value::Window(_)) => prop_object_buffer(i, a.get(2))?,
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    };
    let len;
    let pos;
    {
        let bb = b.borrow();
        len = bb.text.len();
        // GNU get-pos-property never range-checks POSITION itself: the
        // stickiness logic first consults text props at POS-1 (skipped
        // when POS <= BEGV+1), then at POS.  An out-of-range hit signals
        // args-out-of-range for whichever position was consulted.
        let p0 = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if p0 > lo && p0 - 1 > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![Value::Int(p0 - 1), Value::Int(p0 - 1)],
            ));
        }
        if p0 < lo || p0 > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![Value::Int(p0), Value::Int(p0)],
            ));
        }
        pos = pos_idx(len, p0);
    }
    let Some(prop) = i.sym_id(&a[1]) else {
        return Ok(Value::Nil);
    };
    {
        let bb = b.borrow();
        let (v, ov) = char_prop_at_overlay(i, &bb, pos, prop);
        if !ov.is_nil() {
            return Ok(v);
        }
    }
    if pos > 0 {
        let bb = b.borrow();
        if !rear_nonsticky_covers_bb(i, &bb, pos - 1, prop) {
            let v = prop_at_bb(i, &bb, pos - 1, prop);
            if v.truthy() {
                return Ok(v);
            }
        }
    }
    if pos < len {
        let bb = b.borrow();
        if front_sticky_covers_bb(i, &bb, pos, prop) {
            return Ok(prop_at_bb(i, &bb, pos, prop));
        }
    }
    Ok(Value::Nil)
}

fn f_text_properties_at(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(Value::Str(s)) = a.get(1) {
        let pos_v = want_int(i, &a[0])?;
        if pos_v < 0 || pos_v > str_len(s) as i128 {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[0].clone()],
            ));
        }
        let pos = pos_v as usize;
        let pl = str_plist_at(i.str_props(s), pos);
        return Ok(if pl.is_empty() {
            Value::Nil
        } else {
            Value::list(pl)
        });
    }
    // GNU gates OBJECT through CHECK_STRING_OR_BUFFER — windows signal
    // `buffer-or-string-p' (only get-char/get-pos/and-overlay take them).
    buf_or_string_ok(i, a.get(1))?;
    let b = match a.get(1) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len();
    let pos_v = want_int(i, &a[0])?;
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    if pos_v < lo || pos_v > hi {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_idx(len, pos_v);
    // GNU's plist is newest-first (put-text-property prepends):
    // iterate records in reverse and keep the newest value per prop.
    let mut plist: Vec<Value> = Vec::new();
    for tp in bb.text.props.borrow().iter().rev() {
        if pos >= tp.start && pos < tp.end {
            let psym = i.sym(tp.prop);
            let mut dup = false;
            let mut k = 0;
            while k + 1 < plist.len() {
                if crate::lisp::builtins::eq_values(&plist[k], &psym) {
                    dup = true;
                    break;
                }
                k += 2;
            }
            if !dup {
                plist.push(psym);
                plist.push(tp.value.clone());
            }
        }
    }
    if plist.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::list(plist))
    }
}

/// Shared plist-comparison for `next/previous-property-change': two
/// positions "differ" iff any prop value at one is not `eq' to the
/// value at the other (GNU compares each pair with `eq').
fn str_plists_differ(a: &[Value], b: &[Value], i: &Interp) -> bool {
    let mut seen: Vec<u32> = Vec::new();
    let mut k = 0;
    while k + 1 < a.len() {
        if let Some(id) = i.sym_id(&a[k]) {
            seen.push(id);
            let bv = str_plist_get(b, id, i).unwrap_or(Value::Nil);
            if !crate::lisp::builtins::eq_values(&a[k + 1], &bv) {
                return true;
            }
        }
        k += 2;
    }
    let mut k = 0;
    while k + 1 < b.len() {
        if let Some(id) = i.sym_id(&b[k]) {
            if !seen.contains(&id) {
                return true;
            }
        }
        k += 2;
    }
    false
}

/// String-object engine for `next/previous-property-change':
/// (POS &optional OBJECT LIMIT).  A "change" at P means the plist at
/// P differs from the plist at P-1.  No change → LIMIT (when non-nil)
/// else nil.
fn str_prop_change(i: &mut Interp, a: &[Value], forward: bool) -> EvalResult {
    let Value::Str(s) = a.get(1).unwrap() else {
        return Ok(Value::Nil);
    };
    let len = str_len(s) as i128;
    let ivs = i.str_props(s);
    let pos_v = a[0].int().unwrap_or(0);
    // GNU validate_interval_range: POSITION is bounds-checked against
    // the STRING (0..=len) before scanning.
    if pos_v < 0 || pos_v > len {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_v.clamp(0, len);
    let limit = a.get(2);
    let lim = match limit {
        Some(v) if v.truthy() => Some(v.int().unwrap_or(if forward { len } else { 0 })),
        _ => None,
    };
    if forward {
        let mut p = pos + 1;
        while p < lim.unwrap_or(len) {
            let pa = str_plist_at(ivs, p as usize);
            let pb = str_plist_at(ivs, (p - 1) as usize);
            if str_plists_differ(&pa, &pb, i) {
                return Ok(Value::Int(p));
            }
            p += 1;
        }
    } else {
        let mut p = pos - 1;
        while p > lim.unwrap_or(0) && p >= 1 {
            let pa = str_plist_at(ivs, p as usize);
            let pb = str_plist_at(ivs, (p - 1) as usize);
            if str_plists_differ(&pa, &pb, i) {
                return Ok(Value::Int(p));
            }
            p -= 1;
        }
    }
    Ok(match lim {
        Some(l) => Value::Int(l),
        None => Value::Nil,
    })
}

pub(crate) fn f_next_property_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fnext_property_change (POSITION OBJECT LIMIT): LIMIT is
    // coerced (integer-or-marker, `t' exempt) BEFORE POSITION is
    // type/range-checked, and OBJECT defaults to the current buffer.
    if let Some(v) = a.get(2) {
        if v.truthy() && !matches!(v, Value::Sym(s) if i.symbol_name(*s) == "t") {
            crate::lisp::builtins::want_int_or_mark(i, v)?;
        }
    }
    if let Some(Value::Str(_)) = a.get(1) {
        return str_prop_change(i, &a, true);
    }
    buf_or_string_ok(i, a.get(1))?;
    let b = match a.get(1) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let bb = b.borrow();
    let zv = bb.zv;
    let begv = bb.begv;
    let pos_v = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    if pos_v < begv as i128 + 1 || pos_v > zv as i128 + 1 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_idx(zv, pos_v);
    // GNU's interval tree sees only the accessible region: interval
    // STARTS are {BEGV+1} ∪ {raw prop-run edges clamped into
    // [BEGV+1, ZV+1]}.  `next_interval' walks starts > POS; the walk
    // stops at the first interval whose plist differs from the one
    // containing POS (intervals_equal compares against i, not
    // pairwise neighbors).
    let i_plist = bb.plist_at(pos.min(zv));
    let differs_from_i = |bb: &crate::buffer::Buffer, s: i128| -> bool {
        let l = bb.plist_at((s - 1).clamp(0, zv as i128) as usize);
        l.len() != i_plist.len()
            || l.iter().any(|(k, v)| {
                !i_plist
                    .get(k)
                    .map(|rv| crate::lisp::builtins::eq_values(v, rv))
                    .unwrap_or(false)
            })
            || i_plist.iter().any(|(k, _)| !l.contains_key(k))
    };
    // LIMIT t: GNU returns the NEXT INTERVAL'S start outright (or
    // ZV+1 when POS is in the last interval) — the intervals_equal
    // walk is skipped entirely.
    if let Some(v) = a.get(2) {
        if matches!(v, Value::Sym(s) if i.symbol_name(*s) == "t") {
            let mut raw_next = zv as i128 + 1;
            for tp in bb.text.props.borrow().iter() {
                for e in [tp.start, tp.end] {
                    let e1 = (e as i128 + 1).clamp(begv as i128 + 1, zv as i128 + 1);
                    if e1 > pos_v && e1 < raw_next {
                        raw_next = e1;
                    }
                }
            }
            return Ok(Value::Int(raw_next));
        }
    }
    let lim_v = match a.get(2) {
        Some(v) if v.truthy() => {
            Some(crate::lisp::builtins::want_int_or_mark(i, v)?)
        }
        _ => None,
    };
    let mut next: Option<i128> = None;
    for tp in bb.text.props.borrow().iter() {
        for e in [tp.start, tp.end] {
            let e1 = (e as i128 + 1).clamp(begv as i128 + 1, zv as i128 + 1);
            // GNU: edges at/after LIMIT don't count — the
            // `next->position < XFIXNUM(limit)' walk condition.
            if e1 > pos_v
                && lim_v.map(|l| e1 < l).unwrap_or(true)
                && differs_from_i(&bb, e1)
            {
                next = Some(next.map(|n| n.min(e1)).unwrap_or(e1));
            }
        }
    }
    match next {
        Some(p) => Ok(Value::Int(p)),
        None => match a.get(2) {
            // A marker LIMIT was coerced already; GNU returns its
            // integer value, not the marker object.
            Some(v) if v.truthy() => Ok(Value::Int(
                crate::lisp::builtins::want_int_or_mark(i, v)?,
            )),
            _ => Ok(Value::Nil),
        },
    }
}

/// Shared engine for `next/previous-single(-char)-property-change':
/// (POS PROP &optional OBJECT LIMIT). Buffer positions are 1-based,
/// string positions 0-based; a "change" at P means PROP's value at P
/// differs from its value at P-1. No change → LIMIT (defaults:
/// point-max / point-min for buffers, len / 0 for strings).
fn single_prop_change(i: &mut Interp, a: &[Value], forward: bool) -> EvalResult {
    let object = a.get(2);
    let limit = a.get(3);
    // GNU Fnext_single_property_change order (textprop.c:1037):
    //   1) non-nil LIMIT gets CHECK_FIXNUM_COERCE_MARKER first
    //   2) validate_interval_range checks OBJECT via
    //      CHECK_STRING_OR_BUFFER
    //   3) then POSITION's integer-or-marker coerce.
    match limit {
        Some(v) if v.truthy() => {
            want_int_or_mark(i, v)?;
        }
        _ => {}
    }
    match object {
        None | Some(Value::Nil) | Some(Value::Str(_)) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
    let pos_v = want_int(i, &a[0])?;
    // GNU does not CHECK_SYMBOL PROP — any eq key is legal (a
    // non-symbol key can never appear in a plist, so lookups miss).
    let prop = if let Value::Sym(_) = a[1] {
        want_sym(i, &a[1])?
    } else {
        u32::MAX
    };

    // String object: 0-based positions; nil (not LIMIT) when nothing
    // changes — LIMIT is only returned when it's non-nil.
    if let Some(Value::Str(s)) = object {
        let len = str_len(s) as i128;
        // Compute LIMIT before borrowing the interval set.
        let str_lim = match limit {
            Some(v) if v.truthy() => Some(want_int_or_mark(i, v)?),
            _ => None,
        };
        let ivs = i.str_props(s);
        let at = |p: i128| -> Value {
            if p < 0 || p >= len {
                return Value::Nil;
            }
            // GNU `textget' is category-aware.
            str_plist_get_cat(&str_plist_at(ivs, p as usize), prop, i).unwrap_or(Value::Nil)
        };
        // GNU validate_interval_range bounds POSITION against the
        // OBJECT (the string) — out-of-range signals, not clamps.
        if pos_v < 0 || pos_v > len {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[0].clone()],
            ));
        }
        let pos = pos_v;
        let lim = str_lim;
        if forward {
            let mut p = pos + 1;
            while p < lim.unwrap_or(len) {
                if !crate::lisp::builtins::eq_values(&at(p), &at(p - 1)) {
                    return Ok(Value::Int(p));
                }
                p += 1;
            }
            return Ok(match lim {
                Some(l) => Value::Int(l),
                None => Value::Nil,
            });
        }
        let mut p = pos - 1;
        while p > lim.unwrap_or(0) && p >= 1 {
            if !crate::lisp::builtins::eq_values(&at(p), &at(p - 1)) {
                return Ok(Value::Int(p));
            }
            p -= 1;
        }
        return Ok(match lim {
            Some(l) => Value::Int(l),
            None => Value::Nil,
        });
    }
    // LIMIT coercion happens before the buffer borrow.
    // OBJECT must be a buffer or string (GNU CHECK_STRING_OR_BUFFER
    // order: strings already returned above; windows error here).
    buf_or_string_ok(i, object)?;
    let buf_lim = match limit {
        Some(v) if v.truthy() => Some(want_int_or_mark(i, v)?),
        _ => None,
    };
    let b = match object {
        Some(v) if v.truthy() => buf_of(i, v)?,
        _ => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len() as i128;
    // GNU validate_interval_range checks POS against the OBJECT
    // buffer's bounds, not the current buffer's.
    let pos = {
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if pos_v < lo || pos_v > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[0].clone()],
            ));
        }
        pos_v
    };
    let cat = i.intern_soft("category");
    // Only boundaries of a PROP (or `category') interval can change the
    // effective value.  GNU walks an interval tree; probing every char
    // position with a full `text_props' scan was quadratic, and so is
    // probing every candidate against every entry.  Instead, sweep the
    // boundary events once: coverage of an interval [start+1, end] is
    // constant on each segment between consecutive edges, so compute
    // one value per segment and compare neighbours.
    use std::collections::BTreeMap;
    // Collect boundary events as (lisp-pos, ord, is_start); a sorted
    // Vec replaces the per-call BTreeMap so the sweep below streams
    // instead of building an index per candidate.
    let mut events: Vec<(i128, usize, bool)> = Vec::new();
    for (ord, tp) in bb.text.props.borrow().iter().enumerate() {
        if tp.prop == prop || Some(tp.prop) == cat {
            events.push((tp.start as i128 + 1, ord, true));
            events.push((tp.end as i128 + 1, ord, false));
        }
    }
    events.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    // Covering-entry maps: ord -> value; the maximum ord wins (later
    // `text_props' entries override earlier ones).
    let mut prop_map: BTreeMap<usize, Value> = BTreeMap::new();
    let mut cat_map: BTreeMap<usize, Value> = BTreeMap::new();
    let mut cat_cache: std::collections::HashMap<u32, Value> = Default::default();
    // Apply events at positions <= UPTO starting from index FROM;
    // returns the next unconsumed index.  Writes go through &mut on the
    // field selected per-event so neither map is moved.
    fn next_ev<'a>(
        bb: &'a Buffer,
        pm: &mut BTreeMap<usize, Value>,
        cm: &mut BTreeMap<usize, Value>,
        ords: &[(i128, usize, bool)],
        from: usize,
        upto: i128,
        prop: u32,
        cat: Option<u32>,
    ) -> usize {
        let mut k = from;
        while k < ords.len() && ords[k].0 <= upto {
            let (_, ord, is_start) = ords[k];
            let tp = &bb.text.props.borrow()[ord];
            if is_start {
                if tp.prop == prop {
                    pm.insert(ord, tp.value.clone());
                } else if Some(tp.prop) == cat {
                    cm.insert(ord, tp.value.clone());
                }
            } else if tp.prop == prop {
                pm.remove(&ord);
            } else if Some(tp.prop) == cat {
                cm.remove(&ord);
            }
            k += 1;
        }
        k
    }
    fn derive<'a>(
        i: &Interp,
        pm: &BTreeMap<usize, Value>,
        cm: &BTreeMap<usize, Value>,
        cc: &mut std::collections::HashMap<u32, Value>,
        prop: u32,
    ) -> Value {
        if let Some((_, v)) = pm.iter().next_back() {
            v.clone()
        } else if let Some((_, Value::Sym(cs))) = cm.iter().next_back() {
            // GNU `textget' is category-aware.
            let _ = i;
            cc.entry(*cs)
                .or_insert_with(|| i.get_prop(*cs, prop))
                .clone()
        } else {
            Value::Nil
        }
    }
    let default_limit = if forward { len + 1 } else { 1 };
    // GNU: a nil/absent LIMIT means "no limit"; when no change is
    // found the result is nil then — LIMIT is returned only when the
    // caller explicitly passed one.
    let explicit = buf_lim.is_some();
    let lim = buf_lim.unwrap_or(default_limit);
    let no_change = || {
        if explicit {
            Value::Int(lim)
        } else {
            Value::Nil
        }
    };
    let b = &*bb;
    if forward {
        // GNU never reports a "change" at point-max itself: positions
        // beyond the last character are not inspected.  Value at c-1 is
        // the coverage state right before c's events; value at c is
        // after applying them — a difference means c is a change.
        let mut k = next_ev(b, &mut prop_map, &mut cat_map, &events, 0, pos, prop, cat);
        let mut prev = derive(i, &prop_map, &cat_map, &mut cat_cache, prop);
        while k < events.len() {
            let c = events[k].0;
            if c > lim || c > len {
                break;
            }
            let before = prev.clone();
            k = next_ev(b, &mut prop_map, &mut cat_map, &events, k, c, prop, cat);
            let after = derive(i, &prop_map, &cat_map, &mut cat_cache, prop);
            if !crate::lisp::builtins::eq_values(&after, &before) {
                return Ok(Value::Int(c));
            }
            prev = after;
        }
        Ok(no_change())
    } else {
        // Last change strictly before POS: sweep ascending through the
        // range and remember the last boundary where the value flips.
        let mut k = next_ev(
            b,
            &mut prop_map,
            &mut cat_map,
            &events,
            0,
            lim.max(0),
            prop,
            cat,
        );
        let mut prev = derive(i, &prop_map, &cat_map, &mut cat_cache, prop);
        let mut found: Option<i128> = None;
        while k < events.len() {
            let c = events[k].0;
            if c > pos - 1 {
                break;
            }
            let before = prev.clone();
            k = next_ev(b, &mut prop_map, &mut cat_map, &events, k, c, prop, cat);
            let after = derive(i, &prop_map, &cat_map, &mut cat_cache, prop);
            if !crate::lisp::builtins::eq_values(&after, &before) {
                found = Some(c);
            }
            prev = after;
        }
        match found {
            Some(c) if c >= 2 => Ok(Value::Int(c)),
            _ => Ok(no_change()),
        }
    }
}

fn f_next_single_property_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    single_prop_change(i, &a, true)
}

pub(crate) fn f_prev_property_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fprevious_property_change: LIMIT (a[2]) coerces
    // integer-or-marker BEFORE POSITION is checked — `t' is NOT
    // exempt here (only next-property-change has the Qt shortcut).
    if let Some(v) = a.get(2) {
        if v.truthy() {
            crate::lisp::builtins::want_int_or_mark(i, v)?;
        }
    }
    if let Some(Value::Str(_)) = a.get(1) {
        return str_prop_change(i, &a, false);
    }
    buf_or_string_ok(i, a.get(1))?;
    let b = match a.get(1) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let bb = b.borrow();
    let pos_v = want_pos_in_buf(i, &a[0], &bb)?;
    // GNU: LIMIT floors the scan; nil → BEGV (point-min).
    let lim = match a.get(2) {
        Some(v) if v.truthy() => crate::lisp::builtins::want_int_or_mark(i, v)?,
        _ => bb.begv as i128 + 1,
    };
    // GNU reports the end of the first interval before POS whose
    // plist differs from the interval containing POS.
    let prev = prev_prop_edge(i, &bb, pos_v, lim);
    if prev > lim {
        return Ok(Value::Int(prev));
    }
    match a.get(2) {
        Some(v) if v.truthy() => Ok(Value::Int(
            crate::lisp::builtins::want_int_or_mark(i, v)?,
        )),
        _ => Ok(Value::Nil),
    }
}

fn f_prev_single_property_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    single_prop_change(i, &a, false)
}

/// Overlay-aware variant used by next/previous-single-char-property-
/// change: the effective PROP value at 1-based P is
/// `get-char-property-and-overlay' (overlays win over text props).
/// Mirrors single_prop_change's scan semantics and limit handling.
/// 1-based next overlay endpoint strictly after 1-based POS in BB:
/// GNU `Fnext_overlay_change' clamps every endpoint into
/// [BEGV+1, ZV+1] (overlay.c does this because narrowing adjusts
/// them lazily) and returns point-max when none follows.
pub(crate) fn next_ov_change(bb: &crate::buffer::Buffer, pos: i128) -> i128 {
    let (beg1, zv1) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    let mut best = zv1;
    for ov in &bb.overlays {
        if ov.buffer != Some(bb.id) {
            continue;
        }
        for e in [ov.start, ov.end] {
            let e1 = (e as i128 + 1).clamp(beg1, zv1);
            if e1 > pos && e1 < best {
                best = e1;
            }
        }
    }
    best
}

/// 1-based previous overlay endpoint strictly before POS; defaults
/// to point-min (GNU's Fprevious_overlay_change never answers nil).
pub(crate) fn prev_ov_change(bb: &crate::buffer::Buffer, pos: i128) -> i128 {
    let (beg1, zv1) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    let mut best = beg1;
    for ov in &bb.overlays {
        if ov.buffer != Some(bb.id) {
            continue;
        }
        for e in [ov.start, ov.end] {
            let e1 = (e as i128 + 1).clamp(beg1, zv1);
            if e1 < pos && e1 > best {
                best = e1;
            }
        }
    }
    best
}

/// True when the whole PLIST changes across the boundary between
/// One interval of GNU's narrowed tree, reduced to what the property
/// scans observe: `(start, end, plist)` where positions are 1-based
/// and `end` = `start + length` (the interval ENDS at position end).
/// GNU clamps every raw edge into [BEGV+1, ZV+1] — hidden-region
/// intervals collapse to empty [beg1, beg1)/[zv1, zv1) entries that
/// `prev_interval'/`next_interval' still visit (in raw order) but
/// `find_interval' skips.
fn buf_intervals(
    bb: &crate::buffer::Buffer,
) -> Vec<(i128, i128, std::collections::BTreeMap<u32, Value>)> {
    let (beg1, zv1) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    // Raw boundaries from the prop runs (0-based char edges +1).
    let mut bounds: Vec<i128> = vec![1, bb.text.len() as i128 + 1];
    for tp in bb.text.props.borrow().iter() {
        bounds.push(tp.start as i128 + 1);
        bounds.push(tp.end as i128 + 1);
    }
    bounds.sort_unstable();
    bounds.dedup();
    let mut out = Vec::new();
    for w in bounds.windows(2) {
        let (s, e) = (w[0], w[1]);
        // Plist of the raw segment = plist at its first char.
        let pl = bb.plist_at((s - 1).max(0) as usize);
        out.push((s.clamp(beg1, zv1), e.clamp(beg1, zv1), pl));
    }
    out
}

fn plist_maps_differ(
    a: &std::collections::BTreeMap<u32, Value>,
    b: &std::collections::BTreeMap<u32, Value>,
) -> bool {
    a.len() != b.len()
        || a.iter().any(|(k, v)| {
            !b.get(k)
                .map(|rv| crate::lisp::builtins::eq_values(v, rv))
                .unwrap_or(false)
        })
}

/// GNU `find_interval': the interval whose [start, end) contains
/// 1-based position CUR, skipping empties.  (Positions sit on the
/// char INDEX; interval [s,e) covers s..e-1.)
fn buf_interval_at(
    ivs: &[(i128, i128, std::collections::BTreeMap<u32, Value>)],
    cur: i128,
) -> Option<usize> {
    if let Some(ix) = ivs.iter().position(|(s, e, _)| *s <= cur && cur < *e) {
        return Some(ix);
    }
    // CUR at/after the tree's end → the last non-empty interval.
    ivs.iter().rposition(|(s, e, _)| *s < *e)
}

/// GNU `Fnext_property_change' internals: after the interval
/// containing CUR, the start of the first interval whose plist
/// differs — walking is capped by LIMIT (GNU's `next->position <
/// XFIXNUM(limit)'); returns the found start, else LIMIT.
fn next_prop_edge(i: &mut Interp, bb: &crate::buffer::Buffer, cur: i128, lim: i128) -> i128 {
    let _ = i;
    let ivs = buf_intervals(bb);
    let Some(mut j) = buf_interval_at(&ivs, cur) else {
        return lim;
    };
    let ipl = ivs[j].2.clone();
    while let Some((s, _e, pl)) = ivs.get(j + 1) {
        j += 1;
        if *s >= lim {
            return lim;
        }
        if plist_maps_differ(pl, &ipl) {
            return *s;
        }
    }
    lim
}

/// GNU `Fprevious_property_change' internals: the end of the first
/// interval before CUR's whose plist differs from i's.  When CUR is
/// i's own start, i steps one interval back first
/// (`i->position == position').  Result floored by LIMIT.
fn prev_prop_edge(i: &mut Interp, bb: &crate::buffer::Buffer, cur: i128, lim: i128) -> i128 {
    let _ = i;
    let ivs = buf_intervals(bb);
    let Some(mut i_idx) = buf_interval_at(&ivs, cur) else {
        return lim;
    };
    if ivs[i_idx].0 == cur && i_idx > 0 {
        i_idx -= 1;
    }
    let ipl = ivs[i_idx].2.clone();
    // `previous' walks down from i; the reported boundary is the
    // candidate interval's own (clamped) end — the next interval's
    // start.  Equal-plist intervals are transparent only while their
    // end stays above the floor (GNU's walk condition).
    let mut k = i_idx;
    while k > 0 {
        k -= 1;
        let (_s, e, pl) = &ivs[k];
        if !(*e > lim) {
            break;
        }
        if plist_maps_differ(pl, &ipl) {
            return *e;
        }
    }
    lim
}

/// GNU `Fnext_char_property_change': scans forward from 1-based CUR
/// for ANY property/overlay change, bounded by LIMIT (nil → ZV+1).
/// The effective bound is `min(LIMIT, next-overlay-change)' — overlay
/// endpoints clamped into the accessible region count as boundaries.
pub(crate) fn next_char_prop_edge(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    cur: i128,
    lim: i128,
) -> i128 {
    let temp = lim.min(next_ov_change(bb, cur));
    next_prop_edge(i, bb, cur, temp)
}

/// GNU `Fprevious_char_property_change': scans backward from CUR,
/// floored by `max(LIMIT, previous-overlay-change)'.
pub(crate) fn prev_char_prop_edge(
    i: &mut Interp,
    bb: &crate::buffer::Buffer,
    cur: i128,
    lim: i128,
) -> i128 {
    let temp = lim.max(prev_ov_change(bb, cur));
    prev_prop_edge(i, bb, cur, temp)
}

/// Overlay-aware variant used by next/previous-single-char-property-
/// change: the effective PROP value at 1-based P is
/// `get-char-property-and-overlay' (overlays win over text props).
/// Ports GNU's textprop.c loop verbatim: the scan steps through
/// `next/previous-char-property-change' boundaries and compares the
/// PROP value at each against the initial one, so boundaries where
/// only OTHER properties change still advance the walk (a run of
/// equal-PROP intervals is transparent).
fn single_char_prop_change(i: &mut Interp, a: &[Value], forward: bool) -> EvalResult {
    let object = a.get(2);
    let limit = a.get(3);
    // GNU's OBJECT check precedes POSITION: non-nil/non-string objects
    // go through bufferp (nsccp "" face t) → (bufferp t), and bad
    // OBJECT beats bad POSITION (nsccp 'x face t) → (bufferp t)).
    match object {
        Some(Value::Str(_)) | Some(Value::Buffer(_)) | None | Some(Value::Nil) => {}
        Some(other) => return Err(i.wrong_type_mut("bufferp", other)),
    }
    // GNU CHECK_FIXNUM_COERCE_MARKER accepts markers for POSITION.
    let pos_v = want_int_or_mark(i, &a[0])?;
    let prop = if let Value::Sym(_) = a[1] {
        want_sym(i, &a[1])?
    } else {
        u32::MAX
    };
    // String objects have no overlays — same as the text-prop scan.
    if matches!(object, Some(Value::Str(_))) {
        return single_prop_change(i, a, forward);
    }
    let buf_lim = match limit {
        Some(v) if v.truthy() => Some(want_int_or_mark(i, v)?),
        _ => None,
    };
    let b = match object {
        Some(v) if v.truthy() => buf_of(i, v)?,
        _ => cur(i),
    };
    let (beg1, zv1) = {
        let bb = b.borrow();
        (bb.begv as i128 + 1, bb.zv as i128 + 1)
    };
    if pos_v < beg1 || pos_v > zv1 {
        // GNU reports the read position: pos for `next', pos-1 for
        // `previous' (via the Fget_char_property(pos-1) call).
        let rep = if forward {
            a[0].clone()
        } else {
            Value::Int(pos_v - 1)
        };
        return Err(err_sym(i, "args-out-of-range", vec![rep]));
    }
    // `get-char-property' at 1-based P on this buffer; reads outside
    // [BEGV+1, ZV+1] signal args-out-of-range (P), matching GNU's
    // Fget_char_property → validate_interval_range single-datum.
    let at = |i: &mut Interp, p: i128| -> Result<Value, Flow> {
        if p < beg1 || p > zv1 {
            return Err(err_sym(i, "args-out-of-range", vec![Value::Int(p)]));
        }
        let bb = b.borrow();
        Ok(char_prop_at_overlay(i, &bb, (p - 1) as usize, prop).0)
    };
    if forward {
        let lim = buf_lim.unwrap_or(zv1);
        if pos_v >= lim {
            // GNU: position = limit, capped at ZV when past it.
            return Ok(Value::Int(lim.min(zv1)));
        }
        let initial = at(i, pos_v)?;
        let mut p = pos_v;
        loop {
            let p2 = next_char_prop_edge(i, &b.borrow(), p, lim);
            if p2 >= lim {
                return Ok(Value::Int(lim));
            }
            if !crate::lisp::builtins::eq_values(&at(i, p2)?, &initial) {
                return Ok(Value::Int(p2));
            }
            if p2 >= zv1 {
                return Ok(Value::Int(p2));
            }
            p = p2;
        }
    } else {
        let lim = buf_lim.unwrap_or(beg1);
        if pos_v <= lim {
            // GNU: position = limit, floored at BEGV when below it.
            return Ok(Value::Int(lim.max(beg1)));
        }
        let initial = at(i, pos_v - 1)?;
        let mut p = pos_v;
        loop {
            let p2 = prev_char_prop_edge(i, &b.borrow(), p, lim);
            if p2 <= lim {
                return Ok(Value::Int(lim));
            }
            // GNU reads the char BEFORE the boundary — a boundary at
            // BEGV+1 under a sub-point-min LIMIT signals
            // args-out-of-range(BEGV) through this very read.
            if !crate::lisp::builtins::eq_values(&at(i, p2 - 1)?, &initial) {
                return Ok(Value::Int(p2));
            }
            p = p2;
        }
    }
}

pub(crate) fn f_next_single_char_property_change(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    single_char_prop_change(i, &a, true)
}

pub(crate) fn f_prev_single_char_property_change(
    i: &mut Interp,
    a: crate::lisp::Argv,
) -> EvalResult {
    single_char_prop_change(i, &a, false)
}

pub(crate) fn f_remove_list_of_text_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `validate_interval_range' checks OBJECT
    // (buffer-or-string-p) before START/END — like remove-text-properties.
    match a.get(3) {
        None | Some(Value::Nil) | Some(Value::Str(_)) | Some(Value::Buffer(_)) => {}
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    }
    let names = a[2].list_to_vec().unwrap_or_default();
    if let Some(Value::Str(_)) = a.get(3) {
        let removed = str_remove_props(i, &a, names)?;
        return Ok(Value::from_bool(removed));
    }
    // GNU accepts an OBJECT buffer arg — `buf_of' resolves it (nil →
    // current); the hooks run with it current (GNU set_buffer_internal).
    let b = match a.get(3) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let len = b.borrow().text.len();
    // GNU validates START/END against the OBJECT's BEGV..ZV BEFORE
    // the no-change shortcut — an out-of-range call must signal even
    // when nothing would be removed.
    let (sv, ev) = (want_int(i, &a[0])?, want_int(i, &a[1])?);
    {
        let bb = b.borrow();
        let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if sv < lo || sv > hi || ev < lo || ev > hi {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
    }
    // GNU validate_region orders START/END.
    let (s, e) = (pos_idx(len, sv.min(ev)), pos_idx(len, sv.max(ev)));
    let props: Vec<u32> = names.iter().filter_map(|v| i.sym_id(v)).collect();
    // GNU's remove path is lazy: hooks + MODIFF bump only when a
    // property is actually removed.
    let will_change = {
        let bb = b.borrow();
        bb.text
            .props
            .borrow()
            .iter()
            .any(|tp| props.contains(&tp.prop) && tp.start < e && tp.end > s)
    };
    if !will_change {
        return Ok(Value::Nil);
    }
    let bid = b.borrow().id;
    // GNU validate_interval_range(hard) creates the interval tree
    // BEFORE verify — defaults/aliases gate prop writes too.
    b.borrow().text.had_intervals.set(true);
    chg_with_buffer(i, bid, |i| {
        check_writable(i)?;
        text_ro_range(i, s, e)?;
        signal_before_change(i, s + 1, e + 1)?;
        {
            let mut bb = b.borrow_mut();
            for (n, &p) in props.iter().enumerate() {
                buf_record_prop(&mut bb, s, e, &names[n], p, &Value::Nil);
            }
            clip_props(&mut bb.text.props_write(), s, e, |p| {
                props.contains(&p)
            });
            bb.note_prop_modified();
        }
        signal_after_change(i, s + 1, e + 1, (e - s) as i128)
    })?;
    Ok(Value::t())
}

/// Shared engine for `text-property-any' and `text-property-not-all':
/// (START END PROP VALUE &optional OBJECT) → first pos in [S,E) whose
/// PROP is (not-)eq VALUE.
fn text_prop_any(i: &mut Interp, a: &[Value], any: bool) -> EvalResult {
    // GNU Ftext_property_any validates in this order: OBJECT (string or
    // buffer-or-string), then START/END via CHECK_NUMBER_COERCE_MARKER
    // (integer-or-marker-p), then scans; PROP is just a plist key — any
    // atom works, no symbolp check.
    // GNU textprop.c: property keys are always symbols; a non-symbol
    // PROP simply never matches — `textget' does an eq scan that
    // finds nothing (no symbolp check).  So after the START/END
    // coercion below, a non-symbol PROP short-circuits to nil.
    let pid = match &a[2] {
        Value::Sym(s) => Some(*s),
        _ => None,
    };
    let want = a[3].clone();
    let coerce = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            // GNU CHECK_NUMBER_COERCE_MARKER yields the marker's
            // 1-based charpos; Marker.position is the 0-based offset.
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    if let Some(Value::Str(s)) = a.get(4) {
        let len = str_len(s) as i128;
        // GNU validate_interval_range: string positions are 0-based,
        // valid range [0, len]; out-of-range signals (START END) raw.
        let sv = coerce(i, &a[0])?;
        let ev = coerce(i, &a[1])?;
        if sv < 0 || sv > len || ev < 0 || ev > len {
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![a[0].clone(), a[1].clone()],
            ));
        }
        let st = sv;
        let en = ev;
        let ivs = i.str_props(s);
        let mut p = st;
        while p < en {
            let pl = str_plist_at(ivs, p as usize);
            // GNU `textget' is category-aware.
            let v = pid
                .and_then(|id| str_plist_get_cat(&pl, id, i))
                .unwrap_or(Value::Nil);
            if crate::lisp::builtins::eq_values(&v, &want) == any {
                return Ok(Value::Int(p));
            }
            p += 1;
        }
        return Ok(Value::Nil);
    }
    buf_or_string_ok(i, a.get(4))?;
    let b = match a.get(4) {
        Some(v) => buf_of(i, v)?,
        None => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len();
    let sv = coerce(i, &a[0])?;
    let ev = coerce(i, &a[1])?;
    let (lo, hi) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
    if sv < lo || sv > hi || ev < lo || ev > hi {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    // GNU validate_region orders START/END.
    let s = pos_idx(len, sv.min(ev));
    let e = pos_idx(len, sv.max(ev));
    let mut p = s;
    while p < e {
        let v = match pid {
            Some(id) => prop_at_bb(i, &bb, p, id),
            None => Value::Nil,
        };
        if crate::lisp::builtins::eq_values(&v, &want) == any {
            return Ok(Value::Int(p as i128 + 1));
        }
        p += 1;
    }
    Ok(Value::Nil)
}
/// interval in [S,E).  GNU dedups with memq; APPEND puts the new face
/// last, otherwise first.
pub(crate) fn f_add_face_text_property(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let face = a[2].clone();
    let append = a.get(3).map(|v| v.truthy()).unwrap_or(false);
    let combine = |old: &Value| -> Value {
        let mut items: Vec<Value> = match old {
            Value::Nil => Vec::new(),
            Value::Cons(_) => old.list_to_vec().unwrap_or_default(),
            v => vec![v.clone()],
        };
        let already = items
            .iter()
            .any(|x| crate::lisp::builtins::eq_values(x, &face));
        if !already {
            if append {
                items.push(face.clone());
            } else {
                items.insert(0, face.clone());
            }
        }
        match items.len() {
            0 => Value::Nil,
            1 => items.into_iter().next().unwrap(),
            _ => Value::list(items),
        }
    };
    let face_sym = Value::Sym(i.intern("face"));
    if let Some(Value::Str(s)) = a.get(4) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        str_pos_ok(i, &a[4], st.max(en), len)?;
        let (s0, e0) = (st.min(en), st.max(en));
        let mut ivs = std::mem::take(i.str_props_mut(&s));
        let ii: &Interp = i;
        let fid = ii.sym_id(&face_sym).unwrap_or(0);
        let fill = combine(&Value::Nil);
        iv_apply(
            &mut ivs,
            s0,
            e0,
            |pl| {
                // GNU `textget' is category-aware.
                let old = str_plist_get_cat(pl, fid, ii).unwrap_or(Value::Nil);
                let nv = combine(&old);
                if nv.is_nil() {
                    str_plist_remove(pl, fid, ii);
                } else {
                    str_plist_put(pl, &face_sym, &nv);
                }
            },
            if fill.is_nil() {
                None
            } else {
                Some(vec![face_sym.clone(), fill])
            },
        );
        i.set_str_props(&s, ivs);
        return Ok(Value::Nil);
    }
    // GNU validate_interval_range checks `buffer-or-string-p' on OBJECT
    // before looking at BEG/END.
    if let Some(v) = a.get(4) {
        if !v.is_nil() {
            return Err(i.wrong_type_mut("buffer-or-string-p", v));
        }
    }
    // GNU Fadd_face_text_property runs validate_interval_range on the
    // current buffer: BEG/END outside [begv..zv] signal
    // args-out-of-range (pos_idx clamps instead — wrong).
    want_range_in_range(i, &a[0], &a[1])?;
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let fid = want_sym(i, &face_sym)?;
    let cat = i.intern_soft("category");
    let (s, e) = (s.min(e), s.max(e));
    let b = cur(i);
    let bb = b.borrow_mut();
    // Run-wise: every stretch of [s,e) with one old face (and category)
    // gets a single combined entry.
    let (face_runs, cat_runs) = {
        let props = bb.text.props.borrow();
        (
            prop_runs(&props, s, e, fid),
            cat.map(|c| prop_runs(&props, s, e, c)).unwrap_or_default(),
        )
    };
    let mut cuts: Vec<usize> = vec![s, e];
    for (rs, re, _) in face_runs.iter().chain(cat_runs.iter()) {
        cuts.push(*rs);
        cuts.push(*re);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let at = |runs: &[(usize, usize, Value)], p: usize| {
        runs.iter()
            .find(|(rs, re, _)| *rs <= p && p < *re)
            .map(|r| r.2.clone())
            .unwrap_or(Value::Nil)
    };
    let mut segs: Vec<TextProp> = Vec::with_capacity(cuts.len());
    for w in cuts.windows(2) {
        let (p, q) = (w[0], w[1]);
        if p >= q {
            continue;
        }
        let mut old = at(&face_runs, p);
        // GNU `textget' is category-aware.
        if old.is_nil() {
            if let Value::Sym(cs) = at(&cat_runs, p) {
                old = i.get_prop(cs, fid);
            }
        }
        segs.push(TextProp {
            start: p,
            end: q,
            prop: fid,
            value: combine(&old),
        });
    }
    {
        let mut props = bb.text.props_write();
        clip_props(&mut props, s, e, |q| q == fid);
        props.extend(segs);
    }
    Ok(Value::Nil)
}

fn f_text_property_any(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    text_prop_any(i, &a, true)
}

fn f_text_property_not_all(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    text_prop_any(i, &a, false)
}

fn f_propertize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks the property-pair arity BEFORE the string arg:
    // (propertize 1.0 :k) → wrong-number-of-arguments, not stringp.
    if a.len() % 2 == 0 {
        let pname = i.intern("propertize");
        return Err(i.signal_data(
            crate::lisp::obarray::sym::WRONG_NUMBER_OF_ARGUMENTS,
            vec![Value::Sym(pname), Value::Int(a.len() as i128)],
        ));
    }
    match &a[0] {
        Value::Str(s) => {
            // GNU copies the string (with its intervals) and adds the
            // arg plist props over the whole length.
            let len = str_len(s);
            let ns = Rc::new(RefCell::new(s.borrow().clone()));
            let out = Value::Str(ns.clone());
            let mut ivs = i.str_props(s).to_vec();
            let pairs: Vec<(Value, Value)> = a[1..]
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| (c[0].clone(), c[1].clone()))
                .collect();
            if !pairs.is_empty() {
                // GNU conses the args into a reversed properties list,
                // then add_text_properties plputs it — net effect on a
                // fresh interval is the verbatim arg order, while
                // existing props get plput per reversed pair.
                let fill: Vec<Value> = pairs
                    .iter()
                    .flat_map(|(k, v)| [k.clone(), v.clone()])
                    .collect();
                let rpairs: Vec<(Value, Value)> = pairs.iter().rev().cloned().collect();
                let ii: &Interp = i;
                iv_apply(
                    &mut ivs,
                    0,
                    len,
                    |pl| {
                        for (k, v) in &rpairs {
                            if v.is_nil() {
                                if let Some(id) = ii.sym_id(k) {
                                    str_plist_remove(pl, id, ii);
                                }
                            } else {
                                str_plist_put(pl, k, v);
                            }
                        }
                    },
                    Some(fill),
                );
            }
            i.set_str_props(&ns, ivs);
            Ok(out)
        }
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

// ---------- undo ----------

/// Deconstruct a cons cell into (car, cdr), or None for atoms.
fn cons_parts(v: &Value) -> Option<(Value, Value)> {
    match v {
        Value::Cons(c) => {
            let cc = c.borrow();
            Some((cc.car.clone(), cc.cdr.clone()))
        }
        _ => None,
    }
}

/// True when the (BEG . END) or (STRING . POS) positions fall outside
/// the accessible portion of BUFFER (GNU: "outside visible portion").
fn undo_pos_oob(b: &Rc<RefCell<crate::buffer::Buffer>>, beg: i128, end: i128) -> bool {
    let bb = b.borrow();
    beg < bb.begv as i128 + 1 || end > bb.text_len() as i128 + 1
}

/// GNU marker-adjustment: marker position minus OFFSET (1-based).
fn undo_marker_adjust(m: &crate::lisp::value::MarkerRef, off: i128) {
    if m.borrow().buffer.is_some() {
        let np = m.borrow().position as i128 + 1 - off;
        m.borrow_mut().position = np.max(0) as usize;
    }
}

/// The error GNU's `primitive-undo' signals when an entry's positions
/// fall outside the accessible (visible) portion of the buffer.
fn undo_oob_err(i: &mut Interp) -> Flow {
    i.signal_data(
        sym::ERROR,
        vec![Value::string(
            "Changes to be undone are outside visible portion of buffer",
        )],
    )
}

/// GNU `time-equal-p' restricted to the shapes we store in `(t . TIME)'
/// undo entries (fixnum 0 for non-file buffers, or a Lisp timestamp).
fn undo_time_equal(i: &Interp, a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        _ => crate::lisp::builtins::equal_values(i, a, b),
    }
}

/// Apply one non-nil undo element (the `pcase' dispatch of GNU's
/// `primitive-undo').  Returns the remaining LIST after consuming any
/// (MARKER . ADJUSTMENT) records that follow a (TEXT . POS) entry.
fn undo_apply_one(
    i: &mut Interp,
    b: &Rc<RefCell<crate::buffer::Buffer>>,
    next: &Value,
    list: &mut Value,
) -> Result<(), Flow> {
    match next {
        // POSITION: `(goto-char next)'.
        Value::Int(p) => {
            let mut bb = b.borrow_mut();
            let idx = pos_idx(bb.text.len(), *p);
            bb.point = idx.max(bb.begv).min(bb.text_len());
        }
        Value::Cons(_) => {
            let (car, cdr) = cons_parts(next).unwrap();
            if matches!(car, Value::Sym(s) if s == crate::lisp::sym::T) {
                // (t . TIME): previous modtime record; if it matches the
                // visited file's time, mark the buffer unmodified.
                let vft = {
                    let bb = b.borrow();
                    if bb.file_modtime_ns < 0 {
                        Value::Int(-2 - bb.file_modtime_ns)
                    } else {
                        crate::lisp::builtins::misc::ns_to_lisp_time_i(i, bb.file_modtime_ns)
                    }
                };
                if undo_time_equal(i, &cdr, &vft) {
                    f_unlock_buffer(i, vec![].into())?;
                    f_set_buffer_modified_p(i, vec![Value::Nil].into())?;
                }
            } else if matches!(car, Value::Nil) {
                // (nil PROP VAL BEG . END): text property change.
                let (prop, val, tail) = {
                    let (p, r1) = cons_parts(&cdr).ok_or_else(|| undo_unrecognized(i, next))?;
                    let (v, r2) = cons_parts(&r1).ok_or_else(|| undo_unrecognized(i, next))?;
                    (p, v, r2)
                };
                let (beg, end) = cons_parts(&tail).ok_or_else(|| undo_unrecognized(i, next))?;
                let (Some(beg), Some(end)) = (beg.int(), end.int()) else {
                    return Err(undo_unrecognized(i, next));
                };
                if undo_pos_oob(b, beg, end) {
                    return Err(undo_oob_err(i));
                }
                f_put_text_property(i, vec![Value::Int(beg), Value::Int(end), prop, val].into())?;
            } else if matches!(car, Value::Sym(s) if s == i.intern("apply")) {
                // (apply . FUN-ARGS): function undo record.
                let currbuff = i.current_buffer;
                let fun_args = cdr.list_to_vec().map_err(|_| undo_unrecognized(i, next));
                match fun_args {
                    Ok(v) if v.first().and_then(|x| x.int()).is_some() => {
                        // Long format: (apply DELTA START END FUN . ARGS)
                        if v.len() < 4 {
                            return Err(undo_unrecognized(i, next));
                        }
                        let delta = v[0].int().unwrap();
                        let start = v[1].int().unwrap();
                        let end = v[2].int().unwrap();
                        let fun = v[3].clone();
                        let args = v[4..].to_vec();
                        if undo_pos_oob(b, start, end) {
                            return Err(undo_oob_err(i));
                        }
                        let sm = f_copy_marker(i, vec![Value::Int(start), Value::Nil].into())?;
                        let em = f_copy_marker(i, vec![Value::Int(end), Value::t()].into())?;
                        i.apply(&fun, args)?;
                        let (sp, ep) = match (&sm, &em) {
                            (Value::Marker(s), Value::Marker(e)) => {
                                (s.borrow().position as i128, e.borrow().position as i128)
                            }
                            _ => unreachable!(),
                        };
                        if sp + 1 != start || ep + 1 != end + delta {
                            return Err(i.signal_data(
                                sym::ERROR,
                                vec![Value::string(
                                    "Changes undone by function are different from the announced ones",
                                )],
                            ));
                        }
                        f_set_marker(i, vec![sm, Value::Nil].into())?;
                        f_set_marker(i, vec![em, Value::Nil].into())?;
                    }
                    Ok(v) => {
                        // Short format: (apply FUN . ARGS)
                        let (fun, args) = match v.split_first() {
                            Some((f, r)) => (f.clone(), r.to_vec()),
                            None => return Err(undo_unrecognized(i, next)),
                        };
                        i.apply(&fun, args)?;
                    }
                    Err(e) => return Err(e),
                }
                if i.current_buffer != currbuff {
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Undo function switched buffer")],
                    ));
                }
            } else if let (Some(beg), Some(end)) = (car.int(), cdr.int()) {
                // (BEG . END): range was inserted — delete it.
                if undo_pos_oob(b, beg, end) {
                    return Err(undo_oob_err(i));
                }
                let s = (beg - 1).max(0) as usize;
                b.borrow_mut().set_point(s);
                chg_delete(i, s, (end - 1).max(0) as usize)?;
            } else if let Value::Str(text) = &car {
                // (STRING . POS): STRING was deleted — reinsert it.
                let Some(pos) = cdr.int() else {
                    return Err(undo_unrecognized(i, next));
                };
                let apos = pos.unsigned_abs() as usize;
                if undo_pos_oob(b, apos as i128, apos as i128) {
                    return Err(undo_oob_err(i));
                }
                // Consume following (MARKER . ADJUSTMENT) entries whose
                // marker still sits at APOS in this buffer.
                let mut valid_adjs = Vec::new();
                loop {
                    let madj = match cons_parts(list) {
                        Some((e, rest)) => match cons_parts(&e) {
                            Some((Value::Marker(_), cdr)) if cdr.int().is_some() => (e, rest),
                            _ => break,
                        },
                        _ => break,
                    };
                    *list = madj.1;
                    if let Some((Value::Marker(m), _)) = cons_parts(&madj.0) {
                        let mm = m.borrow();
                        if mm.buffer == Some(b.borrow().id)
                            && apos as i128 == mm.position as i128 + 1
                        {
                            valid_adjs.push(madj.0.clone());
                        }
                    }
                }
                {
                    let s = text.borrow().clone();
                    if pos < 0 {
                        b.borrow_mut().set_point((-pos - 1).max(0) as usize);
                        chg_insert_pt(i, &s, false)?;
                    } else {
                        b.borrow_mut().set_point((pos - 1).max(0) as usize);
                        chg_insert_pt(i, &s, false)?;
                        b.borrow_mut().set_point((pos - 1).max(0) as usize);
                    }
                }
                // Apply validated marker adjustments.
                for adj in valid_adjs {
                    if let Some((Value::Marker(m), cdr)) = cons_parts(&adj) {
                        if let Some(off) = cdr.int() {
                            undo_marker_adjust(&m, off);
                        }
                    }
                }
            } else if let Value::Marker(m) = &car {
                // (MARKER . OFFSET) with no matching (TEXT . POS).
                // GNU routes this through `warn' (warnings.el), so it
                // lands in *Warnings* as well as the echo area.
                let warn_sym = i.intern("warn");
                let _ = i.apply(
                    &Value::Sym(warn_sym),
                    vec![
                        Value::string(
                            "Encountered %S entry in undo list with no matching (TEXT . POS) entry",
                        ),
                        next.clone(),
                    ],
                );
                if let Some(off) = cdr.int() {
                    undo_marker_adjust(m, off);
                }
            } else {
                return Err(undo_unrecognized(i, next));
            }
        }
        _ => return Err(undo_unrecognized(i, next)),
    }
    Ok(())
}

fn undo_unrecognized(i: &mut Interp, next: &Value) -> Flow {
    i.signal_data(
        sym::ERROR,
        vec![Value::string(format!(
            "Unrecognized entry in undo list {}",
            i.prin1_to_string(next)
        ))],
    )
}

fn f_primitive_undo(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (primitive-undo N LIST) — GNU defun (simple.el): N counts undo
    // boundaries crossed, not entries.  Returns the remaining list.
    // GNU checks N through `(> arg 0)' each round: non-number/markers
    // signal `number-or-marker-p', dead markers "Marker does not point
    // anywhere", and floats decrement toward 0 via `1-'.
    let mut arg = match &a[0] {
        Value::Int(n) => *n as f64,
        Value::Float(f) => **f,
        Value::Marker(m) => match m.borrow().buffer {
            Some(_) => m.borrow().position as f64 + 1.0,
            None => return Err(i.error("Marker does not point anywhere")),
        },
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    let mut list = a[1].clone();
    let b = cur(i);
    let oldlist = b.borrow().undo_list();
    let mut did_apply = false;
    while arg > 0.0 {
        // Inner loop: apply entries until a nil boundary or list end.
        loop {
            let (next, rest) = match &list {
                Value::Cons(_) => cons_parts(&list).unwrap(),
                // GNU `pop' on a non-nil atom signals like `car'.
                Value::Nil => break,
                _ => return Err(i.wrong_type_mut("listp", &list)),
            };
            list = rest;
            if matches!(next, Value::Nil) {
                break;
            }
            undo_apply_one(i, &b, &next, &mut list)?;
            did_apply = true;
        }
        arg -= 1.0;
    }
    if did_apply {
        // If the applied entries produced no new undo records (e.g.
        // recording disabled), push an `(apply cdr nil)' marker so
        // `undo' can still tell a change happened.
        let mut bb = b.borrow_mut();
        if crate::lisp::builtins::eq_values(&oldlist, &bb.undo_list()) {
            let entry = Value::list(vec![
                Value::Sym(i.intern("apply")),
                Value::Sym(i.intern("cdr")),
                Value::Nil,
            ]);
            bb.push_undo(entry);
        }
    }
    Ok(list)
}

fn f_delete_blank_lines(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // Emacs: on a blank line, delete all contiguous blank lines
    // (leaving just one when there are several). On a nonblank line,
    // delete all blank lines following it.
    let _ = i;
    let b = cur(i);
    let bb = b.borrow_mut();
    let len = bb.text.len();
    let blank_at = |bb: &crate::buffer::Buffer, start: usize, end: usize| {
        bb.text.substring(start, end).trim().is_empty()
    };
    // (start, end) of the line containing char-pos p, with end
    // including the trailing newline when present.
    let line_span = |bb: &crate::buffer::Buffer, p: usize| {
        let l = bb.text.line_of_pos(p);
        let s = bb.text.line_start(l);
        let mut e = bb.text.line_end(s);
        if e < len {
            e += 1;
        }
        (s, e)
    };
    let p = bb.point().min(len);
    let (ls, le) = line_span(&bb, p);
    let mut s = ls;
    let mut e = le;
    if blank_at(&bb, ls, le) {
        // Extend over the whole contiguous blank region.
        while s > 0 {
            let (ps, pe) = line_span(&bb, s - 1);
            if !blank_at(&bb, ps, pe) {
                break;
            }
            s = ps;
        }
        while e < len {
            let (ns, ne) = line_span(&bb, e);
            if !blank_at(&bb, ns, ne) {
                break;
            }
            e = ne;
        }
        // Keep one blank line if the region spans several: delete
        // from the end of the first blank line onward.
        let (fs, fe) = line_span(&bb, s);
        if e - s > fe - fs {
            s = fe;
        }
    } else {
        // Delete following blank lines only (after the current
        // line's terminating newline).
        s = le;
        while e < len {
            let (ns, ne) = line_span(&bb, e);
            if !blank_at(&bb, ns, ne) {
                break;
            }
            e = ne;
        }
        if e == le {
            return Ok(Value::Nil);
        }
    }
    drop(bb);
    chg_delete(i, s, e)?;
    let np = s.min(b.borrow().text.len());
    b.borrow_mut().set_point(np);
    Ok(Value::Nil)
}

// ---------- misc ----------

fn f_gap_position(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU GPT — the gap's
    // leading edge, 1-based.  Moves only on insert/delete (unlike point).
    let buf = cur(i);
    let b = buf.borrow();
    Ok(Value::Int(b.text.gap_position() as i128 + 1))
}
fn f_gap_size(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU GAP_SIZE — the
    // unused gap region; grows on insert, never shrinks.
    let buf2 = cur(i);
    let b2 = buf2.borrow();
    Ok(Value::Int(b2.text.gap_size() as i128))
}
/// UTF-8 byte length of a buffer char: eight-bit proxy chars occupy
/// GNU's 2-byte internal encoding; everything else is its UTF-8 len.
fn buf_char_byte_len(c: char) -> usize {
    match crate::lisp::value::eight_bit_byte(c) {
        Some(_) => 2,
        None => c.len_utf8(),
    }
}

fn f_position_bytes(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: byte position corresponding to char position POSITION —
    // 1 + byte length of the text before it; nil when POSITION is not
    // a valid char position (accepts markers).
    let pos = match &a[0] {
        Value::Int(n) => *n,
        Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
        Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    };
    let sym = i.intern("enable-multibyte-characters");
    let unibyte = !i.symbol_value(sym).truthy();
    let text = cur(i).borrow().text.text();
    let chars: Vec<char> = text.chars().collect();
    if pos < 1 || pos as usize > chars.len() + 1 {
        return Ok(Value::Nil);
    }
    let n = pos as usize - 1;
    if unibyte {
        return Ok(Value::Int(pos));
    }
    let bytes: usize = chars[..n].iter().map(|c| buf_char_byte_len(*c)).sum();
    Ok(Value::Int(1 + bytes as i128))
}
fn f_byte_to_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: char position holding byte position BYTEPOS; a byte inside
    // a multibyte char yields that char's position; nil out of range.
    let n = match &a[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let sym = i.intern("enable-multibyte-characters");
    let unibyte = !i.symbol_value(sym).truthy();
    let text = cur(i).borrow().text.text();
    if unibyte {
        let len = text.chars().count() as i128;
        return Ok(if n >= 1 && n <= len + 1 {
            a[0].clone()
        } else {
            Value::Nil
        });
    }
    if n < 1 {
        return Ok(Value::Nil);
    }
    let mut bytepos = 1usize;
    for (idx, c) in text.chars().enumerate() {
        let w = buf_char_byte_len(c);
        if n as usize <= bytepos + w - 1 {
            return Ok(Value::Int(idx as i128 + 1));
        }
        bytepos += w;
    }
    if n as usize == bytepos {
        // One past the last byte == position after the last char.
        return Ok(Value::Int(text.chars().count() as i128 + 1));
    }
    Ok(Value::Nil)
}
fn f_max_char(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: optional UNICODE arg — nil/omitted →
    // MAX_CHAR (0x3fffff); non-nil → MAX_UNICODE_CHAR (0x10ffff).
    Ok(Value::Int(
        if a.first().map(|v| v.truthy()).unwrap_or(false) {
            0x10ffff
        } else {
            0x3fffff
        },
    ))
}
fn f_barf_if_buffer_read_only(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: optional POSITION is CHECK_FIXNUM'd (a marker whose
    // buffer was deleted reports `fixnump' too), then the read-only check.
    match a.get(0) {
        None | Some(Value::Nil) => {}
        Some(Value::Marker(m)) => {
            if m.borrow().buffer.is_none() {
                return Err(i.wrong_type_mut("fixnump", &a[0].clone()));
            }
        }
        Some(v) => {
            crate::lisp::builtins::want_fixnum(i, v)?;
        }
    }
    check_writable(i)?;
    Ok(Value::Nil)
}

// `install` entry called by Interp::new.
pub fn install_primitives(i: &mut Interp) {
    for s in SUBRS.iter().chain(crate::buffer::extra::SUBRS.iter()) {
        let id = i.intern(s.name);
        i.fset(id, Value::Subr(s));
    }
}
