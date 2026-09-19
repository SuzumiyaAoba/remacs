//! Printing Lisp objects (`prin1`-style readable syntax).

use std::fmt::Write;
use std::rc::Rc;

use super::value::Value;
use super::Interp;

/// Escape a character inside a printed string. Emacs escapes only
/// `"` and `\` — newlines and other control chars print literally.
fn escape_char_for_string(c: char, out: &mut String) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        c => out.push(c),
    }
}

impl Interp {
    /// `prin1` representation: readable, escaped.
    pub fn print_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        self.prin1_inner(v, &mut s, 0);
        s
    }

    /// Alias for `print_to_string` matching the Lisp name.
    pub fn prin1_to_string(&self, v: &Value) -> String {
        self.print_to_string(v)
    }

    /// `princ` representation: human-readable (strings unquoted).
    pub fn princ_to_string(&self, v: &Value) -> String {
        let mut s = String::new();
        self.princ_inner(v, &mut s, 0);
        s
    }

    fn prin1_inner(&self, v: &Value, out: &mut String, depth: usize) {
        if depth > 64 {
            out.push_str("##");
            return;
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
                // as a number (`\52' for the symbol "52").
                if super::reader::parse_number(&name).is_some() {
                    out.push('\\');
                }
                out.push_str(&name);
            }
            Value::Str(s) => {
                out.push('"');
                for c in s.borrow().chars() {
                    escape_char_for_string(c, out);
                }
                out.push('"');
            }
            Value::Cons(_) => self.print_list(v, out, depth),
            Value::Vec(items) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1);
                }
                out.push(']');
            }
            Value::Record(items) => {
                out.push_str("#s(");
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.prin1_inner(item, out, depth + 1);
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
                if let Some(name) = &l.name {
                    let _ = write!(out, "#<function {}>", name);
                } else {
                    out.push_str("(lambda ");
                    self.print_lambda_list(l, out);
                    out.push_str(" ...)");
                }
            }
            Value::Buffer(b) => {
                let _ = write!(out, "#<buffer {}>", b.borrow().name);
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
                let _ = write!(out, "#<window {}>", w.borrow().id);
            }
            Value::Frame(f) => {
                let _ = write!(out, "#<frame {}>", f.borrow().name);
            }
        }
    }

    fn princ_inner(&self, v: &Value, out: &mut String, depth: usize) {
        match v {
            Value::Str(s) => out.push_str(&s.borrow()),
            Value::Cons(_) => self.print_list_princ(v, out, depth),
            Value::Vec(items) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.princ_inner(item, out, depth + 1);
                }
                out.push(']');
            }
            _ => self.prin1_inner(v, out, depth),
        }
    }

    fn print_lambda_list(&self, l: &super::value::Lambda, out: &mut String) {
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

    /// If `v` is a 2-element list (QUOTE x), (FUNCTION x), (\` x),
    /// (\, x) etc., return the abbreviated prefix for `print_quoted`.
    fn quote_abbrev(&self, v: &Value) -> Option<(&'static str, Value)> {
        use super::obarray::sym;
        if !self.print_quoted() {
            return None;
        }
        if let Value::Cons(c) = v {
            let (car, cdr) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            let prefix = match car {
                Value::Sym(sym::QUOTE) => "'",
                Value::Sym(sym::FUNCTION) => "#'",
                Value::Sym(sym::BACKQUOTE) => "`",
                Value::Sym(sym::COMMA) => ",",
                Value::Sym(sym::COMMA_AT) => ",@",
                Value::Sym(sym::COMMA_DOT) => ",.",
                _ => return None,
            };
            if let Value::Cons(c2) = &cdr {
                let (cadr, cddr) = {
                    let b = c2.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if cddr.is_nil() {
                    return Some((prefix, cadr));
                }
            }
        }
        None
    }

    /// Print a (possibly dotted) list.
    fn print_list(&self, v: &Value, out: &mut String, depth: usize) {
        if let Some((prefix, inner)) = self.quote_abbrev(v) {
            out.push_str(prefix);
            self.prin1_inner(&inner, out, depth + 1);
            return;
        }
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
                    if !first {
                        out.push(' ');
                    }
                    first = false;
                    self.prin1_inner(&car, out, depth + 1);
                    cur = next;
                    n += 1;
                    if n > 1000 {
                        out.push_str(" ...");
                        return;
                    }
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.prin1_inner(&other, out, depth + 1);
                    out.push(')');
                    return;
                }
            }
        }
    }

    fn print_list_princ(&self, v: &Value, out: &mut String, depth: usize) {
        if let Some((prefix, inner)) = self.quote_abbrev(v) {
            out.push_str(prefix);
            self.princ_inner(&inner, out, depth + 1);
            return;
        }
        out.push('(');
        let mut cur = v.clone();
        let mut first = true;
        loop {
            match cur {
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    if !first {
                        out.push(' ');
                    }
                    first = false;
                    self.princ_inner(&car, out, depth + 1);
                    cur = next;
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    self.princ_inner(&other, out, depth + 1);
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
