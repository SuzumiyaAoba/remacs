//! Live-definition protection for post-boot GNU subr-x loading.

use super::file::eval_file_lex_dumped;
use super::source::eval_src;
use super::{LoadOutcome, embedded};
use crate::lisp::{Interp, value::Value};

/// Evaluate GNU's verbatim subr-x.el (`lisp/emacs-lisp-subr-x.el') as
/// FILE: the on-disk copy, or the embedded source for `builtin:' names.
pub(super) fn eval_gnu_subr_x<F: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    file: &F,
    dumped_like: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    let file = file.with_text(|text| text.to_owned());
    if !file.starts_with("builtin:") {
        return eval_file_lex_dumped(i, &file, false, dumped_like)
            .map(|_| LoadOutcome::Loaded(file.to_string()));
    }
    match embedded("emacs-lisp-subr-x") {
        Some(src) => eval_src(i, &file, src, false).map(|_| LoadOutcome::Loaded(file.to_string())),
        None => Ok(LoadOutcome::Missing),
    }
}

/// Every live (non-void, non-autoload) function cell.
pub(super) fn live_definitions(i: &Interp) -> std::collections::HashMap<crate::lisp::SymId, Value> {
    let autoload = i.q_soft("autoload");
    i.obarray
        .all_ids()
        .into_iter()
        .filter_map(|id| {
            let f = &i.obarray.symbol(id).function;
            let live = match f {
                Value::Nil => false,
                Value::Sym(s) => *s != crate::lisp::sym::UNBOUND,
                Value::Cons(c) => !matches!(&c.borrow().car, Value::Sym(s) if Some(*s) == autoload),
                _ => true,
            };
            live.then(|| (id, f.clone()))
        })
        .collect()
}

/// GNU's subr-x only fills cells that are void or autoloads at -Q.
/// Names it redefines that already had a live definition here (Rust
/// subr shadows, dump-time compiled twins) keep it, so loading subr-x
/// never swaps a working function for another implementation.  FILE's
/// `(defun . NAME)' load-history items list what the load defined.
pub(super) fn keep_live_definitions(
    i: &mut Interp,
    file: &crate::lisp::text::EmacsStr,
    live: &std::collections::HashMap<crate::lisp::SymId, Value>,
) {
    let lh = i.q("load-history");
    let defun = i.q("defun");
    let entry = i
        .symbol_value(lh)
        .list_to_vec()
        .unwrap_or_default()
        .into_iter()
        .find(|e| {
            matches!(e, Value::Cons(c)
                     if matches!(&c.borrow().car, Value::Str(s) if s.borrow().as_str() == file))
        });
    let Some(Value::Cons(entry)) = entry else {
        return;
    };
    let items = entry.borrow().cdr.list_to_vec().unwrap_or_default();
    for item in items {
        let Value::Cons(c) = &item else { continue };
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if !matches!(car, Value::Sym(s) if s == defun) {
            continue;
        }
        if let Some(old) = i.sym_id(&cdr).and_then(|id| live.get(&id).map(|v| (id, v))) {
            i.obarray.symbol_mut(old.0).function = old.1.clone();
        }
    }
}
