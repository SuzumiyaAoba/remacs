//! Reader subrs: read, read-from-string.

use super::S;
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
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

fn f_read(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
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
            // GNU `Fread' uses `locate_syms = false' — plain `read'
            // never produces positioned symbols.
            let (src, pos) = {
                let bb = b.borrow();
                // `read' stops at point-max (respects narrowing).
                (bb.text.substring(0, bb.text_len()), bb.point)
            };
            // GNU read-from-minibuffer/read buffers carry the buffer
            // object in end-of-file's data.
            let (v, end) = match i.read_from_string_buf(&src, pos) {
                Ok(x) => x,
                // GNU's read-from-buffer signals (end-of-file BUFFER):
                // the data is a one-element list holding the buffer.
                Err(Flow::Signal(Value::Sym(s), _, flags)) if s == sym::END_OF_FILE => {
                    return Err(Flow::Signal(
                        Value::Sym(s),
                        Value::cons(Value::Buffer(b.clone()), Value::Nil),
                        flags,
                    ));
                }
                Err(e) => return Err(e),
            };
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
                    let src = {
                        let bb = b.borrow();
                        bb.text.substring(0, bb.text_len())
                    };
                    let (v, end) = i.read_from_string_buf(&src, pos)?;
                    m.borrow_mut().position = end;
                    Ok(v)
                }
                None => Err(i.signal_data(sym::END_OF_FILE, vec![])),
            }
        }
        Some(other) => {
            // GNU Fread: any other STREAM is FUNCALLed to produce
            // chars — non-functions surface `invalid-function'.
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
        }
    }
}

fn f_read_from_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let src = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let chars: Vec<char> = src.chars().collect();
    let len = chars.len() as i128;
    // START/END are character positions; nil END (or omitted) is the
    // string's end. Out-of-range values signal args-out-of-range.
    let start = match args.get(1) {
        None | Some(Value::Nil) => 0,
        Some(Value::Int(n)) => *n,
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    let end = match args.get(2) {
        None | Some(Value::Nil) => len,
        Some(Value::Int(n)) => *n,
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    // Negative START/END count from the end of the string (GNU).
    let mut s = start;
    let mut e = end;
    if s < 0 {
        s += len;
    }
    if e < 0 {
        e += len;
    }
    if s < 0 || s > len || e < s || e > len {
        let eid = i.intern("args-out-of-range");
        return Err(i.signal_data(
            eid,
            vec![
                args[0].clone(),
                args.get(1).cloned().unwrap_or(Value::Int(0)),
                args.get(2).cloned().unwrap_or(Value::Nil),
            ],
        ));
    }
    let (start, end) = (s, e);
    let sub: String = chars[start as usize..end as usize].iter().collect();
    let (v, pos) = i.read_from_string(&sub, 0)?;
    Ok(Value::cons(v, Value::Int(start + pos as i128)))
}
