//! Manual native visibility regression. Run only under an external watchdog.

#[cfg(target_os = "macos")]
mod probe {
    use gpui::*;
    use objc::{
        msg_send,
        runtime::{BOOL, Object, YES},
        sel, sel_impl,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::time::{Duration, Instant};

    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn native_state(window: &Window) -> (bool, bool) {
        let handle = HasWindowHandle::window_handle(window).expect("native window handle");
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            panic!("expected an AppKit window");
        };
        unsafe {
            let view = handle.ns_view.as_ptr() as *mut Object;
            let native: *mut Object = msg_send![view, window];
            let minimized: BOOL = msg_send![native, isMiniaturized];
            (window.is_visible(), minimized == YES)
        }
    }

    async fn wait_for(
        handle: WindowHandle<Probe>,
        cx: &mut AsyncApp,
        expected: (bool, bool),
        label: &str,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let actual = handle
                .update(cx, |_, window, _| native_state(window))
                .map_err(|error| error.to_string())?;
            if actual == expected {
                println!("{label}: {actual:?}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!("{label}: expected {expected:?}, got {actual:?}"));
            }
            Timer::after(Duration::from_millis(20)).await;
        }
    }

    pub fn run() {
        Application::new().run(move |cx| {
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(80.0), px(80.0)),
                            size(px(320.0), px(160.0)),
                        ))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Remacs visibility regression".into()),
                            ..Default::default()
                        }),
                        focus: false,
                        ..Default::default()
                    },
                    |_, cx| cx.new(|_| Probe),
                )
                .expect("open probe window");
            cx.spawn(async move |cx| {
                let checked: Result<(), String> = async {
                    wait_for(handle, cx, (true, false), "visible").await?;
                    handle
                        .update(cx, |_, window, _| window.set_visible(false))
                        .map_err(|error| error.to_string())?;
                    wait_for(handle, cx, (false, false), "hidden").await?;
                    handle
                        .update(cx, |_, window, _| window.minimize_window())
                        .map_err(|error| error.to_string())?;
                    wait_for(handle, cx, (false, true), "iconified after hidden").await?;
                    handle
                        .update(cx, |_, window, _| window.set_visible(false))
                        .map_err(|error| error.to_string())?;
                    wait_for(handle, cx, (false, false), "hidden after iconified").await?;
                    handle
                        .update(cx, |_, window, _| window.set_visible(true))
                        .map_err(|error| error.to_string())?;
                    wait_for(handle, cx, (true, false), "restored").await?;
                    Ok(())
                }
                .await;
                if let Err(error) = checked {
                    eprintln!("native visibility regression: {error}");
                    std::process::exit(1);
                }
                println!("native visibility regression passed");
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
