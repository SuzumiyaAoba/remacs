//! Font-family list dispatch between the runtime and a GUI font catalog.
mod common;

use remacs::frontend::FontFamilyListBackend;
use std::rc::Rc;

struct TestFontFamilies;

impl FontFamilyListBackend for TestFontFamilies {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(vec!["Menlo".to_owned(), "Noto Sans".to_owned()])
    }
}

#[test]
fn font_family_list_uses_the_backend_for_graphical_frames() {
    common::on_big_stack(|| {
        let (mut interp, _) = common::interp();
        interp.set_font_family_list_backend(Some(Rc::new(TestFontFamilies)));
        let value = interp
            .eval_str(
                r#"(let ((tty-families (font-family-list)))
                    (set-frame-parameter nil 'window-system 'ns)
                    (list tty-families
                          (font-family-list)
                          (font-family-list (selected-frame))))"#,
            )
            .unwrap();
        assert_eq!(
            interp.prin1_to_string(&value).to_unicode_lossy(),
            "(nil (\"Menlo\" \"Noto Sans\") (\"Menlo\" \"Noto Sans\"))"
        );
    });
}
