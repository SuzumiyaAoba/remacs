//! Shared list traversal for readable and display output, including Brent cycle detection.

use super::circle::{circle_active, circle_counted, print_stack_key};
use super::render::{PrintMode, Printer};
use crate::lisp::{print_text::PrintText, value::Value};

impl Printer<'_> {
    pub(super) fn print_list(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), crate::lisp::error::Flow> {
        self.print_list_mode(value, out, depth, bq, PrintMode::Readable)
    }
    pub(super) fn print_list_princ(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), crate::lisp::error::Flow> {
        self.print_list_mode(value, out, depth, bq, PrintMode::Display)
    }
    /// Print a (possibly dotted) list.
    fn print_list_mode(
        &mut self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) -> Result<(), crate::lisp::error::Flow> {
        if let Some((prefix, inner, inner_bq)) = self.quote_abbrev(v, bq)? {
            self.push_text(out, prefix)?;
            mode.render(self, &inner, out, depth + 1, inner_bq)?;
            return Ok(());
        }
        self.push_char(out, '(')?;
        let limit = self.print_length_limit();
        // GNU print.c: `(' ELEM, then ` ' ELEM per continuation cell;
        // a Brent-style tortoise walking the cdr chain detects a cdr
        // cycle and prints `. #N)' with the tortoise's list index.
        let Value::Cons(head) = v else { return Ok(()) };
        let mut cur = {
            let b = head.try_borrow().map_err(|_| {
                self.interp()
                    .error("Cannot print a mutably borrowed cons cell")
            })?;
            let car = b.car.clone();
            let next = b.cdr.clone();
            drop(b);
            if limit == Some(0) {
                // GNU prints `(...)' when print-length is 0.
                self.push_text(out, "...)")?;
                return Ok(());
            }
            mode.render(self, &car, out, depth + 1, bq)?;
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
                    if circle_active()
                        && self.print_var("print-circle").truthy()
                        && print_stack_key(&cur).map(circle_counted).unwrap_or(false)
                    {
                        self.push_text(out, " . ")?;
                        mode.render(self, &cur, out, depth + 1, bq)?;
                        self.push_char(out, ')')?;
                        return Ok(());
                    }
                    if let Some(l) = limit {
                        if printed >= l {
                            self.push_text(out, " ...")?;
                            self.push_char(out, ')')?;
                            return Ok(());
                        }
                    }
                    self.push_char(out, ' ')?;
                    n -= 1;
                    if n == 0 {
                        idx += m;
                        m <<= 1;
                        n = m;
                        tortoise = cur.clone();
                    } else if crate::lisp::builtins::eq_values(&cur, &tortoise) {
                        self.push_format(out, format_args!(". #{idx})"))?;
                        return Ok(());
                    }
                    let Value::Cons(c) = &cur else { unreachable!() };
                    let (car, next) = {
                        let b = c.try_borrow().map_err(|_| {
                            self.interp()
                                .error("Cannot print a mutably borrowed cons cell")
                        })?;
                        (b.car.clone(), b.cdr.clone())
                    };
                    printed += 1;
                    mode.render(self, &car, out, depth + 1, bq)?;
                    cur = next;
                }
                Value::Nil => {
                    self.push_char(out, ')')?;
                    return Ok(());
                }
                other => {
                    self.push_text(out, " . ")?;
                    mode.render(self, &other, out, depth + 1, bq)?;
                    self.push_char(out, ')')?;
                    return Ok(());
                }
            }
        }
    }
}
