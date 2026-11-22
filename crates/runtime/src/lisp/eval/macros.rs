//! Macro call context, expansion loops, and GNU argument-error descriptors.

use super::Interp;
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    value::{Arity, Value},
};
use rustc_hash::FxHashSet;
use std::rc::Rc;

impl Interp {
    /// Expand a macro call in eval position.  GNU `eval_sub' specbinds
    /// `lexical-binding' around the expander call: `t' when
    /// `internal-interpreter-environment' is non-nil (lexical context),
    /// nil otherwise — so `named-let' & friends see whether the emitted
    /// code will run lexically.  Only the EVAL-driven path binds this;
    /// explicit `macroexpand'/`macroexpand-all' calls leave the
    /// variable alone (verified on GNU 31.1).
    pub fn eval_macro_expand(&mut self, mac: &Value, args: &Value) -> EvalResult {
        let mark = self.specbind_depth();
        let lb = self.q("lexical-binding");
        let lex_val = if self.lexenv.is_some() {
            Value::Sym(sym::T)
        } else {
            Value::Nil
        };
        if let Err(e) = self.specbind(lb, lex_val) {
            self.unbind_to(mark)?;
            return Err(e);
        }
        let compiled = self.current_body_compiled
            || self.current_lambda_dumped
            || self.loading_compiled_library
            || self.boot_loading
            || self.loading_dumped;
        let saved_expansion = std::mem::replace(&mut self.compiled_macro_expansion, compiled);
        let r = self.macro_expand_call(mac, args);
        self.compiled_macro_expansion = saved_expansion;
        self.unbind_to(mark)?;
        r
    }

    /// Expand a macro call: call the macro's function on raw args.
    pub fn macro_expand_call(&mut self, mac: &Value, args: &Value) -> EvalResult {
        if self.dbg_flags & 0x200000 != 0 {
            eprintln!(
                "mxcall {} <- {}",
                self.prin1_to_string(mac)
                    .chars()
                    .take(90)
                    .collect::<crate::lisp::text::EmacsText>(),
                self.prin1_to_string(args)
                    .chars()
                    .take(90)
                    .collect::<crate::lisp::text::EmacsText>()
            );
        }
        // Compiled callsites embed the expansion rather than an advised
        // macro function cell. This also covers eager load expansion paths
        // that invoke macro_expand_call without going through eval.rs.
        let expanded_macro = if self.compiled_macro_expansion {
            self.unadvised_base(mac)
        } else {
            mac.clone()
        };
        let mac = &expanded_macro;
        let argv = crate::lisp::builtins::want_list(self, args)?;
        self.macroexp_call_depth += 1;
        // GNU byte-compiles its bundled libraries, so macro expansion
        // of their call sites ran at byte-compile time — before any
        // runtime advice could be installed.  remacs interprets them
        // and re-expands lazily, so when the expansion happens inside
        // compiled-equivalent code (a `compiled'/dumped lambda or a
        // bundled load), run the expander with advice stripped from
        // symbol resolution: otherwise advice added earlier in the
        // session (e.g. (advice-add 'car ...)) corrupts expansions
        // whose expanders call the advised name — `letrec''s
        // (mapcar #'car binders) producing ((ad done) ...) binders.
        let unadvise = self.compiled_macro_expansion;
        if unadvise {
            self.expand_unadvised_depth += 1;
        }
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) => self
                .call_lambda(l, argv.into(), mac)
                .map_err(|f| self.macroexp_wna(mac, f)),
            Value::Cons(_) => {
                let c = match mac {
                    Value::Cons(c) => c,
                    _ => unreachable!(),
                };
                match super::cons_parts(self, c) {
                    Ok((car, cdr)) if self.sym_is(&car, sym::MACRO) => {
                        // Unwrap exactly one `(macro . FUNCTION)' cell, then
                        // funcall FUNCTION. If FUNCTION is itself a macro
                        // cell, GNU reports `invalid-function' for that
                        // object; recursively expanding it loops forever
                        // when the cell is circular.
                        self.apply(&cdr, argv)
                            .map_err(|f| self.macroexp_wna(&cdr, f))
                    }
                    Ok(_) => self.apply(mac, argv).map_err(|f| self.macroexp_wna(mac, f)),
                    Err(error) => Err(error),
                }
            }
            _ => self.apply(mac, argv).map_err(|f| self.macroexp_wna(mac, f)),
        };
        if unadvise {
            self.expand_unadvised_depth -= 1;
        }
        self.macroexp_call_depth -= 1;
        result
    }

    /// `macro_expand_call' wna datum: GNU's expander is a byte-code
    /// object, so `exec_byte_code' reports `(MIN . NONREST)' — not the
    /// function.  remacs stores dumped expanders as `Subr's; translate
    /// a `wrong-number-of-arguments' from the expander call to the same
    /// cons datum (`when' expands to ((1 . 1) 0) like GNU's 0x181
    /// argdesc — `Arity::Many' has no nonrest, so min serves).
    fn macroexp_wna(&mut self, mac: &Value, f: Flow) -> Flow {
        if let Flow::Signal(signal) = &f {
            if signal.is_symbol(sym::WRONG_NUMBER_OF_ARGUMENTS) {
                if let Some((min, max)) = match mac {
                    Value::Subr(s) => match s.arity {
                        Arity::Range { min, max } => Some((min as i128, max as i128)),
                        Arity::Many { min } => Some((min as i128, min as i128)),
                        Arity::Unevalled => None,
                    },
                    // `#[ARGDESC ...]' compiled macro expander (`.elc'
                    // twin): GNU exec_byte_code reports `(MIN . NONREST)' —
                    // `when' → ((1 . 1) 0) from argdesc 385. Compiled
                    // list-argdesc twins use the same shape.
                    // Ordinary user macro functions report themselves as the
                    // WNA datum, including interpreted closures and
                    // `make-byte-code' products. Only compiled-equivalent
                    // expanders on GNU's nativecomp surface report their
                    // packed arity descriptor.
                    Value::Lambda(l) if !l.compiled => None,
                    Value::Lambda(l) => match &l.bc_items {
                        Some(items) => match items.try_borrow() {
                            Ok(items) => match items.first() {
                                Some(argdesc @ (Value::Int(_) | Value::Cons(_) | Value::Nil)) => {
                                    let parsed =
                                        crate::lisp::bytecode::parse_argdesc(self, argdesc);
                                    Some((
                                        parsed.req as i128,
                                        if parsed.has_rest {
                                            1
                                        } else {
                                            parsed.reported_max
                                        },
                                    ))
                                }
                                _ => None,
                            },
                            Err(_) => None,
                        },
                        None => {
                            let al = l.arglist.clone().unwrap_or(Value::Nil);
                            crate::lisp::builtins::misc::raw_arglist_arity(self, &al).map(|a| {
                                match a {
                                    crate::lisp::builtins::misc::FunctionArity::Range {
                                        min,
                                        max,
                                    } => (min as i128, max as i128),
                                    crate::lisp::builtins::misc::FunctionArity::Many { min } => {
                                        (min as i128, 1)
                                    }
                                    crate::lisp::builtins::misc::FunctionArity::Unevalled => (0, 1),
                                }
                            })
                        }
                    },
                    _ => None,
                } {
                    let n = signal
                        .data()
                        .list_to_vec()
                        .ok()
                        .and_then(|v| v.get(1).cloned())
                        .unwrap_or(Value::Int(0));
                    return self.signal_data(
                        sym::WRONG_NUMBER_OF_ARGUMENTS,
                        vec![Value::cons(Value::Int(min), Value::Int(max)), n],
                    );
                }
            }
        }
        f
    }

    /// `macroexpand`: repeatedly expand while the form is a macro call.
    pub fn macroexpand(&mut self, form: &Value) -> EvalResult {
        self.macroexpand_env(form, &Value::Nil)
    }

    /// GNU `macroexpand' (eval.c): ENV is an explicit alist of local
    /// macro definitions shadowing the global ones.  Unlike
    /// `macroexpand-all', it does not consult the dynamically bound
    /// `macroexpand-all-environment' variable.
    pub fn macroexpand_env(&mut self, form: &Value, env: &Value) -> EvalResult {
        self.maybe_quit()?;
        let mut cur = form.clone();
        let mut iters = 0;
        loop {
            iters += 1;
            if self.dbg_flags & 0x200000 != 0 && iters > 500 {
                eprintln!(
                    "macroexpand iter {iters}: {}",
                    self.prin1_to_string(&cur)
                        .chars()
                        .take(200)
                        .collect::<crate::lisp::text::EmacsText>()
                );
            }
            let next = match &cur {
                Value::Cons(c) => {
                    let (car, cdr) = super::cons_parts(self, c)?;
                    // GNU `macroexpand-1' (eval.c): a `(lambda ...)'
                    // form is a function expression whose expansion is
                    // `(function (lambda ...))'.  Since that result is a
                    // fixpoint for `macroexpand', return it directly.
                    if self.sym_is(&car, sym::LAMBDA) {
                        return Ok(Value::list(vec![Value::Sym(sym::FUNCTION), cur.clone()]));
                    }
                    // Enabled `symbols-with-pos' cars expand through
                    // their bare symbol's function cell.
                    let car = crate::lisp::builtins::misc::unpos(self, &car);
                    match car {
                        Value::Sym(id) => {
                            // GNU `macroexpand-1' consults the ENVIRONMENT
                            // argument first.  An entry (SYM . DEF) shadows
                            // SYM's global definition: a nil DEF stops
                            // expansion, otherwise DEF is applied to the
                            // form's argument list (this is how GNU
                            // `cl-flet'/`rx-let' rewrite calls).
                            let mut env_hit = false;
                            let mut env_def = Value::Nil;
                            let mut tail = env.clone();
                            let mut env_seen = FxHashSet::default();
                            let mut until_quit = 1024usize;
                            loop {
                                match tail.clone() {
                                    Value::Nil => break,
                                    Value::Cons(cc) => {
                                        if !env_seen.insert(Rc::as_ptr(&cc) as usize) {
                                            return Err(self.signal_data(
                                                sym::CIRCULAR_LIST,
                                                vec![tail.clone()],
                                            ));
                                        }
                                        let (a, d) = super::cons_parts(self, &cc)?;
                                        if let Value::Cons(e) = &a {
                                            let (ek, ev) = super::cons_parts(self, e)?;
                                            // Enabled sympos keys unwrap to
                                            // their bare symbol id (GNU's
                                            // `assq' via `macroexpand-1'
                                            // compares with `eq').  `assq'
                                            // semantics: the FIRST match
                                            // wins — inner bindings are
                                            // consed in front of outer ones
                                            // (cl-flet shadowing a
                                            // cl-macrolet binding relies on
                                            // it).
                                            if self.sym_id(&ek) == Some(id) {
                                                env_hit = true;
                                                env_def = ev;
                                                break;
                                            }
                                        }
                                        tail = d;
                                        until_quit -= 1;
                                        if until_quit == 0 {
                                            self.maybe_quit()?;
                                            until_quit = 1024;
                                        }
                                    }
                                    // Match GNU's `assq': report an improper
                                    // alist using the original environment,
                                    // but only if lookup reaches its bad tail.
                                    _ => return Err(self.wrong_type_mut("listp", env)),
                                }
                            }
                            if env_hit {
                                if env_def.is_nil() {
                                    return Ok(cur);
                                }
                                let argl = crate::lisp::builtins::want_list(self, &cdr)?;
                                let new = self.apply(&env_def, argl)?;
                                // GNU macroexpand-1 stops when the expander
                                // returns the identical object (the
                                // `cl--labels-convert' cache relies on it).
                                if crate::lisp::builtins::eq_values(&new, &cur) {
                                    return Ok(cur);
                                }
                                cur = new;
                                continue;
                            }
                            let mut f = if self.compiled_macro_expansion || self.boot_loading {
                                self.form_function(id)
                            } else {
                                self.symbol_function(id)
                            };
                            // Inspect the same public alias chain GNU's
                            // macroexpand follows, without autoloading
                            // until the target's macro type is known.
                            if matches!(&f, Value::Sym(alias) if *alias != sym::UNBOUND) {
                                f = crate::lisp::builtins::data::f_indirect_function(
                                    self,
                                    vec![Value::Sym(id)].into(),
                                )?;
                            }
                            // Autoload cell: resolve macro autoloads
                            // (TYPE non-nil); others stop expansion.
                            let auto_id = self.q("autoload");
                            let is_auto = match &f {
                                Value::Cons(cc) => {
                                    let car = super::cons_parts(self, cc)?.0;
                                    self.sym_is(&car, auto_id)
                                }
                                _ => false,
                            };
                            if is_auto {
                                f = crate::lisp::builtins::evalfn::autoload_do_load(self, f, true)?;
                                let still_auto = match &f {
                                    Value::Cons(cc) => {
                                        let car = super::cons_parts(self, cc)?.0;
                                        self.sym_is(&car, auto_id)
                                    }
                                    _ => false,
                                };
                                if still_auto {
                                    return Ok(cur);
                                }
                            }
                            let is_mac = match &f {
                                Value::Lambda(l) => l.is_macro,
                                Value::Cons(cc) => {
                                    let car = super::cons_parts(self, cc)?.0;
                                    self.sym_is(&car, sym::MACRO)
                                }
                                _ => false,
                            };
                            if is_mac {
                                let expanded = self.macro_expand_call(&f, &cdr)?;
                                // GNU stops when an expander returns the
                                // identical form object. Besides terminating
                                // self-returning macros, this preserves the
                                // identity caches used by Lisp macroexpanders.
                                if crate::lisp::builtins::eq_values(&expanded, &cur) {
                                    return Ok(cur);
                                }
                                expanded
                            } else {
                                return Ok(cur);
                            }
                        }
                        _ => return Ok(cur),
                    }
                }
                _ => return Ok(cur),
            };
            cur = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::{obarray::Obarray, value::Lambda};
    use std::{cell::RefCell, rc::Rc, thread};

    #[test]
    fn macroexpand_environment_cycle_signals_circular_list() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let key = interp.intern("qa-macroexpand-unrelated-key");
        let form_head = interp.intern("qa-macroexpand-unrelated-form");
        let env = Value::cons(Value::cons(Value::Sym(key), Value::Nil), Value::Nil);
        if let Value::Cons(cell) = &env {
            cell.borrow_mut().cdr = env.clone();
        }

        let form = Value::list(vec![Value::Sym(form_head)]);
        let result = interp.macroexpand_env(&form, &env);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
        ));
    }

    #[test]
    fn macro_expand_call_restores_depth_when_macro_cell_is_borrowed() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let mac = Value::cons(Value::Sym(sym::MACRO), Value::Nil);
        let Value::Cons(cell) = &mac else {
            unreachable!();
        };
        let borrow = cell.borrow_mut();

        assert!(interp.macro_expand_call(&mac, &Value::Nil).is_err());

        drop(borrow);
        assert_eq!(interp.macroexp_call_depth, 0);
    }

    #[test]
    fn compiled_macro_expand_call_reports_borrowed_macro_cell_without_panicking() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        interp.compiled_macro_expansion = true;
        let mac = Value::cons(Value::Sym(sym::MACRO), Value::Nil);
        let Value::Cons(cell) = &mac else {
            unreachable!();
        };
        let borrow = cell.borrow_mut();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            interp.macro_expand_call(&mac, &Value::Nil)
        }));

        drop(borrow);
        let result = result.expect("compiled macro expansion must not panic");
        assert!(result.is_err());
        assert_eq!(interp.macroexp_call_depth, 0);
        assert_eq!(interp.expand_unadvised_depth, 0);
    }

    #[test]
    fn cyclic_macro_function_cells_signal_invalid_function() {
        thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                // A vacant interpreter has no `error-conditions' registry,
                // so signaling from `apply' would recursively signal
                // `Invalid error symbol'. Use a booted interpreter here.
                let mut interp = Interp::new();
                for compiled in [false, true] {
                    interp.compiled_macro_expansion = compiled;

                    let first = Value::cons(Value::Sym(sym::MACRO), Value::Nil);
                    let second = Value::cons(Value::Sym(sym::MACRO), first.clone());
                    let Value::Cons(first_cell) = &first else {
                        unreachable!();
                    };
                    first_cell.borrow_mut().cdr = second.clone();

                    let result = interp.macro_expand_call(&first, &Value::Nil);

                    // Break the deliberately cyclic Rc graph before dropping it.
                    if let Value::Cons(second_cell) = &second {
                        second_cell.borrow_mut().cdr = Value::Nil;
                    }
                    assert!(matches!(
                        result,
                        Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
                    ));
                    assert_eq!(interp.macroexp_call_depth, 0);
                    assert_eq!(interp.expand_unadvised_depth, 0);

                    let expander =
                        Value::list(vec![Value::Sym(sym::LAMBDA), Value::Nil, Value::Nil]);
                    let inner = Value::cons(Value::Sym(sym::MACRO), expander);
                    let outer = Value::cons(Value::Sym(sym::MACRO), inner);
                    let nested = interp.macro_expand_call(&outer, &Value::Nil);
                    assert!(matches!(
                        nested,
                        Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
                    ));
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn macroexpand_env_reports_a_mutably_borrowed_form_without_panicking() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let head = interp.intern("qa-macroexpand-borrowed-form");
        let form = Value::list(vec![Value::Sym(head)]);
        let Value::Cons(cell) = &form else {
            unreachable!();
        };
        let borrowed = cell.borrow_mut();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            interp.macroexpand_env(&form, &Value::Nil)
        }));
        drop(borrowed);
        let result = result.expect("macroexpand_env must not panic");
        assert!(matches!(result, Err(Flow::Signal(_))));
    }

    #[test]
    fn macroexpand_env_reports_a_mutably_borrowed_environment_without_panicking() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let key = interp.intern("qa-macroexpand-borrowed-env-key");
        let head = interp.intern("qa-macroexpand-borrowed-env-form");
        let form = Value::list(vec![Value::Sym(head)]);
        let env = Value::cons(Value::cons(Value::Sym(key), Value::Nil), Value::Nil);
        let Value::Cons(cell) = &env else {
            unreachable!();
        };
        let borrowed = cell.borrow_mut();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            interp.macroexpand_env(&form, &env)
        }));
        drop(borrowed);
        let result = result.expect("macroexpand_env must not panic");
        assert!(matches!(result, Err(Flow::Signal(_))));
    }

    #[test]
    fn compiled_list_argdesc_macro_wna_does_not_truncate_large_arities() {
        thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::vacant();
                interp.obarray = Obarray::new();
                let arity = 65_536usize;
                let arglist = Value::list(vec![Value::Sym(sym::T); arity]);
                let items = Value::vector_ref(vec![
                    arglist.clone(),
                    Value::string(""),
                    Value::Vec(Value::vector_ref(Vec::new())),
                    Value::Int(0),
                ]);
                let function = Value::Lambda(Rc::new(Lambda {
                    is_macro: true,
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
                    arglist: Some(arglist),
                    plain: false,
                    dumped_doc: false,
                    advice_link: None,
                    compiled: true,
                    bc_items: Some(items),
                    bc_closure: false,
                    bc_prog: RefCell::new(None),
                    compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
                    doc_value: None,
                    env_value: None,
                }));
                let original = interp.wrong_number_of_args(&function, 0);
                let Flow::Signal(signal) = interp.macroexp_wna(&function, original) else {
                    panic!("expected wrong-number-of-arguments")
                };
                let error_data = signal.data().list_to_vec().unwrap();
                let Value::Cons(arity_data) = &error_data[0] else {
                    panic!("expected arity descriptor")
                };
                let arity_data = arity_data.borrow();
                assert_eq!(arity_data.car.int(), Some(arity as i128));
                assert_eq!(arity_data.cdr.int(), Some(arity as i128));
                assert_eq!(error_data[1].int(), Some(0));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
