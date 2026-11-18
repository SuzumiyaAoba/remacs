use remacs_runtime::lisp::{Argv, Arity, EvalResult, Interp, Subr, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

static RETURN_BORROWED_DOC: Subr = Subr {
    name: "qa-return-borrowed-lambda-doc",
    arity: Arity::Range { min: 0, max: 0 },
    func: return_borrowed_doc,
    doc: "",
};

fn return_borrowed_doc(interp: &mut Interp, _args: Argv) -> EvalResult {
    let doc = interp.intern("qa-computed-lambda-doc");
    if let Value::Str(text) = interp.symbol_value(doc) {
        std::mem::forget(text.borrow_mut());
        return Ok(Value::Str(text));
    }
    Ok(Value::Nil)
}

#[test]
fn lambda_form_docstring_borrow_returns_a_lisp_error_instead_of_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let doc = Value::string("borrowed lambda doc");
            let form = Value::list(vec![
                Value::Sym(interp.intern("lambda")),
                Value::Nil,
                doc.clone(),
                Value::Int(1),
            ]);
            let Value::Str(doc_text) = &doc else {
                unreachable!();
            };
            let held = doc_text.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| interp.lambda_from_form(&form, None)));
            drop(held);

            assert!(result.is_ok(), "lambda docstring inspection must not panic");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn computed_lambda_docstring_borrow_returns_a_lisp_error_instead_of_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let doc = interp.intern("qa-computed-lambda-doc");
            let helper = interp.intern("qa-return-borrowed-lambda-doc");
            interp.set_symbol(doc, Value::string("computed lambda doc")).unwrap();
            interp.fset(helper, Value::Subr(&RETURN_BORROWED_DOC));
            let form = Value::list(vec![
                Value::Sym(interp.intern("lambda")),
                Value::Nil,
                Value::list(vec![
                    Value::Sym(interp.intern(":documentation")),
                    Value::list(vec![Value::Sym(helper)]),
                ]),
                Value::Int(1),
            ]);

            let result = catch_unwind(AssertUnwindSafe(|| interp.lambda_from_form(&form, None)));
            assert!(result.is_ok(), "computed docstring inspection must not panic");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}
