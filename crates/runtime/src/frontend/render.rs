//! Resolve faces and render editor state into a frontend-neutral grid.
use super::{MouseObject, MouseObjectSource, MousePosition};
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use crate::{
    buffer::Buffer,
    lisp::{Interp, Value, value::SymId},
};
use num_traits::{Signed, ToPrimitive};
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
    pub overline_color: Option<CellColor>,
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
    /// Source Lisp property for `posn-image` hit testing.
    pub object_source: Option<MouseObjectSource>,
    pub source: GridImageSource,
    /// Selected zero-based frame for multi-frame images.
    pub index: Option<usize>,
    /// Conversion requested by the image descriptor.
    pub conversion: Option<GridImageConversion>,
    /// Whether transformed pixels should be smoothed, when explicitly set.
    pub transform_smoothing: Option<bool>,
    /// Source pixel limit captured from `max-image-size` for this render.
    pub max_image_size: Option<GridImageSizeLimit>,
    /// Dimensions explicitly requested by the image descriptor.
    pub width: Option<usize>,
    pub height: Option<usize>,
    /// Intrinsic dimensions available for SVG images.
    pub intrinsic_width: Option<usize>,
    pub intrinsic_height: Option<usize>,
    /// Image-map hotspots, in the displayed image's pixel coordinates.
    pub map: Vec<GridImageMapArea>,
    /// Optional source rectangle selected by a `display` slice specification.
    pub slice: Option<GridImageSlice>,
}

/// One coordinate in GNU's `(slice X Y WIDTH HEIGHT)` image specification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridImageSlicePart {
    Pixels(usize),
    /// IEEE-754 bits for a fraction of the corresponding image dimension.
    Fraction(u64),
}

impl GridImageSlicePart {
    fn resolve(self, extent: Option<usize>) -> Option<usize> {
        match self {
            Self::Pixels(pixels) => Some(pixels),
            Self::Fraction(bits) => {
                let extent = extent?;
                let fraction = f64::from_bits(bits);
                (fraction.is_finite() && (0.0..=1.0).contains(&fraction))
                    .then(|| (extent as f64 * fraction).round() as usize)
            }
        }
    }
}

/// Source rectangle selected by a `display` slice specification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridImageSlice {
    pub x: GridImageSlicePart,
    pub y: GridImageSlicePart,
    pub width: GridImageSlicePart,
    pub height: GridImageSlicePart,
}

/// Pixel coordinates resolved against an image's displayed dimensions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridImageSlicePixels {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl GridImageSlice {
    pub fn resolve(
        self,
        image_width: Option<usize>,
        image_height: Option<usize>,
    ) -> Option<GridImageSlicePixels> {
        let pixels = GridImageSlicePixels {
            x: self.x.resolve(image_width)?,
            y: self.y.resolve(image_height)?,
            width: self.width.resolve(image_width)?,
            height: self.height.resolve(image_height)?,
        };
        (pixels.width > 0 && pixels.height > 0).then_some(pixels)
    }
}

impl GridImage {
    /// Full displayed size before applying an optional source slice.
    pub fn display_dimensions(&self) -> (Option<usize>, Option<usize>) {
        image_display_dimensions(
            self.width,
            self.height,
            self.intrinsic_width,
            self.intrinsic_height,
        )
    }

    /// Visible size after applying an optional source slice.
    pub fn visible_dimensions(&self) -> (Option<usize>, Option<usize>) {
        image_visible_dimensions(
            self.width,
            self.height,
            self.intrinsic_width,
            self.intrinsic_height,
            self.slice,
        )
    }

    /// Resolve the source rectangle against the full displayed image size.
    pub fn resolved_slice(&self) -> Option<GridImageSlicePixels> {
        let dimensions = self.display_dimensions();
        self.slice?.resolve(dimensions.0, dimensions.1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridImageConversion {
    Disabled,
    EdgeDetection {
        matrix: [i32; 9],
        color_adjust: i32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridImageSizeLimit {
    Pixels(u64),
    /// The IEEE-754 bits of the frame width/height ratio.
    FrameRatioBits(u64),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GridImageMapArea {
    pub id: SymId,
    pub shape: GridImageMapShape,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GridImageMapShape {
    Rectangle {
        left: i128,
        top: i128,
        right: i128,
        bottom: i128,
    },
    Circle {
        x: i128,
        y: i128,
        radius_bits: u64,
    },
    Polygon(Vec<(i128, i128)>),
}

impl GridImageMapArea {
    pub fn contains(&self, x: i128, y: i128) -> bool {
        match &self.shape {
            GridImageMapShape::Rectangle {
                left,
                top,
                right,
                bottom,
            } => x >= *left && y >= *top && x <= *right && y <= *bottom,
            GridImageMapShape::Circle {
                x: center_x,
                y: center_y,
                radius_bits,
            } => {
                let radius = f64::from_bits(*radius_bits);
                let dx = (center_x - x) as f64;
                let dy = (center_y - y) as f64;
                dx * dx + dy * dy <= radius * radius
            }
            GridImageMapShape::Polygon(points) => {
                let Some(&(mut x0, mut y0)) = points.last() else {
                    return false;
                };
                let mut inside = false;
                for &(x1, y1) in points {
                    let previous_x = x0;
                    let previous_y = y0;
                    x0 = x1;
                    y0 = y1;
                    if x0 >= x {
                        if previous_x >= x {
                            continue;
                        }
                    } else if previous_x < x {
                        continue;
                    }
                    if y > y0 && y > previous_y {
                        continue;
                    }
                    if y < y0 + ((previous_y - y0) * (x - x0)) / (previous_x - x0) {
                        inside = !inside;
                    }
                }
                inside
            }
        }
    }
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
    overline_color: Option<Option<CellColor>>,
    strike: Option<bool>,
    strike_color: Option<Option<CellColor>>,
    inverse: Option<bool>,
    extend: Option<bool>,
}

impl StyleMix {
    fn from_cell_style(style: CellStyle) -> StyleMix {
        StyleMix {
            fg: Some(style.fg),
            bg: Some(style.bg),
            distant_foreground: Some(style.distant_foreground),
            font_family: Some(style.font_family),
            font_weight: Some(style.font_weight),
            font_width: Some(style.font_width),
            font_height: Some(style.font_height),
            font_height_scale: None,
            box_style: Some(style.box_style),
            stipple: Some(style.stipple),
            bold: Some(style.bold),
            italic: Some(style.italic),
            underline: Some(style.underline),
            underline_color: Some(style.underline_color),
            underline_wave: Some(style.underline_wave),
            overline: Some(style.overline),
            overline_color: Some(style.overline_color),
            strike: Some(style.strike),
            strike_color: Some(style.strike_color),
            inverse: Some(style.inverse),
            extend: Some(style.extend),
        }
    }

    fn overlay_on(&self, base: CellStyle) -> CellStyle {
        let mut mix = Self::from_cell_style(base);
        mix.merge(self);
        mix.finish()
    }

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
            overline_color,
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
            overline_color: self.overline_color.flatten(),
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
            && self.overline_color == o.overline_color
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
    /// RGB color for the native mouse pointer, from the frame's `mouse-color` parameter.
    pub mouse_color: Option<CellColor>,
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
    pub object: Option<MouseObject>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridInputObjectHit {
    left: usize,
    right: usize,
    object: MouseObject,
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
    object_hits: Vec<GridInputObjectHit>,
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
        let character = self
            .char_offsets
            .partition_point(|offset| *offset < requested);
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
            object: row
                .object_hits
                .iter()
                .rev()
                .find(|hit| column >= hit.left && column < hit.right)
                .map(|hit| hit.object),
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
        mut object_hits: Vec<GridInputObjectHit>,
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
        for hit in &mut object_hits {
            hit.left = hit.left.saturating_add(screen_left);
            hit.right = hit.right.saturating_add(screen_left);
        }
        object_hits.retain(|hit| hit.left < right && hit.right > left);
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
            object_hits,
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
            mouse_color: None,
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
    if matches!(ch as u32, 0x80..=0x9f) {
        4
    } else {
        UnicodeWidthChar::width(ch).unwrap_or(1)
    }
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
    continuation_prefix_width: usize,
    tab_width: usize,
    ctl_arrow: bool,
    word_wrap: bool,
    word_wrap_by_category: bool,
) -> Vec<Range<usize>> {
    if word_wrap {
        return wrap_ranges_by_word(
            text,
            width,
            first_prefix_width,
            continuation_prefix_width,
            tab_width,
            ctl_arrow,
            word_wrap_by_category,
        );
    }
    if (first_prefix_width == 0 && continuation_prefix_width == 0) || width == 0 {
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
        if index > start && column > 0 && column.saturating_add(character_width) > width {
            ranges.push(start..index);
            start = index;
            column = continuation_prefix_width.min(width);
            character_width = if character == '\t' {
                tab_width - column % tab_width
            } else if byte8 {
                4
            } else if ctl_arrow && control_character {
                2
            } else {
                display_char_width(character)
            };
        }
        column = column.saturating_add(character_width);
    }
    ranges.push(start..chars.len());
    ranges
}

fn wrap_ranges_by_word(
    text: &EmacsText,
    width: usize,
    first_prefix_width: usize,
    continuation_prefix_width: usize,
    tab_width: usize,
    ctl_arrow: bool,
    by_category: bool,
) -> Vec<Range<usize>> {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return vec![0..0];
    }
    if width == 0 {
        return vec![0..chars.len()];
    }
    let tab_width = tab_width.max(1);
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let mut column = if start == 0 {
            first_prefix_width
        } else {
            continuation_prefix_width
        }
        .min(width);
        let mut end = start;
        while end < chars.len() {
            let character = chars[end];
            let code = character.code();
            let unicode = character.unicode().unwrap_or('\u{fffd}');
            let char_width = if unicode == '\t' {
                tab_width - column % tab_width
            } else if character.byte8().is_some() {
                4
            } else if ctl_arrow && (code < 0x20 || code == 0x7f) {
                2
            } else {
                display_char_width(unicode)
            };
            if end > start && column.saturating_add(char_width) > width {
                break;
            }
            column = column.saturating_add(char_width);
            end += 1;
        }
        if end == start {
            end += 1;
        }
        if end < chars.len() {
            let split_inside_word = word_wrap_categories_equal(chars[end - 1], chars[end]);
            if split_inside_word {
                let boundary = (start + 1..end)
                    .rev()
                    .find(|boundary| wrap_word_boundary_before(&chars, *boundary, by_category));
                if let Some(boundary) = boundary {
                    end = boundary;
                }
            } else if chars[end].unicode().is_some_and(char::is_whitespace) {
                // The separator cannot fit at the right edge; consume it
                // with this row so the next one starts at the following word.
                end += 1;
            }
        }
        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn wrap_word_boundary_before(chars: &[EmacsChar], boundary: usize, by_category: bool) -> bool {
    if boundary == 0 || boundary >= chars.len() {
        return false;
    }
    let previous = chars[boundary - 1].unicode().unwrap_or('\u{fffd}');
    previous.is_whitespace()
        || (by_category && !word_wrap_categories_equal(chars[boundary - 1], chars[boundary]))
}

fn word_wrap_categories_equal(left: EmacsChar, right: EmacsChar) -> bool {
    fn category(character: char) -> u8 {
        if character.is_whitespace() {
            0
        } else if character.is_alphanumeric() || character == '_' {
            1
        } else {
            2
        }
    }
    let left = left.unicode().unwrap_or('\u{fffd}');
    let right = right.unicode().unwrap_or('\u{fffd}');
    category(left) == category(right)
}

fn display_prefix_value_width(i: &Interp, value: &Value, tab_width: usize) -> usize {
    match value {
        Value::Str(string) => display_columns_of(string.borrow().chars(), tab_width),
        Value::Cons(spec) => {
            let (head, tail) = {
                let spec = spec.borrow();
                (spec.car.clone(), spec.cdr.clone())
            };
            if matches!(head, Value::Sym(symbol) if i.symbol_name(symbol) == "space") {
                display_space_spec_width(i, &tail)
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub(crate) fn visual_line_ranges_for_window(
    i: &Interp,
    buffer_id: usize,
    window: &crate::editor::WindowRef,
    line: &EmacsText,
    line_pos: usize,
) -> Option<Vec<Range<usize>>> {
    let (window_buffer, total_width, hscroll) = {
        let window = window.borrow();
        (window.buffer, window.width, window.hscroll)
    };
    if window_buffer != buffer_id {
        return None;
    }
    let width = crate::editor::win_body_width(i, window).max(1);
    let root_width = crate::editor::frame_of_window(i, window)
        .map(|frame| {
            let frame = frame.borrow();
            frame
                .root
                .as_ref()
                .map(|root| root.borrow().width)
                .unwrap_or(frame.width)
        })
        .unwrap_or(total_width);
    let partial_width = if total_width < root_width {
        let setting = buffer_setting(i, "truncate-partial-width-windows", buffer_id);
        match setting.as_bigint() {
            Some(threshold) => num_bigint::BigInt::from(total_width) < threshold,
            None => setting.truthy(),
        }
    } else {
        false
    };
    if buffer_setting(i, "truncate-lines", buffer_id).truthy() || partial_width || hscroll > 0 {
        return None;
    }
    let tab_width = buffer_setting(i, "tab-width", buffer_id)
        .int()
        .and_then(|width| usize::try_from(width).ok())
        .filter(|width| *width > 0)
        .unwrap_or(8);
    let ctl_arrow = buffer_setting(i, "ctl-arrow", buffer_id).truthy();
    let window_value = Value::Window(window.clone());
    let line_prefix = line_prefix_at_pos(i, buffer_id, &window_value, line_pos);
    let requested_prefix_width = line_prefix
        .as_ref()
        .map(|prefix| display_columns(prefix, tab_width))
        .unwrap_or(0)
        .min(width);
    let first_character_width = line.chars().next().map_or(0, |character| {
        let code = character.code();
        let unicode = character.unicode().unwrap_or('\u{fffd}');
        if unicode == '\t' {
            tab_width - requested_prefix_width % tab_width.max(1)
        } else if character.byte8().is_some() {
            4
        } else if ctl_arrow && (code < 0x20 || code == 0x7f) {
            2
        } else {
            display_char_width(unicode)
        }
    });
    let first_prefix_width = if !line.is_empty()
        && requested_prefix_width.saturating_add(first_character_width) > width
    {
        0
    } else {
        requested_prefix_width
    };
    let wrap_prefix = prefix_value_at_pos(i, buffer_id, &window_value, line_pos, "wrap-prefix");
    let wrap_prefix_width = display_prefix_value_width(i, &wrap_prefix, tab_width)
        .min(width.saturating_sub(first_character_width.min(width)));
    let word_wrap = buffer_setting(i, "word-wrap", buffer_id).truthy();
    let word_wrap_by_category = buffer_setting(i, "word-wrap-by-category", buffer_id).truthy();
    Some(wrap_ranges_with_prefix(
        line,
        width,
        first_prefix_width,
        wrap_prefix_width,
        tab_width,
        ctl_arrow,
        word_wrap,
        word_wrap_by_category,
    ))
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
    let widen = buffer_setting(i, "display-line-numbers-widen", buffer).truthy()
        || line_number_offset != 0;
    let default_width = i
        .buffers
        .get(buffer)
        .map(|buffer| {
            let buffer = buffer.borrow();
            let first_line = if widen {
                0
            } else {
                buffer.text.line_of_pos(buffer.begv)
            };
            let last_line = buffer.text.line_of_pos(buffer.zv.min(buffer.text_len()));
            (last_line.saturating_sub(first_line).saturating_add(1) as i128)
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

fn prefix_value_at_pos(
    i: &Interp,
    buffer_id: usize,
    window: &Value,
    position: usize,
    variable: &str,
) -> Value {
    let property = i.q_soft(variable);
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
    overlay_value
        .or(text_value)
        .unwrap_or_else(|| buffer_setting(i, variable, buffer_id))
}

fn line_prefix_at_pos(
    i: &Interp,
    buffer_id: usize,
    window: &Value,
    position: usize,
) -> Option<EmacsText> {
    let value = prefix_value_at_pos(i, buffer_id, window, position, "line-prefix");
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

fn frame_mouse_color(
    i: &Interp,
    face_ctx: &mut FaceCtx,
    frame: &crate::editor::Frame,
) -> Option<CellColor> {
    let key = i.q_soft("mouse-color")?;
    let value = crate::lisp::eval::plist_get_v(&frame.params, &Value::Sym(key));
    if value.is_nil() {
        return None;
    }
    face_ctx.color(&value)
}

pub(crate) fn gui_face_attributes_supported(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    attributes: &Value,
) -> bool {
    let Ok(items) = attributes.list_to_vec() else {
        return false;
    };
    if items.len() % 2 != 0 {
        return false;
    }

    let mut faces = FaceCtx::new(i, Some(frame));
    let mut any_specified = false;
    for pair in items.chunks_exact(2) {
        let Value::Sym(attribute) = &pair[0] else {
            return false;
        };
        let name = i.symbol_name(*attribute);
        let Some(name) = name.strip_prefix(':') else {
            return false;
        };
        let value = &pair[1];
        if matches!(value, Value::Sym(symbol) if i.symbol_name(*symbol) == "unspecified") {
            continue;
        }
        any_specified = true;
        if matches!(value, Value::Sym(symbol) if i.symbol_name(*symbol) == "reset") {
            continue;
        }
        let supported = match name {
            "foreground" | "background" | "distant-foreground" => faces.color(value).is_some(),
            "family" => matches!(value, Value::Str(_)),
            "width" => font_width_percent(i, value).is_some(),
            "height" => {
                matches!(value, Value::Float(scale) if scale.is_finite() && **scale > 0.0)
                    || font_height_tenths(value).is_some()
            }
            "weight" => match value {
                Value::Int(_) => true,
                Value::Sym(symbol) => matches!(
                    i.symbol_name(*symbol).as_str(),
                    "thin"
                        | "ultra-light"
                        | "extra-light"
                        | "light"
                        | "semi-light"
                        | "book"
                        | "normal"
                        | "regular"
                        | "medium"
                        | "semi-bold"
                        | "demi-bold"
                        | "demibold"
                        | "bold"
                        | "extra-bold"
                        | "ultra-bold"
                        | "extrabold"
                        | "ultrabold"
                        | "black"
                        | "heavy"
                        | "ultra-heavy"
                ),
                _ => false,
            },
            "slant" => matches!(
                value,
                Value::Sym(symbol)
                    if matches!(i.symbol_name(*symbol).as_str(), "normal" | "roman" | "italic" | "oblique")
            ),
            "underline" => match value {
                Value::Nil | Value::Sym(_) => true,
                Value::Str(_) => faces.color(value).is_some(),
                Value::Cons(_) => {
                    let style = crate::lisp::eval::plist_get(value, faces.box_style);
                    let color = crate::lisp::eval::plist_get(value, faces.box_color);
                    (style.is_nil()
                        || matches!(
                            style,
                            Value::Sym(symbol)
                                if matches!(i.symbol_name(symbol).as_str(), "line" | "wave")
                        ))
                        && (color.is_nil() || faces.color(&color).is_some())
                }
                _ => false,
            },
            "overline" | "inverse-video" | "extend" => {
                matches!(value, Value::Nil | Value::Sym(_))
            }
            "strike-through" => match value {
                Value::Nil | Value::Sym(_) => true,
                Value::Str(_) => faces.color(value).is_some(),
                Value::Cons(_) => {
                    let color = crate::lisp::eval::plist_get(value, faces.box_color);
                    color.is_nil() || faces.color(&color).is_some()
                }
                _ => false,
            },
            "box" => {
                if let Value::Str(_) = value {
                    faces.color(value).is_some()
                } else if let Value::Cons(_) = value {
                    let color = crate::lisp::eval::plist_get(value, faces.box_color);
                    color.is_nil() || faces.color(&color).is_some()
                } else {
                    true
                }
            }
            "stipple" => faces.stipple_id(i, value).is_some(),
            "inherit" => matches!(value, Value::Nil | Value::Sym(_) | Value::Cons(_)),
            _ => false,
        };
        if !supported {
            return false;
        }
    }

    let mut base = faces.builtin(i, "default", 0);
    let default_style = base.finish();
    faces.default_style = Some(default_style);
    let overrides = faces.face_plist(i, attributes, 0);
    base.merge(&overrides);
    !any_specified || base.finish() != default_style
}

fn frame_scroll_bar_style(
    i: &Interp,
    face_ctx: &mut FaceCtx,
    frame: &crate::editor::Frame,
) -> CellStyle {
    let mut style = face_ctx.builtin(i, "scroll-bar", 0).finish();
    let parameter_color = |name: &str| {
        i.q_soft(name).and_then(|key| {
            face_ctx.color(&crate::lisp::eval::plist_get_v(
                &frame.params,
                &Value::Sym(key),
            ))
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
        _ => String::new(),
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

fn readable_mode_line_size(size: usize) -> String {
    if size < 1000 {
        return size.to_string();
    }
    let mut whole = size;
    let mut remainder;
    let mut exponent = 0;
    loop {
        remainder = whole % 1000;
        whole /= 1000;
        exponent += 1;
        if whole < 1000 {
            break;
        }
    }
    // Match GNU pint2hrstr: one decimal below ten, then whole units,
    // with half-up rounding and promotion at the end of a unit range.
    let mut tenths = None;
    if whole < 10 {
        let rounded = (remainder + 50) / 100;
        if rounded == 10 {
            whole += 1;
            tenths = (whole < 10).then_some(0);
        } else {
            tenths = Some(rounded);
        }
    } else if remainder >= 500 {
        whole += 1;
        if whole == 1000 {
            whole = 1;
            exponent += 1;
            tenths = Some(0);
        }
    }
    let mut output = match tenths {
        Some(tenths) => format!("{whole}.{tenths}"),
        None => whole.to_string(),
    };
    output.push(b" kMGTPEZYRQ"[exponent] as char);
    output
}

fn format_mode_line_string(
    i: &Interp,
    buffer_id: usize,
    value: &crate::lisp::value::StrRef,
    context: &ModeLineContext,
    fill_remaining: &mut bool,
) -> String {
    let characters: Vec<_> = value.borrow().to_unicode_lossy().chars().collect();
    let mut output = String::new();
    let collect = context.properties.borrow().is_some();
    let mut sources = Vec::new();
    let mut fill_source = None;
    let mut index = 0usize;
    while index < characters.len() {
        let source = index;
        if characters[index] != '%' {
            output.push(characters[index]);
            if collect {
                sources.push(source);
            }
            index += 1;
            continue;
        }
        index += 1;
        if index >= characters.len() {
            output.push('%');
            if collect {
                sources.push(source);
            }
            break;
        }
        if characters[index] == '%' {
            output.push('%');
            if collect {
                sources.push(source);
            }
            index += 1;
            continue;
        }
        if characters[index] == '-' && index + 1 == characters.len() {
            *fill_remaining = true;
            fill_source = Some(source);
            break;
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
            if collect {
                sources.push(source);
            }
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
            'i' => context.buffer_size.to_string(),
            'I' => readable_mode_line_size(context.buffer_size),
            'p' | 'P' | 'o' | 'q' => mode_line_position(context, specifier),
            '@' => {
                context.remote_used.set(true);
                if context.remote { "@" } else { "-" }.to_owned()
            }
            'n' => narrowing_indicator(context.narrowed).to_owned(),
            '*' | '+' | '&' => {
                buffer_state_marker(context.read_only, context.modified, specifier).to_string()
            }
            'e' => {
                if context.memory_full {
                    MEMORY_FULL_WARNING.to_owned()
                } else {
                    String::new()
                }
            }
            '[' | ']' | 's' => String::new(),
            other => other.to_string(),
        };
        if let Some(maximum) = precision {
            field = field.chars().take(maximum).collect();
        }
        if has_width {
            // GNU counts the numeric part separately from %I's suffix.
            let width =
                width.saturating_add(usize::from(specifier == 'I' && context.buffer_size >= 1000));
            field = fit_mode_line_field(&field, width, left_align);
        }
        if collect {
            sources.extend(std::iter::repeat_n(source, field.chars().count()));
        }
        output.push_str(&field);
    }
    if let Some(properties) = context.properties.borrow_mut().as_mut() {
        let properties_at = |position| {
            let plist = Value::list(crate::buffer::primitives::str_plist_at(
                i.str_props(value),
                position,
            ));
            let help = i.q_soft("help-echo").and_then(|property| {
                help_echo_text(crate::lisp::eval::plist_get(&plist, property))
            });
            let identification = i.q_soft("local-map").is_some_and(|property| {
                mode_line_identification_map(
                    i,
                    buffer_id,
                    &crate::lisp::eval::plist_get(&plist, property),
                )
            });
            let pointer = i.q_soft("pointer").and_then(|property| {
                mode_line_pointer_shape(i, &crate::lisp::eval::plist_get(&plist, property))
            });
            ModeLineCharacterProperties {
                help,
                identification,
                pointer,
            }
        };
        properties
            .characters
            .extend(sources.into_iter().map(properties_at));
        if let Some(source) = fill_source {
            properties.fill = Some(properties_at(source));
        }
    }
    output
}

fn mode_line_pointer_shape(i: &Interp, value: &Value) -> Option<GridPointerShape> {
    value.truthy().then(|| grid_pointer_shape(i, Some(value)))
}

fn mode_line_identification_map(i: &Interp, buffer_id: usize, map: &Value) -> bool {
    map.truthy()
        && mode_line_variable(i, buffer_id, "mode-line-buffer-identification-keymap")
            .is_some_and(|identification| crate::lisp::eq_values(map, &identification))
}

fn help_echo_text(value: Value) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.borrow().to_unicode_lossy()),
        _ => None,
    }
}

pub(super) fn mode_line_minor_values(i: &Interp, buffer_id: usize) -> Vec<Value> {
    let Some(alist) = mode_line_variable(i, buffer_id, "minor-mode-alist") else {
        return Vec::new();
    };
    let Ok(entries) = alist.list_to_vec() else {
        return Vec::new();
    };
    let mut output = Vec::new();
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
            output.extend(parts.into_iter().skip(1));
        }
    }
    output
}

fn format_mode_line_minor_modes(
    i: &Interp,
    buffer_id: usize,
    context: &ModeLineContext,
    depth: usize,
    fill_remaining: &mut bool,
) -> String {
    let mut output = String::new();
    for value in mode_line_minor_values(i, buffer_id) {
        if *fill_remaining {
            break;
        }
        output.push_str(&format_mode_line_value(
            i,
            buffer_id,
            &value,
            context,
            depth + 1,
            fill_remaining,
        ));
    }
    output
}

pub(super) fn format_mode_line_value(
    i: &Interp,
    buffer_id: usize,
    value: &Value,
    context: &ModeLineContext,
    depth: usize,
    fill_remaining: &mut bool,
) -> String {
    if depth > 32 || *fill_remaining {
        return String::new();
    }
    match value {
        Value::Nil => String::new(),
        Value::Str(string) => {
            format_mode_line_string(i, buffer_id, string, context, fill_remaining)
        }
        Value::Int(_) | Value::Float(_) => {
            let output = match value {
                Value::Int(number) => number.to_string(),
                Value::Float(number) => number.to_string(),
                _ => unreachable!(),
            };
            if let Some(properties) = context.properties.borrow_mut().as_mut() {
                properties.characters.extend(std::iter::repeat_n(
                    ModeLineCharacterProperties::default(),
                    output.chars().count(),
                ));
            }
            output
        }
        Value::Sym(symbol) => {
            if i.symbol_name(*symbol) == "mode-line-minor-modes" {
                return format_mode_line_minor_modes(
                    i,
                    buffer_id,
                    context,
                    depth + 1,
                    fill_remaining,
                );
            }
            let Some(value) = mode_line_variable(i, buffer_id, &i.symbol_name(*symbol)) else {
                return String::new();
            };
            if matches!(value, Value::Sym(id) if id == *symbol) {
                return String::new();
            }
            format_mode_line_value(i, buffer_id, &value, context, depth + 1, fill_remaining)
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
                        let start = context
                            .properties
                            .borrow()
                            .as_ref()
                            .map(|p| p.characters.len());
                        let output = elements.get(1).map_or_else(String::new, |body| {
                            format_mode_line_value(
                                i,
                                buffer_id,
                                body,
                                context,
                                depth + 1,
                                fill_remaining,
                            )
                        });
                        if let Some(start) = start
                            && let Some(properties) = context.properties.borrow_mut().as_mut()
                        {
                            let property = |name| {
                                let property = i.q_soft(name)?;
                                elements
                                    .get(2..)
                                    .unwrap_or_default()
                                    .chunks_exact(2)
                                    .find(
                                        |pair| matches!(pair[0], Value::Sym(id) if id == property),
                                    )
                                    .map(|pair| &pair[1])
                            };
                            let help =
                                property("help-echo").map(|value| help_echo_text(value.clone()));
                            let identification = property("local-map")
                                .map(|value| mode_line_identification_map(i, buffer_id, value));
                            let pointer =
                                property("pointer").map(|value| mode_line_pointer_shape(i, value));
                            for character in properties.characters[start..]
                                .iter_mut()
                                .chain(properties.fill.iter_mut())
                            {
                                if let Some(help) = &help {
                                    character.help = help.clone();
                                }
                                if let Some(identification) = identification {
                                    character.identification = identification;
                                }
                                if let Some(pointer) = pointer {
                                    character.pointer = pointer;
                                }
                            }
                        }
                        return output;
                    }
                    _ => {}
                }
                if elements.len() > 1 {
                    let condition = mode_line_variable(i, buffer_id, &i.symbol_name(*head))
                        .unwrap_or(Value::Nil)
                        .truthy();
                    let branch = elements.get(if condition { 1 } else { 2 });
                    return branch.map_or_else(String::new, |body| {
                        format_mode_line_value(
                            i,
                            buffer_id,
                            body,
                            context,
                            depth + 1,
                            fill_remaining,
                        )
                    });
                }
            }
            let start = if matches!(elements[0], Value::Int(_)) {
                1
            } else {
                0
            };
            let mut output = String::new();
            for element in &elements[start..] {
                if *fill_remaining {
                    break;
                }
                output.push_str(&format_mode_line_value(
                    i,
                    buffer_id,
                    element,
                    context,
                    depth + 1,
                    fill_remaining,
                ));
            }
            output
        }
        Value::Vec(vector) => {
            let mut output = String::new();
            for element in vector.borrow().iter() {
                if *fill_remaining {
                    break;
                }
                output.push_str(&format_mode_line_value(
                    i,
                    buffer_id,
                    element,
                    context,
                    depth + 1,
                    fill_remaining,
                ));
            }
            output
        }
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

/// GNU keyboard.c's separator names, including empty/single-dash compatibility.
pub(super) fn menu_separator_name_p(label: &str) -> bool {
    // GNU passes a C string to the native menu constructor.
    let label = label.split('\0').next().unwrap_or_default();
    label.bytes().all(|byte| byte == b'-')
        || matches!(
            label.strip_prefix("--"),
            Some(
                "space"
                    | "no-line"
                    | "single-line"
                    | "double-line"
                    | "single-dashed-line"
                    | "double-dashed-line"
                    | "shadow-etched-in"
                    | "shadow-etched-out"
                    | "shadow-etched-in-dash"
                    | "shadow-etched-out-dash"
                    | "shadow-double-etched-in"
                    | "shadow-double-etched-out"
                    | "shadow-double-etched-in-dash"
                    | "shadow-double-etched-out-dash"
            )
        )
}

pub(super) fn menu_label_and_definition(
    i: &Interp,
    key: &Value,
    definition: &Value,
) -> (String, Value, bool) {
    let fallback = menu_key_name(i, key);
    if let Value::Cons(cell) = definition {
        let head = cell.borrow().car.clone();
        if let Value::Str(label) = &head {
            let label = label.borrow().to_unicode_lossy();
            let separator = menu_separator_name_p(&label);
            return (label, cell.borrow().cdr.clone(), separator);
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
                let separator = menu_separator_name_p(&label);
                return (label, definition, separator);
            }
        }
    }
    if let Value::Str(label) = definition {
        let label = label.borrow().to_unicode_lossy();
        if menu_separator_name_p(&label) {
            return (label, Value::Nil, true);
        }
    }
    (fallback, definition.clone(), false)
}

pub(super) fn menu_keymap_from_definition(
    i: &Interp,
    definition: &Value,
    depth: usize,
) -> Option<Value> {
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

fn render_menu_bar(i: &Interp) -> Vec<GridMenu> {
    i.native_menus.clone()
}

fn render_tool_bar(
    i: &Interp,
    frame: &crate::editor::Frame,
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
        .buffer_local_toplevel_value(map_id, frame.selected.borrow().buffer)
        .or_else(|| i.default_toplevel_value(map_id))
        .unwrap_or(Value::Nil);
    let mut items = Vec::new();
    let mut row = 0usize;
    let mut column = 0usize;
    // Unprepared symbol maps retain the legacy read-only rendering path.
    let fallback;
    let bindings = if let Some(items) = i.native_menu_commands.tool_bar(frame.id) {
        items
    } else {
        fallback = crate::editor::keymap_own_bindings(i, &map)
            .into_iter()
            .filter_map(|(key, definition)| {
                let (mut label, command, separator) =
                    menu_label_and_definition(i, &key, &definition);
                let name = match command {
                    Value::Sym(symbol) if i.fbound_p(symbol) => Some(i.symbol_name(symbol)),
                    _ if separator => None,
                    _ => return None,
                };
                if let Ok(parts) = definition.list_to_vec() {
                    let properties = Value::list(parts.into_iter().skip(3).collect());
                    if let Some(property) = i.q_soft(":label")
                        && let Value::Str(value) =
                            crate::lisp::eval::plist_get(&properties, property)
                    {
                        label = value.borrow().to_unicode_lossy();
                    }
                }
                Some(GridMenuItem {
                    label,
                    help: None,
                    command: name,
                    command_id: None,
                    items: Vec::new(),
                    separator,
                    checked: false,
                    key_hint: None,
                })
            })
            .collect::<Vec<_>>();
        &fallback
    };
    for binding in bindings {
        let label = &binding.label;
        let separator = binding.separator;
        if separator {
            column = column.saturating_add(1);
            continue;
        }
        let label_width = label.chars().map(display_char_width).sum::<usize>().max(1);
        let button_width = label_width.saturating_add(2).min(width.max(1));
        if column.saturating_add(button_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        if row >= rows || width == 0 {
            break;
        }
        let text = fit_text_to_cells_with_tab_width(&format!(" {label} "), button_width, 8).0;
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
            place_text_row(screen_row, column, &text, &cell_styles, &character_styles);
        }
        items.push(GridToolBarItem {
            row: screen_row_index,
            byte_range: None,
            left: column,
            width: button_width,
            command: binding.command.clone().unwrap_or_default(),
            command_id: binding.command_id.clone(),
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
            byte_range: None,
            left: column,
            width: label_width.min(button_width),
            command: "tab-bar-select-tab".to_owned(),
            argument: Some(index.saturating_add(1) as i128),
        });
        if close_visible && close_width > 0 {
            items.push(GridTabBarItem {
                row: screen_row,
                byte_range: None,
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
                byte_range: None,
                left: column,
                width: button_width,
                command: "tab-bar-new-tab".to_owned(),
                argument: None,
            });
        }
    }
    items
}

fn render_popup_menu(
    i: &Interp,
    menu: &super::PopupMenu,
    pixel_position: (i128, i128),
    width: usize,
    height: usize,
    cell_width: usize,
    cell_height: usize,
    default_style: CellStyle,
    faces: &mut FaceCtx,
    screen: &mut [Vec<ScreenCell>],
) -> Vec<GridPopupMenuItem> {
    if menu.entries.is_empty() || width < 3 || height < 3 {
        return Vec::new();
    }
    let parents = std::iter::once(None)
        .chain(menu.opened_submenus().into_iter().map(Some))
        .collect::<Vec<_>>();
    let panels = parents
        .iter()
        .map(|&parent| {
            let rows = menu.level_entries(parent);
            let base = parent.map_or(0, |parent| menu.entries[parent].depth + 1);
            let label_width = rows
                .iter()
                .map(|&index| {
                    let entry = &menu.entries[index];
                    entry.label.chars().map(display_char_width).sum::<usize>()
                        + entry.depth.saturating_sub(base) * 2
                        + usize::from(entry.submenu) * 2
                })
                .max()
                .unwrap_or(1);
            let back_width = parent.map_or(0, |parent| {
                menu.entries[parent]
                    .label
                    .chars()
                    .map(display_char_width)
                    .sum::<usize>()
                    + 2
            });
            (
                parent,
                rows,
                label_width
                    .max(back_width)
                    .saturating_add(2)
                    .max(3)
                    .min(width),
            )
        })
        .collect::<Vec<_>>();
    // Keep as many ancestor columns as fit. On a narrow frame, a child panel
    // replaces them and provides a back action in its top border.
    let mut first = 0;
    let mut total_width = panels.iter().map(|panel| panel.2).sum::<usize>();
    while total_width > width && first + 1 < panels.len() {
        total_width -= panels[first].2;
        first += 1;
    }
    menu.focus_visible_levels(&parents[first..]);
    let mut left = usize::try_from(pixel_position.0.max(0))
        .unwrap_or(usize::MAX)
        .checked_div(cell_width.max(1))
        .unwrap_or(0)
        .min(width.saturating_sub(total_width));
    let origin_top = usize::try_from(pixel_position.1.max(0))
        .unwrap_or(usize::MAX)
        .checked_div(cell_height.max(1))
        .unwrap_or(0);
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
    let mut selected_style = style;
    selected_style.fg = style.bg.or(Some(CellColor(0, 0, 0)));
    selected_style.bg = style.fg.or(Some(CellColor(0xee, 0xee, 0xee)));
    let mut hit_items: Vec<GridPopupMenuItem> = Vec::new();
    for (level, (parent, rows, panel_width)) in panels.iter().enumerate().skip(first) {
        if rows.is_empty() {
            continue;
        }
        let panel_height = rows.len().saturating_add(2).min(height);
        let capacity = panel_height - 2;
        let offset = menu.viewport(*parent).0;
        menu.set_viewport(*parent, offset, capacity);
        if let Some(&Some(next)) = parents.get(level + 1) {
            menu.ensure_visible(*parent, next);
        } else if let Some(selected) = menu.highlighted().filter(|index| rows.contains(index)) {
            menu.ensure_visible(*parent, selected);
        }
        let offset = menu.viewport(*parent).0;
        let anchor = if level > first {
            let Some(parent_row) = hit_items.iter().rfind(|item| item.selection == *parent) else {
                break;
            };
            parent_row.row.saturating_sub(1)
        } else {
            origin_top
        };
        let top = anchor.min(height.saturating_sub(panel_height));
        for row_index in top..top + panel_height {
            if let Some(row) = screen.get_mut(row_index) {
                for cell in row.iter_mut().skip(left).take(*panel_width) {
                    *cell = ScreenCell {
                        chars: Vec::new(),
                        style,
                        occupied: true,
                        continuation: false,
                    };
                }
            }
        }
        let mut painted_rows = Vec::new();
        if level == first && first != 0 {
            painted_rows.push((top, parent.unwrap(), true));
        }
        painted_rows.extend(
            rows.iter()
                .copied()
                .skip(offset)
                .take(capacity)
                .enumerate()
                .map(|(line, index)| (top + line + 1, index, false)),
        );
        let base = parent.map_or(0, |parent| menu.entries[parent].depth + 1);
        for (row_index, index, back) in painted_rows {
            let item = &menu.entries[index];
            let text = if back {
                format!("‹ {}", item.label)
            } else if item.separator {
                "─".repeat(panel_width.saturating_sub(2))
            } else {
                format!(
                    "{}{}{}",
                    " ".repeat(item.depth.saturating_sub(base) * 2),
                    item.label,
                    if item.submenu { " ▸" } else { "" }
                )
            };
            let text = fit_text_to_cells_with_tab_width(&text, panel_width.saturating_sub(2), 8).0;
            let row_style = if !back && menu.highlighted() == Some(index) {
                selected_style
            } else {
                style
            };
            let cell_styles = vec![row_style; panel_width.saturating_sub(2)];
            let char_styles = text.chars().map(|_| row_style).collect::<Vec<_>>();
            if let Some(row) = screen.get_mut(row_index) {
                place_text_row(row, left + 1, &text, &cell_styles, &char_styles);
            }
            hit_items.push(GridPopupMenuItem {
                menu_id: menu.id,
                parent: *parent,
                selection: (back || item.submenu || item.value.is_some()).then_some(index),
                row: row_index,
                left: left + 1,
                width: panel_width.saturating_sub(2),
                byte_range: None,
                command: if back || item.separator || item.submenu {
                    None
                } else {
                    item.command.clone()
                },
            });
        }
        left += panel_width;
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
    object_hits: Vec<GridInputObjectHit>,
    compositions: Vec<RenderedComposition>,
    fringes: Vec<RenderedFringe>,
}

#[derive(Clone, PartialEq, Eq)]
struct RenderedFringe {
    left: bool,
    bitmap: usize,
    style: CellStyle,
}

#[derive(Clone, PartialEq, Eq)]
struct RenderedImage {
    column: usize,
    object_source: Option<MouseObjectSource>,
    source: GridImageSource,
    index: Option<usize>,
    conversion: Option<GridImageConversion>,
    transform_smoothing: Option<bool>,
    width: Option<usize>,
    height: Option<usize>,
    masked: bool,
    intrinsic_width: Option<usize>,
    intrinsic_height: Option<usize>,
    map: Vec<GridImageMapArea>,
    slice: Option<GridImageSlice>,
}

impl RenderedImage {
    fn visible_dimensions(&self) -> (Option<usize>, Option<usize>) {
        image_visible_dimensions(
            self.width,
            self.height,
            self.intrinsic_width,
            self.intrinsic_height,
            self.slice,
        )
    }

    fn resolved_slice(&self) -> Option<GridImageSlicePixels> {
        self.slice?.resolve(
            self.display_dimensions().0,
            self.display_dimensions().1,
        )
    }

    fn display_dimensions(&self) -> (Option<usize>, Option<usize>) {
        image_display_dimensions(
            self.width,
            self.height,
            self.intrinsic_width,
            self.intrinsic_height,
        )
    }

    fn cursor_dimensions(&self) -> (Option<f64>, Option<f64>) {
        let dimensions = match (
            self.width,
            self.height,
            self.intrinsic_width,
            self.intrinsic_height,
        ) {
            (Some(width), Some(height), _, _) => (Some(width as f64), Some(height as f64)),
            (Some(width), None, Some(intrinsic_width), Some(intrinsic_height)) => (
                Some(width as f64),
                scale_image_dimension(intrinsic_height, width, intrinsic_width),
            ),
            (None, Some(height), Some(intrinsic_width), Some(intrinsic_height)) => (
                scale_image_dimension(intrinsic_width, height, intrinsic_height),
                Some(height as f64),
            ),
            (width, height, intrinsic_width, intrinsic_height) => (
                width
                    .map(|width| width as f64)
                    .or_else(|| intrinsic_width.map(|width| width as f64)),
                height
                    .map(|height| height as f64)
                    .or_else(|| intrinsic_height.map(|height| height as f64)),
            ),
        };
        self.resolved_slice().map_or(dimensions, |slice| {
            (Some(slice.width as f64), Some(slice.height as f64))
        })
    }
}

fn scale_image_dimension(value: usize, scale: usize, basis: usize) -> Option<f64> {
    if basis == 0 {
        return None;
    }
    Some(value as f64 * scale as f64 / basis as f64)
}

fn image_display_dimensions(
    width: Option<usize>,
    height: Option<usize>,
    intrinsic_width: Option<usize>,
    intrinsic_height: Option<usize>,
) -> (Option<usize>, Option<usize>) {
    match (width, height, intrinsic_width, intrinsic_height) {
        (Some(width), Some(height), _, _) => (Some(width), Some(height)),
        (Some(width), None, Some(intrinsic_width), Some(intrinsic_height)) => (
            Some(width),
            scale_image_dimension(intrinsic_height, width, intrinsic_width)
                .map(|height| height.round() as usize),
        ),
        (None, Some(height), Some(intrinsic_width), Some(intrinsic_height)) => (
            scale_image_dimension(intrinsic_width, height, intrinsic_height)
                .map(|width| width.round() as usize),
            Some(height),
        ),
        (width, height, intrinsic_width, intrinsic_height) => (
            width.or(intrinsic_width),
            height.or(intrinsic_height),
        ),
    }
}

fn image_visible_dimensions(
    width: Option<usize>,
    height: Option<usize>,
    intrinsic_width: Option<usize>,
    intrinsic_height: Option<usize>,
    slice: Option<GridImageSlice>,
) -> (Option<usize>, Option<usize>) {
    let dimensions = image_display_dimensions(width, height, intrinsic_width, intrinsic_height);
    slice
        .and_then(|slice| slice.resolve(dimensions.0, dimensions.1))
        .map_or(dimensions, |slice| (Some(slice.width), Some(slice.height)))
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

#[derive(Clone)]
enum DisplayReplacement {
    Text(String),
    StyledText(DisplayText),
    Space(usize),
    Fringe(RenderedFringe),
    GlyphlessText(String, CellStyle),
    CompositionDefault(usize),
    Image {
        source: GridImageSource,
        object_source: Option<MouseObjectSource>,
        index: Option<usize>,
        conversion: Option<GridImageConversion>,
        transform_smoothing: Option<bool>,
        width: Option<usize>,
        height: Option<usize>,
        masked: bool,
        intrinsic_width: Option<usize>,
        intrinsic_height: Option<usize>,
        map: Vec<GridImageMapArea>,
        slice: Option<GridImageSlice>,
    },
    /// Another character in a run already replaced by its first character.
    Hidden,
}

#[derive(Clone)]
struct DisplayText {
    text: String,
    face_styles: Vec<StyleMix>,
    mouse_face_styles: Vec<StyleMix>,
    mouse_face_active: Vec<bool>,
    mouse_sources: Vec<Option<MouseObjectSource>>,
}

impl DisplayText {
    fn plain(text: String) -> DisplayText {
        let char_count = text.chars().count();
        DisplayText {
            text,
            face_styles: vec![StyleMix::default(); char_count],
            mouse_face_styles: vec![StyleMix::default(); char_count],
            mouse_face_active: vec![false; char_count],
            mouse_sources: vec![None; char_count],
        }
    }

    fn concat(parts: Vec<DisplayText>) -> DisplayText {
        let char_count = parts.iter().map(|part| part.text.chars().count()).sum();
        let mut combined = DisplayText {
            text: String::new(),
            face_styles: Vec::with_capacity(char_count),
            mouse_face_styles: Vec::with_capacity(char_count),
            mouse_face_active: Vec::with_capacity(char_count),
            mouse_sources: Vec::with_capacity(char_count),
        };
        for part in parts {
            combined.text.push_str(&part.text);
            combined.face_styles.extend(part.face_styles);
            combined.mouse_face_styles.extend(part.mouse_face_styles);
            combined.mouse_face_active.extend(part.mouse_face_active);
            combined.mouse_sources.extend(part.mouse_sources);
        }
        combined
    }

    fn columns_after(&self, mut column: usize, tab_width: usize) -> usize {
        for character in self.text.chars() {
            let display = match character {
                '\n' | '\r' => ' ',
                other => other,
            };
            let columns = if display == '\t' {
                tab_width.max(1) - column % tab_width.max(1)
            } else {
                display_char_width(display)
            };
            column = column.saturating_add(columns);
        }
        column
    }

    fn split_to_columns(
        &self,
        start_column: usize,
        width: usize,
        tab_width: usize,
    ) -> (bool, Vec<(DisplayText, usize)>) {
        if width == 0 || self.text.is_empty() {
            return (false, vec![(self.clone(), self.columns_after(start_column, tab_width))]);
        }
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut face_styles = Vec::new();
        let mut mouse_face_styles = Vec::new();
        let mut mouse_face_active = Vec::new();
        let mut mouse_sources = Vec::new();
        let mut column = start_column;
        let mut first_row = true;
        let mut leading_break = false;
        let tab_width = tab_width.max(1);
        for (index, character) in self.text.chars().enumerate() {
            let display = match character {
                '\n' | '\r' => ' ',
                other => other,
            };
            let mut columns = if display == '\t' {
                tab_width - column % tab_width
            } else {
                display_char_width(display)
            };
            if column.saturating_add(columns) > width && column > 0 {
                if text.is_empty() {
                    if first_row && start_column > 0 {
                        leading_break = true;
                        column = 0;
                        columns = if display == '\t' { tab_width } else { columns };
                    }
                } else {
                    parts.push((
                        DisplayText {
                            text: std::mem::take(&mut text),
                            face_styles: std::mem::take(&mut face_styles),
                            mouse_face_styles: std::mem::take(&mut mouse_face_styles),
                            mouse_face_active: std::mem::take(&mut mouse_face_active),
                            mouse_sources: std::mem::take(&mut mouse_sources),
                        },
                        column,
                    ));
                    column = 0;
                    columns = if display == '\t' { tab_width } else { columns };
                }
            }
            text.push(character);
            face_styles.push(self.face_styles[index]);
            mouse_face_styles.push(self.mouse_face_styles[index]);
            mouse_face_active.push(self.mouse_face_active[index]);
            mouse_sources.push(self.mouse_sources[index]);
            column = column.saturating_add(columns);
            first_row = false;
        }
        parts.push((
            DisplayText {
                text,
                face_styles,
                mouse_face_styles,
                mouse_face_active,
                mouse_sources,
            },
            column,
        ));
        (leading_break, parts)
    }

    fn contains_newline(&self) -> bool {
        self.text.contains('\n')
    }

    fn split_newlines(&self) -> Vec<DisplayText> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut face_styles = Vec::new();
        let mut mouse_face_styles = Vec::new();
        let mut mouse_face_active = Vec::new();
        let mut mouse_sources = Vec::new();
        for (index, character) in self.text.chars().enumerate() {
            if character == '\n' {
                parts.push(DisplayText {
                    text: std::mem::take(&mut text),
                    face_styles: std::mem::take(&mut face_styles),
                    mouse_face_styles: std::mem::take(&mut mouse_face_styles),
                    mouse_face_active: std::mem::take(&mut mouse_face_active),
                    mouse_sources: std::mem::take(&mut mouse_sources),
                });
            } else {
                text.push(character);
                face_styles.push(self.face_styles[index]);
                mouse_face_styles.push(self.mouse_face_styles[index]);
                mouse_face_active.push(self.mouse_face_active[index]);
                mouse_sources.push(self.mouse_sources[index]);
            }
        }
        parts.push(DisplayText {
            text,
            face_styles,
            mouse_face_styles,
            mouse_face_active,
            mouse_sources,
        });
        parts
    }
}

impl DisplayReplacement {
    fn is_text(&self) -> bool {
        matches!(self, DisplayReplacement::Text(_) | DisplayReplacement::StyledText(_))
    }

    fn contains_newline(&self) -> bool {
        match self {
            DisplayReplacement::Text(text) => text.contains('\n'),
            DisplayReplacement::StyledText(text) => text.contains_newline(),
            _ => false,
        }
    }

    fn split_text_newlines(&self) -> Option<Vec<DisplayReplacement>> {
        match self {
            DisplayReplacement::Text(text) => Some(
                text.split('\n')
                    .map(|part| DisplayReplacement::Text(part.to_owned()))
                    .collect(),
            ),
            DisplayReplacement::StyledText(text) => Some(
                text.split_newlines()
                    .into_iter()
                    .map(DisplayReplacement::StyledText)
                    .collect(),
            ),
            _ => None,
        }
    }

    fn columns_after(&self, column: usize, tab_width: usize, cell_width: usize) -> usize {
        match self {
            DisplayReplacement::Text(text) | DisplayReplacement::GlyphlessText(text, _) => {
                DisplayText::plain(text.clone()).columns_after(column, tab_width)
            }
            DisplayReplacement::StyledText(text) => text.columns_after(column, tab_width),
            DisplayReplacement::CompositionDefault(source_width) => {
                column.saturating_add(*source_width)
            }
            DisplayReplacement::Space(columns) => column.saturating_add(*columns),
            DisplayReplacement::Image {
                width,
                height,
                intrinsic_width,
                intrinsic_height,
                slice,
                ..
            } => column.saturating_add(
                image_visible_dimensions(
                    *width,
                    *height,
                    *intrinsic_width,
                    *intrinsic_height,
                    *slice,
                )
                    .0
                    .map(|pixels| pixels.div_ceil(cell_width.max(1)))
                    .unwrap_or(1)
                    .max(1),
            ),
            DisplayReplacement::Fringe(_) | DisplayReplacement::Hidden => column,
        }
    }

    fn split_to_columns(
        &self,
        start_column: usize,
        width: usize,
        tab_width: usize,
    ) -> Option<(bool, Vec<(DisplayReplacement, usize)>)> {
        match self {
            DisplayReplacement::Text(text) => {
                let (leading_break, parts) = DisplayText::plain(text.clone()).split_to_columns(
                    start_column,
                    width,
                    tab_width,
                );
                Some((
                    leading_break,
                    parts
                        .into_iter()
                        .map(|(part, column)| (DisplayReplacement::Text(part.text), column))
                        .collect(),
                ))
            }
            DisplayReplacement::StyledText(text) => {
                let (leading_break, parts) =
                    text.split_to_columns(start_column, width, tab_width);
                Some((
                    leading_break,
                    parts
                        .into_iter()
                        .map(|(part, column)| (DisplayReplacement::StyledText(part), column))
                        .collect(),
                ))
            }
            _ => None,
        }
    }
}

/// A display row can reuse a source character while continuing the string
/// which replaces it. Overrides contain only the fragments for this row.
struct DisplayRowPlan {
    range: Range<usize>,
    replacements: HashMap<usize, DisplayReplacement>,
    boundaries: HashMap<usize, Vec<DisplayText>>,
    include_end_boundary: bool,
    hard_break: bool,
    repeated_start: bool,
    continued: bool,
}

impl DisplayRowPlan {
    fn new(range: Range<usize>, line_len: usize) -> Self {
        Self {
            include_end_boundary: range.end == line_len,
            range,
            replacements: HashMap::new(),
            boundaries: HashMap::new(),
            hard_break: false,
            repeated_start: false,
            continued: false,
        }
    }
}

fn display_row_plans(
    i: &Interp,
    faces: Option<&WindowFaces>,
    line: &EmacsText,
    buffer_id: usize,
    line_pos: usize,
    range: Range<usize>,
    line_len: usize,
    max_rows: usize,
    width: usize,
    first_prefix_width: usize,
    continuation_prefix_width: usize,
    tab_width: usize,
    cell_width: usize,
    ctl_arrow: bool,
    wrap_lines: bool,
) -> Vec<DisplayRowPlan> {
    if max_rows == 0 {
        return Vec::new();
    }
    let Some(faces) = faces else {
        return vec![DisplayRowPlan::new(range, line_len)];
    };
    let first = line_pos + range.start;
    let end = line_pos + range.end;
    let has_newline = faces.display_props.iter().any(|(start, stop, replacement, _)| {
        *start < end && *stop > first
            && replacement
                .as_ref()
                .is_some_and(DisplayReplacement::contains_newline)
    }) || faces.overlay_strings.iter().any(|(position, strings)| {
        *position >= first && (*position < end || (*position == end && range.end == line_len))
            && strings.iter().any(DisplayText::contains_newline)
    });
    let has_display_text = faces.display_props.iter().any(|(start, stop, replacement, _)| {
        *start < end
            && *stop > first
            && replacement.as_ref().is_some_and(DisplayReplacement::is_text)
    });
    if !has_newline && !has_display_text {
        return vec![DisplayRowPlan::new(range, line_len)];
    }
    let source_end = range.end;
    let mut current = DisplayRowPlan::new(range.clone(), line_len);
    let mut rows = Vec::new();
    let mut column = if range.start == 0 {
        first_prefix_width
    } else {
        continuation_prefix_width
    };
    let characters = line.chars().collect::<Vec<_>>();

    macro_rules! finish_before_source {
        ($index:expr) => {{
            current.range.end = $index;
            current.include_end_boundary = false;
            current.hard_break = true;
            rows.push(current);
            if rows.len() == max_rows {
                return rows;
            }
            current = DisplayRowPlan::new($index..source_end, line_len);
            current.continued = true;
            column = continuation_prefix_width;
        }};
    }

    macro_rules! finish_after_replacement {
        ($index:expr) => {{
            current.range.end = $index + 1;
            current.include_end_boundary = false;
            current.hard_break = true;
            rows.push(current);
            if rows.len() == max_rows {
                return rows;
            }
            current = DisplayRowPlan::new($index..source_end, line_len);
            current.boundaries.insert($index, Vec::new());
            current.repeated_start = true;
            current.continued = true;
            column = continuation_prefix_width;
        }};
    }

    for index in range.start..=source_end {
        let at_end = index == source_end;
        if at_end && source_end != line_len {
            break;
        }
        let position = line_pos + index;
        let strings = overlay_strings_at(faces, position);
        if strings.iter().any(DisplayText::contains_newline) {
            let text = DisplayText::concat(strings);
            let parts = text.split_newlines();
            let part_count = parts.len();
            for (part_index, part) in parts.into_iter().enumerate() {
                column = part.columns_after(column, tab_width);
                current.boundaries.insert(index, vec![part]);
                if part_index + 1 < part_count {
                    current.range.end = index;
                    current.include_end_boundary = true;
                    current.hard_break = true;
                    rows.push(current);
                    if rows.len() == max_rows {
                        return rows;
                    }
                    current = DisplayRowPlan::new(index..source_end, line_len);
                    current.continued = true;
                    column = continuation_prefix_width;
                }
            }
        } else {
            for string in strings {
                column = string.columns_after(column, tab_width);
            }
        }
        if at_end {
            break;
        }
        let visibility = visibility_at_pos(i, faces, position);
        if visibility != Visibility::Visible {
            if visibility == Visibility::Ellipsis {
                column = column.saturating_add(1);
            }
            continue;
        }
        let character = characters.get(index).copied();
        let replacement = display_replacement_at(faces, position).or_else(|| {
            character.and_then(|character| {
                display_table_replacement(i, buffer_id, faces, character)
            })
        });
        if let Some(replacement) = replacement {
            let parts = replacement
                .split_text_newlines()
                .unwrap_or_else(|| vec![replacement]);
            let part_count = parts.len();
            for (part_index, part) in parts.into_iter().enumerate() {
                let break_after_part = part_index + 1 < part_count;
                if wrap_lines
                    && let Some((leading_break, fragments)) =
                        part.split_to_columns(column, width, tab_width)
                {
                    if leading_break {
                        finish_before_source!(index);
                    }
                    let fragment_count = fragments.len();
                    for (fragment_index, (fragment, end_column)) in
                        fragments.into_iter().enumerate()
                    {
                        current.replacements.insert(index, fragment);
                        column = end_column;
                        if fragment_index + 1 < fragment_count || break_after_part {
                            finish_after_replacement!(index);
                        }
                    }
                } else {
                    let mut end_column = part.columns_after(column, tab_width, cell_width);
                    if wrap_lines && column > 0 && end_column > width {
                        finish_before_source!(index);
                        end_column = part.columns_after(column, tab_width, cell_width);
                    }
                    current.replacements.insert(index, part);
                    column = end_column;
                    if break_after_part {
                        finish_after_replacement!(index);
                    }
                }
            }
        } else if let Some(character) = character {
            let mut columns = if character.unicode() == Some('\t') {
                tab_width.max(1) - column % tab_width.max(1)
            } else if character.byte8().is_some() {
                4
            } else if ctl_arrow && (character.code() < 0x20 || character.code() == 0x7f) {
                2
            } else {
                display_char_width(character.unicode().unwrap_or('\u{fffd}'))
            };
            if wrap_lines && column > 0 && column.saturating_add(columns) > width {
                finish_before_source!(index);
                columns = if character.unicode() == Some('\t') {
                    tab_width.max(1)
                } else if character.byte8().is_some() {
                    4
                } else if ctl_arrow && (character.code() < 0x20 || character.code() == 0x7f) {
                    2
                } else {
                    display_char_width(character.unicode().unwrap_or('\u{fffd}'))
                };
            }
            column = column.saturating_add(columns);
        }
    }
    rows.push(current);
    rows
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
    help_echoes: &mut Vec<GridHelpEcho>,
    pointer_regions: &mut Vec<GridPointerRegion>,
) -> Option<(usize, usize)> {
    let screen_row = screen.get_mut(row)?;
    *context.properties.borrow_mut() = Some(ModeLineProperties::default());
    let mut fill_remaining = false;
    let mut text = format_mode_line_value(i, buffer_id, format, context, 0, &mut fill_remaining);
    let mut properties = context.properties.borrow_mut().take().unwrap_or_default();
    if fill_remaining {
        let occupied = fit_text_to_cells_with_tab_width(&text, width, tab_width).1;
        let count = width.saturating_sub(occupied);
        text.push_str(&"-".repeat(count));
        properties.characters.extend(std::iter::repeat_n(
            properties.fill.unwrap_or_default(),
            count,
        ));
    }
    // Use the same column conversion and clipping as the visible row. In
    // particular, tabs cover several cells and a clipped wide glyph covers none.
    let mut column = 0usize;
    let mut buffer_name_range = None;
    for (character, properties) in text.chars().zip(properties.characters) {
        let character = match character {
            '\n' | '\r' => ' ',
            other => other,
        };
        let cells = if character == '\t' {
            let tab_width = tab_width.max(1);
            tab_width - column % tab_width
        } else {
            display_char_width(character)
        };
        if column.saturating_add(cells) > width {
            break;
        }
        push_pointer_region(
            pointer_regions,
            GridPointerRegion {
                row,
                left: left.saturating_add(column),
                width: cells,
                shape: properties.pointer.unwrap_or(GridPointerShape::Arrow),
            },
        );
        if cells > 0 && properties.identification {
            let start = left.saturating_add(column);
            let end = start.saturating_add(cells);
            match &mut buffer_name_range {
                None => buffer_name_range = Some((start, end)),
                Some((_, right)) if *right == start => *right = end,
                _ => {}
            }
        }
        if cells > 0
            && let Some(text) = properties.help
        {
            push_help_echo(
                help_echoes,
                GridHelpEcho {
                    row,
                    left: left.saturating_add(column),
                    width: cells,
                    text,
                },
            );
        }
        column += cells;
    }
    push_pointer_region(
        pointer_regions,
        GridPointerRegion {
            row,
            left: left.saturating_add(column),
            width: width.saturating_sub(column),
            shape: GridPointerShape::Arrow,
        },
    );
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
    place_text_row(screen_row, left, &row_text, &cell_styles, &character_styles);
    buffer_name_range
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

#[allow(clippy::too_many_arguments)]
fn append_rendered_styled_text(
    text: &DisplayText,
    base_style: CellStyle,
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
    object_hits: &mut Vec<GridInputObjectHit>,
) -> bool {
    for (index, raw_display) in text.text.chars().enumerate() {
        let mut style = text
            .face_styles
            .get(index)
            .copied()
            .unwrap_or_default()
            .overlay_on(base_style);
        if text
            .mouse_face_active
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            style = text
                .mouse_face_styles
                .get(index)
                .copied()
                .unwrap_or_default()
                .overlay_on(style);
        }
        let display = match raw_display {
            '\n' | '\r' => ' ',
            other => other,
        };
        let column_before = *column;
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
        if let Some(source) = text.mouse_sources.get(index).copied().flatten() {
            let start = column_before.max(hscroll);
            let end = (*column).min(end_column);
            let (left, right) = if start < end {
                (start - hscroll, end - hscroll)
            } else if column_before == *column && column_before > hscroll {
                // A combining character occupies the preceding base cell.
                (column_before - hscroll - 1, column_before - hscroll)
            } else {
                continue;
            };
            if left < width && right > left {
                object_hits.push(GridInputObjectHit {
                    left,
                    right: right.min(width),
                    object: MouseObject::String {
                        source,
                        position: index,
                    },
                });
            }
        }
    }
    true
}

fn render_text_row<F, V, D, B>(
    text: &EmacsText,
    hscroll: usize,
    width: usize,
    cell_width: usize,
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
    B: FnMut(usize) -> Vec<DisplayText>,
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
    let mut object_hits = Vec::new();
    let mut compositions = Vec::new();
    let mut fringes = Vec::new();

    for (index, ch) in chars.iter().enumerate() {
        let mut keep_rendering = true;
        for string in boundary_strings_at(index) {
            if !append_rendered_styled_text(
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
                &mut object_hits,
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
            Some(DisplayReplacement::Fringe(fringe)) => {
                if graphical {
                    fringes.push(fringe);
                }
                true
            }
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
            Some(DisplayReplacement::StyledText(replacement)) => {
                append_rendered_styled_text(
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
                    &mut object_hits,
                )
            }
            Some(DisplayReplacement::Space(columns)) => {
                let mut keep_rendering = true;
                for _ in 0..columns {
                    if !append_rendered_character(
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
                    ) {
                        keep_rendering = false;
                        break;
                    }
                }
                keep_rendering
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
                object_source,
                index,
                conversion,
                transform_smoothing,
                width: image_width,
                height: image_height,
                masked,
                intrinsic_width,
                intrinsic_height,
                map,
                slice,
            }) => {
                let image_column = column;
                // Reserve full grid cells for the image before placing later text.
                let image_columns = image_visible_dimensions(
                    image_width,
                    image_height,
                    intrinsic_width,
                    intrinsic_height,
                    slice,
                )
                    .0
                    .map(|pixels| pixels.div_ceil(cell_width.max(1)))
                    .unwrap_or(1)
                    .max(1);
                let visible = image_column >= hscroll && image_column < end_column;
                if visible {
                    images.push(RenderedImage {
                        column: image_column - hscroll,
                        object_source,
                        source,
                        index,
                        conversion,
                        transform_smoothing,
                        width: image_width,
                        height: image_height,
                        masked,
                        intrinsic_width,
                        intrinsic_height,
                        map,
                        slice,
                    });
                }
                let mut keep_rendering = true;
                for _ in 0..image_columns {
                    if !append_rendered_character(
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
                    ) {
                        keep_rendering = false;
                        break;
                    }
                }
                keep_rendering
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
            if !append_rendered_styled_text(
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
                &mut object_hits,
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
        cell_attrs[cell].box_style = None;
        let mut padding_style = default_style;
        padding_style.box_style = None;
        char_attrs.push(padding_style);
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
        object_hits,
        compositions,
        fringes,
    }
}

fn render_display_prefix(
    replacement: &DisplayReplacement,
    width: usize,
    cell_width: usize,
    tab_width: usize,
    default_style: CellStyle,
    graphical: bool,
) -> RenderedRow {
    render_text_row(
        &EmacsText::from(" "),
        0,
        width,
        cell_width,
        0,
        tab_width,
        default_style,
        |_| default_style,
        |_| Visibility::Visible,
        |_| Some(replacement.clone()),
        |_| Vec::new(),
        false,
        graphical,
    )
}

fn prepend_rendered_prefix_row(
    mut prefix: RenderedRow,
    prefix_width: usize,
    mut row: RenderedRow,
) -> RenderedRow {
    if prefix_width == 0 {
        return row;
    }
    let prefix_bytes = prefix.text.len();
    let prefix_char_count = prefix.char_attrs.len();
    let mut text = String::with_capacity(prefix_bytes.saturating_add(row.text.len()));
    text.push_str(&prefix.text);
    text.push_str(&row.text);
    row.text = text;

    prefix.cell_attrs.truncate(prefix_width);
    prefix.cell_attrs.extend(row.cell_attrs);
    row.cell_attrs = prefix.cell_attrs;
    prefix.char_attrs.extend(row.char_attrs);
    row.char_attrs = prefix.char_attrs;
    row.source_offsets
        .iter_mut()
        .for_each(|offset| *offset = offset.saturating_add(prefix_bytes));

    prefix.cell_offsets.truncate(prefix_width);
    prefix.cell_offsets.extend(
        row.cell_offsets
            .iter()
            .map(|offset| offset.saturating_add(prefix_bytes)),
    );
    row.cell_offsets = prefix.cell_offsets;
    row.visible_columns = row.visible_columns.saturating_add(prefix_width);
    row.padding_char_start = prefix_char_count.saturating_add(row.padding_char_start);

    for image in &mut row.images {
        image.column = image.column.saturating_add(prefix_width);
    }
    prefix.images.append(&mut row.images);
    row.images = prefix.images;
    for composition in &mut row.compositions {
        composition.column = composition.column.saturating_add(prefix_width);
    }
    prefix.compositions.append(&mut row.compositions);
    row.compositions = prefix.compositions;
    prefix.fringes.append(&mut row.fringes);
    row.fringes = prefix.fringes;
    row
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

pub fn fit_text_to_cells_with_tab_width(
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
    stipple_files: HashMap<EmacsText, Option<u16>>,
    bitmap_directory: EmacsText,
    /// Resolved face name -> merged StyleMix.
    cache: HashMap<crate::lisp::value::SymId, StyleMix>,
    /// Resolved `default' face values used by :reset in other face specs.
    default_style: Option<CellStyle>,
    /// Foreground from this frame's `foreground-color' parameter. It is a
    /// fallback for the default face, so an explicit default-face foreground
    /// still takes precedence.
    frame_default_foreground: Option<CellColor>,
    /// Background from this frame's `background-color' parameter. It is a
    /// fallback for the default face, so an explicit default-face background
    /// still takes precedence.
    frame_default_background: Option<CellColor>,
}

impl FaceCtx {
    fn new(i: &Interp, frame: Option<&crate::editor::FrameRef>) -> FaceCtx {
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
        let bitmap_directory = frame
            .map(|frame| frame.borrow().selected.borrow().buffer)
            .and_then(|buffer| {
                let symbol = i.q_soft("default-directory")?;
                i.buffer_local_toplevel_value(symbol, buffer)
                    .or_else(|| i.default_toplevel_value(symbol))
            })
            .and_then(|value| match value {
                Value::Str(directory) => Some(directory.borrow().clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::editor::default_directory(i));
        let mut ctx = FaceCtx {
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
            stipple_files: HashMap::new(),
            bitmap_directory,
            cache: HashMap::new(),
            frame_default_foreground: None,
            frame_default_background: None,
            default_style: None,
        };
        if let Some(frame) = frame {
            ctx.frame_default_foreground = i
                .intern_soft("foreground-color")
                .and_then(|parameter| {
                    let value = crate::lisp::eval::plist_get_v(
                        &frame.borrow().params,
                        &Value::Sym(parameter),
                    );
                    ctx.color(&value)
                });
            ctx.frame_default_background = i
                .intern_soft("background-color")
                .and_then(|parameter| {
                    let value = crate::lisp::eval::plist_get_v(
                        &frame.borrow().params,
                        &Value::Sym(parameter),
                    );
                    ctx.color(&value)
                });
        }
        ctx
    }

    /// Parse standard RGB specifications directly, then fall back to the
    /// canonicalized color-name alist.
    fn color(&self, v: &Value) -> Option<CellColor> {
        if let Some((red, green, blue)) = crate::lisp::builtins::misc::parse_color_16(v) {
            return Some(CellColor(
                (red / 256) as u8,
                (green / 256) as u8,
                (blue / 256) as u8,
            ));
        }
        let Value::Str(s) = v else {
            return None;
        };
        let t = s.borrow();
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

    fn stipple_id(&mut self, i: &Interp, value: &Value) -> Option<Option<u16>> {
        if value.is_nil() {
            return Some(None);
        }
        if let Value::Str(file) = value {
            let file = file.borrow().clone();
            if let Some(id) = self.stipple_files.get(&file) {
                return id.map(Some);
            }
            let id = read_stipple_file(i, &file, &self.bitmap_directory)
                .and_then(|stipple| self.intern_stipple(stipple));
            self.stipple_files.insert(file, id);
            return id.map(Some);
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
        self.intern_stipple(stipple).map(Some)
    }

    fn intern_stipple(&mut self, stipple: GridStipple) -> Option<u16> {
        if let Some(id) = self.stipple_ids.get(&stipple) {
            return Some(*id);
        }
        let id = u16::try_from(self.stipples.len()).ok()?;
        self.stipples.push(stipple.clone());
        self.stipple_ids.insert(stipple, id);
        Some(id)
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
                    style.vertical_width = width.saturating_abs();
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
        let mut mix = match i.q_soft(name) {
            Some(face) => self.named(i, &face, depth),
            None => builtin_face_defaults(i, name),
        };
        if mix.font_height_scale.is_some() {
            let default = self.default_style.and_then(|style| style.font_height).unwrap_or(140);
            mix.font_height = Some(mix.resolved_font_height(default));
            mix.font_height_scale = None;
        }
        mix
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
        if i.symbol_name(*name) == "default" {
            // Cache the realized default size, so inheriting it cannot apply
            // its own relative factor for a second time.
            if mix.font_height_scale.is_some() {
                mix.font_height = Some(mix.resolved_font_height(140));
                mix.font_height_scale = None;
            }
            if mix.fg.is_none()
                && let Some(color) = self.frame_default_foreground
            {
                mix.fg = Some(Some(color));
            }
            if mix.bg.is_none()
                && let Some(color) = self.frame_default_background
            {
                mix.bg = Some(Some(color));
            }
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
        let default_style = self.default_style.unwrap_or_else(|| CellStyle {
            fg: self.frame_default_foreground,
            bg: self.frame_default_background,
            ..CellStyle::default()
        });
        let Ok(items) = plist.list_to_vec() else {
            return mix;
        };
        for kv in items.chunks(2) {
            let (Some(k), Some(v)) = (i.sym_id(&kv[0]), kv.get(1)) else {
                continue;
            };
            let is_unspecified = matches!(v, Value::Sym(symbol) if i.symbol_name(*symbol) == "unspecified");
            let is_reset = matches!(v, Value::Sym(symbol) if i.symbol_name(*symbol) == "reset");
            match i.symbol_name(k).as_str() {
                ":foreground" => {
                    if is_reset {
                        mix.fg = Some(default_style.fg);
                    } else if !is_unspecified
                        && let Some(c) = self.color(v)
                    {
                        mix.fg = Some(Some(c));
                    }
                }
                ":distant-foreground" => {
                    if is_reset {
                        mix.distant_foreground = Some(default_style.distant_foreground);
                    } else if !is_unspecified {
                        mix.distant_foreground = Some(self.color(v));
                    }
                }
                ":background" => {
                    if is_reset {
                        mix.bg = Some(default_style.bg);
                    } else if !is_unspecified
                        && let Some(c) = self.color(v)
                    {
                        mix.bg = Some(Some(c));
                    }
                }
                ":family" => {
                    if is_reset {
                        mix.font_family = Some(default_style.font_family);
                    } else if !is_unspecified
                        && let Some(family) = self.font_family_id(v)
                    {
                        mix.font_family = Some(Some(family));
                    }
                }
                ":font" => {
                    if is_reset {
                        mix.font_family = Some(default_style.font_family);
                        mix.font_weight = Some(default_style.font_weight);
                        mix.font_width = Some(default_style.font_width);
                        mix.font_height = Some(default_style.font_height);
                        mix.font_height_scale = None;
                        mix.bold = Some(default_style.bold);
                        mix.italic = Some(default_style.italic);
                    } else if !is_unspecified && let Value::Str(_) = v {
                        if let Some(family) = self.font_family_id(v) {
                            mix.font_family = Some(Some(family));
                        }
                    }
                }
                ":weight" => {
                    if is_reset {
                        mix.font_weight = Some(default_style.font_weight);
                        mix.bold = Some(default_style.bold);
                    } else if !is_unspecified {
                        let weight = font_weight_value(i, v);
                        mix.font_weight = Some(Some(weight));
                        mix.bold = Some(weight >= 600);
                    }
                }
                ":width" => {
                    if is_reset {
                        mix.font_width = Some(default_style.font_width);
                    } else if !is_unspecified
                        && let Some(width) = font_width_percent(i, v)
                    {
                        mix.font_width = Some(Some(width));
                    }
                }
                ":height" => {
                    if is_reset {
                        mix.font_height = Some(default_style.font_height);
                        mix.font_height_scale = None;
                    } else if !is_unspecified {
                        if let Value::Float(scale) = v
                            && scale.is_finite() && **scale > 0.0
                        {
                            mix.font_height = None;
                            mix.font_height_scale = Some(**scale);
                        } else if let Some(height) = font_height_tenths(v) {
                            mix.font_height = Some(Some(height));
                            mix.font_height_scale = None;
                        }
                    }
                }
                ":stipple" => {
                    if is_reset {
                        mix.stipple = Some(default_style.stipple);
                    } else if !is_unspecified {
                        mix.stipple = self.stipple_id(i, v);
                    }
                }
                ":box" => {
                    if is_reset {
                        mix.box_style = Some(default_style.box_style);
                    } else if !is_unspecified {
                        mix.box_style = Some(self.parse_box_style(i, v));
                    }
                }
                ":slant" => {
                    if is_reset {
                        mix.italic = Some(default_style.italic);
                    } else if !is_unspecified {
                        mix.italic = Some(slant_italic(i, v));
                    }
                }
                ":underline" => {
                    if is_reset {
                        mix.underline = Some(default_style.underline);
                        mix.underline_color = Some(default_style.underline_color);
                        mix.underline_wave = Some(default_style.underline_wave);
                    } else if !is_unspecified {
                        mix.underline = Some(v.truthy());
                        mix.underline_color = Some(self.decoration_color(v));
                        mix.underline_wave = Some(self.underline_wave(i, v));
                    }
                }
                ":overline" => {
                    if is_reset {
                        mix.overline = Some(default_style.overline);
                        mix.overline_color = Some(default_style.overline_color);
                    } else if !is_unspecified {
                        mix.overline = Some(v.truthy());
                        mix.overline_color = Some(self.decoration_color(v));
                    }
                }
                ":strike-through" => {
                    if is_reset {
                        mix.strike = Some(default_style.strike);
                        mix.strike_color = Some(default_style.strike_color);
                    } else if !is_unspecified {
                        mix.strike = Some(v.truthy());
                        mix.strike_color = Some(self.decoration_color(v));
                    }
                }
                ":inverse-video" => {
                    if is_reset {
                        mix.inverse = Some(default_style.inverse);
                    } else if !is_unspecified {
                        mix.inverse = Some(v.truthy());
                    }
                }
                ":extend" => {
                    if is_reset {
                        mix.extend = Some(default_style.extend);
                    } else if !is_unspecified {
                        mix.extend = Some(v.truthy());
                    }
                }
                _ => {}
            }
        }
        mix
    }

    /// A `face' property value: a symbol/string names a face, a list
    /// whose car is a keyword is an anonymous face plist, any other
    /// list merges elements with the first element having highest priority, and
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
                                "foreground-color" => mix.fg = Some(Some(c)),
                                "background-color" => mix.bg = Some(Some(c)),
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
                    for it in items.into_iter().rev() {
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

fn scale_font_height(base: u16, scale: f64) -> Option<u16> {
    let height = (f64::from(base) * scale).round();
    (height >= 1.0 && height <= f64::from(u16::MAX)).then_some(height as u16)
}

/// GNU slant mapping: italic/oblique slant the text.
fn slant_italic(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Sym(s) => !matches!(
            i.symbol_name(*s).as_str(),
            "normal"
                | "roman"
                | "unspecified"
                | "reverse-oblique"
                | "reverse-italic"
                | "reversed-oblique"
                | "reversed-italic"
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
            mix.bg = Some(Some(CellColor(190, 190, 190)));
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
    /// The window whose redisplay is using these face values.
    window: Value,
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
    overlay_strings: HashMap<usize, Vec<DisplayText>>,
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
    before: Option<DisplayText>,
    after: Option<DisplayText>,
}

fn window_faces(
    i: &Interp,
    ctx: &mut FaceCtx,
    b: &std::rc::Rc<std::cell::RefCell<Buffer>>,
    win: &Value,
    mouse_position: Option<MousePosition>,
) -> WindowFaces {
    let bb = b.borrow();
    let default_font_height = ctx
        .default_style
        .and_then(|style| style.font_height)
        .unwrap_or(140);
    let mouse_position = mouse_position
        .filter(|position| position.buffer_id == bb.id)
        .map(|position| position.buffer_position);
    // GNU's `redisplay--update-region-highlight' only displays the
    // region while Transient Mark mode and `mark-active' are both set.
    // Nonselected windows use their own window point, and are eligible
    // only when `highlight-nonselected-windows' is non-nil.
    let region = {
        let active = i
            .buffer_local_toplevel_value(ctx.mark_active, bb.id)
            .or_else(|| i.default_toplevel_value(ctx.mark_active))
            .or_else(|| bb.locals.get(&ctx.mark_active).cloned())
            .map(|v| v.truthy())
            .unwrap_or(false);
        let transient_mark_mode = buffer_setting(i, "transient-mark-mode", bb.id).truthy();
        let highlight_nonselected =
            buffer_setting(i, "highlight-nonselected-windows", bb.id).truthy();
        let window_present = matches!(win, Value::Window(_));
        let selected_window = match win {
            Value::Window(window) => crate::editor::sel_window(i)
                .is_some_and(|selected| std::rc::Rc::ptr_eq(&selected, window)),
            _ => false,
        };
        let point = match win {
            Value::Window(_window) if selected_window => bb.point(),
            Value::Window(window) => window.borrow().point,
            _ => bb.point(),
        }
        .clamp(bb.begv, bb.zv);
        let mark = bb.mark.as_ref().and_then(|m| {
            let mm = m.borrow();
            (mm.buffer == Some(bb.id)).then_some(mm.position)
        });
        match (
            active
                && transient_mark_mode
                && window_present
                && (selected_window || highlight_nonselected),
            mark,
        ) {
            (true, Some(mk)) if mk != point => Some((mk.min(point), mk.max(point))),
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
    let font_lock_enabled = buffer_setting(i, "font-lock-mode", bb.id).truthy();
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
            let face = ov.property(i, &Value::Sym(ctx.face));
            let font_lock_face = if font_lock_enabled {
                ov.property(i, &Value::Sym(ctx.font_lock_face))
            } else {
                Value::Nil
            };
            let fv = if font_lock_face.is_nil() {
                face
            } else {
                font_lock_face
            };
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
    let mut overlay_strings: Vec<(usize, crate::buffer::Overlay, OverlayStrings)> = bb
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, overlay)| {
            overlay.buffer == Some(bb.id) && overlay.matches_window(i, win)
        })
        .filter_map(|(overlay_index, overlay)| {
            let mouse_face_active = mouse_position.is_some_and(|position| {
                position >= overlay.start && position < overlay.end
            });
            let mut string_value = |property: Option<crate::lisp::value::SymId>| {
                property.and_then(|property| match overlay.property(i, &Value::Sym(property)) {
                    Value::Str(value) => {
                        let mut text =
                            display_text_from_string(i, ctx, &value, mouse_face_active).0;
                        text.mouse_sources.fill(Some(MouseObjectSource::OverlayProperty {
                            index: overlay_index,
                            property,
                        }));
                        Some(text)
                    }
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
                .then(|| (overlay_index, overlay.clone(), strings))
        })
        .collect();
    let mut overlay_order: Vec<_> = overlay_strings
        .iter()
        .enumerate()
        .map(|(index, (_, overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlay_order);
    let overlay_strings: Vec<OverlayStrings> = overlay_order
        .into_iter()
        .map(|(_, index)| overlay_strings[index].2.clone())
        .collect();
    let mut ending_strings: HashMap<usize, Vec<DisplayText>> = HashMap::new();
    let mut starting_strings: HashMap<usize, Vec<(Option<DisplayText>, Option<DisplayText>)>> =
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
        .filter(|(_, tp)| {
            tp.prop == ctx.face || (font_lock_enabled && tp.prop == ctx.font_lock_face)
        })
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
    let display_property = i.q_soft("display");
    let mut display_props: Vec<(usize, usize, Option<DisplayReplacement>, usize)> =
        display_property.map_or_else(Vec::new, |display| {
            let mut runs: Vec<(usize, usize, Option<DisplayReplacement>, usize)> = Vec::new();
            let mut previous_value: Option<&Value> = None;
            for (idx, property) in properties
                .iter()
                .enumerate()
                .filter(|(_, tp)| tp.prop == display)
            {
                let start = property.start.max(bb.begv);
                let end = property.end.min(bb.zv);
                if start >= end {
                    continue;
                }
                if let Some(last) = runs.last_mut()
                    && last.1 == start
                    && previous_value
                        .is_some_and(|value| crate::lisp::eq_values(value, &property.value))
                {
                    last.1 = end;
                    continue;
                }
                runs.push((
                    start,
                    end,
                    display_value_replacement(
                        i,
                        ctx,
                        &property.value,
                        bb.id,
                        default_font_height,
                        mouse_position.is_some_and(|position| {
                            position >= start && position < end
                        }),
                        Some(MouseObjectSource::TextProperty {
                            position: start,
                            property: display,
                        }),
                    ),
                    idx,
                ));
                previous_value = Some(&property.value);
            }
            runs
        });
    if let Some(display_property) = display_property {
        let overlay_display: Vec<_> = bb
            .overlays
            .iter()
            .enumerate()
            .filter(|(_, overlay)| {
                overlay.buffer == Some(bb.id)
                    && overlay.start < overlay.end
                    && overlay.matches_window(i, win)
            })
            .filter_map(|(overlay_index, overlay)| {
                let value = overlay.property(i, &Value::Sym(display_property));
                let mouse_face_active = mouse_position.is_some_and(|position| {
                    position >= overlay.start && position < overlay.end
                });
                let replacement = display_value_replacement(
                    i,
                    ctx,
                    &value,
                    bb.id,
                    default_font_height,
                    mouse_face_active,
                    Some(MouseObjectSource::OverlayProperty {
                        index: overlay_index,
                        property: display_property,
                    }),
                )?;
                Some((overlay.clone(), replacement, overlay_index))
            })
            .collect();
        let mut order: Vec<_> = overlay_display
            .iter()
            .enumerate()
            .map(|(index, (overlay, _, _))| (overlay, index))
            .collect();
        crate::buffer::sort_overlays_by_priority(i, &mut order);
        let property_count = properties.len();
        for (priority, (_, index)) in order.into_iter().enumerate() {
            let (overlay, replacement, _) = &overlay_display[index];
            display_props.push((
                overlay.start,
                overlay.end,
                Some(replacement.clone()),
                property_count + priority,
            ));
        }
    }
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
        window: win.clone(),
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

fn overlay_strings_at(faces: &WindowFaces, position: usize) -> Vec<DisplayText> {
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

fn image_transform_smoothing(i: &Interp, plist: &Value) -> Option<bool> {
    let property = i.q_soft(":transform-smoothing")?;
    crate::lisp::eval::plist_lookup(plist, property).map(|value| value.truthy())
}

fn image_size_limit(i: &Interp) -> Option<GridImageSizeLimit> {
    let variable = i.q_soft("max-image-size")?;
    match i.symbol_value(variable) {
        Value::Int(limit) => Some(GridImageSizeLimit::Pixels(limit.max(0) as u64)),
        Value::BigInt(limit) => Some(GridImageSizeLimit::Pixels(if limit.is_negative() {
            0
        } else {
            limit.to_u64().unwrap_or(u64::MAX)
        })),
        Value::Float(limit) if limit.is_finite() => Some(
            GridImageSizeLimit::FrameRatioBits(limit.max(0.0).to_bits()),
        ),
        _ => None,
    }
}

fn image_dimension(i: &Interp, value: Value, default_font_height: u16) -> Option<usize> {
    match value {
        Value::Int(n) if n > 0 => usize::try_from(n).ok(),
        Value::Cons(pair) => {
            let (value, unit) = {
                let pair = pair.borrow();
                (pair.car.clone(), pair.cdr.clone())
            };
            let Value::Sym(unit) = unit else {
                return None;
            };
            if i.symbol_name(unit) != "em" {
                return None;
            }
            let ems = match value {
                Value::Int(value) if value > 0 => value as f64,
                Value::Float(value) if value.is_finite() && *value > 0.0 => *value,
                _ => return None,
            };
            let points_to_logical_pixels = if cfg!(any(target_os = "linux", target_os = "windows")) {
                96.0 / 72.0
            } else {
                1.0
            };
            let pixels = ems * (f64::from(default_font_height) / 10.0) * points_to_logical_pixels;
            (pixels.is_finite() && pixels > 0.0 && pixels <= usize::MAX as f64)
                .then_some(pixels.ceil() as usize)
        }
        _ => None,
    }
}

fn svg_pixel_dimension(value: &[u8]) -> Option<usize> {
    let value = std::str::from_utf8(value).ok()?.trim();
    let numeric_end = value
        .char_indices()
        .find(|(_, character)| {
            !character.is_ascii_digit() && !matches!(character, '.' | '+' | '-' | 'e' | 'E')
        })
        .map(|(index, _)| index)
        .unwrap_or(value.len());
    let number = value[..numeric_end].parse::<f64>().ok()?;
    if !number.is_finite() || number <= 0.0 {
        return None;
    }
    let pixels = match value[numeric_end..].trim() {
        "" | "px" => number,
        "pt" => number * (96.0 / 72.0),
        "pc" => number * 16.0,
        "in" => number * 96.0,
        "cm" => number * (96.0 / 2.54),
        "mm" => number * (96.0 / 25.4),
        _ => return None,
    };
    (pixels < usize::MAX as f64).then_some(pixels.round() as usize)
}

fn svg_intrinsic_dimensions(bytes: &[u8]) -> (Option<usize>, Option<usize>) {
    use quick_xml::events::Event;

    let Ok(source) = std::str::from_utf8(bytes) else {
        return (None, None);
    };
    let mut reader = quick_xml::Reader::from_str(source);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element))
                if element
                    .name()
                    .as_ref()
                    .rsplit(|byte| *byte == b':')
                    .next()
                    == Some(b"svg".as_slice()) =>
            {
                let mut width = None;
                let mut height = None;
                for attribute in element.attributes().flatten() {
                    match attribute.key.as_ref() {
                        b"width" => width = svg_pixel_dimension(attribute.value.as_ref()),
                        b"height" => height = svg_pixel_dimension(attribute.value.as_ref()),
                        _ => {}
                    }
                }
                return (width, height);
            }
            Ok(Event::Eof) | Err(_) => return (None, None),
            _ => {}
        }
    }
}

fn read_stipple_file(i: &Interp, file: &EmacsStr, default_directory: &EmacsStr) -> Option<GridStipple> {
    let path = i.q_soft("x-bitmap-file-path")
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    let mut directories = Vec::new();
    if path.is_nil() {
        directories.push(default_directory.to_string());
    } else {
        let mut cursor = path;
        let mut visited = HashSet::new();
        while let Value::Cons(cell) = cursor {
            if !visited.insert(std::rc::Rc::as_ptr(&cell) as usize) {
                break;
            }
            let (directory, next) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            let directory = match directory {
                Value::Str(directory) => {
                    crate::editor::expand_against_dir(i, &directory.borrow(), default_directory)
                }
                Value::Nil => default_directory.to_string(),
                _ => EmacsText::from("/"),
            };
            directories.push(directory);
            cursor = next;
        }
    }
    let mut searched = HashSet::new();
    for directory in directories {
        let filename = crate::editor::expand_against_dir(i, file, &directory);
        // GNU openp strips the local-file quoting prefix at the OS boundary.
        let filename = filename.strip_prefix("/:").unwrap_or(&filename);
        let Ok(native) = filename.to_os_string() else {
            continue;
        };
        let path = std::path::PathBuf::from(native);
        if !searched.insert(path.clone()) || !path.is_file() {
            continue;
        }
        let Ok(contents) = std::fs::read(path) else {
            continue;
        };
        // A readable file earlier in the search path shadows later entries,
        // even if its contents are not a valid bitmap.
        let bitmap = super::bitmap::parse_xbm(&contents)?;
        return Some(GridStipple {
            width: bitmap.width,
            height: bitmap.height,
            data: Arc::from(bitmap.data),
        });
    }
    None
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

fn append_image_load_path_directories(
    i: &Interp,
    buffer_id: usize,
    value: Value,
    directories: &mut Vec<String>,
    visited: &mut HashSet<usize>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    match value {
        Value::Str(directory) => {
            directories.push(image_file_for_buffer(i, buffer_id, &directory.borrow()));
        }
        Value::Sym(symbol) => append_image_load_path_directories(
            i,
            buffer_id,
            i.symbol_value(symbol),
            directories,
            visited,
            depth + 1,
        ),
        Value::Cons(cell) => {
            let mut cursor = Value::Cons(cell);
            let mut entered = Vec::new();
            while let Value::Cons(cell) = cursor {
                let identity = std::rc::Rc::as_ptr(&cell) as usize;
                if !visited.insert(identity) {
                    break;
                }
                entered.push(identity);
                let (car, cdr) = {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                };
                append_image_load_path_directories(
                    i,
                    buffer_id,
                    car,
                    directories,
                    visited,
                    depth + 1,
                );
                cursor = cdr;
            }
            for identity in entered {
                visited.remove(&identity);
            }
        }
        _ => {}
    }
}

fn image_file_from_load_path(i: &Interp, buffer_id: usize, file: &EmacsStr) -> Option<String> {
    let file_name = file.to_unicode_lossy();
    if std::path::Path::new(&file_name).is_absolute() {
        return Some(file_name);
    }
    let image_load_path = i.q_soft("image-load-path")?;
    let mut directories = Vec::new();
    append_image_load_path_directories(
        i,
        buffer_id,
        i.symbol_value(image_load_path),
        &mut directories,
        &mut HashSet::new(),
        0,
    );
    directories.into_iter().find_map(|directory| {
        let candidate = std::path::Path::new(&directory).join(&file_name);
        (candidate.is_file() && std::fs::File::open(&candidate).is_ok())
            .then(|| candidate.to_string_lossy().into_owned())
    })
}

fn image_file_for_buffer_or_load_path(i: &Interp, buffer_id: usize, file: &EmacsStr) -> String {
    image_file_from_load_path(i, buffer_id, file)
        .unwrap_or_else(|| image_file_for_buffer(i, buffer_id, file))
}

fn svg_image_stylesheet(i: &Interp, plist: &Value) -> Option<String> {
    let css = match image_property(i, plist, ":css") {
        Value::Str(css) => Some(css.borrow().to_unicode_lossy()),
        _ => None,
    };
    let foreground = match image_property(i, plist, ":foreground") {
        Value::Str(foreground) => Some(foreground.borrow().to_unicode_lossy()),
        _ => None,
    };

    match (css, foreground) {
        (Some(mut css), Some(foreground)) => {
            css.push_str("\nsvg { color: ");
            css.push_str(&foreground);
            css.push_str("; }\n");
            Some(css)
        }
        (Some(css), None) => Some(css),
        (None, Some(foreground)) => Some(format!("svg {{ color: {foreground}; }}")),
        (None, None) => None,
    }
}

fn image_map_pair(value: &Value) -> Option<(Value, Value)> {
    let Value::Cons(cell) = value else {
        return None;
    };
    let cell = cell.borrow();
    Some((cell.car.clone(), cell.cdr.clone()))
}

fn image_map_point(value: &Value) -> Option<(i128, i128)> {
    let (x, y) = image_map_pair(value)?;
    Some((x.int()?, y.int()?))
}

fn image_map_shape(i: &Interp, area: &Value) -> Option<GridImageMapShape> {
    let (tag, body) = image_map_pair(area)?;
    let Value::Sym(tag) = tag else {
        return None;
    };
    match i.symbol_name(tag).as_str() {
        "rect" => {
            let (top_left, bottom_right) = image_map_pair(&body)?;
            let (left, top) = image_map_point(&top_left)?;
            let (right, bottom) = image_map_point(&bottom_right)?;
            Some(GridImageMapShape::Rectangle {
                left,
                top,
                right,
                bottom,
            })
        }
        "circle" => {
            let (center, radius) = image_map_pair(&body)?;
            let (x, y) = image_map_point(&center)?;
            let radius = match radius {
                Value::Int(radius) => radius as f64,
                Value::Float(radius) => *radius,
                _ => return None,
            };
            Some(GridImageMapShape::Circle {
                x,
                y,
                radius_bits: radius.to_bits(),
            })
        }
        "poly" => {
            let Value::Vec(vertices) = body else {
                return None;
            };
            let vertices = vertices.borrow();
            if vertices.len() < 6 || vertices.len() % 2 != 0 {
                return None;
            }
            let points = vertices
                .chunks_exact(2)
                .map(|pair| Some((pair[0].int()?, pair[1].int()?)))
                .collect::<Option<Vec<_>>>()?;
            Some(GridImageMapShape::Polygon(points))
        }
        _ => None,
    }
}

fn image_map_areas(i: &Interp, value: &Value) -> Vec<GridImageMapArea> {
    let mut result = Vec::new();
    let mut seen = HashSet::<usize>::default();
    let mut entries = value.clone();
    while let Value::Cons(cell) = entries.clone() {
        if !seen.insert(std::rc::Rc::as_ptr(&cell) as usize) {
            break;
        }
        let (entry, rest) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        entries = rest;
        let Some((area, tail)) = image_map_pair(&entry) else {
            continue;
        };
        let Some((id, _properties)) = image_map_pair(&tail) else {
            continue;
        };
        let Value::Sym(id) = id else {
            continue;
        };
        if let Some(shape) = image_map_shape(i, &area) {
            result.push(GridImageMapArea { id, shape });
        }
    }
    result
}

fn image_conversion_number(value: &Value) -> Option<i32> {
    match value {
        Value::Int(number) => i32::try_from(*number).ok(),
        Value::Float(number) if number.is_finite() => Some((**number) as i32),
        _ => None,
    }
}

fn image_conversion_matrix(value: &Value) -> Option<[i32; 9]> {
    let mut values = Vec::with_capacity(9);
    match value {
        Value::Vec(vector) => values.extend(vector.borrow().iter().take(9).cloned()),
        Value::Cons(_) => {
            let mut tail = value.clone();
            let mut seen = HashSet::<usize>::default();
            while values.len() < 9 {
                let Value::Cons(cell) = tail.clone() else {
                    break;
                };
                if !seen.insert(std::rc::Rc::as_ptr(&cell) as usize) {
                    return None;
                }
                let (car, cdr) = {
                    let cell = cell.borrow();
                    (cell.car.clone(), cell.cdr.clone())
                };
                values.push(car);
                tail = cdr;
            }
        }
        _ => return None,
    }
    if values.len() < 9 {
        return None;
    }
    let mut matrix = [0; 9];
    for (index, value) in values.iter().take(9).enumerate() {
        matrix[index] = image_conversion_number(value)?;
    }
    Some(matrix)
}

fn image_conversion(i: &Interp, value: Value) -> Option<GridImageConversion> {
    match value {
        Value::Sym(symbol) => match i.symbol_name(symbol).as_str() {
            "disabled" => Some(GridImageConversion::Disabled),
            "laplace" => Some(GridImageConversion::EdgeDetection {
                matrix: [1, 0, 0, 0, 0, 0, 0, 0, -1],
                color_adjust: 45_000,
            }),
            "emboss" => Some(GridImageConversion::EdgeDetection {
                matrix: [2, -1, 0, -1, 0, 1, 0, 1, -2],
                color_adjust: 32_767,
            }),
            _ => None,
        },
        Value::Cons(cell) => {
            let (kind, properties) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            if !matches!(kind, Value::Sym(symbol) if i.symbol_name(symbol) == "edge-detection") {
                return None;
            }
            let matrix = image_conversion_matrix(&image_property(i, &properties, ":matrix"))?;
            let adjustment = image_property(i, &properties, ":color-adjust");
            let color_adjust = if adjustment.is_nil() {
                32_767
            } else {
                image_conversion_number(&adjustment)?
            };
            Some(GridImageConversion::EdgeDetection {
                matrix,
                color_adjust,
            })
        }
        _ => None,
    }
}

fn image_display_replacement(
    i: &Interp,
    value: &Value,
    buffer_id: usize,
    default_font_height: u16,
    object_source: Option<MouseObjectSource>,
    slice: Option<GridImageSlice>,
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

    let width = image_dimension(
        i,
        image_property(i, &plist, ":width"),
        default_font_height,
    );
    let height = image_dimension(
        i,
        image_property(i, &plist, ":height"),
        default_font_height,
    );
    let index = image_property(i, &plist, ":index")
        .int()
        .and_then(|index| usize::try_from(index).ok());
    let conversion = image_conversion(i, image_property(i, &plist, ":conversion"));
    let transform_smoothing = image_transform_smoothing(i, &plist);
    let masked = !image_property(i, &plist, ":mask").is_nil();
    let map = image_map_areas(i, &image_property(i, &plist, ":map"));
    let file = image_property(i, &plist, ":file");
    if let Value::Str(file) = file {
        let path = image_file_for_buffer_or_load_path(i, buffer_id, &file.borrow());
        let declared_svg =
            matches!(image_property(i, &plist, ":type"), Value::Sym(id) if i.symbol_name(id) == "svg");
        let is_svg = declared_svg || path.to_ascii_lowercase().ends_with(".svg");
        let base_uri = if is_svg {
            match image_property(i, &plist, ":base-uri") {
                Value::Str(base_uri) => Some(image_file_for_buffer(
                    i,
                    buffer_id,
                    &base_uri.borrow(),
                )),
                _ => None,
            }
        } else {
            None
        };
        let css = if is_svg {
            svg_image_stylesheet(i, &plist)
        } else {
            None
        };
        let source = if base_uri.is_some() || css.is_some() {
            GridImageSource::FileWithSvgOptions {
                path,
                base_uri,
                css,
            }
        } else {
            GridImageSource::File(path)
        };
        return Some(DisplayReplacement::Image {
            source,
            object_source,
            index,
            conversion,
            transform_smoothing,
            width,
            height,
            masked,
            intrinsic_width: None,
            intrinsic_height: None,
            map,
            slice,
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
    let bytes = if format == "svg" {
        data.borrow()
            .chars()
            .try_fold(Vec::new(), |mut bytes, character| {
                if let Some(byte) = character.byte8() {
                    bytes.push(byte);
                } else if let Some(unicode) = character.unicode() {
                    let mut encoded = [0; 4];
                    bytes.extend_from_slice(unicode.encode_utf8(&mut encoded).as_bytes());
                } else if let Ok(byte) = u8::try_from(character.code()) {
                    bytes.push(byte);
                } else {
                    return None;
                }
                Some(bytes)
            })?
    } else {
        data.borrow()
            .chars()
            .map(|character| {
                character
                    .byte8()
                    .or_else(|| u8::try_from(character.code()).ok())
            })
            .collect::<Option<Vec<_>>>()?
    };
    let base_uri = if format == "svg" {
        match image_property(i, &plist, ":base-uri") {
            Value::Str(base_uri) => Some(image_file_for_buffer(
                i,
                buffer_id,
                &base_uri.borrow(),
            )),
            _ => None,
        }
    } else {
        None
    };
    let css = if format == "svg" {
        svg_image_stylesheet(i, &plist)
    } else {
        None
    };
    let (intrinsic_width, intrinsic_height) = if format == "svg" {
        svg_intrinsic_dimensions(&bytes)
    } else {
        (None, None)
    };
    let bytes = Arc::<[u8]>::from(bytes);
    let source = if css.is_some() {
        GridImageSource::DataWithSvgOptions {
            format,
            bytes,
            base_uri,
            css,
        }
    } else if let Some(base_uri) = base_uri {
        GridImageSource::DataWithBaseUri {
            format,
            bytes,
            base_uri,
        }
    } else {
        GridImageSource::Data { format, bytes }
    };
    Some(DisplayReplacement::Image {
        source,
        object_source,
        index,
        conversion,
        transform_smoothing,
        width,
        height,
        masked,
        intrinsic_width,
        intrinsic_height,
        map,
        slice,
    })
}

fn display_spec_number(i: &Interp, value: &Value, depth: usize) -> Option<f64> {
    if depth >= 8 {
        return None;
    }
    match value {
        Value::Int(number) => Some(*number as f64),
        Value::BigInt(number) => number.to_f64(),
        Value::Float(number) if number.is_finite() => Some(**number),
        Value::Sym(symbol) => {
            display_spec_number(i, &i.symbol_value(*symbol), depth + 1)
        }
        _ => None,
    }
}

fn display_space_spec_width(i: &Interp, plist: &Value) -> usize {
    let width = image_property(i, plist, ":width");
    let align_to = image_property(i, plist, ":align-to");
    let value = if width.is_nil() { &align_to } else { &width };
    display_spec_number(i, value, 0)
        .unwrap_or(1.0)
        .ceil()
        .clamp(0.0, 1_000_000.0) as usize
}

fn image_slice_part(value: &Value) -> Option<GridImageSlicePart> {
    match value {
        Value::Int(pixels) => usize::try_from(*pixels).ok().map(GridImageSlicePart::Pixels),
        Value::BigInt(pixels) => pixels.to_usize().map(GridImageSlicePart::Pixels),
        Value::Float(fraction) if fraction.is_finite() && (0.0..=1.0).contains(&**fraction) => {
            Some(GridImageSlicePart::Fraction(fraction.to_bits()))
        }
        _ => None,
    }
}

fn image_slice_spec(i: &Interp, value: &Value) -> Option<(GridImageSlice, Value)> {
    let mut elements = value.list_to_vec().ok()?.into_iter();
    let slice_spec = elements.next()?;
    let image_desc = elements.next()?;
    if elements.next().is_some() {
        return None;
    }
    let mut parts = slice_spec.list_to_vec().ok()?.into_iter();
    let Value::Sym(name) = parts.next()? else {
        return None;
    };
    if i.symbol_name(name) != "slice" {
        return None;
    }
    let slice = GridImageSlice {
        x: image_slice_part(&parts.next()?)?,
        y: image_slice_part(&parts.next()?)?,
        width: image_slice_part(&parts.next()?)?,
        height: image_slice_part(&parts.next()?)?,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((slice, image_desc))
}

fn display_value_replacement(
    i: &Interp,
    faces: &mut FaceCtx,
    value: &Value,
    buffer_id: usize,
    default_font_height: u16,
    mouse_face_active: bool,
    object_source: Option<MouseObjectSource>,
) -> Option<DisplayReplacement> {
    if let Some((slice, image_desc)) = image_slice_spec(i, value) {
        return image_display_replacement(
            i,
            &image_desc,
            buffer_id,
            default_font_height,
            object_source,
            Some(slice),
        );
    }
    if let Value::Cons(spec) = value {
        let (head, tail) = {
            let spec = spec.borrow();
            (spec.car.clone(), spec.cdr.clone())
        };
        if let Value::Sym(head) = head {
            let name = i.symbol_name(head);
            if matches!(name.as_str(), "left-fringe" | "right-fringe") {
                let Value::Cons(arguments) = tail else {
                    return None;
                };
                let (bitmap, tail) = {
                    let arguments = arguments.borrow();
                    (arguments.car.clone(), arguments.cdr.clone())
                };
                let bitmap = i
                    .sym_id(&bitmap)
                    .and_then(|symbol| {
                        i.q_soft("fringe")
                            .map(|property| i.get_prop(symbol, property))
                    })
                    .and_then(|index| index.int())
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|index| super::fringe::defined(*index));
                let Some(bitmap) = bitmap else {
                    return Some(DisplayReplacement::Hidden);
                };
                let mut style = faces.builtin(i, "default", 0);
                style.merge(&faces.builtin(i, "fringe", 0));
                if let Value::Cons(face) = tail {
                    style.merge(&faces.face_value(i, &face.borrow().car, 0));
                }
                return Some(DisplayReplacement::Fringe(RenderedFringe {
                    left: name == "left-fringe",
                    bitmap,
                    style: style.finish(),
                }));
            }
            if name == "space" {
                return Some(DisplayReplacement::Space(display_space_spec_width(i, &tail)));
            }
        }
    }
    match value {
        Value::Str(string) => {
            let (mut display_text, has_styles) =
                display_text_from_string(i, faces, string, mouse_face_active);
            if let Some(source) = object_source {
                display_text.mouse_sources.fill(Some(source));
                return Some(DisplayReplacement::StyledText(display_text));
            }
            if has_styles {
                Some(DisplayReplacement::StyledText(display_text))
            } else {
                Some(DisplayReplacement::Text(display_text.text))
            }
        }
        _ => {
            let mut replacement = image_display_replacement(
                i,
                value,
                buffer_id,
                default_font_height,
                object_source,
                None,
            )?;
            if let DisplayReplacement::Image {
                object_source: image_source,
                ..
            } = &mut replacement
            {
                *image_source = object_source;
            }
            Some(replacement)
        }
    }
}

fn display_text_from_string(
    i: &Interp,
    faces: &mut FaceCtx,
    string: &crate::lisp::value::StrRef,
    mouse_face_active: bool,
) -> (DisplayText, bool) {
    let text = string.borrow().to_unicode_lossy();
    let properties = i.str_props(string).to_vec();
    let char_count = text.chars().count();
    let mut face_styles = vec![StyleMix::default(); char_count];
    let mut mouse_face_styles = vec![StyleMix::default(); char_count];
    let mouse_face_active = vec![mouse_face_active; char_count];
    let mut has_face = false;
    let mut has_mouse_face = false;
    for (start, end, plist) in properties {
        let mut face_style = StyleMix::default();
        let mut mouse_face_style = StyleMix::default();
        for pair in plist.chunks_exact(2) {
            let Some(property) = i.sym_id(&pair[0]) else {
                continue;
            };
            if property == faces.face {
                has_face = true;
                face_style.merge(&faces.face_value(i, &pair[1], 0));
            } else if mouse_face_active.first().copied().unwrap_or(false)
                && property == faces.mouse_face
            {
                has_mouse_face = true;
                let mut style = faces.face_value(i, &pair[1], 0);
                style.font_family = None;
                style.font_weight = None;
                style.font_width = None;
                style.font_height = None;
                style.font_height_scale = None;
                style.bold = None;
                style.italic = None;
                mouse_face_style.merge(&style);
            }
        }
        let start = start.min(char_count);
        let end = end.min(char_count);
        if start >= end {
            continue;
        }
        for style in &mut face_styles[start..end] {
            style.merge(&face_style);
        }
        for style in &mut mouse_face_styles[start..end] {
            style.merge(&mouse_face_style);
        }
    }
    (
        DisplayText {
            text,
            face_styles,
            mouse_face_styles,
            mouse_face_active,
            mouse_sources: vec![None; char_count],
        },
        has_face || has_mouse_face,
    )
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

fn emoji_zwj_joiner(chars: &[EmacsChar], index: usize) -> bool {
    if chars.get(index).is_none_or(|character| character.code() != 0x200d) {
        return false;
    }
    let pictographic = |character: EmacsChar| {
        matches!(character.code(), 0x1f000..=0x1faff)
    };
    let extender = |character: EmacsChar| {
        matches!(character.code(), 0xfe0e..=0xfe0f | 0x1f3fb..=0x1f3ff)
    };
    let previous = chars[..index]
        .iter()
        .rev()
        .map(|character| *character)
        .find(|character| !extender(*character));
    let next = chars[index + 1..]
        .iter()
        .copied()
        .find(|character| !extender(*character));
    previous.is_some_and(pictographic) && next.is_some_and(pictographic)
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

fn c1_control_replacement(
    character: EmacsChar,
    escape_glyph_style: CellStyle,
) -> Option<DisplayReplacement> {
    let code = character.code();
    (0x80..=0x9f)
        .contains(&code)
        .then(|| DisplayReplacement::GlyphlessText(format!("\\{code:03o}"), escape_glyph_style))
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

fn filtered_face_spec(i: &Interp, value: &Value) -> Option<(Value, Value)> {
    let Value::Cons(_) = value else {
        return None;
    };
    let items = value.list_to_vec().ok()?;
    if items.len() != 3
        || !i
            .sym_id(&items[0])
            .is_some_and(|symbol| i.symbol_name(symbol) == ":filtered")
    {
        return None;
    }
    Some((items[1].clone(), items[2].clone()))
}

fn face_filter_matches(i: &Interp, filter: &Value, window: &Value) -> bool {
    if i.q_soft("face-filters-always-match")
        .is_some_and(|symbol| i.symbol_value(symbol).truthy())
    {
        return true;
    }
    let Value::Window(window) = window else {
        return false;
    };
    let Ok(items) = filter.list_to_vec() else {
        return false;
    };
    if items.len() != 3
        || !i
            .sym_id(&items[0])
            .is_some_and(|symbol| i.symbol_name(symbol) == ":window")
    {
        return false;
    }
    let parameters = window.borrow().params.clone();
    let actual = crate::lisp::eval::plist_get_v(&parameters, &items[1]);
    crate::lisp::builtins::eq_values_with_interp(i, &actual, &items[2])
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
    if let Some((filter, face_spec)) = filtered_face_spec(i, value) {
        return if face_filter_matches(i, &filter, &faces.window) {
            face_value_with_remapping(
                i,
                ctx,
                faces,
                &face_spec,
                depth + 1,
                remapping_stack,
            )
        } else {
            StyleMix::default()
        };
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
        for spec in items.iter().rev() {
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
        "hourglass" => GridPointerShape::Hourglass,
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
    render_grid_internal(i, width, height, true, cached, Some(frame), None)
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
    render_grid_internal(
        i,
        width,
        height,
        true,
        cached,
        Some(frame),
        mouse_position,
    )
}

fn frame_gamma_exponent(i: &Interp, frame: &crate::editor::FrameRef) -> Option<f64> {
    let parameter = i.intern_soft("screen-gamma")?;
    let value = crate::lisp::eval::plist_get_v(
        &frame.borrow().params,
        &Value::Sym(parameter),
    );
    let screen_gamma = match value {
        Value::Int(value) if value > 0 => value as f64,
        Value::Float(value) if value.is_finite() && *value > 0.0 => *value,
        _ => return None,
    };
    // GNU's viewing gamma is 0.4545. `screen-gamma' specifies the
    // display's gamma, so a value of 2.2 leaves the colors unchanged.
    let exponent = 1.0 / (0.4545 * screen_gamma);
    exponent.is_finite().then_some(exponent)
}

fn face_near_same_color_threshold(i: &Interp) -> i128 {
    i.intern_soft("face-near-same-color-threshold")
        .and_then(|symbol| i.symbol_value(symbol).int())
        .unwrap_or(30_000)
}

fn color_distance(first: CellColor, second: CellColor) -> i128 {
    let channel = |value: u8| i64::from(value) * 257;
    let red = channel(first.0) - channel(second.0);
    let green = channel(first.1) - channel(second.1);
    let blue = channel(first.2) - channel(second.2);
    let red_mean = (channel(first.0) + channel(second.0)) >> 1;
    let red_term = ((2 * 65_536 + red_mean) * red * red) >> 16;
    let blue_term = ((2 * 65_536 + 65_535 - red_mean) * blue * blue) >> 16;
    let distance = (red_term + 4 * green * green + blue_term) >> 16;
    i128::from(distance)
}

fn gamma_correct_color(color: CellColor, exponent: f64) -> CellColor {
    let channel = |value: u8| {
        ((f64::from(value) / 255.0).powf(exponent) * 255.0 + 0.5)
            .floor()
            .clamp(0.0, 255.0) as u8
    };
    CellColor(channel(color.0), channel(color.1), channel(color.2))
}

fn gamma_correct_style(style: &mut CellStyle, exponent: f64) {
    let correct = |color: &mut Option<CellColor>| {
        if let Some(color) = color {
            *color = gamma_correct_color(*color, exponent);
        }
    };
    correct(&mut style.fg);
    correct(&mut style.bg);
    correct(&mut style.distant_foreground);
    correct(&mut style.underline_color);
    correct(&mut style.strike_color);
    if let Some(box_style) = &mut style.box_style {
        correct(&mut box_style.color);
    }
}

fn resolve_grid_style(style: &mut CellStyle, exponent: Option<f64>, threshold: i128) {
    if let Some(exponent) = exponent {
        gamma_correct_style(style, exponent);
    }
    if let Some(distant_foreground) = style.distant_foreground.take() {
        let (foreground, background) = if style.inverse {
            (style.bg, style.fg)
        } else {
            (style.fg, style.bg)
        };
        if threshold > 0
            && let (Some(foreground), Some(background)) = (foreground, background)
            && color_distance(foreground, background) < threshold
        {
            if style.inverse {
                style.bg = Some(distant_foreground);
            } else {
                style.fg = Some(distant_foreground);
            }
        }
    }
}

fn resolve_grid_colors(grid: &mut Grid, exponent: Option<f64>, threshold: i128) {
    resolve_grid_style(&mut grid.default_style, exponent, threshold);
    resolve_grid_style(&mut grid.internal_border_style, exponent, threshold);
    if let Some(color) = &mut grid.default_bg {
        if let Some(exponent) = exponent {
            *color = gamma_correct_color(*color, exponent);
        }
    }
    for row in &mut grid.attrs {
        for style in row {
            resolve_grid_style(style, exponent, threshold);
        }
    }
    for row in &mut grid.char_attrs {
        for style in row {
            resolve_grid_style(style, exponent, threshold);
        }
    }
    for indicator in &mut grid.fill_column_indicators {
        resolve_grid_style(&mut indicator.style, exponent, threshold);
    }
    for bar in &mut grid.scroll_bars {
        resolve_grid_style(&mut bar.style, exponent, threshold);
    }
    for bitmap in &mut grid.fringe_bitmaps {
        resolve_grid_style(&mut bitmap.style, exponent, threshold);
    }
    for divider in &mut grid.bottom_dividers {
        resolve_grid_style(&mut divider.style, exponent, threshold);
        resolve_grid_style(&mut divider.first_pixel_style, exponent, threshold);
        resolve_grid_style(&mut divider.last_pixel_style, exponent, threshold);
    }
    for divider in &mut grid.right_dividers {
        resolve_grid_style(&mut divider.style, exponent, threshold);
        resolve_grid_style(&mut divider.first_pixel_style, exponent, threshold);
        resolve_grid_style(&mut divider.last_pixel_style, exponent, threshold);
    }
    for composition in &mut grid.compositions {
        resolve_grid_style(&mut composition.style, exponent, threshold);
    }
    for cursor in &mut grid.cursors {
        if let Some(color) = &mut cursor.color {
            if let Some(exponent) = exponent {
                *color = gamma_correct_color(*color, exponent);
            }
        }
    }
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
    let max_image_size = image_size_limit(i);
    let frame = frame_override.cloned().or_else(|| i.selected_frame.clone());
    let frame_resize_pixelwise = i
        .q_soft("frame-resize-pixelwise")
        .is_some_and(|symbol| i.symbol_value(symbol).truthy());
    let gamma_exponent = frame
        .as_ref()
        .filter(|frame| crate::editor::frame_is_graphical(i, frame))
        .and_then(|frame| frame_gamma_exponent(i, frame));
    let mut rows = vec![String::new(); height];
    let mut fctx = FaceCtx::new(i, frame.as_ref());
    let default_style = fctx.builtin(i, "default", 0).finish();
    fctx.default_style = Some(default_style);
    let internal_border_style = fctx.builtin(i, "internal-border", 0).finish();
    let default_bg = default_style.bg;
    let mut attrs = vec![vec![default_style; width]; height];
    let mut row_heights = vec![19; height];
    let mut default_line_height = 19;
    let mut default_row_height = 19;
    let mut char_attrs = vec![Vec::new(); height];
    let mut images = Vec::new();
    let mut bidi_disabled_cell_ranges = Vec::<(usize, usize, usize)>::new();
    let mut fringe_bitmaps = Vec::new();
    let mut bidi_paragraph_cell_ranges =
        Vec::<(usize, usize, usize, GridBidiDirection)>::new();
    let mut pending_compositions = Vec::<(usize, usize, Vec<char>, CellStyle)>::new();
    let mut fill_column_indicators = Vec::new();
    let menus = render_menu_bar(i);
    let mut tool_bar_items = Vec::new();
    let mut tab_bar_items = Vec::new();
    let mut help_echoes = Vec::new();
    let mut pointer_regions = Vec::new();
    let tooltip = i.gui_tip.clone();
    let mut popup_menu_items = Vec::new();
    let mut scroll_bars = Vec::new();
    let mut bottom_dividers = Vec::new();
    let mut right_dividers = Vec::new();
    let mut mode_lines = Vec::new();
    let mut header_lines = Vec::new();
    let mut window_text_areas = Vec::new();
    let mut vscrolls = Vec::new();
    let mut mode_rows = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let mut positioned_cursors = Vec::new();
    let mut input_screen_lefts = Vec::new();
    let mut cursor_index = None;
    let mut hover_cursor = None;
    let dragging_symbol = i.q_soft("dragging");
    let freeze_pointer_shape = i
        .q_soft("track-mouse")
        .map(|symbol| i.symbol_value(symbol))
        .is_some_and(
            |value| matches!(value, Value::Sym(symbol) if Some(symbol) == dragging_symbol),
        );
    let (blink_cursor, blink_interval, blink_delay, blink_blinks) = blink_cursor_settings(i);
    let mut input = if include_input {
        input_snapshot(i, frame.as_ref(), cached_input)
    } else {
        None
    };
    let frame = match frame {
        Some(f) => f,
        None => {
            return Grid {
                title: "Remacs".to_owned(),
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
                compositions: Vec::new(),
                attrs,
                char_attrs,
                mode_rows,
                cursor: cursor_pos,
                cursors: Vec::new(),
                cursor_index,
                bidi_disabled_ranges: Vec::new(),
                bidi_paragraph_ranges: Vec::new(),
                frame_resize_pixelwise,
                pixelwise_segments: Vec::new(),
                blink_cursor,
                blink_interval,
                blink_delay,
                blink_blinks,
                blink_elapsed_since_input: 0.0,
                default_bg,
                gamma_exponent: None,
                row_heights,
                default_line_height,
                default_row_height,
                hover_cursor,
                freeze_pointer_shape,
                pointer_visible: true,
                mouse_color: None,
                input,
            };
        }
    };
    let selected_frame = i
        .selected_frame
        .as_ref()
        .is_some_and(|selected| std::rc::Rc::ptr_eq(selected, &frame));
    let fb = frame.borrow();
    let graphical = crate::editor::frame_is_graphical(i, &frame);
    let cursor_color = frame_cursor_color(i, &mut fctx, &fb);
    let scroll_bar_style = if graphical {
        frame_scroll_bar_style(i, &mut fctx, &fb)
    } else {
        CellStyle::default()
    };
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
    let right_divider_width = if graphical {
        crate::editor::frame_parameter_positive_size(i, &frame, "right-divider-width", 0)
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
    let right_divider_styles = (right_divider_width > 0).then(|| {
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
    let mut popup_pixel_position = i
        .gui_popup_menu
        .as_ref()
        .filter(|menu| {
            menu.position.frame_id == fb.id && menu.position.anchor == super::PopupMenuAnchor::Frame
        })
        .map(|menu| (menu.position.x, menu.position.y));
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
    let mut pixelwise_row_boundaries = Vec::<(usize, i128)>::new();
    let mut pixelwise_cell_segments = Vec::<(usize, usize, usize, i128, i128)>::new();
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
            &fb,
            width,
            tool_bar_rows,
            tab_bar_rows,
            default_style,
            &mut fctx,
            &mut screen_rows,
        );
    }
    let mut selected_cursor: Option<(usize, usize, usize, usize)> = None;
    let mut pixelwise_windows = fb.root.iter().cloned().collect::<Vec<_>>();
    while let Some(window) = pixelwise_windows.pop() {
        let (top, window_height, children) = {
            let window = window.borrow();
            (
                window.top,
                window.height,
                window.children.clone(),
            )
        };
        pixelwise_windows.extend(children);
        if window_height > 0 {
            let top_row = scale_edge(top, root_top, scale_y, body_height)
                .saturating_add(chrome_rows);
            let bottom = scale_edge(
                top.saturating_add(window_height),
                root_top,
                scale_y,
                body_height,
            )
            .saturating_add(chrome_rows)
            .min(height);
            if bottom > top_row {
                let (_, top_offset) =
                    crate::editor::winxtra::pixelwise_window_origin_offset(&window);
                let height_adjustment = crate::editor::winxtra::pixelwise_window_extent_adjustment(
                    &window, false,
                );
                pixelwise_row_boundaries.push((bottom - 1, top_offset + height_adjustment));
            }
        }
    }

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
            let (x_offset, _) =
                crate::editor::winxtra::pixelwise_window_origin_offset(w);
            let width_adjustment =
                crate::editor::winxtra::pixelwise_window_extent_adjustment(w, true);
            bottom_dividers.push(GridBottomDivider {
                row_boundary: raw_window_bottom,
                left: raw_window_left,
                width: raw_window_width,
                x_offset,
                width_adjustment,
                thickness: bottom_divider_width,
                style,
                first_pixel_style,
                last_pixel_style,
            });
        }
        let has_right_neighbor = fb.windows.iter().any(|other| {
            let other = other.borrow();
            other.id != window_id
                && !other.minibuffer
                && other.left == raw_left.saturating_add(raw_width)
                && other.top < raw_top.saturating_add(raw_height)
                && raw_top < other.top.saturating_add(other.height)
        });
        if !minibuffer_window
            && raw_window_width > 0
            && raw_window_height > 0
            && has_right_neighbor
            && let Some((style, first_pixel_style, last_pixel_style)) = right_divider_styles
        {
            let (_, y_offset) = crate::editor::winxtra::pixelwise_window_origin_offset(w);
            let width_adjustment =
                crate::editor::winxtra::pixelwise_window_extent_adjustment(w, true);
            let height_adjustment =
                crate::editor::winxtra::pixelwise_window_extent_adjustment(w, false)
                    .saturating_sub(bottom_divider_width as i128);
            right_dividers.push(GridRightDivider {
                column_boundary: raw_window_right,
                top: raw_window_top,
                bottom: raw_window_bottom,
                width_adjustment,
                y_offset,
                height_adjustment,
                thickness: right_divider_width,
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
        let (pixelwise_x_offset, _) =
            crate::editor::winxtra::pixelwise_window_origin_offset(w);
        let pixelwise_width_adjustment =
            crate::editor::winxtra::pixelwise_window_extent_adjustment(w, true);
        if pixelwise_x_offset != 0 || pixelwise_width_adjustment != 0 {
            let left = window_left.min(width);
            let right = window_left.saturating_add(window_width).min(width);
            for row in window_top..window_top.saturating_add(window_height).min(height) {
                pixelwise_cell_segments.push((
                    row,
                    left,
                    right,
                    pixelwise_x_offset,
                    pixelwise_width_adjustment,
                ));
            }
        }
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
        let use_buffer_point = selected_frame && selected_window;
        // Untagged legacy input refers to this frame's selected window.
        let mouse_position = mouse_position.filter(|position| {
            position.buffer_id == buf_id
                && position.window_id.map_or(selected_window, |id| id == window_id)
        });
        let cursor_style = cursor_type_for_window(i, w, buf_id, selected_frame && selected_window);
        let bidi_display_reordering =
            buffer_setting(i, "bidi-display-reordering", buf_id).truthy();
        let bidi_paragraph_direction = match buffer_setting(i, "bidi-paragraph-direction", buf_id)
        {
            Value::Sym(symbol) if i.symbol_name(symbol) == "left-to-right" => {
                Some(GridBidiDirection::LeftToRight)
            }
            Value::Sym(symbol) if i.symbol_name(symbol) == "right-to-left" => {
                Some(GridBidiDirection::RightToLeft)
            }
            _ => None,
        };
        let stretch_cursor = buffer_setting(i, "x-stretch-cursor", buf_id).truthy();
        let tab_width = buffer_tab_width(i, buf_id);
        let ctl_arrow = buffer_setting(i, "ctl-arrow", buf_id).truthy();
        let nobreak_char_display = buffer_setting(i, "nobreak-char-display", buf_id);
        let partial_width_setting =
            buffer_setting(i, "truncate-partial-width-windows", buf_id);
        let partial_width_truncation = if raw_window_width < root_width {
            match partial_width_setting.as_bigint() {
                Some(threshold) => num_bigint::BigInt::from(raw_window_width) < threshold,
                None => partial_width_setting.truthy(),
            }
        } else {
            false
        };
        let wrap_lines = !buffer_setting(i, "truncate-lines", buf_id).truthy()
            && !partial_width_truncation
            && hscroll == 0;
        let mode_line_format = super::mode_line::format(i, buf_id, window_id, "mode-line-format");
        let mode_line_enabled = !buffer_setting(i, "mode-line-format", buf_id).is_nil();
        let tab_line_format = super::mode_line::format(i, buf_id, window_id, "tab-line-format");
        let tab_line_enabled = !buffer_setting(i, "tab-line-format", buf_id).is_nil();
        let header_line_format = super::mode_line::format(i, buf_id, window_id, "header-line-format");
        let header_line_enabled = !buffer_setting(i, "header-line-format", buf_id).is_nil();
        let line_number_mode = buffer_setting(i, "display-line-numbers", buf_id);
        let (line_number_width, line_number_offset) =
            line_number_display_settings(i, buf_id);
        let line_numbers_widen =
            buffer_setting(i, "display-line-numbers-widen", buf_id).truthy()
                || line_number_offset != 0;
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
        let line_numbers_visual =
            matches!(&line_number_mode, Value::Sym(symbol) if i.symbol_name(*symbol) == "visual");
        let line_numbers_relative = matches!(
            &line_number_mode,
            Value::Sym(symbol) if i.symbol_name(*symbol) == "relative"
        );
        let line_numbers_current_absolute =
            buffer_setting(i, "display-line-numbers-current-absolute", buf_id).truthy();
        let point_position = i.buffers.get(buf_id).map_or(start, |buffer| {
            let buffer = buffer.borrow();
            let point = if use_buffer_point {
                buffer.point()
            } else {
                w.borrow().point
            };
            point.clamp(buffer.begv, buffer.zv)
        });
        let visual_number_limit = 100_000usize;
        let visual_point_absolute = if line_numbers_visual {
            i.buffers.get(buf_id).map_or(1, |buffer| {
                let buffer = buffer.borrow();
                let visual_begv = if line_numbers_widen { 0 } else { buffer.begv };
                let visual_zv = if line_numbers_widen {
                    buffer.text_len()
                } else {
                    buffer.zv
                };
                display_row_distance_from_start(
                    &buffer.text,
                    visual_begv,
                    visual_zv,
                    visual_begv,
                    point_position,
                    wrap_lines,
                    content_width,
                    tab_width,
                    ctl_arrow,
                    visual_number_limit,
                )
                .max(0) as i128
                    + 1
                    + line_number_offset
            })
        } else {
            0
        };
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
        let content_bottom = content_top.saturating_add(text_rows);
        if !minibuffer_window && text_rows > 0 && width_after_fringes > 0 {
            window_text_areas.push(GridWindowTextArea {
                window_id,
                left: window_left.saturating_add(fringe_left_cells),
                top: content_top,
                width: width_after_fringes,
                rows: text_rows,
            });
        }
        if let Some(menu) = &i.gui_popup_menu
            && menu.position.frame_id == fb.id
        {
            let position = &menu.position;
            let origin = match position.anchor {
                super::PopupMenuAnchor::Window(id) if id == window_id => {
                    Some((raw_window_left, raw_window_top))
                }
                super::PopupMenuAnchor::TextArea(id) if id == window_id => {
                    Some((content_left_with_margin, content_top))
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
                    let mut start_offset = new_start.saturating_sub(new_start_line_pos);
                    let point_line_pos = bb.text.line_start(pt_line).max(bb.begv);
                    let point_index = pt.saturating_sub(point_line_pos);
                    if new_start_line == pt_line && point_index < start_offset {
                        // Point may move to an earlier visual row within the
                        // same physical line. The physical-line scroll pass
                        // above cannot move `start' back in that case, so
                        // anchor it to the visual row containing point.
                        let line_end = bb.text.line_end(point_line_pos).min(bb.zv);
                        let line = bb.text.substring(point_line_pos, line_end);
                        let line_len = line.chars().count();
                        if let Some(range) = wrap_ranges(&line, content_width, tab_width, ctl_arrow)
                            .into_iter()
                            .find(|range| {
                                point_index >= range.start
                                    && (point_index < range.end
                                        || (point_index == range.end && range.end == line_len))
                            })
                        {
                            new_start = point_line_pos.saturating_add(range.start);
                            start_offset = new_start.saturating_sub(new_start_line_pos);
                        }
                    }
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
        let escape_glyph_style = fctx.builtin(i, "escape-glyph", 0).finish();
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
                    let line_number_base = if line_numbers_widen {
                        0
                    } else {
                        first_buffer_line
                    };
                    let point_line_number = bb
                        .text
                        .line_of_pos(point)
                        .saturating_sub(line_number_base);
                    let point_index = (point_line_number == line_index && point >= ls)
                        .then(|| point - ls);
                    (
                        bb.text.substring(ls, le),
                        ls,
                        row_end,
                        point_index,
                        line_index
                            .saturating_sub(line_number_base)
                            .saturating_add(1),
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
                        || faces
                            .invisible_props
                            .iter()
                            .any(|(start, end, _)| {
                                *start < line_pos.saturating_add(line_len) && *end > line_pos
                            })
                        || faces.invisible_overlays.iter().any(|(overlay, _)| {
                            overlay.start < line_pos.saturating_add(line_len)
                                && overlay.end > line_pos
                        })
                })
                .map(|faces| {
                    let mut hidden_run = false;
                    line.chars()
                        .enumerate()
                        .map(|(index, character)| {
                            let position = line_pos + index;
                            let visibility = visibility_at_pos(i, faces, position);
                            if visibility != Visibility::Visible {
                                let replacement = if visibility == Visibility::Ellipsis
                                    && !hidden_run
                                {
                                    EmacsChar::new('…' as u32).unwrap()
                                } else {
                                    EmacsChar::new(0x200b).unwrap()
                                };
                                hidden_run = true;
                                return replacement;
                            }
                            hidden_run = false;
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
            let wrap_prefix_replacement = if wrap_lines {
                let value = prefix_value_at_pos(
                    i,
                    buf_id,
                    &Value::Window(w.clone()),
                    line_pos,
                    "wrap-prefix",
                );
                display_value_replacement(
                    i,
                    &mut fctx,
                    &value,
                    buf_id,
                    default_style.font_height.unwrap_or(140),
                    false,
                    None,
                )
            } else {
                None
            };
            let wrap_prefix_width = wrap_prefix_replacement
                .as_ref()
                .map(|prefix| prefix.columns_after(0, tab_width, frame_char_width))
                .unwrap_or(0)
                .min(content_width.saturating_sub(
                    if line_len > 0 {
                        first_character_width.min(content_width)
                    } else {
                        0
                    },
                ));
            let word_wrap = buffer_setting(i, "word-wrap", buf_id).truthy();
            let word_wrap_by_category = buffer_setting(i, "word-wrap-by-category", buf_id).truthy();
            let ranges = if wrap_lines {
                wrap_ranges_with_prefix(
                    metric_line,
                    content_width,
                    line_prefix_width,
                    wrap_prefix_width,
                    tab_width,
                    ctl_arrow,
                    word_wrap,
                    word_wrap_by_category,
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
                    &line,
                    buf_id,
                    line_pos,
                    range,
                    line_len,
                    max_rows - plans.len(),
                    content_width,
                    line_prefix_width,
                    wrap_prefix_width,
                    tab_width,
                    frame_char_width,
                    ctl_arrow,
                    wrap_lines,
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
                    .push(DisplayText::plain("…".to_owned()));
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
                let continuation_row =
                    segment_start > 0 || plan.continued || plan.repeated_start;
                let prefix_width = if continuation_row {
                    wrap_prefix_width
                } else if segment_start == 0 {
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
                    let visual_distance = if line_numbers_visual {
                        i.buffers.get(buf_id).map_or(0, |buffer| {
                            let buffer = buffer.borrow();
                            display_row_distance_from_start(
                                &buffer.text,
                                buffer.begv,
                                buffer.zv,
                                segment_pos,
                                point_position,
                                wrap_lines,
                                content_width,
                                tab_width,
                                ctl_arrow,
                                visual_number_limit,
                            )
                            .unsigned_abs() as i128
                        })
                    } else {
                        0
                    };
                    let current_number_line = if line_numbers_visual {
                        visual_distance == 0
                    } else {
                        point_line_number + 1 == line_number
                    };
                    let number_style = if current_number_line {
                        fctx.builtin(i, "line-number-current-line", 0).finish()
                    } else {
                        fctx.builtin(i, "line-number", 0).finish()
                    };
                    let show_number = if line_numbers_visual {
                        true
                    } else {
                        segment_start == 0 && !plan.continued && !plan.repeated_start
                    };
                    let number = if show_number {
                        let absolute = (line_number as i128).saturating_add(line_number_offset);
                        let value = if line_numbers_visual && visual_distance == 0 {
                            if line_numbers_current_absolute {
                                visual_point_absolute
                            } else {
                                0
                            }
                        } else if line_numbers_visual {
                            visual_distance
                        } else if line_numbers_relative {
                            if point_line_number + 1 == line_number {
                                if line_numbers_current_absolute {
                                    absolute
                                } else {
                                    0
                                }
                            } else {
                                ((point_line_number as i128 + 1) - line_number as i128).abs()
                            }
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
                    frame_char_width,
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
                                (!(graphical && emoji_zwj_joiner(&segment_chars, index))).then(|| {
                                    glyphless_display_replacement(
                                        i,
                                        buf_id,
                                        character,
                                        graphical,
                                        glyphless_style,
                                    )
                                })?
                            })
                            .or_else(|| {
                                let character = *segment_chars.get(index)?;
                                c1_control_replacement(character, escape_glyph_style)
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
                if prefix_width > 0 {
                    if continuation_row {
                        if let Some(prefix) = wrap_prefix_replacement.as_ref() {
                            let prefix_row = render_display_prefix(
                                prefix,
                                prefix_width,
                                frame_char_width,
                                tab_width,
                                default_style,
                                graphical,
                            );
                            rendered = prepend_rendered_prefix_row(
                                prefix_row,
                                prefix_width,
                                rendered,
                            );
                        }
                    } else if let Some(prefix) = line_prefix.as_ref() {
                        rendered = prepend_rendered_prefix(
                            prefix,
                            prefix_width,
                            tab_width,
                            rendered,
                        );
                    }
                }
                if let Some(row_height) = row_heights.get_mut(y) {
                    for image in &rendered.images {
                        if let Some(image_height) = image.visible_dimensions().1 {
                            *row_height = (*row_height).max(image_height);
                        }
                    }
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
                        object_source: image.object_source,
                        source: image.source.clone(),
                        index: image.index,
                        conversion: image.conversion,
                        transform_smoothing: image.transform_smoothing,
                        max_image_size,
                        width: image.width,
                        height: image.height,
                        intrinsic_width: image.intrinsic_width,
                        intrinsic_height: image.intrinsic_height,
                        map: image.map.clone(),
                        slice: image.slice,
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
                            // `:extend` carries the background, while its
                            // synthetic tail keeps the grid's base font metrics.
                            for cell in rendered.visible_columns..content_width {
                                let mut t = tail;
                                t.box_style = None;
                                t.font_family = None;
                                t.font_weight = None;
                                t.font_width = None;
                                t.font_height = None;
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
                            tail.box_style = None;
                            tail.font_family = None;
                            tail.font_weight = None;
                            tail.font_width = None;
                            tail.font_height = None;
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
                        rendered.object_hits.clone(),
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
                frame_char_width,
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
                    object_source: image.object_source,
                    source: image.source.clone(),
                    index: image.index,
                    conversion: image.conversion,
                    transform_smoothing: image.transform_smoothing,
                    max_image_size,
                    width: image.width,
                    height: image.height,
                    intrinsic_width: image.intrinsic_width,
                    intrinsic_height: image.intrinsic_height,
                    map: image.map.clone(),
                    slice: image.slice,
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
                    rendered.object_hits.clone(),
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
    let mouse_color = frame_mouse_color(i, &mut fctx, &frame.borrow());
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
        mouse_color,
        input,
    };
    resolve_grid_colors(&mut grid, gamma_exponent, face_near_same_color_threshold(i));
    grid
}

#[cfg(test)]
mod mode_line_size_tests {
    use super::{ModeLineContext, format_mode_line_string, readable_mode_line_size};
    use crate::lisp::{Interp, Value};

    #[test]
    fn short_and_readable_accessible_size_constructs() {
        let i = Interp::new();
        let Value::Str(format) = Value::string("%i/%I") else {
            unreachable!();
        };
        let mut context = ModeLineContext::default();
        context.buffer_size = 1234;
        let mut fill_remaining = false;

        assert_eq!(
            format_mode_line_string(&i, 0, &format, &context, &mut fill_remaining),
            "1234/1.2k"
        );
    }

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

#[cfg(test)]
mod box_padding_tests {
    use super::render_grid;
    use crate::{editor::Frame, lisp::Interp};

    fn interp_with_frame(text: &str) -> Interp {
        let mut i = Interp::new();
        let buffer_id = i.create_buffer("test-buf");
        {
            let buffer = i.buffers.get(buffer_id).unwrap();
            buffer.borrow_mut().insert(text);
            buffer.borrow_mut().set_point(0);
        }
        let minibuffer_id = i.create_buffer(" *Minibuf-0*");
        let frame = Frame::new_tty(buffer_id, minibuffer_id, 80, 25);
        i.selected_frame = Some(frame.clone());
        i.frames.push(frame);
        i.set_current_buffer(buffer_id);
        i
    }

    #[test]
    fn synthetic_row_padding_does_not_inherit_face_box() {
        let mut i = interp_with_frame("x\n");
        i.eval_str("(set-face-attribute 'default nil :box t :extend t)")
            .unwrap();

        let grid = render_grid(&i, 6, 24);
        let chars = &grid.char_attrs[0];
        assert!(chars[0].box_style.is_some());
        assert!(chars[1..].iter().all(|style| style.box_style.is_none()));
        assert!(grid.attrs[0][1..]
            .iter()
            .all(|style| style.box_style.is_none()));
    }
}
