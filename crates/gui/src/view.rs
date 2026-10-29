//! GPUI view construction and painting of frontend-neutral grids.
use super::{
    keys::{
        ModifierMapping, PhysicalModifierState, command_keystroke_with_mapping,
        keystroke_to_code_with_mapping, modifier_state_after_change, mouse_modifier_bits,
        pass_text_to_os, unicode_key_text,
    },
    logic::{COLOR_DROP_PREFIX, FrameEventSender, GuiEvent, GuiGeometry},
    monitor::monitor_geometry,
};
use gpui::*;
use gpui::prelude::{FluentBuilder, InteractiveElement, StatefulInteractiveElement};
use remacs_runtime::frontend::{
    BoxStyle, BoxStyleKind, CellColor, Grid, GridBidiDirection, GridImage, GridImageSource,
    GridMenu, GridMenuItem, GridPointerShape, GridPopupMenuItem, GridScrollBar,
    GridScrollBarOrientation, GridStipple, GridTabBarItem, GridToolBarItem, GridVScroll,
    MouseButton as RemacsMouseButton,
};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
    mpsc::Sender,
};
use std::{cell::Cell, rc::Rc};
mod input;
use input::TextInputState;

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
const CURSOR: u32 = 0xc5c8c6;

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 19.0;

static FONT_FAMILY_NAMES: OnceLock<HashSet<String>> = OnceLock::new();

pub(super) fn font_family(window: &mut Window, requested: Option<String>) -> String {
    let available = FONT_FAMILY_NAMES.get_or_init(|| {
        window
            .text_system()
            .all_font_names()
            .into_iter()
            .map(|family| family.to_lowercase())
            .collect()
    });
    if let Some(requested) = requested
        && available.contains(&requested.to_lowercase())
    {
        return requested;
    }

    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["Menlo", "SF Mono", "Monaco", "Courier New", "Lilex", "DejaVu Sans Mono"]
    } else if cfg!(target_os = "windows") {
        &[
            "Consolas",
            "Cascadia Mono",
            "Lucida Console",
            "Courier New",
            "Lilex",
            "DejaVu Sans Mono",
        ]
    } else {
        &[
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Noto Sans Mono",
            "Ubuntu Mono",
            "Cousine",
            "FreeMono",
            "Lilex",
        ]
    };
    candidates
        .iter()
        .find(|family| available.contains(&family.to_lowercase()))
        .map(|family| (*family).to_owned())
        .unwrap_or_else(|| "monospace".to_owned())
}
const BIDI_PARAGRAPH_LTR_OPEN: &str = "\u{2066}";
const BIDI_PARAGRAPH_RTL_OPEN: &str = "\u{2067}";
const BIDI_PARAGRAPH_CLOSE: &str = "\u{2069}";
const BIDI_DISABLED_OPEN: &str = "\u{2066}\u{202D}";
const BIDI_DISABLED_CLOSE: &str = "\u{202C}\u{2069}";

fn append_styled_piece(
    text: &mut String,
    spans: &mut Vec<(usize, remacs_runtime::frontend::CellStyle)>,
    piece: &str,
    style: remacs_runtime::frontend::CellStyle,
) {
    text.push_str(piece);
    if let Some((len, previous_style)) = spans.last_mut()
        && *previous_style == style
    {
        *len += piece.len();
    } else if !piece.is_empty() {
        spans.push((piece.len(), style));
    }
}

fn grid_layout_index(grid: &Grid, row: usize, byte_index: usize) -> usize {
    let mut index = byte_index;
    for range in grid
        .bidi_paragraph_ranges
        .iter()
        .filter(|range| range.row == row)
    {
        if range.start <= byte_index {
            index += match range.direction {
                GridBidiDirection::LeftToRight => BIDI_PARAGRAPH_LTR_OPEN.len(),
                GridBidiDirection::RightToLeft => BIDI_PARAGRAPH_RTL_OPEN.len(),
            };
        }
        if range.end <= byte_index {
            index += BIDI_PARAGRAPH_CLOSE.len();
        }
    }
    for range in grid
        .bidi_disabled_ranges
        .iter()
        .filter(|range| range.row == row)
    {
        if range.start <= byte_index {
            index += BIDI_DISABLED_OPEN.len();
        }
        if range.end <= byte_index {
            index += BIDI_DISABLED_CLOSE.len();
        }
    }
    index
}

fn grid_pixelwise_x_offset(grid: &Grid, row: usize, byte_index: usize) -> i128 {
    grid.pixelwise_segments
        .iter()
        .find(|segment| {
            segment.row == row && segment.start <= byte_index && byte_index < segment.end
        })
        .or_else(|| {
            grid.pixelwise_segments
                .iter()
                .find(|segment| segment.row == row && segment.end == byte_index)
        })
        .map_or(0, |segment| segment.x_offset)
}

fn grid_line_x(grid: &Grid, row: usize, line: &ShapedLine, byte_index: usize) -> Pixels {
    line.x_for_index(grid_layout_index(grid, row, byte_index))
        + px(grid_pixelwise_x_offset(grid, row, byte_index) as f32)
}

pub(super) fn grid_row_top(grid: &Grid, row: usize, fallback: Pixels) -> Pixels {
    row_top_from_heights(
        &grid.row_heights,
        grid.default_line_height,
        row,
        fallback,
    )
}

pub(super) fn grid_row_height(grid: &Grid, row: usize, fallback: Pixels) -> Pixels {
    row_height_from_heights(
        &grid.row_heights,
        grid.default_line_height,
        row,
        fallback,
    )
}

fn adjusted_row_height(height: usize, default_line_height: usize, fallback: Pixels) -> Pixels {
    let adjustment = height as i128 - default_line_height.max(1) as i128;
    (fallback + px(adjustment as f32)).max(px(1.0))
}

fn row_top_from_heights(
    row_heights: &[usize],
    default_line_height: usize,
    row: usize,
    fallback: Pixels,
) -> Pixels {
    if row_heights.is_empty() {
        return fallback * row as f32;
    }
    let height = row_heights
        .iter()
        .take(row)
        .map(|height| adjusted_row_height(*height, default_line_height, fallback))
        .fold(px(0.0), |total, height| total + height);
    height
}

fn row_height_from_heights(
    row_heights: &[usize],
    default_line_height: usize,
    row: usize,
    fallback: Pixels,
) -> Pixels {
    row_heights
        .get(row)
        .copied()
        .map(|height| adjusted_row_height(height, default_line_height, fallback))
        .unwrap_or(fallback)
}

fn rows_fitting_viewport(grid: &Grid, height: Pixels, fallback: Pixels) -> usize {
    let available = f32::from(height);
    let default_height = f32::from(adjusted_row_height(
        grid.default_row_height,
        grid.default_line_height,
        fallback,
    ));
    let mut used = 0.0;
    let mut rows = 0;
    loop {
        let row_height = grid
            .row_heights
            .get(rows)
            .copied()
            .map(|height| {
                f32::from(adjusted_row_height(
                    height,
                    grid.default_line_height,
                    fallback,
                ))
            })
            .unwrap_or_else(|| {
                if grid.row_heights.is_empty() {
                    f32::from(fallback).max(1.0)
                } else {
                    default_height
                }
            });
        if used + row_height > available {
            break;
        }
        used += row_height;
        rows += 1;
    }
    rows.max(3)
}

fn vscroll_pixels_for_cell(grid: &Grid, row: usize, column: usize) -> usize {
    grid.vscrolls
        .iter()
        .find(|scroll| {
            row >= scroll.top
                && row < scroll.top.saturating_add(scroll.rows)
                && column >= scroll.left
                && column < scroll.left.saturating_add(scroll.width)
        })
        .map_or(0, |scroll| scroll.pixels)
}

fn vscroll_bounds_for_cell(
    grid: &Grid,
    row: usize,
    column: usize,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> Option<Bounds<Pixels>> {
    let scroll = grid.vscrolls.iter().find(|scroll| {
        row >= scroll.top
            && row < scroll.top.saturating_add(scroll.rows)
            && column >= scroll.left
            && column < scroll.left.saturating_add(scroll.width)
    })?;
    let first_row = scroll.top.min(grid.rows.len());
    let end_row = scroll.top.saturating_add(scroll.rows).min(grid.rows.len());
    if first_row >= end_row {
        return None;
    }
    let top = bounds.top() + grid_row_top(grid, first_row, line_height);
    let bottom = bounds.top() + grid_row_top(grid, end_row, line_height);
    Some(Bounds {
        origin: point(bounds.left() + cell_width * scroll.left as f32, top),
        size: size(cell_width * scroll.width as f32, bottom - top),
    })
}

fn with_vscroll_cell_clip<R>(
    grid: &Grid,
    row: usize,
    column: usize,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    window: &mut Window,
    paint: impl FnOnce(&mut Window) -> R,
) -> R {
    match vscroll_bounds_for_cell(grid, row, column, bounds, cell_width, line_height) {
        Some(bounds) => window.with_content_mask(Some(ContentMask { bounds }), paint),
        None => paint(window),
    }
}

fn unscrolled_row_clips(
    grid: &Grid,
    row: usize,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> Option<Vec<Bounds<Pixels>>> {
    let width = grid.attrs.get(row)?.len();
    let mut covered = grid
        .vscrolls
        .iter()
        .filter(|scroll| {
            row >= scroll.top && row < scroll.top.saturating_add(scroll.rows)
        })
        .filter_map(|scroll| {
            let left = scroll.left.min(width);
            let right = scroll.left.saturating_add(scroll.width).min(width);
            (left < right).then_some((left, right))
        })
        .collect::<Vec<_>>();
    if covered.is_empty() {
        return None;
    }
    covered.sort_unstable_by_key(|(left, _)| *left);

    let row_top = bounds.top() + grid_row_top(grid, row, line_height);
    let row_bottom = row_top + grid_row_height(grid, row, line_height);
    let mut clips = Vec::new();
    let mut left = 0;
    for (covered_left, covered_right) in covered {
        if covered_left > left {
            clips.push(Bounds {
                origin: point(bounds.left() + cell_width * left as f32, row_top),
                size: size(cell_width * (covered_left - left) as f32, row_bottom - row_top),
            });
        }
        left = left.max(covered_right);
    }
    if left < width {
        clips.push(Bounds {
            origin: point(bounds.left() + cell_width * left as f32, row_top),
            size: size(cell_width * (width - left) as f32, row_bottom - row_top),
        });
    }
    Some(clips)
}

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

fn frame_content_point(
    position: Point<Pixels>,
    viewport: Size<Pixels>,
    border: Pixels,
) -> Option<Point<Pixels>> {
    let border = f32::from(border);
    let x = f32::from(position.x);
    let y = f32::from(position.y);
    let right = f32::from(viewport.width) - border;
    let bottom = f32::from(viewport.height) - border;
    if x < border || y < border || x >= right || y >= bottom {
        return None;
    }
    Some(point(px(x - border), px(y - border)))
}

fn absolute_mouse_pixel_position(
    position: Point<Pixels>,
    screen_left: i128,
    screen_top: i128,
    content_top_offset: i128,
) -> (i128, i128) {
    (
        screen_left.saturating_add(f32::from(position.x).round() as i128),
        screen_top
            .saturating_add(content_top_offset)
            .saturating_add(f32::from(position.y).round() as i128),
    )
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
    tx: &FrameEventSender,
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

fn gpui_cursor_style(shape: Option<GridPointerShape>) -> CursorStyle {
    match shape {
        Some(GridPointerShape::Text) => CursorStyle::IBeam,
        Some(GridPointerShape::Hand) => CursorStyle::PointingHand,
        Some(GridPointerShape::ResizeHorizontal) => CursorStyle::ResizeLeftRight,
        Some(GridPointerShape::ResizeVertical) => CursorStyle::ResizeUpDown,
        Some(GridPointerShape::Arrow) | None => CursorStyle::Arrow,
    }
}

fn file_drop_uri(path: &std::path::Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut uri = String::from("file://");
    for byte in absolute.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~' | b':')
        {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
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

fn popup_menu_command_at(
    items: &[GridPopupMenuItem],
    point: Point<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
) -> Option<Option<String>> {
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let row = (f32::from(point.y) / f32::from(line_height)).floor().max(0.0) as usize;
    items
        .iter()
        .find(|item| item.row == row && column >= item.left && column < item.left + item.width)
        .map(|item| item.command.clone())
}

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = remacs, no_json)]
struct RunMenuCommand {
    command: String,
}

pub(super) struct EditorView {
    tx: FrameEventSender,
    image_cache_entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_size: Arc<AtomicUsize>,
    pub(super) focus: FocusHandle,
    pub(super) grid: Grid,
    modifier_mapping: ModifierMapping,
    internal_border_width: usize,
    alpha_background: u8,
    appearance: WindowAppearance,
    physical_modifier_state: Rc<Cell<PhysicalModifierState>>,
    installed_menus: Vec<GridMenu>,
    window_title: Option<String>,
    pub(super) cols: usize,
    rows: usize,
    input: TextInputState,
    cell_width: Pixels,
    line_height: Pixels,
    resize_increments: Option<(f32, f32)>,
    geometry: Option<GuiGeometry>,
    runtime_frame_size: Option<(usize, usize, usize, usize)>,
    runtime_frame_position: Option<(Option<i128>, Option<i128>)>,
    scroll_bar_drag: Option<ScrollBarDrag>,
    mouse_down_position: Option<remacs_runtime::frontend::MousePosition>,
    toolbar_pressed: bool,
    bell_flash: bool,
    bell_flash_generation: u64,
    focus_subscriptions: Vec<Subscription>,
    appearance_subscription: Option<Subscription>,
    mouse_face_position: Rc<Cell<Option<(usize, usize)>>>,
    no_accept_focus: Rc<Cell<bool>>,
    auto_raise: Rc<Cell<bool>>,
    auto_lower: Rc<Cell<bool>>,
    last_window_active: Option<bool>,
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
        Self::new_for_frame(tx, None, cx)
    }

    pub(super) fn new_for_frame(
        tx: Sender<GuiEvent>,
        frame_id: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Self {
        EditorView {
            tx: FrameEventSender::new(tx, frame_id),
            image_cache_entries: Arc::new(Mutex::new(HashMap::new())),
            image_cache_size: Arc::new(AtomicUsize::new(0)),
            focus: cx.focus_handle(),
            grid: Grid::blank(0, 0),
            modifier_mapping: ModifierMapping::default(),
            internal_border_width: 0,
            alpha_background: 100,
            appearance: WindowAppearance::Dark,
            physical_modifier_state: Rc::new(Cell::new(PhysicalModifierState::default())),
            installed_menus: Vec::new(),
            window_title: None,
            cols: 80,
            rows: 24,
            input: TextInputState::default(),
            cell_width: px(8.0),
            line_height: px(LINE_HEIGHT),
            resize_increments: None,
            geometry: None,
            runtime_frame_size: None,
            runtime_frame_position: None,
            scroll_bar_drag: None,
            mouse_down_position: None,
            toolbar_pressed: false,
            bell_flash: false,
            bell_flash_generation: 0,
            focus_subscriptions: Vec::new(),
            appearance_subscription: None,
            mouse_face_position: Rc::new(Cell::new(None)),
            no_accept_focus: Rc::new(Cell::new(false)),
            auto_raise: Rc::new(Cell::new(false)),
            auto_lower: Rc::new(Cell::new(false)),
            last_window_active: None,
            #[cfg(target_os = "macos")]
            accessibility_buffer_view: None,
            #[cfg(target_os = "macos")]
            accessibility_screen_view: None,
            #[cfg(target_os = "macos")]
            accessibility_snapshot: None,
        }
    }

    pub(super) fn set_frame_id(&mut self, frame_id: usize) {
        self.tx.set_frame_id(frame_id);
        self.resize_increments = None;
        self.mouse_face_position.set(None);
        self.geometry = None;
        self.runtime_frame_size = None;
        self.runtime_frame_position = None;
    }

    pub(super) fn set_image_cache_metrics(
        &mut self,
        entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
        size: Arc<AtomicUsize>,
    ) {
        self.image_cache_entries = entries;
        self.image_cache_size = size;
    }

    pub(super) fn set_modifier_mapping(
        &mut self,
        mapping: ModifierMapping,
    ) -> bool {
        if self.modifier_mapping == mapping {
            return false;
        }
        self.modifier_mapping = mapping;
        true
    }

    pub(super) fn set_internal_border_width(&mut self, width: usize) -> bool {
        if self.internal_border_width == width {
            return false;
        }
        self.internal_border_width = width;
        true
    }

    pub(super) fn set_alpha_background(&mut self, alpha: u8) -> bool {
        if self.alpha_background == alpha {
            return false;
        }
        self.alpha_background = alpha;
        true
    }

    pub(super) fn set_appearance(&mut self, appearance: WindowAppearance) -> bool {
        if self.appearance == appearance {
            return false;
        }
        self.appearance = appearance;
        true
    }

    pub(super) fn native_position(&self) -> Option<(i128, i128)> {
        self.geometry
            .as_ref()
            .map(|geometry| (geometry.frame_left, geometry.frame_top))
    }

    pub(super) fn needs_frame_size_update(&mut self, width: usize, height: usize) -> bool {
        let extra_height = self
            .grid
            .row_heights
            .iter()
            .map(|row_height| row_height.saturating_sub(self.grid.default_line_height.max(1)))
            .sum();
        let size = (
            width.max(10),
            height.max(3),
            self.internal_border_width,
            extra_height,
        );
        let changed = self.runtime_frame_size != Some(size);
        self.runtime_frame_size = Some(size);
        changed
    }

    pub(super) fn needs_frame_position_update(
        &mut self,
        left: Option<i128>,
        top: Option<i128>,
    ) -> bool {
        let position = (left, top);
        let changed = self.runtime_frame_position != Some(position);
        self.runtime_frame_position = Some(position);
        changed
    }

    pub(super) fn resize_to_cells(
        &mut self,
        width: usize,
        height: usize,
        window: &mut Window,
    ) {
        let width = width.max(10);
        let height = height.max(3);
        let default_style = self.grid.default_style;
        let requested_family = default_style
            .font_family
            .and_then(|id| self.grid.font_families.get(id as usize))
            .cloned();
        let family = font_family(window, requested_family);
        let mut text_font = font(family);
        if let Some(weight) = default_style.font_weight {
            text_font.weight = FontWeight(weight as f32);
        }
        text_font.stretch = FontStretch::from_percentage(default_style.font_width.unwrap_or(100));
        let font_size = default_style
            .font_height
            .map(|height| px(height as f32 / 10.0))
            .unwrap_or(px(FONT_SIZE));
        let font_id = window.text_system().resolve_font(&text_font);
        self.cell_width = window
            .text_system()
            .advance(font_id, font_size, '0')
            .map(|metrics| metrics.width)
            .unwrap_or(px(8.0));
        self.line_height = px(f32::from(font_size) * (LINE_HEIGHT / FONT_SIZE));
        let viewport = window.viewport_size();
        let border = self.internal_border_width as f32 * 2.0;
        let pixel_height = (0..height)
            .map(|row| f32::from(grid_row_height(&self.grid, row, px(self.grid.default_row_height.max(1) as f32))))
            .sum::<f32>();
        let desired_width = self.cell_width * width as f32 + px(border);
        let desired_height = px(pixel_height + border);
        if (f32::from(viewport.width) - f32::from(desired_width)).abs() < 1.0
            && (f32::from(viewport.height) - f32::from(desired_height)).abs() < 1.0
        {
            return;
        }
        window.resize(size(desired_width, desired_height));
    }

    pub(super) fn set_frame_behavior(&self, auto_raise: bool, auto_lower: bool) {
        self.auto_raise.set(auto_raise);
        self.auto_lower.set(auto_lower);
    }

    pub(super) fn set_no_accept_focus(
        &mut self,
        window: &mut Window,
        no_accept_focus: bool,
    ) -> bool {
        if self.no_accept_focus.replace(no_accept_focus) == no_accept_focus {
            return false;
        }
        if no_accept_focus {
            window.blur();
        } else if window.is_window_active() {
            self.focus.focus(window);
        }
        true
    }

    pub(super) fn flash_bell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bell_flash_generation = self.bell_flash_generation.wrapping_add(1);
        let generation = self.bell_flash_generation;
        self.bell_flash = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(140))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.bell_flash_generation == generation {
                    view.bell_flash = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = window.is_window_active();
        if let Some(frame_id) = self.tx.frame_id() {
            if self.last_window_active != Some(active) {
                if active && self.auto_raise.get() {
                    crate::raise_frame_window(window);
                } else if !active && self.auto_lower.get() {
                    crate::lower_frame_window(window);
                }
                let _ = self.tx.send_focus_update(frame_id, active);
                self.last_window_active = Some(active);
            }
        } else {
            self.last_window_active = None;
        }
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
        let requested_family = default_style
            .font_family
            .and_then(|id| font_families.get(id as usize))
            .cloned();
        let family = font_family(window, requested_family);
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
        self.line_height = line_height;
        let font_id = window.text_system().resolve_font(&text_font);
        let cell_width = window
            .text_system()
            .advance(font_id, font_size, '0')
            .map(|s| s.width)
            .unwrap_or(px(8.0));
        self.cell_width = cell_width;
        let resize_increments = if self.grid.frame_resize_pixelwise {
            (1.0, 1.0)
        } else {
            (
                f32::from(cell_width).max(1.0),
                f32::from(line_height).max(1.0),
            )
        };
        if self.resize_increments != Some(resize_increments) {
            window.set_resize_increments(size(
                px(resize_increments.0),
                px(resize_increments.1),
            ));
            self.resize_increments = Some(resize_increments);
        }

        // Report grid-size changes to the logic thread.
        let vp = window.viewport_size();
        let border = px(self.internal_border_width as f32);
        let border_total = border * 2.0;
        let content_width = px((f32::from(vp.width) - f32::from(border_total)).max(0.0));
        let content_height = px((f32::from(vp.height) - f32::from(border_total)).max(0.0));
        let cols = (f32::from(content_width) / f32::from(cell_width)) as usize;
        let rows = rows_fitting_viewport(&self.grid, content_height, line_height);
        let cols = cols.max(10);
        let rows = rows.max(3);
        if (cols, rows) != (self.cols, self.rows) {
            self.cols = cols;
            self.rows = rows;
            if self.tx.frame_id().is_some() {
                let _ = self.tx.send(GuiEvent::Resize(cols, rows));
            }
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
        let pointer_screen_left = frame_left;
        let pointer_screen_top = frame_top;
        let content_top_offset =
            (f32::from(bounds.size.height) - f32::from(vp.height)).round() as i128;
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
            if self.tx.frame_id().is_some() {
                let _ = self.tx.send(GuiEvent::Geometry(geometry));
            }
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

        let default_background = self.grid.default_bg.map_or_else(
            || {
                gamma_correct_rgb(
                    match self.appearance {
                        WindowAppearance::Light | WindowAppearance::VibrantLight => 0xf5f5f5,
                        WindowAppearance::Dark | WindowAppearance::VibrantDark => BG,
                    },
                    self.grid.gamma_exponent,
                )
            },
            |color| ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32,
        );
        let background_alpha = ((self.alpha_background as u32 * 255 + 50) / 100) as u8;
        let default_background_fill = rgba((default_background << 8) | u32::from(background_alpha));
        let internal_border_background = self.grid.internal_border_style.bg.map_or(
            default_background,
            |color| ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32,
        );
        let internal_border_fill = rgba(
            (internal_border_background << 8) | u32::from(background_alpha),
        );
        let modifier_mapping = self.modifier_mapping;
        let physical_modifier_state = self.physical_modifier_state.clone();
        let key_modifier_state = physical_modifier_state.clone();
        let bell_flash = self.bell_flash;
        let mouse_cursor = if self.grid.pointer_visible {
            gpui_cursor_style(self.grid.hover_cursor)
        } else {
            CursorStyle::None
        };
        let grid = self.grid.clone();
        let images = grid.images.clone();
        let image_cache_entries = self.image_cache_entries.clone();
        let image_cache_size = self.image_cache_size.clone();
        let image_vscrolls = grid.vscrolls.clone();
        let help_echoes = grid.help_echoes.clone();
        let help_vscrolls = grid.vscrolls.clone();
        let help_row_heights = grid.row_heights.clone();
        let help_default_line_height = grid.default_line_height;
        let image_row_heights = grid.row_heights.clone();
        let image_default_line_height = grid.default_line_height;
        let explicit_tooltip = grid.tooltip.clone();
        let tx = self.tx.clone();
        let mouse_grid = grid.clone();
        let scroll_grid = grid.clone();
        let scroll_tx = self.tx.clone();
        let scroll_bar_tx = self.tx.clone();
        let file_drop_tx = self.tx.clone();
        let text_drop_tx = self.tx.clone();
        let external_text_drop_tx = self.tx.clone();
        let left_mouse_down_tx = self.tx.clone();
        let left_mouse_down_grid = mouse_grid.clone();
        let left_mouse_down_modifiers = physical_modifier_state.clone();
        let left_mouse_up_tx = self.tx.clone();
        let left_mouse_up_grid = mouse_grid.clone();
        let left_mouse_up_modifiers = physical_modifier_state.clone();
        let middle_mouse_down_tx = self.tx.clone();
        let middle_mouse_down_grid = mouse_grid.clone();
        let middle_mouse_down_modifiers = physical_modifier_state.clone();
        let middle_mouse_up_tx = self.tx.clone();
        let middle_mouse_up_grid = mouse_grid;
        let middle_mouse_up_modifiers = physical_modifier_state.clone();
        let right_mouse_down_tx = self.tx.clone();
        let right_mouse_down_grid = scroll_grid.clone();
        let right_mouse_down_modifiers = physical_modifier_state.clone();
        let right_mouse_up_tx = self.tx.clone();
        let right_mouse_up_grid = scroll_grid.clone();
        let right_mouse_up_modifiers = physical_modifier_state.clone();
        let scroll_modifiers = physical_modifier_state.clone();
        let entity = cx.entity();
        let focus = self.focus.clone();
        let marked = self.input.marked.clone();
        let marked_selection = self.input.selection_byte_range();
        let marked_cursor = self.input.selection_byte_end();
        let marked_origin = self
            .input
            .replacement_range
            .as_ref()
            .and_then(|range| input::visible_input_position(&self.grid, range.start, false));
        let composition_font = text_font.clone();
        let mouse_face_exit_position = self.mouse_face_position.clone();
        let mouse_face_exit_tx = self.tx.clone();
        div()
            .size_full()
            .bg(default_background_fill)
            .border(border)
            .border_color(internal_border_fill)
            .cursor(mouse_cursor)
            .track_focus(&self.focus)
            .key_context("remacs")
            .id("remacs-editor-drop-target")
            .on_drop(cx.listener(
                move |this, paths: &ExternalPaths, window, cx| {
                    let urls = paths
                        .paths()
                        .iter()
                        .map(|path| file_drop_uri(path))
                        .collect::<Vec<_>>();
                    let position = frame_content_point(
                        window.mouse_position(),
                        window.viewport_size(),
                        border,
                    )
                    .and_then(|point| {
                        input::mouse_position_for_point_with_cell_width(
                            &this.grid,
                            point,
                            cell_width,
                            window,
                        )
                    });
                    let _ = file_drop_tx.send(GuiEvent::Drop {
                        position,
                        urls,
                        action: "copy".to_owned(),
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                },
            ))
            .on_drop(cx.listener(
                move |this, text: &String, window, cx| {
                    let position = frame_content_point(
                        window.mouse_position(),
                        window.viewport_size(),
                        border,
                    )
                    .and_then(|point| {
                        input::mouse_position_for_point_with_cell_width(
                            &this.grid,
                            point,
                            cell_width,
                            window,
                        )
                    });
                    let _ = text_drop_tx.send(GuiEvent::Drop {
                        position,
                        urls: vec![text.clone()],
                        action: "copy".to_owned(),
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                },
            ))
            .on_drop(cx.listener(
                move |this, text: &ExternalText, window, cx| {
                    let position = frame_content_point(
                        window.mouse_position(),
                        window.viewport_size(),
                        border,
                    )
                    .and_then(|point| {
                        input::mouse_position_for_point_with_cell_width(
                            &this.grid,
                            point,
                            cell_width,
                            window,
                        )
                    });
                    if let Some((kind, color)) = text
                        .0
                        .strip_prefix(COLOR_DROP_PREFIX)
                        .and_then(|color| color.split_once(':'))
                    {
                        let _ = external_text_drop_tx.send(GuiEvent::ColorDrop {
                            position,
                            color: color.to_owned(),
                            background: kind == "background",
                        });
                    } else {
                        let _ = external_text_drop_tx.send(GuiEvent::Drop {
                            position,
                            urls: vec![text.0.clone()],
                            action: "copy".to_owned(),
                        });
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                },
            ))
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
                        this.mouse_down_position = None;
                        let Some(point) = frame_content_point(
                            event.position,
                            window.viewport_size(),
                            border,
                        ) else {
                            return;
                        };
                        let absolute_position = absolute_mouse_pixel_position(
                            event.position,
                            pointer_screen_left,
                            pointer_screen_top,
                            content_top_offset,
                        );
                        let _ = this.tx.send_mouse_move(None, Some(absolute_position));
                        if let Some(command) = popup_menu_command_at(
                            &left_mouse_down_grid.popup_menu_items,
                            point,
                            cell_width,
                            line_height,
                        ) {
                            this.toolbar_pressed = true;
                            let command = command.unwrap_or_else(|| "ignore".to_owned());
                            let _ = left_mouse_down_tx.send(GuiEvent::MenuCommand(command));
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(command) = tool_bar_command_at(
                            &left_mouse_down_grid.tool_bar_items,
                            point,
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
                            point,
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
                            scroll_bar_hit(&left_mouse_down_grid, point)
                        {
                            this.scroll_bar_drag = Some(ScrollBarDrag {
                                window_id: bar.window_id,
                                orientation: bar.orientation,
                                pointer_offset,
                            });
                            send_scroll_bar_event(
                                &left_mouse_down_tx,
                                &bar,
                                point,
                                pointer_offset,
                            );
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(position) = input::mouse_position_for_point_with_cell_width(
                            &left_mouse_down_grid,
                            point,
                            cell_width,
                            window,
                        )
                        .or_else(|| {
                            input::mode_line_mouse_position(
                                &left_mouse_down_grid,
                                point,
                                cell_width,
                            )
                        }) {
                            this.mouse_down_position = Some(position);
                            let _ = left_mouse_down_tx.send(GuiEvent::MouseDown {
                                button: RemacsMouseButton::Left,
                                click_count: event.click_count,
                                modifiers: mouse_modifier_bits(
                                    &event.modifiers,
                                    modifier_mapping,
                                    left_mouse_down_modifiers.get(),
                                ),
                                position,
                            });
                        }
                    },
                ),
            )
            .on_mouse_move(cx.listener(
                move |this, event: &MouseMoveEvent, window, cx| {
                    let absolute_position = absolute_mouse_pixel_position(
                        event.position,
                        pointer_screen_left,
                        pointer_screen_top,
                        content_top_offset,
                    );
                    let point = frame_content_point(
                        event.position,
                        window.viewport_size(),
                        border,
                    );
                    let position = point.and_then(|point| {
                        input::mouse_position_for_text_point_with_cell_width(
                            &this.grid,
                            point,
                            cell_width,
                            window,
                        )
                    });
                    let target = position.map(|position| {
                        (position.buffer_id, position.buffer_position)
                    });
                    let _ = this.mouse_face_position.replace(target);
                    let _ = this
                        .tx
                        .send_mouse_move(position, Some(absolute_position));
                    if !event.dragging() {
                        return;
                    }
                    if let Some(drag) = this.scroll_bar_drag {
                        let Some(point) = point else {
                            return;
                        };
                        if let Some(bar) = this.grid.scroll_bars.iter().find(|bar| {
                            bar.window_id == drag.window_id
                                && bar.orientation == drag.orientation
                        }) {
                            send_scroll_bar_event(
                                &scroll_bar_tx,
                                bar,
                                point,
                                drag.pointer_offset,
                            );
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        return;
                    }
                    let _ = this.tx.send_mouse_drag_move(position);
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
                    let point = frame_content_point(
                        event.position,
                        window.viewport_size(),
                        border,
                    );
                    let position = point
                        .and_then(|point| {
                            input::mouse_position_for_point_with_cell_width(
                                &left_mouse_up_grid,
                                point,
                                cell_width,
                                window,
                            )
                            .or_else(|| {
                                input::mode_line_mouse_position(
                                    &left_mouse_up_grid,
                                    point,
                                    cell_width,
                                )
                            })
                        })
                        .or_else(|| {
                            this.mouse_down_position.map(|mut position| {
                                position.x = (f32::from(event.position.x) - f32::from(border))
                                    .round() as i128;
                                position.y = (f32::from(event.position.y) - f32::from(border))
                                    .round() as i128;
                                position
                            })
                        });
                    this.mouse_down_position = None;
                    if let Some(position) = position {
                        let _ = left_mouse_up_tx.send(GuiEvent::MouseUp {
                            button: RemacsMouseButton::Left,
                            click_count: event.click_count,
                            modifiers: mouse_modifier_bits(
                                &event.modifiers,
                                modifier_mapping,
                                left_mouse_up_modifiers.get(),
                            ),
                            position,
                        });
                    }
                }),
            )
            .on_mouse_down(MouseButton::Middle, move |event, window, _cx| {
                let point = frame_content_point(event.position, window.viewport_size(), border);
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &middle_mouse_down_grid,
                        point,
                        cell_width,
                        window,
                    )
                    .or_else(|| {
                        input::mode_line_mouse_position(
                            &middle_mouse_down_grid,
                            point,
                            cell_width,
                        )
                    })
                }) {
                    let _ = middle_mouse_down_tx.send(GuiEvent::MouseDown {
                        button: RemacsMouseButton::Middle,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(
                            &event.modifiers,
                            modifier_mapping,
                            middle_mouse_down_modifiers.get(),
                        ),
                        position,
                    });
                }
            })
            .on_mouse_up(MouseButton::Middle, move |event, window, _cx| {
                let point = frame_content_point(event.position, window.viewport_size(), border);
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &middle_mouse_up_grid,
                        point,
                        cell_width,
                        window,
                    )
                    .or_else(|| {
                        input::mode_line_mouse_position(
                            &middle_mouse_up_grid,
                            point,
                            cell_width,
                        )
                    })
                }) {
                    let _ = middle_mouse_up_tx.send(GuiEvent::MouseUp {
                        button: RemacsMouseButton::Middle,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(
                            &event.modifiers,
                            modifier_mapping,
                            middle_mouse_up_modifiers.get(),
                        ),
                        position,
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, move |event, window, _cx| {
                let point = frame_content_point(event.position, window.viewport_size(), border);
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &right_mouse_down_grid,
                        point,
                        cell_width,
                        window,
                    )
                    .or_else(|| {
                        input::mode_line_mouse_position(
                            &right_mouse_down_grid,
                            point,
                            cell_width,
                        )
                    })
                }) {
                    let _ = right_mouse_down_tx.send(GuiEvent::MouseDown {
                        button: RemacsMouseButton::Right,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(
                            &event.modifiers,
                            modifier_mapping,
                            right_mouse_down_modifiers.get(),
                        ),
                        position,
                    });
                }
            })
            .on_mouse_up(MouseButton::Right, move |event, window, _cx| {
                let point = frame_content_point(event.position, window.viewport_size(), border);
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &right_mouse_up_grid,
                        point,
                        cell_width,
                        window,
                    )
                    .or_else(|| {
                        input::mode_line_mouse_position(
                            &right_mouse_up_grid,
                            point,
                            cell_width,
                        )
                    })
                }) {
                    let _ = right_mouse_up_tx.send(GuiEvent::MouseUp {
                        button: RemacsMouseButton::Right,
                        click_count: event.click_count,
                        modifiers: mouse_modifier_bits(
                            &event.modifiers,
                            modifier_mapping,
                            right_mouse_up_modifiers.get(),
                        ),
                        position,
                    });
                }
            })
            .on_scroll_wheel(move |event, window, cx| {
                let point = frame_content_point(event.position, window.viewport_size(), border);
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &scroll_grid,
                        point,
                        cell_width,
                        window,
                    )
                }) {
                    let delta = event.delta.pixel_delta(line_height);
                    let delta_lines = -f32::from(delta.y) / f32::from(line_height);
                    if delta_lines != 0.0 {
                        let _ = scroll_tx.send(GuiEvent::Scroll {
                            delta_lines,
                            modifiers: mouse_modifier_bits(
                                &event.modifiers,
                                modifier_mapping,
                                scroll_modifiers.get(),
                            ),
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
                if command_keystroke_with_mapping(
                    &event.keystroke,
                    modifier_mapping,
                    key_modifier_state.get(),
                ) && let Some(code) = keystroke_to_code_with_mapping(
                    &event.keystroke,
                    modifier_mapping,
                    key_modifier_state.get(),
                )
                {
                    let _ = tx.send(GuiEvent::Key(code));
                    window.prevent_default();
                    cx.stop_propagation();
                } else if pass_text_to_os(
                    &event.keystroke,
                    modifier_mapping,
                    key_modifier_state.get(),
                ) && let Some(text) = unicode_key_text(&event.keystroke)
                {
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
                                let bidi_disabled_ranges = grid
                                    .bidi_disabled_ranges
                                    .iter()
                                    .filter(|range| range.row == y)
                                    .copied()
                                    .collect::<Vec<_>>();
                                let bidi_paragraph_ranges = grid
                                    .bidi_paragraph_ranges
                                    .iter()
                                    .filter(|range| range.row == y)
                                    .copied()
                                    .collect::<Vec<_>>();
                                let mut shaping_text = String::with_capacity(
                                    row.len()
                                        + bidi_disabled_ranges.len() * 12
                                        + bidi_paragraph_ranges.len() * 6,
                                );
                                let mut styled_spans = Vec::new();
                                let mut open_range = 0;
                                let mut close_range = 0;
                                let mut open_paragraph = 0;
                                let mut close_paragraph = 0;
                                for (x, (byte, ch)) in row.char_indices().enumerate() {
                                    let st =
                                        cells.and_then(|c| c.get(x)).copied().unwrap_or_default();
                                    while bidi_paragraph_ranges
                                        .get(open_paragraph)
                                        .is_some_and(|range| range.start == byte)
                                    {
                                        let marker = match bidi_paragraph_ranges[open_paragraph]
                                            .direction
                                        {
                                            GridBidiDirection::LeftToRight => {
                                                BIDI_PARAGRAPH_LTR_OPEN
                                            }
                                            GridBidiDirection::RightToLeft => {
                                                BIDI_PARAGRAPH_RTL_OPEN
                                            }
                                        };
                                        append_styled_piece(
                                            &mut shaping_text,
                                            &mut styled_spans,
                                            marker,
                                            st,
                                        );
                                        open_paragraph += 1;
                                    }
                                    while bidi_disabled_ranges
                                        .get(open_range)
                                        .is_some_and(|range| range.start == byte)
                                    {
                                        append_styled_piece(
                                            &mut shaping_text,
                                            &mut styled_spans,
                                            BIDI_DISABLED_OPEN,
                                            st,
                                        );
                                        open_range += 1;
                                    }
                                    let mut encoded = [0; 4];
                                    append_styled_piece(
                                        &mut shaping_text,
                                        &mut styled_spans,
                                        ch.encode_utf8(&mut encoded),
                                        st,
                                    );
                                    let end = byte + ch.len_utf8();
                                    while bidi_disabled_ranges
                                        .get(close_range)
                                        .is_some_and(|range| range.end == end)
                                    {
                                        append_styled_piece(
                                            &mut shaping_text,
                                            &mut styled_spans,
                                            BIDI_DISABLED_CLOSE,
                                            st,
                                        );
                                        close_range += 1;
                                    }
                                    while bidi_paragraph_ranges
                                        .get(close_paragraph)
                                        .is_some_and(|range| range.end == end)
                                    {
                                        append_styled_piece(
                                            &mut shaping_text,
                                            &mut styled_spans,
                                            BIDI_PARAGRAPH_CLOSE,
                                            st,
                                        );
                                        close_paragraph += 1;
                                    }
                                }
                                let mut runs: Vec<TextRun> = Vec::new();
                                let mut push_run =
                                    |st: remacs_runtime::frontend::CellStyle, len: usize| {
                                        if len == 0 {
                                            return;
                                        }
                                        let (fg, bg) = style_colors(
                                            st,
                                            default_background,
                                            background_alpha,
                                            grid.gamma_exponent,
                                        );
                                        let mut f = text_font.clone();
                                        if let Some(family) = st
                                            .font_family
                                            .and_then(|id| font_families.get(id as usize))
                                        {
                                            f.family = font_family(window, Some(family.clone())).into();
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
                                            background_color: if st.stipple.is_some() {
                                                None
                                            } else {
                                                bg
                                            },
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
                                for (len, style) in styled_spans {
                                    push_run(style, len);
                                }
                                if runs.is_empty() {
                                    runs.push(TextRun {
                                        len: 0,
                                        font: text_font.clone(),
                                        color: rgb(gamma_correct_rgb(FG, grid.gamma_exponent)).into(),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    });
                                }
                                window.text_system().shape_line(
                                    shaping_text.into(),
                                    font_size,
                                    &runs,
                                    None,
                                )
                            })
                            .collect();
                        (shaped, grid)
                    },
                    move |bounds, (lines, grid), window, _cx| {
                        let modifier_state = physical_modifier_state.clone();
                        window.on_modifiers_changed(move |event, _window, _cx| {
                            if let (Some(changed), Some(is_down)) =
                                (event.changed_modifier, event.changed_modifier_is_down)
                            {
                                modifier_state.set(modifier_state_after_change(
                                    modifier_state.get(),
                                    changed,
                                    is_down,
                                ));
                            }
                        });
                        window.on_mouse_event(
                            move |event: &MouseExitEvent, phase, _window, _cx| {
                                if phase == DispatchPhase::Bubble {
                                    let _ = mouse_face_exit_position.replace(None);
                                    let absolute_position = absolute_mouse_pixel_position(
                                        event.position,
                                        pointer_screen_left,
                                        pointer_screen_top,
                                        content_top_offset,
                                    );
                                    let _ = mouse_face_exit_tx
                                        .send_mouse_move(None, Some(absolute_position));
                                }
                            },
                        );
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity).disable_apple_press_and_hold(),
                            _cx,
                        );
                        for (y, line) in lines.iter().enumerate() {
                            if let Some(clips) =
                                unscrolled_row_clips(&grid, y, bounds, cell_width, line_height)
                            {
                                for clip in clips {
                                    window.with_content_mask(
                                        Some(ContentMask { bounds: clip }),
                                        |window| {
                                            paint_grid_row(
                                                &grid,
                                                y,
                                                line,
                                                bounds,
                                                line_height,
                                                cell_width,
                                                default_background,
                                                background_alpha,
                                                window,
                                                _cx,
                                            );
                                        },
                                    );
                                }
                            } else {
                                paint_grid_row(
                                    &grid,
                                    y,
                                    line,
                                    bounds,
                                    line_height,
                                    cell_width,
                                    default_background,
                                    background_alpha,
                                    window,
                                    _cx,
                                );
                            }
                        }
                        paint_vscroll_regions(
                            &grid,
                            &lines,
                            bounds,
                            cell_width,
                            line_height,
                            default_background,
                            background_alpha,
                            window,
                            _cx,
                        );
                        paint_grid_compositions(
                            &grid,
                            &lines,
                            bounds,
                            cell_width,
                            line_height,
                            font_size,
                            &composition_font,
                            default_background,
                            background_alpha,
                            window,
                            _cx,
                        );
                        paint_bottom_dividers(
                            &grid,
                            bounds,
                            cell_width,
                            line_height,
                            default_background,
                            background_alpha,
                            window,
                        );
                        for indicator in &grid.fill_column_indicators {
                            let mut fill_font = composition_font.clone();
                            if let Some(family) = indicator
                                .style
                                .font_family
                                .and_then(|id| grid.font_families.get(id as usize))
                            {
                                fill_font.family = font_family(window, Some(family.clone())).into();
                            }
                            if let Some(weight) = indicator.style.font_weight {
                                fill_font.weight = FontWeight(weight as f32);
                            } else if indicator.style.bold {
                                fill_font = fill_font.bold();
                            }
                            if let Some(width) = indicator.style.font_width {
                                fill_font.stretch = FontStretch::from_percentage(width);
                            }
                            if indicator.style.italic {
                                fill_font = fill_font.italic();
                            }
                            let (color, background_color) = style_colors(
                                indicator.style,
                                default_background,
                                background_alpha,
                                grid.gamma_exponent,
                            );
                            let text = indicator.character.to_string();
                            let line = window.text_system().shape_line(
                                text.clone().into(),
                                font_size,
                                &[TextRun {
                                    len: text.len(),
                                    font: fill_font,
                                    color,
                                    background_color: if indicator.style.stipple.is_some() {
                                        None
                                    } else {
                                        background_color
                                    },
                                    underline: indicator.style.underline.then(|| UnderlineStyle {
                                        thickness: px(1.0),
                                        color: indicator
                                            .style
                                            .underline_color
                                            .map(|color| rgb(color_rgb(color)).into()),
                                        wavy: indicator.style.underline_wave,
                                    }),
                                    strikethrough: indicator.style.strike.then(|| {
                                        StrikethroughStyle {
                                            thickness: px(1.0),
                                            color: indicator
                                                .style
                                                .strike_color
                                                .map(|color| rgb(color_rgb(color)).into()),
                                        }
                                    }),
                                }],
                                None,
                            );
                            let origin = point(
                                bounds.left() + cell_width * indicator.column as f32,
                                bounds.top()
                                    + grid_row_top(&grid, indicator.row, line_height)
                                    - px(vscroll_pixels_for_cell(
                                        &grid,
                                        indicator.row,
                                        indicator.column,
                                    ) as f32),
                            );
                            let row_height =
                                grid_row_height(&grid, indicator.row, line_height);
                            with_vscroll_cell_clip(
                                &grid,
                                indicator.row,
                                indicator.column,
                                bounds,
                                cell_width,
                                line_height,
                                window,
                                |window| {
                                    let _ = line.paint(origin, row_height, window, _cx);
                                },
                            );
                        }
                        let fallback_cursor = grid.cursor.map(|(column, row)| {
                            let x = grid
                                .cursor_index
                                .and_then(|index| {
                                    lines.get(row).map(|line| {
                                        grid_line_x(&grid, row, line, index)
                                    })
                                })
                                .unwrap_or(cell_width * (column as f32));
                            (row, x)
                        });
                        let composition_origin = (!marked.is_empty())
                            .then_some(marked_origin)
                            .flatten()
                            .and_then(|(row, index)| {
                                lines.get(row).map(|line| {
                                    (row, grid_line_x(&grid, row, line, index))
                                })
                            })
                            .or(fallback_cursor);
                        let composition_caret = if !marked.is_empty() {
                            composition_origin.map(|(row, x)| {
                                let origin = point(
                                    bounds.left() + x,
                                    bounds.top() + grid_row_top(&grid, row, line_height)
                                        - px(vscroll_pixels_for_cell(
                                            &grid,
                                            row,
                                            (f32::from(x) / f32::from(cell_width)) as usize,
                                        ) as f32),
                                );
                                let selection_start = marked_selection.start.min(marked.len());
                                let selection_end = marked_selection.end.min(marked.len());
                                let composition_run = |start: usize, end: usize, selected: bool| {
                                    TextRun {
                                        len: end.saturating_sub(start),
                                        font: composition_font.clone(),
                                        color: if selected {
                                            rgb(gamma_correct_rgb(
                                                0xffffff,
                                                grid.gamma_exponent,
                                            ))
                                            .into()
                                        } else {
                                            rgb(gamma_correct_rgb(FG, grid.gamma_exponent)).into()
                                        },
                                        background_color: selected.then(|| {
                                            rgb(gamma_correct_rgb(
                                                0x356fce,
                                                grid.gamma_exponent,
                                            ))
                                            .into()
                                        }),
                                        underline: Some(UnderlineStyle {
                                            thickness: px(1.0),
                                            color: None,
                                            wavy: false,
                                        }),
                                        strikethrough: None,
                                    }
                                };
                                let mut composition_runs = Vec::new();
                                if selection_start == selection_end {
                                    composition_runs.push(composition_run(0, marked.len(), false));
                                } else {
                                    if selection_start > 0 {
                                        composition_runs.push(composition_run(
                                            0,
                                            selection_start,
                                            false,
                                        ));
                                    }
                                    composition_runs.push(composition_run(
                                        selection_start,
                                        selection_end,
                                        true,
                                    ));
                                    if selection_end < marked.len() {
                                        composition_runs.push(composition_run(
                                            selection_end,
                                            marked.len(),
                                            false,
                                        ));
                                    }
                                }
                                let line = window.text_system().shape_line(
                                    marked.clone().into(),
                                    font_size,
                                    &composition_runs,
                                    None,
                                );
                                let column = (f32::from(x) / f32::from(cell_width)) as usize;
                                with_vscroll_cell_clip(
                                    &grid,
                                    row,
                                    column,
                                    bounds,
                                    cell_width,
                                    line_height,
                                    window,
                                    |window| {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin,
                                                size: size(line.width.max(cell_width), line_height),
                                            },
                                            default_background_fill,
                                        ));
                                        let _ = line.paint(origin, line_height, window, _cx);
                                    },
                                );
                                (row, x + line.x_for_index(marked_cursor))
                            })
                        } else {
                            None
                        };

                        let blink_visible = if grid.blink_cursor {
                            let elapsed = grid.blink_elapsed_since_input - grid.blink_delay;
                            let interval = grid.blink_interval.max(0.05);
                            let completed = grid.blink_blinks > 0
                                && elapsed >= interval * 2.0 * grid.blink_blinks as f64;
                            elapsed < 0.0
                                || completed
                                || (elapsed / interval).floor() as u64 % 2 == 1
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
                                    .unwrap_or_else(|| {
                                        grid_line_x(&grid, row, line, cursor.index)
                                    })
                            } else {
                                grid_line_x(&grid, row, line, cursor.index)
                            };
                            let glyph_width = if cursor.selected && composition_caret.is_some() {
                                cell_width
                            } else {
                                cursor
                                    .after_index
                                    .map(|end| {
                                        let end_x = grid_line_x(&grid, row, line, end);
                                        if end_x >= x { end_x - x } else { x - end_x }
                                    })
                                    .filter(|width| *width > px(0.0))
                                    .unwrap_or(cell_width)
                            };
                            let rgb_color = cursor.color.map_or_else(
                                || gamma_correct_rgb(CURSOR, grid.gamma_exponent),
                                |color| {
                                    ((color.0 as u32) << 16)
                                        | ((color.1 as u32) << 8)
                                        | color.2 as u32
                                },
                            );
                            let column = if cursor.selected && composition_caret.is_some() {
                                (f32::from(x) / f32::from(cell_width)) as usize
                            } else {
                                cursor.column
                            };
                            let origin =
                                point(
                                    bounds.left() + x,
                                    bounds.top() + grid_row_top(&grid, row, line_height)
                                        - px(vscroll_pixels_for_cell(&grid, row, column) as f32),
                                );
                            with_vscroll_cell_clip(
                                &grid,
                                row,
                                column,
                                bounds,
                                cell_width,
                                line_height,
                                window,
                                |window| match cursor.shape {
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
                                },
                            );
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
                        if bell_flash {
                            window.paint_quad(fill(bounds, rgba((0xFFFFFF << 8) | 0x80)));
                        }
                    },
                )
                .size_full(),
            )
            .children(
                images
                    .into_iter()
                    .map(|image| {
                        grid_image_element(
                            image,
                            cell_width,
                            line_height,
                            &image_row_heights,
                            image_default_line_height,
                            border,
                            &image_vscrolls,
                            image_cache_entries.clone(),
                            image_cache_size.clone(),
                        )
                    }),
            )
            .children(help_echoes.into_iter().map(|region| {
                let text = region.text;
                let vscroll = help_vscrolls
                    .iter()
                    .find(|scroll| {
                        region.row >= scroll.top
                            && region.row < scroll.top.saturating_add(scroll.rows)
                            && region.left >= scroll.left
                            && region.left < scroll.left.saturating_add(scroll.width)
                    })
                    .map_or(0, |scroll| scroll.pixels);
                let top = row_top_from_heights(
                    &help_row_heights,
                    help_default_line_height,
                    region.row,
                    line_height,
                );
                let height = row_height_from_heights(
                    &help_row_heights,
                    help_default_line_height,
                    region.row,
                    line_height,
                );
                div()
                    .absolute()
                    .left(border + cell_width * region.left as f32)
                    .top(border + top - px(vscroll as f32))
                    .w(cell_width * region.width as f32)
                    .h(height)
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
                        .left(border + px(12.0))
                        .top(border + px(12.0))
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
    row_heights: &[usize],
    default_line_height: usize,
    border: Pixels,
    vscrolls: &[GridVScroll],
    image_cache_entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_size: Arc<AtomicUsize>,
) -> AnyElement {
    let cache_key = image.source.clone();
    let source = match image.source {
        GridImageSource::File(path) => img(std::path::PathBuf::from(path)),
        GridImageSource::FileWithSvgOptions {
            path,
            base_uri,
            css,
        } => img(ImageSource::ResourceWithSvgOptions {
            resource: std::path::PathBuf::from(path).into(),
            base_uri: base_uri.map(std::path::PathBuf::from),
            css,
        }),
        GridImageSource::Data { format, bytes } => {
            let Some(source) = grid_data_image(format, bytes, None, None) else {
                return div().into_any_element();
            };
            source
        }
        GridImageSource::DataWithBaseUri {
            format,
            bytes,
            base_uri,
        } => {
            let Some(source) = grid_data_image(format, bytes, Some(base_uri), None) else {
                return div().into_any_element();
            };
            source
        }
        GridImageSource::DataWithSvgOptions {
            format,
            bytes,
            base_uri,
            css,
        } => {
            let Some(source) = grid_data_image(format, bytes, base_uri, css) else {
                return div().into_any_element();
            };
            source
        }
    };
    let source = source.on_render_image_loaded(move |data| {
        let image_bytes = (0..data.frame_count()).fold(0usize, |total, frame| {
            total.saturating_add(data.as_bytes(frame).map_or(0, <[u8]>::len))
        });
        if let Ok(mut entries) = image_cache_entries.lock() {
            entries.insert(cache_key.clone(), image_bytes);
            let total = entries
                .values()
                .fold(0usize, |total, bytes| total.saturating_add(*bytes));
            image_cache_size.store(total, Ordering::Release);
        }
    });
    let source = match (image.width, image.height) {
        (Some(width), Some(height)) => source.w(px(width as f32)).h(px(height as f32)),
        (Some(width), None) => source.w(px(width as f32)),
        (None, Some(height)) => source.h(px(height as f32)),
        (None, None) => source,
    };
    let image_left = border + cell_width * image.column as f32;
    let image_top =
        border + row_top_from_heights(row_heights, default_line_height, image.row, line_height);
    let scroll = vscrolls.iter().find(|scroll| {
        image.row >= scroll.top
            && image.row < scroll.top.saturating_add(scroll.rows)
            && image.column >= scroll.left
            && image.column < scroll.left.saturating_add(scroll.width)
    });
    if let Some(scroll) = scroll {
        let first_row = scroll.top.min(row_heights.len());
        let end_row = scroll.top.saturating_add(scroll.rows).min(row_heights.len());
        let region_left = border + cell_width * scroll.left as f32;
        let region_top = border
            + row_top_from_heights(
                row_heights,
                default_line_height,
                first_row,
                line_height,
            );
        let region_bottom = border
            + row_top_from_heights(
                row_heights,
                default_line_height,
                end_row,
                line_height,
            );
        let image = div()
            .absolute()
            .left(image_left - region_left)
            .top(image_top - region_top - px(scroll.pixels as f32))
            .child(source);
        div()
            .absolute()
            .left(region_left)
            .top(region_top)
            .w(cell_width * scroll.width as f32)
            .h(region_bottom - region_top)
            .overflow_hidden()
            .child(image)
            .into_any_element()
    } else {
        div()
            .absolute()
            .left(image_left)
            .top(image_top)
            .child(source)
            .into_any_element()
    }
}

fn grid_data_image(
    format: String,
    bytes: Arc<[u8]>,
    base_uri: Option<String>,
    css: Option<String>,
) -> Option<Img> {
    let format = match format.as_str() {
        "png" => ImageFormat::Png,
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "svg" => ImageFormat::Svg,
        "bmp" => ImageFormat::Bmp,
        "tiff" | "tif" => ImageFormat::Tiff,
        _ => return None,
    };
    match (base_uri, css) {
        (Some(base_uri), None) => Some(img(ImageSource::ImageWithBaseUri {
            image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
            base_uri: std::path::PathBuf::from(base_uri),
        })),
        (base_uri, Some(css)) => Some(img(ImageSource::ImageWithSvgOptions {
            image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
            base_uri: base_uri.map(std::path::PathBuf::from),
            css: Some(css),
        })),
        (None, None) => Some(img(Arc::new(Image::from_bytes(format, bytes.to_vec())))),
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

fn stipple_bit(stipple: &GridStipple, x: usize, y: usize) -> bool {
    let width = usize::from(stipple.width);
    let height = usize::from(stipple.height);
    if width == 0 || height == 0 {
        return false;
    }
    let row_bytes = width.div_ceil(8);
    let index = (y % height) * row_bytes + (x % width) / 8;
    stipple
        .data
        .get(index)
        .is_some_and(|byte| byte & (1 << ((x % width) % 8)) != 0)
}

fn paint_stipple_region(
    stipple: &GridStipple,
    style: remacs_runtime::frontend::CellStyle,
    left: Pixels,
    width: Pixels,
    row_top: Pixels,
    row_height: Pixels,
    bounds: Bounds<Pixels>,
    default_background: u32,
    background_alpha: u8,
    gamma_exponent: Option<f64>,
    window: &mut Window,
) {
    let width_px = f32::from(width).ceil().max(0.0) as usize;
    let height_px = f32::from(row_height).ceil().max(0.0) as usize;
    if width_px == 0 || height_px == 0 {
        return;
    }
    let origin = point(bounds.left() + left, bounds.top() + row_top);
    let (foreground, background) = style_colors(
        style,
        default_background,
        background_alpha,
        gamma_exponent,
    );
    window.paint_quad(fill(
        Bounds {
            origin,
            size: size(px(width_px as f32), px(height_px as f32)),
        },
        background.unwrap_or_else(|| background_rgba(default_background, background_alpha)),
    ));

    let origin_x = f32::from(origin.x).floor() as isize;
    let origin_y = f32::from(origin.y).floor() as isize;
    let stipple_width = usize::from(stipple.width).max(1) as isize;
    let stipple_height = usize::from(stipple.height).max(1) as isize;
    for y in 0..height_px {
        let pattern_y = (origin_y + y as isize).rem_euclid(stipple_height) as usize;
        let mut x = 0usize;
        while x < width_px {
            let pattern_x = (origin_x + x as isize).rem_euclid(stipple_width) as usize;
            if !stipple_bit(stipple, pattern_x, pattern_y) {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < width_px {
                let pattern_x = (origin_x + x as isize).rem_euclid(stipple_width) as usize;
                if !stipple_bit(stipple, pattern_x, pattern_y) {
                    break;
                }
                x += 1;
            }
            window.paint_quad(fill(
                Bounds {
                    origin: point(origin.x + px(start as f32), origin.y + px(y as f32)),
                    size: size(px((x - start) as f32), px(1.0)),
                },
                foreground,
            ));
        }
    }
}

fn paint_stipple_backgrounds(
    grid: &Grid,
    row_index: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    row_top: Pixels,
    row_height: Pixels,
    default_background: u32,
    background_alpha: u8,
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
    for (index, (byte, character)) in text.char_indices().enumerate() {
        let Some(style) = styles.get(index).copied() else {
            continue;
        };
        let Some(stipple) = style
            .stipple
            .and_then(|stipple| grid.stipples.get(stipple as usize))
        else {
            continue;
        };
        let end = byte + character.len_utf8();
        let start_x = grid_line_x(grid, row_index, line, byte);
        let end_x = grid_line_x(grid, row_index, line, end);
        let (left, width) = if end_x >= start_x {
            (start_x, end_x - start_x)
        } else {
            (end_x, start_x - end_x)
        };
        paint_stipple_region(
            stipple,
            style,
            left,
            width,
            row_top,
            row_height,
            bounds,
            default_background,
            background_alpha,
            grid.gamma_exponent,
            window,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_grid_line(
    grid: &Grid,
    row: usize,
    line: &ShapedLine,
    origin: Point<Pixels>,
    row_height: Pixels,
    cell_width: Pixels,
    line_height: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
    cx: &mut App,
) {
    let mut segments = grid
        .pixelwise_segments
        .iter()
        .filter(|segment| segment.row == row)
        .collect::<Vec<_>>();
    if segments.is_empty() || cell_width <= px(0.0) {
        let _ = line.paint(origin, line_height, window, cx);
        return;
    }
    segments.sort_unstable_by_key(|segment| (segment.left_column, segment.right_column));
    let columns = grid.attrs.get(row).map_or(0, Vec::len);
    let mut column = 0usize;
    for segment in &segments {
        let left = segment.left_column.min(columns);
        let right = segment.right_column.min(columns);
        if left > column {
            let clip = Bounds {
                origin: point(origin.x + cell_width * column as f32, origin.y),
                size: size(cell_width * (left - column) as f32, row_height),
            };
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                let _ = line.paint(origin, line_height, window, cx);
            });
        }
        column = column.max(right);
    }
    if column < columns {
        let clip = Bounds {
            origin: point(origin.x + cell_width * column as f32, origin.y),
            size: size(cell_width * (columns - column) as f32, row_height),
        };
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            let _ = line.paint(origin, line_height, window, cx);
        });
    }

    let background = rgba((default_background << 8) | u32::from(background_alpha));
    for segment in segments {
        let left = segment.left_column.min(columns);
        let right = segment.right_column.min(columns);
        if left >= right {
            continue;
        }
        let x = origin.x + cell_width * left as f32 + px(segment.x_offset as f32);
        let width = (cell_width * (right - left) as f32 + px(segment.width_adjustment as f32))
            .max(px(0.0));
        if width <= px(0.0) {
            continue;
        }
        let clip = Bounds {
            origin: point(x, origin.y),
            size: size(width, row_height),
        };
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            window.paint_quad(fill(clip, background));
            let shifted_origin = point(origin.x + px(segment.x_offset as f32), origin.y);
            let _ = line.paint(shifted_origin, line_height, window, cx);
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_grid_row(
    grid: &Grid,
    row: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    cell_width: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
    cx: &mut App,
) {
    let row_top = grid_row_top(grid, row, line_height);
    let row_height = grid_row_height(grid, row, line_height);
    let origin = point(bounds.left(), bounds.top() + row_top);
    paint_stipple_backgrounds(
        grid,
        row,
        line,
        bounds,
        row_top,
        row_height,
        default_background,
        background_alpha,
        window,
    );
    paint_grid_line(
        grid,
        row,
        line,
        origin,
        row_height,
        cell_width,
        line_height,
        default_background,
        background_alpha,
        window,
        cx,
    );
    paint_face_boxes(
        grid,
        row,
        line,
        bounds,
        row_top,
        line_height,
        default_background,
        grid.gamma_exponent,
        window,
    );

    let styles = grid
        .char_attrs
        .get(row)
        .filter(|styles| !styles.is_empty())
        .or_else(|| grid.attrs.get(row));
    if let Some(styles) = styles {
        let overline_y = origin.y
            + (line_height - line.ascent - line.descent) / 2.0
            + px(1.0);
        for (index, (byte, ch)) in grid.rows[row].char_indices().enumerate() {
            let Some(style) = styles.get(index).copied() else {
                continue;
            };
            if !style.overline {
                continue;
            }
            let start = grid_line_x(grid, row, line, byte);
            let end = grid_line_x(grid, row, line, byte + ch.len_utf8());
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
                    style_colors(
                        style,
                        default_background,
                        background_alpha,
                        grid.gamma_exponent,
                    )
                    .0,
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_vscroll_regions(
    grid: &Grid,
    lines: &[ShapedLine],
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
    cx: &mut App,
) {
    for scroll in &grid.vscrolls {
        let first_row = scroll.top.min(lines.len());
        let end_row = scroll.top.saturating_add(scroll.rows).min(lines.len());
        if first_row >= end_row || scroll.width == 0 {
            continue;
        }

        let region_top = bounds.top() + grid_row_top(grid, first_row, line_height);
        let region_bottom = bounds.top() + grid_row_top(grid, end_row, line_height);
        let region = Bounds {
            origin: point(bounds.left() + cell_width * scroll.left as f32, region_top),
            size: size(cell_width * scroll.width as f32, region_bottom - region_top),
        };
        let pixels = px(scroll.pixels as f32);

        // The unscrolled rows were clipped out of this window's text area.
        window.with_content_mask(Some(ContentMask { bounds: region }), |window| {
            for row in first_row..end_row {
                let Some(line) = lines.get(row) else {
                    continue;
                };
                let row_top = grid_row_top(grid, row, line_height) - pixels;
                let row_height = grid_row_height(grid, row, line_height);
                let origin = point(bounds.left(), bounds.top() + row_top);
                paint_stipple_backgrounds(
                    grid,
                    row,
                    line,
                    bounds,
                    row_top,
                    row_height,
                    default_background,
                    background_alpha,
                    window,
                );
                paint_grid_line(
                    grid,
                    row,
                    line,
                    origin,
                    row_height,
                    cell_width,
                    line_height,
                    default_background,
                    background_alpha,
                    window,
                    cx,
                );
                paint_face_boxes(
                    grid,
                    row,
                    line,
                    bounds,
                    row_top,
                    line_height,
                    default_background,
                    grid.gamma_exponent,
                    window,
                );

                let styles = grid
                    .char_attrs
                    .get(row)
                    .filter(|styles| !styles.is_empty())
                    .or_else(|| grid.attrs.get(row));
                if let Some(styles) = styles {
                    let overline_y = origin.y
                        + (line_height - line.ascent - line.descent) / 2.0
                        + px(1.0);
                    for (index, (byte, ch)) in grid.rows[row].char_indices().enumerate() {
                        let Some(style) = styles.get(index) else {
                            continue;
                        };
                        if !style.overline {
                            continue;
                        }
                        let start = grid_line_x(grid, row, line, byte);
                        let end = grid_line_x(grid, row, line, byte + ch.len_utf8());
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
                                style_colors(
                                    *style,
                                    default_background,
                                    background_alpha,
                                    grid.gamma_exponent,
                                )
                                .0,
                            ));
                        }
                    }
                }
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_grid_compositions(
    grid: &Grid,
    lines: &[ShapedLine],
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    font_size: Pixels,
    default_font: &Font,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
    cx: &mut App,
) {
    for composition in &grid.compositions {
        let Some(line) = lines.get(composition.row) else {
            continue;
        };
        if composition.components.len() < 2 {
            continue;
        }
        let mut component_font = default_font.clone();
        if let Some(family) = composition
            .style
            .font_family
            .and_then(|id| grid.font_families.get(id as usize))
        {
            component_font.family = font_family(window, Some(family.clone())).into();
        }
        if let Some(weight) = composition.style.font_weight {
            component_font.weight = FontWeight(weight as f32);
        } else if composition.style.bold {
            component_font = component_font.bold();
        }
        if let Some(width) = composition.style.font_width {
            component_font.stretch = FontStretch::from_percentage(width);
        }
        if composition.style.italic {
            component_font = component_font.italic();
        }
        let color = style_colors(
            composition.style,
            default_background,
            background_alpha,
            grid.gamma_exponent,
        )
        .0;
        let x = grid_line_x(grid, composition.row, line, composition.byte_index);
        let origin = point(
            bounds.left() + x,
            bounds.top() + grid_row_top(grid, composition.row, line_height)
                - px(vscroll_pixels_for_cell(grid, composition.row, composition.column) as f32),
        );
        with_vscroll_cell_clip(
            grid,
            composition.row,
            composition.column,
            bounds,
            cell_width,
            line_height,
            window,
            |window| {
                for character in composition.components.iter().skip(1) {
                    let text = character.to_string();
                    let component_line = window.text_system().shape_line(
                        text.clone().into(),
                        font_size,
                        &[TextRun {
                            len: text.len(),
                            font: component_font.clone(),
                            color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }],
                        None,
                    );
                    let _ = component_line.paint(origin, line_height, window, cx);
                }
            },
        );
    }
}

/// CellStyle → (foreground, background) for gpui; `:inverse-video'
/// swaps the two (default palette entries stand in for unset sides).
fn paint_face_boxes(
    grid: &Grid,
    row_index: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    row_top: Pixels,
    line_height: Pixels,
    default_background: u32,
    gamma_exponent: Option<f64>,
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
                    grid,
                    row_index,
                    line,
                    bounds,
                    row_top,
                    line_height,
                    box_style,
                    fg,
                    bg,
                    start,
                    end,
                    default_background,
                    gamma_exponent,
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
                        grid,
                        row_index,
                        line,
                        bounds,
                        row_top,
                        line_height,
                        box_style,
                        fg,
                        bg,
                        start,
                        end,
                        default_background,
                        gamma_exponent,
                        window,
                    );
                }
                current = Some((box_style, style.fg, style.bg, byte, end));
            }
        }
    }
    if let Some((box_style, fg, bg, start, end)) = current {
        paint_box_run(
            grid,
            row_index,
            line,
            bounds,
            row_top,
            line_height,
            box_style,
            fg,
            bg,
            start,
            end,
            default_background,
            gamma_exponent,
            window,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_box_run(
    grid: &Grid,
    row_index: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    row_top: Pixels,
    line_height: Pixels,
    box_style: BoxStyle,
    foreground: Option<CellColor>,
    background: Option<CellColor>,
    start: usize,
    end: usize,
    default_background: u32,
    gamma_exponent: Option<f64>,
    window: &mut Window,
) {
    let x1 = grid_line_x(grid, row_index, line, start);
    let x2 = grid_line_x(grid, row_index, line, end);
    let left = x1.min(x2);
    let right = x1.max(x2);
    if right <= left {
        return;
    }

    let corrected_foreground = gamma_correct_rgb(FG, gamma_exponent);
    let default_foreground = CellColor(
        ((corrected_foreground >> 16) & 0xff) as u8,
        ((corrected_foreground >> 8) & 0xff) as u8,
        (corrected_foreground & 0xff) as u8,
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
    let origin_y = bounds.top() + row_top;
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

fn gamma_correct_rgb(color: u32, exponent: Option<f64>) -> u32 {
    let Some(exponent) = exponent else {
        return color;
    };
    let channel = |shift: u32| {
        let value = ((color >> shift) & 0xff) as f64 / 255.0;
        ((value.powf(exponent) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

fn paint_bottom_dividers(
    grid: &Grid,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
) {
    for divider in &grid.bottom_dividers {
        if divider.width == 0 || divider.thickness == 0 {
            continue;
        }
        let thickness = px(divider.thickness as f32);
        let left = bounds.left() + cell_width * divider.left as f32;
        let width = cell_width * divider.width as f32;
        let top = bounds.top()
            + grid_row_top(grid, divider.row_boundary, line_height)
            - thickness;
        let paint_line = |offset: usize, height: usize, style, window: &mut Window| {
            if height == 0 {
                return;
            }
            let color = style_colors(
                style,
                default_background,
                background_alpha,
                grid.gamma_exponent,
            )
            .0;
            window.paint_quad(fill(
                Bounds {
                    origin: point(left, top + px(offset as f32)),
                    size: size(width, px(height as f32)),
                },
                color,
            ));
        };
        if divider.thickness < 3 {
            paint_line(0, divider.thickness, divider.style, window);
        } else {
            paint_line(0, 1, divider.first_pixel_style, window);
            paint_line(
                1,
                divider.thickness.saturating_sub(2),
                divider.style,
                window,
            );
            paint_line(
                divider.thickness.saturating_sub(1),
                1,
                divider.last_pixel_style,
                window,
            );
        }
    }
}

fn shade_color(color: u32, factor: f32) -> u32 {
    let shade = |shift: u32| (((color >> shift) & 0xff) as f32 * factor).clamp(0.0, 255.0) as u32;
    (shade(16) << 16) | (shade(8) << 8) | shade(0)
}

fn style_colors(
    st: remacs_runtime::frontend::CellStyle,
    default_background: u32,
    background_alpha: u8,
    gamma_exponent: Option<f64>,
) -> (Hsla, Option<Hsla>) {
    let rgb_color = |c: remacs_runtime::frontend::CellColor| {
        ((c.0 as u32) << 16) | ((c.1 as u32) << 8) | c.2 as u32
    };
    let foreground = st
        .fg
        .map(|color| rgb(rgb_color(color)).into())
        .unwrap_or_else(|| rgb(gamma_correct_rgb(FG, gamma_exponent)).into());
    let background = st
        .bg
        .map(|color| background_rgba(rgb_color(color), background_alpha));
    if st.inverse {
        (
            st.bg
                .map(|color| rgb(rgb_color(color)).into())
                .unwrap_or_else(|| rgb(default_background).into()),
            Some(background_rgba(
                st.fg.map(rgb_color).unwrap_or(FG),
                background_alpha,
            )),
        )
    } else {
        (foreground, background)
    }
}

fn background_rgba(color: u32, alpha: u8) -> Hsla {
    rgba((color << 8) | u32::from(alpha)).into()
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

/// Build a view whose grid is managed by the application-level frame pump.
/// Unlike the single-window test helper above, this view does not own a
/// receiver; the app updates it by frame id.
pub(super) fn build_managed_editor_view(
    window: &mut Window,
    cx: &mut Context<EditorView>,
    event_tx: Sender<GuiEvent>,
    frame_id: Option<usize>,
    grid: Option<Grid>,
    focus_on_map: bool,
    no_accept_focus: bool,
) -> EditorView {
    let mut view = EditorView::new_for_frame(event_tx, frame_id, cx);
    view.no_accept_focus.set(no_accept_focus);
    if let Some(grid) = grid {
        view.grid = grid;
    }
    let close_events = view.tx.clone();
    window.on_window_should_close(cx, move |_window, _cx| {
        close_events
            .frame_id()
            .is_some_and(|frame_id| close_events.send_delete_request(frame_id).is_err())
    });
    let frame_events = view.tx.clone();
    let subscription = window.on_focus_out(&view.focus, cx, move |_event, window, cx| {
        if let (Some(frame_id), Some(stack)) = (frame_events.frame_id(), cx.window_stack()) {
            let visible = stack.contains(&window.window_handle());
            let _ = frame_events.send_visibility_update(frame_id, visible);
        }
    });
    view.focus_subscriptions.push(subscription);
    let no_accept_focus = view.no_accept_focus.clone();
    let no_accept_focus_on_focus_in = no_accept_focus.clone();
    let frame_events = view.tx.clone();
    let subscription = window.on_focus_in(&view.focus, cx, move |window, cx| {
        if no_accept_focus_on_focus_in.get() {
            window.blur();
        }
        if let Some(frame_id) = frame_events.frame_id() {
            let visible = cx
                .window_stack()
                .map(|stack| stack.contains(&window.window_handle()))
                .unwrap_or(true);
            let _ = frame_events.send_visibility_update(frame_id, visible);
        }
    });
    view.focus_subscriptions.push(subscription);
    if focus_on_map && !no_accept_focus.get() {
        view.focus.focus(window);
    }
    let appearance_view = cx.entity();
    view.appearance_subscription = Some(window.observe_window_appearance(move |window, cx| {
        let appearance = window.appearance();
        let _ = appearance_view.update(cx, |view, cx| {
            if view.set_appearance(appearance) {
                cx.notify();
            }
        });
    }));
    view
}
