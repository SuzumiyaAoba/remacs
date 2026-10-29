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

// Keep existing eval::* paths usable by runtime code and external callers.
pub use lexenv::{
    LexEnv, LexFrame, lexenv_as_value, lexenv_declare, lexenv_extend, lexenv_lookup, lexenv_root,
    lexframe_bind,
};
pub(crate) use lexenv::{lexenv_declared, lexenv_from_value, lexenv_lookup_checked, lexenv_set};
pub use plist::{
    plist_get, plist_get_v, plist_lookup, plist_lookup_v, plist_put, plist_put_front, plist_put_v,
};

#[path = "threads.rs"]
pub(crate) mod threads;

/// Read a cons cell's `cdr` without borrowing the `RefCell` or cloning
/// the `Value`.  Spine walks only ever follow `cdr` chains — the cells
/// stay alive through the `Rc` links reachable from the list head, and
/// nothing mutates them while these loops run — so a raw pointer read
/// is safe and skips a borrow + two `Rc` refcount ops per step.  The
/// returned `&'static` is a scoped lie: callers must only use it while
/// the owning list value is alive and unmutated.
pub(crate) unsafe fn cdr_ptr(c: &Rc<RefCell<Cons>>) -> &'static Value {
    unsafe { &(*RefCell::as_ptr(c)).cdr }
}

/// Cons-cell pointer of a `Value::Cons`, or null.
pub(crate) fn cons_ptr(v: &Value) -> *const RefCell<Cons> {
    match v {
        Value::Cons(c) => Rc::as_ptr(c),
        _ => std::ptr::null(),
    }
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
            Value::Nil => self.eval_symbol(sym::NIL),
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
            Value::Sym(id) => self.eval_symbol(*id),
            Value::Record(_) => {
                // `symbols-with-pos-enabled' objects evaluate like
                // their bare symbol (GNU: they are real symbols).
                match crate::lisp::builtins::misc::active_sympos_parts(self, form) {
                    Some((s, _)) => self.eval_symbol(s),
                    None => Ok(form.clone()),
                }
            }
            Value::Cons(_) => self.eval_form(form),
        }
    }

    fn eval_symbol(&mut self, id: SymId) -> EvalResult {
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
                return Err(self.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
            }
        }
        Ok(v)
    }

    /// Evaluate a compound form `(fn . args)`.
    fn eval_form(&mut self, form: &Value) -> EvalResult {
        let Value::Cons(cell) = form else {
            unreachable!()
        };
        let (head, args) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let head = crate::lisp::builtins::misc::unpos(self, &head);
        self.with_pending_frame(head, args, |i, frame| {
            let result = i.eval_form_inner(form, frame);
            i.offer_flow(result)
        })
    }

    fn eval_form_inner(&mut self, form: &Value, frame: usize) -> EvalResult {
        let cons = match form {
            Value::Cons(c) => c,
            _ => unreachable!(),
        };
        let (head, args) = {
            let b = cons.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        // `symbols-with-pos-enabled' symbols in operator position are
        // real symbols (GNU parity): dispatch on the bare symbol.
        let head = crate::lisp::builtins::misc::unpos(self, &head);
        // GNU `eval_sub' runs CHECK_LIST on the arg spine before
        // dispatch (and `list_length' once fun is known), so a dotted
        // call `(f a . b)' signals `wrong-type-argument listp b'
        // before the function — even before a special form's body
        // or `if''s test is evaluated.  Circular spines stay legal:
        // GNU's FOR_EACH_TAIL halts on a repeat, so only a truly
        // non-cons tail is an error.
        let spine_len = {
            // Raw-pointer tortoise/hare over the cdr chain: identical
            // semantics to the Value-clone walk it replaces (nil tail
            // ends the spine, a non-cons tail is `listp' error, a
            // pointer meet means circular and legal).
            let mut slow = &args as *const Value;
            let mut fast = &args as *const Value;
            let mut guard = true;
            let mut spine_len: i128 = 0;
            loop {
                let sv = unsafe { &*slow };
                match sv {
                    Value::Cons(c) => {
                        slow = unsafe { cdr_ptr(c) };
                        spine_len += 1;
                    }
                    Value::Nil => break,
                    other => return Err(self.wrong_type_mut("listp", other)),
                }
                if !guard {
                    continue;
                }
                match unsafe { &*fast } {
                    Value::Cons(c) => {
                        let n1 = unsafe { cdr_ptr(c) };
                        match n1 {
                            Value::Cons(c2) => fast = unsafe { cdr_ptr(c2) },
                            _ => {
                                fast = n1;
                                guard = false;
                            }
                        }
                    }
                    _ => guard = false,
                }
                if cons_ptr(unsafe { &*slow }) == cons_ptr(unsafe { &*fast })
                    && matches!(unsafe { &*slow }, Value::Cons(_))
                {
                    // Circular spine: length is unusable.
                    spine_len = -1;
                    break;
                }
            }
            spine_len
        };

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
                // Special form?  GNU checks each special form's own
                // minimum arity inside the evaluator (Fif needs 2,
                // Fsetq needs pairs, ...): report `(SYM N)'.
                if let Some(sf) = super::special::special_form(id) {
                    let min = super::special::special_form_min_args(id) as i128;
                    let n = match spine_len {
                        n if n >= 0 => n,
                        _ => self.raw_list_length(&args)?,
                    };
                    if n < min {
                        // GNU's `defun'/`defmacro' are byte-coded
                        // macros: their too-few-args error carries the
                        // trampoline arity `(2 . 2)' as the function
                        // datum, not the symbol.
                        let fun = if id == sym::DEFUN || id == sym::DEFMACRO {
                            Value::cons(Value::Int(2), Value::Int(2))
                        } else {
                            Value::Sym(id)
                        };
                        return Err(self.wrong_number_of_args(&fun, n));
                    }
                    return sf(self, args);
                }
                if self.dbg_flags & 0x01 != 0 {
                    self.cur_head_form = form.clone();
                }
                // Function cell.
                let fun = self.form_function(id);
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
                        return Err(self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(id)]));
                    }
                }
                // Macro call: GNU's evaluator re-expands the form on
                // every call, but its dumped libraries run
                // byte-compiled so expansion happens once at compile
                // time.  remacs interprets them — caching the
                // expansion on the call-site cons cell (invalidated
                // by `macro_gen' and the fn cell's identity) keeps
                // interpreted cl-*/pcase/eieio code usable.
                // Inside a `compiled' (dumped-library-equivalent)
                // function, a symbol-head call to a byte-op primitive
                // dispatches to its canonical subr — GNU's compiled
                // `(car x)' emits Bcar, which never reads
                // `symbol-function', so advising `car' can't make the
                // advice machinery's own calls recurse.
                if self.dumped_call_depth > 0 {
                    if let Some(s) = self.byte_op_subr(id) {
                        return self.call_function_len(
                            &Value::Subr(s),
                            &args,
                            Some(id),
                            spine_len,
                            frame,
                        );
                    }
                }
                if self.is_macro_function(&fun) {
                    let key = Rc::as_ptr(cons) as usize;
                    // Inside dumped-library (compiled-equivalent) lambdas
                    // macro calls were expanded at "compile" time — i.e.
                    // against the pre-advice cell.  Strip any advice
                    // trampolines so interpreted nadvice/gv code behaves
                    // like GNU's .elc where `when' is an unadvised `if'.
                    let fun = if self.dumped_call_depth > 0 {
                        self.unadvised_base(&fun)
                    } else {
                        fun
                    };
                    if let Some((stamp, fncell, expansion, _pin)) = self.macro_cache.get(&key) {
                        // `(macro . f)' cells are mutated in place by
                        // nadvice's gv machinery, so compare the cdr
                        // too, not just the cons identity.
                        let same_fn = match (&fun, fncell) {
                            (Value::Cons(a), Value::Cons(b)) => {
                                let (aa, bb) = (a.borrow(), b.borrow());
                                crate::lisp::builtins::eq_values(&aa.car, &bb.car)
                                    && crate::lisp::builtins::eq_values(&aa.cdr, &bb.cdr)
                            }
                            _ => crate::lisp::builtins::eq_values(&fun, fncell),
                        };
                        if *stamp == self.macro_gen && same_fn {
                            let expansion = expansion.clone();
                            return self.eval(&expansion);
                        }
                    }
                    let expansion = self.eval_macro_expand(&fun, &args)?;
                    if self.macro_cache.len() >= 262144 {
                        self.macro_cache.clear();
                    }
                    self.macro_cache
                        .insert(key, (self.macro_gen, fun, expansion.clone(), form.clone()));
                    return self.eval(&expansion);
                }
                self.call_function_len(&fun, &args, Some(id), spine_len, frame)
            }
            Value::Cons(_) => {
                // `((lambda (x) ...) a b)` — function position lambda is
                // used directly, not evaluated as a call.
                let b = match &head {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => unreachable!(),
                };
                if self.sym_is(&b, sym::LAMBDA) {
                    // GNU `eval_sub': `((lambda ...) ARGS)' calls
                    // `funcall_lambda' on the RAW form — dynamic
                    // binding, no closure conversion, `invalid-
                    // function'/`wna' data is the form itself.
                    let argv = self.eval_args_len(&args, spine_len)?;
                    self.finish_pending_frame(frame);
                    return self.apply_resolved(&head, argv, head.clone());
                }
                // GNU uses the cons HEAD itself as the function object:
                // `(lambda ...)'/`(closure ...)'/`(macro . f)' are
                // callable, anything else signals invalid-function with
                // the cons (e.g. (('q)) → invalid-function (quote q)).
                self.call_function_len(&head, &args, None, spine_len, frame)
            }
            _ => {
                let fun = self.eval(&head)?;
                self.call_function_len(&fun, &args, None, spine_len, frame)
            }
        }
    }

    /// Evaluate each element of a list form, using PRE_LEN when the
    /// caller already counted the spine (eval_form's check walk).
    pub fn eval_args_len(&mut self, args: &Value, pre_len: i128) -> Result<Argv, Flow> {
        let cap = if pre_len >= 0 {
            pre_len as usize
        } else {
            self.raw_list_length(args)? as usize
        };
        // GNU eval_sub walks and saves the argument spine before evaluating
        // the argument forms. Keep the cons cells so a later car mutation is
        // visible when that argument is reached, while cdr mutations cannot
        // insert, remove, or replace arguments in the active call.
        let mut cells = Vec::with_capacity(cap);
        let mut cur = args.clone();
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    cur = c.borrow().cdr.clone();
                    cells.push(c);
                }
                _ => {
                    return Err(self.error("Invalid function call form (dotted)"));
                }
            }
        }
        let mut out = Vec::with_capacity(cells.len());
        for cell in cells {
            let car = cell.borrow().car.clone();
            out.push(self.eval(&car)?);
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
        let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
        self.with_pending_frame(shown, args.clone(), |i, frame| {
            let result = i.call_function_len(fun, args, sym_name, -1, frame);
            i.offer_flow(result)
        })
    }

    /// `call_function` with a precomputed spine length (-1 = unknown).
    /// eval_form passes the count its arg-check walk already computed.
    fn call_function_len(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<SymId>,
        spine_len: i128,
        frame: usize,
    ) -> EvalResult {
        let fun = crate::lisp::builtins::misc::unpos(self, fun);
        match &fun {
            Value::Sym(id) => {
                // Function alias chain: chase.  GNU's
                // `indirect_function' signals `cyclic-function-indirection'
                // with the symbol whose alias points back into the chain.
                // Most calls resolve in one hop — the `seen' Vec only
                // allocates once an actual alias step happens.
                let mut cur = *id;
                let mut seen: Option<Vec<SymId>> = None;
                loop {
                    // GNU resolves defalias chains in `eval' before
                    // dispatching, so an alias to a special form
                    // (e.g. `inline' -> `progn') is called as a
                    // special form with unevaluated arguments.
                    if let Some(sf) = super::special::special_form(cur) {
                        let min = super::special::special_form_min_args(cur) as i128;
                        let n = if spine_len >= 0 {
                            spine_len
                        } else {
                            self.raw_list_length(&args)?
                        };
                        if n < min {
                            return Err(self.wrong_number_of_args(&Value::Sym(cur), n));
                        }
                        return sf(self, args.clone());
                    }
                    let f = self.callable_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            if seen.is_none() {
                                seen = Some(vec![cur]);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.q("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![Value::Sym(cur)]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().push(cur);
                        }
                        other => {
                            // Emacs reports the originally called symbol
                            // in arity errors, not the resolved one.
                            return self
                                .call_function_len(&other, args, sym_name, spine_len, frame);
                        }
                    }
                }
            }
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => (s.func)(self, Argv::new(vec![args.clone()])),
                _ => {
                    // GNU checks a subr's arity before evaluating the
                    // argument forms, so `(car BAD EXTRA)' reports
                    // `wrong-number-of-arguments' rather than whatever
                    // BAD would signal.
                    let n = if spine_len >= 0 {
                        spine_len
                    } else {
                        self.raw_list_length(args)?
                    };
                    self.check_arity_subr_n(s, n, sym_name)?;
                    let argv = self.eval_args_len(args, spine_len)?;
                    self.finish_pending_frame(frame);
                    // Record the frame like apply_resolved does — GNU's
                    // specpdl holds every call, so backtraces show the
                    // innermost subr (`car(5)') too.
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    self.profiler_checkpoint();
                    self.lisp_stack.push((shown, argv.clone()));
                    let r = (s.func)(self, argv);
                    let r = self.offer_flow(r);
                    self.profiler_checkpoint();
                    self.lisp_stack.pop();
                    r
                }
            },
            Value::Lambda(l) => {
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
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    self.check_arity_lambda_n(l, n, &shown)?;
                }
                // Dumped `#[ARGDESC ...]' twins (compiled, no parsed
                // arglist) get the same early check — GNU's nativecomp
                // signal data is the called symbol, while apply/funcall
                // truncate excess args instead (exec() does that).
                if !l.dumped_doc && l.compiled {
                    if let Some(items) = &l.bc_items {
                        if super::bytecode::is_byte_code(&items.borrow()) {
                            let n = if spine_len >= 0 {
                                spine_len
                            } else {
                                self.raw_list_length(args)?
                            };
                            let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                            super::bytecode::check_argdesc(
                                self,
                                &items.borrow(),
                                n.max(0) as usize,
                                &shown,
                            )?;
                        }
                    }
                }
                let argv = self.eval_args_len(args, spine_len)?;
                self.finish_pending_frame(frame);
                let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                self.apply_resolved(&fun, argv, shown)
            }
            Value::Cons(_) => {
                // A cons as function: `(lambda ...)` form or `(macro . f)`.
                let (car, cdr) = {
                    let c = match &fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    // (macro . lambda) — expand with cdr as function.
                    let expansion = self.eval_macro_expand(&cdr, args)?;
                    return self.eval(&expansion);
                }
                if self.sym_is(&car, sym::LAMBDA) {
                    // Symbol-function indirection's cons form: GNU
                    // `funcall_lambda' on `(lambda ...)'.  `((function
                    // (lambda ...)))' (QUOTE_FUNCTION head) is NOT a
                    // lambda call — `function' is no macro here, so GNU
                    // `invalid-function' on the cons.
                    let argv = self.eval_args_len(args, spine_len)?;
                    self.finish_pending_frame(frame);
                    let shown = sym_name.map(Value::Sym).unwrap_or_else(|| fun.clone());
                    return self.apply_resolved(&fun, argv, shown);
                }
                let auto_id = self.q("autoload");
                if self.sym_is(&car, auto_id) {
                    // (autoload FILE ...) — load, then re-dispatch on
                    // the real definition (macro autoloads expand).
                    let newdef =
                        crate::lisp::builtins::evalfn::autoload_do_load(self, fun.clone(), false)?;
                    return self.call_function_len(&newdef, args, sym_name, spine_len, frame);
                }
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name.map(|s| self.symbol_name(s)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => {
                if self.dbg_flags & 0x01 != 0 {
                    eprintln!(
                        "[invfun_ {} head={:?} callform={}]",
                        self.princ_to_string(&fun),
                        sym_name.map(|s| self.symbol_name(s)),
                        self.princ_to_string(&self.cur_head_form)
                            .chars()
                            .take(200)
                            .collect::<crate::lisp::text::EmacsText>(),
                    );
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
        }
    }

    /// Length of a raw (unevaluated) argument list: the number of cons
    /// cells in its spine.  A dotted tail is not counted; a circular
    /// list signals `circular-list' like GNU's `Flength'.
    pub(crate) fn raw_list_length(&mut self, args: &Value) -> Result<i128, Flow> {
        // Raw-pointer walk — same tortoise/hare logic as before, minus
        // a `Value` clone + drop per spine cell.
        let mut n = 0i128;
        let mut cur = args as *const Value;
        let mut hare: Option<*const Value> = Some(args);
        loop {
            let next = match unsafe { &*cur } {
                Value::Cons(c) => unsafe { cdr_ptr(c) },
                _ => break,
            };
            n += 1;
            cur = next;
            // Hare advances two cells per step while it can; if it
            // meets `cur' the spine loops.
            if let Some(h) = hare {
                let step = |v: *const Value| -> *const Value {
                    match unsafe { &*v } {
                        Value::Cons(c) => unsafe { cdr_ptr(c) },
                        _ => v,
                    }
                };
                let h1 = step(h);
                if matches!(unsafe { &*h1 }, Value::Cons(_)) {
                    hare = Some(step(h1));
                    if let (Value::Cons(_), Some(hp)) = (unsafe { &*cur }, hare) {
                        if cons_ptr(unsafe { &*cur }) == cons_ptr(unsafe { &*hp }) {
                            return Err(crate::lisp::builtins::listfn::err_circular(self));
                        }
                    }
                } else {
                    hare = None;
                }
            }
        }
        Ok(n)
    }

    fn check_arity_subr(
        &self,
        s: &'static super::value::Subr,
        argv: &[Value],
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        self.check_arity_subr_n(s, argv.len() as i128, name)
    }

    fn check_arity_subr_n(
        &self,
        s: &'static super::value::Subr,
        n: i128,
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        // Emacs reports the calling symbol for eval'd calls, the subr
        // object itself for `funcall'/`apply'.
        let who = name.map_or(Value::Subr(s), Value::Sym);
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
            let fast = matches!(callee, Value::Lambda(ref lambda)
                if lambda.bc_items.as_ref().is_some_and(|items|
                    matches!(items.borrow().first(), Some(Value::Int(_)))));
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
                let car = c.borrow().car.clone();
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
        let fun = crate::lisp::builtins::misc::unpos(self, fun);
        let shown = origin.unwrap_or_else(|| fun.clone());
        match &fun {
            Value::Sym(id) => {
                // Most calls resolve in one hop — the `seen' Vec only
                // allocates once an actual alias step happens.
                let mut cur = *id;
                let mut seen: Option<Vec<SymId>> = None;
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
                                seen = Some(vec![cur]);
                            }
                            if seen.as_ref().unwrap().contains(&next) {
                                let sig = self.q("cyclic-function-indirection");
                                return Err(self.signal_data(sig, vec![shown]));
                            }
                            cur = next;
                            seen.as_mut().unwrap().push(cur);
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
                let argv = if l.dumped_doc
                    && !l.is_macro
                    && !l.bad_arglist
                    && !l.is_bytecode()
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
                let (car, _) = {
                    let c = match fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
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

    /// Evaluate a body (progn), returning the last value.
    pub fn eval_body(&mut self, body: &[Value]) -> EvalResult {
        let mut last = Value::Nil;
        for f in body {
            last = self.eval(f)?;
        }
        Ok(last)
    }

    /// Evaluate a list of forms (a `progn`-style list, not a Vec).
    pub fn eval_progn(&mut self, forms: &Value) -> EvalResult {
        let mut last = Value::Nil;
        let mut cur = forms.clone();
        // Fprogn saves both the form and next cdr before eval_sub. A form
        // may replace its own list cells without changing the saved next
        // tail, while mutations to that next cell remain observable.
        while let Value::Cons(cell) = cur {
            let (form, next) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            cur = next;
            last = self.eval(&form)?;
        }
        // Fprogn ignores atom bodies and improper tails.
        Ok(last)
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
