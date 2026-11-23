//! The evaluator: `Interp` state, `eval`, `apply`, and special forms.
//!
//! Scoping: elisp is dynamically scoped by default. Dynamic `let` is
//! implemented exactly like Emacs's specbind: save the symbol's current
//! value cell, overwrite, restore on unwind — so lookup stays O(1).
//! When `lexical-binding` is on, `let` binds in a captured lexical env
//! chain instead (unless the symbol is `defvar`'d special).
//!
//! `Interp` storage and snapshots live in `state`; this module retains
//! evaluation and function-call dispatch. Private modules own symbol/value
//! metadata (`symbols`, `objects`), function-cell resolution (`functions`),
//! source entry points (`source`), macro expansion (`macros`), signal/quit
//! delivery (`control`), and buffer state (`buffers`). Bootstrap, lambda,
//! advice, interactive/minibuffer, lexical/dynamic bindings, diagnostics,
//! property lists, and output each retain their own modules.
//! Existing `eval::*` public paths are preserved through re-exports.

use std::cell::RefCell;

use rustc_hash::FxHashSet;
use std::rc::Rc;

use super::error::{EvalResult, Flow};
use super::obarray::sym;
use super::value::{Argv, Arity, Cons, Lambda, SymId, Value};

mod advice;
mod bindings;
mod bootstrap;
mod buffers;
mod conditions;
mod control;
mod functions;
mod interactive;
mod lambda;
mod lexenv;
mod macros;
mod minibuffer;
mod objects;
mod output;
mod plist;
mod source;
mod state;
mod symbols;
use bindings::SpecBind;
pub(crate) use interactive::{subr_interactive, subr_interactive_form};
pub use minibuffer::{MinibufArgs, MinibufInput};
pub use state::{ExcursionState, Interp, RestrictionState};
use state::{SpecialMacroIdentity, SpecialMacroSnapshot};

/// Per-call behavior for the front-end side of `sit-for'.
#[derive(Clone, Copy)]
pub(crate) struct SitForOptions {
    pub process_timers: bool,
    pub redisplay: bool,
}

#[derive(Clone, Copy)]
enum EvalArgsMode {
    /// Lambda calls keep the initial argument count and read each next cdr
    /// before evaluating the current form.
    Lambda,
    /// N-adic subrs also keep the initial count, but stop if the live spine
    /// is no longer a cons.
    NadicSubr,
    /// Fixed-arity subrs read the current cell's cdr after evaluating its car,
    /// up to the declared maximum argument count.
    FixedSubr { max_args: usize },
}

// Keep existing eval::* paths usable by runtime code and external callers.
pub use lexenv::{
    LexEnv, LexFrame, lexenv_as_value, lexenv_declare, lexenv_extend, lexenv_lookup, lexenv_root,
    lexframe_bind,
};
pub(crate) use lexenv::{
    lexenv_as_value_checked, lexenv_declared, lexenv_declare_checked, lexenv_extend_checked,
    lexenv_from_value, lexenv_lookup_checked, lexenv_set, lexframe_bind_checked,
};
pub(crate) use plist::{plist_get_v_checked, plist_put_v_checked};
pub use plist::{
    plist_get, plist_get_v, plist_lookup, plist_lookup_v, plist_put, plist_put_front, plist_put_v,
    plist_remove_v,
};

#[path = "threads.rs"]
pub(crate) mod threads;

pub(crate) fn cons_parts(i: &Interp, cell: &Rc<RefCell<Cons>>) -> Result<(Value, Value), Flow> {
    let borrowed = cell
        .try_borrow()
        .map_err(|_| i.error("Cannot evaluate a mutably borrowed cons cell"))?;
    Ok((borrowed.car.clone(), borrowed.cdr.clone()))
}

fn cons_cdr(i: &Interp, cell: &Rc<RefCell<Cons>>) -> Result<Value, Flow> {
    cell.try_borrow()
        .map(|borrowed| borrowed.cdr.clone())
        .map_err(|_| i.error("Cannot traverse a mutably borrowed cons cell"))
}

fn cons_car(i: &Interp, cell: &Rc<RefCell<Cons>>) -> Result<Value, Flow> {
    cell.try_borrow()
        .map(|borrowed| borrowed.car.clone())
        .map_err(|_| i.error("Cannot read a mutably borrowed cons cell"))
}

fn same_cons(left: &Value, right: &Value) -> bool {
    matches!((left, right), (Value::Cons(left), Value::Cons(right)) if Rc::ptr_eq(left, right))
}

fn argument_spine_shape(interp: &mut Interp, args: &Value) -> Result<(i128, Option<Value>), Flow> {
    // Tortoise/hare over safe cdr snapshots. Circular spines are marked
    // with -1 so dispatch can preserve function-error ordering; dotted
    // tails are returned for the caller to validate at the right point.
    // Like GNU's list walkers, long spines must remain interruptible.
    interp.maybe_quit()?;
    let mut slow = args.clone();
    let mut fast = args.clone();
    let mut guard = true;
    let mut until_quit = 4096usize;
    let mut spine_len: i128 = 0;
    let mut dotted_tail = None;
    loop {
        match &slow {
            Value::Cons(cell) => {
                slow = cons_cdr(interp, cell)?;
                spine_len += 1;
                until_quit -= 1;
                if until_quit == 0 {
                    interp.maybe_quit()?;
                    until_quit = 4096;
                }
            }
            Value::Nil => break,
            other => {
                dotted_tail = Some(other.clone());
                break;
            }
        }
        if !guard {
            continue;
        }
        match &fast {
            Value::Cons(cell) => {
                let n1 = cons_cdr(interp, cell)?;
                match &n1 {
                    Value::Cons(next) => fast = cons_cdr(interp, next)?,
                    _ => {
                        fast = n1;
                        guard = false;
                    }
                }
            }
            _ => guard = false,
        }
        if same_cons(&slow, &fast) {
            spine_len = -1;
            break;
        }
    }
    Ok((spine_len, dotted_tail))
}

/// Where output from `princ`/`print`/`message` goes. The editor installs a
/// real sink; tests use the buffer.
pub enum OutputSink {
    /// Accumulate into a string (tests, `with-output-to-string`).
    Buffer(Rc<RefCell<super::text::EmacsText>>),
    /// Call a Lisp function with each printed string.
    Function(Value),
    /// Write to process stdout (batch mode).
    Stdout,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecialRecordKind {
    Obarray,
    FontSpec,
    SymbolWithPos,
}

impl Interp {
    fn special_macro_identity(&self, function: &Value) -> Option<SpecialMacroIdentity> {
        match function {
            Value::Lambda(lambda) if lambda.is_macro => {
                Some(SpecialMacroIdentity::Lambda(Rc::downgrade(lambda)))
            }
            Value::Cons(cell) => {
                let borrowed = cell.try_borrow().ok()?;
                if !self.sym_is(&borrowed.car, sym::MACRO) {
                    return None;
                }
                let Value::Lambda(lambda) = &borrowed.cdr else {
                    return None;
                };
                Some(SpecialMacroIdentity::MacroCell {
                    cell: Rc::downgrade(cell),
                    function: Rc::downgrade(lambda),
                })
            }
            _ => None,
        }
    }

    fn special_macro_identity_matches(
        &self,
        identity: &SpecialMacroIdentity,
        function: &Value,
    ) -> bool {
        match (identity, function) {
            (SpecialMacroIdentity::Lambda(weak), Value::Lambda(lambda)) => weak
                .upgrade()
                .is_some_and(|saved| Rc::ptr_eq(&saved, lambda)),
            (
                SpecialMacroIdentity::MacroCell {
                    cell: weak_cell,
                    function: weak_function,
                },
                Value::Cons(cell),
            ) => {
                let Some(saved_cell) = weak_cell.upgrade() else {
                    return false;
                };
                let Some(saved_function) = weak_function.upgrade() else {
                    return false;
                };
                if !Rc::ptr_eq(&saved_cell, cell) {
                    return false;
                }
                cell.try_borrow().is_ok_and(|borrowed| {
                    self.sym_is(&borrowed.car, sym::MACRO)
                        && matches!(&borrowed.cdr, Value::Lambda(lambda)
                            if Rc::ptr_eq(&saved_function, lambda))
                })
            }
            _ => false,
        }
    }

    fn special_macro_cell_repr(
        &mut self,
        function: &Value,
    ) -> Result<crate::lisp::text::EmacsText, Flow> {
        let mark = self.specbind_depth();
        for (name, value) in [
            ("print-length", Value::Nil),
            ("print-level", Value::Nil),
            ("print-circle", Value::t()),
        ] {
            if let Some(symbol) = self.q_soft(name)
                && let Err(error) = self.specbind(symbol, value)
            {
                self.unbind_to(mark)?;
                return Err(error);
            }
        }
        let result = self.prin1_to_string(function);
        self.unbind_to(mark)?;
        Ok(result)
    }

    pub(crate) fn capture_special_macro_snapshots(&mut self) {
        for symbol in [sym::DEFUN, sym::DEFMACRO] {
            if super::special::special_form(symbol).is_none() {
                continue;
            }
            let function = self.symbol_function(symbol);
            if !self.is_macro_function(&function) {
                continue;
            }
            let Some(identity) = self.special_macro_identity(&function) else {
                continue;
            };
            let Ok(printed) = self.special_macro_cell_repr(&function) else {
                continue;
            };
            self.special_macro_snapshots
                .insert(symbol, SpecialMacroSnapshot { identity, printed });
        }
    }

    fn special_form_for_cell(&mut self, symbol: SymId, function: &Value) -> Option<SymId> {
        if let Value::Subr(subr) = function {
            if !matches!(subr.arity, Arity::Unevalled) {
                return None;
            }
            let id = self.q_soft(subr.name)?;
            return super::special::special_form(id).map(|_| id);
        }

        // GNU's byte-compiled defun/defmacro expanders also have evaluator-
        // side implementations for their exact arity error descriptors.
        // Keep that path only for the shipped dumped cell with its original
        // name. A user replacement, including another macro, must run from
        // the live function cell.
        if !matches!(symbol, sym::DEFUN | sym::DEFMACRO)
            || super::special::special_form(symbol).is_none()
        {
            return None;
        }
        let snapshot = self.special_macro_snapshots.get(&symbol)?.clone();
        if self.special_macro_identity_matches(&snapshot.identity, function) {
            return Some(symbol);
        }
        let printed = self.special_macro_cell_repr(function).ok()?;
        if printed != snapshot.printed {
            return None;
        }
        let identity = self.special_macro_identity(function)?;
        self.special_macro_snapshots
            .insert(symbol, SpecialMacroSnapshot { identity, printed });
        Some(symbol)
    }

    fn dispatch_special_form(
        &mut self,
        special_id: SymId,
        args: &Value,
        spine_len: i128,
        dotted_tail: Option<Value>,
        shown: Value,
    ) -> EvalResult {
        let Some(sf) = super::special::special_form(special_id) else {
            return Err(self.error("internal: missing special-form evaluator"));
        };
        if let Some(tail) = dotted_tail {
            return Err(self.wrong_type_mut("listp", &tail));
        }
        let min = super::special::special_form_min_args(special_id) as i128;
        let n = if spine_len >= 0 {
            spine_len
        } else {
            self.raw_list_length(args)?
        };
        if n < min {
            let shown = if special_id == sym::DEFUN || special_id == sym::DEFMACRO {
                Value::cons(Value::Int(2), Value::Int(2))
            } else {
                shown
            };
            return Err(self.wrong_number_of_args(&shown, n));
        }
        sf(self, args.clone())
    }

    // ---------- eval ----------

    pub fn eval(&mut self, form: &Value) -> EvalResult {
        let counter = self.bc_quit_counter.take();
        let unwind = self.unwind_quit.take();
        let result = self.eval_entry(form);
        self.bc_quit_counter = counter;
        if result.is_ok() {
            self.finish_quit_unwind();
            self.unwind_quit = unwind;
        }
        result
    }

    fn eval_entry(&mut self, form: &Value) -> EvalResult {
        if let Some(status) = self.exit_status {
            return Err(Flow::Exit(status));
        }
        self.profiler_poll();
        // eval_sub does not consume nesting depth for symbols or literals.
        if !matches!(form, Value::Cons(_)) {
            let result = self.eval_inner(form);
            return self.offer_flow(result);
        }
        self.with_lisp_eval_depth(false, |i| {
            if i.dbg_flags & 0x02 != 0 && i.eval_depth % 200 == 0 {
                eprintln!(
                    "[depth={}] {}",
                    i.eval_depth,
                    i.princ_to_string(form)
                        .chars()
                        .take(80)
                        .collect::<crate::lisp::text::EmacsText>()
                );
            }
            i.eval_inner(form)
        })
    }

    fn eval_inner(&mut self, form: &Value) -> EvalResult {
        if cfg!(debug_assertions) && self.dbg_flags & 0x08 != 0 {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n > 0 && n % 20_000_000 == 0 {
                let names: Vec<super::text::EmacsText> = self
                    .lisp_stack
                    .iter()
                    .rev()
                    .take(30)
                    .map(|(f, _)| self.prin1_to_string(f))
                    .collect();
                eprintln!("EVAL-WATCH n={n} stack={names:?}");
                eprintln!("EVAL-WATCH form={}", self.prin1_to_string(form));
            }
        }
        match form {
            Value::Nil => self.eval_symbol(sym::NIL, Value::Nil),
            Value::Int(_)
            | Value::BigInt(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Vec(_)
            | Value::Hash(_)
            | Value::Subr(_)
            | Value::Lambda(_)
            | Value::Buffer(_)
            | Value::Marker(_)
            | Value::Window(_)
            | Value::Frame(_)
            | Value::Process(_)
            | Value::Thread(_)
            | Value::Mutex(_)
            | Value::CondVar(_)
            | Value::Finalizer(_) => Ok(form.clone()),
            Value::Sym(id) => self.eval_symbol(*id, Value::Sym(*id)),
            Value::Record(_) => {
                // `symbols-with-pos-enabled' objects evaluate like
                // their bare symbol (GNU: they are real symbols).
                match crate::lisp::builtins::misc::active_sympos_parts(self, form) {
                    Some((s, _)) => self.eval_symbol(s, form.clone()),
                    None => Ok(form.clone()),
                }
            }
            Value::Cons(_) => self.eval_form(form),
        }
    }

    fn eval_symbol(&mut self, id: SymId, reported_symbol: Value) -> EvalResult {
        // Lexical lookup first — GNU `eval_sub' does `Fassq (form,
        // internal-interpreter-environment)' unconditionally ("we do not
        // pay attention to the declared_special flag here"): a closure
        // parameter that became special AFTER capture (e.g. via
        // `internal--define-uninitialized-variable', as
        // `custom-initialize-after-file-load' does) still reads the
        // lexical binding, matching byte-compiled GNU code where the
        // parameter is a stack slot.
        if let Some(v) = lexenv_lookup_checked(&self.lexenv, id).map_err(|e| e.into_flow(self))? {
            return Ok(v);
        }
        let v = self.symbol_value(id);
        if let Value::Sym(s) = &v {
            if *s == sym::UNBOUND {
                if self.dbg_flags & 0x10 != 0 {
                    let name = self.obarray.symbol(id).name.clone();
                    let depth = usize::from(self.lexenv.is_some());
                    let found = lexenv_lookup(&self.lexenv, id).is_some();
                    eprintln!(
                        "DBG void-var {} special={} lexdepth={} lexhas={} specbinds={} case_handlers={}",
                        name,
                        self.obarray.symbol(id).special,
                        depth,
                        found,
                        self.specbind.iter().filter(|s| s.sym == id).count(),
                        self.case_handlers.len(),
                    );
                    for (f, a) in self.lisp_stack.iter().rev().take(25) {
                        eprintln!("   bt {} nargs={}", self.prin1_to_string(f), a.len());
                    }
                }
                return Err(self.signal_data(sym::VOID_VARIABLE, vec![reported_symbol]));
            }
        }
        Ok(v)
    }

    /// Evaluate a compound form `(fn . args)`.
    fn eval_form(&mut self, form: &Value) -> EvalResult {
        let Value::Cons(cell) = form else {
            unreachable!()
        };
        let (head, args) = cons_parts(self, cell)?;
        let displayed_head = if self.sym_id(&head).is_some() {
            head.clone()
        } else {
            crate::lisp::builtins::misc::unpos(self, &head)
        };
        self.with_pending_frame(displayed_head, args, |i, frame| {
            let result = i.eval_form_inner(form, frame);
            i.offer_flow(result)
        })
    }

    fn eval_form_inner(&mut self, form: &Value, frame: usize) -> EvalResult {
        let cons = match form {
            Value::Cons(c) => c,
            _ => unreachable!(),
        };
        // A compiled macro callsite uses its cached expansion without
        // revisiting the mutable source cons or its argument spine.
        if self.current_body_compiled
            && let Some(cache) = self.current_compiled_macro_cache.clone()
            && !cache.borrow().is_empty()
        {
            let site_key = Rc::as_ptr(cons) as usize;
            let compiled_key = (site_key, self.current_compiled_macro_path.clone());
            let expansion =
                cache
                    .borrow()
                    .get(&compiled_key)
                    .and_then(|(cached_form, expansion)| {
                        cached_form
                            .upgrade()
                            .filter(|cached| Rc::ptr_eq(cached, cons))
                            .map(|_| expansion.clone())
                    });
            if let Some(expansion) = expansion {
                return self.eval_at_compiled_site(&expansion, (site_key as u128) << 1);
            }
        }
        let (head_value, args) = cons_parts(self, cons)?;
        // `symbols-with-pos-enabled' symbols in operator position are
        // real symbols (GNU parity): dispatch on the bare symbol.
        let sym_name = self.sym_id(&head_value).map(|_| head_value.clone());
        let head = match self.sym_id(&head_value) {
            Some(id) => Value::Sym(id),
            None => crate::lisp::builtins::misc::unpos(self, &head_value),
        };
        // Count the argument spine before dispatch. GNU checks an immediate
        // non-list tail before operator lookup, but after one or more
        // argument cells it resolves the operator first: void/invalid
        // function errors win, while special forms and callable functions
        // reject the dotted tail before evaluating operands.
        let (spine_len, dotted_tail) = argument_spine_shape(self, &args)?;

        if let Some(tail) = &dotted_tail
            && !matches!(args, Value::Cons(_) | Value::Nil)
        {
            return Err(self.wrong_type_mut("listp", tail));
        }

        if self.dbg_flags & 0x80 != 0 {
            eprintln!(
                "[eval_form head={} args={}]",
                self.princ_to_string(&head),
                self.princ_to_string(&args)
            );
        }
        match &head {
            Value::Sym(id) => {
                let id = *id;
                let fun = self.form_function(id);
                // Special form?  GNU checks each special form's own
                // minimum arity inside the evaluator (Fif needs 2,
                // Fsetq needs pairs, ...): report `(SYM N)'.
                // Dispatch from the live function cell. Redefining or
                // unbinding a special-form name changes eval's behavior in
                // GNU; the name table is only the implementation map for an
                // actual unevalled special subr.
                let special_id = self.special_form_for_cell(id, &fun).or_else(|| {
                    (self.boot_loading
                        && matches!(&fun, Value::Sym(s) if *s == sym::UNBOUND)
                        && super::special::special_form(id).is_some())
                    .then_some(id)
                });
                if let Some(special_id) = special_id {
                    return self.dispatch_special_form(
                        special_id,
                        &args,
                        spine_len,
                        dotted_tail.clone(),
                        sym_name.clone().unwrap_or_else(|| self.sym(id)),
                    );
                }
                // A dedicated byte-op does not consult the function cell.
                // Dispatch before checking for an unbound/advised definition,
                // just as the compiled opcode does.
                if self.current_body_compiled {
                    if let Some(s) = self.byte_op_subr(id) {
                        return self.call_function_len(
                            &Value::Subr(s),
                            &args,
                            Some(sym_name.clone().unwrap_or_else(|| self.sym(id))),
                            spine_len,
                            dotted_tail.clone(),
                            frame,
                            false,
                        );
                    }
                }
                if self.dbg_flags & 0x01 != 0 {
                    self.cur_head_form = form.clone();
                }
                // Function cell.
                if let Value::Sym(s) = &fun {
                    if *s == sym::UNBOUND {
                        if std::env::var_os("REMACS_TRACE_SETF").is_some() {
                            eprintln!(
                                "void head: {} in form {}",
                                self.symbol_name(id),
                                self.prin1_to_string(&form)
                                    .chars()
                                    .take(200)
                                    .collect::<crate::lisp::text::EmacsText>()
                            );
                        }
                        return Err(self.signal_data(
                            sym::VOID_FUNCTION,
                            vec![sym_name.clone().unwrap_or_else(|| self.sym(id))],
                        ));
                    }
                }
                // Macro call: GNU's evaluator re-expands interpreted
                // forms on every call, while byte-compiled call sites
                // already have their macro expansion. The cache below
                // preserves the latter behavior for compiled-equivalent
                // bundled code that remacs evaluates from source.
                // Inside a `compiled' (dumped-library-equivalent)
                // function, a symbol-head call to a byte-op primitive
                // dispatches to its canonical subr — GNU's compiled
                // `(car x)' emits Bcar, which never reads
                // `symbol-function', so advising `car' can't make the
                // advice machinery's own calls recurse.
                if self.is_macro_function_checked(&fun)? {
                    if let Some(tail) = &dotted_tail {
                        return Err(self.wrong_type_mut("listp", tail));
                    }
                    let site_key = Rc::as_ptr(cons) as usize;
                    let compiled_key = (site_key, self.current_compiled_macro_path.clone());
                    // Only compiled-equivalent code has GNU's compile-time
                    // expansion semantics. An interpreted call site must
                    // run its macro expander on every evaluation, since an
                    // expander may observe and change runtime state.
                    // `dumped_call_depth' includes compiled ancestors: a
                    // source `eval' inside one must still expand its input
                    // as source on each evaluation. Use the immediate body
                    // provenance for compiled call sites. Bootstrap and
                    // compiled-library loading keep their separate global
                    // compatibility behavior.
                    let compiled_callsite = self.current_body_compiled;
                    let cache_expansion = self.loading_compiled_library || self.boot_loading;
                    // Inside dumped-library (compiled-equivalent) lambdas
                    // macro calls were expanded at "compile" time — i.e.
                    // against the pre-advice cell.  Strip any advice
                    // trampolines so interpreted nadvice/gv code behaves
                    // like GNU's .elc where `when' is an unadvised `if'.
                    // Match the callsite provenance, not a compiled
                    // ancestor: source `eval' and interpreted callbacks
                    // expand macros against the live advised function cell.
                    let fun = if compiled_callsite || cache_expansion {
                        self.unadvised_base(&fun)
                    } else {
                        fun
                    };
                    if compiled_callsite
                        && let Some(cache) = self.current_compiled_macro_cache.clone()
                    {
                        let expansion = cache.borrow().get(&compiled_key).and_then(
                            |(cached_form, expansion)| {
                                cached_form
                                    .upgrade()
                                    .filter(|cached| Rc::ptr_eq(cached, cons))
                                    .map(|_| expansion.clone())
                            },
                        );
                        if let Some(expansion) = expansion {
                            return self.eval_at_compiled_site(&expansion, (site_key as u128) << 1);
                        }
                        let expansion = self.eval_macro_expand(&fun, &args)?;
                        cache
                            .borrow_mut()
                            .insert(compiled_key, (Rc::downgrade(cons), expansion.clone()));
                        return self.eval_at_compiled_site(&expansion, (site_key as u128) << 1);
                    }
                    if cache_expansion
                        && let Some((stamp, fncell, expansion, _pin)) =
                            self.macro_cache.get(&compiled_key)
                    {
                        // `(macro . f)' cells are mutated in place by
                        // nadvice's gv machinery, so compare the cdr
                        // too, not just the cons identity.
                        let same_fn = match (&fun, fncell) {
                            (Value::Cons(a), Value::Cons(b)) => {
                                let aa = a.try_borrow().map_err(|_| {
                                    self.error("Cannot inspect a mutably borrowed macro function cell")
                                })?;
                                let bb = b.try_borrow().map_err(|_| {
                                    self.error("Cannot inspect a mutably borrowed cached macro function cell")
                                })?;
                                crate::lisp::builtins::eq_values(&aa.car, &bb.car)
                                    && crate::lisp::builtins::eq_values(&aa.cdr, &bb.cdr)
                            }
                            _ => crate::lisp::builtins::eq_values(&fun, fncell),
                        };
                        if *stamp == self.macro_gen && same_fn {
                            let expansion = expansion.clone();
                            return self.eval_at_compiled_site(&expansion, (site_key as u128) << 1);
                        }
                    }
                    let expansion = self.eval_macro_expand(&fun, &args)?;
                    if cache_expansion {
                        if self.macro_cache.len() >= 262144 {
                            self.macro_cache.clear();
                        }
                        self.macro_cache.insert(
                            compiled_key,
                            (self.macro_gen, fun, expansion.clone(), form.clone()),
                        );
                    }
                    return self.eval_at_compiled_site(&expansion, (site_key as u128) << 1);
                }
                self.call_function_len(
                    &fun,
                    &args,
                    Some(sym_name.clone().unwrap_or_else(|| self.sym(id))),
                    spine_len,
                    dotted_tail,
                    frame,
                    true,
                )
            }
            Value::Cons(_) => {
                // `((lambda (x) ...) a b)` — function position lambda is
                // used directly, not evaluated as a call.
                let (b, _) = match &head {
                    Value::Cons(c) => cons_parts(self, c)?,
                    _ => unreachable!(),
                };
                if self.sym_is(&b, sym::LAMBDA) {
                    // Direct lambda calls capture the active interpreted
                    // lexical environment, like evaluating the lambda and
                    // then calling the resulting closure.  Keep the source
                    // form as the displayed callee for its error data.
                    let lambda_tail = cons_cdr(self, match &head {
                        Value::Cons(cell) => cell,
                        _ => unreachable!(),
                    })?;
                    let function = self.function_from_lambda(&lambda_tail)?;
                    // Constructing a lambda can evaluate its `:documentation'
                    // form. Re-read the original argument spine afterward;
                    // that evaluation may repair a dotted tail.
                    let (live_spine_len, live_dotted_tail) =
                        argument_spine_shape(self, &args)?;
                    if let Some(tail) = &live_dotted_tail {
                        return Err(self.wrong_type_mut("listp", tail));
                    }
                    let argv = self.eval_args_len(&args, live_spine_len)?;
                    self.finish_pending_frame(frame);
                    return self.apply_resolved(&function, argv, head.clone());
                }
                // GNU uses the cons HEAD itself as the function object:
                // `(lambda ...)'/`(closure ...)'/`(macro . f)' are
                // callable, anything else signals invalid-function with
                // the cons (e.g. (('q)) → invalid-function (quote q)).
                self.call_function_len(&head, &args, None, spine_len, dotted_tail, frame, false)
            }
            _ => {
                let site = (Rc::as_ptr(cons) as usize as u128) << 1;
                let fun = self.eval_at_compiled_site(&head, site)?;
                let (live_spine_len, live_dotted_tail) = argument_spine_shape(self, &args)?;
                self.call_function_len(
                    &fun,
                    &args,
                    None,
                    live_spine_len,
                    live_dotted_tail,
                    frame,
                    false,
                )
            }
        }
    }

    /// Evaluate each element of a list form, using PRE_LEN when the
    /// caller already counted the spine (eval_form's check walk).
    pub fn eval_args_len(&mut self, args: &Value, pre_len: i128) -> Result<Argv, Flow> {
        self.eval_args_len_mode(args, pre_len, EvalArgsMode::Lambda)
    }

    fn eval_args_len_mode(
        &mut self,
        args: &Value,
        pre_len: i128,
        mode: EvalArgsMode,
    ) -> Result<Argv, Flow> {
        const MAX_INITIAL_ARG_CAPACITY: usize = 4096;
        self.maybe_quit()?;
        let initial_count = if pre_len >= 0 {
            usize::try_from(pre_len).map_err(|_| self.error("Too many arguments"))?
        } else {
            usize::try_from(self.checked_call_spine_length(args)?)
                .map_err(|_| self.error("Too many arguments"))?
        };
        let capacity = match mode {
            EvalArgsMode::FixedSubr { max_args } => max_args,
            EvalArgsMode::Lambda | EvalArgsMode::NadicSubr => initial_count,
        }
        .min(MAX_INITIAL_ARG_CAPACITY);
        let mut out = Vec::with_capacity(capacity);
        let mut cur = args.clone();
        let mut until_quit = 4096usize;

        match mode {
            EvalArgsMode::Lambda => {
                for _ in 0..initial_count {
                    let (form, site, next) = match cur.clone() {
                        Value::Nil => (Value::Nil, 0, Value::Nil),
                        Value::Cons(cell) => {
                            let form = cons_car(self, &cell)?;
                            let next = cons_cdr(self, &cell)?;
                            let site = (Rc::as_ptr(&cell) as usize as u128) << 1;
                            (form, site, next)
                        }
                        other => return Err(self.wrong_type_mut("listp", &other)),
                    };
                    // Lambda calls save the cdr before evaluating each
                    // argument while retaining the original argument count.
                    cur = next;
                    out.push(self.eval_at_compiled_site(&form, site)?);
                    until_quit -= 1;
                    if until_quit == 0 {
                        self.maybe_quit()?;
                        until_quit = 4096;
                    }
                }
            }
            EvalArgsMode::NadicSubr => {
                for _ in 0..initial_count {
                    let Value::Cons(cell) = cur.clone() else {
                        break;
                    };
                    let form = cons_car(self, &cell)?;
                    // GNU's n-adic subr path saves the next tail before
                    // evaluating the current argument.
                    cur = cons_cdr(self, &cell)?;
                    let site = (Rc::as_ptr(&cell) as usize as u128) << 1;
                    out.push(self.eval_at_compiled_site(&form, site)?);
                    until_quit -= 1;
                    if until_quit == 0 {
                        self.maybe_quit()?;
                        until_quit = 4096;
                    }
                }
            }
            EvalArgsMode::FixedSubr { max_args } => {
                for _ in 0..max_args {
                    match cur.clone() {
                        // C fixed-arity subrs receive nil in omitted optional
                        // slots; keep walking the remaining declared slots.
                        Value::Nil => out.push(Value::Nil),
                        Value::Cons(cell) => {
                            let form = cons_car(self, &cell)?;
                            let site = (Rc::as_ptr(&cell) as usize as u128) << 1;
                            out.push(self.eval_at_compiled_site(&form, site)?);
                            // Fixed-arity subrs read the next cdr after the
                            // current argument has finished evaluating.
                            cur = cons_cdr(self, &cell)?;
                        }
                        other => return Err(self.wrong_type_mut("listp", &other)),
                    }
                    until_quit -= 1;
                    if until_quit == 0 {
                        self.maybe_quit()?;
                        until_quit = 4096;
                    }
                }
            }
        }
        Ok(Argv::new(out))
    }

    /// Evaluate each element of a list form.
    pub fn eval_args(&mut self, args: &Value) -> Result<Vec<Value>, Flow> {
        Ok(self.eval_args_len(args, -1)?.as_slice().to_vec())
    }

    /// Call `fun` with raw unevaluated `args` (a list). Evaluates args
    /// unless `fun` is a macro or special.
    pub fn call_function(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<SymId>,
    ) -> EvalResult {
        let sym_name = self
            .sym_id(fun)
            .map(|_| fun.clone())
            .or_else(|| sym_name.map(|id| self.sym(id)));
        let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
        let autoload_origin = self.sym_id(fun).is_some();
        self.with_pending_frame(shown, args.clone(), |i, frame| {
            let result = match argument_spine_shape(i, args) {
                Ok((spine_len, dotted_tail)) => {
                    if let Some(tail) = &dotted_tail
                        && !matches!(args, Value::Cons(_) | Value::Nil)
                    {
                        Err(i.wrong_type_mut("listp", tail))
                    } else {
                        i.call_function_len(
                            fun,
                            args,
                            sym_name,
                            spine_len,
                            dotted_tail,
                            frame,
                            autoload_origin,
                        )
                    }
                }
                Err(error) => Err(error),
            };
            i.offer_flow(result)
        })
    }

    /// `call_function` with a precomputed spine length (-1 = unknown).
    /// eval_form passes the count its arg-check walk already computed.
    fn call_function_len(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<Value>,
        spine_len: i128,
        dotted_tail: Option<Value>,
        frame: usize,
        autoload_origin: bool,
    ) -> EvalResult {
        let fun = match self.sym_id(fun) {
            Some(id) => Value::Sym(id),
            None => crate::lisp::builtins::misc::unpos(self, fun),
        };
        match &fun {
            Value::Sym(id) => {
                // Function alias chain: chase.  GNU's
                // `indirect_function' signals `cyclic-function-indirection'
                // with the symbol whose alias points back into the chain.
                // Most calls resolve in one hop — the cycle-detection set
                // allocates only after an actual alias step.
                let mut cur = *id;
                let mut seen: Option<FxHashSet<SymId>> = None;
                let mut until_quit = 1024usize;
                loop {
                    let f = self.callable_function(cur);
                    // Bootstrap source uses these evaluator paths before
                    // Lisp macro cells such as `defun' have been installed.
                    if self.boot_loading
                        && matches!(&f, Value::Sym(next) if *next == sym::UNBOUND)
                        && super::special::special_form(cur).is_some()
                    {
                        let shown = sym_name.clone().unwrap_or_else(|| self.sym(*id));
                        return self.dispatch_special_form(
                            cur,
                            args,
                            spine_len,
                            dotted_tail.clone(),
                            shown,
                        );
                    }
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                let shown = sym_name.clone().unwrap_or_else(|| self.sym(*id));
                                return Err(self.signal_data(sym::VOID_FUNCTION, vec![shown]));
                            }
                            if seen.is_none() {
                                let mut chain = FxHashSet::default();
                                chain.insert(cur);
                                seen = Some(chain);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.q("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![Value::Sym(cur)]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().insert(cur);
                            until_quit -= 1;
                            if until_quit == 0 {
                                self.maybe_quit()?;
                                until_quit = 1024;
                            }
                        }
                        other => {
                            // Resolve aliases first and inspect the actual
                            // function cell. A name in the special-form
                            // implementation table is not special after
                            // its cell is rebound to a lambda or macro.
                            if let Some(special_id) = self.special_form_for_cell(cur, &other) {
                                let shown = sym_name.clone().unwrap_or_else(|| self.sym(*id));
                                return self.dispatch_special_form(
                                    special_id,
                                    args,
                                    spine_len,
                                    dotted_tail.clone(),
                                    shown,
                                );
                            }
                            // Emacs reports the originally called symbol
                            // in arity errors, not the resolved one.
                            return self.call_function_len(
                                &other,
                                args,
                                sym_name,
                                spine_len,
                                dotted_tail.clone(),
                                frame,
                                autoload_origin,
                            );
                        }
                    }
                }
            }
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => {
                    let special_id = self
                        .q_soft(s.name)
                        .filter(|id| super::special::special_form(*id).is_some());
                    if let Some(special_id) = special_id {
                        let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                        return self.dispatch_special_form(
                            special_id,
                            args,
                            spine_len,
                            dotted_tail.clone(),
                            shown,
                        );
                    }
                    (s.func)(self, Argv::new(vec![args.clone()]))
                }
                _ => {
                    if let Some(tail) = &dotted_tail {
                        return Err(self.wrong_type_mut("listp", tail));
                    }
                    // GNU checks a subr's arity before evaluating the
                    // argument forms, so `(car BAD EXTRA)' reports
                    // `wrong-number-of-arguments' rather than whatever
                    // BAD would signal.
                    let n = if spine_len >= 0 {
                        spine_len
                    } else {
                        self.raw_list_length(args)?
                    };
                    self.check_arity_subr_n(s, n, sym_name.as_ref())?;
                    let eval_mode = match s.arity {
                        Arity::Range { max, .. } if max <= 8 => {
                            EvalArgsMode::FixedSubr {
                                max_args: max as usize,
                            }
                        }
                        Arity::Range { .. } | Arity::Many { .. } => EvalArgsMode::NadicSubr,
                        Arity::Unevalled => unreachable!(),
                    };
                    let argv = self.eval_args_len_mode(args, spine_len, eval_mode)?;
                    self.finish_pending_frame(frame);
                    // Record the frame like apply_resolved does — GNU's
                    // specpdl holds every call, so backtraces show the
                    // innermost subr (`car(5)') too.
                    let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                    self.profiler_checkpoint();
                    let trace_args = if matches!(eval_mode, EvalArgsMode::FixedSubr { .. }) {
                        let count = usize::try_from(n).unwrap_or(0).min(argv.len());
                        Argv::new(argv.as_slice()[..count].to_vec())
                    } else {
                        argv.clone()
                    };
                    self.lisp_stack.push((shown, trace_args));
                    let r = (s.func)(self, argv);
                    let r = self.offer_flow(r);
                    self.profiler_checkpoint();
                    self.lisp_stack.pop();
                    r
                }
            },
            Value::Lambda(l) => {
                if let Some(tail) = &dotted_tail {
                    return Err(self.wrong_type_mut("listp", tail));
                }
                // Macro: expand then eval.
                if l.is_macro {
                    let expansion = self.eval_macro_expand(&fun, args)?;
                    return self.eval(&expansion);
                }
                // GNU byte-compiles its dumped defuns, and compiled
                // functions check arity before argument forms are
                // evaluated (interpreted lambdas do not).  `dumped_doc'
                // marks our dumped-equivalent definitions.
                if l.dumped_doc {
                    let n = if spine_len >= 0 {
                        spine_len
                    } else {
                        self.raw_list_length(args)?
                    };
                    let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                    self.check_arity_lambda_n(l, n, &shown)?;
                }
                // Dumped `#[ARGDESC ...]' twins (compiled, no parsed
                // arglist) get the same early check — GNU's nativecomp
                // signal data is the called symbol, while apply/funcall
                // truncate excess args instead (exec() does that).
                if !l.dumped_doc && l.compiled {
                    if let Some(items) = &l.bc_items {
                        let items =
                            items.try_borrow().map(|items| items.clone()).map_err(|_| {
                                self.error("Cannot inspect a mutably borrowed byte-code object")
                            })?;
                        if super::bytecode::is_byte_code(&items) {
                            let n = if spine_len >= 0 {
                                spine_len
                            } else {
                                self.raw_list_length(args)?
                            };
                            let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                            super::bytecode::check_argdesc(
                                self,
                                &items,
                                n.max(0) as usize,
                                &shown,
                            )?;
                        }
                    }
                }
                let argv = self.eval_args_len(args, spine_len)?;
                self.finish_pending_frame(frame);
                let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                self.apply_resolved(&fun, argv, shown)
            }
            Value::Cons(_) => {
                // A cons as function: `(lambda ...)` form or `(macro . f)`.
                let (car, cdr) = match &fun {
                    Value::Cons(c) => cons_parts(self, c)?,
                    _ => unreachable!(),
                };
                if self.sym_is(&car, sym::MACRO) {
                    if let Some(tail) = &dotted_tail {
                        return Err(self.wrong_type_mut("listp", tail));
                    }
                    // (macro . lambda) — expand with cdr as function.
                    let expansion = self.eval_macro_expand(&cdr, args)?;
                    return self.eval(&expansion);
                }
                if self.sym_is(&car, sym::LAMBDA) {
                    if let Some(tail) = &dotted_tail {
                        return Err(self.wrong_type_mut("listp", tail));
                    }
                    // Symbol-function indirection's cons form: GNU
                    // `funcall_lambda' on `(lambda ...)'.  `((function
                    // (lambda ...)))' (QUOTE_FUNCTION head) is NOT a
                    // lambda call — `function' is no macro here, so GNU
                    // `invalid-function' on the cons.
                    let argv = self.eval_args_len(args, spine_len)?;
                    self.finish_pending_frame(frame);
                    let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                    return self.apply_resolved(&fun, argv, shown);
                }
                let auto_id = self.q("autoload");
                if self.sym_is(&car, auto_id) {
                    // An autoload cell is only a loader when reached through
                    // a symbol's function cell. A literal `(autoload ...)' in
                    // function position is just a malformed function object.
                    if !autoload_origin {
                        return Err(self.wrong_type_mut("symbolp", &fun));
                    }
                    // (autoload FILE ...) — load, then re-dispatch on
                    // the real definition (macro autoloads expand).
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    // Loading an autoload can run arbitrary Lisp, including
                    // code that mutates the caller's form. GNU computes
                    // list_length again after the load, so never reuse
                    // EVAL_SUB's pre-load count for the live argument spine.
                    let spine_len = self.checked_call_spine_length(args)?;
                    return self.call_function_len(
                        &newdef,
                        args,
                        sym_name,
                        spine_len,
                        None,
                        frame,
                        autoload_origin,
                    );
                }
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name
                            .as_ref()
                            .and_then(|name| self.sym_id(name))
                            .map(|id| self.symbol_name(id)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown]))
            }
            Value::Nil => {
                // A nil function object is a void function in GNU's
                // evaluator.  Preserve the called symbol for a named form;
                // a computed nil operator reports nil itself.
                let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                Err(self.signal_data(sym::VOID_FUNCTION, vec![shown]))
            }
            _ => {
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun_ {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name
                            .as_ref()
                            .and_then(|name| self.sym_id(name))
                            .map(|id| self.symbol_name(id)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                let shown = sym_name.clone().unwrap_or_else(|| fun.clone());
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown]))
            }
        }
    }

    /// Length of a raw (unevaluated) argument list: the number of cons
    /// cells in its spine.  A dotted tail is not counted; a circular
    /// list signals `circular-list' like GNU's `Flength'.
    pub(crate) fn raw_list_length(&mut self, args: &Value) -> Result<i128, Flow> {
        // GNU's FOR_EACH_TAIL detector is Brent's walk: its meeting cell
        // is observable as the `circular-list' error datum.  Keep each cdr
        // snapshot owned so a caller holding RefMut gets a Lisp error.
        let mut n = 0i128;
        let mut cur = args.clone();
        let mut tortoise = args.clone();
        let mut max = 2usize;
        let mut steps = 0isize;
        let mut q = 2u16;
        loop {
            let next = match &cur {
                Value::Cons(cell) => cons_cdr(self, cell)?,
                _ => break,
            };
            n += 1;
            cur = next;
            q = q.wrapping_sub(1);
            if q == 0 {
                self.maybe_quit()?;
                steps -= 1;
                if steps <= 0 {
                    max = max.saturating_mul(2);
                    q = max as u16;
                    steps = (max >> 16) as isize;
                    tortoise = cur.clone();
                    continue;
                }
            }
            if same_cons(&cur, &tortoise) {
                return Err(self.signal_data(sym::CIRCULAR_LIST, vec![cur.clone()]));
            }
        }
        Ok(n)
    }

    fn checked_call_spine_length(&mut self, args: &Value) -> Result<i128, Flow> {
        let length = self.raw_list_length(args)?;
        let mut tail = args.clone();
        while let Value::Cons(cell) = tail {
            tail = cons_cdr(self, &cell)?;
        }
        if !tail.is_nil() {
            return Err(self.wrong_type_mut("listp", &tail));
        }
        Ok(length)
    }

    /// Convert a proper Lisp list to a Vec without panicking when a Rust
    /// caller still holds a mutable borrow of one of its cons cells.
    pub(crate) fn proper_list_to_vec(&mut self, list: &Value) -> Result<Vec<Value>, Flow> {
        let length = self.checked_call_spine_length(list)?;
        let capacity = usize::try_from(length).map_err(|_| self.error("List is too large"))?;
        let mut out = Vec::with_capacity(capacity);
        let mut cur = list.clone();
        while let Value::Cons(cell) = cur {
            let (car, cdr) = cons_parts(self, &cell)?;
            out.push(car);
            cur = cdr;
        }
        Ok(out)
    }

    fn check_arity_subr(
        &self,
        s: &'static super::value::Subr,
        argv: &[Value],
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        let name = name.map(|id| self.sym(id));
        self.check_arity_subr_n(s, argv.len() as i128, name.as_ref())
    }

    fn check_arity_subr_n(
        &self,
        s: &'static super::value::Subr,
        n: i128,
        name: Option<&Value>,
    ) -> Result<(), Flow> {
        // Emacs reports the calling symbol for eval'd calls, the subr
        // object itself for `funcall'/`apply'.
        let who = name.cloned().unwrap_or(Value::Subr(s));
        let (min, max) = match s.arity {
            Arity::Range { min, max } => (min as i128, max as i128),
            Arity::Many { min } => {
                if n < min as i128 {
                    return Err(self.wrong_number_of_args(&who, n));
                }
                return Ok(());
            }
            Arity::Unevalled => return Ok(()),
        };
        if n < min || n > max {
            return Err(self.wrong_number_of_args(&who, n));
        }
        Ok(())
    }

    /// Same arity test `call_lambda' performs, but against the raw
    /// argument count — used so dumped (GNU-compiled-equivalent)
    /// functions reject a bad call before arguments are evaluated.
    fn check_arity_lambda_n(&self, l: &Lambda, n: i128, who: &Value) -> Result<(), Flow> {
        let min = l.required.len() as i128;
        if n < min || (l.rest.is_none() && n > min + l.optional.len() as i128) {
            return Err(self.wrong_number_of_args(who, n));
        }
        Ok(())
    }

    /// `apply`/`funcall`: call `fun` with already-evaluated `argv`.
    pub fn apply(&mut self, fun: &Value, argv: impl Into<Argv>) -> EvalResult {
        let argv = argv.into();
        let counter = self.bc_quit_counter.take();
        let unwind = self.unwind_quit.take();
        let result = self.with_lisp_eval_depth(false, |i| i.apply_inner(fun, argv, None));
        self.bc_quit_counter = counter;
        if result.is_ok() {
            self.finish_quit_unwind();
            self.unwind_quit = unwind;
        }
        result
    }

    pub(crate) fn apply_bytecode(&mut self, fun: &Value, argv: impl Into<Argv>) -> EvalResult {
        let argv = argv.into();
        self.with_lisp_eval_depth(true, |i| {
            // GNU's fast lexical-bytecode call stays in the current VM;
            // aliases, autoloads, dynamic bytecode and C callbacks reenter it.
            let callee = match fun {
                Value::Sym(symbol) => i.symbol_function(*symbol),
                other => other.clone(),
            };
            let fast = match &callee {
                Value::Lambda(lambda) => match &lambda.bc_items {
                    Some(items) => {
                        let items = items.try_borrow().map_err(|_| {
                            i.error("Cannot inspect a mutably borrowed byte-code object")
                        })?;
                        matches!(items.first(), Some(Value::Int(_)))
                    }
                    None => false,
                },
                _ => false,
            };
            if fast {
                i.apply_inner(fun, argv, None)
            } else {
                let counter = i.bc_quit_counter.take();
                let result = i.apply_inner(fun, argv, None);
                i.bc_quit_counter = counter;
                result
            }
        })
    }

    /// Dedicated byte-code opcodes call their C primitive directly, without
    /// another funcall frame, depth increment, advice or symbol lookup.
    pub(crate) fn bytecode_subr(&mut self, name: &str, argv: impl Into<Argv>) -> EvalResult {
        if let Some(status) = self.exit_status {
            return Err(Flow::Exit(status));
        }
        let subr = self
            .byte_op_subr_named(name)
            .ok_or_else(|| self.error(format!("Invalid byte-code primitive: {name}")))?;
        let result = (subr.func)(self, argv.into());
        self.offer_flow(result)
    }

    fn apply_inner(&mut self, fun: &Value, argv: Argv, origin: Option<Value>) -> EvalResult {
        if let Some(status) = self.exit_status {
            return Err(Flow::Exit(status));
        }
        if self.dbg_flags & 0x01 != 0 {
            if let Value::Cons(c) = fun {
                let car = cons_parts(self, c)?.0;
                if !(self.sym_is(&car, sym::LAMBDA) || self.sym_is(&car, sym::MACRO)) {
                    eprintln!(
                        "[apply-raw {}]\n{}",
                        self.princ_to_string(fun)
                            .chars()
                            .take(150)
                            .collect::<crate::lisp::text::EmacsText>(),
                        std::backtrace::Backtrace::capture()
                    );
                }
            }
        }
        let shown = origin.or_else(|| self.sym_id(fun).map(|_| fun.clone()));
        let fun = match self.sym_id(fun) {
            Some(id) => Value::Sym(id),
            None => crate::lisp::builtins::misc::unpos(self, fun),
        };
        let shown = shown.unwrap_or_else(|| fun.clone());
        match &fun {
            Value::Sym(id) => {
                // Most calls resolve in one hop — the cycle-detection set
                // allocates only after an actual alias step.
                let mut cur = *id;
                let mut seen: Option<FxHashSet<SymId>> = None;
                let mut until_quit = 1024usize;
                loop {
                    let f = self.callable_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                // GNU `Ffuncall' reports the called
                                // symbol, not the alias chain's tail.
                                return Err(self.signal_data(sym::VOID_FUNCTION, vec![shown]));
                            }
                            if seen.is_none() {
                                let mut chain = FxHashSet::default();
                                chain.insert(cur);
                                seen = Some(chain);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.q("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![shown]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().insert(cur);
                            until_quit -= 1;
                            if until_quit == 0 {
                                self.maybe_quit()?;
                                until_quit = 1024;
                            }
                        }
                        other => return self.apply_resolved(&other, argv, shown),
                    }
                }
            }
            _ => self.apply_resolved(&fun, argv, shown),
        }
    }

    /// Dispatch a resolved (non-symbol) function, recording the call on
    /// `lisp_stack' so `mapbacktrace' can walk live frames.  `shown' is
    /// what backtraces display as the callee — the symbol when the call
    /// came through a symbol's function cell, else the function itself.
    fn apply_resolved(&mut self, fun: &Value, argv: Argv, shown: Value) -> EvalResult {
        if self.dbg_flags & 0x04 != 0 {
            eprintln!(
                "[call] {}",
                self.princ_to_string(&shown)
                    .chars()
                    .take(90)
                    .collect::<crate::lisp::text::EmacsText>()
            );
        }
        if self.bt_at_limit > 0 {
            let limit: usize = self.bt_at_limit;
            if self.lisp_stack.len() >= limit {
                eprintln!("=== REMACS_BT_AT hit, last 40 frames ===");
                for (f, a) in self.lisp_stack.iter().rev().take(40).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f)
                            .chars()
                            .take(80)
                            .collect::<crate::lisp::text::EmacsText>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(120)
                            .collect::<crate::lisp::text::EmacsText>()
                    );
                }
                std::process::exit(3);
            }
        }
        match fun {
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => {
                    // GNU: special forms cannot be funcalled/applied;
                    // the error data is the #<subr> object, not the
                    // calling symbol.
                    Err(self.signal_data(sym::INVALID_FUNCTION, vec![Value::Subr(s)]))
                }
                _ => {
                    self.check_arity_subr(s, &argv, None)?;
                    let prof = self.dbg_flags & 0x1000000 != 0;
                    let t0 = prof.then(std::time::Instant::now);
                    self.profiler_checkpoint();
                    self.lisp_stack.push((shown, argv.clone()));
                    let r = (s.func)(self, argv);
                    let r = self.offer_flow(r);
                    self.profiler_checkpoint();
                    self.lisp_stack.pop();
                    if let Some(t0) = t0 {
                        let e = self.subr_prof.entry(s.name).or_default();
                        e.0 += 1;
                        e.1 += t0.elapsed().as_nanos();
                    }
                    r
                }
            },
            Value::Lambda(l) => {
                // Dumped definitions stand in for GNU's nativecomp'd
                // subrs, and `funcall_subr' hands one with more than 8
                // parameters to its n-adic entry without an upper-bound
                // check: surplus args are dropped.  Call forms already
                // checked arity (`call_function_len', like `eval_sub');
                // bytecode twins get this in `bytecode::exec'.
                let max = l.required.len() + l.optional.len();
                let is_bytecode = l.try_is_bytecode().map_err(|_| {
                    self.error("Cannot inspect a mutably borrowed byte-code object")
                })?;
                let argv = if l.dumped_doc
                    && !l.is_macro
                    && !l.bad_arglist
                    && !is_bytecode
                    && l.rest.is_none()
                    && max > 8
                    && argv.len() > max
                {
                    Argv::new(argv[..max].to_vec())
                } else {
                    argv
                };
                self.profiler_checkpoint();
                self.lisp_stack.push((shown.clone(), argv.clone()));
                let r = self.call_lambda(l, argv, &shown);
                let r = self.offer_flow(r);
                self.profiler_checkpoint();
                self.lisp_stack.pop();
                r
            }
            Value::Cons(_) => {
                let (car, _) = match fun {
                    Value::Cons(c) => cons_parts(self, c)?,
                    _ => unreachable!(),
                };
                if self.sym_is(&car, sym::LAMBDA) {
                    // GNU `Ffuncall' calls the raw cons directly via
                    // `funcall_lambda' — no closure conversion, dynamic
                    // binding, `invalid-function' datum is the form.
                    self.profiler_checkpoint();
                    self.lisp_stack.push((shown.clone(), argv.clone()));
                    let r = self.call_lambda_form(fun, argv);
                    let r = self.offer_flow(r);
                    self.profiler_checkpoint();
                    self.lisp_stack.pop();
                    return r;
                }
                let auto_id = self.q("autoload");
                if self.sym_is(&car, auto_id) {
                    // GNU `Ffuncall': a bare `(autoload ...)' cons as
                    // the function signals `wrong-type-argument'
                    // `symbolp' — autoload indirection only happens
                    // through a symbol's function cell.
                    if !matches!(shown, Value::Sym(_)) {
                        return Err(self.wrong_type_mut("symbolp", fun));
                    }
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    // funcall_general retries the original symbol without
                    // another depth entry or losing its backtrace identity.
                    return self.apply_inner(&newdef, argv, Some(shown));
                }
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[apply-in {} shown={} callform={}]",
                        self.princ_to_string(&fun),
                        self.princ_to_string(&shown),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown.clone()]))
            }
            _ => {
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[applyin_ {} shown={} callform={}]",
                        self.princ_to_string(&fun),
                        self.princ_to_string(&shown),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                // GNU `Ffuncall' signals `void-function' for nil (its
                // "function cell" reads as void); other atoms are
                // `invalid-function'.
                if fun.is_nil() {
                    // The void report is `shown' (the symbol that was
                    // called), not the nil cell.
                    return Err(self.signal_data(sym::VOID_FUNCTION, vec![shown.clone()]));
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![shown.clone()]))
            }
        }
    }

    /// Evaluate a form stored at a stable source location. Compiled-equivalent
    /// macro caches include the location path so shared cons objects at two
    /// separate callsites retain separate macro expansions.
    pub(crate) fn eval_at_compiled_site(&mut self, form: &Value, site: u128) -> EvalResult {
        if !(self.current_body_compiled || self.loading_compiled_library || self.boot_loading) {
            return self.eval(form);
        }
        let saved = self.current_compiled_macro_path.len();
        self.current_compiled_macro_path.push(site);
        let result = self.eval(form);
        self.current_compiled_macro_path.truncate(saved);
        result
    }

    /// Evaluate a body (progn), returning the last value.
    pub fn eval_body(&mut self, body: &[Value]) -> EvalResult {
        self.maybe_quit()?;
        let mut last = Value::Nil;
        let mut until_quit = 4096usize;
        for (index, form) in body.iter().enumerate() {
            let site = ((index as u128) << 1) | 1;
            last = self.eval_at_compiled_site(form, site)?;
            until_quit -= 1;
            if until_quit == 0 {
                self.maybe_quit()?;
                until_quit = 4096;
            }
        }
        Ok(last)
    }

    /// Evaluate a list of forms (a `progn`-style list, not a Vec).
    pub fn eval_progn(&mut self, forms: &Value) -> EvalResult {
        // GNU evaluates each form before advancing through the body and does
        // not pre-scan for cycles. Earlier side effects or errors must win.
        self.maybe_quit()?;
        let mut until_quit = 4096usize;
        let mut last = Value::Nil;
        let mut cur = forms.clone();
        until_quit = 4096;
        // Fprogn saves both the form and next cdr before eval_sub. A form
        // may replace its own list cells without changing the saved next
        // tail, while mutations to that next cell remain observable.
        while let Value::Cons(cell) = cur.clone() {
            let (form, next) = cons_parts(self, &cell)?;
            cur = next;
            let site = (Rc::as_ptr(&cell) as usize as u128) << 1;
            last = self.eval_at_compiled_site(&form, site)?;
            until_quit -= 1;
            if until_quit == 0 {
                self.maybe_quit()?;
                until_quit = 4096;
            }
        }
        // Fprogn ignores atom bodies and improper tails.
        Ok(last)
    }
}

#[cfg(test)]
mod macro_cache_eval_tests {
    use super::*;

    #[test]
    fn special_macro_repr_binds_print_limits_after_unintern() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let print_length = interp.q("print-length");
                interp.obarray.symbol_mut(print_length).value = Value::Int(1);
                assert!(interp.obarray.unintern_symbol(print_length));

                let function = Value::list(vec![
                    Value::Sym(sym::MACRO),
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(3),
                ]);
                let printed = interp.special_macro_cell_repr(&function).unwrap();

                assert_eq!(printed.to_unicode_lossy(), "(macro 1 2 3)");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn cached_macro_comparison_reports_a_mutably_borrowed_old_cell() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str("(defmacro qa-borrowed-cache-macro () 1)")
                    .unwrap();
                let macro_id = interp.intern("qa-borrowed-cache-macro");
                let old_cell = interp.symbol_function(macro_id);
                interp
                    .eval_str("(defmacro qa-borrowed-cache-macro () 42)")
                    .unwrap();
                let form = Value::cons(Value::Sym(macro_id), Value::Nil);
                let Value::Cons(callsite) = &form else {
                    unreachable!();
                };
                let key = (Rc::as_ptr(callsite) as usize, Vec::new());
                interp.macro_cache.insert(
                    key,
                    (interp.macro_gen, old_cell.clone(), Value::Int(1), form.clone()),
                );
                interp.loading_compiled_library = true;

                let Value::Cons(old_cell) = &old_cell else {
                    panic!("macro function cell is not a cons");
                };
                let held_borrow = old_cell.borrow_mut();
                let result = interp.eval(&form);
                drop(held_borrow);
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));

                interp.loading_compiled_library = false;
                let result = interp.eval(&form).unwrap();
                assert!(matches!(result, Value::Int(42)));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    fn mark_compiled_equivalent(i: &mut Interp, name: &str) {
        let id = i.intern(name);
        let Value::Lambda(lambda) = &mut i.obarray.symbol_mut(id).function else {
            panic!("{name} is not a lambda");
        };
        crate::lisp::memory::update_unique_lambda(lambda, |lambda| lambda.compiled = true);
    }

    #[test]
    fn interpreted_function_inside_compiled_context_still_uses_advice() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-boundary-inner (items) (car items))\
                         (defun qa-boundary-car-advice (original &rest args) 'advised)\
                         (advice-add 'car :around #'qa-boundary-car-advice)",
                    )
                    .unwrap();

                // Model a dumped-equivalent function whose body is evaluated
                // as compiled source.  Its call to the user-defined function
                // must not make that function's own interpreted calls ignore
                // advice on byte-op primitives.
                interp
                    .eval_str(
                        "(defun qa-boundary-compiled-direct () (car '(direct)))\
                         (defun qa-boundary-compiled-wrapper (function)\
                           (funcall function '(item)))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-compiled-direct");
                mark_compiled_equivalent(&mut interp, "qa-boundary-compiled-wrapper");

                let direct = interp.eval_str("(qa-boundary-compiled-direct)").unwrap();
                assert_eq!(interp.prin1_to_string(&direct), "direct");

                let result = interp
                    .eval_str("(qa-boundary-compiled-wrapper #'qa-boundary-inner)")
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "advised");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn interpreted_macro_inside_compiled_context_still_uses_advice() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defmacro qa-boundary-macro () ''base)\
                         (defun qa-boundary-macro-helper () 'base-helper)\
                         (defun qa-boundary-macro-helper-advice\
                           (original &rest args) 'advised-helper)\
                         (advice-add 'qa-boundary-macro-helper :around\
                           #'qa-boundary-macro-helper-advice)\
                         (defun qa-boundary-macro-advice (original &rest args)\
                           (list 'quote (qa-boundary-macro-helper)))\
                         (advice-add 'qa-boundary-macro :around\
                           #'qa-boundary-macro-advice)\
                         (defun qa-boundary-macro-inner ()\
                           (qa-boundary-macro))",
                    )
                    .unwrap();

                interp
                    .eval_str(
                        "(defun qa-boundary-macro-wrapper (function)\
                           (funcall function))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-macro-wrapper");

                let result = interp
                    .eval_str("(qa-boundary-macro-wrapper #'qa-boundary-macro-inner)")
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "advised-helper");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn explicit_macroexpand_inside_compiled_context_uses_live_advice() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defmacro qa-boundary-explicit-macro () ''base)\
                         (defun qa-boundary-explicit-helper () 'base-helper)\
                         (defun qa-boundary-explicit-helper-advice\
                           (original &rest args) 'advised-helper)\
                         (advice-add 'qa-boundary-explicit-helper :around\
                           #'qa-boundary-explicit-helper-advice)\
                         (defun qa-boundary-explicit-macro-advice\
                           (original &rest args)\
                           (list 'quote (qa-boundary-explicit-helper)))\
                         (advice-add 'qa-boundary-explicit-macro :around\
                           #'qa-boundary-explicit-macro-advice)\
                         (defun qa-boundary-explicit-wrapper ()\
                           (list\
                            (macroexpand '(qa-boundary-explicit-macro))\
                            (macroexpand-all '(qa-boundary-explicit-macro))))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-explicit-wrapper");

                let result = interp
                    .eval_str(
                        "(equal (qa-boundary-explicit-wrapper)\
                               '((quote advised-helper)\
                                 (quote advised-helper)))",
                    )
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "t");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_macro_callsite_ignores_late_advice() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-boundary-late-helper () 'before)\
                         (defmacro qa-boundary-late-macro ()\
                           (list 'quote (qa-boundary-late-helper)))\
                         (defun qa-boundary-late-wrapper ()\
                           (qa-boundary-late-macro))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-late-wrapper");
                interp
                    .eval_str(
                        "(defun qa-boundary-late-helper-advice\
                           (original &rest args) 'after)\
                         (defun qa-boundary-late-macro-advice\
                           (original &rest args) '(quote macro-after))\
                         (advice-add 'qa-boundary-late-helper :around\
                           #'qa-boundary-late-helper-advice)\
                         (advice-add 'qa-boundary-late-macro :around\
                           #'qa-boundary-late-macro-advice)",
                    )
                    .unwrap();

                let result = interp.eval_str("(qa-boundary-late-wrapper)").unwrap();
                assert_eq!(interp.prin1_to_string(&result), "before");

                interp
                    .eval_str("(fmakunbound 'qa-boundary-late-macro)")
                    .unwrap();
                let wrapper_id = interp.intern("qa-boundary-late-wrapper");
                let tail = interp.intern("qa-boundary-late-tail");
                let Value::Lambda(wrapper) = interp.symbol_function(wrapper_id) else {
                    panic!("compiled wrapper is not a lambda");
                };
                let Value::Cons(callsite) = &wrapper.body[0] else {
                    panic!("compiled wrapper body is not a call form");
                };
                callsite.borrow_mut().cdr = Value::Sym(tail);
                let cached = interp.eval_str("(qa-boundary-late-wrapper)").unwrap();
                assert_eq!(interp.prin1_to_string(&cached), "before");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn explicit_macroexpand_in_compiled_load_context_uses_live_advice() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defmacro qa-load-macro () ''base)\
                         (defun qa-load-helper () 'base-helper)\
                         (defun qa-load-helper-advice (original &rest args)\
                           'advised-helper)\
                         (advice-add 'qa-load-helper :around\
                           #'qa-load-helper-advice)\
                         (defun qa-load-macro-advice (original &rest args)\
                           (list 'quote (qa-load-helper)))\
                         (advice-add 'qa-load-macro :around\
                           #'qa-load-macro-advice)",
                    )
                    .unwrap();
                let form = Value::list(vec![Value::Sym(interp.q("qa-load-macro"))]);
                let expansion = interp
                    .with_eval_context(true, |i| {
                        crate::lisp::builtins::evalfn::macroexpand_all(i, &form)
                    })
                    .unwrap();
                let result = interp.eval(&expansion).unwrap();
                assert_eq!(interp.prin1_to_string(&result), "advised-helper");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_macroexpand_all_uses_pre_advice_macro_cell() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defmacro qa-compiled-load-macro () ''base)\
                         (defun qa-compiled-load-macro-advice\
                           (original &rest args) '(quote advised))\
                         (advice-add 'qa-compiled-load-macro :around\
                           #'qa-compiled-load-macro-advice)",
                    )
                    .unwrap();
                let form = Value::list(vec![Value::Sym(interp.q("qa-compiled-load-macro"))]);
                let expansion = interp
                    .with_eval_context(true, |i| {
                        i.compiled_macro_expansion = true;
                        crate::lisp::builtins::evalfn::macroexpand_all(i, &form)
                    })
                    .unwrap();
                let result = interp.eval(&expansion).unwrap();
                assert_eq!(interp.prin1_to_string(&result), "base");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_macroexpand_1_uses_pre_advice_macro_cell() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defmacro qa-compiled-macroexpand-1-macro () ''base)\
                         (defun qa-compiled-macroexpand-1-advice\
                           (original &rest args) '(quote advised))\
                         (advice-add 'qa-compiled-macroexpand-1-macro :around\
                           #'qa-compiled-macroexpand-1-advice)",
                    )
                    .unwrap();
                let macro_call = Value::list(vec![Value::Sym(
                    interp.q("qa-compiled-macroexpand-1-macro"),
                )]);
                let quoted_call = Value::list(vec![Value::Sym(sym::QUOTE), macro_call]);
                let form = Value::list(vec![Value::Sym(interp.q("macroexpand-1")), quoted_call]);
                let expansion = interp
                    .with_eval_context(true, |i| {
                        i.compiled_macro_expansion = true;
                        i.eval(&form)
                    })
                    .unwrap();
                let result = interp.eval(&expansion).unwrap();
                assert_eq!(interp.prin1_to_string(&result), "base");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_source_byteop_ignores_void_function_cell() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-boundary-unbound-length ()\
                           (length '(item)))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-unbound-length");

                let length = interp.q("length");
                let saved = interp.symbol_function(length);
                interp.eval_str("(fmakunbound 'length)").unwrap();
                let result = interp.eval_str("(qa-boundary-unbound-length)");
                interp.obarray.symbol_mut(length).function = saved;

                let result = result.unwrap();
                assert_eq!(interp.prin1_to_string(&result), "1");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn source_eval_closure_inside_compiled_context_stays_interpreted() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-boundary-length-target nil)\
                         (defun qa-boundary-length-advice (original &rest args)\
                           (if (eq (car args) qa-boundary-length-target)\
                               'advised (apply original args)))\
                         (advice-add 'length :around\
                           #'qa-boundary-length-advice)\
                         (setq qa-boundary-length-target (list 'item))",
                    )
                    .unwrap();

                interp
                    .eval_str(
                        "(defun qa-boundary-eval-closure-wrapper ()\
                           (eval '(lambda (items) (length items)) t))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-boundary-eval-closure-wrapper");

                let result = interp
                    .eval_str(
                        "(funcall (qa-boundary-eval-closure-wrapper)\
                                  qa-boundary-length-target)",
                    )
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "advised");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn eval_inside_compiled_context_does_not_cache_macro_expansion() {
        // Interp bootstrap plus nested Lisp macro expansion can exceed the
        // Rust test harness's default 2 MiB thread stack. Match the 64 MiB
        // stacks used by neighboring evaluator tests.
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar eval-macro-cache-count 0)\
                         (defvar eval-macro-cache-form nil)\
                         (defmacro eval-macro-cache-step ()\
                           (setq eval-macro-cache-count (1+ eval-macro-cache-count))\
                           eval-macro-cache-count)\
                         (setq eval-macro-cache-form '(eval-macro-cache-step))",
                    )
                    .unwrap();

                // Model a source `eval' called by a compiled ancestor. The
                // explicit evaluation context clears its immediate
                // provenance, but keeps the ancestor depth.
                interp.dumped_call_depth += 1;
                let result = interp.eval_str(
                    "(list (eval eval-macro-cache-form)\
                           (eval eval-macro-cache-form)\
                           eval-macro-cache-count)",
                );
                interp.dumped_call_depth -= 1;

                let result = result.unwrap();
                assert_eq!(interp.prin1_to_string(&result), "(1 2 2)");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn macroexpand_all_reports_a_mutably_borrowed_nested_form_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let lambda = Value::list(vec![
                    Value::Sym(sym::LAMBDA),
                    Value::Nil,
                    Value::Int(1),
                ]);
                let function = Value::list(vec![Value::Sym(sym::FUNCTION), lambda.clone()]);
                let Value::Cons(cell) = &lambda else {
                    unreachable!();
                };
                let borrowed = cell.borrow_mut();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::lisp::builtins::evalfn::macroexpand_all(&mut interp, &function)
                }));
                drop(borrowed);
                let result = result.expect("macroexpand-all must not panic");
                assert!(matches!(result, Err(Flow::Signal(_))));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_equivalent_callsite_keeps_its_expansion_after_macro_redefinition() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq qa-cache-expand-count 0 qa-cache-run-count 0)\
                         (defmacro qa-cache-changing-macro ()\
                           (setq qa-cache-expand-count (1+ qa-cache-expand-count))\
                           '(setq qa-cache-run-count (1+ qa-cache-run-count)))",
                    )
                    .unwrap();

                // Mark this lambda as compiled-equivalent. Its source form
                // remains available, so the evaluator must keep the first
                // expansion as a byte-compiled callsite would.
                interp
                    .eval_str(
                        "(defun qa-cache-compiled-callsite ()\
                           (qa-cache-changing-macro))",
                    )
                    .unwrap();
                mark_compiled_equivalent(&mut interp, "qa-cache-compiled-callsite");

                let first = interp.eval_str("(qa-cache-compiled-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&first), "1");
                // The global transient cache is cleared during GC, but the
                // compiled callsite's expansion belongs to its Lambda.
                interp.eval_str("(garbage-collect)").unwrap();
                interp
                    .eval_str(
                        "(defmacro qa-cache-changing-macro ()\
                           (setq qa-cache-expand-count (+ qa-cache-expand-count 10))\
                           '(setq qa-cache-run-count (+ qa-cache-run-count 10)))",
                    )
                    .unwrap();
                let second = interp.eval_str("(qa-cache-compiled-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&second), "2");
                let expand_count = interp.q("qa-cache-expand-count");
                let run_count = interp.q("qa-cache-run-count");
                assert!(matches!(interp.symbol_value(expand_count), Value::Int(1)));
                assert!(matches!(interp.symbol_value(run_count), Value::Int(2)));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_shared_form_occurrences_expand_independently() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq qa-shared-expand-count 0 qa-shared-run-count 0)\
                         (defmacro qa-shared-macro ()\
                           (setq qa-shared-expand-count (1+ qa-shared-expand-count))\
                           '(setq qa-shared-run-count (1+ qa-shared-run-count)))",
                    )
                    .unwrap();

                let callsite = Value::list(vec![Value::Sym(interp.q("qa-shared-macro"))]);
                let shared_arguments = Value::list(vec![
                    Value::Sym(interp.q("list")),
                    callsite.clone(),
                    callsite.clone(),
                ]);
                let shared_progn = Value::list(vec![
                    Value::Sym(interp.q("progn")),
                    callsite.clone(),
                    callsite.clone(),
                ]);
                let function = interp.intern("qa-shared-macro-callsite");
                interp.obarray.symbol_mut(function).function = Value::Lambda(Rc::new(Lambda {
                    is_macro: false,
                    required: Vec::new(),
                    optional: Vec::new(),
                    rest: None,
                    body: vec![
                        callsite.clone(),
                        callsite.clone(),
                        shared_progn,
                        shared_arguments,
                    ],
                    body_value: None,
                    env: None,
                    doc: None,
                    interactive: None,
                    name: None,
                    bad_arglist: false,
                    arglist: Some(Value::Nil),
                    plain: false,
                    dumped_doc: false,
                    advice_link: None,
                    compiled: true,
                    bc_items: None,
                    bc_closure: false,
                    bc_prog: RefCell::new(None),
                    compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
                    doc_value: None,
                    env_value: None,
                }));

                let result = interp.eval_str("(qa-shared-macro-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&result), "(5 6)");
                let expand_count = interp.q("qa-shared-expand-count");
                let run_count = interp.q("qa-shared-run-count");
                assert!(matches!(interp.symbol_value(expand_count), Value::Int(6)));
                assert!(matches!(interp.symbol_value(run_count), Value::Int(6)));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_shared_special_form_branches_keep_distinct_expansions() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq qa-branch-expand-count 0 qa-branch-run-count 0 \
                               qa-branch-choice t)\
                         (defmacro qa-branch-macro ()\
                           (setq qa-branch-expand-count (1+ qa-branch-expand-count))\
                           '(setq qa-branch-run-count (1+ qa-branch-run-count)))",
                    )
                    .unwrap();

                let callsite = Value::list(vec![Value::Sym(interp.q("qa-branch-macro"))]);
                let function = interp.intern("qa-shared-branch-callsite");
                let conditional = Value::list(vec![
                    Value::Sym(sym::IF),
                    Value::Sym(interp.q("qa-branch-choice")),
                    callsite.clone(),
                    callsite,
                ]);
                interp.obarray.symbol_mut(function).function = Value::Lambda(Rc::new(Lambda {
                    is_macro: false,
                    required: Vec::new(),
                    optional: Vec::new(),
                    rest: None,
                    body: vec![conditional],
                    body_value: None,
                    env: None,
                    doc: None,
                    interactive: None,
                    name: None,
                    bad_arglist: false,
                    arglist: Some(Value::Nil),
                    plain: false,
                    dumped_doc: false,
                    advice_link: None,
                    compiled: true,
                    bc_items: None,
                    bc_closure: false,
                    bc_prog: RefCell::new(None),
                    compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
                    doc_value: None,
                    env_value: None,
                }));

                let first = interp.eval_str("(qa-shared-branch-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&first), "1");
                interp.eval_str("(setq qa-branch-choice nil)").unwrap();
                let second = interp.eval_str("(qa-shared-branch-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&second), "2");
                let expand_count = interp.q("qa-branch-expand-count");
                let run_count = interp.q("qa-branch-run-count");
                assert!(matches!(interp.symbol_value(expand_count), Value::Int(2)));
                assert!(matches!(interp.symbol_value(run_count), Value::Int(2)));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn compiled_shared_let_and_setq_occurrences_expand_independently() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq qa-let-setq-expand-count 0 qa-let-setq-run-count 0)\
                         (defmacro qa-let-setq-macro ()\
                           (setq qa-let-setq-expand-count\
                                 (1+ qa-let-setq-expand-count))\
                           '(setq qa-let-setq-run-count\
                                  (1+ qa-let-setq-run-count)))",
                    )
                    .unwrap();

                let callsite = Value::list(vec![Value::Sym(interp.q("qa-let-setq-macro"))]);
                let left = interp.intern("qa-let-setq-left");
                let right = interp.intern("qa-let-setq-right");
                let x = interp.intern("qa-let-setq-x");
                let y = interp.intern("qa-let-setq-y");
                let setq_form = Value::list(vec![
                    Value::Sym(sym::SETQ),
                    Value::Sym(left),
                    callsite.clone(),
                    Value::Sym(right),
                    callsite.clone(),
                ]);
                let let_form = Value::list(vec![
                    Value::Sym(sym::LET),
                    Value::list(vec![
                        Value::list(vec![Value::Sym(x), callsite.clone()]),
                        Value::list(vec![Value::Sym(y), callsite.clone()]),
                    ]),
                    Value::list(vec![
                        Value::Sym(interp.q("list")),
                        Value::Sym(x),
                        Value::Sym(y),
                    ]),
                ]);
                let function = interp.intern("qa-shared-let-setq-callsite");
                interp.obarray.symbol_mut(function).function = Value::Lambda(Rc::new(Lambda {
                    is_macro: false,
                    required: Vec::new(),
                    optional: Vec::new(),
                    rest: None,
                    body: vec![setq_form, let_form],
                    body_value: None,
                    env: None,
                    doc: None,
                    interactive: None,
                    name: None,
                    bad_arglist: false,
                    arglist: Some(Value::Nil),
                    plain: false,
                    dumped_doc: false,
                    advice_link: None,
                    compiled: true,
                    bc_items: None,
                    bc_closure: false,
                    bc_prog: RefCell::new(None),
                    compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
                    doc_value: None,
                    env_value: None,
                }));

                let result = interp.eval_str("(qa-shared-let-setq-callsite)").unwrap();
                assert_eq!(interp.prin1_to_string(&result), "(3 4)");
                let expand_count = interp.q("qa-let-setq-expand-count");
                let run_count = interp.q("qa-let-setq-run-count");
                assert!(matches!(interp.symbol_value(expand_count), Value::Int(4)));
                assert!(matches!(interp.symbol_value(run_count), Value::Int(4)));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod progn_cycle_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn circular_progn_body_evaluates_prefix_before_later_error() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let side_effect = interp.intern("qa-circular-progn-side-effect");
        let form = Value::list(vec![
            Value::Sym(sym::SETQ),
            Value::Sym(side_effect),
            Value::Sym(sym::T),
        ]);
        let failure = Value::list(vec![
            Value::Sym(interp.q("let")),
            Value::list(vec![Value::list(vec![Value::Int(1), Value::Int(2)])]),
            Value::Nil,
        ]);
        let second = Value::cons(failure, Value::Nil);
        let body = Value::cons(form, second.clone());
        if let Value::Cons(cell) = &second {
            cell.borrow_mut().cdr = body.clone();
        }

        let result = interp.eval_progn(&body);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
        ));
        assert!(interp.symbol_value(side_effect).truthy());
    }
}

/// `match-data` contents after a successful search.
#[derive(Clone)]
pub struct MatchData {
    /// Group start/end pairs (0-based char offsets in the searched text).
    pub regs: Vec<Option<usize>>,
    /// True if the match was against a buffer (positions are 1-based
    /// Emacs positions); false for strings (0-based indices).
    pub in_buffer: bool,
    /// Base offset of the searched text in the buffer (0 for strings).
    pub base: usize,
    /// The searched object remains identifiable after its buffer is killed,
    /// as in GNU's last_thing_searched.  Markers then detach, but integer
    /// match data still includes this dead buffer and retains raw positions.
    pub buffer: Option<crate::lisp::value::BufferRef>,
}

#[cfg(test)]
mod top_level_borrow_tests {
    use super::*;

    #[test]
    fn eval_reports_a_mutably_borrowed_top_level_form_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let form = Value::cons(Value::Sym(sym::QUOTE), Value::Nil);
                let Value::Cons(cell) = &form else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.eval(&form),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn eval_reports_a_mutably_borrowed_operator_form_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let operator = Value::list(vec![
                    Value::Sym(sym::LAMBDA),
                    Value::Nil,
                    Value::Sym(sym::T),
                ]);
                let form = Value::cons(operator.clone(), Value::Nil);
                let Value::Cons(cell) = &operator else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.eval(&form),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod progn_borrow_tests {
    use super::*;

    #[test]
    fn eval_progn_reports_a_mutably_borrowed_spine_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let forms = Value::cons(Value::Sym(sym::T), Value::Nil);
                let Value::Cons(cell) = &forms else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.eval_progn(&forms),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod function_borrow_tests {
    use super::*;

    #[test]
    fn call_function_and_apply_report_mutably_borrowed_function_cons() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let function = Value::list(vec![
                    Value::Sym(sym::LAMBDA),
                    Value::Nil,
                    Value::Sym(sym::T),
                ]);
                let Value::Cons(cell) = &function else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.call_function(&function, &Value::Nil, None),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                assert!(matches!(
                    interp.apply(&function, Vec::<Value>::new()),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod argument_borrow_tests {
    use super::*;

    #[test]
    fn eval_args_len_reports_a_mutably_borrowed_spine_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let args = Value::cons(Value::Sym(sym::T), Value::Nil);
                let Value::Cons(cell) = &args else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.eval_args_len(&args, 1),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn want_list_reports_a_mutably_borrowed_spine_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let list = Value::cons(Value::Int(1), Value::Nil);
                let Value::Cons(cell) = &list else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    crate::lisp::builtins::want_list(&mut interp, &list),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn want_list_preserves_dotted_and_circular_list_errors() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let tail = Value::Sym(interp.intern("qa-want-list-dotted-tail"));
                let dotted = Value::cons(Value::Int(1), tail.clone());
                assert!(matches!(
                    crate::lisp::builtins::want_list(&mut interp, &dotted),
                    Err(Flow::Signal(ref signal))
                        if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
                ));

                let circular = Value::cons(Value::Int(1), Value::Nil);
                let Value::Cons(cell) = &circular else {
                    unreachable!();
                };
                cell.borrow_mut().cdr = circular.clone();
                assert!(matches!(
                    crate::lisp::builtins::want_list(&mut interp, &circular),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod macro_cell_borrow_tests {
    use super::*;

    #[test]
    fn eval_reports_a_mutably_borrowed_macro_function_cell_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let function = interp.intern("qa-borrowed-macro-cell");
                let macro_cell = Value::cons(Value::Sym(sym::MACRO), Value::Nil);
                interp.fset(function, macro_cell.clone());
                let Value::Cons(cell) = &macro_cell else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();
                let call = Value::cons(Value::Sym(function), Value::Nil);

                assert!(matches!(
                    interp.eval(&call),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod function_p_borrow_tests {
    use super::*;

    #[test]
    fn function_p_does_not_panic_for_mutably_borrowed_lambda_forms() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let lambda = Value::list(vec![
                    Value::Sym(sym::LAMBDA),
                    Value::Nil,
                    Value::Sym(sym::T),
                ]);
                let Value::Cons(cell) = &lambda else {
                    unreachable!();
                };
                let borrow = cell.borrow_mut();
                assert!(!interp.function_p(&lambda));
                drop(borrow);
                assert!(interp.function_p(&lambda));

                let symbol = interp.intern("qa-borrowed-function-p-symbol");
                interp.fset(symbol, lambda.clone());
                let function_cell = interp.symbol_function(symbol);
                let Value::Cons(cell) = &function_cell else {
                    unreachable!();
                };
                let borrow = cell.borrow_mut();
                assert!(!interp.function_p(&Value::Sym(symbol)));
                drop(borrow);
                assert!(interp.function_p(&Value::Sym(symbol)));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod lambda_arglist_borrow_tests {
    use super::*;

    #[test]
    fn apply_reports_a_mutably_borrowed_raw_lambda_arglist_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let parameter = Value::Sym(interp.intern("qa-borrowed-lambda-arg"));
                let arglist = Value::cons(parameter, Value::Nil);
                let function = Value::list(vec![
                    Value::Sym(sym::LAMBDA),
                    arglist.clone(),
                    Value::Sym(sym::T),
                ]);
                let Value::Cons(cell) = &arglist else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.apply(&function, vec![Value::Int(1)]),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod lambda_header_borrow_tests {
    use super::*;

    #[test]
    fn apply_reports_a_mutably_borrowed_raw_lambda_header_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let parameter = Value::Sym(interp.intern("qa-borrowed-lambda-header-arg"));
                let arglist = Value::cons(parameter, Value::Nil);
                let function =
                    Value::list(vec![Value::Sym(sym::LAMBDA), arglist, Value::Sym(sym::T)]);
                let Value::Cons(root) = &function else {
                    unreachable!();
                };
                let header = root.borrow().cdr.clone();
                let Value::Cons(header) = header else {
                    unreachable!();
                };
                let _borrow = header.borrow_mut();

                assert!(matches!(
                    interp.apply(&function, vec![Value::Int(1)]),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod lambda_constructor_borrow_tests {
    use super::*;

    #[test]
    fn parse_lambda_reports_a_mutably_borrowed_arglist_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let parameter = Value::Sym(interp.intern("qa-borrowed-parse-lambda-arg"));
                let arglist = Value::cons(parameter, Value::Nil);
                let Value::Cons(cell) = &arglist else {
                    unreachable!();
                };
                let borrow = cell.borrow_mut();

                assert!(matches!(
                    interp.parse_lambda(&arglist, &[Value::Sym(sym::T)], None),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                drop(borrow);

                let documentation = Value::Sym(interp.intern(":documentation"));
                let form = Value::list(vec![documentation, Value::Sym(sym::T)]);
                let Value::Cons(cell) = &form else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();
                assert!(matches!(
                    interp.parse_lambda(&Value::Nil, std::slice::from_ref(&form), None),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn function_from_lambda_reports_mutably_borrowed_forms_without_panicking() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let cdr = Value::cons(Value::Nil, Value::Nil);
                let Value::Cons(cell) = &cdr else {
                    unreachable!();
                };
                let borrow = cell.borrow_mut();
                assert!(matches!(
                    interp.function_from_lambda(&cdr),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                drop(borrow);

                let documentation = Value::Sym(interp.intern(":documentation"));
                let form = Value::list(vec![documentation, Value::Sym(sym::T)]);
                let body = Value::cons(form.clone(), Value::Nil);
                let cdr = Value::cons(Value::Nil, body);
                let Value::Cons(cell) = &form else {
                    unreachable!();
                };
                let _borrow = cell.borrow_mut();
                assert!(matches!(
                    interp.function_from_lambda(&cdr),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
