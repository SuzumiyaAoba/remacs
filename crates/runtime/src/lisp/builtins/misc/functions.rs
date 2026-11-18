//! Function metadata, arity, closure construction and gensym.

use crate::lisp::builtins::arg;
use crate::lisp::builtins::want_sym;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::{Arity, Lambda, Value},
};
use rustc_hash::FxHashSet;
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
#[derive(Clone, Copy)]
pub(crate) enum FunctionArity {
    Range { min: usize, max: usize },
    Many { min: usize },
    Unevalled,
}

impl From<Arity> for FunctionArity {
    fn from(arity: Arity) -> Self {
        match arity {
            Arity::Range { min, max } => Self::Range {
                min: min.into(),
                max: max.into(),
            },
            Arity::Many { min } => Self::Many { min: min.into() },
            Arity::Unevalled => Self::Unevalled,
        }
    }
}

pub(crate) fn raw_arglist_arity(i: &mut Interp, arglist_v: &Value) -> Option<FunctionArity> {
    let opt_sym = i.q("&optional");
    let rest_sym = i.q("&rest");
    let (mut min, mut max, mut mode) = (0usize, 0usize, 0);
    let mut cur = arglist_v.clone();
    let mut dotted = Value::Nil;
    let mut seen = FxHashSet::default();
    loop {
        match cur {
            Value::Cons(c) => {
                if !seen.insert(Rc::as_ptr(&c) as usize) {
                    return None;
                }
                let (hd, tl) = {
                    let b = c.try_borrow().ok()?;
                    (b.car.clone(), b.cdr.clone())
                };
                let Some(id) = i.sym_id(&hd) else {
                    return None;
                };
                if id == opt_sym {
                    mode = 1;
                } else if id == rest_sym {
                    // GNU `lambda_arity' stops at the first &rest marker.
                    // It reports the required prefix without validating the
                    // rest variable or any later arglist cells; call-time
                    // validation remains `funcall_lambda's responsibility.
                    return Some(FunctionArity::Many { min });
                } else if mode == 0 {
                    min = min.checked_add(1)?;
                    max = max.checked_add(1)?;
                } else if mode == 1 {
                    max = max.checked_add(1)?;
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
    // A dotted tail before an &rest marker is malformed.
    if !dotted.is_nil() {
        return None;
    }
    Some(FunctionArity::Range { min, max })
}

/// A Lambda's `Arity': parsed arg fields for source closures, the
/// packed/list argdesc in `bc_items[0]' for `#[...]' byte-code objects.
fn lambda_arity(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Option<FunctionArity> {
    // Byte-code/function literals keep the mutable raw arglist in slot 0.
    // Reader circle fixups and slot mutations can replace that value while
    // `l.arglist' remains the construction-time view.
    if l.bc_items.is_some() {
        return lambda_arity_raw(i, l);
    }
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
    let min = l.required.len();
    if l.rest.is_some() {
        Some(FunctionArity::Many { min })
    } else {
        Some(FunctionArity::Range {
            min,
            max: min.checked_add(l.optional.len())?,
        })
    }
}

fn lambda_arity_raw(i: &mut Interp, l: &std::rc::Rc<Lambda>) -> Option<FunctionArity> {
    if let Some(items) = &l.bc_items {
        let argdesc = items.try_borrow().ok()?.first().cloned();
        match argdesc {
            Some(argdesc @ (Value::Int(_) | Value::Cons(_) | Value::Nil)) => {
                let parsed = crate::lisp::bytecode::parse_argdesc(i, &argdesc);
                // `make-byte-code' permits malformed and even circular list
                // descriptors. Calls reject them; introspection must not
                // loop forever or overflow its narrow arity counters.
                if parsed.invalid_at.is_some() || parsed.req == usize::MAX {
                    return None;
                }
                Some(if parsed.has_rest {
                    FunctionArity::Many { min: parsed.req }
                } else {
                    FunctionArity::Range {
                        min: parsed.req,
                        max: parsed.max,
                    }
                })
            }
            // A mutated bytecode function can have an arglist slot that is
            // neither a packed descriptor nor a Lisp parameter list. GNU's
            // `lambda_arity' rejects that object instead of falling back to
            // the arity cached when it was constructed.
            Some(_) => None,
            None => Some(l.arity().into()),
        }
    } else {
        Some(l.arity().into())
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
    let macro_id = i.q("macro");
    let fun = match &fun {
        Value::Cons(c) => {
            let cell = c
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed function cell"))?;
            if matches!(i.sym_id(&cell.car), Some(s) if s == macro_id) {
                cell.cdr.clone()
            } else {
                fun.clone()
            }
        }
        _ => fun,
    };
    // GNU `func_arity': a bare `(autoload ...)' cons that isn't reached
    // through a symbol's function cell signals (symbolp OBJ).
    let auto_id = i.q("autoload");
    let is_autoload = match &fun {
        Value::Cons(c) => {
            let cell = c
                .try_borrow()
                .map_err(|_| i.error("Cannot inspect a mutably borrowed function cell"))?;
            matches!(i.sym_id(&cell.car), Some(s) if s == auto_id)
        }
        _ => false,
    };
    if is_autoload && !matches!(args[0], Value::Sym(_)) {
        return Err(i.wrong_type_mut("symbolp", &args[0]));
    }
    // GNU: `func-arity' on an autoloaded symbol LOADS the file first
    // (Ffunc_arity calls autoload_do_load).
    let fun = if is_autoload {
        crate::lisp::builtins::evalfn::autoload_do_load(i, fun, false)?
    } else {
        fun
    };
    // A `(lambda ARGLIST ...)' list.
    let mut list_arity = |v: &Value| -> Option<FunctionArity> {
        let cells = v.list_to_vec().ok()?;
        let head = i.sym_id(cells.first()?)?;
        // GNU 31.1: only `(lambda ...)' is a valid list function —
        // old-style `(closure ENV ...)' lists are invalid-function.
        if head != sym::LAMBDA {
            return None;
        }
        raw_arglist_arity(i, cells.get(1).unwrap_or(&Value::Nil))
    };

    let arity = match &fun {
        Value::Subr(s) => Some(s.arity.into()),
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
        FunctionArity::Range { min, max } => {
            (Value::integer(min as i128), Value::integer(max as i128))
        }
        FunctionArity::Many { min } => (Value::integer(min as i128), Value::Sym(i.q("many"))),
        FunctionArity::Unevalled => {
            // `(2 . unevalled)' for `if' — min from the special-form table.
            let min = match &args[0] {
                Value::Sym(id) => crate::lisp::special::special_form_min_args(*id),
                Value::Subr(s) => crate::lisp::special::special_form_min_args(i.intern(s.name)),
                _ => 0,
            };
            (Value::integer(min as i128), Value::Sym(i.q("unevalled")))
        }
    };
    Ok(Value::cons(min, max))
}

pub(super) fn f_subr_arity(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU requires the subr object itself (not a symbol naming one).
    let arity = match &args[0] {
        Value::Subr(s) => FunctionArity::from(s.arity),
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
        Value::Lambda(l) if l.compiled && !l.is_macro => match lambda_arity_raw(i, l) {
            Some(arity) => arity,
            None => return Err(i.signal_data(sym::INVALID_FUNCTION, vec![args[0].clone()])),
        },
        other => return Err(i.wrong_type_mut("subrp", other)),
    };
    let (min, max) = match arity {
        FunctionArity::Range { min, max } => (min as i128, Value::integer(max as i128)),
        FunctionArity::Many { min } => (min as i128, Value::Sym(i.q("many"))),
        // Unevalled only ever comes from a Subr (lambdas can't be
        // unevalled), so the special-form table lookup stays on its name.
        FunctionArity::Unevalled => (
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

pub(super) fn f_interpreted_function_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let interpreted = match &args[0] {
        Value::Lambda(lambda) => !lambda
            .try_is_bytecode()
            .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?,
        _ => false,
    };
    Ok(Value::from_bool(interpreted))
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
    let v = make_interpreted_closure(i, &args[0], &args[1], &args[2], false, None)?;
    let Value::Lambda(l) = &v else {
        return Ok(v);
    };
    let doc_str = match doc.as_ref() {
        Some(Value::Str(s)) => Some(
            s.try_borrow()
                .map(|text| (*text).clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed closure docstring"))?,
        ),
        _ => None,
    };
    Ok(Value::Lambda(Rc::new(Lambda {
        doc_value: doc,
        env_value: Some(args[2].clone()),
        interactive: iform.or_else(|| l.interactive.clone()),
        is_macro: l.is_macro,
        required: l.required.clone(),
        optional: l.optional.clone(),
        rest: l.rest,
        body: l.body.clone(),
        body_value: l.body_value.clone(),
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
        compiled_macro_cache: l.compiled_macro_cache.clone(),
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
) -> EvalResult {
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
    let body = match body_v.list_to_vec() {
        Ok(body) => body,
        // GNU checks only that BODY is a cons.  Its Fprogn walk evaluates
        // the cons prefix and ignores an improper atom tail, so retain that
        // prefix instead of turning the whole body into nil.
        Err(crate::lisp::value::ListError::Dotted(_)) => {
            let mut body = Vec::new();
            body_v.each_car(|form| body.push(form.clone()));
            body
        }
        // The parsed view is finite; retain the original list separately so
        // invocation can reject the cycle before evaluating any form.
        Err(crate::lisp::value::ListError::Circular(_)) => {
            let mut body = Vec::new();
            let mut tail = body_v.clone();
            let mut seen = std::collections::HashSet::new();
            while let Value::Cons(cell) = tail {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    break;
                }
                let borrowed = cell.borrow();
                body.push(borrowed.car.clone());
                tail = borrowed.cdr.clone();
            }
            body
        }
        Err(crate::lisp::value::ListError::Borrowed) => {
            return Err(i.error("Cannot inspect a mutably borrowed closure body"));
        }
    };
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
    Ok(Value::Lambda(Rc::new(Lambda {
        is_macro: false,
        required,
        optional,
        rest,
        body,
        body_value: Some(body_v.clone()),
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
        compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
        doc_value: bc_doc,
        env_value: None,
    }))
    .tracked())
}

#[derive(Clone, Copy)]
pub(crate) struct LambdaSlotBorrowed;

impl LambdaSlotBorrowed {
    pub(crate) fn into_flow(self, i: &Interp) -> Flow {
        i.error("Cannot inspect a mutably borrowed function object")
    }
}

/// Checked closure-vector view for Lisp entry points. A Rust caller can hold
/// a mutable borrow of an interpreted closure's captured environment or
/// interactive form, so these reads must fail through Lisp instead of
/// panicking in `RefCell::borrow'.
pub(crate) fn lambda_slot_values_checked(l: &Lambda) -> Result<Vec<Value>, LambdaSlotBorrowed> {
    if let Some(items) = &l.bc_items {
        return items
            .try_borrow()
            .map(|items| items.clone())
            .map_err(|_| LambdaSlotBorrowed);
    }

    let iform_val = match &l.interactive {
        Some(form) => {
            let inner = match form {
                Value::Cons(cell) => cell
                    .try_borrow()
                    .map(|cell| cell.cdr.clone())
                    .map_err(|_| LambdaSlotBorrowed)?,
                other => other.clone(),
            };
            let forms = match inner.list_to_vec() {
                Ok(forms) => forms,
                Err(crate::lisp::value::ListError::Borrowed) => {
                    return Err(LambdaSlotBorrowed);
                }
                Err(_) => Vec::new(),
            };
            match forms.len() {
                1 => Some(forms.into_iter().next().unwrap()),
                0 => None,
                _ => Some(Value::vector(forms)),
            }
        }
        None => None,
    };
    let env = match &l.env_value {
        Some(env) => env.clone(),
        None if l.env.is_none() => Value::Nil,
        None => {
            crate::lisp::eval::lexenv_as_value_checked(&l.env).map_err(|_| LambdaSlotBorrowed)?
        }
    };
    let mut slots = vec![
        l.arglist.clone().unwrap_or(Value::Nil),
        l.body_value
            .clone()
            .unwrap_or_else(|| Value::list(l.body.clone())),
        env,
        Value::Nil,
        l.doc_value.clone().unwrap_or(Value::Nil),
        iform_val.unwrap_or(Value::Nil),
    ];
    slots.truncate(if l.interactive.is_some() {
        6
    } else if l.doc_value.is_some() {
        5
    } else {
        3
    });
    Ok(slots)
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
        Some(Value::Str(s)) => s
            .try_borrow()
            .map(|prompt| (*prompt).clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed read-expression prompt"))?,
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
    if !l
        .try_is_bytecode()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?
    {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    }
    let Some(items) = &l.bc_items else {
        return Err(i.wrong_type_mut("byte-code-function-p", &a[0]));
    };
    let mut new_items = items
        .try_borrow()
        .map(|items| items.clone())
        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code object"))?;
    let env = &a[1..];
    let Value::Vec(consts) = &new_items[2] else {
        return Err(i.wrong_type_mut("vectorp", &new_items[2]));
    };
    let old = consts
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code constants vector"))?;
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
        body_value: l.body_value.clone(),
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
        compiled_macro_cache: l.compiled_macro_cache.clone(),
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

#[cfg(test)]
mod arity_list_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    fn bytecode_lambda(arglist: Value, compiled: bool) -> Rc<Lambda> {
        Rc::new(Lambda {
            is_macro: false,
            required: Vec::new(),
            optional: Vec::new(),
            rest: None,
            body: Vec::new(),
            body_value: None,
            env: None,
            doc: None,
            interactive: None,
            name: None,
            bad_arglist: false,
            arglist: None,
            plain: false,
            dumped_doc: false,
            advice_link: None,
            compiled,
            bc_items: Some(Rc::new(std::cell::RefCell::new(vec![arglist]))),
            bc_closure: false,
            bc_prog: std::cell::RefCell::new(None),
            compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
            doc_value: None,
            env_value: None,
        })
    }

    fn circular_bytecode_lambda(compiled: bool) -> Rc<Lambda> {
        let arglist = Value::cons(Value::Sym(sym::T), Value::Nil);
        if let Value::Cons(cell) = &arglist {
            cell.borrow_mut().cdr = arglist.clone();
        }
        bytecode_lambda(arglist, compiled)
    }

    #[test]
    fn bytecode_arity_keeps_valid_list_shapes_and_wide_counts() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::vacant();
                interp.obarray = Obarray::new();
                let optional = Value::Sym(interp.intern("&optional"));
                let rest = Value::Sym(interp.intern("&rest"));
                let a = Value::Sym(interp.intern("arity-a"));
                let b = Value::Sym(interp.intern("arity-b"));
                let tail = Value::Sym(interp.intern("arity-tail"));
                let arglist = Value::list(vec![a, optional, b, rest, tail]);
                let function = Value::Lambda(bytecode_lambda(arglist, false));
                let arity = f_func_arity(&mut interp, vec![function].into()).unwrap();
                assert_eq!(interp.prin1_to_string(&arity), "(1 . many)");

                let optional = Value::Sym(interp.intern("&optional"));
                let rest = Value::Sym(interp.intern("&rest"));
                let arglist = Value::list(vec![
                    Value::Sym(interp.intern("subr-arity-a")),
                    optional,
                    Value::Sym(interp.intern("subr-arity-b")),
                    rest,
                    Value::Sym(interp.intern("subr-arity-tail")),
                ]);
                let function = Value::Lambda(bytecode_lambda(arglist, true));
                let arity = f_subr_arity(&mut interp, vec![function].into()).unwrap();
                assert_eq!(interp.prin1_to_string(&arity), "(1 . many)");

                let packed = Value::Int(1 | (2 << 8));
                let function = Value::Lambda(bytecode_lambda(packed, true));
                let arity = f_subr_arity(&mut interp, vec![function].into()).unwrap();
                assert_eq!(interp.prin1_to_string(&arity), "(1 . 2)");

                let arglist = Value::list(vec![Value::Sym(sym::T); 65_536]);
                let function = Value::Lambda(bytecode_lambda(arglist, false));
                let arity = f_func_arity(&mut interp, vec![function].into()).unwrap();
                assert_eq!(interp.prin1_to_string(&arity), "(65536 . 65536)");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn bytecode_arity_query_terminates_on_a_circular_dynamic_arglist() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();

        let function = circular_bytecode_lambda(false);
        assert!(lambda_arity_raw(&mut interp, &function).is_none());
        let result = f_func_arity(&mut interp, vec![Value::Lambda(function)].into());
        assert!(matches!(
            result,
            Err(crate::lisp::error::Flow::Signal(ref signal))
                if signal.is_symbol(sym::INVALID_FUNCTION)
        ));

        let function = circular_bytecode_lambda(true);
        let result = f_subr_arity(&mut interp, vec![Value::Lambda(function)].into());
        assert!(matches!(
            result,
            Err(crate::lisp::error::Flow::Signal(ref signal))
                if signal.is_symbol(sym::INVALID_FUNCTION)
        ));
    }

    #[test]
    fn raw_func_arity_rejects_circular_arglists() {
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::vacant();
                interp.obarray = Obarray::new();
                let arglist = Value::cons(Value::Sym(sym::T), Value::Nil);
                if let Value::Cons(cell) = &arglist {
                    cell.borrow_mut().cdr = arglist.clone();
                }
                assert!(raw_arglist_arity(&mut interp, &arglist).is_none());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn func_arity_reports_borrowed_arglists_without_panicking() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let arglist = Value::list(vec![Value::Sym(sym::T)]);
        let body = Value::list(vec![Value::Nil]);
        let function = make_interpreted_closure(
            &mut interp,
            &arglist,
            &body,
            &Value::Nil,
            false,
            None,
        )
        .unwrap();
        let Value::Cons(cell) = &arglist else {
            unreachable!();
        };
        let _borrow = cell.borrow_mut();

        let result = f_func_arity(&mut interp, vec![function].into());
        assert!(matches!(
            result,
            Err(crate::lisp::error::Flow::Signal(ref signal))
                if signal.is_symbol(sym::INVALID_FUNCTION)
        ));
    }
}

#[cfg(test)]
mod interpreted_closure_body_tests {
    use super::*;

    #[test]
    fn constructor_retains_live_body_spine_and_rejects_cycles() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(let* ((body (cons '(+ 20 22) 'ignored))\
                                (function (make-interpreted-closure nil body nil)))\
                           (list (eq (aref function 1) body) (funcall function)))",
                    )
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "(t 42)");

                let result = interp
                    .eval_str(
                        "(let* ((body (list 42))\
                                (function (make-interpreted-closure nil body nil)))\
                           (setcar body 7)\
                           (funcall function))",
                    )
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "7");

                let result = interp
                    .eval_str(
                        "(let* ((body (list '(setq qa-closure-cycle-side t)))\
                                (function (make-interpreted-closure nil body nil))\
                                (qa-closure-cycle-side nil))\
                           (setcdr body body)\
                           (condition-case err\
                               (funcall function)\
                             (circular-list\
                              (list (car err) qa-closure-cycle-side))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&result),
                    "(circular-list nil)"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
