use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r#"(progn
  (defvar remacs-3171-zero-timer nil)
  (defvar remacs-3171-zero-calls 0)
  (let ((timer-list nil)
        (timer-idle-list nil)
        (timer-max-repeats 10)
        (remacs-3171-zero-timer nil)
        (remacs-3171-zero-calls 0))
    (let ((timer (run-with-timer 60 0.0 #'ignore)))
      (unless (equal (timer--repeat-delay timer) 0.0)
        (error "Floating zero repeat interval was discarded"))
      (cancel-timer timer))
    (unwind-protect
        (progn
          (setq remacs-3171-zero-timer
                (run-with-timer
                 0 0
                 (lambda ()
                   (setq remacs-3171-zero-calls
                         (1+ remacs-3171-zero-calls))
                   (when (= remacs-3171-zero-calls 3)
                     (cancel-timer remacs-3171-zero-timer)))))
          (unless (and (timerp remacs-3171-zero-timer)
                       (eql (timer--repeat-delay remacs-3171-zero-timer) 0))
            (error "Zero repeat interval was discarded"))
          ;; Each sleep pumps the timer event loop.  A zero-delay repeating
          ;; timer must rearm across pumps, but the callback cancels itself
          ;; after three turns so this regression remains bounded.
          (dotimes (_ 4)
            (sleep-for 0.001))
          (unless (and (= remacs-3171-zero-calls 3)
                       (not (memq remacs-3171-zero-timer timer-list)))
            (error "Zero timer did not rearm and cancel cleanly: %S"
                   remacs-3171-zero-calls))
          ;; TIME=t has no nonzero integral multiple for REPEAT=0; it should
          ;; still produce a zero-repeat timer scheduled from the current time.
          (let ((timer (run-at-time t 0 #'ignore)))
            (unless (and (timerp timer)
                         (eql (timer--repeat-delay timer) 0))
              (error "run-at-time with TIME=t rejected zero repeat"))
            (cancel-timer timer))
          t)
      (when (timerp remacs-3171-zero-timer)
        (cancel-timer remacs-3171-zero-timer))))
  t)"#;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn evaluate(interp: &mut Interp, source: &str) {
    match interp.eval_str(source) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
fn zero_repeat_timer_rearms_once_per_event_pump() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/timer.el")
        .canonicalize()
        .unwrap();
    let mut interp = Interp::new();
    evaluate(&mut interp, CASES);
    evaluate(
        &mut interp,
        &format!(
            "(progn (fmakunbound 'timer-set-time) \
             (fmakunbound 'timer-event-handler) \
             (fmakunbound 'run-at-time) (fmakunbound 'run-with-timer) \
             (load {} nil t t) t)",
            lisp_string(source.to_str().unwrap())
        ),
    );
    evaluate(&mut interp, CASES);
}
