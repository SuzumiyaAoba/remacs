//! Hide undumped helpers and reinstall deferred loaddefs without replacing live definitions.

use super::super::{
    SymbolSnapshot,
    symbols::{GNU_VOID_FNS, GNU_VOID_FNS_LATE},
};
use crate::lisp::{
    eval::Interp,
    obarray::sym,
    reader::Reader,
    value::{SymId, Value},
};
use rustc_hash::FxHashSet as HashSet;
use std::rc::Rc;

pub(super) fn hide_dispatchers(interp: &mut Interp) {
    // The prelude amalgamates helpers that GNU 31.1 keeps void
    // at -Q: they live in libraries that aren't dumped
    // (subr-x.el, help.el, pcase/rx internals, ...) or simply
    // don't exist upstream (`second', `copy-seq', ...).  A
    // static call-graph over the dumped defs shows no kept
    // (GNU-bound) definition calls these, so void their
    // function cells to match GNU's boot state.  The real
    // lisp/*.el files still rebind them on `require'.
    for name in GNU_VOID_FNS {
        let sid = interp.intern(name);
        let old = interp.obarray.symbol(sid).function.clone();
        if *name == "cl--advice--apply" {
            // `advice-add' trampolines call this dispatcher; GNU
            // voids the name at -Q, so stash the subr on the
            // symbol's plist where Lisp code can't reach it via
            // `symbol-function'.
            let pk = interp.q("cl--advice--apply--fn");
            interp.put_prop(sid, pk, old.clone());
        }
        // Stash only a genuine pre-void definition: a cell that
        // already holds an (autoload ...) form (installed by
        // loaddefs after the real def was made) must not be
        // recorded as the "dumped" definition, and an earlier
        // snapshot must not be overwritten either.
        let pk = interp.q("remacs--dump-fn");
        let auto_id = interp.q("autoload");
        let is_autoload = matches!(
            &old,
            Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
        );
        let have_stash = !matches!(
            interp.get_prop(sid, pk),
            v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
        );
        if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !is_autoload && !have_stash {
            interp.put_prop(sid, pk, old);
        }
        interp.obarray.symbol_mut(sid).function = Value::Sym(sym::UNBOUND);
    }
}

pub(super) fn reinstall(
    interp: &mut Interp,
    pre_boot_syms: &SymbolSnapshot,
    dump_features: &[SymId],
) {
    // GNU's dump-time `autoload' is a plain defalias: every
    // `(autoload ...)' form in loaddefs.elc installs its cell
    // unconditionally, even over definitions the dump itself
    // made (e.g. the `pcase' macro).  Reinstall each loaddefs
    // autoload cell over whatever the amalgamated prelude
    // bound so that first use loads the real library — and
    // its expansion-time internals — exactly like GNU.  The
    // displaced definition goes to `remacs--dump-fn': while a
    // self-hosting file is being loaded (pcase.el expanding
    // `pcase', cl-macs.el expanding `cl-defmacro', ...),
    // `autoload_do_load' resolves the stash instead of
    // recursively loading the same file.
    if let Some(src) = crate::lisp::load::embedded("loaddefs") {
        let chars: Rc<Vec<crate::lisp::text::EmacsChar>> =
            Rc::new(src.chars().map(Into::into).collect());
        let auto_id = interp.q("autoload");
        let pk = interp.q("remacs--dump-fn");
        let void_names: HashSet<SymId> = GNU_VOID_FNS
            .iter()
            .chain(GNU_VOID_FNS_LATE.iter())
            .map(|name| interp.intern(name))
            .collect();
        let mut pos = 0usize;
        loop {
            let form = {
                let mut reader = Reader::with_chars(interp, chars.clone());
                reader.set_position(pos);
                match reader.read() {
                    Ok(Some(f)) => {
                        pos = reader.position();
                        f
                    }
                    other => {
                        if interp.dbg_flags & 0x800 != 0 {
                            eprintln!("[loaddefs-reinstall] reader stopped at {pos}: {other:?}");
                        }
                        break;
                    }
                }
            };
            let Some((name, items)) = parse_autoload(&form, auto_id) else {
                continue;
            };
            let trace = interp.dbg_flags & 0x800 != 0
                && matches!(interp.obarray.name(name), "pcase" | "rx" | "map-let");
            if !should_reinstall(
                interp,
                name,
                &items,
                auto_id,
                &void_names,
                pre_boot_syms,
                dump_features,
                trace,
            ) {
                continue;
            }
            install_autoload(interp, name, &items, auto_id, pk, trace);
        }
    }
}

/// Loaddefs is data: recognize the target and retain the raw metadata
/// arguments, which are unquoted only when installing the selected cell.
fn parse_autoload(form: &Value, auto_id: SymId) -> Option<(SymId, Vec<Value>)> {
    // `(autoload 'name FILE DOC INTERACTIVE TYPE)'
    let items = match &form {
        Value::Cons(_) => form.list_to_vec().ok(),
        _ => None,
    };
    let items = items?;
    if items.len() < 3 {
        return None;
    }
    if !matches!(&items[0], Value::Sym(h) if *h == auto_id) {
        return None;
    }
    // items[1] is (quote name).
    let name = match &items[1] {
        Value::Cons(q) => {
            let qb = q.borrow();
            let rest = match &qb.cdr {
                Value::Cons(r) => r.borrow().car.clone(),
                _ => return None,
            };
            match rest {
                Value::Sym(id) => id,
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((name, items))
}

fn should_reinstall(
    interp: &Interp,
    name: SymId,
    items: &[Value],
    auto_id: SymId,
    void_names: &HashSet<SymId>,
    pre_boot_syms: &SymbolSnapshot,
    dump_features: &[SymId],
    trace: bool,
) -> bool {
    if void_names.contains(&name) {
        if trace {
            eprintln!("[skip {}: void_names]", interp.symbol_name(name));
        }
        return false;
    }
    // GNU's loaddefs.el never carries autoloads for
    // cl-macs/cl-seq/cl-extra — those cookies are scraped
    // into cl-loaddefs.el instead, which only evaluates on
    // the first `(require 'cl-lib)'.  Skip them here so
    // -Q keeps the cl-* cells void like GNU; cl-lib.el's
    // own `(load "cl-loaddefs")' installs them on demand.
    let cl_deferred = match &items[2] {
        Value::Str(s) => {
            let f = s.borrow();
            f.as_str() == "cl-macs" || f.as_str() == "cl-seq" || f.as_str() == "cl-extra"
        }
        _ => false,
    };
    if cl_deferred {
        return false;
    }
    // A file that provides a loaded feature (the
    // dumped libraries above) was already evaluated:
    // in GNU's dump order its real definitions came
    // after the loaddefs autoload cell and win.
    let loaded_feature = match &items[2] {
        Value::Str(s) => match interp
            .obarray
            .lookup_key(&crate::lisp::obarray::InternKey::from_text(&s.borrow()))
        {
            // Compare against the dump-time feature set —
            // `interp.features' is polluted mid-init by
            // libraries that get evaluated and rolled
            // back, and is only reset to `dump_features'
            // at the end of `new'.
            Some(fid) => dump_features.contains(&fid),
            None => false,
        },
        _ => false,
    };
    if loaded_feature {
        if trace {
            eprintln!("[skip {}: loaded_feature]", interp.symbol_name(name));
        }
        return false;
    }
    // A symbol re-defined *after* loaddefs by a later
    // dump-time file (nadvice's `(cl-defmethod
    // cl-print-object ...)' installs the generic
    // dispatcher via `cl-generic-define-method') keeps
    // that definition — GNU's dump order is loaddefs
    // first.  Detect it by comparing the live cell
    // with the pre-boot snapshot: `eq' (Rc pointer
    // identity) means the prelude-folded def is still
    // in place and GNU's autoload replaces it.
    let live = interp.obarray.symbol(name).function.clone();
    // Only a Lisp-level definition counts as "re-defined
    // after loaddefs": a Subr cell predates loaddefs
    // (builtins register before the prelude), so GNU's
    // autoload still shadows it (format-spec).
    // Any non-autoload Lisp-level cell — Lambda,
    // bytecode Vec, or a Cons shape ((macro . λ),
    // closures, advice chains) — counts as "re-defined
    // after loaddefs".  A Subr cell predates loaddefs
    // entirely (builtins register before the prelude),
    // so GNU's autoload still shadows it (format-spec).
    let live_is_lisp_def = match &live {
        Value::Lambda(_) | Value::Vec(_) | Value::Cons(_) => true,
        _ => false,
    };
    let post_loaddefs_def = match pre_boot_syms.get(&name) {
        Some(state) => {
            let snap_f = &state.function;
            // A snapshot cell that's itself an autoload
            // (prelude's `(autoload 'gv-letplace "gv")'
            // forms) IS the GNU -Q state — reinstalling
            // restores it over the prelude-pulled real
            // def (gv.el loaded mid-prelude via `push''s
            // `gv-letplace' expansion).
            let snap_is_autoload = matches!(
                snap_f,
                Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
            );
            !snap_is_autoload && !same_boot_definition(&live, snap_f)
        }
        None => !matches!(live, Value::Sym(s) if s == sym::UNBOUND),
    };
    if live_is_lisp_def && post_loaddefs_def {
        return false;
    }
    true
}

fn install_autoload(
    interp: &mut Interp,
    name: SymId,
    items: &[Value],
    auto_id: SymId,
    pk: SymId,
    trace: bool,
) {
    if trace {
        eprintln!("[reinstall {}]", interp.symbol_name(name));
    }
    // Stash the prelude-folded definition being
    // displaced: code inside the autoloaded file itself
    // can still expand through it via the
    // `loading_same' path in `autoload_do_load', which
    // is what makes self-hosting files (pcase, rx,
    // cl-macs, ...) loadable without .elc bodies.
    let old = interp.obarray.symbol(name).function.clone();
    let was_autoload = matches!(
        &old,
        Value::Cons(c) if matches!(&c.borrow().car, Value::Sym(s) if *s == auto_id)
    );
    let have_stash = !matches!(
        interp.get_prop(name, pk),
        v if v.is_nil() || matches!(v, Value::Sym(s) if s == sym::UNBOUND)
    );
    if !matches!(old, Value::Sym(s) if s == sym::UNBOUND) && !was_autoload && !have_stash {
        interp.put_prop(name, pk, old);
    }
    if interp.dbg_flags & 0x800 != 0
        && matches!(interp.obarray.name(name), "pcase" | "rx" | "map-let")
    {
        eprintln!(
            "[reinstall {} -> {:?}]",
            interp.obarray.name(name),
            interp.symbol_name(name)
        );
    }
    // The read form's tail args are still source
    // syntax: `nil'/'macro' read as (quote macro)
    // etc.  Evaluate them the way `autoload' would —
    // quoted args unquote, literals self-eval — so
    // the cell ends with `t'/`macro', never
    // `(quote t)'.
    let quote_id = sym::QUOTE;
    let mut cell = vec![Value::Sym(auto_id)];
    for arg in &items[2..] {
        let evaluated = match arg {
            Value::Cons(c) => {
                let b = c.borrow();
                if matches!(&b.car, Value::Sym(s) if *s == quote_id) {
                    match &b.cdr {
                        Value::Cons(tail) => tail.borrow().car.clone(),
                        _ => arg.clone(),
                    }
                } else {
                    arg.clone()
                }
            }
            _ => arg.clone(),
        };
        cell.push(evaluated);
    }
    interp.fset(name, Value::list(cell));
}

fn same_boot_definition(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Sym(x), Value::Sym(y)) => x == y,
        (Value::Cons(x), Value::Cons(y)) => Rc::ptr_eq(x, y),
        (Value::Lambda(x), Value::Lambda(y)) => Rc::ptr_eq(x, y),
        (Value::Nil, Value::Nil) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_loaddefs_keeps_metadata_quoted_until_installation() {
        let target = Value::cons(Value::Sym(sym::QUOTE), Value::list(vec![Value::Sym(7)]));
        let metadata = Value::cons(
            Value::Sym(sym::QUOTE),
            Value::list(vec![Value::Sym(sym::T)]),
        );
        let form = Value::list(vec![
            Value::Sym(99),
            target,
            Value::string("library"),
            Value::Nil,
            metadata.clone(),
        ]);
        let (name, items) = parse_autoload(&form, 99).unwrap();
        assert_eq!(name, 7);
        let (Value::Cons(original), Value::Cons(parsed)) = (&metadata, &items[4]) else {
            panic!("metadata was evaluated")
        };
        assert!(Rc::ptr_eq(original, parsed));
        assert!(parse_autoload(&Value::list(vec![Value::Sym(99), Value::Nil]), 99).is_none());
        assert!(parse_autoload(&Value::Int(3), 99).is_none());
    }
}
