//! Manual native minimum-size regression. Run under an external watchdog.

#[cfg(target_os = "macos")]
mod probe {
    use cocoa::foundation::NSSize;
    use gpui::*;
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn native_minimum(window: &Window) -> (f64, f64) {
        let handle = HasWindowHandle::window_handle(window).unwrap();
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            panic!("expected AppKit window");
        };
        unsafe {
            let view = handle.ns_view.as_ptr() as *mut Object;
            let native: *mut Object = msg_send![view, window];
            let minimum: NSSize = msg_send![native, contentMinSize];
            (minimum.width, minimum.height)
        }
    }

    pub fn run() {
        Application::new().run(|cx| {
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(80.0), px(80.0)),
                            size(px(320.0), px(160.0)),
                        ))),
                        window_min_size: Some(size(px(80.0), px(0.0))),
                        focus: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| Probe),
                )
                .unwrap();
            cx.spawn(async move |cx| {
                let initial = handle
                    .update(cx, |_, window, _| native_minimum(window))
                    .unwrap();
                assert_eq!(initial, (80.0, 0.0));
                for (minimum, expected) in [
                    (Some(size(px(99.0), px(0.0))), (99.0, 0.0)),
                    (Some(size(px(296.0), px(126.0))), (296.0, 126.0)),
                    (None, (0.0, 0.0)),
                ] {
                    let actual = handle
                        .update(cx, |_, window, _| {
                            window.set_minimum_size(minimum);
                            native_minimum(window)
                        })
                        .unwrap();
                    if actual != expected {
                        eprintln!("native minimum size: expected {expected:?}, got {actual:?}");
                        std::process::exit(1);
                    }
                    println!("minimum size: {actual:?}");
                }
                println!("native minimum-size regression passed");
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        });
    }
}

#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("this native probe currently requires macOS");
}
