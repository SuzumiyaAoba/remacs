//! Filesystem callbacks may change watches while native events are pending.
#![cfg(target_os = "macos")]
mod common;
#[path = "support/probe.rs"]
mod probe;

fn run_case(name: &'static str, file: &'static str) {
    probe::run(name, file, move || {
        let (mut interp, output) = common::batch_interp();
        let source = format!(
            "(progn (load {:?}) (load {:?}))",
            format!("{}/tests/qa_framework.el", env!("CARGO_MANIFEST_DIR")),
            format!("{}/tests/{file}", env!("CARGO_MANIFEST_DIR")),
        );
        let result = interp.eval_str(&source);
        assert!(
            result.is_ok(),
            "{file}: {:?}\n{}",
            result.err(),
            output.borrow()
        );
        assert!(
            output.borrow().contains("1 checks passed"),
            "{}",
            output.borrow()
        );
    });
}

#[test]
fn directory_callback_removes_own_watch() {
    run_case(
        "directory_callback_removes_own_watch",
        "notify_directory_self.el",
    );
}

#[test]
fn directory_callback_keeps_watch() {
    run_case("directory_callback_keeps_watch", "notify_directory_keep.el");
}

#[test]
fn directory_callback_removes_earlier_watch() {
    run_case(
        "directory_callback_removes_earlier_watch",
        "notify_directory_shift.el",
    );
}

#[test]
fn directory_callback_replaces_own_watch() {
    run_case(
        "directory_callback_replaces_own_watch",
        "notify_directory_replace.el",
    );
}

#[test]
fn directory_callback_creates_another_child() {
    run_case(
        "directory_callback_creates_another_child",
        "notify_directory_create.el",
    );
}

#[test]
fn last_watch_removal_can_be_rearmed() {
    run_case("last_watch_removal_can_be_rearmed", "notify_watch_rearm.el");
}

#[test]
fn rearm_works_with_another_watch_alive() {
    run_case(
        "rearm_works_with_another_watch_alive",
        "notify_watch_guardian.el",
    );
}

#[test]
fn last_deleted_watch_can_be_rearmed() {
    run_case(
        "last_deleted_watch_can_be_rearmed",
        "notify_watch_delete.el",
    );
}
