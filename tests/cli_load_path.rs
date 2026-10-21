//! Command-line directory options are ordered actions after bootstrap.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const REQUIRE: &str = "(progn (require 'qa229-library) (prin1 (qa229-value)))";
const ORDER: &str = "(prin1 (mapcar (lambda (p) (cond ((equal p \"BASE\") 'BASE) ((equal p \"OVERRIDE\") 'OVERRIDE) (t (file-relative-name p command-line-default-directory)))) load-path))";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "remacs-cli-load-path-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for (directory, value) in [
            ("A", "A"),
            ("B", "B"),
            ("C", "C"),
            ("D", "D"),
            ("--eval", "FLAG"),
            ("sub/A", "SUB"),
        ] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
            std::fs::write(root.join(directory).join("qa229-library.el"), format!(";;; -*- lexical-binding: t; -*-\n(defun qa229-value () '{value})\n(provide 'qa229-library)\n")).unwrap();
        }
        // A CLI option must not change the libraries used by internal boot.
        std::fs::write(
            root.join("A/remacs-compat.el"),
            "(error \"CLI directory used during boot\")\n",
        )
        .unwrap();
        std::fs::write(root.join("sub/file.txt"), "fixture\n").unwrap();
        Self(root)
    }

    fn check(&self, args: &[&str], output: &str, error: Option<&str>) {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(
                result.status.code(),
                Some(if error.is_some() { 255 } else { 0 })
            );
            let stdout = String::from_utf8_lossy(&result.stdout);
            if let Some(condition) = error {
                assert_eq!(stdout.split("\nError:").next().unwrap(), output);
                assert!(
                    format!("{stdout}{}", String::from_utf8_lossy(&result.stderr))
                        .contains(condition)
                );
            } else {
                assert_eq!(stdout, output);
            }
        }
        for source in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
            command.current_dir(&self.0).args(args);
            if source {
                command.env("REMACS_NO_ELC", "1");
            } else {
                command.env_remove("REMACS_NO_ELC");
            }
            let result = command.output().unwrap();
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert_eq!(
                result.status.code(),
                Some(if error.is_some() { 255 } else { 0 }),
                "source={source}, {args:?}: {stderr}"
            );
            assert_eq!(
                String::from_utf8_lossy(&result.stdout),
                output,
                "source={source}: {stderr}"
            );
            if let Some(condition) = error {
                assert!(stderr.contains(condition), "{stderr}");
            }
        }
    }

    fn order(&self, options: &[&str], expected: &str) {
        let mut args = vec![
            "-Q",
            "--batch",
            "--eval",
            "(setq load-path (list \"BASE\"))",
        ];
        args.extend_from_slice(options);
        args.extend_from_slice(&["--eval", ORDER]);
        self.check(&args, expected, None);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn directory_aliases_and_inline_form_find_external_libraries_after_boot() {
    let fixture = Fixture::new();
    for (options, expected) in [
        (vec!["-L", "A"], "A"),
        (vec!["--directory", "B"], "B"),
        (vec!["--directory=C"], "C"),
        (vec!["-L", "--eval"], "FLAG"),
    ] {
        let mut args = vec!["-Q", "--batch"];
        args.extend(options);
        args.extend(["--eval", REQUIRE]);
        fixture.check(&args, expected, None);
    }
}

#[test]
fn directory_prefix_order_duplicates_and_trailing_slash_match_gnu() {
    let fixture = Fixture::new();
    fixture.order(&["-L", "A", "--directory", "B"], "(\"A\" \"B\" BASE)");
    fixture.order(&["-L", "A", "-L", "A"], "(\"A\" \"A\" BASE)");
    fixture.order(&["-L", "A/"], "(\"A/\" BASE)");
}

#[test]
fn directory_append_and_subsequent_splice_match_gnu() {
    let fixture = Fixture::new();
    fixture.order(
        &["-L", "A", "-L", ":B", "-L", "C"],
        "(\"A\" \"C\" BASE \"B\")",
    );
    fixture.order(
        &["-L", "A", "-L", "B", "-L", ":C", "-L", "D"],
        "(\"A\" \"D\" \"B\" BASE \"C\")",
    );
    fixture.order(&["-L", ":B", "-L", "A"], "(\"A\" BASE \"B\")");
}

#[test]
fn directory_options_preserve_action_order_and_current_buffer_base() {
    let fixture = Fixture::new();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(princ (condition-case nil (require 'qa229-library) (file-missing :missing)))",
            "-L",
            "A",
            "--eval",
            REQUIRE,
        ],
        ":missingA",
        None,
    );
    fixture.check(
        &[
            "-Q",
            "--batch",
            "sub/file.txt",
            "-L",
            "A",
            "--eval",
            REQUIRE,
        ],
        "SUB",
        None,
    );
    fixture.order(
        &[
            "-L",
            "A",
            "--eval",
            "(setq load-path (list \"OVERRIDE\"))",
            "-L",
            "B",
        ],
        "(OVERRIDE)",
    );
}

#[test]
fn empty_directory_options_report_the_normalizer_error() {
    let fixture = Fixture::new();
    for value in ["", ":"] {
        fixture.check(
            &["-Q", "--batch", "-L", value],
            "",
            Some("args-out-of-range"),
        );
    }
}

#[test]
fn directory_options_from_the_live_queue_are_applied() {
    let fixture = Fixture::new();
    fixture.check(&["-Q", "--batch", "--eval", "(setq command-line-args-left '(\"--directory\" \"A\" \"--eval\" \"(progn (require 'qa229-library) (prin1 (qa229-value)))\"))", "--eval", "(princ :stale)"], "A", None);
}

#[test]
fn separator_keeps_directory_options_literal() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("-L"), "short").unwrap();
    std::fs::write(fixture.0.join("--directory"), "literal").unwrap();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(add-hook 'kill-emacs-hook (lambda () (princ (buffer-string))))",
            "--",
            "-L",
            "--directory",
        ],
        "literal",
        None,
    );
}
