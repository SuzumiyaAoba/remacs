use super::{
    PopupMenuPosition,
    render::{menu_keymap_from_definition, menu_label_and_definition},
};
use crate::lisp::{
    Interp, Value,
    error::{CaseHandler, EvalResult, Flow},
    obarray::sym,
};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

/// One row of a popup snapshot. Values remain on the Lisp logic thread.
#[derive(Clone)]
pub struct PopupMenuEntry {
    pub label: String,
    pub command: Option<String>,
    pub separator: bool,
    pub depth: usize,
    pub value: Option<Value>,
}

/// An immutable set of choices for one synchronous popup request.
pub struct PopupMenu {
    pub id: usize,
    pub position: PopupMenuPosition,
    pub entries: Vec<PopupMenuEntry>,
}

impl PopupMenuEntry {
    fn text(label: String, depth: usize) -> Self {
        Self {
            label,
            command: None,
            separator: false,
            depth,
            value: None,
        }
    }
    fn separator() -> Self {
        Self {
            separator: true,
            ..Self::text(String::new(), 0)
        }
    }
}

/// GNU keyboard.c menu_item_eval_property: dynamic eval with redisplay
/// inhibited, caught conditions becoming nil, and quit still propagating.
fn eval_property(i: &mut Interp, form: &Value) -> EvalResult {
    let mark = i.specbind_depth();
    let inhibit = i.q("inhibit-redisplay");
    if let Err(flow) = i.specbind(inhibit, Value::t()) {
        let _ = i.unbind_to(mark);
        return Err(flow);
    }
    let depth = i.case_handlers.len();
    i.case_handlers.push(CaseHandler::single(Value::t()));
    let result = crate::lisp::builtins::evalfn::f_eval(i, vec![form.clone(), Value::Nil].into());
    let result = i.offer_flow(result);
    i.case_handlers.truncate(depth);
    let result = match result {
        Err(Flow::Signal(signal)) if signal.claims_frame(depth) => {
            let quit = signal.is_symbol(sym::QUIT);
            i.finish_quit_unwind();
            if quit {
                Err(Flow::Quit)
            } else {
                Ok(Value::Nil)
            }
        }
        Err(flow) => {
            i.rebase_quit_unwind();
            Err(flow)
        }
        value => value,
    };
    i.unbind_to(mark)?;
    result
}

/// Check :visible in source order and retain the final :enable form.
fn menu_conditions(i: &mut Interp, binding: &Value) -> Result<Option<Value>, Flow> {
    let menu_item = i.q("menu-item");
    let Value::Cons(cell) = binding else {
        return Ok(Some(Value::t()));
    };
    if !matches!(cell.borrow().car, Value::Sym(symbol) if symbol == menu_item) {
        return Ok(Some(Value::t()));
    }
    let mut properties = binding.clone();
    for _ in 0..3 {
        properties = match properties {
            Value::Cons(cell) => cell.borrow().cdr.clone(),
            _ => return Ok(Some(Value::t())),
        };
    }
    let enable = i.q(":enable");
    let visible = i.q(":visible");
    let force = i.q("enable-disabled-menus-and-buttons");
    let mut enable_form = Value::t();
    let cache_tail = if let Value::Cons(cell) = &properties {
        let cell = cell.borrow();
        matches!(cell.car, Value::Cons(_)).then(|| cell.cdr.clone())
    } else {
        None
    };
    if let Some(tail) = cache_tail {
        properties = tail;
    }
    let mut seen = HashMap::new();
    while let Value::Cons(cell) = properties {
        if seen
            .insert(Rc::as_ptr(&cell) as usize, cell.clone())
            .is_some()
        {
            break;
        }
        let (property, rest) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let Value::Cons(value) = rest else {
            break;
        };
        let form = value.borrow().car.clone();
        if matches!(property, Value::Sym(symbol) if symbol == enable) {
            enable_form = if i.symbol_value(force).truthy() {
                Value::t()
            } else {
                form
            };
        } else if matches!(property, Value::Sym(symbol) if symbol == visible)
            && eval_property(i, &form)?.is_nil()
        {
            return Ok(None);
        }
        // Visibility callbacks may alter later property cells, as in GNU's
        // live plist walk. Retaining visited cells prevents address reuse.
        properties = value.borrow().cdr.clone();
    }
    Ok(Some(enable_form))
}

fn keymap_entries(
    i: &mut Interp,
    keymap: &Value,
    prefix: &[Value],
    depth: usize,
) -> Result<Vec<PopupMenuEntry>, Flow> {
    if depth >= 10 {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for (key, binding) in crate::editor::keymap_own_bindings(i, keymap) {
        // Retain Remacs's fallback key labels for bare bindings. An absent
        // binding is omitted; an explicitly labelled nil binding remains text.
        if binding.is_nil() {
            continue;
        }
        let Some(enable_form) = menu_conditions(i, &binding)? else {
            continue;
        };
        let (label, definition, separator) = menu_label_and_definition(i, &key, &binding);
        let enabled = matches!(enable_form, Value::Sym(symbol) if symbol == sym::T)
            || eval_property(i, &enable_form)?.truthy();
        if separator {
            entries.push(PopupMenuEntry::separator());
            continue;
        }
        let mut keys = prefix.to_vec();
        let event = match key {
            Value::Int(code) => crate::editor::key_name_for(code)
                .map(|name| Value::Sym(i.q(&name)))
                .unwrap_or(Value::Int(code)),
            other => other,
        };
        keys.push(event);
        if let Some(submenu) = menu_keymap_from_definition(i, &definition, depth + 1) {
            if !enabled {
                entries.push(PopupMenuEntry::text(label, depth));
                continue;
            }
            let children = keymap_entries(i, &submenu, &keys, depth + 1)?;
            if !children.is_empty() {
                entries.push(PopupMenuEntry::text(label, depth));
                entries.extend(children);
            }
        } else {
            let command = match &definition {
                Value::Sym(symbol) => Some(i.symbol_name(*symbol)),
                _ => None,
            };
            entries.push(PopupMenuEntry {
                label,
                command,
                separator: false,
                depth,
                value: (enabled && definition.truthy()).then(|| Value::list(keys)),
            });
        }
    }
    Ok(entries)
}

fn menu_entries(i: &mut Interp, menu: &Value) -> Result<Vec<PopupMenuEntry>, Flow> {
    if crate::editor::is_keymap(i, menu) {
        return keymap_entries(i, menu, &[], 0);
    }
    let Ok(parts) = menu.list_to_vec() else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    if parts
        .first()
        .is_some_and(|first| crate::editor::is_keymap(i, first))
    {
        for map in parts {
            let pane = keymap_entries(i, &map, &[], 0)?;
            if !pane.is_empty() {
                if !entries.is_empty() {
                    entries.push(PopupMenuEntry::separator());
                }
                entries.extend(pane);
            }
        }
    } else {
        for pane in parts.into_iter().skip(1) {
            let Ok(items) = pane.list_to_vec() else {
                continue;
            };
            if !entries.is_empty() {
                entries.push(PopupMenuEntry::separator());
            }
            if let Some(Value::Str(title)) = items.first() {
                let title = title.borrow().to_unicode_lossy();
                if !title.is_empty() {
                    entries.push(PopupMenuEntry::text(title, 0));
                }
            }
            for item in items.into_iter().skip(1) {
                match item {
                    Value::Str(label) => {
                        entries.push(PopupMenuEntry::text(label.borrow().to_unicode_lossy(), 1))
                    }
                    Value::Cons(cell) => {
                        let cell = cell.borrow();
                        if let Value::Str(label) = &cell.car {
                            entries.push(PopupMenuEntry {
                                label: label.borrow().to_unicode_lossy(),
                                command: None,
                                separator: false,
                                depth: 1,
                                value: Some(cell.cdr.clone()),
                            });
                        }
                    }
                    _ => entries.push(PopupMenuEntry::separator()),
                }
            }
        }
    }
    Ok(entries)
}

pub(crate) fn show_popup_menu(
    i: &mut Interp,
    menu: &Value,
    position: PopupMenuPosition,
) -> EvalResult {
    if !i.frames.iter().any(|frame| {
        let data = frame.borrow();
        data.id == position.frame_id && !data.dead && crate::editor::frame_is_graphical(i, frame)
    }) {
        return Ok(Value::Nil);
    }
    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
    let menu = Rc::new(PopupMenu {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        position,
        entries: menu_entries(i, menu)?,
    });
    let Some(reader) = i.popup_menu_reader.clone() else {
        // A headless host can inspect the snapshot without an input service.
        i.gui_popup_menu = Some(menu);
        return Ok(Value::Nil);
    };
    match reader(i, menu.clone())? {
        Some(index) => Ok(menu
            .entries
            .get(index)
            .and_then(|item| item.value.clone())
            .unwrap_or(Value::Nil)),
        None if position.from_event => Ok(Value::Nil),
        None => Err(Flow::Quit),
    }
}
