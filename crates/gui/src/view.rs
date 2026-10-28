//! GPUI view construction and painting of frontend-neutral grids.
use super::{
    keys::{command_keystroke, keystroke_to_code, unicode_key_text},
    logic::{GuiEvent, GuiGeometry},
    monitor::monitor_geometry,
};
use gpui::*;
use gpui::prelude::{FluentBuilder, InteractiveElement, StatefulInteractiveElement};
use remacs_runtime::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use remacs_runtime::frontend::{
    BoxStyle, BoxStyleKind, CellColor, Grid, GridImage, GridImageSource, GridMenu, GridMenuItem,
    GridScrollBar, GridScrollBarOrientation, GridTabBarItem, GridToolBarItem,
    MouseButton as RemacsMouseButton,
};
use std::sync::mpsc::Sender;
mod input;
use input::TextInputState;

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
const CURSOR: u32 = 0xc5c8c6;

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 19.0;

#[derive(Clone, Copy)]
struct ScrollBarDrag {
    window_id: usize,
    orientation: GridScrollBarOrientation,
    pointer_offset: f32,
}

struct HelpTooltipView {
    text: String,
}

impl Render for HelpTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().bg(rgb(0x303438)).child(self.text.clone())
    }
}

fn scroll_bar_axis(bar: &GridScrollBar, point: Point<Pixels>) -> f32 {
    let coordinate = match bar.orientation {
        GridScrollBarOrientation::Vertical => point.y,
        GridScrollBarOrientation::Horizontal => point.x,
    };
    let origin = match bar.orientation {
        GridScrollBarOrientation::Vertical => bar.top,
        GridScrollBarOrientation::Horizontal => bar.left,
    };
    f32::from(coordinate) - origin as f32
}

fn scroll_bar_hit(grid: &Grid, point: Point<Pixels>) -> Option<(GridScrollBar, f32)> {
    let x = f32::from(point.x);
    let y = f32::from(point.y);
    grid.scroll_bars.iter().find_map(|bar| {
        let left = bar.left as f32;
        let top = bar.top as f32;
        let right = left + bar.width as f32;
        let bottom = top + bar.height as f32;
        if x < left || x >= right || y < top || y >= bottom {
            return None;
        }
        let axis = scroll_bar_axis(bar, point);
        let pointer_offset = if axis >= bar.thumb_start as f32
            && axis < bar.thumb_start.saturating_add(bar.thumb_size) as f32
        {
            axis - bar.thumb_start as f32
        } else {
            bar.thumb_size as f32 / 2.0
        };
        Some((bar.clone(), pointer_offset))
    })
}

fn scroll_bar_offset(bar: &GridScrollBar, point: Point<Pixels>, pointer_offset: f32) -> usize {
    let axis = scroll_bar_axis(bar, point);
    let track_length = match bar.orientation {
        GridScrollBarOrientation::Vertical => bar.height,
        GridScrollBarOrientation::Horizontal => bar.width,
    };
    let travel = track_length.saturating_sub(bar.thumb_size);
    if travel == 0 || bar.max_offset == 0 {
        return 0;
    }
    let thumb_start = (axis - pointer_offset).clamp(0.0, travel as f32);
    ((thumb_start / travel as f32) * bar.max_offset as f32)
        .round()
        .clamp(0.0, bar.max_offset as f32) as usize
}

fn send_scroll_bar_event(
    tx: &Sender<GuiEvent>,
    bar: &GridScrollBar,
    point: Point<Pixels>,
    pointer_offset: f32,
) {
    let _ = tx.send(GuiEvent::ScrollBar {
        window_id: bar.window_id,
        orientation: bar.orientation,
        offset: scroll_bar_offset(bar, point, pointer_offset),
        max_offset: bar.max_offset,
    });
}

fn tool_bar_command_at(
    items: &[GridToolBarItem],
    point: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> Option<String> {
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let row = (f32::from(point.y) / f32::from(line_height)).floor().max(0.0) as usize;
    items
        .iter()
        .find(|item| item.row == row && column >= item.left && column < item.left + item.width)
        .map(|item| item.command.clone())
}

fn tab_bar_command_at(
    items: &[GridTabBarItem],
    point: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> Option<(String, Vec<i128>)> {
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let row = (f32::from(point.y) / f32::from(line_height)).floor().max(0.0) as usize;
    items
        .iter()
        .find(|item| item.row == row && column >= item.left && column < item.left + item.width)
        .map(|item| {
            (
                item.command.clone(),
                item.argument.into_iter().collect(),
            )
        })
}

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = remacs, no_json)]
struct RunMenuCommand {
    command: String,
}

pub(super) struct EditorView {
    tx: Sender<GuiEvent>,
    pub(super) focus: FocusHandle,
    pub(super) grid: Grid,
    installed_menus: Vec<GridMenu>,
    window_title: Option<String>,
    pub(super) cols: usize,
    rows: usize,
    input: TextInputState,
    cell_width: Pixels,
    geometry: Option<GuiGeometry>,
    scroll_bar_drag: Option<ScrollBarDrag>,
    toolbar_pressed: bool,
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
            installed_menus: Vec::new(),
            window_title: None,
            cols: 80,
            rows: 24,
            input: TextInputState::default(),
            cell_width: px(8.0),
            geometry: None,
            scroll_bar_drag: None,
            toolbar_pressed: false,
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
        if self.window_title.as_deref() != Some(self.grid.title.as_str()) {
            window.set_window_title(&self.grid.title);
            self.window_title = Some(self.grid.title.clone());
        }
        if self.installed_menus != self.grid.menus {
            cx.set_menus(gpui_menus(&self.grid.menus));
            self.installed_menus = self.grid.menus.clone();
        }

        let font_families = self.grid.font_families.clone();
        let default_style = self.grid.default_style;
        let family = default_style
            .font_family
            .and_then(|id| font_families.get(id as usize))
            .cloned()
            .unwrap_or_else(|| "Menlo".to_owned());
        let mut text_font = font(family);
        if let Some(weight) = default_style.font_weight {
            text_font.weight = FontWeight(weight as f32);
        }
        text_font.stretch = FontStretch::from_percentage(default_style.font_width.unwrap_or(100));
        let font_size = default_style
            .font_height
            .map(|height| px(height as f32 / 10.0))
            .unwrap_or(px(FONT_SIZE));
        let line_height = px(f32::from(font_size) * (LINE_HEIGHT / FONT_SIZE));
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
        let images = grid.images.clone();
        let help_echoes = grid.help_echoes.clone();
        let explicit_tooltip = grid.tooltip.clone();
        let tx = self.tx.clone();
        let mouse_grid = grid.clone();
        let scroll_grid = grid.clone();
        let scroll_tx = self.tx.clone();
        let scroll_bar_tx = self.tx.clone();
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
            .on_action(cx.listener(
                |this: &mut EditorView, action: &RunMenuCommand, _window, _cx| {
                    let _ = this
                        .tx
                        .send(GuiEvent::MenuCommand(action.command.clone()));
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    move |this, event: &MouseDownEvent, window, cx| {
                        if let Some(command) = tool_bar_command_at(
                            &left_mouse_down_grid.tool_bar_items,
                            event.position,
                            cell_width,
                            line_height,
                        ) {
                            this.toolbar_pressed = true;
                            let _ = left_mouse_down_tx.send(GuiEvent::MenuCommand(command));
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if let Some((command, args)) = tab_bar_command_at(
                            &left_mouse_down_grid.tab_bar_items,
                            event.position,
                            cell_width,
                            line_height,
                        ) {
                            this.toolbar_pressed = true;
                            let _ = left_mouse_down_tx.send(GuiEvent::CommandArgs { command, args });
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if let Some((bar, pointer_offset)) =
                            scroll_bar_hit(&left_mouse_down_grid, event.position)
                        {
                            this.scroll_bar_drag = Some(ScrollBarDrag {
                                window_id: bar.window_id,
                                orientation: bar.orientation,
                                pointer_offset,
                            });
                            send_scroll_bar_event(
                                &left_mouse_down_tx,
                                &bar,
                                event.position,
                                pointer_offset,
                            );
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(position) = input::mouse_position_for_point(
                            &left_mouse_down_grid,
                            event.position,
                            window,
                        ) {
                            let _ = left_mouse_down_tx.send(GuiEvent::MouseDown {
                                button: RemacsMouseButton::Left,
                                click_count: event.click_count,
                                modifiers: mouse_modifier_bits(&event.modifiers),
                                position,
                            });
                        }
                    },
                ),
            )
            .on_mouse_move(cx.listener(
                move |this, event: &MouseMoveEvent, window, cx| {
                    if !event.dragging() {
                        return;
                    }
                    let Some(drag) = this.scroll_bar_drag else {
                        return;
                    };
                    if let Some(bar) = this.grid.scroll_bars.iter().find(|bar| {
                        bar.window_id == drag.window_id && bar.orientation == drag.orientation
                    }) {
                        send_scroll_bar_event(
                            &scroll_bar_tx,
                            bar,
                            event.position,
                            drag.pointer_offset,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                    if this.toolbar_pressed {
                        this.toolbar_pressed = false;
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.scroll_bar_drag.take().is_some() {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if let Some(position) = input::mouse_position_for_point(
                        &left_mouse_up_grid,
                        event.position,
                        window,
                    ) {
                        let _ = left_mouse_up_tx.send(GuiEvent::MouseUp {
                            button: RemacsMouseButton::Left,
                            click_count: event.click_count,
                            modifiers: mouse_modifier_bits(&event.modifiers),
                            position,
                        });
                    }
                }),
            )
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
            .on_scroll_wheel(move |event, window, cx| {
                if let Some(position) =
                    input::mouse_position_for_point(&scroll_grid, event.position, window)
                {
                    let delta = event.delta.pixel_delta(line_height);
                    let delta_lines = -f32::from(delta.y) / f32::from(line_height);
                    if delta_lines != 0.0 {
                        let _ = scroll_tx.send(GuiEvent::Scroll {
                            delta_lines,
                            modifiers: mouse_modifier_bits(&event.modifiers),
                            position,
                        });
                    }
                    window.prevent_default();
                    cx.stop_propagation();
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
                                        if let Some(family) = st
                                            .font_family
                                            .and_then(|id| font_families.get(id as usize))
                                        {
                                            f.family = family.clone().into();
                                        }
                                        if let Some(weight) = st.font_weight {
                                            f.weight = FontWeight(weight as f32);
                                        } else if st.bold {
                                            f = f.bold();
                                        }
                                        if let Some(width) = st.font_width {
                                            f.stretch = FontStretch::from_percentage(width);
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
                                                color: st
                                                    .underline_color
                                                    .map(|color| rgb(color_rgb(color)).into()),
                                                wavy: st.underline_wave,
                                            }),
                                            strikethrough: st.strike.then(|| StrikethroughStyle {
                                                thickness: px(1.0),
                                                color: st
                                                    .strike_color
                                                    .map(|color| rgb(color_rgb(color)).into()),
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
                            paint_face_boxes(
                                &grid,
                                y,
                                line,
                                bounds,
                                line_height,
                                default_background,
                                window,
                            );
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
                        for bar in &grid.scroll_bars {
                            let track_origin = point(
                                bounds.left() + px(bar.left as f32),
                                bounds.top() + px(bar.top as f32),
                            );
                            let track_size = size(px(bar.width as f32), px(bar.height as f32));
                            if bar.width == 0 || bar.height == 0 {
                                continue;
                            }
                            window.paint_quad(fill(
                                Bounds {
                                    origin: track_origin,
                                    size: track_size,
                                },
                                rgb(0x303438),
                            ));
                            let (thumb_origin, thumb_size) = match bar.orientation {
                                GridScrollBarOrientation::Vertical => (
                                    point(
                                        track_origin.x,
                                        track_origin.y + px(bar.thumb_start as f32),
                                    ),
                                    size(px(bar.width as f32), px(bar.thumb_size as f32)),
                                ),
                                GridScrollBarOrientation::Horizontal => (
                                    point(
                                        track_origin.x + px(bar.thumb_start as f32),
                                        track_origin.y,
                                    ),
                                    size(px(bar.thumb_size as f32), px(bar.height as f32)),
                                ),
                            };
                            window.paint_quad(fill(
                                Bounds {
                                    origin: thumb_origin,
                                    size: thumb_size,
                                },
                                rgb(0x747a80),
                            ));
                        }
                    },
                )
                .size_full(),
            )
            .children(
                images
                    .into_iter()
                    .map(|image| grid_image_element(image, cell_width, line_height)),
            )
            .children(help_echoes.into_iter().map(|region| {
                let text = region.text;
                div()
                    .absolute()
                    .left(cell_width * region.left as f32)
                    .top(line_height * region.row as f32)
                    .w(cell_width * region.width as f32)
                    .h(line_height)
                    .id((
                        "help-echo",
                        (region.row as u64).saturating_mul(1 << 32) + region.left as u64,
                    ))
                    .tooltip(move |_, cx| {
                        cx.new(|_| HelpTooltipView { text: text.clone() }).into()
                    })
            }))
            .when_some(explicit_tooltip, |this, text| {
                this.child(
                    div()
                        .absolute()
                        .left(px(12.0))
                        .top(px(12.0))
                        .bg(rgb(0x303438))
                        .child(text),
                )
            })
    }
}

fn gpui_menus(menus: &[GridMenu]) -> Vec<Menu> {
    menus
        .iter()
        .map(|menu| Menu {
            name: menu.label.clone().into(),
            items: menu.items.iter().filter_map(gpui_menu_item).collect(),
        })
        .collect()
}

fn gpui_menu_item(item: &GridMenuItem) -> Option<MenuItem> {
    if item.separator {
        return Some(MenuItem::separator());
    }
    if !item.items.is_empty() {
        return Some(MenuItem::submenu(Menu {
            name: item.label.clone().into(),
            items: item.items.iter().filter_map(gpui_menu_item).collect(),
        }));
    }
    item.command.as_ref().map(|command| {
        MenuItem::action(
            item.label.clone(),
            RunMenuCommand {
                command: command.clone(),
            },
        )
    })
}

fn grid_image_element(
    image: GridImage,
    cell_width: Pixels,
    line_height: Pixels,
) -> impl IntoElement {
    let source = match image.source {
        GridImageSource::File(path) => img(std::path::PathBuf::from(path)),
        GridImageSource::Data { format, bytes } => {
            let format = match format.as_str() {
                "png" => ImageFormat::Png,
                "jpeg" | "jpg" => ImageFormat::Jpeg,
                "webp" => ImageFormat::Webp,
                "gif" => ImageFormat::Gif,
                "svg" => ImageFormat::Svg,
                "bmp" => ImageFormat::Bmp,
                "tiff" | "tif" => ImageFormat::Tiff,
                _ => return div(),
            };
            img(std::sync::Arc::new(Image::from_bytes(format, bytes.to_vec())))
        }
    };
    let source = match (image.width, image.height) {
        (Some(width), Some(height)) => source.w(px(width as f32)).h(px(height as f32)),
        (Some(width), None) => source.w(px(width as f32)),
        (None, Some(height)) => source.h(px(height as f32)),
        (None, None) => source,
    };
    div()
        .absolute()
        .left(cell_width * image.column as f32)
        .top(line_height * image.row as f32)
        .child(source)
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
fn paint_face_boxes(
    grid: &Grid,
    row_index: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    default_background: u32,
    window: &mut Window,
) {
    let Some(text) = grid.rows.get(row_index) else {
        return;
    };
    let Some(styles) = grid
        .char_attrs
        .get(row_index)
        .filter(|styles| !styles.is_empty())
        .or_else(|| grid.attrs.get(row_index))
    else {
        return;
    };
    let mut current: Option<(BoxStyle, Option<CellColor>, Option<CellColor>, usize, usize)> = None;
    for (index, (byte, character)) in text.char_indices().enumerate() {
        let style = styles.get(index).copied().unwrap_or_default();
        let Some(box_style) = style.box_style else {
            if let Some((box_style, fg, bg, start, end)) = current.take() {
                paint_box_run(
                    line,
                    bounds,
                    row_index,
                    line_height,
                    box_style,
                    fg,
                    bg,
                    start,
                    end,
                    default_background,
                    window,
                );
            }
            continue;
        };
        let end = byte + character.len_utf8();
        match &mut current {
            Some((active, fg, bg, _, active_end))
                if *active == box_style && *fg == style.fg && *bg == style.bg =>
            {
                *active_end = end;
            }
            _ => {
                if let Some((box_style, fg, bg, start, end)) = current.take() {
                    paint_box_run(
                        line,
                        bounds,
                        row_index,
                        line_height,
                        box_style,
                        fg,
                        bg,
                        start,
                        end,
                        default_background,
                        window,
                    );
                }
                current = Some((box_style, style.fg, style.bg, byte, end));
            }
        }
    }
    if let Some((box_style, fg, bg, start, end)) = current {
        paint_box_run(
            line,
            bounds,
            row_index,
            line_height,
            box_style,
            fg,
            bg,
            start,
            end,
            default_background,
            window,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_box_run(
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    row_index: usize,
    line_height: Pixels,
    box_style: BoxStyle,
    foreground: Option<CellColor>,
    background: Option<CellColor>,
    start: usize,
    end: usize,
    default_background: u32,
    window: &mut Window,
) {
    let x1 = line.x_for_index(start);
    let x2 = line.x_for_index(end);
    let left = x1.min(x2);
    let right = x1.max(x2);
    if right <= left {
        return;
    }

    let default_foreground = CellColor(
        ((FG >> 16) & 0xff) as u8,
        ((FG >> 8) & 0xff) as u8,
        (FG & 0xff) as u8,
    );
    let default_background = CellColor(
        ((default_background >> 16) & 0xff) as u8,
        ((default_background >> 8) & 0xff) as u8,
        (default_background & 0xff) as u8,
    );
    let base = box_style.color.unwrap_or_else(|| match box_style.kind {
        BoxStyleKind::Flat => foreground.unwrap_or(default_foreground),
        BoxStyleKind::Released | BoxStyleKind::Pressed => background.unwrap_or(default_background),
    });
    let base = color_rgb(base);
    let (top_color, right_color, bottom_color, left_color) = match box_style.kind {
        BoxStyleKind::Flat => (base, base, base, base),
        BoxStyleKind::Released => (
            shade_color(base, 1.35),
            shade_color(base, 0.65),
            shade_color(base, 0.65),
            shade_color(base, 1.35),
        ),
        BoxStyleKind::Pressed => (
            shade_color(base, 0.65),
            shade_color(base, 1.35),
            shade_color(base, 1.35),
            shade_color(base, 0.65),
        ),
    };
    let vertical = px(f32::from(box_style.vertical_width.unsigned_abs()));
    let horizontal = px(f32::from(box_style.horizontal_width.unsigned_abs()));
    let origin_y = bounds.top() + line_height * row_index as f32;
    let left_x = bounds.left() + left;
    let right_x = bounds.left() + right;
    let vertical_inset = box_style.vertical_width < 0;
    let horizontal_inset = box_style.horizontal_width < 0;
    let (left_x, right_x) = if vertical_inset {
        (left_x, right_x - vertical)
    } else {
        (left_x - vertical / 2.0, right_x - vertical / 2.0)
    };
    let (top_y, bottom_y) = if horizontal_inset {
        (origin_y, origin_y + line_height - horizontal)
    } else {
        (
            origin_y - horizontal / 2.0,
            origin_y + line_height - horizontal / 2.0,
        )
    };

    let mut paint_rect = |x: Pixels, y: Pixels, width: Pixels, height: Pixels, color: u32| {
        if width > px(0.0) && height > px(0.0) {
            window.paint_quad(fill(
                Bounds {
                    origin: point(x, y),
                    size: size(width, height),
                },
                rgb(color),
            ));
        }
    };
    paint_rect(
        left_x,
        top_y,
        right_x - left_x + vertical,
        horizontal,
        top_color,
    );
    paint_rect(
        left_x,
        bottom_y,
        right_x - left_x + vertical,
        horizontal,
        bottom_color,
    );
    paint_rect(
        left_x,
        top_y,
        vertical,
        bottom_y - top_y + horizontal,
        left_color,
    );
    paint_rect(
        right_x,
        top_y,
        vertical,
        bottom_y - top_y + horizontal,
        right_color,
    );
}

fn color_rgb(color: CellColor) -> u32 {
    ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32
}

fn shade_color(color: u32, factor: f32) -> u32 {
    let shade = |shift: u32| (((color >> shift) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
    (shade(16) << 16) | (shade(8) << 8) | shade(0)
}

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
