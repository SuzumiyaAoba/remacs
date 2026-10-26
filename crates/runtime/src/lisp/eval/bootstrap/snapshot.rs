//! Named symbol cells and binding flags retained for compiler-load rollback.

use crate::lisp::{
    obarray::{Symbol, sym},
    value::{SymId, Value},
};
use rustc_hash::FxHashMap as HashMap;

pub(super) type SymbolSnapshot = HashMap<SymId, SymbolState>;

#[derive(Clone)]
pub(super) struct SymbolState {
    pub function: Value,
    pub value: Value,
    pub plist: Value,
    pub special: bool,
    pub constant: bool,
    pub make_local_if_set: bool,
}

impl SymbolState {
    pub fn capture(symbol: &Symbol) -> Self {
        Self {
            function: symbol.function.clone(),
            value: symbol.value.clone(),
            plist: symbol.plist.clone(),
            special: symbol.special,
            constant: symbol.constant,
            make_local_if_set: symbol.make_local_if_set,
        }
    }
}

impl Default for SymbolState {
    fn default() -> Self {
        Self {
            function: Value::Sym(sym::UNBOUND),
            value: Value::Sym(sym::UNBOUND),
            plist: Value::Nil,
            special: false,
            constant: false,
            make_local_if_set: false,
        }
    }
}

impl super::BootRollback {
    pub(super) fn capture(interp: &mut crate::lisp::eval::Interp) -> Self {
        // Snapshot gv.el's post-rollback state for the tail fixup:
        // the custom-reevaluate pass below re-loads gv.el through
        // `advice-remove''s `(setf ...)' expansion, so the GNU -Q
        // cells must be re-applied at the very end of `new'.
        {
            let mut gv_names: rustc_hash::FxHashSet<SymId> = rustc_hash::FxHashSet::default();
            let mut gp: rustc_hash::FxHashSet<(SymId, SymId)> = rustc_hash::FxHashSet::default();
            let mut gd: rustc_hash::FxHashSet<SymId> = rustc_hash::FxHashSet::default();
            if let Some(src) = crate::lisp::load::embedded("gv") {
                super::history::collect_source_syms(interp, src, &mut gv_names, &mut gp, &mut gd);
            }
            gv_names.extend(gd.iter().copied());
            let gv_late_syms = gv_names
                .iter()
                .map(|&sid| {
                    let s = interp.obarray.symbol(sid);
                    (sid, s.function.clone(), s.value.clone(), s.plist.clone())
                })
                .collect();
            let gv_late_props = gp.iter().copied().collect();
            Self {
                symbols: gv_late_syms,
                properties: gv_late_props,
            }
        }
    }
}
