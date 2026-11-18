use remacs_runtime::lisp::{Flow, Interp, Value};
use remacs_runtime::lisp::obarray::sym;

#[test]
fn closure_constructors_report_mutably_borrowed_inputs_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let make_interpreted_closure = Value::Sym(interp.intern("make-interpreted-closure"));
            let body = Value::list(vec![Value::Sym(sym::T)]);
            let doc = interp.eval_str("\"borrowed doc\"").unwrap();
            let Value::Str(doc_text) = &doc else {
                unreachable!();
            };
            let held_doc = doc_text.borrow_mut();
            let result = interp.apply(
                &make_interpreted_closure,
                vec![Value::Nil, body.clone(), Value::Nil, doc.clone()],
            );
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_doc);
            assert!(matches!(
                interp.apply(
                    &make_interpreted_closure,
                    vec![Value::Nil, body, Value::Nil, doc]
                ),
                Ok(Value::Lambda(_))
            ));

            let make_byte_code = Value::Sym(interp.intern("make-byte-code"));
            let code = interp.eval_str("(unibyte-string 192 135)").unwrap();
            let constants = interp.eval_str("[ok]").unwrap();
            let bytecode_doc = interp.eval_str("\"byte-code doc\"").unwrap();
            let Value::Str(doc_text) = &bytecode_doc else {
                unreachable!();
            };
            let held_bytecode_doc = doc_text.borrow_mut();
            let bytecode_args = vec![
                Value::Int(0),
                code.clone(),
                constants.clone(),
                Value::Int(1),
                bytecode_doc.clone(),
            ];
            let result = interp.apply(&make_byte_code, bytecode_args.clone());
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_bytecode_doc);
            assert!(matches!(
                interp.apply(&make_byte_code, bytecode_args),
                Ok(Value::Lambda(_))
            ));

            let read_expression = Value::Sym(interp.intern("read--expression"));
            let prompt = interp.eval_str("\"borrowed prompt\"").unwrap();
            let Value::Str(prompt_text) = &prompt else {
                unreachable!();
            };
            let held_prompt = prompt_text.borrow_mut();
            let result = interp.apply(&read_expression, vec![prompt.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_prompt);

            let make_closure = Value::Sym(interp.intern("make-closure"));
            let prototype = interp
                .eval_str("(make-byte-code 0 (unibyte-string 192 135) [ok] 1)")
                .unwrap();
            let Value::Lambda(lambda) = &prototype else {
                panic!("make-byte-code did not return a byte-code function");
            };
            let slots = lambda.bc_items.as_ref().expect("byte-code slots").clone();

            let held_slots = slots.borrow_mut();
            let result = interp.apply(&make_closure, vec![prototype.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_slots);
            assert!(matches!(
                interp.apply(&make_closure, vec![prototype.clone()]),
                Ok(Value::Lambda(_))
            ));

            let constants = match &slots.borrow()[2] {
                Value::Vec(constants) => constants.clone(),
                _ => panic!("byte-code constants slot is not a vector"),
            };
            let held_constants = constants.borrow_mut();
            let result = interp.apply(&make_closure, vec![prototype.clone(), Value::Int(99)]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_constants);
            assert!(matches!(
                interp.apply(&make_closure, vec![prototype, Value::Int(99)]),
                Ok(Value::Lambda(_))
            ));

            let interpreted = interp.eval_str("#[nil (quote ok) nil]").unwrap();
            let result = interp.apply(&make_closure, vec![interpreted.clone()]);
            let Err(Flow::Signal(signal)) = result else {
                panic!("make-closure accepted an interpreted closure prototype");
            };
            assert!(signal.is_symbol(sym::WRONG_TYPE_ARGUMENT));
            let data = signal.data().list_to_vec().unwrap();
            assert!(matches!(data.first(), Some(Value::Sym(predicate))
                if interp.symbol_name(*predicate) == "byte-code-function-p"));
        })
        .unwrap()
        .join()
        .unwrap();
}
