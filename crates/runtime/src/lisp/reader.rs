//! The Emacs Lisp reader: text -> `Value` s-expressions.
//!
//! Supports the full elisp read syntax: integers (incl. `#x`/`#o`/`#b`/`#Nr`
//! radix), floats, char literals with modifier bits (`?\C-M-a`), strings,
//! symbols (incl. `#:` uninterned and `:` keywords), dotted pairs, vectors,
//! records (`#s(...)`), `#N=`/`#N#` labels, quote/backquote/comma, `#'`,
//! `;` comments, `#!` shebang comments, `#_` and `#@` char skipping.

use super::text::{EmacsChar, EmacsText, TextSource};
use rustc_hash::FxHashMap as HashMap;
use std::rc::Rc;

use super::Interp;
use super::error::Flow;
use super::obarray::sym;
use super::value::Value;

mod circles;
mod dispatch;
mod numbers;
mod quoted;
pub use numbers::parse_number;

// Emacs modifier bits for char literals (from src/character.h).
pub const CHAR_ALT: i128 = 0x0040_0000;
pub const CHAR_SUPER: i128 = 0x0080_0000;
pub const CHAR_HYPER: i128 = 0x0100_0000;
pub const CHAR_SHIFT: i128 = 0x0200_0000;
pub const CHAR_CTL: i128 = 0x0400_0000;
pub const CHAR_META: i128 = 0x0800_0000;

/// Direct-file codes outside the stored character range, indexed by character position.
pub(crate) type OversizedCodes = Rc<HashMap<usize, u32>>;

pub struct Reader<'a> {
    chars: Rc<Vec<EmacsChar>>,
    pub(crate) oversized_codes: Option<OversizedCodes>,
    pos: usize,
    /// Functional input is fetched only when parsing needs another character.
    source: ReadSource,
    interp: &'a mut Interp,
    /// `#N=` labels seen so far (shared structure refs via `#N#`).
    labels: HashMap<u64, Value>,
    /// `read-positioning-symbols' mode: when `Some(base)', every symbol
    /// token reads as a `symbol-with-pos' record carrying position
    /// `base + token_start' (`read' keeps it off).
    pub annotate_pos: Option<i128>,
    /// True when the source is a buffer (GNU `from_buffer_p'): read
    /// errors then carry `(MSG LINE COLUMN)` like `invalid_syntax_lisp'.
    pub from_buffer: bool,
    /// GNU source_t::multibyte controls newly read symbol-name storage.
    /// Literal strings choose their own kind from the characters read.
    pub source_multibyte: bool,
}

fn read_err(interp: &mut Interp, msg: &str) -> Flow {
    let sym_id = interp.q("invalid-read-syntax");
    interp.signal_data(sym_id, vec![Value::string(msg)])
}

/// `invalid-read-syntax' for an invalid reader token, like `#|' or `#z'.
fn read_err_sym(interp: &mut Interp, name: &str) -> Flow {
    read_err(interp, name)
}

/// `invalid-read-syntax' for radix integers: `(integer, radix N)'.
fn read_err_radix(interp: &mut Interp, radix: u32) -> Flow {
    let sym_id = interp.q("invalid-read-syntax");
    interp.signal_data(
        sym_id,
        vec![Value::string(format!("integer, radix {radix}"))],
    )
}

fn eof_err(interp: &mut Interp) -> Flow {
    let sym_id = interp.q("end-of-file");
    interp.signal(sym_id, Value::Nil)
}

/// GNU `error ()' inside the reader (bad escapes, bad modifiers) —
/// (error "msg"), distinct from `invalid-read-syntax'.
fn plain_err(interp: &mut Interp, msg: &str) -> Flow {
    let sym_id = interp.q("error");
    interp.signal_data(sym_id, vec![Value::string(msg)])
}

/// GNU signals a plain `error' when a \u or \U escape exceeds the
/// Unicode range: (error "Non-Unicode character: 0x%x").
fn non_unicode_err(interp: &mut Interp, n: i128) -> Flow {
    let sym_id = interp.q("error");
    interp.signal_data(
        sym_id,
        vec![Value::string(format!("Non-Unicode character: 0x{n:x}"))],
    )
}

impl<'a> Reader<'a> {
    pub fn new<T: TextSource + ?Sized>(interp: &'a mut Interp, src: &T) -> Reader<'a> {
        src.with_text(|text| Reader::with_chars(interp, Rc::new(text.chars().collect())))
    }

    /// A reader over pre-collected chars — callers that re-read the
    /// same source in a loop share one buffer instead of re-collecting.
    pub fn with_chars(interp: &'a mut Interp, chars: Rc<Vec<EmacsChar>>) -> Reader<'a> {
        Reader {
            chars,
            oversized_codes: None,
            pos: 0,
            source: ReadSource::Characters,
            interp,
            labels: HashMap::default(),
            annotate_pos: None,
            from_buffer: false,
            source_multibyte: true,
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos.min(self.chars.len());
    }

    pub fn with_function(interp: &'a mut Interp, source: Value) -> Reader<'a> {
        let mut reader = Self::with_chars(interp, Rc::new(Vec::new()));
        reader.source = ReadSource::Function {
            function: source,
            eof: false,
        };
        reader
    }

    fn fill_to(&mut self, index: usize) -> Result<(), Flow> {
        while index >= self.chars.len() {
            let ReadSource::Function {
                function,
                eof: false,
            } = &self.source
            else {
                break;
            };
            let value = self.interp.apply(function, vec![])?;
            let character = match value {
                Value::Int(code) if (0..=0x3fffff).contains(&code) => {
                    super::value::lisp_char(code as u32)
                        .unwrap_or_else(|| super::value::mule_seq_char(code as u32))
                }
                // GNU source_function_get treats every non-character as EOF.
                _ => {
                    if let ReadSource::Function { eof, .. } = &mut self.source {
                        *eof = true;
                    }
                    break;
                }
            };
            Rc::make_mut(&mut self.chars).push(character);
        }
        Ok(())
    }

    fn peek(&mut self) -> Result<Option<char>, Flow> {
        self.peek_at(0)
    }

    fn peek_at(&mut self, n: usize) -> Result<Option<char>, Flow> {
        let index = self.pos + n;
        self.fill_to(index)?;
        Ok(self.chars.get(index).map(|c| {
            if self
                .oversized_codes
                .as_ref()
                .is_some_and(|codes| codes.contains_key(&index))
            {
                '\u{fffd}'
            } else {
                c.unicode().unwrap_or('\u{fffd}')
            }
        }))
    }

    fn original_at(&self, position: usize) -> EmacsChar {
        let c = self.chars[position];
        if !self.source_multibyte && (0x80..=0xff).contains(&c.code()) {
            EmacsChar::new(0x3fff00 + c.code()).unwrap()
        } else {
            c
        }
    }

    fn last_character(&self) -> EmacsChar {
        self.original_at(self.pos - 1)
    }

    fn last_character_code(&self) -> i128 {
        self.oversized_codes
            .as_ref()
            .and_then(|codes| codes.get(&(self.pos - 1)))
            .map(|code| i128::from(*code))
            .unwrap_or_else(|| super::value::lisp_char_code(self.last_character()))
    }

    fn next(&mut self) -> Result<Option<char>, Flow> {
        let c = self.peek()?;
        if c.is_some() {
            self.pos += 1;
        }
        Ok(c)
    }

    /// Return lookahead through the source's one-argument unread protocol.
    /// Static source cursors are already positioned before that character.
    fn unread_from(&mut self, position: usize) -> Result<(), Flow> {
        let ReadSource::Function {
            function: source, ..
        } = &self.source
        else {
            return Ok(());
        };
        let pending: Vec<EmacsChar> = self.chars[position.min(self.chars.len())..].to_vec();
        for character in pending.iter().rev() {
            self.interp.apply(
                source,
                vec![Value::Int(super::value::lisp_char_code(*character))],
            )?;
        }
        if !pending.is_empty() {
            Rc::make_mut(&mut self.chars).truncate(position);
            if let ReadSource::Function { eof, .. } = &mut self.source {
                *eof = false;
            }
        }
        Ok(())
    }

    fn is_layout(c: char) -> bool {
        // GNU read0/read_symbol: ASCII controls through space and NBSP,
        // not the broader Unicode White_Space property.
        c <= ' ' || c == '\u{a0}'
    }

    /// Skip whitespace and comments. Returns true at EOF.
    fn skip_layout(&mut self) -> Result<bool, Flow> {
        loop {
            match self.peek()? {
                None => return Ok(true),
                Some(c) if Self::is_layout(c) => {
                    self.pos += 1;
                }
                Some(';') => {
                    while let Some(c) = self.peek()? {
                        self.pos += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('#') if self.peek_at(1)? == Some('!') => {
                    while let Some(c) = self.peek()? {
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

    /// Read one object. `None` = clean EOF.  For buffer sources GNU's
    /// `invalid_syntax_lisp' appends (LINE COLUMN) to the error data —
    /// computed at the reader position where the error surfaced.
    pub fn read(&mut self) -> Result<Option<Value>, Flow> {
        let mut r = self.read_inner();
        if r.is_ok() {
            self.unread_from(self.pos)?;
        }
        if let Err(Flow::Signal(signal)) = &mut r {
            let irs = self.interp.q("invalid-read-syntax");
            if self.from_buffer && self.interp.sym_id(&signal.symbol()) == Some(irs) {
                let (line, col) = self.error_pos();
                let mut data2 = signal.data().list_to_vec().unwrap_or_default();
                data2.push(Value::Int(line as i128));
                data2.push(Value::Int(col as i128));
                signal.set_data(Value::list(data2));
            }
        }
        r
    }

    fn read_inner(&mut self) -> Result<Option<Value>, Flow> {
        if self.skip_layout()? {
            return Ok(None);
        }
        Ok(Some(self.read_object()?))
    }

    /// GNU `invalid_syntax_lisp': (LINE COLUMN) of the read error —
    /// 1-based newline count and char column before `pos'.
    fn error_pos(&self) -> (usize, usize) {
        let mut line = 1;
        let mut col = 0;
        for &c in self.chars[..self.pos.min(self.chars.len())].iter() {
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col + 1)
    }
    fn read_object(&mut self) -> Result<Value, Flow> {
        match self.peek()? {
            None => Err(eof_err(self.interp)),
            Some('(') => {
                self.pos += 1;
                self.read_list(')')
            }
            Some('[') => {
                self.pos += 1;
                let items = self.read_seq(']')?;
                // GNU: `[]' reads as the shared zero_vector, so two
                // empty-vector literals are eq.
                if items.is_empty() {
                    Ok(self.interp.zero_vector.clone())
                } else {
                    Ok(Value::vector(items))
                }
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
                let which = match self.peek()? {
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
                let n = match self.next()? {
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
                    Some(_) => {
                        let code = self.last_character_code();
                        self.check_char_delim()?;
                        code
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
            Some('.') if self.at_dot_token()? => Err(read_err(self.interp, ".")),
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
            let c = self.peek()?.unwrap();
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
            if c == '.' && self.peek_at(1)? == Some(close) {
                items.push(self.read_atom()?);
                continue;
            }
            if c == '.' && self.at_dot_token()? {
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
                if self.peek()? == Some(close) {
                    return Err(read_err(self.interp, ")"));
                }
                let tail = self.read_object()?;
                if self.skip_layout()? || self.peek()? != Some(close) {
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
    fn at_dot_token(&mut self) -> Result<bool, Flow> {
        debug_assert_eq!(self.peek()?, Some('.'));
        let result = match self.peek_at(1)? {
            None => true,
            Some(c) => Self::is_layout(c) || matches!(c, '(' | '[' | '"' | '\'' | '`' | ',' | ';'),
        };
        self.unread_from(self.pos + 1)?;
        Ok(result)
    }

    /// Read a sequence of objects until `close` (for vectors).
    fn read_seq(&mut self, close: char) -> Result<Vec<Value>, Flow> {
        let mut items = Vec::new();
        loop {
            if self.skip_layout()? {
                return Err(eof_err(self.interp));
            }
            if self.peek()? == Some(close) {
                self.pos += 1;
                return Ok(items);
            }
            // `.' directly before `]' reads as the symbol `\.'
            // (Emacs: `[a .]' → [a \.]).
            if self.peek()? == Some('.') && self.peek_at(1)? == Some(close) {
                items.push(self.read_atom()?);
                continue;
            }
            items.push(self.read_object()?);
        }
    }

    /// `read_seq' with symbol positioning disabled — GNU's reader sets
    /// `locate_syms = false' inside `#s(...)', `#[...]', `#^[...]' and
    /// `#(...)' literals, so their contents stay bare symbols even
    /// under `read-positioning-symbols'.  (lread.c RE_record,
    /// RE_byte_code, RE_char_table, RE_string_props.)
    fn read_seq_unannotated(&mut self, close: char) -> Result<Vec<Value>, Flow> {
        let saved = self.annotate_pos.take();
        let items = self.read_seq(close);
        self.annotate_pos = saved;
        items
    }

    /// Chars that terminate a symbol/number token.
    /// `#` terminates (expression-prefix syntax); `?` does not
    /// (it's a symbol constituent in elisp).
    fn is_terminator(c: char) -> bool {
        Self::is_layout(c)
            || matches!(
                c,
                '(' | ')' | '[' | ']' | '"' | '\'' | '`' | ',' | ';' | '#'
            )
    }

    /// Read a raw token (symbol constituent chars, `\` escapes included).
    /// The returned bool reports whether any `\` escape was consumed, so
    /// `read_atom' can tell `.' (dotting token) apart from `\.' (symbol).
    fn read_symbol_token(&mut self) -> Result<(EmacsText, bool), Flow> {
        let mut tok = EmacsText::new();
        let mut escaped = false;
        while let Some(c) = self.peek()? {
            if Self::is_terminator(c) {
                break;
            }
            if c == '\\' {
                self.pos += 1;
                if self.next()?.is_some() {
                    tok.push(self.last_character());
                    escaped = true;
                    continue;
                }
                break;
            }
            tok.push(self.original_at(self.pos));
            self.pos += 1;
        }
        self.unread_from(self.pos)?;
        Ok((tok, escaped))
    }

    /// Read a token and classify: number, symbol, or error.
    fn read_atom(&mut self) -> Result<Value, Flow> {
        let start = self.pos;
        let (tok, escaped) = self.read_symbol_token()?;
        if tok.is_empty() {
            return Err(read_err(self.interp, "empty token"));
        }
        // GNU read0: a token that contains a `\` escape is never a
        // number — `3\.5' reads as the symbol `3.5', not the float.
        if !escaped {
            if tok.is_ascii() {
                if let Some(v) = parse_number(&tok.to_unicode_lossy()) {
                    return Ok(v);
                }
            }
        }
        match tok.to_unicode_lossy().as_str() {
            // `.' is only a symbol when `)' or `]' follows directly;
            // `\.' is always the symbol `.' (GNU reads (a \. b) as a
            // 3-element list whose middle element is the `.' symbol).
            "." if escaped
                || self
                    .oversized_codes
                    .as_ref()
                    .is_some_and(|codes| codes.contains_key(&start)) => {}
            "." if matches!(self.peek()?, Some(')') | Some(']')) => {}
            "." => return Err(read_err(self.interp, ".")),
            _ => {}
        }
        let sym = self.symbol_token(&tok, false, true)?;
        Ok(self.located_symbol(sym, start))
    }

    fn located_symbol(&mut self, symbol: super::value::SymId, start: usize) -> Value {
        if symbol == sym::NIL {
            return Value::Nil;
        }
        if let Some(base) = self.annotate_pos {
            return crate::lisp::builtins::misc::make_symbol_with_pos(
                self.interp,
                symbol,
                base + start as i128,
            );
        }
        Value::Sym(symbol)
    }

    fn symbol_token<T: TextSource + ?Sized>(
        &mut self,
        token: &T,
        uninterned: bool,
        shorthand: bool,
    ) -> Result<super::value::SymId, Flow> {
        token.with_text(|token| {
            let obarray = if !uninterned && !token.is_empty() {
                super::builtins::data::resolve_obarray(self.interp, None)?
            } else {
                None
            };
            let core = token
                .chars()
                .all(|c| c.code() < 128 && b"^*+-/<=>_|".contains(&(c.code() as u8)));
            let expanding = shorthand
                && !core
                && self
                    .interp
                    .q_soft("read-symbol-shorthands")
                    .is_some_and(|symbol| {
                        matches!(self.interp.symbol_value(symbol), Value::Cons(_))
                    });
            if !uninterned && !token.is_empty() && !expanding {
                let key = super::obarray::InternKey::from_text_kind(token, self.source_multibyte);
                if let Some(symbol) =
                    super::builtins::data::lookup_in_obarray(self.interp, &obarray, &key)
                {
                    return Ok(symbol);
                }
            }
            // Unibyte sources expose high bytes as byte8 characters while reading,
            // but their symbol names retain the original one-byte representation.
            let name = Value::string(token);
            if let Value::Str(string) = &name {
                if self.source_multibyte && !token.is_empty() {
                    self.interp.mark_multibyte(string);
                } else {
                    self.interp.mark_unibyte(string);
                }
            }
            let Value::Str(string) = name else {
                unreachable!()
            };
            if uninterned {
                let symbol = self.interp.make_symbol(token);
                self.interp.obarray.symbol_mut(symbol).name_string = Some(string);
                return Ok(symbol);
            }
            // GNU's empty #_ / ## paths call Fintern directly, including
            // shorthand expansion and the shared empty unibyte representation.
            if token.is_empty() {
                let value =
                    super::builtins::data::f_intern(self.interp, vec![Value::Str(string)].into())?;
                return Ok(self.interp.sym_id(&value).unwrap());
            }
            // Core punctuation-only names are exempt from reader shorthands.
            let string = if expanding {
                super::builtins::data::expand_symbol_shorthand(
                    self.interp,
                    &string,
                    self.source_multibyte,
                )
                .unwrap_or(string)
            } else {
                string
            };
            Ok(super::builtins::data::intern_in_obarray(
                self.interp,
                &string,
                &obarray,
            ))
        })
    }
}

/// Only functional input has an unread callback and cached EOF. Keeping them
/// together prevents static character input from acquiring stream-only state.
enum ReadSource {
    Characters,
    Function { function: Value, eof: bool },
}
