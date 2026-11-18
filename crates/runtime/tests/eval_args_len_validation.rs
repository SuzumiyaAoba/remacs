use remacs_runtime::frontend::QuitRequest;
use remacs_runtime::lisp::reader::Reader;
use remacs_runtime::lisp::{Argv, Arity, EvalResult, Flow, Interp, Subr, Value, sym};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

static REQUEST_QUIT_DURING_SPECIAL_FORM: Subr = Subr {
    name: "qa-request-quit-during-special-form",
    arity: Arity::Range { min: 0, max: 0 },
    func: request_quit_during_special_form,
    doc: "",
};

static REQUEST_QUIT_AND_RETURN_TRUE: Subr = Subr {
    name: "qa-request-quit-and-return-true",
    arity: Arity::Range { min: 0, max: 0 },
    func: request_quit_and_return_true,
    doc: "",
};

fn request_quit_during_special_form(interp: &mut Interp, _args: Argv) -> EvalResult {
    if let Some(request) = &interp.quit_request {
        request.request();
    }
    Ok(Value::Nil)
}

fn request_quit_and_return_true(interp: &mut Interp, _args: Argv) -> EvalResult {
    if let Some(request) = &interp.quit_request {
        request.request();
    }
    Ok(Value::t())
}

#[test]
fn eval_args_len_rejects_improper_and_circular_spines_without_trusting_the_hint() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let tail = interp.intern("qa-eval-args-dotted-tail");
            let dotted = Value::cons(Value::Int(1), Value::Sym(tail));

            let direct = interp.eval_args_len(&dotted, 1);
            assert!(matches!(
                direct,
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
            ));

            let list = Value::Sym(interp.intern("list"));
            let call = interp.call_function(&list, &dotted, None);
            assert!(matches!(
                call,
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
            ));

            // The public raw-call helper accepts argument lists too. Special
            // forms must not silently treat a dotted argument spine as a
            // shorter proper list.
            let side_effect = interp.intern("qa-call-function-dotted-special-side-effect");
            interp.set_symbol(side_effect, Value::Nil).unwrap();
            let effect = Value::list(vec![
                Value::Sym(sym::SETQ),
                Value::Sym(side_effect),
                Value::t(),
            ]);
            let special_fun = Value::Sym(sym::PROGN);
            let special_args = Value::cons(effect, Value::Sym(tail));
            let special_call = interp.call_function(&special_fun, &special_args, None);
            assert!(matches!(
                special_call,
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
            ));
            assert!(interp.symbol_value(side_effect).is_nil());

            // A nonempty call resolves its function before rejecting a
            // dotted tail, as ordinary eval does.
            let invalid_call = interp.call_function(
                &Value::Int(5),
                &Value::cons(Value::Int(1), Value::Sym(tail)),
                None,
            );
            assert!(matches!(
                invalid_call,
                Err(Flow::Signal(ref signal))
                    if signal.is_symbol(sym::INVALID_FUNCTION)
            ));

            let circular = Value::cons(Value::Int(1), Value::Nil);
            let Value::Cons(cell) = &circular else {
                unreachable!();
            };
            cell.borrow_mut().cdr = circular.clone();
            let result = interp.eval_args_len(&circular, 1);
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
            ));

            let empty = interp
                .eval_args_len(&Value::Nil, i128::MAX)
                .expect("a capacity hint must not force an oversized allocation");
            assert!(empty.as_slice().is_empty());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn evaluating_a_long_argument_spine_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            // A zero-argument primitive rejects the extra arguments after
            // the evaluator scans their spine, without evaluating their forms.
            let mut args = Value::Nil;
            for _ in 0..1_000_000 {
                args = Value::cons(Value::Nil, args);
            }
            let form = Value::cons(Value::Sym(interp.intern("current-buffer")), args);

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(10));
                input_request.request();
            });

            let result = interp.eval(&form);
            input.join().unwrap();

            assert!(
                matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)),
                "a quit arriving during argument-spine validation must interrupt evaluation"
            );
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn call_function_dispatches_direct_special_subr_objects() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let quote = interp.symbol_function(sym::QUOTE);
            let side_effect = interp.intern("qa-direct-quote-subr-side-effect");
            interp.set_symbol(side_effect, Value::Nil).unwrap();
            let expression = Value::list(vec![
                Value::Sym(sym::SETQ),
                Value::Sym(side_effect),
                Value::t(),
            ]);

            let result = interp
                .call_function(&quote, &Value::list(vec![expression.clone()]), None)
                .expect("call_function should evaluate special subr objects");
            assert!(matches!(
                (&result, &expression),
                (Value::Cons(result), Value::Cons(expected)) if Rc::ptr_eq(result, expected)
            ));
            assert!(interp.symbol_value(side_effect).is_nil());

            let tail = Value::Sym(interp.intern("qa-direct-quote-subr-dotted-tail"));
            let dotted = Value::cons(expression, tail);
            assert!(matches!(
                interp.call_function(&quote, &dotted, None),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
            ));
            assert!(interp.symbol_value(side_effect).is_nil());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn eval_args_len_observes_quit_during_the_argument_cell_snapshot() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));
            let mut args = Value::Nil;
            for _ in 0..2_000_000 {
                args = Value::cons(Value::Nil, args);
            }

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.eval_args_len(&args, 2_000_000);
            input.join().unwrap();

            assert!(
                matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)),
                "snapshotting raw argument cells must remain interruptible"
            );
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn lambda_parameter_binding_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));
            let parameter = interp.intern("qa-long-lambda-parameter");
            let parameter_count = 2_000_000;

            let mut arglist = Value::Nil;
            for _ in 0..parameter_count {
                arglist = Value::cons(Value::Sym(parameter), arglist);
            }
            let function = Value::cons(
                Value::Sym(sym::LAMBDA),
                Value::cons(arglist, Value::list(vec![Value::Nil])),
            );
            let arguments = vec![Value::Nil; parameter_count];
            let binding_mark = interp.specbind_depth();

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.apply(&function, arguments);
            input.join().unwrap();

            assert!(
                matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)),
                "binding a long lambda parameter list must remain interruptible"
            );
            assert_eq!(interp.specbind_depth(), binding_mark);
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn progn_lists_and_body_vectors_observe_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            const FORM_COUNT: usize = 2_000_000;

            let mut forms = Value::Nil;
            for _ in 0..FORM_COUNT {
                forms = Value::cons(Value::Nil, forms);
            }
            let scan_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(scan_request.clone()));
            let input_request = scan_request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                input_request.request();
            });
            let scan_result = interp.eval_progn(&forms);
            input.join().unwrap();
            assert!(matches!(
                scan_result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(scan_request.discard_queued_key());

            let body_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(body_request.clone()));
            let body = vec![Value::Nil; FORM_COUNT];
            let input_request = body_request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                input_request.request();
            });
            let body_result = interp.eval_body(&body);
            input.join().unwrap();
            assert!(matches!(
                body_result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(body_request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn source_evaluation_observes_quit_between_literal_top_level_forms() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let source = "nil\n".repeat(500_000);

            let forms_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(forms_request.clone()));
            let input_request = forms_request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let forms_result = interp.eval_forms(&source);
            input.join().unwrap();
            assert!(matches!(
                forms_result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(forms_request.discard_queued_key());

            let source_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(source_request.clone()));
            let input_request = source_request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let source_result = interp.eval_str(&source);
            input.join().unwrap();
            assert!(matches!(
                source_result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(source_request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn reading_a_long_symbol_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let source = "a".repeat(2_000_000);
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));
            let mut reader = Reader::new(&mut interp, &source);

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = reader.read();
            drop(reader);
            input.join().unwrap();

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn let_star_stops_binding_after_a_quit_before_a_later_invalid_spec() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-long-let-star-variable");
            let trigger = interp.intern("qa-request-quit-during-special-form");
            interp.fset(trigger, Value::Subr(&REQUEST_QUIT_DURING_SPECIAL_FORM));
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let first_binding = Value::list(vec![
                Value::Sym(variable),
                Value::list(vec![Value::Sym(trigger)]),
            ]);
            let mut bindings = Value::list(vec![Value::Int(5)]);
            for _ in 0..200_000 {
                bindings = Value::cons(Value::Sym(variable), bindings);
            }
            bindings = Value::cons(first_binding, bindings);
            let form = Value::cons(
                Value::Sym(sym::LET_STAR),
                Value::list(vec![bindings, Value::Nil]),
            );
            let binding_mark = interp.specbind_depth();

            let result = interp.eval(&form);

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert_eq!(interp.specbind_depth(), binding_mark);
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn let_initializer_scan_polls_quit_before_a_later_invalid_spec() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-long-let-variable");
            let trigger = interp.intern("qa-request-quit-during-special-form");
            interp.fset(trigger, Value::Subr(&REQUEST_QUIT_DURING_SPECIAL_FORM));
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let first_binding = Value::list(vec![
                Value::Sym(variable),
                Value::list(vec![Value::Sym(trigger)]),
            ]);
            let mut bindings = Value::list(vec![Value::Int(5)]);
            for _ in 0..200_000 {
                bindings = Value::cons(Value::Sym(variable), bindings);
            }
            bindings = Value::cons(first_binding, bindings);
            let form = Value::cons(
                Value::Sym(sym::LET),
                Value::list(vec![bindings, Value::Nil]),
            );

            let result = interp.eval(&form);

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn setq_pair_scan_observes_quit_after_an_rhs_without_more_callbacks() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-long-setq-variable");
            let trigger = interp.intern("qa-request-quit-during-special-form");
            interp.fset(trigger, Value::Subr(&REQUEST_QUIT_DURING_SPECIAL_FORM));
            interp.set_symbol(variable, Value::Nil).unwrap();
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let mut args = Value::Nil;
            for _ in 0..100_000 {
                args = Value::cons(Value::Nil, args);
                args = Value::cons(Value::Sym(variable), args);
            }
            args = Value::cons(Value::list(vec![Value::Sym(trigger)]), args);
            args = Value::cons(Value::Sym(variable), args);
            let form = Value::cons(Value::Sym(sym::SETQ), args);

            let result = interp.eval(&form);

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn logical_special_form_loops_observe_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let trigger_nil = interp.intern("qa-request-quit-during-special-form");
            let trigger_true = interp.intern("qa-request-quit-and-return-true");
            interp.fset(trigger_nil, Value::Subr(&REQUEST_QUIT_DURING_SPECIAL_FORM));
            interp.fset(trigger_true, Value::Subr(&REQUEST_QUIT_AND_RETURN_TRUE));

            let and_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(and_request.clone()));
            let mut and_args = Value::Nil;
            for _ in 0..2_048 {
                and_args = Value::cons(Value::t(), and_args);
            }
            and_args = Value::cons(Value::list(vec![Value::Sym(trigger_true)]), and_args);
            let and_form = Value::cons(Value::Sym(sym::AND), and_args);
            assert!(matches!(
                interp.eval(&and_form),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(and_request.discard_queued_key());

            let or_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(or_request.clone()));
            let mut or_args = Value::Nil;
            for _ in 0..2_048 {
                or_args = Value::cons(Value::Nil, or_args);
            }
            or_args = Value::cons(Value::list(vec![Value::Sym(trigger_nil)]), or_args);
            let or_form = Value::cons(Value::Sym(sym::OR), or_args);
            assert!(matches!(
                interp.eval(&or_form),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(or_request.discard_queued_key());

            let cond_request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(cond_request.clone()));
            let mut clauses = Value::Nil;
            for _ in 0..2_048 {
                clauses = Value::cons(Value::list(vec![Value::Nil]), clauses);
            }
            let first_clause = Value::list(vec![Value::list(vec![Value::Sym(trigger_nil)])]);
            clauses = Value::cons(first_clause, clauses);
            let cond_form = Value::cons(Value::Sym(sym::COND), clauses);
            assert!(matches!(
                interp.eval(&cond_form),
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(cond_request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn let_binding_phase_polls_quit_after_all_initializers() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let variable = interp.intern("qa-long-let-binding-variable");
            let trigger = interp.intern("qa-request-quit-during-special-form");
            interp.fset(trigger, Value::Subr(&REQUEST_QUIT_DURING_SPECIAL_FORM));
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let mut bindings = Value::list(vec![Value::list(vec![
                Value::Int(5),
                Value::list(vec![Value::Sym(trigger)]),
            ])]);
            for _ in 0..200_000 {
                bindings = Value::cons(Value::Sym(variable), bindings);
            }
            let form = Value::cons(
                Value::Sym(sym::LET),
                Value::list(vec![bindings, Value::Nil]),
            );
            let binding_mark = interp.specbind_depth();

            let result = interp.eval(&form);

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert_eq!(interp.specbind_depth(), binding_mark);
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn parsing_a_long_lambda_parameter_list_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let parameter = interp.intern("qa-long-parse-lambda-parameter");
            let mut params = Value::Nil;
            for _ in 0..2_000_000 {
                params = Value::cons(Value::Sym(parameter), params);
            }
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.parse_lambda(&params, &[], None);
            input.join().unwrap();

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn defun_arglist_validation_observes_quit_before_a_later_invalid_parameter() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let name = interp.intern("qa-long-defun-parameter-validation");
            let parameter = interp.intern("qa-long-defun-parameter");
            let mut params = Value::list(vec![Value::Int(5)]);
            for _ in 0..200_000 {
                params = Value::cons(Value::Sym(parameter), params);
            }
            let form = Value::cons(
                Value::Sym(sym::DEFUN),
                Value::list(vec![Value::Sym(name), params, Value::Nil]),
            );
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.eval(&form);
            input.join().unwrap();

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn long_function_alias_chains_resolve_in_call_and_apply_paths() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let mut target = interp.intern("current-buffer");
            for index in (0..20_000).rev() {
                let alias = interp.intern(&format!("qa-long-function-alias-{index}"));
                interp.fset(alias, Value::Sym(target));
                target = alias;
            }

            let called = interp
                .call_function(&Value::Sym(target), &Value::Nil, None)
                .expect("the alias chain should resolve to current-buffer");
            assert!(matches!(called, Value::Buffer(_)));

            let applied = interp
                .apply(&Value::Sym(target), Vec::<Value>::new())
                .expect("apply should resolve the same alias chain");
            assert!(matches!(applied, Value::Buffer(_)));

            let indirect_function = interp.intern("indirect-function");
            let resolved = interp
                .apply(&Value::Sym(indirect_function), vec![Value::Sym(target)])
                .expect("indirect-function should resolve the alias chain");
            assert!(matches!(
                resolved,
                Value::Subr(subr) if subr.name == "current-buffer"
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn macroexpand_env_search_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let form = Value::list(vec![Value::Sym(
                interp.intern("qa-macroexpand-long-environment-target"),
            )]);
            let unrelated = Value::Sym(interp.intern("qa-macroexpand-long-environment-other"));
            let entry = Value::cons(unrelated, Value::Nil);
            let mut env = Value::Nil;
            for _ in 0..200_000 {
                env = Value::cons(entry.clone(), env);
            }
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.macroexpand_env(&form, &env);
            input.join().unwrap();

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn rest_argument_list_materialization_observes_host_quit_requests() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let rest = interp.intern("qa-long-rest-parameter");
            let arglist = Value::list(vec![Value::Sym(sym::REST), Value::Sym(rest), Value::Int(5)]);
            let function = Value::cons(
                Value::Sym(sym::LAMBDA),
                Value::cons(arglist, Value::list(vec![Value::Nil])),
            );
            let arguments = vec![Value::Nil; 2_000_000];
            let request = Arc::new(QuitRequest::default());
            interp.set_quit_request(Some(request.clone()));
            let binding_mark = interp.specbind_depth();

            let input_request = request.clone();
            let input = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                input_request.request();
            });
            let result = interp.apply(&function, arguments);
            input.join().unwrap();

            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::QUIT)
            ));
            assert_eq!(interp.specbind_depth(), binding_mark);
            assert!(request.discard_queued_key());
        })
        .unwrap()
        .join()
        .unwrap();
}
