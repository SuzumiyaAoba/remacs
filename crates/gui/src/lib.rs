//! GPUI frontend. The evaluator lives on a separate logic thread.
use gpui::*;
use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::OnceLock;
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
    fn GetWindowLongW(hwnd: *mut std::ffi::c_void, index: i32) -> i32;
    fn SetWindowLongW(hwnd: *mut std::ffi::c_void, index: i32, value: i32) -> i32;
    fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        insert_after: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn GetWindowRect(hwnd: *mut std::ffi::c_void, rect: *mut Win32Rect) -> i32;
    fn GetClientRect(hwnd: *mut std::ffi::c_void, rect: *mut Win32Rect) -> i32;
    fn CallWindowProcW(
        previous: isize,
        hwnd: *mut std::ffi::c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    fn DefWindowProcW(
        hwnd: *mut std::ffi::c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    #[cfg(target_pointer_width = "64")]
    fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32) -> isize;
    #[cfg(target_pointer_width = "64")]
    fn SetWindowLongPtrW(hwnd: *mut std::ffi::c_void, index: i32, value: isize) -> isize;
}
#[cfg(target_os = "macos")]
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{
        BOOL, Class, Imp, Method, Object, Sel, YES, class_getInstanceMethod,
        method_setImplementation, object_getClass,
    },
    sel, sel_impl,
};
mod keys;
mod logic;
mod monitor;
mod view;
use logic::{ClipboardRequest, FrameSnapshot, GuiEvent, GuiRequest, start_logic_with_frames};
use view::{EditorView, build_managed_editor_view};

#[cfg(target_os = "macos")]
static GPUI_CAN_BECOME_KEY_ORIGINALS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
#[cfg(target_os = "macos")]
static NO_ACCEPT_FOCUS_ASSOCIATED_KEY: u8 = 0;
#[cfg(target_os = "windows")]
static ORIGINAL_WINDOW_EX_STYLES: OnceLock<Mutex<HashMap<isize, i32>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static WINDOWS_MINIMUM_SIZES: OnceLock<Mutex<HashMap<isize, WindowsMinimumSizeState>>> =
    OnceLock::new();

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Win32Point {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Win32Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct Win32MinMaxInfo {
    reserved: Win32Point,
    max_size: Win32Point,
    max_position: Win32Point,
    min_track_size: Win32Point,
    max_track_size: Win32Point,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct WindowsMinimumSizeState {
    original_window_proc: isize,
    min_width: Option<i32>,
    min_height: Option<i32>,
    nonclient_width: i32,
    nonclient_height: i32,
}

#[cfg(target_os = "macos")]
#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getAssociatedObject(object: id, key: *const std::ffi::c_void) -> id;
    fn objc_setAssociatedObject(object: id, key: *const std::ffi::c_void, value: id, policy: usize);
}

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
static NATIVE_SERVICE_EVENT_SENDER: Mutex<Option<std::sync::mpsc::Sender<GuiEvent>>> =
    Mutex::new(None);
#[cfg(target_os = "macos")]
static NATIVE_SERVICE_PROVIDER_CLASS: OnceLock<usize> = OnceLock::new();
#[cfg(target_os = "macos")]
static NATIVE_SERVICE_PROVIDER: Mutex<usize> = Mutex::new(0);

#[cfg(target_os = "macos")]
fn native_service_provider_class() -> &'static Class {
    let class = *NATIVE_SERVICE_PROVIDER_CLASS.get_or_init(|| {
        let mut declaration = ClassDecl::new("RemacsNativeServiceProvider", class!(NSObject))
            .expect("RemacsNativeServiceProvider must be registered once");
        unsafe {
            declaration.add_method(
                sel!(requestService:userData:error:),
                receive_native_service
                    as extern "C" fn(&Object, Sel, id, id, *mut Object),
            );
        }
        declaration.register() as *const Class as usize
    });
    unsafe { &*(class as *const Class) }
}

#[cfg(target_os = "macos")]
fn appkit_string(value: id) -> Option<String> {
    use std::ffi::CStr;

    if value == nil {
        return None;
    }
    let bytes = unsafe { NSString::UTF8String(value) };
    (!bytes.is_null()).then(|| unsafe { CStr::from_ptr(bytes).to_string_lossy().into_owned() })
}

#[cfg(target_os = "macos")]
fn service_pasteboard_text(pasteboard: id) -> Option<String> {
    if pasteboard == nil {
        return None;
    }
    let types = unsafe {
        NSArray::arrayWithObjects(nil, &[NSPasteboardTypeString, NSStringPboardType])
    };
    let pasteboard_type: id = unsafe { msg_send![pasteboard, availableTypeFromArray: types] };
    if pasteboard_type == nil {
        return None;
    }
    let string: id = unsafe { msg_send![pasteboard, stringForType: pasteboard_type] };
    if let Some(text) = appkit_string(string) {
        return Some(text);
    }
    let data: id = unsafe { msg_send![pasteboard, dataForType: pasteboard_type] };
    if data == nil {
        return None;
    }
    let bytes: *const u8 = unsafe { msg_send![data, bytes] };
    let length: usize = unsafe { msg_send![data, length] };
    if bytes.is_null() {
        return None;
    }
    String::from_utf8(unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec()).ok()
}

#[cfg(target_os = "macos")]
extern "C" fn receive_native_service(
    _this: &Object,
    _cmd: Sel,
    pasteboard: id,
    user_data: id,
    _error: *mut Object,
) {
    let Some(name) = appkit_string(user_data) else {
        return;
    };
    let argument = service_pasteboard_text(pasteboard).unwrap_or_default();
    let sender = NATIVE_SERVICE_EVENT_SENDER
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    if let Some(sender) = sender {
        let _ = sender.send(GuiEvent::NativeService { name, argument });
    }
}

#[cfg(target_os = "macos")]
fn register_native_service_provider(
    event_tx: std::sync::mpsc::Sender<GuiEvent>,
) -> Result<(), String> {
    *NATIVE_SERVICE_EVENT_SENDER
        .lock()
        .map_err(|_| "Unable to access macOS Services state".to_owned())? = Some(event_tx);
    let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    if app == nil {
        return Err("NSApplication is unavailable".to_owned());
    }
    let provider = {
        let mut provider = NATIVE_SERVICE_PROVIDER
            .lock()
            .map_err(|_| "Unable to access macOS Services provider".to_owned())?;
        if *provider == 0 {
            let class = native_service_provider_class();
            let allocated: id = unsafe { msg_send![class, alloc] };
            let instance: id = unsafe { msg_send![allocated, init] };
            if instance == nil {
                return Err("Unable to create the macOS Services provider".to_owned());
            }
            *provider = instance as usize;
        }
        *provider as id
    };
    unsafe {
        let _: () = msg_send![app, setServicesProvider: provider];
    }
    Ok(())
}

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
        .filter(|frame| frame.has_size_parameter)
        .map(|frame| {
            size(
                px(frame.width.max(10) as f32 * 8.0),
                px(frame.height.max(3) as f32 * 19.0),
            )
        })
        .unwrap_or(size(px(960.0), px(640.0)));
    let initial_focus_on_map = initial_frame_size
        .map(|frame| !frame.no_focus_on_map && !frame.no_accept_focus)
        .unwrap_or(true);
    let initial_no_accept_focus = initial_frame_size.is_some_and(|frame| frame.no_accept_focus);
    let initial_skip_taskbar = initial_frame_size.is_some_and(|frame| frame.skip_taskbar);
    let initial_sticky = initial_frame_size.is_some_and(|frame| frame.sticky);
    let initial_shaded = initial_frame_size.is_some_and(|frame| frame.shaded);
    let initial_min_size = initial_frame_size
        .and_then(|frame| {
            frame_minimum_size(
                frame.min_width,
                frame.min_height,
                8.0,
                19.0,
                frame.internal_border_width as f32 * 2.0,
            )
        });

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
        #[cfg(target_os = "macos")]
        if let Err(error) = register_native_service_provider(event_tx.clone()) {
            eprintln!("remacs: macOS Services input unavailable: {error}");
        }
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
                focus: initial_focus_on_map,
                window_min_size: initial_min_size,
                ..Default::default()
            },
            |window, cx| {
                set_native_no_accept_focus(window, initial_no_accept_focus);
                update_native_skip_taskbar(window, initial_skip_taskbar);
                update_native_sticky(window, initial_sticky);
                update_native_shaded(window, initial_shaded);
                update_native_minimum_size(window, initial_min_size);
                if initial_focus_on_map {
                    cx.activate(false);
                }
                cx.new(|cx| {
                    build_managed_editor_view(
                        window,
                        cx,
                        event_tx.clone(),
                        None,
                        None,
                        initial_focus_on_map,
                        initial_no_accept_focus,
                    )
                })
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
            let mut frame_focus_on_map = HashMap::<usize, bool>::new();
            let mut frame_skip_taskbar = HashMap::<usize, bool>::new();
            let mut frame_sticky = HashMap::<usize, bool>::new();
            let mut frame_shaded = HashMap::<usize, bool>::new();
            let mut frame_minimum_cells =
                HashMap::<usize, (Option<usize>, Option<usize>, usize, usize)>::new();
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
                            let no_accept_focus = snapshot.no_accept_focus;
                            let focus_on_map = !snapshot.no_focus_on_map && !no_accept_focus;
                            frame_focus_on_map.insert(frame_id, focus_on_map);
                            let skip_taskbar = snapshot.skip_taskbar;
                            let skip_taskbar_changed =
                                frame_skip_taskbar.get(&frame_id) != Some(&skip_taskbar);
                            frame_skip_taskbar.insert(frame_id, skip_taskbar);
                            let sticky = snapshot.sticky;
                            let sticky_changed = frame_sticky.get(&frame_id) != Some(&sticky);
                            frame_sticky.insert(frame_id, sticky);
                            let shaded = snapshot.shaded;
                            let shaded_changed = frame_shaded.get(&frame_id) != Some(&shaded);
                            frame_shaded.insert(frame_id, shaded);
                            let minimum_cells = (
                                snapshot.min_width,
                                snapshot.min_height,
                                snapshot.grid.default_line_height,
                                internal_border_width,
                            );
                            let minimum_size_changed =
                                frame_minimum_cells.get(&frame_id) != Some(&minimum_cells);
                            frame_minimum_cells.insert(frame_id, minimum_cells);
                            let minimum_size = frame_minimum_size(
                                snapshot.min_width,
                                snapshot.min_height,
                                8.0,
                                snapshot.grid.default_line_height.max(1) as f32,
                                internal_border_width as f32 * 2.0,
                            );
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
                                    if skip_taskbar_changed {
                                        update_native_skip_taskbar(window, skip_taskbar);
                                    }
                                    if sticky_changed {
                                        update_native_sticky(window, sticky);
                                    }
                                    if shaded_changed {
                                        update_native_shaded(window, shaded);
                                    }
                                    if minimum_size_changed {
                                        update_native_minimum_size(window, minimum_size);
                                    }
                                    let modifier_mapping_changed =
                                        view.set_modifier_mapping(modifier_mapping);
                                    let internal_border_changed =
                                        view.set_internal_border_width(internal_border_width);
                                    let alpha_background_changed =
                                        view.set_alpha_background(alpha_background);
                                    if view.set_no_accept_focus(window, no_accept_focus) {
                                        set_native_no_accept_focus(window, no_accept_focus);
                                    }
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
                            let minimum_size = frame_minimum_size(
                                snapshot.min_width,
                                snapshot.min_height,
                                8.0,
                                snapshot.grid.default_line_height.max(1) as f32,
                                snapshot.internal_border_width as f32 * 2.0,
                            );
                            let handle = match cx.open_window(options, move |window, app| {
                                set_native_no_accept_focus(window, no_accept_focus);
                                update_native_skip_taskbar(window, skip_taskbar);
                                update_native_sticky(window, sticky);
                                update_native_shaded(window, shaded);
                                update_native_minimum_size(window, minimum_size);
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
                                        focus_on_map,
                                        no_accept_focus,
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
                                    update_native_skip_taskbar(window, false);
                                    update_native_minimum_size(window, None);
                                    window.remove_window();
                                });
                            }
                            window_modes.remove(&frame_id);
                            frame_behaviors.remove(&frame_id);
                            frame_focus_on_map.remove(&frame_id);
                            frame_skip_taskbar.remove(&frame_id);
                            frame_sticky.remove(&frame_id);
                            frame_shaded.remove(&frame_id);
                            frame_minimum_cells.remove(&frame_id);
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
                        let focus_on_map = frame_focus_on_map
                            .get(&frame_id)
                            .copied()
                            .unwrap_or(true);
                        if let Some(handle) = windows.get(&frame_id).copied() {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                if visible {
                                    show_frame_window(window, focus_on_map);
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
                    #[cfg(target_os = "macos")]
                    GuiRequest::FontSelection {
                        frame_id,
                        current_font,
                        reply,
                    } => {
                        let result = if let Some(handle) = windows.get(&frame_id).copied() {
                            handle
                                .update(cx, move |_view, window, _cx| {
                                    show_native_font_panel(window, &current_font)
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
                    GuiRequest::BeginFileDrag {
                        frame_id,
                        paths,
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
                                    window.begin_file_drag_session(
                                        paths,
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

fn frame_minimum_size(
    min_width: Option<usize>,
    min_height: Option<usize>,
    cell_width: f32,
    line_height: f32,
    border: f32,
) -> Option<Size<Pixels>> {
    if min_width.is_none() && min_height.is_none() {
        return None;
    }
    Some(size(
        min_width
            .map(|width| px(width as f32 * cell_width + border))
            .unwrap_or(px(0.0)),
        min_height
            .map(|height| px(height as f32 * line_height + border))
            .unwrap_or(px(0.0)),
    ))
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
    let window_min_size = frame_minimum_size(
        frame.min_width,
        frame.min_height,
        8.0,
        frame.grid.default_line_height.max(1) as f32,
        internal_border,
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
        window_min_size,
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

fn show_frame_window(window: &Window, focus_on_map: bool) {
    #[cfg(target_os = "macos")]
    if let Some(native_window) = appkit_native_window(window) {
        unsafe {
            let minimized: BOOL = msg_send![native_window, isMiniaturized];
            if minimized == YES {
                let _: () = msg_send![native_window, deminiaturize: nil];
            }
            if !focus_on_map {
                let _: () = msg_send![native_window, orderFront: nil];
                return;
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
unsafe extern "C" fn gpui_can_become_key_window(this: &Object, selector: Sel) -> BOOL {
    let associated_key = std::ptr::addr_of!(NO_ACCEPT_FOCUS_ASSOCIATED_KEY).cast();
    let no_accept_focus = unsafe {
        !objc_getAssociatedObject(this as *const Object as id, associated_key).is_null()
    };
    if no_accept_focus {
        return NO;
    }

    let class = unsafe { object_getClass(this as *const Object) } as usize;
    let original = GPUI_CAN_BECOME_KEY_ORIGINALS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .get(&class)
        .copied();
    if let Some(original) = original {
        let original: unsafe extern "C" fn(&Object, Sel) -> BOOL =
            unsafe { std::mem::transmute(original) };
        unsafe { original(this, selector) }
    } else {
        YES
    }
}

#[cfg(target_os = "macos")]
fn install_gpui_no_accept_focus_hooks() {
    // GPUI 0.2.2 overrides canBecomeKeyWindow to always return YES. Preserve
    // that implementation for ordinary windows, and gate only the GPUI
    // window instances carrying Remacs' no-accept-focus frame parameter.
    let originals = GPUI_CAN_BECOME_KEY_ORIGINALS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut originals = originals.lock().unwrap();
    for class_name in ["GPUIWindow", "GPUIPanel"] {
        let Some(class) = Class::get(class_name) else {
            continue;
        };
        let class = class as *const Class;
        let class_id = class as usize;
        if originals.contains_key(&class_id) {
            continue;
        }
        let method = unsafe { class_getInstanceMethod(class, sel!(canBecomeKeyWindow)) };
        if method.is_null() {
            continue;
        }
        let implementation: unsafe extern "C" fn(&Object, Sel) -> BOOL =
            gpui_can_become_key_window;
        let implementation: Imp = unsafe { std::mem::transmute(implementation) };
        let original = unsafe { method_setImplementation(method as *mut Method, implementation) };
        originals.insert(class_id, original as usize);
    }
}

#[cfg(target_os = "macos")]
fn set_native_no_accept_focus(window: &Window, no_accept_focus: bool) {
    let Some(native_window) = appkit_native_window(window) else {
        return;
    };
    install_gpui_no_accept_focus_hooks();
    let associated_key = std::ptr::addr_of!(NO_ACCEPT_FOCUS_ASSOCIATED_KEY).cast();
    let marker = if no_accept_focus {
        class!(NSObject) as *const Class as *mut Object
    } else {
        nil
    };
    unsafe {
        objc_setAssociatedObject(native_window, associated_key, marker, 0);
    }
}

#[cfg(not(target_os = "macos"))]
fn set_native_no_accept_focus(_window: &Window, _no_accept_focus: bool) {}

fn update_native_skip_taskbar(window: &Window, skip_taskbar: bool) {
    if let Err(error) = set_native_skip_taskbar(window, skip_taskbar) {
        eprintln!("remacs: failed to set skip-taskbar: {error}");
    }
}

fn update_native_sticky(window: &Window, sticky: bool) {
    if let Err(error) = set_native_sticky(window, sticky) {
        eprintln!("remacs: failed to set sticky: {error}");
    }
}

fn update_native_shaded(window: &Window, shaded: bool) {
    if let Err(error) = set_native_shaded(window, shaded) {
        eprintln!("remacs: failed to set shaded: {error}");
    }
}

fn update_native_minimum_size(window: &Window, minimum_size: Option<Size<Pixels>>) {
    if let Err(error) = set_native_minimum_size(window, minimum_size) {
        eprintln!("remacs: failed to set frame minimum size: {error}");
    }
}

#[cfg(target_os = "macos")]
fn set_native_minimum_size(
    window: &Window,
    minimum_size: Option<Size<Pixels>>,
) -> Result<(), String> {
    let Some(native_window) = appkit_native_window(window) else {
        return Err("native frame window is unavailable".to_owned());
    };
    let minimum_size = minimum_size.unwrap_or(size(px(0.0), px(0.0)));
    unsafe {
        let _: () = msg_send![native_window, setContentMinSize: NSSize {
            width: minimum_size.width.to_f64(),
            height: minimum_size.height.to_f64(),
        }];
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_native_sticky(window: &Window, sticky: bool) -> Result<(), String> {
    let Some(native_window) = appkit_native_window(window) else {
        return Err("native frame window is unavailable".to_owned());
    };
    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    let behavior: usize = unsafe { msg_send![native_window, collectionBehavior] };
    let behavior = if sticky {
        behavior | CAN_JOIN_ALL_SPACES
    } else {
        behavior & !CAN_JOIN_ALL_SPACES
    };
    unsafe {
        let _: () = msg_send![native_window, setCollectionBehavior: behavior];
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_native_skip_taskbar(window: &Window, skip_taskbar: bool) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;

    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: i32 = 0x0000_0080;
    const WS_EX_APPWINDOW: i32 = 0x0004_0000;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    let handle = raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Ok(());
    };
    let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
    let hwnd_id = handle.hwnd.get();
    let current = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
    let target = {
        let mut styles = ORIGINAL_WINDOW_EX_STYLES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if skip_taskbar {
            let original = *styles.entry(hwnd_id).or_insert(current);
            (original | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW
        } else {
            styles.remove(&hwnd_id).unwrap_or(current)
        }
    };
    if target != current {
        unsafe {
            SetWindowLongW(hwnd, GWL_EXSTYLE, target);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
struct XcbWindowContext {
    connection: xcb::Connection,
    root: xcb::x::Window,
    window: xcb::x::Window,
}

#[cfg(target_os = "linux")]
fn xcb_window_context(window: &Window) -> Result<Option<XcbWindowContext>, String> {
    use raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    let display = window
        .display_handle()
        .map_err(|error| error.to_string())?;
    let RawDisplayHandle::Xcb(display) = display.as_raw() else {
        // XCB requests are only available for the X11 display backend.
        return Ok(None);
    };
    let Some(raw_connection) = display.connection else {
        return Ok(None);
    };
    let handle = window
        .window_handle()
        .map_err(|error| error.to_string())?;
    let RawWindowHandle::Xcb(handle) = handle.as_raw() else {
        return Ok(None);
    };
    let connection = unsafe {
        xcb::Connection::from_raw_conn_and_extensions_no_drop(
            raw_connection.as_ptr().cast(),
            &[],
            &[],
        )
    };
    let screen_index = usize::try_from(display.screen)
        .map_err(|_| "invalid X11 screen number".to_owned())?;
    let root = connection
        .get_setup()
        .roots()
        .nth(screen_index)
        .ok_or_else(|| "X11 screen is unavailable".to_owned())?
        .root();
    Ok(Some(XcbWindowContext {
        connection,
        root,
        window: xcb::x::Window::new(handle.window.get()),
    }))
}

#[cfg(target_os = "linux")]
fn xcb_intern_atom(
    connection: &xcb::Connection,
    name: &'static [u8],
) -> Result<xcb::x::Atom, String> {
    let cookie = connection.send_request(&xcb::x::InternAtom {
        only_if_exists: false,
        name,
    });
    connection
        .wait_for_reply(cookie)
        .map(|reply| reply.atom())
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn xcb_send_client_message(
    context: &XcbWindowContext,
    message_type: xcb::x::Atom,
    data: [u32; 5],
) -> Result<(), String> {
    let event = xcb::x::ClientMessageEvent::new(
        context.window,
        message_type,
        xcb::x::ClientMessageData::Data32(data),
    );
    let cookie = context.connection.send_request_checked(&xcb::x::SendEvent {
        propagate: false,
        destination: xcb::x::SendEventDest::Window(context.root),
        event_mask: xcb::x::EventMask::SUBSTRUCTURE_REDIRECT
            | xcb::x::EventMask::SUBSTRUCTURE_NOTIFY,
        event: &event,
    });
    context
        .connection
        .check_request(cookie)
        .map_err(|error| error.to_string())?;
    context
        .connection
        .flush()
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn set_native_skip_taskbar(window: &Window, skip_taskbar: bool) -> Result<(), String> {
    use xcb::Xid;

    let Some(context) = xcb_window_context(window)? else {
        return Ok(());
    };
    let net_wm_state = xcb_intern_atom(&context.connection, b"_NET_WM_STATE")?;
    let skip_taskbar_atom = xcb_intern_atom(&context.connection, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager_atom = xcb_intern_atom(&context.connection, b"_NET_WM_STATE_SKIP_PAGER")?;
    let data = [
        if skip_taskbar { 1 } else { 0 },
        skip_taskbar_atom.resource_id(),
        skip_pager_atom.resource_id(),
        1,
        0,
    ];
    xcb_send_client_message(&context, net_wm_state, data)
}

#[cfg(target_os = "linux")]
fn set_native_sticky(window: &Window, sticky: bool) -> Result<(), String> {
    use xcb::Xid;

    let Some(context) = xcb_window_context(window)? else {
        return Ok(());
    };
    // EWMH sticky state pins a window to the viewport, while the desktop
    // property places it on every virtual desktop.
    let net_wm_state = xcb_intern_atom(&context.connection, b"_NET_WM_STATE")?;
    let sticky_atom = xcb_intern_atom(&context.connection, b"_NET_WM_STATE_STICKY")?;
    xcb_send_client_message(
        &context,
        net_wm_state,
        [
            if sticky { 1 } else { 0 },
            sticky_atom.resource_id(),
            0,
            1,
            0,
        ],
    )?;

    let net_wm_desktop = xcb_intern_atom(&context.connection, b"_NET_WM_DESKTOP")?;
    let desktop = if sticky {
        u32::MAX
    } else {
        let current_desktop = xcb_intern_atom(&context.connection, b"_NET_CURRENT_DESKTOP")?;
        let cookie = context.connection.send_request(&xcb::x::GetProperty {
            delete: false,
            window: context.root,
            property: current_desktop,
            r#type: xcb::x::ATOM_CARDINAL,
            long_offset: 0,
            long_length: 1,
        });
        context
            .connection
            .wait_for_reply(cookie)
            .map_err(|error| error.to_string())?
            .value::<u32>()
            .next()
            .unwrap_or(0)
    };
    xcb_send_client_message(
        &context,
        net_wm_desktop,
        [desktop, 1, 0, 0, 0],
    )
}

#[cfg(target_os = "linux")]
fn set_native_shaded(window: &Window, shaded: bool) -> Result<(), String> {
    use xcb::Xid;

    let Some(context) = xcb_window_context(window)? else {
        return Ok(());
    };
    let net_wm_state = xcb_intern_atom(&context.connection, b"_NET_WM_STATE")?;
    let shaded_atom = xcb_intern_atom(&context.connection, b"_NET_WM_STATE_SHADED")?;
    xcb_send_client_message(
        &context,
        net_wm_state,
        [
            if shaded { 1 } else { 0 },
            shaded_atom.resource_id(),
            0,
            1,
            0,
        ],
    )
}

#[cfg(target_os = "linux")]
fn set_native_minimum_size(
    window: &Window,
    minimum_size: Option<Size<Pixels>>,
) -> Result<(), String> {
    let Some(context) = xcb_window_context(window)? else {
        return Ok(());
    };
    const P_MIN_SIZE: u32 = 1 << 4;
    let wm_normal_hints = xcb_intern_atom(&context.connection, b"WM_NORMAL_HINTS")?;
    let wm_size_hints = xcb_intern_atom(&context.connection, b"WM_SIZE_HINTS")?;
    let cookie = context.connection.send_request(&xcb::x::GetProperty {
        delete: false,
        window: context.window,
        property: wm_normal_hints,
        r#type: wm_size_hints,
        long_offset: 0,
        long_length: 18,
    });
    let reply = context
        .connection
        .wait_for_reply(cookie)
        .map_err(|error| error.to_string())?;
    let mut hints: Vec<u32> = reply.value::<u32>().take(18).collect();
    hints.resize(18, 0);
    if let Some(minimum_size) = minimum_size {
        hints[0] |= P_MIN_SIZE;
        hints[5] = minimum_size.width.0.max(0.0).ceil() as u32;
        hints[6] = minimum_size.height.0.max(0.0).ceil() as u32;
    } else {
        hints[0] &= !P_MIN_SIZE;
        hints[5] = 0;
        hints[6] = 0;
    }
    let cookie = context.connection.send_request_checked(&xcb::x::ChangeProperty {
        mode: xcb::x::PropMode::Replace,
        window: context.window,
        property: wm_normal_hints,
        r#type: wm_size_hints,
        data: &hints,
    });
    context
        .connection
        .check_request(cookie)
        .map_err(|error| error.to_string())?;
    context
        .connection
        .flush()
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn remacs_minimum_size_window_proc(
    hwnd: *mut std::ffi::c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    const WM_GETMINMAXINFO: u32 = 0x0024;
    let state = WINDOWS_MINIMUM_SIZES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .get(&(hwnd as isize))
        .copied();
    let Some(state) = state else {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };
    let result = unsafe {
        CallWindowProcW(state.original_window_proc, hwnd, message, wparam, lparam)
    };
    if message == WM_GETMINMAXINFO && lparam != 0 {
        let info = unsafe { &mut *(lparam as *mut Win32MinMaxInfo) };
        if let Some(min_width) = state.min_width {
            info.min_track_size.x = info
                .min_track_size
                .x
                .max(min_width.saturating_add(state.nonclient_width));
        }
        if let Some(min_height) = state.min_height {
            info.min_track_size.y = info
                .min_track_size
                .y
                .max(min_height.saturating_add(state.nonclient_height));
        }
    }
    result
}

#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
unsafe fn windows_get_long_ptr(hwnd: *mut std::ffi::c_void, index: i32) -> isize {
    unsafe { GetWindowLongPtrW(hwnd, index) }
}

#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
unsafe fn windows_set_long_ptr(
    hwnd: *mut std::ffi::c_void,
    index: i32,
    value: isize,
) -> isize {
    unsafe { SetWindowLongPtrW(hwnd, index, value) }
}

#[cfg(all(target_os = "windows", target_pointer_width = "32"))]
unsafe fn windows_get_long_ptr(hwnd: *mut std::ffi::c_void, index: i32) -> isize {
    unsafe { GetWindowLongW(hwnd, index) as isize }
}

#[cfg(all(target_os = "windows", target_pointer_width = "32"))]
unsafe fn windows_set_long_ptr(
    hwnd: *mut std::ffi::c_void,
    index: i32,
    value: isize,
) -> isize {
    unsafe { SetWindowLongW(hwnd, index, value as i32) as isize }
}

#[cfg(target_os = "windows")]
fn set_native_minimum_size(
    window: &Window,
    minimum_size: Option<Size<Pixels>>,
) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;

    const GWLP_WNDPROC: i32 = -4;
    let handle = raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Ok(());
    };
    let hwnd_id = handle.hwnd.get();
    let hwnd = hwnd_id as *mut std::ffi::c_void;
    let (min_width, min_height) = match minimum_size {
        Some(size) => {
            let scale = window.scale_factor();
            (
                Some((size.width.0 * scale).ceil() as i32).filter(|value| *value > 0),
                Some((size.height.0 * scale).ceil() as i32).filter(|value| *value > 0),
            )
        }
        None => (None, None),
    };
    let states = WINDOWS_MINIMUM_SIZES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = states.lock().unwrap();
    if min_width.is_none() && min_height.is_none() {
        if let Some(state) = states.remove(&hwnd_id) {
            drop(states);
            unsafe { windows_set_long_ptr(hwnd, GWLP_WNDPROC, state.original_window_proc) };
        }
        return Ok(());
    }
    if let Some(state) = states.get_mut(&hwnd_id) {
        state.min_width = min_width;
        state.min_height = min_height;
        return Ok(());
    }
    drop(states);

    let original_window_proc = unsafe { windows_get_long_ptr(hwnd, GWLP_WNDPROC) };
    if original_window_proc == 0 {
        return Err("native window procedure is unavailable".to_owned());
    }
    let mut outer = Win32Rect::default();
    let mut client = Win32Rect::default();
    unsafe {
        GetWindowRect(hwnd, &mut outer);
        GetClientRect(hwnd, &mut client);
    }
    let nonclient_width = (outer.right - outer.left - (client.right - client.left)).max(0);
    let nonclient_height = (outer.bottom - outer.top - (client.bottom - client.top)).max(0);
    let replacement = remacs_minimum_size_window_proc as *const () as isize;
    unsafe { windows_set_long_ptr(hwnd, GWLP_WNDPROC, replacement) };
    WINDOWS_MINIMUM_SIZES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(
            hwnd_id,
            WindowsMinimumSizeState {
                original_window_proc,
                min_width,
                min_height,
                nonclient_width,
                nonclient_height,
            },
        );
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn set_native_minimum_size(
    _window: &Window,
    _minimum_size: Option<Size<Pixels>>,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn set_native_sticky(_window: &Window, _sticky: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn set_native_shaded(_window: &Window, _shaded: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn set_native_skip_taskbar(_window: &Window, _skip_taskbar: bool) -> Result<(), String> {
    // macOS has a per-application Dock item rather than a per-window taskbar item.
    Ok(())
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

#[cfg(target_os = "macos")]
static FONT_PANEL_RESULT: Mutex<Option<Option<String>>> = Mutex::new(None);
#[cfg(target_os = "macos")]
static FONT_PANEL_RESPONDER_CLASS: OnceLock<usize> = OnceLock::new();

#[cfg(target_os = "macos")]
fn font_panel_responder_class() -> &'static Class {
    // NSFontManager sends changeFont: through the first-responder chain.
    let class = *FONT_PANEL_RESPONDER_CLASS.get_or_init(|| {
        let mut declaration = ClassDecl::new("RemacsFontPanelResponder", class!(NSView))
            .expect("RemacsFontPanelResponder must be registered once");
        unsafe {
            declaration.add_method(
                sel!(acceptsFirstResponder),
                font_panel_accepts_first_responder as extern "C" fn(&Object, Sel) -> BOOL,
            );
            declaration.add_method(
                sel!(changeFont:),
                font_panel_change_font as extern "C" fn(&Object, Sel, id),
            );
            declaration.add_method(
                sel!(windowWillClose:),
                font_panel_window_will_close as extern "C" fn(&Object, Sel, id),
            );
        }
        declaration.register() as *const Class as usize
    });
    unsafe { &*(class as *const Class) }
}

#[cfg(target_os = "macos")]
extern "C" fn font_panel_accepts_first_responder(_this: &Object, _cmd: Sel) -> BOOL {
    YES
}

#[cfg(target_os = "macos")]
extern "C" fn font_panel_change_font(_this: &Object, _cmd: Sel, sender: id) {
    use std::ffi::CStr;

    let result = if sender == nil {
        None
    } else {
        unsafe {
            let selected: id = msg_send![sender, selectedFont];
            let converted: id = msg_send![sender, convertFont: selected];
            let family: id = msg_send![converted, familyName];
            if family == nil {
                None
            } else {
                let bytes = NSString::UTF8String(family);
                (!bytes.is_null()).then(|| {
                    CStr::from_ptr(bytes).to_string_lossy().into_owned()
                })
            }
        }
    };
    let mut selection = FONT_PANEL_RESULT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if selection.is_none() {
        *selection = Some(result);
    }
    drop(selection);
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, stopModalWithCode: 1_isize];
    }
}

#[cfg(target_os = "macos")]
extern "C" fn font_panel_window_will_close(_this: &Object, _cmd: Sel, _notification: id) {
    let mut selection = FONT_PANEL_RESULT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if selection.is_none() {
        *selection = Some(None);
    }
    drop(selection);
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, stopModalWithCode: 0_isize];
    }
}

#[cfg(target_os = "macos")]
fn show_native_font_panel(window: &Window, current_font: &str) -> Result<Option<String>, String> {
    // The modal AppKit loop keeps the native Font panel responsive while the
    // logic thread waits for x-select-font to return.
    let native_window =
        appkit_native_window(window).ok_or_else(|| "native frame window is unavailable".to_owned())?;
    window.activate_window();
    let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    let manager: id = unsafe { msg_send![class!(NSFontManager), sharedFontManager] };
    let panel: id = unsafe { msg_send![class!(NSFontPanel), sharedFontPanel] };
    if manager == nil || panel == nil {
        return Err("NSFontPanel could not be created".to_owned());
    }

    let font_name: id = unsafe {
        msg_send![NSString::alloc(nil).init_str(current_font), autorelease]
    };
    let initial_font: id = unsafe {
        msg_send![class!(NSFont), fontWithName: font_name size: 14.0_f64]
    };
    if initial_font != nil {
        unsafe {
            let _: () = msg_send![manager, setSelectedFont: initial_font isMultiple: NO];
        }
    }
    unsafe {
        let _: () = msg_send![manager, setAction: sel!(changeFont:)];
        let _: () = msg_send![panel, setWorksWhenModal: YES];
    }

    let responder_class = font_panel_responder_class();
    let responder: id = unsafe { msg_send![responder_class, alloc] };
    let responder: id = unsafe {
        msg_send![responder, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(1.0, 1.0),
        )]
    };
    let content: id = unsafe { msg_send![panel, contentView] };
    let previous_first_responder: id = unsafe { msg_send![panel, firstResponder] };
    unsafe {
        let _: () = msg_send![responder, setAlphaValue: 0.0_f64];
        let _: () = msg_send![content, addSubview: responder];
        let _: () = msg_send![panel, setDelegate: responder];
        let _: BOOL = msg_send![panel, makeFirstResponder: responder];
    }
    *FONT_PANEL_RESULT
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = None;
    unsafe {
        let _: () = msg_send![panel, makeKeyAndOrderFront: native_window];
        let _: NSInteger = msg_send![app, runModalForWindow: panel];
        let _: () = msg_send![panel, orderOut: nil];
        let _: () = msg_send![panel, setDelegate: nil];
        let _: BOOL = msg_send![panel, makeFirstResponder: previous_first_responder];
        let _: () = msg_send![responder, removeFromSuperview];
        let _: id = msg_send![responder, autorelease];
    }
    Ok(FONT_PANEL_RESULT
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
        .flatten())
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
