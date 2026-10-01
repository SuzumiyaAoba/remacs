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
use remacs::term::KeyIo;
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
    fn render(&mut self, _i: &Interp) -> io::Result<()> {
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
    let echoes = Rc::new(RefCell::new(Vec::new()));
    let io = ScriptIo {
        keys: script.into(),
        echoes: echoes.clone(),
    };
    remacs::term::run_editor_with(io, i).expect("editor loop");
    echoes
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
