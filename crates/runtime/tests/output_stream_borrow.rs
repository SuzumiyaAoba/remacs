use remacs_runtime::lisp::obarray::sym;
use remacs_runtime::lisp::{Flow, Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn princ_to_a_mutably_borrowed_buffer_returns_a_lisp_error() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let output = interp
                .eval_str("(get-buffer-create \" *qa-borrowed-output-stream*\")")
                .expect("create output buffer");
            let Value::Buffer(buffer) = output.clone() else {
                panic!("get-buffer-create did not return a buffer");
            };
            let stream = interp.intern("qa-borrowed-output-stream");
            interp.set_symbol(stream, output).unwrap();

            let held = buffer.borrow_mut();
            let at_bol = catch_unwind(AssertUnwindSafe(|| {
                interp.output_at_bol(&Value::Buffer(buffer.clone()))
            }));
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(princ \"payload\" qa-borrowed-output-stream)")
            }));
            drop(held);

            assert!(at_bol.is_ok(), "output_at_bol must not panic on a borrowed buffer");
            assert!(!at_bol.unwrap());
            let result = result.expect("princ must not panic when its buffer stream is borrowed");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let marker = interp
                .eval_str("(with-current-buffer qa-borrowed-output-stream (point-marker))")
                .expect("create output marker");
            let Value::Marker(marker_ref) = marker.clone() else {
                panic!("point-marker did not return a marker");
            };
            let marker_symbol = interp.intern("qa-borrowed-output-marker");
            interp.set_symbol(marker_symbol, marker).unwrap();
            let held = marker_ref.borrow_mut();
            let at_bol = catch_unwind(AssertUnwindSafe(|| {
                interp.output_at_bol(&Value::Marker(marker_ref.clone()))
            }));
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(terpri qa-borrowed-output-marker t)")
            }));
            drop(held);

            assert!(at_bol.is_ok(), "output_at_bol must not panic on a borrowed marker");
            assert!(!at_bol.unwrap());
            let result = result.expect("terpri must not panic when its marker stream is borrowed");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let function = Value::list(vec![
                Value::Sym(interp.intern("lambda")),
                Value::list(vec![Value::Sym(interp.intern("character"))]),
                Value::Nil,
            ]);
            let Value::Cons(function_ref) = function.clone() else {
                unreachable!();
            };
            let function_symbol = interp.intern("qa-borrowed-output-function");
            interp.set_symbol(function_symbol, function).unwrap();
            let held = function_ref.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(princ \"payload\" qa-borrowed-output-function)")
            }));
            drop(held);

            let result = result.expect("princ must not panic when its function stream is borrowed");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
