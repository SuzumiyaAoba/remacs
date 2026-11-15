use std::{cell::RefCell, rc::Rc};

use remacs_runtime::frontend::{FontAtStyle, FontFamilyListBackend};
use remacs_runtime::lisp::{Flow, Interp};

#[derive(Default)]
struct FontBackend {
    requested: RefCell<Vec<FontAtStyle>>,
}

impl FontFamilyListBackend for FontBackend {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    fn resolve_font_at(
        &self,
        _frame_id: usize,
        style: &FontAtStyle,
    ) -> Result<Option<String>, String> {
        self.requested.borrow_mut().push(style.clone());
        Ok(style
            .family
            .as_ref()
            .map(|family| format!("Actual {family}")))
    }
}

#[test]
fn font_at_returns_the_resolved_face_font_for_buffer_and_string_positions() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let backend = Rc::new(FontBackend::default());
            interp.set_font_family_list_backend(Some(backend.clone()));

            let result = match interp.eval_str(
                    "(condition-case error\
                       (progn\
                     (set-frame-parameter nil 'window-system 'gpui)\
                     (make-face 'font-at-test-face)\
                     (set-face-attribute 'font-at-test-face nil\x20\
                                         :family \"Face Family\"\x20\
                                         :weight 'bold\x20\
                                         :width 'expanded\x20\
                                         :height 135)\
                     (let ((buffer (get-buffer-create \"*font-at-test*\")))\
                       (set-window-buffer (selected-window) buffer)\
                       (with-current-buffer buffer\
                         (erase-buffer)\
                         (insert \"a\")\
                         (put-text-property (point-min) (point-max)\
                                            'face 'font-at-test-face)\
                         (list\
                          (let ((font (font-at (point-min))))\
                            (list (fontp font 'font-object) font))\
                          (let* ((string (propertize \"a\"\
                                                     'face 'font-at-test-face))\
                                 (font (font-at 0 nil string)))\
                            (list (fontp font 'font-object) font))))))\
                       (error (list 'ERROR (error-message-string error))))",
                ) {
                Ok(result) => result,
                Err(Flow::Signal(signal)) => {
                    panic!("Lisp evaluation failed: {}", interp.prin1_to_string(&signal.condition))
                }
                Err(flow) => panic!("Lisp evaluation failed: {flow:?}"),
            };

            let font_results = result.list_to_vec().unwrap();
            assert_eq!(font_results.len(), 2);
            for result in font_results {
                let fields = result.list_to_vec().unwrap();
                assert_eq!(interp.prin1_to_string(&fields[0]), "t");
                let remacs_runtime::lisp::Value::Record(font) = &fields[1] else {
                    panic!("font-at did not return a record");
                };
                let font = font.borrow();
                assert_eq!(font.len(), 11);
                let remacs_runtime::lisp::Value::Sym(tag) = &font[0] else {
                    panic!("font record has no type tag");
                };
                assert_eq!(interp.symbol_name(*tag), "font-object");
                let remacs_runtime::lisp::Value::Str(family) = &font[3] else {
                    panic!("font record has no family");
                };
                assert_eq!(family.borrow().to_unicode_lossy(), "Actual Face Family");
                assert!(matches!(&font[6], remacs_runtime::lisp::Value::Int(179_200)));
                assert!(matches!(&font[7], remacs_runtime::lisp::Value::Sym(_)));
                assert!(matches!(&font[8], remacs_runtime::lisp::Value::Int(125)));
                assert!(matches!(&font[9], remacs_runtime::lisp::Value::Int(135)));
            }
            let requested = backend.requested.borrow();
            assert_eq!(requested.len(), 2);
            for style in requested.iter() {
                assert_eq!(style.family.as_deref(), Some("Face Family"));
                assert_eq!(style.weight, Some(700));
                assert_eq!(style.width, Some(125));
                assert_eq!(style.height, Some(135));
                assert!(style.bold);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
