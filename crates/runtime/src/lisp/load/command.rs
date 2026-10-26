//! GNU command-line load-path splice cursor; preserve list-cell identities across options.

use crate::lisp::{Interp, error::EvalResult, value::Value};

/// GNU command-line-1's load-path splice cursor. Preserve the cons identity:
/// Lisp can replace load-path between options, leaving this cursor detached.
#[derive(Default)]
pub struct CommandLineLoadPath {
    splice: Option<crate::lisp::value::ConsRef>,
}

impl CommandLineLoadPath {
    pub fn add(&mut self, i: &mut Interp, directory: Value) -> EvalResult {
        let separator = i.q("path-separator");
        let format = Value::Sym(i.q("format"));
        let pattern = i.apply(
            &format,
            vec![Value::string("\\`%s"), i.symbol_value(separator)],
        )?;
        let matches = Value::Sym(i.q("string-match-p"));
        let append = i
            .apply(&matches, vec![pattern, directory.clone()])?
            .truthy();
        let directory = if append {
            let substring = Value::Sym(i.q("substring"));
            i.apply(&substring, vec![directory, Value::Int(1)])?
        } else {
            directory
        };
        let normalize = Value::Sym(i.q("command-line-normalize-file-name"));
        let directory = i.apply(&normalize, vec![directory])?;
        let expand = Value::Sym(i.q("expand-file-name"));
        let directory = i.apply(&expand, vec![directory])?;
        let load_path = i.q("load-path");
        if append {
            let append_fn = Value::Sym(i.q("append"));
            let paths = i.apply(
                &append_fn,
                vec![i.symbol_value(load_path), Value::list(vec![directory])],
            )?;
            i.set_symbol(load_path, paths.clone())?;
            if self.splice.is_some() {
                // append copied the list: GNU restarts its cursor at the head.
                self.splice = match paths {
                    Value::Cons(cell) => Some(cell),
                    _ => None,
                };
            }
        } else {
            let next = Value::cons(
                directory,
                match &self.splice {
                    Some(cell) => cell.borrow().cdr.clone(),
                    None => i.symbol_value(load_path),
                },
            );
            if let Some(cell) = &self.splice {
                cell.borrow_mut().cdr = next.clone();
            } else {
                i.set_symbol(load_path, next.clone())?;
            }
            self.splice = match next {
                Value::Cons(cell) => Some(cell),
                _ => unreachable!(),
            };
        }
        Ok(Value::Nil)
    }
}
