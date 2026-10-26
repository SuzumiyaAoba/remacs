//! Signal construction, diagnostics, and condition matching.
//!
//! Delivery and unwinding live in `control`; this module constructs
//! conditions and preserves their matching-list identity before cleanup.

use super::Interp;
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};

impl Interp {
    /// `(error "MSG" DATA)' — GNU signal2(Qerror, build_string(MSG),
    /// DATA): the object travels as error data, NOT concatenated into
    /// the message string.
    pub fn error_obj(&self, msg: &str, v: &Value) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg), v.clone()])
    }

    // ---------- errors ----------

    /// `(signal sym (data...))` where data is already a list.
    pub fn signal(&self, sym_id: SymId, data: Value) -> Flow {
        *self.last_error_stack.borrow_mut() = self.backtrace_snapshot();
        if std::env::var_os("QUITDBG").is_some() && sym_id == sym::QUIT {
            let names: Vec<crate::lisp::text::EmacsText> = self
                .lisp_stack
                .iter()
                .rev()
                .take(24)
                .map(|(f, _)| self.prin1_to_string(f))
                .collect();
            eprintln!("SIGQUIT stack={names:?}");
            eprintln!("{}", std::backtrace::Backtrace::capture());
        }
        self.signal_condition(Value::cons(Value::Sym(sym_id), data))
    }

    /// `(signal sym data-list-from-vec)`.
    pub fn signal_data(&self, sym_id: SymId, data: Vec<Value>) -> Flow {
        *self.last_error_stack.borrow_mut() = self.backtrace_snapshot();
        if std::env::var_os("QUITDBG").is_some() && sym_id == sym::QUIT {
            let names: Vec<crate::lisp::text::EmacsText> = self
                .lisp_stack
                .iter()
                .rev()
                .take(24)
                .map(|(f, _)| self.prin1_to_string(f))
                .collect();
            eprintln!("SIGQUIT(stack)={names:?}");
            eprintln!("{}", std::backtrace::Backtrace::capture());
        }
        if let Some(filter) = (self.dbg_flags & 0x800000 != 0)
            .then(|| std::env::var("REMACS_BT_ERR").unwrap_or_default())
        {
            let rendered = self
                .prin1_to_string(&Value::list(data.clone()))
                .chars()
                .take(160)
                .collect::<crate::lisp::text::EmacsText>();
            let shown = format!("{} {}", self.symbol_name(sym_id), rendered);
            if filter == "1" || shown.contains(&filter) {
                eprintln!("=== SIGNAL {shown} ===");
                for (f, a) in self.lisp_stack.iter().rev().take(30).rev() {
                    eprintln!(
                        "  {} {}",
                        self.princ_to_string(f)
                            .chars()
                            .take(80)
                            .collect::<crate::lisp::text::EmacsText>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(100)
                            .collect::<crate::lisp::text::EmacsText>()
                    );
                }
                if self.lisp_stack.is_empty() {
                    eprintln!("{}", std::backtrace::Backtrace::capture());
                }
            }
        }
        self.signal_condition(Value::cons(Value::Sym(sym_id), Value::list(data)))
    }

    /// Retain the matching-list identity before cleanup. Signal delivery
    /// refreshes it after the early Edebug hook, when that hook is active.
    pub(crate) fn signal_condition(&self, condition: Value) -> Flow {
        let mut signal = crate::lisp::error::Signal::from_condition(condition);
        signal.conditions = Some(self.signal_conditions(&signal.symbol()));
        Flow::Signal(signal)
    }

    /// `(error "fmt" args...)` — signals `error` with a formatted message.
    pub fn error(&self, msg: impl Into<crate::lisp::text::EmacsText>) -> Flow {
        let msg: crate::lisp::text::EmacsText = msg.into();
        self.signal_data(sym::ERROR, vec![Value::string(msg)])
    }

    /// GNU C `error (FMT, ...)': doprnt translates FMT's `like this'
    /// quotes per `text-quoting-style' — never the `%s' arguments.
    pub fn error_q<T: crate::lisp::text::TextSource + ?Sized>(
        &self,
        fmt: &str,
        args: &[&T],
    ) -> Flow {
        let args: Vec<crate::lisp::text::EmacsText> =
            args.iter().map(|s| s.with_text(|s| s.to_owned())).collect();
        self.error_q_text(fmt, &args)
    }

    pub(crate) fn error_q_text(&self, fmt: &str, args: &[crate::lisp::text::EmacsText]) -> Flow {
        let fmt = crate::lisp::builtins::evalfn::translate_message_quotes(self, fmt);
        let mut args = args.iter();
        let mut out = crate::lisp::text::EmacsText::with_capacity(fmt.len());
        let mut rest = fmt.as_str();
        while let Some(p) = rest.find("%s") {
            out.push_str(&rest[..p]);
            if let Some(argument) = args.next() {
                out.push_str(argument);
            }
            rest = &rest[p + 2..];
        }
        out.push_str(rest);
        self.error(out)
    }

    /// `wrong-type-argument` signal: pred, value.
    pub fn wrong_type_mut(&mut self, pred: &str, val: &Value) -> Flow {
        if std::env::var_os("REMACS_TRACE_WT").is_some() {
            let top: Vec<String> = self
                .lisp_stack
                .iter()
                .rev()
                .take(14)
                .map(|(f, a)| {
                    format!(
                        "{}({})",
                        self.princ_to_string(f)
                            .chars()
                            .take(60)
                            .collect::<crate::lisp::text::EmacsText>(),
                        self.princ_to_string(&Value::list(a.as_slice().to_vec()))
                            .chars()
                            .take(80)
                            .collect::<crate::lisp::text::EmacsText>()
                    )
                })
                .collect();
            eprintln!("=== wt {pred} on {:?}; top: {}", val, top.join(" <- "));
        }
        let pred_id = self.q(pred);
        self.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred_id), val.clone()],
        )
    }

    /// `wrong-number-of-arguments' — Emacs signals `(FUN ARGC)'.
    pub fn wrong_number_of_args(&self, fun: &Value, argc: i128) -> Flow {
        self.signal_data(
            sym::WRONG_NUMBER_OF_ARGUMENTS,
            vec![fun.clone(), Value::Int(argc)],
        )
    }

    /// Does signal `sig` match any condition name in `handlers`?
    /// `handlers` is a list of condition symbols (or t).
    pub fn signal_matches(&self, sig: &Value, handlers: &Value) -> bool {
        self.conditions_match(&self.signal_conditions(sig), handlers)
    }

    pub(crate) fn signal_conditions(&self, sig: &Value) -> Value {
        self.sym_id(sig)
            .map(|symbol| self.get_prop(symbol, self.q_soft("error-conditions").unwrap_or(0)))
            .unwrap_or(Value::Nil)
    }

    pub(crate) fn conditions_match(&self, conditions: &Value, handlers: &Value) -> bool {
        // A handler's car is either a single condition name or a list
        // of names; `t` matches everything, `nil` (debug) nothing.
        if let Some(s) = self.sym_id(handlers) {
            if s == sym::NIL {
                return false;
            }
            if s == sym::T {
                return true;
            }
            return self.conditions_match(conditions, &Value::list(vec![handlers.clone()]));
        }
        let mut found = false;
        handlers.each_car(|h| {
            if found {
                return;
            }
            match h {
                Value::Sym(s) if *s == sym::T => found = true,
                _ if self.sym_id(h).is_some() => {
                    // Match the names on the cached error-conditions list.
                    // Handler names may be positioned symbols (records)
                    // when `symbols-with-pos-enabled' is on.
                    let cond = self.sym_id(h).unwrap();
                    conditions.each_car(|c| {
                        if self.sym_id(c) == Some(cond) {
                            found = true;
                        }
                    });
                }
                Value::Cons(_) => {
                    // (cond ...) group? Emacs allows a list of symbols too.
                    h.each_car(|c| {
                        if self.conditions_match(conditions, c) {
                            found = true;
                        }
                    });
                }
                _ => {}
            }
        });
        found
    }
}
