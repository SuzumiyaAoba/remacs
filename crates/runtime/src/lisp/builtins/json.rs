//! JSON subrs: `json-parse-string`, `json-parse-buffer`, `json-insert`,
//! `json-serialize` — a self-contained parser/serializer matching GNU's
//! jansson-backed builtins: objects → hash-table/alist/plist, arrays →
//! vector/list, `null`/`:null-object`, `false`/`:false-object`.

use super::S;
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::value::{HashTest, LispHash, Subr, Value};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "json-parse-string",
        many 1,
        f_json_parse_string,
        "Parse JSON STRING into Lisp data."
    ),
    S!(
        "json-parse-buffer",
        many 0,
        f_json_parse_buffer,
        "Parse JSON in current buffer from point."
    ),
    S!(
        "json-insert",
        many 1,
        f_json_insert,
        "Insert JSON serialization of OBJECT at point."
    ),
    S!(
        "json-serialize",
        many 1,
        f_json_serialize,
        "Serialize OBJECT to a JSON string."
    ),
];

#[derive(Clone, Copy, PartialEq)]
enum ObjType {
    Hash,
    Alist,
    Plist,
}

struct Opts {
    obj: ObjType,
    array_list: bool,
    null_obj: Value,
    false_obj: Value,
}

fn plist_args(i: &mut Interp, args: &[Value]) -> Result<Vec<(Value, Value)>, Flow> {
    // GNU json_parse_args: odd arg count → `wta plistp' on the whole
    // arg list BEFORE any keyword check; keys are NOT type-checked
    // (a non-keyword pair falls into `wrong_choice' below).
    if args.len() % 2 != 0 {
        return Err(i.wrong_type_mut("plistp", &Value::list(args.to_vec())));
    }
    let mut out = Vec::new();
    for kv in args.chunks(2) {
        out.push((kv[0].clone(), kv[1].clone()));
    }
    Ok(out)
}

/// GNU `wrong_choice (choices, wrong)': signals `error' whose data is
/// ("One of A, B or C should be specified" WRONG).
fn json_wrong_choice(i: &mut Interp, choices: &[&str], wrong: &Value) -> Flow {
    let mut msg = String::from("One of ");
    for (k, c) in choices.iter().enumerate() {
        if k > 0 {
            msg.push_str(if k + 1 == choices.len() { " or " } else { ", " });
        }
        msg.push_str(c);
    }
    msg.push_str(" should be specified");
    let e = i.q("error");
    i.signal_data(e, vec![Value::string(msg), wrong.clone()])
}

fn parse_opts(i: &mut Interp, rest: &[Value], for_serialize: bool) -> Result<Opts, Flow> {
    let mut o = Opts {
        obj: ObjType::Hash,
        array_list: false,
        null_obj: Value::Sym(i.q(":null")),
        false_obj: Value::Sym(i.q(":false")),
    };
    let pairs = plist_args(i, rest)?;
    let obj_type = i.q(":object-type");
    let array_type = i.q(":array-type");
    let null_object = i.q(":null-object");
    let false_object = i.q(":false-object");
    // GNU json_parse_args iterates BACKWARD so keyword values appearing
    // first take precedence — the FIRST signal from the tail wins.
    for (key, value) in pairs.into_iter().rev() {
        if !for_serialize && i.sym_is(&key, obj_type) {
            let t = i.sym_id(&value).map(|s| i.symbol_name(s).to_string());
            match t.as_deref() {
                Some("hash-table") => o.obj = ObjType::Hash,
                Some("alist") => o.obj = ObjType::Alist,
                Some("plist") => o.obj = ObjType::Plist,
                _ => {
                    return Err(json_wrong_choice(
                        i,
                        &["hash-table", "alist", "plist"],
                        &value,
                    ));
                }
            }
        } else if !for_serialize && i.sym_is(&key, array_type) {
            let t = i.sym_id(&value).map(|s| i.symbol_name(s).to_string());
            match t.as_deref() {
                Some("array") => o.array_list = false,
                Some("list") => o.array_list = true,
                _ => return Err(json_wrong_choice(i, &["array", "list"], &value)),
            }
        } else if i.sym_is(&key, null_object) {
            o.null_obj = value;
        } else if i.sym_is(&key, false_object) {
            o.false_obj = value;
        } else if !for_serialize {
            return Err(json_wrong_choice(
                i,
                &[
                    ":object-type",
                    ":array-type",
                    ":null-object",
                    ":false-object",
                ],
                &value,
            ));
        } else {
            return Err(json_wrong_choice(
                i,
                &[":null-object", ":false-object"],
                &value,
            ));
        }
    }
    Ok(o)
}

/// Lazily install a json error's `error-conditions' chain (as GNU's
/// define_error does) so `condition-case' catches it by parent.
fn json_cond_chain(i: &mut Interp, id: u32) {
    let ec = i.q("error-conditions");
    if i.get_prop(id, ec).truthy() {
        return;
    }
    let name = i.symbol_name(id).to_string();
    // GNU json.c: escape-sequence and trailing-content sit under
    // json-parse-error; invalid-surrogate sits directly under
    // json-error (NOT caught by a json-parse-error handler).
    let parents: &[&str] = match name.as_str() {
        "json-escape-sequence-error" | "json-trailing-content" => {
            &["json-parse-error", "json-error", "error"]
        }
        _ => &["json-error", "error"],
    };
    let mut chain = vec![Value::Sym(id)];
    for p in parents {
        chain.push(Value::Sym(i.intern(p)));
    }
    i.put_prop(id, ec, Value::list(chain));
}

// ---------- parser ----------

struct Parser<'a> {
    s: Vec<char>,
    /// Unibyte sources retain one input character per byte so error and
    /// buffer positions use their original units. Decode quoted UTF-8 only.
    byte_input: bool,
    pos: usize,
    line: usize,
    /// Char index where parsing began — error POSITION is GNU's
    /// `BYTE_TO_CHAR' offset from that start (0 at point/string
    /// start), not the absolute index.
    base: usize,
    i: &'a mut Interp,
    o: &'a Opts,
}

type PErr = Flow;

impl<'a> Parser<'a> {
    /// Signal a json error carrying GNU's (LINE COLUMN POSITION) shape —
    /// column is nil in GNU's reports, position is the 1-based char
    /// offset of the offending spot.
    fn err(&mut self, eof: bool) -> PErr {
        let sym = if eof {
            "json-end-of-file"
        } else {
            "json-parse-error"
        };
        // GNU `json_signal_error': POSITION is BYTE_TO_CHAR of the
        // consumed byte count, relative to the parse start — i.e. the
        // number of chars consumed (the erroring char itself was
        // already consumed for mid-text errors, so it's pos-base+1
        // under remacs's pre-consume `pos' tracking; EOF consumes
        // nothing and reports the exhausted count directly).
        let consumed = self.pos.saturating_sub(self.base);
        let pos = if eof {
            consumed as i128
        } else {
            consumed as i128 + 1
        };
        let id = self.i.intern(sym);
        json_cond_chain(self.i, id);
        self.i.signal_data(
            id,
            vec![Value::Int(self.line as i128), Value::Nil, Value::Int(pos)],
        )
    }

    /// A json-family error after consuming the offending character,
    /// carrying GNU's (LINE nil POSITION) shape.
    fn err_sym(&mut self, sym: &str) -> PErr {
        let pos = self.pos.saturating_sub(self.base) as i128;
        let id = self.i.intern(sym);
        json_cond_chain(self.i, id);
        self.i.signal_data(
            id,
            vec![
                Value::Int(self.line as i128),
                Value::Nil,
                Value::Int(pos),
            ],
        )
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                '\n' => {
                    self.line += 1;
                    self.pos += 1;
                }
                ' ' | '\t' | '\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn expect(&mut self, c: char) -> Result<(), PErr> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(self.peek().is_none()))
        }
    }

    fn value(&mut self) -> Result<Value, PErr> {
        self.ws();
        match self.peek() {
            None => Err(self.err(true)),
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Value::string(self.string()?)),
            Some('t') => self.lit("true", Value::t()),
            Some('f') => {
                let v = self.o.false_obj.clone();
                self.lit("false", v)
            }
            Some('n') => {
                let v = self.o.null_obj.clone();
                self.lit("null", v)
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err(false)),
        }
    }

    /// Error POSITION = chars consumed since parse start (GNU reports
    /// the consumed byte count, which already includes the char that
    /// failed the check — hence no +1 here, unlike `err').
    fn err_consumed(&mut self) -> PErr {
        self.err_consumed_as("json-parse-error")
    }

    fn err_consumed_as(&mut self, name: &str) -> PErr {
        let pos = self.pos.saturating_sub(self.base) as i128;
        let id = self.i.intern(name);
        json_cond_chain(self.i, id);
        self.i.signal_data(
            id,
            vec![Value::Int(self.line as i128), Value::Nil, Value::Int(pos)],
        )
    }

    /// GNU `json_is_token_char': chars that may extend a true/false/
    /// null token (making the whole thing invalid).
    fn is_token_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '-'
    }

    fn lit(&mut self, word: &str, v: Value) -> Result<Value, PErr> {
        for c in word.chars() {
            // GNU signals json-parse-error (not json-end-of-file) for
            // a truncated literal like "tru"; its input was consumed
            // by the literal check, so POSITION is the consumed count.
            match self.peek() {
                None => return Err(self.err_consumed()),
                Some(x) => {
                    self.pos += 1;
                    if x != c {
                        return Err(self.err_consumed());
                    }
                }
            }
        }
        // GNU: a token char after the literal invalidates it ("truex").
        if let Some(c) = self.peek() {
            if Self::is_token_char(c) {
                self.pos += 1;
                return Err(self.err_consumed());
            }
        }
        Ok(v)
    }

    fn utf8_error(&mut self) -> PErr {
        self.err_consumed_as("json-utf8-decode-error")
    }

    fn utf8_byte(&mut self) -> Result<u8, PErr> {
        let Some(c) = self.peek() else {
            return Err(self.err(true));
        };
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
        }
        Ok(c as u8)
    }

    fn utf8_char(&mut self, lead: u8) -> Result<char, PErr> {
        // Match GNU's read order, including EOF while fetching continuation
        // bytes and the consumed position of malformed sequences.
        let second = self.utf8_byte()?;
        if second & 0xc0 != 0x80 || lead <= 0xc1 {
            return Err(self.utf8_error());
        }
        let code = if lead <= 0xdf {
            ((lead & 0x1f) as u32) << 6 | (second & 0x3f) as u32
        } else if lead <= 0xef {
            let third = self.utf8_byte()?;
            if third & 0xc0 != 0x80 {
                return Err(self.utf8_error());
            }
            let code = ((lead & 0x0f) as u32) << 12
                | ((second & 0x3f) as u32) << 6
                | (third & 0x3f) as u32;
            if code < 0x800 || (0xd800..=0xdfff).contains(&code) {
                return Err(self.utf8_error());
            }
            code
        } else if lead <= 0xf7 {
            let third = self.utf8_byte()?;
            let fourth = self.utf8_byte()?;
            if third & 0xc0 != 0x80 || fourth & 0xc0 != 0x80 {
                return Err(self.utf8_error());
            }
            let code = ((lead & 7) as u32) << 18
                | ((second & 0x3f) as u32) << 12
                | ((third & 0x3f) as u32) << 6
                | (fourth & 0x3f) as u32;
            if !(0x10000..=0x10ffff).contains(&code) {
                return Err(self.utf8_error());
            }
            code
        } else {
            return Err(self.utf8_error());
        };
        char::from_u32(code).ok_or_else(|| self.utf8_error())
    }

    fn string(&mut self) -> Result<String, PErr> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err(true)),
                Some('"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        None => return Err(self.err(true)),
                        Some(e) => {
                            self.pos += 1;
                            match e {
                                '"' => out.push('"'),
                                '\\' => out.push('\\'),
                                '/' => out.push('/'),
                                'b' => out.push('\u{8}'),
                                'f' => out.push('\u{c}'),
                                'n' => out.push('\n'),
                                'r' => out.push('\r'),
                                't' => out.push('\t'),
                                'u' => {
                                    let hi = self.hex4()?;
                                    let cp = if (0xd800..0xdc00).contains(&hi) {
                                        // Surrogate pair is mandatory
                                        // after a high surrogate.
                                        if self.peek() == Some('\\') {
                                            self.pos += 1;
                                            if self.peek() == Some('u') {
                                                self.pos += 1;
                                                let lo = self.hex4()?;
                                                if (0xdc00..0xe000).contains(&lo) {
                                                    0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                                                } else {
                                                    return Err(self
                                                        .err_sym("json-invalid-surrogate-error"));
                                                }
                                            } else {
                                                return Err(
                                                    self.err_sym("json-invalid-surrogate-error")
                                                );
                                            }
                                        } else {
                                            return Err(
                                                self.err_sym("json-invalid-surrogate-error")
                                            );
                                        }
                                    } else if (0xdc00..0xe000).contains(&hi) {
                                        // A lone low surrogate.
                                        return Err(self.err_sym("json-invalid-surrogate-error"));
                                    } else {
                                        hi
                                    };
                                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                                }
                                _ => return Err(self.err_sym("json-escape-sequence-error")),
                            }
                        }
                    }
                }
                Some(c) => {
                    if (c as u32) < 0x20 {
                        if c == '\n' {
                            self.line += 1;
                        }
                        return Err(self.err(false));
                    }
                    self.pos += 1;
                    if self.byte_input && c as u32 >= 0x80 {
                        out.push(self.utf8_char(c as u8)?);
                    } else if !is_unicode_char(c) {
                        return Err(self.utf8_error());
                    } else {
                        out.push(c);
                    }
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, PErr> {
        let mut v = 0u32;
        for _ in 0..4 {
            match self.peek().and_then(|c| c.to_digit(16)) {
                Some(d) => {
                    v = v * 16 + d;
                    self.pos += 1;
                }
                // GNU signals json-escape-sequence-error for a
                // non-hex char in \uXXXX (json-end-of-file at EOF).
                None => {
                    return Err(if self.peek().is_none() {
                        self.err(true)
                    } else {
                        self.err_sym("json-escape-sequence-error")
                    });
                }
            }
        }
        Ok(v)
    }

    /// JSON number grammar: -?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?
    /// A `0' is a complete integer part — "01" parses `0' and the
    /// `1' is trailing content.  A missing required digit errors
    /// (json-end-of-file at EOF, json-parse-error otherwise).
    fn number(&mut self) -> Result<Value, PErr> {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        // Integer part.
        match self.peek() {
            Some('0') => self.pos += 1,
            Some('1'..='9') => {
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            other => {
                let eof = other.is_none();
                return Err(self.err(eof));
            }
        }
        // Fraction.
        if self.peek() == Some('.') {
            is_float = true;
            self.pos += 1;
            match self.peek() {
                Some(c) if c.is_ascii_digit() => {}
                other => {
                    let eof = other.is_none();
                    return Err(self.err(eof));
                }
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        // Exponent.
        if matches!(self.peek(), Some('e' | 'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.pos += 1;
            }
            match self.peek() {
                Some(c) if c.is_ascii_digit() => {}
                other => {
                    let eof = other.is_none();
                    return Err(self.err(eof));
                }
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let txt: String = self.s[start..self.pos].iter().collect();
        if !is_float {
            if let Ok(n) = txt.parse::<num_bigint::BigInt>() {
                return Ok(Value::from_bigint(n));
            }
        }
        let value = txt.parse::<f64>().map_err(|_| self.err(false))?;
        if !value.is_finite() {
            return Err(self.err_consumed_as("json-number-out-of-range-error"));
        }
        Ok(Value::float(value))
    }

    fn array(&mut self) -> Result<Value, PErr> {
        self.pos += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(']') {
            self.pos += 1;
        } else {
            loop {
                items.push(self.value()?);
                self.ws();
                match self.peek() {
                    Some(',') => self.pos += 1,
                    Some(']') => {
                        self.pos += 1;
                        break;
                    }
                    other => {
                        let _ = other;
                        return Err(self.err(other.is_none()));
                    }
                }
            }
        }
        if self.o.array_list {
            Ok(Value::list(items))
        } else {
            Ok(Value::vector(items))
        }
    }

    fn object(&mut self) -> Result<Value, PErr> {
        self.pos += 1; // {
        let mut pairs: Vec<(String, Value)> = Vec::new();
        self.ws();
        if self.peek() == Some('}') {
            self.pos += 1;
        } else {
            loop {
                self.ws();
                if self.peek() != Some('"') {
                    return Err(self.err(self.peek().is_none()));
                }
                let key = self.string()?;
                self.ws();
                self.expect(':')?;
                let val = self.value()?;
                pairs.push((key, val));
                self.ws();
                match self.peek() {
                    Some(',') => self.pos += 1,
                    Some('}') => {
                        self.pos += 1;
                        break;
                    }
                    other => return Err(self.err(other.is_none())),
                }
            }
        }
        Ok(match self.o.obj {
            ObjType::Hash => {
                let mut h = LispHash::new(HashTest::Equal);
                for (k, v) in pairs {
                    let key = Value::string(k);
                    let hk =
                        crate::lisp::builtins::hashfn::hash_key_for(self.i, &key, HashTest::Equal);
                    h.map.insert(hk.clone(), v);
                    h.put_key(hk, key);
                }
                Value::Hash(Rc::new(RefCell::new(h))).tracked()
            }
            ObjType::Alist => {
                let mut items = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    items.push(Value::cons(Value::Sym(self.i.intern(&k)), v));
                }
                Value::list(items)
            }
            ObjType::Plist => {
                let mut items = Vec::with_capacity(pairs.len() * 2);
                for (k, v) in pairs {
                    items.push(Value::Sym(self.i.intern(&format!(":{}", k))));
                    items.push(v);
                }
                Value::list(items)
            }
        })
    }
}

fn f_json_parse_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (src, byte_input) = match &a[0] {
        Value::Str(s) => (s.borrow().clone(), i.is_unibyte_str(s)),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let o = parse_opts(i, &a[1..], false)?;
    let mut p = Parser {
        s: src
            .chars()
            .map(|c| if byte_input { (c as u8) as char } else { c })
            .collect(),
        byte_input,
        pos: 0,
        line: 1,
        base: 0,
        i,
        o: &o,
    };
    let v = p.value()?;
    p.ws();
    if p.pos != p.s.len() {
        // GNU: leftover input after a valid value is a dedicated
        // `json-trailing-content' error with (LINE nil POS) data.
        let (pos, line) = (p.pos + 1, p.line as i128);
        drop(p);
        let e = i.q("json-trailing-content");
        json_cond_chain(i, e);
        return Err(i.signal_data(
            e,
            vec![Value::Int(line), Value::Nil, Value::Int(pos as i128)],
        ));
    }
    Ok(v)
}

fn f_json_parse_buffer(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let o = parse_opts(i, &a[..], false)?;
    let buf = i.current_buffer;
    let multibyte = i.q("enable-multibyte-characters");
    let byte_input = i.symbol_value(multibyte).is_nil();
    let (text, point, end) = {
        let Some(b) = i.buffers.get(buf) else {
            return Ok(Value::Nil);
        };
        let bb = b.borrow();
        (bb.text.text(), bb.point, bb.zv)
    };
    // Keep absolute character positions, but never give the parser text
    // beyond the accessible end of a narrowed buffer.
    let chars: Vec<char> = text
        .chars()
        .take(end)
        .map(|c| if byte_input { (c as u8) as char } else { c })
        .collect();
    let start = point.min(chars.len());
    // GNU `buffer_byte_to_line' counts newlines inside the parsed
    // span only, so error LINE starts at 1 at point like POSITION
    // starts at 0.
    let mut p = Parser {
        s: chars,
        byte_input,
        pos: start,
        line: 1,
        base: start,
        i,
        o: &o,
    };
    let v = p.value()?;
    if let Some(b) = p.i.buffers.get(buf) {
        b.borrow_mut().set_point(p.pos);
    }
    Ok(v)
}

fn f_json_insert(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = serialize(i, &a[0], &a[1..])?;
    crate::buffer::primitives::chg_insert_utf8_pt(i, &s)?;
    Ok(Value::Nil)
}

// ---------- serializer ----------

fn is_unicode_char(c: char) -> bool {
    crate::lisp::value::eight_bit_byte(c).is_none()
        && crate::lisp::value::surrogate_code(c).is_none()
        && crate::lisp::value::mule_seq_code(c).is_none()
}

fn check_json_string(i: &mut Interp, value: &Value) -> Result<(), Flow> {
    let Value::Str(string) = value else {
        return Err(i.wrong_type_mut("stringp", value));
    };
    let text = string.borrow();
    if (i.is_unibyte_str(string) && !text.is_ascii()) || !text.chars().all(is_unicode_char) {
        return Err(json_value_err(i, value));
    }
    Ok(())
}

fn esc_into(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_value_err(i: &mut Interp, v: &Value) -> Flow {
    // GNU signals `wrong-type-argument' with predicate `json-value-p'.
    i.wrong_type_mut("json-value-p", v)
}

fn ser_value(
    i: &mut Interp,
    v: &Value,
    o: &Opts,
    out: &mut String,
    remaining: usize,
) -> Result<(), Flow> {
    // GNU compares arbitrary sentinel objects by identity before dispatching
    // on their type; null takes precedence if both options name the same value.
    if crate::lisp::builtins::eq_values(&o.null_obj, v) {
        out.push_str("null");
        return Ok(());
    }
    if crate::lisp::builtins::eq_values(&o.false_obj, v) {
        out.push_str("false");
        return Ok(());
    }
    // GNU json_out_nest counts arrays, hashes and nonempty objects. Nil's
    // empty object and configured sentinels do not consume nesting depth.
    if remaining == 0 && matches!(v, Value::Vec(_) | Value::Cons(_) | Value::Hash(_)) {
        return Err(i.error("Maximum JSON serialization depth exceeded"));
    }
    match v {
        Value::Nil => out.push_str("{}"),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::BigInt(n) => out.push_str(&n.to_string()),
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(json_value_err(i, v));
            }
            // GNU serializes floats via the print.c shortest-%g path:
            // 1e300 is "1e+300", not the raw decimal expansion.
            out.push_str(&crate::lisp::print::format_float(**f));
        }
        Value::Str(s) => {
            check_json_string(i, v)?;
            esc_into(&s.borrow(), out);
        }
        Value::Sym(id) => {
            if *id == crate::lisp::obarray::sym::T {
                out.push_str("true");
            } else {
                return Err(json_value_err(i, v));
            }
        }
        Value::Vec(vv) => {
            out.push('[');
            for (n, e) in vv.borrow().iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                ser_value(i, e, o, out, remaining - 1)?;
            }
            out.push(']');
        }
        Value::Cons(_) => {
            // GNU json_out_object_cons: `is_alist' iff the first
            // ELEMENT is a cons. Alist mode takes (KEY . VALUE)
            // elements — non-cons elements signal (consp ELEM);
            // plist mode consumes two conses per pair — a non-cons
            // value tail signals (consp TAIL). Keys must be bare
            // symbols. After the walk, a surviving non-nil tail
            // signals (listp WHOLE-OBJ).
            let is_alist = {
                let Value::Cons(c) = v else { unreachable!() };
                matches!(c.borrow().car, Value::Cons(_))
            };
            out.push('{');
            let mut tail = v.clone();
            let mut first = true;
            let mut seen = std::collections::HashSet::new();
            // Match GNU FOR_EACH_TAIL's Brent checkpoints, including the
            // actual tail reported as the circular-list error datum.
            let mut cycle_anchor = tail.clone();
            let mut cycle_limit = 2usize;
            let mut cycle_steps = 0usize;
            while let Value::Cons(c) = tail.clone() {
                let (car, cdr) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                let (key, value);
                if is_alist {
                    let Value::Cons(pair) = car else {
                        return Err(i.wrong_type_mut("consp", &car));
                    };
                    let b = pair.borrow();
                    key = b.car.clone();
                    value = b.cdr.clone();
                } else {
                    key = car;
                    if !matches!(cdr, Value::Cons(_)) {
                        return Err(i.wrong_type_mut("consp", &cdr));
                    }
                    let Value::Cons(vcons) = cdr.clone() else {
                        unreachable!()
                    };
                    value = vcons.borrow().car.clone();
                    tail = cdr;
                }
                if !matches!(key, Value::Sym(_)) {
                    return Err(i.wrong_type_mut("symbolp", &key));
                }
                let Value::Sym(id) = key else { unreachable!() };
                // GNU's symset dedupes: a repeated key's emit is
                // skipped entirely, value and all.
                if seen.insert(id) {
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    // Plist keys drop the leading ':'; alist keys don't.
                    let key_name = super::data::f_symbol_name(i, vec![Value::Sym(id)].into())?;
                    check_json_string(i, &key_name)?;
                    let Value::Str(name_string) = key_name else {
                        unreachable!()
                    };
                    let raw = name_string.borrow().clone();
                    let name = if is_alist {
                        raw.to_string()
                    } else {
                        raw.strip_prefix(':').unwrap_or(&raw).to_string()
                    };
                    esc_into(&name, out);
                    out.push(':');
                    ser_value(i, &value, o, out, remaining - 1)?;
                }
                // FOR_EACH_TAIL stepper: advance one cons.
                tail = match &tail {
                    Value::Cons(c) => c.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
                cycle_steps += 1;
                if cycle_steps == cycle_limit {
                    cycle_steps = 0;
                    cycle_limit = cycle_limit.saturating_mul(2);
                    cycle_anchor = tail.clone();
                } else if crate::lisp::builtins::eq_values(&tail, &cycle_anchor) {
                    return Err(i.signal_data(crate::lisp::obarray::sym::CIRCULAR_LIST, vec![tail]));
                }
            }
            if !tail.is_nil() {
                // CHECK_LIST_END carries the whole object as datum.
                return Err(i.wrong_type_mut("listp", v));
            }
            out.push('}');
        }
        Value::Hash(h) => {
            // Collect (key, value) pairs first so the RefCell borrow
            // doesn't span recursive serialization.
            let pairs: Vec<(Value, Value)> = {
                let hh = h.borrow();
                hh.keys
                    .iter()
                    .filter_map(|(hk, k)| hh.map.get(hk).map(|v| (k.clone(), v.clone())))
                    .collect()
            };
            out.push('{');
            for (n, (k, val)) in pairs.iter().enumerate() {
                // GNU requires STRING hash keys for serialization
                // (symbol keys signal wrong-type-argument stringp).
                let name = match k {
                    Value::Str(s) => {
                        check_json_string(i, &k)?;
                        s.borrow().clone()
                    }
                    other => return Err(i.wrong_type_mut("stringp", other)),
                };
                if n > 0 {
                    out.push(',');
                }
                esc_into(&name, out);
                out.push(':');
                ser_value(i, val, o, out, remaining - 1)?;
            }
            out.push('}');
        }
        _ => return Err(json_value_err(i, v)),
    }
    Ok(())
}

fn serialize(i: &mut Interp, v: &Value, rest: &[Value]) -> Result<String, Flow> {
    let o = parse_opts(i, rest, true)?;
    let mut out = String::new();
    ser_value(i, v, &o, &mut out, 50)?;
    Ok(out)
}

fn f_json_serialize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = serialize(i, &a[0], &a[1..])?;
    // GNU's jansson-backed serializer returns a UNIBYTE string holding
    // the UTF-8 output bytes (jansson emits UTF-8).
    let v = Value::string(s.bytes().map(|b| b as char).collect::<String>());
    if let Value::Str(r) = &v {
        i.mark_unibyte(r);
    }
    Ok(v)
}
