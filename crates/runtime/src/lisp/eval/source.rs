//! String/buffer evaluation entry points and positioned source reading.

use super::{Interp, lexenv_root};
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    reader::Reader,
    value::Value,
};
use std::rc::Rc;

impl Interp {
    // ---------- entry points ----------

    /// Read and evaluate all top-level forms in `src`.
    /// Returns the last value.
    pub fn eval_str(&mut self, src: &str) -> EvalResult {
        self.eval_source(src, false)
    }

    /// Buffer evaluation retains buffer-origin reader error positions.
    pub fn eval_str_buf(&mut self, src: &str) -> EvalResult {
        self.eval_source(src, true)
    }

    fn eval_source(&mut self, src: &str, from_buffer: bool) -> EvalResult {
        // Sample lexical-binding once, then restore the caller's lexical
        // environment and compiled provenance even when reading/eval fails.
        let lexical = self
            .obarray
            .intern_soft("lexical-binding")
            .map(|id| self.symbol_value(id).truthy())
            .unwrap_or(false);
        let saved = std::mem::replace(&mut self.lexenv, if lexical { lexenv_root() } else { None });
        let result = self.with_eval_context(false, |i| i.eval_str_inner(src, from_buffer));
        self.lexenv = saved;
        result
    }

    fn eval_str_inner(&mut self, src: &str, from_buffer: bool) -> EvalResult {
        // The reader borrows `self`, so create/drop it per form — but
        // share one collected char buffer across all reads.
        let chars: Rc<Vec<crate::lisp::text::EmacsChar>> =
            Rc::new(src.chars().map(Into::into).collect());
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            self.maybe_quit()?;
            let next = {
                let mut reader = Reader::with_chars(self, chars.clone());
                reader.set_position(pos);
                reader.from_buffer = from_buffer;
                match reader.read()? {
                    Some(f) => Some((f, reader.position())),
                    None => None,
                }
            };
            match next {
                Some((form, end)) => {
                    pos = end;
                    if self.dbg_flags & 0x10000 != 0 {
                        eprintln!(
                            "[eval@{}] {}",
                            end,
                            self.princ_to_string(&form)
                                .chars()
                                .take(80)
                                .collect::<crate::lisp::text::EmacsText>()
                        );
                    }
                    match self.eval(&form) {
                        Ok(v) => last = v,
                        // An uncaught throw is a `no-catch' error.
                        Err(Flow::Throw(tag, val)) => {
                            let nc = self.q("no-catch");
                            return Err(self.signal_data(nc, vec![tag, val]));
                        }
                        Err(f) => {
                            if self.dbg_flags & 0x1000 != 0 {
                                eprintln!(
                                    "[eval-err@{}] {} => {:?}",
                                    end,
                                    self.princ_to_string(&form)
                                        .chars()
                                        .take(120)
                                        .collect::<crate::lisp::text::EmacsText>(),
                                    f
                                );
                            }
                            return Err(f);
                        }
                    }
                }
                None => return Ok(last),
            }
        }
    }

    /// `--eval` semantics: GNU's `command-line-1' reads ONE object from
    /// the argument and evaluates it; trailing forms are ignored.  GNU
    /// runs `(let ((lexical-binding t)) (eval FORM t))' — the eval is
    /// always lexical, with `lexical-binding' bound to t inside.
    pub fn eval_first_form(&mut self, src: &str) -> EvalResult {
        self.eval_first_object(&Value::string(src))
    }

    pub(crate) fn eval_first_object(&mut self, source: &Value) -> EvalResult {
        self.with_eval_context(false, |i| i.eval_first_object_inner(source))
    }

    fn eval_first_object_inner(&mut self, source: &Value) -> EvalResult {
        // read-from-string signals end-of-file for empty/comment-only input;
        // the reader's end position is deliberately ignored by the CLI.
        let Value::Str(text) = source else {
            return Err(self.wrong_type_mut("stringp", source));
        };
        let text = text
            .try_borrow()
            .map(|text| text.clone())
            .map_err(|_| self.error("Cannot inspect a mutably borrowed source string"))?;
        let multibyte = crate::lisp::builtins::strfn::string_multibyte_p(self, source);
        let (form, _) = self.read_from_string_kind(&text, 0, None, multibyte)?;
        let mark = self.specbind_depth();
        let lb = self.q("lexical-binding");
        if let Err(error) = self.specbind(lb, Value::t()) {
            // A watcher can signal after specbind has installed its unwind
            // entry. Restore that entry before returning the binding error.
            self.unbind_to(mark)?;
            return Err(error);
        }
        let saved_lexenv = std::mem::replace(&mut self.lexenv, lexenv_root());
        let result =
            self.with_call_frame("eval", vec![form.clone(), Value::t()], |i| i.eval(&form));
        self.lexenv = saved_lexenv;
        self.unbind_to(mark)?;
        match result {
            Ok(value) => Ok(value),
            Err(Flow::Throw(tag, value)) => {
                let no_catch = self.q("no-catch");
                Err(self.signal_data(no_catch, vec![tag, value]))
            }
            Err(flow) => Err(flow),
        }
    }

    /// Evaluate every top-level form in SRC, returning the last value.
    /// Command-line --eval/--execute use eval_first_form instead.
    pub fn eval_forms(&mut self, src: &str) -> EvalResult {
        self.with_eval_context(false, |i| i.eval_forms_inner(src))
    }

    fn eval_forms_inner(&mut self, src: &str) -> EvalResult {
        let chars: Rc<Vec<crate::lisp::text::EmacsChar>> =
            Rc::new(src.chars().map(Into::into).collect());
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            self.maybe_quit()?;
            let next = {
                let mut reader = Reader::with_chars(self, chars.clone());
                reader.set_position(pos);
                match reader.read() {
                    Ok(Some(form)) => Some((form, reader.position())),
                    // Reader::read distinguishes clean EOF after skipped
                    // whitespace/comments from EOF inside an unfinished
                    // object. Do not turn the latter into successful eval.
                    Ok(None) => None,
                    Err(error) => return Err(error),
                }
            };
            let Some((form, end)) = next else { break };
            if end <= pos {
                break; // reader made no progress — done
            }
            pos = end;
            let mark = self.specbind_depth();
            let lb = self.q("lexical-binding");
            if let Err(error) = self.specbind(lb, Value::t()) {
                // specbind may have pushed its unwind record before a
                // variable watcher signals. Unwind it, allowing cleanup
                // errors to replace the original binding error.
                self.unbind_to(mark)?;
                return Err(error);
            }
            let saved_lexenv = std::mem::replace(&mut self.lexenv, lexenv_root());
            let r = self.with_call_frame("eval", vec![form.clone(), Value::t()], |i| i.eval(&form));
            self.lexenv = saved_lexenv;
            self.unbind_to(mark)?;
            match r {
                Ok(v) => last = v,
                Err(Flow::Throw(tag, val)) => {
                    let nc = self.q("no-catch");
                    return Err(self.signal_data(nc, vec![tag, val]));
                }
                Err(f) => return Err(f),
            }
        }
        Ok(last)
    }

    /// `read-from-string` core: read one object, return it + end position.
    pub fn read_from_string<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        src: &T,
        start: usize,
    ) -> Result<(Value, usize), Flow> {
        self.read_from_string_pos(src, start, None)
    }

    /// `read_from_string' with `read-positioning-symbols' mode: symbol
    /// tokens become `symbol-with-pos' records carrying `base + token
    /// char index' as their position.  Under a non-nil
    /// `symbols-with-pos-enabled' the reader emits GNU's enabled sympos
    /// instead (real symbols for all primitives; `eq' unwraps them).
    /// The flag also activates position emission for plain `read' calls
    /// (GNU `readevalloop' parity).
    pub fn read_from_string_pos<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        src: &T,
        start: usize,
        pos_base: Option<i128>,
    ) -> Result<(Value, usize), Flow> {
        self.read_from_string_kind(src, start, pos_base, true)
    }

    pub(crate) fn read_from_string_kind<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        src: &T,
        start: usize,
        pos_base: Option<i128>,
        multibyte: bool,
    ) -> Result<(Value, usize), Flow> {
        let mut reader = Reader::new(self, src);
        reader.source_multibyte = multibyte;
        reader.set_position(start);
        // GNU parity: `locate_syms' is a per-read flag set only by
        // `read-positioning-symbols' (our explicit pos_base).  Plain
        // `read'/`read-from-string' never position symbols, no matter
        // how `symbols-with-pos-enabled' is bound.
        reader.annotate_pos = pos_base;
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }

    /// Buffer-source read: like `read_from_string_pos' but read errors
    /// carry GNU's `(MSG LINE COLUMN)' invalid-read-syntax data.
    pub fn read_from_string_buf<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        src: &T,
        start: usize,
    ) -> Result<(Value, usize), Flow> {
        let mut reader = Reader::new(self, src);
        reader.set_position(start);
        reader.from_buffer = true;
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_entry_points_unwind_lexical_binding_watcher_errors() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar eval-forms-watcher-ran nil)\
                         (defun eval-forms-watcher (_symbol _value operation _where)\
                           (if (eq operation 'let) (signal 'file-error '(\"bind\")) nil))\
                         (add-variable-watcher 'lexical-binding #'eval-forms-watcher)",
                    )
                    .unwrap();

                let mark = interp.specbind_depth();
                let result = interp.eval_forms("(setq eval-forms-watcher-ran t)");
                let signal_name = match &result {
                    Err(Flow::Signal(signal)) => interp.princ_to_string(&signal.symbol()),
                    _ => "<non-signal>".into(),
                };
                assert!(
                    matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
                    "unexpected eval_forms result: {result:?}, signal={signal_name}"
                );
                assert!(
                    interp
                        .symbol_value(interp.intern_soft("eval-forms-watcher-ran").unwrap())
                        .is_nil()
                );
                assert_eq!(interp.specbind_depth(), mark);

                let result = interp.eval_str("(when t (setq eval-forms-watcher-ran t))");
                assert!(
                    matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
                    "unexpected macro expansion result: {result:?}"
                );
                assert!(
                    interp
                        .symbol_value(interp.intern_soft("eval-forms-watcher-ran").unwrap())
                        .is_nil()
                );
                assert_eq!(interp.specbind_depth(), mark);

                let result = interp.eval_first_form("(setq eval-forms-watcher-ran t)");
                assert!(
                    matches!(result, Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)),
                    "unexpected eval_first_form result: {result:?}"
                );
                assert!(
                    interp
                        .symbol_value(interp.intern_soft("eval-forms-watcher-ran").unwrap())
                        .is_nil()
                );
                assert_eq!(interp.specbind_depth(), mark);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
