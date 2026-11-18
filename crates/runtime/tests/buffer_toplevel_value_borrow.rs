use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn buffer_toplevel_value_primitives_report_a_borrowed_buffer_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(defvar qa-buffer-toplevel-borrow-variable 1) \
                     (make-local-variable 'qa-buffer-toplevel-borrow-variable) \
                     (setq qa-buffer-toplevel-borrow-variable 5)",
                )
                .unwrap();
            let variable = Value::Sym(interp.intern("qa-buffer-toplevel-borrow-variable"));
            let buffer = interp.current_buffer_ref().expect("current buffer");
            let buffer_value = Value::Buffer(buffer.clone());
            let getter = Value::Sym(interp.intern("buffer-local-toplevel-value"));
            let setter = Value::Sym(interp.intern("set-buffer-local-toplevel-value"));
            let binding_locus = Value::Sym(interp.intern("variable-binding-locus"));

            let held = buffer.borrow_mut();
            let get_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&getter, vec![variable.clone(), buffer_value.clone()])
            }));
            let set_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &setter,
                    vec![variable.clone(), Value::Int(9), buffer_value.clone()],
                )
            }));
            let locus_result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&binding_locus, vec![variable.clone()])
            }));
            drop(held);

            assert!(get_result.is_ok(), "buffer-local-toplevel-value must not panic");
            assert!(get_result.unwrap().is_err());
            assert!(set_result.is_ok(), "set-buffer-local-toplevel-value must not panic");
            assert!(set_result.unwrap().is_err());
            assert!(locus_result.is_ok(), "variable-binding-locus must not panic");
            assert!(locus_result.unwrap().is_err());
            assert!(matches!(interp.symbol_value(interp.sym_id(&variable).unwrap()), Value::Int(5)));

            assert!(matches!(
                interp
                    .apply(&getter, vec![variable.clone(), buffer_value.clone()])
                    .unwrap(),
                Value::Int(5)
            ));
            interp
                .apply(
                    &setter,
                    vec![variable.clone(), Value::Int(9), buffer_value.clone()],
                )
                .unwrap();
            assert!(matches!(
                interp
                    .apply(&getter, vec![variable.clone(), buffer_value])
                    .unwrap(),
                Value::Int(9)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
