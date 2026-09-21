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
        2,
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
                    Value::Float(f) => Some(*f),
                    _ => None,
                });
                let y = b.key.int().map(|n| n as f64).or_else(|| match &b.key {
                    Value::Float(f) => Some(*f),
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
    let mut bb = b.borrow_mut();
    let (min, max) = (bb.begv, bb.text_len());
    let mut out = String::new();
    let mut pos = min;
    for (s, o) in sorted.iter().zip(recs.iter()) {
        out.push_str(&bb.text.substring(pos, o.start));
        out.push_str(&bb.text.substring(s.start, s.end));
        pos = o.end;
    }
    out.push_str(&bb.text.substring(pos, max));
    // GNU leaves the last char in place so markers at max survive.
    bb.delete_region(min, max - 1);
    bb.insert_at(min, &out);
    bb.delete_region(max, max + 1);
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

fn f_sort_subr(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_sort_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let reverse = a[0].truthy();
    let (s, e) = beg_end(i, &a, 1, 2)?;
    with_narrow(i, s, e, |i| {
        let mut nr = |i: &mut Interp| line_next(i);
        let mut er = |i: &mut Interp| line_end_move(i);
        sort_engine(i, reverse, Some(&mut nr), Some(&mut er), None, None, None)
    })
}

fn f_sort_paragraphs(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
                if crate::lisp::regexp::looking_at(&re, &text, p - begv).is_none() {
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
                cur(i).borrow_mut().insert("\n");
            }
            Ok(Value::Nil)
        };
        sort_engine(i, reverse, Some(&mut nr), Some(&mut er), None, None, None)
    })
}

fn f_sort_pages(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_sort_fields(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let field = want_int(i, &a[0])?;
    let (s, e) = beg_end(i, &a, 1, 2)?;
    sort_fields_1(i, field, s, e, false)
}

fn f_sort_numeric_fields(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
    match crate::lisp::regexp::search_full(re, &text, pos) {
        Some(regs) if regs[1].unwrap_or(0) <= bound_idx => {
            let e = regs[1].unwrap_or(0);
            i.match_data = Some(crate::lisp::eval::MatchData {
                regs,
                in_buffer: true,
                base: begv,
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

fn f_sort_regexp_fields(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
            c if (c as u32) < 0x20 || c == '\x7f' => col += 2,
            c => {
                col += unicode_width::UnicodeWidthChar::width(c)
                    .unwrap_or(1)
                    .max(1) as i128
            }
        }
        k += 1;
    }
    col
}

/// GNU move-to-column without FORCE: land on first char boundary with
/// col >= goal (chars never split, short lines just stop at EOL).
fn move_to_col(i: &mut Interp, goal: i128) -> EvalResult {
    let b = cur(i);
    let mut bb = b.borrow_mut();
    let tab_width = i
        .symbol_value(i.intern_soft("tab-width").unwrap_or(0))
        .int()
        .unwrap_or(8)
        .max(1);
    let p = bb.point();
    let ls = line_start(&bb.text, p);
    let le = line_end(&bb.text, p).min(bb.text_len());
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
    bb.set_point(k);
    Ok(Value::Int(col))
}

fn f_sort_columns(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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

fn f_reverse_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    let b = cur(i);
    let mut bb = b.borrow_mut();
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
    bb.delete_region(beg, end);
    bb.insert_at(beg, &out);
    Ok(Value::Nil)
}

fn f_delete_duplicate_lines(i: &mut Interp, a: Vec<Value>) -> EvalResult {
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
                cur(i).borrow_mut().delete_region(ls, del_end);
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
        expand_rep(i, &to, &regs, &region, &mut out, false)?;
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
                _ => {
                    return Err(err_sym(
                        i,
                        "error",
                        vec![Value::string("Invalid use of `\\' in replacement text")],
                    ));
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(())
}

fn f_replace_regexp_in_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let pat = want_string(i, &a[0])?;
    let rep = want_string(i, &a[1])?;
    let s = want_string(i, &a[2])?;
    let literal = !arg(&a, 4).is_nil();
    let subexp = match arg(&a, 5) {
        Value::Int(n) => n.max(0) as usize,
        _ => 0,
    };
    let chars: Vec<char> = s.chars().collect();
    let start = match arg(&a, 6) {
        Value::Int(n) => n.max(0) as usize,
        _ => 0,
    };
    if start > chars.len() {
        let e = i.intern("args-out-of-range");
        return Err(i.signal_data(e, vec![Value::string(s.clone()), Value::Int(start as i128)]));
    }
    let case_fold = i
        .symbol_value(i.intern_soft("case-fold-search").unwrap_or(0))
        .truthy();
    let re = crate::lisp::regexp::compile_case(&pat, case_fold)
        .map_err(|e| err_sym(i, "invalid-regexp", vec![Value::string(e.0)]))?;
    // GNU: when START is non-nil the result excludes STRING's prefix.
    let mut out = String::new();
    let mut pos = start;
    let mut n = 0usize;
    while let Some(regs) = crate::lisp::regexp::search_full(&re, &chars, pos) {
        let (ms, me) = (regs[0].unwrap_or(0), regs[1].unwrap_or(0));
        out.extend(&chars[pos..ms]);
        if subexp > 0 {
            // Replace only the SUBEXP group inside the match.
            if let (Some(gs), Some(ge)) = (
                regs.get(2 * subexp).copied().flatten(),
                regs.get(2 * subexp + 1).copied().flatten(),
            ) {
                out.extend(&chars[ms..gs]);
                expand_rep(i, &rep, &regs, &chars, &mut out, literal)?;
                out.extend(&chars[ge..me]);
            } else {
                out.extend(&chars[ms..me]);
            }
        } else {
            expand_rep(i, &rep, &regs, &chars, &mut out, literal)?;
        }
        pos = if me > ms { me } else { me + 1 };
        n += 1;
        if pos > chars.len() || n > 1_000_000 {
            break;
        }
    }
    out.extend(&chars[pos.min(chars.len())..]);
    Ok(Value::string(out))
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
    // GNU substitutes in place: point and markers are undisturbed.
    for p in s..e.min(bb.text.len()) {
        if bb.text.char_at(p) == fc {
            bb.text.set_char_at(p, tc);
        }
    }
    Ok(Value::Nil)
}

fn f_translate_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    check_writable(i)?;
    let (s, e) = beg_end(i, &a, 0, 1)?;
    // TABLE: string of 256 chars mapping byte->char, or char-table.
    // Simple version: treat a string as a lookup table indexed by char.
    let table: Vec<char> = match &a[2] {
        Value::Str(t) => t.borrow().chars().collect(),
        Value::Record(r) => {
            // Char-table (e.g. from make-translation-table-from-alist):
            // aref[c] gives the replacement char.
            let rr = r.borrow();
            match rr.get(2) {
                Some(Value::Vec(v)) => v
                    .borrow()
                    .iter()
                    .map(|x| match x {
                        Value::Int(n) => char::from_u32(*n as u32).unwrap_or('\0'),
                        _ => '\0',
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
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
        .map(|c| {
            table
                .get(c as usize)
                .copied()
                .filter(|t| *t != '\0')
                .unwrap_or(c)
        })
        .collect();
    bb.delete_region(s, e);
    bb.insert_at(s, &out);
    Ok(Value::Nil)
}

fn f_make_translation_table_from_alist(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let items = a[0].list_to_vec().unwrap_or_default();
    let mut vec = vec![Value::Nil; 256];
    for item in items {
        if let Value::Cons(c) = &item {
            let (from, to) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let (Value::Int(f), Value::Int(t)) = (from, to) {
                if (0..256).contains(&f) {
                    vec[f as usize] = Value::Int(t);
                }
            }
        }
    }
    Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
        vec![
            Value::Sym(i.intern("char-table")),
            Value::Sym(i.intern("translation-table")),
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec))),
        ],
    ))))
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
    // Swapping a buffer with itself is a no-op (and avoids a double
    // borrow_mut on the same RefCell).
    if std::rc::Rc::ptr_eq(&cur_b, &other) {
        return Ok(Value::Nil);
    }
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
    // field containing OLD-POS.  GNU: when NEW-POS is nil, use the
    // current point and also move point to the constrained position.
    let len = cur(i).borrow().text_len();
    let (new_pos, move_pt) = match a[0].int().or_else(|| marker_pos(&a[0])) {
        Some(n) => (pos_idx(len, n), false),
        None if a[0].is_nil() => (cur(i).borrow().point(), true),
        None => return Err(i.wrong_type_mut("integer-or-marker-p", &a[0])),
    };
    let fid = i.intern("field");
    let (field_at_new, bounds) = {
        let b = cur(i);
        let bb = b.borrow();
        (prop_at_pos(&bb, new_pos, fid), (bb.begv, bb.text_len()))
    };
    if field_at_new.is_nil() {
        if move_pt {
            cur(i).borrow_mut().set_point(new_pos);
        }
        return Ok(Value::Int(new_pos as i128 + 1));
    }
    let _ = bounds;
    let old_field = prop_at_pos(&cur(i).borrow(), pos_idx(len, a[1].int().unwrap_or(1)), fid);
    let clamped = if crate::lisp::builtins::eq_values(&field_at_new, &old_field) {
        new_pos
    } else {
        // Move to nearest boundary of the old field.
        let s = field_bounds(i, &a[1..], true)?;
        let e = field_bounds(i, &a[1..], false)?;
        new_pos.clamp(s, e)
    };
    if move_pt {
        cur(i).borrow_mut().set_point(clamped);
    }
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
