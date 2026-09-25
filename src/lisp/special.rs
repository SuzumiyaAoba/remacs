//! Special forms: `quote`, `if`, `let`, `defun`, `condition-case`, etc.
//! These receive their arguments unevaluated (as a raw list).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::Interp;
use super::error::{EvalResult, Flow};
use super::eval::LexFrame;
use super::obarray::sym;
use super::value::{SymId, Value};

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
        sym::SAVE_MARK_AND_EXCURSION => sf_save_mark_and_excursion,
        sym::SAVE_CURRENT_BUFFER => sf_save_current_buffer,
        sym::WITH_CURRENT_BUFFER => sf_with_current_buffer,
        sym::SAVE_RESTRICTION => sf_save_restriction,
        sym::TRACK_MOUSE => sf_progn,
        sym::BACKQUOTE => sf_backquote,
        _ => return None,
    })
}

/// Minimum required args for a special form, for `func-arity'.
/// Emacs reports `(min . unevalled)' — e.g. (func-arity 'if) = (2 . unevalled).
pub fn special_form_min_args(id: SymId) -> u16 {
    match id {
        sym::QUOTE
        | sym::FUNCTION
        | sym::PROG1
        | sym::DEFVAR
        | sym::DEFCONST
        | sym::LAMBDA
        | sym::WHILE
        | sym::CATCH
        | sym::UNWIND_PROTECT
        | sym::LET
        | sym::LET_STAR
        | sym::BACKQUOTE => 1,
        sym::IF | sym::PROG2 | sym::DEFUN | sym::DEFMACRO | sym::CONDITION_CASE => 2,
        _ => 0,
    }
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
    /// Is lexical binding currently in effect?  GNU's
    /// `internal-interpreter-environment' alone decides this — a non-nil
    /// env (including the `(t)' sentinel) means lexical context.  The
    /// `lexical-binding' variable is only sampled by eval entry points
    /// (`load', `eval-buffer', `eval'), which install the env for the
    /// whole dynamic extent; `(setq lexical-binding ...)' mid-eval does
    /// not flip the binding mode.
    pub fn lexical_binding_active(&self) -> bool {
        self.lexenv.is_some()
    }

    /// Lexical env captured by a newly created `lambda'/`defun'/`defmacro'.
    /// GNU's `Ffunction' captures `internal-interpreter-environment'
    /// directly: non-nil env → lexical closure; nil → dynamic function.
    /// Eval entry points install a `(t)' root env so file-toplevel
    /// definitions capture a real (empty) lexical scope.
    pub fn lambda_env(&self) -> crate::lisp::eval::LexEnv {
        self.lexenv.clone()
    }

    /// `let`/`let*`/`condition-case` variable binding honoring scoping:
    /// binds lexically when lexical-binding is active and the var isn't
    /// special, else specbinds dynamically.  Like GNU's `Flet', a var is
    /// special when `declared_special' OR named by a scoped `defvar'
    /// marker in the current interpreter environment (`memq' check).
    pub fn bind_var(
        &mut self,
        lex_vars: Option<&Rc<LexFrame>>,
        sym: SymId,
        val: Value,
    ) -> Result<(), Flow> {
        let is_special = self.obarray.symbol(sym).special
            || crate::lisp::eval::lexenv_declared(&self.lexenv, sym);
        match (lex_vars, is_special) {
            (Some(frame), false) => {
                frame.vars.borrow_mut().insert(sym, val);
                Ok(())
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
        // Emacs: #'sym returns the symbol itself (function position is
        // resolved at call time), like quote.
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

/// Validate a let-style VARLIST, returning (VAR . VALUEFORM) pairs.
/// VAR is returned unchecked: GNU defers the `symbolp' test until bind
/// time so that value-forms evaluate before it fires.
fn parse_let_specs(i: &mut Interp, specs: &Value) -> Result<Vec<(Value, Value)>, Flow> {
    match specs {
        Value::Nil => return Ok(Vec::new()),
        Value::Cons(_) => {}
        _ => return Err(i.wrong_type_mut("listp", specs)),
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
                if matches!(spec, Value::Sym(_)) {
                    out.push((spec, Value::Nil));
                } else {
                    // GNU: (cdr (cdr SPEC)) must be nil; the value-form
                    // is (car (cdr SPEC)).
                    let tail = let_cdr(i, &spec)?;
                    if !let_cdr(i, &tail)?.is_nil() {
                        return Err(i.signal_data(
                            sym::ERROR,
                            vec![
                                Value::string("`let' bindings can have only one value-form"),
                                spec.clone(),
                            ],
                        ));
                    }
                    out.push((let_car(i, &spec)?, let_car(i, &tail)?));
                }
                cur = next;
            }
            _ => return Err(i.wrong_type_mut("listp", &cur)),
        }
    }
}

/// `car'/`cdr' with GNU error behavior: nil → nil, other non-cons
/// atoms → wrong-type-argument (listp V).
fn let_car(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

fn let_cdr(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

/// The variable of a let binding spec, checked GNU-late: the `symbolp'
/// test runs at bind time so value-forms evaluate first.
fn let_var(i: &mut Interp, v: &Value) -> Result<SymId, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
}

fn sf_let(i: &mut Interp, args: Value) -> EvalResult {
    let specs = parse_let_specs(i, &car(&args))?;
    let body = cdr(&args);
    // GNU evaluates all value-forms first, then checks/binds the
    // variables (parallel binding).
    let mut evaluated = Vec::with_capacity(specs.len());
    for (var, init) in &specs {
        evaluated.push((var.clone(), i.eval(init)?));
    }
    if i.lexical_binding_active() {
        let frame = Rc::new(LexFrame {
            vars: RefCell::new(HashMap::new()),
            declared: RefCell::new(std::collections::HashSet::new()),
            parent: i.lexenv.clone(),
        });
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        for (v, val) in evaluated {
            r = match let_var(i, &v) {
                Ok(s) => i.bind_var(Some(&frame), s, val).map(|_| Value::Nil),
                Err(e) => Err(e),
            };
            if r.is_err() {
                break;
            }
        }
        let saved = std::mem::replace(&mut i.lexenv, Some(frame));
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.lexenv = saved;
        i.unbind_to(mark)?;
        r
    } else {
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        for (v, val) in evaluated {
            r = match let_var(i, &v) {
                Ok(s) => i.specbind(s, val).map(|_| Value::Nil),
                Err(e) => Err(e),
            };
            if r.is_err() {
                break;
            }
        }
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.unbind_to(mark)?;
        r
    }
}

fn sf_let_star(i: &mut Interp, args: Value) -> EvalResult {
    let specs = parse_let_specs(i, &car(&args))?;
    let body = cdr(&args);
    if i.lexical_binding_active() {
        let frame = Rc::new(LexFrame {
            vars: RefCell::new(HashMap::new()),
            declared: RefCell::new(std::collections::HashSet::new()),
            parent: i.lexenv.clone(),
        });
        let mark = i.specbind_depth();
        let saved = std::mem::replace(&mut i.lexenv, Some(frame.clone()));
        let mut r = Ok(Value::Nil);
        for (var, init) in &specs {
            r = i.eval(init).and_then(|v| {
                let_var(i, var)
                    .and_then(|s| i.bind_var(Some(&frame), s, v))
                    .map(|_| Value::Nil)
            });
            if r.is_err() {
                break;
            }
        }
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.lexenv = saved;
        i.unbind_to(mark)?;
        r
    } else {
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        for (var, init) in &specs {
            r = i
                .eval(init)
                .and_then(|v| let_var(i, var).and_then(|s| i.specbind(s, v)))
                .map(|_| Value::Nil);
            if r.is_err() {
                break;
            }
        }
        if r.is_ok() {
            r = i.eval_progn(&body);
        }
        i.unbind_to(mark)?;
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
                let sid = i
                    .sym_id(&sym_v)
                    .ok_or_else(|| i.wrong_type_mut("symbolp", &sym_v))?;
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
    let init = cadr(&args);
    let has_init = !cdr(&args).is_nil();
    if has_init {
        // `(defvar SYM INIT ...)': GNU marks the symbol permanently
        // special (`declared_special') and installs the default only if
        // the var is currently void.
        i.obarray.symbol_mut(sid).special = true;
        if !i.bound_p(sid) {
            let v = i.eval(&init)?;
            i.set_symbol_default(sid, v)?;
        }
    } else if i.lexenv.is_some() {
        // Bare `(defvar SYM)' in a lexical context is a scoped special
        // declaration: GNU pushes the bare symbol onto
        // `internal-interpreter-environment', so `let'/`let*' in this
        // scope bind SYM dynamically and the declaration unwinds with
        // the scope (it does NOT set `declared_special').
        if !i.obarray.symbol(sid).special {
            crate::lisp::eval::lexenv_declare(&i.lexenv, sid);
        }
    }
    // Dynamic context (nil interpreter environment): bare `(defvar SYM)'
    // "does nothing", per GNU's docstring — all bindings are dynamic
    // anyway, and the var does not become `special-variable-p'.
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
    // defconst may (re)define a constant: bypass the constant check.
    i.obarray.symbol_mut(sid).constant = false;
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
    lambda.env = i.lambda_env();
    i.fset_defalias(sid, Value::Lambda(Rc::new(lambda)))?;
    eval_defun_declarations(i, &name_v, &params, &body, false)?;
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
    lambda.env = i.lambda_env();
    i.fset_defalias(sid, Value::Lambda(Rc::new(lambda)))?;
    eval_defun_declarations(i, &name_v, &params, &body, true)?;
    Ok(name_v)
}

/// GNU `macroexp--defun-declarations': each `(declare (PROP . ARGS))'
/// spec in BODY dispatches to the handler bound in
/// `defun-declarations-alist' (or `macro-declarations-alist' for
/// defmacro), called as (FN NAME ARGLIST . ARGS); the form it returns
/// is evaluated.  Unknown PROP specs are ignored.
fn eval_defun_declarations(
    i: &mut Interp,
    name_v: &Value,
    params: &Value,
    body: &[Value],
    is_macro: bool,
) -> EvalResult {
    let alist_name = if is_macro {
        "macro-declarations-alist"
    } else {
        "defun-declarations-alist"
    };
    let alist_id = match i.intern_soft(alist_name) {
        Some(id) => id,
        None => return Ok(Value::Nil),
    };
    let alist = i.symbol_value(alist_id);
    if !matches!(alist, Value::Cons(_)) {
        return Ok(Value::Nil);
    }
    let declare_id = i.intern("declare");
    // Scan past the optional docstring, then collect `declare' forms.
    let mut start = 0;
    if matches!(body.first(), Some(Value::Str(_))) && body.len() > 1 {
        start = 1;
    }
    let mut extra_forms: Vec<Value> = Vec::new();
    for f in body.iter().skip(start) {
        let is_decl = match f {
            Value::Cons(c) => {
                let b = c.borrow();
                i.sym_is(&b.car, declare_id)
            }
            _ => false,
        };
        if !is_decl {
            break;
        }
        let specs = match cdr(f).list_to_vec() {
            Ok(v) => v,
            Err(_) => continue,
        };
        for spec in specs {
            let (prop, dargs) = match &spec {
                Value::Cons(c) => {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                }
                _ => continue,
            };
            // assq over the handler alist.
            let mut cur = alist.clone();
            let handler = loop {
                match cur {
                    Value::Cons(c) => {
                        let (entry, rest) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        let hit = match &entry {
                            Value::Cons(e) => {
                                let b = e.borrow();
                                crate::lisp::eq_values(&b.car, &prop)
                            }
                            _ => false,
                        };
                        if hit {
                            break match &entry {
                                Value::Cons(e) => {
                                    let b = e.borrow();
                                    match &b.cdr {
                                        Value::Cons(c2) => Some(c2.borrow().car.clone()),
                                        other => Some(other.clone()),
                                    }
                                }
                                _ => None,
                            };
                        }
                        cur = rest;
                    }
                    _ => break None,
                }
            };
            if let Some(h) = handler {
                let mut call = vec![name_v.clone(), params.clone()];
                call.extend(dargs.list_to_vec().unwrap_or_default());
                let form = i.apply(&h, call)?;
                // GNU splices the returned form's progn into the
                // expansion; evaluating the form is equivalent.
                extra_forms.push(form);
            }
        }
    }
    for form in extra_forms {
        i.eval(&form)?;
    }
    Ok(Value::Nil)
}

fn sf_lambda(i: &mut Interp, args: Value) -> EvalResult {
    // `(lambda ...)' self-evaluates to a function object; under
    // lexical-binding it closes over the current lexical env.
    let params = car(&args);
    let body_v = cdr(&args);
    let body = body_v.list_to_vec().unwrap_or_default();
    let mut lambda = i.parse_lambda(&params, &body, None)?;
    lambda.env = i.lambda_env();
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
    // Emacs: a nil tag catch acts like progn — `(throw nil ...)' finds
    // no catch and signals no-catch.
    if tag.is_nil() {
        return i.eval_progn(&body);
    }
    i.catch_tags.push(tag.clone());
    let r = i.eval_progn(&body);
    i.catch_tags.pop();
    match r {
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
    // Register our condition names so a signal raised in BODY knows it
    // will be caught — `handler-bind' handlers only fire for signals
    // no enclosing `condition-case' claims (Emacs debugger semantics).
    let mut cond_names = Vec::new();
    handlers.each_car(|h| cond_names.push(car(h)));
    i.case_handlers.push(Value::list(cond_names));
    let body_result = i.eval(&bodyform);
    i.case_handlers.pop();
    match body_result {
        Ok(v) => Ok(v),
        Err(Flow::Signal(sig, data, offered)) => {
            // Build the condition object: (sig . data)
            let err_val = Value::cons(sig.clone(), data.clone());
            let mut cur = handlers;
            loop {
                match cur {
                    Value::Nil => return Err(Flow::Signal(sig, data, offered)),
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
                                    declared: RefCell::new(std::collections::HashSet::new()),
                                    parent: i.lexenv.clone(),
                                }))
                            } else {
                                None
                            };
                            let saved_lex = match &lex_frame {
                                Some(f) => std::mem::replace(&mut i.lexenv, Some(f.clone())),
                                None => None,
                            };
                            // GNU binds VAR only when it is non-nil.
                            if let Some(vid) = i.sym_id(&var_v).filter(|_| !var_v.is_nil()) {
                                if let Err(e) = i.bind_var(lex_frame.as_ref(), vid, err_val) {
                                    if lex_frame.is_some() {
                                        i.lexenv = saved_lex;
                                    }
                                    let _ = i.unbind_to(mark);
                                    return Err(e);
                                }
                            }
                            let r = i.eval_progn(&hbody);
                            if lex_frame.is_some() {
                                i.lexenv = saved_lex;
                            }
                            i.unbind_to(mark)?;
                            return r;
                        }
                        cur = next;
                    }
                    _ => return Err(Flow::Signal(sig, data, offered)),
                }
            }
        }
        other => other,
    }
}

// ---------- buffer-related special forms ----------

fn sf_save_excursion(i: &mut Interp, args: Value) -> EvalResult {
    let saved = i.save_excursion_state(false);
    let r = i.eval_progn(&args);
    i.restore_excursion_state(saved);
    r
}

fn sf_save_mark_and_excursion(i: &mut Interp, args: Value) -> EvalResult {
    let saved = i.save_excursion_state(true);
    let r = i.eval_progn(&args);
    i.restore_excursion_state(saved);
    r
}

fn sf_save_current_buffer(i: &mut Interp, args: Value) -> EvalResult {
    let old = i.current_buffer;
    let r = i.eval_progn(&args);
    // GNU: set_buffer_if_live — restore only if the buffer survives.
    if i.buffer_live(old) {
        i.set_current_buffer(old);
    }
    r
}

fn sf_with_current_buffer(i: &mut Interp, args: Value) -> EvalResult {
    let buf_v = i.eval(&car(&args))?;
    // GNU's set-buffer/get_buffer: a buffer or its name string;
    // anything else is a `stringp' type error.
    if !matches!(buf_v, Value::Buffer(_) | Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &buf_v));
    }
    let buf_id = i
        .buffer_id_of(&buf_v)
        .ok_or_else(|| i.error(format!("No buffer named {}", i.princ_to_string(&buf_v))))?;
    // GNU's Fset_buffer signals on killed buffers.
    if !i.buffer_live(buf_id) {
        return Err(i.error("Selecting deleted buffer"));
    }
    let old = i.current_buffer;
    i.set_current_buffer(buf_id);
    let body = cdr(&args);
    let r = i.eval_progn(&body);
    if i.buffer_live(old) {
        i.set_current_buffer(old);
    }
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
    // The expansion algorithm is a faithful Lisp port of GNU's
    // emacs-lisp/backquote.el (backquote-process/backquote-listify/
    // backquote-list*) living in the prelude — call it like GNU's
    // `backquote' macro does: (cdr (backquote-process STRUCTURE)).
    let proc = Value::Sym(i.intern("backquote-process"));
    let tagged = i.apply(&proc, vec![car(&args)])?;
    i.eval(&cdr(&tagged))
}
