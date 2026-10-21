//! Alternate GNU spellings share parsing without rewriting argv or operands.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "remacs-cli-forms-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        for name in ["payload.el", "-eval", "--eval=(princ 8)", "--"] {
            std::fs::write(root.join(name), "(princ 7)\n").unwrap();
        }
        std::fs::write(
            root.join("lib/qa230-library.el"),
            "(defvar qa230-value 7)\n(provide 'qa230-library)\n",
        )
        .unwrap();
        Self(root)
    }

    fn check(&self, args: &[&str], expected: &str, error: Option<&str>) {
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let result = Command::new(gnu)
                .current_dir(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(
                result.status.code(),
                Some(if error.is_some() { 255 } else { 0 }),
                "{args:?}"
            );
            let stdout = String::from_utf8_lossy(&result.stdout);
            if let Some(condition) = error {
                assert_eq!(stdout.split("\nError:").next().unwrap(), expected);
                assert!(
                    format!("{stdout}{}", String::from_utf8_lossy(&result.stderr))
                        .contains(condition)
                );
            } else {
                assert_eq!(stdout, expected);
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
                expected,
                "source={source}: {stderr}"
            );
            if let Some(condition) = error {
                assert!(stderr.contains(condition), "{stderr}");
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn eval_and_execute_aliases_and_inline_values_read_one_object() {
    let fixture = Fixture::new();
    for flag in ["-eval", "-execute"] {
        fixture.check(&["-Q", "--batch", flag, "(princ 7) (princ 8)"], "7", None);
    }
    for flag in [
        "--eval=(princ 7) (princ 8)",
        "--execute=(princ 7) (princ 8)",
    ] {
        fixture.check(&["-Q", "--batch", flag], "7", None);
    }
    fixture.check(&["-Q", "--batch", "--eval=(princ \"a=b\")"], "a=b", None);
}

#[test]
fn load_and_funcall_aliases_do_not_consume_a_following_action() {
    let fixture = Fixture::new();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "-load",
            "payload.el",
            "--eval",
            "(princ 8)",
        ],
        "78",
        None,
    );
    fixture.check(
        &["-Q", "--batch", "--load=payload.el", "--eval", "(princ 8)"],
        "78",
        None,
    );
    for option in [vec!["-funcall", "qa230-f"], vec!["--funcall=qa230-f"]] {
        let mut args = vec!["-Q", "--batch", "--eval", "(defun qa230-f () (princ 7))"];
        args.extend(option);
        args.extend(["--eval", "(princ 8)"]);
        fixture.check(&args, "78", None);
    }
}

#[test]
fn directory_and_script_single_hyphen_aliases_work() {
    let fixture = Fixture::new();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "-directory",
            "lib",
            "--eval",
            "(progn (require 'qa230-library) (princ qa230-value))",
        ],
        "7",
        None,
    );
    fixture.check(&["-Q", "--batch", "-script", "payload.el"], "7", None);
}

#[test]
fn original_spellings_remain_in_argv_and_live_queue_forms_are_decoded() {
    let fixture = Fixture::new();
    fixture.check(&["-Q", "--batch", "-eval", "(prin1 (list (and (member \"-eval\" command-line-args) t) (not (member \"--eval\" command-line-args))))"], "(t t)", None);
    fixture.check(&["-Q", "--batch", "--eval=(let (seen) (dolist (arg command-line-args) (when (string-prefix-p \"--eval=\" arg) (setq seen t))) (prin1 (list seen (not (member \"--eval\" command-line-args)))))"], "(t t)", None);
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(setq command-line-args-left '(\"-execute\" \"(princ 7)\" \"--eval=(princ 8)\"))",
            "--eval",
            "(princ :stale)",
        ],
        "78",
        None,
    );
}

#[test]
fn flag_shaped_operands_and_files_after_separator_are_not_normalized() {
    let fixture = Fixture::new();
    fixture.check(
        &["-Q", "--batch", "--load", "-eval", "--eval", "(princ 8)"],
        "78",
        None,
    );
    fixture.check(&["-Q", "--batch", "-load", "--eval=(princ 8)"], "7", None);
    fixture.check(
        &["-Q", "--batch", "-load", "--", "-eval", "(princ 8)"],
        "78",
        None,
    );
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(add-hook 'kill-emacs-hook (lambda () (princ (buffer-string))))",
            "--",
            "-eval",
            "--eval=(princ 8)",
        ],
        "(princ 7)\n",
        None,
    );
}

#[test]
fn empty_inline_eval_values_are_not_missing_operands() {
    let fixture = Fixture::new();
    for flag in ["--eval=", "--execute="] {
        fixture.check(
            &["-Q", "--batch", flag, "--eval", "(kill-emacs 17)"],
            "",
            Some("end-of-file"),
        );
    }
}

#[test]
fn startup_flag_aliases_are_options_instead_of_visited_files() {
    let fixture = Fixture::new();
    for flag in ["-quick", "-no-init-file", "--no-init-file"] {
        let expression = format!("(prin1 (null (get-file-buffer (expand-file-name {flag:?}))))");
        fixture.check(&["--batch", flag, "--eval", &expression], "t", None);
    }
}

#[test]
fn version_alias_uses_the_existing_fast_path() {
    let canonical = Command::new(env!("CARGO_BIN_EXE_remacs"))
        .arg("--version")
        .output()
        .unwrap();
    let alias = Command::new(env!("CARGO_BIN_EXE_remacs"))
        .arg("-version")
        .output()
        .unwrap();
    assert!(canonical.status.success() && alias.status.success());
    assert_eq!(alias.stdout, canonical.stdout);
    assert!(String::from_utf8_lossy(&alias.stdout).starts_with("remacs "));
}
