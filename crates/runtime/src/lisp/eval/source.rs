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
        // read-from-string signals end-of-file for empty/comment-only input;
        // the reader's end position is deliberately ignored by the CLI.
        let Value::Str(text) = source else {
            return Err(self.wrong_type_mut("stringp", source));
        };
        let text = text.borrow().clone();
        let multibyte = crate::lisp::builtins::strfn::string_multibyte_p(self, source);
        let (form, _) = self.read_from_string_kind(&text, 0, None, multibyte)?;
        let mark = self.specbind_depth();
        let lb = self.q("lexical-binding");
        self.specbind(lb, Value::t())?;
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
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            let (form, end) = match self.read_from_string(src, pos) {
                Ok(v) => v,
                Err(Flow::Signal(signal)) => {
                    // A read error past the last real form behaves
                    // like EOF: trailing whitespace/comments only.
                    let is_eof = matches!(&signal.symbol(), Value::Sym(id) if {
                        let n = self.symbol_name(*id);
                        n == "end-of-file"
                    });
                    if is_eof {
                        break;
                    }
                    return Err(Flow::Signal(signal));
                }
                Err(f) => return Err(f),
            };
            if end <= pos {
                break; // reader made no progress — done
            }
            pos = end;
            let mark = self.specbind_depth();
            let lb = self.q("lexical-binding");
            let _ = self.specbind(lb, Value::t());
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
