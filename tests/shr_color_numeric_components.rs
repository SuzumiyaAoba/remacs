mod common;

#[test]
fn shr_color_parses_fractional_and_negative_css_components() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (shr-color->hexadecimal "hsl(38.82, 50%, 50%)")
                  (shr-color->hexadecimal "hsl(38, 50.5%, 50%)")
                  (shr-color->hexadecimal "hsl(0, -25%, 50%)")
                  (shr-color->hexadecimal "rgb(50.5, 0, 0)")
                  (shr-color->hexadecimal "rgb(50.5%, 0%, 0%)")
                  (shr-color->hexadecimal "rgb(-1, 0, 0)")
                  (shr-color->hexadecimal "rgb(999, 0, 0)")
                  (stringp (shr-color->hexadecimal "rgba(50.5, 0, 0, 0.5)"))
                  (stringp (shr-color->hexadecimal "hsla(38.82, 50.5%, 50%, 0.5)"))))"#
        ),
        "(\"#bf9240\" \"#c0913f\" \"#808080\" \"#330000\" \"#810000\" \"#000000\" \"#FF0000\" t t)"
    );
}

#[test]
fn shr_color_clamps_rgb_channels_to_the_8_bit_range() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (shr-color->hexadecimal "rgb(999, 0, 0)")
                  (shr-color->hexadecimal "rgb(0, 999, 0)")
                  (shr-color->hexadecimal "rgb(0, 0, 999)")
                  (shr-color->hexadecimal "rgb(0, 0, -999)")))"#
        ),
        "(\"#FF0000\" \"#00FF00\" \"#0000FF\" \"#000000\")"
    );
}
