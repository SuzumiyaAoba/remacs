//! GNU regexp syntax, group numbering and validation.

use super::{CharSet, KNOWN_POSIX};
use crate::lisp::text::{EmacsChar, EmacsText};

// ---------- parsing ----------

#[derive(Debug)]
pub struct RegexError(pub String);

struct Parser {
    chars: Vec<char>,
    originals: Vec<EmacsChar>,
    pos: usize,
    n_groups: usize,
    open_groups: Vec<usize>,
}

#[derive(Debug, Clone)]
pub(super) enum Ast {
    Empty,
    Char(EmacsChar),
    Any,
    Class(CharSet),
    Group(usize, Box<Ast>),
    ShyGroup(Box<Ast>),
    Alt(Vec<Ast>),
    Concat(Vec<Ast>),
    Repeat {
        node: Box<Ast>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
    Backref(usize),
    Anchor(char), // ^ $ ` ' b B < > l g (l/g = \_< \_>)
    SyntaxClass(u8, bool),
    CategoryClass(u8, bool), // \cX / \CX — category table lookup
}

impl Parser {
    fn last_character(&self) -> EmacsChar {
        self.originals[self.pos - 1]
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// alt := concat (\| concat)*
    fn parse_alt(&mut self) -> Result<Ast, RegexError> {
        let mut branches = vec![self.parse_concat()?];
        loop {
            if self.peek() == Some('\\') && self.chars.get(self.pos + 1) == Some(&'|') {
                self.pos += 2;
                branches.push(self.parse_concat()?);
            } else {
                break;
            }
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            Ok(Ast::Alt(branches))
        }
    }

    /// concat := repeat*
    fn parse_concat(&mut self) -> Result<Ast, RegexError> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some('\\') => match self.chars.get(self.pos + 1) {
                    Some('|') | Some(')') => break,
                    _ => items.push(self.parse_repeat()?),
                },
                _ => items.push(self.parse_repeat()?),
            }
        }
        if items.is_empty() {
            Ok(Ast::Empty)
        } else if items.len() == 1 {
            Ok(items.pop().unwrap())
        } else {
            Ok(Ast::Concat(items))
        }
    }

    /// repeat := atom (quantifier)?
    fn parse_repeat(&mut self) -> Result<Ast, RegexError> {
        let atom = self.parse_atom()?;
        let mut node = atom;
        loop {
            match self.peek() {
                Some('*') | Some('+') | Some('?') => {
                    let q = self.next().unwrap();
                    let greedy = if self.peek() == Some('?') {
                        self.pos += 1;
                        false
                    } else {
                        true
                    };
                    let (min, max) = match q {
                        '*' => (0, None),
                        '+' => (1, None),
                        _ => (0, Some(1)),
                    };
                    node = Ast::Repeat {
                        node: Box::new(node),
                        min,
                        max,
                        greedy,
                    };
                }
                Some('\\') if self.chars.get(self.pos + 1) == Some(&'{') => {
                    self.pos += 2;
                    let (minv, maxv) = self.parse_interval()?;
                    node = Ast::Repeat {
                        node: Box::new(node),
                        min: minv,
                        max: maxv,
                        greedy: true,
                    };
                }
                _ => break,
            }
        }
        Ok(node)
    }

    /// GNU GET_INTERVAL_COUNT validates overflow while reading digits,
    /// before checking EOF or the closer.  A final backslash is a
    /// separate trailing-escape error, not an invalid interval bound.
    fn interval_count(&mut self) -> Result<(usize, bool), RegexError> {
        let mut value = 0usize;
        let mut digits = false;
        loop {
            let c = self
                .peek()
                .ok_or_else(|| RegexError("Unmatched \\{".into()))?;
            if !c.is_ascii_digit() {
                return Ok((value, digits));
            }
            value = value * 10 + (c as usize - '0' as usize);
            if value > 65535 {
                return Err(RegexError("Invalid content of \\{\\}".into()));
            }
            digits = true;
            self.pos += 1;
        }
    }

    fn parse_interval(&mut self) -> Result<(usize, Option<usize>), RegexError> {
        let (min, _) = self.interval_count()?;
        let max = if self.peek() == Some(',') {
            self.pos += 1;
            let (count, digits) = self.interval_count()?;
            digits.then_some(count)
        } else {
            Some(min)
        };
        if max.is_some_and(|max| max < min) || self.next() != Some('\\') {
            return Err(RegexError("Invalid content of \\{\\}".into()));
        }
        match self.next() {
            None => return Err(RegexError("Trailing backslash".into())),
            Some('}') => {}
            _ => return Err(RegexError("Invalid content of \\{\\}".into())),
        }
        Ok((min, max))
    }

    /// atom := char | '.' | '^' | '$' | '[' class ']' | '\' escaped | '(' group ')'
    fn parse_atom(&mut self) -> Result<Ast, RegexError> {
        match self.next() {
            None => Err(RegexError("Premature end of regular expression".into())),
            Some('.') => Ok(Ast::Any),
            Some('^') => Ok(Ast::Anchor('^')),
            Some('$') => Ok(Ast::Anchor('$')),
            Some('[') => self.parse_class(),
            // Emacs syntax: bare parens are literal; groups use \( \).
            Some('(') | Some(')') => Ok(Ast::Char(self.last_character())),
            Some('\\') => self.parse_escape(),
            Some(_) => Ok(Ast::Char(self.last_character())),
        }
    }

    fn parse_escape(&mut self) -> Result<Ast, RegexError> {
        match self.next() {
            None => Err(RegexError("Trailing backslash".into())),
            Some('(') => {
                // \( ... \) group; \(?: ... \) shy group; \(?N: ... \)
                // numbered group (GNU Emacs 29+).
                if self.peek() == Some('?') && self.chars.get(self.pos + 1).is_some() {
                    self.pos += 1;
                    if self.peek() == Some(':') {
                        self.pos += 1;
                        let inner = self.parse_alt()?;
                        self.expect_close()?;
                        return Ok(Ast::ShyGroup(Box::new(inner)));
                    }
                    if !matches!(self.peek(), Some('1'..='9')) {
                        return Err(RegexError("Invalid regular expression".into()));
                    }
                    let mut g = 0i32;
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            g = g
                                .checked_mul(10)
                                .and_then(|g| g.checked_add(c as i32 - '0' as i32))
                                .ok_or_else(|| RegexError("Regular expression too big".into()))?;
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    match self.next() {
                        None => {
                            return Err(RegexError("Premature end of regular expression".into()));
                        }
                        Some(':') => {}
                        _ => return Err(RegexError("Invalid regular expression".into())),
                    }
                    let g = g as usize;
                    if self.open_groups.contains(&g) {
                        return Err(RegexError("Invalid regular expression".into()));
                    }
                    self.n_groups = self.n_groups.max(g);
                    return self.parse_group(g);
                }
                self.n_groups = self
                    .n_groups
                    .checked_add(1)
                    .filter(|n| *n <= i32::MAX as usize)
                    .ok_or_else(|| RegexError("Regular expression too big".into()))?;
                let g = self.n_groups;
                self.parse_group(g)
            }
            Some(')') => {
                // stray \) — the caller (parse_alt) should have stopped;
                // put it back conceptually by erroring.
                self.pos -= 1;
                Err(RegexError("Unmatched ) or \\)".into()))
            }
            Some('|') => {
                self.pos -= 1;
                Err(RegexError("Unmatched \\| or \\|".into()))
            }
            Some('`') => Ok(Ast::Anchor('`')),
            Some('\'') => Ok(Ast::Anchor('\'')),
            // \= — "at point": never matches in a string context.
            Some('=') => Ok(Ast::Anchor('=')),
            Some('b') => Ok(Ast::Anchor('b')),
            Some('B') => Ok(Ast::Anchor('B')),
            Some('<') => Ok(Ast::Anchor('<')),
            Some('>') => Ok(Ast::Anchor('>')),
            Some('_') => match self.peek() {
                // GNU requires a symbol-boundary selector after \_.
                Some('<') => {
                    self.pos += 1;
                    Ok(Ast::Anchor('l'))
                }
                Some('>') => {
                    self.pos += 1;
                    Ok(Ast::Anchor('g'))
                }
                None => Err(RegexError("Premature end of regular expression".into())),
                _ => Err(RegexError("Invalid regular expression".into())),
            },
            Some('{') => {
                // Even without a preceding atom GNU validates the whole
                // interval, then re-reads a valid interval as literal text.
                let start = self.pos;
                self.parse_interval()?;
                self.pos = start;
                Ok(Ast::Char('{'.into()))
            }
            Some('w') => Ok(Ast::SyntaxClass(b'w', false)),
            Some('W') => Ok(Ast::SyntaxClass(b'w', true)),
            Some('s') => {
                let code = self
                    .next()
                    .ok_or(RegexError("Premature end of regular expression".into()))?;
                Ok(Ast::SyntaxClass(code as u8, false))
            }
            Some('S') => {
                let code = self
                    .next()
                    .ok_or(RegexError("Premature end of regular expression".into()))?;
                Ok(Ast::SyntaxClass(code as u8, true))
            }
            Some('c') => {
                let code = self
                    .next()
                    .ok_or(RegexError("Premature end of regular expression".into()))?;
                Ok(Ast::CategoryClass(code as u8, false))
            }
            Some('C') => {
                let code = self
                    .next()
                    .ok_or(RegexError("Premature end of regular expression".into()))?;
                Ok(Ast::CategoryClass(code as u8, true))
            }
            Some(d @ '1'..='9') => {
                let group = d as usize - '0' as usize;
                // GNU checks the logical highest number, including holes
                // introduced by explicit numbering, plus currently open
                // groups.  A missing capture below that number is valid
                // syntax but cannot match at runtime.
                if group > self.n_groups || self.open_groups.contains(&group) {
                    return Err(RegexError("Invalid back reference".into()));
                }
                Ok(Ast::Backref(group))
            }
            Some('n') => Ok(Ast::Char('\n'.into())),
            Some('t') => Ok(Ast::Char('\t'.into())),
            Some('r') => Ok(Ast::Char('\r'.into())),
            Some('f') => Ok(Ast::Char('\x0c'.into())),
            Some('v') => Ok(Ast::Char('\x0b'.into())),
            Some('a') => Ok(Ast::Char('\x07'.into())),
            Some('e') => Ok(Ast::Char('\x1b'.into())),
            Some('d') => Ok(Ast::Char('\x7f'.into())),
            Some(_) => Ok(Ast::Char(self.last_character())), // \. \* \+ \\ etc — literal
        }
    }

    fn expect_close(&mut self) -> Result<(), RegexError> {
        // expect \)
        if self.peek() == Some('\\') && self.chars.get(self.pos + 1) == Some(&')') {
            self.pos += 2;
            Ok(())
        } else {
            // GNU re_compile_pattern: "Unmatched ( or \\(".
            Err(RegexError("Unmatched ( or \\(".into()))
        }
    }

    fn parse_group(&mut self, group: usize) -> Result<Ast, RegexError> {
        self.open_groups.push(group);
        let inner = self.parse_alt()?;
        self.expect_close()?;
        self.open_groups.pop();
        // GNU tracks the logical numbering but emits capture instructions
        // only through MAX_REGNUM, rather than allocating huge registers.
        Ok(if group <= 255 {
            Ast::Group(group, Box::new(inner))
        } else {
            Ast::ShyGroup(Box::new(inner))
        })
    }

    /// [...] char class.
    fn parse_class(&mut self) -> Result<Ast, RegexError> {
        let mut set = CharSet {
            negated: false,
            ranges: Vec::new(),
            singles: Vec::new(),
            posix: Vec::new(),
            syntax: Vec::new(),
            cats: Vec::new(),
        };
        if self.peek() == Some('^') {
            set.negated = true;
            self.pos += 1;
        }
        // `]` as first char is literal.
        let mut first = true;
        let mut no_posix_retry = false;
        let mut prev_char: Option<EmacsChar> = None;
        loop {
            match self.peek() {
                None => return Err(RegexError("Unmatched [ or [^".into())),
                Some(']') if !first => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            first = false;
            // Range?
            if prev_char.is_some()
                && self.peek() == Some('-')
                && self.chars.get(self.pos + 1) != Some(&']')
            {
                self.pos += 1;
                let hi = self.class_char()?;
                let lo = prev_char.take().unwrap();
                set.ranges.push((lo, hi));
                continue;
            }
            if let Some(c) = prev_char.take() {
                set.singles.push(c);
            }
            // POSIX [:name:] or [:^name:]
            if !no_posix_retry
                && self.peek() == Some('[')
                && self.chars.get(self.pos + 1) == Some(&':')
            {
                let save = self.pos;
                self.pos += 2;
                let neg = if self.peek() == Some('^') {
                    self.pos += 1;
                    true
                } else {
                    false
                };
                let mut name = String::new();
                let mut ok = false;
                while let Some(c) = self.peek() {
                    if c == ':' && self.chars.get(self.pos + 1) == Some(&']') {
                        self.pos += 2;
                        ok = true;
                        break;
                    }
                    if c.is_alphabetic() || c == '-' || c == '_' {
                        name.push(c);
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if ok {
                    if neg || !KNOWN_POSIX.contains(&name.as_str()) {
                        return Err(RegexError("Invalid character class name".into()));
                    }
                    set.posix.push((name, neg));
                } else {
                    // GNU `re_compile_pattern': `[:name' without a
                    // `:]]' closer degrades to literal chars — `[[:bad]]'
                    // is class `[:bad' followed by a literal `]'.
                    // `no_posix_retry' keeps the same `[' from
                    // re-entering this branch forever.
                    self.pos = save;
                    prev_char = Some('['.into());
                    no_posix_retry = true;
                }
                continue;
            }
            // Backslashes are ordinary characters inside a GNU bracket
            // expression, including before s/S/c/C.
            prev_char = Some(self.class_char()?);
        }
        if let Some(c) = prev_char {
            set.singles.push(c);
        }
        Ok(Ast::Class(set))
    }

    /// One char inside a class.  GNU: `\' is an ordinary character
    /// inside [...] (POSIX class names are handled by the caller).
    fn class_char(&mut self) -> Result<EmacsChar, RegexError> {
        self.next().ok_or(RegexError("Unmatched [ or [^".into()))?;
        Ok(self.last_character())
    }
}

/// Completed parse state; no mutable parser fields escape this stage.
pub(super) struct ParsedPattern {
    pub ast: Ast,
    pub n_groups: usize,
}

pub(super) fn parse_pattern(pattern: &EmacsText) -> Result<ParsedPattern, RegexError> {
    let originals = pattern.chars().collect::<Vec<_>>();
    let mut p = Parser {
        chars: originals
            .iter()
            .map(|c| c.unicode().unwrap_or('\u{fffd}'))
            .collect(),
        originals,
        pos: 0,
        n_groups: 0,
        open_groups: Vec::new(),
    };
    let ast = p.parse_alt()?;
    if p.pos != p.chars.len() {
        // GNU re_compile_pattern: a leftover `\)' or `\|' is a
        // paren/alternation error, not generic trailing garbage.
        if p.chars.get(p.pos) == Some(&'\\') {
            match p.chars.get(p.pos + 1) {
                Some(')') => return Err(RegexError("Unmatched ) or \\)".into())),
                Some('|') => return Err(RegexError("Unmatched \\| or \\|".into())),
                _ => {}
            }
        }
        return Err(RegexError(format!(
            "trailing garbage in regexp at {}",
            p.pos
        )));
    }
    let n_groups = p.n_groups.min(255);
    Ok(ParsedPattern { ast, n_groups })
}
