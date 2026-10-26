//! Byte-code interpreter for GNU `#[...]' byte-code objects.
//!
//! `make-byte-code' builds a `Value::Lambda' carrying the raw
//! `#[ARGDESC BYTESTR CONSTS DEPTH ...]' slots in `bc_items'; calls to
//! such objects route here (see `eval/lambda.rs::call_lambda_inner').  The
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

use super::EvalResult;
use super::builtins::eq_values;
use super::error::Flow;
use super::eval::{ExcursionState, Interp, RestrictionState};
use super::value::{Lambda, SymId, Value, lisp_char_code};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

mod opcodes;

/// Invocation-local VM state. Handlers drop before unwind records, then the
/// value stack, as when these were independent locals in run().
struct Frame {
    handlers: Vec<BcHandler>,
    unwinds: Vec<Unwind>,
    stack: Vec<Value>,
    pc: usize,
}

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
    /// Old byte-code stores a progn body instead of a function.
    Forms(Value),
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
    CondCase { index: usize },
}

fn pop_handler_scope(i: &mut Interp, handler: &BcHandler) {
    match &handler.kind {
        BcHandlerKind::Catch { .. } => {
            i.catch_tags.pop();
        }
        BcHandlerKind::CondCase { .. } => {
            i.case_handlers.pop();
        }
    }
}

/// `bc_items' has byte-code object shape `[argdesc str consts depth ..]'.
pub(crate) fn is_byte_code(items: &[Value]) -> bool {
    items.len() >= 4
        && matches!(&items[1], Value::Str(_))
        && matches!(&items[2], Value::Vec(_))
        && items[3].int().is_some()
}
/// Decoded byte-code program cached on `Lambda::bc_prog'. The code slot
/// is mutable through its string, so identity alone cannot validate it.
pub(crate) struct BcProg {
    /// Identity of `items[0]' (argdesc) for staleness checks.
    argdesc_key: usize,
    /// Identity of `items[1]' (byte string) for staleness checks.
    code_key: usize,
    /// Source contents at decode time, including raw-byte proxy scalars.
    source_text: super::text::EmacsText,
    /// Decoded instruction bytes.
    code: Vec<u8>,
    /// `parse_argdesc' results.
    req: usize,
    max: usize,
    has_rest: bool,
    dyn_names: Option<Rc<Vec<SymId>>>,
}

/// Cheap identity for cache validation: Rc payloads compare by
/// pointer, immediates by value bits.  A false hit only skips
/// validation work — the slot Values are still compared structurally
/// cheap — so collisions merely rebuild.
fn slot_key(v: &Value) -> usize {
    match v {
        Value::Cons(r) => Rc::as_ptr(r) as usize,
        Value::Str(r) => Rc::as_ptr(r) as usize,
        Value::Vec(r) => Rc::as_ptr(r) as usize,
        Value::Lambda(r) => Rc::as_ptr(r) as usize,
        Value::Hash(r) => Rc::as_ptr(r) as usize,
        Value::Record(r) => Rc::as_ptr(r) as usize,
        Value::Int(n) => (*n as usize) ^ 0x9e37_79b9,
        Value::Nil => 0,
        Value::Sym(s) => (*s as usize) | 1,
        _ => usize::MAX,
    }
}

/// Arity check for a `#[ARGDESC ...]' literal before its arguments
/// are evaluated — GNU's compiled-call contract.  `who' is the symbol
/// shown in `wrong-number-of-arguments' (nativecomp surface).
pub(crate) fn check_argdesc(
    i: &mut Interp,
    items: &[Value],
    n: usize,
    who: &Value,
) -> Result<(), Flow> {
    let argdesc = items.first().cloned().unwrap_or(Value::Nil);
    let (req, max, has_rest, _) = parse_argdesc(i, &argdesc);
    if n < req || (!has_rest && n > max) {
        return Err(i.wrong_number_of_args(who, n as i128));
    }
    Ok(())
}

/// Execute a `#[ARGDESC BYTESTR CONSTS DEPTH ...]' byte-code object.
/// `l' is the Lambda built by `make-byte-code'/the `#[...]' reader;
/// `argv' are the already-evaluated call arguments.
pub(crate) fn exec(
    i: &mut Interp,
    l: &Lambda,
    argv: crate::lisp::Argv,
    who: Option<&Value>,
    named: bool,
) -> EvalResult {
    let items = l
        .bc_items
        .as_ref()
        .map(|r| r.borrow().clone())
        .unwrap_or_default();
    let argdesc = items.first().cloned().unwrap_or(Value::Nil);
    let bytestr = items.get(1).cloned().unwrap_or(Value::Nil);
    let consts = items.get(2).cloned().unwrap_or(Value::Nil);
    // Retain both code-slot identity and its decoded source contents:
    // in-place string edits preserve the Rc address. Constants stay live
    // and are read from their vector for every constant instruction.
    let prog = {
        let key_a = slot_key(&argdesc);
        let key_c = slot_key(&bytestr);
        let hit = l.bc_prog.borrow().as_ref().is_some_and(|p| {
            p.argdesc_key == key_a
                && p.code_key == key_c
                && matches!(&bytestr, Value::Str(s) if *s.borrow() == p.source_text)
        });
        if hit {
            l.bc_prog.borrow().clone().unwrap()
        } else {
            let code = match code_bytes(i, &bytestr) {
                Ok(c) => c,
                Err(e) => return Err(e),
            };
            let (req, max, has_rest, dyn_names) = parse_argdesc(i, &argdesc);
            let p = Rc::new(BcProg {
                argdesc_key: key_a,
                code_key: key_c,
                source_text: match &bytestr {
                    Value::Str(s) => s.borrow().clone(),
                    _ => unreachable!(), // code_bytes validated the string.
                },
                code,
                req,
                max,
                has_rest,
                dyn_names: dyn_names.map(Rc::new),
            });
            *l.bc_prog.borrow_mut() = Some(p.clone());
            p
        }
    };
    // Depth guard for debugging runaway bytecode recursion
    // (REMACS_BC_DEPTH=N dumps recent frames and aborts).
    if let Some(d) = std::env::var("REMACS_BC_DEPTH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        i.bc_depth += 1;
        if i.bc_depth > d {
            eprintln!("=== bc depth {} ===", i.bc_depth);
            for (f, _) in i.lisp_stack.iter().rev().take(40).rev() {
                eprintln!("  {}", i.princ_to_string(f));
            }
            std::process::exit(3);
        }
        let r = run(i, &prog, &bytestr, &consts, &argv, who, named);
        i.bc_depth -= 1;
        return r;
    }
    run(i, &prog, &bytestr, &consts, &argv, who, named)
}

/// `(byte-code BYTESTR CONSTVEC DEPTH)' entry point — no arguments.
pub(crate) fn exec_raw(i: &mut Interp, bytestr: &Value, consts: &Value) -> EvalResult {
    let code = code_bytes(i, bytestr)?;
    let (req, max, has_rest, dyn_names) = parse_argdesc(i, &Value::Int(0));
    let prog = BcProg {
        argdesc_key: 0,
        code_key: 0,
        source_text: match bytestr {
            Value::Str(s) => s.borrow().clone(),
            _ => unreachable!(), // code_bytes validated the string.
        },
        code,
        req,
        max,
        has_rest,
        dyn_names: dyn_names.map(Rc::new),
    };
    run(i, &prog, bytestr, consts, &[], None, false)
}

fn run(
    i: &mut Interp,
    prog: &BcProg,
    source: &Value,
    consts: &Value,
    argv: &[Value],
    who: Option<&Value>,
    named: bool,
) -> EvalResult {
    let Value::Vec(consts) = consts else {
        return Err(i.wrong_type_mut("vectorp", consts));
    };
    let (req, max, has_rest, dyn_names) =
        (prog.req, prog.max, prog.has_rest, prog.dyn_names.as_deref());
    // GNU `funcall_subr' passes ALL supplied args to an n-adic subr
    // (`maxargs == MANY || maxargs > 8' -> `aMANY (numargs, args)');
    // the native trampoline then reads only its declared signature,
    // silently dropping the tail — e.g. `(funcall 'cl--struct-new-class
    // <11 args>)' succeeds while `add-abbrev' (max 3) still wnas.
    // `named' marks the dumped-native-twin surface (GNU's
    // funcall_subr passes all args to an aMANY trampoline).
    let short_argv: Vec<Value>;
    let argv = if named && !has_rest && max > 8 && argv.len() > max {
        short_argv = argv[..max].to_vec();
        &short_argv[..]
    } else {
        argv
    };
    if argv.len() < req || (!has_rest && argv.len() > max) {
        // GNU exec_byte_code signals ((MIN . NONREST) NARGS) for
        // anonymous `#[...]' literals.  Under nativecomp a *named*
        // compiled function shows its `#<subr NAME>' object — our
        // compiled twins reach exec through call_lambda_inner, so the
        // datum is the lambda itself; only nameless literals keep
        // the pair.
        let desc = who
            .cloned()
            .unwrap_or_else(|| Value::cons(Value::Int(req as i128), Value::Int(max as i128)));
        return Err(i.wrong_number_of_args(&desc, argv.len() as i128));
    }
    let specmark = i.specbind_depth();
    // Dynamic-style argdesc (a list of names): each parameter name is
    // specbind'd to its argument — GNU binds them in order, &rest
    // takes the tail as a list.
    if let Some(names) = dyn_names {
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
            if let Err(flow) = i.specbind(*name, v) {
                let flow = offer_exit(i, flow);
                return finish_bindings(i, specmark, Err(flow));
            }
            idx += 1;
        }
    }
    // Lexical argdesc: the args sit on the value stack (first arg
    // deepest, last on top).  Dynamic code leaves the stack empty and
    // reaches args through varref on the specbinds above.
    let stack: Vec<Value> = if dyn_names.is_none() {
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
    let mut frame = Frame {
        stack,
        unwinds: Vec::new(),
        handlers: Vec::new(),
        pc: 0,
    };
    // `byte-pushcatch' records its tag on `catch_tags' too, so a `throw'
    // subr call from inside byte code sees it like a Lisp-level `catch'
    // (GNU's Bpushcatch pushes onto the same handlerlist Fthrow scans).
    let catch_depth = i.catch_tags.len();
    let case_depth = i.case_handlers.len();
    let r = dispatch(i, prog, source, consts.as_ref(), &mut frame);
    i.catch_tags.truncate(catch_depth);
    i.case_handlers.truncate(case_depth);
    // Any unwind records still open (a nonlocal exit or an unbalanced
    // `unbind' count) get cleaned here — GNU's specpdl does the same.
    let cleanup = i.with_quit_preserved(|i| {
        let result = unwind_all(i, &mut frame.unwinds);
        i.unbind_raw_to(specmark).and(result)
    });
    cleanup.and(r)
}

fn finish_bindings(i: &mut Interp, depth: usize, result: EvalResult) -> EvalResult {
    i.unbind_to(depth)?;
    result
}

fn offer_exit(i: &mut Interp, flow: Flow) -> Flow {
    match i.offer_flow(Err(flow)) {
        Err(flow) => flow,
        Ok(_) => unreachable!("offering a nonlocal exit cannot return a value"),
    }
}

fn unwind_all(i: &mut Interp, unwinds: &mut Vec<Unwind>) -> Result<(), Flow> {
    let mut result = Ok(());
    while let Some(u) = unwinds.pop() {
        if let Err(flow) = unwind_one(i, u) {
            // Outer cleanup still runs; its exit replaces an earlier one.
            result = Err(offer_exit(i, flow));
        }
    }
    result
}

fn unwind_one(i: &mut Interp, u: Unwind) -> Result<(), Flow> {
    match u {
        Unwind::VarBind => return i.unbind_bindings(1),
        Unwind::SaveExcursion(s) => i.restore_excursion_state(s),
        Unwind::SaveRestriction(s) => i.restore_restriction_state(s),
        Unwind::SaveCurrentBuffer(old) => {
            if i.buffer_live(old) {
                i.set_current_buffer(old);
            }
        }
        Unwind::Thunk(t) => {
            // GNU runs the thunk during unwind; a failing thunk's
            // error propagates out of the unwind.
            i.apply(&t, Vec::new())?;
        }
        Unwind::Forms(forms) => {
            i.eval_progn(&forms)?;
        }
    }
    Ok(())
}

fn matching_handler(flow: &Flow, handlers: &[BcHandler]) -> Option<usize> {
    handlers.iter().rposition(|h| match (flow, &h.kind) {
        (Flow::Signal(signal), BcHandlerKind::CondCase { index }) => signal.claims_frame(*index),
        (Flow::Throw(tag, _), BcHandlerKind::Catch { tag: target }) => eq_values(tag, target),
        _ => false,
    })
}

/// GNU unwinds to each handler's saved binding depth before removing that
/// handler. Cleanup may signal/throw to that very handler, even when it did
/// not match the original exit. Preserve the chosen destination until a new
/// exit replaces it; a normal cleanup can mutate condition/tag objects.
fn unwind_flow(
    i: &mut Interp,
    mut flow: Flow,
    unwinds: &mut Vec<Unwind>,
    handlers: &mut Vec<BcHandler>,
) -> Result<(usize, usize, Value), Flow> {
    let mut target = matching_handler(&flow, handlers);
    'search: loop {
        let boundary = handlers.last().map_or(0, |h| h.unwind_depth);
        let result = i.with_quit_preserved(|i| {
            while unwinds.len() > boundary {
                unwind_one(i, unwinds.pop().unwrap())?;
            }
            Ok(())
        });
        if let Err(replacement) = result {
            flow = offer_exit(i, replacement);
            target = matching_handler(&flow, handlers);
            continue 'search;
        }
        let Some(h) = handlers.pop() else {
            return Err(flow);
        };
        let index = handlers.len();
        pop_handler_scope(i, &h);
        if target == Some(index) {
            i.finish_quit_unwind();
            let value = match flow {
                Flow::Signal(signal) => signal.condition,
                Flow::Throw(_, value) => value,
                _ => unreachable!("only signals and throws select VM handlers"),
            };
            return Ok((h.dest, h.sp, value));
        }
        i.rebase_quit_unwind();
    }
}

/// The main dispatch loop.  On a Lisp signal/throw the handler stack
/// is consulted innermost-first; unclaimed exits propagate.
fn dispatch(
    i: &mut Interp,
    prog: &BcProg,
    source: &Value,
    consts: &RefCell<Vec<Value>>,
    frame: &mut Frame,
) -> EvalResult {
    let counter = i
        .bc_quit_counter
        .clone()
        .unwrap_or_else(|| Rc::new(std::cell::Cell::new(1)));
    let saved = i.bc_quit_counter.replace(counter.clone());
    let result = dispatch_inner(i, prog, source, consts, frame, &counter);
    i.bc_quit_counter = saved;
    result
}

fn dispatch_inner(
    i: &mut Interp,
    prog: &BcProg,
    source: &Value,
    consts: &RefCell<Vec<Value>>,
    frame: &mut Frame,
    quitcounter: &std::cell::Cell<u8>,
) -> EvalResult {
    let mut code = Cow::Borrowed(prog.code.as_slice());
    let mut source_text = Cow::Borrowed(prog.source_text.as_str());
    let mut seen_tick = None;
    loop {
        i.profiler_poll();
        // A Lisp call, watcher or unwind handler may edit future
        // instructions in this very invocation. The global tick makes
        // the common, unchanged instruction path constant-time.
        if seen_tick != Some(i.string_data_tick) {
            if let Value::Str(s) = source {
                if s.borrow().as_str() != source_text.as_ref() {
                    code = Cow::Owned(code_bytes(i, source)?);
                    source_text = Cow::Owned(s.borrow().clone());
                }
            }
            seen_tick = Some(i.string_data_tick);
        }
        let op = *code.get(frame.pc).unwrap_or(&0) as usize;
        if i.dbg_flags & 0x40 != 0 {
            let Frame {
                stack,
                unwinds,
                handlers,
                pc,
            } = &*frame;
            eprintln!(
                "[bc] pc={} op={} sp={} uw={} hd={} top={}",
                pc,
                op,
                stack.len(),
                unwinds.len(),
                handlers.len(),
                stack
                    .last()
                    .map(|v| i
                        .princ_to_string(v)
                        .chars()
                        .take(30)
                        .collect::<crate::lisp::text::EmacsText>())
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
                if !matches!(
                    stack.get(fpos),
                    Some(Value::Sym(_)) | Some(Value::Subr(_)) | Some(Value::Lambda(_))
                ) {
                    let dump: Vec<crate::lisp::text::EmacsText> = stack
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(24).collect())
                        .collect();
                    eprintln!("[bc]   STACK={:?}", dump);
                }
            }
        }
        frame.pc += 1;
        let step = opcodes::step_op(i, code.as_ref(), consts, frame, op, quitcounter);
        // C's Fsignal runs observers before unwinding VM cleanup records.
        // Direct opcode failures (car, arithmetic, etc.) have not passed
        // through apply/eval's normal signal-delivery boundary yet.
        let step = match step {
            Step::Flow(flow) => Step::Flow(offer_exit(i, flow)),
            other => other,
        };
        match step {
            Step::Next => {}
            Step::Return(v) => return Ok(v),
            Step::Flow(flow) => {
                let (dest, sp, value) =
                    unwind_flow(i, flow, &mut frame.unwinds, &mut frame.handlers)?;
                frame.stack.truncate(sp);
                frame.stack.push(value);
                frame.pc = dest;
                quitcounter.set(u8::MAX);
            }
        }
    }
}

enum Step {
    Next,
    Return(Value),
    Flow(Flow),
}

/// Argument-description decode.  Integer argdesc packs
/// `required | (&rest << 7) | ((required+optional) << 8)'; a list is
/// the literal (dynamic-binding) arglist; nil means zero arguments.
/// Returns (required, max-positional, has-rest, dynamic-names).
pub(crate) fn parse_argdesc(i: &mut Interp, a: &Value) -> (usize, usize, bool, Option<Vec<SymId>>) {
    match a {
        Value::Int(n) => {
            let req = (*n & 0x7f).max(0) as usize;
            let rest = (*n >> 7) & 1 != 0;
            let max = ((*n >> 8) & 0x7f).max(req as i128) as usize;
            (req, max, rest, None)
        }
        Value::Nil => (0, 0, false, Some(Vec::new())),
        Value::Cons(_) => {
            let opt = i.q("&optional");
            let rst = i.q("&rest");
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
