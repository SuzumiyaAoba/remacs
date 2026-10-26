//! Identify symbol mutations made by libraries rolled out of the dumped image.

use crate::lisp::{
    eval::Interp,
    obarray::sym,
    value::{SymId, Value},
};
use rustc_hash::FxHashSet as HashSet;
use std::rc::Rc;

/// Deep-collect every interned symbol reachable inside a `load-history'
/// item (or any Lisp object): the pre-boot rollback restores by symbol
/// name, and side-effect forms (`put', `autoload', `defalias') never
/// produce history items, so the set must be broader than
/// `(defun . name)' pairs.  Cycles and shared tails are bounded by a
/// visited-cons counter.
pub(super) fn collect_item_syms(_i: &Interp, v: &Value, out: &mut HashSet<SymId>) {
    let mut work = vec![v.clone()];
    let mut steps = 0usize;
    while let Some(x) = work.pop() {
        steps += 1;
        if steps > 200_000 {
            break;
        }
        match &x {
            Value::Sym(s) => {
                out.insert(*s);
            }
            Value::Cons(c) => {
                let b = c.borrow();
                work.push(b.car.clone());
                work.push(b.cdr.clone());
            }
            Value::Vec(vec) => {
                for e in vec.borrow().iter() {
                    work.push(e.clone());
                }
            }
            Value::Record(r) => {
                let mut k = 0;
                while let Some(e) = r.borrow().get(k) {
                    work.push(e.clone());
                    k += 1;
                }
            }
            _ => {}
        }
    }
}

/// Read every top-level form of an embedded Lisp source and collect the
/// symbols its evaluation could have mutated — the defined name of
/// `defun'/`defmacro'/`defvar'/`cl-defmethod'/... forms and the target
/// arguments of side-effect calls (`put', `autoload', `defalias',
/// `define-error', `make-obsolete*', `gv-define-*', `fset', `set',
/// `declare-function', `custom-declare-*').  Symbols merely *mentioned*
/// in code (docstring tails, loop variables, called functions) must NOT
/// be collected: the pre-boot rollback restores every collected symbol
/// to its pre-prelude state, which unbound `dolist'/`when' & co. when
/// the set was the union of every symbol in the file.
pub(super) fn collect_source_syms(
    i: &mut Interp,
    src: &str,
    out: &mut HashSet<SymId>,
    prop_out: &mut HashSet<(SymId, SymId)>,
    defs_out: &mut HashSet<SymId>,
) {
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
                collect_form_targets(i, &form, out, prop_out, defs_out);
            }
            None => break,
        }
    }
}

/// Push the symbol interned by `(quote SYM)' or `#'SYM' into `out';
/// non-literal arguments (computed names) are ignored — they cannot be
/// restored by name anyway.
fn collect_quoted(i: &Interp, v: &Value, out: &mut HashSet<SymId>) {
    if let Value::Cons(c) = v {
        let b = c.borrow();
        let is_quote = matches!(
            &b.car,
            Value::Sym(s) if matches!(i.obarray.name(*s), "quote" | "function")
        );
        if is_quote {
            if let Value::Cons(t) = &b.cdr {
                if let Value::Sym(s) = t.borrow().car {
                    out.insert(s);
                }
            }
        }
    }
}

/// Push the literal symbol in `v' (a bare `Value::Sym') into `out'.
fn collect_sym_arg(v: &Value, out: &mut HashSet<SymId>) {
    if let Value::Sym(s) = v {
        out.insert(*s);
    }
}

/// Collect mutation targets from one top-level (or nested container)
/// form.
///
/// Three disjoint output sets:
///   * `out` — symbols whose whole state (function cell, value cell,
///     plist, binding flags) is restored from the pre-boot snapshot:
///     defined names plus non-defining cell writes (`fset', `autoload',
///     `define-obsolete-function-alias', ...).
///   * `prop_out` — `(symbol, prop-name)' pairs: literal `put'/
///     `function-put' targets and the known prop keys written by
///     `gv-define-*'/`make-obsolete*'/`define-error'/advertised-calling-
///     convention.  Only that plist key is removed, preserving props the
///     prelude/dump legitimately installed on the same symbol (unlike a
///     whole-plist restore, which voided `if' itself when gv.el's
///     `(put 'if 'gv-expander ...)' rolled back).
///   * `defs_out` — top-level defining heads only (defun/defmacro/
///     defvar/defconst/defcustom/defalias/defsubst).  A rolled file's
///     own definition must be erased even when the prelude folded the
///     same name (the snapshot then holds a real cell, not void): GNU
///     -Q binds it only via the dump or not at all.
fn collect_form_targets(
    i: &mut Interp,
    v: &Value,
    out: &mut HashSet<SymId>,
    prop_out: &mut HashSet<(SymId, SymId)>,
    defs_out: &mut HashSet<SymId>,
) {
    let mut work = vec![v.clone()];
    while let Some(x) = work.pop() {
        let items = match &x {
            Value::Cons(_) => {
                let mut v = Vec::new();
                let mut cur = x.clone();
                while let Value::Cons(cc) = cur {
                    let bb = cc.borrow();
                    v.push(bb.car.clone());
                    cur = bb.cdr.clone();
                }
                v
            }
            _ => continue,
        };
        if items.is_empty() {
            continue;
        }
        let head = match &items[0] {
            Value::Sym(s) => i.obarray.name(*s).to_string(),
            _ => {
                // Non-symbol head (e.g. `((lambda ...) args)'): scan
                // subforms for nested defining forms.
                for it in &items {
                    if let Value::Cons(_) = it {
                        work.push(it.clone());
                    }
                }
                continue;
            }
        };
        let h = head.as_str();
        // Defining forms: the name is the first argument.  This covers
        // `defun'/`defmacro'/`defvar'/`defcustom'/`defalias'-style heads
        // including `cl-defun'/`cl-defmethod' (name is items[1];
        // `(setf SYM)' method names yield the `(setf ...)' list, so
        // also collect the inner symbol) and `defstruct'/`defclass'
        // (whose accessor/constructor symbols aren't literal in the
        // form — collect the whole form deeply for those).
        let name_is_sym = matches!(&items.get(1), Some(Value::Sym(_)));
        let name_is_quote = matches!(&items.get(1), Some(Value::Cons(_)));
        if h.starts_with("def") || h.starts_with("define-") || h.starts_with("cl-def") {
            if h == "defstruct" || h == "defclass" || h == "cl-defstruct" {
                collect_item_syms(i, &x, out);
                if std::env::var_os("REMACS_TRACE_COLLECT").is_some() {
                    let nm = match items.get(1) {
                        Some(Value::Sym(s)) => i.obarray.name(*s).to_string(),
                        _ => "?".to_string(),
                    };
                    eprintln!(
                        "[defs {} <- {}]",
                        nm,
                        i.princ_to_string(&x)
                            .chars()
                            .take(80)
                            .collect::<crate::lisp::text::EmacsText>()
                    );
                }
            } else if h == "defvar" || h == "defcustom" || h == "defconst" {
                // Rolled-file value defs force-void too: GNU never ran
                // the file, so `boundp' is nil at -Q (pcase--memoize,
                // killed-rectangle, the cl--loop-* caches, ...).  Names
                // the prelude itself defines (macroexp--dynvars, also
                // defvar'd by the rolled subr-x copy) are subtracted
                // from rolled_defs before the restore loop, so their
                // snapshot binding survives.
                if let Value::Sym(s) = items[1] {
                    out.insert(s);
                    defs_out.insert(s);
                }
            } else if name_is_sym {
                if let Value::Sym(s) = items[1] {
                    out.insert(s);
                    defs_out.insert(s);
                    if std::env::var_os("REMACS_TRACE_COLLECT").is_some() {
                        eprintln!(
                            "[defs {} <- {}]",
                            i.obarray.name(s),
                            i.princ_to_string(&x)
                                .chars()
                                .take(90)
                                .collect::<crate::lisp::text::EmacsText>()
                        );
                    }
                }
            } else if name_is_quote {
                collect_quoted(i, &items[1], out);
                // `(setf SYM)' heads and `(defalias 'f (..))' — scan
                // the arg for a literal symbol too.
                if let Value::Cons(c) = &items[1] {
                    let b = c.borrow();
                    let mut inner = b.cdr.clone();
                    while let Value::Cons(t) = inner {
                        let tb = t.borrow();
                        collect_sym_arg(&tb.car, out);
                        inner = tb.cdr.clone();
                    }
                }
            }
            continue;
        }
        // Resolve the literal symbol arg shared by every side-effect
        // head below: `(head 'SYM ...)' / `(head SYM ...)'.
        let sym_arg = |at: usize| -> Option<SymId> {
            let a = items.get(at)?;
            if let Value::Sym(s) = a {
                return Some(*s);
            }
            if let Value::Cons(c) = a {
                let b = c.borrow();
                let quoted = matches!(
                    &b.car,
                    Value::Sym(s) if matches!(i.obarray.name(*s), "quote" | "function")
                );
                if quoted {
                    if let Value::Cons(t) = &b.cdr {
                        if let Value::Sym(s) = t.borrow().car {
                            return Some(s);
                        }
                    }
                }
            }
            None
        };
        match h {
            // (quote SYM) / #'SYM appearing as a *form*: irrelevant.
            "quote" | "function" | "provide" | "require" | "declare" | "declare-function" => {}
            // Literal `(put 'SYM 'KEY ...)'/`function-put': strip just
            // that prop key (a nil value is `remprop' — removing is
            // still correct).  A non-literal key forces a whole-plist
            // restore — encoded with a NIL key sentinel.
            "put" | "function-put" | "define-symbol-prop" => {
                if let Some(sym_id) = sym_arg(1) {
                    match sym_arg(2) {
                        Some(key_id) => {
                            prop_out.insert((sym_id, key_id));
                        }
                        None => {
                            prop_out.insert((sym_id, sym::NIL));
                        }
                    }
                }
            }
            // Known fixed prop keys for non-put heads.
            "make-obsolete" | "define-obsolete-function-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("byte-obsolete-info")));
                }
            }
            "make-obsolete-variable" | "define-obsolete-variable-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("byte-obsolete-variable")));
                }
            }
            "make-obsolete-face" | "define-obsolete-face-alias" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("obsolete-face")));
                }
            }
            "make-obsolete-generalized-variable" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("byte-obsolete-generalized-variable")));
                }
            }
            // All three write the `gv-expander' prop.
            "gv-define-setter" | "gv-define-simple-setter" | "gv-define-expander" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("gv-expander")));
                }
            }
            "define-error" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("error-conditions")));
                    prop_out.insert((s, i.q("error-message")));
                }
            }
            "set-advertised-calling-convention" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("advertised-calling-convention")));
                }
            }
            // `pcase-defmacro' writes NAME's `pcase-macroexpander'
            // prop — not a function cell (e.g. `(pcase-defmacro \`
            // (qpat) ...)' must not void the backquote macro).
            "pcase-defmacro" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, i.q("pcase-macroexpander")));
                }
            }
            // Whole-plist overwrite: restore the snapshot plist.
            "setplist" => {
                if let Some(s) = sym_arg(1) {
                    prop_out.insert((s, sym::NIL));
                }
            }
            // Function/value-cell writes without a definition form:
            // `fset'/`autoload' leave an (autoload ...) cell; `set'
            // assigns the value cell.  The whole symbol still needs
            // restoring so the written cell goes back to its pre-load
            // state.
            "fset" | "set" | "defvaralias" | "autoload" => {
                if let Some(a) = items.get(1) {
                    collect_sym_arg(a, out);
                    collect_quoted(i, a, out);
                }
            }
            // Container/macro heads and everything else: recurse into
            // every element so nested definitions inside `progn'/
            // `eval-when-compile'/`unless' are still collected.  Bare
            // symbols drop out (collect_form_targets ignores non-conses).
            _ => {
                for it in &items {
                    if let Value::Cons(_) = it {
                        work.push(it.clone());
                    }
                }
            }
        }
    }
}
