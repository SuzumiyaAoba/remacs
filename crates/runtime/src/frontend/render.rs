Warning: truncated output (original token count: 90474)
Total output lines: 9847

//! Resolve faces and render editor state into a frontend-neutral grid.
use super::MousePosition;
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use crate::{
    buffer::Buffer,
    lisp::{Interp, Value},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
    /// Deferred until the final foreground/background pair is known.
    pub distant_foreground: Option<CellColor>,
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

#[derive(Clone, Copy)]
struct CursorStyle {
    shape: GridCursorShape,
    thickness: usize,
    box_size: Option<usize>,
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
    /// Dimensions explicitly requested by the image descriptor.
    pub width: Option<usize>,
    pub height: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridFringeBitmapAlign {
    Center,
    Top,
    Bottom,
}

/// A monochrome fringe bitmap. Each byte holds one row, most significant bit
/// on the left, matching GNU's standard eight-pixel fringe bitmaps.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridFringeBitmap {
    pub row: usize,
    pub column: usize,
    pub pixel_width: usize,
    pub bits: Arc<[u8]>,
    pub align: GridFringeBitmapAlign,
    pub repeat: bool,
    pub style: CellStyle,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum GridImageSource {
    File(String),
    FileWithSvgOptions {
        path: String,
        base_uri: Option<String>,
        css: Option<String>,
    },
    Data {
        format: String,
        bytes: Arc<[u8]>,
    },
    DataWithBaseUri {
        format: String,
        bytes: Arc<[u8]>,
        base_uri: String,
    },
    DataWithSvgOptions {
        format: String,
        bytes: Arc<[u8]>,
        base_uri: Option<String>,
        css: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridMenu {
    pub label: String,
    pub items: Vec<GridMenuItem>,
    /// Direct top-level command by symbol name.
    pub command: Option<String>,
    /// Runtime-owned command object, used instead of a symbol name.
    pub command_id: Option<super::NativeMenuCommandId>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridMenuItem {
    pub label: String,
    /// Prepared constant help text for the native menu tooltip.
    pub help: Option<String>,
    pub command: Option<String>,
    /// Runtime-owned command object, used instead of a symbol name.
    pub command_id: Option<super::NativeMenuCommandId>,
    pub items: Vec<GridMenuItem>,
    pub separator: bool,
    /// Selected state of a toggle or radio leaf item.
    pub checked: bool,
    /// Display-only equivalent key sequence or explicit menu hint.
    pub key_hint: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridToolBarItem {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    /// UTF-8 byte range of the painted button in the composed row.
    pub byte_range: Option<(usize, usize)>,
    pub command: String,
    /// Runtime-owned command object; the symbol-name field remains compatible.
    pub command_id: Option<super::NativeMenuCommandId>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridTabBarItem {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    /// UTF-8 byte range of the visible item in the composed row.
    pub byte_range: Option<(usize, usize)>,
    pub command: String,
    pub argument: Option<i128>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridPopupMenuItem {
    pub menu_id: usize,
    /// The submenu owning this column; None denotes the root.
    pub parent: Option<usize>,
    pub selection: Option<usize>,
    pub row: usize,
    pub left: usize,
    pub width: usize,
    /// UTF-8 byte range in the composed row, resolved after cell composition.
    pub byte_range: Option<(usize, usize)>,
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
    /// Foreground colors the thumb; background colors the track.
    pub style: CellStyle,
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
    /// Pixelwise offset from the cell-aligned left edge.
    pub x_offset: i128,
    /// Pixelwise adjustment to the cell-aligned width.
    pub width_adjustment: i128,
    /// Divider thickness in pixels.
    pub thickness: usize,
    pub style: CellStyle,
    pub first_pixel_style: CellStyle,
    pub last_pixel_style: CellStyle,
}

/// A right divider between a window and an adjacent window to its right.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridRightDivider {
    /// Column boundary at the window's right edge.
    pub column_boundary: usize,
    /// Top and bottom row boundaries covered by the divider.
    pub top: usize,
    pub bottom: usize,
    /// Pixelwise adjustment to the window's right edge.
    pub width_adjustment: i128,
    /// Pixelwise offset from the cell-aligned top edge.
    pub y_offset: i128,
    /// Pixelwise adjustment to the bottom edge.
    pub height_adjustment: i128,
    /// Divider thickness in pixels.
    pub thickness: usize,
    pub style: CellStyle,
    pub first_pixel_style: CellStyle,
    pub last_pixel_style: CellStyle,
}

/// A window's mode-line hit area and buffer-identification local-map extent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridModeLine {
    pub window_id: usize,
    pub buffer_id: usize,
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub buffer_name_range: Option<(usize, usize)>,
}

/// A window's visible header-line hit area.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridHeaderLine {
    pub window_id: usize,
    pub buffer_id: usize,
    pub row: usize,
    pub left: usize,
    pub width: usize,
}

/// A visible window text rectangle, including blank space and margins.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridWindowTextArea {
    pub window_id: usize,
    pub left: usize,
    pub top: usize,
    pub width: usize,
    pub rows: usize,
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

/// A text range in a grid row whose source buffer disables bidirectional
/// display reordering. Offsets are UTF-8 byte offsets in `Grid::rows[row]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridBidiDisabledRange {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Paragraph base direction requested by a buffer for one rendered row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum GridBidiDirection {
    LeftToRight,
    RightToLeft,
}

/// A rendered row range with an explicit `bidi-paragraph-direction` value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridBidiParagraphRange {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub direction: GridBidiDirection,
}

/// A rendered window region whose horizontal pixel bounds differ from its
/// character-cell bounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridPixelwiseSegment {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub left_column: usize,
    pub right_column: usize,
    pub x_offset: i128,
    pub width_adjustment: i128,
}

/// Builder state for CellStyle while face specs merge — each field is
/// Option so an explicitly-nil attribute clears a lower layer.
#[derive(Clone, Copy, Default)]
struct StyleMix {
    fg: Option<Option<CellColor>>,
    bg: Option<Option<CellColor>>,
    distant_foreground: Option<Option<CellColor>>,
    font_family: Option<Option<u16>>,
    font_weight: Option<Option<u16>>,
    font_width: Option<Option<u8>>,
    font_height: Option<Option<u16>>,
    /// Relative heights remain composable until a lower face supplies a size.
    font_height_scale: Option<f64>,
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
        if other.font_height.is_some() {
            self.font_height = other.font_height;
            self.font_height_scale = None;
        }
        if let Some(scale) = other.font_height_scale {
            if let Some(height) = self.font_height.flatten() {
                self.font_height = Some(scale_font_height(height, scale).or(Some(height)));
                self.font_height_scale = None;
            } else {
                self.font_height_scale = Some(self.font_height_scale.unwrap_or(1.0) * scale);
            }
        }
        macro_rules! m {
            ($($f:ident),*) => {
                $(if let Some(v) = other.$f { self.$f = Some(v); })*
            };
        }
        m!(
            fg,
            bg,
            distant_foreground,
            font_family,
            font_weight,
            font_width,
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
            fg: self.fg.flatten(),
            bg: self.bg.flatten(),
            distant_foreground: self.distant_foreground.flatten(),
            font_family: self.font_family.flatten(),
            font_weight: self.font_weight.flatten(),
            font_width: self.font_width.flatten(),
            font_height: self.resolved_font_height(140),
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

    fn resolved_font_height(&self, default: u16) -> Option<u16> {
        let height = self.font_height.flatten();
        match self.font_height_scale {
            Some(scale) => scale_font_height(height.unwrap_or(default), scale).or(height),
            None => height,
        }
    }
}

impl PartialEq for StyleMix {
    fn eq(&self, o: &Self) -> bool {
        self.fg == o.fg
            && self.bg == o.bg
            && self.distant_foreground == o.distant_foreground
            && self.font_family == o.font_family
            && self.font_weight == o.font_weight
            && self.font_width == o.font_width
            && self.font_height == o.font_height
            && self.font_height_scale == o.font_height_scale
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
    /// Native title: explicit `title`, or the visible/iconified title format.
    pub title: String,
    /// Face family names referenced by styles in this snapshot.
    pub font_families: Vec<String>,
    /// The realized default face style for the current frame.
    pub default_style: CellStyle,
    /// The realized `internal-border' face style for the current frame.
    pub internal_border_style: CellStyle,
    pub rows: Vec<String>,
    /// Image runs from `display' properties, positioned in grid cells.
    pub images: Vec<GridImage>,
    pub fringe_bitmaps: Vec<GridFringeBitmap>,
    /// Fill-column indicator glyphs overlaying otherwise blank text cells.
    pub fill_column_indicators: Vec<GridFillColumnIndicator>,
    /// Bitmap face backgrounds referenced by `CellStyle::stipple`.
    pub stipples: Vec<GridStipple>,
    pub menus: Vec<GridMenu>,
    pub tool_bar_items: Vec<GridToolBarItem>,
    pub tab_bar_items: Vec<GridTabBarItem>,
    pub help_echoes: Vec<GridHelpEcho>,
    /// Pointer overrides for formatted mode/header/tab-line cells.
    pub pointer_regions: Vec<GridPointerRegion>,
    pub tooltip: Option<String>,
    pub popup_menu_items: Vec<GridPopupMenuItem>,
    pub scroll_bars: Vec<GridScrollBar>,
    pub bottom_dividers: Vec<GridBottomDivider>,
    pub right_dividers: Vec<GridRightDivider>,
    pub mode_lines: Vec<GridModeLine>,
    pub header_lines: Vec<GridHeaderLine>,
    pub window_text_areas: Vec<GridWindowTextArea>,
    pub vscrolls: Vec<GridVScroll>,
    pub compositions: Vec<GridComposition>,
    /// Text ranges whose source buffers have `bidi-display-reordering` nil.
    pub bidi_disabled_ranges: Vec<GridBidiDisabledRange>,
    /// Text ranges whose buffers specify `bidi-paragraph-direction`.
    pub bidi_paragraph_ranges: Vec<GridBidiParagraphRange>,
    /// Whether the GUI native frame may resize by individual pixels.
    pub frame_resize_pixelwise: bool,
    /// Row-local windows with pixelwise horizontal offsets or widths.
    pub pixelwise_segments: Vec<GridPixelwiseSegment>,
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
    pub blink_delay: f64,
    pub blink_blinks: i128,
    /// Time since the most recent GUI input event.
    pub blink_elapsed_since_input: f64,
    /// Default face background used outside the visible text rows.
    pub default_bg: Option<CellColor>,
    /// Gamma exponent for the frame's `screen-gamma' parameter. Color
    /// channels in this grid have already been corrected with this value.
    pub gamma_exponent: Option<f64>,
    /// Pixel height for each display row. Empty grids use the frontend default.
    pub row_heights: Vec<usize>,
    /// Base frame line height before line spacing and per-line overrides.
    pub default_line_height: usize,
    /// Default text row height used when the viewport needs more rows.
    pub default_row_height: usize,
    /// Pointer shape at the current mouse position, if it is over buffer text.
    pub hover_cursor: Option<GridPointerShape>,
    /// Keep the pointer shape while `track-mouse` has the value `dragging`.
    pub freeze_pointer_shape: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridInputPosition {
    pub buffer_id: usize,
    pub buffer_position: usize,
    pub utf16_offset: usize,
    pub column: usize,
    pub window_id: Option<usize>,
}

#[derive(Clone, PartialEq, Eq)]
struct GridInputHitTestRow {
    row: usize,
    buffer_id: usize,
    buffer_start: usize,
    source_offsets: Vec<usize>,
    first_visible_source: usize,
    char_offsets: Arc<[usize]>,
    window_id: Option<usize>,
    left: usize,
    right: usize,
    screen_left: usize,
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
    hit_test_rows: Vec<GridInputHitTestRow>,
    hit_test_char_offsets: HashMap<usize, (u64, Arc<[usize]>)>,
}

impl GridInput {
    /// Number of buffer characters represented by `text`.
    pub fn character_count(&self) -> usize {
        self.char_offsets.len().saturating_sub(1)
    }

    /// Return the UTF-16 offset for a buffer character index in constant time.
    pub fn utf16_offset_for_character(&self, character: usize) -> Option<usize> {
        self.char_offsets.get(character).copied()
    }

    /// Map a UTF-16 offset to a character index without rescanning `text`.
    pub fn character_index_for_utf16_offset(&self, requested: usize, round_up: bool) -> usize {
        let character = self.char_offsets.partition_point(|offset| *offset < requested);
        if round_up {
            character.min(self.character_count())
        } else if self.char_offsets.get(character) == Some(&requested) {
            character.min(self.character_count())
        } else {
            character.saturating_sub(1).min(self.character_count())
        }
    }

    /// Return the UTF-8 source-byte range covered by the hit-test row at a
    /// grid row and column. Used to reject pointer positions on rendered
    /// decorations instead of buffer text.
    pub fn hit_test_source_range(&self, row: usize, column: usize) -> Option<(usize, usize)> {
        let row = self.hit_test_rows.iter().find(|candidate| {
            candidate.row == row && column >= candidate.left && column < candidate.right
        })?;
        let start = *row.source_offsets.iter().min()?;
        let end = *row.source_offsets.iter().max()?;
        Some((start, end))
    }

    /// Map a rendered text byte back to its buffer and window position.
    pub fn hit_test_position(
        &self,
        row: usize,
        column: usize,
        source_byte: usize,
    ) -> Option<GridInputPosition> {
        let row = self.hit_test_rows.iter().find(|candidate| {
            candidate.row == row && column >= candidate.left && column < candidate.right
        })?;
        let character = row
            .source_offsets
            .iter()
            .enumerate()
            .skip(row.first_visible_source)
            .min_by_key(|(_, source_offset)| source_offset.abs_diff(source_byte))?
            .0;
        let buffer_position = row.buffer_start.saturating_add(character);
        let utf16_offset = row.char_offsets.get(buffer_position).copied()?;
        Some(GridInputPosition {
            buffer_id: row.buffer_id,
            buffer_position,
            utf16_offset,
            column: character,
            window_id: row.window_id,
        })
    }

    fn add_hit_test_row(
        &mut self,
        i: &Interp,
        row: usize,
        buffer_id: usize,
        buffer_start: usize,
        source_offsets: Vec<usize>,
        first_visible_source: usize,
        window_id: Option<usize>,
        left: usize,
        right: usize,
        screen_left: usize,
    ) {
        let char_offsets = if buffer_id == self.buffer_id {
            self.char_offsets.clone()
        } else {
            let Some(buffer) = i.buffers.get(buffer_id) else {
                return;
            };
            let mod_tick = buffer.borrow().text.mod_tick.get();
            if let Some((cached_tick, offsets)) = self.hit_test_char_offsets.get(&buffer_id)
                && *cached_tick == mod_tick
            {
                offsets.clone()
            } else {
                let bb = buffer.borrow();
                let source = bb.text.text();
                let mut offsets = Vec::with_capacity(source.chars().count() + 1);
                let mut utf16_offset = 0usize;
                for character in source.chars() {
                    offsets.push(utf16_offset);
                    let character = character.unicode().unwrap_or('\u{fffd}');
                    utf16_offset += character.len_utf16();
                }
                offsets.push(utf16_offset);
                let offsets: Arc<[usize]> = Arc::from(offsets);
                self.hit_test_char_offsets
                    .insert(buffer_id, (mod_tick, offsets.clone()));
                offsets
            }
        };
        self.hit_test_rows.push(GridInputHitTestRow {
            row,
            buffer_id,
            buffer_start,
            source_offsets,
            first_visible_source,
            char_offsets,
            window_id,
            left,
            right,
            screen_left,
        });
    }
}

/// Cursor shapes that can be requested by text pointer properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridPointerShape {
    Text,
    Arrow,
    Hand,
    Hourglass,
    ResizeHorizontal,
    ResizeVertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPointerRegion {
    pub row: usize,
    pub left: usize,
    pub width: usize,
    pub shape: GridPointerShape,
}

fn push_pointer_region(regions: &mut Vec<GridPointerRegion>, next: GridPointerRegion) {
    if next.width == 0 {
        return;
    }
    if let Some(previous) = regions.last_mut()
        && previous.row == next.row
        && previous.shape == next.shape
        && previous.left.saturating_add(previous.width) == next.left
    {
        previous.width = previous.width.saturating_add(next.width);
    } else {
        regions.push(next);
    }
}

impl Grid {
    /// An all-default grid of WIDTH × HEIGHT cells.
    pub fn blank(width: usize, height: usize) -> Grid {
        Grid {
            title: "Remacs".to_owned(),
            font_families: Vec::new(),
            default_style: CellStyle::default(),
            internal_border_style: CellStyle::default(),
            rows: vec![String::new(); height],
            images: Vec::new(),
            fringe_bitmaps: Vec::new(),
            fill_column_indicators: Vec::new(),
            stipples: Vec::new(),
            menus: Vec::new(),
            tool_bar_items: Vec::new(),
            tab_bar_items: Vec::new(),
            help_echoes: Vec::new(),
            pointer_regions: Vec::new(),
            tooltip: None,
            popup_menu_items: Vec::new(),
            scroll_bars: Vec::new(),
            bottom_dividers: Vec::new(),
            right_dividers: Vec::new(),
            mode_lines: Vec::new(),
            header_lines: Vec::new(),
            window_text_areas: Vec::new(),
            vscrolls: Vec::new(),
            compositions: Vec::new(),
            bidi_disabled_ranges: Vec::new(),
            bidi_paragraph_ranges: Vec::new(),
            frame_resize_pixelwise: false,
            pixelwise_segments: Vec::new(),
            attrs: vec![vec![CellStyle::default(); width]; height],
            char_attrs: vec![Vec::new(); height],
            mode_rows: Vec::new(),
            cursor: None,
            cursors: Vec::new(),
            cursor_index: None,
            blink_cursor: false,
            blink_interval: 0.5,
            blink_delay: 0.5,
            blink_blinks: 10,
            blink_elapsed_since_input: 0.0,
            default_bg: None,
            gamma_exponent: None,
            row_heights: Vec::new(),
            default_line_height: 19,
            default_row_height: 19,
            hover_cursor: None,
            freeze_pointer_shape: false,
            pointer_visible: true,
            input: None,
        }
    }
}

fn input_snapshot(
    i: &Interp,
    frame: Option<&crate::editor::FrameRef>,
    cached: Option<&GridInput>,
) -> Option<GridInput> {
    let selected_frame = frame.is_none_or(|frame| {
        i.selected_frame
            .as_ref()
            .is_some_and(|selected| std::rc::Rc::ptr_eq(selected, frame))
    });
    let (frame_buffer, frame_point) = frame.map_or((i.current_buffer, None), |frame| {
        let frame = frame.borrow();
        let window = frame.selected.borrow();
        (window.buffer, Some(window.point))
    });
    // Preserve the current-buffer snapshot for the selected frame (including
    // minibuffer input). Other frames need their own selected window's buffer
    // and point so their native views can hit-test mouse input.
    let buffer_id = if selected_frame {
        i.current_buffer
    } else {
        frame_buffer
    };
    let buffer = i.buffers.get(buffer_id)?.clone();
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
    let point = if selected_frame {
        bb.point()
    } else {
        frame_point.unwrap_or_else(|| bb.point())
    }
    .min(char_offsets.len().saturating_sub(1));
    let cursor = char_offsets.get(point).copied().unwrap_or(0);
    let mark_active = i
        .q_soft("mark-active")
        .and_then(|id| {
            i.buffer_local_toplevel_value(id, bb.id)
                .or_else(|| i.default_toplevel_value(id))
                .or_else(|| bb.locals.get(&id).cloned())
        })
        .is_some_and(|value| value.truthy());
    let transient_mark_mode = buffer_setting(i, "transient-mark-mode", bb.id).truthy();
    let selection = if mark_active && transient_mark_mode {
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
    let mut hit_test_char_offsets = cached
        .map(|snapshot| snapshot.hit_test_char_offsets.clone())
        .unwrap_or_default();
    hit_test_char_offsets.insert(bb.id, (mod_tick, char_offsets.clone()));
    Some(GridInput {
        buffer_id: bb.id,
        text,
        cursor,
        selection,
        rows: Vec::new(),
        mod_tick,
        char_offsets,
        hit_test_rows: Vec::new(),
        hit_test_char_offsets,
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

fn selective_display_next_line_is_hidden(
    i: &Interp,
    buffer_id: usize,
    next_line_number: usize,
    mode: &Value,
    tab_width: usize,
) -> bool {
    if !matches!(mode, Value::Int(threshold) if *threshold > 0) {
        return false;
    }
    let Some(buffer) = i.buffers.get(buffer_id) else {
        return false;
    };
    let buffer = buffer.borrow();
    let line_start = buffer.text.line_start(next_line_number);
    if line_start >= buffer.zv {
        return false;
    }
    let line_end = buffer.text.line_end(line_start).min(buffer.zv);
    selective_display_hides_line(
        mode,
        &buffer.text.substring(line_start, line_end),
        tab_width,
    )
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

pub(crate) fn buffer_setting(i: &Interp, name: &str, buffer: usize) -> Value {
    let Some(id) = i.q_soft(name) else {
        return Value::Nil;
    };
    i.buffer_local_toplevel_value(id, buffer)
        .or_else(|| i.default_toplevel_value(id))
        .unwrap_or(Value::Nil)
}

pub(crate) fn line_number_display_settings(i: &Interp, buffer: usize) -> (usize, i128) {
    let line_number_offset = match buffer_setting(i, "display-line-numbers-offset", buffer) {
        Value::Int(offset) => offset,
        _ => 0,
    };
    let default_width = i
        .buffers
        .get(buffer)
        .map(|buffer| {
            let buffer = buffer.borrow();
            let last_line = buffer.text.line_of_pos(buffer.zv.min(buffer.text_len())) + 1;
            (last_line as i128)
                .saturating_add(line_number_offset)
                .to_string()
                .chars()
                .count()
                .max(1)
        })
        .unwrap_or(1);
    let width = match buffer_setting(i, "display-line-numbers-width", buffer) {
        Value::Int(width) if width > 0 => usize::try_from(width).unwrap_or(default_width),
        _ => default_width,
    };
    (width, line_number_offset)
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

/// Return the display row and column for a buffer position relative to a
/// window's text area, or None when it is outside the visible rows.
pub(crate) fn position_display_cell(
    i: &Interp,
    window: &crate::editor::WindowRef,
    position: usize,
) -> Option<(usize, usize)> {
    let (buffer_id, start, height, hscroll) = {
        let window = window.borrow();
        (window.buffer, window.start, window.height, window.hscroll)
    };
    let width = crate::editor::win_body_width(i, window).max(1);
    let tab_width = buffer_tab_width(i, buffer_id);
    let ctl_arrow = buffer_setting(i, "ctl-arrow", buffer_id).truthy();
    let wrap_lines = !buffer_setting(i, "truncate-lines", buffer_id).truthy();
    let text_rows = height
        .saturating_sub(if buffer_setting(i, "mode-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .saturating_sub(if buffer_setting(i, "header-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .saturating_sub(if buffer_setting(i, "tab-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .max(1);
    if let Some(buffer) = i.buffers.get(buffer_id) {
        let bb = buffer.borrow();
        if position < bb.begv || position > bb.zv || position < start {
            return None;
        }
        let row = display_row_index_from(
            &bb.text,
            bb.begv,
            bb.zv,
            start.max(bb.begv),
            position,
            wrap_lines,
            width,
            tab_width,
            ctl_arrow,
            text_rows,
        )?;
        if row >= text_rows {
            return None;
        }
        let line_start = bb.text.line_start(bb.text.line_of_pos(position)).max(bb.begv);
        let line_end = bb.text.line_end(line_start).min(bb.zv);
        let line = bb.text.substring(line_start, line_end);
        let line_chars = line.chars().collect::<Vec<_>>();
        let offset = position.saturating_sub(line_start).min(line_chars.len());
        let ranges = if wrap_lines {
            wrap_ranges(&line, width, tab_width, ctl_arrow)
        } else {
            vec![0..line_chars.len()]
        };
        let range = ranges
            .iter()
            .find(|range| {
                offset >= range.start
                    && (offset < range.end || (offset == range.end && range.end == line_chars.len()))
            })
            .or_else(|| ranges.iter().rev().find(|range| range.start <= offset))?;
        let mut column = 0usize;
        for character in line_chars.iter().take(offset).skip(range.start) {
            let code = character.code();
            let byte8 = character.byte8().is_some();
            let character = character.unicode().unwrap_or('\u{fffd}');
            let width = if character == '\t' {
                tab_width.max(1) - column % tab_width.max(1)
            } else if byte8 {
                4
            } else if ctl_arrow && (code < 0x20 || code == 0x7f) {
                2
            } else {
                display_char_width(character)
            };
            column = column.saturating_add(width);
        }
        Some((row, column.saturating_sub(hscroll)))
    } else {
        None
    }
}

/// Return the buffer position at a display cell relative to a window's
/// text area. Coordinates are zero-based and account for wrapping, tabs,
/// wide characters, and horizontal scrolling.
pub(crate) fn position_at_display_cell(
    i: &Interp,
    window: &crate::editor::WindowRef,
    row: usize,
    column: usize,
) -> Option<usize> {
    let (buffer_id, start, height, hscroll) = {
        let window = window.borrow();
        (window.buffer, window.start, window.height, window.hscroll)
    };
    let width = crate::editor::win_body_width(i, window).max(1);
    let tab_width = buffer_tab_width(i, buffer_id);
    let ctl_arrow = buffer_setting(i, "ctl-arrow", buffer_id).truthy();
    let wrap_lines = !buffer_setting(i, "truncate-lines", buffer_id).truthy();
    let text_rows = height
        .saturating_sub(usize::from(!buffer_setting(i, "mode-line-format", buffer_id).is_nil()))
        .saturating_sub(usize::from(!buffer_setting(i, "header-line-format", buffer_id).is_nil()))
        .saturating_sub(usize::from(!buffer_setting(i, "tab-line-format", buffer_id).is_nil()))
        .max(1);
    if row >= text_rows {
        return None;
    }
    let buffer = i.buffers.get(buffer_id)?;
    let bb = buffer.borrow();
    let start = start.max(bb.begv).min(bb.zv);
    let first_line = bb.text.line_of_pos(start);
    let last_line = bb.text.line_of_pos(bb.zv);
    let target_column = column.saturating_add(hscroll);
    let mut display_row = 0usize;
    for line_number in first_line..=last_line {
        let line_start = bb.text.line_start(line_number).max(bb.begv);
        if line_start > bb.zv {
            break;
        }
        let line_end = bb.text.line_end(line_start).min(bb.zv);
        let line = bb.text.substring(line_start, line_end);
        let characters = line.chars().collect::<Vec<_>>();
        let line_len = characters.len();
        let from_offset = if line_number == first_line {
            start.saturating_sub(line_start)
        } else {
            0
        };
        let ranges = if wrap_lines {
            wrap_ranges(&line, width, tab_width, ctl_arrow)
        } else {
            vec![0..line_len]
        };
        for range in ranges {
            if line_number == first_line && range.end <= from_offset && range.end < line_len {
                continue;
            }
            if display_row == row {
                let mut display_column = 0usize;
                for (index, character) in characters
                    .iter()
                    .enumerate()
                    .take(range.end)
                    .skip(range.start)
                {
                    let code = character.code();
                    let byte8 = character.byte8().is_some();
                    let character = character.unicode().unwrap_or('\u{fffd}');
                    let width = if character == '\t' {
                        tab_width.max(1) - display_column % tab_width.max(1)
                    } else if byte8 {
                        4
                    } else if ctl_arrow && (code < 0x20 || code == 0x7f) {
                        2
                    } else {
                        display_char_width(character)
                    };
                    if target_column < display_column.saturating_add(width) {
                        return Some(line_start.saturating_add(index));
                    }
                    display_column = display_column.saturating_add(width);
                }
                return Some(line_start.saturating_add(range.end));
            }
            display_row = display_row.saturating_add(1);
            if display_row > row {
                return None;
            }
        }
    }
    None
}

/// Approximate the text range that a redisplay pass can request fontification
/// for in WINDOW. The small overestimate covers line breaks and wide cells.
pub(crate) fn visible_fontification_range(
    i: &Interp,
    window: &crate::editor::WindowRef,
) -> Option<(usize, usize)> {
    let (buffer_id, start, height) = {
        let window = window.borrow();
        (window.buffer, window.start, window.height)
    };
    let buffer = i.buffers.get(buffer_id)?;
    let (begv, zv) = {
        let buffer = buffer.borrow();
        (buffer.begv, buffer.zv)
    };
    let start = start.max(begv).min(zv);
    let rows = height
        .saturating_sub(if buffer_setting(i, "mode-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .saturating_sub(if buffer_setting(i, "header-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .saturating_sub(if buffer_setting(i, "tab-line-format", buffer_id).is_nil() {
            0
        } else {
            1
        })
        .max(1);
    let width = crate::editor::win_body_width(i, window).max(1);
    let wrapped = !buffer_setting(i, "truncate-lines", buffer_id).truthy();
    let cells = if wrapped {
        rows.saturating_mul(width.saturating_add(1))
    } else {
        width.saturating_add(1)
    };
    Some((start, start.saturating_add(cells).min(zv)))
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

fn parse_cursor_type(i: &Interp, value: &Value) -> Option<CursorStyle> {
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
    if explicit_size.is_some() && thickness == 0 && shape != GridCursorShape::Block {
        None
    } else {
        Some(CursorStyle {
            shape,
            // `(box . SIZE)' specifies the masked-image hollow threshold;
            // it does not change the width of a block cursor.
            thickness: if shape == GridCursorShape::Block {
                0
            } else {
                thickness
            },
            box_size: if shape == GridCursorShape::Block {
                explicit_size
            } else {
                None
            },
        })
    }
}

fn cursor_type_for_window(
    i: &Interp,
    window: &std::rc::Rc<std::cell::RefCell<crate::editor::Window>>,
    buffer: usize,
    selected: bool,
) -> Option<CursorStyle> {
    if !window.borrow().cursor_visible {
        return None;
    }
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
        let mut style = parse_cursor_type(i, &effective_type)?;
        match style.shape {
            GridCursorShape::Block => {
                style.shape = GridCursorShape::Hollow;
                style.box_size = None;
            }
            GridCursorShape::Bar if style.thickness > 1 => style.thickness -= 1,
            _ => {}
        }
        Some(style)
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
            if let Some(Some(color)) = face_ctx.named(i, &cursor, 0).bg {
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

fn frame_scroll_bar_style(
    i: &Interp,
    face_ctx: &mut FaceCtx,
    frame: &crate::editor::Frame,
) -> CellStyle {
    let mut style = face_ctx.builtin(i, "scroll-bar", 0).finish();
    let parameter_color = |name: &str| {
        i.q_soft(name).and_then(|key| {
            face_ctx.color(&crate::lisp::eval::plist_get_v(&frame.params, &Value::Sym(key)))
        })
    };
    if let Some(color) = parameter_color("scroll-bar-foreground") {
        style.fg = Some(color);
    }
    if let Some(color) = parameter_color("scroll-bar-background") {
        style.bg = Some(color);
    }
    style.fg.get_or_insert(CellColor(0x74, 0x7a, 0x80));
    style.bg.get_or_insert(CellColor(0x30, 0x34, 0x38));
    style
}

fn blink_cursor_settings(i: &Interp) -> (bool, f64, f64, i128) {
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
    let delay = i
        .q_soft("blink-cursor-delay")
        .and_then(|id| match i.symbol_value(id) {
            Value::Int(value) => Some(value as f64),
            Value::Float(value) => Some(*value),
            _ => None,
        })
        .filter(|value| *value >= 0.0)
        .unwrap_or(0.5)
        .max(0.2);
    let blinks = i
        .q_soft("blink-cursor-blinks")
        .and_then(|id| match i.symbol_value(id) {
            Value::Int(value) => Some(value),
            _ => None,
        })
        .unwrap_or(10);
    (
        enabled,
        interval.filter(|value| *value > 0.0).unwrap_or(0.5),
        delay,
        blinks,
    )
}

const MEMORY_FULL_WARNING: &str = "!MEM FULL! ";

fn buffer_state_marker(read_only: bool, modified: bool, specifier: char) -> char {
    if read_only && specifier == '*' {
        '%'
    } else if modified {
        '*'
    } else if read_only && specifier == '+' {
        '%'
    } else {
        '-'
    }
}

fn narrowing_indicator(narrowed: bool) -> &'static str {
    if narrowed { " Narrow" } else { "" }
}

fn buffer_read_only(i: &Interp, buffer: usize) -> bool {
    i.q_soft("buffer-read-only")
        .is_some_and(|symbol| i.symbol_value_in_buffer(symbol, buffer).truthy())
}

mod frame_title;
use frame_title::{FrameTitleDependencies, frame_title};

#[derive(Clone, Default)]
struct ModeLineCharacterProperties {
    help: Option<String>,
    identification: bool,
    pointer: Option<GridPointerShape>,
}

#[derive(Default)]
struct ModeLineProperties {
    characters: Vec<ModeLineCharacterProperties>,
    fill: Option<ModeLineCharacterProperties>,
}

#[derive(Default)]
pub(super) struct ModeLineContext {
    buffer_name: String,
    file_name: String,
    mode_name: String,
    modified: bool,
    read_only: bool,
    memory_full: bool,
    remote: bool,
    remote_used: std::cell::Cell<bool>,
    properties: std::cell::RefCell<Option<ModeLineProperties>>,
    line: usize,
    column: usize,
    line_count: usize,
    buffer_size: usize,
    window_start_offset: usize,
    window_end_offset: usize,
    narrowed: bool,
}

pub(super) fn mode_line_variable(i: &Interp, buffer_id: usize, name: &str) -> Option<Value> {
    let id = i.q_soft(name)?;
    i.buffer_local_toplevel_value(id, buffer_id)
        .or_else(|| i.default_toplevel_value(id))
}

pub(super) fn prepare_mode_line_remote(
    i: &mut Interp,
    windows: Vec<crate::editor::WindowRef>,
) -> Result<(), crate::lisp::Flow> {
    let caller = i.current_buffer;
    let match_data = i.match_data.clone();
    let result = (|| {
        let mut seen = HashSet::new();
        let mut buffers = Vec::new();
        let window_ids: HashSet<_> = windows
            .iter()
            .filter_map(|window| {
                let window = window.borrow();
                (!window.dead).then_some(window.id)
            })
            .collect();
        // Native titles use the frame-selected window, even when mode-line
        // formats contain no remote construct. Probe the actual formatter so
        // escaped percents, field limits and inactive branches stay inert.
        for frame in &i.frames {
            let dependencies = FrameTitleDependencies::default();
            let graphical = crate::editor::frame_is_graphical(i, frame);
            let (buffer, width, height) = {
                let frame_state = frame.borrow();
                if frame_state.dead || !window_ids.contains(&frame_state.selected.borrow().id) {
                    continue;
                }
                let _ = frame_title(i, frame, graphical, Some(&dependencies));
                (
                    frame_state.selected.borrow().buffer,
                    frame_state.width,
                    frame_state.height,
                )
            };
            if dependencies.position.get() {
                // Position text can change whether a following %@ fits. Settle
                // the viewport before collecting the title's remote dependency.
                let _ = render_frame_grid_with_input_cached(i, frame, width, height, None);
                dependencies.remote.set(false);
                let _ = frame_title(i, frame, graphical, Some(&dependencies));
            }
            if dependencies.remote.get() && i.buffer_live(buffer) && seen.insert(buffer) {
                buffers.push(buffer);
            }
        }
        for window in &windows {
            let (buffer, skip) = {
                let window = window.borrow();
                (window.buffer, window.dead || window.minibuffer)
            };
            if skip || !i.buffer_live(buffer) {
                continue;
            }
            // Use the actual formatter to collect this dependency, including
            // active conditionals, symbols, field widths and escaped percents.
            let probe = ModeLineContext::default();
            for name in ["mode-line-format", "header-line-format", "tab-line-format"] {
                let format = super::mode_line::format(i, buffer, window.borrow().id, name);
                let _ = format_mode_line_value(i, buffer, &format, &probe, 0, &mut false);
            }
            if !probe.remote_used.get() || !seen.insert(buffer) {
                continue;
            }
            buffers.push(buffer);
        }
        for buffer in buffers {
            prepare_mode_line_remote_buffer(i, buffer)?;
        }
        frame_title::prepare(i, &windows, &mut seen)?;
        Ok(())
    })();
    if i.buffer_live(caller) {
        i.set_current_buffer(caller);
    }
    i.match_data = match_data;
    result
}

fn prepare_mode_line_remote_buffer(i: &mut Interp, buffer: usize) -> Result<(), crate::lisp::Flow> {
    let caller = i.current_buffer;
    let matches = i.match_data.clone();
    i.set_current_buffer(buffer);
    let result = (|| {
        let directory_symbol = i.q("default-directory");
        let directory = i.symbol_value(directory_symbol);
        let snapshot = match &directory {
            Value::Str(text) => Value::string(text.borrow().clone()),
            _ => Value::Nil,
        };
        let remote = if matches!(directory, Value::Str(_)) {
            let function = Value::Sym(i.q("file-remote-p"));
            super::mode_line::call_for_redisplay(i, function, vec![directory])
        } else {
            Ok(Value::Nil)
        };
        let cache = i.intern("remacs--mode-line-remote-cache");
        if i.buffer_live(buffer) {
            let value = Value::cons(
                snapshot,
                Value::from_bool(remote.as_ref().is_ok_and(Value::truthy)),
            );
            i.buffers
                .get(buffer)
                .unwrap()
                .borrow_mut()
                .locals
                .insert(cache, value);
        }
        remote?;
        Ok(())
    })();
    if i.buffer_live(caller) {
        i.set_current_buffer(caller);
    }
    i.match_data = matches;
    result
}

fn mode_line_remote(i: &Interp, buffer: usize) -> bool {
    let directory = mode_line_variable(i, buffer, "default-directory").unwrap_or(Value::Nil);
    buffer_directory_remote(i, buffer, &directory)
}

fn buffer_directory_remote(i: &Interp, buffer: usize, current: &Value) -> bool {
    buffer_directory_remote_cached(i, buffer, current).unwrap_or(false)
}

fn buffer_directory_remote_cached(i: &Interp, buffer: usize, current: &Value) -> Option<bool> {
    let Some(Value::Cons(cache)) = mode_line_variable(i, buffer, "remacs--mode-line-remote-cache")
    else {
        return None;
    };
    let (directory, remote) = {
        let cache = cache.borrow();
        (cache.car.clone(), cache.cdr.truthy())
    };
    let (Value::Str(current), Value::Str(saved)) = (current, directory) else {
        return None;
    };
    let same_directory = *current.borrow() == *saved.borrow();
    same_directory.then_some(remote)
}

fn mode_line_context(
    i: &Interp,
    buffer_id: usize,
    point: usize,
    window_start: usize,
    window_end: usize,
    tab_width: usize,
) -> ModeLineContext {
    let Some(buffer) = i.buffers.get(buffer_id) else {
        return ModeLineContext {
            buffer_name: "???".to_owned(),
            file_name: String::new(),
            mode_name: String::new(),
            modified: false,
            read_only: false,
            memory_full: false,
            remote: false,
            remote_used: Default::default(),
            properties: Default::default(),
            line: 0,
            column: 0,
            line_count: 0,
            buffer_size: 0,
            window_start_offset: 0,
            window_end_offset: 0,
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
        read_only: buffer_read_only(i, buffer_id),
        memory_full: mode_line_variable(i, buffer_id, "memory-full")
            .is_some_and(|value| value.truthy()),
        remote: mode_line_remote(i, buffer_id),
        remote_used: Default::default(),
        properties: Default::default(),
        line: point_line.saturating_sub(first_line).saturating_add(1),
        column: display_columns(
            &buffer.text.substring(point_line_start, point),
            tab_width,
        ),
        line_count: last_line.saturating_sub(first_line).saturating_add(1),
        buffer_size: buffer.zv.saturating_sub(buffer.begv),
        window_start_offset: window_start.clamp(buffer.begv, buffer.zv) - buffer.begv,
        window_end_offset: window_end.clamp(buffer.begv, buffer.zv) - buffer.begv,
        narrowed: buffer.begv > 0 || buffer.zv < buffer.size(),
    }
}

fn mode_line_position(context: &ModeLineContext, specifier: char) -> String {
    buffer_position_indicator(
        context.buffer_size,
        context.window_start_offset,
        context.window_end_offset,
        specifier,
    )
}

fn buffer_position_indicator(size: usize, start: usize, end: usize, specifier: char) -> String {
    // GNU percent99 rounds upward; 100 is reserved for the ending bound of %q.
    let percent99 = |position: usize, total: usize| {
        ((position as u128 * 100 + total.max(1) as u128 - 1) / total.max(1) as u128).min(99)
    };
    if specifier == 'q' {
        if start == 0 && end >= size {
            return "All   ".to_owned();
        }
        let top = if start == 0 {
            0
        } else {
            percent99(start, size)
        };
        let bottom = if end >= size {
            100
        } else {
            percent99(end, size)
        };
        return if top == bottom {
            format!("{top}%")
        } else {
            format!("{top}-{bottom}%")
        };
    }
    if end >= size {
        return if start == 0 { "All" } else { "Bottom" }.to_owned();
    }
    if matches!(specifier, 'p' | 'o') && start == 0 {
        return "Top".to_owned();
    }
    let percentage = if specifier == 'o' {
        percent99(start, start.saturating_add(size.saturating_sub(end)))
    } else if specifier == 'P' {
        percent99(end, size)
    } else {
        percent99(start, size)
    };
    let prefix = if specifier == 'P' && start == 0 {
        "Top"
    } else {
        ""
    };
    format!("{prefix}{percentage:2}%")
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
        .and_then(|marker| arrow_marker(mar…50474 tokens truncated…content_left_with_margin, content_top))
                }
                _ => None,
            };
            if let Some((left, top)) = origin {
                let (x_offset, y_offset) = crate::editor::winxtra::pixelwise_window_origin_offset(w);
                popup_pixel_position = Some((
                    (left.saturating_mul(frame_char_width) as i128)
                        .saturating_add(x_offset)
                        .saturating_add(position.x),
                    (top.saturating_mul(frame_char_height) as i128)
                        .saturating_add(y_offset)
                        .saturating_add(position.y),
                ));
            }
        }
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
        let mut point_line_hscroll = None;
        let mut hide_offscreen_cursor = false;
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
                let pt = (if use_buffer_point {
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
                let auto_hscroll_mode = i
                    .buffer_local_toplevel_value(ahs_id, buf_id)
                    .or_else(|| i.default_toplevel_value(ahs_id))
                    .unwrap_or_else(Value::t);
                let ahs_on = auto_hscroll_mode.truthy();
                let current_line_only = matches!(
                    &auto_hscroll_mode,
                    Value::Sym(mode) if i.symbol_name(*mode) == "current-line"
                );
                hide_offscreen_cursor = graphical && !wrap_lines && !ahs_on;
                let new_hscroll = if wrap_lines {
                    0
                } else if current_line_only {
                    // Keep the current line's automatic offset in the
                    // window state; other lines render at min_hscroll.
                    let old_point_line = bb.text.line_of_pos(old_pt.clamp(bb.begv, bb.zv));
                    let base_hscroll = if old_point_line == pt_line {
                        hscroll.max(min_hs)
                    } else {
                        min_hs
                    };
                    let line_hscroll = if content_width > 0 && ahs_on && !suspend_hs {
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
                            base_hscroll,
                            content_width,
                            margin,
                            hscroll_step_setting(i, buf_id),
                            min_hs,
                        )
                        .unwrap_or(base_hscroll)
                    } else {
                        base_hscroll
                    };
                    point_line_hscroll = Some((pt_line, line_hscroll, min_hs));
                    line_hscroll
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
                        style: scroll_bar_style,
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
                        style: scroll_bar_style,
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
            .map(|b| window_faces(i, &mut fctx, &b, &Value::Window(w.clone()), mouse_position));
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
        if let Some(position) = mouse_position
            && position.buffer_id == buf_id
        {
            hover_cursor = Some(grid_pointer_shape(
                i,
                wf.as_ref().and_then(|faces| faces.pointer.as_ref()),
            ));
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
            if screen_row >= body_height || screen_row >= content_bottom {
                break;
            }
            let (
                mut line,
                line_pos,
                row_end,
                mut point_index,
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
                    let point = if use_buffer_point {
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
            let mut hidden_selective_suffix = false;
            let hidden_start = if matches!(selective_display, Value::Sym(symbol) if i.symbol_name(symbol).as_str() == "t")
            {
                line.chars()
                    .position(|character| character.unicode() == Some('\r'))
            } else {
                None
            };
            if let Some(hidden_start) = hidden_start {
                hidden_selective_suffix = hidden_start < line.chars().count();
                let visible_line: EmacsText = line.chars().take(hidden_start).collect();
                line = visible_line;
                point_index = point_index.map(|point| point.min(hidden_start));
            }
            let selective_ellipsis = buffer_setting(i, "selective-display-ellipses", buf_id)
                .truthy()
                && (hidden_selective_suffix
                    || selective_display_next_line_is_hidden(
                        i,
                        buf_id,
                        line_number,
                        &selective_display,
                        tab_width,
                    ));
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
            // Fringe display specifications consume source characters but no
            // text columns. Keep their source indices for wrapping and input.
            let metric_line: Option<EmacsText> = wf
                .as_ref()
                .filter(|faces| {
                    faces
                        .display_props
                        .iter()
                        .any(|(start, end, replacement, _)| {
                            *start < line_pos.saturating_add(line_len)
                                && *end > line_pos
                                && matches!(
                                    replacement,
                                    Some(DisplayReplacement::Fringe(_) | DisplayReplacement::Hidden)
                                )
                        })
                })
                .map(|faces| {
                    line.chars()
                        .enumerate()
                        .map(|(index, character)| {
                            let position = line_pos + index;
                            let zero_width = faces
                                .display_props
                                .iter()
                                .filter(|(start, end, _, _)| position >= *start && position < *end)
                                .max_by_key(|(_, _, _, order)| *order)
                                .is_some_and(|(_, _, replacement, _)| {
                                    matches!(
                                        replacement,
                                        Some(DisplayReplacement::Fringe(_) | DisplayReplacement::Hidden)
                                    )
                                });
                            if zero_width {
                                EmacsChar::new(0x200b).unwrap()
                            } else {
                                character
                            }
                        })
                        .collect()
                });
            let metric_line = metric_line.as_ref().unwrap_or(&line);
            let first_character_width = metric_line.chars().next().map_or(0, |character| {
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
                    metric_line,
                    content_width,
                    line_prefix_width,
                    tab_width,
                    ctl_arrow,
                )
            } else {
                vec![0..line_len]
            };
            let max_rows = body_height
                .min(content_bottom)
                .saturating_sub(screen_row);
            let mut plans = Vec::new();
            for mut range in ranges {
                if range.end < start_offset || (range.end == start_offset && range.end < line_len) {
                    continue;
                }
                if plans.len() == max_rows {
                    break;
                }
                range.start = range.start.max(start_offset);
                plans.extend(display_row_plans(
                    i,
                    wf.as_ref(),
                    line_pos,
                    range,
                    line_len,
                    max_rows - plans.len(),
                ));
            }
            if selective_ellipsis
                && let Some(plan) = plans
                    .iter_mut()
                    .rev()
                    .find(|plan| plan.include_end_boundary)
            {
                plan.boundaries
                    .entry(line_len)
                    .or_default()
                    .push("…".to_owned());
            }
            for plan in plans {
                let range = &plan.range;
                if range.end < start_offset
                    || (range.end == start_offset && range.end < line_len
                        && !(range.is_empty() && plan.include_end_boundary))
                {
                    continue;
                }
                if screen_row >= body_height || screen_row >= content_bottom {
                    break;
                }
                let segment_start = range.start.max(start_offset);
                let segment_end = range.end;
                let segment_len = segment_end.saturating_sub(segment_start);
                let prefix_width = if segment_start == 0 && !plan.continued {
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
                        && !(plan.repeated_start && point == range.start)
                        && (point < segment_end
                            || (point == segment_end && segment_end == line_len && !plan.hard_break)))
                        .then(|| point - segment_start)
                });
                let y = screen_row;
                let newline_spacing = if graphical
                    && !plan.hard_break
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
                    && !plan.hard_break
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
                visible_end = if segment_end == line_len && !plan.hard_break {
                    row_end
                } else {
                    line_pos + segment_end
                };
                let row_hscroll = if wrap_lines {
                    0
                } else if let Some((point_line, point_scroll, other_scroll)) = point_line_hscroll {
                    if line_number.saturating_sub(1) == point_line {
                        point_scroll
                    } else {
                        other_scroll
                    }
                } else {
                    hscroll
                };
                if gutter_width > 0 {
                    let number_style = if point_line_number + 1 == line_number {
                        fctx.builtin(i, "line-number-current-line", 0).finish()
                    } else {
                        fctx.builtin(i, "line-number", 0).finish()
                    };
                    let show_number = (segment_start == 0 || (row == 0 && start_offset > 0))
                        && !plan.continued;
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
                        plan.replacements.get(&(segment_start + index)).cloned()
                            .or_else(|| wf.as_ref()
                                .and_then(|wf| display_replacement_at(wf, segment_pos + index)))
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
                        plan.boundaries.get(&(segment_start + index)).cloned()
                            .unwrap_or_else(|| wf.as_ref()
                                .map(|wf| overlay_strings_at(wf, segment_pos + index))
                                .unwrap_or_default())
                    },
                    plan.include_end_boundary,
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
                if segment_end == line_len
                    && !wrap_lines
                    && display_columns(&line, tab_width)
                        > row_hscroll.saturating_add(text_width)
                {
                    // For a truncated line, the logical row ends beyond the
                    // last glyph in the display row.  Limit `window-end' to
                    // the last source character that produced visible text.
                    let rendered_end = rendered
                        .source_offsets
                        .windows(2)
                        .rposition(|offsets| offsets[1] > offsets[0])
                        .map_or(0, |index| index + 1);
                    let visible_prefix = rendered
                        .source_columns
                        .iter()
                        .take(segment_len)
                        .take_while(|column| {
                            **column
                                < row_hscroll
                                    .saturating_add(prefix_width)
                                    .saturating_add(text_width)
                        })
                        .count();
                    visible_end = segment_pos + rendered_end.min(visible_prefix);
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
                    && display_columns(metric_line, tab_width) > row_hscroll.saturating_add(text_width)
                {
                    right_indicator = "›".to_owned();
                }
                let overlay_arrow = overlay_arrows.iter().find(|(position, _)| {
                    !(plan.repeated_start && *position == segment_pos)
                        && !(plan.hard_break && *position == segment_pos + segment_len)
                        && *position >= segment_pos
                        && (*position < segment_pos.saturating_add(segment_len)
                            || (*position == segment_pos.saturating_add(segment_len)
                                && segment_end == line_len))
                });
                if let Some((_, arrow)) = overlay_arrow {
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
                // User display specifications override automatic indicators.
                // The last specification encountered on each side wins.
                for left in [true, false] {
                    // GNU paints the overlay arrow over a left user bitmap.
                    if left && overlay_arrow.is_some() {
                        continue;
                    }
                    let Some(fringe) = rendered
                        .fringes
                        .iter()
                        .rev()
                        .find(|fringe| fringe.left == left)
                    else {
                        continue;
                    };
                    let (column, cells, pixels) = if left {
                        (window_left, fringe_left_cells, fringe_left_pixels)
                    } else {
                        (right_fringe_left, fringe_right_cells, fringe_right_pixels)
                    };
                    if cells == 0 {
                        continue;
                    }
                    let pixel_width = pixels.min(cells.saturating_mul(frame_char_width));
                    if let Some(bitmap) = super::fringe::get(fringe.bitmap) {
                        if let Some(row) = screen_rows.get_mut(y) {
                            for cell in row.iter_mut().skip(column).take(cells) {
                                *cell = ScreenCell::new(fringe_style);
                            }
                        }
                        fringe_bitmaps.push(GridFringeBitmap {
                            row: y,
                            column,
                            pixel_width,
                            bits: Arc::from(bitmap.rows),
                            align: bitmap.align,
                            repeat: bitmap.repeat,
                            style: fringe.style,
                        });
                    }
                }
                images.extend(rendered.images.iter().filter_map(|image| {
                    let column = content_left.saturating_add(image.column);
                    if column >= width {
                        return None;
                    }
                    Some(GridImage {
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
                        let end_face = style_at_pos(i, &mut fctx, wf, end_pos);
                        let tail = if end_face.extend || line_len == 0 {
                            end_face
                        } else {
                            style_at_pos(i, &mut fctx, wf, end_pos - 1)
                        };
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
                    let point_visible = point_col >= row_hscroll
                        && point_col.saturating_sub(row_hscroll) < content_width;
                    let hide_cursor = hide_offscreen_cursor && !point_visible;
                    if selected_frame && selected_window && !hide_cursor {
                        selected_cursor = Some((y, content_left, col, byte_index));
                    }
                    if !hide_cursor
                        && let Some(cursor_style) = cursor_style
                    {
                        let mut shape = cursor_style.shape;
                        if shape == GridCursorShape::Block
                            && point_visible
                            && let Some(image) = rendered
                                .images
                                .iter()
                                .find(|image| image.column == col)
                        {
                            let large_masked_image = cursor_style.box_size.is_some_and(|size| {
                                let (width, height) = image.cursor_dimensions();
                                let size = size as f64;
                                image.masked
                                    && (width.is_some_and(|width| width > size)
                                        || height.is_some_and(|height| height > size))
                            });
                            if large_masked_image {
                                shape = GridCursorShape::Hollow;
                            }
                        }
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
                                thickness: cursor_style.thickness,
                                color: cursor_color,
                                selected: selected_frame && selected_window,
                            },
                        content_left,
                        ));
                    }
                }
                if let Some(snapshot) = input.as_mut() {
                    // HSCROLL makes the source byte offsets of hidden
                    // characters tie with the first visible character.
                    // Skip that hidden prefix when resolving mouse hits.
                    let scroll_origin = row_hscroll.saturating_add(prefix_width);
                    let first_visible_source = rendered
                        .source_columns
                        .windows(2)
                        .position(|columns| {
                            columns[0] >= scroll_origin || columns[1] > scroll_origin
                        })
                        .unwrap_or_else(|| rendered.source_columns.len().saturating_sub(1));
                    snapshot.add_hit_test_row(
                        i,
                        y,
                        buf_id,
                        segment_pos,
                        rendered.source_offsets.clone(),
                        first_visible_source,
                        Some(window_id),
                        content_left,
                        content_left.saturating_add(content_width),
                        content_left,
                    );
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
                if !bidi_display_reordering && content_width > 0 {
                    bidi_disabled_cell_ranges.push((
                        y,
                        content_left,
                        content_left.saturating_add(content_width).min(width),
                    ));
                }
                if let Some(direction) = bidi_paragraph_direction
                    && content_width > 0
                {
                    bidi_paragraph_cell_ranges.push((
                        y,
                        content_left,
                        content_left.saturating_add(content_width).min(width),
                        direction,
                    ));
                }
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
            for row in screen_row..content_bottom.min(body_height) {
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
        let mode_point = i.buffers.get(buf_id).map_or(0, |buffer| {
            let bb = buffer.borrow();
            (if use_buffer_point {
                bb.point()
            } else {
                w.borrow().point
            })
            .clamp(bb.begv, bb.zv)
        });
        let mode_context = mode_line_context(i, buf_id, mode_point, start, visible_end, tab_width);
        let mut auxiliary_row = window_top;
        if tab_line_enabled && auxiliary_row < content_top {
            let _ = place_formatted_window_row(
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
                &mut help_echoes,
                &mut pointer_regions,
            );
            auxiliary_row += 1;
        }
        if header_line_enabled && auxiliary_row < content_top {
            let _ = place_formatted_window_row(
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
                &mut help_echoes,
                &mut pointer_regions,
            );
            header_lines.push(GridHeaderLine {
                window_id,
                buffer_id: buf_id,
                row: auxiliary_row,
                left: window_left,
                width: window_width,
            });
        }
        // Mode line.
        let my = content_bottom;
        if mode_line_enabled && my < body_height {
            // GNU draws mode lines under `mode-line-active' on the
            // selected window, `mode-line-inactive' elsewhere — both
            // resolve through `mode-line' (inverse on a mono tty).
            // An unresolved (pre-faces.el) mode-line face falls back
            // to plain inverse so the row still reads as a divider.
            let ml_name = if selected_frame && selected_window {
                "mode-line-active"
            } else {
                "mode-line-inactive"
            };
            let buffer_name_range = place_formatted_window_row(
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
                &mut help_echoes,
                &mut pointer_regions,
            );
            mode_lines.push(crate::frontend::GridModeLine {
                window_id,
                buffer_id: buf_id,
                row: my,
                left: window_left,
                width: window_width,
                buffer_name_range,
            });
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
    if i.minibuf_level > 0 && mini_height > 0 && let Some(minibuffer) = &fb.minibuffer {
        window_text_areas.push(GridWindowTextArea {
            window_id: minibuffer.borrow().id,
            left: 0,
            top: mini_top,
            width,
            rows: mini_height,
        });
    }
    if let Some(menu) = &i.gui_popup_menu
        && menu.position.frame_id == fb.id
        && let Some(minibuffer) = &fb.minibuffer
        && matches!(menu.position.anchor,
            super::PopupMenuAnchor::Window(id) | super::PopupMenuAnchor::TextArea(id)
                if id == minibuffer.borrow().id)
    {
        let position = &menu.position;
        popup_pixel_position = Some((
            position.x,
            (mini_top.saturating_mul(frame_char_height) as i128).saturating_add(position.y),
        ));
    }
    if mini_visible_rows > 0 {
        let wf =
            mini_styled.map(|buffer| window_faces(i, &mut fctx, buffer, &Value::Nil, mouse_position));
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
            images.extend(rendered.images.iter().map(|image| {
                GridImage {
                    row: y,
                    column: image.column,
                    source: image.source.clone(),
                    width: image.width,
                    height: image.height,
                }
            }));
            if let (Some(snapshot), Some(buffer)) = (input.as_mut(), mini_buf.as_ref()) {
                let buffer_id = buffer.borrow().id;
                // Echo-area messages do not live in the minibuffer buffer.
                // Map the whole message row to its empty buffer position so
                // clicks still identify the minibuffer window.
                let (buffer_start, source_offsets) = if mini_styled.is_some() {
                    (range.start, rendered.source_offsets.clone())
                } else {
                    (0, vec![0])
                };
                snapshot.add_hit_test_row(
                    i,
                    y,
                    buffer_id,
                    buffer_start,
                    source_offsets,
                    0,
                    fb.minibuffer.as_ref().map(|window| window.borrow().id),
                    0,
                    width,
                    0,
                );
                if snapshot.buffer_id == buffer_id {
                    snapshot.rows.push(GridInputRow {
                        row: y,
                        buffer_start: range.start,
                        source_offsets: rendered.source_offsets.clone(),
                    });
                    input_screen_lefts.push((snapshot.rows.len() - 1, 0));
                }
            }
            place_rendered_row(&mut screen_rows[y], 0, &rendered);
        }
    }
    if graphical
        && let Some(menu) = &i.gui_popup_menu
        && let Some(pixel_position) = popup_pixel_position
    {
        popup_menu_items = render_popup_menu(
            i,
            menu,
            pixel_position,
            width,
            height.saturating_sub(mini_height),
            frame_char_width,
            frame_char_height,
            default_style,
            &mut fctx,
            &mut screen_rows,
        );
    }
    pixelwise_row_boundaries.sort_unstable_by_key(|(row, offset)| (*row, offset.unsigned_abs()));
    let mut boundary_offsets = vec![None; row_heights.len()];
    let mut entries = pixelwise_row_boundaries.into_iter().peekable();
    while let Some((row, offset)) = entries.next() {
        let row = row.min(row_heights.len().saturating_sub(1));
        let mut chosen = offset;
        while entries.peek().is_some_and(|(next_row, _)| *next_row == row) {
            if let Some((_, candidate)) = entries.next()
                && candidate.unsigned_abs() > chosen.unsigned_abs()
            {
                chosen = candidate;
            }
        }
        if let Some(boundary) = boundary_offsets.get_mut(row) {
            *boundary = Some(chosen);
        }
    }
    let mut previous_offset = 0i128;
    for (row, boundary) in boundary_offsets.into_iter().enumerate() {
        let Some(offset) = boundary else {
            continue;
        };
        let delta = offset - previous_offset;
        if let Some(height) = row_heights.get_mut(row) {
            *height = (*height as i128 + delta).clamp(1, usize::MAX as i128) as usize;
        }
        previous_offset = offset;
    }
    let (composed_rows, composed_attrs, composed_char_attrs, cell_offsets) =
        compose_screen_rows(&screen_rows, width, default_style);
    if graphical {
        let default_font_height = default_style
            .font_height
            .map(usize::from)
            .unwrap_or(140)
            .max(1);
        for (row, (text, styles)) in composed_rows
            .iter()
            .zip(&composed_char_attrs)
            .enumerate()
        {
            let font_height = styles.iter().take(text.chars().count())
                .filter_map(|style| style.font_height).max();
            if let Some(font_height) = font_height
                && let Some(row_height) = row_heights.get_mut(row)
            {
                let scaled_height = frame_char_height
                    .saturating_mul(usize::from(font_height))
                    .div_ceil(default_font_height)
                    .max(1);
                *row_height = (*row_height).max(scaled_height);
            }
        }
    }
    rows = composed_rows;
    attrs = composed_attrs;
    char_attrs = composed_char_attrs;
    pixelwise_cell_segments.sort_unstable_by_key(|(row, left, right, _, _)| {
        (*row, *left, *right)
    });
    let mut pixelwise_segments = Vec::<GridPixelwiseSegment>::new();
    for (row, left, right, x_offset, width_adjustment) in pixelwise_cell_segments {
        let Some(offsets) = cell_offsets.get(row) else {
            continue;
        };
        let (Some(start), Some(end)) = (offsets.get(left), offsets.get(right)) else {
            continue;
        };
        if start >= end {
            continue;
        }
        pixelwise_segments.push(GridPixelwiseSegment {
            row,
            start: *start,
            end: *end,
            left_column: left,
            right_column: right,
            x_offset,
            width_adjustment,
        });
    }
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
    bidi_disabled_cell_ranges.sort_unstable();
    let mut bidi_disabled_ranges = Vec::<GridBidiDisabledRange>::new();
    for (row, left, right) in bidi_disabled_cell_ranges {
        let Some(offsets) = cell_offsets.get(row) else {
            continue;
        };
        let (Some(start), Some(end)) = (offsets.get(left), offsets.get(right)) else {
            continue;
        };
        if start >= end {
            continue;
        }
        if let Some(previous) = bidi_disabled_ranges.last_mut()
            && previous.row == row
            && *start <= previous.end
        {
            previous.end = previous.end.max(*end);
        } else {
            bidi_disabled_ranges.push(GridBidiDisabledRange {
                row,
                start: *start,
                end: *end,
            });
        }
    }
    bidi_paragraph_cell_ranges.sort_unstable_by_key(|(row, left, right, direction)| {
        (*row, *left, *right, *direction)
    });
    let mut bidi_paragraph_ranges = Vec::<GridBidiParagraphRange>::new();
    for (row, left, right, direction) in bidi_paragraph_cell_ranges {
        let Some(offsets) = cell_offsets.get(row) else {
            continue;
        };
        let (Some(start), Some(end)) = (offsets.get(left), offsets.get(right)) else {
            continue;
        };
        if start >= end {
            continue;
        }
        if let Some(previous) = bidi_paragraph_ranges.last_mut()
            && previous.row == row
            && previous.direction == direction
            && *start <= previous.end
        {
            previous.end = previous.end.max(*end);
        } else {
            bidi_paragraph_ranges.push(GridBidiParagraphRange {
                row,
                start: *start,
                end: *end,
                direction,
            });
        }
    }
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
        for row in &mut input.hit_test_rows {
            let base = cell_offsets[row.row][row.screen_left.min(width)];
            for offset in &mut row.source_offsets {
                *offset += base;
            }
        }
        let active_buffer = input.buffer_id;
        let visible_buffers = input
            .hit_test_rows
            .iter()
            .map(|row| row.buffer_id)
            .collect::<HashSet<_>>();
        input
            .hit_test_char_offsets
            .retain(|buffer_id, _| {
                *buffer_id == active_buffer || visible_buffers.contains(buffer_id)
            });
    }
    for item in &mut popup_menu_items {
        if let Some(offsets) = cell_offsets.get(item.row)
            && let (Some(start), Some(end)) = (
                offsets.get(item.left),
                offsets.get(item.left.saturating_add(item.width)),
            )
        {
            item.byte_range = Some((*start, *end));
        }
    }
    for item in &mut tab_bar_items {
        if let Some(offsets) = cell_offsets.get(item.row)
            && let (Some(start), Some(end)) = (
                offsets.get(item.left),
                offsets.get(item.left.saturating_add(item.width)),
            )
        {
            item.byte_range = Some((*start, *end));
        }
    }
    for item in &mut tool_bar_items {
        if let Some(offsets) = cell_offsets.get(item.row)
            && let (Some(start), Some(end)) = (
                offsets.get(item.left),
                offsets.get(item.left.saturating_add(item.width)),
            )
        {
            item.byte_range = Some((*start, *end));
        }
    }
    let mut grid = Grid {
        // Redisplay has now published each window's actual endpoint, including
        // wrapping, narrowing, header rows and scrolling in inactive frames.
        title: frame_title(i, &frame, graphical, None),
        font_families: fctx.font_families,
        default_style,
        internal_border_style,
        rows,
        images,
        fringe_bitmaps,
        fill_column_indicators,
        stipples: fctx.stipples,
        menus,
        tool_bar_items,
        tab_bar_items,
        help_echoes,
        pointer_regions,
        tooltip,
        popup_menu_items,
        scroll_bars,
        bottom_dividers,
        right_dividers,
        mode_lines,
        header_lines,
        window_text_areas,
        vscrolls,
        compositions,
        bidi_disabled_ranges,
        bidi_paragraph_ranges,
        frame_resize_pixelwise,
        pixelwise_segments,
        attrs,
        char_attrs,
        mode_rows,
        cursor: cursor_pos,
        cursors,
        cursor_index,
        blink_cursor,
        blink_interval,
        blink_delay,
        blink_blinks,
        blink_elapsed_since_input: 0.0,
        default_bg,
        gamma_exponent,
        row_heights,
        default_line_height,
        default_row_height,
        hover_cursor,
        freeze_pointer_shape,
        pointer_visible: fb.pointer_visible,
        input,
    };
    resolve_grid_colors(&mut grid, gamma_exponent, face_near_same_color_threshold(i));
    grid
}

#[cfg(test)]
mod mode_line_size_tests {
    use super::readable_mode_line_size;

    #[test]
    fn decimal_rounding_and_large_units() {
        for (size, expected) in [
            (0u128, "0"),
            (999, "999"),
            (1000, "1.0k"),
            (1049, "1.0k"),
            (1050, "1.1k"),
            (9949, "9.9k"),
            (9950, "10k"),
            (10000, "10k"),
            (10500, "11k"),
            (999499, "999k"),
            (999500, "1.0M"),
            (1049999, "1.0M"),
            (1050000, "1.1M"),
            (9950000, "10M"),
            (1000000000, "1.0G"),
            (1500000000, "1.5G"),
            (1000000000000, "1.0T"),
            (1000000000000000, "1.0P"),
            (999500000000000000, "1.0E"),
            (9223372036854775807, "9.2E"),
            (18446744073709551615, "18E"),
        ] {
            if let Ok(size) = usize::try_from(size) {
                assert_eq!(readable_mode_line_size(size), expected, "{size}");
            }
        }
    }
}
