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
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

mod navigation;
pub(crate) use navigation::PopupMenuAction;
pub use navigation::PopupMenuNavigation;

/// One row of a popup snapshot. Values remain on the Lisp logic thread.
#[derive(Clone)]
pub struct PopupMenuEntry {
    pub label: String,
    pub command: Option<String>,
    pub separator: bool,
    pub submenu: bool,
    pub depth: usize,
    pub value: Option<Value>,
}

/// Choices and view navigation for one synchronous popup request.
pub struct PopupMenu {
    pub id: usize,
    pub position: PopupMenuPosition,
    pub entries: Vec<PopupMenuEntry>,
    pub navigation: RefCell<PopupMenuNavigation>,
    /// First entry in the current scroll viewport.
    pub scroll_offset: Cell<usize>,
    /// Entry rows available in the most recently rendered viewport.
    pub visible_rows: Cell<usize>,
}

impl PopupMenu {
    /// Scroll the menu while keeping its final page filled when possible.
    pub fn scroll_by(&self, rows: isize) -> bool {
        self.scroll_column(None, rows)
    }
}

impl PopupMenuEntry {
    fn text(label: String, depth: usize) -> Self {
        Self {
            label,
            command: None,
            separator: false,
            submenu: false,
            depth,
            value: None,
        }
    }
    fn separator() -> Self {
        Self::separator_at(0)
    }
    fn separator_at(depth: usize) -> Self {
        Self {
            separator: true,
            ..Self::text(String::new(), depth)
        }
    }
}

/// GNU keyboard.c menu_item_eval_property: dynamic eval with redisplay
/// inhibited, caught conditions becoming nil, and quit still propagating.
fn eval_property(i: &mut Interp, form: &Value) -> EvalResult {
    protected_menu_eval(i, None, |i| {
        crate::lisp::builtins::evalfn::f_eval(i, vec![form.clone(), Value::Nil].into())
    })
}

/// GNU safe_funcall: errors and quits are logged without opening the debugger.
pub(super) fn safe_menu_call(i: &mut Interp, function: Value, args: Vec<Value>) -> EvalResult {
    protected_menu_eval(i, Some((function.clone(), args.clone())), |i| {
        i.apply(&function, args)
    })
}

fn protected_menu_eval(
    i: &mut Interp,
    safe_call: Option<(Value, Vec<Value>)>,
    evaluate: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    let mark = i.specbind_depth();
    let inhibit = i.q("inhibit-redisplay");
    if let Err(flow) = i.specbind(inhibit, Value::t()) {
        let _ = i.unbind_to(mark);
        return Err(flow);
    }
    let depth = i.case_handlers.len();
    i.case_handlers.push(CaseHandler::single(Value::t()));
    let result = evaluate(i);
    let result = i.offer_flow(result);
    i.case_handlers.truncate(depth);
    let result = match result {
        Err(Flow::Signal(signal)) if signal.claims_frame(depth) => {
            let quit = signal.is_symbol(sym::QUIT);
            i.finish_quit_unwind();
            if let Some((function, args)) = &safe_call {
                let call = Value::list(
                    std::iter::once(function.clone())
                        .chain(args.iter().cloned())
                        .collect(),
                );
                let log = format!(
                    "Error muted by safe_call: {} signaled {}",
                    i.prin1_to_string(&call),
                    i.prin1_to_string(&signal.condition)
                );
                i.message_log(&log);
                Ok(Value::Nil)
            } else if quit {
                Err(Flow::Quit)
            } else {
                Ok(Value::Nil)
            }
        }
        Err(Flow::Quit) if safe_call.is_some() => {
            i.finish_quit_unwind();
            i.message_log("Error muted by safe_call: menu computation signaled quit");
            Ok(Value::Nil)
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
    help: Value,
    enable: Value,
    /// Retain the value cell, since a dynamic name can replace its function.
    filter: Option<Value>,
    selected: Value,
    keys: Value,
    key_sequence: Option<Value>,
}

/// Check :visible in source order and retain the final property fields.
fn menu_properties(i: &mut Interp, mut properties: Value) -> Result<Option<MenuProperties>, Flow> {
    let help_property = i.q(":help");
    let enable = i.q(":enable");
    let visible = i.q(":visible");
    let filter = i.q(":filter");
    let button = i.q(":button");
    let toggle = i.q(":toggle");
    let radio = i.q(":radio");
    let keys_property = i.q(":keys");
    let sequence_property = i.q(":key-sequence");
    let force = i.q("enable-disabled-menus-and-buttons");
    let mut enable_form = Value::t();
    let mut help = Value::Nil;
    let mut filter_cell = None;
    let mut selected = Value::Nil;
    let mut keys = Value::Nil;
    let mut key_sequence = None;
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
        } else if matches!(property, Value::Sym(symbol) if symbol == help_property) {
            help = substitute_help(i, form)?;
        } else if matches!(property, Value::Sym(symbol) if symbol == filter) {
            filter_cell = Some(Value::Cons(value.clone()));
        } else if matches!(property, Value::Sym(symbol) if symbol == keys_property) {
            if crate::lisp::builtins::data::functionp_checked(i, &form)? {
                // Unlike evaluated menu forms, GNU calls this callback directly:
                // errors propagate, and no inhibit-redisplay binding is added.
                keys = i.apply(&form, Vec::new())?;
            } else if matches!(form, Value::Cons(_) | Value::Str(_)) {
                keys = form;
            }
        } else if matches!(property, Value::Sym(symbol) if symbol == sequence_property)
            && matches!(
                form,
                Value::Nil | Value::Sym(_) | Value::Str(_) | Value::Vec(_)
            )
        {
            // A dynamic label or filter may replace this cell's value later.
            key_sequence = Some(Value::Cons(value.clone()));
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
        help,
        enable: enable_form,
        filter: filter_cell,
        selected,
        keys,
        key_sequence,
    }))
}

/// GNU keyboard.c substitutes constant help strings while walking the plist.
/// Forms/functions are retained but never evaluated as menu help.
fn substitute_help(i: &mut Interp, help: Value) -> Result<Value, Flow> {
    let Value::Str(text) = &help else {
        return Ok(help);
    };
    let nonempty = !text.borrow().is_empty();
    let property = i.q("help-echo-inhibit-substitution");
    if nonempty
        && crate::buffer::primitives::f_get_text_property(
            i,
            vec![Value::Int(0), Value::Sym(property), help.clone()].into(),
        )?
        .truthy()
    {
        return Ok(help);
    }
    let substitute = Value::Sym(i.q("substitute-command-keys"));
    i.apply(&substitute, vec![help])
}

pub(super) struct ParsedMenuItem {
    help: Value,
    pub(super) label: String,
    pub(super) name: Value,
    pub(super) definition: Value,
    pub(super) separator: bool,
    pub(super) enabled: bool,
    selected: Value,
    keys: Value,
    key_sequence: Option<Value>,
}

impl ParsedMenuItem {
    pub(super) fn help_text(&self) -> Option<String> {
        match &self.help {
            Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
            _ => None,
        }
    }

    /// Resolve the keyboard equivalent after name/filter/enable processing.
    /// Submenus and nonselectable text retain only an explicit string hint.
    pub(super) fn keyboard_equivalent(
        &self,
        i: &mut Interp,
        leaf_command: bool,
    ) -> Result<Option<String>, Flow> {
        let result = if !leaf_command {
            self.keys.clone()
        } else if matches!(self.keys, Value::Str(_)) && self.key_sequence.is_none() {
            let substitute = Value::Sym(i.q("substitute-command-keys"));
            let text = i.apply(&substitute, vec![self.keys.clone()])?;
            // concat checks a redefined substitute-command-keys result just as
            // GNU does. Leading padding is supplied by the native column layout.
            crate::lisp::builtins::strfn::f_concat(i, vec![Value::string(""), text].into())?
        } else {
            let (definition, prefix) = if let Value::Cons(cell) = &self.keys {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            } else {
                (self.definition.clone(), self.keys.clone())
            };
            let mut keys = if let Some(Value::Cons(cell)) = &self.key_sequence {
                cell.borrow().car.clone()
            } else {
                Value::Nil
            };
            if !keys.is_nil() {
                let binding = crate::editor::f_key_binding(
                    i,
                    vec![keys.clone(), Value::Nil, Value::Nil, Value::Nil].into(),
                )?;
                let same = crate::lisp::builtins::eq_values_with_interp;
                let alias = match &definition {
                    Value::Sym(symbol) => same(i, &binding, &i.symbol_function(*symbol)),
                    _ => false,
                };
                if binding.is_nil() || !(same(i, &binding, &definition) || alias) {
                    keys = Value::Nil;
                }
            }
            if keys.is_nil() {
                keys = crate::editor::f_where_is_internal(
                    i,
                    vec![definition, Value::Nil, Value::t(), Value::Nil, Value::Nil].into(),
                )?;
            }
            if keys.is_nil() {
                return Ok(None);
            }
            let mut text = crate::editor::f_key_description(i, vec![keys, Value::Nil].into())?;
            if let Value::Cons(prefix) = prefix {
                let (before, after) = {
                    let prefix = prefix.borrow();
                    (prefix.car.clone(), prefix.cdr.clone())
                };
                if matches!(before, Value::Str(_)) {
                    text = crate::lisp::builtins::strfn::f_concat(i, vec![before, text].into())?;
                }
                if matches!(after, Value::Str(_)) {
                    text = crate::lisp::builtins::strfn::f_concat(i, vec![text, after].into())?;
                }
            }
            text
        };
        Ok(match result {
            Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
            _ => None,
        })
    }

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
                help: properties.help,
                label,
                name: Value::Str(name),
                definition,
                separator,
                enabled,
                selected: properties.selected,
                keys: properties.keys,
                key_sequence: properties.key_sequence,
            }));
        }
    }
    let help = if let Value::Cons(cell) = binding {
        let cell = cell.borrow();
        if matches!(cell.car, Value::Str(_)) {
            if let Value::Cons(tail) = &cell.cdr {
                let value = tail.borrow().car.clone();
                if matches!(value, Value::Str(_)) {
                    value
                } else {
                    Value::Nil
                }
            } else {
                Value::Nil
            }
        } else {
            Value::Nil
        }
    } else {
        Value::Nil
    };
    let help = substitute_help(i, help)?;
    let (label, definition, separator) = menu_label_and_definition(i, key, binding);
    let name = match binding {
        Value::Cons(cell) if matches!(cell.borrow().car, Value::Str(_)) => {
            cell.borrow().car.clone()
        }
        _ => Value::string(label.clone()),
    };
    Ok(Some(ParsedMenuItem {
        help,
        label,
        name,
        definition,
        separator,
        enabled: true,
        selected: Value::Nil,
        keys: Value::Nil,
        key_sequence: None,
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
        }) = keymap_item(i, &key, &binding)?
        else {
            continue;
        };
        if separator {
            entries.push(PopupMenuEntry::separator_at(depth));
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
                let mut parent = PopupMenuEntry::text(label, depth);
                parent.submenu = true;
                entries.push(parent);
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
                submenu: false,
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
                                submenu: false,
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
        navigation: RefCell::new(PopupMenuNavigation::default()),
        scroll_offset: Cell::new(0),
        visible_rows: Cell::new(0),
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
