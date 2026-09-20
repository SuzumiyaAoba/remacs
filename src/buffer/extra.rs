//! Extra buffer subrs: region commands, text-property scans, fields,
//! indentation helpers, and encoding on regions.

use crate::buffer::primitives::{check_writable, cur, err_sym, pos_idx};
use crate::lisp::Interp;
use crate::lisp::builtins::{S, arg, want_int, want_string};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "current-indentation",
        0,
        1,
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
        "reverse-region",
        2,
        2,
        f_reverse_region,
        "Reverse line order in region."
    ),
    S!(
        "how-many",
        2,
        4,
        f_how_many,
        "Count regexp matches in region."
    ),
    S!(
        "count-matches",
        2,
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
        "transpose-regions",
        4,
        5,
        f_transpose_regions,
        "Transpose two regions."
    ),
    S!(
        "subst-char-in-region",
        4,
        4,
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
        f_buffer_mod_tick,
        "Modification counter."
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
        2,
        2,
        f_nil2,
        "Adjust match data (no-op)."
    ),
    S!(
        "text-property-any",
        4,
        5,
        f_text_property_any,
        "First pos in region where PROP is VALUE."
    ),
    S!(
        "text-property-not-all",
        4,
        5,
        f_text_property_not_all,
        "First pos where PROP differs from VALUE."
    ),
    S!(
        "field-beginning",
        0,
        3,
        f_field_beginning,
        "Start of field at POS."
    ),
    S!("field-end", 0, 3, f_field_end, "End of field at POS."),
    S!("field-string", 0, 2, f_field_string, "Field text at POS."),
    S!(
        "field-string-no-properties",
        0,
        2,
        f_field_string,
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
        "get-pos-property",
        2,
        3,
        f_get_pos_property,
        "Property at POS (0-based-ish)."
    ),
    S!(
        "get-char-property-and-overlay",
        3,
        3,
        f_get_char_prop_and_overlay,
        "Prop + overlay at POS."
    ),
    S!(
        "put-char-property",
        4,
        5,
        f_put_char_property,
        "put-text-property (same)."
    ),
    S!(
        "remove-list-of-text-properties",
        3,
        4,
        f_remove_list_of_props,
        "Remove named props in region."
    ),
    S!(
        "add-face-text-property",
        3,
        5,
        f_add_face_text_property,
        "Add face property to region."
    ),
    S!(
        "next-char-property-change",
        1,
        3,
        f_next_prop_change_fwd,
        "Next position where any prop changes."
    ),
    S!(
        "previous-char-property-change",
        1,
        3,
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
        6,
        7,
        f_compute_motion,
        "Compute motion to TO (simplified)."
    ),
    S!(
        "vertical-motion",
        1,
        4,
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
        3,
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
        3,
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
    S!(
        "secure-hash-region",
        3,
        3,
        f_secure_hash_region,
        "Hash region bytes."
    ),
    S!(
        "undo-boundary",
        0,
        0,
        f_undo_boundary,
        "Push undo boundary."
    ),
];

fn f_nil2(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_one(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Int(1))
}

fn beg_end(
    i: &mut Interp,
    a: &[Value],
    b_idx: usize,
    e_idx: usize,
) -> Result<(usize, usize), Flow> {
    let len = cur(i).borrow().text_len();
    let s = a
        .get(b_idx)
        .and_then(|v| v.int().or_else(|| marker_pos(v)))
        .map(|p| pos_idx(len, p))
        .unwrap_or_else(|| cur(i).borrow().begv);
    let e = a
        .get(e_idx)
        .and_then(|v| v.int().or_else(|| marker_pos(v)))
        .map(|p| pos_idx(len, p))
        .unwrap_or_else(|| cur(i).borrow().zv.max(cur(i).borrow().text_len()));
    Ok((s.min(e), s.max(e)))
}

fn marker_pos(v: &Value) -> Option<i128> {
    if let Value::Marker(m) = v {
        Some(m.borrow().position as i128 + 1)
    } else {
        None
    }
}

fn f_current_indentation(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
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

fn f_sort_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let reverse = arg(&a, 0).truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let mut lines: Vec<&str> = region.split('\n').collect();
    // Trailing empty line artifact: if region ends with \n, last elt is "";
    // keep it attached so we preserve the final newline.
    let had_trailing = region.ends_with('\n');
    if had_trailing {
        lines.pop();
    }
    lines.sort();
    if reverse {
        lines.reverse();
    }
    let mut out = lines.join("\n");
    if had_trailing {
        out.push('\n');
    }
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_reverse_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let mut lines: Vec<&str> = region.split('\n').collect();
    let had_trailing = region.ends_with('\n');
    if had_trailing {
        lines.pop();
    }
    lines.reverse();
    let mut out = lines.join("\n");
    if had_trailing {
        out.push('\n');
    }
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_how_many(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pat = want_string(i, &a[0])?;
    let (s, e) = beg_end(i, &a, 1, 2)?;
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let b = cur(i);
    let bb = b.borrow();
    let chars: Vec<char> = bb.text.substring(s, e).chars().collect();
    let mut count = 0i128;
    let mut pos = 0usize;
    while let Some(regs) = crate::lisp::regexp::search_full(&re, &chars, pos) {
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
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let mut out = String::new();
    let mut removed = 0i128;
    let mut start = 0usize;
    for part in region.split_inclusive('\n') {
        let line = part.strip_suffix('\n').unwrap_or(part);
        let chars: Vec<char> = line.chars().collect();
        let hit = crate::lisp::regexp::search_full(&re, &chars, 0).is_some();
        let keep = if keep_match { hit } else { !hit };
        if keep {
            out.push_str(part);
        } else {
            removed += 1;
        }
        start += part.len();
    }
    let _ = start;
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Int(removed))
}

fn f_flush_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    delete_lines_matching(i, &a, false)
}

fn f_keep_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    delete_lines_matching(i, &a, true)
}

fn f_replace_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let from = want_string(i, &a[0])?;
    let to = want_string(i, &a[1])?;
    if from.is_empty() {
        return Ok(Value::Nil);
    }
    let (s, e) = beg_end(i, &a, 2, 3)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let out = region.replace(&from, &to);
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_replace_regexp(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let pat = want_string(i, &a[0])?;
    let to = want_string(i, &a[1])?;
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    let (s, e) = beg_end(i, &a, 2, 3)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region: Vec<char> = bb.text.substring(s, e).chars().collect();
    let mut out = String::new();
    let mut pos = 0usize;
    let mut n = 0usize;
    while let Some(regs) = crate::lisp::regexp::search_full(&re, &region, pos) {
        let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
        out.extend(&region[pos..ms]);
        // Expand \1..\9 and \& in the replacement.
        let mut tc = to.chars().peekable();
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
                    Some('n') => out.push('\n'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else if c == '&' && false {
                // bare & only in old-style replace-string
            } else {
                out.push(c);
            }
        }
        pos = if me > ms { me } else { me + 1 };
        n += 1;
        if pos > region.len() || n > 1_000_000 {
            break;
        }
    }
    out.extend(&region[pos.min(region.len())..]);
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_transpose_regions(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s1, e1) = beg_end(i, &a, 0, 1)?;
    let (s2, e2) = beg_end(i, &a, 2, 3)?;
    if s1 > s2 {
        return f_transpose_regions(
            i,
            vec![a[2].clone(), a[3].clone(), a[0].clone(), a[1].clone()],
        );
    }
    let b = cur(i);
    let mut bb = b.borrow_mut();
    if e1 > s2 {
        return Err(err_sym(i, "error", vec![Value::string("Regions overlap")]));
    }
    let t1 = bb.text.substring(s1, e1);
    let t2 = bb.text.substring(s2, e2);
    bb.delete_region(s2, e2);
    bb.insert_at(s2, &t1);
    bb.delete_region(s1, e1);
    bb.insert_at(s1, &t2);
    Ok(Value::Nil)
}

fn f_subst_char_in_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let from = want_int(i, &a[2])? as u32;
    let to = want_int(i, &a[3])? as u32;
    let fc = char::from_u32(from).unwrap_or('\u{FFFD}');
    let tc = char::from_u32(to).unwrap_or('\u{FFFD}');
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let out: String = region
        .chars()
        .map(|c| if c == fc { tc } else { c })
        .collect();
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_translate_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    // TABLE: string of 256 chars mapping byte->char, or char-table.
    // Simple version: treat a string as a lookup table indexed by char.
    let table: Vec<char> = match &a[2] {
        Value::Str(t) => t.borrow().chars().collect(),
        _ => Vec::new(),
    };
    if table.is_empty() {
        return Ok(Value::Nil);
    }
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let region = bb.text.substring(s, e);
    let out: String = region
        .chars()
        .map(|c| table.get(c as usize).copied().unwrap_or(c))
        .collect();
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_buffer_swap_text(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let other = match &a[0] {
        Value::Buffer(b) => b.clone(),
        Value::Str(s) => {
            let name = s.borrow().clone();
            let id = i.buffers.by_name(&name).ok_or_else(|| {
                err_sym(
                    i,
                    "error",
                    vec![Value::string(format!("No buffer {}", name))],
                )
            })?;
            i.buffers.get(id).unwrap()
        }
        other => return Err(i.wrong_type_mut("bufferp", other)),
    };
    let cur_b = cur(i);
    let (mut mine, mut theirs) = (cur_b.borrow_mut(), other.borrow_mut());
    std::mem::swap(&mut mine.text, &mut theirs.text);
    std::mem::swap(&mut mine.begv, &mut theirs.begv);
    std::mem::swap(&mut mine.zv, &mut theirs.zv);
    mine.point = mine.point.min(mine.text_len());
    theirs.point = theirs.point.min(theirs.text_len());
    Ok(Value::Nil)
}

fn f_buffer_last_name(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = a;
    let _ = i;
    Ok(Value::Nil)
}

fn f_buffer_mod_tick(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    Ok(Value::Int(b.borrow().mod_tick as i128))
}

fn f_store_match_data(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Same as set-match-data plus reseat markers (we use plain ints).
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
    i.match_data = Some(crate::lisp::MatchData {
        regs,
        in_buffer,
        base,
    });
    Ok(Value::Nil)
}

// ---------- text-property scans ----------

fn f_text_property_any(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prop = crate::buffer::primitives::want_sym(i, &a[2])?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let want = &a[3];
    let b = cur(i);
    let bb = b.borrow();
    for p in s..e {
        let v = prop_at_pos(&bb, p, prop);
        if crate::lisp::builtins::eq_values(&v, want) {
            return Ok(Value::Int(p as i128 + 1));
        }
    }
    Ok(Value::Nil)
}

fn f_text_property_not_all(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let prop = crate::buffer::primitives::want_sym(i, &a[2])?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let want = &a[3];
    let b = cur(i);
    let bb = b.borrow();
    for p in s..e {
        let v = prop_at_pos(&bb, p, prop);
        if !crate::lisp::builtins::eq_values(&v, want) {
            return Ok(Value::Int(p as i128 + 1));
        }
    }
    Ok(Value::Nil)
}

fn prop_at_pos(bb: &crate::buffer::Buffer, pos: usize, prop: u32) -> Value {
    for tp in bb.text_props.iter().rev() {
        if tp.prop == prop && pos >= tp.start && pos < tp.end {
            return tp.value.clone();
        }
    }
    Value::Nil
}

fn field_bounds(i: &mut Interp, a: &[Value], at_start: bool) -> Result<usize, Flow> {
    // `field` text property delimits fields; default whole buffer.
    let pos = match a.get(0) {
        Some(v) if !v.is_nil() => {
            let len = cur(i).borrow().text_len();
            pos_idx(len, v.int().unwrap_or(1))
        }
        _ => cur(i).borrow().point(),
    };
    let fid = i.intern("field");
    let b = cur(i);
    let bb = b.borrow();
    let field = prop_at_pos(&bb, pos, fid);
    if field.is_nil() {
        return Ok(if at_start { bb.begv } else { bb.text_len() });
    }
    // Walk to the boundary where the field value changes.
    let mut p = pos;
    if at_start {
        while p > bb.begv {
            let v = prop_at_pos(&bb, p - 1, fid);
            if !crate::lisp::builtins::eq_values(&v, &field) {
                break;
            }
            p -= 1;
        }
        Ok(p)
    } else {
        let len = bb.text_len();
        while p < len {
            let v = prop_at_pos(&bb, p, fid);
            if !crate::lisp::builtins::eq_values(&v, &field) {
                break;
            }
            p += 1;
        }
        Ok(p)
    }
}

fn f_field_beginning(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = field_bounds(i, &a, true)?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_field_end(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let p = field_bounds(i, &a, false)?;
    Ok(Value::Int(p as i128 + 1))
}

fn f_field_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = field_bounds(i, &a, true)?;
    let e = field_bounds(i, &a, false)?;
    let b = cur(i);
    let bb = b.borrow();
    Ok(Value::string(bb.text.substring(s, e)))
}

fn f_delete_field(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let s = field_bounds(i, &a, true)?;
    let e = field_bounds(i, &a, false)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.delete_region(s, e);
    Ok(Value::Nil)
}

fn f_constrain_to_field(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (constrain-to-field NEW-POS OLD-POS ...) → clamp NEW-POS into the
    // field containing OLD-POS.
    let len = cur(i).borrow().text_len();
    let new_pos = pos_idx(len, a[0].int().unwrap_or(1));
    let fid = i.intern("field");
    let (field_at_new, bounds) = {
        let b = cur(i);
        let bb = b.borrow();
        (prop_at_pos(&bb, new_pos, fid), (bb.begv, bb.text_len()))
    };
    if field_at_new.is_nil() {
        return Ok(Value::Int(new_pos as i128 + 1));
    }
    let _ = bounds;
    let old_field = prop_at_pos(&cur(i).borrow(), pos_idx(len, a[1].int().unwrap_or(1)), fid);
    if crate::lisp::builtins::eq_values(&field_at_new, &old_field) {
        return Ok(Value::Int(new_pos as i128 + 1));
    }
    // Move to nearest boundary of the old field.
    let s = field_bounds(i, &a[1..], true)?;
    let e = field_bounds(i, &a[1..], false)?;
    let clamped = new_pos.clamp(s, e);
    Ok(Value::Int(clamped as i128 + 1))
}

fn f_get_pos_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (get-pos-property POS PROP) — PROP at POS.
    let prop = crate::buffer::primitives::want_sym(i, &a[1])?;
    let len = cur(i).borrow().text_len();
    let pos = pos_idx(len, a[0].int().unwrap_or(1));
    Ok(prop_at_pos(&cur(i).borrow(), pos, prop))
}

fn f_get_char_prop_and_overlay(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Return (PROPVAL . OVERLAY) — we merge overlay plists.
    let prop = crate::buffer::primitives::want_sym(i, &a[1])?;
    let len = cur(i).borrow().text_len();
    let pos = pos_idx(len, a[0].int().unwrap_or(1));
    let b = cur(i);
    let bb = b.borrow();
    // Check overlays first (higher precedence).
    for ov in &bb.overlays {
        if pos >= ov.start && pos < ov.end {
            let v = crate::lisp::eval::plist_get(&ov.plist, prop);
            if !v.is_nil() {
                return Ok(Value::cons(v, Value::Nil));
            }
        }
    }
    let v = prop_at_pos(&bb, pos, prop);
    Ok(Value::cons(v, Value::Nil))
}

fn f_put_char_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[0].clone(), a[0].clone(), a[1].clone(), a[2].clone()],
    )
}

fn f_remove_list_of_props(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text_len();
    let s = pos_idx(len, a[0].int().unwrap_or(1));
    let e = pos_idx(len, a[1].int().unwrap_or(1));
    let names: Vec<u32> = a[2]
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| i.sym_id(v))
        .collect();
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let before = bb.text_props.len();
    bb.text_props
        .retain(|tp| !(names.contains(&tp.prop) && tp.start < e && tp.end > s));
    Ok(Value::from_bool(bb.text_props.len() != before))
}

fn f_add_face_text_property(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let fid = i.intern("face");
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[0].clone(), a[1].clone(), Value::Sym(fid), a[2].clone()],
    )
}

fn f_next_prop_change_fwd(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // Like next-property-change but also considers overlays.
    let r = crate::buffer::primitives::f_next_property_change(i, a.clone());
    // Check overlay boundaries too.
    if let Ok(v) = &r {
        let len = cur(i).borrow().text_len();
        let pos = pos_idx(len, a[0].int().unwrap_or(1));
        let b = cur(i);
        let bb = b.borrow();
        let mut next = v.int().map(|x| x as usize - 1);
        for ov in &bb.overlays {
            for cand in [ov.start, ov.end] {
                if cand > pos && next.map(|n| cand < n).unwrap_or(true) {
                    next = Some(cand);
                }
            }
        }
        if let Some(n) = next {
            return Ok(Value::Int(n as i128 + 1));
        }
    }
    r
}

fn f_prev_prop_change(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let len = cur(i).borrow().text_len();
    let pos = pos_idx(len, a[0].int().unwrap_or(1));
    let b = cur(i);
    let bb = b.borrow();
    let mut prev: Option<usize> = None;
    for tp in &bb.text_props {
        for cand in [tp.start, tp.end] {
            if cand < pos && prev.map(|n| cand > n).unwrap_or(true) {
                prev = Some(cand);
            }
        }
    }
    for ov in &bb.overlays {
        for cand in [ov.start, ov.end] {
            if cand < pos && prev.map(|n| cand > n).unwrap_or(true) {
                prev = Some(cand);
            }
        }
    }
    match prev {
        Some(p) => Ok(Value::Int(p as i128 + 1)),
        None => Ok(arg(&a, 1)),
    }
}

fn f_marker_last_position(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let _ = i;
    match &a[0] {
        Value::Marker(m) => Ok(Value::Int(m.borrow().position as i128 + 1)),
        other => Ok(other.clone()),
    }
}

// ---------- motion ----------

fn f_compute_motion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (compute-motion FROM FROMVCONS TO TOVCONS WIDTH OFFSETS WINDOW)
    // → (TO HPOS VPOS PREVHPOS CONTIN). Simplified: column counting.
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
                col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) as i128;
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

fn f_vertical_motion(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    // (vertical-motion LINES &optional WINDOW CUR-COL) — move down LINES,
    // try to keep column. Returns lines moved.
    let lines = a[0].int().unwrap_or(0);
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let len = bb.text_len();
    let mut p = bb.point();
    // Current column.
    let ls = line_start(&bb.text, p);
    let goal_col = p - ls;
    let mut moved = 0i128;
    if lines >= 0 {
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
            if p == 0 {
                break;
            }
            let ls2 = line_start(&bb.text, p.saturating_sub(1));
            p = ls2;
            moved += 1;
        }
    }
    // Move to goal column on the landed line.
    let ls = line_start(&bb.text, p);
    let le = line_end(&bb.text, p);
    bb.set_point((ls + goal_col).min(le));
    Ok(Value::Int(moved))
}

fn line_start(text: &crate::buffer::GapBuffer, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p > 0 && text.char_at(p - 1) != '\n' {
        p -= 1;
    }
    p
}

fn line_end(text: &crate::buffer::GapBuffer, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p < text.len() && text.char_at(p) != '\n' {
        p += 1;
    }
    p
}

fn f_win_line_height(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let _ = i;
    Ok(Value::Int(1))
}

fn f_backward_prefix_chars(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    // Skip back over chars with expression-prefix syntax (' ` , #).
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let mut p = bb.point();
    while p > 0 {
        match bb.text.char_at(p - 1) {
            '\'' | '`' | ',' | '#' => p -= 1,
            _ => break,
        }
    }
    bb.set_point(p);
    Ok(Value::Nil)
}

fn f_move_point_visually(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    f_vertical_motion(i, a).map(|_| Value::Nil)
}

// ---------- region encode/decode ----------

fn region_text(i: &Interp, a: &[Value]) -> Result<(usize, usize, String), Flow> {
    let b = cur(i);
    let bb = b.borrow();
    let len = bb.text_len();
    let s = a
        .get(0)
        .and_then(|v| v.int())
        .map(|p| pos_idx(len, p))
        .unwrap_or(bb.begv);
    let e = a
        .get(1)
        .and_then(|v| v.int())
        .map(|p| pos_idx(len, p))
        .unwrap_or_else(|| bb.zv.max(len));
    Ok((s, e, bb.text.substring(s, e)))
}

fn f_b64_encode_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let (_s, _e, text) = region_text(i, &a)?;
    let no_break = arg(&a, 2).truthy();
    let eng = if no_break {
        base64::engine::general_purpose::STANDARD_NO_PAD
    } else {
        base64::engine::general_purpose::STANDARD
    };
    let encoded = eng.encode(text.as_bytes());
    // Emacs replaces the region contents.
    check_writable(i)?;
    let (s, e, _) = region_text(i, &a)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.delete_region(s, e);
    bb.insert_at(s, &encoded);
    Ok(Value::Nil)
}

fn f_b64url_encode_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let (s, e, text) = region_text(i, &a)?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(text.as_bytes());
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.delete_region(s, e);
    bb.insert_at(s, &encoded);
    Ok(Value::Nil)
}

fn f_b64_decode_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let (s, e, text) = region_text(i, &a)?;
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|_| err_sym(i, "error", vec![Value::string("Invalid base64 data")]))?;
    let decoded = String::from_utf8_lossy(&bytes).to_string();
    check_writable(i)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.delete_region(s, e);
    bb.insert_at(s, &decoded);
    Ok(Value::Nil)
}

fn f_secure_hash_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let (_s, _e, text) = region_text(i, &a)?;
    let algo = i.symbol_name(i.sym_id(&a[0]).unwrap_or(0));
    use sha2::Digest;
    let out = match algo.as_str() {
        "md5" => md5::Md5::digest(text.as_bytes()).to_vec(),
        "sha1" => sha1::Sha1::digest(text.as_bytes()).to_vec(),
        "sha224" => sha2::Sha224::digest(text.as_bytes()).to_vec(),
        "sha256" => sha2::Sha256::digest(text.as_bytes()).to_vec(),
        "sha384" => sha2::Sha384::digest(text.as_bytes()).to_vec(),
        "sha512" => sha2::Sha512::digest(text.as_bytes()).to_vec(),
        _ => {
            return Err(err_sym(
                i,
                "error",
                vec![Value::string(format!("Unknown algorithm {}", algo))],
            ));
        }
    };
    Ok(Value::string(
        out.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
    ))
}

fn f_undo_boundary(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    bb.undo.push(crate::buffer::UndoEntry::Boundary);
    Ok(Value::Nil)
}
