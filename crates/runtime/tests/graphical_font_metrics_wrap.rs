use remacs_runtime::{
    frontend::{
        FontAtStyle, FontGlyphKey, FrameFontMetrics, render_grid, update_frame_font_metrics,
    },
    lisp::Interp,
};
use std::collections::HashMap;

fn count_character(text: &str, character: char) -> usize {
    text.chars().filter(|candidate| *candidate == character).count()
}

#[test]
fn graphical_default_face_metrics_wrap_wide_and_narrow_glyphs_by_pixels() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(graphical_default_face_metrics_wrap_wide_and_narrow_glyphs_by_pixels_inner)
        .expect("spawn font-wrap test thread")
        .join()
        .expect("font-wrap test thread");
}

fn graphical_default_face_metrics_wrap_wide_and_narrow_glyphs_by_pixels_inner() {
    let mut interp = Interp::new();
    interp
        .eval_str(
            r#"(progn
              (set-frame-parameter nil 'window-system 'gpui)
              (set-frame-parameter nil 'menu-bar-lines 0)
              (set-frame-parameter nil 'tool-bar-lines 0)
              (set-frame-parameter nil 'tab-bar-lines 0)
              (set-frame-parameter nil 'vertical-scroll-bars nil)
              (set-frame-size nil 80 12)
              (setq-local truncate-lines nil)
              (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
              (insert (make-string 120 ?W)))"#,
        )
        .unwrap();
    let frame = interp.selected_frame.as_ref().unwrap().clone();
    let frame_id = frame.borrow().id;

    let initial = render_grid(&interp, 80, 12);
    let mut glyph_widths_64 = HashMap::new();
    for (row_index, row) in initial.rows.iter().enumerate() {
        let styles = initial
            .char_attrs
            .get(row_index)
            .filter(|styles| !styles.is_empty())
            .or_else(|| initial.attrs.get(row_index));
        for (column, character) in row.chars().enumerate() {
            if character != 'W' {
                continue;
            }
            let cell_style = styles
                .and_then(|styles| styles.get(column))
                .copied()
                .unwrap_or_default();
            let style = FontAtStyle {
                family: cell_style
                    .font_family
                    .and_then(|index| initial.font_families.get(index as usize).cloned()),
                weight: cell_style.font_weight,
                width: cell_style.font_width,
                height: cell_style.font_height,
                bold: cell_style.bold,
                italic: cell_style.italic,
            };
            glyph_widths_64.insert(
                FontGlyphKey {
                    style: style.clone(),
                    character: 'W',
                },
                11 * 64,
            );
            glyph_widths_64.insert(
                FontGlyphKey {
                    style,
                    character: 'i',
                },
                3 * 64,
            );
        }
    }
    assert!(!glyph_widths_64.is_empty(), "initial grid should contain W");
    update_frame_font_metrics(
        &mut interp,
        frame_id,
        FrameFontMetrics {
            signature: 1,
            default_cell_width_64: 8 * 64,
            glyph_widths_64,
        },
    );

    let wide = render_grid(&interp, 80, 12);
    let text_width = wide
        .window_text_areas
        .first()
        .expect("graphical text area")
        .text_width;
    let wide_glyphs_per_row = text_width * 8 / 11;
    assert_eq!(count_character(&wide.rows[0], 'W'), wide_glyphs_per_row);
    assert_eq!(count_character(&wide.rows[1], 'W'), wide_glyphs_per_row);

    interp
        .eval_str("(setq-local truncate-lines t) (goto-char (point-min))")
        .unwrap();
    let truncated = render_grid(&interp, 80, 12);
    assert_eq!(count_character(&truncated.rows[0], 'W'), wide_glyphs_per_row);
    assert_eq!(count_character(&truncated.rows[1], 'W'), 0);

    interp
        .eval_str("(setq-local truncate-lines nil) (erase-buffer) (insert (make-string 200 ?i))")
        .unwrap();
    let narrow = render_grid(&interp, 80, 12);
    assert_eq!(count_character(&narrow.rows[0], 'i'), 200);
}
