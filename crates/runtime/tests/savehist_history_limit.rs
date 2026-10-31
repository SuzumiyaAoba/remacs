use std::{fs, path::PathBuf, time::SystemTime};

use remacs_runtime::lisp::{Flow, Interp};

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn savehist_applies_additional_limit_once_to_a_minibuffer_history() {
    let directory = std::env::temp_dir().join(format!(
        "remacs-savehist-history-limit-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let _temp_directory = TempDirectory(directory.clone());
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/savehist.el")
        .canonicalize()
        .unwrap();
    let file = directory.join("history");
    let code = format!(
        r#"(progn
          (load {})
          (setq remacs-savehist-combined-history
                '(newest retained must-not-leak another-private-entry))
          (let ((savehist-file {})
                (savehist-file-modes nil)
                (savehist-save-minibuffer-history t)
                (savehist-minibuffer-history-variables
                 '(remacs-savehist-combined-history))
                (savehist-additional-variables
                 '((remacs-savehist-combined-history . 2)))
                (savehist-last-checksum nil))
            (savehist-save)
            (with-temp-buffer
              (insert-file-contents savehist-file)
              (let ((contents (buffer-string)) (count 0) (position 0)
                    (needle "(setq remacs-savehist-combined-history "))
                (while (string-match (regexp-quote needle) contents position)
                  (setq count (1+ count)
                        position (match-end 0)))
                (list
                 (= count 1)
                 (not (null
                       (string-match-p
                        (regexp-quote "(setq remacs-savehist-combined-history '(newest retained))")
                        contents)))
                 (not (string-match-p (regexp-quote "must-not-leak") contents))
                 (not (string-match-p (regexp-quote "another-private-entry") contents))
                 (equal remacs-savehist-combined-history
                        '(newest retained must-not-leak another-private-entry)))))))"#,
        lisp_string(source.to_str().unwrap()),
        lisp_string(file.to_str().unwrap())
    );

    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(interp.prin1_to_string(&result), "(t t t t t)");
}
