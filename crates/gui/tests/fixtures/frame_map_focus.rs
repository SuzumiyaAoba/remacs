//! Native AppKit key-window/input validation of the production mapping policy.
//! Run as an isolated process under the external watchdog.
#[path = "../../src/frame_visibility.rs"]
mod frame_visibility;

use cocoa::{
    base::{NO, YES, id, nil},
    foundation::{NSInteger, NSPoint, NSString},
};
use gpui::{
    App, Application, Context, FocusHandle, IntoElement, Render, Timer, Window, WindowHandle,
    WindowOptions, div, prelude::*,
};
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use remacs_runtime::frontend::FrameVisibility;
use std::{cell::Cell, rc::Rc, time::Duration};

struct InputView {
    focus: FocusHandle,
    count: Rc<Cell<usize>>,
    label: &'static str,
}
impl Render for InputView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus)
            .size_full()
            .child(self.label)
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, _| {
                if event.keystroke.key == "a" {
                    view.count.set(view.count.get() + 1);
                }
            }))
    }
}

fn open(
    app: &mut App,
    label: &'static str,
    count: Rc<Cell<usize>>,
    focus: bool,
) -> WindowHandle<InputView> {
    app.open_window(
        WindowOptions {
            focus,
            show: true,
            ..Default::default()
        },
        |window, app| {
            app.new(|cx| {
                let focus = cx.focus_handle();
                focus.focus(window);
                InputView {
                    focus,
                    count,
                    label,
                }
            })
        },
    )
    .unwrap()
}

fn native(window: &Window) -> id {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        panic!("AppKit handle")
    };
    unsafe { msg_send![handle.ns_view.as_ptr() as id, window] }
}
fn key(window: id) -> bool {
    unsafe {
        let result: objc::runtime::BOOL = msg_send![window, isKeyWindow];
        result == YES
    }
}
fn require(ok: bool, message: &str) {
    if !ok {
        eprintln!("native focus failure: {message}");
        std::process::exit(1);
    }
}
unsafe fn send_key_to_key_window() {
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let window: id = msg_send![app, keyWindow];
        let number: NSInteger = msg_send![window, windowNumber];
        let characters = NSString::alloc(nil).init_str("a");
        let event: id = msg_send![class!(NSEvent),
            keyEventWithType: 10usize location: NSPoint::new(0.0, 0.0)
            modifierFlags: 0usize timestamp: 0.0f64 windowNumber: number context: nil
            characters: characters charactersIgnoringModifiers: characters
            isARepeat: NO keyCode: 0u16];
        let _: () = msg_send![app, sendEvent: event];
        let _: () = msg_send![characters, release];
    }
}

pub fn run() {
    Application::new().run(|app| {
        // Keep these owned regression windows independent of tabbing preferences.
        unsafe {
            let _: () = msg_send![class!(NSWindow), setAllowsAutomaticWindowTabbing: NO];
        }
        let first_count = Rc::new(Cell::new(0));
        let second_count = Rc::new(Cell::new(0));
        let first = open(app, "REmacs focus fixture A", first_count.clone(), true);
        let second = open(app, "REmacs focus fixture B", second_count.clone(), false);
        // Match the reproduction's initial condition: this application and A
        // are active before applying B's initial mapping policy.
        app.activate(true);
        first
            .update(app, |_, window, _| window.activate_window())
            .unwrap();
        let first_native = first.update(app, |_, window, _| native(window)).unwrap();
        let second_native = second.update(app, |_, window, _| native(window)).unwrap();
        app.spawn(async move |cx| {
            Timer::after(Duration::from_millis(60)).await;
            println!(
                "baseline A_key={} B_key={}",
                key(first_native),
                key(second_native)
            );
            require(
                key(first_native) && !key(second_native),
                "orderFront must leave A key initially",
            );
            cx.update(|app| {
                second.update(app, |_, window, _| {
                    require(window.is_visible(), "secondary is already visible");
                    frame_visibility::apply_initial_frame_visibility(
                        window,
                        FrameVisibility::Visible,
                        true,
                    );
                })
            })
            .unwrap()
            .unwrap();
            Timer::after(Duration::from_millis(60)).await;
            require(
                !key(first_native) && key(second_native),
                "default mapped B must become key",
            );
            unsafe {
                send_key_to_key_window();
            }
            Timer::after(Duration::from_millis(30)).await;
            require(
                first_count.get() == 0 && second_count.get() == 1,
                "keyboard input must reach B only",
            );
            println!("default mapping: A_key=false B_key=true A_input=0 B_input=1");
            for (no_focus_on_map, no_accept_focus) in [(true, false), (false, true), (true, true)] {
                let candidate = cx
                    .update(|app| {
                        first
                            .update(app, |_, window, _| window.activate_window())
                            .unwrap();
                        open(
                            app,
                            "REmacs background focus fixture",
                            Rc::new(Cell::new(0)),
                            false,
                        )
                    })
                    .unwrap();
                let candidate_native = cx
                    .update(|app| candidate.update(app, |_, window, _| native(window)))
                    .unwrap()
                    .unwrap();
                Timer::after(Duration::from_millis(40)).await;
                cx.update(|app| {
                    candidate.update(app, |_, window, _| {
                        frame_visibility::apply_initial_frame_visibility(
                            window,
                            FrameVisibility::Visible,
                            !no_focus_on_map && !no_accept_focus,
                        );
                    })
                })
                .unwrap()
                .unwrap();
                Timer::after(Duration::from_millis(40)).await;
                require(
                    key(first_native) && !key(candidate_native),
                    "focus inhibitors must preserve A",
                );
                cx.update(|app| candidate.update(app, |_, window, _| window.remove_window()))
                    .unwrap()
                    .unwrap();
            }
            println!("no-focus-on-map/no-accept-focus: native key window preserved");
            cx.update(|app| app.quit()).unwrap();
        })
        .detach();
    });
}
