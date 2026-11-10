use remacs_runtime::lisp::Interp;

fn eval(i: &mut Interp, source: &str) -> String {
    let value = i.eval_str(source).unwrap_or_else(|flow| panic!("{flow:?}"));
    i.prin1_to_string(&value).to_unicode_lossy()
}

#[test]
fn gui_reports_face_attributes_the_renderer_can_draw() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(|| {
            let mut i = Interp::new();
            eval(&mut i, "(set-frame-parameter nil 'window-system 'ns)");

            assert_eq!(
                eval(&mut i, "(display-supports-face-attributes-p nil)"),
                "t"
            );
            for expression in [
                "(display-supports-face-attributes-p '(:foreground \"red\"))",
                "(display-supports-face-attributes-p '(:background \"blue\"))",
                "(display-supports-face-attributes-p '(:weight bold))",
                "(display-supports-face-attributes-p '(:family \"Menlo\"))",
                "(display-supports-face-attributes-p '(:width condensed))",
                "(display-supports-face-attributes-p '(:height 180))",
                "(display-supports-face-attributes-p '(:slant italic))",
                "(display-supports-face-attributes-p '(:underline t) (selected-frame))",
                "(display-supports-face-attributes-p '(:strike-through t))",
            ] {
                assert_eq!(eval(&mut i, expression), "t", "{expression}");
            }

            for expression in [
                "(display-supports-face-attributes-p '(:foundry \"Adobe\"))",
                "(display-supports-face-attributes-p '(:foreground \"not-a-color\"))",
            ] {
                assert_eq!(eval(&mut i, expression), "nil", "{expression}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
