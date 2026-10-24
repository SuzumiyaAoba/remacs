//! Definition provenance without a checkout, and after moving a saved image.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Self(std::env::temp_dir().join(format!(
            "remacs-history-{}-{nonce}-{name}",
            std::process::id()
        )));
        for name in ["bin", "work", "cache"] {
            std::fs::create_dir_all(fixture.0.join(name)).unwrap();
        }
        std::fs::copy(env!("CARGO_BIN_EXE_remacs"), fixture.0.join("bin/remacs")).unwrap();
        fixture
    }
    fn run(&self, args: &[&str], source: bool) -> String {
        let mut command = Command::new(self.0.join("bin/remacs"));
        command
            .current_dir(self.0.join("work"))
            .args(args)
            .env_remove("EMACSLOADPATH")
            .env("TMPDIR", self.0.join("cache"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if source {
            command.env("REMACS_NO_ELC", "1");
        } else {
            command.env_remove("REMACS_NO_ELC");
        }
        let mut child = command.spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = std::thread::spawn(move || {
            let mut data = Vec::new();
            stdout.read_to_end(&mut data).unwrap();
            data
        });
        let err = std::thread::spawn(move || {
            let mut data = Vec::new();
            stderr.read_to_end(&mut data).unwrap();
            data
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("startup history test timed out");
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let output = String::from_utf8(out.join().unwrap()).unwrap();
        let errors = String::from_utf8_lossy(&err.join().unwrap()).into_owned();
        assert!(status.success(), "{output}\n{errors}");
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn copied_binary_exposes_readable_provenance_and_keeps_external_loads() {
    let fixture = Fixture::new("copied");
    let user = fixture.0.join("work/user-module.el");
    std::fs::write(&user, ";;; -*- lexical-binding: t; -*-\n(defun qa122-external () 122)\n(provide 'qa122-external)\n").unwrap();
    let cache = fixture.0.join("cache").to_string_lossy().into_owned();
    let user = user.to_string_lossy().into_owned();
    let program = format!(
        r#"(progn
      (dolist (entry load-history)
        (let ((file (car entry)))
          (when (stringp file)
            (unless (and (file-readable-p file) (string-prefix-p {cache:?} file))
              (error "Unusable dumped path %S" file)))))
      (require 'loadhist)
      (dolist (name '(cl-defgeneric macroexpand-all forward-paragraph))
        (unless (file-readable-p (symbol-file name 'defun)) (error "Missing provenance %S" name)))
      (let* ((vc-handled-backends nil) (location (find-function-noselect 'cl-defgeneric)))
        (unless (file-readable-p (buffer-file-name (car location))) (error "Unreadable definition")))
      (load {user:?} nil t)
      (unless (and (= (qa122-external) 122)
                   (equal (symbol-file 'qa122-external 'defun) {user:?})
                   (equal (feature-file 'qa122-external) {user:?}))
        (error "External provenance replaced"))
      (princ "ok"))"#
    );
    for source in [false, true] {
        assert_eq!(fixture.run(&["--batch", "--eval", &program], source), "ok");
    }
}

#[test]
fn image_relocates_bundled_provenance_when_installed_sources_disappear() {
    let fixture = Fixture::new("image");
    let lisp = fixture.0.join("lisp");
    std::fs::create_dir(&lisp).unwrap();
    for name in ["simple.el", "cl-generic.el"] {
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("lisp")
                .join(name),
            lisp.join(name),
        )
        .unwrap();
    }
    let image = fixture
        .0
        .join("history.pdmp")
        .to_string_lossy()
        .into_owned();
    let save = format!(
        r#"(progn
      (defvar qa122-captured (symbol-file 'cl-defgeneric 'defun))
      (defvar qa122-literal (copy-sequence qa122-captured))
      (defvar qa122-path-index (make-hash-table :test 'equal))
      (puthash qa122-captured 'present qa122-path-index)
      (let ((current-load-list (list qa122-captured)))
        (fset 'qa122-redefined 'car) (defalias 'qa122-redefined 'cdr))
      (let ((vc-handled-backends nil)) (find-file-noselect qa122-captured))
      (dump-emacs-portable {image:?}) (princ "saved"))"#
    );
    assert_eq!(fixture.run(&["--batch", "--eval", &save], false), "saved");
    std::fs::remove_dir_all(lisp).unwrap();
    std::fs::remove_dir_all(fixture.0.join("cache")).unwrap();
    std::fs::create_dir(fixture.0.join("cache")).unwrap();
    let probe = r#"(progn
      (unless (and (file-readable-p qa122-captured)
                   (eq qa122-captured (symbol-file 'cl-defgeneric 'defun))
                   (eq qa122-captured (car (get 'qa122-redefined 'function-history)))
                   (eq (gethash qa122-captured qa122-path-index) 'present))
        (error "Source identity did not relocate: %S" qa122-captured))
      (when (or (equal qa122-literal qa122-captured) (file-exists-p qa122-literal))
        (error "Unrelated literal string was rewritten"))
      (unless (member (file-name-directory qa122-captured)
                      (mapcar #'file-name-as-directory load-path))
        (error "Source directory missing from load-path"))
      (dolist (entry load-history)
        (when (and (stringp (car entry)) (not (file-readable-p (car entry))))
          (error "Restored missing source %S" (car entry))))
      (with-current-buffer "cl-generic.el"
        (unless (and (file-readable-p buffer-file-name) (file-directory-p default-directory))
          (error "Source buffer did not relocate: %S in %S" buffer-file-name default-directory)))
      (princ "restored"))"#;
    assert_eq!(
        fixture.run(&["--batch", "--dump-file", &image, "--eval", probe], false),
        "restored"
    );
}

#[test]
fn image_retains_an_installed_native_trampoline_after_cell_restoration() {
    let fixture = Fixture::new("native-trampoline");
    if fixture.run(&["--batch", "--eval", "(princ (native-comp-available-p))"], false) != "t" {
        return;
    }
    let image = fixture.0.join("trampoline.pdmp").to_string_lossy().into_owned();
    let output = fixture.0.join("restored.eln").to_string_lossy().into_owned();
    let save = format!(r#"(progn
      (let ((original (symbol-function 'symbol-value)))
        (fset 'symbol-value (lambda (_) 'replacement))
        (fset 'symbol-value original))
      (dump-emacs-portable {image:?}) (princ "saved"))"#);
    assert_eq!(fixture.run(&["--batch", "--eval", &save], false), "saved");
    let probe = format!(r#"(progn
      (require 'comp)
      (defvar qa-image-global 42)
      (let ((function (native-compile
                        (lambda () (setq quit-flag t inhibit-quit nil) qa-image-global)
                        {output:?}))
            (inhibit-quit t) (quit-flag nil))
        (let ((result (condition-case nil
                          (let ((inhibit-quit t)) (funcall function))
                        (quit 'caught))))
          (setq quit-flag nil)
          (prin1 result))))"#);
    assert_eq!(fixture.run(&["--batch", "--dump-file", &image, "--eval", &probe], false), "caught");
}
