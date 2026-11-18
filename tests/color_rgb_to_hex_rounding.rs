mod common;

#[test]
fn color_rgb_to_hex_rounds_halfway_channels_up() {
    assert_eq!(
        common::ev("(progn (require 'color) (color-rgb-to-hex 0.5 0.5 0.5 2))"),
        "\"#808080\""
    );
    assert_eq!(
        common::ev("(progn (require 'color) (color-rgb-to-hex 0.5 0.5 0.5))"),
        "\"#800080008000\""
    );
}
