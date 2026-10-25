//! Emacs-style regular expressions, implemented as a backtracking NFA
//! over `char` positions.
//!
//! Supported syntax (per Emacs):
//!   `.` `*` `+` `?` `*?` `+?` `??` `\{m\}` `\{m,n\}`
//!   `[...]` `[^...]` ranges, `[:class:]` POSIX classes
//!   `^` `$` `` \` `` `\'` `\b` `\B` `\<` `\>` `\_<` `\_>` `\w` `\W` `\sX` `\SX`
//!   `\(...\)` `\(?:...\)` `\|` `\1`–`\9` backrefs

/// A compiled regexp: instruction list + group count.
pub struct Regex {
    prog: Vec<Inst>,
    pub n_groups: usize,
    /// Extra registers (beyond the 2*(groups+1) match registers)
    /// reserved for loop progress checks.
    extra_regs: usize,
    pub case_fold: bool,
    /// Source pattern (for error messages).
    pub source: String,
}

#[derive(Debug, Clone)]
enum Inst {
    /// Match exact char `c` at `pos` (post-fold if case_fold).
    Char(char),
    /// Any char except newline.
    Any,
    /// Char in set.
    Class(CharSet),
    /// Literal string (backref expansion).
    Str(usize), // group index whose captured text must match
    /// ^ — start of buffer/string or after \n.
    Bol,
    /// $ — end of buffer/string or before \n.
    Eol,
    /// \` / \' — absolute start/end.
    Bos,
    Eos,
    /// \= — matches only at `point` (buffer point); never in strings.
    AtPoint,
    /// \b \B \< \> — word boundaries.
    WordB,
    NotWordB,
    WordStart,
    WordEnd,
    /// \_< \_> — symbol boundaries (word OR symbol syntax).
    SymStart,
    SymEnd,
    /// Split: try `a` first (greedy), else `b`.
    Split(usize, usize),
    Jmp(usize),
    /// Jump to `target` unless regs[`n`] == current position — a
    /// repetition whose body matched empty stops looping (like GNU,
    /// which doesn't re-enter a zero-progress iteration).
    JmpUnlessSame(usize, usize),
    /// Record position into register n (2*g = start, 2*g+1 = end).
    Save(usize),
    Match,
}

#[derive(Debug, Clone)]
struct CharSet {
    negated: bool,
    ranges: Vec<(char, char)>,
    singles: Vec<char>,
    posix: Vec<(String, bool)>, // (class-name, negated?)
    syntax: Vec<(u8, bool)>,    // (syntax code, negated?)
    cats: Vec<(u8, bool)>,      // (category bit, negated?) — \cX / \CX
}

impl CharSet {
    fn base_match(&self, c: char, syn: SynFn, cat: CatFn, pos: usize) -> bool {
        self.singles.contains(&c)
            || self.ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi)
            || self
                .syntax
                .iter()
                .any(|(code, neg)| syntax_match(*code, c, syn, pos) != *neg)
            || self.cats.iter().any(|(bit, neg)| cat(c, *bit) != *neg)
    }

    fn contains(&self, c: char, case_fold: bool, syn: SynFn, cat: CatFn, pos: usize) -> bool {
        let mut found = self.base_match(c, syn, cat, pos)
            || self
                .posix
                .iter()
                .any(|(name, neg)| syn.posix(name, c, case_fold) != *neg);
        if !found && case_fold {
            found = syn.case_match(c, &mut |variant| self.base_match(variant, syn, cat, pos));
        }
        found != self.negated
    }
}

/// GNU `char-syntax` values for the *standard* syntax table (dumped from
/// GNU Emacs: `;` is punctuation there, `\n` is whitespace, `$`/`%` word,
/// `&` `*` `+` `-` `/` `<` `=` `>` `|` `_` symbol, `{[`/`]}` paren).
pub fn syntax_code(c: char) -> u8 {
    match c {
        ' ' | '\t' | '\n' | '\x0c' | '\r' => b' ',
        '(' | '[' | '{' => b'(',
        ')' | ']' | '}' => b')',
        '"' => b'"',
        '\\' => b'\\',
        '$' | '%' => b'w',
        '&' | '*' | '+' | '-' | '/' | '<' | '=' | '>' | '|' | '_' => b'_',
        c if c.is_alphanumeric() => b'w',
        c if c.is_ascii() => b'.',
        c if c.is_whitespace() => b' ',
        _ => b'.',
    }
}

fn syntax_match(code: u8, c: char, syn: SynFn, pos: usize) -> bool {
    let sc = syn.code_at(c, pos);
    // GNU accepts `-` as an alias for the whitespace class.
    sc == code || (code == b'-' && sc == b' ')
}

/// Table-free fallback for standalone syntax callbacks and early loadup.
/// The interpreter overrides this with its GNU character tables.
fn posix_match(name: &str, c: char, syn: &dyn Fn(char) -> u8, fold: bool) -> bool {
    match name {
        "alnum" => c.is_alphanumeric(),
        "alpha" => c.is_alphabetic(),
        "digit" => c.is_ascii_digit(),
        "xdigit" => c.is_ascii_hexdigit(),
        "upper" => c.is_uppercase() || (fold && c.is_lowercase()),
        "lower" => c.is_lowercase() || (fold && c.is_uppercase()),
        "space" => syn(c) == b' ',
        "punct" => {
            if c.is_ascii() {
                c.is_ascii_punctuation()
            } else {
                syn(c) != b'w'
            }
        }
        "graph" => !c.is_whitespace() && !c.is_control(),
        "print" => !c.is_control(),
        // GNU regex.c ISCNTRL: `(c) < \' \'` — ASCII controls only;
        // DEL (0x7f) and C1 are NOT in the class.
        "cntrl" => (c as u32) < 0x20,
        "blank" => c == ' ' || c == '\t',
        "word" => syn(c) == b'w',
        // GNU's nonstandard extras (regex.c `RE_CHAR_CLASSES').
        "ascii" => c.is_ascii(),
        "nonascii" => !c.is_ascii(),
        "unibyte" => (c as u32) <= 0xFF,
        "multibyte" => (c as u32) > 0xFF,
        _ => false,
    }
}

/// Class names GNU accepts inside `[:...:]'.
const KNOWN_POSIX: &[&str] = &[
    "alnum",
    "alpha",
    "digit",
    "xdigit",
    "upper",
    "lower",
    "space",
    "punct",
    "graph",
    "print",
    "cntrl",
    "blank",
    "word",
    "ascii",
    "nonascii",
    "unibyte",
    "multibyte",
];

/// Syntax plus character classification supplied by the caller.  Plain
/// syntax callbacks remain usable by standalone engine clients; the editor
/// supplies its live GNU syntax, case and Unicode category tables.
pub trait Syntax {
    fn code(&self, c: char) -> u8;

    fn code_at(&self, c: char, _pos: usize) -> u8 {
        self.code(c)
    }

    fn case_eq(&self, a: char, b: char) -> bool {
        char_eq(a, b, true)
    }

    fn case_match(&self, c: char, test: &mut dyn FnMut(char) -> bool) -> bool {
        c.to_lowercase()
            .chain(c.to_uppercase())
            .any(|variant| variant != c && test(variant))
    }

    fn posix(&self, name: &str, c: char, fold: bool) -> bool {
        posix_match(name, c, &|ch| self.code(ch), fold)
    }
}

impl<F: Fn(char) -> u8 + ?Sized> Syntax for F {
    fn code(&self, c: char) -> u8 {
        self(c)
    }
}

pub type SynFn<'a> = &'a dyn Syntax;

/// GNU character.c and regex-emacs.c predicates.  CATEGORY is the raw
/// general-category enum; UPPER/LOWER come from the current case table.
pub(crate) fn class_matches(
    name: &str,
    c: char,
    syntax: u8,
    category: Option<i128>,
    upper: bool,
    lower: bool,
) -> bool {
    let cp = crate::lisp::value::lisp_char_code(c);
    match name {
        "ascii" => cp < 128,
        "nonascii" => cp >= 128,
        "unibyte" => cp <= 255,
        "multibyte" => cp > 255,
        "digit" => c.is_ascii_digit(),
        "xdigit" => c.is_ascii_hexdigit(),
        "cntrl" => cp < 32,
        "space" => syntax == b' ',
        "word" => syntax == b'w',
        "punct" => {
            if c.is_ascii() {
                c.is_ascii_punctuation()
            } else {
                syntax != b'w'
            }
        }
        "upper" => upper,
        "lower" => lower,
        "blank" => {
            if c.is_ascii() {
                matches!(c, ' ' | '\t')
            } else {
                category == Some(23)
            }
        }
        "alpha" | "alnum" => {
            if c.is_ascii() {
                c.is_ascii_alphabetic() || (name == "alnum" && c.is_ascii_digit())
            } else {
                matches!(category, Some(1..=8 | 10)) || (name == "alnum" && category == Some(9))
            }
        }
        "graph" => {
            if cp <= 255 {
                cp > 32 && !(127..=160).contains(&cp)
            } else {
                category.is_some() && !matches!(category, Some(23..=26 | 28 | 30))
            }
        }
        "print" => {
            if cp <= 255 {
                cp >= 32 && !(127..=159).contains(&cp)
            } else {
                category.is_some() && !matches!(category, Some(26 | 28 | 30))
            }
        }
        _ => false,
    }
}

/// Category-table lookup for `\cX'/`\CX': `(c, BIT)' where BIT is the
/// category character's code (e.g. `j' = 106 for Japanese).
pub type CatFn<'a> = &'a dyn Fn(char, u8) -> bool;

/// No-categories fallback for callers without a category table.
pub fn no_cat(_: char, _: u8) -> bool {
    false
}

/// `syntax_code' (standard table) as a `SynFn' for callers without a
/// buffer syntax table.
pub fn std_syntax(c: char) -> u8 {
    syntax_code(c)
}

fn is_word_char(c: char, syn: SynFn, pos: usize) -> bool {
    syn.code_at(c, pos) == b'w'
}

fn is_sym_char(c: char, syn: SynFn, pos: usize) -> bool {
    matches!(syn.code_at(c, pos), b'w' | b'_')
}

// ---------- parsing ----------

#[derive(Debug)]
pub struct RegexError(pub String);

struct Parser {
    chars: Vec<char>,
    pos: usize,
    n_groups: usize,
    open_groups: Vec<usize>,
}

#[derive(Debug, Clone)]
enum Ast {
    Empty,
    Char(char),
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
            Some('(') | Some(')') => Ok(Ast::Char(self.chars[self.pos - 1])),
            Some('\\') => self.parse_escape(),
            Some(c) => Ok(Ast::Char(c)),
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
                Ok(Ast::Char('{'))
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
            Some('n') => Ok(Ast::Char('\n')),
            Some('t') => Ok(Ast::Char('\t')),
            Some('r') => Ok(Ast::Char('\r')),
            Some('f') => Ok(Ast::Char('\x0c')),
            Some('v') => Ok(Ast::Char('\x0b')),
            Some('a') => Ok(Ast::Char('\x07')),
            Some('e') => Ok(Ast::Char('\x1b')),
            Some('d') => Ok(Ast::Char('\x7f')),
            Some(c) => Ok(Ast::Char(c)), // \. \* \+ \\ etc — literal
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
        let mut prev_char: Option<char> = None;
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
                    prev_char = Some('[');
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
    fn class_char(&mut self) -> Result<char, RegexError> {
        self.next().ok_or(RegexError("Unmatched [ or [^".into()))
    }
}

// ---------- codegen ----------

struct Codegen {
    prog: Vec<Inst>,
    /// Next free register slot for loop progress checks.
    next_reg: usize,
}

impl Codegen {
    fn push(&mut self, i: Inst) -> usize {
        self.prog.push(i);
        self.prog.len() - 1
    }

    fn emit(&mut self, node: &Ast) {
        match node {
            Ast::Empty => {}
            Ast::Char(c) => {
                self.push(Inst::Char(*c));
            }
            Ast::Any => {
                self.push(Inst::Any);
            }
            Ast::Class(s) => {
                self.push(Inst::Class(s.clone()));
            }
            Ast::Backref(g) => {
                self.push(Inst::Str(*g));
            }
            Ast::Anchor('^') => {
                self.push(Inst::Bol);
            }
            Ast::Anchor('$') => {
                self.push(Inst::Eol);
            }
            Ast::Anchor('`') => {
                self.push(Inst::Bos);
            }
            Ast::Anchor('\'') => {
                self.push(Inst::Eos);
            }
            Ast::Anchor('=') => {
                self.push(Inst::AtPoint);
            }
            Ast::Anchor('b') => {
                self.push(Inst::WordB);
            }
            Ast::Anchor('B') => {
                self.push(Inst::NotWordB);
            }
            Ast::Anchor('<') => {
                self.push(Inst::WordStart);
            }
            Ast::Anchor('>') => {
                self.push(Inst::WordEnd);
            }
            Ast::Anchor('l') => {
                self.push(Inst::SymStart);
            }
            Ast::Anchor('g') => {
                self.push(Inst::SymEnd);
            }
            Ast::Anchor(_) => {}
            Ast::SyntaxClass(code, neg) => {
                let set = CharSet {
                    negated: false,
                    ranges: Vec::new(),
                    singles: Vec::new(),
                    posix: Vec::new(),
                    syntax: vec![(*code, *neg)],
                    cats: Vec::new(),
                };
                self.push(Inst::Class(set));
            }
            Ast::CategoryClass(code, neg) => {
                let set = CharSet {
                    negated: false,
                    ranges: Vec::new(),
                    singles: Vec::new(),
                    posix: Vec::new(),
                    syntax: Vec::new(),
                    cats: vec![(*code, *neg)],
                };
                self.push(Inst::Class(set));
            }
            Ast::Group(g, inner) => {
                self.push(Inst::Save(2 * g));
                self.emit(inner);
                self.push(Inst::Save(2 * g + 1));
            }
            Ast::ShyGroup(inner) => self.emit(inner),
            Ast::Concat(items) => {
                for it in items {
                    self.emit(it);
                }
            }
            Ast::Alt(branches) => {
                // chain of splits: split(a_code, next_split)
                let mut jmp_fixups = Vec::new();
                for (k, br) in branches.iter().enumerate() {
                    if k + 1 < branches.len() {
                        let split = self.push(Inst::Split(0, 0));
                        let a_start = self.prog.len();
                        self.emit(br);
                        let jmp = self.push(Inst::Jmp(0));
                        jmp_fixups.push(jmp);
                        let b_start = self.prog.len();
                        self.prog[split] = Inst::Split(a_start, b_start);
                    } else {
                        self.emit(br);
                    }
                }
                let end = self.prog.len();
                for j in jmp_fixups {
                    self.prog[j] = Inst::Jmp(end);
                }
            }
            Ast::Repeat {
                node,
                min,
                max,
                greedy,
            } => {
                // min copies mandatory
                for _ in 0..*min {
                    self.emit(node);
                }
                match max {
                    None => {
                        // loop: split(body, end) with a progress-guarded
                        // back-jump — an iteration that consumed no
                        // input exits the loop instead of spinning.
                        let k = self.next_reg;
                        self.next_reg += 1;
                        let head = self.push(Inst::Save(k));
                        let split = self.push(Inst::Split(0, 0));
                        let body_start = self.prog.len();
                        self.emit(node);
                        self.push(Inst::JmpUnlessSame(k, head));
                        let end = self.prog.len();
                        self.prog[split] = if *greedy {
                            Inst::Split(body_start, end)
                        } else {
                            Inst::Split(end, body_start)
                        };
                    }
                    Some(m) => {
                        let optional = m - min;
                        let mut splits = Vec::new();
                        for _ in 0..optional {
                            let s = self.push(Inst::Split(0, 0));
                            splits.push(s);
                            let body_start = self.prog.len();
                            self.emit(node);
                            let _ = body_start;
                        }
                        let end = self.prog.len();
                        for s in splits {
                            self.prog[s] = if *greedy {
                                Inst::Split(s + 1, end)
                            } else {
                                Inst::Split(end, s + 1)
                            };
                        }
                    }
                }
            }
        }
    }
}

/// Compile a pattern.
pub fn compile(pattern: &str) -> Result<Regex, RegexError> {
    compile_case(pattern, true)
}

pub fn compile_case(pattern: &str, case_fold: bool) -> Result<Regex, RegexError> {
    let mut p = Parser {
        chars: pattern.chars().collect(),
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
    let mut cg = Codegen {
        prog: Vec::with_capacity(64),
        next_reg: 2 * (n_groups + 1),
    };
    cg.push(Inst::Save(0));
    cg.emit(&ast);
    cg.push(Inst::Save(1));
    cg.push(Inst::Match);
    Ok(Regex {
        prog: cg.prog,
        n_groups,
        extra_regs: cg.next_reg - 2 * (n_groups + 1),
        case_fold,
        source: pattern.to_string(),
    })
}

// ---------- matching ----------

pub struct MatchState {
    /// Registers: 2*g start, 2*g+1 end (char positions).
    pub regs: Vec<Option<usize>>,
    pub end: usize,
}

pub type Regs = Vec<Option<usize>>;

fn char_eq(a: char, b: char, fold: bool) -> bool {
    if a == b {
        return true;
    }
    if !fold {
        return false;
    }
    a.to_lowercase().eq(b.to_lowercase()) || a.to_uppercase().eq(b.to_uppercase())
}

/// Recursive matcher. Returns final regs on success.
/// `point` is the buffer point for `\=' (NO_POINT in string searches).
/// `limit`, when not usize::MAX, caps how far into the text the match
/// may consume — GNU `re_match's STOP: a backward search's candidate
/// must end at or before the search start position.
/// `end_cap`, when not usize::MAX, requires the match to end exactly
/// there (POSIX longest-match and `looking-back' probing).
fn run(
    re: &Regex,
    text: &[char],
    mut pc: usize,
    mut sp: usize,
    mut regs: Regs,
    depth: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    limit: usize,
    end_cap: usize,
) -> Option<Regs> {
    if depth > 10_000 {
        return None;
    }
    // Reads never go past the limit, the end cap, or the text itself.
    let cap = limit.min(end_cap).min(text.len());
    loop {
        match &re.prog[pc] {
            Inst::Char(c) => {
                if sp < cap && (text[sp] == *c || (re.case_fold && syn.case_eq(text[sp], *c))) {
                    sp += 1;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Any => {
                if sp < cap && text[sp] != '\n' {
                    sp += 1;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Class(set) => {
                if sp < cap && set.contains(text[sp], re.case_fold, syn, cat, sp) {
                    sp += 1;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Str(g) => {
                let (s, e) = match (regs.get(2 * g), regs.get(2 * g + 1)) {
                    (Some(Some(s)), Some(Some(e))) => (*s, *e),
                    _ => return None,
                };
                let n = e - s;
                if sp + n <= cap
                    && (0..n).all(|k| {
                        (text[sp + k] == text[s + k]
                            || (re.case_fold && syn.case_eq(text[sp + k], text[s + k])))
                    })
                {
                    sp += n;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Bol => {
                if sp == 0 || text[sp - 1] == '\n' {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Eol => {
                if sp >= text.len() || text[sp] == '\n' {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Bos => {
                if sp == 0 {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Eos => {
                if sp >= text.len() {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::WordB => {
                let before = sp > 0 && is_word_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_word_char(text[sp], syn, sp);
                if before != after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::NotWordB => {
                let before = sp > 0 && is_word_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_word_char(text[sp], syn, sp);
                if before == after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::WordStart => {
                let before = sp > 0 && is_word_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_word_char(text[sp], syn, sp);
                if !before && after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::WordEnd => {
                let before = sp > 0 && is_word_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_word_char(text[sp], syn, sp);
                if before && !after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::SymStart => {
                let before = sp > 0 && is_sym_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_sym_char(text[sp], syn, sp);
                if !before && after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::SymEnd => {
                let before = sp > 0 && is_sym_char(text[sp - 1], syn, sp - 1);
                let after = sp < text.len() && is_sym_char(text[sp], syn, sp);
                if before && !after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::AtPoint => {
                if sp == point {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Split(a, b) => {
                if let Some(r) = run(
                    re,
                    text,
                    *a,
                    sp,
                    regs.clone(),
                    depth + 1,
                    syn,
                    cat,
                    point,
                    limit,
                    end_cap,
                ) {
                    return Some(r);
                }
                pc = *b;
            }
            Inst::Jmp(x) => pc = *x,
            Inst::JmpUnlessSame(n, target) => {
                if regs[*n] == Some(sp) {
                    pc += 1;
                } else {
                    pc = *target;
                }
            }
            Inst::Save(n) => {
                regs[*n] = Some(sp);
                pc += 1;
            }
            // end_cap == NO_END means "no end constraint".
            Inst::Match => return (end_cap == usize::MAX || sp == end_cap).then_some(regs),
        }
    }
}

/// `point` value for string searches and other contexts where the
/// `\=' anchor can never match (GNU: \= only matches at buffer point).
pub const NO_POINT: usize = usize::MAX;

/// Try to match at exactly `pos`. Returns regs on success.
pub fn match_at(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    match_at_end(re, text, pos, usize::MAX, syn, cat, point)
}

/// Try to match at `pos` requiring the match to end exactly at `end`.
pub fn match_at_end(
    re: &Regex,
    text: &[char],
    pos: usize,
    end: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    match_at_within(re, text, pos, usize::MAX, end, syn, cat, point)
}

/// Try to match at `pos` on the truncated text `text[..limit]`
/// (usize::MAX = no limit): the match may end anywhere at or before
/// `limit` — GNU `re_match' with STOP, used for backward searches.
/// `end_cap`, when not usize::MAX, additionally requires the match to
/// end exactly there.
pub fn match_at_within(
    re: &Regex,
    text: &[char],
    pos: usize,
    limit: usize,
    end_cap: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    let n = 2 * (re.n_groups + 1);
    let mut regs = run(
        re,
        text,
        0,
        pos,
        vec![None; n + re.extra_regs],
        0,
        syn,
        cat,
        point,
        limit,
        end_cap,
    )?;
    regs.truncate(n);
    Some(regs)
}

/// POSIX match at `pos`: the LONGEST match whose end is <= `limit`
/// (usize::MAX = no limit). Group/subexp tie-breaks stay first-match.
pub fn posix_match_at(
    re: &Regex,
    text: &[char],
    pos: usize,
    limit: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    let mut e = limit.min(text.len());
    while e >= pos {
        if let Some(regs) = match_at_end(re, text, pos, e, syn, cat, point) {
            return Some(regs);
        }
        if e == 0 {
            break;
        }
        e -= 1;
    }
    None
}

/// Search forward from `pos`; returns (match_start, match_end) of group 0.
pub fn search(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    let mut p = pos;
    while p <= text.len() {
        if let Some(regs) = match_at(re, text, p, syn, cat, point) {
            let s = regs[0].unwrap_or(p);
            let e = regs[1].unwrap_or(p);
            return Some((s, e));
        }
        p += 1;
    }
    None
}

/// Search backward from `pos`: GNU `re_search_backward' scans the
/// start position downward and takes the first hit — the match is
/// tried with STOP = pos, so it never consumes past `pos`.
pub fn search_backward(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    let mut p = pos.min(text.len());
    loop {
        if let Some(regs) = match_at_within(re, text, p, pos, usize::MAX, syn, cat, point) {
            let s = regs[0].unwrap_or(p);
            let e = regs[1].unwrap_or(p);
            return Some((s, e));
        }
        if p == 0 {
            return None;
        }
        p -= 1;
    }
}

/// Search with full register info; `posix` selects GNU's leftmost-
/// longest (`re_search` posix mode) instead of first-match.
pub fn search_full(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    let mut p = pos;
    while p <= text.len() {
        let hit = if posix {
            posix_match_at(re, text, p, usize::MAX, syn, cat, point)
        } else {
            match_at(re, text, p, syn, cat, point)
        };
        if let Some(regs) = hit {
            return Some(regs);
        }
        p += 1;
    }
    None
}

/// Backward search returning full regs: the match with the greatest
/// start whose end is at or before `pos` (GNU semantics).
pub fn search_backward_full(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    let mut p = pos.min(text.len());
    loop {
        // GNU re_search_backward: each candidate start is matched with
        // STOP = pos — the match is cut off at `pos` rather than being
        // allowed to extend past it and then rejected.
        let hit = if posix {
            posix_match_at(re, text, p, pos, syn, cat, point)
        } else {
            match_at_within(re, text, p, pos, usize::MAX, syn, cat, point)
        };
        if let Some(regs) = hit {
            return Some(regs);
        }
        if p == 0 {
            return None;
        }
        p -= 1;
    }
}

/// `string-match` / `looking-at` need the text as chars; helpers to
/// convert buffer substrings are in the caller.

/// Simple `looking-at` helper: match at pos.
pub fn looking_at(
    re: &Regex,
    text: &[char],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    if posix {
        posix_match_at(re, text, pos, usize::MAX, syn, cat, point)
    } else {
        match_at(re, text, pos, syn, cat, point)
    }
}
