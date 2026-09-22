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

const MAX_CHAR: i128 = 0x3f_ffff;

pub struct Reader<'a> {
    chars: Vec<char>,
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
    Flow::Signal(Value::Sym(sym_id), Value::list(vec![Value::string(msg)]), false)
}

/// `invalid-read-syntax' with a symbol argument, like Emacs's `#|', `#z'.
fn read_err_sym(interp: &mut Interp, name: &str) -> Flow {
    let sym_id = interp.intern("invalid-read-syntax");
    let data = Value::list(vec![Value::Sym(interp.intern(name))]);
    Flow::Signal(Value::Sym(sym_id), data, false)
}

/// `invalid-read-syntax' for radix integers: `(integer, radix N)'.
fn read_err_radix(interp: &mut Interp, radix: u32) -> Flow {
    let sym_id = interp.intern("invalid-read-syntax");
    let data = Value::list(vec![
        Value::Sym(interp.intern("integer,")),
        Value::Sym(interp.intern("radix")),
        Value::Int(radix as i128),
    ]);
    Flow::Signal(Value::Sym(sym_id), data, false)
}

fn eof_err(interp: &mut Interp) -> Flow {
    let sym_id = interp.intern("end-of-file");
    Flow::Signal(Value::Sym(sym_id), Value::Nil, false)
}

impl<'a> Reader<'a> {
    pub fn new(interp: &'a mut Interp, src: &str) -> Reader<'a> {
        Reader {
            chars: src.chars().collect(),
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
                Some('#') if self.peek_at(1) == Some('@') => {
                    // #@N — skip N chars (used by byte-compiled files).
                    self.pos += 2;
                    let mut n: usize = 0;
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            n = n * 10 + (c as usize - '0' as usize);
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    self.pos = (self.pos + n).min(self.chars.len());
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
                let n = self.read_char_literal()?;
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
                    // Emacs: (invalid-read-syntax expected \))
                    let sym_id = self.interp.intern("invalid-read-syntax");
                    let data = Value::list(vec![
                        Value::Sym(self.interp.intern("expected")),
                        Value::Sym(self.interp.intern(")")),
                    ]);
                    return Err(Flow::Signal(Value::Sym(sym_id), data, false));
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
                let n = self.read_radix_digits(16, 8)?;
                Ok(char::from_u32(n as u32))
            }
            Some('u') => {
                let n = self.read_radix_digits(16, 4)?;
                Ok(char::from_u32(n as u32))
            }
            Some('U') => {
                let n = self.read_radix_digits(16, 8)?;
                Ok(char::from_u32(n as u32))
            }
            Some('C') if self.peek() == Some('-') => {
                self.pos += 1;
                let v = self.read_char_literal()?;
                // Strings hold plain chars: fold C- like ?\C-x (control
                // char); a meta bit survives as base+128 (GNU 8-bit char).
                let base =
                    v & !(CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT);
                let mut ch = ctrl_of(char::from_u32(base as u32).unwrap_or('\0')) as i128;
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
                Some(c) => Ok(Some(ctrl_of(c))),
            },
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
                Ok(char::from_u32(n as u32))
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
                    let folded = match base {
                        32 => Some(0),
                        63 => Some(127),
                        b if (64..128).contains(&b) => Some(b & 0x1f),
                        _ => None,
                    };
                    return Ok(match folded {
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
            Some('s') => Ok(32),
            Some('t') => Ok(9),
            Some('v') => Ok(11),
            Some('x') => self.read_radix_digits(16, 8),
            Some('u') => self.read_radix_digits(16, 4),
            Some('U') => self.read_radix_digits(16, 8),
            Some('^') => match self.next() {
                None => Err(eof_err(self.interp)),
                Some(c) => Ok(ctrl_of(c) as i128),
            },
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
                Ok(n)
            }
            Some(c) => Ok(c as i128),
        }
    }

    /// Read hex/octal/etc digits. `max_digits` caps consumption (0 = unbounded).
    fn read_radix_digits(&mut self, radix: u32, max_digits: usize) -> Result<i128, Flow> {
        let mut n: i128 = 0;
        let mut count = 0;
        while let Some(c) = self.peek() {
            if !c.is_digit(radix) {
                break;
            }
            if max_digits != 0 && count >= max_digits {
                break;
            }
            n = n
                .checked_mul(radix as i128)
                .and_then(|x| x.checked_add(c.to_digit(radix).unwrap() as i128))
                .unwrap_or(MAX_CHAR + 1);
            self.pos += 1;
            count += 1;
        }
        if count == 0 {
            return Err(read_err(self.interp, "invalid radix"));
        }
        Ok(n)
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
                // Uninterned symbol.
                let tok = self.read_symbol_token();
                if tok.is_empty() {
                    return Err(read_err(self.interp, "#: without symbol"));
                }
                Ok(Value::Sym(self.interp.make_symbol(&tok)))
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
                    return Err(read_err_sym(self.interp, "#&"));
                }
                let s = self.read_string()?;
                let bytes: Vec<u32> = match &s {
                    Value::Str(s) => s.borrow().chars().map(|c| c as u32).collect(),
                    _ => vec![],
                };
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
            Some('(') => {
                // `#(' is not Emacs read syntax (vectors are `[...]').
                Err(read_err_sym(self.interp, "#"))
            }
            Some('s') => {
                // `#s(...)' — record object.
                self.pos += 2;
                if self.peek() != Some('(') {
                    return Err(read_err_sym(self.interp, "#s "));
                }
                self.pos += 1;
                let items = self.read_seq(')')?;
                Ok(Value::Record(std::rc::Rc::new(std::cell::RefCell::new(
                    items,
                ))))
            }
            Some('_') => {
                // `#_' — read the next token as an interned symbol
                // (symbol-with-position marker in byte-compiled files).
                self.pos += 2;
                let tok = self.read_symbol_token();
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
                                    Err(read_err(self.interp, "#n# undefined"))
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
    fn read_symbol_token(&mut self) -> String {
        let mut tok = String::new();
        while let Some(c) = self.peek() {
            if Self::is_terminator(c) {
                break;
            }
            if c == '\\' {
                self.pos += 1;
                if let Some(e) = self.next() {
                    tok.push(e);
                    continue;
                }
                break;
            }
            tok.push(c);
            self.pos += 1;
        }
        tok
    }

    /// Read a token and classify: number, symbol, or error.
    fn read_atom(&mut self) -> Result<Value, Flow> {
        let start = self.pos;
        let tok = self.read_symbol_token();
        if tok.is_empty() {
            return Err(read_err(self.interp, "empty token"));
        }
        if let Some(v) = parse_number(&tok) {
            return Ok(v);
        }
        match tok.as_str() {
            // `nil' reads as the nil object, not a symbol cell.
            "nil" => return Ok(Value::Nil),
            // `.' is only a symbol when `)' or `]' follows directly.
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

fn ctrl_of(c: char) -> char {
    let n = c as u32;
    char::from_u32(if (64..128).contains(&n) { n & 0x1f } else { n }).unwrap_or(c)
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
        return Some(Value::Float(f));
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
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
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
