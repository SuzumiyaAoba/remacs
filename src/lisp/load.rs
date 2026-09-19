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

/// Read the file at PATH and evaluate all forms in it.
/// Binds `load-file-name` and `load-in-progress` like Emacs `load`.
pub fn eval_file(i: &mut Interp, path: &str) -> EvalResult {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return Err(i.signal_data(
                crate::lisp::sym::FILE_ERROR,
                vec![
                    Value::string(format!("Opening input file: {}", e)),
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
    let lfn = i.intern("load-file-name");
    let lip = i.intern("load-in-progress");
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    i.specbind(lfn, Value::string(canon));
    i.specbind(lip, Value::t());
    let r = i.eval_str(&src);
    if r.is_ok() {
        // Push the file onto current-load-list's default? Emacs pushes
        // each loaded file; we keep it simple.
        let _ = cll;
    }
    i.unbind_to(mark);
    r
}

/// Load library NAME; returns true if a file was found and loaded.
pub(crate) fn load_library(i: &mut Interp, name: &str) -> Result<bool, crate::lisp::Flow> {
    match locate(i, name) {
        Some(path) => {
            eval_file(i, &path)?;
            Ok(true)
        }
        None => Ok(false),
    }
}
