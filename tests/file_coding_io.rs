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
fn file_coding_round_trips() {
    on_big_stack(|| {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixtures = Fixtures(std::env::temp_dir().join(format!(
            "remacs-file-coding-io-{}-{nonce}",
            std::process::id()
        )));
        std::fs::create_dir(&fixtures.0).unwrap();
        for (name, bytes) in [
            ("bad", &[97, 98, 99, 255, 254, 128, 100, 101, 102, 10][..]),
            ("bom", &[239, 187, 191, 97, 98, 99, 13, 10][..]),
            (
                "cookie",
                &[
                    59, 59, 32, 45, 42, 45, 32, 99, 111, 100, 105, 110, 103, 58, 32, 101, 117, 99,
                    45, 106, 112, 59, 32, 45, 42, 45, 10, 198, 252, 203, 220, 10,
                ][..],
            ),
            (
                "crlf",
                &[
                    108, 105, 110, 101, 49, 13, 10, 108, 105, 110, 101, 50, 13, 10,
                ][..],
            ),
            ("empty", &[][..]),
            (
                "euc",
                &[
                    198, 252, 203, 220, 184, 236, 164, 206, 165, 198, 165, 173, 165, 185, 165, 200,
                    10, 164, 179, 164, 243, 164, 203, 164, 193, 164, 207, 10,
                ][..],
            ),
            ("iso", &[27, 36, 66, 70, 124, 75, 92, 27, 40, 66, 10][..]),
            ("latin", &[99, 97, 102, 233, 10][..]),
            ("mixed", &[195, 169, 255][..]),
            (
                "sjis",
                &[
                    147, 250, 150, 123, 140, 234, 130, 204, 131, 101, 131, 76, 131, 88, 131, 103,
                    10, 130, 177, 130, 241, 130, 201, 130, 191, 130, 205, 10,
                ][..],
            ),
            (
                "tail",
                &[
                    198, 252, 203, 220, 10, 10, 76, 111, 99, 97, 108, 32, 86, 97, 114, 105, 97, 98,
                    108, 101, 115, 58, 10, 99, 111, 100, 105, 110, 103, 58, 32, 101, 117, 99, 45,
                    106, 112, 10, 69, 110, 100, 58, 10,
                ][..],
            ),
            ("u16be", &[101, 229, 103, 44, 0, 13, 0, 10][..]),
            (
                "u16be-bom",
                &[
                    254, 255, 101, 229, 103, 44, 0, 13, 0, 10, 216, 66, 223, 183, 0, 13, 0, 10,
                ][..],
            ),
            (
                "u16le",
                &[255, 254, 229, 101, 44, 103, 10, 0, 66, 216, 183, 223, 10, 0][..],
            ),
            (
                "utf8",
                &[229, 190, 139, 230, 151, 165, 230, 156, 172, 10][..],
            ),
        ] {
            std::fs::write(fixtures.0.join(name), bytes).unwrap();
            assert_eq!(std::fs::read(fixtures.0.join(name)).unwrap(), bytes);
        }
        let root = env!("CARGO_MANIFEST_DIR");
        let setup = format!(
            "(setq qa-file-coding-io-dir {:?})",
            fixtures.0.to_str().unwrap()
        );
        let framework = format!("{root}/tests/qa_framework.el");
        let cases = format!("{root}/tests/qa_file_coding_io.el");
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
