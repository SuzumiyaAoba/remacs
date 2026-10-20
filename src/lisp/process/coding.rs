//! Process coding selection and conversion state.
use super::*;

#[derive(Default)]
pub(super) struct State {
    pub output: super::super::builtins::coding_stream::Decoder,
    pub error: super::super::builtins::coding_stream::Decoder,
    pub input: super::super::builtins::coding_stream::Encoder,
    pub stderr_name: String,
}

fn variable(i: &mut Interp, name: &str) -> Value {
    let id = i.intern(name);
    i.symbol_value(id)
}

fn pair(value: Value) -> (Value, Value) {
    match value {
        Value::Cons(pair) => {
            let pair = pair.borrow();
            (pair.car.clone(), pair.cdr.clone())
        }
        value => (value.clone(), value),
    }
}

pub(super) fn name(i: &mut Interp, coding: &Value) -> Result<String, Flow> {
    if coding.is_nil() {
        return Ok("nil".into());
    }
    super::super::builtins::misc::check_coding(i, coding)
}

/// An explicit scalar sets both directions. A pipe process preserves nil
/// members of an explicit pair; other processes fill them from defaults.
pub(super) fn constructor(
    i: &mut Interp,
    args: &Value,
    pipe: bool,
) -> Result<(String, String), Flow> {
    let explicit = kw(i, args, ":coding");
    let mut defaults = || -> Result<(Value, Value), Flow> {
        let value = variable(i, "default-process-coding-system");
        if !matches!(value, Value::Cons(_) | Value::Nil) {
            return Err(i.wrong_type_mut("listp", &value));
        }
        let (read, write) = pair(value);
        let override_read = variable(i, "coding-system-for-read");
        let override_write = variable(i, "coding-system-for-write");
        Ok((
            if override_read.is_nil() {
                read
            } else {
                override_read
            },
            if override_write.is_nil() {
                write
            } else {
                override_write
            },
        ))
    };
    let (mut read, mut write) = if explicit.is_nil() {
        defaults()?
    } else {
        pair(explicit.clone())
    };
    if !pipe && !explicit.is_nil() && (read.is_nil() || write.is_nil()) {
        let (default_read, default_write) = defaults()?;
        if read.is_nil() {
            read = default_read;
        }
        if write.is_nil() {
            write = default_write;
        }
    }
    // Validate both before changing any process state or creating OS I/O.
    Ok((name(i, &read)?, name(i, &write)?))
}

pub(super) fn encoding_name(i: &mut Interp, coding: &Value) -> Result<String, Flow> {
    name(i, coding)?;
    let resolved = super::super::builtins::misc::coding_inherit_file_eol(i, coding, &Value::Nil)?;
    name(i, &resolved)
}
