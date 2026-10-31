use super::{
    PopupMenuPosition,
    render::{menu_keymap_from_definition, menu_label_and_definition},
};
use crate::lisp::{
    Interp, Value,
    error::{EvalResult, Flow},
};
use std::{
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

fn keymap_entries(
    i: &mut Interp,
    keymap: &Value,
    prefix: &[Value],
    depth: usize,
) -> Vec<PopupMenuEntry> {
    if depth >= 10 {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for (key, binding) in crate::editor::keymap_own_bindings(i, keymap) {
        // Retain Remacs's fallback key labels for bare bindings. An absent
        // binding is omitted; an explicitly labelled nil binding remains text.
        if binding.is_nil() {
            continue;
        }
        let (label, definition, separator) = menu_label_and_definition(i, &key, &binding);
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
            let children = keymap_entries(i, &submenu, &keys, depth + 1);
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
                value: definition.truthy().then(|| Value::list(keys)),
            });
        }
    }
    entries
}

fn menu_entries(i: &mut Interp, menu: &Value) -> Vec<PopupMenuEntry> {
    if crate::editor::is_keymap(i, menu) {
        return keymap_entries(i, menu, &[], 0);
    }
    let Ok(parts) = menu.list_to_vec() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    if parts
        .first()
        .is_some_and(|first| crate::editor::is_keymap(i, first))
    {
        for map in parts {
            let pane = keymap_entries(i, &map, &[], 0);
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
    entries
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
        entries: menu_entries(i, menu),
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
