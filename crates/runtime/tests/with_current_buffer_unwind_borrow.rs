use remacs_runtime::lisp::{Argv, Arity, EvalResult, Interp, Subr, Value, sym};

static HOLD_CURRENT_BUFFER_ARGS: Subr = Subr {
    name: "qa-hold-with-current-buffer-args",
    arity: Arity::Range { min: 0, max: 0 },
    func: hold_current_buffer_args,
    doc: "",
};

fn unexpected_with_current_buffer_call(
    interp: &mut Interp,
    _args: Argv,
) -> EvalResult {
    Err(interp.error("with-current-buffer special dispatch was bypassed"))
}

static WITH_CURRENT_BUFFER_SPECIAL: Subr = Subr {
    name: "with-current-buffer",
    arity: Arity::Unevalled,
    func: unexpected_with_current_buffer_call,
    doc: "",
};

fn hold_current_buffer_args(interp: &mut Interp, _args: Argv) -> EvalResult {
    let args_symbol = interp.intern("qa-with-current-buffer-args");
    if let Value::Cons(args) = interp.symbol_value(args_symbol) {
        // Keep the special form's argument spine borrowed until its body
        // lookup, which must still restore the previously selected buffer.
        std::mem::forget(args.borrow_mut());
    }
    let target_symbol = interp.intern("qa-with-current-buffer-target");
    Ok(interp.symbol_value(target_symbol))
}

#[test]
fn with_current_buffer_restores_selection_after_borrowed_body_spine_error() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let original = interp.current_buffer_ref().expect("current buffer");
            let original_id = original.borrow().id;
            let target_id = interp.create_buffer("*qa-with-current-buffer-target*");
            let target = interp.buffer_value(target_id).expect("target buffer");
            let args_symbol = interp.intern("qa-with-current-buffer-args");
            let target_symbol = interp.intern("qa-with-current-buffer-target");
            let hold = interp.intern("qa-hold-with-current-buffer-args");
            interp.fset(hold, Value::Subr(&HOLD_CURRENT_BUFFER_ARGS));
            // Make the evaluator dispatch to its Rust special-form path;
            // the normal loaded Lisp macro copies the body spine first.
            interp.fset(sym::WITH_CURRENT_BUFFER, Value::Subr(&WITH_CURRENT_BUFFER_SPECIAL));
            interp.set_symbol(target_symbol, target).unwrap();

            let args = Value::list(vec![Value::list(vec![Value::Sym(hold)]), Value::Int(42)]);
            let Value::Cons(root) = &args else {
                panic!("call argument list must be a cons");
            };
            interp
                .set_symbol(args_symbol, Value::Cons(root.clone()))
                .unwrap();
            let form = Value::cons(Value::Sym(sym::WITH_CURRENT_BUFFER), args);

            assert!(interp.eval(&form).is_err());
            let current = interp.current_buffer_ref().expect("current buffer");
            assert_eq!(current.borrow().id, original_id);
        })
        .unwrap()
        .join()
        .unwrap();
}
