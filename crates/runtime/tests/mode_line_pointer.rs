use remacs_runtime::{
    frontend::{Grid, GridPointerShape as Shape, KeyIo, render_grid},
    lisp::{Interp, Value},
};
use std::{io, time::Duration};

struct Frontend;
impl KeyIo for Frontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(None)
    }
    fn unread(&mut self, _: i128) {}
    fn render(&mut self, _: &Interp) -> io::Result<()> {
        Ok(())
    }
    fn draw_echo(&mut self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (40, 12)
    }
}

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn redraw(i: &mut Interp) -> Grid {
    Frontend.run_pre_redisplay_functions(i).unwrap();
    render_grid(i, 40, 12)
}

fn pointer_at(grid: &Grid, row: usize, column: usize) -> Option<Shape> {
    grid.pointer_regions.iter().find_map(|region| {
        (region.row == row && column >= region.left && column < region.left + region.width)
            .then_some(region.shape)
    })
}

fn check(i: &mut Interp, format: &str, expected: &[(usize, usize, Shape)]) {
    eval(
        i,
        &format!(
            "(let ((value {format})) (setq-local mode-line-format value header-line-format value tab-line-format value))"
        ),
    );
    let grid = redraw(i);
    for row in [0, 1, grid.mode_lines[0].row] {
        for column in 0..40 {
            let shape = expected
                .iter()
                .find_map(|(left, width, shape)| {
                    (column >= *left && column < left + width).then_some(*shape)
                })
                .unwrap_or(Shape::Arrow);
            assert_eq!(
                pointer_at(&grid, row, column),
                Some(shape),
                "row {row} column {column}: {format}"
            );
        }
    }
    assert!(
        grid.pointer_regions
            .iter()
            .all(|region| region.width > 0 && region.left + region.width <= 40 && region.row < 12)
    );
}

#[test]
fn auxiliary_pointer_shapes_follow_formatted_properties_and_clear_outside_them() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
          (set-frame-parameter nil 'window-system 'ns)
          (set-frame-parameter nil 'menu-bar-lines 0)
          (set-frame-parameter nil 'tool-bar-lines 0)
          (set-frame-parameter nil 'tab-bar-lines 0)
          (set-frame-parameter nil 'vertical-scroll-bars nil)
          (set-frame-size nil 40 12) (set-window-fringes nil 0 0)
          (setq pre-redisplay-function nil)
          (setq-default fontification-functions nil)
          (setq-local fontification-functions nil tab-width 8)
          (rename-buffer "名" t) (erase-buffer))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    check(
        &mut i,
        r#"'((:propertize " Open " pointer hand) " Plain")"#,
        &[(0, 6, Shape::Hand)],
    );
    check(
        &mut i,
        r#"'((:propertize "Wait" pointer hourglass) " Plain")"#,
        &[(0, 4, Shape::Hourglass)],
    );
    check(
        &mut i,
        r#"(list (let ((text "xxOPENzz")) (put-text-property 2 6 'pointer 'hand text) text))"#,
        &[(2, 4, Shape::Hand)],
    );
    check(
        &mut i,
        r#"'((:propertize "L" pointer hdrag) (:propertize "V" pointer nhdrag) (:propertize "T" pointer text))"#,
        &[
            (0, 1, Shape::ResizeHorizontal),
            (1, 1, Shape::ResizeVertical),
            (2, 1, Shape::Text),
        ],
    );
    check(
        &mut i,
        r#"(list "x\t" (propertize "%b" 'pointer 'hand) " plain")"#,
        &[(8, 2, Shape::Hand)],
    );
    check(
        &mut i,
        r#"'(:eval (list "plain " (propertize "dynamic" 'pointer 'hand)))"#,
        &[(6, 7, Shape::Hand)],
    );
    check(
        &mut i,
        r#"'((:propertize (:propertize "Open" pointer hand) pointer nil))"#,
        &[],
    );
    check(
        &mut i,
        r#"'((:propertize ("a" (:propertize "b" pointer hand)) pointer text))"#,
        &[(0, 2, Shape::Text)],
    );
    check(
        &mut i,
        r#"'("pre" (:propertize "%-" pointer hand) (:propertize "unused" pointer text))"#,
        &[(3, 37, Shape::Hand)],
    );
    check(
        &mut i,
        r#"(list (make-string 39 ?\s) (propertize "名" 'pointer 'hand))"#,
        &[],
    );
    check(
        &mut i,
        r#"(list (make-string 38 ?\s) (propertize "名" 'pointer 'hand))"#,
        &[(38, 2, Shape::Hand)],
    );
    check(
        &mut i,
        r#"'((:propertize "unknown" pointer remacs-unknown-pointer))"#,
        &[],
    );

    eval(
        &mut i,
        r#"(progn
      (setq-local header-line-format nil tab-line-format nil)
      (setq-local mode-line-format '("plain " (:propertize "link" pointer hand)))
      (split-window-right 20))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let grid = redraw(&mut i);
    assert_eq!(grid.mode_lines.len(), 2);
    for line in &grid.mode_lines {
        for column in 0..line.width {
            let expected = if (6..10).contains(&column) {
                Shape::Hand
            } else {
                Shape::Arrow
            };
            assert_eq!(
                pointer_at(&grid, line.row, line.left + column),
                Some(expected)
            );
        }
    }
}
