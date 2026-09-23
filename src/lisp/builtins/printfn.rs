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
        2,
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
        2,
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
    S!("output-switches", many 0, f_noop, ""),
];

fn f_noop(_i: &mut Interp, _args: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_prin1(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = i.print_to_string(&args[0]);
    i.write_output_to(&s, &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_princ(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = i.princ_to_string(&args[0]);
    i.write_output_to(&s, &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_print(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = i.print_to_string(&args[0]);
    // GNU's print emits a newline before and after the object.
    i.write_output_to(&format!("\n{}\n", s), &arg(&args, 1))?;
    Ok(args[0].clone())
}

fn f_terpri(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // GNU: (terpri &optional PRINTCHARFUN ENSURE) — PRINTCHARFUN is the
    // print stream; with ENSURE, output only if not already at BOL.
    let stream = arg(&args, 0);
    // GNU's terpri accepts only t/nil/marker/buffer streams; function
    // streams signal `error' ("Unsupported function argument" FN).
    let dest = match &stream {
        Value::Nil => i.symbol_value(i.standard_output_sym),
        v => v.clone(),
    };
    match &dest {
        Value::Nil => {}
        Value::Sym(sid) if *sid == sym::T => {}
        Value::Marker(_) | Value::Buffer(_) => {}
        _ => {
            return Err(i.signal_data(
                sym::ERROR,
                vec![Value::string("Unsupported function argument"), dest],
            ));
        }
    }
    let ensure = arg(&args, 1).truthy();
    if ensure && i.output_at_bol(&stream) {
        return Ok(Value::Nil);
    }
    i.write_output_to("\n", &stream)?;
    Ok(Value::Sym(sym::T))
}

fn f_write_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Value::Int(n) = &args[0] {
        if let Some(c) = char::from_u32(*n as u32) {
            let mut s = [0u8; 4];
            i.write_output_to(c.encode_utf8(&mut s), &arg(&args, 1))?;
        }
    }
    Ok(args[0].clone())
}

fn f_prin1_to_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let noescape = arg(&args, 1).truthy();
    Ok(Value::string(if noescape {
        i.princ_to_string(&args[0])
    } else {
        i.print_to_string(&args[0])
    }))
}

fn f_with_output_to_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (with-output-to-string &rest BODY)
    let body = args.into_iter().next().unwrap_or(Value::Nil);
    let saved = std::mem::take(&mut i.output_buffer);
    i.capture_output = true;
    let r = i.eval_progn(&body);
    i.capture_output = false;
    let captured = std::mem::replace(&mut i.output_buffer, saved);
    r.map(|_| Value::string(captured))
}
