use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}

const CASES: &str = r#"(progn
  (unless (and (fboundp 'peg-normalize)
               (fboundp 'peg-translate-exp)
               (fboundp 'peg-run))
    (error "PEG source definitions were not loaded"))
  (defvar remacs-test-peg-3050-events nil)
  (defun remacs-test-peg-3050-nested (depth)
    (if (zerop depth)
        "a"
      (list '+ (remacs-test-peg-3050-nested (1- depth)))))
  (defun remacs-test-peg-3050-matcher (expression)
    (eval `(lambda ()
             ,(peg-translate-exp (peg-normalize expression)))
          t))
  (defun remacs-test-peg-3050-matcher-dynamic (expression)
    (eval `(lambda ()
             ,(peg-translate-exp (peg-normalize expression)))
          nil))
  (defun remacs-test-peg-3050-check-failure (text)
    (let ((matcher (remacs-test-peg-3050-matcher '(+ "a"))))
      (with-temp-buffer
        (insert text)
        (goto-char (point-min))
        (condition-case err
            (progn
              (peg-run matcher #'peg-signal-failure)
              (error "One-or-more unexpectedly matched %S" text))
          (peg-search-failed
           (unless (and (= (cadr err) (point-min))
                        (= (point) (point-min))
                        (equal (nth 2 err) '("a")))
             (error "Wrong PEG failure diagnostics: %S" err))
           t)))))
  ;; `peg-normalize' is public and can be applied to its own normalized tree.
  (let ((normalized (peg-normalize '(+ (and "a" "b")))))
    (unless (equal normalized (peg-normalize normalized))
      (error "PEG normalization is no longer idempotent: %S" normalized)))
  ;; One-or-more repeats the whole normalized sequence when given several PEXs.
  (let ((matcher (remacs-test-peg-3050-matcher '(+ "a" "b"))))
    (with-temp-buffer
      (insert "ababc")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Multi-operand one-or-more did not match"))
      (unless (= (point) (+ (point-min) 4))
        (error "Multi-operand one-or-more stopped at %d" (point)))))
  ;; The generated matcher closure also works when evaluated dynamically.
  (let ((matcher (remacs-test-peg-3050-matcher-dynamic '(+ "a"))))
    (with-temp-buffer
      (insert "aa")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Dynamically evaluated one-or-more failed"))
      (unless (= (point) (point-max))
        (error "Dynamic one-or-more stopped at %d" (point)))))
  ;; + has the nullability of its operand and must still traverse named calls.
  (unless (peg-detect-cycles (peg-normalize '(+ (guard t))) nil)
    (error "One-or-more lost its operand's nullability"))
  (when (peg-detect-cycles (peg-normalize '(+ "a")) nil)
    (error "Consuming one-or-more was treated as nullable"))
  (unless (condition-case err
              (progn
                (peg-detect-cycles
                 (peg-normalize '(+ remacs-test-peg-3050-left-recursive))
                 '(remacs-test-peg-3050-left-recursive))
                nil)
            (error (string-match-p "Possible left recursion"
                                   (error-message-string err))))
    (error "One-or-more hid named left recursion"))
  ;; The issue's depth-8 expression has 35 input characters.  A quadratic
  ;; bound is deliberately loose but rejects its old 132K-character output.
  (let ((depth 0))
    (while (<= depth 8)
      (let* ((expression (remacs-test-peg-3050-nested depth))
             (code (peg-translate-exp (peg-normalize expression)))
             (input-size (length (prin1-to-string expression)))
             (code-size (length (prin1-to-string code)))
             (limit (* 16 (1+ input-size) (1+ input-size))))
        (unless (<= code-size limit)
          (error "PEG expansion at depth %d: %d > %d"
                 depth code-size limit)))
      (setq depth (1+ depth))))
  ;; Deep plus expressions remain executable, and the named-rule path uses
  ;; the same normalization, cycle check, and translator.
  (let ((matcher (remacs-test-peg-3050-matcher
                  (remacs-test-peg-3050-nested 8))))
    (with-temp-buffer
      (insert "aaaa")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Nested one-or-more failed to match"))
      (unless (= (point) (point-max))
        (error "Nested one-or-more stopped at %d" (point)))))
  (define-peg-rule remacs-test-peg-3050-many () (+ "a"))
  (let ((matcher (symbol-function
                  (peg--rule-id 'remacs-test-peg-3050-many))))
    (with-temp-buffer
      (insert "aa")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Named one-or-more rule failed"))
      (unless (= (point) (point-max))
        (error "Named one-or-more stopped at %d" (point)))))
  (define-peg-rule remacs-test-peg-3050-parameterized (matcher)
    (+ (funcall matcher)))
  (let ((rule (symbol-function
               (peg--rule-id 'remacs-test-peg-3050-parameterized)))
        (matcher (peg "a")))
    (let ((peg-matcher (lambda () (funcall rule matcher))))
      (with-temp-buffer
        (insert "aaa")
        (goto-char (point-min))
        (unless (peg-run peg-matcher)
          (error "Parameterized named one-or-more rule failed"))
        (unless (= (point) (point-max))
          (error "Parameterized named rule stopped at %d" (point))))))
  ;; A named function defined with dynamic `eval' must keep its PEG argument
  ;; visible from the generated + matcher closure as well.
  (let ((lexical-binding nil))
    (eval `(defalias 'remacs-test-peg-3050-dynamic-parameter
             (lambda (matcher)
               ,(peg-translate-exp
                 (peg-normalize '(+ (funcall matcher)))))
             nil)
          nil))
  (let ((rule (symbol-function 'remacs-test-peg-3050-dynamic-parameter))
        (matcher (remacs-test-peg-3050-matcher-dynamic "a")))
    (let ((peg-matcher (lambda () (funcall rule matcher))))
      (with-temp-buffer
        (insert "aaa")
        (goto-char (point-min))
        (unless (peg-run peg-matcher)
          (error "Dynamically evaluated parameterized rule failed"))
        (unless (= (point) (point-max))
          (error "Dynamic parameterized rule stopped at %d" (point))))))
  ;; A failed ordered choice must discard its deferred action before trying +.
  (let* ((expression
          '(or
            (and (action (push 'discard remacs-test-peg-3050-events)) "b")
            (+ (and "a"
                    (action (push (point) remacs-test-peg-3050-events))))))
         (matcher (remacs-test-peg-3050-matcher expression))
         (remacs-test-peg-3050-events nil))
    (with-temp-buffer
      (insert "aaa")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "Ordered-choice fallback to + failed"))
      (unless (and (= (point) (point-max))
                   (equal (sort (copy-sequence remacs-test-peg-3050-events) #'<)
                          '(2 3 4)))
        (error "Backtracking changed actions or final point: %S at %d"
               remacs-test-peg-3050-events (point)))))
  ;; A repeated operand that consumes a prefix and then fails must roll back
  ;; both its cursor and its deferred actions.
  (let ((remacs-test-peg-3050-events nil)
        (matcher
         (remacs-test-peg-3050-matcher
          '(+ (and (action (push 'committed remacs-test-peg-3050-events))
                   "a" "b")))))
    (with-temp-buffer
      (insert "abac")
      (goto-char (point-min))
      (unless (peg-run matcher)
        (error "One-or-more lost its successful first iteration"))
      (unless (and (= (point) (+ (point-min) 2))
                   (equal remacs-test-peg-3050-events '(committed)))
        (error "Failed repeat leaked point/actions: %S at %d"
               remacs-test-peg-3050-events (point)))))
  ;; Successful zero-width operands still terminate after the first match.
  (let ((matcher (remacs-test-peg-3050-matcher '(+ (guard t)))))
    (with-temp-buffer
      (unless (peg-run matcher)
        (error "Zero-width one-or-more should match once"))
      (unless (= (point) (point-min))
        (error "Zero-width one-or-more moved point"))))
  ;; Failure location and expected-token reporting stay stable.
  (remacs-test-peg-3050-check-failure "b")
  (remacs-test-peg-3050-check-failure "")
  t)"#;

#[test]
fn peg_nested_one_or_more_expansion_is_polynomial_and_keeps_semantics_3050() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let source = root.join("lisp/peg.el").canonicalize().unwrap();
    let code = format!(
        "(progn (load {} nil t t) {})",
        lisp_string(source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => {
            let symbol = format!("{}", interp.prin1_to_string(&signal.symbol()));
            let data = format!("{}", interp.prin1_to_string(&signal.data()));
            panic!("{symbol} {data}")
        }
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
