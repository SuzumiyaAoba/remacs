mod common;

#[test]
fn shr_color_wraps_hsl_hue_at_full_turns() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (shr-color->hexadecimal "hsl(0, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(360, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(720, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(-360, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(-720, 100%, 50%)")
                  (shr-color->hexadecimal "hsl(540, 100%, 50%)")))"#
        ),
        "(\"#ff0000\" \"#ff0000\" \"#ff0000\" \"#ff0000\" \"#ff0000\" \"#00ffff\")"
    );
}
