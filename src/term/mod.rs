//! Terminal front-end (crossterm): raw-mode input, frame rendering,
//! and the editor command loop.
//!
//! Non-blocking design: the loop always `poll`s input with a timeout,
//! renders the frame, and executes commands. Long Lisp operations can
//! interleave with repaints because `sit-for`/`sleep-for` hand control
//! back to this loop rather than blocking inside the evaluator.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Attribute, SetAttribute},
    terminal::{self},
};

use crate::editor::{CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use crate::lisp::value::Value;
use crate::lisp::Interp;

/// The terminal screen.
pub struct Terminal {
    pub width: usize,
    pub height: usize,
    out: io::Stdout,
    /// Pending key events read but not yet consumed (for unread-command-events).
    pending: Vec<i128>,
}

impl Terminal {
    /// Enter raw mode + alternate screen.
    pub fn enter() -> io::Result<Terminal> {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        let w = w.max(10);
        let h = h.max(3);
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        queue!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
        out.flush()?;
        Ok(Terminal {
            width: w as usize,
            height: h as usize,
            out,
            pending: Vec::new(),
        })
    }

    /// Restore the terminal. Always call before exit (RAII via Drop).
    pub fn leave(&mut self) {
        let _ = queue!(
            self.out,
            SetAttribute(Attribute::Reset),
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = self.out.flush();
        let _ = terminal::disable_raw_mode();
    }

    /// Poll for a key event, waiting at most `dur`.
    /// Returns an Emacs key code (char + modifier bits).
    pub fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        if !event::poll(dur)? {
            return Ok(None);
        }
        match event::read()? {
            Event::Key(k) => Ok(key_event_to_code(k)),
            Event::Resize(w, h) => {
                self.width = (w as usize).max(10);
                self.height = (h as usize).max(3);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Push a key back to the pending queue.
    pub fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    /// Draw the frame: each window's buffer, mode lines, minibuffer line.
    pub fn render(&mut self, i: &Interp) -> io::Result<()> {
        let frame = match &i.selected_frame {
            Some(f) => f.clone(),
            None => return Ok(()),
        };
        let (width, height) = (self.width, self.height);
        let fb = frame.borrow();
        let n_windows = fb.windows.len();
        let mini_height = 1usize;
        let body_height = height.saturating_sub(mini_height);

        queue!(self.out, cursor::MoveTo(0, 0))?;

        // Lay out windows vertically.
        let per = if n_windows == 0 {
            body_height
        } else {
            body_height / n_windows
        };
        let mut cursor_pos: Option<(usize, usize)> = None;
        for (wi, w) in fb.windows.iter().enumerate() {
            let top = wi * per;
            let wh = if wi == n_windows - 1 {
                body_height - per * wi
            } else {
                per
            };
            let wh = wh.max(2);
            let (buf_id, start, hscroll) = {
                let wb = w.borrow();
                (wb.buffer, wb.start, wb.hscroll)
            };
            let text_lines: Vec<String> = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let text = bb.text.substring(start, bb.text_len());
                    text.split('\n').map(|s| s.to_string()).collect()
                }
                None => vec![String::new()],
            };
            // Text area: wh-1 lines; last line is the mode line.
            let text_rows = wh.saturating_sub(1);
            for row in 0..text_rows {
                queue!(self.out, cursor::MoveTo(0, (top + row) as u16))?;
                let line = text_lines.get(row).cloned().unwrap_or_default();
                let vis: String = line
                    .chars()
                    .skip(hscroll)
                    .take(width)
                    .collect::<String>();
                let _ = write!(self.out, "{:<width$}", vis, width = width);
            }
            // Mode line (inverted).
            queue!(self.out, cursor::MoveTo(0, (top + text_rows) as u16))?;
            let (name, modified, point_line, point_col) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    (
                        bb.name.clone(),
                        bb.modified,
                        bb.text.line_of_pos(bb.point()) + 1,
                        bb.point() - bb.text.line_start(bb.text.line_of_pos(bb.point())),
                    )
                }
                None => ("???".into(), false, 0, 0),
            };
            let mode = format!(
                "--{}- {}   L{} C{}{}",
                if modified { "**" } else { "--" },
                name,
                point_line,
                point_col,
                if wi == fb.windows.iter().position(|x| std::rc::Rc::ptr_eq(x, &fb.selected)).unwrap_or(usize::MAX) {
                    "  [sel]"
                } else {
                    ""
                }
            );
            let _ = write!(
                self.out,
                "{}{:<width$}{}",
                "\x1b[7m",
                mode,
                "\x1b[0m",
                width = width
            );
            // Track cursor for the selected window.
            if std::rc::Rc::ptr_eq(w, &fb.selected) {
                if let Some(b) = i.buffers.get(buf_id) {
                    let bb = b.borrow();
                    let p = bb.point();
                    let line = bb.text.line_of_pos(p);
                    let ls = bb.text.line_start(line);
                    let start_line = bb.text.line_of_pos(start);
                    let row = line.saturating_sub(start_line);
                    let col = (p - ls).saturating_sub(hscroll);
                    if row < text_rows && col < width {
                        cursor_pos = Some((col, top + row));
                    }
                }
            }
        }
        // Minibuffer / echo area (last line).
        queue!(
            self.out,
            cursor::MoveTo(0, height.saturating_sub(1) as u16)
        )?;
        let mini_text = match fb.minibuffer.as_ref().and_then(|w| {
            i.buffers.get(w.borrow().buffer)
        }) {
            Some(b) => b.borrow().text.text(),
            None => String::new(),
        };
        let msg = if mini_text.is_empty() {
            i.echo_message.clone()
        } else {
            mini_text
        };
        let _ = write!(self.out, "{:<width$}", msg, width = width);
        if let Some((x, y)) = cursor_pos {
            queue!(self.out, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
        }
        self.out.flush()
    }

    /// Draw the echo-area line with TEXT, cursor at its end.
    pub fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        let y = self.height.saturating_sub(1) as u16;
        queue!(
            self.out,
            cursor::MoveTo(0, y),
            terminal::Clear(terminal::ClearType::CurrentLine)
        )?;
        let width = self.width;
        let _ = write!(self.out, "{:<width$}", text, width = width);
        let cx = text.chars().count().min(width.saturating_sub(1));
        queue!(self.out, cursor::MoveTo(cx as u16, y), cursor::Show)?;
        self.out.flush()
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Translate a crossterm key event to an Emacs event code.
fn key_event_to_code(k: KeyEvent) -> Option<i128> {
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
                if c == '?' {
                    127
                } else {
                    (c as i128) & 0x1f
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
    crate::editor::event_code_for(name)
}

// ---------- command loop ----------

/// Run the editor until `C-x C-c` (or `kill-emacs`).
pub fn run_editor(i: &mut Interp) -> io::Result<()> {
    let term = std::rc::Rc::new(std::cell::RefCell::new(Terminal::enter()?));
    // Install the interactive input hook so read-from-minibuffer,
    // M-x, y-or-n-p, read-char, and `interactive' spec codes can read
    // input from inside the evaluator.
    {
        let t = term.clone();
        i.minibuf_reader = Some(std::rc::Rc::new(move |interp, prompt, single| {
            minibuf_loop(&t, interp, prompt, single)
        }));
    }
    // Sync frame geometry.
    {
        let (w, h) = (term.borrow().width, term.borrow().height);
        if let Some(f) = &i.selected_frame {
            f.borrow_mut().width = w;
            f.borrow_mut().height = h;
        }
    }
    let mut keys: Vec<i128> = Vec::new();
    // Set while a universal/digit-argument sequence is being entered:
    // digits, `-', and further C-u keep building `prefix-arg' (Emacs's
    // `universal-argument' continuation behavior).
    let mut arg_mode = false;
    loop {
        if i.quit_editor {
            break;
        }
        term.borrow_mut().render(i)?;
        // Poll input. Sleep deadlines keep the UI responsive.
        let timeout = Duration::from_millis(50);
        let key = match term.borrow_mut().poll_key(timeout)? {
            Some(k) => k,
            None => continue,
        };
        if arg_mode {
            let digit = (48..58).contains(&key);
            let minus = key == 45;
            let cu = key == 21;
            if digit || minus || cu {
                let lce = i.intern("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let name = if cu { "universal-argument" } else { "digit-argument" };
                let id = i.intern(name);
                let _ = i.command_execute(&Value::Sym(id));
                continue;
            }
            arg_mode = false;
        }
        keys.push(key);
        let seq = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
            keys.iter().map(|k| Value::Int(*k)).collect(),
        )));
        // Look up the key sequence.
        let binding = lookup_command(i, &seq);
        match binding {
            LookupResult::Command(cmd) => {
                keys.clear();
                let lce = i.intern("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let this_cmd = i.intern("this-command");
                let prev = i.symbol_value(this_cmd);
                let lc = i.intern("last-command");
                let _ = i.set_symbol(lc, prev);
                let _ = i.set_symbol(this_cmd, cmd.clone());
                let pa = i.intern("prefix-arg");
                let prefix = i.symbol_value(pa);
                let cpa = i.intern("current-prefix-arg");
                let _ = i.set_symbol(cpa, prefix);
                // Consume prefix-arg BEFORE executing so commands that
                // set it (C-u, digit-argument) affect the NEXT command.
                let _ = i.set_symbol(pa, Value::Nil);
                match i.command_execute(&cmd) {
                    Ok(_) => {}
                    Err(crate::lisp::error::Flow::Quit) => {
                        i.message("Quit");
                    }
                    Err(crate::lisp::error::Flow::Signal(sym, data)) => {
                        let name = match &sym {
                            Value::Sym(id) => i.symbol_name(*id),
                            _ => "error".into(),
                        };
                        let msg = format!(
                            "{}: {}",
                            name,
                            data.list_to_vec()
                                .unwrap_or_default()
                                .iter()
                                .map(|v| i.princ_to_string(v))
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        i.message(&msg);
                    }
                    Err(crate::lisp::error::Flow::Throw(tag, v)) => {
                        i.message(&format!(
                            "No catch for tag: {}",
                            i.princ_to_string(&tag)
                        ));
                        let _ = v;
                    }
                }
                // Continue arg entry after C-u / M-digit / M--.
                if let Value::Sym(id) = &cmd {
                    let n = i.symbol_name(*id);
                    arg_mode = matches!(
                        n.as_str(),
                        "universal-argument" | "digit-argument" | "negative-argument"
                    );
                }
            }
            LookupResult::Prefix => {
                // keep reading keys
            }
            LookupResult::None => {
                // Plain char → self-insert.
                if keys.len() == 1 {
                    if let Some(&k) = keys.first() {
                        if k < CHAR_CTL && k >= 32 && k != 127 {
                            if let Some(b) = i.current_buffer_ref() {
                                let ch = char::from_u32(k as u32).unwrap_or('?');
                                b.borrow_mut().insert(&ch.to_string());
                            }
                            keys.clear();
                            continue;
                        }
                        if k == b'\r' as i128 {
                            if let Some(b) = i.current_buffer_ref() {
                                b.borrow_mut().insert("\n");
                            }
                            keys.clear();
                            continue;
                        }
                        if k == 127 {
                            if let Some(b) = i.current_buffer_ref() {
                                let mut bb = b.borrow_mut();
                                let p = bb.point();
                                if p > 0 {
                                    bb.delete_region(p - 1, p);
                                }
                            }
                            keys.clear();
                            continue;
                        }
                    }
                }
                i.message(&format!(
                    "{} is undefined",
                    keys.iter()
                        .map(|k| crate::editor::describe_key_pub(*k))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
                keys.clear();
            }
        }
    }
    Ok(())
}

enum LookupResult {
    Command(Value),
    Prefix,
    None,
}

fn lookup_command(i: &mut Interp, seq: &Value) -> LookupResult {
    let keys = match seq {
        Value::Vec(v) => v.borrow().clone(),
        _ => return LookupResult::None,
    };
    if keys.is_empty() {
        return LookupResult::None;
    }
    // Convert key vector to string for keymap lookup if all plain chars.
    // Our define-key accepts vectors directly.
    let binding = crate::editor::lookup_command_in_maps(i, &keys);
    match binding {
        Some(v) => {
            // is it a keymap (prefix)?
            if let Value::Cons(c) = &v {
                if i.sym_is(&c.borrow().car, i.intern_soft("keymap").unwrap_or(u32::MAX)) {
                    return LookupResult::Prefix;
                }
            }
            if v.truthy() {
                LookupResult::Command(v)
            } else {
                LookupResult::None
            }
        }
        None => {
            // If no binding and the last key extends a prefix → undefined.
            LookupResult::None
        }
    }
}

/// Blocking `sleep-for` replacement at the loop level: repaint during the wait.
pub fn nonblocking_sleep(i: &mut Interp, dur: Duration) -> io::Result<()> {
    let _ = i;
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

/// Nested input loop for minibuffer reads. Runs while the outer
/// command loop is suspended inside `command_execute`.
fn minibuf_loop(
    term: &std::rc::Rc<std::cell::RefCell<Terminal>>,
    i: &mut Interp,
    prompt: &str,
    single: bool,
) -> Result<crate::lisp::eval::MinibufInput, crate::lisp::error::Flow> {
    use crate::lisp::eval::MinibufInput;
    let mut text = String::new();
    loop {
        {
            let mut t = term.borrow_mut();
            let _ = t.draw_echo(&format!("{}{}", prompt, text));
        }
        let k = term
            .borrow_mut()
            .poll_key(Duration::from_secs(86400))
            .ok()
            .flatten();
        let Some(code) = k else {
            continue;
        };
        if single {
            return Ok(MinibufInput::Key(code));
        }
        let base = code & 0x3f_ffff;
        let mods = code & !0x3f_ffff;
        if code == 7 {
            // C-g aborts.
            return Err(crate::lisp::error::Flow::Quit);
        }
        match base {
            13 => return Ok(MinibufInput::Text(text)), // RET
            127 => {
                text.pop();
            }
            c if mods == 0 && (32..0x110000).contains(&c) => {
                if let Some(ch) = char::from_u32(c as u32) {
                    text.push(ch);
                }
            }
            _ => {
                // Ignore other events (arrows, modifiers).
                let _ = i;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

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

    #[test]
    fn lookup_routes_to_bindings() {
        let mut i = crate::lisp::Interp::new();
        let seq = |k: i128| {
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
                Value::Int(k),
            ])))
        };
        // 'a' -> t default -> self-insert-command
        match lookup_command(&mut i, &seq(97)) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "self-insert-command")
            }
            _ => panic!("'a' should resolve to self-insert-command"),
        }
        // C-u -> universal-argument
        match lookup_command(&mut i, &seq(21)) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "universal-argument")
            }
            _ => panic!("C-u should resolve to universal-argument"),
        }
        // C-x alone -> prefix keymap
        assert!(matches!(lookup_command(&mut i, &seq(24)), LookupResult::Prefix));
        // C-x C-c -> save-buffers-kill-emacs
        let two = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Int(24),
            Value::Int(3),
        ])));
        match lookup_command(&mut i, &two) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "save-buffers-kill-emacs")
            }
            _ => panic!("C-x C-c should resolve"),
        }
    }
}
