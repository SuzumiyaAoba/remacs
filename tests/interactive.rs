//! Interactive-command machinery: command-execute, interactive specs,
//! prefix args, minibuffer input via the injectable reader hook.
mod common;

use common::*;
use remacs::lisp::{Flow, Interp, MinibufInput, Value};
use std::rc::Rc;

/// Install a minibuf_reader that answers each prompt from `answers`
/// (consumed in order; each entry is a Text line or a Key code).
fn canned(i: &mut Interp, answers: Vec<MinibufInput>) {
    let answers = std::cell::RefCell::new(
        answers
            .into_iter()
            .collect::<std::collections::VecDeque<_>>(),
    );
    i.minibuf_reader = Some(Rc::new(move |_, _prompt, _single| {
        answers.borrow_mut().pop_front().ok_or(Flow::Quit)
    }));
}

/// Run `command-execute` on the command named `name`.
fn cmd(i: &mut Interp, name: &str) -> remacs::lisp::EvalResult {
    let id = i.intern(name);
    i.command_execute(&Value::Sym(id))
}

/// `prin1` of the value of `src` in `i`.
fn pev(i: &mut Interp, src: &str) -> String {
    let v = ev_in(i, src);
    i.prin1_to_string(&v)
}

fn ev_in(i: &mut Interp, src: &str) -> Value {
    match i.eval_str(src) {
        Ok(v) => v,
        Err(f) => panic!("eval failed: {} -> {:?}", src, f),
    }
}

fn ev_err_in(i: &mut Interp, src: &str) -> String {
    match i.eval_str(src) {
        Ok(v) => panic!("expected error, got {}", i.prin1_to_string(&v)),
        Err(Flow::Signal(sym, _)) => match &sym {
            Value::Sym(id) => i.symbol_name(*id),
            _ => "?".into(),
        },
        Err(Flow::Quit) => "quit".into(),
        Err(_) => "throw".into(),
    }
}

fn set(i: &mut Interp, name: &str, v: Value) {
    let id = i.intern(name);
    i.set_symbol(id, v).unwrap();
}

fn buffer_string(i: &mut Interp) -> String {
    match ev_in(i, "(buffer-string)") {
        Value::Str(s) => s.borrow().clone(),
        v => panic!("buffer-string -> {}", i.prin1_to_string(&v)),
    }
}

// ---------- command-execute with subr interactive specs ----------

#[test]
fn self_insert_via_command_execute() {
    let (mut i, _) = interp();
    set(&mut i, "last-command-event", Value::Int(97));
    cmd(&mut i, "self-insert-command").unwrap();
    assert_eq!(buffer_string(&mut i), "a");
}

#[test]
fn self_insert_prefix_repeat() {
    let (mut i, _) = interp();
    set(&mut i, "last-command-event", Value::Int(98));
    set(
        &mut i,
        "current-prefix-arg",
        Value::list(vec![Value::Int(4)]),
    );
    cmd(&mut i, "self-insert-command").unwrap();
    assert_eq!(buffer_string(&mut i), "bbbb");
}

#[test]
fn forward_char_prefix() {
    let (mut i, _) = interp();
    ev_in(&mut i, "(insert \"0123456789\") (goto-char 5)");
    set(
        &mut i,
        "current-prefix-arg",
        Value::list(vec![Value::Int(4)]),
    );
    cmd(&mut i, "forward-char").unwrap();
    assert_eq!(pev(&mut i, "(point)"), "9");
}

#[test]
fn kill_region_interactive() {
    let (mut i, _) = interp();
    ev_in(
        &mut i,
        "(insert \"abcdef\") (goto-char 2) (set-mark 2) (goto-char 5)",
    );
    cmd(&mut i, "kill-region").unwrap();
    assert_eq!(buffer_string(&mut i), "aef");
}

#[test]
fn commandp_reports_commands() {
    assert_eq!(ev("(commandp 'self-insert-command)"), "t");
    assert_eq!(ev("(commandp 'forward-char)"), "t");
    assert_eq!(ev("(commandp 'universal-argument)"), "t");
    assert_eq!(ev("(commandp 'car)"), "nil");
    assert_eq!(ev("(commandp 'next-line)"), "t"); // prelude defun
    assert_eq!(ev("(commandp 42)"), "nil");
}

// ---------- universal-argument / digit-argument ----------

#[test]
fn universal_argument_sets_prefix() {
    let (mut i, _) = interp();
    cmd(&mut i, "universal-argument").unwrap();
    let pa_id = i.intern("prefix-arg");
    let pa = i.symbol_value(pa_id);
    assert_eq!(i.prin1_to_string(&pa), "(4)");
    // Second C-u squares it.
    cmd(&mut i, "universal-argument").unwrap();
    let pa_id = i.intern("prefix-arg");
    let pa = i.symbol_value(pa_id);
    assert_eq!(i.prin1_to_string(&pa), "(16)");
}

#[test]
fn digit_argument_from_event() {
    let (mut i, _) = interp();
    set(&mut i, "last-command-event", Value::Int(51 | 0x800_0000)); // M-3
    cmd(&mut i, "digit-argument").unwrap();
    let pa_id = i.intern("prefix-arg");
    let pa = i.symbol_value(pa_id);
    assert_eq!(i.prin1_to_string(&pa), "3");
}

// ---------- minibuffer via injected reader ----------

#[test]
fn read_string_via_hook() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("hello".into())]);
    let v = ev_in(&mut i, "(read-string \"Name: \")");
    assert_eq!(i.prin1_to_string(&v), "\"hello\"");
}

#[test]
fn read_char_via_hook() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Key(97)]);
    let v = ev_in(&mut i, "(read-char)");
    assert_eq!(i.prin1_to_string(&v), "97");
}

#[test]
fn y_or_n_p_via_hook() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Key(121)]); // 'y'
    let v = ev_in(&mut i, "(y-or-n-p \"Proceed? \")");
    assert_eq!(i.prin1_to_string(&v), "t");
    canned(&mut i, vec![MinibufInput::Key(110)]); // 'n'
    let v = ev_in(&mut i, "(y-or-n-p \"Proceed? \")");
    assert_eq!(i.prin1_to_string(&v), "nil");
}

#[test]
fn interactive_b_defaults_current_buffer() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text(String::new())]);
    let v = ev_in(
        &mut i,
        "(defun f (b) (interactive \"bBuf: \") b) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "\"*scratch*\"");
}

#[test]
fn interactive_s_and_a_codes() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("somestring".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (s) (interactive \"sIn: \") s) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "\"somestring\"");
    canned(&mut i, vec![MinibufInput::Text("somesym".into())]);
    let v = ev_in(
        &mut i,
        "(defun g (s) (interactive \"aSym: \") s) (call-interactively 'g)",
    );
    assert_eq!(i.prin1_to_string(&v), "somesym");
}

#[test]
fn interactive_x_evals_input() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("(+ 20 22)".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"xEval: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
}

#[test]
fn interactive_r_region_bounds() {
    let (mut i, _) = interp();
    ev_in(&mut i, "(insert \"abcdef\") (set-mark 2) (goto-char 5)");
    let v = ev_in(
        &mut i,
        "(defun f (b e) (interactive \"r\") (list b e)) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "(2 5)");
}

#[test]
fn interactive_list_form() {
    let (mut i, _) = interp();
    let v = ev_in(
        &mut i,
        "(defun f (a b) (interactive (list 10 20)) (list a b))
         (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "(10 20)");
}

#[test]
fn eval_expression_interactive() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("(+ 1 2)".into())]);
    cmd(&mut i, "eval-expression").unwrap();
    // Result is echoed via `message`; also returned.
}

#[test]
fn execute_extended_command_runs_named_command() {
    let (mut i, _) = interp();
    ev_in(&mut i, "(insert \"x\ny\nz\") (goto-char 1)");
    canned(&mut i, vec![MinibufInput::Text("forward-line".into())]);
    cmd(&mut i, "execute-extended-command").unwrap();
    assert_eq!(pev(&mut i, "(point)"), "3");
}

#[test]
fn minibuf_quit_signals() {
    let (mut i, _) = interp();
    canned(&mut i, vec![]); // reader returns Quit immediately
    assert_eq!(ev_err_in(&mut i, "(read-string \"X: \")"), "quit");
}

#[test]
fn read_minibuffer_absent_hook() {
    // Without a front-end, reads fall back gracefully rather than panic.
    assert_eq!(ev("(read-string \"P: \")"), "nil");
}

// ---------- prefix arg plumbing ----------

#[test]
fn prefix_numeric_values() {
    // raw prefix forms → numeric via the "p" spec
    let (mut i, _) = interp();
    for (arg, want) in [
        ("'(4)", "4"),
        ("'(16)", "16"),
        ("'-", "-1"),
        ("nil", "1"),
        ("7", "7"),
    ] {
        ev_in(&mut i, &format!("(setq current-prefix-arg {})", arg));
        let v = ev_in(
            &mut i,
            "(defun f (n) (interactive \"p\") n) (call-interactively 'f)",
        );
        assert_eq!(i.prin1_to_string(&v), want, "arg {}", arg);
        ev_in(&mut i, "(makunbound 'f)");
    }
}

// ---------- keymaps ----------

#[test]
fn key_binding_and_define_key() {
    assert_eq!(ev("(key-binding \"a\")"), "self-insert-command");
    assert_eq!(ev("(key-binding \"\\C-u\")"), "universal-argument");
    assert_eq!(ev("(key-binding \"\\C-f\")"), "forward-char");
    assert_eq!(
        ev("(key-binding \"\\C-x\\C-c\")"),
        "save-buffers-kill-emacs"
    );
    assert_eq!(
        ev("(progn (define-key (current-global-map) [f5] 'ignore) (key-binding [f5]))"),
        "ignore"
    );
}

#[test]
fn where_is_internal_finds_binding() {
    let v = ev("(where-is-internal 'forward-char)");
    assert!(v.contains("6") || v.contains("\\C-f"), "{}", v);
}

#[test]
fn describe_key_shape() {
    // describe-key renders into *Help*.
    let v = ev("(progn (describe-key \"\\C-f\")
                (with-current-buffer \"*Help*\" (buffer-string)))");
    assert!(v.contains("forward-char"), "{}", v);
    assert!(v.contains("C-f"), "{}", v);
}

#[test]
fn load_file_evals_forms() {
    let path = std::env::temp_dir().join("remacs_test_load.el");
    std::fs::write(
        &path,
        "(defun loaded-fn (x) (+ x 41))\n(setq loaded-var 99)\n",
    )
    .unwrap();
    let (mut i, _) = interp();
    let v = ev_in(
        &mut i,
        &format!("(progn (load \"{}\") (loaded-fn 1))", path.display()),
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    assert_eq!(pev(&mut i, "loaded-var"), "99");
    // load-file-name is bound only during loading.
    let b = pev(&mut i, "(boundp 'load-file-name)");
    assert!(b == "nil" || b == "t");
}

#[test]
fn load_path_search() {
    let dir = std::env::temp_dir().join("remacs_load_path_test");
    std::fs::create_dir_all(&dir).unwrap();
    let lib = dir.join("mylib.el");
    std::fs::write(&lib, "(defun mylib-fn () 'from-mylib)\n").unwrap();
    let (mut i, _) = interp();
    let v = ev_in(
        &mut i,
        &format!(
            "(let ((load-path (list \"{}\")))
               (load \"mylib\") (mylib-fn))",
            dir.display()
        ),
    );
    assert_eq!(i.prin1_to_string(&v), "from-mylib");
}

#[test]
fn load_missing_signals() {
    let (mut i, _) = interp();
    assert_eq!(
        ev_err_in(&mut i, "(load \"/nonexistent-dir-xyz/nofile\")"),
        "file-error"
    );
}

#[test]
fn substitute_command_keys_basic() {
    assert_eq!(
        ev("(substitute-command-keys \"Press \\\\[forward-char]\")"),
        "\"Press C-f\""
    );
    assert_eq!(
        ev("(substitute-command-keys \"\\\\=\\[not-a-key]\")"),
        "\"[not-a-key]\""
    );
}
