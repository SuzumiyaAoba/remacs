use remacs_runtime::lisp::obarray::sym;
use remacs_runtime::lisp::value::EmacsText;
use remacs_runtime::lisp::{Flow, Interp, OutputSink, Value};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;

fn flow_summary(interp: &mut Interp, flow: Flow) -> String {
    match flow {
        Flow::Signal(signal) => {
            let symbol = interp.prin1_to_string(&signal.symbol()).to_unicode_lossy();
            let data = interp.prin1_to_string(&signal.data()).to_unicode_lossy();
            format!("signal {symbol} {data}")
        }
        other => format!("{other:?}"),
    }
}

#[test]
fn qa_seq2_source_suite_uses_remacs_char_table_extensions() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let source_root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
            let framework = source_root.join("tests/qa_framework.el");
            let suite = source_root.join("tests/qa_seq2.el");
            let mut interp = Interp::new_with_noninteractive(true);
            let output = Rc::new(RefCell::new(EmacsText::new()));
            interp.output = Some(OutputSink::Buffer(output.clone()));

            let framework_load =
                interp.eval_str(&format!("(load {:?})", framework.to_string_lossy()));
            if let Err(flow) = framework_load {
                let detail = flow_summary(&mut interp, flow);
                panic!(
                    "qa framework load failed from {}: {detail}",
                    framework.display()
                );
            }
            interp.eval_str("(setq qa--remacs t)").unwrap();
            let remacs = interp.eval_str("qa--remacs").unwrap();
            assert!(remacs.truthy(), "qa framework did not detect Remacs");
            let suite_load = interp.eval_str(&format!("(load {:?})", suite.to_string_lossy()));
            if let Err(flow) = suite_load {
                let detail = flow_summary(&mut interp, flow);
                panic!("qa suite load failed from {}: {detail}", suite.display());
            }

            let summary = output.borrow().to_unicode_lossy();
            assert!(
                summary.contains("checks passed"),
                "qa_seq2 summary: {summary}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn char_table_seq_subsequence_is_fast_and_reports_borrow_errors() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let mut interp = Interp::new();
            let result = interp
                .eval_str(
                    "(progn
                       (require 'seq)
                       (let* ((table (make-char-table 'generic nil))
                              (length (seq-length table))
                              (last (1- length)))
                         (set-char-table-range table 0 'first)
                         (set-char-table-range table last 'last)
                         (let ((reversed (seq-reverse table)))
                           (list (vectorp reversed)
                                 (= (length reversed) length)
                                 (aref reversed 0)
                                 (aref reversed last)
                                 (type-of (seq-take table 2))
                                 (type-of (seq-drop table (- length 2)))
                                 (seq-subseq (make-char-table 'generic 'default) 0 3)
                                 (let* ((parent (make-char-table 'generic 'inherited))
                                        (child (make-char-table 'generic nil)))
                                   (set-char-table-parent child parent)
                                   (set-char-table-range child 1 'local)
                                   (seq-subseq child 0 3))))))",
                )
                .unwrap();
            assert_eq!(
                interp.prin1_to_string(&result).to_unicode_lossy(),
                "(t t last first vector vector [default default default] [inherited local inherited])"
            );

            let symbol = interp.intern("qa-seq-char-table-borrowed");
            let table = interp.eval_str("(make-char-table 'generic nil)").unwrap();
            interp.set_symbol(symbol, table.clone()).unwrap();
            let Value::Record(record) = &table else {
                panic!("make-char-table did not return a record");
            };
            let held = record.borrow_mut();
            let result = catch_unwind(AssertUnwindSafe(|| {
                interp.eval_str("(seq-subseq qa-seq-char-table-borrowed 0 1)")
            }));
            drop(held);

            let result = result.expect("char-table seq-subseq must not panic on a borrowed table");
            assert!(matches!(
                result,
                Err(Flow::Signal(ref signal)) if signal.is_symbol(sym::ERROR)
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}
