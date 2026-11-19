mod common;

#[test]
fn java_imenu_matches_deeply_nested_generic_return_types() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'cc-menus)
                 (let ((regexp (cadar cc-imenu-java-generic-expression)))
                   (list
                    (and (string-match regexp
                                       "List<List<List<String>>> method() {")
                         (match-string 5
                                       "List<List<List<String>>> method() {"))
                    (and (string-match regexp
                                       "List<List<List<List<String>>>> method() {")
                         (match-string 5
                                       "List<List<List<List<String>>>> method() {"))
                    (let ((type "String"))
                      (dotimes (_ 8)
                        (setq type (concat "List<" type ">")))
                      (let ((line (concat type " method() {")))
                        (and (string-match regexp line)
                             (match-string 5 line)))))))"#
        ),
        "(\"method\" \"method\" \"method\")"
    );
}
