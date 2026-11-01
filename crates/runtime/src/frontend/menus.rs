use super::{
    GridMenu, GridMenuItem,
    popup::{keymap_item, safe_menu_call},
    render::{menu_keymap_from_definition, menu_separator_name_p},
};
use crate::lisp::{Interp, Value, error::Flow};

fn canonical_bindings(i: &mut Interp, map: &Value) -> Result<Vec<(Value, Value)>, Flow> {
    let function = Value::Sym(i.q("keymap-canonicalize"));
    let canonical = safe_menu_call(i, function, vec![map.clone()])?;
    Ok(crate::editor::keymap_own_bindings(i, &canonical))
}

fn menu_name(name: &Value) -> String {
    match name {
        Value::Str(name) => name.borrow().to_unicode_lossy(),
        _ => String::new(),
    }
}

fn menu_items(i: &mut Interp, map: &Value, depth: usize) -> Result<Vec<GridMenuItem>, Flow> {
    if depth >= 10 {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for (key, binding) in canonical_bindings(i, map)? {
        if binding.is_nil() {
            continue;
        }
        let Some(item) = keymap_item(i, &key, &binding)? else {
            continue;
        };
        if depth == 0 && !item.enabled {
            continue;
        }
        let definition = crate::editor::keymap_def(i, item.definition.clone())?;
        let submenu = menu_keymap_from_definition(i, &definition, depth + 1);
        let leaf_command = depth > 0 && !definition.is_nil() && submenu.is_none();
        let key_hint = item.keyboard_equivalent(i, leaf_command)?;
        let checked = leaf_command && item.button_selected(i)?;
        let label = menu_name(&item.name);
        if menu_separator_name_p(&label) {
            items.push(GridMenuItem {
                label,
                command: None,
                items: Vec::new(),
                separator: true,
                checked: false,
                key_hint: None,
            });
            continue;
        }
        if let Some(map) = submenu {
            let children = if item.enabled {
                menu_items(i, &map, depth + 1)?
            } else {
                Vec::new()
            };
            items.push(GridMenuItem {
                label,
                command: None,
                items: children,
                separator: false,
                checked: false,
                key_hint,
            });
        } else {
            let command = match definition {
                Value::Sym(command) if item.enabled && i.fbound_p(command) => {
                    Some(i.symbol_name(command))
                }
                _ => None,
            };
            items.push(GridMenuItem {
                label,
                command,
                items: Vec::new(),
                separator: false,
                checked,
                key_hint,
            });
        }
    }
    Ok(items)
}

struct MenuBarEntry {
    key: Value,
    name: Value,
    definitions: Vec<Value>,
}

struct MenuPane {
    name: Value,
    items: Vec<GridMenuItem>,
}

fn same_menu_key(i: &Interp, left: &Value, right: &Value) -> bool {
    if crate::lisp::builtins::eq_values_with_interp(i, left, right) {
        return true;
    }
    // Folded boot maps may use the frontend's integer representation of a
    // named event while runtime define-key retains its symbol.
    match (left, right) {
        (Value::Sym(symbol), Value::Int(code)) | (Value::Int(code), Value::Sym(symbol)) => {
            crate::editor::key_name_for(*code).is_some_and(|name| name == i.symbol_name(*symbol))
        }
        _ => false,
    }
}

fn collect_menu_bar(i: &mut Interp) -> Result<Vec<MenuBarEntry>, Flow> {
    let maps = crate::lisp::builtins::misc::menu_bar_maps(i)?;
    let menu_bar_code = crate::editor::event_code_for("menu-bar");
    let undefined = i.q("undefined");
    let menu_item = i.q("menu-item");
    let mut entries: Vec<MenuBarEntry> = Vec::new();
    // GNU visits global maps first, appending new root entries and prepending
    // higher-priority definitions to an existing root's list of panes.
    for map in maps.into_iter().rev() {
        let map = crate::editor::keymap_def(i, map)?;
        if !crate::editor::is_keymap(i, &map) {
            continue;
        }
        let definition = crate::editor::lookup_in_keymap(i, &map, menu_bar_code, true)?;
        let map = crate::editor::keymap_def(i, definition)?;
        if !crate::editor::is_keymap(i, &map) {
            continue;
        }
        let mut seen = Vec::new();
        for (key, binding) in canonical_bindings(i, &map)? {
            if matches!(binding, Value::Sym(symbol) if symbol == undefined) {
                entries.retain(|entry| !same_menu_key(i, &entry.key, &key));
            }
            if binding.is_nil() || seen.iter().any(|old| same_menu_key(i, old, &key)) {
                continue;
            }
            seen.push(key.clone());
            // A bare command is not a labelled menubar entry. In particular,
            // undefined cancels lower-priority entries without adding a label.
            let labelled = match &binding {
                Value::Cons(cell) => {
                    let head = &cell.borrow().car;
                    matches!(head, Value::Str(_))
                        || matches!(head, Value::Sym(symbol) if *symbol == menu_item)
                }
                _ => false,
            };
            if !labelled {
                continue;
            }
            let Some(item) = keymap_item(i, &key, &binding)? else {
                continue;
            };
            if !item.enabled || item.definition.is_nil() || item.separator {
                continue;
            }
            let definition = crate::editor::keymap_def(i, item.definition)?;
            if let Some(old) = entries
                .iter_mut()
                .find(|entry| same_menu_key(i, &entry.key, &key))
            {
                if !(crate::editor::is_keymap(i, &definition)
                    && old
                        .definitions
                        .first()
                        .is_some_and(|map| crate::editor::is_keymap(i, map)))
                {
                    old.definitions.clear();
                }
                old.definitions.insert(0, definition);
            } else {
                entries.push(MenuBarEntry {
                    key,
                    name: item.name,
                    definitions: vec![definition],
                });
            }
        }
    }
    let finals = i.q("menu-bar-final-items");
    let mut finals = i.symbol_value(finals);
    let mut visited = std::collections::HashSet::new();
    while let Value::Cons(cell) = finals {
        if !visited.insert(std::rc::Rc::as_ptr(&cell)) {
            break;
        }
        let (key, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        if let Some(index) = entries
            .iter()
            .position(|entry| same_menu_key(i, &entry.key, &key))
        {
            let entry = entries.remove(index);
            entries.push(entry);
        }
        finals = next;
    }
    Ok(entries)
}

fn compute(i: &mut Interp) -> Result<Vec<GridMenu>, Flow> {
    let mode = i.q("menu-bar-mode");
    if !i.symbol_value(mode).truthy() {
        return Ok(Vec::new());
    }
    let mut pending = Vec::new();
    for entry in collect_menu_bar(i)? {
        let command = match entry.definitions.first() {
            Some(Value::Sym(symbol)) => Some(i.symbol_name(*symbol)),
            _ => None,
        };
        let mut panes = Vec::new();
        for map in entry.definitions {
            if !crate::editor::is_keymap(i, &map) {
                continue;
            }
            let prompt = crate::editor::f_keymap_prompt(i, vec![map.clone()].into())?;
            let name = if prompt.is_nil() {
                entry.name.clone()
            } else {
                prompt
            };
            panes.push(MenuPane {
                name,
                items: menu_items(i, &map, 1)?,
            });
        }
        pending.push((entry.name, command, panes));
    }
    // Keep Lisp name objects until every callback has run, then publish only
    // value-free strings and rows to the rendering thread.
    Ok(pending
        .into_iter()
        .filter_map(|(name, command, panes)| {
            let multiple = panes.len() > 1;
            let mut items = Vec::new();
            for pane in panes {
                let name = menu_name(&pane.name);
                if multiple && !name.is_empty() {
                    items.push(GridMenuItem {
                        label: name,
                        command: None,
                        items: pane.items,
                        separator: false,
                        checked: false,
                        key_hint: None,
                    });
                } else {
                    items.extend(pane.items);
                }
            }
            (!items.is_empty() || command.is_some()).then(|| GridMenu {
                label: menu_name(&name),
                items,
                command,
            })
        })
        .collect())
}

/// Compute the application's native menu on the Lisp thread, outside any
/// borrowed frontend. Rendering only copies the resulting value-free snapshot.
pub fn prepare_native_menus(i: &mut Interp) -> Result<(), Flow> {
    if i.preparing_native_menus {
        return Ok(());
    }
    let Some(frame) = i
        .selected_frame
        .clone()
        .filter(|frame| !frame.borrow().dead && crate::editor::frame_is_graphical(i, frame))
    else {
        i.native_menus.clear();
        return Ok(());
    };
    let buffer = frame.borrow().selected.borrow().buffer;
    if !i.buffer_live(buffer) {
        return Ok(());
    }
    let previous_buffer = i.current_buffer;
    let previous_match = i.match_data.clone();
    let mark = i.specbind_depth();
    i.preparing_native_menus = true;
    i.set_current_buffer(buffer);
    let result = (|| {
        let inhibit = i.q("inhibit-quit");
        i.specbind(inhibit, Value::t())?;
        let debug = i.q("debug-on-next-call");
        i.specbind(debug, Value::Nil)?;
        let updating = i.q("menu-updating-frame");
        i.specbind(updating, Value::Frame(frame))?;
        let flag = i.q("overriding-local-map-menu-flag");
        if !i.symbol_value(flag).truthy() {
            for name in ["overriding-terminal-local-map", "overriding-local-map"] {
                let variable = i.q(name);
                i.specbind(variable, Value::Nil)?;
            }
        }
        compute(i)
    })();
    let restored = i.unbind_to(mark);
    if i.buffer_live(previous_buffer) {
        i.set_current_buffer(previous_buffer);
    }
    i.match_data = previous_match;
    i.preparing_native_menus = false;
    restored?;
    i.native_menus = result?;
    Ok(())
}
