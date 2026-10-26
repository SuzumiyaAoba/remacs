//! Advice chain construction, inspection, and recomposition.

use super::{Interp, lexenv_lookup};
use crate::lisp::obarray::sym;
use crate::lisp::{
    error::{EvalResult, Flow},
    value::{SymId, Value},
};
use std::rc::Rc;

impl Interp {
    /// Copy of SYM's `advice-add' entries: (WHERE FUN . NAME) triples.
    pub fn advice_list(&self, sym: SymId) -> Vec<(SymId, Value, Value)> {
        self.advices
            .iter()
            .find(|(s, _)| *s == sym)
            .map(|(_, a)| a.clone())
            .unwrap_or_default()
    }

    /// Synthesize `(lambda (&rest a) (apply SUBR IDX a))'.
    /// The dispatcher subr is embedded directly rather than looked up
    /// through `cl--advice--apply' — GNU keeps that name void at -Q.
    fn advice_trampoline(&mut self, idx: usize) -> EvalResult {
        let fn_sym = self.q("cl--advice--apply");
        let prop = self.q("cl--advice--apply--fn");
        let mut subr = self.get_prop(fn_sym, prop);
        if !matches!(subr, Value::Subr(_)) {
            // Before the -Q voiding pass the dispatcher still lives in
            // the public function cell (e.g. add-function calls while
            // loading dumped libraries).
            subr = self.symbol_function(fn_sym);
        }
        if !matches!(subr, Value::Subr(_)) {
            return Err(self.error("internal: advice dispatcher missing"));
        }
        let cl_args = self.q("cl--args");
        let form = Value::list(vec![
            Value::Sym(self.q("lambda")),
            Value::list(vec![Value::Sym(self.q("&rest")), Value::Sym(cl_args)]),
            Value::list(vec![
                Value::Sym(self.q("apply")),
                subr,
                Value::Int(idx as i128),
                Value::Sym(cl_args),
            ]),
        ]);
        let mut lam = self.lambda_from_form(&form, None)?;
        lam.advice_link = Some(idx);
        Ok(Value::Lambda(Rc::new(lam)).tracked())
    }

    /// Compose KEY's advice entries into the function-cell value: each
    /// entry wraps the previous layer, most recently added outermost —
    /// like GNU's nested `advice' oclosures.  The innermost `next' is
    /// KEY's `advice_bases' entry.  A `(macro . X)' base composes inside
    /// the macro wrapper so the advice runs at expansion, as in GNU.
    pub(crate) fn compose_advice(&mut self, key: SymId) -> EvalResult {
        let base = self
            .advice_bases
            .iter()
            .find(|(s, _)| *s == key)
            .map(|(_, b)| b.clone())
            .unwrap_or_else(|| self.symbol_function(key));
        let mut next = base;
        let mut macrop = false;
        match &next {
            Value::Cons(c) => {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    macrop = true;
                    next = cdr;
                }
            }
            Value::Lambda(l) if l.is_macro => macrop = true,
            _ => {}
        }
        for (w, f, n) in self.advice_list(key) {
            let idx = self.advice_links.len();
            self.advice_links.push((Value::Sym(w), f, next, n));
            next = self.advice_trampoline(idx)?;
        }
        if macrop {
            next = Value::cons(Value::Sym(sym::MACRO), next);
        }
        Ok(next)
    }

    /// Strip advice trampolines off F, returning the innermost
    /// (pre-advice) function — keeping a `(macro . F)' wrapper if F
    /// itself was macro-wrapped.  Follows `advice_links' entries via
    /// `Lambda.advice_link' and nadvice oclosure layers via their
    /// `nf' env binding.
    pub(crate) fn unadvised_base(&self, f: &Value) -> Value {
        let mut v = f.clone();
        let mut was_macro_cons = false;
        loop {
            match &v {
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if self.sym_is(&car, sym::MACRO) {
                        was_macro_cons = true;
                        v = cdr;
                        continue;
                    }
                    break;
                }
                Value::Lambda(l) => {
                    if let Some(idx) = l.advice_link {
                        if let Some((_, _, next, _)) = self.advice_links.get(idx) {
                            v = next.clone();
                            continue;
                        }
                    }
                    // nadvice's `advice--make' layers bind `nf' (next
                    // function) and `how' in the layer's env — hop
                    // through `nf' to reach the base.  Require `how'
                    // to hold a keyword so unrelated closures that
                    // capture `nf' aren't stripped.
                    if let Some(next) = self.advice_layer_next(l) {
                        v = next;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        if was_macro_cons {
            Value::cons(Value::Sym(sym::MACRO), v)
        } else {
            v
        }
    }

    /// If L is a nadvice `advice' oclosure layer (type tag `advice'
    /// in the closure's docstring slot, `cdr' bound to the next
    /// layer, `how' a keyword), return the next layer.
    fn advice_layer_next(&self, l: &crate::lisp::value::Lambda) -> Option<Value> {
        let advice_tag = self.q_soft("advice")?;
        match &l.doc_value {
            Some(Value::Sym(s)) if *s == advice_tag => {}
            _ => return None,
        }
        let cdr = self.q_soft("cdr")?;
        let how = self.q_soft("how")?;
        let how_v = lexenv_lookup(&l.env, how);
        let is_layer = matches!(&how_v, Some(Value::Sym(s)) if {
            let n = self.symbol_name(*s);
            n.starts_with(':')
        });
        if !is_layer {
            return None;
        }
        lexenv_lookup(&l.env, cdr)
    }

    /// Recompute KEY's function cell after its advice entries changed:
    /// empty → restore the captured base; otherwise → recomposed chain.
    pub(crate) fn recompose_advice(&mut self, key: SymId) -> Result<(), Flow> {
        if self
            .advices
            .iter()
            .find(|(s, _)| *s == key)
            .map(|(_, a)| a.is_empty())
            .unwrap_or(true)
        {
            if let Some(pos) = self.advice_bases.iter().position(|(s, _)| *s == key) {
                let base = self.advice_bases.remove(pos).1;
                self.fset(key, base);
            }
            return Ok(());
        }
        let composed = self.compose_advice(key)?;
        self.fset(key, composed);
        Ok(())
    }

    /// If V is an advice trampoline, its `(WHERE FUN NEXT NAME)'.
    pub fn advice_link_entry(&self, v: &Value) -> Option<(Value, Value, Value, Value)> {
        if let Value::Lambda(l) = v {
            if let Some(idx) = l.advice_link {
                return self.advice_links.get(idx).cloned();
            }
        }
        None
    }

    /// Peel advice layers: innermost `next' of a trampoline chain —
    /// the base definition GNU reaches via `advice--cd*r'.
    pub fn advice_base_value(&self, v: &Value) -> Value {
        let mut cur = v.clone();
        while let Some((_, _, next, _)) = self.advice_link_entry(&cur) {
            cur = next;
        }
        cur
    }
}
