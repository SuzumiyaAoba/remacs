use std::{cell::RefCell, rc::Rc};

use remacs_runtime::frontend::FontFamilyListBackend;
use remacs_runtime::lisp::{Interp, Value};

#[derive(Default)]
struct FontBackend {
    reconsidered_frames: RefCell<Vec<usize>>,
}

impl FontFamilyListBackend for FontBackend {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(vec!["Example Mono".to_owned()])
    }

    fn reconsider_frame_fonts(&self, frame_id: usize) -> Result<(), String> {
        self.reconsidered_frames.borrow_mut().push(frame_id);
        Ok(())
    }
}

#[test]
fn graphical_reconsideration_refreshes_the_host_and_reapplies_font_parameter() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let backend = Rc::new(FontBackend::default());
            interp.set_font_family_list_backend(Some(backend.clone()));

            let tty_result = interp
                .eval_str(
                    "(condition-case error\
                         (reconsider-frame-fonts (selected-frame))\
                       (error error))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&tty_result),
                "(error \"Window system frame should be used\")"
            );

            interp
                .eval_str(
                    "(set-frame-parameter nil 'window-system 'gpui)\
                     (set-frame-parameter nil 'font-parameter \"Example Mono\")",
                )
                .unwrap();
            let result = interp
                .eval_str("(condition-case error (reconsider-frame-fonts nil) (error error))")
                .unwrap();
            assert!(
                matches!(result, Value::Nil),
                "unexpected reconsider-frame-fonts result: {}",
                interp.prin1_to_string(&result)
            );

            let frames = backend.reconsidered_frames.borrow();
            assert_eq!(frames.len(), 1);
            assert_ne!(frames[0], 0);
            let font_parameter = interp
                .eval_str("(frame-parameter nil 'font-parameter)")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&font_parameter), "\"Example Mono\"");
            let default_face_font = interp
                .eval_str("(face-attribute 'default :font nil)")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&default_face_font), "\"Example Mono\"");
        })
        .unwrap()
        .join()
        .unwrap();
}
