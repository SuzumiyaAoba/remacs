fn main() {
    remacs_cli::application::run(Some(remacs_gui::run_gui_with_dump));
}
