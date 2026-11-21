//! Command invocation and interactive argument decoding.

use super::Interp;

mod parser;
mod specs;
use crate::lisp::text::{EmacsChar, EmacsText};
use crate::lisp::{
    error::{EvalResult, Flow},
    value::{SymId, Value},
};
use parser::{SpecPart, SpecReader};
pub(crate) use specs::subr_interactive;

impl Interp {
    /// Interactive-spec line input: front-end reader when available,
    /// else batch stdin (GNU `read_minibuf_noninteractive'), else None.
    pub(super) fn spec_line<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        prompt: &T,
    ) -> Result<Option<Value>, Flow> {
        prompt.with_text(|prompt| {
            if self.minibuf_reader.is_some() {
                return self
                    .minibuf_line(prompt)
                    .map(|line| Some(Value::string(line)));
            }
            self.batch_minibuf_line(prompt)
        })
    }

    /// Value of `current-prefix-arg` (nil when unset).
    pub fn prefix_arg(&self) -> Value {
        self.q_soft("current-prefix-arg")
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil)
    }

    /// The host minibuffer reader supplies a line without applying Emacs's
    /// completion keymap. Validate that line only when the default
    /// interactive reader is active. GNU's noninteractive `completing-read'
    /// path and user replacements return their result without this check.
    pub(crate) fn completing_read_requires_manual_validation(&self) -> bool {
        if self.noninteractive || self.minibuf_reader.is_none() {
            return false;
        }
        let (Some(variable), Some(default_function)) = (
            self.q_soft("completing-read-function"),
            self.q_soft("completing-read-default"),
        ) else {
            return false;
        };
        matches!(self.symbol_value(variable), Value::Sym(id) if id == default_function)
    }

    /// Execute a command by name (M-x dispatch entry point).
    /// The editor's command loop calls this with the command symbol.
    pub fn command_execute(&mut self, cmd: &Value) -> EvalResult {
        self.command_execute_with_keys(cmd, &Value::Nil)
    }

    /// Execute a command while supplying the raw events that triggered it.
    /// Mouse commands need the event object for interactive `e' arguments;
    /// ordinary keyboard dispatch uses `command_execute` above.
    pub(crate) fn command_execute_with_keys(&mut self, cmd: &Value, keys: &Value) -> EvalResult {
        // GNU `command-execute' (simple.el): a command symbol whose
        // `disabled' property is non-nil is not run; instead
        // `disabled-command-function' (a hook var naming a function,
        // or a list of functions) handles it.  The `(query ...)'
        // property form is handled by `command-execute--query' — not
        // yet ported; such commands run normally for now.
        if let Value::Sym(id) = cmd {
            let dis_id = self.q("disabled");
            let dis = self.get_prop(*id, dis_id);
            if !dis.is_nil() {
                let q_id = self.q("query");
                let query = match &dis {
                    Value::Cons(c) => {
                        let cell = c.try_borrow().map_err(|_| {
                            self.error(
                                "Cannot inspect a mutably borrowed disabled-command property",
                            )
                        })?;
                        matches!(cell.car, Value::Sym(s) if s == q_id)
                    }
                    _ => false,
                };
                if !query {
                    let dcf = self.q("disabled-command-function");
                    if self.bound_p(dcf) && self.symbol_value(dcf).truthy() {
                        return crate::lisp::builtins::evalfn::call_hook(
                            self,
                            "disabled-command-function",
                        );
                    }
                }
            }
        }
        // Keyboard macros belong to command-execute, not call-interactively.
        if matches!(cmd, Value::Str(_) | Value::Vec(_)) {
            return crate::editor::f_execute_kbd_macro_pub(self, vec![cmd.clone()].into());
        }
        self.call_interactively_with_keys(cmd, keys)
    }

    /// GNU call-interactively resolves the interactive form (and autoloads)
    /// before rejecting a non-command. Direct calls bypass disabled handling.
    pub fn call_interactively(&mut self, cmd: &Value) -> EvalResult {
        self.call_interactively_with_keys(cmd, &Value::Nil)
    }

    pub(crate) fn call_interactively_with_keys(&mut self, cmd: &Value, keys: &Value) -> EvalResult {
        // GNU callint.c restores command identity after argument reading and
        // before the body runs. Recursive minibuffer commands may replace it.
        let saved_commands = [
            "this-command",
            "this-original-command",
            "real-this-command",
            "last-command",
        ]
        .map(|name| {
            let symbol = self.q(name);
            (symbol, self.symbol_value(symbol))
        });
        let spec = crate::lisp::builtins::data::f_interactive_form(self, vec![cmd.clone()].into())?;
        if !matches!(spec, Value::Cons(_)) {
            return Err(self.wrong_type_mut("commandp", cmd));
        }
        self.interactive_call_depth += 1;
        let result = (|| {
            let events = match keys {
                Value::Vec(events) => events
                    .try_borrow()
                    .map(|events| events.clone())
                    .map_err(|_| self.error("Cannot inspect a mutably borrowed command key vector"))?,
                _ => self.command_key_events.clone(),
            };
            let argv = self.eval_interactive_spec_for(&spec, cmd, &events)?;
            for (symbol, value) in saved_commands {
                self.store_internal_value(symbol, value);
            }
            // GNU dispatches through this symbol, preserving its real frame
            // and honoring a function-cell redefinition.
            let mut arguments = Vec::with_capacity(argv.len() + 1);
            arguments.push(cmd.clone());
            arguments.extend(argv);
            let dispatcher = Value::Sym(self.q("funcall-interactively"));
            self.apply(&dispatcher, arguments)
        })();
        self.interactive_call_depth = self.interactive_call_depth.saturating_sub(1);
        result
    }

    /// Evaluate an `(interactive ...)` spec to an argv.
    /// Non-interactive support: `interactive` with no string → no args;
    /// with a string spec we honor the simple codes when input is
    /// pre-supplied in `command_args`.
    pub fn eval_interactive_spec(&mut self, spec_form: &Value) -> Result<Vec<Value>, Flow> {
        self.eval_interactive_spec_for(spec_form, &Value::Nil, &[])
    }

    pub(super) fn eval_interactive_spec_for(
        &mut self,
        spec_form: &Value,
        command: &Value,
        events: &[Value],
    ) -> Result<Vec<Value>, Flow> {
        // callint reads only the second element of INTERACTIVE-FORM.  The
        // form may be dotted or have a circular tail; walking the whole
        // spine here discarded a perfectly usable spec such as
        // `(interactive (list 42) . #1#)'.
        let spec = match spec_form {
            Value::Cons(form) => {
                let tail = form
                    .try_borrow()
                    .map(|form| form.cdr.clone())
                    .map_err(|_| self.error("Cannot inspect a mutably borrowed interactive form"))?;
                match tail {
                    Value::Cons(spec) => spec
                        .try_borrow()
                        .map(|spec| spec.car.clone())
                        .map_err(|_| {
                            self.error("Cannot inspect a mutably borrowed interactive spec")
                        })?,
                    _ => Value::Nil,
                }
            }
            _ => Value::Nil,
        };
        match spec {
            Value::Nil => Ok(Vec::new()),
            Value::Str(s) => {
                // Parse letter codes. A code's prompt is the text after
                // the code up to the next newline (Emacs spec syntax).
                let chars: Vec<crate::lisp::value::EmacsChar> = s
                    .try_borrow()
                    .map(|text| text.chars().collect())
                    .map_err(|_| self.error("Cannot inspect a mutably borrowed interactive string"))?;
                self.read_interactive_arguments(chars, command, events)
            }
            _ => {
                // (interactive (list (read-string ...) ...)) — evaluate.
                // GNU requires the result to be a list (callint.c).
                let v = self.eval(&spec)?;
                crate::lisp::builtins::want_list(self, &v)
            }
        }
    }
    fn read_interactive_arguments(
        &mut self,
        chars: Vec<EmacsChar>,
        command: &Value,
        events: &[Value],
    ) -> Result<Vec<Value>, Flow> {
        let mut reader = SpecReader::new(chars);
        let mut arguments = InteractiveArguments {
            command,
            events,
            values: Vec::new(),
            parameter_event: 0,
        };
        while let Some(part) = reader.next() {
            match part {
                SpecPart::Reserved => return Err(self.error("\u{2018}+\u{2019} is not used in \u{2018}interactive\u{2019} for ordinary commands")),
                SpecPart::Writable => crate::buffer::primitives::check_writable(self)?,
                SpecPart::Ignored => {},
                SpecPart::SelectEventWindow => {
                    self.select_interactive_event_window(events)?;
                }
                SpecPart::ShiftSelection => {
                    let function = Value::Sym(self.q("handle-shift-selection"));
                    self.apply(&function, Vec::new())?;
                }
                SpecPart::Argument { code, prompt } => {
                    // GNU formats even unused/invalid-code prompts first;
                    // this must remain before dispatch and its side effects.
                    let prompt = self.format_interactive_prompt(prompt, &arguments.values)?;
                    self.append_interactive_argument(code, prompt, &mut arguments)?;
                }
            }
        }
        Ok(arguments.values)
    }

    fn format_interactive_prompt(
        &mut self,
        prompt: EmacsText,
        values: &[Value],
    ) -> Result<EmacsText, Flow> {
        let mut argv = vec![Value::string(prompt)];
        argv.extend_from_slice(values);
        match crate::lisp::builtins::strfn::f_format_message(self, argv.into())? {
            Value::Str(text) => text
                .try_borrow()
                .map(|text| (*text).clone())
                .map_err(|_| self.error("Cannot inspect a mutably borrowed interactive prompt")),
            _ => Ok(EmacsText::new()),
        }
    }

    fn select_interactive_event_window(&mut self, events: &[Value]) -> Result<(), Flow> {
        let event = events.iter().find(|event| matches!(event, Value::Cons(_)));
        let Some(Value::Cons(event)) = event else {
            return Ok(());
        };
        let parameters = event
            .try_borrow()
            .map(|event| event.cdr.clone())
            .map_err(|_| self.error("Cannot inspect a mutably borrowed input event"))?;
        let Value::Cons(parameters) = parameters else {
            return Ok(());
        };
        let location = parameters
            .try_borrow()
            .map(|parameters| parameters.car.clone())
            .map_err(|_| self.error("Cannot inspect a mutably borrowed event parameter"))?;
        let Value::Cons(location) = location else {
            return Ok(());
        };
        let window = location
            .try_borrow()
            .map(|location| location.car.clone())
            .map_err(|_| self.error("Cannot inspect a mutably borrowed event location"))?;
        if matches!(window, Value::Window(_)) {
            crate::lisp::builtins::call_canonical_builtin(
                self,
                "select-window",
                vec![window, Value::Nil],
            )?;
        }
        Ok(())
    }

    fn append_interactive_argument(
        &mut self,
        c: EmacsChar,
        prompt: EmacsText,
        arguments: &mut InteractiveArguments<'_>,
    ) -> Result<(), Flow> {
        match c.unicode().unwrap_or('\u{fffd}') {
            // GNU callint.c: `i' supplies an ignored argument,
            // always nil.  The rest of its line is prompt text,
            // already consumed/formatted above (not more codes).
            'i' => arguments.values.push(Value::Nil),
            'r' => {
                // GNU callint 'r' = region-beginning +
                // region-end: no mark set → "The mark is
                // not set now, so there is no region".
                self.append_interactive_region(arguments)?;
            }
            'R' => {
                // GNU's active-region code supplies two nil arguments when
                // `use-region-p' is false; otherwise it supplies the
                // ascending region bounds as markers, unlike `r'.
                let use_region = Value::Sym(self.q("use-region-p"));
                let active = self.apply(&use_region, Vec::new())?;
                if active.is_nil() {
                    arguments.values.extend([Value::Nil, Value::Nil]);
                } else {
                    self.append_interactive_region_markers(arguments)?;
                }
            }
            'd' => {
                let p = match self.current_buffer_ref() {
                    Some(buffer) => buffer
                        .try_borrow()
                        .map(|buffer| buffer.point)
                        .map_err(|_| self.error("Cannot inspect a mutably borrowed current buffer"))?,
                    None => 0,
                };
                arguments.values.push(Value::Int(p as i128 + 1));
            }
            'm' => {
                let mark =
                    crate::lisp::builtins::call_canonical_builtin(self, "mark", vec![Value::Nil])?;
                if mark.is_nil() {
                    return Err(self.error("The mark is not set now"));
                }
                arguments.values.push(mark);
            }
            'p' | 'P' => {
                let pa = self.prefix_arg();
                let numeric = c == 'p';
                if pa.is_nil() {
                    let fb = self.command_args.first().cloned();
                    arguments.values.push(match fb {
                        Some(v) => v,
                        None => {
                            if numeric {
                                Value::Int(1)
                            } else {
                                Value::Nil
                            }
                        }
                    });
                } else if numeric {
                    arguments.values.push(prefix_numeric(self, &pa)?);
                } else {
                    arguments.values.push(pa);
                }
            }
            'n' | 'N' => {
                let pa = self.prefix_arg();
                // `N' uses a supplied prefix; `n' always
                // prompts (GNU callint.c).
                if c == 'N' && !pa.is_nil() {
                    arguments.values.push(prefix_numeric(self, &pa)?);
                } else if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let rn = self.q("read-number");
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rn), vec![Value::string(prompt)])?);
                }
            }
            'b' | 'B' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    // GNU callint.c: 'b' defaults to
                    // other-buffer only when the minibuffer
                    // window is selected; 'B' always uses
                    // other-buffer ('b' => require-match t).
                    let cur = self
                        .current_buffer_ref()
                        .map(|b| Value::Buffer(b.clone()))
                        .unwrap_or(Value::Nil);
                    let def = if c == 'B' || self.selected_window_is_minibuffer() {
                        let ob = self.q("other-buffer");
                        self.apply(&Value::Sym(ob), vec![cur])?
                    } else {
                        cur
                    };
                    let rb = self.q("read-buffer");
                    let mut argv = vec![Value::string(prompt), def];
                    if c == 'b' {
                        argv.push(Value::t());
                    }
                    arguments.values.push(self.apply(&Value::Sym(rb), argv)?);
                }
            }
            's' | 'M' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let rs = self.q("read-string");
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rs), vec![Value::string(prompt)])?);
                }
            }
            'F' | 'f' | 'D' | 'G' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let rfn = self.q("read-file-name");
                    let default_filename = if c == 'D' {
                        let directory = self.q("default-directory");
                        self.symbol_value(directory)
                    } else {
                        Value::Nil
                    };
                    let must_match = if matches!(c.unicode(), Some('f' | 'D')) {
                        Value::Sym(self.q("lambda"))
                    } else {
                        Value::Nil
                    };
                    let initial = if c == 'G' {
                        Value::string("")
                    } else {
                        Value::Nil
                    };
                    let predicate = if c == 'D' {
                        Value::Sym(self.q("file-directory-p"))
                    } else {
                        Value::Nil
                    };
                    arguments.values.push(self.apply(
                        &Value::Sym(rfn),
                        vec![
                            Value::string(prompt),
                            Value::Nil,
                            default_filename,
                            must_match,
                            initial,
                            predicate,
                        ],
                    )?);
                }
            }
            'z' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let rcs = self.q("read-coding-system");
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rcs), vec![Value::string(prompt)])?);
                }
            }
            'Z' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else if self.prefix_arg().is_nil() {
                    arguments.values.push(Value::Nil);
                } else {
                    let rcs = self.q("read-non-nil-coding-system");
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rcs), vec![Value::string(prompt)])?);
                }
            }
            'a' | 'C' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let predicate = match c.unicode() {
                        Some('a') => "fboundp",
                        Some('C') => "commandp",
                        _ => unreachable!(),
                    };
                    arguments
                        .values
                        .push(self.read_interactive_symbol(prompt, predicate)?);
                }
            }
            'v' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    arguments.values.push(
                        crate::lisp::builtins::call_canonical_builtin(
                            self,
                            "read-variable",
                            vec![Value::string(prompt)],
                        )?,
                    );
                }
            }
            'S' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else if let Some(s) = self.spec_line(&prompt)? {
                    arguments
                        .values
                        .push(crate::lisp::builtins::data::f_intern(self, vec![s].into())?);
                } else {
                    arguments.values.push(Value::Nil);
                }
            }
            'k' | 'K' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let args = vec![Value::string(prompt), Value::from_bool(c == 'K')].into();
                    arguments.values.push(if c == 'K' {
                        crate::editor::f_read_key_sequence_vector(self, args)?
                    } else {
                        crate::editor::f_read_key_sequence(self, args)?
                    });
                }
            }
            'x' | 'X' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    // `x' reads a form (read-minibuffer);
                    // `X' reads and evals (eval-minibuffer).
                    let fname = if c == 'x' {
                        "read-minibuffer"
                    } else {
                        "eval-minibuffer"
                    };
                    let f = self.q(fname);
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(f), vec![Value::string(prompt)])?);
                }
            }
            'c' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    arguments.values.push(crate::editor::f_read_char(
                        self,
                        vec![Value::string(prompt)].into(),
                    )?);
                }
            }
            'e' => {
                if let Some(value) = self.command_args.first() {
                    arguments.values.push(value.clone());
                } else if let Some(offset) = arguments.events[arguments.parameter_event..]
                    .iter()
                    .position(|event| matches!(event, Value::Cons(_)))
                {
                    arguments.parameter_event += offset;
                    arguments
                        .values
                        .push(arguments.events[arguments.parameter_event].clone());
                    arguments.parameter_event += 1;
                } else {
                    let name = match arguments.command {
                        Value::Sym(symbol) => self.symbol_name(*symbol).into(),
                        _ => "command".into(),
                    };
                    return Err(
                        self.error_q_text("%s must be bound to an event with parameters", &[name])
                    );
                }
            }
            // GNU callint.c: the up-event a `k'/`K' read
            // stored after a mouse down-event, else nil —
            // our key reads never split down/up pairs.
            'U' => arguments.values.push(Value::Nil),
            _ => {
                // GNU callint.c `default:': any unknown
                // letter (including `+') signals —
                // silently skipping it makes bogus specs
                // like "~" return nil.
                let letter = (c).code() as i128;
                return Err(self.error_q::<str>(&format!(
                                "Invalid control letter \u{2018}{}\u{2019} (#o{:03o}, #x{:04x}) in interactive calling string",
                                c.to_string(), letter, letter
                            ), &[]));
            }
        }
        Ok(())
    }

    fn append_interactive_region(
        &mut self,
        arguments: &mut InteractiveArguments<'_>,
    ) -> Result<(), Flow> {
        let mark = crate::lisp::builtins::call_canonical_builtin(self, "mark", vec![Value::Nil])?;
        let mark = match mark {
            Value::Nil => {
                return Err(self.error("The mark is not set now, so there is no region"));
            }
            Value::Int(mark) => mark,
            other => return Err(self.wrong_type_mut("integer-or-marker-p", &other)),
        };
        let point = match self.current_buffer_ref() {
            Some(buffer) => buffer
                .try_borrow()
                .map(|buffer| buffer.point as i128 + 1)
                .map_err(|_| self.error("Cannot inspect a mutably borrowed current buffer"))?,
            None => 1,
        };
        let (beginning, end) = if point <= mark {
            (point, mark)
        } else {
            (mark, point)
        };
        arguments.values.push(Value::Int(beginning));
        arguments.values.push(Value::Int(end));
        Ok(())
    }

    fn append_interactive_region_markers(
        &mut self,
        arguments: &mut InteractiveArguments<'_>,
    ) -> Result<(), Flow> {
        let mark = crate::lisp::builtins::call_canonical_builtin(self, "mark-marker", vec![])?;
        let point = crate::lisp::builtins::call_canonical_builtin(self, "point-marker", vec![])?;
        let mark_position = crate::lisp::builtins::call_canonical_builtin(
            self,
            "marker-position",
            vec![mark.clone()],
        )?;
        let point_position = crate::lisp::builtins::call_canonical_builtin(
            self,
            "marker-position",
            vec![point.clone()],
        )?;
        let (mark_position, point_position) = match (mark_position, point_position) {
            (Value::Int(mark), Value::Int(point)) => (mark, point),
            _ => return Err(self.error("The mark is not set now, so there is no region")),
        };
        if mark_position <= point_position {
            arguments.values.extend([mark, point]);
        } else {
            arguments.values.extend([point, mark]);
        }
        Ok(())
    }

    fn read_interactive_symbol(
        &mut self,
        prompt: EmacsText,
        predicate: &str,
    ) -> Result<Value, Flow> {
        let validate = self.completing_read_requires_manual_validation();
        let obarray = self.q("obarray");
        let collection = self.symbol_value(obarray);
        let predicate_symbol = Value::Sym(self.q(predicate));
        let completing_read = Value::Sym(self.q("completing-read"));
        let input = self.apply(
            &completing_read,
            vec![
                Value::string(prompt),
                collection,
                predicate_symbol.clone(),
                Value::t(),
                Value::Nil,
                Value::Nil,
                Value::Nil,
                Value::Nil,
            ],
        )?;
        let symbol = crate::lisp::builtins::data::f_intern(self, vec![input].into())?;
        if validate
            && self
            .apply(&predicate_symbol, vec![symbol.clone()])?
            .is_nil()
        {
            let condition = self.q("user-error");
            return Err(self.signal_data(condition, vec![Value::string("No match")]));
        }
        Ok(symbol)
    }
}

/// Return `(interactive SPEC)' for a primitive.  GNU primitive specs are
/// stored as strings; a leading `(' means the string is an expression to
/// read and evaluate rather than callint letter codes.
pub(crate) fn subr_interactive_form(
    i: &mut Interp,
    subr: &'static crate::lisp::value::Subr,
) -> Option<Value> {
    if let Some(form) = crate::lisp::builtins::comp::native_interactive_form(subr) {
        return if form.is_nil() { None } else { Some(form) };
    }
    let spec = subr_interactive(subr.name)?;
    // "\u{1}nil" is a sentinel for GNU's `(interactive)' (no argument),
    // which `interactive-form' renders as `(interactive nil)'.
    let spec = match spec {
        "\u{1}nil" => Value::Nil,
        s if s.trim_start().starts_with('(') => i
            .read_from_string(s, 0)
            .ok()
            .map(|(form, _)| form)
            .unwrap_or_else(|| Value::string(s)),
        s => Value::string(s),
    };
    Some(Value::list(vec![Value::Sym(i.q("interactive")), spec]))
}

/// Numeric value of a prefix-arg value: (4)→4, (16)→16, (-)→-1, nil→1.
fn prefix_numeric(i: &Interp, v: &Value) -> Result<Value, Flow> {
    match v {
        Value::Nil => Ok(Value::Int(1)),
        Value::Int(n) => Ok(Value::Int(*n)),
        Value::Cons(c) => {
            let car = c
                .try_borrow()
                .map(|cell| cell.car.clone())
                .map_err(|_| i.error("Cannot inspect a mutably borrowed prefix argument"))?;
            match car {
                Value::Int(n) => Ok(Value::Int(n)),
                _ => Ok(Value::Int(-1)),
            }
        }
        Value::Sym(_) => {
            // A bare `-` prefix means -1 (Emacs `prefix-numeric-value`).
            if i.sym_is(v, i.q_soft("-").unwrap_or(SymId::MAX)) {
                Ok(Value::Int(-1))
            } else {
                Ok(Value::Int(1))
            }
        }
        _ => Ok(Value::Int(1)),
    }
}

/// Arguments already produced and the command's parameter-event cursor.
/// Prefix/command_args fallbacks remain observable through the interpreter.
struct InteractiveArguments<'a> {
    command: &'a Value,
    events: &'a [Value],
    values: Vec<Value>,
    parameter_event: usize,
}

#[cfg(test)]
mod borrowed_interactive_input_tests {
    use super::*;
    use crate::lisp::obarray::{Obarray, sym};

    #[test]
    fn eval_interactive_spec_reports_borrowed_forms_and_strings() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();

        let form = Value::list(vec![Value::Sym(sym::INTERACTIVE), Value::Nil]);
        let Value::Cons(form_cell) = &form else {
            unreachable!();
        };
        let held_form = form_cell.borrow_mut();
        let result = interp.eval_interactive_spec(&form);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_form);
        assert!(matches!(interp.eval_interactive_spec(&form), Ok(values) if values.is_empty()));

        let form = Value::list(vec![Value::Sym(sym::INTERACTIVE), Value::Nil]);
        let Value::Cons(form_cell) = &form else {
            unreachable!();
        };
        let Value::Cons(spec_cell) = form_cell.borrow().cdr.clone() else {
            unreachable!();
        };
        let held_spec = spec_cell.borrow_mut();
        let result = interp.eval_interactive_spec(&form);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_spec);
        assert!(matches!(interp.eval_interactive_spec(&form), Ok(values) if values.is_empty()));

        let string = Value::string_ref("");
        let string_value = Value::Str(string.clone());
        let form = Value::list(vec![Value::Sym(sym::INTERACTIVE), string_value.clone()]);
        let text = &string;
        let held_string = text.borrow_mut();
        let result = interp.eval_interactive_spec(&form);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_string);
        assert!(matches!(interp.eval_interactive_spec(&form), Ok(values) if values.is_empty()));
    }

    #[test]
    fn prefix_numeric_reports_a_mutably_borrowed_list_without_panicking() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let prefix = Value::list(vec![Value::Int(4)]);
        let Value::Cons(cell) = &prefix else {
            unreachable!();
        };
        let held = cell.borrow_mut();
        let result = prefix_numeric(&interp, &prefix);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held);
        assert!(matches!(prefix_numeric(&interp, &prefix), Ok(Value::Int(4))));
    }

    #[test]
    fn event_window_selection_reports_borrowed_event_cells() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let location = Value::cons(Value::Nil, Value::Nil);
        let parameters = Value::cons(location.clone(), Value::Nil);
        let event = Value::cons(Value::Nil, parameters.clone());
        let events = [event.clone()];

        let Value::Cons(event_cell) = &event else {
            unreachable!();
        };
        let held_event = event_cell.borrow_mut();
        assert!(matches!(
            interp.select_interactive_event_window(&events),
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_event);

        let Value::Cons(parameter_cell) = &parameters else {
            unreachable!();
        };
        let held_parameters = parameter_cell.borrow_mut();
        assert!(matches!(
            interp.select_interactive_event_window(&events),
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_parameters);

        let Value::Cons(location_cell) = &location else {
            unreachable!();
        };
        let held_location = location_cell.borrow_mut();
        assert!(matches!(
            interp.select_interactive_event_window(&events),
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
        drop(held_location);
        assert!(interp.select_interactive_event_window(&events).is_ok());
    }

    #[test]
    fn command_dispatch_reports_a_borrowed_key_vector() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let command = interp.intern("qa-borrowed-interactive-key-command");
        let interactive = Value::list(vec![Value::Sym(sym::INTERACTIVE), Value::Nil]);
        let body = [interactive, Value::Int(1)];
        let lambda = interp
            .parse_lambda(&Value::Nil, &body, Some(command))
            .unwrap();
        interp.fset(command, Value::Lambda(std::rc::Rc::new(lambda)));
        let command = Value::Sym(command);
        let keys = Value::vector(vec![Value::Int(1)]);
        let Value::Vec(events) = &keys else {
            unreachable!();
        };
        let held_events = events.borrow_mut();

        let result = interp.call_interactively_with_keys(&command, &keys);

        drop(held_events);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
    }

    #[test]
    fn disabled_query_inspection_reports_a_borrowed_property_cell() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let command = interp.intern("qa-borrowed-disabled-query-command");
        let disabled = interp.intern("disabled");
        let query = interp.intern("query");
        let disabled_value = Value::cons(Value::Sym(query), Value::Nil);
        interp.put_prop(command, disabled, disabled_value.clone());
        let Value::Cons(cell) = &disabled_value else {
            unreachable!();
        };
        let held = cell.borrow_mut();

        let result = interp.command_execute_with_keys(&Value::Sym(command), &Value::Nil);

        drop(held);
        assert!(matches!(
            result,
            Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
        ));
    }

    #[test]
    fn interactive_symbol_validation_keeps_user_error_identity_after_unintern() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new_with_noninteractive(false);
                interp.minibuf_reader = Some(std::rc::Rc::new(|_, _, _| {
                    Ok(crate::lisp::eval::MinibufInput::Text(
                        "not-a-command".into(),
                    ))
                }));
                let user_error = interp.q("user-error");
                assert!(interp.completing_read_requires_manual_validation());
                assert!(matches!(
                    interp.read_interactive_symbol("Command: ".into(), "commandp"),
                    Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::USER_ERROR)
                ));
                assert!(interp.obarray.unintern_symbol(user_error));

                let result = interp.read_interactive_symbol("Command: ".into(), "commandp");

                assert!(
                    matches!(
                        result,
                        Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::USER_ERROR)
                    ),
                    "unexpected flow after unintern: {result:?}"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn expression_argument_reader_keeps_its_function_after_unintern() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new_with_noninteractive(false);
                interp.minibuf_reader = Some(std::rc::Rc::new(|_, _, _| {
                    Ok(crate::lisp::eval::MinibufInput::Text("42".into()))
                }));
                for (code, name) in [('x', "read-minibuffer"), ('X', "eval-minibuffer")] {
                    let function = interp.q(name);
                    assert!(interp.obarray.unintern_symbol(function));
                    let result = interp.read_interactive_arguments(
                        vec![crate::lisp::value::EmacsChar::from(code)],
                        &Value::Nil,
                        &[],
                    );
                    assert!(
                        matches!(&result, Ok(values) if values.len() == 1),
                        "unexpected interactive argument result for {name}: {result:?}"
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
