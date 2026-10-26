//! Ordered source evaluation and eager macro expansion while loading.

use super::history::progn_children;
use super::locate::{embedded_entry, path_is_bundled_source};
use crate::lisp::{Interp, error::EvalResult, value::Value};
use std::rc::Rc;

/// Read and eval each top-level form, eagerly expanding macros the way
/// GNU's `internal-macroexpand-for-load' does during `load'.  This is
/// what resolves macro autoloads (e.g. `define-minor-mode') at load
/// time rather than first call.
pub(super) fn eval_str_for_load(
    i: &mut Interp,
    file: &crate::lisp::text::EmacsStr,
    src: &crate::lisp::text::EmacsStr,
    load_buffer: Option<&crate::lisp::value::BufferRef>,
    compiled: bool,
) -> EvalResult {
    let native_bridges = native_backend_bridges(i, file, src);
    let stream = if load_buffer.is_none() {
        i.load_stream.clone()
    } else {
        None
    };
    let oversized_codes = stream
        .as_ref()
        .and_then(|stream| stream.borrow().oversized_codes.clone());
    let mut chars: Rc<Vec<crate::lisp::text::EmacsChar>> = stream
        .as_ref()
        .map(|stream| stream.borrow().chars.clone())
        .unwrap_or_else(|| Rc::new(src.chars().collect()));
    // Refresh from the live buffer even before the first form: binding
    // watchers may already have changed or narrowed the source.
    let mut source_version = None;
    let mut pos = 0usize;
    let mut last = Value::Nil;
    // Capture availability once per stream, as GNU readevalloop does.
    // The symbol itself remains live so redefinitions during the load
    // affect later calls.  Compiled streams have already been expanded.
    let expander = i.q("internal-macroexpand-for-load");
    let expander = if i.fbound_p(expander) && !compiled {
        Value::Sym(expander)
    } else {
        Value::Nil
    };
    let lrf_sym = i.q("load-read-function");
    let source_multibyte = load_buffer
        .map(|buffer| crate::lisp::builtins::readfn::buffer_source_multibyte(i, buffer))
        .unwrap_or(true);
    let stream_source = i.load_stream_symbol.map(Value::Sym).unwrap_or(Value::Nil);
    let result = 'load: loop {
        if let Some(buffer) = load_buffer {
            let b = buffer.borrow();
            if !b.live {
                break 'load Err(i.error("Reading from killed buffer"));
            }
            pos = b.point();
            let version = (b.text.chars_mod_tick.get(), b.zv);
            if source_version != Some(version) {
                chars = Rc::new(b.text.text().chars().take(b.zv).collect());
                source_version = Some(version);
            }
        } else if let Some(stream) = &stream {
            pos = stream.borrow().position;
        }
        // GNU's `readevalloop' peeks at the stream itself; the
        // `load-read-function' is consulted only while content remains,
        // so a trailing-whitespace file never reaches it, and a custom
        // reader's `end-of-file' signal propagates as a real error
        // rather than ending the load cleanly.
        let syntax_at = |position: usize| {
            if oversized_codes
                .as_ref()
                .is_some_and(|codes| codes.contains_key(&position))
            {
                None
            } else {
                chars
                    .get(position)
                    .and_then(|character| character.unicode())
            }
        };
        let next = {
            let lrf = i.symbol_value(lrf_sym);
            let is_default = matches!(&lrf, Value::Nil)
                || matches!(&lrf, Value::Sym(s) if i.symbol_name(*s) == "read");
            if is_default {
                if pos >= chars.len() {
                    break 'load Ok(last);
                }
                let _rt = std::time::Instant::now();
                // Loaded forms never position symbols. Only buffer
                // sources add line/column data to syntax errors.
                let (read, end) = crate::lisp::builtins::readfn::read_parser_at_with_codes(
                    i,
                    chars.clone(),
                    pos,
                    None,
                    load_buffer.is_some(),
                    source_multibyte,
                    oversized_codes.clone(),
                );
                if let Some(buffer) = load_buffer {
                    buffer.borrow_mut().set_point(end);
                }
                if let Some(stream) = &stream {
                    stream.borrow_mut().position = end;
                }
                match read {
                    Err(error) => {
                        let context = match load_buffer {
                            Some(buffer) => Value::Buffer(buffer.clone()),
                            None => {
                                let filename = i.q("load-true-file-name");
                                i.symbol_value(filename)
                            }
                        };
                        break 'load crate::lisp::builtins::readfn::finish_stream_read(
                            i,
                            Err(error),
                            Some(context),
                        );
                    }
                    Ok(Some(f)) => {
                        let e = _rt.elapsed().as_micros();
                        if std::env::var_os("LOAD_TIME_SPLIT").is_some() && e > 3000 {
                            eprintln!("[read {}@{}] {}us", file, pos, e);
                        }
                        Some((f, end))
                    }
                    Ok(None) => None,
                }
            } else if let Some(buffer) = load_buffer {
                let mut p = pos;
                loop {
                    while p < chars.len()
                        && syntax_at(p).is_some_and(|c| {
                            matches!(c, ' ' | '\t' | '\n' | '\x0C' | '\r' | '\u{A0}')
                        })
                    {
                        p += 1;
                    }
                    if p < chars.len() && syntax_at(p) == Some(';') {
                        while p < chars.len() && syntax_at(p) != Some('\n') {
                            p += 1;
                        }
                        continue;
                    }
                    break;
                }
                buffer.borrow_mut().set_point(p);
                if p >= chars.len() {
                    break 'load Ok(last);
                }
                match i.apply(&lrf, vec![Value::Buffer(buffer.clone())]) {
                    Err(f) => break 'load Err(f),
                    Ok(form) => Some((form, buffer.borrow().point())),
                }
            } else {
                let stream = stream.as_ref().expect("direct load stream");
                // GNU `readevalloop::read_next' consumes whitespace and
                // `;'-comments on the stream itself before checking EOF,
                // so the read function only ever sees real input.
                let mut p = stream.borrow().position;
                loop {
                    while p < chars.len()
                        && syntax_at(p).is_some_and(|c| {
                            matches!(c, ' ' | '\t' | '\n' | '\x0C' | '\r' | '\u{A0}')
                        })
                    {
                        p += 1;
                    }
                    if p < chars.len() && syntax_at(p) == Some(';') {
                        while p < chars.len() && syntax_at(p) != Some('\n') {
                            p += 1;
                        }
                        continue;
                    }
                    break;
                }
                stream.borrow_mut().position = p;
                if p >= chars.len() {
                    break 'load Ok(last);
                }
                match i.apply(&lrf, vec![stream_source.clone()]) {
                    Err(f) => break 'load Err(f),
                    Ok(form) => {
                        pos = stream.borrow().position;
                        Some((form, pos))
                    }
                }
            }
        };
        if i.dbg_flags & 0x8000 != 0 {
            if let Some((form, end)) = &next {
                eprintln!(
                    "[load-pos] {}: {}..{} len={} head={}",
                    file,
                    pos,
                    end,
                    chars.len(),
                    i.princ_to_string(form)
                        .chars()
                        .take(60)
                        .collect::<crate::lisp::text::EmacsText>()
                );
            }
        }
        match next {
            Some((form, end)) => {
                pos = end;
                if let Some(buffer) = load_buffer {
                    buffer.borrow_mut().set_point(end);
                }
                if let Some(stream) = &stream {
                    stream.borrow_mut().position = end;
                }
                let t0 = (i.dbg_flags & 0x200 != 0).then(std::time::Instant::now);
                match eval_for_load(i, form.clone(), &expander) {
                    Ok(v) => {
                        // comp.el/comp-run.el initialize the real GNU helper
                        // surface, but their libgccjit driver cannot replace
                        // this build's cc backend. Restore before subsequent
                        // forms, provide callbacks and after-load hooks.
                        for (symbol, function) in &native_bridges {
                            i.obarray.symbol_mut(*symbol).function = function.clone();
                        }
                        last = v;
                        if let Some(t0) = t0 {
                            let ms = t0.elapsed().as_millis();
                            let min: u128 = std::env::var("REMACS_FORM_PROF")
                                .ok()
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(10);
                            if ms >= min {
                                eprintln!(
                                    "[form {}@{}] {}ms {}",
                                    file,
                                    end,
                                    ms,
                                    i.princ_to_string(&form)
                                        .chars()
                                        .take(90)
                                        .collect::<crate::lisp::text::EmacsText>()
                                );
                            }
                        }
                    }
                    Err(f) => {
                        if i.dbg_flags & 0x1000 != 0 {
                            let fdesc = match &f {
                                crate::lisp::Flow::Signal(signal) => format!(
                                    "signal {} {}",
                                    i.prin1_to_string(&signal.symbol()),
                                    i.prin1_to_string(&signal.data())
                                        .chars()
                                        .take(120)
                                        .collect::<crate::lisp::text::EmacsText>()
                                ),
                                other => format!("{:?}", other),
                            };
                            eprintln!(
                                "[load-err@{}] {} => {}",
                                end,
                                i.princ_to_string(&form)
                                    .chars()
                                    .take(120)
                                    .collect::<crate::lisp::text::EmacsText>(),
                                fdesc
                            );
                        }
                        break 'load Err(f);
                    }
                }
            }
            None => break 'load Ok(last),
        }
    };
    result
}

fn native_backend_bridges(
    i: &Interp,
    file: &crate::lisp::text::EmacsStr,
    source: &crate::lisp::text::EmacsStr,
) -> Vec<(crate::lisp::SymId, Value)> {
    if i.boot_loading {
        return Vec::new();
    }
    let identity = file.strip_prefix("builtin:").unwrap_or(file);
    let Some((key, embedded_source)) = embedded_entry(identity) else {
        return Vec::new();
    };
    if !matches!(key, "comp" | "comp-run")
        || source != embedded_source
        || !(file.starts_with("builtin:") || path_is_bundled_source(file))
    {
        return Vec::new();
    }
    [
        "native-compile",
        "native-compile-async",
        "batch-native-compile",
        "batch-byte+native-compile",
    ]
    .into_iter()
    .filter_map(|name| {
        let symbol = i.intern_soft(name)?;
        let function = i.symbol_function(symbol);
        match &function {
            Value::Lambda(_) | Value::Subr(_) => Some((symbol, function)),
            _ => None,
        }
    })
    .collect()
}

/// Expand + eval one top-level form the way GNU's
/// `internal-macroexpand-for-load' does during `load': the form is
/// expanded; a top-level `progn' is spliced so each child is expanded
/// and evaluated in turn (an autoloaded macro in a later child is not
/// resolved until that child's expansion runs, matching GNU's
/// observable autoload timing).
fn eval_for_load(i: &mut Interp, form: Value, expander: &Value) -> EvalResult {
    let expanded = if expander.is_nil() {
        form
    } else {
        // GNU first expands the head, then splices a resulting progn.
        // Each child is expanded only after earlier children evaluate,
        // so macro definitions and autoloads retain their load order.
        let head = expand_for_load(i, expander, form, false)?;
        if let Some(children) = progn_children(i, &head) {
            let mut last = Value::Nil;
            for child in children {
                last = eval_for_load(i, child, expander)?;
            }
            return Ok(last);
        }
        // Non-progn forms, including function bodies, receive the full
        // expansion pass.  The GNU hook also handles expansion failures
        // and cycles, and remains observable to advice and redefinition.
        expand_for_load(i, expander, head, true)?
    };
    match i.eval(&expanded) {
        // GNU records definitions via LOADHIST_ATTACH inside the
        // defining subrs (defalias/defvar/defvaralias/provide/require/
        // internal--define-uninitialized-variable) — evaluation-time,
        // so `byte-code'-wrapped defs in `.elc' files register too.
        Ok(v) => Ok(v),
        Err(crate::lisp::Flow::Throw(tag, val)) => {
            let nc = i.q("no-catch");
            Err(i.signal_data(nc, vec![tag, val]))
        }
        Err(f) => Err(f),
    }
}

fn expand_for_load(i: &mut Interp, expander: &Value, form: Value, full: bool) -> EvalResult {
    i.macroexp_call_depth += 1;
    let compiled = i.loading_compiled_library || i.boot_loading;
    let saved_expansion = std::mem::replace(&mut i.compiled_macro_expansion, compiled);
    let result = i.apply(expander, vec![form, Value::from_bool(full)]);
    i.compiled_macro_expansion = saved_expansion;
    i.macroexp_call_depth -= 1;
    result
}
