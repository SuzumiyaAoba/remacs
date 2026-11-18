use remacs_runtime::lisp::{Flow, Interp, Value};

#[test]
fn cl_equalp_reports_borrowed_values_and_preserves_reverse_vector_order() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.eval_str("(require 'cl-extra)").unwrap();
            let equalp_id = interp.intern("cl-equalp");
            let equalp = interp.symbol_function(equalp_id);

            let left = Value::list(vec![Value::Int(1)]);
            let right = Value::list(vec![Value::Int(1)]);
            let Value::Cons(cell) = &left else {
                unreachable!();
            };
            let held_cons = cell.borrow_mut();
            let result = interp.apply(&equalp, vec![left.clone(), right.clone()]);
            let signal_name = match &result {
                Err(Flow::Signal(signal)) => {
                    interp.prin1_to_string(&signal.symbol()).to_unicode_lossy()
                }
                _ => "<no signal>".into(),
            };
            assert!(matches!(
                &result,
                Err(Flow::Signal(signal)) if signal.is_symbol(remacs_runtime::lisp::obarray::sym::ERROR)
            ), "unexpected equalp result: {result:?}, signal={signal_name}");
            drop(held_cons);
            assert!(interp
                .apply(&equalp, vec![left, right])
                .unwrap()
                .truthy());

            let left = Value::vector(vec![Value::Int(1)]);
            let right = Value::vector(vec![Value::Int(1)]);
            let Value::Vec(cell) = &left else {
                unreachable!();
            };
            let held_vector = cell.borrow_mut();
            let result = interp.apply(&equalp, vec![left.clone(), right.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(remacs_runtime::lisp::obarray::sym::ERROR)
            ));
            drop(held_vector);
            assert!(interp
                .apply(&equalp, vec![left, right])
                .unwrap()
                .truthy());

            let left_cons = Value::list(vec![Value::Int(1)]);
            let right_cons = Value::list(vec![Value::Int(1)]);
            let Value::Cons(cell) = &left_cons else {
                unreachable!();
            };
            let held_cons = cell.borrow_mut();
            let left = Value::vector(vec![left_cons.clone(), Value::Int(1)]);
            let right = Value::vector(vec![right_cons, Value::Int(2)]);
            let result = interp.apply(&equalp, vec![left, right]);
            drop(held_cons);
            assert!(matches!(result, Ok(Value::Nil)));
        })
        .unwrap()
        .join()
        .unwrap();
}
