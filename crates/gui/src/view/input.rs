//! OS text input for the command-loop front-end. The input client owns the
//! provisional composition; committed text belongs to the editor logic thread.

use std::ops::Range;

use super::{
    EditorView, FG, FONT_SIZE, GuiEvent, LINE_HEIGHT, grid_row_height, grid_row_top,
};
use gpui::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, font, point, px,
    rgb, size,
};
use remacs_runtime::frontend::{CellStyle, Grid, GridInput, MouseArea, MousePosition};

#[derive(Clone, Default)]
pub(super) struct TextInputState {
    pub(super) marked: String,
    selection: Range<usize>,
    /// Marked-text interval in the OS-visible UTF-16 document.
    marked_range: Option<Range<usize>>,
    /// Buffer interval replaced by the current marked text.
    pub(super) replacement_range: Option<Range<usize>>,
    composition_buffer_id: Option<usize>,
    discarded_replacement_range: Option<Range<usize>>,
}

fn offset(text: &str, requested: usize, round_up: bool) -> (usize, usize) {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        let end = units + ch.len_utf16();
        if requested < end {
            return if round_up && requested > units {
                (byte + ch.len_utf8(), end)
            } else {
                (byte, units)
            };
        }
        units = end;
    }
    (text.len(), units)
}

fn range(text: &str, requested: Range<usize>) -> (Range<usize>, Range<usize>) {
    let (start, start_units) = offset(text, requested.start, false);
    if requested.is_empty() {
        return (start..start, start_units..start_units);
    }
    let (end, end_units) = offset(text, requested.end.max(requested.start), true);
    (start..end, start_units..end_units)
}

fn selected_document_range(input: &GridInput) -> Range<usize> {
    input
        .selection
        .as_ref()
        .map(|selection| selection.range.clone())
        .unwrap_or(input.cursor..input.cursor)
}

fn grid_row_at_y(grid: &Grid, y: Pixels) -> usize {
    if grid.row_heights.is_empty() {
        return (f32::from(y) / LINE_HEIGHT).floor().max(0.0) as usize;
    }
    let y = f32::from(y).max(0.0);
    let mut top = 0.0;
    for row in 0..grid.row_heights.len() {
        let bottom = top + f32::from(grid_row_height(grid, row, px(LINE_HEIGHT))).max(1.0);
        if y < bottom {
            return row;
        }
        top = bottom;
    }
    grid.row_heights.len()
}

pub(super) fn visible_input_position(
    grid: &Grid,
    utf16_offset: usize,
    round_up: bool,
) -> Option<(usize, usize)> {
    let input = grid.input.as_ref()?;
    let char_offset = input.character_index_for_utf16_offset(utf16_offset, round_up);
    let mut position = None;
    for row in &input.rows {
        let Some(relative) = char_offset.checked_sub(row.buffer_start) else {
            continue;
        };
        let Some(byte) = row.source_offsets.get(relative).copied() else {
            continue;
        };
        let mapped = (row.row, byte);
        if round_up {
            return Some(mapped);
        }
        // Wrapped segment boundaries occur in both adjacent rows. A point
        // or range start belongs to the next row; range ends keep the prior
        // row's trailing boundary.
        position = Some(mapped);
    }
    position
}

fn cursor_moved_to_echo_area(grid: &Grid) -> bool {
    let Some((_, cursor_row)) = grid.cursor else {
        return false;
    };
    let echo_area_row = grid.rows.len().saturating_sub(1);
    if cursor_row != echo_area_row {
        return false;
    }
    let input_cursor = grid.input.as_ref().and_then(|input| {
        visible_input_position(grid, input.cursor, false)
    });
    input_cursor.is_some_and(|position| {
        position.0 != cursor_row
            || grid
                .cursor_index
                .is_some_and(|cursor_index| position.1 != cursor_index)
    })
}

pub(super) fn composition_origin(
    grid: &Grid,
    replacement_range: Option<&Range<usize>>,
) -> Option<(usize, usize)> {
    let replacement = replacement_range?;
    let source_origin = visible_input_position(grid, replacement.start, false)?;
    let (_, cursor_row) = grid.cursor?;
    if cursor_moved_to_echo_area(grid) {
        // Echo-area rendering moves the visual cursor but leaves GridInput's
        // document rows anchored to the source buffer.  Use the rendered
        // cursor position instead of the stale source-buffer position.
        grid.cursor_index.map(|index| (cursor_row, index))
    } else {
        Some(source_origin)
    }
}

fn image_map_area_at_point(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
) -> Option<remacs_runtime::lisp::value::SymId> {
    for image in &grid.images {
        if image.map.is_empty() {
            continue;
        }
        let (Some(width), Some(height)) = (
            image.width.or(image.intrinsic_width),
            image.height.or(image.intrinsic_height),
        ) else {
            continue;
        };
        let left = f32::from(cell_width) * image.column as f32;
        let top = f32::from(grid_row_top(grid, image.row, px(LINE_HEIGHT)))
            - super::vscroll_pixels_for_cell(grid, image.row, image.column) as f32;
        let local_x = f32::from(point.x) - left;
        let local_y = f32::from(point.y) - top;
        if local_x < 0.0
            || local_y < 0.0
            || local_x >= width as f32
            || local_y >= height as f32
        {
            continue;
        }
        let x = local_x.floor() as i128;
        let y = local_y.floor() as i128;
        if let Some(area) = image.map.iter().find(|area| area.contains(x, y)) {
            return Some(area.id);
        }
    }
    None
}

impl TextInputState {
    pub(super) fn accessibility_buffer_projection(
        &self,
        base: &str,
        fallback_selection: Range<usize>,
    ) -> (String, Range<usize>) {
        let selection = self
            .marked_range
            .as_ref()
            .filter(|_| !self.marked.is_empty())
            .map(|marked| {
                marked.start.saturating_add(self.selection.start)
                    ..marked.start.saturating_add(self.selection.end)
            })
            .unwrap_or(fallback_selection);
        (self.virtual_text(base), selection)
    }

    pub(super) fn accessibility_screen_projection(
        &self,
        grid: &Grid,
        fallback_selection: Range<usize>,
    ) -> (String, Range<usize>) {
        let mut rows = grid.rows.clone();
        let fallback_text = rows.join("\n");
        let Some(replacement) = self.replacement_range.as_ref() else {
            return (fallback_text, fallback_selection);
        };
        if self.marked.is_empty() {
            return (fallback_text, fallback_selection);
        }
        let Some((row, start)) = composition_origin(grid, Some(replacement)) else {
            return (fallback_text, fallback_selection);
        };
        let Some(line) = rows.get(row) else {
            return (fallback_text, fallback_selection);
        };
        if start > line.len() || !line.is_char_boundary(start) {
            return (fallback_text, fallback_selection);
        }

        let prefix_units = rows
            .iter()
            .take(row)
            .map(|line| line.encode_utf16().count() + 1)
            .sum::<usize>()
            + line[..start].encode_utf16().count();
        let mut end = start;
        if !cursor_moved_to_echo_area(grid)
            && let Some((end_row, end_index)) =
                visible_input_position(grid, replacement.end, true)
            && end_row == row
            && end_index >= start
            && end_index <= line.len()
            && line.is_char_boundary(end_index)
        {
            end = end_index;
        }
        rows[row].replace_range(start..end, &self.marked);

        let marked_len = self.marked.encode_utf16().count();
        let selection_start = self.selection.start.min(marked_len);
        let selection_end = self.selection.end.min(marked_len).max(selection_start);
        (
            rows.join("\n"),
            prefix_units.saturating_add(selection_start)
                ..prefix_units.saturating_add(selection_end),
        )
    }
}

pub(super) fn mouse_position_for_point_with_cell_width(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
    window: &mut Window,
) -> Option<MousePosition> {
    let input = grid.input.as_ref()?;
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let row = grid_row_at_y(grid, point.y);
    let vscroll = grid
        .vscrolls
        .iter()
        .find(|scroll| {
            row >= scroll.top
                && row < scroll.top.saturating_add(scroll.rows)
                && column >= scroll.left
                && column < scroll.left.saturating_add(scroll.width)
        })
        .map_or(0, |scroll| scroll.pixels);
    let row = grid_row_at_y(grid, point.y + px(vscroll as f32));
    let line = grid.rows.get(row)?;
    let byte = shape_grid_row(grid, row, window, line).closest_index_for_x(point.x);
    let mapped = input.hit_test_position(row, column, byte)?;
    let area = image_map_area_at_point(grid, point, cell_width).map(MouseArea::ImageMap);
    let (origin_x, origin_y) = mapped
        .window_id
        .and_then(|window_id| {
            grid.window_text_areas
                .iter()
                .find(|area| area.window_id == window_id)
        })
        .map(|area| {
            (
                px(f32::from(cell_width) * area.left as f32),
                grid_row_top(grid, area.top, px(LINE_HEIGHT)),
            )
        })
        .unwrap_or((px(0.0), px(0.0)));
    Some(MousePosition {
        buffer_id: mapped.buffer_id,
        buffer_position: mapped.buffer_position,
        utf16_offset: mapped.utf16_offset,
        x: super::device_pixels(point.x - origin_x, window.scale_factor()),
        y: super::device_pixels(point.y - origin_y, window.scale_factor()),
        row,
        column: mapped.column,
        window_id: mapped.window_id,
        area,
        mode_line_buffer_name: false,
    })
}

pub(super) fn mode_line_mouse_position(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
    scale_factor: f32,
) -> Option<MousePosition> {
    let row = grid_row_at_y(grid, point.y);
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let mode_line = grid.mode_lines.iter().find(|mode_line| {
        row == mode_line.row
            && column >= mode_line.left
            && column < mode_line.left.saturating_add(mode_line.width)
    })?;
    Some(MousePosition {
        buffer_id: mode_line.buffer_id,
        buffer_position: 0,
        utf16_offset: 0,
        x: super::device_pixels(point.x, scale_factor),
        y: super::device_pixels(point.y, scale_factor),
        row,
        column: column.saturating_sub(mode_line.left),
        window_id: Some(mode_line.window_id),
        area: Some(MouseArea::ModeLine),
        mode_line_buffer_name: mode_line
            .buffer_name_range
            .is_some_and(|(left, right)| column >= left && column < right),
    })
}

pub(super) fn header_line_mouse_position(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
    scale_factor: f32,
) -> Option<MousePosition> {
    let row = grid_row_at_y(grid, point.y);
    let column = (f32::from(point.x) / f32::from(cell_width))
        .floor()
        .max(0.0) as usize;
    let header = grid.header_lines.iter().find(|header| {
        row == header.row
            && column >= header.left
            && column < header.left.saturating_add(header.width)
    })?;
    Some(MousePosition {
        buffer_id: header.buffer_id,
        buffer_position: 0,
        utf16_offset: 0,
        x: super::device_pixels(point.x, scale_factor),
        y: super::device_pixels(point.y, scale_factor),
        row,
        column: column.saturating_sub(header.left),
        window_id: Some(header.window_id),
        area: Some(MouseArea::HeaderLine),
        mode_line_buffer_name: false,
    })
}

pub(super) fn text_window_at_point(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
) -> Option<usize> {
    let row = grid_row_at_y(grid, point.y);
    let column = (f32::from(point.x) / f32::from(cell_width))
        .floor()
        .max(0.0) as usize;
    grid.window_text_areas
        .iter()
        .find(|area| {
            row >= area.top
                && row < area.top.saturating_add(area.rows)
                && column >= area.left
                && column < area.left.saturating_add(area.width)
        })
        .map(|area| area.window_id)
}

pub(super) fn mouse_position_for_text_point_with_cell_width(
    grid: &Grid,
    point: Point<Pixels>,
    cell_width: Pixels,
    window: &mut Window,
) -> Option<MousePosition> {
    let input = grid.input.as_ref()?;
    let column = (f32::from(point.x) / f32::from(cell_width)).floor().max(0.0) as usize;
    let row = grid_row_at_y(grid, point.y);
    let vscroll = grid
        .vscrolls
        .iter()
        .find(|scroll| {
            row >= scroll.top
                && row < scroll.top.saturating_add(scroll.rows)
                && column >= scroll.left
                && column < scroll.left.saturating_add(scroll.width)
        })
        .map_or(0, |scroll| scroll.pixels);
    let row = grid_row_at_y(grid, point.y + px(vscroll as f32));
    let line = grid.rows.get(row)?;
    let byte = shape_grid_row(grid, row, window, line).closest_index_for_x(point.x);
    let (source_start, source_end) = input.hit_test_source_range(row, column)?;
    (source_start..=source_end)
        .contains(&byte)
        .then(|| mouse_position_for_point_with_cell_width(grid, point, cell_width, window))
        .flatten()
}

fn grid_base_font(grid: &Grid, window: &mut Window) -> (gpui::Font, Pixels) {
    let style = grid.default_style;
    let requested_family = style
        .font_family
        .and_then(|id| grid.font_families.get(id as usize))
        .cloned();
    let mut base_font = font(super::font_family(window, requested_family));
    if let Some(weight) = style.font_weight {
        base_font.weight = gpui::FontWeight(weight as f32);
    }
    base_font.stretch = gpui::FontStretch::from_percentage(style.font_width.unwrap_or(100));
    let font_size = style
        .font_height
        .map(super::face_font_size)
        .unwrap_or(px(FONT_SIZE));
    (base_font, font_size)
}

fn grid_font_for_style(
    grid: &Grid,
    window: &mut Window,
    base_font: &gpui::Font,
    style: CellStyle,
) -> (gpui::Font, Option<Pixels>) {
    let mut run_font = base_font.clone();
    if let Some(family) = style
        .font_family
        .and_then(|id| grid.font_families.get(id as usize))
    {
        run_font.family = super::font_family(window, Some(family.clone())).into();
    }
    if let Some(weight) = style.font_weight {
        run_font.weight = gpui::FontWeight(weight as f32);
    } else if style.bold {
        run_font = run_font.bold();
    }
    if let Some(width) = style.font_width {
        run_font.stretch = gpui::FontStretch::from_percentage(width);
    }
    if style.italic {
        run_font = run_font.italic();
    }
    (
        run_font,
        style.font_height.map(super::face_font_size),
    )
}

fn grid_style_at_char(grid: &Grid, row: usize, character: usize) -> CellStyle {
    grid.char_attrs
        .get(row)
        .filter(|cells| !cells.is_empty())
        .or_else(|| grid.attrs.get(row))
        .and_then(|cells| cells.get(character))
        .copied()
        .unwrap_or_default()
}

fn grid_style_at_byte(grid: &Grid, row: usize, text: &str, byte: usize) -> CellStyle {
    let prefix = text.get(..byte.min(text.len())).unwrap_or("");
    let character = prefix.chars().count().min(text.chars().count().saturating_sub(1));
    grid_style_at_char(grid, row, character)
}

fn grid_font_at_byte(
    grid: &Grid,
    row: usize,
    text: &str,
    byte: usize,
    window: &mut Window,
) -> (gpui::Font, Pixels) {
    let (base_font, base_size) = grid_base_font(grid, window);
    let (font, size) = grid_font_for_style(
        grid,
        window,
        &base_font,
        grid_style_at_byte(grid, row, text, byte),
    );
    (font, size.unwrap_or(base_size))
}

pub(super) fn shape_grid_row(
    grid: &Grid,
    row_index: usize,
    window: &mut Window,
    row: &str,
) -> gpui::ShapedLine {
    let (base_font, base_size) = grid_base_font(grid, window);
    let mut runs = Vec::<gpui::TextRun>::new();
    let mut font_sizes = Vec::new();
    let mut previous_style = None;
    for (character, (_, ch)) in row.char_indices().enumerate() {
        let style = grid_style_at_char(grid, row_index, character);
        if previous_style == Some(style) {
            if let Some(run) = runs.last_mut() {
                run.len += ch.len_utf8();
            }
        } else {
            let (run_font, run_size) = grid_font_for_style(grid, window, &base_font, style);
            runs.push(gpui::TextRun {
                len: ch.len_utf8(),
                font: run_font,
                color: rgb(FG).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            });
            font_sizes.push(run_size);
            previous_style = Some(style);
        }
    }
    let font_id = window.text_system().resolve_font(&base_font);
    let cell_width = window
        .text_system()
        .advance(font_id, base_size, '0')
        .map(|metrics| metrics.width)
        .unwrap_or(px(8.0));
    if runs.is_empty() {
        runs.push(gpui::TextRun {
            len: 0,
            font: base_font,
            color: rgb(FG).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
        font_sizes.push(None);
    }
    let line = window.text_system().shape_line_with_font_sizes(
        row.to_owned().into(),
        base_size,
        &runs,
        &font_sizes,
        None,
    );
    super::align_zwj_glyph_positions(line, row, cell_width, |byte_index| {
        byte_index
    })
}

impl TextInputState {
    fn len(&self) -> usize {
        self.marked.encode_utf16().count()
    }

    fn mark(
        &mut self,
        replacement: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
    ) {
        let (bytes, replaced) = range(&self.marked, replacement.unwrap_or(0..self.len()));
        self.marked.replace_range(bytes, text);
        let end = text.encode_utf16().count();
        let (_, selected) = range(text, selection.unwrap_or(end..end));
        self.selection = replaced.start + selected.start..replaced.start + selected.end;
    }

    pub(super) fn record_composition_buffer(&mut self, buffer_id: Option<usize>) {
        if self.composition_buffer_id.is_none() {
            self.composition_buffer_id = buffer_id;
        }
    }

    pub(super) fn composition_buffer_changed(&self, buffer_id: Option<usize>) -> bool {
        self.composition_buffer_id
            .is_some_and(|origin| buffer_id != Some(origin))
    }

    pub(super) fn discard_stale_composition(&mut self) {
        let discarded = self.replacement_range.clone();
        self.clear_active();
        self.discarded_replacement_range = discarded;
    }

    pub(super) fn is_discarded_replacement(&self, range: Option<&Range<usize>>) -> bool {
        self.discarded_replacement_range
            .as_ref()
            .is_some_and(|discarded| range == Some(discarded))
    }

    pub(super) fn clear_discarded_replacement(&mut self) {
        self.discarded_replacement_range = None;
    }

    fn clear_active(&mut self) -> String {
        self.selection = 0..0;
        self.marked_range = None;
        self.replacement_range = None;
        self.composition_buffer_id = None;
        std::mem::take(&mut self.marked)
    }

    fn take(&mut self) -> String {
        self.clear_active()
    }

    #[cfg(test)]
    fn commit(&mut self, replacement: Option<Range<usize>>, text: &str) -> String {
        self.mark(replacement, text, None);
        self.take()
    }

    #[cfg(test)]
    fn text(&self, requested: Range<usize>) -> Option<(String, Range<usize>)> {
        if requested.start > self.len() || requested.start > requested.end {
            return None;
        }
        let (bytes, actual) = range(&self.marked, requested);
        Some((self.marked[bytes].to_owned(), actual))
    }

    pub(super) fn selection_byte_end(&self) -> usize {
        offset(&self.marked, self.selection.end, false).0
    }

    pub(super) fn selection_byte_range(&self) -> Range<usize> {
        range(&self.marked, self.selection.clone()).0
    }

    fn virtual_text(&self, base: &str) -> String {
        let Some(replacement) = &self.replacement_range else {
            return base.to_owned();
        };
        let (bytes, _) = range(base, replacement.clone());
        let mut text = base.to_owned();
        text.replace_range(bytes, &self.marked);
        text
    }

    fn virtual_position_for_base(&self, position: usize) -> usize {
        if self.marked.is_empty() {
            return position;
        }
        let (Some(marked), Some(replacement)) =
            (&self.marked_range, &self.replacement_range)
        else {
            return position;
        };
        if position <= replacement.start {
            position
        } else if position >= replacement.end {
            position
                .saturating_sub(replacement.end)
                .saturating_add(marked.end)
        } else {
            marked
                .start
                .saturating_add(position.saturating_sub(replacement.start))
                .min(marked.end)
        }
    }

    fn virtual_position_for_buffer(&self, buffer_id: usize, position: usize) -> usize {
        if self.composition_buffer_id == Some(buffer_id) {
            self.virtual_position_for_base(position)
        } else {
            position
        }
    }

    fn base_range_for_virtual(&self, requested: Range<usize>) -> Range<usize> {
        let (Some(marked), Some(replacement)) = (&self.marked_range, &self.replacement_range)
        else {
            return requested;
        };
        if requested == *marked {
            return replacement.clone();
        }
        if requested.is_empty() {
            if requested.start <= marked.start {
                return requested;
            }
            if requested.start >= marked.end {
                let offset = requested
                    .start
                    .saturating_sub(marked.end)
                    .saturating_add(replacement.end);
                return offset..offset;
            }
            return replacement.clone();
        }
        if requested.start < marked.end && requested.end > marked.start {
            let start = if requested.start < marked.start {
                requested.start
            } else {
                replacement.start
            };
            let end = if requested.end > marked.end {
                replacement.end.saturating_add(requested.end - marked.end)
            } else {
                replacement.end
            };
            return start..end;
        }
        let map = |offset: usize| {
            if offset <= marked.start {
                offset
            } else {
                offset
                    .saturating_sub(marked.end - marked.start)
                    .saturating_add(replacement.end - replacement.start)
            }
        };
        map(requested.start)..map(requested.end)
    }

    fn commit_partial_mark(
        &mut self,
        requested: Range<usize>,
        text: &str,
    ) -> Option<(Range<usize>, String)> {
        let (Some(marked), Some(replacement)) =
            (self.marked_range.clone(), self.replacement_range.clone())
        else {
            return None;
        };
        if requested.start < marked.start || requested.end > marked.end {
            return None;
        }
        let local = requested.start - marked.start..requested.end - marked.start;
        let (bytes, actual) = range(&self.marked, local);
        let marked_len = self.len();
        if actual.is_empty() || (actual.start == 0 && actual.end == marked_len) {
            return None;
        }

        let prefix = self.marked[..bytes.start].to_owned();
        let suffix = self.marked[bytes.end..].to_owned();
        let (remaining, position, committed, selection) = if suffix.is_empty() {
            let remaining_len = prefix.encode_utf16().count();
            let selection = self.selection.start.min(remaining_len)
                ..self.selection.end.min(remaining_len);
            (prefix, replacement.start, text.to_owned(), selection)
        } else {
            // Keep the uncommitted suffix marked.  When the requested
            // segment is in the middle, commit its preceding text with it;
            // a text input client can represent only one marked range.
            let committed = if prefix.is_empty() {
                text.to_owned()
            } else {
                prefix + text
            };
            let remaining_len = suffix.encode_utf16().count();
            let selection = self.selection.start.saturating_sub(actual.end).min(remaining_len)
                ..self.selection.end.saturating_sub(actual.end).min(remaining_len);
            let position = replacement
                .start
                .saturating_add(committed.encode_utf16().count());
            (suffix, position, committed, selection)
        };

        let remaining_len = remaining.encode_utf16().count();
        self.marked = remaining;
        self.selection = selection;
        self.replacement_range = Some(position..position);
        self.marked_range = Some(position..position.saturating_add(remaining_len));
        Some((replacement, committed))
    }

    fn mark_document(
        &mut self,
        replacement: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        fallback: Range<usize>,
    ) {
        let requested =
            replacement.unwrap_or_else(|| self.marked_range.clone().unwrap_or(fallback));
        if let Some(marked) = self.marked_range.clone()
            && requested.start >= marked.start
            && requested.end <= marked.end
        {
            let local = requested.start - marked.start..requested.end - marked.start;
            self.mark(Some(local), text, selection);
            self.marked_range = Some(marked.start..marked.start + self.len());
        } else {
            let base_range = self.base_range_for_virtual(requested);
            self.mark(None, text, selection);
            self.replacement_range = Some(base_range.clone());
            self.marked_range = Some(base_range.start..base_range.start + self.len());
        }
    }
}

impl EditorView {
    fn send_replace_text(&self, range: Range<usize>, text: String) {
        if !range.is_empty() || !text.is_empty() {
            let plain_insert = self.grid.input.as_ref().is_none_or(|input| {
                self.input.marked.is_empty()
                    && input.selection.is_none()
                    && range == (input.cursor..input.cursor)
            });
            if plain_insert {
                let _ = self.tx.send(GuiEvent::Text(text));
            } else {
                let _ = self.tx.send(GuiEvent::ReplaceText { range, text });
            }
        }
    }
}

impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        requested: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let base = self
            .grid
            .input
            .as_ref()
            .map(|input| input.text.as_ref())
            .unwrap_or("");
        let text = self.input.virtual_text(base);
        if requested.start > requested.end || requested.start > text.encode_utf16().count() {
            return None;
        }
        let (bytes, actual) = range(&text, requested);
        *adjusted = Some(actual);
        Some(text[bytes].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if let Some(marked) = self.input.marked_range.as_ref()
            && !self.input.marked.is_empty()
        {
            return Some(UTF16Selection {
                range: marked.start + self.input.selection.start
                    ..marked.start + self.input.selection.end,
                reversed: false,
            });
        }
        if let Some(input) = self.grid.input.as_ref() {
            if let Some(selection) = &input.selection {
                return Some(UTF16Selection {
                    range: selection.range.clone(),
                    reversed: selection.reversed,
                });
            }
            return Some(UTF16Selection {
                range: input.cursor..input.cursor,
                reversed: false,
            });
        }
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        (!self.input.marked.is_empty())
            .then(|| self.input.marked_range.clone())
            .flatten()
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.input.marked.is_empty() {
            let range = self
                .input
                .replacement_range
                .clone()
                .or_else(|| self.grid.input.as_ref().map(selected_document_range));
            if let Some(range) = range {
                self.send_replace_text(range, self.input.marked.clone());
            }
        }
        self.input.take();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        replacement: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.is_discarded_replacement(replacement.as_ref()) {
            self.input.take();
            self.input.clear_discarded_replacement();
            cx.notify();
            return;
        }
        // Commit a leading or trailing segment while retaining the rest as
        // marked text.  The base document still needs the original replaced
        // range, but the surviving mark starts next to the committed text.
        if let Some(requested) = replacement.clone() {
            if let Some((range, committed)) = self.input.commit_partial_mark(requested, text) {
                self.send_replace_text(range, committed);
                cx.notify();
                return;
            }
        }
        let fallback = self.grid.input.as_ref().map(selected_document_range);
        let range = replacement
            .map(|range| self.input.base_range_for_virtual(range))
            .or_else(|| self.input.replacement_range.clone())
            .or(fallback)
            .unwrap_or(0..0);
        self.send_replace_text(range, text.to_owned());
        self.input.take();
        self.input.clear_discarded_replacement();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        replacement: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.is_discarded_replacement(replacement.as_ref()) {
            return;
        }
        self.input.clear_discarded_replacement();
        self.input.record_composition_buffer(
            self.grid.input.as_ref().map(|input| input.buffer_id),
        );
        let fallback = self.grid.input.as_ref().map(selected_document_range);
        self.input
            .mark_document(replacement, text, selected, fallback.unwrap_or(0..0));
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        requested: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let Some(input) = self.grid.input.as_ref() else {
            let (column, row) = self.grid.cursor?;
            let row_text = self.grid.rows.get(row)?;
            let cursor_index = self.grid.cursor_index;
            let base_x = self
                .grid
                .cursor_index
                .and_then(|index| {
                    Some(shape_grid_row(&self.grid, row, window, row_text).x_for_index(index))
                })
                .unwrap_or(self.cell_width * column as f32);
            let (composition_font, composition_font_size) = grid_font_at_byte(
                &self.grid,
                row,
                row_text,
                cursor_index.unwrap_or(0),
                window,
            );
            let mark_start = self
                .input
                .marked_range
                .as_ref()
                .map_or(0, |range| range.start);
            let start_byte = offset(
                &self.input.marked,
                requested.start.saturating_sub(mark_start),
                false,
            )
            .0;
            let end_byte = offset(
                &self.input.marked,
                requested.end.saturating_sub(mark_start),
                true,
            )
            .0;
            let composition_line = window.text_system().shape_line(
                self.input.marked.clone().into(),
                composition_font_size,
                &[gpui::TextRun {
                    len: self.input.marked.len(),
                    font: composition_font,
                    color: rgb(FG).into(),
                    background_color: None,
                    underline: Some(gpui::UnderlineStyle {
                        thickness: px(1.0),
                        color: None,
                        wavy: false,
                    }),
                    strikethrough: None,
                }],
                None,
            );
            let start = composition_line.x_for_index(start_byte);
            let end = composition_line.x_for_index(end_byte);
            let (left, text_width) = if end >= start {
                (start, end - start)
            } else {
                (end, start - end)
            };
            let column = (f32::from(base_x) / f32::from(self.cell_width)) as usize;
            return Some(Bounds {
                origin: point(
                    bounds.left() + base_x + left,
                    bounds.top() + grid_row_top(&self.grid, row, px(LINE_HEIGHT))
                        - px(super::vscroll_pixels_for_cell(&self.grid, row, column) as f32),
                ),
                size: size(
                    if requested.is_empty() {
                        px(0.0)
                    } else {
                        text_width.max(self.cell_width)
                    },
                    grid_row_height(&self.grid, row, px(LINE_HEIGHT)),
                ),
            });
        };
        let visible_text = self.input.virtual_text(&input.text);
        let visible_len = visible_text.encode_utf16().count();
        if requested.start > requested.end || requested.start > visible_len {
            return None;
        }

        if let (Some(marked), Some(replacement)) =
            (&self.input.marked_range, &self.input.replacement_range)
            && requested.start >= marked.start
            && requested.end <= marked.end
            && !self.input.marked.is_empty()
        {
            let (row, base_byte) = visible_input_position(&self.grid, replacement.start, false)?;
            let row_text = self.grid.rows.get(row)?;
            let (composition_font, composition_font_size) =
                grid_font_at_byte(&self.grid, row, row_text, base_byte, window);
            let local = requested.start - marked.start..requested.end - marked.start;
            let (bytes, _) = range(&self.input.marked, local);
            let prefix_byte = offset(
                &self.input.marked,
                requested.start.saturating_sub(marked.start),
                false,
            )
            .0;
            let composition = &self.input.marked;
            let composition_line = window.text_system().shape_line(
                composition.clone().into(),
                composition_font_size,
                &[gpui::TextRun {
                    len: composition.len(),
                    font: composition_font,
                    color: rgb(FG).into(),
                    background_color: None,
                    underline: Some(gpui::UnderlineStyle {
                        thickness: px(1.0),
                        color: None,
                        wavy: false,
                    }),
                    strikethrough: None,
                }],
                None,
            );
            let end_byte = bytes.end;
            let start = composition_line.x_for_index(prefix_byte);
            let end = composition_line.x_for_index(end_byte);
            let (left, text_width) = if end >= start {
                (start, end - start)
            } else {
                (end, start - end)
            };
            let base_line = shape_grid_row(&self.grid, row, window, row_text);
            let base_x = base_line.x_for_index(base_byte);
            let column = (f32::from(base_x) / f32::from(self.cell_width)) as usize;
            return Some(Bounds {
                origin: point(
                    bounds.left() + base_x + left,
                    bounds.top() + grid_row_top(&self.grid, row, px(LINE_HEIGHT))
                        - px(super::vscroll_pixels_for_cell(&self.grid, row, column) as f32),
                ),
                size: size(
                    if requested.is_empty() {
                        px(0.0)
                    } else {
                        text_width.max(self.cell_width)
                    },
                    grid_row_height(&self.grid, row, px(LINE_HEIGHT)),
                ),
            });
        }

        let base_range = self.input.base_range_for_virtual(requested.clone());
        let (row, start_byte) = visible_input_position(&self.grid, base_range.start, false)?;
        let end_position = visible_input_position(&self.grid, base_range.end, true);
        let row_text = self.grid.rows.get(row)?;
        let layout = shape_grid_row(&self.grid, row, window, row_text);
        let start = layout.x_for_index(start_byte);
        let end = match end_position {
            Some((end_row, end_byte)) if end_row == row => layout.x_for_index(end_byte),
            _ => input
                .rows
                .iter()
                .find(|input_row| input_row.row == row)
                .and_then(|input_row| input_row.source_offsets.last().copied())
                .map(|byte| layout.x_for_index(byte))
                .unwrap_or(layout.width),
        };
        let (left, text_width) = if end >= start {
            (start, end - start)
        } else {
            (end, start - end)
        };
        let column = (f32::from(start) / f32::from(self.cell_width)) as usize;
        Some(Bounds {
            origin: point(
                bounds.left() + left,
                bounds.top() + grid_row_top(&self.grid, row, px(LINE_HEIGHT))
                    - px(super::vscroll_pixels_for_cell(&self.grid, row, column) as f32),
            ),
            size: size(
                if requested.is_empty() {
                    px(0.0)
                } else {
                    text_width.max(self.cell_width)
                },
                grid_row_height(&self.grid, row, px(LINE_HEIGHT)),
            ),
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let point = super::frame_content_point(
            point,
            window.viewport_size(),
            px(self.internal_border_width as f32),
        )?;
        mouse_position_for_text_point_with_cell_width(&self.grid, point, self.cell_width, window)
            .map(|position| {
                self.input
                    .virtual_position_for_buffer(position.buffer_id, position.utf16_offset)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_uses_utf16_without_splitting_surrogates() {
        let mut input = TextInputState::default();
        input.mark(None, "A😀日", Some(1..3));
        assert_eq!(input.len(), 4);
        assert_eq!(input.selection, 1..3);
        assert_eq!(input.text(2..3), Some(("😀".into(), 1..3)));
        input.mark(Some(1..3), "本", Some(1..1));
        assert_eq!(input.marked, "A本日");
        assert_eq!(input.selection, 2..2);
        assert_eq!(input.commit(None, "日本語"), "日本語");
        assert!(input.marked.is_empty());
        assert_eq!(input.selection, 0..0);
    }

    #[test]
    fn point_hit_testing_maps_base_offsets_into_the_virtual_document() {
        let mut input = TextInputState::default();
        input.record_composition_buffer(Some(17));
        input.mark_document(Some(3..4), "かな", Some(1..2), 3..4);

        assert_eq!(input.virtual_position_for_buffer(17, 3), 3);
        assert_eq!(input.virtual_position_for_buffer(17, 6), 7);
        assert_eq!(input.virtual_position_for_buffer(18, 6), 6);
    }

    #[test]
    fn partial_commit_and_unmark_retain_composition_once() {
        let mut input = TextInputState::default();
        input.mark(None, "日本", None);
        assert_eq!(input.commit(Some(1..2), "語"), "日語");
        assert!(input.take().is_empty());
        input.mark(None, "かな", None);
        assert_eq!(input.take(), "かな");
        assert!(input.take().is_empty());
        assert_eq!(input.commit(None, "A日B😀e\u{301}"), "A日B😀e\u{301}");
    }
    #[test]
    fn buffer_switch_discards_stale_replacement_range() {
        let mut input = TextInputState::default();
        let stale_range = 2..5;
        input.record_composition_buffer(Some(1));
        input.replacement_range = Some(stale_range.clone());
        input.marked_range = Some(2..4);
        input.marked = "かな".to_owned();

        assert!(!input.composition_buffer_changed(Some(1)));
        assert!(input.composition_buffer_changed(Some(2)));
        input.discard_stale_composition();

        assert!(input.marked.is_empty());
        assert!(input.marked_range.is_none());
        assert!(input.replacement_range.is_none());
        assert!(input.is_discarded_replacement(Some(&stale_range)));
    }

    #[test]
    fn echo_area_composition_uses_the_visible_cursor() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(echo_area_composition_uses_the_visible_cursor_inner)
            .expect("spawn IME composition test thread")
            .join()
            .expect("IME composition test thread");
    }

    fn echo_area_composition_uses_the_visible_cursor_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str("(erase-buffer) (insert \"abcdef\") (goto-char 4)")
            .unwrap();
        let mut grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        let replacement = 3..3;

        grid.input.as_mut().unwrap().cursor = replacement.start;
        let source_origin = visible_input_position(&grid, replacement.start, false).unwrap();
        grid.rows.extend([String::new(), String::new()]);
        let echo_row = grid.rows.len() - 1;
        let echo_index = source_origin.1 + 1;
        grid.cursor = Some((echo_index, echo_row));
        grid.cursor_index = Some(echo_index);
        assert_eq!(
            composition_origin(&grid, Some(&replacement)),
            Some((echo_row, echo_index))
        );
    }

    #[test]
    fn accessibility_projections_include_marked_text_and_selection() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(accessibility_projections_include_marked_text_and_selection_inner)
            .expect("spawn accessibility composition test thread")
            .join()
            .expect("accessibility composition test thread");
    }

    fn accessibility_projections_include_marked_text_and_selection_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str("(erase-buffer) (insert \"abcdef\") (goto-char 4)")
            .unwrap();
        let mut grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        let replacement = 3..4;
        grid.input.as_mut().unwrap().cursor = replacement.start;
        let (row, start) = visible_input_position(&grid, replacement.start, false).unwrap();
        let (_, end) = visible_input_position(&grid, replacement.end, true).unwrap();
        grid.cursor = Some((start, row));
        grid.cursor_index = Some(start);

        let mut input = TextInputState::default();
        input.mark_document(
            Some(replacement.clone()),
            "かな",
            Some(1..2),
            replacement.clone(),
        );
        let base = grid.input.as_ref().unwrap().text.as_ref();
        let (buffer_text, buffer_selection) =
            input.accessibility_buffer_projection(base, 0..0);
        assert_eq!(buffer_text, "abcかなef");
        assert_eq!(buffer_selection, 4..5);

        let mut expected_rows = grid.rows.clone();
        expected_rows[row].replace_range(start..end, "かな");
        let prefix_units = grid.rows[..row]
            .iter()
            .map(|line| line.encode_utf16().count() + 1)
            .sum::<usize>()
            + grid.rows[row][..start].encode_utf16().count();
        let (screen_text, screen_selection) = input.accessibility_screen_projection(&grid, 0..0);
        assert_eq!(screen_text, expected_rows.join("\n"));
        assert_eq!(screen_selection, prefix_units + 1..prefix_units + 2);

        let mut echo_grid = grid.clone();
        echo_grid.rows.push("I-search: ".to_owned());
        let echo_row = echo_grid.rows.len() - 1;
        let echo_index = echo_grid.rows[echo_row].len();
        echo_grid.cursor = Some((echo_index, echo_row));
        echo_grid.cursor_index = Some(echo_index);
        let mut expected_echo_rows = echo_grid.rows.clone();
        expected_echo_rows[echo_row].insert_str(echo_index, "かな");
        let echo_prefix_units = echo_grid.rows[..echo_row]
            .iter()
            .map(|line| line.encode_utf16().count() + 1)
            .sum::<usize>()
            + echo_grid.rows[echo_row].encode_utf16().count();
        let (screen_text, screen_selection) =
            input.accessibility_screen_projection(&echo_grid, 0..0);
        assert_eq!(screen_text, expected_echo_rows.join("\n"));
        assert_eq!(screen_selection, echo_prefix_units + 1..echo_prefix_units + 2);
    }

    #[test]
    fn wrapped_boundary_maps_points_to_the_continuation_row() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(wrapped_boundary_maps_points_to_the_continuation_row_inner)
            .expect("spawn wrapped input position test thread")
            .join()
            .expect("wrapped input position test thread");
    }

    fn wrapped_boundary_maps_points_to_the_continuation_row_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp.eval_str("(erase-buffer) (insert \"abcdefghij\")").unwrap();
        let mut grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        let character = {
            let input = grid.input.as_mut().unwrap();
            input.cursor = 4;
            input.character_index_for_utf16_offset(input.cursor, false)
        };
        grid.input.as_mut().unwrap().rows = vec![
            remacs_runtime::frontend::GridInputRow {
                row: 0,
                buffer_start: character - 4,
                source_offsets: vec![0, 1, 2, 3, 4],
            },
            remacs_runtime::frontend::GridInputRow {
                row: 1,
                buffer_start: character,
                source_offsets: vec![0],
            },
        ];
        grid.rows = vec!["abcd".to_owned(), "efgh".to_owned()];
        grid.cursor = Some((0, 1));
        grid.cursor_index = Some(0);
        let input = grid.input.as_ref().unwrap();
        let candidates = input
            .rows
            .iter()
            .filter_map(|row| {
                let relative = character.checked_sub(row.buffer_start)?;
                Some((row.row, *row.source_offsets.get(relative)?))
            })
            .collect::<Vec<_>>();
        assert!(candidates.len() >= 2, "boundary mappings: {candidates:?}");

        assert_eq!(
            visible_input_position(&grid, input.cursor, true),
            Some((0, 4))
        );
        assert_eq!(
            visible_input_position(&grid, input.cursor, false),
            Some((1, 0))
        );
        assert_eq!(
            visible_input_position(&grid, input.cursor, false),
            grid.cursor.map(|(_, row)| (row, grid.cursor_index.unwrap_or(0)))
        );
    }

    #[test]
    fn image_map_hotspots_are_hit_tested_in_image_pixels() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(image_map_hotspots_are_hit_tested_in_image_pixels_inner)
            .expect("spawn image map test thread")
            .join()
            .expect("image map test thread");
    }

    fn image_map_hotspots_are_hit_tested_in_image_pixels_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        let form = r#"(progn
                  (erase-buffer)
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image
                           :type 'svg
                           :data "<svg xmlns='http://www.w3.org/2000/svg' width='40' height='20'><rect width='40' height='20' fill='red'/></svg>"
                           :map '(((rect . ((0 . 0) . (20 . 20))) area1 nil))))))"#;
        if let Err(flow) = interp.eval_str(form) {
            let detail = match &flow {
                remacs_runtime::lisp::error::Flow::Signal(signal) => format!(
                    "{} {}",
                    interp.prin1_to_string(&signal.symbol()).to_unicode_lossy(),
                    interp.prin1_to_string(&signal.data()).to_unicode_lossy()
                ),
                _ => format!("{flow:?}"),
            };
            panic!("image descriptor evaluation failed: {detail}");
        }
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        let image = grid.images.first().expect("rendered SVG image");
        assert_eq!(image.map.len(), 1);
        let area1 = interp.intern("area1");
        let x = px(10.0) + px(8.0 * image.column as f32);
        let y = px(10.0) + grid_row_top(&grid, image.row, px(LINE_HEIGHT));
        assert_eq!(image_map_area_at_point(&grid, point(x, y), px(8.0)), Some(area1));
        assert_eq!(
            image_map_area_at_point(&grid, point(x + px(25.0), y), px(8.0)),
            None
        );
    }

    #[test]
    fn image_descriptor_frame_index_reaches_the_grid() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(image_descriptor_frame_index_reaches_the_grid_inner)
            .expect("spawn image frame index test thread")
            .join()
            .expect("image frame index test thread");
    }

    fn image_descriptor_frame_index_reaches_the_grid_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str(
                r#"(progn
                  (erase-buffer)
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'gif :data "x" :index 1))))"#,
            )
            .unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(grid.images.first().expect("rendered image").index, Some(1));
    }

    #[test]
    fn image_descriptor_conversion_reaches_the_grid() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(image_descriptor_conversion_reaches_the_grid_inner)
            .expect("spawn image conversion test thread")
            .join()
            .expect("image conversion test thread");
    }

    fn image_descriptor_conversion_reaches_the_grid_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str(
                r#"(progn
                  (erase-buffer)
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x" :conversion 'laplace)))
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x" :conversion 'emboss)))
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x"
                           :conversion
                           '(edge-detection :matrix [1 2 3 4 5 6 7 8 9]
                                            :color-adjust 12))))
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x" :conversion 'disabled))))"#,
            )
            .unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(grid.images.len(), 4);
        assert_eq!(
            grid.images[0].conversion,
            Some(remacs_runtime::frontend::GridImageConversion::EdgeDetection {
                matrix: [1, 0, 0, 0, 0, 0, 0, 0, -1],
                color_adjust: 45_000,
            })
        );
        assert_eq!(
            grid.images[1].conversion,
            Some(remacs_runtime::frontend::GridImageConversion::EdgeDetection {
                matrix: [2, -1, 0, -1, 0, 1, 0, 1, -2],
                color_adjust: 32_767,
            })
        );
        assert_eq!(
            grid.images[2].conversion,
            Some(remacs_runtime::frontend::GridImageConversion::EdgeDetection {
                matrix: [1, 2, 3, 4, 5, 6, 7, 8, 9],
                color_adjust: 12,
            })
        );
        assert_eq!(
            grid.images[3].conversion,
            Some(remacs_runtime::frontend::GridImageConversion::Disabled)
        );
    }

    #[test]
    fn image_transform_smoothing_reaches_the_grid() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(image_transform_smoothing_reaches_the_grid_inner)
            .expect("spawn image smoothing test thread")
            .join()
            .expect("image smoothing test thread");
    }

    fn image_transform_smoothing_reaches_the_grid_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str(
                r#"(progn
                  (erase-buffer)
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x")))
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x"
                           :transform-smoothing nil)))
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x"
                           :transform-smoothing t))))"#,
            )
            .unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(grid.images.len(), 3);
        assert_eq!(grid.images[0].transform_smoothing, None);
        assert_eq!(grid.images[1].transform_smoothing, Some(false));
        assert_eq!(grid.images[2].transform_smoothing, Some(true));
    }

    #[test]
    fn max_image_size_reaches_the_grid() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(max_image_size_reaches_the_grid_inner)
            .expect("spawn image size limit test thread")
            .join()
            .expect("image size limit test thread");
    }

    fn max_image_size_reaches_the_grid_inner() {
        let mut interp = remacs_runtime::lisp::Interp::new();
        interp
            .eval_str(
                r#"(progn
                  (setq max-image-size 64)
                  (erase-buffer)
                  (let ((start (point)))
                    (insert " ")
                    (put-text-property
                     start (1+ start) 'display
                     (list 'image :type 'png :data "x"))))"#,
            )
            .unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(
            grid.images[0].max_image_size,
            Some(remacs_runtime::frontend::GridImageSizeLimit::Pixels(64))
        );

        interp.eval_str("(setq max-image-size 0.5)").unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(
            grid.images[0].max_image_size,
            Some(remacs_runtime::frontend::GridImageSizeLimit::FrameRatioBits(0.5f64.to_bits()))
        );

        interp.eval_str("(setq max-image-size nil)").unwrap();
        let grid = remacs_runtime::frontend::render_grid_with_input_cached(
            &interp, 40, 4, None,
        );
        assert_eq!(grid.images[0].max_image_size, None);
    }

}
