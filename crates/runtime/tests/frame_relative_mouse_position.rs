use remacs_runtime::frontend::frame_relative_mouse_position;

#[test]
fn mouse_coordinates_are_measured_from_content_origin() {
    assert_eq!(
        frame_relative_mouse_position((110, 250), (100, 200), 30, (10, 10)),
        ((10, 20), (1, 2))
    );
}
