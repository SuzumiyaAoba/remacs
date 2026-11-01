use super::{
    PopupMenuPosition,
    render::{menu_keymap_from_definition, menu_label_and_definition, menu_separator_name_p},
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

struct MenuProperties {
    enable: Value,
    /// Retain the value cell, since a dynamic name can replace its function.
    filter: Option<Value>,
    selected: Value,
}

/// Check :visible in source order and retain the final property fields.
fn menu_properties(i: &mut Interp, mut properties: Value) -> Result<Option<MenuProperties>, Flow> {
    let enable = i.q(":enable");
    let visible = i.q(":visible");
    let filter = i.q(":filter");
    let button = i.q(":button");
    let toggle = i.q(":toggle");
    let radio = i.q(":radio");
    let force = i.q("enable-disabled-menus-and-buttons");
    let mut enable_form = Value::t();
    let mut filter_cell = None;
    let mut selected = Value::Nil;
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
        } else if matches!(property, Value::Sym(symbol) if symbol == filter) {
            filter_cell = Some(Value::Cons(value.clone()));
        } else if matches!(property, Value::Sym(symbol) if symbol == button)
            && let Value::Cons(button) = form
        {
            let button = button.borrow();
            if matches!(button.car, Value::Sym(symbol) if symbol == toggle || symbol == radio) {
                selected = button.cdr.clone();
            }
        }
        // Visibility callbacks may alter later property cells, as in GNU's
        // live plist walk. Retaining visited cells prevents address reuse.
        properties = value.borrow().cdr.clone();
    }
    Ok(Some(MenuProperties {
        enable: enable_form,
        filter: filter_cell,
        selected,
    }))
}

pub(super) struct ParsedMenuItem {
    pub(super) label: String,
    pub(super) definition: Value,
    pub(super) separator: bool,
    pub(super) enabled: bool,
    selected: Value,
}

impl ParsedMenuItem {
    // GNU evaluates the selected form only for leaf commands, after the name,
    // filter and enable properties. Submenus and plain text skip this step.
    pub(super) fn button_selected(&self, i: &mut Interp) -> Result<bool, Flow> {
        Ok(!self.selected.is_nil() && eval_property(i, &self.selected)?.truthy())
    }
}

pub(super) fn keymap_item(
    i: &mut Interp,
    key: &Value,
    binding: &Value,
) -> Result<Option<ParsedMenuItem>, Flow> {
    let menu_item = i.q("menu-item");
    if let Value::Cons(head) = binding {
        let (tag, tail) = {
            let head = head.borrow();
            (head.car.clone(), head.cdr.clone())
        };
        if matches!(tag, Value::Sym(symbol) if symbol == menu_item) {
            let Value::Cons(name_cell) = tail else {
                return Ok(None);
            };
            let (name, tail) = {
                let cell = name_cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            let (mut definition, tail) = match tail {
                Value::Cons(cell) => {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                }
                Value::Nil => (Value::Nil, Value::Nil),
                _ => return Ok(None),
            };
            let Some(properties) = menu_properties(i, tail)? else {
                return Ok(None);
            };
            let name = if matches!(name, Value::Str(_)) {
                name
            } else {
                eval_property(i, &name)?
            };
            let Value::Str(name) = name else {
                return Ok(None);
            };
            if let Some(Value::Cons(filter)) = properties.filter {
                let function = filter.borrow().car.clone();
                let quoted = Value::list(vec![Value::Sym(sym::QUOTE), definition]);
                definition = eval_property(i, &Value::list(vec![function, quoted]))?;
            }
            let enabled = matches!(properties.enable, Value::Sym(symbol) if symbol == sym::T)
                || eval_property(i, &properties.enable)?.truthy();
            let label = name.borrow().to_unicode_lossy();
            let separator = menu_separator_name_p(&label);
            return Ok(Some(ParsedMenuItem {
                label,
                definition,
                separator,
                enabled,
                selected: properties.selected,
            }));
        }
    }
    let (label, definition, separator) = menu_label_and_definition(i, key, binding);
    Ok(Some(ParsedMenuItem {
        label,
        definition,
        separator,
        enabled: true,
        selected: Value::Nil,
    }))
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
        let Some(ParsedMenuItem {
            label,
            definition,
            separator,
            enabled,
            ..
        }) = keymap_item(i, &key, &binding)? else {
            continue;
        };
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
        let resolved = crate::editor::keymap_def(i, definition.clone())?;
        if let Some(submenu) = menu_keymap_from_definition(i, &resolved, depth + 1) {
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

fn titled_entries(mut entries: Vec<PopupMenuEntry>, title: Value) -> Vec<PopupMenuEntry> {
    if let Value::Str(title) = title {
        entries.insert(0, PopupMenuEntry::separator());
        entries.insert(
            0,
            PopupMenuEntry::text(title.borrow().to_unicode_lossy(), 0),
        );
    }
    entries
}

fn menu_entries(i: &mut Interp, menu: &Value) -> Result<Vec<PopupMenuEntry>, Flow> {
    if crate::editor::is_keymap(i, menu) {
        let entries = keymap_entries(i, menu, &[], 0)?;
        // A single keymap's prompt is obtained after its dynamic items.
        let title = crate::editor::f_keymap_prompt(i, vec![menu.clone()].into())?;
        if let Value::Str(prompt) = &title {
            if !prompt.borrow().is_empty() {
                let hide = i.q("hide");
                let properties = Value::list(crate::buffer::primitives::str_plist_at(
                    i.str_props(prompt),
                    0,
                ));
                if crate::lisp::eval::plist_get(&properties, hide).truthy() {
                    return Ok(entries);
                }
            }
        }
        return Ok(titled_entries(entries, title));
    }
    let Ok(parts) = menu.list_to_vec() else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    let mut title = parts.first().cloned().unwrap_or(Value::Nil);
    if parts
        .first()
        .is_some_and(|first| crate::editor::is_keymap(i, first))
    {
        // For a list of maps, GNU captures the first prompt before computing
        // any items. Keep its string object through those computations.
        title = Value::Nil;
        for map in &parts {
            let prompt = crate::editor::f_keymap_prompt(i, vec![map.clone()].into())?;
            if title.is_nil() && !prompt.is_nil() {
                title = prompt;
            }
        }
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
    Ok(titled_entries(entries, title))
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
