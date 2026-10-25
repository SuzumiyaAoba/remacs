fn main() {
    #[cfg(feature = "gui")]
    let gui = Some(remacs_gui::run_gui_with_dump as remacs_cli::application::GuiRunner);
    #[cfg(not(feature = "gui"))]
    let gui = None;
    remacs_cli::application::run(gui);
}
