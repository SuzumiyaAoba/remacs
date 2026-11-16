use remacs_runtime::lisp::{Interp, Value};
use std::{fs, path::PathBuf, thread, time::SystemTime};

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn gpui_stipple_completion_includes_bitmap_files() {
    thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let unique = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!(
                "remacs-gpui-stipple-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&directory).unwrap();
            let _cleanup = TempDirectory(directory.clone());
            let bitmap = directory.join("checker.xbm");
            fs::write(&bitmap, b"#define checker_width 1\n#define checker_height 1\n").unwrap();

            let mut interp = Interp::new();
            let directory_lisp = interp
                .prin1_to_string(&Value::string(directory.to_string_lossy().into_owned()));
            let bitmap_lisp =
                interp.prin1_to_string(&Value::string(bitmap.to_string_lossy().into_owned()));
            let result = interp
                .eval_str(&format!(
                    "(progn \
                     (set-frame-parameter nil 'window-system 'gpui) \
                     (setq x-bitmap-file-path (list {directory_lisp})) \
                     (and (assoc {bitmap_lisp} \
                                 (face-valid-attribute-values :stipple nil)) t))"
                ))
                .unwrap();
            assert!(result.truthy(), "GPUI stipple completion should include the bitmap");
        })
        .unwrap()
        .join()
        .unwrap();
}
