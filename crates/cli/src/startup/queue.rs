//! Lazily consume the Lisp-owned command-line queue, preserving operand objects.
use super::with_command_line_frames;
use crate::arguments;
use remacs_runtime::host;
use remacs_runtime::lisp::{EvalResult, Flow, Interp, SymId, Value};

pub(super) enum Argument {
    File(String),
    Option {
        name: &'static str,
        operand: Option<Value>,
    },
}

pub(super) struct CommandLine {
    left: SymId,
    parse_options: bool,
}

impl CommandLine {
    pub(super) fn new(i: &mut Interp, args: &[String]) -> Self {
        let left = host::symbol(i, "command-line-args-left");
        let _ = i.set_symbol_default(left, Value::list(args.iter().map(Value::string).collect()));
        Self {
            left,
            parse_options: true,
        }
    }

    /// Read the live value again after each action. Lisp may consume, replace,
    /// localize or mutate the remaining arguments, including the next operand.
    /// Keep the pre-pop tail for Lisp-visible command-line entry frames.
    pub(super) fn next(&mut self, i: &mut Interp) -> Result<Option<(Argument, Value)>, Flow> {
        loop {
            let rest = i.symbol_value(self.left);
            if rest.is_nil() {
                return Ok(None);
            }
            let token_value = self.pop(i, rest.clone())?;
            let token = argument_text(&token_value);
            if self.parse_options && token == "--" {
                self.parse_options = false;
                continue;
            }
            let option = self
                .parse_options
                .then(|| arguments::normalize_option(&token))
                .flatten();
            let Some(option) = option else {
                return Ok(Some((Argument::File(token), rest)));
            };
            let operand = if let Some(value) = option.value {
                let start = token.chars().count() - value.chars().count();
                Some(
                    host::argument_substring(i, token_value, start)
                        .expect("inline operand is a valid token substring"),
                )
            } else if arguments::takes_operand(&token) {
                Some(self.pop(i, rest.clone())?)
            } else {
                None
            };
            return Ok(Some((
                Argument::Option {
                    name: option.name,
                    operand,
                },
                rest,
            )));
        }
    }

    fn pop(&self, i: &mut Interp, rest: Value) -> EvalResult {
        with_command_line_frames(i, rest, |i| pop_string(i, self.left))
    }
}

pub(super) fn argument_text(value: &Value) -> String {
    let Value::Str(text) = value else {
        unreachable!("command-line arguments have been checked as strings")
    };
    text.borrow().to_unicode_lossy()
}

/// Validate lazily so a bad later element does not prevent earlier actions.
fn pop_string(i: &mut Interp, left: SymId) -> EvalResult {
    let (value, tail) = match i.symbol_value(left) {
        Value::Nil => (Value::Nil, Value::Nil),
        Value::Cons(cell) => {
            let pair = cell.borrow();
            (pair.car.clone(), pair.cdr.clone())
        }
        other => return Err(i.wrong_type_mut("listp", &other)),
    };
    i.set_symbol(left, tail)?;
    if !matches!(&value, Value::Str(_)) {
        return Err(i.wrong_type_mut("stringp", &value));
    }
    Ok(value)
}
