//! GPUI view construction and painting of frontend-neutral grids.
use super::{
    keys::{command_keystroke, keystroke_to_code, unicode_key_text},
    logic::{GuiEvent, GuiGeometry},
    monitor::monitor_geometry,
};
use gpui::*;
use remacs_runtime::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use remacs_runtime::frontend::{Grid, MouseButton as RemacsMouseButton};
use std::sync::mpsc::Sender;
mod input;
use input::TextInputState;

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
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
    geometry: Option<GuiGeometry>,
    #[cfg(target_os = "macos")]
    accessibility_buffer_view: Option<usize>,
    #[cfg(target_os = "macos")]
    accessibility_screen_view: Option<usize>,
    #[cfg(target_os = "macos")]
    accessibility_snapshot: Option<(
        String,
        std::ops::Range<usize>,
        String,
        std::ops::Range<usize>,
    )>,
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
            geometry: None,
            #[cfg(target_os = "macos")]
            accessibility_buffer_view: None,
            #[cfg(target_os = "macos")]
            accessibility_screen_view: None,
            #[cfg(target_os = "macos")]
            accessibility_snapshot: None,
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
        let scale_factor = window.scale_factor();
        let to_pixels = |value: Pixels| f32::from(value).round() as i128;
        let bounds = window.bounds();
        let current_display_id = window.display(cx).map(|display| u32::from(display.id()));
        let monitors = monitor_geometry(window, cx);
        let frame_left = to_pixels(bounds.origin.x);
        let frame_top = to_pixels(bounds.origin.y);
        // Translate GPUI's AppKit screen-relative bounds to Emacs's global
        // monitor coordinate system when the frame is on a secondary screen.
        #[cfg(target_os = "macos")]
        let frame_left = frame_left.saturating_add(
            current_display_id
                .and_then(|display_id| {
                    monitors
                        .iter()
                        .find(|monitor| monitor.display_id == display_id)
                })
                .map(|monitor| monitor.native_origin[0])
                .unwrap_or(0),
        );
        #[cfg(target_os = "macos")]
        let frame_top = frame_top.saturating_add(
            current_display_id
                .and_then(|display_id| {
                    monitors
                        .iter()
                        .find(|monitor| monitor.display_id == display_id)
                })
                .map(|monitor| monitor.geometry[1].saturating_sub(monitor.native_origin[1]))
                .unwrap_or(0),
        );
        let geometry = GuiGeometry {
            cols,
            rows,
            pixel_width: to_pixels(vp.width),
            pixel_height: to_pixels(vp.height),
            char_width: to_pixels(cell_width).max(1),
            char_height: to_pixels(line_height).max(1),
            scale_factor: scale_factor as f64,
            frame_left,
            frame_top,
            current_display_id,
            monitors,
        };
        if self.geometry.as_ref() != Some(&geometry) {
            self.geometry = Some(geometry.clone());
            let _ = self.tx.send(GuiEvent::Geometry(geometry));
        }

        #[cfg(target_os = "macos")]
        {
            let screen_text = self.grid.rows.join("\n");
            let screen_selection = visible_grid_selection(&self.grid);
            let (buffer_text, buffer_selection) = self.grid.input.as_ref().map_or_else(
                || (screen_text.clone(), screen_selection.clone()),
                |input| {
                    let selection = input
                        .selection
                        .as_ref()
                        .map(|selection| selection.range.clone())
                        .unwrap_or(input.cursor..input.cursor);
                    (input.text.to_string(), selection)
                },
            );
            let changed = self.accessibility_snapshot.as_ref().is_none_or(
                |(old_buffer, old_buffer_selection, old_screen, old_screen_selection)| {
                    old_buffer != &buffer_text
                        || old_buffer_selection != &buffer_selection
                        || old_screen != &screen_text
                        || old_screen_selection != &screen_selection
                },
            );
            if changed
                || self.accessibility_buffer_view.is_none()
                || self.accessibility_screen_view.is_none()
            {
                self.accessibility_buffer_view = super::accessibility::update_text_view(
                    self.accessibility_buffer_view,
                    &buffer_text,
                    buffer_selection.clone(),
                    "Buffer contents",
                    "Current buffer text and input selection.",
                );
                self.accessibility_screen_view = super::accessibility::update_text_view(
                    self.accessibility_screen_view,
                    &screen_text,
                    screen_selection.clone(),
                    "Visible frame contents",
                    "Text currently visible on the frame, including mode line and minibuffer.",
                );
                self.accessibility_snapshot =
                    Some((buffer_text, buffer_selection, screen_text, screen_selection));
            }
        }

        let default_background = self
            .grid
            .default_bg
            .map(|color| ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32)
            .unwrap_or(BG);
        let grid = self.grid.clone();
        let tx = self.tx.clone();
        let mouse_grid = grid.clone();
        let left_mouse_down_tx = self.tx.clone();
        let left_mouse_down_grid = mouse_grid.clone();
        let left_mouse_up_tx = self.tx.clone();
        let left_mouse_up_grid = mouse_grid.clone();
        let middle_mouse_down_tx = self.tx.clone();
        let middle_mouse_down_grid = mouse_grid.clone();
        let middle_mouse_up_tx = self.tx.clone();
        let middle_mouse_up_grid = mouse_grid;
        let entity = cx.entity();
        let focus = self.focus.clone();
        let marked = self.input.marked.clone();
        let marked_cursor = self.input.selection_byte_end();
        let marked_origin = self
            .input
            .replacement_range
            .as_ref()
            .and_then(|range| input::visible_input_position(&self.grid, range.start, false));
        let composition_font = text_font.clone();
        div()
            .size_full()
            .bg(rgb(default_background))
            .track_focus(&self.focus)
            .key_context("remacs")
            .on_mouse_down(MouseButton::Left, move |event, window, _cx| {
                if let Some(position) =
                    input::mouse_position_for_point(&left_mouse_down_grid, event.position, window)
                {
                    let _ = left_mouse_down_tx.send(GuiEvent::MouseDown {
                        button: RemacsMouseButton::Left,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(&event.modifiers),
                        position,
                    });
                }
            })
            .on_mouse_up(MouseButton::Left, move |event, window, _cx| {
                if let Some(position) =
                    input::mouse_position_for_point(&left_mouse_up_grid, event.position, window)
                {
                    let _ = left_mouse_up_tx.send(GuiEvent::MouseUp {
                        button: RemacsMouseButton::Left,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(&event.modifiers),
                        position,
                    });
                }
            })
            .on_mouse_down(MouseButton::Middle, move |event, window, _cx| {
                if let Some(position) =
                    input::mouse_position_for_point(&middle_mouse_down_grid, event.position, window)
                {
                    let _ = middle_mouse_down_tx.send(GuiEvent::MouseDown {
                        button: RemacsMouseButton::Middle,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(&event.modifiers),
                        position,
                    });
                }
            })
            .on_mouse_up(MouseButton::Middle, move |event, window, _cx| {
                if let Some(position) =
                    input::mouse_position_for_point(&middle_mouse_up_grid, event.position, window)
                {
                    let _ = middle_mouse_up_tx.send(GuiEvent::MouseUp {
                        button: RemacsMouseButton::Middle,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(&event.modifiers),
                        position,
                    });
                }
            })
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
                                let cells = grid
                                    .char_attrs
                                    .get(y)
                                    .filter(|cells| !cells.is_empty())
                                    .or_else(|| grid.attrs.get(y));
                                let mut runs: Vec<TextRun> = Vec::new();
                                let mut cur: Option<remacs_runtime::frontend::CellStyle> = None;
                                let mut run_bytes = 0usize;
                                let mut push_run =
                                    |st: remacs_runtime::frontend::CellStyle, len: usize| {
                                        if len == 0 {
                                            return;
                                        }
                                        let (fg, bg) = style_colors(st, default_background);
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
                            let _ = line.paint(origin, line_height, window, _cx);
                            let char_styles = grid
                                .char_attrs
                                .get(y)
                                .filter(|styles| !styles.is_empty())
                                .or_else(|| grid.attrs.get(y));
                            if let Some(char_styles) = char_styles {
                                let overline_y = origin.y
                                    + (line_height - line.ascent - line.descent) / 2.0
                                    + px(1.0);
                                for (index, (byte, ch)) in grid.rows[y].char_indices().enumerate() {
                                    let Some(style) = char_styles.get(index).copied() else {
                                        continue;
                                    };
                                    if !style.overline {
                                        continue;
                                    }
                                    let start = line.x_for_index(byte);
                                    let end = line.x_for_index(byte + ch.len_utf8());
                                    let (left, width) = if end >= start {
                                        (start, end - start)
                                    } else {
                                        (end, start - end)
                                    };
                                    if width > px(0.0) {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin: point(bounds.left() + left, overline_y),
                                                size: size(width, px(1.0)),
                                            },
                                            style_colors(style, default_background).0,
                                        ));
                                    }
                                }
                            }
                        }
                        let fallback_cursor = grid.cursor.map(|(column, row)| {
                            let x = grid
                                .cursor_index
                                .and_then(|index| {
                                    lines.get(row).map(|line| line.x_for_index(index))
                                })
                                .unwrap_or(cell_width * (column as f32));
                            (row, x)
                        });
                        let composition_origin = (!marked.is_empty())
                            .then_some(marked_origin)
                            .flatten()
                            .and_then(|(row, index)| {
                                lines.get(row).map(|line| (row, line.x_for_index(index)))
                            })
                            .or(fallback_cursor);
                        let composition_caret = if !marked.is_empty() {
                            composition_origin.map(|(row, x)| {
                                let origin = point(
                                    bounds.left() + x,
                                    bounds.top() + line_height * (row as f32),
                                );
                                let line = window.text_system().shape_line(
                                    marked.clone().into(),
                                    font_size,
                                    &[TextRun {
                                        len: marked.len(),
                                        font: composition_font.clone(),
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
                                    rgb(default_background),
                                ));
                                let _ = line.paint(origin, line_height, window, _cx);
                                (row, x + line.x_for_index(marked_cursor))
                            })
                        } else {
                            None
                        };

                        let blink_visible = if grid.blink_cursor {
                            let elapsed = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64();
                            (elapsed / grid.blink_interval.max(0.05)).floor() as u64 % 2 == 0
                        } else {
                            true
                        };
                        for cursor in &grid.cursors {
                            if cursor.selected && grid.blink_cursor && !blink_visible {
                                continue;
                            }
                            let row = if cursor.selected {
                                composition_caret.map(|(row, _)| row).unwrap_or(cursor.row)
                            } else {
                                cursor.row
                            };
                            let Some(line) = lines.get(row) else {
                                continue;
                            };
                            let x = if cursor.selected {
                                composition_caret
                                    .map(|(_, x)| x)
                                    .unwrap_or_else(|| line.x_for_index(cursor.index))
                            } else {
                                line.x_for_index(cursor.index)
                            };
                            let glyph_width = if cursor.selected && composition_caret.is_some() {
                                cell_width
                            } else {
                                cursor
                                    .after_index
                                    .map(|end| {
                                        let end_x = line.x_for_index(end);
                                        if end_x >= x { end_x - x } else { x - end_x }
                                    })
                                    .filter(|width| *width > px(0.0))
                                    .unwrap_or(cell_width)
                            };
                            let rgb_color = cursor.color.map_or(CURSOR, |color| {
                                ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32
                            });
                            let origin =
                                point(bounds.left() + x, bounds.top() + line_height * (row as f32));
                            match cursor.shape {
                                remacs_runtime::frontend::GridCursorShape::Block => {
                                    let width = if cursor.thickness > 0 {
                                        px(cursor.thickness as f32)
                                    } else {
                                        glyph_width
                                    };
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin,
                                            size: size(width, line_height),
                                        },
                                        rgba((rgb_color << 8) | 0x80),
                                    ));
                                }
                                remacs_runtime::frontend::GridCursorShape::Hollow => {
                                    let width = if cursor.thickness > 0 {
                                        px(cursor.thickness as f32)
                                    } else {
                                        glyph_width
                                    };
                                    let color = rgb(rgb_color);
                                    let edge = px(1.0);
                                    let height = line_height;
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin,
                                            size: size(width, edge),
                                        },
                                        color,
                                    ));
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin: point(origin.x, origin.y + height - edge),
                                            size: size(width, edge),
                                        },
                                        color,
                                    ));
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin,
                                            size: size(edge, height),
                                        },
                                        color,
                                    ));
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin: point(origin.x + width - edge, origin.y),
                                            size: size(edge, height),
                                        },
                                        color,
                                    ));
                                }
                                remacs_runtime::frontend::GridCursorShape::Bar => {
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin,
                                            size: size(
                                                px(cursor.thickness.max(1) as f32),
                                                line_height,
                                            ),
                                        },
                                        rgb(rgb_color),
                                    ));
                                }
                                remacs_runtime::frontend::GridCursorShape::HBar => {
                                    let height = px(cursor.thickness.max(1) as f32);
                                    window.paint_quad(fill(
                                        Bounds {
                                            origin: point(
                                                origin.x,
                                                origin.y + line_height - height,
                                            ),
                                            size: size(glyph_width, height),
                                        },
                                        rgb(rgb_color),
                                    ));
                                }
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(target_os = "macos")]
fn visible_grid_selection(grid: &Grid) -> std::ops::Range<usize> {
    let Some((_, row)) = grid.cursor else {
        return 0..0;
    };
    let Some(text) = grid.rows.get(row) else {
        return 0..0;
    };
    let requested_byte = grid.cursor_index.unwrap_or(text.len()).min(text.len());
    let byte = (0..=requested_byte)
        .rev()
        .find(|offset| text.is_char_boundary(*offset))
        .unwrap_or(0);
    let line_start: usize = grid
        .rows
        .iter()
        .take(row)
        .map(|line| line.encode_utf16().count() + 1)
        .sum();
    let position = line_start + text[..byte].encode_utf16().count();
    position..position
}

fn mouse_modifier_bits(modifiers: &Modifiers) -> i128 {
    let mut bits = 0;
    if modifiers.control {
        bits |= CHAR_CTL;
    }
    if modifiers.alt {
        bits |= CHAR_META;
    }
    if modifiers.shift {
        bits |= CHAR_SHIFT;
    }
    if modifiers.platform {
        bits |= CHAR_SUPER;
    }
    bits
}

/// CellStyle → (foreground, background) for gpui; `:inverse-video'
/// swaps the two (default palette entries stand in for unset sides).
fn style_colors(
    st: remacs_runtime::frontend::CellStyle,
    default_background: u32,
) -> (Hsla, Option<Hsla>) {
    let cc = |c: remacs_runtime::frontend::CellColor| -> Hsla {
        rgb(((c.0 as u32) << 16) | ((c.1 as u32) << 8) | c.2 as u32).into()
    };
    let fg = st.fg.map(cc).unwrap_or_else(|| rgb(FG).into());
    let bg = st.bg.map(cc);
    if st.inverse {
        (
            bg.unwrap_or_else(|| rgb(default_background).into()),
            Some(fg),
        )
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
                    if v.grid != grid || grid.blink_cursor {
                        v.grid = grid;
                        cx.notify();
                    }
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
