use std::{rc::Rc, thread};

use remacs_runtime::{
    frontend::{FontFamilyListBackend, render_grid},
    lisp::Interp,
};

struct FontBackend;

impl FontFamilyListBackend for FontBackend {
    fn list_families(&self, _frame_id: usize) -> Result<Vec<String>, String> {
        Ok(vec!["Menlo".to_owned()])
    }
}

#[test]
fn set_frame_font_applies_family_and_point_size_to_graphical_default_face() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp.set_font_family_list_backend(Some(Rc::new(FontBackend)));

            let parameter = interp
                .eval_str(
                    "(set-frame-parameter nil 'window-system 'gpui)\
                     (set-frame-font \"Menlo-20\" nil (list (selected-frame)))\
                     (frame-parameter nil 'font)",
                )
                .unwrap();
            assert_eq!(interp.prin1_to_string(&parameter), "\"Menlo-20\"");

            let grid = render_grid(&interp, 80, 25);
            assert_eq!(grid.default_style.font_height, Some(200));
            let family = grid
                .default_style
                .font_family
                .and_then(|index| grid.font_families.get(usize::from(index)));
            assert_eq!(family.map(String::as_str), Some("Menlo"));
        })
        .unwrap()
        .join()
        .unwrap();
}
