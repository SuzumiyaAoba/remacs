use remacs_runtime::lisp::{Argv, Arity, EvalResult, Interp, Subr, SymId, Value, sym};
use std::rc::Rc;

static HOLD_LATER_BINDING: Subr = Subr {
    name: "qa-hold-later-binding-cell",
    arity: Arity::Range { min: 0, max: 0 },
    func: hold_later_binding_cell,
    doc: "",
};

fn hold_later_binding_cell(interp: &mut Interp, _args: Argv) -> EvalResult {
    let target = interp.intern("qa-let-later-binding-cell");
    if let Value::Cons(cell) = interp.symbol_value(target) {
        // Keep the later binding-list cell borrowed through the watcher call.
        // The next binding read must signal rather than panic.
        std::mem::forget(cell.borrow_mut());
    }
    Ok(Value::Nil)
}

fn borrowed_binding_form(interp: &mut Interp, head: SymId, prefix: &str) -> (Value, SymId) {
    let first_name = format!("{prefix}-first");
    let second_name = format!("{prefix}-second");
    let first = interp.intern(&first_name);
    let second = interp.intern(&second_name);
    let target = interp.intern("qa-let-later-binding-cell");
    let hold = interp.intern("qa-hold-later-binding-cell");
    interp.fset(hold, Value::Subr(&HOLD_LATER_BINDING));
    interp
        .eval_str(&format!(
            "(defvar {first_name} nil) \
             (add-variable-watcher '{first_name} \
               (lambda (_symbol _value operation _where) \
                 (when (eq operation 'let) (qa-hold-later-binding-cell))))"
        ))
        .unwrap();

    let bindings = Value::list(vec![
        Value::list(vec![Value::Sym(first), Value::Int(1)]),
        Value::list(vec![Value::Sym(second), Value::Int(2)]),
    ]);
    let Value::Cons(first_cell) = &bindings else {
        panic!("binding list must be a cons");
    };
    let Value::Cons(second_cell) = first_cell.borrow().cdr.clone() else {
        panic!("second binding cell is missing");
    };
    interp
        .set_symbol(target, Value::Cons(second_cell))
        .unwrap();

    (
        Value::cons(Value::Sym(head), Value::list(vec![bindings, Value::Sym(first)])),
        first,
    )
}

fn borrow_call_argument_spine(interp: &mut Interp, form: &Value) {
    let Value::Cons(form_cell) = form else {
        panic!("call form must be a cons");
    };
    let Value::Cons(args_cell) = form_cell.borrow().cdr.clone() else {
        panic!("call argument spine must be a cons");
    };
    let target = interp.intern("qa-let-later-binding-cell");
    interp
        .set_symbol(target, Value::Cons(args_cell))
        .unwrap();
}

#[test]
fn let_unwinds_earlier_bindings_when_a_later_binding_cell_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let (form, first) = borrowed_binding_form(&mut interp, sym::LET, "qa-let-borrow");
            let mark = interp.specbind_depth();
            assert!(interp.eval(&form).is_err());

            assert_eq!(
                interp.specbind_depth(),
                mark,
                "a failed later binding must unwind earlier dynamic bindings"
            );
            assert!(matches!(interp.symbol_value(first), Value::Nil));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn dynamic_let_star_unwinds_earlier_bindings_when_a_later_cell_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let (form, first) =
                borrowed_binding_form(&mut interp, sym::LET_STAR, "qa-dynamic-let-star");
            let mark = interp.specbind_depth();

            assert!(interp.eval(&form).is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(first), Value::Nil));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn lexical_let_unwinds_earlier_special_bindings_when_a_later_cell_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let root = remacs_runtime::lisp::eval::lexenv_root();
            interp.lexenv = root.clone();
            let (form, first) = borrowed_binding_form(&mut interp, sym::LET, "qa-lex-let");
            let mark = interp.specbind_depth();

            assert!(interp.eval(&form).is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(first), Value::Nil));
            assert!(match (&interp.lexenv, &root) {
                (Some(actual), Some(expected)) => Rc::ptr_eq(actual, expected),
                (None, None) => true,
                _ => false,
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn lexical_let_star_unwinds_bindings_and_lexical_frame_after_a_borrow_error() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let root = remacs_runtime::lisp::eval::lexenv_root();
            interp.lexenv = root.clone();
            let (form, first) =
                borrowed_binding_form(&mut interp, sym::LET_STAR, "qa-lex-let-star");
            let mark = interp.specbind_depth();

            assert!(interp.eval(&form).is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(first), Value::Nil));
            assert!(match (&interp.lexenv, &root) {
                (Some(actual), Some(expected)) => Rc::ptr_eq(actual, expected),
                (None, None) => true,
                _ => false,
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn let_unwinds_bindings_when_the_body_spine_is_borrowed_after_binding() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let (form, first) = borrowed_binding_form(&mut interp, sym::LET, "qa-let-body-spine");
            borrow_call_argument_spine(&mut interp, &form);
            let mark = interp.specbind_depth();

            assert!(interp.eval(&form).is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(first), Value::Nil));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn lexical_let_star_restores_frame_when_the_body_spine_is_borrowed() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let root = remacs_runtime::lisp::eval::lexenv_root();
            interp.lexenv = root.clone();
            let (form, first) =
                borrowed_binding_form(&mut interp, sym::LET_STAR, "qa-lex-let-star-body");
            borrow_call_argument_spine(&mut interp, &form);
            let mark = interp.specbind_depth();

            assert!(interp.eval(&form).is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(interp.symbol_value(first), Value::Nil));
            assert!(match (&interp.lexenv, &root) {
                (Some(actual), Some(expected)) => Rc::ptr_eq(actual, expected),
                (None, None) => true,
                _ => false,
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
