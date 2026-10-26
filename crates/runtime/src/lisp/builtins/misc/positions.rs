//! Positioned-symbol construction and dynamic symbol/record views.

use super::any_sympos_parts;

use crate::lisp::builtins::arg;
use crate::lisp::{
    Interp,
    error::EvalResult,
    obarray::sym,
    value::{SymId, Value},
};

/// Decode the immutable payload of an actual positioned symbol.
/// Ordinary records with the same slots remain ordinary records.
pub(crate) fn raw_symbol_position(value: &Value) -> Option<(SymId, i128)> {
    if value.record_kind() != Some(crate::lisp::value::RecordKind::SymbolWithPos) {
        return None;
    }
    let Value::Record(record) = value else {
        return None;
    };
    let slots = record.borrow();
    match slots.as_slice() {
        [_, Value::Sym(symbol), Value::Int(position)] => Some((*symbol, *position)),
        [_, Value::Nil, Value::Int(position)] => Some((0, *position)),
        _ => None,
    }
}

pub(crate) fn sym_pos_parts(_i: &Interp, value: &Value) -> Option<(SymId, i128)> {
    raw_symbol_position(value)
}

/// The current Lisp view is independent of when the object was made.
pub(crate) fn as_sympos(value: &Value) -> Option<(SymId, i128)> {
    crate::lisp::value::symbol_positions_enabled()
        .then(|| raw_symbol_position(value))
        .flatten()
}

pub(crate) fn symbol_position_for(i: &Interp, value: &Value) -> Option<(SymId, i128)> {
    i.positioned_symbols_enabled()
        .then(|| raw_symbol_position(value))
        .flatten()
}

pub(crate) const SYMPOS_MAGIC: i128 = 0x5EED_CAFE_1;

pub(crate) fn make_sympos(bare: SymId, pos: i128) -> Value {
    Value::record_with_kind(
        vec![Value::Int(SYMPOS_MAGIC), Value::Sym(bare), Value::Int(pos)],
        crate::lisp::value::RecordKind::SymbolWithPos,
    )
}

pub(crate) fn unpos(i: &Interp, value: &Value) -> Value {
    match symbol_position_for(i, value) {
        Some((0, _)) => Value::Nil,
        Some((symbol, _)) => Value::Sym(symbol),
        None => value.clone(),
    }
}

pub(crate) fn sympos_enabled_p(i: &Interp) -> bool {
    i.positioned_symbols_enabled()
}

pub(crate) fn make_symbol_with_pos(i: &mut Interp, sym: SymId, pos: i128) -> Value {
    Value::record_with_kind(
        vec![
            Value::Sym(i.q("symbol-with-pos")),
            Value::Sym(sym),
            Value::Int(pos),
        ],
        crate::lisp::value::RecordKind::SymbolWithPos,
    )
}

pub(super) fn f_bare_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    if let Some((s, _)) = any_sympos_parts(i, &args[0]) {
        return Ok(i.sym(s));
    }
    match &args[0] {
        Value::Sym(_) | Value::Nil => Ok(args[0].clone()),
        other => {
            // GNU `Fbare_symbol' uses CHECK_SYMBOL_OR_SYMBOL_WITH_POS —
            // the signaled predicate is the two-element list.
            let swp = i.q("symbol-with-pos-p");
            let symbolp = i.q("symbolp");
            Err(i.signal_data(
                sym::WRONG_TYPE_ARGUMENT,
                vec![
                    Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                    other.clone(),
                ],
            ))
        }
    }
}

pub(super) fn f_bare_symbol_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // t only for a plain symbol — a positioned symbol is not bare.
    Ok(Value::from_bool(
        matches!(&args[0], Value::Sym(_) | Value::Nil) && any_sympos_parts(i, &args[0]).is_none(),
    ))
}

pub(super) fn f_position_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `Fposition_symbol'
    // CHECK_SYMBOL_OR_SYMBOL_WITH_POS's SYM (the signaled predicate is
    // the two-element list) and accepts a fixnum or a positioned symbol
    // for POS (`fixnum-or-symbol-with-pos-p').
    let s = if let Some((s, _)) = any_sympos_parts(i, &args[0]) {
        s
    } else {
        match i.sym_id(&args[0]) {
            Some(s) => s,
            None => {
                let swp = i.q("symbol-with-pos-p");
                let symbolp = i.q("symbolp");
                return Err(i.signal_data(
                    sym::WRONG_TYPE_ARGUMENT,
                    vec![
                        Value::list(vec![Value::Sym(symbolp), Value::Sym(swp)]),
                        args[0].clone(),
                    ],
                ));
            }
        }
    };
    let pos = match &args[1] {
        Value::Int(n) => *n,
        v => match any_sympos_parts(i, v) {
            Some((_, p)) => p,
            None => return Err(i.wrong_type_mut("fixnum-or-symbol-with-pos-p", &args[1])),
        },
    };
    // GNU `position-symbol' always creates the same object; the dynamic
    // flag controls whether symbol primitives expose it as a symbol.
    Ok(make_symbol_with_pos(i, s, pos))
}

pub(super) fn f_remove_pos_from_symbol(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU `remove-pos-from-symbol' unwraps a positioned
    // symbol; any other object passes through unchanged (unlike
    // `bare-symbol' which type-checks).
    if let Some((s, _)) = sym_pos_parts(i, &args[0]).or_else(|| as_sympos(&args[0])) {
        return Ok(i.sym(s));
    }
    Ok(args[0].clone())
}

pub(super) fn f_symbol_with_pos_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(any_sympos_parts(i, &args[0]).is_some()))
}

pub(super) fn f_symbol_with_pos_pos(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    match any_sympos_parts(i, &args[0]) {
        Some((_, p)) => Ok(Value::Int(p)),
        None => Err(i.wrong_type_mut("symbol-with-pos-p", &args[0])),
    }
}

/// `read-positioning-symbols' — like `read', but every symbol token
/// becomes a `symbol-with-pos' object.  GNU reports 1-based absolute
/// positions for buffers and 0-based read-relative ones for strings
/// and markers. Both entry points share callable-stream and minibuffer
/// dispatch; the minibuffer path returns ordinary objects.
pub(super) fn f_read_positioning_symbols(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    crate::lisp::builtins::readfn::read_source(i, arg(&a, 0), true)
}
