//! Compute ownership and mutation targets before applying dump rollback.

use super::super::{SymbolSnapshot, history::collect_source_syms};
use crate::lisp::{
    eval::Interp,
    value::{SymId, Value},
};
use rustc_hash::FxHashSet as HashSet;

// Roll back compiler-only helpers, never subr-x: its folded definitions
// belong to the dump even though its own feature is absent at -Q. Restore
// gv's autoloads and remove its expander properties so first use loads it.
pub(super) const ROLLED_LIBS: &[&str] = &[
    "rect",
    "send-to",
    "map",
    "cl-macs",
    "cl-lib",
    "cl-loaddefs",
    "pcase",
    "gv",
    "help-macro",
    "remacs-compat",
    "inline",
];

pub(super) struct RollbackPlan {
    pub kept_syms: HashSet<SymId>,
    pub touched: HashSet<SymId>,
    pub file_syms: HashSet<SymId>,
    pub rolled_defs: HashSet<SymId>,
    pub prop_strips: HashSet<(SymId, SymId)>,
    pub restore_set: HashSet<SymId>,
    pub lh: SymId,
    pub hist_items: Vec<Value>,
}

impl RollbackPlan {
    pub fn collect(interp: &mut Interp) -> Self {
        let (lh, hist_items) = read_history(interp);
        let (kept_syms, touched) = history_targets(interp, &hist_items);
        let SourceTargets {
            file_syms,
            prop_strips,
            rolled_defs,
        } = source_targets(interp);
        let restore_set = selected_symbols(&kept_syms, &touched, &file_syms);
        Self {
            kept_syms,
            touched,
            file_syms,
            rolled_defs,
            prop_strips,
            restore_set,
            lh,
            hist_items,
        }
    }

    pub fn trace(&self, interp: &mut Interp, pre_boot_syms: &SymbolSnapshot) {
        let Self {
            kept_syms,
            touched,
            file_syms,
            rolled_defs,
            restore_set,
            ..
        } = self;
        if interp.dbg_flags & 0x800 != 0 {
            let id = interp.q("cl-member");
            eprintln!(
                "[pre-restore] cl-member in_file={} in_touched={} kept={} fn={:?} plist={:?}",
                file_syms.contains(&id),
                touched.contains(&id),
                kept_syms.contains(&id),
                interp.obarray.symbol(id).function,
                interp.obarray.symbol(id).plist
            );
        }
        if interp.dbg_flags & 0x800 != 0 {
            let pb = interp.q("byte-run--parse-body");
            let bq = interp.q("`");
            let md = interp.q("macroexp--dynvars");
            eprintln!(
                "[pre-restore] pb kept={} file={} tch={} | ` kept={} file={} tch={} rst={} | mdyn kept={} file={} tch={} rst={} snapv={:?}",
                kept_syms.contains(&pb),
                file_syms.contains(&pb),
                touched.contains(&pb),
                kept_syms.contains(&bq),
                file_syms.contains(&bq),
                touched.contains(&bq),
                restore_set.contains(&bq),
                kept_syms.contains(&md),
                file_syms.contains(&md),
                touched.contains(&md),
                restore_set.contains(&md),
                pre_boot_syms.get(&md).map(|state| state.value.clone())
            );
        }
        if interp.dbg_flags & 0x800 != 0 {
            for nm in [
                "pcase--memoize",
                "killed-rectangle",
                "cl--do-arglist",
                "cl--lambda-list-keywords",
                "cl-float-epsilon",
                "cl-struct-timer",
                "rectangle-mark-mode-map",
            ] {
                let q = interp.intern(nm);
                eprintln!(
                    "[pre-restore] {nm} defs={} kept={} file={} tch={} rst={} snapfn={:?}",
                    rolled_defs.contains(&q),
                    kept_syms.contains(&q),
                    file_syms.contains(&q),
                    touched.contains(&q),
                    restore_set.contains(&q),
                    pre_boot_syms.get(&q).map(|state| state.function.clone())
                );
            }
        }
    }
}

fn read_history(interp: &mut Interp) -> (SymId, Vec<Value>) {
    let lh = interp.q("load-history");
    let mut hist = interp.symbol_value(lh);
    let mut hist_items: Vec<Value> = Vec::new();
    while let Value::Cons(c) = hist {
        let b = c.borrow();
        hist_items.push(b.car.clone());
        hist = b.cdr.clone();
    }
    (lh, hist_items)
}

fn history_targets(interp: &Interp, hist_items: &[Value]) -> (HashSet<SymId>, HashSet<SymId>) {
    let mut kept_syms = HashSet::default();
    let mut touched = HashSet::default();
    for entry in hist_items {
        let file = match entry {
            Value::Cons(c) => c.borrow().car.clone(),
            _ => continue,
        };
        let rolled = is_rolled_file(&file);
        if let Value::Cons(c) = entry {
            let mut items = c.borrow().cdr.clone();
            while let Value::Cons(ic) = items {
                let item = ic.borrow().car.clone();
                items = ic.borrow().cdr.clone();
                // Both rolled and kept entries record
                // `(TYPE . NAME)' / `(TYPE NAME ...)' items
                // where TYPE is defun/defmacro/autoload/...
                // and bare symbols for defvar'd names.  Only
                // NAME is a mutated symbol — collecting the
                // TYPE token itself (as a deep walk does) pulls
                // `defun' into the restore set and voids the
                // macro byte-run just defined.
                let mut push_sym = |v: &Value| {
                    if let Value::Sym(sid) = v {
                        if rolled {
                            touched.insert(*sid);
                        } else {
                            kept_syms.insert(*sid);
                        }
                    }
                };
                match &item {
                    Value::Cons(ic2) => {
                        let ib = ic2.borrow();
                        match &ib.car {
                            Value::Sym(t)
                                if matches!(
                                    interp.obarray.name(*t),
                                    "defun"
                                        | "defmacro"
                                        | "cl-defgeneric"
                                        | "autoload"
                                        | "t"
                                        | "defvar"
                                        | "defconst"
                                        | "defcustom"
                                        | "defalias"
                                        | "defsubst"
                                        | "defvar-local"
                                        | "defface"
                                        | "defgroup"
                                        | "cl-defun"
                                        | "cl-defmacro"
                                        | "cl-defsubst"
                                        | "cl-deftype"
                                        | "cl-defstruct"
                                        | "defstruct"
                                ) =>
                            {
                                push_sym(&ib.cdr);
                            }
                            Value::Sym(t) if interp.obarray.name(*t) == "cl-defmethod" => {
                                if let Value::Cons(d) = &ib.cdr {
                                    push_sym(&d.borrow().car);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => push_sym(&item),
                }
            }
        }
    }
    (kept_syms, touched)
}

struct SourceTargets {
    file_syms: HashSet<SymId>,
    prop_strips: HashSet<(SymId, SymId)>,
    rolled_defs: HashSet<SymId>,
}

fn source_targets(interp: &mut Interp) -> SourceTargets {
    // Add every symbol literally targeted in the rolled-back
    // sources (autoload/'put/function-put/define-obsolete
    // side effects don't appear in load-history).  These
    // restore unconditionally — a rolled entry's own
    // `(defun . s)' history records for its autoload forms
    // would otherwise shield them as "kept".
    let mut prop_strips: HashSet<(SymId, SymId)> = HashSet::default();
    let mut rolled_defs: HashSet<SymId> = HashSet::default();
    let mut file_syms: HashSet<SymId> = HashSet::default();
    for lib in ROLLED_LIBS {
        if let Some(src) = crate::lisp::load::embedded(lib) {
            collect_source_syms(
                interp,
                src,
                &mut file_syms,
                &mut prop_strips,
                &mut rolled_defs,
            );
            if std::env::var_os("REMACS_TRACE_COLLECT").is_some()
                && rolled_defs.contains(&interp.q("`"))
            {
                eprintln!("[rolled-defs got ` from {lib}]");
            }
        }
    }
    // The prelude itself writes some of the same (SYM . KEY)
    // pairs a rolled file does (e.g. `(put 'cl--block-wrapper
    // 'compiler-macro ...)'): those stay — GNU's dump carries
    // them.  Scan PRELUDE with the same collector and subtract.
    {
        let mut _o = HashSet::default();
        let mut prelude_pairs: HashSet<(SymId, SymId)> = HashSet::default();
        let mut prelude_defs: HashSet<SymId> = HashSet::default();
        collect_source_syms(
            interp,
            crate::lisp::prelude::PRELUDE,
            &mut _o,
            &mut prelude_pairs,
            &mut prelude_defs,
        );
        prop_strips.retain(|pair| !prelude_pairs.contains(pair));
        // Names the prelude itself defines must not force-void
        // on rollback: prelude defs model GNU's dumped Lisp, so
        // GNU -Q binds them even when a rolled file carries a
        // copy (pcase.el's `define-symbol-prop' shim).
        rolled_defs.retain(|s| !prelude_defs.contains(s));
    }
    SourceTargets {
        file_syms,
        prop_strips,
        rolled_defs,
    }
}

fn selected_symbols(
    kept_syms: &HashSet<SymId>,
    touched: &HashSet<SymId>,
    file_syms: &HashSet<SymId>,
) -> HashSet<SymId> {
    let mut restore_set: HashSet<SymId> = HashSet::default();
    // History-named symbols of rolled entries restore, and
    // every symbol the rolled sources target — EXCEPT names a
    // kept library's history records: a kept file wrote those
    // cells *after* the snapshot (byte-run.el's defalias for
    // `byte-run--parse-body'), so restoring the pre-snapshot
    // cell would void a definition GNU keeps.  Names GNU voids
    // anyway (cl-*) are re-voided by GNU_VOID_FNS_LATE below.
    for s in touched.iter().chain(file_syms.iter()) {
        if !kept_syms.contains(s) {
            restore_set.insert(*s);
        }
    }
    restore_set
}

pub(super) fn is_rolled_file(file: &Value) -> bool {
    match file {
        Value::Str(s) => {
            let name = s.borrow();
            ROLLED_LIBS.iter().any(|library| {
                name.ends_with(&format!("/{library}.el"))
                    || name.ends_with(&format!("/{library}.elc"))
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_ownership_wins_over_both_history_and_source_targets() {
        let kept = HashSet::from_iter([2, 3]);
        let history = HashSet::from_iter([1, 2]);
        let source = HashSet::from_iter([3, 4]);
        assert_eq!(
            selected_symbols(&kept, &history, &source),
            HashSet::from_iter([1, 4])
        );
    }

    #[test]
    fn rollback_matches_only_explicit_library_path_suffixes() {
        for filename in ["/cache/cl-lib.el", "/cache/cl-lib.elc", "/cache/gv.el"] {
            assert!(is_rolled_file(&Value::string(filename)), "{filename}");
        }
        for filename in [
            "cl-lib.el",
            "/cache/other-cl-lib.el",
            "/cache/cl-lib.elc.gz",
            "/cache/subr-x.el",
        ] {
            assert!(!is_rolled_file(&Value::string(filename)), "{filename}");
        }
        assert!(!is_rolled_file(&Value::Nil));
    }
}
