//! File loading: locate libraries on load-path, eval .el files.

use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;
use std::path::{Path, PathBuf};

/// Candidate paths for library NAME under DIR (with .elc/.el suffixes).
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let base = dir.join(name);
    if name.ends_with(".el") || name.ends_with(".elc") {
        v.push(base);
    } else {
        v.push(base.with_extension("elc"));
        v.push(base.with_extension("el"));
        v.push(base);
    }
    v
}

/// Find library file NAME on load-path (or as an absolute/relative path).
pub(crate) fn locate(i: &mut Interp, name: &str) -> Option<String> {
    let p = Path::new(name);
    if p.is_absolute() || name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        for c in candidates(Path::new(""), name) {
            if c.is_file() {
                return Some(c.to_string_lossy().into_owned());
            }
        }
        if p.is_file() {
            return Some(name.to_string());
        }
        return None;
    }
    // Search load-path (a list of strings).
    let lp_id = i.intern("load-path");
    let lp = i.symbol_value(lp_id);
    let dirs = lp.list_to_vec().unwrap_or_default();
    for d in dirs {
        if let Value::Str(s) = &d {
            let dir = PathBuf::from(s.borrow().as_str());
            for c in candidates(&dir, name) {
                if c.is_file() {
                    return Some(c.to_string_lossy().into_owned());
                }
            }
        }
    }
    // Also try the built-in lisp/ directory relative to the exe.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exedir) = exe.parent() {
            for prefix in [
                exedir.join("../lisp"),
                exedir.join("../share/remacs/lisp"),
                PathBuf::from("lisp"),
            ] {
                for c in candidates(&prefix, name) {
                    if c.is_file() {
                        return Some(c.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

/// Built-in Lisp libraries embedded in the binary (the repo's lisp/ dir),
/// so `load`/autoload works regardless of the process cwd.
static EMBEDDED_LISP: &[(&str, &str)] = &[
    ("dom", include_str!("../../lisp/dom.el")),
    ("easy-mmode", include_str!("../../lisp/easy-mmode.el")),
    ("eieio", include_str!("../../lisp/eieio.el")),
    ("gv", include_str!("../../lisp/gv.el")),
    ("kmacro", include_str!("../../lisp/kmacro.el")),
    ("let-alist", include_str!("../../lisp/let-alist.el")),
    ("macros", include_str!("../../lisp/macros.el")),
    ("map", include_str!("../../lisp/map.el")),
    ("pcase", include_str!("../../lisp/pcase.el")),
    ("pp", include_str!("../../lisp/pp.el")),
    ("ring", include_str!("../../lisp/ring.el")),
    ("subr-x", include_str!("../../lisp/subr-x.el")),
    ("thingatpt", include_str!("../../lisp/thingatpt.el")),
    ("thunk", include_str!("../../lisp/thunk.el")),
];

/// Embedded source for library NAME (with or without .el/.elc suffix).
fn embedded(name: &str) -> Option<&'static str> {
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    EMBEDDED_LISP
        .iter()
        .find(|(n, _)| *n == stem)
        .map(|(_, src)| *src)
}

/// Read the file at PATH and evaluate all forms in it.
/// Binds `load-file-name` and `load-in-progress` like Emacs `load`.
pub fn eval_file(i: &mut Interp, path: &str) -> EvalResult {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            // GNU signals `file-missing' for ENOENT, `file-error' for
            // other failures, with data (FORMAT REASON PATH).
            let sym = if e.kind() == std::io::ErrorKind::NotFound {
                i.intern("file-missing")
            } else {
                crate::lisp::sym::FILE_ERROR
            };
            return Err(i.signal_data(
                sym,
                vec![
                    Value::string("Opening input file"),
                    Value::string(e.to_string()),
                    Value::string(path),
                ],
            ));
        }
    };
    // Track load-file-name / load-in-progress like Emacs does.
    // Emacs stores the canonical (symlink-resolved) file name.
    let canon = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    eval_src(i, &canon, &src)
}

/// Evaluate SRC as if loaded from file FILE (binds load-file-name,
/// load-in-progress, lexical-binding cookie; runs after-load hooks).
fn eval_src(i: &mut Interp, file: &str, src: &str) -> EvalResult {
    let lfn = i.intern("load-file-name");
    let lip = i.intern("load-in-progress");
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    i.specbind(lfn, Value::string(file))?;
    i.specbind(lip, Value::t())?;
    // Emacs: `load' honors a `lexical-binding' file cookie on the first
    // line (or the second, after a `#!' line); absent → dynamic eval.
    let lex_id = i.intern("lexical-binding");
    let lex_on = file_lexical_binding(&src);
    i.specbind(lex_id, if lex_on { Value::t() } else { Value::Nil })?;
    // GNU's load machinery autoloads these preloaded libraries on the
    // first `load' of any file — before the file's own forms run.
    for feat in ["cl-lib", "cl-loaddefs", "icons", "warnings"] {
        let id = i.intern(feat);
        if !i.features.contains(&id) {
            i.features.insert(0, id);
        }
    }
    // Loading `icons' defines the `icon'/`icon-button' faces (GNU's
    // icons.el does this via defface).
    if !i.face_table.iter().any(|(n, _)| n == "icon") {
        i.face_table.push(("icon".to_string(), Value::Nil));
        i.face_table.push(("icon-button".to_string(), Value::Nil));
    }
    let flist =
        Value::list(i.features.iter().map(|s| i.sym(*s)).collect::<Vec<_>>());
    let fid = i.intern("features");
    i.obarray.symbol_mut(fid).value = flist;
    let r = eval_str_for_load(i, &src);
    if r.is_ok() {
        let _ = cll;
        run_after_load(i, file);
    }
    i.unbind_to(mark)?;
    r
}

/// Fire `after-load-alist' entries whose regexp/symbol key matches FILE
/// (GNU's `load' runs `eval-after-load' hooks this way).
fn run_after_load(i: &mut Interp, file: &str) {
    let alist_sym = i.intern("after-load-alist");
    let sm_sym = i.intern("string-match");
    let entries = i
        .symbol_value(alist_sym)
        .list_to_vec()
        .unwrap_or_default();
    for entry in entries {
        let mut parts = entry.list_to_vec().unwrap_or_default();
        if parts.is_empty() {
            continue;
        }
        let key = parts.remove(0);
        let matches = match key {
            Value::Str(_) => i
                .call_function(
                    &Value::Sym(sm_sym),
                    &Value::list(vec![key.clone(), Value::string(file)]),
                    Some(sm_sym),
                )
                .map(|v| !v.is_nil())
                .unwrap_or(false),
            Value::Sym(s) => {
                // Feature-name keys match the file's basename feature.
                file.ends_with(&format!("{}.el", i.symbol_name(s)))
            }
            _ => false,
        };
        if matches {
            for form in parts {
                // Entries store 0-arg functions (lambdas or closures).
                let _ = i.apply(&form, vec![]);
            }
        }
    }
}

/// Read and eval each top-level form, eagerly expanding macros the way
/// GNU's `internal-macroexpand-for-load' does during `load'.  This is
/// what resolves macro autoloads (e.g. `define-minor-mode') at load
/// time rather than first call.
fn eval_str_for_load(i: &mut Interp, src: &str) -> EvalResult {
    let mut pos = 0usize;
    let mut last = Value::Nil;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::new(i, src);
            reader.set_position(pos);
            match reader.read()? {
                Some(f) => Some((f, reader.position())),
                None => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                last = eval_for_load(i, form)?;
            }
            None => return Ok(last),
        }
    }
}

/// Expand + eval one top-level form the way GNU's
/// `internal-macroexpand-for-load' does during `load': the form is
/// expanded; a top-level `progn' is spliced so each child is expanded
/// and evaluated in turn (an autoloaded macro in a later child is not
/// resolved until that child's expansion runs, matching GNU's
/// observable autoload timing).
fn eval_for_load(i: &mut Interp, form: Value) -> EvalResult {
    // Splice a `progn' BEFORE expanding: GNU expands each spliced child
    // at its own turn, so later autoloaded macros stay unresolved while
    // earlier children evaluate.
    if let Some(children) = progn_children(i, &form) {
        let mut last = Value::Nil;
        for child in children {
            last = eval_for_load(i, child)?;
        }
        return Ok(last);
    }
    let expanded = match crate::lisp::builtins::evalfn::macroexpand_all(i, &form)
    {
        Ok(f) => f,
        // GNU's `internal-macroexpand-for-load' wraps expansion
        // failures: it re-signals (error "Eager macro-expansion
        // failure: %S" err) where err is the original condition as
        // (sym . data); `error' formats eagerly, so the data is a
        // single formatted string.  Non-signal exits propagate.
        Err(crate::lisp::Flow::Signal(sym, data, _)) => {
            let err = Value::cons(sym, data);
            let msg = crate::lisp::builtins::evalfn::apply_format_simple(
                i,
                "Eager macro-expansion failure: %S",
                &[err],
            );
            return Err(i.error(msg));
        }
        Err(f) => return Err(f),
    };
    // A macro may have expanded into a top-level `progn' — splice it too.
    if let Some(children) = progn_children(i, &expanded) {
        let mut last = Value::Nil;
        for child in children {
            last = eval_for_load(i, child)?;
        }
        return Ok(last);
    }
    match i.eval(&expanded) {
        Ok(v) => Ok(v),
        Err(crate::lisp::Flow::Throw(tag, val)) => {
            let nc = i.intern("no-catch");
            Err(i.signal_data(nc, vec![tag, val]))
        }
        Err(f) => Err(f),
    }
}

/// If FORM is `(progn CHILDREN...)', return the children.
fn progn_children(i: &mut Interp, form: &Value) -> Option<Vec<Value>> {
    if let Value::Cons(c) = form {
        let cb = c.borrow();
        if let Value::Sym(s) = &cb.car {
            if *s == i.intern("progn") {
                return cb.cdr.list_to_vec().ok();
            }
        }
    }
    None
}

/// True when the file declares `-*- lexical-binding: t -*-' (or the
/// `lexical-binding: t' local-variable form) on its first line — or on
/// the second line when the first is a `#!' line, like Emacs.
fn file_lexical_binding(src: &str) -> bool {
    let mut lines = src.lines();
    let first = lines.next().unwrap_or("");
    let probe = if first.starts_with("#!") {
        lines.next().unwrap_or("")
    } else {
        first
    };
    probe.contains("lexical-binding: t") || probe.contains("lexical-binding:t")
}

/// Load library NAME; returns true if a file was found and loaded.
pub(crate) fn load_library(i: &mut Interp, name: &str) -> Result<bool, crate::lisp::Flow> {
    match locate(i, name) {
        Some(path) => {
            eval_file(i, &path)?;
            Ok(true)
        }
        // Fall back to the embedded copy of a built-in library, so that
        // autoloads work even when the lisp/ dir isn't reachable by path.
        None => match embedded(name) {
            Some(src) => {
                let virtual_path = format!("builtin:{}", name);
                eval_src(i, &virtual_path, src)?;
                Ok(true)
            }
            None => Ok(false),
        },
    }
}
