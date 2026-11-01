use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-2653-check (label expected actual)
    (unless (equal actual expected)
      (error "Gnus sorted-list #2653 %S: expected %S, got %S"
             label expected actual)))
  ;; A duplicate fixnum returns the same list and leaves its tail intact.
  (let* ((input (list 1 4 8))
         (tail (cdr input))
         (result (gnus-add-to-sorted-list input 4)))
    (unless (and (eq result input)
                 (eq (cdr input) tail)
                 (equal input '(1 4 8)))
      (error "Fixnum duplicate changed the sorted list")))
  ;; Distinct bignum objects with the same numeric value are duplicates.
  (let* ((a (lsh 1 100))
         (b (lsh 1 100))
         (input (list a))
         (tail (cdr input))
         (result (gnus-add-to-sorted-list input b)))
    (unless (= a b)
      (error "Bignum fixture values are not numerically equal"))
    (when (eq a b)
      (error "Bignum fixture did not produce independent objects"))
    (unless (and (eq result input)
                 (eq (car result) a)
                 (eq (cdr input) tail)
                 (= (length result) 1))
      (error "Equal bignum was inserted a second time")))
  ;; Numeric ordering and insertion still work for values beyond fixnum range.
  (let* ((low (lsh 1 100))
         (middle (1+ low))
         (high (lsh 1 101))
         (input (list low high))
         (result (gnus-add-to-sorted-list input middle)))
    (unless (and (eq result input)
                 (= (length result) 3)
                 (= (car result) low)
                 (= (cadr result) middle)
                 (= (caddr result) high))
      (error "Bignum insertion order changed")))
  ;; Ordinary insertion before, between, and after values remains sorted.
  (remacs-2653-check 'insert-before '(2 5 9)
                     (gnus-add-to-sorted-list (list 5 9) 2))
  (let ((input (list 1 5 9)))
    (remacs-2653-check 'insert-between '(1 5 6 9)
                       (gnus-add-to-sorted-list input 6)))
  (remacs-2653-check 'insert-after '(1 5 10)
                     (gnus-add-to-sorted-list (list 1 5) 10))
  (remacs-2653-check 'empty-list '(7)
                     (gnus-add-to-sorted-list nil 7))
  ;; A quoted duplicate list is never mutated.
  (let* ((input '(1 3 5))
         (tail (cdr input))
         (result (gnus-add-to-sorted-list input 3)))
    (unless (and (eq result input)
                 (eq (cdr input) tail)
                 (equal input '(1 3 5)))
      (error "Quoted duplicate list was changed")))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

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
fn gnus_add_to_sorted_list_uses_numeric_equality_for_bignums_2653() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let range = root.join("lisp/range.el");
    let gnus_range = root.join("lisp/gnus-range.el");
    let code = format!(
        "(progn (setq noninteractive t) (load {} nil t t) (load {} nil t t) {})",
        lisp_string(range.to_str().unwrap()),
        lisp_string(gnus_range.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    assert_eq!(eval(&mut interp, &code), "t");
}
