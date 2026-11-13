use remacs_runtime::{frontend::ClipboardBackend, lisp::Interp};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct ClipboardState {
    text: Option<String>,
    change_count: u64,
}

#[derive(Default)]
struct TestClipboard(RefCell<ClipboardState>);

impl TestClipboard {
    fn external_write(&self, text: &str) {
        let mut state = self.0.borrow_mut();
        state.text = Some(text.to_owned());
        state.change_count += 1;
    }
}

impl ClipboardBackend for TestClipboard {
    fn read_text(&self) -> Result<Option<String>, String> {
        Ok(self.0.borrow().text.clone())
    }

    fn write_text(&self, text: String) -> Result<(), String> {
        let mut state = self.0.borrow_mut();
        state.text = Some(text);
        state.change_count += 1;
        Ok(())
    }

    fn clipboard_change_count(&self) -> Result<Option<u64>, String> {
        Ok(Some(self.0.borrow().change_count))
    }

    fn write_text_with_change_count(&self, text: String) -> Result<Option<u64>, String> {
        let mut state = self.0.borrow_mut();
        state.text = Some(text);
        state.change_count += 1;
        Ok(Some(state.change_count))
    }

    fn clear_text_if_change_count(&self, expected: u64) -> Result<(), String> {
        let mut state = self.0.borrow_mut();
        if state.change_count == expected {
            state.text = Some(String::new());
            state.change_count += 1;
        }
        Ok(())
    }
}

fn expect(interp: &mut Interp, source: &str, expected: &str) {
    let value = interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
    assert_eq!(interp.prin1_to_string(&value).to_unicode_lossy(), expected);
}

#[test]
fn clipboard_ownership_uses_change_count_and_disowns_conditionally() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let host = Rc::new(TestClipboard::default());
            interp.set_clipboard_backend(Some(host.clone()));

            expect(
                &mut interp,
                "(ns-own-selection-internal 'CLIPBOARD \"same text\")",
                "\"same text\"",
            );
            expect(&mut interp, "(ns-selection-owner-p 'CLIPBOARD)", "t");

            host.external_write("same text");
            expect(&mut interp, "(ns-selection-owner-p 'CLIPBOARD)", "nil");
            expect(&mut interp, "(ns-disown-selection-internal 'CLIPBOARD)", "nil");
            let state = host.0.borrow();
            assert_eq!(state.text.as_deref(), Some("same text"));
            assert_eq!(state.change_count, 2);
            drop(state);

            expect(
                &mut interp,
                "(ns-own-selection-internal 'CLIPBOARD \"still mine\")",
                "\"still mine\"",
            );
            expect(&mut interp, "(ns-selection-owner-p 'CLIPBOARD)", "t");
            expect(&mut interp, "(ns-disown-selection-internal 'CLIPBOARD)", "nil");
            assert_eq!(host.0.borrow().text.as_deref(), Some(""));
        })
        .unwrap()
        .join()
        .unwrap();
}
