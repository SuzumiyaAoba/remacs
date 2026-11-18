//! GNU opcode execution. Preserve the validation order within each case;
//! the parent owns instruction refresh, signal delivery and handler search.

use super::{BcHandler, BcHandlerKind, Frame, Step, Unwind, pop_handler_scope, unwind_one};
use crate::lisp::{
    builtins::{eq_values, equal_values},
    error::Flow,
    eval::Interp,
    obarray::sym,
    value::Value,
};
use std::cell::RefCell;

fn lisp_call(i: &mut Interp, name: &str, arguments: Vec<Value>) -> Value {
    Value::cons(Value::Sym(i.q(name)), Value::list(arguments))
}

fn lisp_quote(i: &mut Interp, value: Value) -> Value {
    Value::list(vec![Value::Sym(i.q("quote")), value])
}

/// Set up the output buffer used by legacy bytecode opcodes 144/145.
fn temp_output_buffer_setup(i: &mut Interp, name: &Value) -> Result<Value, Flow> {
    if !matches!(name, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", name));
    }

    let old_directory = Value::Sym(i.make_symbol("remacs-bytecode-temp-old-directory"));
    let output_buffer = Value::Sym(i.make_symbol("remacs-bytecode-temp-output-buffer"));
    let buffer_in_body = Value::Sym(i.make_symbol("remacs-bytecode-temp-current-buffer"));
    let old_directory_binding = Value::list(vec![
        old_directory.clone(),
        Value::Sym(i.q("default-directory")),
    ]);
    let buffer_binding = Value::list(vec![
        buffer_in_body.clone(),
        lisp_call(i, "current-buffer", Vec::new()),
    ]);
    let kill_locals = lisp_call(i, "kill-all-local-variables", Vec::new());
    let delete_overlays = lisp_call(i, "delete-all-overlays", Vec::new());
    let enable_multibyte = Value::Sym(i.q("enable-multibyte-characters"));
    let quoted_enable_multibyte = lisp_quote(i, enable_multibyte.clone());
    let default_multibyte = lisp_call(i, "default-value", vec![quoted_enable_multibyte]);
    let assignments = Value::list(vec![
        Value::Sym(i.q("setq")),
        Value::Sym(i.q("default-directory")),
        old_directory.clone(),
        Value::Sym(i.q("buffer-read-only")),
        Value::Nil,
        Value::Sym(i.q("buffer-file-name")),
        Value::Nil,
        Value::Sym(i.q("buffer-undo-list")),
        Value::t(),
        enable_multibyte,
        default_multibyte,
    ]);
    let inhibit_bindings = Value::list(vec![
        Value::list(vec![Value::Sym(i.q("inhibit-read-only")), Value::t()]),
        Value::list(vec![
            Value::Sym(i.q("inhibit-modification-hooks")),
            Value::t(),
        ]),
    ]);
    let erase = lisp_call(i, "erase-buffer", Vec::new());
    let setup_hook = Value::Sym(i.q("temp-buffer-setup-hook"));
    let quoted_setup_hook = lisp_quote(i, setup_hook);
    let run_setup_hook = lisp_call(i, "run-hooks", vec![quoted_setup_hook]);
    let inhibit_modifications = Value::list(vec![
        Value::Sym(i.q("let")),
        inhibit_bindings,
        erase,
        run_setup_hook,
    ]);
    let setup_body = Value::list(vec![
        Value::Sym(i.q("let")),
        Value::list(vec![buffer_binding]),
        kill_locals,
        delete_overlays,
        assignments,
        inhibit_modifications,
        buffer_in_body.clone(),
    ]);
    let get_buffer = lisp_call(i, "get-buffer-create", vec![name.clone()]);
    let with_current_buffer = Value::list(vec![
        Value::Sym(i.q("with-current-buffer")),
        get_buffer,
        setup_body,
    ]);
    let output_binding = Value::list(vec![output_buffer.clone(), with_current_buffer]);
    let setup = Value::list(vec![
        Value::Sym(i.q("let*")),
        Value::list(vec![old_directory_binding, output_binding]),
        output_buffer,
    ]);
    i.eval(&setup)
}

/// Show a legacy bytecode output buffer using GNU's temp-buffer hooks.
fn temp_output_buffer_show(i: &mut Interp, buffer: &Value) -> Result<Value, Flow> {
    let old_directory = Value::Sym(i.make_symbol("remacs-bytecode-temp-show-directory"));
    let output_buffer = Value::Sym(i.make_symbol("remacs-bytecode-temp-show-buffer"));
    let window = Value::Sym(i.make_symbol("remacs-bytecode-temp-show-window"));
    let frame = Value::Sym(i.make_symbol("remacs-bytecode-temp-show-frame"));
    let old_directory_binding = Value::list(vec![
        old_directory.clone(),
        Value::Sym(i.q("default-directory")),
    ]);
    let output_binding = Value::list(vec![output_buffer.clone(), buffer.clone()]);
    let set_directory = Value::list(vec![
        Value::Sym(i.q("setq")),
        Value::Sym(i.q("default-directory")),
        old_directory,
    ]);
    let unmodified = lisp_call(i, "set-buffer-modified-p", vec![Value::Nil]);
    // The legacy C opcode resets BEGV/ZV to the full buffer before showing.
    let widen = lisp_call(i, "widen", Vec::new());
    let point_min = lisp_call(i, "point-min", Vec::new());
    let goto_start = lisp_call(i, "goto-char", vec![point_min]);
    let prepare_buffer = Value::list(vec![
        Value::Sym(i.q("with-current-buffer")),
        output_buffer.clone(),
        set_directory,
        unmodified,
        widen,
        goto_start,
    ]);

    let display = lisp_call(i, "display-buffer", vec![output_buffer.clone()]);
    let window_binding = Value::list(vec![window.clone(), display]);
    let frame_expression = Value::list(vec![
        Value::Sym(i.q("and")),
        window.clone(),
        lisp_call(i, "window-frame", vec![window.clone()]),
    ]);
    let frame_binding = Value::list(vec![frame.clone(), frame_expression]);
    let selected_frame = lisp_call(i, "selected-frame", Vec::new());
    let frame_differs = lisp_call(i, "eq", vec![frame.clone(), selected_frame]);
    let make_visible = Value::list(vec![
        Value::Sym(i.q("unless")),
        frame_differs,
        lisp_call(i, "make-frame-visible", vec![frame]),
    ]);
    let set_minibuffer_scroll_window = Value::list(vec![
        Value::Sym(i.q("setq")),
        Value::Sym(i.q("minibuf-scroll-window")),
        window.clone(),
    ]);
    let point_min_for_start = Value::list(vec![
        Value::Sym(i.q("with-current-buffer")),
        output_buffer.clone(),
        lisp_call(i, "point-min", Vec::new()),
    ]);
    let set_start = lisp_call(
        i,
        "set-window-start",
        vec![window.clone(), point_min_for_start, Value::t()],
    );
    let point_min_for_point = Value::list(vec![
        Value::Sym(i.q("with-current-buffer")),
        output_buffer.clone(),
        lisp_call(i, "point-min", Vec::new()),
    ]);
    let reset_window_state = lisp_call(
        i,
        "remacs--temp-output-buffer-reset-window-state",
        vec![window.clone(), point_min_for_point.clone()],
    );
    let set_point = lisp_call(
        i,
        "set-window-point",
        vec![window.clone(), point_min_for_point],
    );
    let window_buffer = lisp_call(i, "window-buffer", vec![window.clone()]);
    let show_hook = Value::Sym(i.q("temp-buffer-show-hook"));
    let quoted_show_hook = lisp_quote(i, show_hook);
    let run_show_hook = lisp_call(i, "run-hooks", vec![quoted_show_hook]);
    let with_window_buffer = Value::list(vec![
        Value::Sym(i.q("with-current-buffer")),
        window_buffer,
        run_show_hook,
    ]);
    let with_selected = Value::list(vec![
        Value::Sym(i.q("with-selected-window")),
        window.clone(),
        with_window_buffer,
    ]);
    let show_window = Value::list(vec![
        Value::Sym(i.q("when")),
        lisp_call(i, "window-live-p", vec![window.clone()]),
        Value::list(vec![
            Value::Sym(i.q("let*")),
            Value::list(vec![frame_binding]),
            make_visible,
            set_minibuffer_scroll_window,
            set_start,
            set_point,
            reset_window_state,
            with_selected,
        ]),
    ]);
    let show_window = Value::list(vec![
        Value::Sym(i.q("let*")),
        Value::list(vec![window_binding]),
        show_window,
    ]);
    let show_function = Value::Sym(i.q("temp-buffer-show-function"));
    let show = Value::list(vec![
        Value::Sym(i.q("if")),
        show_function.clone(),
        lisp_call(i, "funcall", vec![show_function, output_buffer.clone()]),
        show_window,
    ]);
    let form = Value::list(vec![
        Value::Sym(i.q("let")),
        Value::list(vec![old_directory_binding, output_binding]),
        prepare_buffer,
        show,
        Value::Nil,
    ]);
    i.eval(&form)
}

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
    macro_rules! invalid_bytecode {
        () => {
            return Step::Flow(i.error("Invalid byte-code"))
        };
    }
    macro_rules! pop {
        () => {
            match stack.pop() {
                Some(value) => value,
                None => invalid_bytecode!(),
            }
        };
    }
    macro_rules! tos {
        () => {
            match stack.last() {
                Some(value) => value.clone(),
                None => invalid_bytecode!(),
            }
        };
    }
    macro_rules! fetch {
        () => {{
            let Some(b) = code.get(*pc).copied() else {
                invalid_bytecode!();
            };
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
            let value = match consts.try_borrow() {
                Ok(constants) => constants.get($n).cloned(),
                Err(_) => {
                    return Step::Flow(
                        i.error("Cannot inspect a mutably borrowed byte-code constants vector"),
                    );
                }
            };
            match value {
                Some(value) => value,
                None => return Step::Flow(i.error("Invalid byte-code")),
            }
        }};
    }
    macro_rules! csym {
        ($n:expr) => {{
            let constant = konst!($n);
            match i.sym_id(&constant) {
                Some(symbol) => symbol,
                None => fail!(i.wrong_type_mut("symbolp", &constant)),
            }
        }};
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
            if stack.len() < n {
                invalid_bytecode!();
            }
            let fpos = stack.len() - n;
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
            let Some(idx) = stack.len().checked_sub(n + 1) else {
                invalid_bytecode!();
            };
            let Some(v) = stack.get(idx).cloned() else {
                invalid_bytecode!();
            };
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
            let Some(fpos) = stack.len().checked_sub(n + 1) else {
                invalid_bytecode!();
            };
            let Some(fun) = stack.get(fpos).cloned() else {
                invalid_bytecode!();
            };
            if i.dbg_flags & 0x01 != 0 {
                let bad = if let Value::Cons(c) = &fun {
                    match c.try_borrow() {
                        Ok(cell) => {
                            let lam = i.q("lambda");
                            let mac = i.q("macro");
                            !(i.sym_is(&cell.car, lam) || i.sym_is(&cell.car, mac))
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                };
                if bad {
                    let st: Vec<crate::lisp::text::EmacsText> = stack
                        .iter()
                        .map(|v| i.princ_to_string(v).chars().take(60).collect())
                        .collect();
                    let cs: Vec<crate::lisp::text::EmacsText> = consts
                        .try_borrow()
                        .map(|constants| {
                            constants
                                .iter()
                                .map(|v| i.princ_to_string(v).chars().take(60).collect())
                                .collect()
                        })
                        .unwrap_or_default();
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
            if unwinds.len() < n {
                invalid_bytecode!();
            }
            r!(i.with_quit_preserved(|i| {
                for _ in 0..n {
                    unwind_one(i, unwinds.pop().unwrap())?;
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
                Value::Cons(c) => match c.try_borrow() {
                    Ok(cell) => cell.car.clone(),
                    Err(_) => fail!(i.error("Cannot read a mutably borrowed cons cell")),
                },
                Value::Nil => Value::Nil,
                _ => fail!(i.wrong_type_mut("listp", &v)),
            };
            push!(r);
        }
        65 => {
            let v = pop!();
            let r = match &v {
                Value::Cons(c) => match c.try_borrow() {
                    Ok(cell) => cell.cdr.clone(),
                    Err(_) => fail!(i.error("Cannot read a mutably borrowed cons cell")),
                },
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
            // Legacy bytecode opcode: the TOS is a progn body. GNU records
            // the current window configuration, evaluates that body now,
            // then restores the configuration before returning its value.
            let body = tos!();
            let configuration = r!(crate::editor::f_current_window_configuration(
                i,
                Vec::<Value>::new().into(),
            ));
            unwinds.push(Unwind::SaveWindowConfiguration(configuration));
            match i.eval_progn(&body) {
                Ok(value) => {
                    let Some(unwind) = unwinds.pop() else {
                        invalid_bytecode!();
                    };
                    r!(unwind_one(i, unwind));
                    let Some(top) = stack.last_mut() else {
                        invalid_bytecode!();
                    };
                    *top = value;
                }
                Err(flow) => fail!(flow),
            }
        }
        140 => {
            let s = i.save_restriction_state();
            unwinds.push(Unwind::SaveRestriction(s));
        }
        // Legacy byte-catch: the stack holds TAG below one form to eval.
        141 => {
            let body = pop!();
            let tag = tos!();
            let result = if tag.is_nil() {
                // As with Lisp `catch', nil does not catch `(throw nil ...)'.
                i.eval(&body)
            } else {
                i.catch_tags.push(tag.clone());
                let result = i.eval(&body);
                i.catch_tags.pop();
                match result {
                    Err(Flow::Throw(thrown_tag, value)) if eq_values(&thrown_tag, &tag) => {
                        i.finish_quit_unwind();
                        Ok(value)
                    }
                    Err(flow @ Flow::Throw(_, _)) => {
                        i.rebase_quit_unwind();
                        Err(flow)
                    }
                    Err(flow) => {
                        i.rebase_quit_unwind();
                        Err(flow)
                    }
                    value => value,
                }
            };
            match result {
                Ok(value) => {
                    let Some(top) = stack.last_mut() else {
                        invalid_bytecode!();
                    };
                    *top = value;
                }
                Err(flow) => fail!(flow),
            }
        }
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
        // Legacy byte-condition-case leaves VAR on the stack, with BODY and
        // HANDLERS above it. Reuse the ordinary special-form path so its
        // handler validation and unwind behavior stay in sync.
        143 => {
            let handlers = pop!();
            let body = pop!();
            let variable = tos!();
            let form = Value::cons(
                Value::Sym(sym::CONDITION_CASE),
                Value::cons(variable, Value::cons(body, handlers)),
            );
            let value = r!(i.eval(&form));
            let Some(top) = stack.last_mut() else {
                invalid_bytecode!();
            };
            *top = value;
        }
        144 => {
            let name = tos!();
            let buffer = r!(temp_output_buffer_setup(i, &name));
            let output = i.q("standard-output");
            let depth = i.specbind_depth();
            let result = i.specbind(output, buffer.clone());
            if i.specbind_depth() > depth {
                unwinds.push(Unwind::VarBind);
            }
            r!(result);
            let Some(top) = stack.last_mut() else {
                invalid_bytecode!();
            };
            *top = buffer;
        }
        145 => {
            let value = pop!();
            let buffer = tos!();
            let _ = r!(temp_output_buffer_show(i, &buffer));
            let Some(unwind) = unwinds.pop() else {
                invalid_bytecode!();
            };
            if !matches!(&unwind, Unwind::VarBind) {
                invalid_bytecode!();
            }
            r!(unwind_one(i, unwind));
            let Some(top) = stack.last_mut() else {
                invalid_bytecode!();
            };
            *top = value;
        }
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
                Value::Cons(c) => match c.try_borrow() {
                    Ok(cell) => cell.car.clone(),
                    Err(_) => fail!(i.error("Cannot read a mutably borrowed cons cell")),
                },
                _ => Value::Nil,
            });
        }
        163 => {
            let v = pop!();
            push!(match &v {
                Value::Cons(c) => match c.try_borrow() {
                    Ok(cell) => cell.cdr.clone(),
                    Err(_) => fail!(i.error("Cannot read a mutably borrowed cons cell")),
                },
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
            if stack.len() < n {
                invalid_bytecode!();
            }
            let items: Vec<Value> = stack.split_off(stack.len() - n);
            push!(Value::list(items));
        }
        176 => {
            let n = fetch!() as usize;
            if stack.len() < n {
                invalid_bytecode!();
            }
            let fpos = stack.len() - n;
            let argv: Vec<Value> = stack.split_off(fpos);
            let v = r!(i.bytecode_subr("concat", argv));
            push!(v);
        }
        177 => {
            let n = fetch!() as usize;
            if stack.len() < n {
                invalid_bytecode!();
            }
            let fpos = stack.len() - n;
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
            let Some(idx) = stack.len().checked_sub(k + 1) else {
                invalid_bytecode!();
            };
            let v = tos!();
            stack[idx] = v;
            pop!();
        }
        182 => {
            // discardN: operand byte; 0x80 flag means preserve TOS
            // (remove N elements underneath it).
            let n = fetch!() as usize;
            if n & 0x80 != 0 {
                let n = n & 0x7f;
                if stack.len() < n + 1 {
                    invalid_bytecode!();
                }
                let t = tos!();
                let keep = stack.len() - n - 1;
                stack.truncate(keep);
                push!(t);
            } else {
                if stack.len() < n {
                    invalid_bytecode!();
                }
                let keep = stack.len() - n;
                stack.truncate(keep);
            }
        }
        183 => {
            // switch: the jump table (hash table value → pc offset)
            // sits on TOS with the dispatch value below it; jump when
            // the lookup hits. A miss falls through to the separately
            // emitted default goto; every stored entry must be an offset.
            let table = pop!();
            let key = pop!();
            if let Some(v) = r!(crate::lisp::builtins::hashfn::gethash_entry(i, &key, &table)) {
                if let Some(n) = v.int() {
                    let Ok(destination) = usize::try_from(n) else {
                        fail!(i.error(format!("Invalid byte opcode: op=0, ptr={n}")));
                    };
                    // `usize::MAX` cannot be a byte offset for any allocated
                    // program. Letting it reach dispatch would overflow the
                    // instruction pointer while decoding the synthetic EOF
                    // opcode.
                    if destination == usize::MAX {
                        fail!(i.error(format!("Invalid byte opcode: op=0, ptr={n}")));
                    }
                    branch!(destination);
                } else {
                    invalid_bytecode!();
                }
            }
        }
        192..=255 => push!(konst!(op - 192)),
        _ => fail!(i.error(format!("Invalid byte opcode: op={op}, ptr={}", *pc - 1))),
    }
    Step::Next
}

#[cfg(test)]
mod temp_output_buffer_tests {
    use super::*;

    #[test]
    fn legacy_show_resets_horizontal_scroll_state() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq temp-buffer-setup-hook nil \
                               temp-buffer-show-function nil \
                               temp-buffer-show-hook nil) \
                         (fset 'display-buffer \
                           (lambda (&rest _args) (selected-window)))",
                    )
                    .unwrap();
                let window = interp
                    .selected_frame
                    .as_ref()
                    .expect("selected frame")
                    .borrow()
                    .selected
                    .clone();
                let buffer = temp_output_buffer_setup(
                    &mut interp,
                    &Value::string("*legacy-output-scroll-state*"),
                )
                .unwrap();
                interp
                    .eval_str(
                        "(with-current-buffer \"*legacy-output-scroll-state*\" \
                           (insert \"abc\"))",
                    )
                    .unwrap();
                let Value::Buffer(buffer_ref) = &buffer else {
                    panic!("expected output buffer");
                };
                {
                    let mut state = window.borrow_mut();
                    state.buffer = buffer_ref.borrow().id;
                    state.hscroll = 7;
                    state.min_hscroll = 7;
                    state.suspend_hscroll = true;
                    crate::editor::set_window_old_point(&interp, &mut state, 2);
                }
                temp_output_buffer_show(&mut interp, &buffer).unwrap();

                let state = window.borrow();
                assert_eq!(state.hscroll, 0);
                assert_eq!(state.min_hscroll, 0);
                assert!(!state.suspend_hscroll);
                assert_eq!(state.old_point_position(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn legacy_show_calls_display_buffer_in_the_caller_context() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-legacy-display-buffer-current nil) \
                         (setq temp-buffer-setup-hook nil \
                               temp-buffer-show-function nil \
                               temp-buffer-show-hook nil) \
                         (fset 'display-buffer \
                           (lambda (&rest _args) \
                             (setq qa-legacy-display-buffer-current \
                                   (current-buffer)) \
                             (selected-window)))",
                    )
                    .unwrap();
                let caller = interp.current_buffer;
                let buffer = temp_output_buffer_setup(
                    &mut interp,
                    &Value::string("*legacy-output-current-context*"),
                )
                .unwrap();
                temp_output_buffer_show(&mut interp, &buffer).unwrap();

                let observed_id = interp.q("qa-legacy-display-buffer-current");
                let observed = interp.symbol_value(observed_id);
                let caller =
                    Value::Buffer(interp.buffers.get(caller).expect("caller buffer").clone());
                assert!(crate::lisp::builtins::eq_values(&observed, &caller));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn legacy_show_ignores_a_non_live_display_result() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq temp-buffer-setup-hook nil \
                               temp-buffer-show-function nil \
                               temp-buffer-show-hook nil) \
                         (fset 'display-buffer (lambda (&rest _args) t))",
                    )
                    .unwrap();
                let buffer = temp_output_buffer_setup(
                    &mut interp,
                    &Value::string("*legacy-output-dead-window*"),
                )
                .unwrap();
                assert!(
                    temp_output_buffer_show(&mut interp, &buffer)
                        .unwrap()
                        .is_nil()
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::error::Flow;
    use crate::lisp::obarray::Obarray;

    fn assert_invalid_bytecode(opcode: usize, code: &[u8]) {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let constants = RefCell::new(Vec::new());
        let mut frame = Frame {
            handlers: Vec::new(),
            unwinds: Vec::new(),
            stack: Vec::new(),
            pc: 1,
        };
        let quitcounter = std::cell::Cell::new(1);
        let Step::Flow(Flow::Signal(signal)) = step_op(
            &mut interp,
            code,
            &constants,
            &mut frame,
            opcode,
            &quitcounter,
        ) else {
            panic!("opcode {opcode} accepted a stack underflow");
        };
        assert!(signal.is_symbol(sym::ERROR));
        assert_eq!(
            interp.prin1_to_string(&signal.data()),
            "(\"Invalid byte-code\")"
        );
    }

    #[test]
    fn variable_opcodes_reject_non_symbol_constants() {
        for (opcode, stack) in [
            (8usize, Vec::new()),
            (16, vec![Value::Int(99)]),
            (24, vec![Value::Int(99)]),
        ] {
            let mut interp = Interp::vacant();
            interp.obarray = Obarray::new();
            let constants = RefCell::new(vec![Value::Int(42)]);
            let code = [opcode as u8];
            let mut frame = Frame {
                handlers: Vec::new(),
                unwinds: Vec::new(),
                stack,
                pc: 1,
            };
            let quitcounter = std::cell::Cell::new(1);

            let Step::Flow(Flow::Signal(signal)) = step_op(
                &mut interp,
                &code,
                &constants,
                &mut frame,
                opcode,
                &quitcounter,
            ) else {
                panic!("opcode {opcode} accepted a non-symbol variable constant");
            };
            assert!(signal.is_symbol(sym::WRONG_TYPE_ARGUMENT));
            assert_eq!(interp.prin1_to_string(&signal.data()), "(symbolp 42)");
        }
    }

    #[test]
    fn stack_underflow_does_not_synthesize_nil_operands() {
        assert_invalid_bytecode(1, &[1]); // stack-ref
        assert_invalid_bytecode(32, &[32]); // call
        assert_invalid_bytecode(56, &[56]); // named subr call
        assert_invalid_bytecode(41, &[41]); // unbind one missing unwind record
        assert_invalid_bytecode(136, &[136]); // discard
        assert_invalid_bytecode(133, &[133, 0, 0]); // jump-if-nil
        assert_invalid_bytecode(134, &[134, 0, 0]); // jump-if-not-nil
        assert_invalid_bytecode(135, &[135]); // return
        assert_invalid_bytecode(137, &[137]); // duplicate TOS
        assert_invalid_bytecode(175, &[175, 1]); // listN
        assert_invalid_bytecode(176, &[176, 1]); // concatN
        assert_invalid_bytecode(177, &[177, 1]); // insertN
        assert_invalid_bytecode(178, &[178, 0]); // stack-set
        assert_invalid_bytecode(179, &[179, 1, 0]); // stack-set2
        assert_invalid_bytecode(182, &[182, 1]); // discardN
    }

    #[test]
    fn truncated_instruction_operands_are_rejected() {
        assert_invalid_bytecode(129, &[129, 0]); // constant2 missing high byte
        assert_invalid_bytecode(130, &[130]); // goto missing both destination bytes
        assert_invalid_bytecode(175, &[175]); // listN missing count
    }

    #[test]
    fn save_window_excursion_opcode_evaluates_body_and_restores_on_success_or_error() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        r#"(let* ((before (length (window-list)))
                                 (selected (selected-window))
                                 (normal
                                  (make-byte-code
                                   0 (unibyte-string 192 139 135)
                                   (vector '((split-window) 'answer)) 1))
                                 (failing
                                  (make-byte-code
                                   0 (unibyte-string 192 139 135)
                                   (vector '((split-window) (error "failure"))) 1))
                                 (throwing
                                  (make-byte-code
                                   0 (unibyte-string 192 139 135)
                                   (vector '((split-window)
                                             (throw 'window-excursion-tag 'escaped))) 1)))
                            (list (funcall normal)
                                  (= before (length (window-list)))
                                  (eq selected (selected-window))
                                  (condition-case nil
                                      (funcall failing)
                                    (error 'caught))
                                  (= before (length (window-list)))
                                  (eq selected (selected-window))
                                  (catch 'window-excursion-tag
                                    (funcall throwing))
                                  (= before (length (window-list)))
                                  (eq selected (selected-window))))"#,
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&result),
                    "(answer t t caught t t escaped t t)"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
