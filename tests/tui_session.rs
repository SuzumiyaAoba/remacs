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
    assert_eq!(last.rows[0].trim(), "line-020", "first row: {:?}", last.rows[0]);
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
