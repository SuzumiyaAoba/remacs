//! Real legacy terminal bytes must reach their Emacs bindings unchanged.
#![cfg(unix)]
#[allow(dead_code)]
#[path = "support/probe.rs"]
mod probe;
use std::process::Command;
use std::time::Duration;

fn terminal_case(case: &str, expected: &str) {
    let script = format!(
        "{}/tests/support/terminal_session.py",
        env!("CARGO_MANIFEST_DIR")
    );
    let report = probe::run_command(
        Command::new("python3").args([
            &script,
            "--binary",
            env!("CARGO_BIN_EXE_remacs"),
            "--case",
            case,
        ]),
        Duration::from_secs(150),
    )
    .expect("run owned PTY session");
    assert!(
        !report.timed_out && report.status.success(),
        "{}",
        report.tail
    );
    assert!(report.tail.contains("\"status\": 0"), "{}", report.tail);
    assert!(report.tail.contains("\"frame\": true"), "{}", report.tail);
    assert!(
        report.tail.contains(&format!("\"saved\": \"{expected}\"")),
        "{}",
        report.tail
    );
}

#[test]
fn legacy_control_bytes_reach_their_original_bindings() {
    terminal_case("controls", "3c32383e3c32393e3c33303e3c33313e");
}

#[test]
fn control_underscore_undoes_the_last_insertion() {
    terminal_case("undo", "616263");
}

#[test]
fn startup_eval_continues_into_the_editor() {
    terminal_case("startup-eval", "73746172747570");
}

#[test]
fn startup_load_continues_into_the_editor() {
    terminal_case("startup-load", "73746172747570");
}

#[test]
fn startup_funcall_continues_into_the_editor() {
    terminal_case("startup-funcall", "0a");
}
