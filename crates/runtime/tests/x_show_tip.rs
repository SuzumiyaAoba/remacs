use remacs_runtime::{
    frontend::{GridTooltipPosition, render_grid},
    lisp::{Flow, Interp},
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
    assert_eq!(grid.tooltip.as_deref(), Some("positioned"));
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
    assert_eq!(grid.tooltip.as_deref(), Some("pointer"));
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
    assert_eq!(render_grid(&interp, 80, 24).tooltip.as_deref(), Some("short"));
    eval(&mut interp, "(sleep-for 0.05)");
    let grid = render_grid(&interp, 80, 24);
    assert_eq!(grid.tooltip, None);
    assert_eq!(grid.tooltip_position, None);
}
