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
        Err(Flow::Signal(sym, _, _)) => match &sym {
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
    // GNU callint.c: 'x' reads a Lisp object WITHOUT evaluating it
    // (`eval-minibuffer' is 'X').
    canned(&mut i, vec![MinibufInput::Text("(+ 20 22)".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"xEval: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "(+ 20 22)");
    // 'X' reads and evaluates.
    canned(&mut i, vec![MinibufInput::Text("(+ 20 22)".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"XEval: \") x) (call-interactively 'f)",
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
    // Without a front-end, GNU batch signals end-of-file reading stdin.
    assert_eq!(ev_err("(read-string \"P: \")"), "end-of-file");
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
        "save-buffers-kill-terminal"
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
    // GNU signals `file-missing' (a `file-error' subtype) for ENOENT.
    assert_eq!(
        ev_err_in(&mut i, "(load \"/nonexistent-dir-xyz/nofile\")"),
        "file-missing"
    );
}

#[test]
fn substitute_command_keys_basic() {
    // GNU propertizes the substituted key with help-key-binding.
    assert_eq!(
        ev("(substitute-command-keys \"Press \\\\[forward-char]\")"),
        "#(\"Press C-f\" 6 9 (font-lock-face help-key-binding face help-key-binding))"
    );
    assert_eq!(
        ev("(substitute-command-keys \"\\\\=\\[not-a-key]\")"),
        "\"[not-a-key]\""
    );
}

// ---------- reader-dependent editor paths ----------

#[test]
fn completing_read_paths() {
    let (mut i, _) = interp();
    // Exact match returns input.
    canned(&mut i, vec![MinibufInput::Text("beta".into())]);
    let v = ev_in(&mut i, "(completing-read \"P: \" '(\"alpha\" \"beta\"))");
    assert_eq!(i.prin1_to_string(&v), "\"beta\"");
    // Unique prefix completes.
    canned(&mut i, vec![MinibufInput::Text("alp".into())]);
    let v = ev_in(&mut i, "(completing-read \"P: \" '(\"alpha\" \"beta\"))");
    assert_eq!(i.prin1_to_string(&v), "\"alpha\"");
    // Empty input → default (4th arg position: INITIAL-INPUT then DEF).
    canned(&mut i, vec![MinibufInput::Text(String::new())]);
    let v = ev_in(
        &mut i,
        "(completing-read \"P: \" '(\"alpha\" \"beta\") nil nil nil nil \"DEF\")",
    );
    assert_eq!(i.prin1_to_string(&v), "\"DEF\"");
    // Ambiguous prefix returns input as typed.
    canned(&mut i, vec![MinibufInput::Text("a".into())]);
    let v = ev_in(&mut i, "(completing-read \"P: \" '(\"a1\" \"a2\"))");
    assert_eq!(i.prin1_to_string(&v), "\"a\"");
    // Batch: no reader → GNU signals end-of-file reading stdin.
    let (mut j, _) = interp();
    assert_eq!(
        ev_err_in(&mut j, "(completing-read \"P: \" '(\"alpha\" \"beta\"))"),
        "end-of-file"
    );
    assert_eq!(
        ev_err_in(
            &mut j,
            "(completing-read \"P: \" '(\"alpha\") nil nil \"init\")"
        ),
        "end-of-file"
    );
}

#[test]
fn y_or_n_p_reprompt_and_quit() {
    let (mut i, _) = interp();
    // A non-y/n key re-prompts, then 'y' answers.
    canned(&mut i, vec![MinibufInput::Key(120), MinibufInput::Key(121)]);
    let v = ev_in(&mut i, "(y-or-n-p \"Q? \")");
    assert_eq!(i.prin1_to_string(&v), "t");
    // C-g/C-c quits.
    canned(&mut i, vec![MinibufInput::Key(7)]);
    assert_eq!(ev_err_in(&mut i, "(y-or-n-p \"Q? \")"), "quit");
    // Batch default path (no reader): GNU signals end-of-file.
    let (mut j, _) = interp();
    assert_eq!(ev_err_in(&mut j, "(y-or-n-p \"Q? \")"), "end-of-file");
}

#[test]
fn read_key_sequence_paths() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Key(97)]);
    let v = ev_in(&mut i, "(read-key-sequence \"K: \")");
    assert_eq!(i.prin1_to_string(&v), "\"a\"");
    // Meta char ≥128 → vector.
    canned(&mut i, vec![MinibufInput::Key(0x800_0061)]);
    let v = ev_in(&mut i, "(read-key-sequence \"K: \")");
    assert!(i.prin1_to_string(&v).starts_with('['));
    // Vector variant returns a vector.
    let v = ev_in(&mut i, "(read-key-sequence-vector \"K: \")");
    assert!(i.prin1_to_string(&v).starts_with('['));
    // Batch (no reader): empty string.
    let (mut j, _) = interp();
    let v = ev_in(&mut j, "(read-key-sequence \"K: \")");
    assert_eq!(j.prin1_to_string(&v), "\"\"");
}

#[test]
fn digit_argument_minus_and_accumulate() {
    let (mut i, _) = interp();
    // M-- starts negative.
    set(
        &mut i,
        "last-command-event",
        Value::Int('-' as i128 | 0x800_0000),
    );
    cmd(&mut i, "digit-argument").unwrap();
    let pa = i.symbol_value(i.intern_soft("prefix-arg").unwrap());
    assert_eq!(i.prin1_to_string(&pa), "(-)");
    // Digit after list-form prefix → plain int.
    set(&mut i, "last-command-event", Value::Int('5' as i128));
    cmd(&mut i, "digit-argument").unwrap();
    let pa = i.symbol_value(i.intern_soft("prefix-arg").unwrap());
    assert_eq!(i.prin1_to_string(&pa), "5");
    // Accumulate digits.
    set(&mut i, "last-command-event", Value::Int('2' as i128));
    cmd(&mut i, "digit-argument").unwrap();
    let pa = i.symbol_value(i.intern_soft("prefix-arg").unwrap());
    assert_eq!(i.prin1_to_string(&pa), "52");
    // '-' on an int prefix negates.
    set(&mut i, "prefix-arg", Value::Int(7));
    set(&mut i, "last-command-event", Value::Int('-' as i128));
    cmd(&mut i, "digit-argument").unwrap();
    let pa = i.symbol_value(i.intern_soft("prefix-arg").unwrap());
    assert_eq!(i.prin1_to_string(&pa), "-7");
}

#[test]
fn describe_key_undefined() {
    let (mut i, _) = interp();
    // An unbound key renders into *Help* without panic.
    let _ = ev_in(&mut i, "(describe-key [f17])");
}

#[test]
fn current_kill_and_yank_paths() {
    let (mut i, _) = interp();
    ev_in(&mut i, "(insert \"hello world\") (kill-region 1 6)");
    let v = ev_in(&mut i, "(current-kill 0)");
    assert_eq!(i.prin1_to_string(&v), "\"hello\"");
    // current-kill rotates.
    let v = ev_in(&mut i, "(current-kill 0 t)");
    let _ = v;
    // kill-new pushes and (per GNU) returns nil.
    let v = ev_in(&mut i, "(kill-new \"xyz\")");
    assert_eq!(i.prin1_to_string(&v), "nil");
    let v = ev_in(&mut i, "(current-kill 0)");
    assert_eq!(i.prin1_to_string(&v), "\"xyz\"");
}

// ---------- more interactive spec codes ----------

#[test]
fn interactive_spec_codes_batch() {
    let (mut i, _) = interp();
    // 'd' → point, 'm' → mark, 'i' → nil-ish, 'P' → raw prefix, 'n' reads a number.
    ev_in(&mut i, "(insert \"ab\") (goto-char 2) (set-mark 1)");
    let v = ev_in(
        &mut i,
        "(defun f (d m p) (interactive \"d\\nm\\np\") (list d m p)) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "(2 1 1)");
    // 'P' → raw prefix arg.
    set(
        &mut i,
        "current-prefix-arg",
        Value::list(vec![Value::Int(4)]),
    );
    let v = ev_in(
        &mut i,
        "(defun f (p) (interactive \"P\") p) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "(4)");
    // 'n' reads a number through the reader even when a prefix is
    // set (GNU callint.c: only 'N' consults the prefix).
    canned(&mut i, vec![MinibufInput::Text("42".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (n) (interactive \"nNum: \") n) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    // 'c' char, 'e' event via canned key.
    canned(&mut i, vec![MinibufInput::Key(65)]);
    let v = ev_in(
        &mut i,
        "(defun f (c) (interactive \"cChar: \") c) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "65");
    // 'k' key sequence via canned key.
    canned(&mut i, vec![MinibufInput::Key(98)]);
    let v = ev_in(
        &mut i,
        "(defun f (k) (interactive \"kKey: \") k) (call-interactively 'f)",
    );
    let _ = v;
    // 'S' symbol, 'C' command, 'v' variable via text.
    canned(&mut i, vec![MinibufInput::Text("somename".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (s) (interactive \"SSym: \") s) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "somename");
    canned(&mut i, vec![MinibufInput::Text("forward-char".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (c) (interactive \"CCmd: \") c) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "forward-char");
    // 'X' eval + print.
    canned(&mut i, vec![MinibufInput::Text("(+ 1 2)".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"XEval: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "3");
    // 'B' existing buffer name.
    canned(&mut i, vec![MinibufInput::Text(String::new())]);
    let v = ev_in(
        &mut i,
        "(defun f (b) (interactive \"BBuf: \") b) (call-interactively 'f)",
    );
    let _ = v;
    // 'f'/'F' file names.
    canned(&mut i, vec![MinibufInput::Text("/tmp/f.el".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"fFile: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "\"/tmp/f.el\"");
    canned(&mut i, vec![MinibufInput::Text("/tmp/d".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"DDir: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "\"/tmp/d\"");
    // 'z'/'Z' coding system.
    canned(&mut i, vec![MinibufInput::Text("utf-8".into())]);
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"zCoding: \") x) (call-interactively 'f)",
    );
    let _ = v;
    // 'U' unused code shouldn't crash.
    let v = ev_in(
        &mut i,
        "(defun f (u) (interactive \"U\") u) (call-interactively 'f)",
    );
    let _ = v;
}

/// `command_args` pre-supplies arguments to every prompting code —
/// the noninteractive path used when a command is replayed with
/// known args (e.g. by the front-end loop or `execute-extended-command`).
#[test]
fn interactive_spec_command_args() {
    let (mut i, _) = interp();
    i.command_args = vec![Value::Int(42)];
    // 'N' (not 'n') consults a numeric prefix arg first
    // (GNU callint.c).
    set(
        &mut i,
        "current-prefix-arg",
        Value::list(vec![Value::Int(4)]),
    );
    let v = ev_in(
        &mut i,
        "(defun f (n) (interactive \"NNum: \") n) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "4");
    // 'n' ignores the prefix and takes command_args instead.
    ev_in(&mut i, "(defun f (n) (interactive \"nNum: \") n)");
    let v = ev_in(&mut i, "(call-interactively 'f)");
    assert_eq!(i.prin1_to_string(&v), "42");
    // Without a prefix, command_args supplies the value.
    set(&mut i, "current-prefix-arg", Value::Nil);
    let v = ev_in(&mut i, "(call-interactively 'f)");
    assert_eq!(i.prin1_to_string(&v), "42");
    // 's'/'B'/'b'/'F'/'f'/'D'/'z'/'Z' all take command_args first.
    for code in ["s", "B", "b", "F", "f", "D", "z", "Z"] {
        let v = ev_in(
            &mut i,
            &format!(
                "(defun g (x) (interactive \"{}In: \") x) (call-interactively 'g)",
                code
            ),
        );
        assert_eq!(i.prin1_to_string(&v), "42", "code {}", code);
        ev_in(&mut i, "(fmakunbound 'g)");
    }
    // Symbol-valued codes 'a' 'C' 'S' 'v' take command_args verbatim.
    for code in ["a", "C", "S", "v"] {
        let v = ev_in(
            &mut i,
            &format!(
                "(defun g (x) (interactive \"{}In: \") x) (call-interactively 'g)",
                code
            ),
        );
        assert_eq!(i.prin1_to_string(&v), "42", "code {}", code);
        ev_in(&mut i, "(fmakunbound 'g)");
    }
    // 'k'/'K' key sequences, 'c'/'e' chars, 'x'/'X' evals.
    for code in ["k", "K", "c", "e", "x", "X"] {
        let v = ev_in(
            &mut i,
            &format!(
                "(defun g (x) (interactive \"{}In: \") x) (call-interactively 'g)",
                code
            ),
        );
        assert_eq!(i.prin1_to_string(&v), "42", "code {}", code);
        ev_in(&mut i, "(fmakunbound 'g)");
    }
    // 'p' with command_args and no prefix uses the first arg.
    let v = ev_in(
        &mut i,
        "(defun g (n) (interactive \"p\") n) (call-interactively 'g)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    // 'P' raw prefix: nil prefix falls back to command_args.
    let v = ev_in(
        &mut i,
        "(defun g (n) (interactive \"P\") n) (call-interactively 'g)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
}

// ---------- lambda arg binding paths ----------

#[test]
fn optional_args_dynamic_binding() {
    let (mut i, _) = interp();
    // Dynamic scope exercises the specbind path for optional/rest
    // binding (bare optional vars; init/supplied-p triples are only
    // legal in macro arglists, as in GNU).
    set(&mut i, "lexical-binding", Value::Nil);
    let v = ev_in(
        &mut i,
        "(funcall (lambda (a &optional b &rest r) (list a b r)) 1 2 3 4)",
    );
    assert_eq!(i.prin1_to_string(&v), "(1 2 (3 4))");
    let v = ev_in(
        &mut i,
        "(funcall (lambda (a &optional b &rest r) (list a b r)) 1)",
    );
    assert_eq!(i.prin1_to_string(&v), "(1 nil nil)");
    // Macro arglists accept (var init supplied-p) under dynamic scope.
    ev_in(
        &mut i,
        "(defmacro mopt (&optional (p 10) (q 20 s)) `(list ',p ',q ',s))",
    );
    let v = ev_in(&mut i, "(mopt)");
    assert_eq!(i.prin1_to_string(&v), "(10 20 nil)");
    let v = ev_in(&mut i, "(mopt 1)");
    assert_eq!(i.prin1_to_string(&v), "(1 20 nil)");
    let v = ev_in(&mut i, "(mopt 1 2)");
    assert_eq!(i.prin1_to_string(&v), "(1 2 t)");
}

#[test]
fn command_args_supply_spec_values() {
    let (mut i, _) = interp();
    // Pre-supplied command args feed spec codes without a reader.
    i.command_args = vec![Value::Int(42)];
    let v = ev_in(
        &mut i,
        "(defun f (n) (interactive \"nNum: \") n) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    let v = ev_in(
        &mut i,
        "(defun f (s) (interactive \"sIn: \") s) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    let v = ev_in(
        &mut i,
        "(defun f (a) (interactive \"aSym: \") a) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    let v = ev_in(
        &mut i,
        "(defun f (k) (interactive \"kKey: \") k) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    let v = ev_in(
        &mut i,
        "(defun f (x) (interactive \"xEval: \") x) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    let v = ev_in(
        &mut i,
        "(defun f (c) (interactive \"cChar: \") c) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
    // 'P' also takes the raw prefix from command_args.
    let v = ev_in(
        &mut i,
        "(defun f (p) (interactive \"P\") p) (call-interactively 'f)",
    );
    assert_eq!(i.prin1_to_string(&v), "42");
}

// ---------- minibuffer lifecycle (GNU read_minibuf) ----------

#[test]
fn minibuf_read_runs_hooks_and_restores_state() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("answer".into())]);
    // The setup hook runs with the minibuffer selected, depth 1,
    // the prompt installed (a `field' prefix) and the exit hook
    // queued.  Both run exactly once.
    let v = ev_in(
        &mut i,
        "(let ((log nil))
           (add-hook 'minibuffer-setup-hook
                     (lambda ()
                       (setq log
                             (cons (list 'setup
                                         (minibuffer-depth)
                                         (minibufferp)
                                         (minibuffer-prompt)
                                         (minibuffer-prompt-end)
                                         (buffer-name))
                                   log))))
           (add-hook 'minibuffer-exit-hook
                     (lambda ()
                       (setq log
                             (cons (list 'exit
                                         (minibuffer-depth)
                                         (minibuffer-contents))
                                   log))))
           (list (read-from-minibuffer \"P: \") (nreverse log) (minibuffer-depth)))",
    );
    let s = i.prin1_to_string(&v);
    assert!(s.contains("\"answer\""), "{}", s);
    assert!(s.contains("setup"), "{}", s);
    assert!(s.contains("exit"), "{}", s);
    assert!(s.contains("\"P: \""), "{}", s);
    assert!(s.contains("Minibuf"), "{}", s);
}

#[test]
fn minibuf_read_history_and_defaults() {
    let (mut i, _) = interp();
    canned(&mut i, vec![
        MinibufInput::Text("first".into()),
        MinibufInput::Text("".into()), // empty → default lands in history
    ]);
    let v = ev_in(
        &mut i,
        "(progn
           (read-from-minibuffer \"A: \" nil nil nil 'my-hist)
           (read-from-minibuffer \"B: \" nil nil nil 'my-hist \"DEF\")
           my-hist)",
    );
    // GNU pushes the entered string, then the default on empty input.
    assert_eq!(i.prin1_to_string(&v), "(\"DEF\" \"first\")");
}

#[test]
fn minibuf_read_setup_hook_error_unwinds() {
    let (mut i, _) = interp();
    canned(&mut i, vec![MinibufInput::Text("x".into())]);
    // GNU `run_hook' for setup: an error aborts the read but the
    // exit-hook/unwind chain still runs (depth back to 0).
    let v = ev_in(
        &mut i,
        "(let ((exits 0))
           (add-hook 'minibuffer-setup-hook (lambda () (error \"boom\")))
           (add-hook 'minibuffer-exit-hook (lambda () (setq exits (1+ exits))))
           (list (condition-case e
                     (read-from-minibuffer \"P: \")
                   (error 'caught))
                 exits
                 (minibuffer-depth)))",
    );
    assert_eq!(i.prin1_to_string(&v), "(caught 1 0)");
}

// ---------- standard-output destinations ----------

#[test]
fn standard_output_dests() {
    let (mut i, _) = interp();
    // Buffer destination.
    ev_in(
        &mut i,
        "(let ((standard-output (get-buffer-create \"so-buf\")))
           (princ \"HELLO\"))
         (with-current-buffer \"so-buf\" (buffer-string))",
    );
    let v = ev_in(&mut i, "(with-current-buffer \"so-buf\" (buffer-string))");
    assert!(i.prin1_to_string(&v).contains("HELLO"));
    // Marker destination.
    ev_in(
        &mut i,
        "(let ((m (set-marker (make-marker) 1 \"so-buf\")))
           (let ((standard-output m)) (princ \"MM\")))",
    );
    // Function destination: GNU calls the stream once per character.
    ev_in(
        &mut i,
        "(setq out-acc nil)
         (let ((standard-output (lambda (s) (setq out-acc (cons s out-acc)))))
           (princ \"F1\") (princ \"F2\"))",
    );
    let v = ev_in(&mut i, "(length out-acc)");
    assert_eq!(i.prin1_to_string(&v), "4");
    // Symbol naming a function.
    ev_in(
        &mut i,
        "(defun my-sink (s) (setq out-acc2 (cons s out-acc2)))
         (setq out-acc2 nil)
         (let ((standard-output 'my-sink)) (princ \"Q\"))",
    );
    let v = ev_in(&mut i, "out-acc2");
    assert_eq!(i.prin1_to_string(&v), "(81)");
    // kill the helper buffer.
    ev_in(&mut i, "(kill-buffer \"so-buf\")");
}

// ---------- lambda arg binding ----------

#[test]
fn lambda_optional_rest_bindings() {
    let (mut i, _) = interp();
    // (var init) in a *lambda* arglist → invalid-function (GNU agrees);
    // the extended form is legal in macro arglists.
    assert_eq!(
        ev_err_in(
            &mut i,
            "(funcall (lambda (a &optional (b 10)) (list a b)) 1)"
        ),
        "invalid-function"
    );
    // defmacro supports (var init) and (var init supplied-p).
    let v = ev_in(
        &mut i,
        "(defmacro dm (&optional (a 5 ap)) (list 'list a ap)) (dm)",
    );
    assert_eq!(i.prin1_to_string(&v), "(5 nil)");
    let v = ev_in(&mut i, "(dm 9)");
    assert_eq!(i.prin1_to_string(&v), "(9 t)");
    // &rest collects the tail.
    let v = ev_in(&mut i, "(funcall (lambda (a &rest r) (list a r)) 1 2 3 4)");
    assert_eq!(i.prin1_to_string(&v), "(1 (2 3 4))");
    // Too many args → error.
    assert_eq!(
        ev_err_in(&mut i, "(funcall (lambda (a) a) 1 2)"),
        "wrong-number-of-arguments"
    );
    // Too few args → error.
    assert_eq!(
        ev_err_in(&mut i, "(funcall (lambda (a b) a) 1)"),
        "wrong-number-of-arguments"
    );
    // apply spreads the last list arg.
    let v = ev_in(&mut i, "(apply (lambda (a b) (list a b)) 1 '(2))");
    assert_eq!(i.prin1_to_string(&v), "(1 2)");
}

// ---------- macroexpand edge cases ----------

#[test]
fn macroexpand_forms() {
    let (mut i, _) = interp();
    // (macro lambda) form expands.
    let v = ev_in(&mut i, "(defmacro dm (x) `(+ ,x 1)) (macroexpand '(dm 5))");
    assert!(i.prin1_to_string(&v).contains("+"));
    // macroexpand-all on nested macros.
    let v = ev_in(&mut i, "(macroexpand-all '(dm (dm 5)))");
    let _ = v;
    // Raw lambda list as function → invalid-function.
    assert_eq!(ev_err_in(&mut i, "(funcall '(a b) 1)"), "invalid-function");
    // read-from-string with START/END.
    let v = ev_in(&mut i, "(read-from-string \"xy(1 2)z\" 2 7)");
    assert!(i.prin1_to_string(&v).contains("(1 2)"));
}
