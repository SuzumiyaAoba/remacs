//! Minibuffer lifetime, history, and front-end/batch input.

use super::{Interp, state::MinibufferCaller};
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    value::{SymId, Value},
};
use std::rc::Rc;

fn specbind_in_scope(
    interp: &mut Interp,
    mark: usize,
    symbol: SymId,
    value: Value,
) -> Result<(), Flow> {
    if let Err(error) = interp.specbind(symbol, value) {
        interp.unbind_to(mark)?;
        return Err(error);
    }
    Ok(())
}

impl Interp {
    /// Read minibuffer input via the front-end hook. `single` reads one
    /// raw key event instead of a full line.
    pub fn minibuf_input<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
        single: bool,
    ) -> Result<MinibufInput, Flow> {
        prompt.with_text(|prompt| {
            let reader = self.minibuf_reader.clone();
            match reader {
                Some(r) => r(self, prompt, single),
                None => Err(self.error("minibuffer input unavailable")),
            }
        })
    }

    /// Buffer text after the prompt field of the active minibuffer —
    /// what a finished `read-from-minibuffer' returns.
    pub fn minibuf_contents(&self) -> crate::lisp::text::EmacsText {
        let Some(b) = self.buffers.get(self.current_buffer) else {
            return crate::lisp::text::EmacsText::new();
        };
        let bb = b.borrow();
        let end = bb.text_len();
        let start = self
            .minibuf_prompts
            .last()
            .map(|p| p.chars().count())
            .unwrap_or(0)
            .min(end);
        bb.text.substring(start, end)
    }

    /// GNU `get_minibuffer' (minibuf.c): the ` *Minibuf-{depth}*'
    /// buffer for DEPTH, (re)creating dead or missing entries.
    /// Depth 0 is ` *Minibuf-0*', the never-active null minibuffer.
    pub fn get_minibuffer(&mut self, depth: usize) -> usize {
        while self.minibuf_list.len() <= depth {
            self.minibuf_list.push(usize::MAX);
        }
        let mut id = self.minibuf_list[depth];
        if id == usize::MAX || !self.buffer_live(id) {
            let name = format!(" *Minibuf-{depth}*");
            id = self
                .buffers
                .by_name(&name)
                .unwrap_or_else(|| self.create_buffer(&name));
            self.minibuf_list[depth] = id;
        }
        id
    }

    /// Is `id` a real minibuffer (on `Vminibuffer_list')?
    pub fn is_minibuffer(&self, id: usize) -> bool {
        self.minibuf_list.iter().any(|&x| x == id)
    }

    /// Point `buf`'s frame minibuffer window at buffer `id`
    /// (GNU `set_window_buffer (minibuf_window, buf)').
    pub fn set_minibuf_window_buffer(&mut self, id: usize) {
        let window = self
            .selected_frame
            .as_ref()
            .and_then(|f| f.borrow().minibuffer.clone());
        if let Some(w) = window {
            w.borrow_mut().buffer = id;
            crate::editor::record_buffer_display(self, id);
        }
    }

    /// GNU `read_minibuf' (minibuf.c), interactive path only:
    /// switch to ` *Minibuf-{depth}*', install the prompt with its
    /// `field'/`front-sticky'/`rear-nonsticky' and
    /// `minibuffer-prompt-properties', run `minibuffer-setup-hook',
    /// read input, then run `minibuffer-exit-hook' (safe) and unwind.
    /// GNU runs the setup hook via `run_hook', so an error in it
    /// aborts the read — but the exit hook and the state restoration
    /// still run, exactly like the C unwind-protect chain.
    pub fn minibuf_read<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
        args: MinibufArgs,
    ) -> Result<crate::lisp::text::EmacsText, Flow> {
        prompt.with_text(|prompt| {
            // Fread_from_minibuffer's HIST handling: symbol → (sym . 0),
            // cons → (HISTVAR . HISTPOS), nil HISTVAR → `minibuffer-history'.
            let (histvar, histpos) = match &args.hist {
                Value::Cons(c) => {
                    let (hv, hp) = {
                        let cc = c.borrow();
                        (cc.car.clone(), cc.cdr.int().unwrap_or(0))
                    };
                    (hv, hp)
                }
                v => (v.clone(), 0),
            };
            let histvar = if histvar.is_nil() {
                Value::Sym(self.q("minibuffer-history"))
            } else {
                histvar
            };

            // GNU: `enable-recursive-minibuffers' nil + already inside a
            // minibuffer → `user-error' in the minibuffer window, else
            // `throw 'exit' with the message.
            if self.minibuf_level > 0 {
                let erm = self
                    .intern_soft("enable-recursive-minibuffers")
                    .map(|id| self.symbol_value(id).truthy())
                    .unwrap_or(false);
                if !erm {
                    let msg = "Command attempted to use minibuffer while in minibuffer";
                    if self.is_minibuffer(self.current_buffer) {
                        let ue = self.q("user-error");
                        return Err(self.signal_data(ue, vec![Value::string(msg)]));
                    }
                    return Err(Flow::Throw(Value::Sym(self.q("exit")), Value::string(msg)));
                }
            }

            let mark = self.specbind_depth();
            // specbind minibuffer-default ← DEFALT, inhibit-read-only ← nil.
            let mdef = self.q("minibuffer-default");
            specbind_in_scope(self, mark, mdef, args.defalt.clone())?;
            let iro = self.q("inhibit-read-only");
            specbind_in_scope(self, mark, iro, Value::Nil)?;
            // specbind minibuffer-completing-file-name ← current value;
            // a `lambda' on entry (t from an outer read) normalizes to nil.
            let cf = self.q("minibuffer-completing-file-name");
            let cfv = self.symbol_value(cf);
            specbind_in_scope(self, mark, cf, cfv.clone())?;
            if matches!(cfv, Value::Sym(s) if s == sym::LAMBDA) {
                if let Err(error) = self.set_symbol(cf, Value::Nil) {
                    self.unbind_to(mark)?;
                    return Err(error);
                }
            }

            // GNU creates the depth-N minibuffer before bumping the level.
            let depth = self.minibuf_level.max(0) as usize + 1;
            let mb_id = self.get_minibuffer(depth);
            self.minibuf_level += 1;
            // GNU `get_minibuffer': `Vcommand_loop_level_list[depth]' ←
            // `command_loop_level' (remacs `recursion_depth').  Rewritten
            // on every read, not just when the buffer is created.
            while self.minibuf_cll.len() <= depth {
                self.minibuf_cll.push(0);
            }
            self.minibuf_cll[depth] = self.recursion_depth;
            let saved_buf = self.current_buffer;

            // minibuf_save_list vars (per-level dynamic state).
            let mhp = self.q("minibuffer-history-position");
            let mhv = self.q("minibuffer-history-variable");
            let hform = self
                .intern_soft("minibuffer-help-form")
                .map(|id| self.symbol_value(id))
                .unwrap_or(Value::Nil);
            let hf = self.q("help-form");
            let cpa = self.q("current-prefix-arg");
            let cpav = self.symbol_value(cpa);
            // The C reader restores these per-level Lisp variables from
            // minibuf_save_list in read_minibuf_unwind, before calling
            // minibuffer-inactive-mode. The earlier minibuffer-default,
            // inhibit-read-only, and completing-file-name bindings remain
            // active until the outer unbind_to after that cleanup.
            let per_level_mark = self.specbind_depth();
            let bindings = (|| {
                self.specbind(mhp, Value::Int(histpos))?;
                self.specbind(mhv, histvar.clone())?;
                self.specbind(hf, hform)?;
                self.specbind(cpa, cpav)?;
                // Non-nil completing-file-name becomes `lambda' for this
                // read; a `lambda' on entry was normalized above.
                if self.symbol_value(cf).truthy() {
                    let lam = Value::Sym(self.q("lambda"));
                    self.set_symbol(cf, lam)?;
                }
                // GNU initializes an unbound history variable before the
                // read, and a watcher error aborts the minibuffer entry.
                if let Value::Sym(hsid) = &histvar {
                    if matches!(self.obarray.symbol(*hsid).value, Value::Sym(s) if s == sym::UNBOUND)
                        && !self.obarray.symbol(*hsid).constant
                    {
                        self.set_symbol(*hsid, Value::Nil)?;
                    }
                }
                Ok(())
            })();
            if let Err(error) = bindings {
                self.minibuf_level -= 1;
                self.unbind_to(mark)?;
                return Err(error);
            }

            self.minibuf_prompts.push(prompt.to_string());

            // ---- enter the minibuffer ----
            self.set_current_buffer(mb_id);
            // GNU set_minibuffer_mode: `minibuffer-mode' if fbound (it
            // resets locals and runs its mode hook).
            let mm = self.q("minibuffer-mode");
            let mode_r = if self.fbound_p(mm) {
                self.apply(&Value::Sym(mm), vec![])
            } else {
                Ok(Value::Nil)
            };
            // bset_truncate_lines (current_buffer, Qnil).
            let tl = self.q("truncate-lines");
            if let Some(b) = self.buffers.get(mb_id) {
                b.borrow_mut().locals.insert(tl, Value::Nil);
            }
            // Display the minibuffer in the mini window.
            self.set_minibuf_window_buffer(mb_id);
            // GNU `Fselect_window (minibuf_window, Qnil)': the read runs
            // with the minibuffer window selected (window-minibuffer-p,
            // minibuffer-selected-window & friends observe it).
            let caller = MinibufferCaller::capture(self, saved_buf);
            let selected_window = self
                .selected_frame
                .as_ref()
                .map(|frame| frame.borrow().selected.clone());
            let minibuffer_window = self
                .selected_frame
                .as_ref()
                .and_then(|frame| frame.borrow().minibuffer.clone());
            let already_selected_minibuffer = match (&selected_window, &minibuffer_window) {
                (Some(selected), Some(minibuffer)) => Rc::ptr_eq(selected, minibuffer),
                _ => false,
            };
            // GNU preserves the original caller window during recursive
            // reads in the same minibuffer window, and replaces it only
            // for a new top-level read or a different minibuffer window.
            if depth == 1 || !already_selected_minibuffer {
                self.minibuf_selected_window = selected_window;
            }
            if let Some(f) = &self.selected_frame {
                let mbw = f.borrow().minibuffer.clone();
                if let Some(w) = mbw {
                    f.borrow_mut().selected = w;
                }
            }

            // Erase, insert prompt + initial input — GNU binds
            // inhibit-read-only and inhibit-modification-hooks around it.
            let input_r = mode_r.and_then(|_| {
                let m2 = self.specbind_depth();
                let iro = self.q("inhibit-read-only");
                let imh = self.q("inhibit-modification-hooks");
                let r = (|| {
                    self.specbind(iro, Value::t())?;
                    self.specbind(imh, Value::t())?;
                    self.minibuf_install(prompt, &args.initial, mb_id)
                })();
                self.unbind_to(m2)?;
                r
            });

            // Local keymap: KEYMAP arg or `minibuffer-local-map' (GNU
            // substitutes it for nil before bset_keymap).
            let input_r = input_r.and_then(|_| {
                let map = if args.keymap.is_nil() {
                    self.q_soft("minibuffer-local-map")
                        .map(|id| self.symbol_value(id))
                        .unwrap_or(Value::Nil)
                } else {
                    args.keymap.clone()
                };
                let km = self.q("local-keymap");
                if let Some(b) = self.buffers.get(mb_id) {
                    b.borrow_mut().locals.insert(km, map);
                }
                // GNU: no undo past this point.
                if let Some(b) = self.buffers.get(mb_id) {
                    b.borrow_mut().set_undo_list(Value::Nil);
                }
                // `minibuffer-setup-hook' — GNU `run_hook' (not safe):
                // an error aborts the read after unwinding.
                match crate::lisp::builtins::evalfn::call_hook(self, "minibuffer-setup-hook") {
                    Ok(_) => {
                        // GNU wraps the command loop in
                        // `internal_catch (Qexit, ...)': `exit-minibuffer'
                        // throws `exit', ending the read.  The front-end
                        // loop reports the throw via `minibuf_exited'
                        // (a raw Flow::Throw can't cross its io boundary).
                        let exit_sym = Value::Sym(self.q("exit"));
                        self.catch_tags.push(exit_sym.clone());
                        let prev_catching = self.minibuf_catching_exit;
                        self.minibuf_catching_exit = true;
                        self.minibuf_exited = false;
                        // A scripted `MinibufInput::Key' is dispatched
                        // through the minibuffer's real keymaps (the
                        // front-end's `minibuf_loop' behavior): remap
                        // bindings like y-or-n-p-map's `y-or-n-p-insert-*'
                        // run as commands instead of inserting raw text.
                        let mut keys: Vec<i128> = Vec::new();
                        let mut arg_mode = false;
                        let r = loop {
                            match self.minibuf_input(prompt, false) {
                                Ok(MinibufInput::Key(k)) => {
                                    if let Err(e) = crate::frontend::minibuf_dispatch_key(
                                        self,
                                        k,
                                        &mut keys,
                                        &mut arg_mode,
                                    ) {
                                        break Err(e);
                                    }
                                    if let Some(f) = self.minibuf_exit_fn.take() {
                                        break Ok(MinibufInput::Call(f));
                                    }
                                    if self.minibuf_exited {
                                        break Ok(MinibufInput::Text(self.minibuf_contents()));
                                    }
                                    if let Some(f) = self.minibuf_pending_flow.take() {
                                        break Err(f);
                                    }
                                    continue;
                                }
                                other => break other,
                            }
                        };
                        self.minibuf_catching_exit = prev_catching;
                        self.catch_tags.pop();
                        match r {
                            // An `exit' throw that bypassed the front-end
                            // flag (test readers, throws from Lisp): GNU
                            // signals `error' for a thrown string, else
                            // ends the read with the buffer contents.
                            Err(Flow::Throw(tag, v)) if crate::lisp::eq_values(&tag, &exit_sym) => {
                                match &v {
                                    Value::Str(_) => {
                                        let es = Value::Sym(self.q("error"));
                                        Err(Flow::signal(es, Value::list(vec![v.clone()])))
                                    }
                                    Value::Sym(_) if v.truthy() => Err(Flow::Quit),
                                    _ => Ok(MinibufInput::Text(self.minibuf_contents())),
                                }
                            }
                            // GNU `recursive_edit_1' calls a function
                            // thrown through `exit' before unbinding —
                            // `minibuffer-quit-recursive-edit' throws a
                            // lambda that signals `minibuffer-quit'.
                            Ok(MinibufInput::Call(f)) => match self.apply(&f, vec![]) {
                                Ok(_) => Ok(MinibufInput::Text(self.minibuf_contents())),
                                Err(e) => Err(e),
                            },
                            other => other,
                        }
                    }
                    Err(f) => Err(f),
                }
            });

            // ---- unwind: run_exit_minibuf_hook, then read_minibuf_unwind.
            if self.buffer_live(mb_id) {
                self.set_current_buffer(mb_id);
            }
            let hook_r =
                crate::lisp::builtins::evalfn::safe_call_hook(self, "minibuffer-exit-hook");
            self.minibuf_level -= 1;
            self.minibuf_prompts.pop();
            let per_level_unbind_r = self.unbind_to(per_level_mark);
            // GNU calls `minibuffer-inactive-mode' in the expired
            // minibuffer after restoring the per-level vars, while the
            // outer minibuffer bindings are still active.
            let inactive_mode_r = if self.buffer_live(mb_id) {
                self.set_current_buffer(mb_id);
                let im = self.q("minibuffer-inactive-mode");
                if self.fbound_p(im) {
                    self.apply(&Value::Sym(im), vec![])
                } else {
                    Ok(Value::Nil)
                }
            } else {
                Ok(Value::Nil)
            };
            // The mini window shows the next-less-nested (null) minibuffer.
            let idle = self.get_minibuffer(0);
            self.set_minibuf_window_buffer(idle);
            caller.restore(self);
            // Match read_minibuf's outer unbind_to: it runs after both
            // recorded cleanup functions, so its error has final precedence.
            let unbind_r = self.unbind_to(mark);
            unbind_r?;
            inactive_mode_r?;
            per_level_unbind_r?;
            if let Err(f) = hook_r {
                return Err(f);
            }

            let input = input_r?;

            // History push — after restoring the calling buffer (GNU:
            // "in case the history variable is buffer-local").
            if let MinibufInput::Text(text) = &input {
                let han = self
                    .intern_soft("history-add-new-input")
                    .map(|id| self.symbol_value(id).truthy())
                    .unwrap_or(true);
                let histstring = if !text.is_empty() {
                    Some(text.clone())
                } else {
                    match &args.defalt {
                        Value::Str(s) => Some(s.borrow().clone()),
                        Value::Cons(c) => {
                            let h = c.borrow().car.clone();
                            if let Value::Str(s) = h {
                                Some(s.borrow().clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                };
                if han && histstring.is_some() {
                    let ah = self.q("add-to-history");
                    if self.fbound_p(ah) {
                        self.apply(
                            &Value::Sym(ah),
                            vec![histvar.clone(), Value::string(histstring.unwrap())],
                        )?;
                    }
                }
            }

            match input {
                MinibufInput::Text(t) => Ok(t),
                MinibufInput::Key(k) => Ok(crate::lisp::value::lisp_char(k as u32)
                    .map(|c| c.to_string())
                    .unwrap_or_default()),
                MinibufInput::Event(_) => Ok(crate::lisp::text::EmacsText::new()),
                // Handled in the input phase (applied before unwinding);
                // unreachable here.
                MinibufInput::Call(_) => Ok(crate::lisp::text::EmacsText::new()),
            }
        })
    }

    /// Read a full input line via the front-end hook.
    pub fn minibuf_line<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
    ) -> Result<crate::lisp::text::EmacsText, Flow> {
        self.minibuf_read(prompt, MinibufArgs::default())
    }

    /// The minibuffer-enter half of GNU `read_minibuf': erase the
    /// buffer, insert the prompt with `field'/`front-sticky'/
    /// `rear-nonsticky' plus `minibuffer-prompt-properties' (faces
    /// appended, not overwriting), then insert INITIAL-CONTENTS and
    /// position point.
    fn minibuf_install<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
        initial: &Value,
        mb_id: usize,
    ) -> EvalResult {
        prompt.with_text(|prompt| {
            if let Some(b) = self.buffers.get(mb_id) {
                let mut bb = b.borrow_mut();
                bb.overlays.clear();
                let len = bb.text_len();
                if len > 0 {
                    crate::buffer::primitives::delete_preserving_undo(self, &mut bb, 0, len, false);
                }
                bb.set_point(0);
                if !prompt.is_empty() {
                    bb.insert(prompt);
                }
            }
            let pend = prompt.chars().count();
            // front-sticky t, rear-nonsticky t, field t over the prompt.
            if pend > 0 {
                for (key, val) in [
                    ("front-sticky", Value::t()),
                    ("rear-nonsticky", Value::t()),
                    ("field", Value::t()),
                ] {
                    let key_sym = self.intern(key);
                    crate::buffer::primitives::f_put_text_property(
                        self,
                        vec![
                            Value::Int(1),
                            Value::Int(pend as i128 + 1),
                            Value::Sym(key_sym),
                            val,
                        ]
                        .into(),
                    )?;
                }
                // minibuffer-prompt-properties plist: `face' is appended
                // via add-face-text-property, others via put-text-property.
                let mpp = self
                    .intern_soft("minibuffer-prompt-properties")
                    .map(|id| self.symbol_value(id))
                    .unwrap_or(Value::Nil);
                let face_property = self.intern_soft("face");
                if let Value::Cons(_) = &mpp {
                    let mut list = mpp.clone();
                    let mut seen = rustc_hash::FxHashSet::default();
                    while let Value::Cons(c) = list.clone() {
                        if !seen.insert(Rc::as_ptr(&c) as usize) {
                            return Err(self.signal_data(sym::CIRCULAR_LIST, vec![list.clone()]));
                        }
                        let (key, rest) = {
                            let cc = c.borrow();
                            (cc.car.clone(), cc.cdr.clone())
                        };
                        let (val, rest) = match &rest {
                            Value::Cons(c2) => {
                                let cc2 = c2.borrow();
                                (cc2.car.clone(), cc2.cdr.clone())
                            }
                            _ => (Value::Nil, Value::Nil),
                        };
                        let is_face = matches!(&key, Value::Sym(s) if Some(*s) == face_property);
                        if is_face {
                            crate::buffer::primitives::f_add_face_text_property(
                                self,
                                vec![Value::Int(1), Value::Int(pend as i128 + 1), val, Value::t()]
                                    .into(),
                            )?;
                        } else {
                            crate::buffer::primitives::f_put_text_property(
                                self,
                                vec![Value::Int(1), Value::Int(pend as i128 + 1), key, val].into(),
                            )?;
                        }
                        list = rest;
                    }
                }
            }
            // Initial input: string, or (STRING . POS) where POS is a
            // 1-based offset converted to distance-from-end.
            let (init, pos) = match initial {
                Value::Cons(c) => {
                    let (s, n) = {
                        let cc = c.borrow();
                        (cc.car.clone(), cc.cdr.clone())
                    };
                    let slen = match &s {
                        Value::Str(t) => t.borrow().chars().count() as i128,
                        _ => 0,
                    };
                    let p = match n.int() {
                        Some(x) if x < 1 => -slen,
                        Some(x) => x - 1 - slen,
                        None => 0,
                    };
                    (s, p)
                }
                v => (v.clone(), 0),
            };
            if let Value::Str(s) = &init {
                let s = s.borrow().clone();
                if let Some(b) = self.buffers.get(mb_id) {
                    let mut bb = b.borrow_mut();
                    bb.insert(&s);
                    let len = bb.text_len();
                    let target = (bb.point() as i128 + pos).clamp(pend as i128, len as i128);
                    bb.set_point(target as usize);
                }
            }
            Ok(Value::Nil)
        })
    }

    /// `(end-of-file "Error reading from stdin")' — the signal GNU
    /// raises when noninteractive minibuffer input hits EOF.
    pub fn batch_eof_flow(&mut self) -> Flow {
        let eof = self.q("end-of-file");
        self.signal_data(eof, vec![Value::string("Error reading from stdin")])
    }

    /// GNU `read_minibuf_noninteractive': echo PROMPT on stdout and
    /// read from stdin until `\n', `\r', or EOF.  A `\r' does not
    /// consume a following `\n' (GNU's getchar loop stops at `\r').
    /// EOF before any character signals `end-of-file'; a partial
    /// line at EOF is still returned.  When `read-hide-char' is a
    /// character, that many masking glyphs plus a newline are echoed.
    pub fn batch_read_line<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
    ) -> EvalResult {
        prompt.with_text(|prompt| {
            let mut output = crate::lisp::print_text::PrintText::new();
            output.push_str(prompt);
            crate::stdio::write(&output.stdout_bytes());
            crate::stdio::flush();
            let hide = self
                .intern_soft("read-hide-char")
                .map(|id| self.symbol_value(id))
                .and_then(|v| match v {
                    Value::Int(n) if (0..=0x3fffff).contains(&n) => {
                        crate::lisp::value::lisp_char(n as u32)
                    }
                    _ => None,
                });
            let mut line: Vec<u8> = Vec::new();
            let mut got_eof = false;
            loop {
                match crate::stdio::read_byte() {
                    None => {
                        got_eof = true;
                        break;
                    }
                    Some(b'\n' | b'\r') => break,
                    Some(byte) => line.push(byte),
                }
            }
            if let Some(h) = hide {
                let mut out = crate::lisp::print_text::PrintText::new();
                for _ in 0..line.len() {
                    out.push(h);
                }
                out.push('\n');
                crate::stdio::write(&out.stdout_bytes());
                crate::stdio::flush();
            }
            // GNU `read_minibuf_noninteractive' bypasses the minibuffer
            // entirely — `minibuffer-depth' stays 0, no hooks run.
            if got_eof && line.is_empty() {
                Err(self.batch_eof_flow())
            } else {
                let (text, multibyte) = crate::lisp::value::auto_string_bytes(&line);
                let value = Value::string_with_kind(text, multibyte);
                if let Value::Str(string) = &value {
                    if multibyte {
                        self.mark_multibyte(string);
                    } else {
                        self.mark_unibyte(string);
                    }
                }
                Ok(value)
            }
        })
    }

    /// Minibuffer line input in batch: read one line from stdin when
    /// running noninteractively.  Returns `None' when a front-end
    /// reader should handle it instead.
    pub fn batch_minibuf_line<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
    ) -> Result<Option<Value>, Flow> {
        prompt.with_text(|prompt| {
            if self.minibuf_reader.is_none() && self.noninteractive {
                return self.batch_read_line(prompt).map(Some);
            }
            Ok(None)
        })
    }

    /// True when the selected window is the minibuffer window
    /// (GNU `BASE_EQ (selected_window, minibuf_window)').
    pub fn selected_window_is_minibuffer(&self) -> bool {
        let Some(sel) = crate::editor::sel_window(self) else {
            return false;
        };
        let Some(frame) = &self.selected_frame else {
            return false;
        };
        match &frame.borrow().minibuffer {
            Some(mb) => Rc::ptr_eq(&sel, mb),
            None => false,
        }
    }

    /// GNU `string_to_object' (minibuf.c): read a Lisp object from
    /// VAL; an empty string falls back to DEFALT's string (or first
    /// string element).  Trailing non-whitespace signals
    /// `invalid-read-syntax'.
    pub fn string_to_object(&mut self, val: &Value, defalt: &Value) -> EvalResult {
        let mut source = val.clone();
        if !matches!(&source, Value::Str(s) if !s.borrow().is_empty()) {
            let d = match defalt {
                Value::Cons(c) => c.borrow().car.clone(),
                _ => defalt.clone(),
            };
            if matches!(&d, Value::Str(_)) {
                source = d;
            }
        }
        let text = match &source {
            Value::Str(s) => s.borrow().clone(),
            _ => crate::lisp::text::EmacsText::new(),
        };
        let multibyte = crate::lisp::builtins::strfn::string_multibyte_p(self, &source);
        match self.read_from_string_kind(&text, 0, None, multibyte) {
            Ok((form, pos)) => {
                // Only trailing whitespace may follow the form.
                if text
                    .chars()
                    .skip(pos)
                    .any(|c| c != ' ' && c != '\t' && c != '\n')
                {
                    let irs = self.q("invalid-read-syntax");
                    return Err(self.signal_data(
                        irs,
                        vec![Value::string("Trailing garbage following expression")],
                    ));
                }
                Ok(form)
            }
            Err(_) => {
                let eof = self.q("end-of-file");
                Err(self.signal_data(eof, vec![Value::string("End of file during parsing")]))
            }
        }
    }
}

/// Result of a minibuffer read from the front-end.
pub enum MinibufInput {
    /// A completed input line.
    Text(crate::lisp::text::EmacsText),
    /// A single raw key event code (modifier bits included).
    Key(i128),
    /// A non-character event read through `read-event'.
    Event(Value),
    /// A function thrown through the `exit' catch (GNU calls it
    /// after `recursive_edit_1' unwinds — `minibuffer-quit-recursive-
    /// edit' uses this to signal `minibuffer-quit' post-cleanup).
    Call(Value),
}

/// Per-read arguments mirroring GNU `read_minibuf' (minibuf.c).
pub struct MinibufArgs {
    /// `read-from-minibuffer' HISTORY: nil/`t'/symbol/(HISTVAR . HISTPOS).
    pub hist: Value,
    /// DEFAULT-VALUE → specbound `minibuffer-default' and the
    /// empty-input `histstring'.
    pub defalt: Value,
    /// INITIAL-CONTENTS: nil, string, or (STRING . POS).
    pub initial: Value,
    /// KEYMAP override; nil substitutes `minibuffer-local-map'.
    pub keymap: Value,
}

impl Default for MinibufArgs {
    fn default() -> Self {
        MinibufArgs {
            hist: Value::Nil,
            defalt: Value::Nil,
            initial: Value::Nil,
            keymap: Value::Nil,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minibuffer_prompt_property_errors_propagate_after_unwind() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str("(setq minibuffer-prompt-properties '(5 t))")
                    .unwrap();

                let input_called = Rc::new(std::cell::Cell::new(false));
                let input_called_by_reader = input_called.clone();
                interp.minibuf_reader = Some(Rc::new(move |_, _, _| {
                    input_called_by_reader.set(true);
                    Ok(MinibufInput::Text("answer".into()))
                }));

                let mark = interp.specbind_depth();
                let buffer = interp.current_buffer;
                let level = interp.minibuf_level;
                let result = interp.minibuf_read("Prompt: ", MinibufArgs::default());
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal))
                        if signal.is_symbol(sym::WRONG_TYPE_ARGUMENT)
                ));
                assert!(!input_called.get());
                assert_eq!(interp.specbind_depth(), mark);
                assert_eq!(interp.minibuf_level, level);
                assert_eq!(interp.current_buffer, buffer);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn minibuffer_inactive_mode_errors_propagate_after_unwind() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun minibuffer-inactive-mode ()\
                           (signal 'file-error '(\"inactive mode\")))",
                    )
                    .unwrap();
                interp.minibuf_reader =
                    Some(Rc::new(|_, _, _| Ok(MinibufInput::Text("answer".into()))));

                let mark = interp.specbind_depth();
                let buffer = interp.current_buffer;
                let result = interp.minibuf_read("Prompt: ", MinibufArgs::default());
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert_eq!(interp.specbind_depth(), mark);
                assert_eq!(interp.minibuf_level, 0);
                assert_eq!(interp.current_buffer, buffer);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn minibuffer_inactive_mode_runs_between_per_level_and_outer_unbinding() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(setq minibuffer-default \"outer\" minibuffer-history-position 77 history-add-new-input nil)",
                    )
                    .unwrap();
                interp
                    .eval_str(
                        "(defvar qa-minibuf-inactive-observed nil)\
                         (defun minibuffer-inactive-mode ()\
                           (setq qa-minibuf-inactive-observed\
                                 (list minibuffer-default minibuffer-history-position)))",
                    )
                    .unwrap();
                interp.minibuf_reader = Some(Rc::new(|_, _, _| {
                    Ok(MinibufInput::Text("answer".into()))
                }));

                let result = interp.minibuf_read(
                    "Prompt: ",
                    MinibufArgs {
                        defalt: Value::string("inner"),
                        ..MinibufArgs::default()
                    },
                );
                let result = match result {
                    Ok(result) => result,
                    Err(Flow::Signal(signal)) => panic!(
                        "minibuffer read signaled {}",
                        interp.prin1_to_string(&signal.condition)
                    ),
                    Err(flow) => panic!("minibuffer read returned {flow:?}"),
                };
                assert_eq!(result.as_str(), "answer");
                let observed_id = interp.q("qa-minibuf-inactive-observed");
                let observed = interp.symbol_value(observed_id);
                assert_eq!(interp.prin1_to_string(&observed), "(\"inner\" 77)");
                let default_id = interp.q("minibuffer-default");
                let default_value = interp.symbol_value(default_id);
                assert_eq!(
                    interp.prin1_to_string(&default_value),
                    "\"outer\""
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn later_minibuffer_cleanup_error_replaces_exit_hook_throw() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defun qa-minibuf-exit-throw ()\
                           (throw 'qa-minibuf-exit-tag 'exit-hook))\
                         (setq minibuffer-exit-hook '(qa-minibuf-exit-throw))\
                         (defun minibuffer-inactive-mode ()\
                           (signal 'file-error '(\"inactive cleanup\")))",
                    )
                    .unwrap();
                interp.minibuf_reader =
                    Some(Rc::new(|_, _, _| Ok(MinibufInput::Text("answer".into()))));

                let mark = interp.specbind_depth();
                let buffer = interp.current_buffer;
                let result = interp.minibuf_read("Prompt: ", MinibufArgs::default());
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert_eq!(interp.specbind_depth(), mark);
                assert_eq!(interp.minibuf_level, 0);
                assert_eq!(interp.current_buffer, buffer);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn minibuffer_history_add_errors_propagate_after_unwind() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar qa-minibuffer-history nil)\
                         (setq history-add-new-input t)\
                         (defun qa-minibuffer-history-watcher\
                                (_symbol _value operation _where)\
                           (when (eq operation 'set)\
                             (signal 'file-error '(\"history watcher\"))))\
                         (add-variable-watcher 'qa-minibuffer-history\
                                               #'qa-minibuffer-history-watcher)",
                    )
                    .unwrap();
                interp.minibuf_reader =
                    Some(Rc::new(|_, _, _| Ok(MinibufInput::Text("answer".into()))));

                let mark = interp.specbind_depth();
                let buffer = interp.current_buffer;
                let history = interp.q("qa-minibuffer-history");
                let result = interp.minibuf_read(
                    "Prompt: ",
                    MinibufArgs {
                        hist: Value::Sym(history),
                        ..MinibufArgs::default()
                    },
                );
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert_eq!(interp.specbind_depth(), mark);
                assert_eq!(interp.minibuf_level, 0);
                assert_eq!(interp.current_buffer, buffer);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn minibuf_read_unwinds_a_failed_initial_specbind() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                interp
                    .eval_str(
                        "(defvar minibuf-test-watcher-target nil)\
                         (defvar minibuf-test-watcher-fail-on nil)\
                         (defun minibuf-test-watcher (symbol _value operation _where)\
                           (if (and (eq symbol minibuf-test-watcher-target)\
                                    (eq operation minibuf-test-watcher-fail-on))\
                               (signal (if (eq operation 'let) 'file-error 'arith-error)\
                                       '(\"watcher\"))\
                             nil))\
                         (add-variable-watcher 'minibuffer-default #'minibuf-test-watcher)\
                         (add-variable-watcher 'minibuffer-history-position #'minibuf-test-watcher)\
                         (add-variable-watcher 'inhibit-modification-hooks #'minibuf-test-watcher)",
                    )
                    .unwrap();

                let input_called = Rc::new(std::cell::Cell::new(false));
                let input_called_by_reader = input_called.clone();
                interp.minibuf_reader = Some(Rc::new(move |_, _, _| {
                    input_called_by_reader.set(true);
                    Ok(MinibufInput::Text("answer".into()))
                }));

                let read = |interp: &mut Interp, target: &str, operation: &str| {
                    interp
                        .eval_str(&format!(
                            "(setq minibuf-test-watcher-target '{target} \
                                  minibuf-test-watcher-fail-on '{operation})"
                        ))
                        .unwrap();
                    input_called.set(false);
                    let mark = interp.specbind_depth();
                    let level = interp.minibuf_level;
                    let buffer = interp.current_buffer;
                    let result = interp.minibuf_read("Prompt: ", MinibufArgs::default());
                    (
                        result,
                        input_called.get(),
                        mark,
                        level,
                        buffer,
                        interp.specbind_depth(),
                        interp.minibuf_level,
                        interp.current_buffer,
                    )
                };

                // Initial bindings fail before minibuffer entry.
                let (result, called, mark, level, buffer, depth_after, level_after, buffer_after) =
                    read(&mut interp, "minibuffer-default", "let");
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert!(!called);
                assert_eq!(depth_after, mark);
                assert_eq!(level_after, level);
                assert_eq!(buffer_after, buffer);

                // Per-level minibuffer bindings fail after the level is raised.
                let (result, called, mark, level, buffer, depth_after, level_after, buffer_after) =
                    read(&mut interp, "minibuffer-history-position", "let");
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert!(!called);
                assert_eq!(depth_after, mark);
                assert_eq!(level_after, level);
                assert_eq!(buffer_after, buffer);

                // Temporary prompt-install bindings unwind when their watcher fails.
                let (result, called, mark, level, buffer, depth_after, level_after, buffer_after) =
                    read(&mut interp, "inhibit-modification-hooks", "let");
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::FILE_ERROR)
                ));
                assert!(!called);
                assert_eq!(depth_after, mark);
                assert_eq!(level_after, level);
                assert_eq!(buffer_after, buffer);

                // A cleanup watcher error replaces the successful read result.
                let (result, called, mark, level, buffer, depth_after, level_after, buffer_after) =
                    read(&mut interp, "minibuffer-default", "unlet");
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ARITH_ERROR)
                ));
                assert!(called);
                assert_eq!(depth_after, mark);
                assert_eq!(level_after, level);
                assert_eq!(buffer_after, buffer);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn minibuffer_prompt_property_cycle_signals_and_unwinds() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let properties = Value::list(vec![
                    Value::Sym(interp.intern("face")),
                    Value::Sym(interp.intern("bold")),
                ]);
                let Value::Cons(first) = &properties else {
                    unreachable!()
                };
                let Value::Cons(second) = first.borrow().cdr.clone() else {
                    unreachable!()
                };
                second.borrow_mut().cdr = properties.clone();
                let property = interp.intern("minibuffer-prompt-properties");
                interp.set_symbol(property, properties).unwrap();

                let reader_called = Rc::new(std::cell::Cell::new(false));
                let reader_called_from_hook = reader_called.clone();
                interp.minibuf_reader = Some(Rc::new(move |_, _, _| {
                    reader_called_from_hook.set(true);
                    Ok(MinibufInput::Text("answer".into()))
                }));
                let specbind_mark = interp.specbind_depth();
                let current_buffer = interp.current_buffer;
                let minibuf_level = interp.minibuf_level;
                let result = interp.minibuf_read("Prompt: ", MinibufArgs::default());
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::CIRCULAR_LIST)
                ));
                assert!(!reader_called.get());
                assert_eq!(interp.specbind_depth(), specbind_mark);
                assert_eq!(interp.current_buffer, current_buffer);
                assert_eq!(interp.minibuf_level, minibuf_level);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn uninterned_face_named_prompt_property_stays_a_regular_property() {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let face = interp.make_symbol("face");
                let value = Value::Sym(interp.intern("bold"));
                let properties = Value::list(vec![Value::Sym(face), value.clone()]);
                let property = interp.intern("minibuffer-prompt-properties");
                interp.set_symbol(property, properties).unwrap();
                interp.minibuf_reader =
                    Some(Rc::new(|_, _, _| Ok(MinibufInput::Text("answer".into()))));

                interp
                    .minibuf_read("Prompt: ", MinibufArgs::default())
                    .unwrap();
                let minibuffer = interp.get_minibuffer(1);
                let buffer = Value::Buffer(
                    interp
                        .buffers
                        .get(minibuffer)
                        .expect("minibuffer remains live")
                        .clone(),
                );
                let fake_face = crate::buffer::primitives::f_get_text_property(
                    &mut interp,
                    vec![Value::Int(1), Value::Sym(face), buffer.clone()].into(),
                )
                .unwrap();
                let canonical_face_id = interp.intern("face");
                let canonical_face = crate::buffer::primitives::f_get_text_property(
                    &mut interp,
                    vec![Value::Int(1), Value::Sym(canonical_face_id), buffer].into(),
                )
                .unwrap();
                assert!(crate::lisp::builtins::eq_values(&fake_face, &value));
                assert!(canonical_face.is_nil());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
