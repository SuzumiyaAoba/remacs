use remacs_runtime::lisp::{Flow, Interp};

fn handwrite_rejects_unmapped_unicode_before_printing() {
    let mut interp = Interp::new();
    let result = interp.eval_str(
        r#"(progn
             (require 'handwrite)
             (let ((handwrite-numlines 100000)
                   (handwrite-pagenumbering nil))
               (fset 'y-or-n-p (lambda (&rest _args) nil))
               (with-temp-buffer
                 (insert "Hello あ 😀")
                 (handwrite))))"#,
    );
    let Err(Flow::Signal(signal)) = result else {
        panic!("expected handwrite to reject an unmapped character, got {result:?}");
    };
    let condition = interp
        .prin1_to_string(&signal.condition)
        .to_unicode_lossy();
    assert!(
        condition.contains("handwrite cannot encode character U+3042"),
        "unexpected handwrite error: {condition}"
    );
}

#[test]
fn unsupported_unicode_does_not_produce_silent_postscript() {
    std::thread::Builder::new()
        .name("handwrite-unsupported-character".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(handwrite_rejects_unmapped_unicode_before_printing)
        .unwrap()
        .join()
        .expect("handwrite unsupported-character test thread panicked");
}
