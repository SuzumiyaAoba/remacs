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
use crate::lisp::Interp;
use crate::lisp::value::Value;

/// Frontend-agnostic key/render interface used by the command loop
/// and the nested minibuffer/isearch loops. The crossterm `Terminal`
/// implements it directly; the gpui front-end implements it over a
/// channel to the UI thread.
pub trait KeyIo {
    /// Poll for a key event, waiting at most `dur`. Returns an Emacs
    /// key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>>;
    /// Push a key back onto the pending queue.
    fn unread(&mut self, k: i128);
    /// Repaint the whole frame.
    fn render(&mut self, i: &Interp) -> io::Result<()>;
    /// Show TEXT in the echo area (minibuffer prompt/status).
    fn draw_echo(&mut self, text: &str) -> io::Result<()>;
    /// Current frame size in text cells.
    fn size(&self) -> (usize, usize);
}

/// A text-mode screen snapshot produced by `render_grid`: one string
/// per row, which rows are mode lines (drawn inverted), and the
/// hardware cursor position. Both front-ends share it — the terminal
/// writes it, gpui draws it.
#[derive(Clone)]
pub struct Grid {
    pub rows: Vec<String>,
    pub mode_rows: Vec<usize>,
    pub cursor: Option<(usize, usize)>,
}

/// Compute the screen contents for the selected frame at `width` ×
/// `height` text cells. Pure — reads Interp state only.
pub fn render_grid(i: &Interp, width: usize, height: usize) -> Grid {
    let mut rows = vec![String::new(); height];
    let mut mode_rows = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let frame = match &i.selected_frame {
        Some(f) => f.clone(),
        None => {
            return Grid {
                rows,
                mode_rows,
                cursor: cursor_pos,
            };
        }
    };
    let fb = frame.borrow();
    let n_windows = fb.windows.len();
    let mini_height = 1usize;
    let body_height = height.saturating_sub(mini_height);

    for (wi, w) in fb.windows.iter().enumerate() {
        let per = if n_windows == 0 {
            0
        } else {
            body_height / n_windows
        };
        let top = wi * per;
        let wh = if wi == n_windows - 1 {
            body_height.saturating_sub(per * wi)
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
        let text_rows = wh.saturating_sub(1);
        for row in 0..text_rows {
            let y = top + row;
            if y >= body_height {
                break;
            }
            let line = text_lines.get(row).cloned().unwrap_or_default();
            let vis: String = line.chars().skip(hscroll).take(width).collect();
            rows[y] = format!("{vis:<width$}", width = width);
        }
        // Mode line.
        let my = top + text_rows;
        if my < body_height {
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
                if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    "  [sel]"
                } else {
                    ""
                }
            );
            rows[my] = format!("{mode:<width$}", width = width);
            mode_rows.push(my);
        }
        // Cursor for the selected window.
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
    // Minibuffer / echo area (last row).
    let mini_text = match fb
        .minibuffer
        .as_ref()
        .and_then(|w| i.buffers.get(w.borrow().buffer))
    {
        Some(b) => b.borrow().text.text(),
        None => String::new(),
    };
    let msg = if mini_text.is_empty() {
        i.echo_message.clone()
    } else {
        mini_text
    };
    if let Some(last) = rows.last_mut() {
        *last = format!("{msg:<width$}", width = width);
    }
    Grid {
        rows,
        mode_rows,
        cursor: cursor_pos,
    }
}

/// The terminal screen.
pub struct Terminal {
    pub width: usize,
    pub height: usize,
    out: Box<dyn io::Write>,
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
            out: Box::new(out),
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
}

impl KeyIo for Terminal {
    /// Poll for a key event, waiting at most `dur`.
    /// Returns an Emacs key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
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
    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    /// Draw the frame: each window's buffer, mode lines, minibuffer line.
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let (width, height) = (self.width, self.height);
        let grid = render_grid(i, width, height);
        queue!(self.out, cursor::MoveTo(0, 0))?;
        for (y, row) in grid.rows.iter().enumerate() {
            queue!(self.out, cursor::MoveTo(0, y as u16))?;
            if grid.mode_rows.contains(&y) {
                let _ = write!(self.out, "\x1b[7m{}\x1b[0m", row);
            } else {
                let _ = write!(self.out, "{row}");
            }
        }
        if let Some((x, y)) = grid.cursor {
            queue!(self.out, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
        }
        self.out.flush()
    }

    /// Draw the echo-area line with TEXT, cursor at its end.
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
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

    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
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
                if c == '?' { 127 } else { (c as i128) & 0x1f }
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
    run_editor_with(Terminal::enter()?, i)
}

/// The editor command loop, parameterized over a front-end. Also
/// used by the gpui front-end, which supplies a channel-backed KeyIo
/// and runs this on a dedicated logic thread (Interp is !Send).
pub fn run_editor_with<T: KeyIo + 'static>(frontend: T, i: &mut Interp) -> io::Result<()> {
    let term = std::rc::Rc::new(std::cell::RefCell::new(frontend));
    // Install the interactive input hook so read-from-minibuffer,
    // M-x, y-or-n-p, read-char, and `interactive' spec codes can read
    // input from inside the evaluator.
    {
        let t = term.clone();
        i.minibuf_reader = Some(std::rc::Rc::new(move |interp, prompt, single| {
            minibuf_loop::<T>(&t, interp, prompt, single)
        }));
    }
    // Sync frame geometry.
    {
        let (w, h) = term.borrow().size();
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
        // Replayed macro events take priority over real input;
        // `executing-kbd-macro` clears once the queue drains.
        let key = match i.macro_replay.pop_front() {
            Some(k) => {
                i.macro_replaying = true;
                k
            }
            None => {
                let ek = i.intern("executing-kbd-macro");
                if i.symbol_value(ek).truthy() {
                    let _ = i.set_symbol(ek, Value::Nil);
                }
                // Poll input. Sleep deadlines keep the UI responsive.
                let timeout = Duration::from_millis(50);
                match term.borrow_mut().poll_key(timeout)? {
                    Some(k) => k,
                    None => continue,
                }
            }
        };
        dispatch_key(&term, i, key, &mut keys, &mut arg_mode)?;
        i.macro_replaying = false;
    }
    Ok(())
}

/// Process one key event: extend the key sequence, look up the
/// binding, and execute the command (or self-insert / enter isearch).
/// Shared by every front-end via `run_editor_with`.
fn dispatch_key<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
) -> io::Result<()> {
    {
        if *arg_mode {
            let digit = (48..58).contains(&key);
            let minus = key == 45;
            let cu = key == 21;
            if digit || minus || cu {
                let lce = i.intern("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let name = if cu {
                    "universal-argument"
                } else {
                    "digit-argument"
                };
                let id = i.intern(name);
                let _ = i.command_execute(&Value::Sym(id));
                return Ok(());
            }
            *arg_mode = false;
        }
        keys.push(key);
        // Record the event when defining a keyboard macro (replayed
        // keys are not re-recorded, matching GNU).
        if !i.macro_replaying {
            let dm = i.intern("defining-kbd-macro");
            if i.symbol_value(dm).truthy() {
                i.kbd_macro_events.push(Value::Int(key));
            }
        }
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
                // isearch commands enter a dedicated incremental loop
                // rather than running once through command_execute.
                let isearch = if let Value::Sym(id) = &cmd {
                    match i.symbol_name(*id).as_str() {
                        "isearch-forward" => Some((false, false)),
                        "isearch-backward" => Some((true, false)),
                        "isearch-forward-regexp" => Some((false, true)),
                        "isearch-backward-regexp" => Some((true, true)),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((back, re)) = isearch {
                    isearch_loop(term, i, back, re)?;
                    return Ok(());
                }
                match i.command_execute(&cmd) {
                    Ok(_) => {}
                    Err(crate::lisp::error::Flow::Quit) => {
                        i.message("Quit");
                    }
                    Err(crate::lisp::error::Flow::Signal(sym, data, _)) => {
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
                        i.message(&format!("No catch for tag: {}", i.princ_to_string(&tag)));
                        let _ = v;
                    }
                    Err(crate::lisp::error::Flow::Exit(_)) => {
                        // `kill-emacs' requested termination; the flag is
                        // already set, so the loop exits at the top.
                        i.quit_editor = true;
                    }
                }
                // Continue arg entry after C-u / M-digit / M--.
                if let Value::Sym(id) = &cmd {
                    let n = i.symbol_name(*id);
                    *arg_mode = matches!(
                        n.as_str(),
                        "universal-argument" | "digit-argument" | "negative-argument"
                    );
                }
                // Command boundary: `cancel-kbd-macro-events` truncates
                // back to this mark.
                i.kbd_macro_mark = i.kbd_macro_events.len();
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
                            return Ok(());
                        }
                        if k == b'\r' as i128 {
                            if let Some(b) = i.current_buffer_ref() {
                                b.borrow_mut().insert("\n");
                            }
                            keys.clear();
                            return Ok(());
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
                            return Ok(());
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

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
///
/// `point` semantics follow Emacs: forward searches leave point after
/// the match, backward searches leave it at the match start. C-g
/// restores the entry position; RET exits and sets the mark there.
/// Live state of an incremental search.
struct Isearch {
    query: String,
    /// History of match positions; last is the current match.
    positions: Vec<usize>,
    failing: bool,
    start: usize,
    backward: bool,
    regexp: bool,
}

/// Run one search; returns the new match point (or None when the
/// query doesn't match from `from`). `from` is 0-based.
fn isearch_search(
    i: &mut Interp,
    query: &str,
    backward: bool,
    regexp: bool,
    from: usize,
) -> Option<usize> {
    if query.is_empty() {
        return Some(from);
    }
    let mut esc = String::new();
    for c in query.chars() {
        if c == '\\' || c == '"' {
            esc.push('\\');
        }
        esc.push(c);
    }
    let fn_name = match (backward, regexp) {
        (false, false) => "search-forward",
        (true, false) => "search-backward",
        (false, true) => "re-search-forward",
        (true, true) => "re-search-backward",
    };
    // +1 for the point→char offset used by these primitives.
    let src = format!(
        "(progn (goto-char (min {} (point-max))) ({} \"{}\" nil t))",
        from + 1,
        fn_name,
        esc
    );
    match i.eval_str(&src) {
        Ok(v) if !v.is_nil() => i.current_buffer_ref().map(|b| b.borrow().point()),
        _ => None,
    }
}

/// What the isearch driver should do after a key.
enum IsearchAction {
    /// Keep reading keys.
    Continue,
    /// Exit, restoring point to the entry position (C-g).
    Abort,
    /// Exit, keeping point and pushing the mark at entry (RET).
    Done,
    /// Exit and re-dispatch this key as a command.
    ReDispatch(i128),
}

impl Isearch {
    fn new(i: &Interp, backward: bool, regexp: bool) -> Isearch {
        let start = i
            .current_buffer_ref()
            .map(|b| b.borrow().point())
            .unwrap_or(0);
        Isearch {
            query: String::new(),
            positions: vec![start],
            failing: false,
            start,
            backward,
            regexp,
        }
    }

    fn cur(&self) -> usize {
        *self.positions.last().unwrap_or(&self.start)
    }

    fn go_to(&self, i: &Interp, p: usize) {
        if let Some(b) = i.current_buffer_ref() {
            b.borrow_mut().set_point(p);
        }
    }

    /// Handle one key event; updates query/point/failing state.
    fn step(&mut self, i: &mut Interp, key: i128) -> IsearchAction {
        let cur = self.cur();
        match key {
            // C-g: abort, restore entry position.
            7 => {
                self.go_to(i, self.start);
                IsearchAction::Abort
            }
            // RET: accept; mark goes at the entry position.
            13 => {
                if let Some(b) = i.current_buffer_ref() {
                    b.borrow_mut().mark = Some(self.start);
                }
                IsearchAction::Done
            }
            // C-s / C-r: repeat search in that direction from just
            // past (before) the current match.
            19 | 18 => {
                let back = key == 18;
                let from = if back { cur.saturating_sub(1) } else { cur + 1 };
                match isearch_search(i, &self.query, back, self.regexp, from) {
                    Some(p) => {
                        self.positions.push(p);
                        self.failing = false;
                        self.go_to(i, p);
                    }
                    None => self.failing = true,
                }
                IsearchAction::Continue
            }
            // DEL: unwind one step of the search.
            127 => {
                if self.positions.len() > 1 {
                    self.positions.pop();
                    // If the tail of query grew since the last repeat,
                    // pop a char too.
                    if !self.query.is_empty() {
                        self.query.pop();
                    }
                    self.go_to(i, self.cur());
                    self.failing = false;
                } else if !self.query.is_empty() {
                    self.query.pop();
                    self.failing = false;
                }
                IsearchAction::Continue
            }
            c if c >= 32
                && c < 0x110000
                && c & (CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER) == 0 =>
            {
                if let Some(ch) = char::from_u32(c as u32) {
                    self.query.push(ch);
                }
                // Extend the current match; fall back to a fresh
                // search from the entry point when it no longer hits.
                let from = if self.positions.len() > 1 {
                    cur
                } else {
                    self.start
                };
                let hit = isearch_search(i, &self.query, self.backward, self.regexp, from).or_else(
                    || isearch_search(i, &self.query, self.backward, self.regexp, self.start),
                );
                match hit {
                    Some(p) => {
                        self.positions.push(p);
                        self.failing = false;
                        self.go_to(i, p);
                    }
                    None => self.failing = true,
                }
                IsearchAction::Continue
            }
            other => {
                if let Some(b) = i.current_buffer_ref() {
                    b.borrow_mut().mark = Some(self.start);
                }
                IsearchAction::ReDispatch(other)
            }
        }
    }

    /// Echo-area label, e.g. `Failing I-search backward: foo`.
    fn label(&self) -> String {
        let dir = if self.backward { " backward" } else { "" };
        let re = if self.regexp { " regexp" } else { "" };
        let status = if self.failing { "Failing " } else { "" };
        format!("{}I-search{}{}: {}", status, dir, re, self.query)
    }
}

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
fn isearch_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    backward: bool,
    regexp: bool,
) -> io::Result<()> {
    let mut st = Isearch::new(i, backward, regexp);
    loop {
        {
            let mut t = term.borrow_mut();
            let _ = t.render(i);
            let _ = t.draw_echo(&st.label());
        }
        let key = term
            .borrow_mut()
            .poll_key(Duration::from_secs(86400))?
            .unwrap_or(0);
        match st.step(i, key) {
            IsearchAction::Continue => {}
            IsearchAction::Abort | IsearchAction::Done => break,
            IsearchAction::ReDispatch(other) => {
                let seq = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(
                    other,
                )])));
                if let LookupResult::Command(cmd) = lookup_command(i, &seq) {
                    let _ = i.command_execute(&cmd);
                }
                break;
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
fn minibuf_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
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

    /// An Interp with a tty frame displaying a buffer holding `text`.
    fn interp_with_frame(text: &str) -> crate::lisp::Interp {
        let mut i = crate::lisp::Interp::new();
        let buf_id = i.buffers.create("test-buf");
        {
            let b = i.buffers.get(buf_id).unwrap();
            b.borrow_mut().insert(text);
            b.borrow_mut().set_point(0);
        }
        let mb_id = i.buffers.create(" *Minibuf-0*");
        let frame = crate::editor::Frame::new_tty(buf_id, mb_id, 80, 25);
        i.selected_frame = Some(frame.clone());
        i.frames.push(frame);
        i.current_buffer = buf_id;
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
        let buf2 = i.buffers.create("buf2");
        i.buffers.get(buf2).unwrap().borrow_mut().insert("w2");
        let w2 = crate::editor::Window::new(buf2);
        {
            let mut fb = f.borrow_mut();
            fb.windows.push(w2);
        }
        t.render(&i).unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("w1") && s.contains("w2"));
    }

    /// Interp whose current buffer holds `text`, point at start.
    fn isearch_interp(text: &str) -> crate::lisp::Interp {
        let mut i = crate::lisp::Interp::new();
        let buf_id = i.buffers.create("s");
        i.current_buffer = buf_id;
        i.buffers.get(buf_id).unwrap().borrow_mut().insert(text);
        i.buffers.get(buf_id).unwrap().borrow_mut().set_point(0);
        i
    }

    fn point(i: &Interp) -> usize {
        i.current_buffer_ref().unwrap().borrow().point()
    }

    #[test]
    fn isearch_chars_move_point_to_match() {
        let mut i = isearch_interp("one two one\n");
        let mut st = Isearch::new(&i, false, false);
        assert!(matches!(
            st.step(&mut i, 't' as i128),
            IsearchAction::Continue
        ));
        assert!(matches!(
            st.step(&mut i, 'w' as i128),
            IsearchAction::Continue
        ));
        assert!(matches!(
            st.step(&mut i, 'o' as i128),
            IsearchAction::Continue
        ));
        // "two" ends at index 7.
        assert_eq!(point(&i), 7);
        assert!(!st.failing);
        assert!(st.label().contains("I-search: two"));
    }

    #[test]
    fn isearch_cs_repeats_and_del_unwinds() {
        let mut i = isearch_interp("aa aa\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'a' as i128);
        let first = point(&i);
        st.step(&mut i, 19); // C-s → next match
        assert!(point(&i) > first);
        st.step(&mut i, 19); // no third match → failing
        assert!(st.failing);
        st.step(&mut i, 127); // DEL unwinds
        assert_eq!(point(&i), first);
    }

    #[test]
    fn isearch_cg_restores_entry() {
        let mut i = isearch_interp("xx yy\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'y' as i128);
        assert!(point(&i) > 0);
        assert!(matches!(st.step(&mut i, 7), IsearchAction::Abort));
        assert_eq!(point(&i), 0);
    }

    #[test]
    fn isearch_ret_sets_mark() {
        let mut i = isearch_interp("xx yy\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'y' as i128);
        assert!(matches!(st.step(&mut i, 13), IsearchAction::Done));
        let b = i.current_buffer_ref().unwrap();
        let bb = b.borrow();
        assert_eq!(bb.mark, Some(0));
        assert!(bb.point() > 0);
    }

    #[test]
    fn isearch_backward_searches_backward() {
        let mut i = isearch_interp("aa bb aa\n");
        // Point at end.
        i.current_buffer_ref().unwrap().borrow_mut().set_point(8);
        let mut st = Isearch::new(&i, true, false);
        st.step(&mut i, 'a' as i128);
        // Backward match puts point at match start (index 7).
        assert_eq!(point(&i), 7);
        assert!(st.label().contains("backward"));
    }

    #[test]
    fn isearch_other_key_redispatches() {
        let mut i = isearch_interp("text\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'x' as i128);
        match st.step(&mut i, 24) {
            IsearchAction::ReDispatch(k) => assert_eq!(k, 24),
            _ => panic!("C-x should re-dispatch"),
        }
    }

    #[test]
    fn isearch_failing_then_empty_query() {
        let mut i = isearch_interp("abc\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'z' as i128);
        st.step(&mut i, 'z' as i128);
        assert!(st.failing);
        assert!(st.label().starts_with("Failing"));
        st.step(&mut i, 127); // DEL pops one char
        st.step(&mut i, 127);
        assert_eq!(st.query, "");
    }

    // ---------- key translation ----------

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

    // ---------- input loops ----------

    #[test]
    fn minibuf_loop_reads_text() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, b'b' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        // pending is a stack — reverse order.
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        match minibuf_loop(&term, &mut i, "P: ", false) {
            Ok(crate::lisp::eval::MinibufInput::Text(s)) => assert_eq!(s, "ab"),
            other => panic!("expected Text, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn minibuf_loop_del_and_cg() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        // type 'x', DEL, then C-g abort.
        for k in [b'x' as i128, 127, 7] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        assert!(minibuf_loop(&term, &mut i, "", false).is_err());
    }

    #[test]
    fn minibuf_loop_single_key_mode() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        term.borrow_mut().unread(b'y' as i128);
        let mut i = crate::lisp::Interp::new();
        match minibuf_loop(&term, &mut i, "y/n ", true) {
            Ok(crate::lisp::eval::MinibufInput::Key(k)) => assert_eq!(k, b'y' as i128),
            _ => panic!("expected Key"),
        }
    }

    #[test]
    fn isearch_loop_end_to_end() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        // Type 'y', then RET to finish.
        for k in [b'y' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_frame("xx yy\n");
        isearch_loop(&term, &mut i, false, false).unwrap();
        let b = i.current_buffer_ref().unwrap();
        assert_eq!(b.borrow().point(), 4); // after the first "y" match
    }

    #[test]
    fn isearch_loop_abort_restores() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'y' as i128, 7] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_frame("xx yy\n");
        isearch_loop(&term, &mut i, false, false).unwrap();
        assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), 0);
    }

    #[test]
    fn nonblocking_sleep_short() {
        let mut i = crate::lisp::Interp::new();
        nonblocking_sleep(&mut i, Duration::from_millis(1)).unwrap();
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

    #[test]
    fn lookup_routes_to_bindings() {
        let mut i = crate::lisp::Interp::new();
        let seq = |k: i128| {
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(
                k,
            )])))
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
        assert!(matches!(
            lookup_command(&mut i, &seq(24)),
            LookupResult::Prefix
        ));
        // C-x C-c -> save-buffers-kill-terminal (GNU's binding)
        let two = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Int(24),
            Value::Int(3),
        ])));
        match lookup_command(&mut i, &two) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "save-buffers-kill-terminal")
            }
            _ => panic!("C-x C-c should resolve"),
        }
    }
}
