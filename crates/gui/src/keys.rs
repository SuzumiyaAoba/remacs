//! Translate GPUI command keys to Emacs events; OS text uses the input client.
use gpui::{Keystroke, ModifierKey, Modifiers};
use remacs_runtime::{
    editor::{CHAR_ALT, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER},
    lisp::{Interp, Value},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EmacsModifier {
    Control,
    Meta,
    Alt,
    Hyper,
    Super,
}

impl EmacsModifier {
    fn event_bit(self) -> i128 {
        match self {
            EmacsModifier::Control => CHAR_CTL,
            EmacsModifier::Meta => CHAR_META,
            EmacsModifier::Alt => CHAR_ALT,
            EmacsModifier::Hyper => CHAR_HYPER,
            EmacsModifier::Super => CHAR_SUPER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModifierContext {
    Ordinary,
    Function,
    Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModifierSetting {
    ordinary: Option<EmacsModifier>,
    function: Option<EmacsModifier>,
    mouse: Option<EmacsModifier>,
}

impl ModifierSetting {
    const fn all(modifier: Option<EmacsModifier>) -> Self {
        ModifierSetting {
            ordinary: modifier,
            function: modifier,
            mouse: modifier,
        }
    }

    fn for_context(self, context: ModifierContext) -> Option<EmacsModifier> {
        match context {
            ModifierContext::Ordinary => self.ordinary,
            ModifierContext::Function => self.function,
            ModifierContext::Mouse => self.mouse,
        }
    }
}

/// The Lisp-configured mapping from physical macOS keys to Emacs modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModifierMapping {
    left_control: ModifierSetting,
    right_control: ModifierSetting,
    left_alternate: ModifierSetting,
    right_alternate: ModifierSetting,
    left_command: ModifierSetting,
    right_command: ModifierSetting,
    function: ModifierSetting,
}

impl Default for ModifierMapping {
    fn default() -> Self {
        let control = ModifierSetting::all(Some(EmacsModifier::Control));
        let meta = ModifierSetting::all(Some(EmacsModifier::Meta));
        let super_modifier = ModifierSetting::all(Some(EmacsModifier::Super));
        ModifierMapping {
            left_control: control,
            right_control: control,
            left_alternate: meta,
            right_alternate: meta,
            left_command: super_modifier,
            right_command: super_modifier,
            function: ModifierSetting::all(None),
        }
    }
}

impl ModifierMapping {
    pub(super) fn from_interp(i: &Interp) -> Self {
        if !cfg!(target_os = "macos") {
            return ModifierMapping::default();
        }
        let defaults = ModifierMapping::default();
        let left_control = modifier_setting(i, "ns-control-modifier", defaults.left_control);
        let left_alternate = modifier_setting(i, "ns-alternate-modifier", defaults.left_alternate);
        let left_command = modifier_setting(i, "ns-command-modifier", defaults.left_command);
        ModifierMapping {
            left_control,
            right_control: right_modifier_setting(i, "ns-right-control-modifier", left_control),
            left_alternate,
            right_alternate: right_modifier_setting(
                i,
                "ns-right-alternate-modifier",
                left_alternate,
            ),
            left_command,
            right_command: right_modifier_setting(i, "ns-right-command-modifier", left_command),
            function: modifier_setting(i, "ns-function-modifier", defaults.function),
        }
    }
}

fn modifier_setting(i: &Interp, name: &str, fallback: ModifierSetting) -> ModifierSetting {
    let Some(symbol) = i.intern_soft(name).filter(|symbol| i.bound_p(*symbol)) else {
        return fallback;
    };
    modifier_setting_from_value(i, &i.symbol_value(symbol))
}

fn right_modifier_setting(i: &Interp, name: &str, left: ModifierSetting) -> ModifierSetting {
    let Some(symbol) = i.intern_soft(name).filter(|symbol| i.bound_p(*symbol)) else {
        return left;
    };
    let value = i.symbol_value(symbol);
    if matches!(&value, Value::Sym(value) if i.symbol_name(*value).as_str() == "left") {
        left
    } else {
        modifier_setting_from_value(i, &value)
    }
}

fn modifier_setting_from_value(i: &Interp, value: &Value) -> ModifierSetting {
    if let Value::Cons(_) = value {
        return ModifierSetting {
            ordinary: plist_modifier(i, value, ":ordinary"),
            function: plist_modifier(i, value, ":function"),
            mouse: plist_modifier(i, value, ":mouse"),
        };
    }
    ModifierSetting::all(modifier_from_value(i, value))
}

fn plist_modifier(i: &Interp, plist: &Value, property: &str) -> Option<EmacsModifier> {
    let property = i.intern_soft(property)?;
    let entries = plist.list_to_vec().ok()?;
    for pair in entries.chunks(2) {
        if pair.first().and_then(|key| i.sym_id(key)) == Some(property) {
            return pair.get(1).and_then(|value| modifier_from_value(i, value));
        }
    }
    None
}

fn modifier_from_value(i: &Interp, value: &Value) -> Option<EmacsModifier> {
    let Value::Sym(symbol) = value else {
        return None;
    };
    match i.symbol_name(*symbol).as_str() {
        "control" => Some(EmacsModifier::Control),
        "meta" => Some(EmacsModifier::Meta),
        "alt" => Some(EmacsModifier::Alt),
        "hyper" => Some(EmacsModifier::Hyper),
        "super" => Some(EmacsModifier::Super),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PhysicalModifierState {
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    left_command: bool,
    right_command: bool,
}

pub(super) fn modifier_state_after_change(
    mut state: PhysicalModifierState,
    changed: ModifierKey,
    is_down: bool,
) -> PhysicalModifierState {
    let slot = match changed {
        ModifierKey::LeftControl => &mut state.left_control,
        ModifierKey::RightControl => &mut state.right_control,
        ModifierKey::LeftAlt => &mut state.left_alt,
        ModifierKey::RightAlt => &mut state.right_alt,
        ModifierKey::LeftCommand => &mut state.left_command,
        ModifierKey::RightCommand => &mut state.right_command,
    };
    *slot = is_down;
    state
}

fn modifier_context(ks: &Keystroke) -> ModifierContext {
    let key = ks.key.as_str();
    if key.starts_with("kp-")
        || matches!(
            key,
            "up" | "down"
                | "left"
                | "right"
                | "home"
                | "end"
                | "delete"
                | "insert"
                | "undo"
                | "redo"
                | "pageup"
                | "pagedown"
        )
        || (key.starts_with('f') && key[1..].parse::<u32>().is_ok())
    {
        ModifierContext::Function
    } else {
        ModifierContext::Ordinary
    }
}

fn add_modifier(bits: &mut i128, setting: ModifierSetting, context: ModifierContext) -> bool {
    if let Some(modifier) = setting.for_context(context) {
        *bits |= modifier.event_bit();
        true
    } else {
        false
    }
}

fn add_modifier_group(
    bits: &mut i128,
    active: bool,
    left_active: bool,
    right_active: bool,
    left: ModifierSetting,
    right: ModifierSetting,
    context: ModifierContext,
) -> bool {
    if !active {
        return false;
    }
    let mut mapped = false;
    if left_active || right_active {
        if left_active {
            mapped |= add_modifier(bits, left, context);
        }
        if right_active {
            mapped |= add_modifier(bits, right, context);
        }
    } else {
        // Non-macOS platforms only expose the aggregate modifier state.
        mapped |= add_modifier(bits, left, context);
    }
    !mapped
}

fn modifier_bits(
    modifiers: &Modifiers,
    mapping: ModifierMapping,
    physical: PhysicalModifierState,
    context: ModifierContext,
) -> (i128, bool) {
    let mut bits = if modifiers.shift { CHAR_SHIFT } else { 0 };
    let mut has_unmapped = add_modifier_group(
        &mut bits,
        modifiers.control,
        physical.left_control,
        physical.right_control,
        mapping.left_control,
        mapping.right_control,
        context,
    );
    has_unmapped |= add_modifier_group(
        &mut bits,
        modifiers.alt,
        physical.left_alt,
        physical.right_alt,
        mapping.left_alternate,
        mapping.right_alternate,
        context,
    );
    has_unmapped |= add_modifier_group(
        &mut bits,
        modifiers.platform,
        physical.left_command,
        physical.right_command,
        mapping.left_command,
        mapping.right_command,
        context,
    );
    if modifiers.function {
        if !add_modifier(&mut bits, mapping.function, context) {
            has_unmapped = true;
        }
    }
    (bits, has_unmapped)
}

pub(super) fn mouse_modifier_bits(
    modifiers: &Modifiers,
    mapping: ModifierMapping,
    physical: PhysicalModifierState,
) -> i128 {
    modifier_bits(modifiers, mapping, physical, ModifierContext::Mouse).0
}

fn is_named_key(ks: &Keystroke) -> bool {
    ks.key.starts_with("kp-")
        || matches!(
            ks.key.as_str(),
            "return"
                | "enter"
                | "tab"
                | "backspace"
                | "escape"
                | "pageup"
                | "pagedown"
                | "up"
                | "down"
                | "left"
                | "right"
                | "home"
                | "end"
                | "delete"
                | "insert"
                | "undo"
                | "redo"
        )
        || (ks.key.starts_with('f') && ks.key[1..].parse::<u32>().is_ok())
}

pub(super) fn keystroke_to_code(ks: &Keystroke) -> Option<i128> {
    keystroke_to_code_with_mapping(
        ks,
        ModifierMapping::default(),
        PhysicalModifierState::default(),
    )
}

pub(super) fn keystroke_to_code_with_mapping(
    ks: &Keystroke,
    mapping: ModifierMapping,
    physical: PhysicalModifierState,
) -> Option<i128> {
    let (mut mods, _) = modifier_bits(&ks.modifiers, mapping, physical, modifier_context(ks));
    let named = |name: &str| remacs_runtime::editor::event_code_for(name);
    let base = match ks.key.as_str() {
        // Keep special ASCII keys as named events so they remain distinct
        // from the corresponding control characters (C-m, C-i, and C-[).
        "return" | "enter" => named("return"),
        "tab" => named("tab"),
        "backspace" => 127,
        "escape" => named("escape"),
        "space" => 32,
        "pageup" => named("prior"),
        "pagedown" => named("next"),
        "up" | "down" | "left" | "right" | "home" | "end" | "delete" | "insert" | "undo"
        | "redo" => named(ks.key.as_str()),
        s if s.starts_with("kp-") => named(s),
        s if s.len() > 1 && s.starts_with('f') && s[1..].parse::<u32>().is_ok() => named(s),
        s if s.chars().count() == 1 => {
            let physical = s.chars().next().unwrap_or('\0');
            // For modified combos the physical key is the event base;
            // for plain typing prefer the shift/IME-translated char.
            let mut c = if mods
                & (CHAR_CTL | CHAR_META | CHAR_ALT | CHAR_HYPER | CHAR_SUPER | CHAR_SHIFT)
                == 0
            {
                ks.key_char
                    .as_deref()
                    .and_then(|t| t.chars().next())
                    .unwrap_or(physical)
            } else {
                physical
            };
            if c.is_ascii_uppercase() && ks.modifiers.shift {
                mods &= !CHAR_SHIFT;
            } else if c.is_ascii_lowercase()
                && ks.modifiers.shift
                && mods & CHAR_CTL == 0
            {
                c = c.to_ascii_uppercase();
                mods &= !CHAR_SHIFT;
            }
            if mods & CHAR_CTL != 0 && (matches!(c, '@'..='_') || matches!(c, 'a'..='z')) {
                // GNU folds C-@.._ and C-a..z. Other ASCII events
                // (including C-? and C-/, C--, and C-4) keep CHAR_CTL.
                mods &= !CHAR_CTL;
                (c as i128) & 0x1f
            } else {
                c as i128
            }
        }
        _ => match ks.key_char.as_deref().and_then(|t| t.chars().next()) {
            Some(c) => c as i128,
            None => return None,
        },
    };
    Some(base | mods)
}

pub(super) fn command_keystroke(ks: &Keystroke) -> bool {
    command_keystroke_with_mapping(
        ks,
        ModifierMapping::default(),
        PhysicalModifierState::default(),
    )
}

pub(super) fn command_keystroke_with_mapping(
    ks: &Keystroke,
    mapping: ModifierMapping,
    physical: PhysicalModifierState,
) -> bool {
    let (mods, has_unmapped_modifier) =
        modifier_bits(&ks.modifiers, mapping, physical, modifier_context(ks));
    // An unmapped physical modifier contributes no event bit. Named keys still
    // need to reach Emacs as their unmodified event (for example, Command-left
    // with ns-command-modifier set to `none` is still `left`).
    let shifted_character_keeps_modifier = !has_unmapped_modifier
        && mods & CHAR_SHIFT != 0
        && mods & !CHAR_SHIFT == 0
        && keystroke_to_code_with_mapping(ks, mapping, physical)
            .is_some_and(|code| code & CHAR_SHIFT != 0);
    mods & !CHAR_SHIFT != 0 || is_named_key(ks) || shifted_character_keeps_modifier
}

pub(super) fn pass_text_to_os(
    ks: &Keystroke,
    mapping: ModifierMapping,
    physical: PhysicalModifierState,
) -> bool {
    let (_, has_unmapped_modifier) =
        modifier_bits(&ks.modifiers, mapping, physical, modifier_context(ks));
    !has_unmapped_modifier
}

/// Return text from a Unicode key event when the platform bypasses the input
/// client. ASCII remains owned by the OS input handler to avoid duplicates.
pub(super) fn unicode_key_text(ks: &Keystroke) -> Option<&str> {
    let text = ks
        .key_char
        .as_deref()
        .filter(|text| !text.is_empty())
        .unwrap_or(ks.key.as_str());
    let mut chars = text.chars();
    let first = chars.next()?;
    (!first.is_ascii() && chars.all(|ch| !ch.is_ascii())).then_some(text)
}
