//! Shared editor command dispatch and nested minibuffer input loops.
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use super::KeyIo;
#[cfg(test)]
use super::render_grid;
use crate::{
    editor::{CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER},
    lisp::{Interp, Value},
};
use std::io;
use std::time::{Duration, Instant};
mod isearch;
use isearch::*;

/// A batch startup can reach the command loop when a replacement for
/// `kill-emacs' returns.  The threaded GNU event loop waits for queued
/// events and timers here; it neither enters terminal mode nor consumes
/// redirected stdin as editor commands.
pub fn run_batch_editor(i: &mut Interp) -> Result<(), crate::lisp::Flow> {
    use crate::lisp::{Flow, Value};
    let top_level = Value::Sym(i.q("top-level"));
    i.catch_tags.push(top_level.clone());
    let result = (|| {
        let mut keys = Vec::new();
        let mut arg_mode = false;
        crate::lisp::builtins::evalfn::safe_call_hook(i, "post-command-hook")?;
        loop {
            crate::lisp::eval::threads::yield_now(i)?;
            crate::lisp::builtins::evalfn::thread_wait_events(i)?;
            if let Some(code) = i.exit_status {
                return Err(Flow::Exit(code));
            }
            let Some(event) = crate::editor::pop_unread_command_event(i) else {
                crate::lisp::eval::threads::pause(i, Duration::from_millis(20))?;
                continue;
            };
            let mut flow = None;
            let codes = crate::editor::key_seq(i, &Value::vector(vec![event]))?;
            for key in codes {
                dispatch_key_core_inner(i, key, &mut keys, &mut arg_mode, None, Some(&mut flow))
                    .map_err(|error| i.error(&error.to_string()))?;
                if flow.is_some() {
                    break;
                }
            }
            if let Some(flow) = flow {
                let condition = match flow {
                    Flow::Signal(signal) => signal.condition,
                    Flow::Quit => Value::list(vec![Value::Sym(i.q("quit"))]),
                    Flow::Throw(tag, value)
                        if crate::lisp::builtins::eq_values(&tag, &top_level) =>
                    {
                        return Err(Flow::Throw(tag, value));
                    }
                    Flow::Throw(tag, value) => {
                        Value::list(vec![Value::Sym(i.q("no-catch")), tag, value])
                    }
                    Flow::Exit(code) => return Err(Flow::Exit(code)),
                };
                i.finish_quit_unwind();
                batch_command_error(i, condition)?;
                crate::lisp::builtins::evalfn::safe_call_hook(i, "post-command-hook")?;
            }
        }
    })();
    i.catch_tags.pop();
    match result {
        Err(Flow::Throw(tag, _)) if crate::lisp::builtins::eq_values(&tag, &top_level) => {
            // keyboard.c exits batch mode directly after the command-loop
            // catch returns, even when the public function cell was replaced.
            i.finish_quit_unwind();
            let result = crate::lisp::builtins::evalfn::f_kill_emacs(i, vec![Value::t()].into());
            i.offer_flow(result).map(|_| ())
        }
        other => other,
    }
}

/// keyboard.c's cmd_error adds these scopes around cmd_error_internal.
/// Process callback reporters intentionally use only the latter protocol.
pub fn batch_command_error(i: &mut Interp, condition: Value) -> crate::lisp::EvalResult {
    let mark = i.specbind_depth();
    let result = (|| {
        for name in ["standard-output", "standard-input"] {
            let id = i.intern(name);
            i.specbind(id, Value::t())?;
        }
        for name in ["prefix-arg", "last-prefix-arg"] {
            let id = i.intern(name);
            i.obarray.symbol_mut(id).value = Value::Nil;
        }
        let print_level = i.q("print-level");
        let print_length = i.q("print-length");
        let old_level = i.symbol_value(print_level);
        let old_length = i.symbol_value(print_length);
        i.obarray.symbol_mut(print_level).value = Value::Int(10);
        i.obarray.symbol_mut(print_length).value = Value::Int(10);
        crate::lisp::builtins::evalfn::command_error(i, condition, "")?;
        i.obarray.symbol_mut(print_level).value = old_level;
        i.obarray.symbol_mut(print_length).value = old_length;
        i.clear_quit_flag();
        let inhibit = i.q("inhibit-quit");
        i.obarray.symbol_mut(inhibit).value = Value::Nil;
        Ok(Value::Nil)
    })();
    let result = i.offer_flow(result);
    i.unbind_to(mark).and(result)
}

/// The editor command loop, parameterized over a front-end. Also
/// used by the gpui front-end, which supplies a channel-backed KeyIo
/// and runs this on a dedicated logic thread (Interp is !Send).
pub fn run_editor_with<T: KeyIo + 'static>(frontend: T, i: &mut Interp) -> io::Result<()> {
    let term = std::rc::Rc::new(std::cell::RefCell::new(frontend));
    // Install the interactive input hook so read-from-minibuffer,
    // M-x, y-or-n-p, read-char, and `interactive' spec codes can read
    // input from inside the evaluator.
    {
        let t = term.clone();
        i.minibuf_reader = Some(std::rc::Rc::new(move |interp, prompt, single| {
            minibuf_loop::<T>(&t, interp, prompt, single)
        }));
    }
    // Sync frame geometry.
    {
        let (w, h) = term.borrow().size();
        if let Some(f) = &i.selected_frame {
            f.borrow_mut().width = w;
            f.borrow_mut().height = h;
        }
    }
    let mut keys: Vec<i128> = Vec::new();
    // Set while a universal/digit-argument sequence is being entered:
    // digits, `-', and further C-u keep building `prefix-arg' (Emacs's
    // `universal-argument' continuation behavior).
    let mut arg_mode = false;
    loop {
        if let Err(flow) = crate::lisp::eval::threads::yield_now(i)
            .and_then(|_| crate::lisp::builtins::evalfn::thread_wait_events(i))
        {
            report_flow(i, flow);
        }
        if i.quit_editor {
            break;
        }
        if let Err(flow) = crate::editor::changes::run(i) {
            report_flow(i, flow);
        }
        term.borrow_mut().render(i)?;
        // Replayed macro events take priority over real input;
        // `executing-kbd-macro` clears once the queue drains.
        let key = match i.macro_replay.pop_front() {
            Some(k) => {
                i.macro_replaying = true;
                k
            }
            None => {
                let ek = i.q("executing-kbd-macro");
                if i.symbol_value(ek).truthy() {
                    let _ = i.set_symbol(ek, Value::Nil);
                }
                // Poll input. Sleep deadlines keep the UI responsive.
                let timeout = Duration::from_millis(50);
                match term.borrow_mut().poll_key(timeout)? {
                    Some(k) => k,
                    None => continue,
                }
            }
        };
        dispatch_key(&term, i, key, &mut keys, &mut arg_mode)?;
        i.macro_replaying = false;
        // A command error while replaying a macro aborts the rest of
        // the replay; the queue was already cleared.
        if let Some(f) = i.macro_pending_flow.take() {
            report_flow(i, f);
        }
    }
    Ok(())
}

/// Process one key event: extend the key sequence, look up the
/// binding, and execute the command (or self-insert / enter isearch).
/// Shared by every front-end via `run_editor_with`.
/// Dispatch one key event through the active keymaps — the shared
/// core for the front-end key loop and `minibuf_read`'s scripted
/// input path.  `isearch` intercepts the four incremental-search
/// commands (TUI passes a closure that re-reads keys; the canned
/// reader supplies `None` so isearch runs as an ordinary command).
pub fn dispatch_key_core(
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
    isearch: Option<&mut dyn FnMut(&mut Interp, bool, bool) -> io::Result<()>>,
) -> io::Result<()> {
    dispatch_key_core_inner(i, key, keys, arg_mode, isearch, None)
}

fn dispatch_key_core_inner(
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
    mut isearch: Option<&mut dyn FnMut(&mut Interp, bool, bool) -> io::Result<()>>,
    mut command_flow: Option<&mut Option<crate::lisp::Flow>>,
) -> io::Result<()> {
    {
        if *arg_mode {
            let digit = (48..58).contains(&key);
            let minus = key == 45;
            let cu = key == 21;
            if digit || minus || cu {
                let lce = i.q("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let name = if cu {
                    "universal-argument"
                } else {
                    "digit-argument"
                };
                let id = i.intern(name);
                if i.command_execute(&Value::Sym(id)).is_ok() {
                    post_command_undo_boundary(i);
                }
                return Ok(());
            }
            *arg_mode = false;
        }
        keys.push(key);
        // Record the event when defining a keyboard macro (replayed
        // keys are not re-recorded, matching GNU).
        if !i.macro_replaying {
            let dm = i.q("defining-kbd-macro");
            if i.symbol_value(dm).truthy() {
                i.kbd_macro_events.push(Value::Int(key));
            }
        }
        let seq = Value::vector(keys.iter().map(|k| Value::Int(*k)).collect());
        // Look up the key sequence.
        let binding = lookup_command(i, &seq);
        match binding {
            LookupResult::Command(raw_cmd) => {
                // GNU command_loop_1: a symbol binding is checked
                // against `[remap CMD]' in the active maps first —
                // `self-insert-command' under `y-or-n-p-map' becomes
                // `y-or-n-p-insert-y'.
                let cmd = match &raw_cmd {
                    Value::Sym(_) => crate::editor::command_remapping_pub(i, &raw_cmd)
                        .ok()
                        .filter(|v| v.truthy())
                        .unwrap_or_else(|| raw_cmd.clone()),
                    _ => raw_cmd.clone(),
                };
                keys.clear();
                let lce = i.q("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let this_cmd = i.q("this-command");
                let prev = i.symbol_value(this_cmd);
                let lc = i.q("last-command");
                let _ = i.set_symbol(lc, prev);
                let _ = i.set_symbol(this_cmd, cmd.clone());
                let pa = i.q("prefix-arg");
                let prefix = i.symbol_value(pa);
                let cpa = i.q("current-prefix-arg");
                let _ = i.set_symbol(cpa, prefix);
                // Consume prefix-arg BEFORE executing so commands that
                // set it (C-u, digit-argument) affect the NEXT command.
                let _ = i.set_symbol(pa, Value::Nil);
                // isearch commands enter a dedicated incremental loop
                // rather than running once through command_execute.
                let isearch_cmd = if let Value::Sym(id) = &cmd {
                    match i.symbol_name(*id).as_str() {
                        "isearch-forward" => Some((false, false)),
                        "isearch-backward" => Some((true, false)),
                        "isearch-forward-regexp" => Some((false, true)),
                        "isearch-backward-regexp" => Some((true, true)),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((back, re)) = isearch_cmd {
                    if let Some(hook) = isearch.as_mut() {
                        hook(i, back, re)?;
                        post_command_undo_boundary(i);
                        return Ok(());
                    }
                }
                // GNU command_loop_1: `pre-command-hook' runs before
                // the command, `post-command-hook' after it — even on
                // error (both via safe_run_hooks).
                let _ = crate::lisp::builtins::evalfn::safe_call_hook(i, "pre-command-hook");
                let ignore_command = i.q("ignore");
                let skip_command =
                    matches!(i.symbol_value(this_cmd), Value::Sym(id) if id == ignore_command);
                let mut boundary = false;
                let result = if skip_command {
                    Ok(Value::Nil)
                } else {
                    i.command_execute(&cmd)
                };
                match result {
                    Ok(_) => {
                        boundary = !skip_command;
                    }
                    Err(flow) if command_flow.is_some() => {
                        **command_flow.as_mut().unwrap() = Some(flow);
                    }
                    Err(flow @ crate::lisp::error::Flow::Quit) => command_quit(i, flow),
                    Err(flow @ crate::lisp::error::Flow::Signal(..)) if flow.is_quit() => {
                        command_quit(i, flow);
                    }
                    Err(e @ crate::lisp::error::Flow::Signal(..)) => {
                        if i.macro_replaying || i.macro_strict {
                            // GNU `command_loop_2': an error during a
                            // macro iteration terminates the macro and
                            // propagates out of `execute-kbd-macro'.
                            i.macro_replay.clear();
                            i.macro_pending_flow = Some(e);
                        } else {
                            let crate::lisp::error::Flow::Signal(signal) = e else {
                                unreachable!()
                            };
                            let sym = signal.symbol();
                            let data = signal.data();
                            let msg = signal_text(i, &sym, &data);
                            i.message(&msg);
                        }
                    }
                    Err(crate::lisp::error::Flow::Throw(tag, v)) => {
                        let is_exit = matches!(&tag, Value::Sym(s) if i.symbol_name(*s) == "exit");
                        if i.minibuf_catching_exit && is_exit {
                            // `exit' reached read_minibuf's catch —
                            // GNU recursive_edit_1's value protocol:
                            // nil → contents, t → quit, string →
                            // `error' signal, function → called after
                            // unwinding (minibuffer-quit-recursive-edit).
                            match &v {
                                Value::Nil => i.minibuf_exited = true,
                                Value::Str(s) => {
                                    let msg = Value::string(s.borrow().clone());
                                    let es = Value::Sym(i.q("error"));
                                    i.minibuf_pending_flow =
                                        Some(crate::lisp::error::Flow::signal(
                                            es,
                                            Value::list(vec![msg]),
                                        ));
                                }
                                Value::Sym(_) if v.truthy() => {
                                    i.minibuf_pending_flow = Some(crate::lisp::error::Flow::Quit);
                                }
                                _ => {
                                    // Functionp values are called after
                                    // the read unwinds.
                                    i.minibuf_exit_fn = Some(v.clone());
                                    i.minibuf_exited = true;
                                }
                            }
                        } else if i.minibuf_catching_exit {
                            // A throw for another tag unwinds past the
                            // `exit' catch — abort the read with it.
                            i.minibuf_pending_flow = Some(crate::lisp::error::Flow::Throw(tag, v));
                        } else if i.macro_replaying || i.macro_strict {
                            // Uncaught throw aborts the macro (GNU
                            // command_loop_2 → `no-catch' signal).
                            i.macro_replay.clear();
                            i.macro_pending_flow = Some(crate::lisp::error::Flow::Throw(tag, v));
                        } else {
                            let mut message = EmacsText::from("No catch for tag: ");
            message.append(&i.princ_to_string(&tag));
            i.message(&message);
                        }
                    }
                    Err(crate::lisp::error::Flow::Exit(_)) => {
                        // `kill-emacs' requested termination; the flag is
                        // already set, so the loop exits at the top.
                        i.quit_editor = true;
                    }
                }
                if boundary {
                    post_command_undo_boundary(i);
                }
                if !command_flow.as_ref().is_some_and(|flow| flow.is_some()) {
                    let _ = crate::lisp::builtins::evalfn::safe_call_hook(i, "post-command-hook");
                }
                // Continue arg entry after C-u / M-digit / M--.
                if let Value::Sym(id) = &cmd {
                    let n = i.symbol_name(*id);
                    *arg_mode = matches!(
                        n.as_str(),
                        "universal-argument" | "digit-argument" | "negative-argument"
                    );
                }
                // Command boundary: `cancel-kbd-macro-events` truncates
                // back to this mark.
                i.kbd_macro_mark = i.kbd_macro_events.len();
            }
            LookupResult::Prefix => {
                // keep reading keys
            }
            LookupResult::None => {
                // Plain char → self-insert.  GNU treats these as
                // `self-insert-command'/`newline'/`delete-backward-char'
                // commands: this-command is set and the command hooks run.
                if keys.len() == 1 {
                    if let Some(&k) = keys.first() {
                        let name = if k < CHAR_CTL && k >= 32 && k != 127 {
                            Some("self-insert-command")
                        } else if k == b'\r' as i128 {
                            Some("newline")
                        } else if k == 127 {
                            Some("delete-backward-char")
                        } else {
                            None
                        };
                        if let Some(n) = name {
                            let this_cmd = i.q("this-command");
                            let prev = i.symbol_value(this_cmd);
                            let lc = i.q("last-command");
                            let _ = i.set_symbol(lc, prev);
                            let cmd_sym = Value::Sym(i.intern(n));
                            let _ = i.set_symbol(this_cmd, cmd_sym);
                            let _ = crate::lisp::builtins::evalfn::safe_call_hook(
                                i,
                                "pre-command-hook",
                            );
                            let ignore_command = i.q("ignore");
                            let skip_command = matches!(
                                i.symbol_value(this_cmd),
                                Value::Sym(id) if id == ignore_command
                            );
                            if !skip_command {
                                if k == 127 {
                                    if let Some(b) = i.current_buffer_ref() {
                                        let p = b.borrow().point();
                                        if p > 0 {
                                            let _ = crate::buffer::primitives::chg_delete(i, p - 1, p);
                                        }
                                    }
                                } else if i.current_buffer_ref().is_some() {
                                    let s = if k == b'\r' as i128 {
                                        "\n".to_string()
                                    } else {
                                        char::from_u32(k as u32).unwrap_or('?').to_string()
                                    };
                                    let _ = crate::buffer::primitives::chg_insert_pt(i, &s, false);
                                }
                                post_command_undo_boundary(i);
                            }
                            keys.clear();
                            let _ = crate::lisp::builtins::evalfn::safe_call_hook(
                                i,
                                "post-command-hook",
                            );
                            return Ok(());
                        }
                    }
                }
                i.message(&format!(
                    "{} is undefined",
                    keys.iter()
                        .map(|k| crate::editor::describe_key_pub(*k))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
                keys.clear();
            }
        }
    }
    Ok(())
}

/// Drain the replay queue synchronously, dispatching each queued event
/// through `dispatch_key_core` — the batch/nested path of
/// `execute-kbd-macro` (GNU `command_loop_2' inside `Fexecute_kbd_macro').
/// `stop` is the queue length at which to stop: a nested
/// `execute-kbd-macro' call passes the length it found on entry so the
/// outer call's still-queued events are left for the outer drain.
/// Stops early when a command error set `macro_pending_flow'.
pub fn replay_macro_events(i: &mut Interp, stop: usize) {
    let mut keys: Vec<i128> = Vec::new();
    let mut arg_mode = false;
    // Events consumed in this drain — GNU's executing_kbd_macro_index,
    // the number already read from the current iteration.
    let mut idx: i128 = 0;
    while i.macro_replay.len() > stop {
        let Some(k) = i.macro_replay.pop_front() else {
            break;
        };
        let eki = i.q("executing-kbd-macro-index");
        let _ = i.set_symbol(eki, Value::Int(idx));
        idx += 1;
        i.macro_replaying = true;
        if dispatch_key_core(i, k, &mut keys, &mut arg_mode, None).is_err() {
            i.macro_replaying = false;
            return;
        }
        i.macro_replaying = false;
        if i.macro_pending_flow.is_some() || i.quit_editor {
            return;
        }
    }
}

fn command_quit(i: &mut Interp, flow: crate::lisp::error::Flow) {
    if i.minibuf_catching_exit {
        i.minibuf_pending_flow = Some(flow);
    } else if i.macro_replaying || i.macro_strict {
        i.macro_replay.clear();
        i.macro_pending_flow = Some(flow);
    } else {
        i.message("Quit");
    }
}

/// Echo a deferred command flow the same way the command dispatcher does.
fn report_flow(i: &mut Interp, f: crate::lisp::error::Flow) {
    match f {
        crate::lisp::error::Flow::Quit => i.message("Quit"),
        crate::lisp::error::Flow::Signal(signal) => {
            if signal.is_symbol(crate::lisp::obarray::sym::QUIT) {
                i.message("Quit");
                return;
            }
            let sym = signal.symbol();
            let data = signal.data();
            let msg = signal_text(i, &sym, &data);
            i.message(&msg);
        }
        crate::lisp::error::Flow::Throw(tag, _) => {
            let mut message = EmacsText::from("No catch for tag: ");
            message.append(&i.princ_to_string(&tag));
            i.message(&message);
        }
        crate::lisp::error::Flow::Exit(_) => i.quit_editor = true,
    }
}

fn dispatch_key<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
) -> io::Result<()> {
    let mut hook = |i: &mut Interp, back: bool, re: bool| isearch_loop(term, i, back, re);
    dispatch_key_core(i, key, keys, arg_mode, Some(&mut hook))
}

/// The front-end-independent half of `minibuf_loop' for a canned/
/// scripted input source: dispatch one `MinibufInput::Key' through
/// the minibuffer's real keymaps.  `minibuf_read' owns `keys'/
/// `arg_mode' across calls, then does minibuf_loop's post-dispatch
/// checks (minibuf_exit_fn / minibuf_exited / minibuf_pending_flow).
pub(crate) fn minibuf_dispatch_key(
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
) -> Result<(), crate::lisp::error::Flow> {
    dispatch_key_core(i, key, keys, arg_mode, None)
        .map_err(|e| i.error(format!("minibuffer input: {e}")))
}

/// GNU `command_loop_1' calls `undo-boundary' after each successfully
/// executed command; this pushes a nil boundary into the current
/// buffer's `buffer-undo-list' unless one is already there.
fn post_command_undo_boundary(i: &mut Interp) {
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().undo_boundary();
    }
}

enum LookupResult {
    Command(Value),
    Prefix,
    None,
}

fn lookup_command(i: &mut Interp, seq: &Value) -> LookupResult {
    let keys = match seq {
        Value::Vec(v) => v.borrow().clone(),
        _ => return LookupResult::None,
    };
    if keys.is_empty() {
        return LookupResult::None;
    }
    // Convert key vector to string for keymap lookup if all plain chars.
    // Our define-key accepts vectors directly.
    let binding = match crate::editor::lookup_command_in_maps(i, &keys) {
        Ok(b) => b,
        // A `:filter' (or other keyelt evaluation) error counts as no
        // binding — GNU treats an unfound command the same.
        Err(_) => None,
    };
    match binding {
        Some(v) => {
            // is it a keymap (prefix)?
            if let Value::Cons(c) = &v {
                if i.sym_is(&c.borrow().car, i.q_soft("keymap").unwrap_or(u32::MAX)) {
                    return LookupResult::Prefix;
                }
            }
            if v.truthy() {
                LookupResult::Command(v)
            } else {
                LookupResult::None
            }
        }
        None => {
            // If no binding and the last key extends a prefix → undefined.
            LookupResult::None
        }
    }
}

/// Blocking `sleep-for` replacement at the loop level: repaint during the wait.
pub fn nonblocking_sleep(i: &mut Interp, dur: Duration) -> io::Result<()> {
    let _ = i;
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

/// Nested input loop for minibuffer reads. Runs while the outer
/// command loop is suspended inside `command_execute`.
fn minibuf_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    prompt: &EmacsStr,
    single: bool,
) -> Result<crate::lisp::eval::MinibufInput, crate::lisp::error::Flow> {
    use crate::lisp::eval::MinibufInput;
    // GNU's `read_minibuf' runs the ordinary command loop inside the
    // `exit' catch: keys dispatch through the minibuffer's keymaps,
    // `self-insert-command'/`delete-backward-char' edit the real
    // buffer (the prompt's `read-only'/`field' props protect it),
    // `pre-command-hook'/`post-command-hook' run per command (what
    // icomplete & friends hang on), and `exit-minibuffer' throws
    // `exit' to end the read.
    let mut keys: Vec<i128> = Vec::new();
    let mut arg_mode = false;
    loop {
        crate::lisp::eval::threads::yield_now(i)?;
        crate::lisp::builtins::evalfn::thread_wait_events(i)?;
        crate::editor::changes::run(i)?;
        {
            let mut t = term.borrow_mut();
            let shown = if single {
                prompt.to_string()
            } else {
                minibuf_echo_text(i, prompt)
            };
            let _ = t.draw_echo(&shown.to_unicode_lossy());
        }
        // Propagate io errors — swallowing them as None spins an
        // infinite poll loop on a dead input source.
        let code = match term.borrow_mut().poll_key(Duration::from_millis(50)) {
            Ok(Some(k)) => k,
            Ok(None) => continue,
            Err(e) => return Err(i.error(format!("minibuffer input: {e}"))),
        };
        if single {
            return Ok(MinibufInput::Key(code));
        }
        if let Err(e) = dispatch_key(term, i, code, &mut keys, &mut arg_mode) {
            return Err(i.error(format!("minibuffer input: {e}")));
        }
        if let Some(f) = i.minibuf_exit_fn.take() {
            return Ok(MinibufInput::Call(f));
        }
        if i.minibuf_exited {
            return Ok(MinibufInput::Text(i.minibuf_contents()));
        }
        if let Some(f) = i.minibuf_pending_flow.take() {
            return Err(f);
        }
    }
}

/// The minibuffer's display text for the echo area: buffer text plus
/// overlay `before-string'/`after-string' contributions, like GNU's
/// redisplay (icomplete shows its prospects through an `after-string'
/// on `icomplete-overlay').
fn signal_text(i: &Interp, symbol: &Value, data: &Value) -> EmacsText {
    let mut text = match symbol { Value::Sym(id) => i.symbol_name_text(*id), _ => EmacsText::from("error") };
    text.push_str(": ");
    let parts: Vec<_> = data.list_to_vec().unwrap_or_default().iter().map(|value| i.princ_to_string(value)).collect();
    text.append(&EmacsText::join(&parts, " "));
    text
}

fn minibuf_echo_text(i: &mut Interp, prompt: &EmacsStr) -> EmacsText {
    let Some(b) = i.buffers.get(i.current_buffer) else {
        return prompt.to_string();
    };
    let bb = b.borrow();
    let text = bb.text.text();
    // Collect (pos, string) splices; after-string goes at the
    // overlay's end, before-string at its start.
    let before_sym = i.q("before-string");
    let after_sym = i.q("after-string");
    let mut splices: Vec<(usize, usize, EmacsText)> = Vec::new();
    for ov in &bb.overlays {
        if ov.buffer.is_none() {
            continue;
        }
        let bs = crate::lisp::eval::plist_get(&ov.plist, before_sym);
        let as_ = crate::lisp::eval::plist_get(&ov.plist, after_sym);
        for (v, at_start) in [(bs, true), (as_, false)] {
            if let Value::Str(s) = &v {
                let pos = if at_start { ov.start } else { ov.end };
                splices.push((pos, usize::from(!at_start), s.borrow().clone()));
            }
        }
    }
    splice_echo_text(text, splices)
}

fn splice_echo_text(text: EmacsText, mut splices: Vec<(usize, usize, EmacsText)>) -> EmacsText {
    if splices.is_empty() {
        return text;
    }
    // Splice right-to-left so positions stay valid; later-positioned
    // strings at the same spot come after earlier ones.
    splices.sort_by_key(|&(pos, is_after, ref _s)| (pos, is_after));
    let mut chars: Vec<EmacsChar> = text.chars().collect();
    for (pos, _, s) in splices.into_iter().rev() {
        let p = pos.min(chars.len());
        let tail: Vec<EmacsChar> = chars.split_off(p);
        chars.extend(s.chars());
        chars.extend(tail);
    }
    chars.into_iter().collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
mod wide_echo_text_tests {
    use super::*;
    #[test]
    fn overlay_splices_use_character_offsets_and_preserve_order() {
        let text = EmacsText::from_codes([0xf0080, 0x110000]).unwrap();
        let before = EmacsText::from_codes([0xd800]).unwrap();
        let after = EmacsText::from_codes([0x3fff80]).unwrap();
        let rendered = splice_echo_text(text, vec![(1, 1, after), (1, 0, before)]);
        assert_eq!(rendered.codes().collect::<Vec<_>>(), [0xf0080, 0xd800, 0x3fff80, 0x110000]);
    }
    #[test]
    fn presentation_conversion_does_not_change_logical_echo_text() {
        let text = EmacsText::from_codes([0xf0100, 0x110000]).unwrap();
        let logical = splice_echo_text(text.clone(), Vec::new());
        assert_eq!(logical.to_unicode_lossy(), "\u{f0100}\u{fffd}");
        assert_eq!(logical, text);
    }
}
