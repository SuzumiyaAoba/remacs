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
fn savehist_saves_a_relative_basename_in_default_directory() {
    let directory = std::env::temp_dir().join(format!(
        "remacs-savehist-relative-{}-{}",
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
    let code = format!(
        r#"(progn
          (load {})
          (setq remacs-savehist-relative-value 'saved)
          (let ((default-directory {})
                (savehist-file "history-file")
                (savehist-file-modes nil)
                (savehist-save-minibuffer-history nil)
                (savehist-additional-variables '(remacs-savehist-relative-value))
                (savehist--file-sync-modtime nil)
                (savehist-last-checksum nil)
                (savehist-save-hook nil))
            (savehist-save)
            (file-exists-p "history-file")))"#,
        lisp_string(source.to_str().unwrap()),
        lisp_string(&format!("{}/", directory.to_str().unwrap()))
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
    assert!(result.truthy());
}
