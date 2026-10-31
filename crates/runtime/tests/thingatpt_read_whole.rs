use remacs_runtime::lisp::{Flow, Interp, Value};
use std::path::PathBuf;

const CASES: &str = r#"
(let ((text-quoting-style 'grave))
  (require 'thingatpt)
  (defun remacs-thing-at-point-read-error (string)
    (condition-case err
        (thing-at-point--read-from-whole-string string)
      (error err)))
  (unless (= (thing-at-point--read-from-whole-string "1") 1)
    (error "Single-form input changed"))
  (unless (= (thing-at-point--read-from-whole-string "1 \t\n") 1)
    (error "Trailing ASCII reader layout was rejected"))
  (unless (= (thing-at-point--read-from-whole-string
              (concat "1" (string 0 31 32 160)))
             1)
    (error "Trailing control/NBSP reader layout was rejected"))
  (unless (= (thing-at-point--read-from-whole-string "1 ; trailing comment") 1)
    (error "Trailing semicolon comment was rejected"))
  (unless (= (thing-at-point--read-from-whole-string "1 #! ignored line\n") 1)
    (error "Trailing #! reader comment was rejected"))
  (unless (equal (remacs-thing-at-point-read-error "1 2")
                 '(error "Can't read whole string"))
    (error "A second complete form did not preserve the existing error"))
  (dolist (string '("1 (" "1 (a" "1 \"unterminated"))
    (unless (eq (car-safe (remacs-thing-at-point-read-error string)) 'end-of-file)
      (error "An incomplete trailing form did not preserve end-of-file: %S"
             string)))
  ;; String-reader #@ skips lazy documentation through the unit separator
  ;; or EOF.  After a separator, parsing resumes normally.
  (dolist (string '("1 #@1 text" "1 #@1 text\037" "1 #@"))
    (unless (= (thing-at-point--read-from-whole-string string) 1)
      (error "A #@ skip-to-EOF tail was rejected: %S" string)))
  (unless (eq (car-safe
               (remacs-thing-at-point-read-error "1 #@1 text\037 ("))
              'end-of-file)
    (error "An incomplete form after a #@ delimiter was accepted"))
  (unless (equal (remacs-thing-at-point-read-error "1 #@00")
                 '(error "Can't read whole string"))
    (error "The #@00 nil object was accepted as trailing trivia"))
  (unless (equal
           (remacs-thing-at-point-read-error "1 #@1 text\037 nil")
           '(error "Can't read whole string"))
    (error "A nil form after a #@ delimiter was treated as trivia"))
  ;; A transparent reader prefix must not borrow an appended sentinel.
  (let ((read-circle t))
    (unless (eq (car-safe (remacs-thing-at-point-read-error "1 #1="))
                'end-of-file)
      (error "An incomplete circular-object prefix was accepted")))
  (unless (eq (car-safe (remacs-thing-at-point-read-error "(1")) 'end-of-file)
    (error "The first form's read error changed"))
  t)
"#;

fn eval(interp: &mut Interp, source: &str) {
    interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
}

fn load_source_thingatpt(interp: &mut Interp) {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT")).join("lisp/thingatpt.el");
    let quoted = interp.prin1_to_string(&Value::string(source.to_string_lossy().into_owned()));
    eval(interp, &format!("(load {quoted} nil t)"));
}

#[test]
fn thing_at_point_whole_string_rejects_incomplete_trailing_forms() {
    let mut interp = Interp::new();
    eval(&mut interp, CASES);
    load_source_thingatpt(&mut interp);
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
