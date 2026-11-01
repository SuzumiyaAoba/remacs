//! Evaluate dynamic mode-line constructs before the read-only grid renderer.
use super::render::{
    ModeLineContext, format_mode_line_value, mode_line_minor_values, mode_line_variable,
};
use crate::{
    editor::{WindowRef, frame_of_window, sel_window},
    lisp::{Flow, Interp, Value},
};
use std::{collections::HashSet, rc::Rc};

const FORMATS: [&str; 3] = ["mode-line-format", "header-line-format", "tab-line-format"];
const CACHE: &str = "remacs--mode-line-formats";

pub(super) fn format(i: &Interp, buffer: usize, window: usize, name: &str) -> Value {
    let original = mode_line_variable(i, buffer, name).unwrap_or(Value::Nil);
    let Some(index) = FORMATS.iter().position(|candidate| *candidate == name) else {
        return original;
    };
    let Some(cache) = mode_line_variable(i, buffer, CACHE) else {
        return original;
    };
    let Ok(entries) = cache.list_to_vec() else {
        return original;
    };
    for entry in entries {
        let Value::Cons(entry) = entry else {
            continue;
        };
        let entry = entry.borrow();
        if entry.car.int() != Some(window as i128) {
            continue;
        }
        let Ok(formats) = entry.cdr.list_to_vec() else {
            break;
        };
        let Some(Value::Cons(cached)) = formats.get(index) else {
            break;
        };
        let cached = cached.borrow();
        if crate::lisp::eq_values(&original, &cached.car) {
            return cached.cdr.clone();
        }
        break;
    }
    original
}

fn with_window<T>(
    i: &mut Interp,
    window: &WindowRef,
    body: impl FnOnce(&mut Interp) -> Result<T, Flow>,
) -> Result<T, Flow> {
    let frame =
        frame_of_window(i, window).ok_or_else(|| i.error("Mode-line window has no frame"))?;
    let caller = i.current_buffer;
    let old_frame = i.selected_frame.clone();
    let old_window = sel_window(i);
    let mut frames = vec![frame.clone()];
    if let Some(old) = &old_frame
        && !Rc::ptr_eq(old, &frame)
    {
        frames.push(old.clone());
    }
    let frames: Vec<_> = frames
        .into_iter()
        .map(|frame| {
            let (selected, old_selected) = {
                let frame = frame.borrow();
                (frame.selected.clone(), frame.old_selected.clone())
            };
            (frame, selected, old_selected)
        })
        .collect();
    let mut windows = vec![window.clone()];
    windows.extend(old_window.iter().cloned());
    windows.extend(frames.iter().map(|(_, selected, _)| selected.clone()));
    let window_points: Vec<_> = windows
        .iter()
        .map(|window| (window.clone(), window.borrow().point))
        .collect();
    let buffer = window.borrow().buffer;
    let mut seen = HashSet::new();
    let points: Vec<_> = std::iter::once(caller)
        .chain(windows.iter().map(|window| window.borrow().buffer))
        .filter(|buffer| seen.insert(*buffer))
        .filter_map(|buffer| {
            i.buffers
                .get(buffer)
                .map(|value| (buffer, value.borrow().point()))
        })
        .collect();
    let selected = old_window
        .as_ref()
        .is_some_and(|old| Rc::ptr_eq(old, window));
    let matches = i.match_data.clone();
    let mark = i.specbind_depth();
    let result = (|| {
        let old_selected = i.q("remacs--mode-line-selected-window");
        i.specbind(
            old_selected,
            old_window.map(Value::Window).unwrap_or(Value::Nil),
        )?;
        frame.borrow_mut().selected = window.clone();
        i.selected_frame = Some(frame);
        i.set_current_buffer(buffer);
        if !selected && let Some(buffer) = i.buffers.get(buffer) {
            buffer.borrow_mut().set_point(window.borrow().point);
        }
        body(i)
    })();
    let unwind = i.unbind_to(mark);
    for (buffer, point) in points {
        if let Some(buffer) = i.buffers.get(buffer) {
            buffer.borrow_mut().set_point(point);
        }
    }
    for (window, point) in window_points {
        if !window.borrow().dead {
            window.borrow_mut().point = point;
        }
    }
    for (frame, selected, old_selected) in frames {
        if i.frames.iter().any(|live| Rc::ptr_eq(live, &frame)) && !selected.borrow().dead {
            let mut frame = frame.borrow_mut();
            frame.selected = selected;
            frame.old_selected = old_selected;
        }
    }
    if let Some(frame) = old_frame
        && i.frames.iter().any(|live| Rc::ptr_eq(live, &frame))
    {
        i.selected_frame = Some(frame);
    }
    if i.buffer_live(caller) {
        i.set_current_buffer(caller);
    }
    i.match_data = matches;
    let result = result?;
    unwind?;
    Ok(result)
}

fn evaluate(i: &mut Interp, form: &Value) -> Result<Value, Flow> {
    evaluate_with_quit(i, form, true)
}

pub(super) fn call_for_redisplay(
    i: &mut Interp,
    function: Value,
    args: Vec<Value>,
) -> Result<Value, Flow> {
    if i.q_soft("inhibit-eval-during-redisplay")
        .is_some_and(|symbol| {
            let value = i.symbol_value(symbol);
            value.truthy() && !matches!(value, Value::Sym(crate::lisp::obarray::sym::UNBOUND))
        })
    {
        return Ok(Value::Nil);
    }
    let quote = Value::Sym(i.q("quote"));
    let form = Value::list(
        std::iter::once(function)
            .chain(
                args.into_iter()
                    .map(|arg| Value::list(vec![quote.clone(), arg])),
            )
            .collect(),
    );
    evaluate_with_quit(i, &form, false)
}

fn evaluate_with_quit(i: &mut Interp, form: &Value, inhibit_quit: bool) -> Result<Value, Flow> {
    let mark = i.specbind_depth();
    let result = (|| {
        let redisplay = i.q("inhibit-redisplay");
        i.specbind(redisplay, Value::t())?;
        if inhibit_quit {
            let quit = i.q("inhibit-quit");
            i.specbind(quit, Value::t())?;
        }
        let eval = Value::Sym(i.q("eval"));
        let quote = Value::Sym(i.q("quote"));
        let condition_case = Value::Sym(i.q("condition-case"));
        // A catch-all handler suppresses the debugger without changing the
        // debug options which the user's expression may itself inspect.
        let guarded = Value::list(vec![
            condition_case,
            Value::Nil,
            Value::list(vec![
                eval,
                Value::list(vec![quote, form.clone()]),
                Value::t(),
            ]),
            Value::list(vec![Value::t(), Value::Nil]),
        ]);
        match i.with_eval_context(false, |i| i.eval(&guarded)) {
            Err(Flow::Signal(_)) | Err(Flow::Quit) => Ok(Value::Nil),
            result => result,
        }
    })();
    let unwind = i.unbind_to(mark);
    let result = result?;
    unwind?;
    Ok(result)
}

fn prepare_value(
    i: &mut Interp,
    buffer: usize,
    value: &Value,
    depth: usize,
    unsafe_value: bool,
    fill: &mut bool,
) -> Result<Value, Flow> {
    if depth > 32 || *fill {
        return Ok(Value::Nil);
    }
    match value {
        Value::Sym(symbol) => {
            let name = i.symbol_name(*symbol);
            let risky = i.q("risky-local-variable");
            let unsafe_value = unsafe_value || !i.get_prop(*symbol, risky).truthy();
            if name == "mode-line-minor-modes" {
                let values = mode_line_minor_values(i, buffer);
                let mut result = Vec::new();
                for value in values {
                    result.push(prepare_value(
                        i,
                        buffer,
                        &value,
                        depth + 1,
                        unsafe_value,
                        fill,
                    )?);
                }
                return Ok(Value::list(result));
            }
            let Some(next) = mode_line_variable(i, buffer, &name) else {
                return Ok(Value::Nil);
            };
            if crate::lisp::eq_values(value, &next) {
                return Ok(Value::Nil);
            }
            prepare_value(i, buffer, &next, depth + 1, unsafe_value, fill)
        }
        Value::Cons(_) => {
            let Ok(mut parts) = value.list_to_vec() else {
                return Ok(Value::Nil);
            };
            if parts.is_empty() {
                return Ok(Value::Nil);
            }
            if let Value::Sym(head) = parts[0] {
                match i.symbol_name(head).as_str() {
                    ":eval" => {
                        if unsafe_value {
                            return Ok(Value::Nil);
                        }
                        let Some(form) = parts.get(1) else {
                            return Ok(Value::Nil);
                        };
                        let next = evaluate(i, form)?;
                        return prepare_value(i, buffer, &next, depth + 1, false, fill);
                    }
                    ":propertize" => {
                        if unsafe_value {
                            return Ok(Value::Nil);
                        }
                        if let Some(body) = parts.get(1).cloned() {
                            parts[1] = prepare_value(i, buffer, &body, depth + 1, false, fill)?;
                        }
                        return Ok(Value::list(parts));
                    }
                    _ => {}
                }
                if parts.len() > 1 {
                    let condition = mode_line_variable(i, buffer, &i.symbol_name(head))
                        .unwrap_or(Value::Nil)
                        .truthy();
                    return match parts.get(if condition { 1 } else { 2 }) {
                        Some(branch) => {
                            prepare_value(i, buffer, branch, depth + 1, unsafe_value, fill)
                        }
                        None => Ok(Value::Nil),
                    };
                }
            }
            let mut result = Vec::new();
            for (index, part) in parts.iter().enumerate() {
                result.push(if index == 0 && matches!(part, Value::Int(_)) {
                    part.clone()
                } else {
                    prepare_value(i, buffer, part, depth + 1, unsafe_value, fill)?
                });
            }
            Ok(Value::list(result))
        }
        Value::Vec(values) => {
            let values = values.borrow().clone();
            let mut result = Vec::new();
            for value in values {
                result.push(prepare_value(
                    i,
                    buffer,
                    &value,
                    depth + 1,
                    unsafe_value,
                    fill,
                )?);
            }
            Ok(Value::vector(result))
        }
        _ => {
            let _ = format_mode_line_value(i, buffer, value, &ModeLineContext::default(), 0, fill);
            Ok(value.clone())
        }
    }
}

pub(super) fn prepare(i: &mut Interp, windows: &[WindowRef]) -> Result<(), Flow> {
    let cache = i.intern(CACHE);
    let mut buffers = HashSet::new();
    for window in windows {
        let buffer = window.borrow().buffer;
        if buffers.insert(buffer)
            && let Some(buffer) = i.buffers.get(buffer)
        {
            buffer.borrow_mut().locals.insert(cache, Value::Nil);
        }
    }
    for window in windows {
        let (buffer, id, skip) = {
            let window = window.borrow();
            (window.buffer, window.id, window.dead || window.minibuffer)
        };
        if skip || !i.buffer_live(buffer) {
            continue;
        }
        let mut formats = Vec::new();
        for name in FORMATS {
            let original = mode_line_variable(i, buffer, name).unwrap_or(Value::Nil);
            let prepared = with_window(i, window, |i| {
                prepare_value(i, buffer, &original, 0, false, &mut false)
            })?;
            if window.borrow().dead || frame_of_window(i, window).is_none() {
                break;
            }
            formats.push(Value::cons(original, prepared));
        }
        if formats.len() != FORMATS.len() || window.borrow().buffer != buffer {
            continue;
        }
        if let Some(buffer) = i.buffers.get(buffer) {
            let mut buffer = buffer.borrow_mut();
            let previous = buffer.locals.get(&cache).cloned().unwrap_or(Value::Nil);
            buffer.locals.insert(
                cache,
                Value::cons(
                    Value::cons(Value::Int(id as i128), Value::list(formats)),
                    previous,
                ),
            );
        }
    }
    Ok(())
}
