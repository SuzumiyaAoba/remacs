//! Direct NSMenu menubar activation. Run on the main thread under the watchdog.
use cocoa::base::{id, nil};
use gpui::{Application, Menu, MenuItem, Timer};
use objc::{class, msg_send, sel, sel_impl};
use std::{
    cell::Cell,
    ffi::{CStr, c_void},
    rc::Rc,
    time::Duration,
};

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = root_menu_fixture, no_json)]
struct Run;

unsafe fn text(value: id) -> String {
    unsafe {
        let bytes: *const std::ffi::c_char = msg_send![value, UTF8String];
        if bytes.is_null() {
            String::new()
        } else {
            CStr::from_ptr(bytes).to_string_lossy().into_owned()
        }
    }
}

fn main() {
    Application::new().run(|app| {
        let runs = Rc::new(Cell::new(0usize));
        let on_run = runs.clone();
        app.on_action(move |_: &Run, _| on_run.set(on_run.get() + 1));
        app.set_menu_bar_items(vec![
            MenuItem::submenu(Menu {
                name: "App".into(),
                items: Vec::new(),
            }),
            MenuItem::action("Run now", Run),
            MenuItem::submenu(Menu {
                name: "One item".into(),
                items: vec![MenuItem::action("Inside", Run)],
            }),
        ]);
        app.spawn(async move |cx| {
            Timer::after(Duration::from_millis(20)).await;
            // Validation and action dispatch reenter GPUI; hold no App borrow.
            let passed = unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let menu: id = msg_send![app, mainMenu];
                let _: () = msg_send![menu, update];
                let root: id = msg_send![menu, itemAtIndex: 1usize];
                let submenu: id = msg_send![root, submenu];
                let action: *const c_void = msg_send![root, action];
                let title: id = msg_send![root, title];
                let title = text(title);
                let one: id = msg_send![menu, itemAtIndex: 2usize];
                let one: id = msg_send![one, submenu];
                let count: usize = msg_send![one, numberOfItems];
                println!(
                    "root={title:?} submenu_nil={} action={} one_item_children={count}",
                    submenu == nil,
                    !action.is_null()
                );
                if title != "Run now"
                    || submenu != nil
                    || action.is_null()
                    || one == nil
                    || count != 1
                {
                    false
                } else {
                    let _: () = msg_send![menu, performActionForItemAtIndex: 1usize];
                    true
                }
            };
            if !passed {
                std::process::exit(1);
            }
            Timer::after(Duration::from_millis(20)).await;
            println!("root action runs={}", runs.get());
            if runs.get() != 1 {
                std::process::exit(1);
            }

            // The existing API still creates real submenus and Window's role.
            cx.update(|app| {
                app.set_menus(vec![
                    Menu {
                        name: "App".into(),
                        items: Vec::new(),
                    },
                    Menu {
                        name: "Window".into(),
                        items: vec![MenuItem::action("Inside", Run)],
                    },
                ])
            })
            .unwrap();
            Timer::after(Duration::from_millis(20)).await;
            let legacy = unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let menu: id = msg_send![app, mainMenu];
                let root: id = msg_send![menu, itemAtIndex: 1usize];
                let submenu: id = msg_send![root, submenu];
                let window_menu: id = msg_send![app, windowsMenu];
                let title: id = msg_send![submenu, title];
                let title = text(title);
                println!(
                    "legacy submenu={title:?} window_role={}",
                    submenu == window_menu
                );
                submenu != nil && submenu == window_menu && title == "Window"
            };
            if !legacy {
                std::process::exit(1);
            }
            cx.update(|app| app.quit()).unwrap();
        })
        .detach();
    });
}
