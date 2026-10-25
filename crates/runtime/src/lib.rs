//! Emacs Lisp runtime, buffer/editor state, and frontend-neutral execution.
//!
//! Device adapters live in remacs-cli and remacs-gui. This crate owns the
//! evaluator and the Emacs state exposed to Lisp, plus the shared rendering
//! and command-loop protocol used by those adapters.

pub mod buffer;
pub mod editor;
pub mod lisp;
pub mod lifecycle;
pub mod stdio;
pub mod frontend;
pub mod host;

#[cfg(test)]
#[path = "../../../tests/support/probe.rs"]
mod probe_support;

#[cfg(test)]
mod coverage_tests {
    //! Mirror of tests/probe_all.rs: the unit-test binary carries its
    //! own instrumented copy of this crate, so the probe suite must run
    //! here too for coverage to count.
    use crate::lisp::Interp;

    /// Run `body` on a dedicated 64 MiB-stack thread — the evaluator
    /// legitimately recurses several MB deep on the probes, and
    /// libtest's default 2 MiB test threads overflow.
    fn probe_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(f)
            .expect("spawn probe thread")
            .join()
            .expect("probe thread panicked");
    }

    fn eval_probe(file: &str, source: &str) {
        let mut i = Interp::new();
        match i.eval_str(source) {
            Ok(_) => {}
            Err(f) => {
                let flow_desc = match &f {
                    crate::lisp::Flow::Signal(signal) => {
                        let s = signal.symbol();
                        let d = signal.data();
                        format!("signal {} {}", i.prin1_to_string(&s), i.prin1_to_string(&d))
                    }
                    other => format!("{other:?}"),
                };
                panic!("probe {} failed: {}", file, flow_desc);
            }
        }
    }

    macro_rules! probe {
        ($name:ident, $file:literal) => {
            #[test]
            fn $name() {
                crate::probe_support::run(
                    concat!("coverage_tests::", stringify!($name)),
                    $file,
                    || {
                        eval_probe($file, include_str!(concat!("../../../tests/", $file)));
                    },
                );
            }
        };
    }

    #[test]
    #[ignore = "subprocess fixture for u_probe_error_is_not_replayed"]
    fn probe_error_fixture() {
        probe_stack(|| {
            let path = std::env::var("REMACS_PROBE_ERROR_COUNT").unwrap();
            let source = format!(
                "(progn (write-region \"once\\n\" nil {path:?} t (quote silent)) (error \"intentional probe failure\"))"
            );
            eval_probe("error-fixture", &source);
        });
    }

    #[test]
    fn u_probe_error_is_not_replayed() {
        struct Count(std::path::PathBuf);
        impl Drop for Count {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("remacs-probe-count-{}-{nonce}", std::process::id()));
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        let count = Count(path);
        let report = crate::probe_support::run_command(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "coverage_tests::probe_error_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("REMACS_PROBE_ERROR_COUNT", &count.0),
            std::time::Duration::from_secs(120),
        )
        .unwrap();
        assert!(!report.timed_out);
        assert!(!report.status.success());
        assert!(
            report.tail.contains("intentional probe failure"),
            "{}",
            report.tail
        );
        assert_eq!(std::fs::read_to_string(&count.0).unwrap(), "once\n");
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
        probe_stack(|| {
            let mut i = Interp::new();
            i.command_args = vec![crate::lisp::Value::Int(42)];
            for code in [
                "s", "B", "b", "F", "f", "D", "z", "Z", "a", "C", "S", "v", "k", "K", "c", "e",
                "x", "X", "n", "N",
            ] {
                i.eval_str(&format!(
                    "(progn (defun g (x) (interactive \"{}In: \") x)
                        (call-interactively 'g) (fmakunbound 'g))",
                    code
                ))
                .unwrap();
            }
        });
    }

    /// With a minibuffer reader installed, the prompting spec codes read
    /// their answers through it — mirrors the canned-reader coverage in
    /// tests/interactive.rs but lands inside the instrumented libtest.
    #[test]
    fn u_interactive_spec_reader() {
        probe_stack(|| {
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
        });
    }

    #[test]
    fn u_output_sinks() {
        probe_stack(|| {
            use crate::lisp::OutputSink;
            use std::cell::RefCell;
            use std::rc::Rc;

            // Buffer sink captures princ and message output.
            let mut i = Interp::new();
            i.noninteractive = false;
            let sink = Rc::new(RefCell::new(String::new()));
            i.output = Some(OutputSink::Buffer(sink.clone()));
            i.eval_str("(princ \"into-sink\")").unwrap();
            i.message("logged-msg");
            let got = sink.borrow().clone();
            assert!(got.contains("into-sink"), "{}", got);
            // GNU routes `message' to the echo area + the *Messages* log,
            // NOT to the princ/output sink (a *Messages* with leading space
            // is a separate buffer; the real log is `*Messages*').
            assert!(!got.contains("logged-msg"), "{}", got);
            assert_eq!(i.echo_message, "logged-msg");
            let msgs = i
                .eval_str("(with-current-buffer \"*Messages*\" (buffer-string))")
                .unwrap();
            assert!(i.princ_to_string(&msgs).contains("logged-msg\n"));

            // Function sink is invoked with each printed string.
            let mut i2 = Interp::new();
            i2.eval_str("(setq seen \"\")").unwrap();
            i2.eval_str("(defun collect-s (s) (setq seen (concat seen s)))")
                .unwrap();
            let sid = i2.q("collect-s");
            let f = i2.symbol_function(sid);
            i2.output = Some(OutputSink::Function(f));
            i2.eval_str("(princ \"via-fn\")").unwrap();
            let vsym = i2.q("seen");
            let seen = i2.symbol_value(vsym);
            assert_eq!(i2.princ_to_string(&seen), "via-fn");

            // Stdout sink prints (can't capture; just exercise the path).
            let mut i3 = Interp::new();
            i3.output = Some(OutputSink::Stdout);
            i3.eval_str("(princ \"to-stdout\")").unwrap();
        });
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
