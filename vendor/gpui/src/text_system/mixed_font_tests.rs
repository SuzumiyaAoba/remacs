use super::{TextRun, TextSystem, WindowTextSystem};
use crate::{MacTextSystem, black, font, px};
use std::sync::Arc;

fn text_system() -> WindowTextSystem {
    WindowTextSystem::new(Arc::new(TextSystem::new(Arc::new(MacTextSystem::new()))))
}

fn run(len: usize, family: &'static str) -> TextRun {
    TextRun {
        len,
        font: font(family),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

#[test]
fn mixed_font_sizes_preserve_native_metrics_and_cache_identity() {
    let system = text_system();
    let runs = [run(2, "Helvetica"), run(2, "Helvetica")];
    let normal = system.shape_line("MMMM".into(), px(14.0), &runs, None);
    let explicit = system.shape_line_with_font_sizes(
        "MMMM".into(),
        px(14.0),
        &runs,
        &[Some(px(14.0)), None],
        None,
    );
    assert!(Arc::ptr_eq(&normal.layout, &explicit.layout));
    let mixed = system.shape_line_with_font_sizes(
        "MMMM".into(),
        px(14.0),
        &runs,
        &[None, Some(px(28.0))],
        None,
    );
    assert!(!Arc::ptr_eq(&normal.layout, &mixed.layout));
    assert!(mixed.runs.iter().any(|run| run.font_size == px(14.0)));
    assert!(mixed.runs.iter().any(|run| run.font_size == px(28.0)));
    assert!(mixed.width > normal.width * 1.4);
    assert!(mixed.ascent > normal.ascent * 1.5);
    assert_eq!(
        mixed.runs.iter().map(|run| run.glyphs.len()).sum::<usize>(),
        4
    );
    let repeated = system.shape_line_with_font_sizes(
        "MMMM".into(),
        px(14.0),
        &runs,
        &[None, Some(px(28.0))],
        None,
    );
    assert!(Arc::ptr_eq(&mixed.layout, &repeated.layout));
}

#[test]
fn mixed_font_sizes_keep_unicode_indices_and_fallback_runs() {
    let system = text_system();
    let text = "é中🙂a";
    let runs = [
        run("é".len(), "Helvetica"),
        run("中🙂".len(), "Helvetica"),
        run(1, "Helvetica"),
    ];
    let line = system.shape_line_with_font_sizes(
        text.into(),
        px(14.0),
        &runs,
        &[None, Some(px(28.0)), None],
        None,
    );
    let mut indices = line
        .runs
        .iter()
        .flat_map(|run| run.glyphs.iter().map(|glyph| (glyph.index, run.font_size)))
        .collect::<Vec<_>>();
    indices.sort_by_key(|(index, _)| *index);
    assert_eq!(
        indices,
        vec![(0, px(14.0)), (2, px(28.0)), (5, px(28.0)), (9, px(14.0))]
    );
    assert!(line.x_for_index(9) > line.x_for_index(2));
    assert_eq!(line.closest_index_for_x(line.width), text.len());
}

#[test]
fn mixed_font_sizes_preserve_font_changes_and_ignore_invalid_overrides() {
    let system = text_system();
    let uniform_runs = [run(2, "Helvetica"), run(2, "Helvetica")];
    let normal = system.shape_line("MMMM".into(), px(14.0), &uniform_runs, None);
    let invalid = system.shape_line_with_font_sizes(
        "MMMM".into(),
        px(14.0),
        &uniform_runs,
        &[Some(px(0.0)), Some(px(f32::NAN))],
        None,
    );
    assert!(Arc::ptr_eq(&normal.layout, &invalid.layout));
    let different_fonts = [run(2, "Helvetica"), run(2, "Courier")];
    let line =
        system.shape_line_with_font_sizes("MMMM".into(), px(14.0), &different_fonts, &[], None);
    assert!(line.runs.len() >= 2);
    assert_ne!(line.width, normal.width);
}
