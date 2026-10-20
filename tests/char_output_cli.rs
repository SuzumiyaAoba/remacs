//! Character output must encode Lisp codes, not their Rust proxy scalars.
use std::process::{Command, Stdio};

const PROGRAM: &str = r#"(dolist (code '(65 26085 55296 56319 56320 57343 1114112 2097152))
  (princ (char-to-string code)) (princ ":")
  (prin1 (char-to-string code)) (princ ":")
  (write-char code) (terpri))"#;

#[test]
fn surrogate_and_mule_output_matches_gnu_bytes() {
    let characters: &[&[u8]] = &[
        b"A",
        &[0xe6, 0x97, 0xa5],
        &[0xed, 0xa0, 0x80],
        &[0xed, 0xaf, 0xbf],
        &[0xed, 0xb0, 0x80],
        &[0xed, 0xbf, 0xbf],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xf8, 0x88, 0x80, 0x80, 0x80],
    ];
    let mut expected = Vec::new();
    for character in characters {
        expected.extend_from_slice(character);
        expected.extend_from_slice(b":\"");
        expected.extend_from_slice(character);
        expected.extend_from_slice(b"\":");
        expected.extend_from_slice(character);
        expected.push(b'\n');
    }
    for source in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_remacs"));
        command
            .args(["-Q", "--batch", "--eval", PROGRAM])
            .env_remove("EMACSLOADPATH")
            .env_remove("REMACS_NO_PRELUDE")
            .env_remove("PRELUDE_MAX")
            .stdin(Stdio::null());
        if source {
            command.env("REMACS_NO_ELC", "1");
        } else {
            command.env_remove("REMACS_NO_ELC");
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, expected, "source startup: {source}");
    }
}
