mod common;

#[test]
fn shr_color_clips_out_of_gamut_hsl_components() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (shr-color->hexadecimal "hsl(0, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(0, 150%, 50%)")
                  (shr-color->hexadecimal "hsl(0, 100%, 150%)")
                  (shr-color->hexadecimal "hsl(0, 100%, -50%)")))"#
        ),
        "(\"#ff0000\" \"#ff0000\" \"#ffffff\" \"#000000\")"
    );
}
