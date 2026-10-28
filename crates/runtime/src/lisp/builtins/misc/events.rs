//! Keyboard/mouse event decoding and modifier primitives.

use super::display::decode_display;
use crate::editor::sel_frame;
use crate::lisp::builtins::arg;

use crate::editor::{CHAR_ALT, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use crate::lisp::eq_values;
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::{SymId, Value},
};

// ---------- events ----------

pub(super) fn f_eventp(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU subr.el accepts every integer as a scalar event. A cons must
    // instead have a non-nil, non-keyword symbol as its head.
    if matches!(&a[0], Value::Int(_) | Value::BigInt(_)) {
        return Ok(Value::t());
    }
    let object = match &a[0] {
        Value::Cons(cell) => cell.borrow().car.clone(),
        value => value.clone(),
    };
    let ok = !object.is_nil()
        && i.sym_id(&object).is_some_and(|id| {
            let symbol = i.obarray.symbol(id);
            symbol.uninterned || !i.symbol_name(id).starts_with(':')
        });
    Ok(Value::from_bool(ok))
}

/// GNU's canonical order for `event-modifiers' output:
/// (meta control shift hyper super alt triple double down drag click).
fn mod_list_rank(m: &str) -> usize {
    [
        "meta", "control", "shift", "hyper", "super", "alt", "triple", "double", "down", "drag",
        "click",
    ]
    .iter()
    .position(|x| *x == m)
    .unwrap_or(usize::MAX)
}

/// Parse an event symbol NAME into (char-mods, click-mods, base-name):
/// prefixes `A- C- H- M- S- s-' then `down- drag- double- triple-'.
/// (`click-' is not a name prefix in GNU.)
fn parse_event_symbol(name: &str) -> (Vec<String>, Vec<String>, String) {
    let mut cmods = Vec::new();
    let mut kmods = Vec::new();
    let mut name = name.to_string();
    // GNU parse_modifiers_uncached scans all modifier prefixes in ONE
    // pass, in the order written (drag-M-x parses both).
    loop {
        let mut hit = false;
        for (p, m) in [
            ("A-", "alt"),
            ("C-", "control"),
            ("H-", "hyper"),
            ("M-", "meta"),
            ("S-", "shift"),
            ("s-", "super"),
            ("down-", "down"),
            ("drag-", "drag"),
            ("double-", "double"),
            ("triple-", "triple"),
            ("up-", "up"),
        ] {
            if let Some(r) = name.strip_prefix(p) {
                if p.len() == 2 {
                    cmods.push(m.to_string());
                } else {
                    kmods.push(m.to_string());
                }
                name = r.to_string();
                hit = true;
                break;
            }
        }
        if !hit {
            break;
        }
    }
    (cmods, kmods, name)
}

/// `internal-event-symbol-parse-modifiers' (keyboard.c
/// Fevent_symbol_parse_modifiers): parse SYMBOL like a key lookup
/// does — caches `event-symbol-elements'/`-mask' and returns
/// (BASE . MOD-SYMS).
pub(crate) fn f_internal_event_symbol_parse(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let Some(symbol) = i.sym_id(&a[0]) else {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    };
    parse_event_symbol_cached(i, symbol)?;
    let property = i.q("event-symbol-elements");
    Ok(i.get_prop(symbol, property))
}

fn event_modifier_bit(i: &mut Interp, event: &Value, mask: i128) -> Result<bool, Flow> {
    let bits = crate::lisp::builtins::call_native_builtin(
        i,
        "logand",
        vec![event.clone(), Value::Int(mask)],
    )?;
    let zero = crate::lisp::builtins::call_canonical_builtin(i, "zerop", vec![bits])?;
    Ok(zero.is_nil())
}

pub(super) fn f_event_modifiers(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let event = a[0].clone();
    if crate::lisp::builtins::call_native_builtin(i, "stringp", vec![event.clone()])?.truthy() {
        return Ok(Value::Nil);
    }
    let mut event_type = event;
    if crate::lisp::builtins::call_native_builtin(i, "listp", vec![event_type.clone()])?.truthy() {
        event_type = crate::lisp::builtins::call_canonical_builtin(i, "car", vec![event_type])?;
    }
    if crate::lisp::builtins::call_native_builtin(i, "symbolp", vec![event_type.clone()])?.truthy()
    {
        let parsed = crate::lisp::builtins::call_native_builtin(
            i,
            "internal-event-symbol-parse-modifiers",
            vec![event_type],
        )?;
        return crate::lisp::builtins::call_canonical_builtin(i, "cdr", vec![parsed]);
    }
    let mask = CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
    let character = crate::lisp::builtins::call_native_builtin(
        i,
        "logand",
        vec![event_type.clone(), Value::Int(!mask)],
    )?;
    let mut modifiers = Vec::new();
    if event_modifier_bit(i, &event_type, CHAR_META)? {
        modifiers.push("meta");
    }
    if event_modifier_bit(i, &event_type, CHAR_CTL)?
        || crate::lisp::builtins::call_native_builtin(
            i,
            "<",
            vec![character.clone(), Value::Int(32)],
        )?
        .truthy()
    {
        modifiers.push("control");
    }
    let shifted = if event_modifier_bit(i, &event_type, CHAR_SHIFT)? {
        true
    } else {
        let lower =
            crate::lisp::builtins::call_native_builtin(i, "downcase", vec![character.clone()])?;
        crate::lisp::builtins::call_canonical_builtin(i, "/=", vec![character, lower])?.truthy()
    };
    if shifted {
        modifiers.push("shift");
    }
    for (bit, name) in [
        (CHAR_HYPER, "hyper"),
        (CHAR_SUPER, "super"),
        (CHAR_ALT, "alt"),
    ] {
        if event_modifier_bit(i, &event_type, bit)? {
            modifiers.push(name);
        }
    }
    modifiers.reverse();
    Ok(Value::list(
        modifiers
            .into_iter()
            .map(|name| Value::Sym(i.intern(name)))
            .collect(),
    ))
}

fn event_basic(i: &mut Interp, event: &Value) -> EvalResult {
    if crate::lisp::builtins::call_native_builtin(i, "stringp", vec![event.clone()])?.truthy() {
        return Ok(Value::Nil);
    }
    let mut event = event.clone();
    if crate::lisp::builtins::call_native_builtin(i, "consp", vec![event.clone()])?.truthy() {
        event = crate::lisp::builtins::call_canonical_builtin(i, "car", vec![event])?;
    }
    if crate::lisp::builtins::call_native_builtin(i, "symbolp", vec![event.clone()])?.truthy() {
        let property = Value::Sym(i.q("event-symbol-elements"));
        let parsed = crate::lisp::builtins::call_native_builtin(i, "get", vec![event, property])?;
        return crate::lisp::builtins::call_canonical_builtin(i, "car", vec![parsed]);
    }
    let base = crate::lisp::builtins::call_native_builtin(
        i,
        "logand",
        vec![event, Value::Int(CHAR_ALT - 1)],
    )?;
    let uncontrolled =
        if crate::lisp::builtins::call_native_builtin(i, "<", vec![base.clone(), Value::Int(32)])?
            .truthy()
        {
            crate::lisp::builtins::call_native_builtin(i, "logior", vec![base, Value::Int(64)])?
        } else {
            base
        };
    // The dumped Lisp body catches errors from downcase only. Offer signals
    // with that scope active so observers and clause selection match GNU.
    let depth = i.case_handlers.len();
    i.case_handlers
        .push(crate::lisp::error::CaseHandler::single(Value::Sym(
            sym::ERROR,
        )));
    let result =
        crate::lisp::builtins::call_native_builtin(i, "downcase", vec![uncontrolled.clone()]);
    let result = i.offer_flow(result);
    i.case_handlers.truncate(depth);
    match result {
        Err(Flow::Signal(signal)) if signal.claims_frame(depth) => {
            i.finish_quit_unwind();
            Ok(uncontrolled)
        }
        Err(flow) => {
            i.rebase_quit_unwind();
            Err(flow)
        }
        result => result,
    }
}
/// GNU parse_modifiers caches the parsing result, including an empty base.
/// Existing cons caches are authoritative and parsing does not apply modifiers.
fn parse_event_symbol_cached(i: &mut Interp, s: SymId) -> EvalResult {
    let esem = i.q("event-symbol-element-mask");
    let cached = i.get_prop(s, esem);
    if matches!(cached, Value::Cons(_)) {
        return Ok(cached);
    }
    let name = i.symbol_name(s);
    let (cmods, kmods, base) = parse_event_symbol(&name);
    const ORDER: &[(&str, i128)] = &[
        ("alt", CHAR_ALT),
        ("control", CHAR_CTL),
        ("hyper", CHAR_HYPER),
        ("meta", CHAR_META),
        ("shift", CHAR_SHIFT),
        ("super", CHAR_SUPER),
        ("double", 1 << 4),
        ("triple", 1 << 5),
        ("up", 1 << 0),
        ("down", 1 << 1),
        ("drag", 1 << 2),
    ];
    let mut mask: i128 = 0;
    let mut lispy: Vec<String> = Vec::new();
    for (modname, bit) in ORDER {
        if cmods.iter().any(|m| m == modname) || kmods.iter().any(|m| m == modname) {
            mask |= bit;
            lispy.push(modname.to_string());
        }
    }
    // GNU caches the parse even with zero modifiers (f1 → (f1 . nil)).
    // click modifier rules from parse_modifiers_uncached: `mouse-D'
    // (single digit) unless a down/drag/double/triple modifier is
    // present; `wheel-…' unless double/triple.
    let no_dir = !kmods
        .iter()
        .any(|m| matches!(m.as_str(), "down" | "drag" | "double" | "triple"));
    let is_mouse_d =
        base.len() == 7 && base.starts_with("mouse-") && base.as_bytes()[6].is_ascii_digit();
    let is_wheel = base.len() > 6
        && base.starts_with("wheel-")
        && !kmods
            .iter()
            .any(|m| matches!(m.as_str(), "double" | "triple"));
    if (no_dir && is_mouse_d) || is_wheel {
        mask |= 1 << 3;
        lispy.push("click".to_string());
    }
    // GNU's lispy list order: meta control shift hyper super alt
    // triple double down drag click (lispy_modifier_list walks the
    // bit table descending).
    lispy.sort_by_key(|m| mod_list_rank(m));
    let base_value = crate::lisp::builtins::data::f_intern(i, vec![Value::string(base)].into())?;
    let ese = i.q("event-symbol-elements");
    let mod_syms: Vec<Value> = lispy.iter().map(|m| Value::Sym(i.intern(m))).collect();
    let result = Value::list(vec![base_value.clone(), Value::Int(mask)]);
    i.put_prop(s, esem, result.clone());
    i.put_prop(
        s,
        ese,
        Value::cons(base_value.clone(), Value::list(mod_syms)),
    );
    Ok(result)
}

fn event_cache_car(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().car.clone()),
        value => Err(i.wrong_type_mut("listp", value)),
    }
}

fn event_cache_cdr(i: &mut Interp, value: &Value) -> EvalResult {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::Cons(cell) => Ok(cell.borrow().cdr.clone()),
        value => Err(i.wrong_type_mut("listp", value)),
    }
}

pub(crate) fn reorder_event_symbol(i: &mut Interp, symbol: SymId) -> EvalResult {
    let parsed = parse_event_symbol_cached(i, symbol)?;
    let base = event_cache_car(i, &parsed)?;
    if i.sym_id(&base).is_none() {
        return Err(i.wrong_type_mut("symbolp", &base));
    }
    let tail = event_cache_cdr(i, &parsed)?;
    let mask = event_cache_car(i, &tail)?;
    if !mask.fixnump() {
        return Err(i.wrong_type_mut("fixnump", &mask));
    }
    apply_event_modifiers(i, mask.int().unwrap(), base)
}

pub(super) fn f_event_basic_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    event_basic(i, &a[0])
}

pub(crate) fn f_event_convert_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut base = Value::Nil;
    let mut mods = 0i128;
    let mut tail = a[0].clone();
    // GNU FOR_EACH_TAIL_SAFE uses Brent's walk and stops on a cycle
    // without signalling. Preserve its detection timing: a second base
    // can signal before the cycle is detected.
    let mut tortoise = tail.clone();
    let mut span = 2usize;
    let mut blocks = 0isize;
    let mut steps = 2u16;
    while let Value::Cons(cell) = tail {
        let (event, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        // Only a symbol followed by another element can be a modifier.
        let modifier = if matches!(next, Value::Cons(_)) {
            i.sym_id(&event)
                .map(|id| match i.symbol_name(id).as_str() {
                    "A" | "alt" => CHAR_ALT,
                    "C" | "ctrl" | "control" => CHAR_CTL,
                    "H" | "hyper" => CHAR_HYPER,
                    "M" | "meta" => CHAR_META,
                    "S" | "shift" => CHAR_SHIFT,
                    "s" | "super" => CHAR_SUPER,
                    "up" => 1,
                    "down" => 2,
                    "drag" => 4,
                    "click" => 8,
                    "double" => 16,
                    "triple" => 32,
                    _ => 0,
                })
                .unwrap_or(0)
        } else {
            0
        };
        if modifier != 0 {
            mods |= modifier;
        } else if !base.is_nil() {
            return Err(i.error("Two bases given in one event"));
        } else {
            base = event;
        }
        tail = next;
        steps = steps.wrapping_sub(1);
        let check_cycle = if steps != 0 {
            true
        } else {
            blocks -= 1;
            if blocks > 0 {
                true
            } else {
                span = span.saturating_mul(2);
                steps = span as u16;
                blocks = (span >> 16) as isize;
                tortoise = tail.clone();
                false
            }
        };
        if check_cycle && eq_values(&tail, &tortoise) {
            break;
        }
    }
    if let Some(id) = i.sym_id(&base) {
        let name = i.symbol_name(id);
        let mut chars = name.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            base = Value::Int(crate::lisp::value::lisp_char_code(ch));
        }
    }
    if let Value::Int(code) = &base {
        if base.fixnump() {
            return Ok(Value::Int(crate::editor::apply_mods_ev(*code, mods)));
        }
    }
    let Some(symbol) = i.sym_id(&base) else {
        return Err(i.error("Invalid base event"));
    };
    let base = i.sym(symbol);
    apply_event_modifiers(i, mods, base)
}

/// keyboard.c apply_modifiers: cache and kind inheritance are distinct from
/// parse_modifiers, which must not populate this cache itself.
fn apply_event_modifiers(i: &mut Interp, modifiers: i128, base: Value) -> EvalResult {
    let mods = i128::from(modifiers as i32);
    if base.fixnump() {
        return Ok(Value::Int(base.int().unwrap() | mods));
    }
    let Some(symbol) = i.sym_id(&base) else {
        return Err(i.wrong_type_mut("symbolp", &base));
    };
    // GNU apply_modifiers keeps the original base name, including any
    // existing prefixes. Click is absent from both the name and cache key.
    let cache_property = i.q("modifier-cache");
    let cache = i.get_prop(symbol, cache_property);
    let index = mods & !8;
    let mut entries = cache.clone();
    let mut tortoise = entries.clone();
    let mut span = 1usize;
    let mut steps = 0usize;
    let mut converted = None;
    while let Value::Cons(entry) = entries {
        let pair = entry.borrow().car.clone();
        entries = entry.borrow().cdr.clone();
        if let Value::Cons(pair) = pair {
            if matches!(pair.borrow().car, Value::Int(n) if n == index) {
                converted = Some(pair.borrow().cdr.clone());
                break;
            }
        }
        steps += 1;
        if eq_values(&entries, &tortoise) {
            break;
        }
        if steps == span {
            tortoise = entries.clone();
            span = span.saturating_mul(2);
            steps = 0;
        }
    }
    let converted = if let Some(value) = converted {
        value
    } else {
        let mut name = String::new();
        for (bit, prefix) in [
            (CHAR_ALT, "A-"),
            (CHAR_CTL, "C-"),
            (CHAR_HYPER, "H-"),
            (CHAR_META, "M-"),
            (CHAR_SHIFT, "S-"),
            (CHAR_SUPER, "s-"),
            (16, "double-"),
            (32, "triple-"),
            (1, "up-"),
            (2, "down-"),
            (4, "drag-"),
        ] {
            if mods & bit != 0 {
                name.push_str(prefix);
            }
        }
        name.push_str(&i.symbol_name(symbol));
        let name = Value::string_with_kind(name, true);
        if let Value::Str(string) = &name {
            i.mark_multibyte(string);
        }
        let converted = crate::lisp::builtins::data::f_intern(i, vec![name].into())?;
        let entry = Value::cons(Value::Int(index), converted.clone());
        i.put_prop(symbol, cache_property, Value::cons(entry, cache));
        converted
    };
    let Some(converted_symbol) = i.sym_id(&converted) else {
        return Err(i.wrong_type_mut("symbolp", &converted));
    };
    let kind = i.q("event-kind");
    if i.get_prop(converted_symbol, kind).is_nil() {
        let value = i.get_prop(symbol, kind);
        if !value.is_nil() {
            i.put_prop(converted_symbol, kind, value);
        }
    }
    Ok(converted)
}

pub(super) fn f_listify_key_sequence(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let mut events = crate::lisp::builtins::seq::seq_to_vec(i, &a[0])?;
    // GNU subr.el preserves vector and multibyte-string elements.
    if !matches!(a[0], Value::Vec(_)) && !crate::lisp::builtins::strfn::string_multibyte_p(i, &a[0])
    {
        for event in &mut events {
            if crate::lisp::builtins::arith::f_gt(i, vec![event.clone(), Value::Int(127)].into())?
                .truthy()
            {
                let modifier = i.q("listify-key-sequence-1");
                let modifier = i.eval(&Value::Sym(modifier))?;
                *event = crate::lisp::builtins::arith::f_logxor(
                    i,
                    vec![event.clone(), modifier].into(),
                )?;
            }
        }
    }
    Ok(Value::list(events))
}

pub(super) fn f_key_valid_p(_i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    // GNU's top-level shape: `\`[^ ]+\( [^ ]+\)*\'' — non-empty words
    // joined by SINGLE spaces (double spaces or leading/trailing are
    // invalid).
    if s.is_empty() || s.split(' ').any(|t| t.is_empty()) {
        return Ok(Value::Nil);
    }
    for tok in s.split(' ').filter(|t| !t.is_empty()) {
        // GNU `key-valid-p': modifiers may only be given in the fixed
        // order A-C-H-M-S-s — each optional, but no repeats or
        // reordering.
        let mut rest = tok;
        for m in ["A-", "C-", "H-", "M-", "S-", "s-"] {
            if let Some(r) = rest.strip_prefix(m) {
                rest = r;
            }
        }
        let chs: Vec<crate::lisp::value::EmacsChar> = rest.chars().collect();
        let ok = if chs.len() == 1 {
            // Single char: printable only (GNU rejects control chars
            // and unibyte 128..255 high bytes).
            let c = (chs[0]).code() as u32;
            c >= 32 && !(128..=255).contains(&c)
        } else if rest.starts_with('<') && rest.ends_with('>') {
            // <NAME>: NAME is [-_A-Za-z0-9]+ with NO inner modifier
            // prefixes (GNU rejects `<M-C-down>').
            let inner = &rest[1..rest.len() - 1];
            !inner.is_empty()
                && inner
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && !["A-", "C-", "H-", "M-", "S-", "s-"]
                    .iter()
                    .any(|m| inner.starts_with(m))
        } else {
            // Shorthand names NUL RET TAB LFD ESC SPC DEL, as the whole
            // word or after a `-X' boundary (GNU's `\<…$').
            ["NUL", "RET", "TAB", "LFD", "ESC", "SPC", "DEL"]
                .iter()
                .any(|n| {
                    rest == *n
                        || (rest.ends_with(n)
                            && rest.len() > n.len()
                            && rest.chars().nth(rest.len() - n.len() - 1) == Some('-'.into()))
                })
        };
        if !ok {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::t())
}

pub(super) fn popup_car_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}

pub(super) fn popup_fcar(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => Ok(c.borrow().car.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

pub(super) fn popup_fcdr(i: &mut Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Cons(c) => Ok(c.borrow().cdr.clone()),
        Value::Nil => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("listp", other)),
    }
}

/// menu.c x_popup_menu_1 position decode.  Errors match GNU's
/// `wrong-type-argument' outcomes for every malformed shape.
pub(crate) fn decode_popup_position(i: &mut Interp, pos: &Value) -> Result<(), Flow> {
    if matches!(pos, Value::Nil) {
        return Ok(()); // obsolete precalculate call
    }
    if matches!(pos, Value::Sym(s) if *s == sym::T) {
        return Ok(()); // current mouse position
    }
    let Value::Cons(pc) = pos else {
        return Err(i.wrong_type_mut("listp", pos));
    };
    let (pcar, pcdr) = {
        let b = pc.borrow();
        (b.car.clone(), b.cdr.clone())
    };
    if matches!(&pcar, Value::Sym(s) if matches!(&*i.symbol_name(*s), "menu-bar" | "tab-bar" | "tool-bar"))
    {
        return Ok(());
    }
    if let Value::Cons(c2) = &pcdr {
        if matches!(&c2.borrow().car, Value::Sym(s) if &*i.symbol_name(*s) == "tab-bar") {
            return Ok(());
        }
    }
    let (window, x, y);
    if let Value::Cons(xyc) = &pcar {
        // ((X Y) WINDOW): window = Fcar(Fcdr pos) — Fcar on an
        // atomic tail signals listp on the tail.
        window = popup_fcar(i, &pcdr)?;
        x = xyc.borrow().car.clone();
        // y = Fcar(XCDR(xy)): raw cdr then Fcar — atom → listp.
        y = popup_fcar(i, &xyc.borrow().cdr)?;
    } else {
        // EVENT: tem = CAR_SAFE(CDR_SAFE pos) = cadr (atomic cdr
        // → nil); then Fcar/Fcdr chains signal listp on atoms.
        let tem = popup_car_safe(&pcdr);
        window = popup_fcar(i, &tem)?;
        let t1 = popup_fcdr(i, &tem)?;
        let _tem2 = popup_fcar(i, &t1)?;
        let t2 = popup_fcdr(i, &t1)?;
        let tem3 = popup_fcar(i, &t2)?;
        x = popup_fcar(i, &tem3)?;
        y = popup_fcdr(i, &tem3)?;
    }
    if matches!(x, Value::Nil) && matches!(y, Value::Nil) {
        return Ok(()); // get_current_pos_p
    }
    match window {
        Value::Frame(_) | Value::Window(_) => {}
        w => return Err(i.wrong_type_mut("windowp", &w)),
    }
    for v in [&x, &y] {
        if !matches!(v, Value::Int(_)) {
            return Err(i.wrong_type_mut("integerp", v));
        }
    }
    Ok(())
}

/// `popup-menu' — GNU's Lisp def (menu-bar.el): MENU resolves via
///   (keymapp menu)                    → x-popup-menu → nil in batch
///   (listp menu && keymapp (car))     → list of keymaps → nil
///   (not (listp menu))                → map is nil → nil
///   else easy-menu-create-menu on (car menu) / (cdr menu), whose
///   per-item converter signals "Invalid menu item in easymenu".
/// In --batch x-popup-menu always returns nil so a successfully
/// validated menu still finishes nil without popping anything.
pub(super) fn f_popup_menu(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    fn aref_check(i: &mut Interp, v: &Value, idx: usize) -> Result<(), Flow> {
        let len = match v {
            Value::Vec(rv) => rv.borrow().len(),
            _ => 0,
        };
        if idx >= len {
            let s = i.q("args-out-of-range");
            return Err(i.signal_data(s, vec![v.clone(), Value::Int(idx as i128)]));
        }
        Ok(())
    }
    fn keywordp(i: &Interp, v: &Value) -> bool {
        matches!(v, Value::Sym(id) if i.symbol_name(*id).starts_with(':'))
    }
    // easy-menu-convert-item-1: validate one menu item.
    fn check_item(i: &mut Interp, item: &Value) -> Result<(), Flow> {
        match item {
            Value::Str(_) => Ok(()),
            Value::Cons(c) => {
                let cdr = c.borrow().cdr.clone();
                // (NAME . COMMAND): COMMAND is used verbatim when it is a
                // keymap, else it is the submenu's item list.
                if crate::editor::is_keymap(i, &cdr) {
                    return Ok(());
                }
                check_items(i, &cdr)
            }
            Value::Vec(_) => {
                // (aref item 1) is evaluated before any keyword walking.
                aref_check(i, item, 1)?;
                let (len, active) = match item {
                    Value::Vec(rv) => {
                        let b = rv.borrow();
                        (
                            b.len(),
                            if b.len() > 2 {
                                b[2].clone()
                            } else {
                                Value::t()
                            },
                        )
                    }
                    _ => unreachable!(),
                };
                if keywordp(i, &active) {
                    // Keyword pairs start at index 2; both aref calls run
                    // while (1+ count) < len is still unchecked.
                    let mut count = 2usize;
                    while len > count {
                        aref_check(i, item, count)?;
                        aref_check(i, item, count + 1)?;
                        count += 2;
                    }
                }
                Ok(())
            }
            _ => Err(i.error("Invalid menu item in easymenu")),
        }
    }
    // easy-menu-create-menu: leading KEYWORD ARG pairs are consumed first
    // (any keyword counts; unknown ones are ignored by the pcase), then a
    // mapcar walks the remaining items in order — an atom tail signals
    // listp only after the items before it pass.
    fn check_items(i: &mut Interp, items: &Value) -> Result<(), Flow> {
        let mut cur = items.clone();
        loop {
            match &cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
                    // (and items (cdr items) (keywordp (car items)))
                    match &cdr {
                        Value::Nil => break,
                        Value::Cons(cc) => {
                            if keywordp(i, &car) {
                                cur = cc.borrow().cdr.clone();
                                continue;
                            }
                            break;
                        }
                        // (cdr items) is a non-nil atom and CAR is a
                        // keyword → (cddr items) errors on the atom.
                        other => {
                            if keywordp(i, &car) {
                                return Err(i.wrong_type_mut("listp", other));
                            }
                            break;
                        }
                    }
                }
                // (cdr items) on an atom head: the while condition's first
                // access already signals listp.
                other => return Err(i.wrong_type_mut("listp", other)),
            }
        }
        while let Value::Cons(c) = cur {
            let (item, rest) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            check_item(i, &item)?;
            cur = rest;
        }
        match cur {
            Value::Nil => Ok(()),
            other => Err(i.wrong_type_mut("listp", &other)),
        }
    }
    // GNU `popup-menu' is Lisp (menu-bar.el): POSITION first goes
    // through `popup-menu-normalize-position', then the normalized
    // value is decoded by `x-popup-menu' (menu.c x_popup_menu_1).
    //
    //   normalize:
    //     nil                       → posn-at-point → decodes nil
    //     posnp                     → ((X Y) WINDOW): decode checks
    //                                 windowp + integerp on the
    //                                 posn-x-y pair
    //     car = touchscreen-begin/  → posn-at-point → decodes nil
    //       end
    //     eventp                    → normalize (event-end position)
    //       recursively.  event-end = nth(if consp(nth 2) 2 1);
    //       nthcdr errors listp on the WHOLE list (dotted tail),
    //       nth's Fcar on an atomic tail errors listp on the atom.
    //       A nil event-end falls back to posn-at-point.
    //     else                      → decode as-is
    //
    //   decode (menu.c x_popup_menu_1):
    //     t / car ∈ {menu-bar,tab-bar,tool-bar} / cadr = tab-bar
    //                               → get_current_pos → nil
    //     atom                      → Fcar → listp on the atom
    //     cons, car cons            → ((X Y) W): window = cadr;
    //                                 x = XCAR(car), y = car(cdr(car))
    //     cons, other               → EVENT: tem = cadr-or-nil;
    //                                 window=Fcar(tem), tem2=Fcar(Fcdr
    //                                 tem), tem3=Fcar(Fcdr(Fcdr tem)),
    //                                 x=Fcar(tem3), y=Fcdr(tem3) —
    //                                 Fcar/Fcdr on atoms signal listp
    //     x & y nil                 → get_current_pos → nil
    //     else                      → windowp|framep window, then
    //                                 integerp x, integerp y.

    // fns.c Fnthcdr+Fnth: counting down, a non-nil non-cons tail
    // signals listp on the ORIGINAL list; a count that lands on a tail
    // returns it as-is (atom included) and nth Fcar's it (atom →
    // listp on the atom).
    fn nth(i: &mut Interp, n: i128, list: &Value) -> Result<Value, Flow> {
        let mut tail = list.clone();
        for _ in 0..n.max(0) {
            match tail {
                Value::Cons(c) => tail = c.borrow().cdr.clone(),
                Value::Nil => return Ok(Value::Nil),
                _ => return Err(i.wrong_type_mut("listp", list)),
            }
        }
        match tail {
            Value::Cons(c) => Ok(c.borrow().car.clone()),
            Value::Nil => Ok(Value::Nil),
            atom => Err(i.wrong_type_mut("listp", &atom)),
        }
    }
    use crate::lisp::builtins::misc::{
        popup_car_safe as car_safe, popup_fcar as fcar, popup_fcdr as fcdr,
    };
    // subr.el posnp — the cdr walks use plain `cdr' (signals on an
    // atom), the heads use `car-safe'.
    fn posnp_check(i: &mut Interp, v: &Value) -> Result<bool, Flow> {
        if !matches!(car_safe(v), Value::Window(_)) {
            return Ok(false);
        }
        let obj = fcdr(i, v)?;
        if matches!(car_safe(&obj), Value::Cons(_)) {
            return Ok(false); // AREA-OR-POS must satisfy `atom'
        }
        let obj = fcdr(i, &obj)?;
        if !matches!(car_safe(&car_safe(&obj)), Value::Int(_)) {
            return Ok(false); // XOFFSET
        }
        let d3 = fcdr(i, &obj)?;
        Ok(matches!(car_safe(&d3), Value::Int(_))) // TIMESTAMP
    }
    fn eventp(i: &Interp, v: &Value) -> bool {
        let head = match v {
            Value::Int(_) => return true,
            Value::Sym(_) => v.clone(),
            Value::Cons(c) => c.borrow().car.clone(),
            _ => return false,
        };
        match &head {
            Value::Sym(s) => !i.symbol_name(*s).starts_with(':'),
            _ => false,
        }
    }

    let mut pos = arg(&a, 1).clone();
    let decoded = 'norm: loop {
        match &pos {
            Value::Nil => break 'norm None, // → posn-at-point → nil
            _ => {}
        }
        if posnp_check(i, &pos)? {
            // normalize → ((X Y) WINDOW): windowp is already proven;
            // the x/y decode checks integerp on the posn-x-y pair.
            let xy = nth(i, 2, &pos)?; // posn-x-y
            let x = fcar(i, &xy)?;
            let y = fcdr(i, &xy)?;
            for v in [&x, &y] {
                if !matches!(v, Value::Int(_)) {
                    return Err(i.wrong_type_mut("integerp", v));
                }
            }
            break 'norm None;
        }
        if let Value::Cons(c) = &pos {
            if matches!(&c.borrow().car, Value::Sym(s) if matches!(&*i.symbol_name(*s), "touchscreen-begin" | "touchscreen-end"))
            {
                break 'norm None; // → posn-at-point
            }
        }
        if eventp(i, &pos) {
            match &pos {
                Value::Cons(_) => {
                    let inner = nth(i, 2, &pos)?;
                    let n = if matches!(inner, Value::Cons(_)) {
                        2
                    } else {
                        1
                    };
                    match nth(i, n, &pos)? {
                        Value::Nil => break 'norm None, // → posn-at-point
                        end => {
                            pos = end;
                            continue 'norm;
                        }
                    }
                }
                _ => break 'norm None, // bare atom event → posn-at-point
            }
        }
        break 'norm Some(pos.clone()); // (t position)
    };
    if let Some(p) = decoded {
        decode_popup_position(i, &p)?;
    }
    match &a[0] {
        Value::Nil => return Ok(Value::Nil),
        menu if crate::editor::is_keymap(i, menu) => {}
        Value::Cons(c) => {
            let (car, cdr) = (c.borrow().car.clone(), c.borrow().cdr.clone());
            if crate::editor::is_keymap(i, &car) {
                // A list whose first element is a keymap is a list of panes.
            } else {
                check_items(i, &cdr)?;
            }
        }
        _ => return Ok(Value::Nil),
    }
    if i
        .selected_frame
        .as_ref()
        .is_some_and(|frame| crate::editor::frame_is_graphical(i, frame))
    {
        let (x, y) = i.gui_mouse_position.unwrap_or((0, 0));
        i.gui_popup_menu = Some((a[0].clone(), x, y));
    }
    Ok(Value::Nil)
}

/// `tooltip-mode' — minor-mode semantics: no arg reports, 'toggle
/// flips, numeric <= 0 disables, else enables.  State lives in the
/// `tooltip-mode' variable (seeded t like GNU).
pub(super) fn f_tooltip_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = i.q("tooltip-mode");
    let cur = i.symbol_value(sid).truthy();
    let on = match arg(&a, 0) {
        // GNU minor-mode commands called from Lisp treat nil as ON.
        Value::Nil => true,
        Value::Sym(s) if i.symbol_name(s) == "toggle" => !cur,
        Value::Int(n) => n > 0,
        _ => true,
    };
    let v = Value::from_bool(on);
    let _ = i.set_symbol(sid, v.clone());
    Ok(v)
}

pub(super) fn f_mouse_position(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let fr = match sel_frame(i) {
        Some(f) => Value::Frame(f),
        None => Value::Nil,
    };
    // Emacs: (FRAME nil) on a tty with no mouse.
    Ok(Value::list(vec![fr, Value::Nil]))
}

/// `display-popup-menus-p' — GNU Fdisplay_popup_menus_p: decode_frame
/// first (arg must be a frame when non-nil), then the tty result nil.
pub(super) fn f_display_popup_menus_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let frame = decode_display(i, &arg(&a, 0))?;
    Ok(Value::from_bool(crate::editor::frame_is_graphical(i, &frame)))
}

pub(super) fn f_current_input_mode(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    // GNU batch: (INTERRUPT FLOW META QUIT) = (t nil t 7).
    Ok(Value::list(vec![
        Value::t(),
        Value::Nil,
        Value::t(),
        Value::Int(7),
    ]))
}

pub(super) fn f_lossage_size(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        if v.truthy() {
            match v {
                // GNU keyboard.c Flossage_size: FIXNATP required, then
                // MIN_NUM_RECENT_KEYS (100) .. MAX_NUM_RECENT_KEYS
                // (INT_MAX/EMACS_INT_WIDTH = 33554431 on 64-bit),
                // both reported via user-error.
                Value::Int(n) if *n >= 0 => {
                    if *n < 100 {
                        return Err(i.signal_data(
                            sym::USER_ERROR,
                            vec![Value::string("Value must be >= 100")],
                        ));
                    }
                    if *n > 33_554_431 {
                        return Err(i.signal_data(
                            sym::USER_ERROR,
                            vec![Value::string("Value must be <= 33554431")],
                        ));
                    }
                }
                _ => {
                    return Err(i.signal_data(
                        sym::USER_ERROR,
                        vec![Value::string("Value must be a positive integer")],
                    ));
                }
            }
        }
    }
    Ok(Value::Int(300))
}

pub(super) fn f_mouse_position_root(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::cons(Value::Int(0), Value::Int(0)))
}

/// `remember-mouse-glyph` — GNU checks arg 0: nil selects the current
/// frame then fails the window-system check; a live frame → nil.
pub(super) fn f_remember_mouse_glyph(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match &a[0] {
        Value::Nil => Err(i.error("Window system frame should be used")),
        Value::Frame(_) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("frame-live-p", other)),
    }
}
