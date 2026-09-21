//! gpui front-end: a GPU-rendered window driven by the shared
//! command loop (`term::run_editor_with`).
//!
//! `Interp` is `!Send`, so the evaluator lives on a dedicated logic
//! thread. The window forwards key/resize events over an mpsc
//! channel and paints `render_grid` snapshots pushed back over a
//! bounded (latest-wins) async channel. Neither side blocks the
//! other: the UI stays responsive while Lisp runs, and the logic
//! thread never waits on the paint loop.

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use gpui::*;

use crate::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use crate::lisp::Interp;
use crate::term::{Grid, KeyIo, render_grid};

/// Messages the UI thread sends to the editor logic thread.
enum GuiEvent {
    Key(i128),
    Resize(usize, usize),
}

/// `KeyIo` implementation backed by channels; runs on the logic
/// thread inside `run_editor_with`.
struct ChanIo {
    events: Receiver<GuiEvent>,
    grids: smol::channel::Sender<Grid>,
    /// Keys pushed back via `unread` (unread-command-events).
    pending: Vec<i128>,
    cols: usize,
    rows: usize,
    /// Last grid sent, so `draw_echo` can patch just the echo row.
    last: Option<Grid>,
}

impl ChanIo {
    fn send_grid(&mut self, grid: Grid) -> io::Result<()> {
        // Latest-wins: force_send displaces the oldest queued frame
        // when the bounded channel is full.
        let g = grid.clone();
        self.last = Some(grid);
        self.grids
            .force_send(g)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gui window closed"))?;
        Ok(())
    }
}

impl KeyIo for ChanIo {
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        loop {
            match self.events.recv_timeout(dur) {
                Ok(GuiEvent::Key(k)) => return Ok(Some(k)),
                Ok(GuiEvent::Resize(c, r)) => {
                    self.cols = c.max(10);
                    self.rows = r.max(3);
                    return Ok(None);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(None),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "gui window closed",
                    ));
                }
            }
        }
    }

    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let grid = render_grid(i, self.cols, self.rows);
        self.send_grid(grid)
    }

    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        let mut grid = self.last.clone().unwrap_or(Grid {
            rows: vec![String::new(); self.rows],
            mode_rows: Vec::new(),
            cursor: None,
        });
        if let Some(last) = grid.rows.last_mut() {
            *last = format!("{text:<width$}", width = self.cols);
        }
        let cx = text.chars().count().min(self.cols.saturating_sub(1));
        grid.cursor = Some((cx, self.rows.saturating_sub(1)));
        self.send_grid(grid)
    }

    fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
}

/// Translate a gpui keystroke to an Emacs event code, mirroring
/// `key_event_to_code` in the terminal front-end.
fn keystroke_to_code(ks: &Keystroke) -> Option<i128> {
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
    let named = |name: &str| crate::editor::event_code_for(name);
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

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
const MODE_BG: u32 = 0x373b41;
const CURSOR: u32 = 0xc5c8c6;

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 19.0;

struct EditorView {
    tx: Sender<GuiEvent>,
    focus: FocusHandle,
    grid: Grid,
    cols: usize,
    rows: usize,
}

impl EditorView {
    fn new(tx: Sender<GuiEvent>, cx: &mut Context<Self>) -> Self {
        EditorView {
            tx,
            focus: cx.focus_handle(),
            grid: Grid {
                rows: Vec::new(),
                mode_rows: Vec::new(),
                cursor: None,
            },
            cols: 80,
            rows: 24,
        }
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let text_font = font("Menlo");
        let font_size = px(FONT_SIZE);
        let line_height = px(LINE_HEIGHT);
        let font_id = window.text_system().resolve_font(&text_font);
        let cell_width = window
            .text_system()
            .advance(font_id, font_size, '0')
            .map(|s| s.width)
            .unwrap_or(px(8.0));

        // Report grid-size changes to the logic thread.
        let vp = window.viewport_size();
        let cols = (f32::from(vp.width) / f32::from(cell_width)) as usize;
        let rows = (f32::from(vp.height) / f32::from(line_height)) as usize;
        let cols = cols.max(10);
        let rows = rows.max(3);
        if (cols, rows) != (self.cols, self.rows) {
            self.cols = cols;
            self.rows = rows;
            let _ = self.tx.send(GuiEvent::Resize(cols, rows));
        }

        let grid = self.grid.clone();
        let tx = self.tx.clone();
        div()
            .size_full()
            .bg(rgb(BG))
            .track_focus(&self.focus)
            .key_context("remacs")
            .on_key_down(move |event: &KeyDownEvent, _window, _cx| {
                if let Some(code) = keystroke_to_code(&event.keystroke) {
                    let _ = tx.send(GuiEvent::Key(code));
                }
            })
            .child(canvas(
                move |_bounds, window, _cx| {
                    let shaped: Vec<_> = grid
                        .rows
                        .iter()
                        .map(|row| {
                            window.text_system().shape_line(
                                row.clone().into(),
                                font_size,
                                &[TextRun {
                                    len: row.len(),
                                    font: text_font.clone(),
                                    color: rgb(FG).into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                }],
                                None,
                            )
                        })
                        .collect();
                    (shaped, grid)
                },
                move |bounds, (lines, grid), window, _cx| {
                    for (y, line) in lines.iter().enumerate() {
                        let origin = point(bounds.left(), bounds.top() + line_height * (y as f32));
                        if grid.mode_rows.contains(&y) {
                            window.paint_quad(fill(
                                Bounds {
                                    origin,
                                    size: size(bounds.size.width, line_height),
                                },
                                rgb(MODE_BG),
                            ));
                        }
                        let _ = line.paint(origin, line_height, window, _cx);
                    }
                    if let Some((cxp, yp)) = grid.cursor {
                        let origin = point(
                            bounds.left() + cell_width * (cxp as f32),
                            bounds.top() + line_height * (yp as f32),
                        );
                        window.paint_quad(fill(
                            Bounds {
                                origin,
                                size: size(cell_width, line_height),
                            },
                            rgba((CURSOR << 8) | 0x80),
                        ));
                    }
                },
            ))
    }
}

/// Spawn the logic thread: build an `Interp`, visit `files`, and
/// run the shared command loop against a channel-backed `KeyIo`.
/// Returns the event sender (UI → logic) and the grid receiver
/// (logic → UI). Exposed separately from `run_gui` so tests can
/// drive the whole loop headlessly.
fn start_logic(
    files: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    let (event_tx, event_rx) = mpsc::channel::<GuiEvent>();
    let (grid_tx, grid_rx) = smol::channel::bounded::<Grid>(1);
    std::thread::Builder::new()
        .name("remacs-logic".into())
        .spawn(move || {
            let mut i = Interp::new();
            for f in &files {
                let form = format!("(find-file {:?})", f);
                let _ = i.eval_str(&form);
            }
            let io = ChanIo {
                events: event_rx,
                grids: grid_tx,
                pending: Vec::new(),
                cols: 80,
                rows: 24,
                last: None,
            };
            let _ = crate::term::run_editor_with(io, &mut i);
            // grid_tx drops here → the UI pump below quits the app.
        })
        .map_err(io::Error::other)?;
    Ok((event_tx, grid_rx))
}

/// Launch the gpui front-end. `files` are visited via `find-file`
/// once the evaluator boots on the logic thread.
pub fn run_gui(files: &[String]) -> io::Result<()> {
    let (event_tx, grid_rx) = start_logic(files.to_vec())?;

    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.0), px(640.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                cx.activate(false);
                cx.new(|cx| build_editor_view(window, cx, event_tx, grid_rx))
            },
        )
        .map_err(|e| eprintln!("remacs: cannot open window: {e}"))
        .ok();
    });
    Ok(())
}

/// Construct the root view: focus it for key input and spawn the
/// pump that applies logic-thread grids to the view. Shared between
/// `run_gui` and the headless tests.
fn build_editor_view(
    window: &mut Window,
    cx: &mut Context<EditorView>,
    event_tx: Sender<GuiEvent>,
    grid_rx: smol::channel::Receiver<Grid>,
) -> EditorView {
    let view = EditorView::new(event_tx, cx);
    view.focus.focus(window);
    cx.spawn_in(window, async move |this, cx| {
        while let Ok(grid) = grid_rx.recv().await {
            if this
                .update(cx, |v, cx| {
                    v.grid = grid;
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
        }
        // Logic thread exited (kill-emacs / C-x C-c).
        let _ = cx.update(|_window, cx| cx.quit());
    })
    .detach();
    view
}

#[cfg(test)]
mod tests {
    use super::{ChanIo, EditorView, GuiEvent, build_editor_view, keystroke_to_code, start_logic};
    use crate::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
    use crate::lisp::Interp;
    use crate::term::{Grid, KeyIo};
    use gpui::{Keystroke, Modifiers};
    use std::sync::mpsc::{self, Sender};
    use std::time::Duration;

    fn ks(
        key: &str,
        key_char: Option<&str>,
        control: bool,
        alt: bool,
        shift: bool,
        platform: bool,
    ) -> Keystroke {
        Keystroke {
            modifiers: Modifiers {
                control,
                alt,
                shift,
                platform,
                function: false,
            },
            key: key.into(),
            key_char: key_char.map(|s| s.into()),
        }
    }

    #[test]
    fn keystroke_plain_chars() {
        assert_eq!(
            keystroke_to_code(&ks("a", Some("a"), false, false, false, false)),
            Some(97)
        );
        assert_eq!(
            keystroke_to_code(&ks("z", None, false, false, false, false)),
            Some(122)
        );
        // Digits and punctuation pass through.
        assert_eq!(
            keystroke_to_code(&ks("5", Some("5"), false, false, false, false)),
            Some(53)
        );
    }

    #[test]
    fn keystroke_shift_folds_into_char() {
        // S-a types "A": shift bit dropped, char is uppercase.
        let code = keystroke_to_code(&ks("a", Some("A"), false, false, true, false)).unwrap();
        assert_eq!(code, 65);
        assert_eq!(code & CHAR_SHIFT, 0);
        // Shifted symbol via key_char (S-1 → !).
        let code = keystroke_to_code(&ks("1", Some("!"), false, false, true, false)).unwrap();
        assert_eq!(code & !CHAR_SHIFT, '!' as i128);
    }

    #[test]
    fn keystroke_ctrl_folds_to_control_char() {
        // C-x → 24 like a tty.
        assert_eq!(
            keystroke_to_code(&ks("x", Some("x"), true, false, false, false)),
            Some(24)
        );
        // C-u → 21, C-g → 7.
        assert_eq!(
            keystroke_to_code(&ks("u", None, true, false, false, false)),
            Some(21)
        );
        assert_eq!(
            keystroke_to_code(&ks("g", None, true, false, false, false)),
            Some(7)
        );
        // C-? folds to DEL.
        assert_eq!(
            keystroke_to_code(&ks("?", None, true, false, false, false)),
            Some(127)
        );
    }

    #[test]
    fn keystroke_meta_and_super() {
        // M-f.
        let f = keystroke_to_code(&ks("f", None, false, true, false, false)).unwrap();
        assert_eq!(f, 'f' as i128 | CHAR_META);
        // Cmd-s on macOS → Super.
        let s = keystroke_to_code(&ks("s", None, false, false, false, true)).unwrap();
        assert_eq!(s, 's' as i128 | CHAR_SUPER);
        // C-M-x: ctrl folded, meta kept.
        let x = keystroke_to_code(&ks("x", None, true, true, false, false)).unwrap();
        assert_eq!(x, 24 | CHAR_META);
    }

    #[test]
    fn keystroke_named_keys() {
        use crate::editor::event_code_for;
        assert_eq!(
            keystroke_to_code(&ks("return", None, false, false, false, false)),
            Some(13)
        );
        assert_eq!(
            keystroke_to_code(&ks("tab", None, false, false, false, false)),
            Some(9)
        );
        assert_eq!(
            keystroke_to_code(&ks("backspace", None, false, false, false, false)),
            Some(127)
        );
        assert_eq!(
            keystroke_to_code(&ks("escape", None, false, false, false, false)),
            Some(27)
        );
        assert_eq!(
            keystroke_to_code(&ks("space", Some(" "), false, false, false, false)),
            Some(32)
        );
        assert_eq!(
            keystroke_to_code(&ks("up", None, false, false, false, false)),
            Some(event_code_for("up"))
        );
        assert_eq!(
            keystroke_to_code(&ks("pageup", None, false, false, false, false)),
            Some(event_code_for("prior"))
        );
        assert_eq!(
            keystroke_to_code(&ks("pagedown", None, false, false, false, false)),
            Some(event_code_for("next"))
        );
        assert_eq!(
            keystroke_to_code(&ks("f5", None, false, false, false, false)),
            Some(event_code_for("f5"))
        );
        assert_eq!(
            keystroke_to_code(&ks("delete", None, false, false, false, false)),
            Some(event_code_for("delete"))
        );
    }

    #[test]
    fn keystroke_modified_named_and_unknown() {
        use crate::editor::event_code_for;
        // C-<right> keeps the ctl bit on the symbol event.
        let c = keystroke_to_code(&ks("right", None, true, false, false, false)).unwrap();
        assert_eq!(c, event_code_for("right") | CHAR_CTL);
        // Unknown named keys produce no event.
        assert_eq!(
            keystroke_to_code(&ks("capslock", None, false, false, false, false)),
            None
        );
    }

    fn chan_pair() -> (ChanIo, Sender<GuiEvent>, smol::channel::Receiver<Grid>) {
        let (etx, erx) = mpsc::channel();
        let (gtx, grx) = smol::channel::bounded(1);
        (
            ChanIo {
                events: erx,
                grids: gtx,
                pending: Vec::new(),
                cols: 80,
                rows: 24,
                last: None,
            },
            etx,
            grx,
        )
    }

    #[test]
    fn chanio_key_events_and_unread() {
        let (mut io, tx, _grx) = chan_pair();
        tx.send(GuiEvent::Key(24)).unwrap();
        assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(24));
        // Timeout → None.
        assert_eq!(io.poll_key(Duration::from_millis(5)).unwrap(), None);
        // unread returns before queued events.
        tx.send(GuiEvent::Key(97)).unwrap();
        io.unread(7);
        assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(7));
        assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(97));
    }

    #[test]
    fn chanio_resize_updates_size() {
        let (mut io, tx, _grx) = chan_pair();
        assert_eq!(io.size(), (80, 24));
        tx.send(GuiEvent::Resize(120, 40)).unwrap();
        assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), None);
        assert_eq!(io.size(), (120, 40));
        // Clamped to the minimum.
        tx.send(GuiEvent::Resize(1, 1)).unwrap();
        assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), None);
        assert_eq!(io.size(), (10, 3));
    }

    #[test]
    fn chanio_disconnect_errors() {
        let (mut io, tx, _grx) = chan_pair();
        drop(tx);
        assert!(io.poll_key(Duration::from_millis(10)).is_err());
    }

    #[test]
    fn chanio_render_and_draw_echo() {
        let (mut io, _tx, grx) = chan_pair();
        let mut i = Interp::new();
        i.eval_str(r#"(insert "hello")"#).unwrap();
        io.render(&i).unwrap();
        let grid = grx.try_recv().unwrap();
        assert_eq!(grid.rows.len(), 24);
        assert!(grid.rows[0].starts_with("hello"));
        assert!(
            grid.mode_rows
                .iter()
                .any(|&r| grid.rows[r].contains("scratch"))
        );
        // draw_echo patches only the last row and moves the cursor.
        io.draw_echo("M-x find-").unwrap();
        let grid = grx.try_recv().unwrap();
        assert!(grid.rows.last().unwrap().starts_with("M-x find-"));
        assert_eq!(grid.cursor, Some((9, 23)));
        assert!(grid.rows[0].starts_with("hello"));
    }

    #[test]
    fn chanio_send_grid_latest_wins_and_closed() {
        let (mut io, _tx, grx) = chan_pair();
        // Bounded(1) + force_send: newest displaces the queued frame.
        io.send_grid(Grid {
            rows: vec!["old".into()],
            mode_rows: vec![],
            cursor: None,
        })
        .unwrap();
        io.send_grid(Grid {
            rows: vec!["new".into()],
            mode_rows: vec![],
            cursor: None,
        })
        .unwrap();
        assert_eq!(grx.try_recv().unwrap().rows[0], "new");
        drop(grx);
        assert!(
            io.send_grid(Grid {
                rows: vec![],
                mode_rows: vec![],
                cursor: None
            })
            .is_err()
        );
    }

    #[test]
    fn logic_thread_renders_and_quits() {
        let (tx, rx) = start_logic(Vec::new()).unwrap();
        // The loop paints a frame as soon as it starts.
        let grid = rx.recv_blocking().unwrap();
        assert_eq!(grid.rows.len(), 24);
        assert!(grid.rows.iter().any(|r| r.contains("scratch")));
        // C-x C-c → save-buffers-kill-emacs → loop exits, sender drops.
        tx.send(GuiEvent::Key(24)).unwrap();
        tx.send(GuiEvent::Key(3)).unwrap();
        for _ in 0..50 {
            if rx.recv_blocking().is_err() {
                return;
            }
        }
        panic!("logic thread did not exit after C-x C-c");
    }

    /// A scripted editing session over `ChanIo` — same key codes as the
    /// terminal pty test, covering `dispatch_key`, `isearch_loop` and
    /// `minibuf_loop` for the channel IO flavor.
    #[test]
    fn logic_thread_drives_editing_session() {
        let (tx, rx) = start_logic(Vec::new()).unwrap();
        rx.recv_blocking().unwrap(); // initial frame
        let meta = CHAR_META;
        let keys: Vec<i128> = vec![
            'h' as i128,
            'i' as i128, // self-insert "hi"
            21,          // C-u
            '3' as i128,
            'z' as i128,        // C-u 3 z
            19,                 // C-s → isearch
            'i' as i128,        // search "i"
            127,                // DEL pops the search char
            6,                  // C-f inside isearch
            13,                 // RET exits isearch
            'x' as i128 | meta, // M-x → minibuffer
            'd' as i128,
            'e' as i128,
            's' as i128,
            'c' as i128,
            9,  // TAB completes
            13, // RET runs it
            7,  // C-g safety
            11, // C-k
            25, // C-y
            31, // C-_ undo
            24,
            3, // C-x C-c
        ];
        for k in keys {
            if tx.send(GuiEvent::Key(k)).is_err() {
                break;
            }
        }
        for _ in 0..400 {
            if rx.recv_blocking().is_err() {
                return;
            }
        }
        panic!("logic thread did not exit after scripted session");
    }

    #[test]
    fn headless_view_renders_and_reports_size() {
        let mut app_cx = gpui::TestAppContext::single();
        let (tx, rx) = mpsc::channel::<GuiEvent>();
        let (view, cx) = app_cx.add_window_view(|_window, cx| EditorView::new(tx, cx));
        // The initial render reported the test display's grid size.
        match rx.try_recv().unwrap() {
            GuiEvent::Resize(c, r) => {
                assert!(c >= 10 && r >= 3);
            }
            _ => panic!("expected a resize report"),
        }
        // Push a grid and repaint: exercises the canvas paint path.
        let grid = Grid {
            rows: vec![
                "line one".to_string(),
                "line two".to_string(),
                "--mode".to_string(),
            ],
            mode_rows: vec![2],
            cursor: Some((3, 1)),
        };
        view.update(cx, |v, cx| {
            v.grid = grid;
            cx.notify();
        });
        cx.run_until_parked();
        let cols = view.update(cx, |v, _cx| v.cols);
        assert!(cols >= 10);
    }

    #[test]
    fn build_editor_view_pumps_grids() {
        // Same construction path as run_gui's open_window closure.
        let mut app_cx = gpui::TestAppContext::single();
        let (tx, _rx) = mpsc::channel::<GuiEvent>();
        let (gtx, grx) = smol::channel::bounded::<Grid>(1);
        let handle = app_cx.add_window(|window, cx| build_editor_view(window, cx, tx, grx));
        // A grid pushed through the channel is applied by the pump.
        gtx.force_send(Grid {
            rows: vec!["pumped".to_string()],
            mode_rows: vec![],
            cursor: Some((1, 0)),
        })
        .unwrap();
        app_cx.run_until_parked();
        let text = handle
            .update(&mut app_cx, |v, _w, _cx| v.grid.rows[0].clone())
            .unwrap();
        assert_eq!(text, "pumped");
        // Closing the channel ends the pump (quits in production).
        drop(gtx);
        app_cx.run_until_parked();
    }
}
