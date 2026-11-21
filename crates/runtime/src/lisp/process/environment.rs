//! Build a child-local environment from the current Lisp binding.
use crate::lisp::builtins::misc::lisp_string_bytes;
use crate::lisp::eval::Interp;
use crate::lisp::value::Value;
use crate::lisp::{error::Flow, obarray::sym};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::process::Command;
use std::rc::Rc;

pub(crate) fn apply(i: &mut Interp, command: &mut Command) -> Result<(), Flow> {
    // An empty Lisp environment must not inherit the editor's OS
    // environment. Never mutate the latter: dynamic bindings and other
    // interpreters may be spawning children with different environments.
    command.env_clear();
    let mut environment = i
        .q_soft("process-environment")
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    let mut seen_names = HashSet::new();
    let mut seen_cells = HashSet::new();
    loop {
        let Value::Cons(cell) = &environment else {
            break;
        };
        if !seen_cells.insert(Rc::as_ptr(cell) as usize) {
            return Err(i.signal_data(sym::CIRCULAR_LIST, vec![environment.clone()]));
        }
        let (entry, rest) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        // GNU make_environment_block only consumes the initial string
        // prefix of the list. Its C strings end at the first NUL.
        if !matches!(entry, Value::Str(_)) {
            break;
        }
        let bytes = lisp_string_bytes(i, &entry);
        let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
        let separator = bytes.iter().position(|byte| *byte == b'=');
        let name = &bytes[..separator.unwrap_or(bytes.len())];
        // The first occurrence wins, including a bare NAME tombstone.
        if seen_names.insert(name.to_vec()) {
            if let Some(separator) = separator {
                command.env(
                    OsStr::from_bytes(name),
                    OsStr::from_bytes(&bytes[separator + 1..]),
                );
            }
        }
        environment = rest;
    }
    // GNU corrects an existing PWD to the child's working directory,
    // without introducing it when Lisp left it unset.
    let has_pwd = command
        .get_envs()
        .any(|(name, value)| name == "PWD" && value.is_some());
    if has_pwd {
        if let Some(directory) = command.get_current_dir() {
            let mut bytes = directory.as_os_str().as_bytes();
            while bytes.len() > 2 && bytes.ends_with(b"/") {
                bytes = &bytes[..bytes.len() - 1];
            }
            let pwd = OsStr::from_bytes(bytes).to_os_string();
            command.env("PWD", pwd);
        }
    }
    Ok(())
}

#[cfg(test)]
mod circular_environment_tests {
    use super::*;

    #[test]
    fn child_environment_builder_rejects_a_circular_list() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let environment = Value::cons(Value::string("QA_CYCLE=first"), Value::Nil);
                if let Value::Cons(cell) = &environment {
                    cell.borrow_mut().cdr = environment.clone();
                }
                let symbol = interp.intern("process-environment");
                interp.set_symbol(symbol, environment).unwrap();
                let mut command = Command::new("/bin/true");
                let result = apply(&mut interp, &mut command);
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal))
                        if signal.is_symbol(sym::CIRCULAR_LIST)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
