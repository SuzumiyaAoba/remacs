//! Lisp error-message construction and condition-data formatting.

use super::chartables::CT_MAX_CHAR;
use crate::lisp::error::Flow;

use super::chartables::ct_ref_defalt;

use super::chartables::is_char_table;

use super::threads::want_thread;

use crate::lisp::{Interp, error::EvalResult, obarray::sym, value::Value};

pub(super) fn f_error_message_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
pub fn error_message(i: &mut Interp, obj: &Value) -> crate::lisp::text::EmacsText {
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
        Value::Str(s) if from_prop => Some(
            crate::lisp::builtins::evalfn::translate_message_quotes(i, &s.borrow()),
        ),
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    };
    let mut out = crate::lisp::text::EmacsText::default();
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

pub(super) fn f_documentation_stringp(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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

pub(super) fn f_external_debugging_output(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
    {
        use std::io::Write;
        let character = crate::lisp::text::EmacsChar::new(stripped).unwrap();
        let bytes = character.to_string().to_external_bytes();
        let _ = std::io::stderr().write_all(&bytes);
    }
    Ok(args[0].clone())
}

pub(super) fn f_open_dribble_file(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

pub(super) fn f_open_termscript(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

/// GNU `describe_vector' (keymap.c): iterate VECTOR (a vector or
/// char-table), compress runs sharing a definition, and insert
/// "KEY[.. KEY]" + DESCRIBER output per run.  When BASIC is nil the
/// describer is wrapped like GNU's `describe_vector_princ' (indent-to
/// 16 + terpri).  Char-table runs never straddle the 0x3FFF80
/// boundary and group by `eq'-equal raw/defalt values like GNU's
/// char_table_ref_and_range.
pub(super) fn describe_vector(
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
    let fontify = |i: &mut Interp, k: i128| -> Result<crate::lisp::text::EmacsText, Flow> {
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
            let is_si = matches!(&check_ranges, Value::Sym(s) if *s == i.q("ignore-self-insert"));
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

pub(super) fn f_describe_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
pub(super) fn f_internal_describe_syntax_value(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
    let mut out = crate::lisp::text::EmacsText::default();
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
                    Some(m) => out.push(
                        crate::lisp::value::lisp_char(m)
                            .unwrap_or_else(|| crate::lisp::value::mule_seq_char(m)),
                    ),
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
                    out.push(crate::lisp::text::EmacsChar::new(m).unwrap());
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

/// `help--describe-vector' (keymap.c): specbind `standard-output' to
/// the current buffer, then run `describe_vector' with keymap_p=t —
/// describe-map's lambda is the raw BASIC describer.
pub(super) fn f_help_describe_vector(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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

/// `command-error-default-function' — print "CONTEXT<msg>" for the
/// error data, like GNU's batch error report.  In a noninteractive
/// session this is the toplevel death path: GNU kills the batch job
/// (exit status 255, i.e. kill-emacs -1).
pub(super) fn f_command_error_default(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
            crate::lisp::builtins::evalfn::f_ding(i, vec![Value::t()].into())?;
            let mut msg = context.clone();
            msg.push_str(error_message(i, &a[0]));
            if i.noninteractive {
                i.write_output_to(&msg, &Value::t())?;
            } else {
                i.message(&msg);
            }
            return Ok(Value::Nil);
        }
    }
    let mut msg = context;
    msg.push_str(error_message(i, &a[0]));
    if i.noninteractive {
        eprintln!("{msg}");
        return crate::lisp::builtins::evalfn::f_kill_emacs(i, vec![Value::Int(-1)].into());
    }
    i.message(&msg);
    Ok(Value::Nil)
}

/// `mapbacktrace` — GNU calls FUNCTION with (EVALD FUNC ARGS FLAGS)
/// for each live frame, innermost first, then returns nil.
pub(super) fn f_mapbacktrace(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
                crate::lisp::builtins::eq_values(
                    &i.indirect_function_value(&frame.function),
                    &base_fn,
                )
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

/// `backtrace--locals` — wholenump-checks arg 0; nil (no frame info).
pub(super) fn f_backtrace_locals(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Int(n) if *n >= 0 => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("wholenump", other)),
    }
}

/// `backtrace-eval' — GNU checks NFRAMES (arg 1) with wholenump and
/// THREAD (arg 2) with threadp, then answers nil when the frame is
/// out of range (we keep no suspended frames at all).
pub(super) fn f_backtrace_eval(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
pub(super) fn f_backtrace_debug(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU backtrace.c Fbacktrace_debug: CHECK_FIXNUM then
    // CHECK_WHOLENUMP on LEVEL before debugger dispatch; batch mode
    // no-ops to t.
    match &a[0] {
        Value::Int(n) if *n >= 0 => Ok(Value::from_bool(true)),
        Value::Int(_) => Err(i.wrong_type_mut("wholenump", &a[0])),
        other => Err(i.wrong_type_mut("fixnump", other)),
    }
}

pub(super) fn f_backtrace_frame_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
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
