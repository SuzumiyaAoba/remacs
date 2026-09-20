//! The evaluator: `Interp` state, `eval`, `apply`, and special forms.
//!
//! Scoping: elisp is dynamically scoped by default. Dynamic `let` is
//! implemented exactly like Emacs's specbind: save the symbol's current
//! value cell, overwrite, restore on unwind — so lookup stays O(1).
//! When `lexical-binding` is on, `let` binds in a captured lexical env
//! chain instead (unless the symbol is `defvar`'d special).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::error::{EvalResult, Flow};
use super::obarray::Obarray;
use super::obarray::sym;
use super::reader::Reader;
use super::value::{Arity, Lambda, SymId, Value};

/// A saved dynamic binding (specbind entry).
struct SpecBind {
    sym: SymId,
    /// `Some(buf)` = binding is buffer-local in `buf`; `None` = global cell.
    buf: Option<usize>,
    /// Previous value (`None` = no local binding existed).
    old: Option<Value>,
}

/// A lexical environment frame (chained).
pub struct LexFrame {
    pub vars: RefCell<HashMap<SymId, Value>>,
    pub parent: Option<Rc<LexFrame>>,
}

pub type LexEnv = Option<Rc<LexFrame>>;

pub fn lexenv_lookup(mut env: &LexEnv, sym: SymId) -> Option<Value> {
    while let Some(frame) = env {
        if let Some(v) = frame.vars.borrow().get(&sym) {
            return Some(v.clone());
        }
        env = &frame.parent;
    }
    None
}

/// Where output from `princ`/`print`/`message` goes. The editor installs a
/// real sink; tests use the buffer.
pub enum OutputSink {
    /// Accumulate into a string (tests, `with-output-to-string`).
    Buffer(Rc<RefCell<String>>),
    /// Call a Lisp function with each printed string.
    Function(Value),
    /// Write to process stdout (batch mode).
    Stdout,
}

/// The interpreter: all global Lisp state. Lives on the eval thread.
pub struct Interp {
    pub obarray: Obarray,
    specbind: Vec<SpecBind>,
    /// Current lexical environment (non-nil only under lexical-binding).
    pub lexenv: LexEnv,
    /// The `lexical-binding` default (buffer-local in Emacs; we keep a
    /// default plus per-buffer locals).
    pub buffers: crate::buffer::BufferSet,
    pub current_buffer: usize,
    /// Symbols provided via `provide` (the `features` variable).
    pub features: Vec<SymId>,
    /// Output destination for princ/print/message.
    pub output: Option<OutputSink>,
    /// Last thing shown in the echo area (tests inspect this).
    pub echo_message: String,
    /// Read position for `read` from a buffer/stream — kept simple.
    pub max_lisp_eval_depth: usize,
    pub eval_depth: usize,
    /// Depth of explicit `(eval ...)' calls — lambdas built inside get a
    /// `nil' environment marker like Emacs.
    pub explicit_eval_depth: usize,
    /// C-g sets this; eval checks it between forms.
    pub quit_flag: bool,
    /// Values pushed by `throw` for debugging — not needed.
    pub catch_tags: Vec<Value>,
    /// True while evaluating a function call's arguments.
    pub undo_list: Vec<crate::buffer::UndoEntry>,
    /// `inhibit-read-only` dynamic override.
    pub standard_output_sym: SymId,
    /// Command-loop state used by `interactive` specs.
    pub command_args: Vec<Value>,
    /// The symbol currently being defined by `defun` for recursion.
    pub defining_symbol: Option<SymId>,
    /// History list for `M-x` etc.
    pub read_expression_history: Value,
    /// Eval depth counters for `recursive-edit`.
    pub recursion_depth: usize,
    /// `overriding-terminal-local-map` equivalent — keymap override.
    pub executing_kbd_macro: bool,
    /// Records most-recent error for `error-message-string` debugging.
    pub last_error: Option<(Value, Value)>,
    /// Accumulator while `with-output-to-string` captures output.
    pub output_buffer: String,
    /// When true, `write_output` appends to `output_buffer`.
    pub capture_output: bool,
    /// Registers from the last search (`match-data`).
    pub match_data: Option<MatchData>,
    /// True in `--batch`: `princ` goes to stdout, `message` to stderr.
    pub noninteractive: bool,
    /// All frames (the first is the initial tty frame).
    pub frames: Vec<crate::editor::FrameRef>,
    /// The selected frame.
    pub selected_frame: Option<crate::editor::FrameRef>,
    /// Set by `kill-emacs` to exit the command loop.
    pub quit_editor: bool,
    /// Interactive input hook installed by the terminal front-end.
    /// Called as (interp, prompt, single_key) -> MinibufInput.
    /// `single_key` reads one event (y-or-n-p, read-char).
    pub minibuf_reader:
        Option<std::rc::Rc<dyn Fn(&mut Interp, &str, bool) -> Result<MinibufInput, Flow>>>,
    /// Nesting depth of active minibuffer reads (`minibuffer-depth').
    pub minibuf_level: i32,
}

/// Result of a minibuffer read from the front-end.
pub enum MinibufInput {
    /// A completed input line.
    Text(String),
    /// A single raw key event code (modifier bits included).
    Key(i128),
}

impl Interp {
    pub fn new() -> Interp {
        let mut obarray = Obarray::new();
        let standard_output_sym = obarray.intern("standard-output");
        let emacs_sym = obarray.intern("emacs");
        let mut interp = Interp {
            obarray,
            specbind: Vec::new(),
            lexenv: None,
            buffers: crate::buffer::BufferSet::new(),
            current_buffer: 0,
            features: vec![emacs_sym],
            output: None,
            echo_message: String::new(),
            max_lisp_eval_depth: 1600,
            eval_depth: 0,
            explicit_eval_depth: 0,
            quit_flag: false,
            catch_tags: Vec::new(),
            undo_list: Vec::new(),
            standard_output_sym,
            command_args: Vec::new(),
            defining_symbol: None,
            read_expression_history: Value::Nil,
            recursion_depth: 0,
            executing_kbd_macro: false,
            last_error: None,
            output_buffer: String::new(),
            capture_output: false,
            match_data: None,
            noninteractive: false,
            frames: Vec::new(),
            selected_frame: None,
            quit_editor: false,
            minibuf_reader: None,
            minibuf_level: 0,
        };
        crate::lisp::builtins::install(&mut interp);
        crate::buffer::install_primitives(&mut interp);
        interp.define_error_conditions();
        interp.define_special_variables();
        // The initial buffers every Emacs session has, in GNU's
        // buffer-list order: (scratch Minibuf-0 Messages load
        // Warnings). New buffers append at the end of the order.
        let scratch = interp.buffers.create_exact("*scratch*");
        interp.current_buffer = scratch;
        interp.buffers.create_exact(" *Minibuf-0*");
        interp.buffers.create_exact("*Messages*");
        interp.buffers.create_exact(" *load*");
        interp.buffers.create_exact("*Warnings*");
        crate::editor::install_primitives(&mut interp);
        // Load the Lisp prelude (subr.el subset). Errors here indicate a
        // broken prelude, but don't abort startup.
        let _ = interp.eval_str(crate::lisp::prelude::PRELUDE);
        interp
    }

    // ---------- symbol helpers ----------

    pub fn intern(&mut self, name: &str) -> SymId {
        self.obarray.intern(name)
    }

    pub fn intern_soft(&self, name: &str) -> Option<SymId> {
        self.obarray.intern_soft(name)
    }

    pub fn make_symbol(&mut self, name: &str) -> SymId {
        self.obarray.make_symbol(name)
    }

    /// `Value::Sym`, normalized so id 0 becomes `Value::Nil`.
    pub fn sym(&self, id: SymId) -> Value {
        if id == sym::NIL {
            Value::Nil
        } else {
            Value::Sym(id)
        }
    }

    pub fn symbol_name(&self, id: SymId) -> String {
        self.obarray.name(id).to_string()
    }

    pub fn sym_is(&self, v: &Value, id: SymId) -> bool {
        match v {
            Value::Sym(s) => *s == id,
            Value::Nil => id == sym::NIL,
            _ => false,
        }
    }

    /// The symbol id of a `Value::Sym`/`Value::Nil`, if it is one.
    pub fn sym_id(&self, v: &Value) -> Option<SymId> {
        match v {
            Value::Sym(s) => Some(*s),
            Value::Nil => Some(sym::NIL),
            _ => None,
        }
    }

    /// Global (default) value of a symbol.
    /// Follow `defvaralias' chains: return the ultimate base symbol.
    pub fn var_alias_target(&self, id: SymId) -> SymId {
        let prop = self.obarray.intern_soft("variable-alias").unwrap_or(0);
        let mut cur = id;
        for _ in 0..64 {
            match self.get_prop(cur, prop) {
                Value::Sym(next) => cur = next,
                _ => return cur,
            }
        }
        cur
    }

    pub fn symbol_value(&self, id: SymId) -> Value {
        let id = self.var_alias_target(id);
        // Buffer-local binding in current buffer wins. `try_borrow`:
        // primitives that hold the buffer mutably borrowed may still
        // consult variables (they see the global binding).
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    return v.clone();
                }
            }
        }
        let v = &self.obarray.symbol(id).value;
        if let Value::Sym(s) = v {
            if *s == sym::UNBOUND {
                return Value::Sym(sym::UNBOUND);
            }
        }
        v.clone()
    }

    /// Follow a symbol's function-alias chain; return the final
    /// non-symbol value (or UNBOUND sym).
    pub fn indirect_function_value(&self, v: &Value) -> Value {
        let mut cur = v.clone();
        for _ in 0..64 {
            match cur {
                Value::Sym(id) => {
                    let f = self.symbol_function(id);
                    match f {
                        Value::Sym(next) if next != sym::UNBOUND => cur = Value::Sym(next),
                        _ => return f,
                    }
                }
                other => return other,
            }
        }
        cur
    }

    /// `boundp`: is the effective value non-void?
    pub fn bound_p(&self, id: SymId) -> bool {
        let id = self.var_alias_target(id);
        // Constants (nil, t, keywords) are always bound.
        if self.obarray.symbol(id).constant {
            return true;
        }
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(bb) = b.try_borrow() {
                if let Some(v) = bb.locals.get(&id) {
                    // A void local binding (make-local-variable on an
                    // unbound variable) counts as unbound.
                    return !matches!(v, Value::Sym(s) if *s == sym::UNBOUND);
                }
            }
        }
        !matches!(
            self.obarray.symbol(id).value,
            Value::Sym(s) if s == sym::UNBOUND
        )
    }

    /// Set a variable the way `set`/`setq` does: local binding if the
    /// current buffer has one or the symbol is "automatically local".
    pub fn set_symbol(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        let id = self.var_alias_target(id);
        let constant = self.obarray.symbol(id).constant;
        if constant && id != sym::NIL && id != sym::T {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        if constant {
            // nil/t can't be set at all.
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        let is_auto_local = self.obarray.symbol(id).make_local_if_set;
        if let Some(b) = self.buffers.get(self.current_buffer) {
            if let Ok(mut bb) = b.try_borrow_mut() {
                if is_auto_local || bb.locals.contains_key(&id) {
                    bb.locals.insert(id, val);
                    return Ok(());
                }
            }
        }
        self.obarray.symbol_mut(id).value = val;
        Ok(())
    }

    /// Set the global (default) value regardless of buffer-local bindings.
    pub fn set_symbol_default(&mut self, id: SymId, val: Value) -> Result<(), Flow> {
        if self.obarray.symbol(id).constant {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![self.sym(id)]));
        }
        self.obarray.symbol_mut(id).value = val;
        Ok(())
    }

    pub fn symbol_function(&self, id: SymId) -> Value {
        self.obarray.symbol(id).function.clone()
    }

    pub fn fbound_p(&self, id: SymId) -> bool {
        // Emacs: special forms are fbound too.
        if super::special::special_form(id).is_some() {
            return true;
        }
        !matches!(
            self.obarray.symbol(id).function,
            Value::Sym(s) if s == sym::UNBOUND
        )
    }

    pub fn fset(&mut self, id: SymId, def: Value) {
        self.obarray.symbol_mut(id).function = def;
    }

    /// `get` — symbol property.
    pub fn get_prop(&self, id: SymId, prop: SymId) -> Value {
        let plist = &self.obarray.symbol(id).plist;
        plist_get(plist, prop)
    }

    /// `put` — set symbol property, returns value.
    pub fn put_prop(&mut self, id: SymId, prop: SymId, val: Value) {
        let plist = self.obarray.symbol(id).plist.clone();
        let new_plist = plist_put(&plist, prop, val);
        self.obarray.symbol_mut(id).plist = new_plist;
    }

    // ---------- specbind (dynamic let) ----------

    /// Push a dynamic binding for `sym` to `val`.
    pub fn specbind(&mut self, id: SymId, val: Value) {
        let is_auto_local = self.obarray.symbol(id).make_local_if_set;
        if let Some(b) = self.buffers.get(self.current_buffer) {
            let mut bb = b.borrow_mut();
            if is_auto_local || bb.locals.contains_key(&id) {
                let old = bb.locals.insert(id, val);
                self.specbind.push(SpecBind {
                    sym: id,
                    buf: Some(self.current_buffer),
                    old,
                });
                return;
            }
        }
        let old = self.obarray.symbol(id).value.clone();
        self.obarray.symbol_mut(id).value = val;
        self.specbind.push(SpecBind {
            sym: id,
            buf: None,
            old: Some(old),
        });
    }

    /// Pop `n` specbind entries, restoring values.
    pub fn unbind(&mut self, n: usize) {
        for _ in 0..n {
            let Some(sb) = self.specbind.pop() else {
                return;
            };
            match sb.buf {
                Some(buf_id) => {
                    if let Some(b) = self.buffers.get(buf_id) {
                        let mut bb = b.borrow_mut();
                        match sb.old {
                            Some(v) => bb.locals.insert(sb.sym, v),
                            None => bb.locals.remove(&sb.sym),
                        };
                    }
                }
                None => {
                    if let Some(v) = sb.old {
                        self.obarray.symbol_mut(sb.sym).value = v;
                    }
                }
            }
        }
    }

    pub fn specbind_depth(&self) -> usize {
        self.specbind.len()
    }

    // ---------- errors ----------

    /// `(signal sym (data...))` where data is already a list.
    pub fn signal(&self, sym_id: SymId, data: Value) -> Flow {
        Flow::Signal(Value::Sym(sym_id), data)
    }

    /// `(signal sym data-list-from-vec)`.
    pub fn signal_data(&self, sym_id: SymId, data: Vec<Value>) -> Flow {
        Flow::Signal(Value::Sym(sym_id), Value::list(data))
    }

    /// `(error "fmt" args...)` — signals `error` with a formatted message.
    pub fn error(&self, msg: impl Into<String>) -> Flow {
        self.signal_data(sym::ERROR, vec![Value::string(msg.into())])
    }

    /// `wrong-type-argument` signal: pred, value.
    pub fn wrong_type(&self, pred: &str, val: &Value) -> Flow {
        let pred_id = self.obarray.intern_soft(pred).unwrap_or_else(|| {
            // intern_soft needs &mut; fallback path used only when pred
            // is somehow not yet interned — intern it via a raw path.
            sym::ERROR
        });
        if pred_id == sym::ERROR {
            return self.signal_data(
                sym::WRONG_TYPE_ARGUMENT,
                vec![Value::string(pred), val.clone()],
            );
        }
        self.signal_data(
            sym::WRONG_TYPE_ARGUMENT,
            vec![Value::Sym(pred_id), val.clone()],
        )
    }

    /// Same but usable from `&mut self` contexts (pred interned on demand).
    pub fn wrong_type_mut(&mut self, pred: &str, val: &Value) -> Flow {
        let pred_id = self.intern(pred);
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
        // A handler's car is either a single condition name or a list
        // of names; `t` matches everything, `nil` (debug) nothing.
        if let Value::Sym(s) = handlers {
            if *s == sym::NIL {
                return false;
            }
            return self.signal_matches(sig, &Value::list(vec![handlers.clone()]));
        }
        let mut found = false;
        handlers.each_car(|h| {
            if found {
                return;
            }
            match h {
                Value::Sym(s) if *s == sym::T => found = true,
                Value::Sym(cond) => {
                    // A signal matches if `cond` is the signal symbol or
                    // appears in its `error-conditions` property.
                    if let Value::Sym(sig_id) = sig {
                        if sig_id == cond || self.condition_has(*sig_id, *cond) {
                            found = true;
                        }
                    }
                }
                Value::Cons(_) => {
                    // (cond ...) group? Emacs allows a list of symbols too.
                    h.each_car(|c| {
                        if let (Value::Sym(c_id), Value::Sym(sig_id)) = (c, sig) {
                            if *sig_id == *c_id || self.condition_has(*sig_id, *c_id) {
                                found = true;
                            }
                        }
                    });
                }
                _ => {}
            }
        });
        found
    }

    /// Does error symbol `sig` have `cond` in its `error-conditions`?
    fn condition_has(&self, sig: SymId, cond: SymId) -> bool {
        let conds = self.get_prop(sig, {
            self.obarray.intern_soft("error-conditions").unwrap_or(0)
        });
        let mut found = false;
        conds.each_car(|c| {
            if let Value::Sym(c_id) = c {
                if *c_id == cond {
                    found = true;
                }
            }
        });
        found
    }

    // ---------- entry points ----------

    /// Read and evaluate all top-level forms in `src`.
    /// Returns the last value.
    pub fn eval_str(&mut self, src: &str) -> EvalResult {
        // The reader borrows `self`, so create/drop it per form.
        let mut pos = 0usize;
        let mut last = Value::Nil;
        loop {
            let next = {
                let mut reader = Reader::new(self, src);
                reader.set_position(pos);
                match reader.read()? {
                    Some(f) => Some((f, reader.position())),
                    None => None,
                }
            };
            match next {
                Some((form, end)) => {
                    pos = end;
                    match self.eval(&form) {
                        Ok(v) => last = v,
                        // An uncaught throw is a `no-catch' error.
                        Err(Flow::Throw(tag, val)) => {
                            let nc = self.intern("no-catch");
                            return Err(self.signal_data(nc, vec![tag, val]));
                        }
                        Err(f) => return Err(f),
                    }
                }
                None => return Ok(last),
            }
        }
    }

    /// Read all top-level forms (for tests / loading).
    pub fn read_all(&mut self, src: &str) -> Result<Vec<Value>, Flow> {
        let mut reader = Reader::new(self, src);
        let mut forms = Vec::new();
        while let Some(f) = reader.read()? {
            forms.push(f);
        }
        Ok(forms)
    }

    /// `read-from-string` core: read one object, return it + end position.
    pub fn read_from_string(&mut self, src: &str, start: usize) -> Result<(Value, usize), Flow> {
        let mut reader = Reader::new(self, src);
        reader.set_position(start);
        match reader.read()? {
            None => Err(self.signal(sym::END_OF_FILE, Value::Nil)),
            Some(v) => Ok((v, reader.position())),
        }
    }

    // ---------- eval ----------

    pub fn eval(&mut self, form: &Value) -> EvalResult {
        self.eval_depth += 1;
        if self.eval_depth > self.max_lisp_eval_depth {
            self.eval_depth -= 1;
            return Err(self.error("Lisp nesting exceeds `max-lisp-eval-depth'"));
        }
        let result = self.eval_inner(form);
        self.eval_depth -= 1;
        result
    }

    fn eval_inner(&mut self, form: &Value) -> EvalResult {
        match form {
            Value::Nil => Ok(Value::Nil),
            Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Vec(_)
            | Value::Hash(_)
            | Value::Subr(_)
            | Value::Lambda(_)
            | Value::Buffer(_)
            | Value::Record(_)
            | Value::Marker(_)
            | Value::Window(_)
            | Value::Frame(_) => Ok(form.clone()),
            Value::Sym(id) => self.eval_symbol(*id),
            Value::Cons(_) => self.eval_form(form),
        }
    }

    fn eval_symbol(&mut self, id: SymId) -> EvalResult {
        if id == sym::NIL {
            return Ok(Value::Nil);
        }
        // Lexical lookup first (only when the symbol isn't special).
        if !self.obarray.symbol(id).special {
            if let Some(v) = lexenv_lookup(&self.lexenv, id) {
                return Ok(v);
            }
        }
        let v = self.symbol_value(id);
        if let Value::Sym(s) = &v {
            if *s == sym::UNBOUND {
                return Err(self.signal_data(sym::VOID_VARIABLE, vec![Value::Sym(id)]));
            }
        }
        Ok(v)
    }

    /// Evaluate a compound form `(fn . args)`.
    fn eval_form(&mut self, form: &Value) -> EvalResult {
        if self.quit_flag {
            self.quit_flag = false;
            return Err(Flow::Quit);
        }
        let cons = match form {
            Value::Cons(c) => c,
            _ => unreachable!(),
        };
        let (head, args) = {
            let b = cons.borrow();
            (b.car.clone(), b.cdr.clone())
        };

        match &head {
            Value::Sym(id) => {
                let id = *id;
                // Special form?
                if let Some(sf) = super::special::special_form(id) {
                    return sf(self, args);
                }
                // Function cell.
                let fun = self.symbol_function(id);
                if let Value::Sym(s) = &fun {
                    if *s == sym::UNBOUND {
                        return Err(self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(id)]));
                    }
                }
                self.call_function(&fun, &args, Some(id))
            }
            Value::Cons(_) => {
                // `((lambda (x) ...) a b)` — function position lambda is
                // used directly, not evaluated as a call.
                let b = match &head {
                    Value::Cons(c) => c.borrow().car.clone(),
                    _ => unreachable!(),
                };
                if self.sym_is(&b, sym::LAMBDA) {
                    let lambda = self.lambda_from_form(&head, None)?;
                    let argv = self.eval_args(&args)?;
                    return self.apply(&Value::Lambda(Rc::new(lambda)), argv);
                }
                let fun = self.eval(&head)?;
                self.call_function(&fun, &args, None)
            }
            _ => {
                let fun = self.eval(&head)?;
                self.call_function(&fun, &args, None)
            }
        }
    }

    /// Evaluate each element of a list form.
    pub fn eval_args(&mut self, args: &Value) -> Result<Vec<Value>, Flow> {
        let mut out = Vec::new();
        let mut cur = args.clone();
        loop {
            match cur {
                Value::Nil => return Ok(out),
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    out.push(self.eval(&car)?);
                    cur = next;
                }
                _ => {
                    return Err(self.error("Invalid function call form (dotted)"));
                }
            }
        }
    }

    /// Call `fun` with raw unevaluated `args` (a list). Evaluates args
    /// unless `fun` is a macro or special.
    pub fn call_function(
        &mut self,
        fun: &Value,
        args: &Value,
        sym_name: Option<SymId>,
    ) -> EvalResult {
        match fun {
            Value::Sym(id) => {
                // Function alias chain: chase.
                let mut cur = *id;
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    let f = self.symbol_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            cur = next;
                        }
                        other => {
                            // Emacs reports the originally called symbol
                            // in arity errors, not the resolved one.
                            return self.call_function(&other, args, sym_name);
                        }
                    }
                }
            }
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => (s.func)(self, vec![args.clone()]),
                _ => {
                    let argv = self.eval_args(args)?;
                    self.check_arity_subr(s, &argv, sym_name)?;
                    (s.func)(self, argv)
                }
            },
            Value::Lambda(_) => {
                // Macro: expand then eval.
                if fun.as_lambda().map(|l| l.is_macro).unwrap_or(false) {
                    let expansion = self.macro_expand_call(fun, args)?;
                    return self.eval(&expansion);
                }
                let argv = self.eval_args(args)?;
                self.apply(fun, argv)
            }
            Value::Cons(_) => {
                // A cons as function: `(lambda ...)` form or `(macro . f)`.
                let (car, cdr) = {
                    let c = match fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    // (macro . lambda) — expand with cdr as function.
                    let expansion = self.macro_expand_call(&cdr, args)?;
                    return self.eval(&expansion);
                }
                if self.sym_is(&car, sym::LAMBDA) || self.sym_is(&car, sym::QUOTE_FUNCTION) {
                    let lambda = self.lambda_from_form(fun, sym_name)?;
                    let argv = self.eval_args(args)?;
                    return self.apply(&Value::Lambda(Rc::new(lambda)), argv);
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()])),
        }
    }

    fn check_arity_subr(
        &self,
        s: &'static super::value::Subr,
        argv: &[Value],
        name: Option<SymId>,
    ) -> Result<(), Flow> {
        // Emacs reports the calling symbol for eval'd calls, the subr
        // object itself for `funcall'/`apply'.
        let who = name.map_or(Value::Subr(s), Value::Sym);
        let n = argv.len() as i128;
        let (min, max) = match s.arity {
            Arity::Range { min, max } => (min as i128, max as i128),
            Arity::Many { min } => {
                if n < min as i128 {
                    return Err(self.wrong_number_of_args(&who, n));
                }
                return Ok(());
            }
            Arity::Unevalled => return Ok(()),
        };
        if n < min || n > max {
            return Err(self.wrong_number_of_args(&who, n));
        }
        Ok(())
    }

    /// `apply`/`funcall`: call `fun` with already-evaluated `argv`.
    pub fn apply(&mut self, fun: &Value, argv: Vec<Value>) -> EvalResult {
        match fun {
            Value::Sym(id) => {
                let mut cur = *id;
                let mut hops = 0;
                loop {
                    hops += 1;
                    if hops > 64 {
                        return Err(self.error("Function alias loop"));
                    }
                    let f = self.symbol_function(cur);
                    match f {
                        Value::Sym(next) => {
                            if next == sym::UNBOUND {
                                return Err(
                                    self.signal_data(sym::VOID_FUNCTION, vec![Value::Sym(cur)])
                                );
                            }
                            cur = next;
                        }
                        other => return self.apply(&other, argv),
                    }
                }
            }
            Value::Subr(s) => match s.arity {
                Arity::Unevalled => {
                    // Special form via apply: args are already values;
                    // rebuild a list and call raw (e.g. (apply 'if ...)).
                    let list = Value::list(argv);
                    (s.func)(self, vec![list])
                }
                _ => {
                    self.check_arity_subr(s, &argv, None)?;
                    (s.func)(self, argv)
                }
            },
            Value::Lambda(l) => self.call_lambda(l, argv),
            Value::Cons(_) => {
                let (car, _) = {
                    let c = match fun {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::LAMBDA) {
                    let lambda = self.lambda_from_form(fun, None)?;
                    return self.call_lambda(&Rc::new(lambda), argv);
                }
                Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]))
            }
            _ => Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()])),
        }
    }

    /// Call an interpreted lambda with evaluated args.
    fn call_lambda(&mut self, l: &Rc<Lambda>, argv: Vec<Value>) -> EvalResult {
        // Arity.
        let (min, max_ok) = (l.required.len(), l.rest.is_some());
        if argv.len() < min || (!max_ok && argv.len() > min + l.optional.len()) {
            return Err(self.wrong_number_of_args(&Value::Lambda(l.clone()), argv.len() as i128));
        }

        // Dynamic (non-macro) functions with extended `(var init)'
        // parameters are invalid to call, like Emacs's interpreted
        // functions — the arity check above still runs first.
        if l.bad_arglist && !l.is_macro {
            return Err(self.signal_data(sym::INVALID_FUNCTION, vec![Value::Lambda(l.clone())]));
        }

        if l.env.is_some() {
            // Lexical closure: extend captured env.
            let vars = RefCell::new(HashMap::new());
            self.bind_lambda_args(&l.clone(), &argv, |sym, val| {
                vars.borrow_mut().insert(sym, val);
            });
            // `&optional` defaults may need evaluation in the new env;
            // evaluate them after the frame exists.
            let frame = Rc::new(LexFrame {
                vars,
                parent: l.env.clone(),
            });
            let saved = std::mem::replace(&mut self.lexenv, Some(frame.clone()));
            let mark = self.specbind_depth();
            let r = self.fill_optional_defaults(l, &argv, &frame);
            let result = match r {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved;
            self.unbind_to(mark);
            result
        } else {
            // Dynamic: specbind each parameter, and make sure no lexical
            // env leaks in — dynamic functions can't see callers' lexvars.
            let saved_lex = std::mem::replace(&mut self.lexenv, None);
            let mark = self.specbind_depth();
            let bind_result = self.bind_lambda_args_result(&l.clone(), &argv);
            let result = match bind_result {
                Ok(()) => self.eval_body(&l.body),
                Err(e) => Err(e),
            };
            self.lexenv = saved_lex;
            self.unbind_to(mark);
            result
        }
    }

    /// For lexical closures: evaluate `&optional` defaults inside the
    /// already-created frame for params not supplied.
    fn fill_optional_defaults(
        &mut self,
        l: &Rc<Lambda>,
        argv: &[Value],
        frame: &Rc<LexFrame>,
    ) -> Result<(), Flow> {
        let supplied = argv.len().min(l.required.len() + l.optional.len());
        let mut i = l.required.len();
        for opt in &l.optional {
            if i >= supplied {
                let v = match &opt.default {
                    Some(d) => self.eval(d)?,
                    None => Value::Nil,
                };
                frame.vars.borrow_mut().insert(opt.sym, v);
            }
            i += 1;
        }
        Ok(())
    }

    fn bind_lambda_args(&self, l: &Rc<Lambda>, argv: &[Value], mut f: impl FnMut(SymId, Value)) {
        let mut i = 0;
        for s in &l.required {
            f(*s, argv[i].clone());
            i += 1;
        }
        for opt in &l.optional {
            let v = if i < argv.len() {
                argv[i].clone()
            } else {
                // Evaluate default at call time — needs eval; handled by
                // bind_lambda_args_result path.
                opt.default.clone().unwrap_or(Value::Nil)
            };
            f(opt.sym, v);
            i += 1;
        }
        if let Some(rest) = l.rest {
            let tail = if i < argv.len() {
                Value::list(argv[i..].to_vec())
            } else {
                Value::Nil
            };
            f(rest, tail);
        }
    }

    /// Specbind version that evaluates `&optional` defaults properly.
    fn bind_lambda_args_result(&mut self, l: &Rc<Lambda>, argv: &[Value]) -> Result<(), Flow> {
        let mut i = 0;
        for s in &l.required {
            self.specbind(*s, argv[i].clone());
            i += 1;
        }
        for opt in &l.optional {
            let v = if i < argv.len() {
                argv[i].clone()
            } else {
                match &opt.default {
                    Some(d) => self.eval(d)?,
                    None => Value::Nil,
                }
            };
            self.specbind(opt.sym, v);
            i += 1;
        }
        if let Some(rest) = l.rest {
            let tail = if i < argv.len() {
                Value::list(argv[i..].to_vec())
            } else {
                Value::Nil
            };
            self.specbind(rest, tail);
        }
        Ok(())
    }

    pub fn unbind_to(&mut self, mark: usize) {
        let n = self.specbind.len() - mark;
        self.unbind(n);
    }

    /// Evaluate a body (progn), returning the last value.
    pub fn eval_body(&mut self, body: &[Value]) -> EvalResult {
        let mut last = Value::Nil;
        for f in body {
            last = self.eval(f)?;
        }
        Ok(last)
    }

    /// Evaluate a list of forms (a `progn`-style list, not a Vec).
    pub fn eval_progn(&mut self, forms: &Value) -> EvalResult {
        let mut last = Value::Nil;
        let mut cur = forms.clone();
        loop {
            match cur {
                Value::Nil => return Ok(last),
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    last = self.eval(&car)?;
                    cur = next;
                }
                _ => return Err(self.error("dotted body")),
            }
        }
    }

    /// Expand a macro call: call the macro's function on raw args.
    pub fn macro_expand_call(&mut self, mac: &Value, args: &Value) -> EvalResult {
        let argv = match args.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.error("bad macro args")),
        };
        // The macro's function receives raw forms.
        let result = match mac {
            Value::Lambda(l) if l.is_macro => self.call_lambda(l, argv)?,
            Value::Lambda(l) => self.call_lambda(l, argv)?,
            Value::Cons(_) => {
                let (car, _) = {
                    let c = match mac {
                        Value::Cons(c) => c,
                        _ => unreachable!(),
                    };
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if self.sym_is(&car, sym::MACRO) {
                    let cdr = match mac {
                        Value::Cons(c) => c.borrow().cdr.clone(),
                        _ => unreachable!(),
                    };
                    return self.macro_expand_call(&cdr, args);
                }
                self.apply(mac, argv)?
            }
            _ => self.apply(mac, argv)?,
        };
        Ok(result)
    }

    /// `macroexpand`: repeatedly expand while the form is a macro call.
    pub fn macroexpand(&mut self, form: &Value) -> EvalResult {
        let mut cur = form.clone();
        loop {
            let next = match &cur {
                Value::Cons(c) => {
                    let (car, cdr) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match car {
                        Value::Sym(id) => {
                            let f = self.symbol_function(id);
                            let is_mac = match &f {
                                Value::Lambda(l) => l.is_macro,
                                Value::Cons(cc) => {
                                    let b = cc.borrow();
                                    self.sym_is(&b.car, sym::MACRO)
                                }
                                _ => false,
                            };
                            if is_mac {
                                self.macro_expand_call(&f, &cdr)?
                            } else {
                                return Ok(cur);
                            }
                        }
                        _ => return Ok(cur),
                    }
                }
                _ => return Ok(cur),
            };
            cur = next;
        }
    }

    /// Parse a `(lambda (params) body...)` cons form into a `Lambda`.
    pub fn lambda_from_form(&mut self, form: &Value, name: Option<SymId>) -> Result<Lambda, Flow> {
        let items = match form.list_to_vec() {
            Ok(v) => v,
            Err(_) => return Err(self.signal_data(sym::INVALID_FUNCTION, vec![form.clone()])),
        };
        // items[0] = lambda, items[1] = param list, rest = body
        if items.len() < 2 {
            return Err(self.signal_data(sym::INVALID_FUNCTION, vec![form.clone()]));
        }
        self.parse_lambda(&items[1], &items[2..], name)
    }

    /// Build a `Lambda` from a param list and body forms.
    pub fn parse_lambda(
        &mut self,
        params: &Value,
        body: &[Value],
        name: Option<SymId>,
    ) -> Result<Lambda, Flow> {
        let mut required = Vec::new();
        let mut optional = Vec::new();
        let mut rest = None;
        let mut bad_arglist = false;
        let mut mode = 0u8; // 0 = required, 1 = optional, 2 = rest done
        let plist = params;
        let mut cur = plist.clone();
        loop {
            match cur {
                Value::Nil => break,
                Value::Cons(c) => {
                    let (car, next) = {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    };
                    match self.sym_id(&car) {
                        Some(s) if s == sym::OPTIONAL => mode = 1,
                        Some(s) if s == sym::REST => {
                            mode = 2;
                        }
                        _ => match mode {
                            0 => {
                                if let Some(s) = self.sym_id(&car) {
                                    required.push(s);
                                } else {
                                    // `(var init)' in required position:
                                    // Emacs counts it toward the arity but
                                    // calling the function signals
                                    // invalid-function.
                                    required.push(self.intern("&bad-param"));
                                    bad_arglist = true;
                                }
                            }
                            1 => {
                                // `sym` or `(sym default)` — the extended
                                // form only works via macros/cl- arglists;
                                // calling a plain function with it signals
                                // invalid-function.
                                match self.sym_id(&car) {
                                    Some(s) => optional.push(super::value::OptParam {
                                        sym: s,
                                        default: None,
                                    }),
                                    None => {
                                        if let Value::Cons(_) = car {
                                            let pair = car.list_to_vec().unwrap_or_default();
                                            if let Some(s) =
                                                pair.first().and_then(|v| self.sym_id(v))
                                            {
                                                optional.push(super::value::OptParam {
                                                    sym: s,
                                                    default: pair.get(1).cloned(),
                                                });
                                                bad_arglist = true;
                                            } else {
                                                return Err(self.error("bad &optional parameter"));
                                            }
                                        } else {
                                            return Err(self.error("bad &optional parameter"));
                                        }
                                    }
                                }
                            }
                            _ => {
                                if rest.is_none() {
                                    if let Some(s) = self.sym_id(&car) {
                                        rest = Some(s);
                                    } else {
                                        return Err(self.error("bad &rest parameter"));
                                    }
                                } else {
                                    return Err(self.error("multiple &rest parameters"));
                                }
                            }
                        },
                    }
                    cur = next;
                }
                _ => return Err(self.error("dotted lambda list")),
            }
        }

        // Extract docstring and interactive spec from the body front.
        let mut doc = None;
        let mut interactive = None;
        let mut start = 0;
        if let Some(Value::Str(s)) = body.first() {
            if body.len() > 1 {
                doc = Some(s.borrow().clone());
                start = 1;
            }
        }
        for (i, f) in body.iter().enumerate().skip(start) {
            if let Value::Cons(c) = f {
                let b = c.borrow();
                if self.sym_is(&b.car, sym::INTERACTIVE) {
                    interactive = Some(f.clone());
                    start = i + 1;
                    break;
                }
                if self.sym_is(&b.car, self.intern_soft("declare").unwrap_or(SymId::MAX)) {
                    start = i + 1;
                    continue;
                }
            }
            break;
        }

        Ok(Lambda {
            is_macro: false,
            required,
            optional,
            rest,
            body: body[start..].to_vec(),
            env: self.lexenv.clone(),
            doc,
            interactive,
            name: name.map(|s| self.symbol_name(s)),
            bad_arglist,
            arglist: Some(params.clone()),
            plain: self.explicit_eval_depth > 0,
        })
    }

    fn define_error_conditions(&mut self) {
        let ec = self.intern("error-conditions");
        let put = |interp: &mut Interp, name: &str, conds: &[&str]| {
            let s = interp.intern(name);
            let list = Value::list(conds.iter().map(|c| Value::Sym(interp.intern(c))).collect());
            interp.put_prop(s, ec, list);
        };
        put(self, "error", &["error"]);
        put(self, "quit", &["quit"]);
        put(self, "user-error", &["user-error", "error"]);
        put(self, "arith-error", &["arith-error", "error"]);
        put(
            self,
            "range-error",
            &["range-error", "arith-error", "error"],
        );
        put(
            self,
            "domain-error",
            &["domain-error", "arith-error", "error"],
        );
        put(
            self,
            "overflow-error",
            &["overflow-error", "arith-error", "error"],
        );
        put(
            self,
            "underflow-error",
            &["underflow-error", "arith-error", "error"],
        );
        put(
            self,
            "wrong-type-argument",
            &["wrong-type-argument", "error"],
        );
        put(
            self,
            "wrong-number-of-arguments",
            &["wrong-number-of-arguments", "error"],
        );
        put(self, "args-out-of-range", &["args-out-of-range", "error"]);
        put(self, "void-function", &["void-function", "error"]);
        put(self, "void-variable", &["void-variable", "error"]);
        put(self, "setting-constant", &["setting-constant", "error"]);
        put(self, "invalid-function", &["invalid-function", "error"]);
        put(
            self,
            "invalid-read-syntax",
            &["invalid-read-syntax", "error"],
        );
        put(self, "circular-list", &["circular-list", "error"]);
        put(
            self,
            "beginning-of-buffer",
            &["beginning-of-buffer", "error"],
        );
        put(self, "end-of-buffer", &["end-of-buffer", "error"]);
        put(self, "buffer-read-only", &["buffer-read-only", "error"]);
        put(
            self,
            "text-read-only",
            &["text-read-only", "buffer-read-only", "error"],
        );
        put(self, "mark-inactive", &["mark-inactive", "error"]);
        put(self, "file-error", &["file-error", "error"]);
        put(
            self,
            "file-missing",
            &["file-missing", "file-error", "error"],
        );
        put(self, "end-of-file", &["end-of-file", "error"]);
        put(self, "search-failed", &["search-failed", "error"]);
        put(self, "no-catch", &["no-catch", "error"]);
        put(self, "scan-error", &["scan-error", "error"]);
        put(self, "invalid-regexp", &["invalid-regexp", "error"]);
        put(self, "recursion-error", &["recursion-error", "error"]);
        put(
            self,
            "file-already-exists",
            &["file-already-exists", "file-error", "error"],
        );
        put(
            self,
            "file-supersession",
            &["file-supersession", "file-error", "error"],
        );
        put(
            self,
            "permission-denied",
            &["permission-denied", "file-error", "error"],
        );
        put(
            self,
            "coding-system-error",
            &["coding-system-error", "error"],
        );
        put(
            self,
            "coding-conversion-error",
            &["coding-conversion-error", "error"],
        );
        put(self, "mark-set", &["mark-set"]);
        put(self, "mark-active", &["mark-active"]);

        // `error-message` property strings (as in Emacs's data.c put_error).
        let em = self.intern("error-message");
        let msgs: &[(&str, &str)] = &[
            ("error", "error"),
            ("quit", "Quit"),
            ("user-error", ""),
            ("arith-error", "Arithmetic error"),
            ("range-error", "Arithmetic range error"),
            ("domain-error", "Arithmetic domain error"),
            ("overflow-error", "Arithmetic overflow error"),
            ("underflow-error", "Arithmetic underflow error"),
            ("wrong-type-argument", "Wrong type argument"),
            ("wrong-number-of-arguments", "Wrong number of arguments"),
            ("args-out-of-range", "Args out of range"),
            ("void-function", "Symbol's function definition is void"),
            ("void-variable", "Symbol's value as variable is void"),
            ("setting-constant", "Attempt to set a constant symbol"),
            ("invalid-function", "Invalid function"),
            ("invalid-read-syntax", "Invalid read syntax"),
            ("circular-list", "List contains a loop"),
            ("beginning-of-buffer", "Beginning of buffer"),
            ("end-of-buffer", "End of buffer"),
            ("buffer-read-only", "Buffer is read-only"),
            ("text-read-only", "Text is read-only"),
            ("mark-inactive", "The mark is not active now"),
            ("file-error", "File error"),
            ("file-missing", "File is missing"),
            ("end-of-file", "End of file during parsing"),
            ("search-failed", "Search failed"),
            ("no-catch", "No catch for tag"),
            ("scan-error", "Scan error"),
            ("invalid-regexp", "Invalid regexp"),
            ("coding-system-error", "Invalid coding system"),
            ("coding-conversion-error", "Coding conversion error"),
            ("file-already-exists", "File already exists"),
            ("file-supersession", "File is already being edited"),
            ("permission-denied", "Permission denied"),
            (
                "recursion-error",
                "Variable binding depth exceeds max-specpdl-size",
            ),
        ];
        for (name, msg) in msgs {
            let s = self.intern(name);
            self.put_prop(s, em, Value::string(*msg));
        }
    }

    fn define_special_variables(&mut self) {
        // Variables that are always dynamically bound even under
        // lexical-binding. Real Emacs has hundreds; these cover startup.
        let specials = [
            "standard-output",
            "standard-input",
            "lexical-binding",
            "inhibit-read-only",
            "load-path",
            "features",
            "command-line-args",
            "noninteractive",
            "emacs-version",
            "system-type",
            "debug-on-error",
            "max-lisp-eval-depth",
            "max-specpdl-size",
            "gc-cons-threshold",
            "command-history",
            "values",
            "obarray",
            "deactivate-mark",
            "transient-mark-mode",
            "kill-ring",
            "kill-ring-yank-pointer",
            "kill-ring-max",
            "last-command",
            "this-command",
            "current-prefix-arg",
            "prefix-arg",
            "minibuffer-history",
            "buffer-name-history",
            "read-expression-history",
            "command-line-args-left",
            "window-system",
            "global-map",
            "minibuffer-local-map",
            "overriding-local-map",
            "current-load-list",
            "load-in-progress",
            "load-file-name",
            "user-init-file",
            "print-level",
            "print-length",
            "print-circle",
            "most-positive-fixnum",
            "most-negative-fixnum",
            "before-change-functions",
            "after-change-functions",
            "first-change-hook",
            "post-self-insert-hook",
            "pre-command-hook",
            "post-command-hook",
            "kill-emacs-hook",
            "before-init-hook",
            "after-init-hook",
            "emacs-startup-hook",
            "delay-mode-hooks",
            "executing-kbd-macro",
            "defining-kbd-macro",
            "last-kbd-macro",
            "system-configuration",
            "system-name",
            "emacs-major-version",
            "emacs-minor-version",
            "doc-directory",
            "exec-directory",
            "exec-path",
            "process-environment",
            "path-separator",
            "null-device",
            "invocation-name",
            "invocation-directory",
            "history-length",
            "history-delete-duplicates",
            "history-add-new-input",
            "minibuffer-completion-table",
            "minibuffer-completion-predicate",
            "minibuffer-completion-confirm",
            "minibuffer-completing-file-name",
            "completion-ignore-case",
            "completion-styles",
            "read-circle",
            "find-file-hook",
            "find-file-not-found-hook",
            "write-file-functions",
            "write-contents-functions",
            "after-save-hook",
            "before-save-hook",
            "save-buffer-coding-system",
            "buffer-file-coding-system",
            "coding-system-for-write",
            "coding-system-for-read",
            "auto-mode-alist",
            "interpreter-mode-alist",
            "magic-mode-alist",
            "file-name-handler-alist",
            "completion-ignored-extensions",
            "buffer-offer-save",
            "enable-local-variables",
            "enable-local-eval",
            "safe-local-variable-values",
            "file-local-variables-alist",
            "permanent-local-variables",
            "change-major-mode-hook",
            "after-change-major-mode-hook",
            "make-backup-files",
            "backup-by-copying",
            "version-control",
            "kept-new-versions",
            "kept-old-versions",
            "delete-old-versions",
            "create-lockfiles",
            "temporary-file-directory",
            "revert-buffer-function",
            "auto-save-default",
            "auto-save-interval",
            "auto-save-timeout",
            "auto-save-file-name-transforms",
            "delete-auto-save-files",
            "require-final-newline",
            "kill-buffer-hook",
            "kill-buffer-query-functions",
            "buffer-list-update-hook",
            "indent-tabs-mode",
            "tab-width",
            "fill-column",
            "standard-indent",
            "left-margin",
            "goal-column",
            "next-screen-context-lines",
            "scroll-conservatively",
            "scroll-margin",
            "scroll-up-aggressively",
            "scroll-down-aggressively",
            "scroll-preserve-screen-position",
            "scroll-error-top-bottom",
            "echo-keystrokes",
            "visible-bell",
            "inhibit-startup-screen",
            "inhibit-startup-message",
            "initial-major-mode",
            "initial-scratch-message",
            "user-full-name",
            "user-login-name",
            "user-mail-address",
            "user-uid",
            "kill-read-only-ok",
            "yank-excluded-properties",
            "set-mark-command-repeat-pop",
            "mark-even-if-inactive",
            "regexp-search-ring",
            "search-ring",
            "search-ring-max",
            "regexp-search-ring-max",
            "search-upper-case",
            "search-invisible",
            "search-whitespace-regexp",
            "case-replace",
            "case-fold-search",
            "isearch-forward",
            "isearch-regexp",
            "register-alist",
            "undo-limit",
            "undo-strong-limit",
            "undo-outer-limit",
            "mark-ring-max",
            "global-mark-ring-max",
            "window-min-height",
            "window-min-width",
            "split-height-threshold",
            "split-width-threshold",
            "resize-mini-windows",
            "max-mini-window-height",
            "enable-recursive-minibuffers",
            "minibuffer-message-timeout",
            "read-buffer-function",
            "read-buffer-completion-ignore-case",
            "read-file-name-completion-ignore-case",
            "truncate-lines",
            "comment-start",
            "comment-end",
            "comment-start-skip",
            "comment-end-skip",
            "comment-column",
            "comment-padding",
            "comment-multi-line",
            "comment-empty-lines",
            "paragraph-start",
            "paragraph-separate",
            "paragraph-ignore-fill-prefix",
            "page-delimiter",
            "sentence-end",
            "sentence-end-double-space",
            "adaptive-fill-mode",
            "adaptive-fill-regexp",
            "adaptive-fill-function",
            "fill-prefix",
            "fill-paragraph-function",
            "fill-nobreak-predicate",
            "abbrev-mode",
            "save-abbrevs",
            "abbrev-file-name",
            "only-global-abbrevs",
            "double-click-time",
            "double-click-fuzz",
            "shell-file-name",
            "explicit-shell-file-name",
            "auto-save-hook",
            "delete-exited-processes",
            "process-connection-type",
            "text-quoting-style",
            "undo-in-region",
            "undo-in-progress",
            "shift-select-mode",
            "delete-active-region",
            "yank-handled-properties",
            "query-replace-history",
            "auto-mode-case-fold",
            "use-dialog-box",
            "menu-prompting",
            "delayed-warnings-list",
            "delayed-warnings-hook",
            "minibuffer-prompt-properties",
            "eval-expression-print-level",
            "eval-expression-print-length",
            "indent-line-function",
            "comment-indent-function",
            "major-mode",
            "mode-name",
            "minor-mode-alist",
            "minor-mode-map-alist",
            "emulation-mode-map-alists",
            "global-minor-modes",
            "buffer-read-only",
            "default-directory",
            "buffer-file-name",
            "buffer-file-truename",
            "buffer-undo-list",
            "mark-ring",
            "mark-active",
            "local-keymap",
            "list-buffers-directory",
            "buffer-saved-size",
            "buffer-display-table",
            "buffer-invisibility-spec",
            "selective-display",
            "overwrite-mode",
            "local-abbrev-table",
            "bidi-display-reordering",
            "header-line-format",
            "mode-line-format",
            "default-text-properties",
            "char-property-alias-alist",
            "inhibit-point-motion-hooks",
            "inhibit-field-text-motion",
            "show-trailing-whitespace",
            "indicate-empty-lines",
            "indicate-buffer-boundaries",
            "fringes-outside-margins",
            "word-wrap",
            "wrap-prefix",
            "line-prefix",
            "cache-long-line-scans",
            "cache-long-scans",
            "display-line-numbers",
            "line-spacing",
            "cursor-type",
            "scroll-bar-width",
            "left-fringe-width",
            "right-fringe-width",
            "left-margin-width",
            "right-margin-width",
            "print-gensym",
            "print-escape-newlines",
            "print-quoted",
            "print-unreadable",
            "print-gensym-alist",
            "print-continuous-numbering",
            "print-number-table",
        ];

        for name in &specials {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).special = true;
        }

        // Printer variables with their Emacs defaults.
        for name in ["print-quoted", "print-unreadable"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::t();
        }
        for name in ["print-gensym", "print-escape-newlines", "print-circle"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::Nil;
        }

        // Variables that are automatically buffer-local when set.
        let auto_locals = [
            "buffer-read-only",
            "default-directory",
            "tab-width",
            "fill-column",
            "indent-tabs-mode",
            "truncate-lines",
            "case-fold-search",
            "major-mode",
            "mode-name",
            "buffer-file-name",
            "buffer-file-truename",
            "buffer-undo-list",
            "local-keymap",
            "mark-active",
            "mark-ring",
            "buffer-saved-size",
            "buffer-backed-up",
            "buffer-auto-save-file-name",
            "comment-start",
            "comment-end",
            "comment-start-skip",
            "comment-end-skip",
            "comment-column",
            "comment-padding",
            "comment-multi-line",
            "comment-indent-function",
            "comment-empty-lines",
            "comment-use-syntax",
            "paragraph-start",
            "paragraph-separate",
            "paragraph-ignore-fill-prefix",
            "page-delimiter",
            "sentence-end",
            "sentence-end-base",
            "sentence-end-double-space",
            "sentence-end-without-period",
            "sentence-end-without-space",
            "adaptive-fill-mode",
            "adaptive-fill-regexp",
            "adaptive-fill-first-line-regexp",
            "adaptive-fill-function",
            "fill-prefix",
            "fill-paragraph-function",
            "fill-nobreak-predicate",
            "fill-nospace-between-words",
            "abbrev-table",
            "local-abbrev-table",
            "abbrev-mode",
            "case-fold-search",
            "overwrite-mode",
            "buffer-display-table",
            "selective-display",
            "selective-display-ellipses",
            "indicate-empty-lines",
            "indicate-buffer-boundaries",
            "left-margin",
            "tab-always-indent",
            "standard-indent",
            "goal-column",
            "next-screen-context-lines",
            "scroll-preserve-screen-position",
            "scroll-error-top-bottom",
            "scroll-conservatively",
            "scroll-margin",
            "scroll-up-aggressively",
            "scroll-down-aggressively",
            "left-fringe-width",
            "right-fringe-width",
            "fringes-outside-margins",
            "scroll-bar-width",
            "scroll-bar-height",
            "vertical-scroll-bar",
            "horizontal-scroll-bar",
            "buffer-invisibility-spec",
            "line-spacing",
            "left-margin-width",
            "right-margin-width",
            "buffer-face-mode-face",
            "text-scale-mode-amount",
            "cursor-type",
            "cursor-in-non-selected-windows",
            "mode-line-format",
            "header-line-format",
            "tab-line-format",
            "display-line-numbers",
            "display-line-numbers-type",
            "display-line-numbers-offset",
            "display-line-numbers-width",
            "display-line-numbers-widen",
            "display-line-numbers-current-absolute",
            "display-line-numbers-major-tick",
            "display-line-numbers-minor-tick",
            "wrap-prefix",
            "line-prefix",
            "display-fill-column-indicator",
            "display-fill-column-indicator-column",
            "display-fill-column-indicator-character",
            "show-trailing-whitespace",
            "bidi-paragraph-direction",
            "bidi-paragraph-start-re",
            "bidi-inhibit-bpa",
            "bidi-display-reordering",
            "buffer-file-coding-system",
            "save-buffer-coding-system",
            "coding-system-for-write",
            "coding-system-for-read",
            "enable-multibyte-characters",
            "buffer-read-only",
            "undo-in-progress",
            "undo-in-region",
            "undo-no-redo",
            "undo-no-pull",
            "mark-ring",
            "mark-active",
            "transient-mark-mode",
            "deactivate-mark",
            "permanent-local-variables",
            "file-local-variables-alist",
            "lexical-binding",
            "eval-expression-print-level",
            "eval-expression-print-length",
            "create-lockfiles",
            "backup-enable-predicate",
            "buffer-offer-save",
            "find-file-literally",
            "revert-buffer-function",
            "revert-buffer-in-progress-p",
            "revert-buffer-preserve-modes",
            "before-change-functions",
            "after-change-functions",
            "first-change-hook",
            "activate-mark-hook",
            "deactivate-mark-hook",
            "post-command-hook",
            "pre-command-hook",
            "post-self-insert-hook",
            "delay-mode-hooks",
            "change-major-mode-hook",
            "after-change-major-mode-hook",
            "text-scale-mode-amount",
        ];
        for name in &auto_locals {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).make_local_if_set = true;
            self.obarray.symbol_mut(id).special = true;
        }

        // Initial values.
        let defs: &[(&str, Value)] = &[
            ("emacs-major-version", Value::Int(31)),
            ("emacs-minor-version", Value::Int(1)),
            ("emacs-version", Value::string("31.1.0 (remacs)")),
            ("system-type", Value::Sym(self.intern("darwin"))),
            ("system-name", Value::string("localhost")),
            (
                "system-configuration",
                Value::string("aarch64-apple-darwin"),
            ),
            ("fill-column", Value::Int(70)),
            ("tab-width", Value::Int(8)),
            ("standard-indent", Value::Int(4)),
            ("indent-tabs-mode", Value::t()),
            ("case-fold-search", Value::t()),
            ("case-replace", Value::t()),
            ("noninteractive", Value::Nil),
            ("standard-output", Value::t()),
            ("standard-input", Value::t()),
            (
                "most-positive-fixnum",
                Value::Int(crate::lisp::value::FIXNUM_MAX),
            ),
            (
                "most-negative-fixnum",
                Value::Int(crate::lisp::value::FIXNUM_MIN),
            ),
            ("max-lisp-eval-depth", Value::Int(1600)),
            ("max-specpdl-size", Value::Int(2500)),
            ("gc-cons-threshold", Value::Int(800_000)),
            ("history-length", Value::Int(60)),
            ("kill-ring-max", Value::Int(120)),
            ("mark-ring-max", Value::Int(16)),
            ("global-mark-ring-max", Value::Int(16)),
            ("scroll-margin", Value::Int(0)),
            ("scroll-conservatively", Value::Int(0)),
            ("next-screen-context-lines", Value::Int(2)),
            ("undo-limit", Value::Int(160_000)),
            ("undo-strong-limit", Value::Int(240_000)),
            ("undo-outer-limit", Value::Int(24_000_000)),
            ("echo-keystrokes", Value::Int(1)),
            ("auto-save-interval", Value::Int(300)),
            ("auto-save-timeout", Value::Int(30)),
            ("double-click-time", Value::Int(500)),
            ("double-click-fuzz", Value::Int(3)),
            ("minibuffer-message-timeout", Value::Int(2)),
            ("read-process-output-max", Value::Int(65536)),
            ("max-mini-window-height", Value::Float(0.25)),
            ("window-min-height", Value::Int(4)),
            ("window-min-width", Value::Int(10)),
            ("window-safe-min-height", Value::Int(1)),
            ("window-safe-min-width", Value::Int(2)),
            ("split-height-threshold", Value::Int(80)),
            ("split-width-threshold", Value::Int(160)),
            ("completion-ignore-case", Value::Nil),
            ("read-buffer-completion-ignore-case", Value::Nil),
            ("read-file-name-completion-ignore-case", Value::Nil),
            ("history-delete-duplicates", Value::Nil),
            ("minibuffer-history", Value::Nil),
            ("buffer-name-history", Value::Nil),
            ("kill-ring", Value::Nil),
            ("values", Value::Nil),
            ("print-level", Value::Nil),
            ("print-length", Value::Nil),
            ("print-circle", Value::Nil),
            ("load-path", Value::Nil),
            ("buffer-undo-list", Value::Nil),
            ("mark-ring", Value::Nil),
            ("text-quoting-style", Value::Sym(self.intern("grave"))),
            ("transient-mark-mode", Value::t()),
            ("mark-even-if-inactive", Value::t()),
            ("shift-select-mode", Value::t()),
            ("delete-active-region", Value::t()),
            ("yank-excluded-properties", Value::t()),
            ("inhibit-read-only", Value::Nil),
            ("deactivate-mark", Value::Nil),
            ("buffer-read-only", Value::Nil),
            ("truncate-lines", Value::Nil),
            ("enable-multibyte-characters", Value::t()),
            ("obarray", Value::Nil), // TODO: real obarray object
            (
                "features",
                Value::list(vec![Value::Sym(self.intern("emacs"))]),
            ),
            ("current-load-list", Value::Nil),
            ("load-in-progress", Value::Nil),
            ("command-history", Value::Nil),
            ("regexp-search-ring", Value::Nil),
            ("search-ring", Value::Nil),
            ("register-alist", Value::Nil),
            ("global-map", Value::Nil), // set up by editor init
            ("minibuffer-local-map", Value::Nil),
            ("lexical-binding", Value::Nil),
            ("overriding-local-map", Value::Nil),
            ("auto-mode-alist", Value::Nil),
            ("interpreter-mode-alist", Value::Nil),
            ("magic-mode-alist", Value::Nil),
            ("exec-path", Value::Nil),
            ("process-environment", Value::Nil),
            ("shell-file-name", Value::string("/bin/sh")),
            ("path-separator", Value::string(":")),
            ("null-device", Value::string("/dev/null")),
            ("temporary-file-directory", Value::string("/tmp")),
            ("invocation-name", Value::string("remacs")),
            ("invocation-directory", Value::string("/usr/local/bin/")),
            ("exec-directory", Value::string("/usr/local/bin/")),
            ("doc-directory", Value::string("/usr/share/emacs/")),
            ("command-line-args", Value::Nil),
            (
                "initial-major-mode",
                Value::Sym(self.intern("lisp-interaction-mode")),
            ),
            (
                "initial-scratch-message",
                Value::string(
                    ";; This buffer is for text that is not saved, and for Lisp evaluation.\n;; To create a file, visit it with C-x C-f and enter text in its buffer.\n\n",
                ),
            ),
            ("inhibit-startup-screen", Value::Nil),
            ("user-full-name", Value::string("user")),
            ("user-login-name", Value::string("user")),
            ("user-mail-address", Value::string("user@localhost")),
            ("user-real-login-name", Value::string("user")),
            ("user-uid", Value::Int(1000)),
            ("user-real-uid", Value::Int(1000)),
            ("history-add-new-input", Value::t()),
            ("history-delete-duplicates", Value::Nil),
            ("save-abbrevs", Value::Sym(self.intern("silently"))),
            ("debug-on-error", Value::Nil),
            ("sentence-end-double-space", Value::t()),
            ("scroll-error-top-bottom", Value::Nil),
            ("scroll-preserve-screen-position", Value::Nil),
            ("set-mark-command-repeat-pop", Value::Nil),
            ("visible-bell", Value::Nil),
            ("inhibit-startup-message", Value::t()),
            ("inhibit-startup-echo-area-message", Value::Nil),
            ("use-dialog-box", Value::Nil),
            ("menu-prompting", Value::Nil),
            ("window-system", Value::Nil),
            ("delayed-warnings-list", Value::Nil),
            ("delayed-warnings-hook", Value::Nil),
            ("minibuffer-prompt-properties", Value::Nil),
            ("read-buffer-function", Value::Nil),
            (
                "completion-styles",
                Value::list(vec![Value::Sym(self.intern("basic"))]),
            ),
        ];
        for (name, val) in defs {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = val.clone();
        }
    }

    // ---------- buffers ----------

    /// The current buffer's shared handle.
    pub fn current_buffer_ref(&self) -> Option<Rc<RefCell<crate::buffer::Buffer>>> {
        self.buffers.get(self.current_buffer)
    }

    /// Buffer id for a `Value::Buffer` or `Value::Str` (name).
    pub fn buffer_id_of(&self, v: &Value) -> Option<usize> {
        match v {
            Value::Buffer(b) => Some(b.borrow().id),
            Value::Str(s) => self.buffers.by_name(&s.borrow()),
            _ => None,
        }
    }

    /// `Value` for a live buffer id.
    pub fn buffer_value(&self, id: usize) -> Option<Value> {
        self.buffers.get(id).map(|r| Value::Buffer(r.clone()))
    }

    pub fn buffer_name(&self, id: usize) -> Option<String> {
        self.buffers.get(id).map(|b| b.borrow().name.clone())
    }

    pub fn set_current_buffer(&mut self, id: usize) {
        // Emacs: `set-buffer'/`with-current-buffer' do not update
        // buffer-list recency — only window selection/display does.
        if self.buffers.get(id).is_some() {
            self.current_buffer = id;
        }
    }

    /// With `set-buffer` semantics — no error for dead/missing.
    pub fn current_buffer_is_live(&self) -> bool {
        self.buffers.get(self.current_buffer).is_some()
    }

    // ---------- excursions ----------

    /// Snapshot for `save-excursion`: buffer + point (+mark).
    pub fn save_excursion_state(&self) -> ExcursionState {
        let b = self.current_buffer_ref();
        let (point, mark, mark_active) = b
            .as_ref()
            .map(|r| {
                let bb = r.borrow();
                (bb.point, bb.mark, bb.mark_active)
            })
            .unwrap_or((0, None, false));
        ExcursionState {
            buffer: self.current_buffer,
            point,
            mark,
            mark_active,
        }
    }

    pub fn restore_excursion_state(&mut self, s: ExcursionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            {
                let mut bb = b.borrow_mut();
                bb.set_point(s.point);
                bb.mark = s.mark;
                bb.mark_active = s.mark_active;
            }
            self.set_current_buffer(s.buffer);
        }
    }

    /// Snapshot for `save-restriction` (narrowing bounds).
    pub fn save_restriction_state(&self) -> RestrictionState {
        let b = self.current_buffer_ref();
        let (begv, zv) = b
            .as_ref()
            .map(|r| {
                let bb = r.borrow();
                (bb.begv, bb.zv)
            })
            .unwrap_or((0, 0));
        RestrictionState {
            buffer: self.current_buffer,
            begv,
            zv,
        }
    }

    pub fn restore_restriction_state(&mut self, s: RestrictionState) {
        if let Some(b) = self.buffers.get(s.buffer) {
            let mut bb = b.borrow_mut();
            bb.begv = s.begv.min(bb.text.len());
            bb.zv = s.zv.max(bb.begv).min(bb.text.len());
        }
    }

    // ---------- output / echo ----------

    /// Send printed output to the current destination
    /// (`standard-output`, capture buffer, or the editor's sink).
    pub fn write_output(&mut self, s: &str) {
        if self.capture_output {
            self.output_buffer.push_str(s);
            return;
        }
        // `standard-output` may name a buffer, a marker, a function, or t.
        let dest = self.symbol_value(self.standard_output_sym);
        match dest {
            Value::Buffer(b) => {
                b.borrow_mut().insert(s);
            }
            Value::Marker(m) => {
                let mm = m.borrow();
                if let Some(buf_id) = mm.buffer {
                    let pos = mm.position;
                    drop(mm);
                    if let Some(b) = self.buffers.get(buf_id) {
                        b.borrow_mut().insert_at(pos, s);
                    }
                }
            }
            Value::Lambda(_) | Value::Subr(_) => {
                let arg = Value::string(s);
                let _ = self.apply(&dest, vec![arg]);
            }
            Value::Sym(sid) if sid != sym::T => {
                // A symbol naming a print function.
                let arg = Value::string(s);
                let _ = self.apply(&dest, vec![arg]);
            }
            _ => {
                // nil or t: the real print destination — echo area
                // interactively, stdout in batch.
                match &self.output {
                    Some(OutputSink::Buffer(buf)) => buf.borrow_mut().push_str(s),
                    Some(OutputSink::Function(f)) => {
                        let f = f.clone();
                        let arg = Value::string(s);
                        let _ = self.apply(&f, vec![arg]);
                    }
                    Some(OutputSink::Stdout) => {
                        print!("{}", s);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                    None => self.echo_message.push_str(s),
                }
            }
        }
    }

    /// `message` — show a string in the echo area (and log to *Messages*).
    pub fn message(&mut self, s: &str) {
        if self.noninteractive {
            eprintln!("{}", s);
            return;
        }
        self.echo_message = s.to_string();
        if let Some(mb) = self.buffers.by_name(" *Messages*") {
            if let Some(b) = self.buffers.get(mb) {
                let mut bb = b.borrow_mut();
                let tl = bb.text.len();
                bb.text.insert(tl, s);
                bb.text.insert(tl + s.chars().count(), "\n");
            }
        }
        if let Some(OutputSink::Buffer(buf)) = &self.output {
            buf.borrow_mut().push_str(s);
            buf.borrow_mut().push('\n');
        }
    }

    /// Read minibuffer input via the front-end hook. `single` reads one
    /// raw key event instead of a full line.
    pub fn minibuf_input(&mut self, prompt: &str, single: bool) -> Result<MinibufInput, Flow> {
        let reader = self.minibuf_reader.clone();
        match reader {
            Some(r) => r(self, prompt, single),
            None => Err(self.error("minibuffer input unavailable")),
        }
    }

    /// Read a full input line via the front-end hook.
    pub fn minibuf_line(&mut self, prompt: &str) -> Result<String, Flow> {
        self.minibuf_level += 1;
        let r = self.minibuf_input(prompt, false);
        self.minibuf_level -= 1;
        match r? {
            MinibufInput::Text(t) => Ok(t),
            MinibufInput::Key(k) => Ok(char::from_u32(k as u32)
                .map(|c| c.to_string())
                .unwrap_or_default()),
        }
    }

    /// Value of `current-prefix-arg` (nil when unset).
    pub fn prefix_arg(&self) -> Value {
        self.intern_soft("current-prefix-arg")
            .map(|id| self.symbol_value(id))
            .unwrap_or(Value::Nil)
    }

    /// `wrong-type-argument` where pred is a value (used by builtins that
    /// pass arbitrary predicates).
    pub fn error_obj(&self, msg: &str, v: &Value) -> Flow {
        self.signal_data(
            sym::ERROR,
            vec![Value::string(format!(
                "{}: {}",
                msg,
                self.princ_to_string(v)
            ))],
        )
    }

    /// Execute a command by name (M-x dispatch entry point).
    /// The editor's command loop calls this with the command symbol.
    pub fn command_execute(&mut self, cmd: &Value) -> EvalResult {
        // Resolve to function, then call with interactive args.
        let fun = match cmd {
            Value::Sym(id) => self.symbol_function(*id),
            other => other.clone(),
        };
        // Interactive spec: evaluate it to get args (simplified — the
        // editor drives real prompting; batch mode uses defaults).
        if let Some(l) = fun.as_lambda() {
            if let Some(spec) = &l.interactive {
                let argv = self.eval_interactive_spec(spec)?;
                return self.apply(&fun, argv);
            }
        }
        // Subrs: honor the declared interactive spec, if any.
        if let Value::Subr(s) = &fun {
            if let Some(spec) = subr_interactive(s.name) {
                let isym = self.intern("interactive");
                let spec_form = Value::list(vec![Value::Sym(isym), Value::string(spec)]);
                let argv = self.eval_interactive_spec(&spec_form)?;
                return self.apply(&fun, argv);
            }
        }
        self.apply(&fun, vec![])
    }

    /// Evaluate an `(interactive ...)` spec to an argv.
    /// Non-interactive support: `interactive` with no string → no args;
    /// with a string spec we honor the simple codes when input is
    /// pre-supplied in `command_args`.
    pub fn eval_interactive_spec(&mut self, spec_form: &Value) -> Result<Vec<Value>, Flow> {
        let items = spec_form.list_to_vec().unwrap_or_default();
        let spec = items.get(1).cloned().unwrap_or(Value::Nil);
        match spec {
            Value::Nil => {
                // (interactive) or (interactive (list ...))
                if items.len() > 1 {
                    if let Value::Cons(c) = &items[1] {
                        let b = c.borrow();
                        if self.sym_is(&b.car, self.intern_soft("list").unwrap_or(SymId::MAX)) {
                            let list_expr = b.cdr.clone();
                            drop(b);
                            let v = self.eval(&list_expr)?;
                            return Ok(v.list_to_vec().unwrap_or_default());
                        }
                    }
                }
                Ok(Vec::new())
            }
            Value::Str(s) => {
                // Parse letter codes. A code's prompt is the text after
                // the code up to the next newline (Emacs spec syntax).
                let chars: Vec<char> = s.borrow().chars().collect();
                let mut out = Vec::new();
                let mut pos = 0usize;
                while pos < chars.len() {
                    let c = chars[pos];
                    pos += 1;
                    match c {
                        // Prefix modifiers and prompt separators produce
                        // no argument.
                        '*' | '^' | '@' | '\n' => {}
                        'r' => {
                            let (p, m) = self
                                .current_buffer_ref()
                                .map(|b| {
                                    let bb = b.borrow();
                                    (bb.point, bb.mark.unwrap_or(bb.point))
                                })
                                .unwrap_or((0, 0));
                            // `r' yields region-beginning/region-end
                            // (ordered) like Emacs.
                            let (beg, end) = if p <= m { (p, m) } else { (m, p) };
                            out.push(Value::Int(beg as i128 + 1));
                            out.push(Value::Int(end as i128 + 1));
                        }
                        'd' => {
                            let p = self
                                .current_buffer_ref()
                                .map(|b| b.borrow().point)
                                .unwrap_or(0);
                            out.push(Value::Int(p as i128 + 1));
                        }
                        'm' => {
                            let m = self
                                .current_buffer_ref()
                                .and_then(|b| b.borrow().mark)
                                .unwrap_or(0);
                            out.push(Value::Int(m as i128 + 1));
                        }
                        'p' | 'P' => {
                            let pa = self.prefix_arg();
                            let numeric = c == 'p';
                            if pa.is_nil() {
                                let fb = self.command_args.first().cloned();
                                out.push(match fb {
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
                                out.push(prefix_numeric(self, &pa));
                            } else {
                                out.push(pa);
                            }
                            skip_prompt(&chars, &mut pos);
                        }
                        'n' | 'N' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            let pa = self.prefix_arg();
                            if !pa.is_nil() {
                                out.push(prefix_numeric(self, &pa));
                            } else if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                let n = s.trim().parse::<i128>().unwrap_or(0);
                                out.push(Value::Int(n));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        's' | 'B' | 'b' | 'F' | 'f' | 'D' | 'z' | 'Z' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                // `b' defaults to the current buffer on
                                // empty input (Emacs spec semantics).
                                if s.is_empty() && c == 'b' {
                                    let n = self
                                        .current_buffer_ref()
                                        .map(|b| b.borrow().name.clone())
                                        .unwrap_or_default();
                                    out.push(Value::string(n));
                                } else {
                                    out.push(Value::string(s));
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'a' | 'C' | 'S' | 'v' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                out.push(Value::Sym(self.intern(&s)));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'k' | 'K' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                out.push(Value::string(s));
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'x' | 'X' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                let s = self.minibuf_line(&prompt)?;
                                match self.read_from_string(&s, 0) {
                                    Ok((form, _)) => {
                                        if c == 'x' {
                                            out.push(self.eval(&form)?);
                                        } else {
                                            out.push(form);
                                        }
                                    }
                                    Err(_) => out.push(Value::Nil),
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        'c' | 'e' => {
                            let prompt = take_prompt(&chars, &mut pos);
                            if let Some(v) = self.command_args.first() {
                                out.push(v.clone());
                            } else if self.minibuf_reader.is_some() {
                                match self.minibuf_input(&prompt, true)? {
                                    MinibufInput::Key(k) => out.push(Value::Int(k)),
                                    MinibufInput::Text(t) => {
                                        let n = t.chars().next().map(|c| c as i128).unwrap_or(0);
                                        out.push(Value::Int(n));
                                    }
                                }
                            } else {
                                out.push(Value::Nil);
                            }
                        }
                        _ => {
                            // Unknown/prompting code → nil in batch.
                            skip_prompt(&chars, &mut pos);
                            out.push(Value::Nil);
                        }
                    }
                }
                Ok(out)
            }
            _ => {
                // (interactive (list (read-string ...) ...)) — evaluate.
                let v = self.eval(&spec)?;
                Ok(v.list_to_vec().unwrap_or_default())
            }
        }
    }
}

/// Interactive specs for subrs that Emacs declares `interactive'.
/// `commandp`/`command-execute` consult this for primitives.
pub(crate) fn subr_interactive(name: &str) -> Option<&'static str> {
    const T: &[(&str, &str)] = &[
        ("self-insert-command", "p"),
        ("forward-char", "p"),
        ("backward-char", "p"),
        ("delete-char", "p\nP"),
        ("delete-backward-char", "p\nP"),
        ("move-beginning-of-line", "p"),
        ("move-end-of-line", "p"),
        ("forward-word", "p"),
        ("backward-word", "p"),
        ("forward-sexp", "p"),
        ("backward-sexp", "p"),
        ("forward-line", "p"),
        ("newline", "p\nP"),
        ("open-line", "p\nP"),
        ("indent-line-to", "p"),
        ("indent-rigidly", "r\nP"),
        ("transpose-chars", "p"),
        ("kill-line", "P\np"),
        ("kill-region", "r"),
        ("kill-whole-line", "p"),
        ("kill-word", "p"),
        ("backward-kill-word", "p"),
        ("yank", "P"),
        ("yank-pop", "p"),
        ("undo", "p"),
        ("scroll-up-command", "P"),
        ("scroll-down-command", "P"),
        ("scroll-other-window", "p"),
        ("upcase-word", "p"),
        ("downcase-word", "p"),
        ("capitalize-word", "p"),
        ("upcase-region", "r"),
        ("downcase-region", "r"),
        ("zap-to-char", "p\ncZap to char: "),
        ("just-one-space", "p"),
        ("delete-horizontal-space", "p"),
        ("delete-indentation", "p"),
        ("digit-argument", "p"),
        ("negative-argument", "p"),
        ("universal-argument", ""),
        ("abort-recursive-edit", ""),
        ("suspend-emacs", ""),
        ("kill-emacs", "P"),
        ("save-buffer", "p"),
        ("write-file", "FWrite file: "),
        ("find-file", "FFind file: "),
        ("other-window", "p\np"),
        ("delete-window", "p"),
        ("delete-other-windows", "p"),
        ("split-window-below", "P"),
        ("split-window-right", "p"),
        ("narrow-to-region", "r"),
        ("narrow-to-page", "r"),
        ("widen", ""),
        ("beginning-of-defun", "p"),
        ("end-of-defun", "p"),
        ("mark-defun", ""),
        ("narrow-to-defun", ""),
        ("what-cursor-position", "P"),
        ("insert-char", "p\nP"),
        ("erase-buffer", ""),
        ("bury-buffer", "bBury buffer: "),
        ("kill-buffer", "bKill buffer: "),
        ("move-to-window-line", "P"),
        ("recenter", "P"),
        ("count-words-region", ""),
        ("eval-expression", "xEval: "),
        ("execute-extended-command", "P"),
        ("mark-page", "p"),
        ("count-lines-page", "p"),
    ];
    T.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
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
            if i.sym_is(v, i.intern_soft("-").unwrap_or(SymId::MAX)) {
                Value::Int(-1)
            } else {
                Value::Int(1)
            }
        }
        _ => Value::Int(1),
    }
}

/// Consume an interactive-spec prompt (text until `\n` or end).
fn take_prompt(chars: &[char], pos: &mut usize) -> String {
    let mut p = String::new();
    while *pos < chars.len() && chars[*pos] != '\n' {
        p.push(chars[*pos]);
        *pos += 1;
    }
    if *pos < chars.len() {
        *pos += 1;
    }
    p
}

/// Skip an interactive-spec prompt without capturing it.
fn skip_prompt(chars: &[char], pos: &mut usize) {
    let _ = take_prompt(chars, pos);
}

/// Saved point/buffer for `save-excursion`.
pub struct ExcursionState {
    pub buffer: usize,
    pub point: usize,
    pub mark: Option<usize>,
    pub mark_active: bool,
}

/// Saved narrowing bounds for `save-restriction`.
pub struct RestrictionState {
    pub buffer: usize,
    pub begv: usize,
    pub zv: usize,
}

/// `match-data` contents after a successful search.
#[derive(Clone)]
pub struct MatchData {
    /// Group start/end pairs (0-based char offsets in the searched text).
    pub regs: Vec<Option<usize>>,
    /// True if the match was against a buffer (positions are 1-based
    /// Emacs positions); false for strings (0-based indices).
    pub in_buffer: bool,
    /// Base offset of the searched text in the buffer (0 for strings).
    pub base: usize,
}

// `Symbol` extension: make-local-if-set flag stored in obarray? We need the
// field on Symbol; it's added in obarray.rs.
pub fn plist_get(plist: &Value, prop: SymId) -> Value {
    let mut cur = plist.clone();
    let mut n = 0;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        // Value is cadr.
                        if let Value::Cons(c2) = &next {
                            return c2.borrow().car.clone();
                        }
                        return Value::Nil;
                    }
                }
                // advance two
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        cur = n2;
                    }
                    _ => return Value::Nil,
                }
                n += 1;
                if n > 10000 {
                    return Value::Nil;
                }
            }
            _ => return Value::Nil,
        }
    }
}

/// `put` on a plist value — returns a new plist with prop set.
/// Mutates in place when possible (Emacs mutates the plist).
pub fn plist_put(plist: &Value, prop: SymId, val: Value) -> Value {
    let mut cur = plist.clone();
    // Tracks the last cons cell reached while walking pairs — the
    // value cell of the final pair — so new props append at the end.
    let mut last_pair_end: Option<super::value::ConsRef> = None;
    loop {
        match cur {
            Value::Cons(c) => {
                let (car, next) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let Value::Sym(s) = car {
                    if s == prop {
                        if let Value::Cons(c2) = &next {
                            c2.borrow_mut().car = val;
                            return plist.clone();
                        }
                        return plist.clone();
                    }
                }
                match next {
                    Value::Cons(c2) => {
                        let n2 = {
                            let b = c2.borrow();
                            b.cdr.clone()
                        };
                        last_pair_end = Some(c2);
                        cur = n2;
                    }
                    _ => break,
                }
            }
            _ => break,
        }
    }
    // Not found: append (prop val) at the END of the plist,
    // matching Emacs's put/plist-put ordering.
    match last_pair_end {
        Some(cell) => {
            cell.borrow_mut().cdr = Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil));
            plist.clone()
        }
        None => Value::cons(Value::Sym(prop), Value::cons(val, Value::Nil)),
    }
}

impl Value {
    pub fn as_lambda(&self) -> Option<&Rc<Lambda>> {
        match self {
            Value::Lambda(l) => Some(l),
            _ => None,
        }
    }
}
