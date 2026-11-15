use std::{cell::RefCell, rc::Rc};

use remacs_runtime::frontend::{GridImageSource, ImageBackend, ImageInfo};
use remacs_runtime::lisp::{Interp, Value};

#[derive(Default)]
struct Backend {
    infos: RefCell<Vec<(usize, GridImageSource, Option<usize>)>>,
    flushes: RefCell<Vec<(Vec<usize>, GridImageSource)>>,
    clears: RefCell<Vec<(Option<Vec<usize>>, Option<String>)>>,
}

impl ImageBackend for Backend {
    fn image_info(
        &self,
        frame_id: usize,
        source: GridImageSource,
        index: Option<usize>,
    ) -> Result<Option<ImageInfo>, String> {
        self.infos.borrow_mut().push((frame_id, source, index));
        Ok(Some(ImageInfo {
            width: 10,
            height: 6,
            has_mask: true,
            frame_count: 3,
            frame_delay: Some(0.1),
        }))
    }

    fn flush_image(&self, frame_ids: Vec<usize>, source: GridImageSource) -> Result<(), String> {
        self.flushes.borrow_mut().push((frame_ids, source));
        Ok(())
    }

    fn clear_image_cache(
        &self,
        frame_ids: Option<Vec<usize>>,
        filename: Option<String>,
    ) -> Result<(), String> {
        self.clears.borrow_mut().push((frame_ids, filename));
        Ok(())
    }
}

#[test]
fn graphical_image_queries_and_cache_operations_use_the_host_backend() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let backend = Rc::new(Backend::default());
            interp.set_image_backend(Some(backend.clone()));
            let spec = "'(image :type png :data \"abc\")";

            let tty_result = interp
                .eval_str(&format!(
                    "(condition-case error (image-size {spec} t) (error error))"
                ))
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&tty_result),
                "(error \"Window system frame should be used\")"
            );

            interp
                .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                .unwrap();
            let pixel_size = interp.eval_str(&format!("(image-size {spec} t)")).unwrap();
            assert_eq!(interp.prin1_to_string(&pixel_size), "(10 . 6)");
            let character_size = interp
                .eval_str(&format!(
                    "(let ((size (image-size {spec})))\
                       (and (floatp (car size)) (floatp (cdr size))))"
                ))
                .unwrap();
            assert_eq!(interp.prin1_to_string(&character_size), "t");
            let mask = interp.eval_str(&format!("(image-mask-p {spec})")).unwrap();
            assert_eq!(interp.prin1_to_string(&mask), "t");
            let metadata = interp
                .eval_str(&format!("(image-metadata {spec})"))
                .unwrap();
            assert_eq!(interp.prin1_to_string(&metadata), "(delay 0.1 count 3)");
            assert!(matches!(
                interp.eval_str(&format!("(image-flush {spec} t)")).unwrap(),
                Value::Nil
            ));
            assert!(matches!(
                interp.eval_str("(clear-image-cache nil)").unwrap(),
                Value::Nil
            ));
            assert!(matches!(
                interp
                    .eval_str("(clear-image-cache \"sample.png\")")
                    .unwrap(),
                Value::Nil
            ));

            let infos = backend.infos.borrow();
            assert_eq!(infos.len(), 4);
            assert!(infos.iter().all(|(frame_id, _, _)| *frame_id != 0));
            assert!(matches!(
                &infos[0].1,
                GridImageSource::Data { format, .. } if format == "png"
            ));
            assert_eq!(backend.flushes.borrow().len(), 1);
            assert_eq!(
                backend.clears.borrow().as_slice(),
                &[
                    (Some(vec![infos[0].0]), None),
                    (None, Some("sample.png".into()))
                ]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
