//! Top-level readable/display rendering and unreadable-printer callback sequencing.

use super::circle::{
    BEING_PRINTED, CIRCLE, CircleCtx, NtSubst, PreparedCircle, circle_active, circle_label,
    finish_print_number_table, finish_print_number_table_shared, number_table_subst,
    print_identity_key, printed_circle_labels,
};
use crate::lisp::{
    Interp,
    error::Flow,
    print_text::{OutputToken, PrintText},
    text::{EmacsText, TextSource},
    value::{HashKey, Value},
};
use std::cell::RefCell;
use std::ops::Deref;

thread_local! {
    static CALLBACK_PRINT_BUFFERS: RefCell<Vec<CallbackPrintBuffer>> = const { RefCell::new(Vec::new()) };
}

const MAX_PRINT_NESTING: usize = 200;

struct CallbackPrintBuffer {
    output: PrintText,
}

struct CallbackPrintScope {
    active: bool,
}

impl CallbackPrintScope {
    fn enter(output: &PrintText) -> Self {
        CALLBACK_PRINT_BUFFERS.with(|buffers| {
            buffers.borrow_mut().push(CallbackPrintBuffer {
                output: output.clone(),
            });
        });
        Self { active: true }
    }

    fn finish(mut self) -> PrintText {
        self.active = false;
        CALLBACK_PRINT_BUFFERS.with(|buffers| {
            buffers
                .borrow_mut()
                .pop()
                .map(|buffer| buffer.output)
                .unwrap_or_default()
        })
    }
}

impl Drop for CallbackPrintScope {
    fn drop(&mut self) {
        if self.active {
            CALLBACK_PRINT_BUFFERS.with(|buffers| {
                buffers.borrow_mut().pop();
            });
        }
    }
}

fn record_recursive_print(output: &PrintText) {
    CALLBACK_PRINT_BUFFERS.with(|buffers| {
        if let Some(buffer) = buffers.borrow_mut().last_mut() {
            let saved = buffer.output.clone();
            buffer.output.restore_recursive_print(output, &saved);
        }
    });
}

enum InterpRef<'a> {
    Shared(&'a Interp),
    Mutable(&'a mut Interp),
}

/// Rendering access. Ordinary diagnostic printers use a shared interpreter
/// reference; Lisp-visible printers retain mutable access so unreadable
/// callbacks can run at their position in the object traversal.
pub(super) struct Printer<'a> {
    interp: InterpRef<'a>,
    /// `print_prepare' saves the caller's current buffer and makes it current
    /// again before each unreadable-object callback.
    callback_buffer: Option<usize>,
    /// A function stream receives each token at the point the renderer emits
    /// it, so Lisp mutations can affect the remaining object traversal.
    output_stream: Option<Value>,
    /// Shared diagnostic rendering cannot invoke mutable Lisp callbacks; it
    /// records number-table transitions for the caller to commit afterward.
    defer_number_table_events: bool,
}

impl<'a> Printer<'a> {
    fn shared(interp: &'a Interp) -> Self {
        Self {
            interp: InterpRef::Shared(interp),
            callback_buffer: None,
            output_stream: None,
            defer_number_table_events: true,
        }
    }

    fn mutable(
        interp: &'a mut Interp,
        callback_buffer: usize,
        output_stream: Option<Value>,
    ) -> Self {
        Self {
            interp: InterpRef::Mutable(interp),
            callback_buffer: Some(callback_buffer),
            output_stream,
            defer_number_table_events: false,
        }
    }

    pub(super) fn interp(&self) -> &Interp {
        match &self.interp {
            InterpRef::Shared(interp) => interp,
            InterpRef::Mutable(interp) => interp,
        }
    }

    fn check_print_depth(&self, value: &Value, depth: usize) -> Result<(), Flow> {
        if depth < MAX_PRINT_NESTING {
            return Ok(());
        }
        // GNU's `print-level' truncates conses before the recursive-depth
        // guard, while vectors and other objects ignore that variable.
        let level_truncates_cons = matches!(value, Value::Cons(_))
            && self
                .interp()
                .print_level_limit()
                .is_some_and(|level| depth >= level);
        if level_truncates_cons {
            Ok(())
        } else {
            Err(self
                .interp()
                .error("Apparently circular structure being printed"))
        }
    }

    pub(super) fn interp_mut(&mut self) -> Option<&mut Interp> {
        match &mut self.interp {
            InterpRef::Shared(_) => None,
            InterpRef::Mutable(interp) => Some(*interp),
        }
    }

    pub(super) fn push_token(
        &mut self,
        out: &mut PrintText,
        token: OutputToken,
    ) -> Result<(), Flow> {
        out.push_token(token);
        if let Some(stream) = self.output_stream.clone()
            && let Some(interp) = self.interp_mut()
        {
            interp.apply(&stream, vec![Value::Int(token.function_code())])?;
        }
        Ok(())
    }

    pub(super) fn push_char(
        &mut self,
        out: &mut PrintText,
        character: impl Into<crate::lisp::text::EmacsChar>,
    ) -> Result<(), Flow> {
        self.push_token(out, OutputToken::Character(character.into()))
    }

    pub(super) fn push_text<T: TextSource + ?Sized>(
        &mut self,
        out: &mut PrintText,
        text: &T,
    ) -> Result<(), Flow> {
        text.with_text(|text| {
            for character in text.chars() {
                self.push_char(out, character)?;
            }
            Ok(())
        })
    }

    pub(super) fn push_format(
        &mut self,
        out: &mut PrintText,
        arguments: std::fmt::Arguments<'_>,
    ) -> Result<(), Flow> {
        self.push_text(out, &arguments.to_string())
    }

    pub(super) fn push_printed_byte(&mut self, out: &mut PrintText, byte: u8) -> Result<(), Flow> {
        self.push_token(out, OutputToken::PrintedByte(byte))
    }

    fn record_number_event(
        &mut self,
        output: &mut PrintText,
        label: (usize, usize, Option<HashKey>),
    ) -> Result<(), Flow> {
        let (identity, number, entry_key) = label;
        if self.defer_number_table_events {
            output.push_number_event(identity, number, entry_key);
        } else if let Some(interp) = self.interp_mut() {
            finish_print_number_table(interp, &[(identity, number, entry_key)])?;
        } else {
            output.push_number_event(identity, number, entry_key);
        }
        Ok(())
    }
}

impl Deref for Printer<'_> {
    type Target = Interp;

    fn deref(&self) -> &Self::Target {
        self.interp()
    }
}

#[derive(Clone, Copy)]
pub(super) enum PrintMode {
    Readable,
    Display,
}

impl PrintMode {
    pub(super) fn render(
        self,
        printer: &mut Printer<'_>,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), Flow> {
        match self {
            Self::Readable => printer.prin1_inner(value, out, depth, bq)?,
            Self::Display => printer.princ_inner(value, out, depth, bq)?,
        }
        Ok(())
    }
    pub(super) fn escaped(self) -> bool {
        matches!(self, Self::Readable)
    }
}

/// Restore the thread-local recursion/circle state on normal return and on a
/// Rust unwind.
struct PrintScope {
    mark: usize,
    previous: Option<CircleCtx>,
}

impl PrintScope {
    fn enter(i: &Interp, value: &Value, prepared: Option<PreparedCircle>) -> Self {
        let mark = BEING_PRINTED.with(|stack| stack.borrow().len());
        let previous = i.install_circle_ctx(value, prepared);
        Self { mark, previous }
    }
}

impl Drop for PrintScope {
    fn drop(&mut self) {
        CIRCLE.with(|circle| *circle.borrow_mut() = self.previous.take());
        BEING_PRINTED.with(|stack| stack.borrow_mut().truncate(self.mark));
    }
}

impl Printer<'_> {
    pub(super) fn prin1_inner(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), Flow> {
        self.print_inner_mode(value, out, depth, bq, PrintMode::Readable)
    }
    pub(super) fn princ_inner(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
    ) -> Result<(), Flow> {
        self.print_inner_mode(value, out, depth, bq, PrintMode::Display)
    }
    fn print_inner_mode(
        &mut self,
        v: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) -> Result<(), Flow> {
        // GNU consults circle labels and `print-number-table' before trying
        // `print-unreadable-function'. The callback replaces only the
        // object's fallback representation, after any `#N=' prefix.
        let key = print_identity_key(self, v);
        if let Some(k) = key {
            if circle_active()
                && self.print_var("print-circle").truthy()
                && super::circle::print_circle_candidate(self, v)
            {
                let number_table = self.print_var("print-number-table");
                if !matches!(number_table, Value::Hash(_))
                    && let Some(interp) = self.interp_mut()
                {
                    return Err(interp.wrong_type_mut("hash-table-p", &number_table));
                }
            }
            let mut prefix_written = false;
            let mut pending_number_event = None;
            if let Some(sub) = number_table_subst(self, v, k, out)? {
                match sub {
                    NtSubst::Replace(t) => {
                        self.push_text(out, &t)?;
                        return Ok(());
                    }
                    // `#N=' prefix, then print the object body.
                    NtSubst::Prefix(t, label) => {
                        self.push_text(out, &t)?;
                        prefix_written = true;
                        if let Some(label) = label {
                            // The number-table label is visible to nested
                            // objects as soon as its complete `#N=' prefix
                            // has been emitted. Deferring this event until
                            // the object's body finishes can print a second
                            // `#N=' for a cycle that points back into that
                            // body.
                            self.record_number_event(out, label)?;
                        }
                    }
                }
            }
            if circle_active() && self.print_var("print-circle").truthy() {
                // `print-circle': shared/cyclic objects print `#N='
                // at first occurrence and `#N#' afterwards.
                match circle_label(k) {
                    Some((n, false)) => {
                        self.push_format(out, format_args!("#{n}#"))?;
                        return Ok(());
                    }
                    Some((n, true)) => {
                        if !prefix_written {
                            self.push_format(out, format_args!("#{n}="))?;
                            if let Some(label) = printed_circle_labels().last().cloned() {
                                if self.output_stream.is_some() {
                                    self.record_number_event(out, label)?;
                                } else {
                                    pending_number_event = Some(label);
                                }
                            }
                        }
                    }
                    None => {}
                }
                self.check_print_depth(v, depth)?;
                let rendered = self.render_unreadable_or_object(v, out, depth, bq, mode);
                if rendered.is_ok()
                    && let Some(label) = pending_number_event
                {
                    self.record_number_event(out, label)?;
                }
                return rendered;
            }
            // GNU's being-printed check: an object still on the print
            // stack prints as `#N' (its stack index).
            let hit = BEING_PRINTED.with(|bp| bp.borrow().iter().position(|&x| x == k));
            if let Some(i) = hit {
                self.push_format(out, format_args!("#{i}"))?;
                return Ok(());
            }
            self.check_print_depth(v, depth)?;
            BEING_PRINTED.with(|bp| bp.borrow_mut().push(k));
            let result = self.render_unreadable_or_object(v, out, depth, bq, mode);
            BEING_PRINTED.with(|bp| {
                bp.borrow_mut().pop();
            });
            if result.is_ok()
                && let Some(label) = pending_number_event
            {
                self.record_number_event(out, label)?;
            }
            return result;
        }
        self.check_print_depth(v, depth)?;
        self.render_unreadable_or_object(v, out, depth, bq, mode)
    }

    fn render_unreadable_or_object(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) -> Result<(), Flow> {
        if self.try_unreadable_callback(value, out, mode)? {
            Ok(())
        } else {
            self.render_object(value, out, depth, bq, mode)
        }
    }

    fn render_object(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        depth: usize,
        bq: bool,
        mode: PrintMode,
    ) -> Result<(), Flow> {
        match mode {
            PrintMode::Readable => self.prin1_inner_obj_raw(value, out, depth, bq)?,
            PrintMode::Display => self.princ_inner_obj_raw(value, out, depth, bq)?,
        }
        Ok(())
    }

    /// GNU calls `print-unreadable-function' before emitting the object's
    /// fallback `#<...>' representation. Doing this in the recursive walk
    /// lets callbacks' nested prints affect the remaining outer traversal.
    fn try_unreadable_callback(
        &mut self,
        value: &Value,
        out: &mut PrintText,
        mode: PrintMode,
    ) -> Result<bool, Flow> {
        if !self.unreadable_callback_candidate(value) {
            return Ok(false);
        }
        let callback_buffer = self.callback_buffer;
        let Some(interp) = self.interp_mut() else {
            return Ok(false);
        };
        let variable = interp.q("print-unreadable-function");
        let callback = interp.symbol_value(variable);
        if !crate::lisp::builtins::data::functionp_val(interp, &callback) {
            return Ok(false);
        }
        let mark = interp.specbind_depth();
        let buffer_scope = CallbackPrintScope::enter(out);
        let result = (|| {
            // GNU suppresses recursive use of the same unreadable printer.
            interp.specbind(variable, Value::Nil)?;
            if let Some(buffer) = callback_buffer {
                if interp.current_buffer != buffer && interp.buffers.get(buffer).is_some() {
                    interp.set_current_buffer(buffer);
                }
            }
            interp.apply(
                &callback,
                vec![value.clone(), Value::from_bool(mode.escaped())],
            )
        })();
        interp.unbind_to(mark)?;
        *out = buffer_scope.finish();
        match result? {
            Value::Nil => Ok(false),
            Value::Str(replacement) => {
                let replacement = replacement.borrow().clone();
                self.push_text(out, &replacement)?;
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn unreadable_callback_candidate(&self, value: &Value) -> bool {
        match value {
            // GNU routes every buffer through print_vectorlike_unreadable,
            // which calls the callback before choosing between the readable
            // #<buffer ...> form and princ's live buffer name.
            Value::Buffer(_) => true,
            Value::Subr(_)
            | Value::Marker(_)
            | Value::Window(_)
            | Value::Frame(_)
            | Value::Process(_)
            | Value::Thread(_)
            | Value::Mutex(_)
            | Value::CondVar(_)
            | Value::Finalizer(_) => true,
            Value::Lambda(lambda) => lambda.compiled && self.lambda_name(lambda).is_some(),
            Value::Vec(items) => self.overlay_repr(items).is_some(),
            Value::Record(items) => {
                let record = Value::Record(items.clone());
                if crate::editor::window_configuration_object(&record) {
                    return true;
                }
                if crate::lisp::builtins::misc::raw_symbol_position(&record).is_some()
                    && !self.print_var("print-symbols-bare").truthy()
                {
                    return true;
                }
                let Ok(slots) = items.try_borrow() else {
                    return false;
                };
                if record.record_kind() == Some(crate::lisp::value::RecordKind::Obarray) {
                    return true;
                }
                if let Some(Value::Sym(tag)) = slots.first() {
                    let name = self.symbol_name(*tag);
                    if crate::editor::is_terminal(self.interp(), &record)
                        && matches!(slots.as_slice(), [_, Value::Int(_), Value::Str(_)])
                    {
                        return true;
                    }
                    if name == "sub-char-table" {
                        return false;
                    }
                    if crate::lisp::builtins::treesit::treesit_repr(self, slots.as_slice())
                        .is_some()
                    {
                        return true;
                    }
                }
                record.record_kind() == Some(crate::lisp::value::RecordKind::FontSpec)
            }
            _ => false,
        }
    }
}

impl Interp {
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
    /// Render readable output, evaluating unreadable callbacks in traversal order.
    pub fn print_to_string_cb(&mut self, value: &Value) -> Result<PrintText, Flow> {
        self.render_with_callback(value, PrintMode::Readable, None)
    }
    pub fn princ_to_string_cb(&mut self, value: &Value) -> Result<PrintText, Flow> {
        self.render_with_callback(value, PrintMode::Display, None)
    }

    /// Render and send one Lisp-visible printer call to STREAM. Function
    /// streams receive characters in order, with `print-number-table'
    /// transitions applied only after each numbered prefix succeeds.
    pub fn print_to_output_cb(
        &mut self,
        value: &Value,
        stream: &Value,
        readable: bool,
    ) -> Result<(), Flow> {
        let destination = self.checked_output_stream(stream)?;
        let function_stream = !self.capture_output
            && !matches!(
                &destination,
                Value::Buffer(_) | Value::Marker(_) | Value::Nil
            )
            && !matches!(&destination, Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T);
        let mode = if readable {
            PrintMode::Readable
        } else {
            PrintMode::Display
        };
        if function_stream {
            self.render_with_callback(value, mode, Some(destination))?;
            return Ok(());
        }
        let destination_buffer = match &destination {
            Value::Buffer(buffer) => Some(buffer.borrow().id),
            Value::Marker(marker) => marker.borrow().buffer,
            _ => None,
        };
        let text = self.render_with_callback_in_buffer(value, mode, None, destination_buffer)?;
        self.write_output_to(&text, &destination)
    }
    fn render_text(&self, value: &Value, mode: PrintMode) -> PrintText {
        self.render_text_with_circle_state(value, mode, None)
    }
    fn render_text_with_circle_state(
        &self,
        value: &Value,
        mode: PrintMode,
        prepared: Option<PreparedCircle>,
    ) -> PrintText {
        let mut text = PrintText::new();
        let mut printer = Printer::shared(self);
        let scope = PrintScope::enter(printer.interp(), value, prepared);
        let rendered = mode.render(&mut printer, value, &mut text, 0, false);
        drop(scope);
        if rendered.is_err() {
            // Rust callers use the infallible convenience printer. Preserve
            // that API's no-panic contract when a RefCell-backed object is
            // temporarily unavailable; Lisp-visible printers use the
            // callback path and propagate the actual error instead.
            text = PrintText::new();
            text.push_str("#<unprintable object>");
            return text;
        }
        let labels = text
            .number_events()
            .iter()
            .map(|event| (event.identity, event.number, event.entry_key.clone()))
            .collect::<Vec<_>>();
        finish_print_number_table_shared(self, &labels);
        text
    }
    fn render_with_callback(
        &mut self,
        value: &Value,
        mode: PrintMode,
        output_stream: Option<Value>,
    ) -> Result<PrintText, Flow> {
        self.render_with_callback_in_buffer(value, mode, output_stream, None)
    }
    fn render_with_callback_in_buffer(
        &mut self,
        value: &Value,
        mode: PrintMode,
        output_stream: Option<Value>,
        requested_buffer: Option<usize>,
    ) -> Result<PrintText, Flow> {
        let original_buffer = self.current_buffer;
        let callback_buffer = requested_buffer
            .filter(|buffer| self.buffers.get(*buffer).is_some())
            .unwrap_or(original_buffer);
        if self.current_buffer != callback_buffer {
            self.set_current_buffer(callback_buffer);
        }
        let result = (|| {
            self.reset_print_numbering();
            let prepared = if self.print_var("print-circle").truthy() {
                super::circle::preprocess_for_print(self, value)?
            } else {
                None
            };
            let mut text = PrintText::new();
            let mut printer = Printer::mutable(self, callback_buffer, output_stream);
            let scope = PrintScope::enter(printer.interp(), value, prepared);
            let rendered = mode.render(&mut printer, value, &mut text, 0, false);
            drop(scope);
            drop(printer);
            rendered?;
            record_recursive_print(&text);
            Ok(text)
        })();
        if self.current_buffer != original_buffer && self.buffers.get(original_buffer).is_some() {
            self.set_current_buffer(original_buffer);
        }
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
                entry_keys: Default::default(),
                printed_entries: Default::default(),
                substitutions: Default::default(),
                printed_order: Vec::new(),
                next: 17,
            })
        });
        BEING_PRINTED.with(|stack| stack.borrow_mut().push(42));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _scope = PrintScope::enter(&i, &Value::Nil, None);
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
