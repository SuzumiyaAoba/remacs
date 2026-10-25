//! File loading: locate libraries on load-path, eval .el files.

use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// GNU command-line-1's load-path splice cursor. Preserve the cons identity:
/// Lisp can replace load-path between options, leaving this cursor detached.
#[derive(Default)]
pub struct CommandLineLoadPath {
    splice: Option<crate::lisp::value::ConsRef>,
}

impl CommandLineLoadPath {
    pub fn add(&mut self, i: &mut Interp, directory: Value) -> EvalResult {
        let separator = i.q("path-separator");
        let format = Value::Sym(i.q("format"));
        let pattern = i.apply(
            &format,
            vec![Value::string("\\`%s"), i.symbol_value(separator)],
        )?;
        let matches = Value::Sym(i.q("string-match-p"));
        let append = i
            .apply(&matches, vec![pattern, directory.clone()])?
            .truthy();
        let directory = if append {
            let substring = Value::Sym(i.q("substring"));
            i.apply(&substring, vec![directory, Value::Int(1)])?
        } else {
            directory
        };
        let normalize = Value::Sym(i.q("command-line-normalize-file-name"));
        let directory = i.apply(&normalize, vec![directory])?;
        let expand = Value::Sym(i.q("expand-file-name"));
        let directory = i.apply(&expand, vec![directory])?;
        let load_path = i.q("load-path");
        if append {
            let append_fn = Value::Sym(i.q("append"));
            let paths = i.apply(
                &append_fn,
                vec![i.symbol_value(load_path), Value::list(vec![directory])],
            )?;
            i.set_symbol(load_path, paths.clone())?;
            if self.splice.is_some() {
                // append copied the list: GNU restarts its cursor at the head.
                self.splice = match paths {
                    Value::Cons(cell) => Some(cell),
                    _ => None,
                };
            }
        } else {
            let next = Value::cons(
                directory,
                match &self.splice {
                    Some(cell) => cell.borrow().cdr.clone(),
                    None => i.symbol_value(load_path),
                },
            );
            if let Some(cell) = &self.splice {
                cell.borrow_mut().cdr = next.clone();
            } else {
                i.set_symbol(load_path, next.clone())?;
            }
            self.splice = match next {
                Value::Cons(cell) => Some(cell),
                _ => unreachable!(),
            };
        }
        Ok(Value::Nil)
    }
}

/// Consuming input for direct/compiled loads, exposed as GNU's private
/// `get-file-char' stream symbol. Source loads use their real buffer.
pub(crate) struct LoadStream {
    pub chars: Rc<Vec<char>>,
    pub position: usize,
}

/// Candidate paths GNU's `openp' would try for NAME in DIR.
/// NOSUFFIX opens NAME literally; MUST-SUFFIX requires NAME to carry
/// a recognized `.el'/`.elc' suffix already.
fn candidates(dir: &Path, name: &str, nosuffix: bool, mustsuffix: bool) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let base = dir.join(name);
    // GNU Fload: MUST-SUFFIX falls away when NAME has a directory part
    // or already ends in .el/.elc/.eln — then suffixes are tried and
    // the bare name is a valid hit too (via the "" rep suffix).
    let bare_ok = nosuffix
        || !mustsuffix
        || name.contains('/')
        || name.ends_with(".el")
        || name.ends_with(".elc")
        || name.ends_with(".eln");
    if nosuffix {
        v.push(base);
    } else {
        // get-load-suffixes order (no native comp: .elc/.el first,
        // then .gz reps via load-file-rep-suffixes), bare name last
        // when MUST-SUFFIX doesn't bar it.
        if name.ends_with(".el") || name.ends_with(".elc") || name.ends_with(".eln") {
            v.push(base);
        } else {
            v.push(base.with_extension("eln"));
            v.push(base.with_extension("elc"));
            v.push(base.with_extension("el"));
        }
        if bare_ok {
            v.push(dir.join(name));
        }
    }
    v
}

/// Find library file NAME on load-path (or as an absolute/relative path).
/// Directories don't count as hits here.
/// GNU `openp' result: a hit is either a file or a directory — the
/// directory case must reach `Fload' so it can signal "Is a directory".
pub(crate) enum Found {
    File(String),
    Dir(String),
}

/// Pick the first existing candidate; with `load-prefer-newer' the
/// newest of the file candidates in this dir wins (GNU `openp').
fn pick_existing(paths: Vec<PathBuf>, prefer_newer: bool) -> Option<Found> {
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for p in paths {
        if p.is_dir() {
            // GNU `openp' returns directories too — a dir shadows any
            // later file candidate at the same position.
            return Some(Found::Dir(p.to_string_lossy().into_owned()));
        }
        if !p.is_file() {
            continue;
        }
        if !prefer_newer {
            return Some(Found::File(p.to_string_lossy().into_owned()));
        }
        let m = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &best {
            Some((_, bm)) if *bm >= m => {}
            _ => best = Some((p, m)),
        }
    }
    best.map(|(p, _)| Found::File(p.to_string_lossy().into_owned()))
}

/// Locate a load candidate, also reporting directory hits: GNU's `openp'
/// opens directories happily and `Fload' signals "Is a directory"
/// afterwards, so the distinction must survive the search.
pub(crate) fn locate_full(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    mustsuffix: bool,
) -> Option<Found> {
    // GNU's `openp' consults `load-prefer-newer' per directory: the
    // newest of the candidates inside a dir wins before the next dir.
    let prefer_newer = !nosuffix
        && !mustsuffix
        && i.symbol_value(i.q_soft("load-prefer-newer").unwrap_or(0))
            .truthy();
    // GNU's `openp' on "" stats "" and misses (ENOENT) — never a hit.
    if name.is_empty() {
        return None;
    }
    // GNU's `openp' expand-file-name's `~/`/`~user/` names up front —
    // an existing `~' directory must surface as Found::Dir so `load'
    // reports "Is a directory" instead of ENOENT.
    let expanded;
    let name = if name.starts_with('~') {
        expanded = crate::editor::expand_file_name_str(i, name);
        expanded.as_str()
    } else {
        name
    };
    let p = Path::new(name);
    let source_suffixes = if nosuffix
        || name.ends_with(".el") || name.ends_with(".elc") || name.ends_with(".eln")
    {
        vec![String::new()]
    } else {
        vec![".el".to_string()]
    };
    if p.is_absolute() || name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        if let Some(hit) = pick_existing(
            candidates(Path::new(""), name, nosuffix, mustsuffix),
            prefer_newer,
        ) {
            return Some(hit);
        }
        if p.is_absolute() {
            return None;
        }
        // Relative name with a directory part ("language/burmese",
        // "quail/arabic").  GNU resolves it against load-path entries
        // (`leim' + "quail/arabic" → leim/quail/arabic.el); try the
        // same against every search dir.
        for dir in search_dirs(i) {
            if let Some(hit) =
                pick_existing(candidates(&dir, name, nosuffix, mustsuffix), prefer_newer)
            {
                return Some(hit);
            }
            if let Ok(Some(path)) = locate_cache_source(i, &dir, name, &source_suffixes) {
                return Some(Found::File(path));
            }
        }
        // Our bundled tree is flat under lisp/, with colliding
        // basenames renamed to SUBDIR-BASE.el; also try that spelling,
        // then the bare basename (unique basenames like "arabic").
        let flat = name.replace('/', "-");
        let base = name.rsplit('/').next().unwrap_or(name);
        for dir_s in builtin_dirs() {
            let dir = PathBuf::from(dir_s);
            for n in [&flat, base] {
                if let Some(hit) =
                    pick_existing(candidates(&dir, n, nosuffix, mustsuffix), prefer_newer)
                {
                    return Some(hit);
                }
            }
        }
        return None;
    }
    // Search load-path (a list of strings).
    for dir in search_dirs(i) {
        if let Some(hit) = pick_existing(candidates(&dir, name, nosuffix, mustsuffix), prefer_newer)
        {
            return Some(hit);
        }
        if let Ok(Some(path)) = locate_cache_source(i, &dir, name, &source_suffixes) {
            return Some(Found::File(path));
        }
    }
    None
}

/// `load-path' dirs followed by the bundled lisp/ dirs.
fn search_dirs(i: &mut Interp) -> Vec<PathBuf> {
    let lp_id = i.q("load-path");
    let lp = i.symbol_value(lp_id);
    let mut v: Vec<PathBuf> = lp
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| match d {
            Value::Str(s) => Some(PathBuf::from(s.borrow().as_str())),
            _ => None,
        })
        .collect();
    v.extend(builtin_dirs().into_iter().map(PathBuf::from));
    v
}

/// Directories searched for bundled libraries after `load-path'
/// (mirrors `locate''s built-in fallback): the exe-relative lisp/
/// dirs, the development cwd's lisp/, and an already-created source cache.
/// Return absolute paths so Lisp default-directory cannot reinterpret hits.
pub(crate) fn builtin_dirs() -> Vec<String> {
    let mut v = std::env::current_dir()
        .ok()
        .map(|cwd| vec![cwd.join("lisp").to_string_lossy().into_owned()])
        .unwrap_or_default();
    v.extend(installed_library_dirs());
    v
}

/// Publish the post-bootstrap load path. Environment directories must
/// not replace internal bootstrap libraries before the dumped surface
/// has been reconstructed. Empty EMACSLOADPATH entries insert defaults.
pub(crate) fn finish_load_path(i: &mut Interp) {
    let load_path = i.q("load-path");
    let mut defaults = i.symbol_value(load_path).list_to_vec().unwrap_or_default();
    let environment = std::env::var_os("EMACSLOADPATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>());
    let needs_defaults = environment
        .as_ref()
        .is_none_or(|paths| paths.iter().any(|path| path.as_os_str().is_empty()));
    if needs_defaults {
        // A copied executable may have no installed source directory.
        // Publish one core source as an anchor; the existing resolver
        // materializes other embedded sources when they are requested.
        if defaults.is_empty() {
            if let Some(source) = embedded("simple") {
                let _ = super::embedded_cache::materialize("simple", source);
            }
        }
        if let Some(cache) = super::embedded_cache::directory() {
            let canonical = std::fs::canonicalize(&cache).ok();
            // Cache publication uses its logical temporary path. Keep
            // that spelling in load-path so repeated lookups are stable.
            for directory in &mut defaults {
                if let Value::Str(path) = directory {
                    if canonical.is_some()
                        && std::fs::canonicalize(path.borrow().as_str()).ok() == canonical
                    {
                        *directory = Value::string(cache.to_string_lossy().into_owned());
                    }
                }
            }
            let cache = Value::string(cache.to_string_lossy().into_owned());
            if !defaults
                .iter()
                .any(|path| super::builtins::equal_values(i, path, &cache).unwrap_or(false))
            {
                defaults.push(cache);
            }
        }
    }
    let paths = match environment {
        None => defaults,
        Some(paths) => {
            let mut expanded = Vec::new();
            for path in paths {
                if path.as_os_str().is_empty() {
                    expanded.extend(defaults.iter().cloned());
                } else {
                    expanded.push(Value::string(path.to_string_lossy().into_owned()));
                }
            }
            expanded
        }
    };
    let paths = Value::list(paths);
    if !super::builtins::equal_values(i, &i.symbol_value(load_path), &paths).unwrap_or(false) {
        let _ = i.set_symbol(load_path, paths);
    }
}

/// Runtime source locations whose contents belong to the executable.
fn installed_library_dirs() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exedir) = exe.parent() {
            v.push(exedir.join("../lisp").to_string_lossy().into_owned());
            v.push(
                exedir
                    .join("../share/remacs/lisp")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    if let Some(directory) = super::embedded_cache::directory() {
        v.push(directory.to_string_lossy().into_owned());
    }
    v
}

/// Distinguish shipped sources from user overrides, including a user's
/// cwd/lisp directory. All roots are compared after canonicalization.
fn path_is_bundled_source(path: &str) -> bool {
    let Ok(p) = std::fs::canonicalize(path) else {
        return false;
    };
    let mut roots = installed_library_dirs();
    // Classify an explicitly referenced development source tree without
    // making installed executables depend on that tree for searching.
    roots.push(
        Path::new(env!("REMACS_SOURCE_ROOT"))
            .join("lisp")
            .to_string_lossy()
            .into_owned(),
    );
    roots.iter().any(|d| {
        std::fs::canonicalize(d)
            .map(|cd| p.starts_with(&cd))
            .unwrap_or(false)
    })
}

/// Resolved-load identity for comparing against `loads_in_progress':
/// `builtin:' names and on-disk bundled sources with an embedded key
/// all normalize to `builtin:<key>' — the
/// three spellings (virtual path, disk `.el', disk `.elc' twin
/// fallback) one file can appear under while loading.  Everything
/// else is the literal resolved path GNU's `Fequal' would compare.
pub(crate) fn load_key(path: &str) -> String {
    let name = path.strip_prefix("builtin:").unwrap_or(path);
    if path.starts_with("builtin:") || path_is_bundled_source(path) {
        if let Some((key, _)) = embedded_entry(name) {
            let key = if key == "emacs-lisp-subr-x" {
                "subr-x"
            } else {
                key
            };
            return format!("builtin:{key}");
        }
    }
    path.to_string()
}

use remacs_libraries::{EMBEDDED_LISP, embedded_elc};

/// Resolve a source key without consuming another filename suffix.
fn embedded_key(stem: &str) -> Option<(&'static str, &'static str)> {
    EMBEDDED_LISP
        .iter()
        .find(|(key, _)| *key == stem)
        .or_else(|| {
            stem.rsplit_once('/').and_then(|(dir, base)| {
                let flat = format!("{dir}-{base}");
                EMBEDDED_LISP.iter().find(|(key, _)| *key == flat)
            })
        })
        .or_else(|| {
            let base = stem.rsplit('/').next().unwrap_or(stem);
            EMBEDDED_LISP.iter().find(|(key, _)| {
                *key == base
                    || (key.len() == base.len()
                        && key
                            .bytes()
                            .map(|byte| if byte == b'/' { b'-' } else { byte })
                            .eq(base.bytes()))
            })
        })
        .copied()
}

fn embedded_entry(name: &str) -> Option<(&'static str, &'static str)> {
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    embedded_key(stem)
}

/// Embedded source for library NAME (including a returned flat filename).
pub(crate) fn embedded(name: &str) -> Option<&'static str> {
    embedded_entry(name).map(|(_, source)| source)
}

/// Locate the default bundled source after the caller's search path.
/// Explicit PATH searches never enter here. Only real .el candidates
/// participate, so NOSUFFIX and representation suffixes remain meaningful.
pub(crate) fn locate_bundled_source(
    i: &mut Interp,
    name: &str,
    suffixes: &[String],
) -> Result<Option<String>, crate::lisp::Flow> {
    if Path::new(name).is_absolute()
        || name.starts_with('~')
        || Path::new(name).components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Ok(None);
    }
    for suffix in suffixes {
        let candidate = format!("{name}{suffix}");
        let Some(stem) = candidate.strip_suffix(".el") else {
            continue;
        };
        let Some((mut key, mut source)) = embedded_key(stem) else {
            continue;
        };
        // The compatibility amalgam is only a bootstrap artifact.
        if !i.boot_loading && key == "subr-x" {
            (key, source) = embedded_key("emacs-lisp-subr-x").unwrap();
        }
        return bundled_source_file(i, name, key, source).map(Some);
    }
    Ok(None)
}

/// The published source directory represents embedded libraries at its
/// actual load-path position, before directories that follow a default
/// path placeholder. Other directories retain ordinary filesystem lookup.
pub(crate) fn locate_cache_source(
    i: &mut Interp,
    directory: &Path,
    name: &str,
    suffixes: &[String],
) -> Result<Option<String>, crate::lisp::Flow> {
    if i.boot_loading
        || Path::new(name).is_absolute()
        || name.starts_with('~')
        || Path::new(name).components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Ok(None);
    }
    let Some(cache) = super::embedded_cache::directory() else {
        return Ok(None);
    };
    let Ok(cache) = std::fs::canonicalize(cache) else {
        return Ok(None);
    };
    if std::fs::canonicalize(directory).ok().as_ref() != Some(&cache) {
        return Ok(None);
    }
    for suffix in suffixes {
        let candidate = format!("{name}{suffix}");
        let Some(stem) = candidate.strip_suffix(".el") else {
            continue;
        };
        let Some((mut key, mut source)) = embedded_key(stem) else {
            continue;
        };
        // Basename fallback remains a final bundled lookup, rather than
        // claiming arbitrary qualified filenames at this directory.
        if key != stem && key.replace('/', "-") != stem {
            continue;
        }
        if key == "subr-x" {
            (key, source) = embedded_key("emacs-lisp-subr-x").unwrap();
        }
        return materialized_source_file(i, name, key, source).map(Some);
    }
    Ok(None)
}

fn bundled_source_file(
    i: &mut Interp,
    name: &str,
    key: &str,
    source: &str,
) -> Result<String, crate::lisp::Flow> {
    let filename = format!("{}.el", key.replace('/', "-"));
    for directory in builtin_dirs() {
        let path = Path::new(&directory).join(&filename);
        if path.is_file() {
            return Ok(path.to_string_lossy().into_owned());
        }
    }
    materialized_source_file(i, name, key, source)
}

/// Definition provenance must use shipped source, not a cwd/lisp override.
pub(crate) fn dump_source_file(i: &mut Interp, name: &str) -> Result<String, crate::lisp::Flow> {
    let (key, source) = embedded_key(name)
        .ok_or_else(|| i.error(format!("Missing dumped library source: {name}")))?;
    let filename = format!("{}.el", key.replace('/', "-"));
    for directory in builtin_dirs() {
        let path = Path::new(&directory).join(&filename);
        if path.is_file() && path_is_bundled_source(&path.to_string_lossy()) {
            return Ok(path.to_string_lossy().into_owned());
        }
    }
    materialized_source_file(i, name, key, source)
}

pub(crate) fn bundled_history_path(file: &str) -> bool {
    file.starts_with("builtin:") || path_is_bundled_source(file)
}

/// Only unchanged shipped sources can be republished by a portable image.
pub(crate) fn dump_source_identity(file: &str) -> Option<&'static str> {
    let prelude = concat!(env!("REMACS_SOURCE_ROOT"), "/crates/runtime/src/lisp/prelude.el");
    let (key, source) = if file == prelude {
        ("remacs-prelude", super::prelude::PRELUDE)
    } else {
        if !path_is_bundled_source(file) {
            return None;
        }
        embedded_entry(file)?
    };
    (std::fs::read(file).ok()?.as_slice() == source.as_bytes()).then_some(key)
}

fn materialized_source_file(
    i: &mut Interp,
    name: &str,
    key: &str,
    source: &str,
) -> Result<String, crate::lisp::Flow> {
    let path = super::embedded_cache::materialize(key, source).map_err(|error| {
        i.signal_data(
            crate::lisp::sym::FILE_ERROR,
            vec![
                Value::string("Writing embedded library source"),
                Value::string(error.to_string()),
                Value::string(name),
            ],
        )
    })?;
    Ok(path.to_string_lossy().into_owned())
}

/// Normalize bundled source hits to the source that normal load uses.
pub(crate) fn located_source(
    i: &mut Interp,
    name: &str,
    path: String,
) -> Result<String, crate::lisp::Flow> {
    let stem = name.strip_suffix(".el").unwrap_or(name);
    if !i.boot_loading && stem == "subr-x" && path_is_bundled_source(&path) {
        let source = Path::new(&path).with_file_name("emacs-lisp-subr-x.el");
        if source.is_file() {
            return Ok(source.to_string_lossy().into_owned());
        }
        let (key, source) = embedded_key("emacs-lisp-subr-x").unwrap();
        return bundled_source_file(i, name, key, source);
    }
    Ok(path)
}

/// Decode file bytes for `load' like GNU's `utf-8-emacs' decode:
/// valid UTF-8 decodes normally; 0xF5..0xF7 lead bytes with three
/// continuation bytes are GNU-internal `emacs'-charset chars (codes
/// > #x10FFFF — e.g. ethio-util.el's old mule text) and map to
/// private-use chars U+E000+(code & #x1FFF), preserving distinctness
/// for `eq'/`memq'.  Other undecodable bytes become U+FFFD.
fn decode_load_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut ix = 0;
    while ix < bytes.len() {
        let b = bytes[ix];
        if (0xF5..=0xF7).contains(&b)
            && ix + 3 < bytes.len()
            && bytes[ix + 1..=ix + 3]
                .iter()
                .all(|c| (0x80..0xC0).contains(c))
        {
            let code = (((b & 0x07) as u32) << 18)
                | (((bytes[ix + 1] & 0x3F) as u32) << 12)
                | (((bytes[ix + 2] & 0x3F) as u32) << 6)
                | (bytes[ix + 3] & 0x3F) as u32;
            // GNU's charset-`emacs' codes live above #x1A0000; fold
            // into the BMP private-use area deterministically.
            out.push(char::from_u32(0xE000 + (code & 0x1FFF)).unwrap_or('\u{FFFD}'));
            ix += 4;
            continue;
        }
        // Valid UTF-8 prefix, then loop so the byte at the error
        // position gets the 0xF5..0xF7 check before falling back.
        match std::str::from_utf8(&bytes[ix..]) {
            Ok(s) => {
                out.push_str(s);
                break;
            }
            Err(e) => {
                let vlen = e.valid_up_to();
                out.push_str(std::str::from_utf8(&bytes[ix..ix + vlen]).unwrap_or(""));
                ix += vlen;
                if vlen > 0 {
                    continue;
                }
                out.push('\u{FFFD}');
                ix += e.error_len().unwrap_or(bytes.len() - ix).max(1);
            }
        }
    }
    out
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
    eval_file_lex_dumped(i, path, force_lex, false)
}

fn eval_file_lex_dumped(
    i: &mut Interp,
    path: &str,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    // Compressed files follow jka-compr's source path: only an actual
    // .elc suffix forces rejection when the compiler header is absent.
    let elc = path.ends_with(".elc");
    if !force_lex
        && elc
        && std::fs::metadata(path).is_ok_and(|metadata| !metadata.is_file() && !metadata.is_dir())
    {
        return Err(i.error_q("File `%s' was not compiled in Emacs", &[path]));
    }
    // GNU decodes the file with `utf-8-emacs' (load-coding auto-detect):
    // internal charset `emacs' chars (4-byte leads 0xF5..0xF7, codes
    // > #x10FFFF) decode without error.  Our strings can't hold those
    // codes, so decode them to distinct private-use chars — `?X'
    // literals and `eq'/`memq' comparisons stay consistent within a
    // file (ethio-util.el relies on this).
    let bytes = std::fs::read(path).map_err(|error| {
        let symbol = if error.kind() == std::io::ErrorKind::NotFound {
            i.q("file-missing")
        } else {
            crate::lisp::sym::FILE_ERROR
        };
        i.signal_data(
            symbol,
            vec![
                Value::string("Opening input file"),
                Value::string(error.to_string()),
                Value::string(path),
            ],
        )
    })?;
    let bytes = if path.ends_with(".gz") {
        gunzip(&bytes).ok_or_else(|| {
            i.signal_data(
                crate::lisp::sym::FILE_ERROR,
                vec![
                    Value::string("Opening input file"),
                    Value::string("Malformed or misplaced compressed data"),
                    Value::string(path),
                ],
            )
        })?
    } else {
        bytes
    };
    // Fload validates the compiler header before reading/evaluating forms.
    // --script uses eval-buffer instead and does not impose this check.
    let version = if force_lex {
        0
    } else {
        compiled_file_version(i, &bytes)?
    };
    if !force_lex && elc && version == 0 {
        return Err(i.error_q("File `%s' was not compiled in Emacs", &[path]));
    }
    let compiled = !force_lex && (elc || version > 1);
    let src = decode_load_bytes(&bytes);
    // Track load-file-name / load-in-progress like Emacs does.  GNU
    // binds `load-file-name' to the filename `openp' located — the
    // tried path itself, not its truename.
    eval_src_kind(i, path, &src, force_lex, dumped_like, compiled)
}

/// GNU safe_to_load_version: inspect at most 512 raw bytes, searching
/// after the first newline. Empty files return 1; zero rejects .elc.
/// The marker alone is insufficient, and historical headers remain valid.
fn compiled_file_version(i: &mut Interp, bytes: &[u8]) -> Result<i32, crate::lisp::Flow> {
    let bytes = &bytes[..bytes.len().min(512)];
    if bytes.is_empty() {
        return Ok(1);
    }
    let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') else {
        return Ok(0);
    };
    let variable = i.q("bytecomp-version-regexp");
    let pattern = i.symbol_value(variable);
    let Value::Str(pattern) = &pattern else {
        return Err(i.wrong_type_mut("stringp", &pattern));
    };
    let regexp = super::regexp::compile_case(pattern.borrow().as_str(), true).map_err(|error| {
        let symbol = i.q("invalid-regexp");
        i.signal_data(symbol, vec![Value::string(error.0)])
    })?;
    let chars: Vec<_> = bytes[newline..]
        .iter()
        .map(|byte| char::from(*byte))
        .collect();
    let syntax = crate::editor::re_syntax(i);
    if super::regexp::search(
        &regexp,
        &chars,
        0,
        &syntax,
        &crate::editor::re_category(i),
        super::regexp::NO_POINT,
    )
    .is_none()
    {
        return Ok(0);
    }
    Ok(if newline > 4 {
        i32::from(bytes[4] as i8)
    } else {
        1
    })
}

/// Decompress gzip data; None on malformed input.
fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut d = flate2::read::GzDecoder::new(bytes);
    let mut out = Vec::new();
    d.read_to_end(&mut out).ok().map(|_| out)
}

/// Evaluate SRC as if loaded from file FILE (binds load-file-name,
/// load-in-progress, lexical-binding cookie; runs after-load hooks).
fn eval_src(i: &mut Interp, file: &str, src: &str, force_lex: bool) -> EvalResult {
    eval_src_opts(i, file, src, force_lex, false)
}

fn eval_src_opts(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    let compiled = src.starts_with(";ELC\u{1f}\u{0}\u{0}\u{0}");
    eval_src_kind(i, file, src, force_lex, dumped_like, compiled)
}

fn eval_src_kind(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
    compiled: bool,
) -> EvalResult {
    // Fload resets lexical binding before entering the source converter.
    let depth = i.specbind_depth();
    let lexical = i.q("lexical-binding");
    let result = (|| {
        i.specbind(lexical, Value::Nil)?;
        eval_src_bound(i, file, src, force_lex, dumped_like, compiled)
    })();
    let result = i.offer_flow(result);
    i.unbind_to(depth).and(result)
}

fn eval_src_bound(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
    compiled: bool,
) -> EvalResult {
    // On-disk source loads use GNU's load-with-code-conversion buffer.
    // Boot/embedded libraries model dumped code, and compiled streams
    // have their own direct reader, so neither acquires this buffer.
    // --script's distinct current-buffer/shebang rules stay on its existing path.
    let source = !force_lex && !i.boot_loading && !file.starts_with("builtin:") && !compiled;
    if source {
        let flags = i
            .lisp_stack
            .iter()
            .rev()
            .find_map(|(function, args)| {
                let function = i.indirect_function_value(function);
                match function {
                    Value::Subr(subr) if subr.name == "load" => Some((
                        args.get(1).cloned().unwrap_or(Value::Nil),
                        args.get(2).cloned().unwrap_or(Value::Nil),
                    )),
                    Value::Subr(subr) if subr.name == "require" => {
                        Some((args.get(2).cloned().unwrap_or(Value::Nil), Value::t()))
                    }
                    _ => None,
                }
            })
            .unwrap_or((Value::Nil, Value::t()));
        let args = vec![
            Value::string(file),
            Value::string(file),
            Value::from_bool(flags.0.truthy()),
            Value::from_bool(flags.1.truthy()),
        ];
        i.with_call_frame("load-with-code-conversion", args, |i| {
            eval_src_in_context(i, file, src, force_lex, dumped_like, source, compiled)
        })
    } else {
        eval_src_in_context(i, file, src, force_lex, dumped_like, source, compiled)
    }
}

fn eval_src_in_context(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
    source: bool,
    compiled: bool,
) -> EvalResult {
    let load_buffer = if source {
        let dir_sym = i.q("default-directory");
        let dir = i.symbol_value(dir_sym);
        let generate = Value::Sym(i.q("generate-new-buffer"));
        let value = i.apply(&generate, vec![Value::string(" *load*")])?;
        let Value::Buffer(buffer) = value else {
            return Err(i.wrong_type_mut("bufferp", &value));
        };
        let multi = i.q("enable-multibyte-characters");
        {
            let mut b = buffer.borrow_mut();
            b.locals.insert(dir_sym, dir);
            b.locals.insert(multi, Value::t());
            b.insert(src);
            b.set_point(0);
        }
        Some(buffer)
    } else {
        None
    };
    let saved_excursion = load_buffer.as_ref().map(|_| i.save_excursion_state(false));
    let saved_stream = i.load_stream.clone();
    let saved_lexenv = i.lexenv.clone();
    let mark = i.specbind_depth();
    let result = (|| {
        if load_buffer.is_none() {
            if i.load_stream_symbol.is_none() {
                let symbol = i.make_symbol("get-file-char");
                i.load_stream_symbol = Some(symbol);
            }
            i.load_stream = Some(Rc::new(RefCell::new(LoadStream {
                chars: Rc::new(src.chars().collect()),
                position: 0,
            })));
        }
        eval_src_opts_inner(
            i,
            file,
            src,
            force_lex,
            dumped_like,
            load_buffer.as_ref(),
            compiled,
        )
    })();
    let result = i.offer_flow(result);
    i.lexenv = saved_lexenv;
    if let Some(saved) = saved_excursion {
        i.restore_excursion_state(saved);
    }
    let unwind = i.unbind_to(mark);
    i.load_stream = saved_stream;
    if let Some(buffer) = load_buffer {
        // As in mule.el's unwind-protect, suppress kill queries/hooks but
        // retain ordinary buffer-list update notifications.
        let cleanup_mark = i.specbind_depth();
        let cleanup = (|| {
            for name in ["kill-buffer-hook", "kill-buffer-query-functions"] {
                let symbol = i.intern(name);
                i.specbind(symbol, Value::Nil)?;
            }
            let kill = Value::Sym(i.q("kill-buffer"));
            i.apply(&kill, vec![Value::Buffer(buffer)])
        })();
        i.unbind_to(cleanup_mark)?;
        cleanup?;
    }
    unwind?;
    if result.is_ok() {
        run_after_load(i, file);
    }
    result
}
fn eval_src_opts_inner(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
    load_buffer: Option<&crate::lisp::value::BufferRef>,
    compiled: bool,
) -> EvalResult {
    let lfn = i.q("load-file-name");
    let ltfn = i.q("load-true-file-name");
    let lip = i.q("load-in-progress");
    let cll = i.q("current-load-list");
    let mark = i.specbind_depth();
    let filename = Value::string(file);
    i.specbind(lfn, filename.clone())?;
    // GNU lread.c: `load-true-file-name' binds `found' — openp's hit
    // (load-path-expanded, NOT canonicalized; /tmp stays /tmp on macOS
    // since openp's path isn't a symlink resolution).
    i.specbind(ltfn, filename)?;
    i.specbind(lip, Value::t())?;
    if let Some(buffer) = load_buffer {
        // mule.el discovers file shorthands in the source buffer before
        // eval-buffer's input/output scopes and lexical-cookie callback.
        let automatic = i.q("set-auto-coding-for-load");
        i.specbind(automatic, Value::t())?;
        let operation = i.q("inhibit-file-name-operation");
        i.specbind(operation, Value::Nil)?;
        let caller = i.current_buffer;
        i.set_current_buffer(buffer.borrow().id);
        let hook = i.q("hack-read-symbol-shorthands-function");
        let function = i.symbol_value(hook);
        let result = if function.is_nil() { Ok(Value::Nil) }
                     else { i.apply(&function, vec![]) };
        let result = i.offer_flow(result);
        if i.buffer_live(caller) { i.set_current_buffer(caller); }
        let shorthands = result?;
        let variable = i.q("read-symbol-shorthands");
        i.specbind(variable, shorthands)?;
        buffer.borrow_mut().note_modified(false);
        let list = i.q("eval-buffer-list");
        let old = i.symbol_value(list);
        i.specbind(list, Value::cons(Value::Buffer(buffer.clone()), old))?;
        let output = i.q("standard-output");
        i.specbind(output, Value::Nil)?;
    }
    // Source conversion uses eval-buffer's cookie/default protocol. Direct
    // compiled streams read a cookie, then the toplevel default without
    // calling the fallback hook. Keep the hook's actual value, not just t.
    let lex_id = i.q("lexical-binding");
    if let Some(buffer) = load_buffer {
        if !buffer.borrow().locals.contains_key(&lex_id) {
            let (cookie, consumed) = {
                let b = buffer.borrow();
                lexical_cookie(&b.text.substring(b.begv, b.zv))
            };
            let begv = buffer.borrow().begv;
            buffer.borrow_mut().set_point(begv + consumed);
            let value = lexical_binding_value(i, cookie, Some(Value::Buffer(buffer.clone())))?;
            i.specbind(lex_id, value)?;
        }
        let begv = buffer.borrow().begv;
        buffer.borrow_mut().set_point(begv);
    } else {
        let (cookie, consumed) = lexical_cookie(src);
        if let Some(stream) = &i.load_stream {
            stream.borrow_mut().position = consumed;
        }
        let value = if force_lex {
            Value::t()
        } else {
            let from = (!compiled && !i.boot_loading && !file.starts_with("builtin:"))
                .then(|| Value::string(file));
            lexical_binding_value(i, cookie, from)?
        };
        i.set_symbol(lex_id, value)?;
    }
    let lex_on = i.symbol_value(lex_id).truthy();
    let input = i.q("standard-input");
    let stream = match load_buffer {
        Some(buffer) => Value::Buffer(buffer.clone()),
        None => Value::Sym(i.load_stream_symbol.expect("direct load stream symbol")),
    };
    i.specbind(input, stream)?;
    if file.ends_with(".elc") {
        let variable = i.q("read-symbol-shorthands");
        i.specbind(variable, Value::Nil)?;
    }
    // Avoid self-autoloading the source optimizer during compiler bootstrap.
    // Ordinary post-boot loads retain the caller's actual optimizer setting.
    let optimizer_source = |path: &str| {
        (path.starts_with("builtin:") || path_is_bundled_source(path))
            && embedded_entry(path.strip_prefix("builtin:").unwrap_or(path))
                .is_some_and(|(key, _)| key == "byte-opt")
    };
    let bo = i.q("byte-optimize");
    if i.bound_p(bo) && (i.boot_loading || optimizer_source(file)
        || i.loads_in_progress.iter().any(|path| optimizer_source(path))) {
        i.specbind(bo, Value::Nil)?;
    }
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
    // NOTE: `features' is a plain variable whose current (possibly
    // dynamically bound) value `provide' pushes onto — GNU has no
    // second feature set, so nothing is synced from `i.features' here.
    // The interpreter's `features' vec is only a boot-time ledger for
    // the dump/prelude bookkeeping.
    i.specbind(cll, Value::list(vec![Value::string(file)]))?;
    // GNU readevalloop keeps top-level dynamic declarations local to
    // this load, while allowing successive forms to share them.
    let dynvars = i.q("macroexp--dynvars");
    if i.bound_p(dynvars) {
        i.specbind(dynvars, i.symbol_value(dynvars))?;
    }
    // Embedded (`builtin:') libraries play the role of GNU's dumped
    // .elc files: functions defined while they load keep
    // `dumped_doc' docstring semantics.
    let was_dumped = i.loading_dumped;
    i.loading_dumped |= dumped_like || file.starts_with("builtin:");
    // A validated compiled stream marks `#[...]' literals read from it
    // as compiled functions (GNU's byte-code objects); combined with
    // `loading_dumped' they present the nativecomp `subr' surface.
    let was_elc = i.reading_elc;
    i.reading_elc = was_elc || compiled;
    // GNU lread.c Fload: `found' is consed onto Vloads_in_progress
    // BEFORE the recursion check, so the signal's datum — `(found .
    // loads-in-progress)' — contains the offending entry.  The error
    // fires when the count exceeds 3 (the 4th nested load of the same
    // path); files that never finish loading would otherwise recurse
    // to max-lisp-eval-depth.
    i.loads_in_progress.push(file.to_string());
    if i.loads_in_progress.iter().filter(|f| *f == file).count() > 3 {
        i.loads_in_progress.pop();
        i.unbind_to(mark)?;
        i.lexenv = saved_lexenv;
        i.reading_elc = was_elc;
        i.loading_dumped = was_dumped;
        let mut data = vec![Value::string("Recursive load"), Value::string(file)];
        // Vloads_in_progress is newest-first in GNU; our Vec pushes
        // newest last, so iterate in reverse.
        for f in i.loads_in_progress.iter().rev() {
            data.push(Value::string(f.clone()));
        }
        return Err(i.signal_data(crate::lisp::sym::ERROR, data));
    }
    let compiled_library = i.loading_compiled_library;
    let r = i.with_eval_context(compiled_library, |i| {
        eval_str_for_load(i, file, &src, load_buffer, compiled)
    });
    // Reader failures are raised here too, while all load bindings and
    // the source buffer still belong to the signaling context.
    let r = i.offer_flow(r);
    let popped = i.loads_in_progress.pop();
    debug_assert!(popped.as_deref() == Some(file));
    i.reading_elc = was_elc;
    i.loading_dumped = was_dumped;
    // File-scoped `defvar' declarations live on the root frame; merge
    // them into `file_declared' before unwinding so `let's in later
    // code bind those names dynamically — what the byte-compiled
    // equivalent of the file would produce.
    if let Some(root) = &i.lexenv {
        let mut fd = i.file_declared.borrow_mut();
        for s in root.declared.borrow().iter() {
            fd.insert(*s);
        }
    }
    i.lexenv = saved_lexenv;
    // Keep dynamic load bindings balanced if history comparison signals.
    let r = (|| {
        if r.is_ok() {
            build_history(i, Value::string(file), true)?;
        }
        r
    })();
    i.unbind_to(mark)?;
    r
}

/// Finish GNU's readevalloop history. The filename marker is the last
/// element of current-load-list and becomes the entry's car on reversal.
pub(crate) fn build_history(i: &mut Interp, filename: Value, entire: bool) -> EvalResult {
    let variable = i.q("load-history");
    let current_variable = i.q("current-load-list");
    let current = i.symbol_value(current_variable);
    let new_items = crate::lisp::builtins::want_list(i, &current)?;
    let mut root = i.symbol_value(variable);
    if matches!(root, Value::Sym(symbol) if symbol == crate::lisp::sym::UNBOUND) {
        root = Value::Nil;
    }
    let _ = crate::lisp::builtins::want_list(i, &root)?;
    let mut tail = root.clone();
    let mut previous: Option<crate::lisp::value::ConsRef> = None;
    let mut found = false;
    while let Value::Cons(cell) = tail {
        let (entry, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let (key, entries) = match &entry {
            Value::Cons(entry) => {
                let entry = entry.borrow();
                (entry.car.clone(), entry.cdr.clone())
            }
            Value::Nil => (Value::Nil, Value::Nil),
            _ => return Err(i.wrong_type_mut("listp", &entry)),
        };
        if crate::lisp::builtins::equal_values(i, &key, &filename)? {
            found = true;
            if entire {
                if let Some(previous) = &previous {
                    previous.borrow_mut().cdr = next.clone();
                } else {
                    root = next.clone();
                }
            } else {
                // GNU compares each new item against the original entry
                // snapshot when merging a partially evaluated buffer.
                let old_items = crate::lisp::builtins::want_list(i, &entry)?;
                for item in &new_items {
                    let mut present = false;
                    for old in &old_items {
                        if crate::lisp::builtins::equal_values(i, item, old)? {
                            present = true;
                            break;
                        }
                    }
                    if !present {
                        cell.borrow_mut().car =
                            Value::cons(key.clone(), Value::cons(item.clone(), entries.clone()));
                    }
                }
                previous = Some(cell.clone());
            }
        } else {
            previous = Some(cell.clone());
        }
        tail = next;
    }
    if entire || !found {
        let entry = crate::lisp::builtins::seq::f_nreverse_seq(i, vec![current].into())?;
        root = Value::cons(entry, root);
        i.obarray.symbol_mut(current_variable).value = Value::t();
    }
    i.obarray.symbol_mut(variable).value = root;
    Ok(Value::Nil)
}

/// GNU's `do-after-load-evaluation' (subr.el): run the matching
/// `after-load-alist' string keys plus `after-load-functions' for the
/// file just loaded.  Early boot loads skip it: nothing is registered
/// yet and the defun doesn't exist yet.
fn run_after_load(i: &mut Interp, file: &str) {
    if let Some(dale) = i.q_soft("do-after-load-evaluation") {
        if i.fbound_p(dale) {
            let _ = i.apply(&Value::Sym(dale), vec![Value::string(file)]);
        }
    }
}

/// Read and eval each top-level form, eagerly expanding macros the way
/// GNU's `internal-macroexpand-for-load' does during `load'.  This is
/// what resolves macro autoloads (e.g. `define-minor-mode') at load
/// time rather than first call.
fn eval_str_for_load(
    i: &mut Interp,
    file: &str,
    src: &str,
    load_buffer: Option<&crate::lisp::value::BufferRef>,
    compiled: bool,
) -> EvalResult {
    let native_bridges = native_backend_bridges(i, file, src);
    let stream = if load_buffer.is_none() {
        i.load_stream.clone()
    } else {
        None
    };
    let mut chars: Rc<Vec<char>> = stream
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
                let (read, end) = crate::lisp::builtins::readfn::read_parser_at(
                    i, chars.clone(), pos, None, load_buffer.is_some(), source_multibyte,
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
                            i, Err(error), Some(context),
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
                        && matches!(chars[p], ' ' | '\t' | '\n' | '\x0C' | '\r' | '\u{A0}')
                    {
                        p += 1;
                    }
                    if p < chars.len() && chars[p] == ';' {
                        while p < chars.len() && chars[p] != '\n' {
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
                        && matches!(chars[p], ' ' | '\t' | '\n' | '\x0C' | '\r' | '\u{A0}')
                    {
                        p += 1;
                    }
                    if p < chars.len() && chars[p] == ';' {
                        while p < chars.len() && chars[p] != '\n' {
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
                    i.princ_to_string(form).chars().take(60).collect::<String>()
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
                                        .collect::<String>()
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
                                    i.prin1_to_string(&signal.data()).chars().take(120).collect::<String>()
                                ),
                                other => format!("{:?}", other),
                            };
                            eprintln!(
                                "[load-err@{}] {} => {}",
                                end,
                                i.princ_to_string(&form)
                                    .chars()
                                    .take(120)
                                    .collect::<String>(),
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
    file: &str,
    source: &str,
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

/// Record a `load-history' entry for the embedded startup prelude so
/// `symbol-file' and the find-func family can locate prelude-defined
/// symbols.  GNU records every dumped library in `load-history' the same
/// way; FILE names the on-disk copy of the prelude source.
pub(crate) fn record_prelude_load_history(i: &mut Interp, file: &str, src: &str) {
    let cll = i.q("current-load-list");
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
    let lh = i.q("load-history");
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
    let quote_id = i.q("quote");
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
    let defun_id = i.q("defun");
    let defface_id = i.q("defface");
    let require_id = i.q("require");
    let entry = match head_name.as_str() {
        h if DEFUN_HEADS.contains(&h) => name(&items).map(|nm| Value::cons(i.sym(defun_id), nm)),
        h if DEFVAR_HEADS.contains(&h) => name(&items),
        // `defface' expands to `custom-declare-face' before eval; GNU's
        // C subr attaches (defface . FACE) to `current-load-list'.
        "defface" | "custom-declare-face" => {
            name(&items).map(|nm| Value::cons(i.sym(defface_id), nm))
        }
        // `provide' is NOT recorded by the form reader in GNU: Fprovide
        // does its own LOADHIST_ATTACH (catches nested provides too) —
        // recording here too would push a duplicate entry.
        "provide" => None,
        "require" => name(&items).map(|nm| Value::cons(i.sym(require_id), nm)),
        _ => None,
    };
    if let Some(e) = entry {
        let cll = i.q("current-load-list");
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
            if *s == i.q("progn") {
                return cb.cdr.list_to_vec().ok();
            }
        }
    }
    None
}

/// Parse GNU lread.c's first-line cookie, including its 99-byte fields
/// and non-overlapping marker state machine. The consumed position is in
/// characters and includes the comment line and any initial shebang.
pub(crate) fn lexical_cookie(source: &str) -> (Option<bool>, usize) {
    fn marker(ch: char, state: &mut u8) -> bool {
        match *state {
            0 => *state = u8::from(ch == '-'),
            1 => *state = if ch == '*' { 2 } else { 0 },
            _ => {
                *state = 0;
                return ch == '-';
            }
        }
        false
    }
    fn trim(field: &mut Vec<u8>) {
        while matches!(field.last(), Some(b' ' | b'\t')) {
            field.pop();
        }
    }
    fn c_string(field: &[u8]) -> &[u8] {
        field.split(|byte| *byte == 0).next().unwrap_or_default()
    }
    let start = if source.starts_with("#!") {
        source.find('\n').map_or(source.len(), |end| end + 1)
    } else {
        0
    };
    let preceding = source[..start].chars().count();
    let rest = &source[start..];
    if !rest.starts_with(';') {
        return (None, preceding);
    }
    let (line, newline) = rest.split_once('\n').map_or((rest, 0), |(line, _)| (line, 1));
    let consumed = preceding + line.chars().count() + newline;
    let mut chars = line.chars().skip(1);
    let mut state = 0;
    let mut in_variables = false;
    for ch in chars.by_ref() {
        if marker(ch, &mut state) {
            in_variables = true;
            break;
        }
    }
    while in_variables {
        let mut ch = chars.next();
        while matches!(ch, Some(' ' | '\t')) {
            ch = chars.next();
        }
        let mut name = Vec::new();
        state = 0;
        while let Some(character) = ch {
            if character == ':' || !in_variables {
                break;
            }
            if name.len() < 99 {
                name.push(character as u8);
            }
            if marker(character, &mut state) {
                in_variables = false;
            }
            ch = chars.next();
        }
        if !in_variables || ch.is_none() {
            break;
        }
        trim(&mut name);
        ch = chars.next();
        while matches!(ch, Some(' ' | '\t')) {
            ch = chars.next();
        }
        let mut value = Vec::new();
        state = 0;
        while let Some(character) = ch {
            if character == ';' || !in_variables {
                break;
            }
            if value.len() < 99 {
                value.push(character as u8);
            }
            if marker(character, &mut state) {
                in_variables = false;
            }
            ch = chars.next();
        }
        if !in_variables {
            value.truncate(value.len().saturating_sub(3));
        }
        trim(&mut value);
        if c_string(&name) == b"lexical-binding" {
            return (Some(c_string(&value) != b"nil"), consumed);
        }
    }
    (None, consumed)
}

/// Resolve a scanned cookie through GNU's observable default callback.
/// FROM=None represents a direct compiled stream, which bypasses that hook.
pub(crate) fn lexical_binding_value(
    i: &mut Interp,
    cookie: Option<bool>,
    from: Option<Value>,
) -> EvalResult {
    if let Some(enabled) = cookie {
        return Ok(Value::from_bool(enabled));
    }
    if let Some(from) = from {
        let hook = i.q("internal--get-default-lexical-binding-function");
        let function = i.symbol_value(hook);
        if !function.is_nil() {
            return i.apply(&function, vec![from]);
        }
    }
    let lexical = i.q("lexical-binding");
    Ok(i.default_toplevel_value(lexical).unwrap_or(Value::Nil))
}

/// Outcome of a `load' attempt.
pub(crate) enum LoadOutcome {
    /// The file was found and evaluated to completion — payload is the
    /// resolved file name (what `load-file-name' was bound to).
    Loaded(String),
    /// No candidate matched (GNU's openp returning -2).
    Missing,
    /// A directory matched: GNU's openp opens it and `Fload' then
    /// fstat's it into a file-error "Is a directory".  Kept out of the
    /// `Err' channel so `load' NOERROR can suppress just this class.
    Dir(String),
}

/// Evaluate GNU's verbatim subr-x.el (`lisp/emacs-lisp-subr-x.el') as
/// FILE: the on-disk copy, or the embedded source for `builtin:' names.
fn eval_gnu_subr_x(
    i: &mut Interp,
    file: &str,
    dumped_like: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    if !file.starts_with("builtin:") {
        return eval_file_lex_dumped(i, file, false, dumped_like)
            .map(|_| LoadOutcome::Loaded(file.to_string()));
    }
    match embedded("emacs-lisp-subr-x") {
        Some(src) => eval_src(i, file, src, false).map(|_| LoadOutcome::Loaded(file.to_string())),
        None => Ok(LoadOutcome::Missing),
    }
}

/// Every live (non-void, non-autoload) function cell.
fn live_definitions(i: &Interp) -> std::collections::HashMap<crate::lisp::SymId, Value> {
    let autoload = i.q_soft("autoload");
    i.obarray
        .all_ids()
        .into_iter()
        .filter_map(|id| {
            let f = &i.obarray.symbol(id).function;
            let live = match f {
                Value::Nil => false,
                Value::Sym(s) => *s != crate::lisp::sym::UNBOUND,
                Value::Cons(c) => !matches!(&c.borrow().car, Value::Sym(s) if Some(*s) == autoload),
                _ => true,
            };
            live.then(|| (id, f.clone()))
        })
        .collect()
}

/// GNU's subr-x only fills cells that are void or autoloads at -Q.
/// Names it redefines that already had a live definition here (Rust
/// subr shadows, dump-time compiled twins) keep it, so loading subr-x
/// never swaps a working function for another implementation.  FILE's
/// `(defun . NAME)' load-history items list what the load defined.
fn keep_live_definitions(
    i: &mut Interp,
    file: &str,
    live: &std::collections::HashMap<crate::lisp::SymId, Value>,
) {
    let lh = i.q("load-history");
    let defun = i.q("defun");
    let entry = i
        .symbol_value(lh)
        .list_to_vec()
        .unwrap_or_default()
        .into_iter()
        .find(|e| {
            matches!(e, Value::Cons(c)
                     if matches!(&c.borrow().car, Value::Str(s) if s.borrow().as_str() == file))
        });
    let Some(Value::Cons(entry)) = entry else {
        return;
    };
    let items = entry.borrow().cdr.list_to_vec().unwrap_or_default();
    for item in items {
        let Value::Cons(c) = &item else { continue };
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if !matches!(car, Value::Sym(s) if s == defun) {
            continue;
        }
        if let Some(old) = i.sym_id(&cdr).and_then(|id| live.get(&id).map(|v| (id, v))) {
            i.obarray.symbol_mut(old.0).function = old.1.clone();
        }
    }
}

/// Load library NAME; returns Ok(Loaded(resolved-path)) on success.
pub(crate) fn load_library(i: &mut Interp, name: &str) -> Result<LoadOutcome, crate::lisp::Flow> {
    load_library_opts_msg(i, name, false, false, true, false)
}

/// `load_library' honoring `load''s NOSUFFIX and MUST-SUFFIX.  Like
/// GNU, `Loading %s...' is echoed (through `message') before the
/// file's forms run; ` (source)' marks files read from Lisp source
/// rather than byte-compiled output.
pub(crate) fn load_library_opts(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    mustsuffix: bool,
    can_elc: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    load_library_opts_msg(i, name, nosuffix, mustsuffix, can_elc, false)
}

/// Full form: NOMESSAGE suppresses the `Loading %s...' echo (GNU
/// Fload NOMESSAGE — autoload loads pass t).
pub(crate) fn load_library_opts_msg(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    mustsuffix: bool,
    can_elc: bool,
    nomessage: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    let t0 = std::time::Instant::now();
    let announce = |i: &mut Interp, file: &str| {
        // Boot-library loads stand in for GNU's loadup.el, which runs at
        // dump time — `emacs -Q --batch' echoes none of them.
        if nomessage || i.boot_loading {
            return;
        }
        let fmt = if file.ends_with(".el") {
            "Loading %s (source)..."
        } else {
            "Loading %s..."
        };
        let msg = i.q("message");
        let _ = i.apply(
            &Value::Sym(msg),
            vec![Value::string(fmt), Value::string(file)],
        );
    };
    // A bundled library stands in for GNU's shipped .elc: its
    // macroexpansion ran at byte-compile time, so the `gensym-counter'
    // bumps our interpreted load performs must not leak into the
    // session.  Save/restore it around the load.
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    // GNU ships shortdoc's definitions as compiled data.  Eagerly expanding
    // that data would autoload shortdoc, which requires the same file again.
    // Other compiled twins remain restricted to the boot window.
    let compiled_twin_allowed =
        i.boot_loading || (stem == "shortdoc-doc" && !name.ends_with(".el"));
    let found = locate_full(i, name, nosuffix, mustsuffix);
    let bundled = match &found {
        Some(Found::File(path)) => path_is_bundled_source(path) && embedded(path).is_some(),
        Some(Found::Dir(_)) => false,
        None => embedded(stem).is_some(),
    };
    let gc = i.q("gensym-counter");
    let saved_gc = bundled.then(|| i.symbol_value(gc));
    // Post-boot `subr-x' reads GNU's verbatim subr-x.el, not the
    // bundled lisp/subr-x.el: that file amalgamates the dump-time
    // compatibility definitions (loaded once at boot) with GNU's
    // subr-x, so re-reading it re-ran ~2300 definitions and its
    // top-level setq/defconst/add-hook forms — replacing subr shadows
    // and later libraries' definitions (replace.el's
    // `replace-highlight') and resetting live state
    // (`file-name-handler-alist', `electric-quote-string').
    let saved_compiled_library = i.loading_compiled_library;
    i.loading_compiled_library = can_elc
        && bundled
        && !nosuffix
        && !name.ends_with(".el")
        && !name.ends_with(".elc")
        && match &found {
            Some(Found::File(path)) => path_is_bundled_source(path),
            Some(Found::Dir(_)) => false,
            None => true,
        };
    let gnu_subr_x = !i.boot_loading
        && ((stem == "subr-x" && !name.contains('/'))
            || matches!(&found, Some(Found::File(path)) if path_is_bundled_source(path)
                && matches!(embedded_entry(path), Some(("subr-x" | "emacs-lisp-subr-x", _)))));
    let live_defs = gnu_subr_x.then(|| live_definitions(i));
    let result = match found {
        Some(Found::Dir(d)) => Ok(LoadOutcome::Dir(d)),
        Some(Found::File(path)) if gnu_subr_x && path_is_bundled_source(&path) => {
            let real = Path::new(&path).with_file_name("emacs-lisp-subr-x.el");
            let file = if real.is_file() {
                real.to_string_lossy().into_owned()
            } else {
                "builtin:subr-x".to_string()
            };
            announce(i, &file);
            eval_gnu_subr_x(i, &file, bundled)
        }
        Some(Found::File(path)) => {
            // Boot twins stand in for GNU's dumped bytecode.  The data-only
            // shortdoc-doc twin is also used after boot, avoiding its source
            // macro's mutual dependency with shortdoc.  Other libraries keep
            // their post-boot source behavior.  Explicit source loads, user
            // overrides, load-prefer-newer and REMACS_NO_ELC retain the source.
            let use_elc = can_elc
                && compiled_twin_allowed
                && std::env::var_os("REMACS_NO_ELC").is_none()
                && !nosuffix
                && !mustsuffix
                && path.ends_with(".el")
                && !name.contains('/')
                && !name.ends_with(".el")
                && path_is_bundled_source(&path)
                && embedded_elc(stem).is_some()
                && !i
                    .symbol_value(i.q_soft("load-prefer-newer").unwrap_or(0))
                    .truthy();
            if use_elc {
                let src = embedded_elc(stem).unwrap();
                let virtual_path = format!("builtin:{}", stem);
                announce(i, &virtual_path);
                // `.elc' top-level forms eval dynamically in GNU
                // (`loading_dumped' still marks defs compiled);
                // lexical env lives inside each `#[...]' constants
                // vector, not in the file's eval scope.  force_lex
                // would make the file's bare `defvar's file-scoped
                // lexicals that later varrefs can't see.
                let r = eval_src_opts(i, &virtual_path, src, false, true)
                    .map(|_| LoadOutcome::Loaded(virtual_path.clone()));
                match r {
                    Ok(o) => Ok(o),
                    Err(f) => {
                        // The AOT blob was compiled against GNU's full
                        // boot order — calls to helpers our prelude
                        // guards differently (cl-generic-define before
                        // cl-generic loads, GNU-only subrs like
                        // `easy-menu-do-define') signal during eval.
                        // Fall back to the interpreted `.el' source,
                        // which carries the prelude's compat shims.
                        if std::env::var_os("REMACS_DEBUG_ELC").is_some() {
                            eprintln!(
                                "[elc-fail] {stem}: {} | head={}",
                                match &f {
                                    crate::lisp::Flow::Signal(signal) => format!(
                                        "{} {}",
                                        i.prin1_to_string(&signal.symbol()),
                                        i.prin1_to_string(&signal.data())
                                    ),
                                    o => format!("{o:?}"),
                                },
                                i.princ_to_string(&i.cur_head_form)
                                    .chars()
                                    .take(160)
                                    .collect::<String>()
                            );
                        }
                        announce(i, &path);
                        eval_file_lex_dumped(i, &path, false, bundled)
                            .map(|_| LoadOutcome::Loaded(path))
                    }
                }
            } else {
                announce(i, &path);
                if can_elc && path.ends_with(".eln") {
                    // Native-compiled unit: register via dlopen instead of
                    // evaluating source.  GNU pushes `found' onto
                    // Vloads_in_progress for these loads too.
                    i.loads_in_progress.push(path.clone());
                    let r = crate::lisp::builtins::comp::native_load_file(i, &path)
                        .map(|_| LoadOutcome::Loaded(path.clone()));
                    i.loads_in_progress.pop();
                    r
                } else {
                    eval_file_lex_dumped(i, &path, false, bundled)
                        .map(|_| LoadOutcome::Loaded(path))
                }
            }
        }
        // Fall back to the embedded copy of a built-in library, so that
        // autoloads work even when the lisp/ dir isn't reachable by path.
        // A `lisp-elc' blob wins during boot or for shortdoc's compiled data.
        // Other twins remain scoped to boot, preserving post-boot source
        // definitions.  REMACS_NO_ELC disables implicit compiled twins.
        None if gnu_subr_x => {
            announce(i, name);
            eval_gnu_subr_x(i, "builtin:subr-x", bundled)
        }
        None => match embedded_elc(stem).filter(|_| {
            can_elc
                && compiled_twin_allowed
                && std::env::var_os("REMACS_NO_ELC").is_none()
        }) {
            Some(src) => {
                announce(i, name);
                let virtual_path = format!("builtin:{}", stem);
                // `.elc' forms eval dynamically in GNU — no force_lex.
                match eval_src_opts(i, &virtual_path, src, false, true) {
                    Ok(_) => Ok(LoadOutcome::Loaded(virtual_path)),
                    Err(f) => {
                        // Same fallback as the Found::File arm: an
                        // AOT blob that hits a GNU-only call or
                        // boot-order dependency retries as source.
                        match embedded(name) {
                            Some(src) => {
                                announce(i, name);
                                eval_src(i, &virtual_path, src, false)
                                    .map(|_| LoadOutcome::Loaded(virtual_path))
                            }
                            None => Err(f),
                        }
                    }
                }
            }
            None => match embedded(name) {
                Some(src) => {
                    announce(i, name);
                    let virtual_path = format!("builtin:{}", name);
                    eval_src(i, &virtual_path, src, false)
                        .map(|_| LoadOutcome::Loaded(virtual_path))
                }
                None => Ok(LoadOutcome::Missing),
            },
        },
    };
    i.loading_compiled_library = saved_compiled_library;
    if let (Some(live), Ok(LoadOutcome::Loaded(file))) = (&live_defs, &result) {
        keep_live_definitions(i, file, live);
    }
    if let Some(saved) = &saved_gc {
        // A file that DEFVARs `gensym-counter' itself (subr.el does)
        // legitimately changes the cell from UNBOUND: restoring would
        // unbind it again and break later loads' macroexpansion.
        if matches!(saved, Value::Sym(s) if *s == crate::lisp::sym::UNBOUND)
            && !matches!(
                i.obarray.symbol(gc).value,
                Value::Sym(s) if s == crate::lisp::sym::UNBOUND
            )
        {
            return result;
        }
    }
    if let Some(v) = saved_gc {
        i.obarray.symbol_mut(gc).value = v;
    }
    if std::env::var_os("REMACS_LOAD_DEBUG").is_some() {
        let ms = t0.elapsed().as_millis();
        match &result {
            Err(f) => {
                let msg = match f {
                    crate::lisp::Flow::Signal(signal) => {
                        let sig = signal.symbol();
                        let data = signal.data();
                        format!(
                            "signal {} {}",
                            i.print_to_string(&sig),
                            i.print_to_string(&data)
                        )
                    }
                    crate::lisp::Flow::Throw(t, _) => {
                        format!("throw {}", i.print_to_string(t))
                    }
                    _ => "other flow".to_string(),
                };
                eprintln!("[load-debug] {name}: {msg} ({ms}ms)");
            }
            Ok(o) => {
                let tag = match o {
                    LoadOutcome::Loaded(p) => format!("loaded {p}"),
                    LoadOutcome::Missing => "missing".to_string(),
                    LoadOutcome::Dir(d) => format!("dir {d}"),
                };
                eprintln!("[load-debug] {name}: {tag} ({ms}ms)");
            }
        }
    }
    result
}
