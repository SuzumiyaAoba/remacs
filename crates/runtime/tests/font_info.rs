use std::{cell::RefCell, rc::Rc};

use remacs_runtime::frontend::{FontFamilyListBackend, FontInfo};
use remacs_runtime::lisp::{Interp, Value};

#[derive(Default)]
struct FontBackend {
    info_queries: RefCell<Vec<(usize, String)>>,
}

impl FontFamilyListBackend for FontBackend {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(vec!["Example Mono".to_owned()])
    }

    fn font_info(&self, frame_id: usize, name: &str) -> Result<Option<FontInfo>, String> {
        self.info_queries
            .borrow_mut()
            .push((frame_id, name.to_owned()));
        if name != "Example Mono" {
            return Ok(None);
        }
        Ok(Some(FontInfo {
            opened_name: name.to_owned(),
            full_name: "Example Mono Regular".to_owned(),
            size: 14,
            height: 16,
            baseline_offset: 0,
            relative_compose: 0,
            default_ascent: 12,
            max_width: 8,
            ascent: 12,
            descent: 4,
            space_width: 4,
            average_width: 8,
            filename: None,
        }))
    }
}

#[test]
fn font_info_uses_the_graphical_backend_and_returns_gnu_vector_shape() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let backend = Rc::new(FontBackend::default());
            interp.set_font_family_list_backend(Some(backend.clone()));

            let tty_result = interp
                .eval_str(
                    "(condition-case error\
                         (font-info \"Example Mono\" (selected-frame))\
                       (error error))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&tty_result),
                "(error \"Window system frame should be used\")"
            );

            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();
            let info = interp
                .eval_str("(font-info \"Example Mono\" (selected-frame))")
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&info),
                "[\"Example Mono\" \"Example Mono Regular\" 14 16 0 0 12 8 12 4 4 8 nil nil]"
            );
            let missing = interp.eval_str("(font-info \"Missing Family\")").unwrap();
            assert!(matches!(missing, Value::Nil));

            let queries = backend.info_queries.borrow();
            assert_eq!(queries.len(), 2);
            assert_eq!(queries[0].1, "Example Mono");
            assert_eq!(queries[1].1, "Missing Family");
            assert_ne!(queries[0].0, 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
