//! Byte-code interpreter for GNU `#[...]' byte-code objects.
//!
//! `make-byte-code' builds a `Value::Lambda' carrying the raw
//! `#[ARGDESC BYTESTR CONSTS DEPTH ...]' slots in `bc_items'; calls to
//! such objects route here (see `eval.rs::call_lambda_inner').  The
//! reader's `#[...]' literals land in the same shape, so `.elc' files
//! produced by `byte-compile-file' execute directly.
//!
//! Operand encoding (bytecomp.el `byte-defop' table):
//!   - ops 8..47 (varref/varset/varbind/call/unbind): low 3 bits hold
//!     the operand inline (0..5); 6 → one operand byte follows;
//!     7 → a two-byte little-endian operand follows.
//!   - 1..7: `stack-ref' with immediate depth.
//!   - 192..255: `constant' with immediate index.
//!   - goto/pushcatch/pushconditioncase/constant2/stack-set2: u16 LE.
//!   - listN/concatN/insertN/stack-set/discardN: u8.

use super::builtins::{eq_values, equal_values};
use super::error::Flow;
use super::eval::{ExcursionState, Interp, RestrictionState};
use super::obarray::sym;
use super::value::{lisp_char_code, Lambda, SymId, Value};
use super::EvalResult;

/// One `specpdl'-style record pushed by varbind/save-*/unwind-protect
/// and popped by `byte-unbind'.
enum Unwind {
    /// One entry pushed through `Interp::specbind'.
    VarBind,
    SaveExcursion(ExcursionState),
    SaveRestriction(RestrictionState),
    SaveCurrentBuffer(usize),
    /// `unwind-protect' thunk: called on unwind (normal or nonlocal).
    Thunk(Value),
}

/// A catch/condition-case frame record from pushcatch/pushconditioncase.
struct BcHandler {
    /// Byte offset to jump to when the handler fires.
    dest: usize,
    /// Value-stack depth to restore before jumping.
    sp: usize,
    /// `unwinds' depth recorded at push time — inner bindings made
    /// inside the protected body are unwound on handler entry.
    unwind_depth: usize,
    kind: BcHandlerKind,
}

enum BcHandlerKind {
    Catch { tag: Value },
    CondCase { conditions: Value },
}

/// `bc_items' has byte-code object shape `[argdesc str consts depth ..]'.
pub(crate) fn is_byte_code(items: &[Value]) -> bool {
    items.len() >= 4
        && matches!(&items[1], Value::Str(_))
        && matches!(&items[2], Value::Vec(_))
        && items[3].int().is_some()
}

/// Execute a `#[ARGDESC BYTESTR CONSTS DEPTH ...]' byte-code object.
/// `l' is the Lambda built by `make-byte-code'/the `#[...]' reader;
/// `argv' are the already-evaluated call arguments.
pub(crate) fn exec(i: &mut Interp, l: &Lambda, argv: Vec<Value>) -> EvalResult {
    let items = l
        .bc_items
        .as_ref()
        .map(|r| r.borrow().clone())
        .unwrap_or_default();
    let argdesc = items.first().cloned().unwrap_or(Value::Nil);
    let bytestr = items.get(1).cloned().unwrap_or(Value::Nil);
    let consts = items.get(2).cloned().unwrap_or(Value::Nil);
    run(i, &argdesc, &bytestr, &consts, argv)
}

/// `(byte-code BYTESTR CONSTVEC DEPTH)' entry point — no arguments.
pub(crate) fn exec_raw(i: &mut Interp, bytestr: &Value, consts: &Value) -> EvalResult {
    run(i, &Value::Int(0), bytestr, consts, Vec::new())
}

fn run(
    i: &mut Interp,
    argdesc: &Value,
    bytestr: &Value,
    consts: &Value,
    argv: Vec<Value>,
) -> EvalResult {
    let code = code_bytes(i, bytestr)?;
    let consts = match consts {
        Value::Vec(v) => v.borrow().clone(),
        _ => return Err(i.wrong_type_mut("vectorp", consts)),
    };
    // Arity check against the args-description.
    let (req, max, has_rest, dyn_names) = parse_argdesc(i, argdesc);
    if argv.len() < req || (!has_rest && argv.len() > max) {
        return Err(i.wrong_number_of_args(argdesc, argv.len() as i128));
    }
    let specmark = i.specbind_depth();
    // Dynamic-style argdesc (a list of names): each parameter name is
    // specbind'd to its argument — GNU binds them in order, &rest
    // takes the tail as a list.
    if let Some(names) = &dyn_names {
        let mut idx = 0usize;
        for name in names {
            let v = if idx < argv.len() {
                // A &rest parameter (the last name when has_rest) gets
                // the remaining arguments as a list.
                if has_rest && idx + 1 == names.len() {
                    Value::list(argv[idx..].to_vec())
                } else {
                    argv[idx].clone()
                }
            } else {
                Value::Nil
            };
            i.specbind(*name, v)?;
            idx += 1;
        }
    }
    // Lexical argdesc: the args sit on the value stack (first arg
    // deepest, last on top).  Dynamic code leaves the stack empty and
    // reaches args through varref on the specbinds above.
    let mut stack: Vec<Value> = if dyn_names.is_none() {
        let mut s = Vec::with_capacity(max + has_rest as usize);
        let pushed = argv.len().min(max);
        s.extend(argv[..pushed].iter().cloned());
        if max < argv.len() {
            // &rest: the tail goes into one list slot.
            s.push(Value::list(argv[pushed..].to_vec()));
        } else {
            // Pad missing optionals (and an unused &rest slot) with
            // nil, like GNU's `for (i = nargs - rest; i < nonrest;
            // i++) PUSH (Qnil)'.
            for _ in (argv.len() as isize - has_rest as isize)..(max as isize) {
                s.push(Value::Nil);
            }
        }
        s
    } else {
        Vec::new()
    };
    let mut unwinds: Vec<Unwind> = Vec::new();
    let mut handlers: Vec<BcHandler> = Vec::new();
    // `byte-pushcatch' records its tag on `catch_tags' too, so a `throw'
    // subr call from inside byte code sees it like a Lisp-level `catch'
    // (GNU's Bpushcatch pushes onto the same handlerlist Fthrow scans).
    let catch_depth = i.catch_tags.len();
    let r = dispatch(i, &code, &consts, &mut stack, &mut unwinds, &mut handlers);
    i.catch_tags.truncate(catch_depth);
    // Any unwind records still open (a nonlocal exit or an unbalanced
    // `unbind' count) get cleaned here — GNU's specpdl does the same.
    unwind_all(i, &mut unwinds);
    let _ = i.unbind_to(specmark);
    r
}

fn unwind_all(i: &mut Interp, unwinds: &mut Vec<Unwind>) {
    while let Some(u) = unwinds.pop() {
        unwind_one(i, u);
    }
}

fn unwind_one(i: &mut Interp, u: Unwind) {
    match u {
        Unwind::VarBind => {
            let _ = i.unbind(1);
        }
        Unwind::SaveExcursion(s) => i.restore_excursion_state(s),
        Unwind::SaveRestriction(s) => i.restore_restriction_state(s),
        Unwind::SaveCurrentBuffer(old) => i.set_current_buffer(old),
        Unwind::Thunk(t) => {
            // GNU runs the thunk during unwind; a failing thunk's
            // error propagates out of the unwind.
            let _ = i.apply(&t, Vec::new());
        }
    }
}

/// The main dispatch loop.  On a Lisp signal/throw the handler stack
/// is consulted innermost-first; unclaimed exits propagate.
fn dispatch(
    i: &mut Interp,
    code: &[u8],
    consts: &[Value],
    stack: &mut Vec<Value>,
    unwinds: &mut Vec<Unwind>,
    handlers: &mut Vec<BcHandler>,
) -> EvalResult {
    let mut pc = 0usize;
    loop {
        if i.quit_flag {
            i.quit_flag = false;
            return Err(Flow::Quit);
        }
        let op = *code.get(pc).unwrap_or(&0) as usize;
        if std::env::var_os("REMACS_TRACE_BC").is_some() {
            eprintln!(
                "[bc] pc={} op={} sp={} uw={} hd={} top={}",
                pc,
                op,
                stack.len(),
                unwinds.len(),
                handlers.len(),
                stack
                    .last()
                    .map(|v| i.princ_to_string(v).chars().take(30).collect::<String>())
                    .unwrap_or_default()
            );
            // call ops: show the function + args actually passed.
            if (32..=39).contains(&op) || op == 128 + 128 {
                let nargs = op - 32;
                let fpos = stack.len().wrapping_sub(nargs + 1);
                let fnv = stack
                    .get(fpos)
                    .map(|v| i.princ_to_string(v))
                    .unwrap_or_default();
                eprintln!("[bc]   CALL nargs={} fn={}", nargs, fnv);
                if !matches!(stack.get(fpos), Some(Value::Sym(_)) | Some(Value::Subr(_)) | Some(Value::Lambda(_))) {
                    let dump: Vec<String> = stack
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(24).collect())
                        .collect();
                    eprintln!("[bc]   STACK={:?}", dump);
                }
            }
        }
        pc += 1;
        match step_op(i, code, consts, stack, unwinds, handlers, &mut pc, op) {
            Step::Next => {}
            Step::Return(v) => return Ok(v),
            Step::Flow(Flow::Signal(sig, data, offered)) => {
                // Innermost matching condition-case claims the signal.
                let mut claimed = false;
                while let Some(h) = handlers.last() {
                    let hit = match &h.kind {
                        BcHandlerKind::CondCase { conditions } => {
                            i.signal_matches(&sig, conditions)
                        }
                        _ => false,
                    };
                    let h = handlers.pop().unwrap();
                    if matches!(h.kind, BcHandlerKind::Catch { .. }) {
                        i.catch_tags.pop();
                    }
                    if hit {
                        while unwinds.len() > h.unwind_depth {
                            let u = unwinds.pop().unwrap();
                            unwind_one(i, u);
                        }
                        stack.truncate(h.sp);
                        stack.push(Value::cons(sig.clone(), data.clone()));
                        pc = h.dest;
                        claimed = true;
                        break;
                    }
                    // Non-matching handlers between the signal and the
                    // claiming handler are discarded (GNU parity).
                }
                if !claimed {
                    return Err(Flow::Signal(sig, data, offered));
                }
            }
            Step::Flow(Flow::Throw(tag, val)) => {
                let mut claimed = false;
                while let Some(h) = handlers.last() {
                    let hit = match &h.kind {
                        BcHandlerKind::Catch { tag: t } => eq_values(t, &tag),
                        _ => false,
                    };
                    let h = handlers.pop().unwrap();
                    if matches!(h.kind, BcHandlerKind::Catch { .. }) {
                        i.catch_tags.pop();
                    }
                    if hit {
                        while unwinds.len() > h.unwind_depth {
                            let u = unwinds.pop().unwrap();
                            unwind_one(i, u);
                        }
                        stack.truncate(h.sp);
                        stack.push(val.clone());
                        pc = h.dest;
                        claimed = true;
                        break;
                    }
                }
                if !claimed {
                    return Err(Flow::Throw(tag, val));
                }
            }
            Step::Flow(f) => return Err(f),
        }
    }
}

enum Step {
    Next,
    Return(Value),
    Flow(Flow),
}

/// Execute one opcode; `pc` is already past the opcode byte.
#[allow(clippy::too_many_arguments)]
fn step_op(
    i: &mut Interp,
    code: &[u8],
    consts: &[Value],
    stack: &mut Vec<Value>,
    unwinds: &mut Vec<Unwind>,
    handlers: &mut Vec<BcHandler>,
    pc: &mut usize,
    op: usize,
) -> Step {
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
        ($n:expr) => {
            consts.get($n).cloned().unwrap_or(Value::Nil)
        };
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
    // Pop `count` operands and apply the named subr.
    macro_rules! scall {
        ($name:expr, $count:expr) => {{
            let n = $count;
            let fpos = stack.len().saturating_sub(n);
            let argv: Vec<Value> = stack.split_off(fpos);
            let s = Value::Sym(i.intern($name));
            let v = r!(i.apply(&s, argv));
            push!(v);
        }};
    }
    match op {
        // Reserved invalid opcode — GNU treats it as corrupted
        // byte code.
        0 => fail!(i.error("Invalid byte code")),
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
            r!(i.specbind(id, v));
            unwinds.push(Unwind::VarBind);
        }
        // call: fn + N args on the stack.
        32..=39 => {
            let n = operand!();
            let fpos = stack.len().saturating_sub(n + 1);
            let fun = stack.get(fpos).cloned().unwrap_or(Value::Nil);
            if std::env::var_os("DBG_FUNCALL").is_some() {
                let bad = if let Value::Cons(c) = &fun {
                    let car = c.borrow().car.clone();
                    let lam = i.intern("lambda");
                    let mac = i.intern("macro");
                    !(i.sym_is(&car, lam) || i.sym_is(&car, mac))
                } else {
                    false
                };
                if bad {
                    let st: Vec<String> = stack
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(60).collect())
                        .collect();
                    let cs: Vec<String> = consts
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(60).collect())
                        .collect();
                    eprintln!(
                        "[bc-call fun={} n={} fpos={} pc={}]\n  stack: [{}]\n  code: {}\n  consts: [{}]",
                        i.princ_to_string(&fun),
                        n,
                        fpos,
                        *pc,
                        st.join(" "),
                        code.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" "),
                        cs.join(" "),
                    );
                }
            }
            let argv: Vec<Value> = stack.split_off(fpos + 1);
            stack.pop(); // fn slot
            let v = r!(i.apply(&fun, argv));
            push!(v);
        }
        // unbind N unwind records.
        40..=47 => {
            let n = operand!();
            for _ in 0..n {
                if let Some(u) = unwinds.pop() {
                    unwind_one(i, u);
                }
            }
        }
        // pophandler: drop the innermost handler.
        48 => {
            if let Some(h) = handlers.pop()
                && matches!(h.kind, BcHandlerKind::Catch { .. })
            {
                i.catch_tags.pop();
            }
        }
        // pushconditioncase DEST: stack top = conditions list.
        49 => {
            let dest = fetch2!();
            let conditions = pop!();
            handlers.push(BcHandler {
                dest,
                sp: stack.len(),
                unwind_depth: unwinds.len(),
                kind: BcHandlerKind::CondCase { conditions },
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
        64 => {
            let v = pop!();
            push!(match &v {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => Value::Nil,
            });
        }
        65 => {
            let v = pop!();
            push!(match &v {
                Value::Cons(c) => c.borrow().cdr.clone(),
                _ => Value::Nil,
            });
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
        83 | 84 => {
            // sub1 / add1
            let v = pop!();
            let d = if op == 83 { -1.0 } else { 1.0 };
            push!(match v {
                Value::Int(n) => Value::Int(n + d as i128),
                Value::Float(f) => Value::float(*f + d),
                other => fail!(i.wrong_type_mut("number-or-marker-p", &other)),
            });
        }
        85 => scall!("=", 2),
        86 => scall!(">", 2),
        87 => scall!("<", 2),
        88 => scall!("<=", 2),
        89 => scall!(">=", 2),
        90 => scall!("-", 2),
        91 => {
            let v = pop!();
            push!(match v {
                Value::Int(n) => Value::Int(-n),
                Value::Float(f) => Value::float(-*f),
                other => fail!(i.wrong_type_mut("number-or-marker-p", &other)),
            });
        }
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
        130 => *pc = fetch2!(),
        131 => {
            let dest = fetch2!();
            let v = pop!();
            if v.is_nil() {
                *pc = dest;
            }
        }
        132 => {
            let dest = fetch2!();
            let v = pop!();
            if v.truthy() {
                *pc = dest;
            }
        }
        133 => {
            let dest = fetch2!();
            if tos!().is_nil() {
                *pc = dest;
            } else {
                pop!();
            }
        }
        134 => {
            let dest = fetch2!();
            if tos!().truthy() {
                *pc = dest;
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
        // unwind-protect: stack top is the compiled thunk; it runs
        // when the next `unbind' (or nonlocal exit) pops it.
        142 => {
            let thunk = pop!();
            unwinds.push(Unwind::Thunk(thunk));
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
            push!(Value::from_bool(equal_values(i, &a, &b)));
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
            push!(Value::from_bool(matches!(v, Value::Int(_) | Value::Float(_))));
        }
        168 => {
            let v = pop!();
            push!(Value::from_bool(matches!(v, Value::Int(_))));
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
            let s = Value::Sym(i.intern("concat"));
            let v = r!(i.apply(&s, argv));
            push!(v);
        }
        177 => {
            let n = fetch!() as usize;
            let fpos = stack.len().saturating_sub(n);
            let argv: Vec<Value> = stack.split_off(fpos);
            let s = Value::Sym(i.intern("insert"));
            let v = r!(i.apply(&s, argv));
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
            let gethash = Value::Sym(i.intern("gethash"));
            let sentinel = Value::Sym(i.intern("remacs--switch-miss"));
            let v = r!(i.apply(&gethash, vec![key, table, sentinel.clone()]));
            if !eq_values(&v, &sentinel) {
                if let Some(n) = v.int() {
                    *pc = n.max(0) as usize;
                }
            }
        }
        192..=255 => push!(konst!(op - 192)),
        _ => fail!(i.error("Invalid byte code")),
    }
    Step::Next
}

/// Argument-description decode.  Integer argdesc packs
/// `required | (&rest << 7) | ((required+optional) << 8)'; a list is
/// the literal (dynamic-binding) arglist; nil means zero arguments.
/// Returns (required, max-positional, has-rest, dynamic-names).
fn parse_argdesc(
    i: &mut Interp,
    a: &Value,
) -> (usize, usize, bool, Option<Vec<SymId>>) {
    match a {
        Value::Int(n) => {
            let req = (*n & 0x7f).max(0) as usize;
            let rest = (*n >> 7) & 1 != 0;
            let max = ((*n >> 8) & 0x7f).max(req as i128) as usize;
            (req, max, rest, None)
        }
        Value::Nil => (0, 0, false, Some(Vec::new())),
        Value::Cons(_) => {
            let opt = i.intern("&optional");
            let rst = i.intern("&rest");
            let mut req = 0usize;
            let mut max = 0usize;
            let mut rest = false;
            let mut names = Vec::new();
            let mut mode = 0; // 0 req, 1 opt, 2 rest
            let mut cur = a.clone();
            while let Value::Cons(c) = cur {
                let (hd, tl) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                match i.sym_id(&hd) {
                    Some(s) if s == opt => mode = 1,
                    Some(s) if s == rst => mode = 2,
                    Some(s) => {
                        names.push(s);
                        match mode {
                            0 => {
                                req += 1;
                                max += 1;
                            }
                            1 => max += 1,
                            _ => rest = true,
                        }
                    }
                    None => {}
                }
                cur = tl;
            }
            (req, max, rest, Some(names))
        }
        _ => (0, usize::MAX, true, None),
    }
}

/// Extract the instruction bytes: the byte-code string is unibyte —
/// each char stands for one byte (eight-bit chars proxy via
/// `lisp_char_code' → strip the 0x3FFF00 tag back to the raw byte).
fn code_bytes(i: &mut Interp, v: &Value) -> Result<Vec<u8>, Flow> {
    let Value::Str(s) = v else {
        return Err(i.wrong_type_mut("stringp", v));
    };
    let s = s.borrow();
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let code = lisp_char_code(c);
        if (0x3FFF00..=0x3FFFFF).contains(&code) {
            out.push((code & 0xff) as u8);
        } else if (0..=0xFF).contains(&code) {
            out.push(code as u8);
        } else {
            // A multibyte char in a byte-code string is not a valid
            // instruction stream.
            return Err(i.error("Invalid byte code (multibyte string)"));
        }
    }
    Ok(out)
}
