use std::{rc::Rc, thread};

use remacs_runtime::{frontend::FontFamilyListBackend, lisp::Interp};

struct FontBackend;

impl FontFamilyListBackend for FontBackend {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(vec!["Example Mono".to_owned(), "Other Family".to_owned()])
    }
}

fn assert_font_entity(interp: &Interp, font: &remacs_runtime::lisp::Value, family: &str) {
    let remacs_runtime::lisp::Value::Record(font) = font else {
        panic!("expected a font-entity record");
    };
    let font = font.borrow();
    assert_eq!(font.len(), 11);
    let remacs_runtime::lisp::Value::Sym(tag) = &font[0] else {
        panic!("font entity has no type tag");
    };
    assert_eq!(interp.symbol_name(*tag), "font-entity");
    let remacs_runtime::lisp::Value::Str(actual_family) = &font[3] else {
        panic!("font entity has no family");
    };
    assert_eq!(actual_family.borrow().to_unicode_lossy(), family);
}

#[test]
fn find_font_and_list_fonts_return_entities_for_available_families() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.set_font_family_list_backend(Some(Rc::new(FontBackend)));
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();

            let found = interp
                .eval_str("(find-font (font-spec :family \"Example Mono\"))")
                .unwrap();
            assert_font_entity(&interp, &found, "Example Mono");
            let found_p = interp
                .eval_str("(fontp (find-font (font-spec :family \"Example Mono\")) 'font-entity)")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&found_p), "t");

            let listed = interp
                .eval_str("(list-fonts (font-spec :family \"Example Mono\"))")
                .unwrap();
            let listed = listed.list_to_vec().unwrap();
            assert_eq!(listed.len(), 1);
            assert_font_entity(&interp, &listed[0], "Example Mono");

            let limited = interp
                .eval_str("(list-fonts (font-spec) (selected-frame) nil 1)")
                .unwrap();
            assert_eq!(limited.list_to_vec().unwrap().len(), 1);

            let missing = interp
                .eval_str("(find-font (font-spec :family \"Missing Family\"))")
                .unwrap();
            assert!(matches!(missing, remacs_runtime::lisp::Value::Nil));
        })
        .unwrap()
        .join()
        .unwrap();
}
