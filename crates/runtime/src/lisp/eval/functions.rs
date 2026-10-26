//! Function-cell lookup, hidden definitions, primitive identity, and definition history.

use super::Interp;
use crate::lisp::{
    error::Flow,
    obarray::sym,
    value::{SymId, Value},
};
use rustc_hash::FxHashMap as HashMap;

impl Interp {
    /// Follow a symbol's function-alias chain; return the final
    /// non-symbol value (or UNBOUND sym).
    pub fn indirect_function_value(&self, v: &Value) -> Value {
        let mut cur = crate::lisp::builtins::misc::unpos(self, v);
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
    /// `functionp' on a value: subrs, lambdas, `(lambda ...)' conses,
    /// and symbols whose function cell holds a non-macro definition.
    pub fn function_p(&self, v: &Value) -> bool {
        match v {
            Value::Subr(_) | Value::Lambda(_) => true,
            Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
            Value::Sym(id) => {
                // GNU resolves `defalias' chains: the alias's final
                // target decides.
                let mut f = self.symbol_function(*id);
                let mut hops = 0usize;
                while let Value::Sym(next) = f {
                    if next == sym::UNBOUND || hops >= 64 {
                        return false;
                    }
                    hops += 1;
                    f = self.symbol_function(next);
                }
                match f {
                    Value::Subr(s) => self
                        .intern_soft(s.name)
                        .and_then(|x| crate::lisp::special::special_form(x))
                        .is_none(),
                    Value::Lambda(l) => !l.is_macro,
                    Value::Cons(c) => self.sym_is(&c.borrow().car, sym::LAMBDA),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn symbol_function(&self, id: SymId) -> Value {
        self.obarray.symbol(id).function.clone()
    }

    /// Hidden macros remain available to the interpreted implementation
    /// of dumped code, without exposing them to users.
    fn dumped_macros_allowed(&self) -> bool {
        self.current_lambda_dumped || self.loading_compiled_library || self.boot_loading
    }

    /// Function cell used by ordinary calls.
    pub(super) fn callable_function(&mut self, id: SymId) -> Value {
        self.resolve_function(id, FunctionPosition::Call)
    }

    fn resolve_function(&mut self, id: SymId, position: FunctionPosition) -> Value {
        let f = self.symbol_function(id);
        // A `nil' cell (fset to nil, or a cell that holds nil rather
        // than `unbound') is a void function in GNU terms: `funcall
        // nil' reports `void-function', not `invalid-function'.
        let f = if matches!(position, FunctionPosition::Call) && matches!(f, Value::Nil) {
            Value::Sym(sym::UNBOUND)
        } else if !matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            f
        } else {
            self.dumped_function_for_call(id).unwrap_or(f)
        };
        // Inside a macro expander running for compiled-equivalent
        // (dumped/bundled) code, resolve to the unadvised base: GNU
        // expanded these call sites at byte-compile time, before any
        // advice could exist.
        if self.expand_unadvised_depth > 0 {
            self.unadvised_base(&f)
        } else {
            f
        }
    }

    /// Hidden dump-time definition for SYM, if one was stashed while its
    /// public function cell was voided for GNU -Q compatibility.
    fn dumped_function(&mut self, id: SymId) -> Option<Value> {
        let prop = self.q("remacs--dump-fn");
        let hidden = self.get_prop(id, prop);
        if hidden.is_nil() || matches!(hidden, Value::Sym(s) if s == sym::UNBOUND) {
            None
        } else {
            Some(hidden)
        }
    }

    /// Folded runtime functions remain compatibility fallbacks until
    /// their source/autoload paths fully provide them (face-remap, rx,
    /// etc.). Voided macros are only compiler implementation details.
    fn dumped_function_for_call(&mut self, id: SymId) -> Option<Value> {
        let function = self.dumped_function(id)?;
        if self.is_macro_function(&function) && !self.dumped_macros_allowed() {
            None
        } else {
            Some(function)
        }
    }

    /// Function cell for a form head. Voided macro cells stay void in
    /// user code; dumped bodies may use their hidden compiler macros.
    pub(super) fn form_function(&mut self, id: SymId) -> Value {
        self.resolve_function(id, FunctionPosition::Form)
    }

    pub(super) fn is_macro_function(&self, v: &Value) -> bool {
        match v {
            Value::Lambda(l) => l.is_macro,
            Value::Cons(c) => self.sym_is(&c.borrow().car, sym::MACRO),
            _ => false,
        }
    }

    pub fn fbound_p(&self, id: SymId) -> bool {
        // Emacs: special forms are fbound too.
        if crate::lisp::special::special_form(id).is_some() {
            return true;
        }
        // GNU fboundp: a nil or unbound cell is void.  (defalias 'x nil)
        // stores nil, dropping fboundp but leaving symbol-function nil.
        !matches!(self.obarray.symbol(id).function, Value::Nil)
            && !matches!(
                self.obarray.symbol(id).function,
                Value::Sym(s) if s == sym::UNBOUND
            )
    }
    /// GNU permits function definitions on t and keywords, but nil must
    /// retain a nil function cell. Check before replacing definitions or advice.
    pub(crate) fn check_function_assignment(&mut self, id: SymId, def: &Value) -> Result<(), Flow> {
        if id == sym::NIL && !def.is_nil() {
            return Err(self.signal_data(sym::SETTING_CONSTANT, vec![Value::Nil]));
        }
        Ok(())
    }
    pub fn fset(&mut self, id: SymId, def: Value) {
        if self.definition_history_enabled {
            if let Value::Subr(old) = self.symbol_function(id) {
                let enabled = self
                    .q_soft("native-comp-enable-subr-trampolines")
                    .map_or(true, |variable| self.symbol_value(variable).truthy());
                if enabled
                    && old.name == self.symbol_name(id)
                    && !crate::lisp::builtins::eq_values(&Value::Subr(old), &def)
                {
                    self.native_trampolines.insert(old.name.to_string());
                }
            }
        }
        // GNU stores `(macro . fn)' as the literal function cell, so
        // `symbol-function' returns that cons — a stable identity that
        // nadvice's `(cdr nf)' gv place mutates in place.  Call paths
        // unwrap the cons at dispatch.
        let def = match &def {
            Value::Lambda(l) if l.is_macro => Value::cons(Value::Sym(sym::MACRO), def.clone()),
            _ => def,
        };
        self.macro_gen += 1;
        self.obarray.symbol_mut(id).function = def;
    }

    /// The canonical subr for SYM when SYM names a dedicated GNU
    /// byte-code opcode — `car', `cdr', `list', `concat', `eq', the
    /// arithmetic ops, ... — or None.  GNU's compiler emits a
    /// dedicated opcode for each of these calls, so compiled code
    /// invokes the primitive directly and `symbol-function' (and any
    /// advice layered on it) is never consulted; remacs routes the
    /// equivalent calls through this table while a `compiled'
    /// (shipped-library-equivalent) function is running.
    pub(super) fn byte_op_subr(&mut self, id: SymId) -> Option<&'static crate::lisp::value::Subr> {
        if self.byte_op_subrs.is_none() {
            let mut m = HashMap::default();
            let mut by_name = HashMap::default();
            // Names mirrored from GNU's bytecomp.el `byte-defop' table
            // (opcodes 56-168): each compiled call to one of these is
            // a dedicated opcode, not a `byte-call' through
            // `symbol-function'.
            for s in crate::lisp::builtins::dump_subrs() {
                if is_byte_op_primitive(s.name) {
                    let id = self.intern(s.name);
                    m.insert(id, s);
                    by_name.insert(s.name, s);
                    let alias = match s.name {
                        "string-equal" => Some("string="),
                        "string-lessp" => Some("string<"),
                        _ => None,
                    };
                    if let Some(alias) = alias {
                        m.insert(self.intern(alias), s);
                        by_name.insert(alias, s);
                    }
                }
            }
            self.byte_op_subrs = Some(m);
            self.byte_op_subrs_by_name = Some(by_name);
        }
        self.byte_op_subrs.as_ref().unwrap().get(&id).copied()
    }

    pub(super) fn byte_op_subr_named(
        &mut self,
        name: &str,
    ) -> Option<&'static crate::lisp::value::Subr> {
        if self.byte_op_subrs_by_name.is_none() {
            // Build from names without interning them: a byte-code opcode
            // remains tied to the canonical primitive even after Lisp has
            // removed that name from the public obarray.
            let mut by_name = HashMap::default();
            let mut add = |subr: &'static crate::lisp::value::Subr| {
                by_name.insert(subr.name, subr);
                match subr.name {
                    "string-equal" => {
                        by_name.insert("string=", subr);
                    }
                    "string-lessp" => {
                        by_name.insert("string<", subr);
                    }
                    _ => {}
                }
            };
            if let Some(map) = &self.byte_op_subrs {
                for subr in map.values() {
                    add(*subr);
                }
            } else {
                for subr in crate::lisp::builtins::dump_subrs()
                    .iter()
                    .filter(|subr| is_byte_op_primitive(subr.name))
                {
                    add(*subr);
                }
            }
            self.byte_op_subrs_by_name = Some(by_name);
        }
        self.byte_op_subrs_by_name
            .as_ref()
            .and_then(|map| map.get(name).copied())
    }

    /// GNU data.c's history is a filename/old-definition plist. Reusing
    /// an inactive file removes its old change; repeating the active file
    /// keeps the definition from before that file's first change.
    fn record_function_history(&mut self, id: SymId) -> Result<(), Flow> {
        let old = self.symbol_function(id);
        if old.is_nil() || matches!(old, Value::Sym(s) if s == sym::UNBOUND) {
            return Ok(());
        }
        let current = self.q("current-load-list");
        let entries = self.symbol_value(current).list_to_vec().unwrap_or_default();
        let file = match entries.last() {
            Some(file @ Value::Str(_)) => file.clone(),
            _ => Value::Nil,
        };
        let property = self.q("function-history");
        let past = self.get_prop(id, property);
        let fields = match past.list_to_vec() {
            Ok(fields) => fields,
            Err(crate::lisp::value::ListError::Dotted(tail)) => {
                return Err(self.wrong_type_mut("listp", &tail));
            }
            Err(crate::lisp::value::ListError::Circular(tail)) => {
                let error = self.q("circular-list");
                return Err(self.signal_data(error, vec![tail]));
            }
        };
        for position in (0..fields.len()).step_by(2) {
            if crate::lisp::builtins::equal_values(self, &fields[position], &file)? {
                if position == 0 {
                    return Ok(());
                }
                let mut previous = past.clone();
                for _ in 0..position - 2 {
                    let Value::Cons(cell) = previous else {
                        unreachable!()
                    };
                    previous = cell.borrow().cdr.clone();
                }
                let mut matched = past.clone();
                for _ in 0..position {
                    let Value::Cons(cell) = matched else {
                        unreachable!()
                    };
                    matched = cell.borrow().cdr.clone();
                }
                let Value::Cons(previous) = previous else {
                    unreachable!()
                };
                let Value::Cons(matched) = matched else {
                    unreachable!()
                };
                previous.borrow_mut().cdr = matched.borrow().cdr.clone();
                break;
            }
        }
        self.put_prop(id, property, Value::cons(file, Value::cons(old, past)));
        Ok(())
    }

    /// `defalias'-level write to SYM's function cell: when SYM carries
    /// advice, the new definition substitutes the chain's base (GNU's
    /// `advice--defalias-fset'); otherwise a plain `fset'.
    pub fn fset_defalias(&mut self, id: SymId, def: Value) -> Result<(), Flow> {
        self.check_function_assignment(id, &def)?;
        if self.definition_history_enabled {
            self.record_function_history(id)?;
        }
        self.name_compiled_lambda(id, &def);
        if self.advice_bases.iter().any(|(s, _)| *s == id) {
            if let Some(pos) = self.advice_bases.iter().position(|(s, _)| *s == id) {
                self.advice_bases[pos].1 = def;
            }
            self.recompose_advice(id)
        } else {
            self.fset(id, def);
            Ok(())
        }
    }
}

fn is_byte_op_primitive(name: &str) -> bool {
    matches!(
        name,
        "car"
            | "cdr"
            | "cons"
            | "list"
            | "nth"
            | "nthcdr"
            | "memq"
            | "member"
            | "assq"
            | "elt"
            | "length"
            | "aref"
            | "aset"
            | "nreverse"
            | "setcar"
            | "setcdr"
            | "nconc"
            | "concat"
            | "substring"
            | "get"
            | "set"
            | "fset"
            | "symbolp"
            | "consp"
            | "stringp"
            | "listp"
            | "not"
            | "null"
            | "eq"
            | "equal"
            | "symbol-value"
            | "symbol-function"
            | "symbol-name"
            | "eqlsign"
            | "gtr"
            | "lss"
            | "leq"
            | "geq"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "max"
            | "min"
            | "="
            | "<"
            | ">"
            | "<="
            | ">="
            | "1+"
            | "1-"
            | "point"
            | "point-max"
            | "point-min"
            | "goto-char"
            | "insert"
            | "char-after"
            | "char-before"
            | "following-char"
            | "preceding-char"
            | "current-column"
            | "indent-to"
            | "eolp"
            | "eobp"
            | "bolp"
            | "bobp"
            | "current-buffer"
            | "set-buffer"
            | "forward-char"
            | "forward-word"
            | "skip-chars-forward"
            | "skip-chars-backward"
            | "forward-line"
            | "char-syntax"
            | "buffer-substring"
            | "delete-region"
            | "narrow-to-region"
            | "widen"
            | "end-of-line"
            | "set-marker"
            | "match-beginning"
            | "match-end"
            | "upcase"
            | "downcase"
            | "string="
            | "string<"
            | "string-equal"
            | "string-lessp"
            | "numberp"
            | "integerp"
            | "car-safe"
            | "cdr-safe"
            | "interactive-p"
    )
}

/// Ordinary calls normalize a nil cell to void-function; form-head lookup
/// retains nil. Hidden helpers and expansion-time advice rules are shared.
enum FunctionPosition {
    Call,
    Form,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nil_function_cells_are_void_for_calls_but_preserved_for_form_heads() {
        let mut i = Interp::vacant();
        i.obarray = crate::lisp::obarray::Obarray::new();
        let symbol = i.intern("nil-function-cell");
        i.fset(symbol, Value::Nil);
        assert!(matches!(i.callable_function(symbol), Value::Sym(s) if s == sym::UNBOUND));
        assert!(i.form_function(symbol).is_nil());
        assert!(i.symbol_function(symbol).is_nil());
        assert!(!i.fbound_p(symbol));
    }
}
