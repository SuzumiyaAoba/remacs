//! The Emacs Lisp reader: text -> `Value` s-expressions.
//!
//! Supports the full elisp read syntax: integers (incl. `#x`/`#o`/`#b`/`#Nr`
//! radix), floats, char literals with modifier bits (`?\C-M-a`), strings,
//! symbols (incl. `#:` uninterned and `:` keywords), dotted pairs, vectors,
//! records (`#s(...)`), `#N=`/`#N#` labels, quote/backquote/comma, `#'`,
//! `;` comments, `#!` shebang comments, `#_` and `#@` char skipping.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::Interp;
use super::error::Flow;
use super::obarray::sym;
use super::value::{SymId, Value};

// Emacs modifier bits for char literals (from src/character.h).
pub const CHAR_ALT: i128 = 0x0040_0000;
pub const CHAR_SUPER: i128 = 0x0080_0000;
pub const CHAR_HYPER: i128 = 0x0100_0000;
pub const CHAR_SHIFT: i128 = 0x0200_0000;
pub const CHAR_CTL: i128 = 0x0400_0000;
pub const CHAR_META: i128 = 0x0800_0000;

pub struct Reader<'a> {
    chars: Rc<Vec<char>>,
    pos: usize,
    interp: &'a mut Interp,
    /// `#N=` labels seen so far (shared structure refs via `#N#`).
    labels: HashMap<u32, Value>,
    /// Labels currently being defined — `#N#` inside a `#N=` object
    /// is a forward (possibly circular) reference.
    pending_labels: Vec<u32>,
    /// Placeholder symbols for pending forward references; replaced
    /// by the real object once its `#N=` finishes reading.
    label_markers: HashMap<u32, SymId>,
    /// `read-positioning-symbols' mode: when `Some(base)', every symbol
    /// token reads as a `symbol-with-pos' record carrying position
    /// `base + token_start' (`read' keeps it off).
    pub annotate_pos: Option<i128>,
}

fn read_err(interp: &mut Interp, msg: &str) -> Flow {
    let sym_id = interp.intern("invalid-read-syntax");
    Flow::Signal(
        Value::Sym(sym_id),
        Value::list(vec![Value::string(msg)]),
        false,
    )
}

/// `invalid-read-syntax' with a symbol argument, like Emacs's `#|', `#z'.
fn read_err_sym(interp: &mut Interp, name: &str) -> Flow {
    let sym_id = interp.intern("invalid-read-syntax");
    let data = Value::list(vec![Value::string(name)]);
    Flow::Signal(Value::Sym(sym_id), data, false)
}

/// `invalid-read-syntax' for radix integers: `(integer, radix N)'.
fn read_err_radix(interp: &mut Interp, radix: u32) -> Flow {
    let sym_id = interp.intern("invalid-read-syntax");
    let data = Value::list(vec![Value::string(format!("integer, radix {radix}"))]);
    Flow::Signal(Value::Sym(sym_id), data, false)
}

fn eof_err(interp: &mut Interp) -> Flow {
    let sym_id = interp.intern("end-of-file");
    Flow::Signal(Value::Sym(sym_id), Value::Nil, false)
}

/// GNU `error ()' inside the reader (bad escapes, bad modifiers) —
/// (error "msg"), distinct from `invalid-read-syntax'.
fn plain_err(interp: &mut Interp, msg: &str) -> Flow {
    let sym_id = interp.intern("error");
    Flow::Signal(
        Value::Sym(sym_id),
        Value::list(vec![Value::string(msg)]),
        false,
    )
}

/// GNU signals a plain `error' when a \u or \U escape exceeds the
/// Unicode range: (error "Non-Unicode character: 0x%x").
fn non_unicode_err(interp: &mut Interp, n: i128) -> Flow {
    let sym_id = interp.intern("error");
    Flow::Signal(
        Value::Sym(sym_id),
        Value::list(vec![Value::string(format!(
            "Non-Unicode character: 0x{n:x}"
        ))]),
        false,
    )
}

impl<'a> Reader<'a> {
    pub fn new(interp: &'a mut Interp, src: &str) -> Reader<'a> {
        Reader::with_chars(interp, Rc::new(src.chars().collect()))
    }

    /// A reader over pre-collected chars — callers that re-read the
    /// same source in a loop share one buffer instead of re-collecting.
    pub fn with_chars(interp: &'a mut Interp, chars: Rc<Vec<char>>) -> Reader<'a> {
        Reader {
            chars,
            pos: 0,
            interp,
            labels: HashMap::new(),
            pending_labels: Vec::new(),
            label_markers: HashMap::new(),
            annotate_pos: None,
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos.min(self.chars.len());
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Skip whitespace and comments. Returns true at EOF.
    fn skip_layout(&mut self) -> Result<bool, Flow> {
        loop {
            match self.peek() {
                None => return Ok(true),
                Some(c) if c.is_whitespace() => {
                    self.pos += 1;
                }
                Some(';') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('#') if self.peek_at(1) == Some('!') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }

                _ => return Ok(false),
            }
        }
    }

    /// Read one object. `None` = clean EOF.
    pub fn read(&mut self) -> Result<Option<Value>, Flow> {
        if self.skip_layout()? {
            return Ok(None);
        }
        Ok(Some(self.read_object()?))
    }

    fn read_object(&mut self) -> Result<Value, Flow> {
        match self.peek() {
            None => Err(eof_err(self.interp)),
            Some('(') => {
                self.pos += 1;
                self.read_list(')')
            }
            Some('[') => {
                self.pos += 1;
                let items = self.read_seq(']')?;
                Ok(Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(items))))
            }
            Some('\'') => {
                self.pos += 1;
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                let obj = self.read_object()?;
                Ok(Value::list(vec![Value::Sym(sym::QUOTE), obj]))
            }
            Some('`') => {
                self.pos += 1;
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                let obj = self.read_object()?;
                Ok(Value::list(vec![Value::Sym(sym::BACKQUOTE), obj]))
            }
            Some(',') => {
                self.pos += 1;
                let which = match self.peek() {
                    Some('@') => {
                        self.pos += 1;
                        sym::COMMA_AT
                    }
                    _ => sym::COMMA,
                };
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                let obj = self.read_object()?;
                Ok(Value::list(vec![Value::Sym(which), obj]))
            }
            Some('"') => {
                self.pos += 1;
                self.read_string()
            }
            Some('?') => {
                self.pos += 1;
                let n = match self.next() {
                    None => return Err(eof_err(self.interp)),
                    // GNU accepts literal `? ' / `?\t' with no
                    // delimiter check (so `(list ? x)' works).
                    Some(' ') => 32,
                    Some('\t') => 9,
                    Some('\\') => {
                        let v = self.read_char_escape()?;
                        self.check_char_delim()?;
                        v
                    }
                    Some(c) => {
                        self.check_char_delim()?;
                        c as i128
                    }
                };
                // GNU folds byte8 chars (0x3FFF80..0x3FFFFF) back to
                // the raw byte value: `?\x80' reads as 128.
                const MODS: i128 =
                    CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
                let base = n & !MODS;
                let n = if (0x3f_ff80..=0x3f_ffff).contains(&base) {
                    (n & MODS) | (base - 0x3f_ff00)
                } else {
                    n
                };
                Ok(Value::Int(n))
            }
            Some('#') => self.read_dispatch(),
            Some(')') | Some(']') => Err(read_err(self.interp, ")")),
            Some('.') if self.at_dot_token() => Err(read_err(self.interp, ".")),
            _ => self.read_atom(),
        }
    }

    /// Read a balanced sequence until `close`, allowing dot syntax in lists.
    fn read_list(&mut self, close: char) -> Result<Value, Flow> {
        let mut items: Vec<Value> = Vec::new();
        let dotted = false;
        loop {
            if self.skip_layout()? {
                return Err(eof_err(self.interp));
            }
            let c = self.peek().unwrap();
            if c == close {
                self.pos += 1;
                let mut tail = Value::Nil;
                for item in items.into_iter().rev() {
                    tail = Value::cons(item, tail);
                }
                return Ok(tail);
            }
            // A `.' directly before the close paren is the symbol `\.'
            // (Emacs reads `(a .)' as (a \.), `[a .]' as [a \.]).
            if c == '.' && self.peek_at(1) == Some(close) {
                self.pos += 1;
                items.push(Value::Sym(self.interp.intern(".")));
                continue;
            }
            if c == '.' && self.at_dot_token() {
                // GNU: a dotting token as the first element is illegal
                // ("." in wrong context → invalid-read-syntax).
                if items.is_empty() {
                    return Err(read_err(self.interp, "."));
                }
                self.pos += 1;
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                // GNU: `(a . )' is invalid — the dot must be followed
                // by an object, not the close paren.
                if self.peek() == Some(close) {
                    return Err(read_err(self.interp, ")"));
                }
                let tail = self.read_object()?;
                if self.skip_layout()? || self.peek() != Some(close) {
                    // Emacs: (invalid-read-syntax "expected )")
                    return Err(read_err(self.interp, "expected )"));
                }
                // A `. nil' tail is just a proper list end.
                let tail = if tail.is_nil() { Value::Nil } else { tail };
                self.pos += 1;
                let mut result = tail;
                for item in items.into_iter().rev() {
                    result = Value::cons(item, result);
                }
                return Ok(result);
            }
            let obj = self.read_object()?;
            // `obj` was parsed as a token; detect a standalone `.` token
            // read as symbol — handled above via at_dot_token.
            let _ = dotted;
            items.push(obj);
        }
    }

    /// True if positioned at a `.` dotting token.  GNU reads a `.`
    /// directly before `)' or `]' as the symbol `\.', and a `.'
    /// followed by any other delimiter (whitespace, quotes, openers) as
    /// the dotted-tail token.
    fn at_dot_token(&self) -> bool {
        debug_assert_eq!(self.peek(), Some('.'));
        match self.peek_at(1) {
            None => true,
            Some(c) => c.is_whitespace() || matches!(c, '(' | '[' | '"' | '\'' | '`' | ',' | ';'),
        }
    }

    /// Read a sequence of objects until `close` (for vectors).
    fn read_seq(&mut self, close: char) -> Result<Vec<Value>, Flow> {
        let mut items = Vec::new();
        loop {
            if self.skip_layout()? {
                return Err(eof_err(self.interp));
            }
            if self.peek() == Some(close) {
                self.pos += 1;
                return Ok(items);
            }
            // `.' directly before `]' reads as the symbol `\.'
            // (Emacs: `[a .]' → [a \.]).
            if self.peek() == Some('.') && self.peek_at(1) == Some(close) {
                self.pos += 1;
                items.push(Value::Sym(self.interp.intern(".")));
                continue;
            }
            items.push(self.read_object()?);
        }
    }

    fn read_string(&mut self) -> Result<Value, Flow> {
        let mut out = String::new();
        let mut multibyte = false;
        loop {
            match self.next() {
                None => return Err(eof_err(self.interp)),
                Some('"') => {
                    let v = Value::string(out);
                    // GNU: a literal is unibyte unless it contains a
                    // decoded source char ≥0x80 or an escape producing
                    // a char >0xFF; \NNN escapes ≤0xFF stay bytes.
                    if multibyte {
                        if let Value::Str(r) = &v {
                            self.interp.mark_multibyte(r);
                        }
                    } else if let Value::Str(r) = &v {
                        self.interp.mark_unibyte(r);
                    }
                    return Ok(v);
                }
                Some('\\') => {
                    let c = self.read_string_escape()?;
                    if let Some(c) = c {
                        if (c as u32) > 0xFF {
                            multibyte = true;
                        }
                        out.push(c);
                    }
                }
                Some(c) => {
                    if (c as u32) >= 0x80 {
                        multibyte = true;
                    }
                    out.push(c);
                }
            }
        }
    }

    /// One escape inside a string. Returns None for `\<newline>` continuation.
    fn read_string_escape(&mut self) -> Result<Option<char>, Flow> {
        match self.next() {
            None => Err(eof_err(self.interp)),
            Some('\n') => Ok(None),
            Some(' ') => Ok(Some(' ')),
            Some('a') => Ok(Some('\x07')),
            Some('b') => Ok(Some('\x08')),
            Some('d') => Ok(Some('\x7f')),
            Some('e') => Ok(Some('\x1b')),
            Some('f') => Ok(Some('\x0c')),
            Some('n') => Ok(Some('\n')),
            Some('r') => Ok(Some('\r')),
            Some('s') => Ok(Some(' ')),
            Some('t') => Ok(Some('\t')),
            Some('v') => Ok(Some('\x0b')),
            Some('x') => {
                let n = self.read_hex_char_escape()?;
                if n & (CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT) != 0
                {
                    return Err(read_err(self.interp, "Invalid modifier in string"));
                }
                Ok(crate::lisp::value::lisp_char(n as u32))
            }
            Some('u') => {
                let n = self.read_exact_hex(4)?;
                Ok(char::from_u32(n as u32))
            }
            Some('U') => {
                let n = self.read_exact_hex(8)?;
                if n > 0x10_ffff {
                    return Err(non_unicode_err(self.interp, n));
                }
                Ok(char::from_u32(n as u32))
            }
            Some('N') => {
                let n = self.read_named_char()?;
                Ok(crate::lisp::value::lisp_char(n as u32))
            }
            Some('C') if self.peek() == Some('-') => {
                self.pos += 1;
                let v = self.read_char_literal()?;
                // Strings hold plain chars: fold C- like ?\C-x (control
                // char); a meta bit survives as base+128 (GNU 8-bit char).
                let base =
                    v & !(CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT);
                // GNU allows \C-SPC / \^SPC as a literal NUL (bug#55738).
                let mut ch = if base == 32 && v & CHAR_CTL != 0 {
                    0
                } else {
                    match ctrl_fold(base) {
                        Some(f) => f,
                        // Non-foldable \C-x keeps CHAR_CTL — a modifier
                        // GNU rejects inside strings.
                        None => return Err(read_err(self.interp, "Invalid modifier in string")),
                    }
                };
                if v & CHAR_META != 0 && ch < 0x80 {
                    ch |= 0x80;
                }
                Ok(char::from_u32(ch as u32))
            }
            Some('M') if self.peek() == Some('-') => {
                self.pos += 1;
                let v = self.read_char_literal()?;
                // Meta chars live in strings as base+128 (like GNU's
                // 8-bit metafied chars), so key lookup sees M-x.
                let base =
                    v & !(CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT);
                Ok(char::from_u32(if base < 0x80 {
                    base as u32 | 0x80
                } else {
                    base as u32
                }))
            }
            Some('^') => match self.next() {
                None => Err(eof_err(self.interp)),
                Some(' ') => Ok(Some('\0')),
                Some(c) => match ctrl_fold(c as i128) {
                    Some(f) => Ok(char::from_u32(f as u32)),
                    None => Err(read_err(self.interp, "Invalid modifier in string")),
                },
            },
            Some(c @ ('C' | 'M' | 'S' | 'H' | 'A')) => Err(read_err(
                self.interp,
                &format!("Invalid escape char syntax: \\{c} not followed by -"),
            )),
            Some(c) if c.is_digit(8) => {
                let mut n = (c as i128) - ('0' as i128);
                for _ in 0..2 {
                    match self.peek() {
                        Some(d) if d.is_digit(8) => {
                            n = n * 8 + (d as i128 - '0' as i128);
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                // GNU: octal escapes in 0x80..0xFF are raw bytes.
                if (0x80..0x100).contains(&n) {
                    n = 0x3F_FF00 + n;
                }
                Ok(crate::lisp::value::lisp_char(n as u32))
            }
            Some(c) => Ok(Some(c)),
        }
    }

    /// Read a char literal after `?` or `\` inside a string modifier.
    /// Returns the char code with modifier bits.
    fn read_char_literal(&mut self) -> Result<i128, Flow> {
        match self.next() {
            None => Err(eof_err(self.interp)),
            Some('\\') => self.read_char_escape(),
            Some(c) => Ok(c as i128),
        }
    }

    /// GNU requires the char after a `?' literal to terminate it:
    /// EOF, a control/space char (code ≤ 32), or one of
    /// `"' ; ( ) [ ] # ? ` , .'.
    fn check_char_delim(&mut self) -> Result<(), Flow> {
        match self.peek() {
            None => Ok(()),
            Some(c) if (c as u32) <= 32 => Ok(()),
            Some('"' | '\'' | ';' | '(' | ')' | '[' | ']' | '#' | '?' | '`' | ',' | '.') => Ok(()),
            _ => Err(read_err(self.interp, "?")),
        }
    }

    /// GNU `\x' escape: one or more hex digits. With 1–2 digits,
    /// values ≥ 0x80 become byte8 chars (BYTE8_TO_CHAR → 0x3FFFxx).
    /// Larger values may carry modifier bits.
    fn read_hex_char_escape(&mut self) -> Result<i128, Flow> {
        let mut val: i128 = 0;
        let mut count = 0usize;
        while let Some(c) = self.peek() {
            match c.to_digit(16) {
                Some(d) => {
                    val = val
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(d as i128))
                        .unwrap_or(0x1000_0000);
                    // GNU caps at CHAR_META | (CHAR_META - 1) = 0xFFFFFFF.
                    if val > CHAR_META | (CHAR_META - 1) {
                        return Err(plain_err(
                            self.interp,
                            &format!("Hex character out of range: \\x{val:x}"),
                        ));
                    }
                    self.pos += 1;
                    if count < 3 {
                        count += 1;
                    }
                }
                None => break,
            }
        }
        if count == 0 {
            return Err(plain_err(
                self.interp,
                "Invalid escape char syntax: \\x not followed by hex digit",
            ));
        }
        if count < 3 && val >= 0x80 {
            val = 0x3F_FF00 + val;
        }
        Ok(val)
    }

    /// GNU `\u'/`\U' escapes need exactly N hex digits; a non-hex
    /// character or EOF is an error.
    fn read_exact_hex(&mut self, n: usize) -> Result<i128, Flow> {
        let mut v: i128 = 0;
        for _ in 0..n {
            match self.next() {
                None => {
                    return Err(plain_err(
                        self.interp,
                        &format!("Malformed Unicode escape: \\u{v:x}"),
                    ));
                }
                Some(c) => match c.to_digit(16) {
                    Some(d) => v = v * 16 + d as i128,
                    None => {
                        return Err(plain_err(
                            self.interp,
                            &format!("Non-hex character used for Unicode escape: {c}"),
                        ));
                    }
                },
            }
        }
        Ok(v)
    }

    /// GNU `\N{name}' — Unicode char name (whitespace normalized to a
    /// single space) or `U+XXXX'.
    fn read_named_char(&mut self) -> Result<i128, Flow> {
        if self.next() != Some('{') {
            return Err(read_err(self.interp, "Expected opening brace after \\N"));
        }
        let mut name = String::new();
        let mut ws = false;
        loop {
            match self.next() {
                None => return Err(eof_err(self.interp)),
                Some('}') => break,
                Some(c) if (c as u32) >= 0x80 => {
                    return Err(read_err(
                        self.interp,
                        &format!("Invalid character U+{:04X} in character name", c as u32),
                    ));
                }
                Some(c) if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') => {
                    if !ws {
                        name.push(' ');
                    }
                    ws = true;
                }
                Some(c) => {
                    ws = false;
                    name.push(c);
                }
            }
        }
        if name.is_empty() {
            return Err(read_err(self.interp, "Empty character name"));
        }
        let code: Option<i128> = if name.len() > 2 && name[..2].eq_ignore_ascii_case("u+") {
            i128::from_str_radix(&name[2..], 16).ok()
        } else {
            unicode_names2::character(&name).map(|c| c as i128)
        };
        match code {
            Some(n) if (0..=0x10_ffff).contains(&n) && !(0xd800..0xe000).contains(&n) => Ok(n),
            _ => Err(read_err(self.interp, &format!("\\N{{{name}}}"))),
        }
    }

    /// Char literal after `?\`.
    fn read_char_escape(&mut self) -> Result<i128, Flow> {
        match self.next() {
            None => Err(eof_err(self.interp)),
            // Modifier prefixes must come first: `?\C-a`, `?\M-x`, `?\s-k`.
            Some(c @ ('C' | 'M' | 'S' | 'H' | 'A' | 's')) if self.peek() == Some('-') => {
                self.pos += 1;
                let bit = match c {
                    'C' => CHAR_CTL,
                    'M' => CHAR_META,
                    'S' => CHAR_SHIFT,
                    'H' => CHAR_HYPER,
                    'A' => CHAR_ALT,
                    's' => CHAR_SUPER,
                    _ => unreachable!(),
                };
                let inner = self.read_char_literal()?;
                // `C-' folds the base char to a control character when
                // possible instead of setting the control bit:
                // ?\C-a == 1, ?\C-@ == 0, ?\C-? == 127. Chars that can't
                // fold (e.g. digits, multibyte) keep CHAR_CTL.
                if bit == CHAR_CTL {
                    const ALL_MODS: i128 =
                        CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
                    let base = inner & !ALL_MODS;
                    let mods = inner & (ALL_MODS & !CHAR_CTL);
                    return Ok(match ctrl_fold(base) {
                        Some(c) => mods | c,
                        None => mods | CHAR_CTL | base,
                    });
                }
                Ok(inner | bit)
            }
            Some('a') => Ok(7),
            Some('b') => Ok(8),
            Some('d') => Ok(127),
            Some('e') => Ok(27),
            Some('f') => Ok(12),
            Some('n') => Ok(10),
            Some('r') => Ok(13),
            // `\s' not followed by `-' is just a space (GNU).
            Some('s') => Ok(32),
            Some('t') => Ok(9),
            Some('v') => Ok(11),
            Some('x') => self.read_hex_char_escape(),
            Some('u') => {
                let n = self.read_exact_hex(4)?;
                if n > 0x10_ffff {
                    return Err(non_unicode_err(self.interp, n));
                }
                Ok(n)
            }
            Some('U') => {
                let n = self.read_exact_hex(8)?;
                if n > 0x10_ffff {
                    return Err(non_unicode_err(self.interp, n));
                }
                Ok(n)
            }
            Some('N') => self.read_named_char(),
            Some('^') => {
                let inner = match self.next() {
                    None => return Err(eof_err(self.interp)),
                    Some('\\') => self.read_char_escape()?,
                    Some(c) => c as i128,
                };
                const ALL_MODS: i128 =
                    CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
                let mods = inner & (ALL_MODS & !CHAR_CTL);
                let base = inner & !ALL_MODS;
                Ok(mods | ctrl_fold(base).unwrap_or(CHAR_CTL | base))
            }
            Some('\n') => Err(plain_err(
                self.interp,
                "Invalid escape char syntax: \\<newline>",
            )),
            Some(c @ ('C' | 'M' | 'S' | 'H' | 'A')) => Err(plain_err(
                self.interp,
                &format!("Invalid escape char syntax: \\{c} not followed by -"),
            )),
            Some(c) if c.is_digit(8) => {
                let mut n = (c as i128) - ('0' as i128);
                for _ in 0..2 {
                    match self.peek() {
                        Some(d) if d.is_digit(8) => {
                            n = n * 8 + (d as i128 - '0' as i128);
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                // GNU: octal escapes in 0x80..0xFF are raw bytes.
                if (0x80..0x100).contains(&n) {
                    n = 0x3F_FF00 + n;
                }
                Ok(n)
            }
            Some(c) => Ok(c as i128),
        }
    }

    /// `#`-dispatch read.
    fn read_dispatch(&mut self) -> Result<Value, Flow> {
        debug_assert_eq!(self.peek(), Some('#'));
        match self.peek_at(1) {
            Some('\'') => {
                self.pos += 2;
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                let obj = self.read_object()?;
                Ok(Value::list(vec![Value::Sym(sym::FUNCTION), obj]))
            }
            Some(':') => {
                self.pos += 2;
                // Uninterned symbol; GNU accepts an empty name (`#:'),
                // which reads back as `##'.
                let (tok, _) = self.read_symbol_token();
                Ok(Value::Sym(self.interp.make_symbol(&tok)))
            }
            Some('#') => {
                // `##' — the interned empty-name symbol (this is its
                // print representation), used e.g. in `declare-function'
                // arglists to mean "unknown signature".
                self.pos += 2;
                Ok(Value::Sym(self.interp.intern("")))
            }
            Some('&') => {
                // `#&N"..."' — bool vector literal: N bits packed
                // LSB-first within each string byte.
                self.pos += 2;
                let mut n: u32 = 0;
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        n = n * 10 + d.to_digit(10).unwrap();
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if self.next() != Some('"') {
                    return Err(read_err(self.interp, "#&"));
                }
                let s = self.read_string()?;
                let bytes: Vec<u32> = match &s {
                    Value::Str(s) => {
                        // GNU rejects multibyte strings; raw bytes
                        // (eight-bit proxies) count as their byte value.
                        let mut v: Vec<u32> = Vec::new();
                        for c in s.borrow().chars() {
                            match crate::lisp::value::eight_bit_byte(c) {
                                Some(b) => v.push(b as u32),
                                None if (c as u32) < 0x80 => v.push(c as u32),
                                None => {
                                    return Err(read_err(self.interp, "#&..."));
                                }
                            }
                        }
                        v
                    }
                    _ => vec![],
                };
                // GNU requires SCHARS == ceil(N/8); it also accepts the
                // Emacs 19 form where N counts the string's total bits
                // minus the last byte: N == (SCHARS-1)*8.
                let schars = bytes.len();
                if !(schars == (n as usize + 7) / 8
                    || (schars >= 1 && n as usize == (schars - 1) * 8))
                {
                    return Err(read_err(self.interp, "#&..."));
                }
                let mut bits = Vec::with_capacity(n as usize);
                for k in 0..n as usize {
                    let byte = bytes.get(k / 8).copied().unwrap_or(0);
                    bits.push(byte >> (k % 8) & 1 != 0);
                }
                Ok(crate::lisp::builtins::misc::make_bool_vector(
                    self.interp,
                    bits,
                ))
            }
            Some('@') => {
                // `#@NNN' — used by .elc files to skip lazy doc
                // strings/byte-code.  For non-file sources GNU skips
                // to the next \037 byte (or EOF), then re-reads.
                // `#@00' skips to EOF and yields nil.
                self.pos += 2;
                let mut n: usize = 0;
                let mut digits = 0;
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        n = n * 10 + (c as usize - '0' as usize);
                        self.pos += 1;
                        digits += 1;
                    } else {
                        break;
                    }
                }
                if digits == 2 && n == 0 {
                    self.pos = self.chars.len();
                    return Ok(Value::Nil);
                }
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == '\u{1f}' {
                        break;
                    }
                }
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                self.read_object()
            }
            Some('(') => {
                // `#("str" BEG END (plist) ...)' — propertized string
                // literal; after the string each (beg end plist)
                // triple attaches properties to that char range.
                self.pos += 2;
                let items = self.read_seq(')')?;
                match items.first() {
                    Some(Value::Str(s)) => {
                        let s = s.clone();
                        let mut ivs: Vec<(usize, usize, Vec<Value>)> = Vec::new();
                        let rest = &items[1..];
                        if rest.len() % 3 != 0 {
                            return Err(read_err_sym(self.interp, "#"));
                        }
                        for t in rest.chunks(3) {
                            let (Value::Int(b0), Value::Int(e0)) = (&t[0], &t[1]) else {
                                return Err(read_err_sym(self.interp, "#"));
                            };
                            let pl = t[2].list_to_vec().unwrap_or_default();
                            ivs.push((*b0 as usize, *e0 as usize, pl));
                        }
                        if !ivs.is_empty() {
                            self.interp.set_str_props(&s, ivs);
                        }
                        Ok(Value::Str(s))
                    }
                    _ => Err(read_err_sym(self.interp, "#")),
                }
            }
            Some('^') => {
                // `#^[DEFALT PARENT PURPOSE ASCII S1..S64 EXTRAS...]'
                // — char-table literal (the inverse of the printer's
                // `#^' format); `#^^[DEPTH MIN-CHAR S0..SN]' is a
                // sub char-table.  Both land in the Record layout
                // used by misc::char_table_vec.
                self.pos += 2;
                let sub = self.peek() == Some('^');
                if sub {
                    self.pos += 1;
                }
                if self.next() != Some('[') {
                    return Err(read_err_sym(self.interp, "#^"));
                }
                let items = self.read_seq(']')?;
                if sub {
                    let mut rec = Vec::with_capacity(items.len() + 1);
                    rec.push(Value::Sym(self.interp.intern("sub-char-table")));
                    rec.extend(items);
                    return Ok(Value::Record(Rc::new(std::cell::RefCell::new(rec))));
                }
                if items.len() < 3 {
                    return Err(read_err_sym(self.interp, "#^"));
                }
                let defalt = items[0].clone();
                let parent = items[1].clone();
                let purpose = items[2].clone();
                let mut slots: Vec<Value> = items.iter().skip(3).take(65).cloned().collect();
                slots.resize(65, Value::Nil);
                let mut rec = vec![
                    Value::Sym(self.interp.intern("char-table")),
                    purpose,
                    Value::Vec(Rc::new(std::cell::RefCell::new(slots))),
                ];
                rec.extend(items.iter().skip(68).cloned());
                let t = Value::Record(Rc::new(std::cell::RefCell::new(rec)));
                if !defalt.is_nil() {
                    self.interp.set_char_table_defalt(&t, defalt);
                }
                if !parent.is_nil() {
                    self.interp.set_char_table_parent(&t, parent);
                }
                Ok(t)
            }
            Some('[') => {
                // `#[ARGLIST BODY ENV]' — a function object.  GNU reads
                // byte-code here; ours is an interpreted Lambda.  Like
                // GNU we reject the degenerate shapes `#[]' and `#[x]'
                // (the first element must be a list or nil arglist).
                self.pos += 2;
                let items = self.read_seq(']')?;
                match items.first() {
                    None => return Err(read_err_sym(self.interp, "Invalid byte-code object")),
                    // GNU also accepts an integer arglist — the
                    // compact `args & 0x7ff...' encoding used by the
                    // byte-compiler's own constant vectors.
                    Some(v) if !v.is_nil() && !matches!(v, Value::Cons(_) | Value::Int(_)) => {
                        return Err(read_err_sym(self.interp, "Invalid byte-code object"));
                    }
                    _ => {}
                }
                let arglist = items.first().cloned().unwrap_or(Value::Nil);
                let body = items.get(1).cloned().unwrap_or(Value::Nil);
                let env = items.get(2).cloned().unwrap_or(Value::Nil);
                // `#[... nil]' prints with a `nil' env (plain lambda);
                // `#[... (t)]' is a top-level dynamic function.
                let plain = env.is_nil();
                Ok(crate::lisp::builtins::misc::make_interpreted_closure(
                    self.interp,
                    &arglist,
                    &body,
                    &env,
                    plain,
                    Some(items),
                ))
            }
            Some('s') => {
                // `#s(...)' — record object.
                self.pos += 2;
                if self.peek() != Some('(') {
                    return Err(read_err_sym(self.interp, "#s "));
                }
                self.pos += 1;
                let items = self.read_seq(')')?;
                // `#s(TYPE ...)' — the type symbol is required.
                if items.is_empty() {
                    return Err(read_err_sym(self.interp, "#s"));
                }
                Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
                    items,
                ))))
            }
            Some('_') => {
                // `#_' — read the next token as an interned symbol
                // (symbol-with-position marker in byte-compiled files).
                self.pos += 2;
                let (tok, _) = self.read_symbol_token();
                Ok(Value::Sym(self.interp.intern(&tok)))
            }
            Some('|') => Err(read_err_sym(self.interp, "#|")),
            Some('<') => Err(read_err_sym(self.interp, "#<")),
            Some('$') => {
                // #$ — the name of the file being loaded
                // (Emacs substitutes load-file-name, nil outside load).
                self.pos += 2;
                let name = self
                    .interp
                    .intern_soft("load-file-name")
                    .filter(|id| self.interp.bound_p(*id))
                    .map(|id| self.interp.symbol_value(id));
                match name {
                    Some(Value::Str(_)) => Ok(name.unwrap()),
                    _ => Ok(Value::Nil),
                }
            }
            Some('x') | Some('X') => {
                self.pos += 2;
                let n = self.read_radix_int(16)?;
                Ok(Value::Int(n))
            }
            Some('o') | Some('O') => {
                self.pos += 2;
                let n = self.read_radix_int(8)?;
                Ok(Value::Int(n))
            }
            Some('b') | Some('B') => {
                self.pos += 2;
                let n = self.read_radix_int(2)?;
                Ok(Value::Int(n))
            }
            Some(c) if c.is_ascii_digit() => {
                // #Nr radix, #N= label, #N# label reference.
                self.pos += 1;
                let mut n: u32 = 0;
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        n = n * 10 + d.to_digit(10).unwrap();
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                match self.next() {
                    Some('r') | Some('R') => {
                        let v = self.read_radix_int(n)?;
                        Ok(Value::Int(v))
                    }
                    Some('=') => {
                        // #N= — label the next object; #N# inside it
                        // resolves to a placeholder patched afterwards.
                        if self.skip_layout()? {
                            return Err(eof_err(self.interp));
                        }
                        self.pending_labels.push(n);
                        let obj = self.read_object();
                        self.pending_labels.pop();
                        let obj = obj?;
                        // `#N=#N#' — the labelled object is nothing but
                        // the placeholder itself; GNU rejects it.
                        if let Value::Sym(s) = &obj {
                            if self.label_markers.get(&n) == Some(s) {
                                return Err(read_err(self.interp, "nonsensical self-reference"));
                            }
                        }
                        self.patch_label(n, &obj);
                        self.labels.insert(n, obj.clone());
                        Ok(obj)
                    }
                    Some('#') => {
                        // #N# — reference a labeled object.
                        match self.labels.get(&n) {
                            Some(v) => Ok(v.clone()),
                            None => {
                                if self.pending_labels.contains(&n) {
                                    if !self.label_markers.contains_key(&n) {
                                        let m = self.interp.make_symbol(&format!("#label{}#", n));
                                        self.label_markers.insert(n, m);
                                    }
                                    Ok(Value::Sym(self.label_markers[&n]))
                                } else {
                                    Err(read_err(self.interp, &format!("#{n}#")))
                                }
                            }
                        }
                    }
                    other => {
                        let c = other.unwrap_or(' ');
                        Err(read_err_sym(self.interp, &format!("#{n}{c}")))
                    }
                }
            }
            other => {
                let c = other.unwrap_or(' ');
                Err(read_err_sym(self.interp, &format!("#{c}")))
            }
        }
    }

    /// Replace the placeholder symbol for label `n` inside `obj`
    /// with `obj` itself, producing shared/circular structure.
    fn patch_label(&mut self, n: u32, obj: &Value) {
        let Some(marker) = self.label_markers.get(&n).copied() else {
            return;
        };
        let mut seen: HashSet<usize> = HashSet::new();
        patch_in(obj, marker, obj, &mut seen);
    }

    fn read_radix_int(&mut self, radix: u32) -> Result<i128, Flow> {
        if !(2..=36).contains(&radix) {
            return Err(read_err_radix(self.interp, radix));
        }
        let neg = match self.peek() {
            Some('-') => {
                self.pos += 1;
                true
            }
            Some('+') => {
                self.pos += 1;
                false
            }
            _ => false,
        };
        let mut n: i128 = 0;
        let mut any = false;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() {
                match c.to_digit(radix) {
                    Some(d) => {
                        n = n
                            .checked_mul(radix as i128)
                            .and_then(|x| x.checked_add(d as i128))
                            .unwrap_or(i128::MAX);
                        any = true;
                        self.pos += 1;
                    }
                    None => return Err(read_err_radix(self.interp, radix)),
                }
            } else {
                break;
            }
        }
        if !any {
            return Err(read_err_radix(self.interp, radix));
        }
        Ok(if neg { -n } else { n })
    }

    /// Chars that terminate a symbol/number token.
    /// `#` terminates (expression-prefix syntax); `?` does not
    /// (it's a symbol constituent in elisp).
    fn is_terminator(c: char) -> bool {
        c.is_whitespace()
            || matches!(
                c,
                '(' | ')' | '[' | ']' | '"' | '\'' | '`' | ',' | ';' | '#'
            )
    }

    /// Read a raw token (symbol constituent chars, `\` escapes included).
    /// The returned bool reports whether any `\` escape was consumed, so
    /// `read_atom' can tell `.' (dotting token) apart from `\.' (symbol).
    fn read_symbol_token(&mut self) -> (String, bool) {
        let mut tok = String::new();
        let mut escaped = false;
        while let Some(c) = self.peek() {
            if Self::is_terminator(c) {
                break;
            }
            if c == '\\' {
                self.pos += 1;
                if let Some(e) = self.next() {
                    tok.push(e);
                    escaped = true;
                    continue;
                }
                break;
            }
            tok.push(c);
            self.pos += 1;
        }
        (tok, escaped)
    }

    /// Read a token and classify: number, symbol, or error.
    fn read_atom(&mut self) -> Result<Value, Flow> {
        let start = self.pos;
        let (tok, escaped) = self.read_symbol_token();
        if tok.is_empty() {
            return Err(read_err(self.interp, "empty token"));
        }
        if let Some(v) = parse_number(&tok) {
            return Ok(v);
        }
        match tok.as_str() {
            // `nil' reads as the nil object, not a symbol cell.
            "nil" => return Ok(Value::Nil),
            // `.' is only a symbol when `)' or `]' follows directly;
            // `\.' is always the symbol `.' (GNU reads (a \. b) as a
            // 3-element list whose middle element is the `.' symbol).
            "." if escaped => {}
            "." if matches!(self.peek(), Some(')') | Some(']')) => {}
            "." => return Err(read_err(self.interp, ".")),
            _ => {}
        }
        let sym = self.interp.intern(&tok);
        if let Some(base) = self.annotate_pos {
            return Ok(crate::lisp::builtins::misc::make_symbol_with_pos(
                self.interp,
                sym,
                base + start as i128,
            ));
        }
        Ok(Value::Sym(sym))
    }
}

/// GNU's control-char fold: `@'..`_' and `a'..'z' fold to &0x1f,
/// `?' folds to 127. Anything else keeps the CHAR_CTL modifier.
fn ctrl_fold(base: i128) -> Option<i128> {
    match base {
        63 => Some(127),
        b if (64..96).contains(&b) || (97..123).contains(&b) => Some(b & 0x1f),
        _ => None,
    }
}

/// Try parsing a token as a number (int or float).
/// Returns `Some(value)` if it parses fully as a number.
///
/// Emacs quirk: `1.` reads as *integer* 1, while `1.e3`, `1.5`, `.5`,
/// `1e3` are floats.
pub fn parse_number(tok: &str) -> Option<Value> {
    if tok.is_empty() {
        return None;
    }
    if let Ok(i) = tok.parse::<i128>() {
        return Some(Value::Int(i));
    }
    if let Some(f) = parse_float(tok) {
        return Some(Value::float(f));
    }
    // `123.` (digits + trailing dot, no exponent) => integer.
    if let Some(head) = tok.strip_suffix('.') {
        if !head.is_empty()
            && head
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'+' || b == b'-')
        {
            if let Ok(i) = head.parse::<i128>() {
                return Some(Value::Int(i));
            }
        }
    }
    None
}

fn parse_float(tok: &str) -> Option<f64> {
    let bytes = tok.as_bytes();
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let start_digits = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - start_digits;
    let mut frac_digits = 0usize;
    let mut saw_dot = false;
    if i < bytes.len() && bytes[i] == b'.' {
        saw_dot = true;
        i += 1;
        let fs = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - fs;
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    let mut saw_exp = false;
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        saw_exp = true;
        i += 1;
        let mut exp_sign = None;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            exp_sign = Some(bytes[i]);
            i += 1;
        }
        // GNU's special literals: `[eE]+NaN' and `[eE]+INF' — the
        // exponent sign must be `+', the suffix is case-sensitive.
        if exp_sign == Some(b'+') {
            let rest = &tok[i..];
            if rest == "NaN" || rest == "INF" {
                let neg = tok.starts_with('-');
                return Some(if rest == "NaN" {
                    if neg { -f64::NAN } else { f64::NAN }
                } else if neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                });
            }
        }
        let es = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return None; // `1e` with no exponent digits isn't a number
        }
    }
    if i != bytes.len() {
        return None;
    }
    // `1.` (dot but no fraction digits and no exponent) is an integer in
    // elisp — handled by parse_number's integer fallback.
    if saw_dot && frac_digits == 0 && !saw_exp {
        return None;
    }
    tok.parse::<f64>().ok()
}

/// Replace occurrences of `marker` (a placeholder SymId) inside `v`
/// with `target`, descending into conses/vectors/records. `seen`
/// guards against revisiting cells (matters once the structure is
/// already circular).
fn patch_in(v: &Value, marker: SymId, target: &Value, seen: &mut HashSet<usize>) {
    match v {
        Value::Cons(c) => {
            let ptr = Rc::as_ptr(c) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if matches!(&car, Value::Sym(s) if *s == marker) {
                c.borrow_mut().car = target.clone();
            } else {
                patch_in(&car, marker, target, seen);
            }
            if matches!(&cdr, Value::Sym(s) if *s == marker) {
                c.borrow_mut().cdr = target.clone();
            } else {
                patch_in(&cdr, marker, target, seen);
            }
        }
        Value::Vec(vec) => {
            let ptr = Rc::as_ptr(vec) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let items: Vec<Value> = vec.borrow().clone();
            let mut touched = false;
            let items: Vec<Value> = items
                .into_iter()
                .map(|it| {
                    if matches!(&it, Value::Sym(s) if *s == marker) {
                        touched = true;
                        target.clone()
                    } else {
                        patch_in(&it, marker, target, seen);
                        it
                    }
                })
                .collect();
            if touched {
                *vec.borrow_mut() = items;
            }
        }
        Value::Record(rec) => {
            let ptr = Rc::as_ptr(rec) as usize;
            if !seen.insert(ptr) {
                return;
            }
            let items: Vec<Value> = rec.borrow().clone();
            let mut touched = false;
            let items: Vec<Value> = items
                .into_iter()
                .map(|it| {
                    if matches!(&it, Value::Sym(s) if *s == marker) {
                        touched = true;
                        target.clone()
                    } else {
                        patch_in(&it, marker, target, seen);
                        it
                    }
                })
                .collect();
            if touched {
                *rec.borrow_mut() = items;
            }
        }
        _ => {}
    }
}
