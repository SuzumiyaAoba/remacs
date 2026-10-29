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

    /// Execute a command by name (M-x dispatch entry point).
    /// The editor's command loop calls this with the command symbol.
    pub fn command_execute(&mut self, cmd: &Value) -> EvalResult {
        self.command_execute_with_keys(cmd, &Value::Nil)
    }

    /// Execute a command while supplying the raw events that triggered it.
    /// Mouse commands need the event object for interactive `e' arguments;
    /// ordinary keyboard dispatch uses `command_execute` above.
    pub(crate) fn command_execute_with_keys(
        &mut self,
        cmd: &Value,
        keys: &Value,
    ) -> EvalResult {
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
                let query = matches!(
                    &dis,
                    Value::Cons(c)
                        if matches!(c.borrow().car, Value::Sym(s) if s == q_id)
                );
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
        let spec = crate::lisp::builtins::data::f_interactive_form(self, vec![cmd.clone()].into())?;
        if !matches!(spec, Value::Cons(_)) {
            return Err(self.wrong_type_mut("commandp", cmd));
        }
        self.interactive_call_depth += 1;
        let result = (|| {
            let events = match keys {
                Value::Vec(events) => events.borrow().clone(),
                _ => Vec::new(),
            };
            let argv = self.eval_interactive_spec_for(&spec, cmd, &events)?;
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
        let items = spec_form.list_to_vec().unwrap_or_default();
        let spec = items.get(1).cloned().unwrap_or(Value::Nil);
        match spec {
            Value::Nil => Ok(Vec::new()),
            Value::Str(s) => {
                // Parse letter codes. A code's prompt is the text after
                // the code up to the next newline (Emacs spec syntax).
                let chars: Vec<crate::lisp::value::EmacsChar> = s.borrow().chars().collect();
                self.read_interactive_arguments(chars, command, events)
            }
            _ => {
                // (interactive (list (read-string ...) ...)) — evaluate.
                // GNU requires the result to be a list (callint.c).
                let v = self.eval(&spec)?;
                match v.list_to_vec() {
                    Ok(argv) => Ok(argv),
                    Err(_) => Err(self.wrong_type_mut("listp", &v)),
                }
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
            Value::Str(text) => Ok(text.borrow().clone()),
            _ => Ok(EmacsText::new()),
        }
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
                let (p, m) = self
                    .current_buffer_ref()
                    .map(|b| {
                        let bb = b.borrow();
                        (bb.point, bb.mark_position())
                    })
                    .unwrap_or((0, None));
                let m = match m {
                    Some(m) => m,
                    None => {
                        return Err(self.error("The mark is not set now, so there is no region"));
                    }
                };
                let (beg, end) = if p <= m { (p, m) } else { (m, p) };
                arguments.values.push(Value::Int(beg as i128 + 1));
                arguments.values.push(Value::Int(end as i128 + 1));
            }
            'd' => {
                let p = self
                    .current_buffer_ref()
                    .map(|b| b.borrow().point)
                    .unwrap_or(0);
                arguments.values.push(Value::Int(p as i128 + 1));
            }
            'm' => {
                let m = self
                    .current_buffer_ref()
                    .and_then(|b| b.borrow().mark_position())
                    .unwrap_or(0);
                arguments.values.push(Value::Int(m as i128 + 1));
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
                    arguments.values.push(prefix_numeric(self, &pa));
                } else {
                    arguments.values.push(pa);
                }
            }
            'n' | 'N' => {
                let pa = self.prefix_arg();
                // `N' uses a supplied prefix; `n' always
                // prompts (GNU callint.c).
                if c == 'N' && !pa.is_nil() {
                    arguments.values.push(prefix_numeric(self, &pa));
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
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rfn), vec![Value::string(prompt)])?);
                }
            }
            'z' | 'Z' => {
                if let Some(v) = self.command_args.first() {
                    arguments.values.push(v.clone());
                } else {
                    let rcs = self.q("read-coding-system");
                    arguments
                        .values
                        .push(self.apply(&Value::Sym(rcs), vec![Value::string(prompt)])?);
                }
            }
            'a' | 'C' | 'S' | 'v' => {
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
                    let f = self.intern(fname);
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
                    let name = self.prin1_to_string(arguments.command);
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
fn prefix_numeric(i: &Interp, v: &Value) -> Value {
    match v {
        Value::Nil => Value::Int(1),
        Value::Int(n) => Value::Int(*n),
        Value::Cons(c) => match &c.borrow().car {
            Value::Int(n) => Value::Int(*n),
            _ => Value::Int(-1),
        },
        Value::Sym(_) => {
            // A bare `-` prefix means -1 (Emacs `prefix-numeric-value`).
            if i.sym_is(v, i.q_soft("-").unwrap_or(SymId::MAX)) {
                Value::Int(-1)
            } else {
                Value::Int(1)
            }
        }
        _ => Value::Int(1),
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
