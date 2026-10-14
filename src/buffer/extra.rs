//! Extra buffer subrs: region commands, text-property scans, fields,
//! indentation helpers, and encoding on regions.

use crate::buffer::primitives::{buf_of, check_writable, cur, err_sym, pos_idx};
use crate::lisp::Interp;
use crate::lisp::builtins::{S, arg, want_int, want_string};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "current-indentation",
        0,
        0,
        f_current_indentation,
        "Column of first nonblank char."
    ),
    S!(
        "sort-lines",
        2,
        3,
        f_sort_lines,
        "Sort lines in region alphabetically."
    ),
    S!(
        "sort-subr",
        3,
        6,
        f_sort_subr,
        "General text sorting routine to divide buffer into records and sort them."
    ),
    S!(
        "sort-fields",
        3,
        3,
        f_sort_fields,
        "Sort lines in region lexicographically by the ARGth field."
    ),
    S!(
        "sort-numeric-fields",
        3,
        3,
        f_sort_numeric_fields,
        "Sort lines in region numerically by the ARGth field."
    ),
    S!(
        "sort-regexp-fields",
        5,
        5,
        f_sort_regexp_fields,
        "Sort text in region lexicographically by regexp-selected records/keys."
    ),
    S!(
        "sort-columns",
        1,
        3,
        f_sort_columns,
        "Sort lines in region alphabetically by a range of columns."
    ),
    S!(
        "sort-paragraphs",
        3,
        3,
        f_sort_paragraphs,
        "Sort paragraphs in region alphabetically."
    ),
    S!(
        "sort-pages",
        3,
        3,
        f_sort_pages,
        "Sort pages in region alphabetically."
    ),
    S!(
        "delete-duplicate-lines",
        2,
        7,
        f_delete_duplicate_lines,
        "Delete all but one copy of identical lines in the region."
    ),
    S!(
        "reverse-region",
        2,
        2,
        f_reverse_region,
        "Reverse line order in region."
    ),
    S!(
        "how-many",
        1,
        4,
        f_how_many,
        "Count regexp matches in region."
    ),
    S!(
        "count-matches",
        1,
        4,
        f_how_many,
        "Count regexp matches in region."
    ),
    S!(
        "flush-lines",
        1,
        4,
        f_flush_lines,
        "Delete lines matching REGEXP."
    ),
    S!(
        "keep-lines",
        1,
        4,
        f_keep_lines,
        "Delete lines not matching REGEXP."
    ),
    S!(
        "replace-string",
        2,
        5,
        f_replace_string,
        "Replace all FROM with TO."
    ),
    S!(
        "replace-regexp",
        2,
        7,
        f_replace_regexp,
        "Replace all REGEXP matches with TO."
    ),
    S!(
        "replace-regexp-in-string",
        3,
        7,
        f_replace_regexp_in_string,
        "Replace REGEXP matches in STRING."
    ),
    S!(
        "transpose-regions",
        4,
        5,
        f_transpose_regions,
        "Transpose two regions."
    ),
    S!(
        "subst-char-in-region",
        4,
        5,
        f_subst_char_in_region,
        "Replace FROMCHAR with TOCHAR in region."
    ),
    S!(
        "translate-region",
        3,
        3,
        f_translate_region,
        "Translate chars via TABLE."
    ),
    S!(
        "make-translation-table-from-alist",
        1,
        1,
        f_make_translation_table_from_alist,
        "Build a char-table translation table from ALIST."
    ),
    S!(
        "buffer-swap-text",
        1,
        1,
        f_buffer_swap_text,
        "Swap text with BUFFER."
    ),
    S!(
        "buffer-last-name",
        0,
        1,
        f_buffer_last_name,
        "Name before last rename (nil)."
    ),
    S!(
        "buffer-chars-modified-tick",
        0,
        1,
        f_buffer_chars_mod_tick,
        "Modification counter that only counts text changes, not\n\
         text-property changes."
    ),
    S!(
        "buffer-modified-tick",
        0,
        1,
        f_buffer_mod_tick,
        "Modification counter."
    ),
    S!(
        "store-match-data",
        1,
        2,
        f_store_match_data,
        "Set match data from LIST."
    ),
    S!(
        "match-data--translate",
        1,
        1,
        f_match_data_translate,
        "Add N to all positions in the match data."
    ),
    S!(
        "field-beginning",
        0,
        3,
        f_field_beginning,
        "Start of field at POS."
    ),
    S!("field-end", 0, 3, f_field_end, "End of field at POS."),
    S!("field-string", 0, 1, f_field_string, "Field text at POS."),
    S!(
        "field-string-no-properties",
        0,
        1,
        f_field_string_no_properties,
        "Field text (no props)."
    ),
    S!("delete-field", 0, 1, f_delete_field, "Delete field at POS."),
    S!(
        "constrain-to-field",
        2,
        5,
        f_constrain_to_field,
        "Clamp NEW-POS to field of OLD-POS."
    ),
    S!(
        "get-char-property-and-overlay",
        2,
        3,
        f_get_char_prop_and_overlay,
        "Prop + overlay at POS."
    ),
    S!(
        "next-char-property-change",
        1,
        2,
        f_next_prop_change_fwd,
        "Next position where any prop changes."
    ),
    S!(
        "previous-char-property-change",
        1,
        2,
        f_prev_prop_change,
        "Previous prop boundary."
    ),
    S!(
        "marker-last-position",
        1,
        1,
        f_marker_last_position,
        "Last known position of MARKER."
    ),
    S!(
        "compute-motion",
        7,
        7,
        f_compute_motion,
        "Compute motion to TO (simplified)."
    ),
    S!(
        "vertical-motion",
        1,
        3,
        f_vertical_motion,
        "Move point LINES lines."
    ),
    S!(
        "line-pixel-height",
        0,
        0,
        f_one,
        "Pixel height of a line (1 col)."
    ),
    S!(
        "window-line-height",
        0,
        2,
        f_win_line_height,
        "Height of line in window (1)."
    ),
    S!(
        "backward-prefix-chars",
        0,
        0,
        f_backward_prefix_chars,
        "Skip back over prefix chars."
    ),
    S!(
        "move-point-visually",
        1,
        1,
        f_move_point_visually,
        "Move point visually (simplified)."
    ),
    S!(
        "base64-encode-region",
        2,
        3,
        f_b64_encode_region,
        "Base64-encode region."
    ),
    S!(
        "base64-decode-region",
        2,
        4,
        f_b64_decode_region,
        "Base64-decode region."
    ),
    S!(
        "base64url-encode-region",
        2,
        3,
        f_b64url_encode_region,
        "Base64url encode region."
    ),
    // `secure-hash-region' is not a GNU primitive (secure-hash takes
    // a buffer/string object instead) — intentionally unregistered.
    S!(
        "undo-boundary",
        0,
        0,
        f_undo_boundary,
        "Push undo boundary."
    ),
    S!(
        "base64url-encode-string",
        1,
        2,
        f_b64url_encode_string,
        "URL-safe base64 of STRING; NO-PADDING strips `='."
    ),
    S!(
        "get-byte",
        0,
        2,
        f_get_byte,
        "Byte value of char at POSITION in STRING."
    ),
    S!(
        "find-file-name-handler",
        2,
        2,
        f_find_file_name_handler,
        "Handler for FILE per `file-name-handler-alist', or nil."
    ),
    S!(
        "replace-region-contents",
        3,
        6,
        f_replace_region_contents,
        "Replace region with SOURCE (a buffer, string, or fn)."
    ),
    S!(
        "combine-after-change-execute",
        0,
        0,
        f_combine_after_change_execute,
        "Run deferred after-change functions (no-op: ours are eager)."
    ),
    S!(
        "translate-region-internal",
        3,
        3,
        f_translate_region_internal,
        "Translate chars in START..END through TABLE (a string)."
    ),
    S!(
        "buffer-line-statistics",
        0,
        1,
        f_buffer_line_statistics,
        "Return (LINES LONGEST-LINE MEAN-LINE-LENGTH) for the buffer."
    ),
];

fn f_b64url_encode_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let s = want_string(i, &a[0])?;
    let no_pad = a.get(1).map(|v| v.truthy()).unwrap_or(false);
    let eng = if no_pad {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
    } else {
        base64::engine::general_purpose::URL_SAFE
    };
    Ok(Value::string(eng.encode(s.as_bytes())))
}

fn f_get_byte(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (get-byte &optional POSITION STRING) — GNU: STRING is stringp-checked
    // first; then POSITION is wholenump-checked for strings (0-based BYTE
    // index into the UTF-8/unibyte storage) and integer-or-marker-checked
    // for the buffer (1-based pos).
    match a.get(1) {
        Some(Value::Str(s)) => {
            let s = s.borrow().clone();
            let pos = match &a[0] {
                // GNU: nil POSITION → 0 for the string case.
                Value::Nil => 0,
                Value::Int(n) if *n >= 0 => *n,
                other => return Err(i.wrong_type_mut("wholenump", other)),
            };
            // GNU: pos is a CHAR index (0-based); the fetched char must be
            // ASCII or an eight-bit char, whose raw byte value is returned.
            // Unibyte strings store byte values as chars/proxies so a char
            // index is the byte index here too.
            let c = s.chars().nth(pos as usize).map(|c| c as u32);
            let Some(c) = c else {
                return Err(i.signal_data(
                    crate::lisp::sym::ARGS_OUT_OF_RANGE,
                    vec![a[1].clone(), a[0].clone()],
                ));
            };
            if c < 128 {
                Ok(Value::Int(c as i128))
            } else if let Some(b) = char::from_u32(c).and_then(crate::lisp::value::eight_bit_byte) {
                // GNU returns the byte value of eight-bit chars.
                Ok(Value::Int(b as i128))
            } else {
                Err(i.signal_data(
                    crate::lisp::sym::ERROR,
                    vec![Value::string(format!(
                        "Not an ASCII nor an 8-bit character: {}",
                        crate::lisp::value::lisp_char_code(char::from_u32(c).unwrap_or('\u{FFFD}'))
                    ))],
                ))
            }
        }
        Some(other) if !other.is_nil() => Err(i.wrong_type_mut("stringp", other)),
        _ => {
            // Buffer text at 1-based POSITION — GNU FETCH_CHAR; ASCII and
            // 8-bit chars only, else "Not an ASCII nor an 8-bit character".
            // Internal point/marker storage is 0-based → +1 to get the
            // 1-based char position GNU means.
            let (pos, explicit) = match a.get(0) {
                None | Some(Value::Nil) => {
                    let b = cur(i);
                    (b.borrow().point() as i128 + 1, false)
                }
                Some(Value::Int(n)) => (*n, true),
                Some(Value::Marker(m)) => match m.borrow().buffer {
                    // GNU Fget_byte signals "Marker does not point
                    // anywhere" for a detached marker.
                    Some(_) => (m.borrow().position as i128 + 1, true),
                    None => return Err(i.error("Marker does not point anywhere")),
                },
                Some(other) => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
            };
            let b = cur(i);
            let bb = b.borrow();
            // GNU: an explicit POSITION must satisfy BEGV <= pos < ZV and
            // signals (args-out-of-range (pos BEGV ZV)); a defaulted nil
            // POSITION uses PT, where PT==ZV at eob reads the trailing
            // byte area → 0.
            if explicit && (pos < 1 || pos as usize > bb.text.len()) {
                return Err(i.signal_data(
                    crate::lisp::sym::ARGS_OUT_OF_RANGE,
                    vec![
                        a.get(0).cloned().unwrap_or(Value::Nil),
                        Value::Int(1),
                        Value::Int(bb.text.len() as i128 + 1),
                    ],
                ));
            }
            if pos < 1 || pos as usize > bb.text.len() + 1 {
                return Err(i.signal_data(
                    crate::lisp::sym::ARGS_OUT_OF_RANGE,
                    vec![a.get(0).cloned().unwrap_or(Value::Nil)],
                ));
            }
            let c = if pos as usize == bb.text.len() + 1 {
                0
            } else {
                bb.text.char_at(pos as usize - 1) as u32
            };
            if c < 128 {
                Ok(Value::Int(c as i128))
            } else if let Some(b) = char::from_u32(c).and_then(crate::lisp::value::eight_bit_byte) {
                Ok(Value::Int(b as i128))
            } else {
                Err(i.signal_data(
                    crate::lisp::sym::ERROR,
                    vec![Value::string(format!(
                        "Not an ASCII nor an 8-bit character: {}",
                        crate::lisp::value::lisp_char_code(char::from_u32(c).unwrap_or('\u{FFFD}'))
                    ))],
                ))
            }
        }
    }
}

/// Shared helper: GNU `find-file-name-handler' scan of
/// `file-name-handler-alist' for FILE serving OPERATION.
pub(crate) fn find_file_name_handler(
    i: &mut Interp,
    file: &str,
    operation: Value,
) -> Option<Value> {
    let fid = i.intern("file-name-handler-alist");
    let alist = i.symbol_value(fid);
    let ops_id = i.intern("operations");
    let mut cur = alist;
    // GNU fileio.c Ffind_file_name_handler:
    // - every matching (REGEXP . HANDLER) is scanned; the match with
    //   the largest start position wins (`match_pos > pos', ties keep
    //   the earlier entry).
    // - a handler symbol carrying an `operations' property serves only
    //   the listed operations.
    // - `inhibit-file-name-handlers' applies ONLY when OPERATION
    //   equals the `inhibit-file-name-operation' variable's value —
    //   `tramp-run-real-handler' binds it to the inner operation, so
    //   handlers suppress themselves for that call — while ordinary
    //   operations ignore it (GNU-verified on 31.1).
    let ifno = i.intern("inhibit-file-name-operation");
    let inhibit_target = i.symbol_value(ifno);
    let inhibited: Option<Value> = if crate::lisp::builtins::eq_values(&operation, &inhibit_target)
    {
        let iid = i.intern("inhibit-file-name-handlers");
        Some(i.symbol_value(iid))
    } else {
        None
    };
    let syn = crate::editor::re_syntax(i);
    let cat = crate::editor::re_category(i);
    let chars: Vec<char> = file.chars().collect();
    let mut best: Option<Value> = None;
    let mut best_pos: isize = -1;
    loop {
        match cur {
            Value::Cons(c) => {
                let (cell, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                cur = next;
                if let Value::Cons(pair) = &cell {
                    let (re, handler) = {
                        let b = pair.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if let Value::Str(pat) = &re {
                        let pat = pat.borrow().clone();
                        let match_start = crate::lisp::regexp::compile_case(&pat, false)
                            .ok()
                            .and_then(|re| {
                                crate::lisp::regexp::search_full(
                                    &re,
                                    &chars,
                                    0,
                                    &syn,
                                    &cat,
                                    crate::lisp::regexp::NO_POINT,
                                    false,
                                )
                            })
                            .and_then(|regs| regs.first().copied().flatten());
                        let Some(ms) = match_start else { continue };
                        // Non-nil `operations' property: only the listed
                        // operations are served.
                        if let Value::Sym(hs) = &handler {
                            let ops = i.get_prop(*hs, ops_id);
                            if !ops.is_nil() {
                                let mut listed = false;
                                let mut oc = ops;
                                while let Value::Cons(cc) = oc {
                                    let (h, n) = {
                                        let b = cc.borrow();
                                        (b.car.clone(), b.cdr.clone())
                                    };
                                    if crate::lisp::builtins::eq_values(&h, &operation) {
                                        listed = true;
                                        break;
                                    }
                                    oc = n;
                                }
                                if !listed {
                                    continue;
                                }
                            }
                        }
                        if let Some(inh) = &inhibited {
                            let mut ic = inh.clone();
                            let mut skip = false;
                            while let Value::Cons(cc) = ic {
                                let (h, n) = {
                                    let b = cc.borrow();
                                    (b.car.clone(), b.cdr.clone())
                                };
                                if crate::lisp::builtins::eq_values(&h, &handler) {
                                    skip = true;
                                    break;
                                }
                                ic = n;
                            }
                            if skip {
                                continue;
                            }
                        }
                        if ms as isize > best_pos {
                            best_pos = ms as isize;
                            best = Some(handler);
                        }
                    }
                }
            }
            _ => return best,
        }
    }
}

fn f_find_file_name_handler(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (find-file-name-handler FILE OPERATION) — scan
    let file = want_string(i, &a[0])?;
    // GNU fileio.c Ffind_file_name_handler: a matching handler symbol
    // carrying an `operations' property only serves the operations
    // listed there (e.g. `jka-compr-handler' declines
    // `file-name-sans-versions' but serves `insert-file-contents').
    // (`inhibit-file-name-handlers' is consulted where C code *calls*
    // handlers, not by this subr — GNU ignores it here.)
    Ok(find_file_name_handler(i, &file, a[1].clone()).unwrap_or(Value::Nil))
}

fn f_replace_region_contents(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (replace-region-contents BEG END SOURCE &rest) — SOURCE is a
    // buffer, a string, or a 0-arg function returning one.
    let (s, e, _) = region_text(i, &a)?;
    let src = a[2].clone();
    let text = match &src {
        Value::Str(st) => st.borrow().clone(),
        Value::Buffer(_) => {
            let b = buf_of(i, &src)?;
            b.borrow().text.text()
        }
        Value::Sym(_) | Value::Lambda(_) | Value::Subr(_) | Value::Cons(_) => {
            let r = i.apply(&src, vec![])?;
            match &r {
                Value::Str(st) => st.borrow().clone(),
                Value::Buffer(_) => buf_of(i, &r)?.borrow().text.text(),
                other => return Err(i.wrong_type_mut("stringp", other)),
            }
        }
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    check_writable(i)?;
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &text)?;
    Ok(Value::t())
}

fn f_combine_after_change_execute(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// (translate-region-internal START END TABLE) — GNU
/// `translate-region-internal': TABLE is a string or a char-table of
/// `translation-table' purpose; returns the number of chars changed.
fn f_translate_region_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (s, e) = beg_end(i, &a, 0, 1)?;
    check_writable(i)?;
    Ok(Value::Int(translate_region_core(i, s, e, &a[2])?))
}

/// (buffer-line-statistics &optional BUFFER) → (LINES LONGEST MEAN)
/// over BUFFER's accessible portion; GNU decodes the arg with
/// decode_buffer (nil→current, buffer→itself, live name→buffer,
/// other strings→"No buffer named", others→stringp).
fn f_buffer_line_statistics(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = match &crate::lisp::builtins::arg(&a, 0) {
        Value::Nil => cur(i),
        Value::Buffer(b) => b.clone(),
        Value::Str(s) => match i.buffers.by_name(&s.borrow()) {
            Some(id) => match i.buffers.get(id) {
                Some(b) => b,
                None => return Err(i.error(&format!("No buffer named {}", s.borrow()))),
            },
            None => return Err(i.error(&format!("No buffer named {}", s.borrow()))),
        },
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let bb = b.borrow();
    let (lo, hi) = (bb.begv, bb.zv.min(bb.text.len()));
    let mut lines = 0usize;
    let mut longest = 0usize;
    let mut total = 0usize;
    let mut cur_len = 0usize;
    for pos in lo..hi {
        if bb.text.char_at(pos) == '\n' {
            lines += 1;
            total += cur_len;
            longest = longest.max(cur_len);
            cur_len = 0;
        } else {
            cur_len += 1;
        }
    }
    if cur_len > 0 {
        lines += 1;
        total += cur_len;
        longest = longest.max(cur_len);
    }
    let mean = if lines > 0 {
        total as f64 / lines as f64
    } else {
        0.0
    };
    Ok(Value::list(vec![
        Value::Int(lines as i128),
        Value::Int(longest as i128),
        Value::float(mean),
    ]))
}

/// `match-data--translate' (search.c): add N to every recorded
/// position, clamping at 0.  GNU stores "positions as opaque pair
/// values" — ours are plain usize regs inside `MatchData'.
fn f_match_data_translate(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: CHECK_FIXNUM → `fixnump'.
    let n = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    if let Some(md) = &mut i.match_data {
        for r in &mut md.regs {
            if let Some(p) = r {
                let np = *p as i128 + n;
                *p = np.max(0) as usize;
            }
        }
    }
    Ok(Value::Nil)
}

fn f_one(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Int(1))
}

fn beg_end(
    i: &mut Interp,
    a: &[Value],
    b_idx: usize,
    e_idx: usize,
) -> Result<(usize, usize), Flow> {
    // GNU `&optional START END' decode: absent -> BEGV/ZV, but a
    // PRESENT non-number/non-marker (including nil) signals
    // integer-or-marker-p via validate_region.
    let s = match a.get(b_idx) {
        None => cur(i).borrow().begv,
        Some(v) => pos_idx(
            cur(i).borrow().text_len(),
            crate::lisp::builtins::want_int_or_mark(i, v)?,
        ),
    };
    let e = match a.get(e_idx) {
        None => cur(i).borrow().zv.max(cur(i).borrow().text_len()),
        Some(v) => pos_idx(
            cur(i).borrow().text_len(),
            crate::lisp::builtins::want_int_or_mark(i, v)?,
        ),
    };
    Ok((s.min(e), s.max(e)))
}

fn f_current_indentation(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let bb = b.borrow();
    let pos = bb.point();
    let ls = line_start(&bb.text, pos);
    let mut col = 0i128;
    let mut p = ls;
    let end = bb.text_len();
    loop {
        if p >= end {
            break;
        }
        match bb.text.char_at(p) {
            ' ' => col += 1,
            '\t' => col += 8 - (col % 8),
            _ => break,
        }
        p += 1;
    }
    Ok(Value::Int(col))
}

// ---------- sort engine (mirrors GNU sort.el `sort-subr`) ----------

/// Result of a startkey callback: a returned key, a nil return (use the
/// buffer substring path), or a skipped record (GNU's `(throw 'key nil)`).
enum KeyOut {
    Val(Value),
    Nil,
    Skip,
}

#[derive(Clone)]
struct SortRec {
    key: Value,
    /// 0-based record bounds (key positions stay 1-based inside the key).
    start: usize,
    end: usize,
}

type MoveFn<'a> = &'a mut dyn FnMut(&mut Interp) -> EvalResult;
type KeyFn<'a> = &'a mut dyn FnMut(&mut Interp) -> Result<KeyOut, Flow>;

fn pt(i: &Interp) -> usize {
    cur(i).borrow().point()
}
fn zv(i: &Interp) -> usize {
    cur(i).borrow().text_len()
}
fn goto(i: &mut Interp, p: usize) {
    cur(i).borrow_mut().set_point(p);
}

/// `(forward-line 1)`: next line start, or zv.
fn line_next(i: &mut Interp) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let zv = bb.text_len();
    let mut p = bb.point().min(zv);
    while p < zv {
        let nl = bb.text.char_at(p) == '\n';
        p += 1;
        if nl {
            break;
        }
    }
    bb.set_point(p);
    Ok(Value::Nil)
}

/// `(forward-line -1)`: previous line start.
fn line_prev(i: &mut Interp) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let beg = bb.begv;
    let mut p = line_start(&bb.text, bb.point());
    if p > beg {
        p = line_start(&bb.text, p - 1);
    }
    bb.set_point(p);
    Ok(Value::Nil)
}

/// `(end-of-line)`: next `\n` or zv.
fn line_end_move(i: &mut Interp) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let zv = bb.text_len();
    let mut p = bb.point().min(zv);
    while p < zv && bb.text.char_at(p) != '\n' {
        p += 1;
    }
    bb.set_point(p);
    Ok(Value::Nil)
}

fn eolp(i: &Interp) -> bool {
    let b = cur(i);
    let bb = b.borrow();
    let p = bb.point();
    let le = line_end(&bb.text, p);
    p >= le
}
fn bolp(i: &Interp) -> bool {
    let b = cur(i);
    let bb = b.borrow();
    bb.point() <= line_start(&bb.text, bb.point())
}

/// skip-chars-forward/backward while `f` holds.
fn skip_while(i: &mut Interp, fwd: bool, f: impl Fn(char) -> bool) {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let beg = bb.begv;
    let zv = bb.text_len();
    let mut p = bb.point().clamp(beg, zv);
    if fwd {
        while p < zv && f(bb.text.char_at(p)) {
            p += 1;
        }
    } else {
        while p > beg && f(bb.text.char_at(p - 1)) {
            p -= 1;
        }
    }
    bb.set_point(p);
}

fn is_blank(c: char) -> bool {
    matches!(c, ' ' | '\t')
}
fn not_blank_nl(c: char) -> bool {
    !matches!(c, ' ' | '\t' | '\n')
}

/// Run `f` with the current buffer narrowed to [s,e) and point at `s`,
/// restoring the restriction and point afterwards (save-restriction +
/// save-excursion).
fn with_narrow<T>(
    i: &mut Interp,
    s: usize,
    e: usize,
    f: impl FnOnce(&mut Interp) -> Result<T, Flow>,
) -> Result<T, Flow> {
    let (pb, pz, pp) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.begv, bb.zv, bb.point)
    };
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        bb.begv = s.min(e);
        bb.zv = s.max(e).min(bb.text.len());
        let nbeg = bb.begv;
        bb.set_point(nbeg);
    }
    let r = f(i);
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        bb.begv = pb.min(bb.text.len());
        bb.zv = pz.min(bb.text.len()).max(bb.begv);
        bb.set_point(pp);
    }
    r
}

fn is_key_tag(i: &mut Interp, tag: &Value) -> bool {
    matches!(tag, Value::Sym(s) if *s == i.intern("key"))
}

/// The key computation of `sort-build-lists`: call startkeyfun, else take
/// the substring between point and where endkeyfun (or endrecfun, setting
/// `done`) leaves point.
fn sort_key(
    i: &mut Interp,
    startkey: &mut Option<KeyFn<'_>>,
    endkey: &mut Option<MoveFn<'_>>,
    endrec: &mut Option<MoveFn<'_>>,
    done: &mut bool,
) -> Result<Value, Flow> {
    if let Some(f) = startkey.as_deref_mut() {
        match f(i)? {
            KeyOut::Val(v) => return Ok(v),
            KeyOut::Skip => return Ok(Value::Nil),
            KeyOut::Nil => {}
        }
    }
    let kstart = pt(i);
    if let Some(f) = endkey.as_deref_mut() {
        f(i)?;
    } else if let Some(f) = endrec.as_deref_mut() {
        f(i)?;
        *done = true;
    } else {
        // GNU: (funcall nil) -> void-function nil.
        let sym = i.intern("void-function");
        return Err(i.signal_data(sym, vec![Value::Nil]));
    }
    let kend = pt(i);
    Ok(Value::cons(
        Value::Int(kstart as i128 + 1),
        Value::Int(kend as i128 + 1),
    ))
}

/// Compare two strings like GNU's string</compare-buffer-substrings:
/// lexicographic by character, folding case when `fold`.
fn cmp_str_fold(a: &str, b: &str, fold: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ac = a.chars();
    let mut bc = b.chars();
    loop {
        match (ac.next(), bc.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let (x, y) = if fold {
                    (
                        x.to_lowercase().next().unwrap_or(x),
                        y.to_lowercase().next().unwrap_or(y),
                    )
                } else {
                    (x, y)
                };
                match x.cmp(&y) {
                    Ordering::Equal => {}
                    o => return o,
                }
            }
        }
    }
}

/// Port of GNU sort.el `sort-subr`: divide the accessible text into sort
/// records via the movement callbacks, stable-sort them by key, and
/// rebuild the region keeping the inter-record gaps in place.
fn sort_engine(
    i: &mut Interp,
    reverse: bool,
    mut nextrec: Option<MoveFn<'_>>,
    mut endrec: Option<MoveFn<'_>>,
    mut startkey: Option<KeyFn<'_>>,
    mut endkey: Option<MoveFn<'_>>,
    predicate: Option<&Value>,
) -> EvalResult {
    check_writable(i)?;
    let saved_point = pt(i);
    let r = sort_engine_inner(
        i,
        reverse,
        &mut nextrec,
        &mut endrec,
        &mut startkey,
        &mut endkey,
        predicate,
    );
    goto(i, saved_point);
    r
}

fn sort_engine_inner(
    i: &mut Interp,
    reverse: bool,
    nextrec: &mut Option<MoveFn<'_>>,
    endrec: &mut Option<MoveFn<'_>>,
    startkey: &mut Option<KeyFn<'_>>,
    endkey: &mut Option<MoveFn<'_>>,
    predicate: Option<&Value>,
) -> EvalResult {
    // ---- sort-build-lists (records collected in buffer order) ----
    // GNU wraps key computation in (catch 'key ...); pushing the tag lets
    // Lisp-level `throw' reach us instead of signaling `no-catch'.
    let ksym = i.intern("key");
    i.catch_tags.push(Value::Sym(ksym));
    let mut recs: Vec<SortRec> = Vec::new();
    let mut build = |i: &mut Interp| -> Result<(), Flow> {
        loop {
            let (p, zv) = (pt(i), zv(i));
            if p >= zv {
                break;
            }
            let start_rec = p;
            let mut done = false;
            // GNU wraps the whole key computation in (catch 'key ...).
            let key = match sort_key(i, startkey, endkey, endrec, &mut done) {
                Err(Flow::Throw(tag, v)) if is_key_tag(i, &tag) => v,
                r => r?,
            };
            // GNU: (cond ((prog1 done (setq done nil))) (endrecfun ...)
            //      (nextrecfun ... done t))
            if done {
                done = false;
            } else if let Some(f) = endrec.as_deref_mut() {
                f(i)?;
            } else if let Some(f) = nextrec.as_deref_mut() {
                f(i)?;
                done = true;
            }
            if key.truthy() {
                recs.push(SortRec {
                    key,
                    start: start_rec,
                    end: pt(i),
                });
            }
            if !done {
                if let Some(f) = nextrec.as_deref_mut() {
                    f(i)?;
                }
            }
            if pt(i) <= start_rec {
                break; // malformed movement fns would loop forever (as in GNU)
            }
        }
        Ok(())
    };
    let build_r = build(i);
    i.catch_tags.pop();
    build_r?;
    if recs.is_empty() {
        return Ok(Value::Nil);
    }
    // ---- sort ----
    enum Kind {
        Num,
        Pos,
        Str,
    }
    let kind = match &recs[0].key {
        Value::Int(_) | Value::Float(_) => Kind::Num,
        Value::Cons(_) => Kind::Pos,
        _ => Kind::Str,
    };
    let fold = i
        .symbol_value(i.intern_soft("sort-fold-case").unwrap_or(0))
        .truthy();
    let mut sorted = recs.clone();
    let mut sort_err: Option<Flow> = None;
    let mut cmp = |a: &SortRec, b: &SortRec| -> Result<std::cmp::Ordering, Flow> {
        use std::cmp::Ordering;
        if let Some(p) = predicate {
            let ab = i.apply(p, vec![a.key.clone(), b.key.clone()])?.truthy();
            if ab {
                return Ok(Ordering::Less);
            }
            let ba = i.apply(p, vec![b.key.clone(), a.key.clone()])?.truthy();
            return Ok(if ba {
                Ordering::Greater
            } else {
                Ordering::Equal
            });
        }
        match kind {
            Kind::Num => {
                let x = a.key.int().map(|n| n as f64).or_else(|| match &a.key {
                    Value::Float(f) => Some(**f),
                    _ => None,
                });
                let y = b.key.int().map(|n| n as f64).or_else(|| match &b.key {
                    Value::Float(f) => Some(**f),
                    _ => None,
                });
                Ok(match (x, y) {
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
                    _ => Ordering::Equal,
                })
            }
            Kind::Pos => {
                let buf = cur(i);
                let bb = buf.borrow();
                let (s1, e1) = cons_bounds(&a.key);
                let (s2, e2) = cons_bounds(&b.key);
                let len = bb.text.len();
                let t1 = bb.text.substring(s1.min(len), e1.min(len));
                let t2 = bb.text.substring(s2.min(len), e2.min(len));
                Ok(cmp_str_fold(&t1, &t2, fold))
            }
            Kind::Str => {
                if let (Value::Str(x), Value::Str(y)) = (&a.key, &b.key) {
                    Ok(cmp_str_fold(&x.borrow(), &y.borrow(), fold))
                } else {
                    Ok(std::cmp::Ordering::Equal)
                }
            }
        }
    };
    sorted.sort_by(|a, b| match cmp(a, b) {
        Ok(o) => o,
        Err(e) => {
            if sort_err.is_none() {
                sort_err = Some(e);
            }
            std::cmp::Ordering::Equal
        }
    });
    if let Some(e) = sort_err {
        return Err(e);
    }
    if reverse {
        sorted.reverse();
    }
    // ---- sort-reorder-buffer ----
    let b = cur(i);
    let (min, max, out) = {
        let bb = b.borrow();
        let (min, max) = (bb.begv, bb.text_len());
        let mut out = String::new();
        let mut pos = min;
        for (s, o) in sorted.iter().zip(recs.iter()) {
            out.push_str(&bb.text.substring(pos, o.start));
            out.push_str(&bb.text.substring(s.start, s.end));
            pos = o.end;
        }
        out.push_str(&bb.text.substring(pos, max));
        (min, max, out)
    };
    // GNU leaves the last char in place so markers at max survive.
    crate::buffer::primitives::chg_delete(i, min, max - 1)?;
    crate::buffer::primitives::chg_insert(i, min, &out)?;
    crate::buffer::primitives::chg_delete(i, max, max + 1)?;
    Ok(Value::Nil)
}

/// 0-based substring bounds from a cons key (1-based positions).
fn cons_bounds(v: &Value) -> (usize, usize) {
    if let Value::Cons(c) = v {
        let c = c.borrow();
        let s = c.car.int().unwrap_or(1);
        let e = c.cdr.int().unwrap_or(s);
        (
            (s.max(1) - 1) as usize,
            (e.max(1) - 1).max(s.max(1) - 1) as usize,
        )
    } else {
        (0, 0)
    }
}

fn f_sort_subr(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let reverse = a[0].truthy();
    let nv = a[1].clone();
    let ev = a[2].clone();
    let skv = a.get(3).cloned().filter(|v| v.truthy());
    let ekv = a.get(4).cloned().filter(|v| v.truthy());
    let pred = a.get(5).cloned().filter(|v| v.truthy());
    let mut nr = |i: &mut Interp| i.apply(&nv, Vec::new());
    let mut er = |i: &mut Interp| i.apply(&ev, Vec::new());
    let mut sk = |i: &mut Interp| -> Result<KeyOut, Flow> {
        match i.apply(skv.as_ref().unwrap(), Vec::new()) {
            Ok(v) if v.truthy() => Ok(KeyOut::Val(v)),
            Ok(_) => Ok(KeyOut::Nil),
            Err(e) => Err(e),
        }
    };
    let mut ek = |i: &mut Interp| i.apply(ekv.as_ref().unwrap(), Vec::new());
    sort_engine(
        i,
        reverse,
        if nv.truthy() { Some(&mut nr) } else { None },
        if ev.truthy() { Some(&mut er) } else { None },
        skv.as_ref().map(|_| &mut sk as KeyFn<'_>),
        ekv.as_ref().map(|_| &mut ek as MoveFn<'_>),
        pred.as_ref(),
    )
}

fn f_sort_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let reverse = a[0].truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    with_narrow(i, s, e, |i| {
        let mut nr = |i: &mut Interp| line_next(i);
        let mut er = |i: &mut Interp| line_end_move(i);
        sort_engine(i, reverse, Some(&mut nr), Some(&mut er), None, None, None)
    })
}

fn f_sort_paragraphs(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let reverse = a[0].truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    let sep = match i.symbol_value(i.intern_soft("paragraph-separate").unwrap_or(0)) {
        Value::Str(s) => s.borrow().clone(),
        _ => "[ \t\x0c]*$".to_string(),
    };
    let fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&sep, fold)
        .map_err(|er| err_sym(i, "invalid-regexp", vec![Value::string(er.0)]))?;
    let fp = Value::Sym(i.intern("forward-paragraph"));
    with_narrow(i, s, e, |i| {
        let mut nr = |i: &mut Interp| {
            loop {
                let (p, zv) = (pt(i), zv(i));
                if p >= zv {
                    break;
                }
                let syn = crate::editor::re_syntax(i);
                let (text, begv) = {
                    let b = cur(i);
                    let bb = b.borrow();
                    (
                        bb.text
                            .substring(bb.begv, bb.text_len())
                            .chars()
                            .collect::<Vec<char>>(),
                        bb.begv,
                    )
                };
                if crate::lisp::regexp::looking_at(
                    &re,
                    &text,
                    p - begv,
                    &syn,
                    &crate::editor::re_category(i),
                    p - begv,
                    false,
                )
                .is_none()
                {
                    break;
                }
                line_next(i)?;
            }
            Ok(Value::Nil)
        };
        let mut er = |i: &mut Interp| {
            i.apply(&fp, Vec::new())?;
            // GNU adds a newline so trailing paragraphs aren't merged.
            if pt(i) >= zv(i) && !bolp(i) {
                crate::buffer::primitives::chg_insert_pt(i, "\n", false)?;
            }
            Ok(Value::Nil)
        };
        sort_engine(i, reverse, Some(&mut nr), Some(&mut er), None, None, None)
    })
}

fn f_sort_pages(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let reverse = a[0].truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    let fp = Value::Sym(i.intern("forward-page"));
    with_narrow(i, s, e, |i| {
        let mut nr = |i: &mut Interp| {
            skip_while(i, true, |c| c == '\n');
            Ok(Value::Nil)
        };
        let mut er = |i: &mut Interp| i.apply(&fp, Vec::new());
        sort_engine(i, reverse, Some(&mut nr), Some(&mut er), None, None, None)
    })
}

/// GNU sort-skip-fields: position at field N of the current line
/// (N negative counts from the right). Errors when the line lacks it.
fn sort_skip_fields(i: &mut Interp, n: i128) -> EvalResult {
    let too_few = |i: &mut Interp| -> Flow {
        let line = {
            let b = cur(i);
            let bb = b.borrow();
            let p = bb.point();
            bb.text
                .substring(line_start(&bb.text, p), line_end(&bb.text, p))
        };
        err_sym(
            i,
            "error",
            vec![Value::string(format!("Line has too few fields: {}", line))],
        )
    };
    if n > 0 {
        for _ in 1..n {
            skip_while(i, true, is_blank);
            skip_while(i, true, not_blank_nl);
        }
        skip_while(i, true, is_blank);
        if eolp(i) {
            return Err(too_few(i));
        }
    } else {
        line_end_move(i)?;
        for _ in 1..(-n) {
            skip_while(i, false, is_blank);
            skip_while(i, false, not_blank_nl);
        }
        skip_while(i, false, is_blank);
        if bolp(i) {
            return Err(too_few(i));
        }
        skip_while(i, false, not_blank_nl);
    }
    Ok(Value::Nil)
}

fn sort_fields_1(i: &mut Interp, field: i128, s: usize, e: usize, numeric: bool) -> EvalResult {
    // GNU normalizes its own `field' parameter to 1, but the startkeyfun
    // lambda is dynamically bound to the caller's FIELD -- so a 0 arg ends
    // up running sort-skip-fields(0), which lands at the last field.
    // Passing the raw value reproduces that.
    let base_default = i
        .symbol_value(i.intern_soft("sort-numeric-base").unwrap_or(0))
        .int()
        .unwrap_or(10);
    with_narrow(i, s, e, |i| {
        let mut nr = |i: &mut Interp| line_next(i);
        let mut er = |i: &mut Interp| line_end_move(i);
        let mut sk = |i: &mut Interp| -> Result<KeyOut, Flow> {
            if !numeric {
                sort_skip_fields(i, field)?;
                return Ok(KeyOut::Nil);
            }
            // GNU: blank lines get key 0 without touching field checks.
            {
                let b = cur(i);
                let bb = b.borrow();
                let p = bb.point();
                let le = line_end(&bb.text, p);
                let mut k = p;
                while k < le && is_blank(bb.text.char_at(k)) {
                    k += 1;
                }
                if k == le {
                    return Ok(KeyOut::Val(Value::Int(0)));
                }
            }
            sort_skip_fields(i, field)?;
            // Base detection: "0x"<hex> or "0"<octal> (case-folded).
            let b = cur(i);
            let mut bb = b.borrow_mut();
            let le = line_end(&bb.text, bb.point());
            let p = bb.point();
            let c0 = if p < le {
                Some(bb.text.char_at(p))
            } else {
                None
            };
            let c1 = if p + 1 < le {
                Some(bb.text.char_at(p + 1))
            } else {
                None
            };
            let c2 = if p + 2 < le {
                Some(bb.text.char_at(p + 2))
            } else {
                None
            };
            let mut base = base_default;
            if c0 == Some('0')
                && matches!(c1, Some('x') | Some('X'))
                && c2.is_some_and(|c| c.is_ascii_hexdigit())
            {
                base = 16;
                bb.set_point(p + 2);
            } else if c0 == Some('0') && c1.is_some_and(|c| ('0'..='7').contains(&c)) {
                base = 8;
                bb.set_point(p + 1);
            }
            let p = bb.point();
            let le = line_end(&bb.text, p);
            let mut e2 = p;
            while e2 < le && not_blank_nl(bb.text.char_at(e2)) {
                e2 += 1;
            }
            let txt = bb.text.substring(p, e2);
            Ok(KeyOut::Val(Value::Int(string_to_num_base(&txt, base))))
        };
        let mut ek = |i: &mut Interp| {
            skip_while(i, true, not_blank_nl);
            Ok(Value::Nil)
        };
        sort_engine(
            i,
            false,
            Some(&mut nr),
            Some(&mut er),
            Some(&mut sk),
            Some(&mut ek),
            None,
        )
    })
}

/// Longest numeric prefix parsed in `base` (GNU string-to-number).
fn string_to_num_base(t: &str, base: i128) -> i128 {
    let t = t.trim();
    if base != 10 && (2..=16).contains(&base) {
        let neg = t.starts_with('-');
        let digits = t.trim_start_matches(['+', '-']);
        let take: String = digits
            .chars()
            .take_while(|c| c.to_digit(base as u32).is_some())
            .collect();
        return i128::from_str_radix(&take, base as u32)
            .map(|n| if neg { -n } else { n })
            .unwrap_or(0);
    }
    // base 10 (or invalid base): parse the longest numeric prefix.
    let mut n = 0;
    let mut seen_digit = false;
    let chars: Vec<char> = t.chars().collect();
    while n < chars.len() {
        let c = chars[n];
        if c.is_ascii_digit() {
            seen_digit = true;
            n += 1;
        } else if (c == '+' || c == '-') && n == 0 {
            n += 1;
        } else {
            break;
        }
    }
    // float tail: .digits, exponent
    if n < chars.len() && chars[n] == '.' {
        let mut m = n + 1;
        while m < chars.len() && chars[m].is_ascii_digit() {
            m += 1;
        }
        if m > n + 1 || seen_digit {
            n = m;
        }
    }
    if n < chars.len() && matches!(chars[n], 'e' | 'E') {
        let mut m = n + 1;
        if m < chars.len() && matches!(chars[m], '+' | '-') {
            m += 1;
        }
        let d0 = m;
        while m < chars.len() && chars[m].is_ascii_digit() {
            m += 1;
        }
        if m > d0 {
            n = m;
        }
    }
    let s = &t[..t.char_indices().nth(n).map(|(i, _)| i).unwrap_or(t.len())];
    if !seen_digit && !s.chars().any(|c| c.is_ascii_digit()) {
        return 0;
    }
    if let Ok(v) = s.parse::<i128>() {
        return v;
    }
    s.parse::<f64>().map(|f| f as i128).unwrap_or(0)
}

fn f_sort_fields(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let field = want_int(i, &a[0])?;
    let (s, e) = beg_end(i, &a, 1, 2)?;
    sort_fields_1(i, field, s, e, false)
}

fn f_sort_numeric_fields(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let field = want_int(i, &a[0])?;
    let (s, e) = beg_end(i, &a, 1, 2)?;
    sort_fields_1(i, field, s, e, true)
}

/// Forward re-search on the current buffer; sets match data on hit.
/// `bound` is a 0-based absolute limit on the match end. When
/// `move_on_fail` (GNU noerror='move) point goes to `bound`/zv on failure.
fn re_search_fwd(
    i: &mut Interp,
    re: &crate::lisp::regexp::Regex,
    bound: Option<usize>,
    move_on_fail: bool,
) -> EvalResult {
    let (text, pos, begv) = {
        let b = cur(i);
        let bb = b.borrow();
        (
            bb.text
                .substring(bb.begv, bb.text_len())
                .chars()
                .collect::<Vec<char>>(),
            bb.point().saturating_sub(bb.begv),
            bb.begv,
        )
    };
    let bound_idx = bound.map(|b| b.saturating_sub(begv)).unwrap_or(text.len());
    let syn = crate::editor::re_syntax(i);
    match crate::lisp::regexp::search_full(
        re,
        &text,
        pos,
        &syn,
        &crate::editor::re_category(i),
        pos,
        false,
    ) {
        Some(regs) if regs[1].unwrap_or(0) <= bound_idx => {
            let e = regs[1].unwrap_or(0);
            i.match_data = Some(crate::lisp::eval::MatchData {
                regs,
                in_buffer: true,
                base: begv,
                buffer: Some(i.current_buffer),
            });
            cur(i).borrow_mut().set_point(begv + e);
            Ok(Value::t())
        }
        _ => {
            if move_on_fail {
                let to = begv + bound_idx.min(text.len());
                cur(i).borrow_mut().set_point(to);
            }
            Ok(Value::Nil)
        }
    }
}

/// 0-based absolute position of match boundary N, or None.
fn match_pos(i: &Interp, n: usize, end: bool) -> Option<usize> {
    i.match_data.as_ref().and_then(|md| {
        md.regs
            .get(2 * n + usize::from(end))
            .copied()
            .flatten()
            .map(|p| p + md.base)
    })
}

fn f_sort_regexp_fields(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let reverse = a[0].truthy();
    let record_pat = want_string(i, &a[1])?;
    // GNU: "" or "\&" -> group 0; "\<digit>" -> group N; number -> N;
    // otherwise a regexp searched inside each record.
    enum KeySpec {
        Group(usize),
        Re(String),
    }
    let key = match &a[2] {
        Value::Str(s) => {
            let s = s.borrow().clone();
            if s.is_empty() || s == "\\&" {
                KeySpec::Group(0)
            } else if s.len() == 2
                && s.starts_with('\\')
                && s.chars().nth(1).is_some_and(|c| ('1'..='9').contains(&c))
            {
                KeySpec::Group(s.chars().nth(1).unwrap() as usize - '0' as usize)
            } else {
                KeySpec::Re(s)
            }
        }
        // GNU: (numberp key-regexp) only after string normalization; a
        // non-string argument errors wrong-type-argument stringp.
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let (s, e) = beg_end(i, &a, 3, 4)?;
    let ambient_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let fold = i
        .symbol_value(i.intern_soft("sort-fold-case").unwrap_or(0))
        .truthy();
    let record_re0 = crate::lisp::regexp::compile_case(&record_pat, ambient_fold)
        .map_err(|er| err_sym(i, "invalid-regexp", vec![Value::string(er.0)]))?;
    let record_re = crate::lisp::regexp::compile_case(&record_pat, fold)
        .map_err(|er| err_sym(i, "invalid-regexp", vec![Value::string(er.0)]))?;
    let key_re = match &key {
        KeySpec::Re(pat) => Some(
            crate::lisp::regexp::compile_case(pat, fold)
                .map_err(|er| err_sym(i, "invalid-regexp", vec![Value::string(er.0)]))?,
        ),
        _ => None,
    };
    with_narrow(i, s, e, |i| {
        let nbeg = cur(i).borrow().begv;
        goto(i, nbeg);
        // GNU: (re-search-forward re nil t) -- no move on failure; then
        // record-end = (point), and (goto-char (match-beginning 0)).
        // In a fresh buffer match data reads as 0 -> goto clamps to begv.
        re_search_fwd(i, &record_re0, None, false)?;
        let rec_end = std::rc::Rc::new(std::cell::Cell::new(pt(i)));
        goto(i, match_pos(i, 0, false).unwrap_or(0));
        let rec_end2 = rec_end.clone();
        let mut nr = move |i: &mut Interp| {
            let oldpos = pt(i);
            if re_search_fwd(i, &record_re, None, true)?.truthy() {
                rec_end2.set(match_pos(i, 0, true).unwrap_or_else(|| pt(i)));
                if rec_end2.get() == oldpos {
                    // Empty match: skip a char and search again (GNU).
                    goto(i, pt(i) + 1);
                    re_search_fwd(i, &record_re, None, true)?;
                    rec_end2.set(match_pos(i, 0, true).unwrap_or_else(|| pt(i)));
                }
                goto(i, match_pos(i, 0, false).unwrap_or_else(|| pt(i)));
            }
            Ok(Value::Nil)
        };
        let rec_end3 = rec_end.clone();
        let mut er = move |i: &mut Interp| {
            goto(i, rec_end3.get());
            Ok(Value::Nil)
        };
        let rec_end4 = rec_end.clone();
        let mut sk = move |i: &mut Interp| -> Result<KeyOut, Flow> {
            match &key {
                KeySpec::Group(n) => {
                    let n = *n;
                    match (match_pos(i, n, false), match_pos(i, n, true)) {
                        (Some(b), Some(e)) => Ok(KeyOut::Val(Value::cons(
                            Value::Int(b as i128 + 1),
                            Value::Int(e as i128 + 1),
                        ))),
                        _ => Ok(KeyOut::Skip),
                    }
                }
                KeySpec::Re(_) => {
                    let bound = rec_end4.get();
                    if re_search_fwd(i, key_re.as_ref().unwrap(), Some(bound), false)?.truthy() {
                        let b = match_pos(i, 0, false).unwrap_or(0);
                        let e = match_pos(i, 0, true).unwrap_or(0);
                        Ok(KeyOut::Val(Value::cons(
                            Value::Int(b as i128 + 1),
                            Value::Int(e as i128 + 1),
                        )))
                    } else {
                        Ok(KeyOut::Skip)
                    }
                }
            }
        };
        sort_engine(
            i,
            reverse,
            Some(&mut nr),
            Some(&mut er),
            Some(&mut sk),
            None,
            None,
        )
    })
}

/// Display column of absolute position `p` (tabs via `tab-width`).
fn col_at(i: &Interp, p: usize) -> i128 {
    let b = cur(i);
    let bb = b.borrow();
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let ls = line_start(&bb.text, p);
    let mut col = 0i128;
    let mut k = ls;
    while k < p {
        match bb.text.char_at(k) {
            '\t' => col = (col / tab_width + 1) * tab_width,
            c => col += crate::buffer::primitives::char_width(c),
        }
        k += 1;
    }
    col
}

/// GNU move-to-column without FORCE: land on first char boundary with
/// col >= goal (chars never split, short lines just stop at EOL).
fn move_to_col(i: &mut Interp, goal: i128) -> EvalResult {
    // `tab-width' is buffer-local: read it before borrow_mut so
    // `symbol_value' can see the local binding.
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let p = bb.point();
    let ls = line_start(&bb.text, p);
    let le = line_end(&bb.text, p).min(bb.text_len());
    let mut col = 0i128;
    let mut k = ls;
    while k < le && col < goal {
        match bb.text.char_at(k) {
            '\t' => col = (col / tab_width + 1) * tab_width,
            c => col += crate::buffer::primitives::char_width(c),
        }
        k += 1;
    }
    bb.set_point(k);
    Ok(Value::Int(col))
}

fn f_sort_columns(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let reverse = a[0].truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    let (col_s, col_e) = (col_at(i, s), col_at(i, e));
    let (col_start, col_end) = (col_s.min(col_e), col_s.max(col_e));
    let (beg1, end1) = {
        let b = cur(i);
        let bb = b.borrow();
        let beg1 = line_start(&bb.text, s.min(e));
        // GNU: (goto-char (max beg end)) (forward-line) -> next line start.
        let mut end1 = line_end(&bb.text, s.max(e));
        if end1 < bb.text_len() {
            end1 += 1;
        }
        (beg1, end1)
    };
    // GNU rejects tabs in the sorted lines.
    {
        let b = cur(i);
        let bb = b.borrow();
        if bb.text.substring(beg1, end1).contains('\t') {
            drop(bb);
            return Err(err_sym(
                i,
                "error",
                vec![Value::string(
                    "sort-columns does not work with tabs -- use M-x untabify",
                )],
            ));
        }
    }
    with_narrow(i, beg1, end1, |i| {
        let nbeg = cur(i).borrow().begv;
        goto(i, nbeg);
        let mut nr = |i: &mut Interp| line_next(i);
        let mut er = |i: &mut Interp| line_end_move(i);
        let mut sk = move |i: &mut Interp| {
            move_to_col(i, col_start)?;
            Ok(KeyOut::Nil)
        };
        let mut ek = move |i: &mut Interp| move_to_col(i, col_end);
        sort_engine(
            i,
            reverse,
            Some(&mut nr),
            Some(&mut er),
            Some(&mut sk),
            Some(&mut ek),
            None,
        )
    })
}

fn f_reverse_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let b = cur(i);
    let bb = b.borrow_mut();
    // GNU: beg moves to the start of the next line unless already at BOL;
    // end moves to the end of the previous line unless at a non-empty EOL.
    let beg = if line_start(&bb.text, s) == s {
        s
    } else {
        let le = line_end(&bb.text, s);
        if le < bb.text_len() { le + 1 } else { le }
    };
    let end = {
        let le = line_end(&bb.text, e);
        if e == le && line_start(&bb.text, e) < e {
            e
        } else {
            // (forward-line -1) (end-of-line)
            let prev = if line_start(&bb.text, e) > bb.begv {
                line_start(&bb.text, line_start(&bb.text, e) - 1)
            } else {
                line_start(&bb.text, e)
            };
            line_end(&bb.text, prev)
        }
    };
    if end <= beg {
        drop(bb);
        return Err(err_sym(
            i,
            "user-error",
            vec![Value::string("There are no full lines in the region")],
        ));
    }
    let mut lines = Vec::new();
    let mut p = beg;
    loop {
        let le = line_end(&bb.text, p);
        lines.push(bb.text.substring(p, le));
        if le >= end {
            break;
        }
        p = le + 1;
    }
    let out = lines.iter().rev().cloned().collect::<Vec<_>>().join("\n");
    drop(bb);
    crate::buffer::primitives::chg_delete(i, beg, end)?;
    crate::buffer::primitives::chg_insert(i, beg, &out)?;
    Ok(Value::Nil)
}

fn f_delete_duplicate_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let (mut beg, mut end) = beg_end(i, &a, 0, 1)?;
    let reverse = arg(&a, 2).truthy();
    let adjacent = arg(&a, 3).truthy();
    let keep_blanks = arg(&a, 4).truthy();
    let interactive = arg(&a, 5).truthy();
    let mut seen = std::collections::HashSet::new();
    let mut prev_line = String::new();
    let mut first_line = false;
    let mut count = 0i128;
    goto(i, if reverse { end } else { beg });
    if reverse && bolp(i) {
        goto(i, pt(i).saturating_sub(1));
    }
    loop {
        let (p, zv, begv) = {
            let b = cur(i);
            let bb = b.borrow();
            (bb.point(), bb.text_len(), bb.begv)
        };
        if reverse {
            if first_line {
                break;
            }
        } else if !(p < end && p < zv) {
            break;
        }
        if reverse {
            first_line = p <= beg || p <= begv;
        }
        let (ls, le) = {
            let b = cur(i);
            let bb = b.borrow();
            (line_start(&bb.text, p), line_end(&bb.text, p))
        };
        let line = cur(i).borrow().text.substring(ls, le);
        if keep_blanks && line.is_empty() {
            line_next(i)?;
        } else {
            let dup = if adjacent {
                line == prev_line
            } else {
                seen.contains(&line)
            };
            if dup {
                // GNU: delete line incl. its newline (or to eob).
                let del_end = if le < zv { le + 1 } else { le };
                crate::buffer::primitives::chg_delete(i, ls, del_end)?;
                // Marker semantics for beg/end.
                let dlen = del_end - ls;
                if beg > del_end {
                    beg -= dlen;
                } else if beg > ls {
                    beg = ls;
                }
                if end > del_end {
                    end -= dlen;
                } else if end > ls {
                    end = ls;
                }
                let cap = ls.min(cur(i).borrow().text_len());
                goto(i, cap);
                if reverse {
                    line_prev(i)?;
                }
                count += 1;
            } else {
                if adjacent {
                    prev_line = line;
                } else {
                    seen.insert(line);
                }
                if reverse {
                    line_prev(i)?;
                } else {
                    line_next(i)?;
                }
            }
        }
    }
    if interactive {
        i.message(&format!(
            "Deleted {} {}duplicate line{}{}",
            count,
            if adjacent { "adjacent " } else { "" },
            if count == 1 { "" } else { "s" },
            if reverse { " backward" } else { "" }
        ));
    }
    Ok(Value::Int(count))
}

fn f_how_many(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pat = want_string(i, &a[0])?;
    // GNU defaults RSTART to point (not point-min) when nil.
    let (mut s, e) = beg_end(i, &a, 1, 2)?;
    if a.get(1).map(|v| v.is_nil()).unwrap_or(true) {
        s = cur(i).borrow().point.min(e);
    }
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let syn = crate::editor::re_syntax(i);
    let b = cur(i);
    let bb = b.borrow();
    let chars: Vec<char> = bb.text.substring(s, e).chars().collect();
    let mut count = 0i128;
    let mut pos = 0usize;
    while let Some(regs) = crate::lisp::regexp::search_full(
        &re,
        &chars,
        pos,
        &syn,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
        false,
    ) {
        let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
        count += 1;
        pos = if me > ms { me } else { me + 1 };
        if pos > chars.len() {
            break;
        }
    }
    // Interactive calls print the count in the echo area.
    if arg(&a, 3).truthy() {
        i.message(&format!("{} occurrences", count));
    }
    Ok(Value::Int(count))
}

fn delete_lines_matching(i: &mut Interp, a: &[Value], keep_match: bool) -> EvalResult {
    check_writable(i)?;
    let pat = want_string(i, &a[0])?;
    let (s, e) = beg_end(i, a, 1, 2)?;
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let syn = crate::editor::re_syntax(i);
    let b = cur(i);
    let bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let mut out = String::new();
    let mut removed = 0i128;
    let mut start = 0usize;
    for part in region.split_inclusive('\n') {
        let line = part.strip_suffix('\n').unwrap_or(part);
        let chars: Vec<char> = line.chars().collect();
        let hit = crate::lisp::regexp::search_full(
            &re,
            &chars,
            0,
            &syn,
            &crate::editor::re_category(i),
            crate::lisp::regexp::NO_POINT,
            false,
        )
        .is_some();
        let keep = if keep_match { hit } else { !hit };
        if keep {
            out.push_str(part);
        } else {
            removed += 1;
        }
        start += part.len();
    }
    let _ = start;
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &out)?;
    Ok(Value::Int(removed))
}

fn f_flush_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    delete_lines_matching(i, &a, false)
}

fn f_keep_lines(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    delete_lines_matching(i, &a, true)
}

fn f_replace_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let from = want_string(i, &a[0])?;
    let to = want_string(i, &a[1])?;
    if from.is_empty() {
        return Ok(Value::Nil);
    }
    let (s, e) = beg_end(i, &a, 2, 3)?;
    let b = cur(i);
    let bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let out = region.replace(&from, &to);
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &out)?;
    Ok(Value::Nil)
}

fn f_replace_regexp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let pat = want_string(i, &a[0])?;
    let to = want_string(i, &a[1])?;
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let (s, e) = beg_end(i, &a, 2, 3)?;
    let syn = crate::editor::re_syntax(i);
    let b = cur(i);
    let bb = b.borrow_mut();
    let region: Vec<char> = bb.text.substring(s, e).chars().collect();
    let mut out = String::new();
    let mut pos = 0usize;
    let mut n = 0usize;
    while let Some(regs) = crate::lisp::regexp::search_full(
        &re,
        &region,
        pos,
        &syn,
        &crate::editor::re_category(i),
        crate::lisp::regexp::NO_POINT,
        false,
    ) {
        let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
        out.extend(&region[pos..ms]);
        expand_rep(i, &to, &regs, &region, &mut out, false)?;
        pos = if me > ms { me } else { me + 1 };
        n += 1;
        if pos > region.len() || n > 1_000_000 {
            break;
        }
    }
    out.extend(&region[pos.min(region.len())..]);
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &out)?;
    Ok(Value::Nil)
}

/// Expand `\1..\9' and `\&' in REP against match registers REGS over
/// REGION, appending to OUT. LITERAL inserts REP unchanged.
/// Expand `\\&`, `\\N` and `\\\\` in a regexp replacement for
/// `replace-regexp-in-string'.  GNU keeps `\?' literal and signals
/// "Invalid use of `\\' in replacement text" for anything else; a group
/// that did not match expands to the empty string.
fn expand_rep(
    i: &mut Interp,
    rep: &str,
    regs: &[Option<usize>],
    region: &[char],
    out: &mut String,
    literal: bool,
) -> Result<(), Flow> {
    if literal {
        out.push_str(rep);
        return Ok(());
    }
    let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
    let mut tc = rep.chars().peekable();
    while let Some(c) = tc.next() {
        if c == '\\' {
            match tc.next() {
                Some(d @ '1'..='9') => {
                    let g = (d as usize) - ('0' as usize);
                    if let (Some(gs), Some(ge)) = (
                        regs.get(2 * g).copied().flatten(),
                        regs.get(2 * g + 1).copied().flatten(),
                    ) {
                        out.extend(&region[gs..ge]);
                    }
                }
                Some('&') => out.extend(&region[ms..me]),
                Some('?') => out.push_str("\\?"),
                Some('\\') => out.push('\\'),
                _ => {
                    return Err(err_sym(
                        i,
                        "error",
                        vec![Value::string("Invalid use of ‘\\’ in replacement text")],
                    ));
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(())
}

/// Propertized char-slice of `src` (a `Value::Str`) over [f, t), matching
/// GNU's `substring` (plist reversal on copy, per copy_text_properties).
fn str_slice(i: &mut Interp, src: &Value, f: usize, t: usize) -> Value {
    let s = match src {
        Value::Str(s) => s.borrow().clone(),
        _ => return src.clone(),
    };
    let chars: Vec<char> = s.chars().collect();
    let ns = std::rc::Rc::new(std::cell::RefCell::new(
        chars[f.min(chars.len())..t.min(chars.len())]
            .iter()
            .collect::<String>(),
    ));
    if let Value::Str(src_s) = src {
        if i.has_str_props(src_s) {
            let ivs: Vec<(usize, usize, Vec<Value>)> = i
                .str_props(src_s)
                .iter()
                .filter_map(|(a, b, pl)| {
                    let lo = (*a).max(f);
                    let hi = (*b).min(t);
                    (lo < hi).then(|| {
                        (
                            lo - f,
                            hi - f,
                            crate::buffer::primitives::plist_pairs_rev(pl),
                        )
                    })
                })
                .collect();
            i.set_str_props(&ns, ivs);
        }
    }
    Value::Str(ns)
}

fn f_replace_regexp_in_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pat = want_string(i, &a[0])?;
    // GNU's REP may be a string or a function called with the matched
    // text (subr.el `replace-regexp-in-string').
    let rep_fn = match &a[1] {
        Value::Str(_) => None,
        f => Some(f.clone()),
    };
    let rep = match &a[1] {
        Value::Str(s) => s.borrow().clone(),
        _ => String::new(),
    };
    let s = want_string(i, &a[2])?;
    let fixedcase = arg(&a, 3).truthy();
    let literal = !arg(&a, 4).is_nil();
    let subexp = match arg(&a, 5) {
        Value::Int(n) => n.max(0) as usize,
        _ => 0,
    };
    let chars: Vec<char> = s.chars().collect();
    let l = chars.len();
    let start = match arg(&a, 6) {
        Value::Nil => 0usize,
        v => want_int(i, &v)?.max(0) as usize,
    };
    if start > l {
        let e = i.intern("args-out-of-range");
        return Err(i.signal_data(e, vec![Value::string(s.clone()), Value::Int(start as i128)]));
    }
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    // GNU (subr.el `replace-regexp-in-string'):
    //   (while (and (< start l) (string-match regexp string start))
    //     mb = match-beg, me = match-end; if empty, me = mb+1;
    //     str = string[mb..me]; match data translated by -mb;
    //     piece = (replace-match rep fixedcase literal str subexp)
    //     — replace-match splices rep BEFORE the rest of str, so an
    //     empty match inserts rep before the skipped char, and a
    //     trailing empty match at mb == l never happens (start < l).)
    let mut pieces: Vec<String> = Vec::new();
    let mut pos = start;
    let mut n = 0usize;
    let syn = crate::editor::re_syntax(i);
    while pos < l {
        let regs = match crate::lisp::regexp::search_full(
            &re,
            &chars,
            pos,
            &syn,
            &crate::editor::re_category(i),
            crate::lisp::regexp::NO_POINT,
            false,
        ) {
            Some(r) => r,
            None => break,
        };
        let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
        let we = if me == ms { (ms + 1).min(l) } else { me };
        pieces.push(chars[pos..ms].iter().collect());
        // Match data translated to str = string[ms..we].
        let tregs: Vec<Option<usize>> = regs.iter().map(|r| r.map(|x| x - ms)).collect();
        let str_chars = &chars[ms..we];
        // GNU: a function REP is called with `match-string 0' under
        // the translated match data; the result is then expanded.
        let rep_text: String;
        let rep_str = if let Some(f) = &rep_fn {
            let saved_md = i.match_data.clone();
            i.match_data = Some(crate::lisp::eval::MatchData {
                regs: tregs.clone(),
                in_buffer: false,
                base: 0,
                buffer: None,
            });
            let mstr = str_slice(i, &a[2], ms, me);
            let rv = i.apply(f, vec![mstr]);
            i.match_data = saved_md;
            rep_text = want_string(i, &rv?)?;
            rep_text.as_str()
        } else {
            rep.as_str()
        };
        let (gs, ge) = if subexp > 0 {
            match (
                tregs.get(2 * subexp).copied().flatten(),
                tregs.get(2 * subexp + 1).copied().flatten(),
            ) {
                (Some(s), Some(e)) => (s, e),
                // GNU `replace-match': any subexp beyond the match
                // registers — existing-but-unmatched or absent
                // entirely — signals "replace-match subexpression
                // does not exist".
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
            }
        } else {
            (0, me - ms)
        };
        let mut rep_piece = String::new();
        expand_rep(i, rep_str, &tregs, str_chars, &mut rep_piece, literal)?;
        if !fixedcase {
            let old: String = str_chars[gs.min(str_chars.len())..ge.min(str_chars.len())]
                .iter()
                .collect();
            rep_piece = crate::buffer::primitives::match_case(&old, &rep_piece);
        }
        let mut piece: String = str_chars[..gs.min(str_chars.len())].iter().collect();
        piece.push_str(&rep_piece);
        piece.extend(str_chars[ge.min(str_chars.len())..].iter());
        pieces.push(piece);
        pos = we;
        n += 1;
        if n > 1_000_000 {
            break;
        }
    }
    pieces.push(chars[pos.min(l)..].iter().collect());
    Ok(Value::string(pieces.concat()))
}

fn f_transpose_regions(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let (s1, e1) = beg_end(i, &a, 0, 1)?;
    let (s2, e2) = beg_end(i, &a, 2, 3)?;
    if s1 > s2 {
        return f_transpose_regions(
            i,
            vec![a[2].clone(), a[3].clone(), a[0].clone(), a[1].clone()].into(),
        );
    }
    let b = cur(i);
    let bb = b.borrow_mut();
    if e1 > s2 {
        return Err(err_sym(i, "error", vec![Value::string("Regions overlap")]));
    }
    let t1 = bb.text.substring(s1, e1);
    let t2 = bb.text.substring(s2, e2);
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s2, e2)?;
    crate::buffer::primitives::chg_insert(i, s2, &t1)?;
    crate::buffer::primitives::chg_delete(i, s1, e1)?;
    crate::buffer::primitives::chg_insert(i, s1, &t2)?;
    Ok(Value::Nil)
}

fn f_subst_char_in_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    // GNU Fsubst_char_in_region: validate_interval_range on START/END
    // signals (args-out-of-range BUFFER START END) when out of bounds
    // — no clamping.
    let (s_raw, e_raw) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let (begv, len) = {
        let b = cur(i);
        let bb = b.borrow();
        (bb.begv as i128, bb.text_len() as i128)
    };
    if s_raw < begv + 1 || e_raw > len + 1 || s_raw > e_raw {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![Value::Buffer(cur(i)), a[0].clone(), a[1].clone()],
        ));
    }
    let (s, e) = (s_raw as usize - 1, e_raw as usize - 1);
    let from = match &a[2] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let to = match &a[3] {
        Value::Int(n) if (0..=0x3FFFFF).contains(n) => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let fc = char::from_u32(from).unwrap_or('\u{FFFD}');
    let tc = char::from_u32(to).unwrap_or('\u{FFFD}');
    // GNU calls modify_text (before-change + MODIFF bump) only when a
    // match exists, at the FIRST match position, covering [pos, end);
    // after-change spans first..last substitution.  No match -> no
    // hooks, no tick, nil.
    let (first, last) = {
        let b = cur(i);
        let bb = b.borrow();
        let mut first = None;
        let mut last = 0;
        for p in s..e.min(bb.text.len()) {
            if bb.text.char_at(p) == fc {
                if first.is_none() {
                    first = Some(p);
                }
                last = p + 1;
            }
        }
        match first {
            Some(f) => (f, last),
            None => return Ok(Value::Nil),
        }
    };
    crate::lisp::builtins::evalfn::signal_before_change(i, first + 1, e + 1)?;
    {
        let b = cur(i);
        let mut bb = b.borrow_mut();
        // GNU substitutes in place: point and markers are undisturbed.
        for p in first..e.min(bb.text.len()) {
            if bb.text.char_at(p) == fc {
                bb.text.set_char_at(p, tc);
            }
        }
        bb.note_text_change(e - first);
    }
    crate::lisp::builtins::evalfn::signal_after_change(i, first + 1, last + 1, last - first)?;
    Ok(Value::Nil)
}

/// GNU `check_translation': VAL is an alist (([FROM-CHAR ...] . TO)
/// ...) — find the entry whose FROM sequence matches the text at
/// POS, returning (FROM-LEN . TO).
fn check_translation(chars: &[char], pos: usize, val: &Value) -> Option<(usize, Value)> {
    let mut cur = val.clone();
    while let Value::Cons(c) = cur {
        let (elt, next) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        cur = next;
        let (from, to) = match &elt {
            Value::Cons(e) => {
                let b = e.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            _ => continue,
        };
        let seq = match &from {
            Value::Vec(v) => v.borrow().clone(),
            _ => continue,
        };
        if seq.len() > chars.len() - pos {
            continue;
        }
        let matches = seq.iter().enumerate().all(|(k, v)| {
            matches!(v, Value::Int(n) if char::from_u32(*n as u32) == Some(chars[pos + k]))
        });
        if matches {
            return Some((seq.len(), to));
        }
    }
    None
}

/// Core of GNU `translate-region-internal': map chars in S..E
/// through TABLE (a string or a `translation-table' char-table),
/// returning the number of characters changed.
fn translate_region_core(i: &mut Interp, s: usize, e: usize, table: &Value) -> Result<i128, Flow> {
    // TABLE: string of 256 chars mapping byte->char, or char-table
    // with `translation-table' purpose (GNU signals otherwise).
    let strtab: Option<Vec<char>> = match table {
        Value::Str(t) => Some(t.borrow().chars().collect()),
        _ => None,
    };
    if strtab.is_none() {
        let is_tt = crate::lisp::builtins::misc::is_char_table(i, table)
            && matches!(table, Value::Record(r)
                if matches!(r.borrow().get(1), Some(Value::Sym(s)) if i.symbol_name(*s) == "translation-table"));
        if !is_tt {
            return Err(i.error("Not a translation table"));
        }
    }
    let b = cur(i);
    let bb = b.borrow_mut();
    let region: Vec<char> = bb.text.substring(s, e).chars().collect();
    let ii: &Interp = i;
    // GNU `translate-region-internal': per char, the table's value is
    // a fixnum char (1:1), a [TO-CHAR ...] vector, or a CONS alist of
    // (([FROM-CHAR ...] . TO) ...) checked against the text at point.
    let mut out = String::new();
    let mut pos = 0usize;
    let mut changed = 0usize;
    while pos < region.len() {
        let oc = region[pos];
        let val = if let Some(t) = &strtab {
            match t.get(oc as usize) {
                Some(&nc) if nc != '\0' => Value::Int(nc as i128),
                _ => Value::Nil,
            }
        } else {
            crate::lisp::builtins::misc::char_table_ref(ii, table, oc as usize)
        };
        match val {
            Value::Int(n) => {
                if let Some(nc) = char::from_u32(n as u32) {
                    out.push(nc);
                    if nc != oc {
                        changed += 1;
                    }
                } else {
                    out.push(oc);
                }
                pos += 1;
            }
            Value::Vec(v) => {
                let start = out.len();
                for ch in v.borrow().iter() {
                    if let Value::Int(c) = ch {
                        if let Some(c) = char::from_u32(*c as u32) {
                            out.push(c);
                        }
                    }
                }
                changed += out[start..].chars().count();
                pos += 1;
            }
            Value::Cons(_) => match check_translation(&region, pos, &val) {
                Some((len, to)) => {
                    let start = out.len();
                    match to {
                        Value::Int(c) => {
                            if let Some(c) = char::from_u32(c as u32) {
                                out.push(c);
                            }
                        }
                        Value::Vec(v) => {
                            for ch in v.borrow().iter() {
                                if let Value::Int(c) = ch {
                                    if let Some(c) = char::from_u32(*c as u32) {
                                        out.push(c);
                                    }
                                }
                            }
                        }
                        _ => {
                            for c in &region[pos..pos + len] {
                                out.push(*c);
                            }
                        }
                    }
                    changed += out[start..].chars().count();
                    pos += len;
                }
                None => {
                    out.push(oc);
                    pos += 1;
                }
            },
            _ => {
                out.push(oc);
                pos += 1;
            }
        }
    }
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &out)?;
    Ok(changed as i128)
}

fn f_translate_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    // GNU's Lisp `translate-region' resolves a SYMBOL table through
    // its `translation-table' property, then delegates to
    // `translate-region-internal'.
    let mut table = a[2].clone();
    if let Value::Sym(sid) = &table {
        let tpid = i.intern("translation-table");
        let p = i.get_prop(*sid, tpid);
        // GNU: (or (char-table-p val) (error "Invalid translation table
        // name: %s" table)).
        if !crate::lisp::builtins::misc::is_char_table(i, &p) {
            let name = i.symbol_name(*sid).to_string();
            return Err(i.error(&format!("Invalid translation table name: {}", name)));
        }
        table = p;
    }
    Ok(Value::Int(translate_region_core(i, s, e, &table)?))
}

/// GNU mule.el `make-translation-table-from-alist': builds a forward
/// table plus a reverse table (extra slot 0); extra slot 1 is the
/// max FROM sequence length.  Each cell may hold TO, or a list of
/// (([FROM-CHAR ...] . TO) ...) built by nconc'ing new entries.
fn f_make_translation_table_from_alist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let items = a[0].list_to_vec().unwrap_or_default();
    let tag = Value::Sym(i.intern("translation-table"));
    let forward =
        crate::lisp::builtins::misc::make_ct(i, tag.clone(), Value::Nil, vec![Value::Nil; 2]);
    let reverse = crate::lisp::builtins::misc::make_ct(i, tag, Value::Nil, vec![Value::Nil; 2]);
    for pass in 0..2 {
        let table = if pass == 0 { &forward } else { &reverse };
        let mut max_lookup = 1i128;
        for item in &items {
            let (car, cdr) = match item {
                Value::Cons(c) => {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                }
                _ => continue,
            };
            let (from, to) = if pass == 0 { (car, cdr) } else { (cdr, car) };
            let idx = match &from {
                Value::Int(c) => Some(*c),
                Value::Vec(v) => {
                    let v = v.borrow();
                    if v.is_empty() {
                        None
                    } else {
                        max_lookup = max_lookup.max(v.len() as i128);
                        match &v[0] {
                            Value::Int(c) => Some(*c),
                            _ => None,
                        }
                    }
                }
                _ => None,
            };
            let Some(idx) = idx else { continue };
            if !(0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(&idx) {
                continue;
            }
            let val = crate::lisp::builtins::misc::char_table_ref(i, table, idx as usize);
            let cell = if !val.is_nil() {
                // Existing entry: a non-list value is first wrapped
                // as ([idx] . val), then (from . to) is nconc'd on.
                let mut vl = match &val {
                    Value::Cons(_) => val.list_to_vec().unwrap_or_default(),
                    _ => vec![Value::cons(
                        Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(
                            idx,
                        )]))),
                        val.clone(),
                    )],
                };
                let f =
                    match &from {
                        Value::Int(c) => Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
                            vec![Value::Int(*c)],
                        ))),
                        _ => from.clone(),
                    };
                vl.push(Value::cons(f, to));
                Value::list(vl)
            } else if matches!(from, Value::Int(_)) {
                to
            } else {
                Value::list(vec![Value::cons(from.clone(), to)])
            };
            crate::lisp::builtins::misc::ct_set(i, table, idx as u32, cell);
        }
        // Extra slot 1: max-lookup for this direction.
        if let Value::Record(r) = table {
            r.borrow_mut()[4] = Value::Int(max_lookup);
        }
    }
    // Extra slot 0 of the forward table: the reverse table.
    if let Value::Record(r) = &forward {
        r.borrow_mut()[3] = reverse;
    }
    Ok(forward)
}

fn f_buffer_swap_text(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fbuffer_swap_text takes a buffer OBJECT (CHECK_BUFFER), not
    // a name — and the checks run in this order: dead, then indirect,
    // then has-indirect-children.
    let other = match &a[0] {
        Value::Buffer(b) => b.clone(),
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    let cur_b = cur(i);
    if !other.borrow().live {
        return Err(err_sym(
            i,
            "error",
            vec![Value::string("Cannot swap a dead buffer’s text")],
        ));
    }
    // Swapping a buffer with itself is a no-op (and avoids a double
    // borrow_mut on the same RefCell).
    if std::rc::Rc::ptr_eq(&cur_b, &other) {
        return Ok(Value::Nil);
    }
    if cur_b.borrow().base_buffer.is_some() || other.borrow().base_buffer.is_some() {
        return Err(err_sym(
            i,
            "error",
            vec![Value::string("Cannot swap indirect buffers’s text")],
        ));
    }
    // Neither buffer may have indirect children hanging off its text.
    let (cur_id, other_id) = (cur_b.borrow().id, other.borrow().id);
    for cid in i.buffers.list() {
        let is_child = i
            .buffers
            .get(cid)
            .and_then(|cb| cb.borrow().base_buffer.as_ref().map(|br| br.borrow().id))
            .map(|bid| bid == cur_id || bid == other_id)
            .unwrap_or(false);
        if is_child {
            return Err(err_sym(
                i,
                "error",
                vec![Value::string(
                    "One of the buffers to swap has indirect buffers",
                )],
            ));
        }
    }
    let (mut mine, mut theirs) = (cur_b.borrow_mut(), other.borrow_mut());
    // GNU swapfield(own_text): the text storage — including intervals
    // (our SharedText::props) and MODIFF ticks — moves wholesale.
    std::mem::swap(&mut mine.text, &mut theirs.text);
    // Move the viewer registrations with the text: post-swap each
    // buffer views the other's former SharedText.
    mine.text.remove_viewer(&other);
    mine.text.add_viewer(&cur_b);
    theirs.text.remove_viewer(&cur_b);
    theirs.text.add_viewer(&other);
    // swapfield pt/begv/zv — point and narrowing travel with the text.
    std::mem::swap(&mut mine.point, &mut theirs.point);
    std::mem::swap(&mut mine.begv, &mut theirs.begv);
    std::mem::swap(&mut mine.zv, &mut theirs.zv);
    // GNU swapfield_(undo_list), swapfield_(mark), swapfield_(mark_active).
    let undo_sym = mine.undo_sym;
    let mu = mine.locals.shift_remove(&undo_sym).unwrap_or(Value::Nil);
    let tu = theirs.locals.shift_remove(&undo_sym).unwrap_or(Value::Nil);
    mine.locals.insert(undo_sym, tu);
    theirs.locals.insert(undo_sym, mu);
    std::mem::swap(&mut mine.mark, &mut theirs.mark);
    let ma = i.intern("mark-active");
    let (mm_a, tm_a) = (
        mine.locals.shift_remove(&ma),
        theirs.locals.shift_remove(&ma),
    );
    if let Some(v) = tm_a {
        mine.locals.insert(ma, v);
    }
    if let Some(v) = mm_a {
        theirs.locals.insert(ma, v);
    }
    for s in [
        "enable-multibyte-characters",
        "bidi-display-reordering",
        "bidi-paragraph-direction",
        "bidi-paragraph-separate-re",
        "bidi-paragraph-start-re",
    ] {
        let sid = i.intern(s);
        let (mv, tv) = (
            mine.locals.shift_remove(&sid),
            theirs.locals.shift_remove(&sid),
        );
        if let Some(v) = tv {
            mine.locals.insert(sid, v);
        }
        if let Some(v) = mv {
            theirs.locals.insert(sid, v);
        }
    }
    // GNU swap_buffer_overlays: overlay objects stay with their buffer
    // handle but their positions… actually GNU swaps the overlay
    // trees — each overlay's `buffer' retargets to the other buffer.
    std::mem::swap(&mut mine.overlays, &mut theirs.overlays);
    for ov in &mut mine.overlays {
        if ov.buffer == Some(other_id) {
            ov.buffer = Some(cur_id);
        }
    }
    for ov in &mut theirs.overlays {
        if ov.buffer == Some(cur_id) {
            ov.buffer = Some(other_id);
        }
    }
    // GNU: markers follow the text — each buffer's marker list moves
    // to the other buffer and the markers retarget their owner.
    std::mem::swap(&mut mine.markers, &mut theirs.markers);
    for w in &mine.markers {
        if let Some(m) = w.upgrade() {
            let mut mm = m.borrow_mut();
            if mm.buffer == Some(other_id) {
                mm.buffer = Some(cur_id);
            }
        }
    }
    for w in &theirs.markers {
        if let Some(m) = w.upgrade() {
            let mut mm = m.borrow_mut();
            if mm.buffer == Some(cur_id) {
                mm.buffer = Some(other_id);
            }
        }
    }
    // GNU bumps MODIFF/CHARS_MODIFF on both texts after the swap.
    mine.text.mod_tick.set(mine.text.mod_tick.get() + 1);
    mine.text
        .chars_mod_tick
        .set(mine.text.chars_mod_tick.get() + 1);
    theirs.text.mod_tick.set(theirs.text.mod_tick.get() + 1);
    theirs
        .text
        .chars_mod_tick
        .set(theirs.text.chars_mod_tick.get() + 1);
    Ok(Value::Nil)
}

fn f_buffer_last_name(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: last name before the last rename or the kill; a fresh
    // buffer's last name is its creation name.
    let b = crate::buffer::primitives::buf_of(i, &crate::lisp::builtins::arg(&a, 0))?;
    match &b.borrow().last_name {
        Some(n) => Ok(Value::string(n.clone())),
        None => Ok(Value::Nil),
    }
}

fn f_buffer_mod_tick(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: optional BUFFER arg via buf_of.
    let b = crate::buffer::primitives::buf_of(i, &crate::lisp::builtins::arg(&a, 0))?;
    // GNU stores the raw fixnum — negative TICKs read back negative.
    Ok(Value::Int(b.borrow().text.mod_tick.get() as i64 as i128))
}

fn f_buffer_chars_mod_tick(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let b = crate::buffer::primitives::buf_of(i, &crate::lisp::builtins::arg(&a, 0))?;
    Ok(Value::Int(b.borrow().text.chars_mod_tick.get() as i128))
}

fn f_store_match_data(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Same as set-match-data plus reseat markers (we use plain ints).
    let items = a[0].list_to_vec().unwrap_or_default();
    let mut regs = Vec::new();
    let in_buffer = i.match_data.as_ref().map(|m| m.in_buffer).unwrap_or(true);
    let base = i.match_data.as_ref().map(|m| m.base).unwrap_or(0);
    let off = base + if in_buffer { 1 } else { 0 };
    let to_pos = |v: &Value| -> Option<usize> {
        // GNU accepts markers in the list too (match-data returns them).
        match v {
            Value::Marker(m) => {
                let mm = m.borrow();
                mm.buffer.map(|_| mm.position.saturating_sub(base))
            }
            _ => v.int().map(|p| (p as usize).saturating_sub(off)),
        }
    };
    let mut k = 0;
    while k < items.len() {
        let s = to_pos(&items[k]);
        let e = items.get(k + 1).and_then(|v| to_pos(v));
        regs.push(s);
        regs.push(e);
        k += 2;
    }
    i.match_data = Some(crate::lisp::MatchData {
        regs,
        in_buffer,
        base,
        buffer: None,
    });
    Ok(Value::Nil)
}

// ---------- text-property scans ----------

/// GNU `find_field' (editfns.c): fields left/right of POS, the
/// at_field_start/at_field_end edge detection driven by the
/// rear-sticky `get-pos-property' value, then boundary scans via
/// previous/next-single-char-property-change.  Positions are 1-based
/// Lisp positions; BEG_LIMIT/END_LIMIT clamp the scans.
fn find_field(
    i: &mut Interp,
    pos_v: Option<i128>,
    merge_at_boundary: bool,
    beg_limit: Option<&Value>,
    beg: Option<&mut i128>,
    end_limit: Option<&Value>,
    end: Option<&mut i128>,
) -> Result<(), Flow> {
    let pos = pos_v.unwrap_or_else(|| cur(i).borrow().point() as i128 + 1);
    let begv = cur(i).borrow().begv as i128 + 1;
    let zv = cur(i).borrow().zv as i128 + 1;
    // GNU find_field validates the position itself before any
    // get-pos-property call: out-of-[BEGV,ZV] signals a bare
    // (args-out-of-range POS) — the single-element payload.
    if pos < begv || pos > zv {
        return Err(err_sym(i, "args-out-of-range", vec![Value::Int(pos)]));
    }
    let field_sym = i.intern("field");
    let boundary_sym = i.intern("boundary");
    let gcp = |i: &mut Interp, p: i128| -> Value {
        if p < begv || p > zv {
            return Value::Nil;
        }
        let idx = (p - 1).max(0) as usize;
        let b = cur(i);
        let bb = b.borrow();
        crate::buffer::primitives::char_prop_at_overlay(i, &bb, idx, field_sym).0
    };
    let gpos = |i: &mut Interp, p: i128| -> EvalResult {
        crate::buffer::primitives::f_get_pos_property(
            i,
            vec![Value::Int(p), Value::Sym(field_sym)].into(),
        )
    };
    let next_change = |i: &mut Interp, p: i128, lim: Option<&Value>| -> EvalResult {
        let limit = match lim {
            Some(v) if !v.is_nil() => {
                Value::Int(crate::buffer::primitives::text_property_position(i, v)?.min(zv))
            }
            _ => Value::Nil,
        };
        crate::buffer::primitives::f_next_single_char_property_change(
            i,
            vec![Value::Int(p), Value::Sym(field_sym), Value::Nil, limit].into(),
        )
    };
    let prev_change = |i: &mut Interp, p: i128, lim: Option<&Value>| -> EvalResult {
        let limit = match lim {
            Some(v) if !v.is_nil() => {
                Value::Int(crate::buffer::primitives::text_property_position(i, v)?)
            }
            _ => Value::Nil,
        };
        crate::buffer::primitives::f_prev_single_char_property_change(
            i,
            vec![Value::Int(p), Value::Sym(field_sym), Value::Nil, limit].into(),
        )
    };
    let after_field = gcp(i, pos);
    let before_field = if pos > begv {
        gcp(i, pos - 1)
    } else {
        after_field.clone()
    };
    let mut at_field_start = false;
    let mut at_field_end = false;
    if !merge_at_boundary {
        let field = gpos(i, pos)?;
        if !crate::lisp::builtins::eq_values(&field, &after_field) {
            at_field_end = true;
        }
        if !crate::lisp::builtins::eq_values(&field, &before_field) {
            at_field_start = true;
        }
        if field.is_nil() && at_field_start && at_field_end {
            // An inserted char would have a nil field while both
            // sides are non-nil — a non-sticky field meant for
            // display, not a real boundary (GNU's comint-prompt case).
            at_field_start = false;
            at_field_end = false;
        }
    }
    if let Some(b) = beg {
        if at_field_start {
            *b = pos;
        } else {
            let mut p = pos;
            if merge_at_boundary
                && crate::lisp::builtins::eq_values(&before_field, &Value::Sym(boundary_sym))
            {
                p = prev_change(i, p, beg_limit)?.int().unwrap_or(begv);
            }
            let p2 = prev_change(i, p, beg_limit)?;
            *b = p2.int().unwrap_or(begv);
        }
    }
    if let Some(e) = end {
        if at_field_end {
            *e = pos;
        } else {
            let mut p = pos;
            if merge_at_boundary
                && crate::lisp::builtins::eq_values(&after_field, &Value::Sym(boundary_sym))
            {
                p = next_change(i, p, end_limit)?.int().unwrap_or(zv);
            }
            let p2 = next_change(i, p, end_limit)?;
            *e = p2.int().unwrap_or(zv);
        }
    }
    Ok(())
}

/// (BEG, END) 1-based bounds of the field at POS via `find_field'.
fn find_field_bounds(
    i: &mut Interp,
    pos_v: Option<i128>,
    merge: bool,
    blim: Option<i128>,
    elim: Option<i128>,
) -> Result<(i128, i128), Flow> {
    let (mut b, mut e) = (0i128, 0i128);
    let blim = blim.map(Value::Int);
    let elim = elim.map(Value::Int);
    find_field(
        i,
        pos_v,
        merge,
        blim.as_ref(),
        Some(&mut b),
        elim.as_ref(),
        Some(&mut e),
    )?;
    Ok((b, e))
}

fn f_field_beginning(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match a.first() {
        Some(v) if !v.is_nil() => Some(crate::buffer::primitives::text_property_position(i, v)?),
        _ => None,
    };
    let mut beg = 0;
    find_field(
        i,
        pos,
        arg(&a, 1).truthy(),
        a.get(2),
        Some(&mut beg),
        None,
        None,
    )?;
    Ok(Value::Int(beg))
}

fn f_field_end(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let pos = match a.first() {
        Some(v) if !v.is_nil() => Some(crate::buffer::primitives::text_property_position(i, v)?),
        _ => None,
    };
    let mut end = 0;
    find_field(
        i,
        pos,
        arg(&a, 1).truthy(),
        None,
        None,
        a.get(2),
        Some(&mut end),
    )?;
    Ok(Value::Int(end))
}

fn f_field_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decode_position_or_default: a present non-nil POS must be an
    // integer or marker — anything else signals integer-or-marker-p.
    let pos = match a.get(0) {
        Some(v) if !v.is_nil() => Some(match v {
            Value::Int(n) => *n,
            Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
            Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
            _ => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
        }),
        _ => None,
    };
    let (s, e) = find_field_bounds(i, pos, false, None, None)?;
    let v = crate::buffer::primitives::f_buffer_substring(
        i,
        vec![Value::Int(s), Value::Int(e)].into(),
    )?;
    // field-string-no-properties shares this fn; the caller strips
    // via the OBJECT arg convention — distinguish by subr name is not
    // possible here, so look at the registration: the -no-properties
    // variant calls f_field_string_no_properties below.
    Ok(v)
}

fn f_field_string_no_properties(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decode_position_or_default: a present non-nil POS must be an
    // integer or marker — anything else signals integer-or-marker-p.
    let pos = match a.get(0) {
        Some(v) if !v.is_nil() => Some(match v {
            Value::Int(n) => *n,
            Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
            Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
            _ => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
        }),
        _ => None,
    };
    let (s, e) = find_field_bounds(i, pos, false, None, None)?;
    crate::buffer::primitives::f_buffer_substring_no_properties(
        i,
        vec![Value::Int(s), Value::Int(e)].into(),
    )
}

fn f_delete_field(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    check_writable(i)?;
    // GNU decode_position_or_default: a present non-nil POS must be an
    // integer or marker — anything else signals integer-or-marker-p.
    let pos = match a.get(0) {
        Some(v) if !v.is_nil() => Some(match v {
            Value::Int(n) => *n,
            Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
            Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
            _ => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
        }),
        _ => None,
    };
    let (s, e) = find_field_bounds(i, pos, false, None, None)?;
    if s != e {
        crate::buffer::primitives::chg_delete(i, (s - 1) as usize, (e - 1) as usize)?;
    }
    Ok(Value::Nil)
}

fn f_constrain_to_field(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fconstrain_to_field (editfns.c): clamp NEW-POS into the
    // field OLD-POS lives in; nil NEW-POS means point (and moves it).
    let move_pt = a[0].is_nil();
    let new_v = if move_pt {
        cur(i).borrow().point() as i128 + 1
    } else {
        // want_int_or_mark carries GNU's CHECK_FIXNUM_COERCE_MARKER:
        // int → position, live marker → 1-based position, detached
        // marker → "Marker does not point anywhere".
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?
    };
    let old_v = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
    let merge = arg(&a, 2).truthy();
    let only_in_line = arg(&a, 3).truthy();
    let inhibit_prop = a.get(4).cloned().unwrap_or(Value::Nil);
    let field_sym = i.intern("field");
    let begv = cur(i).borrow().begv as i128 + 1;
    let zv = cur(i).borrow().zv as i128 + 1;
    // GNU calls Fget_char_property at each position — an out-of-range
    // pos signals args-out-of-range (POS) rather than returning nil.
    let gcp = |i: &mut Interp, p: i128| -> Result<Value, Flow> {
        if p < begv || p > zv {
            return Err(err_sym(i, "args-out-of-range", vec![Value::Int(p)]));
        }
        let b = cur(i);
        let bb = b.borrow();
        Ok(crate::buffer::primitives::char_prop_at_overlay(i, &bb, (p - 1) as usize, field_sym).0)
    };
    let fwd = new_v > old_v;
    let mut result = new_v;
    let inhibit_motion = i
        .intern_soft("inhibit-field-text-motion")
        .map(|id| i.symbol_value(id).truthy())
        .unwrap_or(false);
    // GNU order: gcp runs only when motion isn't inhibited and
    // new_pos != old_pos (BASE_EQ short-circuits before the calls);
    // `captured' is computed first via get-pos-property.
    let mut captured = false;
    if inhibit_prop.truthy() {
        let psym = i.sym_id(&inhibit_prop).unwrap_or(sym::NIL);
        let pos_val = crate::buffer::primitives::f_get_pos_property(
            i,
            vec![Value::Int(old_v), inhibit_prop.clone()].into(),
        )?;
        captured = pos_val.truthy()
            && (old_v <= begv
                || crate::buffer::primitives::char_prop_at_overlay(
                    i,
                    &cur(i).borrow(),
                    (old_v - 1) as usize,
                    psym,
                )
                .0
                .truthy()
                || crate::buffer::primitives::char_prop_at_overlay(
                    i,
                    &cur(i).borrow(),
                    (old_v - 2).max(0) as usize,
                    psym,
                )
                .0
                .truthy());
    }
    let mut has_field = false;
    if !inhibit_motion && new_v != old_v && !captured {
        if !gcp(i, new_v)?.is_nil() || !gcp(i, old_v)?.is_nil() {
            has_field = true;
        } else {
            if new_v > begv && !gcp(i, new_v - 1)?.is_nil() {
                has_field = true;
            }
            if !has_field && old_v > begv && !gcp(i, old_v - 1)?.is_nil() {
                has_field = true;
            }
        }
    }
    if !inhibit_motion && new_v != old_v && has_field && !captured {
        let field_bound = if fwd {
            let (_, e) = find_field_bounds(i, Some(old_v), merge, None, Some(new_v))?;
            e
        } else {
            let (b, _) = find_field_bounds(i, Some(old_v), merge, Some(new_v), None)?;
            b
        };
        // If ESCAPE-FROM-EDGE moved the bound past NEW-POS, NEW-POS is
        // already inside the right field — leave it.
        let need = if field_bound < new_v { fwd } else { !fwd };
        if need {
            let same_line = if only_in_line {
                // Constraint is OK only when NEW-POS and FIELD_BOUND
                // share a line.
                let b = cur(i);
                let bb = b.borrow();
                let (lo, hi) = (new_v.min(field_bound), new_v.max(field_bound));
                let (s, e) = ((lo - 1).max(0) as usize, (hi - 1).max(0) as usize);
                !bb.text.substring(s, e.min(bb.text.len())).contains('\n')
            } else {
                true
            };
            if same_line {
                result = field_bound;
            }
        }
    }
    if move_pt {
        cur(i).borrow_mut().set_point((result - 1).max(0) as usize);
    }
    Ok(Value::Int(result))
}

fn f_get_char_prop_and_overlay(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `get_char_property_and_overlay': (PROPVAL . OVERLAY) where
    // OVERLAY is the winning overlay or nil for a text property.
    // GNU order: POSITION type (integer-or-marker-p) plus marker liveness
    // — a detached marker signals "Marker does not point anywhere" before
    // the OBJECT check — then OBJECT (nil = current buffer, a window
    // stands for its buffer, a string takes the get-text-property path,
    // anything else signals `buffer-or-string-p'), then POS against
    // BEGV..ZV with a bare-POS `args-out-of-range' datum.
    match &a[0] {
        Value::Int(_) => {}
        Value::Marker(m) if m.borrow().buffer.is_some() => {}
        Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
    let b = match a.get(2) {
        None | Some(Value::Nil) => cur(i),
        Some(Value::Buffer(_)) | Some(Value::Window(_)) => {
            crate::buffer::primitives::prop_object_buffer(i, a.get(2))?
        }
        Some(Value::Str(_)) => {
            // Strings have no overlays: (text-prop . nil) via the
            // get-text-property path (its string-length check applies).
            // GNU's text_property requires a fixnat POSITION, so a
            // marker — even a live one — yields (nil . nil), never a
            // coercion or range check.
            if matches!(&a[0], Value::Marker(_)) {
                return Ok(Value::cons(Value::Nil, Value::Nil));
            }
            let v = crate::buffer::primitives::f_get_text_property(i, a)?;
            return Ok(Value::cons(v, Value::Nil));
        }
        Some(other) => return Err(i.wrong_type_mut("buffer-or-string-p", other)),
    };
    // GNU resolves POSITION inside the OBJECT before the PROP test:
    // an out-of-range POS signals even when PROP is not a symbol.
    let pos = {
        let bb = b.borrow();
        let p = crate::buffer::primitives::want_pos_in_buf_dup(i, &a[0], &bb, false)?;
        pos_idx(bb.text.len(), p)
    };
    let Some(prop) = i.sym_id(&a[1]) else {
        // The PROPVAL car is consed unconditionally: a non-symbol PROP
        // still yields (nil . nil).
        return Ok(Value::cons(Value::Nil, Value::Nil));
    };
    let bb = b.borrow();
    let (v, ov) = crate::buffer::primitives::char_prop_at_overlay(i, &bb, pos, prop);
    Ok(Value::cons(v, ov))
}

fn f_next_prop_change_fwd(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `next-char-property-change': (POSITION &optional LIMIT) —
    // no OBJECT arg at all; the second arg is LIMIT, an
    // integer-or-marker.  GNU coerces LIMIT (type + marker liveness)
    // before range-checking POS, clamps it only at ZV — a LIMIT below
    // POS is returned raw — then Fnext_overlay_change /
    // Fnext_property_change bounded by it.
    let pos_v = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let len = cur(i).borrow().text_len();
    let begv = cur(i).borrow().begv;
    let explicit_lim = match a.get(1) {
        Some(v) if v.truthy() => Some(crate::lisp::builtins::want_int_or_mark(i, v)?),
        _ => None,
    };
    if pos_v < begv as i128 + 1 || pos_v > len as i128 + 1 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_idx(len, pos_v);
    // 1-based LIMIT; an explicit value is capped at ZV but not at BEGV.
    let lim = explicit_lim
        .map(|l| l.min(len as i128 + 1))
        .unwrap_or(len as i128 + 1);
    let mut next: Option<usize> = None;
    {
        let b = cur(i);
        let bb = b.borrow();
        let mut take = |cand: usize| {
            // A boundary at 0-based index CAND is position CAND+1.
            if cand > pos && cand as i128 + 1 <= lim && next.map(|n| cand < n).unwrap_or(true) {
                next = Some(cand);
            }
        };
        for cand in bb.text.props.borrow().change_points(false) {
            take(cand);
        }
        for ov in &bb.overlays {
            if ov.buffer == Some(bb.id) {
                take(ov.start);
                take(ov.end);
            }
        }
    }
    Ok(Value::Int(next.map(|p| p as i128 + 1).unwrap_or(lim)))
}

/// `previous-char-property-change' — like `previous-property-change'
/// but also considers overlays; defaults to point-min (not nil) when
/// nothing changes before POS.
fn f_prev_prop_change(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU `previous-char-property-change': (POSITION &optional LIMIT) —
    // no OBJECT arg.  GNU coerces LIMIT (type + marker liveness) before
    // range-checking POS, then clamps it only at BEGV — a LIMIT above
    // POS is returned raw.  Default LIMIT is point-min.
    let pos_v = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let len = cur(i).borrow().text_len();
    let begv = cur(i).borrow().begv;
    let explicit_lim = match a.get(1) {
        Some(v) if v.truthy() => Some(crate::lisp::builtins::want_int_or_mark(i, v)?),
        _ => None,
    };
    if pos_v < begv as i128 + 1 || pos_v > len as i128 + 1 {
        return Err(err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[0].clone()],
        ));
    }
    let pos = pos_idx(len, pos_v);
    // 1-based LIMIT; an explicit value is floored at BEGV but not
    // capped at ZV.
    let lim = explicit_lim
        .map(|l| l.max(begv as i128 + 1))
        .unwrap_or(begv as i128 + 1);
    let mut prev: Option<usize> = None;
    {
        let b = cur(i);
        let bb = b.borrow();
        let mut take = |cand: usize| {
            if cand < pos && cand as i128 + 1 >= lim && prev.map(|n| cand > n).unwrap_or(true) {
                prev = Some(cand);
            }
        };
        for cand in bb.text.props.borrow().change_points(false) {
            take(cand);
        }
        for ov in &bb.overlays {
            if ov.buffer == Some(bb.id) {
                take(ov.start);
                take(ov.end);
            }
        }
    }
    Ok(Value::Int(prev.map(|p| p as i128 + 1).unwrap_or(lim)))
}

fn f_marker_last_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU returns the marker's
    // position; non-marker signals wrong-type-argument markerp.
    match &a[0] {
        Value::Marker(m) => Ok(Value::Int(m.borrow().position as i128 + 1)),
        other => Err(i.wrong_type_mut("markerp", other)),
    }
}

// ---------- motion ----------

fn f_compute_motion(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (compute-motion FROM FROMVCONS TO TOVCONS WIDTH OFFSETS WINDOW)
    // GNU indent.c Fcompute_motion arg checks, in order:
    // CHECK_FIXNUM_COERCE_MARKER(FROM), CHECK_CONS(FROMPOS) +
    // fixnum car/cdr, CHECK_FIXNUM_COERCE_MARKER(TO), TOPOS nil|cons of
    // fixnums, CHECK_FIXNUM(WIDTH), OFFSETS nil|cons of fixnums,
    // decode_live_window(WINDOW).
    match &a[0] {
        Value::Int(_) => {}
        Value::Marker(_) => {}
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
    let mut pair = |v: &Value| -> Result<(), Flow> {
        match v {
            Value::Nil => Ok(()),
            Value::Cons(c) => {
                let cc = c.borrow();
                match &cc.car {
                    Value::Int(_) => {}
                    other => return Err(i.wrong_type_mut("fixnump", other)),
                }
                match &cc.cdr {
                    Value::Int(_) => {}
                    other => return Err(i.wrong_type_mut("fixnump", other)),
                }
                Ok(())
            }
            other => Err(i.wrong_type_mut("consp", other)),
        }
    };
    match &a[1] {
        Value::Cons(c) => {
            let cc = c.borrow();
            match &cc.car {
                Value::Int(_) => {}
                other => return Err(i.wrong_type_mut("fixnump", other)),
            }
            match &cc.cdr {
                Value::Int(_) => {}
                other => return Err(i.wrong_type_mut("fixnump", other)),
            }
        }
        other => return Err(i.wrong_type_mut("consp", other)),
    }
    match &a[2] {
        Value::Int(_) | Value::Marker(_) => {}
        other => return Err(i.wrong_type_mut("integer-or-marker-p", other)),
    }
    pair(&a[3])?;
    match &a[4] {
        Value::Int(_) => {}
        other => return Err(i.wrong_type_mut("fixnump", other)),
    }
    pair(&a[5])?;
    let _w = crate::editor::win_live(i, &a[6])?;
    let to = a.get(2).and_then(|v| v.int()).unwrap_or(1);
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text_len();
    let target = pos_idx(len, to);
    let mut col = 0i128;
    let mut vpos = 0i128;
    let mut p = bb.begv;
    let mut last_bol = 0i128;
    while p < target {
        match bb.text.char_at(p) {
            '\n' => {
                vpos += 1;
                col = 0;
                last_bol = col;
            }
            '\t' => {
                last_bol = col;
                col += 8 - (col % 8);
            }
            c => {
                last_bol = col;
                col += crate::buffer::primitives::char_width(c);
            }
        }
        p += 1;
    }
    Ok(Value::list(vec![
        Value::Int(target as i128 + 1),
        Value::Int(col),
        Value::Int(vpos),
        Value::Int(last_bol),
        Value::Nil,
    ]))
}

fn f_vertical_motion(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (vertical-motion LINES &optional WINDOW CUR-COL).  LINES may be a
    // cons (COLS . LINES); without a real display matrix (batch) GNU
    // moves LINES lines and lands at the beginning of the line, and the
    // return value is the signed number of lines actually moved.
    // GNU Fvertical_motion checks LINES first: a cons means
    // (numberp CAR — floats allowed) for the columns and fixnump CDR
    // for the lines; a bare LINES must be a fixnum.  Then the optional
    // WINDOW decodes via `window-live-p'.
    let lines = match &a[0] {
        Value::Cons(c) => {
            let cc = c.borrow();
            match &cc.car {
                Value::Int(_) | Value::Float(_) => {}
                other => return Err(i.wrong_type_mut("numberp", other)),
            }
            match &cc.cdr {
                Value::Int(n) => *n,
                other => return Err(i.wrong_type_mut("fixnump", other)),
            }
        }
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    if let Some(w) = a.get(1) {
        let _ = crate::editor::win_live(i, w)?;
    }
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text_len();
    let mut p = bb.point();
    let mut moved = 0i128;
    if lines == 0 {
        // Zero lines: GNU still lands on the beginning of the line.
        p = line_start(&bb.text, p);
    } else if lines > 0 {
        for _ in 0..lines {
            let eol = line_end(&bb.text, p);
            if eol >= len {
                p = len;
                break;
            }
            p = eol + 1;
            moved += 1;
        }
    } else {
        for _ in 0..(-lines) {
            // Each step lands on the beginning of the previous line.
            let bol = line_start(&bb.text, p);
            if bol == 0 {
                p = 0;
                break;
            }
            p = line_start(&bb.text, bol - 1);
            moved -= 1;
        }
    }
    bb.set_point(p);
    Ok(Value::Int(moved))
}

fn line_start(text: &crate::buffer::SharedText, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p > 0 && text.char_at(p - 1) != '\n' {
        p -= 1;
    }
    p
}

fn line_end(text: &crate::buffer::SharedText, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p < text.len() && text.char_at(p) != '\n' {
        p += 1;
    }
    p
}

fn f_win_line_height(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fwindow_line_height decodes
    // the optional WINDOW via `window-live-p' first; then returns
    // (HEIGHT VPOS YPOS OFFY) or nil — always nil in batch (no glyphs).
    let _ = crate::editor::win_err_live(i, a.get(1).unwrap_or(&Value::Nil))?;
    if i.noninteractive {
        return Ok(Value::Nil);
    }
    // tty approximation: every line is 1 row tall; VPOS is the row
    // index inside the window (arg 0 = line offset, -1 = last line).
    let w = crate::editor::sel_window(i).unwrap();
    let height = w.borrow().height;
    let line = a.get(0).and_then(|v| v.int()).unwrap_or(0);
    let vpos = if line < 0 {
        height - 1
    } else {
        (line as usize).min(height.saturating_sub(1))
    };
    Ok(Value::list(vec![
        Value::Int(1),
        Value::Int(vpos as i128),
        Value::Int(0),
        Value::Int(0),
    ]))
}

fn f_backward_prefix_chars(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU: skip back over unquoted `''-class chars and `p'-flagged chars.
    let upto = cur(i).borrow().point() + 1;
    let syn = crate::editor::Syn::current(i, upto);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let (beg, pt) = (bb.begv, bb.point());
    let p = {
        let text = bb.text.as_slice();
        crate::buffer::primitives::backward_prefix_chars(&syn, &text, beg, pt)
    };
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_move_point_visually(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fmove_point_visually (xdisp.c): CHECK_FIXNUM, then
    // dir = sign(arg) (0 counts as -1).  In a batch/TTY frame the glyph
    // matrix is empty, so it falls into the `simulate_display' path:
    // PT<=BEGV&&dir<0 -> beginning-of-buffer, PT>=ZV&&dir>0 ->
    // end-of-buffer; otherwise it reseats the display iterator and
    // steps to the next visual column.  In our no-glyph model the
    // batch-observable steps are: from BEGV rightward one char; any
    // interior position GNU's iterator signals (args-out-of-range PT
    // PT) via the interval tree — we reproduce that shape rather than
    // silently stepping.
    let dir = match &a[0] {
        Value::Int(n) if *n > 0 => 1i128,
        Value::Int(_) => -1i128,
        v => return Err(i.wrong_type_mut("fixnump", v)),
    };
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let pt = bb.point() as i128 + 1;
    let begv = bb.begv as i128 + 1;
    let zv = bb.zv as i128 + 1;
    let edge_lo = bb.begv;
    let edge_hi = bb.zv;
    if pt <= begv && dir < 0 {
        bb.set_point(edge_lo);
        drop(bb);
        return Err(i.signal_data(crate::lisp::sym::BEGINNING_OF_BUFFER, vec![]));
    }
    if pt >= zv && dir > 0 {
        bb.set_point(edge_hi);
        drop(bb);
        return Err(i.signal_data(crate::lisp::sym::END_OF_BUFFER, vec![]));
    }
    if pt == begv && dir > 0 {
        // Only batch-observable non-error step: BEGV + rightward.
        bb.set_point(pt as usize);
        drop(bb);
        return Ok(Value::Int(pt + 1));
    }
    let pos = Value::Int(pt);
    drop(bb);
    let sym = i.intern("args-out-of-range");
    Err(i.signal_data(sym, vec![pos.clone(), pos]))
}

// ---------- region encode/decode ----------

fn region_text(i: &mut Interp, a: &[Value]) -> Result<(usize, usize, String), Flow> {
    // GNU validate_region: BEG/END decode as integer-or-marker
    // (present-but-nil signals), are swapped into ascending order,
    // and must lie inside BEGV..ZV — otherwise
    // `args-out-of-range (BUFFER BEG END)' with the ORIGINAL args.
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text_len();
    let mut dec = |v: Option<&Value>| -> Result<i128, Flow> {
        match v {
            Some(v) => match v {
                Value::Int(n) => Ok(*n),
                Value::Marker(m) if m.borrow().buffer.is_some() => {
                    Ok(m.borrow().position as i128 + 1)
                }
                Value::Marker(_) => Err(i.error("Marker does not point anywhere")),
                _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
            },
            None => {
                let sym = i.intern("wrong-number-of-arguments");
                Err(i.signal_data(sym, Vec::new()))
            }
        }
    };
    let sp = dec(a.get(0))?;
    let ep = dec(a.get(1))?;
    let beg = sp.min(ep);
    let end = sp.max(ep);
    let begv1 = bb.begv as i128 + 1;
    let zv1 = bb.zv as i128 + 1;
    if !(begv1 <= beg && end <= zv1) {
        let buf = Value::Buffer(b.clone());
        let a0 = a.get(0).cloned().unwrap_or(Value::Nil);
        let a1 = a.get(1).cloned().unwrap_or(Value::Nil);
        return Err(crate::editor::err_sym(
            i,
            "args-out-of-range",
            vec![buf, a0, a1],
        ));
    }
    let s = pos_idx(len, beg);
    let e = pos_idx(len, end);
    Ok((s, e, bb.text.substring(s, e)))
}

fn f_b64_encode_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let (_s, _e, text) = region_text(i, &a)?;
    let no_break = arg(&a, 2).truthy();
    let eng = if no_break {
        base64::engine::general_purpose::STANDARD_NO_PAD
    } else {
        base64::engine::general_purpose::STANDARD
    };
    let encoded = eng.encode(text.as_bytes());
    // Emacs replaces the region contents and returns its new length.
    check_writable(i)?;
    let (s, e, _) = region_text(i, &a)?;
    let b = cur(i);
    let bb = b.borrow_mut();
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &encoded)?;
    Ok(Value::Int(encoded.chars().count() as i128))
}

fn f_b64url_encode_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let (s, e, text) = region_text(i, &a)?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text.as_bytes());
    check_writable(i)?;
    let b = cur(i);
    let bb = b.borrow_mut();
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &encoded)?;
    Ok(Value::Int(encoded.chars().count() as i128))
}

fn f_b64_decode_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use base64::Engine;
    let (s, e, text) = region_text(i, &a)?;
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|_| err_sym(i, "error", vec![Value::string("Invalid base64 data")]))?;
    let decoded = String::from_utf8_lossy(&bytes).to_string();
    check_writable(i)?;
    let b = cur(i);
    let bb = b.borrow_mut();
    drop(bb);
    crate::buffer::primitives::chg_delete(i, s, e)?;
    crate::buffer::primitives::chg_insert(i, s, &decoded)?;
    Ok(Value::Int(decoded.chars().count() as i128))
}

fn f_undo_boundary(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.undo_boundary();
    Ok(Value::Nil)
}
