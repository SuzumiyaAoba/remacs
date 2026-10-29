use remacs_runtime::{frontend::ClipboardBackend, lisp::Interp};
use std::rc::Rc;

struct TestClipboard;

impl ClipboardBackend for TestClipboard {
    fn read_text(&self) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn write_text(&self, _text: String) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn display_selections_p_requires_a_graphical_frame_and_clipboard_backend() {
    let mut interp = Interp::new();
    interp
        .eval_str("(set-frame-parameter nil 'window-system 'ns)")
        .unwrap();
    let result = interp.eval_str("(display-selections-p)").unwrap();
    assert_eq!(interp.prin1_to_string(&result), "nil");

    let backend: Rc<dyn ClipboardBackend> = Rc::new(TestClipboard);
    interp.set_clipboard_backend(Some(backend));
    let result = interp.eval_str("(display-selections-p)").unwrap();
    assert_eq!(interp.prin1_to_string(&result), "t");

    interp
        .eval_str("(set-frame-parameter nil 'window-system nil)")
        .unwrap();
    let result = interp.eval_str("(display-selections-p)").unwrap();
    assert_eq!(interp.prin1_to_string(&result), "nil");
}
