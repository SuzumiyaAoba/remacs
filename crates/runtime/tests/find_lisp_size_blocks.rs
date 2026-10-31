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
fn find_lisp_size_switch_uses_ceiling_kibibytes() {
    let directory = std::env::temp_dir().join(format!(
        "remacs-find-lisp-size-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let _temp_directory = TempDirectory(directory.clone());
    for (name, size) in [("empty", 0), ("exact", 1024), ("over", 1025)] {
        fs::write(directory.join(name), vec![b'x'; size]).unwrap();
    }

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../lisp/find-lisp.el")
        .canonicalize()
        .unwrap();
    let paths = ["empty", "exact", "over"]
        .map(|name| lisp_string(directory.join(name).to_str().unwrap()))
        .join(" ");
    let code = format!(
        "(progn (load {}) (mapcar (lambda (path)\
           (substring (find-lisp-format path (file-attributes path 'string)\
                       '(?s) (current-time)) 0 4))\
         (list {})))",
        lisp_string(source.to_str().unwrap()),
        paths
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
    assert_eq!(interp.prin1_to_string(&result), "(\"   0\" \"   1\" \"   2\")");
}
