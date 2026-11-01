use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r###"(progn
  (unless (and (fboundp 'peg-normalize)
               (fboundp 'peg-detect-cycles)
               (fboundp 'define-peg-rule)
               (fboundp 'macroexp-warn-and-return)
               (fboundp 'peg-run))
    (error "PEG source definitions were not loaded"))
  (defun remacs-peg-3016-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-peg-3016-cycle-error-p (expression path)
    (condition-case err
        (progn (peg-detect-cycles expression path) nil)
      (error (string-match-p "Possible left recursion"
                             (error-message-string err)))))

  ;; A nullable left branch must not short-circuit cycle analysis on the right.
  (remacs-peg-3016-assert
   (remacs-peg-3016-cycle-error-p
    '(or (guard nil) (call peg-cycle-probe))
    '(peg-cycle-probe))
   "Nullable left branch hid right-branch left recursion")
  ;; A consuming left branch must still traverse the right branch.
  (remacs-peg-3016-assert
   (remacs-peg-3016-cycle-error-p
    (peg-normalize '(or "x" (call peg-cycle-probe))) '(peg-cycle-probe))
   "Consuming left branch hid right-branch left recursion")

  ;; Nullability remains OR of both branch results.
  (unless (peg-detect-cycles (peg-normalize '(or (guard nil) "x")) nil)
    (error "OR lost nullability from its left branch"))
  (when (peg-detect-cycles
        (peg-normalize '(or "x" (call peg-unknown-rule))) nil)
    (error "Nonnullable OR was reported nullable"))

  ;; Cycle analysis must describe guards without executing their expressions.
  (defvar remacs-peg-3016-guard-ran nil)
  (let ((remacs-peg-3016-guard-ran nil))
    (unless (peg-detect-cycles
             (peg-normalize
              '(or (guard (setq remacs-peg-3016-guard-ran t)) "x")) nil)
      (error "Guard-containing OR lost nullability"))
    (when remacs-peg-3016-guard-ran
      (error "Cycle analysis evaluated a guard expression")))

  ;; Check the real define-peg-rule expansion path.  PEG deliberately reports
  ;; this detector error through macroexp-warn-and-return; turn that one warning
  ;; into an error here so the test proves the actual rule definition saw it.
  (defvar remacs-peg-3016-original-warn nil)
  (defvar remacs-peg-3016-warning nil)
  (defun remacs-peg-3016-warn-as-error (message form &rest args)
    (if (string-match-p "Possible left recursion" message)
        (progn
          (setq remacs-peg-3016-warning message)
          (error "%s" message))
      (apply remacs-peg-3016-original-warn message form args)))
  (setq remacs-peg-3016-original-warn (symbol-function 'macroexp-warn-and-return))
  (let ((remacs-peg-3016-warning nil)
        (compile-error nil))
    (cl-letf (((symbol-function 'macroexp-warn-and-return)
               #'remacs-peg-3016-warn-as-error))
      (setq compile-error
            (condition-case err
                (progn
                  (eval '(define-peg-rule remacs-peg-3016-recursive ()
                           (or (guard nil)
                               (call remacs-peg-3016-recursive))))
                  nil)
              (error (error-message-string err)))))
    (remacs-peg-3016-assert
     (and remacs-peg-3016-warning compile-error
          (string-match-p "Possible left recursion" compile-error)
          (string-match-p "remacs-peg-3016-recursive" compile-error))
     (format "Real rule-definition path missed recursion: warning=%S error=%S"
             remacs-peg-3016-warning compile-error)))

  ;; A valid nullable ordered choice still compiles and matches its right arm.
  (define-peg-rule remacs-peg-3016-valid ()
    (or (guard nil) "a"))
  (let ((matcher (peg remacs-peg-3016-valid)))
    (with-temp-buffer
      (insert "a")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Valid nullable choice did not match"))
      (unless (= (point) (point-max))
        (error "Valid nullable choice stopped at %d" (point)))))

  ;; Existing PEG normalization and failure diagnostics stay stable.
  (let ((normalized (peg-normalize '(+ (and "a" "b")))))
    (unless (equal normalized (peg-normalize normalized))
      (error "PEG normalization is no longer idempotent: %S" normalized)))
  (let ((matcher (peg (+ "a"))))
    (with-temp-buffer
      (insert "b")
      (goto-char (point-min))
      (condition-case err
          (progn
            (peg-run matcher #'peg-signal-failure)
            (error "Failure diagnostic matcher unexpectedly succeeded"))
        (peg-search-failed
         (unless (and (= (cadr err) (point-min))
                      (= (point) (point-min))
                      (equal (nth 2 err) '("a")))
           (error "PEG failure diagnostics changed: %S" err))))))
  t)"###;

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

#[test]
fn peg_or_cycle_detection_visits_both_branches_3016() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let source = root.join("lisp/peg.el").canonicalize().unwrap();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
