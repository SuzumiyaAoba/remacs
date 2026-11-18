//! Special forms: `quote`, `if`, `let`, `defun`, `condition-case`, etc.
//! These receive their arguments unevaluated (as a raw list).

use rustc_hash::FxHashSet;
use std::rc::Rc;

use super::Interp;
use super::error::{EvalResult, Flow};
use super::eval::{LexFrame, cons_parts};
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
        // `track-mouse' is a Lisp macro in GNU 31.1 (subr.el), not a
        // special form - kept out so `special-form-p' returns nil.
        sym::PROG1 => sf_prog1,
        sym::PROG2 => sf_prog2,
        sym::AND => sf_and,
        sym::OR => sf_or,
        sym::LET => sf_let,
        sym::LET_STAR => sf_let_star,
        sym::SETQ => sf_setq,
        // `setq-default' is NOT a special form in GNU: subr.el defines
        // it as a macro over `set-default' (prelude does the same).
        sym::DEFVAR => sf_defvar,
        sym::DEFCONST => sf_defconst,
        // eval-side dispatch only: `defun'/`defmacro'/`lambda' carry
        // MACRO cells in GNU (fboundp sees them) — never registered as
        // Unevalled subrs, so special-form-p/indirect-function stay
        // nil on the symbol surface.
        sym::DEFUN => sf_defun,
        sym::DEFMACRO => sf_defmacro,
        sym::LAMBDA => sf_lambda,
        sym::WHILE => sf_while,
        sym::CATCH => sf_catch,
        sym::UNWIND_PROTECT => sf_unwind_protect,
        sym::CONDITION_CASE => sf_condition_case,
        sym::INTERACTIVE => |_i, _a| Ok(Value::Nil),
        sym::SAVE_EXCURSION => sf_save_excursion,
        // eval-side dispatch for names GNU keeps as Lisp macros.
        sym::SAVE_MARK_AND_EXCURSION => sf_save_mark_and_excursion,
        sym::SAVE_CURRENT_BUFFER => sf_save_current_buffer,
        sym::WITH_CURRENT_BUFFER => sf_with_current_buffer,
        sym::SAVE_RESTRICTION => sf_save_restriction,
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
        | sym::LAMBDA
        | sym::WHILE
        | sym::CATCH
        | sym::UNWIND_PROTECT
        | sym::LET
        | sym::LET_STAR
        | sym::BACKQUOTE => 1,
        sym::IF
        | sym::PROG2
        | sym::DEFUN
        | sym::DEFMACRO
        | sym::CONDITION_CASE
        // GNU DEFUN's min covers DEFCONST too (SYMBOL INITVALUE).
        | sym::DEFCONST => 2,
        _ => 0,
    }
}

// ---------- arg access helpers ----------

fn car(i: &Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.car.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed cons cell")),
        _ => Ok(Value::Nil),
    }
}

fn cdr(i: &Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.cdr.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed cons cell")),
        _ => Ok(Value::Nil),
    }
}

fn cadr(i: &Interp, v: &Value) -> Result<Value, Flow> {
    car(i, &cdr(i, v)?)
}

fn nth_arg(i: &Interp, v: &Value, n: usize) -> Result<Value, Flow> {
    let mut cur = v.clone();
    for _ in 0..n {
        cur = cdr(i, &cur)?;
    }
    car(i, &cur)
}

fn cons_site(cell: &Rc<std::cell::RefCell<crate::lisp::value::Cons>>) -> u128 {
    (Rc::as_ptr(cell) as usize as u128) << 1
}

fn argument_site(args: &Value, index: usize) -> Option<u128> {
    let mut cur = args.clone();
    for _ in 0..index {
        cur = match cur {
            Value::Cons(cell) => cell.try_borrow().ok()?.cdr.clone(),
            _ => Value::Nil,
        };
    }
    match cur {
        Value::Cons(cell) => Some(cons_site(&cell)),
        _ => None,
    }
}

fn eval_at_argument(i: &mut Interp, form: &Value, args: &Value, index: usize) -> EvalResult {
    match argument_site(args, index) {
        Some(site) => i.eval_at_compiled_site(form, site),
        None => i.eval(form),
    }
}

fn eval_argument(i: &mut Interp, args: &Value, index: usize) -> EvalResult {
    let form = nth_arg(i, args, index)?;
    eval_at_argument(i, &form, args, index)
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
        crate::lisp::eval::lexenv_from_value(&crate::lisp::eval::lexenv_as_value(&self.lexenv))
    }

    pub(crate) fn lambda_env_checked(&mut self) -> Result<crate::lisp::eval::LexEnv, Flow> {
        let raw = crate::lisp::eval::lexenv_as_value_checked(&self.lexenv);
        let raw = raw.map_err(|error| error.into_flow(self))?;
        Ok(crate::lisp::eval::lexenv_from_value(&raw))
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
        // Constants (nil/t/keywords) can never be bound: GNU's
        // let_bind reaches specbind's `trapped_write == NOWRITE'
        // check in both dynamic and lexical eval and signals
        // `setting-constant'.
        if self.obarray.symbol(sym).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(sym)]));
        }
        let is_special = self.obarray.symbol(sym).special
            || crate::lisp::eval::lexenv_declared(&self.lexenv, sym)
                .map_err(|e| e.into_flow(self))?
            || self.file_declared.borrow().contains(&sym);
        match (lex_vars, is_special) {
            (Some(frame), false) => {
                crate::lisp::eval::lexframe_bind_checked(frame, sym, val)
                    .map_err(|error| error.into_flow(self))?;
                Ok(())
            }
            _ => self.specbind(sym, val),
        }
    }
}

// ---------- the special forms ----------

fn sf_quote(i: &mut Interp, args: Value) -> EvalResult {
    // GNU Fquote: `wrong-number-of-arguments' on >1 operand; the
    // reported count is Flength of the unevalled arg list (like
    // Ffunction), not the literal 2.
    if cdr(i, &args)?.truthy() {
        let n = args.list_to_vec().map(|v| v.len() as i128).unwrap_or(0);
        return Err(i.wrong_number_of_args(&Value::Sym(sym::QUOTE), n));
    }
    car(i, &args)
}

fn sf_function(i: &mut Interp, args: Value) -> EvalResult {
    // GNU Ffunction: `wrong-number-of-arguments' on >1 operand —
    // reported count is Flength of the UNEVALLED arg list.
    if cdr(i, &args)?.truthy() {
        let n = args.list_to_vec().map(|v| v.len() as i128).unwrap_or(0);
        return Err(i.wrong_number_of_args(&Value::Sym(sym::FUNCTION), n));
    }
    let arg = car(i, &args)?;
    match &arg {
        Value::Cons(_) => {
            let head = car(i, &arg)?;
            if i.sym_is(&head, sym::LAMBDA) {
                return i.function_from_lambda(&cdr(i, &arg)?);
            }
            Ok(arg)
        }
        // Emacs: #'sym returns the symbol itself (function position is
        // resolved at call time), like quote.
        _ => Ok(arg),
    }
}

fn sf_if(i: &mut Interp, args: Value) -> EvalResult {
    let cond = eval_argument(i, &args, 0)?;
    if cond.truthy() {
        eval_argument(i, &args, 1)
    } else {
        // else-part is a progn.
        i.eval_progn(&cdr(i, &cdr(i, &args)?)?)
    }
}

fn sf_cond(i: &mut Interp, args: Value) -> EvalResult {
    reject_circular_spine(i, &args)?;
    // Read the next cdr after evaluating the test, matching GNU's eval
    // order. Keep the current cell's Rc alive across eval: a test can
    // unlink this cell by mutating an earlier clause-list cell.
    let mut cur = args;
    let mut seen = FxHashSet::default();
    let mut until_quit = 1024usize;
    loop {
        match cur.clone() {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                }
                let clause = cons_parts(i, &cell)?.0;
                // GNU's Fcond XCARs each clause: a non-list clause
                // signals `wrong-type-argument' (listp CLAUSE).
                if !matches!(clause, Value::Cons(_) | Value::Nil) {
                    return Err(i.wrong_type_mut("listp", &clause));
                }
                let test = match &clause {
                    Value::Cons(clause) => cons_parts(i, clause)?.0,
                    Value::Nil => Value::Nil,
                    _ => unreachable!("cond clause type was checked above"),
                };
                let v = i.eval_at_compiled_site(&test, cons_site(&cell))?;
                if v.truthy() {
                    // GNU evaluates the test before reading the selected
                    // clause's body. The test may mutate or remove that
                    // body, so retain the clause object and fetch its cdr
                    // only after the test returns.
                    let body = match &clause {
                        Value::Cons(clause) => cons_parts(i, clause)?.1,
                        Value::Nil => Value::Nil,
                        _ => unreachable!("cond clause type was checked above"),
                    };
                    if body.is_nil() {
                        return Ok(v);
                    }
                    return i.eval_progn(&body);
                }
                cur = cons_parts(i, &cell)?.1;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            // Fcond stops when a previously proper argument spine is
            // changed to an atom while a test is running. The dispatcher
            // still rejects an initially dotted call before entering here.
            _ => return Ok(Value::Nil),
        }
    }
}

fn sf_progn(i: &mut Interp, args: Value) -> EvalResult {
    i.eval_progn(&args)
}

fn sf_prog1(i: &mut Interp, args: Value) -> EvalResult {
    let first = eval_argument(i, &args, 0)?;
    i.eval_progn(&cdr(i, &args)?)?;
    Ok(first)
}

fn sf_prog2(i: &mut Interp, args: Value) -> EvalResult {
    eval_argument(i, &args, 0)?;
    let second = eval_argument(i, &args, 1)?;
    i.eval_progn(&cdr(i, &cdr(i, &args)?)?)?;
    Ok(second)
}

fn sf_and(i: &mut Interp, args: Value) -> EvalResult {
    reject_circular_spine(i, &args)?;
    let mut cur = args;
    let mut last = Value::t();
    let mut seen = FxHashSet::default();
    let mut until_quit = 1024usize;
    loop {
        match cur.clone() {
            Value::Nil => return Ok(last),
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                }
                // GNU saves the next cons before evaluating this form.
                // Mutating this cell's cdr cannot insert or replace the
                // next argument, though mutating that saved cell's car
                // remains visible when it is reached.
                let (form, next) = cons_parts(i, &cell)?;
                last = i.eval_at_compiled_site(&form, cons_site(&cell))?;
                if last.is_nil() {
                    return Ok(Value::Nil);
                }
                cur = next;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            // GNU's list walk stops at an atom tail introduced by an
            // earlier operand; initially dotted calls are rejected by the
            // evaluator before this special form starts.
            _ => return Ok(last),
        }
    }
}

fn sf_or(i: &mut Interp, args: Value) -> EvalResult {
    reject_circular_spine(i, &args)?;
    let mut cur = args;
    let mut seen = FxHashSet::default();
    let mut until_quit = 1024usize;
    loop {
        match cur.clone() {
            Value::Nil => return Ok(Value::Nil),
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                }
                // Match `and'/'progn': freeze the next cons link before
                // evaluating the current form, but read its car later.
                let (form, next) = cons_parts(i, &cell)?;
                let v = i.eval_at_compiled_site(&form, cons_site(&cell))?;
                if v.truthy() {
                    return Ok(v);
                }
                cur = next;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            _ => return Ok(Value::Nil),
        }
    }
}

/// Validate and read one let-style binding specification.
/// VAR is returned unchecked: GNU defers the `symbolp' test until bind
/// time so that value-forms evaluate before it fires. A bare variable has
/// no value-form: unlike `(VAR)', its implicit nil must not be evaluated.
fn parse_let_spec(i: &mut Interp, spec: &Value) -> Result<(Value, Option<Value>), Flow> {
    if i.sym_id(spec).is_some() {
        // A bare symbol spec (possibly a positioned symbol) binds to nil.
        return Ok((spec.clone(), None));
    }

    // GNU: (cdr (cdr SPEC)) must be nil; the value-form is (car (cdr SPEC)).
    let tail = let_cdr(i, spec)?;
    if !let_cdr(i, &tail)?.is_nil() {
        // GNU's signal data splices the spec's elements: `(error MSG + x 1)'.
        let mut data = vec![Value::string("`let' bindings can have only one value-form")];
        match spec.list_to_vec() {
            Ok(elements) => data.extend(elements),
            Err(crate::lisp::value::ListError::Dotted(_)) => {
                spec.each_car(|e| data.push(e.clone()));
            }
            // GNU includes the offending cyclic spec as one datum rather
            // than looping while splicing its cars into the error list.
            Err(crate::lisp::value::ListError::Circular(_)) => data.push(spec.clone()),
            Err(crate::lisp::value::ListError::Borrowed) => {
                return Err(i.error("Cannot inspect a mutably borrowed binding list"));
            }
        }
        return Err(i.signal_data(sym::ERROR, data));
    }
    Ok((let_car(i, spec)?, Some(let_car(i, &tail)?)))
}

/// Read the current variable name from a saved binding-list cell.
/// Parallel `let' evaluates every initializer before it installs bindings,
/// so earlier or later initializers can mutate the names in the list.
fn let_spec_var(i: &mut Interp, spec: &Value) -> Result<Value, Flow> {
    if i.sym_id(spec).is_some() {
        Ok(spec.clone())
    } else {
        let_car(i, spec)
    }
}

fn eval_let_init(i: &mut Interp, init: &Option<Value>, site: u128) -> EvalResult {
    match init {
        Some(form) => i.eval_at_compiled_site(form, site),
        None => Ok(Value::Nil),
    }
}

/// `car'/`cdr' with GNU error behavior: nil → nil, other non-cons
/// atoms → wrong-type-argument (listp V).
fn let_car(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.car.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed binding spec")),
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

fn let_cdr(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(c) => c
            .try_borrow()
            .map(|cell| cell.cdr.clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed binding spec")),
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

/// GNU validates these special forms' argument spines before evaluating
/// their first form. Preserve dotted-tail handling in the caller while
/// rejecting circular spines up front.
fn reject_circular_spine(i: &mut Interp, list: &Value) -> Result<(), Flow> {
    let mut seen = FxHashSet::default();
    let mut tail = list.clone();
    let mut until_quit = 1024usize;
    while let Value::Cons(cell) = tail.clone() {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail.clone()]));
        }
        tail = cdr(i, &tail)?;
        until_quit -= 1;
        if until_quit == 0 {
            i.maybe_quit()?;
            until_quit = 1024;
        }
    }
    Ok(())
}

/// The variable of a let binding spec, checked GNU-late: the `symbolp'
/// test runs at bind time so value-forms evaluate first.
fn let_var(i: &mut Interp, v: &Value) -> Result<SymId, Flow> {
    i.sym_id(v).ok_or_else(|| i.wrong_type_mut("symbolp", v))
}

fn sf_let(i: &mut Interp, args: Value) -> EvalResult {
    let mut cur = car(i, &args)?;
    // GNU detects an already-circular binding list before evaluating any
    // initializer. Initializers can mutate the live spine, so the main
    // walk also remembers cells and checks for cycles introduced later.
    let mut seen = FxHashSet::default();
    let mut scan = cur.clone();
    let mut scan_steps = 0usize;
    while let Value::Cons(cell) = scan.clone() {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![scan.clone()]));
        }
        scan_steps = scan_steps.wrapping_add(1);
        if scan_steps & 1023 == 0 {
            i.maybe_quit()?;
        }
        scan = cdr(i, &scan)?;
    }
    seen.clear();
    // GNU walks the binding list as it evaluates each initializer. The
    // current binding's value-form is captured before evaluation, but the
    // next cdr is read after it, so edits to later specs remain visible.
    // Keep each outer binding cell until the parallel binding phase: GNU
    // reads its variable name only after all initializers have run.
    let mut evaluated = Vec::new();
    let mut until_quit = 1024usize;
    loop {
        match cur.clone() {
            Value::Nil => break,
            Value::Cons(cell) => {
                if !seen.insert(Rc::as_ptr(&cell) as usize) {
                    return Err(i.signal_data(sym::CIRCULAR_LIST, vec![Value::Cons(cell.clone())]));
                }
                let spec = car(i, &Value::Cons(cell.clone()))?;
                let (_, init) = parse_let_spec(i, &spec)?;
                let value = eval_let_init(i, &init, cons_site(&cell))?;
                evaluated.push((Value::Cons(cell.clone()), value));
                cur = cdr(i, &Value::Cons(cell.clone()))?;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            other => return Err(i.wrong_type_mut("listp", &other)),
        }
    }
    if i.lexical_binding_active() {
        let frame = crate::lisp::eval::lexenv_extend_checked(&i.lexenv)
            .map_err(|error| error.into_flow(i))?;
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        let mut until_quit = 1024usize;
        for (binding, val) in evaluated {
            let spec = match car(i, &binding) {
                Ok(spec) => spec,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            let v = match let_spec_var(i, &spec) {
                Ok(v) => v,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            if i.dbg_flags & 0x100 != 0 && let_var(i, &v).map(|s| s == sym::NIL).unwrap_or(false) {
                eprintln!(
                    "[let-nil-lex] spec={:?} val={:?}",
                    i.princ_to_string(&v),
                    i.princ_to_string(&val)
                );
            }
            r = match let_var(i, &v) {
                Ok(s) => i.bind_var(Some(&frame), s, val).map(|_| Value::Nil),
                Err(e) => Err(e),
            };
            if r.is_err() {
                break;
            }
            until_quit -= 1;
            if until_quit == 0 {
                if let Err(error) = i.maybe_quit() {
                    r = Err(error);
                    break;
                }
                until_quit = 1024;
            }
        }
        r = i.offer_flow(r);
        let saved = std::mem::replace(&mut i.lexenv, Some(frame));
        if r.is_ok() {
            // Read the body after binding: initializer forms can mutate the
            // active call's argument spine before GNU reaches Fprogn.
            r = match cdr(i, &args) {
                Ok(body) => i.eval_progn(&body),
                Err(error) => Err(error),
            };
        }
        r = i.offer_flow(r);
        i.lexenv = saved;
        i.unbind_to(mark)?;
        r
    } else {
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        let mut until_quit = 1024usize;
        for (binding, val) in evaluated {
            let spec = match car(i, &binding) {
                Ok(spec) => spec,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            let v = match let_spec_var(i, &spec) {
                Ok(v) => v,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            if i.dbg_flags & 0x100 != 0 && let_var(i, &v).map(|s| s == sym::NIL).unwrap_or(false) {
                eprintln!(
                    "[let-nil] spec={:?} val={:?}",
                    i.princ_to_string(&v),
                    i.princ_to_string(&val)
                );
            }
            r = match let_var(i, &v) {
                Ok(s) => i.specbind(s, val).map(|_| Value::Nil),
                Err(e) => Err(e),
            };
            if r.is_err() {
                break;
            }
            until_quit -= 1;
            if until_quit == 0 {
                if let Err(error) = i.maybe_quit() {
                    r = Err(error);
                    break;
                }
                until_quit = 1024;
            }
        }
        if r.is_ok() {
            r = match cdr(i, &args) {
                Ok(body) => i.eval_progn(&body),
                Err(error) => Err(error),
            };
        }
        r = i.offer_flow(r);
        i.unbind_to(mark)?;
        r
    }
}

fn sf_let_star(i: &mut Interp, args: Value) -> EvalResult {
    let mut cur = car(i, &args)?;
    if i.lexical_binding_active() {
        let frame = crate::lisp::eval::lexenv_extend_checked(&i.lexenv)
            .map_err(|error| error.into_flow(i))?;
        let mark = i.specbind_depth();
        let saved = std::mem::replace(&mut i.lexenv, Some(frame.clone()));
        let mut r = Ok(Value::Nil);
        let mut seen_bindings = FxHashSet::default();
        let mut until_quit = 1024usize;
        loop {
            let cell = match cur.clone() {
                Value::Nil => break,
                Value::Cons(cell) => {
                    if !seen_bindings.insert(Rc::as_ptr(&cell) as usize) {
                        r = Err(i.signal_data(sym::CIRCULAR_LIST, vec![Value::Cons(cell.clone())]));
                        break;
                    }
                    cell
                }
                other => {
                    r = Err(i.wrong_type_mut("listp", &other));
                    break;
                }
            };
            let spec = match car(i, &Value::Cons(cell.clone())) {
                Ok(spec) => spec,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            let (var, init) = match parse_let_spec(i, &spec) {
                Ok(binding) => binding,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            let value = match eval_let_init(i, &init, cons_site(&cell)) {
                Ok(value) => value,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            r = let_var(i, &var)
                .and_then(|s| i.bind_var(Some(&frame), s, value))
                .map(|_| Value::Nil);
            if r.is_err() {
                break;
            }
            cur = match cdr(i, &Value::Cons(cell.clone())) {
                Ok(next) => next,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            until_quit -= 1;
            if until_quit == 0 {
                if let Err(error) = i.maybe_quit() {
                    r = Err(error);
                    break;
                }
                until_quit = 1024;
            }
        }
        if r.is_ok() {
            r = match cdr(i, &args) {
                Ok(body) => i.eval_progn(&body),
                Err(error) => Err(error),
            };
        }
        r = i.offer_flow(r);
        i.lexenv = saved;
        i.unbind_to(mark)?;
        r
    } else {
        let mark = i.specbind_depth();
        let mut r = Ok(Value::Nil);
        let mut seen_bindings = FxHashSet::default();
        let mut until_quit = 1024usize;
        loop {
            let cell = match cur.clone() {
                Value::Nil => break,
                Value::Cons(cell) => {
                    if !seen_bindings.insert(Rc::as_ptr(&cell) as usize) {
                        r = Err(i.signal_data(sym::CIRCULAR_LIST, vec![Value::Cons(cell.clone())]));
                        break;
                    }
                    cell
                }
                other => {
                    r = Err(i.wrong_type_mut("listp", &other));
                    break;
                }
            };
            let spec = match car(i, &Value::Cons(cell.clone())) {
                Ok(spec) => spec,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            let (var, init) = match parse_let_spec(i, &spec) {
                Ok(binding) => binding,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            let value = match eval_let_init(i, &init, cons_site(&cell)) {
                Ok(value) => value,
                Err(e) => {
                    r = Err(e);
                    break;
                }
            };
            r = let_var(i, &var)
                .and_then(|s| i.specbind(s, value))
                .map(|_| Value::Nil);
            if r.is_err() {
                break;
            }
            cur = match cdr(i, &Value::Cons(cell.clone())) {
                Ok(next) => next,
                Err(error) => {
                    r = Err(error);
                    break;
                }
            };
            until_quit -= 1;
            if until_quit == 0 {
                if let Err(error) = i.maybe_quit() {
                    r = Err(error);
                    break;
                }
                until_quit = 1024;
            }
        }
        if r.is_ok() {
            r = match cdr(i, &args) {
                Ok(body) => i.eval_progn(&body),
                Err(error) => Err(error),
            };
        }
        r = i.offer_flow(r);
        i.unbind_to(mark)?;
        r
    }
}

fn setq_pairs(i: &mut Interp, args: &Value, default: bool, who: SymId) -> EvalResult {
    let mut cur = args.clone();
    let mut seen = FxHashSet::default();
    let mut scan = cur.clone();
    let mut scan_until_quit = 1024usize;
    while let Value::Cons(cell) = scan.clone() {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![scan.clone()]));
        }
        scan = cdr(i, &scan)?;
        scan_until_quit -= 1;
        if scan_until_quit == 0 {
            i.maybe_quit()?;
            scan_until_quit = 1024;
        }
    }
    // RHS evaluation can mutate the remaining argument spine; continue
    // checking visited cells while processing the saved pair cursors.
    seen.clear();
    let mut last = Value::Nil;
    let mut n = 0i128;
    let mut until_quit = 1024usize;
    loop {
        match cur.clone() {
            Value::Nil => return Ok(last),
            Value::Cons(c) => {
                if !seen.insert(Rc::as_ptr(&c) as usize) {
                    return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                }
                let (sym_v, next1) = cons_parts(i, &c)?;
                n += 1;
                let (val_f, next2) = {
                    match &next1 {
                        Value::Cons(c2) => cons_parts(i, c2)?,
                        Value::Nil => (Value::Nil, Value::Nil),
                        other => {
                            // Dotted tail counts as one more element in
                            // GNU's arity report.
                            let _ = other;
                            n += 1;
                            return Err(i.wrong_number_of_args(&Value::Sym(who), n));
                        }
                    }
                };
                // GNU `Fsetq': a dangling symbol without a value form
                // signals `wrong-number-of-arguments' before any symbolp
                // check; there is no RHS to evaluate in this case.
                if matches!(next2, Value::Nil) && !matches!(next1, Value::Cons(_)) {
                    return Err(i.wrong_number_of_args(&Value::Sym(who), n));
                }
                n += 1;
                // GNU evaluates a paired RHS before validating its variable
                // name.  Thus `(setq 5 (setq side 'ran))' runs the RHS and
                // only then signals `(wrong-type-argument symbolp 5)'.
                last = i.eval_at_compiled_site(&val_f, cons_site(&c))?;
                let sid = i
                    .sym_id(&sym_v)
                    .ok_or_else(|| i.wrong_type_mut("symbolp", &sym_v))?;
                if default {
                    i.set_symbol_default(sid, last.clone())?;
                } else {
                    // GNU `set_internal': the lexical env is consulted
                    // via `Fassq' unconditionally — declared_special is
                    // ignored (a closure parameter that is special still
                    // lives in the env frame; `funcall_lambda' pushes
                    // ALL params there).  Gating on `special' here wrote
                    // the global cell while reads resolved the param —
                    // e.g. a defun arg named `values' lost its setq.
                    let done = crate::lisp::eval::lexenv_set(&i.lexenv, sid, last.clone())
                        .map_err(|e| e.into_flow(i))?;
                    if !done {
                        i.set_symbol(sid, last.clone())?;
                    }
                }
                cur = next2;
                until_quit -= 1;
                if until_quit == 0 {
                    i.maybe_quit()?;
                    until_quit = 1024;
                }
            }
            // GNU stops after the last complete variable/value pair if an
            // RHS changes the remaining spine to an atom. An initially
            // dotted call is rejected by the evaluator before `setq' runs.
            _ => return Ok(last),
        }
    }
}

fn sf_setq(i: &mut Interp, args: Value) -> EvalResult {
    setq_pairs(i, &args, false, sym::SETQ)
}

fn sf_defvar(i: &mut Interp, args: Value) -> EvalResult {
    // GNU eval.c: (defvar NAME [INIT [DOCSTRING]]) — a 4th argument
    // signals `(error "Too many arguments")'.
    if !cdr(i, &cdr(i, &cdr(i, &args)?)?)?.is_nil() {
        return Err(i.error("Too many arguments".to_string()));
    }
    let name_v = car(i, &args)?;
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    // GNU's defvar operates through `indirect-variable'.
    let sid = i.var_alias_target(sid);
    let init = cadr(i, &args)?;
    let has_init = !cdr(i, &args)?.is_nil();
    if has_init {
        // `(defvar SYM INIT ...)': GNU marks the symbol permanently
        // special (`declared_special') and installs the default only if
        // the DEFAULT binding is currently void (Fdefvar_1 consults
        // `default_bound_p', not `boundp' — a buffer-local binding in
        // the current buffer must not suppress it).
        i.obarray.symbol_mut(sid).special = true;
        if matches!(i.obarray.symbol(sid).value, Value::Sym(s) if s == sym::UNBOUND) {
            let v = eval_at_argument(i, &init, &args, 1)?;
            i.set_symbol_default(sid, v)?;
        }
    } else if i.lexenv.is_some() {
        // Bare `(defvar SYM)' in a lexical context is a scoped special
        // declaration: GNU pushes the bare symbol onto
        // `internal-interpreter-environment', so `let'/`let*' in this
        // scope bind SYM dynamically and the declaration unwinds with
        // the scope (it does NOT set `declared_special').
        if !i.obarray.symbol(sid).special {
            crate::lisp::eval::lexenv_declare_checked(&i.lexenv, sid)
                .map_err(|error| error.into_flow(i))?;
        }
    }
    // Dynamic context (nil interpreter environment): bare `(defvar SYM)'
    // "does nothing", per GNU's docstring — all bindings are dynamic
    // anyway, and the var does not become `special-variable-p'.
    let doc = nth_arg(i, &args, 2)?;
    if let Value::Str(s) = doc {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.q("variable-documentation");
        let doc = s
            .try_borrow()
            .map(|text| (*text).clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed docstring"))?;
        i.put_prop(sid, vd, Value::string(doc));
    }
    // GNU eval.c `defvar' LOADHIST_ATTACHes the bare symbol — this is
    // what records `(defvar SYM)' inside compiled `byte-code' forms,
    // where the definition can't be seen by scanning top-level forms.
    crate::lisp::builtins::evalfn::loadhist_attach(i, name_v.clone());
    Ok(name_v)
}

fn sf_defconst(i: &mut Interp, args: Value) -> EvalResult {
    // Same arg cap as `defvar' — `(defconst S V D X)' signals
    // `(error "Too many arguments")'.
    if !cdr(i, &cdr(i, &cdr(i, &args)?)?)?.is_nil() {
        return Err(i.error("Too many arguments".to_string()));
    }
    let name_v = car(i, &args)?;
    let sid = match i.sym_id(&name_v) {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    // GNU Fdefconst_1: special + set_default + docstring, plus
    // `risky-local-variable t'.  Constancy is NOT enforced at runtime
    // ("not actually enforced by Emacs Lisp") — only nil, t and
    // keywords are NOWRITE; defconst must never set `constant'.
    i.obarray.symbol_mut(sid).special = true;
    let v = eval_argument(i, &args, 1)?;
    i.set_symbol_default(sid, v)?;
    let doc = nth_arg(i, &args, 2)?;
    if let Value::Str(s) = doc {
        // GNU: Fput(symbol, Qvariable_documentation, doc).
        let vd = i.q("variable-documentation");
        let doc = s
            .try_borrow()
            .map(|text| (*text).clone())
            .map_err(|_| i.error("Cannot inspect a mutably borrowed docstring"))?;
        i.put_prop(sid, vd, Value::string(doc));
    }
    let rlv = i.q("risky-local-variable");
    i.put_prop(sid, rlv, Value::t());
    // GNU routes `defconst' through `defvar' (eval.c), which
    // LOADHIST_ATTACHes the bare symbol.
    crate::lisp::builtins::evalfn::loadhist_attach(i, name_v.clone());
    Ok(name_v)
}

fn sf_defun(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(i, &args)?;
    // GNU's defun macro (byte-run.el): `(or name (error ...))' runs
    // BEFORE the arglist check, and name's `symbolp' check is deferred
    // to `defalias' in the expanded form — so arglist errors come first.
    if name_v.is_nil() {
        let msg = format!(
            "Cannot define '{}' as a function",
            i.princ_to_string(&name_v)
        );
        return Err(i.error(msg));
    }
    let params = cadr(i, &args)?;
    // `(null (and (listp arglist) (null (delq t (mapcar #'symbolp
    // arglist)))))' → `(error "Malformed arglist: %s" arglist)'.
    // mapcar walks the spine: a non-symbol *tail* signals `listp TAIL'.
    {
        // `(listp ARGLIST)' first: non-list atoms are malformed
        // (GNU: `(and (listp arglist) ...)'), not `listp' errors.
        let malformed = match &params {
            Value::Nil => false,
            Value::Cons(_) => {
                let mut cur = params.clone();
                let mut seen_params = FxHashSet::default();
                let mut malformed = false;
                let mut until_quit = 1024usize;
                loop {
                    match &cur {
                        Value::Nil => break,
                        Value::Cons(c) => {
                            if !seen_params.insert(Rc::as_ptr(c) as usize) {
                                return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                            }
                            let (head, next) = cons_parts(i, c)?;
                            if i.sym_id(&head).is_none() {
                                malformed = true;
                            }
                            cur = next;
                            until_quit -= 1;
                            if until_quit == 0 {
                                i.maybe_quit()?;
                                until_quit = 1024;
                            }
                        }
                        // `mapcar' walks the spine: dotted tail → listp.
                        other => return Err(i.wrong_type_mut("listp", other)),
                    }
                }
                malformed
            }
            _ => true,
        };
        if malformed {
            return Err(i.error(format!("Malformed arglist: {}", i.princ_to_string(&params))));
        }
    }
    let body_v = cdr(i, &cdr(i, &args)?)?;
    let body = match body_v.list_to_vec() {
        Ok(body) => body,
        Err(crate::lisp::value::ListError::Dotted(tail)) => {
            return Err(i.wrong_type_mut("listp", &tail));
        }
        Err(crate::lisp::value::ListError::Circular(tail)) => {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail]));
        }
        Err(crate::lisp::value::ListError::Borrowed) => {
            return Err(i.error("Cannot inspect a mutably borrowed function body"));
        }
    };
    // GNU expands declaration handlers before evaluating the resulting
    // `defalias' form. If a handler signals, leave any prior definition
    // untouched; its returned action forms run after the definition below.
    let declaration_actions = defun_declaration_actions(i, &name_v, &params, &body, false)?;
    let sid = i.sym_id(&name_v);
    // Evaluating `defalias' arguments constructs the function value before
    // defalias checks NAME. Use a placeholder only for parse_lambda's
    // defun-body handling; invalid names cannot expose the resulting lambda.
    let parse_name = Some(sid.unwrap_or(sym::NIL));
    let mut lambda = i.parse_lambda(&params, &body, parse_name)?;
    // defun never captures a lexical env from the definition site in the
    // dynamic model; under lexical-binding it captures the file env.
    lambda.env = i.lambda_env_checked()?;
    let function = if i.boot_loading {
        Value::Lambda(Rc::new(lambda)).tracked()
    } else {
        i.function_from_parts(
            params.clone(),
            Value::list(lambda.body),
            lambda.doc_value.unwrap_or(Value::Nil),
            lambda.interactive.unwrap_or(Value::Nil),
        )?
    };
    let sid = match sid {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    i.fset_defalias(sid, function)?;
    // GNU `defun' reaches `defalias' (eval.c), which
    // LOADHIST_ATTACHes `(defun . SYM)'.
    let defun_id = i.q("defun");
    crate::lisp::builtins::evalfn::loadhist_attach(i, Value::cons(i.sym(defun_id), name_v.clone()));
    eval_defun_declaration_actions(i, &declaration_actions)?;
    Ok(name_v)
}

fn sf_defmacro(i: &mut Interp, args: Value) -> EvalResult {
    let name_v = car(i, &args)?;
    let params = cadr(i, &args)?;
    // GNU defmacro expands first — the arglist listp check runs before
    // defalias's symbolp check on NAME.
    match &params {
        Value::Nil | Value::Cons(_) => {}
        other => return Err(i.wrong_type_mut("listp", other)),
    }
    let body_v = cdr(i, &cdr(i, &args)?)?;
    let body = match body_v.list_to_vec() {
        Ok(body) => body,
        Err(crate::lisp::value::ListError::Dotted(tail)) => {
            return Err(i.wrong_type_mut("listp", &tail));
        }
        Err(crate::lisp::value::ListError::Circular(tail)) => {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail]));
        }
        Err(crate::lisp::value::ListError::Borrowed) => {
            return Err(i.error("Cannot inspect a mutably borrowed macro body"));
        }
    };
    let declaration_actions = defun_declaration_actions(i, &name_v, &params, &body, true)?;
    let sid = i.sym_id(&name_v);
    let parse_name = Some(sid.unwrap_or(sym::NIL));
    let mut lambda = i.parse_lambda(&params, &body, parse_name)?;
    lambda.is_macro = true;
    lambda.env = i.lambda_env_checked()?;
    let function = if i.boot_loading {
        Value::Lambda(Rc::new(lambda)).tracked()
    } else {
        let function = i.function_from_parts(
            params.clone(),
            Value::list(lambda.body),
            lambda.doc_value.unwrap_or(Value::Nil),
            lambda.interactive.unwrap_or(Value::Nil),
        )?;
        Value::cons(Value::Sym(sym::MACRO), function)
    };
    let sid = match sid {
        Some(s) => s,
        None => return Err(i.wrong_type_mut("symbolp", &name_v)),
    };
    i.fset_defalias(sid, function)?;
    // GNU `defmacro' reaches `defalias' the same way.
    let defun_id = i.q("defun");
    crate::lisp::builtins::evalfn::loadhist_attach(i, Value::cons(i.sym(defun_id), name_v.clone()));
    eval_defun_declaration_actions(i, &declaration_actions)?;
    Ok(name_v)
}

fn sf_lambda(i: &mut Interp, args: Value) -> EvalResult {
    i.function_from_lambda(&args)
}

/// GNU `macroexp--defun-declarations': each `(declare (PROP . ARGS))'
/// spec in BODY dispatches to the handler bound in
/// `defun-declarations-alist' (or `macro-declarations-alist' for
/// defmacro), called as (FN NAME ARGLIST . ARGS); the form it returns
/// is evaluated.  Unknown PROP specs are ignored.
fn defun_declaration_actions(
    i: &mut Interp,
    name_v: &Value,
    params: &Value,
    body: &[Value],
    is_macro: bool,
) -> Result<Vec<Value>, Flow> {
    let alist_name = if is_macro {
        "macro-declarations-alist"
    } else {
        "defun-declarations-alist"
    };
    let alist_id = match i.intern_soft(alist_name) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let alist = i.symbol_value(alist_id);
    if !matches!(alist, Value::Cons(_)) {
        return Ok(Vec::new());
    }
    let declare_id = i.q("declare");
    let documentation_id = i.q(":documentation");
    // Match byte-run's `macroexp--parse-body' prefix scan. Documentation
    // forms and duplicate declarations are consumed, but only its first
    // accepted `declare' form reaches the property handlers.
    let mut declaration_form = None;
    let mut interactive_seen = false;
    for (index, form) in body.iter().enumerate() {
        if matches!(form, Value::Str(_)) && index + 1 < body.len() {
            continue;
        }
        let Value::Cons(cell) = form else {
            break;
        };
        let head = cons_parts(i, cell)?.0;
        if i.sym_is(&head, documentation_id) {
            continue;
        }
        if i.sym_is(&head, declare_id) {
            if declaration_form.is_none() && (is_macro || !interactive_seen) {
                declaration_form = Some(form.clone());
            }
            continue;
        }
        if i.sym_is(&head, sym::INTERACTIVE) {
            if !is_macro {
                interactive_seen = true;
            }
            continue;
        }
        break;
    }
    let mut extra_forms: Vec<Value> = Vec::new();
    if let Some(form) = declaration_form {
        let declaration_clauses = cdr(i, &form)?;
        let no_function = Value::Nil;
        crate::lisp::builtins::seq::map_seq(i, &no_function, &declaration_clauses, |i, spec| {
            let (prop, dargs) = match spec {
                Value::Cons(c) => cons_parts(i, c)?,
                Value::Nil => (Value::Nil, Value::Nil),
                other => return Err(i.wrong_type_mut("listp", other)),
            };
            // assq over the handler alist.
            let mut cur = alist.clone();
            let handler = loop {
                match cur {
                    Value::Cons(c) => {
                        let (entry, rest) = cons_parts(i, &c)?;
                        let hit = match &entry {
                            Value::Cons(e) => crate::lisp::eq_values(&cons_parts(i, e)?.0, &prop),
                            _ => false,
                        };
                        if hit {
                            break match &entry {
                                Value::Cons(e) => {
                                    let (_, tail) = cons_parts(i, e)?;
                                    match tail {
                                        Value::Cons(c2) => Some(cons_parts(i, &c2)?.0),
                                        Value::Nil => Some(Value::Nil),
                                        other => {
                                            return Err(i.wrong_type_mut("listp", &other));
                                        }
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
                let declaration_args = match dargs.list_to_vec() {
                    Ok(args) => args,
                    Err(crate::lisp::value::ListError::Dotted(tail)) => {
                        return Err(i.wrong_type_mut("listp", &tail));
                    }
                    Err(crate::lisp::value::ListError::Circular(tail)) => {
                        return Err(i.signal_data(sym::CIRCULAR_LIST, vec![tail]));
                    }
                    Err(crate::lisp::value::ListError::Borrowed) => {
                        return Err(
                            i.error("Cannot inspect a mutably borrowed function declaration")
                        );
                    }
                };
                call.extend(declaration_args);
                let form = i.apply(&h, call)?;
                // GNU splices the returned form's progn into the
                // expansion; evaluating the form is equivalent.
                extra_forms.push(form);
            }
            Ok(())
        })?;
    }
    Ok(extra_forms)
}

fn eval_defun_declaration_actions(i: &mut Interp, forms: &[Value]) -> EvalResult {
    for (index, form) in forms.iter().enumerate() {
        i.eval_at_compiled_site(form, ((index as u128) << 1) | 1)?;
    }
    Ok(Value::Nil)
}

fn sf_while(i: &mut Interp, args: Value) -> EvalResult {
    let test = car(i, &args)?;
    let body = cdr(i, &args)?;
    let mut iters: u64 = 0;
    loop {
        let v = eval_at_argument(i, &test, &args, 0)?;
        if v.is_nil() {
            return Ok(Value::Nil);
        }
        i.maybe_quit()?;
        i.eval_progn(&body)?;
        iters += 1;
        if i.dbg_flags & 0x20 != 0 && iters == 2_000_000 {
            let names: Vec<String> = i
                .lisp_stack
                .iter()
                .rev()
                .take(24)
                .map(|(f, a)| {
                    format!(
                        "{} {}",
                        i.prin1_to_string(f),
                        i.prin1_to_string(&Value::list(a.as_slice().to_vec()))
                    )
                })
                .collect();
            eprintln!("WHILE-LOOP-STACK: {names:?}");
            let cond = i.prin1_to_string(&test);
            let b = i.prin1_to_string(&body);
            eprintln!("WHILE-LOOP-FORM: cond={cond} body={b}");
        }
    }
}

fn sf_catch(i: &mut Interp, args: Value) -> EvalResult {
    let tag = eval_argument(i, &args, 0)?;
    let body = cdr(i, &args)?;
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
                i.finish_quit_unwind();
                Ok(v)
            } else {
                i.rebase_quit_unwind();
                Err(Flow::Throw(t, v))
            }
        }
        Err(flow) => {
            i.rebase_quit_unwind();
            Err(flow)
        }
        other => other,
    }
}

fn sf_unwind_protect(i: &mut Interp, args: Value) -> EvalResult {
    let form = car(i, &args)?;
    let cleanup = cdr(i, &args)?;
    let result = eval_at_argument(i, &form, &args, 0);
    let result = i.offer_flow(result);
    let clean = i.with_quit_preserved(|i| i.eval_progn(&cleanup));
    match (result, clean) {
        (Ok(v), Ok(_)) => Ok(v),
        (Ok(_), Err(e)) => Err(e),
        (Err(e), Ok(_)) => Err(e),
        (Err(_), Err(e2)) => Err(e2),
    }
}

/// Unlike let, GNU binds a condition-case variable lexically whenever
/// an interpreter environment exists, even for special/declared symbols.
fn bind_handler_var(
    i: &mut Interp,
    frame: Option<&Rc<LexFrame>>,
    var: SymId,
    value: Value,
) -> Result<(), Flow> {
    if let Some(frame) = frame {
        crate::lisp::eval::lexframe_bind_checked(frame, var, value)
            .map_err(|error| error.into_flow(i))?;
        Ok(())
    } else {
        i.specbind(var, value)
    }
}

fn sf_condition_case(i: &mut Interp, args: Value) -> EvalResult {
    let var_v = car(i, &args)?;
    // GNU Fcondition_case: VAR must be nil or a symbol.
    if var_v.truthy() && i.sym_id(&var_v).is_none() {
        return Err(i.wrong_type_mut("symbolp", &var_v));
    }
    let bodyform = cadr(i, &args)?;
    let handlers = cdr(i, &cdr(i, &args)?)?;
    // GNU internal_lisp_condition_case validates every handler spec
    // BEFORE the body runs: each handler is nil, or a cons whose car
    // is a symbol or a (non-empty) list of condition names.  The
    // error message uses princ-style printing (Fprin1_to_string t).
    {
        let mut cur = handlers.clone();
        let mut seen_handlers = FxHashSet::default();
        let mut until_quit = 1024usize;
        loop {
            match cur.clone() {
                Value::Nil => break,
                Value::Cons(c) => {
                    if !seen_handlers.insert(Rc::as_ptr(&c) as usize) {
                        return Err(i.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
                    }
                    let (h, next) = cons_parts(i, &c)?;
                    let ok = match &h {
                        Value::Nil => true,
                        Value::Cons(_) => {
                            let cond = car(i, &h)?;
                            i.sym_id(&cond).is_some() || matches!(cond, Value::Cons(_))
                        }
                        _ => false,
                    };
                    if !ok {
                        let msg = format!("Invalid condition handler: {}", i.princ_to_string(&h));
                        return Err(i.error(&msg));
                    }
                    cur = next;
                    until_quit -= 1;
                    if until_quit == 0 {
                        i.maybe_quit()?;
                        until_quit = 1024;
                    }
                }
                _ => break,
            }
        }
    }
    // Register our condition names so a signal raised in BODY knows it
    // will be caught — `handler-bind' handlers only fire for signals
    // no enclosing `condition-case' claims (Emacs debugger semantics).
    let success = i.q(":success");
    let mut clauses = Vec::new();
    let mut cond_names = Vec::new();
    let mut success_handler = None;
    let mut handler_scan = handlers.clone();
    let mut until_quit = 1024usize;
    while let Value::Cons(cell) = handler_scan {
        let (h, next) = super::eval::cons_parts(i, &cell)?;
        let conditions = car(i, &h)?;
        if i.sym_id(&conditions) == Some(success) {
            success_handler = Some(h.clone());
        } else {
            cond_names.push(conditions);
            clauses.push(h.clone());
        }
        handler_scan = next;
        until_quit -= 1;
        if until_quit == 0 {
            i.maybe_quit()?;
            until_quit = 1024;
        }
    }
    // GNU retains the original clause objects in an alloca table. A cleanup
    // can replace the caller's handler-list cells without replacing that table.
    let handlers = Value::list(clauses);
    let case_depth = i.case_handlers.len();
    i.case_handlers.push(crate::lisp::error::CaseHandler {
        clauses: cond_names,
    });
    let body_result = eval_at_argument(i, &bodyform, &args, 1);
    let body_result = i.offer_flow(body_result);
    i.case_handlers.pop();
    match body_result {
        Ok(v) => {
            // GNU: a `:success' pseudo-handler runs when BODYFORM
            // returns normally — VAR is bound to the body's value.
            let cur = Value::list(success_handler.into_iter().collect());
            loop {
                match cur {
                    Value::Nil => return Ok(v),
                    Value::Cons(c) => {
                        let handler = car(i, &Value::Cons(c.clone()))?;
                        let hbody = cdr(i, &handler)?;
                        let mark = i.specbind_depth();
                        let lex_frame = if i.lexical_binding_active() {
                            Some(
                                crate::lisp::eval::lexenv_extend_checked(&i.lexenv)
                                    .map_err(|error| error.into_flow(i))?,
                            )
                        } else {
                            None
                        };
                        let saved_lex = match &lex_frame {
                            Some(f) => std::mem::replace(&mut i.lexenv, Some(f.clone())),
                            None => None,
                        };
                        if let Some(vid) = i.sym_id(&var_v).filter(|_| !var_v.is_nil()) {
                            if let Err(e) = bind_handler_var(i, lex_frame.as_ref(), vid, v.clone())
                            {
                                if lex_frame.is_some() {
                                    i.lexenv = saved_lex;
                                }
                                i.unbind_to(mark)?;
                                return Err(e);
                            }
                        }
                        // GNU Fprogn on an atom body (dotted handler
                        // like `(error . v)') returns nil — atoms are
                        // not evaluated.
                        let r = if matches!(hbody, Value::Cons(_) | Value::Nil) {
                            i.eval_progn(&hbody)
                        } else {
                            Ok(Value::Nil)
                        };
                        if lex_frame.is_some() {
                            i.lexenv = saved_lex;
                        }
                        i.unbind_to(mark)?;
                        return r;
                    }
                    _ => return Ok(v),
                }
            }
        }
        Err(Flow::Signal(signal)) => {
            let err_val = signal.condition.clone();
            let mut cur = handlers;
            let mut clause = 0;
            loop {
                match cur {
                    Value::Nil => {
                        i.rebase_quit_unwind();
                        return Err(Flow::Signal(signal));
                    }
                    Value::Cons(c) => {
                        let (handler, next) = cons_parts(i, &c)?;
                        if signal.claims_clause(case_depth, clause) {
                            i.finish_quit_unwind();
                            // Bind var and run handler body.
                            let hbody = cdr(i, &handler)?;
                            let mark = i.specbind_depth();
                            let lex_frame = if i.lexical_binding_active() {
                                Some(
                                    crate::lisp::eval::lexenv_extend_checked(&i.lexenv)
                                        .map_err(|error| error.into_flow(i))?,
                                )
                            } else {
                                None
                            };
                            let saved_lex = match &lex_frame {
                                Some(f) => std::mem::replace(&mut i.lexenv, Some(f.clone())),
                                None => None,
                            };
                            // GNU binds VAR only when it is non-nil.
                            if let Some(vid) = i.sym_id(&var_v).filter(|_| !var_v.is_nil()) {
                                if let Err(e) =
                                    bind_handler_var(i, lex_frame.as_ref(), vid, err_val)
                                {
                                    if lex_frame.is_some() {
                                        i.lexenv = saved_lex;
                                    }
                                    i.unbind_to(mark)?;
                                    return Err(e);
                                }
                            }
                            // GNU Fprogn on an atom handler body
                            // (dotted `(error . v)') returns nil.
                            let r = if matches!(hbody, Value::Cons(_) | Value::Nil) {
                                i.eval_progn(&hbody)
                            } else {
                                Ok(Value::Nil)
                            };
                            if lex_frame.is_some() {
                                i.lexenv = saved_lex;
                            }
                            i.unbind_to(mark)?;
                            return r;
                        }
                        cur = next;
                        clause += 1;
                    }
                    _ => {
                        i.rebase_quit_unwind();
                        return Err(Flow::Signal(signal));
                    }
                }
            }
        }
        Err(flow) => {
            i.rebase_quit_unwind();
            Err(flow)
        }
    }
}

#[cfg(test)]
mod circular_condition_case_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn circular_handler_spine_signals_before_evaluating_the_body() {
        let mut interp = Interp::vacant();
        interp.obarray = crate::lisp::obarray::Obarray::new();
        let body = Value::Sym(interp.intern("qa-handler-body-must-not-run"));
        let clause = Value::list(vec![Value::Sym(sym::ERROR), Value::Nil]);
        let handlers = Value::cons(clause, Value::Nil);
        if let Value::Cons(cell) = &handlers {
            cell.borrow_mut().cdr = handlers.clone();
        }
        let args = Value::cons(Value::Nil, Value::cons(body, handlers));

        let result = sf_condition_case(&mut interp, args);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
        assert_eq!(interp.specbind_depth(), 0);
    }

    #[test]
    fn long_handler_validation_observes_host_quit_requests() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let request = Arc::new(crate::frontend::QuitRequest::default());
                interp.set_quit_request(Some(request.clone()));

                let clause = Value::list(vec![Value::Sym(sym::ERROR)]);
                let mut handlers = Value::list(vec![Value::Int(5)]);
                for _ in 0..500_000 {
                    handlers = Value::cons(clause.clone(), handlers);
                }
                let args = Value::cons(Value::Nil, Value::cons(Value::Nil, handlers));

                let input_request = request.clone();
                let input = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    input_request.request();
                });
                let result = sf_condition_case(&mut interp, args);
                input.join().unwrap();

                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
                ));
                assert!(request.discard_queued_key());
                assert_eq!(interp.case_handlers.len(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod circular_defun_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn circular_defun_arglist_signals_circular_list() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let name = Value::Sym(interp.intern("qa-circular-defun-arglist"));
        let variable = Value::Sym(interp.intern("qa-circular-defun-variable"));
        let params = Value::cons(variable, Value::Nil);
        if let Value::Cons(cell) = &params {
            cell.borrow_mut().cdr = params.clone();
        }

        let result = sf_defun(
            &mut interp,
            Value::list(vec![name, params, Value::Sym(sym::T)]),
        );
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
    }
}

#[cfg(test)]
mod malformed_definition_body_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn defun_and_defmacro_reject_circular_bodies() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        for (name, kind) in [
            ("qa-circular-defun-body", sym::DEFUN),
            ("qa-circular-defmacro-body", sym::DEFMACRO),
        ] {
            let args = Value::list(vec![
                Value::Sym(interp.intern(name)),
                Value::Nil,
                Value::Int(42),
            ]);
            let body = cdr(&interp, &cdr(&interp, &args).unwrap()).unwrap();
            if let Value::Cons(cell) = &body {
                cell.borrow_mut().cdr = body.clone();
            }
            let result = if kind == sym::DEFUN {
                sf_defun(&mut interp, args)
            } else {
                sf_defmacro(&mut interp, args)
            };
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
            ));
        }
    }

    #[test]
    fn defun_rejects_a_dotted_body_tail() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let name = Value::Sym(interp.intern("qa-dotted-defun-body"));
        let args = Value::list(vec![name, Value::Nil, Value::Int(42)]);
        let body = cdr(&interp, &cdr(&interp, &args).unwrap()).unwrap();
        let tail = Value::Sym(interp.intern("qa-defun-body-tail"));
        if let Value::Cons(cell) = &body {
            cell.borrow_mut().cdr = tail;
        }
        let result = sf_defun(&mut interp, args);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
        ));
    }
}

#[cfg(test)]
mod circular_let_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn parallel_and_sequential_let_preserve_cycle_error_order() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(defvar qa-circular-let-init-side-effect nil)\
                         (let ((bindings\
                                (list (list 'x\
                                            '(setq qa-circular-let-init-side-effect 'ran)))))\
                           (setcdr bindings bindings)\
                           (setq qa-circular-let-init-side-effect nil)\
                           (let ((parallel\
                                  (condition-case err\
                                      (eval (list 'let bindings nil))\
                                    (circular-list\
                                     (list (car err) qa-circular-let-init-side-effect)))))\
                             (setq qa-circular-let-init-side-effect nil)\
                             (list parallel\
                                   (condition-case err\
                                       (eval (list 'let* bindings nil))\
                                     (circular-list\
                                      (list (car err)\
                                            qa-circular-let-init-side-effect))))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&result),
                    "((circular-list nil) (circular-list ran))"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    fn circular_bindings(var: SymId) -> Value {
        let bindings = Value::cons(Value::Sym(var), Value::Nil);
        if let Value::Cons(cell) = &bindings {
            cell.borrow_mut().cdr = bindings.clone();
        }
        bindings
    }

    #[test]
    fn let_rejects_circular_binding_spines_before_initialization() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let var = interp.intern("qa-circular-let-variable");

        let depth = interp.specbind_depth();
        let result = sf_let(&mut interp, Value::cons(circular_bindings(var), Value::Nil));
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
        assert_eq!(interp.specbind_depth(), depth);
    }
}

#[cfg(test)]
mod circular_setq_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn circular_setq_spine_signals_before_evaluating_rhs() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let target = interp.intern("qa-circular-setq-target");
        let side_effect = interp.intern("qa-circular-setq-side-effect");
        let rhs = Value::list(vec![
            Value::Sym(sym::SETQ),
            Value::Sym(side_effect),
            Value::list(vec![Value::Sym(sym::QUOTE), Value::Sym(sym::T)]),
        ]);
        let args = Value::list(vec![Value::Sym(target), rhs]);
        if let Value::Cons(first) = &args {
            let Value::Cons(value_cell) = first.borrow().cdr.clone() else {
                unreachable!();
            };
            value_cell.borrow_mut().cdr = args.clone();
        }

        let result = sf_setq(&mut interp, args);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
        assert!(matches!(
            interp.symbol_value(side_effect),
            Value::Sym(symbol) if symbol == sym::UNBOUND
        ));
    }
}

#[cfg(test)]
mod circular_short_circuit_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    fn circular_list(value: Value) -> Value {
        let list = Value::cons(value, Value::Nil);
        if let Value::Cons(cell) = &list {
            cell.borrow_mut().cdr = list.clone();
        }
        list
    }

    fn signals_circular(result: EvalResult) -> bool {
        matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST))
    }

    #[test]
    fn and_or_and_cond_validate_cycles_before_short_circuiting() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();

        assert!(signals_circular(sf_and(
            &mut interp,
            circular_list(Value::Nil)
        )));
        assert!(signals_circular(sf_or(
            &mut interp,
            circular_list(Value::t())
        )));
        let clause = Value::list(vec![Value::Sym(sym::T), Value::Sym(sym::T)]);
        assert!(signals_circular(sf_cond(
            &mut interp,
            circular_list(clause)
        )));
    }
}

#[cfg(test)]
mod nested_cons_borrow_tests {
    use super::*;

    #[test]
    fn cond_reports_a_mutably_borrowed_nested_clause_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let clause = Value::list(vec![Value::Int(1), Value::Int(7)]);
                let form = Value::cons(
                    Value::Sym(sym::COND),
                    Value::list(vec![clause.clone()]),
                );
                let Value::Cons(cell) = &clause else {
                    unreachable!();
                };
                let held = cell.borrow_mut();

                let result = interp.eval(&form);
                drop(held);
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                assert!(matches!(interp.eval(&form), Ok(Value::Int(7))));
            })
            .unwrap()
            .join()
            .unwrap();
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
    // Match simple.el's macro: save the mark, restore ordinary excursion
    // first, then restore the mark (and its hooks) as unwind-protect cleanup.
    let save_mark = Value::Sym(i.q("save-mark-and-excursion--save"));
    let mark = i.apply(&save_mark, vec![])?;
    let saved = i.save_excursion_state(false);
    let r = i.eval_progn(&args);
    i.restore_excursion_state(saved);
    let restore_mark = Value::Sym(i.q("save-mark-and-excursion--restore"));
    i.apply(&restore_mark, vec![mark])?;
    r
}

fn sf_with_current_buffer(i: &mut Interp, args: Value) -> EvalResult {
    let buf_v = eval_argument(i, &args, 0)?;
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
    let r = match cdr(i, &args) {
        Ok(body) => i.eval_progn(&body),
        Err(error) => Err(error),
    };
    if i.buffer_live(old) {
        i.set_current_buffer(old);
    }
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

fn sf_save_restriction(i: &mut Interp, args: Value) -> EvalResult {
    let saved = i.save_restriction_state();
    let r = i.eval_progn(&args);
    i.restore_restriction_state(saved);
    r
}

// ---------- backquote ----------
