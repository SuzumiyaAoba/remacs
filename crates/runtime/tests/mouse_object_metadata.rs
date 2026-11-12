use remacs_runtime::frontend::{MouseObject, MouseObjectSource, render_grid_with_input_cached};
use remacs_runtime::lisp::Interp;

fn render_mouse_object_grid(source: &str) -> (Interp, remacs_runtime::frontend::Grid) {
    let mut interp = Interp::new();
    interp
        .eval_str(&format!(
            "(progn\
               (set-frame-parameter nil 'window-system 'ns)\
               (set-frame-parameter nil 'menu-bar-lines 0)\
               (set-frame-parameter nil 'tool-bar-lines 0)\
               (set-frame-parameter nil 'tab-bar-lines 0)\
               (set-frame-parameter nil 'vertical-scroll-bars nil)\
               (setq-default mode-line-format nil header-line-format nil tab-line-format nil)\
               (switch-to-buffer (get-buffer-create \"*mouse-object-metadata*\"))\
               (erase-buffer) (insert \"X\")\
               {source})"
        ))
        .unwrap();
    let grid = render_grid_with_input_cached(&interp, 80, 12, None);
    (interp, grid)
}

fn text_row_and_left(grid: &remacs_runtime::frontend::Grid) -> (usize, usize) {
    let input = grid.input.as_ref().unwrap();
    let row = input.rows.first().unwrap().row;
    let left = grid
        .window_text_areas
        .iter()
        .find(|area| row >= area.top && row < area.top.saturating_add(area.rows))
        .unwrap()
        .left;
    (row, left)
}

#[test]
fn display_string_hit_testing_keeps_the_original_string_and_offset() {
    let (mut interp, grid) = render_mouse_object_grid("(put-text-property 1 2 'display \"AB\")");
    let input = grid.input.as_ref().unwrap();
    let (row, left) = text_row_and_left(&grid);
    let display = interp.intern("display");

    for (column_offset, string_position) in [(0, 0), (1, 1)] {
        let hit = input
            .hit_test_position(row, left + column_offset, 0)
            .unwrap();
        assert_eq!(
            hit.object,
            Some(MouseObject::String {
                source: MouseObjectSource::TextProperty {
                    position: 0,
                    property: display,
                },
                position: string_position,
            })
        );
    }
}

#[test]
fn overlay_string_hit_testing_keeps_its_overlay_and_offset() {
    let (mut interp, grid) = render_mouse_object_grid(
        "(let ((overlay (make-overlay 1 1)))\
           (overlay-put overlay 'after-string \"LINK\"))",
    );
    let input = grid.input.as_ref().unwrap();
    let (row, left) = text_row_and_left(&grid);
    let after_string = interp.intern("after-string");

    let hit = input.hit_test_position(row, left + 2, 0).unwrap();
    assert_eq!(
        hit.object,
        Some(MouseObject::String {
            source: MouseObjectSource::OverlayProperty {
                index: 0,
                property: after_string,
            },
            position: 2,
        })
    );
}

#[test]
fn image_rendering_retains_its_source_display_property() {
    let (mut interp, grid) = render_mouse_object_grid(
        "(put-text-property 1 2 'display\
           (create-image\
             \"<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'/>\"\
             'svg t))",
    );
    let image = grid.images.first().expect("rendered SVG image");
    assert_eq!(
        image.object_source,
        Some(MouseObjectSource::TextProperty {
            position: 0,
            property: interp.intern("display"),
        })
    );
}
