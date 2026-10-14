//! Binary fixtures for insert-file-contents; avoid depending on write-region
//! to construct the bytes whose decoding is under test. Optionally set
//! GNU_EMACS to a GNU Emacs 31.1 binary to run the same Lisp oracle checks.
mod common;

use common::{interp, on_big_stack};
use std::path::PathBuf;

struct Fixtures(PathBuf);
impl Drop for Fixtures {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn insert_file_contents_decoding() {
    on_big_stack(|| {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixtures = Fixtures(
            std::env::temp_dir().join(format!("remacs-file-coding-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir(&fixtures.0).unwrap();
        for (name, bytes) in [
            ("original", b"line1\nline2\xc3\xa9\xff\n".as_slice()),
            ("mixed", b"\xc3\xa9\xff".as_slice()),
            ("valid", b"\xc3\xa9".as_slice()),
            ("bom", b"\xef\xbb\xbf\xc3\xa9".as_slice()),
            ("crlf", b"A\r\nB\r\n".as_slice()),
            ("mac", b"A\rB\r".as_slice()),
            ("mixed-eol", b"A\r\nB\n".as_slice()),
            ("latin", b"\xe9".as_slice()),
            ("invalid", b"\xff\x80\xc0\xaf".as_slice()),
            ("truncated", b"\xe3\x81".as_slice()),
            (
                "utf8-boundaries",
                b"\xed\xa0\x80\xf0\x80\x80\xaf\xe2(\xa1\xf4\x8f\xbf\xbf".as_slice(),
            ),
            ("shift-jis", b"\x82\xa0".as_slice()),
        ] {
            std::fs::write(fixtures.0.join(name), bytes).unwrap();
        }
        let root = env!("CARGO_MANIFEST_DIR");
        let setup = format!(
            "(setq qa-file-coding-dir {:?})",
            fixtures.0.to_str().unwrap()
        );
        let framework = format!("{root}/tests/qa_framework.el");
        let cases = format!("{root}/tests/qa_file_decoding.el");
        if let Some(gnu) = std::env::var_os("GNU_EMACS") {
            let output = std::process::Command::new(gnu)
                .args([
                    "-Q", "--batch", "--eval", &setup, "-l", &framework, "-l", &cases,
                ])
                .output()
                .expect("run GNU Emacs oracle");
            assert!(
                output.status.success(),
                "GNU oracle failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let (mut i, output) = interp();
        i.noninteractive = true;
        let source = format!("{setup} (load {framework:?}) (load {cases:?})");
        if i.eval_str(&source).is_err() {
            panic!(
                "file decoding eval error; captured output:\n{}",
                output.borrow()
            );
        }
        assert!(
            output.borrow().contains("checks passed"),
            "{}",
            output.borrow()
        );
    });
}
