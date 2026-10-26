//! Function metadata, arity, closure construction and gensym.

use crate::lisp::builtins::arg;
use crate::lisp::builtins::want_sym;
use crate::lisp::{
    Interp,
    error::EvalResult,
    obarray::sym,
    value::{Arity, Lambda, Value},
};
use std::rc::Rc;

fn gensym_literal(i: &mut Interp, key: &str, text: &str) -> Value {
    if let Some(value) = i.native_lisp_literals.get(key) {
        return value.clone();
    }
    let string = Value::string_ref(text);
    i.mark_unibyte(&string);
    let value = Value::Str(string);
    i.native_lisp_literals.insert(key.to_owned(), value.clone());
    value
}

pub(super) fn f_gensym(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // subr.el: update the counter before formatting. GNU's dumped native
    // function reads the old counter once and inlines the 1+ operation.
    let counter = match i.native_lisp_literals.get("gensym/counter") {
        Some(Value::Sym(symbol)) => *symbol,
        _ => {
            let symbol = i.q("gensym-counter");
            i.native_lisp_literals
                .insert("gensym/counter".into(), Value::Sym(symbol));
            symbol
        }
    };
    let number =
        crate::lisp::builtins::call_native_builtin(i, "symbol-value", vec![Value::Sym(counter)])?;
    let next = crate::lisp::builtins::call_canonical_builtin(i, "1+", vec![number.clone()])?;
    i.set_symbol(counter, next)?;
    let prefix = arg(&args, 0);
    let prefix = if prefix.is_nil() {
        gensym_literal(i, "gensym/prefix", "g")
    } else {
        prefix
    };
    let format = gensym_literal(i, "gensym/format", "%s%d");
    let name =
        crate::lisp::builtins::call_native_builtin(i, "format", vec![format, prefix, number])?;
    crate::lisp::builtins::call_native_builtin(i, "make-symbol", vec![name])
}

/// GNU `lisp_func_arity' walk over a raw interpreted arglist:
/// non-symbol elements and a bare `. tail' (not after `&rest') make
/// the arglist malformed → None → invalid-function.
pub(crate) fn raw_arglist_arity(i: &mut Interp, arglist_v: &Value) -> Option<Arity> {
    let opt_sym = i.q("&optional");
    let rest_sym = i.q("&rest");
    let (mut min, mut max, mut many, mut mode) = (0u16, 0u16, false, 0);
    let mut cur = arglist_v.clone();
    let mut dotted = Value::Nil;
    loop {
        match cur {
            Value::Cons(c) => {
                let (hd, tl) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let Some(id) = i.sym_id(&hd) else {
                    return None;
                };
                if id == opt_sym {
                    mode = 1;
                } else if id == rest_sym {
                    many = true;
                    mode = 2;
                } else if mode == 0 {
                    min += 1;
                    max += 1;
                } else if mode == 1 {
                    max += 1;
                }
                cur = tl;
            }
            Value::Nil => break,
            other => {
                dotted = other;
                break;
            }
        }
    }
    // `(a . r)' is malformed; `(a &rest . r)' binds r as rest-arg —
    // GNU reads it through `&rest', and the tail must name a symbol.
    if !dotted.is_nil() && (!many || i.sym_id(&dotted).is_none()) {
        return None;
    }
    Some(if many {
        Arity::Many { min }
    } else {
        Arity::Range { min, max }
    })
}

/// A Lambda's `Arity': parsed arg fields for source closures, the
/// packed/list argdesc in `bc_items[0]' for `#[...]' byte-code objects.
fn lambda_arity(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Option<Arity> {
    // GNU `func_arity' consults the raw arglist for interpreted
    // functions — `(b 1)' optional specs or a non-`&rest' dot make
    // it malformed → invalid-function (only callable lambdas carry
    // a parsed arity).
    if let Some(al) = &l.arglist {
        if al.is_nil() || matches!(al, Value::Cons(_)) {
            return raw_arglist_arity(i, al);
        }
    }
    if l.bad_arglist {
        return None;
    }
    Some(lambda_arity_raw(i, l))
}

fn lambda_arity_raw(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Arity {
    if let Some(items) = &l.bc_items {
        match items.borrow().first() {
            Some(Value::Int(n)) => {
                let req = (*n & 0x7f) as u16;
                let rest = (*n >> 7) & 1 != 0;
                let max = ((*n >> 8) & 0x7f) as u16;
                if rest {
                    Arity::Many { min: req }
                } else {
                    Arity::Range { min: req, max }
                }
            }
            Some(list @ (Value::Cons(_) | Value::Nil)) => {
                let opt = i.q("&optional");
                let rst = i.q("&rest");
                let (mut min, mut max, mut rest, mut mode) = (0u16, 0u16, false, 0);
                let mut cur = list.clone();
                while let Value::Cons(c) = cur {
                    let (hd, tl) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match i.sym_id(&hd) {
                        Some(s) if s == opt => mode = 1,
                        Some(s) if s == rst => {
                            rest = true;
                            mode = 2;
                        }
                        Some(_) => {
                            min += (mode == 0) as u16;
                            max += (mode <= 1) as u16;
                        }
                        None => {}
                    }
                    cur = tl;
                }
                if rest {
                    Arity::Many { min }
                } else {
                    Arity::Range { min, max }
                }
            }
            _ => l.arity(),
        }
    } else {
        l.arity()
    }
}
pub(super) fn f_func_arity(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU signals void-function for unbound symbols (indirect-function).
    if let Some(id) = i.sym_id(&args[0]) {
        if matches!(i.symbol_function(id), Value::Sym(s) if s == sym::UNBOUND) {
            return Err(i.signal_data(sym::VOID_FUNCTION, vec![args[0].clone()]));
        }
    }
    let fun = i.indirect_function_value(&args[0]);
    // GNU eval.c: (macro . X) is unwrapped one level before arity.
    let fun = match &fun {
        Value::Cons(c) if matches!(i.sym_id(&c.borrow().car), Some(s) if s == i.q("macro")) => {
            c.borrow().cdr.clone()
        }
        _ => fun,
    };
    // GNU `func_arity': a bare `(autoload ...)' cons that isn't reached
    // through a symbol's function cell signals (symbolp OBJ).
    let auto_id = i.q("autoload");
    if let Value::Cons(c) = &fun {
        if !matches!(args[0], Value::Sym(_))
            && matches!(i.sym_id(&c.borrow().car), Some(s) if s == auto_id)
        {
            return Err(i.wrong_type_mut("symbolp", &args[0]));
        }
    }
    // GNU: `func-arity' on an autoloaded symbol LOADS the file first
    // (Ffunc_arity calls autoload_do_load).
    let fun = match &fun {
        Value::Cons(c) if matches!(i.sym_id(&c.borrow().car), Some(s) if s == auto_id) => {
            crate::lisp::builtins::evalfn::autoload_do_load(i, fun.clone(), false)?
        }
        _ => fun,
    };
    // A `(lambda ARGLIST ...)' list.
    let mut list_arity = |v: &Value| -> Option<Arity> {
        let cells = v.list_to_vec().ok()?;
        let head = i.sym_id(cells.first()?)?;
        let name = i.symbol_name(head);
        // GNU 31.1: only `(lambda ...)' is a valid list function —
        // old-style `(closure ENV ...)' lists are invalid-function.
        if name != "lambda" {
            return None;
        }
        raw_arglist_arity(i, cells.get(1).unwrap_or(&Value::Nil))
    };

    let arity = match &fun {
        Value::Subr(s) => Some(s.arity),
        Value::Lambda(l) => lambda_arity(i, l),
        v => list_arity(v),
    };
    let arity = match arity {
        Some(a) => a,
        None => {
            return Err(i.signal_data(sym::INVALID_FUNCTION, vec![args[0].clone()]));
        }
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min, Value::Int(max as i128)),
        Arity::Many { min } => (min, Value::Sym(i.q("many"))),
        Arity::Unevalled => {
            // `(2 . unevalled)' for `if' — min from the special-form table.
            let min = match &args[0] {
                Value::Sym(id) => crate::lisp::special::special_form_min_args(*id),
                Value::Subr(s) => crate::lisp::special::special_form_min_args(i.intern(s.name)),
                _ => 0,
            };
            (min, Value::Sym(i.q("unevalled")))
        }
    };
    Ok(Value::cons(Value::Int(min as i128), max))
}

pub(super) fn f_subr_arity(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU requires the subr object itself (not a symbol naming one).
    let arity = match &args[0] {
        Value::Subr(s) => s.arity,
        // GNU 31.1 nativecomp: Lisp-defined functions report `subrp' t
        // and `subr-arity' returns their arglist-derived range.  remacs
        // marks dumped definitions `compiled' (`f_subrp'), so a compiled
        // non-macro lambda must be accepted here too — and that INCLUDES
        // `.elc'-twin `#[...]' objects, which GNU native-compiles to
        // real subrs (their `subrp' is already t, so `subr-arity' must
        // accept them or it contradicts `subrp').
        // Only RAW `#[...]' literals (`compiled' false — e.g. what
        // `apply-partially' returns or a reader byte-code vector) stay
        // non-subrp and signal `wrong-type-argument' like GNU.
        Value::Lambda(l) if l.compiled && !l.is_macro => lambda_arity_raw(i, l),
        other => return Err(i.wrong_type_mut("subrp", other)),
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min as i128, Value::Int(max as i128)),
        Arity::Many { min } => (min as i128, Value::Sym(i.q("many"))),
        // Unevalled only ever comes from a Subr (lambdas can't be
        // unevalled), so the special-form table lookup stays on its name.
        Arity::Unevalled => (
            crate::lisp::special::special_form_min_args(match &args[0] {
                Value::Subr(s) => i.intern(s.name),
                _ => unreachable!("unevalled arity on non-subr"),
            }) as i128,
            Value::Sym(i.q("unevalled")),
        ),
    };
    Ok(Value::cons(Value::Int(min), max))
}

pub(super) fn f_closurep(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `closurep' is t for interpreted closures AND byte-code
    // objects alike (both are `function' cells that close over state);
    // only subrs/primitives and non-functions answer nil.
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

pub(super) fn f_interpreted_function_p(_i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        matches!(&args[0], Value::Lambda(l) if !l.is_bytecode()),
    ))
}

pub(crate) fn f_make_interpreted_closure(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // (make-interpreted-closure ARGS BODY ENV &optional DOCSTRING IFORM)
    // — GNU eval.c: slots [args body env nil docstring iform], length
    // 3/5/6 by trailing non-nil slots.  IFORM is `(interactive X)'.
    // GNU's checks: `consp' on BODY first (a nil body is an error), then
    // `listp' on ARGS; ENV is never checked.
    if !matches!(args[1], Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &args[1]));
    }
    // CHECK_LIST checks the outer shape only.  The binding walk diagnoses
    // a dotted argument tail when the resulting closure is called.
    if !matches!(args[0], Value::Nil | Value::Cons(_)) {
        return Err(i.wrong_type_mut("listp", &args[0]));
    }
    let doc = args.get(3).cloned().filter(|v| !v.is_nil());
    let iform = args.get(4).cloned().filter(|v| !v.is_nil());
    let v = make_interpreted_closure(i, &args[0], &args[1], &args[2], false, None);
    let Value::Lambda(l) = &v else {
        return Ok(v);
    };
    let doc_str = doc.as_ref().and_then(|d| match d {
        Value::Str(s) => Some(s.borrow().clone()),
        _ => None,
    });
    Ok(Value::Lambda(Rc::new(Lambda {
        doc_value: doc,
        env_value: Some(args[2].clone()),
        interactive: iform.or_else(|| l.interactive.clone()),
        is_macro: l.is_macro,
        required: l.required.clone(),
        optional: l.optional.clone(),
        rest: l.rest,
        body: l.body.clone(),
        env: l.env.clone(),
        doc: doc_str.or_else(|| l.doc.clone()),
        name: l.name.clone(),
        bad_arglist: l.bad_arglist,
        arglist: l.arglist.clone(),
        plain: l.plain,
        dumped_doc: l.dumped_doc,
        compiled: l.compiled,
        advice_link: l.advice_link,
        bc_items: l.bc_items.clone(),
        bc_closure: l.bc_closure,
        bc_prog: std::cell::RefCell::new(None),
    }))
    .tracked())
}

/// Build an interpreted-closure Lambda from ARGLIST, BODY (a list of
/// body forms) and ENV (nil → dynamic, `(t)' → dynamic top-level,
/// otherwise an alist lexical frame).  `plain' mirrors `Lambda.plain':
/// the printer shows `nil' for the env of plain lambdas, `(t)' for
/// defun-produced ones — matching Emacs 31's `#[args body env]' repr.
/// `bc_items' carries the raw `#[...]' elements for `arrayp'/`aref'
/// when the object was read as a byte-code literal.
pub(crate) fn make_interpreted_closure(
    i: &mut Interp,
    arglist_v: &Value,
    body_v: &Value,
    env_v: &Value,
    plain: bool,
    bc_items: Option<Vec<Value>>,
) -> Value {
    // Parse ARGS (a list arglist) into required/optional/rest.
    let arglist = arglist_v.list_to_vec().unwrap_or_default();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut mode = 0; // 0 req, 1 opt, 2 rest
    let opt_sym = i.q("&optional");
    let rest_sym = i.q("&rest");
    for a in &arglist {
        if let Some(id) = i.sym_id(a) {
            if id == opt_sym {
                mode = 1;
                continue;
            }
            if id == rest_sym {
                mode = 2;
                continue;
            }
            if mode == 2 {
                rest = Some(id);
                continue;
            }
            if mode == 0 {
                required.push(id);
            } else {
                optional.push(crate::lisp::value::OptParam {
                    sym: id,
                    default: None,
                    supplied: None,
                });
            }
        }
    }
    let body = body_v.list_to_vec().unwrap_or_default();
    let env = crate::lisp::eval::lexenv_from_value(env_v);
    // A `#[ARGDESC STR CONSTS DEPTH DOC ISPEC]' literal read while
    // evaluating a `.elc' dump blob stands in for GNU's byte-compiled
    // (or, on nativecomp builds, subr-shaped) function object: mark it
    // `compiled' so `subrp'/`type-of' report the nativecomp surface,
    // and pull the doc/interactive slots into the Lambda like
    // `parse_lambda' does for source definitions.  Plain `#[...]' reads
    // (user code, `make-byte-code') stay `compiled' = nil.
    let elc_compiled = i.loading_dumped
        && bc_items
            .as_ref()
            .map(|items| crate::lisp::bytecode::is_byte_code(items))
            .unwrap_or(false);
    let mut bc_doc = None;
    let mut bc_doc_str = None;
    let mut bc_interactive = None;
    if elc_compiled {
        let items = bc_items.as_ref().unwrap();
        if let Some(dv) = items.get(4).filter(|v| !v.is_nil()) {
            if let Value::Str(s) = dv {
                bc_doc_str = Some(s.borrow().clone());
            }
            bc_doc = Some(dv.clone());
        }
        if let Some(spec) = items.get(5).filter(|v| !v.is_nil()) {
            bc_interactive = Some(Value::list(vec![
                Value::Sym(i.q("interactive")),
                spec.clone(),
            ]));
        }
    }
    Value::Lambda(Rc::new(Lambda {
        is_macro: false,
        required,
        optional,
        rest,
        body,
        env,
        doc: bc_doc_str,
        interactive: bc_interactive,
        name: None,
        bad_arglist: false,
        arglist: Some(arglist_v.clone()),
        plain,
        dumped_doc: false,
        compiled: elc_compiled,
        advice_link: None,
        bc_items: bc_items.map(|v| crate::lisp::Value::vector_ref(v)),
        bc_closure: false,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: bc_doc,
        env_value: None,
    }))
    .tracked()
}

/// GNU's closure element view for `aref'/`length'/`copy-sequence':
/// `[ARGS BODY ENV nil DOCSTRING IFORM]' truncated to 3, 5, or 6
/// slots — 6 when interactive, 5 when a docstring/type slot is
/// present, else 3 (eval.c `Fmake_interpreted_closure').  Byte-code
/// `#[...]' literals keep their literal elements.
pub(crate) fn lambda_slot_values(l: &Lambda) -> Vec<Value> {
    if let Some(items) = &l.bc_items {
        return items.borrow().clone();
    }
    // GNU's iform slot holds the spec *inside* `(interactive ...)':
    // its single form, or a vector when several forms follow.
    let iform_val = l.interactive.as_ref().and_then(|f| {
        let inner = match f {
            Value::Cons(c) => c.borrow().cdr.clone(),
            other => other.clone(),
        };
        let forms = inner.list_to_vec().unwrap_or_default();
        match forms.len() {
            1 => Some(forms.into_iter().next().unwrap()),
            0 => None,
            _ => Some(Value::vector(forms)),
        }
    });
    // GNU sizes by `!NILP (iform)' — `(interactive)' alone still
    // produces a 6-slot closure with a nil slot 5.
    let has_if = l.interactive.is_some();
    let env = l.env_value.clone().unwrap_or_else(|| {
        if l.env.is_none() {
            Value::Nil
        } else {
            crate::lisp::eval::lexenv_as_value(&l.env)
        }
    });
    let mut slots = vec![
        l.arglist.clone().unwrap_or(Value::Nil),
        Value::list(l.body.clone()),
        env,
        Value::Nil,
        l.doc_value.clone().unwrap_or(Value::Nil),
        iform_val.unwrap_or(Value::Nil),
    ];
    slots.truncate(if has_if {
        6
    } else if l.doc_value.is_some() {
        5
    } else {
        3
    });
    slots
}

pub(super) fn f_make_var_non_special(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Finternal_make_var_non_special: CHECK_SYMBOL
    // on VARIABLE before clearing its special flag.
    let id = want_sym(i, &args[0])?;
    i.obarray.symbol_mut(id).special = false;
    Ok(Value::Nil)
}

pub(super) fn f_special_variable_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match i.sym_id(&args[0]) {
        Some(id) => {
            // GNU: every interned keyword is self-quoting/defconst'd,
            // hence special.
            if i.symbol_name(id).starts_with(':') {
                return Ok(Value::t());
            }
            Ok(Value::from_bool(i.obarray.symbol(id).special))
        }
        None => Err(i.wrong_type_mut("symbolp", &args[0])),
    }
}

pub(super) fn f_primitive_function_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::from_bool(matches!(a[0], Value::Subr(_))))
}

pub(super) fn f_read_expression(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (read--expression PROMPT &optional INITIAL-CONTENTS) — read a
    // string from the minibuffer, then `read' one form from it.
    let prompt = match a.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        _ => crate::lisp::text::EmacsText::default(),
    };
    let input = if i.minibuf_reader.is_some() {
        // GNU read--expression → read-from-minibuffer with
        // `read-expression-history' as HISTVAR.
        let args = crate::lisp::eval::MinibufArgs {
            hist: Value::Sym(i.q("read-expression-history")),
            initial: a.get(1).cloned().unwrap_or(Value::Nil),
            ..Default::default()
        };
        Value::string(i.minibuf_read(&prompt, args)?)
    } else if i.noninteractive {
        // GNU `read_minibuf_noninteractive' reads a line from stdin.
        i.batch_read_line(&prompt)?
    } else {
        // GNU reads an answer from the minibuffer; in batch that is
        // stdin and signals end-of-file with this message.
        let eof = i.q("end-of-file");
        return Err(i.signal_data(eof, vec![Value::string("Error reading from stdin")]));
    };
    // GNU `string_to_object': parse the line; empty input has no
    // default here, so it signals `end-of-file'.
    i.string_to_object(&input, &Value::Nil)
}

/// `make-closure` — GNU's `Fmake_closure': splice the runtime
/// ENV values over the `V0..Vn' placeholder slots at the head of the
/// prototype's constant vector (see `byte-compile-make-closure'),
/// producing a real byte-code closure.
pub(super) fn f_make_closure(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Value::Lambda(l) = &a[0] else {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    };
    let Some(items) = &l.bc_items else {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    };
    let mut new_items = items.borrow().clone();
    let env = &a[1..];
    let Value::Vec(consts) = &new_items[2] else {
        return Err(i.wrong_type_mut("vectorp", &new_items[2]));
    };
    let old = consts.borrow();
    let mut spliced: Vec<Value> = env.to_vec();
    spliced.extend_from_slice(&old[env.len().min(old.len())..]);
    drop(old);
    new_items[2] = Value::vector(spliced);
    Ok(Value::Lambda(Rc::new(Lambda {
        is_macro: l.is_macro,
        required: l.required.clone(),
        optional: l.optional.clone(),
        rest: l.rest,
        body: l.body.clone(),
        env: l.env.clone(),
        doc: l.doc.clone(),
        interactive: l.interactive.clone(),
        name: l.name.clone(),
        bad_arglist: l.bad_arglist,
        arglist: l.arglist.clone(),
        plain: l.plain,
        dumped_doc: l.dumped_doc,
        compiled: l.compiled,
        advice_link: l.advice_link.clone(),
        bc_items: Some(crate::lisp::Value::vector_ref(new_items)),
        bc_closure: true,
        bc_prog: std::cell::RefCell::new(None),
        doc_value: l.doc_value.clone(),
        env_value: l.env_value.clone(),
    }))
    .tracked())
}

/// `subr-native-elisp-p' — t when SUBR is a native-compiled Lisp
/// function (our cc-emitted code), nil for built-in primitives.
pub(super) fn f_subr_native_elisp_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Subr(s) => Ok(Value::from_bool(
            crate::lisp::builtins::comp::is_native(s).is_some(),
        )),
        other => Err(i.wrong_type_mut("subrp", other)),
    }
}
