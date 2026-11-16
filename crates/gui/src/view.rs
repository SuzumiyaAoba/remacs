//! GPUI view construction and painting of frontend-neutral grids.
use super::{
    keys::{
        ModifierMapping, PhysicalModifierState, clear_physical_modifier_state,
        command_keystroke_with_mapping,
        is_native_command_w, keystroke_to_code_with_mapping, modifier_state_after_change,
        mouse_modifier_bits, pass_text_to_os, unicode_key_text,
    },
    logic::{COLOR_DROP_PREFIX, FrameEventSender, FramePosition, GuiEvent, GuiGeometry},
    monitor::monitor_geometry,
};
use gpui::prelude::{FluentBuilder, InteractiveElement, StatefulInteractiveElement};
use gpui::*;
use remacs_runtime::frontend::{
    BoxStyle, BoxStyleKind, CellColor, CellStyle, FontAtStyle, FontGlyphKey, FrameFontMetrics,
    Grid, GridBidiDirection, GridCursor, GridImage, GridImageConversion, GridImageSizeLimit,
    GridImageSource, GridMenu, GridMenuItem,
    GridPointerShape, GridScrollBar, GridScrollBarOrientation, GridStipple, GridTooltipPosition,
    GridVScroll, GridWindowRowHeights, MouseArea, MouseButton as RemacsMouseButton, MouseInput,
    MousePosition,
    NativeMenuCommandId,
    QuitRequest, WeakNativeMenuCommandId,
};
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
    mpsc::Sender,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use unicode_width::UnicodeWidthChar;

fn external_color_drop_event(
    position: Option<MousePosition>,
    color: &ExternalColor,
) -> Option<GuiEvent> {
    let (kind, color) = color
        .0
        .strip_prefix(COLOR_DROP_PREFIX)?
        .split_once(':')?;
    Some(GuiEvent::ColorDrop {
        position,
        color: color.to_owned(),
        background: kind == "background",
    })
}

fn external_text_drop_event(position: Option<MousePosition>, text: &ExternalText) -> GuiEvent {
    GuiEvent::Drop {
        position,
        urls: vec![text.0.clone()],
        action: "copy".to_owned(),
    }
}

#[cfg(test)]
mod autoselect_tests;
#[cfg(test)]
mod color_drop_tests;
#[cfg(test)]
mod window_activation_tests;
#[cfg(test)]
mod frame_metrics_tests;
#[cfg(all(test, target_os = "macos"))]
mod begin_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod break_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod button_release_tests;
#[cfg(all(test, target_os = "macos"))]
mod char_line_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod clear_line_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod execute_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod find_key_tests;
mod fringe;
#[cfg(all(test, target_os = "macos"))]
mod help_key_tests;
#[cfg(test)]
mod icon_title_tests;
#[cfg(test)]
mod image_data_tests;
mod input;
#[cfg(all(test, target_os = "macos"))]
mod insert_key_tests;
#[cfg(test)]
mod minimum_cells_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(all(test, target_os = "macos"))]
mod pause_print_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod popup_dismiss_tests;
#[cfg(all(test, target_os = "macos"))]
mod popup_hit_tests;
#[cfg(all(test, target_os = "macos"))]
mod popup_navigation_tests;
#[cfg(all(test, target_os = "macos"))]
mod popup_scroll_tests;
#[cfg(all(test, target_os = "macos"))]
mod select_key_tests;
#[cfg(all(test, target_os = "macos"))]
mod tab_hit_tests;
#[cfg(all(test, target_os = "macos"))]
mod tool_command_object_tests;
#[cfg(all(test, target_os = "macos"))]
mod tool_hit_tests;
#[cfg(all(test, target_os = "macos"))]
mod undo_redo_key_tests;
// The test platform uses window activation for cursor ownership on macOS.
#[cfg(all(test, target_os = "macos"))]
mod header_wheel_tests;
#[cfg(all(test, target_os = "macos"))]
mod pointer_tests;
#[cfg(all(test, target_os = "macos"))]
mod pointer_window_tests;
#[cfg(all(test, target_os = "macos"))]
mod wheel_window_tests;
#[cfg(test)]
mod tooltip_position_tests;
use input::TextInputState;

// Colors (Tomorrow Night-ish, matching typical Emacs themes).
const FG: u32 = 0xc5c8c6;
const BG: u32 = 0x1d1f21;
const CURSOR: u32 = 0xc5c8c6;

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 19.0;
const POINTS_TO_LOGICAL_PIXELS: f32 = if cfg!(any(target_os = "linux", target_os = "windows")) {
    96.0 / 72.0
} else {
    1.0
};

fn point_size_to_logical_pixels(height_tenths: u16, pixels_per_point: f32) -> Pixels {
    px(height_tenths as f32 / 10.0 * pixels_per_point)
}

pub(super) fn face_font_size(height_tenths: u16) -> Pixels {
    point_size_to_logical_pixels(height_tenths, POINTS_TO_LOGICAL_PIXELS)
}

#[cfg(test)]
mod face_font_size_tests {
    use super::face_font_size;

    #[test]
    fn face_height_uses_platform_logical_pixel_scale() {
        let expected = if cfg!(any(target_os = "linux", target_os = "windows")) {
            16.0
        } else {
            12.0
        };
        let actual = f32::from(face_font_size(120));
        assert!(
            (actual - expected).abs() < 0.01,
            "got {actual}, expected {expected}"
        );
    }
}

static FONT_FAMILY_NAMES: OnceLock<Mutex<Option<HashSet<String>>>> = OnceLock::new();

fn query_font_family_names(window: &mut Window) -> HashSet<String> {
    window
        .text_system()
        .all_font_names()
        .into_iter()
        .map(|family| family.to_lowercase())
        .collect()
}

pub(super) fn refresh_font_family_names_cache(window: &mut Window) {
    let mut cached = FONT_FAMILY_NAMES
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *cached = Some(query_font_family_names(window));
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AccessibilityFrameKey {
    left: i128,
    top: i128,
    width: i128,
    height: i128,
    scale_factor_bits: u64,
    display_id: Option<u32>,
}

#[cfg(target_os = "macos")]
fn accessibility_frame_key(geometry: &GuiGeometry) -> AccessibilityFrameKey {
    AccessibilityFrameKey {
        left: geometry.frame_left,
        top: geometry.frame_top,
        width: geometry.pixel_width,
        height: geometry.pixel_height,
        scale_factor_bits: geometry.scale_factor.to_bits(),
        display_id: geometry.current_display_id,
    }
}

pub(super) fn font_family(window: &mut Window, requested: Option<String>) -> String {
    let mut cached = FONT_FAMILY_NAMES
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let available = cached.get_or_insert_with(|| query_font_family_names(window));
    if let Some(requested) = requested
        && available.contains(&requested.to_lowercase())
    {
        return requested;
    }

    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "Menlo",
            "SF Mono",
            "Monaco",
            "Courier New",
            "Lilex",
            "DejaVu Sans Mono",
        ]
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

pub(super) fn frame_font_metrics(
    grid: &Grid,
    window: &mut Window,
) -> (Font, Pixels, Pixels, Pixels) {
    let default_style = grid.default_style;
    let requested_family = default_style
        .font_family
        .and_then(|id| grid.font_families.get(id as usize))
        .cloned();
    let family = font_family(window, requested_family);
    let mut text_font = font(family);
    if let Some(weight) = default_style.font_weight {
        text_font.weight = FontWeight(weight as f32);
    } else if default_style.bold {
        text_font = text_font.bold();
    }
    if default_style.italic {
        text_font = text_font.italic();
    }
    text_font.stretch = FontStretch::from_percentage(default_style.font_width.unwrap_or(100));
    let font_size = default_style
        .font_height
        .map(face_font_size)
        .unwrap_or(px(FONT_SIZE));
    let font_id = window.text_system().resolve_font(&text_font);
    let cell_width = window
        .text_system()
        .advance(font_id, font_size, '0')
        .map(|metrics| metrics.width)
        .unwrap_or(px(8.0));
    let line_height = px(f32::from(font_size) * (LINE_HEIGHT / FONT_SIZE));
    (text_font, font_size, cell_width, line_height)
}

fn font_style_for_grid(grid: &Grid, style: CellStyle) -> FontAtStyle {
    FontAtStyle {
        family: style
            .font_family
            .and_then(|id| grid.font_families.get(id as usize).cloned()),
        weight: style.font_weight,
        width: style.font_width,
        height: style.font_height,
        bold: style.bold,
        italic: style.italic,
    }
}

fn font_for_grid_style(
    default_font: &Font,
    style: &FontAtStyle,
    window: &mut Window,
) -> Font {
    let mut font = default_font.clone();
    if let Some(family) = &style.family {
        font.family = font_family(window, Some(family.clone())).into();
    }
    if let Some(weight) = style.weight {
        font.weight = FontWeight(weight as f32);
    } else if style.bold {
        font = font.bold();
    }
    if let Some(width) = style.width {
        font.stretch = FontStretch::from_percentage(width);
    }
    if style.italic {
        font = font.italic();
    }
    font
}

fn pixel_width_64(width: Pixels) -> u32 {
    (f32::from(width).max(0.0) * 64.0)
        .round()
        .clamp(0.0, u32::MAX as f32) as u32
}

fn measure_frame_font_metrics(
    grid: &Grid,
    default_font: &Font,
    default_font_size: Pixels,
    default_cell_width: Pixels,
    window: &mut Window,
) -> FrameFontMetrics {
    let default_style = font_style_for_grid(grid, grid.default_style);
    let mut signature = DefaultHasher::new();
    let resolved_default_family = font_family(window, default_style.family.clone());
    resolved_default_family.hash(&mut signature);
    default_style.hash(&mut signature);
    f32::from(default_font_size).to_bits().hash(&mut signature);
    f32::from(default_cell_width).to_bits().hash(&mut signature);
    let signature = signature.finish();

    let mut glyph_widths_64 = HashMap::new();
    let mut fonts = HashMap::<FontAtStyle, (Font, Pixels)>::new();
    for (row_index, row) in grid.rows.iter().enumerate() {
        let styles = grid
            .char_attrs
            .get(row_index)
            .filter(|styles| !styles.is_empty())
            .or_else(|| grid.attrs.get(row_index));
        for (column, character) in row.chars().enumerate() {
            let cell_style = styles
                .and_then(|styles| styles.get(column))
                .copied()
                .unwrap_or_default();
            let style = font_style_for_grid(grid, cell_style);
            let key = FontGlyphKey {
                style: style.clone(),
                character,
            };
            if glyph_widths_64.contains_key(&key) {
                continue;
            }
            let (font, font_size) = fonts.entry(style.clone()).or_insert_with(|| {
                let font = font_for_grid_style(default_font, &style, window);
                let font_size = style.height.map(face_font_size).unwrap_or(default_font_size);
                (font, font_size)
            }).clone();
            let font_id = window.text_system().resolve_font(&font);
            let width = window
                .text_system()
                .advance(font_id, font_size, character)
                .map(|metrics| metrics.width)
                .unwrap_or(default_cell_width);
            glyph_widths_64.insert(key, pixel_width_64(width));
        }
    }
    FrameFontMetrics {
        signature,
        default_cell_width_64: pixel_width_64(default_cell_width).max(1),
        glyph_widths_64,
    }
}

pub(super) fn font_info(
    grid: &Grid,
    window: &mut Window,
    name: &str,
) -> Option<remacs_runtime::frontend::FontInfo> {
    let opened_name = window
        .text_system()
        .all_font_names()
        .into_iter()
        .find(|family| family.eq_ignore_ascii_case(name))?;
    let font = font(opened_name.clone());
    let font_id = window.text_system().resolve_font(&font);
    let font_size = grid
        .default_style
        .font_height
        .map(face_font_size)
        .unwrap_or(px(FONT_SIZE));
    let bounds = window.text_system().bounding_box(font_id, font_size);
    let top = f32::from(bounds.origin.y);
    let bottom = top + f32::from(bounds.size.height);
    let ascent = (-top).max(0.0);
    let descent = bottom.max(0.0);
    let space_width = window
        .text_system()
        .advance(font_id, font_size, ' ')
        .map(|size| f32::from(size.width))
        .unwrap_or(0.0);
    let mut max_width: f32 = 0.0;
    let mut total_width: f32 = 0.0;
    let mut width_count = 0usize;
    for character in ' '..='~' {
        if let Ok(size) = window.text_system().advance(font_id, font_size, character) {
            let width = f32::from(size.width);
            max_width = max_width.max(width);
            total_width += width;
            width_count += 1;
        }
    }
    let average_width = if width_count == 0 {
        0.0
    } else {
        total_width / width_count as f32
    };
    let rounded = |value: f32| value.round() as i128;

    Some(remacs_runtime::frontend::FontInfo {
        opened_name: opened_name.clone(),
        full_name: opened_name,
        size: rounded(f32::from(font_size)),
        height: rounded(ascent + descent),
        baseline_offset: 0,
        relative_compose: 0,
        default_ascent: rounded(ascent),
        max_width: rounded(max_width),
        ascent: rounded(ascent),
        descent: rounded(descent),
        space_width: rounded(space_width),
        average_width: rounded(average_width),
        filename: None,
    })
}

pub(super) fn resolve_font_at(window: &mut Window, style: &FontAtStyle) -> Option<String> {
    let mut requested = font(font_family(window, style.family.clone()));
    if let Some(weight) = style.weight {
        requested.weight = FontWeight(weight as f32);
    } else if style.bold {
        requested = requested.bold();
    }
    requested.stretch = FontStretch::from_percentage(style.width.unwrap_or(100));
    if style.italic {
        requested = requested.italic();
    }
    let font_id = window.text_system().resolve_font(&requested);
    window
        .text_system()
        .get_font_for_id(font_id)
        .map(|resolved| resolved.family.to_string())
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

fn gui_display_char_width(ch: char) -> usize {
    if matches!(ch as u32, 0x80..=0x9f) {
        4
    } else {
        UnicodeWidthChar::width(ch).unwrap_or(1)
    }
}

pub(super) fn align_zwj_glyph_positions<F>(
    mut line: ShapedLine,
    row: &str,
    cell_width: Pixels,
    layout_index_for_byte: F,
) -> ShapedLine
where
    F: Fn(usize) -> usize,
{
    if cell_width <= px(0.0) || !row.contains('\u{200d}') {
        return line;
    }

    let characters: Vec<_> = row.char_indices().collect();
    let mut columns = 0usize;
    let mut in_joined_sequence = false;
    for (index, (byte, character)) in characters.iter().enumerate() {
        if *character == '\u{200d}' {
            in_joined_sequence = true;
            continue;
        }
        columns = columns.saturating_add(gui_display_char_width(*character));
        if !in_joined_sequence {
            continue;
        }
        let next_character = characters.get(index + 1).map(|(_, character)| *character);
        if next_character.is_some_and(|next| {
            next == '\u{200d}' || gui_display_char_width(next) == 0
        }) {
            continue;
        }

        let byte_end = *byte + character.len_utf8();
        let layout_index = layout_index_for_byte(byte_end);
        let target_x = cell_width * columns as f32;
        let delta = target_x - line.x_for_index(layout_index);
        if f32::from(delta).abs() > 1.0 {
            line.shift_glyphs_from_index(layout_index, delta);
        }
        in_joined_sequence = false;
    }
    line
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
    row_top_from_heights(&grid.row_heights, grid.default_line_height, row, fallback)
}

pub(super) fn grid_row_height(grid: &Grid, row: usize, fallback: Pixels) -> Pixels {
    row_height_from_heights(&grid.row_heights, grid.default_line_height, row, fallback)
}

fn grid_window_row_metrics(grid: &Grid, window_id: usize) -> Option<&GridWindowRowHeights> {
    grid.window_row_heights
        .iter()
        .find(|metrics| metrics.window_id == window_id)
}

fn grid_window_row_top(
    grid: &Grid,
    window_id: usize,
    row: usize,
    fallback: Pixels,
) -> Pixels {
    let Some(metrics) = grid_window_row_metrics(grid, window_id) else {
        return grid_row_top(grid, row, fallback);
    };
    if row < metrics.top {
        return grid_row_top(grid, row, fallback);
    }
    grid_row_top(grid, metrics.top, fallback)
        + row_top_from_heights(
            &metrics.heights,
            grid.default_line_height,
            row.saturating_sub(metrics.top),
            fallback,
        )
}

fn grid_window_row_height(
    grid: &Grid,
    window_id: usize,
    row: usize,
    fallback: Pixels,
) -> Pixels {
    let Some(metrics) = grid_window_row_metrics(grid, window_id) else {
        return grid_row_height(grid, row, fallback);
    };
    if row < metrics.top {
        return grid_row_height(grid, row, fallback);
    }
    row_height_from_heights(
        &metrics.heights,
        grid.default_line_height,
        row.saturating_sub(metrics.top),
        fallback,
    )
}

fn scroll_row_height(
    grid: &Grid,
    window_id: Option<usize>,
    row: usize,
    fallback: Pixels,
) -> Pixels {
    window_id
        .map(|window_id| grid_window_row_height(grid, window_id, row, fallback))
        .unwrap_or_else(|| grid_row_height(grid, row, fallback))
        .max(px(1.0))
}

fn wheel_delta_lines(delta: ScrollDelta, row_height: Pixels) -> f32 {
    match delta {
        ScrollDelta::Pixels(delta) => -f32::from(delta.y) / f32::from(row_height.max(px(1.0))),
        ScrollDelta::Lines(delta) => -delta.y,
    }
}

#[cfg(test)]
mod wheel_delta_line_spacing_tests {
    use super::{scroll_row_height, wheel_delta_lines};
    use gpui::{ScrollDelta, point, px};
    use remacs_runtime::frontend::Grid;

    #[test]
    fn pixel_deltas_use_the_target_window_row_height() {
        let mut grid = Grid::blank(20, 4);
        grid.default_line_height = 19;
        grid.row_heights = vec![31; 4];
        grid.window_row_heights.push(super::GridWindowRowHeights {
            window_id: 7,
            top: 0,
            heights: vec![31; 4],
            text_offsets: vec![0; 4],
        });

        let row_height = scroll_row_height(&grid, Some(7), 1, px(19.0));
        assert_eq!(row_height, px(31.0));
        assert_eq!(
            wheel_delta_lines(
                ScrollDelta::Pixels(point(px(0.0), px(-31.0))),
                row_height,
            ),
            1.0
        );
        assert_eq!(
            wheel_delta_lines(ScrollDelta::Lines(point(0.0, -1.5)), row_height),
            1.5
        );
    }
}

fn grid_window_row_text_offset(grid: &Grid, window_id: usize, row: usize) -> Pixels {
    let Some(metrics) = grid_window_row_metrics(grid, window_id) else {
        return px(0.0);
    };
    if row < metrics.top {
        return px(0.0);
    }
    metrics
        .text_offsets
        .get(row.saturating_sub(metrics.top))
        .map(|offset| px(*offset as f32))
        .unwrap_or(px(0.0))
}

fn grid_row_top_for_cell(
    grid: &Grid,
    row: usize,
    column: usize,
    fallback: Pixels,
) -> Pixels {
    grid.window_text_areas
        .iter()
        .find(|area| {
            row >= area.top
                && row < area.top.saturating_add(area.rows)
                && column >= area.left.saturating_sub(area.left_fringe)
                && column
                    < area
                        .left
                        .saturating_add(area.width)
                        .saturating_add(area.right_fringe)
        })
        .map_or_else(
            || grid_row_top(grid, row, fallback),
            |area| {
                grid_window_row_top(grid, area.window_id, row, fallback)
                    + grid_window_row_text_offset(grid, area.window_id, row)
            },
        )
}

fn grid_row_height_for_cell(
    grid: &Grid,
    row: usize,
    column: usize,
    fallback: Pixels,
) -> Pixels {
    grid.window_text_areas
        .iter()
        .find(|area| {
            row >= area.top
                && row < area.top.saturating_add(area.rows)
                && column >= area.left.saturating_sub(area.left_fringe)
                && column
                    < area
                        .left
                        .saturating_add(area.width)
                        .saturating_add(area.right_fringe)
        })
        .map_or_else(
            || grid_row_height(grid, row, fallback),
            |area| {
                (grid_window_row_height(grid, area.window_id, row, fallback)
                    - grid_window_row_text_offset(grid, area.window_id, row))
                .max(px(1.0))
            },
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
    let top = bounds.top() + grid_window_row_top(grid, scroll.window_id, first_row, line_height);
    let bottom =
        bounds.top() + grid_window_row_top(grid, scroll.window_id, end_row, line_height);
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

fn unscrolled_column_ranges(grid: &Grid, row: usize, width: usize) -> Vec<(usize, usize)> {
    let mut covered = grid
        .vscrolls
        .iter()
        .filter(|scroll| row >= scroll.top && row < scroll.top.saturating_add(scroll.rows))
        .filter_map(|scroll| {
            let left = scroll.left.min(width);
            let right = scroll.left.saturating_add(scroll.width).min(width);
            (left < right).then_some((left, right))
        })
        .collect::<Vec<_>>();
    covered.sort_unstable_by_key(|(left, _)| *left);
    let mut ranges = Vec::new();
    let mut left = 0usize;
    for (covered_left, covered_right) in covered {
        if covered_left > left {
            ranges.push((left, covered_left));
        }
        left = left.max(covered_right);
    }
    if left < width {
        ranges.push((left, width));
    }
    ranges
}

#[derive(Clone, Copy)]
struct ScrollBarDrag {
    window_id: usize,
    orientation: GridScrollBarOrientation,
    pointer_offset: f32,
    start_position: MousePosition,
    click_count: usize,
    modifiers: i128,
    dragged: bool,
}

#[derive(Clone, Copy)]
struct TabBarDrag {
    from_tab: i128,
    dragged: bool,
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

fn device_pixels(value: Pixels, scale_factor: f32) -> i128 {
    (f32::from(value) * scale_factor).round() as i128
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TooltipPlacement {
    left: Option<Pixels>,
    right: Option<Pixels>,
    top: Option<Pixels>,
    bottom: Option<Pixels>,
}

fn tooltip_placement(
    position: GridTooltipPosition,
    frame_left: i128,
    frame_top: i128,
    content_top_offset: i128,
    viewport: Size<Pixels>,
    pointer: Point<Pixels>,
) -> TooltipPlacement {
    let left = position
        .left
        .map(|left| px(left.saturating_sub(frame_left) as f32));
    let right = if left.is_none() {
        position
            .right
            .map(|right| viewport.width - px(right.saturating_sub(frame_left) as f32))
    } else {
        None
    };
    let left = left.or_else(|| {
        right
            .is_none()
            .then(|| pointer.x + px(position.dx as f32))
    });

    let top = position.top.map(|top| {
        px(top
            .saturating_sub(frame_top)
            .saturating_sub(content_top_offset) as f32)
    });
    let bottom = if top.is_none() {
        position.bottom.map(|bottom| {
            viewport.height
                - px(bottom
                    .saturating_sub(frame_top)
                    .saturating_sub(content_top_offset) as f32)
        })
    } else {
        None
    };
    let top = top.or_else(|| {
        bottom
            .is_none()
            .then(|| pointer.y + px(position.dy as f32))
    });

    TooltipPlacement {
        left,
        right,
        top,
        bottom,
    }
}

#[cfg(test)]
mod device_pixel_tests {
    use super::device_pixels;
    use gpui::px;

    #[test]
    fn logical_frame_dimensions_convert_to_device_pixels() {
        assert_eq!(device_pixels(px(960.0), 2.0), 1920);
        assert_eq!(device_pixels(px(640.0), 2.0), 1280);
        assert_eq!(device_pixels(px(8.0), 2.0), 16);
    }
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

fn scroll_bar_mouse_position(
    bar: &GridScrollBar,
    point: Point<Pixels>,
    pointer_offset: f32,
    scale_factor: f32,
) -> MousePosition {
    let offset = scroll_bar_offset(bar, point, pointer_offset);
    MousePosition {
        buffer_id: bar.buffer_id,
        buffer_position: 0,
        utf16_offset: 0,
        x: device_pixels(point.x - px(bar.left as f32), scale_factor),
        y: device_pixels(point.y - px(bar.top as f32), scale_factor),
        row: if bar.orientation == GridScrollBarOrientation::Vertical {
            offset
        } else {
            0
        },
        column: if bar.orientation == GridScrollBarOrientation::Horizontal {
            offset
        } else {
            0
        },
        window_id: Some(bar.window_id),
        area: Some(match bar.orientation {
            GridScrollBarOrientation::Vertical => MouseArea::VerticalScrollBar,
            GridScrollBarOrientation::Horizontal => MouseArea::HorizontalScrollBar,
        }),
        object: None,
        timestamp: input::mouse_timestamp_ms(),
        mode_line_buffer_name: false,
    }
}

fn send_scroll_bar_event(
    tx: &FrameEventSender,
    bar: &GridScrollBar,
    point: Point<Pixels>,
    pointer_offset: f32,
    mouse: Option<MouseInput>,
) {
    let _ = tx.send(GuiEvent::ScrollBar {
        window_id: bar.window_id,
        orientation: bar.orientation,
        offset: scroll_bar_offset(bar, point, pointer_offset),
        max_offset: bar.max_offset,
        text_width: bar.text_width,
        mouse,
    });
}

fn gpui_cursor_style(shape: Option<GridPointerShape>) -> CursorStyle {
    match shape {
        Some(GridPointerShape::Text) => CursorStyle::IBeam,
        Some(GridPointerShape::Hand) => CursorStyle::PointingHand,
        Some(GridPointerShape::Hourglass) => CursorStyle::Wait,
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
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        file_drop_uri_from_bytes(absolute.as_os_str().as_bytes())
    }
    #[cfg(not(unix))]
    {
        file_drop_uri_from_path(&absolute.to_string_lossy())
    }
}

fn append_file_uri_bytes(uri: &mut String, component: &[u8], authority: bool) {
    for &byte in component {
        let safe_component_byte = if authority {
            matches!(byte, b':' | b'[' | b']')
        } else {
            matches!(byte, b'/' | b':')
        };
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'~')
            || safe_component_byte
        {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
}

fn append_file_uri_component(uri: &mut String, component: &str, authority: bool) {
    append_file_uri_bytes(uri, component.as_bytes(), authority);
}

fn file_drop_uri_from_bytes(path: &[u8]) -> String {
    let mut uri = String::from("file://");
    append_file_uri_bytes(&mut uri, path, false);
    uri
}

fn file_drop_uri_from_path(path: &str) -> String {
    if let Some(unc_path) = path.strip_prefix(r"\\") {
        let normalized = unc_path.replace('\\', "/");
        if let Some((authority, share_path)) = normalized.split_once('/') {
            let share_path = share_path.trim_start_matches('/');
            if !authority.is_empty() && !share_path.is_empty() {
                let mut uri = String::from("file://");
                append_file_uri_component(&mut uri, authority, true);
                uri.push('/');
                append_file_uri_component(&mut uri, share_path, false);
                return uri;
            }
        }
    }

    let bytes = path.as_bytes();
    let windows_drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    let path = if windows_drive_path {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    let mut uri = if windows_drive_path {
        String::from("file:///")
    } else {
        String::from("file://")
    };
    append_file_uri_component(&mut uri, &path, false);
    uri
}

#[derive(Clone)]
struct CommandHitRegion {
    bounds: Bounds<Pixels>,
    row: usize,
    command: String,
    command_id: Option<NativeMenuCommandId>,
    argument: Option<i128>,
}

#[derive(PartialEq)]
enum ButtonAction {
    Tool {
        command: String,
        command_id: Option<NativeMenuCommandId>,
    },
    Tab {
        command: String,
        command_id: Option<NativeMenuCommandId>,
        argument: Option<i128>,
    },
    Popup {
        menu_id: usize,
        parent: Option<usize>,
        index: usize,
    },
}

#[derive(PartialEq)]
struct PressedButton {
    bounds: Bounds<Pixels>,
    action: ButtonAction,
}

impl PressedButton {
    fn cancel_event(self) -> Option<GuiEvent> {
        match self.action {
            ButtonAction::Popup { menu_id, .. } => Some(GuiEvent::PopupMenuSelection {
                menu_id,
                index: None,
            }),
            _ => None,
        }
    }

    fn into_event(self) -> GuiEvent {
        match self.action {
            ButtonAction::Tool {
                command,
                command_id,
            } => match command_id {
                Some(command_id) => GuiEvent::NativeMenuCommand(command_id),
                None => GuiEvent::MenuCommand(command),
            },
            ButtonAction::Tab {
                command_id: Some(command_id),
                ..
            } => GuiEvent::NativeMenuCommand(command_id),
            ButtonAction::Tab {
                command,
                argument,
                ..
            } => GuiEvent::CommandArgs {
                command,
                args: argument.into_iter().collect(),
            },
            ButtonAction::Popup { menu_id, index, .. } => GuiEvent::PopupMenuSelection {
                menu_id,
                index: Some(index),
            },
        }
    }
}

fn grid_text_range_bounds(
    grid: &Grid,
    lines: &[ShapedLine],
    row: usize,
    range: (usize, usize),
    line_height: Pixels,
) -> Option<Bounds<Pixels>> {
    let (start, end) = range;
    let text = grid.rows.get(row)?;
    let line = lines.get(row)?;
    let selected = text.get(start..end)?;
    let mut left = grid_line_x(grid, row, line, start);
    let mut right = left;
    for byte in selected
        .char_indices()
        .map(|(byte, _)| start + byte)
        .chain(std::iter::once(end))
    {
        let x = grid_line_x(grid, row, line, byte);
        left = left.min(x);
        right = right.max(x);
    }
    (right > left).then(|| Bounds {
        origin: point(left, grid_row_top(grid, row, line_height)),
        size: size(right - left, grid_row_height(grid, row, line_height)),
    })
}

fn grid_cell_column_byte_index(text: &str, column: usize) -> usize {
    let mut current_column = 0usize;
    for (byte_index, character) in text.char_indices() {
        if current_column >= column {
            return byte_index;
        }
        let width = if character == '\t' {
            8 - current_column % 8
        } else {
            gui_display_char_width(character)
        };
        current_column = current_column.saturating_add(width);
        if current_column >= column {
            return byte_index + character.len_utf8();
        }
    }
    text.len()
}

fn grid_cell_range_bounds(
    grid: &Grid,
    lines: &[ShapedLine],
    row: usize,
    left_column: usize,
    width: usize,
    line_height: Pixels,
) -> Option<Bounds<Pixels>> {
    let text = grid.rows.get(row)?;
    let line = lines.get(row)?;
    let start = grid_cell_column_byte_index(text, left_column);
    let end = grid_cell_column_byte_index(text, left_column.saturating_add(width));
    let start_x = grid_line_x(grid, row, line, start);
    let end_x = grid_line_x(grid, row, line, end);
    let left = start_x.min(end_x);
    let right = start_x.max(end_x);
    let height = grid_row_height(grid, row, line_height);
    (right > left && height > px(0.0)).then(|| Bounds {
        origin: point(left, grid_row_top(grid, row, line_height)),
        size: size(right - left, height),
    })
}

fn tool_bar_hit_regions(
    grid: &Grid,
    lines: &[ShapedLine],
    line_height: Pixels,
) -> Vec<CommandHitRegion> {
    grid.tool_bar_items
        .iter()
        .filter_map(|item| {
            if !item.enabled || (item.command.is_empty() && item.command_id.is_none()) {
                return None;
            }
            Some(CommandHitRegion {
                bounds: grid_text_range_bounds(
                    grid,
                    lines,
                    item.row,
                    item.byte_range?,
                    line_height,
                )?,
                row: item.row,
                command: item.command.clone(),
                command_id: item.command_id.clone(),
                argument: None,
            })
        })
        .collect()
}

fn tab_bar_hit_regions(
    grid: &Grid,
    lines: &[ShapedLine],
    line_height: Pixels,
) -> Vec<CommandHitRegion> {
    grid.tab_bar_items
        .iter()
        .filter_map(|item| {
            Some(CommandHitRegion {
                bounds: item.byte_range.map_or_else(
                    || {
                        grid_cell_range_bounds(
                            grid,
                            lines,
                            item.row,
                            item.left,
                            item.width,
                            line_height,
                        )
                    },
                    |range| {
                        grid_text_range_bounds(grid, lines, item.row, range, line_height)
                    },
                )?,
                row: item.row,
                command: item.command.clone(),
                command_id: item.command_id.clone(),
                argument: item.argument,
            })
        })
        .collect()
}

#[derive(Clone)]
struct PopupMenuHitRegion {
    menu_id: usize,
    parent: Option<usize>,
    selection: Option<usize>,
    bounds: Bounds<Pixels>,
}

fn popup_menu_hit_regions(
    grid: &Grid,
    lines: &[ShapedLine],
    line_height: Pixels,
) -> Vec<PopupMenuHitRegion> {
    grid.popup_menu_items
        .iter()
        .filter_map(|item| {
            Some(PopupMenuHitRegion {
                menu_id: item.menu_id,
                parent: item.parent,
                selection: item.selection,
                bounds: grid_text_range_bounds(
                    grid,
                    lines,
                    item.row,
                    item.byte_range?,
                    line_height,
                )?,
            })
        })
        .collect()
}

fn popup_menu_event_at(items: &[PopupMenuHitRegion], point: Point<Pixels>) -> Option<GuiEvent> {
    let menu_id = items.first()?.menu_id;
    let hit = items.iter().find(|item| {
        point.x >= item.bounds.left()
            && point.x < item.bounds.right()
            && point.y >= item.bounds.top()
            && point.y < item.bounds.bottom()
    });
    match hit {
        Some(item) => item.selection.map(|index| GuiEvent::PopupMenuSelection {
            menu_id,
            index: Some(index),
        }),
        None => Some(GuiEvent::PopupMenuSelection {
            menu_id,
            index: None,
        }),
    }
}

fn popup_menu_hover_event_at(
    items: &[PopupMenuHitRegion],
    point: Point<Pixels>,
) -> Option<GuiEvent> {
    let hit = items.iter().rev().find(|item| {
        point.x >= item.bounds.left()
            && point.x < item.bounds.right()
            && point.y >= item.bounds.top()
            && point.y < item.bounds.bottom()
    })?;
    Some(GuiEvent::PopupMenuHover {
        menu_id: hit.menu_id,
        parent: hit.parent,
        index: hit.selection,
    })
}

#[cfg(test)]
mod popup_selection_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn popup_click_returns_a_choice_or_dismissal_without_a_command() {
        let items = [
            PopupMenuHitRegion {
                parent: None,
                menu_id: 23,
                selection: Some(4),
                bounds: Bounds {
                    origin: point(px(32.0), px(40.0)),
                    size: size(px(80.0), px(20.0)),
                },
            },
            PopupMenuHitRegion {
                parent: None,
                menu_id: 23,
                selection: None,
                bounds: Bounds {
                    origin: point(px(32.0), px(60.0)),
                    size: size(px(80.0), px(20.0)),
                },
            },
        ];
        assert!(matches!(
            popup_menu_event_at(&items, point(px(40.0), px(40.0))),
            Some(GuiEvent::PopupMenuSelection {
                menu_id: 23,
                index: Some(4)
            })
        ));
        assert!(matches!(
            popup_menu_event_at(&items, point(px(0.0), px(0.0))),
            Some(GuiEvent::PopupMenuSelection {
                menu_id: 23,
                index: None
            })
        ));
        assert!(popup_menu_event_at(&items, point(px(40.0), px(60.0))).is_none());
        assert!(popup_menu_event_at(&[], point(px(0.0), px(0.0))).is_none());
    }
}

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = remacs, no_json)]
struct RunMenuCommand {
    command: NativeMenuAction,
}

#[derive(Clone, PartialEq)]
enum NativeMenuAction {
    Named(String),
    Object(WeakNativeMenuCommandId),
}

impl RunMenuCommand {
    fn from_binding(name: Option<&str>, token: Option<&NativeMenuCommandId>) -> Option<Self> {
        let command = if let Some(token) = token {
            NativeMenuAction::Object(token.downgrade())
        } else {
            NativeMenuAction::Named(name?.to_owned())
        };
        Some(Self { command })
    }

    fn event(&self) -> Option<GuiEvent> {
        match &self.command {
            NativeMenuAction::Named(name) => Some(GuiEvent::MenuCommand(name.clone())),
            NativeMenuAction::Object(token) => token.upgrade().map(GuiEvent::NativeMenuCommand),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RuntimeFramePosition {
    left: Option<FramePosition>,
    top: Option<FramePosition>,
    parent_bounds: Option<(u32, u32, u32, u32)>,
    frame_size: (u32, u32),
}

fn runtime_frame_position_changed(
    previous: &mut Option<RuntimeFramePosition>,
    left: Option<FramePosition>,
    top: Option<FramePosition>,
    parent_bounds: Option<&Bounds<Pixels>>,
    frame_size: Size<Pixels>,
) -> bool {
    let parent_bounds = parent_bounds.map(|bounds| {
        (
            f32::from(bounds.origin.x).to_bits(),
            f32::from(bounds.origin.y).to_bits(),
            f32::from(bounds.size.width).to_bits(),
            f32::from(bounds.size.height).to_bits(),
        )
    });
    let position = RuntimeFramePosition {
        left,
        top,
        parent_bounds,
        frame_size: (
            f32::from(frame_size.width).to_bits(),
            f32::from(frame_size.height).to_bits(),
        ),
    };
    let changed = *previous != Some(position);
    *previous = Some(position);
    changed
}

#[cfg(test)]
mod runtime_frame_position_tests {
    use super::{FramePosition, runtime_frame_position_changed};
    use gpui::{Bounds, Pixels, point, px, size};

    fn bounds(left: f32, top: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(left), px(top)),
            size: size(px(width), px(height)),
        }
    }

    #[test]
    fn moving_or_resizing_parent_invalidates_child_position_cache() {
        let mut cache = None;
        let initial = bounds(10.0, 20.0, 400.0, 300.0);
        let moved = bounds(110.0, 120.0, 400.0, 300.0);
        let resized = bounds(110.0, 120.0, 500.0, 300.0);

        assert!(runtime_frame_position_changed(
            &mut cache,
            Some(FramePosition::Pixels(20)),
            Some(FramePosition::Pixels(20)),
            Some(&initial),
            size(px(100.0), px(80.0)),
        ));
        assert!(!runtime_frame_position_changed(
            &mut cache,
            Some(FramePosition::Pixels(20)),
            Some(FramePosition::Pixels(20)),
            Some(&initial),
            size(px(100.0), px(80.0)),
        ));
        assert!(runtime_frame_position_changed(
            &mut cache,
            Some(FramePosition::Pixels(20)),
            Some(FramePosition::Pixels(20)),
            Some(&moved),
            size(px(100.0), px(80.0)),
        ));
        assert!(runtime_frame_position_changed(
            &mut cache,
            Some(FramePosition::Pixels(20)),
            Some(FramePosition::Pixels(20)),
            Some(&resized),
            size(px(100.0), px(80.0)),
        ));
        assert!(runtime_frame_position_changed(
            &mut cache,
            Some(FramePosition::Pixels(20)),
            Some(FramePosition::Pixels(20)),
            Some(&resized),
            size(px(120.0), px(80.0)),
        ));
    }
}

pub(super) struct EditorView {
    tx: FrameEventSender,
    image_cache_entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_epochs: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_size: Arc<AtomicUsize>,
    pub(super) focus: FocusHandle,
    pub(super) grid: Grid,
    popup_hit_regions: Rc<RefCell<Vec<PopupMenuHitRegion>>>,
    tab_hit_regions: Rc<RefCell<Vec<CommandHitRegion>>>,
    tool_hit_regions: Rc<RefCell<Vec<CommandHitRegion>>>,
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
    minimum_cells: (Option<usize>, Option<usize>),
    native_minimum_size: Option<(Size<Pixels>, f32)>,
    geometry: Option<GuiGeometry>,
    runtime_frame_size: Option<(usize, usize, usize, usize)>,
    runtime_frame_position: Option<RuntimeFramePosition>,
    font_metrics_cache: Option<FrameFontMetrics>,
    sent_font_metrics_frame_id: Option<usize>,
    scroll_bar_drag: Option<ScrollBarDrag>,
    tab_bar_drag: Option<TabBarDrag>,
    follow_tooltip_drag: bool,
    mouse_down_positions: HashMap<MouseButton, MousePosition>,
    mouse_position_poll_started: bool,
    consumed_mouse_buttons: HashSet<MouseButton>,
    pressed_button: Option<PressedButton>,
    middle_tab_close_index: Option<i128>,
    button_activation_subscription: Option<Subscription>,
    bell_flash: bool,
    bell_flash_generation: u64,
    focus_subscriptions: Vec<Subscription>,
    appearance_subscription: Option<Subscription>,
    mouse_face_position: Rc<Cell<Option<(usize, usize)>>>,
    no_accept_focus: Rc<Cell<bool>>,
    auto_raise: Rc<Cell<bool>>,
    auto_lower: Rc<Cell<bool>>,
    last_window_active: Option<bool>,
    native_close_on_alt_f4: Rc<Cell<bool>>,
    native_close_on_command_w: Rc<Cell<bool>>,
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
    #[cfg(target_os = "macos")]
    accessibility_geometry_snapshot: Option<AccessibilityFrameKey>,
    #[cfg(any(target_os = "linux", target_os = "windows", test))]
    accessibility_tree_snapshot: Option<(String, String, String, std::ops::Range<usize>)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FrameActivationUpdate {
    raise: bool,
    lower: bool,
    focus: Option<bool>,
}

fn frame_activation_update(
    previous: Option<bool>,
    active: bool,
    auto_raise: bool,
    auto_lower: bool,
) -> FrameActivationUpdate {
    let changed = previous != Some(active);
    FrameActivationUpdate {
        raise: changed && active && auto_raise,
        lower: previous == Some(true) && !active && auto_lower,
        focus: (changed && (previous.is_some() || active)).then_some(active),
    }
}

impl EditorView {
    pub(super) fn set_grid(&mut self, grid: Grid, window: &mut Window) {
        if self
            .input
            .composition_buffer_changed(grid.input.as_ref().map(|input| input.buffer_id))
        {
            self.input.discard_stale_composition();
        }
        self.grid = grid;
        // Minimized windows may not render again until they are restored.
        self.update_window_title(window);
    }

    fn publish_frame_font_metrics(
        &mut self,
        default_font: &Font,
        default_font_size: Pixels,
        default_cell_width: Pixels,
        window: &mut Window,
    ) {
        let Some(frame_id) = self.tx.frame_id() else {
            return;
        };
        let measured = measure_frame_font_metrics(
            &self.grid,
            default_font,
            default_font_size,
            default_cell_width,
            window,
        );
        let mut changed = false;
        if self
            .font_metrics_cache
            .as_ref()
            .is_none_or(|cached| cached.signature != measured.signature)
        {
            self.font_metrics_cache = Some(measured);
            changed = true;
        } else if let Some(cached) = self.font_metrics_cache.as_mut() {
            for (key, width) in measured.glyph_widths_64 {
                if cached.glyph_widths_64.get(&key) != Some(&width) {
                    cached.glyph_widths_64.insert(key, width);
                    changed = true;
                }
            }
        }
        if self.sent_font_metrics_frame_id != Some(frame_id) {
            changed = true;
        }
        if changed
            && let Some(metrics) = self.font_metrics_cache.clone()
            && self
                .tx
                .send(GuiEvent::FrameFontMetrics { frame_id, metrics })
                .is_ok()
        {
            self.sent_font_metrics_frame_id = Some(frame_id);
        }
    }

    fn update_window_title(&mut self, window: &mut Window) {
        if self.window_title.as_deref() != Some(self.grid.title.as_str()) {
            window.set_window_title(&self.grid.title);
            self.window_title = Some(self.grid.title.clone());
        }
    }

    pub(super) fn new(tx: Sender<GuiEvent>, cx: &mut Context<Self>) -> Self {
        Self::new_for_frame_with_quit(tx, None, None, cx)
    }

    pub(super) fn new_for_frame(
        tx: Sender<GuiEvent>,
        frame_id: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_for_frame_with_quit(tx, frame_id, None, cx)
    }

    fn new_for_frame_with_quit(
        tx: Sender<GuiEvent>,
        frame_id: Option<usize>,
        quit_request: Option<Arc<QuitRequest>>,
        cx: &mut Context<Self>,
    ) -> Self {
        EditorView {
            tx: FrameEventSender::new(tx, frame_id, quit_request),
            image_cache_entries: Arc::new(Mutex::new(HashMap::new())),
            image_cache_epochs: Arc::new(Mutex::new(HashMap::new())),
            image_cache_size: Arc::new(AtomicUsize::new(0)),
            focus: cx.focus_handle(),
            grid: Grid::blank(0, 0),
            popup_hit_regions: Rc::new(RefCell::new(Vec::new())),
            tab_hit_regions: Rc::new(RefCell::new(Vec::new())),
            tool_hit_regions: Rc::new(RefCell::new(Vec::new())),
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
            minimum_cells: (None, None),
            native_minimum_size: None,
            geometry: None,
            runtime_frame_size: None,
            runtime_frame_position: None,
            font_metrics_cache: None,
            sent_font_metrics_frame_id: None,
            scroll_bar_drag: None,
            tab_bar_drag: None,
            follow_tooltip_drag: false,
            mouse_down_positions: HashMap::new(),
            mouse_position_poll_started: false,
            consumed_mouse_buttons: HashSet::new(),
            pressed_button: None,
            middle_tab_close_index: None,
            button_activation_subscription: None,
            bell_flash: false,
            bell_flash_generation: 0,
            focus_subscriptions: Vec::new(),
            appearance_subscription: None,
            mouse_face_position: Rc::new(Cell::new(None)),
            no_accept_focus: Rc::new(Cell::new(false)),
            auto_raise: Rc::new(Cell::new(false)),
            auto_lower: Rc::new(Cell::new(false)),
            last_window_active: None,
            native_close_on_alt_f4: Rc::new(Cell::new(false)),
            native_close_on_command_w: Rc::new(Cell::new(false)),
            #[cfg(target_os = "macos")]
            accessibility_buffer_view: None,
            #[cfg(target_os = "macos")]
            accessibility_screen_view: None,
            #[cfg(target_os = "macos")]
            accessibility_snapshot: None,
            #[cfg(target_os = "macos")]
            accessibility_geometry_snapshot: None,
            #[cfg(any(target_os = "linux", target_os = "windows", test))]
            accessibility_tree_snapshot: None,
        }
    }

    pub(super) fn set_frame_id(&mut self, frame_id: usize) {
        self.tx.set_frame_id(frame_id);
        self.follow_tooltip_drag = false;
        self.resize_increments = None;
        self.native_minimum_size = None;
        self.mouse_face_position.set(None);
        self.geometry = None;
        self.runtime_frame_size = None;
        self.runtime_frame_position = None;
        #[cfg(target_os = "macos")]
        {
            self.accessibility_geometry_snapshot = None;
        }
        #[cfg(any(target_os = "linux", target_os = "windows", test))]
        {
            self.accessibility_tree_snapshot = None;
        }
    }

    pub(super) fn set_follow_tooltip_drag(&mut self, follow: bool) {
        self.follow_tooltip_drag = follow;
    }

    /// Snapshot the action and painted bounds together: two adjacent buttons
    /// can invoke the same command without being the same pressed item.
    fn button_at(&self, point: Point<Pixels>) -> Option<PressedButton> {
        let contains = |bounds: Bounds<Pixels>| {
            point.x >= bounds.left()
                && point.x < bounds.right()
                && point.y >= bounds.top()
                && point.y < bounds.bottom()
        };
        if !self.grid.popup_menu_items.is_empty() {
            let popup = self.popup_hit_regions.borrow();
            let item = popup.iter().rev().find(|item| contains(item.bounds))?;
            return Some(PressedButton {
                bounds: item.bounds,
                action: ButtonAction::Popup {
                    menu_id: item.menu_id,
                    parent: item.parent,
                    index: item.selection?,
                },
            });
        }
        if let Some(item) = self
            .tool_hit_regions
            .borrow()
            .iter()
            .find(|item| contains(item.bounds))
        {
            return Some(PressedButton {
                bounds: item.bounds,
                action: ButtonAction::Tool {
                    command: item.command.clone(),
                    command_id: item.command_id.clone(),
                },
            });
        }
        let tabs = self.tab_hit_regions.borrow();
        let item = tabs.iter().find(|item| contains(item.bounds))?;
        Some(PressedButton {
            bounds: item.bounds,
            action: ButtonAction::Tab {
                command: item.command.clone(),
                command_id: item.command_id.clone(),
                argument: item.argument,
            },
        })
    }

    pub(super) fn set_image_cache_metrics(
        &mut self,
        entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
        size: Arc<AtomicUsize>,
    ) {
        self.image_cache_entries = entries;
        self.image_cache_size = size;
    }

    pub(super) fn set_modifier_mapping(&mut self, mapping: ModifierMapping) -> bool {
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
        left: Option<FramePosition>,
        top: Option<FramePosition>,
        parent_bounds: Option<&Bounds<Pixels>>,
        frame_size: Size<Pixels>,
    ) -> bool {
        runtime_frame_position_changed(
            &mut self.runtime_frame_position,
            left,
            top,
            parent_bounds,
            frame_size,
        )
    }

    pub(super) fn resize_to_cells(&mut self, width: usize, height: usize, window: &mut Window) {
        let width = width.max(super::MIN_FRAME_COLUMNS);
        let height = height.max(3);
        let (_, _, cell_width, line_height) = frame_font_metrics(&self.grid, window);
        self.cell_width = cell_width;
        self.line_height = line_height;
        let minimum = self.update_minimum_size(window, self.cell_width, self.line_height);
        let viewport = window.viewport_size();
        let border = self.internal_border_width as f32 * 2.0;
        let pixel_height = (0..height)
            .map(|row| {
                f32::from(grid_row_height(
                    &self.grid,
                    row,
                    px(self.grid.default_row_height.max(1) as f32),
                ))
            })
            .sum::<f32>();
        let desired_width = (self.cell_width * width as f32 + px(border)).max(minimum.width);
        let desired_height = px(pixel_height + border).max(minimum.height);
        if (f32::from(viewport.width) - f32::from(desired_width)).abs() < 1.0
            && (f32::from(viewport.height) - f32::from(desired_height)).abs() < 1.0
        {
            return;
        }
        window.resize(size(desired_width, desired_height));
    }

    pub(super) fn set_minimum_cells(
        &mut self,
        width: Option<usize>,
        height: Option<usize>,
    ) -> bool {
        let cells = (width, height);
        if self.minimum_cells == cells {
            return false;
        }
        self.minimum_cells = cells;
        self.native_minimum_size = None;
        true
    }

    pub(super) fn apply_minimum_cells(
        &mut self,
        window: &Window,
        width: Option<usize>,
        height: Option<usize>,
    ) {
        self.set_minimum_cells(width, height);
        self.update_minimum_size(window, self.cell_width, self.line_height);
    }

    pub(super) fn update_minimum_size(
        &mut self,
        window: &Window,
        cell_width: Pixels,
        line_height: Pixels,
    ) -> Size<Pixels> {
        let minimum = super::frame_minimum_size(
            self.minimum_cells.0,
            self.minimum_cells.1,
            f32::from(cell_width),
            f32::from(line_height),
            self.internal_border_width as f32 * 2.0,
        )
        .expect("minimum grid width");
        let state = (minimum, window.scale_factor());
        if self.native_minimum_size != Some(state) {
            window.set_minimum_size(Some(minimum));
            self.native_minimum_size = Some(state);
        }
        minimum
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
        window.set_client_frame_id(self.tx.frame_id());
        if !self.mouse_position_poll_started {
            self.mouse_position_poll_started = true;
            cx.spawn_in(window, async move |this, cx| {
                let mut previous = None;
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;
                    let result = cx.update_root(|_, window, app| {
                        let mouse = window.mouse_position();
                        let viewport = window.viewport_size();
                        let bounds = window.bounds();
                        this.update(app, |view, _cx| {
                            let point = frame_content_point(
                                mouse,
                                viewport,
                                px(view.internal_border_width as f32),
                            );
                            let position = point.and_then(|point| {
                                input::mouse_position_for_text_point_with_cell_width(
                                    &view.grid,
                                    point,
                                    view.cell_width,
                                    window,
                                )
                            });
                            let text_window_id = point.and_then(|point| {
                                input::text_window_at_point(&view.grid, point, view.cell_width)
                            });
                            let screen_origin = view
                                .geometry
                                .as_ref()
                                .map(|geometry| (geometry.frame_left, geometry.frame_top))
                                .unwrap_or((0, 0));
                            let content_top_offset = (f32::from(bounds.size.height)
                                - f32::from(viewport.height))
                            .round() as i128;
                            let absolute_position = absolute_mouse_pixel_position(
                                mouse,
                                screen_origin.0,
                                screen_origin.1,
                                content_top_offset,
                            );
                            let current = (position, absolute_position, text_window_id);
                            if previous.is_some_and(|previous| previous != current) {
                                let _ = view.tx.send_mouse_move_for_window(
                                    position,
                                    Some(absolute_position),
                                    text_window_id,
                                    0,
                                );
                            }
                            previous = Some(current);
                        })
                    });
                    if !matches!(result, Ok(Ok(()))) {
                        break;
                    }
                }
            })
            .detach();
        }
        if self.button_activation_subscription.is_none() {
            self.button_activation_subscription =
                Some(cx.observe_window_activation(window, |view, window, _cx| {
                    let active = window.is_window_active();
                    if !active {
                        view.pressed_button = None;
                    }
                    if let Some(frame_id) = view.tx.frame_id() {
                        let update = frame_activation_update(
                            view.last_window_active,
                            active,
                            view.auto_raise.get(),
                            view.auto_lower.get(),
                        );
                        view.last_window_active = Some(active);
                        if update.raise {
                            window.activate_window();
                        } else if update.lower {
                            crate::lower_frame_window(window);
                        }
                        if let Some(focused) = update.focus {
                            let _ = view.tx.send_focus_update(frame_id, focused);
                        }
                    }
                }));
        }
        window.set_cursor_style_frozen(self.grid.freeze_pointer_shape);
        window.set_mouse_cursor_color(self.grid.mouse_color.map(|CellColor(r, g, b)| [r, g, b]));
        if self.grid.tooltip.is_none() {
            self.follow_tooltip_drag = false;
        }
        let active = window.is_window_active();
        if !active {
            self.physical_modifier_state
                .set(PhysicalModifierState::default());
        }
        if let Some(frame_id) = self.tx.frame_id() {
            let update = frame_activation_update(
                self.last_window_active,
                active,
                self.auto_raise.get(),
                self.auto_lower.get(),
            );
            self.last_window_active = Some(active);
            if update.raise {
                window.activate_window();
            } else if update.lower {
                crate::lower_frame_window(window);
            }
            if let Some(focused) = update.focus {
                let _ = self.tx.send_focus_update(frame_id, focused);
            }
        } else {
            self.last_window_active = None;
        }
        self.update_window_title(window);
        if self.installed_menus != self.grid.menus {
            cx.set_menu_bar_items(gpui_menus(&self.grid.menus));
            self.installed_menus = self.grid.menus.clone();
        }

        let font_families = self.grid.font_families.clone();
        let (text_font, font_size, cell_width, line_height) =
            frame_font_metrics(&self.grid, window);
        self.publish_frame_font_metrics(&text_font, font_size, cell_width, window);
        self.line_height = line_height;
        self.cell_width = cell_width;
        let minimum = self.update_minimum_size(window, cell_width, line_height);
        let resize_increments = if self.grid.frame_resize_pixelwise {
            (1.0, 1.0)
        } else {
            (
                f32::from(cell_width).max(1.0),
                f32::from(line_height).max(1.0),
            )
        };
        if self.resize_increments != Some(resize_increments) {
            window.set_resize_increments(size(px(resize_increments.0), px(resize_increments.1)));
            self.resize_increments = Some(resize_increments);
        }

        // Report grid-size changes to the logic thread.
        let vp = window.viewport_size();
        if vp.width < minimum.width || vp.height < minimum.height {
            // Minimum hints constrain interactive resizing, but a font change
            // or programmatic resize can also leave an existing window too small.
            window.resize(size(
                vp.width.max(minimum.width),
                vp.height.max(minimum.height),
            ));
        }
        let border = px(self.internal_border_width as f32);
        let border_total = border * 2.0;
        let content_width = px((f32::from(vp.width) - f32::from(border_total)).max(0.0));
        let content_height = px((f32::from(vp.height) - f32::from(border_total)).max(0.0));
        let cols = (f32::from(content_width) / f32::from(cell_width)) as usize;
        let rows = rows_fitting_viewport(&self.grid, content_height, line_height);
        let cols = cols.max(super::MIN_FRAME_COLUMNS);
        let rows = rows.max(3);
        if (cols, rows) != (self.cols, self.rows) {
            self.cols = cols;
            self.rows = rows;
            if self.tx.frame_id().is_some() {
                let _ = self.tx.send(GuiEvent::Resize(cols, rows));
            }
        }
        let scale_factor = window.scale_factor();
        let to_pixels = |value: Pixels| device_pixels(value, scale_factor);
        let to_screen_coordinate = |value: Pixels| f32::from(value).round() as i128;
        let bounds = window.bounds();
        let current_display_id = window.display(cx).map(|display| u32::from(display.id()));
        let monitors = monitor_geometry(window, cx);
        let frame_left = to_screen_coordinate(bounds.origin.x);
        let frame_top = to_screen_coordinate(bounds.origin.y);
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
                .map(|monitor| monitor.geometry[1])
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
            outer_width: to_pixels(bounds.size.width),
            outer_height: to_pixels(bounds.size.height),
            char_width: to_pixels(cell_width).max(1),
            char_height: to_pixels(line_height).max(1),
            scale_factor: scale_factor as f64,
            frame_left,
            frame_top,
            fullscreen: window.is_fullscreen(),
            maximized: window.is_maximized(),
            current_display_id,
            monitors,
        };
        #[cfg(target_os = "macos")]
        let accessibility_geometry = accessibility_frame_key(&geometry);
        if self.geometry.as_ref() != Some(&geometry) {
            self.geometry = Some(geometry.clone());
            if self.tx.frame_id().is_some() {
                let _ = self.tx.send(GuiEvent::Geometry(geometry));
            }
        }

        #[cfg(target_os = "macos")]
        {
            let accessibility_window =
                super::appkit_native_window(window).map(|window| window as usize);
            let (screen_text, screen_selection) = self
                .input
                .accessibility_screen_projection(&self.grid, visible_grid_selection(&self.grid));
            let (buffer_text, buffer_selection) = self.grid.input.as_ref().map_or_else(
                || {
                    self.input
                        .accessibility_buffer_projection(&screen_text, screen_selection.clone())
                },
                |input| {
                    let selection = input
                        .selection
                        .as_ref()
                        .map(|selection| selection.range.clone())
                        .unwrap_or(input.cursor..input.cursor);
                    self.input
                        .accessibility_buffer_projection(&input.text, selection)
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
                    accessibility_window,
                    &buffer_text,
                    buffer_selection.clone(),
                    "Buffer contents",
                    "Current buffer text and input selection.",
                );
                self.accessibility_screen_view = super::accessibility::update_text_view(
                    self.accessibility_screen_view,
                    accessibility_window,
                    &screen_text,
                    screen_selection.clone(),
                    "Visible frame contents",
                    "Text currently visible on the frame, including mode line and minibuffer.",
                );
                self.accessibility_snapshot =
                    Some((buffer_text, buffer_selection, screen_text, screen_selection));
            }
            if self.accessibility_geometry_snapshot != Some(accessibility_geometry) {
                super::accessibility::update_text_view_frame(
                    self.accessibility_buffer_view,
                    accessibility_window,
                );
                super::accessibility::update_text_view_frame(
                    self.accessibility_screen_view,
                    accessibility_window,
                );
                self.accessibility_geometry_snapshot = Some(accessibility_geometry);
            }
        }

        #[cfg(any(target_os = "linux", target_os = "windows", test))]
        {
            let (screen_text, screen_selection) = self
                .input
                .accessibility_screen_projection(&self.grid, 0..0);
            let (buffer_text, buffer_selection) = self.grid.input.as_ref().map_or_else(
                || {
                    self.input
                        .accessibility_buffer_projection(&screen_text, screen_selection.clone())
                },
                |input| {
                    let selection = input
                        .selection
                        .as_ref()
                        .map(|selection| selection.range.clone())
                        .unwrap_or(input.cursor..input.cursor);
                    self.input
                        .accessibility_buffer_projection(&input.text, selection)
                },
            );
            let title = self.grid.title.clone();
            let changed = self.accessibility_tree_snapshot.as_ref().is_none_or(
                |(old_title, old_buffer, old_screen, old_buffer_selection)| {
                    old_title != &title
                        || old_buffer != &buffer_text
                        || old_screen != &screen_text
                        || old_buffer_selection != &buffer_selection
                },
            );
            if changed {
                window.set_accessibility_tree(super::accessibility_tree::update(
                    &title,
                    &buffer_text,
                    buffer_selection.clone(),
                    &screen_text,
                ));
                self.accessibility_tree_snapshot =
                    Some((title, buffer_text, screen_text, buffer_selection));
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
        let internal_border_background = self
            .grid
            .internal_border_style
            .bg
            .map_or(default_background, |color| {
                ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32
            });
        let internal_border_fill =
            rgba((internal_border_background << 8) | u32::from(background_alpha));
        let modifier_mapping = self.modifier_mapping;
        let physical_modifier_state = self.physical_modifier_state.clone();
        let key_modifier_state = physical_modifier_state.clone();
        let mouse_move_modifier_state = physical_modifier_state.clone();
        let bell_flash = self.bell_flash;
        let mouse_point =
            frame_content_point(window.mouse_position(), vp, border);
        let image_pointer = mouse_point.and_then(|point| {
            input::image_pointer_at_point(&self.grid, point, cell_width)
        });
        let over_void_text_area = mouse_point.is_some_and(|point| {
            input::void_text_area_at_point(&self.grid, point, cell_width, window)
        });
        let mouse_cursor = if self.grid.pointer_visible {
            let shape = image_pointer.or(if over_void_text_area {
                Some(self.grid.void_text_area_pointer)
            } else {
                self.grid.hover_cursor
            });
            gpui_cursor_style(shape)
        } else {
            CursorStyle::None
        };
        let mut grid = self.grid.clone();
        let block_cursor_glyphs = prepare_block_cursor_glyphs(&mut grid, default_background);
        let images = grid.images.clone();
        let image_cursors = grid.cursors.clone();
        let image_blink_cursor = grid.blink_cursor;
        let image_blink_visible = grid_cursor_blink_visible(&grid);
        let image_gamma_exponent = grid.gamma_exponent;
        let image_cache_entries = self.image_cache_entries.clone();
        let image_cache_epochs = self.image_cache_epochs.clone();
        let image_cache_size = self.image_cache_size.clone();
        let pointer_regions = grid.pointer_regions.clone();
        let pointer_row_heights = grid.row_heights.clone();
        let pointer_default_line_height = grid.default_line_height;
        let pointer_visible = grid.pointer_visible;
        let explicit_tooltip = (!self.follow_tooltip_drag)
            .then(|| grid.tooltip.clone())
            .flatten();
        let tooltip_position = grid.tooltip_position.unwrap_or_default();
        let tooltip_pointer = tooltip_position.pointer.map_or_else(
            || window.mouse_position(),
            |(x, y)| {
                point(
                    px((x.saturating_sub(frame_left)) as f32),
                    px((y
                        .saturating_sub(frame_top)
                        .saturating_sub(content_top_offset)) as f32),
                )
            },
        );
        let tooltip_placement = tooltip_placement(
            tooltip_position,
            frame_left,
            frame_top,
            content_top_offset,
            vp,
            tooltip_pointer,
        );
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
        let popup_hit_for_input = self.popup_hit_regions.clone();
        let popup_hit_for_shape = self.popup_hit_regions.clone();
        let popup_hit_for_scroll = self.popup_hit_regions.clone();
        let tab_hit_for_shape = self.tab_hit_regions.clone();
        let tab_hit_for_scroll = self.tab_hit_regions.clone();
        let tool_hit_for_shape = self.tool_hit_regions.clone();
        let left_mouse_down_modifiers = physical_modifier_state.clone();
        let left_mouse_up_tx = self.tx.clone();
        let left_mouse_up_grid = mouse_grid.clone();
        let left_mouse_up_modifiers = physical_modifier_state.clone();
        let extra_mouse_tx = self.tx.clone();
        let extra_mouse_grid = mouse_grid;
        let extra_mouse_modifiers = physical_modifier_state.clone();
        let scroll_modifiers = physical_modifier_state.clone();
        let entity = cx.entity();
        let focus = self.focus.clone();
        #[cfg(target_os = "windows")]
        let native_close_on_alt_f4 = self.native_close_on_alt_f4.clone();
        let native_close_on_command_w = self.native_close_on_command_w.clone();
        let native_close_on_command_w_on_key_up = self.native_close_on_command_w.clone();
        let marked = self.input.marked.clone();
        let marked_selection = self.input.selection_byte_range();
        let marked_cursor = self.input.selection_byte_end();
        let marked_replacement = self.input.replacement_range.clone();
        let marked_origin =
            input::composition_origin(&self.grid, self.input.replacement_range.as_ref());
        let composition_font = text_font.clone();
        let tooltip_text_font = text_font.clone();
        let tooltip_gamma_exponent = grid.gamma_exponent;
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
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, window, cx| {
                let urls = paths
                    .paths()
                    .iter()
                    .map(|path| file_drop_uri(path))
                    .collect::<Vec<_>>();
                let position =
                    frame_content_point(window.mouse_position(), window.viewport_size(), border)
                        .and_then(|point| {
                            input::mouse_position_for_point_with_cell_width(
                                &this.grid, point, cell_width, window,
                            )
                        });
                let _ = file_drop_tx.send(GuiEvent::Drop {
                    position,
                    urls,
                    action: "copy".to_owned(),
                });
                window.prevent_default();
                cx.stop_propagation();
            }))
            .on_drop(cx.listener(move |this, text: &String, window, cx| {
                let position =
                    frame_content_point(window.mouse_position(), window.viewport_size(), border)
                        .and_then(|point| {
                            input::mouse_position_for_point_with_cell_width(
                                &this.grid, point, cell_width, window,
                            )
                        });
                let _ = text_drop_tx.send(GuiEvent::Drop {
                    position,
                    urls: vec![text.clone()],
                    action: "copy".to_owned(),
                });
                window.prevent_default();
                cx.stop_propagation();
            }))
            .on_drop(cx.listener(move |this, color: &ExternalColor, window, cx| {
                let position =
                    frame_content_point(window.mouse_position(), window.viewport_size(), border)
                        .and_then(|point| {
                            input::mouse_position_for_point_with_cell_width(
                                &this.grid, point, cell_width, window,
                            )
                        });
                let position = position.or_else(|| {
                    frame_content_point(
                        window.mouse_position(),
                        window.viewport_size(),
                        border,
                    )
                    .and_then(|point| {
                        input::mode_line_mouse_position(
                            &this.grid,
                            point,
                            cell_width,
                            window.scale_factor(),
                        )
                    })
                });
                if let Some(event) = external_color_drop_event(position, color) {
                    let _ = this.tx.send(event);
                }
                window.prevent_default();
                cx.stop_propagation();
            }))
            .on_drop(cx.listener(move |_this, text: &ExternalText, window, cx| {
                let position =
                    frame_content_point(window.mouse_position(), window.viewport_size(), border)
                        .and_then(|point| {
                            input::mouse_position_for_point_with_cell_width(
                                &_this.grid, point, cell_width, window,
                            )
                        });
                let _ = external_text_drop_tx.send(external_text_drop_event(position, text));
                window.prevent_default();
                cx.stop_propagation();
            }))
            .on_action(cx.listener(
                |this: &mut EditorView, action: &RunMenuCommand, _window, _cx| {
                    if let Some(event) = action.event() {
                        let _ = this.tx.send(event);
                    }
                },
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.mouse_down_positions.remove(&MouseButton::Left);
                    this.pressed_button = None;
                    this.tab_bar_drag = None;
                    this.consumed_mouse_buttons.remove(&MouseButton::Left);
                    let Some(point) =
                        frame_content_point(event.position, window.viewport_size(), border)
                    else {
                        return;
                    };
                    let absolute_position = absolute_mouse_pixel_position(
                        event.position,
                        pointer_screen_left,
                        pointer_screen_top,
                        content_top_offset,
                    );
                    let _ = this.tx.send_mouse_move(None, Some(absolute_position));
                    if !left_mouse_down_grid.popup_menu_items.is_empty() {
                        this.consumed_mouse_buttons.insert(MouseButton::Left);
                        this.pressed_button = this.button_at(point);
                        if let Some(event @ GuiEvent::PopupMenuSelection { index: None, .. }) =
                            popup_menu_event_at(&popup_hit_for_input.borrow(), point)
                        {
                            let _ = left_mouse_down_tx.send(event);
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if let Some(button) = this.button_at(point) {
                        this.tab_bar_drag = match &button.action {
                            ButtonAction::Tab {
                                command,
                                command_id: None,
                                argument: Some(from_tab),
                            } if command == "tab-bar-select-tab" => Some(TabBarDrag {
                                from_tab: *from_tab,
                                dragged: false,
                            }),
                            _ => None,
                        };
                        this.consumed_mouse_buttons.insert(MouseButton::Left);
                        this.pressed_button = Some(button);
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
                        send_scroll_bar_event(&left_mouse_down_tx, &bar, point, pointer_offset);
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
                            window.scale_factor(),
                        )
                    }) {
                        this.mouse_down_positions
                            .insert(MouseButton::Left, position);
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
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    let absolute_position = absolute_mouse_pixel_position(
                        event.position,
                        pointer_screen_left,
                        pointer_screen_top,
                        content_top_offset,
                    );
                    let point = frame_content_point(event.position, window.viewport_size(), border);
                    let position = point.and_then(|point| {
                        input::mouse_position_for_text_point_with_cell_width(
                            &this.grid, point, cell_width, window,
                        )
                    });
                    let text_window_id = point.and_then(|point| {
                        input::text_window_at_point(&this.grid, point, cell_width)
                    });
                    let modifiers = mouse_modifier_bits(
                        &event.modifiers,
                        modifier_mapping,
                        mouse_move_modifier_state.get(),
                    );
                    let target =
                        position.map(|position| (position.buffer_id, position.buffer_position));
                    let _ = this.mouse_face_position.replace(target);
                    let _ = this.tx.send_mouse_move_for_window(
                        position,
                        Some(absolute_position),
                        text_window_id,
                        modifiers,
                    );
                    {
                        let popup = this.popup_hit_regions.borrow();
                        if !popup.is_empty() {
                            if let Some(event) =
                                point.and_then(|point| popup_menu_hover_event_at(&popup, point))
                            {
                                let _ = this.tx.send(event);
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                    }
                    if !event.dragging() {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left)
                        && this.consumed_mouse_buttons.contains(&MouseButton::Left)
                        && let Some(drag) = this.tab_bar_drag
                    {
                        this.tab_bar_drag = Some(TabBarDrag {
                            dragged: true,
                            ..drag
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if event
                        .pressed_button
                        .is_some_and(|button| this.consumed_mouse_buttons.contains(&button))
                    {
                        return;
                    }
                    if let Some(drag) = this.scroll_bar_drag {
                        let Some(point) = point else {
                            return;
                        };
                        if let Some(bar) = this.grid.scroll_bars.iter().find(|bar| {
                            bar.window_id == drag.window_id && bar.orientation == drag.orientation
                        }) {
                            this.scroll_bar_drag = Some(ScrollBarDrag {
                                dragged: true,
                                ..drag
                            });
                            send_scroll_bar_event(
                                &scroll_bar_tx,
                                bar,
                                point,
                                drag.pointer_offset,
                                None,
                            );
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        return;
                    }
                    let _ = this.tx.send_mouse_drag_move(position, modifiers);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                    let down_position = this.mouse_down_positions.remove(&MouseButton::Left);
                    if this.consumed_mouse_buttons.remove(&MouseButton::Left) {
                        let pressed = this.pressed_button.take();
                        let current =
                            frame_content_point(event.position, window.viewport_size(), border)
                                .and_then(|point| this.button_at(point));
                        if let Some(drag) = this.tab_bar_drag.take().filter(|drag| drag.dragged) {
                            let _ = left_mouse_up_tx.send(GuiEvent::CommandArgs {
                                command: "tab-bar-select-tab".to_owned(),
                                args: vec![drag.from_tab],
                            });
                            let target_tab = current.as_ref().and_then(|button| {
                                match &button.action {
                                    ButtonAction::Tab {
                                        command,
                                        command_id: None,
                                        argument: Some(tab),
                                    } if command == "tab-bar-select-tab" => Some(*tab),
                                    _ => None,
                                }
                            });
                            if let Some(to_tab) = target_tab
                                && to_tab != drag.from_tab
                            {
                                let _ = left_mouse_up_tx.send(GuiEvent::CommandArgs {
                                    command: "tab-bar-move-tab-to".to_owned(),
                                    args: vec![to_tab, drag.from_tab],
                                });
                            }
                        } else if let Some(pressed) = pressed {
                            if current.as_ref() == Some(&pressed) {
                                let _ = left_mouse_up_tx.send(pressed.into_event());
                            } else if let Some(event) = pressed.cancel_event() {
                                let _ = left_mouse_up_tx.send(event);
                            }
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if let Some(drag) = this.scroll_bar_drag.take() {
                        if let Some(point) =
                            frame_content_point(event.position, window.viewport_size(), border)
                            && let Some(bar) = this.grid.scroll_bars.iter().find(|bar| {
                                bar.window_id == drag.window_id
                                    && bar.orientation == drag.orientation
                            })
                        {
                            let end_position = if drag.dragged {
                                scroll_bar_mouse_position(
                                    bar,
                                    point,
                                    drag.pointer_offset,
                                    window.scale_factor(),
                                )
                            } else {
                                drag.start_position
                            };
                            let mouse = MouseInput {
                                button: RemacsMouseButton::Left,
                                click_count: drag.click_count,
                                modifiers: drag.modifiers,
                                start: drag.start_position,
                                end: end_position,
                            };
                            send_scroll_bar_event(
                                &left_mouse_up_tx,
                                bar,
                                point,
                                drag.pointer_offset,
                                Some(mouse),
                            );
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    let point = frame_content_point(event.position, window.viewport_size(), border);
                    let hit = point
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
                                    window.scale_factor(),
                                )
                            })
                        });
                    let scale_factor = window.scale_factor();
                    let position = input::mouse_up_position(
                        hit,
                        down_position,
                        device_pixels(event.position.x - border, scale_factor),
                        device_pixels(event.position.y - border, scale_factor),
                    );
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
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, window, cx| {
                    this.mouse_down_positions.remove(&MouseButton::Left);
                    if this.consumed_mouse_buttons.remove(&MouseButton::Left) {
                        this.tab_bar_drag = None;
                        if let Some(event) = this
                            .pressed_button
                            .take()
                            .and_then(PressedButton::cancel_event)
                        {
                            let _ = this.tx.send(event);
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }),
            )
            .map(|mut element| {
                for (button, remacs_button) in [
                    (MouseButton::Middle, RemacsMouseButton::Middle),
                    (MouseButton::Right, RemacsMouseButton::Right),
                    (
                        MouseButton::Navigate(NavigationDirection::Back),
                        RemacsMouseButton::Back,
                    ),
                    (
                        MouseButton::Navigate(NavigationDirection::Forward),
                        RemacsMouseButton::Forward,
                    ),
                ] {
                    let down_tx = extra_mouse_tx.clone();
                    let down_grid = extra_mouse_grid.clone();
                    let down_modifiers = extra_mouse_modifiers.clone();
                    element = element.on_mouse_down(
                        button,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.mouse_down_positions.remove(&button);
                            if button == MouseButton::Middle {
                                this.middle_tab_close_index = None;
                            }
                            if let Some(menu) = down_grid.popup_menu_items.first() {
                                this.consumed_mouse_buttons.insert(button);
                                let _ = down_tx.send(GuiEvent::PopupMenuSelection {
                                    menu_id: menu.menu_id,
                                    index: None,
                                });
                                window.prevent_default();
                                cx.stop_propagation();
                                return;
                            }
                            let point =
                                frame_content_point(event.position, window.viewport_size(), border);
                            if button == MouseButton::Middle {
                                let tab_index = point
                                    .and_then(|point| this.button_at(point))
                                    .and_then(|pressed| match pressed.action {
                                        ButtonAction::Tab {
                                            argument: Some(tab_index),
                                            ..
                                        } => Some(tab_index),
                                        _ => None,
                                    });
                                if let Some(tab_index) = tab_index {
                                    this.middle_tab_close_index = Some(tab_index);
                                    this.consumed_mouse_buttons.insert(button);
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    return;
                                }
                            }
                            if let Some(position) = point.and_then(|point| {
                                input::mouse_position_for_point_with_cell_width(
                                    &down_grid, point, cell_width, window,
                                )
                                .or_else(|| {
                                    input::mode_line_mouse_position(
                                        &down_grid,
                                        point,
                                        cell_width,
                                        window.scale_factor(),
                                    )
                                })
                            }) {
                                this.mouse_down_positions.insert(button, position);
                                let _ = down_tx.send(GuiEvent::MouseDown {
                                    button: remacs_button,
                                    click_count: event.click_count,
                                    modifiers: mouse_modifier_bits(
                                        &event.modifiers,
                                        modifier_mapping,
                                        down_modifiers.get(),
                                    ),
                                    position,
                                });
                            }
                        }),
                    );
                    let up_tx = extra_mouse_tx.clone();
                    let up_grid = extra_mouse_grid.clone();
                    let up_modifiers = extra_mouse_modifiers.clone();
                    element = element.on_mouse_up(
                        button,
                        cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                            let down_position = this.mouse_down_positions.remove(&button);
                            if this.consumed_mouse_buttons.remove(&button) {
                                if button == MouseButton::Middle
                                    && let Some(tab_index) = this.middle_tab_close_index.take()
                                    && let Some(point) = frame_content_point(
                                        event.position,
                                        window.viewport_size(),
                                        border,
                                    )
                                    && this.button_at(point).is_some_and(|pressed| {
                                        matches!(pressed.action,
                                            ButtonAction::Tab {
                                                argument: Some(current),
                                                ..
                                            } if current == tab_index)
                                    })
                                {
                                    let _ = up_tx.send(GuiEvent::CommandArgs {
                                        command: "tab-bar-close-tab".to_owned(),
                                        args: vec![tab_index],
                                    });
                                }
                                window.prevent_default();
                                cx.stop_propagation();
                                return;
                            }
                            let point =
                                frame_content_point(event.position, window.viewport_size(), border);
                            let hit = point.and_then(|point| {
                                input::mouse_position_for_point_with_cell_width(
                                    &up_grid, point, cell_width, window,
                                )
                                .or_else(|| {
                                    input::mode_line_mouse_position(
                                        &up_grid,
                                        point,
                                        cell_width,
                                        window.scale_factor(),
                                    )
                                })
                            });
                            let scale_factor = window.scale_factor();
                            if let Some(position) = input::mouse_up_position(
                                hit,
                                down_position,
                                device_pixels(event.position.x - border, scale_factor),
                                device_pixels(event.position.y - border, scale_factor),
                            ) {
                                let _ = up_tx.send(GuiEvent::MouseUp {
                                    button: remacs_button,
                                    click_count: event.click_count,
                                    modifiers: mouse_modifier_bits(
                                        &event.modifiers,
                                        modifier_mapping,
                                        up_modifiers.get(),
                                    ),
                                    position,
                                });
                            }
                        }),
                    );
                    if button == MouseButton::Middle {
                        element = element.on_mouse_up_out(
                            button,
                            cx.listener(move |this, _event: &MouseUpEvent, window, cx| {
                                this.mouse_down_positions.remove(&button);
                                if this.middle_tab_close_index.take().is_some() {
                                    this.consumed_mouse_buttons.remove(&button);
                                    window.prevent_default();
                                    cx.stop_propagation();
                                }
                            }),
                        );
                    }
                }
                element
            })
            .on_scroll_wheel(move |event, window, cx| {
                if let Some(menu) = scroll_grid.popup_menu_items.first() {
                    let delta = event.delta.pixel_delta(line_height);
                    let delta_lines = -f32::from(delta.y) / f32::from(line_height);
                    if delta_lines != 0.0 {
                        let point =
                            frame_content_point(event.position, window.viewport_size(), border);
                        let regions = popup_hit_for_scroll.borrow();
                        let parent = point
                            .and_then(|point| {
                                regions
                                    .iter()
                                    .rev()
                                    .find(|item| {
                                        point.x >= item.bounds.left()
                                            && point.x < item.bounds.right()
                                            && point.y >= item.bounds.top()
                                            && point.y < item.bounds.bottom()
                                    })
                                    .map(|item| item.parent)
                            })
                            .unwrap_or_else(|| {
                                scroll_grid
                                    .popup_menu_items
                                    .last()
                                    .and_then(|item| item.parent)
                            });
                        let event = if parent.is_none() {
                            GuiEvent::PopupMenuScroll {
                                menu_id: menu.menu_id,
                                delta_lines,
                            }
                        } else {
                            GuiEvent::PopupMenuScrollAt {
                                menu_id: menu.menu_id,
                                parent,
                                delta_lines,
                            }
                        };
                        let _ = scroll_tx.send(event);
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                let point = frame_content_point(event.position, window.viewport_size(), border);
                let vertical_delta = event.delta.pixel_delta(line_height).y;
                let tab_position = point.and_then(|point| {
                    let row = tab_hit_for_scroll
                        .borrow()
                        .iter()
                        .find(|item| {
                            point.x >= item.bounds.left()
                                && point.x < item.bounds.right()
                                && point.y >= item.bounds.top()
                                && point.y < item.bounds.bottom()
                        })
                        .map(|item| item.row)?;
                    input::tab_bar_mouse_position(
                        &scroll_grid,
                        point,
                        cell_width,
                        window.scale_factor(),
                        row,
                    )
                });
                if let Some(position) = tab_position
                    && f32::from(vertical_delta) != 0.0
                {
                    let delta_lines = -f32::from(vertical_delta) / f32::from(line_height);
                    let pixel_delta = event.delta.precise().then(|| {
                        (
                            f32::from(event.delta.pixel_delta(cell_width).x).round() as i128,
                            f32::from(vertical_delta).round() as i128,
                        )
                    });
                    let modifiers = mouse_modifier_bits(
                        &event.modifiers,
                        modifier_mapping,
                        scroll_modifiers.get(),
                    );
                    let _ = scroll_tx.send(GuiEvent::Scroll {
                        delta: delta_lines,
                        horizontal: false,
                        pixel_delta,
                        modifiers,
                        position,
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }
                if let Some(position) = point.and_then(|point| {
                    input::mouse_position_for_point_with_cell_width(
                        &scroll_grid,
                        point,
                        cell_width,
                        window,
                    )
                    .or_else(|| {
                        input::mode_line_mouse_position(
                            &scroll_grid,
                            point,
                            cell_width,
                            window.scale_factor(),
                        )
                    })
                    .or_else(|| {
                        input::header_line_mouse_position(
                            &scroll_grid,
                            point,
                            cell_width,
                            window.scale_factor(),
                        )
                    })
                }) {
                    let vertical_delta = event.delta.pixel_delta(line_height);
                    let horizontal_delta = event.delta.pixel_delta(cell_width);
                    let delta_lines = wheel_delta_lines(
                        event.delta,
                        scroll_row_height(
                            &scroll_grid,
                            position.window_id,
                            position.row,
                            line_height,
                        ),
                    );
                    let delta_columns = -f32::from(horizontal_delta.x) / f32::from(cell_width);
                    let pixel_delta = event.delta.precise().then(|| {
                        (
                            f32::from(vertical_delta.x).round() as i128,
                            f32::from(vertical_delta.y).round() as i128,
                        )
                    });
                    if delta_lines != 0.0 {
                        let _ = scroll_tx.send(GuiEvent::Scroll {
                            delta: delta_lines,
                            horizontal: false,
                            pixel_delta,
                            modifiers: mouse_modifier_bits(
                                &event.modifiers,
                                modifier_mapping,
                                scroll_modifiers.get(),
                            ),
                            position,
                        });
                    }
                    if delta_columns != 0.0 {
                        let _ = scroll_tx.send(GuiEvent::Scroll {
                            delta: delta_columns,
                            horizontal: true,
                            pixel_delta: None,
                            modifiers: mouse_modifier_bits(
                                &event.modifiers,
                                modifier_mapping,
                                scroll_modifiers.get(),
                            ),
                            position,
                        });
                    }
                    if matches!(event.touch_phase, TouchPhase::Ended) {
                        let _ = scroll_tx.send(GuiEvent::TouchEnd {
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
                if is_native_command_w(&event.keystroke) {
                    native_close_on_command_w.set(true);
                    return;
                }
                #[cfg(target_os = "windows")]
                {
                    native_close_on_alt_f4.set(false);
                    if event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.shift
                        && !event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.function
                        && !event.keystroke.modifiers.hyper
                    {
                        match event.keystroke.key.as_str() {
                            "space" => return,
                            "f4" => {
                                native_close_on_alt_f4.set(true);
                                return;
                            }
                            _ => {}
                        }
                    }
                }
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
                ) {
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
            .on_key_up(move |event: &gpui::KeyUpEvent, _window, _cx| {
                if event.keystroke.key == "w" {
                    native_close_on_command_w_on_key_up.set(false);
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
                                        let marker =
                                            match bidi_paragraph_ranges[open_paragraph].direction {
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
                                let mut font_sizes = Vec::new();
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
                                            f.family =
                                                font_family(window, Some(family.clone())).into();
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
                                        font_sizes.push(st.font_height.map(face_font_size));
                                    };
                                for (len, style) in styled_spans {
                                    push_run(style, len);
                                }
                                if runs.is_empty() {
                                    runs.push(TextRun {
                                        len: 0,
                                        font: text_font.clone(),
                                        color: rgb(gamma_correct_rgb(FG, grid.gamma_exponent))
                                            .into(),
                                        background_color: None,
                                        underline: None,
                                        strikethrough: None,
                                    });
                                    font_sizes.push(None);
                                }
                                let line = window.text_system().shape_line_with_font_sizes(
                                    shaping_text.into(),
                                    font_size,
                                    &runs,
                                    &font_sizes,
                                    None,
                                );
                                align_zwj_glyph_positions(
                                    line,
                                    row,
                                    cell_width,
                                    |byte_index| grid_layout_index(&grid, y, byte_index),
                                )
                            })
                            .collect();
                        *popup_hit_for_shape.borrow_mut() =
                            popup_menu_hit_regions(&grid, &shaped, line_height);
                        *tab_hit_for_shape.borrow_mut() =
                            tab_bar_hit_regions(&grid, &shaped, line_height);
                        *tool_hit_for_shape.borrow_mut() =
                            tool_bar_hit_regions(&grid, &shaped, line_height);
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
                            paint_grid_row_with_window_heights(
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
                                    + grid_row_top_for_cell(
                                        &grid,
                                        indicator.row,
                                        indicator.column,
                                        line_height,
                                    )
                                    - px(vscroll_pixels_for_cell(
                                        &grid,
                                        indicator.row,
                                        indicator.column,
                                    ) as f32),
                            );
                            let row_height = grid_row_height_for_cell(
                                &grid,
                                indicator.row,
                                indicator.column,
                                line_height,
                            );
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
                                    lines
                                        .get(row)
                                        .map(|line| grid_line_x(&grid, row, line, index))
                                })
                                .unwrap_or(cell_width * (column as f32));
                            (row, x)
                        });
                        let composition_origin = (!marked.is_empty())
                            .then_some(marked_origin)
                            .flatten()
                            .and_then(|(row, index)| {
                                lines
                                    .get(row)
                                    .map(|line| (row, grid_line_x(&grid, row, line, index)))
                            })
                            .or(fallback_cursor);
                        let (composition_foreground, composition_background) = style_colors(
                            grid.default_style,
                            default_background,
                            background_alpha,
                            grid.gamma_exponent,
                        );
                        let composition_background =
                            composition_background.unwrap_or(default_background_fill.into());
                        let composition_caret = if !marked.is_empty() {
                            composition_origin.map(|(row, x)| {
                                let column = (f32::from(x) / f32::from(cell_width)) as usize;
                                let row_height = grid_row_height_for_cell(
                                    &grid,
                                    row,
                                    column,
                                    line_height,
                                );
                                let origin = point(
                                    bounds.left() + x,
                                    bounds.top()
                                        + grid_row_top_for_cell(&grid, row, column, line_height)
                                        - px(vscroll_pixels_for_cell(&grid, row, column) as f32),
                                );
                                let selection_start = marked_selection.start.min(marked.len());
                                let selection_end = marked_selection.end.min(marked.len());
                                let composition_run =
                                    |start: usize, end: usize, selected: bool| TextRun {
                                        len: end.saturating_sub(start),
                                        font: composition_font.clone(),
                                        color: composition_foreground,
                                        background_color: selected.then(|| {
                                            rgb(gamma_correct_rgb(0x356fce, grid.gamma_exponent))
                                                .into()
                                        }),
                                        underline: Some(UnderlineStyle {
                                            thickness: px(1.0),
                                            color: None,
                                            wavy: false,
                                        }),
                                        strikethrough: None,
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
                                let replacement_end_x = marked_replacement
                                    .as_ref()
                                    .and_then(|range| {
                                        input::visible_input_position(&grid, range.end, true)
                                    })
                                    .filter(|(end_row, _)| *end_row == row)
                                    .and_then(|(_, end_index)| {
                                        lines.get(row).map(|base_line| {
                                            grid_line_x(&grid, row, base_line, end_index)
                                        })
                                    });
                                let can_shift_suffix =
                                    marked_replacement.is_none() || replacement_end_x.is_some();
                                let replaced_delta =
                                    replacement_end_x.map_or(px(0.0), |end_x| end_x - x);
                                let replaced_width = px(f32::from(replaced_delta).abs());
                                with_vscroll_cell_clip(
                                    &grid,
                                    row,
                                    column,
                                    bounds,
                                    cell_width,
                                    row_height,
                                    window,
                                    |window| {
                                        window.paint_quad(fill(
                                            Bounds {
                                                origin,
                                                size: size(
                                                    line.width.max(replaced_width).max(cell_width),
                                                    line_height,
                                                ),
                                            },
                                            composition_background,
                                        ));
                                        let _ = line.paint(origin, line_height, window, _cx);
                                        if can_shift_suffix {
                                            if let Some(base_line) = lines.get(row) {
                                                let suffix_left = origin.x + line.width;
                                                let suffix_width = (bounds.left()
                                                    + bounds.size.width
                                                    - suffix_left)
                                                    .max(px(0.0));
                                                if suffix_width > px(0.0) {
                                                    let clip = Bounds {
                                                        origin: point(suffix_left, origin.y),
                                                        size: size(suffix_width, line_height),
                                                    };
                                                    let shifted_origin = point(
                                                        bounds.left() + line.width - replaced_delta,
                                                        origin.y,
                                                    );
                                                    window.with_content_mask(
                                                        Some(ContentMask { bounds: clip }),
                                                        |window| {
                                                            let _ = base_line.paint(
                                                                shifted_origin,
                                                                line_height,
                                                                window,
                                                                _cx,
                                                            );
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                    },
                                );
                                (row, x + line.x_for_index(marked_cursor))
                            })
                        } else {
                            None
                        };

                        let blink_visible = grid_cursor_blink_visible(&grid);
                        for cursor in &grid.cursors {
                            // Image children are painted after this canvas.
                            // Their cursor overlay is drawn above the image instead.
                            if grid.images.iter().any(|image| {
                                image.row == cursor.row && image.column == cursor.column
                            }) {
                                continue;
                            }
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
                                    .unwrap_or_else(|| grid_line_x(&grid, row, line, cursor.index))
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
                            let row_height =
                                grid_row_height_for_cell(&grid, row, column, line_height);
                            let origin = point(
                                bounds.left() + x,
                                bounds.top()
                                    + grid_row_top_for_cell(&grid, row, column, line_height)
                                    - px(vscroll_pixels_for_cell(&grid, row, column) as f32),
                            );
                            with_vscroll_cell_clip(
                                &grid,
                                row,
                                column,
                                bounds,
                                cell_width,
                                row_height,
                                window,
                                |window| match cursor.shape {
                                    remacs_runtime::frontend::GridCursorShape::Block => {
                                        let width = if cursor.thickness > 0 {
                                            px(cursor.thickness as f32)
                                        } else {
                                            glyph_width
                                        };
                                        if !block_cursor_glyphs
                                            .contains(&(cursor.row, cursor.index))
                                        {
                                            window.paint_quad(fill(
                                                Bounds {
                                                    origin,
                                                    size: size(width, line_height),
                                                },
                                                rgba((rgb_color << 8) | 0x80),
                                            ));
                                        }
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
                        fringe::paint(
                            &grid,
                            bounds,
                            cell_width,
                            line_height,
                            default_background,
                            background_alpha,
                            window,
                        );
                        for bar in &grid.scroll_bars {
                            let (thumb_color, track_color) = scroll_bar_colors(
                                bar,
                                default_background,
                                background_alpha,
                                grid.gamma_exponent,
                            );
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
                                track_color,
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
                                thumb_color,
                            ));
                        }
                        paint_right_dividers(
                            &grid,
                            bounds,
                            cell_width,
                            line_height,
                            default_background,
                            background_alpha,
                            window,
                        );
                        if bell_flash {
                            window.paint_quad(fill(bounds, rgba((0xFFFFFF << 8) | 0x80)));
                        }
                    },
                )
                .size_full(),
            )
            .children(pointer_regions.into_iter().map(|region| {
                let top = row_top_from_heights(
                    &pointer_row_heights,
                    pointer_default_line_height,
                    region.row,
                    line_height,
                );
                let height = row_height_from_heights(
                    &pointer_row_heights,
                    pointer_default_line_height,
                    region.row,
                    line_height,
                );
                div()
                    .absolute()
                    .left(border + cell_width * region.left as f32)
                    .top(border + top)
                    .w(cell_width * region.width as f32)
                    .h(height)
                    .cursor(if pointer_visible {
                        gpui_cursor_style(Some(region.shape))
                    } else {
                        CursorStyle::None
                    })
            }))
            .children(images.into_iter().map(|image| {
                let cursors = image_cursors
                    .iter()
                    .copied()
                    .filter(|cursor| {
                        cursor.row == image.row
                            && cursor.column == image.column
                            && (!cursor.selected || !image_blink_cursor || image_blink_visible)
                    })
                    .collect();
                grid_image_element(
                    image,
                    cell_width,
                    line_height,
                    &grid,
                    border,
                    cursors,
                    image_gamma_exponent,
                    image_cache_entries.clone(),
                    image_cache_epochs.clone(),
                    image_cache_size.clone(),
                )
            }))
            .when_some(explicit_tooltip, |this, tooltip_text| {
                let runs = tooltip_text
                    .runs
                    .iter()
                    .map(|run| {
                        let style = run.style;
                        let (color, background_color) = style_colors(
                            style,
                            default_background,
                            background_alpha,
                            tooltip_gamma_exponent,
                        );
                        let mut font = tooltip_text_font.clone();
                        if let Some(family) = style
                            .font_family
                            .and_then(|id| tooltip_text.font_families.get(id as usize))
                        {
                            font.family = font_family(window, Some(family.clone())).into();
                        }
                        if let Some(weight) = style.font_weight {
                            font.weight = FontWeight(weight as f32);
                        } else if style.bold {
                            font = font.bold();
                        }
                        if let Some(width) = style.font_width {
                            font.stretch = FontStretch::from_percentage(width);
                        }
                        if style.italic {
                            font = font.italic();
                        }
                        TextRun {
                            len: run.len,
                            font,
                            color,
                            background_color: if style.stipple.is_some() {
                                None
                            } else {
                                background_color
                            },
                            underline: style.underline.then(|| UnderlineStyle {
                                thickness: px(1.0),
                                color: style
                                    .underline_color
                                    .map(|color| rgb(color_rgb(color)).into()),
                                wavy: style.underline_wave,
                            }),
                            strikethrough: style.strike.then(|| StrikethroughStyle {
                                thickness: px(1.0),
                                color: style
                                    .strike_color
                                    .map(|color| rgb(color_rgb(color)).into()),
                            }),
                        }
                    })
                    .collect::<Vec<_>>();
                let text = if runs.is_empty() {
                    gpui::StyledText::new(tooltip_text.text.clone())
                } else {
                    gpui::StyledText::new(tooltip_text.text.clone()).with_runs(runs)
                };
                let (tooltip_foreground, tooltip_background) = style_colors(
                    tooltip_text.style,
                    default_background,
                    background_alpha,
                    tooltip_gamma_exponent,
                );
                let tooltip_background =
                    tooltip_background.unwrap_or_else(|| rgb(0x303438).into());
                let tooltip = div().absolute();
                let tooltip = if let Some(left) = tooltip_placement.left {
                    tooltip.left(left)
                } else if let Some(right) = tooltip_placement.right {
                    tooltip.right(right)
                } else {
                    tooltip
                };
                let tooltip = if let Some(top) = tooltip_placement.top {
                    tooltip.top(top)
                } else if let Some(bottom) = tooltip_placement.bottom {
                    tooltip.bottom(bottom)
                } else {
                    tooltip
                };
                this.child(
                    tooltip
                        .bg(tooltip_background)
                        .text_color(tooltip_foreground)
                        .text_size(
                            tooltip_text
                                .style
                                .font_height
                                .map(face_font_size)
                                .unwrap_or(font_size),
                        )
                        .font_family(tooltip_text_font.family.clone())
                        .child(text),
                )
            })
    }
}

fn gpui_menus(menus: &[GridMenu]) -> Vec<MenuItem> {
    let roots = menus.iter().map(|menu| {
        if let Some(action) =
            RunMenuCommand::from_binding(menu.command.as_deref(), menu.command_id.as_ref())
        {
            MenuItem::action(menu.label.clone(), action)
        } else {
            MenuItem::submenu(Menu {
                name: menu.label.clone().into(),
                items: menu.items.iter().filter_map(gpui_menu_item).collect(),
            })
        }
    });
    #[cfg(target_os = "macos")]
    {
        // AppKit reserves the first root for the application menu. Keep Lisp's
        // first command/menu outside that slot, like GNU's NS frontend.
        let application = MenuItem::submenu(Menu {
            name: "REmacs".into(),
            items: vec![MenuItem::action(
                "Quit REmacs",
                RunMenuCommand {
                    command: NativeMenuAction::Named("save-buffers-kill-terminal".into()),
                },
            )],
        });
        std::iter::once(application).chain(roots).collect()
    }
    #[cfg(not(target_os = "macos"))]
    roots.collect()
}

fn gpui_menu_item(item: &GridMenuItem) -> Option<MenuItem> {
    if item.separator {
        return Some(MenuItem::separator());
    }
    if !item.items.is_empty() {
        let submenu = MenuItem::submenu(Menu {
            name: item.label.clone().into(),
            items: item.items.iter().filter_map(gpui_menu_item).collect(),
        });
        let submenu = if let Some(hint) = &item.key_hint {
            submenu.key_hint(hint.clone())
        } else {
            submenu
        };
        return Some(if let Some(help) = &item.help {
            submenu.tooltip(help.clone())
        } else {
            submenu
        });
    }
    let mut result = if let Some(action) =
        RunMenuCommand::from_binding(item.command.as_deref(), item.command_id.as_ref())
    {
        MenuItem::action(item.label.clone(), action)
    } else {
        MenuItem::disabled(item.label.clone())
    }
    .checked(item.checked);
    if let Some(hint) = &item.key_hint {
        result = result.key_hint(hint.clone());
    }
    if let Some(help) = &item.help {
        result = result.tooltip(help.clone());
    }
    Some(result)
}

#[cfg(test)]
mod native_menu_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn native_help_keeps_actions_disabled_state_hints_and_submenus() {
        let row = GridMenuItem {
            label: "Run".into(),
            help: Some("Runs the test command".into()),
            command: Some("ignore".into()),
            command_id: None,
            items: Vec::new(),
            separator: false,
            checked: true,
            key_hint: Some("C-c h".into()),
        };
        let item = gpui_menu_item(&row).unwrap().owned().clone();
        let gpui::OwnedMenuItem::WithTooltip { item, tooltip } = item else {
            panic!("help missing")
        };
        assert_eq!(tooltip, "Runs the test command");
        assert!(
            matches!(*item, gpui::OwnedMenuItem::Action { checked: true, key_hint: Some(ref hint), .. } if hint == "C-c h")
        );
        let disabled = GridMenuItem {
            command: None,
            ..row.clone()
        };
        let gpui::OwnedMenuItem::WithTooltip { item, .. } =
            gpui_menu_item(&disabled).unwrap().owned()
        else {
            panic!("help missing")
        };
        assert!(matches!(
            *item,
            gpui::OwnedMenuItem::Disabled { checked: true, .. }
        ));
        let submenu = GridMenuItem {
            label: "More".into(),
            help: Some("Submenu help".into()),
            command: None,
            command_id: None,
            items: vec![row],
            separator: false,
            checked: false,
            key_hint: Some("C-x 4".into()),
        };
        let gpui::OwnedMenuItem::WithTooltip { item, tooltip } =
            gpui_menu_item(&submenu).unwrap().owned().clone()
        else {
            panic!("help missing")
        };
        assert_eq!(tooltip, "Submenu help");
        let gpui::OwnedMenuItem::SubmenuWithKeyHint { menu, key_hint } = *item else {
            panic!("submenu missing")
        };
        assert_eq!(key_hint, "C-x 4");
        assert!(
            matches!(&menu.items[0], gpui::OwnedMenuItem::WithTooltip { tooltip, .. } if tooltip == "Runs the test command")
        );
    }

    #[::core::prelude::v1::test]
    fn lambda_menu_bindings_become_selectable_native_actions() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str(
                r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (setq global-map (make-sparse-keymap) menu-bar-update-hook nil
                pre-redisplay-function nil qa-native-runs 0 qa-native-named-runs 0)
          (setq-local fontification-functions nil)
          (defun qa-native-named () (interactive)
            (setq qa-native-named-runs (1+ qa-native-named-runs)))
          (let ((map (make-sparse-keymap)))
            (define-key map [named] '(menu-item "Run named" qa-native-named))
            (define-key map [run]
              '(menu-item "Run lambda" (lambda () (interactive)
                                         (setq qa-native-runs (1+ qa-native-runs))
                                         (message "ran"))))
            (define-key global-map [menu-bar qa-lambda] (cons "QA Lambda" map)))
          (menu-bar-mode 1))"#,
            )
            .unwrap();
        remacs_runtime::frontend::prepare_native_menus(&mut interp).unwrap();
        let grid = remacs_runtime::frontend::render_grid(&interp, 80, 24);
        let menu = grid
            .menus
            .iter()
            .find(|menu| menu.label == "QA Lambda")
            .unwrap();
        let item = menu
            .items
            .iter()
            .find(|item| item.label == "Run lambda")
            .unwrap();
        assert!(
            matches!(gpui_menu_item(item), Some(MenuItem::Action { .. })),
            "an interactive lambda must produce a selectable native action"
        );
        let MenuItem::Action { action, .. } = gpui_menu_item(item).unwrap() else {
            unreachable!()
        };
        let action = action
            .as_any()
            .downcast_ref::<RunMenuCommand>()
            .unwrap()
            .clone();
        let (send, receive) = std::sync::mpsc::channel();
        let (grids, _received_grids) = smol::channel::bounded(1);
        send.send(action.event().expect("the displayed command is live"))
            .unwrap();
        let named = menu
            .items
            .iter()
            .find(|item| item.label == "Run named")
            .unwrap();
        assert_eq!(named.command.as_deref(), Some("qa-native-named"));
        let MenuItem::Action { action: named, .. } = gpui_menu_item(named).unwrap() else {
            unreachable!()
        };
        let named = named.as_any().downcast_ref::<RunMenuCommand>().unwrap();
        let event = named.event().unwrap();
        assert!(
            matches!(&event, GuiEvent::NativeMenuCommand(_)),
            "named items also need their original menu key sequence"
        );
        send.send(event).unwrap();
        drop(send);
        drop(grid);
        interp
            .eval_str("(setq global-map (make-sparse-keymap))")
            .unwrap();
        remacs_runtime::frontend::prepare_native_menus(&mut interp).unwrap();
        let frontend = Rc::new(RefCell::new(crate::logic::ChanIo::new_for_test(
            receive, grids,
        )));
        let error =
            remacs_runtime::frontend::run_editor_with_shared(frontend, &mut interp).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        assert!(interp.eval_str("(= qa-native-runs 1)").unwrap().truthy());
        assert!(
            interp
                .eval_str("(= qa-native-named-runs 1)")
                .unwrap()
                .truthy()
        );
        assert_eq!(interp.echo_message, "ran");
        assert!(
            action.event().is_none(),
            "retired native actions must release their command tokens"
        );
    }

    #[::core::prelude::v1::test]
    fn root_commands_are_actions_and_single_item_menus_remain_submenus() {
        let roots = vec![
            GridMenu {
                label: "Run now".into(),
                command: Some("ignore".into()),
                command_id: None,
                items: Vec::new(),
            },
            GridMenu {
                label: "One item".into(),
                command: None,
                command_id: None,
                items: vec![GridMenuItem {
                    help: None,
                    label: "Child".into(),
                    command: Some("ignore".into()),
                    command_id: None,
                    items: Vec::new(),
                    separator: false,
                    checked: false,
                    key_hint: None,
                }],
            },
        ];
        let app = gpui::TestAppContext::single();
        app.update(|cx| {
            cx.set_menu_bar_items(gpui_menus(&roots));
            let items = cx.get_menu_bar_items().unwrap();
            let offset = usize::from(cfg!(target_os = "macos"));
            let MenuItem::Action { action, .. } = gpui_menus(&roots).remove(offset) else {
                panic!("a root command must be a direct action");
            };
            assert!(matches!(&action.as_any().downcast_ref::<RunMenuCommand>().unwrap().command,
                NativeMenuAction::Named(name) if name == "ignore"));
            assert!(matches!(&items[offset], gpui::OwnedMenuItem::Action { name, .. } if name == "Run now"));
            assert!(matches!(&items[offset + 1], gpui::OwnedMenuItem::Submenu(menu) if menu.name.as_str() == "One item" && menu.items.len() == 1));
            let legacy = cx.get_menus().unwrap();
            assert_eq!(legacy.len(), offset + 1);
            assert_eq!(legacy[offset].name.as_str(), "One item");
            if offset != 0 {
                assert_eq!(legacy[0].name.as_str(), "REmacs");
            }
            cx.set_menus(vec![Menu { name: "Legacy".into(), items: Vec::new() }]);
            assert!(matches!(&cx.get_menu_bar_items().unwrap()[0], gpui::OwnedMenuItem::Submenu(menu) if menu.name.as_str() == "Legacy"));
        });
    }

    #[::core::prelude::v1::test]
    fn nonselectable_native_rows_keep_their_label_without_an_action() {
        let item = GridMenuItem {
            help: None,
            label: "Disabled".into(),
            command: None,
            command_id: None,
            items: Vec::new(),
            separator: false,
            checked: false,
            key_hint: None,
        };
        let converted = gpui_menu_item(&item).unwrap();
        assert!(
            matches!(&converted, MenuItem::Disabled { name, checked: false, key_hint: None } if name.as_str() == "Disabled")
        );
        assert!(
            matches!(converted.owned().clone(), gpui::OwnedMenuItem::Disabled { name, checked: false, key_hint: None } if name == "Disabled")
        );
        let enabled = GridMenuItem {
            help: None,
            command: Some("ignore".into()),
            command_id: None,
            ..item
        };
        assert!(matches!(
            gpui_menu_item(&enabled),
            Some(MenuItem::Action { .. })
        ));
    }

    #[::core::prelude::v1::test]
    fn checked_native_rows_preserve_state_and_action_through_owned_clones() {
        for checked in [true, false] {
            for command in [None, Some("ignore".into())] {
                let item = GridMenuItem {
                    help: None,
                    label: "Toggle".into(),
                    command: command.clone(),
                    command_id: None,
                    items: Vec::new(),
                    separator: false,
                    checked,
                    key_hint: None,
                };
                let owned = gpui_menu_item(&item).unwrap().owned().clone();
                match (owned, command) {
                    (gpui::OwnedMenuItem::Disabled { checked: state, .. }, None)
                    | (gpui::OwnedMenuItem::Action { checked: state, .. }, Some(_)) => {
                        assert_eq!(state, checked);
                    }
                    _ => panic!("native item must preserve its action and check state"),
                }
            }
        }
    }

    #[::core::prelude::v1::test]
    fn native_keyboard_hints_survive_conversion_without_changing_actions() {
        for command in [None, Some("ignore".into())] {
            let item = GridMenuItem {
                help: None,
                label: "Run".into(),
                command: command.clone(),
                command_id: None,
                items: Vec::new(),
                separator: false,
                checked: true,
                key_hint: Some("C-c z".into()),
            };
            let owned = gpui_menu_item(&item).unwrap().owned().clone();
            let hint = match (owned, command) {
                (
                    gpui::OwnedMenuItem::Disabled {
                        key_hint,
                        checked: true,
                        ..
                    },
                    None,
                )
                | (
                    gpui::OwnedMenuItem::Action {
                        key_hint,
                        checked: true,
                        ..
                    },
                    Some(_),
                ) => key_hint,
                _ => panic!("keyboard hints must preserve the item's action and state"),
            };
            assert_eq!(hint.as_deref(), Some("C-c z"));
        }
    }

    #[::core::prelude::v1::test]
    fn native_submenu_hints_keep_children_and_survive_owned_clones() {
        let item = GridMenuItem {
            help: None,
            label: "Branch".into(),
            command: None,
            command_id: None,
            items: vec![GridMenuItem {
                help: None,
                label: "Run".into(),
                command: Some("ignore".into()),
                command_id: None,
                items: Vec::new(),
                separator: false,
                checked: false,
                key_hint: None,
            }],
            separator: false,
            checked: false,
            key_hint: Some("C-x 4".into()),
        };
        let owned = gpui_menu_item(&item).unwrap().owned().clone();
        let gpui::OwnedMenuItem::SubmenuWithKeyHint { menu, key_hint } = owned else {
            panic!("a submenu with a keyboard hint must remain a submenu");
        };
        assert_eq!(menu.name.as_str(), "Branch");
        assert_eq!(key_hint, "C-x 4");
        assert!(matches!(&menu.items[0], gpui::OwnedMenuItem::Action { .. }));
    }
}

fn grid_image_element(
    image: GridImage,
    cell_width: Pixels,
    line_height: Pixels,
    grid: &Grid,
    border: Pixels,
    cursors: Vec<GridCursor>,
    gamma_exponent: Option<f64>,
    image_cache_entries: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_epochs: Arc<Mutex<HashMap<GridImageSource, usize>>>,
    image_cache_size: Arc<AtomicUsize>,
) -> AnyElement {
    let slice = image.resolved_slice();
    let cache_key = image.source.clone();
    let cache_epoch = image_cache_epochs
        .lock()
        .ok()
        .and_then(|epochs| epochs.get(&cache_key).copied())
        .unwrap_or(0);
    let mut id_hasher = DefaultHasher::new();
    image.row.hash(&mut id_hasher);
    image.column.hash(&mut id_hasher);
    cache_epoch.hash(&mut id_hasher);
    let element_id = ("remacs-image", id_hasher.finish());
    let frame_index = image.index;
    let conversion = image.conversion;
    let transform_smoothing = image.transform_smoothing;
    let max_image_size = image.max_image_size.map(|limit| match limit {
        GridImageSizeLimit::Pixels(pixels) => ImageSizeLimit::Pixels(pixels),
        GridImageSizeLimit::FrameRatioBits(bits) => {
            ImageSizeLimit::FrameRatio(f64::from_bits(bits))
        }
    });
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
            let previous = entries.insert(cache_key.clone(), image_bytes).unwrap_or(0);
            let _ = image_cache_size.fetch_update(Ordering::AcqRel, Ordering::Acquire, |total| {
                Some(total.saturating_sub(previous).saturating_add(image_bytes))
            });
        }
    });
    let source = match frame_index {
        Some(index) => source.frame_index(index),
        None => source,
    };
    let source = match conversion {
        Some(GridImageConversion::Disabled) => source.disabled_conversion(),
        Some(GridImageConversion::EdgeDetection {
            matrix,
            color_adjust,
        }) => source.edge_detection(matrix, color_adjust),
        None => source,
    };
    let source = match transform_smoothing {
        Some(smoothing) => source.transform_smoothing(smoothing),
        None => source,
    };
    let source = match max_image_size {
        Some(limit) => source.max_image_size(limit),
        None => source,
    };
    let source = match (image.width, image.height) {
        (Some(width), Some(height)) => source.w(px(width as f32)).h(px(height as f32)),
        (Some(width), None) => source.w(px(width as f32)),
        (None, Some(height)) => source.h(px(height as f32)),
        (None, None) => source,
    };
    let source = grid_image_slice_element(source.id(element_id).into_any_element(), slice);
    let image_left = border + cell_width * image.column as f32;
    let image_column = image.column;
    let image_top =
        border + grid_row_top_for_cell(grid, image.row, image.column, line_height);
    let scroll = grid.vscrolls.iter().find(|scroll| {
        image.row >= scroll.top
            && image.row < scroll.top.saturating_add(scroll.rows)
            && image.column >= scroll.left
            && image.column < scroll.left.saturating_add(scroll.width)
    });
    if let Some(scroll) = scroll {
        let first_row = scroll.top.min(grid.row_heights.len());
        let end_row = scroll
            .top
            .saturating_add(scroll.rows)
            .min(grid.row_heights.len());
        let region_left = border + cell_width * scroll.left as f32;
        let region_top =
            border + grid_window_row_top(grid, scroll.window_id, first_row, line_height);
        let region_bottom =
            border + grid_window_row_top(grid, scroll.window_id, end_row, line_height);
        let mut image_element = div()
            .absolute()
            .left(image_left - region_left)
            .top(image_top - region_top - px(scroll.pixels as f32))
            .child(source);
        for cursor in cursors {
            image_element = image_element.child(grid_image_cursor_overlay(
                cursor,
                image_column,
                cell_width,
                line_height,
                gamma_exponent,
            ));
        }
        div()
            .absolute()
            .left(region_left)
            .top(region_top)
            .w(cell_width * scroll.width as f32)
            .h(region_bottom - region_top)
            .overflow_hidden()
            .child(image_element)
            .into_any_element()
    } else {
        let mut image_element = div()
            .absolute()
            .left(image_left)
            .top(image_top)
            .child(source);
        for cursor in cursors {
            image_element = image_element.child(grid_image_cursor_overlay(
                cursor,
                image_column,
                cell_width,
                line_height,
                gamma_exponent,
            ));
        }
        image_element.into_any_element()
    }
}

fn grid_image_slice_element(
    source: AnyElement,
    slice: Option<remacs_runtime::frontend::GridImageSlicePixels>,
) -> AnyElement {
    let Some(slice) = slice else {
        return source;
    };
    div()
        .relative()
        .w(px(slice.width as f32))
        .h(px(slice.height as f32))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(px(-(slice.x as f32)))
                .top(px(-(slice.y as f32)))
                .child(source),
        )
        .into_any_element()
}

fn grid_cursor_blink_visible(grid: &Grid) -> bool {
    if !grid.blink_cursor {
        return true;
    }
    let elapsed = grid.blink_elapsed_since_input - grid.blink_delay;
    let interval = grid.blink_interval.max(0.05);
    let completed = grid.blink_blinks > 0 && elapsed >= interval * 2.0 * grid.blink_blinks as f64;
    elapsed < 0.0 || completed || (elapsed / interval).floor() as u64 % 2 == 1
}

fn prepare_block_cursor_glyphs(grid: &mut Grid, default_background: u32) -> HashSet<(usize, usize)> {
    let blink_visible = grid_cursor_blink_visible(grid);
    let cursors = grid.cursors.clone();
    let frame_background = CellColor(
        (default_background >> 16) as u8,
        (default_background >> 8) as u8,
        default_background as u8,
    );
    let mut styled = HashSet::new();
    for cursor in cursors {
        if cursor.shape != remacs_runtime::frontend::GridCursorShape::Block
            || cursor.thickness > 0
            || (cursor.selected && grid.blink_cursor && !blink_visible)
            || grid.images.iter().any(|image| {
                image.row == cursor.row && image.column == cursor.column
            })
            || grid.compositions.iter().any(|composition| {
                composition.row == cursor.row && composition.byte_index == cursor.index
            })
        {
            continue;
        }
        let Some(char_index) = grid.rows.get(cursor.row).and_then(|text| {
            text.char_indices()
                .enumerate()
                .find_map(|(index, (byte, _))| (byte == cursor.index).then_some(index))
        }) else {
            continue;
        };
        let mut styles = grid
            .char_attrs
            .get(cursor.row)
            .filter(|styles| !styles.is_empty())
            .or_else(|| grid.attrs.get(cursor.row))
            .cloned()
            .unwrap_or_default();
        styles.resize(char_index.saturating_add(1), Default::default());
        let cursor_background = cursor.color.unwrap_or_else(|| {
            let color = gamma_correct_rgb(CURSOR, grid.gamma_exponent);
            CellColor(
                (color >> 16) as u8,
                (color >> 8) as u8,
                color as u8,
            )
        });
        let style = &mut styles[char_index];
        style.fg = Some(frame_background);
        style.bg = Some(cursor_background);
        style.distant_foreground = None;
        style.inverse = false;
        style.extend = false;
        style.stipple = None;
        if grid.char_attrs.len() <= cursor.row {
            grid.char_attrs.resize_with(cursor.row + 1, Vec::new);
        }
        grid.char_attrs[cursor.row] = styles;
        styled.insert((cursor.row, cursor.index));
    }
    styled
}

fn grid_image_cursor_overlay(
    cursor: GridCursor,
    image_column: usize,
    cell_width: Pixels,
    line_height: Pixels,
    gamma_exponent: Option<f64>,
) -> AnyElement {
    let rgb_color = cursor.color.map_or_else(
        || gamma_correct_rgb(CURSOR, gamma_exponent),
        |color| ((color.0 as u32) << 16) | ((color.1 as u32) << 8) | color.2 as u32,
    );
    let width = match cursor.shape {
        remacs_runtime::frontend::GridCursorShape::Bar => px(cursor.thickness.max(1) as f32),
        remacs_runtime::frontend::GridCursorShape::HBar => cell_width,
        remacs_runtime::frontend::GridCursorShape::Block
        | remacs_runtime::frontend::GridCursorShape::Hollow => {
            if cursor.thickness > 0 {
                px(cursor.thickness as f32)
            } else {
                cell_width
            }
        }
    };
    let height = match cursor.shape {
        remacs_runtime::frontend::GridCursorShape::HBar => px(cursor.thickness.max(1) as f32),
        _ => line_height,
    };
    let left = cell_width * cursor.column.saturating_sub(image_column) as f32;
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, (), window, _cx| match cursor.shape {
            remacs_runtime::frontend::GridCursorShape::Block => {
                window.paint_quad(fill(
                    Bounds {
                        origin: bounds.origin,
                        size: size(width, height),
                    },
                    rgba((rgb_color << 8) | 0x80),
                ));
            }
            remacs_runtime::frontend::GridCursorShape::Hollow => {
                let color = rgb(rgb_color);
                let edge = px(1.0);
                window.paint_quad(fill(
                    Bounds {
                        origin: bounds.origin,
                        size: size(width, edge),
                    },
                    color,
                ));
                window.paint_quad(fill(
                    Bounds {
                        origin: point(bounds.origin.x, bounds.origin.y + height - edge),
                        size: size(width, edge),
                    },
                    color,
                ));
                window.paint_quad(fill(
                    Bounds {
                        origin: bounds.origin,
                        size: size(edge, height),
                    },
                    color,
                ));
                window.paint_quad(fill(
                    Bounds {
                        origin: point(bounds.origin.x + width - edge, bounds.origin.y),
                        size: size(edge, height),
                    },
                    color,
                ));
            }
            remacs_runtime::frontend::GridCursorShape::Bar => {
                window.paint_quad(fill(
                    Bounds {
                        origin: bounds.origin,
                        size: size(width, height),
                    },
                    rgb(rgb_color),
                ));
            }
            remacs_runtime::frontend::GridCursorShape::HBar => {
                window.paint_quad(fill(
                    Bounds {
                        origin: point(bounds.origin.x, bounds.origin.y + line_height - height),
                        size: size(width, height),
                    },
                    rgb(rgb_color),
                ));
            }
        },
    )
    .absolute()
    .left(left)
    .top(px(0.0))
    .w(width)
    .h(line_height)
    .into_any_element()
}

fn grid_data_image(
    format: String,
    bytes: Arc<[u8]>,
    base_uri: Option<String>,
    css: Option<String>,
) -> Option<Img> {
    grid_data_image_source(format, bytes, base_uri, css).map(img)
}

fn grid_data_image_source(
    format: String,
    bytes: Arc<[u8]>,
    base_uri: Option<String>,
    css: Option<String>,
) -> Option<ImageSource> {
    if format == "pbm" {
        let image = Image::from_pnm_bytes(bytes.to_vec())?;
        return Some(ImageSource::Image(Arc::new(image)));
    }
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
        (Some(base_uri), None) => Some(ImageSource::ImageWithBaseUri {
            image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
            base_uri: std::path::PathBuf::from(base_uri),
        }),
        (base_uri, Some(css)) => Some(ImageSource::ImageWithSvgOptions {
            image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
            base_uri: base_uri.map(std::path::PathBuf::from),
            css: Some(css),
        }),
        (None, None) => Some(ImageSource::Image(Arc::new(Image::from_bytes(
            format,
            bytes.to_vec(),
        )))),
    }
}

fn grid_image_source(source: &GridImageSource) -> Option<ImageSource> {
    match source {
        GridImageSource::File(path) => Some(ImageSource::from(std::path::PathBuf::from(path))),
        GridImageSource::FileWithSvgOptions {
            path,
            base_uri,
            css,
        } => Some(ImageSource::ResourceWithSvgOptions {
            resource: std::path::PathBuf::from(path).into(),
            base_uri: base_uri.as_deref().map(std::path::PathBuf::from),
            css: css.clone(),
        }),
        GridImageSource::Data { format, bytes } => {
            grid_data_image_source(format.clone(), bytes.clone(), None, None)
        }
        GridImageSource::DataWithBaseUri {
            format,
            bytes,
            base_uri,
        } => grid_data_image_source(format.clone(), bytes.clone(), Some(base_uri.clone()), None),
        GridImageSource::DataWithSvgOptions {
            format,
            bytes,
            base_uri,
            css,
        } => grid_data_image_source(format.clone(), bytes.clone(), base_uri.clone(), css.clone()),
    }
}

fn image_format_for_path(path: &str) -> Option<ImageFormat> {
    let extension = std::path::Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::Webp),
        "gif" => Some(ImageFormat::Gif),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        _ => None,
    }
}

fn image_from_data(format: &str, bytes: &[u8]) -> Result<Image, String> {
    if matches!(format, "pbm" | "ppm" | "pgm" | "pam") {
        return Image::from_pnm_bytes(bytes.to_vec())
            .ok_or_else(|| "invalid portable anymap image".to_owned());
    }
    let format = match format.to_ascii_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "svg" => ImageFormat::Svg,
        "bmp" => ImageFormat::Bmp,
        "tif" | "tiff" => ImageFormat::Tiff,
        _ => return Err("unsupported image format".to_owned()),
    };
    Ok(Image::from_bytes(format, bytes.to_vec()))
}

fn image_from_grid_source(
    source: &GridImageSource,
) -> Result<(Image, Option<std::path::PathBuf>, Option<String>), String> {
    match source {
        GridImageSource::File(path) => {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            let image = if matches!(
                std::path::Path::new(path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("pbm" | "ppm" | "pgm" | "pam")
            ) {
                Image::from_pnm_bytes(bytes)
                    .ok_or_else(|| "invalid portable anymap image".to_owned())?
            } else {
                let format = image_format_for_path(path)
                    .ok_or_else(|| "unsupported image format".to_owned())?;
                Image::from_bytes(format, bytes)
            };
            return Ok((image, None, None));
        }
        GridImageSource::FileWithSvgOptions {
            path,
            base_uri,
            css,
        } => Ok((
            Image::from_bytes(
                ImageFormat::Svg,
                std::fs::read(path).map_err(|error| error.to_string())?,
            ),
            base_uri.as_deref().map(std::path::PathBuf::from),
            css.clone(),
        )),
        GridImageSource::Data { format, bytes } => {
            Ok((image_from_data(format, bytes)?, None, None))
        }
        GridImageSource::DataWithBaseUri {
            format,
            bytes,
            base_uri,
        } => Ok((
            image_from_data(format, bytes)?,
            Some(std::path::PathBuf::from(base_uri)),
            None,
        )),
        GridImageSource::DataWithSvgOptions {
            format,
            bytes,
            base_uri,
            css,
        } => Ok((
            image_from_data(format, bytes)?,
            base_uri.as_deref().map(std::path::PathBuf::from),
            css.clone(),
        )),
    }
}

pub(super) fn inspect_image_source(
    source: &GridImageSource,
    index: Option<usize>,
    app: &App,
) -> Result<Option<remacs_runtime::frontend::ImageInfo>, String> {
    let svg_renderer = app.svg_renderer();
    let (image, base_uri, css) = image_from_grid_source(source)?;
    let data = image
        .to_image_data_with_svg_options(svg_renderer.clone(), base_uri.as_deref(), css.as_deref())
        .map_err(|error| error.to_string())?;
    let frame_index = index.unwrap_or(0);
    if frame_index >= data.frame_count() {
        return Ok(None);
    }
    let size = data.size(frame_index);
    let svg_scale = if image.format == ImageFormat::Svg {
        2
    } else {
        1
    };
    let width = size.width.0.max(0) as usize / svg_scale;
    let height = size.height.0.max(0) as usize / svg_scale;
    let pixels = data.as_bytes(frame_index).unwrap_or_default();
    let has_mask = pixels
        .chunks_exact(4)
        .any(|pixel| pixel.get(3).is_some_and(|alpha| *alpha < 255));
    let (delay_numerator, delay_denominator) = data.delay(frame_index).numer_denom_ms();
    let frame_delay = (delay_numerator > 0 && delay_denominator > 0)
        .then(|| delay_numerator as f64 / delay_denominator as f64 / 1000.0);
    Ok(Some(remacs_runtime::frontend::ImageInfo {
        width,
        height,
        has_mask,
        frame_count: data.frame_count(),
        frame_delay,
    }))
}

fn image_source_matches_filename(source: &GridImageSource, filename: &str) -> bool {
    match source {
        GridImageSource::File(path) | GridImageSource::FileWithSvgOptions { path, .. } => {
            path == filename
        }
        GridImageSource::Data { .. }
        | GridImageSource::DataWithBaseUri { .. }
        | GridImageSource::DataWithSvgOptions { .. } => false,
    }
}

fn remove_grid_image_asset(source: &GridImageSource, cx: &mut Context<EditorView>) {
    if let Some(source) = grid_image_source(source) {
        let app = <Context<'_, EditorView> as std::borrow::BorrowMut<App>>::borrow_mut(cx);
        source.remove_asset(app);
    }
}

fn bump_image_cache_epoch(view: &EditorView, source: &GridImageSource) {
    if let Ok(mut epochs) = view.image_cache_epochs.lock() {
        let epoch = epochs.entry(source.clone()).or_default();
        *epoch = epoch.saturating_add(1);
    }
}

pub(super) fn flush_image_source(
    view: &mut EditorView,
    source: &GridImageSource,
    cx: &mut Context<EditorView>,
) {
    remove_grid_image_asset(source, cx);
    bump_image_cache_epoch(view, source);
    if let Ok(mut entries) = view.image_cache_entries.lock() {
        let removed = entries.remove(source).unwrap_or(0);
        let _ = view
            .image_cache_size
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |total| {
                Some(total.saturating_sub(removed))
            });
    }
    cx.notify();
}

pub(super) fn clear_image_cache(
    view: &mut EditorView,
    filename: Option<&str>,
    cx: &mut Context<EditorView>,
) {
    let removed = if let Ok(mut entries) = view.image_cache_entries.lock() {
        let removed: Vec<_> = entries
            .keys()
            .filter(|source| {
                filename.is_none_or(|filename| image_source_matches_filename(source, filename))
            })
            .cloned()
            .collect();
        let removed_bytes = removed.iter().fold(0usize, |total, source| {
            total.saturating_add(entries.remove(source).unwrap_or(0))
        });
        let _ = view
            .image_cache_size
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |total| {
                Some(total.saturating_sub(removed_bytes))
            });
        removed
    } else {
        Vec::new()
    };
    for source in removed {
        remove_grid_image_asset(&source, cx);
        bump_image_cache_epoch(view, &source);
    }
    cx.notify();
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
    let (foreground, background) =
        style_colors(style, default_background, background_alpha, gamma_exponent);
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
        let width =
            (cell_width * (right - left) as f32 + px(segment.width_adjustment as f32)).max(px(0.0));
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
fn paint_grid_row_at(
    grid: &Grid,
    row: usize,
    line: &ShapedLine,
    bounds: Bounds<Pixels>,
    row_top: Pixels,
    row_height: Pixels,
    text_offset: Pixels,
    line_height: Pixels,
    cell_width: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
    cx: &mut App,
) {
    let paint_line_height = line_height.max(line.ascent + line.descent);
    let text_top = row_top + text_offset;
    let text_height = (row_height - text_offset).max(px(1.0));
    let origin = point(bounds.left(), bounds.top() + text_top);
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
        text_height,
        cell_width,
        paint_line_height,
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
        text_top,
        paint_line_height,
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
                let overline_y = overline_y_for_byte(
                    origin.y,
                    paint_line_height,
                    line,
                    byte,
                    window,
                );
                window.paint_quad(fill(
                    Bounds {
                        origin: point(bounds.left() + left, overline_y),
                        size: size(width, px(1.0)),
                    },
                    overline_color(
                        style,
                        default_background,
                        background_alpha,
                        grid.gamma_exponent,
                    ),
                ));
            }
        }
    }
}

fn overline_y_from_run_ascent(
    origin_y: Pixels,
    line_height: Pixels,
    line_ascent: Pixels,
    line_descent: Pixels,
    run_ascent: Pixels,
) -> Pixels {
    origin_y + (line_height - line_ascent - line_descent) / 2.0 + line_ascent - run_ascent
        + px(1.0)
}

fn overline_y_for_byte(
    origin_y: Pixels,
    line_height: Pixels,
    line: &ShapedLine,
    byte: usize,
    window: &Window,
) -> Pixels {
    let run_ascent = line
        .runs
        .iter()
        .filter_map(|run| {
            run.glyphs
                .iter()
                .map(|glyph| glyph.index)
                .min()
                .filter(|start| *start <= byte)
                .map(|start| (start, run))
        })
        .max_by_key(|(start, _)| *start)
        .map(|(_, run)| run)
        .map(|run| window.text_system().ascent(run.font_id, run.font_size))
        .unwrap_or(line.ascent);
    overline_y_from_run_ascent(origin_y, line_height, line.ascent, line.descent, run_ascent)
}

#[allow(clippy::too_many_arguments)]
fn paint_grid_row_with_window_heights(
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
    let areas = grid
        .window_text_areas
        .iter()
        .filter(|area| row >= area.top && row < area.top.saturating_add(area.rows))
        .copied()
        .collect::<Vec<_>>();
    let columns = grid.attrs.get(row).map_or(0, Vec::len);
    let unscrolled = unscrolled_column_ranges(grid, row, columns);
    let mut covered_areas = areas
        .iter()
        .map(|area| {
            (
                area.left.min(columns),
                area.left.saturating_add(area.width).min(columns),
            )
        })
        .filter(|(left, right)| left < right)
        .collect::<Vec<_>>();
    covered_areas.sort_unstable();
    let global_top = grid_row_top(grid, row, line_height);
    let global_height = grid_row_height(grid, row, line_height);
    let mut uncovered = Vec::new();
    let mut left = 0usize;
    for (area_left, area_right) in covered_areas {
        if area_left > left {
            uncovered.push((left, area_left));
        }
        left = left.max(area_right);
    }
    if left < columns {
        uncovered.push((left, columns));
    }
    for (uncovered_left, uncovered_right) in uncovered {
        for (scroll_left, scroll_right) in &unscrolled {
            let left = uncovered_left.max(*scroll_left);
            let right = uncovered_right.min(*scroll_right);
            if left >= right {
                continue;
            }
            let clip = Bounds {
                origin: point(
                    bounds.left() + cell_width * left as f32,
                    bounds.top() + global_top,
                ),
                size: size(cell_width * (right - left) as f32, global_height),
            };
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                paint_grid_row_at(
                    grid,
                    row,
                    line,
                    bounds,
                    global_top,
                    global_height,
                    px(0.0),
                    line_height,
                    cell_width,
                    default_background,
                    background_alpha,
                    window,
                    cx,
                );
            });
        }
    }

    for area in areas {
        let row_top = grid_window_row_top(grid, area.window_id, row, line_height);
        let row_height = grid_window_row_height(grid, area.window_id, row, line_height);
        let area_left = area.left.min(columns);
        let area_right = area.left.saturating_add(area.width).min(columns);
        for (scroll_left, scroll_right) in &unscrolled {
            let left = area_left.max(*scroll_left);
            let right = area_right.min(*scroll_right);
            if left >= right {
                continue;
            }
            let clip = Bounds {
                origin: point(
                    bounds.left() + cell_width * left as f32,
                    bounds.top() + row_top,
                ),
                size: size(cell_width * (right - left) as f32, row_height),
            };
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                paint_grid_row_at(
                    grid,
                    row,
                    line,
                    bounds,
                    row_top,
                    row_height,
                    grid_window_row_text_offset(grid, area.window_id, row),
                    line_height,
                    cell_width,
                    default_background,
                    background_alpha,
                    window,
                    cx,
                );
            });
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

        let region_top =
            bounds.top() + grid_window_row_top(grid, scroll.window_id, first_row, line_height);
        let region_bottom =
            bounds.top() + grid_window_row_top(grid, scroll.window_id, end_row, line_height);
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
                let row_top = grid_window_row_top(grid, scroll.window_id, row, line_height) - pixels;
                let row_height = grid_window_row_height(grid, scroll.window_id, row, line_height);
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
                            let overline_y =
                                overline_y_for_byte(origin.y, line_height, line, byte, window);
                            window.paint_quad(fill(
                                Bounds {
                                    origin: point(bounds.left() + left, overline_y),
                                    size: size(width, px(1.0)),
                                },
                                overline_color(
                                    *style,
                                    default_background,
                                    background_alpha,
                                    grid.gamma_exponent,
                                ),
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
        let row_height = grid_row_height_for_cell(
            grid,
            composition.row,
            composition.column,
            line_height,
        );
        let origin = point(
            bounds.left() + x,
            bounds.top()
                + grid_row_top_for_cell(
                    grid,
                    composition.row,
                    composition.column,
                    line_height,
                )
                - px(vscroll_pixels_for_cell(grid, composition.row, composition.column) as f32),
        );
        with_vscroll_cell_clip(
            grid,
            composition.row,
            composition.column,
            bounds,
            cell_width,
            row_height,
            window,
            |window| {
                let text = composition.components.iter().collect::<String>();
                let composition_line = window.text_system().shape_line(
                    text.clone().into(),
                    font_size,
                    &[TextRun {
                        len: text.len(),
                        font: component_font,
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                let _ = composition_line.paint(origin, line_height, window, cx);
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
        ((value.powf(exponent) * 255.0 + 0.5)
            .floor()
            .clamp(0.0, 255.0) as u32)
            << shift
    };
    channel(16) | channel(8) | channel(0)
}

fn paint_right_dividers(
    grid: &Grid,
    bounds: Bounds<Pixels>,
    cell_width: Pixels,
    line_height: Pixels,
    default_background: u32,
    background_alpha: u8,
    window: &mut Window,
) {
    for divider in &grid.right_dividers {
        if divider.thickness == 0 || divider.bottom <= divider.top {
            continue;
        }
        let thickness = px(divider.thickness as f32);
        let left = bounds.left()
            + cell_width * divider.column_boundary as f32
            + px(divider.width_adjustment as f32)
            - thickness;
        let top = bounds.top()
            + grid_row_top(grid, divider.top, line_height)
            + px(divider.y_offset as f32);
        let bottom = bounds.top()
            + grid_row_top(grid, divider.bottom, line_height)
            + px(divider.height_adjustment as f32);
        let height = (bottom - top).max(px(0.0));
        if height == px(0.0) {
            continue;
        }
        let paint_column = |offset: usize, width: usize, style, window: &mut Window| {
            if width == 0 {
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
                    origin: point(left + px(offset as f32), top),
                    size: size(px(width as f32), height),
                },
                color,
            ));
        };
        if divider.thickness < 3 {
            paint_column(0, divider.thickness, divider.style, window);
        } else {
            paint_column(0, 1, divider.first_pixel_style, window);
            paint_column(
                1,
                divider.thickness.saturating_sub(2),
                divider.style,
                window,
            );
            paint_column(
                divider.thickness.saturating_sub(1),
                1,
                divider.last_pixel_style,
                window,
            );
        }
    }
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
        let left = bounds.left() + cell_width * divider.left as f32 + px(divider.x_offset as f32);
        let width =
            (cell_width * divider.width as f32 + px(divider.width_adjustment as f32)).max(px(0.0));
        let top = bounds.top() + grid_row_top(grid, divider.row_boundary, line_height) - thickness;
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

fn overline_color(
    style: CellStyle,
    default_background: u32,
    background_alpha: u8,
    gamma_exponent: Option<f64>,
) -> Hsla {
    style
        .overline_color
        .map(|color| rgb(color_rgb(color)).into())
        .unwrap_or_else(|| {
            style_colors(style, default_background, background_alpha, gamma_exponent).0
        })
}

#[cfg(test)]
mod overline_color_tests {
    use super::overline_color;
    use gpui::rgb;
    use remacs_runtime::frontend::CellColor;
    use remacs_runtime::lisp::Interp;

    #[test]
    fn explicit_overline_color_survives_face_resolution() {
        let mut i = Interp::new();
        i.eval_str(
            r##"(progn
              (make-face 'qa-overline-explicit)
              (set-face-attribute 'qa-overline-explicit nil
                                  :foreground "#00ff00" :overline "#ff0000")
              (make-face 'qa-overline-foreground)
              (set-face-attribute 'qa-overline-foreground nil
                                  :foreground "#0000ff" :overline t)
              (erase-buffer)
              (insert (propertize "R" 'face 'qa-overline-explicit)
                      (propertize "T" 'face 'qa-overline-foreground)))"##,
        )
        .unwrap();
        let grid = remacs_runtime::frontend::render_grid(&i, 20, 4);
        let styles = &grid.char_attrs[0];
        let explicit = styles[0];
        let foreground = styles[1];

        assert!(explicit.overline);
        assert_eq!(explicit.overline_color, Some(CellColor(255, 0, 0)));
        assert!(foreground.overline);
        assert_eq!(foreground.overline_color, None);
        assert_eq!(overline_color(explicit, 0, 255, None), rgb(0xff0000).into());
        assert_eq!(overline_color(foreground, 0, 255, None), rgb(0x0000ff).into());
    }
}

fn scroll_bar_colors(
    bar: &GridScrollBar,
    default_background: u32,
    background_alpha: u8,
    gamma_exponent: Option<f64>,
) -> (Hsla, Hsla) {
    let (thumb, track) = style_colors(
        bar.style,
        default_background,
        background_alpha,
        gamma_exponent,
    );
    (
        thumb,
        track.unwrap_or_else(|| background_rgba(default_background, background_alpha)),
    )
}

fn background_rgba(color: u32, alpha: u8) -> Hsla {
    rgba((color << 8) | u32::from(alpha)).into()
}

#[cfg(test)]
mod overline_position_tests {
    use super::{overline_y_for_byte, overline_y_from_run_ascent};
    use gpui::{
        Context, IntoElement, Render, TestAppContext, TextRun, Window, div, font, px, rgb,
    };

    struct TextMetricsView;

    impl Render for TextMetricsView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[test]
    fn overline_tracks_the_ascent_of_its_font_run() {
        let origin_y = px(10.0);
        let line_height = px(24.0);
        let line_ascent = px(16.0);
        let line_descent = px(5.0);

        let tall_run = overline_y_from_run_ascent(
            origin_y,
            line_height,
            line_ascent,
            line_descent,
            px(16.0),
        );
        let short_run = overline_y_from_run_ascent(
            origin_y,
            line_height,
            line_ascent,
            line_descent,
            px(12.0),
        );

        assert_eq!(tall_run, px(12.5));
        assert_eq!(short_run, px(16.5));
    }

    #[test]
    fn overline_position_uses_the_ascent_of_the_matching_shaped_run() {
        let mut app = TestAppContext::single_with_native_text_system();
        let handle = app.add_window(|_, _| TextMetricsView);
        let (run_count, first_y, second_y) = handle
            .update(&mut app, |_, window, _| {
                let candidates = window
                    .text_system()
                    .all_font_names()
                    .into_iter()
                    .map(|name| {
                        let font = font(name.clone());
                        let id = window.text_system().resolve_font(&font);
                        let ascent = window.text_system().ascent(id, px(16.0));
                        (name, id, ascent)
                    })
                    .collect::<Vec<_>>();
                let (first_name, first_id, first_ascent) = candidates
                    .first()
                    .expect("text system should provide at least one font")
                    .clone();
                let (second_name, _, _) = candidates
                    .iter()
                    .find(|(_, id, ascent)| *id != first_id && *ascent != first_ascent)
                    .cloned()
                    .expect("text system should provide fonts with different ascents");
                let make_run = |name| TextRun {
                    len: 1,
                    font: font(name),
                    color: rgb(0x000000).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let runs = [make_run(first_name), make_run(second_name)];
                let font_sizes = [None, None];
                let line = window.text_system().shape_line_with_font_sizes(
                    "ab".into(),
                    px(16.0),
                    &runs,
                    &font_sizes,
                    None,
                );
                (
                    line.runs.len(),
                    overline_y_for_byte(px(0.0), px(24.0), &line, 0, window),
                    overline_y_for_byte(px(0.0), px(24.0), &line, 1, window),
                )
            })
            .unwrap();

        assert!(run_count >= 2, "each face should produce a shaped run");
        assert_ne!(first_y, second_y);
    }
}

#[cfg(test)]
mod scroll_bar_color_tests {
    use super::scroll_bar_colors;
    use gpui::{rgb, rgba};
    use remacs_runtime::frontend::{CellColor, CellStyle, GridScrollBar, GridScrollBarOrientation};

    fn bar(style: CellStyle) -> GridScrollBar {
        GridScrollBar {
            window_id: 1,
            buffer_id: 1,
            orientation: GridScrollBarOrientation::Vertical,
            style,
            left: 0,
            top: 0,
            width: 8,
            height: 100,
            thumb_start: 0,
            thumb_size: 20,
            max_offset: 80,
        }
    }

    #[::core::prelude::v1::test]
    fn foreground_paints_thumb_and_background_paints_track() {
        let bar = bar(CellStyle {
            fg: Some(CellColor(255, 0, 0)),
            bg: Some(CellColor(255, 255, 0)),
            ..Default::default()
        });
        let (thumb, track) = scroll_bar_colors(&bar, 0, 255, None);
        assert_eq!(thumb, rgb(0xff0000).into());
        assert_eq!(track, rgb(0xffff00).into());
    }

    #[::core::prelude::v1::test]
            text_width: 80,
    fn inverse_alpha_and_corrected_colors_use_the_common_style_rules() {
        let bar = bar(CellStyle {
            fg: Some(CellColor(128, 64, 32)),
            bg: Some(CellColor(16, 32, 48)),
            inverse: true,
            ..Default::default()
        });
        let (thumb, track) = scroll_bar_colors(&bar, 0, 128, Some(0.5));
        // Explicit grid colors have already been gamma corrected by runtime.
        assert_eq!(thumb, rgb(0x102030).into());
        assert_eq!(track, rgba(0x80402080).into());
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
            if !matches!(
                cx.update(|window, cx| this.update(cx, |v, cx| {
                    if v.grid != grid || grid.blink_cursor {
                        v.set_grid(grid, window);
                        cx.notify();
                    }
                })),
                Ok(Ok(()))
            ) {
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
    quit_request: Arc<QuitRequest>,
    frame_id: Option<usize>,
    grid: Option<Grid>,
    focus_on_map: bool,
    no_accept_focus: bool,
) -> EditorView {
    let mut view = EditorView::new_for_frame_with_quit(event_tx, frame_id, Some(quit_request), cx);
    view.no_accept_focus.set(no_accept_focus);
    if let Some(grid) = grid {
        view.set_grid(grid, window);
    }
    let close_events = view.tx.clone();
    let native_close_on_alt_f4 = view.native_close_on_alt_f4.clone();
    let native_close_on_command_w = view.native_close_on_command_w.clone();
    window.on_window_should_close(cx, move |_window, cx| {
        let alt_f4_close = native_close_on_alt_f4.replace(false);
        let command_w_close = native_close_on_command_w.replace(false);
        if cx.windows().len() <= 1 {
            if alt_f4_close || command_w_close {
                let _ = close_events.send(GuiEvent::PowerOff);
            } else if close_events.frame_id().is_none() {
                let _ = close_events.send_delete_request_or_defer();
            }
            return false;
        }
        close_events.send_delete_request_or_defer().is_err()
    });
    let frame_events = view.tx.clone();
    let focus_modifier_state = view.physical_modifier_state.clone();
    let subscription = window.on_focus_out(&view.focus, cx, move |_event, window, _cx| {
        clear_physical_modifier_state(&focus_modifier_state);
        if let Some(frame_id) = frame_events.frame_id() {
            let visibility = crate::native_frame_visibility(window);
            let _ = frame_events.send_visibility_update(frame_id, visibility);
        }
    });
    view.focus_subscriptions.push(subscription);
    let no_accept_focus = view.no_accept_focus.clone();
    let no_accept_focus_on_focus_in = no_accept_focus.clone();
    let frame_events = view.tx.clone();
    let subscription = window.on_focus_in(&view.focus, cx, move |window, _cx| {
        if no_accept_focus_on_focus_in.get() {
            window.blur();
        }
        if let Some(frame_id) = frame_events.frame_id() {
            let visibility = crate::native_frame_visibility(window);
            let _ = frame_events.send_visibility_update(frame_id, visibility);
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

#[cfg(all(test, target_os = "macos"))]
mod accessibility_frame_key_tests {
    use super::*;

    fn geometry() -> GuiGeometry {
        GuiGeometry {
            cols: 80,
            rows: 24,
            pixel_width: 640,
            pixel_height: 480,
            outer_width: 640,
            outer_height: 500,
            char_width: 8,
            char_height: 19,
            scale_factor: 1.0,
            frame_left: 10,
            frame_top: 20,
            fullscreen: false,
            maximized: false,
            current_display_id: Some(1),
            monitors: Vec::new(),
        }
    }

    #[::core::prelude::v1::test]
    fn key_changes_when_native_window_moves_or_resizes() {
        let original = geometry();
        let original_key = accessibility_frame_key(&original);

        let mut moved = original.clone();
        moved.frame_left += 100;
        assert_ne!(accessibility_frame_key(&moved), original_key);

        let mut resized = original;
        resized.pixel_width += 100;
        assert_ne!(accessibility_frame_key(&resized), original_key);
    }
}

#[cfg(test)]
mod file_drop_uri_tests {
    use super::file_drop_uri_from_path;
    #[cfg(unix)]
    use super::file_drop_uri;
    #[cfg(unix)]
    use std::path::Path;

    #[cfg(unix)]
    #[test]
    fn file_drop_uri_escapes_utf8_paths() {
        assert_eq!(
            file_drop_uri(Path::new("/tmp/remacs drop/日本.txt")),
            "file:///tmp/remacs%20drop/%E6%97%A5%E6%9C%AC.txt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_drop_uri_preserves_non_utf8_path_bytes() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

        let path = PathBuf::from(OsString::from_vec(b"/tmp/remacs-\xff.txt".to_vec()));
        assert_eq!(file_drop_uri(&path), "file:///tmp/remacs-%FF.txt");
    }

    #[test]
    fn windows_drive_path_uses_a_local_file_uri() {
        assert_eq!(
            file_drop_uri_from_path(r"C:\Temp\sample file.txt"),
            "file:///C:/Temp/sample%20file.txt"
        );
    }

    #[test]
    fn windows_unc_path_maps_the_server_to_uri_authority() {
        assert_eq!(
            file_drop_uri_from_path(r"\\server\share\sample file.txt"),
            "file://server/share/sample%20file.txt"
        );
        assert_eq!(
            file_drop_uri_from_path(r"\\server\share\日本.txt"),
            "file://server/share/%E6%97%A5%E6%9C%AC.txt"
        );
    }
}
