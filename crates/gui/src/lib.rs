//! GPUI frontend. The evaluator lives on a separate logic thread.
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
mod accessibility;
#[cfg(target_os = "macos")]
mod applescript;
#[cfg(target_os = "macos")]
use cocoa::appkit::{NSPasteboard, NSPasteboardTypeString, NSStringPboardType};
#[cfg(target_os = "macos")]
use cocoa::base::{NO, id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSArray, NSInteger, NSPoint, NSRect, NSSize, NSString};
#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSBeep();
    fn NSPerformService(service_name: id, pasteboard: id) -> objc::runtime::BOOL;
}
#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBeep(u_type: u32) -> i32;
}
#[cfg(target_os = "macos")]
use objc::{class, msg_send, runtime::{BOOL, YES}, sel, sel_impl};
mod keys;
mod logic;
mod monitor;
mod view;
use logic::{ClipboardRequest, FrameSnapshot, GuiEvent, GuiRequest, start_logic_with_frames};
use view::{EditorView, build_managed_editor_view};

#[cfg(target_os = "macos")]
fn perform_ns_service(service: &str, send: Option<&str>) -> Result<Option<String>, String> {
    use std::ffi::CStr;

    let service_name: id =
        unsafe { msg_send![NSString::alloc(nil).init_str(service), autorelease] };
    let pasteboard = unsafe { NSPasteboard::pasteboardWithUniqueName(nil) };
    if pasteboard == nil {
        return Err("Unable to create a service pasteboard".to_owned());
    }

    let send_types = unsafe {
        NSArray::arrayWithObjects(nil, &[NSPasteboardTypeString, NSStringPboardType])
    };
    if let Some(send) = send {
        let send: id = unsafe { msg_send![NSString::alloc(nil).init_str(send), autorelease] };
        unsafe {
            pasteboard.declareTypes_owner(send_types, nil);
            pasteboard.setString_forType(send, NSPasteboardTypeString);
            pasteboard.setString_forType(send, NSStringPboardType);
        }
    } else {
        unsafe {
            pasteboard.clearContents();
        }
    }

    if unsafe { NSPerformService(service_name, pasteboard) } == NO {
        return Err("service not available".to_owned());
    }
    if unsafe { NSArray::count(pasteboard.types()) } == 0 {
        return Ok(Some(String::new()));
    }

    let return_type = unsafe { pasteboard.availableTypeFromArray(send_types) };
    if return_type == nil {
        return Ok(None);
    }
    let string = unsafe { pasteboard.stringForType(return_type) };
    if string != nil {
        let bytes = unsafe { NSString::UTF8String(string) };
        if !bytes.is_null() {
            return Ok(Some(
                unsafe { CStr::from_ptr(bytes) }
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
    }
    let data = unsafe { pasteboard.dataForType(return_type) };
    if data == nil {
        return Ok(None);
    }
    let bytes: *const u8 = unsafe { msg_send![data, bytes] };
    let length: usize = unsafe { msg_send![data, length] };
    if bytes.is_null() {
        return Ok(None);
    }
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
    Ok(String::from_utf8(bytes.to_vec()).ok())
}

/// Launch the gpui front-end. `files` are visited via `find-file`
/// once the evaluator boots on the logic thread.
#[cfg(target_os = "macos")]
static DOCK_ATTENTION_REQUEST_ID: Mutex<NSInteger> = Mutex::new(-1);

#[cfg(target_os = "macos")]
fn set_dock_badge(badge: Option<&str>) -> Result<(), String> {
    let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    let dock: id = unsafe { msg_send![app, dockTile] };
    if dock == nil {
        return Ok(());
    }
    let badge: id = badge.map_or(nil, |badge| unsafe {
        msg_send![NSString::alloc(nil).init_str(badge), autorelease]
    });
    unsafe {
        let _: () = msg_send![dock, setBadgeLabel: badge];
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_dock_progress(progress: Option<f64>) -> Result<(), String> {
    let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    let dock: id = unsafe { msg_send![app, dockTile] };
    if dock == nil {
        return Ok(());
    }
    let content: id = unsafe { msg_send![dock, contentView] };
    let mut indicator: id = nil;
    if content != nil {
        let subviews: id = unsafe { msg_send![content, subviews] };
        let count: usize = unsafe { msg_send![subviews, count] };
        if count > 0 {
            let last: id = unsafe { msg_send![subviews, lastObject] };
            let is_indicator: BOOL =
                unsafe { msg_send![last, isKindOfClass: class!(NSLevelIndicator)] };
            if is_indicator == YES {
                indicator = last;
            }
        }
    }
    let content: id = if content == nil {
        let image_view: id = unsafe { msg_send![class!(NSImageView), alloc] };
        let image_view: id = unsafe { msg_send![image_view, init] };
        let icon: id = unsafe { msg_send![app, applicationIconImage] };
        unsafe {
            let _: () = msg_send![image_view, setImage: icon];
            let _: () = msg_send![dock, setContentView: image_view];
            let _: id = msg_send![image_view, autorelease];
        }
        image_view
    } else {
        content
    };
    if indicator == nil {
        let icon: id = unsafe { msg_send![app, applicationIconImage] };
        let size: NSSize = unsafe { msg_send![icon, size] };
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(size.width, 0.10 * size.height),
        );
        let allocated: id = unsafe { msg_send![class!(NSLevelIndicator), alloc] };
        indicator = unsafe { msg_send![allocated, initWithFrame: frame] };
        unsafe {
            let _: () = msg_send![indicator, setWantsLayer: YES];
            let _: () = msg_send![indicator, setEnabled: NO];
            let _: () = msg_send![indicator, setLevelIndicatorStyle: 1usize];
            let accent: id = msg_send![class!(NSColor), controlAccentColor];
            let _: () = msg_send![indicator, setFillColor: accent];
            let _: () = msg_send![indicator, setMinValue: 0.0_f64];
            let _: () = msg_send![indicator, setMaxValue: 1.0_f64];
            let _: () = msg_send![content, addSubview: indicator];
            let _: id = msg_send![indicator, autorelease];
        }
    }
    let value = progress.filter(|value| value.is_finite() && (0.0..=1.0).contains(value));
    unsafe {
        let _: () = msg_send![indicator, setDoubleValue: value.unwrap_or(0.0)];
        let _: () = msg_send![indicator, setHidden: if value.is_some() { NO } else { YES }];
        let _: () = msg_send![dock, display];
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn request_dock_user_attention(
    urgency: Option<remacs_runtime::frontend::UserAttentionUrgency>,
) -> Result<(), String> {
    let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    if app == nil {
        return Ok(());
    }
    let mut request = DOCK_ATTENTION_REQUEST_ID
        .lock()
        .map_err(|_| "Unable to access Dock attention state".to_owned())?;
    if *request != -1 {
        unsafe {
            let _: () = msg_send![app, cancelUserAttentionRequest: *request];
        }
        *request = -1;
    }
    if let Some(urgency) = urgency {
        let kind = match urgency {
            remacs_runtime::frontend::UserAttentionUrgency::Critical => 0,
            remacs_runtime::frontend::UserAttentionUrgency::Informational => 10,
        };
        *request = unsafe { msg_send![app, requestUserAttention: kind] };
    }
    Ok(())
}

pub fn run_gui(files: &[String]) -> io::Result<()> {
    run_gui_with_dump(files, None, &[])
}

pub fn run_gui_with_dump(
    files: &[String],
    dump_file: Option<&str>,
    arguments: &[String],
) -> io::Result<()> {
    let (event_tx, gui_requests, clipboard_requests, initial_frame_size) = start_logic_with_frames(
        files.to_vec(),
        dump_file.map(str::to_owned),
        arguments.to_vec(),
    )?;
    // Use explicit runtime dimensions from the user's frame alists before
    // opening the native window. The first frame snapshot corrects this
    // estimate using actual font metrics.
    let initial_window_size = initial_frame_size
        .map(|frame| {
            size(
                px(frame.width.max(10) as f32 * 8.0),
                px(frame.height.max(3) as f32 * 19.0),
            )
        })
        .unwrap_or(size(px(960.0), px(640.0)));

    let open_error = Arc::new(Mutex::new(None));
    let app_open_error = Arc::clone(&open_error);
    let application = Application::new();
    let open_file_tx = event_tx.clone();
    application.on_open_files(move |files| {
        let files = files
            .into_iter()
            .map(|file| (file.path.to_string_lossy().into_owned(), file.line))
            .collect();
        let _ = open_file_tx.send(GuiEvent::OpenFiles(files));
    });
    application.run(move |cx: &mut App| {
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
                    initial_window_size,
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
            let mut window_backgrounds = HashMap::<usize, WindowBackgroundAppearance>::new();
            let mut window_titlebars = HashMap::<usize, (bool, bool)>::new();
            let mut window_appearances = HashMap::<usize, Option<WindowAppearance>>::new();
            let mut window_represented_files = HashMap::<usize, Option<String>>::new();
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
                            let modifier_mapping = snapshot.modifier_mapping;
                            let alpha_active = snapshot.alpha_active;
                            let alpha_inactive = snapshot.alpha_inactive;
                            let alpha_background = snapshot.alpha_background;
                            let internal_border_width = snapshot.internal_border_width;
                            let background_appearance = frame_background_appearance(&snapshot);
                            let ns_appearance = snapshot.ns_appearance;
                            let represented_file = snapshot.represented_file.clone();
                            let titlebar_appearance = (
                                snapshot.parent_frame.is_none() && !snapshot.undecorated,
                                snapshot.ns_transparent_titlebar,
                            );
                            let fullscreen = snapshot.fullscreen.clone();
                            let frame_size = (snapshot.width, snapshot.height);
                            let frame_left = snapshot.left;
                            let frame_top = snapshot.top;
                            let parent_bounds = snapshot
                                .parent_frame
                                .and_then(|parent| windows.get(&parent))
                                .and_then(|parent| {
                                    parent
                                        .update(cx, |_view, window, _cx| window.bounds())
                                        .ok()
                                });
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
                                let requested_parent_bounds = parent_bounds;
                                let update_behavior =
                                    frame_behaviors.get(&frame_id) != Some(&behavior);
                                let update_background =
                                    window_backgrounds.get(&frame_id) != Some(&background_appearance);
                                let update_titlebar =
                                    window_titlebars.get(&frame_id) != Some(&titlebar_appearance);
                                let update_appearance =
                                    window_appearances.get(&frame_id) != Some(&ns_appearance);
                                let update_represented_file =
                                    window_represented_files.get(&frame_id)
                                        != Some(&represented_file);
                                let requested_behavior = behavior.clone();
                                let requested_represented_file = represented_file.clone();
                                let _ = handle.update(cx, move |view, window, cx| {
                                    let modifier_mapping_changed =
                                        view.set_modifier_mapping(modifier_mapping);
                                    let internal_border_changed =
                                        view.set_internal_border_width(internal_border_width);
                                    let alpha_background_changed =
                                        view.set_alpha_background(alpha_background);
                                    if update_appearance {
                                        window.set_window_appearance(ns_appearance);
                                    }
                                    let appearance_changed = view.set_appearance(window.appearance());
                                    if update_represented_file {
                                        window.set_represented_file(requested_represented_file);
                                    }
                                    if view.grid != grid || grid.blink_cursor {
                                        view.grid = grid;
                                        cx.notify();
                                    } else if modifier_mapping_changed
                                        || internal_border_changed
                                        || alpha_background_changed
                                        || appearance_changed
                                    {
                                        cx.notify();
                                    }
                                    window.set_opacity(frame_opacity(
                                        alpha_active,
                                        alpha_inactive,
                                        window.is_window_active(),
                                    ));
                                    if update_background {
                                        window.set_background_appearance(background_appearance);
                                    }
                                    if update_titlebar {
                                        window.set_titlebar_appearance(
                                            titlebar_appearance.0,
                                            titlebar_appearance.1,
                                        );
                                    }
                                    if requested_fullscreen.is_none() {
                                        apply_frame_snapshot_bounds(
                                            view,
                                            window,
                                            cx,
                                            frame_size.0,
                                            frame_size.1,
                                            frame_left,
                                            frame_top,
                                            requested_parent_bounds,
                                        );
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
                                window_backgrounds.insert(frame_id, background_appearance);
                                window_titlebars.insert(frame_id, titlebar_appearance);
                                window_appearances.insert(frame_id, ns_appearance);
                                window_represented_files.insert(frame_id, represented_file);
                                native_visibility.entry(frame_id).or_insert(true);
                                continue;
                            }
                            if let Some(handle) = primary_window.take() {
                                let initial_fullscreen = fullscreen.clone();
                                let initial_behavior = behavior.clone();
                                let initial_modifier_mapping = modifier_mapping;
                                let initial_internal_border_width = internal_border_width;
                                let initial_alpha_active = alpha_active;
                                let initial_alpha_inactive = alpha_inactive;
                                let initial_alpha_background = alpha_background;
                                let initial_background_appearance = background_appearance;
                                let initial_ns_appearance = ns_appearance;
                                let initial_represented_file = represented_file.clone();
                                let represented_file_for_window = represented_file.clone();
                                let initial_titlebar_appearance = titlebar_appearance;
                                let initial_left = frame_left;
                                let initial_top = frame_top;
                                let _ = handle.update(cx, move |view, window, cx| {
                                    view.set_frame_id(frame_id);
                                    view.grid = grid;
                                    view.set_modifier_mapping(initial_modifier_mapping);
                                    view.set_internal_border_width(initial_internal_border_width);
                                    view.set_alpha_background(initial_alpha_background);
                                    window.set_opacity(frame_opacity(
                                        initial_alpha_active,
                                        initial_alpha_inactive,
                                        window.is_window_active(),
                                    ));
                                    window.set_background_appearance(
                                        initial_background_appearance,
                                    );
                                    window.set_titlebar_appearance(
                                        initial_titlebar_appearance.0,
                                        initial_titlebar_appearance.1,
                                    );
                                    window.set_window_appearance(initial_ns_appearance);
                                    view.set_appearance(window.appearance());
                                    window.set_represented_file(represented_file_for_window);
                                    if initial_fullscreen.is_none() {
                                        apply_frame_snapshot_bounds(
                                            view,
                                            window,
                                            cx,
                                            frame_size.0,
                                            frame_size.1,
                                            initial_left,
                                            initial_top,
                                            None,
                                        );
                                    }
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
                                window_backgrounds.insert(frame_id, initial_background_appearance);
                                window_titlebars.insert(frame_id, initial_titlebar_appearance);
                                window_appearances.insert(frame_id, initial_ns_appearance);
                                window_represented_files
                                    .insert(frame_id, initial_represented_file);
                                native_visibility.insert(frame_id, true);
                                windows.insert(frame_id, handle);
                                continue;
                            }

                            let options = frame_window_options(&snapshot, &windows, cx);
                            let modifier_mapping = snapshot.modifier_mapping;
                            let alpha_active = snapshot.alpha_active;
                            let alpha_inactive = snapshot.alpha_inactive;
                            let alpha_background = snapshot.alpha_background;
                            let background_appearance = frame_background_appearance(&snapshot);
                            let ns_appearance = snapshot.ns_appearance;
                            let represented_file = snapshot.represented_file.clone();
                            let represented_file_for_window = represented_file.clone();
                            let titlebar_appearance = (
                                snapshot.parent_frame.is_none() && !snapshot.undecorated,
                                snapshot.ns_transparent_titlebar,
                            );
                            let event_tx = event_tx.clone();
                            let handle = match cx.open_window(options, move |window, app| {
                                window.set_opacity(frame_opacity(
                                    alpha_active,
                                    alpha_inactive,
                                    window.is_window_active(),
                                ));
                                window.set_background_appearance(background_appearance);
                                window.set_titlebar_appearance(
                                    titlebar_appearance.0,
                                    titlebar_appearance.1,
                                );
                                window.set_window_appearance(ns_appearance);
                                window.set_represented_file(represented_file_for_window);
                                let window_appearance = window.appearance();
                                app.new(|cx| {
                                    let mut view = build_managed_editor_view(
                                        window,
                                        cx,
                                        event_tx,
                                        Some(frame_id),
                                        Some(grid),
                                    );
                                    view.set_modifier_mapping(modifier_mapping);
                                    view.set_alpha_background(alpha_background);
                                    view.set_appearance(window_appearance);
                                    view
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
                            window_backgrounds.insert(frame_id, background_appearance);
                            window_titlebars.insert(frame_id, titlebar_appearance);
                            window_appearances.insert(frame_id, ns_appearance);
                            window_represented_files.insert(frame_id, represented_file);
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
                            window_backgrounds.remove(&frame_id);
                            window_titlebars.remove(&frame_id);
                            window_appearances.remove(&frame_id);
                            window_represented_files.remove(&frame_id);
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
                    GuiRequest::WarpPointer {
                        frame_id,
                        x,
                        y,
                        absolute,
                        reply,
                    } => {
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            handle
                                .update(cx, |_view, window, cx| {
                                    monitor::warp_mouse_pointer(window, cx, x, y, absolute)
                                })
                                .map_err(|error| error.to_string())
                                .and_then(|result| result)
                        } else {
                            Err(format!("GUI frame {frame_id} is not open"))
                        };
                        let _ = reply.send(result);
                    }
                    GuiRequest::Bell { frame_id, visible } => {
                        if visible {
                            let handle = frame_id
                                .and_then(|frame_id| windows.get(&frame_id).copied())
                                .or_else(|| windows.values().next().copied());
                            if handle
                                .and_then(|handle| {
                                    handle
                                        .update(cx, |view, window, cx| {
                                            view.flash_bell(window, cx)
                                        })
                                        .ok()
                                })
                                .is_none()
                            {
                                ring_native_bell();
                            }
                        } else {
                            ring_native_bell();
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
                    GuiRequest::AboutPanel {
                        frame_id,
                        version,
                        reply,
                    } => {
                        let result = if windows.contains_key(&frame_id) {
                            show_native_about_panel(&version)
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
                    GuiRequest::HideApp { hide, reply } => {
                        let result = cx
                            .update(|app| {
                                if hide {
                                    app.hide();
                                } else {
                                    app.activate(false);
                                }
                            })
                            .map_err(|error| error.to_string());
                        let _ = reply.send(result);
                    }
                    GuiRequest::HideOtherApps(reply) => {
                        let result = cx
                            .update(|app| app.hide_other_apps())
                            .map_err(|error| error.to_string());
                        let _ = reply.send(result);
                    }
                    #[cfg(target_os = "macos")]
                    GuiRequest::AppleScript { script, reply } => {
                        let result = cx
                            .update(|_app| applescript::execute(&script))
                            .unwrap_or_else(|error| Err(error.to_string()));
                        let _ = reply.send(result);
                    }
                    #[cfg(target_os = "macos")]
                    GuiRequest::PerformService {
                        service,
                        send,
                        reply,
                    } => {
                        let result = cx
                            .update(|_app| perform_ns_service(&service, send.as_deref()))
                            .map_err(|error| error.to_string())
                            .and_then(|result| result);
                        let _ = reply.send(result);
                    }
                    #[cfg(target_os = "macos")]
                    GuiRequest::DockBadge { badge, reply } => {
                        let result = cx
                            .update(|_app| set_dock_badge(badge.as_deref()))
                            .map_err(|error| error.to_string())
                            .and_then(|result| result);
                        let _ = reply.send(result);
                    }
                    #[cfg(target_os = "macos")]
                    GuiRequest::DockProgress { progress, reply } => {
                        let result = cx
                            .update(|_app| set_dock_progress(progress))
                            .map_err(|error| error.to_string())
                            .and_then(|result| result);
                        let _ = reply.send(result);
                    }
                    #[cfg(target_os = "macos")]
                    GuiRequest::UserAttention { urgency, reply } => {
                        let result = cx
                            .update(|_app| request_dock_user_attention(urgency))
                            .map_err(|error| error.to_string())
                            .and_then(|result| result);
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
                    GuiRequest::BeginTextDrag {
                        frame_id,
                        text,
                        action,
                        started,
                        completed,
                    } => {
                        let operation = match action {
                            remacs_runtime::frontend::DragAction::Copy => {
                                ExternalDragOperation::Copy
                            }
                            remacs_runtime::frontend::DragAction::Move => {
                                ExternalDragOperation::Move
                            }
                            remacs_runtime::frontend::DragAction::Link => {
                                ExternalDragOperation::Link
                            }
                            remacs_runtime::frontend::DragAction::Private => {
                                ExternalDragOperation::Generic
                            }
                        };
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            handle
                                .update(cx, move |_view, window, _cx| {
                                    window.begin_text_drag_session(
                                        text,
                                        operation,
                                        move |operation| {
                                            let operation = operation.map(|operation| match operation {
                                                ExternalDragOperation::Copy => {
                                                    remacs_runtime::frontend::DragAction::Copy
                                                }
                                                ExternalDragOperation::Move => {
                                                    remacs_runtime::frontend::DragAction::Move
                                                }
                                                ExternalDragOperation::Link => {
                                                    remacs_runtime::frontend::DragAction::Link
                                                }
                                                ExternalDragOperation::Generic => {
                                                    remacs_runtime::frontend::DragAction::Private
                                                }
                                            });
                                            let _ = completed.send(operation);
                                        },
                                    )
                                })
                                .map_err(|error| error.to_string())
                                .and_then(|result| result)
                        } else {
                            Err(format!("GUI frame {frame_id} is not open"))
                        };
                        let _ = started.send(result);
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
    let internal_border = frame.internal_border_width as f32 * 2.0;
    let extra_row_height = frame
        .grid
        .row_heights
        .iter()
        .map(|height| height.saturating_sub(frame.grid.default_line_height.max(1)))
        .sum::<usize>();
    let content_size = size(
        px((frame.width.max(10) as f32 * 8.0).max(80.0) + internal_border),
        px((frame.height.max(3) as f32 * 19.0)
            .max(57.0)
            + extra_row_height as f32
            + internal_border),
    );
    let base_window = frame
        .parent_frame
        .and_then(|parent| windows.get(&parent))
        .or_else(|| windows.values().next());
    let parent_bounds = base_window
        .and_then(|parent| parent.update(cx, |_view, window, _cx| window.bounds()).ok());
    let parameter_position = frame.left.zip(frame.top).map(|(left, top)| {
        if child {
            let position = parent_bounds
                .map(|bounds| bounds.origin + point(px(left as f32), px(top as f32)))
                .unwrap_or_else(|| point(px(left as f32), px(top as f32)));
            let display_id = base_window
                .and_then(|base| {
                    base.update(cx, |_view, window, cx| {
                        window.display(cx).map(|display| u32::from(display.id()))
                    })
                    .ok()
                })
                .flatten();
            (position, display_id)
        } else {
            base_window
                .and_then(|base| {
                    base.update(cx, |_view, window, cx| {
                        frame_position_in_window(left, top, None, window, cx)
                    })
                    .ok()
                })
                .unwrap_or_else(|| {
                    (
                        point(px(left as f32), px(top as f32)),
                        None,
                    )
                })
        }
    });
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
        } else if let Some((position, _)) = parameter_position {
            Bounds::new(position, content_size)
        } else if frame.left.is_some() || frame.top.is_some() {
            let default_origin = parent_bounds.origin + point(px(36.0), px(36.0));
            Bounds::new(
                point(
                    frame.left.map(|value| px(value as f32)).unwrap_or(default_origin.x),
                    frame.top.map(|value| px(value as f32)).unwrap_or(default_origin.y),
                ),
                content_size,
            )
        } else {
            Bounds::new(
                parent_bounds.origin + point(px(36.0), px(36.0)),
                content_size,
            )
        }
    } else {
        Bounds::new(
            parameter_position
                .map(|(position, _)| position)
                .unwrap_or(point(px(80.0), px(80.0))),
            content_size,
        )
    };
    let window_bounds = match frame.fullscreen.as_deref() {
        Some("maximized") => WindowBounds::Maximized(bounds),
        Some("fullboth" | "fullwidth" | "fullheight") => WindowBounds::Fullscreen(bounds),
        _ => WindowBounds::Windowed(bounds),
    };
    WindowOptions {
        window_bounds: Some(window_bounds),
        window_background: frame_background_appearance(frame),
        display_id: parameter_position
            .and_then(|(_, display_id)| display_id)
            .map(DisplayId::from),
        kind: if child {
            WindowKind::Floating
        } else {
            WindowKind::Normal
        },
        window_decorations: (child || frame.undecorated).then_some(WindowDecorations::Client),
        titlebar: if child || frame.undecorated {
            None
        } else {
            Some(TitlebarOptions {
                title: Some(frame.name.clone().into()),
                titlebar_appears_transparent: frame.ns_transparent_titlebar,
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

fn frame_background_appearance(frame: &FrameSnapshot) -> WindowBackgroundAppearance {
    if frame.alpha_background < 100 || frame.alpha_active < 100 || frame.alpha_inactive < 100 {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

fn frame_opacity(alpha_active: u8, alpha_inactive: u8, active: bool) -> f32 {
    f32::from(if active { alpha_active } else { alpha_inactive }) / 100.0
}

fn frame_position_in_window(
    left: i128,
    top: i128,
    parent_bounds: Option<Bounds<Pixels>>,
    window: &Window,
    cx: &App,
) -> (Point<Pixels>, Option<u32>) {
    if let Some(parent_bounds) = parent_bounds {
        return (
            parent_bounds.origin + point(px(left as f32), px(top as f32)),
            window.display(cx).map(|display| u32::from(display.id())),
        );
    }
    #[cfg(target_os = "macos")]
    {
        let monitors = monitor::monitor_geometry(window, cx);
        let display_id = window.display(cx).map(|display| u32::from(display.id()));
        let monitor = monitors
            .iter()
            .find(|monitor| {
                let [x, y, width, height] = monitor.geometry;
                left >= x
                    && left < x.saturating_add(width)
                    && top >= y
                    && top < y.saturating_add(height)
            })
            .or_else(|| {
                display_id.and_then(|display_id| {
                    monitors
                        .iter()
                        .find(|monitor| monitor.display_id == display_id)
                })
            });
        if let Some(monitor) = monitor {
            return (
                point(
                    px(left.saturating_sub(monitor.native_origin[0]) as f32),
                    px(
                        top.saturating_sub(
                            monitor.geometry[1].saturating_sub(monitor.native_origin[1]),
                        ) as f32,
                    ),
                ),
                Some(monitor.display_id),
            );
        }
    }
    (
        point(px(left as f32), px(top as f32)),
        window.display(cx).map(|display| u32::from(display.id())),
    )
}

fn apply_frame_snapshot_bounds(
    view: &mut EditorView,
    window: &mut Window,
    cx: &mut Context<EditorView>,
    width: usize,
    height: usize,
    left: Option<i128>,
    top: Option<i128>,
    parent_bounds: Option<Bounds<Pixels>>,
) {
    if view.needs_frame_size_update(width, height) {
        view.resize_to_cells(width, height, window);
    }
    let position_changed = view.needs_frame_position_update(left, top);
    if position_changed
        && let (Some(left), Some(top)) = (left, top)
        && view.native_position() != Some((left, top))
    {
        let (position, display_id) = frame_position_in_window(
            left,
            top,
            parent_bounds,
            window,
            cx,
        );
        window.set_position(position, display_id);
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

fn ring_native_bell() {
    #[cfg(target_os = "macos")]
    unsafe {
        NSBeep();
    }
    #[cfg(target_os = "windows")]
    unsafe {
        let _ = MessageBeep(0);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use std::io::Write;
        let _ = std::io::stderr().write_all(b"\x07");
    }
}

#[cfg(target_os = "macos")]
fn show_native_about_panel(version: &str) -> Result<(), String> {
    unsafe {
        let options: id = msg_send![class!(NSMutableDictionary), dictionary];
        let name_key: id = msg_send![
            NSString::alloc(nil).init_str("ApplicationName"),
            autorelease
        ];
        let name: id = msg_send![NSString::alloc(nil).init_str("Remacs"), autorelease];
        let version_key: id =
            msg_send![NSString::alloc(nil).init_str("Version"), autorelease];
        let version_value: id =
            msg_send![NSString::alloc(nil).init_str(version), autorelease];
        let app_version_key: id =
            msg_send![NSString::alloc(nil).init_str("ApplicationVersion"), autorelease];
        let app_version: id =
            msg_send![NSString::alloc(nil).init_str(version), autorelease];
        let _: () = msg_send![options, setObject: name forKey: name_key];
        let _: () = msg_send![options, setObject: version_value forKey: version_key];
        let _: () = msg_send![options, setObject: app_version forKey: app_version_key];
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, orderFrontStandardAboutPanelWithOptions: options];
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn show_native_about_panel(_version: &str) -> Result<(), String> {
    Err("The native About panel is available only on macOS".to_owned())
}

#[cfg(test)]
use keys::{command_keystroke, keystroke_to_code};
#[cfg(test)]
use logic::{ChanIo, GuiEvent, start_logic, start_logic_with_dump_for_test};
#[cfg(test)]
use view::EditorView;
#[cfg(test)]
mod tests;
