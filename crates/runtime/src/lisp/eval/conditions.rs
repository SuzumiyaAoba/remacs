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
use rustc_hash::FxHashSet;
use std::rc::Rc;

impl Interp {
    fn capture_signal_backtrace(&self) {
        let frames = self.backtrace_snapshot();
        if let Ok(mut last) = self.last_error_stack.try_borrow_mut() {
            *last = frames;
        }
    }

    /// `(error "MSG" DATA)' — GNU signal2(Qerror, build_string(MSG),
    /// DATA): the object travels as error data, NOT concatenated into
    /// the message string.
    pub fn error_obj(&self, msg: &str, v: &Value) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg), v.clone()])
    }

    // ---------- errors ----------

    /// `(signal sym (data...))` where data is already a list.
    pub fn signal(&self, sym_id: SymId, data: Value) -> Flow {
        self.capture_signal_backtrace();
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
        self.capture_signal_backtrace();
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
            return self.condition_list_contains(conditions, s);
        }
        // A condition-name list is flat: nested lists are not recursively
        // groups (e.g. `((error))' does not match `error' in GNU).  The
        // outer list may still be circular, so stop on a repeated cons
        // instead of using `each_car' (which deliberately has no cycle
        // check).
        let mut list = handlers.clone();
        let mut seen_handlers = FxHashSet::default();
        loop {
            let Value::Cons(cell) = list else { break };
            if !seen_handlers.insert(Rc::as_ptr(&cell) as usize) {
                break;
            }
            let (handler, next) = match cell.try_borrow() {
                Ok(cell) => (cell.car.clone(), cell.cdr.clone()),
                Err(_) => return false,
            };
            match self.sym_id(&handler) {
                Some(s) if s == sym::T => return true,
                // Handler names may be positioned symbols (records).
                Some(s) if self.condition_list_contains(conditions, s) => return true,
                _ => {}
            }
            list = next;
        }
        false
    }

    fn condition_list_contains(&self, conditions: &Value, name: SymId) -> bool {
        let mut list = conditions.clone();
        let mut seen = FxHashSet::default();
        loop {
            let Value::Cons(cell) = list else {
                return false;
            };
            if !seen.insert(Rc::as_ptr(&cell) as usize) {
                return false;
            }
            let (condition, next) = match cell.try_borrow() {
                Ok(cell) => (cell.car.clone(), cell.cdr.clone()),
                Err(_) => return false,
            };
            if self.sym_id(&condition) == Some(name) {
                return true;
            }
            list = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn circular_condition_and_handler_lists_terminate() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::vacant();
                interp.obarray = Obarray::new();

                let conditions = Value::cons(Value::Sym(sym::ERROR), Value::Nil);
                if let Value::Cons(cell) = &conditions {
                    cell.borrow_mut().cdr = conditions.clone();
                }
                let handlers = Value::list(vec![Value::Sym(sym::ERROR)]);
                assert!(interp.conditions_match(&conditions, &handlers));

                let nonmatching_conditions = Value::cons(Value::Sym(sym::FILE_ERROR), Value::Nil);
                if let Value::Cons(cell) = &nonmatching_conditions {
                    cell.borrow_mut().cdr = nonmatching_conditions.clone();
                }
                assert!(!interp.conditions_match(&nonmatching_conditions, &handlers));

                let circular_handlers = Value::cons(Value::Sym(sym::ERROR), Value::Nil);
                if let Value::Cons(cell) = &circular_handlers {
                    cell.borrow_mut().cdr = circular_handlers.clone();
                }
                let normal_conditions = Value::list(vec![Value::Sym(sym::ERROR)]);
                assert!(interp.conditions_match(&normal_conditions, &circular_handlers));

                let nested_handler = Value::cons(Value::Nil, Value::Nil);
                if let Value::Cons(cell) = &nested_handler {
                    cell.borrow_mut().car = nested_handler.clone();
                }
                assert!(!interp.conditions_match(&normal_conditions, &nested_handler));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn mutably_borrowed_condition_and_handler_lists_do_not_panic() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let conditions = Value::list(vec![Value::Sym(sym::ERROR)]);
        let handlers = Value::list(vec![Value::Sym(sym::ERROR)]);

        let Value::Cons(handler_cell) = &handlers else {
            unreachable!();
        };
        let held_handler = handler_cell.borrow_mut();
        assert!(!interp.conditions_match(&conditions, &handlers));
        drop(held_handler);
        assert!(interp.conditions_match(&conditions, &handlers));

        let Value::Cons(condition_cell) = &conditions else {
            unreachable!();
        };
        let held_condition = condition_cell.borrow_mut();
        assert!(!interp.conditions_match(&conditions, &handlers));
        drop(held_condition);
        assert!(interp.conditions_match(&conditions, &handlers));
    }

    #[test]
    fn condition_handlers_do_not_flatten_nested_condition_names() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let result = interp
                    .eval_str(
                        "(let ((ran nil))\
                         (condition-case outer\
                             (condition-case nil\
                                 (progn (setq ran t) (signal 'error '(body)))\
                               (((error)) 'handled))\
                           (error (list (car outer) (cdr outer) ran))))",
                    )
                    .unwrap();
                assert_eq!(interp.prin1_to_string(&result), "(error (body) t)");

                let nested_handler_bind = interp
                    .eval_str(
                        "(let ((ran nil))\
                         (condition-case outer\
                             (handler-bind ((((error)) (lambda (_) (setq ran t))))\
                               (signal 'error '(body)))\
                           (error (list (car outer) (cdr outer) ran))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&nested_handler_bind),
                    "(error (body) nil)"
                );

                let flat_handler_bind = interp
                    .eval_str(
                        "(let ((ran nil))\
                         (condition-case outer\
                             (handler-bind ((error (lambda (_) (setq ran t))))\
                               (signal 'error '(body)))\
                           (error (list (car outer) (cdr outer) ran))))",
                    )
                    .unwrap();
                assert_eq!(
                    interp.prin1_to_string(&flat_handler_bind),
                    "(error (body) t)"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn signal_construction_skips_a_mutably_borrowed_backtrace_slot() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let borrowed = interp.last_error_stack.borrow_mut();
        let result = catch_unwind(AssertUnwindSafe(|| {
            interp.signal_data(sym::ERROR, vec![Value::string("borrowed backtrace")])
        }));
        drop(borrowed);

        assert!(result.is_ok(), "signal creation must not panic on a borrow");
        assert!(matches!(
            result.unwrap(),
            Flow::Signal(ref signal) if signal.is_symbol(sym::ERROR)
        ));
    }
}
