//! GPUI view construction and painting of frontend-neutral grids.
use super::{
    keys::{command_keystroke, keystroke_to_code, unicode_key_text},
    logic::GuiEvent,
};
use gpui::*;
use remacs_runtime::frontend::Grid;
use std::sync::mpsc::Sender;
mod input;
use input::TextInputState;

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
const MODE_BG: u32 = 0x373b41;
const CURSOR: u32 = 0xc5c8c6;

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 19.0;

pub(super) struct EditorView {
    tx: Sender<GuiEvent>,
    pub(super) focus: FocusHandle,
    pub(super) grid: Grid,
    pub(super) cols: usize,
    rows: usize,
    input: TextInputState,
    cell_width: Pixels,
}

impl EditorView {
    pub(super) fn new(tx: Sender<GuiEvent>, cx: &mut Context<Self>) -> Self {
        EditorView {
            tx,
            focus: cx.focus_handle(),
            grid: Grid::blank(0, 0),
            cols: 80,
            rows: 24,
            input: TextInputState::default(),
            cell_width: px(8.0),
        }
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_font = font("Menlo");
        let font_size = px(FONT_SIZE);
        let line_height = px(LINE_HEIGHT);
        let font_id = window.text_system().resolve_font(&text_font);
        let cell_width = window
            .text_system()
            .advance(font_id, font_size, '0')
            .map(|s| s.width)
            .unwrap_or(px(8.0));
        self.cell_width = cell_width;

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
        let entity = cx.entity();
        let focus = self.focus.clone();
        let marked = self.input.marked.clone();
        let marked_cursor = self.input.selection_byte_end();
        let composition_font = text_font.clone();
        div()
            .size_full()
            .bg(rgb(BG))
            .track_focus(&self.focus)
            .key_context("remacs")
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                // Printable text is delivered by the OS input handler. Sending
                // it here as well would insert ordinary ASCII twice.
                if command_keystroke(&event.keystroke)
                    && let Some(code) = keystroke_to_code(&event.keystroke)
                {
                    let _ = tx.send(GuiEvent::Key(code));
                    window.prevent_default();
                    cx.stop_propagation();
                } else if let Some(text) = unicode_key_text(&event.keystroke) {
                    // Some macOS text injection paths report non-ASCII text
                    // as a key event without calling the input client.
                    let _ = tx.send(GuiEvent::Text(text.to_owned()));
                    window.prevent_default();
                    cx.stop_propagation();
                }
            })
            .child(
                canvas(
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
                                let mut cur: Option<remacs_runtime::frontend::CellStyle> = None;
                                let mut run_bytes = 0usize;
                                let mut push_run =
                                    |st: remacs_runtime::frontend::CellStyle, len: usize| {
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
                                    let st =
                                        cells.and_then(|c| c.get(x)).copied().unwrap_or_default();
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
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), _cx);
                        for (y, line) in lines.iter().enumerate() {
                            let origin =
                                point(bounds.left(), bounds.top() + line_height * (y as f32));
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
                            let mut origin = point(
                                bounds.left() + cell_width * (cxp as f32),
                                bounds.top() + line_height * (yp as f32),
                            );
                            if !marked.is_empty() {
                                let line = window.text_system().shape_line(
                                    marked.clone().into(),
                                    font_size,
                                    &[TextRun {
                                        len: marked.len(),
                                        font: composition_font,
                                        color: rgb(FG).into(),
                                        background_color: None,
                                        underline: Some(UnderlineStyle {
                                            thickness: px(1.0),
                                            color: None,
                                            wavy: false,
                                        }),
                                        strikethrough: None,
                                    }],
                                    None,
                                );
                                window.paint_quad(fill(
                                    Bounds {
                                        origin,
                                        size: size(line.width.max(cell_width), line_height),
                                    },
                                    rgb(BG),
                                ));
                                let _ = line.paint(origin, line_height, window, _cx);
                                origin.x += line.x_for_index(marked_cursor);
                            }
                            window.paint_quad(fill(
                                Bounds {
                                    origin,
                                    size: size(cell_width, line_height),
                                },
                                rgba((CURSOR << 8) | 0x80),
                            ));
                        }
                    },
                )
                .size_full(),
            )
    }
}

/// CellStyle → (foreground, background) for gpui; `:inverse-video'
/// swaps the two (default palette entries stand in for unset sides).
fn style_colors(st: remacs_runtime::frontend::CellStyle) -> (Hsla, Option<Hsla>) {
    let cc = |c: remacs_runtime::frontend::CellColor| -> Hsla {
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

/// Construct the root view: focus it for key input and spawn the
/// pump that applies logic-thread grids to the view. Shared between
/// `run_gui` and the headless tests.
pub(super) fn build_editor_view(
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
