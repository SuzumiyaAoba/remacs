use remacs_runtime::{
    frontend::{Grid, render_grid},
    lisp::{Interp, Value},
};

#[test]
fn only_the_dynamic_dragging_value_freezes_pointer_shape() {
    let mut i = Interp::new();
    assert!(!Grid::blank(20, 6).freeze_pointer_shape);
    for (value, frozen) in [
        ("nil", false),
        ("t", false),
        ("'dragging", true),
        ("(make-symbol \"dragging\")", false),
        ("'dropping", false),
        ("0", false),
    ] {
        i.eval_str(&format!("(setq track-mouse {value})")).unwrap();
        assert_eq!(
            render_grid(&i, 20, 6).freeze_pointer_shape,
            frozen,
            "{value}"
        );
    }
    i.eval_str("(setq track-mouse nil)").unwrap();
    let track_mouse = i.intern("track-mouse");
    let dragging = Value::Sym(i.intern("dragging"));
    let mark = i.specbind_depth();
    i.specbind(track_mouse, dragging).unwrap();
    assert!(render_grid(&i, 20, 6).freeze_pointer_shape);
    let inner = i.specbind_depth();
    i.specbind(track_mouse, Value::t()).unwrap();
    assert!(!render_grid(&i, 20, 6).freeze_pointer_shape);
    i.unbind_to(inner).unwrap();
    assert!(render_grid(&i, 20, 6).freeze_pointer_shape);
    i.unbind_to(mark).unwrap();
    assert!(!render_grid(&i, 20, 6).freeze_pointer_shape);

    i.eval_str("(setq track-mouse 'dragging)").unwrap();
    i.selected_frame = None;
    assert!(render_grid(&i, 20, 6).freeze_pointer_shape);
}
