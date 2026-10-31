use remacs_runtime::{frontend::ClipboardBackend, lisp::Interp};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct HostSelections {
    clipboard: RefCell<Option<String>>,
    primary: RefCell<Option<String>>,
    fail_primary: Cell<bool>,
}

impl ClipboardBackend for HostSelections {
    fn read_text(&self) -> Result<Option<String>, String> {
        Ok(self.clipboard.borrow().clone())
    }

    fn write_text(&self, text: String) -> Result<(), String> {
        *self.clipboard.borrow_mut() = Some(text);
        Ok(())
    }

    fn supports_primary_selection(&self) -> bool {
        true
    }

    fn read_primary_text(&self) -> Result<Option<String>, String> {
        if self.fail_primary.get() {
            return Err("primary service failed".to_owned());
        }
        Ok(self.primary.borrow().clone())
    }

    fn write_primary_text(&self, text: String) -> Result<(), String> {
        if self.fail_primary.get() {
            return Err("primary service failed".to_owned());
        }
        *self.primary.borrow_mut() = Some(text);
        Ok(())
    }
}

struct ClipboardOnly(Rc<HostSelections>);

impl ClipboardBackend for ClipboardOnly {
    fn read_text(&self) -> Result<Option<String>, String> {
        self.0.read_text()
    }
    fn write_text(&self, text: String) -> Result<(), String> {
        self.0.write_text(text)
    }
}

fn expect(interp: &mut Interp, source: &str, expected: &str) {
    let value = interp
        .eval_str(source)
        .unwrap_or_else(|flow| panic!("{source}: {flow:?}"));
    assert_eq!(
        interp.prin1_to_string(&value).to_unicode_lossy(),
        expected,
        "{source}"
    );
}

#[test]
fn primary_selection_reads_and_writes_the_host_independently_of_clipboard() {
    let mut interp = Interp::new();
    let host = Rc::new(HostSelections::default());
    *host.clipboard.borrow_mut() = Some("external clipboard".to_owned());
    *host.primary.borrow_mut() = Some("external primary".to_owned());
    interp.set_clipboard_backend(Some(host.clone()));
    expect(
        &mut interp,
        "(progn (setq window-system 'gpui initial-window-system 'gpui) (set-frame-parameter nil 'window-system 'gpui))",
        "nil",
    );
    expect(&mut interp, "(gui-get-selection)", "\"external primary\"");
    expect(
        &mut interp,
        "(gui-get-selection 'CLIPBOARD 'STRING)",
        "\"external clipboard\"",
    );
    expect(
        &mut interp,
        "(gui-backend-selection-exists-p 'PRIMARY)",
        "t",
    );
    expect(
        &mut interp,
        "(gui-backend-selection-owner-p 'PRIMARY)",
        "nil",
    );
    expect(
        &mut interp,
        "(progn (gui-set-selection 'PRIMARY \"日本語 primary\") (gui-backend-selection-owner-p 'PRIMARY))",
        "t",
    );
    assert_eq!(host.primary.borrow().as_deref(), Some("日本語 primary"));
    assert_eq!(
        host.clipboard.borrow().as_deref(),
        Some("external clipboard")
    );
    expect(
        &mut interp,
        "(progn (gui-set-selection 'CLIPBOARD \"own clipboard\") (gui-get-selection 'PRIMARY 'UTF8_STRING))",
        "\"日本語 primary\"",
    );
    assert_eq!(host.clipboard.borrow().as_deref(), Some("own clipboard"));

    *host.primary.borrow_mut() = Some("changed elsewhere".to_owned());
    expect(
        &mut interp,
        "(gui-get-selection 'PRIMARY 'TEXT)",
        "\"changed elsewhere\"",
    );
    expect(
        &mut interp,
        "(gui-backend-selection-owner-p 'PRIMARY)",
        "nil",
    );
    expect(
        &mut interp,
        "(progn (gui-set-selection 'PRIMARY nil) (gui-get-selection))",
        "\"changed elsewhere\"",
    );
    expect(
        &mut interp,
        "(progn (gui-set-selection 'PRIMARY \"owned\") (gui-set-selection 'PRIMARY nil) (gui-backend-selection-owner-p 'PRIMARY))",
        "nil",
    );
    assert_eq!(host.primary.borrow().as_deref(), Some(""));
    assert_eq!(host.clipboard.borrow().as_deref(), Some("own clipboard"));

    expect(
        &mut interp,
        "(progn (gui-set-selection 'PRIMARY \"old\") t)",
        "t",
    );
    *host.primary.borrow_mut() = None;
    expect(&mut interp, "(gui-get-selection)", "nil");
    expect(
        &mut interp,
        "(gui-backend-selection-exists-p (quote PRIMARY))",
        "nil",
    );
    expect(
        &mut interp,
        "(gui-backend-selection-owner-p (quote PRIMARY))",
        "nil",
    );
    expect(
        &mut interp,
        "(progn (ns-own-selection-internal nil \"nil primary\") (ns-get-selection nil 'STRING))",
        "\"nil primary\"",
    );
    assert_eq!(host.primary.borrow().as_deref(), Some("nil primary"));
    expect(
        &mut interp,
        "(progn (gui-set-selection 'SECONDARY \"secondary\") (gui-get-selection 'SECONDARY))",
        "\"secondary\"",
    );
    assert_eq!(host.primary.borrow().as_deref(), Some("nil primary"));

    host.fail_primary.set(true);
    expect(
        &mut interp,
        "(condition-case nil (gui-get-selection) (error 'failed))",
        "failed",
    );
    expect(
        &mut interp,
        "(condition-case nil (gui-set-selection 'PRIMARY \"failed write\") (error 'failed))",
        "failed",
    );
    host.fail_primary.set(false);
    assert_eq!(host.primary.borrow().as_deref(), Some("nil primary"));
    expect(&mut interp, "(gui-backend-selection-owner-p 'PRIMARY)", "t");

    // Existing clipboard-only adapters retain process-local PRIMARY behavior.
    interp.set_clipboard_backend(Some(Rc::new(ClipboardOnly(host.clone()))));
    expect(
        &mut interp,
        "(progn (gui-set-selection 'PRIMARY \"local only\") (gui-get-selection))",
        "\"local only\"",
    );
    expect(&mut interp, "(gui-backend-selection-owner-p 'PRIMARY)", "t");
    assert_eq!(host.primary.borrow().as_deref(), Some("nil primary"));
    expect(
        &mut interp,
        "(gui-get-selection 'CLIPBOARD)",
        "\"own clipboard\"",
    );
    // Loading select must retain the NS methods installed later in bootstrap.
    expect(
        &mut interp,
        "(progn (setq window-system 'ns) (set-frame-parameter nil 'window-system 'ns) (gui-get-selection 'CLIPBOARD))",
        "\"own clipboard\"",
    );
    expect(
        &mut interp,
        "(gui-get-selection 'PRIMARY)",
        "\"local only\"",
    );
    expect(
        &mut interp,
        "(progn (setq window-system nil) (set-frame-parameter nil 'window-system nil) (gui-get-selection 'CLIPBOARD))",
        "nil",
    );
}
