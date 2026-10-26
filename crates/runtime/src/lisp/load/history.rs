//! GNU load-history merging and definition/dependency records.

use crate::lisp::{Interp, error::EvalResult, value::Value};
use std::rc::Rc;

/// Finish GNU's readevalloop history. The filename marker is the last
/// element of current-load-list and becomes the entry's car on reversal.
pub(crate) fn build_history(i: &mut Interp, filename: Value, entire: bool) -> EvalResult {
    let variable = i.q("load-history");
    let current_variable = i.q("current-load-list");
    let current = i.symbol_value(current_variable);
    let new_items = crate::lisp::builtins::want_list(i, &current)?;
    let mut root = i.symbol_value(variable);
    if matches!(root, Value::Sym(symbol) if symbol == crate::lisp::sym::UNBOUND) {
        root = Value::Nil;
    }
    let _ = crate::lisp::builtins::want_list(i, &root)?;
    let mut tail = root.clone();
    let mut previous: Option<crate::lisp::value::ConsRef> = None;
    let mut found = false;
    while let Value::Cons(cell) = tail {
        let (entry, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let (key, entries) = match &entry {
            Value::Cons(entry) => {
                let entry = entry.borrow();
                (entry.car.clone(), entry.cdr.clone())
            }
            Value::Nil => (Value::Nil, Value::Nil),
            _ => return Err(i.wrong_type_mut("listp", &entry)),
        };
        if crate::lisp::builtins::equal_values(i, &key, &filename)? {
            found = true;
            if entire {
                if let Some(previous) = &previous {
                    previous.borrow_mut().cdr = next.clone();
                } else {
                    root = next.clone();
                }
            } else {
                // GNU compares each new item against the original entry
                // snapshot when merging a partially evaluated buffer.
                let old_items = crate::lisp::builtins::want_list(i, &entry)?;
                for item in &new_items {
                    let mut present = false;
                    for old in &old_items {
                        if crate::lisp::builtins::equal_values(i, item, old)? {
                            present = true;
                            break;
                        }
                    }
                    if !present {
                        cell.borrow_mut().car =
                            Value::cons(key.clone(), Value::cons(item.clone(), entries.clone()));
                    }
                }
                previous = Some(cell.clone());
            }
        } else {
            previous = Some(cell.clone());
        }
        tail = next;
    }
    if entire || !found {
        let entry = crate::lisp::builtins::seq::f_nreverse_seq(i, vec![current].into())?;
        root = Value::cons(entry, root);
        i.obarray.symbol_mut(current_variable).value = Value::t();
    }
    i.obarray.symbol_mut(variable).value = root;
    Ok(Value::Nil)
}

/// Record a `load-history' entry for the embedded startup prelude so
/// `symbol-file' and the find-func family can locate prelude-defined
/// symbols.  GNU records every dumped library in `load-history' the same
/// way; FILE names the on-disk copy of the prelude source.
pub(crate) fn record_prelude_load_history(i: &mut Interp, file: &str, src: &str) {
    let cll = i.q("current-load-list");
    let mark = i.specbind_depth();
    if i.specbind(cll, Value::Nil).is_err() {
        return;
    }
    let chars: Rc<Vec<crate::lisp::text::EmacsChar>> =
        Rc::new(src.chars().map(Into::into).collect());
    let mut pos = 0usize;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(pos);
            match reader.read() {
                Ok(f) => f.map(|v| (v, reader.position())),
                Err(_) => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                record_form_tree(i, &form);
            }
            None => break,
        }
    }
    let mut entries = i.symbol_value(cll).list_to_vec().unwrap_or_default();
    entries.reverse();
    let entry = Value::cons(Value::string(file), Value::list(entries));
    let lh = i.q("load-history");
    let cur = match i.symbol_value(lh) {
        Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
        v => v,
    };
    i.obarray.symbol_mut(lh).value = Value::cons(entry, cur);
    let _ = i.unbind_to(mark);
}

/// Record one top-level prelude form, splicing `progn' children the way
/// `eval_for_load' does during `load'.
pub(super) fn record_form_tree(i: &mut Interp, form: &Value) {
    if let Some(children) = progn_children(i, form) {
        for child in children {
            record_form_tree(i, &child);
        }
    } else {
        record_load_entry(i, form);
    }
}

/// Record a `load-history' entry for a successfully evaluated top-level
/// form during `load', pushing onto `current-load-list' the way GNU's
/// lread.c does.  GNU's entry shapes: `(defun . SYM)' for function-ish
/// definitions (defun/defmacro/defsubst/defalias/autoload, plus the
/// `defun' leaf that mode/defgeneric macros expand into), bare `SYM'
/// for variables, `(defface . SYM)', `(provide . FEAT)' and
/// `(require . FEAT)'.
pub(super) fn record_load_entry(i: &mut Interp, form: &Value) {
    let Some(items) = form.list_to_vec().ok() else {
        return;
    };
    let Some(&Value::Sym(head)) = items.first() else {
        return;
    };
    // NAME is (cadr FORM); unwrap a `quote' wrapper (`provide' et al).
    let quote_id = i.q("quote");
    let name = |items: &[Value]| -> Option<Value> {
        match items.get(1)? {
            v @ Value::Sym(_) => Some(v.clone()),
            Value::Cons(_) => {
                let q = items[1].list_to_vec().ok()?;
                if q.len() == 2 && matches!(&q[0], Value::Sym(s) if *s == quote_id) {
                    Some(q[1].clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };
    const DEFUN_HEADS: &[&str] = &[
        "defun",
        "defmacro",
        "defsubst",
        "defalias",
        "autoload",
        "cl-defun",
        "cl-defmacro",
        "define-minor-mode",
        "define-derived-mode",
        "cl-defgeneric",
        "cl-defmethod",
        "define-obsolete-function-alias",
    ];
    const DEFVAR_HEADS: &[&str] = &[
        "defvar",
        "defconst",
        "defvar-local",
        "defcustom",
        "defvar-keymap",
        "custom-declare-variable",
        "define-obsolete-variable-alias",
    ];
    let head_name = i.symbol_name(head).to_string();
    let defun_id = i.q("defun");
    let defface_id = i.q("defface");
    let require_id = i.q("require");
    let entry = match head_name.as_str() {
        h if DEFUN_HEADS.contains(&h) => name(&items).map(|nm| Value::cons(i.sym(defun_id), nm)),
        h if DEFVAR_HEADS.contains(&h) => name(&items),
        // `defface' expands to `custom-declare-face' before eval; GNU's
        // C subr attaches (defface . FACE) to `current-load-list'.
        "defface" | "custom-declare-face" => {
            name(&items).map(|nm| Value::cons(i.sym(defface_id), nm))
        }
        // `provide' is NOT recorded by the form reader in GNU: Fprovide
        // does its own LOADHIST_ATTACH (catches nested provides too) —
        // recording here too would push a duplicate entry.
        "provide" => None,
        "require" => name(&items).map(|nm| Value::cons(i.sym(require_id), nm)),
        _ => None,
    };
    if let Some(e) = entry {
        let cll = i.q("current-load-list");
        let cur = match i.symbol_value(cll) {
            Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
            v => v,
        };
        i.obarray.symbol_mut(cll).value = Value::cons(e, cur);
    }
}

/// If FORM is `(progn CHILDREN...)', return the children.
pub(super) fn progn_children(i: &mut Interp, form: &Value) -> Option<Vec<Value>> {
    if let Value::Cons(c) = form {
        let cb = c.borrow();
        if let Value::Sym(s) = &cb.car {
            if *s == i.q("progn") {
                return cb.cdr.list_to_vec().ok();
            }
        }
    }
    None
}
