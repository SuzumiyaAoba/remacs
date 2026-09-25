//! remacs — an Emacs-compatible editor written in Rust.
//!
//! Architecture: a faithful Emacs *Lisp runtime* (values, reader,
//! evaluator, builtins) layered over a gap-buffer text core, an
//! editor layer (keymaps, command loop, windows, minibuffer), and a
//! terminal front-end that never blocks the UI thread.

pub mod buffer;
pub mod editor;
pub mod gui;
pub mod lisp;
pub mod term;

#[cfg(test)]
mod coverage_tests {
    //! Mirror of tests/probe_all.rs: the unit-test binary carries its
    //! own instrumented copy of this crate, so the probe suite must run
    //! here too for coverage to count.
    use crate::lisp::Interp;

    // Probes that spawn child/pipe/network/serial processes compete for
    // ptys and ports; serialize them so a parallel burst can't exhaust
    // the system.
    static PROBE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    macro_rules! probe {
        ($name:ident, $file:literal) => {
            #[test]
            fn $name() {
                let _lock = PROBE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                let mut i = Interp::new();
                let src = include_str!(concat!("../tests/", $file));
                match i.eval_str(src) {
                    Ok(_) => {}
                    Err(f) => {
                        // Bisect to the failing top-level form so
                        // flaky failures identify themselves.
                        let chars: std::rc::Rc<Vec<char>> = std::rc::Rc::new(src.chars().collect());
                        let mut pos = 0usize;
                        let mut report = String::new();
                        loop {
                            let next = {
                                let mut r =
                                    crate::lisp::reader::Reader::with_chars(&mut i, chars.clone());
                                r.set_position(pos);
                                match r.read() {
                                    Ok(Some(_)) => r.position(),
                                    _ => break,
                                }
                            };
                            let form_src = &src[pos..next];
                            let form = {
                                let mut r = crate::lisp::reader::Reader::new(&mut i, form_src);
                                match r.read() {
                                    Ok(Some(v)) => v,
                                    _ => break,
                                }
                            };
                            match i.eval(&form) {
                                Err(e) => {
                                    report = format!(" at byte {pos}: {form_src} => {e:?}");
                                    break;
                                }
                                Ok(_) => pos = next,
                            }
                        }
                        panic!("probe {} failed: {:?}{}", $file, f, report);
                    }
                }
            }
        };
    }

    probe!(u_probe_all, "probe_all.el");
    probe!(u_probe_deep, "probe_deep.el");
    probe!(u_probe_edge, "probe_edge.el");
    probe!(u_probe_runtime, "probe_runtime.el");
    probe!(u_probe_editor, "probe_editor.el");
    probe!(u_probe_buffer2, "probe_buffer2.el");
    probe!(u_probe_lisp2, "probe_lisp2.el");
    probe!(u_probe_misc2, "probe_misc2.el");
    probe!(u_probe_misc3, "probe_misc3.el");
    probe!(u_probe_printread, "probe_printread.el");
    probe!(u_probe_eval2, "probe_eval2.el");
    probe!(u_probe_editor2, "probe_editor2.el");
    probe!(u_probe_buffer3, "probe_buffer3.el");
    probe!(u_probe_buffer4, "probe_buffer4.el");
    probe!(u_probe_buffer5, "probe_buffer5.el");
    probe!(u_probe_editor4, "probe_editor4.el");
    probe!(u_probe_editor5, "probe_editor5.el");
    probe!(u_probe_editor3, "probe_editor3.el");
    probe!(u_probe_editor6, "probe_editor6.el");
    probe!(u_probe_gap, "probe_gap.el");
    probe!(u_probe_lisp3, "probe_lisp3.el");
    probe!(u_probe_misc4, "probe_misc4.el");
    probe!(u_probe_eval3, "probe_eval3.el");
    probe!(u_probe_eval4, "probe_eval4.el");
    probe!(u_probe_eval5, "probe_eval5.el");
    probe!(u_probe_eval6, "probe_eval6.el");
    probe!(u_probe_eval7, "probe_eval7.el");
    probe!(u_probe_eval8, "probe_eval8.el");
    probe!(u_probe_eval9, "probe_eval9.el");
    probe!(u_probe_eval10, "probe_eval10.el");
    probe!(u_probe_eval11, "probe_eval11.el");
    probe!(u_probe_eval12, "probe_eval12.el");
    probe!(u_probe_eval13, "probe_eval13.el");
    probe!(u_probe_format, "probe_format.el");
    probe!(u_probe_registers, "probe_registers.el");
    probe!(u_probe_rect, "probe_rect.el");
    probe!(u_probe_sort, "probe_sort.el");
    probe!(u_probe_eval14, "probe_eval14.el");
    probe!(u_probe_paragraphs, "probe_paragraphs.el");
    probe!(u_probe_fillcomment, "probe_fillcomment.el");

    /// `command_args` pre-supplies arguments to every prompting
    /// interactive-spec code — the path used when a command is replayed
    /// with known args.
    #[test]
    fn u_interactive_spec_command_args() {
        let mut i = Interp::new();
        i.command_args = vec![crate::lisp::Value::Int(42)];
        for code in [
            "s", "B", "b", "F", "f", "D", "z", "Z", "a", "C", "S", "v", "k", "K", "c", "e", "x",
            "X", "n", "N",
        ] {
            i.eval_str(&format!(
                "(progn (defun g (x) (interactive \"{}In: \") x)
                        (call-interactively 'g) (fmakunbound 'g))",
                code
            ))
            .unwrap();
        }
    }

    /// With a minibuffer reader installed, the prompting spec codes read
    /// their answers through it — mirrors the canned-reader coverage in
    /// tests/interactive.rs but lands inside the instrumented libtest.
    #[test]
    fn u_interactive_spec_reader() {
        use crate::lisp::MinibufInput;
        use std::cell::RefCell;
        use std::collections::VecDeque;
        use std::rc::Rc;
        let mut i = Interp::new();
        let answers = RefCell::new(VecDeque::from([
            MinibufInput::Text("5".into()),       // n
            MinibufInput::Text("str".into()),     // s
            MinibufInput::Text("".into()),        // b → current buffer
            MinibufInput::Text("mksym".into()),   // a → symbol
            MinibufInput::Text("seq".into()),     // k
            MinibufInput::Text("(+ 1 2)".into()), // x → evals
            MinibufInput::Text("(+ 1 2)".into()), // X → evals+prints
            MinibufInput::Key(65),                // c → char code
            MinibufInput::Text("txt".into()),     // c → Text arm
        ]));
        i.minibuf_reader = Some(Rc::new(move |_, _, _| {
            answers
                .borrow_mut()
                .pop_front()
                .ok_or(crate::lisp::Flow::Quit)
        }));
        for code in ["n", "s", "b", "a", "k", "x", "X", "c", "c"] {
            i.eval_str(&format!(
                "(progn (defun g (x) (interactive \"{}In: \") x)
                        (call-interactively 'g) (fmakunbound 'g))",
                code
            ))
            .unwrap();
        }
    }

    #[test]
    fn u_output_sinks() {
        use crate::lisp::OutputSink;
        use std::cell::RefCell;
        use std::rc::Rc;

        // Buffer sink captures princ and message output.
        let mut i = Interp::new();
        i.noninteractive = false;
        let sink = Rc::new(RefCell::new(String::new()));
        i.output = Some(OutputSink::Buffer(sink.clone()));
        i.eval_str("(princ \"into-sink\")").unwrap();
        i.eval_str("(get-buffer-create \" *Messages*\")").unwrap();
        i.message("logged-msg");
        let got = sink.borrow().clone();
        assert!(got.contains("into-sink"), "{}", got);
        assert!(got.contains("logged-msg"), "{}", got);
        assert_eq!(i.echo_message, "logged-msg");

        // Function sink is invoked with each printed string.
        let mut i2 = Interp::new();
        i2.eval_str("(setq seen \"\")").unwrap();
        i2.eval_str("(defun collect-s (s) (setq seen (concat seen s)))")
            .unwrap();
        let sid = i2.intern("collect-s");
        let f = i2.symbol_function(sid);
        i2.output = Some(OutputSink::Function(f));
        i2.eval_str("(princ \"via-fn\")").unwrap();
        let vsym = i2.intern("seen");
        let seen = i2.symbol_value(vsym);
        assert_eq!(i2.princ_to_string(&seen), "via-fn");

        // Stdout sink prints (can't capture; just exercise the path).
        let mut i3 = Interp::new();
        i3.output = Some(OutputSink::Stdout);
        i3.eval_str("(princ \"to-stdout\")").unwrap();
    }

    #[test]
    fn u_uptime_text() {
        use crate::lisp::builtins::misc::uptime_text;
        assert_eq!(uptime_text(0), "0 seconds");
        assert_eq!(uptime_text(1), "1 second");
        assert_eq!(uptime_text(59), "59 seconds");
        assert_eq!(uptime_text(86400), "1 day, 00:00:00");
        assert_eq!(uptime_text(90061), "1 day, 01:01:01");
        assert_eq!(
            uptime_text(2 * 86400 + 3 * 3600 + 4 * 60 + 5),
            "2 days, 03:04:05"
        );
    }
}
