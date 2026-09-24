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

fn plist_args(i: &mut Interp, args: &[Value]) -> Result<Vec<(String, Value)>, Flow> {
    if args.len() % 2 != 0 {
        return Err(i.wrong_type_mut("plistp", &Value::list(args.to_vec())));
    }
    let mut out = Vec::new();
    for kv in args.chunks(2) {
        let name = match &kv[0] {
            Value::Sym(s) => i.symbol_name(*s).to_string(),
            other => return Err(i.wrong_type_mut("plistp", other)),
        };
        out.push((name, kv[1].clone()));
    }
    Ok(out)
}

fn parse_opts(i: &mut Interp, rest: &[Value], for_serialize: bool) -> Result<Opts, Flow> {
    let mut o = Opts {
        obj: ObjType::Hash,
        array_list: false,
        null_obj: Value::Sym(i.intern(":null")),
        false_obj: Value::Sym(i.intern(":false")),
    };
    let mut bad_obj: Option<Value> = None;
    let mut bad_arr: Option<Value> = None;
    // For serialize: the value of the last :object-type/:array-type
    // keyword in argument order (GNU rejects them there outright).
    let mut last_ser_key: Option<Value> = None;
    for (k, v) in plist_args(i, rest)? {
        match k.as_str() {
            ":object-type" => {
                last_ser_key = Some(v.clone());
                let t = i.sym_id(&v).map(|s| i.symbol_name(s).to_string());
                match t.as_deref() {
                    Some("hash-table") => o.obj = ObjType::Hash,
                    Some("alist") => o.obj = ObjType::Alist,
                    Some("plist") => o.obj = ObjType::Plist,
                    _ => bad_obj = Some(v),
                }
            }
            ":array-type" => {
                last_ser_key = Some(v.clone());
                let t = i.sym_id(&v).map(|s| i.symbol_name(s).to_string());
                match t.as_deref() {
                    Some("array") => o.array_list = false,
                    Some("list") => o.array_list = true,
                    _ => bad_arr = Some(v),
                }
            }
            ":null-object" => o.null_obj = v,
            ":false-object" => o.false_obj = v,
            // GNU rejects unknown keywords outright.
            _ => {
                let e = i.intern("error");
                return Err(i.signal_data(
                    e,
                    vec![
                        Value::string(format!(
                            "Keyword argument {} not one of (:object-type :array-type :null-object :false-object)",
                            k
                        )),
                    ],
                ));
            }
        }
    }
    // GNU validates in this order: bad object-type first, then the
    // serialize restriction, then bad array-type.
    if let Some(v) = bad_obj {
        let e = i.intern("error");
        return Err(i.signal_data(
            e,
            vec![
                Value::string("One of hash-table, alist or plist should be specified"),
                v,
            ],
        ));
    }
    // json-serialize accepts only :null-object and :false-object;
    // a present :object-type/:array-type errors with its value
    // (the last one in argument order when both appear).
    if for_serialize {
        if let Some(v) = last_ser_key {
            let e = i.intern("error");
            return Err(i.signal_data(
                e,
                vec![
                    Value::string(
                        "One of :null-object or :false-object should be specified",
                    ),
                    v,
                ],
            ));
        }
    }
    if let Some(v) = bad_arr {
        let e = i.intern("error");
        return Err(i.signal_data(
            e,
            vec![
                Value::string("One of array or list should be specified"),
                v,
            ],
        ));
    }
    Ok(o)
}

/// Lazily install a json error's `error-conditions' chain (as GNU's
/// define_error does) so `condition-case' catches it by parent.
fn json_cond_chain(i: &mut Interp, id: u32) {
    let ec = i.intern("error-conditions");
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
    pos: usize,
    line: usize,
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
        self.err_sym(sym)
    }

    /// A json-family error by name, carrying GNU's (LINE COLUMN
    /// POSITION) shape — column is nil, position the 1-based char
    /// offset of the offending spot.
    fn err_sym(&mut self, sym: &str) -> PErr {
        let pos = if self.s.is_empty() { 0 } else { self.pos + 1 };
        let id = self.i.intern(sym);
        json_cond_chain(self.i, id);
        self.i.signal_data(
            id,
            vec![
                Value::Int(self.line as i128),
                Value::Nil,
                Value::Int(pos as i128),
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

    fn lit(&mut self, word: &str, v: Value) -> Result<Value, PErr> {
        for c in word.chars() {
            // GNU signals json-parse-error (not json-end-of-file) for
            // a truncated literal like "tru".
            if self.peek() != Some(c) {
                return Err(self.err(false));
            }
            self.pos += 1;
        }
        Ok(v)
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
                                                    0x10000
                                                        + ((hi - 0xd800) << 10)
                                                        + (lo - 0xdc00)
                                                } else {
                                                    return Err(self.err_sym(
                                                        "json-invalid-surrogate-error",
                                                    ));
                                                }
                                            } else {
                                                return Err(self.err_sym(
                                                    "json-invalid-surrogate-error",
                                                ));
                                            }
                                        } else {
                                            return Err(self.err_sym(
                                                "json-invalid-surrogate-error",
                                            ));
                                        }
                                    } else if (0xdc00..0xe000).contains(&hi) {
                                        // A lone low surrogate.
                                        return Err(
                                            self.err_sym("json-invalid-surrogate-error")
                                        );
                                    } else {
                                        hi
                                    };
                                    out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                                }
                                _ => {
                                    return Err(self.err_sym("json-escape-sequence-error"))
                                }
                            }
                        }
                    }
                }
                Some(c) => {
                    if (c as u32) < 0x20 {
                        return Err(self.err(false));
                    }
                    out.push(c);
                    self.pos += 1;
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
            if let Ok(n) = txt.parse::<i128>() {
                return Ok(Value::Int(n));
            }
        }
        txt.parse::<f64>()
            .map(Value::float)
            .map_err(|_| self.err(false))
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
            Ok(Value::Vec(Rc::new(RefCell::new(items))))
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
                Value::Hash(Rc::new(RefCell::new(h)))
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

fn f_json_parse_string(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let src = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let o = parse_opts(i, &a[1..], false)?;
    let mut p = Parser {
        s: src.chars().collect(),
        pos: 0,
        line: 1,
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
        let e = i.intern("json-trailing-content");
        json_cond_chain(i, e);
        return Err(i.signal_data(
            e,
            vec![Value::Int(line), Value::Nil, Value::Int(pos as i128)],
        ));
    }
    Ok(v)
}

fn f_json_parse_buffer(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let o = parse_opts(i, &a[..], false)?;
    let buf = i.current_buffer;
    let (text, point) = {
        let Some(b) = i.buffers.get(buf) else {
            return Ok(Value::Nil);
        };
        let bb = b.borrow();
        (bb.text.text(), bb.point)
    };
    let chars: Vec<char> = text.chars().collect();
    let start = point.min(chars.len());
    // Line count should reflect the position within the buffer.
    let line = 1 + chars[..start].iter().filter(|c| **c == '\n').count();
    let mut p = Parser {
        s: chars,
        pos: start,
        line,
        i,
        o: &o,
    };
    let v = p.value()?;
    if let Some(b) = p.i.buffers.get(buf) {
        b.borrow_mut().set_point(p.pos);
    }
    Ok(v)
}

fn f_json_insert(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let s = serialize(i, &a[0], &a[1..])?;
    crate::buffer::primitives::chg_insert_pt(i, &s, false)?;
    Ok(Value::Nil)
}

// ---------- serializer ----------

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

fn ser_value(i: &mut Interp, v: &Value, o: &Opts, out: &mut String) -> Result<(), Flow> {
    match v {
        Value::Nil => out.push_str("{}"),
        Value::Int(n) => out.push_str(&n.to_string()),
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(json_value_err(i, v));
            }
            let mut s = format!("{}", f);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                s.push_str(".0");
            }
            out.push_str(&s);
        }
        Value::Str(s) => esc_into(&s.borrow(), out),
        Value::Sym(id) => {
            if crate::lisp::builtins::eq_values(&o.null_obj, v) {
                out.push_str("null");
            } else if crate::lisp::builtins::eq_values(&o.false_obj, v) {
                out.push_str("false");
            } else if *id == crate::lisp::obarray::sym::T {
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
                ser_value(i, e, o, out)?;
            }
            out.push(']');
        }
        Value::Cons(_) => {
            let items = v.list_to_vec().map_err(|_| json_value_err(i, v))?;
            // plist iff first element is a keyword symbol.
            let is_plist = matches!(items.first(), Some(Value::Sym(s)) if i.symbol_name(*s).starts_with(':'));
            if is_plist {
                out.push('{');
                let mut it = items.iter();
                let mut first = true;
                while let Some(k) = it.next() {
                    let Value::Sym(id) = k else {
                        return Err(i.wrong_type_mut("symbolp", k));
                    };
                    let name = i.symbol_name(*id).trim_start_matches(':').to_string();
                    let Some(v) = it.next() else {
                        return Err(json_value_err(i, k));
                    };
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    esc_into(&name, out);
                    out.push(':');
                    ser_value(i, v, o, out)?;
                }
                out.push('}');
            } else {
                // alist: each element (KEY . VALUE) with symbol key.
                out.push('{');
                for (n, e) in items.iter().enumerate() {
                    let Value::Cons(c) = e else {
                        return Err(i.wrong_type_mut("symbolp", e));
                    };
                    let (k, val) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    let Value::Sym(id) = k else {
                        return Err(i.wrong_type_mut("symbolp", &k));
                    };
                    if n > 0 {
                        out.push(',');
                    }
                    let name = i.symbol_name(id).to_string();
                    esc_into(&name, out);
                    out.push(':');
                    ser_value(i, &val, o, out)?;
                }
                out.push('}');
            }
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
                    Value::Str(s) => s.borrow().clone(),
                    other => return Err(i.wrong_type_mut("stringp", other)),
                };
                if n > 0 {
                    out.push(',');
                }
                esc_into(&name, out);
                out.push(':');
                ser_value(i, val, o, out)?;
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
    ser_value(i, v, &o, &mut out)?;
    Ok(out)
}

fn f_json_serialize(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    Ok(Value::string(serialize(i, &a[0], &a[1..])?))
}
