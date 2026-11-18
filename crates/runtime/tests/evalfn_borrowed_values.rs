use remacs_runtime::lisp::{Argv, Arity, EvalResult, Flow, Interp, Subr, Value, sym};
use std::panic::AssertUnwindSafe;

static HOLD_HOOK_LIST: Subr = Subr {
    name: "qa-hold-borrowed-hook-list",
    arity: Arity::Range { min: 0, max: 0 },
    func: hold_hook_list,
    doc: "",
};

fn hold_hook_list(interp: &mut Interp, _args: Argv) -> EvalResult {
    let hook = interp.intern("qa-hook-list-borrowed-by-callback");
    if let Value::Cons(cell) = interp.symbol_value(hook) {
        std::mem::forget(cell.borrow_mut());
    }
    Ok(Value::Nil)
}

#[test]
fn evaluation_error_helpers_report_borrowed_cons_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();

            let error_type = Value::Sym(interp.intern("error-type"));
            let error_object = Value::cons(Value::Sym(sym::ERROR), Value::Nil);
            let Value::Cons(cell) = &error_object else {
                unreachable!();
            };
            let held = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&error_type, vec![error_object.clone()])
            }));
            drop(held);
            let result = result.expect("error-type must not panic on a borrowed condition");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let ignore_error = Value::Sym(interp.intern("ignore-error"));
            let raw_args = Value::cons(Value::Sym(sym::ERROR), Value::Nil);
            let Value::Cons(args_cell) = &raw_args else {
                unreachable!();
            };
            let held = args_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.call_function(&ignore_error, &raw_args, None)
            }));
            drop(held);
            let result = result.expect("ignore-error must not panic on borrowed raw arguments");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            assert!(matches!(
                interp.call_function(&ignore_error, &raw_args, None),
                Ok(Value::Nil)
            ));

            let access_file = Value::Sym(interp.intern("access-file"));
            let filename = Value::string("borrowed-filename");
            let Value::Str(filename_text) = &filename else {
                unreachable!();
            };
            let held = filename_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &access_file,
                    vec![filename.clone(), Value::string("%s")],
                )
            }));
            drop(held);
            let result = result.expect("access-file must not panic on a borrowed filename");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let message = Value::Sym(interp.intern("message"));
            let message_text = Value::string("borrowed message");
            let Value::Str(text) = &message_text else {
                unreachable!();
            };
            let held = text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&message, vec![message_text.clone()])
            }));
            drop(held);
            let result = result.expect("message must not panic on a borrowed string");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let load = Value::Sym(interp.intern("load"));
            let load_name = Value::string("borrowed-library");
            let Value::Str(load_name_text) = &load_name else {
                unreachable!();
            };
            let held = load_name_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&load, vec![load_name.clone()])
            }));
            drop(held);
            let result = result.expect("load must not panic on a borrowed filename");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let locate_library = Value::Sym(interp.intern("locate-library"));
            let library_name = Value::string("borrowed-library");
            let Value::Str(library_name_text) = &library_name else {
                unreachable!();
            };
            let held = library_name_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&locate_library, vec![library_name.clone()])
            }));
            drop(held);
            let result = result.expect("locate-library must not panic on a borrowed name");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let suffix = Value::string(".el");
            let suffix_var = interp.intern("load-file-rep-suffixes");
            interp
                .set_symbol(suffix_var, Value::list(vec![suffix.clone()]))
                .unwrap();
            let Value::Str(suffix_text) = &suffix else {
                unreachable!();
            };
            let held = suffix_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&locate_library, vec![Value::string("qa-no-such-library")])
            }));
            drop(held);
            let result = result.expect("locate-library must not panic on a borrowed suffix");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let eval_buffer = Value::Sym(interp.intern("eval-buffer"));
            let current_buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer) = &current_buffer else {
                panic!("current-buffer did not return a buffer");
            };
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&eval_buffer, vec![current_buffer.clone()])
            }));
            drop(held);
            let result = result.expect("eval-buffer must not panic on a borrowed buffer");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let source_filename = Value::string("borrowed-source.el");
            let Value::Str(source_text) = &source_filename else {
                unreachable!();
            };
            let held = source_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &eval_buffer,
                    vec![current_buffer.clone(), Value::Nil, source_filename.clone()],
                )
            }));
            drop(held);
            let result = result.expect("eval-buffer must not panic on a borrowed source filename");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let current_buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer) = &current_buffer else {
                panic!("current-buffer did not return a buffer");
            };
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&eval_buffer, Vec::<Value>::new())
            }));
            drop(held);
            let result = result.expect("eval-buffer must not panic on a borrowed current buffer");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let eval_region = Value::Sym(interp.intern("eval-region"));
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&eval_region, vec![Value::Int(1), Value::Int(1)])
            }));
            drop(held);
            let result = result.expect("eval-region must not panic on a borrowed current buffer");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let save_mark = Value::Sym(interp.intern("save-mark-and-excursion--save"));
            let current_buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer) = &current_buffer else {
                panic!("current-buffer did not return a buffer");
            };
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&save_mark, Vec::<Value>::new())
            }));
            drop(held);
            let result = result.expect("save-mark-and-excursion--save must not panic on a borrowed buffer");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let restore_mark = Value::Sym(interp.intern("save-mark-and-excursion--restore"));
            let saved = Value::cons(Value::Nil, Value::Nil);
            let Value::Cons(saved_cell) = &saved else {
                unreachable!();
            };
            let held = saved_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&restore_mark, vec![saved.clone()])
            }));
            drop(held);
            let result = result.expect("mark restoration must not panic on a borrowed saved form");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let saved_marker = interp.eval_str("(copy-marker 1)").unwrap();
            let Value::Marker(marker) = &saved_marker else {
                panic!("copy-marker did not return a marker");
            };
            let saved = Value::cons(saved_marker.clone(), Value::Nil);
            let held = marker.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&restore_mark, vec![saved])
            }));
            drop(held);
            let result = result.expect("mark restoration must not panic on a borrowed marker");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let current_time_zone = Value::Sym(interp.intern("current-time-zone"));
            let zone = Value::string("UTC");
            let Value::Str(zone_text) = &zone else {
                unreachable!();
            };
            let held = zone_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&current_time_zone, vec![Value::Nil, zone.clone()])
            }));
            drop(held);
            let result = result.expect("current-time-zone must not panic on a borrowed string");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let format_time_string = Value::Sym(interp.intern("format-time-string"));
            let format = Value::string("%Y");
            let Value::Str(format_text) = &format else {
                unreachable!();
            };
            let held = format_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&format_time_string, vec![format.clone(), Value::Nil])
            }));
            drop(held);
            let result = result.expect("format-time-string must not panic on a borrowed format");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let autoloadp = Value::Sym(interp.intern("autoloadp"));
            let autoload = Value::cons(
                Value::Sym(interp.intern("autoload")),
                Value::list(vec![Value::string("missing-library")]),
            );
            let Value::Cons(autoload_cell) = &autoload else {
                unreachable!();
            };
            let held = autoload_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&autoloadp, vec![autoload.clone()])
            }));
            drop(held);
            let result = result.expect("autoloadp must not panic on a borrowed cell");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            assert!(matches!(
                interp.apply(&autoloadp, vec![autoload]),
                Ok(Value::Sym(value)) if value == sym::T
            ));

            let autoload_do_load = Value::Sym(interp.intern("autoload-do-load"));
            let autoload_file = Value::string("borrowed-autoload-file");
            let Value::Str(autoload_file_text) = &autoload_file else {
                unreachable!();
            };
            let autoload = Value::list(vec![
                Value::Sym(interp.intern("autoload")),
                autoload_file.clone(),
                Value::Nil,
                Value::Nil,
                Value::Nil,
            ]);
            let held = autoload_file_text.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&autoload_do_load, vec![autoload.clone()])
            }));
            drop(held);
            let result = result.expect("autoload-do-load must not panic on a borrowed file name");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let parse_body = Value::Sym(interp.intern("macroexp-parse-body"));
            let decl = Value::list(vec![
                Value::Sym(interp.intern("declare")),
                Value::Sym(interp.intern("special")),
                Value::Sym(interp.intern("qa-macroexp-parse-body-var")),
            ]);
            let body = Value::list(vec![decl.clone(), Value::Int(1)]);
            let Value::Cons(decl_cell) = &decl else {
                unreachable!();
            };
            let held = decl_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&parse_body, vec![body.clone()])
            }));
            drop(held);
            let result = result.expect("macroexp-parse-body must not panic on a borrowed form");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let advice_add = Value::Sym(interp.intern("advice-add"));
            let identity = Value::Sym(interp.intern("identity"));
            let around = Value::Sym(interp.intern(":around"));
            let ignore = Value::Sym(interp.intern("ignore"));
            let name_property = Value::cons(
                Value::Sym(interp.intern("name")),
                Value::Sym(interp.intern("qa-borrowed-advice-name")),
            );
            let Value::Cons(name_property_cell) = &name_property else {
                unreachable!();
            };
            let props = Value::list(vec![name_property.clone()]);
            let held = name_property_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &advice_add,
                    vec![identity.clone(), around.clone(), ignore.clone(), props.clone()],
                )
            }));
            drop(held);
            let result = result.expect("advice-add must not panic on borrowed properties");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let wrapper = interp.intern("qa-borrowed-advice-wrapper");
            let original_property = interp.intern("cl--advice--orig");
            let original = Value::cons(identity.clone(), Value::Nil);
            interp.put_prop(wrapper, original_property, original.clone());
            let variable = interp.intern("qa-borrowed-advice-variable");
            interp
                .set_symbol(variable, Value::Sym(wrapper))
                .unwrap();
            let place = Value::list(vec![
                Value::Sym(interp.intern("var")),
                Value::Sym(variable),
            ]);
            let remove_function = Value::Sym(interp.intern("cl--remove-function"));
            let Value::Cons(original_cell) = &original else {
                unreachable!();
            };
            let held = original_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&remove_function, vec![place, ignore.clone()])
            }));
            drop(held);
            let result = result.expect("remove-function must not panic on borrowed metadata");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let run_hooks = Value::Sym(interp.intern("run-hooks"));
            let hook = interp.intern("qa-borrowed-hook-list");
            let hook_list = Value::list(vec![Value::Sym(interp.intern("ignore"))]);
            interp.set_symbol(hook, hook_list.clone()).unwrap();
            let Value::Cons(cell) = &hook_list else {
                unreachable!();
            };
            let held = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&run_hooks, vec![Value::Sym(hook)])
            }));
            drop(held);
            let result = result.expect("run-hooks must not panic on a borrowed hook list");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            assert!(matches!(
                interp.apply(&run_hooks, vec![Value::Sym(hook)]),
                Ok(Value::Nil)
            ));

            let add_hook = Value::Sym(interp.intern("add-hook"));
            let local_hook = interp.intern("qa-borrowed-local-hook-variable");
            interp
                .eval_str(
                    "(defvar qa-borrowed-local-hook-variable nil)\
                     (make-local-variable 'qa-borrowed-local-hook-variable)",
                )
                .unwrap();
            let current_buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer) = &current_buffer else {
                panic!("current-buffer did not return a buffer");
            };
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &add_hook,
                    vec![Value::Sym(local_hook), ignore.clone()],
                )
            }));
            drop(held);
            let result = result.expect("add-hook must not panic with a borrowed local-binding map");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));

            let depth_hook = interp.intern("qa-borrowed-hook-depth-variable");
            let depth_symbol = interp.make_symbol("qa-borrowed-hook-depth-alist");
            let depth_function = interp.intern("ignore");
            let depth_pair = Value::cons(Value::Sym(depth_function), Value::Int(5));
            interp
                .set_symbol_default(depth_symbol, Value::list(vec![depth_pair.clone()]))
                .unwrap();
            let depth_property = interp.intern("hook--depth-alist");
            interp.put_prop(depth_hook, depth_property, Value::Sym(depth_symbol));
            let Value::Cons(pair_cell) = &depth_pair else {
                unreachable!();
            };
            let held = pair_cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(
                    &add_hook,
                    vec![
                        Value::Sym(depth_hook),
                        Value::Sym(depth_function),
                        Value::Int(10),
                    ],
                )
            }));
            drop(held);
            let result = result.expect("add-hook must not panic on borrowed depth data");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let normal = interp
                .apply(
                    &add_hook,
                    vec![
                        Value::Sym(depth_hook),
                        Value::Sym(depth_function),
                        Value::Int(10),
                    ],
                )
                .unwrap();
            assert!(matches!(
                normal,
                Value::Cons(_)
            ));

            interp
                .eval_str(
                    "(erase-buffer) (insert \"x\")\
                     (put-text-property 1 2 'modification-hooks '(ignore))",
                )
                .unwrap();
            let hook_list = interp
                .eval_str("(get-text-property 1 'modification-hooks)")
                .unwrap();
            let Value::Cons(cell) = &hook_list else {
                panic!("modification-hooks property is not a list");
            };
            let held = cell.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(delete-region 1 2)")
            }));
            drop(held);
            let result = result.expect("modification hooks must not panic on a borrowed list");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
            let after = interp.eval_str("(buffer-string)").unwrap();
            assert!(matches!(
                &after,
                Value::Str(text) if text.borrow().to_unicode_lossy() == "x"
            ));
            interp.eval_str("(delete-region 1 2)").unwrap();

            let current_buffer = interp.eval_str("(current-buffer)").unwrap();
            let Value::Buffer(buffer) = &current_buffer else {
                panic!("current-buffer did not return a buffer");
            };
            let held = buffer.borrow_mut();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(insert \"x\")")
            }));
            drop(held);
            let result = result.expect("buffer modification must not panic on a borrowed buffer");
            assert!(result.is_err());

            let callback = interp.intern("qa-hold-borrowed-hook-list");
            interp.fset(callback, Value::Subr(&HOLD_HOOK_LIST));
            let callback_hook = interp.intern("qa-hook-list-borrowed-by-callback");
            interp
                .set_symbol(
                    callback_hook,
                    Value::list(vec![Value::Sym(callback)]),
                )
                .unwrap();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                interp.apply(&run_hooks, vec![Value::Sym(callback_hook)])
            }));
            let result = result.expect("run-hooks must not panic after a callback borrows its list");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
