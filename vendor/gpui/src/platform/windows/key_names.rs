use crate::{KeyDownEvent, Keystroke, Modifiers, PlatformInput};
use smallvec::{SmallVec, smallvec};

// Win32 virtual-key values are fixed independently of the active layout.
// https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
fn immutable_name(vkey: u16) -> Option<&'static str> {
    Some(match vkey {
        0x20 => "space",     // VK_SPACE
        0x08 => "backspace", // VK_BACK
        0x0d => "enter",     // VK_RETURN
        0x13 => "pause",     // VK_PAUSE
        0x09 => "tab",       // VK_TAB
        0x26 => "up",        // VK_UP
        0x28 => "down",      // VK_DOWN
        0x27 => "right",     // VK_RIGHT
        0x25 => "left",      // VK_LEFT
        0x24 => "home",      // VK_HOME
        0x23 => "end",       // VK_END
        0x21 => "pageup",    // VK_PRIOR
        0x22 => "pagedown",  // VK_NEXT
        0xa6 => "back",      // VK_BROWSER_BACK
        0xa7 => "forward",   // VK_BROWSER_FORWARD
        0x1b => "escape",    // VK_ESCAPE
        0x2b => "execute",   // VK_EXECUTE
        0x2c => "print",     // VK_SNAPSHOT
        0x2d => "insert",    // VK_INSERT
        0x2e => "delete",    // VK_DELETE
        0x2f => "help",      // VK_HELP
        0x5d => "menu",      // VK_APPS
        0x70 => "f1",        // VK_F1
        0x71 => "f2",        // VK_F2
        0x72 => "f3",        // VK_F3
        0x73 => "f4",        // VK_F4
        0x74 => "f5",        // VK_F5
        0x75 => "f6",        // VK_F6
        0x76 => "f7",        // VK_F7
        0x77 => "f8",        // VK_F8
        0x78 => "f9",        // VK_F9
        0x79 => "f10",       // VK_F10
        0x7a => "f11",       // VK_F11
        0x7b => "f12",       // VK_F12
        0x7c => "f13",       // VK_F13
        0x7d => "f14",       // VK_F14
        0x7e => "f15",       // VK_F15
        0x7f => "f16",       // VK_F16
        0x80 => "f17",       // VK_F17
        0x81 => "f18",       // VK_F18
        0x82 => "f19",       // VK_F19
        0x83 => "f20",       // VK_F20
        0x84 => "f21",       // VK_F21
        0x85 => "f22",       // VK_F22
        0x86 => "f23",       // VK_F23
        0x87 => "f24",       // VK_F24
        _ => return None,
    })
}

pub(crate) fn immutable_keystroke(vkey: u16, modifiers: Modifiers) -> Option<Keystroke> {
    Some(Keystroke {
        key: immutable_name(vkey)?.to_owned(),
        key_char: None,
        modifiers,
    })
}

/// Print Screen can arrive as key-up only. Preserve ordinary pairs while
/// synthesizing the missing press for consumers that listen to key-down.
#[derive(Default)]
pub(crate) struct KeyEventState {
    snapshot_pressed: bool,
}

impl KeyEventState {
    pub(crate) fn normalize(
        &mut self,
        vkey: u16,
        input: PlatformInput,
    ) -> SmallVec<[PlatformInput; 2]> {
        if vkey == 0x2c {
            // VK_SNAPSHOT
            match &input {
                PlatformInput::KeyDown(_) => self.snapshot_pressed = true,
                PlatformInput::KeyUp(event) => {
                    if !std::mem::take(&mut self.snapshot_pressed) {
                        let press = KeyDownEvent {
                            keystroke: event.keystroke.clone(),
                            is_held: false,
                        };
                        return smallvec![PlatformInput::KeyDown(press), input];
                    }
                }
                _ => {}
            }
        }
        smallvec![input]
    }

    pub(crate) fn reset(&mut self) {
        self.snapshot_pressed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_named_virtual_key(code: u16, name: &str) {
        for modified in [false, true] {
            let modifiers = Modifiers {
                control: modified,
                alt: modified,
                shift: modified,
                platform: modified,
                function: false,
            };
            let key = immutable_keystroke(code, modifiers).expect(name);
            assert_eq!(key.key, name);
            assert!(key.key_char.is_none());
            assert_eq!(key.modifiers, modifiers);
        }
    }

    #[test]
    fn help_virtual_key_is_named_non_text_and_preserves_modifiers() {
        assert_named_virtual_key(0x2f, "help");
    }
    #[test]
    fn execute_virtual_key_is_named_non_text_and_preserves_modifiers() {
        assert_named_virtual_key(0x2b, "execute");
    }
    #[test]
    fn pause_virtual_key_is_named_non_text_and_preserves_modifiers() {
        assert_named_virtual_key(0x13, "pause");
    }
    #[test]
    fn print_screen_virtual_key_is_named_non_text_and_preserves_modifiers() {
        assert_named_virtual_key(0x2c, "print");
    }

    #[test]
    fn existing_immutable_names_and_layout_fallback_are_preserved() {
        let cases = [
            (0x20, "space"),
            (0x08, "backspace"),
            (0x0d, "enter"),
            (0x09, "tab"),
            (0x26, "up"),
            (0x28, "down"),
            (0x27, "right"),
            (0x25, "left"),
            (0x24, "home"),
            (0x23, "end"),
            (0x21, "pageup"),
            (0x22, "pagedown"),
            (0xa6, "back"),
            (0xa7, "forward"),
            (0x1b, "escape"),
            (0x2d, "insert"),
            (0x2e, "delete"),
            (0x5d, "menu"),
        ];
        for (code, expected) in cases {
            assert_eq!(
                immutable_keystroke(code, Modifiers::none()).unwrap().key,
                expected
            );
        }
        for n in 1..=24 {
            assert_eq!(
                immutable_keystroke(0x6f + n, Modifiers::none())
                    .unwrap()
                    .key,
                format!("f{n}")
            );
        }
        for code in [0x41, 0x30, 0x70 - 1, 0x88] {
            assert!(immutable_keystroke(code, Modifiers::none()).is_none());
        }
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use crate::KeyUpEvent;

    fn key(name: &str) -> Keystroke {
        Keystroke {
            key: name.into(),
            key_char: None,
            modifiers: Modifiers {
                control: true,
                alt: true,
                shift: true,
                platform: true,
                function: false,
            },
        }
    }
    fn down(name: &str) -> PlatformInput {
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: key(name),
            is_held: false,
        })
    }
    fn up(name: &str) -> PlatformInput {
        PlatformInput::KeyUp(KeyUpEvent {
            keystroke: key(name),
        })
    }

    #[test]
    fn snapshot_release_only_emits_one_press_then_release_with_modifiers() {
        let mut state = KeyEventState::default();
        for _ in 0..2 {
            let events = state.normalize(0x2c, up("print"));
            assert_eq!(events.len(), 2);
            let PlatformInput::KeyDown(press) = &events[0] else {
                panic!("missing synthetic press")
            };
            assert_eq!(press.keystroke, key("print"));
            assert!(!press.is_held);
            assert!(
                matches!(&events[1], PlatformInput::KeyUp(release) if release.keystroke == key("print"))
            );
        }
    }
    #[test]
    fn snapshot_with_a_real_press_does_not_emit_a_duplicate() {
        let mut state = KeyEventState::default();
        let press = state.normalize(0x2c, down("print"));
        assert_eq!(press.len(), 1);
        assert!(matches!(&press[0], PlatformInput::KeyDown(_)));
        let release = state.normalize(0x2c, up("print"));
        assert_eq!(release.len(), 1);
        assert!(matches!(&release[0], PlatformInput::KeyUp(_)));
    }
    #[test]
    fn other_keys_stay_single_and_focus_reset_drops_the_old_press() {
        let mut state = KeyEventState::default();
        assert_eq!(state.normalize(0x13, down("pause")).len(), 1);
        let release = state.normalize(0x13, up("pause"));
        assert_eq!(release.len(), 1);
        assert!(matches!(&release[0], PlatformInput::KeyUp(_)));
        state.normalize(0x2c, down("print"));
        assert_eq!(state.normalize(0x41, down("a")).len(), 1);
        state.reset();
        assert_eq!(state.normalize(0x2c, up("print")).len(), 2);
    }
}
