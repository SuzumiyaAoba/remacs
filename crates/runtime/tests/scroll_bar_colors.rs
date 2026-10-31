use remacs_runtime::{
    editor::FrameRef,
    frontend::{CellColor, Grid, GridScrollBarOrientation, render_frame_grid_with_input_cached},
    lisp::{Flow, Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn check(interp: &Interp, frame: &FrameRef, foreground: CellColor, background: CellColor) -> Grid {
    let grid = render_frame_grid_with_input_cached(interp, frame, 40, 12, None);
    assert_eq!(grid.scroll_bars.len(), 2);
    assert!(
        grid.scroll_bars
            .iter()
            .any(|bar| bar.orientation == GridScrollBarOrientation::Vertical)
    );
    assert!(
        grid.scroll_bars
            .iter()
            .any(|bar| bar.orientation == GridScrollBarOrientation::Horizontal)
    );
    for bar in &grid.scroll_bars {
        assert_eq!(bar.style.fg, Some(foreground));
        assert_eq!(bar.style.bg, Some(background));
    }
    grid
}

#[test]
fn scroll_bars_follow_faces_frame_parameters_and_color_correction() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        r#"(progn
      (modify-frame-parameters nil '((window-system . ns) (width . 40) (height . 12)
         (char-width . 8) (char-height . 16) (vertical-scroll-bars . right)
         (horizontal-scroll-bars . bottom) (scroll-bar-width . 8) (scroll-bar-height . 8)
         (tool-bar-lines . 0) (tab-bar-lines . 0)))
      (setq-local truncate-lines t)
      (erase-buffer) (dotimes (_ 50) (insert (make-string 120 ?x) "\n")) (goto-char 1)
      (set-face-attribute 'scroll-bar nil :foreground "red" :background "yellow"))"#,
    );
    let primary = interp.selected_frame.clone().unwrap();
    remacs_runtime::editor::reflow_frame_windows(&primary, 40, 12);
    let face_grid = check(
        &interp,
        &primary,
        CellColor(255, 0, 0),
        CellColor(255, 255, 0),
    );
    let parameters = eval(
        &mut interp,
        "(list (frame-parameter nil 'scroll-bar-foreground) (frame-parameter nil 'scroll-bar-background))",
    );
    assert_eq!(interp.prin1_to_string(&parameters), r#"("red" "yellow")"#);

    eval(
        &mut interp,
        r##"(modify-frame-parameters nil
      '((scroll-bar-foreground . "#112233") (scroll-bar-background . "#445566")))"##,
    );
    let parameter_grid = check(
        &interp,
        &primary,
        CellColor(0x11, 0x22, 0x33),
        CellColor(0x44, 0x55, 0x66),
    );
    assert_ne!(face_grid.scroll_bars, parameter_grid.scroll_bars);
    eval(
        &mut interp,
        "(set-face-attribute 'scroll-bar nil :foreground 'unspecified :background 'unspecified)",
    );
    check(
        &interp,
        &primary,
        CellColor(0x11, 0x22, 0x33),
        CellColor(0x44, 0x55, 0x66),
    );
    // A subsequent face write takes effect after explicit parameter colors.
    eval(
        &mut interp,
        r#"(set-face-attribute 'scroll-bar nil :foreground "cyan" :background "magenta")"#,
    );
    check(
        &interp,
        &primary,
        CellColor(0, 255, 255),
        CellColor(255, 0, 255),
    );

    let second = eval(
        &mut interp,
        r##"(make-frame '((window-system . ns) (width . 40) (height . 12)
      (vertical-scroll-bars . right) (horizontal-scroll-bars . bottom)
      (scroll-bar-width . 8) (scroll-bar-height . 8)
      (scroll-bar-foreground . "blue") (scroll-bar-background . "#00ff00")))"##,
    );
    let Value::Frame(second) = second else {
        panic!("second frame");
    };
    check(&interp, &second, CellColor(0, 0, 255), CellColor(0, 255, 0));
    eval(
        &mut interp,
        r#"(set-face-attribute 'scroll-bar nil :foreground "red")"#,
    );
    check(
        &interp,
        &primary,
        CellColor(255, 0, 0),
        CellColor(255, 0, 255),
    );
    check(&interp, &second, CellColor(0, 0, 255), CellColor(0, 255, 0));
    let set_face = Value::Sym(interp.intern("set-face-attribute"));
    let face = Value::Sym(interp.intern("scroll-bar"));
    let foreground = Value::Sym(interp.intern(":foreground"));
    interp
        .apply(
            &set_face,
            vec![
                face,
                Value::Frame(second.clone()),
                foreground,
                Value::string("yellow"),
            ],
        )
        .unwrap();
    check(
        &interp,
        &second,
        CellColor(255, 255, 0),
        CellColor(0, 255, 0),
    );
    check(
        &interp,
        &primary,
        CellColor(255, 0, 0),
        CellColor(255, 0, 255),
    );

    eval(
        &mut interp,
        r#"(progn
      (internal-set-lisp-face-attribute 'scroll-bar :foreground "white" 0)
      (set-face-background 'scroll-bar "black" 0))"#,
    );
    check(
        &interp,
        &primary,
        CellColor(255, 255, 255),
        CellColor(0, 0, 0),
    );
    check(
        &interp,
        &second,
        CellColor(255, 255, 255),
        CellColor(0, 0, 0),
    );
    eval(
        &mut interp,
        "(progn (set-face-attribute 'scroll-bar t :foreground 'unspecified :background 'unspecified) (set-face-attribute 'scroll-bar nil :foreground nil :background nil) (set-frame-parameter nil 'scroll-bar-foreground nil) (set-frame-parameter nil 'scroll-bar-background nil))",
    );
    check(
        &interp,
        &primary,
        CellColor(0x74, 0x7a, 0x80),
        CellColor(0x30, 0x34, 0x38),
    );
    eval(
        &mut interp,
        r##"(progn
      (set-face-attribute 'default nil :foreground "#808080" :background "#404040")
      (set-face-attribute 'scroll-bar nil :foreground "#808080" :background "#404040" :inverse-video t)
      (set-frame-parameter nil 'screen-gamma 1.0))"##,
    );
    let grid = render_frame_grid_with_input_cached(&interp, &primary, 40, 12, None);
    assert!(grid.gamma_exponent.is_some());
    assert_ne!(grid.default_style.fg, Some(CellColor(128, 128, 128)));
    for bar in &grid.scroll_bars {
        assert_eq!(bar.style.fg, grid.default_style.fg);
        assert_eq!(bar.style.bg, grid.default_style.bg);
        assert!(bar.style.inverse);
    }
}
