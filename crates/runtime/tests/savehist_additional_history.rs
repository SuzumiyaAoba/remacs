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
fn selected_minibuffer_histories_save_when_global_history_saving_is_disabled() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/savehist.el")
        .canonicalize()
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "remacs-savehist-additional-history-{}-{}",
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
          (defvar remacs-savehist-selected-history nil)
          (let ((command-history '(unique-audit-command))
                (remacs-savehist-selected-history '(first second))
                (savehist-file {})
                (savehist-file-modes nil)
                (savehist-save-minibuffer-history nil)
                (savehist-additional-variables
                 '(command-history (remacs-savehist-selected-history . 1)))
                (savehist-minibuffer-history-variables
                 '(command-history remacs-savehist-selected-history))
                (savehist-save-hook nil)
                (savehist--file-sync-modtime nil)
                (savehist-last-checksum nil))
            (savehist-save)
            (with-temp-buffer
              (insert-file-contents savehist-file)
              (let ((contents (buffer-string)))
                (list
                 (and (string-match-p
                       (regexp-quote "(setq command-history '(unique-audit-command))")
                       contents)
                      t)
                 (and (string-match-p
                       (regexp-quote "(setq remacs-savehist-selected-history '(first))")
                       contents)
                      t)
                 (not (string-match-p
                       (regexp-quote "(setq remacs-savehist-selected-history '(first second))")
                       contents)))))))"#,
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
