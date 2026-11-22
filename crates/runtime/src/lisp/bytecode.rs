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
//!   - 1..5: `stack-ref' depth is encoded in the opcode; opcode 6 reads
//!     one depth byte, and opcode 7 reads a little-endian u16 depth.
//!   - 192..255: `constant' with immediate index.
//!   - goto/pushcatch/pushconditioncase/constant2/stack-set2: u16 LE.
//!   - listN/concatN/insertN/stack-set/discardN: u8.

use super::EvalResult;
use super::builtins::eq_values;
use super::error::Flow;
use super::eval::{ExcursionState, Interp, RestrictionState};
use super::obarray::sym;
use super::value::{Lambda, SymId, Value, lisp_char_code};
use rustc_hash::FxHashSet;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

mod opcodes;

/// GNU's per-thread byte-code stack has 512 Ki Lisp_Object slots.
const MAX_BYTECODE_STACK_SLOTS: usize = 512 * 1024;

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
    SaveWindowConfiguration(Value),
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
    /// The raw upper argument-count field used in GNU's WNA error data.
    /// For packed descriptors this is signed `argdesc >> 8', so an
    /// unbounded maximum can remain -1 although execution uses `max'.
    wna_max: i128,
    dyn_names: Option<Rc<Vec<SymId>>>,
    dyn_rest_index: Option<usize>,
    invalid_argdesc_at: Option<usize>,
}

/// Parsed call contract for a byte-code argument descriptor. `invalid_at`
/// preserves GNU's validation order: too few required arguments signal WNA
/// before a malformed tail is inspected; once that threshold is met, the
/// malformed arglist signals `invalid-function`.
pub(crate) struct ParsedArgDesc {
    pub req: usize,
    pub max: usize,
    /// Upper bound as GNU reports it, before converting the packed signed
    /// field to the nonnegative execution/storage limit.
    pub reported_max: i128,
    pub has_rest: bool,
    pub dyn_names: Option<Vec<SymId>>,
    pub dyn_rest_index: Option<usize>,
    pub invalid_at: Option<usize>,
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
    // Only packed integer descriptors take GNU's bytecode fast path, which
    // checks arity before evaluating arguments. List descriptors are handled
    // by funcall_lambda's ordinary argument binder after evaluation.
    let Value::Int(_) = argdesc else {
        return Ok(());
    };
    let parsed = parse_argdesc(i, &argdesc);
    if n < parsed.req {
        return Err(i.wrong_number_of_args(who, n as i128));
    }
    if parsed.invalid_at.is_some_and(|invalid_at| n >= invalid_at) {
        return Err(i.signal_data(sym::INVALID_FUNCTION, vec![who.clone()]));
    }
    if !parsed.has_rest && n > parsed.max {
        return Err(i.wrong_number_of_args(who, n as i128));
    }
    Ok(())
}

/// Execute a `#[ARGDESC BYTESTR CONSTS DEPTH ...]' byte-code object.
/// `l' is the Lambda built by `make-byte-code'/the `#[...]' reader;
/// `argv' are the already-evaluated call arguments.
pub(crate) fn exec(
    i: &mut Interp,
    l: &Rc<Lambda>,
    argv: crate::lisp::Argv,
    who: Option<&Value>,
    named: bool,
) -> EvalResult {
    let items = match &l.bc_items {
        Some(items) => items
            .try_borrow()
            .map(|items| items.clone())
            .map_err(|_| i.error("Cannot execute a mutably borrowed byte-code object"))?,
        None => Vec::new(),
    };
    let argdesc = items.first().cloned().unwrap_or(Value::Nil);
    // GNU treats only an integer ARGLIST as the bytecode fast path. Other
    // values are parsed as ordinary formal-argument lists; a non-nil,
    // non-cons value therefore signals invalid-function before bytecode is
    // decoded or executed.
    if !matches!(argdesc, Value::Int(_) | Value::Nil | Value::Cons(_)) {
        return Err(i.signal_data(sym::INVALID_FUNCTION, vec![Value::Lambda(l.clone())]));
    }
    let stack_depth = match items.get(3) {
        Some(Value::Int(depth)) if *depth >= 0 => usize::try_from(*depth).ok(),
        _ => None,
    };
    let Some(stack_depth) = stack_depth else {
        return Err(i.error("Invalid byte-code"));
    };
    let bytestr = items.get(1).cloned().unwrap_or(Value::Nil);
    let consts = items.get(2).cloned().unwrap_or(Value::Nil);
    // Retain both code-slot identity and its decoded source contents:
    // in-place string edits preserve the Rc address. Constants stay live
    // and are read from their vector for every constant instruction.
    let prog = {
        let key_a = slot_key(&argdesc);
        let key_c = slot_key(&bytestr);
        // Arglist conses can be modified in place through `aref'/`setcar'.
        // Their Rc identity is therefore insufficient to validate the
        // cached arity/binding map. Immutable immediate descriptors remain
        // cacheable; mutable or unsupported descriptor shapes are reparsed.
        let cacheable_argdesc = matches!(&argdesc, Value::Nil | Value::Int(_));
        let hit = cacheable_argdesc
            && l.bc_prog.borrow().as_ref().is_some_and(|p| {
                p.argdesc_key == key_a
                    && p.code_key == key_c
                    && matches!(&bytestr, Value::Str(s)
                        if s.try_borrow()
                            .is_ok_and(|source| source.as_str() == p.source_text.as_str()))
            });
        if hit {
            l.bc_prog.borrow().clone().unwrap()
        } else {
            let code = match code_bytes(i, &bytestr) {
                Ok(c) => c,
                Err(e) => return Err(e),
            };
            let parsed_argdesc = parse_argdesc(i, &argdesc);
            let wna_max = match &argdesc {
                Value::Int(descriptor) => *descriptor >> 8,
                _ => parsed_argdesc.reported_max,
            };
            let p = Rc::new(BcProg {
                argdesc_key: key_a,
                code_key: key_c,
                source_text: match &bytestr {
                    Value::Str(s) => s
                        .try_borrow()
                        .map(|source| (*source).clone())
                        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code string"))?,
                    _ => unreachable!(), // code_bytes validated the string.
                },
                code,
                req: parsed_argdesc.req,
                max: parsed_argdesc.max,
                has_rest: parsed_argdesc.has_rest,
                wna_max,
                dyn_names: parsed_argdesc.dyn_names.map(Rc::new),
                dyn_rest_index: parsed_argdesc.dyn_rest_index,
                invalid_argdesc_at: parsed_argdesc.invalid_at,
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
        let r = run(i, &prog, &bytestr, &consts, &argv, who, named, stack_depth);
        i.bc_depth -= 1;
        return r;
    }
    run(i, &prog, &bytestr, &consts, &argv, who, named, stack_depth)
}

/// `(byte-code BYTESTR CONSTVEC DEPTH)' entry point — no arguments.
pub(crate) fn exec_raw(
    i: &mut Interp,
    bytestr: &Value,
    consts: &Value,
    stack_depth: usize,
) -> EvalResult {
    let code = code_bytes(i, bytestr)?;
    let argdesc = parse_argdesc(i, &Value::Int(0));
    let prog = BcProg {
        argdesc_key: 0,
        code_key: 0,
        source_text: match bytestr {
            Value::Str(s) => s
                .try_borrow()
                .map(|source| (*source).clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code string"))?,
            _ => unreachable!(), // code_bytes validated the string.
        },
        code,
        req: argdesc.req,
        max: argdesc.max,
        has_rest: argdesc.has_rest,
        wna_max: 0,
        dyn_names: argdesc.dyn_names.map(Rc::new),
        dyn_rest_index: argdesc.dyn_rest_index,
        invalid_argdesc_at: argdesc.invalid_at,
    };
    run(i, &prog, bytestr, consts, &[], None, false, stack_depth)
}

fn run(
    i: &mut Interp,
    prog: &BcProg,
    source: &Value,
    consts: &Value,
    argv: &[Value],
    who: Option<&Value>,
    named: bool,
    stack_depth: usize,
) -> EvalResult {
    let Value::Vec(consts) = consts else {
        return Err(i.wrong_type_mut("vectorp", consts));
    };
    if stack_depth > MAX_BYTECODE_STACK_SLOTS {
        return Err(i.error("Bytecode stack overflow"));
    }
    let (req, max, has_rest, dyn_names, dyn_rest_index) = (
        prog.req,
        prog.max,
        prog.has_rest,
        prog.dyn_names.as_deref(),
        prog.dyn_rest_index,
    );
    // GNU `funcall_subr' passes ALL supplied args to an n-adic subr
    // (`maxargs == MANY || maxargs > 8' -> `aMANY (numargs, args)');
    // the native trampoline then reads only its declared signature,
    // silently dropping the tail — e.g. `(funcall 'cl--struct-new-class
    // <11 args>)' succeeds while `add-abbrev' (max 3) still wnas.
    // `named' marks the dumped-native-twin surface (GNU's
    // funcall_subr passes all args to an aMANY trampoline).
    let short_argv: Vec<Value>;
    let argv =
        if prog.invalid_argdesc_at.is_none() && named && !has_rest && max > 8 && argv.len() > max {
            short_argv = argv[..max].to_vec();
            &short_argv[..]
        } else {
            argv
        };
    if argv.len() < req {
        // When the caller has no function datum, GNU's compiled macro
        // expanders signal ((MIN . NONREST) NARGS). User-created byte-code
        // functions pass their function object through `who'.
        let desc = who
            .cloned()
            .unwrap_or_else(|| Value::cons(Value::Int(req as i128), Value::Int(prog.wna_max)));
        return Err(i.wrong_number_of_args(&desc, argv.len() as i128));
    }
    if prog
        .invalid_argdesc_at
        .is_some_and(|invalid_at| argv.len() >= invalid_at)
    {
        let function = who
            .cloned()
            .unwrap_or_else(|| Value::cons(Value::Int(req as i128), Value::Int(max as i128)));
        return Err(i.signal_data(sym::INVALID_FUNCTION, vec![function]));
    }
    if !has_rest && argv.len() > max {
        let desc = who
            .cloned()
            .unwrap_or_else(|| Value::cons(Value::Int(req as i128), Value::Int(max as i128)));
        return Err(i.wrong_number_of_args(&desc, argv.len() as i128));
    }
    let stack_slots = if dyn_names.is_none() {
        let Some(stack_slots) = max.checked_add(has_rest as usize) else {
            return Err(i.error("Invalid byte-code"));
        };
        if stack_slots > stack_depth {
            return Err(i.error("Invalid byte-code"));
        }
        stack_slots
    } else {
        0
    };
    let specmark = i.specbind_depth();
    // Dynamic-style argdesc (a list of names): each parameter name is
    // specbind'd to its argument — GNU binds them in order, &rest
    // takes the tail as a list.
    if let Some(names) = dyn_names {
        let mut arg_index = 0usize;
        for (index, name) in names.iter().enumerate() {
            let v = if dyn_rest_index == Some(index) {
                let rest = Value::list(argv[arg_index..].to_vec());
                arg_index = argv.len();
                rest
            } else if dyn_rest_index.is_some_and(|rest_index| index > rest_index) {
                // Names following &rest remain bound, but receive nil once
                // the rest parameter has consumed the argument tail.
                Value::Nil
            } else if arg_index < argv.len() {
                let value = argv[arg_index].clone();
                arg_index += 1;
                value
            } else {
                Value::Nil
            };
            if let Err(flow) = i.specbind(*name, v) {
                let flow = offer_exit(i, flow);
                return finish_bindings(i, specmark, Err(flow));
            }
        }
    }
    // Lexical argdesc: the args sit on the value stack (first arg
    // deepest, last on top).  Dynamic code leaves the stack empty and
    // reaches args through varref on the specbinds above.
    let stack: Vec<Value> = if dyn_names.is_none() {
        let mut s = Vec::with_capacity(stack_slots);
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
        Unwind::SaveWindowConfiguration(configuration) => {
            crate::editor::f_set_window_configuration(i, vec![configuration].into())?;
        }
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
                let latest_source = s
                    .try_borrow()
                    .map(|source| (*source).clone())
                    .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code string"))?;
                if latest_source.as_str() != source_text.as_ref() {
                    code = Cow::Owned(code_bytes(i, source)?);
                    source_text = Cow::Owned(latest_source);
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
pub(crate) fn parse_argdesc(i: &mut Interp, a: &Value) -> ParsedArgDesc {
    match a {
        Value::Int(n) => {
            let req = (*n & 0x7f).max(0) as usize;
            let rest = (*n >> 7) & 1 != 0;
            // GNU uses the packed upper field as-is. An inconsistent
            // descriptor with max < required remains uncallable rather
            // than being normalized into a valid fixed-arity descriptor.
            let packed_max = *n >> 8;
            let max = if packed_max < 0 {
                (packed_max & 0x7f) as usize
            } else {
                packed_max as usize
            };
            ParsedArgDesc {
                req,
                max,
                reported_max: packed_max,
                has_rest: rest,
                dyn_names: None,
                dyn_rest_index: None,
                invalid_at: None,
            }
        }
        Value::Nil => ParsedArgDesc {
            req: 0,
            max: 0,
            reported_max: 0,
            has_rest: false,
            dyn_names: Some(Vec::new()),
            dyn_rest_index: None,
            invalid_at: None,
        },
        Value::Cons(_) => {
            // `make-byte-code' accepts ARGLIST without validating its
            // spine. GNU treats a circular arglist as an uncallable
            // function and reports wrong-number-of-arguments for every
            // supplied count; never walk it indefinitely here.
            if argdesc_is_circular(a) {
                return ParsedArgDesc {
                    req: usize::MAX,
                    max: 0,
                    reported_max: 0,
                    has_rest: false,
                    dyn_names: Some(Vec::new()),
                    dyn_rest_index: None,
                    invalid_at: None,
                };
            }
            let opt = i.q("&optional");
            let rst = i.q("&rest");
            let mut req = 0usize;
            let mut max = 0usize;
            let mut names = Vec::new();
            let mut mode = 0u8; // 0 required, 1 optional, 2 after &rest
            let mut optional_seen = false;
            let mut rest_seen = false;
            let mut rest_index = None;
            let mut invalid_at = None;
            let mut cur = a.clone();
            while let Value::Cons(c) = cur.clone() {
                let (hd, tl) = match c.try_borrow() {
                    Ok(b) => (b.car.clone(), b.cdr.clone()),
                    Err(_) => {
                        invalid_at = Some(req);
                        break;
                    }
                };
                cur = tl;
                match i.sym_id(&hd) {
                    Some(s) if s == opt => {
                        if optional_seen || rest_seen {
                            invalid_at = Some(req);
                            break;
                        }
                        optional_seen = true;
                        mode = 1;
                    }
                    Some(s) if s == rst => {
                        if rest_seen {
                            invalid_at = Some(req);
                            break;
                        }
                        rest_seen = true;
                        mode = 2;
                    }
                    Some(s) => {
                        if mode == 2 && rest_index.is_none() {
                            rest_index = Some(names.len());
                        }
                        names.push(s);
                        match mode {
                            0 => {
                                req += 1;
                                max += 1;
                            }
                            1 => max += 1,
                            _ => {}
                        }
                    }
                    None => {
                        invalid_at = Some(req);
                        break;
                    }
                }
            }
            if invalid_at.is_none() && !cur.is_nil() {
                // A dotted arglist tail is checked after the required
                // prefix has had a chance to produce its WNA error.
                invalid_at = Some(req);
            }
            if rest_seen && rest_index.is_none() {
                invalid_at.get_or_insert(req);
            }
            ParsedArgDesc {
                req,
                max,
                reported_max: max as i128,
                has_rest: rest_index.is_some(),
                dyn_names: Some(names),
                dyn_rest_index: rest_index,
                invalid_at,
            }
        }
        _ => ParsedArgDesc {
            req: 0,
            max: 0,
            reported_max: 0,
            has_rest: false,
            dyn_names: None,
            dyn_rest_index: None,
            invalid_at: Some(0),
        },
    }
}

pub(crate) fn argdesc_is_circular(argdesc: &Value) -> bool {
    let mut current = argdesc.clone();
    let mut seen = FxHashSet::default();
    while let Value::Cons(cell) = current {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            return true;
        }
        let Ok(borrowed) = cell.try_borrow() else {
            return true;
        };
        current = borrowed.cdr.clone();
    }
    false
}

/// Extract the instruction bytes: the byte-code string is unibyte —
/// each char stands for one byte (eight-bit chars proxy via
/// `lisp_char_code' → strip the 0x3FFF00 tag back to the raw byte).
pub(crate) fn code_bytes(i: &mut Interp, v: &Value) -> Result<Vec<u8>, Flow> {
    let Value::Str(s) = v else {
        return Err(i.wrong_type_mut("stringp", v));
    };
    let s = s
        .try_borrow()
        .map_err(|_| i.error("Cannot inspect a mutably borrowed byte-code string"))?;
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

#[cfg(test)]
mod argdesc_tests {
    use crate::lisp::{Interp, Value};

    #[test]
    fn code_bytes_returns_a_lisp_error_for_a_mutably_borrowed_string() {
        let mut interp = Interp::vacant();
        interp.obarray = crate::lisp::obarray::Obarray::new();
        let bytes = Value::string("x");
        let Value::Str(string) = &bytes else {
            unreachable!();
        };
        let borrowed = string.borrow_mut();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::code_bytes(&mut interp, &bytes)
        }));
        drop(borrowed);

        assert!(matches!(
            result,
            Ok(Err(crate::lisp::error::Flow::Signal(signal)))
                if signal.is_symbol(crate::lisp::obarray::sym::ERROR)
        ));
    }

    fn with_application_stack(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn mutable_dynamic_argdesc_is_reparsed_between_calls() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(progn\
                       (defvar bytecode-argdesc-x 1)\
                       (defvar bytecode-argdesc-y 2)\
                       (let ((function\
                              (make-byte-code '(bytecode-argdesc-x)\
                                              (unibyte-string 8 135)\
                                              [bytecode-argdesc-x] 1)))\
                         (list (funcall function 7)\
                               (progn\
                                 (setcar (aref function 0) 'bytecode-argdesc-y)\
                                 (funcall function 9)))))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(7 1)");
        });
    }

    #[test]
    fn malformed_dynamic_argdescs_and_rest_positions_match_lisp_calls() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(progn\
                       (defun bytecode-argdesc-error (function arguments)\
                         (condition-case err\
                             (list 'ok (apply function arguments))\
                           (error (list (car err) (eq (cadr err) function)))))\
                       (let* ((code (unibyte-string 192 135))\
                              (constants [ok])\
                              (dotted (make-byte-code '(x . y) code constants 1))\
                              (non-symbol (make-byte-code '(1) code constants 1))\
                              (bare-rest (make-byte-code '(&rest) code constants 1))\
                              (repeated-optional\
                               (make-byte-code '(&optional x &optional y)\
                                               code constants 1))\
                              (rest-slots\
                               (make-byte-code '(&rest rx ry)\
                                               (unibyte-string 8 135) [rx] 1)))\
                         (list (bytecode-argdesc-error dotted nil)\
                               (bytecode-argdesc-error dotted '(a))\
                               (bytecode-argdesc-error dotted '(a b))\
                               (bytecode-argdesc-error non-symbol nil)\
                               (bytecode-argdesc-error bare-rest nil)\
                               (bytecode-argdesc-error repeated-optional nil)\
                               (list (funcall rest-slots)\
                                     (funcall rest-slots 'a 'b)))))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&result),
                "((wrong-number-of-arguments t) (invalid-function t) (invalid-function t) (invalid-function t) (invalid-function t) (invalid-function t) (nil (a b)))"
            );
        });
    }

    #[test]
    fn circular_dynamic_argdesc_rejects_calls_without_walking_forever() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(let* ((read-circle t)\
                            (arglist (read \"#1=(x . #1#)\"))\
                            (function (make-byte-code arglist\
                                                     (string-as-unibyte \"\") [] 0)))\
                       (mapcar (lambda (arguments)\
                                 (condition-case err\
                                     (apply function arguments)\
                                   (wrong-number-of-arguments\
                                    (eq (cadr err) function))))\
                               '(nil (1) (1 2))))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(t t t)");
        });
    }

    #[test]
    fn make_byte_code_rejects_nonfixnum_nonlist_argdescs() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(mapcar\
                       (lambda (argdesc)\
                         (condition-case error\
                             (progn\
                               (make-byte-code argdesc (unibyte-string 192 135) [ok] 1)\
                               'accepted)\
                           (error (list (car error) (cadr error)))))\
                       (list 'bad \"bad\" [bad] 2305843009213693952))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&result),
                "((error \"Invalid byte-code object\") (error \"Invalid byte-code object\") (error \"Invalid byte-code object\") (error \"Invalid byte-code object\"))"
            );
        });
    }

    #[test]
    fn mutated_nonlist_argdesc_is_rejected_before_bytecode_decode() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let function = interp
                .eval_str("(make-byte-code 0 (unibyte-string 51) [] 0)")
                .unwrap();
            let variable = interp.intern("bytecode-mutated-nonlist-argdesc");
            interp.set_symbol(variable, function.clone()).unwrap();
            let malformed = Value::Sym(interp.intern("not-an-arglist"));
            let Value::Lambda(lambda) = &function else {
                panic!("make-byte-code did not return a byte-code function");
            };
            lambda
                .bc_items
                .as_ref()
                .expect("byte-code slots")
                .borrow_mut()[0] = malformed;

            let result = interp
                .eval_str(
                    "(let ((bytecode-mutated-arg-evaluated nil))\
                       (list\
                         (condition-case error\
                             (funcall bytecode-mutated-nonlist-argdesc)\
                           (invalid-function\
                            (eq (cadr error) bytecode-mutated-nonlist-argdesc)))\
                         (condition-case error\
                             (funcall bytecode-mutated-nonlist-argdesc\
                                      (setq bytecode-mutated-arg-evaluated t))\
                           (invalid-function\
                            (eq (cadr error) bytecode-mutated-nonlist-argdesc)))\
                         bytecode-mutated-arg-evaluated\
                         (condition-case error\
                             (func-arity bytecode-mutated-nonlist-argdesc)\
                           (invalid-function\
                            (eq (cadr error) bytecode-mutated-nonlist-argdesc)))))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(t t t t)");
        });
    }

    #[test]
    fn compiled_dynamic_argdesc_checks_after_argument_evaluation() {
        with_application_stack(|| {
            let mut interp = Interp::new();
            let required = interp.intern("bytecode-dynamic-required");
            let malformed_tail = interp.intern("bytecode-dynamic-malformed-tail");
            let params = Value::list(vec![Value::Sym(required)]);
            let mut lambda = interp
                .parse_lambda(&params, &[Value::Sym(required)], None)
                .unwrap();
            lambda.compiled = true;
            let byte_code = interp.eval_str("(unibyte-string 192 135)").unwrap();
            let constants = interp.eval_str("[ok]").unwrap();
            lambda.bc_items = Some(std::rc::Rc::new(std::cell::RefCell::new(vec![
                Value::cons(Value::Sym(required), Value::Sym(malformed_tail)),
                byte_code,
                constants,
                Value::Int(1),
            ])));
            let function = Value::Lambda(std::rc::Rc::new(lambda));
            let function_name = interp.intern("bytecode-compiled-dynamic-argdesc");
            interp.fset(function_name, function);

            let result = interp
                .eval_str(
                    "(let ((bytecode-dynamic-arg-evaluated nil))\
                       (list\
                         (condition-case error\
                             (bytecode-compiled-dynamic-argdesc\
                              (setq bytecode-dynamic-arg-evaluated t))\
                           (invalid-function 'invalid))\
                         bytecode-dynamic-arg-evaluated))",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&result), "(invalid t)");
        });
    }

    #[test]
    fn byte_code_execution_rejects_slots_borrowed_by_its_rust_caller() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let function = interp
                    .eval_str("(make-byte-code 0 (unibyte-string 192 135) [ok] 1)")
                    .unwrap();
                let Value::Lambda(lambda) = function else {
                    unreachable!();
                };
                let mut lambda = std::rc::Rc::try_unwrap(lambda)
                    .unwrap_or_else(|_| panic!("byte-code lambda unexpectedly shared"));
                lambda.compiled = true;
                let lambda = std::rc::Rc::new(lambda);
                let function = Value::Lambda(lambda.clone()).tracked();
                let items = lambda.bc_items.as_ref().expect("byte-code slots").clone();
                let function_name = interp.intern("qa-borrowed-bytecode-call");
                interp.fset(function_name, function.clone());
                let held_borrow = items.borrow_mut();

                let apply_result = interp.apply(&function, Vec::<Value>::new());
                let fast_result = interp.apply_bytecode(&function, Vec::<Value>::new());
                let eval_result = interp.eval_str("(qa-borrowed-bytecode-call)");

                drop(held_borrow);
                assert!(matches!(
                    apply_result,
                    Err(crate::lisp::Flow::Signal(ref signal))
                        if signal.is_symbol(crate::lisp::obarray::sym::ERROR)
                ));
                assert!(matches!(
                    fast_result,
                    Err(crate::lisp::Flow::Signal(ref signal))
                        if signal.is_symbol(crate::lisp::obarray::sym::ERROR)
                ));
                assert!(matches!(
                    eval_result,
                    Err(crate::lisp::Flow::Signal(ref signal))
                        if signal.is_symbol(crate::lisp::obarray::sym::ERROR)
                ));
                let result = interp.apply(&function, Vec::<Value>::new()).unwrap();
                assert_eq!(interp.prin1_to_string(&result), "ok");
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
