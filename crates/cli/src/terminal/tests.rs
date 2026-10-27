use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use remacs_runtime::editor::{CHAR_HYPER, CHAR_META, CHAR_SUPER, event_code_for as named_code};
use remacs_runtime::frontend::dispatch_key_core;
use remacs_runtime::{editor, lisp};
use std::cell::RefCell;
use std::rc::Rc;
/// A `Write` sink over a shared buffer so tests can inspect what
/// `render`/`draw_echo` emitted.
struct SharedBuf(Rc<RefCell<Vec<u8>>>);
impl io::Write for SharedBuf {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn test_term(w: usize, h: usize) -> (Terminal, Rc<RefCell<Vec<u8>>>) {
    let buf = Rc::new(RefCell::new(Vec::new()));
    (
        Terminal {
            width: w,
            height: h,
            out: Box::new(SharedBuf(buf.clone())),
            pending: Vec::new(),
        },
        buf,
    )
}
fn interp_with_frame(text: &str) -> lisp::Interp {
    let mut i = lisp::Interp::new();
    let buf_id = i.create_buffer("test-buf");
    {
        let b = i.buffers.get(buf_id).unwrap();
        b.borrow_mut().insert(text);
        b.borrow_mut().set_point(0);
    }
    let mb_id = i.create_buffer(" *Minibuf-0*");
    let frame = editor::Frame::new_tty(buf_id, mb_id, 80, 25);
    i.selected_frame = Some(frame.clone());
    i.frames.push(frame);
    i.set_current_buffer(buf_id);
    i
}
#[test]
fn render_draws_buffer_and_mode_line() {
    let (mut t, out) = test_term(80, 25);
    let i = interp_with_frame("hello\nworld");
    t.render(&i).unwrap();
    let s = String::from_utf8_lossy(&out.borrow()).to_string();
    assert!(s.contains("hello"), "render should show buffer text");
    assert!(s.contains("test-buf"), "mode line shows buffer name");
    assert!(s.contains("\x1b[7m"), "mode line is reverse-video");
    let _ = i;
}
#[test]
fn render_shows_echo_message_when_minibuf_empty() {
    let (mut t, out) = test_term(40, 10);
    let mut i = interp_with_frame("x");
    i.message("a message");
    t.render(&i).unwrap();
    let s = String::from_utf8_lossy(&out.borrow()).to_string();
    assert!(s.contains("a message"));
}
#[test]
fn render_minibuffer_text_wins_over_echo() {
    let (mut t, out) = test_term(40, 10);
    let mut i = interp_with_frame("x");
    i.message("echo-msg");
    let mb = {
        let f = i.selected_frame.as_ref().unwrap().borrow();
        f.minibuffer.clone().unwrap().borrow().buffer
    };
    i.buffers.get(mb).unwrap().borrow_mut().insert("mini-text");
    t.render(&i).unwrap();
    let s = String::from_utf8_lossy(&out.borrow()).to_string();
    assert!(s.contains("mini-text"));
    assert!(!s.contains("echo-msg"));
}
#[test]
fn draw_echo_pads_and_positions_cursor() {
    let (mut t, out) = test_term(20, 6);
    t.draw_echo("Prompt: abc").unwrap();
    let s = String::from_utf8_lossy(&out.borrow()).to_string();
    assert!(s.contains("Prompt: abc"));
    assert!(s.contains("\x1b["), "uses cursor-move escapes");
}
#[test]
fn unread_key_is_returned_by_poll() {
    let (mut t, _) = test_term(10, 4);
    t.unread(97);
    assert_eq!(t.pending.pop(), Some(97));
}
#[test]
fn render_handles_multi_window_frame() {
    let (mut t, out) = test_term(80, 10);
    let mut i = interp_with_frame("w1");
    // Split the frame's window in two.
    let f = i.selected_frame.as_ref().unwrap().clone();
    let buf2 = i.create_buffer("buf2");
    i.buffers.get(buf2).unwrap().borrow_mut().insert("w2");
    let w2 = editor::Window::new(buf2);
    {
        let mut fb = f.borrow_mut();
        fb.windows.push(w2);
    }
    t.render(&i).unwrap();
    let s = String::from_utf8_lossy(&out.borrow()).to_string();
    assert!(s.contains("w1") && s.contains("w2"));
}
#[test]
fn key_event_to_code_legacy_control_bytes() {
    for (input, expected) in [('4', 28), ('5', 29), ('6', 30), ('7', 31)] {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char(input), KeyModifiers::CONTROL)),
            Some(expected),
        );
        assert_eq!(
            key_event_to_code(ev(
                KeyCode::Char(input),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            )),
            Some(expected | CHAR_META),
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char(input), KeyModifiers::NONE)),
            Some(input as i128),
        );
    }
}
#[test]
fn key_event_to_code_plain_and_modified_chars() {
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::NONE)),
        Some('a' as i128)
    );
    // C-u folds to 21.
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(21)
    );
    // C-? folds to 127 (DEL).
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('?'), KeyModifiers::CONTROL)),
        Some(127)
    );
    // Meta sets CHAR_META on the char.
    let m = key_event_to_code(ev(KeyCode::Char('x'), KeyModifiers::ALT)).unwrap();
    assert_eq!(m & !0x3f_ffff, CHAR_META);
    // Shift on a printable char is folded into the character.
    let s = key_event_to_code(ev(KeyCode::Char('A'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(s, 'A' as i128);
    // C-M-a keeps meta + folds control.
    let cm = key_event_to_code(ev(
        KeyCode::Char('a'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ))
    .unwrap();
    assert_eq!(cm & !0x3f_ffff, CHAR_META);
    assert_eq!(cm & 0x3f_ffff, 1);
    // Super/hyper pass through as modifier bits.
    let sup = key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::SUPER)).unwrap();
    assert_eq!(sup & !0x3f_ffff, CHAR_SUPER);
    let hyp = key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::HYPER)).unwrap();
    assert_eq!(hyp & !0x3f_ffff, CHAR_HYPER);
}
#[test]
fn key_event_to_code_named_keys() {
    assert_eq!(
        key_event_to_code(ev(KeyCode::Enter, KeyModifiers::NONE)),
        Some(13)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Tab, KeyModifiers::NONE)),
        Some(9)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Backspace, KeyModifiers::NONE)),
        Some(127)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Esc, KeyModifiers::NONE)),
        Some(27)
    );
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Delete,
        KeyCode::Insert,
    ] {
        assert!(key_event_to_code(ev(code, KeyModifiers::NONE)).is_some());
    }
    assert!(key_event_to_code(ev(KeyCode::F(5), KeyModifiers::NONE)).is_some());
    // Unmapped codes yield None.
    assert_eq!(
        key_event_to_code(ev(KeyCode::Null, KeyModifiers::NONE)),
        None
    );
}
#[test]
fn modified_named_keys_follow_lisp_bindings() {
    let mut i = Interp::new();
    i.eval_str("(progn (insert \"alpha\\nbeta\") (goto-char (point-max)))")
        .unwrap();
    let mut keys = Vec::new();
    let mut arg_mode = false;
    for (key, expected) in [(KeyCode::Home, "1"), (KeyCode::End, "11")] {
        let code = key_event_to_code(ev(key, KeyModifiers::CONTROL)).unwrap();
        dispatch_key_core(&mut i, code, &mut keys, &mut arg_mode, None).unwrap();
        let point = i.eval_str("(point)").unwrap();
        assert_eq!(i.prin1_to_string(&point), expected);
    }
    i.eval_str("(setq qa297-hit 0)").unwrap();
    for (key, modifiers, name) in [
        (
            KeyCode::F(5),
            KeyModifiers::ALT | KeyModifiers::SHIFT,
            "M-S-f5",
        ),
        (
            KeyCode::Right,
            KeyModifiers::CONTROL | KeyModifiers::SUPER,
            "C-s-right",
        ),
        (
            KeyCode::Left,
            KeyModifiers::HYPER | KeyModifiers::SHIFT,
            "H-S-left",
        ),
        (
            KeyCode::Delete,
            KeyModifiers::CONTROL
                | KeyModifiers::ALT
                | KeyModifiers::SHIFT
                | KeyModifiers::SUPER
                | KeyModifiers::HYPER,
            "C-H-M-S-s-delete",
        ),
    ] {
        i.eval_str(&format!(
            "(global-set-key [{name}] (lambda () (interactive) (setq qa297-hit (1+ qa297-hit))))"
        ))
        .unwrap();
        let code = key_event_to_code(ev(key, modifiers)).unwrap();
        dispatch_key_core(&mut i, code, &mut keys, &mut arg_mode, None).unwrap();
    }
    let hit = i.eval_str("qa297-hit").unwrap();
    assert_eq!(i.prin1_to_string(&hit), "4");
    assert!(keys.is_empty());
}
fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: mods,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}
#[test]
fn ctrl_chars_fold_to_control_codes() {
    // C-u -> 21, C-x -> 24, C-a -> 1 (Emacs event representation).
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(21)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        Some(24)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        Some(1)
    );
    // C-? -> DEL
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('?'), KeyModifiers::CONTROL)),
        Some(127)
    );
    // C-@ -> 0 (crossterm may deliver it as C-2 or C-Space variant)
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('@'), KeyModifiers::CONTROL)),
        Some(0)
    );
}
#[test]
fn meta_sets_bit() {
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('f'), KeyModifiers::ALT)),
        Some(b'f' as i128 | CHAR_META)
    );
    // M-C-f -> meta | control-folded char
    assert_eq!(
        key_event_to_code(ev(
            KeyCode::Char('f'),
            KeyModifiers::ALT | KeyModifiers::CONTROL
        )),
        Some(6 | CHAR_META)
    );
}
#[test]
fn named_keys_get_symbol_codes() {
    assert_eq!(
        key_event_to_code(ev(KeyCode::Up, KeyModifiers::empty())),
        Some(named_code("up"))
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::PageDown, KeyModifiers::empty())),
        Some(named_code("next"))
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::F(3), KeyModifiers::empty())),
        Some(named_code("f3"))
    );
    // M-<up>
    assert_eq!(
        key_event_to_code(ev(KeyCode::Up, KeyModifiers::ALT)),
        Some(named_code("up") | CHAR_META)
    );
}
#[test]
fn plain_chars_and_whitespace() {
    assert_eq!(
        key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::empty())),
        Some(97)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Enter, KeyModifiers::empty())),
        Some(13)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Tab, KeyModifiers::empty())),
        Some(9)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Backspace, KeyModifiers::empty())),
        Some(127)
    );
    assert_eq!(
        key_event_to_code(ev(KeyCode::Esc, KeyModifiers::empty())),
        Some(27)
    );
}
/// Control sequences and flush succeed, but the data write fails.
struct RejectText;
impl io::Write for RejectText {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.starts_with(b"hello") {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "text output failed",
            ))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn terminal_propagates_text_write_errors() {
    let mut terminal = Terminal {
        width: 20,
        height: 5,
        out: Box::new(RejectText),
        pending: Vec::new(),
    };
    let i = interp_with_frame("hello");
    assert_eq!(
        terminal.render(&i).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        terminal.draw_echo("hello").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}
