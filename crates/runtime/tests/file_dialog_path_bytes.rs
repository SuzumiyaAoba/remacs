#[cfg(unix)]
mod unix {
    use std::{
        ffi::OsString,
        os::unix::ffi::{OsStrExt, OsStringExt},
        path::PathBuf,
        rc::Rc,
        thread,
    };

    use remacs_runtime::{frontend::FileDialogBackend, lisp::Interp};

    struct FileDialog(PathBuf);

    impl FileDialogBackend for FileDialog {
        fn read_file_name(
            &self,
            _prompt: &str,
            _directory: Option<&str>,
            _must_match: bool,
            _initial: Option<&str>,
            _directory_only: bool,
        ) -> Result<Option<PathBuf>, String> {
            Ok(Some(self.0.clone()))
        }
    }

    #[test]
    fn native_file_dialog_preserves_non_utf8_path_bytes() {
        thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let path = PathBuf::from(OsString::from_vec(b"/tmp/remacs-\xff.txt".to_vec()));
                let mut interp = Interp::new();
                interp.set_file_dialog_backend(Some(Rc::new(FileDialog(path.clone()))));
                interp
                    .eval_str("(set-frame-parameter nil 'window-system 'gpui)")
                    .unwrap();

                let selected = interp
                    .eval_str("(ns-read-file-name \"Open\" nil nil nil nil)")
                    .unwrap();
                let remacs_runtime::lisp::Value::Str(selected) = selected else {
                    panic!("file dialog did not return a string");
                };
                let native = selected.borrow().to_os_string().unwrap();
                assert_eq!(native.as_os_str().as_bytes(), path.as_os_str().as_bytes());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
