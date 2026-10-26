//! File loading: locate libraries on load-path, eval .el files.

use crate::lisp::{Interp, value::Value};
use remacs_libraries::embedded_elc;
use std::{path::Path, rc::Rc};

mod command;
mod compat;
mod eager;
mod file;
mod history;
mod lexical;
mod locate;
mod policy;
mod source;

pub use command::CommandLineLoadPath;
use compat::{eval_gnu_subr_x, keep_live_definitions, live_definitions};
use file::eval_file_lex_dumped;
pub use file::{eval_file, eval_file_script};
pub(crate) use history::{build_history, record_prelude_load_history};
pub(crate) use lexical::{lexical_binding_value, lexical_cookie};
pub(crate) use locate::{
    Found, builtin_dirs, bundled_history_path, dump_source_file, dump_source_identity, embedded,
    filename_text, finish_load_path, load_key, locate_bundled_source, locate_cache_source,
    locate_full, located_source, unicode_name,
};
use locate::{embedded_entry, path_is_bundled_source};
use policy::{POSTBOOT_AOT_LIBRARIES, STANDALONE_POSTBOOT_AOT_LIBRARIES};
use source::{eval_src, eval_src_opts};

/// Consuming input for direct/compiled loads, exposed as GNU's private
/// `get-file-char' stream symbol. Source loads use their real buffer.
pub(crate) struct LoadStream {
    pub chars: Rc<Vec<super::text::EmacsChar>>,
    pub oversized_codes: Option<super::reader::OversizedCodes>,
    pub position: usize,
}

/// Outcome of a `load' attempt.
pub(crate) enum LoadOutcome {
    /// The file was found and evaluated to completion — payload is the
    /// resolved file name (what `load-file-name' was bound to).
    Loaded(crate::lisp::text::EmacsText),
    /// No candidate matched (GNU's openp returning -2).
    Missing,
    /// A directory matched: GNU's openp opens it and `Fload' then
    /// fstat's it into a file-error "Is a directory".  Kept out of the
    /// `Err' channel so `load' NOERROR can suppress just this class.
    Dir(crate::lisp::text::EmacsText),
}

/// Load library NAME; returns Ok(Loaded(resolved-path)) on success.
pub(crate) fn load_library<N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &N,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    load_library_opts_msg(i, name, false, false, true, false)
}

/// `load_library' honoring `load''s NOSUFFIX and MUST-SUFFIX.  Like
/// GNU, `Loading %s...' is echoed (through `message') before the
/// file's forms run; ` (source)' marks files read from Lisp source
/// rather than byte-compiled output.
pub(crate) fn load_library_opts<N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &N,
    nosuffix: bool,
    mustsuffix: bool,
    can_elc: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    load_library_opts_msg(i, name, nosuffix, mustsuffix, can_elc, false)
}

/// Full form: NOMESSAGE suppresses the `Loading %s...' echo (GNU
/// Fload NOMESSAGE — autoload loads pass t).
pub(crate) fn load_library_opts_msg<N: crate::lisp::text::TextSource + ?Sized>(
    i: &mut Interp,
    name: &N,
    nosuffix: bool,
    mustsuffix: bool,
    can_elc: bool,
    nomessage: bool,
) -> Result<LoadOutcome, crate::lisp::Flow> {
    let name = name.with_text(|text| text.to_owned());
    let t0 = std::time::Instant::now();
    let announce = |i: &mut Interp, file: &crate::lisp::text::EmacsStr| {
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
        .unwrap_or(&name);
    // GNU ships shortdoc's definitions as compiled data.  Eagerly expanding
    // that data would autoload shortdoc, which requires the same file again.
    // Other compiled twins remain restricted to the boot window.
    let registry_stem_text = unicode_name(&stem);
    let registry_stem = registry_stem_text.as_deref().unwrap_or("");
    let org_aot_parent = i.reading_elc
        && i.loads_in_progress.iter().any(|path| {
            let parent = path.strip_prefix("builtin:").unwrap_or(path);
            let parent = parent.rsplit('/').next().unwrap_or(parent);
            let parent = parent
                .strip_suffix(".elc")
                .or_else(|| parent.strip_suffix(".el"))
                .unwrap_or(parent);
            matches!(
                unicode_name(&parent).as_deref(),
                Some("org" | "org-element")
            )
        });
    let org_modules_loading = i.lisp_stack.iter().any(|(function, _)| {
        matches!(function, Value::Sym(id) if i.symbol_name(*id) == "org-load-modules-maybe")
    });
    let postboot_org_aot = POSTBOOT_AOT_LIBRARIES.contains(&registry_stem)
        && !name.ends_with(".el")
        && (matches!(registry_stem, "org" | "org-element")
            || org_aot_parent
            || org_modules_loading);
    let standalone_postboot_aot =
        STANDALONE_POSTBOOT_AOT_LIBRARIES.contains(&registry_stem) && !name.ends_with(".el");
    let compiled_twin_allowed = i.boot_loading
        || (stem == "shortdoc-doc" && !name.ends_with(".el"))
        || postboot_org_aot
        || standalone_postboot_aot;
    let found = locate_full(i, &name, nosuffix, mustsuffix);
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
            let real = crate::editor::native_path(i, &path)?.with_file_name("emacs-lisp-subr-x.el");
            let file = if real.is_file() {
                crate::lisp::text::EmacsText::from_os_str(real.as_os_str())
            } else {
                crate::lisp::text::EmacsText::from("builtin:subr-x")
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
                && embedded_elc(registry_stem).is_some()
                && !i
                    .symbol_value(i.q_soft("load-prefer-newer").unwrap_or(0))
                    .truthy();
            if use_elc {
                let src = embedded_elc(registry_stem).unwrap();
                // Post-boot Org AOT twins replace the bundled source only;
                // keep its real path as the public load/symbol identity.
                let virtual_path = if POSTBOOT_AOT_LIBRARIES.contains(&registry_stem) {
                    path.clone()
                } else {
                    crate::lisp::text::EmacsText::from(format!("builtin:{}", stem))
                };
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
                                    .collect::<crate::lisp::text::EmacsText>()
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
            announce(i, &name);
            eval_gnu_subr_x(i, "builtin:subr-x", bundled)
        }
        None => match embedded_elc(registry_stem).filter(|_| {
            can_elc && compiled_twin_allowed && std::env::var_os("REMACS_NO_ELC").is_none()
        }) {
            Some(src) => {
                announce(i, &name);
                let virtual_path = crate::lisp::text::EmacsText::from(format!("builtin:{}", stem));
                // `.elc' forms eval dynamically in GNU — no force_lex.
                match eval_src_opts(i, &virtual_path, src, false, true) {
                    Ok(_) => Ok(LoadOutcome::Loaded(virtual_path)),
                    Err(f) => {
                        // Same fallback as the Found::File arm: an
                        // AOT blob that hits a GNU-only call or
                        // boot-order dependency retries as source.
                        match embedded(&name) {
                            Some(src) => {
                                announce(i, &name);
                                eval_src(i, &virtual_path, src, false)
                                    .map(|_| LoadOutcome::Loaded(virtual_path))
                            }
                            None => Err(f),
                        }
                    }
                }
            }
            None => match embedded(&name) {
                Some(src) => {
                    announce(i, &name);
                    let virtual_path =
                        crate::lisp::text::EmacsText::from(format!("builtin:{}", name));
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
