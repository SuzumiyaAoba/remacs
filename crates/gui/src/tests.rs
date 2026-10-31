use super::keys::unicode_key_text;
use super::{
    ChanIo, EditorView, GuiEvent, build_editor_view, command_keystroke, keystroke_to_code,
    start_logic,
};
use gpui::{EntityInputHandler, Keystroke, Modifiers, point, px};
use remacs_runtime::editor::{CHAR_CTL, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use remacs_runtime::frontend::{
    Grid, InputEvent, KeyIo, MouseButton, MouseInput, MousePosition, render_grid_with_input_cached,
};
use remacs_runtime::lisp::Interp;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

fn ks(
    key: &str,
    key_char: Option<&str>,
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,
) -> Keystroke {
    Keystroke {
        modifiers: Modifiers {
            control,
            alt,
            shift,
            platform,
            function: false,
        },
        key: key.into(),
        key_char: key_char.map(|s| s.into()),
    }
}

#[test]
fn multichar_os_text_does_not_enter_the_command_key_path() {
    assert!(!command_keystroke(&ks(
        "日本語",
        Some("日本語"),
        false,
        false,
        false,
        false
    )));
    assert!(!command_keystroke(&ks(
        "",
        Some("A日B"),
        false,
        false,
        false,
        false
    )));
    assert!(command_keystroke(&ks(
        "return", None, false, false, false, false
    )));
    assert!(command_keystroke(&ks(
        "x",
        Some("x"),
        true,
        false,
        false,
        false
    )));
}

#[test]
fn non_ascii_key_text_has_a_fallback_when_the_input_client_is_skipped() {
    assert_eq!(
        unicode_key_text(&ks("日", Some("日"), false, false, false, false)),
        Some("日")
    );
    assert_eq!(
        unicode_key_text(&ks("😀", Some("😀"), false, false, false, false)),
        Some("😀")
    );
    assert_eq!(
        unicode_key_text(&ks("a", Some("a"), false, false, false, false)),
        None
    );
    assert_eq!(
        unicode_key_text(&ks("", Some("A日B"), false, false, false, false)),
        None
    );
}

#[test]
fn keystroke_plain_chars() {
    assert_eq!(
        keystroke_to_code(&ks("a", Some("a"), false, false, false, false)),
        Some(97)
    );
    assert_eq!(
        keystroke_to_code(&ks("z", None, false, false, false, false)),
        Some(122)
    );
    // Digits and punctuation pass through.
    assert_eq!(
        keystroke_to_code(&ks("5", Some("5"), false, false, false, false)),
        Some(53)
    );
}

#[test]
fn keystroke_shift_folds_into_char() {
    // S-a types "A": shift bit dropped, char is uppercase.
    let code = keystroke_to_code(&ks("a", Some("A"), false, false, true, false)).unwrap();
    assert_eq!(code, 65);
    assert_eq!(code & CHAR_SHIFT, 0);
    // A shifted symbol keeps its physical key as the event base (S-1).
    let code = keystroke_to_code(&ks("1", Some("!"), false, false, true, false)).unwrap();
    assert_eq!(code, '1' as i128 | CHAR_SHIFT);
}

#[test]
fn shift_only_symbol_keys_use_the_command_path() {
    let shifted_space = ks("space", Some(" "), false, false, true, false);
    assert!(command_keystroke(&shifted_space));
    assert_eq!(
        keystroke_to_code(&shifted_space),
        Some(' ' as i128 | CHAR_SHIFT)
    );

    let shifted_digit = ks("1", Some("!"), false, false, true, false);
    assert!(command_keystroke(&shifted_digit));
    assert_eq!(
        keystroke_to_code(&shifted_digit),
        Some('1' as i128 | CHAR_SHIFT)
    );

    // A shifted letter folds to its uppercase character and can stay on the
    // OS text-input path.
    let shifted_letter = ks("a", Some("A"), false, false, true, false);
    assert!(!command_keystroke(&shifted_letter));
    assert_eq!(keystroke_to_code(&shifted_letter), Some('A' as i128));
}

#[test]
fn keystroke_ctrl_folds_to_control_char() {
    // C-x → 24 like a tty.
    assert_eq!(
        keystroke_to_code(&ks("x", Some("x"), true, false, false, false)),
        Some(24)
    );
    // C-u → 21, C-g → 7.
    assert_eq!(
        keystroke_to_code(&ks("u", None, true, false, false, false)),
        Some(21)
    );
    assert_eq!(
        keystroke_to_code(&ks("g", None, true, false, false, false)),
        Some(7)
    );
    // C-? folds to DEL.
    assert_eq!(
        keystroke_to_code(&ks("?", None, true, false, false, false)),
        Some(127)
    );
}

#[test]
fn keystroke_meta_and_super() {
    // M-f.
    let f = keystroke_to_code(&ks("f", None, false, true, false, false)).unwrap();
    assert_eq!(f, 'f' as i128 | CHAR_META);
    // Cmd-s on macOS → Super.
    let s = keystroke_to_code(&ks("s", None, false, false, false, true)).unwrap();
    assert_eq!(s, 's' as i128 | CHAR_SUPER);
    // C-M-x: ctrl folded, meta kept.
    let x = keystroke_to_code(&ks("x", None, true, true, false, false)).unwrap();
    assert_eq!(x, 24 | CHAR_META);
}

#[test]
fn keystroke_named_keys() {
    use remacs_runtime::editor::event_code_for;
    assert_eq!(
        keystroke_to_code(&ks("return", None, false, false, false, false)),
        Some(event_code_for("return"))
    );
    assert_eq!(
        keystroke_to_code(&ks("tab", None, false, false, false, false)),
        Some(event_code_for("tab"))
    );
    assert_eq!(
        keystroke_to_code(&ks("backspace", None, false, false, false, false)),
        Some(127)
    );
    assert_eq!(
        keystroke_to_code(&ks("escape", None, false, false, false, false)),
        Some(event_code_for("escape"))
    );
    assert_eq!(
        keystroke_to_code(&ks("space", Some(" "), false, false, false, false)),
        Some(32)
    );
    assert_eq!(
        keystroke_to_code(&ks("up", None, false, false, false, false)),
        Some(event_code_for("up"))
    );
    assert_eq!(
        keystroke_to_code(&ks("pageup", None, false, false, false, false)),
        Some(event_code_for("prior"))
    );
    assert_eq!(
        keystroke_to_code(&ks("pagedown", None, false, false, false, false)),
        Some(event_code_for("next"))
    );
    assert_eq!(
        keystroke_to_code(&ks("f5", None, false, false, false, false)),
        Some(event_code_for("f5"))
    );
    assert_eq!(
        keystroke_to_code(&ks("delete", None, false, false, false, false)),
        Some(event_code_for("delete"))
    );
}

#[test]
fn keystroke_modified_named_and_unknown() {
    use remacs_runtime::editor::event_code_for;
    // C-<right> keeps the ctl bit on the symbol event.
    let c = keystroke_to_code(&ks("right", None, true, false, false, false)).unwrap();
    assert_eq!(c, event_code_for("right") | CHAR_CTL);
    // Unknown named keys produce no event.
    assert_eq!(
        keystroke_to_code(&ks("capslock", None, false, false, false, false)),
        None
    );
}

#[test]
fn help_keysym_reaches_the_named_help_binding() {
    let help = ks("help", None, false, false, false, false);
    assert!(command_keystroke(&help));
    assert_eq!(
        keystroke_to_code(&help),
        Some(remacs_runtime::editor::event_code_for("help"))
    );
}

#[test]
fn break_keysym_reaches_the_named_break_binding() {
    let key = ks("break", None, false, false, false, false);
    assert!(command_keystroke(&key));
    assert_eq!(
        keystroke_to_code(&key),
        Some(remacs_runtime::editor::event_code_for("break"))
    );
}

#[test]
fn begin_keysym_reaches_the_named_begin_binding() {
    let key = ks("begin", None, false, false, false, false);
    assert!(command_keystroke(&key));
    assert_eq!(
        keystroke_to_code(&key),
        Some(remacs_runtime::editor::event_code_for("begin"))
    );
}

#[test]
fn modified_named_keys_follow_gui_command_bindings() {
    let mut i = Interp::new();
    i.eval_str("(progn (insert \"alpha\\nbeta\") (goto-char (point-max)))")
        .unwrap();
    let mut keys = Vec::new();
    let mut arg_mode = false;
    let home = keystroke_to_code(&ks("home", None, true, false, false, false)).unwrap();
    remacs_runtime::frontend::dispatch_key_core(&mut i, home, &mut keys, &mut arg_mode, None)
        .unwrap();
    let point = i.eval_str("(point)").unwrap();
    assert_eq!(i.prin1_to_string(&point), "1");
    i.eval_str("(progn (defun qa297-gui-command () (interactive) (insert \"GUI\")) (global-set-key [C-M-S-s-f5] (quote qa297-gui-command)))")
            .unwrap();
    let command = keystroke_to_code(&ks("f5", None, true, true, true, true)).unwrap();
    remacs_runtime::frontend::dispatch_key_core(&mut i, command, &mut keys, &mut arg_mode, None)
        .unwrap();
    let text = i.eval_str("(buffer-string)").unwrap();
    assert_eq!(i.princ_to_string(&text), "GUIalpha\nbeta");
    assert!(keys.is_empty());
}

fn chan_pair() -> (ChanIo, Sender<GuiEvent>, smol::channel::Receiver<Grid>) {
    let (etx, erx) = mpsc::channel();
    let (gtx, grx) = smol::channel::bounded(1);
    (
        ChanIo {
            events: erx,
            quit_request: None,
            grids: gtx,
            legacy_grid_output: true,
            gui_requests: None,
            pending: Vec::new(),
            pending_open_files: std::collections::VecDeque::new(),
            cols: 80,
            rows: 24,
            last: None,
            render_requested: false,
            geometry: None,
            frame_sizes: std::collections::HashMap::new(),
            frame_geometries: std::collections::HashMap::new(),
            frame_grids: std::collections::HashMap::new(),
            latest_snapshots: Vec::new(),
            frame_snapshot_mailbox: std::sync::Arc::new(
                super::logic::FrameSnapshotMailbox::default(),
            ),
            active_frame_id: None,
            primary_frame_id: None,
            pending_frame_selection: None,
            pending_frame_visibility: None,
            pending_frame_focus: None,
            pending_frame_delete: None,
            pending_power_off: false,
            geometry_symbols: None,
            mouse_down: None,
            scroll_remainders: Default::default(),
        },
        etx,
        grx,
    )
}

#[test]
fn chanio_key_events_and_unread() {
    let (mut io, tx, _grx) = chan_pair();
    tx.send(GuiEvent::Key(24)).unwrap();
    assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(24));
    // Timeout → None.
    assert_eq!(io.poll_key(Duration::from_millis(5)).unwrap(), None);
    // unread returns before queued events.
    tx.send(GuiEvent::Key(97)).unwrap();
    io.unread(7);
    assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(7));
    assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), Some(97));
}

#[test]
fn chanio_resize_updates_size() {
    let (mut io, tx, _grx) = chan_pair();
    assert_eq!(io.size(), (80, 24));
    tx.send(GuiEvent::Resize(120, 40)).unwrap();
    assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), None);
    assert_eq!(io.size(), (120, 40));
    // Clamped to the minimum.
    tx.send(GuiEvent::Resize(1, 1)).unwrap();
    assert_eq!(io.poll_key(Duration::from_millis(10)).unwrap(), None);
    assert_eq!(io.size(), (10, 3));
}

#[test]
fn chanio_disconnect_errors() {
    let (mut io, tx, _grx) = chan_pair();
    drop(tx);
    assert!(io.poll_key(Duration::from_millis(10)).is_err());
}

#[test]
fn chanio_mouse_click_keeps_down_position_and_modifiers() {
    let (mut io, tx, _grx) = chan_pair();
    let start = MousePosition {
        buffer_id: 7,
        buffer_position: 4,
        utf16_offset: 4,
        x: 31,
        y: 12,
        row: 0,
        column: 4,
        window_id: None,
        area: None,
        mode_line_buffer_name: false,
    };
    let end = MousePosition {
        buffer_position: 9,
        utf16_offset: 9,
        x: 58,
        row: 0,
        column: 9,
        ..start
    };
    tx.send(GuiEvent::MouseDown {
        button: MouseButton::Left,
        click_count: 1,
        modifiers: CHAR_SHIFT,
        position: start,
    })
    .unwrap();
    assert!(matches!(
        io.poll_input(Duration::from_millis(10)).unwrap(),
        Some(InputEvent::MouseDown(MouseInput {
            button: MouseButton::Left,
            modifiers: CHAR_SHIFT,
            start: down,
            end: same,
            ..
        })) if down == start && same == start
    ));

    tx.send(GuiEvent::MouseUp {
        button: MouseButton::Left,
        click_count: 1,
        modifiers: 0,
        position: end,
    })
    .unwrap();
    assert!(matches!(
        io.poll_input(Duration::from_millis(10)).unwrap(),
        Some(InputEvent::Mouse(MouseInput {
            button: MouseButton::Left,
            modifiers,
            start: down,
            end: up,
            ..
        })) if modifiers == CHAR_SHIFT && down == start && up == end
    ));
}

#[test]
fn chanio_render_and_draw_echo() {
    let (mut io, _tx, grx) = chan_pair();
    let mut i = Interp::new();
    i.eval_str(r#"(insert "hello")"#).unwrap();
    io.render(&i).unwrap();
    let grid = grx.try_recv().unwrap();
    assert_eq!(grid.rows.len(), 24);
    assert!(grid.rows[0].starts_with("hello"));
    assert!(
        grid.mode_rows
            .iter()
            .any(|&r| grid.rows[r].contains("scratch"))
    );
    // draw_echo patches only the last row and moves the cursor.
    io.draw_echo("M-x find-").unwrap();
    let grid = grx.try_recv().unwrap();
    assert!(grid.rows.last().unwrap().starts_with("M-x find-"));
    assert_eq!(grid.cursor, Some((9, 23)));
    assert!(grid.rows[0].starts_with("hello"));
}

#[test]
fn chanio_send_grid_latest_wins_and_closed() {
    let (mut io, _tx, grx) = chan_pair();
    // Bounded(1) + force_send: newest displaces the queued frame.
    {
        let mut g = Grid::blank(0, 0);
        g.rows = vec!["old".into()];
        io.send_grid(g)
    }
    .unwrap();
    {
        let mut g = Grid::blank(0, 0);
        g.rows = vec!["new".into()];
        io.send_grid(g)
    }
    .unwrap();
    assert_eq!(grx.try_recv().unwrap().rows[0], "new");
    drop(grx);
    assert!(io.send_grid(Grid::blank(0, 0)).is_err());
}

#[test]
fn logic_thread_renders_and_quits() {
    let (tx, rx) = start_logic(Vec::new()).unwrap();
    // The loop paints a frame as soon as it starts.
    let grid = rx.recv_blocking().unwrap();
    assert_eq!(grid.rows.len(), 24);
    assert!(grid.rows.iter().any(|r| r.contains("scratch")));
    // C-x C-c → save-buffers-kill-emacs → loop exits, sender drops.
    tx.send(GuiEvent::Key(24)).unwrap();
    tx.send(GuiEvent::Key(3)).unwrap();
    for _ in 0..50 {
        if rx.recv_blocking().is_err() {
            return;
        }
    }
    panic!("logic thread did not exit after C-x C-c");
}

#[test]
fn logic_thread_applies_directory_options_before_visiting_files() {
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Owned(std::env::temp_dir().join(format!(
                    "remacs-gui-load-path-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )));
    for (name, value) in [("A", "loaded-A"), ("B", "loaded-B")] {
        std::fs::create_dir_all(root.0.join(name)).unwrap();
        std::fs::write(
            root.0.join(name).join("qa229-gui.el"),
            format!("(defun qa229-gui-value () \"{value}\")\n(provide 'qa229-gui)\n"),
        )
        .unwrap();
    }
    let file = root.0.join("payload.txt");
    std::fs::write(&file, "GUI").unwrap();
    let file = file.to_string_lossy().into_owned();
    let (tx, rx) = super::start_logic_with_dump_for_test(
        vec![file.clone()],
        None,
        vec![
            "-directory".into(),
            root.0.join("A").to_string_lossy().into_owned(),
            format!("--directory={}", root.0.join("B").display()),
            file,
        ],
    )
    .unwrap();
    let wait_for = |text: &str| {
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            if let Ok(grid) = rx.try_recv() {
                if grid.rows.iter().any(|row| row.contains(text)) {
                    break;
                }
            }
            assert!(!rx.is_closed(), "logic thread closed before {text}");
            assert!(
                std::time::Instant::now() < deadline,
                "no GUI grid contained {text}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    wait_for("GUI");
    tx.send(GuiEvent::Key(':' as i128 | CHAR_META)).unwrap();
    for ch in "(progn (require 'qa229-gui) (insert (qa229-gui-value)) (set-buffer-modified-p nil))"
        .chars()
    {
        tx.send(GuiEvent::Key(ch as i128)).unwrap();
    }
    tx.send(GuiEvent::Key(13)).unwrap();
    wait_for("loaded-AGUI");
    tx.send(GuiEvent::Key(24)).unwrap();
    tx.send(GuiEvent::Key(3)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !rx.is_closed() {
        assert!(
            std::time::Instant::now() < deadline,
            "GUI logic thread did not quit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn startup_actions_run_on_gui_logic_thread() {
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Owned(std::env::temp_dir().join(format!(
            "remacs-gui-startup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    std::fs::create_dir(&root.0).unwrap();
    for kind in ["eval", "load", "funcall"] {
        let file = root.0.join(format!("{kind}.txt"));
        std::fs::write(&file, "").unwrap();
        let marker = format!("GUI-{kind}");
        let form = format!(
            "(progn (insert (if noninteractive \"BAD-BATCH\" \"{marker}\")) (set-buffer-modified-p nil))"
        );
        let mut arguments = vec![file.to_string_lossy().into_owned()];
        match kind {
            "eval" => arguments.extend(["--eval".into(), form]),
            "load" => {
                let script = root.0.join("startup.el");
                std::fs::write(
                    &script,
                    format!(";;; -*- lexical-binding: t; -*-\n{form}\n"),
                )
                .unwrap();
                arguments.extend(["-l".into(), script.to_string_lossy().into_owned()]);
            }
            _ => {
                let script = root.0.join("command.el");
                std::fs::write(
                    &script,
                    format!(
                        ";;; -*- lexical-binding: t; -*-\n(defun qa341-startup-command () {form})\n"
                    ),
                )
                .unwrap();
                arguments.extend([
                    "-l".into(),
                    script.to_string_lossy().into_owned(),
                    "-f".into(),
                    "qa341-startup-command".into(),
                ]);
            }
        }
        let (tx, rx) = super::start_logic_with_dump_for_test(Vec::new(), None, arguments).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if let Ok(grid) = rx.try_recv() {
                if grid.rows.iter().any(|row| row.contains(&marker)) {
                    break;
                }
            }
            assert!(!rx.is_closed(), "GUI startup closed before {marker}");
            assert!(
                std::time::Instant::now() < deadline,
                "no first frame with {marker}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        tx.send(super::GuiEvent::Key(24)).unwrap();
        tx.send(super::GuiEvent::Key(3)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !rx.is_closed() {
            assert!(
                std::time::Instant::now() < deadline,
                "GUI did not quit after {kind}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[test]
fn missing_dump_fails_before_ui_startup() {
    let path = std::env::temp_dir().join(format!(
        "remacs-missing-image-{}-{}.pdmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(
        super::start_logic_with_dump(
            Vec::new(),
            Some(path.to_string_lossy().into_owned()),
            Vec::new()
        )
        .is_err()
    );
}

/// A scripted editing session over `ChanIo` — same key codes as the
/// terminal pty test, covering `dispatch_key`, `isearch_loop` and
/// `minibuf_loop` for the channel IO flavor.
#[test]
fn logic_thread_drives_editing_session() {
    let (tx, rx) = start_logic(Vec::new()).unwrap();
    rx.recv_blocking().unwrap(); // initial frame
    let meta = CHAR_META;
    let keys: Vec<i128> = vec![
        'h' as i128,
        'i' as i128, // self-insert "hi"
        21,          // C-u
        '3' as i128,
        'z' as i128,        // C-u 3 z
        19,                 // C-s → isearch
        'i' as i128,        // search "i"
        127,                // DEL pops the search char
        6,                  // C-f inside isearch
        13,                 // RET exits isearch
        'x' as i128 | meta, // M-x → minibuffer
        'd' as i128,
        'e' as i128,
        's' as i128,
        'c' as i128,
        9,  // TAB completes
        13, // RET runs it
        7,  // C-g safety
        11, // C-k
        25, // C-y
        31, // C-_ undo
        24,
        3, // C-x C-c
    ];
    for k in keys {
        if tx.send(GuiEvent::Key(k)).is_err() {
            break;
        }
    }
    for _ in 0..400 {
        if rx.recv_blocking().is_err() {
            return;
        }
    }
    panic!("logic thread did not exit after scripted session");
}

#[test]
fn chanio_committed_text_preserves_unicode_and_unread_order() {
    let (mut io, tx, _grx) = chan_pair();
    tx.send(GuiEvent::Text("A日B😀e\u{301}".into())).unwrap();
    assert_eq!(io.poll_key(Duration::ZERO).unwrap(), Some('A' as i128));
    io.unread(7);
    assert_eq!(io.poll_key(Duration::ZERO).unwrap(), Some(7));
    for ch in "日B😀e\u{301}".chars() {
        assert_eq!(io.poll_key(Duration::ZERO).unwrap(), Some(ch as i128));
    }
    tx.send(GuiEvent::Text(String::new())).unwrap();
    assert_eq!(io.poll_key(Duration::ZERO).unwrap(), None);
}

#[test]
fn registered_input_delivers_text_once_and_keeps_commands_separate() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|window, cx| {
        let view = EditorView::new(tx, cx);
        view.focus.focus(window);
        view
    });
    app.simulate_input(handle.into(), "Aa! 日本語😀");
    let text: String = rx
        .try_iter()
        .filter_map(|event| match event {
            GuiEvent::ForFrame { .. } => None,
            GuiEvent::FrameVisibility { .. } => None,
            GuiEvent::FrameFocus { .. } => None,
            GuiEvent::ColorDrop { .. } => None,
            GuiEvent::MouseDragMove { .. } => None,
            #[cfg(target_os = "macos")]
            GuiEvent::NativeService { .. } => None,
            GuiEvent::FrameDeleteRequest { .. } | GuiEvent::PowerOff => None,
            GuiEvent::Text(text) => Some(text),
            GuiEvent::CommandArgs { .. } => None,
            GuiEvent::Drop { .. } => None,
            GuiEvent::Resize(..) => None,
            GuiEvent::Geometry(_) => None,
            GuiEvent::Scroll { .. } => None,
            GuiEvent::ScrollBar { .. } => None,
            GuiEvent::MouseDown { .. } | GuiEvent::MouseUp { .. } => None,
            GuiEvent::Key(code) => panic!("plain text also produced key {code}"),
            GuiEvent::ReplaceText { range, text } => {
                panic!("plain text also produced ranged replacement {range:?}: {text:?}")
            }
        })
        .collect();
    assert_eq!(text, "Aa! 日本語😀");
    app.simulate_keystrokes(handle.into(), "ctrl-x ctrl-s alt-f cmd-x return backspace");
    let codes: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            GuiEvent::ForFrame { .. } => None,
            GuiEvent::FrameVisibility { .. } => None,
            GuiEvent::FrameFocus { .. } => None,
            GuiEvent::ColorDrop { .. } => None,
            GuiEvent::MouseDragMove { .. } => None,
            #[cfg(target_os = "macos")]
            GuiEvent::NativeService { .. } => None,
            GuiEvent::FrameDeleteRequest { .. } | GuiEvent::PowerOff => None,
            GuiEvent::Key(code) => Some(code),
            GuiEvent::CommandArgs { .. } => None,
            GuiEvent::Drop { .. } => None,
            GuiEvent::Resize(..) => None,
            GuiEvent::Geometry(_) => None,
            GuiEvent::Scroll { .. } => None,
            GuiEvent::ScrollBar { .. } => None,
            GuiEvent::MouseDown { .. } | GuiEvent::MouseUp { .. } => None,
            GuiEvent::Text(text) => panic!("command also inserted {text:?}"),
            GuiEvent::ReplaceText { range, text } => {
                panic!("command also replaced {range:?} with {text:?}")
            }
        })
        .collect();
    assert_eq!(
        codes,
        vec![
            24,
            19,
            'f' as i128 | CHAR_META,
            'x' as i128 | CHAR_SUPER,
            13,
            127
        ]
    );
}

#[test]
fn input_character_index_maps_visible_text_and_rejects_outside_points() {
    let mut interp = Interp::new();
    let buffer = interp.create_buffer("input-hit-test");
    interp.set_current_buffer(buffer);
    interp
        .buffers
        .get(buffer)
        .unwrap()
        .borrow_mut()
        .insert("A日B\n");
    let grid = render_grid_with_input_cached(&interp, 80, 24, None);
    assert!(grid.input.is_some());

    let mut app = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel();
    let handle = app.add_window(|window, cx| {
        let mut view = EditorView::new(tx, cx);
        view.grid = grid;
        view
    });
    let (inside, following, outside) = handle
        .update(&mut app, |view, window, cx| {
            let inside = view.character_index_for_point(point(px(4.0), px(10.0)), window, cx);
            let following = view.character_index_for_point(point(px(26.0), px(10.0)), window, cx);
            let outside = view.character_index_for_point(point(px(200.0), px(10.0)), window, cx);
            (inside, following, outside)
        })
        .unwrap();
    assert_eq!(inside, Some(0));
    assert_eq!(following, Some(2));
    assert_eq!(outside, None);
}

#[test]
fn input_handler_commits_multichar_text_and_composition_only_once() {
    let mut app = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel();
    let handle = app.add_window(|window, cx| {
        let mut view = EditorView::new(tx, cx);
        view.focus.focus(window);
        view.grid = Grid::blank(80, 24);
        view.grid.cursor = Some((2, 3));
        view
    });
    handle
        .update(&mut app, |view, window, cx| {
            view.replace_and_mark_text_in_range(None, "にほん", Some(3..3), window, cx);
            assert_eq!(view.marked_text_range(window, cx), Some(0..3));
            view.replace_and_mark_text_in_range(None, "日本", Some(2..2), window, cx);
        })
        .unwrap();
    app.run_until_parked();
    assert!(
        rx.try_iter()
            .all(|event| matches!(event, GuiEvent::Resize(..) | GuiEvent::Geometry(_)))
    );
    handle
        .update(&mut app, |view, window, cx| {
            view.replace_text_in_range(None, "日本語A日B😀e\u{301}", window, cx);
            view.unmark_text(window, cx);
            assert_eq!(view.marked_text_range(window, cx), None);
            assert_eq!(
                view.selected_text_range(false, window, cx).unwrap().range,
                0..0
            );
        })
        .unwrap();
    let text: Vec<_> = rx
        .try_iter()
        .filter_map(|event| match event {
            GuiEvent::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(text, vec!["日本語A日B😀e\u{301}"]);
}

#[test]
fn registered_text_input_reaches_logic_and_saved_utf8_file() {
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Owned(std::env::temp_dir().join(format!(
                "remacs-gui-unicode-{}-{}.txt",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
    std::fs::write(&file.0, "").unwrap();
    let (tx, grids) = start_logic(vec![file.0.to_string_lossy().into_owned()]).unwrap();
    let mut app = gpui::TestAppContext::single();
    let handle = app.add_window(|window, cx| build_editor_view(window, cx, tx, grids));
    // Exercise the registered input client, rather than calling the view's
    // handler directly. This is a headless GPUI test; native macOS IME
    // event delivery still needs a separate real-window check.
    app.simulate_input(handle.into(), "日本語A日B😀e\u{301}");
    app.simulate_keystrokes(handle.into(), "ctrl-x ctrl-s ctrl-x ctrl-c");
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        app.run_until_parked();
        let text = std::fs::read_to_string(&file.0).unwrap();
        if text.trim_end_matches('\n') == "日本語A日B😀e\u{301}" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "saved GUI text was {text:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn headless_view_renders_and_reports_size() {
    let mut app_cx = gpui::TestAppContext::single();
    let (tx, rx) = mpsc::channel::<GuiEvent>();
    let (view, cx) = app_cx.add_window_view(|_window, cx| EditorView::new(tx, cx));
    // The initial render reported the test display's grid size.
    match rx.try_recv().unwrap() {
        GuiEvent::Resize(c, r) => {
            assert!(c >= 10 && r >= 3);
        }
        _ => panic!("expected a resize report"),
    }
    // Push a grid and repaint: exercises the canvas paint path.
    let mut grid = Grid::blank(10, 3);
    grid.rows = vec![
        "line one".to_string(),
        "line two".to_string(),
        "--mode".to_string(),
    ];
    grid.mode_rows = vec![2];
    grid.cursor = Some((3, 1));
    view.update(cx, |v, cx| {
        v.grid = grid;
        cx.notify();
    });
    cx.run_until_parked();
    let cols = view.update(cx, |v, _cx| v.cols);
    assert!(cols >= 10);
}

#[test]
fn build_editor_view_pumps_grids() {
    // Same construction path as run_gui's open_window closure.
    let mut app_cx = gpui::TestAppContext::single();
    let (tx, _rx) = mpsc::channel::<GuiEvent>();
    let (gtx, grx) = smol::channel::bounded::<Grid>(1);
    let handle = app_cx.add_window(|window, cx| build_editor_view(window, cx, tx, grx));
    // A grid pushed through the channel is applied by the pump.
    let mut g = Grid::blank(10, 1);
    g.rows = vec!["pumped".to_string()];
    g.cursor = Some((1, 0));
    gtx.force_send(g).unwrap();
    app_cx.run_until_parked();
    let text = handle
        .update(&mut app_cx, |v, _w, _cx| v.grid.rows[0].clone())
        .unwrap();
    assert_eq!(text, "pumped");
    // Closing the channel ends the pump (quits in production).
    drop(gtx);
    app_cx.run_until_parked();
}
