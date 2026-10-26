//! Top-level readable/display rendering and unreadable-printer callback sequencing.

use super::circle::{
    BEING_PRINTED, CIRCLE, CircleCtx, NtSubst, UNREADABLE_HITS, circle_active, circle_label,
    number_table_subst, print_identity_key,
};
use crate::lisp::{Interp, error::Flow, print_text::PrintText, text::EmacsText, value::Value};
use std::fmt::Write;

#[derive(Clone, Copy)]
pub(super) enum PrintMode {
    Readable,
    Display,
}

impl PrintMode {
    pub(super) fn render(
        self,
        i: &Interp,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) {
        match self {
            Self::Readable => i.prin1_inner(value, out, depth, bq),
            Self::Display => i.princ_inner(value, out, depth, bq),
        }
    }
    pub(super) fn escaped(self) -> bool {
        matches!(self, Self::Readable)
    }
}

/// Pure rendering executes no Lisp callbacks. Restore the thread-local
/// recursion/circle state before callbacks, and on a Rust unwind as well.
struct PrintScope {
    mark: usize,
    previous: Option<CircleCtx>,
}

impl PrintScope {
    fn enter(i: &Interp, value: &Value) -> Self {
        let mark = BEING_PRINTED.with(|stack| stack.borrow().len());
        let previous = i.install_circle_ctx(value);
        Self { mark, previous }
    }
}

impl Drop for PrintScope {
    fn drop(&mut self) {
        CIRCLE.with(|circle| *circle.borrow_mut() = self.previous.take());
        BEING_PRINTED.with(|stack| stack.borrow_mut().truncate(self.mark));
    }
}

impl Interp {
    pub(super) fn prin1_inner(&self, value: &Value, out: &mut PrintText, depth: usize, bq: bool) {
        self.print_inner_mode(value, out, depth, bq, PrintMode::Readable)
    }
    pub(super) fn princ_inner(&self, value: &Value, out: &mut PrintText, depth: usize, bq: bool) {
        self.print_inner_mode(value, out, depth, bq, PrintMode::Display)
    }
    fn print_inner_mode(
        &self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) {
        let key = print_identity_key(self, v);
        if let Some(k) = key {
            if let Some(sub) = number_table_subst(self, v) {
                match sub {
                    NtSubst::Replace(t) => {
                        out.push_str(&t);
                        return;
                    }
                    // `#N=' prefix, then print the object body.
                    NtSubst::Prefix(t) => out.push_str(&t),
                }
            }
            if circle_active() {
                // `print-circle': shared/cyclic objects print `#N='
                // at first occurrence and `#N#' afterwards.
                match circle_label(k) {
                    Some((n, false)) => {
                        let _ = write!(out, "#{n}#");
                        return;
                    }
                    Some((n, true)) => {
                        let _ = write!(out, "#{n}=");
                    }
                    None => {}
                }
                self.render_object(v, out, depth, bq, mode);
                return;
            }
            // GNU's being-printed check: an object still on the print
            // stack prints as `#N' (its stack index).
            let hit = BEING_PRINTED.with(|bp| bp.borrow().iter().position(|&x| x == k));
            if let Some(i) = hit {
                let _ = write!(out, "#{i}");
                return;
            }
            BEING_PRINTED.with(|bp| bp.borrow_mut().push(k));
            self.render_object(v, out, depth, bq, mode);
            BEING_PRINTED.with(|bp| {
                bp.borrow_mut().pop();
            });
            return;
        }
        self.render_object(v, out, depth, bq, mode);
    }

    fn render_object(
        &self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) {
        let start = out.len();
        match mode {
            PrintMode::Readable => self.prin1_inner_obj_raw(value, out, depth, bq),
            PrintMode::Display => self.princ_inner_obj_raw(value, out, depth, bq),
        }
        if out.span_is_unreadable(start) {
            UNREADABLE_HITS.with(|hits| hits.borrow_mut().push((start, out.len(), value.clone())));
        }
    }
    /// Readable representation with escaped strings and symbol names.
    pub fn print_to_string(&self, value: &Value) -> EmacsText {
        self.render_text(value, PrintMode::Readable).to_text(true)
    }
    pub fn prin1_to_string(&self, value: &Value) -> EmacsText {
        self.print_to_string(value)
    }
    /// Display representation, retaining raw string and symbol contents.
    pub fn princ_to_string(&self, value: &Value) -> EmacsText {
        self.render_text(value, PrintMode::Display).to_text(true)
    }
    /// Replace unreadable spans after rendering and restoring the print scope.
    pub fn print_to_string_cb(&mut self, value: &Value) -> Result<PrintText, Flow> {
        self.render_with_callback(value, PrintMode::Readable)
    }
    pub fn princ_to_string_cb(&mut self, value: &Value) -> Result<PrintText, Flow> {
        self.render_with_callback(value, PrintMode::Display)
    }
    fn render_text(&self, value: &Value, mode: PrintMode) -> PrintText {
        let mut text = PrintText::new();
        let scope = PrintScope::enter(self, value);
        mode.render(self, value, &mut text, 0, false);
        drop(scope);
        text
    }
    fn render_with_callback(&mut self, value: &Value, mode: PrintMode) -> Result<PrintText, Flow> {
        self.reset_print_numbering();
        UNREADABLE_HITS.with(|hits| hits.borrow_mut().clear());
        let mut text = self.render_text(value, mode);
        let hits = UNREADABLE_HITS.with(|hits| std::mem::take(&mut *hits.borrow_mut()));
        if hits.is_empty() {
            return Ok(text);
        }
        let variable = self.q("print-unreadable-function");
        let callback = self.symbol_value(variable);
        if !crate::lisp::builtins::data::functionp_val(self, &callback) {
            return Ok(text);
        }
        // Splice backwards so callbacks cannot invalidate pending offsets.
        for (lo, hi, object) in hits.into_iter().rev() {
            let replacement = self.call_unreadable_printer(&callback, object, mode.escaped())?;
            if let Value::Str(replacement) = replacement {
                text.replace_range(lo..hi, &replacement.borrow());
            }
        }
        Ok(text)
    }
    fn call_unreadable_printer(
        &mut self,
        callback: &Value,
        object: Value,
        escape: bool,
    ) -> Result<Value, crate::lisp::error::Flow> {
        let mark = self.specbind_depth();
        let result = (|| {
            // GNU suppresses recursive use of the same unreadable printer.
            let variable = self.q("print-unreadable-function");
            self.specbind(variable, Value::Nil)?;
            self.apply(callback, vec![object, Value::from_bool(escape)])
        })();
        self.unbind_to(mark)?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_unwind_restores_the_previous_print_recursion_stack() {
        let mut i = Interp::vacant();
        i.obarray = crate::lisp::obarray::Obarray::new();
        let circle = i.intern("print-circle");
        i.obarray.symbol_mut(circle).value = Value::t();
        CIRCLE.with(|circle| {
            *circle.borrow_mut() = Some(CircleCtx {
                seen: Default::default(),
                labels: Default::default(),
                printed: Default::default(),
                next: 17,
            })
        });
        BEING_PRINTED.with(|stack| stack.borrow_mut().push(42));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _scope = PrintScope::enter(&i, &Value::Nil);
            BEING_PRINTED.with(|stack| stack.borrow_mut().push(99));
            panic!("test rendering unwind");
        }));
        assert!(result.is_err());
        BEING_PRINTED.with(|stack| assert_eq!(&*stack.borrow(), &[42]));
        CIRCLE.with(|circle| {
            assert_eq!(
                circle.borrow().as_ref().map(|context| context.next),
                Some(17)
            )
        });
        CIRCLE.with(|circle| *circle.borrow_mut() = None);
        BEING_PRINTED.with(|stack| stack.borrow_mut().clear());
    }
}
