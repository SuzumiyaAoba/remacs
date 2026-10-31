//! Native macOS dialog-header and static-text regression. Compile against GPUI and run on the
//! main thread under an external watchdog. Each sheet is closed automatically.
#[path = "../../src/dialogs.rs"]
mod dialogs;

use cocoa::{
    base::{BOOL, NO, id, nil},
    foundation::NSArray,
};
use gpui::{AppContext, Application, Context, IntoElement, Render, Styled, Timer, Window, div};
use objc::{class, msg_send, sel, sel_impl};
use std::{ffi::CStr, time::Duration};

struct Empty;
impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

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

unsafe fn contains_text(view: id, expected: &str) -> bool {
    let string_value_supported: BOOL =
        unsafe { msg_send![view, respondsToSelector: sel!(stringValue)] };
    if string_value_supported != NO {
        let value: id = unsafe { msg_send![view, stringValue] };
        if unsafe { string_value(value) }.is_some_and(|value| value.contains(expected)) {
            return true;
        }
    }
    let string_supported: BOOL = unsafe { msg_send![view, respondsToSelector: sel!(string)] };
    if string_supported != NO {
        let value: id = unsafe { msg_send![view, string] };
        if unsafe { string_value(value) }.is_some_and(|value| value.contains(expected)) {
            return true;
        }
    }
    let children: id = unsafe { msg_send![view, subviews] };
    for index in 0..unsafe { children.count() } {
        if unsafe { contains_text(children.objectAtIndex(index), expected) } {
            return true;
        }
    }
    false
}

unsafe fn press_ok(view: id, expected_tag: isize) -> bool {
    let button: BOOL = unsafe { msg_send![view, isKindOfClass: class!(NSButton)] };
    if button != NO {
        let title: id = unsafe { msg_send![view, title] };
        let tag: isize = unsafe { msg_send![view, tag] };
        if tag == expected_tag && unsafe { string_value(title) }.as_deref() == Some("OK") {
            let _: () = unsafe { msg_send![view, performClick: nil] };
            return true;
        }
    }
    let children: id = unsafe { msg_send![view, subviews] };
    for index in 0..unsafe { children.count() } {
        if unsafe { press_ok(children.objectAtIndex(index), expected_tag) } {
            return true;
        }
    }
    false
}

fn main() {
    Application::new().run(|app| {
        app.activate(true);
        let window = app.open_window(Default::default(), |_, cx| cx.new(|_| Empty)).unwrap();
        app.spawn(async move |cx| {
            for (header, expected, text, expected_index) in [
                (false, "Question", None, 0usize),
                (true, "Information", None, 0),
                (false, "Question", Some("Select an operation:"), 1),
            ] {
                let response = cx.update(|app| window.update(app, |_, window, app| {
                    let mut buttons = dialogs::dialog_prompt_buttons(vec!["OK".into()]);
                    if let Some(text) = text { buttons.insert(0, gpui::PromptButton::text(text)); }
                    window.prompt(dialogs::dialog_prompt_level(Some(header)), "Header body", None, &buttons, app)
                }).unwrap()).unwrap();
                let mut matched = None;
                for _ in 0..100 {
                    Timer::after(Duration::from_millis(50)).await;
                    matched = cx.update(|_| unsafe {
                        let app: id = msg_send![class!(NSApplication), sharedApplication];
                        let windows: id = msg_send![app, windows];
                        for index in 0..windows.count() {
                            let sheet = windows.objectAtIndex(index);
                            let visible: BOOL = msg_send![sheet, isVisible];
                            let is_sheet: BOOL = msg_send![sheet, isSheet];
                            if visible == NO || is_sheet == NO { continue; }
                            let title = string_value(msg_send![sheet, title]);
                            let visibility: isize = msg_send![sheet, titleVisibility];
                            let mask: u64 = msg_send![sheet, styleMask];
                            let content: id = msg_send![sheet, contentView];
                            let text_found = text.is_none_or(|text| contains_text(content, text));
                            let pressed = press_ok(content, expected_index as isize);
                            println!("header={header} text={text:?} title={title:?} title_visibility={visibility} style={mask:#x} text_found={text_found} pressed={pressed}");
                            if !pressed {
                                let parent: id = msg_send![sheet, sheetParent];
                                let _: () = msg_send![parent, endSheet: sheet returnCode: 0isize];
                            }
                            return Some(pressed && text_found && title.as_deref() == Some(expected) && visibility == 0 && mask & 1 != 0);
                        }
                        None
                    }).unwrap();
                    if matched.is_some() { break; }
                }
                let passed = if let Some(matched) = matched {
                    let result = response.await;
                    println!("header={header} result={result:?}");
                    matched && matches!(result, Ok(index) if index == expected_index)
                } else {
                    eprintln!("no native sheet appeared within 5 seconds");
                    false
                };
                if !passed { std::process::exit(1); }
            }
            cx.update(|app| app.quit()).unwrap();
        }).detach();
    });
}
