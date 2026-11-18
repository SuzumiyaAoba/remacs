use remacs_runtime::lisp::{Interp, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn dbus_init_bus_reports_a_mutably_borrowed_address_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let address = Value::string("unix:path=/tmp/remacs-dbus-test");
            let Value::Str(address_string) = &address else {
                unreachable!();
            };
            let init_bus = Value::Sym(interp.intern("dbus--init-bus"));
            assert!(interp
                .eval_str("(fboundp 'dbus--init-bus)")
                .unwrap()
                .truthy());

            let held = address_string.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&init_bus, vec![address.clone()])
            }));
            drop(held);

            assert!(result.is_ok(), "dbus--init-bus must not panic on a borrowed address");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
