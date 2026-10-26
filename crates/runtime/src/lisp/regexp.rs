//! Emacs-style regular expressions, implemented as a backtracking NFA
//! over `char` positions.
//!
//! Supported syntax (per Emacs):
//!   `.` `*` `+` `?` `*?` `+?` `??` `\{m\}` `\{m,n\}`
//!   `[...]` `[^...]` ranges, `[:class:]` POSIX classes
//!   `^` `$` `` \` `` `\'` `\b` `\B` `\<` `\>` `\_<` `\_>` `\w` `\W` `\sX` `\SX`
//!   `\(...\)` `\(?:...\)` `\|` `\1`–`\9` backrefs

use super::text::{EmacsChar, EmacsText};

mod codegen;
mod matching;
mod parser;
pub use codegen::{compile, compile_case};
pub use matching::{
    MatchState, NO_POINT, Regs, looking_at, match_at, match_at_end, match_at_within,
    posix_match_at, search, search_backward, search_backward_full, search_full,
};
pub use parser::RegexError;

/// A compiled regexp: instruction list + group count.
pub struct Regex {
    prog: Vec<Inst>,
    pub n_groups: usize,
    /// Extra registers (beyond the 2*(groups+1) match registers)
    /// reserved for loop progress checks.
    extra_regs: usize,
    pub case_fold: bool,
    /// Source pattern (for error messages).
    pub source: EmacsText,
}

#[derive(Debug, Clone)]
enum Inst {
    /// Match exact char `c` at `pos` (post-fold if case_fold).
    Char(EmacsChar),
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
    ranges: Vec<(EmacsChar, EmacsChar)>,
    singles: Vec<EmacsChar>,
    posix: Vec<(String, bool)>, // (class-name, negated?)
    syntax: Vec<(u8, bool)>,    // (syntax code, negated?)
    cats: Vec<(u8, bool)>,      // (category bit, negated?) — \cX / \CX
}

impl CharSet {
    fn base_match(&self, c: EmacsChar, syn: SynFn, cat: CatFn, pos: usize) -> bool {
        self.singles.contains(&c)
            || self.ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi)
            || self
                .syntax
                .iter()
                .any(|(code, neg)| syntax_match(*code, c, syn, pos) != *neg)
            || self.cats.iter().any(|(bit, neg)| cat(c, *bit) != *neg)
    }

    fn contains(&self, c: EmacsChar, case_fold: bool, syn: SynFn, cat: CatFn, pos: usize) -> bool {
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
pub fn syntax_code(c: impl Into<EmacsChar>) -> u8 {
    let Some(c) = c.into().unicode() else {
        return b'.';
    };
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

fn syntax_match(code: u8, c: EmacsChar, syn: SynFn, pos: usize) -> bool {
    let sc = syn.code_at(c, pos);
    // GNU accepts `-` as an alias for the whitespace class.
    sc == code || (code == b'-' && sc == b' ')
}

/// Table-free fallback for standalone syntax callbacks and early loadup.
/// The interpreter overrides this with its GNU character tables.
fn posix_match(name: &str, c: EmacsChar, syn: &dyn Fn(EmacsChar) -> u8, fold: bool) -> bool {
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
        "cntrl" => c.code() < 0x20,
        "blank" => c == ' ' || c == '\t',
        "word" => syn(c) == b'w',
        // GNU's nonstandard extras (regex.c `RE_CHAR_CLASSES').
        "ascii" => c.is_ascii(),
        "nonascii" => !c.is_ascii(),
        "unibyte" => c.code() <= 0xFF,
        "multibyte" => c.code() > 0xFF,
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
    fn code(&self, c: EmacsChar) -> u8;

    fn code_at(&self, c: EmacsChar, _pos: usize) -> u8 {
        self.code(c)
    }

    fn case_eq(&self, a: EmacsChar, b: EmacsChar) -> bool {
        char_eq(a, b, true)
    }

    fn case_match(&self, c: EmacsChar, test: &mut dyn FnMut(EmacsChar) -> bool) -> bool {
        c.to_lowercase()
            .chain(c.to_uppercase())
            .any(|variant| variant != c && test(variant))
    }

    fn posix(&self, name: &str, c: EmacsChar, fold: bool) -> bool {
        posix_match(name, c, &|ch| self.code(ch), fold)
    }
}

impl<F: Fn(char) -> u8 + ?Sized> Syntax for F {
    fn code(&self, c: EmacsChar) -> u8 {
        self(c.unicode().unwrap_or('\u{fffd}'))
    }
}

pub type SynFn<'a> = &'a dyn Syntax;

/// GNU character.c and regex-emacs.c predicates.  CATEGORY is the raw
/// general-category enum; UPPER/LOWER come from the current case table.
pub(crate) fn class_matches(
    name: &str,
    c: EmacsChar,
    syntax: u8,
    category: Option<i128>,
    upper: bool,
    lower: bool,
) -> bool {
    let cp = i128::from(c.code());
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
                (c == ' ' || c == '\t')
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
pub type CatFn<'a> = &'a dyn Fn(EmacsChar, u8) -> bool;

/// No-categories fallback for callers without a category table.
pub fn no_cat(_: EmacsChar, _: u8) -> bool {
    false
}

/// `syntax_code' (standard table) as a `SynFn' for callers without a
/// buffer syntax table.
pub fn std_syntax(c: char) -> u8 {
    syntax_code(c)
}

fn is_word_char(c: EmacsChar, syn: SynFn, pos: usize) -> bool {
    syn.code_at(c, pos) == b'w'
}

fn is_sym_char(c: EmacsChar, syn: SynFn, pos: usize) -> bool {
    matches!(syn.code_at(c, pos), b'w' | b'_')
}

fn char_eq(a: EmacsChar, b: EmacsChar, fold: bool) -> bool {
    if a == b {
        return true;
    }
    if !fold {
        return false;
    }
    a.to_lowercase().eq(b.to_lowercase()) || a.to_uppercase().eq(b.to_uppercase())
}

#[cfg(test)]
mod wide_character_tests {
    use super::*;

    #[test]
    fn literals_and_classes_distinguish_unicode_from_extended_characters() {
        let codes = [
            0xf0080, 0x3fff80, 0xf0100, 0xd800, 0xfffd, 0x110000, 0x200000,
        ];
        let text: Vec<EmacsChar> = codes
            .into_iter()
            .map(|c| EmacsChar::new(c).unwrap())
            .collect();
        for (position, code) in codes.into_iter().enumerate() {
            let literal = EmacsText::from_codes([code]).unwrap();
            let regex = compile(&literal).unwrap();
            assert_eq!(
                search(&regex, &text, 0, &std_syntax, &no_cat, 0),
                Some((position, position + 1)),
                "literal {code:x}"
            );
            let mut class = EmacsText::from("[");
            class.append(&literal);
            class.push(']');
            let regex = compile(&class).unwrap();
            assert_eq!(
                search(&regex, &text, 0, &std_syntax, &no_cat, 0),
                Some((position, position + 1)),
                "class {code:x}"
            );
        }
    }

    #[test]
    fn legacy_unicode_callers_and_wide_ranges_share_the_engine() {
        let regex = compile("a+\\(b\\)").unwrap();
        let unicode: Vec<char> = "aab".chars().collect();
        assert_eq!(
            search(&regex, &unicode, 0, &std_syntax, &no_cat, 0),
            Some((0, 3))
        );
        let range =
            EmacsText::from_codes([b'[' as u32, 0x110000, b'-' as u32, 0x110002, b']' as u32])
                .unwrap();
        let regex = compile(&range).unwrap();
        let text: Vec<EmacsChar> = [0xf0b00, 0x110001]
            .into_iter()
            .map(|c| EmacsChar::new(c).unwrap())
            .collect();
        assert_eq!(
            search(&regex, &text, 0, &std_syntax, &no_cat, 0),
            Some((1, 2))
        );
    }
}

#[cfg(test)]
mod stage_contract_tests {
    use super::*;

    #[test]
    fn backtracking_restores_capture_registers_before_backreferences() {
        let regex = compile_case("\\(ab\\|a\\)b\\1", false).unwrap();
        let text: Vec<char> = "aba".chars().collect();
        assert_eq!(
            match_at(&regex, &text, 0, &std_syntax, &no_cat, NO_POINT),
            Some(vec![Some(0), Some(3), Some(0), Some(1)])
        );
    }

    #[test]
    fn posix_longest_match_keeps_its_distinct_selection_policy() {
        let regex = compile_case("a\\|aa", false).unwrap();
        let text: Vec<char> = "aa".chars().collect();
        let first = search_full(&regex, &text, 0, &std_syntax, &no_cat, NO_POINT, false).unwrap();
        let longest = search_full(&regex, &text, 0, &std_syntax, &no_cat, NO_POINT, true).unwrap();
        assert_eq!(&first[..2], &[Some(0), Some(1)]);
        assert_eq!(&longest[..2], &[Some(0), Some(2)]);
    }

    #[test]
    fn repetitions_with_empty_bodies_still_make_progress() {
        let regex = compile_case("\\(a*\\)*b", false).unwrap();
        let text: Vec<char> = "b".chars().collect();
        assert_eq!(
            search(&regex, &text, 0, &std_syntax, &no_cat, NO_POINT),
            Some((0, 1))
        );
    }
}
