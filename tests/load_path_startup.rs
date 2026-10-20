//! Real startup with a copied executable, without a checkout or installed
//! Lisp tree. Set GNU_EMACS to compare environment path semantics directly.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Self(
            std::env::temp_dir().join(format!("remacs-load-path-{}-{nonce}", std::process::id())),
        );
        for name in ["bin", "work", "cache", "A", "B"] {
            std::fs::create_dir_all(fixture.0.join(name)).unwrap();
        }
        std::fs::copy(env!("CARGO_BIN_EXE_remacs"), fixture.0.join("bin/remacs")).unwrap();
        for name in ["A", "B"] {
            let directory = fixture.0.join(name);
            std::fs::write(directory.join("qa108-external.el"),
                ";;; -*- lexical-binding: t; -*-\n(defun qa108-external () 108)\n(provide 'qa108-external)\n").unwrap();
            std::fs::write(directory.join("avl-tree.el"), format!(
                ";;; -*- lexical-binding: t; -*-\n(defun qa108-origin () '{name})\n(provide 'avl-tree)\n")).unwrap();
        }
        // These environment directories must not hijack internal boot.
        std::fs::write(
            fixture.0.join("A/remacs-compat.el"),
            "(error \"Environment used during bootstrap\")\n",
        )
        .unwrap();
        fixture
    }

    fn check(&self, environment: &str, labels: &str, origin: &str, no_elc: bool, external: bool) {
        let a = self.0.join("A").to_string_lossy().into_owned();
        let b = self.0.join("B").to_string_lossy().into_owned();
        let expression = format!(
            r#"
(let (labels)
  (dolist (directory load-path)
    (cond ((or (equal directory {a:?}) (equal directory "../A")) (push 'A labels))
          ((equal directory {b:?}) (push 'B labels))
          ((or (file-readable-p (expand-file-name "simple.el" directory))
               (file-readable-p (expand-file-name "simple.elc" directory))
               (file-readable-p (expand-file-name "simple.el.gz" directory))) (push 'D labels))))
  (unless (equal (nreverse labels) '{labels}) (error "Path order: %S" load-path))
  (when {external}
    (require 'qa108-external)
    (unless (= (qa108-external) 108) (error "External feature lookup")))
  (load "avl-tree" nil t)
  (unless (eq (if (fboundp 'qa108-origin) (qa108-origin) 'default) '{origin})
    (error "Library priority: %S" load-path))
  (let ((path (locate-library "avl-tree")))
    (unless (and (file-readable-p path)
                 (member (directory-file-name (file-name-directory path))
                         (mapcar (lambda (directory) (directory-file-name (expand-file-name directory))) load-path)))
      (error "Library outside public load-path: %S" path)))
  (prin1 'ok))
"#,
            external = if external { "t" } else { "nil" }
        );
        let mut programs = vec![self.0.join("bin/remacs")];
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            programs.push(PathBuf::from(gnu));
        }
        for program in programs {
            let mut command = Command::new(&program);
            command
                .current_dir(self.0.join("work"))
                .env("TMPDIR", self.0.join("cache"))
                .env("EMACSLOADPATH", environment)
                .args(["-Q", "--batch", "--eval", &expression]);
            if no_elc {
                command.env("REMACS_NO_ELC", "1");
            } else {
                command.env_remove("REMACS_NO_ELC");
            }
            let (status, stdout, stderr) = run(command);
            assert!(
                status && stdout == "ok",
                "{program:?}, EMACSLOADPATH={environment:?}: {stdout}\n{stderr}"
            );
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(mut command: Command) -> (bool, String, String) {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    };
    let output = read(Box::new(stdout));
    let errors = read(Box::new(stderr));
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status.success();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    (
        status,
        String::from_utf8_lossy(&output.join().unwrap()).into_owned(),
        String::from_utf8_lossy(&errors.join().unwrap()).into_owned(),
    )
}

#[test]
fn environment_paths_and_default_placeholders() {
    let fixture = Fixture::new();
    let a = fixture.0.join("A").to_string_lossy().into_owned();
    let b = fixture.0.join("B").to_string_lossy().into_owned();
    for (environment, labels, origin, external) in [
        (a.clone(), "(A)", "A", true),
        (format!("{a}::{b}"), "(A D B)", "A", true),
        (format!(":{b}"), "(D B)", "default", true),
        (format!("{a}:"), "(A D)", "A", true),
        (String::new(), "(D)", "default", false),
        (":".into(), "(D D)", "default", false),
        (format!("{a}:{a}"), "(A A)", "A", true),
        ("../A".into(), "(A)", "A", true),
    ] {
        fixture.check(&environment, labels, origin, false, external);
    }
}

#[test]
fn source_boot_uses_internal_libraries_before_environment_paths() {
    let fixture = Fixture::new();
    let environment = format!(
        "{}::{}",
        fixture.0.join("A").display(),
        fixture.0.join("B").display()
    );
    fixture.check(&environment, "(A D B)", "A", true, true);
}
