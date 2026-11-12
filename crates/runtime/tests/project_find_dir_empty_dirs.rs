use remacs_runtime::lisp::Interp;
use std::{fs, path::PathBuf};

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[test]
fn project_find_dir_includes_empty_directories() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(project_find_dir_includes_empty_directories_inner)
        .expect("spawn project directory test thread")
        .join()
        .expect("project directory test thread");
}

fn project_find_dir_includes_empty_directories_inner() {
    let root = std::env::temp_dir().join(format!(
        "remacs-project-empty-dir-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("empty")).unwrap();
    fs::create_dir_all(root.join("nonempty")).unwrap();
    fs::write(root.join("nonempty/file.txt"), "file\n").unwrap();
    let _temp_dir = TempDir(root.clone());

    let mut interp = Interp::new();
    let root = lisp_string(&format!("{}/", root.display()));
    let setup = format!(
        r#"(progn
          (require 'project)
          (let ((project-read-file-name-function
                 (lambda (_prompt files &rest _)
                   (setq qa-project-find-dir-candidates files)
                   "./nonempty/")))
            (cl-letf (((symbol-function 'project-current)
                       (lambda (&optional _) 'fake))
                      ((symbol-function 'project-root)
                       (lambda (_) {root}))
                      ((symbol-function 'project-files)
                       (lambda (_) '("./nonempty/file.txt")))
                      ((symbol-function 'dired) (lambda (_) nil)))
              (project-find-dir)))
          qa-project-find-dir-candidates)"#
    );
    interp.eval_str(&setup).unwrap();

    let empty = interp
        .eval_str("(member \"./empty/\" qa-project-find-dir-candidates)")
        .unwrap();
    assert!(empty.truthy());
    let nonempty = interp
        .eval_str("(member \"./nonempty/\" qa-project-find-dir-candidates)")
        .unwrap();
    assert!(nonempty.truthy());
}
