//! Monitor bounds for GUI frame/display reporting.

use super::logic::MonitorGeometry;
use gpui::{App, Window};

fn monitor_name(display_id: u32) -> String {
    format!("display-{display_id}")
}

#[cfg(target_os = "macos")]
pub(super) fn monitor_geometry(_window: &Window, _cx: &App) -> Vec<MonitorGeometry> {
    use cocoa::{
        appkit::NSScreen,
        base::{id, nil},
        foundation::{NSArray, NSDictionary, NSString},
    };
    use core_graphics::display::{CGDirectDisplayID, CGDisplayBounds, CGDisplayScreenSize};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let screens = NSScreen::screens(nil);
        let count = NSArray::count(screens) as usize;
        let mut monitors = Vec::with_capacity(count);
        for index in 0..count {
            let screen = NSArray::objectAtIndex(screens, index as u64);
            let frame = NSScreen::frame(screen);
            let visible = NSScreen::visibleFrame(screen);
            let description = NSScreen::deviceDescription(screen);
            let key = NSString::alloc(nil).init_str("NSScreenNumber");
            let key: id = msg_send![key, autorelease];
            let number = NSDictionary::objectForKey_(description, key);
            if number == nil {
                continue;
            }
            let display: CGDirectDisplayID = msg_send![number, unsignedIntValue];
            let display_id = display;
            let bounds = CGDisplayBounds(display);
            let physical_size = CGDisplayScreenSize(display);
            let work_top =
                frame.origin.y + frame.size.height - visible.origin.y - visible.size.height;
            monitors.push(MonitorGeometry {
                display_id,
                name: monitor_name(display_id),
                geometry: [
                    bounds.origin.x.round() as i128,
                    bounds.origin.y.round() as i128,
                    bounds.size.width.round() as i128,
                    bounds.size.height.round() as i128,
                ],
                workarea: [
                    (bounds.origin.x + visible.origin.x - frame.origin.x).round() as i128,
                    (bounds.origin.y + work_top).round() as i128,
                    visible.size.width.round() as i128,
                    visible.size.height.round() as i128,
                ],
                mm_size: [
                    Some(physical_size.width.round() as i128),
                    Some(physical_size.height.round() as i128),
                ],
                native_origin: [
                    frame.origin.x.round() as i128,
                    frame.origin.y.round() as i128,
                ],
            });
        }
        monitors
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn monitor_geometry(_window: &Window, cx: &App) -> Vec<MonitorGeometry> {
    let monitors: Vec<_> = cx
        .displays()
        .into_iter()
        .map(|display| {
            let bounds = display.bounds();
            let display_id = u32::from(display.id());
            let geometry = [
                f32::from(bounds.origin.x).round() as i128,
                f32::from(bounds.origin.y).round() as i128,
                f32::from(bounds.size.width).round() as i128,
                f32::from(bounds.size.height).round() as i128,
            ];
            MonitorGeometry {
                display_id,
                name: monitor_name(display_id),
                geometry,
                workarea: geometry,
                mm_size: [None, None],
                native_origin: [0, 0],
            }
        })
        .collect();
    monitors
}

pub(super) fn warp_mouse_pointer(
    window: &Window,
    cx: &App,
    x: i128,
    y: i128,
    absolute: bool,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::{display::CGDisplay, geometry::CGPoint};

        let display_id = window
            .display(cx)
            .map(|display| u32::from(display.id()))
            .ok_or_else(|| "GUI frame has no display".to_owned())?;
        let monitor = monitor_geometry(window, cx)
            .into_iter()
            .find(|monitor| monitor.display_id == display_id)
            .ok_or_else(|| "GUI frame display geometry is unavailable".to_owned())?;
        let (screen_x, screen_y) = if absolute {
            (x, y)
        } else {
            let bounds = window.bounds();
            let viewport = window.viewport_size();
            let frame_left = f32::from(bounds.origin.x).round() as i128 + monitor.native_origin[0];
            let frame_top =
                (f32::from(bounds.origin.y).round() as i128).saturating_add(monitor.geometry[1]);
            let content_left_offset = ((f32::from(bounds.size.width) - f32::from(viewport.width))
                / 2.0)
                .max(0.0)
                .round() as i128;
            let content_top_offset = (f32::from(bounds.size.height) - f32::from(viewport.height))
                .max(0.0)
                .round() as i128;
            (
                frame_left
                    .saturating_sub(monitor.geometry[0])
                    .saturating_add(content_left_offset)
                    .saturating_add(x),
                frame_top
                    .saturating_sub(monitor.geometry[1])
                    .saturating_add(content_top_offset)
                    .saturating_add(y),
            )
        };
        let global_x = monitor.geometry[0].saturating_add(screen_x);
        let global_y = monitor.geometry[1].saturating_add(screen_y);
        CGDisplay::warp_mouse_cursor_position(CGPoint::new(global_x as f64, global_y as f64))
            .map_err(|error| format!("Unable to move mouse pointer: {error:?}"))
    }
    #[cfg(target_os = "linux")]
    {
        warp_mouse_pointer_x11(window, cx, x, y, absolute)
    }
    #[cfg(target_os = "windows")]
    {
        warp_mouse_pointer_windows(window, cx, x, y, absolute)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (window, cx, x, y, absolute);
        Err("Mouse pointer warping is not supported by this GUI backend".to_owned())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn pointer_screen_coordinates(
    window: &Window,
    cx: &App,
    x: i128,
    y: i128,
    absolute: bool,
) -> Result<(i128, i128), String> {
    let (x, y) = if absolute {
        let display = window
            .display(cx)
            .ok_or_else(|| "GUI frame has no display".to_owned())?;
        let bounds = display.bounds();
        (
            (f32::from(bounds.origin.x).round() as i128).saturating_add(x),
            (f32::from(bounds.origin.y).round() as i128).saturating_add(y),
        )
    } else {
        let bounds = window.bounds();
        let viewport = window.viewport_size();
        let content_left_offset =
            ((f32::from(bounds.size.width) - f32::from(viewport.width)) / 2.0)
                .max(0.0)
                .round() as i128;
        let content_top_offset = (f32::from(bounds.size.height) - f32::from(viewport.height))
            .max(0.0)
            .round() as i128;
        (
            (f32::from(bounds.origin.x).round() as i128)
                .saturating_add(content_left_offset)
                .saturating_add(x),
            (f32::from(bounds.origin.y).round() as i128)
                .saturating_add(content_top_offset)
                .saturating_add(y),
        )
    };
    let scale_factor = f64::from(window.scale_factor());
    Ok((
        (x as f64 * scale_factor).round() as i128,
        (y as f64 * scale_factor).round() as i128,
    ))
}

#[cfg(target_os = "linux")]
fn warp_mouse_pointer_x11(
    window: &Window,
    cx: &App,
    x: i128,
    y: i128,
    absolute: bool,
) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window
        .window_handle()
        .map_err(|error| format!("Unable to get GUI window handle: {error:?}"))?;
    if !matches!(
        handle.as_raw(),
        RawWindowHandle::Xcb(_) | RawWindowHandle::Xlib(_)
    ) {
        return Err("Mouse pointer warping is unavailable on Wayland".to_owned());
    }

    let (connection, default_screen_num) = xcb::Connection::connect(None)
        .map_err(|error| format!("Unable to connect to the X server: {error}"))?;
    let screen_num = window
        .display(cx)
        .map(|display| u32::from(display.id()) as usize)
        .unwrap_or(default_screen_num as usize);
    let screen = connection
        .get_setup()
        .roots()
        .nth(screen_num)
        .ok_or_else(|| "X11 root window is unavailable".to_owned())?;
    let (x, y) = pointer_screen_coordinates(window, cx, x, y, absolute)?;
    let dst_x = i16::try_from(x).map_err(|_| "X11 mouse X coordinate is out of range")?;
    let dst_y = i16::try_from(y).map_err(|_| "X11 mouse Y coordinate is out of range")?;
    connection
        .send_and_check_request(&xcb::x::WarpPointer {
            src_window: xcb::x::WINDOW_NONE,
            dst_window: screen.root(),
            src_x: 0,
            src_y: 0,
            src_width: 0,
            src_height: 0,
            dst_x,
            dst_y,
        })
        .map_err(|error| format!("Unable to move mouse pointer: {error:?}"))
}

#[cfg(target_os = "windows")]
fn warp_mouse_pointer_windows(
    window: &Window,
    cx: &App,
    x: i128,
    y: i128,
    absolute: bool,
) -> Result<(), String> {
    let (x, y) = pointer_screen_coordinates(window, cx, x, y, absolute)?;
    let x = i32::try_from(x).map_err(|_| "Windows mouse X coordinate is out of range")?;
    let y = i32::try_from(y).map_err(|_| "Windows mouse Y coordinate is out of range")?;
    if unsafe { super::SetCursorPos(x, y) } == 0 {
        return Err(format!(
            "Unable to move mouse pointer: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
