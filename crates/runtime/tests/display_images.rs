use std::{rc::Rc, thread};

use remacs_runtime::{frontend::ImageBackend, lisp::Interp};

struct ImageHost;

impl ImageBackend for ImageHost {}

#[test]
fn display_images_p_tracks_installed_graphical_image_support() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();

            let unsupported = interp.eval_str("(display-images-p)").unwrap();
            assert_eq!(interp.prin1_to_string(&unsupported), "nil");
            let flag = interp
                .eval_str("remacs--image-rendering-supported")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&flag), "nil");

            interp.set_image_backend(Some(Rc::new(ImageHost)));
            let supported = interp.eval_str("(display-images-p)").unwrap();
            assert_eq!(interp.prin1_to_string(&supported), "t");
            let flag = interp
                .eval_str("remacs--image-rendering-supported")
                .unwrap();
            assert_eq!(interp.prin1_to_string(&flag), "t");

            interp.set_image_backend(None);
            let unsupported = interp.eval_str("(display-images-p)").unwrap();
            assert_eq!(interp.prin1_to_string(&unsupported), "nil");
        })
        .unwrap()
        .join()
        .unwrap();
}
