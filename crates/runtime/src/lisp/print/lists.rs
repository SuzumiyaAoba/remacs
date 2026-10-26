//! Shared list traversal for readable and display output, including Brent cycle detection.

use super::circle::{circle_active, circle_counted, print_stack_key};
use super::render::PrintMode;
use crate::lisp::{Interp, print_text::PrintText, value::Value};
use std::fmt::Write;

impl Interp {
    pub(super) fn print_list(&self, value: &Value, out: &mut PrintText, depth: usize, bq: bool) {
        self.print_list_mode(value, out, depth, bq, PrintMode::Readable)
    }
    pub(super) fn print_list_princ(
        &self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) {
        self.print_list_mode(value, out, depth, bq, PrintMode::Display)
    }
    /// Print a (possibly dotted) list.
    fn print_list_mode(
        &self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq) {
            out.push_str(prefix);
            mode.render(self, &inner, out, depth + 1, inner_bq);
            return;
        }
        let limit = self.print_length_limit();
        out.push('(');
        // GNU print.c: `(' ELEM, then ` ' ELEM per continuation cell;
        // a Brent-style tortoise walking the cdr chain detects a cdr
        // cycle and prints `. #N)' with the tortoise's list index.
        let Value::Cons(head) = v else { return };
        let mut cur = {
            let b = head.borrow();
            let car = b.car.clone();
            let next = b.cdr.clone();
            drop(b);
            if limit == Some(0) {
                // GNU prints `(...)' when print-length is 0.
                out.push_str("...)");
                return;
            }
            mode.render(self, &car, out, depth + 1, bq);
            next
        };
        let mut tortoise = v.clone();
        let (mut n, mut m, mut idx) = (2usize, 2usize, 0usize);
        let mut printed = 1usize;
        loop {
            match cur {
                Value::Cons(_) => {
                    // `print-circle': a shared/cyclic tail prints as
                    // `. #N=(...)' (first occurrence) or `. #N#';
                    // GNU checks this before `print-length'.
                    if circle_active() && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
                    {
                        out.push_str(" . ");
                        mode.render(self, &cur, out, depth + 1, bq);
                        out.push(')');
                        return;
                    }
                    if let Some(l) = limit {
                        if printed >= l {
                            out.push_str(" ...");
                            out.push(')');
                            return;
                        }
                    }
                    out.push(' ');
                    n -= 1;
                    if n == 0 {
                        idx += m;
                        m <<= 1;
                        n = m;
                        tortoise = cur.clone();
                    } else if crate::lisp::builtins::eq_values(&cur, &tortoise) {
                        let _ = write!(out, ". #{idx})");
                        return;
                    }
                    let Value::Cons(c) = &cur else { unreachable!() };
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    printed += 1;
                    mode.render(self, &car, out, depth + 1, bq);
                    cur = next;
                }
                Value::Nil => {
                    out.push(')');
                    return;
                }
                other => {
                    out.push_str(" . ");
                    mode.render(self, &other, out, depth + 1, bq);
                    out.push(')');
                    return;
                }
            }
        }
    }
}
