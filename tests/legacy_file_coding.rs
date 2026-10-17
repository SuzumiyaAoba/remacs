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
fn legacy_insert_file_contents_decoding() {
    on_big_stack(|| {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixtures = Fixtures(std::env::temp_dir().join(format!(
            "remacs-legacy-file-coding-{}-{nonce}",
            std::process::id()
        )));
        std::fs::create_dir(&fixtures.0).unwrap();
        for (name, bytes) in [
            (
                "case-0-iso-2022-jp",
                &[27, 36, 66, 70, 124, 75, 92, 27, 40, 66, 13, 10][..],
            ),
            ("case-1-cp932", &[135, 64, 129, 96, 13, 10][..]),
            ("case-2-cp1252", &[65, 128, 140, 13, 10][..]),
            ("case-3-euc-kr", &[199, 209, 177, 185, 13, 10][..]),
            ("case-4-gb18030", &[214, 208, 148, 57, 252, 54, 13, 10][..]),
            ("case-5-utf-8-emacs", &[248, 136, 128, 128, 128, 13, 10][..]),
            ("case-6-iso-2022-jp", &[27, 36, 66, 70][..]),
            ("case-7-cp932", &[129, 32, 255][..]),
            ("case-8-cp1252", &[129, 128, 157][..]),
            ("case-9-euc-kr", &[199][..]),
            ("case-10-gb18030", &[129, 48, 129][..]),
            ("case-11-utf-8-emacs", &[237, 160, 128][..]),
            (
                "case-12-chinese-hz",
                &[65, 126, 123, 86, 80, 78, 68, 126, 125, 66][..],
            ),
            ("case-13-utf-7", &[43, 90, 101, 86, 110, 76, 65, 45][..]),
        ] {
            std::fs::write(fixtures.0.join(name), bytes).unwrap();
            assert_eq!(std::fs::read(fixtures.0.join(name)).unwrap(), bytes);
        }
        let root = env!("CARGO_MANIFEST_DIR");
        let setup = format!(
            "(setq qa-legacy-file-coding-dir {:?})",
            fixtures.0.to_str().unwrap()
        );
        let framework = format!("{root}/tests/qa_framework.el");
        let cases = format!("{root}/tests/qa_legacy_file_decoding.el");
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
