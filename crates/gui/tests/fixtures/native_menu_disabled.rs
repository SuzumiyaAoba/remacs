//! Native NSMenu check. Run on the main thread under the external watchdog.
use cocoa::base::{BOOL, NO, id, nil};
use gpui::{Application, Menu, MenuItem, Timer};
use objc::{class, msg_send, sel, sel_impl};
use std::{ffi::CStr, time::Duration};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = menu_fixture, no_json)]
struct Enabled;

fn main() {
    Application::new().run(|app| {
        app.set_menus(vec![
            Menu {
                name: "App".into(),
                items: Vec::new(),
            },
            Menu {
                name: "QA Test".into(),
                items: vec![
                    MenuItem::disabled("Disabled").checked(true),
                    MenuItem::action("Enabled", Enabled).checked(true),
                    MenuItem::action("Unchecked", Enabled),
                ],
            },
        ]);
        app.spawn(async move |cx| {
            Timer::after(Duration::from_millis(20)).await;
            // Native menu validation can call back into GPUI. Inspect outside an
            // App update borrow so the delegate can safely validate actions.
            let passed = unsafe {
                let native_app: id = msg_send![class!(NSApplication), sharedApplication];
                let menu: id = msg_send![native_app, mainMenu];
                let root: id = msg_send![menu, itemAtIndex: 1usize];
                let submenu: id = msg_send![root, submenu];
                let _: () = msg_send![submenu, update];
                let disabled: id = msg_send![submenu, itemAtIndex: 0usize];
                let action_item: id = msg_send![submenu, itemAtIndex: 1usize];
                let unchecked: id = msg_send![submenu, itemAtIndex: 2usize];
                let title: id = msg_send![disabled, title];
                let bytes: *const std::ffi::c_char = msg_send![title, UTF8String];
                let title = CStr::from_ptr(bytes).to_string_lossy();
                let enabled: BOOL = msg_send![disabled, isEnabled];
                let target: id = msg_send![disabled, target];
                let action: *const std::ffi::c_void = msg_send![disabled, action];
                let tag: isize = msg_send![disabled, tag];
                let active_action: *const std::ffi::c_void = msg_send![action_item, action];
                let disabled_state: isize = msg_send![disabled, state];
                let active_state: isize = msg_send![action_item, state];
                let unchecked_state: isize = msg_send![unchecked, state];
                println!(
                    "title={title:?} enabled={enabled} target_nil={} action_nil={} tag={tag} normal_action={}",
                    target == nil,
                    action.is_null(),
                    !active_action.is_null()
                );
                println!(
                    "check states: disabled={disabled_state} active={active_state} unchecked={unchecked_state}"
                );
                title == "Disabled"
                    && enabled == NO
                    && target == nil
                    && action.is_null()
                    && tag == -1
                    && !active_action.is_null()
                    && disabled_state == 1
                    && active_state == 1
                    && unchecked_state == 0
            };
            if !passed {
                std::process::exit(1);
            }
            cx.update(|app| app.quit()).unwrap();
        })
        .detach();
    });
}
