//! Miscellaneous subrs: symbols, time values, hashing/crypto, file
//! attributes, environment, and small editor glue that doesn't belong
//! to a larger category module.

use std::cell::RefCell;
use std::rc::Rc;

use super::{S, arg, want_string};
use crate::lisp::Interp;
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{Arity, Lambda, Subr, Value};

pub(crate) static SUBRS: &[Subr] = &[
    S!("gensym", 0, 1, f_gensym, "New uninterned symbol gN."),
    S!(
        "func-arity",
        1,
        1,
        f_func_arity,
        "Return (MIN . MAX) arity of FUNCTION."
    ),
    S!(
        "subr-arity",
        1,
        1,
        f_func_arity,
        "Return (MIN . MAX) arity of subr."
    ),
    S!("closurep", 1, 1, f_closurep, "t if OBJECT is a closure."),
    S!(
        "interpreted-function-p",
        1,
        1,
        f_interpreted_function_p,
        "t if FUNCTION is interpreted."
    ),
    S!(
        "make-interpreted-closure",
        3,
        3,
        f_make_interpreted_closure,
        "Build a closure from args/env/body."
    ),
    S!(
        "obarray-clear",
        0,
        1,
        f_obarray_clear,
        "Empty the obarray (keeps core syms)."
    ),
    S!(
        "getenv-internal",
        1,
        2,
        f_getenv_internal,
        "Environment variable value."
    ),
    S!(
        "command-modes",
        1,
        1,
        f_command_modes,
        "Modes a command applies to (nil)."
    ),
    S!(
        "abort-minibuffers",
        0,
        0,
        f_abort_minibuffers,
        "Abort any active minibuffer."
    ),
    S!(
        "accessible-keymaps",
        1,
        2,
        f_accessible_keymaps,
        "List (PREFIX . KEYMAP) reachable from MAP."
    ),
    S!(
        "map-keymap",
        2,
        3,
        f_map_keymap,
        "Call FUNCTION on each binding in KEYMAP."
    ),
    S!("map-keymap-internal", 2, 2, f_map_keymap, ""),
    S!(
        "keymap--get-keyelt",
        2,
        2,
        f_keymap_get_keyelt,
        "Return (BINDING . DEF) for OBJECT."
    ),
    S!(
        "describe-buffer-bindings",
        1,
        3,
        f_describe_bindings,
        "Print key bindings of BUFFER."
    ),
    S!(
        "set--this-command-keys",
        1,
        1,
        f_set_this_command_keys,
        "Set this-command-keys (stub)."
    ),
    S!(
        "documentation-stringp",
        1,
        1,
        f_documentation_stringp,
        "t if OBJECT is a docstring."
    ),
    S!(
        "error-message-string",
        1,
        1,
        f_error_message_string,
        "Format an error data list."
    ),
    S!(
        "external-debugging-output",
        1,
        1,
        f_external_debugging_output,
        "Write CHAR to stderr."
    ),
    S!(
        "open-dribble-file",
        1,
        1,
        f_open_dribble_file,
        "Record keystrokes to FILE (stub)."
    ),
    S!(
        "open-termscript",
        1,
        1,
        f_open_termscript,
        "Record terminal output to FILE (stub)."
    ),
    S!(
        "send-string-to-terminal",
        1,
        2,
        f_send_string_to_terminal,
        "Send STRING to the terminal."
    ),
    S!(
        "flush-standard-output",
        0,
        0,
        f_flush_stdout,
        "Flush stdout."
    ),
    S!(
        "encode-time",
        0,
        9,
        f_encode_time,
        "Convert time components to Lisp time."
    ),
    S!(
        "decode-time",
        0,
        3,
        f_decode_time,
        "Decompose Lisp time into components."
    ),
    S!("time-add", 2, 2, f_time_add, "Add two Lisp time values."),
    S!(
        "time-subtract",
        2,
        2,
        f_time_subtract,
        "Subtract two Lisp time values."
    ),
    S!("time-less-p", 2, 2, f_time_less_p, "t if TIME1 < TIME2."),
    S!("time-equal-p", 2, 2, f_time_equal_p, "t if TIME1 == TIME2."),
    S!(
        "time-convert",
        1,
        3,
        f_time_convert,
        "Convert TIME to FORM ticks."
    ),
    S!("emacs-uptime", 0, 1, f_emacs_uptime, "Process uptime."),
    S!(
        "current-cpu-time",
        0,
        0,
        f_current_cpu_time,
        "CPU time used by this process."
    ),
    S!(
        "set-time-zone-rule",
        1,
        1,
        f_set_time_zone,
        "Set TZ (returns t)."
    ),
    S!(
        "secure-hash",
        2,
        5,
        f_secure_hash,
        "Cryptographic hash of OBJECT."
    ),
    S!("md5", 1, 5, f_md5, "MD5 hash of OBJECT."),
    S!(
        "base64-encode-string",
        1,
        2,
        f_b64_encode_string,
        "Base64 encode STRING."
    ),
    S!(
        "base64-decode-string",
        1,
        2,
        f_b64_decode_string,
        "Base64 decode STRING."
    ),
    S!(
        "buffer-hash",
        0,
        1,
        f_buffer_hash,
        "Hash of buffer contents."
    ),
    S!(
        "make-temp-file-internal",
        4,
        4,
        f_make_temp_file_internal,
        "Create a temp file."
    ),
    S!(
        "make-symbolic-link",
        2,
        3,
        f_make_symbolic_link,
        "Create symlink FILENAME -> TARGET."
    ),
    S!(
        "directory-name-p",
        1,
        1,
        f_directory_name_p,
        "t if NAME ends in a slash."
    ),
    S!(
        "file-name-case-insensitive-p",
        1,
        1,
        f_file_name_case_insensitive_p,
        "t on case-insensitive FS."
    ),
    S!(
        "file-attributes-lessp",
        2,
        2,
        f_file_attributes_lessp,
        "t if ATTRS1 < ATTRS2 (mtime)."
    ),
    S!(
        "set-file-times",
        1,
        3,
        f_set_file_times,
        "Set file times (stub)."
    ),
    S!("file-acl", 1, 1, f_nil, "ACL list (unsupported)."),
    S!(
        "get-file-buffer",
        1,
        1,
        f_get_file_buffer,
        "Buffer visiting FILENAME."
    ),
    S!("unlock-file", 1, 1, f_nil, "Unlock FILE (no-op)."),
    S!("lock-file", 1, 1, f_nil, "Lock FILE (no-op)."),
    S!(
        "bare-symbol",
        1,
        1,
        f_bare_symbol,
        "Symbol without position info."
    ),
    S!(
        "bare-symbol-p",
        1,
        1,
        f_bare_symbol_p,
        "t if OBJECT is a symbol without position."
    ),
    S!(
        "position-symbol",
        2,
        2,
        f_position_symbol,
        "Symbol with position (ignored)."
    ),
    S!(
        "remove-pos-from-symbol",
        1,
        1,
        f_bare_symbol,
        "Strip position (ignored)."
    ),
    S!(
        "symbol-with-pos-p",
        1,
        1,
        f_false,
        "t if OBJECT is a positioned symbol."
    ),
    S!(
        "symbol-with-pos-pos",
        1,
        1,
        f_nil,
        "Position of a positioned symbol."
    ),
    S!(
        "internal-make-var-non-special",
        1,
        1,
        f_make_var_non_special,
        "Clear SPECIAL flag."
    ),
    S!(
        "special-variable-p",
        1,
        1,
        f_special_variable_p,
        "t if SYMBOL is special."
    ),
];

// ---------- symbols / functions ----------

fn f_gensym(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Emacs: PREFIX is a string or number (a number becomes the
    // numeric prefix of the name, e.g. (gensym 5) -> symbol "5N").
    let prefix = match args.get(0) {
        Some(Value::Str(s)) => s.borrow().clone(),
        Some(Value::Int(n)) => n.to_string(),
        _ => "g".to_string(),
    };
    Ok(Value::Sym(i.obarray.gensym(&prefix)))
}

fn f_func_arity(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let fun = i.indirect_function_value(&args[0]);
    let arity = match &fun {
        Value::Subr(s) => s.arity,
        Value::Lambda(l) => l.arity(),
        _ => return Err(i.signal_data(sym::VOID_FUNCTION, vec![args[0].clone()])),
    };
    let (min, max) = match arity {
        Arity::Range { min, max } => (min, Value::Int(max as i128)),
        Arity::Many { min } => (min, Value::Sym(i.intern("many"))),
        Arity::Unevalled => (0, Value::Sym(i.intern("unevalled"))),
    };
    Ok(Value::cons(Value::Int(min as i128), max))
}

fn f_closurep(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

fn f_interpreted_function_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Lambda(_))))
}

fn f_make_interpreted_closure(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (make-interpreted-closure ARGS BODY ENV) → Lambda value.
    // Parse ARGS (a list arglist) into required/optional/rest.
    let arglist = args[0].list_to_vec().unwrap_or_default();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut rest = None;
    let mut mode = 0; // 0 req, 1 opt, 2 rest
    let opt_sym = i.intern("&optional");
    let rest_sym = i.intern("&rest");
    for a in &arglist {
        if let Some(id) = i.sym_id(a) {
            if id == opt_sym {
                mode = 1;
                continue;
            }
            if id == rest_sym {
                mode = 2;
                continue;
            }
            if mode == 2 {
                rest = Some(id);
                continue;
            }
            if mode == 0 {
                required.push(id);
            } else {
                optional.push(crate::lisp::value::OptParam {
                    sym: id,
                    default: None,
                });
            }
        }
    }
    let body = args[1].list_to_vec().unwrap_or_default();
    let env = match &args[2] {
        Value::Nil => None,
        // An env value is a list of binding alists — model as a flat
        // alist lexical frame.
        alist => {
            let mut vars = std::collections::HashMap::new();
            let mut cur = alist.clone();
            // Peel off a possible outer context list.
            loop {
                let next = match &cur {
                    Value::Cons(c) => {
                        let b = c.borrow();
                        let elem = b.car.clone();
                        let rest = b.cdr.clone();
                        // Elements may be (sym . val) conses or bare syms.
                        if let Value::Cons(p) = &elem {
                            let pb = p.borrow();
                            if let Some(sid) = i.sym_id(&pb.car) {
                                vars.insert(sid, pb.cdr.clone());
                            }
                        }
                        rest
                    }
                    _ => Value::Nil,
                };
                if matches!(next, Value::Nil) {
                    break;
                }
                cur = next;
            }
            Some(Rc::new(crate::lisp::LexFrame {
                vars: RefCell::new(vars),
                parent: None,
            }))
        }
    };
    Ok(Value::Lambda(Rc::new(Lambda {
        is_macro: false,
        required,
        optional,
        rest,
        body,
        env,
        doc: None,
        interactive: None,
        name: None,
        bad_arglist: false,
    })))
}

fn f_obarray_clear(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Symbol ids are positional across the whole runtime (buffer locals,
    // specbind), so actually clearing the obarray would corrupt state.
    // Treat as a no-op that returns its argument.
    Ok(arg(&args, 0))
}

fn f_getenv_internal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    match std::env::var(&name) {
        Ok(v) => Ok(Value::string(v)),
        Err(_) => Ok(Value::Nil),
    }
}

fn f_command_modes(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = args;
    Ok(Value::Nil)
}

fn f_abort_minibuffers(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn keymap_of(i: &mut Interp, v: &Value) -> Result<Option<Vec<Value>>, Flow> {
    // Our keymaps are lists whose car is the symbol `keymap` and whose
    // cdr is an alist of (KEY . DEF) or sparse vectors. Return pairs.
    match v {
        Value::Cons(c) => {
            let (head, tail) = {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if i.sym_id(&head) != i.intern_soft("keymap") {
                return Ok(None);
            }
            let mut pairs = Vec::new();
            let mut cur = tail;
            loop {
                match cur {
                    Value::Nil => break,
                    Value::Cons(cc) => {
                        let (elem, next) = {
                            let b = cc.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        pairs.push(elem);
                        cur = next;
                    }
                    _ => break,
                }
            }
            Ok(Some(pairs))
        }
        _ => Ok(None),
    }
}

fn f_accessible_keymaps(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let prefix = arg(&args, 1);
    let _ = prefix;
    match keymap_of(i, &args[0])? {
        Some(pairs) => {
            let out: Vec<Value> = pairs
                .into_iter()
                .map(|p| Value::cons(Value::Nil, p))
                .collect();
            Ok(Value::list(out))
        }
        None => Err(i.wrong_type_mut("keymapp", &args[0])),
    }
}

fn f_map_keymap(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match keymap_of(i, &args[1])? {
        Some(pairs) => {
            for p in pairs {
                let (k, def) = match &p {
                    Value::Cons(c) => {
                        let b = c.borrow();
                        (b.car.clone(), b.cdr.clone())
                    }
                    _ => continue,
                };
                let fnv = args[0].clone();
                let _ = i.apply(&fnv, vec![k, def]);
            }
            Ok(Value::Nil)
        }
        None => Err(i.wrong_type_mut("keymapp", &args[1])),
    }
}

fn f_keymap_get_keyelt(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Cons(_) => Ok(args[0].clone()),
        _ => Ok(Value::Nil),
    }
}

fn f_describe_bindings(i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    i.write_output("Key bindings not implemented\n");
    Ok(Value::Nil)
}

fn f_set_this_command_keys(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_documentation_stringp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Str(_))))
}

fn f_error_message_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Mirrors print_error_message in print.c.
    // Fast path: (error STRING) → STRING.
    if let Value::Cons(c) = &args[0] {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(d) = &cdr {
            let (d1, drest) = {
                let b = d.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if i.sym_id(&car) == Some(sym::ERROR) && matches!(d1, Value::Str(_)) && drest.is_nil() {
                return Ok(d1);
            }
        }
    }
    Ok(Value::string(error_message(i, &args[0])))
}

/// Format an error object `(SYMBOL . DATA)` the way Emacs's
/// `print_error_message` does.
pub fn error_message(i: &mut Interp, obj: &Value) -> String {
    let (errname, data) = match obj {
        Value::Cons(c) => {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        }
        _ => (obj.clone(), Value::Nil),
    };
    let errname_id = i.sym_id(&errname);
    let (errmsg, file_error, tail_list): (Value, bool, Vec<Value>);
    if errname_id == Some(sym::ERROR) {
        // For `error`, the first data item is the message.
        let items = data.list_to_vec().unwrap_or_default();
        errmsg = items.first().cloned().unwrap_or(Value::Nil);
        file_error = false;
        tail_list = items.get(1..).map(|s| s.to_vec()).unwrap_or_default();
    } else {
        let mut msg = Value::Nil;
        let mut ferror = false;
        if let Some(id) = errname_id {
            let plist = i.obarray.symbol(id).plist.clone();
            let em_id = i.intern("error-message");
            msg = crate::lisp::eval::plist_get(&plist, em_id);
            let ec_id = i.intern("error-conditions");
            let conds = crate::lisp::eval::plist_get(&plist, ec_id);
            let fe_id = i.intern("file-error");
            ferror = conds
                .list_to_vec()
                .unwrap_or_default()
                .iter()
                .any(|c| i.sym_id(c) == Some(fe_id));
        }
        errmsg = msg;
        file_error = ferror;
        tail_list = data.list_to_vec().unwrap_or_default();
    }
    let (errmsg, tail): (Value, &[Value]) = if file_error && !tail_list.is_empty() {
        // file-error: first data item is the message string.
        (tail_list[0].clone(), &tail_list[1..])
    } else {
        (errmsg, &tail_list[..])
    };
    // quote-curve the message text (substitute-command-keys applies
    // text-quoting-style 'curve to doc/error strings).
    let msg_text = match &errmsg {
        Value::Str(s) => Some(curve_quotes(&s.borrow())),
        _ => None,
    };
    let mut out = String::new();
    let mut sep: Option<&str> = Some(": ");
    match &msg_text {
        None => out.push_str("peculiar error"),
        Some(t) if t.is_empty() => sep = None,
        Some(t) => out.push_str(t),
    }
    let princ_mode =
        file_error || errname_id == Some(sym::END_OF_FILE) || errname_id == Some(sym::USER_ERROR);
    for item in tail {
        if let Some(s) = sep {
            out.push_str(s);
        }
        sep = Some(", ");
        if princ_mode {
            out.push_str(&i.princ_to_string(item));
        } else {
            out.push_str(&i.prin1_to_string(item));
        }
    }
    out
}

/// Apply Emacs's default `text-quoting-style` (curve) to a message
/// template: `'` after a word char → `’`, before a word char → `‘`;
/// `` ` `` before a word char → `‘`.
fn curve_quotes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (k, &c) in chars.iter().enumerate() {
        match c {
            '\'' => {
                let prev = if k > 0 {
                    chars.get(k - 1).copied()
                } else {
                    None
                };
                out.push(
                    if prev
                        .map(|p| p.is_alphanumeric() || p == '\'')
                        .unwrap_or(false)
                    {
                        '\u{2019}'
                    } else {
                        '\u{2018}'
                    },
                );
            }
            '`' => out.push('\u{2018}'),
            _ => out.push(c),
        }
    }
    out
}

fn f_external_debugging_output(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Some(c) = args[0].int() {
        eprint!("{}", char::from_u32(c as u32).unwrap_or('?'));
    }
    Ok(args[0].clone())
}

fn f_open_dribble_file(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_open_termscript(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_send_string_to_terminal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let s = want_string(i, &args[0])?;
    print!("{}", s);
    use std::io::Write;
    let _ = std::io::stdout().flush();
    Ok(Value::Nil)
}

fn f_flush_stdout(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    Ok(Value::Nil)
}

fn f_nil(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_false(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::Nil)
}

fn f_bare_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(_) => Ok(args[0].clone()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_bare_symbol_p(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::from_bool(matches!(&args[0], Value::Sym(_))))
}

fn f_position_symbol(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match &args[0] {
        Value::Sym(_) => Ok(args[0].clone()),
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

fn f_make_var_non_special(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    if let Some(id) = i.sym_id(&args[0]) {
        i.obarray.symbol_mut(id).special = false;
    }
    Ok(Value::Nil)
}

fn f_special_variable_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    match i.sym_id(&args[0]) {
        Some(id) => Ok(Value::from_bool(i.obarray.symbol(id).special)),
        None => Err(i.wrong_type_mut("symbolp", &args[0])),
    }
}

// ---------- time values ----------
//
// Lisp time is (HIGH LOW MICRO PICO) or an integer seconds/ticks. We
// convert to microseconds (i128) internally.

fn lisp_time_to_us(i: &mut Interp, v: &Value) -> Result<i128, Flow> {
    match v {
        Value::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            Ok(now.as_micros() as i128)
        }
        Value::Int(n) => Ok(*n as i128 * 1_000_000),
        Value::Float(f) => Ok((*f * 1e6) as i128),
        Value::Cons(_) => {
            // Walk the conses — a dotted tail means (TICKS . HZ).
            let mut elems: Vec<i128> = Vec::new();
            let mut tail = v.clone();
            let mut hz: Option<i128> = None;
            loop {
                let step = match &tail {
                    Value::Cons(c) => {
                        let (car, cdr) = {
                            let b = c.borrow();
                            (b.car.clone(), b.cdr.clone())
                        };
                        if let Value::Int(n) = car {
                            elems.push(n as i128);
                        }
                        Some(cdr)
                    }
                    Value::Int(n) => {
                        hz = Some(*n as i128);
                        None
                    }
                    _ => None,
                };
                match step {
                    Some(cdr) => tail = cdr,
                    None => break,
                }
                if elems.len() > 4 {
                    break;
                }
            }
            if let Some(hz) = hz {
                let ticks = elems.first().copied().unwrap_or(0);
                return Ok(ticks * 1_000_000 / hz.max(1));
            }
            let n = |k: usize| elems.get(k).copied().unwrap_or(0);
            let ticks = match elems.len() {
                0 => 0,
                1 => n(0),
                _ => n(0) * 65536 + n(1),
            };
            Ok(ticks * 1_000_000 + n(2))
        }
        _ => Err(i.wrong_type_mut("listp", v)),
    }
}

fn us_to_lisp_time(us: i128) -> Value {
    let secs = us.div_euclid(1_000_000);
    let micro = us.rem_euclid(1_000_000);
    let hi = secs.div_euclid(65536);
    let lo = secs.rem_euclid(65536);
    Value::list(vec![
        Value::Int(hi as i128),
        Value::Int(lo as i128),
        Value::Int(micro as i128),
        Value::Int(0),
    ])
}

/// Minimal POSIX tm for `localtime_r`.
#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i128,
    tm_zone: *const u8,
}

unsafe extern "C" {
    fn localtime_r(timep: *const i128, result: *mut Tm) -> *mut Tm;
}

/// Days since 1970-01-01 for (y, m, d) — Howard Hinnant's algorithm.
fn days_from_civil(y: i128, m: i128, d: i128) -> i128 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn f_encode_time(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // (encode-time SECOND MINUTE HOUR DAY MONTH YEAR &rest) or a list.
    let items: Vec<Value> = if args.len() == 1 {
        match args[0].list_to_vec() {
            Ok(v) => v,
            Err(_) => args.clone(),
        }
    } else {
        args.clone()
    };
    let get = |k: usize| -> i128 { items.get(k).and_then(|v| v.int()).unwrap_or(0) };
    let (sec, min, hour, day, mon, year) = (get(0), get(1), get(2), get(3), get(4), get(5));
    let days = days_from_civil(year, mon.max(1).min(12), day.max(1));
    let mut secs = days * 86400 + hour * 3600 + min * 60 + sec;
    // ZONE (index 8) may give an explicit offset in seconds.
    if let Some(Value::Int(off)) = items.get(8) {
        secs -= off;
    } else {
        // Interpret as local time: find the local UTC offset at this
        // approximate instant via localtime_r.
        let t = secs;
        let mut tm = unsafe { std::mem::zeroed::<Tm>() };
        unsafe { localtime_r(&t, &mut tm) };
        secs -= tm.tm_gmtoff;
    }
    Ok(us_to_lisp_time(secs as i128 * 1_000_000))
}

fn f_decode_time(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let t = match args.get(0) {
        Some(v) => lisp_time_to_us(i, v)?,
        None => lisp_time_to_us(i, &Value::Nil)?,
    };
    let secs = (t / 1_000_000) as i128;
    let tm = unsafe {
        let mut tm = std::mem::zeroed::<Tm>();
        localtime_r(&secs, &mut tm);
        tm
    };
    Ok(Value::list(vec![
        Value::Int(tm.tm_sec as i128),
        Value::Int(tm.tm_min as i128),
        Value::Int(tm.tm_hour as i128),
        Value::Int(tm.tm_mday as i128),
        Value::Int(tm.tm_mon as i128 + 1),
        Value::Int(tm.tm_year as i128 + 1900),
        Value::Int(tm.tm_wday as i128),
        if tm.tm_isdst > 0 {
            Value::t()
        } else {
            Value::Nil
        },
        Value::Int(tm.tm_gmtoff),
    ]))
}

fn f_time_add(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(us_to_lisp_time(a + b))
}

fn f_time_subtract(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(us_to_lisp_time(a - b))
}

fn f_time_less_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(Value::from_bool(a < b))
}

fn f_time_equal_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let a = lisp_time_to_us(i, &args[0])?;
    let b = lisp_time_to_us(i, &args[1])?;
    Ok(Value::from_bool(a == b))
}

fn f_time_convert(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let us = lisp_time_to_us(i, &args[0])?;
    let form = args.get(1);
    let hz = args.get(2).and_then(|v| v.int()).unwrap_or(1_000_000);
    match form {
        Some(Value::Sym(_)) => {
            let name = i.symbol_name(i.sym_id(&args[1]).unwrap_or(0));
            if name == "integer" {
                Ok(Value::Int((us * hz as i128 / 1_000_000) as i128))
            } else {
                Ok(us_to_lisp_time(us))
            }
        }
        Some(Value::Int(h)) => Ok(Value::Int((us * *h as i128 / 1_000_000) as i128)),
        _ => Ok(us_to_lisp_time(us)),
    }
}

fn f_emacs_uptime(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let start = START.get_or_init(std::time::Instant::now);
    let secs = start.elapsed().as_secs() as i128;
    if let Some(fmt) = a.get(0) {
        if fmt.truthy() {
            // (emacs-uptime FORMAT) formats via format-time-string.
            let fts = i.intern("format-time-string");
            if i.fbound_p(fts) {
                let t = us_to_lisp_time(start.elapsed().as_micros() as i128);
                let q = |v: &Value| Value::list(vec![Value::Sym(sym::QUOTE), v.clone()]);
                let args = Value::list(vec![q(fmt), q(&t)]);
                return i.call_function(&Value::Sym(fts), &args, None);
            }
        }
    }
    let days = secs / 86400;
    let s = if days <= 0 {
        format!("{} second{}", secs, if secs == 1 { "" } else { "s" })
    } else {
        let rem = secs % 86400;
        format!(
            "{} day{}, {:02}:{:02}:{:02}",
            days,
            if days == 1 { "" } else { "s" },
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    };
    Ok(Value::string(s))
}

fn f_current_cpu_time(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    let start = START.get_or_init(std::time::Instant::now);
    Ok(us_to_lisp_time(start.elapsed().as_micros() as i128))
}

static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn f_set_time_zone(_i: &mut Interp, _a: Vec<Value>) -> EvalResult {
    Ok(Value::t())
}

// ---------- hashing / base64 ----------

fn hash_hex(algo: &str, bytes: &[u8]) -> Result<String, Flow> {
    use sha2::Digest;
    let out = match algo {
        "md5" => md5::Md5::digest(bytes).to_vec(),
        "sha1" => sha1::Sha1::digest(bytes).to_vec(),
        "sha224" => sha2::Sha224::digest(bytes).to_vec(),
        "sha256" => sha2::Sha256::digest(bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(bytes).to_vec(),
        "sha512" => sha2::Sha512::digest(bytes).to_vec(),
        _ => return Err(Flow::Signal(Value::Nil, Value::Nil)),
    };
    Ok(out.iter().map(|b| format!("{:02x}", b)).collect())
}

fn secure_hash_str(i: &mut Interp, args: &[Value]) -> Result<String, Flow> {
    let algo_sym = i.sym_id(&args[0]).unwrap_or(u32::MAX);
    let algo = i.symbol_name(algo_sym);
    let obj = &args[1];
    let bytes: Vec<u8> = match obj {
        Value::Str(s) => s.borrow().as_bytes().to_vec(),
        _ => {
            // Buffer text (current buffer or 4th arg).
            let b = i
                .current_buffer_ref()
                .ok_or_else(|| i.error("No current buffer"))?;
            let bb = b.borrow();
            bb.text.text().into_bytes()
        }
    };
    hash_hex(&algo, &bytes).map_err(|_| {
        i.signal_data(
            sym::ERROR,
            vec![Value::string(format!("Unknown hash algorithm {}", algo))],
        )
    })
}

fn f_secure_hash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    Ok(Value::string(secure_hash_str(i, &args)?))
}

fn f_md5(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let mut a = vec![Value::Sym(i.intern("md5"))];
    a.extend(args);
    f_secure_hash(i, a)
}

fn f_b64_encode_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let s = want_string(i, &args[0])?;
    let no_break = arg(&args, 1).truthy();
    let out = if no_break {
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(s.as_bytes())
    } else {
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    };
    Ok(Value::string(out))
}

fn f_b64_decode_string(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use base64::Engine;
    let s = want_string(i, &args[0])?;
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    match base64::engine::general_purpose::STANDARD.decode(cleaned.as_bytes()) {
        Ok(bytes) => Ok(Value::string(String::from_utf8_lossy(&bytes).to_string())),
        Err(_) => Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid base64 data")])),
    }
}

fn f_buffer_hash(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let b = match args.get(0) {
        Some(Value::Buffer(b)) => b.clone(),
        _ => i
            .current_buffer_ref()
            .ok_or_else(|| i.error("No current buffer"))?,
    };
    let text = b.borrow().text.text();
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    Ok(Value::Int(h.finish() as i128))
}

// ---------- files ----------

fn f_make_temp_file_internal(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let prefix = want_string(i, &args[0])?;
    let dir = std::env::temp_dir();
    for n in 0..1000u32 {
        let cand = dir.join(format!(
            "{}{}",
            prefix,
            if n == 0 {
                random_suffix()
            } else {
                format!("{}{}", random_suffix(), n)
            }
        ));
        match std::fs::File::create_new(&cand) {
            Ok(_) => return Ok(Value::string(cand.to_string_lossy().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(i.signal_data(
                    sym::FILE_ERROR,
                    vec![
                        Value::string(format!("Creating temp file: {}", e)),
                        Value::string(cand.to_string_lossy().to_string()),
                    ],
                ));
            }
        }
    }
    Err(i.signal_data(
        sym::FILE_ERROR,
        vec![Value::string("Cannot create temp file")],
    ))
}

fn random_suffix() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        ^ (std::process::id() << 16);
    format!("{:x}", n)
}

fn f_make_symbolic_link(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let target = want_string(i, &args[0])?;
    let name = want_string(i, &args[1])?;
    match std::os::unix::fs::symlink(&target, &name) {
        Ok(()) => Ok(Value::Nil),
        Err(e) => Err(i.signal_data(
            sym::FILE_ERROR,
            vec![
                Value::string(format!("Making symbolic link: {}", e)),
                Value::string(name),
            ],
        )),
    }
}

fn f_directory_name_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let _ = i;
    let s = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        _ => return Ok(Value::Nil),
    };
    Ok(Value::from_bool(s.ends_with('/')))
}

fn f_file_name_case_insensitive_p(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let path = want_string(i, &args[0])?;
    // macOS default FS is case-insensitive; probe by trying to stat a
    // case-flipped name.
    let flipped: String = path
        .chars()
        .map(|c| {
            if c.is_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    if flipped == path {
        return Ok(Value::from_bool(std::path::Path::new(&path).exists()));
    }
    Ok(Value::from_bool(
        std::path::Path::new(&flipped).exists() && std::path::Path::new(&path).exists(),
    ))
}

fn f_file_attributes_lessp(_i: &mut Interp, args: Vec<Value>) -> EvalResult {
    // Compare mtime (index 5 in the attributes list) — simplified:
    // compare nth 5 element if present, else nil.
    let get_mtime = |v: &Value| -> Option<i128> {
        let items = v.list_to_vec().ok()?;
        items.get(5).and_then(|x| x.int())
    };
    match (get_mtime(&args[0]), get_mtime(&args[1])) {
        (Some(a), Some(b)) => Ok(Value::from_bool(a < b)),
        _ => Ok(Value::Nil),
    }
}

fn f_set_file_times(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    if !std::path::Path::new(&name).exists() {
        return Err(i.signal_data(
            sym::FILE_MISSING,
            vec![Value::string(format!(
                "Setting file times: no such file {}",
                name
            ))],
        ));
    }
    Ok(Value::t())
}

fn f_get_file_buffer(i: &mut Interp, args: Vec<Value>) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let canonical = std::fs::canonicalize(&name)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| name.clone());
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let bb = b.borrow();
            if let Some(f) = &bb.file_name {
                let fc = std::fs::canonicalize(f)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| f.clone());
                if fc == canonical {
                    return Ok(i.buffer_value(id).unwrap_or(Value::Nil));
                }
            }
        }
    }
    Ok(Value::Nil)
}
