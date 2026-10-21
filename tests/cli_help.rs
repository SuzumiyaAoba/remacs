//! Help is an initial fast path, with the same argument boundaries as actions.
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "remacs-cli-help-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("cache")).unwrap();
        std::fs::create_dir(root.join("bin")).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_remacs"), root.join("bin/remacs")).unwrap();
        for file in ["--help", "-help"] {
            std::fs::write(root.join(file), "(princ 9)\n").unwrap();
        }
        Self(root)
    }

    fn run(&self, args: &[&str], source: bool) -> Output {
        let mut command = Command::new(self.0.join("bin/remacs"));
        command
            .current_dir(&self.0)
            .env("TMPDIR", self.0.join("cache"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if source {
            command.env("REMACS_NO_ELC", "1");
        } else {
            command.env_remove("REMACS_NO_ELC");
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "CLI timed out: {args:?}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    }

    fn help(&self, args: &[&str]) {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("Usage:"));
        }
        for source in [false, true] {
            let result = self.run(args, source);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let text = String::from_utf8_lossy(&result.stdout);
            assert!(text.starts_with("Usage: remacs "), "{text}");
            for option in [
                "--help",
                "--version",
                "--batch",
                "--eval",
                "--execute",
                "--funcall",
                "--load",
                "--directory",
                "--script",
                "--dump-file",
                "--no-window-system",
            ] {
                assert!(text.contains(option), "missing {option}: {text}");
            }
            assert!(result.stderr.is_empty());
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_aliases_print_usage_without_bootstrapping() {
    let fixture = Fixture::new();
    for option in ["--help", "-help"] {
        fixture.help(&[option]);
        fixture.help(&["-Q", "--batch", option]);
    }
    assert!(std::fs::read_dir(fixture.0.join("cache"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn help_preempts_actions_without_bootstrapping() {
    let fixture = Fixture::new();
    fixture.help(&["-Q", "--batch", "--eval", "(kill-emacs 17)", "--help"]);
    assert!(std::fs::read_dir(fixture.0.join("cache"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn help_retains_selected_image_validation_and_restore() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("invalid.pdmp"), "not an image").unwrap();
    for path in ["missing.pdmp", "invalid.pdmp"] {
        for source in [false, true] {
            let result = fixture.run(&["--dump-file", path, "--help"], source);
            assert!(!result.status.success());
            assert!(!String::from_utf8_lossy(&result.stdout).contains("Usage:"));
            assert!(!result.stderr.is_empty());
        }
    }
    let image = fixture.0.join("valid.pdmp");
    let expression = format!("(dump-emacs-portable {:?})", image.to_string_lossy());
    let created = fixture.run(&["-Q", "--batch", "--eval", &expression], false);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    for source in [false, true] {
        let result = fixture.run(
            &[
                "--dump-file",
                image.to_str().unwrap(),
                "--help",
                "--eval",
                "(kill-emacs 17)",
            ],
            source,
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).starts_with("Usage: remacs "));
    }
}

#[test]
fn version_keeps_priority_over_help_in_either_order() {
    let fixture = Fixture::new();
    let version = fixture.run(&["--version"], false);
    assert!(version.status.success());
    for args in [
        ["--help", "--version"],
        ["--version", "--help"],
        ["-help", "-version"],
        ["-version", "-help"],
    ] {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(result.status.success());
            assert!(!String::from_utf8_lossy(&result.stdout).contains("Usage:"));
        }
        let result = fixture.run(&args, false);
        assert!(result.status.success());
        assert_eq!(result.stdout, version.stdout);
        assert!(result.stderr.is_empty());
    }
    assert!(std::fs::read_dir(fixture.0.join("cache"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn help_in_operands_or_after_separator_remains_literal() {
    let fixture = Fixture::new();
    for args in [
        vec!["-Q", "--batch", "--load", "--help"],
        vec!["-Q", "--batch", "-load", "-help"],
    ] {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&fixture.0)
                .args(&args)
                .output()
                .unwrap();
            assert!(result.status.success());
            assert_eq!(result.stdout, b"9");
        }
        for source in [false, true] {
            let result = fixture.run(&args, source);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"9");
        }
    }
    for source in [false, true] {
        let result = fixture.run(
            &[
                "-Q",
                "--batch",
                "--eval",
                "(add-hook 'kill-emacs-hook (lambda () (princ (buffer-string))))",
                "--",
                "--help",
                "-help",
            ],
            source,
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"(princ 9)\n");
    }
}

#[test]
fn help_injected_after_startup_retains_the_gnu_runtime_error() {
    let fixture = Fixture::new();
    let args = [
        "-Q",
        "--batch",
        "--eval",
        "(setq command-line-args-left '(\"--help\"))",
    ];
    if let Some(gnu) = std::env::var_os("GNU_EMACS") {
        let result = Command::new(gnu)
            .current_dir(&fixture.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(255));
        assert!(!String::from_utf8_lossy(&result.stdout).contains("Usage:"));
    }
    for source in [false, true] {
        let result = fixture.run(&args, source);
        assert_eq!(result.status.code(), Some(255));
        assert!(!String::from_utf8_lossy(&result.stdout).contains("Usage:"));
        assert!(String::from_utf8_lossy(&result.stderr).contains("Unknown option"));
    }
}
