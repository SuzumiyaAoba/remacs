use super::{
    GridMenu, GridMenuItem,
    popup::{keymap_item, safe_menu_call},
    render::{menu_keymap_from_definition, menu_separator_name_p},
};
use crate::lisp::{Interp, Value, error::Flow};

fn menu_key(i: &mut Interp, key: &Value) -> Value {
    if let Value::Int(code) = key
        && let Some(name) = crate::editor::key_name_for(*code)
    {
        return Value::Sym(i.intern(&name));
    }
    key.clone()
}

fn command_binding(
    i: &mut Interp,
    definition: &Value,
    enabled: bool,
    keys: &[Value],
) -> (Option<String>, Option<super::NativeMenuCommandId>) {
    if !enabled || definition.is_nil() || crate::editor::is_keymap(i, definition) {
        return (None, None);
    }
    let name = if let Value::Sym(symbol) = definition {
        let name = i.symbol_name(*symbol);
        if i.obarray.intern_soft(&name) == Some(*symbol) {
            Some(name)
        } else {
            None
        }
    } else {
        None
    };
    (
        name,
        Some(i.native_menu_commands.register(definition, keys)),
    )
}

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

fn menu_items(
    i: &mut Interp,
    map: &Value,
    depth: usize,
    prefix: &[Value],
) -> Result<Vec<GridMenuItem>, Flow> {
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
        let mut keys = prefix.to_vec();
        keys.push(menu_key(i, &key));
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
                help: None,
                command: None,
                command_id: None,
                items: Vec::new(),
                separator: true,
                checked: false,
                key_hint: None,
            });
            continue;
        }
        if let Some(map) = submenu {
            let children = if item.enabled {
                menu_items(i, &map, depth + 1, &keys)?
            } else {
                Vec::new()
            };
            items.push(GridMenuItem {
                label,
                help: item.help_text(),
                command: None,
                command_id: None,
                items: children,
                separator: false,
                checked: false,
                key_hint,
            });
        } else {
            let (command, command_id) = command_binding(i, &definition, item.enabled, &keys);
            items.push(GridMenuItem {
                label,
                help: item.help_text(),
                command,
                command_id,
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
    // GNU runs activation and update hooks before collecting menu maps.
    // Each callback uses safe_run_hooks error handling, like command hooks.
    for hook in ["activate-menubar-hook", "menu-bar-update-hook"] {
        crate::lisp::builtins::evalfn::safe_call_hook(i, hook)?;
    }
    let mut pending = Vec::new();
    let menu_bar = Value::Sym(i.q("menu-bar"));
    for entry in collect_menu_bar(i)? {
        let keys = vec![menu_bar.clone(), menu_key(i, &entry.key)];
        let (command, command_id) = entry
            .definitions
            .first()
            .map(|definition| command_binding(i, definition, true, &keys))
            .unwrap_or_default();
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
                items: menu_items(i, &map, 1, &keys)?,
            });
        }
        pending.push((entry.name, command, command_id, panes));
    }
    // Keep Lisp name objects until every callback has run, then publish only
    // a value-free snapshot to the rendering thread.
    Ok(pending
        .into_iter()
        .filter_map(|(name, command, command_id, panes)| {
            let multiple = panes.len() > 1;
            let mut items = Vec::new();
            for pane in panes {
                let name = menu_name(&pane.name);
                if multiple && !name.is_empty() {
                    items.push(GridMenuItem {
                        label: name,
                        help: None,
                        command: None,
                        command_id: None,
                        items: pane.items,
                        separator: false,
                        checked: false,
                        key_hint: None,
                    });
                } else {
                    items.extend(pane.items);
                }
            }
            (!items.is_empty() || command.is_some() || command_id.is_some()).then(|| GridMenu {
                label: menu_name(&name),
                items,
                command,
                command_id,
            })
        })
        .collect())
}

fn prepare_menu_bar(i: &mut Interp) -> Result<(), Flow> {
    i.native_menu_commands.prune();
    let Some(frame) = i
        .selected_frame
        .clone()
        .filter(|frame| !frame.borrow().dead && crate::editor::frame_is_graphical(i, frame))
    else {
        i.native_menus.clear();
        i.native_menu_commands.prune();
        return Ok(());
    };
    let buffer = frame.borrow().selected.borrow().buffer;
    if !i.buffer_live(buffer) {
        return Ok(());
    }
    let previous_buffer = i.current_buffer;
    let previous_match = i.match_data.clone();
    let mark = i.specbind_depth();
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
    restored?;
    i.native_menus = result?;
    i.native_menu_commands.prune();
    Ok(())
}

fn tool_bar_items(
    i: &mut Interp,
    frame: &crate::editor::FrameRef,
) -> Result<Vec<GridMenuItem>, Flow> {
    let map_symbol = i.q("tool-bar-map");
    let primary_map = i.symbol_value(map_symbol);
    let secondary_map = i
        .q_soft("secondary-tool-bar-map")
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    let position = crate::lisp::eval::plist_get_v(
        &frame.borrow().params,
        &Value::Sym(i.q("tool-bar-position")),
    );
    let at_bottom = matches!(&position, Value::Sym(symbol) if i.symbol_name(*symbol) == "bottom");
    let at_top = position.is_nil()
        || matches!(&position, Value::Sym(symbol) if i.symbol_name(*symbol) == "top");
    let secondary_visible = secondary_map.truthy() && (at_top || at_bottom);
    // GNU's `tool-bar-make-keymap' joins the two maps with a row break;
    // retain that ordering in the immutable snapshot prepared for rendering.
    let primary_bindings = crate::editor::keymap_own_bindings(i, &primary_map);
    let mut bindings = Vec::new();
    let mut wrap_at = None;
    if secondary_visible && at_bottom {
        bindings.extend(
            crate::editor::keymap_own_bindings(i, &secondary_map)
                .into_iter()
                .map(|(key, binding)| (key, binding, true)),
        );
        wrap_at = Some(bindings.len());
        bindings.extend(
            primary_bindings
                .into_iter()
                .map(|(key, binding)| (key, binding, false)),
        );
    } else {
        bindings.extend(
            primary_bindings
                .into_iter()
                .map(|(key, binding)| (key, binding, false)),
        );
        if secondary_visible {
            wrap_at = Some(bindings.len());
            bindings.extend(
                crate::editor::keymap_own_bindings(i, &secondary_map)
                    .into_iter()
                    .map(|(key, binding)| (key, binding, true)),
            );
        }
    }
    let mut items = Vec::new();
    let mut wrap_pending = false;
    for (index, (key, binding, is_secondary)) in bindings.into_iter().enumerate() {
        if wrap_at == Some(index) {
            wrap_pending = true;
        }
        let Some(item) = keymap_item(i, &key, &binding)? else {
            continue;
        };
        let definition = crate::editor::keymap_def(i, item.definition.clone())?;
        if !item.separator
            && (definition.is_nil()
                || menu_keymap_from_definition(i, &definition, 0).is_some()
                || matches!(definition, Value::Sym(symbol) if !i.fbound_p(symbol)))
        {
            continue;
        }
        let mut label = item.label.clone();
        if let Ok(parts) = binding.list_to_vec()
            && matches!(parts.first(), Some(Value::Sym(symbol)) if i.symbol_name(*symbol) == "menu-item")
        {
            let properties = Value::list(parts.into_iter().skip(3).collect());
            let property = i.q(":label");
            if let Value::Str(text) = crate::lisp::eval::plist_get(&properties, property) {
                label = text.borrow().to_unicode_lossy();
            }
        }
        let prefix = Value::Sym(i.q("tool-bar"));
        let key = menu_key(i, &key);
        // Keep stable key objects for registry identity. Dispatch materializes
        // GNU's parameterized toolbar event using the originating frame.
        let keys = vec![prefix, key, Value::Frame(frame.clone())];
        let toolbar_definition = if is_secondary {
            modifier_bar_toolbar_command(i, &definition).unwrap_or_else(|| definition.clone())
        } else {
            definition.clone()
        };
        let (command, command_id) = command_binding(
            i,
            &toolbar_definition,
            item.enabled && !item.separator,
            &keys,
        );
        if wrap_pending {
            if items.iter().any(|item: &GridMenuItem| !item.separator) {
                items.push(GridMenuItem {
                    label: String::new(),
                    help: None,
                    command: None,
                    command_id: None,
                    items: Vec::new(),
                    separator: true,
                    checked: false,
                    key_hint: None,
                });
            }
            wrap_pending = false;
        }
        items.push(GridMenuItem {
            label,
            help: item.help_text(),
            command,
            command_id,
            items: Vec::new(),
            separator: item.separator,
            checked: false,
            key_hint: None,
        });
    }
    Ok(items)
}

/// The modifier toolbar keymap's commands are unused by GNU; input decoding
/// calls these wrappers so multiple modifier buttons can be combined.
fn modifier_bar_toolbar_command(i: &Interp, definition: &Value) -> Option<Value> {
    let Value::Sym(symbol) = definition else {
        return None;
    };
    let name = i.symbol_name(*symbol);
    let modifier = name
        .strip_prefix("event-apply-")?
        .strip_suffix("-modifier")?;
    let wrapper_name = format!("tool-bar-event-apply-{modifier}-modifier");
    let wrapper = i.intern_soft(&wrapper_name)?;
    i.fbound_p(wrapper).then_some(Value::Sym(wrapper))
}

fn prepare_tool_bars(i: &mut Interp) -> Result<Vec<(usize, Vec<GridMenuItem>)>, Flow> {
    let mut tool_bars = Vec::new();
    for frame in i.frames.clone() {
        if frame.borrow().dead
            || !crate::editor::frame_is_graphical(i, &frame)
            || crate::editor::frame_parameter_positive_size(i, &frame, "tool-bar-lines", 0) == 0
        {
            continue;
        }
        let window = frame.borrow().selected.clone();
        if !i.buffer_live(window.borrow().buffer) {
            continue;
        }
        let items = super::mode_line::with_window(i, &window, |i| {
            let mark = i.specbind_depth();
            let updating = i.q("menu-updating-frame");
            let result = i
                .specbind(updating, Value::Frame(frame.clone()))
                .and_then(|_| tool_bar_items(i, &frame));
            let unwind = i.unbind_to(mark);
            let items = result?;
            unwind?;
            Ok(items)
        })?;
        tool_bars.push((frame.borrow().id, items));
    }
    Ok(tool_bars)
}

/// Prepare value-free menu and toolbar snapshots on the Lisp thread. Command
/// objects stay in the runtime registry while any snapshot/input owns its ID.
pub fn prepare_native_menus(i: &mut Interp) -> Result<(), Flow> {
    if i.preparing_native_menus {
        return Ok(());
    }
    i.preparing_native_menus = true;
    let result = prepare_menu_bar(i).and_then(|_| prepare_tool_bars(i));
    let result = result.map(|tool_bars| i.native_menu_commands.set_tool_bars(tool_bars));
    i.preparing_native_menus = false;
    i.native_menu_commands.prune();
    result
}
