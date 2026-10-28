//! Redisplay change notifications and the old-buffer snapshots they publish.

use super::*;

pub(crate) fn frame_windows(frame: &FrameRef) -> Vec<WindowRef> {
    let frame = frame.borrow();
    frame
        .windows
        .iter()
        .chain(frame.minibuffer.iter())
        .filter(|w| !w.borrow().dead)
        .cloned()
        .collect()
}

fn window_size(i: &Interp, window: &WindowRef) -> [usize; 5] {
    let body = win_body_width(i, window);
    let window = window.borrow();
    [
        window.width,
        window.height,
        body,
        window.margins.0,
        window.margins.1,
    ]
}

fn call_list(i: &mut Interp, mut list: Value, args: &[Value], muted: bool) -> Result<(), Flow> {
    let mut seen = std::collections::HashSet::new();
    while let Value::Cons(cell) = list {
        if !seen.insert(Rc::as_ptr(&cell) as usize) {
            break;
        }
        let function = cell.borrow().car.clone();
        if !matches!(function, Value::Sym(sym::T)) {
            match i.apply(&function, args.to_vec()) {
                Ok(_) => (),
                Err(Flow::Signal(signal)) if muted => {
                    let error = signal.condition;
                    let call = Value::cons(function, Value::list(args.to_vec()));
                    let message = format!(
                        "Error muted by safe_call: {} signaled {}",
                        i.prin1_to_string(&call),
                        i.prin1_to_string(&error)
                    );
                    i.message_log(&message);
                }
                Err(flow) if muted && flow.is_quit() => (),
                Err(flow) => return Err(flow),
            }
        }
        list = cell.borrow().cdr.clone();
    }
    Ok(())
}

fn local(i: &mut Interp, hook: &str, buffer: usize, window: &WindowRef) -> Result<(), Flow> {
    if !i.buffer_live(buffer) {
        return Ok(());
    }
    let symbol = i.intern(hook);
    let functions = i
        .buffers
        .get(buffer)
        .and_then(|b| b.borrow().locals.get(&symbol).cloned());
    if let Some(functions) = functions {
        let previous = i.current_buffer;
        i.set_current_buffer(buffer);
        let result = call_list(i, functions, &[Value::Window(window.clone())], true);
        if i.buffer_live(previous) {
            i.set_current_buffer(previous);
        }
        result?;
    }
    Ok(())
}

fn global(i: &mut Interp, hook: &str, frame: &FrameRef) -> Result<(), Flow> {
    let symbol = i.intern(hook);
    let functions = i.obarray.symbol(symbol).value.clone();
    call_list(i, functions, &[Value::Frame(frame.clone())], true)
}

fn configuration_hook(i: &mut Interp, frame: &FrameRef) -> Result<(), Flow> {
    let symbol = i.q("window-configuration-change-hook");
    let default = i.obarray.symbol(symbol).value.clone();
    let original_frame = i.selected_frame.clone();
    let original_buffer = i.current_buffer;
    let original_window = sel_window(i);
    let restore_buffer = original_window
        .as_ref()
        .is_some_and(|w| w.borrow().buffer != original_buffer);
    if let Some(window) = &original_window {
        i.set_current_buffer(window.borrow().buffer);
    }
    let switched_frame = original_frame
        .as_ref()
        .is_none_or(|old| !Rc::ptr_eq(old, frame));
    let result = (|| {
        if switched_frame {
            let window = frame.borrow().selected.clone();
            f_select_window(i, vec![Value::Window(window), Value::t()].into())?;
        }
        let windows = frame.borrow().windows.clone();
        for window in windows {
            if window.borrow().dead {
                continue;
            }
            let buffer = window.borrow().buffer;
            let functions = i
                .buffers
                .get(buffer)
                .and_then(|b| b.borrow().locals.get(&symbol).cloned());
            if let Some(functions) = functions {
                let previous = sel_window(i);
                f_select_window(i, vec![Value::Window(window), Value::t()].into())?;
                let result = call_list(i, functions, &[], false);
                if let Some(previous) = previous.filter(|w| !w.borrow().dead) {
                    let _ = f_select_window(i, vec![Value::Window(previous), Value::t()].into());
                }
                result?;
            }
        }
        call_list(i, default, &[], false)
    })();
    if switched_frame {
        if let Some(frame) = original_frame.filter(|f| !f.borrow().dead) {
            let window = frame.borrow().selected.clone();
            if !window.borrow().dead {
                let _ = f_select_window(i, vec![Value::Window(window), Value::t()].into());
            }
        }
    }
    if restore_buffer && i.buffer_live(original_buffer) {
        i.set_current_buffer(original_buffer);
    }
    result
}

fn record(i: &Interp) {
    for frame in i.frames.iter().filter(|f| !f.borrow().dead) {
        let windows = frame_windows(frame);
        let (stamp, id, clock) = {
            let mut data = frame.borrow_mut();
            let selected = data.selected.clone();
            let selected_id = selected.borrow().id;
            let stamp = data.change_stamp.get().wrapping_add(1).max(1);
            data.change_stamp.set(stamp);
            data.change_window_count = windows.len();
            data.change_selected = Some(selected_id);
            data.change_frame_selected = i
                .selected_frame
                .as_ref()
                .is_some_and(|f| Rc::ptr_eq(f, frame));
            data.change_size = Some((data.width, data.height));
            data.window_state_change = false;
            data.old_selected = Some(selected);
            (stamp, data.id, data.change_stamp.clone())
        };
        for window in windows {
            let size = window_size(i, &window);
            let mut data = window.borrow_mut();
            data.old_buffer = i.buffer_value(data.buffer).unwrap_or(Value::Nil);
            data.change_stamp = stamp;
            data.change_frame = Some(id);
            data.change_clock = Some(clock.clone());
            data.change_size = Some(size);
        }
    }
}

/// Called from a mutable, non-recursive redisplay boundary. Batch evaluation
/// does not call this, just as GNU's batch frames do not run change functions.
pub fn run(i: &mut Interp) -> Result<bool, Flow> {
    let inhibit = i.q("inhibit-redisplay");
    if i.symbol_value(inhibit).truthy() {
        return Ok(false);
    }
    let mark = i.specbind_depth();
    i.specbind(inhibit, Value::t())?;
    let mut changed = false;
    let result = (|| {
        for frame in i.frames.clone() {
            if frame.borrow().dead {
                continue;
            }
            let windows = frame_windows(&frame);
            let (stamp, old_selected, selected, frame_selected, frame_size, flag, previous_count) = {
                let data = frame.borrow();
                (
                    data.change_stamp.get(),
                    data.change_selected,
                    data.selected.borrow().id,
                    data.change_frame_selected
                        != i.selected_frame
                            .as_ref()
                            .is_some_and(|f| Rc::ptr_eq(f, &frame)),
                    data.change_size != Some((data.width, data.height)),
                    data.window_state_change,
                    data.change_window_count,
                )
            };
            let selection = old_selected != Some(selected);
            let mut buffers = false;
            let mut sizes = frame_size;
            for window in &windows {
                if window.borrow().dead {
                    continue;
                }
                let size = window_size(i, window);
                let (buffer, old, buffer_change, size_change, selection_change) = {
                    let data = window.borrow();
                    let same_buffer = matches!(&data.old_buffer, Value::Buffer(b) if b.borrow().id == data.buffer);
                    let buffer_change = !same_buffer || data.change_stamp != stamp;
                    (
                        data.buffer,
                        data.old_buffer.clone(),
                        buffer_change,
                        buffer_change || data.change_size != Some(size),
                        (selection || frame_selected)
                            && (old_selected == Some(data.id) || selected == data.id),
                    )
                };
                buffers |= buffer_change;
                sizes |= size_change;
                changed |= buffer_change || size_change || selection_change || flag;
                if buffer_change {
                    if let Value::Buffer(old) = old {
                        let old = old.borrow().id;
                        local(i, "window-buffer-change-functions", old, window)?;
                    }
                    local(i, "window-buffer-change-functions", buffer, window)?;
                }
                if !window.borrow().dead && size_change {
                    local(i, "window-size-change-functions", buffer, window)?;
                }
                if !window.borrow().dead && selection_change {
                    local(i, "window-selection-change-functions", buffer, window)?;
                }
                if !window.borrow().dead && (buffer_change || size_change || selection_change) {
                    local(i, "window-state-change-functions", buffer, window)?;
                }
            }
            let deleted = windows.len() < previous_count;
            if frame.borrow().dead {
                continue;
            }
            if buffers || deleted {
                global(i, "window-buffer-change-functions", &frame)?;
            }
            if sizes {
                global(i, "window-size-change-functions", &frame)?;
            }
            if selection || frame_selected {
                global(i, "window-selection-change-functions", &frame)?;
            }
            if buffers || sizes || deleted || selection || frame_selected || flag {
                changed = true;
                global(i, "window-state-change-functions", &frame)?;
            }
            if sizes || deleted {
                configuration_hook(i, &frame)?;
            }
        }
        if changed {
            crate::lisp::builtins::evalfn::safe_call_hook(i, "window-state-change-hook")?;
        }
        Ok(())
    })();
    if changed {
        record(i);
    }
    let _ = i.unbind_to(mark);
    result.map(|()| changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notify(i: &mut Interp, _args: crate::lisp::Argv) -> crate::lisp::EvalResult {
        run(i)?;
        Ok(Value::Nil)
    }

    static NOTIFY: Subr = crate::lisp::builtins::S!("qa289-notify", 0, 0, notify, "");

    #[test]
    fn notifications_match_gnu_terminal_results() {
        std::thread::Builder::new()
            .stack_size(128 << 20)
            .spawn(|| {
                let mut i = Interp::new();
                let name = i.q("qa289-notify");
                i.fset(name, Value::Subr(&NOTIFY));
                let path = concat!(
                    env!("REMACS_SOURCE_ROOT"),
                    "/tests/window_change_notifications.el"
                );
                i.eval_str(&format!("(load {:?} nil t)", path)).unwrap();
                let probes = [
                    "qa289-basic",
                    "qa289-dead",
                    "qa289-hooks",
                    "qa289-mutating",
                    "qa289-throw",
                ];
                let expected: Vec<_> =
                    include_str!("../../../../tests/window_change_notifications.expected")
                        .lines()
                        .collect();
                assert_eq!(probes.len(), expected.len());
                for (probe, expected) in probes.iter().zip(expected) {
                    let result = i.eval_str(&format!("({probe})")).unwrap();
                    assert_eq!(i.prin1_to_string(&result), expected, "{probe}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
