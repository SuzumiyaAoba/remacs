//! GNU lexical-cookie scanning and default-binding callback protocol.

use crate::lisp::{Interp, error::EvalResult, value::Value};

/// Parse GNU lread.c's first-line cookie, including its 99-byte fields
/// and non-overlapping marker state machine. The consumed position is in
/// characters and includes the comment line and any initial shebang.
pub(crate) fn lexical_cookie<T: crate::lisp::text::TextSource + ?Sized>(
    source: &T,
) -> (Option<bool>, usize) {
    source.with_text(|source| {
        fn marker(ch: crate::lisp::text::EmacsChar, state: &mut u8) -> bool {
            match *state {
                0 => *state = u8::from(ch == '-'),
                1 => *state = if ch == '*' { 2 } else { 0 },
                _ => {
                    *state = 0;
                    return ch == '-';
                }
            }
            false
        }
        fn trim(field: &mut Vec<u8>) {
            while matches!(field.last(), Some(b' ' | b'\t')) {
                field.pop();
            }
        }
        fn c_string(field: &[u8]) -> &[u8] {
            field.split(|byte| *byte == 0).next().unwrap_or_default()
        }
        let start = if source.starts_with("#!") {
            source.find('\n').map_or(source.len(), |end| end + 1)
        } else {
            0
        };
        let preceding = source[..start].chars().count();
        let rest = &source[start..];
        if !rest.starts_with(';') {
            return (None, preceding);
        }
        let (line, newline) = rest
            .split_once('\n')
            .map_or((rest, 0), |(line, _)| (line, 1));
        let consumed = preceding + line.chars().count() + newline;
        let mut chars = line.chars().skip(1);
        let mut state = 0;
        let mut in_variables = false;
        for ch in chars.by_ref() {
            if marker(ch, &mut state) {
                in_variables = true;
                break;
            }
        }
        while in_variables {
            let mut ch = chars.next();
            while ch.is_some_and(|c| c == ' ' || c == '\t') {
                ch = chars.next();
            }
            let mut name = Vec::new();
            state = 0;
            while let Some(character) = ch {
                if character == ':' || !in_variables {
                    break;
                }
                if name.len() < 99 {
                    name.push(character.code() as u8);
                }
                if marker(character, &mut state) {
                    in_variables = false;
                }
                ch = chars.next();
            }
            if !in_variables || ch.is_none() {
                break;
            }
            trim(&mut name);
            ch = chars.next();
            while ch.is_some_and(|c| c == ' ' || c == '\t') {
                ch = chars.next();
            }
            let mut value = Vec::new();
            state = 0;
            while let Some(character) = ch {
                if character == ';' || !in_variables {
                    break;
                }
                if value.len() < 99 {
                    value.push(character.code() as u8);
                }
                if marker(character, &mut state) {
                    in_variables = false;
                }
                ch = chars.next();
            }
            if !in_variables {
                value.truncate(value.len().saturating_sub(3));
            }
            trim(&mut value);
            if c_string(&name) == b"lexical-binding" {
                return (Some(c_string(&value) != b"nil"), consumed);
            }
        }
        (None, consumed)
    })
}

#[cfg(test)]
mod wide_cookie_tests {
    use super::lexical_cookie;
    use crate::lisp::text::EmacsText;
    use crate::lisp::*;
    #[test]
    fn consumed_positions_count_extended_characters_once() {
        let mut source = EmacsText::from(";");
        source.append(&EmacsText::from_codes([0xf0080, 0x3fff80, 0xd800, 0x110000]).unwrap());
        source.push_str(" -*- lexical-binding: nil; -*-\n");
        let consumed = source.len();
        source.push_str("(+ 1 2)");
        assert_eq!(lexical_cookie(&source), (Some(false), consumed));
    }
    #[test]
    fn shebang_and_first_declaration_rules_survive_wide_input() {
        let mut source = EmacsText::from("#! /bin/");
        source.append(&EmacsText::from_codes([0x200000]).unwrap());
        source.push_str("\n; -*- lexical-binding: t; lexical-binding: nil; -*-\n");
        let consumed = source.len();
        source.push_str("(message \"done\")");
        assert_eq!(lexical_cookie(&source), (Some(true), consumed));
        assert_eq!(
            lexical_cookie("(+ 1 2) ; -*- lexical-binding: t -*-"),
            (None, 0)
        );
    }
}

/// Resolve a scanned cookie through GNU's observable default callback.
/// FROM=None represents a direct compiled stream, which bypasses that hook.
pub(crate) fn lexical_binding_value(
    i: &mut Interp,
    cookie: Option<bool>,
    from: Option<Value>,
) -> EvalResult {
    if let Some(enabled) = cookie {
        return Ok(Value::from_bool(enabled));
    }
    if let Some(from) = from {
        let hook = i.q("internal--get-default-lexical-binding-function");
        let function = i.symbol_value(hook);
        if !function.is_nil() {
            return i.apply(&function, vec![from]);
        }
    }
    let lexical = i.q("lexical-binding");
    Ok(i.default_toplevel_value(lexical).unwrap_or(Value::Nil))
}
