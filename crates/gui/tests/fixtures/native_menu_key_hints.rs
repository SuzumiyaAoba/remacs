//! Native NSMenu keyboard columns. Run on the main thread under the watchdog.
use cocoa::{
    base::{BOOL, NO, id, nil},
    foundation::NSRange,
};
use gpui::{Application, KeyBinding, Menu, MenuItem, Timer};
use objc::{class, msg_send, sel, sel_impl};
use std::{
    ffi::{CStr, c_void},
    ptr,
    time::Duration,
};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = menu_hint_fixture, no_json)]
struct Run;

fn menus(hint: &str) -> Vec<Menu> {
    vec![
        Menu {
            name: "App".into(),
            items: Vec::new(),
        },
        Menu {
            name: "QA Keys".into(),
            items: vec![
                MenuItem::action("Run", Run)
                    .checked(true)
                    .key_hint(hint.to_owned()),
                MenuItem::disabled("A much longer disabled item")
                    .checked(true)
                    .key_hint("RET"),
                MenuItem::action("Plain", Run),
                MenuItem::submenu(Menu {
                    name: "Nested".into(),
                    items: vec![MenuItem::action("Left", Run).key_hint("<left>")],
                })
                .key_hint("C-x 4"),
                MenuItem::action("Empty", Run).key_hint(""),
                MenuItem::separator(),
            ],
        },
    ]
}

unsafe fn text(value: id) -> String {
    if value == nil {
        return String::new();
    }
    unsafe {
        let bytes: *const std::ffi::c_char = msg_send![value, UTF8String];
        if bytes.is_null() {
            String::new()
        } else {
            CStr::from_ptr(bytes).to_string_lossy().into_owned()
        }
    }
}

unsafe fn column(item: id) -> Option<(String, isize, f64)> {
    unsafe {
        let attributed: id = msg_send![item, attributedTitle];
        if attributed == nil {
            return None;
        }
        let title: id = msg_send![attributed, string];
        let string = text(title);
        if string.is_empty() {
            return None;
        }
        let key: id =
            msg_send![class!(NSString), stringWithUTF8String: c"NSParagraphStyle".as_ptr()];
        let style: id = msg_send![attributed, attribute: key atIndex: 0usize effectiveRange: ptr::null_mut::<NSRange>()];
        if style == nil {
            return None;
        }
        let tabs: id = msg_send![style, tabStops];
        let count: usize = msg_send![tabs, count];
        if count != 1 {
            return None;
        }
        let tab: id = msg_send![tabs, objectAtIndex: 0usize];
        let alignment: isize = msg_send![tab, alignment];
        let location: f64 = msg_send![tab, location];
        Some((string, alignment, location))
    }
}

unsafe fn check(hint: &str) -> bool {
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let main: id = msg_send![app, mainMenu];
        let root: id = msg_send![main, itemAtIndex: 1usize];
        let menu: id = msg_send![root, submenu];
        let _: () = msg_send![menu, update];
        let run: id = msg_send![menu, itemAtIndex: 0usize];
        let disabled: id = msg_send![menu, itemAtIndex: 1usize];
        let plain: id = msg_send![menu, itemAtIndex: 2usize];
        let branch: id = msg_send![menu, itemAtIndex: 3usize];
        let nested: id = msg_send![branch, submenu];
        let left: id = msg_send![nested, itemAtIndex: 0usize];
        let empty: id = msg_send![menu, itemAtIndex: 4usize];
        let separator: id = msg_send![menu, itemAtIndex: 5usize];
        let run_column = column(run);
        let disabled_column = column(disabled);
        let left_column = column(left);
        let branch_column = column(branch);
        let empty_column = column(empty);
        println!(
            "run={run_column:?} disabled={disabled_column:?} branch={branch_column:?} nested={left_column:?} empty={empty_column:?}"
        );
        let Some((run_title, run_alignment, run_location)) = run_column else {
            return false;
        };
        let Some((disabled_title, disabled_alignment, disabled_location)) = disabled_column else {
            return false;
        };
        let Some((left_title, left_alignment, left_location)) = left_column else {
            return false;
        };
        let Some((branch_title, branch_alignment, branch_location)) = branch_column else {
            return false;
        };
        let Some((empty_title, _, empty_location)) = empty_column else {
            return false;
        };
        let run_equivalent: id = msg_send![run, keyEquivalent];
        let plain_equivalent: id = msg_send![plain, keyEquivalent];
        let plain_title: id = msg_send![plain, title];
        let action: *const c_void = msg_send![run, action];
        let disabled_action: *const c_void = msg_send![disabled, action];
        let disabled_enabled: BOOL = msg_send![disabled, isEnabled];
        let state: isize = msg_send![run, state];
        let separator: BOOL = msg_send![separator, isSeparatorItem];
        println!(
            "native accelerators: hinted={:?} plain={:?}",
            text(run_equivalent),
            text(plain_equivalent)
        );
        run_title == format!("Run\t{hint}")
            && disabled_title == "A much longer disabled item\t↩"
            && left_title == "Left\t←"
            && empty_title == "Empty\t"
            && branch_title == "Nested\tC-x 4"
            && run_alignment == 1
            && disabled_alignment == 1
            && left_alignment == 1
            && branch_alignment == 1
            && run_location > 40.0
            && run_location == disabled_location
            && run_location == empty_location
            && run_location == branch_location
            && left_location < run_location
            && text(run_equivalent).is_empty()
            && text(plain_equivalent) == "j"
            && text(plain_title) == "Plain"
            && !action.is_null()
            && disabled_action.is_null()
            && disabled_enabled == NO
            && state == 1
            && separator != NO
    }
}

fn main() {
    Application::new().run(|app| {
        app.bind_keys([KeyBinding::new("cmd-j", Run, None)]);
        app.set_menus(menus("C-c z"));
        app.spawn(async move |cx| {
            Timer::after(Duration::from_millis(20)).await;
            // Menu validation calls into GPUI; inspect outside an App borrow.
            if !unsafe { check("C-c z") } {
                std::process::exit(1);
            }
            cx.update(|app| app.set_menus(menus("C-c q"))).unwrap();
            Timer::after(Duration::from_millis(20)).await;
            if !unsafe { check("C-c q") } {
                std::process::exit(1);
            }
            cx.update(|app| app.quit()).unwrap();
        })
        .detach();
    });
}
