use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// In-memory frontend: exercise the shared loop without terminal I/O.
struct TestFrontend {
    width: usize,
    height: usize,
    out: Rc<RefCell<Vec<u8>>>,
    pending: Vec<i128>,
}
impl KeyIo for TestFrontend {
    fn poll_key(&mut self, _: Duration) -> io::Result<Option<i128>> {
        Ok(self.pending.pop())
    }
    fn unread(&mut self, key: i128) {
        self.pending.push(key);
    }
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let grid = render_grid(i, self.width, self.height);
        for row in grid.rows {
            self.out.borrow_mut().extend_from_slice(row.as_bytes());
        }
        Ok(())
    }
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        self.out.borrow_mut().extend_from_slice(text.as_bytes());
        Ok(())
    }
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}
fn test_term(width: usize, height: usize) -> (TestFrontend, Rc<RefCell<Vec<u8>>>) {
    let out = Rc::new(RefCell::new(Vec::new()));
    (
        TestFrontend {
            width,
            height,
            out: out.clone(),
            pending: Vec::new(),
        },
        out,
    )
}
fn interp_with_frame(text: &str) -> crate::lisp::Interp {
    let mut i = crate::lisp::Interp::new();
    let buf_id = i.create_buffer("test-buf");
    {
        let b = i.buffers.get(buf_id).unwrap();
        b.borrow_mut().insert(text);
        b.borrow_mut().set_point(0);
    }
    let mb_id = i.create_buffer(" *Minibuf-0*");
    let frame = crate::editor::Frame::new_tty(buf_id, mb_id, 80, 25);
    i.selected_frame = Some(frame.clone());
    i.frames.push(frame);
    i.set_current_buffer(buf_id);
    i
}

#[test]
fn isearch_extends_match_in_place_and_overlays_paint() {
    let mut i = interp_with_frame("xxx yyy xxx\n");
    let mut st = Isearch::new(&i, false, false);
    for c in "xxx".chars() {
        st.step(&mut i, c as i128);
    }
    // GNU semantics: each char re-searches from the match's
    // START (`isearch-other-end'), so the span grows in place.
    assert_eq!(st.spans, vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
    assert_eq!(st.positions, vec![0, 1, 2, 3]);
    st.refresh_overlays(&mut i);
    let grid = render_grid(&i, 80, 24);
    // Current match 0..3 inverse (isearch), the second xxx at
    // 8..11 lazy-highlighted (underline).
    assert!(
        grid.attrs[0][..3].iter().all(|c| c.inverse),
        "{:?}",
        &grid.attrs[0][..4]
    );
    assert!(
        grid.attrs[0][8..11].iter().all(|c| c.underline),
        "{:?}",
        &grid.attrs[0][8..11]
    );
    assert!(!grid.attrs[0][4].inverse && !grid.attrs[0][4].underline);
}

/// Interp whose current buffer holds `text`, point at start.
fn isearch_interp(text: &str) -> crate::lisp::Interp {
    let mut i = crate::lisp::Interp::new();
    let buf_id = i.create_buffer("s");
    i.set_current_buffer(buf_id);
    i.buffers.get(buf_id).unwrap().borrow_mut().insert(text);
    i.buffers.get(buf_id).unwrap().borrow_mut().set_point(0);
    i
}

fn point(i: &Interp) -> usize {
    i.current_buffer_ref().unwrap().borrow().point()
}

fn mouse_position(i: &Interp, character: usize) -> MousePosition {
    let buffer_id = i.current_buffer_ref().unwrap().borrow().id;
    MousePosition {
        buffer_id,
        buffer_position: character,
        utf16_offset: character,
        x: character as i128,
        y: 0,
        row: 0,
        column: character,
    }
}

fn dispatch_test_mouse(i: &mut Interp, mouse: MouseInput) {
    let (frontend, _) = test_term(80, 25);
    let term = Rc::new(RefCell::new(frontend));
    let mut keys = Vec::new();
    let mut arg_mode = false;
    dispatch_mouse(&term, i, mouse, &mut keys, &mut arg_mode).unwrap();
}

#[test]
fn mouse_click_moves_point_to_hit_tested_position() {
    let mut i = interp_with_frame("alpha beta\n");
    i.eval_str("(require 'mouse) (setq mouse-1-click-follows-link nil)")
        .unwrap();
    let position = mouse_position(&i, 7);
    dispatch_test_mouse(
        &mut i,
        MouseInput {
            button: MouseButton::Left,
            click_count: 1,
            modifiers: 0,
            start: position,
            end: position,
        },
    );
    assert_eq!(point(&i), 7);
}

#[test]
fn double_and_triple_mouse_clicks_select_word_and_line() {
    let mut double = interp_with_frame("alpha beta\nsecond line\n");
    double
        .eval_str("(require 'mouse) (setq mouse-1-click-follows-link nil)")
        .unwrap();
    let position = mouse_position(&double, 2);
    dispatch_test_mouse(
        &mut double,
        MouseInput {
            button: MouseButton::Left,
            click_count: 2,
            modifiers: 0,
            start: position,
            end: position,
        },
    );
    let selected = double
        .eval_str("(buffer-substring-no-properties (region-beginning) (region-end))")
        .unwrap();
    assert_eq!(double.prin1_to_string(&selected), "\"alpha\"");

    let mut triple = interp_with_frame("alpha beta\nsecond line\n");
    triple
        .eval_str("(require 'mouse) (setq mouse-1-click-follows-link nil)")
        .unwrap();
    let position = mouse_position(&triple, 2);
    dispatch_test_mouse(
        &mut triple,
        MouseInput {
            button: MouseButton::Left,
            click_count: 3,
            modifiers: 0,
            start: position,
            end: position,
        },
    );
    let selected = triple
        .eval_str("(buffer-substring-no-properties (region-beginning) (region-end))")
        .unwrap();
    assert!(triple.prin1_to_string(&selected).starts_with("\"alpha beta"));
    assert!(!triple.prin1_to_string(&selected).contains("second"));

    let mut drag = interp_with_frame("alpha beta\n");
    drag.eval_str("(require 'mouse)").unwrap();
    let start = mouse_position(&drag, 1);
    let end = mouse_position(&drag, 7);
    dispatch_test_mouse(
        &mut drag,
        MouseInput {
            button: MouseButton::Left,
            click_count: 1,
            modifiers: 0,
            start,
            end,
        },
    );
    let selected = drag
        .eval_str("(buffer-substring-no-properties (region-beginning) (region-end))")
        .unwrap();
    assert_eq!(drag.prin1_to_string(&selected), "\"lpha b\"");
}

#[test]
fn mouse_1_link_click_translates_to_mouse_2_at_the_same_position() {
    let mut i = interp_with_frame("link body\n");
    i.eval_str(
        r#"
        (require 'mouse)
        (setq mouse-1-click-follows-link t qa-mouse-link-followed nil)
        (put-text-property 1 5 'follow-link t)
        (define-key global-map [mouse-2]
          (lambda (event) (interactive "e") (setq qa-mouse-link-followed 'global)))
        (let ((map (make-sparse-keymap)))
          (define-key map [mouse-2]
            (lambda (event) (interactive "e") (setq qa-mouse-link-followed 'local)))
          (put-text-property 1 5 'keymap map))
    "#,
    )
    .unwrap();
    let position = mouse_position(&i, 2);
    dispatch_mouse_down(
        &mut i,
        MouseInput {
            button: MouseButton::Left,
            click_count: 1,
            modifiers: 0,
            start: position,
            end: position,
        },
    );
    dispatch_test_mouse(
        &mut i,
        MouseInput {
            button: MouseButton::Left,
            click_count: 1,
            modifiers: 0,
            start: position,
            end: position,
        },
    );
    let followed = i.eval_str("qa-mouse-link-followed").unwrap();
    assert_eq!(i.prin1_to_string(&followed), "local");
}

#[test]
fn isearch_chars_move_point_to_match() {
    let mut i = isearch_interp("one two one\n");
    let mut st = Isearch::new(&i, false, false);
    assert!(matches!(
        st.step(&mut i, 't' as i128),
        IsearchAction::Continue
    ));
    assert!(matches!(
        st.step(&mut i, 'w' as i128),
        IsearchAction::Continue
    ));
    assert!(matches!(
        st.step(&mut i, 'o' as i128),
        IsearchAction::Continue
    ));
    // "two" ends at index 7.
    assert_eq!(point(&i), 7);
    assert!(!st.failing);
    assert!(st.label().contains("I-search: two"));
}

#[test]
fn isearch_cs_repeats_and_del_unwinds() {
    let mut i = isearch_interp("aa aa\n");
    let mut st = Isearch::new(&i, false, false);
    st.step(&mut i, 'a' as i128);
    let first = point(&i);
    st.step(&mut i, 19); // C-s → next match
    assert!(point(&i) > first);
    st.step(&mut i, 19); // no third match → failing
    assert!(st.failing);
    st.step(&mut i, 127); // DEL unwinds
    assert_eq!(point(&i), first);
}

#[test]
fn isearch_cg_restores_entry() {
    let mut i = isearch_interp("xx yy\n");
    let mut st = Isearch::new(&i, false, false);
    st.step(&mut i, 'y' as i128);
    assert!(point(&i) > 0);
    assert!(matches!(st.step(&mut i, 7), IsearchAction::Abort));
    assert_eq!(point(&i), 0);
}

#[test]
fn isearch_ret_sets_mark() {
    let mut i = isearch_interp("xx yy\n");
    let mut st = Isearch::new(&i, false, false);
    st.step(&mut i, 'y' as i128);
    assert!(matches!(st.step(&mut i, 13), IsearchAction::Done));
    let b = i.current_buffer_ref().unwrap();
    let bb = b.borrow();
    assert_eq!(bb.mark_position(), Some(0));
    assert!(bb.point() > 0);
}

#[test]
fn isearch_backward_searches_backward() {
    let mut i = isearch_interp("aa bb aa\n");
    // Point at end.
    i.current_buffer_ref().unwrap().borrow_mut().set_point(8);
    let mut st = Isearch::new(&i, true, false);
    st.step(&mut i, 'a' as i128);
    // Backward match puts point at match start (index 7).
    assert_eq!(point(&i), 7);
    assert!(st.label().contains("backward"));
}

#[test]
fn isearch_other_key_redispatches() {
    let mut i = isearch_interp("text\n");
    let mut st = Isearch::new(&i, false, false);
    st.step(&mut i, 'x' as i128);
    match st.step(&mut i, 24) {
        IsearchAction::ReDispatch(k) => assert_eq!(k, 24),
        _ => panic!("C-x should re-dispatch"),
    }
}

#[test]
fn isearch_failing_then_empty_query() {
    let mut i = isearch_interp("abc\n");
    let mut st = Isearch::new(&i, false, false);
    st.step(&mut i, 'z' as i128);
    st.step(&mut i, 'z' as i128);
    assert!(st.failing);
    assert!(st.label().starts_with("Failing"));
    st.step(&mut i, 127); // DEL pops one char
    st.step(&mut i, 127);
    assert_eq!(st.query, "");
}

// ---------- key translation ----------

#[test]
fn modified_named_keys_round_trip_without_hashing_the_prefix() {
    use crate::editor::{event_code_for, key_name_for};
    let left = event_code_for("left") | CHAR_CTL | CHAR_META;
    assert_eq!(event_code_for("M-C-left"), left);
    assert_eq!(event_code_for("C-M-left"), left);
    assert_eq!(key_name_for(left).as_deref(), Some("C-M-left"));
    assert_eq!(crate::editor::describe_key(left), "C-M-<left>");
    // These names have identical initial hash slots. Modifier handling
    // must preserve the existing collision resolution for base names.
    let dp = event_code_for("dp");
    let f2 = event_code_for("f2");
    assert_ne!(dp, f2);
    assert_eq!(event_code_for("C-dp"), dp | CHAR_CTL);
    assert_eq!(event_code_for("C-f2"), f2 | CHAR_CTL);
    assert_eq!(key_name_for(dp | CHAR_CTL).as_deref(), Some("C-dp"));
    assert_eq!(key_name_for(f2 | CHAR_CTL).as_deref(), Some("C-f2"));
}

// ---------- input loops ----------

/// An Interp whose current buffer is a minibuffer that already
/// holds `prompt` (what `minibuf_read' leaves for the reader):
/// the prompt, the local map, the `exit' catch, and the prompt
/// stack entry `minibuf_contents' reads.
fn interp_with_minibuffer(prompt: &str) -> crate::lisp::Interp {
    let mut i = crate::lisp::Interp::new();
    let mb = i.create_buffer(" *Minibuf-1*");
    let map = i
        .intern_soft("minibuffer-local-map")
        .map(|id| i.symbol_value(id))
        .unwrap_or(Value::Nil);
    let km = i.q("local-keymap");
    {
        let b = i.buffers.get(mb).unwrap();
        let mut bb = b.borrow_mut();
        bb.insert(prompt);
        bb.locals.insert(km, map);
    }
    i.set_current_buffer(mb);
    i.minibuf_prompts.push(prompt.into());
    i.minibuf_level = 1;
    i.minibuf_catching_exit = true;
    let exit_sym = Value::Sym(i.q("exit"));
    i.catch_tags.push(exit_sym);
    i
}

#[test]
fn minibuf_loop_reads_text() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    for k in [b'a' as i128, b'b' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    // pending is a stack — reverse order.
    term.borrow_mut().pending.reverse();
    let mut i = interp_with_minibuffer("P: ");
    match minibuf_loop(&term, &mut i, &crate::lisp::text::EmacsText::from("P: "), false) {
        Ok(crate::lisp::eval::MinibufInput::Text(s)) => assert_eq!(s, "ab"),
        other => panic!("expected Text, got {:?}", other.map(|_| ())),
    }
    // The typed text lives in the minibuffer buffer.
    let b = i.current_buffer_ref().unwrap();
    assert_eq!(b.borrow().text.text(), "P: ab");
}

#[test]
fn minibuf_loop_del_and_cg() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    // type 'x', DEL, then C-g abort.
    for k in [b'x' as i128, 127, 7] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = interp_with_minibuffer("");
    // GNU: `abort-minibuffers' throws `exit' with a function that
    // signals `minibuffer-quit' after unwind.
    match minibuf_loop(&term, &mut i, &crate::lisp::text::EmacsText::from(""), false) {
        Ok(crate::lisp::eval::MinibufInput::Call(f)) => {
            let r = i.apply(&f, vec![]);
            assert!(
                matches!(r, Err(crate::lisp::Flow::Signal(..))),
                "expected minibuffer-quit signal, got {:?}",
                r.map(|_| ())
            );
        }
        other => panic!("expected Call, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn minibuf_loop_icomplete_exhibits() {
    // GNU icomplete: `icomplete-minibuffer-setup' runs via
    // `minibuffer-setup-hook' inside `minibuf_read', installing a
    // buffer-local `post-command-hook' that exhibits completion
    // prospects through an overlay `after-string'.
    let (t, out) = test_term(40, 5);
    let term = Rc::new(RefCell::new(t));
    for k in [b'a' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = crate::lisp::Interp::new();
    i.eval_str("(require 'icomplete) (icomplete-mode 1)")
        .unwrap();
    // `completing-read''s dynamic context (the specbind f_completing_read
    // installs around minibuf_read).
    let mark = i.specbind_depth();
    let mct = i.q("minibuffer-completion-table");
    let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
    let _ = i.specbind(mct, table);
    let t2 = term.clone();
    i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
        minibuf_loop(&t2, interp, prompt, single)
    }));
    let r = i.minibuf_read("Pick: ", Default::default());
    let _ = i.unbind_to(mark);
    assert_eq!(r.unwrap(), "a");
    let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
    // GNU renders `a[lp]{ha | ine}' for input `a' against
    // (alpha alpine beta): [lp] is the common completion,
    // {ha | ine} the remaining prospects.
    assert!(
        drawn.contains("a[lp]{ha | ine}"),
        "expected prospects in echo, got: {drawn:?}"
    );
}

#[test]
fn minibuf_loop_fido_ret() {
    // fido-mode: RET runs `icomplete-fido-ret' →
    // `icomplete-force-complete-and-exit' → completes to the
    // first flex-sorted match and exits with it.
    let (t, out) = test_term(40, 5);
    let term = Rc::new(RefCell::new(t));
    for k in [b'a' as i128, b'p' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = crate::lisp::Interp::new();
    i.eval_str("(require 'icomplete) (fido-mode 1)").unwrap();
    let mark = i.specbind_depth();
    let mct = i.q("minibuffer-completion-table");
    let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
    let _ = i.specbind(mct, table);
    let t2 = term.clone();
    i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
        minibuf_loop(&t2, interp, prompt, single)
    }));
    let r = i.minibuf_read("Pick: ", Default::default());
    let _ = i.unbind_to(mark);
    let got = match r {
        Ok(v) => v,
        Err(f) => {
            let d = match &f {
                crate::lisp::error::Flow::Signal(signal) => format!(
                    "signal {} {}",
                    i.prin1_to_string(&signal.symbol()),
                    i.prin1_to_string(&signal.data())
                ),
                other => format!("{other:?}"),
            };
            let msgs = i
                .buffers
                .by_name("*Messages*")
                .and_then(|id| i.buffers.get(id))
                .map(|b| {
                    let t = b.borrow().text.text();
                    t.chars()
                        .rev()
                        .take(800)
                        .collect::<crate::lisp::text::EmacsText>()
                        .chars()
                        .rev()
                        .collect::<crate::lisp::text::EmacsText>()
                })
                .unwrap_or_default();
            panic!(
                "minibuf_read failed: {d}\necho: {}\nmessages-tail: {msgs}",
                String::from_utf8_lossy(&out.borrow())
            );
        }
    };
    assert!(
        got == "alpha" || got == "alpine",
        "expected flex match, got: {got:?}"
    );
    let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
    // fido shows full prospect names: `ap{alpha | alpine}'.
    assert!(drawn.contains("ap{alpha | alpine}"), "echo: {drawn:?}");
}

#[test]
fn minibuf_loop_icomplete_vertical() {
    // icomplete-vertical-mode renders prospects one-per-line via
    // `icomplete--render-vertical' (exercises the loop rewrites).
    let (t, out) = test_term(40, 8);
    let term = Rc::new(RefCell::new(t));
    for k in [b'a' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = crate::lisp::Interp::new();
    i.eval_str("(require 'icomplete) (icomplete-mode 1) (icomplete-vertical-mode 1)")
        .unwrap();
    let mark = i.specbind_depth();
    let mct = i.q("minibuffer-completion-table");
    let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
    let _ = i.specbind(mct, table);
    let t2 = term.clone();
    i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
        minibuf_loop(&t2, interp, prompt, single)
    }));
    let r = i.minibuf_read("Pick: ", Default::default());
    let _ = i.unbind_to(mark);
    assert_eq!(r.unwrap(), "a");
    let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
    // Vertical layout: each prospect on its own line.
    assert!(
        drawn.contains("alpha") || drawn.contains("alpine"),
        "echo: {drawn:?}"
    );
}

#[test]
fn minibuf_loop_mb_depth_nested() {
    // GNU mb-depth: `minibuffer-depth-setup' (on
    // `minibuffer-setup-hook') puts a `[N] ' `before-string'
    // overlay on the prompt when `minibuffer-depth' > 1.
    let (t, _out) = test_term(30, 5);
    let term = Rc::new(RefCell::new(t));
    for k in [b'a' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = crate::lisp::Interp::new();
    i.eval_str(
        "(require 'mb-depth)
             (minibuffer-depth-indicate-mode 1)
             (defvar mb-probe-depth nil)
             (defvar mb-probe-ind nil)
             ;; Record depth + the indicator overlay inside the
             ;; nested read (appended so it runs after
             ;; `minibuffer-depth-setup').
             (add-hook 'minibuffer-setup-hook
                       (lambda ()
                         (setq mb-probe-depth (minibuffer-depth))
                         (dolist (ov (overlays-in (point-min) (point-max)))
                           (let ((s (overlay-get ov 'before-string)))
                             (when s (setq mb-probe-ind s)))))
                       t)
             ;; One-shot: fire a nested read from inside the outer
             ;; read's command loop (GNU reaches this via e.g. M-x
             ;; in the minibuffer).
             (defun mb-probe-cmd ()
               (remove-hook 'post-command-hook #'mb-probe-cmd)
               (let ((enable-recursive-minibuffers t))
                 (read-string \"L2: \")))
             (add-hook 'post-command-hook #'mb-probe-cmd)",
    )
    .unwrap();
    let t2 = term.clone();
    i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
        // Nested reads (level ≥ 2) get canned input; the outer
        // read (level 1) uses the real command loop.
        if interp.minibuf_level > 1 {
            return Ok(crate::lisp::eval::MinibufInput::Text("inner".into()));
        }
        minibuf_loop(&t2, interp, prompt, single)
    }));
    let r = i.minibuf_read("L1: ", Default::default());
    assert_eq!(r.unwrap(), "a");
    // The nested read ran at depth 2 with the depth indicator.
    let d = i.eval_str("mb-probe-depth").unwrap();
    assert!(matches!(&d, crate::lisp::Value::Int(n) if *n == 2));
    let ind = i.eval_str("mb-probe-ind").unwrap();
    let s = match &ind {
        crate::lisp::Value::Str(s) => s.borrow().clone(),
        other => panic!("expected indicator string, got {other:?}"),
    };
    assert_eq!(s, "[2] ");
}

#[test]
fn minibuf_loop_single_key_mode() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    term.borrow_mut().unread(b'y' as i128);
    let mut i = crate::lisp::Interp::new();
    match minibuf_loop(&term, &mut i, &crate::lisp::text::EmacsText::from("y/n "), true) {
        Ok(crate::lisp::eval::MinibufInput::Key(k)) => assert_eq!(k, b'y' as i128),
        _ => panic!("expected Key"),
    }
}

#[test]
fn isearch_loop_end_to_end() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    // Type 'y', then RET to finish.
    for k in [b'y' as i128, 13] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = interp_with_frame("xx yy\n");
    isearch_loop(&term, &mut i, false, false).unwrap();
    let b = i.current_buffer_ref().unwrap();
    assert_eq!(b.borrow().point(), 4); // after the first "y" match
}

#[test]
fn isearch_loop_abort_restores() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    for k in [b'y' as i128, 7] {
        term.borrow_mut().unread(k);
    }
    term.borrow_mut().pending.reverse();
    let mut i = interp_with_frame("xx yy\n");
    isearch_loop(&term, &mut i, false, false).unwrap();
    assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), 0);
}

#[test]
fn nonblocking_sleep_short() {
    let mut i = crate::lisp::Interp::new();
    nonblocking_sleep(&mut i, Duration::from_millis(1)).unwrap();
}

#[test]
fn dispatch_inserts_undo_boundaries_per_command() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    let mut i = interp_with_frame("");
    let mut keys = Vec::new();
    let mut arg_mode = false;
    // Each self-insert char is one command → one undo boundary.
    dispatch_key(&term, &mut i, b'a' as i128, &mut keys, &mut arg_mode).unwrap();
    dispatch_key(&term, &mut i, b'b' as i128, &mut keys, &mut arg_mode).unwrap();
    let b = i.current_buffer_ref().unwrap();
    assert_eq!(b.borrow().text.text(), "ab");
    // C-/ runs `undo' → removes the last command's "b" insertion.
    dispatch_key(&term, &mut i, 31, &mut keys, &mut arg_mode).unwrap();
    assert_eq!(b.borrow().text.text(), "a");
}

#[test]
fn dispatch_self_insert_expands_abbrev() {
    let (t, _out) = test_term(20, 5);
    let term = Rc::new(RefCell::new(t));
    let mut i = interp_with_frame("");
    i.eval_str("(define-abbrev global-abbrev-table \"tq\" \"quick\") (abbrev-mode 1)")
        .unwrap();
    let mut keys = Vec::new();
    let mut arg_mode = false;
    dispatch_key(&term, &mut i, b't' as i128, &mut keys, &mut arg_mode).unwrap();
    dispatch_key(&term, &mut i, b'q' as i128, &mut keys, &mut arg_mode).unwrap();
    dispatch_key(&term, &mut i, b' ' as i128, &mut keys, &mut arg_mode).unwrap();
    let b = i.current_buffer_ref().unwrap();
    assert_eq!(b.borrow().text.text(), "quick ");
    // With abbrev-mode off, no expansion happens.
    i.eval_str("(abbrev-mode 0)").unwrap();
    let mut keys = Vec::new();
    dispatch_key(&term, &mut i, b't' as i128, &mut keys, &mut arg_mode).unwrap();
    dispatch_key(&term, &mut i, b'q' as i128, &mut keys, &mut arg_mode).unwrap();
    dispatch_key(&term, &mut i, b' ' as i128, &mut keys, &mut arg_mode).unwrap();
    assert_eq!(b.borrow().text.text(), "quick tq ");
}

#[test]
fn lookup_routes_to_bindings() {
    let mut i = crate::lisp::Interp::new();
    let seq = |k: i128| Value::vector(vec![Value::Int(k)]);
    // 'a' -> t default -> self-insert-command
    match lookup_command(&mut i, &seq(97)) {
        LookupResult::Command(v) => {
            assert_eq!(i.princ_to_string(&v), "self-insert-command")
        }
        _ => panic!("'a' should resolve to self-insert-command"),
    }
    // C-u -> universal-argument
    match lookup_command(&mut i, &seq(21)) {
        LookupResult::Command(v) => {
            assert_eq!(i.princ_to_string(&v), "universal-argument")
        }
        _ => panic!("C-u should resolve to universal-argument"),
    }
    // C-x alone -> prefix keymap
    assert!(matches!(
        lookup_command(&mut i, &seq(24)),
        LookupResult::Prefix
    ));
    // C-x C-c -> save-buffers-kill-terminal (GNU's binding)
    let two = Value::vector(vec![Value::Int(24), Value::Int(3)]);
    match lookup_command(&mut i, &two) {
        LookupResult::Command(v) => {
            assert_eq!(i.princ_to_string(&v), "save-buffers-kill-terminal")
        }
        _ => panic!("C-x C-c should resolve"),
    }
}
