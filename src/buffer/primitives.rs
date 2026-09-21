//! Buffer primitives: the DEFUNs operating on buffers, point, mark,
//! markers, narrowing, buffer-local variables, search/match, text
//! properties, and undo.
//!
//! Position convention: Emacs positions are 1-based; internally the
//! `Buffer` uses 0-based char indices. `pt` = `bb.point + 1`.

use crate::buffer::{Buffer, TextProp};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::eval::MatchData;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Marker, Subr, Value};
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
        1,
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
        0,
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
        1,
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
        0,
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
    S!("set-default-toplevel-value", 2, 2, f_set_default, ""),
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
        "line-number-at-pos",
        0,
        2,
        f_line_number_at_pos,
        "Line number of POSITION."
    ),
    S!(
        "count-lines",
        2,
        2,
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
    S!(
        "forward-sexp",
        0,
        1,
        f_forward_sexp,
        "Move across a balanced expression."
    ),
    S!(
        "backward-sexp",
        0,
        1,
        f_backward_sexp,
        "Move back across a balanced expression."
    ),
    S!("scan-lists", 3, 3, f_scan_lists, "Scan lists."),
    S!(
        "scan-sexps",
        2,
        2,
        f_scan_sexps,
        "Scan COUNT sexps from FROM."
    ),
    S!("down-list", 0, 1, f_down_list, "Move down into a list."),
    S!("up-list", 0, 1, f_up_list, "Move out of a list."),
    S!("forward-list", 0, 1, f_forward_list, "Move across a list."),
    S!(
        "backward-list",
        0,
        1,
        f_backward_list,
        "Move back across a list."
    ),
    S!(
        "backward-up-list",
        0,
        1,
        f_backward_up_list,
        "Move up out of a list."
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
    S!("insert-and-inherit", many 0, f_insert, "Insert with inherited props."),
    S!("insert-before-markers", many 0, f_insert_before_markers, "Insert before markers."),
    S!("insert-before-markers-and-inherit", many 0, f_insert_before_markers, ""),
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
        0,
        1,
        f_open_line,
        "Insert newline without moving point."
    ),
    S!(
        "delete-char",
        0,
        2,
        f_delete_char,
        "Delete N chars after point."
    ),
    S!(
        "delete-backward-char",
        0,
        2,
        f_delete_backward_char,
        "Delete N chars before point."
    ),
    S!(
        "backward-delete-char-untabify",
        1,
        2,
        f_backward_delete_char_untabify,
        "Delete N chars backward, untabifying."
    ),
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
        2,
        f_kill_region,
        "Kill text between START and END."
    ),
    S!(
        "append-next-kill",
        0,
        0,
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
        f_buffer_substring,
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
    S!("current-word", 0, 3, f_current_word, "Word at point."),
    S!(
        "thing-at-point",
        1,
        2,
        f_thing_at_point,
        "Thing at point (word/symbol/line)."
    ),
    S!(
        "bounds-of-thing-at-point",
        1,
        2,
        f_bounds_of_thing_at_point,
        "Bounds of thing at point."
    ),
    S!("word-at-point", 0, 0, f_word_at_point, "Word at point."),
    S!(
        "symbol-at-point",
        0,
        0,
        f_symbol_at_point,
        "Symbol at point."
    ),
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
    S!("activate-mark", 0, 0, f_activate_mark, "Activate the mark."),
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
    // --- markers ---
    S!("markerp", 1, 1, f_markerp, "t if OBJECT is a marker."),
    S!(
        "make-marker",
        0,
        0,
        f_make_marker,
        "Create a marker pointing nowhere."
    ),
    S!("copy-marker", 1, 2, f_copy_marker, "Copy MARKER."),
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
    S!("looking-at-p", 1, 1, f_looking_at, "Predicate version."),
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
    S!("search-forward-regexp", 1, 4, f_re_search_forward, ""),
    S!("search-backward-regexp", 1, 4, f_re_search_backward, ""),
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
    S!("posix-looking-at", 1, 1, f_looking_at, ""),
    S!("posix-string-match", 2, 3, f_string_match, ""),
    S!("posix-search-forward", 1, 4, f_re_search_forward, ""),
    S!("posix-search-backward", 1, 4, f_re_search_backward, ""),
    S!("word-search-forward", 1, 4, f_search_forward, ""),
    S!("word-search-backward", 1, 4, f_search_backward, ""),
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
    S!("get-char-property", 2, 3, f_get_text_property, ""),
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
        f_next_single_property_change,
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
        f_prev_single_property_change,
        ""
    ),
    S!("propertize", many 1, f_propertize, "Return STRING (props ignored)."),
    S!("text-props-copy", 1, 1, f_identity, ""),
    S!("object-intervals", 0, 0, f_nil, ""),
    // --- undo ---
    S!("undo", 0, 1, f_undo, "Undo some changes."),
    S!(
        "primitive-undo",
        2,
        2,
        f_primitive_undo,
        "Apply undo entries."
    ),
    S!("undo-start", 0, 0, f_undo_start, ""),
    S!("undo-more", 1, 1, f_undo, ""),
    S!("undo-auto-amalgamate", 0, 0, f_noop, ""),
    S!("cancel-change-group", 0, 0, f_noop, ""),
    S!("activate-change-group", 0, 0, f_noop, ""),
    S!("accept-change-group", 1, 1, f_accept_change_group, ""),
    S!("handle-change-group", 0, 0, f_noop, ""),
    S!("undo-outer-limit-truncate", 0, 0, f_noop, ""),
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
    S!("max-char", 0, 0, f_max_char, "Max character code."),
    S!(
        "barf-if-buffer-read-only",
        0,
        2,
        f_barf_if_buffer_read_only,
        "Signal if read-only."
    ),
    S!("verify-visited-file-modtime", 0, 1, f_t, ""),
    S!("clear-visited-file-modtime", 0, 0, f_nil, ""),
    S!("visited-file-modtime", 0, 0, f_zero, ""),
    S!("set-visited-file-modtime", 0, 1, f_nil, ""),
    S!("lock-buffer", 0, 1, f_nil, ""),
    S!("unlock-buffer", 0, 0, f_nil, ""),
    S!("file-locked-p", 1, 1, f_nil, ""),
    S!("ask-user-about-lock", many 0, f_nil, ""),
    S!("internal-set-alist", 0, 0, f_nil, ""),
    S!(
        "compare-buffer-substrings",
        6,
        6,
        f_compare_buffer_substrings,
        "Compare two buffer substrings."
    ),
];

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

/// GNU: 0 when equal, else +/-(1 + number of matching leading chars).
/// Honors `case-fold-search`.
fn f_compare_buffer_substrings(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b1 = buf_of(i, &a[0])?;
    let b2 = buf_of(i, &a[3])?;
    let (s1, s2) = {
        let bb1 = b1.borrow();
        let bb2 = b2.borrow();
        let (b, e) = (
            pos_idx(bb1.text.len(), want_int(i, &a[1])?),
            pos_idx(bb1.text.len(), want_int(i, &a[2])?),
        );
        let (b2x, e2x) = (
            pos_idx(bb2.text.len(), want_int(i, &a[4])?),
            pos_idx(bb2.text.len(), want_int(i, &a[5])?),
        );
        (
            bb1.text.substring(b.min(e), b.max(e)),
            bb2.text.substring(b2x.min(e2x), b2x.max(e2x)),
        )
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
fn f_t(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}
fn f_zero(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0))
}
fn f_identity(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a.into_iter().next().unwrap_or(Value::Nil))
}
fn f_accept_change_group(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU validates the handle is a change-group cons; accept = drop it.
    if !matches!(a[0], Value::Cons(_)) && !matches!(a[0], Value::Nil) {
        return Err(i.wrong_type_mut("listp", &a[0]));
    }
    Ok(Value::Nil)
}

fn f_noop(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}
fn f_progn_raw(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    i.eval_progn(&a.into_iter().next().unwrap_or(Value::Nil))
}
fn f_second_form_raw(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (combine-change-calls BEG END . BODY) — eval BODY.
    let args = a.into_iter().next().unwrap_or(Value::Nil);
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

pub(crate) fn want_sym(i: &mut Interp, v: &Value) -> Result<u32, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
}

/// Resolve optional buffer arg (Value::Buffer/Str/nil) to a shared ref.
pub(crate) fn buf_of(
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
        other => Err(i.wrong_type_mut("bufferp", other)),
    }
}

pub(crate) fn cur(i: &Interp) -> Rc<RefCell<crate::buffer::Buffer>> {
    i.current_buffer_ref().unwrap()
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

/// 0-based clamped index from an Emacs position arg.
pub(crate) fn pos_idx(len_or_zv: usize, p: i128) -> usize {
    (p.max(1) as usize - 1).min(len_or_zv)
}

pub(crate) fn marker_value(m: Rc<RefCell<Marker>>) -> Value {
    Value::Marker(m)
}

// ---------- buffer objects ----------

fn f_current_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(i.buffer_value(i.current_buffer).unwrap_or(Value::Nil))
}

fn f_bufferp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Buffer(_))))
}

fn f_buffer_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    Ok(Value::string(b.borrow().name.clone()))
}

fn f_get_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_get_buffer_create(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            let name = s.borrow().clone();
            let id = match i.buffers.by_name(&name) {
                Some(id) => id,
                None => i.buffers.create(&name),
            };
            Ok(i.buffer_value(id).unwrap_or(Value::Nil))
        }
        Value::Buffer(b) => Ok(Value::Buffer(b.clone())),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

fn f_generate_new_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let id = i.buffers.create(&name);
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

fn f_generate_new_buffer_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    Ok(Value::string(i.buffers.unique_name(&name)))
}

fn f_buffer_live_p(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Buffer(_))))
}

/// Kill `id` and repair `current_buffer`: GNU always has a live
/// buffer, so killing the last one yields a fresh *scratch*.
pub(crate) fn kill_buffer_keep_current(i: &mut Interp, id: usize) -> bool {
    if !i.buffers.kill(id) {
        return false;
    }
    if i.current_buffer == id {
        let next = i
            .buffers
            .other(id)
            .or_else(|| i.buffers.list().first().copied())
            .unwrap_or_else(|| i.buffers.create("*scratch*"));
        i.current_buffer = next;
    }
    true
}

fn f_kill_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = match a.get(0) {
        None => i.current_buffer,
        Some(v) => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&v))))?,
    };
    Ok(Value::from_bool(kill_buffer_keep_current(i, id)))
}

fn f_buffer_list(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let ids = i.buffers.list();
    Ok(Value::list(
        ids.iter().filter_map(|id| i.buffer_value(*id)).collect(),
    ))
}

fn f_other_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let exclude = a
        .get(0)
        .and_then(|v| i.buffer_id_of(v))
        .unwrap_or(i.current_buffer);
    match i.buffers.other(exclude) {
        Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}

fn f_set_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[0]))))?;
    i.set_current_buffer(id);
    Ok(a[0].clone())
}

fn f_rename_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = match a.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(v) if v.is_nil() => {
            // rename to file name or keep
            let b = cur(i);
            let bb = b.borrow();
            bb.name.clone()
        }
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
        None => cur(i).borrow().name.clone(),
    };
    let unique = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let actual = if unique {
        i.buffers.rename(i.current_buffer, &name)
    } else {
        if i.buffers.by_name(&name).is_some() {
            return Err(i.error(&format!("Buffer name \"{}\" is in use", name)));
        }
        i.buffers.rename(i.current_buffer, &name)
    };
    Ok(Value::string(actual))
}

fn f_bury_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Move to end of order — our `touch` moves to front; implement bury
    // by reordering BufferSet::order? We don't have a public API; emulate
    // by killing nothing and moving to end via touch-in-reverse is not
    // possible — add order manipulation via a small method on BufferSet.
    let id = match a.get(0) {
        Some(v) if v.truthy() => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&v))))?,
        _ => i.current_buffer,
    };
    i.buffers.bury(id);
    Ok(Value::Nil)
}

fn f_bury_buffer_internal(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let id = i
        .buffer_id_of(&a[0])
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&a[0]))))?;
    i.buffers.bury(id);
    Ok(Value::Nil)
}

fn f_unbury_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let ids = i.buffers.list();
    match ids.last() {
        Some(id) => Ok(i.buffer_value(*id).unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}

fn f_buffer_size(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    // Emacs: ZV - BEGV (accessible portion under narrowing).
    let bb = b.borrow();
    Ok(Value::Int(
        bb.zv.saturating_sub(bb.begv).min(bb.size()) as i128
    ))
}

fn f_buffer_modified_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    Ok(Value::from_bool(b.borrow().modified))
}

pub(crate) fn f_set_buffer_modified_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let flag = a[0].truthy();
    cur(i).borrow_mut().modified = flag;
    Ok(a[0].clone())
}

fn f_not_modified(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let flag = arg(&a, 0).truthy();
    cur(i).borrow_mut().modified = flag;
    Ok(Value::Nil)
}

fn f_buffer_file_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    match &b.borrow().file_name {
        Some(f) => Ok(Value::string(f.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_buffer_base_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let base = b.borrow().base_buffer;
    Ok(base.and_then(|id| i.buffer_value(id)).unwrap_or(Value::Nil))
}

/// (make-indirect-buffer BASE-BUFFER NAME &optional CLONE
/// INHIBIT-BUFFER-HOOKS) — create a buffer sharing BASE-BUFFER's text.
/// The name must not already be in use; CLONE copies point, mark,
/// narrowing, and locals from the base.
fn f_make_indirect_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let base_id = match &a[0] {
        Value::Buffer(b) => b.borrow().id,
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    if !i
        .buffers
        .get(base_id)
        .map(|b| b.borrow().live)
        .unwrap_or(false)
    {
        return Err(i.error("Selecting deleted buffer"));
    }
    let name = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    if i.buffers.by_name(&name).is_some() {
        return Err(i.error("Buffer name is already in use"));
    }
    let clone = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let id = i.buffers.create_exact(&name);
    {
        let base = i.buffers.get(base_id).unwrap();
        let bb = base.borrow();
        let nb = i.buffers.get(id).unwrap();
        let mut n = nb.borrow_mut();
        n.text = bb.text.clone();
        n.base_buffer = Some(base_id);
        if clone {
            n.point = bb.point;
            n.mark = bb.mark;
            n.mark_active = bb.mark_active;
            n.begv = bb.begv;
            n.zv = bb.zv;
            n.locals = bb.locals.clone();
            n.file_name = bb.file_name.clone();
        }
    }
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

fn f_buffer_local_variables(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let pairs: Vec<Value> = {
        let bb = b.borrow();
        bb.locals
            .iter()
            .map(|(s, v)| Value::cons(i.sym(*s), v.clone()))
            .collect()
    };
    Ok(Value::list(pairs))
}

fn f_buffer_local_value(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let b = buf_of(i, &a[1])?;
    if let Some(v) = b.borrow().locals.get(&sid) {
        return Ok(v.clone());
    }
    Ok(i.obarray.symbol(sid).value.clone())
}

fn f_make_local_variable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if !bb.locals.contains_key(&sid) {
        // The local binding starts with the default value, or void
        // (the unbound sentinel) when the variable is unbound.
        bb.locals.insert(sid, i.obarray.symbol(sid).value.clone());
    }
    Ok(a[0].clone())
}

fn f_make_variable_buffer_local(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    i.obarray.symbol_mut(sid).make_local_if_set = true;
    Ok(a[0].clone())
}

fn f_kill_local_variable(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    cur(i).borrow_mut().locals.remove(&sid);
    Ok(a[0].clone())
}

fn f_kill_all_local_variables(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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
    let keep_set: std::collections::HashSet<u32> = keep.into_iter().collect();
    bb.locals.retain(|s, _| keep_set.contains(s));
    Ok(Value::Nil)
}

fn f_local_variable_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let b = buf_of(i, &arg(&a, 1))?;
    Ok(Value::from_bool(b.borrow().locals.contains_key(&sid)))
}

fn f_local_variable_if_set_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let auto = i.obarray.symbol(sid).make_local_if_set;
    if auto {
        return Ok(Value::t());
    }
    let b = buf_of(i, &arg(&a, 1))?;
    Ok(Value::from_bool(b.borrow().locals.contains_key(&sid)))
}

fn f_default_value(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    Ok(i.obarray.symbol(sid).value.clone())
}

fn f_set_default(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    i.set_symbol_default(sid, a[1].clone())?;
    Ok(a[1].clone())
}

fn f_default_boundp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    Ok(Value::from_bool(!matches!(
        i.obarray.symbol(sid).value,
        Value::Sym(s) if s == sym::UNBOUND
    )))
}

// ---------- case tables ----------

fn is_case_table(i: &Interp, v: &Value) -> bool {
    // A case table is a char-table whose subtype is `case-table'.
    crate::lisp::builtins::misc::is_char_table(i, v)
        && matches!(
            v,
            Value::Record(r)
                if matches!(r.borrow().get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "case-table")
        )
}

fn want_case_table(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    if is_case_table(i, v) {
        Ok(())
    } else {
        Err(i.wrong_type_mut("case-table-p", v))
    }
}

fn f_current_case_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let local = i
        .buffers
        .get(i.current_buffer)
        .and_then(|b| b.try_borrow().ok().map(|bb| bb.case_table.clone()))
        .flatten();
    match local {
        Some(t) => Ok(t),
        None => Ok(i.standard_case_table()),
    }
}

fn f_standard_case_table(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(i.standard_case_table())
}

fn f_case_table_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(is_case_table(i, &a[0])))
}

fn f_set_case_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_case_table(i, &a[0])?;
    if let Some(b) = i.buffers.get(i.current_buffer) {
        if let Ok(mut bb) = b.try_borrow_mut() {
            bb.case_table = Some(a[0].clone());
        }
    }
    Ok(a[0].clone())
}

fn f_set_standard_case_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    want_case_table(i, &a[0])?;
    i.standard_case_table = Some(a[0].clone());
    Ok(a[0].clone())
}

fn f_buffer_disable_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    bb.undo_enabled = false;
    bb.undo.clear();
    Ok(Value::Nil)
}

fn f_buffer_enable_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    bb.undo_enabled = true;
    Ok(Value::Nil)
}

// ---------- point & motion ----------

fn f_point(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().point() as i128 + 1))
}

fn f_point_min(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().begv as i128 + 1))
}

fn f_point_max(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().text_len() as i128 + 1))
}

fn f_goto_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => {
            let (pos, buf) = {
                let mm = m.borrow();
                (mm.position, mm.buffer)
            };
            if let Some(bid) = buf {
                i.set_current_buffer(bid);
            }
            cur(i).borrow_mut().set_point(pos);
            Ok(Value::Int(pos as i128 + 1))
        }
        v => {
            let p = want_int(i, v)?;
            let b = cur(i);
            let mut bb = b.borrow_mut();
            let idx = pos_idx(bb.text.len(), p);
            // Emacs clamps to the accessible region.
            let idx = idx.max(bb.begv).min(bb.text_len());
            bb.point = idx;
            Ok(Value::Int(idx as i128 + 1))
        }
    }
}

fn f_forward_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
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

fn f_backward_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    f_forward_char(i, vec![Value::Int(-n)])
}

fn f_forward_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if n >= 0 {
        for _ in 0..n {
            let mut p = bb.point();
            let len = bb.text_len();
            // skip non-word, then word
            while p < len && !is_word(bb.text.char_at(p)) {
                p += 1;
            }
            while p < len && is_word(bb.text.char_at(p)) {
                p += 1;
            }
            bb.set_point(p);
        }
    } else {
        for _ in 0..-n {
            let mut p = bb.point();
            while p > bb.begv && !is_word(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            while p > bb.begv && is_word(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            bb.set_point(p);
        }
    }
    Ok(Value::Nil)
}

fn f_backward_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    f_forward_word(i, vec![Value::Int(-n)])
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn f_forward_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
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

fn f_beginning_of_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Emacs: (beginning-of-line N) = forward-line(N-1) then BOL.
    // N <= 0 moves to the BOL of an earlier line (N=0 → previous line).
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n - 1).max(0) as usize;
    let p = bb.text.line_start(target).max(bb.begv);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_end_of_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: end-of-line N = forward-line(N-1) then end of line; for
    // N <= 0, if the backward move cannot complete, point stays at the
    // first line's BOL (no end-of-line).
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
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

fn f_bobp(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(bb.point() <= bb.begv))
}

fn f_eobp(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(bb.point() >= bb.text_len()))
}

fn f_bolp(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    Ok(Value::from_bool(
        p == 0 || bb.text.char_at(p.saturating_sub(1)) == '\n' || p <= bb.begv,
    ))
}

fn f_eolp(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    Ok(Value::from_bool(
        p >= bb.text_len() || bb.text.char_at(p) == '\n',
    ))
}

fn new_marker_at(i: &mut Interp, buf: usize, pos: usize) -> Value {
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

fn f_point_marker(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_point_min_marker(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.begv)
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_point_max_marker(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.text_len())
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_char_after(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(Value::Marker(m)) => m.borrow().position,
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    if p >= bb.text_len() || p >= bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(bb.text.char_at(p) as i128))
}

fn f_char_before(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(Value::Marker(m)) => m.borrow().position,
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    if p == 0 || p > bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(bb.text.char_at(p - 1) as i128))
}

fn f_following_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    if p >= bb.text_len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(bb.text.char_at(p) as i128))
}

fn f_preceding_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    if p == 0 {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(bb.text.char_at(p - 1) as i128))
}

fn f_pos_bol(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    let line = bb.text.line_of_pos(p);
    Ok(Value::Int(bb.text.line_start(line) as i128 + 1))
}

fn f_pos_eol(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    Ok(Value::Int(bb.text.line_end(p) as i128 + 1))
}

fn f_line_beginning_position(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let saved = cur(i).borrow().point();
    let r = f_beginning_of_line(i, vec![a.get(0).cloned().unwrap_or(Value::Int(1))]);
    let p = cur(i).borrow().point();
    cur(i).borrow_mut().set_point(saved);
    r?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_line_end_position(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let saved = cur(i).borrow().point();
    let r = f_end_of_line(i, vec![a.get(0).cloned().unwrap_or(Value::Int(1))]);
    let p = cur(i).borrow().point();
    cur(i).borrow_mut().set_point(saved);
    r?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_line_number_at_pos(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    // Emacs counts lines in the whole buffer unless narrowed-absolute.
    Ok(Value::Int(
        bb.text.line_of_pos(p.min(bb.text.len())) as i128 + 1,
    ))
}

fn f_count_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let (s_arg, e_arg) = (want_int(i, &a[0])?, want_int(i, &a[1])?);
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
    Ok(Value::Int(n))
}

fn f_current_column(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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
            c if (c as u32) < 0x20 || c == '\x7f' => col += 2,
            c => {
                col += unicode_width::UnicodeWidthChar::width(c)
                    .unwrap_or(1)
                    .max(1) as i128
            }
        }
        k += 1;
    }
    Ok(Value::Int(col))
}

fn f_move_to_column(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let goal = want_int(i, &a[0])?.max(0);
    let force = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let ls = bb.text.line_start(bb.text.line_of_pos(bb.point()));
    let le = bb.text.line_end(bb.point());
    let mut col = 0i128;
    let mut k = ls;
    while k < le && col < goal {
        match bb.text.char_at(k) {
            '\t' => col = (col / tab_width + 1) * tab_width,
            c if (c as u32) < 0x20 || c == '\x7f' => col += 2,
            c => {
                col += unicode_width::UnicodeWidthChar::width(c)
                    .unwrap_or(1)
                    .max(1) as i128
            }
        }
        k += 1;
    }
    // GNU lands on the first char boundary where col >= goal (a char is
    // never split), then returns the column actually reached.
    if col < goal && force {
        // Extend with spaces.
        let pad = (goal - col) as usize;
        let s: String = " ".repeat(pad);
        let at = bb.text.line_end(bb.point());
        bb.text.insert(at, &s);
        bb.zv += pad;
        bb.set_point(at + pad);
        Ok(Value::Int(goal))
    } else {
        bb.set_point(k);
        Ok(Value::Int(col))
    }
}

fn f_forward_comment(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let count = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text_len();
    let mut p = bb.point();
    let mut comments = 0i128;
    // GNU drives this off the syntax table: `<` starts a comment, `>` ends
    // it.  The standard syntax table has no comment characters at all, so
    // `forward-comment' returns nil in fundamental-mode buffers — callers
    // (e.g. `comment-forward') then fall back to regexps.
    if count >= 0 {
        loop {
            while p < len && crate::lisp::regexp::syntax_code(bb.text.char_at(p)) == b' ' {
                p += 1;
            }
            if p < len && crate::lisp::regexp::syntax_code(bb.text.char_at(p)) == b'<' {
                p += 1;
                while p < len && crate::lisp::regexp::syntax_code(bb.text.char_at(p)) != b'>' {
                    p += 1;
                }
                if p < len {
                    p += 1;
                }
                comments += 1;
            } else {
                break;
            }
        }
    } else {
        loop {
            while p > bb.begv && crate::lisp::regexp::syntax_code(bb.text.char_at(p - 1)) == b' ' {
                p -= 1;
            }
            if p > bb.begv && crate::lisp::regexp::syntax_code(bb.text.char_at(p - 1)) == b'>' {
                p -= 1;
                while p > bb.begv
                    && crate::lisp::regexp::syntax_code(bb.text.char_at(p - 1)) != b'<'
                {
                    p -= 1;
                }
                comments += 1;
            } else {
                break;
            }
        }
    }
    bb.set_point(p);
    Ok(if comments >= count.abs() && comments > 0 {
        Value::Sym(sym::T)
    } else {
        Value::Nil
    })
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

fn f_skip_chars_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_skip_chars_backward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn syntax_char_of(c: char) -> u8 {
    crate::lisp::regexp::syntax_code(c)
}

fn f_skip_syntax_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    let neg = spec.starts_with('^');
    // `-` is an alias for the whitespace class, like in regexps.
    let codes: Vec<u8> = spec
        .trim_start_matches('^')
        .bytes()
        .map(|c| if c == b'-' { b' ' } else { c })
        .collect();
    let mut p = bb.point();
    while p < lim.min(bb.text.len()) {
        let hit = codes.contains(&syntax_char_of(bb.text.char_at(p)));
        if hit == neg {
            break;
        }
        p += 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

fn f_skip_syntax_backward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    let neg = spec.starts_with('^');
    let codes: Vec<u8> = spec
        .trim_start_matches('^')
        .bytes()
        .map(|c| if c == b'-' { b' ' } else { c })
        .collect();
    let mut p = bb.point();
    while p > lim {
        let hit = codes.contains(&syntax_char_of(bb.text.char_at(p - 1)));
        if hit == neg {
            break;
        }
        p -= 1;
    }
    let moved = p as i128 - bb.point() as i128;
    bb.set_point(p);
    Ok(Value::Int(moved))
}

/// Move point forward over one sexp.  Like GNU: at EOB outside any
/// sexp, stop silently instead of signaling.
fn forward_sexp_once(i: &mut Interp, bb: &mut Buffer) -> Result<(), Flow> {
    let mut p = bb.point();
    let len = bb.text_len();
    // skip whitespace
    while p < len && bb.text.char_at(p).is_whitespace() {
        p += 1;
    }
    if p >= len {
        bb.set_point(len);
        return Ok(());
    }
    let c = bb.text.char_at(p);
    if c == '(' || c == '[' || c == '{' {
        // scan balanced
        let mut depth = 0i128;
        let mut in_str = false;
        let mut esc = false;
        let mut k = p;
        while k < len {
            let ch = bb.text.char_at(k);
            if in_str {
                if esc {
                    esc = false;
                } else if ch == '\\' {
                    esc = true;
                } else if ch == '"' {
                    in_str = false;
                }
            } else {
                match ch {
                    '"' => in_str = true,
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            k += 1;
        }
        if depth != 0 {
            return Err(err_sym(
                i,
                "scan-error",
                vec![Value::string("Unbalanced parentheses")],
            ));
        }
        bb.set_point(k + 1);
    } else if c == ')' || c == ']' || c == '}' {
        return Err(err_sym(
            i,
            "scan-error",
            vec![Value::string("Unbalanced parentheses")],
        ));
    } else if c == '"' {
        let mut k = p + 1;
        let mut esc = false;
        let mut closed = false;
        while k < len {
            let ch = bb.text.char_at(k);
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                closed = true;
                break;
            }
            k += 1;
        }
        if !closed {
            return Err(err_sym(
                i,
                "scan-error",
                vec![Value::string("Unbalanced parentheses")],
            ));
        }
        bb.set_point(k + 1);
    } else {
        // atom: symbol chars
        while p < len {
            let ch = bb.text.char_at(p);
            if ch.is_whitespace() || "()[]{}\"'`,;".contains(ch) {
                break;
            }
            p += 1;
        }
        bb.set_point(p);
    }
    Ok(())
}

/// Move point backward over one sexp.  At BOB, stop silently (GNU).
fn backward_sexp_once(i: &mut Interp, bb: &mut Buffer) -> Result<(), Flow> {
    let mut p = bb.point();
    while p > bb.begv && bb.text.char_at(p - 1).is_whitespace() {
        p -= 1;
    }
    if p <= bb.begv {
        bb.set_point(bb.begv);
        return Ok(());
    }
    let c = bb.text.char_at(p - 1);
    if matches!(c, ')' | ']' | '}') {
        let mut depth = 0i128;
        let mut k = p;
        while k > bb.begv {
            let ch = bb.text.char_at(k - 1);
            match ch {
                ')' | ']' | '}' => depth += 1,
                '(' | '[' | '{' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            k -= 1;
        }
        if depth != 0 {
            return Err(err_sym(
                i,
                "scan-error",
                vec![Value::string("Unbalanced parentheses")],
            ));
        }
        bb.set_point(k - 1);
    } else {
        while p > bb.begv {
            let ch = bb.text.char_at(p - 1);
            if ch.is_whitespace() || "()[]{}\"'`,;".contains(ch) {
                break;
            }
            p -= 1;
        }
        bb.set_point(p);
    }
    Ok(())
}

fn f_forward_sexp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for _ in 0..n.abs() {
        if n < 0 {
            backward_sexp_once(i, &mut *bb)?;
        } else {
            forward_sexp_once(i, &mut *bb)?;
        }
    }
    Ok(Value::Nil)
}

fn f_backward_sexp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for _ in 0..n.abs() {
        if n < 0 {
            forward_sexp_once(i, &mut *bb)?;
        } else {
            backward_sexp_once(i, &mut *bb)?;
        }
    }
    Ok(Value::Nil)
}

fn sexp_is_open(c: char) -> bool {
    matches!(c, '(' | '[' | '{')
}
fn sexp_is_close(c: char) -> bool {
    matches!(c, ')' | ']' | '}')
}

/// Index of the close matching the open at `open` (0-based).
fn sexp_match_close(text: &[char], open: usize) -> Option<usize> {
    let mut d = 0i32;
    for (j, c) in text.iter().enumerate().skip(open) {
        if sexp_is_open(*c) {
            d += 1;
        } else if sexp_is_close(*c) {
            d -= 1;
            if d == 0 {
                return Some(j);
            }
        }
    }
    None
}

/// (complete pairs, unmatched opens) for the text before `pos`.
fn sexp_pairs_before(text: &[char], pos: usize) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut stack = Vec::new();
    let mut pairs = Vec::new();
    for (j, c) in text.iter().enumerate().take(pos.min(text.len())) {
        if sexp_is_open(*c) {
            stack.push(j);
        } else if sexp_is_close(*c) {
            if let Some(o) = stack.pop() {
                pairs.push((o, j));
            }
        }
    }
    (pairs, stack)
}

/// Index of the open matching the close at `close` (0-based).
fn sexp_match_open(text: &[char], close: usize) -> Option<usize> {
    let mut d = 0i32;
    for j in (0..=close.min(text.len().saturating_sub(1))).rev() {
        let c = text[j];
        if sexp_is_close(c) {
            d += 1;
        } else if sexp_is_open(c) {
            d -= 1;
            if d == 0 {
                return Some(j);
            }
        }
    }
    None
}

/// Core of `scan-lists`: returns the 0-based landing position, None
/// for "stays put", Err for scan-error.
///
/// GNU semantics: DEPTH is the assumed paren-level at FROM.  Each step
/// scans forward (COUNT>0) or backward (COUNT<0) for the paren that
/// brings the level back to 0, landing just outside it (after `)' /
/// before `(').  At level 0 a step skips a whole balanced pair; a
/// mismatched paren (close going forward, open going backward) or an
/// unbalanced group signals scan-error; running out of text at level 0
/// returns nil.
fn scan_lists_impl(
    text: &[char],
    pos: usize,
    count: i128,
    depth: i128,
) -> Result<Option<usize>, ()> {
    let len = text.len();
    let mut level = depth;
    let mut p = pos;
    let mut remaining = count.unsigned_abs();
    if count > 0 {
        while remaining > 0 {
            let mut i = p;
            let mut landed = None;
            while i < len {
                let c = text[i];
                if sexp_is_open(c) {
                    if level == 0 {
                        match sexp_match_close(text, i) {
                            Some(cl) => {
                                landed = Some(cl + 1);
                                break;
                            }
                            None => return Err(()),
                        }
                    }
                    level += 1;
                } else if sexp_is_close(c) {
                    if level == 0 {
                        return Err(());
                    }
                    level -= 1;
                    if level == 0 {
                        landed = Some(i + 1);
                        break;
                    }
                }
                i += 1;
            }
            match landed {
                Some(np) => p = np,
                // Ran out of text: nil at level 0, error if still inside.
                None => return if level == 0 { Ok(None) } else { Err(()) },
            }
            remaining -= 1;
        }
        Ok(Some(p))
    } else if count < 0 {
        while remaining > 0 {
            let mut i = p as i64 - 1;
            let mut landed = None;
            while i >= 0 {
                let c = text[i as usize];
                if sexp_is_close(c) {
                    if level == 0 {
                        match sexp_match_open(text, i as usize) {
                            Some(o) => {
                                landed = Some(o);
                                break;
                            }
                            None => return Err(()),
                        }
                    }
                    level += 1;
                } else if sexp_is_open(c) {
                    if level == 0 {
                        return Err(());
                    }
                    level -= 1;
                    if level == 0 {
                        landed = Some(i as usize);
                        break;
                    }
                }
                i -= 1;
            }
            match landed {
                Some(np) => p = np,
                // Ran out of text: nil at level 0, error if still inside.
                None => return if level == 0 { Ok(None) } else { Err(()) },
            }
            remaining -= 1;
        }
        Ok(Some(p))
    } else {
        Ok(Some(pos))
    }
}

fn f_scan_lists(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_int(i, &a[0])?;
    let count = want_int(i, &a[1])?;
    let depth = want_int(i, &a[2])?;
    let text: Vec<char> = {
        let b = cur(i);
        let bb = b.borrow();
        bb.text.text().chars().collect()
    };
    let pos = pos_idx(text.len(), from);
    match scan_lists_impl(&text, pos, count, depth) {
        Ok(Some(p)) => Ok(Value::Int(p as i128 + 1)),
        Ok(None) => Ok(Value::Nil),
        Err(()) => Err(scan_error(i)),
    }
}

fn f_scan_sexps(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_int(i, &a[0])?;
    let count = want_int(i, &a[1])?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let saved = bb.point();
    let pos = pos_idx(bb.text_len(), from);
    bb.set_point(pos);
    let mut result = pos;
    let mut steps = 0i128;
    let mut failed = false;
    let mut flow = None;
    while steps < count.abs() {
        let before = bb.point();
        let r = if count < 0 {
            backward_sexp_once(i, &mut bb)
        } else {
            forward_sexp_once(i, &mut bb)
        };
        match r {
            Ok(()) => {
                let after = bb.point();
                if after == before {
                    failed = true;
                    break;
                }
                result = after;
                steps += 1;
            }
            Err(e) => {
                flow = Some(e);
                break;
            }
        }
    }
    bb.set_point(saved);
    drop(bb);
    if let Some(e) = flow {
        return Err(e);
    }
    if failed || steps < count.abs() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(result as i128 + 1))
}

fn scan_error(i: &mut Interp) -> Flow {
    let sym = i.intern("scan-error");
    i.signal_data(sym, Vec::new())
}

/// Emacs's `(scan-error "Unbalanced parentheses" BEG END)' — 1-based.
fn scan_error_at(i: &mut Interp, beg0: usize, end0: usize) -> Flow {
    let sym = i.intern("scan-error");
    i.signal_data(
        sym,
        vec![
            Value::string("Unbalanced parentheses"),
            Value::Int(beg0 as i128 + 1),
            Value::Int(end0 as i128 + 1),
        ],
    )
}

fn nav_text(i: &Interp) -> (Vec<char>, usize) {
    let b = i.buffers.get(i.current_buffer).unwrap();
    let bb = b.borrow();
    (bb.text.text().chars().collect(), bb.point())
}

fn f_down_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    let (text, mut p) = nav_text(i);
    for _ in 0..n {
        let mut j = p;
        while j < text.len() && !sexp_is_open(text[j]) {
            j += 1;
        }
        if j >= text.len() {
            return Err(scan_error_at(i, p, text.len()));
        }
        p = j + 1;
    }
    cur(i).borrow_mut().set_point(p);
    Ok(Value::Nil)
}

fn f_up_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    let (text, mut p) = nav_text(i);
    for _ in 0..n {
        let (_, stack) = sexp_pairs_before(&text, p);
        match stack.last() {
            Some(&o) => match sexp_match_close(&text, o) {
                Some(c) => p = c + 1,
                None => return Err(scan_error_at(i, o, text.len())),
            },
            None => return Err(scan_error_at(i, p, text.len())),
        }
    }
    cur(i).borrow_mut().set_point(p);
    Ok(Value::Nil)
}

fn f_forward_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    let (text, p) = nav_text(i);
    match scan_lists_impl(&text, p, n, 0) {
        Ok(Some(np)) => {
            cur(i).borrow_mut().set_point(np);
            Ok(Value::Nil)
        }
        Ok(None) => Ok(Value::Nil),
        Err(()) => Err(scan_error_at(i, p, text.len())),
    }
}

fn f_backward_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    let (text, p) = nav_text(i);
    match scan_lists_impl(&text, p, -n, 0) {
        Ok(Some(np)) => {
            cur(i).borrow_mut().set_point(np);
            Ok(Value::Nil)
        }
        Ok(None) => Ok(Value::Nil),
        Err(()) => Err(scan_error_at(i, p, 0)),
    }
}

fn f_backward_up_list(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    let (text, mut p) = nav_text(i);
    for _ in 0..n {
        let (_, stack) = sexp_pairs_before(&text, p);
        match stack.last() {
            Some(&o) => p = o,
            None => return Err(scan_error_at(i, p, 0)),
        }
    }
    cur(i).borrow_mut().set_point(p);
    Ok(Value::Nil)
}

pub(crate) fn f_syntax_after(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?;
    let b = cur(i);
    let bb = b.borrow();
    let idx = pos_idx(bb.text_len(), pos);
    if idx >= bb.text_len() {
        return Ok(Value::Nil);
    }
    let c = bb.text.char_at(idx);
    // GNU syntax classes: 0 ws, 1 punct, 2 word, 3 symbol, 4 open,
    // 5 close, 6 expr-prefix, 7 string-quote, 8 math, 9 escape,
    // 10 charquote, 11 comment-start, 12 comment-end, 14/15 fences.
    let cls: i128 = match crate::lisp::regexp::syntax_code(c) {
        b' ' => 0,
        b'w' => 2,
        b'_' => 3,
        b'(' => 4,
        b')' => 5,
        b'\'' => 6,
        b'"' => 7,
        b'\\' => 9,
        b'<' => 11,
        b'>' => 12,
        _ => 1,
    };
    let matching: Option<char> = match cls {
        4 => Some(match c {
            '(' => ')',
            '[' => ']',
            _ => '}',
        }),
        5 => Some(match c {
            ')' => '(',
            ']' => '[',
            _ => '{',
        }),
        _ => None,
    };
    // GNU returns a dotted pair (CLASS . MATCHING-CHAR) for
    // open/close classes, a singleton list otherwise.
    Ok(match matching {
        Some(m) => Value::cons(Value::Int(cls), Value::Int(m as i128)),
        None => Value::list(vec![Value::Int(cls)]),
    })
}

fn f_looking_back(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    // Non-greedy: shortest match ending at point (nearest start).
    // Greedy: longest (smallest start wins).
    let order: Vec<usize> = if greedy {
        (lo..=pos).collect()
    } else {
        (lo..=pos).rev().collect()
    };
    for start in order {
        if let Some(regs) = crate::lisp::regexp::match_at(&re, &text, start) {
            if regs[1] == Some(pos) {
                i.match_data = Some(MatchData {
                    regs,
                    in_buffer: true,
                    base,
                });
                return Ok(Value::t());
            }
        }
    }
    Ok(Value::Nil)
}

fn f_last_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match i.buffers.order.last().copied() {
        Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
        None => Ok(Value::Nil),
    }
}

// ---------- insertion & deletion ----------

fn insert_str_at_point(i: &mut Interp, s: &str, before_markers: bool) -> Result<(), Flow> {
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if before_markers {
        bb.insert_before_markers(s);
    } else {
        bb.insert(s);
    }
    Ok(())
}

fn f_insert(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    for v in &a {
        match v {
            Value::Str(s) => {
                let t = s.borrow().clone();
                insert_str_at_point(i, &t, false)?;
            }
            Value::Int(n) => {
                if let Some(c) = char::from_u32(*n as u32) {
                    insert_str_at_point(i, &c.to_string(), false)?;
                } else {
                    return Err(i.wrong_type_mut("characterp", v));
                }
            }
            other => return Err(i.wrong_type_mut("char-or-string-p", other)),
        }
    }
    Ok(Value::Nil)
}

fn f_insert_before_markers(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    for v in &a {
        match v {
            Value::Str(s) => {
                let t = s.borrow().clone();
                insert_str_at_point(i, &t, true)?;
            }
            Value::Int(n) => {
                if let Some(c) = char::from_u32(*n as u32) {
                    insert_str_at_point(i, &c.to_string(), true)?;
                } else {
                    return Err(i.wrong_type_mut("characterp", v));
                }
            }
            other => return Err(i.wrong_type_mut("char-or-string-p", other)),
        }
    }
    Ok(Value::Nil)
}

fn f_insert_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let c = match &a[0] {
        Value::Int(n) => {
            char::from_u32(*n as u32).ok_or_else(|| i.wrong_type_mut("characterp", &a[0]))?
        }
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let count = a.get(1).and_then(|v| v.int()).unwrap_or(1).max(0);
    check_writable(i)?;
    let s: String = std::iter::repeat(c).take(count as usize).collect();
    insert_str_at_point(i, &s, false)?;
    Ok(Value::Nil)
}

fn f_insert_buffer_substring(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let src = buf_of(i, &a[0])?;
    let (text, len) = {
        let bb = src.borrow();
        (bb.text.text(), bb.text.len())
    };
    let s = a
        .get(1)
        .and_then(|v| v.int())
        .map(|p| pos_idx(len, p))
        .unwrap_or(0);
    let e = a
        .get(2)
        .and_then(|v| v.int())
        .map(|p| pos_idx(len, p))
        .unwrap_or(len);
    let chars: Vec<char> = text.chars().collect();
    let sub: String = chars[s.min(e)..e.max(s)].iter().collect();
    check_writable(i)?;
    insert_str_at_point(i, &sub, false)?;
    Ok(Value::Nil)
}

fn f_self_insert_command(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(0);
    // `last-command-event` holds the char.
    let ev = i.symbol_value(i.intern_soft("last-command-event").unwrap_or(0));
    let c = match ev {
        Value::Int(x) => char::from_u32(x as u32),
        _ => None,
    };
    if let Some(c) = c {
        let s: String = std::iter::repeat(c).take(n as usize).collect();
        insert_str_at_point(i, &s, false)?;
    }
    Ok(Value::Nil)
}

fn f_newline(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    insert_str_at_point(i, &"\n".repeat(n as usize), false)?;
    Ok(Value::Nil)
}

fn f_open_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1);
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    bb.insert_at(p, &"\n".repeat(n as usize));
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_delete_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let zv = bb.text_len();
    if n > 0 {
        if p >= zv {
            return Err(i.signal_data(sym::END_OF_BUFFER, vec![]));
        }
        let end = (p + n as usize).min(zv);
        bb.delete_region(p, end);
    } else if n < 0 {
        if p <= bb.begv {
            return Err(i.signal_data(sym::BEGINNING_OF_BUFFER, vec![]));
        }
        let start = (p as i128 + n).max(bb.begv as i128) as usize;
        bb.delete_region(start, p);
    }
    Ok(Value::Nil)
}

fn f_delete_backward_char(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    f_delete_char(
        i,
        vec![Value::Int(-n), a.get(1).cloned().unwrap_or(Value::Nil)],
    )
}

fn f_backward_delete_char_untabify(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Untabify: when deleting a single char that lands inside a tab,
    // GNU converts the tab to spaces first. Our buffer stores '\t'
    // literally; deleting the tab char is the closest equivalent.
    let n = want_int(i, &a[0])?;
    f_delete_char(
        i,
        vec![Value::Int(-n), a.get(1).cloned().unwrap_or(Value::Nil)],
    )
}

fn f_beginning_of_visual_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // tty: visual lines == logical lines.
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(0);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n).max(0) as usize;
    let p = bb.text.line_start(target).max(bb.begv);
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_end_of_visual_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(0);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let cur_line = bb.text.line_of_pos(bb.point());
    let target = (cur_line as i128 + n).max(0) as usize;
    let p = bb
        .text
        .line_end(bb.text.line_start(target))
        .min(bb.text_len());
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_forward_visible_line(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = f_forward_line(i, vec![arg(&a, 0)])?;
    Ok(Value::Nil)
}

fn f_delete_and_extract_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let (s, e) = (s.min(e), s.max(e));
    Ok(Value::string(bb.delete_region(s, e)))
}

fn f_delete_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_delete_and_extract_region(i, a).map(|_| Value::Nil)
}

fn f_erase_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let tlen = bb.text.len();
    bb.delete_region(0, tlen);
    bb.begv = 0;
    bb.zv = 0;
    bb.set_point(0);
    Ok(Value::Nil)
}

fn f_kill_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Record into kill-ring then delete.
    let text = f_delete_and_extract_region(i, a.clone())?;
    if let Value::Str(s) = &text {
        push_kill_ring(i, s.borrow().clone());
    }
    Ok(Value::Nil)
}

/// Push a string onto the kill-ring (a plain list var).
pub(crate) fn push_kill_ring(i: &mut Interp, s: String) {
    let kr = i.intern("kill-ring");
    let cur = i.symbol_value(kr);
    let mut items = cur.list_to_vec().unwrap_or_default();
    // `append-next-kill' concatenates onto the newest entry.
    if i.append_next_kill {
        i.append_next_kill = false;
        if let Some(Value::Str(prev)) = items.first_mut() {
            prev.borrow_mut().push_str(&s);
            i.obarray.symbol_mut(kr).value = Value::list(items);
            return;
        }
    }
    items.insert(0, Value::string(s));
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
}

fn f_append_next_kill(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.append_next_kill = true;
    i.message("If the next command is a kill, it will append");
    Ok(Value::Nil)
}

// ---------- buffer text access ----------

fn f_buffer_substring(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let (s, e) = (s.min(e), s.max(e));
    Ok(Value::string(bb.text.substring(s, e)))
}

fn f_filter_buffer_substring(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let delete = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    // `filter-buffer-substring-function' hook: GNU delegates the whole
    // operation — call3(FUN, BEG, END, DELETE) returns the substring.
    let f = i
        .intern_soft("filter-buffer-substring-function")
        .map(|h| i.symbol_value(h))
        .filter(|v| !matches!(v, Value::Sym(s) if *s == sym::UNBOUND))
        .unwrap_or(Value::Nil);
    if f.truthy() {
        let args = Value::list(vec![a[0].clone(), a[1].clone(), Value::from_bool(delete)]);
        return i.call_function(&f, &args, None);
    }
    let text = f_buffer_substring(i, a[..2].to_vec())?;
    if delete {
        let b = cur(i);
        let (s, e) = {
            let bb = b.borrow();
            let len = bb.text.len();
            let s = pos_idx(len, want_int(i, &a[0])?);
            let e = pos_idx(len, want_int(i, &a[1])?);
            (s.min(e), s.max(e))
        };
        b.borrow_mut().delete_region(s, e);
    }
    Ok(text)
}

fn f_buffer_string(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::string(bb.text.substring(bb.begv, bb.text_len())))
}

fn f_buffer_substring_with_bidi_context(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = want_int(i, &a[0])?;
    let e = want_int(i, &a[1])?;
    let (s, e) = (s.min(e), s.max(e));
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text_len();
    if s < bb.begv as i128 + 1 || e > len as i128 + 1 {
        let sym = i.intern("args-out-of-range");
        let args = vec![a[0].clone(), a[1].clone()];
        drop(bb);
        return Err(i.signal_data(sym, args));
    }
    Ok(Value::string(
        bb.text.substring((s - 1) as usize, (e - 1) as usize),
    ))
}

fn f_bounds_of_thing_at_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym_id = want_sym(i, &a[0])?;
    let name = i.symbol_name(sym_id);
    let pred: fn(char) -> bool = match name.as_str() {
        "word" | "number" => |c| c.is_alphanumeric() || c == '_',
        "symbol" => |c| c.is_alphanumeric() || "_-?!*+/<>=:$%&~^.".contains(c),
        _ => return Ok(Value::Nil),
    };
    match thing_bounds(i, pred) {
        Some((s, e)) => Ok(Value::cons(
            Value::Int(s as i128 + 1),
            Value::Int(e as i128 + 1),
        )),
        None => Ok(Value::Nil),
    }
}

fn thing_bounds(i: &mut Interp, pred: fn(char) -> bool) -> Option<(usize, usize)> {
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

fn f_current_word(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match thing_bounds(i, |c| c.is_alphanumeric() || c == '_') {
        Some((s, e)) => {
            let b = cur(i);
            Ok(Value::string(b.borrow().text.substring(s, e)))
        }
        None => Ok(Value::Nil),
    }
}

fn f_word_at_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_current_word(i, a)
}

fn f_symbol_at_point(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    match thing_bounds(i, |c| {
        c.is_alphanumeric() || "_-?!*+/<>=:$%&~^.".contains(c)
    }) {
        Some((s, e)) => {
            let b = cur(i);
            let name = b.borrow().text.substring(s, e);
            Ok(Value::Sym(i.intern(&name)))
        }
        None => Ok(Value::Nil),
    }
}

fn f_thing_at_point(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sym_id = want_sym(i, &a[0])?;
    let name = i.symbol_name(sym_id);
    match name.as_str() {
        "word" => f_current_word(i, vec![]),
        "symbol" => {
            match thing_bounds(i, |c| {
                c.is_alphanumeric() || "_-?!*+/<>=:$%&~^.".contains(c)
            }) {
                Some((s, e)) => {
                    let b = cur(i);
                    Ok(Value::string(b.borrow().text.substring(s, e)))
                }
                None => Ok(Value::Nil),
            }
        }
        "line" => {
            let b = cur(i);
            let bb = b.borrow();
            let ls = bb.text.line_start(bb.text.line_of_pos(bb.point()));
            let le = bb.text.line_end(bb.point());
            Ok(Value::string(bb.text.substring(ls, le)))
        }
        "number" => f_current_word(i, vec![]),
        "filename" | "url" | "email" | "sexp" | "sentence" | "defun" | "list" | "whitespace"
        | "page" => Ok(Value::Nil),
        _ => Ok(Value::Nil),
    }
}

// ---------- mark & region ----------

fn f_mark(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: the mark marker always exists once the buffer is live, so an
    // unset mark yields nil, not an error.  (The "mark is not set"
    // error comes from region-beginning/region-end, not `mark`.)
    match bb.mark {
        Some(m) => Ok(Value::Int(m as i128 + 1)),
        None => Ok(Value::Nil),
    }
}

fn f_set_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let p = match &a[0] {
        Value::Marker(m) => m.borrow().position,
        v => pos_idx(len, want_int(i, v)?),
    };
    bb.mark = Some(p);
    Ok(Value::Int(p as i128 + 1))
}

fn f_mark_marker(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let (id, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.mark.unwrap_or(0))
    };
    Ok(new_marker_at(i, id, pos))
}

fn f_push_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let p = match a.get(0) {
        Some(v) if v.truthy() => pos_idx(len, want_int(i, v)?),
        _ => bb.point(),
    };
    bb.mark = Some(p);
    // push onto mark-ring (a buffer-local list var)
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    let cur_ring = bb.locals.get(&ring_sym).cloned().unwrap_or(Value::Nil);
    let mut items = cur_ring.list_to_vec().unwrap_or_default();
    let m = Rc::new(RefCell::new(Marker {
        buffer: Some(bb.id),
        position: p,
        insertion_type: false,
    }));
    bb.register_marker(&m);
    items.push(marker_value(m));
    let max = i
        .symbol_value(i.intern_soft("mark-ring-max").unwrap_or(0))
        .int()
        .unwrap_or(16) as usize;
    if items.len() > max {
        items.remove(0);
    }
    bb.locals.insert(ring_sym, Value::list(items));
    if a.get(2).map(|v| v.truthy()).unwrap_or(false) {
        bb.mark_active = true;
    }
    // GNU (push-mark LOCATION NOMSG ACTIVATE): messages unless NOMSG.
    let nomsg = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    drop(bb);
    if !nomsg {
        i.message("Mark set");
    }
    Ok(Value::Nil)
}

fn f_pop_mark(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    let ring = bb.locals.get(&ring_sym).cloned().unwrap_or(Value::Nil);
    let mut items = ring.list_to_vec().unwrap_or_default();
    if let Some(m) = items.pop() {
        if let Value::Marker(mm) = &m {
            bb.mark = Some(mm.borrow().position);
            bb.set_point(mm.borrow().position);
        }
    } else {
        bb.mark = None;
    }
    bb.locals.insert(ring_sym, Value::list(items));
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

fn f_set_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU keeps the (key) cell even when VALUE is nil.
    reg_set(i, a[0].clone(), a[1].clone())?;
    Ok(a[1].clone())
}

fn f_get_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(reg_get(i, &a[0]))
}

fn f_number_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn region_bounds(i: &mut Interp) -> Result<(usize, usize), Flow> {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark {
        Some(m) => {
            let p = bb.point();
            Ok((m.min(p), m.max(p)))
        }
        None => Err(no_mark_err(i)),
    }
}

fn f_increment_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
        let nv = Value::Float(old + nf);
        reg_set(i, reg, nv.clone())?;
        return Ok(nv);
    }
    // Anything else (string, nil, unset) appends the active region's
    // text to the register, erroring when no mark is set.
    let (lo, hi) = region_bounds(i)?;
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

fn f_point_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (buf, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.id, bb.point())
    };
    let m = new_marker_at(i, buf, pos);
    reg_set(i, a[0].clone(), m.clone())?;
    Ok(m)
}

fn f_jump_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let val = reg_get(i, &a[0]);
    match &val {
        Value::Marker(m) => {
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
                    f_set_buffer(i, vec![name]).map(|_| val.clone())
                }
                "file" | "file-query" => {
                    let target = rest.list_to_vec().ok().and_then(|v| v.into_iter().next());
                    match target {
                        Some(name) => {
                            let find = Value::Sym(i.intern("find-file"));
                            i.call_function(&find, &Value::list(vec![name]), None)?;
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

fn f_insert_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
        // A position/marker register inserts nothing and returns nil.
        Value::Marker(_) => Ok(Value::Nil),
        _ => Err(err_sym(
            i,
            "user-error",
            vec![Value::string("Register does not contain text")],
        )),
    }
}

fn f_copy_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // REGION non-nil means START/END are ignored and the marked region
    // is used (GNU's 5th arg).
    let (lo, hi) = if a.get(4).map(|v| v.truthy()).unwrap_or(false) {
        region_bounds(i)?
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
        let b = cur(i);
        let mut bb = b.borrow_mut();
        bb.delete_region(lo, hi);
    }
    reg_set(i, a[0].clone(), Value::string(text))?;
    Ok(Value::Nil)
}

fn f_window_configuration_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_frame_configuration_to_register(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

/// Display width of one char (same rules as `current-column`).
fn rect_char_width(c: char, tab: i128) -> i128 {
    let _ = tab;
    if (c as u32) < 0x20 || c == '\x7f' {
        2
    } else {
        unicode_width::UnicodeWidthChar::width(c)
            .unwrap_or(1)
            .max(1) as i128
    }
}

fn rect_tab_width(i: &Interp) -> i128 {
    i.symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1)
}

/// Column of char index P on the line starting at LS.
pub(crate) fn rect_col_at(
    text: &crate::buffer::gapbuf::GapBuffer,
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
        let mut bb = b.borrow_mut();
        f(&mut bb, ls, le, c0, c1, tab)?;
        final_point = bb.point();
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
    text: &crate::buffer::gapbuf::GapBuffer,
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

fn f_kill_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let segs = rect_extract_span(i, &a, true, fill)?;
    rect_set_killed(i, segs);
    Ok(Value::Nil)
}

fn f_delete_extract_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let segs = rect_extract_span(i, &a, true, fill)?;
    Ok(Value::list(segs.into_iter().map(Value::string).collect()))
}

fn f_extract_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let segs = rect_extract_span(i, &a, false, false)?;
    Ok(Value::list(segs.into_iter().map(Value::string).collect()))
}

fn f_copy_rectangle_as_kill(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let segs = rect_extract_span(i, &a, false, false)?;
    rect_set_killed(i, segs);
    Ok(Value::Nil)
}

fn f_delete_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fill = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    rect_apply(i, &a, |bb, ls, le, c0, c1, tab| {
        let p0 = rect_delete_line(bb, ls, le, c0, c1, fill, tab);
        bb.set_point(p0);
        Ok(())
    })
}

fn f_clear_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_open_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_delete_whitespace_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_string_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = rect_string_arg(i, &a[2])?;
    rect_string_lines(i, &a, &s, true)
}

fn f_string_insert_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = rect_string_arg(i, &a[2])?;
    rect_string_lines(i, &a, &s, false)
}

/// Shared body of yank-rectangle / insert-rectangle: GNU's
/// insert-rectangle — first line at point, following lines at the same
/// column (coerced with `t`), creating lines at EOF.
fn rect_insert_segs(i: &mut Interp, segs: Vec<String>) -> EvalResult {
    check_writable(i)?;
    // GNU pushes the mark at the upper-left corner unconditionally.
    f_push_mark(i, vec![])?;
    if segs.is_empty() {
        return Ok(Value::Nil);
    }
    let tab = rect_tab_width(i);
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
    Ok(Value::Nil)
}

fn f_yank_rectangle(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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

fn f_insert_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_rectangle_number_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
            let nlines = match f_count_lines(i, vec![a[0].clone(), a[1].clone()])? {
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

fn f_spaces_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = match &a[0] {
        Value::Int(n) => *n,
        Value::Marker(m) => m.borrow().position as i128 + 1,
        other => return Err(i.wrong_type_mut("number-or-marker-p", other)),
    };
    Ok(Value::string(" ".repeat(n.max(0) as usize)))
}

fn f_rectangle_dimensions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_rectangle_position_as_coordinates(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let p = pos_idx(len, want_int(i, &a[0])?);
    let line = bb.text.line_of_pos(p) as i128 + 1;
    let tab = rect_tab_width(i);
    let col = rect_col_at(&bb.text, bb.text.line_start(bb.text.line_of_pos(p)), p, tab);
    Ok(Value::cons(Value::Int(col), Value::Int(line)))
}

fn f_rectangle_intersect_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_extract_rectangle_bounds(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
fn f_apply_on_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
fn f_operate_on_rectangle(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_region_beginning(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark {
        Some(m) => Ok(Value::Int((m.min(bb.point()) + 1) as i128)),
        None => Err(err_sym(i, "mark-inactive", vec![])),
    }
}

fn f_region_end(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark {
        Some(m) => Ok(Value::Int((m.max(bb.point()) + 1) as i128)),
        None => Err(err_sym(i, "mark-inactive", vec![])),
    }
}

fn f_region_active_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let tmm = i
        .symbol_value(i.intern_soft("transient-mark-mode").unwrap_or(0))
        .truthy();
    Ok(Value::from_bool(
        bb.mark.is_some() && (bb.mark_active || !tmm),
    ))
}

fn f_deactivate_mark(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    b.borrow_mut().mark_active = false;
    Ok(Value::Nil)
}

fn f_activate_mark(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if bb.mark.is_some() {
        bb.mark_active = true;
        Ok(Value::t())
    } else {
        Ok(Value::Nil)
    }
}

fn f_exchange_point_and_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    match bb.mark {
        Some(m) => {
            let p = bb.point();
            bb.set_point(m);
            bb.mark = Some(p);
            if !a.get(0).map(|v| v.truthy()).unwrap_or(false) {
                bb.mark_active = true;
            }
            Ok(Value::Nil)
        }
        None => Err(err_sym(i, "mark-inactive", vec![])),
    }
}

fn f_use_region_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_region_active_p(i, a)
}

// ---------- narrowing ----------

fn f_narrow_to_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let (s, e) = (s.min(e), s.max(e));
    bb.begv = s;
    bb.zv = e;
    if bb.point < s || bb.point > e {
        bb.point = s;
    }
    Ok(Value::Nil)
}

fn f_widen(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.begv = 0;
    bb.zv = bb.text.len();
    Ok(Value::Nil)
}

// ---------- markers ----------

fn f_markerp(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&a[0], Value::Marker(_))))
}

fn f_make_marker(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(marker_value(Rc::new(RefCell::new(Marker {
        buffer: None,
        position: 0,
        insertion_type: false,
    }))))
}

fn f_copy_marker(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => {
            let mm = m.borrow();
            let nm = Marker {
                buffer: mm.buffer,
                position: mm.position,
                insertion_type: a.get(1).map(|v| v.truthy()).unwrap_or(mm.insertion_type),
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
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_set_marker(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let m = match &a[0] {
        Value::Marker(m) => m.clone(),
        other => return Err(i.wrong_type_mut("markerp", other)),
    };
    let buf_id = match a.get(2) {
        Some(v) if v.truthy() => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&v))))?,
        _ => match &a[1] {
            Value::Marker(mm) => mm.borrow().buffer.unwrap_or(i.current_buffer),
            _ => i.current_buffer,
        },
    };
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

fn f_marker_position(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_marker_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => match m.borrow().buffer {
            Some(id) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
            None => Ok(Value::Nil),
        },
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_marker_insertion_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Marker(m) => Ok(Value::from_bool(m.borrow().insertion_type)),
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

fn f_set_marker_insertion_type(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_looking_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    let (text, pos) = {
        let b = cur(i);
        let bb = b.borrow();
        (
            bb.text
                .substring(bb.begv, bb.text_len())
                .chars()
                .collect::<Vec<char>>(),
            bb.point() - bb.begv,
        )
    };
    match crate::lisp::regexp::looking_at(&re, &text, pos) {
        Some(regs) => {
            i.match_data = Some(MatchData {
                regs,
                in_buffer: true,
                base: cur(i).borrow().begv,
            });
            Ok(Value::t())
        }
        None => Ok(Value::Nil),
    }
}

fn f_string_match(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pat = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let text = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let start = a.get(2).and_then(|v| v.int()).unwrap_or(0).max(0) as usize;
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let chars: Vec<char> = text.chars().collect();
    match crate::lisp::regexp::search_full(&re, &chars, start.min(chars.len())) {
        Some(regs) => {
            let start0 = regs[0].unwrap_or(0);
            i.match_data = Some(MatchData {
                regs,
                in_buffer: false,
                base: 0,
            });
            Ok(Value::Int(start0 as i128))
        }
        None => Ok(Value::Nil),
    }
}

/// GNU's `string-match-p` does not change the match data.
fn f_string_match_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let saved = i.match_data.clone();
    let r = f_string_match(i, a);
    i.match_data = saved;
    r
}

pub(crate) fn search_common(
    i: &mut Interp,
    a: &[Value],
    re: Option<&crate::lisp::regexp::Regex>,
    needle: Option<&str>,
    backward: bool,
) -> EvalResult {
    let bound = a.get(1).and_then(|v| v.int());
    let noerror = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let count = a.get(3).and_then(|v| v.int()).unwrap_or(1);
    let (text, pos, begv) = {
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
                    crate::lisp::regexp::search_backward_full(r, &text, cur_pos)
                } else {
                    crate::lisp::regexp::search_full(r, &text, cur_pos)
                }
            }
            (_, Some(n)) => literal_search(&text, n, cur_pos, backward),
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
            None => break,
        }
    }
    match found {
        Some(regs) => {
            let e = regs[1].unwrap_or(0);
            let s = regs[0].unwrap_or(0);
            let landing = if backward { s } else { e };
            cur(i).borrow_mut().set_point(landing + begv);
            i.match_data = Some(MatchData {
                regs,
                in_buffer: true,
                base: begv,
            });
            Ok(Value::Int(landing as i128 + begv as i128 + 1))
        }
        None => {
            if noerror {
                // GNU: noerror neither nil nor t moves point to the
                // search limit (BOUND, else eob/bob in search dir).
                let noerror_t = matches!(&a[2], Value::Sym(s) if i.sym_is(&Value::Sym(*s), sym::T));
                if !noerror_t {
                    let limit = match bound {
                        Some(b) => b.max(1) as usize - 1,
                        None if backward => 0,
                        None => text.len(),
                    };
                    let buf = cur(i);
                    let blen = buf.borrow().text_len();
                    buf.borrow_mut().set_point(limit.min(blen));
                }
                Ok(Value::Nil)
            } else {
                Err(i.signal_data(sym::SEARCH_FAILED, vec![a[0].clone()]))
            }
        }
    }
}

/// Literal (non-regexp) search.
pub(crate) fn literal_search(
    text: &[char],
    needle: &str,
    from: usize,
    backward: bool,
) -> Option<crate::lisp::regexp::Regs> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return Some(vec![Some(from.min(text.len())), Some(from.min(text.len()))]);
    }
    if backward {
        // GNU: the match must end at or before `from`.
        let mut p = match from.min(text.len()).checked_sub(n.len()) {
            Some(p) => p,
            None => return None,
        };
        loop {
            if text[p..p + n.len()] == n[..] {
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
            if text[p..p + n.len()] == n[..] {
                return Some(vec![Some(p), Some(p + n.len())]);
            }
            p += 1;
        }
        None
    }
}

fn f_re_search_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, false)
}

fn f_re_search_backward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let re = regexp_compile(i, &a[0])?;
    search_common(i, &a, Some(&re), None, true)
}

fn f_search_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let needle = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    search_common(i, &a, None, Some(&needle), false)
}

fn f_search_backward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let needle = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    search_common(i, &a, None, Some(&needle), true)
}

fn f_match_beginning(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?.max(0) as usize;
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

fn f_match_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?.max(0) as usize;
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

fn f_match_data(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _integers = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    let _reuse = a.get(1);
    let _reseat = a.get(2);
    match &i.match_data {
        Some(md) => {
            let mut items = Vec::new();
            for k in 0..(md.regs.len() / 2) {
                let s = md.regs.get(2 * k).copied().flatten();
                let e = md.regs.get(2 * k + 1).copied().flatten();
                let off = md.base + if md.in_buffer { 1 } else { 0 };
                items.push(match s {
                    Some(p) => Value::Int((p + off) as i128),
                    None => Value::Nil,
                });
                items.push(match e {
                    Some(p) => Value::Int((p + off) as i128),
                    None => Value::Nil,
                });
            }
            Ok(Value::list(items))
        }
        None => Ok(Value::Nil),
    }
}

fn f_set_match_data(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let items = a[0].list_to_vec().unwrap_or_default();
    let mut regs = Vec::new();
    let in_buffer = i.match_data.as_ref().map(|m| m.in_buffer).unwrap_or(true);
    let base = i.match_data.as_ref().map(|m| m.base).unwrap_or(0);
    let off = base + if in_buffer { 1 } else { 0 };
    let mut k = 0;
    while k < items.len() {
        let s = items[k].int().map(|p| (p as usize).saturating_sub(off));
        let e = items
            .get(k + 1)
            .and_then(|v| v.int())
            .map(|p| (p as usize).saturating_sub(off));
        regs.push(s);
        regs.push(e);
        k += 2;
    }
    i.match_data = Some(MatchData {
        regs,
        in_buffer,
        base,
    });
    Ok(Value::Nil)
}

fn f_match_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = want_int(i, &a[0])?.max(0) as usize;
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
            _ => Ok(Value::Nil),
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

fn f_replace_match(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let newtext = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        Value::Nil => String::new(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let fixedcase = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let literal = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let str_arg = match a.get(3) {
        Some(Value::Str(s)) => Some(s.borrow().clone()),
        _ => None,
    };
    let subexp = a.get(4).and_then(|v| v.int()).unwrap_or(0).max(0) as usize;
    let md = match &i.match_data {
        Some(m) => m.clone(),
        None => {
            return Err(err_sym(i, "args-out-of-range", vec![]));
        }
    };
    // STRING path: substitute within the given string's match registers.
    if let Some(text) = str_arg {
        let src: Vec<char> = text.chars().collect();
        let rep = expand_replacement(i, &newtext, literal, &md, &src)?;
        let (s, e) = match (
            md.regs.get(2 * subexp).copied().flatten(),
            md.regs.get(2 * subexp + 1).copied().flatten(),
        ) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![Value::Int(subexp as i128)],
                ));
            }
        };
        let mut out: String = src[..s.min(src.len())].iter().collect();
        out.push_str(&rep);
        out.extend(src[e.min(src.len())..].iter());
        return Ok(Value::string(out));
    }
    if !md.in_buffer {
        return Ok(Value::Nil);
    }
    let (s, e) = match (
        md.regs.get(2 * subexp).copied().flatten(),
        md.regs.get(2 * subexp + 1).copied().flatten(),
    ) {
        (Some(s), Some(e)) => (s, e),
        _ => return Err(i.error("match subexp did not match")),
    };
    // Buffer path: build replacement against buffer text.
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let start = md.base + s;
    let end = md.base + e;
    let old = bb.text.substring(start, end.min(bb.text.len()));
    let src: Vec<char> = bb.text.substring(bb.begv, bb.text.len()).chars().collect();
    let mut rep = expand_replacement(i, &newtext, literal, &md, &src)?;
    if !fixedcase {
        rep = match_case(&old, &rep);
    }
    let tlen = bb.text.len();
    bb.delete_region(start, end.min(tlen));
    bb.insert_at(start, &rep);
    bb.set_point(start + rep.chars().count());
    Ok(Value::Nil)
}

fn f_match_substitute_replacement(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let newtext = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
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
            // Emacs: (args-out-of-range BUFFER 0 SCHARS(replacement)).
            let b = cur(i);
            let n = newtext.chars().count() as i128;
            return Err(err_sym(
                i,
                "args-out-of-range",
                vec![Value::Buffer(b), Value::Int(0), Value::Int(n)],
            ));
        }
    };
    let rep = expand_replacement(i, &newtext, literal, &md, &src)?;
    Ok(Value::string(rep))
}

/// Case-matching for `replace-match`: if OLD is all-caps/capitalized,
/// apply the same to NEW.
fn match_case(old: &str, new: &str) -> String {
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

fn f_regexp_quote(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_regexp_opt(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_regexp_opt_depth(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

pub(crate) fn f_put_text_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let prop = want_sym(i, &a[2])?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.text_props.push(TextProp {
        start: s,
        end: e,
        prop,
        value: a[3].clone(),
    });
    Ok(Value::Nil)
}

fn f_add_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let plist = a[2].list_to_vec().unwrap_or_default();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let mut k = 0;
    while k + 1 < plist.len() {
        if let Some(p) = i.sym_id(&plist[k]) {
            bb.text_props.push(TextProp {
                start: s,
                end: e,
                prop: p,
                value: plist[k + 1].clone(),
            });
        }
        k += 2;
    }
    Ok(Value::t())
}

pub(crate) fn f_remove_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let plist = a[2].list_to_vec().unwrap_or_default();
    let props: Vec<u32> = plist
        .iter()
        .step_by(2)
        .filter_map(|v| i.sym_id(v))
        .collect();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let before = bb.text_props.len();
    bb.text_props
        .retain(|tp| !(props.contains(&tp.prop) && tp.start < e && tp.end > s));
    Ok(Value::from_bool(bb.text_props.len() != before))
}

fn f_set_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Remove all props in range, then add the plist.
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    {
        let b = cur(i);
        b.borrow_mut()
            .text_props
            .retain(|tp| !(tp.start < e && tp.end > s));
    }
    f_add_text_properties(i, a)
}

pub(crate) fn prop_at(i: &mut Interp, pos: usize, prop: u32) -> Value {
    let b = cur(i);
    let bb = b.borrow();
    // Last write wins.
    for tp in bb.text_props.iter().rev() {
        if tp.prop == prop && pos >= tp.start && pos < tp.end {
            return tp.value.clone();
        }
    }
    Value::Nil
}

pub(crate) fn f_get_text_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Emacs arg order: (get-text-property POSITION PROP &optional OBJECT).
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let prop = want_sym(i, &a[1])?;
    Ok(prop_at(i, pos, prop))
}

fn f_text_properties_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let b = cur(i);
    let bb = b.borrow();
    // Collect props in write order (later overrides earlier).
    let mut plist: Vec<Value> = Vec::new();
    for tp in &bb.text_props {
        if pos >= tp.start && pos < tp.end {
            let psym = i.sym(tp.prop);
            // remove earlier entry for same prop
            let mut k = 0;
            while k + 1 < plist.len() {
                if crate::lisp::builtins::eq_values(&plist[k], &psym) {
                    plist.drain(k..k + 2);
                } else {
                    k += 2;
                }
            }
            plist.push(psym);
            plist.push(tp.value.clone());
        }
    }
    if plist.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::list(plist))
    }
}

pub(crate) fn f_next_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let b = cur(i);
    let bb = b.borrow();
    // Any interval boundary after pos counts as a change.
    let mut next: Option<usize> = None;
    for tp in &bb.text_props {
        for cand in [tp.start, tp.end] {
            if cand > pos {
                next = Some(next.map(|n| n.min(cand)).unwrap_or(cand));
            }
        }
    }
    match next {
        Some(p) => Ok(Value::Int(p as i128 + 1)),
        None => match a.get(1) {
            Some(v) if v.truthy() => Ok(a[1].clone()),
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
    let pos_v = a[0].int().unwrap_or(0);
    let prop = want_sym(i, &a[1])?;
    let object = a.get(2);
    let limit = a.get(3);

    // String object: our strings carry no properties, so the value at
    // every position is nil — nothing ever changes; return LIMIT.
    if let Some(Value::Str(s)) = object {
        let len = s.borrow().chars().count() as i128;
        let default = if forward { len } else { 0 };
        return Ok(match limit {
            Some(v) if v.truthy() => v.clone(),
            _ => Value::Int(default),
        });
    }
    let b = match object {
        Some(v) if v.truthy() => buf_of(i, v)?,
        _ => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len() as i128;
    // pos is a 1-based Lisp position; char index = pos-1.
    let pos = pos_v.clamp(1, len + 1);
    let at = |p: i128| -> Value {
        if p < 1 || p > len {
            return Value::Nil;
        }
        for tp in bb.text_props.iter().rev() {
            if tp.prop == prop && (p as usize - 1) >= tp.start && (p as usize - 1) < tp.end
            {
                return tp.value.clone();
            }
        }
        Value::Nil
    };
    let default_limit = if forward { len + 1 } else { 1 };
    let lim = match limit {
        Some(v) if v.truthy() => v.int().unwrap_or(default_limit),
        _ => default_limit,
    };
    if forward {
        let mut p = pos + 1;
        while p <= lim {
            if !crate::lisp::builtins::eq_values(&at(p), &at(p - 1)) {
                return Ok(Value::Int(p));
            }
            p += 1;
        }
        Ok(Value::Int(lim))
    } else {
        // Last change strictly before POS: a boundary at P counts when
        // at(P) != at(P-1); GNU never returns POS itself.
        let mut p = pos - 1;
        while p > lim && p >= 2 {
            if !crate::lisp::builtins::eq_values(&at(p), &at(p - 1)) {
                return Ok(Value::Int(p));
            }
            p -= 1;
        }
        Ok(Value::Int(lim))
    }
}

fn f_next_single_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    single_prop_change(i, &a, true)
}

fn f_prev_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let b = cur(i);
    let bb = b.borrow();
    let mut prev: Option<usize> = None;
    for tp in &bb.text_props {
        for cand in [tp.start, tp.end] {
            if cand < pos {
                prev = Some(prev.map(|n| n.max(cand)).unwrap_or(cand));
            }
        }
    }
    match prev {
        Some(p) => Ok(Value::Int(p as i128 + 1)),
        None => match a.get(1) {
            Some(v) if v.truthy() => Ok(a[1].clone()),
            _ => Ok(Value::Nil),
        },
    }
}

fn f_prev_single_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    single_prop_change(i, &a, false)
}

fn f_propertize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // We don't attach props to string objects; return the string.
    match &a[0] {
        Value::Str(_) => Ok(a[0].clone()),
        other => Err(i.wrong_type_mut("stringp", other)),
    }
}

// ---------- undo ----------

fn f_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1).max(1) as usize;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for _ in 0..n {
        // Undo one "change group": entries up to the last Boundary.
        // First, if the last entry is a Boundary, pop it.
        if matches!(bb.undo.last(), Some(crate::buffer::UndoEntry::Boundary)) {
            bb.undo.pop();
        }
        // Collect entries until we hit a Boundary.
        let mut group = Vec::new();
        while let Some(e) = bb.undo.pop() {
            if matches!(e, crate::buffer::UndoEntry::Boundary) {
                break;
            }
            group.push(e);
        }
        if group.is_empty() {
            // Nothing left to undo.
            let msg = if bb.undo_enabled {
                "No further undo information"
            } else {
                "No undo information in this buffer"
            };
            drop(bb);
            let sym = i.intern("user-error");
            return Err(i.signal_data(sym, vec![Value::string(msg)]));
        }
        // Apply in reverse.
        for e in group.into_iter().rev() {
            apply_undo(&mut bb, &e);
        }
    }
    Ok(Value::Nil)
}

fn f_delete_blank_lines(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Emacs: on a blank line, delete all contiguous blank lines
    // (leaving just one when there are several). On a nonblank line,
    // delete all blank lines following it.
    let _ = i;
    let b = cur(i);
    let mut bb = b.borrow_mut();
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
    bb.delete_region(s, e);
    let np = s.min(bb.text.len());
    bb.set_point(np);
    Ok(Value::Nil)
}

fn f_undo_start(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Push an undo boundary so the next `undo` treats preceding
    // records as one group (like Emacs's undo-start).
    let b = cur(i);
    b.borrow_mut().undo.push(crate::buffer::UndoEntry::Boundary);
    let _ = i;
    Ok(Value::Nil)
}

fn apply_undo(bb: &mut crate::buffer::Buffer, e: &crate::buffer::UndoEntry) {
    match e {
        crate::buffer::UndoEntry::Insertion { start, end } => {
            let s = (*start).min(bb.text.len());
            let en = (*end).min(bb.text.len());
            bb.text.delete(s, en);
        }
        crate::buffer::UndoEntry::Deletion { pos, text } => {
            let s = *pos;
            bb.text.insert(s.min(bb.text.len()), text);
        }
        crate::buffer::UndoEntry::Point(p) => {
            bb.point = (*p).min(bb.text.len());
        }
        crate::buffer::UndoEntry::Boundary => {}
    }
    if bb.zv > bb.text.len() {
        bb.zv = bb.text.len();
    }
}

fn f_primitive_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (primitive-undo N LIST)
    let n = a[0].int().unwrap_or(0).max(0) as usize;
    let items = a[1].list_to_vec().unwrap_or_default();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let mut applied = 0;
    let mut rest = Vec::new();
    for (k, item) in items.iter().enumerate() {
        if applied >= n {
            rest = items[k..].to_vec();
            break;
        }
        match item {
            Value::Nil => {}
            Value::Cons(c) => {
                let (car, cdr) = {
                    let cc = c.borrow();
                    (cc.car.clone(), cc.cdr.clone())
                };
                match (car.int(), cdr.int()) {
                    (Some(s), Some(e)) => {
                        // (beg . end) insertion
                        let s0 = (s - 1).max(0) as usize;
                        let e0 = (e - 1).max(0) as usize;
                        let tlen = bb.text.len();
                        bb.text.delete(s0.min(tlen), e0.min(tlen));
                        applied += 1;
                    }
                    _ => {
                        // (text . pos) deletion
                        if let Value::Str(t) = &car {
                            let p = cdr.int().unwrap_or(1).max(1) as usize - 1;
                            let tlen = bb.text.len();
                            bb.text.insert(p.min(tlen), &t.borrow());
                            applied += 1;
                        }
                    }
                }
            }
            Value::Int(p) => {
                bb.point = (*p - 1).max(0) as usize;
                applied += 1;
            }
            _ => {}
        }
    }
    bb.zv = bb.text.len();
    Ok(Value::list(rest))
}

// ---------- misc ----------

fn f_gap_position(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(cur(i).borrow().point() as i128 + 1))
}
fn f_gap_size(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(64))
}
fn f_position_bytes(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Chars == bytes in our model (multibyte not implemented).
    Ok(a[0].clone())
}
fn f_byte_to_position(_i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(a[0].clone())
}
fn f_max_char(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(0x3fffff))
}
fn f_barf_if_buffer_read_only(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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
