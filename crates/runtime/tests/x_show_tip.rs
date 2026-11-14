use remacs_runtime::{
    frontend::{CellColor, GridTooltipPosition, render_frame_grid_with_input_cached, render_grid},
    lisp::{Flow, Interp, Value},
};

fn eval(interp: &mut Interp, source: &str) {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
}

#[test]
fn x_show_tip_uses_position_arguments_and_timeout() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(x_show_tip_uses_position_arguments_and_timeout_inner)
        .unwrap()
        .join()
        .unwrap();
}

fn x_show_tip_uses_position_arguments_and_timeout_inner() {
    let mut interp = Interp::new();
    eval(
        &mut interp,
        "(progn (set-frame-parameter nil 'window-system 'ns) \
         (set-frame-parameter nil 'left 40) \
         (set-frame-parameter nil 'top 50))",
    );
    interp.gui_mouse_absolute_position = Some((120, 130));

    eval(
        &mut interp,
        "(x-show-tip \"positioned\" nil '((left . 200) (top . 300)) 10 7 -4)",
    );
    let grid = render_grid(&interp, 80, 24);
    assert_eq!(grid.tooltip.as_ref().map(|tip| tip.text.as_str()), Some("positioned"));
    assert_eq!(
        grid.tooltip_position,
        Some(GridTooltipPosition {
            left: Some(200),
            right: None,
            top: Some(300),
            bottom: None,
            pointer: Some((120, 130)),
            dx: 7,
            dy: -4,
        })
    );

    eval(&mut interp, "(x-show-tip \"pointer\" nil nil 10 5 -10)");
    let grid = render_grid(&interp, 80, 24);
    assert_eq!(grid.tooltip.as_ref().map(|tip| tip.text.as_str()), Some("pointer"));
    assert_eq!(
        grid.tooltip_position,
        Some(GridTooltipPosition {
            left: None,
            right: None,
            top: None,
            bottom: None,
            pointer: Some((120, 130)),
            dx: 5,
            dy: -10,
        })
    );

    eval(&mut interp, "(x-show-tip \"short\" nil nil 0.01)");
    assert_eq!(
        render_grid(&interp, 80, 24)
            .tooltip
            .as_ref()
            .map(|tip| tip.text.as_str()),
        Some("short")
    );
    eval(&mut interp, "(sleep-for 0.05)");
    let grid = render_grid(&interp, 80, 24);
    assert_eq!(grid.tooltip, None);
    assert_eq!(grid.tooltip_position, None);
}

#[test]
fn x_show_tip_targets_frame_and_preserves_face_properties() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            eval(
                &mut interp,
                "(set-frame-parameter nil 'window-system 'ns)",
            );
            let primary = interp.selected_frame.as_ref().unwrap().clone();
            let Value::Frame(secondary) = interp
                .eval_str(
                    "(setq x-show-tip-secondary \
                          (make-frame '((window-system . ns) (name . \"x-show-tip-secondary\"))))",
                )
                .unwrap()
            else {
                panic!("expected secondary frame");
            };
            eval(
                &mut interp,
                "(set-face-attribute 'tooltip nil :foreground \"blue\" :background \"yellow\")",
            );
            eval(
                &mut interp,
                "(set-face-attribute 'error nil :foreground \"red\" :weight 'bold)",
            );
            eval(
                &mut interp,
                "(x-show-tip (propertize \"styled tip\" 'face 'error) x-show-tip-secondary)",
            );

            let primary_grid =
                render_frame_grid_with_input_cached(&interp, &primary, 80, 24, None);
            assert!(primary_grid.tooltip.is_none());
            let secondary_grid =
                render_frame_grid_with_input_cached(&interp, &secondary, 80, 24, None);
            let tooltip = secondary_grid.tooltip.as_ref().expect("target tooltip");
            assert_eq!(tooltip.text, "styled tip");
            assert_eq!(tooltip.style.fg, Some(CellColor(0, 0, 255)));
            assert_eq!(tooltip.style.bg, Some(CellColor(255, 255, 0)));
            assert_eq!(tooltip.runs.len(), 1);
            assert_eq!(tooltip.runs[0].len, "styled tip".len());
            assert_eq!(tooltip.runs[0].style.fg, Some(CellColor(255, 0, 0)));
            assert_eq!(tooltip.runs[0].style.bg, Some(CellColor(255, 255, 0)));
        })
        .unwrap()
        .join()
        .unwrap();
}
