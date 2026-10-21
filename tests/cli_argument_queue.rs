//! GNU command-line-args-left is a live Lisp queue, not a suffix offset.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "remacs-cli-queue-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
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
                Some(if error.is_some() { 255 } else { 0 }),
                "{args:?}"
            );
            let stdout = String::from_utf8_lossy(&result.stdout);
            if let Some(data) = error {
                // The reference writes its debugger to stdout. Compare the
                // program output before that diagnostic separately.
                assert_eq!(stdout.split("\nError:").next().unwrap(), output);
                let diagnostic = format!("{stdout}{}", String::from_utf8_lossy(&result.stderr));
                assert!(
                    diagnostic.contains("wrong-type-argument") && diagnostic.contains(data),
                    "{diagnostic}"
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
            let errors = String::from_utf8_lossy(&result.stderr);
            assert_eq!(
                result.status.code(),
                Some(if error.is_some() { 255 } else { 0 }),
                "source={source}, {args:?}: {errors}"
            );
            assert_eq!(
                String::from_utf8_lossy(&result.stdout),
                output,
                "source={source}: {errors}"
            );
            if let Some(data) = error {
                assert!(
                    errors.contains("wrong-type-argument") && errors.contains(data),
                    "{errors}"
                );
            }
        }
    }

    fn consumer(&self, body: &str, tail: &[&str], output: &str, error: Option<&str>) {
        let definition = format!("(defun qa227-action () {body})");
        let mut args = vec!["-Q", "--batch", "--eval", &definition, "-f", "qa227-action"];
        args.extend_from_slice(tail);
        self.check(&args, output, error);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn funcall_can_consume_all_or_a_prefix_or_nothing() {
    let fixture = Fixture::new();
    fixture.consumer(
        "(setq command-line-args-left nil)",
        &["--eval", "(princ 1)"],
        "",
        None,
    );
    fixture.consumer(
        "(setq command-line-args-left (nthcdr 2 command-line-args-left))",
        &["--eval", "(princ 1)", "--eval", "(princ 2)"],
        "2",
        None,
    );
    fixture.consumer("nil", &["--eval", "(princ 1)"], "1", None);
    fixture.consumer(
        "(make-local-variable 'command-line-args-left)",
        &["--eval", "(princ 1)"],
        "1",
        None,
    );
}

#[test]
fn funcall_can_delete_middle_arguments_and_mutate_an_operand() {
    let fixture = Fixture::new();
    fixture.consumer(
        "(setcdr (cdr command-line-args-left) (nthcdr 4 command-line-args-left))",
        &[
            "--eval",
            "(princ 1)",
            "--eval",
            "(princ 2)",
            "--eval",
            "(princ 3)",
        ],
        "13",
        None,
    );
    fixture.consumer(
        "(setcar (cdr command-line-args-left) \"(princ 9)\")",
        &["--eval", "(princ 1)"],
        "9",
        None,
    );
}

#[test]
fn funcall_can_replace_or_prepend_remaining_arguments() {
    let fixture = Fixture::new();
    fixture.consumer(
        "(setq command-line-args-left (list \"--eval\" \"(princ 9)\"))",
        &["--eval", "(princ 1)"],
        "9",
        None,
    );
    fixture.consumer("(setq command-line-args-left (append (list \"--eval\" \"(princ 9)\") command-line-args-left))",
                     &["--eval", "(princ 1)"], "91", None);
}

#[test]
fn invalid_queue_elements_and_tails_fail_after_prior_actions() {
    let fixture = Fixture::new();
    for (body, output, error) in [
        ("(setq command-line-args-left '(17))", "", "stringp 17"),
        (
            "(setq command-line-args-left '(\"--eval\" . 17))",
            "",
            "listp 17",
        ),
        (
            "(setq command-line-args-left '(\"--eval\" \"(princ 1)\" 17))",
            "1",
            "stringp 17",
        ),
        (
            "(setq command-line-args-left '(\"--eval\" \"(princ 1)\" . 17))",
            "1",
            "listp 17",
        ),
        (
            "(setq command-line-args-left '(\"--eval\"))",
            "",
            "stringp nil",
        ),
    ] {
        fixture.consumer(body, &["--eval", "(princ 8)"], output, Some(error));
    }
}

#[test]
fn eval_and_load_can_also_consume_the_live_queue() {
    let fixture = Fixture::new();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--eval",
            "(setq command-line-args-left nil)",
            "--eval",
            "(princ 8)",
        ],
        "",
        None,
    );
    std::fs::write(
        fixture.0.join("consume.el"),
        "(setq command-line-args-left nil)\n",
    )
    .unwrap();
    fixture.check(
        &[
            "-Q",
            "--batch",
            "--load",
            "consume.el",
            "--eval",
            "(princ 8)",
        ],
        "",
        None,
    );
}

#[test]
fn replacement_can_chain_a_new_funcall_and_consume_its_own_operands() {
    let fixture = Fixture::new();
    fixture.check(&["-Q", "--batch", "--eval",
        "(progn (defun qa227-next () (setq command-line-args-left (nthcdr 2 command-line-args-left))) (defun qa227-action () (setq command-line-args-left '(\"--funcall\" \"qa227-next\" \"--eval\" \"(princ 1)\" \"--eval\" \"(princ 2)\"))))",
        "-f", "qa227-action", "--eval", "(princ 8)"], "2", None);
}

#[test]
fn replacement_separator_is_processed_once_and_keeps_following_flags_literal() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("--eval"), "first").unwrap();
    std::fs::write(fixture.0.join("(princ 123)"), "literal").unwrap();
    fixture.check(&["-Q", "--batch", "--eval",
        "(progn (add-hook 'kill-emacs-hook (lambda () (princ (buffer-string)))) (defun qa227-action () (setq command-line-args-left '(\"--\" \"--eval\" \"(princ 123)\"))))",
        "--funcall", "qa227-action", "--eval", "(princ 8)"], "literal", None);
}
