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
                .map(|(key, variable, default)| (i.q(key), i.q(variable), *default))
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
        i.print_to_output_cb(&args[0], &stream, true)?;
        Ok(args[0].clone())
    })
}

fn f_princ(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let stream = i.checked_output_stream(&arg(&args, 1))?;
    i.print_to_output_cb(&args[0], &stream, false)?;
    Ok(args[0].clone())
}

fn f_print(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let stream = i.checked_output_stream(&arg(&args, 1))?;
    // GNU's print emits a newline before and after the object.
    i.write_output_to("\n", &stream)?;
    i.print_to_output_cb(&args[0], &stream, true)?;
    i.write_output_to("\n", &stream)?;
    Ok(args[0].clone())
}

fn check_terpri_function_stream(
    i: &mut Interp,
    stream: &Value,
) -> Result<(), crate::lisp::error::Flow> {
    let Value::Cons(cell) = stream else {
        return Ok(());
    };
    let head = cell
        .try_borrow()
        .map(|cell| cell.car.clone())
        .map_err(|_| i.error("Cannot inspect a mutably borrowed output function"))?;
    if i.sym_is(&head, sym::LAMBDA) {
        // GNU's ENSURE path cannot determine BOL for a raw lambda-list
        // stream and signals before taking the no-output shortcut. Ordinary
        // `princ' to that same stream remains supported.
        let mut data = vec![Value::string("Unsupported function argument")];
        match stream.list_to_vec() {
            Ok(arguments) => data.extend(arguments),
            Err(_) => data.push(stream.clone()),
        }
        return Err(i.signal_data(sym::ERROR, data));
    }
    Ok(())
}

fn f_terpri(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU print_prepare validates the stream before the ENSURE/BOL
    // shortcut, including when no newline would be emitted.
    let stream = i.checked_output_stream(&arg(&args, 0))?;
    let ensure = arg(&args, 1).truthy();
    if ensure {
        check_terpri_function_stream(i, &stream)?;
        if matches!(stream, Value::Cons(_)) {
            // Function-like conses and invalid cons streams have no BOL
            // state. Applying them validates/calls the stream before an
            // ENSURE shortcut can suppress the observable result.
            i.write_output_to("\n", &stream)?;
            return Ok(Value::Sym(sym::T));
        }
        let valid_stream_kind = matches!(
            stream,
            Value::Nil
                | Value::Sym(_)
                | Value::Buffer(_)
                | Value::Marker(_)
                | Value::Lambda(_)
                | Value::Subr(_)
        ) || i.sym_id(&stream).is_some();
        if !valid_stream_kind {
            // Non-callable values are invalid even when the output sink is
            // already at beginning-of-line.  Applying them produces GNU's
            // `invalid-function' datum without writing any output.
            return i
                .apply(&stream, vec![Value::Int('\n' as i128)])
                .map(|_| Value::Sym(sym::T));
        }
    }
    if ensure && i.output_at_bol_checked(&stream)? {
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
    let mark = i.specbind_depth();
    if let Err(error) = i.specbind(so, buf_val) {
        let unwind = i.unbind_to(mark);
        let _ = i.buffers.kill(buf_id, |_| false);
        unwind?;
        return Err(error);
    }
    let r = i.eval_progn(&body);
    let unwind = i.unbind_to(mark);
    let text = i
        .buffers
        .get(buf_id)
        .map(|b| {
            let bb = b.borrow();
            bb.text.substring(0, bb.text.len())
        })
        .unwrap_or_default();
    let _ = i.buffers.kill(buf_id, |_| false);
    unwind?;
    match r {
        Ok(_) => Ok(Value::string(text)),
        Err(f) => Err(f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_overrides_keep_their_key_and_variable_after_unintern() {
        let mut interp = Interp::vacant();
        interp.obarray = crate::lisp::obarray::Obarray::new();
        let key = interp.q("length");
        let variable = interp.q("print-length");
        interp.obarray.symbol_mut(variable).value = Value::Int(10);
        assert!(interp.obarray.unintern_symbol(key));
        assert!(interp.obarray.unintern_symbol(variable));
        let replacement_key = interp.intern("length");
        let replacement_variable = interp.intern("print-length");
        assert_ne!(key, replacement_key);
        assert_ne!(variable, replacement_variable);

        let overrides = Value::list(vec![Value::cons(Value::Sym(key), Value::Int(2))]);
        let result = with_print_overrides(&mut interp, &overrides, false, |interp| {
            Ok(interp.symbol_value(variable))
        })
        .unwrap();

        assert!(matches!(result, Value::Int(2)));
        assert!(matches!(interp.symbol_value(variable), Value::Int(10)));
    }

    #[test]
    fn with_output_to_string_cleans_up_failed_standard_output_binding() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun output-string-watcher (_symbol _value operation _where)\
                           (if (eq operation 'let) (signal 'file-error '(\"bind\")) nil))\
                         (add-variable-watcher 'standard-output #'output-string-watcher)",
                    )
                    .unwrap();
                let mark = interp.specbind_depth();
                let output = interp.symbol_value(interp.standard_output_sym);
                let result = f_with_output_to_string(&mut interp, vec![Value::Nil].into());
                assert!(
                    matches!(result, Err(crate::lisp::Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
                    "unexpected with-output-to-string result: {result:?}"
                );
                assert_eq!(interp.specbind_depth(), mark);
                assert!(crate::lisp::builtins::eq_values(
                    &interp.symbol_value(interp.standard_output_sym),
                    &output
                ));
                assert!(interp.buffers.by_name(" *string-output*").is_none());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
