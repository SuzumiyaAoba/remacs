//! Matching/search entry points and ordered recursive NFA execution.

use super::{CatFn, Inst, Regex, SynFn, is_sym_char, is_word_char};
use crate::lisp::text::{CharSequence, EmacsChar};

// ---------- matching ----------

pub struct MatchState {
    /// Registers: 2*g start, 2*g+1 end (char positions).
    pub regs: Vec<Option<usize>>,
    pub end: usize,
}

pub type Regs = Vec<Option<usize>>;

/// Recursive matcher. Returns final regs on success.
/// `point` is the buffer point for `\=' (NO_POINT in string searches).
/// `limit`, when not usize::MAX, caps how far into the text the match
/// may consume — GNU `re_match's STOP: a backward search's candidate
/// must end at or before the search start position.
/// `end_cap`, when not usize::MAX, requires the match to end exactly
/// there (POSIX longest-match and `looking-back' probing).
struct MatchContext<'a> {
    re: &'a Regex,
    text: &'a [EmacsChar],
    syn: SynFn<'a>,
    cat: CatFn<'a>,
    point: usize,
    limit: usize,
    end_cap: usize,
}

impl MatchContext<'_> {
    fn run(&self, mut pc: usize, mut sp: usize, mut regs: Regs, depth: usize) -> Option<Regs> {
        let Self {
            re,
            text,
            syn,
            cat,
            point,
            limit,
            end_cap,
        } = *self;
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
                    if let Some(r) = self.run(*a, sp, regs.clone(), depth + 1) {
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
}

/// `point` value for string searches and other contexts where the
/// `\=' anchor can never match (GNU: \= only matches at buffer point).
pub const NO_POINT: usize = usize::MAX;

/// Try to match at exactly `pos`. Returns regs on success.
fn match_at_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    match_at_end_wide(re, text, pos, usize::MAX, syn, cat, point)
}

/// Try to match at `pos` requiring the match to end exactly at `end`.
fn match_at_end_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    end: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    match_at_within_wide(re, text, pos, usize::MAX, end, syn, cat, point)
}

/// Try to match at `pos` on the truncated text `text[..limit]`
/// (usize::MAX = no limit): the match may end anywhere at or before
/// `limit` — GNU `re_match' with STOP, used for backward searches.
/// `end_cap`, when not usize::MAX, additionally requires the match to
/// end exactly there.
fn match_at_within_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    limit: usize,
    end_cap: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    let n = 2 * (re.n_groups + 1);
    let context = MatchContext {
        re,
        text,
        syn,
        cat,
        point,
        limit,
        end_cap,
    };
    let mut regs = context.run(0, pos, vec![None; n + re.extra_regs], 0)?;
    regs.truncate(n);
    Some(regs)
}

/// POSIX match at `pos`: the LONGEST match whose end is <= `limit`
/// (usize::MAX = no limit). Group/subexp tie-breaks stay first-match.
fn posix_match_at_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    limit: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    let mut e = limit.min(text.len());
    while e >= pos {
        if let Some(regs) = match_at_end_wide(re, text, pos, e, syn, cat, point) {
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
fn search_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    let mut p = pos;
    while p <= text.len() {
        if let Some(regs) = match_at_wide(re, text, p, syn, cat, point) {
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
fn search_backward_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    let mut p = pos.min(text.len());
    loop {
        if let Some(regs) = match_at_within_wide(re, text, p, pos, usize::MAX, syn, cat, point) {
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
fn search_full_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    let mut p = pos;
    while p <= text.len() {
        let hit = if posix {
            posix_match_at_wide(re, text, p, usize::MAX, syn, cat, point)
        } else {
            match_at_wide(re, text, p, syn, cat, point)
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
fn search_backward_full_wide(
    re: &Regex,
    text: &[EmacsChar],
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
            posix_match_at_wide(re, text, p, pos, syn, cat, point)
        } else {
            match_at_within_wide(re, text, p, pos, usize::MAX, syn, cat, point)
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
fn looking_at_wide(
    re: &Regex,
    text: &[EmacsChar],
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    if posix {
        posix_match_at_wide(re, text, pos, usize::MAX, syn, cat, point)
    } else {
        match_at_wide(re, text, pos, syn, cat, point)
    }
}

pub fn match_at<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    text.with_chars(|text| match_at_wide(re, text, pos, syn, cat, point))
}

pub fn match_at_end<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    end: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    text.with_chars(|text| match_at_end_wide(re, text, pos, end, syn, cat, point))
}

pub fn match_at_within<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    limit: usize,
    end_cap: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    text.with_chars(|text| match_at_within_wide(re, text, pos, limit, end_cap, syn, cat, point))
}

pub fn posix_match_at<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    limit: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<Regs> {
    text.with_chars(|text| posix_match_at_wide(re, text, pos, limit, syn, cat, point))
}

pub fn search<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    text.with_chars(|text| search_wide(re, text, pos, syn, cat, point))
}

pub fn search_backward<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
) -> Option<(usize, usize)> {
    text.with_chars(|text| search_backward_wide(re, text, pos, syn, cat, point))
}

pub fn search_full<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    text.with_chars(|text| search_full_wide(re, text, pos, syn, cat, point, posix))
}

pub fn search_backward_full<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    text.with_chars(|text| search_backward_full_wide(re, text, pos, syn, cat, point, posix))
}

pub fn looking_at<T: CharSequence + ?Sized>(
    re: &Regex,
    text: &T,
    pos: usize,
    syn: SynFn,
    cat: CatFn,
    point: usize,
    posix: bool,
) -> Option<Regs> {
    text.with_chars(|text| looking_at_wide(re, text, pos, syn, cat, point, posix))
}
