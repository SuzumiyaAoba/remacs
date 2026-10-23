//! End-to-end editor-loop session: scripted key events driven through
//! `run_editor_with` exercise the real dispatch path — find-file
//! (C-x C-f) with the filename completion minibuffer (default-directory
//! initial input, RET via `minibuffer-completion-exit', TAB via
//! `minibuffer-complete'), M-x (`read-extended-command' must-match),
//! C-x b buffer switching, C-x C-s saving, and C-x C-c quitting.
//!
//! Regression coverage for: literal "\\u{1}nil" interactive-spec
//! strings (which broke every `(interactive)' subr, including
//! `exit-minibuffer'), `read-file-name' bypassing
//! `read-file-name-function' (no initial directory, no filename
//! keymap), and `minibuf_loop' swallowing poll errors into an
//! infinite redraw spin.
use remacs::lisp::Interp;
use remacs::term::{Grid, KeyIo, render_grid};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;
use std::time::Duration;

/// A `KeyIo` feeding a fixed script of Emacs event codes; an exhausted
/// script errors out so a wedged loop fails fast instead of hanging.
/// Echo texts are shared with the caller for post-run assertions.
struct ScriptIo {
    keys: VecDeque<i128>,
    echoes: Rc<RefCell<Vec<String>>>,
    grids: Rc<RefCell<Vec<Grid>>>,
}

impl KeyIo for ScriptIo {
    fn poll_key(&mut self, _d: Duration) -> io::Result<Option<i128>> {
        match self.keys.pop_front() {
            Some(k) => Ok(Some(k)),
            None => Err(io::Error::new(io::ErrorKind::TimedOut, "script done")),
        }
    }
    fn unread(&mut self, k: i128) {
        self.keys.push_front(k);
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let (w, h) = self.size();
        self.grids.borrow_mut().push(render_grid(i, w, h));
        Ok(())
    }
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        self.echoes.borrow_mut().push(text.to_string());
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (80, 24)
    }
}

const C_X: i128 = 24;
const RET: i128 = b'\r' as i128;
const TAB: i128 = b'\t' as i128;
const META: i128 = 0x0800_0000;

fn keys(script: &mut Vec<i128>, s: &str) {
    script.extend(s.chars().map(|c| c as i128));
}

fn run(script: Vec<i128>, i: &mut Interp) -> Rc<RefCell<Vec<String>>> {
    run_capture(script, i).0
}

/// Run the editor loop; returns (echo texts, rendered grids).
fn run_capture(
    script: Vec<i128>,
    i: &mut Interp,
) -> (Rc<RefCell<Vec<String>>>, Rc<RefCell<Vec<Grid>>>) {
    let echoes = Rc::new(RefCell::new(Vec::new()));
    let grids = Rc::new(RefCell::new(Vec::new()));
    let io = ScriptIo {
        keys: script.into(),
        echoes: echoes.clone(),
        grids: grids.clone(),
    };
    remacs::term::run_editor_with(io, i).expect("editor loop");
    (echoes, grids)
}

/// One scripted session covering the everyday command path end to end:
///   C-x C-f /tmp/… RET  (filename read: default dir + completion map)
///   "hello" self-insert,  C-x C-s  (write the visited file)
///   C-x b *scratch* RET (must-match buffer completion)
///   M-x forward-word RET (extended-command completion read)
///   C-x C-c             (save-buffers-kill-terminal)

/// Run `f` on a dedicated 64 MiB-stack thread — `Interp` is not Send,
/// and libtest's 2 MiB test threads overflow on editor-loop paths.
fn run_on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("test thread panicked");
}

#[test]
fn tui_window_change_history_is_published_at_redisplay() {
    run_on_big_stack(|| {
        let mut i = Interp::new();
        i.eval_str(r#"(progn
          (setq qa289-ui-target (get-buffer-create "qa289-ui-target")
                qa289-ui-seen nil)
          (defun qa289-ui-switch ()
            (interactive)
            (setq qa289-ui-before (window-old-buffer))
            (switch-to-buffer qa289-ui-target)
            (setq qa289-ui-after-command (window-old-buffer)))
          (setq window-buffer-change-functions
                (list (lambda (frame)
                        (let ((window (frame-selected-window frame)))
                          (when (and (eq (window-buffer window) qa289-ui-target)
                                     (eq (window-old-buffer window) qa289-ui-before))
                            (setq qa289-ui-seen t)))))))"#).unwrap();
        let mut script = vec![META | b'x' as i128];
        keys(&mut script, "qa289-ui-switch");
        script.extend([RET, C_X, 3]);
        run(script, &mut i);
        let value = i.eval_str(r#"(list
          (eq qa289-ui-before (get-buffer "*scratch*"))
          (eq qa289-ui-before qa289-ui-after-command)
          qa289-ui-seen
          (eq (window-old-buffer) qa289-ui-target))"#).unwrap();
        assert_eq!(i.prin1_to_string(&value), "(t t t t)");
    });
}

#[test]
fn tui_schedules_workers_while_waiting_for_input() {
    struct IdleIo {
        script: ScriptIo,
        idle_polls: usize,
    }
    impl KeyIo for IdleIo {
        fn poll_key(&mut self, duration: Duration) -> io::Result<Option<i128>> {
            if self.idle_polls > 0 {
                self.idle_polls -= 1;
                Ok(None)
            } else {
                self.script.poll_key(duration)
            }
        }
        fn unread(&mut self, key: i128) {
            self.script.unread(key)
        }
        fn render(&mut self, i: &Interp) -> io::Result<()> {
            self.script.render(i)
        }
        fn draw_echo(&mut self, text: &str) -> io::Result<()> {
            self.script.draw_echo(text)
        }
        fn size(&self) -> (usize, usize) {
            self.script.size()
        }
    }
    run_on_big_stack(|| {
        let mut i = Interp::new();
        i.eval_str(
            "(setq qa196-tui-stage 0) \
            (make-thread (lambda () (setq qa196-tui-stage 1) \
              (thread-yield) (setq qa196-tui-stage 2)))",
        )
        .unwrap();
        let io = IdleIo {
            idle_polls: 4,
            script: ScriptIo {
                keys: vec![C_X, 3].into(),
                echoes: Rc::new(RefCell::new(Vec::new())),
                grids: Rc::new(RefCell::new(Vec::new())),
            },
        };
        remacs::term::run_editor_with(io, &mut i).unwrap();
        let value = i.eval_str("qa196-tui-stage").unwrap();
        assert!(matches!(value, remacs::lisp::Value::Int(2)));
    });
}

#[test]
fn tui_session_find_file_mx_switch_save_quit() {
    run_on_big_stack(move || {
        let file = std::env::temp_dir().join("remacs_tui_session.txt");
        let _ = std::fs::remove_file(&file);
        let mut i = Interp::new();

        let mut script = Vec::new();
        // C-x C-f <abs path> RET — visits the file.
        script.extend([C_X, 6]);
        keys(&mut script, &file.to_string_lossy());
        script.push(RET);
        // Self-insert text into the visited file's buffer, then save.
        keys(&mut script, "hello-remacs");
        script.extend([C_X, 19]); // C-x C-s
        // C-x b *scratch* RET — must-match buffer-name completion.
        script.extend([C_X, b'b' as i128]);
        keys(&mut script, "*scratch*");
        script.push(RET);
        // M-x forward-word RET — read-extended-command completion.
        script.push(META | 120);
        keys(&mut script, "forward-word");
        script.push(RET);
        // C-x C-c quits.
        script.extend([C_X, 3]);

        let echoes = run(script, &mut i);

        // Ended in *scratch* after the buffer switch; quit flag set.
        let bn = i
            .buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().name.clone());
        assert_eq!(bn.as_deref(), Some("*scratch*"));
        assert!(i.quit_editor);
        // The minibuffer showed "Find file: " with the default directory
        // pre-inserted (rfn-eshadow's {} marker or the dir text itself).
        assert!(
            echoes.borrow().iter().any(|e| e.contains("Find file: ")),
            "echoes: {:?}",
            echoes.borrow()
        );
        // The visited file was really written by C-x C-s.
        let written = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(written.contains("hello-remacs"), "got: {written:?}");
        let _ = std::fs::remove_file(&file);
    })
}

#[test]
fn tui_yank_imports_external_text_and_preserves_properties() {
    run_on_big_stack(|| {
        let mut i = Interp::new();
        i.eval_str("(setq kill-ring nil kill-ring-yank-pointer nil interprogram-cut-function nil qa129-pastes 0 interprogram-paste-function (lambda () (setq qa129-pastes (1+ qa129-pastes)) (propertize \"PASTE\" 'qa129-tag 7)))").unwrap();
        let echoes = run(vec![25, C_X, 3], &mut i); // C-y, C-x C-c
        let result = i.eval_str("(and (equal (buffer-string) \"PASTE\") (= (get-text-property 1 'qa129-tag) 7) (= qa129-pastes 1) (eq kill-ring-yank-pointer kill-ring))").unwrap();
        assert!(!result.is_nil(), "{}", i.prin1_to_string(&result));
        assert!(i.quit_editor);
        assert!(
            !echoes
                .borrow()
                .iter()
                .any(|text| text.contains("Kill ring is empty"))
        );
    });
}

/// TAB in the find-file minibuffer runs `minibuffer-complete' through
/// the filename completion keymap (RET afterwards still exits).
#[test]
fn tui_find_file_tab_completion_runs() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        let mut script = vec![C_X, 6];
        keys(&mut script, "/tmp/");
        script.push(TAB); // minibuffer-complete over /tmp/ entries
        keys(&mut script, "zz_tui_tab.txt");
        script.push(RET);
        script.extend([C_X, 3]);
        run(script, &mut i);
        let bn = i
            .buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().name.clone());
        // The read finished and the file was visited.
        assert_eq!(bn.as_deref(), Some("zz_tui_tab.txt"));
        let _ = std::fs::remove_file(std::env::temp_dir().join("zz_tui_tab.txt"));
    })
}

/// C-g in a filename minibuffer aborts the read (minibuffer-quit) and
/// lands back in the previous buffer.
#[test]
fn tui_find_file_quit_aborts_read() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        let mut script = vec![C_X, 6];
        keys(&mut script, "/tmp/nope-xyz");
        script.push(7); // C-g → throw 'exit
        script.extend([C_X, 3]);
        run(script, &mut i);
        let bn = i
            .buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().name.clone());
        // No file buffer was visited; we're back at the startup buffer.
        assert_ne!(bn.as_deref(), Some("nope-xyz"));
        assert!(i.quit_editor);
    })
}

/// C-x C-f on a directory runs `dired-noselect' through
/// `find-directory-functions' (GNU `find-file-noselect' parity):
/// the minibuffer exits and a `dired-mode' buffer is displayed.
/// Regression: the Rust `find-file-noselect' subr used to visit
/// directories as empty regular-file buffers, and a nil
/// `directory-listing-before-filename-regexp' (shadowed prelude
/// `defvar') made `dired-move-to-filename' signal `stringp nil'.
#[test]
fn tui_find_file_directory_opens_dired() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        let mut script = vec![C_X, 6];
        keys(&mut script, "/tmp/");
        script.push(RET);
        script.extend([C_X, 3]);
        run(script, &mut i);
        let bn = i
            .buffers
            .get(i.current_buffer)
            .map(|b| b.borrow().name.clone());
        assert_eq!(bn.as_deref(), Some("tmp"));
        let mode = i
            .eval_str("(buffer-local-value 'major-mode (current-buffer))")
            .expect("major-mode eval");
        assert!(
            matches!(&mode, remacs::lisp::Value::Sym(s) if i.symbol_name(*s) == "dired-mode"),
            "mode: {:?}",
            mode
        );
        assert!(i.quit_editor);
    })
}

/// The display must follow point: `render_grid' keeps each window's
/// start scrolled so the cursor stays inside the window (GNU
/// `redisplay_window' behavior).  Regression: window start used to be
/// frozen at 0, so moving point below the window bottom left the
/// buffer text unchanged and hid the cursor.
#[test]
fn tui_window_scrolls_to_keep_point_visible() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (dotimes (n 60) (insert (format \"line-%02d\\n\" n)))
           (goto-char (point-min)))",
        )
        .expect("fill scratch");

        // 30× C-n puts point on line-30 — past the last text row of an
        // 80×24 screen (body 23, one mode line → 22 text rows).
        let mut script = vec![14; 30];
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);

        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "line-09");
        assert_eq!(last.rows[21].trim(), "line-30");
        assert_eq!(last.cursor, Some((0, 21)), "cursor: {:?}", last.cursor);
        // Mode line (row 22) reports the new point line.
        assert!(
            last.rows[22].contains("L31"),
            "mode line: {:?}",
            last.rows[22]
        );

        // Moving point far above the window recenters it (M-<).
        i.quit_editor = false;
        let mut script = vec![META | b'<' as i128];
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "line-00");
        assert_eq!(last.rows[1].trim(), "line-01");
        assert_eq!(last.cursor, Some((0, 0)), "cursor: {:?}", last.cursor);

        // Point beyond the right edge scrolls the window horizontally
        // (line truncation model: hscroll follows the cursor column).
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (insert (make-string 120 ?x))
           (goto-char (point-min)))",
        )
        .expect("long line");
        i.quit_editor = false;
        let mut script = vec![5]; // C-e → column 120
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "x".repeat(79));
        assert_eq!(last.cursor, Some((79, 0)), "cursor: {:?}", last.cursor);

        // C-a back to column 0 scrolls the window back to column 0.
        i.quit_editor = false;
        let mut script = vec![1]; // C-a
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "x".repeat(80));
        assert_eq!(last.cursor, Some((0, 0)), "cursor: {:?}", last.cursor);
    })
}

/// C-v (`scroll-up-command`) must actually scroll: `window_scroll'
/// moves the window's `start' AND — when point would leave the
/// window — moves point to the first visible line, like GNU's
/// `window_scroll_line_based'.  Regression: `f_scroll_up' used to
/// move only `start', so `render_grid's point-visible snap undid the
/// scroll on the very next frame — C-v appeared to do nothing
/// whenever point sat above the new visible range.
/// GNU 31.1 observed (80×24 tty): C-v → point lands on the first
/// visible line, start advances by text-height −
/// `next-screen-context-lines' (20).
#[test]
fn tui_scroll_up_command_scrolls_and_moves_point() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (dotimes (n 200) (insert (format \"line-%03d\\n\" n)))
           (goto-char (point-min)))",
        )
        .expect("fill scratch");

        // C-v then C-x C-c.  text_rows = 22, context 2 → start +20.
        let script = vec![22, C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(
            last.rows[0].trim(),
            "line-020",
            "first row: {:?}",
            last.rows[0]
        );
        // Point is on the first visible line (GNU behavior).
        assert_eq!(last.cursor, Some((0, 0)), "cursor: {:?}", last.cursor);
        assert!(
            last.rows[22].contains("L21"),
            "mode line: {:?}",
            last.rows[22]
        );

        // A second C-v scrolls another page — the window is not stuck.
        i.quit_editor = false;
        let script = vec![22, C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "line-040");

        // M-v (`scroll-down-command`) scrolls back a page; point (line-040)
        // stays inside the window, so it keeps its position (GNU only
        // moves point when the scroll would leave it outside).
        i.quit_editor = false;
        let script = vec![META | b'v' as i128, C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "line-020");
        assert_eq!(last.cursor, Some((0, 20)), "cursor: {:?}", last.cursor);

        // A numeric prefix scrolls that many lines (C-u 5 C-v); point
        // (line-040) remains visible → unchanged.
        i.quit_editor = false;
        let script = vec![21, b'5' as i128, 22, C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(last.rows[0].trim(), "line-025");
        assert_eq!(last.cursor, Some((0, 15)), "cursor: {:?}", last.cursor);
    })
}

/// `scroll-left'/`scroll-right'/`set-window-hscroll' must not be
/// undone by the point-visible snap: GNU sets
/// `suspend_auto_hscroll', which holds auto-hscroll until point
/// moves (xdisp.c `window-point != old_pointm').  While suspended
/// the cursor clamps to the window edge rather than hiding.
/// Regression: without the flag the very next frame snapped
/// `hscroll' back to the cursor column.
#[test]
fn tui_hscroll_suspends_until_point_moves() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (insert (make-string 120 ?x))
           (goto-char (point-min)))",
        )
        .expect("long line");

        // One render to settle hscroll=0 at point 0.  Then scroll-left
        // 60 via eval (same path as C-x > through the subr) and render
        // again: the view must stay scrolled, cursor clamped to col 0.
        let script = vec![C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        assert_eq!(grids.borrow().last().unwrap().cursor, Some((0, 0)));

        i.quit_editor = false;
        i.eval_str("(scroll-left 60)").expect("scroll-left");
        let script = vec![C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(
            last.rows[0].trim(),
            "x".repeat(60), // cols 60..120 of the 120-char line
            "row0: {:?}",
            last.rows[0]
        );
        // Cursor: point col 0 is left of hscroll=60 → clamped to col 0.
        assert_eq!(last.cursor, Some((0, 0)), "cursor: {:?}", last.cursor);

        // scroll-right partially restores; suspension still holds.
        i.quit_editor = false;
        i.eval_str("(scroll-right 20)").expect("scroll-right");
        let script = vec![C_X, 3];
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        assert_eq!(
            last.rows[0].trim(),
            "x".repeat(80), // cols 40..120
            "row0: {:?}",
            last.rows[0]
        );

        // Moving point (C-f) lifts the suspension → auto-hscroll puts
        // the cursor back inside the window.
        i.quit_editor = false;
        let script = vec![6, C_X, 3]; // C-f
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().expect("a rendered grid").clone();
        // Point col 1, auto-hscroll re-shows it: hscroll drops back to
        // the point column and the cursor sits at column 0.
        assert_eq!(last.cursor, Some((0, 0)), "cursor: {:?}", last.cursor);
    })
}

/// C-s must paint the match like GNU isearch: the current match gets
/// the `isearch' face (inverse on this tty) and every other visible
/// match gets `lazy-highlight' (underline).  Regression: render_grid
/// ignored faces entirely, so hits stayed invisible.
#[test]
fn tui_isearch_highlights_matches() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (insert \"xxx yyy xxx\\n\")
           (goto-char (point-min)))",
        )
        .expect("fill scratch");

        // C-s x x x RET C-x C-c
        let mut script = vec![19];
        keys(&mut script, "xxx");
        script.push(RET);
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);

        // Find the frame rendered while the search was still open:
        // the last one whose row 0 cells carry the highlights.
        let all = grids.borrow();
        let hit = all
            .iter()
            .rev()
            .find(|g| g.attrs[0][..3].iter().all(|c| c.inverse))
            .expect("no frame showed the current isearch match")
            .clone();
        assert_eq!(hit.rows[0].trim(), "xxx yyy xxx");
        // Current match "xxx" (cols 0..3): isearch face = inverse.
        assert!(
            hit.attrs[0][..3].iter().all(|c| c.inverse),
            "current match cells: {:?}",
            &hit.attrs[0][..5]
        );
        assert!(!hit.attrs[0][3].inverse, "gap char not highlighted");
        // Second match (cols 8..11): lazy-highlight face = underline.
        assert!(
            hit.attrs[0][8..11].iter().all(|c| c.underline),
            "lazy match cells: {:?}",
            &hit.attrs[0][8..11]
        );
        assert!(!hit.attrs[0][8].inverse);

        // After RET the overlays are gone — no stray highlighting.
        let last = grids.borrow().last().unwrap().clone();
        assert!(
            last.attrs[0].iter().all(|c| !c.inverse && !c.underline),
            "leftover highlight: {:?}",
            &last.attrs[0][..8]
        );

        // A failing search leaves no current-match overlay either.
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (insert \"abc\\n\")
           (goto-char (point-min)))",
        )
        .expect("fill scratch");
        let mut script = vec![19];
        keys(&mut script, "zz");
        script.push(7); // C-g
        script.extend([C_X, 3]);
        let (_, grids) = run_capture(script, &mut i);
        let last = grids.borrow().last().unwrap().clone();
        assert!(
            last.attrs[0].iter().all(|c| !c.inverse && !c.underline),
            "failed search left highlight: {:?}",
            &last.attrs[0][..8]
        );
    })
}

/// Text-property and anonymous-list faces paint their cells too —
/// `render_grid' resolves the `face' prop the same way overlay
/// faces resolve.
#[test]
fn tui_face_text_props_and_region_paint() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            "(with-current-buffer \"*scratch*\"
           (erase-buffer)
           (insert \"abcdef\\n\")
           (put-text-property 1 3 'face '(:foreground \"red\" :underline t))
           (let ((ov (make-overlay 4 6)))
             (overlay-put ov 'face '(bold)))
           (set-mark 7)
           (goto-char 2)
           (setq mark-active t))",
        )
        .expect("set props");
        let grid = render_grid(&i, 80, 24);
        // 'face text prop: anonymous plist face — fg red + underline.
        let c0 = grid.attrs[0][0];
        assert_eq!(c0.fg, Some(remacs::term::CellColor(255, 0, 0)));
        assert!(c0.underline, "prop face underline: {c0:?}");
        assert!(!grid.attrs[0][2].underline);
        // Overlay 'face '(bold).
        assert!(
            grid.attrs[0][3].bold,
            "overlay face: {:?}",
            grid.attrs[0][3]
        );
        assert!(grid.attrs[0][4].bold);
        assert!(!grid.attrs[0][5].bold);
        // Active region mark(7)→point(2) covers cols 1..6: bg gray.
        for x in 1..6 {
            assert_eq!(
                grid.attrs[0][x].bg,
                Some(remacs::term::CellColor(190, 190, 190)),
                "region cell {x}: {:?}",
                grid.attrs[0][x]
            );
        }
        assert!(grid.attrs[0][0].bg.is_none(), "cell 0 outside region");
        assert!(grid.attrs[0][6].bg.is_none(), "cell 6 outside region");
    })
}

#[test]
fn tui_overlay_priority_matches_effective_face() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        for (bounds, apriority, bpriority, expected) in [
            ((1, 7, 1, 7), "'(nil . 10)", "'(nil . 1)", (255, 0, 0)),
            ((3, 5, 1, 5), "0", "0", (255, 0, 0)),
            ((1, 5, 1, 7), "'(nil . 1)", "'(nil . 10)", (255, 0, 0)),
            ((1, 7, 1, 7), "'(nil . 1)", "'(nil . 10)", (0, 0, 255)),
            ((3, 5, 1, 7), "1", "2", (0, 0, 255)),
        ] {
            let (abegin, aend, bbegin, bend) = bounds;
            i.eval_str(&format!(
                r##"(with-current-buffer "*scratch*"
                  (erase-buffer) (delete-all-overlays) (insert "abcdef\n")
                  (setq mark-active nil) (goto-char 1)
                  (let ((a (make-overlay {abegin} {aend}))
                        (b (make-overlay {bbegin} {bend})))
                    (overlay-put a 'priority {apriority})
                    (overlay-put b 'priority {bpriority})
                    (overlay-put a 'face '(:foreground "#ff0000"))
                    (overlay-put b 'face '(:foreground "#0000ff"))))"##
            ))
            .expect("set overlay precedence");
            let face = i.eval_str("(get-char-property 3 'face)").unwrap();
            let hex = if expected == (255, 0, 0) {
                "#ff0000"
            } else {
                "#0000ff"
            };
            assert_eq!(i.prin1_to_string(&face), format!("(:foreground \"{hex}\")"));
            let grid = render_grid(&i, 80, 24);
            assert_eq!(
                grid.attrs[0][2].fg,
                Some(remacs::term::CellColor(expected.0, expected.1, expected.2)),
                "painted precedence for {bounds:?}: {:?}",
                grid.attrs[0][2]
            );
        }
    })
}

#[test]
fn tui_category_overlay_faces_follow_defaults_and_nil_shadows() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            r##"(with-current-buffer "*scratch*"
              (erase-buffer) (insert "abc\n") (goto-char 1)
              (setq mark-active nil
                    qa312-category (make-symbol "qa312-paint")
                    qa312-overlay (make-overlay 1 4))
              (put qa312-category 'face '(:foreground "#ff0000"))
              (overlay-put qa312-overlay 'category qa312-category))"##,
        )
        .expect("set inherited overlay face");
        for (change, expected) in [
            ("nil", (255, 0, 0)),
            (
                r##"(put qa312-category 'face '(:foreground "#0000ff"))"##,
                (0, 0, 255),
            ),
            (
                r##"(progn (overlay-put qa312-overlay 'face nil)
                (put-text-property 1 4 'face '(:foreground "#00ff00")))"##,
                (0, 255, 0),
            ),
        ] {
            i.eval_str(change).expect("change inherited face");
            let grid = render_grid(&i, 80, 24);
            assert_eq!(
                grid.attrs[0][1].fg,
                Some(remacs::term::CellColor(expected.0, expected.1, expected.2)),
                "category face after {change}: {:?}",
                grid.attrs[0][1]
            );
        }
    })
}

#[test]
fn tui_window_overlay_faces_match_window_property_queries() {
    run_on_big_stack(move || {
        let mut i = Interp::new();
        i.eval_str(
            r##"(with-current-buffer "*scratch*"
              (erase-buffer) (delete-other-windows) (insert "abc\n") (goto-char 1)
              (setq mark-active nil qa313-top (selected-window)
                    qa313-bottom (split-window-below))
              (put-text-property 1 4 'face '(:foreground "#0000ff")))"##,
        )
        .expect("set window face fixture");
        for (restriction, top_red) in [
            ("(overlay-put qa313-overlay 'window qa313-bottom)", false),
            (
                "(progn (put qa313-category 'window qa313-bottom)
                (overlay-put qa313-overlay 'category qa313-category))",
                false,
            ),
            ("(overlay-put qa313-overlay 'window t)", true),
            (
                "(progn (put qa313-category 'window qa313-bottom)
                (overlay-put qa313-overlay 'category qa313-category)
                (overlay-put qa313-overlay 'window nil))",
                true,
            ),
        ] {
            i.eval_str(&format!(
                r##"(progn (delete-all-overlays)
                  (setq qa313-category (make-symbol "qa313-paint")
                        qa313-overlay (make-overlay 1 4))
                  (overlay-put qa313-overlay 'face '(:foreground "#ff0000"))
                  {restriction})"##
            ))
            .expect("set overlay window restriction");
            let face = i.eval_str("(get-char-property 2 'face qa313-top)").unwrap();
            let hex = if top_red { "#ff0000" } else { "#0000ff" };
            assert_eq!(i.prin1_to_string(&face), format!("(:foreground \"{hex}\")"));
            let insertion_face = i.eval_str("(get-pos-property 2 'face qa313-top)").unwrap();
            assert_eq!(
                i.prin1_to_string(&insertion_face),
                "(:foreground \"#ff0000\")"
            );
            let grid = render_grid(&i, 80, 24);
            // The grid scales the window tree to the requested terminal size.
            // The lower text area starts after the first rendered mode line.
            let bottom = grid.mode_rows.iter().copied().min().unwrap() + 1;
            assert!(grid.rows[bottom].starts_with("abc"));
            let top = if top_red { (255, 0, 0) } else { (0, 0, 255) };
            assert_eq!(
                grid.attrs[0][1].fg,
                Some(remacs::term::CellColor(top.0, top.1, top.2))
            );
            assert_eq!(
                grid.attrs[bottom][1].fg,
                Some(remacs::term::CellColor(255, 0, 0)),
                "lower window after {restriction}: row={bottom}, mode rows={:?}, rows={:?}",
                grid.mode_rows,
                grid.rows
            );
        }
    })
}

/// Ending a definition is already its first execution; C-x e adds one more.
#[test]
fn tui_macro_end_keeps_the_recorded_iteration() {
    run_on_big_stack(|| {
        for repeat_after_end in [false, true] {
            let mut interp = Interp::new_with_noninteractive(false);
            interp
                .eval_str("(switch-to-buffer (get-buffer-create \"macro-repeat\"))")
                .unwrap();
            let mut script = vec![C_X, b'(' as i128, b'x' as i128, C_X, b')' as i128];
            if repeat_after_end {
                script.extend([C_X, b'e' as i128]);
            }
            script.extend([C_X, 3]);
            run(script, &mut interp);
            let value = interp
                .eval_str("(with-current-buffer \"macro-repeat\" (buffer-string))")
                .unwrap();
            assert_eq!(
                interp.princ_to_string(&value),
                if repeat_after_end { "xx" } else { "x" }
            );
        }
    });
}
