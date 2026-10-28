//! GPUI frontend. The evaluator lives on a separate logic thread.
use gpui::*;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
mod accessibility;
mod keys;
mod logic;
mod monitor;
mod view;
use logic::{ClipboardRequest, FrameSnapshot, GuiRequest, start_logic_with_frames};
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
            let mut primary_window = Some(primary_window);
            while let Ok(request) = gui_requests.recv().await {
                match request {
                    GuiRequest::FrameSnapshots(snapshots) => {
                        for snapshot in snapshots {
                            let frame_id = snapshot.frame_id;
                            let grid = snapshot.grid.clone();
                            if let Some(handle) = windows.get(&frame_id).copied() {
                                let _ = handle.update(cx, move |view, _window, cx| {
                                    if view.grid != grid || grid.blink_cursor {
                                        view.grid = grid;
                                        cx.notify();
                                    }
                                });
                                continue;
                            }
                            if let Some(handle) = primary_window.take() {
                                let _ = handle.update(cx, move |view, _window, cx| {
                                    view.set_frame_id(frame_id);
                                    view.grid = grid;
                                    cx.notify();
                                });
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
                            windows.insert(frame_id, handle);
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
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
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

#[cfg(test)]
use keys::{command_keystroke, keystroke_to_code};
#[cfg(test)]
use logic::{ChanIo, GuiEvent, start_logic, start_logic_with_dump_for_test};
#[cfg(test)]
use view::EditorView;
#[cfg(test)]
mod tests;
