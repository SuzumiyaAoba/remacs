//! Special forms: `quote`, `if`, `let`, `defun`, `condition-case`, etc.
//! These receive their arguments unevaluated (as a raw list).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::error::{EvalResult, Flow};
use super::eval::LexFrame;
use super::obarray::sym;
use super::value::{SymId, Value};
use super::Interp;

type SpecialFn = fn(&mut Interp, Value) -> EvalResult;

/// Dispatch table for special forms.
pub fn special_form(id: SymId) -> Option<SpecialFn> {
    Some(match id {
        sym::QUOTE => sf_quote,
        sym::FUNCTION => sf_function,
        sym::IF => sf_if,
        sym::COND => sf_cond,
        sym::PROGN => sf_progn,
        sym::PROG1 => sf_prog1,
        sym::PROG2 => sf_prog2,
        sym::AND => sf_and,
        sym::OR => sf_or,
        sym::LET => sf_let,
        sym::LET_STAR => sf_let_star,
        sym::SETQ => sf_setq,
        sym::SETQ_DEFAULT => sf_setq_default,
        sym::DEFVAR => sf_defvar,
        sym::DEFCONST => sf_defconst,
        sym::DEFUN => sf_defun,
        sym::DEFMACRO => sf_defmacro,
        sym::LAMBDA => sf_lambda,
        sym::WHILE => sf_while,
        sym::CATCH => sf_catch,
        sym::UNWIND_PROTECT => sf_unwind_protect,
        sym::CONDITION_CASE => sf_condition_case,
        sym::INTERACTIVE => |_i, _a| Ok(Value::Nil),
        sym::SAVE_EXCURSION => sf_save_excursion,
        sym::SAVE_CURRENT_BUFFER => sf_save_current_buffer,
        sym::WITH_CURRENT_BUFFER => sf_with_current_buffer,
        sym::SAVE_RESTRICTION => sf_save_restriction,
        sym::TRACK_MOUSE => sf_progn,
        sym::BACKQUOTE => sf_backquote,
        _ => return None,
    })
}

// ---------- arg access helpers ----------

fn car(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}

fn cdr(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().cdr.clone(),
        _ => Value::Nil,
    }
}

fn cadr(v: &Value) -> Value {
    car(&cdr(v))
}

fn nth_arg(v: &Value, n: usize) -> Value {
    let mut cur = v.clone();
    for _ in 0..n {
        cur = cdr(&cur);
    }
    car(&cur)
}



impl Interp {
    /// Is `lexical-binding` currently in effect?
    pub fn lexical_binding_active(&self) -> bool {
        let id = self.obarray.intern_soft("lexical-binding").unwrap_or(0);
        self.symbol_value(id).truthy()
    }

    /// `let`/`let*`/`condition-case` variable binding honoring scoping:
    /// binds lexically when lexical-binding is active and the var isn't
    /// special, else specbinds dynamically.
    pub fn bind_var(
        &mut self,
        lex_vars: Option<&Rc<LexFrame>>,
        sym: SymId,
        val: Value,
    ) {
        let is_special = self.obarray.symbol(sym).special;
        match (lex_vars, is_special) {
            (Some(frame), false) => {
                frame.vars.borrow_mut().insert(sym, val);
            }
            _ => self.specbind(sym, val),
        }
    }
}

// ---------- the special forms ----------

fn sf_quote(_i: &mut Interp, args: Value) -> EvalResult {
    Ok(car(&args))
}

fn sf_function(i: &mut Interp, args: Value) -> EvalResult {
    let arg = car(&args);
    match &arg {
        Value::Cons(_) => {
            let head = car(&arg);
            if i.sym_is(&head, sym::LAMBDA) {
                let lambda = i.lambda_from_form(&arg, None)?;
                return Ok(Value::Lambda(Rc::new(lambda)));
            }
            Ok(arg)
        }
        Value::Sym(_) => {
            // Emacs: #'sym resolves to the symbol's function cell.
            let f = i.indirect_function_value(&arg);
            match f {
                Value::Sym(s) if s == sym::UNBOUND => Ok(arg.clone()),
                _ => Ok(f),
            }
        }
        _ => Ok(arg),
    }
}

fn sf_if(i: &mut Interp, args: Value) -> EvalResult {
    let cond = i.eval(&car(&args))?;
    if cond.truthy() {
        i.eval(&cadr(&args))
    } else {
        // else-part is a progn.
        i.eval_progn(&cdr(&cdr(&args)))
    }
}

fn sf_cond(i: &mut Interp, args: Value) -> EvalResult {
    let mut cur = args;
    loop {
        match cur {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(c) => {
                let (clause, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let test = car(&clause);
                let v = i.eval(&test)?;
                if v.truthy() {
                    let body = cdr(&clause);
                    if body.is_nil() {
                        return Ok(v);
                    }
                    return i.eval_progn(&body);
                }
                cur = next;
            }
            _ => return Err(i.error("bad cond clause")),
        }
    }
}

fn sf_progn(i: &mut Interp, args: Value) -> EvalResult {
    i.eval_progn(&args)
}

fn sf_prog1(i: &mut Interp, args: Value) -> EvalResult {
    let first = i.eval(&car(&args))?;
    i.eval_progn(&cdr(&args))?;
    Ok(first)
}

fn sf_prog2(i: &mut Interp, args: Value) -> EvalResult {
    i.eval(&car(&args))?;
    let second = i.eval(&cadr(&args))?;
    i.eval_progn(&cdr(&cdr(&args)))?;
    Ok(second)
}

fn sf_and(i: &mut Interp, args: Value) -> EvalResult {
    let mut cur = args;
    let mut last = Value::t();
    loop {
        match cur {
            Value::Nil => return Ok(last),
            Value::Cons(c) => {
                let (car_v, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                last = i.eval(&car_v)?;
                if last.is_nil() {
                    return Ok(Value::Nil);
                }
                cur = next;
            }
            _ => return Err(i.error("dotted and")),
        }
    }
}

fn sf_or(i: &mut Interp, args: Value) -> EvalResult {
    let mut cur = args;
    loop {
        match cur {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(c) => {
                let (car_v, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let v = i.eval(&car_v)?;
                if v.truthy() {
                    return Ok(v);
                }
                cur = next;
            }
            _ => return Err(i.error("dotted or")),
        }
    }
}

/// Parse a `let`/`let*` varspec list into (sym, init-form) pairs.
fn parse_let_specs(i: &mut Interp, specs: &Value) -> Result<Vec<(SymId, Value)>, Flow> {
    match specs {
        Value::Nil => return Ok(Vec::new()),
        Value::Sym(id) => {
            // (let (var) ...) — bare symbol in the list
            return Ok(vec![(*id, Value::Nil)]);
        }
        _ => {}
    }
    let mut out = Vec::new();
    let mut cur = specs.clone();
    loop {
        match cur {
            Value::Nil => return Ok(out),
            Value::Cons(c) => {
                let (spec, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                match &spec {
                    Value::Sym(id) => out.push((*id, Value::Nil)),
                    Value::Cons(_) => {
                        let pair = spec.list_to_vec().map_err(|_| {
                            i.error("Bad binding in `let'")
                        })?;
                        let s = pair.first().and_then(|v| i.sym_id(v)).ok_or_else(|| {
                            i.error("Bad binding in `let'")
                        })?;
                        out.push((s, pair.get(1).cloned().unwrap_or(Value::Nil)));
                    }
                    _ => return Err(i.error("Bad binding in `let'")),
                }
                cur = next;
            }
            _ => return Err(i.error("Bad binding list in `let'")),
        }
    }
}

fn sf_let(i: &mut Interp, args: Value) -> EvalResult {
    let specs = parse_let_specs(i, &car(&args))?;
    let body = cdr(&args);
    if i.lexical_binding_active() {
        // Parallel binding: evaluate all inits in the outer env first.
        let mut evaluated = Vec::with_capacity(specs.len());
        for (s, init) in &specs {
            let v = if init.is_nil() && matches!(&car(&args), Value::Cons(c) if {
                let b = c.borrow();
                matches!(&b.car, Value::Cons(_) | Value::Nil) || b.car.is_nil()
            }) {
                i.eval(init)?
            } else {
                i.eval(init)?
            };
            evaluated.push((*s, v));
        }
        let frame = Rc::new(LexFrame {
            vars: RefCell::new(HashMap::new()),
            parent: i.lexenv.clone(),
        });
        let mark = i.specbind_depth();
        for (s, v) in evaluated {
            i.bind_var(Some(&frame), s, v);
        }
        let saved = std::mem::replace(&mut i.lexenv, Some(frame));
        let r = i.eval_progn(&body);
        i.lexenv = saved;
        i.unbind_to(mark);
        r
    } else {
        let mark = i.specbind_depth();
        // Evaluate all inits first (parallel binding).
        let mut evaluated = Vec::with_capacity(specs.len());
        let mut err = None;
        for (s, init) in &specs {
            match i.eval(init) {
                Ok(v) => evaluated.push((*s, v)),
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = err {
            return Err(e);
        }
        for (s, v) in evaluated {
            i.specbind(s, v);
        }
        let r = i.eval_progn(&body);
        i.unbind_to(mark);
        r
    }
}

fn sf_let_star(i: &mut Interp, args: Value) -> EvalResult {
    let specs = parse_let_specs(i, &car(&args))?;
    let body = cdr(&args);
    if i.lexical_binding_active() {
        let frame = Rc::new(LexFrame {
            vars: RefCell::new(HashMap::new()),
            parent: i.lexenv.clone(),
        });
        let mark = i.specbind_depth();
        let saved = std::mem::replace(&mut i.lexenv, Some(frame.clone()));
        let mut r = Ok(Value::Nil);
        for (s, init) in &specs {
            match i.eval(init) {
                Ok(v) => i.bind_var(Some(&frame), *s, v),
                Err(e) => {
                    r = Err(e);
                    break;
                }
            }
        }
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.lexenv = saved;
        i.unbind_to(mark);
        r
    } else {
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        for (s, init) in &specs {
            match i.eval(init) {
                Ok(v) => i.specbind(*s, v),
                Err(e) => {
                    r = Err(e);
                    break;
                }
            }
        }
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.unbind_to(mark);
        r
    }
}

fn setq_pairs(i: &mut Interp, args: &Value, default: bool) -> EvalResult {
    let mut cur = args.clone();
    let mut last = Value::Nil;
    let mut _n = 0usize;
    loop {
        match cur {
            Value::Nil => return Ok(last),
            Value::Cons(c) => {
                let (sym_v, next1) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let (val_f, next2) = {
                    match &next1 {
                        Value::Cons(c2) => {
                            let b = c2.borrow();
                            (b.car.clone(), b.cdr.clone())
                        }
                        _ => (Value::Nil, Value::Nil),
                    }
                };
                let sid = i.sym_id(&sym_v).ok_or_else(|| {
                    i.wrong_type_mut("symbolp", &sym_v)
                })?;
                last = i.eval(&val_f)?;
                if default {
                    i.set_symbol_default(sid, last.clone())?;
                } else {
                    // Under lexical-binding, setq writes the lexenv slot
                    // if one exists.
                    let is_special = i.obarray.symbol(sid).special;
                    let mut done = false;
                    if !is_special && i.lexenv.is_some() {
                        let mut env = &i.lexenv;
                        while let Some(frame) = env {
                            if frame.vars.borrow().contains_key(&sid) {
                                frame.vars.borrow_mut().insert(sid, last.clone());
                                done = true;
                                break;
                            }
                            env = &frame.parent;
                        }
                    }
                    if !done {
                        i.set_symbol(sid, last.clone())?;
                    }
                }
                cur = next2;
                _n += 1;
            }
            _ => return Err(i.error("odd number of setq args")),
        }
    }
}

fn sf_setq(i: &mut Interp, args: Value) -> EvalResult {
    setq_pairs(i, &args, false)
}

fn sf_setq_default(i: &mut Interp, args: Value) -> EvalResult {
    setq_pairs(i, &args, true)
}

fn sf_defvar(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(&args);
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    i.obarray.symbol_mut(sid).special = true;
    let init = cadr(&args);
    let has_init = !cdr(&args).is_nil();
    if has_init {
        // defvar sets the default only if the var is currently void.
        if !i.bound_p(sid) {
            let v = i.eval(&init)?;
            i.set_symbol_default(sid, v)?;
        }
    }
    let doc = nth_arg(&args, 2);
    if let Value::Str(s) = doc {
        let doc_str = s.borrow().clone();
        i.obarray.symbol_mut(sid).variable_documentation = Some(doc_str);
    }
    Ok(name_v)
}

fn sf_defconst(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(&args);
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    i.obarray.symbol_mut(sid).special = true;
    let v = i.eval(&cadr(&args))?;
    i.set_symbol_default(sid, v)?;
    i.obarray.symbol_mut(sid).constant = true;
    Ok(name_v)
}

fn sf_defun(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(&args);
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    let params = cadr(&args);
    let body_v = cdr(&cdr(&args));
    let body = body_v.list_to_vec().unwrap_or_default();
    let mut lambda = i.parse_lambda(&params, &body, Some(sid))?;
    // defun never captures a lexical env from the definition site in the
    // dynamic model; under lexical-binding it captures the file env.
    lambda.env = i.lexenv.clone();
    i.fset(sid, Value::Lambda(Rc::new(lambda)));
    Ok(name_v)
}

fn sf_defmacro(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(&args);
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    let params = cadr(&args);
    let body_v = cdr(&cdr(&args));
    let body = body_v.list_to_vec().unwrap_or_default();
    let mut lambda = i.parse_lambda(&params, &body, Some(sid))?;
    lambda.is_macro = true;
    lambda.env = i.lexenv.clone();
    i.fset(sid, Value::Lambda(Rc::new(lambda)));
    Ok(name_v)
}

fn sf_lambda(i: &mut Interp, args: Value) -> EvalResult {
    // `(lambda ...)' self-evaluates to a function object.
    let params = car(&args);
    let body_v = cdr(&args);
    let body = body_v.list_to_vec().unwrap_or_default();
    let lambda = i.parse_lambda(&params, &body, None)?;
    Ok(Value::Lambda(Rc::new(lambda)))
}

fn sf_while(i: &mut Interp, args: Value) -> EvalResult {
    let test = car(&args);
    let body = cdr(&args);
    loop {
        if i.quit_flag {
            i.quit_flag = false;
            return Err(Flow::Quit);
        }
        let v = i.eval(&test)?;
        if v.is_nil() {
            return Ok(Value::Nil);
        }
        i.eval_progn(&body)?;
    }
}

fn sf_catch(i: &mut Interp, args: Value) -> EvalResult {
    let tag = i.eval(&car(&args))?;
    let body = cdr(&args);
    match i.eval_progn(&body) {
        Err(Flow::Throw(t, v)) => {
            if crate::lisp::eq_values(&t, &tag) {
                Ok(v)
            } else {
                Err(Flow::Throw(t, v))
            }
        }
        other => other,
    }
}

fn sf_unwind_protect(i: &mut Interp, args: Value) -> EvalResult {
    let form = car(&args);
    let cleanup = cdr(&args);
    let result = i.eval(&form);
    let clean = i.eval_progn(&cleanup);
    match (result, clean) {
        (Ok(v), Ok(_)) => Ok(v),
        (Ok(_), Err(e)) => Err(e),
        (Err(e), Ok(_)) => Err(e),
        (Err(_), Err(e2)) => Err(e2),
    }
}

fn sf_condition_case(i: &mut Interp, args: Value) -> EvalResult {
    let var_v = car(&args);
    let bodyform = cadr(&args);
    let handlers = cdr(&cdr(&args));
    match i.eval(&bodyform) {
        Ok(v) => Ok(v),
        Err(Flow::Signal(sig, data)) => {
            // Build the condition object: (sig . data)
            let err_val = Value::cons(sig.clone(), data.clone());
            let mut cur = handlers;
            loop {
                match cur {
                    Value::Nil => return Err(Flow::Signal(sig, data)),
                    Value::Cons(c) => {
                        let (handler, next) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        let conds = car(&handler);
                        if i.signal_matches(&sig, &conds) {
                            // Bind var and run handler body.
                            let hbody = cdr(&handler);
                            let mark = i.specbind_depth();
                            let lex_frame = if i.lexical_binding_active() {
                                Some(Rc::new(LexFrame {
                                    vars: RefCell::new(HashMap::new()),
                                    parent: i.lexenv.clone(),
                                }))
                            } else {
                                None
                            };
                            let saved_lex = match &lex_frame {
                                Some(f) => {
                                    std::mem::replace(&mut i.lexenv, Some(f.clone()))
                                }
                                None => None,
                            };
                            if let Some(vid) = i.sym_id(&var_v) {
                                i.bind_var(lex_frame.as_ref(), vid, err_val);
                            }
                            let r = i.eval_progn(&hbody);
                            if lex_frame.is_some() {
                                i.lexenv = saved_lex;
                            }
                            i.unbind_to(mark);
                            return r;
                        }
                        cur = next;
                    }
                    _ => return Err(Flow::Signal(sig, data)),
                }
            }
        }
        other => other,
    }
}

// ---------- buffer-related special forms ----------

fn sf_save_excursion(i: &mut Interp, args: Value) -> EvalResult {
    let saved = i.save_excursion_state();
    let r = i.eval_progn(&args);
    i.restore_excursion_state(saved);
    r
}

fn sf_save_current_buffer(i: &mut Interp, args: Value) -> EvalResult {
    let old = i.current_buffer;
    let r = i.eval_progn(&args);
    i.set_current_buffer(old);
    r
}

fn sf_with_current_buffer(i: &mut Interp, args: Value) -> EvalResult {
    let buf_v = i.eval(&car(&args))?;
    let buf_id = i.buffer_id_of(&buf_v).ok_or_else(|| {
        i.wrong_type_mut("bufferp", &buf_v)
    })?;
    let old = i.current_buffer;
    i.set_current_buffer(buf_id);
    let body = cdr(&args);
    let r = i.eval_progn(&body);
    i.set_current_buffer(old);
    r
}

fn sf_save_restriction(i: &mut Interp, args: Value) -> EvalResult {
    let saved = i.save_restriction_state();
    let r = i.eval_progn(&args);
    i.restore_restriction_state(saved);
    r
}

// ---------- backquote ----------

fn sf_backquote(i: &mut Interp, args: Value) -> EvalResult {
    let expanded = backquote_expand(i, &car(&args), 0);
    i.eval(&expanded)
}

/// `` ` `` expansion producing a form that builds the structure.
pub fn backquote_expand(i: &mut Interp, v: &Value, depth: usize) -> Value {
    match v {
        Value::Cons(c) => {
            let (a, d) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // (,x) or (,x . rest)
            if let Some(inner) = comma_inner(i, &a) {
                return match d {
                    Value::Nil => inner,
                    _ => {
                        let d_exp = backquote_expand(i, &d, depth);
                        Value::list(vec![
                            Value::Sym(i.intern("cons")),
                            inner,
                            d_exp,
                        ])
                    }
                };
            }
            if let Some(inner) = comma_at_inner(i, &a) {
                let d_exp = backquote_expand(i, &d, depth);
                return Value::list(vec![
                    Value::Sym(i.intern("append")),
                    inner,
                    d_exp,
                ]);
            }
            // Nested backquote: `(a `(b ,c)) — inner bq expands first.
            if i.sym_is(&a, sym::BACKQUOTE) {
                let inner = backquote_expand(i, &cadr(&a), depth + 1);
                let inner_exp = backquote_expand(i, &inner, depth + 1);
                let d_exp = backquote_expand(i, &d, depth);
                return Value::list(vec![
                    Value::Sym(i.intern("list")),
                    Value::list(vec![Value::Sym(sym::QUOTE), Value::Sym(sym::BACKQUOTE)]),
                    Value::list(vec![
                        Value::Sym(i.intern("list")),
                        inner_exp,
                        d_exp,
                    ]),
                ]);
            }
            // (,@x . rest) at element position inside list is handled by
            // comma_at_inner above since `a` is the element.
            let a_exp = backquote_expand(i, &a, depth);
            let d_exp = backquote_expand(i, &d, depth);
            Value::list(vec![Value::Sym(i.intern("cons")), a_exp, d_exp])
        }
        Value::Vec(items) => {
            // `[a ,b ,@c] -> (vconcat [...] x [...])
            let parts: Vec<Value> = items.borrow().clone();
            let mut segments: Vec<Value> = Vec::new();
            let mut cur_list: Vec<Value> = Vec::new();
            for p in parts {
                if let Some(inner) = comma_at_inner(i, &p) {
                    if !cur_list.is_empty() {
                        let lit = Value::Vec(Rc::new(RefCell::new(cur_list.clone())));
                        let exp = backquote_expand(i, &lit, depth);
                        segments.push(exp);
                        cur_list.clear();
                    }
                    segments.push(inner);
                } else {
                    cur_list.push(p);
                }
            }
            if !cur_list.is_empty() {
                let lit = Value::Vec(Rc::new(RefCell::new(cur_list.clone())));
                let exp = backquote_expand(i, &lit, depth);
                segments.push(exp);
            }
            if segments.len() == 1 {
                return segments.pop().unwrap();
            }
            let mut form = vec![Value::Sym(i.intern("vconcat"))];
            form.extend(segments);
            Value::list(form)
        }
        _ => Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]),
    }
}

/// If `v` is `(, x)` return `x`.
fn comma_inner(i: &Interp, v: &Value) -> Option<Value> {
    if let Value::Cons(c) = v {
        let b = c.borrow();
        if i.sym_is(&b.car, sym::COMMA) {
            return Some(car(&b.cdr));
        }
    }
    None
}

/// If `v` is `(,@ x)` or `(,. x)` return `x`.
fn comma_at_inner(i: &Interp, v: &Value) -> Option<Value> {
    if let Value::Cons(c) = v {
        let b = c.borrow();
        if i.sym_is(&b.car, sym::COMMA_AT) || i.sym_is(&b.car, sym::COMMA_DOT) {
            return Some(car(&b.cdr));
        }
    }
    None
}
