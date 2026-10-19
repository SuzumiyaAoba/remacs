//! End-of-options handling in the real binary, including startup scans.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "remacs-cli-separator-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(dir: &Path, args: &[&str]) -> (String, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_remacs"))
        .args(args)
        .current_dir(dir)
        // Startup assertions must observe the CLI, not the test runner.
        .env_remove("REMACS_NONINTERACTIVE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "CLI timed out: {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let output = child.wait_with_output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or(-1),
    )
}

fn succeeds(dir: &Path, args: &[&str], expected: &str) {
    let (out, err, code) = run(dir, args);
    assert_eq!(code, 0, "{args:?}: {err}");
    assert_eq!(out, expected, "{args:?}: {err}");
}

#[test]
fn separator_does_not_evaluate_following_arguments() {
    let fixture = Fixture::new();
    succeeds(
        &fixture.0,
        &["-Q", "--batch", "--", "--eval", "(princ 123)"],
        "",
    );
}

#[test]
fn separator_visits_all_flags_and_repeated_separator_as_files() {
    let fixture = Fixture::new();
    // A load/script handler must not execute this file after `--`.
    fixture.write("payload.el", "(princ \"UNEXPECTED\")");
    let names = [
        "--eval",
        "(princ 123)",
        "--execute",
        "--load",
        "-l",
        "--script",
        "payload.el",
        "-f",
        "--funcall",
        "--ieval",
        "--version",
        "--batch",
        "-batch",
        "-nw",
        "--no-window-system",
        "-Q",
        "--quick",
        "-q",
        "--no-init",
        "--no-x-resources",
        "-no-x-resources",
        "--",
    ];
    let quoted = names
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(" ");
    let hook = format!(
        "(add-hook 'kill-emacs-hook (lambda () \
         (dolist (name '({quoted})) \
         (princ (if (get-file-buffer (expand-file-name name)) \"Y\" \"N\")))))"
    );
    let mut args = vec!["-Q", "--batch", "--eval", hook.as_str(), "--"];
    args.extend(names);
    succeeds(&fixture.0, &args, &"Y".repeat(names.len()));
}

#[test]
fn separator_preserves_mixed_file_and_action_order() {
    let fixture = Fixture::new();
    fixture.write("one.txt", "one|");
    fixture.write("two.txt", "two|");
    fixture.write("load.el", "(princ \"loaded|\")");
    succeeds(
        &fixture.0,
        &[
            "-Q",
            "--batch",
            "one.txt",
            "--eval",
            "(princ (buffer-string))",
            "-l",
            "load.el",
            "two.txt",
            "--eval",
            "(princ (buffer-string))",
            "--eval",
            "(add-hook 'kill-emacs-hook (lambda () (princ (file-name-nondirectory (buffer-file-name)))))",
            "--",
            "--eval",
            "last.txt",
        ],
        "one|loaded|two|last.txt",
    );
}

#[test]
fn option_operands_can_be_separator_or_startup_flags() {
    let fixture = Fixture::new();
    fixture.write("--", "(princ \"separator|\")");
    fixture.write("--version", "(princ \"version|\")");
    fixture.write("--batch", "(princ \"batch|\")");
    fixture.write("--script", "(princ \"script|\")");
    succeeds(
        &fixture.0,
        &[
            "-Q",
            "--batch",
            "--load",
            "--",
            "-l",
            "--version",
            "--load",
            "--batch",
            "-l",
            "--script",
            "--eval",
            "(princ \"eval\")",
            "--",
        ],
        "separator|version|batch|script|eval",
    );
}

#[test]
fn separator_is_an_eval_operand_when_expected() {
    let fixture = Fixture::new();
    let (out, err, code) = run(
        &fixture.0,
        &["-Q", "--batch", "--eval", "--", "--eval", "(princ 123)"],
    );
    assert_eq!(code, 255, "{err}");
    assert_eq!(out, "");
    assert!(err.contains("void-variable"), "{err}");
}

#[test]
fn trailing_batch_and_script_filenames_do_not_select_batch_mode() {
    let fixture = Fixture::new();
    let (out, err, code) = run(
        &fixture.0,
        &["-Q", "-nw", "--", "--batch", "--script", "--version"],
    );
    assert_ne!(code, 0, "must reach the terminal without a tty");
    assert!(err.contains("terminal error"), "{err}");
    assert_eq!(out, "");
}

#[test]
fn trailing_init_flags_do_not_change_startup_settings() {
    let fixture = Fixture::new();
    // Seed a known value: the interpreter currently defaults this to t
    // even without -Q. Literal flag filenames must not change it.
    succeeds(
        &fixture.0,
        &[
            "--batch",
            "--eval",
            "(progn (setq inhibit-x-resources nil) (add-hook 'kill-emacs-hook (lambda () (prin1 inhibit-x-resources))))",
            "--",
            "-Q",
            "--quick",
            "--no-x-resources",
            "-no-x-resources",
        ],
        "nil",
    );
}

#[test]
fn trailing_batch_flags_do_not_change_initial_lisp_state() {
    let fixture = Fixture::new();
    // --eval itself sets noninteractive while running, but this mode's
    // initial value was chosen during Interp::new, before that action.
    succeeds(
        &fixture.0,
        &[
            "--eval",
            "(prin1 global-font-lock-mode)",
            "--",
            "--batch",
            "--script",
        ],
        "t",
    );
}

#[test]
fn funcall_consumed_files_and_separator_are_not_reprocessed() {
    let fixture = Fixture::new();
    fixture.write("input.txt", "input");
    succeeds(
        &fixture.0,
        &[
            "-Q",
            "--batch",
            "--eval",
            "(defun cli-consume () (setq command-line-args-left (cddr command-line-args-left)))",
            "-f",
            "cli-consume",
            "input.txt",
            "--",
            "--eval",
            r#"(princ (if (get-file-buffer (expand-file-name "input.txt")) "VISITED" "UNVISITED"))"#,
            "--eval",
            r#"(progn (defun cli-consume-all () (setq command-line-args-left nil))
                (add-hook 'kill-emacs-hook (lambda ()
                  (princ (if (get-file-buffer (expand-file-name "ignored.txt")) "|VISITED" "|UNVISITED")))))"#,
            "-f",
            "cli-consume-all",
            "ignored.txt",
        ],
        "UNVISITED|UNVISITED",
    );
}
