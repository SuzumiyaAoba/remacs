//! Printing subrs: prin1, princ, print, terpri, prin1-to-string, with-output-to-string.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "prin1",
        1,
        3,
        f_prin1,
        "Print OBJECT readably to standard-output."
    ),
    S!(
        "princ",
        1,
        2,
        f_princ,
        "Print OBJECT human-readably to standard-output."
    ),
    S!(
        "print",
        1,
        2,
        f_print,
        "Print OBJECT readably, preceded by newline and space."
    ),
    S!("terpri", 0, 2, f_terpri, "Output a newline."),
    S!("write-char", 1, 2, f_write_char, "Output CHARACTER."),
    S!(
        "prin1-to-string",
        1,
        3,
        f_prin1_to_string,
        "Return printed representation of OBJECT."
    ),
    // `princ-to-string' was removed in GNU Emacs 31 (obsoleted in 29).
    S!(
        "with-output-to-string",
        raw,
        f_with_output_to_string,
        "Capture output as string."
    ),
    // print-escape-newlines / print-gensym / print-quoted are
    // VARIABLES in Emacs, not functions — defined in eval.rs.
    S!(
        "format-proper-list-p",
        1,
        1,
        f_format_proper_list_p,
        "Length of OBJECT if a proper list, nil otherwise."
    ),
    S!("output-switches", many 0, f_noop, ""),
];

/// GNU fns.c `format-proper-list-p': the `format' arg validator —
/// length of a proper list, nil otherwise (no error on non-lists,
/// dotted or circular lists).
fn f_format_proper_list_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut cur = a[0].clone();
    let mut n: i128 = 0;
    let mut guard = 0usize;
    loop {
        match &cur {
            Value::Cons(c) => {
                guard += 1;
                if guard > 500_000 {
                    return Ok(Value::Nil);
                }
                let next = c.borrow().cdr.clone();
                cur = next;
                n += 1;
            }
            Value::Nil => return Ok(Value::Int(n)),
            _ => return Ok(Value::Nil),
        }
    }
}

fn f_noop(_i: &mut Interp, _args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::Nil)
}

// GNU print.c print_create_variable_mapping. The historical spelling is
// an accepted alias, including its duplicate binding when defaults are used.
const PRINT_SETTINGS: [(&str, &str, bool); 17] = [
    ("length", "print-length", false),
    ("level", "print-level", false),
    ("circle", "print-circle", false),
    ("quoted", "print-quoted", true),
    ("escape-newlines", "print-escape-newlines", false),
    (
        "escape-control-characters",
        "print-escape-control-characters",
        false,
    ),
    ("escape-nonascii", "print-escape-nonascii", false),
    ("escape-multibyte", "print-escape-multibyte", false),
    (
        "charset-text-property",
        "print-charset-text-property",
        false,
    ),
    ("unreadable-function", "print-unreadable-function", false),
    ("unreadeable-function", "print-unreadable-function", false),
    ("gensym", "print-gensym", false),
    ("continuous-numbering", "print-continuous-numbering", false),
    ("number-table", "print-number-table", false),
    ("float-format", "float-output-format", false),
    (
        "integers-as-characters",
        "print-integers-as-characters",
        false,
    ),
    ("symbols-bare", "print-symbols-bare", false),
];

fn with_print_overrides(
    i: &mut Interp,
    overrides: &Value,
    string_output: bool,
    body: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    let mark = i.specbind_depth();
    let result = (|| {
        if string_output {
            let hooks = i.q("inhibit-modification-hooks");
            i.specbind(hooks, Value::t())?;
        }
        if !overrides.is_nil() {
            // GNU creates the complete mapping even for an invalid override.
            let mapping: Vec<_> = PRINT_SETTINGS
                .iter()
                .map(|(key, variable, default)| (i.intern(key), i.intern(variable), *default))
                .collect();
            let bind_defaults = |i: &mut Interp| -> Result<(), crate::lisp::error::Flow> {
                for &(_, variable, default) in &mapping {
                    i.specbind(variable, Value::from_bool(default))?;
                }
                Ok(())
            };
            if i.sym_is(overrides, sym::T) {
                bind_defaults(i)?;
            } else {
                let mut remaining = overrides.clone();
                let mut visited = std::collections::HashSet::new();
                while !remaining.is_nil() {
                    let Value::Cons(cell) = &remaining else {
                        return Err(i.wrong_type_mut("consp", &remaining));
                    };
                    if !visited.insert(std::rc::Rc::as_ptr(cell) as usize) {
                        return Err(i.signal_data(sym::CIRCULAR_LIST, vec![remaining.clone()]));
                    }
                    let setting = cell.borrow().car.clone();
                    if i.sym_is(&setting, sym::T) {
                        bind_defaults(i)?;
                    } else if let Value::Cons(pair) = setting {
                        let (key, value) = {
                            let pair = pair.borrow();
                            (pair.car.clone(), pair.cdr.clone())
                        };
                        let variable = mapping
                            .iter()
                            .find(|(key_id, _, _)| super::eq_values(&key, &Value::Sym(*key_id)))
                            .map(|(_, variable, _)| *variable);
                        let Some(variable) = variable else {
                            return Err(i.wrong_type_mut("symbolp", &Value::Nil));
                        };
                        i.specbind(variable, value)?;
                    } else {
                        return Err(i.wrong_type_mut("consp", &setting));
                    }
                    // A variable watcher can change the remaining alist.
                    let next = cell.borrow().cdr.clone();
                    if !next.is_nil() && !matches!(next, Value::Cons(_)) {
                        return Err(i.wrong_type_mut("consp", &remaining));
                    }
                    remaining = next;
                }
            }
        }
        body(i)
    })();
    i.unbind_to(mark)?;
    result
}

fn f_prin1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU captures a nil destination's standard-output value before
    // override watchers run, but validates that destination afterwards.
    let output = arg(&args, 1);
    let output = if output.is_nil() {
        let standard = i.q("standard-output");
        let value = i.symbol_value(standard);
        if value.is_nil() { Value::t() } else { value }
    } else {
        output
    };
    with_print_overrides(i, &arg(&args, 2), false, |i| {
        let stream = i.checked_output_stream(&output)?;
        let s = i.print_to_string_cb(&args[0])?;
        i.write_output_to(&s, &stream)?;
        Ok(args[0].clone())
    })
}

fn f_princ(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let stream = i.checked_output_stream(&arg(&args, 1))?;
    let s = i.princ_to_string_cb(&args[0])?;
    i.write_output_to(&s, &stream)?;
    Ok(args[0].clone())
}

fn f_print(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let stream = i.checked_output_stream(&arg(&args, 1))?;
    let s = i.print_to_string_cb(&args[0])?;
    // GNU's print emits a newline before and after the object.
    i.write_output_to(&format!("\n{}\n", s), &stream)?;
    Ok(args[0].clone())
}

fn f_terpri(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU print_prepare validates the stream before the ENSURE/BOL
    // shortcut, including when no newline would be emitted.
    let stream = i.checked_output_stream(&arg(&args, 0))?;
    let ensure = arg(&args, 1).truthy();
    if ensure && i.output_at_bol(&stream) {
        return Ok(Value::Nil);
    }
    match &stream {
        Value::Nil => {}
        Value::Sym(sid) if *sid == sym::T => {}
        Value::Marker(_) | Value::Buffer(_) => {}
        _ => {
            return i
                .apply(&stream, vec![Value::Int('\n' as i128)])
                .map(|_| Value::Sym(sym::T));
        }
    }
    i.write_output_to("\n", &stream)?;
    Ok(Value::Sym(sym::T))
}

fn f_write_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fwrite_char checks fixnump, then printchar narrows to unsigned
    // int. Function streams receive that value before CHAR_STRING folds
    // modifiers or rejects invalid codes for text destinations.
    let n = match &args[0] {
        Value::Int(n) if args[0].fixnump() => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    let stream = i.checked_output_stream(&arg(&args, 1))?;
    let code = n as u32;
    if !matches!(&stream, Value::Nil | Value::Buffer(_) | Value::Marker(_))
        && !i.sym_is(&stream, sym::T)
    {
        i.apply(&stream, vec![Value::Int(code as i128)])?;
        return Ok(args[0].clone());
    }
    let code = write_char_text_code(code);
    if code > 0x3FFFFF {
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Invalid character: {code:x}"))],
        ));
    }
    let mut output = crate::lisp::print_text::PrintText::new();
    if (0x3fff80..=0x3fffff).contains(&code) {
        output.push_written_byte((code - 0x3fff00) as u8);
    } else {
        output.push(crate::lisp::value::lisp_char(code).unwrap());
    }
    i.write_output_to(&output, &stream)?;
    Ok(args[0].clone())
}

/// GNU char_string resolves ASCII Shift/Control and discards the six
/// event modifier bits. Other high bits remain invalid character bits.
fn write_char_text_code(mut code: u32) -> u32 {
    const MODIFIERS: u32 = 0x0FC0_0000;
    const SHIFT: u32 = 0x0200_0000;
    const CONTROL: u32 = 0x0400_0000;
    if code & !MODIFIERS <= 0x7F {
        if code & SHIFT != 0 && (b'a' as u32..=b'z' as u32).contains(&(code & 0x7F)) {
            code -= (b'a' - b'A') as u32;
        }
        if code & CONTROL != 0 {
            let base = code & 0x7F;
            let resolved = match base {
                0x20 => 0,
                0x3F => 0x7F,
                0x40..=0x5F | 0x61..=0x7A => base & 0x1F,
                _ => base,
            };
            code = (code & !0x7F) | resolved;
        }
    }
    code & !MODIFIERS
}

fn f_prin1_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    with_print_overrides(i, &arg(&args, 2), true, |i| {
        let s = if arg(&args, 1).truthy() {
            i.princ_to_string_cb(&args[0])?
        } else {
            i.print_to_string_cb(&args[0])?
        };
        // Text sinks expose byte8 sentinels as GNU's escaped ASCII text.
        Ok(Value::string(Interp::sink_text(&s).as_ref()))
    })
}

fn f_with_output_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (with-output-to-string &rest BODY)
    // GNU's macro binds `standard-output' to a temp BUFFER — code inside
    // may `(with-current-buffer standard-output ...)' (vc-git relies on
    // it), so a bare string capture is wrong.
    let body = Value::list(args.iter().cloned().collect());
    let buf_id = i.create_buffer_exact(" *string-output*");
    let buf_val = i
        .buffers
        .get(buf_id)
        .map(Value::Buffer)
        .unwrap_or(Value::Nil);
    let so = i.standard_output_sym;
    i.specbind(so, buf_val)?;
    let r = i.eval_progn(&body);
    let _ = i.unbind(1);
    let text = i
        .buffers
        .get(buf_id)
        .map(|b| {
            let bb = b.borrow();
            bb.text.substring(0, bb.text.len())
        })
        .unwrap_or_default();
    let _ = i.buffers.kill(buf_id, |_| false);
    match r {
        Ok(_) => Ok(Value::string(text)),
        Err(f) => Err(f),
    }
}
