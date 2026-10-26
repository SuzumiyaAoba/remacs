//! Load-path search, source identity, and embedded-source publication.

use crate::lisp::{Interp, value::Value};
use std::path::{Path, PathBuf};

pub(crate) fn unicode_name<T: crate::lisp::text::TextSource + ?Sized>(name: &T) -> Option<String> {
    name.with_text(|text| text.chars().map(|character| character.unicode()).collect())
}

pub(crate) fn filename_text(i: &Interp, value: &Value) -> Option<crate::lisp::text::EmacsText> {
    let Value::Str(name) = value else {
        return None;
    };
    let unibyte = i.is_unibyte_str(name);
    Some(
        name.borrow()
            .chars()
            .map(|character| {
                if unibyte && (128..=255).contains(&character.code()) {
                    crate::lisp::text::EmacsChar::new(0x3fff00 + character.code()).unwrap()
                } else {
                    character
                }
            })
            .collect(),
    )
}

/// Candidate paths GNU's `openp' would try for NAME in DIR.
/// NOSUFFIX opens NAME literally; MUST-SUFFIX requires NAME to carry
/// a recognized `.el'/`.elc' suffix already.
fn candidates(
    dir: &Path,
    name: &crate::lisp::text::EmacsStr,
    nosuffix: bool,
    mustsuffix: bool,
) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let Ok(native_name) = name.to_os_string() else {
        return v;
    };
    let base = dir.join(&native_name);
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
            v.push(dir.join(&native_name));
        }
    }
    v
}

/// Find library file NAME on load-path (or as an absolute/relative path).
/// Directories don't count as hits here.
/// GNU `openp' result: a hit is either a file or a directory — the
/// directory case must reach `Fload' so it can signal "Is a directory".
pub(crate) enum Found {
    File(crate::lisp::text::EmacsText),
    Dir(crate::lisp::text::EmacsText),
}

/// Pick the first existing candidate; with `load-prefer-newer' the
/// newest of the file candidates in this dir wins (GNU `openp').
fn pick_existing(paths: Vec<PathBuf>, prefer_newer: bool) -> Option<Found> {
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for p in paths {
        if p.is_dir() {
            // GNU `openp' returns directories too — a dir shadows any
            // later file candidate at the same position.
            return Some(Found::Dir(crate::lisp::text::EmacsText::from_os_str(
                p.as_os_str(),
            )));
        }
        if !p.is_file() {
            continue;
        }
        if !prefer_newer {
            return Some(Found::File(crate::lisp::text::EmacsText::from_os_str(
                p.as_os_str(),
            )));
        }
        let m = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &best {
            Some((_, bm)) if *bm >= m => {}
            _ => best = Some((p, m)),
        }
    }
    best.map(|(p, _)| Found::File(crate::lisp::text::EmacsText::from_os_str(p.as_os_str())))
}

/// Locate a load candidate, also reporting directory hits: GNU's `openp'
/// opens directories happily and `Fload' signals "Is a directory"
/// afterwards, so the distinction must survive the search.
pub(crate) fn locate_full<N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &N,
    nosuffix: bool,
    mustsuffix: bool,
) -> Option<Found> {
    let name = name.with_text(|text| text.to_owned());
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
        expanded = crate::editor::expand_file_name_str(i, &name);
        expanded
    } else {
        name.to_owned()
    };
    let native_name = name.to_os_string().ok()?;
    let p = Path::new(&native_name);
    let source_suffixes =
        if nosuffix || name.ends_with(".el") || name.ends_with(".elc") || name.ends_with(".eln") {
            vec![String::new()]
        } else {
            vec![".el".to_string()]
        };
    if p.is_absolute() || name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        if let Some(hit) = pick_existing(
            candidates(Path::new(""), &name, nosuffix, mustsuffix),
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
                pick_existing(candidates(&dir, &name, nosuffix, mustsuffix), prefer_newer)
            {
                return Some(hit);
            }
            if let Ok(Some(path)) = locate_cache_source(i, &dir, &name, &source_suffixes) {
                return Some(Found::File(path));
            }
        }
        // Our bundled tree is flat under lisp/, with colliding
        // basenames renamed to SUBDIR-BASE.el; also try that spelling,
        // then the bare basename (unique basenames like "arabic").
        let flat = name.replace('/', "-");
        let base = name.rsplit('/').next().unwrap_or(&name);
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
        if let Some(hit) =
            pick_existing(candidates(&dir, &name, nosuffix, mustsuffix), prefer_newer)
        {
            return Some(hit);
        }
        if let Ok(Some(path)) = locate_cache_source(i, &dir, &name, &source_suffixes) {
            return Some(Found::File(path));
        }
    }
    None
}

/// `load-path' dirs followed by the bundled lisp/ dirs.
pub(super) fn search_dirs(i: &mut Interp) -> Vec<PathBuf> {
    let lp_id = i.q("load-path");
    let lp = i.symbol_value(lp_id);
    let mut v: Vec<PathBuf> = lp
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| match d {
            Value::Str(_) => filename_text(i, d)?.to_os_string().ok().map(PathBuf::from),
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
pub(crate) fn builtin_dirs() -> Vec<PathBuf> {
    let mut v = std::env::current_dir()
        .ok()
        .map(|cwd| vec![cwd.join("lisp")])
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
                let _ = crate::lisp::embedded_cache::materialize("simple", source);
            }
        }
        if let Some(cache) = crate::lisp::embedded_cache::directory() {
            let canonical = std::fs::canonicalize(&cache).ok();
            // Cache publication uses its logical temporary path. Keep
            // that spelling in load-path so repeated lookups are stable.
            for directory in &mut defaults {
                if let Value::Str(path) = directory {
                    if canonical.is_some()
                        && path
                            .borrow()
                            .to_os_string()
                            .ok()
                            .and_then(|path| std::fs::canonicalize(path).ok())
                            == canonical
                    {
                        *directory = Value::string(crate::lisp::text::EmacsText::from_os_str(
                            cache.as_os_str(),
                        ));
                    }
                }
            }
            let cache = Value::string(crate::lisp::text::EmacsText::from_os_str(cache.as_os_str()));
            if !defaults
                .iter()
                .any(|path| crate::lisp::builtins::equal_values(i, path, &cache).unwrap_or(false))
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
                    expanded.push(Value::string(crate::lisp::text::EmacsText::from_os_str(
                        path.as_os_str(),
                    )));
                }
            }
            expanded
        }
    };
    let paths = Value::list(paths);
    if !crate::lisp::builtins::equal_values(i, &i.symbol_value(load_path), &paths).unwrap_or(false)
    {
        let _ = i.set_symbol(load_path, paths);
    }
}

/// Runtime source locations whose contents belong to the executable.
fn installed_library_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exedir) = exe.parent() {
            v.push(exedir.join("../lisp"));
            v.push(exedir.join("../share/remacs/lisp"));
        }
    }
    if let Some(directory) = crate::lisp::embedded_cache::directory() {
        v.push(directory);
    }
    v
}

/// Distinguish shipped sources from user overrides, including a user's
/// cwd/lisp directory. All roots are compared after canonicalization.
pub(super) fn path_is_bundled_source<T: crate::lisp::text::TextSource + ?Sized>(path: &T) -> bool {
    path.with_text(|path| {
        let Some(p) = path
            .to_os_string()
            .ok()
            .and_then(|path| std::fs::canonicalize(path).ok())
        else {
            return false;
        };
        let mut roots = installed_library_dirs();
        // Classify an explicitly referenced development source tree without
        // making installed executables depend on that tree for searching.
        roots.push(Path::new(env!("REMACS_SOURCE_ROOT")).join("lisp"));
        roots.iter().any(|d| {
            std::fs::canonicalize(d)
                .map(|cd| p.starts_with(&cd))
                .unwrap_or(false)
        })
    })
}

/// Resolved-load identity for comparing against `loads_in_progress':
/// `builtin:' names and on-disk bundled sources with an embedded key
/// all normalize to `builtin:<key>' — the
/// three spellings (virtual path, disk `.el', disk `.elc' twin
/// fallback) one file can appear under while loading.  Everything
/// else is the literal resolved path GNU's `Fequal' would compare.
pub(crate) fn load_key<P: crate::lisp::text::TextSource + ?Sized>(
    path: &P,
) -> crate::lisp::text::EmacsText {
    let path = path.with_text(|text| text.to_owned());
    let name = path.strip_prefix("builtin:").unwrap_or(&path);
    if path.starts_with("builtin:") || path_is_bundled_source(&path) {
        if let Some((key, _)) = embedded_entry(name) {
            let key = if key == "emacs-lisp-subr-x" {
                "subr-x"
            } else {
                key
            };
            return format!("builtin:{key}").into();
        }
    }
    path.to_string()
}

use remacs_libraries::EMBEDDED_LISP;

/// Resolve a source key without consuming another filename suffix.
pub(super) fn embedded_key(stem: &str) -> Option<(&'static str, &'static str)> {
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

pub(super) fn embedded_entry_unicode(name: &str) -> Option<(&'static str, &'static str)> {
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    embedded_key(stem)
}

/// Embedded source for library NAME (including a returned flat filename).
/// Locate the default bundled source after the caller's search path.
/// Explicit PATH searches never enter here. Only real .el candidates
/// participate, so NOSUFFIX and representation suffixes remain meaningful.
pub(crate) fn locate_bundled_source_unicode(
    i: &mut Interp,
    name: &str,
    suffixes: &[String],
) -> Result<Option<crate::lisp::text::EmacsText>, crate::lisp::Flow> {
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
pub(crate) fn locate_cache_source_unicode(
    i: &mut Interp,
    directory: &Path,
    name: &str,
    suffixes: &[String],
) -> Result<Option<crate::lisp::text::EmacsText>, crate::lisp::Flow> {
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
    let Some(cache) = crate::lisp::embedded_cache::directory() else {
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

pub(super) fn bundled_source_file(
    i: &mut Interp,
    name: &str,
    key: &str,
    source: &str,
) -> Result<crate::lisp::text::EmacsText, crate::lisp::Flow> {
    let filename = format!("{}.el", key.replace('/', "-"));
    for directory in builtin_dirs() {
        let path = Path::new(&directory).join(&filename);
        if path.is_file() {
            return Ok(crate::lisp::text::EmacsText::from_os_str(path.as_os_str()));
        }
    }
    materialized_source_file(i, name, key, source)
}

/// Definition provenance must use shipped source, not a cwd/lisp override.
pub(crate) fn dump_source_file(
    i: &mut Interp,
    name: &str,
) -> Result<crate::lisp::text::EmacsText, crate::lisp::Flow> {
    let (key, source) = embedded_key(name)
        .ok_or_else(|| i.error(format!("Missing dumped library source: {name}")))?;
    let filename = format!("{}.el", key.replace('/', "-"));
    for directory in builtin_dirs() {
        let path = Path::new(&directory).join(&filename);
        if path.is_file()
            && path_is_bundled_source(&crate::lisp::text::EmacsText::from_os_str(path.as_os_str()))
        {
            return Ok(crate::lisp::text::EmacsText::from_os_str(path.as_os_str()));
        }
    }
    materialized_source_file(i, name, key, source)
}

pub(crate) fn bundled_history_path(file: &str) -> bool {
    file.starts_with("builtin:") || path_is_bundled_source(file)
}

/// Only unchanged shipped sources can be republished by a portable image.
pub(crate) fn dump_source_identity<T: crate::lisp::text::TextSource + ?Sized>(
    file: &T,
) -> Option<&'static str> {
    file.with_text(|file| {
        let path = std::path::PathBuf::from(file.to_os_string().ok()?);
        let prelude = concat!(
            env!("REMACS_SOURCE_ROOT"),
            "/crates/runtime/src/lisp/prelude.el"
        );
        let (key, source) = if file == prelude {
            ("remacs-prelude", crate::lisp::prelude::PRELUDE)
        } else {
            if !path_is_bundled_source(file) {
                return None;
            }
            // Bundled basenames are Unicode registry keys; the directory
            // itself can contain arbitrary native path bytes.
            embedded_entry(path.file_name()?.to_str()?)?
        };
        (std::fs::read(path).ok()?.as_slice() == source.as_bytes()).then_some(key)
    })
}

pub(super) fn materialized_source_file(
    i: &mut Interp,
    name: &str,
    key: &str,
    source: &str,
) -> Result<crate::lisp::text::EmacsText, crate::lisp::Flow> {
    let path = crate::lisp::embedded_cache::materialize(key, source).map_err(|error| {
        i.signal_data(
            crate::lisp::sym::FILE_ERROR,
            vec![
                Value::string("Writing embedded library source"),
                Value::string(error.to_string()),
                Value::string(name),
            ],
        )
    })?;
    Ok(crate::lisp::text::EmacsText::from_os_str(path.as_os_str()))
}

/// Normalize bundled source hits to the source that normal load uses.
pub(crate) fn located_source<N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &N,
    path: crate::lisp::text::EmacsText,
) -> Result<crate::lisp::text::EmacsText, crate::lisp::Flow> {
    let name = name.with_text(|text| text.to_owned());
    let stem = name.strip_suffix(".el").unwrap_or(&name);
    if !i.boot_loading && stem == "subr-x" && path_is_bundled_source(&path) {
        let source = PathBuf::from(
            path.to_os_string()
                .map_err(|_| i.error("Invalid source filename"))?,
        )
        .with_file_name("emacs-lisp-subr-x.el");
        if source.is_file() {
            return Ok(crate::lisp::text::EmacsText::from_os_str(
                source.as_os_str(),
            ));
        }
        let (key, source) = embedded_key("emacs-lisp-subr-x").unwrap();
        return bundled_source_file(i, &unicode_name(&name).unwrap_or_default(), key, source);
    }
    Ok(path)
}

pub(super) fn embedded_entry<T: crate::lisp::text::TextSource + ?Sized>(
    name: &T,
) -> Option<(&'static str, &'static str)> {
    embedded_entry_unicode(&unicode_name(name)?)
}
pub(crate) fn embedded<T: crate::lisp::text::TextSource + ?Sized>(
    name: &T,
) -> Option<&'static str> {
    embedded_entry(name).map(|(_, source)| source)
}
pub(crate) fn locate_bundled_source<T: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &T,
    suffixes: &[String],
) -> Result<Option<crate::lisp::text::EmacsText>, crate::lisp::Flow> {
    let Some(name) = unicode_name(name) else {
        return Ok(None);
    };
    locate_bundled_source_unicode(i, &name, suffixes)
}
pub(crate) fn locate_cache_source<T: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    directory: &Path,
    name: &T,
    suffixes: &[String],
) -> Result<Option<crate::lisp::text::EmacsText>, crate::lisp::Flow> {
    let Some(name) = unicode_name(name) else {
        return Ok(None);
    };
    locate_cache_source_unicode(i, directory, &name, suffixes)
}
