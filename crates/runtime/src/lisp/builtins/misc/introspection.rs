//! Function argument-list recovery from live definitions and GNU documentation.

use super::documentation::doc_text;

use crate::lisp::{
    Interp,
    error::EvalResult,
    value::{Arity, Value},
};

pub(super) fn f_help_function_arglist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // A (lambda PARAMS . BODY) form: extract PARAMS directly.
    let mut v = i.indirect_function_value(&a[0]);
    if let Value::Cons(_) = v {
        if let Ok(items) = v.list_to_vec() {
            let head_lam = matches!(items.first(), Some(Value::Sym(s))
            if *s == i.q("lambda") || *s == i.q("closure"));
            if head_lam {
                if let Ok(l) = i.lambda_from_form(&v, None) {
                    v = Value::Lambda(std::rc::Rc::new(l)).tracked();
                }
            }
        }
    }
    match v {
        Value::Lambda(l) => {
            let mut v: Vec<Value> = l.required.iter().map(|s| Value::Sym(*s)).collect();
            if !l.optional.is_empty() {
                v.push(Value::Sym(i.q("&optional")));
                for o in &l.optional {
                    v.push(Value::Sym(o.sym));
                }
            }
            if let Some(r) = l.rest {
                v.push(Value::Sym(i.q("&rest")));
                v.push(Value::Sym(r));
            }
            Ok(Value::list(v))
        }
        Value::Subr(s) => {
            // PRESERVE-NAMES: GNU recovers the real arg names from the
            // docstring's `(fn ARGLIST)' trailer, lowercased.
            if a.get(1).map(|v| v.truthy()).unwrap_or(false) {
                let doc = doc_text(i, &format!("F{}", s.name)).unwrap_or_else(|| s.doc.to_string());
                if let Some(list) = doc_arglist(i, &doc) {
                    return Ok(list);
                }
            }
            Ok(match s.arity {
                Arity::Range { min, max } => {
                    let mut v = Vec::new();
                    for k in 0..min {
                        v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                    }
                    if max > min {
                        v.push(Value::Sym(i.q("&optional")));
                        for k in min..max {
                            v.push(Value::Sym(i.intern(&format!("arg{}", k + 1))));
                        }
                    }
                    Value::list(v)
                }
                Arity::Many { min } => {
                    let mut v: Vec<Value> = (0..min)
                        .map(|k| Value::Sym(i.intern(&format!("arg{}", k + 1))))
                        .collect();
                    v.push(Value::Sym(i.q("&rest")));
                    v.push(Value::Sym(i.q("rest")));
                    Value::list(v)
                }
                // Special forms: GNU `func-arity' reports
                // (MIN . unevalled); the arglist is `arg1..argN &rest
                // rest' — e.g. `if' → (arg1 arg2 &rest rest).
                Arity::Unevalled => {
                    let min = crate::lisp::special::special_form_min_args(i.intern(s.name));
                    let mut v: Vec<Value> = (0..min)
                        .map(|k| Value::Sym(i.intern(&format!("arg{}", k + 1))))
                        .collect();
                    v.push(Value::Sym(i.q("&rest")));
                    v.push(Value::Sym(i.q("rest")));
                    Value::list(v)
                }
            })
        }
        _ => Ok(Value::t()),
    }
}

/// Parse the `(fn ARGS...)' trailer of a subr docstring into a list of
/// lowercased arg-name symbols (GNU's PRESERVE-NAMES path).
fn doc_arglist(i: &mut Interp, doc: &str) -> Option<Value> {
    let start = doc.rfind("(fn ")?;
    let inner = &doc[start + 4..];
    let mut depth = 1i32;
    let mut end = inner.len();
    for (off, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = off;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let names: Vec<Value> = inner[..end]
        .split_whitespace()
        .map(|t| Value::Sym(i.intern(&t.to_lowercase())))
        .collect();
    Some(Value::list(names))
}
