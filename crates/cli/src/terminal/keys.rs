//! Translate crossterm key events into Emacs character and named-key codes.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use remacs_runtime::editor::{self, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER};

/// Translate a crossterm key event to an Emacs event code.
pub(super) fn key_event_to_code(k: KeyEvent) -> Option<i128> {
    let mut mods = 0i128;
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        mods |= CHAR_CTL;
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        mods |= CHAR_META;
    }
    if k.modifiers.contains(KeyModifiers::SHIFT) {
        // Shift on printable chars arrives pre-shifted in `code`.
        mods |= CHAR_SHIFT;
    }
    if k.modifiers.contains(KeyModifiers::SUPER) {
        mods |= CHAR_SUPER;
    }
    if k.modifiers.contains(KeyModifiers::HYPER) {
        mods |= CHAR_HYPER;
    }
    // ALT is sometimes delivered as META; treat as M-.
    let base = match k.code {
        KeyCode::Char(c) => {
            if c.is_ascii_uppercase() && k.modifiers.contains(KeyModifiers::SHIFT) {
                mods &= !CHAR_SHIFT; // shift folded into the char
            }
            if mods & CHAR_CTL != 0 && (c as i128) < 128 {
                // Emacs folds C-<ascii> to the control char (C-u → 21).
                mods &= !CHAR_CTL;
                match c {
                    // crossterm 0.29 represents raw 0x1c..0x1f as C-4..C-7.
                    '4'..='7' => 0x1c + (c as i128 - '4' as i128),
                    '?' => 127,
                    _ => (c as i128) & 0x1f,
                }
            } else {
                c as i128
            }
        }
        KeyCode::Enter => b'\r' as i128,
        KeyCode::Tab => b'\t' as i128,
        KeyCode::Backspace => 127,
        KeyCode::Esc => 27,
        KeyCode::Up => named_code("up"),
        KeyCode::Down => named_code("down"),
        KeyCode::Left => named_code("left"),
        KeyCode::Right => named_code("right"),
        KeyCode::Home => named_code("home"),
        KeyCode::End => named_code("end"),
        KeyCode::PageUp => named_code("prior"),
        KeyCode::PageDown => named_code("next"),
        KeyCode::Delete => named_code("delete"),
        KeyCode::Insert => named_code("insert"),
        KeyCode::F(n) => named_code(&format!("f{}", n)),
        _ => return None,
    };
    Some(base | mods)
}

fn named_code(name: &str) -> i128 {
    editor::event_code_for(name)
}
