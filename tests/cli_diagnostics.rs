//! Diagnostic evaluation preserves its exit status and reports byte positions.
use std::process::{Command, Output, Stdio};

fn evaluate(source: &str, source_boot: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
    command
        .args(["-Q", "--ieval", source, "--eval", "(princ :unexpected)"])
        .stdin(Stdio::null());
    if source_boot {
        command.env("REMACS_NO_ELC", "1");
    } else {
        command.env_remove("REMACS_NO_ELC");
    }
    command.output().unwrap()
}

#[test]
fn diagnostic_success_exits_before_later_actions() {
    for source_boot in [false, true] {
        let output = evaluate(
            "(setq cli-diagnostic 1) (setq cli-diagnostic 2)",
            source_boot,
        );
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn diagnostic_failure_reports_the_form_after_multibyte_text() {
    let first = "(setq cli-diagnostic \"あ\")";
    let failure = " (error \"位置\")";
    let source = format!("{first}{failure}");
    for source_boot in [false, true] {
        let output = evaluate(&source, source_boot);
        assert_eq!(output.status.code(), Some(255), "{:?}", output);
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("eval_str err: signal error"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains(&format!(
                "FAIL at byte {}: {failure} => signal error",
                first.len()
            )),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("panicked"), "{diagnostic}");
    }
}
