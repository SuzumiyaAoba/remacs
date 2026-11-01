use remacs_runtime::{
    editor::FrameRef,
    frontend::render_frame_grid_with_input_cached,
    lisp::{Flow, Interp, Value},
};

fn eval(i: &mut Interp, source: &str) -> Value {
    i.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{source}: {} {}",
            i.prin1_to_string(&signal.symbol()),
            i.prin1_to_string(&signal.data())
        ),
        other => panic!("{source}: {other:?}"),
    })
}

#[track_caller]
fn check(i: &mut Interp, frame: &FrameRef, z: &str, with_eol: &str) {
    let current = i.current_buffer;
    let selected = i.selected_frame.as_ref().unwrap().clone();
    for form in [r#""%z|%Z|%%z|%%Z""#, r#"("" "%z" "|" "%Z" "|%%z|%%Z")"#] {
        eval(i, &format!("(setq-default frame-title-format '{form})"));
        assert_eq!(
            render_frame_grid_with_input_cached(i, frame, 48, 12, None).title,
            format!("{z}|{with_eol}|%z|%Z")
        );
        assert_eq!(i.current_buffer, current);
        assert!(std::rc::Rc::ptr_eq(
            i.selected_frame.as_ref().unwrap(),
            &selected
        ));
    }
}

#[test]
fn frame_title_coding_constructs_use_frame_buffer_terminal_and_live_metadata() {
    let mut i = Interp::new();
    eval(
        &mut i,
        r#"(progn
        (set-frame-parameter nil 'window-system 'ns)
        (set-frame-parameter nil 'menu-bar-lines 0)
        (set-frame-parameter nil 'tool-bar-lines 0)
        (set-frame-parameter nil 'tab-bar-lines 0)
        (set-frame-size nil 48 12)
        (setq-default mode-line-format nil header-line-format nil tab-line-format nil)
        (switch-to-buffer (get-buffer-create "coding-primary"))
        (set-keyboard-coding-system 'utf-8-unix)
        (set-terminal-coding-system 'utf-8-unix))"#,
    );
    let primary = i.selected_frame.as_ref().unwrap().clone();
    let Value::Frame(other) = eval(
        &mut i,
        r#"(setq qa-title-other
        (make-frame '((window-system . ns) (width . 48) (height . 12))))"#,
    ) else {
        panic!("other graphical frame")
    };
    let Value::Frame(tty) = eval(
        &mut i,
        r#"(setq qa-title-tty
        (make-frame '((window-system . nil) (width . 48) (height . 12))))"#,
    ) else {
        panic!("terminal frame")
    };
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer (get-buffer-create "coding-other")
          (setq buffer-file-coding-system 'utf-8-mac))
        (set-window-buffer (frame-selected-window qa-title-other) "coding-other")
        (set-window-buffer (frame-selected-window qa-title-tty) "coding-primary")
        (set-buffer (get-buffer-create "coding-unrelated")))"#,
    );
    // GNU 31.1 interactive format-mode-line: GUI omits the two TTY
    // keyboard/terminal mnemonics. Every mnemonic is blank in unibyte buffers.
    for multibyte in [true, false] {
        eval(
            &mut i,
            &format!(
                "(with-current-buffer \"coding-primary\" (set-buffer-multibyte {}))",
                if multibyte { "t" } else { "nil" }
            ),
        );
        for (coding, mnemonic, eol) in [
            ("nil", "-", ":"),
            ("utf-8", "U", ":"),
            ("utf-8-unix", "U", ":"),
            ("utf-8-dos", "U", "(DOS)"),
            ("utf-8-mac", "U", "(Mac)"),
            ("latin-1", "1", ":"),
            ("raw-text", "t", ":"),
            ("no-conversion", "=", ":"),
            ("binary", "=", ":"),
            ("undecided", "-", ":"),
            ("utf-16le", "U", ":"),
            ("qa-title-unknown", "-", ":"),
        ] {
            eval(
                &mut i,
                &format!(
                    "(with-current-buffer \"coding-primary\" (setq buffer-file-coding-system '{coding}))"
                ),
            );
            let z = if multibyte { mnemonic } else { " " };
            let with_eol = format!("{z}{eol}");
            check(&mut i, &primary, z, &with_eol);
            check(&mut i, &other, "U", "U(Mac)");
            let prefix = if multibyte { "UU" } else { "  " };
            check(
                &mut i,
                &tty,
                &format!("{prefix}{z}"),
                &format!("{prefix}{with_eol}"),
            );
        }
    }
    eval(
        &mut i,
        r#"(progn
        (define-coding-system 'qa-title-custom "Title coding test"
          :coding-type 'utf-8 :mnemonic ?λ :eol-type 'dos)
        (define-coding-system-alias 'qa-title-alias 'qa-title-custom)
        (with-current-buffer "coding-primary"
          (set-buffer-multibyte t)
          (setq buffer-file-coding-system 'qa-title-alias)))"#,
    );
    check(&mut i, &primary, "λ", "λ(DOS)");
    eval(
        &mut i,
        "(coding-system-put 'qa-title-alias :mnemonic \"XYZ\")",
    );
    check(&mut i, &primary, "X", "X(DOS)");
    eval(&mut i, "(coding-system-put 'qa-title-alias :mnemonic ?λ)");
    eval(
        &mut i,
        "(coding-system-put 'qa-title-custom :eol-type 'mac)",
    );
    check(&mut i, &primary, "λ", "λ(DOS)");
    let eol = i.intern("eol-mnemonic-dos");
    for (value, expected) in [
        (Value::Int('界' as i128), "λ界"),
        (Value::string("改行"), "λ改行"),
        (Value::Nil, "λ(*invalid*)"),
    ] {
        let mark = i.specbind_depth();
        i.specbind(eol, value).unwrap();
        check(&mut i, &primary, "λ", expected);
        i.unbind_to(mark).unwrap();
    }
    eval(
        &mut i,
        r#"(progn
        (with-current-buffer "coding-primary" (setq buffer-file-coding-system nil))
        (setq eol-mnemonic-undecided "AUTO" eol-mnemonic-unix "LF"))"#,
    );
    check(&mut i, &primary, "-", "-AUTO");
    eval(
        &mut i,
        r#"(with-current-buffer "coding-primary" (setq buffer-file-coding-system 'utf-8-unix))"#,
    );
    check(&mut i, &primary, "U", "ULF");
    eval(&mut i, "(coding-system-put 'cp65001 :mnemonic ?X)");
    check(&mut i, &primary, "X", "XLF");
    check(&mut i, &other, "X", "X(Mac)");
    eval(&mut i, r#"(setq-default frame-title-format '(-2 "%Z"))"#);
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 48, 12, None).title,
        "XL"
    );
    eval(&mut i, r#"(setq-default frame-title-format '(8 "%Z"))"#);
    assert_eq!(
        render_frame_grid_with_input_cached(&i, &primary, 48, 12, None).title,
        "XLF     "
    );
}
