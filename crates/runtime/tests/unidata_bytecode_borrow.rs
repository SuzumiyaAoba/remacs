use remacs_runtime::lisp::{Interp, Value};
use std::panic::AssertUnwindSafe;

fn unidata_marked_bytecode(interp: &mut Interp) -> Value {
    let function = interp
        .eval_str("(make-byte-code 257 (unibyte-string 192 135) [ok] 1)")
        .unwrap();
    let Value::Lambda(lambda) = &function else {
        panic!("make-byte-code did not return a byte-code function");
    };
    let items = lambda.bc_items.as_ref().expect("byte-code slots").clone();
    let mut items = items.borrow_mut();
    items.push(Value::cons(
        Value::string("/generated/unidata-gen.elc"),
        Value::Int(0),
    ));
    drop(items);
    function
}

#[test]
fn unidata_fast_path_does_not_panic_on_borrowed_constants() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let function = unidata_marked_bytecode(&mut interp);
            let Value::Lambda(lambda) = &function else {
                unreachable!();
            };
            let items = lambda.bc_items.as_ref().expect("byte-code slots");
            let constants = match &items.borrow()[2] {
                Value::Vec(constants) => constants.clone(),
                _ => panic!("byte-code constants slot is not a vector"),
            };
            let held = constants.borrow_mut();

            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&function, vec![Value::Int(1)])
            }));
            drop(held);

            assert!(result.is_ok(), "Unicode bytecode dispatch must not panic");
            assert!(result.unwrap().is_err());
            assert!(matches!(
                interp.apply(&function, vec![Value::Int(1)]),
                Ok(Value::Sym(symbol)) if symbol == interp.intern_soft("ok").unwrap()
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
