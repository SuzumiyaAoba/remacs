use remacs_runtime::lisp::{Argv, Arity, EvalResult, Flow, Interp, Subr, Value, sym};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

static EVAL_SOURCE_ENTRYPOINTS: Subr = Subr {
    name: "qa-eval-source-entrypoints",
    arity: Arity::Range { min: 0, max: 0 },
    func: eval_source_entrypoints,
    doc: "",
};

fn eval_source_entrypoints(interp: &mut Interp, _args: Argv) -> EvalResult {
    let forms = interp.eval_forms("(function (lambda () nil))")?;
    let first = interp.eval_first_form("(function (lambda () nil))")?;
    Ok(Value::list(vec![forms, first]))
}

#[test]
fn eval_forms_rejects_an_incomplete_final_form() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let marker = interp.intern("qa-eval-forms-prefix-side-effect");
            let result = interp.eval_forms("(setq qa-eval-forms-prefix-side-effect 42)\n(");

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::END_OF_FILE)
            ));
            assert!(matches!(interp.symbol_value(marker), Value::Int(42)));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn eval_forms_accepts_only_trailing_whitespace_and_comments() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            assert!(matches!(
                interp.eval_forms("(+ 1 2) ; trailing comment\n  "),
                Ok(Value::Int(3))
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn eval_first_object_reports_a_mutably_borrowed_string_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let source = Value::string("(+ 1 2)");
            let Value::Str(text) = &source else {
                unreachable!();
            };
            let held = text.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                remacs_runtime::host::eval_first_object(&mut interp, &source)
            }));
            drop(held);

            assert!(result.is_ok(), "borrowed source inspection must not panic");
            assert!(result.unwrap().is_err());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn source_entrypoints_do_not_inherit_compiled_caller_provenance() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let subr = interp.intern("qa-eval-source-entrypoints");
            interp.fset(subr, Value::Subr(&EVAL_SOURCE_ENTRYPOINTS));

            let call = Value::list(vec![Value::Sym(subr)]);
            let mut caller = interp
                .parse_lambda(&Value::Nil, &[call], None)
                .expect("compiled test caller");
            caller.compiled = true;
            let result = interp
                .apply(&Value::Lambda(Rc::new(caller)), Vec::<Value>::new())
                .expect("source entrypoints");

            let closures = result.list_to_vec().expect("two closure results");
            assert_eq!(closures.len(), 2);
            for closure in closures {
                let Value::Lambda(closure) = closure else {
                    panic!("function form did not return a closure");
                };
                assert!(
                    !closure.compiled,
                    "source evaluation inherited its compiled caller's provenance"
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
