//! Resolve faces and render editor state into a frontend-neutral grid.
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use crate::{
    buffer::Buffer,
    lisp::{Interp, Value},
};
use std::{collections::HashMap, ops::Range, sync::Arc};
use unicode_width::UnicodeWidthChar;

/// A terminal color: RGB. Face color specs (names, `#rgb' forms) are
/// resolved to this at render time; the crossterm front-end emits
/// true-color SGR, the gpui front-end paints it directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellColor(pub u8, pub u8, pub u8);

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
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
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
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    overline: Option<bool>,
    strike: Option<bool>,
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
            bold,
            italic,
            underline,
            overline,
            strike,
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
            bold: self.bold.unwrap_or(false),
            italic: self.italic.unwrap_or(false),
            underline: self.underline.unwrap_or(false),
            overline: self.overline.unwrap_or(false),
            strike: self.strike.unwrap_or(false),
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
            && self.bold == o.bold
            && self.italic == o.italic
            && self.underline == o.underline
            && self.overline == o.overline
            && self.strike == o.strike
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
    /// Unicode text and visible positions for the active OS text client.
    pub input: Option<GridInput>,
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

impl Grid {
    /// An all-default grid of WIDTH × HEIGHT cells.
    pub fn blank(width: usize, height: usize) -> Grid {
        Grid {
            title: "Remacs".to_owned(),
            font_families: Vec::new(),
            default_style: CellStyle::default(),
            rows: vec![String::new(); height],
            attrs: vec![vec![CellStyle::default(); width]; height],
            char_attrs: vec![Vec::new(); height],
            mode_rows: Vec::new(),
            cursor: None,
            cursors: Vec::new(),
            cursor_index: None,
            blink_cursor: false,
            blink_interval: 0.5,
            default_bg: None,
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

struct RenderedRow {
    text: String,
    cell_attrs: Vec<CellStyle>,
    char_attrs: Vec<CellStyle>,
    source_offsets: Vec<usize>,
    cell_offsets: Vec<usize>,
    visible_columns: usize,
    padding_char_start: usize,
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
fn render_text_row<F>(
    text: &EmacsText,
    hscroll: usize,
    width: usize,
    tab_width: usize,
    default_style: CellStyle,
    mut style_at: F,
) -> RenderedRow
where
    F: FnMut(usize) -> CellStyle,
{
    let chars: Vec<_> = text.chars().collect();
    let mut rendered = String::new();
    let mut char_attrs = Vec::new();
    let mut cell_attrs = vec![default_style; width];
    let mut source_offsets = vec![0; chars.len() + 1];
    let mut cell_offsets = vec![0; width + 1];
    let mut column = 0usize;
    let mut visible_columns = 0usize;
    let end_column = hscroll.saturating_add(width);
    let tab_width = tab_width.max(1);

    for (index, ch) in chars.iter().enumerate() {
        let display = match ch.unicode().unwrap_or('\u{fffd}') {
            '\n' | '\r' => ' ',
            other => other,
        };
        let columns = if display == '\t' {
            tab_width - column % tab_width
        } else {
            display_char_width(display)
        };
        let start_column = column;
        let next_column = column.saturating_add(columns);
        let before = rendered.len();

        if columns == 0 {
            if start_column >= hscroll && start_column < end_column {
                let style = style_at(index);
                rendered.push(display);
                char_attrs.push(style);
            }
            source_offsets[index + 1] = if next_column <= hscroll {
                0
            } else {
                rendered.len()
            };
            continue;
        }

        if next_column <= hscroll {
            source_offsets[index + 1] = 0;
            column = next_column;
            continue;
        }
        if start_column >= end_column {
            source_offsets[index + 1] = rendered.len();
            source_offsets[index + 2..].fill(rendered.len());
            break;
        }

        let cell_start = start_column.saturating_sub(hscroll);
        let cell_end = next_column.min(end_column).saturating_sub(hscroll);
        let style = style_at(index);
        if start_column < hscroll || next_column > end_column {
            // A horizontal boundary can bisect a wide glyph. Preserve its
            // remaining cell geometry with blanks instead of shifting the
            // following text or drawing half a glyph outside the viewport.
            for cell in cell_start..cell_end {
                cell_offsets[cell] = rendered.len();
                cell_attrs[cell] = style;
                rendered.push(' ');
                char_attrs.push(style);
                cell_offsets[cell + 1] = rendered.len();
            }
        } else {
            for cell in cell_start..cell_end {
                cell_offsets[cell] = before;
                cell_attrs[cell] = style;
            }
            if display == '\t' {
                for cell in cell_start..cell_end {
                    rendered.push(' ');
                    char_attrs.push(style);
                    cell_offsets[cell + 1] = rendered.len();
                }
            } else {
                rendered.push(display);
                char_attrs.push(style);
            }
            cell_offsets[cell_end] = rendered.len();
        }
        visible_columns = visible_columns.max(cell_end);
        source_offsets[index + 1] = rendered.len();
        column = next_column;
    }

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
        cell_offsets,
        visible_columns,
        padding_char_start,
    }
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
    mark_active: u32,
    inherit: u32,
    /// `color-name-rgb-alist' folded into (canonical name -> rgb).
    colors: HashMap<EmacsText, (u8, u8, u8)>,
    /// Face family names interned for compact, copyable cell styles.
    font_families: Vec<String>,
    font_family_ids: HashMap<String, u16>,
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
            mark_active: sym("mark-active"),
            inherit: sym(":inherit"),
            colors,
            font_families: Vec::new(),
            font_family_ids: HashMap::new(),
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
                ":weight" => {
                    let weight = font_weight_value(i, v);
                    mix.font_weight = Some(weight);
                    mix.bold = Some(weight >= 600);
                }
                ":width" => mix.font_width = font_width_percent(i, v),
                ":height" => mix.font_height = font_height_tenths(v),
                ":slant" => mix.italic = Some(slant_italic(i, v)),
                ":underline" => mix.underline = Some(v.truthy()),
                ":overline" => mix.overline = Some(v.truthy()),
                ":strike-through" => mix.strike = Some(v.truthy()),
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
    /// Overlay snapshots and face values. Precedence is computed for the
    /// overlays covering each position, including interval containment.
    overlays: Vec<(crate::buffer::Overlay, Value)>,
    /// 'face/'font-lock-face' text-property runs: (start, end, value,
    /// insertion index).  Later-inserted props merge on top (GNU's
    /// interval "last write wins").
    prop_faces: Vec<(usize, usize, Value, usize)>,
    /// Active region as a half-open range when the mark is active.
    region: Option<(usize, usize)>,
}

fn window_faces(
    i: &Interp,
    ctx: &FaceCtx,
    b: &std::rc::Rc<std::cell::RefCell<Buffer>>,
    win: &Value,
) -> WindowFaces {
    let bb = b.borrow();
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
    let prop_faces: Vec<(usize, usize, Value, usize)> = bb
        .text
        .props
        .borrow()
        .iter()
        .enumerate()
        .filter(|(_, tp)| tp.prop == ctx.face || tp.prop == ctx.font_lock_face)
        .map(|(idx, tp)| (tp.start, tp.end, tp.value.clone(), idx))
        .collect();
    WindowFaces {
        overlays,
        prop_faces,
        region,
    }
}

/// The merged face style for buffer position POS under WF.  GNU
/// `compute_char_face' order: `default' seeds, the active `region'
/// face, then text-property faces (insertion order — last write
/// wins), then overlays in ascending priority.
fn style_at_pos(i: &Interp, ctx: &mut FaceCtx, wf: &WindowFaces, pos: usize) -> CellStyle {
    let mut mix = ctx.builtin(i, "default", 0);
    if let Some((rs, re)) = wf.region {
        if pos >= rs && pos < re {
            mix.merge(&ctx.builtin(i, "region", 0));
        }
    }
    for (s, e, v, _) in &wf.prop_faces {
        if pos >= *s && pos < *e {
            let v = v.clone();
            mix.merge(&ctx.face_value(i, &v, 0));
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
        mix.merge(&ctx.face_value(i, &face, 0));
    }
    mix.finish()
}

/// Compute the screen contents for the selected frame at `width` ×
/// `height` text cells.  Like GNU's `redisplay_window' it also keeps
/// each window's point on screen: a window's `start' is scrolled the
/// minimal amount to show point, and recentered when point sits more
/// than a window-height away (scrolling can't reach it cheaply).
pub fn render_grid(i: &Interp, width: usize, height: usize) -> Grid {
    render_grid_internal(i, width, height, false, None)
}

/// Render a GUI input grid, reusing TEXT from a previous snapshot when
/// the same buffer has not changed.
pub fn render_grid_with_input_cached(
    i: &Interp,
    width: usize,
    height: usize,
    cached: Option<&GridInput>,
) -> Grid {
    render_grid_internal(i, width, height, true, cached)
}

fn render_grid_internal(
    i: &Interp,
    width: usize,
    height: usize,
    include_input: bool,
    cached_input: Option<&GridInput>,
) -> Grid {
    let mut rows = vec![String::new(); height];
    let mut fctx = FaceCtx::new(i);
    let default_style = fctx.builtin(i, "default", 0).finish();
    let default_bg = default_style.bg;
    let mut attrs = vec![vec![default_style; width]; height];
    let mut char_attrs = vec![Vec::new(); height];
    let mut mode_rows = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let mut positioned_cursors = Vec::new();
    let mut input_screen_lefts = Vec::new();
    let mut cursor_index = None;
    let (blink_cursor, blink_interval) = blink_cursor_settings(i);
    let mut input = if include_input {
        input_snapshot(i, cached_input)
    } else {
        None
    };
    let frame = match &i.selected_frame {
        Some(f) => f.clone(),
        None => {
            return Grid {
                title: "Remacs".to_owned(),
                font_families: fctx.font_families,
                default_style,
                rows,
                attrs,
                char_attrs,
                mode_rows,
                cursor: cursor_pos,
                cursors: Vec::new(),
                cursor_index,
                blink_cursor,
                blink_interval,
                default_bg,
                input,
            };
        }
    };
    let fb = frame.borrow();
    let title = frame_title(i, &fb);
    let cursor_color = frame_cursor_color(i, &mut fctx, &fb);
    let mini_height = 1usize;
    let body_height = height.saturating_sub(mini_height);
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
    let mut selected_cursor: Option<(usize, usize, usize, usize)> = None;

    for w in &fb.windows {
        let (buf_id, start, hscroll, raw_left, raw_top, raw_width, raw_height) = {
            let wb = w.borrow();
            (
                wb.buffer, wb.start, wb.hscroll, wb.left, wb.top, wb.width, wb.height,
            )
        };
        let window_left = scale_edge(raw_left, root_left, scale_x, width);
        let window_top = scale_edge(raw_top, root_top, scale_y, body_height);
        let window_right = scale_edge(
            raw_left.saturating_add(raw_width),
            root_left,
            scale_x,
            width,
        );
        let window_bottom = scale_edge(
            raw_top.saturating_add(raw_height),
            root_top,
            scale_y,
            body_height,
        );
        let window_width = window_right.saturating_sub(window_left);
        let available_height = body_height.saturating_sub(window_top);
        let window_height = window_bottom
            .saturating_sub(window_top)
            .max(2)
            .min(available_height);
        if window_width == 0 || window_height == 0 {
            continue;
        }
        let selected_window = std::rc::Rc::ptr_eq(w, &fb.selected);
        let cursor_style = cursor_type_for_window(i, w, buf_id, selected_window);
        let stretch_cursor = buffer_setting(i, "x-stretch-cursor", buf_id).truthy();
        let tab_width = buffer_tab_width(i, buf_id);
        let text_rows = window_height.saturating_sub(1);
        // Keep point inside the window: scroll `start' the minimal
        // amount, or recenter when point is more than a window-height
        // away, and likewise `hscroll' horizontally (GNU
        // `redisplay_window' semantics — the window's start/hscroll
        // are redisplay state, so updating them here is correct).
        let (start, hscroll) = match i.buffers.get(buf_id) {
            Some(b) if text_rows > 0 => {
                let (suspend_hs, old_pt, min_hs) = {
                    let wb = w.borrow();
                    (wb.suspend_hscroll, wb.old_point_position(), wb.min_hscroll)
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
                wb_mut.record_old_point(pt);
                drop(wb_mut);
                let suspend_hs = suspend_hs && pt == old_pt;
                let pt_line = bb.text.line_of_pos(pt);
                let start_line = bb.text.line_of_pos(start);
                let new_start = if pt_line < start_line {
                    let up = start_line - pt_line;
                    if up > text_rows {
                        bb.text.line_start(pt_line.saturating_sub(text_rows / 2))
                    } else {
                        bb.text.line_start(pt_line)
                    }
                } else if pt_line >= start_line + text_rows {
                    let down = pt_line - (start_line + text_rows - 1);
                    if down > text_rows {
                        bb.text.line_start(pt_line.saturating_sub(text_rows / 2))
                    } else {
                        bb.text.line_start(pt_line + 1 - text_rows)
                    }
                } else {
                    start
                };
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
                let new_hscroll = if window_width > 0 && ahs_on && !suspend_hs {
                    let col = display_columns(
                        &bb.text.substring(bb.text.line_start(pt_line), pt),
                        tab_width,
                    );
                    if col < hscroll {
                        col
                    } else if col >= hscroll + window_width {
                        col + 1 - window_width
                    } else {
                        hscroll
                    }
                    .max(min_hs)
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
        // Per-window face context (overlays, text props, region).
        let wf = i
            .buffers
            .get(buf_id)
            .map(|b| window_faces(i, &fctx, &b, &Value::Window(w.clone())));
        let mut visible_end = start;
        for row in 0..text_rows {
            let y = window_top + row;
            if y >= body_height {
                break;
            }
            // Line-aligned rendering: `start' is a line start because
            // the scroll code above always lands it on line_start().
            let (line, line_pos, row_end, point_index) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let ls = bb.text.line_start(bb.text.line_of_pos(start) + row);
                    let le = bb.text.line_end(ls);
                    let row_end = if le < bb.text_len() && bb.text.char_at(le) == '\n' {
                        le + 1
                    } else {
                        le
                    };
                    let point = if selected_window {
                        bb.point()
                    } else {
                        w.borrow().point
                    }
                    .clamp(bb.begv, bb.zv);
                    let point_line = bb.text.line_start(bb.text.line_of_pos(point));
                    let point_index = (point_line == ls).then(|| point - ls);
                    (bb.text.substring(ls, le), ls, row_end, point_index)
                }
                None => (EmacsText::new(), 0, 0, None),
            };
            visible_end = row_end;
            let mut rendered = render_text_row(
                &line,
                hscroll,
                window_width,
                tab_width,
                default_style,
                |index| {
                    wf.as_ref()
                        .map(|wf| style_at_pos(i, &mut fctx, wf, line_pos + index))
                        .unwrap_or(default_style)
                },
            );
            // Empty tail of the row: the line-end face extends its
            // background over it iff the face's :extend holds (GNU
            // extend_face_to_end_of_line).
            if let Some(wf) = &wf {
                let end_pos = line_pos + line.chars().count();
                let tail = style_at_pos(i, &mut fctx, wf, end_pos);
                if tail.extend {
                    for cell in rendered.visible_columns..window_width {
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
            if let Some(point_index) = point_index
                && window_width > 0
            {
                let point_col = display_columns_of(line.chars().take(point_index), tab_width);
                let col = point_col.saturating_sub(hscroll).min(window_width - 1);
                let byte_index = if point_col < hscroll {
                    0
                } else if point_col.saturating_sub(hscroll) >= window_width {
                    rendered.cell_offsets[window_width - 1]
                } else {
                    rendered
                        .source_offsets
                        .get(point_index)
                        .copied()
                        .unwrap_or(rendered.text.len())
                };
                if selected_window {
                    selected_cursor = Some((y, window_left, col, byte_index));
                }
                if let Some((shape, thickness)) = cursor_style {
                    let stretch = line.chars().nth(point_index).and_then(|ch| {
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
                        (point_col >= hscroll
                            && point_col.saturating_sub(hscroll) < window_width
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
                        window_left,
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
                        buffer_start: line_pos,
                        source_offsets: rendered.source_offsets.clone(),
                    });
                    input_screen_lefts.push((row_index, window_left));
                }
            }
            place_rendered_row(&mut screen_rows[y], window_left, &rendered);
        }
        let tick = i
            .buffers
            .get(buf_id)
            .map(|b| b.borrow().text.mod_tick.get())
            .unwrap_or(0);
        w.borrow_mut().redisplay_end = Some((buf_id, tick, start, visible_end));
        // Mode line.
        let my = window_top + text_rows;
        if my < body_height {
            let (name, modified, point_line, point_col) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let point = if selected_window {
                        bb.point()
                    } else {
                        w.borrow().point
                    }
                    .clamp(bb.begv, bb.zv);
                    let point_line = bb.text.line_of_pos(point);
                    let point_line_start = bb.text.line_start(point_line);
                    (
                        bb.name.clone(),
                        bb.is_modified(),
                        point_line + 1,
                        display_columns(&bb.text.substring(point_line_start, point), tab_width),
                    )
                }
                None => ("???".into(), false, 0, 0),
            };
            let mode = format!(
                "--{}- {}   L{} C{}{}",
                if modified { "**" } else { "--" },
                name,
                point_line,
                point_col,
                if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    "  [sel]"
                } else {
                    ""
                }
            );
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
            let mut ml = fctx.builtin(i, ml_name, 0).finish();
            if ml == CellStyle::default() {
                ml.inverse = true;
            }
            let mode_row = fit_text_to_cells_with_tab_width(&mode, window_width, tab_width).0;
            let mode_cells = vec![ml; window_width];
            let mode_chars = mode_row.chars().map(|_| ml).collect::<Vec<_>>();
            place_text_row(
                &mut screen_rows[my],
                window_left,
                &mode_row,
                &mode_cells,
                &mode_chars,
            );
            mode_rows.push(my);
        }
    }
    // Minibuffer / echo area (last row).  The minibuffer's own text
    // properties apply (minibuffer-prompt's `face' prop colors the
    // prompt); a plain echo message renders under `default'.
    let mini_buf = fb
        .minibuffer
        .as_ref()
        .and_then(|w| i.buffers.get(w.borrow().buffer));
    let mini_text = match &mini_buf {
        Some(b) => b.borrow().text.text(),
        None => EmacsText::new(),
    };
    let (msg, styled) = if mini_text.is_empty() {
        (i.echo_message.clone(), None)
    } else {
        (mini_text, mini_buf.as_ref())
    };
    let tab_width = mini_buf
        .as_ref()
        .map(|buffer| buffer_tab_width(i, buffer.borrow().id))
        .unwrap_or(8);
    if let Some(y) = rows.len().checked_sub(1) {
        let wf = styled.map(|b| window_faces(i, &fctx, b, &Value::Nil));
        let rendered = render_text_row(&msg, 0, width, tab_width, default_style, |index| {
            wf.as_ref()
                .map(|wf| style_at_pos(i, &mut fctx, wf, index))
                .unwrap_or(default_style)
        });
        if let (Some(snapshot), Some(buffer)) = (input.as_mut(), mini_buf.as_ref())
            && snapshot.buffer_id == buffer.borrow().id
        {
            snapshot.rows.push(GridInputRow {
                row: y,
                buffer_start: 0,
                source_offsets: rendered.source_offsets.clone(),
            });
            input_screen_lefts.push((snapshot.rows.len() - 1, 0));
        }
        place_rendered_row(&mut screen_rows[y], 0, &rendered);
    }
    let (composed_rows, composed_attrs, composed_char_attrs, cell_offsets) =
        compose_screen_rows(&screen_rows, width, default_style);
    rows = composed_rows;
    attrs = composed_attrs;
    char_attrs = composed_char_attrs;
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
        attrs,
        char_attrs,
        mode_rows,
        cursor: cursor_pos,
        cursors,
        cursor_index,
        blink_cursor,
        blink_interval,
        default_bg,
        input,
    }
}
