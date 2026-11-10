use remacs_runtime::lisp::{Flow, Interp};

fn eval(i: &mut Interp, source: &str) -> String {
    let value = i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            i.prin1_to_string(&signal.symbol()).to_unicode_lossy(),
            i.prin1_to_string(&signal.data()).to_unicode_lossy()
        ),
        other => panic!("{other:?}"),
    });
    i.prin1_to_string(&value).to_unicode_lossy()
}

#[test]
fn ns_display_mm_dimensions_use_monitor_mm_size() {
    std::thread::Builder::new()
        .stack_size(128 << 20)
        .spawn(|| {
            let mut i = Interp::new();
            let dimensions = eval(
                &mut i,
                r#"(let ((frame (selected-frame)))
                     (set-frame-parameter frame 'window-system 'ns)
                     (set-frame-parameter
                      frame 'display-monitor-attributes-list
                      (list (list (list 'mm-size 530 300)
                                  (list 'frames (list frame)))))
                     (list
                      (cadr (assq 'mm-size (frame-monitor-attributes frame)))
                      (caddr (assq 'mm-size (frame-monitor-attributes frame)))
                      (x-display-mm-width frame)
                      (x-display-mm-height frame)))"#,
            );
            assert_eq!(dimensions, "(530 300 530 300)");

            let unavailable = eval(
                &mut i,
                r#"(let ((frame (selected-frame)))
                     (set-frame-parameter
                      frame 'display-monitor-attributes-list
                      (list (list (list 'mm-size nil nil)
                                  (list 'frames (list frame)))))
                     (list (x-display-mm-width frame)
                           (x-display-mm-height frame)))"#,
            );
            assert_eq!(unavailable, "(nil nil)");
        })
        .unwrap()
        .join()
        .unwrap();
}
