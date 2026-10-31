use std::{fs, path::PathBuf, time::SystemTime};

use remacs_runtime::lisp::{Flow, Interp};

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lisp_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[test]
fn ignored_variables_are_excluded_from_all_additional_entry_forms() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/savehist.el")
        .canonicalize()
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "remacs-savehist-ignored-additional-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let _temp_directory = TempDirectory(directory.clone());
    let file = directory.join("history");
    let code = format!(
        r#"(progn
          (load {})
          (defvar remacs-savehist-ignored-plain nil)
          (defvar remacs-savehist-ignored-limited nil)
          (defvar remacs-savehist-allowed-value nil)
          (let ((remacs-savehist-ignored-plain "SENSITIVE-PLAIN-VALUE")
                (remacs-savehist-ignored-limited '("SENSITIVE-LIST-VALUE" keep))
                (remacs-savehist-allowed-value "VISIBLE-VALUE")
                (savehist-file {})
                (savehist-file-modes nil)
                (savehist-save-minibuffer-history nil)
                (savehist-additional-variables
                 '(remacs-savehist-ignored-plain
                   (remacs-savehist-ignored-limited . 1)
                   remacs-savehist-allowed-value))
                (savehist-ignored-variables
                 '(remacs-savehist-ignored-plain remacs-savehist-ignored-limited))
                (savehist-minibuffer-history-variables nil)
                (savehist-save-hook nil)
                (savehist--file-sync-modtime nil)
                (savehist-last-checksum nil))
            (savehist-save)
            (with-temp-buffer
              (insert-file-contents savehist-file)
              (let ((contents (buffer-string)))
                (list
                 (not (string-match-p "SENSITIVE-PLAIN-VALUE" contents))
                 (not (string-match-p "SENSITIVE-LIST-VALUE" contents))
                 (and (string-match-p
                       (regexp-quote "(setq remacs-savehist-allowed-value '\"VISIBLE-VALUE\")")
                       contents)
                      t))))))"#,
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
    assert_eq!(interp.prin1_to_string(&result), "(t t t)");
}
