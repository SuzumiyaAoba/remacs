//! Printing subrs: prin1, princ, print, terpri, prin1-to-string, with-output-to-string.

use super::{S, arg};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
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
    S!("terpri", 0, 1, f_terpri, "Output a newline."),
    S!("write-char", 1, 2, f_write_char, "Output CHARACTER."),
    S!(
        "prin1-to-string",
        1,
        2,
        f_prin1_to_string,
        "Return printed representation of OBJECT."
    ),
    S!(
        "princ-to-string",
        1,
        1,
        f_princ_to_string,
        "Return princ representation of OBJECT."
    ),
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
    i.write_output(&s);
    Ok(args[0].clone())
}

fn f_princ_to_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::string(i.princ_to_string(&args[0])))
}

fn f_princ(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = i.princ_to_string(&args[0]);
    i.write_output(&s);
    Ok(args[0].clone())
}

fn f_print(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = i.print_to_string(&args[0]);
    i.write_output(&format!("\n{} ", s));
    Ok(args[0].clone())
}

fn f_terpri(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let n = match args.get(0) {
        Some(Value::Int(n)) => (*n).max(1),
        _ => 1,
    };
    for _ in 0..n {
        i.write_output("\n");
    }
    Ok(Value::Nil)
}

fn f_write_char(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Value::Int(n) = &args[0] {
        if let Some(c) = char::from_u32(*n as u32) {
            let mut s = [0u8; 4];
            i.write_output(c.encode_utf8(&mut s));
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
