//! GNU opcode execution. Preserve the validation order within each case;
//! the parent owns instruction refresh, signal delivery and handler search.

use super::{BcHandler, BcHandlerKind, Frame, Step, Unwind, pop_handler_scope, unwind_one};
use crate::lisp::{
    builtins::{eq_values, equal_values},
    eval::Interp,
    obarray::sym,
    value::Value,
};
use std::cell::RefCell;

/// Execute one opcode; `pc` is already past the opcode byte.
pub(super) fn step_op(
    i: &mut Interp,
    code: &[u8],
    consts: &RefCell<Vec<Value>>,
    frame: &mut Frame,
    op: usize,
    quitcounter: &std::cell::Cell<u8>,
) -> Step {
    let Frame {
        stack,
        unwinds,
        handlers,
        pc,
    } = frame;
    macro_rules! push {
        ($v:expr) => {
            stack.push($v)
        };
    }
    macro_rules! pop {
        () => {
            stack.pop().unwrap_or(Value::Nil)
        };
    }
    macro_rules! tos {
        () => {
            stack.last().cloned().unwrap_or(Value::Nil)
        };
    }
    macro_rules! fetch {
        () => {{
            let b = *code.get(*pc).unwrap_or(&0);
            *pc += 1;
            b
        }};
    }
    macro_rules! fetch2 {
        () => {{
            let lo = fetch!() as usize;
            let hi = fetch!() as usize;
            lo | (hi << 8)
        }};
    }
    macro_rules! operand {
        () => {{
            let k = op & 7;
            match k {
                6 => fetch!() as usize,
                7 => fetch2!(),
                _ => k,
            }
        }};
    }
    macro_rules! konst {
        ($n:expr) => {{
            // Malformed programs must not fabricate nil (or reproduce GNU's
            // unchecked memory read). Check every access: code and constants
            // can be changed by Lisp while this invocation is running.
            let value = consts.borrow().get($n).cloned();
            match value {
                Some(value) => value,
                None => return Step::Flow(i.error("Invalid byte-code")),
            }
        }};
    }
    macro_rules! csym {
        ($n:expr) => {
            i.sym_id(&konst!($n)).unwrap_or(0)
        };
    }
    macro_rules! fail {
        ($f:expr) => {
            return Step::Flow($f)
        };
    }
    macro_rules! r {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(f) => fail!(f),
            }
        };
    }
    macro_rules! branch {
        ($dest:expr) => {{
            let destination = $dest;
            if destination < *pc {
                let count = quitcounter.get().wrapping_add(1);
                quitcounter.set(count);
                if count == 0 {
                    quitcounter.set(1);
                    r!(i.maybe_quit());
                }
            }
            *pc = destination;
        }};
    }
    // Pop `count` operands and apply the named subr.
    macro_rules! scall {
        ($name:expr, $count:expr) => {{
            let n = $count;
            let fpos = stack.len().saturating_sub(n);
            let argv: Vec<Value> = stack.split_off(fpos);
            let v = r!(i.bytecode_subr($name, argv));
            push!(v);
        }};
    }
    match op {
        // Reserved invalid opcode — GNU treats it as corrupted
        // byte code.
        0 => fail!(i.error(format!("Invalid byte opcode: op={op}, ptr={}", *pc - 1))),
        // stack-ref: push the element N below TOS.  N comes from the
        // 3-bit operand like the other 0..47 ops (6 = one following
        // byte, 7 = two following bytes); op 0 is reserved above.
        1..=7 => {
            let n = operand!();
            let idx = stack.len().saturating_sub(n + 1);
            let v = stack.get(idx).cloned().unwrap_or(Value::Nil);
            push!(v);
        }
        // varref: push the symbol-value of consts[operand]; GNU's
        // Bvarref routes unbound cells through Fsymbol_value, which
        // signals `void-variable' rather than exposing the sentinel.
        8..=15 => {
            let n = operand!();
            let id = csym!(n);
            let v = i.symbol_value(id);
            if matches!(v, Value::Sym(s) if s == sym::UNBOUND) {
                fail!(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
            }
            push!(v);
        }
        // varset: pop value, set consts[operand] symbol.
        16..=23 => {
            let n = operand!();
            let id = csym!(n);
            let v = pop!();
            r!(i.set_symbol(id, v));
        }
        // varbind: pop value, specbind consts[operand].
        24..=31 => {
            let n = operand!();
            let id = csym!(n);
            let v = pop!();
            let depth = i.specbind_depth();
            let result = i.specbind(id, v);
            if i.specbind_depth() > depth {
                unwinds.push(Unwind::VarBind);
            }
            r!(result);
        }
        // call: fn sits BELOW its N args on the stack (deepest); GNU
        // `BcallN' DISCARDs N args then reads call_fun = TOP.
        32..=39 => {
            let n = operand!();
            let fpos = stack.len().saturating_sub(n + 1);
            let fun = stack.get(fpos).cloned().unwrap_or(Value::Nil);
            if i.dbg_flags & 0x01 != 0 {
                let bad = if let Value::Cons(c) = &fun {
                    let car = c.borrow().car.clone();
                    let lam = i.q("lambda");
                    let mac = i.q("macro");
                    !(i.sym_is(&car, lam) || i.sym_is(&car, mac))
                } else {
                    false
                };
                if bad {
                    let st: Vec<crate::lisp::text::EmacsText> = stack
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(60).collect())
                        .collect();
                    let cs: Vec<crate::lisp::text::EmacsText> = consts
                        .borrow()
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(60).collect())
                        .collect();
                    eprintln!(
                        "[bc-call fun={} n={} fpos={} pc={}]\n  stack: [{}]\n  code: {}\n  consts: [{}]",
                        i.princ_to_string(&fun),
                        n,
                        fpos,
                        *pc,
                        crate::lisp::text::EmacsText::join(&st, " "),
                        code.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" "),
                        crate::lisp::text::EmacsText::join(&cs, " "),
                    );
                }
            }
            let argv: Vec<Value> = stack.split_off(fpos + 1);
            stack.pop(); // fn slot
            let v = r!(i.apply_bytecode(&fun, argv));
            push!(v);
        }
        // unbind N unwind records.
        40..=47 => {
            let n = operand!();
            r!(i.with_quit_preserved(|i| {
                for _ in 0..n {
                    if let Some(u) = unwinds.pop() {
                        unwind_one(i, u)?;
                    }
                }
                Ok(())
            }));
        }
        // pophandler: drop the innermost handler.
        48 => {
            if let Some(h) = handlers.pop() {
                pop_handler_scope(i, &h);
            }
        }
        // pushconditioncase DEST: stack top = conditions list.
        49 => {
            let dest = fetch2!();
            let conditions = pop!();
            let index = i.case_handlers.len();
            i.case_handlers
                .push(crate::lisp::error::CaseHandler::single(conditions));
            handlers.push(BcHandler {
                dest,
                sp: stack.len(),
                unwind_depth: unwinds.len(),
                kind: BcHandlerKind::CondCase { index },
            });
        }
        // pushcatch DEST: stack top = tag.
        50 => {
            let dest = fetch2!();
            let tag = pop!();
            i.catch_tags.push(tag.clone());
            handlers.push(BcHandler {
                dest,
                sp: stack.len(),
                unwind_depth: unwinds.len(),
                kind: BcHandlerKind::Catch { tag },
            });
        }
        56 => scall!("nth", 2),
        57 => {
            let v = pop!();
            push!(Value::from_bool(i.sym_id(&v).is_some()));
        }
        58 => {
            let v = pop!();
            push!(Value::from_bool(matches!(v, Value::Cons(_))));
        }
        59 => {
            let v = pop!();
            push!(Value::from_bool(matches!(v, Value::Str(_))));
        }
        60 => {
            let v = pop!();
            push!(Value::from_bool(matches!(v, Value::Cons(_) | Value::Nil)));
        }
        61 => {
            let b = pop!();
            let a = pop!();
            push!(Value::from_bool(eq_values(&a, &b)));
        }
        62 => scall!("memq", 2),
        63 => {
            let v = pop!();
            push!(Value::from_bool(v.is_nil()));
        }
        // Bcar/Bcdr: GNU's CAR()/CDR() macros - nil -> nil, cons ->
        // car/cdr, anything else signals `wrong-type-argument listp'
        // (car_error).  Only car-safe/cdr-safe (ops 162/163) swallow.
        64 => {
            let v = pop!();
            let r = match &v {
                Value::Cons(c) => c.borrow().car.clone(),
                Value::Nil => Value::Nil,
                _ => fail!(i.wrong_type_mut("listp", &v)),
            };
            push!(r);
        }
        65 => {
            let v = pop!();
            let r = match &v {
                Value::Cons(c) => c.borrow().cdr.clone(),
                Value::Nil => Value::Nil,
                _ => fail!(i.wrong_type_mut("listp", &v)),
            };
            push!(r);
        }
        66 => {
            let d = pop!();
            let a = pop!();
            push!(Value::cons(a, d));
        }
        67 => scall!("list", 1),
        68 => scall!("list", 2),
        69 => scall!("list", 3),
        70 => scall!("list", 4),
        71 => scall!("length", 1),
        72 => scall!("aref", 2),
        73 => scall!("aset", 3),
        // symbol-value: GNU's Fsymbol_value signals `void-variable'
        // for unbound cells; the raw UNBOUND sentinel must never
        // reach the Lisp stack.
        74 => {
            let v = pop!();
            match i.sym_id(&v) {
                Some(id) => {
                    let val = i.symbol_value(id);
                    if matches!(val, Value::Sym(s) if s == sym::UNBOUND) {
                        fail!(i.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
                    }
                    push!(val);
                }
                None => fail!(i.wrong_type_mut("symbolp", &v)),
            }
        }
        // symbol-function: GNU's Fsymbol_function exposes the void
        // cell as nil (byte-opt--fget walks alias chains and relies
        // on nil terminating them).
        75 => {
            let v = pop!();
            match i.sym_id(&v) {
                Some(id) => match i.symbol_function(id) {
                    Value::Sym(s) if s == sym::UNBOUND => push!(Value::Nil),
                    f => push!(f),
                },
                None => fail!(i.wrong_type_mut("symbolp", &v)),
            }
        }
        76 => scall!("set", 2),
        77 => scall!("fset", 2),
        78 => scall!("get", 2),
        79 => scall!("substring", 3),
        80 => scall!("concat", 2),
        81 => scall!("concat", 3),
        82 => scall!("concat", 4),
        83 => scall!("1-", 1),
        84 => scall!("1+", 1),
        85 => scall!("=", 2),
        86 => scall!(">", 2),
        87 => scall!("<", 2),
        88 => scall!("<=", 2),
        89 => scall!(">=", 2),
        90 => scall!("-", 2),
        91 => scall!("-", 1),
        92 => scall!("+", 2),
        93 => scall!("max", 2),
        94 => scall!("min", 2),
        95 => scall!("*", 2),
        96 => scall!("point", 0),
        98 => scall!("goto-char", 1),
        99 => scall!("insert", 1),
        100 => scall!("point-max", 0),
        101 => scall!("point-min", 0),
        102 => scall!("char-after", 1),
        103 => scall!("following-char", 0),
        104 => scall!("preceding-char", 0),
        105 => scall!("current-column", 0),
        106 => scall!("indent-to", 1),
        108 => scall!("eolp", 0),
        109 => scall!("eobp", 0),
        110 => scall!("bolp", 0),
        111 => scall!("bobp", 0),
        112 => {
            let cur = i.current_buffer;
            match i.buffers.get(cur) {
                Some(b) => push!(Value::Buffer(b.clone())),
                None => push!(Value::Nil),
            }
        }
        113 => scall!("set-buffer", 1),
        // save-current-buffer: records the current buffer for a later
        // `unbind'.
        114 => unwinds.push(Unwind::SaveCurrentBuffer(i.current_buffer)),
        // interactive-p (obsolete since Emacs 23, never emitted).
        116 => push!(Value::Nil),
        117 => scall!("forward-char", 1),
        118 => scall!("forward-word", 1),
        119 => scall!("skip-chars-forward", 2),
        120 => scall!("skip-chars-backward", 2),
        121 => scall!("forward-line", 1),
        122 => scall!("char-syntax", 1),
        123 => scall!("buffer-substring", 2),
        124 => scall!("delete-region", 2),
        125 => scall!("narrow-to-region", 2),
        126 => scall!("widen", 0),
        127 => scall!("end-of-line", 1),
        129 => {
            let n = fetch2!();
            push!(konst!(n));
        }
        130 => {
            let destination = fetch2!();
            branch!(destination);
        }
        131 => {
            let dest = fetch2!();
            let v = pop!();
            if v.is_nil() {
                branch!(dest);
            }
        }
        132 => {
            let dest = fetch2!();
            let v = pop!();
            if v.truthy() {
                branch!(dest);
            }
        }
        133 => {
            let dest = fetch2!();
            if tos!().is_nil() {
                branch!(dest);
            } else {
                pop!();
            }
        }
        134 => {
            let dest = fetch2!();
            if tos!().truthy() {
                branch!(dest);
            } else {
                pop!();
            }
        }
        135 => return Step::Return(pop!()),
        136 => {
            pop!();
        }
        137 => {
            let v = tos!();
            push!(v);
        }
        138 => {
            let s = i.save_excursion_state(false);
            unwinds.push(Unwind::SaveExcursion(s));
        }
        139 => {
            // save-window-excursion (obsolete): GNU saves the window
            // configuration; approximate with save-excursion so the
            // `unbind' count still balances.
            let s = i.save_excursion_state(false);
            unwinds.push(Unwind::SaveExcursion(s));
        }
        140 => {
            let s = i.save_restriction_state();
            unwinds.push(Unwind::SaveRestriction(s));
        }
        // byte-catch: not generated since Emacs 25.
        141 => fail!(i.error("byte-catch opcode is obsolete")),
        // GNU classifies the action at registration, before the body can
        // change a symbol's function cell. Non-functions are legacy forms.
        142 => {
            let action = pop!();
            let function = r!(crate::lisp::builtins::data::functionp_checked(i, &action));
            unwinds.push(if function {
                Unwind::Thunk(action)
            } else {
                Unwind::Forms(action)
            });
        }
        143 => fail!(i.error("byte-condition-case opcode is obsolete")),
        147 => scall!("set-marker", 3),
        148 => scall!("match-beginning", 1),
        149 => scall!("match-end", 1),
        150 => scall!("upcase", 1),
        151 => scall!("downcase", 1),
        152 => scall!("string=", 2),
        153 => scall!("string<", 2),
        154 => {
            let b = pop!();
            let a = pop!();
            push!(Value::from_bool(r!(equal_values(i, &a, &b))));
        }
        155 => scall!("nthcdr", 2),
        156 => scall!("elt", 2),
        157 => scall!("member", 2),
        158 => scall!("assq", 2),
        159 => scall!("nreverse", 1),
        160 => scall!("setcar", 2),
        161 => scall!("setcdr", 2),
        162 => {
            let v = pop!();
            push!(match &v {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => Value::Nil,
            });
        }
        163 => {
            let v = pop!();
            push!(match &v {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => Value::Nil,
            });
        }
        164 => scall!("nconc", 2),
        165 => scall!("/", 2),
        166 => scall!("%", 2),
        167 => {
            let v = pop!();
            push!(Value::from_bool(
                v.integerp() || matches!(v, Value::Float(_))
            ));
        }
        168 => {
            let v = pop!();
            push!(Value::from_bool(v.integerp()));
        }
        175 => {
            let n = fetch!() as usize;
            let items: Vec<Value> = stack.split_off(stack.len().saturating_sub(n));
            push!(Value::list(items));
        }
        176 => {
            let n = fetch!() as usize;
            let fpos = stack.len().saturating_sub(n);
            let argv: Vec<Value> = stack.split_off(fpos);
            let v = r!(i.bytecode_subr("concat", argv));
            push!(v);
        }
        177 => {
            let n = fetch!() as usize;
            let fpos = stack.len().saturating_sub(n);
            let argv: Vec<Value> = stack.split_off(fpos);
            let v = r!(i.bytecode_subr("insert", argv));
            push!(v);
        }
        // stack-set / stack-set2: TOS replaces the element K below it,
        // then TOS is popped.
        178 | 179 => {
            let k = if op == 178 {
                fetch!() as usize
            } else {
                fetch2!()
            };
            let idx = stack.len().saturating_sub(k + 1);
            let v = tos!();
            if idx < stack.len() {
                stack[idx] = v;
            }
            pop!();
        }
        182 => {
            // discardN: operand byte; 0x80 flag means preserve TOS
            // (remove N elements underneath it).
            let n = fetch!() as usize;
            if n & 0x80 != 0 {
                let n = n & 0x7f;
                let t = tos!();
                let keep = stack.len().saturating_sub(n + 1);
                stack.truncate(keep);
                push!(t);
            } else {
                let keep = stack.len().saturating_sub(n);
                stack.truncate(keep);
            }
        }
        183 => {
            // switch: the jump table (hash table value → pc offset)
            // sits on TOS with the dispatch value below it; jump when
            // the lookup hits.
            let table = pop!();
            let key = pop!();
            let sentinel = Value::Sym(i.q("remacs--switch-miss"));
            let v = r!(crate::lisp::builtins::hashfn::f_gethash(
                i,
                vec![key, table, sentinel.clone()].into(),
            ));
            if !eq_values(&v, &sentinel) {
                if let Some(n) = v.int() {
                    branch!(n.max(0) as usize);
                }
            }
        }
        192..=255 => push!(konst!(op - 192)),
        _ => fail!(i.error(format!("Invalid byte opcode: op={op}, ptr={}", *pc - 1))),
    }
    Step::Next
}
