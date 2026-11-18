use remacs_runtime::lisp::{Flow, Interp, Value, sym};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn prin1_to_string_does_not_panic_when_a_closure_environment_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let closure = interp
                .eval_str("(eval '(let ((qa-print-env-value 1)) (lambda () qa-print-env-value)) t)")
                .expect("create lexical closure");
            let Value::Lambda(lambda) = &closure else {
                panic!("eval did not return a lambda");
            };
            let frame = lambda.env.as_ref().expect("closure has lexical environment");
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = frame.raw.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&closure)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![closure.clone()])
                }));
                drop(held);

                assert!(
                    rust_result.is_ok(),
                    "Rust printing must not panic on a borrowed closure environment"
                );
                assert!(lisp_result.is_ok(), "Lisp printing must not panic");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_cons_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let cons = Value::cons(Value::Int(1), Value::Nil);
            let Value::Cons(cell) = &cons else {
                unreachable!();
            };

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = cell.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&cons)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![cons.clone()])
                }));
                drop(held);

                assert!(rust_result.is_ok(), "Rust printing must not panic on a borrowed cons");
                assert!(lisp_result.is_ok(), "Lisp printing must not panic on a borrowed cons");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_vector_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let vector = Value::Vec(Value::vector_ref(vec![Value::Int(1)]));
            let Value::Vec(items) = &vector else {
                unreachable!();
            };

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = items.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&vector)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![vector.clone()])
                }));
                drop(held);

                assert!(rust_result.is_ok(), "Rust printing must not panic on a borrowed vector");
                assert!(lisp_result.is_ok(), "Lisp printing must not panic on a borrowed vector");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_string_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let string = Value::string("borrowed string");
            let Value::Str(text) = &string else {
                unreachable!();
            };

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = text.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&string)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![string.clone()])
                }));
                drop(held);

                assert!(rust_result.is_ok(), "Rust printing must not panic on a borrowed string");
                assert!(lisp_result.is_ok(), "Lisp printing must not panic on a borrowed string");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_hash_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let hash = interp.eval_str("(make-hash-table :test 'eq)").unwrap();
            let Value::Hash(table) = &hash else {
                panic!("make-hash-table did not return a hash table");
            };

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = table.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&hash)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![hash.clone()])
                }));
                drop(held);

                assert!(rust_result.is_ok(), "Rust printing must not panic on a borrowed hash");
                assert!(lisp_result.is_ok(), "Lisp printing must not panic on a borrowed hash");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_record_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let tag = Value::Sym(interp.intern("qa-borrowed-printer-record"));
            let record = Value::Record(Value::vector_ref(vec![tag, Value::Int(1)]));
            let Value::Record(items) = &record else {
                unreachable!();
            };

            for print_circle in [false, true] {
                interp
                    .eval_str(if print_circle {
                        "(setq print-circle t)"
                    } else {
                        "(setq print-circle nil)"
                    })
                    .unwrap();
                let held = items.borrow_mut();
                let rust_result =
                    catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&record)));
                let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                    interp.apply(&prin1_to_string, vec![record.clone()])
                }));
                drop(held);

                assert!(rust_result.is_ok(), "Rust printing must not panic on a borrowed record");
                assert!(lisp_result.is_ok(), "Lisp printing must not panic on a borrowed record");
                assert!(matches!(
                    lisp_result.unwrap(),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                ));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_mutably_borrowed_editor_objects_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            macro_rules! check_borrowed {
                ($object:expr, $borrow:expr, $kind:literal) => {{
                    let object = $object.clone();
                    let held = $borrow;
                    let rust_result = catch_unwind(AssertUnwindSafe(|| {
                        interp.prin1_to_string(&object)
                    }));
                    let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                        interp.apply(&prin1_to_string, vec![object.clone()])
                    }));
                    drop(held);
                    assert!(rust_result.is_ok(), "Rust printer panicked on borrowed {}", $kind);
                    assert!(lisp_result.is_ok(), "Lisp printer panicked on borrowed {}", $kind);
                    assert!(matches!(
                        lisp_result.unwrap(),
                        Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
                    ));
                }};
            }

            let buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer_ref) = &buffer else {
                panic!("current-buffer did not return a buffer");
            };
            check_borrowed!(buffer, buffer_ref.borrow_mut(), "buffer");

            let marker = interp.eval_str("(point-marker)").unwrap();
            let Value::Marker(marker_ref) = &marker else {
                panic!("point-marker did not return a marker");
            };
            check_borrowed!(marker, marker_ref.borrow_mut(), "marker");

            let window = interp.eval_str("(selected-window)").unwrap();
            let Value::Window(window_ref) = &window else {
                panic!("selected-window did not return a window");
            };
            check_borrowed!(window, window_ref.borrow_mut(), "window");

            let frame = interp.eval_str("(selected-frame)").unwrap();
            let Value::Frame(frame_ref) = &frame else {
                panic!("selected-frame did not return a frame");
            };
            check_borrowed!(frame, frame_ref.borrow_mut(), "frame");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_mutably_borrowed_sync_objects_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let mutex = interp.eval_str("(make-mutex \"qa-printer-mutex\")").unwrap();
            let Value::Mutex(mutex_ref) = &mutex else {
                panic!("make-mutex did not return a mutex");
            };

            let held = mutex_ref.borrow_mut();
            let rust_result = catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&mutex)));
            let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&prin1_to_string, vec![mutex.clone()])
            }));
            drop(held);

            assert!(rust_result.is_ok(), "Rust printer panicked on a borrowed mutex");
            assert!(lisp_result.is_ok(), "Lisp printer panicked on a borrowed mutex");
            assert!(matches!(
                lisp_result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let condvar = interp
                .eval_str(
                    "(make-condition-variable (make-mutex \"qa-printer-condvar-mutex\"))",
                )
                .unwrap();
            let Value::CondVar(condvar_ref) = &condvar else {
                panic!("make-condition-variable did not return a condition variable");
            };

            let held = condvar_ref.borrow_mut();
            let rust_result = catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&condvar)));
            let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&prin1_to_string, vec![condvar.clone()])
            }));
            drop(held);

            assert!(rust_result.is_ok(), "Rust printer panicked on a borrowed condvar");
            assert!(lisp_result.is_ok(), "Lisp printer panicked on a borrowed condvar");
            assert!(matches!(
                lisp_result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_a_mutably_borrowed_process_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let process = interp
                .eval_str(
                    "(make-process :name \"qa-printer-borrowed-process\"\
                                   :command '(\"/usr/bin/true\")\
                                   :connection-type 'pipe :noquery t)",
                )
                .expect("make process");
            let Value::Process(process_ref) = &process else {
                panic!("make-process did not return a process");
            };

            let held = process_ref.borrow_mut();
            let rust_result = catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&process)));
            let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&prin1_to_string, vec![process.clone()])
            }));
            drop(held);

            assert!(rust_result.is_ok(), "Rust printer panicked on a borrowed process");
            assert!(lisp_result.is_ok(), "Lisp printer panicked on a borrowed process");
            assert!(matches!(
                lisp_result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let delete_process = Value::Sym(interp.intern("delete-process"));
            let _ = interp.apply(&delete_process, vec![process]);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_does_not_panic_on_a_borrowed_float_output_format() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let float_output_format = interp.intern("float-output-format");
            let format = Value::string("%.17g");
            interp.set_symbol(float_output_format, format.clone()).unwrap();
            let Value::Str(format_text) = &format else {
                unreachable!();
            };
            let float = interp.eval_str("1.5").unwrap();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));

            let held = format_text.borrow_mut();
            let rust_result =
                catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&float)));
            let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&prin1_to_string, vec![float.clone()])
            }));
            drop(held);

            assert!(rust_result.is_ok(), "Rust printer panicked on a borrowed float format");
            assert!(lisp_result.is_ok(), "Lisp printer panicked on a borrowed float format");
            assert!(matches!(lisp_result.unwrap(), Ok(Value::Str(_))));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn prin1_to_string_reports_mutably_borrowed_bytecode_slots_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let prin1_to_string = Value::Sym(interp.intern("prin1-to-string"));
            let function = interp
                .eval_str("(make-byte-code 0 (unibyte-string 192 135) [ok] 1)")
                .unwrap();
            let Value::Lambda(lambda) = &function else {
                panic!("make-byte-code did not return a byte-code function");
            };
            let items = lambda
                .bc_items
                .as_ref()
                .expect("byte-code function has no slot vector")
                .clone();

            let held = items.borrow_mut();
            let rust_result = catch_unwind(AssertUnwindSafe(|| interp.prin1_to_string(&function)));
            let lisp_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&prin1_to_string, vec![function.clone()])
            }));
            drop(held);

            assert!(rust_result.is_ok(), "Rust printer panicked on borrowed byte-code slots");
            assert!(lisp_result.is_ok(), "Lisp printer panicked on borrowed byte-code slots");
            assert!(matches!(
                lisp_result.unwrap(),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
