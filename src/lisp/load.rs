//! File loading: locate libraries on load-path, eval .el files.

use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
    ("avl-tree", include_str!("../../lisp/avl-tree.el")),
    ("cl-macs", include_str!("../../lisp/cl-macs.el")),
    ("cl-seq", include_str!("../../lisp/cl-seq.el")),
    ("cl-extra", include_str!("../../lisp/cl-extra.el")),
    ("color", include_str!("../../lisp/color.el")),
    ("dom", include_str!("../../lisp/dom.el")),
    ("easy-mmode", include_str!("../../lisp/easy-mmode.el")),
    ("eieio", include_str!("../../lisp/eieio.el")),
    ("face-remap", include_str!("../../lisp/face-remap.el")),
    ("gv", include_str!("../../lisp/gv.el")),
    ("hi-lock", include_str!("../../lisp/hi-lock.el")),
    ("icomplete", include_str!("../../lisp/icomplete.el")),
    ("imenu", include_str!("../../lisp/imenu.el")),
    ("kmacro", include_str!("../../lisp/kmacro.el")),
    ("let-alist", include_str!("../../lisp/let-alist.el")),
    ("macros", include_str!("../../lisp/macros.el")),
    ("map", include_str!("../../lisp/map.el")),
    ("minibuf-eldef", include_str!("../../lisp/minibuf-eldef.el")),
    ("pcase", include_str!("../../lisp/pcase.el")),
    ("pp", include_str!("../../lisp/pp.el")),
    ("pulse", include_str!("../../lisp/pulse.el")),
    ("repeat", include_str!("../../lisp/repeat.el")),
    ("ring", include_str!("../../lisp/ring.el")),
    ("subr-x", include_str!("../../lisp/subr-x.el")),
    ("thingatpt", include_str!("../../lisp/thingatpt.el")),
    ("thunk", include_str!("../../lisp/thunk.el")),
    ("time-date", include_str!("../../lisp/time-date.el")),
    ("savehist", include_str!("../../lisp/savehist.el")),
    ("saveplace", include_str!("../../lisp/saveplace.el")),
    ("mb-depth", include_str!("../../lisp/mb-depth.el")),
    ("radix-tree", include_str!("../../lisp/radix-tree.el")),
    ("skeleton", include_str!("../../lisp/skeleton.el")),
    ("doctor", include_str!("../../lisp/doctor.el")),
    ("mpuz", include_str!("../../lisp/mpuz.el")),
    ("rx", include_str!("../../lisp/rx.el")),
    ("tabulated-list", include_str!("../../lisp/tabulated-list.el")),
    ("display-line-numbers", include_str!("../../lisp/display-line-numbers.el")),
    ("ewoc", include_str!("../../lisp/ewoc.el")),
    ("ansi-color", include_str!("../../lisp/ansi-color.el")),
    ("regi", include_str!("../../lisp/regi.el")),
    ("tempo", include_str!("../../lisp/tempo.el")),
    ("autoinsert", include_str!("../../lisp/autoinsert.el")),
    ("dabbrev", include_str!("../../lisp/dabbrev.el")),
    ("expand", include_str!("../../lisp/expand.el")),
    ("tq", include_str!("../../lisp/tq.el")),
    ("let-alist", include_str!("../../lisp/let-alist.el")),
    ("tildify", include_str!("../../lisp/tildify.el")),
    ("timezone", include_str!("../../lisp/timezone.el")),
    ("cookie1", include_str!("../../lisp/cookie1.el")),
    ("env", include_str!("../../lisp/env.el")),
    ("novice", include_str!("../../lisp/novice.el")),
    ("delim-col", include_str!("../../lisp/delim-col.el")),
    ("align", include_str!("../../lisp/align.el")),
    ("filecache", include_str!("../../lisp/filecache.el")),
    ("rot13", include_str!("../../lisp/rot13.el")),
    ("soundex", include_str!("../../lisp/soundex.el")),
    ("hex-util", include_str!("../../lisp/hex-util.el")),
    ("password-cache", include_str!("../../lisp/password-cache.el")),
    ("indent-aux", include_str!("../../lisp/indent-aux.el")),
    ("underline", include_str!("../../lisp/underline.el")),
    ("studly", include_str!("../../lisp/studly.el")),
    ("dissociate", include_str!("../../lisp/dissociate.el")),
    ("talk", include_str!("../../lisp/talk.el")),
    ("spook", include_str!("../../lisp/spook.el")),
    ("ring-bell-fns", include_str!("../../lisp/ring-bell-fns.el")),
    ("gamegrid", include_str!("../../lisp/gamegrid.el")),
    ("gomoku", include_str!("../../lisp/gomoku.el")),
    ("hanoi", include_str!("../../lisp/hanoi.el")),
    ("life", include_str!("../../lisp/life.el")),
    ("solitaire", include_str!("../../lisp/solitaire.el")),
    ("md4", include_str!("../../lisp/md4.el")),
    ("elide-head", include_str!("../../lisp/elide-head.el")),
    ("external-completion", include_str!("../../lisp/external-completion.el")),
    ("case-table", include_str!("../../lisp/case-table.el")),
    ("chistory", include_str!("../../lisp/chistory.el")),
    ("midnight", include_str!("../../lisp/midnight.el")),
    ("cl-lib", include_str!("../../lisp/cl-lib.el")),
    ("format-spec", include_str!("../../lisp/format-spec.el")),
    ("tabify", include_str!("../../lisp/tabify.el")),
    ("scroll-lock", include_str!("../../lisp/scroll-lock.el")),
    ("obarray", include_str!("../../lisp/obarray.el")),
    ("ansi-osc", include_str!("../../lisp/ansi-osc.el")),
    ("flow-ctrl", include_str!("../../lisp/flow-ctrl.el")),
    ("help-macro", include_str!("../../lisp/help-macro.el")),
    ("master", include_str!("../../lisp/master.el")),
    ("rfn-eshadow", include_str!("../../lisp/rfn-eshadow.el")),
    ("scroll-all", include_str!("../../lisp/scroll-all.el")),
    ("send-to", include_str!("../../lisp/send-to.el")),
    ("userlock", include_str!("../../lisp/userlock.el")),
    ("delsel", include_str!("../../lisp/delsel.el")),
    ("help-at-pt", include_str!("../../lisp/help-at-pt.el")),
    ("rtree", include_str!("../../lisp/rtree.el")),
    ("time-stamp", include_str!("../../lisp/time-stamp.el")),
    ("emacs-lock", include_str!("../../lisp/emacs-lock.el")),
    ("mouse-copy", include_str!("../../lisp/mouse-copy.el")),
    ("t-mouse", include_str!("../../lisp/t-mouse.el")),
    ("bind-key", include_str!("../../lisp/bind-key.el")),
    ("misc", include_str!("../../lisp/misc.el")),
    ("mouse-drag", include_str!("../../lisp/mouse-drag.el")),
    ("lpr", include_str!("../../lisp/lpr.el")),
    ("loadhist", include_str!("../../lisp/loadhist.el")),
    ("help-at-pt", include_str!("../../lisp/help-at-pt.el")),
    ("jka-cmpr-hook", include_str!("../../lisp/jka-cmpr-hook.el")),
    ("jka-compr", include_str!("../../lisp/jka-compr.el")),
    ("hl-line", include_str!("../../lisp/hl-line.el")),
    ("ecomplete", include_str!("../../lisp/ecomplete.el")),
    ("avoid", include_str!("../../lisp/avoid.el")),
    ("visual-wrap", include_str!("../../lisp/visual-wrap.el")),
    ("uniquify", include_str!("../../lisp/uniquify.el")),
    ("view", include_str!("../../lisp/view.el")),
    ("whitespace", include_str!("../../lisp/whitespace.el")),
    ("windmove", include_str!("../../lisp/windmove.el")),
    ("winner", include_str!("../../lisp/winner.el")),
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
    eval_file_lex(i, path, false)
}

/// `--script FILE`: like `load`, but `lexical-binding' is forced on —
/// GNU's `command-line--load-script' does `setq-local lexical-binding t'
/// before eval-buffer, independent of any file cookie.
pub fn eval_file_script(i: &mut Interp, path: &str) -> EvalResult {
    eval_file_lex(i, path, true)
}

fn eval_file_lex(i: &mut Interp, path: &str, force_lex: bool) -> EvalResult {
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
    // GNU resolves the file's truename for `load-file-name' (e.g.
    // /tmp -> /private/tmp on macOS); fall back to the absolutized
    // path when canonicalization fails.
    let canon = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            if std::path::Path::new(path).is_absolute() {
                path.to_string()
            } else {
                std::env::current_dir()
                    .map(|d| d.join(path).to_string_lossy().into_owned())
                    .unwrap_or_else(|_| path.to_string())
            }
        });
    eval_src(i, &canon, &src, force_lex)
}

/// Evaluate SRC as if loaded from file FILE (binds load-file-name,
/// load-in-progress, lexical-binding cookie; runs after-load hooks).
fn eval_src(i: &mut Interp, file: &str, src: &str, force_lex: bool) -> EvalResult {
    let lfn = i.intern("load-file-name");
    let lip = i.intern("load-in-progress");
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    i.specbind(lfn, Value::string(file))?;
    i.specbind(lip, Value::t())?;
    // Emacs: `load' honors a `lexical-binding' file cookie on the first
    // line (or the second, after a `#!' line); absent → dynamic eval.
    let lex_id = i.intern("lexical-binding");
    let lex_on = force_lex || file_lexical_binding(&src);
    i.specbind(lex_id, if lex_on { Value::t() } else { Value::Nil })?;
    // GNU's `readevalloop' specbinds `internal-interpreter-environment'
    // to `(t)' for lexical files (nil for dynamic) for the file's whole
    // dynamic extent: toplevel bare `defvar's become file-scoped special
    // declarations that unwind when the load finishes.
    let saved_lexenv = std::mem::replace(
        &mut i.lexenv,
        if lex_on {
            crate::lisp::eval::lexenv_root()
        } else {
            None
        },
    );
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
    let flist = Value::list(i.features.iter().map(|s| i.sym(*s)).collect::<Vec<_>>());
    let fid = i.intern("features");
    i.obarray.symbol_mut(fid).value = flist;
    i.specbind(cll, Value::Nil)?;
    // Embedded (`builtin:') libraries play the role of GNU's dumped
    // .elc files: functions defined while they load keep
    // `dumped_doc' docstring semantics.
    let was_dumped = i.loading_dumped;
    i.loading_dumped |= file.starts_with("builtin:");
    let r = eval_str_for_load(i, &src);
    i.loading_dumped = was_dumped;
    i.lexenv = saved_lexenv;
    if r.is_ok() {
        // GNU records the file in `load-history': (FILE . ENTRIES),
        // newest file first, entries in evaluation order.
        let mut entries = i.symbol_value(cll).list_to_vec().unwrap_or_default();
        entries.reverse();
        let entry = Value::cons(Value::string(file), Value::list(entries));
        let lh = i.intern("load-history");
        let cur = match i.symbol_value(lh) {
            // Loads can run before the prelude `defvar' initializes it.
            Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
            v => v,
        };
        i.obarray.symbol_mut(lh).value = Value::cons(entry, cur);
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
    let entries = i.symbol_value(alist_sym).list_to_vec().unwrap_or_default();
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
    let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
    let mut pos = 0usize;
    let mut last = Value::Nil;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
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
    let expanded = match crate::lisp::builtins::evalfn::macroexpand_all(i, &form) {
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
        Ok(v) => {
            record_load_entry(i, &expanded);
            Ok(v)
        }
        Err(crate::lisp::Flow::Throw(tag, val)) => {
            let nc = i.intern("no-catch");
            Err(i.signal_data(nc, vec![tag, val]))
        }
        Err(f) => Err(f),
    }
}

/// Record a `load-history' entry for the embedded startup prelude so
/// `symbol-file' and the find-func family can locate prelude-defined
/// symbols.  GNU records every dumped library in `load-history' the same
/// way; FILE names the on-disk copy of the prelude source.
pub(crate) fn record_prelude_load_history(i: &mut Interp, file: &str, src: &str) {
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    if i.specbind(cll, Value::Nil).is_err() {
        return;
    }
    let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
    let mut pos = 0usize;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(pos);
            match reader.read() {
                Ok(f) => f.map(|v| (v, reader.position())),
                Err(_) => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                record_form_tree(i, &form);
            }
            None => break,
        }
    }
    let mut entries = i.symbol_value(cll).list_to_vec().unwrap_or_default();
    entries.reverse();
    let entry = Value::cons(Value::string(file), Value::list(entries));
    let lh = i.intern("load-history");
    let cur = match i.symbol_value(lh) {
        Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
        v => v,
    };
    i.obarray.symbol_mut(lh).value = Value::cons(entry, cur);
    let _ = i.unbind_to(mark);
}

/// Record one top-level prelude form, splicing `progn' children the way
/// `eval_for_load' does during `load'.
fn record_form_tree(i: &mut Interp, form: &Value) {
    if let Some(children) = progn_children(i, form) {
        for child in children {
            record_form_tree(i, &child);
        }
    } else {
        record_load_entry(i, form);
    }
}

/// Record a `load-history' entry for a successfully evaluated top-level
/// form during `load', pushing onto `current-load-list' the way GNU's
/// lread.c does.  GNU's entry shapes: `(defun . SYM)' for function-ish
/// definitions (defun/defmacro/defsubst/defalias/autoload, plus the
/// `defun' leaf that mode/defgeneric macros expand into), bare `SYM'
/// for variables, `(defface . SYM)', `(provide . FEAT)' and
/// `(require . FEAT)'.
fn record_load_entry(i: &mut Interp, form: &Value) {
    let Some(items) = form.list_to_vec().ok() else {
        return;
    };
    let Some(&Value::Sym(head)) = items.first() else {
        return;
    };
    // NAME is (cadr FORM); unwrap a `quote' wrapper (`provide' et al).
    let quote_id = i.intern("quote");
    let name = |items: &[Value]| -> Option<Value> {
        match items.get(1)? {
            v @ Value::Sym(_) => Some(v.clone()),
            Value::Cons(_) => {
                let q = items[1].list_to_vec().ok()?;
                if q.len() == 2 && matches!(&q[0], Value::Sym(s) if *s == quote_id) {
                    Some(q[1].clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };
    const DEFUN_HEADS: &[&str] = &[
        "defun",
        "defmacro",
        "defsubst",
        "defalias",
        "autoload",
        "cl-defun",
        "cl-defmacro",
        "define-minor-mode",
        "define-derived-mode",
        "cl-defgeneric",
        "cl-defmethod",
        "define-obsolete-function-alias",
    ];
    const DEFVAR_HEADS: &[&str] = &[
        "defvar",
        "defconst",
        "defvar-local",
        "defcustom",
        "defvar-keymap",
        "custom-declare-variable",
        "define-obsolete-variable-alias",
    ];
    let head_name = i.symbol_name(head).to_string();
    let defun_id = i.intern("defun");
    let defface_id = i.intern("defface");
    let provide_id = i.intern("provide");
    let require_id = i.intern("require");
    let entry = match head_name.as_str() {
        h if DEFUN_HEADS.contains(&h) => name(&items).map(|nm| Value::cons(i.sym(defun_id), nm)),
        h if DEFVAR_HEADS.contains(&h) => name(&items),
        // `defface' expands to `custom-declare-face' before eval; GNU's
        // C subr attaches (defface . FACE) to `current-load-list'.
        "defface" | "custom-declare-face" => {
            name(&items).map(|nm| Value::cons(i.sym(defface_id), nm))
        }
        "provide" => name(&items).map(|nm| Value::cons(i.sym(provide_id), nm)),
        "require" => name(&items).map(|nm| Value::cons(i.sym(require_id), nm)),
        _ => None,
    };
    if let Some(e) = entry {
        let cll = i.intern("current-load-list");
        let cur = match i.symbol_value(cll) {
            Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
            v => v,
        };
        i.obarray.symbol_mut(cll).value = Value::cons(e, cur);
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
                eval_src(i, &virtual_path, src, false)?;
                Ok(true)
            }
            None => Ok(false),
        },
    }
}
