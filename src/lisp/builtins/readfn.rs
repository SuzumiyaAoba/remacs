//! Reader subrs: read, read-from-string.

use super::S;
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("read", 0, 1, f_read, "Read one Lisp object."),
    S!(
        "read-from-string",
        1,
        3,
        f_read_from_string,
        "Read one object from STRING."
    ),
];

fn f_read(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (read &optional STREAM) — stream may be a string, buffer, marker,
    // function, or nil (stdin/minibuffer — unsupported → error).
    match args.get(0) {
        None | Some(Value::Nil) => Err(i.error("read: stdin not supported")),
        Some(Value::Str(s)) => {
            let src = s.borrow().clone();
            let (v, _) = i.read_from_string(&src, 0)?;
            Ok(v)
        }
        Some(Value::Buffer(b)) => {
            // Read one form starting at point, advance point past it.
            let (src, pos) = {
                let bb = b.borrow();
                (bb.text.text(), bb.point)
            };
            let (v, end) = i.read_from_string(&src, pos)?;
            b.borrow_mut().set_point(end);
            Ok(v)
        }
        Some(Value::Marker(m)) => {
            let (buf, pos) = {
                let mm = m.borrow();
                (mm.buffer, mm.position)
            };
            match buf.and_then(|id| i.buffers.get(id)) {
                Some(b) => {
                    let src = b.borrow().text.text();
                    let (v, end) = i.read_from_string(&src, pos)?;
                    m.borrow_mut().position = end;
                    Ok(v)
                }
                None => Err(i.signal_data(sym::END_OF_FILE, vec![])),
            }
        }
        Some(other) => {
            // A function stream: called with no args to produce a char.
            if matches!(other, Value::Lambda(_) | Value::Subr(_) | Value::Sym(_)) {
                let mut src = String::new();
                loop {
                    let c = i.apply(other, vec![])?;
                    match c {
                        Value::Int(n) => {
                            if let Some(ch) = char::from_u32(n as u32) {
                                src.push(ch);
                            }
                        }
                        _ => break,
                    }
                }
                let (v, _) = i.read_from_string(&src, 0)?;
                Ok(v)
            } else {
                Err(i.wrong_type_mut("streamp", other))
            }
        }
    }
}

fn f_read_from_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let src = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let start = args.get(1).and_then(|v| v.int()).unwrap_or(0).max(0) as usize;
    let (v, end) = i.read_from_string(&src, start)?;
    Ok(Value::cons(v, Value::Int(end as i128)))
}


