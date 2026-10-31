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

fn check(i: &mut Interp, format: &str, expected: &[(usize, usize, &str)]) {
    eval(
        i,
        &format!(
            "(let ((value {format})) (setq-local mode-line-format value header-line-format value tab-line-format value))"
        ),
    );
    let grid = redraw(i);
    let mode_row = grid.mode_lines[0].row;
    for row in [0, 1, mode_row] {
        let actual: Vec<_> = grid
            .help_echoes
            .iter()
            .filter(|region| region.row == row)
            .map(|region| (region.left, region.width, region.text.as_str()))
            .collect();
        assert_eq!(actual, expected, "row {row}, format {format}");
    }
}

#[test]
fn auxiliary_line_help_echo_follows_the_formatted_visible_characters() {
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
        r#"'((:propertize " Help " help-echo "Help text") " Plain")"#,
        &[(0, 6, "Help text")],
    );
    check(
        &mut i,
        r#"(list (propertize "AA" 'help-echo "first") " gap " (let ((text "xxHelpzz")) (put-text-property 2 6 'help-echo "second" text) text))"#,
        &[(0, 2, "first"), (9, 4, "second")],
    );
    check(
        &mut i,
        r#"(list "A" (propertize "%b" 'help-echo "buffer") "!")"#,
        &[(1, 2, "buffer")],
    );
    check(
        &mut i,
        r#"(list "A" (propertize "%%" 'help-echo "percent") " plain")"#,
        &[(1, 1, "percent")],
    );
    check(
        &mut i,
        r#"'("x\t" (:propertize "漢a" help-echo "wide") " plain")"#,
        &[(8, 3, "wide")],
    );
    check(
        &mut i,
        r#"(list (make-string 38 ?\s) (propertize "漢a" 'help-echo "clipped"))"#,
        &[(38, 2, "clipped")],
    );
    check(
        &mut i,
        r#"'("pre" (:eval (list " " (propertize "dynamic" 'help-echo "eval"))) "plain")"#,
        &[(4, 7, "eval")],
    );
    check(
        &mut i,
        r#"'((:propertize ("a" (:propertize "b" help-echo "inner")) help-echo "outer") "gap" (:propertize (:propertize "c" help-echo "hidden") help-echo nil))"#,
        &[(0, 2, "outer")],
    );
    check(
        &mut i,
        r#"'("pre" (:propertize "%-" help-echo "fill") (:propertize "unused" help-echo "unused"))"#,
        &[(3, 37, "fill")],
    );
    check(&mut i, "'((:propertize \"text\" help-echo nil))", &[]);

    eval(
        &mut i,
        r#"(progn
          (setq-local tab-line-format nil header-line-format nil)
          (setq-local mode-line-format '(" Plain " (:propertize "Help" help-echo "window")))
          (split-window-right 20))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(i.selected_frame.as_ref().unwrap(), 40, 12);
    let grid = redraw(&mut i);
    for line in &grid.mode_lines {
        let regions: Vec<_> = grid
            .help_echoes
            .iter()
            .filter(|r| r.row == line.row && r.left >= line.left && r.left < line.left + line.width)
            .collect();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].left, line.left + 7);
        assert_eq!(regions[0].width, 4);
        assert_eq!(regions[0].text, "window");
    }
}
