//! GPUI frontend. The evaluator lives on a separate logic thread.
use gpui::*;
use std::io;
use std::sync::{Arc, Mutex};
mod keys;
mod logic;
mod view;
use logic::start_logic_with_dump;
use view::build_editor_view;

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
    let (event_tx, grid_rx) = start_logic_with_dump(
        files.to_vec(),
        dump_file.map(str::to_owned),
        arguments.to_vec(),
    )?;

    let open_error = Arc::new(Mutex::new(None));
    let app_open_error = Arc::clone(&open_error);
    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = cx.open_window(
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
                cx.new(|cx| build_editor_view(window, cx, event_tx, grid_rx))
            },
        ) {
            let message = format!("remacs: cannot open window: {error}");
            eprintln!("{message}");
            *app_open_error.lock().unwrap() = Some(message);
            cx.quit();
        }
    });
    match open_error.lock().unwrap().take() {
        Some(message) => Err(io::Error::new(io::ErrorKind::Other, message)),
        None => Ok(()),
    }
}

#[cfg(test)]
use keys::{command_keystroke, keystroke_to_code};
#[cfg(test)]
use logic::{ChanIo, GuiEvent, start_logic};
#[cfg(test)]
use view::EditorView;
#[cfg(test)]
mod tests;
