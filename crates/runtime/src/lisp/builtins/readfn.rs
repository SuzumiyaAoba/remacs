//! Reader subrs: read, read-from-string.

use super::S;
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Subr, Value};
use std::rc::Rc;

pub(crate) static SUBRS: &[Subr] = &[
    S!("read", 0, 1, f_read, "Read one Lisp object."),
    S!(
        "read-from-string",
        1,
        3,
        f_read_from_string,
        "Read one object from STRING."
    ),
];

pub(crate) fn default_read_source(i: &mut Interp, source: Value) -> Value {
    if source.is_nil() {
        let input = i.q("standard-input");
        i.symbol_value(input)
    } else {
        source
    }
}

pub(crate) fn is_load_stream(i: &Interp, source: &Value) -> bool {
    i.load_stream_symbol
        .is_some_and(|symbol| i.sym_id(source) == Some(symbol))
}

pub(crate) fn read_parser_at(
    i: &mut Interp,
    chars: Rc<Vec<crate::lisp::text::EmacsChar>>,
    position: usize,
    annotate_pos: Option<i128>,
    from_buffer: bool,
    multibyte: bool,
) -> (Result<Option<Value>, Flow>, usize) {
    read_parser_at_with_codes(
        i,
        chars,
        position,
        annotate_pos,
        from_buffer,
        multibyte,
        None,
    )
}

pub(crate) fn read_parser_at_with_codes(
    i: &mut Interp,
    chars: Rc<Vec<crate::lisp::text::EmacsChar>>,
    position: usize,
    annotate_pos: Option<i128>,
    from_buffer: bool,
    multibyte: bool,
    oversized_codes: Option<crate::lisp::reader::OversizedCodes>,
) -> (Result<Option<Value>, Flow>, usize) {
    let invalid_syntax = i.q("invalid-read-syntax");
    let (result, mut end) = {
        let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
        reader.oversized_codes = oversized_codes.clone();
        reader.set_position(position);
        reader.from_buffer = from_buffer;
        reader.source_multibyte = multibyte;
        reader.annotate_pos = annotate_pos;
        let result = reader.read();
        (result, reader.position().max(position))
    };
    // Diagnostic positions identify a consumed delimiter/hash token.
    // A consuming stream advances past it even when parsing signals.
    if let Err(Flow::Signal(signal)) = &result {
        if signal.is_symbol(invalid_syntax)
            && !oversized_codes
                .as_ref()
                .is_some_and(|codes| codes.contains_key(&end))
        {
            if let Some(Value::Str(message)) = signal
                .data()
                .list_to_vec()
                .ok()
                .and_then(|v| v.first().cloned())
            {
                let message = message.borrow();
                if (message.as_str() == ")"
                    && chars.get(end).is_some_and(|c| *c == ')' || *c == ']'))
                    || (message.as_str() == "." && chars.get(end).is_some_and(|c| *c == '.'))
                    || (message.starts_with('#')
                        && message.chars().last() == chars.get(end).copied())
                    || (message.as_str() == "nonsensical self-reference"
                        && chars.get(end).is_some_and(|c| *c == '#'))
                {
                    end += 1;
                }
            }
        }
    }
    (result, end)
}

pub(crate) fn finish_stream_read(
    i: &mut Interp,
    result: Result<Option<Value>, Flow>,
    eof_context: Option<Value>,
) -> EvalResult {
    match result {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(i.signal_data(sym::END_OF_FILE, eof_context.into_iter().collect())),
        Err(Flow::Signal(mut signal)) if signal.is_symbol(sym::END_OF_FILE) => {
            signal.set_data(Value::list(eof_context.into_iter().collect()));
            Err(Flow::Signal(signal))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn read_buffer_stream(i: &mut Interp, source: &Value, positioning: bool) -> EvalResult {
    let (buffer, marker) = match source {
        Value::Buffer(buffer) => (buffer.clone(), None),
        Value::Marker(marker) => {
            let buffer = marker.borrow().buffer.and_then(|id| i.buffers.get(id));
            let Some(buffer) = buffer else {
                return Err(i.signal_data(sym::END_OF_FILE, vec![]));
            };
            (buffer, Some(marker.clone()))
        }
        _ => unreachable!("buffer or marker stream"),
    };
    let multibyte = buffer_source_multibyte(i, &buffer);
    let (chars, position) = {
        let buffer = buffer.borrow();
        (
            Rc::new(
                buffer
                    .text
                    .substring(0, buffer.text_len())
                    .chars()
                    .collect(),
            ),
            marker
                .as_ref()
                .map(|m| m.borrow().position)
                .unwrap_or_else(|| buffer.point()),
        )
    };
    let base = if marker.is_some() {
        -(position as i128)
    } else {
        1
    };
    let (result, end) = read_parser_at(
        i,
        chars,
        position,
        positioning.then_some(base),
        marker.is_none(),
        multibyte,
    );
    if let Some(marker) = marker {
        marker.borrow_mut().position = end;
        finish_stream_read(i, result, None)
    } else {
        buffer.borrow_mut().set_point(end);
        finish_stream_read(i, result, Some(Value::Buffer(buffer)))
    }
}

pub(crate) fn read_load_stream(i: &mut Interp, positioning: bool) -> EvalResult {
    let Some(stream) = i.load_stream.clone() else {
        return Err(i.signal_data(sym::END_OF_FILE, vec![]));
    };
    let (chars, position, oversized_codes) = {
        let stream = stream.borrow();
        (
            stream.chars.clone(),
            stream.position,
            stream.oversized_codes.clone(),
        )
    };
    let (result, end) = read_parser_at_with_codes(
        i,
        chars,
        position,
        positioning.then_some(-(position as i128)),
        false,
        true,
        oversized_codes,
    );
    stream.borrow_mut().position = end;
    let filename = i.q("load-true-file-name");
    let context = i.symbol_value(filename);
    finish_stream_read(i, result, Some(context))
}

fn f_read(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    read_source(i, super::arg(&args, 0), false)
}

pub(crate) fn read_source(i: &mut Interp, source: Value, positioning: bool) -> EvalResult {
    let source = default_read_source(i, source);
    let read_char = i.q("read-char");
    if matches!(source, Value::Sym(symbol) if symbol == sym::T || symbol == read_char) {
        let reader = Value::Sym(i.q("read-minibuffer"));
        return i.apply(&reader, vec![Value::string("Lisp expression: ")]);
    }
    if is_load_stream(i, &source) {
        return read_load_stream(i, positioning);
    }
    match &source {
        Value::Str(s) => {
            let src = s.borrow().clone();
            let multibyte = super::strfn::string_multibyte_p(i, &source);
            let (v, _) = i.read_from_string_kind(&src, 0, positioning.then_some(0), multibyte)?;
            Ok(v)
        }
        Value::Buffer(_) | Value::Marker(_) => read_buffer_stream(i, &source, positioning),
        other => {
            // GNU Fread: any other STREAM is FUNCALLed to produce
            // chars — non-functions surface `invalid-function'.
            let result = {
                let mut reader = crate::lisp::reader::Reader::with_function(i, other.clone());
                reader.annotate_pos = positioning.then_some(0);
                reader.read()
            };
            match result {
                Ok(Some(value)) => Ok(value),
                Ok(None) => Err(i.signal_data(sym::END_OF_FILE, vec![])),
                // Preserve signals from the callback, including its EOF data.
                Err(error) => Err(error),
            }
        }
    }
}

fn f_read_from_string(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let multibyte = super::strfn::string_multibyte_p(i, &args[0]);
    let src = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let chars: Vec<crate::lisp::value::EmacsChar> = src.chars().collect();
    let len = chars.len() as i128;
    // START/END are character positions; nil END (or omitted) is the
    // string's end. Out-of-range values signal args-out-of-range.
    let start = match args.get(1) {
        None | Some(Value::Nil) => 0,
        Some(Value::Int(n)) => *n,
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    let end = match args.get(2) {
        None | Some(Value::Nil) => len,
        Some(Value::Int(n)) => *n,
        Some(other) => return Err(i.wrong_type_mut("integerp", other)),
    };
    // Negative START/END count from the end of the string (GNU).
    let mut s = start;
    let mut e = end;
    if s < 0 {
        s += len;
    }
    if e < 0 {
        e += len;
    }
    if s < 0 || s > len || e < s || e > len {
        let eid = i.q("args-out-of-range");
        return Err(i.signal_data(
            eid,
            vec![
                args[0].clone(),
                args.get(1).cloned().unwrap_or(Value::Int(0)),
                args.get(2).cloned().unwrap_or(Value::Nil),
            ],
        ));
    }
    let (start, end) = (s, e);
    let sub: crate::lisp::text::EmacsText = chars[start as usize..end as usize].iter().collect();
    let (v, pos) = i.read_from_string_kind(&sub, 0, None, multibyte)?;
    Ok(Value::cons(v, Value::Int(start + pos as i128)))
}

pub(crate) fn buffer_source_multibyte(
    i: &mut Interp,
    buffer: &crate::lisp::value::BufferRef,
) -> bool {
    let variable = i.q("enable-multibyte-characters");
    buffer
        .borrow()
        .locals
        .get(&variable)
        .unwrap_or(&i.obarray.symbol(variable).value)
        .truthy()
}
