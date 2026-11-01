//! Inspect real NSMenuItem tooltip/state and activation under the watchdog.
use cocoa::base::{BOOL, NO, id, nil};
use gpui::{Application, Menu, MenuItem, Timer};
use objc::{class, msg_send, sel, sel_impl};
use std::{cell::Cell, ffi::CStr, rc::Rc, time::Duration};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = help_fixture, no_json)]
struct Run;

unsafe fn text(value: id) -> Option<String> {
    if value == nil {
        return None;
    }
    unsafe {
        let bytes: *const std::ffi::c_char = msg_send![value, UTF8String];
        Some(CStr::from_ptr(bytes).to_string_lossy().into_owned())
    }
}

fn main() {
    Application::new().run(|app| {
        let calls = Rc::new(Cell::new(0));
        let received = calls.clone();
        app.on_action(move |_: &Run, _| received.set(received.get() + 1));
        app.set_menu_bar_items(vec![
            MenuItem::submenu(Menu { name: "App".into(), items: Vec::new() }),
            MenuItem::submenu(Menu { name: "QA Help".into(), items: vec![
                MenuItem::action("Run", Run).tooltip("Runs the test command").checked(true).key_hint("C-c h"),
                MenuItem::disabled("Disabled").checked(true).tooltip("Still described"),
                MenuItem::submenu(Menu { name: "More".into(), items: vec![
                    MenuItem::action("Child", Run).tooltip("Child help"),
                ] }).tooltip("Submenu help").key_hint("C-x 4"),
            ] }),
            MenuItem::submenu(Menu { name: "Window".into(), items: Vec::new() }).tooltip("Window help"),
        ]);
        assert_eq!(app.get_menus().unwrap().len(), 3);
        app.spawn(async move |cx| {
            Timer::after(Duration::from_millis(20)).await;
            let passed = unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let bar: id = msg_send![app, mainMenu];
                let root: id = msg_send![bar, itemAtIndex: 1usize];
                let menu: id = msg_send![root, submenu];
                let _: () = msg_send![menu, update];
                let run: id = msg_send![menu, itemAtIndex: 0usize];
                let disabled: id = msg_send![menu, itemAtIndex: 1usize];
                let more: id = msg_send![menu, itemAtIndex: 2usize];
                let children: id = msg_send![more, submenu];
                let child: id = msg_send![children, itemAtIndex: 0usize];
                let tip: id = msg_send![run, toolTip];
                let disabled_tip: id = msg_send![disabled, toolTip];
                let more_tip: id = msg_send![more, toolTip];
                let child_tip: id = msg_send![child, toolTip];
                let title: id = msg_send![run, attributedTitle];
                let title: id = msg_send![title, string];
                let state: isize = msg_send![run, state];
                let enabled: BOOL = msg_send![disabled, isEnabled];
                let window_root: id = msg_send![bar, itemAtIndex: 2usize];
                let window_tip: id = msg_send![window_root, toolTip];
                let window: id = msg_send![window_root, submenu];
                let windows_menu: id = msg_send![app, windowsMenu];
                let help = text(tip); let heading = text(title);
                println!("tooltip={help:?} title={heading:?} checked={state} disabled={enabled} window_role={}", window == windows_menu);
                let passed = help.as_deref() == Some("Runs the test command")
                    && text(disabled_tip).as_deref() == Some("Still described")
                    && text(more_tip).as_deref() == Some("Submenu help")
                    && text(child_tip).as_deref() == Some("Child help")
                    && text(window_tip).as_deref() == Some("Window help")
                    && heading.is_some_and(|title| title.starts_with("Run\t"))
                    && state == 1 && enabled == NO && window == windows_menu;
                if passed { let _: () = msg_send![menu, performActionForItemAtIndex: 0usize]; }
                passed
            };
            if !passed { std::process::exit(1); }
            Timer::after(Duration::from_millis(20)).await;
            println!("native action calls={}", calls.get());
            if calls.get() != 1 { std::process::exit(1); }
            cx.update(|app| app.set_menus(vec![
                Menu { name: "App".into(), items: Vec::new() },
                Menu { name: "QA Help".into(), items: vec![MenuItem::action("Run", Run)] },
            ])).unwrap();
            Timer::after(Duration::from_millis(20)).await;
            let cleared = unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let bar: id = msg_send![app, mainMenu];
                let root: id = msg_send![bar, itemAtIndex: 1usize];
                let menu: id = msg_send![root, submenu];
                let run: id = msg_send![menu, itemAtIndex: 0usize];
                let tip: id = msg_send![run, toolTip];
                tip == nil
            };
            println!("tooltip cleared={cleared}");
            if !cleared { std::process::exit(1); }
            cx.update(|app| app.quit()).unwrap();
        }).detach();
    });
}
