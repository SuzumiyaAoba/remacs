use super::{GridMenu, GridMenuItem, popup::keymap_item, render::menu_keymap_from_definition};
use crate::lisp::{Interp, Value, error::Flow};

fn menu_items(i: &mut Interp, map: &Value, depth: usize) -> Result<Vec<GridMenuItem>, Flow> {
    if depth >= 10 {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for (key, binding) in crate::editor::keymap_own_bindings(i, map) {
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
        if item.separator {
            items.push(GridMenuItem {
                label: item.label,
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
                label: item.label,
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
                label: item.label,
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

fn compute(i: &mut Interp) -> Result<Vec<GridMenu>, Flow> {
    let mode = i.q("menu-bar-mode");
    if !i.symbol_value(mode).truthy() {
        return Ok(Vec::new());
    }
    let global = i.q("global-map");
    let map = i.symbol_value(global);
    let menu_bar_code = crate::editor::event_code_for("menu-bar");
    let definition = crate::editor::keymap_own_bindings(i, &map)
        .into_iter()
        .find_map(|(key, value)| {
            let matches = match key {
                Value::Int(code) => code == menu_bar_code,
                Value::Sym(symbol) => i.symbol_name(symbol) == "menu-bar",
                _ => false,
            };
            matches.then_some(value)
        });
    let Some(definition) = definition else {
        return Ok(Vec::new());
    };
    let definition = crate::editor::keymap_def(i, definition)?;
    let Some(map) = menu_keymap_from_definition(i, &definition, 0) else {
        return Ok(Vec::new());
    };
    Ok(menu_items(i, &map, 0)?
        .into_iter()
        .filter(|item| !item.separator && !item.items.is_empty())
        .map(|item| GridMenu {
            label: item.label,
            items: item.items,
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
        let updating = i.q("menu-updating-frame");
        i.specbind(updating, Value::Frame(frame))?;
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
