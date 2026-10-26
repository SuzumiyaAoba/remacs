//! Translate a validated pattern AST into ordered backtracking instructions.

use super::parser::{Ast, ParsedPattern, RegexError, parse_pattern};
use super::{CharSet, Inst, Regex};
use crate::lisp::text::TextSource;

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
pub fn compile<T: TextSource + ?Sized>(pattern: &T) -> Result<Regex, RegexError> {
    compile_case(pattern, true)
}

pub fn compile_case<T: TextSource + ?Sized>(
    pattern: &T,
    case_fold: bool,
) -> Result<Regex, RegexError> {
    let pattern = pattern.with_text(|text| text.to_owned());
    let ParsedPattern { ast, n_groups } = parse_pattern(&pattern)?;
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
