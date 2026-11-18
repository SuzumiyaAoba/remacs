use remacs_runtime::lisp::{Argv, Arity, EvalResult, Interp, Subr, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

static BORROW_BUFFER_ON_UNLET: Subr = Subr {
    name: "qa-borrow-buffer-on-unlet",
    arity: Arity::Range { min: 4, max: 4 },
    func: borrow_buffer_on_unlet,
    doc: "",
};

fn borrow_buffer_on_unlet(interp: &mut Interp, args: Argv) -> EvalResult {
    if matches!(args.get(2), Some(Value::Sym(operation))
        if interp.intern_soft("unlet") == Some(*operation))
        && let Some(buffer) = interp.current_buffer_ref()
    {
        // Simulate a host watcher that leaves the original buffer borrowed
        // after its callback returns.
        std::mem::forget(buffer.borrow_mut());
    }
    Ok(Value::Nil)
}

#[test]
fn unlet_watcher_created_buffer_borrow_does_not_panic_or_drop_unwind_record() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-unlet-buffer-borrow-variable");
            let watcher = interp.intern("qa-borrow-buffer-on-unlet");
            interp.fset(watcher, Value::Subr(&BORROW_BUFFER_ON_UNLET));
            interp
                .eval_str(
                    "(defvar qa-unlet-buffer-borrow-variable nil) \
                     (make-local-variable 'qa-unlet-buffer-borrow-variable) \
                     (add-variable-watcher 'qa-unlet-buffer-borrow-variable \
                                           'qa-borrow-buffer-on-unlet)",
                )
                .unwrap();

            let mark = interp.specbind_depth();
            interp.specbind(variable, Value::Int(42)).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| interp.unbind_to(mark)));

            assert!(
                result.is_ok(),
                "a watcher-created buffer borrow must not panic during unwind"
            );
            assert!(result.unwrap().is_err());
            assert_eq!(
                interp.specbind_depth(),
                mark + 1,
                "keep the unwind record so cleanup can be retried"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
