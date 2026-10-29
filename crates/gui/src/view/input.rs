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
use remacs_runtime::frontend::{Grid, GridInput, MousePosition};

#[derive(Clone, Default)]
pub(super) struct TextInputState {
    pub(super) marked: String,
    selection: Range<usize>,
    /// Marked-text interval in the OS-visible UTF-16 document.
    marked_range: Option<Range<usize>>,
    /// Buffer interval replaced by the current marked text.
    pub(super) replacement_range: Option<Range<usize>>,
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
    let (byte, _) = offset(&input.text, utf16_offset, round_up);
    let char_offset = input.text[..byte].chars().count();
    input.rows.iter().find_map(|row| {
        let relative = char_offset.checked_sub(row.buffer_start)?;
        row.source_offsets
            .get(relative)
            .copied()
            .map(|byte| (row.row, byte))
    })
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
    let input_row = input.rows.iter().find(|input_row| input_row.row == row)?;
    let line = grid.rows.get(row)?;
    let byte = shape_grid_row(window, line).closest_index_for_x(point.x);
    let (character, _) = input_row
        .source_offsets
        .iter()
        .enumerate()
        .min_by_key(|(_, source_offset)| source_offset.abs_diff(byte))?;
    let buffer_position = input_row
        .buffer_start
        .saturating_add(character)
        .min(input.text.chars().count());
    let utf16_offset = input
        .text
        .chars()
        .take(buffer_position)
        .map(char::len_utf16)
        .sum();
    Some(MousePosition {
        buffer_id: input.buffer_id,
        buffer_position,
        utf16_offset,
        x: f32::from(point.x).round() as i128,
        y: f32::from(point.y).round() as i128,
        row,
        column: character,
    })
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
    let input_row = input.rows.iter().find(|input_row| input_row.row == row)?;
    let line = grid.rows.get(row)?;
    let byte = shape_grid_row(window, line).closest_index_for_x(point.x);
    let source_end = input_row.source_offsets.last()?;
    (byte <= *source_end)
        .then(|| mouse_position_for_point_with_cell_width(grid, point, cell_width, window))
        .flatten()
}

fn shape_grid_row(window: &Window, row: &str) -> gpui::ShapedLine {
    window.text_system().shape_line(
        row.to_owned().into(),
        px(FONT_SIZE),
        &[gpui::TextRun {
            len: row.len(),
            font: font("Menlo"),
            color: rgb(FG).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    )
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

    fn take(&mut self) -> String {
        self.selection = 0..0;
        self.marked_range = None;
        self.replacement_range = None;
        std::mem::take(&mut self.marked)
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
            let base_x = self
                .grid
                .cursor_index
                .and_then(|index| {
                    let row_text = self.grid.rows.get(row)?;
                    Some(shape_grid_row(window, row_text).x_for_index(index))
                })
                .unwrap_or(self.cell_width * column as f32);
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
                px(FONT_SIZE),
                &[gpui::TextRun {
                    len: self.input.marked.len(),
                    font: font("Menlo"),
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
                    text_width.max(self.cell_width),
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
                px(FONT_SIZE),
                &[gpui::TextRun {
                    len: composition.len(),
                    font: font("Menlo"),
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
            let row_text = self.grid.rows.get(row)?;
            let base_line = shape_grid_row(window, row_text);
            let base_x = base_line.x_for_index(base_byte);
            let column = (f32::from(base_x) / f32::from(self.cell_width)) as usize;
            return Some(Bounds {
                origin: point(
                    bounds.left() + base_x + left,
                    bounds.top() + grid_row_top(&self.grid, row, px(LINE_HEIGHT))
                        - px(super::vscroll_pixels_for_cell(&self.grid, row, column) as f32),
                ),
                size: size(
                    text_width.max(self.cell_width),
                    grid_row_height(&self.grid, row, px(LINE_HEIGHT)),
                ),
            });
        }

        let base_range = self.input.base_range_for_virtual(requested);
        let (row, start_byte) = visible_input_position(&self.grid, base_range.start, false)?;
        let end_position = visible_input_position(&self.grid, base_range.end, true);
        let row_text = self.grid.rows.get(row)?;
        let layout = shape_grid_row(window, row_text);
        let start = layout.x_for_index(start_byte);
        let end = match end_position {
            Some((end_row, end_byte)) if end_row == row => layout.x_for_index(end_byte),
            _ => layout.width,
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
                text_width.max(self.cell_width),
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
        mouse_position_for_point_with_cell_width(&self.grid, point, self.cell_width, window)
            .map(|position| position.utf16_offset)
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
}
