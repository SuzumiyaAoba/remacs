use remacs_runtime::lisp::{Argv, Arity, EvalResult, Interp, Subr, Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

static BORROW_FILE_NAME_ON_LET: Subr = Subr {
    name: "qa-borrow-file-name-on-let",
    arity: Arity::Range { min: 4, max: 4 },
    func: borrow_file_name_on_let,
    doc: "",
};

static BORROW_FILE_NAME_ON_UNLET: Subr = Subr {
    name: "qa-borrow-file-name-on-unlet",
    arity: Arity::Range { min: 4, max: 4 },
    func: borrow_file_name_on_unlet,
    doc: "",
};

static BORROW_FILE_NAME_ON_SET: Subr = Subr {
    name: "qa-borrow-file-name-on-set",
    arity: Arity::Range { min: 4, max: 4 },
    func: borrow_file_name_on_set,
    doc: "",
};

fn borrow_file_name_on_let(interp: &mut Interp, args: Argv) -> EvalResult {
    if matches!(args.get(2), Some(Value::Sym(operation))
        if interp.intern_soft("let") == Some(*operation))
        && let Some(Value::Str(string)) = args.get(1)
    {
        std::mem::forget(string.borrow_mut());
    }
    Ok(Value::Nil)
}

fn borrow_file_name_on_unlet(interp: &mut Interp, args: Argv) -> EvalResult {
    if matches!(args.get(2), Some(Value::Sym(operation))
        if interp.intern_soft("unlet") == Some(*operation))
        && let Some(Value::Str(string)) = args.get(1)
    {
        std::mem::forget(string.borrow_mut());
    }
    Ok(Value::Nil)
}

fn borrow_file_name_on_set(interp: &mut Interp, args: Argv) -> EvalResult {
    if matches!(args.get(2), Some(Value::Sym(operation))
        if interp.intern_soft("set") == Some(*operation))
        && let Some(Value::Str(string)) = args.get(1)
    {
        std::mem::forget(string.borrow_mut());
    }
    Ok(Value::Nil)
}

#[test]
fn borrowed_file_name_from_let_watcher_signals_and_unwinds_without_panicking() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(make-local-variable 'buffer-file-name) \
                     (setq buffer-file-name \"old-name\")",
                )
                .unwrap();
            let watcher = interp.intern("qa-borrow-file-name-on-let");
            let variable = interp.intern("buffer-file-name");
            interp.fset(watcher, Value::Subr(&BORROW_FILE_NAME_ON_LET));
            interp
                .eval_str("(add-variable-watcher 'buffer-file-name 'qa-borrow-file-name-on-let)")
                .unwrap();

            let mark = interp.specbind_depth();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.specbind(variable, Value::string("new-name"))
            }));

            assert!(result.is_ok(), "buffer-file-name binding must not panic");
            assert!(result.unwrap().is_err());
            assert_eq!(interp.specbind_depth(), mark);
            assert!(matches!(
                interp.symbol_value(variable),
                Value::Str(ref value) if value.borrow().as_str() == "old-name"
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_saved_file_name_from_unlet_watcher_keeps_unwind_record() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(make-local-variable 'buffer-file-name) \
                     (setq buffer-file-name \"old-name\")",
                )
                .unwrap();
            let watcher = interp.intern("qa-borrow-file-name-on-unlet");
            let variable = interp.intern("buffer-file-name");
            interp.fset(watcher, Value::Subr(&BORROW_FILE_NAME_ON_UNLET));
            interp
                .eval_str("(add-variable-watcher 'buffer-file-name 'qa-borrow-file-name-on-unlet)")
                .unwrap();

            let mark = interp.specbind_depth();
            interp.specbind(variable, Value::string("new-name")).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| interp.unbind_to(mark)));

            assert!(result.is_ok(), "restoring buffer-file-name must not panic");
            assert!(result.unwrap().is_err());
            assert_eq!(
                interp.specbind_depth(),
                mark + 1,
                "keep the unwind record if a watcher borrows its saved string"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn borrowed_file_name_from_set_watcher_signals_before_writing_local_state() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str(
                    "(make-local-variable 'buffer-file-name) \
                     (setq buffer-file-name \"old-name\")",
                )
                .unwrap();
            let watcher = interp.intern("qa-borrow-file-name-on-set");
            let variable = interp.intern("buffer-file-name");
            interp.fset(watcher, Value::Subr(&BORROW_FILE_NAME_ON_SET));
            interp
                .eval_str("(add-variable-watcher 'buffer-file-name 'qa-borrow-file-name-on-set)")
                .unwrap();

            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.set_symbol(variable, Value::string("new-name"))
            }));

            assert!(result.is_ok(), "setting buffer-file-name must not panic");
            assert!(result.unwrap().is_err());
            assert!(matches!(
                interp.symbol_value(variable),
                Value::Str(ref value) if value.borrow().as_str() == "old-name"
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
