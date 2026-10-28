//! GPUI frontend. The evaluator lives on a separate logic thread.
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
mod accessibility;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSInteger, NSString};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, runtime::{BOOL, YES}, sel, sel_impl};
mod keys;
mod logic;
mod monitor;
mod view;
use logic::{ClipboardRequest, FrameSnapshot, GuiEvent, GuiRequest, start_logic_with_frames};
use view::{EditorView, build_managed_editor_view};

/// Launch the gpui front-end. `files` are visited via `find-file`
/// once the evaluator boots on the logic thread.
pub fn run_gui(files: &[String]) -> io::Result<()> {
    run_gui_with_dump(files, None, &[])
}

pub fn run_gui_with_dump(
    files: &[String],
    dump_file: Option<&str>,
    arguments: &[String],
) -> io::Result<()> {
    let (event_tx, gui_requests, clipboard_requests) = start_logic_with_frames(
        files.to_vec(),
        dump_file.map(str::to_owned),
        arguments.to_vec(),
    )?;

    let open_error = Arc::new(Mutex::new(None));
    let app_open_error = Arc::clone(&open_error);
    Application::new().run(move |cx: &mut App| {
        #[cfg(target_os = "macos")]
        register_power_off_listener(event_tx.clone());
        cx.spawn(async move |cx| {
            while let Ok(request) = clipboard_requests.recv().await {
                match request {
                    ClipboardRequest::Ready(ready) => {
                        ready.store(true, std::sync::atomic::Ordering::Release);
                    }
                    ClipboardRequest::Read(reply) => {
                        let result = cx
                            .update(|app| {
                                app.read_from_clipboard()
                                    .and_then(|item| item.text())
                            })
                            .map_err(|error| error.to_string());
                        let _ = reply.send(result);
                    }
                    ClipboardRequest::Write(text, reply) => {
                        let result = cx
                            .update(|app| {
                                app.write_to_clipboard(ClipboardItem::new_string(text));
                            })
                            .map_err(|error| error.to_string());
                        if let Some(reply) = reply {
                            let _ = reply.send(result);
                        }
                    }
                }
            }
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let primary_window = match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(960.0), px(640.0)),
                    cx,
                ))),
                ..Default::default()
            },
            |window, cx| {
                cx.activate(false);
                cx.new(|cx| build_managed_editor_view(window, cx, event_tx.clone(), None, None))
            },
        ) {
            Ok(window) => window,
            Err(error) => {
                let message = format!("remacs: cannot open window: {error}");
                eprintln!("{message}");
                *app_open_error.lock().unwrap() = Some(message);
                cx.quit();
                return;
            }
        };

        cx.spawn(async move |cx| {
            let mut windows = HashMap::<usize, WindowHandle<EditorView>>::new();
            let mut window_modes = HashMap::<usize, Option<String>>::new();
            let mut frame_behaviors = HashMap::<usize, (bool, bool, Option<String>, bool)>::new();
            let mut native_visibility = HashMap::<usize, bool>::new();
            let mut primary_window = Some(primary_window);
            while let Ok(request) = gui_requests.recv().await {
                match request {
                    GuiRequest::FrameSnapshots(snapshots) => {
                        let live_frames: HashSet<_> =
                            snapshots.iter().map(|snapshot| snapshot.frame_id).collect();
                        for snapshot in snapshots {
                            let frame_id = snapshot.frame_id;
                            let grid = snapshot.grid.clone();
                            let fullscreen = snapshot.fullscreen.clone();
                            let behavior = (
                                snapshot.auto_raise,
                                snapshot.auto_lower,
                                snapshot.z_group.clone(),
                                snapshot.parent_frame.is_some(),
                            );
                            if let Some(handle) = windows.get(&frame_id).copied() {
                                let update_window_mode =
                                    window_modes.get(&frame_id) != Some(&fullscreen);
                                let requested_fullscreen = fullscreen.clone();
                                let update_behavior =
                                    frame_behaviors.get(&frame_id) != Some(&behavior);
                                let requested_behavior = behavior.clone();
                                let _ = handle.update(cx, move |view, window, cx| {
                                    if view.grid != grid || grid.blink_cursor {
                                        view.grid = grid;
                                        cx.notify();
                                    }
                                    if update_window_mode {
                                        apply_frame_window_state(
                                            window,
                                            requested_fullscreen.as_deref(),
                                        );
                                    }
                                    if update_behavior {
                                        view.set_frame_behavior(
                                            requested_behavior.0,
                                            requested_behavior.1,
                                        );
                                        set_frame_z_group_window(
                                            window,
                                            requested_behavior.2.as_deref(),
                                            requested_behavior.3,
                                        );
                                    }
                                });
                                window_modes.insert(frame_id, fullscreen);
                                frame_behaviors.insert(frame_id, behavior);
                                native_visibility.entry(frame_id).or_insert(true);
                                continue;
                            }
                            if let Some(handle) = primary_window.take() {
                                let initial_fullscreen = fullscreen.clone();
                                let initial_behavior = behavior.clone();
                                let _ = handle.update(cx, move |view, window, cx| {
                                    view.set_frame_id(frame_id);
                                    view.grid = grid;
                                    cx.notify();
                                    apply_frame_window_state(
                                        window,
                                        initial_fullscreen.as_deref(),
                                    );
                                    view.set_frame_behavior(
                                        initial_behavior.0,
                                        initial_behavior.1,
                                    );
                                    set_frame_z_group_window(
                                        window,
                                        initial_behavior.2.as_deref(),
                                        initial_behavior.3,
                                    );
                                });
                                window_modes.insert(frame_id, fullscreen);
                                frame_behaviors.insert(frame_id, behavior);
                                native_visibility.insert(frame_id, true);
                                windows.insert(frame_id, handle);
                                continue;
                            }

                            let options = frame_window_options(&snapshot, &windows, cx);
                            let event_tx = event_tx.clone();
                            let handle = match cx.open_window(options, move |window, app| {
                                app.new(|cx| {
                                    build_managed_editor_view(
                                        window,
                                        cx,
                                        event_tx,
                                        Some(frame_id),
                                        Some(grid),
                                    )
                                })
                            }) {
                                Ok(handle) => handle,
                                Err(error) => {
                                    eprintln!("remacs: cannot open frame window: {error}");
                                    continue;
                                }
                            };
                            window_modes.insert(frame_id, snapshot.fullscreen.clone());
                            frame_behaviors.insert(frame_id, behavior.clone());
                            let _ = handle.update(cx, move |view, window, _cx| {
                                view.set_frame_behavior(behavior.0, behavior.1);
                                set_frame_z_group_window(
                                    window,
                                    behavior.2.as_deref(),
                                    behavior.3,
                                );
                            });
                            native_visibility.insert(frame_id, true);
                            windows.insert(frame_id, handle);
                        }
                        let removed: Vec<_> = windows
                            .keys()
                            .filter(|frame_id| !live_frames.contains(frame_id))
                            .copied()
                            .collect();
                        for frame_id in removed {
                            if let Some(handle) = windows.remove(&frame_id) {
                                let _ = handle.update(cx, |_view, window, _cx| {
                                    window.remove_window();
                                });
                            }
                            window_modes.remove(&frame_id);
                            frame_behaviors.remove(&frame_id);
                            native_visibility.remove(&frame_id);
                        }
                        if let Some(stack) = cx.update(|app| app.window_stack()).ok().flatten() {
                            for (frame_id, handle) in &windows {
                                let any_window: AnyWindowHandle = (*handle).into();
                                let visible = handle
                                    .update(cx, |_view, window, _cx| {
                                        native_frame_visible(window)
                                    })
                                    .ok()
                                    .flatten()
                                    .unwrap_or_else(|| stack.contains(&any_window));
                                if native_visibility.get(frame_id) != Some(&visible) {
                                    native_visibility.insert(*frame_id, visible);
                                    let _ = event_tx.send(GuiEvent::FrameVisibility {
                                        frame_id: *frame_id,
                                        visible,
                                    });
                                }
                            }
                        }
                    }
                    GuiRequest::Prompt {
                        frame_id,
                        message,
                        detail,
                        buttons,
                        reply,
                    } => {
                        let buttons: Vec<_> = buttons.into_iter().map(PromptButton::new).collect();
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            match handle.update(cx, |_view, window, cx| {
                                window.prompt(
                                    PromptLevel::Info,
                                    &message,
                                    detail.as_deref(),
                                    &buttons,
                                    cx,
                                )
                            }) {
                                Ok(response) => Ok(response.await.ok()),
                                Err(error) => Err(error.to_string()),
                            }
                        } else {
                            Err(format!("GUI frame {frame_id} is not open"))
                        };
                        let _ = reply.send(result);
                    }
                    GuiRequest::FrameVisibility { frame_id, visible } => {
                        native_visibility.insert(frame_id, visible);
                        if let Some(handle) = windows.get(&frame_id).copied() {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                if visible {
                                    show_frame_window(window);
                                } else {
                                    window.minimize_window();
                                }
                            });
                        }
                    }
                    GuiRequest::FrameOrder { frame_id, raise } => {
                        if let Some(handle) = windows.get(&frame_id).copied() {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                if raise {
                                    raise_frame_window(window);
                                } else {
                                    lower_frame_window(window);
                                }
                            });
                        }
                    }
                    GuiRequest::ColorPanel { frame_id, reply } => {
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            handle
                                .update(cx, |_view, window, _cx| {
                                    show_native_color_panel(window)
                                })
                                .map_err(|error| error.to_string())
                                .and_then(|result| result)
                        } else {
                            Err(format!("GUI frame {frame_id} is not open"))
                        };
                        let _ = reply.send(result);
                    }
                    GuiRequest::CharacterPalette { frame_id, reply } => {
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            handle
                                .update(cx, |_view, window, _cx| {
                                    show_native_character_palette(window)
                                })
                                .map_err(|error| error.to_string())
                                .and_then(|result| result)
                        } else {
                            Err(format!("GUI frame {frame_id} is not open"))
                        };
                        let _ = reply.send(result);
                    }
                    GuiRequest::FileDialog {
                        prompt,
                        directory,
                        must_match,
                        initial,
                        directory_only,
                        reply,
                    } => {
                        // GPUI's path panel currently accepts mode and prompt;
                        // its platform API does not expose the initial path.
                        let _ = (directory, must_match, initial);
                        let options = PathPromptOptions {
                            files: !directory_only,
                            directories: directory_only,
                            multiple: false,
                            prompt: Some(prompt.into()),
                        };
                        let result = match cx.update(|app| app.prompt_for_paths(options)) {
                            Ok(response) => match response.await {
                                Ok(Ok(Some(paths))) => Ok(paths
                                    .into_iter()
                                    .next()
                                    .map(|path| path.to_string_lossy().into_owned())),
                                Ok(Ok(None)) => Ok(None),
                                Ok(Err(error)) => Err(error.to_string()),
                                Err(error) => Err(error.to_string()),
                            },
                            Err(error) => Err(error.to_string()),
                        };
                        let _ = reply.send(result);
                    }
                }
            }
            let _ = cx.update(|app| app.quit());
        })
        .detach();
    });
    match open_error.lock().unwrap().take() {
        Some(message) => Err(io::Error::new(io::ErrorKind::Other, message)),
        None => Ok(()),
    }
}

fn frame_window_options(
    frame: &FrameSnapshot,
    windows: &HashMap<usize, WindowHandle<EditorView>>,
    cx: &mut AsyncApp,
) -> WindowOptions {
    let child = frame.parent_frame.is_some();
    let content_size = size(
        px((frame.width.max(10) as f32 * 8.0).max(80.0)),
        px((frame.height.max(3) as f32 * 19.0).max(57.0)),
    );
    let base_window = frame
        .parent_frame
        .and_then(|parent| windows.get(&parent))
        .or_else(|| windows.values().next());
    let parent_bounds = base_window
        .and_then(|parent| parent.update(cx, |_view, window, _cx| window.bounds()).ok());
    let bounds = if let Some(parent_bounds) = parent_bounds {
        if child {
            let left = frame.left.map(|value| value as f32).unwrap_or_else(|| {
                (f32::from(parent_bounds.size.width) - f32::from(content_size.width)) / 2.0
            });
            let top = frame.top.map(|value| value as f32).unwrap_or_else(|| {
                (f32::from(parent_bounds.size.height) - f32::from(content_size.height)) / 2.0
            });
            Bounds::new(
                parent_bounds.origin + point(px(left), px(top)),
                content_size,
            )
        } else {
            Bounds::new(
                parent_bounds.origin + point(px(36.0), px(36.0)),
                content_size,
            )
        }
    } else {
        Bounds::new(point(px(80.0), px(80.0)), content_size)
    };
    let window_bounds = match frame.fullscreen.as_deref() {
        Some("maximized") => WindowBounds::Maximized(bounds),
        Some("fullboth" | "fullwidth" | "fullheight") => WindowBounds::Fullscreen(bounds),
        _ => WindowBounds::Windowed(bounds),
    };
    WindowOptions {
        window_bounds: Some(window_bounds),
        kind: if child {
            WindowKind::Floating
        } else {
            WindowKind::Normal
        },
        titlebar: if child || frame.undecorated {
            None
        } else {
            Some(TitlebarOptions {
                title: Some(frame.name.clone().into()),
                ..Default::default()
            })
        },
        focus: false,
        is_movable: !child,
        is_resizable: !child,
        is_minimizable: !child,
        ..Default::default()
    }
}

fn apply_frame_window_state(window: &Window, fullscreen: Option<&str>) {
    match fullscreen {
        Some("maximized") => {
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            if !window.is_maximized() {
                window.zoom_window();
            }
        }
        Some("fullboth" | "fullwidth" | "fullheight") => {
            if window.is_maximized() {
                window.zoom_window();
            }
            if !window.is_fullscreen() {
                window.toggle_fullscreen();
            }
        }
        _ => {
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            if window.is_maximized() {
                window.zoom_window();
            }
        }
    }
}

fn native_frame_visible(window: &Window) -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        let native_window = appkit_native_window(window)?;
        unsafe {
            let minimized: BOOL = msg_send![native_window, isMiniaturized];
            Some(minimized != YES)
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        None
    }
}

fn show_frame_window(window: &Window) {
    #[cfg(target_os = "macos")]
    if let Some(native_window) = appkit_native_window(window) {
        unsafe {
            let minimized: BOOL = msg_send![native_window, isMiniaturized];
            if minimized == YES {
                let _: () = msg_send![native_window, deminiaturize: nil];
            }
        }
    }
    window.activate_window();
}

#[cfg(target_os = "macos")]
fn register_power_off_listener(event_tx: std::sync::mpsc::Sender<GuiEvent>) {
    let workspace: id = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    let notification_center: id = unsafe { msg_send![workspace, notificationCenter] };
    let name = unsafe { NSString::alloc(nil).init_str("NSWorkspaceWillPowerOffNotification") };
    let handler = block::ConcreteBlock::new(move |_notification: id| {
        let _ = event_tx.send(GuiEvent::PowerOff);
    })
    .copy();
    let _: id = unsafe {
        msg_send![notification_center, addObserverForName:name object:nil queue:nil usingBlock:handler]
    };
}

fn raise_frame_window(window: &Window) {
    window.activate_window();
}

fn lower_frame_window(window: &Window) {
    #[cfg(target_os = "macos")]
    if let Some(native_window) = appkit_native_window(window) {
        unsafe {
            let _: () = msg_send![native_window, orderBack: nil];
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

fn set_frame_z_group_window(window: &Window, z_group: Option<&str>, child: bool) {
    #[cfg(target_os = "macos")]
    if let Some(native_window) = appkit_native_window(window) {
        let level: NSInteger = match z_group {
            Some("above") if child => 101,
            Some("above") => 3,
            Some("below") => 0,
            _ if child => 101,
            _ => 0,
        };
        unsafe {
            let _: () = msg_send![native_window, setLevel: level];
            match z_group {
                Some("above") => {
                    let _: () = msg_send![native_window, orderFront: nil];
                }
                Some("below") => {
                    let _: () = msg_send![native_window, orderBack: nil];
                }
                _ => {}
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    match z_group {
        Some("above") => window.activate_window(),
        Some("below") => lower_frame_window(window),
        _ => {
            let _ = (window, child);
        }
    }
}

#[cfg(target_os = "macos")]
fn appkit_native_window(window: &Window) -> Option<id> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    unsafe {
        let view = handle.ns_view.as_ptr() as id;
        let native_window: id = msg_send![view, window];
        (!native_window.is_null()).then_some(native_window)
    }
}

fn show_native_color_panel(window: &Window) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        appkit_native_window(window)
            .ok_or_else(|| "native frame window is unavailable".to_owned())?;
        window.activate_window();
        unsafe {
            let panel: id = msg_send![class!(NSColorPanel), sharedColorPanel];
            if panel.is_null() {
                return Err("NSColorPanel could not be created".to_owned());
            }
            let _: () = msg_send![panel, makeKeyAndOrderFront: nil];
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err("NS color panel is available only on macOS".to_owned())
    }
}

fn show_native_character_palette(window: &Window) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        window.activate_window();
        unsafe {
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, orderFrontCharacterPalette: nil];
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err("Character Viewer is available only on macOS".to_owned())
    }
}

#[cfg(test)]
use keys::{command_keystroke, keystroke_to_code};
#[cfg(test)]
use logic::{ChanIo, GuiEvent, start_logic, start_logic_with_dump_for_test};
#[cfg(test)]
use view::EditorView;
#[cfg(test)]
mod tests;
