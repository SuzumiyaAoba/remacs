use remacs_runtime::lisp::obarray::sym;
use remacs_runtime::lisp::{Flow, Interp, Value};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

#[test]
fn func_arity_reports_borrowed_function_data_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();

            let slot_function = interp
                .eval_str("(make-byte-code 0 (unibyte-string 192 135) [ok] 1)")
                .unwrap();
            let slot_name = interp.intern("qa-borrowed-bytecode-arity-slots");
            interp.fset(slot_name, slot_function.clone());
            let Value::Lambda(lambda) = &slot_function else {
                panic!("make-byte-code did not return a byte-code function");
            };
            let slots = lambda.bc_items.as_ref().expect("byte-code slots").clone();
            let held_slots = slots.borrow_mut();
            let result = interp.eval_str("(func-arity 'qa-borrowed-bytecode-arity-slots)");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
            ));
            drop(held_slots);
            let normal_arity = interp
                .eval_str("(func-arity 'qa-borrowed-bytecode-arity-slots)")
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&normal_arity).to_unicode_lossy(),
                "(0 . 0)"
            );

            let arglist_function = interp
                .eval_str(
                    "(make-byte-code '(qa-borrowed-bytecode-arity-arg)\
                                    (unibyte-string 192 135) [ok] 1)",
                )
                .unwrap();
            let arglist_name = interp.intern("qa-borrowed-bytecode-arity-arglist");
            interp.fset(arglist_name, arglist_function.clone());
            let Value::Lambda(lambda) = &arglist_function else {
                panic!("make-byte-code did not return a byte-code function");
            };
            let slots = lambda.bc_items.as_ref().expect("byte-code slots").clone();
            let argdesc = slots.borrow()[0].clone();
            let Value::Cons(argdesc_cell) = argdesc else {
                panic!("list argdesc was not stored in the byte-code slots");
            };
            let held_argdesc = argdesc_cell.borrow_mut();
            let result = interp.eval_str("(func-arity 'qa-borrowed-bytecode-arity-arglist)");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
            ));
            let result = interp.eval_str("(qa-borrowed-bytecode-arity-arglist 7)");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::WRONG_NUMBER_OF_ARGUMENTS)
            ));
            drop(held_argdesc);
            let normal_arity = interp
                .eval_str("(func-arity 'qa-borrowed-bytecode-arity-arglist)")
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&normal_arity).to_unicode_lossy(),
                "(1 . 1)"
            );
            let normal_result = interp
                .eval_str("(qa-borrowed-bytecode-arity-arglist 7)")
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&normal_result).to_unicode_lossy(),
                "ok"
            );

            let code_function = interp
                .eval_str("(make-byte-code 0 (unibyte-string 192 135) [ok] 1)")
                .unwrap();
            let first_result = interp.apply(&code_function, Vec::<Value>::new()).unwrap();
            assert_eq!(
                interp.prin1_to_string(&first_result).to_unicode_lossy(),
                "ok"
            );
            let Value::Lambda(lambda) = &code_function else {
                panic!("make-byte-code did not return a byte-code function");
            };
            let slots = lambda.bc_items.as_ref().expect("byte-code slots").clone();
            let code = slots.borrow()[1].clone();
            let Value::Str(code) = code else {
                panic!("byte-code code slot is not a string");
            };
            let held_code = code.borrow_mut();
            let result = interp.apply(&code_function, Vec::<Value>::new());
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_code);
            let normal_result = interp.apply(&code_function, Vec::<Value>::new()).unwrap();
            assert_eq!(
                interp.prin1_to_string(&normal_result).to_unicode_lossy(),
                "ok"
            );
            let constants = match &slots.borrow()[2] {
                Value::Vec(constants) => constants.clone(),
                _ => panic!("byte-code constants slot is not a vector"),
            };
            let held_constants = constants.borrow_mut();
            let result = interp.apply(&code_function, Vec::<Value>::new());
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_constants);
            let normal_result = interp.apply(&code_function, Vec::<Value>::new()).unwrap();
            assert_eq!(
                interp.prin1_to_string(&normal_result).to_unicode_lossy(),
                "ok"
            );

            for (opcode, expected) in [(64, "1"), (65, "2"), (162, "1"), (163, "2")] {
                let source = format!(
                    "(make-byte-code 0 (unibyte-string 192 {opcode} 135) [(1 . 2)] 1)"
                );
                let function = interp.eval_str(&source).unwrap();
                let Value::Lambda(lambda) = &function else {
                    panic!("make-byte-code did not return a byte-code function");
                };
                let slots = lambda.bc_items.as_ref().expect("byte-code slots").clone();
                let constants = match &slots.borrow()[2] {
                    Value::Vec(constants) => constants.clone(),
                    _ => panic!("byte-code constants slot is not a vector"),
                };
                let constant = constants.borrow()[0].clone();
                let Value::Cons(constant_cell) = constant else {
                    panic!("byte-code constant is not a cons");
                };
                let held_constant = constant_cell.borrow_mut();
                let result = interp.apply(&function, Vec::<Value>::new());
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
                drop(held_constant);
                let normal_result = interp.apply(&function, Vec::<Value>::new()).unwrap();
                assert_eq!(
                    interp.prin1_to_string(&normal_result).to_unicode_lossy(),
                    expected
                );
            }

            let func_arity = Value::Sym(interp.intern("func-arity"));
            let macro_function = interp
                .eval_str(
                    "(progn (defmacro qa-borrowed-macro-arity (arg) arg)\
                            (symbol-function 'qa-borrowed-macro-arity))",
                )
                .unwrap();
            let Value::Cons(macro_cell) = &macro_function else {
                panic!("macro function cell is not a cons");
            };
            let functionp = Value::Sym(interp.intern("functionp"));
            let macrop = Value::Sym(interp.intern("macrop"));
            let held_macro = macro_cell.borrow_mut();
            let result = interp.apply(&func_arity, vec![macro_function.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let result = interp.apply(&functionp, vec![macro_function.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let result = interp.apply(&macrop, vec![macro_function.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_macro);
            assert!(matches!(
                interp.apply(&func_arity, vec![macro_function.clone()]),
                Ok(Value::Cons(_))
            ));
            assert!(interp
                .apply(&functionp, vec![macro_function.clone()])
                .unwrap()
                .is_nil());
            assert!(interp
                .apply(&macrop, vec![macro_function])
                .unwrap()
                .truthy());

            let autoload = interp.eval_str("(list 'autoload \"unused-library\")").unwrap();
            let Value::Cons(autoload_cell) = &autoload else {
                unreachable!();
            };
            let held_autoload = autoload_cell.borrow_mut();
            let result = interp.apply(&func_arity, vec![autoload.clone()]);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            drop(held_autoload);
            assert!(matches!(
                interp.apply(&func_arity, vec![autoload]),
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
            ));

            // The native compiler signature helper also inspects a list
            // byte-code argdesc. A Rust caller can keep that cons borrowed;
            // report the conflict as a Lisp error instead of panicking.
            let argdesc = Value::cons(
                Value::Sym(interp.intern("qa-borrowed-comp-signature-arg")),
                Value::Nil,
            );
            let mut lambda = interp.parse_lambda(&argdesc, &[], None).unwrap();
            lambda.compiled = true;
            lambda.bc_items = Some(Rc::new(RefCell::new(vec![argdesc.clone()])));
            let function = Value::Lambda(Rc::new(lambda));
            let signature = Value::Sym(interp.intern("comp--subr-signature"));
            let valid_signature = interp
                .apply(&signature, vec![function.clone()])
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&valid_signature).to_unicode_lossy(),
                "\"anonymous(1 . 1)\""
            );
            let Value::Lambda(lambda) = &function else {
                unreachable!();
            };
            let slots = lambda.bc_items.as_ref().unwrap().clone();
            let held_slots = slots.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&signature, vec![function.clone()])
            }));
            drop(held_slots);
            assert!(result.is_ok(), "signature lookup must not panic on borrowed slots");
            assert!(matches!(
                result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let Value::Cons(argdesc_cell) = &argdesc else {
                unreachable!();
            };
            let held_argdesc = argdesc_cell.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&signature, vec![function.clone()])
            }));
            drop(held_argdesc);
            assert!(result.is_ok(), "signature lookup must not panic");
            assert!(matches!(
                result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
            ));

            argdesc_cell.borrow_mut().cdr = argdesc.clone();
            assert!(matches!(
                interp.apply(&signature, vec![function]),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::INVALID_FUNCTION)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
