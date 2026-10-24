//! Real process re-execution, using disposable executables and loop guards.
#![cfg(unix)]
use std::ffi::OsString;
use std::io::Read;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Gnu {
    binary: PathBuf,
    dump: Option<PathBuf>,
    native: Option<PathBuf>,
}

fn gnu() -> Option<&'static Gnu> {
    static GNU: OnceLock<Option<Gnu>> = OnceLock::new();
    GNU.get_or_init(|| {
        let binary = PathBuf::from(std::env::var_os("GNU_EMACS")?);
        let output = Command::new(&binary).args(["-Q", "--batch", "--eval",
            "(progn (when (fboundp 'pdumper-stats) (princ (cdr (assq 'dump-file-name (pdumper-stats))))) (terpri) (when (boundp 'native-comp-eln-load-path) (princ (car (last native-comp-eln-load-path)))))"])
            .output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = String::from_utf8(output.stdout).unwrap();
        let mut lines = text.lines();
        let mut path = || lines.next().filter(|s| !s.is_empty() && *s != "nil").map(PathBuf::from);
        Some(Gnu { binary, dump: path(), native: path() })
    }).as_ref()
}

struct Fixture {
    root: PathBuf,
    executable: PathBuf,
    arguments: Vec<OsString>,
    source: bool,
}

impl Fixture {
    fn new(name: &str, source: bool, reference: Option<&Gnu>) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("remacs-restart-{}-{name}-{nonce}", std::process::id()));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir(root.join("cache")).unwrap();
        let executable = root.join("bin").join(if reference.is_some() { "emacs" } else { "remacs" });
        let original = reference.map(|g| g.binary.as_path()).unwrap_or_else(|| env!("CARGO_BIN_EXE_remacs").as_ref());
        std::fs::copy(original, &executable).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut arguments = Vec::new();
        if let Some(gnu) = reference {
            if let Some(native) = &gnu.native {
                // pdumper relocates preloaded native units relative to execdir.
                symlink(native, root.join("native-lisp")).unwrap();
            }
            if let Some(dump) = &gnu.dump {
                arguments.extend([OsString::from("--dump-file"), dump.as_os_str().to_owned()]);
            }
        }
        Self { root, executable, arguments, source }
    }

    fn expand(&self, text: &str) -> String {
        let quote = |path: PathBuf| serde_json::to_string(path.to_str().unwrap()).unwrap();
        text.replace("@EXE@", &quote(self.executable.clone()))
            .replace("@MARKER@", &quote(self.root.join("marker")))
            .replace("@TRACE@", &quote(self.root.join("trace")))
    }

    fn run(&self, first: &str, second: &str, program: Option<PathBuf>) -> Output {
        let script = self.root.join("restart.el");
        let text = format!(";;; -*- lexical-binding: t; -*-\n(if (file-exists-p @MARKER@)\n(progn {second})\n(with-temp-file @MARKER@ (insert (number-to-string (emacs-pid))))\n{first})\n");
        std::fs::write(&script, self.expand(&text)).unwrap();
        let mut command = Command::new(program.unwrap_or_else(|| self.executable.clone()));
        command.current_dir(&self.root).arg("-Q").args(&self.arguments)
            .args(["--batch", "--load"]).arg(script)
            .env("QA_RESTART_ENV", "original")
            .env("TMPDIR", self.root.join("cache"))
            .env_remove("EMACSLOADPATH")
            .env("PATH", std::env::join_paths(std::iter::once(self.root.join("bin"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()))).unwrap())
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if self.source { command.env("REMACS_NO_ELC", "1"); }
        else { command.env_remove("REMACS_NO_ELC"); }
        let mut child = command.spawn().unwrap();
        let read = |mut pipe: Box<dyn Read + Send>| std::thread::spawn(move || {
            let mut bytes = Vec::new(); pipe.read_to_end(&mut bytes).unwrap(); bytes
        });
        let stdout = read(Box::new(child.stdout.take().unwrap()));
        let stderr = read(Box::new(child.stderr.take().unwrap()));
        let deadline = Instant::now() + Duration::from_secs(300);
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline {
                timed_out = true;
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output = Output { status, stdout: stdout.join().unwrap(), stderr: stderr.join().unwrap() };
        assert!(!timed_out, "restart timed out: {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        output
    }

    fn check(&self, first: &str, second: &str, code: i32, stdout: &str, trace: Option<&str>, program: Option<PathBuf>) -> Output {
        let output = self.run(first, second, program);
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "source={}\n{}\n{err}", self.source, String::from_utf8_lossy(&output.stdout));
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "source={}\n{err}", self.source);
        if let Some(expected) = trace {
            assert_eq!(std::fs::read_to_string(self.root.join("trace")).unwrap_or_default(), expected);
        }
        assert!(!err.contains("panicked"), "{err}");
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.root); }
}

fn each(name: &str, source: bool, run: impl Fn(&Fixture)) {
    let reference_only = std::env::var_os("REMACS_RESTART_GNU_ONLY").is_some();
    if !reference_only {
        run(&Fixture::new(name, false, None));
        if source { run(&Fixture::new(name, true, None)); }
    }
    if let Some(reference) = gnu() { run(&Fixture::new(name, false, Some(reference))); }
    else { assert!(!reference_only, "GNU_EMACS is required for reference-only checks"); }
}

const BAD_RESTART: &str = "(princ \"BAD-RESTART\") (kill-emacs 31)";

#[test]
fn restart_keeps_the_pid_original_arguments_and_os_environment() {
    each("identity", true, |f| {
        f.check(r#"
          (write-region "FIRST\n" nil @TRACE@ t 'silent)
          (setq kill-emacs-hook (list (lambda () (write-region "HOOK\n" nil @TRACE@ t 'silent))))
          (setq command-line-args '("changed") command-line-args-left nil)
          (setenv "QA_RESTART_ENV" "modified")
          (unwind-protect (kill-emacs 17 '(restart))
            (write-region "CLEANUP\n" nil @TRACE@ t 'silent))"#,
          r#"(prin1 (list (= (emacs-pid) (string-to-number (with-temp-buffer (insert-file-contents @MARKER@) (buffer-string)))) (getenv "QA_RESTART_ENV"))) (kill-emacs 23)"#,
          23, "(t \"original\")", Some("FIRST\nHOOK\n"), None);
    });
}

#[test]
fn aborted_restart_does_not_arm_a_later_normal_exit() {
    each("aborted", true, |f| {
        f.check(r#"(let ((kill-emacs-hook (list (lambda () (princ "HOOK") (throw 'abort 'stopped)))))
            (prin1 (catch 'abort (unwind-protect (kill-emacs 17 t) (princ "CLEANUP")))))
          (princ "AFTER")"#, BAD_RESTART, 0, "HOOKCLEANUPstoppedAFTER", None, None);
    });
}

#[test]
fn an_inner_normal_exit_cancels_the_outer_restart() {
    each("inner-normal", false, |f| {
        f.check(r#"(setq kill-emacs-hook (list (lambda () (setq kill-emacs-hook nil) (princ "HOOK") (kill-emacs 29))))
          (unwind-protect (kill-emacs 17 t) (princ "CLEANUP"))"#,
          BAD_RESTART, 29, "HOOK", None, None);
    });
}

#[test]
fn an_inner_restart_supersedes_the_outer_normal_exit() {
    each("inner-restart", false, |f| {
        f.check(r#"(setq kill-emacs-hook (list (lambda () (setq kill-emacs-hook nil)
              (write-region "HOOK\n" nil @TRACE@ t 'silent) (kill-emacs 44 t))))
          (unwind-protect (kill-emacs 17) (write-region "CLEANUP\n" nil @TRACE@ t 'silent))"#,
          "(princ \"SECOND\") (kill-emacs 23)", 23, "SECOND", Some("HOOK\n"), None);
    });
}

#[test]
fn a_restart_from_a_worker_skips_the_parent_cleanup() {
    each("worker", false, |f| {
        f.check(r#"(let ((worker (make-thread (lambda () (kill-emacs 17 t)))))
            (unwind-protect (thread-join worker) (write-region "CLEANUP" nil @TRACE@ t 'silent)))"#,
          "(princ \"SECOND\") (kill-emacs 23)", 23, "SECOND", Some(""), None);
    });
}

#[test]
fn missing_executable_is_rejected_before_shutdown_hooks() {
    each("missing'quote`", false, |f| {
        f.check(r#"(delete-file @EXE@)
          (let* ((count 0) (kill-emacs-hook (list (lambda () (setq count (1+ count))))))
            (condition-case err (kill-emacs 17 t)
              (error (unless (equal err (list 'error (format-message "Emacs executable \"%s\" can't be found" @EXE@))) (signal (car err) (cdr err)))))
            (prin1 count))"#, BAD_RESTART, 0, "0", None, None);
    });
}

#[test]
fn deleting_the_executable_in_a_hook_reports_exec_failure_and_keeps_the_status() {
    each("deleted-in-hook", false, |f| {
        let output = f.check(r#"(setq kill-emacs-hook (list (lambda () (princ "HOOK") (delete-file @EXE@))))
          (unwind-protect (kill-emacs 17 t) (princ "CLEANUP"))"#,
          BAD_RESTART, 17, "HOOK", None, None);
        assert!(String::from_utf8_lossy(&output.stderr).contains("Unable to re-execute Emacs: No such file or directory"));
    });
}

#[test]
fn restart_validation_checks_existence_rather_than_execute_permission() {
    each("permission", false, |f| {
        let output = f.check(r#"(set-file-modes @EXE@ #o600)
          (setq kill-emacs-hook (list (lambda () (princ "HOOK"))))
          (unwind-protect (kill-emacs 17 t) (princ "CLEANUP"))"#,
          BAD_RESTART, 17, "HOOK", None, None);
        assert!(String::from_utf8_lossy(&output.stderr).contains("Unable to re-execute Emacs: Permission denied"));
    });
}

#[test]
fn a_symlink_invocation_reexecutes_its_original_target() {
    each("symlink", false, |f| {
        let alias = f.root.join("bin/alias");
        symlink(&f.executable, &alias).unwrap();
        f.check("(delete-file \"bin/alias\") (kill-emacs 17 t)",
          "(princ \"SECOND\") (kill-emacs 23)", 23, "SECOND", None, Some(alias));
    });
}

#[test]
fn a_path_invocation_reexecutes_its_resolved_executable() {
    each("path", false, |f| {
        f.check("(setenv \"PATH\" \"/absent\") (kill-emacs 17 t)",
          "(princ \"SECOND\") (kill-emacs 23)", 23, "SECOND", None,
          Some(PathBuf::from(f.executable.file_name().unwrap())));
    });
}

#[test]
fn successful_exec_discards_unflushed_batch_output() {
    each("buffered-output", false, |f| {
        f.check(r#"(princ "FIRST") (setq kill-emacs-hook (list (lambda () (princ "HOOK")))) (kill-emacs 17 t)"#,
          "(princ \"SECOND\") (kill-emacs 23)", 23, "SECOND", None, None);
    });
}

#[test]
fn explicit_flush_publishes_output_before_exec() {
    each("flushed-output", false, |f| {
        f.check(r#"(princ "FIRST") (flush-standard-output) (princ "DISCARDED") (kill-emacs 17 t)"#,
          "(princ \"SECOND\") (kill-emacs 23)", 23, "FIRSTSECOND", None, None);
    });
}

#[test]
fn terminal_modes_are_restored_before_reexecution() {
    each("terminal", false, |f| {
        let script = f.root.join("terminal.el");
        std::fs::write(&script, f.expand(r#";;; -*- lexical-binding: t; -*-
          (if (file-exists-p @MARKER@)
              (kill-emacs 23)
            (global-set-key (kbd "C-c C-r")
              (lambda () (interactive) (kill-emacs 17 t)))
            (with-temp-file @MARKER@ (insert "ready")))"#)).unwrap();
        let output = Command::new("python3").arg("-B")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/restart_session.py"))
            .arg(&f.executable).arg(script).arg(f.root.join("marker")).args(&f.arguments)
            .output().unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["status"], 23);
        assert_eq!(result["terminal_restored"], true);
    });
}
