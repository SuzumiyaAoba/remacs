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
        match self.events.recv_timeout(dur) {
            Ok(GuiEvent::Key(k)) => Ok(Some(k)),
            Ok(GuiEvent::Resize(c, r)) => {
                self.cols = c.max(10);
                self.rows = r.max(3);
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "gui window closed",
            )),
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
        let mut grid = self
            .last
            .clone()
            .unwrap_or_else(|| Grid::blank(self.cols, self.rows));
        if let Some(last) = grid.rows.last_mut() {
            *last = format!("{text:<width$}", width = self.cols);
        }
        if let Some(last) = grid.attrs.last_mut() {
            last.iter_mut()
                .for_each(|c| *c = crate::term::CellStyle::default());
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
            grid: Grid::blank(0, 0),
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
                    // One TextRun per contiguous same-style span so
                    // face attributes (colors, bold, underline, …)
                    // land on the cells that carry them.
                    let shaped: Vec<_> = grid
                        .rows
                        .iter()
                        .enumerate()
                        .map(|(y, row)| {
                            let cells = grid.attrs.get(y);
                            let mut runs: Vec<TextRun> = Vec::new();
                            let mut cur: Option<crate::term::CellStyle> = None;
                            let mut run_bytes = 0usize;
                            let mut push_run = |st: crate::term::CellStyle, len: usize| {
                                if len == 0 {
                                    return;
                                }
                                let (fg, bg) = style_colors(st);
                                let mut f = text_font.clone();
                                if st.bold {
                                    f = f.bold();
                                }
                                if st.italic {
                                    f = f.italic();
                                }
                                runs.push(TextRun {
                                    len,
                                    font: f,
                                    color: fg,
                                    background_color: bg,
                                    underline: st.underline.then(|| UnderlineStyle {
                                        thickness: px(1.0),
                                        color: None,
                                        wavy: false,
                                    }),
                                    strikethrough: st.strike.then(|| StrikethroughStyle {
                                        thickness: px(1.0),
                                        color: None,
                                    }),
                                });
                            };
                            for (x, ch) in row.chars().enumerate() {
                                let st = cells
                                    .and_then(|c| c.get(x))
                                    .copied()
                                    .unwrap_or_default();
                                match cur {
                                    Some(c) if c == st => run_bytes += ch.len_utf8(),
                                    _ => {
                                        if let Some(c) = cur {
                                            push_run(c, run_bytes);
                                        }
                                        cur = Some(st);
                                        run_bytes = ch.len_utf8();
                                    }
                                }
                            }
                            if let Some(c) = cur {
                                push_run(c, run_bytes);
                            }
                            if runs.is_empty() {
                                runs.push(TextRun {
                                    len: 0,
                                    font: text_font.clone(),
                                    color: rgb(FG).into(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                });
                            }
                            window.text_system().shape_line(
                                row.clone().into(),
                                font_size,
                                &runs,
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

/// CellStyle → (foreground, background) for gpui; `:inverse-video'
/// swaps the two (default palette entries stand in for unset sides).
fn style_colors(st: crate::term::CellStyle) -> (Hsla, Option<Hsla>) {
    let cc = |c: crate::term::CellColor| -> Hsla {
        rgb(((c.0 as u32) << 16) | ((c.1 as u32) << 8) | c.2 as u32).into()
    };
    let fg = st.fg.map(cc).unwrap_or_else(|| rgb(FG).into());
    let bg = st.bg.map(cc);
    if st.inverse {
        (bg.unwrap_or_else(|| rgb(BG).into()), Some(fg))
    } else {
        (fg, bg)
    }
}

/// Spawn the logic thread: build an `Interp`, visit `files`, and
/// run the shared command loop against a channel-backed `KeyIo`.
/// Returns the event sender (UI → logic) and the grid receiver
/// (logic → UI). Exposed separately from `run_gui` so tests can
/// drive the whole loop headlessly.
#[cfg(test)]
fn start_logic(
    files: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    start_logic_with_dump(files, None, Vec::new())
}

fn apply_startup_arguments(
    i: &mut Interp,
    files: &[String],
    arguments: &[String],
) -> crate::lisp::EvalResult {
    let base_symbol = i.intern("command-line-default-directory");
    let base = i.symbol_value(base_symbol);
    let visit = |i: &mut Interp, name: &str| {
        let expand = crate::lisp::Value::Sym(i.intern("expand-file-name"));
        let file = i.apply(
            &expand,
            vec![crate::lisp::Value::string(name), base.clone()],
        )?;
        let find = crate::lisp::Value::Sym(i.intern("find-file"));
        i.apply(&find, vec![file])
    };
    if arguments.is_empty() {
        for name in files {
            visit(i, name)?;
        }
    } else {
        let mut paths = crate::lisp::load::CommandLineLoadPath::default();
        for argument in crate::cli::arguments(arguments) {
            match argument {
                crate::cli::Argument::File(name) => {
                    visit(i, name)?;
                }
                crate::cli::Argument::Option {
                    index,
                    name: "-L" | "--directory",
                } => {
                    let directory = arguments
                        .get(index + 1)
                        .map(crate::lisp::Value::string)
                        .unwrap_or(crate::lisp::Value::Nil);
                    paths.add(i, directory)?;
                }
                crate::cli::Argument::Option { name, .. } if name.starts_with("--directory=") => {
                    paths.add(i, crate::lisp::Value::string(&name[12..]))?;
                }
                _ => {}
            }
        }
    }
    Ok(crate::lisp::Value::Nil)
}

fn start_logic_with_dump(
    files: Vec<String>,
    dump_file: Option<String>,
    arguments: Vec<String>,
) -> io::Result<(Sender<GuiEvent>, smol::channel::Receiver<Grid>)> {
    let wait_for_dump = dump_file.is_some();
    let (startup_tx, startup_rx) = mpsc::channel::<Result<(), String>>();
    let (event_tx, event_rx) = mpsc::channel::<GuiEvent>();
    let (grid_tx, grid_rx) = smol::channel::bounded::<Grid>(1);
    std::thread::Builder::new()
        .name("remacs-logic".into())
        // Eval/macroexpand recursion needs far more than the default
        // thread stack (see main.rs's headless worker).
        .stack_size(512 * 1024 * 1024)
        .spawn(move || {
            let mut i = match dump_file {
                None => Interp::new(),
                Some(path) => match crate::lisp::dump::load_with_arguments(
                    std::path::Path::new(&path),
                    Some(&arguments),
                ) {
                    Ok(interpreter) => interpreter,
                    Err(crate::lisp::dump::LoadError::Exit(code)) => {
                        std::process::exit(code as i32)
                    }
                    Err(error) => {
                        let _ = startup_tx.send(Err(error.to_string()));
                        return;
                    }
                },
            };
            if let Err(error) = apply_startup_arguments(&mut i, &files, &arguments) {
                let message = format!("GUI startup: {error:?}");
                eprintln!("remacs: {message}");
                let _ = startup_tx.send(Err(message));
                return;
            }
            let _ = startup_tx.send(Ok(()));
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
    if wait_for_dump {
        startup_rx
            .recv()
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
    }
    Ok((event_tx, grid_rx))
}

/// Launch the gpui front-end. `files` are visited via `find-file`
/// once the evaluator boots on the logic thread.
pub fn run_gui(files: &[String]) -> io::Result<()> {
    run_gui_with_dump(files, None, &[])
}

pub fn run_gui_with_dump(
    files: &[String],
    dump_file: Option<&str>,
    arguments: &[String],
) -> io::Result<()> {
    let (event_tx, grid_rx) = start_logic_with_dump(
        files.to_vec(),
        dump_file.map(str::to_owned),
        arguments.to_vec(),
    )?;

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
        {
            let mut g = Grid::blank(0, 0);
            g.rows = vec!["old".into()];
            io.send_grid(g)
        }
        .unwrap();
        {
            let mut g = Grid::blank(0, 0);
            g.rows = vec!["new".into()];
            io.send_grid(g)
        }
        .unwrap();
        assert_eq!(grx.try_recv().unwrap().rows[0], "new");
        drop(grx);
        assert!(
            io.send_grid(Grid::blank(0, 0))
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

    #[test]
    fn logic_thread_applies_directory_options_before_visiting_files() {
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let root = Owned(
            std::env::temp_dir().join(format!(
                "remacs-gui-load-path-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )),
        );
        for (name, value) in [("A", "loaded-A"), ("B", "loaded-B")] {
            std::fs::create_dir_all(root.0.join(name)).unwrap();
            std::fs::write(
                root.0.join(name).join("qa229-gui.el"),
                format!("(defun qa229-gui-value () \"{value}\")\n(provide 'qa229-gui)\n"),
            )
            .unwrap();
        }
        let file = root.0.join("payload.txt");
        std::fs::write(&file, "GUI").unwrap();
        let file = file.to_string_lossy().into_owned();
        let (tx, rx) = super::start_logic_with_dump(
            vec![file.clone()],
            None,
            vec![
                "-L".into(),
                root.0.join("A").to_string_lossy().into_owned(),
                format!("--directory={}", root.0.join("B").display()),
                file,
            ],
        )
        .unwrap();
        let wait_for = |text: &str| {
            let deadline = std::time::Instant::now() + Duration::from_secs(120);
            loop {
                if let Ok(grid) = rx.try_recv() {
                    if grid.rows.iter().any(|row| row.contains(text)) {
                        break;
                    }
                }
                assert!(!rx.is_closed(), "logic thread closed before {text}");
                assert!(
                    std::time::Instant::now() < deadline,
                    "no GUI grid contained {text}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        wait_for("GUI");
        tx.send(GuiEvent::Key(':' as i128 | CHAR_META)).unwrap();
        for ch in "(progn (require 'qa229-gui) (insert (qa229-gui-value)) (set-buffer-modified-p nil))"
            .chars()
        {
            tx.send(GuiEvent::Key(ch as i128)).unwrap();
        }
        tx.send(GuiEvent::Key(13)).unwrap();
        wait_for("loaded-AGUI");
        tx.send(GuiEvent::Key(24)).unwrap();
        tx.send(GuiEvent::Key(3)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !rx.is_closed() {
            assert!(
                std::time::Instant::now() < deadline,
                "GUI logic thread did not quit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn missing_dump_fails_before_ui_startup() {
        let path = std::env::temp_dir().join(format!(
            "remacs-missing-image-{}-{}.pdmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(
            super::start_logic_with_dump(
                Vec::new(),
                Some(path.to_string_lossy().into_owned()),
                Vec::new()
            )
            .is_err()
        );
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
        let mut grid = Grid::blank(10, 3);
        grid.rows = vec![
            "line one".to_string(),
            "line two".to_string(),
            "--mode".to_string(),
        ];
        grid.mode_rows = vec![2];
        grid.cursor = Some((3, 1));
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
        let mut g = Grid::blank(10, 1);
        g.rows = vec!["pumped".to_string()];
        g.cursor = Some((1, 0));
        gtx.force_send(g)
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
