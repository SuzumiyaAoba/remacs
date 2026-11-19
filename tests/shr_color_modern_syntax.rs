mod common;

#[test]
fn shr_color_parses_modern_space_separated_rgb_and_hsl() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (shr-color->hexadecimal "hsl(120, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(120 100% 50%)")
                  (shr-color->hexadecimal "hsl(120 100 50)")
                  (shr-color->hexadecimal "rgb(255, 0, 0)")
                  (shr-color->hexadecimal "rgb(255 0 0)")
                  (shr-color->hexadecimal "rgb(50.5 0 0)")))"#
        ),
        "(\"#00ff00\" \"#00ff00\" \"#00ff00\" \"#FF0000\" \"#FF0000\" \"#330000\")"
    );
}
