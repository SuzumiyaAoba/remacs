//! Translate GPUI command keys to Emacs events; OS text uses the input client.
use gpui::Keystroke;
use remacs_runtime::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};

pub(super) fn keystroke_to_code(ks: &Keystroke) -> Option<i128> {
    let mut mods = 0i128;
    if ks.modifiers.control {
        mods |= CHAR_CTL;
    }
    if ks.modifiers.alt {
        mods |= CHAR_META;
    }
    if ks.modifiers.platform {
        // Cmd on macOS == Super, as GNU Emacs on macOS does.
        mods |= CHAR_SUPER;
    }
    if ks.modifiers.shift {
        mods |= CHAR_SHIFT;
    }
    let named = |name: &str| remacs_runtime::editor::event_code_for(name);
    let base = match ks.key.as_str() {
        "return" | "enter" => b'\r' as i128,
        "tab" => b'\t' as i128,
        "backspace" => 127,
        "escape" => 27,
        "space" => 32,
        "pageup" => named("prior"),
        "pagedown" => named("next"),
        "up" | "down" | "left" | "right" | "home" | "end" | "delete" | "insert" => {
            named(ks.key.as_str())
        }
        s if s.len() > 1 && s.starts_with('f') && s[1..].parse::<u32>().is_ok() => named(s),
        s if s.chars().count() == 1 => {
            let physical = s.chars().next().unwrap_or('\0');
            // For modified combos the physical key is the event base;
            // for plain typing prefer the shift/IME-translated char.
            let c = if mods & (CHAR_CTL | CHAR_META | CHAR_SUPER) == 0 {
                ks.key_char
                    .as_deref()
                    .and_then(|t| t.chars().next())
                    .unwrap_or(physical)
            } else {
                physical
            };
            if c.is_ascii_uppercase() && ks.modifiers.shift {
                mods &= !CHAR_SHIFT;
            }
            if mods & CHAR_CTL != 0 && (c as i128) < 128 {
                // Emacs folds C-<ascii> to the control char (C-u → 21).
                mods &= !CHAR_CTL;
                if c == '?' { 127 } else { (c as i128) & 0x1f }
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
    ks.modifiers.control
        || ks.modifiers.alt
        || ks.modifiers.platform
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
        )
        || (ks.key.starts_with('f') && ks.key[1..].parse::<u32>().is_ok())
}
