//! Source/direct-load bindings and explicit input-buffer/stream lifecycle.

use super::eager::eval_str_for_load;
use super::locate::{embedded_entry, path_is_bundled_source};
use super::{LoadStream, build_history, lexical_binding_value, lexical_cookie};
use crate::lisp::{Interp, error::EvalResult, value::Value};
use std::{cell::RefCell, rc::Rc};

/// Evaluate SRC as if loaded from file FILE (binds load-file-name,
/// load-in-progress, lexical-binding cookie; runs after-load hooks).
pub(super) fn eval_src<
    F: crate::lisp::text::TextSource + ?Sized,
    S: crate::lisp::text::TextSource + ?Sized,
>(
    i: &mut Interp,
    file: &F,
    src: &S,
    force_lex: bool,
) -> EvalResult {
    eval_src_opts(i, file, src, force_lex, false)
}

pub(super) fn eval_src_opts<
    F: crate::lisp::text::TextSource + ?Sized,
    S: crate::lisp::text::TextSource + ?Sized,
>(
    i: &mut Interp,
    file: &F,
    src: &S,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    let compiled = src.with_text(|src| src.starts_with(";ELC\u{1f}\u{0}\u{0}\u{0}"));
    eval_src_kind(i, file, src, force_lex, dumped_like, compiled)
}

pub(super) fn eval_src_kind<
    F: crate::lisp::text::TextSource + ?Sized,
    S: crate::lisp::text::TextSource + ?Sized,
>(
    i: &mut Interp,
    file: &F,
    src: &S,
    force_lex: bool,
    dumped_like: bool,
    compiled: bool,
) -> EvalResult {
    eval_src_kind_with_codes(i, file, src, force_lex, dumped_like, compiled, None)
}

pub(super) fn eval_src_kind_with_codes<
    F: crate::lisp::text::TextSource + ?Sized,
    S: crate::lisp::text::TextSource + ?Sized,
>(
    i: &mut Interp,
    file: &F,
    src: &S,
    force_lex: bool,
    dumped_like: bool,
    compiled: bool,
    oversized_codes: Option<&crate::lisp::reader::OversizedCodes>,
) -> EvalResult {
    file.with_text(|file| {
        src.with_text(|src| {
            LoadRequest {
                file,
                src,
                force_lex,
                dumped_like,
                compiled,
                oversized_codes,
            }
            .eval(i)
        })
    })
}

/// Request metadata is immutable across nested callbacks and unwinding.
/// Keeping the flags named prevents source/compiled/script booleans from
/// being transposed when moving between load phases.
#[derive(Clone, Copy)]
struct LoadRequest<'a> {
    file: &'a crate::lisp::text::EmacsStr,
    src: &'a crate::lisp::text::EmacsStr,
    force_lex: bool,
    dumped_like: bool,
    compiled: bool,
    oversized_codes: Option<&'a crate::lisp::reader::OversizedCodes>,
}

impl LoadRequest<'_> {
    fn eval(&self, i: &mut Interp) -> EvalResult {
        // Fload resets lexical binding before entering the source converter.
        let depth = i.specbind_depth();
        let lexical = i.q("lexical-binding");
        let result = (|| {
            i.specbind(lexical, Value::Nil)?;
            self.eval_bound(i)
        })();
        let result = i.offer_flow(result);
        i.unbind_to(depth).and(result)
    }

    fn eval_bound(&self, i: &mut Interp) -> EvalResult {
        let Self {
            file,
            force_lex,
            compiled,
            ..
        } = *self;
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
                self.eval_in_context(i, source)
            })
        } else {
            self.eval_in_context(i, source)
        }
    }

    fn eval_in_context(&self, i: &mut Interp, source: bool) -> EvalResult {
        let Self { file, src, .. } = *self;
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
        let caller = LoadCaller::capture(i, load_buffer.is_some());
        let result = (|| {
            if load_buffer.is_none() {
                if i.load_stream_symbol.is_none() {
                    let symbol = i.make_symbol("get-file-char");
                    i.load_stream_symbol = Some(symbol);
                }
                i.load_stream = Some(Rc::new(RefCell::new(LoadStream {
                    chars: Rc::new(src.chars().collect()),
                    oversized_codes: self.oversized_codes.cloned(),
                    position: 0,
                })));
            }
            self.eval_inner(i, load_buffer.as_ref())
        })();
        let result = i.offer_flow(result);
        let unwind = caller.restore(i);
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
    fn eval_inner(
        &self,
        i: &mut Interp,
        load_buffer: Option<&crate::lisp::value::BufferRef>,
    ) -> EvalResult {
        let Self {
            file,
            src,
            force_lex,
            dumped_like,
            compiled,
            ..
        } = *self;
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
            let result = if function.is_nil() {
                Ok(Value::Nil)
            } else {
                i.apply(&function, vec![])
            };
            let result = i.offer_flow(result);
            if i.buffer_live(caller) {
                i.set_current_buffer(caller);
            }
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
        let optimizer_source = |path: &crate::lisp::text::EmacsStr| {
            (path.starts_with("builtin:") || path_is_bundled_source(path))
                && embedded_entry(path.strip_prefix("builtin:").unwrap_or(path))
                    .is_some_and(|(key, _)| key == "byte-opt")
        };
        let bo = i.q("byte-optimize");
        if i.bound_p(bo)
            && (i.boot_loading
                || optimizer_source(file)
                || i.loads_in_progress
                    .iter()
                    .any(|path| optimizer_source(path)))
        {
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
        if i.loads_in_progress.iter().filter(|f| &***f == file).count() > 3 {
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
}

/// GNU's `do-after-load-evaluation' (subr.el): run the matching
/// `after-load-alist' string keys plus `after-load-functions' for the
/// file just loaded.  Early boot loads skip it: nothing is registered
/// yet and the defun doesn't exist yet.
fn run_after_load(i: &mut Interp, file: &crate::lisp::text::EmacsStr) {
    if let Some(dale) = i.q_soft("do-after-load-evaluation") {
        if i.fbound_p(dale) {
            let _ = i.apply(&Value::Sym(dale), vec![Value::string(file)]);
        }
    }
}

/// Source errors are offered before restoring this state. Unbinding happens
/// after lexical/excursion restoration and before restoring the direct stream.
struct LoadCaller {
    excursion: Option<crate::lisp::eval::ExcursionState>,
    stream: Option<Rc<RefCell<LoadStream>>>,
    lexenv: crate::lisp::eval::LexEnv,
    depth: usize,
}

impl LoadCaller {
    fn capture(i: &mut Interp, buffered: bool) -> Self {
        Self {
            excursion: buffered.then(|| i.save_excursion_state(false)),
            stream: i.load_stream.clone(),
            lexenv: i.lexenv.clone(),
            depth: i.specbind_depth(),
        }
    }

    fn restore(self, i: &mut Interp) -> Result<(), crate::lisp::Flow> {
        i.lexenv = self.lexenv;
        if let Some(saved) = self.excursion {
            i.restore_excursion_state(saved);
        }
        let result = i.unbind_to(self.depth);
        i.load_stream = self.stream;
        result
    }
}
