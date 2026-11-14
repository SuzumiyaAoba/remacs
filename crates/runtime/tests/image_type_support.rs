use remacs_runtime::{
    frontend::{GridImageSource, prepare_native_menus, render_grid},
    lisp::Interp,
};

#[test]
fn image_type_availability_matches_the_gui_renderer() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut i = Interp::new();
            let available = i
                .eval_str(
                    "(mapcar #'image-type-available-p '(png gif jpeg webp bmp pbm tiff svg xpm xbm postscript heic))",
                )
                .unwrap();
            assert_eq!(
                i.prin1_to_string(&available).to_unicode_lossy(),
                "(t t t t t t t t nil nil nil nil)"
            );

            i.eval_str(
                r#"(progn
                     (set-frame-parameter nil 'window-system 'ns)
                     (set-frame-parameter nil 'menu-bar-lines 0)
                     (set-frame-parameter nil 'tool-bar-lines 1)
                     (set-frame-parameter nil 'tab-bar-lines 0)
                     (setq menu-bar-mode nil tool-bar-map (make-sparse-keymap))
                     (defun qa-pbm-image ()
                       '(image :type pbm :data "P1\n2 2\n0 1\n1 0\n"))
                     (define-key tool-bar-map [pbm]
                       '(menu-item "PBM" ignore :image (qa-pbm-image))))"#,
            )
            .unwrap();
            prepare_native_menus(&mut i).unwrap();

            let grid = render_grid(&i, 60, 12);
            assert_eq!(grid.images.len(), 1);
            assert!(matches!(
                grid.images[0].source,
                GridImageSource::Data { ref format, .. } if format == "pbm"
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
