use std::{path::PathBuf, time::SystemTime};

use remacs_runtime::lisp::{Flow, Interp};

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[test]
fn savehist_list_limits_are_exact_and_do_not_mutate_values() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/savehist.el")
        .canonicalize()
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "remacs-savehist-limits-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let _temp_directory = TempDirectory(directory.clone());
    let file = directory.join("history");
    let code = format!(
        r#"(progn
          (load {})
          (setq remacs-savehist-zero '(a b c)
                remacs-savehist-one '(a b c)
                remacs-savehist-two '(a b c)
                remacs-savehist-long '(a b c d e))
          (let ((savehist-file {})
                (savehist-file-modes nil)
                (savehist-save-minibuffer-history nil)
                (savehist-additional-variables
                 '((remacs-savehist-zero . 0)
                   (remacs-savehist-one . 1)
                   (remacs-savehist-two . 2)
                   (remacs-savehist-long . 3)))
                (savehist-minibuffer-history-variables nil)
                (savehist-last-checksum nil))
            (savehist-save)
            (with-temp-buffer
              (insert-file-contents savehist-file)
              (let ((contents (buffer-string)))
                (list
                 (not (null
                       (and (string-match-p (regexp-quote "(setq remacs-savehist-zero 'nil)") contents)
                            (not (string-match-p (regexp-quote "(setq remacs-savehist-zero '(a b c))") contents))
                            (string-match-p (regexp-quote "(setq remacs-savehist-one '(a))") contents)
                            (string-match-p (regexp-quote "(setq remacs-savehist-two '(a b))") contents)
                            (string-match-p (regexp-quote "(setq remacs-savehist-long '(a b c))") contents)
                            (not (string-match-p (regexp-quote "(setq remacs-savehist-long '(a b c d e))") contents)))))
                 (equal remacs-savehist-zero '(a b c))
                 (equal remacs-savehist-one '(a b c))
                 (equal remacs-savehist-two '(a b c))
                 (equal remacs-savehist-long '(a b c d e)))))))"#,
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
