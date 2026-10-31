use remacs_runtime::{
    frontend::{Grid, render_frame_grid_with_input_cached, render_grid},
    lisp::{Flow, Interp, Value},
};
use std::path::{Path, PathBuf};

const MODERN: &str = "#define p_width 9\n#define p_height 2\n\
    static unsigned char p_bits[] = {0x81, 0x01, 0x55, 0x01};\n";
const SMALL: &str = "#define p_width 8\n#define p_height 1\n\
    static char p_bits[] = {0x42};\n";
const LEGACY: &str = "#define p_width 17\n#define p_height 2\n\
    static short p_bits[] = {0x0201, 0xff03, 0x0504, 0xee06};\n";

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "remacs-stipple-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn eval(interp: &mut Interp, source: &str) -> Value {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    })
}

fn native_name(path: &Path) -> Value {
    Value::string(path.to_str().expect("UTF-8 fixture directory"))
}

fn set_var(interp: &mut Interp, name: &str, value: Value) {
    let symbol = interp.intern(name);
    interp.set_symbol_default(symbol, value).unwrap();
}

fn set_file(interp: &mut Interp, name: Value) {
    set_var(interp, "stipple-test-file", name);
    eval(
        interp,
        "(set-face-attribute 'default nil :stipple stipple-test-file)",
    );
}

fn assert_pattern(grid: &Grid, width: u16, height: u16, data: &[u8]) {
    let id = grid.attrs[0][0]
        .stipple
        .expect("file stipple on rendered text");
    let bitmap = &grid.stipples[usize::from(id)];
    assert_eq!((bitmap.width, bitmap.height), (width, height));
    assert_eq!(&*bitmap.data, data);
    assert!(
        grid.attrs[0][..4]
            .iter()
            .all(|style| style.stipple == Some(id))
    );
}

#[test]
fn bitmap_files_resolve_into_the_same_patterns_as_inline_stipples() {
    let root = Directory::new();
    let first = root.0.join("first");
    let second = root.0.join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::create_dir(first.join("sub")).unwrap();
    for name in ["pattern.xbm", "reverse.xbm"] {
        std::fs::write(first.join(name), MODERN).unwrap();
        std::fs::write(second.join(name), SMALL).unwrap();
    }
    std::fs::write(first.join("default.xbm"), LEGACY).unwrap();
    std::fs::write(second.join("default.xbm"), SMALL).unwrap();
    std::fs::write(first.join("sub/relative.xbm"), MODERN).unwrap();
    std::fs::write(first.join("模様.xbm"), SMALL).unwrap();
    std::fs::write(first.join("broken.xbm"), "not a bitmap").unwrap();
    std::fs::write(second.join("broken.xbm"), SMALL).unwrap();

    let mut interp = Interp::new();
    set_var(&mut interp, "stipple-test-directory", native_name(&first));
    set_var(
        &mut interp,
        "stipple-test-second-directory",
        native_name(&second),
    );
    eval(
        &mut interp,
        "(progn
      (set-frame-parameter nil 'window-system 'ns)
      (with-current-buffer \"*scratch*\"
        (erase-buffer) (insert \"stipple test\") (goto-char 1)
        (setq default-directory stipple-test-directory mark-active nil)))",
    );

    set_var(
        &mut interp,
        "x-bitmap-file-path",
        Value::list(vec![native_name(&first), native_name(&second)]),
    );
    set_file(&mut interp, Value::string("pattern.xbm"));
    let grid = render_grid(&interp, 80, 24);
    assert_pattern(&grid, 9, 2, &[0x81, 1, 0x55, 1]);

    // A file and an equivalent explicit pattern share one grid bitmap.
    eval(
        &mut interp,
        "(progn
      (make-face 'stipple-inline)
      (set-face-attribute 'stipple-inline nil :stipple
                          (vector 9 2 (unibyte-string #x81 1 #x55 1)))
      (put-text-property 2 3 'face 'stipple-inline))",
    );
    let grid = render_grid(&interp, 80, 24);
    assert_pattern(&grid, 9, 2, &[0x81, 1, 0x55, 1]);
    assert_eq!(grid.stipples.len(), 1);
    eval(
        &mut interp,
        "(remove-text-properties 1 (point-max) '(face nil))",
    );

    set_var(
        &mut interp,
        "x-bitmap-file-path",
        Value::list(vec![native_name(&second), native_name(&first)]),
    );
    set_file(&mut interp, Value::string("reverse.xbm"));
    assert_pattern(&render_grid(&interp, 80, 24), 8, 1, &[0x42]);

    // Absolute names bypass the directory search.
    set_file(&mut interp, native_name(&first.join("pattern.xbm")));
    assert_pattern(&render_grid(&interp, 80, 24), 9, 2, &[0x81, 1, 0x55, 1]);

    set_var(&mut interp, "x-bitmap-file-path", Value::Nil);
    set_file(&mut interp, Value::string("default.xbm"));
    assert_pattern(&render_grid(&interp, 80, 24), 17, 2, &[1, 2, 3, 4, 5, 6]);
    set_var(
        &mut interp,
        "x-bitmap-file-path",
        Value::list(vec![Value::string("sub")]),
    );
    set_file(&mut interp, Value::string("relative.xbm"));
    assert_pattern(&render_grid(&interp, 80, 24), 9, 2, &[0x81, 1, 0x55, 1]);

    set_var(
        &mut interp,
        "x-bitmap-file-path",
        Value::list(vec![native_name(&first), native_name(&second)]),
    );
    set_file(&mut interp, Value::string("broken.xbm"));
    assert!(render_grid(&interp, 80, 24).stipples.is_empty());
    set_file(&mut interp, Value::string("missing.xbm"));
    assert!(render_grid(&interp, 80, 24).stipples.is_empty());
    set_file(&mut interp, Value::Nil);
    assert!(render_grid(&interp, 80, 24).stipples.is_empty());
    set_file(&mut interp, native_name(&first.join("模様.xbm")));
    assert_pattern(&render_grid(&interp, 80, 24), 8, 1, &[0x42]);

    // Rendering another frame must use that frame's buffer directory.
    set_var(&mut interp, "x-bitmap-file-path", Value::Nil);
    set_file(&mut interp, Value::string("default.xbm"));
    let other = eval(
        &mut interp,
        "(let ((frame (make-frame)))
      (with-selected-frame frame
        (switch-to-buffer (get-buffer-create \"stipple-other\"))
        (setq default-directory stipple-test-second-directory)
        (insert \"other stipple\") (goto-char 1))
      frame)",
    );
    let Value::Frame(other) = other else {
        panic!("new frame");
    };
    let grid = render_frame_grid_with_input_cached(&interp, &other, 80, 24, None);
    assert_pattern(&grid, 8, 1, &[0x42]);

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = first.join(std::ffi::OsString::from_vec(b"raw-\xff.xbm".to_vec()));
        match std::fs::write(&path, SMALL) {
            Ok(()) => {
                let filename = eval(
                    &mut interp,
                    "(concat stipple-test-directory \"/raw-\"
          (string-as-multibyte (unibyte-string 255)) \".xbm\")",
                );
                set_file(&mut interp, filename);
                assert_pattern(&render_grid(&interp, 80, 24), 8, 1, &[0x42]);
            }
            // APFS rejects these names before the renderer can read them.
            Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => {}
            Err(error) => panic!("create raw-byte bitmap: {error}"),
        }
    }
}
