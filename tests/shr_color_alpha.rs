mod common;

#[test]
fn shr_color_preserves_alpha_and_composites_transparent_foregrounds() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr)
                 (list
                  (shr-color->hexadecimal "rgba(255, 0, 0, 1)")
                  (shr-color->hexadecimal "rgba(255, 0, 0, 0.5)")
                  (shr-color->hexadecimal "rgba(255, 0, 0, 50%)")
                  (shr-color->hexadecimal "rgba(255, 0, 0, 0)")
                  (shr-color->hexadecimal "hsla(120, 100%, 50%, 0)")
                  (shr-color->hexadecimal "transparent")
                  (shr-color->hexadecimal "#f008")
                  (shr-color->hexadecimal "#ff000080")
                  (shr-color-check "rgba(255, 0, 0, 0.5)" "#FFFFFF")
                  (shr-color-check "transparent" "#FFFFFF")))"#
        ),
        "(\"#FF0000FF\" \"#FF000080\" \"#FF000080\" \"#FF000000\" \"#00FF0000\" \"#00000000\" \"#FF000088\" \"#FF000080\" (\"#FFFFFF\" \"#ff8080\") (\"#FFFFFF\" \"#FFFFFF\"))"
    );
}
