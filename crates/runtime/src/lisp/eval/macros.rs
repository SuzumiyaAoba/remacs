//! Macro call context, expansion loops, and GNU argument-error descriptors.

use super::Interp;
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    value::{Arity, Value},
};

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
            return Err(e);
        }
        let compiled =
            self.current_lambda_dumped || self.loading_compiled_library || self.boot_loading;
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
        let argv = match args.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.error("bad macro args")),
        };
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
        let unadvise = self.dumped_call_depth > 0 || self.loading_dumped;
        if unadvise {
            self.expand_unadvised_depth += 1;
        }
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) => self
                .call_lambda(l, argv.into(), mac)
                .map_err(|f| self.macroexp_wna(mac, f)),
            Value::Cons(_) => {
                let (car, _) = {
                    let c = match mac {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    let cdr = match mac {
                        Value::Cons(c) => c.borrow().cdr.clone(),
                        _ => unreachable!(),
                    };
                    if unadvise {
                        self.expand_unadvised_depth -= 1;
                    }
                    self.macroexp_call_depth -= 1;
                    return self.macro_expand_call(&cdr, args);
                }
                self.apply(mac, argv).map_err(|f| self.macroexp_wna(mac, f))
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
                        Arity::Range { min, max } => Some((min, max)),
                        Arity::Many { min } => Some((min, min)),
                        Arity::Unevalled => None,
                    },
                    // `#[ARGDESC ...]' byte-code expander (`.elc' twin):
                    // GNU exec_byte_code reports `(MIN . NONREST)' —
                    // `when' → ((1 . 1) 0) from argdesc 385.  List-argdesc
                    // twins (`#[(exp &rest cases) ...]') and interpreted
                    // lambdas get the same `(MIN . NONREST)' shape.
                    Value::Lambda(l) => match &l.bc_items {
                        Some(items) => {
                            let items = items.borrow();
                            match items.first() {
                                Some(argdesc @ (Value::Int(_) | Value::Cons(_) | Value::Nil)) => {
                                    let (req, _max, rest, _) =
                                        crate::lisp::bytecode::parse_argdesc(self, argdesc);
                                    Some((req as u16, if rest { 1 } else { _max as u16 }))
                                }
                                _ => None,
                            }
                        }
                        None => {
                            let al = l.arglist.clone().unwrap_or(Value::Nil);
                            crate::lisp::builtins::misc::raw_arglist_arity(self, &al).map(|a| {
                                match a {
                                    Arity::Range { min, max } => (min, max),
                                    Arity::Many { min } => (min, 1),
                                    Arity::Unevalled => (0, 1),
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
                        vec![
                            Value::cons(Value::Int(min as i128), Value::Int(max as i128)),
                            n,
                        ],
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
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
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
                            loop {
                                match tail {
                                    Value::Cons(cc) => {
                                        let (a, d) = {
                                            let b = cc.borrow();
                                            (b.car.clone(), b.cdr.clone())
                                        };
                                        if let Value::Cons(e) = &a {
                                            let (ek, ev) = {
                                                let b = e.borrow();
                                                (b.car.clone(), b.cdr.clone())
                                            };
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
                                    }
                                    _ => break,
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
                                Value::Cons(cc) => self.sym_is(&cc.borrow().car, auto_id),
                                _ => false,
                            };
                            if is_auto {
                                f = crate::lisp::builtins::evalfn::autoload_do_load(self, f, true)?;
                                let still_auto = match &f {
                                    Value::Cons(cc) => self.sym_is(&cc.borrow().car, auto_id),
                                    _ => false,
                                };
                                if still_auto {
                                    return Ok(cur);
                                }
                            }
                            let is_mac = match &f {
                                Value::Lambda(l) => l.is_macro,
                                Value::Cons(cc) => {
                                    let b = cc.borrow();
                                    self.sym_is(&b.car, sym::MACRO)
                                }
                                _ => false,
                            };
                            if is_mac {
                                self.macro_expand_call(&f, &cdr)?
                            } else {
                                return Ok(cur);
                            }
                        }
                        _ => return Ok(cur),
                    }
                }
                _ => return Ok(cur),
            };
            // GNU stops when a macro returns the exact form it was given.
            // Without this identity check, `macroexpand' loops forever on
            // self-preserving macros.
            if crate::lisp::builtins::eq_values(&next, &cur) {
                return Ok(cur);
            }
            cur = next;
        }
    }
}
