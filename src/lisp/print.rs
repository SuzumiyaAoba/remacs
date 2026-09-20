//! Printing Lisp objects (`prin1`-style readable syntax).

use std::fmt::Write;
use std::rc::Rc;

use super::Interp;
use super::value::Value;

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines and other control chars print literally
/// unless `print-escape-newlines`/`print-escape-multibyte` are set.
fn escape_char_for_string(c: char, out: &mut String, nl: bool, mb: bool) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        '\n' if nl => out.push_str("\\n"),
        c if mb && (c as u32) > 0x7f => {
            let _ = write!(out, "\\x{:x}", c as u32);
        }
        c => out.push(c),
    }
}

/// Does `c` need a backslash escape inside a printed symbol name?
/// Emacs escapes the chars that would otherwise terminate or
/// re-interpret the symbol token.
fn sym_char_needs_escape(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '\'' | '"' | '`' | ',' | ';' | '#' | '\\'
    ) || c.is_whitespace()
        || (c as u32) < 0x20
        || c == '\u{7f}'
}

/// Print a symbol name, backslash-escaping special chars (`a\ b`, `\,`).
fn push_sym_name(name: &str, out: &mut String) {
    if name.is_empty() {
        out.push_str("##");
        return;
    }
    for c in name.chars() {
        if sym_char_needs_escape(c) {
            out.push('\\');
        }
        out.push(c);
    }
}

impl Interp {
    /// `prin1` representation: readable, escaped.
    pub fn print_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        self.prin1_inner(v, &mut s, 0, false);
        s
    }

    /// Alias for `print_to_string` matching the Lisp name.
    pub fn prin1_to_string(&self, v: &Value) -> String {
        self.print_to_string(v)
    }

    /// `princ` representation: human-readable (strings unquoted).
    pub fn princ_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        self.princ_inner(v, &mut s, 0, false);
        s
    }

    fn prin1_inner(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if depth > 64 {
            out.push_str("##");
            return;
        }
        // print-level: nested structure deeper than the limit prints "...".
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                match v {
                    Value::Cons(_) | Value::Vec(_) | Value::Record(_) | Value::Hash(_) => {
                        out.push_str("...");
                        return;
                    }
                    _ => {}
                }
            }
        }
        match v {
            Value::Nil => out.push_str("nil"),
            Value::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Float(f) => {
                let _ = write!(out, "{}", format_float(*f));
            }
            Value::Sym(id) => {
                if self.obarray.symbol(*id).uninterned && self.print_gensym() {
                    out.push_str("#:");
                }
                let name = self.symbol_name(*id);
                // Emacs escapes a symbol whose name would read back
                // as a number (`\52') or a bare dot (`\.').
                if super::reader::parse_number(&name).is_some() || name == "." {
                    out.push('\\');
                }
                push_sym_name(&name, out);
            }
            Value::Str(s) => {
                let nl = self.print_escape_newlines();
                let mb = self.print_escape_multibyte();
                out.push('"');
                for c in s.borrow().chars() {
                    escape_char_for_string(c, out, nl, mb);
                }
                out.push('"');
            }
            Value::Cons(_) => self.print_list(v, out, depth, bq),
            Value::Vec(items) => {
                let limit = self.print_length_limit();
                out.push('[');
                let mut n = 0usize;
                let mut truncated = false;
                for item in items.borrow().iter() {
                    if let Some(l) = limit {
                        if n >= l {
                            truncated = true;
                            break;
                        }
                    }
                    if n > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1, bq);
                    n += 1;
                }
                if truncated {
                    out.push_str(if n == 0 { "..." } else { " ..." });
                }
                out.push(']');
            }
            Value::Record(items) => {
                let rr = items.borrow();
                // Bool vectors print `#&N"bytes"' with bits packed
                // LSB-first per byte.
                let is_bv = matches!(rr.first(), Some(Value::Sym(t))
                    if self.symbol_name(*t) == "bool-vector");
                if is_bv {
                    if let Some(Value::Vec(bits)) = rr.get(1) {
                        let bits = bits.borrow();
                        let n = bits.len();
                        let _ = write!(out, "#&{}\"", n);
                        for k in 0..n.div_ceil(8) {
                            let mut byte: u32 = 0;
                            for j in 0..8 {
                                if let Some(Value::Int(b)) = bits.get(k * 8 + j) {
                                    if *b != 0 {
                                        byte |= 1 << j;
                                    }
                                }
                            }
                            match byte {
                                34 => out.push_str("\\\""),
                                92 => out.push_str("\\\\"),
                                0..=127 => out.push(byte as u8 as char),
                                _ => {
                                    let _ = write!(out, "\\{:03o}", byte);
                                }
                            }
                        }
                        out.push('"');
                        return;
                    }
                }
                drop(rr);
                let rr = items.borrow();
                out.push_str("#s(");
                for (i, item) in rr.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1, bq);
                }
                out.push(')');
            }
            Value::Hash(_) => {
                out.push_str("#s(hash-table)");
            }
            Value::Subr(s) => {
                let _ = write!(out, "#<subr {}>", s.name);
            }
            Value::Lambda(l) => {
                // Emacs 31 prints interpreted functions like
                // #[(x) (x) nil] — arglist, body forms, environment.
                out.push_str("#[");
                self.print_lambda_list(l, out);
                out.push(' ');
                // An empty body is a single implicit nil form.
                let body = if l.body.is_empty() {
                    Value::list(vec![Value::Nil])
                } else {
                    Value::list(l.body.clone())
                };
                self.prin1_inner(&body, out, depth + 1, bq);
                out.push(' ');
                match &l.env {
                    None => out.push_str("nil"),
                    Some(frame) => {
                        let env = lex_frame_to_value(self, frame);
                        self.prin1_inner(&env, out, depth + 1, bq);
                    }
                }
                out.push(']');
            }
            Value::Buffer(b) => {
                let bb = b.borrow();
                if bb.live {
                    let _ = write!(out, "#<buffer {}>", bb.name);
                } else {
                    out.push_str("#<killed buffer>");
                }
            }
            Value::Marker(m) => {
                let b = m.borrow();
                match b.buffer {
                    Some(buf) => {
                        let _ = write!(
                            out,
                            "#<marker at {} in {}>",
                            b.position + 1,
                            self.buffer_name_by_id(buf)
                        );
                    }
                    None => out.push_str("#<marker in no buffer>"),
                }
            }
            Value::Window(w) => {
                let b = w.borrow();
                let _ = write!(
                    out,
                    "#<window {} on {}>",
                    b.id,
                    self.buffer_name_by_id(b.buffer)
                );
            }
            Value::Frame(f) => {
                let _ = write!(out, "#<frame {}>", f.borrow().name);
            }
        }
    }

    fn princ_inner(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some(level) = self.print_level_limit() {
            if depth >= level {
                match v {
                    Value::Cons(_) | Value::Vec(_) | Value::Record(_) | Value::Hash(_) => {
                        out.push_str("...");
                        return;
                    }
                    _ => {}
                }
            }
        }
        match v {
            Value::Str(s) => out.push_str(&s.borrow()),
            // princ prints symbol names raw — no backslash escapes.
            Value::Sym(id) => out.push_str(&self.symbol_name(*id)),
            Value::Cons(_) => self.print_list_princ(v, out, depth, bq),
            Value::Vec(items) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.princ_inner(item, out, depth + 1, bq);
                }
                out.push(']');
            }
            _ => self.prin1_inner(v, out, depth, bq),
        }
    }

    fn print_lambda_list(&self, l: &super::value::Lambda, out: &mut String) {
        if l.required.is_empty() && l.optional.is_empty() && l.rest.is_none() {
            out.push_str("nil");
            return;
        }
        out.push('(');
        let mut first = true;
        let mut names: Vec<String> = l
            .required
            .iter()
            .map(|s| self.symbol_name(*s).to_string())
            .collect();
        if !l.optional.is_empty() {
            names.push("&optional".into());
            for o in &l.optional {
                names.push(self.symbol_name(o.sym).to_string());
            }
        }
        if let Some(r) = l.rest {
            names.push("&rest".into());
            names.push(self.symbol_name(r).to_string());
        }
        for n in names {
            if !first {
                out.push(' ');
            }
            first = false;
            out.push_str(&n);
        }
        out.push(')');
    }

    /// `print-gensym` variable (default nil): print uninterned
    /// symbols with `#:' only when non-nil.
    fn print_gensym(&self) -> bool {
        self.intern_soft("print-gensym")
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false)
    }

    /// `print-quoted` variable (default t): abbreviate
    /// (quote x) -> 'x, (function x) -> #'x, (\` x) -> `x, etc.
    fn print_quoted(&self) -> bool {
        self.intern_soft("print-quoted")
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(true)
    }

    fn print_var(&self, name: &str) -> Value {
        self.intern_soft(name)
            .filter(|id| self.bound_p(*id))
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil)
    }

    /// `print-length` (default nil): max elements printed per sequence.
    fn print_length_limit(&self) -> Option<usize> {
        self.print_var("print-length")
            .int()
            .map(|n| n.max(0) as usize)
    }

    /// `print-level` (default nil): max nesting depth before "...".
    fn print_level_limit(&self) -> Option<usize> {
        self.print_var("print-level")
            .int()
            .map(|n| n.max(0) as usize)
    }

    /// `print-escape-newlines` (default nil): escape `\n` in strings.
    fn print_escape_newlines(&self) -> bool {
        self.print_var("print-escape-newlines").truthy()
    }

    /// `print-escape-multibyte` (default nil): hex-escape non-ASCII.
    fn print_escape_multibyte(&self) -> bool {
        self.print_var("print-escape-multibyte").truthy()
    }

    /// If `v` is a 2-element list (QUOTE x), (FUNCTION x), (\` x),
    /// (\, x) etc., return the abbreviated prefix for `print_quoted`.
    fn quote_abbrev(&self, v: &Value, bq: bool) -> Option<(&'static str, Value, bool)> {
        use super::obarray::sym;
        if !self.print_quoted() {
            return None;
        }
        if let Value::Cons(c) = v {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            // `(\, x)` sugar only inside a `` ` `` context; outside the
            // comma symbol prints escaped as `\,`.
            let (prefix, inner_bq) = match car {
                Value::Sym(sym::QUOTE) => ("'", bq),
                Value::Sym(sym::FUNCTION) => ("#'", bq),
                Value::Sym(sym::BACKQUOTE) => ("`", true),
                Value::Sym(sym::COMMA) if bq => (",", bq),
                Value::Sym(sym::COMMA_AT) if bq => (",@", bq),
                Value::Sym(sym::COMMA_DOT) if bq => (",.", bq),
                _ => return None,
            };
            if let Value::Cons(c2) = &cdr {
                let (cadr, cddr) = {
                    let b = c2.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if cddr.is_nil() {
                    return Some((prefix, cadr, inner_bq));
                }
            }
        }
        None
    }

    /// Print a (possibly dotted) list.
    fn print_list(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq) {
            out.push_str(prefix);
            self.prin1_inner(&inner, out, depth + 1, inner_bq);
            return;
        }
        let limit = self.print_length_limit();
        out.push('(');
        let mut cur = v.clone();
        let mut first = true;
        let mut n = 0usize;
        loop {
            match cur {
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if let Some(l) = limit {
                        if n >= l {
                            out.push_str(if n == 0 { "..." } else { " ..." });
                            out.push(')');
                            return;
                        }
                    }
                    if !first {
                        out.push(' ');
                    }
                    first = false;
                    self.prin1_inner(&car, out, depth + 1, bq);
                    cur = next;
                    n += 1;
                    if n > 1000 {
                        out.push_str(" ...");
                        out.push(')');
                        return;
                    }
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.prin1_inner(&other, out, depth + 1, bq);
                    out.push(')');
                    return;
                }
            }
        }
    }

    fn print_list_princ(&self, v: &Value, out: &mut String, depth: usize, bq: bool) {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq) {
            out.push_str(prefix);
            self.princ_inner(&inner, out, depth + 1, inner_bq);
            return;
        }
        let limit = self.print_length_limit();
        out.push('(');
        let mut cur = v.clone();
        let mut first = true;
        let mut n = 0usize;
        loop {
            match cur {
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if let Some(l) = limit {
                        if n >= l {
                            out.push_str(if n == 0 { "..." } else { " ..." });
                            out.push(')');
                            return;
                        }
                    }
                    if !first {
                        out.push(' ');
                    }
                    first = false;
                    self.princ_inner(&car, out, depth + 1, bq);
                    cur = next;
                    n += 1;
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.princ_inner(&other, out, depth + 1, bq);
                    out.push(')');
                    return;
                }
            }
        }
    }

    fn buffer_name_by_id(&self, id: usize) -> String {
        self.buffer_name(id)
            .unwrap_or_else(|| "*killed buffer*".into())
    }
}

/// Convert a lexical environment chain to a printable alist-of-frames.
fn lex_frame_to_value(i: &Interp, frame: &Rc<crate::lisp::eval::LexFrame>) -> Value {
    let mut frames: Vec<Value> = Vec::new();
    let mut cur = Some(frame.clone());
    while let Some(f) = cur {
        let mut pairs: Vec<Value> = Vec::new();
        for (sym_id, val) in f.vars.borrow().iter() {
            pairs.push(Value::cons(Value::Sym(*sym_id), val.clone()));
        }
        pairs.sort_by_key(|v| {
            if let Value::Cons(c) = v {
                if let Value::Sym(s) = &c.borrow().car {
                    return i.symbol_name(*s).to_string();
                }
            }
            String::new()
        });
        frames.push(Value::list(pairs));
        cur = f.parent.clone();
    }
    Value::list(frames)
}

/// C `%.{prec}g` formatting: `prec` significant digits, scientific
/// notation when the decimal exponent is < -4 or >= prec, trailing
/// zeros stripped.
fn format_g(x: f64, prec: usize) -> String {
    // Get `prec` significant digits plus the decimal exponent.
    let sci = format!("{:.*e}", prec - 1, x); // "-d.ddde-12"
    let epos = sci.find('e').unwrap();
    let exp: i32 = sci[epos + 1..].parse().unwrap();
    let mant = &sci[..epos];
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if exp < -4 || exp >= prec as i32 {
        // Scientific: d.ddde±XX (exponent at least two digits).
        out.push(digits.as_bytes()[0] as char);
        let frac: String = digits[1..].trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let _ = write!(out, "{:02}", exp.abs());
    } else if exp >= 0 {
        // Fixed: digits with decimal point inside (exp < prec).
        let ip = exp as usize + 1;
        let (int_part, frac_part) = if digits.len() > ip {
            (&digits[..ip], &digits[ip..])
        } else {
            (digits.as_str(), "")
        };
        out.push_str(int_part);
        for _ in digits.len()..ip {
            out.push('0');
        }
        let frac: String = frac_part.trim_end_matches('0').into();
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
    } else {
        // 0.000ddd
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        let frac: String = digits.trim_end_matches('0').into();
        out.push_str(&frac);
    }
    out
}

/// Emacs-style float printing (`float_to_string`): shortest of
/// `%.15g`–`%.17g` that reads back to the same double.
/// `1.5`, `1.0`, `1e+20`, `1.0e+INF`, `-1.0e+INF`, `0.0e+NaN`.
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        return "0.0e+NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "1.0e+INF" } else { "-1.0e+INF" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let mut s = String::new();
    for prec in [15usize, 16, 17] {
        s = format_g(f, prec);
        if s.parse::<f64>().ok() == Some(f) {
            break;
        }
    }
    // A float must always print with a decimal point or exponent.
    if !s.contains('.') && !s.contains('e') {
        s.push_str(".0");
    }
    s
}

/// Compare two `Rc` targets for `eq` identity.
pub fn rc_ptr_eq<T>(a: &Rc<T>, b: &Rc<T>) -> bool {
    Rc::ptr_eq(a, b)
}
