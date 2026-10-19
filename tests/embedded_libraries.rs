//! An installed executable must locate its own libraries without the
//! checkout, current directory, or executable-relative source directories.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn standalone_expression() -> String {
    let checkout = env!("CARGO_MANIFEST_DIR")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        r#"(let ((load-path nil) (electric-quote-string 'standalone))
      (let* ((names '("simple" "subr-x" "cl-lib" "semantic/ctxt" "quail/emoji"))
             (paths (mapcar #'locate-library names))
             (descriptions (mapcar
                (lambda (path)
                  (list (file-name-absolute-p path) (file-readable-p path)
                        (not (string-prefix-p "{checkout}/" path)))) paths))
             (before (symbol-function 'replace-highlight)))
        (load (nth 1 paths) nil t)
        (let ((subrx-state (list (eq before (symbol-function 'replace-highlight))
                                electric-quote-string)))
          (load (locate-library "avl-tree") nil t)
          (load (nth 3 paths) nil t)
          (load (nth 4 paths) nil t)
          (load (car paths) nil t)
          (prin1 (list descriptions subrx-state
                       (featurep 'avl-tree) (featurep 'semantic/ctxt) (featurep 'emoji)
                       (locate-library "qa53-no-such-library")
                       (locate-library "simple" t)
                       (locate-library "simple" nil '("/qa53-no-path"))
                       (let* ((directory (expand-file-name "lisp"))
                              (file (expand-file-name "subr-x.el" directory))
                              (load-path (list directory)))
                         (make-directory directory)
                         (with-temp-file file (insert "(setq qa53-override 'user)\n"))
                         (let ((path (locate-library "subr-x")))
                           (load path nil t)
                           (list (equal path file) (bound-and-true-p qa53-override)))))))))"#
    )
}

fn run_standalone(no_elc: bool) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Sandbox(std::env::temp_dir().join(format!(
        "remacs-standalone-{}-{nonce}-{no_elc}",
        std::process::id()
    )));
    std::fs::create_dir(&root.0).unwrap();
    for directory in ["bin", "work", "cache"] {
        std::fs::create_dir_all(root.0.join(directory)).unwrap();
    }
    let executable = root.0.join("bin/remacs");
    std::fs::copy(env!("CARGO_BIN_EXE_remacs"), &executable).unwrap();
    let mut command = Command::new(executable);
    command
        .current_dir(root.0.join("work"))
        .env("TMPDIR", root.0.join("cache"))
        .args(["-Q", "--batch", "--eval", &standalone_expression()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if no_elc {
        command.env("REMACS_NO_ELC", "1");
    } else {
        command.env_remove("REMACS_NO_ELC");
    }
    let mut child = command.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let output = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(90);
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break (status, false);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break (child.wait().unwrap(), true);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = String::from_utf8(output.join().unwrap()).unwrap();
    let stderr = String::from_utf8_lossy(&errors.join().unwrap()).into_owned();
    let diagnostic = stderr
        .lines()
        .take(12)
        .chain(stderr.lines().rev().take(8))
        .map(|line| line.chars().take(300).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !timed_out,
        "standalone library load timed out (NO_ELC={no_elc}): {diagnostic}"
    );
    assert!(
        status.success(),
        "standalone library load failed (NO_ELC={no_elc}): {stdout}\n{diagnostic}"
    );
    assert_eq!(
        stdout,
        "(((t t t) (t t t) (t t t) (t t t) (t t t)) (t standalone) t t t nil nil nil (t user))",
        "{stderr}"
    );
}

#[test]
fn standalone_locate_paths_are_readable_and_loadable() {
    run_standalone(false);
}

#[test]
fn standalone_source_boot_locate_paths_are_readable_and_loadable() {
    run_standalone(true);
}
