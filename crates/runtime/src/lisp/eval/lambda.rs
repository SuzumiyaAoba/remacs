//! Lambda construction, argument binding, and interpreted/byte-code dispatch.

use super::{Interp, lexenv_as_value_checked, lexenv_extend_checked, lexframe_bind_checked};
use crate::lisp::{
    error::{EvalResult, Flow},
    obarray::sym,
    value::{Argv, Lambda, SymId, Value},
};
use rustc_hash::FxHashSet;
use std::rc::Rc;

// FNV-1a fingerprints of the exact bytecode bodies emitted by GNU Emacs
// 31.1's unidata-gen.elc. Include code length in each signature.
const UNIDATA_DESCRIPTION_CODE: (usize, u64) = (5, 0x1b80_e259_5834_af96);
const UNIDATA_EXTRA_SLOT_CODE: (usize, u64) = (30, 0x2465_fa11_5ca6_0a4e);
const UNIDATA_PROPERTY_CODE: [(usize, u64); 2] = [
    (452, 0x291e_9ef9_d149_7e33),
    (337, 0xf2c7_c08e_52a4_45a1),
];

fn unidata_gen_literal(items: &[Value]) -> bool {
    let Some(Value::Cons(location)) = items.get(4) else {
        return false;
    };
    let Ok(location) = location.try_borrow() else {
        return false;
    };
    let (Value::Str(filename), Value::Int(_)) = (&location.car, &location.cdr) else {
        return false;
    };
    filename
        .try_borrow()
        .ok()
        .map(|filename| {
            filename
                .to_unicode_lossy()
                .rsplit(['/', '\\'])
                .next()
                == Some("unidata-gen.elc")
        })
        .unwrap_or(false)
}

fn unidata_code_signature(i: &mut Interp, items: &[Value]) -> Option<(usize, u64)> {
    let bytes = crate::lisp::bytecode::code_bytes(i, items.get(1)?).ok()?;
    let fingerprint = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    Some((bytes.len(), fingerprint))
}

impl Interp {
    /// Record the bound name for a compiled lambda that doesn't carry
    /// one — `#[...]' literals read from `.elc' files arrive nameless,
    /// and `defalias' is the point where GNU's nativecomp loads learn
    /// the bound symbol (`#<subr NAME>' print + wna datum).  Keyed on
    /// the Rc pointer since `Lambda' is not Clone.
    pub(super) fn name_compiled_lambda(&mut self, id: SymId, def: &Value) {
        if let Value::Lambda(l) = def {
            if l.name.is_none() && l.compiled {
                self.compiled_fn_names
                    .borrow_mut()
                    .push((Rc::downgrade(l), self.symbol_name_text(id)));
            }
        }
    }

    /// Effective display name of a lambda: `l.name' plus the
    /// `defalias'-stamped name for `.elc' twins.
    pub(crate) fn lambda_name(&self, l: &Rc<Lambda>) -> Option<crate::lisp::text::EmacsText> {
        if let Some(n) = &l.name {
            return Some(n.clone());
        }
        let ptr = Rc::as_ptr(l);
        let names = self.compiled_fn_names.borrow();
        names.iter().find_map(|(w, n)| {
            (w.upgrade().is_some_and(|r| Rc::as_ptr(&r) == ptr)).then(|| n.clone())
        })
    }

    /// Call an interpreted lambda with evaluated args.  `shown' is what
    /// `wrong-number-of-arguments' reports as the function — GNU prints
    /// the called symbol when invoked by name.
    pub(super) fn call_lambda(&mut self, l: &Rc<Lambda>, argv: Argv, shown: &Value) -> EvalResult {
        self.with_lambda_context(l, |i| i.call_lambda_inner(l, argv, shown))
    }

    /// Dispatch for `#[ARGDESC BYTE-CODE CONSTANTS DEPTH FILE]' literals
    /// read from the dumped uni-*.el char-table extra slots.  GNU runs
    /// the embedded byte-code (unidata-gen.el's `unidata-get-*'/
    /// `unidata-put-*'/describe helpers); we handle the get/put and
    /// alist-description shapes here.  Other functions run through the
    /// ordinary byte-code interpreter.
    fn unidata_bc_dispatch(&mut self, l: &Rc<Lambda>, argv: &[Value]) -> Option<EvalResult> {
        // Only bare `#[...]' literals read from the dumped uni-*.el data
        // route here.  `make-closure' products splice their captured env
        // into consts[0..]; a mutated box can masquerade as an alist and
        // would be short-circuited into Nil, skipping the real bytecode
        // (seq-reduce's acc cell hit exactly this).
        if l.bc_closure {
            return None;
        }
        if !matches!(&l.arglist, Some(Value::Int(_))) {
            return None;
        }
        let items = match l.bc_items.as_ref()?.try_borrow() {
            Ok(items) => items.clone(),
            Err(_) => {
                return Some(Err(
                    self.error("Cannot inspect a mutably borrowed byte-code object"),
                ));
            }
        };
        // The argdesc and constants alone are not unique to these data
        // functions. Verify the generated-file marker and bytecode body so a
        // user-created #[...] function with an alist constant still runs in
        // the ordinary VM (including after its code has been mutated).
        if !unidata_gen_literal(&items) {
            return None;
        }
        let code_signature = unidata_code_signature(self, &items)?;
        // Only the uni-*.el literals: argdesc 257 (1 arg) or 771 (3
        // args).  A generic guard — without it any `#[...]' closure
        // whose consts happen to look like an alist (e.g.
        // `case-table-get-table's ((up . 0) ...) table) gets its
        // 1-arg calls hijacked into `assq' lookups instead of the
        // real byte code / arity error.
        let argdesc_n = match items.first() {
            Some(Value::Int(n)) => *n,
            _ => return None,
        };
        if argdesc_n != 257 && argdesc_n != 771 {
            return None;
        }
        let consts = match items.get(2) {
            Some(Value::Vec(v)) => match v.try_borrow() {
                Ok(consts) => consts.clone(),
                Err(_) => {
                    return Some(Err(
                        self.error("Cannot inspect a mutably borrowed byte-code constants vector"),
                    ));
                }
            },
            _ => return None,
        };
        let cts = self.q("char-table-extra-slot");
        if argv.len() == 3 && argdesc_n == 771 {
            let (ch, raw, tbl) = (&argv[0], &argv[1], &argv[2]);
            let Value::Int(n) = ch else { return None };
            if !crate::lisp::builtins::misc::is_char_table(self, tbl) {
                return None;
            }
            if code_signature == UNIDATA_EXTRA_SLOT_CODE
                && consts.len() == 3
                && matches!(consts[0], Value::Int(0))
                && matches!(&consts[1], Value::Sym(s) if *s == cts)
                && matches!(consts[2], Value::Int(1))
            {
                // `unidata-put-decomposition'/`unidata-put-name': flush
                // the compressed block through the slot-1 getter first,
                // then store VAL verbatim.  Their initial aref validates
                // the original integer before any native narrowing.
                if !ch.fixnump() {
                    return Some(Err(self.wrong_type_mut("fixnump", ch)));
                }
                if !(0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(n) {
                    return Some(Err(self.wrong_type_mut("characterp", ch)));
                }
                let cur =
                    crate::lisp::builtins::misc::char_table_ref(self, tbl, (*n).max(0) as usize);
                if let Value::Str(s) = &cur {
                    let marker = match s.try_borrow() {
                        Ok(contents) => contents.chars().next().is_some_and(|c| c == '\0'),
                        Err(_) => {
                            return Some(Err(
                                self.error("Cannot inspect a mutably borrowed unidata string"),
                            ));
                        }
                    };
                    if marker {
                        let _ = crate::lisp::builtins::charset::uniprop_lookup(
                            self,
                            tbl,
                            (*n).max(0) as u32,
                            cur.clone(),
                        );
                    }
                }
                crate::lisp::builtins::misc::ct_set(self, tbl, (*n).max(0) as u32, raw.clone());
                return Some(Ok(raw.clone()));
            }
            // `unidata-get-decomposition'/`unidata-get-name' & kin:
            // decode RAW through the table's slot-1 machinery (keyed on
            // the property name in slot 0).
            if UNIDATA_PROPERTY_CODE.contains(&code_signature)
                && consts
                    .iter()
                    .any(|c| matches!(c, Value::Sym(s) if *s == cts))
            {
                return Some(Ok(crate::lisp::builtins::charset::uniprop_lookup(
                    self,
                    tbl,
                    (*n).max(0) as u32,
                    raw.clone(),
                )));
            }
            return None;
        }
        if argv.len() == 1
            && argdesc_n == 257
            && code_signature == UNIDATA_DESCRIPTION_CODE
        {
            if consts.is_empty() {
                return None;
            }
            // Decomposition's mapconcat bytecode must run normally:
            // it validates sequences/elements and preserves composition
            // properties.  A matching first constant is not enough to
            // identify an arbitrary bytecode function as unidata either.
            // `unidata-describe-*' alist lookups: (cdr (assq VAL ALIST)).
            let alist = consts.first()?;
            if matches!(alist, Value::Cons(_) | Value::Vec(_)) {
                let v = argv[0].clone();
                let elems: Vec<Value> = match alist {
                    Value::Cons(_) => {
                        let mut elems = Vec::new();
                        let mut cursor = alist.clone();
                        let mut seen = FxHashSet::default();
                        loop {
                            match cursor {
                                Value::Nil => break,
                                Value::Cons(cell) => {
                                    if !seen.insert(Rc::as_ptr(&cell) as usize) {
                                        return None;
                                    }
                                    let (car, cdr) = match cell.try_borrow() {
                                        Ok(cell) => (cell.car.clone(), cell.cdr.clone()),
                                        Err(_) => {
                                            return Some(Err(self.error(
                                                "Cannot inspect a mutably borrowed unidata alist",
                                            )));
                                        }
                                    };
                                    elems.push(car);
                                    cursor = cdr;
                                }
                                _ => return None,
                            }
                        }
                        elems
                    }
                    Value::Vec(vv) => match vv.try_borrow() {
                        Ok(elems) => elems.clone(),
                        Err(_) => {
                            return Some(Err(self.error(
                                "Cannot inspect a mutably borrowed unidata vector",
                            )));
                        }
                    },
                    _ => Vec::new(),
                };
                // Require a unidata-style alist: every element a cons
                // pair `(KEY . STRING)' — uni-*.el tables map property
                // keys to description strings.  `make-closure'
                // products splice closure vars into consts[0..] and
                // arbitrary `#[257]' literals (byte-compiled lambdas
                // whose consts[0] happens to be a list, e.g. the
                // `oclosure--lambda' mapcar helper with consts[0] =
                // ((progn nil))) must fall through to the real VM.
                let mut pairs = Vec::with_capacity(elems.len());
                for element in elems {
                    let Value::Cons(pair) = element else {
                        return None;
                    };
                    let (key, value) = match pair.try_borrow() {
                        Ok(pair) => (pair.car.clone(), pair.cdr.clone()),
                        Err(_) => {
                            return Some(Err(self.error(
                                "Cannot inspect a mutably borrowed unidata alist entry",
                            )));
                        }
                    };
                    if !matches!(&value, Value::Str(_)) {
                        return None;
                    }
                    pairs.push((key, value));
                }
                for (key, value) in pairs {
                    if crate::lisp::builtins::eq_values(&key, &v) {
                        return Some(Ok(value));
                    }
                }
                return Some(Ok(Value::Nil));
            }
        }
        None
    }

    /// GNU `funcall_lambda' on a raw `(lambda ...)' cons — the
    /// `funcall'/`apply' path, which never converts the form to a
    /// closure.  Args bind dynamically (`lexenv' is nil for raw forms);
    /// `(lambda)', `(lambda . X)', `(lambda a ...)' and dotted/non-
    /// symbol arglist elements all signal `invalid-function FORM'
    /// (datum is the form itself, not a closure).  The body runs as
    /// GNU's `Fprogn (XCDR (XCDR fun))': cons elements evaluate, a
    /// non-cons tail ends evaluation silently.
    pub(super) fn call_lambda_form(&mut self, fun: &Value, argv: Argv) -> EvalResult {
        self.with_eval_context(false, |i| i.call_lambda_form_inner(fun, argv))
    }

    fn call_lambda_form_inner(&mut self, fun: &Value, argv: Argv) -> EvalResult {
        let lambda_tail = match fun {
            Value::Cons(c) => super::cons_cdr(self, c)?,
            _ => unreachable!(),
        };
        let arglist_cell = match &lambda_tail {
            Value::Cons(cell) => Rc::clone(cell),
            _ => {
                return Err(self.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]));
            }
        };
        let arglist = super::cons_car(self, &arglist_cell)?;
        // Raw lambda forms are dynamically bound; eval_progn preserves
        // GNU's treatment of dotted/atom bodies and mutations during eval.
        self.with_lambda_bindings(None, |i| {
            i.bind_lambda_walk(&arglist, &argv, fun, |i, s, v| i.specbind(s, v))?;
            // GNU fetches XCDR(XCDR(FUN)) after parameter binding. A
            // variable watcher can replace the raw lambda's body while its
            // parameters are being bound, so use the live function spine.
            let body = match fun {
                Value::Cons(cell) => match super::cons_cdr(i, cell)? {
                    Value::Cons(tail) => super::cons_cdr(i, &tail)?,
                    _ => Value::Nil,
                },
                _ => Value::Nil,
            };
            i.eval_progn(&body)
        })
    }

    /// GNU `funcall_lambda's binding loop over the RAW arglist:
    /// `&optional'/`&rest' are the only keywords; each element must be
    /// a bare symbol (`invalid-function FUN' otherwise — FUN is the
    /// closure for closures, the `(lambda ...)' cons for raw forms);
    /// a second `&optional', `&optional' after `&rest', a second
    /// `&rest', a bare trailing keyword, or a non-nil dotted tail all
    /// signal `invalid-function FUN'; too few required args signal
    /// `wna FUN N' mid-walk, too many after it.  `bind' is either
    /// specbind (dynamic) or frame binding (lexical — GNU binds EVERY
    /// param into the closure env, specials included).
    fn bind_lambda_walk(
        &mut self,
        arglist: &Value,
        argv: &[Value],
        fun: &Value,
        mut bind: impl FnMut(&mut Interp, SymId, Value) -> Result<(), Flow>,
    ) -> Result<(), Flow> {
        let inv = |i: &Interp| -> Flow { i.signal_data(sym::INVALID_FUNCTION, vec![fun.clone()]) };
        let wna = |i: &Interp, n: usize| -> Flow {
            i.signal_data(
                sym::WRONG_NUMBER_OF_ARGUMENTS,
                vec![fun.clone(), Value::Int(n as i128)],
            )
        };
        let opt_kw = sym::OPTIONAL;
        let rest_kw = sym::REST;
        let mut syms = arglist.clone();
        let mut arg_i = 0usize;
        let mut until_quit = 4096usize;
        let mut optional = false;
        let mut rest = false;
        let mut previous_rest = false;
        let mut seen_states = FxHashSet::default();
        loop {
            let current_cell = match &syms {
                Value::Cons(cell) => Rc::clone(cell),
                _ => break,
            };
            // A circular argument list with optional parameters can
            // otherwise bind nil forever. Include the binding state in the
            // key: ordinary required-argument cycles still reach GNU's
            // arity error as supplied arguments run out.
            let state = (
                Rc::as_ptr(&current_cell) as usize,
                arg_i,
                optional,
                rest,
                previous_rest,
            );
            if !seen_states.insert(state) {
                return Err(inv(self));
            }
            let (elt, _) = super::cons_parts(self, &current_cell)?;
            let s = match self.sym_id(&elt) {
                Some(s) => s,
                None => return Err(inv(self)),
            };
            if s == rest_kw {
                if rest || previous_rest {
                    return Err(inv(self));
                }
                rest = true;
                previous_rest = true;
            } else if s == opt_kw {
                if optional || rest || previous_rest {
                    return Err(inv(self));
                }
                optional = true;
            } else {
                let arg = if rest {
                    // GNU `Flist (nargs - i, &arg_vector[i])'.
                    let mut v = Value::Nil;
                    let mut until_quit = 4096usize;
                    for value in argv[arg_i..].iter().rev() {
                        v = Value::cons(value.clone(), v);
                        until_quit -= 1;
                        if until_quit == 0 {
                            self.maybe_quit()?;
                            until_quit = 4096;
                        }
                    }
                    arg_i = argv.len();
                    v
                } else if arg_i < argv.len() {
                    let v = argv[arg_i].clone();
                    arg_i += 1;
                    v
                } else if !optional {
                    return Err(wna(self, argv.len()));
                } else {
                    Value::Nil
                };
                bind(self, s, arg)?;
                previous_rest = false;
            }
            // `specbind' can run variable watchers. GNU reads the current
            // cell's cdr after binding its parameter, so a watcher that
            // mutates the raw lambda arglist changes which parameter is
            // visited next.
            syms = super::cons_cdr(self, &current_cell)?;
            until_quit -= 1;
            if until_quit == 0 {
                self.maybe_quit()?;
                until_quit = 4096;
            }
        }
        if !syms.is_nil() || previous_rest {
            return Err(inv(self));
        }
        if arg_i < argv.len() {
            return Err(wna(self, argv.len()));
        }
        Ok(())
    }

    fn call_lambda_inner(&mut self, l: &Rc<Lambda>, argv: Argv, shown: &Value) -> EvalResult {
        let items = match &l.bc_items {
            Some(items) => {
                Some(items.try_borrow().map(|items| items.clone()).map_err(|_| {
                    self.error("Cannot inspect a mutably borrowed byte-code object")
                })?)
            }
            None => None,
        };
        let is_bytecode = items
            .as_ref()
            .is_some_and(|items| crate::lisp::bytecode::is_byte_code(items));
        if let Some(items) = &items {
            if let Some(r) = self.unidata_bc_dispatch(l, &argv) {
                return r;
            }
            // `#[ARGDESC ...]' shape → the byte-code interpreter (also
            // reached by `#[...]' literals read from `.elc' files).
            // GNU's packed integer argdescs report the descriptor, while
            // list-argdesc user byte-code functions report the function
            // object even when anonymous.
            if is_bytecode {
                // During macro expansion, compiled-equivalent expanders
                // retain the packed descriptor; direct nativecomp calls use
                // the function object expected by the subr surface.
                let named = self.lambda_name(l).is_some() || (l.compiled && !is_bytecode);
                let funv = Value::Lambda(l.clone());
                let argdesc = items.first().cloned();
                let list_argdesc = matches!(&argdesc, Some(Value::Nil | Value::Cons(_)));
                let parsed = argdesc
                    .as_ref()
                    .map(|argdesc| crate::lisp::bytecode::parse_argdesc(self, argdesc));
                let malformed_argdesc = parsed
                    .as_ref()
                    .is_some_and(|parsed| parsed.invalid_at.is_some() || parsed.req == usize::MAX);
                let who = if !l.compiled && list_argdesc {
                    Some(&funv)
                } else if !l.compiled {
                    None
                } else if self.macroexp_call_depth == 0 && (named || malformed_argdesc) {
                    if named { Some(&funv) } else { Some(shown) }
                } else {
                    None
                };
                return crate::lisp::bytecode::exec(self, l, argv, who, named);
            }
        }
        // GNU's non-byte-code `funcall_lambda' path reports the function
        // object. Named bundled nativecomp closures use their subr-shaped
        // function object; byte-code objects are handled above.
        let fun = if self.lambda_name(l).is_some() || (l.compiled && !is_bytecode) {
            Value::Lambda(l.clone())
        } else {
            match &items {
                Some(items) if is_bytecode => match items.first() {
                    Some(Value::Int(desc)) => {
                        let parsed = crate::lisp::bytecode::parse_argdesc(self, &Value::Int(*desc));
                        Value::cons(
                            Value::Int(parsed.req as i128),
                            Value::Int(parsed.max as i128),
                        )
                    }
                    _ => Value::Lambda(l.clone()),
                },
                _ => Value::Lambda(l.clone()),
            }
        };
        // Interpreted `#[...]' closures retain their raw slots as the
        // authoritative function object. Reader circle fixups can replace
        // a top-level slot while the parsed Lambda fields remain stale.
        let interpreted_items = items
            .as_ref()
            .filter(|items| items.len() >= 3 && !is_bytecode);
        let arglist = interpreted_items
            .and_then(|items| items.first().cloned())
            .or_else(|| items.as_ref().and_then(|items| items.first().cloned()))
            .or_else(|| l.arglist.clone())
            .unwrap_or(Value::Nil);
        let body_value = interpreted_items
            .and_then(|items| items.get(1).cloned())
            .or_else(|| l.body_value.clone());
        let env = interpreted_items
            .map(|items| crate::lisp::eval::lexenv_from_value(&items[2]))
            .unwrap_or_else(|| l.env.clone());

        if env.is_some() {
            // Lexical closure: GNU's `funcall_lambda' pushes EVERY
            // `(param . arg)' pair onto the closure env — `defvar'd
            // specials, scoped-declared names, even nil/t — since
            // `internal-interpreter-environment' is not consulted here.
            let frame = lexenv_extend_checked(&env).map_err(|error| error.into_flow(self))?;
            self.with_lambda_bindings(Some(frame.clone()), |i| {
                i.bind_lambda_walk(&arglist, &argv, &fun, |_i, s, v| {
                    lexframe_bind_checked(&frame, s, v).map_err(|error| error.into_flow(_i))?;
                    Ok(())
                })?;
                match &body_value {
                    Some(body) => i.eval_progn(body),
                    None => i.eval_body(&l.body),
                }
            })
        } else {
            // Dynamic functions cannot see the caller's lexical variables.
            self.with_lambda_bindings(None, |i| {
                i.bind_lambda_walk(&arglist, &argv, &fun, |i, s, v| i.specbind(s, v))?;
                match &body_value {
                    Some(body) => i.eval_progn(body),
                    None => i.eval_body(&l.body),
                }
            })
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

    /// GNU Ffunction's lambda constructor protocol. Keep the raw parts
    /// intact for a user-supplied constructor; its result need not itself
    /// be a function. Native validation belongs to the fallback constructor.
    pub fn function_from_lambda(&mut self, cdr: &Value) -> EvalResult {
        let (args, mut body) = match cdr {
            Value::Nil => (Value::Nil, Value::Nil),
            Value::Cons(cell) => super::cons_parts(self, cell)?,
            other => return Err(self.wrong_type_mut("listp", other)),
        };
        let mut docstring = Value::Nil;
        let mut iform = Value::Nil;
        if let Value::Cons(cell) = &body {
            let (first, rest) = super::cons_parts(self, cell)?;
            if matches!(first, Value::Str(_)) && !rest.is_nil() {
                docstring = first;
                body = rest;
            } else if let Value::Cons(form) = first {
                let (head, tail) = super::cons_parts(self, &form)?;
                if matches!(head, Value::Sym(sym) if self.intern_soft(":documentation") == Some(sym))
                {
                    let (expression, site) = match tail {
                        Value::Cons(cell) => (
                            super::cons_car(self, &cell)?,
                            (Rc::as_ptr(&cell) as usize as u128) << 1,
                        ),
                        Value::Nil => (Value::Nil, 0),
                        other => return Err(self.wrong_type_mut("listp", &other)),
                    };
                    docstring = self.eval_at_compiled_site(&expression, site)?;
                    body = rest;
                }
            }
        }
        if let Value::Cons(cell) = &body {
            let (first, rest) = super::cons_parts(self, cell)?;
            let is_interactive = if let Value::Cons(form) = &first {
                self.sym_is(&super::cons_car(self, form)?, sym::INTERACTIVE)
            } else {
                false
            };
            if is_interactive {
                iform = first;
                body = rest;
            }
        }
        self.function_from_parts(args, body, docstring, iform)
    }

    pub fn function_from_parts(
        &mut self,
        args: Value,
        mut body: Value,
        docstring: Value,
        iform: Value,
    ) -> EvalResult {
        if body.is_nil() {
            body = Value::list(vec![Value::Nil]);
        }
        let constructor_id = self.q("internal-make-interpreted-closure-function");
        let constructor = self.symbol_value(constructor_id);
        let env = lexenv_as_value_checked(&self.lexenv).map_err(|error| error.into_flow(self))?;
        let arguments = vec![args, body, env.clone(), docstring, iform];
        let mut value = if !env.is_nil() && constructor.truthy() {
            self.apply(&constructor, arguments)?
        } else {
            crate::lisp::builtins::misc::f_make_interpreted_closure(self, arguments.into())?
        };
        // Keep the port's nativecomp metadata on closures created by a
        // compiled body or during bootstrap, including constructors reached
        // via cconv. A compiled ancestor can run source `eval', so use the
        // immediate body's provenance rather than its call depth. Never
        // rewrite a shared object returned by a custom constructor.
        if let Value::Lambda(lambda) = &mut value {
            crate::lisp::memory::update_unique_lambda(lambda, |lambda| {
                lambda.compiled |= self.loading_dumped || self.current_body_compiled;
                lambda.dumped_doc |= self.loading_dumped;
                lambda.plain = self.explicit_eval_depth > 0;
            });
        }
        Ok(value)
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
        let mut bad_arglist = false;
        let mut rest = None;
        // GNU's `funcall_lambda' rest/optional state: `&rest' seen but
        // its parameter not yet bound (`rest_seen && rest.is_none()' =
        // GNU's `previous_rest').
        let mut rest_seen = false;
        let mut mode = 0u8; // 0 = required, 1 = optional, 2 = post-&rest
        let mut seen_params = FxHashSet::default();
        let mut until_quit = 1024usize;
        // GNU's `funcall_lambda' only knows `&optional'/`&rest' — every
        // other `&'-prefixed name (`&body', `&key', `&whole', ...) is an
        // ordinary parameter symbol.  A second keyword, `&optional'
        // after `&rest', or a bare `&rest'/`&optional' at the tail makes
        // the arglist call-invalid rather than a parse error.
        let plist = params;
        let mut cur = plist.clone();
        loop {
            // Clone for the match: the `other' arm compares `cur' to
            // `plist' after the match scrutinee consumes the clone.
            match cur.clone() {
                Value::Nil => break,
                Value::Cons(c) => {
                    // The raw arglist remains attached to the Lambda and
                    // the call walker decides whether a cyclic shape is
                    // callable. Stop parsing after one pass so closure
                    // construction cannot loop on user-mutated lists.
                    if !seen_params.insert(Rc::as_ptr(&c) as usize) {
                        bad_arglist = true;
                        break;
                    }
                    let (car, next) = super::cons_parts(self, &c)?;
                    match self.sym_id(&car) {
                        Some(s) if s == sym::OPTIONAL => {
                            // GNU: `&optional' after `&rest' or a
                            // second `&optional' → call-invalid.
                            if mode != 0 {
                                bad_arglist = true;
                            }
                            mode = 1;
                        }
                        Some(s) if s == sym::REST => {
                            // GNU: a second `&rest' → call-invalid.
                            // `rest_seen' latches on the keyword itself
                            // so a bare trailing `&rest' is invalid.
                            if rest_seen {
                                bad_arglist = true;
                            }
                            rest_seen = true;
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
                                    required.push(self.q("&bad-param"));
                                    bad_arglist = true;
                                }
                            }
                            1 => {
                                // `sym` or `(sym default)` — the extended
                                // form only works via macros/cl- arglists;
                                // calling a plain function with it signals
                                // invalid-function.
                                match self.sym_id(&car) {
                                    Some(s) => optional.push(crate::lisp::value::OptParam {
                                        sym: s,
                                        default: None,
                                        supplied: None,
                                    }),
                                    None => {
                                        if let Value::Cons(_) = car {
                                            let pair = car.list_to_vec().unwrap_or_default();
                                            if let Some(s) =
                                                pair.first().and_then(|v| self.sym_id(v))
                                            {
                                                optional.push(crate::lisp::value::OptParam {
                                                    sym: s,
                                                    default: pair.get(1).cloned(),
                                                    supplied: pair
                                                        .get(2)
                                                        .and_then(|v| self.sym_id(v)),
                                                });
                                                bad_arglist = true;
                                            } else {
                                                // GNU validates each raw
                                                // parameter when the
                                                // function is called. Keep
                                                // construction successful;
                                                // bind_lambda_walk will
                                                // signal invalid-function
                                                // with the closure object.
                                                bad_arglist = true;
                                            }
                                        } else {
                                            bad_arglist = true;
                                        }
                                    }
                                }
                            }
                            _ => {
                                // GNU `funcall_lambda': after `&rest'
                                // consumed the arg list (i = nargs),
                                // further symbols bind nil; a non-symbol
                                // element is call-invalid.
                                if rest.is_none() {
                                    if let Some(s) = self.sym_id(&car) {
                                        rest = Some(s);
                                    } else {
                                        // `(&rest 5)': invalid at call.
                                        bad_arglist = true;
                                    }
                                } else if self.sym_id(&car).is_none() {
                                    // `(&rest a 5)': invalid at call.
                                    bad_arglist = true;
                                }
                                // Post-`&rest' symbols need no slot:
                                // GNU binds them nil at call time
                                // (`i = nargs' fast path), which the
                                // raw-arglist call walk reproduces.
                            }
                        },
                    }
                    cur = next;
                    until_quit -= 1;
                    if until_quit == 0 {
                        self.maybe_quit()?;
                        until_quit = 1024;
                    }
                }
                // A dotted tail: GNU's `funcall_lambda' rejects ANY
                // non-nil tail at call time (`!NILP (syms_left)' →
                // invalid-function), symbol tails like `(a . b)'
                // included.  The closure still builds; the arglist
                // prints verbatim.
                Value::Sym(_) => {
                    bad_arglist = true;
                    break;
                }
                _ => {
                    // `cur == plist' only when the ARGLIST itself is
                    // this atom: internal-make-closure asserts
                    // `(listp args)' → `cl-assertion-failed'.
                    if crate::lisp::builtins::eq_values(&cur, plist) {
                        let assert_id = self.q("cl-assertion-failed");
                        let listp_id = self.q("listp");
                        let args_id = self.q("args");
                        return Err(self.signal_data(
                            assert_id,
                            vec![Value::list(vec![Value::Sym(listp_id), Value::Sym(args_id)])],
                        ));
                    }
                    // Mid-list non-symbol tail `(a . 2)': call-invalid.
                    bad_arglist = true;
                    break;
                }
            }
        }
        // GNU's `previous_rest' end state: a `&rest' keyword with no
        // following parameter name is call-invalid.
        if rest_seen && rest.is_none() {
            bad_arglist = true;
        }

        // Extract docstring and interactive spec from the body front.
        let mut doc = None;
        let mut doc_value = None;
        let mut interactive = None;
        let mut start = 0;
        if let Some(Value::Str(s)) = body.first() {
            if body.len() > 1 {
                doc = Some(
                    s.try_borrow()
                        .map(|text| (*text).clone())
                        .map_err(|_| self.error("Cannot inspect a mutably borrowed lambda docstring"))?,
                );
                doc_value = Some(body.first().unwrap().clone());
                start = 1;
            }
        } else if let Some(Value::Cons(c)) = body.first() {
            // GNU `Flambda' handles `(:documentation <form>)' as a
            // computed docstring: the form is evaluated at lambda
            // construction and consumed (eval.c).  The result is the
            // closure's docstring slot — any value (oclosure.el stores
            // its type symbol there).
            let (head, tail) = super::cons_parts(self, c)?;
            if matches!(&head, Value::Sym(s)
                if self.intern_soft(":documentation") == Some(*s))
            {
                let (form, site) = match tail {
                    Value::Cons(args) => (
                        super::cons_car(self, &args)?,
                        (Rc::as_ptr(&args) as usize as u128) << 1,
                    ),
                    _ => (Value::Nil, 0),
                };
                let v = self.eval_at_compiled_site(&form, site)?;
                if let Value::Str(s) = &v {
                    doc = Some(
                        s.try_borrow()
                            .map(|text| (*text).clone())
                            .map_err(|_| {
                                self.error("Cannot inspect a mutably borrowed lambda docstring")
                            })?,
                    );
                }
                doc_value = Some(v);
                start = 1;
            }
        }
        for (i, f) in body.iter().enumerate().skip(start) {
            if let Value::Cons(c) = f {
                let (head, _) = super::cons_parts(self, c)?;
                if self.sym_is(&head, sym::INTERACTIVE) {
                    interactive = Some(f.clone());
                    start = i + 1;
                    break;
                }
                // `(declare ...)' specs are a `defun'/`defmacro'
                // facility — `macroexp-parse-body' strips them there.
                // A bare `lambda' has no declaration handling (GNU
                // `Ffunction' keeps them in the body).
                if name.is_some()
                    && self.sym_is(&head, self.q_soft("declare").unwrap_or(SymId::MAX))
                {
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
            body_value: None,
            env: self.lambda_env_checked()?,
            doc,
            interactive,
            name: name.map(|s| self.symbol_name_text(s)),
            bad_arglist,
            arglist: Some(params.clone()),
            plain: self.explicit_eval_depth > 0,
            dumped_doc: self.loading_dumped,
            compiled: self.loading_dumped || self.current_body_compiled,
            advice_link: None,
            bc_items: None,
            bc_closure: false,
            bc_prog: std::cell::RefCell::new(None),
            compiled_macro_cache: crate::lisp::value::CompiledMacroCache::default(),
            doc_value,
            env_value: None,
        })
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

#[cfg(test)]
mod circular_arglist_tests {
    use super::*;
    use crate::lisp::obarray::Obarray;

    #[test]
    fn circular_optional_lambda_arglist_signals_and_unwinds() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let mut interp = Interp::new();
                let variable = interp.intern("qa-circular-lambda-variable");
                let arglist = Value::list(vec![
                    Value::Sym(variable),
                    Value::Sym(sym::OPTIONAL),
                    Value::Sym(interp.intern("qa-circular-lambda-optional")),
                ]);
                let third = match &arglist {
                    Value::Cons(first) => match &first.borrow().cdr {
                        Value::Cons(second) => second.borrow().cdr.clone(),
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                };
                if let Value::Cons(cell) = &third {
                    cell.borrow_mut().cdr = third.clone();
                } else {
                    unreachable!();
                }
                let function = Value::cons(
                    Value::Sym(sym::LAMBDA),
                    Value::cons(arglist, Value::list(vec![Value::Sym(sym::T)])),
                );
                let depth = interp.specbind_depth();
                let result = interp.apply(&function, vec![Value::Int(1)]);
                assert!(matches!(
                    result,
                    Err(Flow::Signal(ref signal))
                        if signal.is_symbol(sym::INVALID_FUNCTION)
                ));
                assert_eq!(interp.specbind_depth(), depth);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn parsing_a_required_parameter_cycle_stops_after_one_pass() {
        let mut interp = Interp::vacant();
        interp.obarray = Obarray::new();
        let variable = interp.intern("qa-circular-required-variable");
        let params = Value::cons(Value::Sym(variable), Value::Nil);
        if let Value::Cons(cell) = &params {
            cell.borrow_mut().cdr = params.clone();
        }
        let lambda = interp
            .parse_lambda(&params, &[Value::Sym(sym::T)], None)
            .unwrap();
        assert!(lambda.bad_arglist);
        assert_eq!(lambda.required, vec![variable]);
    }
}
