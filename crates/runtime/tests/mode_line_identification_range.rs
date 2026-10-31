use remacs_runtime::{
    frontend::{Grid, KeyIo, render_grid},
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

fn check(i: &mut Interp, format: &str, expected: Option<(usize, usize)>) {
    eval(i, &format!("(setq-local mode-line-format {format})"));
    let grid = redraw(i);
    assert_eq!(grid.mode_lines.len(), 1);
    assert_eq!(grid.mode_lines[0].buffer_name_range, expected, "{format}");
}

#[test]
fn identification_hit_area_comes_from_its_local_map_not_repeated_text() {
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
          (setq-local fontification-functions nil header-line-format nil tab-line-format nil tab-width 8)
          (rename-buffer "same" t) (erase-buffer))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);

    // The real default identification has a local-map on the whole %12b field.
    check(
        &mut i,
        "'(\"%b | \" mode-line-buffer-identification)",
        Some((7, 19)),
    );
    check(&mut i, "'(\"%b | %b\")", None);
    check(
        &mut i,
        "'(\"same | \" mode-line-buffer-identification \" same\")",
        Some((7, 19)),
    );
    check(
        &mut i,
        "'(\"x\\t\" mode-line-buffer-identification)",
        Some((8, 20)),
    );
    check(
        &mut i,
        "(list (make-string 35 ?\\s) 'mode-line-buffer-identification)",
        Some((35, 40)),
    );

    eval(
        &mut i,
        "(setq-local mode-line-buffer-identification (propertized-buffer-identification \"ID:%b\"))",
    );
    check(
        &mut i,
        "'(\"%b | \" mode-line-buffer-identification)",
        Some((7, 14)),
    );
    check(
        &mut i,
        "'(:eval (list \"%b | \" mode-line-buffer-identification))",
        Some((7, 14)),
    );
    check(
        &mut i,
        "'((:propertize mode-line-buffer-identification local-map nil))",
        None,
    );

    eval(&mut i, "(rename-buffer \"名\" t)");
    check(
        &mut i,
        "'(\"%b | \" mode-line-buffer-identification)",
        Some((5, 10)),
    );

    eval(
        &mut i,
        "(setq-local mode-line-format '(\"%b | \" mode-line-buffer-identification)) (split-window-right 20)",
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let grid = redraw(&mut i);
    assert_eq!(grid.mode_lines.len(), 2);
    for line in &grid.mode_lines {
        assert_eq!(
            line.buffer_name_range,
            Some((line.left + 5, line.left + 10))
        );
    }
}
