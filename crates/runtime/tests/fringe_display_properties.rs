use remacs_runtime::{
    frontend::{Grid, GridFringeBitmapAlign, render_grid},
    lisp::{Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"))
}

fn reset(interp: &mut Interp, source: &str) -> Grid {
    eval(
        interp,
        "(progn (erase-buffer) (remove-overlays) (setq-local line-prefix nil truncate-lines nil overlay-arrow-position nil overlay-arrow-string nil) (set-window-fringes nil 8 8))",
    );
    eval(interp, source);
    render_grid(interp, 40, 12)
}

fn graphical_fringe_interpreter() -> Interp {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'char-width 8)
      (set-frame-parameter nil 'char-height 16)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 12)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil
                  indicate-empty-lines nil)
      (set-window-fringes nil 8 8))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);
    interp
}

fn fringe_bitmap_at(grid: &Grid, row: usize, column: usize) -> &remacs_runtime::frontend::GridFringeBitmap {
    grid.fringe_bitmaps
        .iter()
        .find(|bitmap| bitmap.row == row && bitmap.column == column)
        .unwrap_or_else(|| panic!("missing fringe bitmap at row {row}, column {column}"))
}

#[test]
fn fringe_indicator_alist_selects_boundary_bitmaps() {
    let mut interp = graphical_fringe_interpreter();
    let grid = reset(
        &mut interp,
        r#"(progn
      (insert "top\nmiddle\nbottom")
      (setq-local fringe-indicator-alist
                  '((up . right-arrow)
                    (down . left-arrow)
                    (top . right-triangle)
                    (bottom . left-triangle)))
      (setq-local indicate-buffer-boundaries '((top . left) (t . right)))
      (goto-char (point-min)))"#,
    );
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    assert_eq!(
        &*fringe_bitmap_at(&grid, 0, 0).bits,
        &[0xc0, 0xf0, 0xf8, 0xfc, 0xfc, 0xf8, 0xf0, 0xc0]
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, 2, 39).bits,
        &[0x03, 0x0f, 0x1f, 0x3f, 0x3f, 0x1f, 0x0f, 0x03]
    );
}

#[test]
fn fringe_indicator_alist_selects_truncation_continuation_empty_line_and_overlay_bitmaps() {
    let mut interp = graphical_fringe_interpreter();
    let alist = "((truncation . (left-arrow right-arrow)) \
                  (continuation . (left-triangle right-triangle)) \
                  (empty-line . right-arrow) \
                  (overlay-arrow . left-arrow))";

    let grid = reset(
        &mut interp,
        &format!(
            r#"(progn
              (setq-local fringe-indicator-alist '{alist}
                          indicate-buffer-boundaries nil truncate-lines t)
              (insert (make-string 50 ?x))
              (goto-char (point-min)))"#
        ),
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, 0, 39).bits,
        &[0x18, 0x0c, 0x06, 0x3f, 0x3f, 0x06, 0x0c, 0x18]
    );

    let grid = reset(
        &mut interp,
        &format!(
            r#"(progn
              (setq-local fringe-indicator-alist '{alist}
                          indicate-buffer-boundaries nil truncate-lines nil)
              (insert (make-string 50 ?x))
              (goto-char (point-min)))"#
        ),
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, 1, 0).bits,
        &[0x03, 0x0f, 0x1f, 0x3f, 0x3f, 0x1f, 0x0f, 0x03]
    );

    let grid = reset(
        &mut interp,
        &format!(
            r#"(progn
              (setq-local fringe-indicator-alist '{alist}
                          indicate-buffer-boundaries nil indicate-empty-lines t)
              (insert "line")
              (goto-char (point-min)))"#
        ),
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, 1, 39).bits,
        &[0x18, 0x0c, 0x06, 0x3f, 0x3f, 0x06, 0x0c, 0x18]
    );

    let grid = reset(
        &mut interp,
        &format!(
            r#"(progn
              (setq-local fringe-indicator-alist '{alist}
                          indicate-buffer-boundaries nil
                          overlay-arrow-position (copy-marker (point-min))
                          overlay-arrow-string ">")
              (insert "line")
              (goto-char (point-min)))"#
        ),
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, 0, 0).bits,
        &[0x18, 0x30, 0x60, 0xfc, 0xfc, 0x60, 0x30, 0x18]
    );
}

#[test]
fn indicate_buffer_boundaries_shows_scroll_arrows_at_window_edges() {
    let mut interp = graphical_fringe_interpreter();
    eval(&mut interp, "(set-frame-size nil 40 6)");
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 6);

    let grid = reset(
        &mut interp,
        r#"(progn
      (setq-local indicate-buffer-boundaries 'left)
      (dotimes (_ 30) (insert "line\n"))
      (goto-char 61)
      (set-window-start nil 51 t))"#,
    );
    let area = grid.window_text_areas.first().unwrap();
    assert!(area.rows >= 3);
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    assert_eq!(
        &*fringe_bitmap_at(&grid, area.top, 0).bits,
        &[0x18, 0x3c, 0x7e, 0xff, 0x18, 0x18, 0x18, 0x18]
    );
    assert_eq!(
        &*fringe_bitmap_at(&grid, area.top + area.rows - 1, 0).bits,
        &[0x18, 0x18, 0x18, 0x18, 0xff, 0x7e, 0x3c, 0x18]
    );

    let grid = reset(
        &mut interp,
        r#"(progn
      (insert "top\nmiddle\nbottom")
      (setq-local indicate-buffer-boundaries t)
      (goto-char (point-min)))"#,
    );
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    assert!(grid.fringe_bitmaps.iter().all(|bitmap| bitmap.column == 0));
}

#[test]
fn indicate_buffer_boundaries_alist_places_indicators_independently() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'char-width 8)
      (set-frame-parameter nil 'char-height 16)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 12)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil
                  indicate-empty-lines nil)
      (set-window-fringes nil 8 8))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);

    let grid = reset(
        &mut interp,
        r#"(progn
      (insert "top\nmiddle\nbottom")
      (setq-local indicate-buffer-boundaries '((top . left) (t . right)))
      (goto-char (point-min)))"#,
    );
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    let top = grid
        .fringe_bitmaps
        .iter()
        .find(|bitmap| bitmap.row == 0 && bitmap.column == 0)
        .unwrap();
    assert_eq!(&*top.bits, &[0xfc, 0xfc, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0x00]);
    let bottom = grid
        .fringe_bitmaps
        .iter()
        .find(|bitmap| bitmap.row == 2 && bitmap.column == 39)
        .unwrap();
    assert_eq!(&*bottom.bits, &[0xfc, 0xfc, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0, 0x00]);

    let grid = reset(
        &mut interp,
        r#"(progn
      (insert "top\nmiddle\nbottom")
      (setq-local indicate-buffer-boundaries '((top . nil) (t . right)))
      (goto-char (point-min)))"#,
    );
    assert_eq!(grid.fringe_bitmaps.len(), 1);
    assert_eq!(grid.fringe_bitmaps[0].row, 2);
    assert_eq!(grid.fringe_bitmaps[0].column, 39);
}

#[test]
fn overlay_arrow_string_nil_suppresses_the_fringe_arrow() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'char-width 8)
      (set-frame-parameter nil 'char-height 16)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 12)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil
                  indicate-empty-lines nil)
      (set-window-fringes nil 8 8))"#,
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);

    let grid = reset(
        &mut interp,
        r#"(progn
      (insert "line")
      (setq-local overlay-arrow-position (copy-marker (point-min))
                  overlay-arrow-string nil))"#,
    );
    assert_eq!(grid.rows[0].chars().next(), Some(' '));
}

fn body(grid: &Grid, row: usize) -> String {
    grid.rows[row]
        .chars()
        .skip(1)
        .take(38)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn fringe_display_replaces_text_with_positioned_bitmaps_and_face_colors() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r##"(progn
      (set-frame-parameter nil 'window-system 'ns)
      (set-frame-parameter nil 'char-width 8)
      (set-frame-parameter nil 'char-height 16)
      (set-frame-parameter nil 'menu-bar-lines 0)
      (set-frame-parameter nil 'tool-bar-lines 0)
      (set-frame-parameter nil 'tab-bar-lines 0)
      (set-frame-parameter nil 'vertical-scroll-bars nil)
      (set-frame-parameter nil 'horizontal-scroll-bars nil)
      (set-frame-size nil 40 12)
      (setq-local mode-line-format nil header-line-format nil tab-line-format nil)
      (setq-local indicate-empty-lines nil indicate-buffer-boundaries nil)
      (set-face-attribute 'default nil :foreground "#102030" :background "#ffffff")
      (set-face-attribute 'fringe nil :foreground "#445566" :background "#778899")
      (make-face 'remacs-test-fringe-face)
      (set-face-attribute 'remacs-test-fringe-face nil :foreground "#ff0000"))"##,
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);
    let grid = reset(
        &mut interp,
        r#"(progn
      (insert (propertize "L" 'display '(left-fringe right-arrow remacs-test-fringe-face)))
      (insert "abc")
      (insert (propertize "R" 'display '(right-fringe left-arrow)))
      (goto-char 3))"#,
    );
    assert_eq!(body(&grid, 0), "abc");
    assert_eq!(grid.cursor, Some((2, 0)));
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    let left = grid
        .fringe_bitmaps
        .iter()
        .find(|bitmap| bitmap.column == 0)
        .unwrap();
    let right = grid
        .fringe_bitmaps
        .iter()
        .find(|bitmap| bitmap.column == 39)
        .unwrap();
    assert_eq!(left.row, 0);
    assert_eq!(left.pixel_width, 8);
    assert_eq!(
        &*left.bits,
        &[0x18, 0x0c, 0x06, 0x3f, 0x3f, 0x06, 0x0c, 0x18]
    );
    assert_eq!(
        left.style.fg,
        Some(remacs_runtime::frontend::CellColor(255, 0, 0))
    );
    assert_eq!(
        left.style.bg,
        Some(remacs_runtime::frontend::CellColor(0x77, 0x88, 0x99))
    );
    assert_eq!(right.bits[1], 0x30);
    assert_eq!(
        right.style.fg,
        Some(remacs_runtime::frontend::CellColor(0x44, 0x55, 0x66))
    );

    let grid = reset(
        &mut interp,
        "(progn (insert \"aXXb\") (put-text-property 2 4 'display '(left-fringe right-arrow)) (put-text-property 3 4 'face 'bold) (goto-char 4))",
    );
    assert_eq!(body(&grid, 0), "ab");
    assert_eq!(grid.fringe_bitmaps.len(), 1);
    assert_eq!(grid.cursor, Some((2, 0)));
    let grid = reset(
        &mut interp,
        "(progn (insert \"aXb\") (put-text-property 2 3 'display '(left-fringe right-arrow)) (overlay-put (make-overlay 2 3) 'display '(right-fringe left-arrow)) (goto-char 1))",
    );
    assert_eq!(body(&grid, 0), "ab");
    assert_eq!(grid.fringe_bitmaps.len(), 1);
    assert_eq!(grid.fringe_bitmaps[0].column, 39);
    let grid = reset(&mut interp, "(progn (insert \"a\\nbXc\") (put-text-property 4 5 'display '(left-fringe down-arrow)) (goto-char 1))".replace("\\n", "\n").as_str());
    assert_eq!(body(&grid, 1), "bc");
    assert_eq!(grid.fringe_bitmaps[0].row, 1);
    assert_eq!(grid.fringe_bitmaps[0].align, GridFringeBitmapAlign::Bottom);

    let grid = reset(
        &mut interp,
        r#"(progn
      (insert (propertize "X" 'display '(left-fringe right-arrow)))
      (insert "ok") (goto-char 1)
      (setq-local overlay-arrow-position (copy-marker 1) overlay-arrow-string "=>"))"#,
    );
    assert_eq!(body(&grid, 0), "ok");
    assert_eq!(grid.fringe_bitmaps.len(), 1);
    assert_eq!(grid.fringe_bitmaps[0].column, 0);
    assert_eq!(
        &*grid.fringe_bitmaps[0].bits,
        &[0xc0, 0xf0, 0xf8, 0xfc, 0xfc, 0xf8, 0xf0, 0xc0]
    );

    for specification in [
        "(left-fringe missing-bitmap)",
        "(right-fringe 42)",
        "(left-fringe nil)",
    ] {
        let grid = reset(
            &mut interp,
            &format!(
                "(progn (insert \"aXb\") (put-text-property 2 3 'display '{specification}) (goto-char 1))"
            ),
        );
        assert_eq!(body(&grid, 0), "ab");
        assert!(grid.fringe_bitmaps.is_empty());
    }
    let grid = reset(
        &mut interp,
        "(progn (insert \"aXb\") (put-text-property 2 3 'display '(left-fringe)) (goto-char 1))",
    );
    assert_eq!(body(&grid, 0), "aXb");
    let grid = reset(
        &mut interp,
        "(progn (insert \"aXb\") (put-text-property 2 3 'display '(left-fringe right-arrow)) (set-window-fringes nil 0 0) (goto-char 1))",
    );
    assert_eq!(grid.rows[0].trim_end(), "ab");
    assert!(grid.fringe_bitmaps.is_empty());
    let grid = reset(
        &mut interp,
        "(progn (set-frame-parameter nil 'window-system nil) (insert \"aXb\") (put-text-property 2 3 'display '(left-fringe right-arrow)) (goto-char 1))",
    );
    assert_eq!(grid.rows[0].trim_end(), "ab");
    assert!(grid.fringe_bitmaps.is_empty());
    eval(&mut interp, "(set-frame-parameter nil 'window-system 'ns)");

    let grid = reset(
        &mut interp,
        "(progn (put 'remacs-test-fringe-alias 'fringe (get 'right-arrow 'fringe)) (insert (propertize \"X\" 'display '(left-fringe remacs-test-fringe-alias))) (insert (make-string 38 ?a)) (goto-char 1))",
    );
    assert_eq!(body(&grid, 0), "a".repeat(38));
    assert_eq!(body(&grid, 1), "");
    assert_eq!(grid.fringe_bitmaps.len(), 1);
    let grid = reset(
        &mut interp,
        "(progn (setq-local line-prefix \">\") (insert (propertize \"X\" 'display '(left-fringe right-arrow))) (insert (make-string 37 ?b)) (goto-char 1))",
    );
    assert_eq!(body(&grid, 0), format!(">{}", "b".repeat(37)));
    assert_eq!(body(&grid, 1), "");

    let names = eval(&mut interp, "fringe-bitmaps").list_to_vec().unwrap();
    assert_eq!(names.len(), 24);
    for name in names {
        let name = interp.prin1_to_string(&name).to_unicode_lossy();
        let grid = reset(
            &mut interp,
            &format!(
                "(progn (insert (propertize \"X\" 'display '(left-fringe {name}))) (insert \"ok\") (goto-char 1))"
            ),
        );
        assert_eq!(body(&grid, 0), "ok", "{name}");
        assert_eq!(grid.fringe_bitmaps.len(), 1, "{name}");
        assert!(!grid.fringe_bitmaps[0].bits.is_empty());
        if name == "up-arrow" {
            assert_eq!(grid.fringe_bitmaps[0].align, GridFringeBitmapAlign::Top);
        }
        if name == "empty-line" {
            assert!(grid.fringe_bitmaps[0].repeat);
        }
    }

    let grid = reset(
        &mut interp,
        r##"(progn
      (set-face-attribute 'remacs-test-fringe-face nil :foreground "#111111" :background "#111111" :distant-foreground "#ff0000")
      (set-frame-parameter nil 'screen-gamma 1.0)
      (insert (propertize "X" 'display '(left-fringe right-arrow remacs-test-fringe-face)))
      (insert (propertize "a" 'face 'remacs-test-fringe-face)) (goto-char 1))"##,
    );
    assert_eq!(grid.fringe_bitmaps[0].style.fg, grid.attrs[0][1].fg);
    assert_eq!(grid.fringe_bitmaps[0].style.bg, grid.attrs[0][1].bg);
    eval(&mut interp, "(set-frame-parameter nil 'screen-gamma nil)");

    let _ = reset(
        &mut interp,
        "(progn (insert (propertize \"X\" 'display '(left-fringe right-arrow))) (insert \"one\") (goto-char 1))",
    );
    eval(
        &mut interp,
        "(progn (with-current-buffer (get-buffer-create \"fringe-other\") (erase-buffer) (insert (propertize \"X\" 'display '(left-fringe down-arrow))) (insert \"two\") (setq-local mode-line-format nil header-line-format nil tab-line-format nil)) (setq remacs-test-fringe-window (split-window-right 20)) (set-window-buffer remacs-test-fringe-window \"fringe-other\") (set-window-fringes remacs-test-fringe-window 4 0))",
    );
    remacs_runtime::editor::reflow_frame_windows(interp.selected_frame.as_ref().unwrap(), 40, 12);
    let grid = render_grid(&interp, 40, 12);
    assert_eq!(grid.fringe_bitmaps.len(), 2);
    assert_eq!(
        (
            grid.fringe_bitmaps[0].column,
            grid.fringe_bitmaps[0].pixel_width
        ),
        (0, 8)
    );
    assert_eq!(
        (
            grid.fringe_bitmaps[1].column,
            grid.fringe_bitmaps[1].pixel_width
        ),
        (20, 4)
    );
}
