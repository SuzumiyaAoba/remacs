use remacs_runtime::{
    frontend::{ClipboardBackend, ClipboardInfo},
    lisp::Interp,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct ClipboardState {
    text: Option<String>,
    change_count: u64,
    info: Option<ClipboardInfo>,
    change_count_available: bool,
}

#[derive(Default)]
struct TestClipboard(RefCell<ClipboardState>);

impl TestClipboard {
    fn enable_change_count(&self) {
        self.0.borrow_mut().change_count_available = true;
    }

    fn clear(&self) {
        let mut state = self.0.borrow_mut();
        state.text = None;
        state.change_count += 1;
        state.info = Some(ClipboardInfo {
            has_data: false,
            targets: Vec::new(),
        });
    }

    fn external_write(&self, text: &str) {
        let mut state = self.0.borrow_mut();
        state.text = Some(text.to_owned());
        state.change_count += 1;
        state.info = Some(ClipboardInfo {
            has_data: true,
            targets: vec!["STRING".to_owned()],
        });
    }

    fn external_set_info(&self, has_data: bool, targets: Vec<String>) {
        let mut state = self.0.borrow_mut();
        state.text = None;
        state.change_count += 1;
        state.info = Some(ClipboardInfo { has_data, targets });
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
        state.info = Some(ClipboardInfo {
            has_data: true,
            targets: vec!["STRING".to_owned()],
        });
        Ok(())
    }

    fn clear_text(&self) -> Result<(), String> {
        self.clear();
        Ok(())
    }

    fn clipboard_info(&self) -> Result<Option<ClipboardInfo>, String> {
        Ok(self.0.borrow().info.clone())
    }

    fn clipboard_change_count(&self) -> Result<Option<u64>, String> {
        let state = self.0.borrow();
        Ok(state
            .change_count_available
            .then_some(state.change_count))
    }

    fn write_text_with_change_count(&self, text: String) -> Result<Option<u64>, String> {
        self.write_text(text)?;
        self.clipboard_change_count()
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
            host.enable_change_count();
            interp.set_clipboard_backend(Some(host.clone()));
            expect(
                &mut interp,
                "(progn (setq window-system 'ns initial-window-system 'ns) (set-frame-parameter nil 'window-system 'ns))",
                "nil",
            );

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
            assert_eq!(host.0.borrow().text, None);
            assert!(!host.0.borrow().info.as_ref().unwrap().has_data);

            expect(
                &mut interp,
                "(progn (ns-own-selection-internal 'CLIPBOARD (propertize \"probe\" 'qa-selection-property t)) t)",
                "t",
            );
            expect(
                &mut interp,
                "(get-text-property 0 'qa-selection-property (gui-get-selection 'CLIPBOARD 'STRING))",
                "t",
            );
            expect(
                &mut interp,
                "(ns-own-selection-internal 'PRIMARY 'qa-local-selection)",
                "qa-local-selection",
            );
            expect(
                &mut interp,
                "(gui-get-selection 'PRIMARY 'STRING)",
                "qa-local-selection",
            );

            host.external_write("external clipboard");
            expect(&mut interp, "(ns-selection-owner-p 'CLIPBOARD)", "nil");
            expect(
                &mut interp,
                "(gui-get-selection 'CLIPBOARD 'STRING)",
                "\"external clipboard\"",
            );

            host.external_set_info(true, vec!["image/png".to_owned()]);
            expect(
                &mut interp,
                "(equal (gui-get-selection 'CLIPBOARD 'TARGETS) [TARGETS image/png])",
                "t",
            );
            expect(
                &mut interp,
                "(gui-backend-selection-exists-p 'CLIPBOARD)",
                "t",
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn clipboard_disown_without_change_count_clears_the_owned_payload() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let host = Rc::new(TestClipboard::default());
            interp.set_clipboard_backend(Some(host.clone()));
            expect(
                &mut interp,
                "(progn (setq window-system 'ns initial-window-system 'ns) (set-frame-parameter nil 'window-system 'ns))",
                "nil",
            );

            expect(
                &mut interp,
                "(ns-own-selection-internal 'CLIPBOARD \"owned without count\")",
                "\"owned without count\"",
            );
            expect(&mut interp, "(ns-selection-owner-p 'CLIPBOARD)", "t");
            expect(&mut interp, "(ns-disown-selection-internal 'CLIPBOARD)", "nil");
            assert_eq!(host.0.borrow().text, None);
            assert!(!host.0.borrow().info.as_ref().unwrap().has_data);
        })
        .unwrap()
        .join()
        .unwrap();
}
