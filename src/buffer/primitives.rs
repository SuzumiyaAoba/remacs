//! Buffer primitives: the DEFUNs operating on buffers, point, mark,
//! markers, narrowing, buffer-local variables, search/match, text
//! properties, and undo.
//!
//! Position convention: Emacs positions are 1-based; internally the
//! `Buffer` uses 0-based char indices. `pt` = `bb.point + 1`.

use crate::buffer::{file_truename, lock_file_name, lock_owner_string, Buffer, TextProp};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::eval::MatchData;
use crate::lisp::builtins::want_string;
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
        "clone-indirect-buffer",
        1,
        2,
        f_clone_indirect_buffer,
        "Create an indirect buffer cloning the current buffer."
    ),
    S!(
        "clone-buffer",
        0,
        1,
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
    S!("current-word", 0, 3, f_current_word, "Word at point."),
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
    S!("get-pos-property", 2, 3, f_get_text_property, ""),
    S!(
        "remove-list-of-text-properties",
        3,
        4,
        f_remove_list_of_text_properties,
        ""
    ),
    S!("text-property-any", 4, 5, f_text_property_any, ""),
    S!(
        "text-property-not-all",
        4,
        5,
        f_text_property_not_all,
        ""
    ),
    S!(
        "add-face-text-property",
        3,
        5,
        f_add_face_text_property,
        ""
    ),
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
    S!(
        "verify-visited-file-modtime",
        1,
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
    S!(
        "unlock-file",
        1,
        1,
        f_unlock_file,
        "Unlock FILE."
    ),
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

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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

fn f_verify_visited_file_modtime(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_clear_visited_file_modtime(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.file_modtime_ns = -2;
    bb.file_modtime_size = -1;
    Ok(Value::Nil)
}

fn f_visited_file_modtime(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: negative ns is a flag — visited-file-modtime returns
    // UNKNOWN_MODTIME_NSECS - ns (0 when unknown, -1 when file missing).
    if bb.file_modtime_ns < 0 {
        return Ok(Value::Int(-2 - bb.file_modtime_ns));
    }
    Ok(crate::lisp::builtins::misc::ns_to_lisp_time(bb.file_modtime_ns))
}

fn f_set_visited_file_modtime(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
                // GNU: a file name that doesn't stat records the
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

/// Parse a lock-file target `USER@HOST.PID' or `USER@HOST.PID:BOOT'
/// into (user, host, pid); None if malformed.
fn parse_lock_target(target: &str) -> Option<(String, String, u32)> {
    let (user, rest) = target.split_once('@')?;
    let (host, pid) = rest.rsplit_once('.')?;
    let pid = pid.split(':').next()?.parse().ok()?;
    Some((user.to_string(), host.to_string(), pid))
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
/// stale (dead pid on this host, or unparseable target).
fn lock_owner(target: &str) -> Result<LockOwner, ()> {
    let (user, host, pid) = parse_lock_target(target).ok_or(())?;
    let same_host = host == crate::buffer::our_host_name();
    if same_host && pid == std::process::id() {
        return Ok(LockOwner::Ours);
    }
    if same_host {
        // Same host: check whether the process is still alive.
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
        (bb.modified, bb.file_name.clone())
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

fn f_lock_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU lock-buffer: "Lock FILE, if current buffer is modified."
    // The modified check precedes the FILE argument handling.
    {
        let b = cur(i);
        let bb = b.borrow();
        if !bb.modified {
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

fn f_unlock_buffer(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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

fn f_file_locked_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
fn f_lock_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
fn f_unlock_file(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
fn f_file_acl(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _file = want_string(i, &a[0])?;
    Ok(Value::Nil)
}

fn f_ask_user_about_lock(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU prompts interactively; in batch or when the file isn't
    // locked by another Emacs, this signals `file-locked'.
    let file = a[0].clone();
    let other = a.get(1).cloned().unwrap_or(Value::Nil);
    Err(signal_file_locked(i, file, other))
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
fn f_object_intervals(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
                        if pl.is_empty() { Value::Nil } else { Value::list(pl) },
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
        // GNU's Fget_buffer signals stringp for non-buffer/non-name args.
        other => Err(i.wrong_type_mut("stringp", other)),
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
    // GNU kill-buffer unlocks the killed buffer's file lock.
    if let Some(b) = i.buffers.get(id) {
        b.borrow_mut().release_lock_file();
    }
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
        None | Some(Value::Nil) => i.current_buffer,
        Some(v) => i
            .buffer_id_of(v)
            .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&v))))?,
    };
    // GNU Fkill_buffer: first run `kill-buffer-query-functions' (each
    // returning nil aborts the kill), then run the buffer-local
    // `kill-buffer-hook' with the doomed buffer current.
    let prev = i.current_buffer;
    i.set_current_buffer(id);
    let outcome = (|| -> Result<bool, Flow> {
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
        Ok(true) => Ok(Value::from_bool(kill_buffer_keep_current(i, id))),
        Ok(false) => Ok(Value::Nil),
        Err(e) => Err(e),
    }
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
    // GNU: Fget_buffer signals `stringp' for non-buffer/non-name args;
    // a missing name is a plain "No buffer named" error.
    if !matches!(a[0], Value::Buffer(_) | Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &a[0]));
    }
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
    // GNU buffer.c: when `uniquify-buffer-name-style' is non-nil, the
    // subr hands the rename to `uniquify--rename-buffer-advice' (which
    // may rename the buffer again, adding directory components).  The
    // variable is preset non-nil here, so guard on fboundp too.
    let ustyle = i.intern_soft("uniquify-buffer-name-style");
    if ustyle
        .map(|s| i.symbol_value(s).truthy())
        .unwrap_or(false)
    {
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
    if flag {
        barf_if_file_locked(i)?;
    }
    cur(i).borrow_mut().note_modified(flag);
    Ok(Value::Nil)
}

fn f_not_modified(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let flag = arg(&a, 0).truthy();
    if flag {
        barf_if_file_locked(i)?;
    }
    cur(i).borrow_mut().note_modified(flag);
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
            n.begv = bb.begv;
            n.zv = bb.zv;
            n.locals = bb.locals.clone();
            n.file_name = bb.file_name.clone();
        }
    }
    Ok(i.buffer_value(id).unwrap_or(Value::Nil))
}

/// GNU `clone-indirect-buffer'/`clone-buffer': an indirect buffer on
/// the current buffer with state cloned (locals, point, narrowing);
/// the name is uniquified like `generate-new-buffer-name'.
fn clone_buffer(i: &mut Interp, name: &str) -> Value {
    let base_id = i.current_buffer;
    let id = i.buffers.create(name);
    {
        let base = i.buffers.get(base_id).unwrap();
        let bb = base.borrow();
        let nb = i.buffers.get(id).unwrap();
        let mut n = nb.borrow_mut();
        n.text = bb.text.clone();
        n.base_buffer = Some(base_id);
        n.point = bb.point;
        n.mark = bb.mark;
        n.begv = bb.begv;
        n.zv = bb.zv;
        n.locals = bb.locals.clone();
        n.file_name = bb.file_name.clone();
    }
    i.buffer_value(id).unwrap_or(Value::Nil)
}

fn f_clone_indirect_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let name = want_string(i, &a[0])?;
    Ok(clone_buffer(i, &name))
}

fn f_clone_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU: NEWNAME defaults to the current buffer's name; the result
    // is uniquified in either case.
    let name = match a.first() {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(other) if !other.is_nil() => return Err(i.wrong_type_mut("stringp", other)),
        _ => cur(i).borrow().name.clone(),
    };
    Ok(clone_buffer(i, &name))
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

pub(crate) fn f_kill_all_local_variables(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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
    // GNU `DEFVAR_PER_BUFFER' variables are local in every buffer.
    if i.obarray.symbol(sid).always_local {
        return Ok(Value::t());
    }
    let b = buf_of(i, &arg(&a, 1))?;
    Ok(Value::from_bool(b.borrow().locals.contains_key(&sid)))
}

fn f_local_variable_if_set_p(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let sym = i.obarray.symbol(sid);
    if sym.make_local_if_set || sym.always_local {
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
    Ok(i.current_case_table())
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

fn f_copy_case_table(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_buffer_disable_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU defun: (with-current-buffer buffer (setq buffer-undo-list t))
    let b = buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    bb.set_undo_list(Value::t());
    Ok(Value::t())
}

fn f_buffer_enable_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // GNU subr: reset buffer-undo-list to nil only when disabled (t).
    let b = buf_of(i, &arg(&a, 0))?;
    let mut bb = b.borrow_mut();
    if bb.undo_disabled() {
        bb.set_undo_list(Value::Nil);
    }
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
    // GNU scan_words: word constituents = syntax class 'w' in the
    // buffer's syntax table.  Fetch the chain before borrowing the
    // buffer (the lookup itself touches the buffer).
    let syn = crate::editor::syntax_table_entries(i);
    let wordp = |c: char| crate::editor::syntax_entry_code(syn.as_ref(), c) == b'w';
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // GNU returns t on success, nil when point can't move (buffer edge).
    let mut ok = true;
    if n >= 0 {
        for _ in 0..n {
            let mut p = bb.point();
            let len = bb.text_len();
            // skip non-word, then word
            while p < len && !wordp(bb.text.char_at(p)) {
                p += 1;
            }
            while p < len && wordp(bb.text.char_at(p)) {
                p += 1;
            }
            if p == bb.point() {
                ok = false;
            }
            bb.set_point(p);
        }
    } else {
        for _ in 0..-n {
            let mut p = bb.point();
            while p > bb.begv && !wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            while p > bb.begv && wordp(bb.text.char_at(p - 1)) {
                p -= 1;
            }
            if p == bb.point() {
                ok = false;
            }
            bb.set_point(p);
        }
    }
    Ok(Value::from_bool(ok))
}

fn f_backward_word(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let n = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    f_forward_word(i, vec![Value::Int(-n)])
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
    // nil outside the accessible portion (GNU checks BEGV <= p < ZV).
    if p < bb.begv || p >= bb.text_len() || p >= bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(bb.text.char_at(p))))
}

fn f_char_before(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = match a.get(0) {
        Some(Value::Marker(m)) => m.borrow().position,
        Some(v) if v.truthy() => pos_idx(bb.text.len(), want_int(i, v)?),
        _ => bb.point(),
    };
    if p <= bb.begv || p > bb.text_len() || p > bb.text.len() {
        return Ok(Value::Nil);
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(bb.text.char_at(p - 1))))
}

fn f_following_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    // GNU returns 0 (not nil) at the end of the accessible portion.
    if p >= bb.text_len() {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(bb.text.char_at(p))))
}

fn f_preceding_char(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    if p <= bb.begv {
        return Ok(Value::Int(0));
    }
    Ok(Value::Int(crate::lisp::value::lisp_char_code(bb.text.char_at(p - 1))))
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
    // GNU: if the greater position is not at the start of a line,
    // the partial line counts too.
    if e > s && bb.text.char_at(e - 1) != '\n' {
        n += 1;
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
            c => col += char_width(c),
        }
        k += 1;
    }
    Ok(Value::Int(col))
}

fn f_move_to_column(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let goal = want_int(i, &a[0])?.max(0);
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
        bb.insert_at(at, &s);
        bb.set_point(at + pad);
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
fn find_defun_start(
    syn: &crate::editor::Syn,
    text: &[char],
    stop: usize,
    pos: usize,
) -> usize {
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
        scan_sexps_fwd(syn, text, stop, ds, comment_end, &mut st, i128::MIN, false, 0);
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

fn f_skip_syntax_forward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let syn = crate::editor::Syn::current(i);
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

fn f_skip_syntax_backward(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let spec = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let syn = crate::editor::Syn::current(i);
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

fn f_forward_comment(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let count = a.get(0).and_then(|v| v.int()).unwrap_or(1);
    // Resolve the syntax table before the buffer borrow (the borrow
    // would make the lookup silently fall back to the standard table).
    let syn = crate::editor::Syn::current(i);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let text: Vec<char> = bb.text.text().chars().collect();
    let beg = bb.begv;
    let stop = bb.zv.min(bb.text_len());
    let (np, ok) = forward_comment_scan(&syn, &text, beg, stop, bb.point(), count);
    bb.set_point(np);
    Ok(if ok { Value::Sym(sym::T) } else { Value::Nil })
}

/// Text of the current buffer plus point, as a char vec; also the
/// narrowed [BEGV,ZV) bounds (0-based).
fn nav_text(i: &Interp) -> (Vec<char>, usize, usize, usize) {
    let b = i.buffers.get(i.current_buffer).unwrap();
    let bb = b.borrow();
    let text: Vec<char> = bb.text.text().chars().collect();
    (text, bb.point(), bb.begv, bb.zv.min(bb.text_len()))
}

/// GNU `scan-lists': scan COUNT lists from FROM starting at DEPTH.
/// Returns the landing charpos, nil at the boundary between objects.
fn f_scan_lists(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_int(i, &a[0])?;
    let count = want_int(i, &a[1])?;
    let depth = want_int(i, &a[2])?;
    let syn = crate::editor::Syn::current(i);
    let (text, _, beg, stop) = nav_text(i);
    let pos = pos_idx(stop, from).max(beg);
    match scan_lists_gnu(&syn, &text, beg, stop, pos, count, depth as i64, false) {
        Ok(Some(p)) => Ok(Value::Int(p as i128 + 1)),
        Ok(None) => Ok(Value::Nil),
        Err(e) => Err(scan_err_flow(i, e)),
    }
}

fn f_scan_sexps(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let from = want_int(i, &a[0])?;
    let count = want_int(i, &a[1])?;
    let syn = crate::editor::Syn::current(i);
    let (text, _, beg, stop) = nav_text(i);
    let pos = pos_idx(stop, from).max(beg);
    match scan_lists_gnu(&syn, &text, beg, stop, pos, count, 0, true) {
        Ok(Some(p)) => Ok(Value::Int(p as i128 + 1)),
        Ok(None) => Ok(Value::Nil),
        Err(e) => Err(scan_err_flow(i, e)),
    }
}

/// GNU `syntax-after': (CLASS . MATCHING-CHAR) for the char at POS.
pub(crate) fn f_syntax_after(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pos = want_int(i, &a[0])?;
    let syn = crate::editor::Syn::current(i);
    let b = cur(i);
    let bb = b.borrow();
    let idx = pos_idx(bb.text_len(), pos);
    if idx >= bb.text_len() {
        return Ok(Value::Nil);
    }
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
    barf_if_file_locked(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if before_markers {
        bb.insert_before_markers(s);
    } else {
        bb.insert(s);
    }
    Ok(())
}

/// Copy a string arg's own text properties into the buffer for the
/// just-inserted range [base, base+len).  GNU `insert' copies the
/// string's properties verbatim (no inheritance from surrounding
/// text — that is `insert-and-inherit's job).
fn copy_str_props(i: &mut Interp, s: &crate::lisp::value::StrRef, base: usize) {
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
                bb.text_props.push(TextProp {
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

fn f_insert(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    for v in &a {
        match v {
            Value::Str(s) => {
                let t = s.borrow().clone();
                let n = t.chars().count();
                let base = cur(i).borrow().point();
                insert_str_at_point(i, &t, false)?;
                copy_str_props(i, s, base);
                debug_assert_eq!(cur(i).borrow().point(), base + n);
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
                let base = cur(i).borrow().point();
                insert_str_at_point(i, &t, true)?;
                copy_str_props(i, s, base);
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
    let (lo, hi) = (s.min(e), e.max(s));
    let sub: String = chars[lo..hi].iter().collect();
    // GNU copies the source range's text properties along with the
    // characters.
    let props: Vec<TextProp> = src
        .borrow()
        .text_props
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
            bb.text_props.push(tp);
        }
        bb.note_prop_modified();
    }
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
        // GNU `internal_self_insert': before inserting a character
        // with non-word syntax, expand the abbrev before point when
        // `abbrev-mode' is on; an abbrev hook with a non-nil
        // `no-self-insert' property suppresses the insertion.
        if n > 0 && self_insert_expand_abbrev(i, c)? {
            return Ok(Value::Nil);
        }
        let s: String = std::iter::repeat(c).take(n as usize).collect();
        insert_str_at_point(i, &s, false)?;
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
    barf_if_file_locked(i)?;
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
    barf_if_file_locked(i)?;
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
    // GNU (simple.el): (setq this-command 'kill-region) and likewise
    // for last-command — the last setq's value is the return value.
    let kr = Value::Sym(i.intern("kill-region"));
    for name in ["this-command", "last-command"] {
        let id = i.intern(name);
        i.obarray.symbol_mut(id).value = kr.clone();
    }
    Ok(kr)
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

fn buf_props_as_ivs(
    props: &[TextProp],
    s: usize,
    e: usize,
) -> Vec<(usize, usize, Vec<Value>)> {
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

fn buffer_substring_impl(i: &mut Interp, a: &[Value], with_props: bool) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let (s, e) = (s.min(e), s.max(e));
    let text = bb.text.substring(s, e);
    let ivs = if with_props {
        buf_props_as_ivs(&bb.text_props, s, e)
    } else {
        Vec::new()
    };
    drop(bb);
    let v = Value::string(text);
    if !ivs.is_empty() {
        if let Value::Str(sr) = &v {
            i.set_str_props(sr, ivs);
        }
    }
    Ok(v)
}

fn f_buffer_substring(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    buffer_substring_impl(i, &a, true)
}

fn f_buffer_substring_no_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    buffer_substring_impl(i, &a, false)
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
    let (s0, e0) = ((s - 1) as usize, (e - 1) as usize);
    let text = bb.text.substring(s0, e0);
    let ivs = buf_props_as_ivs(&bb.text_props, s0, e0);
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

fn f_current_word(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // GNU's `current-word' (simple.el) grabs syntaxes "w_" — word OR
    // symbol constituents — via the buffer's syntax table.
    let syn = crate::editor::syntax_table_entries(i);
    match thing_bounds(i, |c| {
        matches!(crate::editor::syntax_entry_code(syn.as_ref(), c), b'w' | b'_')
    }) {
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
fn deactivate_mark(i: &mut Interp, bb: &mut Buffer, force: bool) {
    if !(force || region_active(i, bb)) {
        return;
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
}

/// GNU `activate-mark' core: set mark-active; with NO-TMM leave
/// transient-mark-mode alone, else set it buffer-locally to 'lambda
/// when unset.  Only runs when the mark exists and region is inactive.
fn activate_mark(i: &Interp, bb: &mut Buffer, no_tmm: bool) {
    if bb.mark.is_none() || region_active(i, bb) {
        return;
    }
    bb.locals.insert(ma_id(i), Value::t());
    let tmm = tmm_id(i);
    if !(buf_var(i, bb, tmm).truthy() || no_tmm) {
        let lambda = i.intern_soft("lambda").unwrap_or(0);
        bb.locals.insert(tmm, Value::Sym(lambda));
    }
}

fn f_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: (or force (not tmm) mark-active mark-even-if-inactive) else
    // signal 'mark-inactive.
    let mei = i.intern_soft("mark-even-if-inactive").unwrap_or(0);
    let ok = a.get(0).map(|v| v.truthy()).unwrap_or(false)
        || !buf_var(i, &bb, tmm_id(i)).truthy()
        || mark_active(i, &bb)
        || buf_var(i, &bb, mei).truthy();
    match bb.mark {
        _ if !ok => Err(err_sym(i, "mark-inactive", vec![])),
        Some(m) => Ok(Value::Int(m as i128 + 1)),
        None => Ok(Value::Nil),
    }
}

fn f_set_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if a[0].truthy() {
        let len = bb.text.len();
        let p = match &a[0] {
            Value::Marker(m) => m.borrow().position,
            v => pos_idx(len, want_int(i, v)?),
        };
        bb.mark = Some(p);
        // (activate-mark 'no-tmm): mark-active on, tmm untouched.
        if !region_active(i, &bb) {
            bb.locals.insert(ma_id(i), Value::t());
        }
    } else {
        // (deactivate-mark t) then clear the mark in any mode.
        deactivate_mark(i, &mut bb, true);
        bb.locals.insert(ma_id(i), Value::Nil);
        bb.mark = None;
    }
    Ok(Value::Nil)
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
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    // GNU pushes the OLD mark onto mark-ring before installing the new
    // one: (add-to-history 'mark-ring (copy-marker (mark-marker)) max).
    if let Some(old) = bb.mark {
        let mut items = bb
            .locals
            .get(&ring_sym)
            .cloned()
            .unwrap_or(Value::Nil)
            .list_to_vec()
            .unwrap_or_default();
        let m = Rc::new(RefCell::new(Marker {
            buffer: Some(bb.id),
            position: old,
            insertion_type: false,
        }));
        bb.register_marker(&m);
        items.insert(0, marker_value(m));
        let max = buf_var(i, &bb, i.intern_soft("mark-ring-max").unwrap_or(0))
            .int()
            .unwrap_or(16) as usize;
        items.truncate(max);
        bb.locals.insert(ring_sym, Value::list(items));
    }
    bb.mark = Some(p);
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

fn f_pop_mark(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let ring_sym = i.intern_soft("mark-ring").unwrap_or(0);
    let mut items = bb
        .locals
        .get(&ring_sym)
        .cloned()
        .unwrap_or(Value::Nil)
        .list_to_vec()
        .unwrap_or_default();
    if !items.is_empty() {
        // GNU: (nconc mark-ring (list (copy-marker (mark-marker)))) then
        // take (car mark-ring) — the ring is a rotation, not a stack.
        if let Some(mpos) = bb.mark {
            let m = Rc::new(RefCell::new(Marker {
                buffer: Some(bb.id),
                position: mpos,
                insertion_type: false,
            }));
            bb.register_marker(&m);
            items.push(marker_value(m));
        }
        if let Value::Marker(mm) = &items[0] {
            bb.mark = Some(mm.borrow().position.min(bb.text.len()));
        }
        // (set-marker (car mark-ring) nil): kill the popped marker.
        if let Value::Marker(mm) = &items[0] {
            mm.borrow_mut().buffer = None;
        }
        items.remove(0);
        bb.locals.insert(ring_sym, Value::list(items));
    }
    // GNU's pop-mark calls (deactivate-mark) unconditionally; it is
    // itself gated on (region-active-p).
    deactivate_mark(i, &mut bb, false);
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
        let nv = Value::float(**old + nf);
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

/// GNU's `char_width' (character.c): the display column width of one
/// buffer character.  Control chars (with `ctl-arrow' t) render as
/// `^X' = 2 columns; C1 chars render as octal `\NNN' = 4; everything
/// else comes from `char-width-table' (CHAR_WIDTH_RANGES), which also
/// covers zero-width (tag) chars and East-Asian double-width chars.
/// TAB and newline are handled by the caller.
pub(crate) fn char_width(c: char) -> i128 {
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

/// GNU signals `(error "The mark is not set now, so there is no region")'
/// from region-beginning/region-end when the buffer has no mark.
fn no_region_err(i: &mut Interp) -> Flow {
    let e = i.intern("error");
    i.signal_data(
        e,
        vec![Value::string("The mark is not set now, so there is no region")],
    )
}

fn f_region_beginning(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark {
        Some(m) => Ok(Value::Int((m.min(bb.point()) + 1) as i128)),
        None => Err(no_region_err(i)),
    }
}

fn f_region_end(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    match bb.mark {
        Some(m) => Ok(Value::Int((m.max(bb.point()) + 1) as i128)),
        None => Err(no_region_err(i)),
    }
}

fn f_region_active_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::from_bool(region_active(i, &bb)))
}

fn f_deactivate_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let force = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    deactivate_mark(i, &mut bb, force);
    Ok(Value::Nil)
}

fn f_activate_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let no_tmm = a.get(0).map(|v| v.truthy()).unwrap_or(false);
    activate_mark(i, &mut bb, no_tmm);
    Ok(Value::Nil)
}

fn f_exchange_point_and_mark(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    // GNU's defun: (mark t) omark; (set-mark (point)); (goto-char omark);
    // then deactivate or activate per ARG and the region state.
    let Some(omark) = bb.mark else {
        let ue = i.intern("user-error");
        return Err(i.signal_data(
            ue,
            vec![Value::string("No mark set in this buffer")],
        ));
    };
    let was_active = region_active(i, &bb);
    let tmmv = buf_var(i, &bb, tmm_id(i));
    let temp_highlight = car_is_only(&tmmv, i);
    let p = bb.point();
    bb.mark = Some(p);
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
    if arg != rhs {
        deactivate_mark(i, &mut bb, false);
    } else {
        activate_mark(i, &mut bb, false);
    }
    Ok(Value::Nil)
}

fn f_use_region_p(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    // GNU: (and (region-active-p) (or (/= beg end) (and
    // use-empty-active-region (not down-mouse-1) (not mouse-movement)))).
    if !region_active(i, &bb) {
        return Ok(Value::Nil);
    }
    // GNU calls (region-end)/(region-beginning) here, which signal when
    // mark-active is non-nil but the mark was never set.
    let Some(m) = bb.mark else {
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
    let ok = !matches!(&car, Value::Sym(s) if *s == dm1)
        && !matches!(&car, Value::Sym(s) if *s == mm);
    Ok(Value::from_bool(ok))
}

// ---------- narrowing ----------

fn f_narrow_to_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let sl = want_int(i, &a[0])?;
    let el = want_int(i, &a[1])?;
    // GNU validates against the unrestricted bounds and signals
    // args-out-of-range with the original arguments.
    let (sl, el) = (sl.min(el), sl.max(el));
    if !(1 <= sl && sl <= el && el <= len as i128 + 1) {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    let (s, e) = ((sl - 1) as usize, (el - 1) as usize);
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

fn f_labeled_narrow_to_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (BEG END LABEL): narrow like narrow-to-region, recording the
    // previous bounds under LABEL for internal--labeled-widen.
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text.len();
    let sl = want_int(i, &a[0])?;
    let el = want_int(i, &a[1])?;
    let (sl, el) = (sl.min(el), sl.max(el));
    if !(1 <= sl && sl <= el && el <= len as i128 + 1) {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    let (s, e) = ((sl - 1) as usize, (el - 1) as usize);
    let (pb, pz) = (bb.begv, bb.zv);
    bb.begv = s;
    bb.zv = e;
    if bb.point < s || bb.point > e {
        bb.point = s;
    }
    bb.narrow_labels.push((pb, pz, a[2].clone()));
    Ok(Value::Nil)
}

fn f_labeled_widen(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_set_buffer_modified_tick(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (TICK &optional BUFFER)
    let tick = match &a[0] {
        Value::Int(n) if *n >= 0 => *n as u64,
        _ => return Err(i.wrong_type_mut("wholenump", &a[0])),
    };
    let b = buf_of(i, &crate::lisp::builtins::arg(&a, 1))?;
    b.borrow_mut().mod_tick = tick;
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
    let bound = match a.get(1) {
        Some(Value::Nil) | None => None,
        Some(v) => Some(want_int(i, v)?),
    };
    let noerror = a.get(2).map(|v| v.truthy()).unwrap_or(false);
    let count = match a.get(3) {
        Some(Value::Nil) | None => 1,
        Some(v) => want_int(i, v)?,
    };
    // GNU: a negative COUNT reverses the search direction (and BOUND
    // then limits the flipped direction).
    let backward = backward != (count < 0);
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
    if let Some(Value::Str(s)) = a.get(4) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        str_pos_ok(i, &a[4], st.max(en), len)?;
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
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let prop = want_sym(i, &a[2])?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    buf_record_prop(&mut bb, s, e, &a[2], prop, &a[3]);
    bb.text_props.push(TextProp {
        start: s,
        end: e,
        prop,
        value: a[3].clone(),
    });
    bb.note_prop_modified();
    Ok(Value::Nil)
}

/// GNU `record_property_change' over [S,E) (0-based): push one
/// `(nil PROP OLD BEG . END)' undo entry per contiguous run of OLD
/// values that are not `eq' NEW.
fn buf_record_prop(
    bb: &mut Buffer,
    s: usize,
    e: usize,
    prop: &Value,
    pid: u32,
    new: &Value,
) {
    let mut p = s;
    while p < e {
        let old = bb.prop_value_at(p, pid);
        if crate::lisp::builtins::eq_values(&old, new) {
            p += 1;
            continue;
        }
        let mut q = p + 1;
        while q < e && crate::lisp::builtins::eq_values(&bb.prop_value_at(q, pid), &old) {
            q += 1;
        }
        bb.record_prop_change(prop.clone(), old, p + 1, q + 1);
        p = q;
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
    let cat = i.intern_soft("category")?;
    match str_plist_get(pl, cat, i) {
        Some(Value::Sym(c)) => match i.get_prop(c, id) {
            Value::Nil => None,
            v => Some(v),
        },
        _ => None,
    }
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
fn str_plist_at(ivs: &[(usize, usize, Vec<Value>)], p: usize) -> Vec<Value> {
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

fn f_add_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let plist = a[2].list_to_vec().unwrap_or_default();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    buf_add_props(&mut bb, i, s, e, &plist, true)
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
            bb.text_props.push(TextProp {
                start: s,
                end: e,
                prop: p,
                value: plist[k + 1].clone(),
            });
        }
        k += 2;
    }
    bb.note_prop_modified();
    Ok(Value::from_bool(changed))
}

/// True when every position in [S,E) already carries PROP == VAL.
fn buf_prop_uniform(
    bb: &Buffer,
    s: usize,
    e: usize,
    prop: u32,
    val: &Value,
) -> bool {
    for p in s..e.max(s) {
        let cur = bb
            .text_props
            .iter()
            .rev()
            .find(|tp| tp.prop == prop && tp.start <= p && tp.end > p)
            .map(|tp| tp.value.clone())
            .unwrap_or(Value::Nil);
        if !crate::lisp::builtins::eq_values(&cur, val) {
            return false;
        }
    }
    true
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

pub(crate) fn f_remove_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(Value::Str(_)) = a.get(3) {
        // PROPERTIES is a plist; remove the named keys.
        let plist = a[2].list_to_vec().unwrap_or_default();
        let names: Vec<Value> = plist.into_iter().step_by(2).collect();
        let removed = str_remove_props(i, &a, names)?;
        return Ok(Value::from_bool(removed));
    }
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
    for (n, &p) in props.iter().enumerate() {
        buf_record_prop(&mut bb, s, e, &plist[n * 2], p, &Value::Nil);
    }
    let before = bb.text_props.len();
    bb.text_props
        .retain(|tp| !(props.contains(&tp.prop) && tp.start < e && tp.end > s));
    bb.note_prop_modified();
    Ok(Value::from_bool(bb.text_props.len() != before))
}

fn f_set_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(Value::Str(s)) = a.get(3) {
        let s = s.clone();
        let len = str_len(&s);
        let st = want_int(i, &a[0])?.max(0) as usize;
        let en = want_int(i, &a[1])?.max(0) as usize;
        str_pos_ok(i, &a[3], st.max(en), len)?;
        let (s0, e0) = (st.min(en), st.max(en));
        let plist = a[2].list_to_vec().unwrap_or_default();
        let fill = if plist.is_empty() { None } else { Some(plist.clone()) };
        let mut ivs = std::mem::take(i.str_props_mut(&s));
        // GNU replaces the interval's plist wholesale.
        iv_apply(&mut ivs, s0, e0, |pl| *pl = plist.clone(), fill);
        i.set_str_props(&s, ivs);
        return Ok(Value::t());
    }
    // Remove all props in range, then add the plist.
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let plist = a[2].list_to_vec().unwrap_or_default();
    let b = cur(i);
    {
        let mut bb = b.borrow_mut();
        buf_record_set(&mut bb, i, s, e, &plist);
        bb.text_props
            .retain(|tp| !(tp.start < e && tp.end > s));
    }
    {
        let mut bb = b.borrow_mut();
        buf_add_props(&mut bb, i, s, e, &plist, false)?;
    }
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
                || !pl
                    .iter()
                    .all(|(k, v)| old.get(k).is_some_and(|o| crate::lisp::builtins::eq_values(o, v)))
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

pub(crate) fn prop_at(i: &mut Interp, pos: usize, prop: u32) -> Value {
    let cat = i.intern_soft("category");
    let b = cur(i);
    let bb = b.borrow();
    // Last write wins.
    let mut catval = Value::Nil;
    for tp in bb.text_props.iter().rev() {
        if pos >= tp.start && pos < tp.end {
            if tp.prop == prop {
                return tp.value.clone();
            }
            if Some(tp.prop) == cat && catval.is_nil() {
                catval = tp.value.clone();
            }
        }
    }
    // GNU `lookup_char_property' fallback: consult the `category'
    // symbol's plist when PROP isn't present directly.
    if cat.is_some() {
        if let Value::Sym(cs) = catval {
            return i.get_prop(cs, prop);
        }
    }
    Value::Nil
}

pub(crate) fn f_get_text_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Emacs arg order: (get-text-property POSITION PROP &optional OBJECT).
    if let Some(Value::Str(s)) = a.get(2) {
        let pos = want_int(i, &a[0])?.max(0) as usize;
        let prop = want_sym(i, &a[1])?;
        let pl = str_plist_at(i.str_props(s), pos);
        return Ok(str_plist_get_cat(&pl, prop, i).unwrap_or(Value::Nil));
    }
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let prop = want_sym(i, &a[1])?;
    Ok(prop_at(i, pos, prop))
}

fn f_text_properties_at(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(Value::Str(s)) = a.get(1) {
        let pos = want_int(i, &a[0])?.max(0) as usize;
        let pl = str_plist_at(i.str_props(s), pos);
        return Ok(if pl.is_empty() { Value::Nil } else { Value::list(pl) });
    }
    let len = cur(i).borrow().text.len();
    let pos = pos_idx(len, want_int(i, &a[0])?);
    let b = cur(i);
    let bb = b.borrow();
    // GNU's plist is newest-first (put-text-property prepends):
    // iterate records in reverse and keep the newest value per prop.
    let mut plist: Vec<Value> = Vec::new();
    for tp in bb.text_props.iter().rev() {
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

pub(crate) fn f_next_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(Value::Str(_)) = a.get(1) {
        return str_prop_change(i, &a, true);
    }
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
        None => match a.get(2) {
            Some(v) if v.truthy() => Ok(a[2].clone()),
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
    // GNU accepts an integer or a marker for POSITION and LIMIT.
    let pos_v = want_int(i, &a[0])?;
    let prop = want_sym(i, &a[1])?;
    let object = a.get(2);
    let limit = a.get(3);

    // String object: 0-based positions; nil (not LIMIT) when nothing
    // changes — LIMIT is only returned when it's non-nil.
    if let Some(Value::Str(s)) = object {
        let len = str_len(s) as i128;
        // Compute LIMIT before borrowing the interval set.
        let str_lim = match limit {
            Some(v) if v.truthy() => Some(want_int(i, v)?),
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
        let pos = pos_v.clamp(0, len);
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
    let buf_lim = match limit {
        Some(v) if v.truthy() => Some(want_int(i, v)?),
        _ => None,
    };
    let b = match object {
        Some(v) if v.truthy() => buf_of(i, v)?,
        _ => cur(i),
    };
    let bb = b.borrow();
    let len = bb.text.len() as i128;
    // pos is a 1-based Lisp position; char index = pos-1.
    let pos = pos_v.clamp(1, len + 1);
    let cat = i.intern_soft("category");
    let at = |p: i128| -> Value {
        if p < 1 || p > len {
            return Value::Nil;
        }
        let mut catval = Value::Nil;
        for tp in bb.text_props.iter().rev() {
            if (p as usize - 1) >= tp.start && (p as usize - 1) < tp.end {
                if tp.prop == prop {
                    return tp.value.clone();
                }
                if Some(tp.prop) == cat && catval.is_nil() {
                    catval = tp.value.clone();
                }
            }
        }
        // GNU `textget' is category-aware.
        if let Value::Sym(cs) = catval {
            return i.get_prop(cs, prop);
        }
        Value::Nil
    };
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
    if forward {
        let mut p = pos + 1;
        while p <= lim {
            if !crate::lisp::builtins::eq_values(&at(p), &at(p - 1)) {
                return Ok(Value::Int(p));
            }
            p += 1;
        }
        Ok(no_change())
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
        Ok(no_change())
    }
}

fn f_next_single_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    single_prop_change(i, &a, true)
}

pub(crate) fn f_prev_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    if let Some(Value::Str(_)) = a.get(1) {
        return str_prop_change(i, &a, false);
    }
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
        None => match a.get(2) {
            Some(v) if v.truthy() => Ok(a[2].clone()),
            _ => Ok(Value::Nil),
        },
    }
}

fn f_prev_single_property_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    single_prop_change(i, &a, false)
}

pub(crate) fn f_remove_list_of_text_properties(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (START END LIST-OF-PROPERTIES &optional OBJECT)
    let names = a[2].list_to_vec().unwrap_or_default();
    if let Some(Value::Str(_)) = a.get(3) {
        let removed = str_remove_props(i, &a, names)?;
        return Ok(Value::from_bool(removed));
    }
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let props: Vec<u32> = names.iter().filter_map(|v| i.sym_id(v)).collect();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    for (n, &p) in props.iter().enumerate() {
        buf_record_prop(&mut bb, s, e, &names[n], p, &Value::Nil);
    }
    let before = bb.text_props.len();
    bb.text_props
        .retain(|tp| !(props.contains(&tp.prop) && tp.start < e && tp.end > s));
    bb.note_prop_modified();
    Ok(Value::from_bool(bb.text_props.len() != before))
}

/// Shared engine for `text-property-any' and `text-property-not-all':
/// (START END PROP VALUE &optional OBJECT) → first pos in [S,E) whose
/// PROP is (not-)eq VALUE.
fn text_prop_any(i: &mut Interp, a: &[Value], any: bool) -> EvalResult {
    let prop = want_sym(i, &a[2])?;
    let want = a[3].clone();
    if let Some(Value::Str(s)) = a.get(4) {
        let len = str_len(s) as i128;
        let st = want_int(i, &a[0])?.clamp(0, len);
        let en = want_int(i, &a[1])?.clamp(0, len);
        let ivs = i.str_props(s);
        let mut p = st;
        while p < en {
            let pl = str_plist_at(ivs, p as usize);
            // GNU `textget' is category-aware.
            let v = str_plist_get_cat(&pl, prop, i).unwrap_or(Value::Nil);
            if crate::lisp::builtins::eq_values(&v, &want) == any {
                return Ok(Value::Int(p));
            }
            p += 1;
        }
        return Ok(Value::Nil);
    }
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let mut p = s;
    while p < e {
        let v = prop_at(i, p, prop);
        if crate::lisp::builtins::eq_values(&v, &want) == any {
            return Ok(Value::Int(p as i128 + 1));
        }
        p += 1;
    }
    Ok(Value::Nil)
}

fn f_text_property_any(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    text_prop_any(i, &a, true)
}

fn f_text_property_not_all(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    text_prop_any(i, &a, false)
}

/// `add-face-text-property': combine FACE into the `face' prop of each
/// interval in [S,E).  GNU dedups with memq; APPEND puts the new face
/// last, otherwise first.
pub(crate) fn f_add_face_text_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
            if fill.is_nil() { None } else { Some(vec![face_sym.clone(), fill]) },
        );
        i.set_str_props(&s, ivs);
        return Ok(Value::Nil);
    }
    let len = cur(i).borrow().text.len();
    let s = pos_idx(len, want_int(i, &a[0])?);
    let e = pos_idx(len, want_int(i, &a[1])?);
    let fid = want_sym(i, &face_sym)?;
    let cat = i.intern_soft("category");
    let mut p = s;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    while p < e {
        let mut old = Value::Nil;
        let mut catval = Value::Nil;
        for tp in bb.text_props.iter().rev() {
            if p >= tp.start && p < tp.end {
                if tp.prop == fid {
                    old = tp.value.clone();
                    break;
                }
                if Some(tp.prop) == cat && catval.is_nil() {
                    catval = tp.value.clone();
                }
            }
        }
        // GNU `textget' is category-aware.
        if old.is_nil() {
            if let Value::Sym(cs) = catval {
                old = i.get_prop(cs, fid);
            }
        }
        bb.text_props.push(TextProp {
            start: p,
            end: p + 1,
            prop: fid,
            value: combine(&old),
        });
        p += 1;
    }
    Ok(Value::Nil)
}

fn f_propertize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    match &a[0] {
        Value::Str(s) => {
            // GNU copies the string (with its intervals) and adds the
            // arg plist props over the whole length.
            let len = str_len(s);
            let ns = Rc::new(RefCell::new(s.borrow().clone()));
            let out = Value::Str(ns.clone());
            let mut ivs = i.str_props(s).to_vec();
            // GNU signals wrong-number-of-arguments on a dangling
            // property name.
            if a.len() % 2 == 0 {
                let pname = i.intern("propertize");
                return Err(i.signal_data(
                    crate::lisp::obarray::sym::WRONG_NUMBER_OF_ARGUMENTS,
                    vec![Value::Sym(pname), Value::Int(a.len() as i128)],
                ));
            }
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
                let rpairs: Vec<(Value, Value)> =
                    pairs.iter().rev().cloned().collect();
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
                        crate::lisp::builtins::misc::ns_to_lisp_time(bb.file_modtime_ns)
                    }
                };
                if undo_time_equal(i, &cdr, &vft) {
                    f_unlock_buffer(i, vec![])?;
                    f_set_buffer_modified_p(i, vec![Value::Nil])?;
                }
            } else if matches!(car, Value::Nil) {
                // (nil PROP VAL BEG . END): text property change.
                let (prop, val, tail) = {
                    let (p, r1) = cons_parts(&cdr).ok_or_else(|| undo_unrecognized(i, next))?;
                    let (v, r2) = cons_parts(&r1).ok_or_else(|| undo_unrecognized(i, next))?;
                    (p, v, r2)
                };
                let (beg, end) =
                    cons_parts(&tail).ok_or_else(|| undo_unrecognized(i, next))?;
                let (Some(beg), Some(end)) = (beg.int(), end.int()) else {
                    return Err(undo_unrecognized(i, next));
                };
                if undo_pos_oob(b, beg, end) {
                    return Err(undo_oob_err(i));
                }
                f_put_text_property(
                    i,
                    vec![Value::Int(beg), Value::Int(end), prop, val],
                )?;
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
                        let sm = f_copy_marker(i, vec![Value::Int(start), Value::Nil])?;
                        let em = f_copy_marker(i, vec![Value::Int(end), Value::t()])?;
                        i.apply(&fun, args)?;
                        let (sp, ep) = match (&sm, &em) {
                            (Value::Marker(s), Value::Marker(e)) => (
                                s.borrow().position as i128,
                                e.borrow().position as i128,
                            ),
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
                        f_set_marker(i, vec![sm, Value::Nil])?;
                        f_set_marker(i, vec![em, Value::Nil])?;
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
                let mut bb = b.borrow_mut();
                let s = (beg - 1).max(0) as usize;
                bb.set_point(s);
                bb.delete_region((beg - 1).max(0) as usize, (end - 1).max(0) as usize);
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
                            Some((Value::Marker(_), cdr))
                                if cdr.int().is_some() =>
                            {
                                (e, rest)
                            }
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
                    let mut bb = b.borrow_mut();
                    if pos < 0 {
                        bb.set_point((-pos - 1).max(0) as usize);
                        bb.insert(&text.borrow());
                    } else {
                        bb.set_point((pos - 1).max(0) as usize);
                        bb.insert(&text.borrow());
                        bb.set_point((pos - 1).max(0) as usize);
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
                let _ = crate::lisp::builtins::evalfn::f_warn(
                    i,
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

fn f_primitive_undo(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (primitive-undo N LIST) — GNU defun (simple.el): N counts undo
    // boundaries crossed, not entries.  Returns the remaining list.
    let mut arg = a[0].int().unwrap_or(0);
    let mut list = a[1].clone();
    let b = cur(i);
    let oldlist = b.borrow().undo_list();
    let mut did_apply = false;
    while arg > 0 {
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
        arg -= 1;
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
