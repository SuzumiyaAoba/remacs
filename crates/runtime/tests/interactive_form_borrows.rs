use remacs_runtime::lisp::obarray::sym;
use remacs_runtime::lisp::{Flow, Interp, Value};

#[test]
fn interactive_form_reports_mutably_borrowed_specs_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(defun qa-borrowed-interactive-command ()\
                       (interactive) 'ok)",
                )
                .unwrap();
            let command = interp.intern("qa-borrowed-interactive-command");
            let function = interp.symbol_function(command);
            let Value::Lambda(lambda) = function else {
                panic!("command function is not a lambda");
            };
            let spec = lambda.interactive.clone().expect("interactive form");
            let Value::Cons(cell) = &spec else {
                panic!("interactive form is not a cons");
            };
            let held_spec = cell.borrow_mut();
            let result = interp.eval_str("(interactive-form 'qa-borrowed-interactive-command)");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_spec);
            let normal = interp
                .eval_str("(interactive-form 'qa-borrowed-interactive-command)")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&normal), "(interactive nil)");

            let form = Value::list(vec![
                Value::Sym(sym::LAMBDA),
                Value::Nil,
                Value::list(vec![Value::Sym(sym::INTERACTIVE), Value::Nil]),
                Value::Sym(sym::T),
            ]);
            let Value::Cons(function_cell) = &form else {
                unreachable!();
            };
            let Value::Cons(body_cell) = function_cell.borrow().cdr.clone() else {
                unreachable!();
            };
            let Value::Cons(spec_cell) = body_cell.borrow().cdr.clone() else {
                unreachable!();
            };
            let interactive_form = spec_cell.borrow().car.clone();
            let Value::Cons(interactive_cell) = &interactive_form else {
                unreachable!();
            };
            let held_nested_spec = interactive_cell.borrow_mut();
            let interactive_form_fn = Value::Sym(interp.intern("interactive-form"));
            let result = interp.apply(&interactive_form_fn, vec![form.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_nested_spec);
            let normal = interp.apply(&interactive_form_fn, vec![form]).unwrap();
            assert_eq!(interp.prin1_to_string(&normal), "(interactive nil)");

            let interpreted_bytecode = interp
                .eval_str("#[nil (quote ok) nil nil nil [\"\"]]")
                .unwrap();
            let Value::Lambda(lambda) = &interpreted_bytecode else {
                panic!("byte-code literal did not return a function");
            };
            let items = lambda.bc_items.as_ref().expect("byte-code slots").clone();
            let specifications = match &items.borrow()[5] {
                Value::Vec(specifications) => specifications.clone(),
                _ => panic!("byte-code interactive slot is not a vector"),
            };
            let held_specifications = specifications.borrow_mut();
            let result = interp.apply(&interactive_form_fn, vec![interpreted_bytecode.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_specifications);
            let normal = interp
                .apply(&interactive_form_fn, vec![interpreted_bytecode])
                .unwrap();
            assert_eq!(interp.prin1_to_string(&normal), "(interactive \"\")");
        })
        .unwrap()
        .join()
        .unwrap();
}
