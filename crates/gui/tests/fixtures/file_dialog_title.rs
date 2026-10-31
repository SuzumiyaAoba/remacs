//! Native macOS regression fixture. Compile against the built GPUI library and
//! run on the main thread under an external watchdog; no path is selected.
#[path = "../../src/file_dialog.rs"]
mod file_dialog;

use cocoa::{
    appkit::{NSApp, NSOpenPanel},
    base::{BOOL, NO, id, nil},
    foundation::NSArray,
};
use gpui::{Application, Timer};
use objc::{class, msg_send, sel, sel_impl};
use std::{ffi::CStr, time::Duration};

unsafe fn string_value(value: id) -> Option<String> {
    if value == nil {
        return None;
    }
    let bytes: *const std::ffi::c_char = unsafe { msg_send![value, UTF8String] };
    Some(
        unsafe { CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn main() {
    Application::new().run(move |cx| {
        cx.activate(true);
        let default_prompt = unsafe {
            let panel = NSOpenPanel::openPanel(nil);
            string_value(msg_send![panel, prompt])
        };
        let response = cx.prompt_for_paths(file_dialog::path_prompt_options(
            "Choose Existing File 日本語".into(),
            Some("/private/tmp/".into()),
            None,
            false,
        ));
        cx.spawn(async move |cx| {
            let mut labels_match = None;
            for _ in 0..100 {
                Timer::after(Duration::from_millis(50)).await;
                labels_match = cx.update(|_| unsafe {
                    let app = NSApp();
                    let windows: id = msg_send![app, windows];
                    for index in 0..windows.count() {
                        let panel = windows.objectAtIndex(index);
                        let is_panel: BOOL = msg_send![panel, isKindOfClass: class!(NSOpenPanel)];
                        let visible: BOOL = msg_send![panel, isVisible];
                        if is_panel == NO || visible == NO {
                            continue;
                        }
                        let title = string_value(msg_send![panel, title]);
                        let prompt = string_value(msg_send![panel, prompt]);
                        let result = title.as_deref() == Some("Choose Existing File 日本語")
                            && prompt == default_prompt;
                        println!("native panel title={title:?} button={prompt:?} default_button={default_prompt:?}");
                        let _: () = msg_send![panel, cancel: nil];
                        return Some(result);
                    }
                    None
                }).unwrap();
                if labels_match.is_some() {
                    break;
                }
            }
            let passed = if let Some(labels_match) = labels_match {
                let cancelled = matches!(response.await, Ok(Ok(None)));
                println!("native file dialog labels_match={labels_match} cancelled={cancelled}");
                labels_match && cancelled
            } else {
                eprintln!("native file dialog did not become visible within 5 seconds");
                false
            };
            // App::quit exits the process, so report assertion failure first.
            if !passed {
                std::process::exit(1);
            }
            cx.update(|app| app.quit()).unwrap();
        }).detach();
    });
}
