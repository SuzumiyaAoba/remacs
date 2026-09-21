//! Shared test harness: evaluate elisp in a fresh `Interp` and compare
//! against expected printed output.
#![allow(dead_code)]

use remacs::lisp::{Flow, Interp, OutputSink, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// A fresh interpreter with captured output.
pub fn interp() -> (Interp, Rc<RefCell<String>>) {
    let mut i = Interp::new();
    let out = Rc::new(RefCell::new(String::new()));
    i.output = Some(OutputSink::Buffer(out.clone()));
    (i, out)
}

/// Evaluate `src`; return the `prin1` representation of the last form's value.
/// Panics with a readable message on an uncaught signal.
pub fn ev(src: &str) -> String {
    let (mut i, _) = interp();
    match i.eval_str(src) {
        Ok(v) => i.prin1_to_string(&v),
        Err(f) => panic!("eval failed: {} -> {}", src, flow_str(&mut i, &f)),
    }
}

/// Evaluate `src`; return captured princ/print/message output.
pub fn ev_out(src: &str) -> String {
    let (mut i, out) = interp();
    match i.eval_str(src) {
        Ok(_) => out.borrow().clone(),
        Err(f) => panic!("eval failed: {} -> {}", src, flow_str(&mut i, &f)),
    }
}

/// Evaluate `src` expecting an error; return the error symbol name.
pub fn ev_err(src: &str) -> String {
    let (mut i, _) = interp();
    match i.eval_str(src) {
        Ok(v) => panic!("expected error, got {} for {}", i.prin1_to_string(&v), src),
        Err(Flow::Signal(sym, _, _)) => match &sym {
            Value::Sym(id) => i.symbol_name(*id),
            _ => "non-symbol-signal".into(),
        },
        Err(Flow::Throw(_, _)) => "uncaught-throw".into(),
        Err(Flow::Quit) => "quit".into(),
    }
}

/// Evaluate `src`; return `Ok(prin1)` or `Err(symbol-name)`.
pub fn ev_result(src: &str) -> Result<String, String> {
    let (mut i, _) = interp();
    match i.eval_str(src) {
        Ok(v) => Ok(i.prin1_to_string(&v)),
        Err(Flow::Signal(sym, _, _)) => Err(match &sym {
            Value::Sym(id) => i.symbol_name(*id),
            _ => "non-symbol-signal".into(),
        }),
        Err(_) => Err("nonlocal-exit".into()),
    }
}

fn flow_str(i: &mut Interp, f: &Flow) -> String {
    match f {
        Flow::Signal(sym, data, _) => {
            let name = match sym {
                Value::Sym(id) => i.symbol_name(*id),
                _ => "?".into(),
            };
            format!("signal {} {}", name, i.prin1_to_string(data))
        }
        Flow::Throw(t, v) => format!("throw {} {}", i.prin1_to_string(t), i.prin1_to_string(v)),
        Flow::Quit => "quit".into(),
    }
}
