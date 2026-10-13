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

fn f_prin1(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU print.c Fprin1: OVERRIDES is
    // validated (non-nil must be t or a cons) BEFORE the stream is
    // resolved — print_bind_overrides runs inside print_to_buffer/string
    // ahead of the destination lookup.
    let ov = arg(&args, 2);
    if !ov.is_nil() && !i.sym_is(&ov, sym::T) && !matches!(ov, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &ov));
    }
    let s = i.print_to_string_cb(&args[0])?;
    i.write_output_to(&s, &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_princ(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = i.princ_to_string_cb(&args[0])?;
    i.write_output_to(&s, &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_print(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = i.print_to_string_cb(&args[0])?;
    // GNU's print emits a newline before and after the object.
    i.write_output_to(&format!("\n{}\n", s), &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_terpri(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU: (terpri &optional PRINTCHARFUN ENSURE) — with ENSURE non-nil
    // and output already at BOL, return nil WITHOUT ever resolving
    // PRINTCHARFUN (print.c Fterpri: check BOL first).  Both probes
    // use the CURRENT buffer's point for BOL-ness.
    let stream = arg(&args, 0);
    let ensure = arg(&args, 1).truthy();
    if ensure && i.output_at_bol(&stream) {
        return Ok(Value::Nil);
    }
    let dest = match &stream {
        Value::Nil => i.symbol_value(i.standard_output_sym),
        v => v.clone(),
    };
    match &dest {
        Value::Nil => {}
        Value::Sym(sid) if *sid == sym::T => {}
        Value::Marker(_) | Value::Buffer(_) => {}
        _ => {
            return i
                .apply(&dest, vec![Value::Int('\n' as i128)])
                .map(|_| Value::Sym(sym::T));
        }
    }
    i.write_output_to("\n", &stream)?;
    Ok(Value::Sym(sym::T))
}

fn f_write_char(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fwrite_char: CHECK_FIXNUM only — the range check lives in
    // printchar: negatives (and out-of-charset codes) signal
    // (error "Invalid character: %o") with the *unsigned* octal;
    // eight-bit codes write as the byte, big codes as U+FFFD.
    let n = match &args[0] {
        Value::Int(n) => *n,
        other => return Err(i.wrong_type_mut("fixnump", other)),
    };
    if n < 0 {
        // GNU prints the negative code as `f' + 6 hex digits of the
        // 22-bit masked value: -1 → "f03fffff", -5 → "f03ffffb".
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "Invalid character: f{:06x}",
                n & 0x3FFFFF
            ))],
        ));
    }
    // Eight-bit codes emit a sentinel so sinks get GNU's semantics:
    // function → code, text → the eight-bit char, stdout → overlong.
    if let Some(b) = (0x3FFF80..=0x3FFFFF)
        .contains(&n)
        .then(|| (n - 0x3FFF00) as u8)
    {
        let mut buf = [0u8; 4];
        let ch = crate::lisp::value::write_char_eight_bit(b);
        return i
            .write_output_to(ch.encode_utf8(&mut buf), &arg(&args, 1))
            .map(|_| args[0].clone());
    }
    let c = crate::lisp::value::lisp_char(n as u32).unwrap_or('\u{FFFD}');
    let mut s = [0u8; 4];
    i.write_output_to(c.encode_utf8(&mut s), &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_prin1_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU print_bind_overrides: OVERRIDES non-nil must be t or a cons —
    // other atoms signal `consp' on the whole OVERRIDES arg.
    let ov = arg(&args, 2);
    if !ov.is_nil() && !i.sym_is(&ov, sym::T) && !matches!(ov, Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &ov));
    }
    let noescape = arg(&args, 1).truthy();
    let s = if noescape {
        i.princ_to_string_cb(&args[0])?
    } else {
        i.print_to_string_cb(&args[0])?
    };
    // String sinks render eight-bit bytes as `\NNN' ASCII text like GNU's
    // strout — sentinels never reach the Lisp value.
    Ok(Value::string(
        crate::lisp::eval::Interp::sink_text(&s).as_ref(),
    ))
}

fn f_with_output_to_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (with-output-to-string &rest BODY)
    // GNU's macro binds `standard-output' to a temp BUFFER — code inside
    // may `(with-current-buffer standard-output ...)' (vc-git relies on
    // it), so a bare string capture is wrong.
    let body = Value::list(args.iter().cloned().collect());
    let buf_id = i.buffers.create_exact(" *string-output*");
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
