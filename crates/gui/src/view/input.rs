//! OS text input for the command-loop front-end. The input client owns the
//! provisional composition; committed text belongs to the editor logic thread.

use std::ops::Range;

use super::{EditorView, FG, FONT_SIZE, GuiEvent, LINE_HEIGHT};
use gpui::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, font, point, px,
    rgb, size,
};

#[derive(Clone, Default)]
pub(super) struct TextInputState {
    pub(super) marked: String,
    selection: Range<usize>,
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
        std::mem::take(&mut self.marked)
    }

    fn commit(&mut self, replacement: Option<Range<usize>>, text: &str) -> String {
        self.mark(replacement, text, None);
        self.take()
    }

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
}

impl EditorView {
    fn send_text(&self, text: String) {
        if !text.is_empty() {
            let _ = self.tx.send(GuiEvent::Text(text));
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
        let (text, actual) = self.input.text(requested)?;
        *adjusted = Some(actual);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.input.selection.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        (!self.input.marked.is_empty()).then(|| 0..self.input.len())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.take();
        self.send_text(text);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        replacement: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.input.commit(replacement, text);
        self.send_text(text);
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
        self.input.mark(replacement, text, selected);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        requested: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (column, row) = self.grid.cursor?;
        let (prefix, _) = self.input.text(0..requested.start)?;
        let (text, _) = self.input.text(requested)?;
        let cursor_x = self
            .grid
            .cursor_index
            .and_then(|index| {
                let row_text = self.grid.rows.get(row)?;
                let font = font("Menlo");
                let layout = _window.text_system().shape_line(
                    row_text.clone().into(),
                    px(FONT_SIZE),
                    &[gpui::TextRun {
                        len: row_text.len(),
                        font,
                        color: rgb(FG).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                Some(layout.x_for_index(index))
            })
            .unwrap_or(self.cell_width * column as f32);
        let composition = self.input.marked.clone();
        let composition_line = _window.text_system().shape_line(
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
        let start = composition_line.x_for_index(prefix.len());
        let end = composition_line.x_for_index(prefix.len() + text.len());
        let left = start.min(end);
        let text_width = if end >= start {
            end - start
        } else {
            start - end
        }
        .max(self.cell_width);
        Some(Bounds {
            origin: point(
                bounds.left() + cursor_x + left,
                bounds.top() + px(LINE_HEIGHT) * row as f32,
            ),
            size: size(text_width, px(LINE_HEIGHT)),
        })
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
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
