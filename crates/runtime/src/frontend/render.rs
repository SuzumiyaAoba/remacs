//! Resolve faces and render editor state into a frontend-neutral grid.
use super::MousePosition;
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use crate::{
    buffer::Buffer,
    lisp::{Interp, Value},
};
use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
    sync::Arc,
};
use unicode_width::UnicodeWidthChar;

/// A terminal color: RGB. Face color specs (names, `#rgb' forms) are
/// resolved to this at render time; the crossterm front-end emits
/// true-color SGR, the gpui front-end paints it directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellColor(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BoxStyleKind {
    #[default]
    Flat,
    Released,
    Pressed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BoxStyle {
    /// Left and right border widths. Negative widths are drawn inside.
    pub vertical_width: i16,
    /// Top and bottom border widths. Negative widths are drawn inside.
    pub horizontal_width: i16,
    pub color: Option<CellColor>,
    pub kind: BoxStyleKind,
}

impl Default for BoxStyle {
    fn default() -> Self {
        BoxStyle {
            vertical_width: 1,
            horizontal_width: 1,
            color: None,
            kind: BoxStyleKind::Flat,
        }
    }
}

/// The merged face attributes for one screen cell.  `Option' fields
/// are tri-state during merging (None = unspecified, Some(false) =
/// explicitly cleared) and collapsed to plain bools once merged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellStyle {
    pub fg: Option<CellColor>,
    pub bg: Option<CellColor>,
    /// Index into `Grid::font_families` for an explicit face family.
    pub font_family: Option<u16>,
    /// Explicit font weight, in GNU's 100–900 scale.
    pub font_weight: Option<u16>,
    /// Explicit font width as a percentage of normal width.
    pub font_width: Option<u8>,
    /// Explicit font height in tenths of a point.
    pub font_height: Option<u16>,
    pub box_style: Option<BoxStyle>,
    /// Index into `Grid::stipples` for a bitmap background pattern.
    pub stipple: Option<u16>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub underline_color: Option<CellColor>,
    pub underline_wave: bool,
    pub overline: bool,
    pub strike: bool,
    pub strike_color: Option<CellColor>,
    /// `:inverse-video': swap foreground/background.  Front-ends invert
    /// themselves (so a colored inverse cell keeps its colors swapped
    /// rather than emitting SGR 7 on top of them).
    pub inverse: bool,
    /// The face's `:extend' flag: the face's background covers the
    /// empty cells past the end of the line (region-face semantics).
    pub extend: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridCursorShape {
    Block,
    Hollow,
    Bar,
    HBar,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridCursor {
    pub row: usize,
    pub column: usize,
    /// UTF-8 start offset in the rendered row.
    pub index: usize,
    /// UTF-8 end offset when the cursor should stretch over the next glyph.
    pub after_index: Option<usize>,
    pub shape: GridCursorShape,
    /// Pixel width for bar/hbar cursors and explicit cursor widths.
    pub thickness: usize,
    pub color: Option<CellColor>,
    pub selected: bool,
}

/// A 1-bit bitmap used as a face background stipple.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GridStipple {
    pub width: u16,
    pub height: u16,
    pub data: Arc<[u8]>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridImage {
    pub row: usize,
    pub column: usize,
    pub source: GridImageSource,
    pub width: Option<usize>,
    pub height: Option<usize>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GridImageSource {
    File(String),
    Data { format: String, bytes: Arc<[u8]> },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridMenu {
    pub label: String,
    pub items: Vec<GridMenuItem>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridMenuItem {
    pub label: String,
    pub command: Option<String>,
    pub items: Vec<GridMenuItem>,
    pub separator: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridToolBarItem {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub command: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridTabBarItem {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub command: String,
    pub argument: Option<i128>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridPopupMenuItem {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub command: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridHelpEcho {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub text: String,
}

fn push_help_echo(regions: &mut Vec<GridHelpEcho>, next: GridHelpEcho) {
    if let Some(previous) = regions.last_mut()
        && previous.row == next.row
        && previous.text == next.text
        && next.left <= previous.left.saturating_add(previous.width)
    {
        let end = previous
            .left
            .saturating_add(previous.width)
            .max(next.left.saturating_add(next.width));
        previous.width = end.saturating_sub(previous.left);
    } else {
        regions.push(next);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridScrollBarOrientation {
    Vertical,
    Horizontal,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridScrollBar {
    pub window_id: usize,
    pub buffer_id: usize,
    pub orientation: GridScrollBarOrientation,
    /// Frame-local pixel position and extent.
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub height: usize,
    /// Thumb position and extent along the track's scrolling axis.
    pub thumb_start: usize,
    pub thumb_size: usize,
    /// Maximum logical scroll offset represented by the track.
    pub max_offset: usize,
}

/// A bottom divider between a window and the screen area below it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridBottomDivider {
    /// Row immediately below the divider.
    pub row_boundary: usize,
    /// Horizontal position and extent in frame cells.
    pub left: usize,
    pub width: usize,
    /// Divider thickness in pixels.
    pub thickness: usize,
    pub style: CellStyle,
    pub first_pixel_style: CellStyle,
    pub last_pixel_style: CellStyle,
}

/// A window's mode-line hit area, with the displayed buffer-name extent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridModeLine {
    pub window_id: usize,
    pub buffer_id: usize,
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub buffer_name_range: Option<(usize, usize)>,
}

/// A window body whose contents are raised by a pixel scroll offset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridVScroll {
    pub window_id: usize,
    /// Window text area in frame cells.
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub rows: usize,
    /// Pixel amount to raise the contents.
    pub pixels: usize,
}

/// A display composition whose additional components are painted together.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridComposition {
    pub row: usize,
    pub column: usize,
    /// UTF-8 byte offset of the base component in `Grid::rows[row]`.
    pub byte_index: usize,
    pub components: Vec<char>,
    pub style: CellStyle,
}

/// Builder state for CellStyle while face specs merge — each field is
/// Option so an explicitly-nil attribute clears a lower layer.
#[derive(Clone, Copy, Default)]
struct StyleMix {
    fg: Option<CellColor>,
    bg: Option<CellColor>,
    font_family: Option<u16>,
    font_weight: Option<u16>,
    font_width: Option<u8>,
    font_height: Option<u16>,
    box_style: Option<Option<BoxStyle>>,
    stipple: Option<Option<u16>>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    underline_color: Option<Option<CellColor>>,
    underline_wave: Option<bool>,
    overline: Option<bool>,
    strike: Option<bool>,
    strike_color: Option<Option<CellColor>>,
    inverse: Option<bool>,
    extend: Option<bool>,
}

impl StyleMix {
    /// Later (higher-priority) layer wins wherever it specifies a value.
    fn merge(&mut self, other: &StyleMix) {
        macro_rules! m {
            ($($f:ident),*) => {
                $(if let Some(v) = other.$f { self.$f = Some(v); })*
            };
        }
        m!(
            fg,
            bg,
            font_family,
            font_weight,
            font_width,
            font_height,
            box_style,
            stipple,
            bold,
            italic,
            underline,
            underline_color,
            underline_wave,
            overline,
            strike,
            strike_color,
            inverse,
            extend
        );
    }

    fn finish(&self) -> CellStyle {
        CellStyle {
            fg: self.fg,
            bg: self.bg,
            font_family: self.font_family,
            font_weight: self.font_weight,
            font_width: self.font_width,
            font_height: self.font_height,
            box_style: self.box_style.flatten(),
            stipple: self.stipple.flatten(),
            bold: self.bold.unwrap_or(false),
            italic: self.italic.unwrap_or(false),
            underline: self.underline.unwrap_or(false),
            underline_color: self.underline_color.flatten(),
            underline_wave: self.underline_wave.unwrap_or(false),
            overline: self.overline.unwrap_or(false),
            strike: self.strike.unwrap_or(false),
            strike_color: self.strike_color.flatten(),
            inverse: self.inverse.unwrap_or(false),
            extend: self.extend.unwrap_or(false),
        }
    }
}

impl PartialEq for StyleMix {
    fn eq(&self, o: &Self) -> bool {
        self.fg == o.fg
            && self.bg == o.bg
            && self.font_family == o.font_family
            && self.font_weight == o.font_weight
            && self.font_width == o.font_width
            && self.font_height == o.font_height
            && self.box_style == o.box_style
            && self.stipple == o.stipple
            && self.bold == o.bold
            && self.italic == o.italic
            && self.underline == o.underline
            && self.underline_color == o.underline_color
            && self.underline_wave == o.underline_wave
            && self.overline == o.overline
            && self.strike == o.strike
            && self.strike_color == o.strike_color
            && self.inverse == o.inverse
            && self.extend == o.extend
    }
}

/// A text-mode screen snapshot produced by `render_grid`: one string
/// per row, face attributes by display cell and by character, mode-line
/// rows, and cursor positions for terminals and shaped GUI text.
#[derive(Clone, PartialEq)]
pub struct Grid {
    /// Native frame title computed from `frame-title-format`.
    pub title: String,
    /// Face family names referenced by styles in this snapshot.
    pub font_families: Vec<String>,
    /// The realized default face style for the current frame.
    pub default_style: CellStyle,
    pub rows: Vec<String>,
    /// Image runs from `display' properties, positioned in grid cells.
    pub images: Vec<GridImage>,
    /// Fill-column indicator glyphs overlaying otherwise blank text cells.
    pub fill_column_indicators: Vec<GridFillColumnIndicator>,
    /// Bitmap face backgrounds referenced by `CellStyle::stipple`.
    pub stipples: Vec<GridStipple>,
    pub menus: Vec<GridMenu>,
    pub tool_bar_items: Vec<GridToolBarItem>,
    pub tab_bar_items: Vec<GridTabBarItem>,
    pub help_echoes: Vec<GridHelpEcho>,
    pub tooltip: Option<String>,
    pub popup_menu_items: Vec<GridPopupMenuItem>,
    pub scroll_bars: Vec<GridScrollBar>,
    pub bottom_dividers: Vec<GridBottomDivider>,
    pub mode_lines: Vec<GridModeLine>,
    pub vscrolls: Vec<GridVScroll>,
    pub compositions: Vec<GridComposition>,
    /// Cell styles, one entry for each display column in a row.
    pub attrs: Vec<Vec<CellStyle>>,
    /// Character styles, `char_attrs[y][x]` alongside each row's chars.
    pub char_attrs: Vec<Vec<CellStyle>>,
    pub mode_rows: Vec<usize>,
    pub cursor: Option<(usize, usize)>,
    /// Per-window text cursors; terminal output uses `cursor` only.
    pub cursors: Vec<GridCursor>,
    /// UTF-8 byte offset of the cursor in the rendered row at `cursor.1`.
    pub cursor_index: Option<usize>,
    pub blink_cursor: bool,
    pub blink_interval: f64,
    /// Default face background used outside the visible text rows.
    pub default_bg: Option<CellColor>,
    /// Pixel height for each display row. Empty grids use the frontend default.
    pub row_heights: Vec<usize>,
    /// Base frame line height before line spacing and per-line overrides.
    pub default_line_height: usize,
    /// Default text row height used when the viewport needs more rows.
    pub default_row_height: usize,
    /// Pointer shape at the current mouse position, if it is over buffer text.
    pub hover_cursor: Option<GridPointerShape>,
    /// Whether the native mouse pointer should be visible over this frame.
    pub pointer_visible: bool,
    /// Unicode text and visible positions for the active OS text client.
    pub input: Option<GridInput>,
}

/// A buffer fill-column glyph positioned in a rendered grid row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridFillColumnIndicator {
    pub row: usize,
    pub column: usize,
    pub character: char,
    pub style: CellStyle,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GridInputSelection {
    pub range: Range<usize>,
    pub reversed: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GridInputRow {
    pub row: usize,
    /// Character offset in `GridInput::text` of the first source character.
    pub buffer_start: usize,
    /// Source character boundaries mapped to UTF-8 row byte offsets.
    pub source_offsets: Vec<usize>,
}

#[derive(Clone, PartialEq)]
pub struct GridInput {
    pub buffer_id: usize,
    pub text: Arc<str>,
    /// Buffer point as a UTF-16 offset into `text`.
    pub cursor: usize,
    /// Active region as a UTF-16 range into `text`.
    pub selection: Option<GridInputSelection>,
    pub rows: Vec<GridInputRow>,
    mod_tick: u64,
    char_offsets: Arc<[usize]>,
}

/// Cursor shapes that can be requested by text pointer properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridPointerShape {
    Text,
    Arrow,
    Hand,
    ResizeHorizontal,
    ResizeVertical,
}

impl Grid {
    /// An all-default grid of WIDTH × HEIGHT cells.
    pub fn blank(width: usize, height: usize) -> Grid {
        Grid {
            title: "Remacs".to_owned(),
            font_families: Vec::new(),
            default_style: CellStyle::default(),
            rows: vec![String::new(); height],
            images: Vec::new(),
            fill_column_indicators: Vec::new(),
            stipples: Vec::new(),
            menus: Vec::new(),
            tool_bar_items: Vec::new(),
            tab_bar_items: Vec::new(),
            help_echoes: Vec::new(),
            tooltip: None,
            popup_menu_items: Vec::new(),
            scroll_bars: Vec::new(),
            bottom_dividers: Vec::new(),
            mode_lines: Vec::new(),
            vscrolls: Vec::new(),
            compositions: Vec::new(),
            attrs: vec![vec![CellStyle::default(); width]; height],
            char_attrs: vec![Vec::new(); height],
            mode_rows: Vec::new(),
            cursor: None,
            cursors: Vec::new(),
            cursor_index: None,
            blink_cursor: false,
            blink_interval: 0.5,
            default_bg: None,
            row_heights: Vec::new(),
            default_line_height: 19,
            default_row_height: 19,
            hover_cursor: None,
            pointer_visible: true,
            input: None,
        }
    }
}

fn input_snapshot(i: &Interp, cached: Option<&GridInput>) -> Option<GridInput> {
    let buffer = i.current_buffer_ref()?;
    let bb = buffer.borrow();
    let mod_tick = bb.text.mod_tick.get();
    let (text, char_offsets) = match cached {
        Some(snapshot) if snapshot.buffer_id == bb.id && snapshot.mod_tick == mod_tick => {
            (snapshot.text.clone(), snapshot.char_offsets.clone())
        }
        _ => {
            let source = bb.text.text();
            let mut text = String::with_capacity(source.chars().count());
            let mut char_offsets = Vec::with_capacity(source.chars().count() + 1);
            let mut utf16_offset = 0usize;
            for ch in source.chars() {
                char_offsets.push(utf16_offset);
                let ch = ch.unicode().unwrap_or('\u{fffd}');
                text.push(ch);
                utf16_offset += ch.len_utf16();
            }
            char_offsets.push(utf16_offset);
            (Arc::<str>::from(text), Arc::<[usize]>::from(char_offsets))
        }
    };
    let point = bb.point().min(char_offsets.len().saturating_sub(1));
    let cursor = char_offsets.get(point).copied().unwrap_or(0);
    let mark_active = i
        .q_soft("mark-active")
        .and_then(|id| {
            i.buffer_local_toplevel_value(id, bb.id)
                .or_else(|| i.default_toplevel_value(id))
                .or_else(|| bb.locals.get(&id).cloned())
        })
        .is_some_and(|value| value.truthy());
    let selection = if mark_active {
        bb.mark.as_ref().and_then(|mark| {
            let mark = mark.borrow();
            if mark.buffer != Some(bb.id) {
                return None;
            }
            let mark_position = mark.position.min(char_offsets.len().saturating_sub(1));
            (mark_position != point).then(|| {
                let mark_offset = char_offsets.get(mark_position).copied().unwrap_or(0);
                GridInputSelection {
                    range: mark_offset.min(cursor)..mark_offset.max(cursor),
                    reversed: cursor < mark_offset,
                }
            })
        })
    } else {
        None
    };
    Some(GridInput {
        buffer_id: bb.id,
        text,
        cursor,
        selection,
        rows: Vec::new(),
        mod_tick,
        char_offsets,
    })
}

fn display_char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(1)
}

fn display_columns(text: &EmacsText, tab_width: usize) -> usize {
    display_columns_of(text.chars(), tab_width)
}

fn display_columns_of(chars: impl Iterator<Item = EmacsChar>, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    let mut columns = 0usize;
    for ch in chars {
        if ch.byte8().is_some() {
            columns = columns.saturating_add(4);
            continue;
        }
        let ch = ch.unicode().unwrap_or('\u{fffd}');
        let width = if ch == '\t' {
            tab_width - columns % tab_width
        } else {
            display_char_width(ch)
        };
        columns = columns.saturating_add(width);
    }
    columns
}

fn wrap_ranges(
    text: &EmacsText,
    width: usize,
    tab_width: usize,
    ctl_arrow: bool,
) -> Vec<Range<usize>> {
    let chars: Vec<_> = text.chars().collect();
    if chars.is_empty() {
        return vec![0..0];
    }
    if width == 0 {
        return vec![0..chars.len()];
    }

    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut column = 0usize;
    let tab_width = tab_width.max(1);
    for (index, character) in chars.iter().enumerate() {
        let byte8 = character.byte8().is_some();
        let code = character.code();
        let character = character.unicode().unwrap_or('\u{fffd}');
        let control_character = code < 0x20 || code == 0x7f;
        let mut character_width = if character == '\t' {
            tab_width - column % tab_width
        } else if byte8 {
            4
        } else if ctl_arrow && control_character {
            2
        } else {
            display_char_width(character)
        };
        if column > 0 && column.saturating_add(character_width) > width {
            ranges.push(start..index);
            start = index;
            column = 0;
            if character == '\t' {
                character_width = tab_width;
            }
        }
        column = column.saturating_add(character_width);
    }
    ranges.push(start..chars.len());
    ranges
}

fn wrap_ranges_with_prefix(
    text: &EmacsText,
    width: usize,
    first_prefix_width: usize,
    tab_width: usize,
    ctl_arrow: bool,
) -> Vec<Range<usize>> {
    if first_prefix_width == 0 || width == 0 {
        return wrap_ranges(text, width, tab_width, ctl_arrow);
    }
    let chars: Vec<_> = text.chars().collect();
    if chars.is_empty() {
        return vec![0..0];
    }

    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut column = first_prefix_width;
    let tab_width = tab_width.max(1);
    for (index, character) in chars.iter().enumerate() {
        let byte8 = character.byte8().is_some();
        let code = character.code();
        let character = character.unicode().unwrap_or('\u{fffd}');
        let control_character = code < 0x20 || code == 0x7f;
        let mut character_width = if character == '\t' {
            tab_width - column % tab_width
        } else if byte8 {
            4
        } else if ctl_arrow && control_character {
            2
        } else {
            display_char_width(character)
        };
        if column > 0 && column.saturating_add(character_width) > width {
            ranges.push(start..index);
            start = index;
            column = 0;
            if character == '\t' {
                character_width = tab_width;
            }
        }
        column = column.saturating_add(character_width);
    }
    ranges.push(start..chars.len());
    ranges
}

fn selective_display_hides_line(mode: &Value, text: &EmacsText, tab_width: usize) -> bool {
    let Value::Int(threshold) = mode else {
        return false;
    };
    if *threshold <= 0 {
        return false;
    }
    let tab_width = tab_width.max(1);
    let mut indentation = 0usize;
    for character in text.chars() {
        match character.unicode().unwrap_or('\u{fffd}') {
            ' ' => indentation = indentation.saturating_add(1),
            '\t' => indentation = indentation.saturating_add(tab_width - indentation % tab_width),
            _ => break,
        }
    }
    indentation >= usize::try_from(*threshold).unwrap_or(usize::MAX)
}

fn buffer_tab_width(i: &Interp, buffer: usize) -> usize {
    let id = i.q_soft("tab-width").unwrap_or(u32::MAX);
    i.buffer_local_toplevel_value(id, buffer)
        .or_else(|| i.default_toplevel_value(id))
        .and_then(|value| match value {
            Value::Int(width) if width > 0 => usize::try_from(width).ok(),
            _ => None,
        })
        .filter(|width| *width > 0)
        .unwrap_or(8)
}

fn buffer_setting(i: &Interp, name: &str, buffer: usize) -> Value {
    let Some(id) = i.q_soft(name) else {
        return Value::Nil;
    };
    i.buffer_local_toplevel_value(id, buffer)
        .or_else(|| i.default_toplevel_value(id))
        .unwrap_or(Value::Nil)
}

fn display_row_index_from(
    text: &crate::buffer::SharedText,
    begv: usize,
    zv: usize,
    from: usize,
    to: usize,
    wrap_lines: bool,
    width: usize,
    tab_width: usize,
    ctl_arrow: bool,
    max_rows: usize,
) -> Option<usize> {
    if to < from {
        return None;
    }
    let from_line = text.line_of_pos(from);
    let to_line = text.line_of_pos(to);
    if !wrap_lines || width == 0 {
        return Some(to_line.saturating_sub(from_line).min(max_rows));
    }

    let mut row = 0usize;
    for line_number in from_line..=to_line {
        let line_start = text.line_start(line_number).max(begv);
        if line_start > zv {
            break;
        }
        let line_end = text.line_end(line_start).min(zv);
        let line = text.substring(line_start, line_end);
        let line_len = line.chars().count();
        let from_offset = if line_number == from_line {
            from.saturating_sub(line_start)
        } else {
            0
        };
        let to_offset = to.saturating_sub(line_start);
        for range in wrap_ranges(&line, width, tab_width, ctl_arrow) {
            if line_number == from_line && range.end <= from_offset && range.end < line_len {
                continue;
            }
            if line_number == to_line
                && to_offset >= range.start
                && (to_offset < range.end || (to_offset == range.end && range.end == line_len))
            {
                return Some(row);
            }
            row = row.saturating_add(1);
            if row >= max_rows {
                return Some(max_rows);
            }
        }
    }
    Some(row.saturating_sub(1))
}

fn display_row_distance_from_start(
    text: &crate::buffer::SharedText,
    begv: usize,
    zv: usize,
    start: usize,
    point: usize,
    wrap_lines: bool,
    width: usize,
    tab_width: usize,
    ctl_arrow: bool,
    max_rows: usize,
) -> isize {
    let (from, to, sign) = if point >= start {
        (start, point, 1isize)
    } else {
        (point, start, -1isize)
    };
    let rows = display_row_index_from(
        text,
        begv,
        zv,
        from,
        to,
        wrap_lines,
        width,
        tab_width,
        ctl_arrow,
        max_rows,
    )
    .unwrap_or(0)
    .min(isize::MAX as usize) as isize;
    rows.saturating_mul(sign)
}

fn display_row_start_before_point(
    text: &crate::buffer::SharedText,
    begv: usize,
    zv: usize,
    point: usize,
    rows_before: usize,
    wrap_lines: bool,
    width: usize,
    tab_width: usize,
    ctl_arrow: bool,
) -> usize {
    if !wrap_lines || width == 0 {
        let point_line = text.line_of_pos(point);
        return text.line_start(point_line.saturating_sub(rows_before)).max(begv);
    }

    let mut line_number = text.line_of_pos(point);
    let mut remaining = rows_before;
    loop {
        let line_start = text.line_start(line_number).max(begv);
        let line_end = text.line_end(line_start).min(zv);
        let line = text.substring(line_start, line_end);
        let line_len = line.chars().count();
        let point_offset = point.saturating_sub(line_start);
        let ranges = wrap_ranges(&line, width, tab_width, ctl_arrow);
        let point_range = ranges
            .iter()
            .position(|range| {
                point_offset >= range.start
                    && (point_offset < range.end
                        || (point_offset == range.end && range.end == line_len))
            })
            .or_else(|| ranges.iter().rposition(|range| range.start <= point_offset))
            .unwrap_or(0);
        for range in ranges[..=point_range].iter().rev() {
            if remaining == 0 {
                return line_start.saturating_add(range.start).max(begv);
            }
            remaining -= 1;
        }
        if line_start <= begv || line_number == 0 {
            return begv;
        }
        line_number -= 1;
    }
}

fn effective_scroll_margin(i: &Interp, buffer: usize, text_rows: usize) -> usize {
    let margin = buffer_setting(i, "scroll-margin", buffer)
        .int()
        .unwrap_or(0)
        .max(0) as usize;
    if margin == 0 || text_rows == 0 {
        return 0;
    }
    let ratio = match buffer_setting(i, "maximum-scroll-margin", buffer) {
        Value::Float(ratio) if ratio.is_finite() => (*ratio).clamp(0.0, 0.5),
        _ => 0.25,
    };
    margin
        .min((text_rows.saturating_sub(1)) / 2)
        .min((text_rows as f64 * ratio) as usize)
}

fn automatic_scroll_target_row(
    point_row: isize,
    scrolling_down: bool,
    text_rows: usize,
    margin: usize,
    scroll_conservatively: usize,
    scroll_step: usize,
    aggressive: Option<f64>,
) -> usize {
    if text_rows == 0 {
        return 0;
    }
    let bottom = text_rows.saturating_sub(1).saturating_sub(margin);
    let center = (text_rows / 2).clamp(margin.min(bottom), bottom);
    if scrolling_down {
        if scroll_conservatively > 100 {
            return bottom;
        }
        if scroll_conservatively > 0 {
            let threshold = bottom as isize;
            let margin_extra = if margin > 0 { 1 } else { 0 };
            let amount = point_row
                .saturating_sub(threshold)
                .saturating_add(margin_extra)
                .max(1) as usize;
            return if amount <= scroll_conservatively {
                point_row.saturating_sub(amount.min(isize::MAX as usize) as isize)
                    .clamp(0, bottom as isize) as usize
            } else {
                center
            };
        }
        if scroll_step > 0 {
            let target = point_row.saturating_sub(scroll_step.min(isize::MAX as usize) as isize);
            return if (margin.min(bottom) as isize..=bottom as isize).contains(&target) {
                target as usize
            } else {
                center
            };
        }
        if let Some(fraction) = aggressive {
            let offset = (fraction.clamp(0.0, 1.0) * text_rows as f64) as usize;
            return bottom.saturating_sub(offset).max(margin.min(bottom));
        }
    } else {
        if scroll_conservatively > 100 {
            return margin.min(bottom);
        }
        if scroll_conservatively > 0 {
            let amount = (margin as isize).saturating_sub(point_row).max(1) as usize;
            return if amount <= scroll_conservatively {
                point_row.saturating_add(amount.min(isize::MAX as usize) as isize)
                    .clamp(margin.min(bottom) as isize, bottom as isize)
                    as usize
            } else {
                center
            };
        }
        if scroll_step > 0 {
            let target = point_row.saturating_add(scroll_step.min(isize::MAX as usize) as isize);
            return if (margin.min(bottom) as isize..=bottom as isize).contains(&target) {
                target as usize
            } else {
                center
            };
        }
        if let Some(fraction) = aggressive {
            let offset = (fraction.clamp(0.0, 1.0) * text_rows as f64) as usize;
            return margin.saturating_add(offset).clamp(margin.min(bottom), bottom);
        }
    }
    center
}

/// Horizontal scroll distance: absolute columns, a window-width fraction, or
/// GNU's default recentering behavior.
#[derive(Clone, Copy)]
enum HScrollStep {
    Center,
    Columns(usize),
    Fraction(f64),
}

fn hscroll_step_setting(i: &Interp, buffer: usize) -> HScrollStep {
    match buffer_setting(i, "hscroll-step", buffer) {
        Value::Int(step) if (1..=i32::MAX as i128).contains(&step) => {
            HScrollStep::Columns(step as usize)
        }
        Value::Float(step) if step.is_finite() && *step > 0.0 => {
            HScrollStep::Fraction((*step).clamp(0.0, 1.0))
        }
        _ => HScrollStep::Center,
    }
}

fn automatic_hscroll_position(
    column: usize,
    line_columns: usize,
    point_at_end_of_line: bool,
    hscroll: usize,
    content_width: usize,
    margin: usize,
    step: HScrollStep,
    min_hscroll: usize,
) -> Option<usize> {
    // C's hscroll_window_tree scrolls only when point reaches a margin
    // while text remains in that direction, or when point is off-screen.
    if content_width == 0 {
        return None;
    }
    let before_window = column < hscroll;
    let cursor_x = column.saturating_sub(hscroll);
    let right_boundary = hscroll.saturating_add(content_width);
    let right_margin_start = content_width.saturating_sub(margin);
    let scrolling_right = !before_window
        && (column >= right_boundary
            || (column < line_columns && cursor_x >= right_margin_start));
    let scrolling_left = before_window || (hscroll > 0 && cursor_x <= margin);
    if !scrolling_right && !scrolling_left {
        return None;
    }

    let target_x = match (step, scrolling_right) {
        (HScrollStep::Center, true) if point_at_end_of_line => content_width.saturating_sub(4),
        (HScrollStep::Center, _) => content_width / 2,
        (HScrollStep::Columns(columns), true) => {
            content_width.saturating_sub(columns.saturating_add(margin))
        }
        (HScrollStep::Columns(columns), false) => margin.saturating_add(columns),
        (HScrollStep::Fraction(fraction), true) => {
            ((content_width as f64 * (1.0 - fraction)) as usize).saturating_sub(margin)
        }
        (HScrollStep::Fraction(fraction), false) => {
            ((content_width as f64 * fraction) as usize).saturating_add(margin)
        }
    };
    Some(column.saturating_sub(target_x).max(min_hscroll))
}

fn buffer_fill_column_indicator(
    i: &Interp,
    ctx: &mut FaceCtx,
    faces: Option<&WindowFaces>,
    buffer: usize,
) -> Option<(usize, char, CellStyle)> {
    if !buffer_setting(i, "display-fill-column-indicator", buffer).truthy() {
        return None;
    }
    let Value::Int(character) = buffer_setting(i, "display-fill-column-indicator-character", buffer)
    else {
        return None;
    };
    let character = u32::try_from(character)
        .ok()
        .and_then(EmacsChar::new)
        .and_then(EmacsChar::unicode)?;
    let column = match buffer_setting(i, "display-fill-column-indicator-column", buffer) {
        Value::Sym(symbol) if i.symbol_name(symbol) == "t" => {
            buffer_setting(i, "fill-column", buffer).int()?
        }
        Value::Int(column) => column,
        _ => return None,
    };
    let column = usize::try_from(column).ok()?;
    let style = faces
        .map(|faces| remapped_named_face(i, ctx, faces, "fill-column-indicator", 0).finish())
        .unwrap_or_else(|| ctx.builtin(i, "fill-column-indicator", 0).finish());
    Some((column, character, style))
}

fn line_prefix_at_pos(
    i: &Interp,
    buffer_id: usize,
    window: &Value,
    position: usize,
) -> Option<EmacsText> {
    let property = i.q_soft("line-prefix");
    let (overlay_value, text_value) = if let (Some(property), Some(buffer)) =
        (property, i.buffers.get(buffer_id))
    {
        let buffer = buffer.borrow();
        let overlays: Vec<_> = buffer
            .overlays
            .iter()
            .enumerate()
            .filter(|(_, overlay)| {
                overlay.buffer == Some(buffer_id)
                    && position >= overlay.start
                    && position < overlay.end
                    && overlay.matches_window(i, window)
            })
            .filter_map(|(slot, overlay)| {
                let value = overlay.property(i, &Value::Sym(property));
                (!value.is_nil()).then_some((overlay, slot, value))
            })
            .collect();
        let mut order: Vec<_> = overlays
            .iter()
            .enumerate()
            .map(|(candidate, (overlay, _, _))| (*overlay, candidate))
            .collect();
        crate::buffer::sort_overlays_by_priority(i, &mut order);
        let overlay_value = order
            .last()
            .map(|(_, candidate)| overlays[*candidate].2.clone());
        let text_value = buffer
            .text
            .props
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, prop)| {
                prop.prop == property && position >= prop.start && position < prop.end
            })
            .max_by_key(|(index, _)| *index)
            .map(|(_, prop)| prop.value.clone());
        (overlay_value, text_value)
    } else {
        (None, None)
    };
    let value = overlay_value
        .or(text_value)
        .unwrap_or_else(|| buffer_setting(i, "line-prefix", buffer_id));
    match value {
        Value::Str(string) => Some(string.borrow().chars().collect()),
        _ => None,
    }
}

fn symbol_is(i: &Interp, value: &Value, name: &str) -> bool {
    i.q_soft(name)
        .is_some_and(|id| matches!(value, Value::Sym(symbol) if *symbol == id))
}

fn parse_cursor_type(i: &Interp, value: &Value) -> Option<(GridCursorShape, usize)> {
    let (name, explicit_size) = match value {
        Value::Sym(symbol) => (i.symbol_name(*symbol), None),
        Value::Cons(cons) => {
            let cons = cons.borrow();
            let Value::Sym(symbol) = &cons.car else {
                return None;
            };
            let size = cons.cdr.int().and_then(|size| usize::try_from(size).ok());
            (i.symbol_name(*symbol), size)
        }
        _ => return None,
    };
    let (shape, default_size) = match name.as_str() {
        "t" | "box" => (GridCursorShape::Block, 0),
        "hollow" => (GridCursorShape::Hollow, 0),
        "bar" => (GridCursorShape::Bar, 2),
        "hbar" => (GridCursorShape::HBar, 2),
        _ => return None,
    };
    let thickness = explicit_size.unwrap_or(default_size);
    if explicit_size.is_some() && thickness == 0 {
        None
    } else {
        Some((shape, thickness))
    }
}

fn cursor_type_for_window(
    i: &Interp,
    window: &std::rc::Rc<std::cell::RefCell<crate::editor::Window>>,
    buffer: usize,
    selected: bool,
) -> Option<(GridCursorShape, usize)> {
    let buffer_type = buffer_setting(i, "cursor-type", buffer);
    let window_type = window.borrow().cursor_type.clone();
    let effective_type = if symbol_is(i, &window_type, "t") {
        buffer_type
    } else {
        window_type
    };
    if selected {
        return parse_cursor_type(i, &effective_type);
    }
    let nonselected = buffer_setting(i, "cursor-in-non-selected-windows", buffer);
    if nonselected.is_nil() || effective_type.is_nil() {
        return None;
    }
    if symbol_is(i, &nonselected, "t") {
        Some((GridCursorShape::Hollow, 0))
    } else {
        parse_cursor_type(i, &nonselected)
    }
}

fn frame_cursor_color(
    i: &Interp,
    face_ctx: &mut FaceCtx,
    frame: &crate::editor::Frame,
) -> Option<CellColor> {
    // A Lisp change to the cursor face is the graphical cursor's highest
    // priority color.  Keep frame-parameter updates authoritative until that
    // face has been explicitly changed.
    if let (Some(cursor), Some(override_property)) = (
        i.q_soft("cursor"),
        i.q_soft("remacs--cursor-face-background-override"),
    ) {
        if i.get_prop(cursor, override_property).truthy() {
            if let Some(color) = face_ctx.named(i, &cursor, 0).bg {
                return Some(color);
            }
        }
    }

    let value = i
        .q_soft("cursor-color")
        .map(|id| crate::lisp::eval::plist_get_v(&frame.params, &Value::Sym(id)))
        .filter(|value| !value.is_nil())
        .unwrap_or_else(|| Value::string("white"));
    face_ctx.color(&value)
}

fn blink_cursor_settings(i: &Interp) -> (bool, f64) {
    let enabled = i
        .q_soft("blink-cursor-mode")
        .is_some_and(|id| i.symbol_value(id).truthy());
    let interval = i
        .q_soft("blink-cursor-interval")
        .and_then(|id| match i.symbol_value(id) {
            Value::Int(value) => Some(value as f64),
            Value::Float(value) => Some(*value),
            _ => None,
        });
    (
        enabled,
        interval.filter(|value| *value > 0.0).unwrap_or(0.5),
    )
}

struct FrameTitleContext<'a> {
    buffer_name: &'a str,
    file_name: Option<&'a str>,
    frame_name: &'a str,
    modified: bool,
}

fn format_frame_title_string(format: &str, context: &FrameTitleContext<'_>) -> String {
    let mut title = String::with_capacity(format.len());
    let mut chars = format.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            title.push(character);
            continue;
        }
        let Some(specifier) = chars.next() else {
            title.push('%');
            break;
        };
        match specifier {
            '%' => title.push('%'),
            'b' => title.push_str(context.buffer_name),
            'f' => title.push_str(context.file_name.unwrap_or(context.buffer_name)),
            'F' => title.push_str(context.frame_name),
            '*' | '+' => title.push(if context.modified { '*' } else { '-' }),
            '-' => title.push('-'),
            other => {
                title.push('%');
                title.push(other);
            }
        }
    }
    title
}

fn format_frame_title_value(
    i: &Interp,
    value: &Value,
    context: &FrameTitleContext<'_>,
    depth: usize,
) -> String {
    if depth >= 32 {
        return String::new();
    }
    match value {
        Value::Nil => String::new(),
        Value::Str(string) => {
            format_frame_title_string(&string.borrow().to_unicode_lossy(), context)
        }
        Value::Sym(symbol) => {
            if i.symbol_name(*symbol) == "t" {
                return context.buffer_name.to_owned();
            }
            let symbol_value = i.symbol_value(*symbol);
            if matches!(&symbol_value, Value::Sym(other) if other == symbol) {
                i.symbol_name(*symbol).to_string()
            } else {
                format_frame_title_value(i, &symbol_value, context, depth + 1)
            }
        }
        Value::Cons(_) => value
            .list_to_vec()
            .map(|elements| {
                elements
                    .iter()
                    .map(|element| format_frame_title_value(i, element, context, depth + 1))
                    .collect()
            })
            .unwrap_or_default(),
        Value::Int(number) => number.to_string(),
        _ => String::new(),
    }
}

fn frame_title(i: &Interp, frame: &crate::editor::Frame) -> String {
    let buffer_id = frame.selected.borrow().buffer;
    let Some(buffer) = i.buffers.get(buffer_id) else {
        return frame.name.borrow().to_unicode_lossy();
    };
    let (buffer_name, file_name, modified) = {
        let buffer = buffer.borrow();
        (
            buffer.name.to_unicode_lossy(),
            buffer
                .file_name
                .as_ref()
                .map(|name| name.to_unicode_lossy()),
            buffer.is_modified(),
        )
    };
    let frame_name = frame.name.borrow().to_unicode_lossy();
    let context = FrameTitleContext {
        buffer_name: &buffer_name,
        file_name: file_name.as_deref(),
        frame_name: &frame_name,
        modified,
    };
    let Some(format) = i
        .q_soft("frame-title-format")
        .map(|symbol| i.symbol_value(symbol))
    else {
        return buffer_name;
    };
    if matches!(format, Value::Nil) {
        return buffer_name;
    }
    format_frame_title_value(i, &format, &context, 0)
}

struct ModeLineContext {
    buffer_name: String,
    file_name: String,
    mode_name: String,
    modified: bool,
    read_only: bool,
    line: usize,
    column: usize,
    line_count: usize,
    buffer_size: usize,
    point_offset: usize,
    narrowed: bool,
}

fn mode_line_variable(i: &Interp, buffer_id: usize, name: &str) -> Option<Value> {
    let id = i.q_soft(name)?;
    i.buffer_local_toplevel_value(id, buffer_id)
        .or_else(|| i.default_toplevel_value(id))
}

fn mode_line_context(
    i: &Interp,
    buffer_id: usize,
    point: usize,
    tab_width: usize,
) -> ModeLineContext {
    let Some(buffer) = i.buffers.get(buffer_id) else {
        return ModeLineContext {
            buffer_name: "???".to_owned(),
            file_name: String::new(),
            mode_name: String::new(),
            modified: false,
            read_only: false,
            line: 0,
            column: 0,
            line_count: 0,
            buffer_size: 0,
            point_offset: 0,
            narrowed: false,
        };
    };
    let buffer = buffer.borrow();
    let point = point.clamp(buffer.begv, buffer.zv);
    let first_line = buffer.text.line_of_pos(buffer.begv);
    let point_line = buffer.text.line_of_pos(point);
    let last_line = buffer.text.line_of_pos(buffer.zv);
    let point_line_start = buffer.text.line_start(point_line);
    let mode_name = match buffer_setting(i, "mode-name", buffer_id) {
        Value::Str(name) => name.borrow().to_unicode_lossy(),
        Value::Sym(name) => i.symbol_name(name),
        value => i.princ_to_string(&value).to_unicode_lossy(),
    };
    ModeLineContext {
        buffer_name: buffer.name.to_unicode_lossy(),
        file_name: buffer
            .file_name
            .as_ref()
            .map(|name| name.to_unicode_lossy())
            .unwrap_or_default(),
        mode_name,
        modified: buffer.is_modified(),
        read_only: buffer_setting(i, "buffer-read-only", buffer_id).truthy(),
        line: point_line.saturating_sub(first_line).saturating_add(1),
        column: display_columns(
            &buffer.text.substring(point_line_start, point),
            tab_width,
        ),
        line_count: last_line.saturating_sub(first_line).saturating_add(1),
        buffer_size: buffer.zv.saturating_sub(buffer.begv),
        point_offset: point.saturating_sub(buffer.begv),
        narrowed: buffer.begv > 0 || buffer.zv < buffer.text_len(),
    }
}

fn frame_fringe_pixels(i: &Interp, parameters: &Value, name: &str) -> usize {
    i.q_soft(name)
        .map(|parameter| crate::lisp::eval::plist_get(parameters, parameter))
        .and_then(|value| match value {
            Value::Int(width) => usize::try_from(width.max(0)).ok(),
            _ => None,
        })
        .unwrap_or(8)
}

fn fringe_pixels_to_cells(pixels: usize, cell_width: usize) -> usize {
    let cell_width = cell_width.max(1);
    if pixels == 0 {
        0
    } else {
        pixels.saturating_add(cell_width - 1) / cell_width
    }
}

fn overlay_arrow_positions(i: &Interp, buffer_id: usize) -> Vec<(usize, String)> {
    fn arrow_marker(value: &Value, buffer_id: usize) -> Option<usize> {
        let Value::Marker(marker) = value else {
            return None;
        };
        let marker = marker.borrow();
        (marker.buffer == Some(buffer_id)).then_some(marker.position)
    }
    fn arrow_string(i: &Interp, value: Value, fallback: &str) -> String {
        match value {
            Value::Str(string) => string.borrow().to_unicode_lossy(),
            _ => fallback.to_owned(),
        }
    }

    let default_string = mode_line_variable(i, buffer_id, "overlay-arrow-string")
        .map(|value| arrow_string(i, value, ">"))
        .unwrap_or_else(|| ">".to_owned());
    let mut arrows = Vec::new();
    if let Some(position) = mode_line_variable(i, buffer_id, "overlay-arrow-position")
        .as_ref()
        .and_then(|marker| arrow_marker(marker, buffer_id))
    {
        arrows.push((position, default_string.clone()));
    }

    if let Some(mut variables) = i.q_soft("overlay-arrow-variable-list").map(|id| i.symbol_value(id))
    {
        let arrow_string_property = i.q_soft("overlay-arrow-string");
        while let Value::Cons(cell) = variables {
            let (variable, next) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            variables = next;
            let Value::Sym(variable_id) = variable else {
                continue;
            };
            let value = mode_line_variable(i, buffer_id, &i.symbol_name(variable_id))
                .unwrap_or(Value::Nil);
            let Some(position) = arrow_marker(&value, buffer_id) else {
                continue;
            };
            let specific = arrow_string_property
                .map(|property| i.get_prop(variable_id, property))
                .unwrap_or(Value::Nil);
            arrows.push((position, arrow_string(i, specific, &default_string)));
        }
    }
    arrows
}

fn fit_mode_line_field(field: &str, width: usize, left_align: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let characters: Vec<_> = field.chars().collect();
    if characters.len() > width {
        if width >= 3 {
            return format!("{}…", characters[..width - 1].iter().collect::<String>());
        }
        return characters[..width].iter().collect();
    }
    let padding = " ".repeat(width - characters.len());
    if left_align {
        format!("{field}{padding}")
    } else {
        format!("{padding}{field}")
    }
}

fn format_mode_line_string(value: &str, context: &ModeLineContext) -> String {
    let characters: Vec<_> = value.chars().collect();
    let mut output = String::new();
    let mut index = 0usize;
    while index < characters.len() {
        if characters[index] != '%' {
            output.push(characters[index]);
            index += 1;
            continue;
        }
        index += 1;
        if index >= characters.len() {
            output.push('%');
            break;
        }
        if characters[index] == '%' {
            output.push('%');
            index += 1;
            continue;
        }
        let left_align = characters[index] == '-';
        if left_align {
            index += 1;
        }
        let mut width = 0usize;
        let mut has_width = false;
        while index < characters.len() && characters[index].is_ascii_digit() {
            has_width = true;
            width = width
                .saturating_mul(10)
                .saturating_add(characters[index].to_digit(10).unwrap_or(0) as usize);
            index += 1;
        }
        let mut precision = None;
        if index < characters.len() && characters[index] == '.' {
            index += 1;
            let mut value = 0usize;
            while index < characters.len() && characters[index].is_ascii_digit() {
                value = value
                    .saturating_mul(10)
                    .saturating_add(characters[index].to_digit(10).unwrap_or(0) as usize);
                index += 1;
            }
            precision = Some(value);
        }
        if index >= characters.len() {
            output.push('%');
            break;
        }
        let specifier = characters[index];
        index += 1;
        let mut field = match specifier {
            'b' => context.buffer_name.clone(),
            'f' => context.file_name.clone(),
            'm' | 'M' => context.mode_name.clone(),
            'l' => context.line.to_string(),
            'c' => context.column.to_string(),
            'C' => context.column.saturating_add(1).to_string(),
            'L' => context.line_count.to_string(),
            'I' => context.buffer_size.to_string(),
            'p' | 'P' => {
                if context.buffer_size == 0 {
                    "All".to_owned()
                } else if context.point_offset == 0 {
                    "Top".to_owned()
                } else if context.point_offset >= context.buffer_size {
                    "Bot".to_owned()
                } else {
                    let percentage = context.point_offset.saturating_mul(100)
                        / context.buffer_size.max(1);
                    format!("{percentage:2}%")
                }
            }
            'n' => {
                if context.narrowed {
                    " Narrow".to_owned()
                } else {
                    String::new()
                }
            }
            '*' => {
                if context.read_only {
                    "%".to_owned()
                } else if context.modified {
                    "*".to_owned()
                } else {
                    "-".to_owned()
                }
            }
            '+' => {
                if context.modified {
                    "*"
                } else {
                    "-"
                }
                .to_owned()
            }
            '[' | ']' | 'e' | 's' => String::new(),
            other => other.to_string(),
        };
        if let Some(maximum) = precision {
            field = field.chars().take(maximum).collect();
        }
        if has_width {
            field = fit_mode_line_field(&field, width, left_align);
        }
        output.push_str(&field);
    }
    output
}

fn mode_line_help_echo(
    i: &Interp,
    buffer_id: usize,
    value: &Value,
    depth: usize,
) -> Option<String> {
    if depth > 24 {
        return None;
    }
    let help_echo = i.q_soft("help-echo");
    match value {
        Value::Str(string) => {
            let plist = Value::list(crate::buffer::primitives::str_plist_at(
                i.str_props(string),
                0,
            ));
            help_echo
                .map(|property| crate::lisp::eval::plist_get(&plist, property))
                .and_then(|value| match value {
                    Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
                    _ => None,
                })
        }
        Value::Sym(symbol) => mode_line_variable(i, buffer_id, &i.symbol_name(*symbol))
            .filter(|current| !matches!(current, Value::Sym(id) if id == symbol))
            .and_then(|current| mode_line_help_echo(i, buffer_id, &current, depth + 1)),
        Value::Cons(_) => {
            let elements = value.list_to_vec().ok()?;
            if matches!(elements.first(), Some(Value::Sym(symbol)) if i.symbol_name(*symbol) == ":propertize")
                && elements.len() >= 2
            {
                if let Some(property) = help_echo {
                    let plist = Value::list(elements.iter().skip(2).cloned().collect());
                    if let Value::Str(text) = crate::lisp::eval::plist_get(&plist, property) {
                        return Some(text.borrow().to_unicode_lossy());
                    }
                }
                return mode_line_help_echo(i, buffer_id, &elements[1], depth + 1);
            }
            elements
                .iter()
                .find_map(|element| mode_line_help_echo(i, buffer_id, element, depth + 1))
        }
        Value::Vec(vector) => vector
            .borrow()
            .iter()
            .find_map(|element| mode_line_help_echo(i, buffer_id, element, depth + 1)),
        _ => None,
    }
}

fn format_mode_line_minor_modes(
    i: &Interp,
    buffer_id: usize,
    context: &ModeLineContext,
    depth: usize,
) -> String {
    let Some(alist) = mode_line_variable(i, buffer_id, "minor-mode-alist") else {
        return String::new();
    };
    let Ok(entries) = alist.list_to_vec() else {
        return String::new();
    };
    let mut output = String::new();
    for entry in entries {
        let Ok(parts) = entry.list_to_vec() else {
            continue;
        };
        let Some(Value::Sym(mode)) = parts.first() else {
            continue;
        };
        let active = mode_line_variable(i, buffer_id, &i.symbol_name(*mode))
            .unwrap_or(Value::Nil)
            .truthy();
        if active {
            for value in parts.iter().skip(1) {
                output.push_str(&format_mode_line_value(
                    i,
                    buffer_id,
                    value,
                    context,
                    depth + 1,
                ));
            }
        }
    }
    output
}

fn format_mode_line_value(
    i: &Interp,
    buffer_id: usize,
    value: &Value,
    context: &ModeLineContext,
    depth: usize,
) -> String {
    if depth > 32 {
        return String::new();
    }
    match value {
        Value::Nil => String::new(),
        Value::Str(string) => {
            format_mode_line_string(&string.borrow().to_unicode_lossy(), context)
        }
        Value::Int(number) => number.to_string(),
        Value::Float(number) => number.to_string(),
        Value::Sym(symbol) => {
            if i.symbol_name(*symbol) == "mode-line-minor-modes" {
                return format_mode_line_minor_modes(i, buffer_id, context, depth + 1);
            }
            let Some(value) = mode_line_variable(i, buffer_id, &i.symbol_name(*symbol)) else {
                return String::new();
            };
            if matches!(value, Value::Sym(id) if id == *symbol) {
                return String::new();
            }
            format_mode_line_value(i, buffer_id, &value, context, depth + 1)
        }
        Value::Cons(_) => {
            let Ok(elements) = value.list_to_vec() else {
                return String::new();
            };
            if elements.is_empty() {
                return String::new();
            }
            if let Value::Sym(head) = &elements[0] {
                match i.symbol_name(*head).as_str() {
                    ":eval" => return String::new(),
                    ":propertize" => {
                        return elements.get(1).map_or_else(String::new, |body| {
                            format_mode_line_value(i, buffer_id, body, context, depth + 1)
                        });
                    }
                    _ => {}
                }
                if elements.len() > 1 {
                    let condition = mode_line_variable(i, buffer_id, &i.symbol_name(*head))
                        .unwrap_or(Value::Nil)
                        .truthy();
                    if condition {
                        if elements.len() == 2 {
                            return format_mode_line_value(
                                i,
                                buffer_id,
                                &elements[1],
                                context,
                                depth + 1,
                            );
                        }
                        return elements[1..]
                            .iter()
                            .map(|element| {
                                format_mode_line_value(
                                    i,
                                    buffer_id,
                                    element,
                                    context,
                                    depth + 1,
                                )
                            })
                            .collect();
                    }
                    if elements.len() >= 3 {
                        return format_mode_line_value(
                            i,
                            buffer_id,
                            elements.last().unwrap_or(&Value::Nil),
                            context,
                            depth + 1,
                        );
                    }
                    return String::new();
                }
            }
            let start = if matches!(elements[0], Value::Int(_)) {
                1
            } else {
                0
            };
            elements[start..]
                .iter()
                .map(|element| {
                    format_mode_line_value(i, buffer_id, element, context, depth + 1)
                })
                .collect()
        }
        Value::Vec(vector) => vector
            .borrow()
            .iter()
            .map(|element| format_mode_line_value(i, buffer_id, element, context, depth + 1))
            .collect(),
        _ => String::new(),
    }
}

fn menu_key_name(i: &Interp, key: &Value) -> String {
    match key {
        Value::Sym(symbol) => i.symbol_name(*symbol),
        Value::Int(code) => char::from_u32(*code as u32)
            .map(|character| character.to_string())
            .unwrap_or_else(|| code.to_string()),
        _ => i.princ_to_string(key).to_unicode_lossy(),
    }
}

fn menu_label_and_definition(
    i: &Interp,
    key: &Value,
    definition: &Value,
) -> (String, Value, bool) {
    let fallback = menu_key_name(i, key);
    if let Value::Cons(cell) = definition {
        let head = cell.borrow().car.clone();
        if let Value::Str(label) = &head {
            return (
                label.borrow().to_unicode_lossy(),
                cell.borrow().cdr.clone(),
                false,
            );
        }
        if matches!(head, Value::Sym(symbol) if i.symbol_name(symbol) == "menu-item") {
            if let Ok(items) = definition.list_to_vec() {
                let label = items
                    .get(1)
                    .and_then(|value| match value {
                        Value::Str(label) => Some(label.borrow().to_unicode_lossy()),
                        _ => None,
                    })
                    .unwrap_or_else(|| fallback.clone());
                let definition = items.get(2).cloned().unwrap_or(Value::Nil);
                let separator = label == "--" || label.contains("separator");
                return (label, definition, separator);
            }
        }
    }
    let separator = fallback.contains("separator")
        || matches!(definition, Value::Str(label) if label.borrow().to_unicode_lossy() == "--");
    if separator {
        ("--".to_owned(), Value::Nil, true)
    } else {
        (fallback, definition.clone(), false)
    }
}

fn menu_keymap_from_definition(i: &Interp, definition: &Value, depth: usize) -> Option<Value> {
    if depth > 16 {
        return None;
    }
    if crate::editor::is_keymap(i, definition) {
        return Some(definition.clone());
    }
    match definition {
        Value::Sym(symbol) => {
            let value = i.symbol_value(*symbol);
            if crate::editor::is_keymap(i, &value) {
                return Some(value);
            }
            let function = i.symbol_function(*symbol);
            crate::editor::is_keymap(i, &function).then_some(function)
        }
        Value::Cons(cell) => {
            let (head, tail) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            if matches!(head, Value::Str(_)) {
                return menu_keymap_from_definition(i, &tail, depth + 1);
            }
            if matches!(head, Value::Sym(symbol) if i.symbol_name(symbol) == "menu-item")
                && let Ok(items) = definition.list_to_vec()
            {
                return items
                    .get(2)
                    .and_then(|definition| menu_keymap_from_definition(i, definition, depth + 1));
            }
            None
        }
        _ => None,
    }
}

fn menu_items(i: &Interp, keymap: &Value, depth: usize) -> Vec<GridMenuItem> {
    if depth > 8 {
        return Vec::new();
    }
    crate::editor::keymap_own_bindings(i, keymap)
        .into_iter()
        .filter_map(|(key, definition)| {
            let (label, definition, separator) =
                menu_label_and_definition(i, &key, &definition);
            if separator {
                return Some(GridMenuItem {
                    label,
                    command: None,
                    items: Vec::new(),
                    separator: true,
                });
            }
            if definition.is_nil() {
                return None;
            }
            if let Some(submenu) = menu_keymap_from_definition(i, &definition, depth + 1) {
                let items = menu_items(i, &submenu, depth + 1);
                return (!items.is_empty()).then_some(GridMenuItem {
                    label,
                    command: None,
                    items,
                    separator: false,
                });
            }
            let Value::Sym(command) = definition else {
                return None;
            };
            i.fbound_p(command).then(|| GridMenuItem {
                label,
                command: Some(i.symbol_name(command)),
                items: Vec::new(),
                separator: false,
            })
        })
        .collect()
}

fn render_menu_bar(i: &Interp) -> Vec<GridMenu> {
    if !mode_line_variable(i, i.current_buffer, "menu-bar-mode")
        .unwrap_or(Value::Nil)
        .truthy()
    {
        return Vec::new();
    }
    let Some(global_map) = mode_line_variable(i, i.current_buffer, "global-map") else {
        return Vec::new();
    };
    let menu_bar_key = crate::editor::event_code_for("menu-bar");
    let Some((_, definition)) = crate::editor::keymap_own_bindings(i, &global_map)
        .into_iter()
        .find(|(key, _)| matches!(key, Value::Int(code) if *code == menu_bar_key))
    else {
        return Vec::new();
    };
    let Some(menu_bar_map) = menu_keymap_from_definition(i, &definition, 0) else {
        return Vec::new();
    };
    menu_items(i, &menu_bar_map, 0)
        .into_iter()
        .filter(|item| !item.separator && !item.items.is_empty())
        .map(|item| GridMenu {
            label: item.label,
            items: item.items,
        })
        .collect()
}

fn render_tool_bar(
    i: &Interp,
    width: usize,
    rows: usize,
    row_offset: usize,
    default_style: CellStyle,
    faces: &mut FaceCtx,
    screen: &mut [Vec<ScreenCell>],
) -> Vec<GridToolBarItem> {
    let mut background = faces.builtin(i, "tool-bar", 0).finish();
    if background.fg.is_none() {
        background.fg = default_style.fg;
    }
    if background.bg.is_none() {
        background.bg = default_style.bg.or(Some(CellColor(0x33, 0x36, 0x3a)));
    }
    for row in screen.iter_mut().skip(row_offset).take(rows) {
        for cell in row {
            *cell = ScreenCell {
                chars: Vec::new(),
                style: background,
                occupied: true,
                continuation: false,
            };
        }
    }
    let Some(map_id) = i.q_soft("tool-bar-map") else {
        return Vec::new();
    };
    let map = i
        .buffer_local_toplevel_value(map_id, i.current_buffer)
        .or_else(|| i.default_toplevel_value(map_id))
        .unwrap_or(Value::Nil);
    let mut items = Vec::new();
    let mut row = 0usize;
    let mut column = 0usize;
    for (key, definition) in crate::editor::keymap_own_bindings(i, &map) {
        let (mut label, command, separator) = menu_label_and_definition(i, &key, &definition);
        if separator {
            column = column.saturating_add(1);
            continue;
        }
        let Value::Sym(command) = command else {
            continue;
        };
        if !i.fbound_p(command) {
            continue;
        }
        if let Ok(parts) = definition.list_to_vec()
            && matches!(parts.first(), Some(Value::Sym(id)) if i.symbol_name(*id) == "menu-item")
        {
            let properties = Value::list(parts.into_iter().skip(3).collect());
            if let Some(label_symbol) = i.q_soft(":label")
                && let Value::Str(value) = crate::lisp::eval::plist_get(&properties, label_symbol)
            {
                label = value.borrow().to_unicode_lossy();
            }
        }
        let label_width = label
            .chars()
            .map(display_char_width)
            .sum::<usize>()
            .max(1);
        let button_width = label_width.saturating_add(2).min(width.max(1));
        if column.saturating_add(button_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        if row >= rows || width == 0 {
            break;
        }
        let text = fit_text_to_cells_with_tab_width(
            &format!(" {label} "),
            button_width,
            8,
        )
        .0;
        let mut button_style = background;
        button_style.box_style = Some(BoxStyle {
            vertical_width: 1,
            horizontal_width: 1,
            color: background.fg,
            kind: BoxStyleKind::Released,
        });
        let cell_styles = vec![button_style; button_width];
        let character_styles = text.chars().map(|_| button_style).collect::<Vec<_>>();
        let screen_row_index = row_offset.saturating_add(row);
        if let Some(screen_row) = screen.get_mut(screen_row_index) {
            place_text_row(
                screen_row,
                column,
                &text,
                &cell_styles,
                &character_styles,
            );
        }
        items.push(GridToolBarItem {
            row: screen_row_index,
            left: column,
            width: button_width,
            command: i.symbol_name(command),
        });
        column = column.saturating_add(button_width).saturating_add(1);
    }
    items
}

fn tab_entry_parameter(i: &Interp, entry: &Value, name: &str) -> Value {
    let Some(property) = i.q_soft(name) else {
        return Value::Nil;
    };
    let Ok(items) = entry.list_to_vec() else {
        return Value::Nil;
    };
    items
        .into_iter()
        .find_map(|item| {
            let Value::Cons(cell) = item else {
                return None;
            };
            let cell = cell.borrow();
            matches!(cell.car, Value::Sym(symbol) if symbol == property)
                .then(|| cell.cdr.clone())
        })
        .unwrap_or(Value::Nil)
}

fn render_tab_bar(
    i: &Interp,
    frame_params: &Value,
    width: usize,
    rows: usize,
    row_offset: usize,
    default_style: CellStyle,
    faces: &mut FaceCtx,
    screen: &mut [Vec<ScreenCell>],
) -> Vec<GridTabBarItem> {
    let mut background = faces.builtin(i, "tab-bar", 0).finish();
    if background.fg.is_none() {
        background.fg = default_style.fg;
    }
    if background.bg.is_none() {
        background.bg = default_style.bg.or(Some(CellColor(0x2d, 0x30, 0x33)));
    }
    for row in screen.iter_mut().skip(row_offset).take(rows) {
        for cell in row {
            *cell = ScreenCell {
                chars: Vec::new(),
                style: background,
                occupied: true,
                continuation: false,
            };
        }
    }

    let tabs = i
        .q_soft("tabs")
        .map(|property| crate::lisp::eval::plist_get(frame_params, property))
        .and_then(|value| value.list_to_vec().ok())
        .unwrap_or_default();
    let tab_count = tabs.len().max(1);
    let close_setting = mode_line_variable(i, i.current_buffer, "tab-bar-close-button-show")
        .unwrap_or(Value::Nil);
    let close_button = mode_line_variable(i, i.current_buffer, "tab-bar-close-button")
        .and_then(|value| match value {
            Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
            _ => None,
        })
        .unwrap_or_else(|| " ×".to_owned());
    let mut items = Vec::new();
    let mut row = 0usize;
    let mut column = 0usize;

    for index in 0..tab_count {
        let tab = tabs.get(index);
        let current = tab.is_none_or(|tab| {
            matches!(tab, Value::Cons(cell)
                if matches!(cell.borrow().car, Value::Sym(symbol) if i.symbol_name(symbol) == "current-tab"))
        });
        let name = tab
            .map(|tab| tab_entry_parameter(i, tab, "name"))
            .unwrap_or_else(|| {
                i.buffers
                    .get(i.current_buffer)
                    .map(|buffer| Value::string(buffer.borrow().name.clone()))
                    .unwrap_or(Value::Nil)
            });
        let name = match name {
            Value::Str(name) => name.borrow().to_unicode_lossy(),
            _ => format!("Tab {}", index.saturating_add(1)),
        };
        let close_visible = match &close_setting {
            Value::Nil => false,
            Value::Sym(symbol) if i.symbol_name(*symbol) == "selected" => current,
            Value::Sym(symbol) if i.symbol_name(*symbol) == "non-selected" => !current,
            _ => close_setting.truthy(),
        };
        let label_width = name
            .chars()
            .map(display_char_width)
            .sum::<usize>()
            .saturating_add(2)
            .max(3);
        let close_width = if close_visible {
            close_button.chars().map(display_char_width).sum::<usize>()
        } else {
            0
        };
        let button_width = label_width
            .saturating_add(close_width)
            .min(width.max(1));
        if column.saturating_add(button_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        if row >= rows || width == 0 {
            break;
        }
        let mut style = faces
            .builtin(i, if current { "tab-bar-tab" } else { "tab-bar-tab-inactive" }, 0)
            .finish();
        if style.fg.is_none() {
            style.fg = background.fg;
        }
        if style.bg.is_none() {
            style.bg = background.bg;
        }
        style.box_style = Some(BoxStyle {
            vertical_width: 1,
            horizontal_width: 1,
            color: style.fg,
            kind: BoxStyleKind::Released,
        });
        let label = if close_visible {
            format!(" {name}{close_button} ")
        } else {
            format!(" {name} ")
        };
        let label = fit_text_to_cells_with_tab_width(&label, button_width, 8).0;
        let cell_styles = vec![style; button_width];
        let character_styles = label.chars().map(|_| style).collect::<Vec<_>>();
        let screen_row = row_offset.saturating_add(row);
        if let Some(screen_row) = screen.get_mut(screen_row) {
            place_text_row(screen_row, column, &label, &cell_styles, &character_styles);
        }
        items.push(GridTabBarItem {
            row: screen_row,
            left: column,
            width: label_width.min(button_width),
            command: "tab-bar-select-tab".to_owned(),
            argument: Some(index.saturating_add(1) as i128),
        });
        if close_visible && close_width > 0 {
            items.push(GridTabBarItem {
                row: screen_row,
                left: column.saturating_add(label_width.min(button_width)),
                width: close_width.min(button_width.saturating_sub(label_width.min(button_width))),
                command: "tab-bar-close-tab".to_owned(),
                argument: Some(index.saturating_add(1) as i128),
            });
        }
        column = column.saturating_add(button_width).saturating_add(1);
    }

    let new_button_show = mode_line_variable(i, i.current_buffer, "tab-bar-new-button-show")
        .unwrap_or(Value::t())
        .truthy();
    if new_button_show && width > 0 {
        let label = mode_line_variable(i, i.current_buffer, "tab-bar-new-button")
            .and_then(|value| match value {
                Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
                _ => None,
            })
            .unwrap_or_else(|| " + ".to_owned());
        let button_width = label
            .chars()
            .map(display_char_width)
            .sum::<usize>()
            .max(1)
            .min(width);
        if column.saturating_add(button_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        if row < rows {
            let mut style = faces.builtin(i, "tab-bar-tab-inactive", 0).finish();
            if style.fg.is_none() {
                style.fg = background.fg;
            }
            if style.bg.is_none() {
                style.bg = background.bg;
            }
            style.box_style = Some(BoxStyle {
                vertical_width: 1,
                horizontal_width: 1,
                color: style.fg,
                kind: BoxStyleKind::Released,
            });
            let label = fit_text_to_cells_with_tab_width(&label, button_width, 8).0;
            let cell_styles = vec![style; button_width];
            let character_styles = label.chars().map(|_| style).collect::<Vec<_>>();
            let screen_row = row_offset.saturating_add(row);
            if let Some(screen_row) = screen.get_mut(screen_row) {
                place_text_row(screen_row, column, &label, &cell_styles, &character_styles);
            }
            items.push(GridTabBarItem {
                row: screen_row,
                left: column,
                width: button_width,
                command: "tab-bar-new-tab".to_owned(),
                argument: None,
            });
        }
    }
    items
}

fn flatten_popup_items(
    items: &[GridMenuItem],
    depth: usize,
    flat: &mut Vec<(String, Option<String>, bool, usize)>,
) {
    for item in items {
        if item.separator {
            flat.push((String::new(), None, true, depth));
        } else if item.items.is_empty() {
            flat.push((item.label.clone(), item.command.clone(), false, depth));
        } else {
            flat.push((item.label.clone(), None, false, depth));
            flatten_popup_items(&item.items, depth.saturating_add(1), flat);
        }
    }
}

fn render_popup_menu(
    i: &Interp,
    menu: &Value,
    pixel_position: (i128, i128),
    width: usize,
    height: usize,
    cell_width: usize,
    cell_height: usize,
    default_style: CellStyle,
    faces: &mut FaceCtx,
    screen: &mut [Vec<ScreenCell>],
) -> Vec<GridPopupMenuItem> {
    let menu_items = if crate::editor::is_keymap(i, menu) {
        menu_items(i, menu, 0)
    } else if let Value::Cons(cell) = menu {
        let first = cell.borrow().car.clone();
        if crate::editor::is_keymap(i, &first) {
            menu_items(i, &first, 0)
        } else {
            return Vec::new();
        }
    } else {
        return Vec::new();
    };
    let mut flat_items = Vec::new();
    flatten_popup_items(&menu_items, 0, &mut flat_items);
    if flat_items.is_empty() || width == 0 || height == 0 {
        return Vec::new();
    }
    let max_label_width = flat_items
        .iter()
        .map(|(label, _, separator, depth)| {
            if *separator {
                1
            } else {
                label
                    .chars()
                    .map(display_char_width)
                    .sum::<usize>()
                    .saturating_add(depth.saturating_mul(2))
            }
        })
        .max()
        .unwrap_or(1);
    let menu_width = max_label_width.saturating_add(2).max(3).min(width);
    let menu_height = flat_items.len().saturating_add(2).min(height);
    if menu_height < 3 || menu_width < 3 {
        return Vec::new();
    }
    let left = usize::try_from(pixel_position.0.max(0))
        .unwrap_or(usize::MAX)
        .checked_div(cell_width.max(1))
        .unwrap_or(0)
        .min(width.saturating_sub(menu_width));
    let top = usize::try_from(pixel_position.1.max(0))
        .unwrap_or(usize::MAX)
        .checked_div(cell_height.max(1))
        .unwrap_or(0)
        .min(height.saturating_sub(menu_height));
    let mut style = faces.builtin(i, "menu", 0).finish();
    if style.fg.is_none() {
        style.fg = default_style.fg;
    }
    if style.bg.is_none() {
        style.bg = default_style.bg.or(Some(CellColor(0x2e, 0x30, 0x33)));
    }
    style.box_style = Some(BoxStyle {
        vertical_width: 1,
        horizontal_width: 1,
        color: style.fg,
        kind: BoxStyleKind::Flat,
    });
    for row_index in top..top.saturating_add(menu_height) {
        if let Some(row) = screen.get_mut(row_index) {
            for cell in row.iter_mut().skip(left).take(menu_width) {
                *cell = ScreenCell {
                    chars: Vec::new(),
                    style,
                    occupied: true,
                    continuation: false,
                };
            }
        }
    }
    let mut hit_items = Vec::new();
    for (index, (label, command, separator, depth)) in flat_items
        .iter()
        .take(menu_height.saturating_sub(2))
        .enumerate()
    {
        let item_row = top.saturating_add(index).saturating_add(1);
        let text = if *separator {
            "─".repeat(menu_width.saturating_sub(2))
        } else {
            format!("{}{}", " ".repeat(depth.saturating_mul(2)), label)
        };
        let text = fit_text_to_cells_with_tab_width(&text, menu_width.saturating_sub(2), 8).0;
        let cell_styles = vec![style; menu_width.saturating_sub(2)];
        let char_styles = text.chars().map(|_| style).collect::<Vec<_>>();
        if let Some(row) = screen.get_mut(item_row) {
            place_text_row(
                row,
                left.saturating_add(1),
                &text,
                &cell_styles,
                &char_styles,
            );
        }
        hit_items.push(GridPopupMenuItem {
            row: item_row,
            left: left.saturating_add(1),
            width: menu_width.saturating_sub(2),
            command: if *separator { None } else { command.clone() },
        });
    }
    hit_items
}

fn scroll_thumb(
    track_length: usize,
    total: usize,
    visible: usize,
    offset: usize,
    minimum_size: usize,
) -> Option<(usize, usize)> {
    if track_length == 0 || visible == 0 || total == 0 {
        return None;
    }
    let max_offset = total.saturating_sub(visible);
    let thumb_size = if max_offset == 0 {
        track_length
    } else {
        track_length
            .saturating_mul(visible)
            .checked_div(total)
            .unwrap_or(0)
            .max(minimum_size)
            .min(track_length)
    };
    let travel = track_length.saturating_sub(thumb_size);
    let thumb_start = if max_offset == 0 {
        0
    } else {
        travel
            .saturating_mul(offset.min(max_offset))
            .checked_div(max_offset)
            .unwrap_or(0)
    };
    Some((thumb_start, thumb_size))
}

struct RenderedRow {
    text: String,
    cell_attrs: Vec<CellStyle>,
    char_attrs: Vec<CellStyle>,
    source_offsets: Vec<usize>,
    source_columns: Vec<usize>,
    cell_offsets: Vec<usize>,
    visible_columns: usize,
    padding_char_start: usize,
    images: Vec<RenderedImage>,
    compositions: Vec<RenderedComposition>,
}

#[derive(Clone, PartialEq, Eq)]
struct RenderedImage {
    column: usize,
    source: GridImageSource,
    width: Option<usize>,
    height: Option<usize>,
}

#[derive(Clone, PartialEq, Eq)]
struct RenderedComposition {
    column: usize,
    components: Vec<char>,
    style: CellStyle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Visible,
    Hidden,
    Ellipsis,
}

#[derive(Clone, PartialEq, Eq)]
enum DisplayReplacement {
    Text(String),
    GlyphlessText(String, CellStyle),
    CompositionDefault(usize),
    Image {
        source: GridImageSource,
        width: Option<usize>,
        height: Option<usize>,
    },
    /// Another character in a run already replaced by its first character.
    Hidden,
}

#[derive(Clone)]
struct ScreenCell {
    chars: Vec<(char, CellStyle)>,
    style: CellStyle,
    occupied: bool,
    continuation: bool,
}

impl ScreenCell {
    fn new(style: CellStyle) -> ScreenCell {
        ScreenCell {
            chars: Vec::new(),
            style,
            occupied: false,
            continuation: false,
        }
    }
}

fn place_rendered_row(screen: &mut [ScreenCell], left: usize, row: &RenderedRow) {
    place_text_row(screen, left, &row.text, &row.cell_attrs, &row.char_attrs);
}

fn place_text_row(
    screen: &mut [ScreenCell],
    left: usize,
    text: &str,
    cell_attrs: &[CellStyle],
    char_attrs: &[CellStyle],
) {
    let mut column = 0usize;
    let mut last_lead = None;
    for (index, ch) in text.chars().enumerate() {
        let char_width = display_char_width(ch);
        let style = char_attrs.get(index).copied().unwrap_or_default();
        if char_width == 0 {
            let target = last_lead
                .unwrap_or_else(|| left + column.min(screen.len().saturating_sub(left + 1)));
            if let Some(cell) = screen.get_mut(target) {
                cell.chars.push((ch, style));
                cell.occupied = true;
                cell.continuation = false;
            }
            continue;
        }
        let Some(target) = screen.get_mut(left.saturating_add(column)) else {
            break;
        };
        target.chars.clear();
        target.chars.push((ch, style));
        target.style = cell_attrs.get(column).copied().unwrap_or_default();
        target.occupied = true;
        target.continuation = false;
        last_lead = Some(left + column);
        for offset in 1..char_width {
            let Some(continuation) = screen.get_mut(left.saturating_add(column + offset)) else {
                break;
            };
            continuation.chars.clear();
            continuation.style = cell_attrs.get(column + offset).copied().unwrap_or(style);
            continuation.occupied = true;
            continuation.continuation = true;
        }
        column = column.saturating_add(char_width);
    }
}

fn screen_row_find_text_range(
    screen_row: &[ScreenCell],
    left: usize,
    width: usize,
    text: &str,
) -> Option<(usize, usize)> {
    let characters: Vec<char> = text.chars().collect();
    if characters.is_empty() {
        return None;
    }
    let right = left.saturating_add(width).min(screen_row.len());
    for start in left..right {
        if screen_row[start].continuation || screen_row[start].chars.is_empty() {
            continue;
        }
        let mut column = start;
        let mut index = 0;
        while index < characters.len() && column < right {
            let cell = &screen_row[column];
            if cell.continuation {
                column += 1;
                continue;
            }
            if cell.chars.is_empty() {
                break;
            }
            for (character, _) in &cell.chars {
                if characters.get(index) != Some(character) {
                    index = usize::MAX;
                    break;
                }
                index += 1;
            }
            if index == usize::MAX {
                break;
            }
            column += display_char_width(cell.chars[0].0).max(1);
        }
        if index == characters.len() {
            return Some((start, column));
        }
    }
    None
}

fn place_fringe_glyph(
    screen: &mut [Vec<ScreenCell>],
    row: usize,
    column: usize,
    fringe_width: usize,
    glyph: &str,
    style: CellStyle,
) {
    if fringe_width == 0 || glyph.is_empty() {
        return;
    }
    let glyph = if glyph.chars().count() > fringe_width {
        glyph
            .chars()
            .last()
            .map(|character| character.to_string())
            .unwrap_or_default()
    } else {
        glyph.to_owned()
    };
    let cells = vec![style; fringe_width];
    let characters = glyph.chars().map(|_| style).collect::<Vec<_>>();
    if let Some(screen_row) = screen.get_mut(row) {
        place_text_row(screen_row, column, &glyph, &cells, &characters);
    }
}

fn place_formatted_window_row(
    i: &Interp,
    faces: &mut FaceCtx,
    screen: &mut [Vec<ScreenCell>],
    row: usize,
    left: usize,
    width: usize,
    tab_width: usize,
    buffer_id: usize,
    format: &Value,
    context: &ModeLineContext,
    face_name: &str,
) {
    let Some(screen_row) = screen.get_mut(row) else {
        return;
    };
    let text = format_mode_line_value(i, buffer_id, format, context, 0);
    let row_text = fit_text_to_cells_with_tab_width(&text, width, tab_width).0;
    let mut style = faces.builtin(i, face_name, 0).finish();
    if style == CellStyle::default()
        && matches!(
            face_name,
            "mode-line-active" | "mode-line-inactive" | "header-line"
        )
    {
        style.inverse = true;
    }
    let cell_styles = vec![style; width];
    let character_styles = row_text.chars().map(|_| style).collect::<Vec<_>>();
    place_text_row(
        screen_row,
        left,
        &row_text,
        &cell_styles,
        &character_styles,
    );
}

fn compose_screen_rows(
    screen: &[Vec<ScreenCell>],
    width: usize,
    default_style: CellStyle,
) -> (
    Vec<String>,
    Vec<Vec<CellStyle>>,
    Vec<Vec<CellStyle>>,
    Vec<Vec<usize>>,
) {
    let mut rows = Vec::with_capacity(screen.len());
    let mut attrs = Vec::with_capacity(screen.len());
    let mut char_attrs = Vec::with_capacity(screen.len());
    let mut cell_offsets = Vec::with_capacity(screen.len());
    for screen_row in screen {
        let mut text = String::new();
        let mut cell_styles = Vec::with_capacity(width);
        let mut char_styles = Vec::new();
        let mut offsets = vec![0; width + 1];
        for column in 0..width {
            offsets[column] = text.len();
            let cell = &screen_row[column];
            cell_styles.push(cell.style);
            if !cell.continuation {
                if cell.chars.is_empty() {
                    text.push(' ');
                    char_styles.push(if cell.occupied {
                        cell.style
                    } else {
                        default_style
                    });
                } else {
                    for (ch, style) in &cell.chars {
                        text.push(*ch);
                        char_styles.push(*style);
                    }
                }
            }
            offsets[column + 1] = text.len();
        }
        rows.push(text);
        attrs.push(cell_styles);
        char_attrs.push(char_styles);
        cell_offsets.push(offsets);
    }
    (rows, attrs, char_attrs, cell_offsets)
}

/// Render a Lisp line into a fixed number of display cells. The source
/// and cell offset maps let the GUI position its caret in GPUI's shaped
/// line while the terminal keeps using cell coordinates.
fn append_rendered_character(
    display: char,
    style: CellStyle,
    hscroll: usize,
    width: usize,
    end_column: usize,
    tab_width: usize,
    column: &mut usize,
    visible_columns: &mut usize,
    rendered: &mut String,
    cell_attrs: &mut [CellStyle],
    char_attrs: &mut Vec<CellStyle>,
    cell_offsets: &mut [usize],
) -> bool {
    let columns = if display == '\t' {
        tab_width - *column % tab_width
    } else {
        display_char_width(display)
    };
    let start_column = *column;
    let next_column = column.saturating_add(columns);
    let before = rendered.len();

    if columns == 0 {
        if start_column >= hscroll && start_column < end_column {
            rendered.push(display);
            char_attrs.push(style);
        }
        return true;
    }

    if next_column <= hscroll {
        *column = next_column;
        return true;
    }
    if start_column >= end_column {
        return false;
    }

    let cell_start = start_column.saturating_sub(hscroll);
    let cell_end = next_column.min(end_column).saturating_sub(hscroll);
    if start_column < hscroll || next_column > end_column {
        // A horizontal boundary can bisect a wide glyph. Preserve its
        // remaining cell geometry with blanks instead of shifting the
        // following text or drawing half a glyph outside the viewport.
        for cell in cell_start..cell_end {
            if let Some(offset) = cell_offsets.get_mut(cell) {
                *offset = rendered.len();
            }
            if let Some(attribute) = cell_attrs.get_mut(cell) {
                *attribute = style;
            }
            rendered.push(' ');
            char_attrs.push(style);
            if let Some(offset) = cell_offsets.get_mut(cell + 1) {
                *offset = rendered.len();
            }
        }
    } else {
        for cell in cell_start..cell_end {
            if let Some(offset) = cell_offsets.get_mut(cell) {
                *offset = before;
            }
            if let Some(attribute) = cell_attrs.get_mut(cell) {
                *attribute = style;
            }
        }
        if display == '\t' {
            for cell in cell_start..cell_end {
                rendered.push(' ');
                char_attrs.push(style);
                if let Some(offset) = cell_offsets.get_mut(cell + 1) {
                    *offset = rendered.len();
                }
            }
        } else {
            rendered.push(display);
            char_attrs.push(style);
        }
        if let Some(offset) = cell_offsets.get_mut(cell_end) {
            *offset = rendered.len();
        }
    }
    *visible_columns = (*visible_columns).max(cell_end.min(width));
    *column = next_column;
    true
}

#[allow(clippy::too_many_arguments)]
fn append_rendered_text(
    text: &str,
    style: CellStyle,
    hscroll: usize,
    width: usize,
    end_column: usize,
    tab_width: usize,
    column: &mut usize,
    visible_columns: &mut usize,
    rendered: &mut String,
    cell_attrs: &mut [CellStyle],
    char_attrs: &mut Vec<CellStyle>,
    cell_offsets: &mut [usize],
) -> bool {
    for raw_display in text.chars() {
        let display = match raw_display {
            '\n' | '\r' => ' ',
            other => other,
        };
        if !append_rendered_character(
            display,
            style,
            hscroll,
            width,
            end_column,
            tab_width,
            column,
            visible_columns,
            rendered,
            cell_attrs,
            char_attrs,
            cell_offsets,
        ) {
            return false;
        }
    }
    true
}

fn render_text_row<F, V, D, B>(
    text: &EmacsText,
    hscroll: usize,
    width: usize,
    column_offset: usize,
    tab_width: usize,
    default_style: CellStyle,
    mut style_at: F,
    mut visibility_at: V,
    mut display_at: D,
    mut boundary_strings_at: B,
    include_end_boundary: bool,
    graphical: bool,
) -> RenderedRow
where
    F: FnMut(usize) -> CellStyle,
    V: FnMut(usize) -> Visibility,
    D: FnMut(usize) -> Option<DisplayReplacement>,
    B: FnMut(usize) -> Vec<String>,
{
    let chars: Vec<_> = text.chars().collect();
    let mut rendered = String::new();
    let mut char_attrs = Vec::new();
    let mut cell_attrs = vec![default_style; width];
    let mut source_offsets = vec![0; chars.len() + 1];
    let mut source_columns = vec![0; chars.len() + 1];
    let mut cell_offsets = vec![0; width + 1];
    let hscroll = hscroll.saturating_add(column_offset);
    let mut column = column_offset;
    let mut visible_columns = 0usize;
    let end_column = hscroll.saturating_add(width);
    let tab_width = tab_width.max(1);
    let mut hidden_run = false;
    let mut images = Vec::new();
    let mut compositions = Vec::new();

    for (index, ch) in chars.iter().enumerate() {
        let mut keep_rendering = true;
        for string in boundary_strings_at(index) {
            if !append_rendered_text(
                &string,
                default_style,
                hscroll,
                width,
                end_column,
                tab_width,
                &mut column,
                &mut visible_columns,
                &mut rendered,
                &mut cell_attrs,
                &mut char_attrs,
                &mut cell_offsets,
            ) {
                keep_rendering = false;
                break;
            }
        }
        let offset = if column <= hscroll { 0 } else { rendered.len() };
        source_offsets[index] = offset;
        source_columns[index] = column;
        if !keep_rendering {
            source_offsets[index..].fill(offset);
            source_columns[index..].fill(column);
            break;
        }
        let visibility = visibility_at(index);
        let draw_ellipsis = visibility == Visibility::Ellipsis && !hidden_run;
        if visibility != Visibility::Visible && !draw_ellipsis {
            source_offsets[index + 1] = rendered.len();
            source_columns[index + 1] = column;
            hidden_run = true;
            continue;
        }
        hidden_run = visibility != Visibility::Visible;
        let style = style_at(index);
        let replacement = if draw_ellipsis {
            None
        } else {
            display_at(index)
        };
        let keep_rendering = match replacement {
            Some(DisplayReplacement::Hidden) => true,
            Some(DisplayReplacement::Text(replacement)) => {
                append_rendered_text(
                    &replacement,
                    style,
                    hscroll,
                    width,
                    end_column,
                    tab_width,
                    &mut column,
                    &mut visible_columns,
                    &mut rendered,
                    &mut cell_attrs,
                    &mut char_attrs,
                    &mut cell_offsets,
                )
            }
            Some(DisplayReplacement::GlyphlessText(replacement, glyphless_style)) => {
                append_rendered_text(
                    &replacement,
                    glyphless_style,
                    hscroll,
                    width,
                    end_column,
                    tab_width,
                    &mut column,
                    &mut visible_columns,
                    &mut rendered,
                    &mut cell_attrs,
                    &mut char_attrs,
                    &mut cell_offsets,
                )
            }
            Some(DisplayReplacement::CompositionDefault(source_width)) => {
                let mut components: Vec<char> = chars
                    .iter()
                    .skip(index)
                    .take(source_width)
                    .filter_map(|character| character.unicode())
                    .collect();
                if components.is_empty() {
                    components.push(ch.unicode().unwrap_or('\u{fffd}'));
                }
                if graphical {
                    if let Some(base) = components.first().copied() {
                        let base_column = column;
                        let keep_rendering = append_rendered_character(
                            base,
                            style,
                            hscroll,
                            width,
                            end_column,
                            tab_width,
                            &mut column,
                            &mut visible_columns,
                            &mut rendered,
                            &mut cell_attrs,
                            &mut char_attrs,
                            &mut cell_offsets,
                        );
                        if keep_rendering
                            && components.len() > 1
                            && base_column >= hscroll
                            && base_column < end_column
                        {
                            compositions.push(RenderedComposition {
                                column: base_column.saturating_sub(hscroll),
                                components,
                                style,
                            });
                        }
                        keep_rendering
                    } else {
                        true
                    }
                } else {
                    let text = components.into_iter().collect::<String>();
                    append_rendered_text(
                        &text,
                        style,
                        hscroll,
                        width,
                        end_column,
                        tab_width,
                        &mut column,
                        &mut visible_columns,
                        &mut rendered,
                        &mut cell_attrs,
                        &mut char_attrs,
                        &mut cell_offsets,
                    )
                }
            }
            Some(DisplayReplacement::Image {
                source,
                width: image_width,
                height: image_height,
            }) => {
                let image_column = column;
                let visible = image_column >= hscroll && image_column < end_column;
                if visible {
                    images.push(RenderedImage {
                        column: image_column - hscroll,
                        source,
                        width: image_width,
                        height: image_height,
                    });
                }
                append_rendered_character(
                    ' ',
                    style,
                    hscroll,
                    width,
                    end_column,
                    tab_width,
                    &mut column,
                    &mut visible_columns,
                    &mut rendered,
                    &mut cell_attrs,
                    &mut char_attrs,
                    &mut cell_offsets,
                )
            }
            None => {
                let display = if draw_ellipsis {
                    '…'
                } else {
                    match ch.unicode().unwrap_or('\u{fffd}') {
                        '\n' | '\r' => ' ',
                        other => other,
                    }
                };
                append_rendered_character(
                    display,
                    style,
                    hscroll,
                    width,
                    end_column,
                    tab_width,
                    &mut column,
                    &mut visible_columns,
                    &mut rendered,
                    &mut cell_attrs,
                    &mut char_attrs,
                    &mut cell_offsets,
                )
            }
        };
        let offset = if column <= hscroll { 0 } else { rendered.len() };
        source_offsets[index + 1] = offset;
        source_columns[index + 1] = column;
        if !keep_rendering {
            source_offsets[index + 2..].fill(offset);
            source_columns[index + 2..].fill(column);
            break;
        }
    }

    if include_end_boundary {
        for string in boundary_strings_at(chars.len()) {
            if !append_rendered_text(
                &string,
                default_style,
                hscroll,
                width,
                end_column,
                tab_width,
                &mut column,
                &mut visible_columns,
                &mut rendered,
                &mut cell_attrs,
                &mut char_attrs,
                &mut cell_offsets,
            ) {
                break;
            }
        }
    }
    let offset = if column <= hscroll { 0 } else { rendered.len() };
    source_offsets[chars.len()] = offset;
    source_columns[chars.len()] = column;

    // The line end maps to the end of its visible text, before padding.
    let padding_char_start = char_attrs.len();
    for cell in visible_columns..width {
        cell_offsets[cell] = rendered.len();
        rendered.push(' ');
        char_attrs.push(default_style);
        cell_offsets[cell + 1] = rendered.len();
    }
    cell_offsets[width] = rendered.len();
    RenderedRow {
        text: rendered,
        cell_attrs,
        char_attrs,
        source_offsets,
        source_columns,
        cell_offsets,
        visible_columns,
        padding_char_start,
        images,
        compositions,
    }
}

fn prepend_rendered_prefix(
    prefix: &EmacsText,
    prefix_width: usize,
    tab_width: usize,
    mut row: RenderedRow,
) -> RenderedRow {
    if prefix_width == 0 {
        return row;
    }
    let prefix: String = prefix
        .chars()
        .map(|character| character.unicode().unwrap_or('\u{fffd}'))
        .collect();
    let prefix = fit_text_to_cells_with_tab_width(&prefix, prefix_width, tab_width).0;
    let prefix_bytes = prefix.len();
    let prefix_char_count = prefix.chars().count();
    let mut prefix_cell_offsets = vec![0; prefix_width + 1];
    let mut column = 0usize;
    for (byte, character) in prefix.char_indices() {
        let width = display_char_width(character);
        for cell in column..column.saturating_add(width).min(prefix_width) {
            prefix_cell_offsets[cell] = byte;
        }
        column = column.saturating_add(width);
    }
    for offset in prefix_cell_offsets.iter_mut().skip(column).take(prefix_width - column) {
        *offset = prefix_bytes;
    }
    prefix_cell_offsets[prefix_width] = prefix_bytes;

    let mut text = String::with_capacity(prefix_bytes.saturating_add(row.text.len()));
    text.push_str(&prefix);
    text.push_str(&row.text);
    row.text = text;
    let mut cell_attrs = vec![CellStyle::default(); prefix_width];
    cell_attrs.extend(row.cell_attrs);
    row.cell_attrs = cell_attrs;
    let mut char_attrs = vec![CellStyle::default(); prefix_char_count];
    char_attrs.extend(row.char_attrs);
    row.char_attrs = char_attrs;
    row.source_offsets
        .iter_mut()
        .for_each(|offset| *offset = offset.saturating_add(prefix_bytes));
    let mut cell_offsets = prefix_cell_offsets[..prefix_width].to_vec();
    cell_offsets.extend(
        row.cell_offsets
            .iter()
            .map(|offset| offset.saturating_add(prefix_bytes)),
    );
    row.cell_offsets = cell_offsets;
    row.visible_columns = row.visible_columns.saturating_add(prefix_width);
    row.padding_char_start = row.padding_char_start.saturating_add(prefix_char_count);
    for image in &mut row.images {
        image.column = image.column.saturating_add(prefix_width);
    }
    for composition in &mut row.compositions {
        composition.column = composition.column.saturating_add(prefix_width);
    }
    row
}

/// Truncate and pad text to WIDTH display columns. The tuple contains the
/// padded text, occupied columns before padding, and visible text's byte end.
pub fn fit_text_to_cells(text: &str, width: usize) -> (String, usize, usize) {
    fit_text_to_cells_with_tab_width(text, width, 8)
}

fn fit_text_to_cells_with_tab_width(
    text: &str,
    width: usize,
    tab_width: usize,
) -> (String, usize, usize) {
    let mut rendered = String::new();
    let mut columns = 0usize;
    let tab_width = tab_width.max(1);
    for raw_ch in text.chars() {
        let ch = match raw_ch {
            '\n' | '\r' => ' ',
            _ => raw_ch,
        };
        let char_width = if ch == '\t' {
            tab_width - columns % tab_width
        } else {
            display_char_width(ch)
        };
        let next = columns.saturating_add(char_width);
        if next > width {
            break;
        }
        if ch == '\t' {
            rendered.push_str(&" ".repeat(char_width));
        } else {
            rendered.push(ch);
        }
        columns = next;
    }
    let visible_bytes = rendered.len();
    rendered.push_str(&" ".repeat(width.saturating_sub(columns)));
    (rendered, columns, visible_bytes)
}
// ---------- face resolution ----------
//
// GNU realizes faces in xfaces.c (`realize_face' +
// `compute_char_face'): the `default' face seeds every cell, the
// active region and `face'/`font-lock-face' text properties overlay
// it, and overlays stack on top by ascending `priority'.  The face
// attribute layers here are the same: defface specs land in
// `Interp::face_table' via `internal-set-lisp-face-attribute'/the
// prelude's `face-spec-set', `(set-face-attribute FACE t ...)' goes to
// `face_t_table', and `face-alias' links resolve like GNU's
// `lface_from_face_name'.

/// Per-render face computation: symbol/keyword ids looked up once,
/// the color name table from `color-name-rgb-alist', and a cache of
/// already-resolved named faces.
struct FaceCtx {
    // Symbol ids for property names (SymId::MAX = missing).
    face: u32,
    font_lock_face: u32,
    mouse_face: u32,
    pointer: u32,
    line_spacing: u32,
    line_height: u32,
    mark_active: u32,
    face_remapping_alist: u32,
    inherit: u32,
    invisible: u32,
    buffer_invisibility_spec: u32,
    box_color: u32,
    box_line_width: u32,
    box_style: u32,
    /// `color-name-rgb-alist' folded into (canonical name -> rgb).
    colors: HashMap<EmacsText, (u8, u8, u8)>,
    /// Face family names interned for compact, copyable cell styles.
    font_families: Vec<String>,
    font_family_ids: HashMap<String, u16>,
    /// Bitmap face backgrounds interned for compact cell styles.
    stipples: Vec<GridStipple>,
    stipple_ids: HashMap<GridStipple, u16>,
    /// Resolved face name -> merged StyleMix.
    cache: HashMap<crate::lisp::value::SymId, StyleMix>,
}

impl FaceCtx {
    fn new(i: &Interp) -> FaceCtx {
        let sym = |n: &str| i.intern_soft(n).unwrap_or(u32::MAX);
        let mut colors: HashMap<EmacsText, (u8, u8, u8)> = HashMap::new();
        // The eight standard colors come from tty-standard-colors so
        // they resolve even if color-name-rgb-alist never loads.
        for (n, r, g, b) in [
            ("black", 0u32, 0u32, 0u32),
            ("red", 65535, 0, 0),
            ("green", 0, 65535, 0),
            ("yellow", 65535, 65535, 0),
            ("blue", 0, 0, 65535),
            ("magenta", 65535, 0, 65535),
            ("cyan", 0, 65535, 65535),
            ("white", 65535, 65535, 65535),
        ] {
            colors.insert(
                EmacsText::from(n),
                ((r / 256) as u8, (g / 256) as u8, (b / 256) as u8),
            );
        }
        if let Value::Cons(_) = i.symbol_value(sym("color-name-rgb-alist")) {
            let alist = i.symbol_value(sym("color-name-rgb-alist"));
            if let Ok(entries) = alist.list_to_vec() {
                for e in entries {
                    if let Value::Cons(c) = &e {
                        let (k, v) = {
                            let cb = c.borrow();
                            (cb.car.clone(), cb.cdr.clone())
                        };
                        if let Value::Str(name) = &k {
                            if let Ok(rgb) = v.list_to_vec() {
                                let mut c8 = [0u8; 3];
                                let mut ok = rgb.len() >= 3;
                                for (n, x) in rgb.iter().take(3).enumerate() {
                                    match x {
                                        Value::Int(v) => {
                                            c8[n] = ((*v).clamp(0, 65535) / 256) as u8;
                                        }
                                        _ => ok = false,
                                    }
                                }
                                if ok {
                                    colors.insert(
                                        name.borrow().to_lowercase().replace(' ', ""),
                                        (c8[0], c8[1], c8[2]),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        FaceCtx {
            face: sym("face"),
            font_lock_face: sym("font-lock-face"),
            mouse_face: sym("mouse-face"),
            pointer: sym("pointer"),
            line_spacing: sym("line-spacing"),
            line_height: sym("line-height"),
            mark_active: sym("mark-active"),
            face_remapping_alist: sym("face-remapping-alist"),
            inherit: sym(":inherit"),
            invisible: sym("invisible"),
            buffer_invisibility_spec: sym("buffer-invisibility-spec"),
            box_color: sym(":color"),
            box_line_width: sym(":line-width"),
            box_style: sym(":style"),
            colors,
            font_families: Vec::new(),
            font_family_ids: HashMap::new(),
            stipples: Vec::new(),
            stipple_ids: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// GNU `tty_color_standard_values' + `tty-color-canonicalize':
    /// `#rgb'/`#rrggbb' parse directly, else the canonicalized name is
    /// looked up in the color-name alist.
    fn color(&self, v: &Value) -> Option<CellColor> {
        let Value::Str(s) = v else {
            return None;
        };
        let t = s.borrow();
        if let Some(hex) = t.strip_prefix('#') {
            let parse = |h: &EmacsStr| {
                h.chars().try_fold(0u8, |value, c| c.to_digit(16).map(|digit| value * 16 + digit as u8))
            };
            return match hex.len() {
                6 => Some(CellColor(
                    parse(&hex[0..2])?,
                    parse(&hex[2..4])?,
                    parse(&hex[4..6])?,
                )),
                3 => {
                    let mut it = hex.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8));
                    let (r, g, b) = (it.next()?, it.next()?, it.next()?);
                    Some(CellColor(r * 17, g * 17, b * 17))
                }
                _ => None,
            };
        }
        let canon = t.to_lowercase().replace(' ', "");
        self.colors.get(&canon).map(|&(r, g, b)| CellColor(r, g, b))
    }

    fn font_family_id(&mut self, value: &Value) -> Option<u16> {
        let Value::Str(name) = value else {
            return None;
        };
        let family = name.borrow().to_unicode_lossy();
        if let Some(id) = self.font_family_ids.get(&family) {
            return Some(*id);
        }
        let id = u16::try_from(self.font_families.len()).ok()?;
        self.font_families.push(family.clone());
        self.font_family_ids.insert(family, id);
        Some(id)
    }

    fn stipple_id(&mut self, value: &Value) -> Option<Option<u16>> {
        if value.is_nil() {
            return Some(None);
        }
        let values = match value {
            Value::Vec(vector) => vector.borrow().clone(),
            Value::Cons(_) => value.list_to_vec().ok()?,
            _ => return None,
        };
        if values.len() != 3 {
            return None;
        }
        let width = usize::try_from(values[0].int()?.max(1)).ok()?;
        let height = usize::try_from(values[1].int()?.max(1)).ok()?;
        let width = u16::try_from(width).ok()?;
        let height = u16::try_from(height).ok()?;
        let Value::Str(data) = &values[2] else {
            return None;
        };
        let bytes: Vec<u8> = data
            .borrow()
            .chars()
            .map(|character| {
                character
                    .byte8()
                    .or_else(|| u8::try_from(character.code()).ok())
            })
            .collect::<Option<_>>()?;
        let row_bytes = usize::from(width).div_ceil(8);
        let data_len = row_bytes.checked_mul(usize::from(height))?;
        if bytes.len() < data_len {
            return None;
        }
        let stipple = GridStipple {
            width,
            height,
            data: Arc::<[u8]>::from(bytes[..data_len].to_vec()),
        };
        if let Some(id) = self.stipple_ids.get(&stipple) {
            return Some(Some(*id));
        }
        let id = u16::try_from(self.stipples.len()).ok()?;
        self.stipples.push(stipple.clone());
        self.stipple_ids.insert(stipple, id);
        Some(Some(id))
    }

    fn parse_box_style(&self, i: &Interp, value: &Value) -> Option<BoxStyle> {
        if value.is_nil() {
            return None;
        }
        let mut style = BoxStyle::default();
        match value {
            Value::Str(_) => style.color = self.color(value),
            Value::Cons(_) => {
                let line_width = crate::lisp::eval::plist_get(value, self.box_line_width);
                if let Value::Int(width) = line_width {
                    let width = width.clamp(i16::MIN as i128, i16::MAX as i128) as i16;
                    style.vertical_width = width;
                    style.horizontal_width = width;
                } else if let Value::Cons(widths) = line_width {
                    let (vertical, horizontal) = {
                        let widths = widths.borrow();
                        (widths.car.int(), widths.cdr.int())
                    };
                    if let Some(width) = vertical {
                        style.vertical_width =
                            width.clamp(i16::MIN as i128, i16::MAX as i128) as i16;
                    }
                    if let Some(width) = horizontal {
                        style.horizontal_width =
                            width.clamp(i16::MIN as i128, i16::MAX as i128) as i16;
                    }
                }
                style.color = self.color(&crate::lisp::eval::plist_get(value, self.box_color));
                if let Value::Sym(box_style) = crate::lisp::eval::plist_get(value, self.box_style) {
                    style.kind = match i.symbol_name(box_style).as_str() {
                        "released-button" => BoxStyleKind::Released,
                        "pressed-button" => BoxStyleKind::Pressed,
                        _ => BoxStyleKind::Flat,
                    };
                }
            }
            _ => {}
        }
        Some(style)
    }

    fn decoration_color(&self, value: &Value) -> Option<CellColor> {
        match value {
            Value::Cons(_) => self.color(&crate::lisp::eval::plist_get(value, self.box_color)),
            _ => self.color(value),
        }
    }

    fn underline_wave(&self, i: &Interp, value: &Value) -> bool {
        let Value::Cons(_) = value else {
            return false;
        };
        matches!(
            crate::lisp::eval::plist_get(value, self.box_style),
            Value::Sym(style) if i.symbol_name(style) == "wave"
        )
    }

    /// Merge one named face's attributes (spec layer first, then the
    /// new-frame defaults layer like GNU's realize_default_face, then
    /// the compiled-in basic-face fallbacks).
    fn builtin(&mut self, i: &Interp, name: &str, depth: u32) -> StyleMix {
        match i.q_soft(name) {
            Some(face) => self.named(i, &face, depth),
            None => builtin_face_defaults(i, name),
        }
    }

    fn named(&mut self, i: &Interp, name: &crate::lisp::value::SymId, depth: u32) -> StyleMix {
        if let Some(s) = self.cache.get(name) {
            return *s;
        }
        // The realized value: face_table plist (defface/theme layer)
        // over the new-frame defaults layer over built-in defaults.
        let mut mix = builtin_face_defaults(i, crate::editor::builtin_face_name(i, *name).unwrap_or(""));
        if let Some((_, plist)) = i.face_t_table.iter().find(|(n, _)| n == name) {
            mix.merge(&self.face_plist(i, plist, depth));
        }
        if let Some((_, plist)) = i.face_table.iter().find(|(n, _)| n == name) {
            mix.merge(&self.face_plist(i, plist, depth));
        }
        self.cache.insert(*name, mix);
        mix
    }

    /// One face's attribute plist: :inherit expands first (GNU merge
    /// order), then the face's own attributes override it.
    fn face_plist(&mut self, i: &Interp, plist: &Value, depth: u32) -> StyleMix {
        let mut mix = StyleMix::default();
        if depth >= 16 {
            return mix;
        }
        let inherit = crate::lisp::eval::plist_get(plist, self.inherit);
        if inherit.truthy() {
            mix.merge(&self.face_value(i, &inherit, depth + 1));
        }
        mix.merge(&self.attrs(i, plist));
        mix
    }

    /// The attribute keywords of a plist, without :inherit.
    /// The attribute keywords of a plist, without :inherit.
    fn attrs(&mut self, i: &Interp, plist: &Value) -> StyleMix {
        let mut mix = StyleMix::default();
        let mut distant_fg = None;
        let Ok(items) = plist.list_to_vec() else {
            return mix;
        };
        for kv in items.chunks(2) {
            let (Some(k), Some(v)) = (i.sym_id(&kv[0]), kv.get(1)) else {
                continue;
            };
            match i.symbol_name(k).as_str() {
                ":foreground" => {
                    if let Some(c) = self.color(v) {
                        mix.fg = Some(c);
                    }
                }
                ":distant-foreground" => distant_fg = self.color(v),
                ":background" => {
                    if let Some(c) = self.color(v) {
                        mix.bg = Some(c);
                    }
                }
                ":family" => mix.font_family = self.font_family_id(v),
                ":font" => {
                    if let Value::Str(_) = v {
                        mix.font_family = self.font_family_id(v);
                    }
                }
                ":weight" => {
                    let weight = font_weight_value(i, v);
                    mix.font_weight = Some(weight);
                    mix.bold = Some(weight >= 600);
                }
                ":width" => mix.font_width = font_width_percent(i, v),
                ":height" => mix.font_height = font_height_tenths(v),
                ":stipple" => mix.stipple = self.stipple_id(v),
                ":box" => mix.box_style = Some(self.parse_box_style(i, v)),
                ":slant" => mix.italic = Some(slant_italic(i, v)),
                ":underline" => {
                    mix.underline = Some(v.truthy());
                    mix.underline_color = Some(self.decoration_color(v));
                    mix.underline_wave = Some(self.underline_wave(i, v));
                }
                ":overline" => mix.overline = Some(v.truthy()),
                ":strike-through" => {
                    mix.strike = Some(v.truthy());
                    mix.strike_color = Some(self.decoration_color(v));
                }
                ":inverse-video" => mix.inverse = Some(v.truthy()),
                ":extend" => mix.extend = Some(v.truthy()),
                _ => {}
            }
        }
        // :distant-foreground is only used when it contrasts with the
        // background; on this coarse model it fills fg when the face
        // sets no real :foreground (GNU drops it when it blends in).
        if mix.fg.is_none() {
            mix.fg = distant_fg;
        }
        mix
    }

    /// A `face' property value: a symbol/string names a face, a list
    /// whose car is a keyword is an anonymous face plist, any other
    /// list merges each element in order, and
    /// (foreground-color . C)/(background-color . C) pairs set colors.
    fn face_value(&mut self, i: &Interp, v: &Value, depth: u32) -> StyleMix {
        if depth >= 16 {
            return StyleMix::default();
        }
        match v {
            Value::Sym(s) => {
                match crate::editor::face_resolve(i, s) {
                    Some(t) => self.named(i, &t, depth + 1),
                    None => StyleMix::default(),
                }
            }
            Value::Str(s) => {
                match crate::editor::face_symbol(i, v).and_then(|face| crate::editor::face_resolve(i, &face)) {
                    Some(t) => self.named(i, &t, depth + 1),
                    None => StyleMix::default(),
                }
            }
            Value::Cons(_) => {
                let Ok(items) = v.list_to_vec() else {
                    // Dotted pair: (foreground-color . COLOR) /
                    // (background-color . COLOR).
                    let (car, cdr) = {
                        let Value::Cons(c) = v else {
                            return StyleMix::default();
                        };
                        let cb = c.borrow();
                        (cb.car.clone(), cb.cdr.clone())
                    };
                    let mut mix = StyleMix::default();
                    if let Some(cs) = i.sym_id(&car).map(|s| i.symbol_name(s)) {
                        if let Some(c) = self.color(&cdr) {
                            match cs.as_str() {
                                "foreground-color" => mix.fg = Some(c),
                                "background-color" => mix.bg = Some(c),
                                _ => {}
                            }
                        }
                    }
                    return mix;
                };
                let is_plist = i
                    .sym_id(&items[0])
                    .map(|s| i.symbol_name(s).starts_with(':'))
                    .unwrap_or(false);
                if is_plist {
                    // Anonymous face: a plist of attributes.
                    self.face_plist(i, v, depth + 1)
                } else {
                    let mut mix = StyleMix::default();
                    for it in items {
                        mix.merge(&self.face_value(i, &it, depth + 1));
                    }
                    mix
                }
            }
            _ => StyleMix::default(),
        }
    }
}

/// GNU weight mapping (xfaces.c `face_numeric_weight').
fn font_weight_value(i: &Interp, v: &Value) -> u16 {
    let n: i128 = match v {
        Value::Int(n) => *n,
        Value::Sym(s) => match i.symbol_name(*s).as_str() {
            "thin" => 200,
            "ultra-light" | "extra-light" => 275,
            "light" => 300,
            "semi-light" | "book" => 380,
            "normal" | "regular" => 400,
            "medium" => 500,
            "semi-bold" | "demi-bold" | "demibold" => 600,
            "bold" => 700,
            "extra-bold" | "ultra-bold" | "extrabold" | "ultrabold" => 800,
            "black" | "heavy" | "ultra-heavy" => 900,
            _ => 400,
        },
        _ => 400,
    };
    n.clamp(100, 900) as u16
}

fn font_width_percent(i: &Interp, value: &Value) -> Option<u8> {
    match value {
        Value::Int(width) => Some((*width).clamp(50, 200) as u8),
        Value::Sym(symbol) => match i.symbol_name(*symbol).as_str() {
            "ultra-condensed" => Some(50),
            "extra-condensed" => Some(62),
            "condensed" | "compressed" | "narrow" => Some(75),
            "semi-condensed" | "demi-condensed" => Some(87),
            "normal" | "medium" | "regular" => Some(100),
            "semi-expanded" | "demi-expanded" => Some(112),
            "expanded" => Some(125),
            "extra-expanded" => Some(150),
            "ultra-expanded" | "wide" => Some(200),
            _ => None,
        },
        _ => None,
    }
}

fn font_height_tenths(value: &Value) -> Option<u16> {
    match value {
        Value::Int(height) if *height > 0 => u16::try_from(*height).ok(),
        _ => None,
    }
}

/// GNU slant mapping: italic/oblique slant the text.
fn slant_italic(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Sym(s) => !matches!(
            i.symbol_name(*s).as_str(),
            "normal" | "roman" | "unspecified" | "reversed-oblique" | "reversed-italic"
        ),
        _ => false,
    }
}

/// The attributes a face has even with no face_table row.  These
/// mirror the `(t ...)'/`((type tty) (class mono) ...)' fallbacks in
/// GNU's defface specs — what `face-spec-set' would leave on a
/// monochrome tty — for the faces `lisp/faces.el'/`lisp/isearch.el'
/// define but that aren't booted into `face_table'.
fn builtin_face_defaults(i: &Interp, name: &str) -> StyleMix {
    let mut mix = StyleMix::default();
    match name {
        "default" => {
            mix.bold = Some(false);
            mix.italic = Some(false);
            mix.underline = Some(false);
            mix.overline = Some(false);
            mix.strike = Some(false);
            mix.inverse = Some(false);
            mix.extend = Some(false);
        }
        "bold" => mix.bold = Some(true),
        "italic" => mix.italic = Some(true),
        "bold-italic" => {
            mix.bold = Some(true);
            mix.italic = Some(true);
        }
        "underline" => mix.underline = Some(true),
        // faces.el/isearch.el `(t :inverse-video t)' and
        // `((type tty) (class mono)) :inverse-video t' entries.
        "isearch"
        | "isearch-fail"
        | "highlight"
        | "secondary-selection"
        | "trailing-whitespace"
        | "match"
        | "query-replace"
        | "mode-line"
        | "mode-line-active"
        | "mode-line-inactive" => mix.inverse = Some(true),
        "region" => {
            // GNU's last-resort `(t :background "gray" :extend t)'.
            mix.bg = Some(CellColor(190, 190, 190));
            mix.extend = Some(true);
        }
        "lazy-highlight" => mix.underline = Some(true),
        "isearch-group-1" | "isearch-group-2" => mix.merge(&builtin_face_defaults(i, "isearch")),
        // `(t (:weight bold))' entries.
        "mode-line-emphasis" | "mode-line-buffer-id" => mix.bold = Some(true),
        _ => {}
    }
    mix
}

/// One window's face context for render_grid: the buffer's overlay
/// faces and `face'/`font-lock-face' text-property runs collected
/// once per render, and the active-region range.
struct WindowFaces {
    /// Buffer-local face replacements. Keep each cdr intact so direct
    /// composite specs remain distinct from face-remap helper entries.
    face_remappings: Vec<(crate::lisp::value::SymId, Value)>,
    /// Overlay snapshots and face values. Precedence is computed for the
    /// overlays covering each position, including interval containment.
    overlays: Vec<(crate::buffer::Overlay, Value)>,
    /// 'face/'font-lock-face' text-property runs: (start, end, value,
    /// insertion index).  Later-inserted props merge on top (GNU's
    /// interval "last write wins").
    prop_faces: Vec<(usize, usize, Value, usize)>,
    /// The active mouse-face run from the hovered text or overlay.
    mouse_face: Option<(usize, usize, Value)>,
    /// The pointer property supplied by the hovered text or overlay.
    pointer: Option<Value>,
    /// Newline line-spacing text-property runs.
    line_spacing_props: Vec<(usize, usize, Value, usize)>,
    /// Newline line-spacing overlay properties, with window restrictions applied.
    line_spacing_overlays: Vec<(crate::buffer::Overlay, Value)>,
    /// Newline line-height text-property runs.
    line_height_props: Vec<(usize, usize, Value, usize)>,
    /// Newline line-height overlay properties, with window restrictions applied.
    line_height_overlays: Vec<(crate::buffer::Overlay, Value)>,
    /// `help-echo` text-property runs and overlay values for hover tooltips.
    help_echo_props: Vec<(usize, usize, Value, usize)>,
    help_echo_overlays: Vec<(crate::buffer::Overlay, Value)>,
    /// `display` text-property runs; the run is replaced once, at its start.
    display_props: Vec<(usize, usize, Option<DisplayReplacement>, usize)>,
    composition_props: Vec<(usize, usize, Option<DisplayReplacement>, usize)>,
    /// Per-window, per-buffer, and standard display tables in precedence order.
    display_tables: Vec<Value>,
    /// Overlay strings positioned at the overlay's start/end boundaries.
    overlay_strings: HashMap<usize, Vec<String>>,
    /// Invisible text ranges and visible overlays carrying `invisible`.
    invisible_overlays: Vec<(crate::buffer::Overlay, Value)>,
    invisible_props: Vec<(usize, usize, Value)>,
    invisibility_spec: Value,
    /// Active region as a half-open range when the mark is active.
    region: Option<(usize, usize)>,
    /// Whether this buffer requests trailing whitespace highlighting.
    show_trailing_whitespace: bool,
    /// Trailing spaces/tabs on the currently rendered line.
    trailing_whitespace: Option<Range<usize>>,
}

#[derive(Clone)]
struct OverlayStrings {
    start: usize,
    end: usize,
    before: Option<String>,
    after: Option<String>,
}

fn window_faces(
    i: &Interp,
    ctx: &FaceCtx,
    b: &std::rc::Rc<std::cell::RefCell<Buffer>>,
    win: &Value,
    mouse_position: Option<MousePosition>,
) -> WindowFaces {
    let bb = b.borrow();
    let mouse_position = mouse_position
        .filter(|position| position.buffer_id == bb.id)
        .map(|position| position.buffer_position);
    // Region: GNU shows it between the mark and the buffer's point
    // while `mark-active' is non-nil.
    let region = {
        let active = i
            .buffer_local_toplevel_value(ctx.mark_active, bb.id)
            .or_else(|| i.default_toplevel_value(ctx.mark_active))
            .or_else(|| bb.locals.get(&ctx.mark_active).cloned())
            .map(|v| v.truthy())
            .unwrap_or(false);
        let mark = bb.mark.as_ref().and_then(|m| {
            let mm = m.borrow();
            (mm.buffer == Some(bb.id)).then_some(mm.position)
        });
        match (active, mark) {
            (true, Some(mk)) if mk != bb.point() => Some((mk.min(bb.point()), mk.max(bb.point()))),
            _ => None,
        }
    };
    let face_remappings = if ctx.face_remapping_alist == u32::MAX {
        Vec::new()
    } else {
        i.buffer_local_toplevel_value(ctx.face_remapping_alist, bb.id)
            .or_else(|| i.default_toplevel_value(ctx.face_remapping_alist))
            .or_else(|| bb.locals.get(&ctx.face_remapping_alist).cloned())
            .and_then(|alist| alist.list_to_vec().ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let Value::Cons(entry) = entry else {
                    return None;
                };
                let (face, remapping) = {
                    let entry = entry.borrow();
                    (entry.car.clone(), entry.cdr.clone())
                };
                Some((i.sym_id(&face)?, remapping))
            })
            .collect()
    };
    // Overlays covering this buffer, for this window (a `window'
    // property restricts an overlay to one window — lazy-highlight
    // uses it). Face candidates retain their slot order until point-specific
    // priority/containment ordering is computed below.
    let overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, ov)| {
            if ov.buffer != Some(bb.id) || ov.start >= ov.end {
                return false;
            }
            ov.matches_window(i, win)
        })
        .filter_map(|(_, ov)| {
            let fv = ov.property(i, &Value::Sym(ctx.face));
            if fv.is_nil() {
                return None;
            }
            Some((ov.clone(), fv))
        })
        .collect();
    let invisible_overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .filter(|ov| ov.buffer == Some(bb.id) && ov.start < ov.end && ov.matches_window(i, win))
        .filter_map(|overlay| {
            let value = overlay.property(i, &Value::Sym(ctx.invisible));
            (!value.is_nil()).then(|| (overlay.clone(), value))
        })
        .collect();
    let help_echo = i.q_soft("help-echo");
    let help_echo_overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .filter(|overlay| overlay.buffer == Some(bb.id) && overlay.matches_window(i, win))
        .filter_map(|overlay| {
            let value = help_echo
                .map(|property| overlay.property(i, &Value::Sym(property)))
                .unwrap_or(Value::Nil);
            (!value.is_nil()).then(|| (overlay.clone(), value))
        })
        .collect();
    let before_string = i.q_soft("before-string");
    let after_string = i.q_soft("after-string");
    let mut overlay_strings: Vec<(crate::buffer::Overlay, OverlayStrings)> = bb
        .overlays
        .iter()
        .filter(|overlay| overlay.buffer == Some(bb.id) && overlay.matches_window(i, win))
        .filter_map(|overlay| {
            let string_value = |property: Option<crate::lisp::value::SymId>| {
                property.and_then(|property| match overlay.property(i, &Value::Sym(property)) {
                    Value::Str(value) => Some(value.borrow().to_unicode_lossy()),
                    _ => None,
                })
            };
            let strings = OverlayStrings {
                start: overlay.start,
                end: overlay.end,
                before: string_value(before_string),
                after: string_value(after_string),
            };
            (strings.before.is_some() || strings.after.is_some())
                .then(|| (overlay.clone(), strings))
        })
        .collect();
    let mut overlay_order: Vec<_> = overlay_strings
        .iter()
        .enumerate()
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlay_order);
    let overlay_strings: Vec<OverlayStrings> = overlay_order
        .into_iter()
        .map(|(_, index)| overlay_strings[index].1.clone())
        .collect();
    let mut ending_strings: HashMap<usize, Vec<String>> = HashMap::new();
    let mut starting_strings: HashMap<usize, Vec<(Option<String>, Option<String>)>> =
        HashMap::new();
    for overlay in overlay_strings {
        if overlay.start < overlay.end
            && let Some(after) = &overlay.after
        {
            ending_strings
                .entry(overlay.end)
                .or_default()
                .push(after.clone());
        }
        let empty_after = (overlay.start == overlay.end)
            .then(|| overlay.after.clone())
            .flatten();
        if overlay.before.is_some() || empty_after.is_some() {
            starting_strings
                .entry(overlay.start)
                .or_default()
                .push((overlay.before, empty_after));
        }
    }
    let mut overlay_strings = ending_strings;
    for (position, starts) in starting_strings {
        let strings = overlay_strings.entry(position).or_default();
        for (before, after) in starts {
            if let Some(before) = before {
                strings.push(before);
            }
            if let Some(after) = after {
                strings.push(after);
            }
        }
    }
    let properties = bb.text.props.borrow();
    let prop_faces: Vec<(usize, usize, Value, usize)> = properties
        .iter()
        .enumerate()
        .filter(|(_, tp)| tp.prop == ctx.face || tp.prop == ctx.font_lock_face)
        .map(|(idx, tp)| (tp.start, tp.end, tp.value.clone(), idx))
        .collect();
    let help_echo_props: Vec<(usize, usize, Value, usize)> = help_echo.map_or_else(
        Vec::new,
        |property| {
            properties
                .iter()
                .enumerate()
                .filter(|(_, tp)| tp.prop == property)
                .map(|(idx, tp)| (tp.start, tp.end, tp.value.clone(), idx))
                .collect()
        },
    );
    let display_props: Vec<(usize, usize, Option<DisplayReplacement>, usize)> =
        i.q_soft("display").map_or_else(Vec::new, |display| {
            properties
                .iter()
                .enumerate()
                .filter(|(_, tp)| tp.prop == display)
                .map(|(idx, tp)| {
                    (
                        tp.start,
                        tp.end,
                        display_value_replacement(i, &tp.value, bb.id),
                        idx,
                    )
                })
                .collect()
        });
    let composition_props: Vec<(usize, usize, Option<DisplayReplacement>, usize)> =
        i.q_soft("composition").map_or_else(Vec::new, |composition| {
            properties
                .iter()
                .enumerate()
                .filter(|(_, tp)| tp.prop == composition)
                .map(|(idx, tp)| {
                    (
                        tp.start,
                        tp.end,
                        composition_display_replacement(&tp.value),
                        idx,
                    )
                })
                .collect()
        });
    let invisible_props: Vec<(usize, usize, Value)> = properties
        .iter()
        .filter(|tp| tp.prop == ctx.invisible)
        .map(|tp| (tp.start, tp.end, tp.value.clone()))
        .collect();
    let mouse_face_prop = mouse_position.and_then(|position| {
        properties
            .iter()
            .enumerate()
            .filter(|(_, tp)| {
                tp.prop == ctx.mouse_face
                    && !tp.value.is_nil()
                    && position >= tp.start
                    && position < tp.end
            })
            .max_by_key(|(index, _)| *index)
            .map(|(_, tp)| (tp.start, tp.end, tp.value.clone()))
    });
    let mouse_face_overlay = mouse_position.and_then(|position| {
        let candidates: Vec<_> = bb
            .overlays
            .iter()
            .enumerate()
            .filter(|(_, overlay)| {
                overlay.buffer == Some(bb.id)
                    && overlay.start <= position
                    && position < overlay.end
                    && overlay.matches_window(i, win)
            })
            .filter_map(|(slot, overlay)| {
                let face = overlay.property(i, &Value::Sym(ctx.mouse_face));
                (!face.is_nil()).then_some((overlay, slot, face))
            })
            .collect();
        let mut order: Vec<_> = candidates
            .iter()
            .enumerate()
            .map(|(candidate, (overlay, _, _))| (*overlay, candidate))
            .collect();
        crate::buffer::sort_overlays_by_priority(i, &mut order);
        order.last().map(|(_, candidate)| {
            let (overlay, _, face) = &candidates[*candidate];
            (overlay.start, overlay.end, face.clone())
        })
    });
    let mouse_face = mouse_face_overlay.or(mouse_face_prop);
    let pointer_prop = mouse_position.and_then(|position| {
        properties
            .iter()
            .enumerate()
            .filter(|(_, tp)| {
                tp.prop == ctx.pointer
                    && !tp.value.is_nil()
                    && position >= tp.start
                    && position < tp.end
            })
            .max_by_key(|(index, _)| *index)
            .map(|(_, tp)| tp.value.clone())
    });
    let pointer_overlay = mouse_position.and_then(|position| {
        let candidates: Vec<_> = bb
            .overlays
            .iter()
            .enumerate()
            .filter(|(_, overlay)| {
                overlay.buffer == Some(bb.id)
                    && overlay.start <= position
                    && position < overlay.end
                    && overlay.matches_window(i, win)
            })
            .filter_map(|(slot, overlay)| {
                let shape = overlay.property(i, &Value::Sym(ctx.pointer));
                (!shape.is_nil()).then_some((overlay, slot, shape))
            })
            .collect();
        let mut order: Vec<_> = candidates
            .iter()
            .enumerate()
            .map(|(candidate, (overlay, _, _))| (*overlay, candidate))
            .collect();
        crate::buffer::sort_overlays_by_priority(i, &mut order);
        order.last().map(|(_, candidate)| candidates[*candidate].2.clone())
    });
    let pointer = pointer_overlay.or(pointer_prop);
    let line_spacing_props = properties
        .iter()
        .enumerate()
        .filter(|(_, tp)| tp.prop == ctx.line_spacing)
        .map(|(index, tp)| (tp.start, tp.end, tp.value.clone(), index))
        .collect();
    let line_spacing_overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .filter(|overlay| {
            overlay.buffer == Some(bb.id)
                && overlay.start < overlay.end
                && overlay.matches_window(i, win)
        })
        .filter_map(|overlay| {
            let spacing = overlay.property(i, &Value::Sym(ctx.line_spacing));
            (!spacing.is_nil()).then(|| (overlay.clone(), spacing))
        })
        .collect();
    let line_height_props = properties
        .iter()
        .enumerate()
        .filter(|(_, tp)| tp.prop == ctx.line_height)
        .map(|(index, tp)| (tp.start, tp.end, tp.value.clone(), index))
        .collect();
    let line_height_overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .filter(|overlay| {
            overlay.buffer == Some(bb.id)
                && overlay.start < overlay.end
                && overlay.matches_window(i, win)
        })
        .filter_map(|overlay| {
            let height = overlay.property(i, &Value::Sym(ctx.line_height));
            (!height.is_nil()).then(|| (overlay.clone(), height))
        })
        .collect();
    let invisibility_spec = i
        .buffer_local_toplevel_value(ctx.buffer_invisibility_spec, bb.id)
        .or_else(|| i.default_toplevel_value(ctx.buffer_invisibility_spec))
        .or_else(|| bb.locals.get(&ctx.buffer_invisibility_spec).cloned())
        .unwrap_or(Value::Nil);
    let window_display_table = match win {
        Value::Window(window) => i
            .q_soft("display-table")
            .map(|property| crate::lisp::eval::plist_get(&window.borrow().params, property))
            .unwrap_or(Value::Nil),
        _ => Value::Nil,
    };
    let buffer_display_table = buffer_setting(i, "buffer-display-table", bb.id);
    let standard_display_table = i
        .q_soft("standard-display-table")
        .and_then(|id| i.default_toplevel_value(id))
        .unwrap_or(Value::Nil);
    let display_tables = [
        window_display_table,
        buffer_display_table,
        standard_display_table,
    ]
    .into_iter()
    .filter(|table| crate::lisp::builtins::misc::is_char_table(i, table))
    .collect();
    WindowFaces {
        face_remappings,
        overlays,
        prop_faces,
        mouse_face,
        pointer,
        line_spacing_props,
        line_spacing_overlays,
        line_height_props,
        line_height_overlays,
        help_echo_props,
        help_echo_overlays,
        display_props,
        composition_props,
        display_tables,
        overlay_strings,
        invisible_overlays,
        invisible_props,
        invisibility_spec,
        region,
        show_trailing_whitespace: buffer_setting(i, "show-trailing-whitespace", bb.id).truthy(),
        trailing_whitespace: None,
    }
}

fn overlay_strings_at(faces: &WindowFaces, position: usize) -> Vec<String> {
    faces
        .overlay_strings
        .get(&position)
        .cloned()
        .unwrap_or_default()
}

fn help_echo_at(i: &Interp, faces: &WindowFaces, position: usize) -> Option<String> {
    let mut overlays: Vec<_> = faces
        .help_echo_overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| position >= overlay.start && position < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    for (_, index) in overlays.into_iter().rev() {
        let value = &faces.help_echo_overlays[index].1;
        if let Value::Str(text) = value {
            return Some(text.borrow().to_unicode_lossy());
        }
    }
    faces
        .help_echo_props
        .iter()
        .filter(|(start, end, _, _)| position >= *start && position < *end)
        .max_by_key(|(_, _, _, insertion)| insertion)
        .and_then(|(_, _, value, _)| match value {
            Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
            _ => None,
        })
}

fn line_spacing_pixels(value: &Value, default_line_height: usize) -> Option<usize> {
    match value {
        Value::Int(pixels) => Some(usize::try_from((*pixels).max(0)).unwrap_or(usize::MAX)),
        Value::Float(ratio) if ratio.is_finite() => {
            let pixels = (ratio.max(0.0) * default_line_height as f64).round();
            Some(pixels.min(usize::MAX as f64) as usize)
        }
        _ => None,
    }
}

fn line_spacing_at_pos(
    i: &Interp,
    faces: &WindowFaces,
    position: usize,
    default_spacing: usize,
    default_line_height: usize,
) -> usize {
    let mut overlays: Vec<_> = faces
        .line_spacing_overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| position >= overlay.start && position < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    let overlay_spacing = overlays
        .last()
        .and_then(|(_, index)| {
            line_spacing_pixels(
                &faces.line_spacing_overlays[*index].1,
                default_line_height,
            )
        });
    let property_spacing = faces
        .line_spacing_props
        .iter()
        .filter(|(start, end, _, _)| position >= *start && position < *end)
        .max_by_key(|(_, _, _, insertion)| *insertion)
        .and_then(|(_, _, value, _)| line_spacing_pixels(value, default_line_height));
    default_spacing.max(overlay_spacing.or(property_spacing).unwrap_or(default_spacing))
}

#[derive(Clone, Copy)]
enum LineHeightProperty {
    ContentsOnly,
    Minimum(usize),
    Total(usize),
}

fn height_spec_pixels(value: &Value, default_line_height: usize) -> Option<usize> {
    match value {
        Value::Int(height) if *height > 0 => usize::try_from(*height).ok(),
        Value::Float(ratio) if (**ratio).is_finite() && **ratio > 0.0 => {
            Some((**ratio * default_line_height as f64).round().min(usize::MAX as f64) as usize)
        }
        Value::Cons(_) => {
            let Value::Cons(cons) = value else {
                unreachable!()
            };
            let (face, ratio) = {
                let cons = cons.borrow();
                (cons.car.clone(), cons.cdr.clone())
            };
            let ratio = match ratio {
                Value::Nil => 1.0,
                Value::Int(ratio) => ratio as f64,
                Value::Float(ratio) => *ratio,
                _ => return None,
            };
            let _ = face;
            (ratio.is_finite() && ratio > 0.0).then(|| {
                (ratio * default_line_height as f64)
                    .round()
                    .min(usize::MAX as f64) as usize
            })
        }
        _ => None,
    }
}

fn line_height_property(value: &Value, default_line_height: usize) -> Option<LineHeightProperty> {
    if matches!(value, Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T) {
        return Some(LineHeightProperty::ContentsOnly);
    }
    if let Value::Cons(_) = value
        && let Ok(items) = value.list_to_vec()
        && items.len() == 2
        && let Some(total) = height_spec_pixels(&items[1], default_line_height)
    {
        return Some(LineHeightProperty::Total(total));
    }
    height_spec_pixels(value, default_line_height).map(LineHeightProperty::Minimum)
}

fn line_height_at_pos(
    i: &Interp,
    faces: &WindowFaces,
    position: usize,
    default_line_height: usize,
) -> Option<LineHeightProperty> {
    let mut overlays: Vec<_> = faces
        .line_height_overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| position >= overlay.start && position < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    let overlay_value = overlays
        .last()
        .map(|(_, index)| &faces.line_height_overlays[*index].1);
    let property_value = faces
        .line_height_props
        .iter()
        .filter(|(start, end, _, _)| position >= *start && position < *end)
        .max_by_key(|(_, _, _, insertion)| *insertion)
        .map(|(_, _, value, _)| value);
    overlay_value
        .or(property_value)
        .and_then(|value| line_height_property(value, default_line_height))
}

fn image_property(i: &Interp, plist: &Value, name: &str) -> Value {
    i.q_soft(name)
        .map(|property| crate::lisp::eval::plist_get(plist, property))
        .unwrap_or(Value::Nil)
}

fn image_dimension(value: Value) -> Option<usize> {
    match value {
        Value::Int(n) if n > 0 => usize::try_from(n).ok(),
        _ => None,
    }
}

fn image_file_for_buffer(i: &Interp, buffer_id: usize, file: &EmacsStr) -> String {
    let file = file.to_unicode_lossy();
    let path = std::path::Path::new(&file);
    if path.is_absolute() {
        return file;
    }
    let default_directory = i
        .q_soft("default-directory")
        .and_then(|id| {
            i.buffer_local_toplevel_value(id, buffer_id)
                .or_else(|| i.default_toplevel_value(id))
        })
        .and_then(|value| match value {
            Value::Str(directory) => Some(directory.borrow().to_unicode_lossy()),
            _ => None,
        });
    match default_directory {
        Some(directory) => std::path::Path::new(&directory)
            .join(path)
            .to_string_lossy()
            .into_owned(),
        None => file,
    }
}

fn image_display_replacement(
    i: &Interp,
    value: &Value,
    buffer_id: usize,
) -> Option<DisplayReplacement> {
    let Value::Cons(cell) = value else {
        return None;
    };
    let (head, plist) = {
        let cell = cell.borrow();
        (cell.car.clone(), cell.cdr.clone())
    };
    if !matches!(head, Value::Sym(id) if i.symbol_name(id) == "image") {
        return None;
    }

    let width = image_dimension(image_property(i, &plist, ":width"));
    let height = image_dimension(image_property(i, &plist, ":height"));
    let file = image_property(i, &plist, ":file");
    if let Value::Str(file) = file {
        return Some(DisplayReplacement::Image {
            source: GridImageSource::File(image_file_for_buffer(
                i,
                buffer_id,
                &file.borrow(),
            )),
            width,
            height,
        });
    }

    let data = image_property(i, &plist, ":data");
    let Value::Str(data) = data else {
        return None;
    };
    let Value::Sym(format) = image_property(i, &plist, ":type") else {
        return None;
    };
    let format = i.symbol_name(format).to_lowercase();
    if !matches!(
        format.as_str(),
        "png" | "jpeg" | "jpg" | "webp" | "gif" | "svg" | "bmp" | "tiff" | "tif"
    ) {
        return None;
    }
    let bytes = data
        .borrow()
        .chars()
        .map(|character| {
            character
                .byte8()
                .or_else(|| u8::try_from(character.code()).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    Some(DisplayReplacement::Image {
        source: GridImageSource::Data {
            format,
            bytes: bytes.into(),
        },
        width,
        height,
    })
}

fn display_value_replacement(
    i: &Interp,
    value: &Value,
    buffer_id: usize,
) -> Option<DisplayReplacement> {
    match value {
        Value::Str(string) => Some(DisplayReplacement::Text(
            string.borrow().to_unicode_lossy(),
        )),
        _ => image_display_replacement(i, value, buffer_id),
    }
}

fn display_replacement_at(
    faces: &WindowFaces,
    position: usize,
) -> Option<DisplayReplacement> {
    fn from_property_run(
        properties: &[(usize, usize, Option<DisplayReplacement>, usize)],
        position: usize,
    ) -> Option<DisplayReplacement> {
        let (start, _, replacement, _) = properties
            .iter()
            .filter(|(start, end, _, _)| position >= *start && position < *end)
            .max_by_key(|(_, _, _, insertion)| insertion)?;
        let replacement = replacement.as_ref()?;
        if position == *start {
            Some(replacement.clone())
        } else {
            Some(DisplayReplacement::Hidden)
        }
    }

    from_property_run(&faces.display_props, position)
        .or_else(|| from_property_run(&faces.composition_props, position))
}

fn composition_display_replacement(value: &Value) -> Option<DisplayReplacement> {
    let Value::Cons(outer) = value else {
        return None;
    };
    let composition_spec = outer.borrow().car.clone();
    let (width, components) = match composition_spec {
        Value::Int(width) => (usize::try_from(width).ok()?, Value::Nil),
        Value::Cons(spec) => {
            let spec = spec.borrow();
            let Value::Int(width) = &spec.car else {
                return None;
            };
            (usize::try_from(*width).ok()?, spec.cdr.clone())
        }
        _ => return None,
    };
    if width == 0 {
        return None;
    }
    if components.is_nil() {
        return Some(DisplayReplacement::CompositionDefault(width));
    }
    let mut text = String::new();
    fn append_character(text: &mut String, value: &Value) {
        if let Value::Int(code) = value
            && let Ok(code) = u32::try_from(*code)
            && let Some(character) = EmacsChar::new(code).and_then(EmacsChar::unicode)
        {
            text.push(character);
        }
    }
    match components {
        Value::Int(code) => {
            let code = u32::try_from(code).ok()?;
            text.push(EmacsChar::new(code)?.unicode().unwrap_or('\u{fffd}'));
        }
        Value::Str(string) => text.push_str(&string.borrow().to_unicode_lossy()),
        Value::Vec(vector) => {
            for component in vector.borrow().iter() {
                append_character(&mut text, component);
            }
        }
        Value::Cons(_) => {
            let components = components.list_to_vec().ok()?;
            for component in &components {
                append_character(&mut text, component);
            }
        }
        _ => return None,
    }
    (!text.is_empty()).then_some(DisplayReplacement::Text(text))
}

fn raw_byte_text(i: &Interp, buffer_id: usize, byte: u8) -> String {
    if buffer_setting(i, "display-raw-bytes-as-hex", buffer_id).truthy() {
        format!("\\x{byte:02x}")
    } else {
        format!("\\{byte:03o}")
    }
}

fn emacs_character_display_text(i: &Interp, buffer_id: usize, character: EmacsChar) -> String {
    if let Some(byte) = character.byte8() {
        raw_byte_text(i, buffer_id, byte)
    } else {
        character.unicode().unwrap_or('\u{fffd}').to_string()
    }
}

fn display_table_text(i: &Interp, buffer_id: usize, value: &Value) -> Option<String> {
    fn append_character(i: &Interp, buffer_id: usize, text: &mut String, value: &Value) {
        if let Value::Int(code) = value
            && let Ok(code) = u32::try_from(*code)
            && let Some(character) = EmacsChar::new(code)
        {
            text.push_str(&emacs_character_display_text(i, buffer_id, character));
        }
    }
    match value {
        Value::Int(code) => {
            let code = u32::try_from(*code).ok()?;
            Some(emacs_character_display_text(i, buffer_id, EmacsChar::new(code)?))
        }
        Value::Str(string) => Some(
            string
                .borrow()
                .chars()
                .map(|character| emacs_character_display_text(i, buffer_id, character))
                .collect(),
        ),
        Value::Vec(vector) => {
            let mut text = String::new();
            for character in vector.borrow().iter() {
                append_character(i, buffer_id, &mut text, character);
            }
            Some(text)
        }
        Value::Cons(_) => {
            let characters = value.list_to_vec().ok()?;
            let mut text = String::new();
            for character in &characters {
                append_character(i, buffer_id, &mut text, character);
            }
            Some(text)
        }
        _ => None,
    }
}

fn display_table_replacement(
    i: &Interp,
    buffer_id: usize,
    faces: &WindowFaces,
    character: EmacsChar,
) -> Option<DisplayReplacement> {
    for table in &faces.display_tables {
        let replacement =
            crate::lisp::builtins::misc::char_table_ref(i, table, character.code() as usize);
        if replacement.is_nil() {
            continue;
        }
        if let Some(text) = display_table_text(i, buffer_id, &replacement) {
            return Some(DisplayReplacement::Text(text));
        }
    }
    None
}

fn raw_byte_replacement(
    i: &Interp,
    buffer_id: usize,
    character: EmacsChar,
) -> Option<DisplayReplacement> {
    character
        .byte8()
        .map(|byte| DisplayReplacement::Text(raw_byte_text(i, buffer_id, byte)))
}

fn glyphless_display_replacement(
    i: &Interp,
    buffer_id: usize,
    character: EmacsChar,
    graphical: bool,
    glyphless_style: CellStyle,
) -> Option<DisplayReplacement> {
    let table = buffer_setting(i, "glyphless-char-display", buffer_id);
    if !crate::lisp::builtins::misc::is_char_table(i, &table) {
        return None;
    }
    let value = crate::lisp::builtins::misc::char_table_ref(
        i,
        &table,
        character.code() as usize,
    );
    if value.is_nil() {
        return None;
    }

    let value = match value {
        Value::Cons(pair) => {
            let pair = pair.borrow();
            if graphical {
                pair.car.clone()
            } else {
                pair.cdr.clone()
            }
        }
        value => value,
    };
    if value.is_nil() {
        return None;
    }

    let (text, boxed) = match value {
        Value::Str(string) => (string.borrow().to_unicode_lossy(), true),
        Value::Sym(method) => match i.symbol_name(method).as_str() {
            "zero-width" => return Some(DisplayReplacement::Hidden),
            "thin-space" => (" ".to_owned(), false),
            "empty-box" => (" ".to_owned(), true),
            "hex-code" => (format!("{:X}", character.code()), true),
            "acronym" | "bidi-control" => {
                let acronym = i
                    .q_soft("char-acronym-table")
                    .map(|id| i.symbol_value(id))
                    .map(|table| {
                        crate::lisp::builtins::misc::char_table_ref(
                            i,
                            &table,
                            character.code() as usize,
                        )
                    })
                    .and_then(|value| match value {
                        Value::Str(string) => Some(string.borrow().to_unicode_lossy()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "UNK".to_owned());
                (acronym, true)
            }
            _ => return None,
        },
        _ => return None,
    };

    if graphical {
        let mut style = glyphless_style;
        if boxed && style.box_style.is_none() {
            style.box_style = Some(BoxStyle {
                vertical_width: 1,
                horizontal_width: 1,
                color: style.fg,
                kind: BoxStyleKind::Flat,
            });
        }
        Some(DisplayReplacement::GlyphlessText(text, style))
    } else if boxed {
        Some(DisplayReplacement::Text(format!("[{text}]")))
    } else {
        Some(DisplayReplacement::Text(text))
    }
}

fn nobreak_char_face(nobreak_display: &Value, character: EmacsChar) -> Option<&'static str> {
    if nobreak_display.is_nil() {
        return None;
    }
    let code = character.code();
    let space = matches!(
        code,
        0x00a0 | 0x1680 | 0x2000..=0x200a | 0x202f | 0x205f | 0x3000
    );
    let hyphen = matches!(code, 0x00ad | 0x2010 | 0x2011);
    if !space && !hyphen {
        return None;
    }
    if matches!(nobreak_display, Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T)
    {
        Some(if space {
            "nobreak-space"
        } else {
            "nobreak-hyphen"
        })
    } else {
        Some("escape-glyph")
    }
}

fn nobreak_char_replacement(
    nobreak_display: &Value,
    character: EmacsChar,
) -> Option<DisplayReplacement> {
    if nobreak_display.is_nil() {
        return None;
    }
    let code = character.code();
    let space = matches!(
        code,
        0x00a0 | 0x1680 | 0x2000..=0x200a | 0x202f | 0x205f | 0x3000
    );
    let hyphen = matches!(code, 0x00ad | 0x2010 | 0x2011);
    if !space && !hyphen {
        return None;
    }
    let escaped = !matches!(
        nobreak_display,
        Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T
    );
    if escaped {
        Some(DisplayReplacement::Text(if space {
            "\\ ".to_owned()
        } else {
            "\\-".to_owned()
        }))
    } else if code == 0x00ad {
        Some(DisplayReplacement::Text("-".to_owned()))
    } else {
        None
    }
}

fn ctl_arrow_replacement(character: EmacsChar, ctl_arrow: bool) -> Option<DisplayReplacement> {
    let code = character.code();
    if !ctl_arrow || (code >= 0x20 && code != 0x7f) || matches!(code, 0x09 | 0x0a) {
        return None;
    }
    let caret = char::from_u32(code ^ 0x40)?;
    Some(DisplayReplacement::Text(format!("^{caret}")))
}

fn invisibility_for_property(property: &Value, spec: &Value) -> Visibility {
    if matches!(spec, Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T) {
        return if property.is_nil() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    fn match_spec(property: &Value, spec: &Value) -> Option<Visibility> {
        let mut tail = spec.clone();
        while let Value::Cons(cell) = tail {
            let (entry, next) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            if crate::lisp::builtins::eq_values(property, &entry) {
                return Some(Visibility::Hidden);
            }
            if let Value::Cons(pair) = &entry {
                let pair = pair.borrow();
                if crate::lisp::builtins::eq_values(property, &pair.car) {
                    return Some(if pair.cdr.is_nil() {
                        Visibility::Hidden
                    } else {
                        Visibility::Ellipsis
                    });
                }
            }
            tail = next;
        }
        None
    }

    if let Some(visibility) = match_spec(property, spec) {
        return visibility;
    }
    let mut property_tail = property.clone();
    while let Value::Cons(cell) = property_tail {
        let (element, next) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        if let Some(visibility) = match_spec(&element, spec) {
            return visibility;
        }
        property_tail = next;
    }
    Visibility::Visible
}

fn visibility_at_pos(i: &Interp, faces: &WindowFaces, pos: usize) -> Visibility {
    let mut overlays: Vec<_> = faces
        .invisible_overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| pos >= overlay.start && pos < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    if let Some((_, index)) = overlays.into_iter().next_back() {
        return invisibility_for_property(
            &faces.invisible_overlays[index].1,
            &faces.invisibility_spec,
        );
    }
    if let Some((_, _, property)) = faces
        .invisible_props
        .iter()
        .rev()
        .find(|(start, end, _)| pos >= *start && pos < *end)
    {
        return invisibility_for_property(property, &faces.invisibility_spec);
    }
    Visibility::Visible
}

/// The merged face style for buffer position POS under WF.  GNU
/// `compute_char_face' order: `default' seeds, the active `region'
/// face, then text-property faces (insertion order — last write
/// wins), then overlays in ascending priority.
fn style_at_pos(i: &Interp, ctx: &mut FaceCtx, wf: &WindowFaces, pos: usize) -> CellStyle {
    style_at_pos_with_named_face(i, ctx, wf, pos, None)
}

fn echo_message_style(
    i: &Interp,
    ctx: &mut FaceCtx,
    properties: &[(usize, usize, Vec<Value>)],
    position: usize,
) -> CellStyle {
    let face = ctx.face;
    let font_lock_face = ctx.font_lock_face;
    let mut mix = ctx.builtin(i, "default", 0);
    for (_, _, plist) in properties
        .iter()
        .filter(|(start, end, _)| position >= *start && position < *end)
    {
        for pair in plist.chunks_exact(2) {
            let Some(property) = i.sym_id(&pair[0]) else {
                continue;
            };
            if property == face || property == font_lock_face {
                mix.merge(&ctx.face_value(i, &pair[1], 0));
            }
        }
    }
    mix.finish()
}

fn style_at_pos_with_named_face(
    i: &Interp,
    ctx: &mut FaceCtx,
    wf: &WindowFaces,
    pos: usize,
    extra_face: Option<&str>,
) -> CellStyle {
    let mut mix = remapped_named_face(i, ctx, wf, "default", 0);
    if let Some((rs, re)) = wf.region {
        if pos >= rs && pos < re {
            mix.merge(&remapped_named_face(i, ctx, wf, "region", 0));
        }
    }
    for (s, e, v, _) in &wf.prop_faces {
        if pos >= *s && pos < *e {
            let v = v.clone();
            mix.merge(&face_value_with_remapping(
                i,
                ctx,
                wf,
                &v,
                0,
                &mut Vec::new(),
            ));
        }
    }
    let mut overlays: Vec<_> = wf
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| pos >= overlay.start && pos < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    for (_, index) in overlays {
        let face = wf.overlays[index].1.clone();
        mix.merge(&face_value_with_remapping(
            i,
            ctx,
            wf,
            &face,
            0,
            &mut Vec::new(),
        ));
    }
    if wf.show_trailing_whitespace
        && wf
            .trailing_whitespace
            .as_ref()
            .is_some_and(|range| pos >= range.start && pos < range.end)
    {
        mix.merge(&remapped_named_face(i, ctx, wf, "trailing-whitespace", 0));
    }
    if let Some((start, end, face)) = &wf.mouse_face
        && pos >= *start
        && pos < *end
    {
        let mut mouse_face = face_value_with_remapping(i, ctx, wf, face, 0, &mut Vec::new());
        // GNU keeps the underlying text metrics while applying mouse-face.
        mouse_face.font_family = None;
        mouse_face.font_weight = None;
        mouse_face.font_width = None;
        mouse_face.font_height = None;
        mouse_face.bold = None;
        mouse_face.italic = None;
        mix.merge(&mouse_face);
    }
    if let Some(face) = extra_face {
        mix.merge(&remapped_named_face(i, ctx, wf, face, 0));
    }
    mix.finish()
}

/// Return the face specs in a `face-remap-add-relative' entry in the
/// order the renderer must merge them. The helper stores relative specs
/// before the base face, while the base has the lowest priority.
fn face_remapping_specs(
    i: &Interp,
    remapping: &Value,
    face: crate::lisp::value::SymId,
) -> (Vec<Value>, bool) {
    let Ok(specs) = remapping.list_to_vec() else {
        return (vec![remapping.clone()], false);
    };
    let is_plist = specs
        .first()
        .and_then(|value| i.sym_id(value))
        .is_some_and(|symbol| i.symbol_name(symbol).starts_with(':'));
    if is_plist {
        return (vec![remapping.clone()], false);
    }
    let is_relative_sequence =
        specs.len() > 1 && matches!(specs.last(), Some(Value::Sym(symbol)) if *symbol == face);
    if is_relative_sequence {
        (specs, true)
    } else {
        (vec![remapping.clone()], false)
    }
}

fn remapped_named_face(
    i: &Interp,
    ctx: &mut FaceCtx,
    faces: &WindowFaces,
    name: &str,
    depth: u32,
) -> StyleMix {
    if let Some(face) = i.intern_soft(name) {
        face_value_with_remapping(i, ctx, faces, &Value::Sym(face), depth, &mut Vec::new())
    } else {
        ctx.builtin(i, name, depth)
    }
}

/// Resolve a face spec while replacing named faces through the current
/// buffer's face-remapping-alist. A remapped face can refer to itself
/// as its base; that reference must use the normal face definition.
fn face_value_with_remapping(
    i: &Interp,
    ctx: &mut FaceCtx,
    faces: &WindowFaces,
    value: &Value,
    depth: u32,
    remapping_stack: &mut Vec<crate::lisp::value::SymId>,
) -> StyleMix {
    if depth >= 16 {
        return StyleMix::default();
    }
    let face = crate::editor::face_symbol(i, value);
    if let Some(face) = face {
        if remapping_stack.contains(&face) {
            return ctx.face_value(i, value, depth);
        }
        if let Some(remapping) = faces
            .face_remappings
            .iter()
            .find(|(candidate, _)| *candidate == face)
            .map(|(_, remapping)| remapping.clone())
        {
            let (specs, reverse) = face_remapping_specs(i, &remapping, face);
            let mut mix = StyleMix::default();
            remapping_stack.push(face);
            if reverse {
                for spec in specs.iter().rev() {
                    mix.merge(&face_value_with_remapping(
                        i,
                        ctx,
                        faces,
                        spec,
                        depth + 1,
                        remapping_stack,
                    ));
                }
            } else {
                for spec in &specs {
                    mix.merge(&face_value_with_remapping(
                        i,
                        ctx,
                        faces,
                        spec,
                        depth + 1,
                        remapping_stack,
                    ));
                }
            }
            remapping_stack.pop();
            return mix;
        }
    }

    if let Value::Cons(_) = value {
        let Ok(items) = value.list_to_vec() else {
            return ctx.face_value(i, value, depth);
        };
        let is_plist = items
            .first()
            .and_then(|item| i.sym_id(item))
            .is_some_and(|symbol| i.symbol_name(symbol).starts_with(':'));
        if is_plist {
            let inherit = crate::lisp::eval::plist_get(value, ctx.inherit);
            let mut mix = StyleMix::default();
            if inherit.truthy() {
                mix.merge(&face_value_with_remapping(
                    i,
                    ctx,
                    faces,
                    &inherit,
                    depth + 1,
                    remapping_stack,
                ));
            }
            mix.merge(&ctx.attrs(i, value));
            return mix;
        }
        let mut mix = StyleMix::default();
        for spec in &items {
            mix.merge(&face_value_with_remapping(
                i,
                ctx,
                faces,
                spec,
                depth + 1,
                remapping_stack,
            ));
        }
        return mix;
    }
    ctx.face_value(i, value, depth)
}

fn grid_pointer_shape(i: &Interp, value: Option<&Value>) -> GridPointerShape {
    let Some(Value::Sym(symbol)) = value else {
        return GridPointerShape::Text;
    };
    match i.symbol_name(*symbol).as_str() {
        "text" | "nil" => GridPointerShape::Text,
        "hand" => GridPointerShape::Hand,
        "hdrag" => GridPointerShape::ResizeHorizontal,
        "nhdrag" => GridPointerShape::ResizeVertical,
        "arrow" | "vdrag" | "modeline" => GridPointerShape::Arrow,
        _ => GridPointerShape::Arrow,
    }
}

/// Compute the screen contents for the selected frame at `width` ×
/// `height` text cells.  Like GNU's `redisplay_window' it also keeps
/// each window's point on screen: a window's `start' is scrolled the
/// minimal amount to show point, and recentered when point sits more
/// than a window-height away (scrolling can't reach it cheaply).
pub fn render_grid(i: &Interp, width: usize, height: usize) -> Grid {
    render_grid_internal(i, width, height, false, None, None, None)
}

/// Render a GUI input grid, reusing TEXT from a previous snapshot when
/// the same buffer has not changed.
pub fn render_grid_with_input_cached(
    i: &Interp,
    width: usize,
    height: usize,
    cached: Option<&GridInput>,
) -> Grid {
    render_grid_internal(i, width, height, true, cached, None, None)
}

/// Render a GUI input grid with mouse-face highlighting at MOUSE_POSITION.
pub fn render_grid_with_mouse_position_cached(
    i: &Interp,
    width: usize,
    height: usize,
    cached: Option<&GridInput>,
    mouse_position: Option<MousePosition>,
) -> Grid {
    render_grid_internal(i, width, height, true, cached, None, mouse_position)
}

/// Render FRAME without changing the interpreter's selected frame. This is
/// used by multi-window front-ends to keep every native frame up to date.
pub fn render_frame_grid_with_input_cached(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    width: usize,
    height: usize,
    cached: Option<&GridInput>,
) -> Grid {
    let include_input = i
        .selected_frame
        .as_ref()
        .is_some_and(|selected| std::rc::Rc::ptr_eq(selected, frame));
    render_grid_internal(i, width, height, include_input, cached, Some(frame), None)
}

/// Render FRAME with mouse-face highlighting at MOUSE_POSITION without
/// changing the interpreter's selected frame.
pub fn render_frame_grid_with_mouse_position_cached(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    width: usize,
    height: usize,
    cached: Option<&GridInput>,
    mouse_position: Option<MousePosition>,
) -> Grid {
    let include_input = i
        .selected_frame
        .as_ref()
        .is_some_and(|selected| std::rc::Rc::ptr_eq(selected, frame));
    render_grid_internal(
        i,
        width,
        height,
        include_input,
        cached,
        Some(frame),
        mouse_position,
    )
}

fn minibuffer_display_rows(
    characters: &[EmacsChar],
    width: usize,
    tab_width: usize,
    wrap_lines: bool,
    ctl_arrow: bool,
) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut line_start = 0;
    for line_end in 0..=characters.len() {
        if line_end != characters.len() && characters[line_end].unicode() != Some('\n') {
            continue;
        }
        let line: EmacsText = characters[line_start..line_end].iter().copied().collect();
        let ranges = if wrap_lines {
            wrap_ranges(&line, width, tab_width, ctl_arrow)
        } else {
            vec![0..line.len()]
        };
        rows.extend(
            ranges
                .into_iter()
                .map(|range| line_start + range.start..line_start + range.end),
        );
        line_start = line_end.saturating_add(1);
    }
    rows
}

fn minibuffer_display_height(
    i: &Interp,
    current_height: usize,
    requested_rows: usize,
    frame_height: usize,
    chrome_rows: usize,
) -> usize {
    let available = frame_height.saturating_sub(chrome_rows);
    if available == 0 {
        return 0;
    }
    let current_height = current_height.max(1).min(available);
    let resize_mode = i
        .q_soft("resize-mini-windows")
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    if resize_mode.is_nil() {
        return current_height;
    }

    let configured_max = i
        .q_soft("max-mini-window-height")
        .map(|id| i.symbol_value(id))
        .map_or(1, |value| match value {
            Value::Int(lines) => usize::try_from(lines).unwrap_or(1),
            Value::Float(ratio) if ratio.is_finite() => (ratio.max(0.0) * frame_height as f64)
                .ceil()
                .min(usize::MAX as f64)
                as usize,
            _ => 1,
        });
    let grow_capacity = available.saturating_sub(1).max(1).min(available);
    let maximum = configured_max.max(1).min(grow_capacity);
    let requested = requested_rows.max(1).min(maximum);
    let grow_only = i.q_soft("grow-only");
    if matches!(resize_mode, Value::Sym(id) if Some(id) == grow_only) {
        current_height.max(requested).min(available)
    } else {
        requested
    }
}

fn render_grid_internal(
    i: &Interp,
    width: usize,
    height: usize,
    include_input: bool,
    cached_input: Option<&GridInput>,
    frame_override: Option<&crate::editor::FrameRef>,
    mouse_position: Option<MousePosition>,
) -> Grid {
    let mut rows = vec![String::new(); height];
    let mut fctx = FaceCtx::new(i);
    let default_style = fctx.builtin(i, "default", 0).finish();
    let default_bg = default_style.bg;
    let mut attrs = vec![vec![default_style; width]; height];
    let mut row_heights = vec![19; height];
    let mut default_line_height = 19;
    let mut default_row_height = 19;
    let mut char_attrs = vec![Vec::new(); height];
    let mut images = Vec::new();
    let mut pending_compositions = Vec::<(usize, usize, Vec<char>, CellStyle)>::new();
    let mut fill_column_indicators = Vec::new();
    let menus = render_menu_bar(i);
    let mut tool_bar_items = Vec::new();
    let mut tab_bar_items = Vec::new();
    let mut help_echoes = Vec::new();
    let tooltip = i.gui_tip.clone();
    let mut popup_menu_items = Vec::new();
    let mut scroll_bars = Vec::new();
    let mut bottom_dividers = Vec::new();
    let mut mode_lines = Vec::new();
    let mut vscrolls = Vec::new();
    let mut mode_rows = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let mut positioned_cursors = Vec::new();
    let mut input_screen_lefts = Vec::new();
    let mut cursor_index = None;
    let mut hover_cursor = None;
    let (blink_cursor, blink_interval) = blink_cursor_settings(i);
    let mut input = if include_input {
        input_snapshot(i, cached_input)
    } else {
        None
    };
    let frame = match frame_override.cloned().or_else(|| i.selected_frame.clone()) {
        Some(f) => f,
        None => {
            return Grid {
                title: "Remacs".to_owned(),
                font_families: fctx.font_families,
                default_style,
                rows,
                images,
                fill_column_indicators,
                stipples: fctx.stipples,
                menus,
                tool_bar_items,
                tab_bar_items,
                help_echoes,
                tooltip,
                popup_menu_items,
                scroll_bars,
                bottom_dividers,
                mode_lines,
                vscrolls,
                compositions: Vec::new(),
                attrs,
                char_attrs,
                mode_rows,
                cursor: cursor_pos,
                cursors: Vec::new(),
                cursor_index,
                blink_cursor,
                blink_interval,
                default_bg,
                row_heights,
                default_line_height,
                default_row_height,
                hover_cursor,
                pointer_visible: true,
                input,
            };
        }
    };
    let fb = frame.borrow();
    let title = frame_title(i, &fb);
    let cursor_color = frame_cursor_color(i, &mut fctx, &fb);
    let graphical = crate::editor::frame_is_graphical(i, &frame);
    let frame_char_width = i
        .q_soft("char-width")
        .map(|id| crate::lisp::eval::plist_get(&fb.params, id))
        .and_then(|value| match value {
            Value::Int(width) if width > 0 => usize::try_from(width).ok(),
            _ => None,
        })
        .unwrap_or(8);
    let frame_char_height = crate::editor::frame_char_height(i, &frame).max(1);
    row_heights.fill(frame_char_height);
    default_line_height = frame_char_height;
    default_row_height = frame_char_height;
    let bottom_divider_width = if graphical {
        crate::editor::frame_parameter_positive_size(i, &frame, "bottom-divider-width", 0)
    } else {
        0
    };
    let bottom_divider_styles = (bottom_divider_width > 0).then(|| {
        (
            fctx.builtin(i, "window-divider", 0).finish(),
            fctx.builtin(i, "window-divider-first-pixel", 0).finish(),
            fctx.builtin(i, "window-divider-last-pixel", 0).finish(),
        )
    });
    let frame_line_spacing = i
        .q_soft("line-spacing")
        .map(|property| crate::lisp::eval::plist_get(&fb.params, property))
        .unwrap_or(Value::Nil);
    let default_fringe_left = if graphical {
        frame_fringe_pixels(i, &fb.params, "left-fringe")
    } else {
        0
    };
    let default_fringe_right = if graphical {
        frame_fringe_pixels(i, &fb.params, "right-fringe")
    } else {
        0
    };
    let mini_buf = fb
        .minibuffer
        .as_ref()
        .and_then(|w| i.buffers.get(w.borrow().buffer));
    let mini_text = mini_buf
        .as_ref()
        .map(|buffer| buffer.borrow().text.text())
        .unwrap_or_default();
    let (mini_message, mini_styled) = if mini_text.is_empty() {
        (i.echo_message.clone(), None)
    } else {
        (mini_text, mini_buf.as_ref())
    };
    let mini_tab_width = mini_buf
        .as_ref()
        .map(|buffer| buffer_tab_width(i, buffer.borrow().id))
        .unwrap_or(8);
    let mini_wrap_lines = if mini_styled.is_none() {
        !i.q_soft("message-truncate-lines")
            .is_some_and(|symbol| i.symbol_value(symbol).truthy())
    } else {
        mini_buf.as_ref().is_none_or(|buffer| {
            !buffer_setting(i, "truncate-lines", buffer.borrow().id).truthy()
        })
    };
    let mini_ctl_arrow = mini_buf
        .as_ref()
        .is_some_and(|buffer| buffer_setting(i, "ctl-arrow", buffer.borrow().id).truthy());
    let mini_characters: Vec<_> = mini_message.chars().collect();
    let mini_rows = minibuffer_display_rows(
        &mini_characters,
        width,
        mini_tab_width,
        mini_wrap_lines,
        mini_ctl_arrow,
    );
    let tool_bar_rows = if graphical {
        crate::editor::frame_parameter_positive_size(i, &frame, "tool-bar-lines", 0)
            .min(height.saturating_sub(1))
    } else {
        0
    };
    let tab_bar_rows = if graphical {
        crate::editor::frame_parameter_positive_size(i, &frame, "tab-bar-lines", 0)
            .min(height.saturating_sub(1 + tool_bar_rows))
    } else {
        0
    };
    let chrome_rows = tool_bar_rows.saturating_add(tab_bar_rows);
    let current_minibuffer_height = fb
        .minibuffer
        .as_ref()
        .map(|window| window.borrow().height)
        .unwrap_or(1);
    let mini_height = minibuffer_display_height(
        i,
        current_minibuffer_height,
        mini_rows.len(),
        height,
        chrome_rows,
    );
    let body_height = height
        .saturating_sub(mini_height)
        .saturating_sub(chrome_rows);
    let (root_left, root_top, root_width, root_height) = fb
        .root
        .as_ref()
        .map(|root| {
            let root = root.borrow();
            (root.left, root.top, root.width, root.height)
        })
        .unwrap_or((0, 0, width, body_height));
    let scale_x = if root_width > 0 {
        width as f64 / root_width as f64
    } else {
        1.0
    };
    let scale_y = if root_height > 0 {
        body_height as f64 / root_height as f64
    } else {
        1.0
    };
    let scale_edge = |position: usize, origin: usize, scale: f64, limit: usize| {
        ((position.saturating_sub(origin) as f64 * scale).round() as usize).min(limit)
    };
    let mut screen_rows: Vec<Vec<ScreenCell>> = (0..height)
        .map(|_| (0..width).map(|_| ScreenCell::new(default_style)).collect())
        .collect();
    if tab_bar_rows > 0 {
        tab_bar_items = render_tab_bar(
            i,
            &fb.params,
            width,
            tab_bar_rows,
            0,
            default_style,
            &mut fctx,
            &mut screen_rows,
        );
    }
    if tool_bar_rows > 0 {
        tool_bar_items = render_tool_bar(
            i,
            width,
            tool_bar_rows,
            tab_bar_rows,
            default_style,
            &mut fctx,
            &mut screen_rows,
        );
    }
    let mut selected_cursor: Option<(usize, usize, usize, usize)> = None;

    for w in &fb.windows {
        let (
            window_id,
            minibuffer_window,
            buf_id,
            start,
            hscroll,
            vscroll,
            raw_left,
            raw_top,
            raw_width,
            raw_height,
            margins,
            fringes,
        ) = {
            let wb = w.borrow();
            (
                wb.id,
                wb.minibuffer,
                wb.buffer,
                wb.start,
                wb.hscroll,
                wb.vscroll,
                wb.left,
                wb.top,
                wb.width,
                wb.height,
                wb.margins,
                wb.fringes,
            )
        };
        let raw_window_left = scale_edge(raw_left, root_left, scale_x, width);
        let raw_window_top = scale_edge(raw_top, root_top, scale_y, body_height)
            .saturating_add(chrome_rows);
        let raw_window_right = scale_edge(
            raw_left.saturating_add(raw_width),
            root_left,
            scale_x,
            width,
        );
        let raw_window_bottom = scale_edge(
            raw_top.saturating_add(raw_height),
            root_top,
            scale_y,
            body_height,
        )
        .saturating_add(chrome_rows);
        let raw_window_width = raw_window_right.saturating_sub(raw_window_left);
        let raw_window_height = raw_window_bottom
            .saturating_sub(raw_window_top)
            .max(2)
            .min(body_height.saturating_sub(raw_window_top));
        if !minibuffer_window
            && raw_window_width > 0
            && raw_window_bottom > 0
            && let Some((style, first_pixel_style, last_pixel_style)) = bottom_divider_styles
        {
            bottom_dividers.push(GridBottomDivider {
                row_boundary: raw_window_bottom,
                left: raw_window_left,
                width: raw_window_width,
                thickness: bottom_divider_width,
                style,
                first_pixel_style,
                last_pixel_style,
            });
        }
        let (
            scroll_width,
            vertical_scroll_side,
            scroll_height,
            horizontal_scroll_side,
            _,
        ) = crate::editor::winxtra::window_scroll_bar_settings(i, w);
        let vertical_scroll_cells = if vertical_scroll_side.is_some() {
            scroll_width
                .unwrap_or(0)
                .div_ceil(frame_char_width)
                .min(raw_window_width)
        } else {
            0
        };
        let horizontal_scroll_rows = if horizontal_scroll_side.is_some() {
            scroll_height
                .unwrap_or(0)
                .div_ceil(frame_char_height)
                .min(raw_window_height)
        } else {
            0
        };
        let left_scroll_cells = if vertical_scroll_side == Some(true) {
            vertical_scroll_cells
        } else {
            0
        };
        let top_scroll_rows = if horizontal_scroll_side == Some(true) {
            horizontal_scroll_rows
        } else {
            0
        };
        let window_left = raw_window_left.saturating_add(left_scroll_cells);
        let window_top = raw_window_top.saturating_add(top_scroll_rows);
        let window_width = raw_window_width.saturating_sub(vertical_scroll_cells);
        let window_height = raw_window_height.saturating_sub(horizontal_scroll_rows);
        let (fringe_left_pixels, fringe_right_pixels) = if graphical {
            fringes
                .map(|(left, right, _, _)| {
                    (
                        left.unwrap_or(default_fringe_left),
                        right.unwrap_or(default_fringe_right),
                    )
                })
                .unwrap_or((default_fringe_left, default_fringe_right))
        } else {
            (0, 0)
        };
        let fringe_left_cells = fringe_pixels_to_cells(fringe_left_pixels, frame_char_width)
            .min(window_width.saturating_sub(1));
        let fringe_right_cells = fringe_pixels_to_cells(fringe_right_pixels, frame_char_width)
            .min(window_width.saturating_sub(fringe_left_cells.saturating_add(1)));
        let right_fringe_left = window_left
            .saturating_add(window_width.saturating_sub(fringe_right_cells));
        let width_after_fringes =
            window_width.saturating_sub(fringe_left_cells.saturating_add(fringe_right_cells));
        let margin_left = margins.0.min(width_after_fringes.saturating_sub(1));
        let margin_right = margins
            .1
            .min(width_after_fringes.saturating_sub(margin_left.saturating_add(1)));
        let content_left_with_margin = window_left
            .saturating_add(fringe_left_cells)
            .saturating_add(margin_left);
        let content_width_with_margins =
            width_after_fringes.saturating_sub(margin_left.saturating_add(margin_right));
        if window_width == 0 || window_height == 0 {
            continue;
        }
        let selected_window = std::rc::Rc::ptr_eq(w, &fb.selected);
        let cursor_style = cursor_type_for_window(i, w, buf_id, selected_window);
        let stretch_cursor = buffer_setting(i, "x-stretch-cursor", buf_id).truthy();
        let tab_width = buffer_tab_width(i, buf_id);
        let ctl_arrow = buffer_setting(i, "ctl-arrow", buf_id).truthy();
        let nobreak_char_display = buffer_setting(i, "nobreak-char-display", buf_id);
        let wrap_lines = !buffer_setting(i, "truncate-lines", buf_id).truthy();
        let mode_line_format = buffer_setting(i, "mode-line-format", buf_id);
        let mode_line_enabled = !mode_line_format.is_nil();
        let tab_line_format = buffer_setting(i, "tab-line-format", buf_id);
        let tab_line_enabled = !tab_line_format.is_nil();
        let header_line_format = buffer_setting(i, "header-line-format", buf_id);
        let header_line_enabled = !header_line_format.is_nil();
        let line_number_mode = buffer_setting(i, "display-line-numbers", buf_id);
        let line_number_offset = match buffer_setting(i, "display-line-numbers-offset", buf_id) {
            Value::Int(offset) => offset,
            _ => 0,
        };
        let default_line_number_width = i
            .buffers
            .get(buf_id)
            .map(|buffer| {
                let bb = buffer.borrow();
                let last_line = bb.text.line_of_pos(bb.zv.min(bb.text_len())) + 1;
                (last_line as i128)
                    .saturating_add(line_number_offset)
                    .to_string()
                    .chars()
                    .count()
                    .max(1)
            })
            .unwrap_or(1);
        let line_number_width = match buffer_setting(i, "display-line-numbers-width", buf_id) {
            Value::Int(width) if width > 0 => usize::try_from(width).unwrap_or(default_line_number_width),
            _ => default_line_number_width,
        };
        let gutter_width = if line_number_mode.truthy() {
            line_number_width
                .saturating_add(1)
                .min(content_width_with_margins.saturating_sub(1))
        } else {
            0
        };
        let gutter_left = content_left_with_margin;
        let content_left = gutter_left.saturating_add(gutter_width);
        let content_width = content_width_with_margins.saturating_sub(gutter_width);
        let fringe_style = fctx.builtin(i, "fringe", 0).finish();
        let indicate_boundaries = buffer_setting(i, "indicate-buffer-boundaries", buf_id);
        let show_left_boundaries = match &indicate_boundaries {
            Value::Nil => false,
            Value::Sym(symbol) => matches!(i.symbol_name(*symbol).as_str(), "t" | "left"),
            _ => indicate_boundaries.truthy(),
        };
        let show_right_boundaries = matches!(
            &indicate_boundaries,
            Value::Sym(symbol) if matches!(i.symbol_name(*symbol).as_str(), "t" | "right")
        );
        let indicate_empty_lines = buffer_setting(i, "indicate-empty-lines", buf_id).truthy();
        let overlay_arrows = overlay_arrow_positions(i, buf_id);
        let (first_buffer_line, last_buffer_line) = i
            .buffers
            .get(buf_id)
            .map(|buffer| {
                let bb = buffer.borrow();
                (bb.text.line_of_pos(bb.begv), bb.text.line_of_pos(bb.zv))
            })
            .unwrap_or((usize::MAX, usize::MAX));
        let requested_top_rows = usize::from(tab_line_enabled) + usize::from(header_line_enabled);
        let top_rows = requested_top_rows.min(window_height.saturating_sub(usize::from(mode_line_enabled)));
        let text_rows = window_height
            .saturating_sub(top_rows + usize::from(mode_line_enabled));
        let content_top = window_top + top_rows;
        if graphical && vscroll > 0 && text_rows > 0 && window_width > 0 {
            vscrolls.push(crate::frontend::GridVScroll {
                window_id,
                left: window_left,
                top: content_top,
                width: window_width,
                rows: text_rows,
                pixels: vscroll,
            });
        }
        // Keep point in the visible rows using GNU's automatic scroll
        // settings. `start' and `hscroll' are redisplay state, so updating
        // them here is correct.
        let (start, hscroll) = match i.buffers.get(buf_id) {
            Some(b) if text_rows > 0 => {
                let (suspend_hs, suspend_vs, old_pt, min_hs) = {
                    let wb = w.borrow();
                    (
                        wb.suspend_hscroll,
                        wb.suspend_vscroll,
                        wb.old_point_position(),
                        wb.min_hscroll,
                    )
                };
                let bb = b.borrow();
                let pt = (if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    bb.point()
                } else {
                    w.borrow().point
                })
                .clamp(bb.begv, bb.zv);
                // GNU xdisp.c: point movement since the last redisplay
                // lifts an hscroll suspension; then record it as the
                // new old_point.
                let mut wb_mut = w.borrow_mut();
                if wb_mut.suspend_hscroll && pt != old_pt {
                    wb_mut.suspend_hscroll = false;
                }
                if wb_mut.suspend_vscroll && pt != old_pt {
                    wb_mut.suspend_vscroll = false;
                }
                wb_mut.record_old_point(pt);
                drop(wb_mut);
                let suspend_hs = suspend_hs && pt == old_pt;
                let suspend_vs = suspend_vs && pt == old_pt;
                let pt_line = bb.text.line_of_pos(pt);
                let margin = effective_scroll_margin(i, buf_id, text_rows);
                let scroll_conservatively = if minibuffer_window
                    && buffer_setting(i, "scroll-minibuffer-conservatively", buf_id).truthy()
                {
                    101
                } else {
                    buffer_setting(i, "scroll-conservatively", buf_id)
                        .int()
                        .unwrap_or(0)
                        .max(0) as usize
                };
                let scroll_step = buffer_setting(i, "scroll-step", buf_id)
                    .int()
                    .unwrap_or(0)
                    .max(0) as usize;
                let row_limit = text_rows
                    .saturating_add(margin)
                    .saturating_add(scroll_conservatively.min(100))
                    .saturating_add(scroll_step.min(10_000))
                    .saturating_add(2)
                    .max(1);
                let point_row = display_row_distance_from_start(
                    &bb.text,
                    bb.begv,
                    bb.zv,
                    start,
                    pt,
                    wrap_lines,
                    content_width,
                    tab_width,
                    ctl_arrow,
                    row_limit,
                );
                let bottom_margin_row = text_rows.saturating_sub(1).saturating_sub(margin);
                let scroll_direction = if point_row < 0
                    || (margin > 0 && point_row < margin as isize)
                {
                    Some(false)
                } else if point_row >= text_rows as isize
                    || (margin > 0 && point_row >= bottom_margin_row as isize)
                {
                    Some(true)
                } else {
                    None
                };
                let mut new_start = if suspend_vs {
                    start
                } else {
                    let target_row = scroll_direction.map(|scrolling_down| {
                        let aggressive_name = if scrolling_down {
                            "scroll-up-aggressively"
                        } else {
                            "scroll-down-aggressively"
                        };
                        let aggressive = match buffer_setting(i, aggressive_name, buf_id) {
                            Value::Int(value) => Some(value as f64),
                            Value::Float(value) => Some(*value),
                            _ => None,
                        }
                        .filter(|value| value.is_finite());
                        automatic_scroll_target_row(
                            point_row,
                            scrolling_down,
                            text_rows,
                            margin,
                            scroll_conservatively,
                            scroll_step,
                            aggressive,
                        )
                    });
                    target_row.map_or(start, |rows_before| {
                        display_row_start_before_point(
                            &bb.text,
                            bb.begv,
                            bb.zv,
                            pt,
                            rows_before,
                            wrap_lines,
                            content_width,
                            tab_width,
                            ctl_arrow,
                        )
                    })
                };
                if wrap_lines && content_width > 0 && !suspend_vs {
                    let new_start_line = bb.text.line_of_pos(new_start);
                    let new_start_line_pos = bb.text.line_start(new_start_line).max(bb.begv);
                    let start_offset = new_start.saturating_sub(new_start_line_pos);
                    let point_line_pos = bb.text.line_start(pt_line).max(bb.begv);
                    let point_index = pt.saturating_sub(point_line_pos);
                    let mut recent_starts = VecDeque::new();
                    let mut visual_row = 0usize;
                    let mut point_visual_row = None;
                    'lines: for line_number in new_start_line..=pt_line {
                        let line_pos = bb.text.line_start(line_number).max(bb.begv);
                        if line_pos > bb.zv {
                            break;
                        }
                        let line_end = bb.text.line_end(line_pos).min(bb.zv);
                        let line = bb.text.substring(line_pos, line_end);
                        let line_len = line.chars().count();
                        for range in wrap_ranges(&line, content_width, tab_width, ctl_arrow) {
                            if line_number == new_start_line
                                && range.end <= start_offset
                                && range.end < line_len
                            {
                                continue;
                            }
                            recent_starts.push_back(line_pos + range.start);
                            if recent_starts.len() > text_rows {
                                recent_starts.pop_front();
                            }
                            if line_number == pt_line
                                && point_index >= range.start
                                && (point_index < range.end
                                    || (point_index == range.end && range.end == line_len))
                            {
                                point_visual_row = Some(visual_row);
                                break 'lines;
                            }
                            visual_row = visual_row.saturating_add(1);
                        }
                    }
                    if point_visual_row.is_some_and(|row| row >= text_rows)
                        && let Some(row_start) = recent_starts.front()
                    {
                        new_start = *row_start;
                    }
                }
                // Auto-hscroll only when `auto-hscroll-mode' allows it
                // for this window's buffer and not suspended by an
                // explicit `scroll-left'/`-right'/`set-window-hscroll'.
                // GNU's min_hscroll floors the automatic value.
                let ahs_id = i.q_soft("auto-hscroll-mode").unwrap_or(u32::MAX);
                let ahs_on = i
                    .buffer_local_toplevel_value(ahs_id, buf_id)
                    .or_else(|| i.default_toplevel_value(ahs_id))
                    .map(|v| v.truthy())
                    .unwrap_or(true);
                let new_hscroll = if wrap_lines {
                    0
                } else if content_width > 0 && ahs_on && !suspend_hs {
                    let line_start = bb.text.line_start(pt_line).max(bb.begv);
                    let line_end = bb.text.line_end(line_start).min(bb.zv);
                    let col = display_columns(&bb.text.substring(line_start, pt), tab_width);
                    let line_columns =
                        display_columns(&bb.text.substring(line_start, line_end), tab_width);
                    let margin = buffer_setting(i, "hscroll-margin", buf_id)
                        .int()
                        .unwrap_or(0)
                        .clamp(0, 1_000_000) as usize;
                    automatic_hscroll_position(
                        col,
                        line_columns,
                        pt >= line_end,
                        hscroll,
                        content_width,
                        margin,
                        hscroll_step_setting(i, buf_id),
                        min_hs,
                    )
                    .unwrap_or(hscroll)
                } else {
                    hscroll
                };
                {
                    let mut wb = w.borrow_mut();
                    if new_start != start {
                        wb.start = new_start;
                    }
                    if new_hscroll != hscroll {
                        wb.hscroll = new_hscroll;
                    }
                }
                (new_start, new_hscroll)
            }
            _ => (start, hscroll),
        };
        if graphical && !minibuffer_window {
            let (first_line, last_line) = i
                .buffers
                .get(buf_id)
                .map(|buffer| {
                    let bb = buffer.borrow();
                    (bb.text.line_of_pos(bb.begv), bb.text.line_of_pos(bb.zv))
                })
                .unwrap_or((0, 0));
            let total_lines = last_line.saturating_sub(first_line).saturating_add(1);
            let top_line = i
                .buffers
                .get(buf_id)
                .map(|buffer| buffer.borrow().text.line_of_pos(start));
            if let (Some(side), Some(bar_width), Some(top_line)) =
                (vertical_scroll_side, scroll_width, top_line)
                && bar_width > 0
                && text_rows > 0
            {
                let track_height = text_rows.saturating_mul(frame_char_height);
                if let Some((thumb_start, thumb_size)) = scroll_thumb(
                    track_height,
                    total_lines,
                    text_rows,
                    top_line.saturating_sub(first_line),
                    frame_char_height.saturating_mul(2),
                ) {
                    let bar_left = if side {
                        raw_window_left.saturating_mul(frame_char_width)
                    } else {
                        raw_window_right
                            .saturating_mul(frame_char_width)
                            .saturating_sub(bar_width)
                    };
                    scroll_bars.push(GridScrollBar {
                        window_id,
                        buffer_id: buf_id,
                        orientation: GridScrollBarOrientation::Vertical,
                        left: bar_left,
                        top: content_top.saturating_mul(frame_char_height),
                        width: bar_width,
                        height: track_height,
                        thumb_start,
                        thumb_size,
                        max_offset: total_lines.saturating_sub(text_rows),
                    });
                }
            }
            if let (Some(side), Some(bar_height)) = (horizontal_scroll_side, scroll_height)
                && bar_height > 0
                && content_width > 0
                && text_rows > 0
                && !wrap_lines
            {
                let max_columns = i.buffers.get(buf_id).map_or(0, |buffer| {
                    let bb = buffer.borrow();
                    let first = bb.text.line_of_pos(bb.begv);
                    let last = bb.text.line_of_pos(bb.zv);
                    (first..=last)
                        .map(|line| {
                            let line_start = bb.text.line_start(line).max(bb.begv);
                            let line_end = bb.text.line_end(line_start).min(bb.zv);
                            display_columns(&bb.text.substring(line_start, line_end), tab_width)
                        })
                        .max()
                        .unwrap_or(0)
                });
                let track_width = content_width.saturating_mul(frame_char_width);
                if let Some((thumb_start, thumb_size)) = scroll_thumb(
                    track_width,
                    max_columns,
                    content_width,
                    hscroll,
                    frame_char_width.saturating_mul(2),
                ) {
                    let top = if side {
                        raw_window_top.saturating_mul(frame_char_height)
                    } else {
                        raw_window_bottom
                            .saturating_mul(frame_char_height)
                            .saturating_sub(bar_height)
                    };
                    scroll_bars.push(GridScrollBar {
                        window_id,
                        buffer_id: buf_id,
                        orientation: GridScrollBarOrientation::Horizontal,
                        left: content_left.saturating_mul(frame_char_width),
                        top,
                        width: track_width,
                        height: bar_height,
                        thumb_start,
                        thumb_size,
                        max_offset: max_columns.saturating_sub(content_width),
                    });
                }
            }
        }
        // Per-window face context (overlays, text props, region).
        let mut wf = i
            .buffers
            .get(buf_id)
            .map(|b| window_faces(i, &fctx, &b, &Value::Window(w.clone()), mouse_position));
        let glyphless_style = fctx.builtin(i, "glyphless-char", 0).finish();
        let fill_column_indicator =
            buffer_fill_column_indicator(i, &mut fctx, wf.as_ref(), buf_id);
        let buffer_line_spacing = buffer_setting(i, "line-spacing", buf_id);
        let default_spacing_value = if buffer_line_spacing.is_nil() {
            frame_line_spacing.clone()
        } else {
            buffer_line_spacing
        };
        let default_line_spacing = if graphical {
            line_spacing_pixels(&default_spacing_value, frame_char_height).unwrap_or(0)
        } else {
            0
        };
        if selected_window {
            default_row_height = frame_char_height.saturating_add(default_line_spacing);
        }
        if selected_window
            && let Some(position) = mouse_position
            && position.buffer_id == buf_id
        {
            hover_cursor = Some(grid_pointer_shape(
                i,
                wf.as_ref().and_then(|faces| faces.pointer.as_ref()),
            ));
        }
        let mode_point = i.buffers.get(buf_id).map_or(0, |buffer| {
            let bb = buffer.borrow();
            (if selected_window {
                bb.point()
            } else {
                w.borrow().point
            })
            .clamp(bb.begv, bb.zv)
        });
        let mode_context = mode_line_context(i, buf_id, mode_point, tab_width);
        let mut auxiliary_row = window_top;
        if tab_line_enabled && auxiliary_row < content_top {
            place_formatted_window_row(
                i,
                &mut fctx,
                &mut screen_rows,
                auxiliary_row,
                window_left,
                window_width,
                tab_width,
                buf_id,
                &tab_line_format,
                &mode_context,
                "tab-line",
            );
            if let Some(text) = mode_line_help_echo(i, buf_id, &tab_line_format, 0) {
                push_help_echo(&mut help_echoes, GridHelpEcho {
                    row: auxiliary_row,
                    left: window_left,
                    width: window_width,
                    text,
                });
            }
            auxiliary_row += 1;
        }
        if header_line_enabled && auxiliary_row < content_top {
            place_formatted_window_row(
                i,
                &mut fctx,
                &mut screen_rows,
                auxiliary_row,
                window_left,
                window_width,
                tab_width,
                buf_id,
                &header_line_format,
                &mode_context,
                "header-line",
            );
            if let Some(text) = mode_line_help_echo(i, buf_id, &header_line_format, 0) {
                push_help_echo(&mut help_echoes, GridHelpEcho {
                    row: auxiliary_row,
                    left: window_left,
                    width: window_width,
                    text,
                });
            }
        }
        let mut visible_end = start;
        let mut screen_row = content_top;
        let selective_display = buffer_setting(i, "selective-display", buf_id);
        let logical_rows = i.buffers.get(buf_id).map_or(text_rows, |buffer| {
            let bb = buffer.borrow();
            let first = bb.text.line_of_pos(start);
            let last = bb.text.line_of_pos(bb.zv);
            last.saturating_sub(first).saturating_add(1)
        });
        for row in 0..logical_rows {
            if screen_row >= body_height || screen_row >= window_top + text_rows {
                break;
            }
            let (
                line,
                line_pos,
                row_end,
                point_index,
                line_number,
                point_line_number,
                line_is_complete,
            ) =
                match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let line_index = bb.text.line_of_pos(start) + row;
                    let physical_start = bb.text.line_start(line_index);
                    if physical_start > bb.zv {
                        break;
                    }
                    let physical_end = bb.text.line_end(physical_start);
                    let line_is_complete = physical_end <= bb.zv;
                    let ls = physical_start.max(bb.begv);
                    let le = physical_end.min(bb.zv);
                    let row_end = if le == physical_end
                        && physical_end < bb.text_len()
                        && bb.text.char_at(physical_end) == '\n'
                    {
                        physical_end + 1
                    } else {
                        le
                    };
                    let point = if selected_window {
                        bb.point()
                    } else {
                        w.borrow().point
                    }
                    .clamp(bb.begv, bb.zv);
                    let point_line_number = bb.text.line_of_pos(point);
                    let point_index = (point_line_number == line_index && point >= ls)
                        .then(|| point - ls);
                    (
                        bb.text.substring(ls, le),
                        ls,
                        row_end,
                        point_index,
                        line_index + 1,
                        point_line_number,
                        line_is_complete,
                    )
                }
                None => (EmacsText::new(), 0, 0, None, 1, usize::MAX, false),
            };
            visible_end = row_end;
            let line_len = line.chars().count();
            let trailing_whitespace = if line_is_complete
                && point_index != Some(line_len)
                && wf
                    .as_ref()
                    .is_some_and(|faces| faces.show_trailing_whitespace)
            {
                let trailing_chars = line
                    .chars()
                    .rev()
                    .take_while(|character| {
                        character
                            .unicode()
                            .is_some_and(|character| matches!(character, ' ' | '\t'))
                    })
                    .count();
                (trailing_chars > 0).then(|| {
                    line_pos + line_len - trailing_chars..line_pos + line_len
                })
            } else {
                None
            };
            if let Some(faces) = wf.as_mut() {
                faces.trailing_whitespace = trailing_whitespace;
            }
            if selective_display_hides_line(&selective_display, &line, tab_width) {
                continue;
            }
            let start_offset = if row == 0 {
                start.saturating_sub(line_pos).min(line_len)
            } else {
                0
            };
            let line_prefix = (start_offset == 0)
                .then(|| line_prefix_at_pos(i, buf_id, &Value::Window(w.clone()), line_pos))
                .flatten();
            let requested_prefix_width = line_prefix
                .as_ref()
                .map(|prefix| display_columns(prefix, tab_width))
                .unwrap_or(0)
                .min(content_width);
            let first_character_width = line.chars().next().map_or(0, |character| {
                let byte8 = character.byte8().is_some();
                let code = character.code();
                let character = character.unicode().unwrap_or('\u{fffd}');
                if character == '\t' {
                    tab_width - requested_prefix_width % tab_width.max(1)
                } else if byte8 {
                    4
                } else if ctl_arrow && (code < 0x20 || code == 0x7f) {
                    2
                } else {
                    display_char_width(character)
                }
            });
            let line_prefix_width = if wrap_lines
                && line_len > 0
                && requested_prefix_width.saturating_add(first_character_width) > content_width
            {
                0
            } else {
                requested_prefix_width
            };
            let ranges = if wrap_lines {
                wrap_ranges_with_prefix(
                    &line,
                    content_width,
                    line_prefix_width,
                    tab_width,
                    ctl_arrow,
                )
            } else {
                vec![0..line_len]
            };
            for range in ranges {
                if range.end < start_offset
                    || (range.end == start_offset && range.end < line_len)
                {
                    continue;
                }
                if screen_row >= body_height || screen_row >= window_top + text_rows {
                    break;
                }
                let segment_start = range.start.max(start_offset);
                let segment_end = range.end;
                let segment_len = segment_end.saturating_sub(segment_start);
                let prefix_width = if segment_start == 0 {
                    line_prefix_width
                } else {
                    0
                };
                let text_width = content_width.saturating_sub(prefix_width);
                let segment_text: EmacsText = line
                    .chars()
                    .skip(segment_start)
                    .take(segment_len)
                    .collect();
                let segment_chars: Vec<_> = segment_text.chars().collect();
                let segment_pos = line_pos + segment_start;
                let point_index = point_index.and_then(|point| {
                    (point >= segment_start
                        && (point < segment_end
                            || (point == segment_end && segment_end == line_len)))
                        .then_some(point - segment_start)
                });
                let y = screen_row;
                let newline_spacing = if graphical
                    && segment_end == line_len
                    && row_end > line_pos.saturating_add(line_len)
                {
                    wf.as_ref()
                        .map(|faces| {
                            line_spacing_at_pos(
                                i,
                                faces,
                                line_pos.saturating_add(line_len),
                                default_line_spacing,
                                frame_char_height,
                            )
                        })
                        .unwrap_or(default_line_spacing)
                } else {
                    default_line_spacing
                };
                let explicit_line_height = if graphical
                    && segment_end == line_len
                    && row_end > line_pos.saturating_add(line_len)
                {
                    wf.as_ref().and_then(|faces| {
                        line_height_at_pos(
                            i,
                            faces,
                            line_pos.saturating_add(line_len),
                            frame_char_height,
                        )
                    })
                } else {
                    None
                };
                let display_row_height = match explicit_line_height {
                    Some(LineHeightProperty::ContentsOnly) => frame_char_height,
                    Some(LineHeightProperty::Total(total)) => frame_char_height.max(total),
                    Some(LineHeightProperty::Minimum(height)) => frame_char_height
                        .max(height)
                        .saturating_add(newline_spacing),
                    None => frame_char_height.saturating_add(newline_spacing),
                };
                if let Some(row_height) = row_heights.get_mut(y) {
                    // Grid rows are shared across side-by-side windows, so
                    // reserve enough height for the tallest row at this y.
                    *row_height = (*row_height).max(display_row_height);
                }
                visible_end = if segment_end == line_len {
                    row_end
                } else {
                    line_pos + segment_end
                };
                let row_hscroll = if wrap_lines { 0 } else { hscroll };
                if gutter_width > 0 {
                    let number_style = if point_line_number + 1 == line_number {
                        fctx.builtin(i, "line-number-current-line", 0).finish()
                    } else {
                        fctx.builtin(i, "line-number", 0).finish()
                    };
                    let show_number = segment_start == 0 || (row == 0 && start_offset > 0);
                    let number = if show_number {
                        let absolute = (line_number as i128).saturating_add(line_number_offset);
                        let value = if matches!(line_number_mode, Value::Sym(symbol) if i.symbol_name(symbol) == "relative")
                            && point_line_number + 1 != line_number
                        {
                            ((point_line_number as i128 + 1) - line_number as i128).abs()
                        } else {
                            absolute
                        };
                        format!("{value:>line_number_width$} ")
                    } else {
                        String::new()
                    };
                    let prefix = fit_text_to_cells_with_tab_width(
                        &number,
                        gutter_width,
                        tab_width,
                    )
                    .0;
                    let prefix_cells = vec![number_style; gutter_width];
                    let prefix_chars = prefix.chars().map(|_| number_style).collect::<Vec<_>>();
                    place_text_row(
                        &mut screen_rows[y],
                        gutter_left,
                        &prefix,
                        &prefix_cells,
                        &prefix_chars,
                    );
                }
                let mut rendered = render_text_row(
                    &segment_text,
                    row_hscroll,
                    text_width,
                    prefix_width,
                    tab_width,
                    default_style,
                    |index| {
                        wf.as_ref().map_or(default_style, |wf| {
                            let position = segment_pos + index;
                            let extra_face = segment_chars.get(index).and_then(|character| {
                                let has_display_override = display_replacement_at(wf, position)
                                    .is_some()
                                    || display_table_replacement(
                                        i,
                                        buf_id,
                                        wf,
                                        *character,
                                    )
                                    .is_some();
                                (!has_display_override)
                                    .then(|| nobreak_char_face(&nobreak_char_display, *character))
                                    .flatten()
                            });
                            style_at_pos_with_named_face(
                                i,
                                &mut fctx,
                                wf,
                                position,
                                extra_face,
                            )
                        })
                    },
                    |index| {
                        wf.as_ref()
                            .map(|wf| visibility_at_pos(i, wf, segment_pos + index))
                            .unwrap_or(Visibility::Visible)
                    },
                    |index| {
                        wf.as_ref()
                            .and_then(|wf| display_replacement_at(wf, segment_pos + index))
                            .or_else(|| {
                                let wf = wf.as_ref()?;
                                let character = *segment_chars.get(index)?;
                                display_table_replacement(i, buf_id, wf, character)
                            })
                            .or_else(|| {
                                let character = *segment_chars.get(index)?;
                                nobreak_char_replacement(&nobreak_char_display, character)
                            })
                            .or_else(|| {
                                let character = *segment_chars.get(index)?;
                                glyphless_display_replacement(
                                    i,
                                    buf_id,
                                    character,
                                    graphical,
                                    glyphless_style,
                                )
                            })
                            .or_else(|| {
                                let character = *segment_chars.get(index)?;
                                raw_byte_replacement(i, buf_id, character)
                            })
                            .or_else(|| {
                                segment_chars
                                    .get(index)
                                    .and_then(|character| ctl_arrow_replacement(*character, ctl_arrow))
                            })
                    },
                    |index| {
                        wf.as_ref()
                            .map(|wf| overlay_strings_at(wf, segment_pos + index))
                            .unwrap_or_default()
                    },
                    segment_end == line_len,
                    graphical,
                );
                if prefix_width > 0
                    && let Some(prefix) = line_prefix.as_ref()
                {
                    rendered = prepend_rendered_prefix(
                        prefix,
                        prefix_width,
                        tab_width,
                        rendered,
                    );
                }
                pending_compositions.extend(rendered.compositions.iter().filter_map(
                    |composition| {
                        let column = content_left.saturating_add(composition.column);
                        (column < width).then(|| {
                            (
                                y,
                                column,
                                composition.components.clone(),
                                composition.style,
                            )
                        })
                    },
                ));
                if let Some(wf) = &wf {
                    for index in 0..segment_len {
                        let Some(text) = help_echo_at(i, wf, segment_pos + index) else {
                            continue;
                        };
                        let source_column = rendered
                            .source_columns
                            .get(index)
                            .copied()
                            .unwrap_or(0);
                        let source_end = rendered
                            .source_columns
                            .get(index + 1)
                            .copied()
                            .unwrap_or_else(|| source_column.saturating_add(1));
                        let visible_column = source_column.saturating_sub(row_hscroll);
                        if visible_column >= content_width {
                            continue;
                        }
                        let visible_width = source_end
                            .saturating_sub(source_column)
                            .max(1)
                            .min(content_width.saturating_sub(visible_column));
                        push_help_echo(&mut help_echoes, GridHelpEcho {
                            row: y,
                            left: content_left.saturating_add(visible_column),
                            width: visible_width,
                            text,
                        });
                    }
                }
                let first_line = line_number.saturating_sub(1) == first_buffer_line;
                let last_line = line_number.saturating_sub(1) == last_buffer_line;
                let mut left_indicator = if wrap_lines && segment_start > 0 {
                    "↪".to_owned()
                } else {
                    String::new()
                };
                let mut right_indicator = String::new();
                if show_left_boundaries {
                    if first_line {
                        left_indicator = "↑".to_owned();
                    }
                    if last_line {
                        left_indicator = if first_line { "↕" } else { "↓" }.to_owned();
                    }
                }
                if show_right_boundaries {
                    if first_line {
                        right_indicator = "↑".to_owned();
                    }
                    if last_line {
                        right_indicator = if first_line { "↕" } else { "↓" }.to_owned();
                    }
                }
                if indicate_empty_lines && line_len == 0 && last_line {
                    right_indicator = "·".to_owned();
                }
                if !wrap_lines
                    && display_columns(&line, tab_width) > row_hscroll.saturating_add(text_width)
                {
                    right_indicator = "›".to_owned();
                }
                if let Some((_, arrow)) = overlay_arrows.iter().find(|(position, _)| {
                    *position >= segment_pos
                        && (*position < segment_pos.saturating_add(segment_len)
                            || (*position == segment_pos.saturating_add(segment_len)
                                && segment_end == line_len))
                }) {
                    left_indicator = arrow.clone();
                }
                place_fringe_glyph(
                    &mut screen_rows,
                    y,
                    window_left,
                    fringe_left_cells,
                    &left_indicator,
                    fringe_style,
                );
                place_fringe_glyph(
                    &mut screen_rows,
                    y,
                    right_fringe_left,
                    fringe_right_cells,
                    &right_indicator,
                    fringe_style,
                );
                images.extend(rendered.images.iter().filter_map(|image| {
                    let column = content_left.saturating_add(image.column);
                    (column < width).then(|| GridImage {
                        row: y,
                        column,
                        source: image.source.clone(),
                        width: image.width,
                        height: image.height,
                    })
                }));
                // Empty tail of the row: the line-end face extends its
                // background over it iff the face's :extend holds (GNU
                // extend_face_to_end_of_line).
                if segment_end == line_len {
                    if let Some(wf) = &wf {
                        let end_pos = line_pos + line_len;
                        let tail = style_at_pos(i, &mut fctx, wf, end_pos);
                        if tail.extend {
                            for cell in rendered.visible_columns..content_width {
                                let mut t = tail;
                                t.fg = None;
                                t.bold = false;
                                t.italic = false;
                                t.underline = false;
                                t.overline = false;
                                t.strike = false;
                                t.inverse = false;
                                t.extend = false;
                                rendered.cell_attrs[cell] = t;
                            }
                            let mut tail = tail;
                            tail.fg = None;
                            tail.bold = false;
                            tail.italic = false;
                            tail.underline = false;
                            tail.overline = false;
                            tail.strike = false;
                            tail.inverse = false;
                            tail.extend = false;
                            for style in &mut rendered.char_attrs[rendered.padding_char_start..] {
                                *style = tail;
                            }
                        }
                    }
                }
                if let Some(point_index) = point_index
                    && content_width > 0
                {
                    let point_col = rendered
                        .source_columns
                        .get(point_index)
                        .copied()
                        .unwrap_or(0);
                    let col = point_col
                        .saturating_sub(row_hscroll)
                        .min(content_width - 1);
                    let byte_index = if point_col < row_hscroll {
                        0
                    } else if point_col.saturating_sub(row_hscroll) >= content_width {
                        rendered.cell_offsets[content_width - 1]
                    } else {
                        rendered
                            .source_offsets
                            .get(point_index)
                            .copied()
                            .unwrap_or(rendered.text.len())
                    };
                    if selected_window {
                        selected_cursor = Some((y, content_left, col, byte_index));
                    }
                    if let Some((shape, thickness)) = cursor_style {
                        let stretch = segment_text.chars().nth(point_index).and_then(|ch| {
                            let ch = ch.unicode().unwrap_or('\u{fffd}');
                            let char_width = if ch == '\t' {
                                tab_width - point_col % tab_width
                            } else {
                                display_char_width(ch)
                            };
                            let stretchable = if ch.is_whitespace() {
                                stretch_cursor && char_width > 1
                            } else {
                                char_width > 1
                            };
                            (point_col >= row_hscroll
                                && point_col.saturating_sub(row_hscroll) < content_width
                                && stretchable)
                                .then(|| rendered.source_offsets.get(point_index + 1).copied())
                                .flatten()
                        });
                        positioned_cursors.push((
                            GridCursor {
                                row: y,
                                column: col,
                                index: byte_index,
                                after_index: stretch,
                                shape,
                                thickness,
                                color: cursor_color,
                                selected: selected_window,
                            },
                        content_left,
                        ));
                    }
                }
                if selected_window
                    && input
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.buffer_id == buf_id)
                {
                    if let Some(snapshot) = input.as_mut() {
                        let row_index = snapshot.rows.len();
                        snapshot.rows.push(GridInputRow {
                            row: y,
                            buffer_start: segment_pos,
                            source_offsets: rendered.source_offsets.clone(),
                        });
                        input_screen_lefts.push((row_index, content_left));
                    }
                }
                place_rendered_row(&mut screen_rows[y], content_left, &rendered);
                if (!wrap_lines || segment_start == 0)
                    && let Some((indicator_column, character, style)) = fill_column_indicator
                    && indicator_column >= row_hscroll
                {
                    let visible_column = indicator_column - row_hscroll;
                    if visible_column < content_width {
                        let column = content_left.saturating_add(visible_column);
                        if screen_rows[y]
                            .get(column)
                            .is_some_and(|cell| !cell.occupied)
                        {
                            fill_column_indicators.push(GridFillColumnIndicator {
                                row: y,
                                column,
                                character,
                                style,
                            });
                        }
                    }
                }
                screen_row += 1;
            }
        }
        if indicate_empty_lines && fringe_right_cells > 0 {
            for row in screen_row..(window_top + text_rows).min(body_height) {
                place_fringe_glyph(
                    &mut screen_rows,
                    row,
                    right_fringe_left,
                    fringe_right_cells,
                    "·",
                    fringe_style,
                );
            }
        }
        let tick = i
            .buffers
            .get(buf_id)
            .map(|b| b.borrow().text.mod_tick.get())
            .unwrap_or(0);
        w.borrow_mut().redisplay_end = Some((buf_id, tick, start, visible_end));
        // Mode line.
        let my = window_top + text_rows;
        if mode_line_enabled && my < body_height {
            // GNU draws mode lines under `mode-line-active' on the
            // selected window, `mode-line-inactive' elsewhere — both
            // resolve through `mode-line' (inverse on a mono tty).
            // An unresolved (pre-faces.el) mode-line face falls back
            // to plain inverse so the row still reads as a divider.
            let ml_name = if std::rc::Rc::ptr_eq(w, &fb.selected) {
                "mode-line-active"
            } else {
                "mode-line-inactive"
            };
            place_formatted_window_row(
                i,
                &mut fctx,
                &mut screen_rows,
                my,
                window_left,
                window_width,
                tab_width,
                buf_id,
                &mode_line_format,
                &mode_context,
                ml_name,
            );
            let buffer_name_range = screen_rows.get(my).and_then(|row| {
                screen_row_find_text_range(
                    row,
                    window_left,
                    window_width,
                    &mode_context.buffer_name,
                )
            });
            mode_lines.push(crate::frontend::GridModeLine {
                window_id,
                buffer_id: buf_id,
                row: my,
                left: window_left,
                width: window_width,
                buffer_name_range,
            });
            if let Some(text) = mode_line_help_echo(i, buf_id, &mode_line_format, 0) {
                push_help_echo(&mut help_echoes, GridHelpEcho {
                    row: my,
                    left: window_left,
                    width: window_width,
                    text,
                });
            }
            mode_rows.push(my);
        }
    }
    // A divider belongs to the window above it. Reserve its pixels at
    // that row boundary, counting a shared boundary only once even when
    // side-by-side windows meet above the same screen area.
    let mut divider_rows = HashMap::<usize, usize>::new();
    for divider in &bottom_dividers {
        if divider.width == 0 || divider.thickness == 0 || divider.row_boundary == 0 {
            continue;
        }
        divider_rows
            .entry(divider.row_boundary)
            .and_modify(|thickness| *thickness = (*thickness).max(divider.thickness))
            .or_insert(divider.thickness);
    }
    for (row_boundary, thickness) in divider_rows {
        if let Some(row_height) = row_heights.get_mut(row_boundary.saturating_sub(1)) {
            *row_height = row_height.saturating_add(thickness);
        }
    }
    // Render visible minibuffer lines at the bottom of the frame. When
    // sourced from the minibuffer buffer, styles and input positions keep
    // offsets relative to the full buffer.
    let mini_visible_rows = mini_height.min(mini_rows.len());
    let mini_point = if mini_styled.is_some() {
        mini_buf
            .as_ref()
            .map(|buffer| buffer.borrow().point())
            .unwrap_or(mini_characters.len())
    } else {
        mini_characters.len()
    };
    let point_row = mini_rows
        .iter()
        .position(|range| {
            (mini_point >= range.start && mini_point < range.end)
                || (range.start == range.end && mini_point == range.start)
                || (mini_point == range.end
                    && (range.end == mini_characters.len()
                        || mini_characters
                            .get(range.end)
                            .is_some_and(|character| character.unicode() == Some('\n'))))
        })
        .unwrap_or_else(|| mini_rows.len().saturating_sub(1));
    let mini_first_row = point_row
        .saturating_sub(mini_visible_rows.saturating_sub(1))
        .min(mini_rows.len().saturating_sub(mini_visible_rows));
    let mini_top = rows.len().saturating_sub(mini_height);
    if mini_visible_rows > 0 {
        let wf =
            mini_styled.map(|buffer| window_faces(i, &fctx, buffer, &Value::Nil, mouse_position));
        if let (Some(position), Some(buffer)) = (mouse_position, mini_buf.as_ref())
            && position.buffer_id == buffer.borrow().id
        {
            hover_cursor = Some(grid_pointer_shape(
                i,
                wf.as_ref().and_then(|faces| faces.pointer.as_ref()),
            ));
        }
        for (row_index, range) in mini_rows
            .iter()
            .enumerate()
            .skip(mini_first_row)
            .take(mini_visible_rows)
        {
            let Some(y) = mini_top.checked_add(row_index - mini_first_row) else {
                continue;
            };
            let line: EmacsText = mini_characters[range.clone()].iter().copied().collect();
            let rendered = render_text_row(
                &line,
                0,
                width,
                0,
                mini_tab_width,
                default_style,
                |index| {
                    wf.as_ref()
                        .map(|wf| style_at_pos(i, &mut fctx, wf, range.start + index))
                        .unwrap_or_else(|| {
                            echo_message_style(
                                i,
                                &mut fctx,
                                &i.echo_message_props,
                                range.start + index,
                            )
                        })
                },
                |_| Visibility::Visible,
                |index| {
                    wf.as_ref()
                        .and_then(|wf| display_replacement_at(wf, range.start + index))
                },
                |index| {
                    wf.as_ref()
                        .map(|wf| overlay_strings_at(wf, range.start + index))
                        .unwrap_or_default()
                },
                range.end == mini_characters.len()
                    || mini_characters
                        .get(range.end)
                        .is_some_and(|character| character.unicode() == Some('\n')),
                graphical,
            );
            pending_compositions.extend(rendered.compositions.iter().filter_map(
                |composition| {
                    (composition.column < width).then(|| {
                        (
                            y,
                            composition.column,
                            composition.components.clone(),
                            composition.style,
                        )
                    })
                },
            ));
            images.extend(rendered.images.iter().map(|image| GridImage {
                row: y,
                column: image.column,
                source: image.source.clone(),
                width: image.width,
                height: image.height,
            }));
            if let (Some(snapshot), Some(buffer)) = (input.as_mut(), mini_buf.as_ref())
                && snapshot.buffer_id == buffer.borrow().id
            {
                snapshot.rows.push(GridInputRow {
                    row: y,
                    buffer_start: range.start,
                    source_offsets: rendered.source_offsets.clone(),
                });
                input_screen_lefts.push((snapshot.rows.len() - 1, 0));
            }
            place_rendered_row(&mut screen_rows[y], 0, &rendered);
        }
    }
    if graphical
        && let Some((menu, pixel_x, pixel_y)) = &i.gui_popup_menu
    {
        popup_menu_items = render_popup_menu(
            i,
            menu,
            (*pixel_x, *pixel_y),
            width,
            body_height,
            frame_char_width,
            frame_char_height,
            default_style,
            &mut fctx,
            &mut screen_rows,
        );
    }
    let (composed_rows, composed_attrs, composed_char_attrs, cell_offsets) =
        compose_screen_rows(&screen_rows, width, default_style);
    rows = composed_rows;
    attrs = composed_attrs;
    char_attrs = composed_char_attrs;
    let compositions = pending_compositions
        .into_iter()
        .filter_map(|(row, column, components, style)| {
            Some(GridComposition {
                row,
                column,
                byte_index: *cell_offsets.get(row)?.get(column)?,
                components,
                style,
            })
        })
        .collect();
    let cursors = positioned_cursors
        .into_iter()
        .map(|(mut cursor, screen_left)| {
            let base = cell_offsets[cursor.row][screen_left.min(width)];
            cursor.column += screen_left;
            cursor.index += base;
            cursor.after_index = cursor.after_index.map(|index| index + base);
            cursor
        })
        .collect();
    if let Some((row, left, column, index)) = selected_cursor {
        let base = cell_offsets[row][left.min(width)];
        cursor_pos = Some((left + column, row));
        cursor_index = Some(base + index);
    }
    if let Some(input) = input.as_mut() {
        for (row_index, screen_left) in input_screen_lefts {
            let row = &mut input.rows[row_index];
            let base = cell_offsets[row.row][screen_left.min(width)];
            for offset in &mut row.source_offsets {
                *offset += base;
            }
        }
    }
    Grid {
        title,
        font_families: fctx.font_families,
        default_style,
        rows,
        images,
        fill_column_indicators,
        stipples: fctx.stipples,
        menus,
        tool_bar_items,
        tab_bar_items,
        help_echoes,
        tooltip,
        popup_menu_items,
        scroll_bars,
        bottom_dividers,
        mode_lines,
        vscrolls,
        compositions,
        attrs,
        char_attrs,
        mode_rows,
        cursor: cursor_pos,
        cursors,
        cursor_index,
        blink_cursor,
        blink_interval,
        default_bg,
        row_heights,
        default_line_height,
        default_row_height,
        hover_cursor,
        pointer_visible: fb.pointer_visible,
        input,
    }
}
