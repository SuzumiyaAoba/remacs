mod common;

#[test]
fn java_imenu_matches_methods_with_annotation_element_values() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'cc-menus)
                 (let ((regexp (cadar cc-imenu-java-generic-expression)))
                   (list
                    (and (string-match regexp "void method(String arg) {")
                         (match-string 7 "void method(String arg) {"))
                    (and (string-match
                          regexp "void method(@Anno(value=\"x\") String arg) {")
                         (match-string
                          7 "void method(@Anno(value=\"x\") String arg) {")))))"#
        ),
        "(\"method\" \"method\")"
    );
}
