//! Emacs-style regular expressions, implemented as a backtracking NFA
//! over `char` positions.
//!
//! Supported syntax (per Emacs):
//!   `.` `*` `+` `?` `*?` `+?` `??` `\{m\}` `\{m,n\}`
//!   `[...]` `[^...]` ranges, `[:class:]` POSIX classes, `\sX` in classes
//!   `^` `$` `` \` `` `\'` `\b` `\B` `\<` `\>` `\w` `\W` `\sX` `\SX`
//!   `\(...\)` `\(?:...\)` `\|` `\1`–`\9` backrefs

/// A compiled regexp: instruction list + group count.
pub struct Regex {
    prog: Vec<Inst>,
    pub n_groups: usize,
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
    /// \b \B \< \> — word boundaries.
    WordB,
    NotWordB,
    WordStart,
    WordEnd,
    /// Split: try `a` first (greedy), else `b`.
    Split(usize, usize),
    Jmp(usize),
    /// Record position into register n (2*g = start, 2*g+1 = end).
    Save(usize),
    Match,
}

#[derive(Debug, Clone)]
struct CharSet {
    negated: bool,
    ranges: Vec<(char, char)>,
    singles: Vec<char>,
    posix: Vec<(&'static str, bool)>, // (class-name, negated?)
    syntax: Vec<(u8, bool)>,          // (syntax code, negated?)
}

impl CharSet {
    fn base_match(&self, c: char) -> bool {
        self.singles.contains(&c)
            || self.ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi)
            || self
                .posix
                .iter()
                .any(|(name, neg)| posix_match(name, c) != *neg)
            || self
                .syntax
                .iter()
                .any(|(code, neg)| syntax_match(*code, c) != *neg)
    }

    fn contains(&self, c: char, case_fold: bool) -> bool {
        let mut found = self.base_match(c);
        if !found && case_fold {
            for lc in c.to_lowercase() {
                if lc != c && self.base_match(lc) {
                    found = true;
                    break;
                }
            }
            if !found {
                for uc in c.to_uppercase() {
                    if uc != c && self.base_match(uc) {
                        found = true;
                        break;
                    }
                }
            }
        }
        found != self.negated
    }
}

/// GNU `char-syntax` values for the standard syntax table.
pub fn syntax_code(c: char) -> u8 {
    match c {
        ' ' | '\t' | '\x0c' => b' ',
        '\n' => b'>',
        '(' | '[' => b'(',
        ')' | ']' => b')',
        '"' => b'"',
        '\'' | '`' | ',' | '#' => b'\'',
        ';' => b'<',
        '\\' => b'\\',
        c if c.is_alphanumeric() => b'w',
        c if c.is_ascii() => b'_',
        c if c.is_whitespace() => b' ',
        _ => b'.',
    }
}

fn syntax_match(code: u8, c: char) -> bool {
    let sc = syntax_code(c);
    // GNU accepts `-` as an alias for the whitespace class.
    sc == code || (code == b'-' && sc == b' ')
}

fn posix_match(name: &str, c: char) -> bool {
    match name {
        "alnum" | "digit" | "xdigit" | "alpha" | "upper" | "lower" | "space" | "punct"
        | "graph" | "print" | "cntrl" | "blank" | "word" => {
            let r = match name {
                "alnum" => c.is_alphanumeric(),
                "alpha" => c.is_alphabetic(),
                "digit" => c.is_ascii_digit(),
                "xdigit" => c.is_ascii_hexdigit(),
                "upper" => c.is_uppercase(),
                "lower" => c.is_lowercase(),
                "space" => c.is_whitespace(),
                "punct" => c.is_ascii_punctuation(),
                "graph" => !c.is_whitespace() && !c.is_control(),
                "print" => !c.is_control(),
                "cntrl" => c.is_control(),
                "blank" => c == ' ' || c == '\t',
                "word" => syntax_code(c) == b'w',
                _ => false,
            };
            r
        }
        _ => false,
    }
}

fn is_word_char(c: char) -> bool {
    syntax_code(c) == b'w'
}

// ---------- parsing ----------

#[derive(Debug)]
pub struct RegexError(pub String);

struct Parser {
    chars: Vec<char>,
    pos: usize,
    n_groups: usize,
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
    Anchor(char), // ^ $ ` ' b B < >
    SyntaxClass(u8, bool),
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
                    // interval \{m,n\}
                    let save = self.pos;
                    self.pos += 2;
                    let mut min = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            min.push(c);
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let mut max = None;
                    if self.peek() == Some(',') {
                        self.pos += 1;
                        let mut mx = String::new();
                        while let Some(c) = self.peek() {
                            if c.is_ascii_digit() {
                                mx.push(c);
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                        max = if mx.is_empty() {
                            None
                        } else {
                            Some(mx.parse::<usize>().unwrap_or(usize::MAX))
                        };
                    } else if !min.is_empty() {
                        max = min.parse::<usize>().ok();
                    }
                    if self.peek() == Some('\\') && self.chars.get(self.pos + 1) == Some(&'}') {
                        self.pos += 2;
                        let minv = min.parse::<usize>().unwrap_or(0);
                        node = Ast::Repeat {
                            node: Box::new(node),
                            min: minv,
                            max,
                            greedy: true,
                        };
                    } else {
                        // not a valid interval — restore and treat \{ as literal
                        self.pos = save;
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(node)
    }

    /// atom := char | '.' | '^' | '$' | '[' class ']' | '\' escaped | '(' group ')'
    fn parse_atom(&mut self) -> Result<Ast, RegexError> {
        match self.next() {
            None => Err(RegexError("unexpected end".into())),
            Some('.') => Ok(Ast::Any),
            Some('^') => Ok(Ast::Anchor('^')),
            Some('$') => Ok(Ast::Anchor('$')),
            Some('[') => self.parse_class(),
            Some('(') => Err(RegexError("unescaped ( — use \\( for groups".into())),
            Some(')') => Err(RegexError("unescaped )".into())),
            Some('\\') => self.parse_escape(),
            Some(c) => Ok(Ast::Char(c)),
        }
    }

    fn parse_escape(&mut self) -> Result<Ast, RegexError> {
        match self.next() {
            None => Err(RegexError("trailing backslash".into())),
            Some('(') => {
                // \( ... \) group; check for \(?:
                if self.peek() == Some('?') && self.chars.get(self.pos + 1) == Some(&':') {
                    self.pos += 2;
                    let inner = self.parse_alt()?;
                    self.expect_close()?;
                    Ok(Ast::ShyGroup(Box::new(inner)))
                } else {
                    self.n_groups += 1;
                    let g = self.n_groups;
                    let inner = self.parse_alt()?;
                    self.expect_close()?;
                    Ok(Ast::Group(g, Box::new(inner)))
                }
            }
            Some(')') => {
                // stray \) — the caller (parse_alt) should have stopped;
                // put it back conceptually by erroring.
                self.pos -= 1;
                Err(RegexError("unmatched \\)".into()))
            }
            Some('|') => {
                self.pos -= 1;
                Err(RegexError("unmatched \\|".into()))
            }
            Some('`') => Ok(Ast::Anchor('`')),
            Some('\'') => Ok(Ast::Anchor('\'')),
            Some('b') => Ok(Ast::Anchor('b')),
            Some('B') => Ok(Ast::Anchor('B')),
            Some('<') => Ok(Ast::Anchor('<')),
            Some('>') => Ok(Ast::Anchor('>')),
            Some('w') => Ok(Ast::SyntaxClass(b'w', false)),
            Some('W') => Ok(Ast::SyntaxClass(b'w', true)),
            Some('s') => {
                let code = self.next().ok_or(RegexError("\\s without code".into()))?;
                Ok(Ast::SyntaxClass(code as u8, false))
            }
            Some('S') => {
                let code = self.next().ok_or(RegexError("\\S without code".into()))?;
                Ok(Ast::SyntaxClass(code as u8, true))
            }
            Some(d @ '1'..='9') => Ok(Ast::Backref(d as usize - '0' as usize)),
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
            Err(RegexError("unterminated group".into()))
        }
    }

    /// [...] char class.
    fn parse_class(&mut self) -> Result<Ast, RegexError> {
        let mut set = CharSet {
            negated: false,
            ranges: Vec::new(),
            singles: Vec::new(),
            posix: Vec::new(),
            syntax: Vec::new(),
        };
        if self.peek() == Some('^') {
            set.negated = true;
            self.pos += 1;
        }
        // `]` as first char is literal.
        let mut first = true;
        let mut prev_char: Option<char> = None;
        loop {
            match self.peek() {
                None => return Err(RegexError("unterminated char class".into())),
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
            if self.peek() == Some('[') && self.chars.get(self.pos + 1) == Some(&':') {
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
                    set.posix.push((Box::leak(name.into_boxed_str()), neg));
                } else {
                    self.pos = save;
                    prev_char = Some('[');
                }
                continue;
            }
            // \sX syntax class inside [].
            if self.peek() == Some('\\')
                && matches!(self.chars.get(self.pos + 1), Some('s') | Some('S'))
            {
                self.pos += 1;
                let neg = self.next() == Some('S');
                let code = self.next().ok_or(RegexError("bad \\s in class".into()))?;
                set.syntax.push((code as u8, neg));
                continue;
            }
            prev_char = Some(self.class_char()?);
        }
        if let Some(c) = prev_char {
            set.singles.push(c);
        }
        Ok(Ast::Class(set))
    }

    /// One char inside a class (handles \-escapes for control chars).
    fn class_char(&mut self) -> Result<char, RegexError> {
        match self.next() {
            None => Err(RegexError("unterminated class".into())),
            Some('\\') => match self.next() {
                Some('n') => Ok('\n'),
                Some('t') => Ok('\t'),
                Some('r') => Ok('\r'),
                Some('f') => Ok('\x0c'),
                Some('v') => Ok('\x0b'),
                Some('a') => Ok('\x07'),
                Some('e') => Ok('\x1b'),
                Some('d') => Ok('\x7f'),
                Some(c) => Ok(c),
                None => Err(RegexError("trailing \\".into())),
            },
            Some(c) => Ok(c),
        }
    }
}

// ---------- codegen ----------

struct Codegen {
    prog: Vec<Inst>,
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
            Ast::Anchor(_) => {}
            Ast::SyntaxClass(code, neg) => {
                let set = CharSet {
                    negated: false,
                    ranges: Vec::new(),
                    singles: Vec::new(),
                    posix: Vec::new(),
                    syntax: vec![(*code, *neg)],
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
                        // loop: split(body, end) with back-jump
                        let split = self.push(Inst::Split(0, 0));
                        let body_start = self.prog.len();
                        self.emit(node);
                        self.push(Inst::Jmp(split));
                        let end = self.prog.len();
                        self.prog[split] = if *greedy {
                            Inst::Split(body_start, end)
                        } else {
                            Inst::Split(end, body_start)
                        };
                        // lazy loop exit still needs to try body later
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
    };
    let ast = p.parse_alt()?;
    if p.pos != p.chars.len() {
        return Err(RegexError(format!(
            "trailing garbage in regexp at {}",
            p.pos
        )));
    }
    let mut cg = Codegen {
        prog: Vec::with_capacity(64),
    };
    cg.push(Inst::Save(0));
    cg.emit(&ast);
    cg.push(Inst::Save(1));
    cg.push(Inst::Match);
    Ok(Regex {
        prog: cg.prog,
        n_groups: p.n_groups,
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
fn run(
    re: &Regex,
    text: &[char],
    mut pc: usize,
    mut sp: usize,
    mut regs: Regs,
    depth: usize,
) -> Option<Regs> {
    if depth > 10_000 {
        return None;
    }
    loop {
        match &re.prog[pc] {
            Inst::Char(c) => {
                if sp < text.len() && char_eq(text[sp], *c, re.case_fold) {
                    sp += 1;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Any => {
                if sp < text.len() && text[sp] != '\n' {
                    sp += 1;
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Class(set) => {
                if sp < text.len() && set.contains(text[sp], re.case_fold) {
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
                if sp + n <= text.len()
                    && (0..n).all(|k| char_eq(text[sp + k], text[s + k], re.case_fold))
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
                let before = sp > 0 && is_word_char(text[sp - 1]);
                let after = sp < text.len() && is_word_char(text[sp]);
                if before != after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::NotWordB => {
                let before = sp > 0 && is_word_char(text[sp - 1]);
                let after = sp < text.len() && is_word_char(text[sp]);
                if before == after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::WordStart => {
                let before = sp > 0 && is_word_char(text[sp - 1]);
                let after = sp < text.len() && is_word_char(text[sp]);
                if !before && after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::WordEnd => {
                let before = sp > 0 && is_word_char(text[sp - 1]);
                let after = sp < text.len() && is_word_char(text[sp]);
                if before && !after {
                    pc += 1;
                } else {
                    return None;
                }
            }
            Inst::Split(a, b) => {
                if let Some(r) = run(re, text, *a, sp, regs.clone(), depth + 1) {
                    return Some(r);
                }
                pc = *b;
            }
            Inst::Jmp(x) => pc = *x,
            Inst::Save(n) => {
                regs[*n] = Some(sp);
                pc += 1;
            }
            Inst::Match => return Some(regs),
        }
    }
}

/// Try to match at exactly `pos`. Returns regs on success.
pub fn match_at(re: &Regex, text: &[char], pos: usize) -> Option<Regs> {
    run(re, text, 0, pos, vec![None; 2 * (re.n_groups + 1)], 0)
}

/// Search forward from `pos`; returns (match_start, match_end) of group 0.
pub fn search(re: &Regex, text: &[char], pos: usize) -> Option<(usize, usize)> {
    let mut p = pos;
    while p <= text.len() {
        if let Some(regs) = match_at(re, text, p) {
            let s = regs[0].unwrap_or(p);
            let e = regs[1].unwrap_or(p);
            return Some((s, e));
        }
        p += 1;
    }
    None
}

/// Search backward from `pos` (find the latest match starting <= pos).
pub fn search_backward(re: &Regex, text: &[char], pos: usize) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut p = 0;
    while p <= pos.min(text.len()) {
        if let Some(regs) = match_at(re, text, p) {
            let s = regs[0].unwrap_or(p);
            let e = regs[1].unwrap_or(p);
            if s <= pos && e >= s {
                if e <= pos || s == pos {
                    best = Some((s, e));
                }
            }
        }
        p += 1;
    }
    best
}

/// Full match info for `match-data`.
pub struct FullMatch {
    pub regs: Regs,
}

/// Search with full register info.
pub fn search_full(re: &Regex, text: &[char], pos: usize) -> Option<Regs> {
    let mut p = pos;
    while p <= text.len() {
        if let Some(regs) = match_at(re, text, p) {
            return Some(regs);
        }
        p += 1;
    }
    None
}

/// Backward search returning full regs: the match with the greatest
/// start whose end is at or before `pos` (GNU semantics).
pub fn search_backward_full(re: &Regex, text: &[char], pos: usize) -> Option<Regs> {
    let mut best: Option<Regs> = None;
    let mut p = 0;
    while p <= pos.min(text.len()) {
        if let Some(regs) = match_at(re, text, p) {
            let s = regs[0].unwrap_or(p);
            let e = regs[1].unwrap_or(p);
            if s <= pos && e <= pos {
                best = Some(regs);
            }
        }
        p += 1;
    }
    best
}

/// `string-match` / `looking-at` need the text as chars; helpers to
/// convert buffer substrings are in the caller.

/// Simple `looking-at` helper: match at pos.
pub fn looking_at(re: &Regex, text: &[char], pos: usize) -> Option<Regs> {
    match_at(re, text, pos)
}
