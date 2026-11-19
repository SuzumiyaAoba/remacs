mod common;

#[test]
fn shr_color_css_function_names_ignore_case_fold_search() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'shr-color)
                 (list
                  (let ((case-fold-search t))
                    (shr-color->hexadecimal "RGB(255,0,0)"))
                  (let ((case-fold-search nil))
                    (shr-color->hexadecimal "RGB(255,0,0)"))
                  (let ((case-fold-search nil))
                    (shr-color->hexadecimal "HsL(120,100%,50%)"))))"#
        ),
        "(\"#FF0000\" \"#FF0000\" \"#00ff00\")"
    );
}
