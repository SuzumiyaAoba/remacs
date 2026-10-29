//! Print destinations, echo-area output, and *Messages* history.
//!
//! Keep destination validation, character representation, and message-log
//! updates together; evaluation and minibuffer input remain in the parent.

use super::{Interp, OutputSink};
use crate::lisp::{error::Flow, obarray::sym, print_text, text, value::Value};

impl Interp {
    // ---------- output / echo ----------

    /// Send printed output to the current destination
    /// (`standard-output`, capture buffer, or the editor's sink).
    pub fn write_output<T: print_text::OutputSource + ?Sized>(
        &mut self,
        s: &T,
    ) -> Result<(), Flow> {
        self.write_output_to(s, &Value::Nil)
    }

    /// Is the print destination STREAM currently at the beginning of a
    /// line?  Buffers/markers peek at the preceding character; the real
    /// output sink uses `out_last_char` (GNU's print_position).
    pub fn output_at_bol(&mut self, stream: &Value) -> bool {
        if self.capture_output {
            return self.output_buffer.ends_with('\n');
        }
        let dest = match stream {
            Value::Nil => self.symbol_value(self.standard_output_sym),
            v => v.clone(),
        };
        match dest {
            Value::Buffer(b) => {
                let bb = b.borrow();
                let p = bb.point();
                p <= bb.begv || bb.text.char_at(p - 1) == '\n'
            }
            Value::Marker(m) => {
                let mm = m.borrow();
                match mm.buffer.and_then(|id| self.buffers.get(id)) {
                    Some(b) => {
                        let bb = b.borrow();
                        mm.position <= bb.begv || bb.text.char_at(mm.position - 1) == '\n'
                    }
                    None => false,
                }
            }
            // Function streams carry no position state — GNU treats
            // them as never at BOL.
            Value::Lambda(_) | Value::Subr(_) => false,
            Value::Sym(sid) if sid != sym::T => false,
            _ => self.out_last_char.is_some_and(|c| c == '\n'),
        }
    }

    pub(crate) fn sink_text_mb<T: print_text::OutputSource + ?Sized>(
        s: &T,
        mb: bool,
    ) -> text::EmacsText {
        s.with_output(|output| output.to_text(mb))
    }

    pub(crate) fn sink_text<T: print_text::OutputSource + ?Sized>(s: &T) -> text::EmacsText {
        Self::sink_text_mb(s, true)
    }

    fn fn_stream_code(token: print_text::OutputToken) -> i128 {
        token.function_code()
    }

    pub(crate) fn checked_output_stream(&self, stream: &Value) -> Result<Value, Flow> {
        let dest = match stream {
            Value::Nil => self.symbol_value(self.standard_output_sym),
            v => v.clone(),
        };
        match &dest {
            Value::Buffer(b) if !b.borrow().live => {
                return Err(self.error("Selecting deleted buffer"));
            }
            Value::Marker(m)
                if m.borrow()
                    .buffer
                    .and_then(|id| self.buffers.get(id))
                    .is_none() =>
            {
                return Err(self.error("Marker does not point anywhere"));
            }
            _ => {}
        }
        // A nil standard-output means t; retain the resolved destination
        // even if a print-unreadable-function callback changes the variable.
        Ok(if dest.is_nil() { Value::t() } else { dest })
    }

    /// Send printed output to STREAM (nil → `standard-output', t → the
    /// real output sink, buffer/marker → insert, function → call).
    pub fn write_output_to<T: print_text::OutputSource + ?Sized>(
        &mut self,
        s: &T,
        stream: &Value,
    ) -> Result<(), Flow> {
        s.with_output(|s| {
        let dest = self.checked_output_stream(stream)?;
        self.out_last_char = s.last_character().or(self.out_last_char);
        if self.capture_output {
            self.output_buffer.push_str(&Self::sink_text(s));
            return Ok(());
        }
        match &dest {
            Value::Buffer(b) => {
                let bb = b.borrow();
                let bid = bb.id;
                let ems = self.q("enable-multibyte-characters");
                let mb = bb
                    .locals
                    .get(&ems)
                    .map(|v| v.truthy())
                    .unwrap_or_else(|| self.symbol_value(ems).truthy());
                drop(bb);
                let text = Self::sink_text_mb(s, mb);
                crate::buffer::primitives::chg_with_buffer(self, bid, |i| {
                    crate::buffer::primitives::chg_insert_pt(i, &text, false)
                })?;
            }
            Value::Marker(m) => {
                // GNU inserts before the marker, then advances it past
                // the inserted text (escapes are inserted as text).
                let mm = m.borrow();
                if let Some(buf_id) = mm.buffer {
                    let pos = mm.position;
                    drop(mm);
                    let ems = self.q("enable-multibyte-characters");
                    let mb = self
                        .buffers
                        .get(buf_id)
                        .map(|b| {
                            let bb = b.borrow();
                            bb.locals
                                .get(&ems)
                                .map(|v| v.truthy())
                                .unwrap_or_else(|| self.symbol_value(ems).truthy())
                        })
                        .unwrap_or(true);
                    let text = Self::sink_text_mb(s, mb);
                    if self.buffers.get(buf_id).is_some() {
                        crate::buffer::primitives::chg_with_buffer(self, buf_id, |i| {
                            crate::buffer::primitives::chg_insert(i, pos, &text)
                        })?;
                    }
                    let newpos = pos + text.chars().count();
                    m.borrow_mut().position = newpos;
                }
            }
            // GNU calls a function print stream once per character;
            // unibyte byte-chars arrive as raw bytes (princ) or the
            // eight-bit char code (prin1), per fn_stream_code.
            Value::Lambda(_) | Value::Subr(_) => {
                for ch in s.tokens() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            Value::Cons(c)
                if {
                    let cb = c.borrow();
                    matches!(&cb.car, Value::Sym(s) if *s == self.q("lambda") || *s == self.q("closure"))
                } =>
            {
                for ch in s.tokens() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            Value::Sym(sid) if *sid != sym::T => {
                for ch in s.tokens() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
            // GNU `printcharfun': any non-nil/t, non-buffer, non-marker,
            // non-function value is treated as a FUNCTION name — ints,
            // strings, vectors, non-function lists, hash-tables all die
            // in funcall with (invalid-function VALUE).
            Value::Nil | Value::Sym(_) => {
                // nil or t: the real print destination — echo area
                // interactively, stdout in batch.
                match &self.output {
                    Some(OutputSink::Buffer(buf)) => buf.borrow_mut().push_str(&Self::sink_text(s)),
                    Some(OutputSink::Function(f)) => {
                        let f = f.clone();
                        let arg = Value::string(Self::sink_text(s).as_ref());
                        let _ = self.apply(&f, vec![arg]);
                    }
                    Some(OutputSink::Stdout) => {
                        crate::stdio::write(&s.stdout_bytes());
                        self.stderr_need_newline = true;
                    }
                    None => self.echo_message.push_str(&Self::sink_text(s)),
                }
            }
            _ => {
                // Non-nil/t, non-buffer/marker/function values are
                // function NAMES — apply yields the GNU errors
                // ((invalid-function 5) / ((invalid-function (1)) etc).
                for ch in s.tokens() {
                    self.apply(&dest, vec![Value::Int(Self::fn_stream_code(ch))])?;
                }
            }
        }
        Ok(())
            })
    }

    pub(crate) fn set_echo_message_with_properties(
        &mut self,
        message: text::EmacsText,
        properties: Vec<(usize, usize, Vec<Value>)>,
    ) {
        self.echo_message = message;
        self.echo_message_props = properties;
    }

    /// `message` — show a string in the echo area and log it to
    /// *Messages*.  GNU's `message_dolog' logs in batch too: the
    /// buffer is created on demand (undo disabled), consecutive
    /// duplicate lines collapse into `msg [N times]', and
    /// `message-log-max' bounds the line count from the top.
    /// Insertions bypass read-only/change-hook machinery (GNU's
    /// `insert_1_both' with PREPARE=false).
    pub fn message<T: text::TextSource + ?Sized>(&mut self, s: &T) {
        self.message_with_properties(s, Vec::new());
    }

    pub(crate) fn message_with_properties<T: text::TextSource + ?Sized>(
        &mut self,
        s: &T,
        properties: Vec<(usize, usize, Vec<Value>)>,
    ) {
        s.with_text(|s| {
            // Boot loads construct the dumped image.  Their normal messages
            // are logged and discarded at the end of startup, like GNU's
            // dump-time output, instead of entering the user's echo session.
            if self.boot_loading {
                self.message_log(s);
                return;
            }
            if self.noninteractive {
                if self.stderr_need_newline {
                    eprintln!();
                }
                self.stderr_need_newline = false;
                let mut stderr = std::io::stderr().lock();
                let bytes = s.to_owned().to_internal_bytes(true).unwrap();
                let _ = std::io::Write::write_all(&mut stderr, &bytes);
                let _ = std::io::Write::write_all(&mut stderr, b"\n");
            } else {
                self.set_echo_message_with_properties(s.to_string(), properties);
            }
            self.message_log(s);
            // GNU routes `message' to stderr in batch and the echo area
            // interactively — never to the `princ'/stdout stream.
        })
    }

    /// `message_dolog' body: append `s\n' to *Messages*, folding a run
    /// of identical messages into `msg [N times]'.  All positions are
    /// 0-based char indices.
    pub(crate) fn message_log<T: text::TextSource + ?Sized>(&mut self, s: &T) {
        s.with_text(|s| {
            let mut line = s.to_owned();
            line.push('\n');
            let mb = match self.buffers.by_name("*Messages*") {
                Some(id) => id,
                None => {
                    let id = self.create_buffer("*Messages*");
                    // GNU message_dolog sets buffer-undo-list=t on a fresh
                    // *Messages* (it "doesn't have undo").
                    if let Some(b) = self.buffers.get(id) {
                        b.borrow_mut().set_undo_list(Value::t());
                    }
                    id
                }
            };
            let Some(b) = self.buffers.get(mb) else {
                return;
            };
            let mut bb = b.borrow_mut();
            let z = bb.text_len();
            // GNU appends `s\n' then message_log_check_duplicate compares
            // it with the previous line; equal lines fold.
            let prev_bol = {
                let mut i = z;
                if i > 0 && bb.text.char_at(i - 1) == '\n' {
                    i -= 1;
                }
                while i > 0 && bb.text.char_at(i - 1) != '\n' {
                    i -= 1;
                }
                i
            };
            if prev_bol < z {
                let mut eol = prev_bol;
                while eol < z && bb.text.char_at(eol) != '\n' {
                    eol += 1;
                }
                let prev: text::EmacsText = (prev_bol..eol).map(|k| bb.text.char_at(k)).collect();
                // Strip a trailing " [N times]" suffix for the base text.
                let (base, count) = match prev.rfind(" [") {
                    Some(pos) => {
                        let suf = &prev[pos + 2..];
                        let digits = suf.strip_suffix(" times]");
                        match digits {
                            Some(d) if !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()) => {
                                (prev[..pos].to_string(), d.parse::<usize>().unwrap_or(1))
                            }
                            _ => (prev.clone(), 1),
                        }
                    }
                    None => (prev.clone(), 1),
                };
                if &*base == s {
                    let n = count + 1;
                    // GNU del_range_both(prev_bol, this_bol): drop the old
                    // line (incl. newline), then re-append the suffix.
                    crate::buffer::primitives::delete_preserving_undo(
                        self,
                        &mut bb,
                        prev_bol,
                        eol + 1,
                        false,
                    );
                    let z3 = bb.text_len();
                    bb.raw_insert_at(z3, &line);
                    let z2 = bb.text_len();
                    bb.raw_insert_at(z2.saturating_sub(1), &format!(" [{} times]", n));
                    let z4 = bb.text_len();
                    bb.set_point(z4);
                    drop(bb);
                    self.trim_message_log(mb);
                    return;
                }
            }
            let at_end = bb.point() >= z;
            bb.raw_insert_at(z, &line);
            if at_end {
                let z5 = bb.text_len();
                bb.set_point(z5);
            }
            drop(bb);
            self.trim_message_log(mb);
        })
    }

    /// `message-log-max': keep at most that many trailing lines in
    /// *Messages* (GNU del_range_both(BEG, BOL-of-line N-before-end)).
    fn trim_message_log(&mut self, mb: usize) {
        let ml = self.q("message-log-max");
        let Value::Int(maxl) = self.symbol_value(ml) else {
            return;
        };
        if maxl <= 0 {
            return;
        }
        let Some(b) = self.buffers.get(mb) else {
            return;
        };
        let mut bb = b.borrow_mut();
        let z = bb.text_len();
        let mut lines = 0usize;
        let mut cut = 0usize;
        for k in (0..z).rev() {
            if bb.text.char_at(k) == '\n' {
                lines += 1;
                if lines > maxl as usize {
                    cut = k + 1;
                    break;
                }
            }
        }
        if cut > 0 {
            crate::buffer::primitives::delete_preserving_undo(self, &mut bb, 0, cut, false);
        }
    }
}
