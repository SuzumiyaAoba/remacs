//! Apply compiler-only library rollback in its required bootstrap order.
//!
//! Ownership is collected before cells are changed. Cell/property restoration
//! precedes history pruning and registry repair; autoloads are installed last,
//! then gv's resulting state is retained across delayed custom initialization.

mod autoloads;
mod plan;
mod registries;

use super::{BootRollback, SymbolSnapshot, SymbolState};
use crate::lisp::{
    eval::Interp,
    obarray::sym,
    value::{SymId, Value},
};
use plan::{ROLLED_LIBS, RollbackPlan};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

pub(super) fn restore(
    interp: &mut Interp,
    pre_boot_syms: &SymbolSnapshot,
    pre_prelude_plists: &HashMap<SymId, Value>,
    dump_features: &[SymId],
) -> BootRollback {
    let plan = RollbackPlan::collect(interp);
    plan.trace(interp, pre_boot_syms);
    restore_symbols(interp, &plan, pre_boot_syms, pre_prelude_plists);
    restore_properties(interp, &plan, pre_boot_syms);
    prune_libraries(interp, &plan);
    registries::restore(interp);
    autoloads::hide_dispatchers(interp);
    autoloads::reinstall(interp, pre_boot_syms, dump_features);
    BootRollback::capture(interp)
}

fn restore_symbols(
    interp: &mut Interp,
    plan: &RollbackPlan,
    pre_boot_syms: &SymbolSnapshot,
    pre_prelude_plists: &HashMap<SymId, Value>,
) {
    let RollbackPlan {
        restore_set,
        rolled_defs,
        kept_syms,
        ..
    } = plan;
    // Restore cells and binding flags together, then clear buffer-local copies.
    // A force-void name's plist came from cl-loaddefs/declare
    // writes inside the cl chain — restore the pre-prelude
    // plist.  The `remacs--dump-fn' stash below keeps the
    // folded def reachable for `(require 'cl-lib)'/self-hosting.
    for sid in restore_set {
        let sid = *sid;
        let SymbolState {
            function: f,
            value: v,
            plist: p,
            special: sp,
            constant: cn,
            make_local_if_set: ml,
        } = pre_boot_syms.get(&sid).cloned().unwrap_or_default();
        // A name the rolled file *defined* voids unless a kept
        // file also defines it: prelude's own defs were already
        // subtracted from `rolled_defs' above, so what remains
        // here is never part of GNU's -Q dump state — even when
        // the snapshot holds a cell from the file's own
        // eval-when-compile run during the prelude
        // (pcase--memoize).  Names GNU voids that prelude folds
        // (cl-*) are re-voided by GNU_VOID_FNS_LATE after this.
        let force_void = rolled_defs.contains(&sid) && !kept_syms.contains(&sid);
        {
            let s = interp.obarray.symbol_mut(sid);
            s.function = if force_void {
                Value::Sym(sym::UNBOUND)
            } else {
                f
            };
            s.value = if force_void {
                Value::Sym(sym::UNBOUND)
            } else {
                v
            };
            s.plist = if force_void {
                pre_prelude_plists.get(&sid).cloned().unwrap_or(Value::Nil)
            } else {
                p
            };
            s.special = if force_void { false } else { sp };
            s.constant = if force_void { false } else { cn };
            s.make_local_if_set = if force_void { false } else { ml };
            s.variable_documentation = None;
            s.var_alias = None;
        }
        if force_void {
            // Preserve the dump-time stash the earlier pass
            // planted: self-hosting loads (cl-macs expanding
            // `cl-defmacro', pcase.el expanding `pcase')
            // resolve it via `autoload_do_load'.
            let dfk = interp.q("remacs--dump-fn");
            let stashed = match pre_boot_syms.get(&sid) {
                Some(state) => {
                    let mut cur = state.plist.clone();
                    let mut found = Value::Nil;
                    while let Value::Cons(c) = cur {
                        let b = c.borrow();
                        if matches!(&b.car, Value::Sym(s) if *s == dfk) {
                            found = match &b.cdr {
                                Value::Cons(c2) => c2.borrow().car.clone(),
                                _ => Value::Nil,
                            };
                            break;
                        }
                        cur = match &b.cdr {
                            Value::Cons(c2) => c2.borrow().cdr.clone(),
                            _ => Value::Nil,
                        };
                    }
                    found
                }
                None => Value::Nil,
            };
            let stash_live = match &stashed {
                Value::Nil => false,
                Value::Sym(s) => *s != sym::UNBOUND,
                _ => true,
            };
            if stash_live {
                interp.put_prop(sid, dfk, stashed);
            }
        }
        for bid in interp.buffers.list() {
            if let Some(b) = interp.buffers.get(bid) {
                b.borrow_mut().locals.shift_remove(&sid);
            }
        }
    }
}

fn restore_properties(interp: &mut Interp, plan: &RollbackPlan, pre_boot_syms: &SymbolSnapshot) {
    let prop_strips = &plan.prop_strips;
    // Strip the exact prop keys the rolled sources wrote
    // (`gv-expander' on `nth'/`if'/..., `compiler-macro' on
    // `cl-member'/..., obsolete/error metadata): the load ran
    // during the *prelude* for the cl chain (pcase's
    // eval-when-compile pulls gv→cl-lib→cl-loaddefs), so the
    // post-prelude snapshot already carries them and the
    // cell/plist restore above cannot remove them.  A NIL key
    // sentinel restores the whole plist from the snapshot.
    for (sym_id, key_id) in prop_strips {
        let sym_id = *sym_id;
        let key_id = *key_id;
        if key_id == sym::NIL {
            if let Some(state) = pre_boot_syms.get(&sym_id) {
                interp.obarray.symbol_mut(sym_id).plist = state.plist.clone();
            }
        } else {
            let plist = interp.obarray.symbol(sym_id).plist.clone();
            interp.obarray.symbol_mut(sym_id).plist = super::properties::without(&plist, key_id);
        }
    }
    interp.macro_gen += 1;
}

fn prune_libraries(interp: &mut Interp, plan: &RollbackPlan) {
    let RollbackPlan { lh, hist_items, .. } = plan;
    // Drop the rolled entries themselves from load-history.
    let new_hist: Vec<Value> = hist_items
        .iter()
        .filter(|entry| {
            let file = match entry {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => return true,
            };
            !plan::is_rolled_file(&file)
        })
        .cloned()
        .collect();
    let _ = interp.set_symbol(*lh, Value::list(new_hist));
    // Purge the rolled features from `interp.features' AND the
    // `features' Lisp var (rebuilt from `interp.features' only
    // at the very end of `new'): while they linger,
    // `autoload_do_load''s feature-present shortcut resolves a
    // gv/cl-macs autoload through the dump-time stash without
    // loading the file, leaving side effects like `cond''s
    // `gv-expander' unregistered (boot-reeval of
    // `electric-indent-mode' broke this way).
    {
        let rolled_ids: HashSet<SymId> = ROLLED_LIBS.iter().map(|l| interp.intern(l)).collect();
        interp.features.retain(|f| !rolled_ids.contains(f));
        let fid = interp.q("features");
        if let Value::Cons(_) = interp.symbol_value(fid) {
            let items = interp.symbol_value(fid).list_to_vec().unwrap_or_default();
            let kept: Vec<Value> = items
                .into_iter()
                .filter(|v| {
                    !matches!(
                        interp.sym_id(v),
                        Some(id) if rolled_ids.contains(&id)
                    )
                })
                .collect();
            let _ = interp.set_symbol(fid, Value::list(kept));
        }
    }
}
