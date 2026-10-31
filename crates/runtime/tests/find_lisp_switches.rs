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
fn find_lisp_dired_passes_supported_switches_as_option_letters() {
    let directory = std::env::temp_dir().join(format!(
        "remacs-find-lisp-switches-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("subdir")).unwrap();
    let _temp_directory = TempDirectory(directory.clone());

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/find-lisp.el")
        .canonicalize()
        .unwrap();
    let code = format!(
        r#"(progn
          (load {})
          (require 'cl-lib)
          (let ((default-directory {}) seen)
            (dolist (case '(("-Fi -s -c -u" (?F ?i ?s ?c ?u) t t)
                            ("--classify --inode --size --time=ctime --time=atime"
                             (?F ?i ?s ?c ?u) t t)
                            ("" nil nil nil)
                            (nil nil nil nil)
                            ("-l" nil nil nil)
                            ("--file-type --tabsize=custom" nil nil nil)))
              (let ((switches (car case))
                    (expected-options (cadr case))
                    (expect-size-and-inode (nth 2 case))
                    (expect-classified-name (nth 3 case))
                    (dired-actual-switches (car case)))
                (cl-letf (((symbol-function 'find-lisp-format-time)
                           (lambda (_attributes options _now)
                             (setq seen options)
                             "time")))
                  (with-temp-buffer
                    (find-lisp-find-dired-insert-file "subdir" (current-buffer))
                    (unless (and (equal seen expected-options)
                                 (if expect-size-and-inode
                                     (string-match-p
                                      "\\`[[:space:]]*[0-9]+[[:space:]]+[0-9]+ d"
                                      (buffer-string))
                                   (string-match-p "\\`[[:space:]]*d"
                                                   (buffer-string)))
                                 (eq expect-classified-name
                                     (not (null (string-match-p "subdir/\n\\'"
                                                               (buffer-string))))))
                      (error "Dired switches were not applied: %S %S"
                             switches (buffer-string)))))))
          t))"#,
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
