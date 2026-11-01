//! Run with `cargo test -p remacs-gui --test native_frame_map_focus` under
//! the external watchdog. This target opens its own AppKit windows.

#[cfg(target_os = "macos")]
#[path = "fixtures/frame_map_focus.rs"]
mod fixture;

fn main() {
    #[cfg(target_os = "macos")]
    fixture::run();
}
