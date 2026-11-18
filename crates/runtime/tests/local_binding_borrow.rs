use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn borrowed_local_slot_keeps_its_unwind_record_for_a_later_retry() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(defvar qa-borrowed-local-unbind-variable 10)\
                     (make-local-variable 'qa-borrowed-local-unbind-variable)\
                     (setq qa-borrowed-local-unbind-variable 10)",
                )
                .unwrap();
            let symbol = interp.intern("qa-borrowed-local-unbind-variable");
            let buffer = interp.current_buffer_ref().expect("current buffer");

            let mark = interp.specbind_depth();
            interp.specbind(symbol, Value::Int(20)).unwrap();
            assert!(matches!(interp.symbol_value(symbol), Value::Int(20)));

            let borrow = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| interp.unbind_to(mark)));
            drop(borrow);

            let result = result.expect("unbinding must not panic while the buffer is borrowed");
            assert!(result.is_err());
            assert_eq!(interp.specbind_depth(), mark + 1);

            interp.unbind_to(mark).unwrap();
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(symbol), Value::Int(10)));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_symbol_plist_does_not_panic_during_get() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(put 'qa-borrowed-plist-symbol 'qa-borrowed-plist-key 42)")
                .unwrap();
            let plist = interp
                .eval_str("(symbol-plist 'qa-borrowed-plist-symbol)")
                .unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("symbol plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(get 'qa-borrowed-plist-symbol 'qa-borrowed-plist-key)")
            }));
            drop(borrow);

            let result = result.expect("get must not panic while its plist is borrowed");
            assert!(result.is_err(), "get must report the unavailable plist read");
            assert!(matches!(
                interp
                    .eval_str("(get 'qa-borrowed-plist-symbol 'qa-borrowed-plist-key)")
                    .unwrap(),
                Value::Int(42)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_symbol_plist_does_not_panic_during_rust_put_prop() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let symbol = interp.intern("qa-borrowed-put-prop-symbol");
            let property = interp.intern("qa-borrowed-put-prop-key");
            interp.put_prop(symbol, property, Value::Int(42));
            let plist = interp
                .eval_str("(symbol-plist 'qa-borrowed-put-prop-symbol)")
                .unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("symbol plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.put_prop(symbol, property, Value::Int(43))
            }));
            drop(borrow);

            result.expect("put_prop must not panic while its plist is borrowed");
            assert!(matches!(interp.get_prop(symbol, property), Value::Int(43)));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_symbol_plist_does_not_panic_during_put() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(put 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key 42)")
                .unwrap();
            let plist = interp
                .eval_str("(symbol-plist 'qa-borrowed-put-plist-symbol)")
                .unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("symbol plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(put 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key 43)")
            }));
            drop(borrow);

            let result = result.expect("put must not panic while its plist is borrowed");
            assert!(result.is_err(), "put must report the unavailable plist write");
            assert!(matches!(
                interp
                    .eval_str("(get 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key)")
                    .unwrap(),
                Value::Int(42)
            ));
            interp
                .eval_str("(put 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key 43)")
                .unwrap();
            let plist = interp
                .eval_str("(symbol-plist 'qa-borrowed-put-plist-symbol)")
                .unwrap();
            let Value::Cons(key_cell) = &plist else {
                panic!("symbol plist is not a cons");
            };
            let value_cell = match key_cell.borrow().cdr.clone() {
                Value::Cons(cell) => cell,
                _ => panic!("symbol plist value cell is missing"),
            };
            let borrow = value_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(put 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key 45)")
            }));
            drop(borrow);
            let result = result.expect("put must not panic when its value cell is borrowed");
            assert!(result.is_err(), "put must report the unavailable value cell");
            assert!(matches!(
                interp
                    .eval_str("(get 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key)")
                    .unwrap(),
                Value::Int(43)
            ));
            interp
                .eval_str(
                    "(put 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-second-key 44)",
                )
                .unwrap();
            assert!(matches!(
                interp
                    .eval_str("(get 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-key)")
                    .unwrap(),
                Value::Int(43)
            ));
            assert!(matches!(
                interp
                    .eval_str("(get 'qa-borrowed-put-plist-symbol 'qa-borrowed-put-plist-second-key)")
                    .unwrap(),
                Value::Int(44)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_value_plist_does_not_panic_during_plist_put() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq qa-borrowed-value-plist (list 'qa-key 42))")
                .unwrap();
            let plist = interp.eval_str("qa-borrowed-value-plist").unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("value plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(plist-put qa-borrowed-value-plist 'qa-key 43)")
            }));
            drop(borrow);

            let result = result.expect("plist-put must not panic while its list is borrowed");
            assert!(result.is_err(), "plist-put must report the unavailable list write");
            assert!(matches!(
                interp.eval_str("(plist-get qa-borrowed-value-plist 'qa-key)").unwrap(),
                Value::Int(42)
            ));
            interp
                .eval_str("(plist-put qa-borrowed-value-plist 'qa-key 43)")
                .unwrap();
            let value_cell = match cell.borrow().cdr.clone() {
                Value::Cons(cell) => cell,
                _ => panic!("value plist value cell is missing"),
            };
            let borrow = value_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(plist-put qa-borrowed-value-plist 'qa-key 45)")
            }));
            drop(borrow);
            let result = result.expect("plist-put must not panic when its value cell is borrowed");
            assert!(result.is_err(), "plist-put must report the unavailable value cell");
            assert!(matches!(
                interp.eval_str("(plist-get qa-borrowed-value-plist 'qa-key)").unwrap(),
                Value::Int(43)
            ));
            interp
                .eval_str("(plist-put qa-borrowed-value-plist 'qa-second-key 44)")
                .unwrap();
            assert!(matches!(
                interp.eval_str("(plist-get qa-borrowed-value-plist 'qa-key)").unwrap(),
                Value::Int(43)
            ));
            assert!(matches!(
                interp
                    .eval_str("(plist-get qa-borrowed-value-plist 'qa-second-key)")
                    .unwrap(),
                Value::Int(44)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_value_plist_does_not_panic_during_plist_get() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq qa-borrowed-get-plist (list 'qa-key 42))")
                .unwrap();
            let plist = interp.eval_str("qa-borrowed-get-plist").unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("value plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(plist-get qa-borrowed-get-plist 'qa-key)")
            }));
            let custom_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(plist-get qa-borrowed-get-plist 'qa-key #'eq)")
            }));
            drop(borrow);

            let result = result.expect("plist-get must not panic while its list is borrowed");
            assert!(result.is_err(), "plist-get must report the unavailable list read");
            let custom_result = custom_result
                .expect("custom-predicate plist-get must not panic while its list is borrowed");
            assert!(custom_result.is_err());
            assert!(matches!(
                interp
                    .eval_str("(plist-get qa-borrowed-get-plist 'qa-key)")
                    .unwrap(),
                Value::Int(42)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn defvaralias_reports_borrowed_buffer_without_installing_alias() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let new_variable = interp.intern("qa-borrowed-defvaralias-new");
            let base_variable = interp.intern("qa-borrowed-defvaralias-base");
            let defvaralias = Value::Sym(interp.intern("defvaralias"));
            let buffer = interp.current_buffer_ref().expect("current buffer");

            let held = buffer.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &defvaralias,
                    vec![Value::Sym(new_variable), Value::Sym(base_variable)],
                )
            }));
            drop(held);

            assert!(result.is_ok(), "defvaralias must not panic on a borrowed buffer");
            assert!(result.unwrap().is_err());
            assert!(interp.obarray.symbol(new_variable).var_alias.is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_value_plist_does_not_panic_during_plist_member() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq qa-borrowed-member-plist (list 'qa-key 42))")
                .unwrap();
            let plist = interp.eval_str("qa-borrowed-member-plist").unwrap();
            let Value::Cons(cell) = &plist else {
                panic!("value plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(plist-member qa-borrowed-member-plist 'qa-key)")
            }));
            drop(borrow);

            let result = result.expect("plist-member must not panic while its list is borrowed");
            assert!(result.is_err(), "plist-member must report the unavailable list read");
            assert!(matches!(
                interp
                    .eval_str("(plist-get qa-borrowed-member-plist 'qa-key)")
                    .unwrap(),
                Value::Int(42)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn plist_put_front_falls_back_when_the_value_cell_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prop = Value::Sym(interp.intern("qa-borrowed-front-plist-key"));
            let plist = Value::list(vec![prop.clone(), Value::Int(1)]);
            let Value::Cons(key_cell) = &plist else {
                panic!("plist is not a cons");
            };
            let value_cell = match key_cell.borrow().cdr.clone() {
                Value::Cons(cell) => cell,
                _ => panic!("plist value cell is missing"),
            };

            let borrow = value_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                remacs_runtime::lisp::eval::plist_put_front(&plist, &prop, Value::Int(2))
            }));
            drop(borrow);

            let updated = result.expect("plist_put_front must not panic on a borrowed value cell");
            assert!(matches!(
                remacs_runtime::lisp::eval::plist_get_v(&updated, &prop),
                Value::Int(2)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_plist_removal_does_not_panic_or_discard_the_list() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let property = Value::Sym(interp.intern("qa-borrowed-remove-key"));
            let plist = Value::list(vec![property.clone(), Value::Int(1)]);
            let Value::Cons(cell) = &plist else {
                panic!("plist is not a cons");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                remacs_runtime::lisp::eval::plist_remove_v(&plist, &property)
            }));
            drop(borrow);

            let result = result.expect("plist removal must not panic on a borrowed list");
            assert_eq!(
                interp.prin1_to_string(&result),
                "(qa-borrowed-remove-key 1)"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_cons_primitives_signal_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq qa-borrowed-cons (cons 1 2))")
                .unwrap();
            let cons = interp.eval_str("qa-borrowed-cons").unwrap();
            let Value::Cons(cell) = &cons else {
                panic!("value is not a cons");
            };

            let borrow = cell.borrow_mut();
            for (name, form) in [
                ("car", "(car qa-borrowed-cons)"),
                ("cdr", "(cdr qa-borrowed-cons)"),
                ("car-safe", "(car-safe qa-borrowed-cons)"),
                ("cdr-safe", "(cdr-safe qa-borrowed-cons)"),
                ("setcar", "(setcar qa-borrowed-cons 7)"),
                ("setcdr", "(setcdr qa-borrowed-cons 8)"),
            ] {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| interp.eval_str(form)));
                let result = result.unwrap_or_else(|_| panic!("{name} panicked on a borrowed cons"));
                assert!(result.is_err(), "{name} must signal while the cons is borrowed");
            }
            drop(borrow);

            assert!(matches!(
                interp.eval_str("(car qa-borrowed-cons)").unwrap(),
                Value::Int(1)
            ));
            assert!(matches!(
                interp.eval_str("(cdr qa-borrowed-cons)").unwrap(),
                Value::Int(2)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_lexical_frame_does_not_panic_during_closure_slot_primitives() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let function = interp.intern("qa-borrowed-slot-function");
            interp.lexenv = remacs_runtime::lisp::eval::lexenv_root();
            let lambda = interp
                .parse_lambda(&Value::Nil, &[Value::Nil], Some(function))
                .unwrap();
            let frame = lambda.env.as_ref().expect("lexical frame").clone();
            let shared_env = lambda.env.clone();
            let function_value = Value::Lambda(std::rc::Rc::new(lambda));
            interp.obarray.symbol_mut(function).value = function_value;
            let second_function = interp.intern("qa-borrowed-slot-function-two");
            let mut second_lambda = interp
                .parse_lambda(&Value::Nil, &[Value::Nil], Some(second_function))
                .unwrap();
            second_lambda.env = shared_env;
            interp.obarray.symbol_mut(second_function).value =
                Value::Lambda(std::rc::Rc::new(second_lambda));

            let borrow = frame.raw.borrow_mut();
            for form in [
                "(length qa-borrowed-slot-function)",
                "(length= qa-borrowed-slot-function 3)",
                "(aref qa-borrowed-slot-function 2)",
                "(append qa-borrowed-slot-function nil)",
                "(vconcat qa-borrowed-slot-function)",
                "(equal qa-borrowed-slot-function qa-borrowed-slot-function-two)",
                "(sxhash qa-borrowed-slot-function)",
                "(sxhash-equal-including-properties qa-borrowed-slot-function)",
            ] {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| interp.eval_str(form)));
                let result = result.unwrap_or_else(|_| {
                    panic!("{form} panicked on a borrowed closure environment")
                });
                assert!(
                    result.is_err(),
                    "{form} must report the unavailable closure slots"
                );
            }
            drop(borrow);

            let raw_environment = frame.raw.borrow().clone();
            let Value::Cons(environment_cell) = &raw_environment else {
                panic!("lexical closure environment is not a list");
            };
            let borrow = environment_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(sxhash qa-borrowed-slot-function)")
            }));
            drop(borrow);
            let result = result.expect("sxhash must not panic on a borrowed environment cell");
            assert!(
                result.is_err(),
                "sxhash must report the unavailable closure slots"
            );

            let interactive_function = interp.intern("qa-borrowed-interactive-function");
            let interactive_form =
                Value::list(vec![Value::Sym(interp.intern("interactive")), Value::Nil]);
            let interactive_lambda = interp
                .parse_lambda(&Value::Nil, &[interactive_form], Some(interactive_function))
                .unwrap();
            let interactive = interactive_lambda
                .interactive
                .clone()
                .expect("lambda has no interactive slot");
            let Value::Cons(interactive_cell) = &interactive else {
                panic!("interactive slot is not a cons");
            };
            interp.obarray.symbol_mut(interactive_function).value =
                Value::Lambda(std::rc::Rc::new(interactive_lambda));
            let borrow = interactive_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(length qa-borrowed-interactive-function)")
            }));
            drop(borrow);
            let result = result.expect("length must not panic on a borrowed interactive slot");
            assert!(result.is_err(), "borrowed interactive slot must signal");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_bytecode_slots_do_not_panic_during_closure_slot_primitives() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(r#"(setq qa-borrowed-bytecode-function #[257 "\207" [] 1])"#)
                .unwrap();
            let function = interp.eval_str("qa-borrowed-bytecode-function").unwrap();
            let Value::Lambda(lambda) = &function else {
                panic!("value is not a byte-code function");
            };
            let items = lambda
                .bc_items
                .as_ref()
                .expect("byte-code object has no slot vector")
                .clone();

            let borrow = items.borrow_mut();
            for form in [
                "(length qa-borrowed-bytecode-function)",
                "(aref qa-borrowed-bytecode-function 0)",
                "(vconcat qa-borrowed-bytecode-function)",
                "(sxhash qa-borrowed-bytecode-function)",
            ] {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| interp.eval_str(form)));
                let result = result.unwrap_or_else(|_| {
                    panic!("{form} panicked on a borrowed byte-code slot vector")
                });
                assert!(
                    result.is_err(),
                    "{form} must report unavailable byte-code slots"
                );
            }
            drop(borrow);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn safe_length_signals_instead_of_panicking_on_a_borrowed_cons() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(setq qa-borrowed-length-list (list 1 2 3))")
                .unwrap();
            let list = interp.eval_str("qa-borrowed-length-list").unwrap();
            let Value::Cons(cell) = &list else {
                panic!("value is not a list");
            };

            let borrow = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(safe-length qa-borrowed-length-list)")
            }));
            let proper = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(proper-list-p qa-borrowed-length-list)")
            }));
            drop(borrow);

            let result = result.expect("safe-length must not panic on a borrowed cons");
            assert!(result.is_err(), "safe-length must report the unavailable traversal");
            let proper = proper.expect("proper-list-p must not panic on a borrowed cons");
            assert!(proper.is_err(), "proper-list-p must report the unavailable traversal");
            assert!(matches!(
                interp.eval_str("(safe-length qa-borrowed-length-list)").unwrap(),
                Value::Int(3)
            ));
            assert!(matches!(
                interp.eval_str("(proper-list-p qa-borrowed-length-list)").unwrap(),
                Value::Int(3)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
